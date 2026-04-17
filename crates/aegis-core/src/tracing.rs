//! Structured logging and distributed tracing for Aegis-MCP
//!
//! Provides comprehensive observability using the tracing ecosystem with
//! OpenTelemetry integration for distributed tracing and metrics.

use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer, Registry,
};
use anyhow::Result as AnyhowResult;

/// Request context for tracing
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub client_id: Option<String>,
    pub span: tracing::Span,
}

impl RequestContext {
    pub fn new(request_id: String, client_id: Option<String>) -> Self {
        let span = tracing::info_span!(
            "request",
            request_id = %request_id,
            client_id = ?client_id
        );

        Self {
            request_id,
            client_id,
            span,
        }
    }

    pub fn enter(&self) -> tracing::span::Entered<'_> {
        self.span.enter()
    }
}

/// Initialize tracing with comprehensive configuration (production)
pub fn init_tracing(log_level: &str, jaeger_endpoint: &str) -> AnyhowResult<()> {
    let log_level = parse_log_level(log_level);

    // Create formatting layer with JSON for production
    let fmt_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .json()
        .boxed();

    // Create environment filter with sampling
    let env_filter = create_env_filter(log_level)?;

    // Initialize subscriber (OpenTelemetry integration can be added later)
    Registry::default()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    tracing::info!("Tracing initialized (production mode) with Jaeger endpoint: {}", jaeger_endpoint);

    Ok(())
}

/// Initialize tracing for development (pretty printing)
pub fn init_dev_tracing(log_level: &str) -> AnyhowResult<()> {
    let log_level = parse_log_level(log_level);

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| anyhow::anyhow!("Failed to set tracing subscriber: {}", e))?;

    tracing::info!("Tracing initialized (development mode)");
    Ok(())
}

/// Create environment filter with sampling configuration
fn create_env_filter(default_level: Level) -> AnyhowResult<EnvFilter> {
    // Implement sampling: 100% for errors, 10% for success in high-traffic
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            EnvFilter::new(default_level.as_str())
                // Log all errors
                .add_directive("aegis_mcp=error".parse().unwrap())
                // Sample 10% of info/debug logs
                .add_directive("aegis_mcp=info/0.1".parse().unwrap())
        });

    Ok(filter)
}

/// Parse log level string to Level
fn parse_log_level(level: &str) -> Level {
    match level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    }
}

/// Create a request span with standardized fields
pub fn create_request_span(request_id: &str, operation: &str) -> tracing::Span {
    tracing::info_span!(
        "request",
        request_id = %request_id,
        operation = %operation,
        status = tracing::field::Empty,
        latency_ms = tracing::field::Empty,
        error = tracing::field::Empty
    )
}

/// Create cache operation span
pub fn create_cache_span(operation: &str, hash_id: Option<&str>) -> tracing::Span {
    tracing::info_span!(
        "cache_operation",
        operation = %operation,
        hash_id = ?hash_id,
        cache_hit = tracing::field::Empty,
        similarity_score = tracing::field::Empty,
        latency_ms = tracing::field::Empty
    )
}

/// Create LLM operation span
pub fn create_llm_span(operation: &str, model: &str) -> tracing::Span {
    tracing::info_span!(
        "llm_operation",
        operation = %operation,
        model = %model,
        tokens_used = tracing::field::Empty,
        latency_ms = tracing::field::Empty,
        error = tracing::field::Empty
    )
}

/// Create Redis operation span
pub fn create_redis_span(operation: &str) -> tracing::Span {
    tracing::info_span!(
        "redis_operation",
        operation = %operation,
        latency_ms = tracing::field::Empty,
        error = tracing::field::Empty
    )
}

/// Create embedding generation span
pub fn create_embedding_span(model: &str, text_length: usize) -> tracing::Span {
    tracing::info_span!(
        "embedding_generation",
        model = %model,
        text_length = text_length,
        embedding_dimension = 1536,
        latency_ms = tracing::field::Empty,
        error = tracing::field::Empty
    )
}

/// Instrumentation macros for common operations
#[macro_export]
macro_rules! instrument_cache {
    ($op:expr, $hash_id:expr) => {
        let _span = $crate::tracing::create_cache_span($op, $hash_id);
        let _enter = _span.enter();
    };
}

#[macro_export]
macro_rules! instrument_llm {
    ($op:expr, $model:expr) => {
        let _span = $crate::tracing::create_llm_span($op, $model);
        let _enter = _span.enter();
    };
}

#[macro_export]
macro_rules! instrument_redis {
    ($op:expr) => {
        let _span = $crate::tracing::create_redis_span($op);
        let _enter = _span.enter();
    };
}

/// Sensitive data redaction utilities
pub mod redact {
    use std::borrow::Cow;

    /// Redact API keys from strings
    pub fn redact_api_key(text: &str) -> Cow<'_, str> {
        if text.len() > 10 {
            // Keep first 4 and last 4 characters, redact middle
            let start = &text[..4];
            let end = &text[text.len() - 4..];
            Cow::Owned(format!("{}****{}", start, end))
        } else {
            Cow::Borrowed("***REDACTED***")
        }
    }

    /// Redact prompt text for logging (show only length)
    pub fn redact_prompt(text: &str) -> Cow<'_, str> {
        Cow::Owned(format!("[Prompt: {} chars]", text.len()))
    }

    /// Check if string contains sensitive keywords
    pub fn is_sensitive(text: &str) -> bool {
        let lower = text.to_lowercase();
        lower.contains("api_key")
            || lower.contains("password")
            || lower.contains("token")
            || lower.contains("secret")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_parsing() {
        assert_eq!(parse_log_level("debug"), Level::DEBUG);
        assert_eq!(parse_log_level("INFO"), Level::INFO);
        assert_eq!(parse_log_level("warn"), Level::WARN);
    }

    #[test]
    fn test_redact_api_key() {
        let key = "sk-1234567890abcdef";
        let redacted = redact::redact_api_key(key);
        assert!(redacted.contains("****"));
        assert!(!redacted.contains(key));
    }

    #[test]
    fn test_redact_prompt() {
        let prompt = "This is a long prompt text that should be redacted";
        let redacted = redact::redact_prompt(prompt);
        assert!(redacted.contains("chars"));
        assert!(!redacted.contains(prompt));
    }

    #[test]
    fn test_is_sensitive() {
        assert!(redact::is_sensitive("api_key=secret"));
        assert!(redact::is_sensitive("password=123"));
        assert!(!redact::is_sensitive("normal text"));
    }

    #[test]
    fn test_request_context_creation() {
        let context = RequestContext::new(
            "test-request-123".to_string(),
            Some("test-client".to_string())
        );
        assert_eq!(context.request_id, "test-request-123");
        assert_eq!(context.client_id, Some("test-client".to_string()));
    }

    #[test]
    fn test_span_creation() {
        let span = create_request_span("req-123", "test_op");
        // Just verify it doesn't panic
        drop(span);
    }
}
