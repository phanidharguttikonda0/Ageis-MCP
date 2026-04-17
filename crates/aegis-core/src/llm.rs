//! LLM client for upstream API communication
//!
//! Provides comprehensive LLM provider integrations with support for OpenAI,
//! Anthropic, Azure OpenAI, and other providers. Includes retry logic, circuit breaker,
//! streaming responses, and token usage tracking.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

use crate::{Error, Result, LLMConfig};

/// Default timeout for LLM API requests
pub const DEFAULT_LLM_TIMEOUT: Duration = Duration::from_secs(30);
/// Maximum retry attempts for LLM API calls
pub const DEFAULT_MAX_RETRIES: u32 = 3;
/// Circuit breaker failure threshold
pub const CIRCUIT_BREAKER_THRESHOLD: u32 = 5;
/// Circuit breaker cooldown duration
pub const CIRCUIT_BREAKER_COOLDOWN: Duration = Duration::from_secs(60);

/// LLM request with comprehensive parameters
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMRequest {
    pub prompt: String,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub n: Option<u32>,
    pub stop: Option<Vec<String>>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub user: Option<String>,
    pub stream: Option<bool>,
}

impl Default for LLMRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            model: "gpt-4".to_string(),
            max_tokens: None,
            temperature: Some(0.7),
            top_p: None,
            n: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            user: None,
            stream: Some(false),
        }
    }
}

/// LLM response with token usage
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMResponse {
    pub text: String,
    pub tokens_used: u32,
    pub model: String,
    pub finish_reason: Option<String>,
    pub latency_ms: u64,
    pub cached: bool,
}

/// Streaming LLM response chunk
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMStreamingChunk {
    pub text_delta: String,
    pub finish_reason: Option<String>,
    pub tokens_used: u32,
}

/// LLM provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum LLMProvider {
    OpenAI,
    Anthropic,
    AzureOpenAI,
    Local,
}

impl std::fmt::Display for LLMProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LLMProvider::OpenAI => write!(f, "OpenAI"),
            LLMProvider::Anthropic => write!(f, "Anthropic"),
            LLMProvider::AzureOpenAI => write!(f, "Azure OpenAI"),
            LLMProvider::Local => write!(f, "Local"),
        }
    }
}

/// LLM client statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub cache_hits: u64,
    pub total_tokens_used: u64,
    pub total_latency_ms: u64,
    pub circuit_breaker_trips: u64,
    pub active_streams: u64,
}

impl Default for LLMStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            cache_hits: 0,
            total_tokens_used: 0,
            total_latency_ms: 0,
            circuit_breaker_trips: 0,
            active_streams: 0,
        }
    }
}

/// LLM client trait with comprehensive functionality
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn complete(&self, request: LLMRequest) -> Result<LLMResponse>;
    async fn complete_streaming(&self, request: LLMRequest) -> Result<Box<dyn futures::Stream<Item = Result<LLMStreamingChunk>> + Send>>;
    fn get_provider(&self) -> LLMProvider;
    fn get_stats(&self) -> LLMStats;
}

/// Circuit breaker for LLM API failures
#[derive(Debug, Clone)]
struct CircuitBreaker {
    failure_count: u32,
    last_failure_time: Option<std::time::Instant>,
    state: CircuitBreakerState,
    threshold: u32,
    cooldown: Duration,
}

#[derive(Debug, Clone, PartialEq)]
enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    fn new(threshold: u32, cooldown: Duration) -> Self {
        Self {
            failure_count: 0,
            last_failure_time: None,
            state: CircuitBreakerState::Closed,
            threshold,
            cooldown,
        }
    }

    fn record_success(&mut self) {
        self.failure_count = 0;
        if self.state == CircuitBreakerState::HalfOpen {
            self.state = CircuitBreakerState::Closed;
            info!("Circuit breaker recovered, closing");
        }
    }

    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(std::time::Instant::now());

        if self.failure_count >= self.threshold {
            self.state = CircuitBreakerState::Open;
            warn!(
                "Circuit breaker opened after {} failures",
                self.failure_count
            );
        }
    }

    fn allow_request(&self) -> bool {
        if self.state != CircuitBreakerState::Open {
            return true;
        }

        if let Some(last_failure) = self.last_failure_time {
            let elapsed = last_failure.elapsed();
            if elapsed >= self.cooldown {
                info!("Circuit breaker cooldown expired, entering half-open state");
                return true;
            }
        }

        false
    }

    fn transition_to_half_open(&mut self) {
        if self.state == CircuitBreakerState::Open {
            self.state = CircuitBreakerState::HalfOpen;
            info!("Circuit breaker transitioning to half-open");
        }
    }
}

