//! Main gateway application service

use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::{
    adapter::{
        HttpAdapter, WebSocketAdapter, ServiceDiscoveryAdapter, MonitoringAdapter, AuthAdapter,
    },
    application::{RequestRouter, LoadBalancer, ProtocolConverter},
    config::{ConfigManager, ConfigEvent, GatewayConfig},
    domain::{ServiceRegistry, HealthCheckConfig},
    port::{
        ServiceDiscovery, ServiceDiscoveryConfig, DiscoveryStrategy, StaticServiceConfig,
        Authentication, AuthConfig, Monitoring,
    },
    Result, GatewayError,
};

/// Main gateway application
#[derive(Debug)]
pub struct Gateway {
    config_manager: ConfigManager,
    service_registry: ServiceRegistry,
    router: Arc<RequestRouter>,
    discovery: Arc<dyn ServiceDiscovery>,
    auth: Arc<dyn Authentication>,
    monitoring: Arc<dyn Monitoring>,
    http_adapter: Option<HttpAdapter>,
    websocket_adapter: Option<WebSocketAdapter>,
    shutdown_sender: broadcast::Sender<()>,
}

impl Gateway {
    /// Start the gateway
    pub async fn start(mut self) -> Result<()> {
        info!("Starting A2A Gateway");
        
        // Start configuration hot reload
        self.config_manager.start_hot_reload().await?;
        
        // Start service discovery
        self.discovery.start().await?;
        
        // Start monitoring
        self.monitoring.start().await?;
        
        // Start HTTP adapter if configured
        if let Some(http_adapter) = &self.http_adapter {
            let adapter = http_adapter.clone();
            let shutdown_receiver = self.shutdown_sender.subscribe();
            
            tokio::spawn(async move {
                tokio::select! {
                    result = adapter.start() => {
                        if let Err(e) = result {
                            error!("HTTP adapter failed: {}", e);
                        }
                    }
                    _ = shutdown_receiver.recv() => {
                        info!("HTTP adapter shutting down");
                    }
                }
            });
        }
        
        // Start WebSocket adapter if configured
        if let Some(websocket_adapter) = &self.websocket_adapter {
            let _adapter = websocket_adapter.clone();
            let _shutdown_receiver = self.shutdown_sender.subscribe();
            
            // WebSocket adapter would be started here
            // For now, it's integrated with the HTTP adapter
        }
        
        // Start configuration change handler
        self.start_config_change_handler().await?;
        
        // Wait for shutdown signal
        self.wait_for_shutdown().await?;
        
        // Graceful shutdown
        self.shutdown().await?;
        
        Ok(())
    }
    
