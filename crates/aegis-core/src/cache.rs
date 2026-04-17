//! Cache engine and related types
//!
//! Placeholder module for cache functionality - will be implemented in subsequent issues.

use serde::{Deserialize, Serialize};

/// Cache entry representing a stored prompt-response pair
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheEntry {
    pub hash_id: String,
    pub prompt_text: String,
    pub embedding: Vec<f32>,
    pub response_payload: String,
    pub similarity_score: Option<f32>,
}

/// Main cache engine - will be fully implemented in Issue #7
pub struct CacheEngine;

impl Default for CacheEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheEngine {
    pub fn new() -> Self {
        Self
    }
}
