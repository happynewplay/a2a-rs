//! Service domain models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;

use crate::{Result, GatewayError};

/// Service information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Unique service ID
    pub id: String,
    
    /// Service name
    pub name: String,
    
    /// Service URL
    pub url: Url,
    
    /// Service weight for load balancing
    pub weight: u32,
    
    /// Service tags
    pub tags: Vec<String>,
    
    /// Service status
    pub status: ServiceStatus,
    
    /// Last health check time
    pub last_health_check: Option<DateTime<Utc>>,
    
    /// Service metadata
    pub metadata: HashMap<String, String>,
    
    /// A2A agent card information
    pub agent_card: Option<a2a_rs::domain::AgentCard>,
    
    /// Registration time
    pub registered_at: DateTime<Utc>,
    
    /// Last updated time
    pub updated_at: DateTime<Utc>,
}

/// Service status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceStatus {
    /// Service is healthy and available
    Healthy,
    
    /// Service is unhealthy but still registered
    Unhealthy,
    
    /// Service is temporarily unavailable
    Unavailable,
    
    /// Service is being drained (no new requests)
    Draining,
    
    /// Service is unknown (just registered, not checked yet)
    Unknown,
}

impl ServiceInfo {
    /// Create a new service info
    pub fn new(name: String, url: Url) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            url,
            weight: 100,
            tags: Vec::new(),
            status: ServiceStatus::Unknown,
            last_health_check: None,
            metadata: HashMap::new(),
            agent_card: None,
            registered_at: now,
            updated_at: now,
        }
    }
    
    /// Create a new service info with weight
    pub fn with_weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }
    
    /// Create a new service info with tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
    
    /// Create a new service info with metadata
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }
    
    /// Update service status
    pub fn update_status(&mut self, status: ServiceStatus) {
        self.status = status;
        self.last_health_check = Some(Utc::now());
        self.updated_at = Utc::now();
    }
    
    /// Update agent card
    pub fn update_agent_card(&mut self, agent_card: a2a_rs::domain::AgentCard) {
        self.agent_card = Some(agent_card);
        self.updated_at = Utc::now();
    }
    
    /// Check if service is available for requests
    pub fn is_available(&self) -> bool {
        matches!(self.status, ServiceStatus::Healthy)
    }
    
    /// Check if service matches tags
    pub fn matches_tags(&self, required_tags: &[String]) -> bool {
        if required_tags.is_empty() {
            return true;
        }
        
        required_tags.iter().all(|tag| self.tags.contains(tag))
    }
    
    /// Check if service has skill
    pub fn has_skill(&self, skill_name: &str) -> bool {
        if let Some(agent_card) = &self.agent_card {
            agent_card.skills.iter().any(|skill| skill.name == skill_name)
        } else {
            false
        }
    }
}

/// Service registry for managing registered services
#[derive(Debug, Clone)]
pub struct ServiceRegistry {
    services: Arc<RwLock<HashMap<String, ServiceInfo>>>,
}

impl ServiceRegistry {
    /// Create a new service registry
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a new service
    pub async fn register(&self, service: ServiceInfo) -> Result<()> {
        let mut services = self.services.write().await;
        services.insert(service.id.clone(), service);
        Ok(())
    }
    
    /// Unregister a service
    pub async fn unregister(&self, service_id: &str) -> Result<()> {
        let mut services = self.services.write().await;
        services.remove(service_id);
        Ok(())
    }
    
    /// Get a service by ID
    pub async fn get(&self, service_id: &str) -> Result<Option<ServiceInfo>> {
        let services = self.services.read().await;
        Ok(services.get(service_id).cloned())
    }
    
    /// Get all services
    pub async fn get_all(&self) -> Result<Vec<ServiceInfo>> {
        let services = self.services.read().await;
        Ok(services.values().cloned().collect())
    }
    
    /// Get healthy services
    pub async fn get_healthy(&self) -> Result<Vec<ServiceInfo>> {
        let services = self.services.read().await;
        Ok(services
            .values()
            .filter(|service| service.is_available())
            .cloned()
            .collect())
    }
    
    /// Get services by tags
    pub async fn get_by_tags(&self, tags: &[String]) -> Result<Vec<ServiceInfo>> {
        let services = self.services.read().await;
        Ok(services
            .values()
            .filter(|service| service.matches_tags(tags))
            .cloned()
            .collect())
    }
    
