//! Port layer for the A2A Gateway
//!
//! Defines the interfaces that the gateway needs, independent of implementation.

pub mod service_discovery;
pub mod load_balancing;
pub mod authentication;
pub mod monitoring;

pub use service_discovery::{ServiceDiscovery, ServiceDiscoveryConfig, DiscoveryStrategy, StaticServiceConfig, ServiceEvent};
pub use load_balancing::{LoadBalancing, LoadBalancingStats, ServiceStats};
pub use authentication::{Authentication, AuthContext, AuthResult, AuthPrincipal, AuthConfig, AuthStrategy, PathPattern, ApiKeyLocation};
pub use monitoring::{Monitoring, MetricsSnapshot, HealthStatus, HealthState, HealthCheck as MonitoringHealthCheck, MonitoringConfig, PrometheusConfig, JaegerConfig};
