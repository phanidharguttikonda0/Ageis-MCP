//! Aegis-Core: Core business logic and shared types for Aegis-MCP
//!
//! This library provides the core functionality for the Aegis-MCP semantic caching server,
//! including configuration management, Redis operations, embedding generation, and
//! MCP protocol implementation.

pub mod config;
pub mod error;
pub mod cache;
pub mod embedding;
pub mod llm;
pub mod mcp;
pub mod tracing;
pub mod metrics;

// Re-exports for convenience
pub use config::{AppConfig, ServerConfig, RedisConfig, CacheConfig, LLMConfig, ObservabilityConfig};
pub use error::{Error, Result};
pub use cache::{CacheEngine, CacheEntry};
pub use embedding::{EmbeddingService, EmbeddingRequest};
pub use llm::{LLMClient, LLMRequest, LLMResponse};
pub use mcp::{MCPServer, MCPRequest, MCPResponse};
pub use tracing::{
    RequestContext, init_tracing, init_dev_tracing,
    create_request_span, create_cache_span, create_llm_span, create_redis_span, create_embedding_span,
};
pub use metrics::{Metrics};

/// Semantic caching server and governance layer for LLM requests
///
/// Aegis-MCP acts as an intelligent proxy that intercepts LLM prompts via the Model Context Protocol (MCP),
/// generates vector embeddings, queries Redis for similar cached prompts, and either returns cached responses
/// or forwards to upstream LLM providers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = "Aegis-MCP";

/// Default cache similarity threshold (cosine distance)
pub const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.95;

/// Default embedding dimension (OpenAI text-embedding-ada-002)
pub const DEFAULT_EMBEDDING_DIMENSION: usize = 1536;
