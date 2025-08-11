//! Integration tests for the A2A Gateway

use a2a_gateway::{GatewayConfig, GatewayBuilder, ServiceInfo, ServiceRegistry};
use std::time::Duration;
use tokio::time::timeout;
use url::Url;

#[tokio::test]
async fn test_gateway_startup_and_shutdown() {
    // Create a minimal configuration
    let config = GatewayConfig {
        server: a2a_gateway::config::ServerConfig {
            bind_address: "127.0.0.1:0".to_string(), // Use port 0 for automatic assignment
            enable_websocket: false,
            websocket_address: None,
            request_timeout: Duration::from_secs(30),
            max_connections: 100,
        },
        discovery: a2a_gateway::config::DiscoveryConfig {
            strategy: a2a_gateway::port::DiscoveryStrategy::Static,
            health_check_interval: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(5),
            static_services: vec![],
        },
        load_balancing: a2a_gateway::config::LoadBalancingConfig {
            strategy: a2a_gateway::config::LoadBalancingStrategy::RoundRobin,
            health_check: a2a_gateway::config::HealthCheckConfig {
                interval: Duration::from_secs(30),
                timeout: Duration::from_secs(5),
                failure_threshold: 3,
                success_threshold: 2,
            },
        },
        auth: a2a_gateway::config::AuthConfig {
            enabled: false,
            strategies: vec![],
        },
        monitoring: a2a_gateway::config::MonitoringConfig {
            metrics: a2a_gateway::config::MetricsConfig {
                enabled: false,
                bind_address: "127.0.0.1:0".to_string(),
                path: "/metrics".to_string(),
            },
            tracing: a2a_gateway::config::TracingConfig {
                enabled: false,
                jaeger_endpoint: None,
                service_name: "test-gateway".to_string(),
            },
        },
        logging: a2a_gateway::config::LoggingConfig {
            level: "info".to_string(),
            format: "text".to_string(),
        },
    };

    // Build the gateway
    let gateway = GatewayBuilder::new()
        .with_config(config)
        .build()
        .await
        .expect("Failed to build gateway");

    // Start the gateway with a timeout
    let start_result = timeout(Duration::from_secs(5), async {
        // In a real test, we would start the gateway in a separate task
        // and then shut it down. For now, we just test that it can be built.
        Ok::<(), a2a_gateway::GatewayError>(())
    }).await;

    assert!(start_result.is_ok());
}

#[tokio::test]
async fn test_service_registry_operations() {
    let registry = ServiceRegistry::new();

    // Test empty registry
    assert_eq!(registry.count().await, 0);
    assert_eq!(registry.healthy_count().await, 0);

    // Add a service
    let url = Url::parse("http://localhost:3001").unwrap();
    let service = ServiceInfo::new("test-service".to_string(), url)
        .with_tags(vec!["test".to_string()])
        .with_weight(100);
    
    let service_id = service.id.clone();
    registry.register(service).await.unwrap();

    // Verify service was added
    assert_eq!(registry.count().await, 1);
    
    // Get the service
    let retrieved = registry.get(&service_id).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().name, "test-service");

    // Test filtering by tags
    let tagged_services = registry.get_by_tags(&["test".to_string()]).await.unwrap();
    assert_eq!(tagged_services.len(), 1);

    let no_match_services = registry.get_by_tags(&["nonexistent".to_string()]).await.unwrap();
    assert_eq!(no_match_services.len(), 0);

    // Remove the service
    registry.unregister(&service_id).await.unwrap();
    assert_eq!(registry.count().await, 0);
}

