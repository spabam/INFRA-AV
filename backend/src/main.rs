mod api;
mod auth;
mod browser;
mod clamav;
mod config;
mod models;
mod queue;
mod report;
mod signatures;
mod worker;
mod version {
    include!(concat!(env!("OUT_DIR"), "/product-version.rs"));
}

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::AppConfig;

#[derive(Debug, Parser)]
#[command(author, version = version::VERSION, about = "INFRA-AV Rust backend")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Server,
    Worker {
        #[arg(long)]
        once: bool,
        #[arg(long, default_value_t = 30)]
        interval_seconds: u64,
    },
    Signatures {
        #[arg(long)]
        once: bool,
        #[arg(long, default_value_t = 14_400)]
        interval_seconds: u64,
    },
    BootstrapUser {
        #[arg(long)]
        username: String,
        #[arg(long)]
        password: String,
        #[arg(long, default_value = "admin")]
        role: String,
        #[arg(long)]
        output: PathBuf,
    },
}

#[actix_web::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Server => {
            let config = AppConfig::from_env(true)?;
            config.ensure_directories()?;
            api::run_server(config).await?;
        }
        Commands::Worker {
            once,
            interval_seconds,
        } => {
            let config = AppConfig::from_env(false)?;
            config.ensure_directories()?;
            let interval = Duration::from_secs(interval_seconds);
            worker::run(config, once, interval)?;
        }
        Commands::Signatures {
            once,
            interval_seconds,
        } => {
            let config = AppConfig::from_env(false)?;
            std::fs::create_dir_all(&config.temp_dir)?;
            signatures::run(&config, once, Duration::from_secs(interval_seconds.max(1)))?;
        }
        Commands::BootstrapUser {
            username,
            password,
            role,
            output,
        } => {
            auth::bootstrap_user_file(&output, &username, &password, &role)?;
        }
    }

    Ok(())
}
