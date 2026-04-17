# Troubleshooting Guide for Aegis-MCP

## Common Issues and Solutions

### 1. Service Startup Issues

#### Problem: Container exits immediately

**Symptoms**:
```
docker logs aegis-mcp
# Container exits with code 1
```

**Diagnosis**:
```bash
# Check logs
docker-compose logs aegis-mcp
kubectl logs -l app=aegis-mcp

# Check configuration
docker-compose config
kubectl get configmap aegis-mcp-config -o yaml
```

**Solutions**:
1. **Missing Environment Variables**:
   ```bash
   # Check required variables
   docker-compose config | grep AEGIS_
   
   # Add missing variables to .env file
   echo "OPENAI_API_KEY=sk-..." >> .env
   ```

2. **Redis Connection Failure**:
   ```bash
   # Verify Redis is running
   docker-compose ps redis
   kubectl get pods -l app=redis
   
   # Test connectivity
   docker-compose exec aegis-mcp ping redis-service
   ```

3. **Invalid Configuration**:
   ```bash
   # Validate configuration
   cargo run --bin aegis-mcp -- --help
   
   # Check syntax
   cat config/production.toml
   ```

#### Problem: Service starts but can't handle requests

**Symptoms**:
```bash
curl http://localhost:8080/health
# Returns 503 Service Unavailable
```

**Diagnosis**:
```bash
# Check health endpoints
curl http://localhost:8080/health
curl http://localhost:8080/metrics
curl http://localhost:8080/stats/cache

# Check pod status
kubectl get pods -l app=aegis-mcp
kubectl describe pod <pod-name>
```

**Solutions**:
1. **Redis Not Ready**:
   ```bash
   # Wait for Redis to be ready
   kubectl wait --for=condition=ready pod -l app=redis
   
   # Check Redis health
   kubectl exec -it redis-0 -- redis-cli ping
   ```

2. **Missing Dependencies**:
   ```bash
   # Verify all services are running
   docker-compose ps
   kubectl get services
   
   # Check network connectivity
   docker-compose exec aegis-mcp nc -zv redis-service 6379
   ```

3. **Resource Exhaustion**:
   ```bash
   # Check resource usage
   kubectl top pod -l app=aegis-mcp
   
   # Increase resources if needed
   kubectl set resources deployment/aegis-mcp \
     --requests=memory=512Mi,cpu=500m \
     --limits=memory=2Gi,cpu=1000m
   ```

### 2. Cache Performance Issues

#### Problem: Low cache hit rate (<70%)

**Symptoms**:
```bash
curl http://localhost:8080/stats/cache | jq '.hit_rate'
# Returns: 0.45 (45%)
```

**Diagnosis**:
```bash
# Check cache statistics
curl http://localhost:8080/stats/cache | jq '.'

# Monitor similarity scores
curl http://localhost:8080/metrics | grep cache_similarity
```

**Solutions**:
1. **Adjust Similarity Threshold**:
   ```bash
   # Lower threshold for more matches
   export AEGIS_CACHE_SIMILARITY_THRESHOLD=0.85
   
   # Or update ConfigMap
   kubectl patch configmap aegis-mcp-config \
     --type=json \
     -p='{"data":{"AEGIS_CACHE_SIMILARITY_THRESHOLD":"0.85"}}'
   ```

2. **Increase Cache TTL**:
   ```bash
   # Longer TTL keeps entries longer
   export AEGIS_CACHE_TTL_SECONDS=7200
   ```

3. **Cache Warming**:
   ```bash
   # Pre-populate cache with common prompts
   curl -X POST http://localhost:8080/api/v1/cache/warm \
     -H "Content-Type: application/json" \
     -d '{"prompts": ["What is Rust?", "Explain AI"]}'
   ```

#### Problem: Cache misses are slow (>100ms)

**Symptoms**:
```bash
# Cache misses take >100ms
curl http://localhost:8080/metrics | grep cache_search_duration
```

**Diagnosis**:
```bash
# Check Redis performance
docker-compose exec redis redis-cli --latency-history
kubectl exec -it redis-0 -- redis-cli --latency-history

# Monitor network latency
docker-compose exec aegis-mcp ping -c 10 redis-service
```

