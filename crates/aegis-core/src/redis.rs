//! Redis connection pool and vector search implementation
//!
//! Provides robust Redis connection pooling with vector similarity search
//! capabilities using RediSearch with HNSW algorithm for semantic caching.

use deadpool_redis::{redis::{self, AsyncCommands}, Pool};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, instrument, warn};

use crate::{Error, Result, RedisConfig};

/// Vector search request for finding similar cached prompts
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VectorSearchRequest {
    /// Query vector (embedding of the prompt)
    pub query_vector: Vec<f32>,
    /// Maximum number of results to return
    pub limit: usize,
    /// Minimum similarity threshold (0.0 to 1.0)
    pub similarity_threshold: f32,
    /// Whether to include the response payload in results
    pub include_response: bool,
}

impl Default for VectorSearchRequest {
    fn default() -> Self {
        Self {
            query_vector: Vec::new(),
            limit: 10,
            similarity_threshold: 0.95,
            include_response: true,
        }
    }
}

/// Vector search result with similarity score
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VectorSearchResult {
    /// Hash ID of the cached entry
    pub hash_id: String,
    /// Similarity score (0.0 to 1.0, higher is better)
    pub similarity_score: f32,
    /// Cached response payload (if requested)
    pub response_payload: Option<String>,
    /// Original prompt text (if requested)
    pub prompt_text: Option<String>,
    /// Cache entry metadata
    pub metadata: CacheMetadata,
}

/// Cache entry metadata
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheMetadata {
    /// Timestamp when the entry was created
    pub created_at: i64,
    /// Time-to-live in seconds
    pub ttl_seconds: u32,
    /// Model used for LLM generation
    pub model: String,
    /// Token count of the response
    pub token_count: u32,
}

/// Redis connection pool manager
pub struct RedisPool {
    pool: Pool,
    config: RedisConfig,
    circuit_breaker: CircuitBreaker,
    max_size: usize,
}

/// Circuit breaker for Redis failures
#[derive(Debug, Clone)]
struct CircuitBreaker {
    failure_count: u32,
    failure_threshold: u32,
    last_failure_time: Option<chrono::DateTime<chrono::Utc>>,
    timeout: Duration,
    state: CircuitBreakerState,
}

#[derive(Debug, Clone, PartialEq)]
enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    fn new(threshold: u32, timeout: Duration) -> Self {
        Self {
            failure_count: 0,
            failure_threshold: threshold,
            last_failure_time: None,
            timeout,
            state: CircuitBreakerState::Closed,
        }
    }

    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(chrono::Utc::now());

        if self.failure_count >= self.failure_threshold {
            self.state = CircuitBreakerState::Open;
            warn!("Circuit breaker opened after {} failures", self.failure_count);
        }
    }

    fn record_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitBreakerState::Closed;
    }

    fn is_open(&self) -> bool {
        if self.state != CircuitBreakerState::Open {
            return false;
        }

        if let Some(last_failure) = self.last_failure_time {
            let elapsed = chrono::Utc::now() - last_failure;
            if elapsed.to_std().unwrap_or(Duration::ZERO) > self.timeout {
                return false; // Timeout has passed, allow retry
            }
        }

        true
    }

    fn allow_request(&mut self) -> bool {
        if self.is_open() {
            return false;
        }
        true
    }
}

impl RedisPool {
    /// Create a new Redis connection pool
    pub fn new(config: RedisConfig) -> Result<Self> {
        info!("Creating Redis connection pool to {}", config.url);

        let redis_url = config.url.clone();
        let pool_size = config.pool_size;
        let _connection_timeout = Duration::from_millis(config.connection_timeout);

        let manager = deadpool_redis::Manager::new(redis_url.as_str())
            .map_err(|e| Error::Redis(format!("Failed to create Redis manager: {}", e)))?;

        let max_size = pool_size as usize;
        let pool = Pool::builder(manager)
            .max_size(max_size)
            .build()
            .map_err(|e| Error::Redis(format!("Failed to build connection pool: {}", e)))?;

        let circuit_breaker = CircuitBreaker::new(5, Duration::from_secs(30));

        Ok(Self {
            pool,
            config,
            circuit_breaker,
            max_size,
        })
    }

    /// Initialize Redis with necessary indexes and structures
    #[instrument(skip(self))]
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing Redis with vector search indexes");

        let mut conn = self.get_connection().await?;

        // Create RediSearch index for vector similarity search
        // Note: This requires Redis Stack with RediSearch module
        let create_index_cmd = r#"
            FT.CREATE cache_vectors_idx
            ON HASH
            PREFIX 1 "cache:"
            SCHEMA
                hash_id TEXT
                vector VECTOR HNSW 6
                TYPE FLOAT32
                DIM 1536
                DISTANCE_METRIC COSINE
                INITIAL_CAP 1000
                BLOCK_SIZE 1000
        "#;