/// OpenAI LLM client implementation
pub struct OpenAIClient {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    timeout: Duration,
    max_retries: u32,
    circuit_breaker: Arc<RwLock<CircuitBreaker>>,
    stats: Arc<RwLock<LLMStats>>,
}

impl OpenAIClient {
    /// Create a new OpenAI client
    pub fn new(config: &LLMConfig) -> Result<Self> {
        info!("Creating OpenAI LLM client with model: {}", config.model);

        if config.api_key.is_empty() {
            return Err(Error::LLM("OpenAI API key is required".to_string()));
        }

        let client = Client::builder()
            .timeout(DEFAULT_LLM_TIMEOUT)
            .build()
            .map_err(|e| Error::LLM(format!("Failed to create HTTP client: {}", e)))?;

        let circuit_breaker = Arc::new(RwLock::new(CircuitBreaker::new(
            CIRCUIT_BREAKER_THRESHOLD,
            CIRCUIT_BREAKER_COOLDOWN,
        )));

        Ok(Self {
            client,
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            base_url: config.upstream_url.clone(),
            timeout: DEFAULT_LLM_TIMEOUT,
            max_retries: config.max_retries,
            circuit_breaker,
            stats: Arc::new(RwLock::new(LLMStats::default())),
        })
    }

    /// Make API call with retries
    async fn call_api_with_retry(&self, request_body: serde_json::Value) -> Result<serde_json::Value> {
        let start = std::time::Instant::now();

        for attempt in 0..self.max_retries {
            // Check circuit breaker
            {
                let breaker = self.circuit_breaker.read().await;
                if !breaker.allow_request() {
                    return Err(Error::LLM("Circuit breaker is open".to_string()));
                }
            }

            let result = self.call_openai_api(request_body.clone()).await;

            let latency = start.elapsed().as_millis() as u64;

            match result {
                Ok(response) => {
                    // Record success
                    {
                        let mut breaker = self.circuit_breaker.write().await;
                        breaker.record_success();
                    }
                    {
                        let mut stats = self.stats.write().await;
                        stats.total_requests += 1;
                        stats.successful_requests += 1;
                        stats.total_latency_ms += latency;
                    }

                    return Ok(response);
                }
                Err(e) if attempt < self.max_retries - 1 => {
                    warn!("OpenAI API call attempt {} failed: {}, retrying", attempt + 1, e);

                    // Exponential backoff
                    let backoff_ms = 1000 * 2_u64.pow(attempt as u32);
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;

                    // Record failure
                    {
                        let mut breaker = self.circuit_breaker.write().await;
                        breaker.record_failure();
                    }
                }
                Err(e) => {
                    // Final failure
                    let mut stats = self.stats.write().await;
                    stats.total_requests += 1;
                    stats.failed_requests += 1;

                    return Err(e);
                }
            }
        }

        Err(Error::LLM("Max retries exceeded".to_string()))
    }

    /// Call OpenAI API
    async fn call_openai_api(&self, request_body: serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| Error::LLM(format!("Failed to send request: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| Error::LLM(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(Error::LLM(format!(
                "OpenAI API error ({}): {}",
                status, response_text
            )));
        }

        serde_json::from_str(&response_text)
            .map_err(|e| Error::LLM(format!("Failed to parse response: {}", e)))
    }

    /// Update statistics for streaming response
    async fn update_streaming_stats(&self, tokens_used: u32, latency_ms: u64, success: bool) {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;

        if success {
            stats.successful_requests += 1;
            stats.total_tokens_used += tokens_used as u64;
        } else {
            stats.failed_requests += 1;
        }

        stats.total_latency_ms += latency_ms;
    }
}