**Solutions**:
1. **Optimize Redis Configuration**:
   ```ini
   # redis.conf
   maxmemory-policy allkeys-lru
   save 900 1
   save 300 10
   ```
   
2. **Increase Connection Pool**:
   ```bash
   export AEGIS_REDIS_POOL_SIZE=20
   ```

3. **Use Local Caching**:
   ```bash
   # Enable embedding cache
   export AEGIS_EMBEDDING_CACHE_ENABLED=true
   ```

### 3. LLM API Issues

#### Problem: High LLM API failure rate (>10%)

**Symptoms**:
```bash
curl http://localhost:8080/metrics | grep llm_errors
# llm_errors_total{job="aegis-mcp"} 150
```

**Diagnosis**:
```bash
# Check LLM client status
curl http://localhost:8080/metrics | grep llm_

# Test API connectivity
curl -X POST http://localhost:8080/api/v1/test/llm \
  -H "Content-Type: application/json" \
  -d '{"prompt": "test"}'

# Check rate limits
curl https://api.openai.com/v1/rate_limits
```

**Solutions**:
1. **Check API Key Validity**:
   ```bash
   # Verify API key is valid
   export OPENAI_API_KEY=sk-...
   curl https://api.openai.com/v1/models \
     -H "Authorization: Bearer $OPENAI_API_KEY"
   ```

2. **Implement Rate Limiting**:
   ```bash
   # Reduce concurrent requests
   export AEGIS_LLM_MAX_CONCURRENT_REQUESTS=5
   
   # Or use Kubernetes Horizontal Pod Autoscaler
   kubectl autoscale deployment/aegis-mcp \
     --min=3 --max=10 --cpu-percent=70
   ```

3. **Optimize Retry Logic**:
   ```bash
   # Reduce retries for faster failure
   export AEGIS_LLM_MAX_RETRIES=1
   ```

#### Problem: High LLM API costs

**Symptoms**:
```bash
# Bill is much higher than expected
curl http://localhost:8080/metrics | grep llm_tokens
# Shows high token usage
```

**Diagnosis**:
```bash
# Analyze token usage
curl http://localhost:8080/metrics | grep llm_tokens_total

# Check cache hit rate
curl http://localhost:8080/stats/cache | jq '.hit_rate'
```

**Solutions**:
1. **Improve Cache Hit Rate**:
   - Lower similarity threshold
   - Increase cache TTL
   - Implement cache warming

2. **Use Cost-Effective Models**:
   ```bash
   # Switch to GPT-3.5-turbo
   export AEGIS_LLM_MODEL=gpt-3.5-turbo
   ```

3. **Implement Request Batching**:
   ```bash
   # Batch similar requests
   export AEGIS_BATCH_ENABLED=true
   ```

### 4. Memory Issues

#### Problem: Out of Memory (OOMKilled)

**Symptoms**:
```bash
kubectl get pods
# NAME              READY   STATUS     RESTARTS
# aegis-mcp-xxxxx   0/1     OOMKilled  3
```

**Diagnosis**:
```bash
# Check memory usage
kubectl top pod -l app=aegis-mcp
kubectl exec -it aegis-mcp-xxxxx -- free -h

# Check memory limits
kubectl describe pod aegis-mcp-xxxxx | grep -A 5 Limits
```

**Solutions**:
1. **Increase Memory Limits**:
   ```yaml
   resources:
     requests:
       memory: "512Mi"
     limits:
       memory: "2Gi"
   ```

2. **Reduce Cache Size**:
   ```bash
   export AEGIS_CACHE_MAX_SIZE=5000
   ```

3. **Optimize Embedding Cache**:
   ```bash
   # Limit embedding cache size
   export AEGIS_EMBEDDING_CACHE_MAX_SIZE=1000
   ```

#### Problem: Memory leaks

**Symptoms**:
```bash
# Memory usage increases over time
kubectl top pod -l app=aegis-mcp --containers
```

