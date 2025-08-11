//! Load balancer application service

use async_trait::async_trait;
use rand::Rng;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

use crate::{
    domain::{ServiceInfo, LoadBalancingStrategy, RequestContext, LoadBalancerState},
    port::{LoadBalancing, LoadBalancingStats, ServiceStats},
    Result, GatewayError,
};

/// Load balancer implementation
#[derive(Debug)]
pub struct LoadBalancer {
    state: Arc<LoadBalancerState>,
    stats: Arc<parking_lot::RwLock<LoadBalancingStats>>,
}

impl LoadBalancer {
    /// Create a new load balancer
    pub fn new() -> Self {
        Self {
            state: Arc::new(LoadBalancerState::new()),
            stats: Arc::new(parking_lot::RwLock::new(LoadBalancingStats {
                total_requests: 0,
                failed_requests: 0,
                avg_response_time: Duration::from_millis(0),
                service_stats: HashMap::new(),
            })),
        }
    }
    
    /// Select service using round-robin strategy
    fn select_round_robin(&self, services: &[ServiceInfo]) -> Option<ServiceInfo> {
        if services.is_empty() {
            return None;
        }
        
        let index = self.state.next_round_robin(services.len());
        services.get(index).cloned()
    }
    
    /// Select service using weighted round-robin strategy
    fn select_weighted_round_robin(&self, services: &[ServiceInfo]) -> Option<ServiceInfo> {
        if services.is_empty() {
            return None;
        }
        
        // Calculate total weight
        let total_weight: u32 = services.iter().map(|s| s.weight).sum();
        if total_weight == 0 {
            return self.select_round_robin(services);
        }
        
        // Generate random number in range [0, total_weight)
        let mut rng = rand::thread_rng();
        let mut random_weight = rng.gen_range(0..total_weight);
        
        // Find the service corresponding to the random weight
        for service in services {
            if random_weight < service.weight {
                return Some(service.clone());
            }
            random_weight -= service.weight;
        }
        
        // Fallback to first service
        services.first().cloned()
    }
    
    /// Select service using least connections strategy
    fn select_least_connections(&self, services: &[ServiceInfo]) -> Option<ServiceInfo> {
        if services.is_empty() {
            return None;
        }
        
        let mut min_connections = usize::MAX;
        let mut selected_service = None;
        
        for service in services {
            let connections = self.state.get_connections(&service.id);
            if connections < min_connections {
                min_connections = connections;
                selected_service = Some(service.clone());
            }
        }
        
        selected_service
    }
    
