//! WebSocket adapter for the A2A Gateway

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::HeaderMap,
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    application::{RequestRouter, ProtocolConverter},
    domain::{ServiceRegistry, RequestContext},
    port::{Authentication, AuthContext, AuthResult},
    Result, GatewayError,
};

/// WebSocket connection information
#[derive(Debug, Clone)]
pub struct WebSocketConnection {
    pub id: String,
    pub client_ip: Option<std::net::IpAddr>,
    pub authenticated: bool,
    pub principal: Option<crate::port::AuthPrincipal>,
}

/// WebSocket adapter state
#[derive(Clone)]
pub struct WebSocketAdapterState {
    pub router: Arc<RequestRouter>,
    pub protocol_converter: Arc<ProtocolConverter>,
    pub auth: Arc<dyn Authentication>,
    pub registry: ServiceRegistry,
    pub connections: Arc<RwLock<HashMap<String, WebSocketConnection>>>,
}

/// WebSocket adapter for the A2A Gateway
#[derive(Debug)]
pub struct WebSocketAdapter {
    state: WebSocketAdapterState,
    bind_address: String,
}

impl WebSocketAdapter {
    /// Create a new WebSocket adapter
    pub fn new(
        router: Arc<RequestRouter>,
        protocol_converter: Arc<ProtocolConverter>,
        auth: Arc<dyn Authentication>,
        registry: ServiceRegistry,
        bind_address: String,
    ) -> Self {
        let state = WebSocketAdapterState {
            router,
            protocol_converter,
            auth,
            registry,
            connections: Arc::new(RwLock::new(HashMap::new())),
        };
        
        Self {
            state,
            bind_address,
        }
    }
    
    /// Handle WebSocket upgrade
    pub async fn handle_upgrade(
        State(state): State<WebSocketAdapterState>,
        ws: WebSocketUpgrade,
        headers: HeaderMap,
    ) -> Response {
        debug!("WebSocket upgrade requested");
        
        ws.on_upgrade(move |socket| handle_websocket(socket, state, headers))
    }
    
    /// Get connection count
    pub async fn connection_count(&self) -> usize {
        self.state.connections.read().await.len()
    }
    
    /// Broadcast message to all connections
    pub async fn broadcast(&self, message: &str) -> Result<()> {
        let connections = self.state.connections.read().await;
        
        for (connection_id, _connection) in connections.iter() {
            // In a real implementation, we would need to store the WebSocket senders
            // and send messages to them. This is a simplified version.
            debug!("Broadcasting message to connection: {}", connection_id);
        }
        
        Ok(())
    }
}

/// Handle WebSocket connection
async fn handle_websocket(
    socket: WebSocket,
    state: WebSocketAdapterState,
    headers: HeaderMap,
) {
    let connection_id = Uuid::new_v4().to_string();
    
    info!("New WebSocket connection: {}", connection_id);
    
    // Create connection info
    let connection = WebSocketConnection {
        id: connection_id.clone(),
        client_ip: None, // Would extract from headers in real implementation
        authenticated: false,
        principal: None,
    };
    
    // Register connection
    {
        let mut connections = state.connections.write().await;
        connections.insert(connection_id.clone(), connection);
    }
    
    // Split the socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();
    
    // Create channels for communication
    let (tx, mut rx) = mpsc::channel::<String>(32);
    
    // Spawn task to handle outgoing messages
    let connection_id_clone = connection_id.clone();
    let outgoing_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if let Err(e) = sender.send(Message::Text(message)).await {
                error!("Failed to send WebSocket message: {}", e);
                break;
            }
        }
        debug!("Outgoing message task ended for connection: {}", connection_id_clone);
    });
    
    // Handle incoming messages
    let connection_id_clone = connection_id.clone();
    let state_clone = state.clone();
    let incoming_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Err(e) = handle_websocket_message(
                        &connection_id_clone,
                        &text,
                        &state_clone,
                        &tx,
                    ).await {
                        error!("Error handling WebSocket message: {}", e);
                        let error_response = create_error_message(&e.to_string());
                        if let Err(send_err) = tx.send(error_response).await {
                            error!("Failed to send error response: {}", send_err);
                        }
                    }
                }
                Ok(Message::Binary(_)) => {
                    warn!("Binary WebSocket messages not supported");
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket connection closed: {}", connection_id_clone);
                    break;
                }
                Ok(Message::Ping(data)) => {
                    if let Err(e) = tx.send(format!("{{\"type\":\"pong\",\"data\":\"{}\"}}", 
                                                   base64::encode(&data))).await {
                        error!("Failed to send pong: {}", e);
                    }
                }
                Ok(Message::Pong(_)) => {
                    debug!("Received pong from connection: {}", connection_id_clone);
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
            }
        }
        debug!("Incoming message task ended for connection: {}", connection_id_clone);
    });
    
    // Wait for either task to complete
    tokio::select! {
        _ = incoming_task => {},
        _ = outgoing_task => {},
    }
    
    // Clean up connection
    {
        let mut connections = state.connections.write().await;
        connections.remove(&connection_id);
    }
    
    info!("WebSocket connection closed and cleaned up: {}", connection_id);
}

