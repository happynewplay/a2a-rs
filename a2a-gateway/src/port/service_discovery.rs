//! Service discovery port definitions

use async_trait::async_trait;
use std::collections::HashMap;

use crate::{
    domain::{ServiceInfo, HealthCheckResult},
    Result,
};

/// Service discovery events
#[derive(Debug, Clone)]
pub enum ServiceEvent {
    /// Service was registered
    Registered(ServiceInfo),
    
    /// Service was unregistered
    Unregistered(String), // service_id
    
    /// Service health status changed
    HealthChanged {
        service_id: String,
        result: HealthCheckResult,
    },
    
    /// Service metadata updated
    Updated(ServiceInfo),
}

/// Service discovery port
#[async_trait]
pub trait ServiceDiscovery: Send + Sync {
    /// Start service discovery
    async fn start(&self) -> Result<()>;
    
    /// Stop service discovery
    async fn stop(&self) -> Result<()>;
    
    /// Discover services
    async fn discover(&self) -> Result<Vec<ServiceInfo>>;
    
    /// Register a service manually
    async fn register(&self, service: ServiceInfo) -> Result<()>;
    
    /// Unregister a service
    async fn unregister(&self, service_id: &str) -> Result<()>;
    
    /// Get all discovered services
    async fn get_services(&self) -> Result<Vec<ServiceInfo>>;
    
    /// Get a specific service
    async fn get_service(&self, service_id: &str) -> Result<Option<ServiceInfo>>;
    
    /// Perform health check on a service
    async fn health_check(&self, service_id: &str) -> Result<HealthCheckResult>;
    
    /// Subscribe to service events
    async fn subscribe(&self) -> Result<tokio::sync::mpsc::Receiver<ServiceEvent>>;
}

/// Service discovery configuration
#[derive(Debug, Clone)]
pub struct ServiceDiscoveryConfig {
    /// Discovery strategy
    pub strategy: DiscoveryStrategy,
    
    /// Health check configuration
    pub health_check: crate::domain::HealthCheckConfig,
    
    /// Additional configuration
    pub config: HashMap<String, String>,
}

/// Discovery strategy
#[derive(Debug, Clone)]
pub enum DiscoveryStrategy {
    /// Static configuration
    Static {
        services: Vec<StaticServiceConfig>,
    },
    
    /// DNS-based discovery
    Dns {
        domain: String,
        port: u16,
    },
    
    /// Consul-based discovery
    Consul {
        address: String,
        service_name: String,
    },
    
    /// Kubernetes-based discovery
    Kubernetes {
        namespace: String,
        label_selector: String,
    },
    
    /// File-based discovery (watch a file for changes)
    File {
        path: String,
    },
}

/// Static service configuration
#[derive(Debug, Clone)]
pub struct StaticServiceConfig {
    /// Service name
    pub name: String,
    
    /// Service URL
    pub url: String,
    
    /// Service weight
    pub weight: u32,
    
    /// Service tags
    pub tags: Vec<String>,
    
    /// Service metadata
    pub metadata: HashMap<String, String>,
}
