use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub jwt_secret: String,
    /// Shared secret with the UUUUI broker, used to verify the SSO handoff token
    /// (broker GET /session/token). When unset, auth.sso is disabled.
    pub broker_secret: Option<String>,
    /// Previous broker secret, accepted during a key-rotation overlap window
    /// (see UUUUI/shared/uuuui-identity/CONTRACT.md). Unset outside a rotation.
    pub broker_secret_previous: Option<String>,
    pub data_dir: PathBuf,
    pub pending_dir: PathBuf,
    pub processing_dir: PathBuf,
    pub done_dir: PathBuf,
    pub failed_dir: PathBuf,
    pub result_dir: PathBuf,
    pub log_dir: PathBuf,
    pub quarantine_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub pam_service: String,
    pub default_role: String,
    pub users_file: PathBuf,
    pub clamscan_bin: String,
    pub freshclam_bin: String,
    pub allowed_scan_roots: Vec<PathBuf>,
    pub allowed_write_roots: Vec<PathBuf>,
    pub allowed_database_roots: Vec<PathBuf>,
    pub allow_device_scans: bool,
    pub cors_origins: Vec<String>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let project_root = env::var("INFRA_AV_PROJECT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from("/home/spabam/Dropbox/Documenti/Secondary-Projects/INFRA-AV")
            });

        let data_dir = env::var("INFRA_AV_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| project_root.join("backend").join("data"));
        let users_file = env::var("INFRA_AV_USERS_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                project_root
                    .join("backend")
                    .join("config")
                    .join("users.json")
            });

        let action_dir = data_dir.join("action");
        let result_dir = data_dir.join("result");
        let log_dir = data_dir.join("log");
        let quarantine_dir = data_dir.join("quarantine");
        let temp_dir = data_dir.join("temp");

        let allowed_scan_roots = parse_paths(
            "INFRA_AV_ALLOWED_SCAN_ROOTS",
            &[
                PathBuf::from("/home"),
                PathBuf::from("/srv"),
                PathBuf::from("/tmp"),
            ],
        );
        let allowed_write_roots = parse_paths(
            "INFRA_AV_ALLOWED_WRITE_ROOTS",
            std::slice::from_ref(&quarantine_dir),
        );
        let allowed_database_roots = parse_paths(
            "INFRA_AV_ALLOWED_DATABASE_ROOTS",
            &[
                PathBuf::from("/var/lib/clamav"),
                PathBuf::from("/usr/local/share/clamav"),
            ],
        );

        let cors_origins = env::var("INFRA_AV_CORS_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:8080,http://127.0.0.1:8080".to_string())
            .split(',')
            .filter_map(|value| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            })
            .collect::<Vec<_>>();

        Ok(Self {
            host: env::var("INFRA_AV_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("INFRA_AV_PORT")
                .ok()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(5000),
            jwt_secret: env::var("INFRA_AV_JWT_SECRET")
                .context("INFRA_AV_JWT_SECRET must be set")?,
            broker_secret: env::var("INFRA_AV_BROKER_SECRET")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            broker_secret_previous: env::var("INFRA_AV_BROKER_SECRET_PREVIOUS")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            data_dir,
            pending_dir: action_dir.join("pending"),
            processing_dir: action_dir.join("processing"),
            done_dir: action_dir.join("done"),
            failed_dir: action_dir.join("failed"),
            result_dir,
            log_dir,
            quarantine_dir,
            temp_dir,
            pam_service: env::var("INFRA_AV_PAM_SERVICE")
                .unwrap_or_else(|_| "infra-av".to_string()),
            // Least-privilege fallback: an account that authenticates but has no
            // explicit role (a PAM user absent from users.json, or a non-admin
            // UUUUI SSO user) gets "member", not "admin". Administrators are
            // granted explicitly (users.json, or UUUUI admin/superadmin roles).
            default_role: env::var("INFRA_AV_DEFAULT_ROLE")
                .unwrap_or_else(|_| "member".to_string()),
            users_file,
            clamscan_bin: env::var("INFRA_AV_CLAMSCAN_BIN")
                .unwrap_or_else(|_| "/usr/bin/clamscan".to_string()),
            freshclam_bin: env::var("INFRA_AV_FRESHCLAM_BIN")
                .unwrap_or_else(|_| "/usr/bin/freshclam".to_string()),
            allowed_scan_roots,
            allowed_write_roots,
            allowed_database_roots,
            allow_device_scans: env::var("INFRA_AV_ALLOW_DEVICE_SCANS")
                .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
                .unwrap_or(false),
            cors_origins,
        })
    }

    pub fn ensure_directories(&self) -> Result<()> {
        let directories = [
            &self.data_dir,
            &self.pending_dir,
            &self.processing_dir,
            &self.done_dir,
            &self.failed_dir,
            &self.result_dir,
            &self.log_dir,
            &self.quarantine_dir,
            &self.temp_dir,
        ];

        for directory in directories {
            fs::create_dir_all(directory)
                .with_context(|| format!("failed to create {}", directory.display()))?;
        }

        if let Some(parent) = self.users_file.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        Ok(())
    }
}

fn parse_paths(key: &str, defaults: &[PathBuf]) -> Vec<PathBuf> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .split(',')
                .filter_map(|item| {
                    let trimmed = item.trim();
                    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
                })
                .collect::<Vec<_>>()
        })
        .filter(|paths| !paths.is_empty())
        .unwrap_or_else(|| defaults.to_vec())
}