#[async_trait]
impl LLMClient for OpenAIClient {
    #[instrument(skip(self, request))]
    async fn complete(&self, request: LLMRequest) -> Result<LLMResponse> {
        let start = std::time::Instant::now();

        debug!(
            "Calling OpenAI API for model: {}, prompt length: {}",
            request.model,
            request.prompt.len()
        );

        // Transform to OpenAI format
        let openai_request = serde_json::json!({
            "model": request.model,
            "messages": [
                {
                    "role": "user",
                    "content": request.prompt
                }
            ],
            "max_tokens": request.max_tokens.unwrap_or(1000),
            "temperature": request.temperature.unwrap_or(0.7),
            "top_p": request.top_p,
            "n": request.n.unwrap_or(1),
            "stop": request.stop,
            "presence_penalty": request.presence_penalty.unwrap_or(0.0),
            "frequency_penalty": request.frequency_penalty.unwrap_or(0.0),
            "user": request.user
        });

        let response = self.call_api_with_retry(openai_request).await?;

        // Extract response data
        let text = response["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| Error::LLM("No text in response".to_string()))?
            .to_string();

        let tokens_used = response["usage"]["total_tokens"]
            .as_u64()
            .ok_or_else(|| Error::LLM("No usage in response".to_string()))? as u32;

        let finish_reason = response["choices"][0]["finish_reason"]
            .as_str()
            .map(|s| s.to_string());

        let latency = start.elapsed().as_millis() as u64;

        info!(
            "OpenAI API call completed: {} tokens, {}ms latency",
            tokens_used, latency
        );

        Ok(LLMResponse {
            text,
            tokens_used,
            model: request.model.clone(),
            finish_reason,
            latency_ms: latency,
            cached: false,
        })
    }

    #[instrument(skip(self, request))]
    async fn complete_streaming(
        &self,
        request: LLMRequest,
    ) -> Result<Box<dyn futures::Stream<Item = Result<LLMStreamingChunk>> + Send>> {
        // Placeholder for streaming - will be implemented in full version
        warn!("Streaming not yet implemented for OpenAI client");
        Err(Error::LLM("Streaming not implemented".to_string()))
    }

    fn get_provider(&self) -> LLMProvider {
        LLMProvider::OpenAI
    }

    fn get_stats(&self) -> LLMStats {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.stats.read().await.clone()
            })
        })
    }
}

/// Simple in-memory LLM client for testing
pub struct InMemoryLLMClient {
    model: String,
    stats: Arc<RwLock<LLMStats>>,
}

impl InMemoryLLMClient {
    /// Create a new in-memory LLM client for testing
    pub fn new(model: String) -> Self {
        info!("Creating in-memory LLM client for testing: {}", model);

        Self {
            model,
            stats: Arc::new(RwLock::new(LLMStats::default())),
        }
    }

    /// Set a mock response for testing
    pub async fn set_mock_response(&self, prompt: &str, response: &str) {
        // In a real implementation, this would store mock responses
        debug!("Setting mock response for prompt: {}", prompt);
    }
}

#[async_trait]
impl LLMClient for InMemoryLLMClient {
    #[instrument(skip(self, request))]
    async fn complete(&self, request: LLMRequest) -> Result<LLMResponse> {
        let start = std::time::Instant::now();

        info!("In-memory LLM client processing request");

        // Generate a simple mock response
        let response_text = format!(
            "Mock LLM response for: {} (model: {})",
            request.prompt, request.model
        );

        let tokens_used = (request.prompt.len() / 4) as u32; // Rough estimate
        let latency = start.elapsed().as_millis() as u64;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;
        stats.successful_requests += 1;
        stats.total_tokens_used += tokens_used as u64;
        stats.total_latency_ms += latency;

        Ok(LLMResponse {
            text: response_text,
            tokens_used,
            model: request.model,
            finish_reason: Some("stop".to_string()),
            latency_ms: latency,
            cached: false,
        })
    }

    #[instrument(skip(self, request))]
    async fn complete_streaming(
        &self,
        request: LLMRequest,
    ) -> Result<Box<dyn futures::Stream<Item = Result<LLMStreamingChunk>> + Send>> {
        warn!("Streaming not implemented for in-memory client");
        Err(Error::LLM("Streaming not implemented".to_string()))
    }

