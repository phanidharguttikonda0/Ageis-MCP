//! Embedding generation service
//!
//! Provides vector embedding generation for semantic similarity search with support
//! for multiple providers including OpenAI and local models.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

use crate::{Error, Result, LLMConfig};

/// Default embedding model (OpenAI text-embedding-ada-002)
pub const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-ada-002";
/// Default embedding dimension (OpenAI text-embedding-ada-002)
pub const DEFAULT_EMBEDDING_DIMENSION: usize = 1536;
/// Maximum text length for embeddings (in characters)
pub const MAX_EMBEDDING_TEXT_LENGTH: usize = 8191;

/// Request for embedding generation
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingRequest {
    pub text: String,
    pub model: String,
}

impl Default for EmbeddingRequest {
    fn default() -> Self {
        Self {
            text: String::new(),
            model: DEFAULT_EMBEDDING_MODEL.to_string(),
        }
    }
}

/// Embedding response from upstream API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingResponse {
    pub embedding: Vec<f32>,
    pub model: String,
    pub tokens_used: u32,
    pub latency_ms: u64,
}

/// Embedding generation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingStats {
    pub total_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub total_tokens: u64,
    pub total_latency_ms: u64,
    pub average_latency_ms: f64,
}

impl Default for EmbeddingStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            cache_hits: 0,
            cache_misses: 0,
            total_tokens: 0,
            total_latency_ms: 0,
            average_latency_ms: 0.0,
        }
    }
}

/// Embedding generation service - will be fully implemented in Issue #6
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>>;
    async fn generate_embedding_with_response(&self, text: &str) -> Result<EmbeddingResponse>;
    fn get_stats(&self) -> EmbeddingStats;
}

/// OpenAI embedding provider
pub struct OpenAIEmbeddingProvider {
    client: Client,
    api_key: String,
    model: String,
    cache: Arc<RwLock<HashMap<String, Vec<f32>>>>,
    stats: Arc<RwLock<EmbeddingStats>>,
    timeout: Duration,
    max_retries: u32,
}

impl OpenAIEmbeddingProvider {
    /// Create a new OpenAI embedding provider
    pub fn new(config: &LLMConfig) -> Result<Self> {
        info!("Creating OpenAI embedding provider with model: {}", config.model);

        if config.api_key.is_empty() {
            return Err(Error::Embedding("OpenAI API key is required".to_string()));
        }

        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout))
            .build()
            .map_err(|e| Error::Embedding(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            api_key: config.api_key.clone(),
            model: if config.model.contains("embedding") {
                config.model.clone()
            } else {
                DEFAULT_EMBEDDING_MODEL.to_string()
            },
            cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(EmbeddingStats::default())),
            timeout: Duration::from_millis(config.timeout),
            max_retries: config.max_retries,
        })
    }

    /// Generate embedding with retries and exponential backoff
    async fn generate_with_retry(&self, text: &str) -> Result<EmbeddingResponse> {
        let start = std::time::Instant::now();

        for attempt in 0..self.max_retries {
            let result = self.call_openai_api(text).await;

            if result.is_ok() || attempt == self.max_retries - 1 {
                let latency = start.elapsed().as_millis() as u64;
                let mut response = result?;

                response.latency_ms = latency;
                return Ok(response);
            }

            // Exponential backoff
            let backoff_ms = 1000 * 2_u64.pow(attempt as u32);
            warn!("Embedding generation attempt {} failed, retrying in {}ms", attempt + 1, backoff_ms);
            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
        }

        Err(Error::Embedding("Max retries exceeded".to_string()))
    }

    /// Call OpenAI API
    async fn call_openai_api(&self, text: &str) -> Result<EmbeddingResponse> {
        let request_body = OpenAIEmbeddingRequest {
            model: self.model.clone(),
            input: text.to_string(),
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| Error::Embedding(format!("Failed to send request: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| Error::Embedding(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(Error::Embedding(format!(
                "OpenAI API error ({}): {}",
                status, response_text
            )));
        }

        let openai_response: OpenAIEmbeddingResponse = serde_json::from_str(&response_text)
            .map_err(|e| Error::Embedding(format!("Failed to parse response: {}", e)))?;

        if openai_response.data.is_empty() {
            return Err(Error::Embedding("No embedding data in response".to_string()));
        }

        let embedding = openai_response.data[0].embedding.clone();
        let tokens_used = openai_response.usage.total_tokens;

        // Validate embedding dimension
        if embedding.len() != DEFAULT_EMBEDDING_DIMENSION {
            return Err(Error::Embedding(format!(
                "Invalid embedding dimension: expected {}, got {}",
                DEFAULT_EMBEDDING_DIMENSION,
                embedding.len()
            )));
        }

        Ok(EmbeddingResponse {
            embedding,
            model: self.model.clone(),
            tokens_used,
            latency_ms: 0, // Will be set by caller
        })
    }

    /// Update statistics
    async fn update_stats(&self, tokens_used: u32, latency_ms: u64, cache_hit: bool) {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;

        if cache_hit {
            stats.cache_hits += 1;
        } else {
            stats.cache_misses += 1;
            stats.total_tokens += tokens_used as u64;
            stats.total_latency_ms += latency_ms;

            if stats.total_requests > 0 {
                stats.average_latency_ms =
                    stats.total_latency_ms as f64 / stats.total_requests as f64;
            }
        }
    }

    /// Clear the embedding cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        let size = cache.len();
        cache.clear();
        info!("Cleared embedding cache ({} entries)", size);
    }

    /// Get cache size
    pub async fn cache_size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
}

