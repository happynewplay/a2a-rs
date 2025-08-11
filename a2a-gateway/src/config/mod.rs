//! Configuration management for the A2A Gateway

pub mod manager;
pub mod watcher;

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use crate::{Result, GatewayError};

pub use manager::{ConfigManager, ConfigManagerBuilder, ConfigEvent};

/// Main gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Server configuration
    pub server: ServerConfig,
    
    /// Service discovery configuration
    pub discovery: DiscoveryConfig,
    
    /// Load balancing configuration
    pub load_balancing: LoadBalancingConfig,
    
    /// Authentication configuration
    pub auth: AuthConfig,
    
    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Bind address for HTTP server
    pub bind_address: String,
    
    /// Enable WebSocket support
    pub enable_websocket: bool,
    
    /// WebSocket bind address
    pub websocket_address: Option<String>,
    
    /// Request timeout
    pub request_timeout: Duration,
    
    /// Maximum concurrent connections
    pub max_connections: usize,
}

/// Service discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Discovery strategy
    pub strategy: DiscoveryStrategy,
    
    /// Health check interval
    pub health_check_interval: Duration,
    
    /// Health check timeout
    pub health_check_timeout: Duration,
    
    /// Static services (for static discovery)
    pub static_services: Vec<StaticServiceConfig>,
}

/// Discovery strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DiscoveryStrategy {
    /// Static configuration
    Static,
    
    /// DNS-based discovery
    Dns { domain: String },
    
    /// Consul-based discovery
    Consul { address: String },
    
    /// Kubernetes-based discovery
    Kubernetes { namespace: String },
}

/// Static service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticServiceConfig {
    /// Service name
    pub name: String,
    
    /// Service URL
    pub url: String,
    
    /// Service weight for load balancing
    pub weight: u32,
    
    /// Service tags
    pub tags: Vec<String>,
}

/// Load balancing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingConfig {
    /// Load balancing strategy
    pub strategy: LoadBalancingStrategy,
    
    /// Health check configuration
    pub health_check: HealthCheckConfig,
}

/// Load balancing strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    /// Round-robin
    RoundRobin,
    
    /// Weighted round-robin
    WeightedRoundRobin,
    
    /// Least connections
    LeastConnections,
    
    /// Random
    Random,
    
    /// IP hash
    IpHash,
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
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable authentication
    pub enabled: bool,
    
    /// Authentication strategies
    pub strategies: Vec<AuthStrategy>,
}

/// Authentication strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthStrategy {
    /// Bearer token authentication
    BearerToken { tokens: Vec<String> },
    
    /// API key authentication
    ApiKey { keys: Vec<String> },
    
    /// JWT authentication
    Jwt { secret: String },
    
    /// OAuth2 authentication
    OAuth2 { 
        client_id: String,
        client_secret: String,
        issuer_url: String,
    },
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable metrics
    pub metrics: MetricsConfig,
    
    /// Enable tracing
    pub tracing: TracingConfig,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,
    
    /// Metrics bind address
    pub bind_address: String,
    
    /// Metrics path
    pub path: String,
}

/// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable tracing
    pub enabled: bool,
    
    /// Jaeger endpoint
    pub jaeger_endpoint: Option<String>,
    
    /// Service name for tracing
    pub service_name: String,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,
    
    /// Log format (json or text)
    pub format: String,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                bind_address: "0.0.0.0:8080".to_string(),
                enable_websocket: true,
                websocket_address: Some("0.0.0.0:8081".to_string()),
                request_timeout: Duration::from_secs(30),
                max_connections: 1000,
            },
            discovery: DiscoveryConfig {
                strategy: DiscoveryStrategy::Static,
                health_check_interval: Duration::from_secs(30),
                health_check_timeout: Duration::from_secs(5),
                static_services: vec![],
            },
            load_balancing: LoadBalancingConfig {
                strategy: LoadBalancingStrategy::RoundRobin,
                health_check: HealthCheckConfig {
                    interval: Duration::from_secs(30),
                    timeout: Duration::from_secs(5),
                    failure_threshold: 3,
                    success_threshold: 2,
                },
            },
            auth: AuthConfig {
                enabled: false,
                strategies: vec![],
            },
            monitoring: MonitoringConfig {
                metrics: MetricsConfig {
                    enabled: true,
                    bind_address: "0.0.0.0:9090".to_string(),
                    path: "/metrics".to_string(),
                },
                tracing: TracingConfig {
                    enabled: true,
                    jaeger_endpoint: None,
                    service_name: "a2a-gateway".to_string(),
                },
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "text".to_string(),
            },
        }
    }
}

