//! Error types for the A2A Gateway

use thiserror::Error;

/// Result type alias for the gateway
pub type Result<T> = std::result::Result<T, GatewayError>;

/// Main error type for the A2A Gateway
#[derive(Error, Debug)]
pub enum GatewayError {
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Service discovery errors
    #[error("Service discovery error: {0}")]
    ServiceDiscovery(String),

    /// Load balancing errors
    #[error("Load balancing error: {0}")]
    LoadBalancing(String),

    /// Authentication errors
    #[error("Authentication error: {0}")]
    Authentication(String),

    /// Protocol conversion errors
    #[error("Protocol conversion error: {0}")]
    ProtocolConversion(String),

    /// Routing errors
    #[error("Routing error: {0}")]
    Routing(String),

    /// Network/IO errors
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),

    /// HTTP client errors
    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    /// WebSocket errors
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// A2A protocol errors
    #[error("A2A protocol error: {0}")]
    A2A(#[from] a2a_rs::domain::A2AError),

    /// Health check errors
    #[error("Health check error: {0}")]
    HealthCheck(String),

    /// Monitoring errors
    #[error("Monitoring error: {0}")]
    Monitoring(String),

    /// Service not found
    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    /// Service unavailable
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Timeout errors
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Internal errors
    #[error("Internal error: {0}")]
    Internal(String),
}

impl GatewayError {
    /// Create a new configuration error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        Self::Config(msg.into())
    }

    /// Create a new service discovery error
    pub fn service_discovery<S: Into<String>>(msg: S) -> Self {
        Self::ServiceDiscovery(msg.into())
    }

    /// Create a new load balancing error
    pub fn load_balancing<S: Into<String>>(msg: S) -> Self {
        Self::LoadBalancing(msg.into())
    }

    /// Create a new authentication error
    pub fn authentication<S: Into<String>>(msg: S) -> Self {
        Self::Authentication(msg.into())
    }

    /// Create a new routing error
    pub fn routing<S: Into<String>>(msg: S) -> Self {
        Self::Routing(msg.into())
    }

    /// Create a new WebSocket error
    pub fn websocket<S: Into<String>>(msg: S) -> Self {
        Self::WebSocket(msg.into())
    }

    /// Create a new health check error
    pub fn health_check<S: Into<String>>(msg: S) -> Self {
        Self::HealthCheck(msg.into())
    }

    /// Create a new service not found error
    pub fn service_not_found<S: Into<String>>(service: S) -> Self {
        Self::ServiceNotFound(service.into())
    }

    /// Create a new service unavailable error
    pub fn service_unavailable<S: Into<String>>(service: S) -> Self {
        Self::ServiceUnavailable(service.into())
    }

    /// Create a new timeout error
    pub fn timeout<S: Into<String>>(msg: S) -> Self {
        Self::Timeout(msg.into())
    }

    /// Create a new internal error
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::Internal(msg.into())
    }
}
