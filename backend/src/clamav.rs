use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use regex::Regex;

use crate::config::AppConfig;
use crate::models::{ClamAvOptions, ScanSummary, ScanType};

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub preview: String,
    pub log_path: PathBuf,
}

pub fn build_command(
    config: &AppConfig,
    request_id: &str,
    username: &str,
    target: &str,
    scan_type: ScanType,
    options: &ClamAvOptions,
) -> Result<CommandSpec> {
    validate_option_combinations(options)?;

    let validated_target = validate_target(config, target, scan_type)?;
    let move_dir = validate_optional_write_path(config, options.move_path.as_str())?;
    let copy_dir = validate_optional_write_path(config, options.copy.as_str())?;
    validate_scan_mutation(
        &config.read_only_scan_roots,
        &validated_target,
        scan_type,
        options,
        [move_dir.as_deref(), copy_dir.as_deref()],
    )?;
    let database_dir = validate_optional_database_path(config, options.database.as_str())?;
    let temp_dir = validate_temp_dir(config, options.tempdir.as_str())?;

    let log_dir = config.log_dir.join(username);
    fs::create_dir_all(&log_dir)
        .with_context(|| format!("failed to create {}", log_dir.display()))?;
    let log_path = log_dir.join(format!("{request_id}.clamscan.log"));

    let mut args = vec![format!("--log={}", log_path.display())];

    if options.recursive && scan_type == ScanType::Directory {
        args.push("--recursive".to_string());
    }
    if options.infected {
        args.push("--infected".to_string());
    }
    if options.remove {
        args.push("--remove=yes".to_string());
    }
    if let Some(path) = move_dir {
        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create {}", path.display()))?;
        args.push(format!("--move={}", path.display()));
    }
    if let Some(path) = copy_dir {
        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create {}", path.display()))?;
        args.push(format!("--copy={}", path.display()));
    }

    push_if_non_empty(&mut args, "--exclude-dir", &options.exclude_dir);
    push_if_non_empty(&mut args, "--exclude", &options.exclude);
    push_if_non_empty(&mut args, "--include-dir", &options.include_dir);
    push_if_non_empty(&mut args, "--include", &options.include);

    args.push(format!("--max-filesize={}", options.max_file_size));
    args.push(format!("--max-scansize={}", options.max_scan_size));
    args.push(format!("--max-recursion={}", options.max_recursion));
    args.push(format!("--max-files={}", options.max_files));

    if !options.cross_fs {
        args.push("--cross-fs=no".to_string());
    }
    args.push(format!(
        "--follow-dir-symlinks={}",
        if options.follow_dir_symlinks { 1 } else { 0 }
    ));
    args.push(format!(
        "--follow-file-symlinks={}",
        if options.follow_file_symlinks { 1 } else { 0 }
    ));

    push_boolean_disable(&mut args, "--scan-pe", options.scan_pe);
    push_boolean_disable(&mut args, "--scan-elf", options.scan_elf);
    push_boolean_disable(&mut args, "--scan-ole2", options.scan_ole2);
    push_boolean_disable(&mut args, "--scan-pdf", options.scan_pdf);
    push_boolean_disable(&mut args, "--scan-swf", options.scan_swf);
    push_boolean_disable(&mut args, "--scan-hwp3", options.scan_hwp3);
    push_boolean_disable(&mut args, "--scan-xmldocs", options.scan_xml_docs);
    push_boolean_disable(&mut args, "--scan-mail", options.scan_mail);
    push_boolean_disable(&mut args, "--scan-html", options.scan_html);
    push_boolean_disable(&mut args, "--scan-archive", options.scan_archive);

    if !options.heuristic_alerts {
        args.push("--heuristic-alerts=no".to_string());
    }
    if options.detect_pua {
        args.push("--detect-pua=yes".to_string());
        push_if_non_empty(&mut args, "--exclude-pua", &options.exclude_pua);
        push_if_non_empty(&mut args, "--include-pua", &options.include_pua);
    }
    if options.detect_structured {
        args.push("--detect-structured=yes".to_string());
        args.push(format!(
            "--structured-ssn-count={}",
            options.structured_ssn_count
        ));
        args.push(format!(
            "--structured-cc-count={}",
            options.structured_cc_count
        ));
    }

    if options.alert_broken_executables {
        args.push("--alert-broken=yes".to_string());
    }
    if options.alert_encrypted {
        args.push("--alert-encrypted=yes".to_string());
    }
    if options.alert_encrypted_archive {
        args.push("--alert-encrypted-archive=yes".to_string());
    }
    if options.alert_encrypted_doc {
        args.push("--alert-encrypted-doc=yes".to_string());
    }
    if options.alert_macros {
        args.push("--alert-macros=yes".to_string());
    }
    if options.alert_phishing_ssl_mismatch {
        args.push("--alert-phishing-ssl=yes".to_string());
    }
    if options.alert_phishing_cloak {
        args.push("--alert-phishing-cloak=yes".to_string());
    }
    if options.alert_partition_intersection {
        args.push("--alert-partition-intersection=yes".to_string());
    }

    if let Some(path) = database_dir {
        args.push(format!("--database={}", path.display()));
    }
    if options.official_db_only {
        args.push("--official-db-only=yes".to_string());
    }
    if !options.bytecode {
        args.push("--bytecode=no".to_string());
    } else {
        args.push(format!("--bytecode-timeout={}", options.bytecode_timeout));
    }

    if options.verbose {
        args.push("--verbose".to_string());
    }
    if options.debug {
        args.push("--debug".to_string());
    }
    if options.quiet {
        args.push("--quiet".to_string());
    }
    if options.bell {
        args.push("--bell".to_string());
    }
    if options.no_summary {
        args.push("--no-summary".to_string());
    }
    if options.stdout {
        args.push("--stdout".to_string());
    }

    if !options.normalize {
        args.push("--normalize=no".to_string());
    }
    args.push(format!("--tempdir={}", temp_dir.display()));
    if options.leave_temps {
        args.push("--leave-temps=yes".to_string());
    }

    args.push(validated_target.display().to_string());

    Ok(CommandSpec {
        program: config.clamscan_bin.clone(),
        preview: render_preview(&config.clamscan_bin, &args),
        args,
        log_path,
    })
}

