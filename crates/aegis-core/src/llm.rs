//! LLM client for upstream API communication
//!
//! Placeholder module for LLM client - will be implemented in Issue #8.

use serde::{Deserialize, Serialize};

/// LLM request
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMRequest {
    pub prompt: String,
    pub model: String,
}

/// LLM response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMResponse {
    pub text: String,
    pub tokens_used: u32,
}

/// LLM client trait - will be fully implemented in Issue #8
#[allow(async_fn_in_trait)]
pub trait LLMClient: Send + Sync {
    async fn complete(&self, request: LLMRequest) -> Result<LLMResponse, crate::Error>;
}
