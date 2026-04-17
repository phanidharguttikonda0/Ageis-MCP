# Architecture Documentation for Aegis-MCP

## System Overview

Aegis-MCP is a semantic caching server and governance layer designed to intercept traffic from agentic IDEs (like Cursor) to LLM providers. It uses vector embeddings to intelligently cache and semantically match similar prompts, dramatically reducing redundant LLM API costs while maintaining response quality.

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Agentic IDEs                             │
│                 (Cursor, JetBrains, etc.)                    │
└───────────────────────┬─────────────────────────────────────┘
                        │ MCP Protocol (JSON-RPC 2.0)
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                    Aegis-MCP Server                         │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              HTTP Server (Axum)                        │  │
│  │  • Health checks  • Metrics  • Cache management      │  │
│  └─────────────────┬─────────────────────────────────────┘  │
│                    │                                         │
│  ┌─────────────────▼─────────────────────────────────────┐  │
│  │            MCP Protocol Handler                        │  │
│  │  • JSON-RPC 2.0  • Tool invocation  • Error handling   │  │
│  └─────────────────┬─────────────────────────────────────┘  │
│                    │                                         │
│  ┌─────────────────▼─────────────────────────────────────┐  │
│  │          Cache Orchestration Engine                    │  │
│  │  • Request deduplication  • Similarity matching       │  │
│  │  • Embedding generation  • TTL management             │  │
│  └─────────────┬───────────────────┬──────────────────────┘  │
│                │                   │                          │
│  ┌─────────────▼─────────┐   ┌───▼──────────────────────┐  │
│  │  Embedding Service     │   │   Vector Cache           │  │
│  │  • OpenAI ada-002      │   │  • Redis Stack          │  │
│  │  • In-memory cache     │   │  • HNSW vector search    │  │
│  └─────────────┬───────────┘   └───┬──────────────────────┘  │
│                │                   │                          │
│  ┌─────────────▼───────────────────▼──────────────────────┐  │
│  │               LLM Client                               │  │
│  │  • OpenAI API  • Retry logic  • Circuit breaker       │  │
│  └────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
                  ┌─────────────────┐
                  │  OpenAI API     │
                  │  (or compatible)│
                  └─────────────────┘
```

## Core Components

### 1. HTTP Server (Axum)

**Purpose**: High-performance HTTP server for serving MCP protocol

**Key Features**:
- Async request handling with Tokio
- Comprehensive routing (health, metrics, cache endpoints)
- Middleware stack (tracing, compression, CORS)
- Graceful shutdown handling
- Circuit breaker pattern for resilience

**Endpoints**:
- `GET /health` - Health check and service status
- `GET /metrics` - Prometheus metrics
- `GET /stats/cache` - Cache performance statistics
- `POST /api/v1/cache/invalidate` - Cache invalidation
- `POST /` - MCP JSON-RPC 2.0 endpoint

### 2. MCP Protocol Handler

**Purpose**: Model Context Protocol implementation for IDE integration

**Key Features**:
- JSON-RPC 2.0 compliance
- Tool invocation framework
- Resource and prompt discovery
- Capability negotiation
- Error handling with proper error codes

**Supported Methods**:
- `initialize` - Handshake and capability negotiation
- `list_tools` - Discover available tools
- `call_tool` - Invoke tools (cache_lookup, cache_stats, etc.)
- `list_resources` - Discover available resources
- `list_prompts` - Discover available prompts
- `complete` - Autocomplete functionality

### 3. Cache Orchestration Engine

**Purpose**: Core caching logic with semantic similarity matching

**Key Features**:
- Request deduplication (prevent concurrent identical requests)
- Vector similarity search with configurable thresholds
- TTL-based cache expiration
- Async write-back for performance
- Per-stage timeout handling

**Flow**:
```
Request → Generate Embedding → Search Cache → Match? → Return Cache
                                                    ↓
                                                Call LLM → Cache Response
