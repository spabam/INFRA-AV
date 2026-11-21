use std::fs;
use std::io::Read;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;

use crate::clamav::parse_summary;
use crate::config::AppConfig;
use crate::models::{JobExecution, JobResult, JobStatus, QueuedJob};
use crate::{queue, report};

pub fn run(config: AppConfig, once: bool, interval: Duration) -> Result<()> {
    loop {
        {
            let _lease = if config.split_worker {
                let lease = queue::lock(&config, ".worker.lock")?;
                recover_interrupted(&config)?;
                Some(lease)
            } else {
                None
            };
            process_next(&config)?;
        }
        if once {
            return Ok(());
        }
        thread::sleep(interval);
    }
}

fn recover_interrupted(config: &AppConfig) -> Result<()> {
    let _control = queue::lock(config, ".control.lock")?;
    for entry in fs::read_dir(&config.processing_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let job = queue::read_job(&path)?;
        if config.done_dir.join(entry.file_name()).is_file()
            || config.failed_dir.join(entry.file_name()).is_file()
        {
            fs::remove_file(&path)?;
        } else {
            finish_start_failure(
                config,
                &path,
                job,
                "Scan interrupted by worker shutdown. Verify the target state before retrying.".to_string(),
            )?;
        }
    }
    Ok(())
}

fn process_next(config: &AppConfig) -> Result<bool> {
    let Some((mut job, processing_path)) = queue::take_next_pending(config)? else {
        return Ok(false);
    };

    let command = match queue::rebuild_command(config, &job) {
        Ok(command) => command,
        Err(error) => {
            finish_start_failure(
                config,
                &processing_path,
                job,
                format!("scan validation failed before execution: {error:#}"),
            )?;
            return Ok(true);
        }
    };
    let control = queue::lock(config, ".control.lock")?;
    job.status = JobStatus::Processing;
    job.status_updated_at = Utc::now();
    job.execution = Some(JobExecution {
        worker_pid: std::process::id(),
        child_pid: None,
        started_at: Utc::now(),
        paused: false,
        cancel_requested: false,
    });
    queue::write_processing_job(&processing_path, &job)?;

    let started_at = job
        .execution
        .as_ref()
        .map(|value| value.started_at)
        .unwrap_or_else(Utc::now);
    let child = match Command::new(&command.program)
        .args(&command.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            finish_start_failure(
                config,
                &processing_path,
                job,
                format!("failed to execute {}: {error}", command.program),
            )?;
            return Ok(true);
        }
    };
    if let Some(execution) = job.execution.as_mut() {
        execution.child_pid = Some(child.id());
    }
    queue::write_processing_job(&processing_path, &job)?;
    drop(control);

    let output = if config.split_worker {
        controlled_output(child, &processing_path)
    } else {
        child.wait_with_output().map_err(Into::into)
    }
    .with_context(|| format!("failed to wait for {}", command.program))?;
    let finished_at = Utc::now();

    let mut combined = Vec::new();
    combined.extend(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string),
    );
    combined.extend(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .map(str::to_string),
    );

    if let Some(parent) = command.log_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    write_lines(&command.log_path, &combined).ok();

    let control = queue::lock(config, ".control.lock")?;
    let persisted_job = queue::read_job(&processing_path)?;
    let cancel_requested = persisted_job
        .execution
        .as_ref()
        .map(|value| value.cancel_requested)
        .unwrap_or(false);
    let signal = output.status.signal();
    let exit_code = output.status.code().unwrap_or_else(|| {
        if cancel_requested {
            130
        } else {
            signal.map(|value| 128 + value).unwrap_or(2)
        }
    });
    let summary = parse_summary(&combined, exit_code);

    job = persisted_job;
    job.status = if cancel_requested {
        JobStatus::Cancelled
    } else if exit_code <= 1 {
        JobStatus::Done
    } else {
        JobStatus::Failed
    };
    job.status_updated_at = finished_at;
    job.execution = None;
    job.result = Some(JobResult {
        started_at: Some(started_at),
        finished_at: Some(finished_at),
        exit_code,
        output: combined,
        summary,
        error_message: if cancel_requested {
            Some("scan stopped by user".to_string())
        } else {
            (exit_code > 1).then(|| "clamscan returned an error".to_string())
        },
    });

    queue::finalize_job(config, &processing_path, &job)?;
    drop(control);
    if let Err(error) = report::persist_report(config, &job) {
        eprintln!(
            "failed to persist HTML report for {}: {error:#}",
            job.request_id
        );
    }
    Ok(true)
}

