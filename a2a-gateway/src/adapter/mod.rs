//! Adapter layer for the A2A Gateway
//!
//! Contains concrete implementations of the ports.

pub mod http;
pub mod websocket;
pub mod discovery;
pub mod monitoring;
pub mod auth;

pub use http::HttpAdapter;
pub use websocket::WebSocketAdapter;
pub use discovery::ServiceDiscoveryAdapter;
pub use monitoring::MonitoringAdapter;
pub use auth::AuthAdapter;