    /// Select service using random strategy
    fn select_random(&self, services: &[ServiceInfo]) -> Option<ServiceInfo> {
        if services.is_empty() {
            return None;
        }
        
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..services.len());
        services.get(index).cloned()
    }
    
    /// Select service using IP hash strategy
    fn select_ip_hash(&self, services: &[ServiceInfo], context: &RequestContext) -> Option<ServiceInfo> {
        if services.is_empty() {
            return None;
        }
        
        // Use client IP for hashing, fallback to path if no IP
        let hash_input = context.client_ip
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| context.path.clone());
        
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hash_input.hash(&mut hasher);
        let hash = hasher.finish();
        
        let index = (hash as usize) % services.len();
        services.get(index).cloned()
    }
    
    /// Select service using least response time strategy
    fn select_least_response_time(&self, services: &[ServiceInfo]) -> Option<ServiceInfo> {
        if services.is_empty() {
            return None;
        }
        
        let mut min_response_time = Duration::from_secs(u64::MAX);
        let mut selected_service = None;
        
        for service in services {
            let response_time = self.state.get_response_time(&service.id)
                .unwrap_or(Duration::from_millis(100)); // Default to 100ms for new services
            
            if response_time < min_response_time {
                min_response_time = response_time;
                selected_service = Some(service.clone());
            }
        }
        
        selected_service
    }
    
    /// Update service statistics
    fn update_service_stats(&self, service_id: &str, response_time: Option<Duration>, failed: bool) {
        let mut stats = self.stats.write();
        
        // Update global stats
        stats.total_requests += 1;
        if failed {
            stats.failed_requests += 1;
        }
        
        // Update per-service stats
        let service_stats = stats.service_stats.entry(service_id.to_string()).or_insert_with(|| {
            ServiceStats {
                service_id: service_id.to_string(),
                request_count: 0,
                failed_count: 0,
                active_connections: 0,
                avg_response_time: Duration::from_millis(0),
                last_response_time: None,
                success_rate: 1.0,
            }
        });
        
        service_stats.request_count += 1;
        if failed {
            service_stats.failed_count += 1;
        }
        
        service_stats.active_connections = self.state.get_connections(service_id);
        
        if let Some(rt) = response_time {
            service_stats.last_response_time = Some(rt);
            
            // Update average response time (simple moving average)
            let total_time = service_stats.avg_response_time.as_millis() as u64 * (service_stats.request_count - 1)
                + rt.as_millis() as u64;
            service_stats.avg_response_time = Duration::from_millis(total_time / service_stats.request_count);
        }
        
        // Update success rate
        service_stats.success_rate = if service_stats.request_count > 0 {
            1.0 - (service_stats.failed_count as f64 / service_stats.request_count as f64)
        } else {
            1.0
        };
        
        // Update global average response time
        let total_response_time: u64 = stats.service_stats.values()
            .map(|s| s.avg_response_time.as_millis() as u64 * s.request_count)
            .sum();
        
        if stats.total_requests > 0 {
            stats.avg_response_time = Duration::from_millis(total_response_time / stats.total_requests);
        }
    }
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LoadBalancing for LoadBalancer {
    async fn select_service(
        &self,
        services: &[ServiceInfo],
        strategy: LoadBalancingStrategy,
        context: &RequestContext,
    ) -> Result<Option<ServiceInfo>> {
        // Filter only healthy services
        let healthy_services: Vec<ServiceInfo> = services
            .iter()
            .filter(|service| service.is_available())
            .cloned()
            .collect();
        
        if healthy_services.is_empty() {
            debug!("No healthy services available for load balancing");
            return Ok(None);
        }
        
        let selected = match strategy {
            LoadBalancingStrategy::RoundRobin => {
                self.select_round_robin(&healthy_services)
            }
            LoadBalancingStrategy::WeightedRoundRobin => {
                self.select_weighted_round_robin(&healthy_services)
            }
            LoadBalancingStrategy::LeastConnections => {
                self.select_least_connections(&healthy_services)
            }
            LoadBalancingStrategy::Random => {
                self.select_random(&healthy_services)
            }
            LoadBalancingStrategy::IpHash => {
                self.select_ip_hash(&healthy_services, context)
            }
            LoadBalancingStrategy::LeastResponseTime => {
                self.select_least_response_time(&healthy_services)
            }
        };
        
        if let Some(ref service) = selected {
            debug!("Selected service '{}' using strategy {:?}", service.name, strategy);
        }
        
        Ok(selected)
    }
    
    async fn request_start(&self, service_id: &str) -> Result<()> {
        self.state.increment_connections(service_id);
        debug!("Request started for service: {}", service_id);
        Ok(())
    }
    
    async fn request_complete(&self, service_id: &str, response_time: Duration) -> Result<()> {
        self.state.decrement_connections(service_id);
        self.state.update_response_time(service_id, response_time);
        self.update_service_stats(service_id, Some(response_time), false);
        
        debug!("Request completed for service: {} in {:?}", service_id, response_time);
        Ok(())
    }
    
    async fn request_failed(&self, service_id: &str, error: &str) -> Result<()> {
        self.state.decrement_connections(service_id);
        self.update_service_stats(service_id, None, true);
        
        warn!("Request failed for service: {} - {}", service_id, error);
        Ok(())
    }
    
    async fn get_stats(&self) -> Result<LoadBalancingStats> {
        let stats = self.stats.read();
        Ok(stats.clone())
    }
    
    async fn reset(&self) -> Result<()> {
        // Reset state
        self.state.round_robin_counter.store(0, std::sync::atomic::Ordering::Relaxed);
        self.state.connection_counts.clear();
        self.state.response_times.clear();
        
        // Reset stats
        let mut stats = self.stats.write();
        stats.total_requests = 0;
        stats.failed_requests = 0;
        stats.avg_response_time = Duration::from_millis(0);
        stats.service_stats.clear();
        
        debug!("Load balancer state reset");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ServiceInfo, ServiceStatus};
    use std::net::IpAddr;
    use url::Url;

    fn create_test_services() -> Vec<ServiceInfo> {
        vec![
            {
                let url = Url::parse("http://localhost:3001").unwrap();
                let mut service = ServiceInfo::new("service1".to_string(), url);
                service.update_status(ServiceStatus::Healthy);
                service
            },
            {
                let url = Url::parse("http://localhost:3002").unwrap();
                let mut service = ServiceInfo::new("service2".to_string(), url);
                service.update_status(ServiceStatus::Healthy);
                service
            },
            {
                let url = Url::parse("http://localhost:3003").unwrap();
                let mut service = ServiceInfo::new("service3".to_string(), url);
                service.update_status(ServiceStatus::Healthy);
                service
            },
        ]
    }

    #[tokio::test]
    async fn test_round_robin_selection() {
        let load_balancer = LoadBalancer::new();
        let services = create_test_services();
        let context = RequestContext::new("/test".to_string());

        // Test round-robin selection
        for i in 0..6 {
            let selected = load_balancer
                .select_service(&services, LoadBalancingStrategy::RoundRobin, &context)
                .await
                .unwrap()
                .unwrap();

            let expected_index = i % services.len();
            assert_eq!(selected.name, services[expected_index].name);
        }
    }

    #[tokio::test]
    async fn test_random_selection() {
        let load_balancer = LoadBalancer::new();
        let services = create_test_services();
        let context = RequestContext::new("/test".to_string());

        // Test random selection (just ensure it returns a valid service)
        for _ in 0..10 {
            let selected = load_balancer
                .select_service(&services, LoadBalancingStrategy::Random, &context)
                .await
                .unwrap()
                .unwrap();

            assert!(services.iter().any(|s| s.name == selected.name));
        }
    }

    #[tokio::test]
    async fn test_ip_hash_selection() {
        let load_balancer = LoadBalancer::new();
        let services = create_test_services();

        let context1 = RequestContext::new("/test".to_string())
            .with_client_ip(IpAddr::V4("192.168.1.1".parse().unwrap()));
        let context2 = RequestContext::new("/test".to_string())
            .with_client_ip(IpAddr::V4("192.168.1.2".parse().unwrap()));

        // Same IP should always get the same service
        let selected1a = load_balancer
            .select_service(&services, LoadBalancingStrategy::IpHash, &context1)
            .await
            .unwrap()
            .unwrap();
        let selected1b = load_balancer
            .select_service(&services, LoadBalancingStrategy::IpHash, &context1)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(selected1a.name, selected1b.name);

        // Different IPs might get different services
        let selected2 = load_balancer
            .select_service(&services, LoadBalancingStrategy::IpHash, &context2)
            .await
            .unwrap()
            .unwrap();

        assert!(services.iter().any(|s| s.name == selected2.name));
    }

    #[tokio::test]
    async fn test_no_healthy_services() {
        let load_balancer = LoadBalancer::new();
        let mut services = create_test_services();

        // Mark all services as unhealthy
        for service in &mut services {
            service.update_status(ServiceStatus::Unhealthy);
        }

        let context = RequestContext::new("/test".to_string());
        let selected = load_balancer
            .select_service(&services, LoadBalancingStrategy::RoundRobin, &context)
            .await
            .unwrap();

        assert!(selected.is_none());
    }

    #[tokio::test]
    async fn test_request_tracking() {
        let load_balancer = LoadBalancer::new();
        let service_id = "test-service";

        // Start request
        load_balancer.request_start(service_id).await.unwrap();

        // Complete request
        let duration = Duration::from_millis(100);
        load_balancer.request_complete(service_id, duration).await.unwrap();

        // Check stats
        let stats = load_balancer.get_stats().await.unwrap();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.failed_requests, 0);

        let service_stats = stats.service_stats.get(service_id).unwrap();
        assert_eq!(service_stats.request_count, 1);
        assert_eq!(service_stats.failed_count, 0);
        assert_eq!(service_stats.active_connections, 0);
    }

    #[tokio::test]
    async fn test_request_failure_tracking() {
        let load_balancer = LoadBalancer::new();
        let service_id = "test-service";

        // Start and fail request
        load_balancer.request_start(service_id).await.unwrap();
        load_balancer.request_failed(service_id, "Connection timeout").await.unwrap();

        // Check stats
        let stats = load_balancer.get_stats().await.unwrap();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.failed_requests, 1);

        let service_stats = stats.service_stats.get(service_id).unwrap();
        assert_eq!(service_stats.request_count, 1);
        assert_eq!(service_stats.failed_count, 1);
        assert_eq!(service_stats.success_rate, 0.0);
    }
}