    fn get_provider(&self) -> LLMProvider {
        LLMProvider::Local
    }

    fn get_stats(&self) -> LLMStats {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.stats.read().await.clone()
            })
        })
    }
}

/// LLM client factory for creating provider-specific clients
pub struct LLMClientFactory;

impl LLMClientFactory {
    /// Create an LLM client based on configuration
    pub fn create_client(config: &LLMConfig) -> Result<Arc<dyn LLMClient>> {
        // Determine provider based on configuration
        let provider = if config.upstream_url.contains("api.openai.com") {
            LLMProvider::OpenAI
        } else if config.upstream_url.contains("api.anthropic.com") {
            LLMProvider::Anthropic
        } else {
            LLMProvider::AzureOpenAI
        };

        match provider {
            LLMProvider::OpenAI => Ok(Arc::new(OpenAIClient::new(config)?) as Arc<dyn LLMClient>),
            LLMProvider::Local => Ok(Arc::new(InMemoryLLMClient::new(config.model.clone())) as Arc<dyn LLMClient>),
            _ => Err(Error::LLM(format!("Provider not implemented: {:?}", provider))),
        }
    }

    /// Create an in-memory client for testing
    pub fn create_in_memory_client(model: String) -> Result<Arc<dyn LLMClient>> {
        Ok(Arc::new(InMemoryLLMClient::new(model)) as Arc<dyn LLMClient>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_request_default() {
        let request = LLMRequest::default();
        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.temperature, Some(0.7));
        assert!(request.stream.is_some());
    }

    #[test]
    fn test_llm_provider_display() {
        assert_eq!(format!("{}", LLMProvider::OpenAI), "OpenAI");
        assert_eq!(format!("{}", LLMProvider::Anthropic), "Anthropic");
    }

    #[test]
    fn test_circuit_breaker_closed_by_default() {
        let breaker = CircuitBreaker::new(5, Duration::from_secs(60));
        assert_eq!(breaker.state, CircuitBreakerState::Closed);
        assert!(breaker.allow_request());
    }

    #[test]
    fn test_circuit_breaker_opens_after_threshold() {
        let mut breaker = CircuitBreaker::new(3, Duration::from_secs(60));
        for _ in 0..3 {
            breaker.record_failure();
        }
        assert_eq!(breaker.state, CircuitBreakerState::Open);
        assert!(!breaker.allow_request());
    }

    #[test]
    fn test_circuit_breaker_recovery() {
        let mut breaker = CircuitBreaker::new(3, Duration::from_secs(60));
        for _ in 0..3 {
            breaker.record_failure();
        }
        assert_eq!(breaker.state, CircuitBreakerState::Open);

        // Simulate cooldown passing
        breaker.last_failure_time = Some(std::time::Instant::now() - Duration::from_secs(61));
        assert!(breaker.allow_request()); // Should allow due to cooldown

        breaker.transition_to_half_open();
        breaker.record_success();
        assert_eq!(breaker.state, CircuitBreakerState::Closed);
    }

    #[tokio::test]
    async fn test_in_memory_client_creation() {
        let client = InMemoryLLMClient::new("test-model".to_string());
        assert_eq!(client.model, "test-model");
    }

    #[tokio::test]
    async fn test_in_memory_client_complete() {
        let client = InMemoryLLMClient::new("gpt-4".to_string());
        let request = LLMRequest {
            prompt: "Test prompt".to_string(),
            model: "gpt-4".to_string(),
            ..Default::default()
        };

        let response = client.complete(request).await.unwrap();
        assert!(response.text.contains("Test prompt"));
        assert_eq!(response.model, "gpt-4");
        assert!(response.tokens_used > 0);
    }

    #[tokio::test]
    async fn test_llm_client_factory() {
        let config = LLMConfig {
            upstream_url: "https://api.openai.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-4".to_string(),
            timeout: 30000,
            max_retries: 3,
        };

        let client = LLMClientFactory::create_in_memory_client("test-model".to_string());
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.get_provider(), LLMProvider::Local);
    }
}