//! Service discovery adapter implementations

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};
use url::Url;

use crate::{
    domain::{ServiceInfo, ServiceRegistry, ServiceStatus, HealthCheck, HttpHealthCheck, HealthCheckConfig},
    port::{ServiceDiscovery, ServiceEvent, ServiceDiscoveryConfig, DiscoveryStrategy, StaticServiceConfig},
    Result, GatewayError,
};

/// Service discovery adapter
#[derive(Debug)]
pub struct ServiceDiscoveryAdapter {
    config: ServiceDiscoveryConfig,
    registry: ServiceRegistry,
    health_checker: Arc<dyn HealthCheck>,
    event_sender: mpsc::Sender<ServiceEvent>,
    event_receiver: Arc<RwLock<Option<mpsc::Receiver<ServiceEvent>>>>,
    running: Arc<RwLock<bool>>,
}

impl ServiceDiscoveryAdapter {
    /// Create a new service discovery adapter
    pub fn new(config: ServiceDiscoveryConfig, registry: ServiceRegistry) -> Self {
        let (event_sender, event_receiver) = mpsc::channel(100);
        
        Self {
            config,
            registry,
            health_checker: Arc::new(HttpHealthCheck::new()),
            event_sender,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Create with custom health checker
    pub fn with_health_checker(
        mut self,
        health_checker: Arc<dyn HealthCheck>,
    ) -> Self {
        self.health_checker = health_checker;
        self
    }
    
    /// Start the discovery process based on strategy
    async fn start_discovery(&self) -> Result<()> {
        match &self.config.strategy {
            DiscoveryStrategy::Static { services } => {
                self.start_static_discovery(services.clone()).await
            }
            DiscoveryStrategy::Dns { domain, port } => {
                self.start_dns_discovery(domain.clone(), *port).await
            }
            DiscoveryStrategy::Consul { address, service_name } => {
                self.start_consul_discovery(address.clone(), service_name.clone()).await
            }
            DiscoveryStrategy::Kubernetes { namespace, label_selector } => {
                self.start_k8s_discovery(namespace.clone(), label_selector.clone()).await
            }
            DiscoveryStrategy::File { path } => {
                self.start_file_discovery(path.clone()).await
            }
        }
    }
    
    /// Start static service discovery
    async fn start_static_discovery(&self, services: Vec<StaticServiceConfig>) -> Result<()> {
        info!("Starting static service discovery with {} services", services.len());
        
        for service_config in services {
            let url = Url::parse(&service_config.url)
                .map_err(|e| GatewayError::config(format!("Invalid service URL '{}': {}", service_config.url, e)))?;
            
            let mut service = ServiceInfo::new(service_config.name.clone(), url)
                .with_weight(service_config.weight)
                .with_tags(service_config.tags.clone())
                .with_metadata(service_config.metadata.clone());
            
            // Try to fetch agent card
            if let Ok(agent_card) = self.fetch_agent_card(&service.url.to_string()).await {
                service.update_agent_card(agent_card);
            }
            
            // Register the service
            self.registry.register(service.clone()).await?;
            
            // Send registration event
            if let Err(e) = self.event_sender.send(ServiceEvent::Registered(service)).await {
                warn!("Failed to send registration event: {}", e);
            }
        }
        
        Ok(())
    }
    
    /// Start DNS-based discovery (placeholder)
    async fn start_dns_discovery(&self, _domain: String, _port: u16) -> Result<()> {
        warn!("DNS-based discovery not yet implemented");
        Ok(())
    }
    
    /// Start Consul-based discovery (placeholder)
    async fn start_consul_discovery(&self, _address: String, _service_name: String) -> Result<()> {
        warn!("Consul-based discovery not yet implemented");
        Ok(())
    }
    
    /// Start Kubernetes-based discovery (placeholder)
    async fn start_k8s_discovery(&self, _namespace: String, _label_selector: String) -> Result<()> {
        warn!("Kubernetes-based discovery not yet implemented");
        Ok(())
    }
    
    /// Start file-based discovery (placeholder)
    async fn start_file_discovery(&self, _path: String) -> Result<()> {
        warn!("File-based discovery not yet implemented");
        Ok(())
    }
    
    /// Start health checking loop
    async fn start_health_checking(&self) -> Result<()> {
        let registry = self.registry.clone();
        let health_checker = self.health_checker.clone();
        let config = self.config.health_check.clone();
        let event_sender = self.event_sender.clone();
        let running = self.running.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(config.interval);
            
            while *running.read().await {
                interval.tick().await;
                
                let services = match registry.get_all().await {
                    Ok(services) => services,
                    Err(e) => {
                        error!("Failed to get services for health check: {}", e);
                        continue;
                    }
                };
                
                for service in services {
                    let result = health_checker.check(&service.url.to_string(), &config).await;
                    
                    // Update service status based on health check result
                    let new_status = match result.status {
                        crate::domain::HealthStatus::Healthy => ServiceStatus::Healthy,
                        crate::domain::HealthStatus::Unhealthy => ServiceStatus::Unhealthy,
                        _ => ServiceStatus::Unknown,
                    };
                    
                    if let Err(e) = registry.update_status(&service.id, new_status).await {
                        error!("Failed to update service status: {}", e);
                    }
                    
                    // Send health change event
                    if let Err(e) = event_sender.send(ServiceEvent::HealthChanged {
                        service_id: service.id.clone(),
                        result,
                    }).await {
                        warn!("Failed to send health change event: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Fetch agent card from service
    async fn fetch_agent_card(&self, service_url: &str) -> Result<a2a_rs::domain::AgentCard> {
        let client = reqwest::Client::new();
        let agent_card_url = if service_url.ends_with('/') {
            format!("{}/.well-known/agent-card", service_url.trim_end_matches('/'))
        } else {
            format!("{}/.well-known/agent-card", service_url)
        };
        
        debug!("Fetching agent card from: {}", agent_card_url);
        
        let response = client
            .get(&agent_card_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| GatewayError::service_discovery(format!("Failed to fetch agent card: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(GatewayError::service_discovery(format!(
                "Agent card request failed with status: {}",
                response.status()
            )));
        }
        
        let agent_card: a2a_rs::domain::AgentCard = response
            .json()
            .await
            .map_err(|e| GatewayError::service_discovery(format!("Failed to parse agent card: {}", e)))?;
        
        debug!("Successfully fetched agent card for service: {}", agent_card.name);
        Ok(agent_card)
    }
}

#[async_trait]
impl ServiceDiscovery for ServiceDiscoveryAdapter {
    async fn start(&self) -> Result<()> {
        info!("Starting service discovery");
        
        {
            let mut running = self.running.write().await;
            if *running {
                return Err(GatewayError::service_discovery("Service discovery is already running"));
            }
            *running = true;
        }
        
        // Start discovery based on strategy
        self.start_discovery().await?;
        
        // Start health checking
        self.start_health_checking().await?;
        
        info!("Service discovery started successfully");
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        info!("Stopping service discovery");
        
        {
            let mut running = self.running.write().await;
            *running = false;
        }
        
        info!("Service discovery stopped");
        Ok(())
    }
    
    async fn discover(&self) -> Result<Vec<ServiceInfo>> {
        self.registry.get_all().await
    }
    
    async fn register(&self, service: ServiceInfo) -> Result<()> {
        self.registry.register(service.clone()).await?;
        
        if let Err(e) = self.event_sender.send(ServiceEvent::Registered(service)).await {
            warn!("Failed to send registration event: {}", e);
        }
        
        Ok(())
    }
    
    async fn unregister(&self, service_id: &str) -> Result<()> {
        self.registry.unregister(service_id).await?;
        
        if let Err(e) = self.event_sender.send(ServiceEvent::Unregistered(service_id.to_string())).await {
            warn!("Failed to send unregistration event: {}", e);
        }
        
        Ok(())
    }
    
    async fn get_services(&self) -> Result<Vec<ServiceInfo>> {
        self.registry.get_all().await
    }
    
    async fn get_service(&self, service_id: &str) -> Result<Option<ServiceInfo>> {
        self.registry.get(service_id).await
    }
    
    async fn health_check(&self, service_id: &str) -> Result<crate::domain::HealthCheckResult> {
        let service = self.registry.get(service_id).await?
            .ok_or_else(|| GatewayError::service_not_found(service_id))?;
        
        let result = self.health_checker.check(&service.url.to_string(), &self.config.health_check).await;
        
        // Update service status
        let new_status = match result.status {
            crate::domain::HealthStatus::Healthy => ServiceStatus::Healthy,
            crate::domain::HealthStatus::Unhealthy => ServiceStatus::Unhealthy,
            _ => ServiceStatus::Unknown,
        };
        
        self.registry.update_status(service_id, new_status).await?;
        
        Ok(result)
    }
    
    async fn subscribe(&self) -> Result<tokio::sync::mpsc::Receiver<ServiceEvent>> {
        let mut receiver_guard = self.event_receiver.write().await;
        receiver_guard.take()
            .ok_or_else(|| GatewayError::service_discovery("Event receiver already taken"))
    }
}