```

### 4. Embedding Service

**Purpose**: Generate vector embeddings for semantic similarity

**Key Features**:
- OpenAI text-embedding-ada-002 integration
- In-memory caching for identical prompts
- Retry logic with exponential backoff
- Token usage tracking

**Embedding Specs**:
- Model: `text-embedding-ada-002`
- Dimensions: 1536
- Max text length: 8191 characters

### 5. Redis Vector Store

**Purpose**: High-performance vector similarity search

**Key Features**:
- Redis Stack with RediSearch
- HNSW (Hierarchical Navigable Small World) algorithm
- Cosine similarity search
- Circuit breaker for API resilience
- Connection pooling with deadpool-redis

**Data Structures**:
```
Hash ID → {
  "prompt_text": "...",
  "embedding": [0.1, 0.2, ...],
  "response": "...",
  "metadata": {...}
}
```

### 6. LLM Client

**Purpose**: Upstream LLM provider integration

**Key Features**:
- OpenAI API client with full Chat Completions support
- Circuit breaker pattern (threshold: 5 failures, cooldown: 60s)
- Retry logic with exponential backoff (max 3 retries)
- Provider abstraction (OpenAI, Anthropic, Azure, Local)
- Token usage tracking and statistics

**Supported Providers**:
- OpenAI (GPT-4, GPT-3.5-turbo)
- Anthropic (Claude) - coming soon
- Azure OpenAI - coming soon
- Local models - for testing

## Data Flow

### Request Lifecycle

```
1. IDE sends MCP request (JSON-RPC 2.0)
2. HTTP Server receives and validates
3. MCP Handler parses request
4. Cache Engine checks for deduplication
5. Generate embedding for prompt
6. Search Redis for similar prompts
7. If similarity > threshold: Return cached response
8. Else: Call LLM API, cache response, return result
9. Update metrics and logging
10. Return MCP response to IDE
```

### Caching Decision Flow

```
┌─────────────────────────────────────────┐
│  New Request                           │
└───────────┬─────────────────────────────┘
            │
            ▼
    ┌───────────────┐
    │ Deduplication │ ← Check for concurrent identical requests
    │   Check       │
    └───────┬───────┘
            │
            ▼
    ┌───────────────┐
    │ Generate      │
    │ Embedding     │ ← OpenAI ada-002 (cached locally)
    └───────┬───────┘
            │
            ▼
    ┌───────────────┐
    │ Vector Search │ ← Redis HNSW search
    │   (Redis)     │
    └───────┬───────┘
            │
            ▼
     ┌────────────┐
     │ Similarity │ ← threshold: 0.95
     │  Check     │
     └────┬───┬───┘
          │   │
    Yes   │   │  No
          │   │
          ▼   ▼
    ┌─────────┐ ┌─────────┐
    │ Return  │ │  Call   │
    │ Cache   │ │   LLM   │
    └─────────┘ └────┬────┘
                   │
                   ▼
            ┌──────────┐
            │ Cache    │ ← Async write-back
            │ Response │
            └────┬─────┘
                 │
                 ▼
          ┌──────────┐
          │ Return   │
          │ Response │
          └──────────┘
```

## Technology Stack

### Core Technologies

- **Language**: Rust (2024 edition)
- **Web Framework**: Axum (async HTTP server)
- **Runtime**: Tokio (async runtime)
- **Serialization**: Serde (JSON/binary)
- **Vector Database**: Redis Stack with RediSearch
- **Embeddings**: OpenAI text-embedding-ada-002
- **LLM**: OpenAI GPT-4/GPT-3.5-turbo
- **Protocol**: Model Context Protocol (MCP)

### Infrastructure

- **Containerization**: Docker
- **Orchestration**: Kubernetes
- **Monitoring**: Prometheus + Grafana
- **Tracing**: OpenTelemetry + Jaeger
- **Logging**: Structured JSON logs
- **CI/CD**: GitHub Actions

## Configuration Management

### Configuration Layers

1. **File-based**: `config/{environment}.toml`
2. **Environment Variables**: Override config files
3. **Defaults**: Built-in fallbacks

### Priority Order

```
Environment Variables → Config Files → Default Values
```

### Example Configuration

```toml
[server]
host = "0.0.0.0"
port = 8080
max_connections = 100

[redis]
url = "redis://localhost:6379"
pool_size = 10
circuit_breaker_threshold = 5
circuit_breaker_cooldown = 60

[cache]
similarity_threshold = 0.95
max_cache_size = 10000
ttl_seconds = 3600