    /// Get services by skill
    pub async fn get_by_skill(&self, skill_name: &str) -> Result<Vec<ServiceInfo>> {
        let services = self.services.read().await;
        Ok(services
            .values()
            .filter(|service| service.has_skill(skill_name))
            .cloned()
            .collect())
    }
    
    /// Update service status
    pub async fn update_status(&self, service_id: &str, status: ServiceStatus) -> Result<()> {
        let mut services = self.services.write().await;
        if let Some(service) = services.get_mut(service_id) {
            service.update_status(status);
            Ok(())
        } else {
            Err(GatewayError::service_not_found(service_id))
        }
    }
    
    /// Update service agent card
    pub async fn update_agent_card(
        &self,
        service_id: &str,
        agent_card: a2a_rs::domain::AgentCard,
    ) -> Result<()> {
        let mut services = self.services.write().await;
        if let Some(service) = services.get_mut(service_id) {
            service.update_agent_card(agent_card);
            Ok(())
        } else {
            Err(GatewayError::service_not_found(service_id))
        }
    }
    
    /// Get service count
    pub async fn count(&self) -> usize {
        let services = self.services.read().await;
        services.len()
    }
    
    /// Get healthy service count
    pub async fn healthy_count(&self) -> usize {
        let services = self.services.read().await;
        services.values().filter(|s| s.is_available()).count()
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_service_info_creation() {
        let url = Url::parse("http://localhost:3000").unwrap();
        let service = ServiceInfo::new("test-service".to_string(), url.clone());

        assert_eq!(service.name, "test-service");
        assert_eq!(service.url, url);
        assert_eq!(service.weight, 100);
        assert_eq!(service.status, ServiceStatus::Unknown);
        assert!(service.tags.is_empty());
        assert!(service.metadata.is_empty());
    }

    #[test]
    fn test_service_info_builder() {
        let url = Url::parse("http://localhost:3000").unwrap();
        let mut metadata = HashMap::new();
        metadata.insert("env".to_string(), "test".to_string());

        let service = ServiceInfo::new("test-service".to_string(), url)
            .with_weight(200)
            .with_tags(vec!["tag1".to_string(), "tag2".to_string()])
            .with_metadata(metadata.clone());

        assert_eq!(service.weight, 200);
        assert_eq!(service.tags, vec!["tag1", "tag2"]);
        assert_eq!(service.metadata, metadata);
    }

    #[test]
    fn test_service_availability() {
        let url = Url::parse("http://localhost:3000").unwrap();
        let mut service = ServiceInfo::new("test-service".to_string(), url);

        // Initially unknown, not available
        assert!(!service.is_available());

        // Set to healthy
        service.update_status(ServiceStatus::Healthy);
        assert!(service.is_available());

        // Set to unhealthy
        service.update_status(ServiceStatus::Unhealthy);
        assert!(!service.is_available());
    }

    #[test]
    fn test_service_tag_matching() {
        let url = Url::parse("http://localhost:3000").unwrap();
        let service = ServiceInfo::new("test-service".to_string(), url)
            .with_tags(vec!["web".to_string(), "api".to_string()]);

        // Empty tags should match
        assert!(service.matches_tags(&[]));

        // Single matching tag
        assert!(service.matches_tags(&["web".to_string()]));

        // Multiple matching tags
        assert!(service.matches_tags(&["web".to_string(), "api".to_string()]));

        // Non-matching tag
        assert!(!service.matches_tags(&["database".to_string()]));

        // Partial match (requires all tags)
        assert!(!service.matches_tags(&["web".to_string(), "database".to_string()]));
    }

    #[tokio::test]
    async fn test_service_registry() {
        let registry = ServiceRegistry::new();

        // Initially empty
        assert_eq!(registry.count().await, 0);
        assert_eq!(registry.healthy_count().await, 0);

        // Register a service
        let url = Url::parse("http://localhost:3000").unwrap();
        let mut service = ServiceInfo::new("test-service".to_string(), url);
        service.update_status(ServiceStatus::Healthy);
        let service_id = service.id.clone();

        registry.register(service).await.unwrap();

        assert_eq!(registry.count().await, 1);
        assert_eq!(registry.healthy_count().await, 1);

        // Get service
        let retrieved = registry.get(&service_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-service");

        // Update status
        registry.update_status(&service_id, ServiceStatus::Unhealthy).await.unwrap();
        assert_eq!(registry.healthy_count().await, 0);

        // Unregister service
        registry.unregister(&service_id).await.unwrap();
        assert_eq!(registry.count().await, 0);
    }
}
