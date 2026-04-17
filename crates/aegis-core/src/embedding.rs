//! Embedding generation service
//!
//! Placeholder module for embedding generation - will be implemented in Issue #6.

use serde::{Deserialize, Serialize};

/// Request for embedding generation
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingRequest {
    pub text: String,
    pub model: String,
}

/// Embedding generation service - will be fully implemented in Issue #6
#[allow(async_fn_in_trait)]
pub trait EmbeddingService: Send + Sync {
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, crate::Error>;
}
