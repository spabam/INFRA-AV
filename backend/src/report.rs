use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde_json::{Value, json};

use crate::config::AppConfig;
use crate::models::{JobStatus, QueuedJob};

/// Project evidence carries counts and bounded, project-relative detections.
/// The standalone HTML report and scanner logs stay in Antivirus.
pub fn folder_evidence(job: QueuedJob, target: &Path) -> Option<Value> {
    let result = job.result?;
    let detections: Vec<_> = result.output.iter()
        .filter_map(|line| {
            let (path, signature) = line.strip_suffix(" FOUND")?.rsplit_once(": ")?;
            let relative = Path::new(path).strip_prefix(target).ok()?;
            let relative_text = relative.to_str()?;
            if relative_text.is_empty()
                || relative_text.len() > 512
                || relative_text.chars().any(char::is_control)
                || relative.components().any(|part| !matches!(part, Component::Normal(_)))
                || signature.is_empty()
                || signature.len() > 160
                || signature.chars().any(char::is_control)
            {
                return None;
            }
            Some(json!({"path": relative_text, "signature": signature}))
        })
        .take(40)
        .collect();
    let summary = result.summary.map(|summary| {
        json!({
            "filesScanned": summary.files_scanned,
            "infectedFiles": summary.infected_files,
            "directoriesScanned": summary.directories_scanned,
            "clean": job.status == JobStatus::Done && summary.clean,
        })
    });
    Some(json!({
        "requestId": job.request_id,
        "target": target,
        "status": job.status,
        "submittedAt": job.submitted_at,
        "finishedAt": result.finished_at,
        "summary": summary,
        "detections": detections,
    }))
}

pub fn report_filename(request_id: &str) -> String {
    format!("{request_id}.html")
}

pub fn report_path(config: &AppConfig, request_id: &str) -> PathBuf {
    config.result_dir.join(report_filename(request_id))
}

pub fn read_or_render_report(config: &AppConfig, job: &QueuedJob) -> Result<(String, String)> {
    if job.result.is_none() {
        bail!("report is available only after the scan completes");
    }

    let filename = report_filename(&job.request_id);
    let path = report_path(config, &job.request_id);
    if path.exists() {
        let html = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        return Ok((filename, html));
    }

    let html = render_report(job);
    fs::write(&path, &html).with_context(|| format!("failed to write {}", path.display()))?;
    Ok((filename, html))
}