pub fn probe_command(program: &str, version_flag: &str) -> (bool, String) {
    match Command::new(program).arg(version_flag).output() {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (output.status.success(), text)
        }
        Err(error) => (false, error.to_string()),
    }
}

pub fn parse_summary(lines: &[String], exit_code: i32) -> Option<ScanSummary> {
    let mut summary = ScanSummary {
        files_scanned: None,
        infected_files: None,
        directories_scanned: None,
        data_scanned: None,
        data_read: None,
        duration: None,
        start_date: None,
        end_date: None,
        clean: false,
    };

    let mut found_summary = false;

    for line in lines {
        if let Some((_, value)) = split_summary_line(line, "Scanned directories:") {
            summary.directories_scanned = value.parse().ok();
            found_summary = true;
        } else if let Some((_, value)) = split_summary_line(line, "Scanned files:") {
            summary.files_scanned = value.parse().ok();
            found_summary = true;
        } else if let Some((_, value)) = split_summary_line(line, "Infected files:") {
            summary.infected_files = value.parse().ok();
            found_summary = true;
        } else if let Some((_, value)) = split_summary_line(line, "Data scanned:") {
            summary.data_scanned = Some(value.to_string());
            found_summary = true;
        } else if let Some((_, value)) = split_summary_line(line, "Data read:") {
            summary.data_read = Some(value.to_string());
            found_summary = true;
        } else if let Some((_, value)) = split_summary_line(line, "Time:") {
            summary.duration = Some(value.to_string());
            found_summary = true;
        } else if let Some((_, value)) = split_summary_line(line, "Start Date:") {
            summary.start_date = Some(value.to_string());
            found_summary = true;
        } else if let Some((_, value)) = split_summary_line(line, "End Date:") {
            summary.end_date = Some(value.to_string());
            found_summary = true;
        }
    }

    summary.clean = exit_code == 0 && summary.infected_files == Some(0);
    found_summary.then_some(summary)
}

