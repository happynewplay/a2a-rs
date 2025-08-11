//! Integration tests for the A2A Gateway

use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use url::Url;

use a2a_gateway::{
    config::{GatewayConfig, ServerConfig, DiscoveryConfig, DiscoveryStrategy, StaticServiceConfig},
    domain::{ServiceInfo, ServiceRegistry, RequestContext},
    application::{LoadBalancer, RequestRouter},
    port::LoadBalancing,
    Result,
};

/// Test service registry operations
#[tokio::test]
async fn test_service_registry() -> Result<()> {
    let registry = ServiceRegistry::new();
    
    // Test empty registry
    assert_eq!(registry.count().await, 0);
    assert!(registry.get_all().await?.is_empty());
    
    // Register a service
    let service = ServiceInfo::new(
        "test-service".to_string(),
        Url::parse("http://localhost:3001").unwrap(),
    )
    .with_weight(100)
    .with_tags(vec!["test".to_string(), "demo".to_string()]);
    
    let service_id = service.id.clone();
    registry.register(service.clone()).await?;
    
    // Test registry after registration
    assert_eq!(registry.count().await, 1);
    assert_eq!(registry.get_all().await?.len(), 1);
    
    // Test getting specific service
    let retrieved = registry.get(&service_id).await?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().name, "test-service");
    
    // Test getting by tags
    let tagged_services = registry.get_by_tags(&["test".to_string()]).await?;
    assert_eq!(tagged_services.len(), 1);
    
    let no_match_services = registry.get_by_tags(&["nonexistent".to_string()]).await?;
    assert!(no_match_services.is_empty());
    
    // Test unregistration
    registry.unregister(&service_id).await?;
    assert_eq!(registry.count().await, 0);
    
    Ok(())
}

/// Test load balancer functionality
#[tokio::test]
async fn test_load_balancer() -> Result<()> {
    let load_balancer = LoadBalancer::new();
    
    // Create test services
    let services = vec![
        ServiceInfo::new(
            "service-1".to_string(),
            Url::parse("http://localhost:3001").unwrap(),
        ).with_weight(100),
        ServiceInfo::new(
            "service-2".to_string(),
            Url::parse("http://localhost:3002").unwrap(),
        ).with_weight(200),
        ServiceInfo::new(
            "service-3".to_string(),
            Url::parse("http://localhost:3003").unwrap(),
        ).with_weight(50),
    ];
    
    let context = RequestContext::new("/test".to_string(), "GET".to_string());
    
    // Test round-robin selection
    let selection = load_balancer
        .select_service(&services, a2a_gateway::domain::LoadBalancingStrategy::RoundRobin, &context)
        .await?;
    
    assert!(selection.is_some());
    let selection = selection.unwrap();
    assert!(services.iter().any(|s| s.id == selection.service.id));
    
    // Test weighted round-robin selection
    let selection = load_balancer
        .select_service(&services, a2a_gateway::domain::LoadBalancingStrategy::WeightedRoundRobin, &context)
        .await?;
    
    assert!(selection.is_some());
    
    // Test random selection
    let selection = load_balancer
        .select_service(&services, a2a_gateway::domain::LoadBalancingStrategy::Random, &context)
        .await?;
    
    assert!(selection.is_some());
    
    // Test with empty services
    let empty_services = vec![];
    let selection = load_balancer
        .select_service(&empty_services, a2a_gateway::domain::LoadBalancingStrategy::RoundRobin, &context)
        .await?;
    
    assert!(selection.is_none());
    
    Ok(())
}

/// Test request routing
#[tokio::test]
async fn test_request_router() -> Result<()> {
    let load_balancer = std::sync::Arc::new(LoadBalancer::new());
    let router = RequestRouter::new(load_balancer);
    
    // Add default rules
    router.add_default_rules().await?;
    
    // Test getting rules
    let rules = router.get_rules().await?;
    assert!(!rules.is_empty());
    
    // Create test services
    let services = vec![
        ServiceInfo::new(
            "agent-service".to_string(),
            Url::parse("http://localhost:3001").unwrap(),
        ),
    ];
    
    // Test agent card routing
    let context = RequestContext::new("/.well-known/agent-card".to_string(), "GET".to_string());
    let filtered_services = router.find_services(&context, &services).await?;
    assert_eq!(filtered_services.len(), 1);
    
    // Test task routing
    let context = RequestContext::new("/tasks/send".to_string(), "POST".to_string());
    let filtered_services = router.find_services(&context, &services).await?;
    assert_eq!(filtered_services.len(), 1);
    
    // Test service selection
    let selection = router.select_service(&context, &services).await?;
    assert!(selection.is_some());
    
    Ok(())
}

