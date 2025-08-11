//! Authentication port definitions

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::Result;

/// Authentication context
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Authentication scheme type
    pub scheme_type: String,
    
    /// Credential (token, key, etc.)
    pub credential: String,
    
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    
    /// Request path
    pub path: String,
    
    /// Request method
    pub method: String,
    
    /// Client IP address
    pub client_ip: Option<std::net::IpAddr>,
}

impl AuthContext {
    /// Create a new auth context
    pub fn new(
        scheme_type: String,
        credential: String,
        path: String,
        method: String,
    ) -> Self {
        Self {
            scheme_type,
            credential,
            metadata: HashMap::new(),
            path,
            method,
            client_ip: None,
        }
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
    
    /// Set client IP
    pub fn with_client_ip(mut self, ip: std::net::IpAddr) -> Self {
        self.client_ip = Some(ip);
        self
    }
    
    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

/// Authentication principal (authenticated user/service)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthPrincipal {
    /// Principal ID
    pub id: String,
    
    /// Authentication scheme used
    pub scheme: String,
    
    /// Principal attributes
    pub attributes: HashMap<String, String>,
    
    /// Permissions/roles
    pub permissions: Vec<String>,
}

impl AuthPrincipal {
    /// Create a new auth principal
    pub fn new(id: String, scheme: String) -> Self {
        Self {
            id,
            scheme,
            attributes: HashMap::new(),
            permissions: Vec::new(),
        }
    }
    
    /// Add an attribute
    pub fn with_attribute(mut self, key: String, value: String) -> Self {
        self.attributes.insert(key, value);
        self
    }
    
    /// Add a permission
    pub fn with_permission(mut self, permission: String) -> Self {
        self.permissions.push(permission);
        self
    }
    
    /// Check if principal has a specific permission
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string())
    }
    
    /// Get attribute value
    pub fn get_attribute(&self, key: &str) -> Option<&String> {
        self.attributes.get(key)
    }
}

/// Authentication result
#[derive(Debug, Clone)]
pub enum AuthResult {
    /// Authentication successful
    Success(AuthPrincipal),
    
    /// Authentication failed
    Failed(String),
    
    /// Authentication required but not provided
    Required,
}

/// Authentication port
#[async_trait]
pub trait Authentication: Send + Sync {
    /// Authenticate a request
    async fn authenticate(&self, context: &AuthContext) -> Result<AuthResult>;
    
    /// Check if authentication is required for a path
    async fn is_required(&self, path: &str, method: &str) -> Result<bool>;
    
    /// Authorize a principal for a specific action
    async fn authorize(
        &self,
        principal: &AuthPrincipal,
        resource: &str,
        action: &str,
    ) -> Result<bool>;
    
    /// Get supported authentication schemes
    fn get_schemes(&self) -> Vec<String>;
    
    /// Validate a token/credential
    async fn validate_credential(&self, scheme: &str, credential: &str) -> Result<bool>;
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable authentication
    pub enabled: bool,
    
    /// Authentication strategies
    pub strategies: Vec<AuthStrategy>,
    
    /// Paths that require authentication
    pub protected_paths: Vec<PathPattern>,
    
    /// Paths that are exempt from authentication
    pub exempt_paths: Vec<PathPattern>,
    
    /// Default permissions for authenticated users
    pub default_permissions: Vec<String>,
}

/// Authentication strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthStrategy {
    /// Bearer token authentication
    BearerToken { 
        tokens: Vec<String>,
        permissions: Option<Vec<String>>,
    },
    
    /// API key authentication
    ApiKey { 
        keys: Vec<String>,
        location: ApiKeyLocation,
        name: String,
        permissions: Option<Vec<String>>,
    },
    
    /// JWT authentication
    Jwt { 
        secret: String,
        algorithm: String,
        issuer: Option<String>,
        audience: Option<String>,
    },
    
    /// OAuth2 authentication
    OAuth2 { 
        client_id: String,
        client_secret: String,
        issuer_url: String,
        scopes: Vec<String>,
    },
    
    /// Basic authentication
    Basic {
        users: HashMap<String, String>, // username -> password
        permissions: Option<HashMap<String, Vec<String>>>, // username -> permissions
    },
}

/// API key location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiKeyLocation {
    /// In HTTP header
    Header,
    
    /// In query parameter
    Query,
    
    /// In cookie
    Cookie,
}

/// Path pattern for authentication rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPattern {
    /// Path pattern (supports wildcards)
    pub pattern: String,
    
    /// HTTP methods (empty means all methods)
    pub methods: Vec<String>,
    
    /// Required permissions for this path
    pub permissions: Vec<String>,
}

impl PathPattern {
    /// Check if a path matches this pattern
    pub fn matches(&self, path: &str, method: &str) -> bool {
        // Check method
        if !self.methods.is_empty() && !self.methods.contains(&method.to_uppercase()) {
            return false;
        }
        
        // Check path pattern
        self.matches_path(path)
    }
    
    /// Check if a path matches the pattern
    fn matches_path(&self, path: &str) -> bool {
        // Simple wildcard matching
        if self.pattern.ends_with("*") {
            let prefix = &self.pattern[..self.pattern.len() - 1];
            path.starts_with(prefix)
        } else {
            path == self.pattern
        }
    }
}