fn split_summary_line<'a>(line: &'a str, prefix: &'a str) -> Option<(&'a str, &'a str)> {
    line.strip_prefix(prefix)
        .map(|value| (prefix, value.trim()))
}

fn validate_option_combinations(options: &ClamAvOptions) -> Result<()> {
    let destructive_modes = usize::from(options.remove)
        + usize::from(!options.move_path.trim().is_empty())
        + usize::from(!options.copy.trim().is_empty());
    if destructive_modes > 1 {
        bail!("remove, move and copy are mutually exclusive");
    }

    let numeric_value = Regex::new(r"^[0-9]+[KMGTP]?$").expect("valid regex");
    for value in [
        options.max_file_size.as_str(),
        options.max_scan_size.as_str(),
        options.max_recursion.as_str(),
        options.max_files.as_str(),
        options.bytecode_timeout.as_str(),
        options.structured_ssn_count.as_str(),
        options.structured_cc_count.as_str(),
    ] {
        if !numeric_value.is_match(value) {
            bail!("invalid numeric value: {}", value);
        }
    }

    Ok(())
}

fn validate_scan_mutation(
    protected: &[PathBuf],
    target: &Path,
    scan_type: ScanType,
    options: &ClamAvOptions,
    destinations: [Option<&Path>; 2],
) -> Result<()> {
    let changes_source = options.remove || !options.move_path.trim().is_empty();
    let recursive = scan_type == ScanType::Directory && options.recursive;
    for root in protected {
        if destinations.iter().flatten().any(|path| path.starts_with(root)) {
            bail!("ClamAV output destination is in a read-only scan root");
        }
        if changes_source
            && (target.starts_with(root)
                || (recursive && root.starts_with(target))
                || (scan_type == ScanType::Directory
                    && (options.follow_dir_symlinks || options.follow_file_symlinks)))
        {
            bail!("Remove and quarantine moves cannot modify read-only scan roots");
        }
    }
    Ok(())
}

fn validate_target(config: &AppConfig, target: &str, scan_type: ScanType) -> Result<PathBuf> {
    let raw_path = PathBuf::from(target);
    if !raw_path.is_absolute() {
        bail!("target must be an absolute path");
    }

    let resolved = fs::canonicalize(&raw_path)
        .with_context(|| format!("failed to resolve {}", raw_path.display()))?;
    if raw_path != resolved
        && config
            .read_only_scan_roots
            .iter()
            .any(|root| raw_path.starts_with(root) || resolved.starts_with(root))
    {
        bail!("Scan targets in read-only storage require a direct path");
    }

    let metadata = fs::metadata(&resolved)
        .with_context(|| format!("failed to stat target {}", raw_path.display()))?;

    match scan_type {
        ScanType::File if !metadata.is_file() => bail!("target is not a file"),
        ScanType::Directory if !metadata.is_dir() => bail!("target is not a directory"),
        ScanType::Device => {
            if !config.allow_device_scans {
                bail!("device scans are disabled");
            }
            if !metadata.file_type().is_block_device() {
                bail!("target is not a block device");
            }
        }
        _ => {}
    }

    if scan_type != ScanType::Device
        && !config
            .allowed_scan_roots
            .iter()
            .any(|root| resolved.starts_with(root))
    {
        bail!(
            "target {} is outside the allowed scan roots",
            resolved.display()
        );
    }

    Ok(resolved)
}

/// Confine a caller-supplied path to one of `roots`. A `starts_with` check on
/// the raw path is not enough: component-wise `starts_with` accepts
/// `<root>/../../escape`, so a `..` could redirect clamscan's `--move`/`--copy`/
/// `--database`/`--tempdir` outside the intended directory. We reject any `..`
/// component outright, and — resolving symlinks whenever the path (or a root)
/// already exists — require the resolved location to stay under a resolved root,
/// so a symlink cannot redirect writes out either.
fn within_roots(path: &Path, roots: &[PathBuf]) -> bool {
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return false;
    }
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    roots.iter().any(|root| {
        let root = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        resolved.starts_with(&root)
    })
}

