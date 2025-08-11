//! Authentication adapter implementations

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use std::collections::HashMap;
use tracing::{debug, warn};

use crate::{
    port::{
        Authentication, AuthContext, AuthResult, AuthPrincipal, AuthConfig, AuthStrategy,
        PathPattern, ApiKeyLocation,
    },
    Result, GatewayError,
};

/// Authentication adapter
#[derive(Debug)]
pub struct AuthAdapter {
    config: AuthConfig,
    strategies: Vec<Box<dyn AuthenticationStrategy>>,
}

impl AuthAdapter {
    /// Create a new authentication adapter
    pub fn new(config: AuthConfig) -> Result<Self> {
        let mut strategies: Vec<Box<dyn AuthenticationStrategy>> = Vec::new();
        
        for strategy_config in &config.strategies {
            let strategy = create_strategy(strategy_config.clone())?;
            strategies.push(strategy);
        }
        
        Ok(Self {
            config,
            strategies,
        })
    }
    
    /// Check if a path requires authentication
    fn path_requires_auth(&self, path: &str, method: &str) -> bool {
        // Check exempt paths first
        for exempt_pattern in &self.config.exempt_paths {
            if exempt_pattern.matches(path, method) {
                return false;
            }
        }
        
        // Check protected paths
        for protected_pattern in &self.config.protected_paths {
            if protected_pattern.matches(path, method) {
                return true;
            }
        }
        
        // Default behavior based on config
        self.config.enabled
    }
    
    /// Get required permissions for a path
    fn get_required_permissions(&self, path: &str, method: &str) -> Vec<String> {
        for pattern in &self.config.protected_paths {
            if pattern.matches(path, method) {
                return pattern.permissions.clone();
            }
        }
        Vec::new()
    }
}

#[async_trait]
impl Authentication for AuthAdapter {
    async fn authenticate(&self, context: &AuthContext) -> Result<AuthResult> {
        if !self.config.enabled {
            debug!("Authentication disabled, allowing request");
            return Ok(AuthResult::Success(AuthPrincipal::new(
                "anonymous".to_string(),
                "none".to_string(),
            )));
        }
        
        // Try each authentication strategy
        for strategy in &self.strategies {
            if strategy.supports_scheme(&context.scheme_type) {
                match strategy.authenticate(context).await? {
                    AuthResult::Success(mut principal) => {
                        // Add default permissions
                        for permission in &self.config.default_permissions {
                            principal = principal.with_permission(permission.clone());
                        }
                        
                        debug!("Authentication successful for principal: {}", principal.id);
                        return Ok(AuthResult::Success(principal));
                    }
                    AuthResult::Failed(reason) => {
                        debug!("Authentication failed with strategy {}: {}", 
                               strategy.name(), reason);
                        continue;
                    }
                    AuthResult::Required => {
                        continue;
                    }
                }
            }
        }
        
        warn!("No authentication strategy succeeded for scheme: {}", context.scheme_type);
        Ok(AuthResult::Failed("Authentication failed".to_string()))
    }
    
    async fn is_required(&self, path: &str, method: &str) -> Result<bool> {
        Ok(self.path_requires_auth(path, method))
    }
    
    async fn authorize(
        &self,
        principal: &AuthPrincipal,
        resource: &str,
        action: &str,
    ) -> Result<bool> {
        let required_permissions = self.get_required_permissions(resource, action);
        
        if required_permissions.is_empty() {
            // No specific permissions required
            return Ok(true);
        }
        
        // Check if principal has all required permissions
        for permission in &required_permissions {
            if !principal.has_permission(permission) {
                debug!("Principal {} lacks permission: {}", principal.id, permission);
                return Ok(false);
            }
        }
        
        debug!("Authorization successful for principal: {}", principal.id);
        Ok(true)
    }
    
    fn get_schemes(&self) -> Vec<String> {
        self.strategies.iter().map(|s| s.scheme()).collect()
    }
    
    async fn validate_credential(&self, scheme: &str, credential: &str) -> Result<bool> {
        for strategy in &self.strategies {
            if strategy.supports_scheme(scheme) {
                return strategy.validate_credential(credential).await;
            }
        }
        
        Ok(false)
    }
}

/// Authentication strategy trait
#[async_trait]
trait AuthenticationStrategy: Send + Sync {
    /// Strategy name
    fn name(&self) -> &str;
    
    /// Supported scheme
    fn scheme(&self) -> String;
    
    /// Check if this strategy supports the given scheme
    fn supports_scheme(&self, scheme: &str) -> bool;
    
    /// Authenticate using this strategy
    async fn authenticate(&self, context: &AuthContext) -> Result<AuthResult>;
    
    /// Validate a credential
    async fn validate_credential(&self, credential: &str) -> Result<bool>;
}

/// Bearer token authentication strategy
#[derive(Debug)]
struct BearerTokenStrategy {
    tokens: Vec<String>,
    permissions: Vec<String>,
}

#[async_trait]
impl AuthenticationStrategy for BearerTokenStrategy {
    fn name(&self) -> &str {
        "bearer_token"
    }
    
    fn scheme(&self) -> String {
        "bearer".to_string()
    }
    
    fn supports_scheme(&self, scheme: &str) -> bool {
        scheme.to_lowercase() == "bearer"
    }
    
