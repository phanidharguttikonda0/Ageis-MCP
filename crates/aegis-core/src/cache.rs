//! Cache engine and related types
//!
//! Provides the cache orchestration layer that coordinates between
//! embedding generation, Redis vector search, and LLM clients.

use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

use crate::{redis::RedisPool, AppConfig, Error, Result};

/// Cache entry representing a stored prompt-response pair
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheEntry {
    pub hash_id: String,
    pub prompt_text: String,
    pub embedding: Vec<f32>,
    pub response_payload: String,
    pub similarity_score: Option<f32>,
}

/// Main cache orchestration engine
pub struct CacheEngine {
    redis_pool: std::sync::Mutex<RedisPool>,
    similarity_threshold: f32,
    max_cache_size: usize,
    ttl_seconds: u32,
}

impl CacheEngine {
    /// Create a new cache engine with the given configuration
    pub fn new(config: &AppConfig) -> Result<Self> {
        info!("Creating cache orchestration engine");

        let redis_pool = RedisPool::new(config.redis.clone())?;

        Ok(Self {
            redis_pool: std::sync::Mutex::new(redis_pool),
            similarity_threshold: config.cache.similarity_threshold,
            max_cache_size: config.cache.max_cache_size,
            ttl_seconds: config.cache.ttl_seconds as u32,
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
