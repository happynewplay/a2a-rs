# A2A Gateway Project Summary

## 项目概述

A2A Gateway 是一个高性能的 Agent-to-Agent (A2A) 协议网关，提供服务发现、负载均衡、认证、监控等功能。该项目基于 Rust 开发，采用六边形架构设计，具有高可扩展性和可维护性。

## 核心功能

### 🔍 服务发现与注册
- 自动发现 A2A 代理服务
- 支持静态配置和动态发现
- 健康检查和服务状态监控
- 服务注册表管理

### ⚖️ 负载均衡与路由
- 多种负载均衡策略：轮询、权重、最少连接、随机、IP哈希
- 基于技能的智能路由
- 故障转移和熔断机制
- 灵活的路由规则配置

### 🔐 认证与安全
- 支持多种认证方式：Bearer Token、API Key、JWT、OAuth2
- 统一认证代理
- 权限控制和访问策略
- 安全审计

### 🔄 协议适配与转换
- HTTP/WebSocket 协议支持
- 消息格式标准化
- 流式处理支持
- 协议版本兼容

### 📊 监控与可观测性
- Prometheus 指标收集
- 分布式链路追踪
- 健康检查端点
- 结构化日志记录

### ⚙️ 配置管理
- 灵活的配置系统
- 支持热重载
- 环境变量覆盖
- 配置验证

## 技术架构

### 六边形架构
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

### 核心模块

#### Domain Layer (领域层)
- `ServiceInfo`: 服务信息模型
- `ServiceRegistry`: 服务注册表
- `RoutingRule`: 路由规则
- `HealthCheck`: 健康检查

#### Port Layer (端口层)
- `ServiceDiscovery`: 服务发现接口
- `LoadBalancing`: 负载均衡接口
- `Authentication`: 认证接口
- `Monitoring`: 监控接口

#### Application Layer (应用层)
- `Gateway`: 网关核心服务
- `RequestRouter`: 请求路由器
- `LoadBalancer`: 负载均衡器
- `ProtocolConverter`: 协议转换器

#### Adapter Layer (适配器层)
- `HttpAdapter`: HTTP 协议适配器
- `WebSocketAdapter`: WebSocket 协议适配器
- `ServiceDiscoveryAdapter`: 服务发现适配器
- `MonitoringAdapter`: 监控适配器

## 项目结构

```
a2a-gateway/
├── src/
│   ├── adapter/          # 适配器层
│   │   ├── http.rs
│   │   ├── websocket.rs
│   │   ├── discovery.rs
│   │   ├── monitoring.rs
│   │   └── auth.rs
│   ├── application/      # 应用层
│   │   ├── gateway.rs
│   │   ├── router.rs
│   │   ├── load_balancer.rs
│   │   └── protocol_converter.rs
│   ├── config/          # 配置管理
│   │   ├── mod.rs
│   │   └── watcher.rs
│   ├── domain/          # 领域层
│   │   ├── service.rs
│   │   ├── routing.rs
│   │   └── health.rs
│   ├── port/            # 端口层
│   │   ├── service_discovery.rs
│   │   ├── load_balancing.rs
│   │   ├── authentication.rs
│   │   └── monitoring.rs
│   ├── error.rs         # 错误类型
│   ├── lib.rs           # 库入口
│   └── main.rs          # 程序入口
├── tests/               # 测试
│   └── integration_tests.rs
├── benches/             # 性能测试
│   └── gateway_bench.rs
├── Cargo.toml           # 项目配置
├── README.md            # 项目说明
├── API.md               # API 文档
├── DEPLOYMENT.md        # 部署指南
└── gateway.example.yaml # 配置示例
```

## 主要依赖

### 核心依赖
- `tokio`: 异步运行时
- `axum`: Web 框架
- `serde`: 序列化/反序列化
- `tracing`: 日志和追踪
- `reqwest`: HTTP 客户端

### A2A 协议
- `a2a-rs`: A2A 协议核心库

### 监控和可观测性
- `metrics`: 指标收集
- `opentelemetry`: 分布式追踪
- `prometheus`: 指标导出

## 配置示例

```yaml
server:
  bind_address: "0.0.0.0:8080"
  enable_websocket: true
  max_connections: 1000

discovery:
  strategy:
    type: "Static"
  static_services:
    - name: "reimbursement-agent"
      url: "http://localhost:3001"
      weight: 100
      tags: ["finance"]

load_balancing:
  strategy: "RoundRobin"

auth:
  enabled: true
  strategies:
    - type: "BearerToken"
      tokens: ["secure-token"]

monitoring:
  metrics:
    enabled: true
    bind_address: "0.0.0.0:9090"
```

## 使用方法

### 构建和运行

```bash
# 构建
cargo build --release

# 运行
./target/release/a2a-gateway --config gateway.yaml

# 或使用 cargo
cargo run -- --config gateway.yaml
```

### Docker 部署

```bash
# 构建镜像
docker build -t a2a-gateway:latest .

# 运行容器
docker run -p 8080:8080 -p 9090:9090 a2a-gateway:latest
```

### Kubernetes 部署

```bash
# 应用配置
kubectl apply -f k8s/

# 检查状态
kubectl get pods -l app=a2a-gateway
```

## API 端点

### 管理端点
- `GET /health` - 健康检查
- `GET /services` - 服务列表
- `GET /metrics` - Prometheus 指标
- `POST /reload` - 重载配置

### A2A 协议端点
- `GET /.well-known/agent-card` - 代理卡片
- `POST /tasks/send` - 发送任务
- `POST /tasks/sendSubscribe` - 流式任务
- `GET /tasks/{id}` - 获取任务状态

## 测试

### 单元测试
```bash
cargo test
```

### 集成测试
```bash
cargo test --test integration_tests
```

### 性能测试
```bash
cargo bench
```

## 监控指标

### 核心指标
- `a2a_gateway_requests_total` - 请求总数
- `a2a_gateway_request_duration_seconds` - 请求延迟
- `a2a_gateway_services_healthy` - 健康服务数
- `a2a_gateway_load_balancer_selections` - 负载均衡选择

### 健康检查
- 服务发现状态
- 负载均衡器状态
- 认证服务状态
- 后端服务健康状态

## 扩展性

### 添加新的负载均衡策略
1. 在 `LoadBalancingStrategy` 枚举中添加新策略
2. 在 `LoadBalancer` 中实现选择逻辑
3. 更新配置和文档

### 添加新的认证方式
1. 实现 `Authentication` trait
2. 在 `AuthAdapter` 中注册新策略
3. 更新配置模式

### 添加新的服务发现方式
1. 实现 `ServiceDiscovery` trait
2. 在配置中添加新的发现策略
3. 更新适配器

## 性能特性

- **高并发**: 支持数千个并发连接
- **低延迟**: 微秒级路由决策
- **高可用**: 自动故障转移
- **可扩展**: 水平扩展支持

## 安全特性

- **认证**: 多种认证方式支持
- **授权**: 基于角色的访问控制
- **审计**: 完整的请求日志
- **加密**: TLS/HTTPS 支持

## 未来规划

- [ ] 支持更多服务发现后端（Consul、etcd）
- [ ] 实现更高级的路由策略
- [ ] 添加 gRPC 协议支持
- [ ] 实现配置 UI 界面
- [ ] 支持插件系统

## 贡献指南

1. Fork 项目
2. 创建功能分支
3. 提交更改
4. 创建 Pull Request

## 许可证

MIT OR Apache-2.0
