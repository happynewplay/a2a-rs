//! Protocol converter for A2A messages

use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, warn};

use crate::{Result, GatewayError};

/// Protocol converter for handling different message formats and versions
#[derive(Debug, Clone)]
pub struct ProtocolConverter {
    /// Supported protocol versions
    supported_versions: Vec<String>,
    
    /// Default protocol version
    default_version: String,
}

impl ProtocolConverter {
    /// Create a new protocol converter
    pub fn new() -> Self {
        Self {
            supported_versions: vec!["1.0".to_string(), "2.0".to_string()],
            default_version: "2.0".to_string(),
        }
    }
    
    /// Create with custom supported versions
    pub fn with_versions(supported_versions: Vec<String>, default_version: String) -> Self {
        Self {
            supported_versions,
            default_version,
        }
    }
    
    /// Convert HTTP request to A2A format
    pub async fn http_to_a2a(
        &self,
        method: &str,
        path: &str,
        headers: &HashMap<String, String>,
        body: &[u8],
    ) -> Result<Value> {
        debug!("Converting HTTP request to A2A: {} {}", method, path);
        
        // Parse the request body as JSON
        let request_data: Value = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(body)
                .map_err(|e| GatewayError::protocol_conversion(format!("Invalid JSON body: {}", e)))?
        };
        
        // Determine the A2A operation based on path and method
        let a2a_request = match (method, path) {
            ("POST", path) if path.contains("/tasks/send") => {
                self.convert_send_task_request(request_data, headers).await?
            }
            ("POST", path) if path.contains("/tasks/sendSubscribe") => {
                self.convert_send_task_streaming_request(request_data, headers).await?
            }
            ("GET", path) if path.contains("/tasks/") => {
                self.convert_get_task_request(path, headers).await?
            }
            ("DELETE", path) if path.contains("/tasks/") => {
                self.convert_cancel_task_request(path, headers).await?
            }
            ("GET", path) if path.contains("/.well-known/agent-card") => {
                self.convert_agent_card_request(headers).await?
            }
            _ => {
                return Err(GatewayError::protocol_conversion(format!(
                    "Unsupported HTTP operation: {} {}",
                    method, path
                )));
            }
        };
        