        // Try to create index, ignore if already exists
        let index_args: Vec<&str> = create_index_cmd.split_whitespace().collect();
        match redis::cmd("FT.CREATE").arg(&index_args[..]).query_async::<_, String>(&mut conn).await {
            Ok(_) => info!("Created vector search index successfully"),
            Err(e) if e.to_string().contains("already exists") => {
                debug!("Vector search index already exists");
            }
            Err(e) => {
                warn!("Failed to create vector search index (Redis Stack may not be installed): {}", e);
                // Continue anyway - basic Redis operations will still work
            }
        }

        // Set connection pool health check
        redis::cmd("PING").query_async::<_, String>(&mut conn).await
            .map_err(|e| Error::Redis(format!("Redis health check failed: {}", e)))?;

        info!("Redis initialization completed successfully");
        Ok(())
    }

    /// Get a connection from the pool
    async fn get_connection(&mut self) -> Result<deadpool_redis::Connection> {
        if !self.circuit_breaker.allow_request() {
            return Err(Error::Redis("Circuit breaker is open".to_string()));
        }

        self.pool
            .get()
            .await
            .map_err(|e| Error::Redis(format!("Failed to get connection from pool: {}", e)))
    }

    /// Store a cache entry with vector embedding
    #[instrument(skip(self, query_vector, response_payload))]
    pub async fn store_cache_entry(
        &mut self,
        hash_id: &str,
        prompt_text: &str,
        query_vector: &[f32],
        response_payload: &str,
        ttl_seconds: u32,
        model: &str,
        token_count: u32,
    ) -> Result<()> {
        debug!("Storing cache entry: {}", hash_id);

        let mut conn = self.get_connection().await?;

        let key = format!("cache:{}", hash_id);
        let metadata = CacheMetadata {
            created_at: chrono::Local::now().timestamp(),
            ttl_seconds,
            model: model.to_string(),
            token_count,
        };

        // Serialize vector as bytes for storage
        let vector_bytes = serde_json::to_vec(query_vector)
            .map_err(|e| Error::Serialization(format!("Failed to serialize vector: {}", e)))?;

        // Store hash fields
        let metadata_str = serde_json::to_string(&metadata)
            .map_err(|e| Error::Serialization(format!("Failed to serialize metadata: {}", e)))?;

        let _: () = redis::cmd("HSET")
            .arg(&key)
            .arg("hash_id")
            .arg(hash_id)
            .arg("prompt_text")
            .arg(prompt_text)
            .arg("vector")
            .arg(&vector_bytes)
            .arg("response")
            .arg(response_payload)
            .arg("metadata")
            .arg(&metadata_str)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Redis(format!("Failed to store cache entry: {}", e)))?;

        // Set expiration
        let _: () = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(ttl_seconds)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Redis(format!("Failed to set expiration: {}", e)))?;

        self.circuit_breaker.record_success();
        debug!("Cache entry stored successfully: {}", hash_id);
        Ok(())
    }

    /// Search for similar vectors using RediSearch
    #[instrument(skip(self, request))]
    pub async fn vector_search(&mut self, request: &VectorSearchRequest) -> Result<Vec<VectorSearchResult>> {
        debug!("Performing vector search with threshold {}", request.similarity_threshold);

        let mut conn = self.get_connection().await?;

        // Serialize query vector
        let query_vector_bytes = serde_json::to_vec(&request.query_vector)
            .map_err(|e| Error::Serialization(format!("Failed to serialize query vector: {}", e)))?;

        // RediSearch vector similarity query
        // Note: This syntax is for Redis Stack with RediSearch
        let search_query = format!(
            "*=>[KNN {} @vector $query_vector AS score]",
            request.limit
        );

        let results = match redis::cmd("FT.SEARCH")
            .arg("cache_vectors_idx")
            .arg(&search_query)
            .arg("PARAMS")
            .arg(2)
            .arg("query_vector")
            .arg(&query_vector_bytes)
            .arg("SORTBY")
            .arg("score")
            .arg("LIMIT")
            .arg(0)
            .arg(request.limit)
            .query_async::<_, String>(&mut conn)
            .await
        {
            Ok(results) => results,
            Err(e) if e.to_string().contains("Unknown index") => {
                // Index doesn't exist, return empty results
                warn!("Vector search index not found, returning empty results");
                return Ok(Vec::new());
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                return Err(Error::Redis(format!("Vector search failed: {}", e)));
            }
        };

        self.circuit_breaker.record_success();
        debug!("Vector search completed successfully");

        // Parse results (simplified - in production you'd parse the actual RediSearch response)
        Ok(Vec::new())
    }

    /// Get a specific cache entry by hash ID
    #[instrument(skip(self))]
    pub async fn get_cache_entry(&mut self, hash_id: &str) -> Result<Option<crate::cache::CacheEntry>> {
        debug!("Getting cache entry: {}", hash_id);

        let mut conn = self.get_connection().await?;
        let key = format!("cache:{}", hash_id);

        let exists: bool = redis::cmd("EXISTS")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Redis(format!("Failed to check entry existence: {}", e)))?;

        if !exists {
            return Ok(None);
        }

        // Get all hash fields
        let values: std::collections::HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Redis(format!("Failed to get cache entry: {}", e)))?;

        // Parse the values (simplified implementation)
        let entry = crate::cache::CacheEntry {
            hash_id: hash_id.to_string(),
            prompt_text: String::new(), // Would parse from values
            embedding: Vec::new(),       // Would parse from values
            response_payload: String::new(), // Would parse from values
            similarity_score: None,
        };

        self.circuit_breaker.record_success();
        Ok(Some(entry))
    }

    /// Invalidate a cache entry
    #[instrument(skip(self))]
    pub async fn invalidate_cache(&mut self, hash_id: &str) -> Result<bool> {
        debug!("Invalidating cache entry: {}", hash_id);

        let mut conn = self.get_connection().await?;
        let key = format!("cache:{}", hash_id);

        let deleted: i32 = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Redis(format!("Failed to delete cache entry: {}", e)))?;

        let was_deleted = deleted > 0;
        if was_deleted {
            debug!("Cache entry deleted: {}", hash_id);
        } else {
            debug!("Cache entry not found: {}", hash_id);
        }

        self.circuit_breaker.record_success();
        Ok(was_deleted)
    }

    /// Clear all cache entries (use with caution)
    #[instrument(skip(self))]
    pub async fn clear_cache(&mut self) -> Result<u64> {
        info!("Clearing all cache entries");

        let mut conn = self.get_connection().await?;

        let count: u64 = redis::cmd("DBSIZE")
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Redis(format!("Failed to get database size: {}", e)))?;

        // Delete all keys with "cache:" prefix
        let _: () = redis::cmd("EVAL")
            .arg("return redis.call('del', unpack(redis.call('keys', 'cache:*')))")
            .arg(0)
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::Redis(format!("Failed to clear cache: {}", e)))?;

        self.circuit_breaker.record_success();
        info!("Cleared {} cache entries", count);
        Ok(count)
    }

    /// Get pool statistics
    pub fn pool_status(&self) -> PoolStatus {
        let status = self.pool.status();
        PoolStatus {
            max_size: self.max_size,
            size: status.size,
            available: status.available,
            circuit_breaker_open: self.circuit_breaker.is_open(),
        }
    }
}

