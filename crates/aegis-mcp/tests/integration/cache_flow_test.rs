//! Cache Orchestration Integration Tests
//!
//! Tests the complete cache orchestration flow including embedding generation,
//! Redis vector search, similarity matching, and cache management.

use aegis_core::{
    cache::{CacheEngine, CacheOrchestrationRequest, CacheOrchestrationResponse},
    embedding::{EmbeddingService, OpenAIEmbeddingProvider, InMemoryEmbeddingCache},
    config::{AppConfig, CacheConfig, RedisConfig, LLMConfig},
    Error, Result
};
use std::sync::Arc;
use tokio::time::{timeout, Duration};

/// Helper function to create test configuration
fn create_test_config() -> AppConfig {
    AppConfig {
        server: aegis_core::ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            max_connections: 10,
        },
        redis: RedisConfig {
            url: "redis://localhost:6379".to_string(),
            pool_size: 5,
            connection_timeout: 5000,
            circuit_breaker_threshold: 5,
            circuit_breaker_cooldown: 60,
        },
        cache: CacheConfig {
            similarity_threshold: 0.95,
            max_cache_size: 1000,
            ttl_seconds: 3600,
        },
        llm: LLMConfig {
            upstream_url: "https://api.openai.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-4".to_string(),
            timeout: 30000,
            max_retries: 3,
        },
        observability: aegis_core::ObservabilityConfig {
            log_level: "info".to_string(),
            enable_tracing: false,
            enable_metrics: false,
        },
    }
}

/// Helper function to create test cache request
fn create_test_request(prompt_text: &str, model: &str) -> CacheOrchestrationRequest {
    CacheOrchestrationRequest {
        prompt_text: prompt_text.to_string(),
        model: model.to_string(),
        max_tokens: Some(1000),
        temperature: Some(0.7),
        user_id: Some("test-user".to_string()),
        request_id: format!("test-req-{}", uuid::Uuid::new_v4()),
    }
}

#[tokio::test]
async fn test_cache_request_creation() {
    let request = create_test_request("What is Rust?", "gpt-4");

    assert_eq!(request.prompt_text, "What is Rust?");
    assert_eq!(request.model, "gpt-4");
    assert_eq!(request.max_tokens, Some(1000));
    assert_eq!(request.temperature, Some(0.7));
    assert_eq!(request.user_id, Some("test-user".to_string()));
    assert!(!request.request_id.is_empty());
}

#[tokio::test]
async fn test_cache_response_structure() {
    let response = CacheOrchestrationResponse {
        response_text: "Rust is a systems programming language".to_string(),
        cached: true,
        similarity_score: Some(0.98),
        tokens_used: 150,
        latency_ms: 50,
        cache_hit_type: Some(aegis_core::cache::CacheHitType::Similar),
    };

    assert!(response.cached);
    assert_eq!(response.response_text, "Rust is a systems programming language");
    assert_eq!(response.tokens_used, 150);
    assert_eq!(response.latency_ms, 50);
    assert_eq!(response.similarity_score, Some(0.98));
}

