//! Load balancing port definitions

use async_trait::async_trait;
use std::time::Duration;

use crate::{
    domain::{ServiceInfo, LoadBalancingStrategy, RequestContext},
    Result,
};

/// Load balancing port
#[async_trait]
pub trait LoadBalancing: Send + Sync {
    /// Select a service from the available services using the specified strategy
    async fn select_service(
        &self,
        services: &[ServiceInfo],
        strategy: LoadBalancingStrategy,
        context: &RequestContext,
    ) -> Result<Option<ServiceInfo>>;
    
    /// Notify the load balancer that a request is starting
    async fn request_start(&self, service_id: &str) -> Result<()>;
    
    /// Notify the load balancer that a request has completed
    async fn request_complete(&self, service_id: &str, response_time: Duration) -> Result<()>;
    
    /// Notify the load balancer that a request has failed
    async fn request_failed(&self, service_id: &str, error: &str) -> Result<()>;
    
    /// Get load balancing statistics
    async fn get_stats(&self) -> Result<LoadBalancingStats>;
    
    /// Reset load balancing state
    async fn reset(&self) -> Result<()>;
}

/// Load balancing statistics
#[derive(Debug, Clone)]
pub struct LoadBalancingStats {
    /// Total number of requests
    pub total_requests: u64,
    
    /// Total number of failed requests
    pub failed_requests: u64,
    
    /// Average response time
    pub avg_response_time: Duration,
    
    /// Per-service statistics
    pub service_stats: std::collections::HashMap<String, ServiceStats>,
}

/// Per-service statistics
#[derive(Debug, Clone)]
pub struct ServiceStats {
    /// Service ID
    pub service_id: String,
    
    /// Number of requests sent to this service
    pub request_count: u64,
    
    /// Number of failed requests
    pub failed_count: u64,
    
    /// Current active connections
    pub active_connections: usize,
    
    /// Average response time
    pub avg_response_time: Duration,
    
    /// Last response time
    pub last_response_time: Option<Duration>,
    
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
}