[llm]
upstream_url = "https://api.openai.com/v1"
api_key = "${OPENAI_API_KEY}"
model = "gpt-4"
timeout = 30000
max_retries = 3
```

## Resilience Patterns

### 1. Circuit Breaker

**Purpose**: Prevent cascading failures

**Implementation**:
- Threshold: 5 consecutive failures
- Cooldown: 60 seconds
- States: Closed → Open → Half-Open → Closed

**Usage**: Redis, LLM API calls

### 2. Retry Logic

**Purpose**: Handle transient failures

**Implementation**:
- Max retries: 3
- Backoff: Exponential (1s, 2s, 4s)
- Applies to: LLM API, embedding generation

### 3. Request Deduplication

**Purpose**: Prevent concurrent identical requests

**Implementation**:
- Semaphore-based (1 permit per request key)
- Request key: hash(prompt + model)
- Automatic cleanup after completion

### 4. Graceful Degradation

**Purpose**: Maintain availability during partial failures

**Implementation**:
- Fallback to upstream LLM if cache fails
- Return cached results with lower similarity if needed
- Circuit breaker trips before complete failure

## Performance Characteristics

### Scalability

- **Horizontal Scaling**: Stateless design enables multiple instances
- **Vertical Scaling**: CPU and memory intensive (embeddings)
- **Cache Sizing**: 10K+ entries per instance
- **Request Rate**: 100+ req/s per instance (cache hits)

### Latency

- **Cache Hit**: ~10ms (Redis + similarity search)
- **Cache Miss**: ~2-5s (embedding + LLM API)
- **P95 Latency**: <100ms (cache hit scenario)

### Resource Usage

**Per Instance**:
- Memory: 256MB - 1GB
- CPU: 0.1 - 1.0 cores
- Network: 10 Mbps (burst)

**Redis**:
- Memory: 2GB+ (for embeddings)
- CPU: 0.5 cores
- Network: 1 Gbps

## Security Architecture

### API Key Management

- Storage: Kubernetes secrets / environment variables
- Rotation: Monthly recommended
- Auditing: Log all API calls with keys
- Scope: Minimal required permissions

### Network Security

- TLS: All connections encrypted
- Network policies: Restrict pod-to-pod communication
- Ingress: Only expose required ports
- Egress: Control external API access

### Data Protection

- Prompt redaction in logs
- API key masking in metrics
- Encryption at rest (Redis)
- Secure token handling

## Monitoring & Observability

### Key Metrics

**Service Health**:
- Uptime: 99.9% target
- Error rate: <0.1%
- Response time: P95 <100ms (cache hit)

**Cache Performance**:
- Hit rate: >80% target
- Similarity distribution: Monitor threshold effectiveness
- Cache size: Monitor growth and eviction

**LLM Operations**:
- API success rate: >95%
- Token usage: Track costs and trends
- Rate limiting: Monitor against quotas

### Alerting

**Critical Alerts**:
- Service down (>1 minute)
- Error rate spike (>5%)
- Cache failure (>50% miss rate)
- LLM API failure (>10%)

**Warning Alerts**:
- High memory usage (>80%)
- High latency (P95 >500ms)
- Low cache hit rate (<70%)

## Deployment Patterns

### Development

```
docker-compose up
cargo run
```

### Staging

```
kubectl apply -f k8s/base/
kubectl apply -f k8s/staging/
```

### Production

```
kubectl apply -f k8s/base/
kubectl apply -f k8s/production/
```

### Multi-Region

```
Region 1: us-east-1 (3 instances)
Region 2: us-west-2 (3 instances)
Global Load Balancer → Route to nearest region
```

## Future Enhancements

### Planned Features

1. **Multi-LLM Support**: Anthropic, Cohere, local models
2. **Advanced Caching**: Hierarchical caching, cache warming
3. **Streaming Responses**: Server-sent events for real-time updates
4. **Federated Learning**: Distributed cache optimization
5. **Cost Optimization**: Dynamic threshold adjustment based on costs

### Architecture Evolution

1. **Service Mesh**: Istio for advanced traffic management
2. **Event-Driven**: Kafka for cache invalidation events
3. **Edge Deployment**: Cloudflare Workers for edge caching
4. **ML Pipeline**: Automated cache optimization

---

*Last Updated: 2024-04-17*  
*Version: 1.0.0*  
*Maintained by: Platform Team*