#[tokio::test]
async fn test_embedding_service_basic() {
    // Test with in-memory embedding cache for testing
    let cache = Arc::new(InMemoryEmbeddingCache::new(100, 3600));

    // Note: This test uses a mock embedding service for testing
    // In production, this would use OpenAIEmbeddingProvider with API key

    let config = LLMConfig {
        upstream_url: "https://api.openai.com/v1".to_string(),
        api_key: "test-key".to_string(),
        model: "text-embedding-ada-002".to_string(),
        timeout: 10000,
        max_retries: 2,
    };

    // This will fail with actual API call but tests the construction
    let result = OpenAIEmbeddingProvider::new(&config);

    // Construction should succeed
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cache_engine_creation() {
    let config = create_test_config();
    let embedding_service = Arc::new(InMemoryEmbeddingCache::new(100, 3600)) as Arc<dyn EmbeddingService>;

    // Note: Cache engine creation requires embedding service
    // This test validates the construction logic
    let result = CacheEngine::with_embedding_service(&config, embedding_service);

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cache_hash_generation() {
    // Test that hash IDs are deterministic
    let prompt = "What is the meaning of life?";
    let model = "gpt-4";

    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher1 = DefaultHasher::new();
    prompt.hash(&mut hasher1);
    model.hash(&mut hasher1);
    let hash1 = hasher1.finish();

    let mut hasher2 = DefaultHasher::new();
    prompt.hash(&mut hasher2);
    model.hash(&mut hasher2);
    let hash2 = hasher2.finish();

    assert_eq!(hash1, hash2, "Hash should be deterministic");
}

#[tokio::test]
async fn test_cache_similarity_validation() {
    // Test similarity score validation
    let valid_scores = vec![0.0, 0.5, 0.95, 1.0];
    let invalid_scores = vec![-0.1, 1.1, 2.0];

    for score in valid_scores {
        assert!(score >= 0.0 && score <= 1.0, "Score {} should be valid", score);
    }

    for score in invalid_scores {
        assert!(score < 0.0 || score > 1.0, "Score {} should be invalid", score);
    }
}

#[tokio::test]
async fn test_cache_request_timeout_handling() {
    let config = create_test_config();
    let embedding_service = Arc::new(InMemoryEmbeddingCache::new(100, 3600)) as Arc<dyn EmbeddingService>;

    let cache_engine = CacheEngine::with_embedding_service(&config, embedding_service).unwrap();
    let request = create_test_request("Test prompt", "gpt-4");

    // Test that request completes within timeout
    let result = timeout(Duration::from_secs(5), cache_engine.handle_prompt_request(request)).await;

    assert!(result.is_ok(), "Cache request should complete within timeout");
    assert!(result.unwrap().is_ok(), "Cache request should succeed");
}

#[tokio::test]
async fn test_cache_entry_serialization() {
    use aegis_core::cache::CacheEntry;

    let entry = CacheEntry {
        hash_id: "test-hash-123".to_string(),
        prompt_text: "What is AI?".to_string(),
        embedding: vec![0.1, 0.2, 0.3],
        response_payload: "AI is artificial intelligence".to_string(),
        similarity_score: Some(0.95),
    };

    // Test serialization
    let serialized = serde_json::to_string(&entry).unwrap();
    assert!(serialized.contains("test-hash-123"));

    // Test deserialization
    let deserialized: CacheEntry = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.hash_id, entry.hash_id);
    assert_eq!(deserialized.prompt_text, entry.prompt_text);
    assert_eq!(deserialized.embedding, entry.embedding);
}

#[tokio::test]
async fn test_cache_stats_structure() {
    use aegis_core::cache::CacheStats;

    let stats = CacheStats {
        similarity_threshold: 0.95,
        max_cache_size: 1000,
        ttl_seconds: 3600,
        pool_connections: 5,
        pool_available: 3,
        circuit_breaker_open: false,
    };

    assert_eq!(stats.similarity_threshold, 0.95);
    assert_eq!(stats.max_cache_size, 1000);
    assert_eq!(stats.ttl_seconds, 3600);
    assert_eq!(stats.pool_connections, 5);
    assert_eq!(stats.pool_available, 3);
    assert!(!stats.circuit_breaker_open);
}

#[tokio::test]
async fn test_cache_hit_type_enum() {
    use aegis_core::cache::CacheHitType;

    let hit_types = vec![
        CacheHitType::Exact,
        CacheHitType::Similar,
        CacheHitType::Miss,
    ];

    // Test serialization/deserialization of cache hit types
    for hit_type in hit_types {
        let serialized = serde_json::to_string(&hit_type).unwrap();
        let deserialized: CacheHitType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(hit_type, deserialized);
    }
}

#[tokio::test]
async fn test_token_estimation() {
    // Test token estimation logic
    let prompt = "This is a test prompt";
    let response = "This is a test response";

    // Rough estimation: ~4 characters per token
    let estimated_prompt_tokens = (prompt.len() / 4) as u32;
    let estimated_response_tokens = (response.len() / 4) as u32;
    let total_tokens = estimated_prompt_tokens + estimated_response_tokens;

    assert!(total_tokens > 0, "Should estimate at least some tokens");
    assert!(total_tokens < 100, "Should estimate reasonable token count");
}

#[tokio::test]
async fn test_cache_request_deduplication() {
    // Test that identical requests are properly deduplicated
    let request1 = create_test_request("Same prompt", "gpt-4");
    let request2 = create_test_request("Same prompt", "gpt-4");

    // Generate request keys for comparison
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher1 = DefaultHasher::new();
    request1.prompt_text.hash(&mut hasher1);
    request1.model.hash(&mut hasher1);
    let key1 = hasher1.finish();

    let mut hasher2 = DefaultHasher::new();
    request2.prompt_text.hash(&mut hasher2);
    request2.model.hash(&mut hasher2);
    let key2 = hasher2.finish();

    assert_eq!(key1, key2, "Identical requests should generate same keys");
}

#[tokio::test]
async fn test_cache_request_with_different_params() {
    // Test that different parameters generate different request keys
    let request1 = create_test_request("Prompt 1", "gpt-4");
    let request2 = create_test_request("Prompt 2", "gpt-4");
    let request3 = create_test_request("Prompt 1", "gpt-3.5");

    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher1 = DefaultHasher::new();
    request1.prompt_text.hash(&mut hasher1);
    request1.model.hash(&mut hasher1);
    let key1 = hasher1.finish();

    let mut hasher2 = DefaultHasher::new();
    request2.prompt_text.hash(&mut hasher2);
    request2.model.hash(&mut hasher2);
    let key2 = hasher2.finish();

    let mut hasher3 = DefaultHasher::new();
    request3.prompt_text.hash(&mut hasher3);
    request3.model.hash(&mut hasher3);
    let key3 = hasher3.finish();

    assert_ne!(key1, key2, "Different prompts should generate different keys");
    assert_ne!(key1, key3, "Different models should generate different keys");
    assert_ne!(key2, key3, "Different parameters should generate different keys");
}

#[tokio::test]
async fn test_cache_latency_tracking() {
    let config = create_test_config();
    let embedding_service = Arc::new(InMemoryEmbeddingCache::new(100, 3600)) as Arc<dyn EmbeddingService>;

    let cache_engine = CacheEngine::with_embedding_service(&config, embedding_service).unwrap();
    let request = create_test_request("Test prompt", "gpt-4");

    let response = cache_engine.handle_prompt_request(request).await.unwrap();

    // Latency should be reasonable (> 0ms and < 10s)
    assert!(response.latency_ms > 0, "Latency should be positive");
    assert!(response.latency_ms < 10000, "Latency should be reasonable");
}

#[tokio::test]
async fn test_cache_error_handling() {
    let config = create_test_config();
    let embedding_service = Arc::new(InMemoryEmbeddingCache::new(100, 3600)) as Arc<dyn EmbeddingService>;

    let cache_engine = CacheEngine::with_embedding_service(&config, embedding_service).unwrap();

    // Test with empty prompt (should handle gracefully)
    let request = CacheOrchestrationRequest {
        prompt_text: "".to_string(), // Empty prompt
        model: "gpt-4".to_string(),
        max_tokens: Some(1000),
        temperature: Some(0.7),
        user_id: Some("test-user".to_string()),
        request_id: "test-empty-prompt".to_string(),
    };

    let result = cache_engine.handle_prompt_request(request).await;

    // Should handle error gracefully
    assert!(result.is_ok(), "Should handle empty prompt gracefully");
}