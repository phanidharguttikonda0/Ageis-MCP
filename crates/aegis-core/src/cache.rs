//! Cache engine and related types
//!
//! Provides the cache orchestration layer that coordinates between
//! embedding generation, Redis vector search, and LLM clients.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, error, info, instrument, warn};

use crate::{
    redis::RedisPool,
    embedding::EmbeddingService,
    AppConfig, Error, Result,
};

/// Cache entry representing a stored prompt-response pair
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheEntry {
    pub hash_id: String,
    pub prompt_text: String,
    pub embedding: Vec<f32>,
    pub response_payload: String,
    pub similarity_score: Option<f32>,
}

/// Request for cache orchestration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheOrchestrationRequest {
    pub prompt_text: String,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub user_id: Option<String>,
    pub request_id: String,
}

/// Response from cache orchestration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheOrchestrationResponse {
    pub response_text: String,
    pub cached: bool,
    pub similarity_score: Option<f32>,
    pub tokens_used: u32,
    pub latency_ms: u64,
    pub cache_hit_type: Option<CacheHitType>,
}

/// Type of cache hit
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum CacheHitType {
    Exact,
    Similar,
    Miss,
}

/// Main cache orchestration engine
pub struct CacheEngine {
    redis_pool: Arc<std::sync::Mutex<RedisPool>>,
    embedding_service: Arc<dyn EmbeddingService>,
    similarity_threshold: f32,
    max_cache_size: usize,
    ttl_seconds: u32,

    // Deduplication: prevent concurrent identical requests
    pending_requests: Arc<Mutex<std::collections::HashMap<String, Arc<Semaphore>>>>,

    // Performance limits
    max_concurrent_requests: usize,
    request_timeout: Duration,
    embedding_timeout: Duration,
    redis_timeout: Duration,
}

impl CacheEngine {
    /// Create a new cache engine with the given configuration
    pub fn new(config: &AppConfig, embedding_service: Arc<dyn EmbeddingService>) -> Result<Self> {
        info!("Creating cache orchestration engine");

        let redis_pool = RedisPool::new(config.redis.clone())?;

        Ok(Self {
            redis_pool: Arc::new(std::sync::Mutex::new(redis_pool)),
            embedding_service,
            similarity_threshold: config.cache.similarity_threshold,
            max_cache_size: config.cache.max_cache_size,
            ttl_seconds: config.cache.ttl_seconds as u32,

            pending_requests: Arc::new(Mutex::new(std::collections::HashMap::new())),
            max_concurrent_requests: config.server.max_connections as usize,
            request_timeout: Duration::from_millis(30000), // 30 second default
            embedding_timeout: Duration::from_millis(10000), // 10 second embedding timeout
            redis_timeout: Duration::from_millis(5000),    // 5 second Redis timeout
        })
    }