#[async_trait]
impl EmbeddingService for OpenAIEmbeddingProvider {
    #[instrument(skip(self, text))]
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let response = self.generate_embedding_with_response(text).await?;
        Ok(response.embedding)
    }

    #[instrument(skip(self, text))]
    async fn generate_embedding_with_response(&self, text: &str) -> Result<EmbeddingResponse> {
        debug!("Generating embedding for text of length {}", text.len());

        // Validate text length
        if text.len() > MAX_EMBEDDING_TEXT_LENGTH {
            return Err(Error::Embedding(format!(
                "Text too long: {} characters (max: {})",
                text.len(),
                MAX_EMBEDDING_TEXT_LENGTH
            )));
        }

        // Check cache first
        let cache_key = format!("{}:{}", self.model, text);
        {
            let cache = self.cache.read().await;
            if let Some(embedding) = cache.get(&cache_key) {
                debug!("Cache hit for embedding request");
                self.update_stats(0, 0, true).await;
                return Ok(EmbeddingResponse {
                    embedding: embedding.clone(),
                    model: self.model.clone(),
                    tokens_used: 0,
                    latency_ms: 0,
                });
            }
        }

        // Generate new embedding
        debug!("Cache miss, calling embedding API");
        let response = self.generate_with_retry(text).await?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, response.embedding.clone());
        }

        // Update statistics
        self.update_stats(response.tokens_used, response.latency_ms, false).await;

        info!(
            "Generated embedding: {} tokens, {}ms latency",
            response.tokens_used, response.latency_ms
        );

        Ok(response)
    }

    fn get_stats(&self) -> EmbeddingStats {
        // Note: This is a synchronous method, so we can't await
        // In production, you might want to use a different approach
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.stats.read().await.clone()
            })
        })
    }
}

/// OpenAI API request format
#[derive(Debug, Serialize)]
struct OpenAIEmbeddingRequest {
    model: String,
    input: String,
}

/// OpenAI API response format
#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<OpenAIEmbeddingData>,
    usage: OpenAIEmbeddingUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingUsage {
    total_tokens: u32,
    prompt_tokens: u32,
}

/// Simple in-memory embedding cache
pub struct InMemoryEmbeddingCache {
    cache: Arc<RwLock<HashMap<String, Vec<f32>>>>,
    max_size: usize,
    ttl_seconds: u64,
}

impl InMemoryEmbeddingCache {
    /// Create a new in-memory cache
    pub fn new(max_size: usize, ttl_seconds: u64) -> Self {
        info!(
            "Creating in-memory embedding cache (max_size: {}, ttl: {}s)",
            max_size, ttl_seconds
        );

        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            ttl_seconds,
        }
    }

    /// Get embedding from cache
    pub async fn get(&self, key: &str) -> Option<Vec<f32>> {
        let cache = self.cache.read().await;
        cache.get(key).cloned()
    }

    /// Store embedding in cache
    pub async fn set(&self, key: String, embedding: Vec<f32>) {
        let mut cache = self.cache.write().await;

        // Simple cache eviction when full
        if cache.len() >= self.max_size {
            // Remove a random entry
            let key_to_remove = cache.keys().next().cloned();
            if let Some(key) = key_to_remove {
                cache.remove(&key);
            }
        }

        cache.insert(key, embedding);
    }

    /// Clear cache
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Get cache size
    pub async fn size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_request_default() {
        let request = EmbeddingRequest::default();
        assert_eq!(request.model, DEFAULT_EMBEDDING_MODEL);
        assert!(request.text.is_empty());
    }

    #[test]
    fn test_embedding_stats_default() {
        let stats = EmbeddingStats::default();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
    }

    #[test]
    fn test_max_embedding_text_length() {
        assert_eq!(MAX_EMBEDDING_TEXT_LENGTH, 8191);
        assert_eq!(DEFAULT_EMBEDDING_DIMENSION, 1536);
    }

    #[tokio::test]
    async fn test_in_memory_cache() {
        let cache = InMemoryEmbeddingCache::new(10, 3600);

        // Test get on empty cache
        assert!(cache.get("test_key").await.is_none());

        // Test set and get
        let embedding = vec![0.1, 0.2, 0.3];
        cache.set("test_key".to_string(), embedding.clone()).await;
        let retrieved = cache.get("test_key").await;
        assert_eq!(retrieved, Some(embedding));

        // Test cache size
        assert_eq!(cache.size().await, 1);

        // Test clear
        cache.clear().await;
        assert_eq!(cache.size().await, 0);
    }

    #[tokio::test]
    async fn test_in_memory_cache_eviction() {
        let cache = InMemoryEmbeddingCache::new(2, 3600);

        // Fill cache to max capacity
        cache.set("key1".to_string(), vec![0.1]).await;
        cache.set("key2".to_string(), vec![0.2]).await;
        assert_eq!(cache.size().await, 2);

        // Add third item, should evict one
        cache.set("key3".to_string(), vec![0.3]).await;
        assert_eq!(cache.size().await, 2); // Should still be at max capacity
    }
}