fn validate_optional_write_path(config: &AppConfig, value: &str) -> Result<Option<PathBuf>> {
    if value.trim().is_empty() {
        return Ok(None);
    }

    let path = PathBuf::from(value.trim());
    if !path.is_absolute() {
        bail!("write path must be absolute");
    }

    if !within_roots(&path, &config.allowed_write_roots) {
        bail!(
            "write path {} is outside the allowed write roots",
            path.display()
        );
    }

    Ok(Some(path))
}

fn validate_optional_database_path(config: &AppConfig, value: &str) -> Result<Option<PathBuf>> {
    if value.trim().is_empty() {
        return Ok(None);
    }

    let path = PathBuf::from(value.trim());
    if !path.is_absolute() {
        bail!("database path must be absolute");
    }
    if !path.exists() {
        bail!("database path {} does not exist", path.display());
    }

    if !within_roots(&path, &config.allowed_database_roots) {
        bail!(
            "database path {} is outside the allowed database roots",
            path.display()
        );
    }

    Ok(Some(path))
}

fn validate_temp_dir(config: &AppConfig, value: &str) -> Result<PathBuf> {
    if value.trim().is_empty() {
        fs::create_dir_all(&config.temp_dir)
            .with_context(|| format!("failed to create {}", config.temp_dir.display()))?;
        return Ok(config.temp_dir.clone());
    }

    let path = PathBuf::from(value.trim());
    if !path.is_absolute() {
        bail!("tempdir must be absolute");
    }
    if !within_roots(&path, std::slice::from_ref(&config.temp_dir)) {
        bail!(
            "tempdir {} must stay under {}",
            path.display(),
            config.temp_dir.display()
        );
    }
    fs::create_dir_all(&path).with_context(|| format!("failed to create {}", path.display()))?;
    Ok(path)
}

fn push_if_non_empty(args: &mut Vec<String>, flag: &str, value: &str) {
    if !value.trim().is_empty() {
        args.push(format!("{flag}={value}"));
    }
}

fn push_boolean_disable(args: &mut Vec<String>, flag: &str, enabled: bool) {
    if !enabled {
        args.push(format!("{flag}=no"));
    }
}

fn render_preview(program: &str, args: &[String]) -> String {
    let mut parts = vec![program.to_string()];
    parts.extend(args.iter().map(|value| shell_escape(value)));
    parts.join(" \\\n  ")
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "/._=-".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn clean_summary_requires_success_and_an_explicit_zero_detection_count() {
        for (lines, exit_code, clean) in [
            (vec!["Scanned files: 1"], 0, false),
            (vec!["Infected files: invalid"], 0, false),
            (vec!["Infected files: 1"], 0, false),
            (vec!["Infected files: 0"], 2, false),
            (vec!["Infected files: 0"], 0, true),
        ] {
            let lines = lines.into_iter().map(str::to_string).collect::<Vec<_>>();
            let summary = super::parse_summary(&lines, exit_code).expect("summary fields present");
            assert_eq!(summary.clean, clean, "{lines:?}, exit {exit_code}");
        }
        assert!(super::parse_summary(&["partial raw output".to_string()], 0).is_none());
    }

    use super::{render_preview, within_roots};
    use std::path::{Path, PathBuf};

    #[test]
    fn preview_quotes_paths_with_spaces() {
        let rendered = render_preview(
            "clamscan",
            &[
                "--log=/tmp/a.log".to_string(),
                "/tmp/with space".to_string(),
            ],
        );
        assert!(rendered.contains("'/tmp/with space'"));
    }

    #[test]
    fn within_roots_rejects_parent_dir_escapes() {
        let roots = vec![PathBuf::from("/var/lib/infra-av/quarantine")];
        // In-root paths are accepted (they need not exist yet).
        assert!(within_roots(
            Path::new("/var/lib/infra-av/quarantine/moved"),
            &roots
        ));
        // A `..` component that lexically climbs out is rejected even though a
        // naive component-wise starts_with would accept it.
        assert!(!within_roots(
            Path::new("/var/lib/infra-av/quarantine/../../../etc"),
            &roots
        ));
        // A sibling outside the root is rejected.
        assert!(!within_roots(Path::new("/etc/cron.d"), &roots));
    }
}