impl GatewayConfig {
    /// Load configuration from a file
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| GatewayError::config(format!("Failed to read config file: {}", e)))?;

        let mut config: Self = serde_yaml::from_str(&content)
            .map_err(|e| GatewayError::config(format!("Failed to parse config file: {}", e)))?;

        // Apply environment variable overrides
        config.apply_env_overrides()?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Save configuration to a file
    pub async fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yaml::to_string(self)
            .map_err(|e| GatewayError::config(format!("Failed to serialize config: {}", e)))?;

        tokio::fs::write(path, content).await
            .map_err(|e| GatewayError::config(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Apply environment variable overrides
    pub fn apply_env_overrides(&mut self) -> Result<()> {
        use std::env;

        // Server configuration overrides
        if let Ok(bind_address) = env::var("A2A_GATEWAY_BIND_ADDRESS") {
            self.server.bind_address = bind_address;
        }

        if let Ok(enable_websocket) = env::var("A2A_GATEWAY_ENABLE_WEBSOCKET") {
            self.server.enable_websocket = enable_websocket.parse()
                .map_err(|e| GatewayError::config(format!("Invalid ENABLE_WEBSOCKET value: {}", e)))?;
        }

        if let Ok(websocket_address) = env::var("A2A_GATEWAY_WEBSOCKET_ADDRESS") {
            self.server.websocket_address = Some(websocket_address);
        }

        if let Ok(max_connections) = env::var("A2A_GATEWAY_MAX_CONNECTIONS") {
            self.server.max_connections = max_connections.parse()
                .map_err(|e| GatewayError::config(format!("Invalid MAX_CONNECTIONS value: {}", e)))?;
        }

        // Authentication configuration overrides
        if let Ok(auth_enabled) = env::var("A2A_GATEWAY_AUTH_ENABLED") {
            self.auth.enabled = auth_enabled.parse()
                .map_err(|e| GatewayError::config(format!("Invalid AUTH_ENABLED value: {}", e)))?;
        }

        // Monitoring configuration overrides
        if let Ok(metrics_enabled) = env::var("A2A_GATEWAY_METRICS_ENABLED") {
            self.monitoring.metrics.enabled = metrics_enabled.parse()
                .map_err(|e| GatewayError::config(format!("Invalid METRICS_ENABLED value: {}", e)))?;
        }

        if let Ok(metrics_bind) = env::var("A2A_GATEWAY_METRICS_BIND_ADDRESS") {
            self.monitoring.metrics.bind_address = metrics_bind;
        }

        if let Ok(tracing_enabled) = env::var("A2A_GATEWAY_TRACING_ENABLED") {
            self.monitoring.tracing.enabled = tracing_enabled.parse()
                .map_err(|e| GatewayError::config(format!("Invalid TRACING_ENABLED value: {}", e)))?;
        }

        if let Ok(jaeger_endpoint) = env::var("A2A_GATEWAY_JAEGER_ENDPOINT") {
            self.monitoring.tracing.jaeger_endpoint = Some(jaeger_endpoint);
        }

        // Logging configuration overrides
        if let Ok(log_level) = env::var("A2A_GATEWAY_LOG_LEVEL") {
            self.logging.level = log_level;
        }

        if let Ok(log_format) = env::var("A2A_GATEWAY_LOG_FORMAT") {
            self.logging.format = log_format;
        }

        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate server configuration
        if self.server.bind_address.is_empty() {
            return Err(GatewayError::config("Server bind address cannot be empty"));
        }

        if self.server.max_connections == 0 {
            return Err(GatewayError::config("Max connections must be greater than 0"));
        }

        // Validate discovery configuration
        if self.discovery.static_services.is_empty() {
            match &self.discovery.strategy {
                DiscoveryStrategy::Static => {
                    return Err(GatewayError::config("Static discovery requires at least one service"));
                }
                _ => {} // Other strategies are OK with empty static services
            }
        }

        // Validate authentication configuration
        if self.auth.enabled && self.auth.strategies.is_empty() {
            return Err(GatewayError::config("Authentication enabled but no strategies configured"));
        }

        // Validate monitoring configuration
        if self.monitoring.metrics.enabled && self.monitoring.metrics.bind_address.is_empty() {
            return Err(GatewayError::config("Metrics enabled but no bind address configured"));
        }

        Ok(())
    }

    /// Merge with another configuration (for hot reload)
    pub fn merge(&mut self, other: &GatewayConfig) -> Result<()> {
        // Only merge certain fields that are safe to change at runtime

        // Update discovery configuration
        self.discovery.health_check_interval = other.discovery.health_check_interval;
        self.discovery.health_check_timeout = other.discovery.health_check_timeout;
        self.discovery.static_services = other.discovery.static_services.clone();

        // Update load balancing configuration
        self.load_balancing.strategy = other.load_balancing.strategy;
        self.load_balancing.health_check = other.load_balancing.health_check.clone();

        // Update authentication configuration (with caution)
        if !other.auth.strategies.is_empty() {
            self.auth.strategies = other.auth.strategies.clone();
        }

        // Update monitoring configuration
        self.monitoring.metrics.enabled = other.monitoring.metrics.enabled;
        self.monitoring.tracing.enabled = other.monitoring.tracing.enabled;

        // Update logging configuration
        self.logging.level = other.logging.level.clone();
        self.logging.format = other.logging.format.clone();

        // Validate the merged configuration
        self.validate()?;

        Ok(())
    }
}
