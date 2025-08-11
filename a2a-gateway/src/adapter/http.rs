//! HTTP adapter for the A2A Gateway

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{any, get, post},
    Json, Router,
};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    compression::CompressionLayer,
};
use tracing::{debug, error, info, warn};

use crate::{
    application::{RequestRouter, ProtocolConverter, RequestContextBuilder},
    domain::{ServiceRegistry, RequestContext},
    port::{Authentication, AuthContext, AuthResult},
    Result, GatewayError,
};

/// HTTP adapter state
#[derive(Clone)]
pub struct HttpAdapterState {
    pub router: Arc<RequestRouter>,
    pub protocol_converter: Arc<ProtocolConverter>,
    pub auth: Arc<dyn Authentication>,
    pub registry: ServiceRegistry,
}

/// HTTP adapter for the A2A Gateway
#[derive(Debug)]
pub struct HttpAdapter {
    state: HttpAdapterState,
    bind_address: String,
}

impl HttpAdapter {
    /// Create a new HTTP adapter
    pub fn new(
        router: Arc<RequestRouter>,
        protocol_converter: Arc<ProtocolConverter>,
        auth: Arc<dyn Authentication>,
        registry: ServiceRegistry,
        bind_address: String,
    ) -> Self {
        let state = HttpAdapterState {
            router,
            protocol_converter,
            auth,
            registry,
        };
        
        Self {
            state,
            bind_address,
        }
    }
    
    /// Create the Axum router
    pub fn create_router(&self) -> Router {
        Router::new()
            // A2A protocol endpoints
            .route("/.well-known/agent-card", get(handle_agent_card))
            .route("/tasks/send", post(handle_send_task))
            .route("/tasks/sendSubscribe", post(handle_send_task_streaming))
            .route("/tasks/:task_id", get(handle_get_task))
            .route("/tasks/:task_id", axum::routing::delete(handle_cancel_task))
            
            // Gateway management endpoints
            .route("/health", get(handle_health))
            .route("/services", get(handle_list_services))
            .route("/stats", get(handle_stats))
            .route("/reload", post(handle_reload))
            
            // Catch-all for proxying other requests
            .route("/*path", any(handle_proxy))
            
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CompressionLayer::new())
                    .layer(CorsLayer::permissive())
            )
            .with_state(self.state.clone())
    }
    
    /// Start the HTTP server
    pub async fn start(&self) -> Result<()> {
        let app = self.create_router();
        
        let listener = tokio::net::TcpListener::bind(&self.bind_address)
            .await
            .map_err(|e| GatewayError::network(e))?;
        
        info!("HTTP adapter listening on {}", self.bind_address);
        
        axum::serve(listener, app)
            .await
            .map_err(|e| GatewayError::internal(format!("HTTP server error: {}", e)))?;
        
        Ok(())
    }
}

