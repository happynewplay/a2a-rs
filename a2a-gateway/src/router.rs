use crate::{handlers, storage::AgentRegistry};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn create_router(registry: Arc<dyn AgentRegistry>) -> Router {
    Router::new()
        .route("/register", post(handlers::register))
        .route("/deregister", post(handlers::deregister))
        .route("/heartbeat", post(handlers::heartbeat))
        .route("/agents", get(handlers::list_agents))
        .route("/agents/:id", get(handlers::get_agent))
        .route("/agents/search", get(handlers::search_agents))
        .with_state(registry)
}
