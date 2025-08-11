# A2A Gateway Deployment Guide

This guide covers various deployment scenarios for the A2A Gateway.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Configuration](#configuration)
- [Deployment Methods](#deployment-methods)
  - [Docker Deployment](#docker-deployment)
  - [Kubernetes Deployment](#kubernetes-deployment)
  - [Systemd Service](#systemd-service)
  - [Binary Deployment](#binary-deployment)
- [Monitoring and Observability](#monitoring-and-observability)
- [Security Considerations](#security-considerations)
- [Troubleshooting](#troubleshooting)

## Prerequisites

- Rust 1.70+ (for building from source)
- Docker (for containerized deployment)
- Kubernetes cluster (for K8s deployment)
- Access to A2A agent services

## Configuration

### Environment Variables

The gateway supports configuration via environment variables:

```bash
# Server Configuration
export A2A_GATEWAY_BIND_ADDRESS="0.0.0.0:8080"
export A2A_GATEWAY_ENABLE_WEBSOCKET="true"
export A2A_GATEWAY_MAX_CONNECTIONS="1000"

# Authentication
export A2A_GATEWAY_AUTH_ENABLED="true"

# Monitoring
export A2A_GATEWAY_METRICS_ENABLED="true"
export A2A_GATEWAY_METRICS_BIND_ADDRESS="0.0.0.0:9090"

# Logging
export A2A_GATEWAY_LOG_LEVEL="info"
export A2A_GATEWAY_LOG_FORMAT="json"
```

### Configuration File

Create a `gateway.yaml` configuration file:

```yaml
server:
  bind_address: "0.0.0.0:8080"
  enable_websocket: true
  websocket_address: "0.0.0.0:8081"
  request_timeout: "30s"
  max_connections: 1000

discovery:
  strategy:
    type: "Static"
  health_check_interval: "30s"
  health_check_timeout: "5s"
  static_services:
    - name: "agent-1"
      url: "http://agent-1:3001"
      weight: 100
      tags: ["production"]
    - name: "agent-2"
      url: "http://agent-2:3001"
      weight: 100
      tags: ["production"]

load_balancing:
  strategy: "RoundRobin"
  health_check:
    interval: "30s"
    timeout: "5s"
    failure_threshold: 3
    success_threshold: 2

auth:
  enabled: true
  strategies:
    - type: "BearerToken"
      tokens: ["your-secure-token"]

monitoring:
  metrics:
    enabled: true
    bind_address: "0.0.0.0:9090"
    path: "/metrics"
  tracing:
    enabled: true
    service_name: "a2a-gateway"

logging:
  level: "info"
  format: "json"
```

## Deployment Methods

### Docker Deployment

#### 1. Create Dockerfile

```dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/a2a-gateway /usr/local/bin/
COPY gateway.yaml /etc/a2a-gateway/

EXPOSE 8080 8081 9090

CMD ["a2a-gateway", "--config", "/etc/a2a-gateway/gateway.yaml"]
```

#### 2. Build and Run

```bash
# Build image
docker build -t a2a-gateway:latest .

# Run container
docker run -d \
  --name a2a-gateway \
  -p 8080:8080 \
  -p 8081:8081 \
  -p 9090:9090 \
  -v $(pwd)/gateway.yaml:/etc/a2a-gateway/gateway.yaml \
  a2a-gateway:latest
```

#### 3. Docker Compose

```yaml
version: '3.8'

services:
  a2a-gateway:
    build: .
    ports:
      - "8080:8080"
      - "8081:8081"
      - "9090:9090"
    volumes:
      - ./gateway.yaml:/etc/a2a-gateway/gateway.yaml
    environment:
      - A2A_GATEWAY_LOG_LEVEL=info
    depends_on:
      - agent-1
      - agent-2
    restart: unless-stopped

  agent-1:
    image: your-a2a-agent:latest
    ports:
      - "3001:3001"

  agent-2:
    image: your-a2a-agent:latest
    ports:
      - "3002:3001"
```

### Kubernetes Deployment

#### 1. ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: a2a-gateway-config
data:
  gateway.yaml: |
    server:
      bind_address: "0.0.0.0:8080"
      enable_websocket: true
      max_connections: 1000
    discovery:
      strategy:
        type: "Kubernetes"
        namespace: "default"
    load_balancing:
      strategy: "RoundRobin"
    monitoring:
      metrics:
        enabled: true
        bind_address: "0.0.0.0:9090"
```

#### 2. Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: a2a-gateway
  labels:
    app: a2a-gateway
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
          name: http
        - containerPort: 8081
          name: websocket
        - containerPort: 9090
          name: metrics
        volumeMounts:
        - name: config
          mountPath: /etc/a2a-gateway
        env:
        - name: A2A_GATEWAY_LOG_LEVEL
          value: "info"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
      volumes:
      - name: config
        configMap:
          name: a2a-gateway-config
```

#### 3. Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: a2a-gateway
  labels:
    app: a2a-gateway
spec:
  type: LoadBalancer
  ports:
  - port: 80
    targetPort: 8080
    name: http
  - port: 8081
    targetPort: 8081
    name: websocket
  - port: 9090
    targetPort: 9090
    name: metrics
  selector:
    app: a2a-gateway
```

### Systemd Service

#### 1. Create Service File

```ini
# /etc/systemd/system/a2a-gateway.service
[Unit]
Description=A2A Gateway
After=network.target

[Service]
Type=simple
User=a2a-gateway
Group=a2a-gateway
WorkingDirectory=/opt/a2a-gateway
ExecStart=/opt/a2a-gateway/bin/a2a-gateway --config /opt/a2a-gateway/config/gateway.yaml
Restart=always
RestartSec=5
Environment=A2A_GATEWAY_LOG_LEVEL=info

[Install]
WantedBy=multi-user.target
```

#### 2. Install and Start

```bash
# Create user
sudo useradd --system --shell /bin/false a2a-gateway

# Create directories
sudo mkdir -p /opt/a2a-gateway/{bin,config,logs}
sudo chown -R a2a-gateway:a2a-gateway /opt/a2a-gateway

# Copy binary and config
sudo cp target/release/a2a-gateway /opt/a2a-gateway/bin/
sudo cp gateway.yaml /opt/a2a-gateway/config/

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable a2a-gateway
sudo systemctl start a2a-gateway

# Check status
sudo systemctl status a2a-gateway
```

### Binary Deployment

#### 1. Build Release Binary

```bash
cargo build --release
```

#### 2. Install

```bash
# Copy binary
sudo cp target/release/a2a-gateway /usr/local/bin/

# Create config directory
sudo mkdir -p /etc/a2a-gateway
sudo cp gateway.yaml /etc/a2a-gateway/

# Run
a2a-gateway --config /etc/a2a-gateway/gateway.yaml
```

## Monitoring and Observability

### Prometheus Metrics

The gateway exposes Prometheus metrics on `/metrics` endpoint:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'a2a-gateway'
    static_configs:
      - targets: ['localhost:9090']
```

### Grafana Dashboard

Import the provided Grafana dashboard for monitoring:

- Request rate and latency
- Service health status
- Load balancing distribution
- Error rates

### Logging

Configure structured logging:

```yaml
logging:
  level: "info"
  format: "json"
```

Use log aggregation tools like ELK stack or Loki for centralized logging.

## Security Considerations

### Authentication

- Use strong bearer tokens or JWT secrets
- Rotate credentials regularly
- Implement proper RBAC

### Network Security

- Use TLS/HTTPS in production
- Implement network policies in Kubernetes
- Use firewalls to restrict access

### Container Security

- Use non-root user in containers
- Scan images for vulnerabilities
- Use minimal base images

## Troubleshooting

### Common Issues

#### Gateway Won't Start

```bash
# Check configuration
a2a-gateway --config gateway.yaml --validate

# Check logs
journalctl -u a2a-gateway -f
```

#### Services Not Discovered

```bash
# Check service discovery logs
curl http://localhost:8080/services

# Verify agent endpoints
curl http://agent-url/.well-known/agent-card
```

#### High Memory Usage

- Check service count and connection limits
- Monitor metrics for memory leaks
- Adjust resource limits

### Health Checks

```bash
# Gateway health
curl http://localhost:8080/health

# Metrics
curl http://localhost:9090/metrics

# Service list
curl http://localhost:8080/services
```

### Performance Tuning

- Adjust `max_connections` based on load
- Tune health check intervals
- Configure appropriate timeouts
- Use connection pooling for backend services

## Support

For issues and questions:

- Check the logs first
- Review configuration
- Consult the API documentation
- Open an issue on GitHub