/// Handle individual WebSocket message
async fn handle_websocket_message(
    connection_id: &str,
    message: &str,
    state: &WebSocketAdapterState,
    response_sender: &mpsc::Sender<String>,
) -> Result<()> {
    debug!("Handling WebSocket message from {}: {}", connection_id, message);
    
    // Parse the message as JSON
    let message_data: serde_json::Value = serde_json::from_str(message)
        .map_err(|e| GatewayError::protocol_conversion(format!("Invalid JSON: {}", e)))?;
    
    // Check if this is an authentication message
    if let Some(msg_type) = message_data.get("type").and_then(|t| t.as_str()) {
        if msg_type == "auth" {
            return handle_authentication(connection_id, &message_data, state, response_sender).await;
        }
    }
    
    // Check if connection is authenticated (if required)
    let connection = {
        let connections = state.connections.read().await;
        connections.get(connection_id).cloned()
    };
    
    let connection = connection.ok_or_else(|| {
        GatewayError::internal("Connection not found")
    })?;
    
    // For now, we'll allow unauthenticated connections
    // In a real implementation, you might require authentication for certain operations
    
    // Convert WebSocket message to A2A format
    let a2a_message = state.protocol_converter.websocket_to_a2a(message).await?;
    
    // Create request context
    let context = RequestContext::new("/ws".to_string())
        .with_custom("connection_id".to_string(), connection_id.to_string())
        .with_custom("protocol".to_string(), "websocket".to_string());
    
    // Route the request
    let service = state.router.route(&context).await?
        .ok_or_else(|| GatewayError::service_unavailable("No services available"))?;
    
    // Forward the message to the selected service
    let response = forward_websocket_message(&service.url.to_string(), &a2a_message).await?;
    
    // Convert response back to WebSocket format
    let ws_response = state.protocol_converter.a2a_to_websocket(&response).await?;
    
    // Send response back to client
    response_sender.send(ws_response).await
        .map_err(|e| GatewayError::internal(format!("Failed to send response: {}", e)))?;
    
    Ok(())
}

/// Handle authentication message
async fn handle_authentication(
    connection_id: &str,
    message: &serde_json::Value,
    state: &WebSocketAdapterState,
    response_sender: &mpsc::Sender<String>,
) -> Result<()> {
    debug!("Handling authentication for connection: {}", connection_id);
    
    // Extract authentication data
    let scheme = message.get("scheme")
        .and_then(|s| s.as_str())
        .unwrap_or("bearer")
        .to_string();
    
    let credential = message.get("credential")
        .and_then(|c| c.as_str())
        .ok_or_else(|| GatewayError::authentication("Missing credential"))?
        .to_string();
    
    // Create auth context
    let auth_context = AuthContext::new(
        scheme,
        credential,
        "/ws".to_string(),
        "WEBSOCKET".to_string(),
    );
    
    // Authenticate
    let auth_result = state.auth.authenticate(&auth_context).await?;
    
    let response = match auth_result {
        AuthResult::Success(principal) => {
            // Update connection with authentication info
            {
                let mut connections = state.connections.write().await;
                if let Some(connection) = connections.get_mut(connection_id) {
                    connection.authenticated = true;
                    connection.principal = Some(principal.clone());
                }
            }
            
            serde_json::json!({
                "type": "auth_response",
                "success": true,
                "principal": {
                    "id": principal.id,
                    "scheme": principal.scheme
                }
            }).to_string()
        }
        AuthResult::Failed(reason) => {
            serde_json::json!({
                "type": "auth_response",
                "success": false,
                "error": reason
            }).to_string()
        }
        AuthResult::Required => {
            serde_json::json!({
                "type": "auth_response",
                "success": false,
                "error": "Authentication required"
            }).to_string()
        }
    };
    
    response_sender.send(response).await
        .map_err(|e| GatewayError::internal(format!("Failed to send auth response: {}", e)))?;
    
    Ok(())
}

/// Forward WebSocket message to backend service
async fn forward_websocket_message(
    service_url: &str,
    message: &serde_json::Value,
) -> Result<serde_json::Value> {
    // For now, we'll use HTTP to forward the message
    // In a real implementation, you might want to establish WebSocket connections
    // to backend services as well
    
    let client = reqwest::Client::new();
    let url = format!("{}/", service_url.trim_end_matches('/'));
    
    debug!("Forwarding WebSocket message to: {}", url);
    
    let response = client
        .post(&url)
        .json(message)
        .send()
        .await
        .map_err(|e| GatewayError::network(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    
    if !response.status().is_success() {
        return Err(GatewayError::internal(format!(
            "Backend service returned error: {}",
            response.status()
        )));
    }
    
    let response_data: serde_json::Value = response.json().await
        .map_err(|e| GatewayError::protocol_conversion(format!("Invalid response JSON: {}", e)))?;
    
    Ok(response_data)
}

/// Create error message
fn create_error_message(error: &str) -> String {
    serde_json::json!({
        "type": "error",
        "error": error
    }).to_string()
}
