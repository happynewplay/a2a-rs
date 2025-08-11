//! A2A Gateway main entry point

use a2a_gateway::{GatewayConfig, Gateway, GatewayBuilder, Result};
use clap::Parser;
use std::path::PathBuf;
use tracing::{info, error};

#[derive(Parser)]
#[command(name = "a2a-gateway")]
#[command(about = "A high-performance gateway for the Agent-to-Agent (A2A) protocol")]
#[command(version)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "gateway.yaml")]
    config: PathBuf,

    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Bind address
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    bind: String,

    /// Enable metrics endpoint
    #[arg(long)]
    metrics: bool,

    /// Metrics bind address
    #[arg(long, default_value = "0.0.0.0:9090")]
    metrics_bind: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    init_tracing(&cli.log_level)?;

    info!("Starting A2A Gateway");
    info!("Config file: {}", cli.config.display());
    info!("Bind address: {}", cli.bind);

    // Load configuration
    let config = if cli.config.exists() {
        GatewayConfig::from_file(&cli.config).await?
    } else {
        warn!("Configuration file not found, using default configuration");
        GatewayConfig::default()
    };
    info!("Configuration loaded successfully");

    // Build and start the gateway
    let gateway = GatewayBuilder::new()
        .with_config(config)
        .with_bind_address(cli.bind)
        .with_metrics(cli.metrics, cli.metrics_bind)
        .build()
        .await?;

    info!("Gateway built successfully, starting...");

    // Start the gateway
    if let Err(e) = gateway.start().await {
        error!("Gateway failed to start: {}", e);
        return Err(e);
    }

    Ok(())
}

fn init_tracing(level: &str) -> Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}