fn controlled_output(mut child: Child, processing_path: &Path) -> Result<Output> {
    let mut stdout = child.stdout.take().context("missing scanner stdout")?;
    let mut stderr = child.stderr.take().context("missing scanner stderr")?;
    // Only the process that owns the child interprets its PID. The PID cannot
    // be reused between a live try_wait and a signal until this owner reaps it.
    thread::scope(|scope| -> Result<Output> {
        let stdout = scope.spawn(move || {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).map(|_| bytes)
        });
        let stderr = scope.spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).map(|_| bytes)
        });
        let mut paused = false;
        let mut cancellation: Option<Instant> = None;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if let Some(execution) = queue::read_job(processing_path)
                .ok()
                .and_then(|job| job.execution)
            {
                let pid = child.id() as libc::pid_t;
                if execution.cancel_requested {
                    if let Some(when) = cancellation {
                        if when.elapsed() >= Duration::from_secs(2) {
                            let _ = child.kill();
                        }
                    } else {
                        unsafe {
                            libc::kill(pid, libc::SIGCONT);
                            libc::kill(pid, libc::SIGTERM);
                        }
                        cancellation = Some(Instant::now());
                    }
                } else if execution.paused != paused {
                    let signal = if execution.paused {
                        libc::SIGSTOP
                    } else {
                        libc::SIGCONT
                    };
                    if unsafe { libc::kill(pid, signal) } == 0 {
                        paused = execution.paused;
                    }
                }
            }
            thread::sleep(Duration::from_millis(100));
        };
        Ok(Output {
            status,
            stdout: stdout
                .join()
                .map_err(|_| anyhow::anyhow!("scanner stdout reader panicked"))??,
            stderr: stderr
                .join()
                .map_err(|_| anyhow::anyhow!("scanner stderr reader panicked"))??,
        })
    })
}

/// A rejected target or unavailable executable is terminal, not a running scan.
/// Persist it before moving on so the queue worker does not silently strand jobs
/// or replay an operation after a service restart.
fn finish_start_failure(
    config: &AppConfig,
    path: &Path,
    mut job: QueuedJob,
    message: String,
) -> Result<()> {
    let finished_at = Utc::now();
    job.status = JobStatus::Failed;
    job.status_updated_at = finished_at;
    job.execution = None;
    job.result = Some(JobResult {
        started_at: None,
        finished_at: Some(finished_at),
        exit_code: 2,
        output: vec![message.clone()],
        summary: None,
        error_message: Some(message),
    });
    queue::finalize_job(config, path, &job)?;
    if let Err(error) = report::persist_report(config, &job) {
        eprintln!(
            "failed to persist HTML report for {}: {error:#}",
            job.request_id
        );
    }
    Ok(())
}

