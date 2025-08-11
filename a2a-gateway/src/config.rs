//! Handles loading configuration from the environment.

#[derive(Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
}

impl Config {
    /// Creates a new Config instance, loading values from environment variables
    /// with fallbacks to default values.
    ///
    /// - `GATEWAY_HOST`: The host address to bind to (default: "127.0.0.1").
    /// - `GATEWAY_PORT`: The port to bind to (default: 3000).
    pub fn new() -> Self {
        let host = std::env::var("GATEWAY_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("GATEWAY_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3000);

        Self { host, port }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
