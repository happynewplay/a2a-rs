//! Monitoring adapter implementation

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::{
    port::{
        Monitoring, MetricsSnapshot, HealthStatus, HealthState, HealthCheck,
        CounterMetric, HistogramMetric, GaugeMetric, SystemMetrics, MonitoringConfig,
    },
    domain::ServiceRegistry,
    Result, GatewayError,
};

/// Monitoring adapter
#[derive(Debug)]
pub struct MonitoringAdapter {
    config: MonitoringConfig,
    metrics: Arc<RwLock<InternalMetrics>>,
    health_checks: Arc<RwLock<HashMap<String, HealthCheck>>>,
    service_registry: ServiceRegistry,
    start_time: Instant,
    running: Arc<RwLock<bool>>,
}

/// Internal metrics storage
#[derive(Debug, Default)]
struct InternalMetrics {
    counters: HashMap<String, (u64, HashMap<String, String>)>,
    histograms: HashMap<String, (Vec<f64>, HashMap<String, String>)>,
    gauges: HashMap<String, (f64, HashMap<String, String>)>,
}

impl MonitoringAdapter {
    /// Create a new monitoring adapter
    pub fn new(config: MonitoringConfig, service_registry: ServiceRegistry) -> Self {
        Self {
            config,
            metrics: Arc::new(RwLock::new(InternalMetrics::default())),
            health_checks: Arc::new(RwLock::new(HashMap::new())),
            service_registry,
            start_time: Instant::now(),
            running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Start background monitoring tasks
    async fn start_background_tasks(&self) -> Result<()> {
        if self.config.metrics_enabled {
            self.start_metrics_collection().await?;
        }
        
        if self.config.health_checks_enabled {
            self.start_health_checks().await?;
        }
        
        Ok(())
    }
    
    /// Start metrics collection task
    async fn start_metrics_collection(&self) -> Result<()> {
        let metrics = self.metrics.clone();
        let interval = self.config.metrics_interval;
        let running = self.running.clone();
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            while *running.read().await {
                interval_timer.tick().await;
                
                // Collect system metrics
                if let Err(e) = Self::collect_system_metrics(&metrics).await {
                    error!("Failed to collect system metrics: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    /// Start health checks task
    async fn start_health_checks(&self) -> Result<()> {
        let health_checks = self.health_checks.clone();
        let service_registry = self.service_registry.clone();
        let interval = self.config.health_check_interval;
        let running = self.running.clone();
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            while *running.read().await {
                interval_timer.tick().await;
                
                // Perform health checks
                if let Err(e) = Self::perform_health_checks(&health_checks, &service_registry).await {
                    error!("Failed to perform health checks: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    /// Collect system metrics
    async fn collect_system_metrics(metrics: &Arc<RwLock<InternalMetrics>>) -> Result<()> {
        let mut metrics_guard = metrics.write().await;
        
        // CPU usage (simplified - in real implementation, use a proper system metrics library)
        let cpu_usage = Self::get_cpu_usage().await;
        metrics_guard.gauges.insert(
            "system_cpu_usage".to_string(),
            (cpu_usage, HashMap::new()),
        );
        
        // Memory usage
        let (memory_usage, memory_percent) = Self::get_memory_usage().await;
        metrics_guard.gauges.insert(
            "system_memory_usage_bytes".to_string(),
            (memory_usage as f64, HashMap::new()),
        );
        metrics_guard.gauges.insert(
            "system_memory_usage_percent".to_string(),
            (memory_percent, HashMap::new()),
        );
        
        // Active tasks (simplified)
        let active_tasks = Self::get_active_tasks().await;
        metrics_guard.gauges.insert(
            "system_active_tasks".to_string(),
            (active_tasks as f64, HashMap::new()),
        );
        
        debug!("Collected system metrics");
        Ok(())
    }
    
    /// Perform health checks
    async fn perform_health_checks(
        health_checks: &Arc<RwLock<HashMap<String, HealthCheck>>>,
        service_registry: &ServiceRegistry,
    ) -> Result<()> {
        let start_time = Instant::now();
        
        // Check service registry health
        let service_count = service_registry.count().await;
        let healthy_count = service_registry.healthy_count().await;
        
        let registry_check = if service_count == 0 {
            HealthCheck::unhealthy(
                "service_registry".to_string(),
                "No services registered".to_string(),
                start_time.elapsed(),
            )
        } else if healthy_count == 0 {
            HealthCheck::degraded(
                "service_registry".to_string(),
                format!("No healthy services ({} total)", service_count),
                start_time.elapsed(),
            )
        } else if healthy_count < service_count {
            HealthCheck::degraded(
                "service_registry".to_string(),
                format!("{}/{} services healthy", healthy_count, service_count),
                start_time.elapsed(),
            )
        } else {
            HealthCheck::healthy("service_registry".to_string(), start_time.elapsed())
        };
        
        // Update health checks
        {
            let mut checks = health_checks.write().await;
            checks.insert("service_registry".to_string(), registry_check);
        }
        
        debug!("Performed health checks");
        Ok(())
    }
    
    /// Get CPU usage (simplified implementation)
    async fn get_cpu_usage() -> f64 {
        // In a real implementation, you would use a system metrics library
        // like `sysinfo` or read from /proc/stat on Linux
        0.0
    }
    
    /// Get memory usage (simplified implementation)
    async fn get_memory_usage() -> (u64, f64) {
        // In a real implementation, you would use a system metrics library
        (0, 0.0)
    }
    
    /// Get active tasks count (simplified implementation)
    async fn get_active_tasks() -> u64 {
        // In a real implementation, you would track actual task counts
        0
    }
    
    /// Calculate histogram statistics
    fn calculate_histogram_stats(values: &[f64]) -> (f64, f64, f64, HashMap<String, f64>) {
        if values.is_empty() {
            return (0.0, 0.0, 0.0, HashMap::new());
        }
        
        let mut sorted_values = values.to_vec();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let min = sorted_values[0];
        let max = sorted_values[sorted_values.len() - 1];
        let sum: f64 = sorted_values.iter().sum();
        let mean = sum / sorted_values.len() as f64;
        
        let mut percentiles = HashMap::new();
        percentiles.insert("p50".to_string(), Self::percentile(&sorted_values, 0.5));
        percentiles.insert("p90".to_string(), Self::percentile(&sorted_values, 0.9));
        percentiles.insert("p95".to_string(), Self::percentile(&sorted_values, 0.95));
        percentiles.insert("p99".to_string(), Self::percentile(&sorted_values, 0.99));
        
        (min, max, mean, percentiles)
    }
    
    /// Calculate percentile
    fn percentile(sorted_values: &[f64], p: f64) -> f64 {
        if sorted_values.is_empty() {
            return 0.0;
        }
        
        let index = (p * (sorted_values.len() - 1) as f64).round() as usize;
        sorted_values[index.min(sorted_values.len() - 1)]
    }
}

#[async_trait]
impl Monitoring for MonitoringAdapter {
    async fn record_metric(&self, name: &str, value: f64, tags: Option<HashMap<String, String>>) -> Result<()> {
        self.record_gauge(name, value, tags).await
    }
    
    async fn increment_counter(&self, name: &str, tags: Option<HashMap<String, String>>) -> Result<()> {
        let mut metrics = self.metrics.write().await;
        let entry = metrics.counters.entry(name.to_string()).or_insert((0, HashMap::new()));
        entry.0 += 1;
        if let Some(tags) = tags {
            entry.1 = tags;
        }
        Ok(())
    }
    
    async fn record_histogram(&self, name: &str, value: f64, tags: Option<HashMap<String, String>>) -> Result<()> {
        let mut metrics = self.metrics.write().await;
        let entry = metrics.histograms.entry(name.to_string()).or_insert((Vec::new(), HashMap::new()));
        entry.0.push(value);
        if let Some(tags) = tags {
            entry.1 = tags;
        }
        
        // Keep only recent values to prevent memory growth
        if entry.0.len() > 10000 {
            entry.0.drain(0..5000);
        }
        
        Ok(())
    }
    
    async fn record_gauge(&self, name: &str, value: f64, tags: Option<HashMap<String, String>>) -> Result<()> {
        let mut metrics = self.metrics.write().await;
        metrics.gauges.insert(name.to_string(), (value, tags.unwrap_or_default()));
        Ok(())
    }
    
    async fn record_request(
        &self,
        method: &str,
        path: &str,
        status_code: u16,
        duration: Duration,
        service_id: Option<&str>,
    ) -> Result<()> {
        let mut tags = HashMap::new();
        tags.insert("method".to_string(), method.to_string());
        tags.insert("path".to_string(), path.to_string());
        tags.insert("status_code".to_string(), status_code.to_string());
        if let Some(service_id) = service_id {
            tags.insert("service_id".to_string(), service_id.to_string());
        }
        
        // Record request count
        self.increment_counter("http_requests_total", Some(tags.clone())).await?;
        
        // Record request duration
        self.record_histogram("http_request_duration_seconds", duration.as_secs_f64(), Some(tags)).await?;
        
        Ok(())
    }
    
    async fn record_error(&self, error: &str, context: Option<HashMap<String, String>>) -> Result<()> {
        let mut tags = context.unwrap_or_default();
        tags.insert("error".to_string(), error.to_string());
        
        self.increment_counter("errors_total", Some(tags)).await
    }
    
    async fn get_metrics(&self) -> Result<MetricsSnapshot> {
        let metrics = self.metrics.read().await;
        
        let mut counters = HashMap::new();
        for (name, (value, tags)) in &metrics.counters {
            counters.insert(name.clone(), CounterMetric {
                name: name.clone(),
                value: *value,
                tags: tags.clone(),
            });
        }
        
        let mut histograms = HashMap::new();
        for (name, (values, tags)) in &metrics.histograms {
            let (min, max, mean, percentiles) = Self::calculate_histogram_stats(values);
            histograms.insert(name.clone(), HistogramMetric {
                name: name.clone(),
                count: values.len() as u64,
                sum: values.iter().sum(),
                min,
                max,
                mean,
                percentiles,
                tags: tags.clone(),
            });
        }
        
        let mut gauges = HashMap::new();
        for (name, (value, tags)) in &metrics.gauges {
            gauges.insert(name.clone(), GaugeMetric {
                name: name.clone(),
                value: *value,
                tags: tags.clone(),
            });
        }
        
        let system = SystemMetrics {
            cpu_usage: gauges.get("system_cpu_usage").map(|g| g.value).unwrap_or(0.0),
            memory_usage: gauges.get("system_memory_usage_bytes").map(|g| g.value as u64).unwrap_or(0),
            memory_usage_percent: gauges.get("system_memory_usage_percent").map(|g| g.value).unwrap_or(0.0),
            active_connections: gauges.get("active_connections").map(|g| g.value as u64).unwrap_or(0),
            uptime: self.start_time.elapsed().as_secs(),
            active_tasks: gauges.get("system_active_tasks").map(|g| g.value as u64).unwrap_or(0),
        };
        
        Ok(MetricsSnapshot {
            timestamp: chrono::Utc::now(),
            counters,
            histograms,
            gauges,
            system,
        })
    }
    
    async fn get_health(&self) -> Result<HealthStatus> {
        let checks = self.health_checks.read().await;
        
        // Determine overall health status
        let overall_status = if checks.is_empty() {
            HealthState::Unknown
        } else if checks.values().all(|check| check.status == HealthState::Healthy) {
            HealthState::Healthy
        } else if checks.values().any(|check| check.status == HealthState::Unhealthy) {
            HealthState::Unhealthy
        } else {
            HealthState::Degraded
        };
        
        let mut metadata = HashMap::new();
        metadata.insert("uptime".to_string(), self.start_time.elapsed().as_secs().to_string());
        metadata.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
        
        Ok(HealthStatus {
            status: overall_status,
            checks: checks.clone(),
            timestamp: chrono::Utc::now(),
            metadata,
        })
    }
    
    async fn start(&self) -> Result<()> {
        info!("Starting monitoring adapter");
        
        {
            let mut running = self.running.write().await;
            if *running {
                return Err(GatewayError::internal("Monitoring is already running"));
            }
            *running = true;
        }
        
        self.start_background_tasks().await?;
        
        info!("Monitoring adapter started successfully");
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        info!("Stopping monitoring adapter");
        
        {
            let mut running = self.running.write().await;
            *running = false;
        }
        
        info!("Monitoring adapter stopped");
        Ok(())
    }
}