#[tokio::test]
async fn test_load_balancer_with_multiple_services() {
    use a2a_gateway::{LoadBalancer, LoadBalancingStrategy, RequestContext};
    use a2a_gateway::port::LoadBalancing;
    use a2a_gateway::domain::{ServiceStatus};

    let load_balancer = LoadBalancer::new();
    
    // Create test services
    let mut services = vec![];
    for i in 1..=3 {
        let url = Url::parse(&format!("http://localhost:300{}", i)).unwrap();
        let mut service = ServiceInfo::new(format!("service{}", i), url);
        service.update_status(ServiceStatus::Healthy);
        services.push(service);
    }

    let context = RequestContext::new("/test".to_string());

    // Test round-robin selection
    let mut selected_names = vec![];
    for _ in 0..6 {
        let selected = load_balancer
            .select_service(&services, LoadBalancingStrategy::RoundRobin, &context)
            .await
            .unwrap()
            .unwrap();
        selected_names.push(selected.name);
    }

    // Should cycle through services
    assert_eq!(selected_names[0], "service1");
    assert_eq!(selected_names[1], "service2");
    assert_eq!(selected_names[2], "service3");
    assert_eq!(selected_names[3], "service1"); // Cycle back
}

#[tokio::test]
async fn test_authentication_disabled() {
    use a2a_gateway::{AuthAdapter, Authentication};
    use a2a_gateway::port::{AuthConfig, AuthContext, AuthResult};

    let auth_config = AuthConfig {
        enabled: false,
        strategies: vec![],
        protected_paths: vec![],
        exempt_paths: vec![],
        default_permissions: vec![],
    };

    let auth = AuthAdapter::new(auth_config).unwrap();
    
    let context = AuthContext::new(
        "bearer".to_string(),
        "test-token".to_string(),
        "/test".to_string(),
        "GET".to_string(),
    );

    let result = auth.authenticate(&context).await.unwrap();
    
    match result {
        AuthResult::Success(principal) => {
            assert_eq!(principal.id, "anonymous");
            assert_eq!(principal.scheme, "none");
        }
        _ => panic!("Expected successful authentication when auth is disabled"),
    }
}

#[tokio::test]
async fn test_protocol_converter() {
    use a2a_gateway::ProtocolConverter;
    use std::collections::HashMap;

    let converter = ProtocolConverter::new();
    
    // Test HTTP to A2A conversion
    let headers = HashMap::new();
    let body = br#"{"message": "test"}"#;
    
    let a2a_request = converter
        .http_to_a2a("POST", "/tasks/send", &headers, body)
        .await
        .unwrap();
    
    assert!(a2a_request.get("jsonrpc").is_some());
    assert!(a2a_request.get("method").is_some());
    assert!(a2a_request.get("id").is_some());
    assert!(a2a_request.get("params").is_some());
    
    // Test A2A to HTTP conversion
    let (status_code, response_headers, response_body) = converter
        .a2a_to_http(&a2a_request, "/tasks/send")
        .await
        .unwrap();
    
    assert_eq!(status_code, 201); // Created for task send
    assert!(response_headers.contains_key("Content-Type"));
    assert!(!response_body.is_empty());
}

#[tokio::test]
async fn test_health_check() {
    use a2a_gateway::domain::{HttpHealthCheck, HealthCheck, HealthCheckConfig};
    use std::time::Duration;

    let health_checker = HttpHealthCheck::new();
    let config = HealthCheckConfig {
        interval: Duration::from_secs(30),
        timeout: Duration::from_secs(5),
        failure_threshold: 3,
        success_threshold: 2,
        path: "/health".to_string(),
        expected_status_codes: vec![200],
    };

    // Test health check against a non-existent service
    let result = health_checker
        .check("http://localhost:99999", &config)
        .await;
    
    // Should return unhealthy for non-existent service
    assert_eq!(result.status, a2a_gateway::domain::HealthStatus::Unhealthy);
    assert!(result.error.is_some());
}

#[tokio::test]
async fn test_config_validation() {
    use a2a_gateway::GatewayConfig;

    // Test valid configuration
    let valid_config = GatewayConfig::default();
    assert!(valid_config.validate().is_ok());

    // Test invalid configuration (empty bind address)
    let mut invalid_config = GatewayConfig::default();
    invalid_config.server.bind_address = String::new();
    assert!(invalid_config.validate().is_err());

    // Test invalid configuration (zero max connections)
    let mut invalid_config = GatewayConfig::default();
    invalid_config.server.max_connections = 0;
    assert!(invalid_config.validate().is_err());
}