fn write_lines(path: &Path, lines: &[String]) -> Result<()> {
    fs::write(path, lines.join("\n")).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;

    use uuid::Uuid;

    use crate::config::AppConfig;
    use crate::models::{ClamAvOptions, JobStatus, ScanActionRequest, ScanType};

    use super::run;

    fn test_config(root: &PathBuf) -> AppConfig {
        AppConfig {
            host: "127.0.0.1".to_string(),
            port: 5000,
            jwt_secret: "test-secret".to_string(),
            broker_secret: None,
            broker_secret_previous: None,
            data_dir: root.join("data"),
            pending_dir: root.join("data/action/pending"),
            processing_dir: root.join("data/action/processing"),
            done_dir: root.join("data/action/done"),
            failed_dir: root.join("data/action/failed"),
            result_dir: root.join("data/result"),
            log_dir: root.join("data/log"),
            quarantine_dir: root.join("data/quarantine"),
            temp_dir: root.join("data/temp"),
            pam_service: "infra-av".to_string(),
            default_role: "admin".to_string(),
            users_file: root.join("config/users.json"),
            clamscan_bin: "/usr/bin/clamscan".to_string(),
            freshclam_bin: "/usr/bin/freshclam".to_string(),
            split_worker: false,
            read_only_scan_roots: vec![],
            allowed_scan_roots: vec![root.join("targets")],
            allowed_write_roots: vec![root.join("data/quarantine")],
            allowed_database_roots: vec![PathBuf::from("/var/lib/clamav")],
            allow_device_scans: false,
            cors_origins: vec![],
        }
    }

    #[test]
    fn read_only_scan_targets_require_direct_paths_but_local_symlinks_work() {
        use std::os::unix::fs::symlink;

        let (root, mut config, local) = fixture("protected-target");
        let protected = root.join("targets/drive");
        fs::create_dir_all(&protected).unwrap();
        config.read_only_scan_roots.push(protected.clone());
        let stored = protected.join("stored.txt");
        let inside_alias = protected.join("inside-alias.txt");
        let escape_alias = protected.join("escape-alias.txt");
        let local_alias = local.with_file_name("local-alias.txt");
        fs::write(&stored, "benign stored content").unwrap();
        symlink(&stored, &inside_alias).unwrap();
        symlink(&local, &escape_alias).unwrap();
        symlink(&local, &local_alias).unwrap();

        let validate = |path: &std::path::Path| {
            crate::clamav::build_command(
                &config,
                "fixture",
                "tester",
                path.to_str().unwrap(),
                ScanType::File,
                &ClamAvOptions::default(),
            )
        };
        assert!(validate(&stored).is_ok());
        assert!(validate(&local_alias).is_ok());
        for alias in [inside_alias, escape_alias] {
            let result = validate(&alias);
            assert!(matches!(result, Err(error) if error.to_string().contains("require a direct path")));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn worker_executes_real_clamscan_binary() {
        let root = std::env::temp_dir().join(format!("infra-av-worker-test-{}", Uuid::new_v4()));
        let config = test_config(&root);
        config
            .ensure_directories()
            .expect("test directories should be created");
        fs::create_dir_all(root.join("targets")).expect("target directory should exist");

        let target_file = root.join("targets/real-clamscan-check.json");
        fs::write(&target_file, "{\"status\":\"clean\"}")
            .expect("test target file should be created");

        let submit = crate::queue::submit_job(
            &config,
            "tester",
            ScanActionRequest {
                timestamp: chrono::Utc::now().to_rfc3339(),
                user: "tester".to_string(),
                action_type: "av".to_string(),
                target: target_file.display().to_string(),
                scan_type: ScanType::File,
                options: ClamAvOptions::default(),
            },
        )
        .expect("job should be queued");

        run(config.clone(), true, Duration::ZERO).expect("worker should execute clamscan");

        let job = crate::queue::load_job(&config, &submit.request_id)
            .expect("job should be readable")
            .expect("job should exist");
        assert_eq!(job.status, JobStatus::Done);
        let result = job.result.expect("job should have a result");
        assert_eq!(result.exit_code, 0);
        assert!(
            result
                .output
                .iter()
                .any(|line| line.contains(&target_file.display().to_string())),
            "clamscan output should mention the real scanned file"
        );

        let log_path = config
            .log_dir
            .join("tester")
            .join(format!("{}.clamscan.log", submit.request_id));
        assert!(log_path.exists(), "clamscan log file should be written");

        fs::remove_dir_all(root).ok();
    }

    fn fixture(name: &str) -> (PathBuf, AppConfig, PathBuf) {
        let root = std::env::temp_dir().join(format!("infra-av-{name}-{}", Uuid::new_v4()));
        let config = test_config(&root);
        config.ensure_directories().unwrap();
        fs::create_dir_all(root.join("targets")).unwrap();
        let target = root.join("targets/fixture.txt");
        fs::write(&target, "harmless fixture\n").unwrap();
        (root, config, target)
    }

    fn submit_fixture(
        config: &AppConfig,
        target: &std::path::Path,
        options: ClamAvOptions,
    ) -> String {
        crate::queue::submit_job(
            config,
            "tester",
            ScanActionRequest {
                timestamp: chrono::Utc::now().to_rfc3339(),
                user: "tester".into(),
                action_type: "av".into(),
                target: target.display().to_string(),
                scan_type: ScanType::File,
                options,
            },
        )
        .unwrap()
        .request_id
    }

    #[test]
    fn folder_report_evidence_respects_ownership_and_matches_only_the_project() {
        let (root, config, fixture_file) = fixture("folder-evidence");
        let target = fixture_file.parent().unwrap();
        let submitted_at = chrono::Utc::now();
        let finish = |owner: &str, seconds: i64| {
            let submitted = crate::queue::submit_job(
                &config,
                owner,
                ScanActionRequest {
                    timestamp: submitted_at.to_rfc3339(),
                    user: owner.into(),
                    action_type: "av".into(),
                    target: target.display().to_string(),
                    scan_type: ScanType::Directory,
                    options: ClamAvOptions::default(),
                },
            ).unwrap();
            let (mut job, processing) = crate::queue::take_next_pending(&config).unwrap().unwrap();
            assert_eq!(job.request_id, submitted.request_id);
            job.status = JobStatus::Done;
            job.result = Some(crate::models::JobResult {
                started_at: Some(submitted_at),
                finished_at: Some(submitted_at + chrono::Duration::seconds(seconds)),
                exit_code: 1,
                output: vec![
                    format!("{}: Example.Signature FOUND", fixture_file.display()),
                    "/other/project.txt: Outside.Signature FOUND".into(),
                ],
                summary: None,
                error_message: None,
            });
            crate::queue::finalize_job(&config, &processing, &job).unwrap();
            submitted.request_id
        };
        let own_id = finish("owner", 1);
        let newest_id = finish("other", 2);
        let owned = crate::queue::latest_folder_report(&config, target, Some("owner")).unwrap().unwrap();
        assert_eq!(owned.request_id, own_id);
        let evidence = crate::report::folder_evidence(owned, target).unwrap();
        assert_eq!(evidence["detections"], serde_json::json!([{"path": "fixture.txt", "signature": "Example.Signature"}]));
        assert_eq!(crate::queue::latest_folder_report(&config, target, None).unwrap().unwrap().request_id, newest_id);
        assert!(crate::queue::latest_folder_report(&config, target, Some("stranger")).unwrap().is_none());
        assert!(crate::queue::latest_folder_report(&config, &target.join("another-project"), None).unwrap().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unavailable_executable_is_a_terminal_failure_with_a_report() {
        let (root, mut config, target) = fixture("missing-executable");
        config.clamscan_bin = root.join("missing-clamscan").display().to_string();
        let id = submit_fixture(&config, &target, ClamAvOptions::default());
        run(config.clone(), true, Duration::ZERO).unwrap();
        let job = crate::queue::load_job(&config, &id).unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert!(job.execution.is_none());
        let result = job.result.unwrap();
        assert!(result.error_message.unwrap().contains("failed to execute"));
        assert!(
            result.summary.is_none(),
            "execution failure must not claim a clean scan"
        );
        assert!(crate::report::report_path(&config, &id).is_file());
        let counts = crate::queue::queue_counts(&config).unwrap();
        assert_eq!(counts.processing, 0);
        assert_eq!(counts.failed, 1);
        run(config.clone(), true, Duration::ZERO).unwrap();
        assert_eq!(
            crate::queue::queue_counts(&config).unwrap().failed,
            1,
            "failed commands are never retried automatically"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn target_removed_after_submission_does_not_strand_the_queue() {
        let (root, config, target) = fixture("missing-target");
        let id = submit_fixture(&config, &target, ClamAvOptions::default());
        fs::remove_file(target).unwrap();
        run(config.clone(), true, Duration::ZERO).unwrap();
        let job = crate::queue::load_job(&config, &id).unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert!(
            job.result
                .unwrap()
                .error_message
                .unwrap()
                .contains("validation failed before execution")
        );
        assert_eq!(crate::queue::queue_counts(&config).unwrap().processing, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn real_clamav_fixture_database_covers_clean_detection_quarantine_and_removal() {
        let (root, mut config, target) = fixture("safe-signature");
        let database = root.join("fixture-database");
        fs::create_dir_all(&database).unwrap();
        let fixture_text = "VANTAGE harmless AV regression fixture";
        let signature = fixture_text
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        fs::write(
            database.join("fixture.ndb"),
            format!("VantageRegressionFixture:0:*:{signature}\n"),
        )
        .unwrap();
        config.allowed_database_roots = vec![database.clone()];
        for action in ["clean", "detect", "quarantine", "remove"] {
            let infected = action != "clean";
            fs::write(
                &target,
                if infected {
                    fixture_text
                } else {
                    "ordinary clean fixture"
                },
            )
            .unwrap();
            let mut options = ClamAvOptions::default();
            options.database = database.display().to_string();
            if action == "quarantine" {
                options.move_path = config.quarantine_dir.display().to_string();
            }
            if action == "remove" {
                options.remove = true;
            }
            let id = submit_fixture(&config, &target, options);
            run(config.clone(), true, Duration::ZERO).unwrap();
            let job = crate::queue::load_job(&config, &id).unwrap().unwrap();
            assert_eq!(job.status, JobStatus::Done, "{action}: {:?}", job.result);
            let result = job.result.unwrap();
            assert_eq!(
                result.exit_code,
                if infected { 1 } else { 0 },
                "{action}: {:?}",
                result.output
            );
            let summary = result.summary.unwrap();
            assert_eq!(summary.clean, !infected);
            assert_eq!(summary.infected_files, Some(if infected { 1 } else { 0 }));
            assert!(crate::report::report_path(&config, &id).is_file());
            if action == "quarantine" {
                assert!(!target.exists());
                assert_eq!(
                    fs::read_to_string(config.quarantine_dir.join("fixture.txt")).unwrap(),
                    fixture_text
                );
            } else if action == "remove" {
                assert!(!target.exists());
            } else {
                assert!(target.exists());
            }
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pending_cancellation_and_worker_claim_have_only_one_winner() {
        let (root, config, target) = fixture("cancel-race");
        for _ in 0..32 {
            let id = submit_fixture(&config, &target, ClamAvOptions::default());
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
            let cancel_config = config.clone();
            let cancel_id = id.clone();
            let cancel_barrier = barrier.clone();
            let cancel = std::thread::spawn(move || {
                cancel_barrier.wait();
                crate::queue::cancel_pending_job(&cancel_config, &cancel_id).unwrap()
            });
            let worker_config = config.clone();
            let worker_barrier = barrier.clone();
            let worker = std::thread::spawn(move || {
                worker_barrier.wait();
                crate::queue::take_next_pending(&worker_config).unwrap()
            });
            barrier.wait();
            let cancelled = cancel.join().unwrap();
            let claimed = worker.join().unwrap();
            assert_ne!(
                cancelled.is_some(),
                claimed.is_some(),
                "both paths claimed a single request"
            );
            if let Some((_, path)) = claimed {
                fs::remove_file(path).unwrap();
            }
            if let Some(job) = cancelled {
                fs::remove_file(config.failed_dir.join(job.filename)).unwrap();
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
