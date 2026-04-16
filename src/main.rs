mod audit;
mod config;
mod i18n;
mod providers;
mod render;
mod types;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::process;
use tracing_subscriber::EnvFilter;

use config::Config;
use types::{FeedbackRequest, TimeoutKind};

#[derive(Parser)]
#[command(name = "openfeedback")]
#[command(about = "Human-in-the-loop decision gate CLI for AI agents")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a feedback request and wait for human response
    Send {
        /// Title of the request
        #[arg(short, long)]
        title: String,

        /// Path to a markdown file with the request body
        #[arg(long)]
        body_file: Option<String>,

        /// Inline body text
        #[arg(long)]
        body: Option<String>,

        /// Timeout in seconds (default: from config)
        #[arg(long)]
        timeout: Option<u64>,

        /// Override which provider to use (disables failover).
        /// E.g. `--provider discord` or `--provider telegram`.
        #[arg(long)]
        provider: Option<String>,
    },

    /// Initialize config file with defaults
    Init,
}

#[tokio::main]
async fn main() -> Result<()> {
    // rustls 0.23 requires an explicit CryptoProvider. Install ring globally
    // so tokio-tungstenite can negotiate TLS without extra plumbing.
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            let path = Config::config_path();
            if path.exists() {
                eprintln!("Config already exists at {}", path.display());
                process::exit(1);
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, Config::generate_default())?;
            eprintln!("Config created at {}", path.display());
            eprintln!("Edit it with your provider credentials.");
        }

        Commands::Send {
            title,
            body_file,
            body,
            timeout,
            provider,
        } => {
            let config = Config::load()?;
            let body_content = render::load_body(body_file.as_deref(), body.as_deref())?;
            let timeout_secs = timeout.unwrap_or(config.default_timeout);

            let request = FeedbackRequest {
                title: title.clone(),
                body: body_content,
                timeout_secs,
                reject_feedback_timeout_secs: config.reject_feedback_timeout,
                timeout_kind: TimeoutKind::Final,
            };

            let plan = providers::orchestrator::plan(&config, provider.as_deref())?;
            let response =
                providers::orchestrator::run(plan, timeout_secs, request).await?;

            // Write audit log
            audit::log_response(&config.logging.audit_file, &response)?;

            // Output JSON to stdout for agent consumption
            let json = serde_json::to_string_pretty(&response)?;
            println!("{json}");

            // Exit with appropriate code
            process::exit(response.exit_code());
        }
    }

    Ok(())
}