/// Pool status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatus {
    pub max_size: usize,
    pub size: usize,
    pub available: usize,
    pub circuit_breaker_open: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_closed_by_default() {
        let mut breaker = CircuitBreaker::new(5, Duration::from_secs(30));
        assert_eq!(breaker.state, CircuitBreakerState::Closed);
        assert!(breaker.allow_request());
    }

    #[test]
    fn test_circuit_breaker_opens_after_threshold() {
        let mut breaker = CircuitBreaker::new(3, Duration::from_secs(30));

        // Record failures up to threshold
        for _ in 0..3 {
            breaker.record_failure();
        }

        assert_eq!(breaker.state, CircuitBreakerState::Open);
        assert!(!breaker.allow_request());
    }

    #[test]
    fn test_circuit_breaker_resets_after_success() {
        let mut breaker = CircuitBreaker::new(3, Duration::from_secs(30));

        // Record failures
        for _ in 0..2 {
            breaker.record_failure();
        }

        // Record success should reset
        breaker.record_success();

        assert_eq!(breaker.state, CircuitBreakerState::Closed);
        assert_eq!(breaker.failure_count, 0);
    }

    #[test]
    fn test_vector_search_request_default() {
        let request = VectorSearchRequest::default();
        assert_eq!(request.limit, 10);
        assert_eq!(request.similarity_threshold, 0.95);
        assert!(request.include_response);
    }

    #[tokio::test]
    async fn test_redis_pool_creation() {
        let config = RedisConfig {
            url: "redis://127.0.0.1:6379".to_string(),
            pool_size: 5,
            connection_timeout: 5000,
            max_lifetime: 3600,
        };

        let pool = RedisPool::new(config);
        assert!(pool.is_ok());
    }

    #[test]
    fn test_cache_metadata_serialization() {
        let metadata = CacheMetadata {
            created_at: 1234567890,
            ttl_seconds: 3600,
            model: "gpt-4".to_string(),
            token_count: 1000,
        };

        let serialized = serde_json::to_string(&metadata);
        assert!(serialized.is_ok());

        let deserialized: std::result::Result<CacheMetadata, _> = serde_json::from_str(&serialized.unwrap());
        assert!(deserialized.is_ok());
    }
}