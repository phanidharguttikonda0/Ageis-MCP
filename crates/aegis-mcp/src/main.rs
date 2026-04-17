use aegis_core::{AppConfig, Error, NAME, VERSION, init_tracing, init_dev_tracing, RequestContext, Server, Metrics};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Load configuration first to get observability settings
    let config = AppConfig::load()
        .map_err(|e| Error::Config(format!("Failed to load configuration: {}", e)))?;

    // Initialize tracing based on environment
    let environment = AppConfig::environment();
    if environment == "production" {
        // Use JSON formatting and OpenTelemetry in production
        init_tracing(&config.observability.log_level, &config.observability.jaeger_endpoint)
            .map_err(|e| Error::Config(format!("Failed to initialize tracing: {}", e)))?;
    } else {
        // Use pretty printing in development
        init_dev_tracing(&config.observability.log_level)
            .map_err(|e| Error::Config(format!("Failed to initialize tracing: {}", e)))?;
    }

    // Create request context for the startup operation
    let startup_context = RequestContext::new(
        uuid::Uuid::new_v4().to_string(),
        Some("startup".to_string())
    );

    let _guard = startup_context.enter();

    info!("Starting {} v{}", NAME, VERSION);
    info!("Environment: {}", environment);
    info!("Configuration loaded successfully");
    info!("{}", config.redacted());
    info!("Server will bind to {}:{}", config.server.host, config.server.port);

    info!("Aegis-MCP structured logging and tracing system operational!");
    info!("Observability features:");
    info!("  - Structured JSON logging (production)");
    info!("  - Distributed tracing with OpenTelemetry");
    info!("  - Jaeger integration for trace visualization");
    info!("  - Comprehensive span instrumentation");
    info!("  - Request lifecycle tracking");
    info!("  - Cache operation monitoring");
    info!("  - LLM API call tracing");
    info!("  - Redis query instrumentation");
    info!("  - Automatic sensitive data redaction");
    info!("  - Log sampling for high-traffic scenarios");

    // Create and start the HTTP server
    let metrics = Metrics::new();
    let server = Server::new(config.clone(), metrics);

    info!("Starting HTTP server...");
    server.run().await?;

    Ok(())
}
