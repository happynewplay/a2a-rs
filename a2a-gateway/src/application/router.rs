//! Request router application service

use std::sync::Arc;
use tracing::{debug, warn};

use crate::{
    domain::{ServiceInfo, ServiceRegistry, RoutingRule, RoutingStrategy, RequestContext},
    port::{LoadBalancing, LoadBalancingStats},
    application::LoadBalancer,
    Result, GatewayError,
};

/// Request router
#[derive(Debug)]
pub struct RequestRouter {
    registry: ServiceRegistry,
    load_balancer: Arc<dyn LoadBalancing>,
    rules: Vec<RoutingRule>,
}

impl RequestRouter {
    /// Create a new request router
    pub fn new(registry: ServiceRegistry) -> Self {
        Self {
            registry,
            load_balancer: Arc::new(LoadBalancer::new()),
            rules: Vec::new(),
        }
    }
    
    /// Create with custom load balancer
    pub fn with_load_balancer(
        registry: ServiceRegistry,
        load_balancer: Arc<dyn LoadBalancing>,
    ) -> Self {
        Self {
            registry,
            load_balancer,
            rules: Vec::new(),
        }
    }
    
    /// Add a routing rule
    pub fn add_rule(&mut self, rule: RoutingRule) {
        self.rules.push(rule);
        // Sort rules by priority (higher priority first)
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }
    
    /// Remove a routing rule by name
    pub fn remove_rule(&mut self, name: &str) {
        self.rules.retain(|rule| rule.name != name);
    }
    
    /// Get all routing rules
    pub fn get_rules(&self) -> &[RoutingRule] {
        &self.rules
    }
    
    /// Route a request to an appropriate service
    pub async fn route(&self, context: &RequestContext) -> Result<Option<ServiceInfo>> {
        debug!("Routing request for path: {}", context.path);
        
        // Find the first matching rule
        let matching_rule = self.rules.iter().find(|rule| rule.matches(context));
        
        if let Some(rule) = matching_rule {
            debug!("Found matching rule: {}", rule.name);
            self.route_with_rule(rule, context).await
        } else {
            debug!("No matching rule found, using default routing");
            self.route_default(context).await
        }
    }
    
    /// Route using a specific rule
    async fn route_with_rule(
        &self,
        rule: &RoutingRule,
        context: &RequestContext,
    ) -> Result<Option<ServiceInfo>> {
        // Get candidate services based on routing strategy
        let candidates = self.get_candidate_services(&rule.strategy).await?;
        
        if candidates.is_empty() {
            warn!("No candidate services found for rule: {}", rule.name);
            return Ok(None);
        }
        
        debug!("Found {} candidate services for rule: {}", candidates.len(), rule.name);
        
        // Use load balancer to select from candidates
        self.load_balancer
            .select_service(&candidates, rule.load_balancing, context)
            .await
    }
    
    /// Default routing (route to any available service)
    async fn route_default(&self, context: &RequestContext) -> Result<Option<ServiceInfo>> {
        let services = self.registry.get_healthy().await?;
        
        if services.is_empty() {
            warn!("No healthy services available for default routing");
            return Ok(None);
        }
        
        // Use round-robin for default routing
        self.load_balancer
            .select_service(&services, crate::domain::LoadBalancingStrategy::RoundRobin, context)
            .await
    }
    
    /// Get candidate services based on routing strategy
    async fn get_candidate_services(&self, strategy: &RoutingStrategy) -> Result<Vec<ServiceInfo>> {
        match strategy {
            RoutingStrategy::ByName { name } => {
                let all_services = self.registry.get_healthy().await?;
                Ok(all_services
                    .into_iter()
                    .filter(|service| service.name == *name)
                    .collect())
            }
            RoutingStrategy::ByTags { tags } => {
                self.registry.get_by_tags(tags).await
            }
            RoutingStrategy::BySkill { skill } => {
                self.registry.get_by_skill(skill).await
            }
            RoutingStrategy::Custom { criteria } => {
                // For custom criteria, we'll filter services based on metadata
                let all_services = self.registry.get_healthy().await?;
                Ok(all_services
                    .into_iter()
                    .filter(|service| {
                        criteria.iter().all(|(key, value)| {
                            service.metadata.get(key).map_or(false, |v| v == value)
                        })
                    })
                    .collect())
            }
            RoutingStrategy::Any => {
                self.registry.get_healthy().await
            }
        }
    }
    
    /// Notify that a request is starting
    pub async fn request_start(&self, service_id: &str) -> Result<()> {
        self.load_balancer.request_start(service_id).await
    }
    
    /// Notify that a request has completed
    pub async fn request_complete(
        &self,
        service_id: &str,
        response_time: std::time::Duration,
    ) -> Result<()> {
        self.load_balancer.request_complete(service_id, response_time).await
    }
    
    /// Notify that a request has failed
    pub async fn request_failed(&self, service_id: &str, error: &str) -> Result<()> {
        self.load_balancer.request_failed(service_id, error).await
    }
    
    /// Get load balancing statistics
    pub async fn get_stats(&self) -> Result<LoadBalancingStats> {
        self.load_balancer.get_stats().await
    }
    
    /// Reset routing state
    pub async fn reset(&self) -> Result<()> {
        self.load_balancer.reset().await
    }
}

/// Route request context builder
pub struct RequestContextBuilder {
    context: RequestContext,
}

impl RequestContextBuilder {
    /// Create a new request context builder
    pub fn new(path: String) -> Self {
        Self {
            context: RequestContext::new(path),
        }
    }
    
    /// Add a header
    pub fn header(mut self, name: String, value: String) -> Self {
        self.context = self.context.with_header(name, value);
        self
    }
    
    /// Add a query parameter
    pub fn query_param(mut self, name: String, value: String) -> Self {
        self.context = self.context.with_query_param(name, value);
        self
    }
    
    /// Set client IP
    pub fn client_ip(mut self, ip: std::net::IpAddr) -> Self {
        self.context = self.context.with_client_ip(ip);
        self
    }
    
    /// Add custom data
    pub fn custom(mut self, key: String, value: String) -> Self {
        self.context = self.context.with_custom(key, value);
        self
    }
    
    /// Build the request context
    pub fn build(self) -> RequestContext {
        self.context
    }
}
