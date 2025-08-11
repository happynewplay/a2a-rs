# A2A Gateway

A high-performance gateway for the Agent-to-Agent (A2A) protocol that provides service discovery, load balancing, authentication, and monitoring capabilities.

## Features

- **Service Discovery**: Automatic discovery and registration of A2A agents
- **Load Balancing**: Multiple strategies (round-robin, weighted, least-connections, etc.)
- **Protocol Support**: HTTP and WebSocket with seamless conversion
- **Authentication**: Integrated auth proxy with multiple schemes (Bearer Token, API Key, JWT, OAuth2)
- **Monitoring**: Comprehensive metrics, tracing, and health checks
- **Configuration**: Flexible configuration with hot-reload support

## Architecture

The gateway follows a hexagonal architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────┐
│                Gateway Core                 │
├─────────────┬─────────────┬─────────────────┤
│   Adapter   │ Application │     Domain      │
│    Layer    │    Layer    │     Layer       │
│             │             │                 │
│ HTTP/WS     │ Routing     │ Service         │
│ Auth        │ Load Bal.   │ Registry        │
│ Discovery   │ Protocol    │ Health          │
│ Monitoring  │ Conversion  │ Monitor         │
└─────────────┴─────────────┴─────────────────┘
```

## Quick Start

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd a2a-rs/a2a-gateway

# Build the gateway
cargo build --release
```

### Configuration

Copy the example configuration and modify as needed:

```bash
cp gateway.example.yaml gateway.yaml
```

### Running

```bash
# Run with default configuration
cargo run

# Run with custom configuration
cargo run -- --config custom-gateway.yaml

# Run with custom bind address
cargo run -- --bind 0.0.0.0:9000

# Enable metrics
cargo run -- --metrics --metrics-bind 0.0.0.0:9090
```

## Configuration

The gateway uses YAML configuration files. See `gateway.example.yaml` for a complete example.

### Server Configuration

```yaml
server:
  bind_address: "0.0.0.0:8080"
  enable_websocket: true
  websocket_address: "0.0.0.0:8081"
  request_timeout: "30s"
  max_connections: 1000
```

### Service Discovery

```yaml
discovery:
  strategy:
    type: "Static"
  static_services:
    - name: "agent-1"
      url: "http://localhost:3001"
      weight: 100
      tags: ["finance"]
```

### Load Balancing

```yaml
load_balancing:
  strategy: "RoundRobin"  # RoundRobin, WeightedRoundRobin, LeastConnections, Random, IpHash
  health_check:
    interval: "30s"
    timeout: "5s"
    failure_threshold: 3
    success_threshold: 2
```

### Authentication

```yaml
auth:
  enabled: true
  strategies:
    - type: "BearerToken"
      tokens: ["token1", "token2"]
    - type: "Jwt"
      secret: "your-jwt-secret"
```

### Monitoring

```yaml
monitoring:
  metrics:
    enabled: true
    bind_address: "0.0.0.0:9090"
    path: "/metrics"
  tracing:
    enabled: true
    service_name: "a2a-gateway"
```

## API Endpoints

### Gateway Endpoints

- `GET /health` - Health check
- `GET /metrics` - Prometheus metrics (if enabled)
- `GET /services` - List registered services
- `POST /reload` - Reload configuration

### A2A Protocol Endpoints

The gateway proxies all A2A protocol endpoints:

- `GET /.well-known/agent-card` - Agent card discovery
- `POST /tasks/send` - Send task to agent
- `POST /tasks/sendSubscribe` - Send task with streaming
- `GET /tasks/{id}` - Get task status
- `WebSocket /ws` - WebSocket connection for real-time updates

## Development

### Project Structure

```
src/
├── adapter/          # Adapter layer (HTTP, WebSocket, etc.)
│   ├── auth.rs      # Authentication adapters
│   ├── discovery.rs # Service discovery adapters
│   ├── http.rs      # HTTP protocol adapter
│   ├── monitoring.rs # Monitoring and metrics adapters
│   └── websocket.rs # WebSocket protocol adapter
├── application/      # Application layer (Gateway, Router, etc.)
│   ├── gateway.rs   # Main gateway application
│   ├── load_balancer.rs # Load balancing logic
│   ├── protocol_converter.rs # Protocol conversion
│   └── router.rs    # Request routing
├── config/          # Configuration management
│   ├── manager.rs   # Configuration manager with hot reload
│   └── mod.rs       # Configuration types and validation
├── domain/          # Domain layer (Service registry, etc.)
│   ├── health.rs    # Health check models
│   ├── routing.rs   # Routing and load balancing models
│   └── service.rs   # Service registry and models
├── port/            # Port layer (interfaces)
│   ├── authentication.rs # Authentication interfaces
│   ├── load_balancing.rs # Load balancing interfaces
│   ├── monitoring.rs # Monitoring interfaces
│   └── service_discovery.rs # Service discovery interfaces
├── error.rs         # Error types
├── lib.rs           # Library root
└── main.rs          # Binary entry point
```

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with features
cargo build --features "metrics,tracing"

