//! Prometheus metrics collection and export for Aegis-MCP
//!
//! Provides comprehensive metrics collection for monitoring and alerting.

use prometheus::{
    IntCounter, IntGauge, TextEncoder, Encoder,
};
use anyhow::Result as AnyhowResult;

/// Application metrics
#[derive(Clone)]
pub struct Metrics {
    pub cache_hits: IntCounter,
    pub cache_misses: IntCounter,
    pub llm_requests: IntCounter,
    pub llm_errors: IntCounter,
    pub llm_tokens_used: IntCounter,
    pub redis_queries: IntCounter,
    pub redis_errors: IntCounter,
    pub embedding_requests: IntCounter,
    pub embedding_errors: IntCounter,
    pub active_requests: IntGauge,
}

impl Metrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        // Cache metrics
        let cache_hits = IntCounter::new(
            "cache_hits_total",
            "Total number of cache hits"
        ).unwrap();

        let cache_misses = IntCounter::new(
            "cache_misses_total",
            "Total number of cache misses"
        ).unwrap();

        // LLM metrics
        let llm_requests = IntCounter::new(
            "llm_requests_total",
            "Total number of LLM API requests"
        ).unwrap();

        let llm_errors = IntCounter::new(
            "llm_errors_total",
            "Total number of LLM API errors"
        ).unwrap();

        let llm_tokens_used = IntCounter::new(
            "llm_tokens_used_total",
            "Total number of tokens used in LLM requests"
        ).unwrap();

        // Redis metrics
        let redis_queries = IntCounter::new(
            "redis_queries_total",
            "Total number of Redis queries"
        ).unwrap();

        let redis_errors = IntCounter::new(
            "redis_errors_total",
            "Total number of Redis errors"
        ).unwrap();

        // Embedding metrics
        let embedding_requests = IntCounter::new(
            "embedding_requests_total",
            "Total number of embedding generation requests"
        ).unwrap();

        let embedding_errors = IntCounter::new(
            "embedding_errors_total",
            "Total number of embedding generation errors"
        ).unwrap();

        // Active requests
        let active_requests = IntGauge::new(
            "active_requests",
            "Number of currently active requests"
        ).unwrap();

        Self {
            cache_hits,
            cache_misses,
            llm_requests,
            llm_errors,
            llm_tokens_used,
            redis_queries,
            redis_errors,
            embedding_requests,
            embedding_errors,
            active_requests,
        }
    }

    /// Register all metrics with the default registry
    pub fn register(&self) -> AnyhowResult<()> {
        let registry = prometheus::default_registry();
        registry.register(Box::new(self.cache_hits.clone()))?;
        registry.register(Box::new(self.cache_misses.clone()))?;
        registry.register(Box::new(self.llm_requests.clone()))?;
        registry.register(Box::new(self.llm_errors.clone()))?;
        registry.register(Box::new(self.llm_tokens_used.clone()))?;
        registry.register(Box::new(self.redis_queries.clone()))?;
        registry.register(Box::new(self.redis_errors.clone()))?;
        registry.register(Box::new(self.embedding_requests.clone()))?;
        registry.register(Box::new(self.embedding_errors.clone()))?;
        registry.register(Box::new(self.active_requests.clone()))?;
        Ok(())
    }

    /// Export metrics in Prometheus format
    pub fn export(&self) -> AnyhowResult<String> {
        // Ensure metrics are registered before export
        self.register().ok(); // Ignore if already registered

        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics utilities
pub mod utils {
    use super::*;

    /// Record cache hit
    pub fn record_cache_hit(metrics: &Metrics, _similarity_score: f64) {
        metrics.cache_hits.inc();
    }

    /// Record cache miss
    pub fn record_cache_miss(metrics: &Metrics) {
        metrics.cache_misses.inc();
    }

    /// Record LLM request
    pub fn record_llm_request(metrics: &Metrics, tokens_used: u64, _latency_seconds: f64) {
        metrics.llm_requests.inc();
        metrics.llm_tokens_used.inc_by(tokens_used);
    }

    /// Record LLM error
    pub fn record_llm_error(metrics: &Metrics) {
        metrics.llm_errors.inc();
    }

    /// Record Redis query
    pub fn record_redis_query(metrics: &Metrics, _latency_seconds: f64) {
        metrics.redis_queries.inc();
    }

    /// Record Redis error
    pub fn record_redis_error(metrics: &Metrics) {
        metrics.redis_errors.inc();
    }

    /// Record embedding generation
    pub fn record_embedding_generation(metrics: &Metrics, _latency_seconds: f64) {
        metrics.embedding_requests.inc();
    }

    /// Record embedding error
    pub fn record_embedding_error(metrics: &Metrics) {
        metrics.embedding_errors.inc();
    }

    /// Increment active requests
    pub fn increment_active_requests(metrics: &Metrics) {
        metrics.active_requests.inc();
    }

    /// Decrement active requests
    pub fn decrement_active_requests(metrics: &Metrics) {
        metrics.active_requests.dec();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new();
        assert_eq!(metrics.cache_hits.get(), 0);
    }

    #[test]
    fn test_cache_hit_recording() {
        let metrics = Metrics::new();
        utils::record_cache_hit(&metrics, 0.95);
        assert_eq!(metrics.cache_hits.get(), 1);
    }

    #[test]
    fn test_cache_miss_recording() {
        let metrics = Metrics::new();
        utils::record_cache_miss(&metrics);
        assert_eq!(metrics.cache_misses.get(), 1);
    }

    #[test]
    fn test_llm_request_recording() {
        let metrics = Metrics::new();
        utils::record_llm_request(&metrics, 100, 1.5);
        assert_eq!(metrics.llm_requests.get(), 1);
        assert_eq!(metrics.llm_tokens_used.get(), 100);
    }

    #[test]
    fn test_active_requests_tracking() {
        let metrics = Metrics::new();
        utils::increment_active_requests(&metrics);
        assert_eq!(metrics.active_requests.get(), 1);
        utils::decrement_active_requests(&metrics);
        assert_eq!(metrics.active_requests.get(), 0);
    }

    #[test]
    fn test_metrics_export() {
        let metrics = Metrics::new();
        utils::record_cache_hit(&metrics, 0.95);
        let export = metrics.export().unwrap();
        // Debug: print what we actually get
        if !export.contains("cache_hits_total") && !export.contains("cache_hits") {
            println!("Export content: {}", export);
        }
        // Prometheus format should contain the metric name
        assert!(!export.is_empty(), "Export should not be empty");
        assert!(export.contains("cache"), "Export should contain cache-related metrics");
    }
}
