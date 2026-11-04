use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use uuid::Uuid;

use crate::clamav::{CommandSpec, build_command};
use crate::config::AppConfig;
use crate::models::{
    JobStatus, QueueCounts, QueuedJob, ScanActionRequest, ScanListItem, ScanSubmitResponse,
};

pub fn submit_job(
    config: &AppConfig,
    username: &str,
    request: ScanActionRequest,
) -> Result<ScanSubmitResponse> {
    if request.action_type != "av" {
        bail!("unsupported action type {}", request.action_type);
    }

    let request_id = generate_request_id(username);
    let filename = format!("{request_id}.av.json");

    let command = build_command(
        config,
        &request_id,
        username,
        &request.target,
        request.scan_type,
        &request.options,
    )?;

    let now = Utc::now();
    let mut metadata = BTreeMap::new();
    metadata.insert("client_timestamp".to_string(), request.timestamp);
    metadata.insert("client_user".to_string(), request.user);

    let job = QueuedJob {
        schema_version: 1,
        request_id: request_id.clone(),
        submitted_at: now,
        submitted_by: username.to_string(),
        target: request.target,
        scan_type: request.scan_type,
        options: request.options,
        status: JobStatus::Pending,
        status_updated_at: now,
        filename: filename.clone(),
        command_preview: command.preview.clone(),
        server_metadata: metadata,
        execution: None,
        result: None,
    };

    let path = config.pending_dir.join(&filename);
    write_json_atomic(&path, &job)?;

    Ok(ScanSubmitResponse {
        request_id,
        filename,
        status: JobStatus::Pending,
        command_preview: command.preview,
    })
}

pub fn take_next_pending(config: &AppConfig) -> Result<Option<(QueuedJob, PathBuf)>> {
    let mut files = fs::read_dir(&config.pending_dir)
        .with_context(|| format!("failed to read {}", config.pending_dir.display()))?
        .filter_map(|entry| entry.ok().map(|value| value.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();
    files.sort();

    for pending_path in files {
        let processing_path = config.processing_dir.join(
            pending_path
                .file_name()
                .context("pending file has no filename")?,
        );
        if fs::rename(&pending_path, &processing_path).is_ok() {
            let job = read_job(&processing_path)?;
            return Ok(Some((job, processing_path)));
        }
    }

    Ok(None)
}

pub fn load_job(config: &AppConfig, request_id: &str) -> Result<Option<QueuedJob>> {
    Ok(load_job_with_path(config, request_id)?.map(|(job, _)| job))
}

pub fn load_job_with_path(config: &AppConfig, request_id: &str) -> Result<Option<(QueuedJob, PathBuf)>> {
    let locations = [
        &config.pending_dir,
        &config.processing_dir,
        &config.done_dir,
        &config.failed_dir,
    ];

    for directory in locations {
        let path = directory.join(format!("{request_id}.av.json"));
        if path.exists() {
            return Ok(Some((read_job(&path)?, path)));
        }
    }

    Ok(None)
}

pub fn list_jobs(config: &AppConfig, limit: usize) -> Result<Vec<ScanListItem>> {
    let locations = [
        &config.pending_dir,
        &config.processing_dir,
        &config.done_dir,
        &config.failed_dir,
    ];
    let mut items = Vec::new();

    for directory in locations {
        if !directory.exists() {
            continue;
        }
        for path in fs::read_dir(directory)
            .with_context(|| format!("failed to read {}", directory.display()))?
            .filter_map(|entry| entry.ok().map(|value| value.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        {
            let job = read_job(&path)?;
            items.push(ScanListItem {
                request_id: job.request_id,
                status: job.status,
                target: job.target,
                submitted_by: job.submitted_by,
                submitted_at: job.submitted_at,
            });
        }
    }

    items.sort_by(|left, right| right.submitted_at.cmp(&left.submitted_at));
    items.truncate(limit);
    Ok(items)
}

pub fn write_processing_job(path: &Path, job: &QueuedJob) -> Result<()> {
    write_json_atomic(path, job)
}

pub fn write_job(path: &Path, job: &QueuedJob) -> Result<()> {
    write_json_atomic(path, job)
}

pub fn finalize_job(
    config: &AppConfig,
    processing_path: &Path,
    job: &QueuedJob,
) -> Result<PathBuf> {
    let final_dir = match job.status {
        JobStatus::Done => &config.done_dir,
        JobStatus::Failed | JobStatus::Cancelled => &config.failed_dir,
        _ => bail!("cannot finalize job without terminal status"),
    };

    let final_path = final_dir.join(&job.filename);
    write_json_atomic(&final_path, job)?;
    fs::remove_file(processing_path).ok();
    Ok(final_path)
}

pub fn queue_counts(config: &AppConfig) -> Result<QueueCounts> {
    Ok(QueueCounts {
        pending: count_json_files(&config.pending_dir)?,
        processing: count_json_files(&config.processing_dir)?,
        done: count_json_files(&config.done_dir)?,
        failed: count_json_files(&config.failed_dir)?,
    })
}

pub fn rebuild_command(config: &AppConfig, job: &QueuedJob) -> Result<CommandSpec> {
    build_command(
        config,
        &job.request_id,
        &job.submitted_by,
        &job.target,
        job.scan_type,
        &job.options,
    )
}

pub fn cancel_pending_job(config: &AppConfig, request_id: &str) -> Result<Option<QueuedJob>> {
    let pending_path = config.pending_dir.join(format!("{request_id}.av.json"));
    if !pending_path.exists() {
        return Ok(None);
    }

    let mut job = read_job(&pending_path)?;
    job.status = JobStatus::Cancelled;
    job.status_updated_at = Utc::now();
    job.execution = None;
    job.result = Some(crate::models::JobResult {
        started_at: None,
        finished_at: Some(Utc::now()),
        exit_code: 130,
        output: vec!["Scan request cancelled before execution.".to_string()],
        summary: None,
        error_message: Some("scan stopped by user before execution started".to_string()),
    });

    let final_path = config.failed_dir.join(&job.filename);
    write_json_atomic(&final_path, &job)?;
    fs::remove_file(&pending_path).ok();
    Ok(Some(job))
}

fn generate_request_id(username: &str) -> String {
    let ts = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let suffix = Uuid::new_v4().simple().to_string();
    let safe_username = username
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect::<String>();
    format!("{ts}.{safe_username}.{}.av", &suffix[..8])
}

pub fn read_job(path: &Path) -> Result<QueuedJob> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str::<QueuedJob>(&content)
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn write_json_atomic(path: &Path, job: &QueuedJob) -> Result<()> {
    let content = serde_json::to_vec_pretty(job).context("failed to serialize job")?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, content)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

fn count_json_files(directory: &Path) -> Result<usize> {
    Ok(fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|value| value.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .count())
}
