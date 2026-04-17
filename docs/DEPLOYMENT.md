# Production Deployment Guide for Aegis-MCP

This guide covers deploying and operating Aegis-MCP in production environments.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start with Docker Compose](#quick-start-with-docker-compose)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Environment Configuration](#environment-configuration)
- [Monitoring and Observability](#monitoring-and-observability)
- [Performance Tuning](#performance-tuning)
- [Security Considerations](#security-considerations)
- [Troubleshooting](#troubleshooting)
- [Disaster Recovery](#disaster-recovery)

## Prerequisites

### System Requirements

- **Docker**: 20.10+ with Docker Compose 2.0+
- **Kubernetes**: 1.21+ (for K8s deployments)
- **Memory**: 512MB minimum, 2GB+ recommended for production
- **CPU**: 1 core minimum, 2+ cores recommended
- **Storage**: 10GB+ for Redis persistence
- **Network**: Stable internet connection for LLM APIs

### Software Requirements

- Docker Engine 20.10+
- Docker Compose 2.0+
- kubectl 1.21+
- Helm 3.0+ (optional)
- Redis Stack 7.2+

### External Dependencies

- OpenAI API key (or compatible LLM provider)
- Redis Stack with vector search capabilities
- Optional: Jaeger for distributed tracing
- Optional: Prometheus/Grafana for metrics

## Quick Start with Docker Compose

### 1. Clone and Configure

```bash
git clone https://github.com/phanidharguttikonda0/Ageis-MCP.git
cd Ageis-MCP

# Copy example environment file
cp .env.example .env

# Edit .env with your configuration
nano .env
```

### 2. Build and Start

```bash
# Build and start all services
docker-compose up -d

# Check service status
docker-compose ps
docker-compose logs -f aegis-mcp
```

### 3. Verify Deployment

```bash
# Health check
curl http://localhost:8080/health

# Metrics endpoint
curl http://localhost:8080/metrics

# Cache statistics
curl http://localhost:8080/stats/cache
```

### 4. Access Services

- **Aegis-MCP API**: http://localhost:8080
- **Redis Insight**: http://localhost:8001
- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000 (admin/admin)

## Kubernetes Deployment

### 1. Prepare Your Cluster

```bash
# Verify cluster connectivity
kubectl cluster-info
kubectl get nodes

# Create namespace (optional)
kubectl create namespace aegis-mcp
```

### 2. Create Secrets

```bash
# Create secret with your API keys
kubectl create secret generic aegis-secrets \
  --from-literal=openai-api-key='sk-...' \
  --from-literal=llm-upstream-url='https://api.openai.com/v1' \
  --from-literal=llm-model='gpt-4' \
  --namespace=default
```

### 3. Deploy Base Components

```bash
# Deploy Redis
kubectl apply -f k8s/base/redis.yaml

# Deploy Aegis-MCP
kubectl apply -f k8s/base/deployment.yaml

# Verify deployment
kubectl get pods -l app=aegis-mcp
kubectl get services
```

### 4. Deploy Production Components

```bash
# Deploy Ingress and monitoring
kubectl apply -f k8s/production/ingress.yaml

# Verify ingress
kubectl get ingress aegis-mcp-ingress
```

### 5. Scaling and Updates

```bash
# Scale deployment
kubectl scale deployment/aegis-mcp --replicas=5

# Update deployment
kubectl set image deployment/aegis-mcp aegis-mcp=aegis-mcp:v2.0.0

# Rollback if needed
kubectl rollout undo deployment/aegis-mcp
```

## Environment Configuration

### Required Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `AEGIS_LLM_API_KEY` | OpenAI API key | - | `sk-...` |
| `AEGIS_LLM_UPSTREAM_URL` | LLM API endpoint | `https://api.openai.com/v1` | Custom endpoint |
| `AEGIS_REDIS_URL` | Redis connection URL | `redis://localhost:6379` | `redis://redis:6379` |

### Optional Variables

| Variable | Description | Default | Recommended |
|----------|-------------|---------|-------------|
| `AEGIS_SERVER_HOST` | Server bind address | `0.0.0.0` | `0.0.0.0` |
| `AEGIS_SERVER_PORT` | Server port | `8080` | `8080` |
| `AEGIS_CACHE_SIMILARITY_THRESHOLD` | Similarity threshold (0-1) | `0.95` | `0.90-0.97` |
| `AEGIS_CACHE_TTL_SECONDS` | Cache TTL | `3600` | `7200` |
| `RUST_LOG` | Logging level | `info` | `info` |
| `AEGIS_LLM_MODEL` | LLM model | `gpt-4` | `gpt-3.5-turbo` |

### Configuration Files

1. **Docker Compose**: Set in `docker-compose.yml`
2. **Kubernetes**: Use ConfigMaps and Secrets
3. **File-based**: Use `config/production.toml`

## Monitoring and Observability

### Metrics Endpoints

Aegis-MCP exposes Prometheus metrics at `/metrics`:

- **HTTP Metrics**: Request count, latency, error rate
- **Cache Metrics**: Hit rate, miss rate, similarity scores
- **LLM Metrics**: API calls, token usage, costs
- **System Metrics**: Memory, CPU, goroutines

### Key Metrics to Monitor

```yaml
# Application Health
up{job="aegis-mcp"}                    # Service availability
http_requests_total                     # Total requests
http_request_duration_seconds           # Request latency

# Cache Performance
cache_hits_total                        # Cache hits
cache_misses_total                      # Cache misses
cache_hit_rate                          # Hit ratio

# LLM Operations
llm_requests_total                      # LLM API calls
llm_tokens_used_total                   # Token consumption
llm_errors_total                        # API failures

# Resource Usage
container_memory_usage_bytes            # Memory usage
container_cpu_usage_seconds_total       # CPU usage
```

### Distributed Tracing

```bash
# Enable Jaeger tracing
export AEGIS_JAEGER_ENDPOINT="http://jaeger:4318"

# View traces at
open http://localhost:16686
```

### Log Aggregation

Aegis-MCP uses structured JSON logging:

```json
{
  "timestamp": "2024-04-17T10:00:00Z",
  "level": "info",
  "target": "aegis_core::cache",
  "message": "Cache hit for request",
  "request_id": "req-123",
  "similarity_score": 0.98
}
```

## Performance Tuning

### Cache Optimization

1. **Similarity Threshold**: Adjust based on use case
   - High precision: `0.95-0.98`
   - High recall: `0.85-0.92`

2. **TTL Settings**: Balance cost vs freshness
   - Static content: `7200s` (2 hours)
   - Dynamic content: `1800s` (30 minutes)

3. **Cache Size**: Monitor and adjust
   ```bash
   # Monitor cache size
   curl http://localhost:8080/stats/cache | jq '.cache_entries'
   ```

### Connection Pooling

```toml
[redis]
pool_size = 20              # Increase for high traffic
connection_timeout = 3000   # Reduce for faster failover
max_lifetime = 1800         # Balance connection reuse
```

### LLM Rate Limiting

```toml
[llm]
max_retries = 2              # Reduce to fail fast
timeout = 15000             # 15 second timeout
request_timeout = 10000     # Per-request timeout
```

### Resource Limits

**Kubernetes Resource Requests:**

```yaml
resources:
  requests:
    memory: "256Mi"
    cpu: "200m"
  limits:
    memory: "1Gi"
    cpu: "1000m"
```

**Horizontal Pod Autoscaling:**

```yaml
autoscaling:
  minReplicas: 3
  maxReplicas: 20
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80
```

## Security Considerations

### API Key Management

1. **Never commit API keys** to version control
2. **Use Kubernetes secrets** or environment variables
3. **Rotate keys regularly** (recommend monthly)
4. **Monitor usage** for anomalies

### Network Security

```yaml
# Kubernetes NetworkPolicy
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: aegis-mcp-network-policy
spec:
  podSelector:
    matchLabels:
      app: aegis-mcp
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - podSelector: {}
    ports:
    - protocol: TCP
      port: 8080
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: redis
    ports:
    - protocol: TCP
      port: 6379
```

### TLS Configuration

```nginx
# TLS configuration in Ingress
spec:
  tls:
  - hosts:
    - aegis-mcp.yourdomain.com
    secretName: aegis-mcp-tls
```

## Troubleshooting

### Common Issues

#### 1. Service Won't Start

**Symptoms**: Container exits immediately

**Diagnosis**:
```bash
docker logs aegis-mcp
kubectl logs -l app=aegis-mcp
```

**Solutions**:
- Check environment variables are set
- Verify Redis connectivity
- Validate API keys

#### 2. High Memory Usage

**Symptoms**: OOMKilled or memory spikes

**Diagnosis**:
```bash
kubectl top pod -l app=aegis-mcp
```

**Solutions**:
- Increase memory limits
- Reduce cache size
- Tune GC settings

#### 3. Cache Miss Rate High

**Symptoms**: >50% cache miss rate

**Diagnosis**:
```bash
curl http://localhost:8080/stats/cache | jq '.hit_rate'
```

**Solutions**:
- Lower similarity threshold
- Increase cache TTL
- Check embedding quality

#### 4. LLM API Failures

**Symptoms**: Increased error rate

**Diagnosis**:
```bash
curl http://localhost:8080/metrics | grep llm_errors
```

**Solutions**:
- Check API key validity
- Verify rate limits
- Implement retry backoff

### Debug Mode

```bash
# Enable debug logging
export RUST_LOG=debug
export AEGIS_LOG_LEVEL=debug

# Run with debug output
cargo run
# or
docker-compose up
```

### Health Checks

```bash
# Comprehensive health check
#!/bin/bash
echo "Checking Aegis-MCP health..."

# Service health
curl -f http://localhost:8080/health || exit 1

# Redis connectivity
docker exec aegis-redis redis-cli ping || exit 1

# Cache stats
curl http://localhost:8080/stats/cache | jq .

echo "All systems operational"
```

## Disaster Recovery

### Backup Strategy

1. **Redis Data Backup**:
   ```bash
   # Automated backup
   kubectl exec -it redis-0 -- redis-cli BGSAVE
   
   # Copy RDB files
   kubectl cp default/redis-0:/data/dump.rdb ./backup/
   ```

2. **Configuration Backup**:
   ```bash
   # Backup Kubernetes configs
   kubectl get configmaps -o yaml > backup/configmaps.yaml
   kubectl get secrets -o yaml > backup/secrets.yaml
   ```

### Recovery Procedures

1. **Redis Recovery**:
   ```bash
   # Restore from backup
   kubectl cp backup/dump.rdb default/redis-0:/data/dump.rdb
   kubectl exec -it redis-0 -- redis-cli SHUTDOWN NOSAVE
   ```

2. **Service Recovery**:
   ```bash
   # Rollback deployment
   kubectl rollout undo deployment/aegis-mcp
   
   # Scale up if needed
   kubectl scale deployment/aegis-mcp --replicas=10
   ```

### High Availability Setup

1. **Multi-Region Deployment**:
   - Deploy across multiple Kubernetes clusters
   - Use global load balancer
   - Implement cross-region replication

2. **Redis HA**:
   ```yaml
   # Redis Sentinel configuration
   sentinel monitor mymaster redis 6379 2
   sentinel down-after-milliseconds mymaster 5000
   sentinel failover-timeout mymaster 10000
   ```

## Maintenance

### Rolling Updates

```bash
# Zero-downtime deployment
kubectl set image deployment/aegis-mcp \
  aegis-mcp=aegis-mcp:v2.0.0 \
  --timeout=5m

# Monitor rollout
kubectl rollout status deployment/aegis-mcp
```

### Cache Warming

```bash
# Pre-populate cache after deployment
curl -X POST http://localhost:8080/api/v1/cache/warm \
  -H "Content-Type: application/json" \
  -d '{"prompts": ["common prompt 1", "common prompt 2"]}'
```

### Performance Testing

```bash
# Load testing with k6
k6 run --vus 10 --duration 30s load-test.js

# Benchmark cache performance
ab -n 10000 -c 100 http://localhost:8080/api/v1/cache/lookup
```

## Support and Resources

- **Documentation**: https://docs.yourdomain.com/aegis-mcp
- **GitHub Issues**: https://github.com/phanidharguttikonda0/Ageis-MCP/issues
- **Runbooks**: https://docs.yourdomain.com/runbooks
- **Slack**: #aegis-mcp-support

---

*Last Updated: 2024-04-17*
*Version: 1.0.0*