/// Handle agent card requests
async fn handle_agent_card(
    State(state): State<HttpAdapterState>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<impl IntoResponse, AppError> {
    debug!("Handling agent card request");
    
    // Create request context
    let context = create_request_context(&Method::GET, &uri, &headers)?;
    
    // Route the request
    let service = state.router.route(&context).await?
        .ok_or_else(|| GatewayError::service_unavailable("No services available"))?;
    
    // Forward request to selected service
    let response = forward_request(&service.url.to_string(), &Method::GET, "/.well-known/agent-card", &headers, &[]).await?;
    
    Ok(response)
}

/// Handle send task requests
async fn handle_send_task(
    State(state): State<HttpAdapterState>,
    headers: HeaderMap,
    uri: Uri,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, AppError> {
    debug!("Handling send task request");
    
    let start_time = Instant::now();
    
    // Authenticate request
    let auth_result = authenticate_request(&state, &Method::POST, &uri, &headers).await?;
    if let AuthResult::Failed(reason) = auth_result {
        return Ok(create_error_response(StatusCode::UNAUTHORIZED, &reason));
    }
    
    // Create request context
    let context = create_request_context(&Method::POST, &uri, &headers)?;
    
    // Route the request
    let service = state.router.route(&context).await?
        .ok_or_else(|| GatewayError::service_unavailable("No services available"))?;
    
    // Notify router about request start
    state.router.request_start(&service.id).await?;
    
    // Forward request to selected service
    let result = forward_request(&service.url.to_string(), &Method::POST, "/tasks/send", &headers, &body).await;
    
    // Notify router about request completion
    let response_time = start_time.elapsed();
    match &result {
        Ok(_) => {
            state.router.request_complete(&service.id, response_time).await?;
        }
        Err(e) => {
            state.router.request_failed(&service.id, &e.to_string()).await?;
        }
    }
    
    Ok(result?)
}

/// Handle send task streaming requests
async fn handle_send_task_streaming(
    State(state): State<HttpAdapterState>,
    headers: HeaderMap,
    uri: Uri,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, AppError> {
    debug!("Handling send task streaming request");
    
    // Similar to handle_send_task but for streaming
    let context = create_request_context(&Method::POST, &uri, &headers)?;
    let service = state.router.route(&context).await?
        .ok_or_else(|| GatewayError::service_unavailable("No services available"))?;
    
    let response = forward_request(&service.url.to_string(), &Method::POST, "/tasks/sendSubscribe", &headers, &body).await?;
    Ok(response)
}

/// Handle get task requests
async fn handle_get_task(
    State(state): State<HttpAdapterState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<impl IntoResponse, AppError> {
    debug!("Handling get task request for task: {}", task_id);
    
    let context = create_request_context(&Method::GET, &uri, &headers)?;
    let service = state.router.route(&context).await?
        .ok_or_else(|| GatewayError::service_unavailable("No services available"))?;
    
    let path = format!("/tasks/{}", task_id);
    let response = forward_request(&service.url.to_string(), &Method::GET, &path, &headers, &[]).await?;
    Ok(response)
}

/// Handle cancel task requests
async fn handle_cancel_task(
    State(state): State<HttpAdapterState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<impl IntoResponse, AppError> {
    debug!("Handling cancel task request for task: {}", task_id);
    
    let context = create_request_context(&Method::DELETE, &uri, &headers)?;
    let service = state.router.route(&context).await?
        .ok_or_else(|| GatewayError::service_unavailable("No services available"))?;
    
    let path = format!("/tasks/{}", task_id);
    let response = forward_request(&service.url.to_string(), &Method::DELETE, &path, &headers, &[]).await?;
    Ok(response)
}

/// Handle health check requests
async fn handle_health(
    State(state): State<HttpAdapterState>,
) -> Result<impl IntoResponse, AppError> {
    let service_count = state.registry.count().await;
    let healthy_count = state.registry.healthy_count().await;
    
    let health_data = serde_json::json!({
        "status": "healthy",
        "services": {
            "total": service_count,
            "healthy": healthy_count
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    
    Ok(Json(health_data))
}

/// Handle list services requests
async fn handle_list_services(
    State(state): State<HttpAdapterState>,
) -> Result<impl IntoResponse, AppError> {
    let services = state.registry.get_all().await?;
    Ok(Json(services))
}

/// Handle stats requests
async fn handle_stats(
    State(state): State<HttpAdapterState>,
) -> Result<impl IntoResponse, AppError> {
    let stats = state.router.get_stats().await?;
    Ok(Json(stats))
}

/// Handle reload requests
async fn handle_reload(
    State(_state): State<HttpAdapterState>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement configuration reload
    warn!("Configuration reload not yet implemented");
    Ok(Json(serde_json::json!({"message": "Reload not yet implemented"})))
}

/// Handle proxy requests (catch-all)
async fn handle_proxy(
    State(state): State<HttpAdapterState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, AppError> {
    debug!("Handling proxy request: {} {}", method, uri.path());
    
    let context = create_request_context(&method, &uri, &headers)?;
    let service = state.router.route(&context).await?
        .ok_or_else(|| GatewayError::service_unavailable("No services available"))?;
    
    let response = forward_request(&service.url.to_string(), &method, uri.path(), &headers, &body).await?;
    Ok(response)
}

/// Create request context from HTTP request
fn create_request_context(
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<RequestContext, GatewayError> {
    let mut context = RequestContextBuilder::new(uri.path().to_string());
    
    // Add headers
    for (name, value) in headers {
        if let Ok(value_str) = value.to_str() {
            context = context.header(name.to_string(), value_str.to_string());
        }
    }
    
    // Add query parameters
    if let Some(query) = uri.query() {
        for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
            context = context.query_param(key.to_string(), value.to_string());
        }
    }
    
    // Add method as custom data
    context = context.custom("method".to_string(), method.to_string());
    
    Ok(context.build())
}

/// Authenticate request
async fn authenticate_request(
    state: &HttpAdapterState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<AuthResult, GatewayError> {
    // Check if authentication is required
    if !state.auth.is_required(uri.path(), method.as_str()).await? {
        return Ok(AuthResult::Success(crate::port::AuthPrincipal::new(
            "anonymous".to_string(),
            "none".to_string(),
        )));
    }
    
    // Extract authentication information from headers
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            let parts: Vec<&str> = auth_str.splitn(2, ' ').collect();
            if parts.len() == 2 {
                let scheme = parts[0].to_lowercase();
                let credential = parts[1].to_string();
                
                let auth_context = AuthContext::new(
                    scheme,
                    credential,
                    uri.path().to_string(),
                    method.to_string(),
                );
                
                return state.auth.authenticate(&auth_context).await;
            }
        }
    }
    
    Ok(AuthResult::Required)
}

/// Forward request to backend service
async fn forward_request(
    service_url: &str,
    method: &Method,
    path: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Response<Body>, GatewayError> {
    let client = reqwest::Client::new();
    let url = format!("{}{}", service_url.trim_end_matches('/'), path);
    
    debug!("Forwarding request to: {} {}", method, url);
    
    let mut request_builder = match method {
        &Method::GET => client.get(&url),
        &Method::POST => client.post(&url),
        &Method::PUT => client.put(&url),
        &Method::DELETE => client.delete(&url),
        &Method::PATCH => client.patch(&url),
        _ => return Err(GatewayError::internal(format!("Unsupported method: {}", method))),
    };
    
    // Add headers (excluding host and other problematic headers)
    for (name, value) in headers {
        let name_str = name.as_str();
        if !["host", "content-length", "transfer-encoding"].contains(&name_str.to_lowercase().as_str()) {
            if let Ok(value_str) = value.to_str() {
                request_builder = request_builder.header(name_str, value_str);
            }
        }
    }
    
    // Add body if present
    if !body.is_empty() {
        request_builder = request_builder.body(body.to_vec());
    }
    
    let response = request_builder
        .send()
        .await
        .map_err(|e| GatewayError::network(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    
    // Convert reqwest response to axum response
    let status = StatusCode::from_u16(response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    
    let mut response_headers = HeaderMap::new();
    for (name, value) in response.headers() {
        response_headers.insert(name, value.clone());
    }
    
    let body_bytes = response.bytes().await
        .map_err(|e| GatewayError::network(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    
    let mut response_builder = Response::builder().status(status);
    for (name, value) in response_headers {
        response_builder = response_builder.header(name, value);
    }
    
    response_builder
        .body(Body::from(body_bytes.to_vec()))
        .map_err(|e| GatewayError::internal(format!("Failed to build response: {}", e)))
}

/// Create error response
fn create_error_response(status: StatusCode, message: &str) -> Response<Body> {
    let error_body = serde_json::json!({
        "error": {
            "code": status.as_u16(),
            "message": message
        }
    });
    
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(error_body.to_string()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

/// Application error wrapper
#[derive(Debug)]
struct AppError(GatewayError);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            GatewayError::Authentication(_) => StatusCode::UNAUTHORIZED,
            GatewayError::ServiceNotFound(_) => StatusCode::NOT_FOUND,
            GatewayError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            GatewayError::Timeout(_) => StatusCode::REQUEST_TIMEOUT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        
        create_error_response(status, &self.0.to_string())
    }
}

impl<E> From<E> for AppError
where
    E: Into<GatewayError>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
