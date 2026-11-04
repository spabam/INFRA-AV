use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::os::unix::process::ExitStatusExt;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::clamav::parse_summary;
use crate::config::AppConfig;
use crate::models::{JobExecution, JobResult, JobStatus};
use crate::{queue, report};

pub fn run(config: AppConfig, once: bool, interval: Duration) -> Result<()> {
    if once {
        let _ = process_next(&config)?;
        return Ok(());
    }

    loop {
        let _ = process_next(&config)?;
        thread::sleep(interval);
    }
}

fn process_next(config: &AppConfig) -> Result<bool> {
    let Some((mut job, processing_path)) = queue::take_next_pending(config)? else {
        return Ok(false);
    };

    let command = queue::rebuild_command(config, &job)?;
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
    let child = Command::new(&command.program)
        .args(&command.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to execute {}", command.program))?;
    if let Some(execution) = job.execution.as_mut() {
        execution.child_pid = Some(child.id());
    }
    queue::write_processing_job(&processing_path, &job)?;

    let output = child
        .wait_with_output()
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

    let persisted_job = queue::read_job(&processing_path).unwrap_or_else(|_| job.clone());
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
    if let Err(error) = report::persist_report(config, &job) {
        eprintln!(
            "failed to persist HTML report for {}: {error:#}",
            job.request_id
        );
    }
    Ok(true)
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
            allowed_scan_roots: vec![root.join("targets")],
            allowed_write_roots: vec![root.join("data/quarantine")],
            allowed_database_roots: vec![PathBuf::from("/var/lib/clamav")],
            allow_device_scans: false,
            cors_origins: vec![],
        }
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
}
