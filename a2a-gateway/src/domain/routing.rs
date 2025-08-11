//! Routing domain models

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::{domain::ServiceInfo, Result, GatewayError};

/// Routing strategy for selecting services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingStrategy {
    /// Route by service name
    ByName { name: String },
    
    /// Route by service tags
    ByTags { tags: Vec<String> },
    
    /// Route by agent skill
    BySkill { skill: String },
    
    /// Route by custom criteria
    Custom { criteria: HashMap<String, String> },
    
    /// Route to any available service
    Any,
}

/// Load balancing strategy
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    /// Round-robin selection
    RoundRobin,
    
    /// Weighted round-robin selection
    WeightedRoundRobin,
    
    /// Least connections selection
    LeastConnections,
    
    /// Random selection
    Random,
    
    /// IP hash-based selection
    IpHash,
    
    /// Least response time selection
    LeastResponseTime,
}

/// Routing rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    /// Rule name
    pub name: String,
    
    /// Rule priority (higher number = higher priority)
    pub priority: u32,
    
    /// Routing strategy
    pub strategy: RoutingStrategy,
    
    /// Load balancing strategy
    pub load_balancing: LoadBalancingStrategy,
    
    /// Rule conditions
    pub conditions: Vec<RoutingCondition>,
    
    /// Rule metadata
    pub metadata: HashMap<String, String>,
}

/// Routing condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingCondition {
    /// Path prefix condition
    PathPrefix { prefix: String },
    
    /// Header condition
    Header { name: String, value: String },
    
    /// Query parameter condition
    QueryParam { name: String, value: String },
    
    /// Client IP condition
    ClientIp { ip: IpAddr },
    
    /// Custom condition
    Custom { key: String, value: String },
}

impl RoutingRule {
    /// Create a new routing rule
    pub fn new(name: String, strategy: RoutingStrategy, load_balancing: LoadBalancingStrategy) -> Self {
        Self {
            name,
            priority: 0,
            strategy,
            load_balancing,
            conditions: Vec::new(),
            metadata: HashMap::new(),
        }
    }
    
    /// Set rule priority
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }
    
    /// Add a condition
    pub fn with_condition(mut self, condition: RoutingCondition) -> Self {
        self.conditions.push(condition);
        self
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
    
    /// Check if the rule matches the request context
    pub fn matches(&self, context: &RequestContext) -> bool {
        if self.conditions.is_empty() {
            return true;
        }
        
        self.conditions.iter().all(|condition| {
            match condition {
                RoutingCondition::PathPrefix { prefix } => {
                    context.path.starts_with(prefix)
                }
                RoutingCondition::Header { name, value } => {
                    context.headers.get(name).map_or(false, |v| v == value)
                }
                RoutingCondition::QueryParam { name, value } => {
                    context.query_params.get(name).map_or(false, |v| v == value)
                }
                RoutingCondition::ClientIp { ip } => {
                    context.client_ip.map_or(false, |client_ip| client_ip == *ip)
                }
                RoutingCondition::Custom { key, value } => {
                    context.custom.get(key).map_or(false, |v| v == value)
                }
            }
        })
    }
}

/// Request context for routing decisions
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Request path
    pub path: String,
    
    /// Request headers
    pub headers: HashMap<String, String>,
    
    /// Query parameters
    pub query_params: HashMap<String, String>,
    
    /// Client IP address
    pub client_ip: Option<IpAddr>,
    
    /// Custom context data
    pub custom: HashMap<String, String>,
}

impl RequestContext {
    /// Create a new request context
    pub fn new(path: String) -> Self {
        Self {
            path,
            headers: HashMap::new(),
            query_params: HashMap::new(),
            client_ip: None,
            custom: HashMap::new(),
        }
    }
    
    /// Add a header
    pub fn with_header(mut self, name: String, value: String) -> Self {
        self.headers.insert(name, value);
        self
    }
    
    /// Add a query parameter
    pub fn with_query_param(mut self, name: String, value: String) -> Self {
        self.query_params.insert(name, value);
        self
    }
    
    /// Set client IP
    pub fn with_client_ip(mut self, ip: IpAddr) -> Self {
        self.client_ip = Some(ip);
        self
    }
    
    /// Add custom data
    pub fn with_custom(mut self, key: String, value: String) -> Self {
        self.custom.insert(key, value);
        self
    }
}

/// Load balancer state for tracking connections and metrics
#[derive(Debug)]
pub struct LoadBalancerState {
    /// Round-robin counter
    pub round_robin_counter: AtomicUsize,
    
    /// Connection counts per service
    pub connection_counts: Arc<dashmap::DashMap<String, AtomicUsize>>,
    
    /// Response time tracking
    pub response_times: Arc<dashmap::DashMap<String, std::time::Duration>>,
}

impl LoadBalancerState {
    /// Create a new load balancer state
    pub fn new() -> Self {
        Self {
            round_robin_counter: AtomicUsize::new(0),
            connection_counts: Arc::new(dashmap::DashMap::new()),
            response_times: Arc::new(dashmap::DashMap::new()),
        }
    }
    
    /// Get next round-robin index
    pub fn next_round_robin(&self, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        self.round_robin_counter.fetch_add(1, Ordering::Relaxed) % count
    }
    
    /// Increment connection count for a service
    pub fn increment_connections(&self, service_id: &str) {
        self.connection_counts
            .entry(service_id.to_string())
            .or_insert_with(|| AtomicUsize::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }
    
    /// Decrement connection count for a service
    pub fn decrement_connections(&self, service_id: &str) {
        if let Some(count) = self.connection_counts.get(service_id) {
            count.fetch_sub(1, Ordering::Relaxed);
        }
    }
    
    /// Get connection count for a service
    pub fn get_connections(&self, service_id: &str) -> usize {
        self.connection_counts
            .get(service_id)
            .map_or(0, |count| count.load(Ordering::Relaxed))
    }
    
    /// Update response time for a service
    pub fn update_response_time(&self, service_id: &str, response_time: std::time::Duration) {
        self.response_times.insert(service_id.to_string(), response_time);
    }
    
    /// Get response time for a service
    pub fn get_response_time(&self, service_id: &str) -> Option<std::time::Duration> {
        self.response_times.get(service_id).map(|entry| *entry.value())
    }
}

impl Default for LoadBalancerState {
    fn default() -> Self {
        Self::new()
    }
}