/// Test configuration loading and validation
#[tokio::test]
async fn test_configuration() -> Result<()> {
    // Test default configuration
    let config = GatewayConfig::default();
    config.validate()?;
    
    // Test configuration with static services
    let mut config = GatewayConfig::default();
    config.discovery.strategy = DiscoveryStrategy::Static;
    config.discovery.static_services = vec![
        StaticServiceConfig {
            name: "test-service".to_string(),
            url: "http://localhost:3001".to_string(),
            weight: 100,
            tags: vec!["test".to_string()],
            metadata: HashMap::new(),
        }
    ];
    
    config.validate()?;
    
    // Test invalid configuration (empty bind address)
    let mut invalid_config = GatewayConfig::default();
    invalid_config.server.bind_address = String::new();
    
    assert!(invalid_config.validate().is_err());
    
    Ok(())
}

/// Test request context creation
#[tokio::test]
async fn test_request_context() {
    let mut context = RequestContext::new("/test".to_string(), "GET".to_string())
        .with_header("content-type".to_string(), "application/json".to_string())
        .with_param("id".to_string(), "123".to_string())
        .with_client_ip("192.168.1.1".to_string())
        .with_metadata("custom".to_string(), "value".to_string());
    
    assert_eq!(context.path, "/test");
    assert_eq!(context.method, "GET");
    assert_eq!(context.headers.get("content-type"), Some(&"application/json".to_string()));
    assert_eq!(context.params.get("id"), Some(&"123".to_string()));
    assert_eq!(context.client_ip, Some("192.168.1.1".to_string()));
    assert_eq!(context.metadata.get("custom"), Some(&"value".to_string()));
}

/// Test service health status updates
#[tokio::test]
async fn test_service_health_updates() -> Result<()> {
    let registry = ServiceRegistry::new();
    
    // Register a service
    let service = ServiceInfo::new(
        "health-test-service".to_string(),
        Url::parse("http://localhost:3001").unwrap(),
    );
    
    let service_id = service.id.clone();
    registry.register(service).await?;
    
    // Test initial status (should be Unknown)
    let service = registry.get(&service_id).await?.unwrap();
    assert_eq!(service.status, a2a_gateway::domain::ServiceStatus::Unknown);
    
    // Update to healthy
    registry.update_status(&service_id, a2a_gateway::domain::ServiceStatus::Healthy).await?;
    let service = registry.get(&service_id).await?.unwrap();
    assert_eq!(service.status, a2a_gateway::domain::ServiceStatus::Healthy);
    assert!(service.is_available());
    
    // Update to unhealthy
    registry.update_status(&service_id, a2a_gateway::domain::ServiceStatus::Unhealthy).await?;
    let service = registry.get(&service_id).await?.unwrap();
    assert_eq!(service.status, a2a_gateway::domain::ServiceStatus::Unhealthy);
    assert!(!service.is_available());
    
    // Test healthy services filtering
    let healthy_services = registry.get_healthy().await?;
    assert!(healthy_services.is_empty());
    
    // Update back to healthy
    registry.update_status(&service_id, a2a_gateway::domain::ServiceStatus::Healthy).await?;
    let healthy_services = registry.get_healthy().await?;
    assert_eq!(healthy_services.len(), 1);
    
    Ok(())
}

/// Test load balancer connection tracking
#[tokio::test]
async fn test_load_balancer_connections() -> Result<()> {
    let load_balancer = LoadBalancer::new();
    
    let service_id = "test-service-123";
    
    // Test initial connections (should be 0)
    let connections = load_balancer.get_connections().await?;
    assert_eq!(connections.get(service_id), None);
    
    // Increment connections
    load_balancer.update_connections(service_id, 1).await?;
    let connections = load_balancer.get_connections().await?;
    assert_eq!(connections.get(service_id), Some(&1));
    
    // Increment again
    load_balancer.update_connections(service_id, 2).await?;
    let connections = load_balancer.get_connections().await?;
    assert_eq!(connections.get(service_id), Some(&3));
    
    // Decrement
    load_balancer.update_connections(service_id, -1).await?;
    let connections = load_balancer.get_connections().await?;
    assert_eq!(connections.get(service_id), Some(&2));
    
    // Test response time tracking
    load_balancer.update_response_time(service_id, Duration::from_millis(100)).await?;
    let response_times = load_balancer.get_response_times().await?;
    assert!(response_times.contains_key(service_id));
    
    // Reset stats
    load_balancer.reset_stats().await?;
    let connections = load_balancer.get_connections().await?;
    assert!(connections.is_empty());
    
    Ok(())
}
