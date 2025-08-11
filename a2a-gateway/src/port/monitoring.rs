//! Monitoring port definitions

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::Result;

/// Monitoring port
#[async_trait]
pub trait Monitoring: Send + Sync {
    /// Record a metric
    async fn record_metric(&self, name: &str, value: f64, tags: Option<HashMap<String, String>>) -> Result<()>;
    
    /// Increment a counter
    async fn increment_counter(&self, name: &str, tags: Option<HashMap<String, String>>) -> Result<()>;
    
    /// Record a histogram value
    async fn record_histogram(&self, name: &str, value: f64, tags: Option<HashMap<String, String>>) -> Result<()>;
    
    /// Record a gauge value
    async fn record_gauge(&self, name: &str, value: f64, tags: Option<HashMap<String, String>>) -> Result<()>;
    
    /// Record request metrics
    async fn record_request(
        &self,
        method: &str,
        path: &str,
        status_code: u16,
        duration: Duration,
        service_id: Option<&str>,
    ) -> Result<()>;
    
    /// Record error
    async fn record_error(&self, error: &str, context: Option<HashMap<String, String>>) -> Result<()>;
    
    /// Get current metrics
    async fn get_metrics(&self) -> Result<MetricsSnapshot>;
    
    /// Get health status
    async fn get_health(&self) -> Result<HealthStatus>;
    
    /// Start monitoring
    async fn start(&self) -> Result<()>;
    
    /// Stop monitoring
    async fn stop(&self) -> Result<()>;
}

/// Metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Timestamp of the snapshot
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Counter metrics
    pub counters: HashMap<String, CounterMetric>,
    
    /// Histogram metrics
    pub histograms: HashMap<String, HistogramMetric>,
    
    /// Gauge metrics
    pub gauges: HashMap<String, GaugeMetric>,
    
    /// System metrics
    pub system: SystemMetrics,
}

/// Counter metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterMetric {
    pub name: String,
    pub value: u64,
    pub tags: HashMap<String, String>,
}

/// Histogram metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramMetric {
    pub name: String,
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub percentiles: HashMap<String, f64>, // e.g., "p50", "p95", "p99"
    pub tags: HashMap<String, String>,
}

/// Gauge metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaugeMetric {
    pub name: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
}

/// System metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// CPU usage percentage
    pub cpu_usage: f64,
    
    /// Memory usage in bytes
    pub memory_usage: u64,
    
    /// Memory usage percentage
    pub memory_usage_percent: f64,
    
    /// Number of active connections
    pub active_connections: u64,
    
    /// Uptime in seconds
    pub uptime: u64,
    
    /// Number of goroutines/tasks
    pub active_tasks: u64,
}

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall health status
    pub status: HealthState,
    
    /// Health checks
    pub checks: HashMap<String, HealthCheck>,
    
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Health state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthState {
    /// System is healthy
    Healthy,
    
    /// System is degraded but functional
    Degraded,
    
    /// System is unhealthy
    Unhealthy,
    
    /// Health status is unknown
    Unknown,
}

/// Individual health check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Check name
    pub name: String,
    
    /// Check status
    pub status: HealthState,
    
    /// Check message
    pub message: Option<String>,
    
    /// Check duration
    pub duration: Duration,
    
    /// Last check time
    pub last_check: chrono::DateTime<chrono::Utc>,
    
    /// Check metadata
    pub metadata: HashMap<String, String>,
}

impl HealthCheck {
    /// Create a healthy check
    pub fn healthy(name: String, duration: Duration) -> Self {
        Self {
            name,
            status: HealthState::Healthy,
            message: None,
            duration,
            last_check: chrono::Utc::now(),
            metadata: HashMap::new(),
        }
    }
    
    /// Create an unhealthy check
    pub fn unhealthy(name: String, message: String, duration: Duration) -> Self {
        Self {
            name,
            status: HealthState::Unhealthy,
            message: Some(message),
            duration,
            last_check: chrono::Utc::now(),
            metadata: HashMap::new(),
        }
    }
    
    /// Create a degraded check
    pub fn degraded(name: String, message: String, duration: Duration) -> Self {
        Self {
            name,
            status: HealthState::Degraded,
            message: Some(message),
            duration,
            last_check: chrono::Utc::now(),
            metadata: HashMap::new(),
        }
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable metrics collection
    pub metrics_enabled: bool,
    
    /// Metrics collection interval
    pub metrics_interval: Duration,
    
    /// Metrics retention period
    pub metrics_retention: Duration,
    
    /// Enable health checks
    pub health_checks_enabled: bool,
    
    /// Health check interval
    pub health_check_interval: Duration,
    
    /// Prometheus metrics configuration
    pub prometheus: Option<PrometheusConfig>,
    
    /// Jaeger tracing configuration
    pub jaeger: Option<JaegerConfig>,
}

/// Prometheus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusConfig {
    /// Enable Prometheus metrics
    pub enabled: bool,
    
    /// Bind address for metrics endpoint
    pub bind_address: String,
    
    /// Metrics endpoint path
    pub path: String,
    
    /// Metrics namespace
    pub namespace: String,
}

/// Jaeger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JaegerConfig {
    /// Enable Jaeger tracing
    pub enabled: bool,
    
    /// Jaeger agent endpoint
    pub agent_endpoint: String,
    
    /// Service name
    pub service_name: String,
    
    /// Sampling rate (0.0 to 1.0)
    pub sampling_rate: f64,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            metrics_interval: Duration::from_secs(10),
            metrics_retention: Duration::from_secs(3600), // 1 hour
            health_checks_enabled: true,
            health_check_interval: Duration::from_secs(30),
            prometheus: Some(PrometheusConfig {
                enabled: true,
                bind_address: "0.0.0.0:9090".to_string(),
                path: "/metrics".to_string(),
                namespace: "a2a_gateway".to_string(),
            }),
            jaeger: Some(JaegerConfig {
                enabled: false,
                agent_endpoint: "127.0.0.1:6831".to_string(),
                service_name: "a2a-gateway".to_string(),
                sampling_rate: 0.1,
            }),
        }
    }
}