    async fn authenticate(&self, context: &AuthContext) -> Result<AuthResult> {
        if self.tokens.contains(&context.credential) {
            let mut principal = AuthPrincipal::new(
                context.credential.clone(),
                "bearer".to_string(),
            );
            
            // Add strategy-specific permissions
            for permission in &self.permissions {
                principal = principal.with_permission(permission.clone());
            }
            
            Ok(AuthResult::Success(principal))
        } else {
            Ok(AuthResult::Failed("Invalid bearer token".to_string()))
        }
    }
    
    async fn validate_credential(&self, credential: &str) -> Result<bool> {
        Ok(self.tokens.contains(&credential.to_string()))
    }
}

/// API key authentication strategy
#[derive(Debug)]
struct ApiKeyStrategy {
    keys: Vec<String>,
    location: ApiKeyLocation,
    name: String,
    permissions: Vec<String>,
}

#[async_trait]
impl AuthenticationStrategy for ApiKeyStrategy {
    fn name(&self) -> &str {
        "api_key"
    }
    
    fn scheme(&self) -> String {
        "apikey".to_string()
    }
    
    fn supports_scheme(&self, scheme: &str) -> bool {
        scheme.to_lowercase() == "apikey"
    }
    
    async fn authenticate(&self, context: &AuthContext) -> Result<AuthResult> {
        if self.keys.contains(&context.credential) {
            let mut principal = AuthPrincipal::new(
                context.credential.clone(),
                "apikey".to_string(),
            );
            
            // Add location metadata
            principal = principal.with_attribute(
                "location".to_string(),
                format!("{:?}", self.location),
            );
            
            // Add strategy-specific permissions
            for permission in &self.permissions {
                principal = principal.with_permission(permission.clone());
            }
            
            Ok(AuthResult::Success(principal))
        } else {
            Ok(AuthResult::Failed("Invalid API key".to_string()))
        }
    }
    
    async fn validate_credential(&self, credential: &str) -> Result<bool> {
        Ok(self.keys.contains(&credential.to_string()))
    }
}

/// Basic authentication strategy
#[derive(Debug)]
struct BasicStrategy {
    users: HashMap<String, String>,
    permissions: HashMap<String, Vec<String>>,
}

#[async_trait]
impl AuthenticationStrategy for BasicStrategy {
    fn name(&self) -> &str {
        "basic"
    }
    
    fn scheme(&self) -> String {
        "basic".to_string()
    }
    
    fn supports_scheme(&self, scheme: &str) -> bool {
        scheme.to_lowercase() == "basic"
    }
    
    async fn authenticate(&self, context: &AuthContext) -> Result<AuthResult> {
        // Decode base64 credential
        let decoded = general_purpose::STANDARD
            .decode(&context.credential)
            .map_err(|_| GatewayError::authentication("Invalid base64 encoding"))?;
        
        let credential_str = String::from_utf8(decoded)
            .map_err(|_| GatewayError::authentication("Invalid UTF-8 in credential"))?;
        
        let parts: Vec<&str> = credential_str.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Ok(AuthResult::Failed("Invalid basic auth format".to_string()));
        }
        
        let username = parts[0];
        let password = parts[1];
        
        if let Some(stored_password) = self.users.get(username) {
            if stored_password == password {
                let mut principal = AuthPrincipal::new(
                    username.to_string(),
                    "basic".to_string(),
                );
                
                // Add user-specific permissions
                if let Some(user_permissions) = self.permissions.get(username) {
                    for permission in user_permissions {
                        principal = principal.with_permission(permission.clone());
                    }
                }
                
                Ok(AuthResult::Success(principal))
            } else {
                Ok(AuthResult::Failed("Invalid password".to_string()))
            }
        } else {
            Ok(AuthResult::Failed("User not found".to_string()))
        }
    }
    
    async fn validate_credential(&self, credential: &str) -> Result<bool> {
        // For basic auth, we need to decode and check username/password
        let decoded = general_purpose::STANDARD.decode(credential).unwrap_or_default();
        let credential_str = String::from_utf8(decoded).unwrap_or_default();
        let parts: Vec<&str> = credential_str.splitn(2, ':').collect();
        
        if parts.len() == 2 {
            let username = parts[0];
            let password = parts[1];
            
            if let Some(stored_password) = self.users.get(username) {
                return Ok(stored_password == password);
            }
        }
        
        Ok(false)
    }
}

/// Create an authentication strategy from configuration
fn create_strategy(config: AuthStrategy) -> Result<Box<dyn AuthenticationStrategy>> {
    match config {
        AuthStrategy::BearerToken { tokens, permissions } => {
            Ok(Box::new(BearerTokenStrategy {
                tokens,
                permissions: permissions.unwrap_or_default(),
            }))
        }
        AuthStrategy::ApiKey { keys, location, name, permissions } => {
            Ok(Box::new(ApiKeyStrategy {
                keys,
                location,
                name,
                permissions: permissions.unwrap_or_default(),
            }))
        }
        AuthStrategy::Basic { users, permissions } => {
            Ok(Box::new(BasicStrategy {
                users,
                permissions: permissions.unwrap_or_default(),
            }))
        }
        AuthStrategy::Jwt { .. } => {
            // JWT strategy would require additional dependencies
            Err(GatewayError::authentication("JWT authentication not yet implemented"))
        }
        AuthStrategy::OAuth2 { .. } => {
            // OAuth2 strategy would require additional dependencies
            Err(GatewayError::authentication("OAuth2 authentication not yet implemented"))
        }
    }
}
