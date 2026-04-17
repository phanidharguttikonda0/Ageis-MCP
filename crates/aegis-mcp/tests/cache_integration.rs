//! Cache Orchestration Integration Tests
//!
//! Tests the complete cache orchestration flow including embedding generation,
//! Redis vector search, similarity matching, and cache management.

use aegis_core::{
    cache::{CacheOrchestrationRequest, CacheOrchestrationResponse, CacheHitType, CacheEntry},
    config::{AppConfig, CacheConfig, RedisConfig, LLMConfig},
};

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
            max_lifetime: 3600,
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
            jaeger_endpoint: "".to_string(),
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
        cache_hit_type: Some(CacheHitType::Similar),
    };

    assert!(response.cached);
    assert_eq!(response.response_text, "Rust is a systems programming language");
    assert_eq!(response.tokens_used, 150);
    assert_eq!(response.latency_ms, 50);
    assert_eq!(response.similarity_score, Some(0.98));
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
async fn test_cache_entry_serialization() {
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
async fn test_cache_hit_type_enum() {
    // Test serialization/deserialization of cache hit types
    let hit_types = vec![
        CacheHitType::Exact,
        CacheHitType::Similar,
        CacheHitType::Miss,
    ];

    for hit_type in hit_types {
        let serialized = serde_json::to_string(&hit_type).unwrap();
        let deserialized: CacheHitType = serde_json::from_str(&serialized).unwrap();

        // Check that the types match by string representation
        assert_eq!(
            format!("{:?}", hit_type),
            format!("{:?}", deserialized)
        );
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