//! Domain layer for the A2A Gateway
//!
//! Contains the core business logic and domain models.

pub mod service;
pub mod routing;
pub mod health;

pub use service::{ServiceInfo, ServiceRegistry, ServiceStatus};
pub use routing::{RoutingRule, RoutingStrategy, LoadBalancingStrategy, RequestContext, LoadBalancerState};
pub use health::{HealthStatus, HealthCheck, HealthCheckConfig, HttpHealthCheck};
