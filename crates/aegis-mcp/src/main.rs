use aegis_core::{Config, Error, Result, NAME, VERSION};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| Error::Config(format!("Failed to set tracing subscriber: {}", e)))?;

    info!("Starting {} v{}", NAME, VERSION);

    // Load configuration
    let config = Config::load()
        .map_err(|e| Error::Config(format!("Failed to load configuration: {}", e)))?;

    info!("Configuration loaded successfully");
    info!("Server will bind to {}:{}", config.server.host, config.server.port);

    info!("Aegis-MCP workspace structure initialized successfully!");
    info!("Dependencies: axum, tokio, redis, serde, tracing, config, uuid, etc.");
    info!("Feature flags: development, testing, production");
    info!("Ready for next implementation phase");

    Ok(())
}