    /// Start configuration change handler
    async fn start_config_change_handler(&self) -> Result<()> {
        let mut config_receiver = self.config_manager.subscribe();
        let discovery = self.discovery.clone();
        
        tokio::spawn(async move {
            while let Ok(event) = config_receiver.recv().await {
                match event {
                    ConfigEvent::Reloaded(new_config) => {
                        info!("Configuration reloaded, applying changes");
                        
                        // Handle discovery configuration changes
                        if let Err(e) = Self::handle_discovery_config_change(&discovery, &new_config).await {
                            error!("Failed to apply discovery configuration changes: {}", e);
                        }
                        
                        // Other configuration changes would be handled here
                    }
                    ConfigEvent::ReloadFailed(error) => {
                        error!("Configuration reload failed: {}", error);
                    }
                    ConfigEvent::FileModified(path) => {
                        info!("Configuration file modified: {}", path.display());
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Handle discovery configuration changes
    async fn handle_discovery_config_change(
        discovery: &Arc<dyn ServiceDiscovery>,
        config: &GatewayConfig,
    ) -> Result<()> {
        // For static discovery, update the service list
        if let DiscoveryStrategy::Static = config.discovery.strategy {
            // Remove all existing services and re-register from config
            // This is a simplified approach; in production, you'd want to diff the changes
            for service_config in &config.discovery.static_services {
                let url = url::Url::parse(&service_config.url)
                    .map_err(|e| GatewayError::config(format!("Invalid service URL: {}", e)))?;
                
                let service = crate::domain::ServiceInfo::new(service_config.name.clone(), url)
                    .with_weight(service_config.weight)
                    .with_tags(service_config.tags.clone())
                    .with_metadata(service_config.metadata.clone());
                
                discovery.register(service).await?;
            }
        }
        
        Ok(())
    }
    
    /// Wait for shutdown signal
    async fn wait_for_shutdown(&self) -> Result<()> {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C, shutting down");
            }
            _ = self.wait_for_sigterm() => {
                info!("Received SIGTERM, shutting down");
            }
        }
        Ok(())
    }
    
    /// Wait for SIGTERM signal (Unix only)
    #[cfg(unix)]
    async fn wait_for_sigterm(&self) {
        use tokio::signal::unix::{signal, SignalKind};
        
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
        sigterm.recv().await;
    }
    
    /// Wait for SIGTERM signal (Windows - no-op)
    #[cfg(not(unix))]
    async fn wait_for_sigterm(&self) {
        // On Windows, we only handle Ctrl+C
        std::future::pending::<()>().await;
    }
    
    /// Graceful shutdown
    async fn shutdown(&self) -> Result<()> {
        info!("Starting graceful shutdown");
        
        // Send shutdown signal to all components
        if let Err(e) = self.shutdown_sender.send(()) {
            warn!("Failed to send shutdown signal: {}", e);
        }
        
        // Stop monitoring
        if let Err(e) = self.monitoring.stop().await {
            error!("Failed to stop monitoring: {}", e);
        }
        
        // Stop service discovery
        if let Err(e) = self.discovery.stop().await {
            error!("Failed to stop service discovery: {}", e);
        }
        
        // Give components time to shut down gracefully
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        info!("Gateway shutdown complete");
        Ok(())
    }
}

/// Gateway builder for easier construction
#[derive(Debug)]
pub struct GatewayBuilder {
    config: Option<GatewayConfig>,
    bind_address: Option<String>,
    metrics_enabled: bool,
    metrics_bind_address: Option<String>,
}

impl GatewayBuilder {
    /// Create a new gateway builder
    pub fn new() -> Self {
        Self {
            config: None,
            bind_address: None,
            metrics_enabled: false,
            metrics_bind_address: None,
        }
    }
    
    /// Set configuration
    pub fn with_config(mut self, config: GatewayConfig) -> Self {
        self.config = Some(config);
        self
    }
    
    /// Set bind address
    pub fn with_bind_address(mut self, address: String) -> Self {
        self.bind_address = Some(address);
        self
    }
    
    /// Enable metrics
    pub fn with_metrics(mut self, enabled: bool, bind_address: String) -> Self {
        self.metrics_enabled = enabled;
        self.metrics_bind_address = Some(bind_address);
        self
    }
    
    /// Build the gateway
    pub async fn build(self) -> Result<Gateway> {
        let mut config = self.config.unwrap_or_default();
        
        // Apply builder overrides
        if let Some(bind_address) = self.bind_address {
            config.server.bind_address = bind_address;
        }
        
        if self.metrics_enabled {
            config.monitoring.metrics.enabled = true;
            if let Some(metrics_bind) = self.metrics_bind_address {
                config.monitoring.metrics.bind_address = metrics_bind;
            }
        }
        
        // Create configuration manager
        let config_manager = ConfigManager::new(
            "gateway.yaml",
            true, // Enable hot reload
            Duration::from_secs(5),
        ).await?;
        
        // Create service registry
        let service_registry = ServiceRegistry::new();
        
        // Create load balancer
        let load_balancer = Arc::new(LoadBalancer::new());
        
        // Create request router
        let router = Arc::new(RequestRouter::with_load_balancer(
            service_registry.clone(),
            load_balancer,
        ));
        
        // Create protocol converter
        let protocol_converter = Arc::new(ProtocolConverter::new());
        
        // Create service discovery
        let discovery_config = ServiceDiscoveryConfig {
            strategy: config.discovery.strategy.clone(),
            health_check: HealthCheckConfig {
                interval: config.discovery.health_check_interval,
                timeout: config.discovery.health_check_timeout,
                failure_threshold: config.load_balancing.health_check.failure_threshold,
                success_threshold: config.load_balancing.health_check.success_threshold,
                path: "/.well-known/agent-card".to_string(),
                expected_status_codes: vec![200],
            },
            config: std::collections::HashMap::new(),
        };
        
        let discovery = Arc::new(ServiceDiscoveryAdapter::new(
            discovery_config,
            service_registry.clone(),
        ));
        
        // Create authentication
        let auth = Arc::new(AuthAdapter::new(config.auth.clone())?);
        
        // Create monitoring
        let monitoring_config = crate::port::MonitoringConfig {
            metrics_enabled: config.monitoring.metrics.enabled,
            metrics_interval: Duration::from_secs(10),
            metrics_retention: Duration::from_secs(3600),
            health_checks_enabled: true,
            health_check_interval: Duration::from_secs(30),
            prometheus: Some(crate::port::PrometheusConfig {
                enabled: config.monitoring.metrics.enabled,
                bind_address: config.monitoring.metrics.bind_address.clone(),
                path: config.monitoring.metrics.path.clone(),
                namespace: "a2a_gateway".to_string(),
            }),
            jaeger: config.monitoring.tracing.jaeger_endpoint.as_ref().map(|endpoint| {
                crate::port::JaegerConfig {
                    enabled: config.monitoring.tracing.enabled,
                    agent_endpoint: endpoint.clone(),
                    service_name: config.monitoring.tracing.service_name.clone(),
                    sampling_rate: 0.1,
                }
            }),
        };
        
        let monitoring = Arc::new(MonitoringAdapter::new(
            monitoring_config,
            service_registry.clone(),
        ));
        
        // Create HTTP adapter
        let http_adapter = Some(HttpAdapter::new(
            router.clone(),
            protocol_converter.clone(),
            auth.clone(),
            service_registry.clone(),
            config.server.bind_address.clone(),
        ));
        
        // Create WebSocket adapter if enabled
        let websocket_adapter = if config.server.enable_websocket {
            Some(WebSocketAdapter::new(
                router.clone(),
                protocol_converter.clone(),
                auth.clone(),
                service_registry.clone(),
                config.server.websocket_address
                    .clone()
                    .unwrap_or_else(|| "0.0.0.0:8081".to_string()),
            ))
        } else {
            None
        };
        
        // Create shutdown channel
        let (shutdown_sender, _) = broadcast::channel(1);
        
        Ok(Gateway {
            config_manager,
            service_registry,
            router,
            discovery,
            auth,
            monitoring,
            http_adapter,
            websocket_adapter,
            shutdown_sender,
        })
    }
}

impl Default for GatewayBuilder {
    fn default() -> Self {
        Self::new()
    }
}
