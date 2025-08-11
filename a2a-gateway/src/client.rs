//! A client for interacting with the A2A Gateway service.

use a2a_rs::AgentCard;
use reqwest::Client;
use serde::de::DeserializeOwned;
use url::Url;

// The request structs are defined in the `handlers` module. We need to make them
// accessible to the client.
use crate::handlers::{DeregisterRequest, HeartbeatRequest, RegisterRequest, SearchQuery};

/// An error type for the GatewayClient.
#[derive(Debug, thiserror::Error)]
pub enum GatewayClientError {
    #[error("URL parsing error: {0}")]
    UrlParse(#[from] url::ParseError),
    #[error("Request error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

/// A client for the A2A Gateway.
#[derive(Debug, Clone)]
pub struct GatewayClient {
    base_url: Url,
    client: Client,
}

impl GatewayClient {
    /// Creates a new `GatewayClient` with the given base URL for the gateway service.
    pub fn new(base_url: String) -> Result<Self, GatewayClientError> {
        Ok(Self {
            base_url: Url::parse(&base_url)?,
            client: Client::new(),
        })
    }

    /// A helper function to perform a POST request.
    async fn post<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<(), GatewayClientError> {
        let url = self.base_url.join(path)?;
        self.client
            .post(url)
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// A helper function to perform a GET request.
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, GatewayClientError> {
        let url = self.base_url.join(path)?;
        Ok(self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    /// A helper function to perform a GET request with query parameters.
    async fn get_with_query<T: DeserializeOwned, Q: serde::Serialize>(
        &self,
        path: &str,
        query: &Q,
    ) -> Result<T, GatewayClientError> {
        let url = self.base_url.join(path)?;
        Ok(self
            .client
            .get(url)
            .query(query)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    /// Registers an agent with the gateway.
    pub async fn register(
        &self,
        agent_card: AgentCard,
        ttl_seconds: u64,
    ) -> Result<(), GatewayClientError> {
        let body = RegisterRequest {
            agent_card,
            ttl_seconds,
        };
        self.post("register", &body).await
    }

    /// Deregisters an agent from the gateway.
    pub async fn deregister(&self, agent_id: String) -> Result<(), GatewayClientError> {
        let body = DeregisterRequest { agent_id };
        self.post("deregister", &body).await
    }

    /// Sends a heartbeat to the gateway to keep an agent's registration alive.
    pub async fn heartbeat(
        &self,
        agent_id: String,
        ttl_seconds: u64,
    ) -> Result<(), GatewayClientError> {
        let body = HeartbeatRequest {
            agent_id,
            ttl_seconds,
        };
        self.post("heartbeat", &body).await
    }

    /// Lists all agents currently registered with the gateway.
    pub async fn list_agents(&self) -> Result<Vec<AgentCard>, GatewayClientError> {
        self.get("agents").await
    }

    /// Finds all agents that have a specific skill.
    pub async fn find_by_skill(&self, skill: String) -> Result<Vec<AgentCard>, GatewayClientError> {
        let query = SearchQuery { skill };
        self.get_with_query("agents/search", &query).await
    }
}
