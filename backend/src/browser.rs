use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::config::AppConfig;
use crate::models::{
    ScanBrowseEntry, ScanBrowseEntryKind, ScanBrowseRequest, ScanBrowseResponse, ScanType,
};

pub fn browse_targets(config: &AppConfig, request: ScanBrowseRequest) -> Result<ScanBrowseResponse> {
    match request.scan_type {
        ScanType::Device => browse_devices(config),
        ScanType::File | ScanType::Directory => browse_filesystem(config, request),
    }
}

fn browse_filesystem(config: &AppConfig, request: ScanBrowseRequest) -> Result<ScanBrowseResponse> {
    let roots = existing_roots(&config.allowed_scan_roots);
    if roots.is_empty() {
        bail!("no allowed scan roots are available on this system");
    }

    let current_path = match request.path.filter(|value| !value.trim().is_empty()) {
        Some(path) => validate_browsable_directory(config, &path)?,
        None => roots[0].clone(),
    };
    let parent_path = current_path.parent().and_then(|parent| {
        if roots.iter().any(|root| parent.starts_with(root)) {
            Some(parent.display().to_string())
        } else {
            None
        }
    });

    let mut entries = fs::read_dir(&current_path)
        .with_context(|| format!("failed to read {}", current_path.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| browse_entry_for_path(request.scan_type, entry.path()).transpose())
        .collect::<Result<Vec<_>>>()?;

    entries.sort_by(|left, right| {
        rank_entry_kind(left.kind)
            .cmp(&rank_entry_kind(right.kind))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });

    Ok(ScanBrowseResponse {
        scan_type: request.scan_type,
        current_path: Some(current_path.display().to_string()),
        parent_path,
        can_select_current: request.scan_type == ScanType::Directory,
        roots: roots
            .iter()
            .map(|value| value.display().to_string())
            .collect::<Vec<_>>(),
        entries,
    })
}

fn browse_devices(config: &AppConfig) -> Result<ScanBrowseResponse> {
    if !config.allow_device_scans {
        bail!("device scans are disabled");
    }

    let mut entries = fs::read_dir("/sys/class/block")
        .context("failed to read /sys/class/block")?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| device_entry(entry.path()).transpose())
        .collect::<Result<Vec<_>>>()?;

    entries.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(ScanBrowseResponse {
        scan_type: ScanType::Device,
        current_path: None,
        parent_path: None,
        can_select_current: false,
        roots: Vec::new(),
        entries,
    })
}

fn existing_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .filter(|path| path.is_dir())
        .cloned()
        .collect::<Vec<_>>()
}

fn validate_browsable_directory(config: &AppConfig, value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value.trim());
    if !path.is_absolute() {
        bail!("browse path must be absolute");
    }

    let resolved =
        fs::canonicalize(&path).with_context(|| format!("failed to resolve {}", path.display()))?;
    if !resolved.is_dir() {
        bail!("browse path is not a directory");
    }
    if !config
        .allowed_scan_roots
        .iter()
        .any(|root| resolved.starts_with(root))
    {
        bail!(
            "browse path {} is outside the allowed scan roots",
            resolved.display()
        );
    }

    Ok(resolved)
}

fn browse_entry_for_path(scan_type: ScanType, path: PathBuf) -> Result<Option<ScanBrowseEntry>> {
    let metadata = match fs::metadata(&path) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    if metadata.is_dir() {
        return Ok(Some(ScanBrowseEntry {
            name: file_name,
            path: path.display().to_string(),
            kind: ScanBrowseEntryKind::Directory,
            selectable: scan_type == ScanType::Directory,
            description: Some("Directory".to_string()),
        }));
    }

    if metadata.is_file() {
        return Ok(Some(ScanBrowseEntry {
            name: file_name,
            path: path.display().to_string(),
            kind: ScanBrowseEntryKind::File,
            selectable: scan_type == ScanType::File,
            description: Some(format_file_description(&metadata.len())),
        }));
    }

    Ok(None)
}

fn device_entry(sys_path: PathBuf) -> Result<Option<ScanBrowseEntry>> {
    let name = sys_path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.is_empty()
        || name.starts_with("loop")
        || name.starts_with("ram")
        || name.starts_with("zram")
    {
        return Ok(None);
    }

    let dev_path = Path::new("/dev").join(&name);
    if !dev_path.exists() {
        return Ok(None);
    }

    let description = format_device_description(&sys_path)?;
    Ok(Some(ScanBrowseEntry {
        name,
        path: dev_path.display().to_string(),
        kind: ScanBrowseEntryKind::Device,
        selectable: true,
        description: Some(description),
    }))
}

fn format_device_description(sys_path: &Path) -> Result<String> {
    let sectors = fs::read_to_string(sys_path.join("size"))
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0);
    let bytes = sectors.saturating_mul(512);
    let size = format_file_description(&bytes);

    let model = fs::read_to_string(sys_path.join("device/model"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let is_partition = sys_path.join("partition").exists();
    let kind = if is_partition { "Partition" } else { "Block device" };

    Ok(match model {
        Some(model) => format!("{kind} • {size} • {model}"),
        None => format!("{kind} • {size}"),
    })
}

fn format_file_description(bytes: &u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = *bytes as f64;
    let mut unit_index = 0usize;
    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", *bytes, UNITS[unit_index])
    } else {
        format!("{value:.1} {}", UNITS[unit_index])
    }
}

fn rank_entry_kind(kind: ScanBrowseEntryKind) -> u8 {
    match kind {
        ScanBrowseEntryKind::Root => 0,
        ScanBrowseEntryKind::Directory => 1,
        ScanBrowseEntryKind::File => 2,
        ScanBrowseEntryKind::Device => 3,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use uuid::Uuid;

    use crate::config::AppConfig;
    use crate::models::{ScanBrowseEntryKind, ScanBrowseRequest, ScanType};

    use super::browse_targets;

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
            allowed_scan_roots: vec![root.join("allowed")],
            allowed_write_roots: vec![root.join("data/quarantine")],
            allowed_database_roots: vec![PathBuf::from("/var/lib/clamav")],
            allow_device_scans: false,
            cors_origins: vec![],
        }
    }

    #[test]
    fn browse_defaults_to_first_root_and_lists_current_directory_only() {
        let root = std::env::temp_dir().join(format!("infra-av-browser-test-{}", Uuid::new_v4()));
        let allowed_root = root.join("allowed");
        let nested_dir = allowed_root.join("nested");
        let visible_file = allowed_root.join("visible.txt");

        fs::create_dir_all(&nested_dir).expect("allowed root should be created");
        fs::write(&visible_file, "scan me").expect("visible file should be created");

        let response = browse_targets(
            &test_config(&root),
            ScanBrowseRequest {
                scan_type: ScanType::Directory,
                path: None,
            },
        )
        .expect("browse should succeed");

        assert_eq!(
            response.current_path.as_deref(),
            Some(allowed_root.to_string_lossy().as_ref())
        );
        assert!(response.parent_path.is_none());
        assert!(response.can_select_current);
        assert!(response.entries.iter().all(|entry| entry.kind != ScanBrowseEntryKind::Root));
        assert!(response.entries.iter().any(|entry| {
            entry.kind == ScanBrowseEntryKind::Directory && entry.path == nested_dir.display().to_string()
        }));
        assert!(response.entries.iter().any(|entry| {
            entry.kind == ScanBrowseEntryKind::File
                && entry.path == visible_file.display().to_string()
                && !entry.selectable
        }));
    }
}
