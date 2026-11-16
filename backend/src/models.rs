use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEnvelope {
    pub action: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T>
where
    T: Serialize,
{
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    pub fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(ApiError {
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub username: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub product_version: String,
    pub backend_version: String,
    pub clamscan: CommandProbe,
    pub freshclam: CommandProbe,
    pub queue: QueueCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandProbe {
    pub ok: bool,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureUpdateResponse {
    pub exit_code: i32,
    pub output: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSubmitResponse {
    pub request_id: String,
    pub filename: String,
    pub status: JobStatus,
    pub command_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatusRequest {
    #[serde(rename = "requestId")]
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanBrowseRequest {
    #[serde(rename = "scanType")]
    pub scan_type: ScanType,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanBrowseResponse {
    pub scan_type: ScanType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_path: Option<String>,
    pub can_select_current: bool,
    pub roots: Vec<String>,
    pub entries: Vec<ScanBrowseEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanBrowseEntry {
    pub name: String,
    pub path: String,
    pub kind: ScanBrowseEntryKind,
    pub selectable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScanBrowseEntryKind {
    Root,
    Directory,
    File,
    Device,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanControlRequest {
    #[serde(rename = "requestId")]
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanControlResponse {
    pub request_id: String,
    pub status: JobStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatusResponse {
    pub request_id: String,
    pub status: JobStatus,
    pub target: String,
    pub submitted_by: String,
    pub submitted_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    pub command_preview: String,
    pub output: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ScanSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub report_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReportResponse {
    pub request_id: String,
    pub filename: String,
    pub html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanListRequest {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanListResponse {
    pub items: Vec<ScanListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanListItem {
    pub request_id: String,
    pub status: JobStatus,
    pub target: String,
    pub submitted_by: String,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanActionRequest {
    pub timestamp: String,
    pub user: String,
    #[serde(rename = "type")]
    pub action_type: String,
    pub target: String,
    #[serde(rename = "scanType")]
    pub scan_type: ScanType,
    pub options: ClamAvOptions,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScanType {
    File,
    Directory,
    Device,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClamAvOptions {
    #[serde(default = "default_true")]
    pub recursive: bool,
    #[serde(default)]
    pub infected: bool,
    #[serde(default)]
    pub remove: bool,
    #[serde(default, rename = "move")]
    pub move_path: String,
    #[serde(default)]
    pub copy: String,
    #[serde(default)]
    pub exclude_dir: String,
    #[serde(default)]
    pub exclude: String,
    #[serde(default)]
    pub include_dir: String,
    #[serde(default)]
    pub include: String,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: String,
    #[serde(default = "default_max_scan_size")]
    pub max_scan_size: String,
    #[serde(default = "default_max_recursion")]
    pub max_recursion: String,
    #[serde(default = "default_max_files")]
    pub max_files: String,
    #[serde(default = "default_true")]
    pub cross_fs: bool,
    #[serde(default)]
    pub follow_dir_symlinks: bool,
    #[serde(default)]
    pub follow_file_symlinks: bool,
    #[serde(default)]
    pub bell: bool,
    #[serde(default)]
    pub no_summary: bool,
    #[serde(default)]
    pub stdout: bool,
    #[serde(default)]
    pub verbose: bool,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub quiet: bool,
    #[serde(default)]
    pub log: String,
    #[serde(default)]
    pub database: String,
    #[serde(default)]
    pub official_db_only: bool,
    #[serde(default = "default_true")]
    pub bytecode: bool,
    #[serde(default = "default_bytecode_timeout")]
    pub bytecode_timeout: String,
    #[serde(default)]
    pub detect_pua: bool,
    #[serde(default)]
    pub exclude_pua: String,
    #[serde(default)]
    pub include_pua: String,
    #[serde(default)]
    pub detect_structured: bool,
    #[serde(default = "default_structured_count")]
    pub structured_ssn_count: String,
    #[serde(default = "default_structured_count")]
    pub structured_cc_count: String,
    #[serde(default = "default_true")]
    pub scan_pe: bool,
    #[serde(default = "default_true")]
    pub scan_elf: bool,
    #[serde(default = "default_true")]
    pub scan_ole2: bool,
    #[serde(default = "default_true")]
    pub scan_pdf: bool,
    #[serde(default = "default_true")]
    pub scan_swf: bool,
    #[serde(default = "default_true")]
    pub scan_hwp3: bool,
    #[serde(default = "default_true")]
    pub scan_xml_docs: bool,
    #[serde(default = "default_true")]
    pub scan_mail: bool,
    #[serde(default = "default_true")]
    pub scan_html: bool,
    #[serde(default = "default_true")]
    pub scan_archive: bool,
    #[serde(default)]
    pub alert_broken_executables: bool,
    #[serde(default)]
    pub alert_encrypted: bool,
    #[serde(default)]
    pub alert_encrypted_archive: bool,
    #[serde(default)]
    pub alert_encrypted_doc: bool,
    #[serde(default)]
    pub alert_macros: bool,
    #[serde(default, rename = "alertPhishingSSLMismatch")]
    pub alert_phishing_ssl_mismatch: bool,
    #[serde(default)]
    pub alert_phishing_cloak: bool,
    #[serde(default)]
    pub alert_partition_intersection: bool,
    #[serde(default = "default_true")]
    pub heuristic_alerts: bool,
    #[serde(default = "default_true")]
    pub normalize: bool,
    #[serde(default)]
    pub tempdir: String,
    #[serde(default)]
    pub leave_temps: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedJob {
    pub schema_version: u32,
    pub request_id: String,
    pub submitted_at: DateTime<Utc>,
    pub submitted_by: String,
    pub target: String,
    pub scan_type: ScanType,
    pub options: ClamAvOptions,
    pub status: JobStatus,
    pub status_updated_at: DateTime<Utc>,
    pub filename: String,
    pub command_preview: String,
    pub server_metadata: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<JobExecution>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<JobResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobExecution {
    pub worker_pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_pid: Option<u32>,
    pub started_at: DateTime<Utc>,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub cancel_requested: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    pub exit_code: i32,
    pub output: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ScanSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Processing,
    Paused,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueueCounts {
    pub pending: usize,
    pub processing: usize,
    pub done: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub files_scanned: Option<u64>,
    pub infected_files: Option<u64>,
    pub directories_scanned: Option<u64>,
    pub data_scanned: Option<String>,
    pub data_read: Option<String>,
    pub duration: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub clean: bool,
}

impl Default for ClamAvOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            infected: false,
            remove: false,
            move_path: String::new(),
            copy: String::new(),
            exclude_dir: String::new(),
            exclude: String::new(),
            include_dir: String::new(),
            include: String::new(),
            max_file_size: default_max_file_size(),
            max_scan_size: default_max_scan_size(),
            max_recursion: default_max_recursion(),
            max_files: default_max_files(),
            cross_fs: true,
            follow_dir_symlinks: false,
            follow_file_symlinks: false,
            bell: false,
            no_summary: false,
            stdout: false,
            verbose: false,
            debug: false,
            quiet: false,
            log: String::new(),
            database: String::new(),
            official_db_only: false,
            bytecode: true,
            bytecode_timeout: default_bytecode_timeout(),
            detect_pua: false,
            exclude_pua: String::new(),
            include_pua: String::new(),
            detect_structured: false,
            structured_ssn_count: default_structured_count(),
            structured_cc_count: default_structured_count(),
            scan_pe: true,
            scan_elf: true,
            scan_ole2: true,
            scan_pdf: true,
            scan_swf: true,
            scan_hwp3: true,
            scan_xml_docs: true,
            scan_mail: true,
            scan_html: true,
            scan_archive: true,
            alert_broken_executables: false,
            alert_encrypted: false,
            alert_encrypted_archive: false,
            alert_encrypted_doc: false,
            alert_macros: false,
            alert_phishing_ssl_mismatch: false,
            alert_phishing_cloak: false,
            alert_partition_intersection: false,
            heuristic_alerts: true,
            normalize: true,
            tempdir: String::new(),
            leave_temps: false,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_max_file_size() -> String {
    "100M".to_string()
}

fn default_max_scan_size() -> String {
    "400M".to_string()
}

fn default_max_recursion() -> String {
    "17".to_string()
}

fn default_max_files() -> String {
    "10000".to_string()
}

fn default_bytecode_timeout() -> String {
    "10000".to_string()
}

fn default_structured_count() -> String {
    "3".to_string()
}
