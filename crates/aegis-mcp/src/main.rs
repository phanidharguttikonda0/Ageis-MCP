use aegis_core::{AppConfig, Error, NAME, VERSION};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Load configuration first to get log level
    let config = AppConfig::load()
        .map_err(|e| Error::Config(format!("Failed to load configuration: {}", e)))?;

    // Initialize tracing with configured log level
    let log_level = match config.observability.log_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&config.observability.log_level))
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| Error::Config(format!("Failed to set tracing subscriber: {}", e)))?;

    info!("Starting {} v{}", NAME, VERSION);
    info!("Environment: {}", AppConfig::environment());
    info!("Configuration loaded successfully");
    info!("{}", config.redacted());
    info!("Server will bind to {}:{}", config.server.host, config.server.port);

    info!("Aegis-MCP configuration management system operational!");
    info!("Dependencies: axum, tokio, redis, serde, tracing, config, uuid, etc.");
    info!("Feature flags: development, testing, production");
    info!("Multi-source configuration: environment variables > config files > defaults");
    info!("Validation enabled: fail-fast on invalid configuration");
    info!("Secret redaction active: API keys hidden from logs");

    Ok(())
}
