//! Health check domain models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{Result, GatewayError};

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Service is healthy
    Healthy,
    
    /// Service is unhealthy
    Unhealthy,
    
    /// Health check is in progress
    Checking,
    
    /// Health status is unknown
    Unknown,
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Service ID
    pub service_id: String,
    
    /// Health status
    pub status: HealthStatus,
    
    /// Response time
    pub response_time: Duration,
    
    /// Error message (if unhealthy)
    pub error: Option<String>,
    
    /// Check timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl HealthCheckResult {
    /// Create a healthy result
    pub fn healthy(service_id: String, response_time: Duration) -> Self {
        Self {
            service_id,
            status: HealthStatus::Healthy,
            response_time,
            error: None,
            timestamp: Utc::now(),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    /// Create an unhealthy result
    pub fn unhealthy(service_id: String, error: String) -> Self {
        Self {
            service_id,
            status: HealthStatus::Unhealthy,
            response_time: Duration::from_secs(0),
            error: Some(error),
            timestamp: Utc::now(),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    /// Create a checking result
    pub fn checking(service_id: String) -> Self {
        Self {
            service_id,
            status: HealthStatus::Checking,
            response_time: Duration::from_secs(0),
            error: None,
            timestamp: Utc::now(),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Health check interval
    pub interval: Duration,
    
    /// Health check timeout
    pub timeout: Duration,
    
    /// Number of consecutive failures before marking unhealthy
    pub failure_threshold: u32,
    
    /// Number of consecutive successes before marking healthy
    pub success_threshold: u32,
    
    /// Health check path (relative to service URL)
    pub path: String,
    
    /// Expected HTTP status codes for healthy response
    pub expected_status_codes: Vec<u16>,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            failure_threshold: 3,
            success_threshold: 2,
            path: "/.well-known/agent-card".to_string(),
            expected_status_codes: vec![200],
        }
    }
}

/// Health check trait
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// Perform health check on a service
    async fn check(&self, service_url: &str, config: &HealthCheckConfig) -> HealthCheckResult;
}

/// HTTP health check implementation
#[derive(Debug, Clone)]
pub struct HttpHealthCheck {
    client: reqwest::Client,
}

impl HttpHealthCheck {
    /// Create a new HTTP health check
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
    
    /// Create with custom client
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for HttpHealthCheck {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl HealthCheck for HttpHealthCheck {
    async fn check(&self, service_url: &str, config: &HealthCheckConfig) -> HealthCheckResult {
        let service_id = service_url.to_string(); // In real implementation, this would be the actual service ID
        let start_time = std::time::Instant::now();
        
        // Construct health check URL
        let health_url = if service_url.ends_with('/') {
            format!("{}{}", service_url.trim_end_matches('/'), config.path)
        } else {
            format!("{}{}", service_url, config.path)
        };
        
        // Perform the health check
        match self
            .client
            .get(&health_url)
            .timeout(config.timeout)
            .send()
            .await
        {
            Ok(response) => {
                let response_time = start_time.elapsed();
                let status_code = response.status().as_u16();
                
                if config.expected_status_codes.contains(&status_code) {
                    HealthCheckResult::healthy(service_id, response_time)
                        .with_metadata("status_code".to_string(), status_code.to_string())
                        .with_metadata("url".to_string(), health_url)
                } else {
                    HealthCheckResult::unhealthy(
                        service_id,
                        format!("Unexpected status code: {}", status_code),
                    )
                    .with_metadata("status_code".to_string(), status_code.to_string())
                    .with_metadata("url".to_string(), health_url)
                }
            }
            Err(e) => HealthCheckResult::unhealthy(
                service_id,
                format!("Health check failed: {}", e),
            )
            .with_metadata("url".to_string(), health_url),
        }
    }
}