# Check code formatting
cargo fmt --check

# Run clippy for linting
cargo clippy -- -D warnings
```

### Testing

```bash
# Run all tests
cargo test

# Run unit tests only
cargo test --lib

# Run integration tests
cargo test --test integration_test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_service_registry

# Run with coverage (requires cargo-tarpaulin)
cargo tarpaulin --out html
```

### Performance Testing

```bash
# Install wrk for load testing
# On Ubuntu/Debian: sudo apt install wrk
# On macOS: brew install wrk

# Start the gateway
cargo run

# Run load test
wrk -t12 -c400 -d30s http://localhost:8080/health

# Test with different endpoints
wrk -t12 -c400 -d30s -s scripts/post.lua http://localhost:8080/tasks/send
```

## Deployment

### Docker Deployment

Create a `Dockerfile`:

```dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/a2a-gateway /usr/local/bin/a2a-gateway
COPY gateway.yaml /etc/a2a-gateway/gateway.yaml

EXPOSE 8080 8081 9090

CMD ["a2a-gateway", "--config", "/etc/a2a-gateway/gateway.yaml"]
```

Build and run:

```bash
# Build image
docker build -t a2a-gateway .

# Run container
docker run -p 8080:8080 -p 8081:8081 -p 9090:9090 a2a-gateway
```

### Kubernetes Deployment

Create `k8s-deployment.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: a2a-gateway
spec:
  replicas: 3
  selector:
    matchLabels:
      app: a2a-gateway
  template:
    metadata:
      labels:
        app: a2a-gateway
    spec:
      containers:
      - name: a2a-gateway
        image: a2a-gateway:latest
        ports:
        - containerPort: 8080
        - containerPort: 8081
        - containerPort: 9090
        env:
        - name: A2A_GATEWAY_LOG_LEVEL
          value: "info"
        - name: A2A_GATEWAY_METRICS_ENABLED
          value: "true"
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
---
apiVersion: v1
kind: Service
metadata:
  name: a2a-gateway-service
spec:
  selector:
    app: a2a-gateway
  ports:
  - name: http
    port: 8080
    targetPort: 8080
  - name: websocket
    port: 8081
    targetPort: 8081
  - name: metrics
    port: 9090
    targetPort: 9090
  type: LoadBalancer
```

Deploy:

```bash
kubectl apply -f k8s-deployment.yaml
```

### Environment Variables

The gateway supports configuration via environment variables:

- `A2A_GATEWAY_BIND_ADDRESS`: Server bind address
- `A2A_GATEWAY_ENABLE_WEBSOCKET`: Enable WebSocket support
- `A2A_GATEWAY_WEBSOCKET_ADDRESS`: WebSocket bind address
- `A2A_GATEWAY_MAX_CONNECTIONS`: Maximum concurrent connections
- `A2A_GATEWAY_AUTH_ENABLED`: Enable authentication
- `A2A_GATEWAY_METRICS_ENABLED`: Enable metrics collection
- `A2A_GATEWAY_METRICS_BIND_ADDRESS`: Metrics server bind address
- `A2A_GATEWAY_TRACING_ENABLED`: Enable distributed tracing
- `A2A_GATEWAY_JAEGER_ENDPOINT`: Jaeger collector endpoint
- `A2A_GATEWAY_LOG_LEVEL`: Log level (trace, debug, info, warn, error)
- `A2A_GATEWAY_LOG_FORMAT`: Log format (text, json)

### Monitoring and Observability

The gateway provides comprehensive monitoring capabilities:

#### Metrics

Prometheus metrics are available at `/metrics` endpoint:

- `http_requests_total`: Total HTTP requests
- `http_request_duration_seconds`: Request duration histogram
- `active_connections`: Current active connections
- `service_health_status`: Service health status
- `load_balancer_requests_total`: Load balancer request counts

#### Health Checks

Health check endpoint at `/health` provides:

```json
{
  "status": "healthy",
  "services": {
    "total": 3,
    "healthy": 2
  },
  "timestamp": "2023-12-07T10:30:00Z"
}
```

#### Distributed Tracing

When enabled, the gateway exports traces to Jaeger for request flow analysis.

### Production Considerations

1. **Resource Limits**: Set appropriate CPU and memory limits
2. **Health Checks**: Configure liveness and readiness probes
3. **Logging**: Use structured logging in production
4. **Security**: Enable authentication and use HTTPS
5. **Monitoring**: Set up alerts for key metrics
6. **Backup**: Regularly backup configuration files
7. **Updates**: Plan for rolling updates with zero downtime

## License

This project is licensed under the MIT OR Apache-2.0 license.