**Diagnosis**:
```bash
# Enable memory profiling
export RUST_LOG=debug
export AEGIS_DEBUG_MEMORY=true

# Check for connection leaks
curl http://localhost:8080/metrics | grep connections
```

**Solutions**:
1. **Update Dependencies**:
   ```bash
   cargo update
   ```

2. **Implement Connection Pooling**:
   ```bash
   # Ensure connection pooling is enabled
   export AEGIS_CONNECTION_POOL_ENABLED=true
   ```

3. **Regular Pod Restarts**:
   ```yaml
   # Add periodic restarts
   spec:
     template:
       spec:
         containers:
         - name: aegis-mcp
           # ... other config
           lifecycle:
             postStart:
               exec:
                 command: ["/bin/sh", "-c", "sleep 3600 && exit 0"]
   ```

### 5. Network Issues

#### Problem: Connection timeouts

**Symptoms**:
```bash
curl http://localhost:8080/health
# curl: (28) Connection timed out
```

**Diagnosis**:
```bash
# Check network connectivity
kubectl exec -it aegis-mcp-xxxxx -- ping redis-service
kubectl exec -it aegis-mcp-xxxxx -- nc -zv redis-service 6379

# Check DNS resolution
kubectl exec -it aegis-mcp-xxxxx -- nslookup redis-service
```

**Solutions**:
1. **Fix DNS Configuration**:
   ```yaml
   # Add DNS search domains
   dnsPolicy: "None"
   dnsConfig:
     nameservers:
       - 8.8.8.8
     searches:
       - default.svc.cluster.local
       - svc.cluster.local
   ```

2. **Add Network Policies**:
   ```yaml
   apiVersion: networking.k8s.io/v1
   kind: NetworkPolicy
   metadata:
     name: aegis-mcp-network
   spec:
     podSelector:
       matchLabels:
         app: aegis-mcp
     policyTypes:
     - Ingress
     - Egress
     egress:
     - to:
       - podSelector:
           matchLabels:
             app: redis
     ```

3. **Adjust Timeouts**:
   ```bash
   # Increase connection timeout
   export AEGIS_REDIS_CONNECTION_TIMEOUT=10000
   ```

#### Problem: High network latency

**Symptoms**:
```bash
# Requests take >1s
curl -w "@-" -o /dev/null http://localhost:8080/health
# time_total: 1.234
```

**Diagnosis**:
```bash
# Test network latency
docker-compose exec aegis-mcp ping -c 10 redis-service

# Check network policies
kubectl get networkpolicies
```

**Solutions**:
1. **Use Sidecar Pattern**:
   ```yaml
   # Deploy Redis as sidecar
   spec:
     template:
       spec:
         containers:
         - name: aegis-mcp
         - name: redis
           image: redis:latest
   ```

2. **Optimize Keepalive Settings**:
   ```bash
   # Adjust TCP keepalive
   export AEGIS_TCP_KEEPALIVE=true
   export AEGIS_TCP_KEEPALIVE_IDLE=60
   ```

3. **Enable HTTP/2**:
   ```yaml
   # Use HTTP/2 for better performance
   apiVersion: networking.k8s.io/v1
   kind: Ingress
   metadata:
     annotations:
       nginx.ingress.kubernetes.io/http2-push-preload: "true"
   ```

### 6. Performance Degradation

#### Problem: Response time increased significantly

**Symptoms**:
```bash
# P95 latency increased from 100ms to 500ms
curl http://localhost:8080/metrics | grep http_request_duration_seconds
```

**Diagnosis**:
```bash
# Check resource usage
kubectl top pod -l app=aegis-mcp

# Analyze request patterns
curl http://localhost:8080/metrics | grep rate(http_requests)

# Check database performance
kubectl exec -it redis-0 -- redis-cli --latency-history
```

**Solutions**:
1. **Scale Horizontally**:
   ```bash
   kubectl scale deployment/aegis-mcp --replicas=10
   ```

2. **Enable Caching**:
   ```bash
   # Verify caching is enabled
   curl http://localhost:8080/stats/cache | jq '.caching_enabled'
   ```

3. **Optimize Database Queries**:
   ```bash
   # Add indexes to Redis
   kubectl exec -it redis-0 -- redis-cli FT.CREATE idx_prompt_embedding SCHEMA ...
   ```

