use a2a_rs::AgentCard;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;

/// The entry stored in the registry for each agent.
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub agent_card: AgentCard,
    pub expires_at: DateTime<Utc>,
}

/// A trait defining the storage interface for the agent registry.
/// This allows for different storage backends to be implemented.
#[async_trait]
pub trait AgentRegistry: Send + Sync {
    /// Registers a new agent or updates an existing one.
    fn register(&self, agent_card: AgentCard, ttl: Duration) -> Result<(), String>;

    /// Deregisters an agent.
    fn deregister(&self, agent_id: &str) -> Result<(), String>;

    /// Updates the TTL of an agent.
    fn heartbeat(&self, agent_id: &str, ttl: Duration) -> Result<(), String>;

    /// Retrieves an agent by its ID.
    fn get(&self, agent_id: &str) -> Option<AgentCard>;

    /// Lists all registered agents.
    fn list(&self) -> Vec<AgentCard>;

    /// Searches for agents with a specific skill.
    fn search_by_skill(&self, skill: &str) -> Vec<AgentCard>;

    /// Removes all expired agents from the registry.
    fn prune_expired(&self);
}

/// An in-memory implementation of the `AgentRegistry` trait using DashMap.
#[derive(Debug, Default)]
pub struct InMemoryRegistry {
    agents: DashMap<String, RegistryEntry>,
}

impl InMemoryRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AgentRegistry for InMemoryRegistry {
    fn register(&self, agent_card: AgentCard, ttl: Duration) -> Result<(), String> {
        // NOTE: We are using the agent's `name` as its unique ID in the registry.
        // This assumes that all registered agents have a unique name.
        // A more robust implementation might use a different unique identifier.
        let agent_id = agent_card.name.clone();
        let entry = RegistryEntry {
            agent_card,
            expires_at: Utc::now() + ttl,
        };
        self.agents.insert(agent_id, entry);
        Ok(())
    }

    fn deregister(&self, agent_id: &str) -> Result<(), String> {
        self.agents.remove(agent_id);
        Ok(())
    }

    fn heartbeat(&self, agent_id: &str, ttl: Duration) -> Result<(), String> {
        match self.agents.get_mut(agent_id) {
            Some(mut entry) => {
                entry.value_mut().expires_at = Utc::now() + ttl;
                Ok(())
            }
            None => Err(format!("Agent with ID '{}' not found", agent_id)),
        }
    }

    fn get(&self, agent_id: &str) -> Option<AgentCard> {
        self.agents
            .get(agent_id)
            .map(|entry| entry.value().agent_card.clone())
    }

    fn list(&self) -> Vec<AgentCard> {
        self.agents
            .iter()
            .map(|entry| entry.value().agent_card.clone())
            .collect()
    }

    fn search_by_skill(&self, skill: &str) -> Vec<AgentCard> {
        self.agents
            .iter()
            .filter(|entry| {
                entry
                    .value()
                    .agent_card
                    .skills
                    .iter()
                    .any(|s| s.id == skill)
            })
            .map(|entry| entry.value().agent_card.clone())
            .collect()
    }

    fn prune_expired(&self) {
        let now = Utc::now();
        self.agents.retain(|_, entry| entry.expires_at > now);
    }
}