        Ok(a2a_request)
    }
    
    /// Convert A2A response to HTTP format
    pub async fn a2a_to_http(
        &self,
        a2a_response: &Value,
        original_path: &str,
    ) -> Result<(u16, HashMap<String, String>, Vec<u8>)> {
        debug!("Converting A2A response to HTTP for path: {}", original_path);
        
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        
        // Handle different response types
        if let Some(error) = a2a_response.get("error") {
            // Error response
            let status_code = self.map_error_to_status_code(error);
            let body = serde_json::to_vec(a2a_response)
                .map_err(|e| GatewayError::protocol_conversion(format!("Failed to serialize error response: {}", e)))?;
            
            return Ok((status_code, headers, body));
        }
        
        // Success response
        let status_code = if original_path.contains("/tasks/send") {
            201 // Created
        } else {
            200 // OK
        };
        
        let body = serde_json::to_vec(a2a_response)
            .map_err(|e| GatewayError::protocol_conversion(format!("Failed to serialize response: {}", e)))?;
        
        Ok((status_code, headers, body))
    }
    
    /// Convert WebSocket message to A2A format
    pub async fn websocket_to_a2a(&self, message: &str) -> Result<Value> {
        debug!("Converting WebSocket message to A2A");
        
        let ws_data: Value = serde_json::from_str(message)
            .map_err(|e| GatewayError::protocol_conversion(format!("Invalid WebSocket JSON: {}", e)))?;
        
        // WebSocket messages are typically already in A2A format
        // Just validate and potentially upgrade version
        self.validate_and_upgrade_version(ws_data).await
    }
    
    /// Convert A2A message to WebSocket format
    pub async fn a2a_to_websocket(&self, a2a_message: &Value) -> Result<String> {
        debug!("Converting A2A message to WebSocket");
        
        // WebSocket messages are typically in A2A format
        serde_json::to_string(a2a_message)
            .map_err(|e| GatewayError::protocol_conversion(format!("Failed to serialize WebSocket message: {}", e)))
    }
    
    /// Convert send task request
    async fn convert_send_task_request(
        &self,
        request_data: Value,
        headers: &HashMap<String, String>,
    ) -> Result<Value> {
        let mut a2a_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent/sendMessage",
            "id": self.generate_request_id(),
        });
        
        // Extract parameters from request data
        if let Some(params) = request_data.as_object() {
            a2a_request["params"] = Value::Object(params.clone());
        } else {
            a2a_request["params"] = request_data;
        }
        
        // Add headers as metadata if needed
        if let Some(task_id) = headers.get("X-Task-ID") {
            if let Some(params) = a2a_request["params"].as_object_mut() {
                params.insert("taskId".to_string(), Value::String(task_id.clone()));
            }
        }
        
        Ok(a2a_request)
    }
    
    /// Convert send task streaming request
    async fn convert_send_task_streaming_request(
        &self,
        request_data: Value,
        headers: &HashMap<String, String>,
    ) -> Result<Value> {
        let mut a2a_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent/sendMessageStreaming",
            "id": self.generate_request_id(),
        });
        
        if let Some(params) = request_data.as_object() {
            a2a_request["params"] = Value::Object(params.clone());
        } else {
            a2a_request["params"] = request_data;
        }
        
        // Add streaming-specific headers
        if let Some(task_id) = headers.get("X-Task-ID") {
            if let Some(params) = a2a_request["params"].as_object_mut() {
                params.insert("taskId".to_string(), Value::String(task_id.clone()));
            }
        }
        
        Ok(a2a_request)
    }
    
    /// Convert get task request
    async fn convert_get_task_request(
        &self,
        path: &str,
        _headers: &HashMap<String, String>,
    ) -> Result<Value> {
        // Extract task ID from path
        let task_id = self.extract_task_id_from_path(path)?;
        
        let a2a_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent/getTask",
            "id": self.generate_request_id(),
            "params": {
                "taskId": task_id
            }
        });
        
        Ok(a2a_request)
    }
    
    /// Convert cancel task request
    async fn convert_cancel_task_request(
        &self,
        path: &str,
        _headers: &HashMap<String, String>,
    ) -> Result<Value> {
        let task_id = self.extract_task_id_from_path(path)?;
        
        let a2a_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent/cancelTask",
            "id": self.generate_request_id(),
            "params": {
                "taskId": task_id
            }
        });
        
        Ok(a2a_request)
    }
    
    /// Convert agent card request
    async fn convert_agent_card_request(
        &self,
        _headers: &HashMap<String, String>,
    ) -> Result<Value> {
        let a2a_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent/getAgentCard",
            "id": self.generate_request_id(),
            "params": {}
        });
        
        Ok(a2a_request)
    }
    
    /// Validate and upgrade A2A message version if needed
    async fn validate_and_upgrade_version(&self, mut message: Value) -> Result<Value> {
        // Check if message has version information
        let version = message.get("jsonrpc")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0");
        
        if !self.supported_versions.contains(&version.to_string()) {
            warn!("Unsupported protocol version: {}, upgrading to {}", version, self.default_version);
            
            // Upgrade to default version
            if let Some(obj) = message.as_object_mut() {
                obj.insert("jsonrpc".to_string(), Value::String(self.default_version.clone()));
            }
        }
        
        Ok(message)
    }
    
    /// Extract task ID from URL path
    fn extract_task_id_from_path(&self, path: &str) -> Result<String> {
        // Extract task ID from paths like "/tasks/{task_id}" or "/tasks/{task_id}/status"
        let parts: Vec<&str> = path.split('/').collect();
        
        if let Some(tasks_index) = parts.iter().position(|&part| part == "tasks") {
            if let Some(task_id) = parts.get(tasks_index + 1) {
                if !task_id.is_empty() {
                    return Ok(task_id.to_string());
                }
            }
        }
        
        Err(GatewayError::protocol_conversion("Could not extract task ID from path"))
    }
    
    /// Generate a unique request ID
    fn generate_request_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
    
    /// Map A2A error to HTTP status code
    fn map_error_to_status_code(&self, error: &Value) -> u16 {
        if let Some(code) = error.get("code").and_then(|c| c.as_i64()) {
            match code {
                -32700 => 400, // Parse error
                -32600 => 400, // Invalid Request
                -32601 => 404, // Method not found
                -32602 => 400, // Invalid params
                -32603 => 500, // Internal error
                -32000 => 404, // Task not found
                -32001 => 401, // Unauthorized
                -32002 => 403, // Forbidden
                -32003 => 409, // Conflict
                -32004 => 429, // Rate limited
                _ => 500,      // Unknown error
            }
        } else {
            500 // Default to internal server error
        }
    }
}

impl Default for ProtocolConverter {
    fn default() -> Self {
        Self::new()
    }
}