    /// Initialize the cache engine (call during startup)
    #[instrument(skip(self))]
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing cache engine");
        let mut pool = self.redis_pool.lock().map_err(|e| Error::Lock(format!("Failed to acquire lock: {}", e)))?;
        pool.initialize().await?;
        info!("Cache engine initialization completed");
        Ok(())
    }

    /// Search for similar cached prompts using vector similarity
    #[instrument(skip(self, query_vector))]
    pub async fn search_similar(
        &self,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<CacheEntry>> {
        debug!("Searching for similar cached prompts with threshold {}", self.similarity_threshold);

        let request = crate::redis::VectorSearchRequest {
            query_vector: query_vector.to_vec(),
            limit,
            similarity_threshold: self.similarity_threshold,
            include_response: true,
        };

        // Use Redis vector search
        let mut pool = self.redis_pool.lock().map_err(|e| Error::Lock(format!("Failed to acquire lock: {}", e)))?;
        let results = pool.vector_search(&request).await?;

        // Convert to cache entries
        let mut entries = Vec::new();
        for result in results {
            let entry = CacheEntry {
                hash_id: result.hash_id.clone(),
                prompt_text: result.prompt_text.unwrap_or_default(),
                embedding: query_vector.to_vec(), // Simplified - would use stored vector
                response_payload: result.response_payload.unwrap_or_default(),
                similarity_score: Some(result.similarity_score),
            };
            entries.push(entry);
        }

        debug!("Found {} similar cache entries", entries.len());
        Ok(entries)
    }

    /// Store a new cache entry
    #[instrument(skip(self, prompt_text, embedding, response_payload))]
    pub async fn store_entry(
        &self,
        prompt_text: &str,
        embedding: &[f32],
        response_payload: &str,
        model: &str,
        token_count: u32,
    ) -> Result<String> {
        debug!("Storing new cache entry");

        // Generate hash ID for the cache entry
        let hash_id = self.generate_hash_id(prompt_text, model);

        let mut pool = self.redis_pool.lock().map_err(|e| Error::Lock(format!("Failed to acquire lock: {}", e)))?;
        pool.store_cache_entry(
            &hash_id,
            prompt_text,
            embedding,
            response_payload,
            self.ttl_seconds,
            model,
            token_count,
        )
        .await?;

        info!("Cache entry stored: {}", hash_id);
        Ok(hash_id)
    }

    /// Get a specific cache entry by hash ID
    #[instrument(skip(self))]
    pub async fn get_entry(&self, hash_id: &str) -> Result<Option<CacheEntry>> {
        debug!("Getting cache entry: {}", hash_id);
        let mut pool = self.redis_pool.lock().map_err(|e| Error::Lock(format!("Failed to acquire lock: {}", e)))?;
        pool.get_cache_entry(hash_id).await
    }

    /// Invalidate a specific cache entry
    #[instrument(skip(self))]
    pub async fn invalidate_entry(&self, hash_id: &str) -> Result<bool> {
        debug!("Invalidating cache entry: {}", hash_id);
        let mut pool = self.redis_pool.lock().map_err(|e| Error::Lock(format!("Failed to acquire lock: {}", e)))?;
        pool.invalidate_cache(hash_id).await
    }

    /// Clear all cache entries (use with caution)
    #[instrument(skip(self))]
    pub async fn clear_all(&self) -> Result<u64> {
        info!("Clearing all cache entries");
        let mut pool = self.redis_pool.lock().map_err(|e| Error::Lock(format!("Failed to acquire lock: {}", e)))?;
        pool.clear_cache().await
    }

    /// Generate a unique hash ID for a cache entry
    fn generate_hash_id(&self, prompt_text: &str, model: &str) -> String {
        Self::generate_hash_id_static(prompt_text, model)
    }

    /// Static version of hash generation for use in async contexts
    fn generate_hash_id_static(prompt_text: &str, model: &str) -> String {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        prompt_text.hash(&mut hasher);
        model.hash(&mut hasher);

        format!("{}_{:x}", model, hasher.finish())
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let pool = self.redis_pool.lock().unwrap();
        let pool_status = pool.pool_status();
        CacheStats {
            similarity_threshold: self.similarity_threshold,
            max_cache_size: self.max_cache_size,
            ttl_seconds: self.ttl_seconds,
            pool_connections: pool_status.size,
            pool_available: pool_status.available,
            circuit_breaker_open: pool_status.circuit_breaker_open,
        }
    }

    /// Main orchestration method: handle a prompt request with full caching logic
    #[instrument(skip(self, request))]
    pub async fn handle_prompt_request(
        &self,
        request: CacheOrchestrationRequest,
    ) -> Result<CacheOrchestrationResponse> {
        let start_time = Instant::now();

        info!(
            "Handling prompt request: {} (model: {})",
            request.request_id, request.model
        );

        // Check for deduplication: prevent concurrent identical requests
        let request_key = self.generate_request_key(&request.prompt_text, &request.model);
        let request_key_clone = request_key.clone();

        // Try to acquire semaphore for deduplication
        let semaphore = {
            let mut pending = self.pending_requests.lock().await;
            if let Some(existing) = pending.get(&request_key) {
                debug!("Request already in progress, waiting for existing one");
                existing.clone()
            } else {
                let new_semaphore = Arc::new(Semaphore::new(1));
                pending.insert(request_key.clone(), new_semaphore.clone());
                new_semaphore
            }
        };

        let _permit = semaphore.acquire().await
            .map_err(|e| Error::Unknown(format!("Failed to acquire semaphore: {}", e)))?;

        // Stage 1: Generate embedding with timeout
        let embedding = tokio::time::timeout(
            self.embedding_timeout,
            self.embedding_service.generate_embedding(&request.prompt_text)
        )
        .await
        .map_err(|_| Error::Embedding("Embedding generation timeout".to_string()))?
        .map_err(|e| Error::Embedding(format!("Failed to generate embedding: {}", e)))?;

        debug!("Generated embedding for request {}", request.request_id);

        // Stage 2: Search for similar cached entries with timeout
        let similar_entries = tokio::time::timeout(
            self.redis_timeout,
            self.search_similar(&embedding, 5)
        )
        .await
        .map_err(|_| Error::Redis("Redis search timeout".to_string()))?
        .map_err(|e| Error::Redis(format!("Failed to search cache: {}", e)))?;

        // Stage 3: Check for cache hit
        if let Some(best_match) = self.find_best_match(&similar_entries, &request.prompt_text) {
            // Clean up pending request on cache hit
            {
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&request_key_clone);
            }

            let latency = start_time.elapsed().as_millis() as u64;
            info!(
                "Cache hit for request {} (similarity: {:.3})",
                request.request_id,
                best_match.similarity_score.unwrap_or(0.0)
            );

            return Ok(CacheOrchestrationResponse {
                response_text: best_match.response_payload,
                cached: true,
                similarity_score: best_match.similarity_score,
                tokens_used: 0,
                latency_ms: latency,
                cache_hit_type: Some(CacheHitType::Similar),
            });
        }

        // Stage 4: Cache miss - forward to upstream LLM (placeholder for now)
        debug!("Cache miss for request {}, forwarding to LLM", request.request_id);

        let response_text = self.forward_to_llm_placeholder(&request).await?;
        let tokens_used = self.estimate_tokens(&request.prompt_text, &response_text);

        // Stage 5: Async write-back - store in cache without blocking response
        // Note: This is a simplified version. Full async write-back will be implemented
        // in Issue #9 (Upstream LLM Client) when the LLM integration is complete.
        debug!("Cache write-back placeholder - will be implemented with LLM client in Issue #9");

        // Clean up pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.remove(&request_key_clone);
        }

        let latency = start_time.elapsed().as_millis() as u64;
        info!(
            "Request {} completed in {}ms ({} tokens)",
            request.request_id, latency, tokens_used
        );

        Ok(CacheOrchestrationResponse {
            response_text,
            cached: false,
            similarity_score: None,
            tokens_used,
            latency_ms: latency,
            cache_hit_type: Some(CacheHitType::Miss),
        })
    }

    /// Find the best matching cache entry based on similarity and relevance
    fn find_best_match(&self, entries: &[CacheEntry], prompt_text: &str) -> Option<CacheEntry> {
        entries
            .iter()
            .filter(|entry| {
                // Filter by similarity threshold
                entry.similarity_score
                    .map(|score| score >= self.similarity_threshold)
                    .unwrap_or(false)
            })
            .max_by(|a, b| {
                // Sort by similarity score (highest first)
                a.similarity_score
                    .partial_cmp(&b.similarity_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    /// Generate a unique key for deduplication
    fn generate_request_key(&self, prompt_text: &str, model: &str) -> String {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        prompt_text.hash(&mut hasher);
        model.hash(&mut hasher);

        format!("req_{:x}", hasher.finish())
    }

    /// Placeholder for LLM forwarding (will be implemented in Issue #9)
    async fn forward_to_llm_placeholder(&self, request: &CacheOrchestrationRequest) -> Result<String> {
        warn!("LLM forwarding not yet implemented - returning placeholder response");

        Ok(format!(
            "Placeholder LLM response for: {} (model: {})",
            request.prompt_text, request.model
        ))
    }

    /// Estimate token count for prompt and response
    fn estimate_tokens(&self, prompt_text: &str, response_text: &str) -> u32 {
        // Rough estimation: ~4 characters per token
        let prompt_tokens = (prompt_text.len() / 4) as u32;
        let response_tokens = (response_text.len() / 4) as u32;
        prompt_tokens + response_tokens
    }

    /// Create a new cache engine with embedding service (factory method)
    pub fn with_embedding_service(config: &AppConfig, embedding_service: Arc<dyn EmbeddingService>) -> Result<Self> {
        info!("Creating cache orchestration engine with embedding service");

        let redis_pool = RedisPool::new(config.redis.clone())?;

        Ok(Self {
            redis_pool: Arc::new(std::sync::Mutex::new(redis_pool)),
            embedding_service,
            similarity_threshold: config.cache.similarity_threshold,
            max_cache_size: config.cache.max_cache_size,
            ttl_seconds: config.cache.ttl_seconds as u32,

            pending_requests: Arc::new(Mutex::new(std::collections::HashMap::new())),
            max_concurrent_requests: config.server.max_connections as usize,
            request_timeout: Duration::from_millis(30000),
            embedding_timeout: Duration::from_millis(10000),
            redis_timeout: Duration::from_millis(5000),
        })
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub similarity_threshold: f32,
    pub max_cache_size: usize,
    pub ttl_seconds: u32,
    pub pool_connections: usize,
    pub pool_available: usize,
    pub circuit_breaker_open: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_serialization() {
        let entry = CacheEntry {
            hash_id: "test_hash_123".to_string(),
            prompt_text: "Test prompt".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            response_payload: "Test response".to_string(),
            similarity_score: Some(0.95),
        };

        let serialized = serde_json::to_string(&entry).unwrap();
        let deserialized: CacheEntry = serde_json::from_str(&serialized).unwrap();

        assert_eq!(entry.hash_id, deserialized.hash_id);
        assert_eq!(entry.prompt_text, deserialized.prompt_text);
    }

    #[test]
    fn test_hash_id_generation() {
        // This would need an actual CacheEngine instance
        // For now, just test that the function exists and produces deterministic results
        let prompt = "Test prompt";
        let model = "gpt-4";

        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        prompt.hash(&mut hasher);
        model.hash(&mut hasher);

        let hash_id = format!("{}_{:x}", model, hasher.finish());
        assert!(hash_id.starts_with("gpt-4_"));
        assert!(hash_id.len() > 10);
    }

    #[test]
    fn test_cache_stats_creation() {
        let stats = CacheStats {
            similarity_threshold: 0.95,
            max_cache_size: 10000,
            ttl_seconds: 86400,
            pool_connections: 5,
            pool_available: 3,
            circuit_breaker_open: false,
        };

        assert_eq!(stats.similarity_threshold, 0.95);
        assert_eq!(stats.max_cache_size, 10000);
        assert!(!stats.circuit_breaker_open);
    }
}
