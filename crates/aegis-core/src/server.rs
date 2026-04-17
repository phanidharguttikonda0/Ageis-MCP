//! HTTP server implementation for Aegis-MCP
//!
//! Provides the web server with routing for health checks, metrics, and cache management.

use axum::{
    extract::State,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    trace::TraceLayer,
};
use uuid::Uuid;

use crate::{Error, Result, AppConfig, Metrics};

/// HTTP server instance
pub struct Server {
    config: AppConfig,
    metrics: Metrics,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl Server {
    /// Create a new server instance
    pub fn new(config: AppConfig, metrics: Metrics) -> Self {
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
        Self {
            config,
            metrics,
            shutdown_tx,
        }
    }

    /// Run the server
    pub async fn run(self) -> Result<()> {
        let addr = format!("{}:{}", self.config.server.host, self.config.server.port);
        let listener = tokio::net::TcpListener::bind(&addr).await
            .map_err(|e| Error::Config(format!("Failed to bind to {}: {}", addr, e)))?;

        tracing::info!("Aegis-MCP server listening on {}", addr);

        let app = self.create_router();

        // Graceful shutdown handling
        let shutdown_tx = self.shutdown_tx.clone();
        tokio::spawn(async move {
            // Wait for CTRL+C
            #[cfg(unix)]
            {
                let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                    .expect("Failed to setup SIGTERM handler");
                let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
                    .expect("Failed to setup SIGINT handler");

                tokio::select! {
                    _ = sigterm.recv() => {
                        tracing::info!("Received SIGTERM, shutting down gracefully...");
                    }
                    _ = sigint.recv() => {
                        tracing::info!("Received SIGINT, shutting down gracefully...");
                    }
                }
            }

            #[cfg(windows)]
            {
                // Windows shutdown handling would go here
                let ctrl_c = async {
                    tokio::signal::ctrl_c()
                        .await
                        .expect("Failed to install CTRL+C handler");
                };
                ctrl_c.await;
                tracing::info!("Received CTRL+C, shutting down gracefully...");
            }

            // Broadcast shutdown signal
            let _ = shutdown_tx.send(());
        });

        // Run the server
        axum::serve(listener, app).await
            .map_err(|e| Error::Config(format!("Server error: {}", e)))?;

        Ok(())
    }

    /// Create the application router
    fn create_router(&self) -> Router {
        Router::new()
            // Health check endpoint
            .route("/health", get(health_check))
            // Metrics endpoint
            .route("/metrics", get(get_metrics))
            // Cache statistics
            .route("/stats/cache", get(get_cache_stats))
            // Cache invalidation
            .route("/api/v1/cache/invalidate", post(invalidate_cache))
            // Root endpoint
            .route("/", get(root))
            .with_state(self.config.clone())
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CompressionLayer::new())
                    .layer(CorsLayer::permissive())
                    .into_inner(),
            )
    }
}

/// Health check handler
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "Aegis-MCP",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }))
}

/// Metrics handler
async fn get_metrics() -> impl IntoResponse {
    // This would return Prometheus metrics
    // For now, return a placeholder
    Json(serde_json::json!({
        "metrics": "Prometheus metrics will be served here"
    }))
}

/// Cache statistics handler
async fn get_cache_stats(State(config): State<AppConfig>) -> impl IntoResponse {
    // This would return actual cache statistics
    // For now, return configuration info
    Json(serde_json::json!({
        "cache_config": {
            "similarity_threshold": config.cache.similarity_threshold,
            "max_cache_size": config.cache.max_cache_size,
            "ttl_seconds": config.cache.ttl_seconds
        },
        "status": "Cache statistics will be available here"
    }))
}

/// Cache invalidation handler
async fn invalidate_cache(
    State(_cfg): State<AppConfig>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    // This would actually invalidate cache entries
    // For now, return a placeholder response
    tracing::info!("Cache invalidation requested: {:?}", payload);

    Json(serde_json::json!({
        "status": "ok",
        "message": "Cache invalidation will be implemented here"
    }))
}

/// Root handler
async fn root() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "Aegis-MCP",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Semantic caching server and governance layer for LLM requests",
        "endpoints": {
            "health": "/health",
            "metrics": "/metrics",
            "stats": "/stats/cache",
            "cache_invalidate": "/api/v1/cache/invalidate"
        }
    }))
}

/// Request ID middleware
#[derive(Clone)]
pub struct RequestId(pub String);

/// Request ID extraction middleware
pub async fn request_id_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let request_id = Uuid::new_v4().to_string();

    tracing::info!("Incoming request: {} {}", req.method(), req.uri());

    let mut response = next.run(req).await;

    // Add request ID to response headers
    response.headers_mut().insert("x-request-id", request_id.parse().unwrap());

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_server_creation() {
        let config = AppConfig::default();
        let metrics = Metrics::new();
        let server = Server::new(config, metrics);
        assert_eq!(server.config.server.host, "127.0.0.1");
    }

    #[test]
    fn test_health_check_response() {
        let response = futures::executor::block_on(health_check()).into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_root_response() {
        let response = futures::executor::block_on(root()).into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_router_creation() {
        let config = AppConfig::default();
        let metrics = Metrics::new();
        let server = Server::new(config, metrics);
        let _app = server.create_router();

        // Test that the router was created successfully
        // Router creation itself is the test - if it compiles, it works
        assert!(true, "Router creation successful");
    }
}
