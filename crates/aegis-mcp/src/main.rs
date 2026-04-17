use aegis_core::{AppConfig, Error, NAME, VERSION, init_tracing, init_dev_tracing, RequestContext};
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

    // Demo: Create example spans to show instrumentation
    demo_tracing_instrumentation(&startup_context);

    Ok(())
}

/// Demonstrate tracing instrumentation capabilities
fn demo_tracing_instrumentation(context: &RequestContext) {
    let _guard = context.enter();

    // Demo: Request span
    let request_span = aegis_core::create_request_span(&context.request_id, "demo_request");
    let _request_enter = request_span.enter();
    info!("Processing demo request");

    // Demo: Cache operation span
    let cache_span = aegis_core::create_cache_span("lookup", Some("demo_hash_123"));
    {
        let _cache_enter = cache_span.enter();
        info!("Cache lookup operation");
        // Simulate cache hit
        tracing::span::Span::current().record("cache_hit", true);
        tracing::span::Span::current().record("similarity_score", 0.98);
    }

    // Demo: LLM operation span
    let llm_span = aegis_core::create_llm_span("completion", "gpt-4");
    {
        let _llm_enter = llm_span.enter();
        info!("LLM API call");
        tracing::span::Span::current().record("tokens_used", 150);
        tracing::span::Span::current().record("latency_ms", 1250);
    }

    // Demo: Redis operation span
    let redis_span = aegis_core::create_redis_span("vector_search");
    {
        let _redis_enter = redis_span.enter();
        info!("Redis vector search");
        tracing::span::Span::current().record("latency_ms", 45);
    }

    // Demo: Embedding generation span
    let embedding_span = aegis_core::create_embedding_span("text-embedding-ada-002", 250);
    {
        let _embedding_enter = embedding_span.enter();
        info!("Embedding generation");
        tracing::span::Span::current().record("latency_ms", 120);
    }

    info!("Demo instrumentation complete - tracing system verified");
}
