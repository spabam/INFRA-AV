use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::config::AppConfig;

pub fn update(config: &AppConfig) -> Result<(i32, Vec<String>)> {
    let lock = if config.split_worker {
        config.temp_dir.join("freshclam.lock")
    } else {
        PathBuf::from("/var/lock/infra-av-freshclam.lock")
    };
    let mut command = Command::new("flock");
    command
        .args(["-n"])
        .arg(lock)
        .arg(&config.freshclam_bin)
        .arg("--stdout");
    if let Some(path) = std::env::var_os("INFRA_AV_FRESHCLAM_CONF") {
        command.arg("--config-file").arg(path);
    }
    let output = command
        .output()
        .with_context(|| format!("failed to execute {}", config.freshclam_bin))?;
    let mut lines = Vec::new();
    lines.extend(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string),
    );
    lines.extend(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .map(str::to_string),
    );
    Ok((output.status.code().unwrap_or(56), lines))
}

pub fn run(config: &AppConfig, once: bool, interval: Duration) -> Result<()> {
    loop {
        match update(config) {
            Ok((exit_code, output)) => {
                for line in output {
                    println!("{line}");
                }
                if once {
                    if exit_code != 0 {
                        bail!("freshclam exited with status {exit_code}");
                    }
                    return Ok(());
                }
                if exit_code != 0 {
                    log::warn!("freshclam exited with status {exit_code}");
                }
            }
            Err(error) if once => return Err(error),
            Err(error) => log::warn!("signature update failed: {error:#}"),
        }
        thread::sleep(interval);
    }
}