#### Problem: High CPU usage

**Symptoms**:
```bash
kubectl top pod -l app=aegis-mcp
# NAME              CPU(cores)   MEMORY(bytes)
# aegis-mcp-xxxxx   900m (90%)    512Mi (50%)
```

**Diagnosis**:
```bash
# Profile CPU usage
kubectl exec -it aegis-mcp-xxxxx -- perf top

# Check for goroutine leaks
curl http://localhost:8080/metrics | grep goroutines
```

**Solutions**:
1. **Optimize Embedding Generation**:
   ```bash
   # Use cheaper embedding model
   export AEGIS_EMBEDDING_MODEL=text-embedding-3-small
   ```

2. **Implement Rate Limiting**:
   ```yaml
   apiVersion: networking.k8s.io/v1
   kind: Ingress
   metadata:
     annotations:
       nginx.ingress.kubernetes.io/limit-rps: "100"
   ```

3. **Scale Vertically**:
   ```yaml
   resources:
     limits:
       cpu: "2000m"
   ```

## Emergency Procedures

### Complete Service Outage

1. **Check Status**:
   ```bash
   kubectl get pods -l app=aegis-mcp
   kubectl get events --sort-by='.lastTimestamp'
   ```

2. **Rollback Deployment**:
   ```bash
   kubectl rollout undo deployment/aegis-mcp
   ```

3. **Scale Up Emergency**:
   ```bash
   kubectl scale deployment/aegis-mcp --replicas=20
   ```

### Data Loss Prevention

1. **Redis Backup**:
   ```bash
   kubectl exec -it redis-0 -- redis-cli BGSAVE
   kubectl cp default/redis-0:/data/dump.rdb ./backup/
   ```

2. **Configuration Backup**:
   ```bash
   kubectl get configmaps -o yaml > backup/configs.yaml
   ```

3. **Secrets Rotation**:
   ```bash
   # Rotate API keys
   kubectl delete secret aegis-secrets
   kubectl create secret generic aegis-secrets \
     --from-literal=openai-api-key='sk-new...'
   ```

### Performance Recovery

1. **Enable Maintenance Mode**:
   ```bash
   kubectl annotate deployment/aegis-mcp maintenance-mode="true"
   ```

2. **Clear Cache if Needed**:
   ```bash
   curl -X POST http://localhost:8080/api/v1/cache/clear \
     -H "Content-Type: application/json"
   ```

3. **Restart Services**:
   ```bash
   kubectl rollout restart deployment/aegis-mcp
   ```

## Debug Tools

### Live Debugging

```bash
# Port-forward to local machine
kubectl port-forward deployment/aegis-mcp 8080:8080

# Open shell in container
kubectl exec -it aegis-mcp-xxxxx -- /bin/sh

# Monitor logs in real-time
kubectl logs -f -l app=aegis-mcp
```

### Performance Profiling

```bash
# Enable profiling
export AEGIS_PROFILING_ENABLED=true

# Collect profiling data
curl http://localhost:8080/debug/pprof/heap > heap.prof
```

### Network Debugging

```bash
# Test connectivity
kubectl exec -it aegis-mcp-xxxxx -- curl -v http://redis-service:6379

# Trace network path
kubectl exec -it aegis-mcp-xxxxx -- traceroute redis-service
```

## Support Resources

### Getting Help

- **Documentation**: https://docs.yourdomain.com/aegis-mcp
- **GitHub Issues**: https://github.com/phanidharguttikonda0/Ageis-MCP/issues
- **Slack**: #aegis-mcp-support
- **Email**: support@yourdomain.com

### Diagnostic Information Collection

When reporting issues, collect:

```bash
# System information
kubectl version
docker version
uname -a

# Pod information
kubectl describe pod <pod-name>
kubectl logs <pod-name>

# Service information
kubectl get services
kubectl get endpoints

# Metrics
curl http://localhost:8080/metrics
curl http://localhost:8080/stats/cache
```

---

*Last Updated: 2024-04-17*  
*Version: 1.0.0*