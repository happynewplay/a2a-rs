use crate::storage::AgentRegistry;
use a2a_rs::AgentCard;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::Duration;
use serde::Deserialize;
use std::sync::Arc;

// A type alias for our shared state. This makes it easier to change the
// underlying registry implementation later on.
pub type AppState = Arc<dyn AgentRegistry>;

// --- Request Body Structs ---

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub agent_card: AgentCard,
    pub ttl_seconds: u64,
}

#[derive(Deserialize)]
pub struct DeregisterRequest {
    pub agent_id: String,
}

#[derive(Deserialize)]
pub struct HeartbeatRequest {
    pub agent_id: String,
    pub ttl_seconds: u64,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub skill: String,
}

// --- API Handlers ---

pub async fn register(
    State(registry): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let ttl = Duration::seconds(payload.ttl_seconds as i64);
    registry
        .register(payload.agent_card, ttl)
        .map(|_| StatusCode::OK)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

pub async fn deregister(
    State(registry): State<AppState>,
    Json(payload): Json<DeregisterRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    registry
        .deregister(&payload.agent_id)
        .map(|_| StatusCode::OK)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

pub async fn heartbeat(
    State(registry): State<AppState>,
    Json(payload): Json<HeartbeatRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let ttl = Duration::seconds(payload.ttl_seconds as i64);
    registry
        .heartbeat(&payload.agent_id, ttl)
        .map(|_| StatusCode::OK)
        .map_err(|e| (StatusCode::NOT_FOUND, e))
}

pub async fn list_agents(State(registry): State<AppState>) -> Json<Vec<AgentCard>> {
    Json(registry.list())
}

pub async fn get_agent(
    State(registry): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<AgentCard>, StatusCode> {
    registry
        .get(&agent_id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn search_agents(
    State(registry): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Json<Vec<AgentCard>> {
    Json(registry.search_by_skill(&query.skill))
}
