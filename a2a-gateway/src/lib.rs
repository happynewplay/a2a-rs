//! # A2A Gateway
//!
//! A high-performance gateway for the Agent-to-Agent (A2A) protocol that provides:
//! - Service discovery and registration
//! - Load balancing and intelligent routing
//! - Protocol adaptation (HTTP/WebSocket)
//! - Authentication and security
//! - Monitoring and observability
//!
//! ## Architecture
//!
//! The gateway follows a hexagonal architecture with clear separation of concerns:
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │                Gateway Core                 │
//! ├─────────────┬─────────────┬─────────────────┤
//! │   Adapter   │ Application │     Domain      │
//! │    Layer    │    Layer    │     Layer       │
//! │             │             │                 │
//! │ HTTP/WS     │ Routing     │ Service         │
//! │ Auth        │ Load Bal.   │ Registry        │
//! │ Discovery   │ Protocol    │ Health          │
//! │ Monitoring  │ Conversion  │ Monitor         │
//! └─────────────┴─────────────┴─────────────────┘
//! ```
//!
//! ## Features
//!
//! - **Service Discovery**: Automatic discovery and registration of A2A agents
//! - **Load Balancing**: Multiple strategies (round-robin, weighted, least-connections)
//! - **Protocol Support**: HTTP and WebSocket with seamless conversion
//! - **Authentication**: Integrated auth proxy with multiple schemes
//! - **Monitoring**: Comprehensive metrics, tracing, and health checks
//! - **Configuration**: Flexible configuration with hot-reload support

pub mod adapter;
pub mod application;
pub mod config;
pub mod domain;
pub mod error;
pub mod port;

// Re-export key types for convenience
pub use config::{GatewayConfig, ConfigManager, ConfigManagerBuilder, ConfigEvent};
pub use domain::{ServiceInfo, ServiceRegistry, RoutingRule, LoadBalancingStrategy, RoutingStrategy};
pub use error::{GatewayError, Result};

// Re-export application services
pub use application::{
    Gateway, GatewayBuilder, LoadBalancer, ProtocolConverter, RequestRouter,
};

// Re-export adapters
pub use adapter::{
    HttpAdapter, WebSocketAdapter, ServiceDiscoveryAdapter, MonitoringAdapter, AuthAdapter,
};

// Re-export ports
pub use port::{
    ServiceDiscovery, LoadBalancing, Authentication, Monitoring,
};