pub fn persist_report(config: &AppConfig, job: &QueuedJob) -> Result<PathBuf> {
    if job.result.is_none() {
        bail!("report is available only after the scan completes");
    }

    let html = render_report(job);
    let path = report_path(config, &job.request_id);
    fs::write(&path, html).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

fn render_report(job: &QueuedJob) -> String {
    let result = job
        .result
        .as_ref()
        .expect("terminal reports are generated only for completed jobs");

    let infected_files = result
        .summary
        .as_ref()
        .and_then(|summary| summary.infected_files);
    let has_detections = infected_files.is_some_and(|count| count > 0);
    let clean = result
        .summary
        .as_ref()
        .map(|summary| summary.clean)
        .unwrap_or(false)
        && infected_files == Some(0)
        && job.status == JobStatus::Done;

    let status_class = if job.status == JobStatus::Cancelled {
        "status-warning"
    } else if job.status == JobStatus::Failed {
        "status-failed"
    } else if clean {
        "status-clean"
    } else if has_detections {
        "status-infected"
    } else {
        "status-warning"
    };

    let status_label = if job.status == JobStatus::Cancelled {
        "Scan Stopped"
    } else if job.status == JobStatus::Failed {
        "Scan Failed"
    } else if clean {
        "No Threats Detected"
    } else if has_detections {
        "Threats Detected"
    } else {
        "Scan Completed — Summary Unavailable"
    };

    let threat_items = result
        .output
        .iter()
        .filter(|line| line.ends_with(" FOUND"))
        .map(|line| format!("<li>{}</li>", escape_html(line)))
        .collect::<Vec<_>>();

    let threat_section = if threat_items.is_empty() {
        let message = if clean {
            "No infected files were reported by INFRA-AV for this scan."
        } else if has_detections {
            "Infected files were reported, but individual detection details are unavailable. Review the raw output."
        } else {
            "No complete clean-scan result is available. An incomplete or failed scan must not be interpreted as clean."
        };
        format!("<div class=\"panel\"><h3>Detections</h3><p class=\"muted\">{message}</p></div>")
    } else {
        format!(
            "<div class=\"panel\"><h3>Detections</h3><ul class=\"threat-list\">{}</ul></div>",
            threat_items.join("")
        )
    };

    let summary = result.summary.as_ref();
    let summary_cards = [
        (
            "Status",
            status_label.to_string(),
            if clean {
                "value-good"
            } else if has_detections || job.status == JobStatus::Failed {
                "value-bad"
            } else {
                "value-neutral"
            },
        ),
        (
            "Files Scanned",
            summary
                .and_then(|value| value.files_scanned)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            "value-neutral",
        ),
        (
            "Infected Files",
            summary
                .and_then(|value| value.infected_files)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "Unknown".to_string()),
            if has_detections { "value-bad" } else if clean { "value-good" } else { "value-neutral" },
        ),
        (
            "Duration",
            summary
                .and_then(|value| value.duration.clone())
                .unwrap_or_else(|| "-".to_string()),
            "value-neutral",
        ),
    ]
    .iter()
    .map(|(label, value, class_name)| {
        format!(
            "<div class=\"metric\"><span class=\"metric-label\">{}</span><span class=\"metric-value {}\">{}</span></div>",
            escape_html(label),
            class_name,
            escape_html(value)
        )
    })
    .collect::<Vec<_>>()
    .join("");

    let metadata_rows = vec![
        ("Request ID".to_string(), job.request_id.clone()),
        ("Submitted By".to_string(), job.submitted_by.clone()),
        ("Target".to_string(), job.target.clone()),
        ("Scan Type".to_string(), job.scan_type.as_ref().to_string()),
        ("Submitted At".to_string(), job.submitted_at.to_rfc3339()),
        (
            "Started At".to_string(),
            result
                .started_at
                .map(|value| value.to_rfc3339())
                .unwrap_or_else(|| "-".to_string()),
        ),
        (
            "Finished At".to_string(),
            result
                .finished_at
                .map(|value| value.to_rfc3339())
                .unwrap_or_else(|| "-".to_string()),
        ),
        ("Exit Code".to_string(), result.exit_code.to_string()),
    ]
    .into_iter()
    .map(|(label, value)| {
        format!(
            "<tr><th>{}</th><td>{}</td></tr>",
            escape_html(&label),
            escape_html(&value)
        )
    })
    .collect::<Vec<_>>()
    .join("");

    let summary_rows = [
        (
            "Scanned Directories",
            summary
                .and_then(|value| value.directories_scanned)
                .map(|value| value.to_string()),
        ),
        (
            "Data Scanned",
            summary.and_then(|value| value.data_scanned.clone()),
        ),
        (
            "Data Read",
            summary.and_then(|value| value.data_read.clone()),
        ),
        (
            "Start Date",
            summary.and_then(|value| value.start_date.clone()),
        ),
        ("End Date", summary.and_then(|value| value.end_date.clone())),
    ]
    .iter()
    .filter_map(|(label, value)| {
        value.as_ref().map(|actual| {
            format!(
                "<tr><th>{}</th><td>{}</td></tr>",
                escape_html(label),
                escape_html(actual)
            )
        })
    })
    .collect::<Vec<_>>()
    .join("");

    let error_block = result
        .error_message
        .as_ref()
        .map(|message| {
            format!(
                "<div class=\"panel warning\"><h3>Execution Note</h3><p>{}</p></div>",
                escape_html(message)
            )
        })
        .unwrap_or_default();

    let raw_output = escape_html(&result.output.join("\n"));
    let command_preview = escape_html(&job.command_preview);
    let generated_at = escape_html(&Utc::now().to_rfc3339());

    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>INFRA-AV Scan Report</title>
    <style>
      :root {{
        color-scheme: light;
        --bg: #f5f7fb;
        --panel: #ffffff;
        --line: #d9e1ec;
        --ink: #163046;
        --muted: #5d7285;
        --good: #1b7f57;
        --bad: #b93832;
        --warn: #9a6400;
        --accent: #0b4f71;
      }}
      * {{ box-sizing: border-box; }}
      body {{
        margin: 0;
        font-family: "Segoe UI", Tahoma, Geneva, Verdana, sans-serif;
        background: linear-gradient(180deg, #f2f6fb 0%, #edf2f7 100%);
        color: var(--ink);
      }}
      .page {{
        max-width: 1080px;
        margin: 0 auto;
        padding: 32px 24px 48px;
      }}
      .hero {{
        display: grid;
        gap: 18px;
        padding: 28px;
        border-radius: 24px;
        border: 1px solid rgba(11, 79, 113, 0.12);
        background:
          radial-gradient(circle at top right, rgba(11, 79, 113, 0.14), transparent 38%),
          linear-gradient(135deg, #123047, #0c567b);
        color: #f5fbff;
        box-shadow: 0 24px 60px rgba(11, 49, 80, 0.18);
      }}
      .eyebrow {{
        font-size: 12px;
        letter-spacing: 0.16em;
        text-transform: uppercase;
        opacity: 0.8;
      }}
      h1 {{
        margin: 0;
        font-size: 30px;
        line-height: 1.15;
      }}
      .subtitle {{
        margin: 0;
        max-width: 60ch;
        color: rgba(245, 251, 255, 0.82);
      }}
      .status-badge {{
        display: inline-flex;
        align-items: center;
        gap: 10px;
        width: fit-content;
        padding: 10px 16px;
        border-radius: 999px;
        font-weight: 700;
        font-size: 14px;
        background: rgba(255, 255, 255, 0.12);
        border: 1px solid rgba(255, 255, 255, 0.18);
      }}
      .status-badge::before {{
        content: "";
        width: 10px;
        height: 10px;
        border-radius: 999px;
        background: currentColor;
      }}
      .status-clean {{ color: #92ffd6; }}
      .status-infected {{ color: #ffaea9; }}
      .status-failed {{ color: #ffd39a; }}
      .status-warning {{ color: #ffd39a; }}
      .grid {{
        display: grid;
        gap: 18px;
        margin-top: 24px;
      }}
      .metrics {{
        grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      }}
      .columns {{
        grid-template-columns: 1.1fr 0.9fr;
      }}
      .panel {{
        background: var(--panel);
        border: 1px solid var(--line);
        border-radius: 20px;
        padding: 22px;
        box-shadow: 0 18px 40px rgba(15, 35, 52, 0.06);
      }}
      .panel.warning {{
        border-color: rgba(154, 100, 0, 0.28);
        background: #fff8e9;
      }}
      .metric {{
        padding: 20px;
        border-radius: 18px;
        border: 1px solid var(--line);
        background: rgba(255, 255, 255, 0.72);
      }}
      .metric-label {{
        display: block;
        font-size: 12px;
        letter-spacing: 0.08em;
        text-transform: uppercase;
        color: var(--muted);
      }}
      .metric-value {{
        display: block;
        margin-top: 10px;
        font-size: 26px;
        font-weight: 800;
      }}
      .value-good {{ color: var(--good); }}
      .value-bad {{ color: var(--bad); }}
      .value-neutral {{ color: var(--ink); }}
      h2, h3 {{
        margin: 0 0 14px;
        font-size: 18px;
      }}
      table {{
        width: 100%;
        border-collapse: collapse;
      }}
      th, td {{
        padding: 10px 0;
        border-bottom: 1px solid var(--line);
        text-align: left;
        vertical-align: top;
      }}
      th {{
        width: 34%;
        color: var(--muted);
        font-weight: 600;
      }}
      td {{
        font-family: "Courier New", Courier, monospace;
        word-break: break-word;
      }}
      .threat-list {{
        margin: 0;
        padding-left: 20px;
      }}
      .threat-list li {{
        margin-bottom: 8px;
        color: var(--bad);
        font-weight: 700;
      }}
      .muted {{
        color: var(--muted);
      }}
      pre {{
        margin: 0;
        padding: 18px;
        overflow: auto;
        border-radius: 16px;
        border: 1px solid #0f1a24;
        background: #0f1720;
        color: #f4f8fb;
        font-size: 12px;
        line-height: 1.55;
        white-space: pre-wrap;
        word-break: break-word;
      }}
      .footer {{
        margin-top: 20px;
        color: var(--muted);
        font-size: 12px;
        text-align: right;
      }}
      @media (max-width: 900px) {{
        .columns {{
          grid-template-columns: 1fr;
        }}
        .page {{
          padding: 20px 14px 32px;
        }}
        .hero {{
          padding: 22px;
        }}
      }}
    </style>
  </head>
  <body>
    <main class="page">
      <section class="hero">
        <div class="eyebrow">INFRA-AV</div>
        <div>
          <h1>INFRA-AV Scan Report</h1>
          <p class="subtitle">Structured scan results for request {request_id}.</p>
        </div>
        <div class="status-badge {status_class}">{status_label}</div>
      </section>

      <section class="grid metrics">{summary_cards}</section>

      <section class="grid columns">
        <div class="panel">
          <h2>Scan Metadata</h2>
          <table>{metadata_rows}</table>
        </div>
        <div class="panel">
          <h2>Scan Summary</h2>
          <table>{summary_rows}</table>
        </div>
      </section>

      <section class="grid columns">
        {threat_section}
        <div class="panel">
          <h3>Command Preview</h3>
          <pre>{command_preview}</pre>
        </div>
      </section>

      {error_block}

      <section class="panel">
        <h2>Raw INFRA-AV Output</h2>
        <pre>{raw_output}</pre>
      </section>

      <div class="footer">Generated by INFRA-AV at {generated_at}</div>
    </main>
  </body>
</html>"#,
        request_id = escape_html(&job.request_id),
        status_class = status_class,
        status_label = escape_html(status_label),
        summary_cards = summary_cards,
        metadata_rows = metadata_rows,
        summary_rows = summary_rows,
        threat_section = threat_section,
        command_preview = command_preview,
        error_block = error_block,
        raw_output = raw_output,
        generated_at = generated_at,
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

trait ScanTypeLabel {
    fn as_ref(&self) -> &str;
}

impl ScanTypeLabel for crate::models::ScanType {
    fn as_ref(&self) -> &str {
        match self {
            crate::models::ScanType::File => "file",
            crate::models::ScanType::Directory => "directory",
            crate::models::ScanType::Device => "device",
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::models::{ClamAvOptions, JobResult, JobStatus, QueuedJob, ScanSummary, ScanType};

    use super::render_report;

    #[test]
    fn rendered_report_has_infra_av_title_and_escapes_html() {
        let job = QueuedJob {
            schema_version: 1,
            request_id: "req-1".to_string(),
            submitted_at: Utc::now(),
            submitted_by: "admin".to_string(),
            target: "/tmp/test-<file>.txt".to_string(),
            scan_type: ScanType::File,
            options: ClamAvOptions::default(),
            status: JobStatus::Done,
            status_updated_at: Utc::now(),
            filename: "req-1.av.json".to_string(),
            command_preview: "clamscan /tmp/test-<file>.txt".to_string(),
            server_metadata: Default::default(),
            execution: None,
            result: Some(JobResult {
                started_at: Some(Utc::now()),
                finished_at: Some(Utc::now()),
                exit_code: 0,
                output: vec!["/tmp/test-<file>.txt: OK".to_string()],
                summary: Some(ScanSummary {
                    files_scanned: Some(1),
                    infected_files: Some(0),
                    directories_scanned: Some(0),
                    data_scanned: Some("0.01 MB".to_string()),
                    data_read: Some("0.01 MB".to_string()),
                    duration: Some("1 sec".to_string()),
                    start_date: Some("today".to_string()),
                    end_date: Some("today".to_string()),
                    clean: true,
                }),
                error_message: None,
            }),
        };

        let report = render_report(&job);
        assert!(report.contains("<title>INFRA-AV Scan Report</title>"));
        assert!(report.contains("&lt;file&gt;"));
        assert!(report.contains("No Threats Detected"));
        for status in [JobStatus::Done, JobStatus::Failed, JobStatus::Cancelled] {
            let mut incomplete = job.clone();
            incomplete.status = status;
            incomplete.result.as_mut().unwrap().summary = None;
            let report = render_report(&incomplete);
            assert!(!report.contains("No Threats Detected"));
            assert!(!report.contains("No infected files were reported"));
            assert!(report.contains("value-neutral\">Unknown</span>"));
            assert!(report.contains("must not be interpreted as clean"));
        }
    }
}
