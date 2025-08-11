use a2a_gateway::{router, storage::InMemoryRegistry};
use a2a_rs::{AgentCard, AgentCapabilities, AgentSkill};
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpListener; // Use tokio's TcpListener

// --- Test Helper ---

/// Spawns the gateway server in the background on a random port.
/// Returns the address of the server (e.g., "http://127.0.0.1:12345").
async fn spawn_app() -> String {
    let listener = TcpListener::bind("127.0.0.1:0") // This now correctly uses tokio::net::TcpListener
        .await
        .expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let registry = Arc::new(InMemoryRegistry::new());
    let app = router::create_router(registry);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://127.0.0.1:{}", port)
}

// --- Test Cases ---

#[tokio::test]
async fn test_register_and_get_agent() {
    // Arrange
    let address = spawn_app().await;
    let client = reqwest::Client::new();
    let agent_card = AgentCard::builder()
        .name("test-agent".to_string())
        .description("An agent for testing".to_string())
        .url("http://test-agent.local".to_string())
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![])
        .default_input_modes(vec!["text".to_string()]) // Added missing field
        .default_output_modes(vec!["text".to_string()]) // Added missing field
        .build();

    let register_payload = json!({
        "agent_card": agent_card,
        "ttl_seconds": 60
    });

    // Act: Register the agent
    let response = client
        .post(&format!("{}/register", address))
        .json(&register_payload)
        .send()
        .await
        .expect("Failed to execute register request.");

    // Assert: Registration was successful
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    // Act: Get the agent by its name (which we use as ID)
    let response = client
        .get(&format!("{}/agents/test-agent", address))
        .send()
        .await
        .expect("Failed to execute get request.");

    // Assert: Get was successful and returns the correct agent
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let received_card: AgentCard = response
        .json()
        .await
        .expect("Failed to parse agent card from response.");
    assert_eq!(agent_card.name, received_card.name);
    assert_eq!(agent_card.description, received_card.description);
}

#[tokio::test]
async fn test_deregister_agent() {
    // Arrange
    let address = spawn_app().await;
    let client = reqwest::Client::new();
    let agent_card = AgentCard::builder()
        .name("test-agent-to-deregister".to_string())
        .description("This agent will be deregistered".to_string())
        .url("http://deregister-agent.local".to_string())
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![])
        .default_input_modes(vec!["text".to_string()])
        .default_output_modes(vec!["text".to_string()])
        .build();

    // Act: Register the agent first
    client
        .post(&format!("{}/register", address))
        .json(&json!({ "agent_card": agent_card, "ttl_seconds": 60 }))
        .send()
        .await
        .expect("Failed to execute register request.");

    // Act: Deregister the agent
    let deregister_response = client
        .post(&format!("{}/deregister", address))
        .json(&json!({ "agent_id": "test-agent-to-deregister" }))
        .send()
        .await
        .expect("Failed to execute deregister request.");

    // Assert: Deregistration was successful
    assert_eq!(deregister_response.status(), reqwest::StatusCode::OK);

    // Act: Try to get the agent again
    let get_response = client
        .get(&format!("{}/agents/test-agent-to-deregister", address))
        .send()
        .await
        .expect("Failed to execute get request.");

    // Assert: The agent is no longer found
    assert_eq!(get_response.status(), reqwest::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_heartbeat_ok_for_known_agent() {
    // Arrange
    let address = spawn_app().await;
    let client = reqwest::Client::new();
    let agent_card = AgentCard::builder()
        .name("heartbeat-agent".to_string())
        .description("This agent will send a heartbeat".to_string())
        .url("http://heartbeat-agent.local".to_string())
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![])
        .default_input_modes(vec!["text".to_string()])
        .default_output_modes(vec!["text".to_string()])
        .build();

    // Act: Register the agent
    client
        .post(&format!("{}/register", address))
        .json(&json!({ "agent_card": agent_card, "ttl_seconds": 60 }))
        .send()
        .await
        .expect("Failed to register agent.");

    // Act: Send a heartbeat
    let heartbeat_response = client
        .post(&format!("{}/heartbeat", address))
        .json(&json!({ "agent_id": "heartbeat-agent", "ttl_seconds": 120 }))
        .send()
        .await
        .expect("Failed to send heartbeat.");

    // Assert: Heartbeat was successful
    assert_eq!(heartbeat_response.status(), reqwest::StatusCode::OK);

    // Act: Send a heartbeat for an unknown agent
    let bad_heartbeat_response = client
        .post(&format!("{}/heartbeat", address))
        .json(&json!({ "agent_id": "unknown-agent", "ttl_seconds": 120 }))
        .send()
        .await
        .expect("Failed to send heartbeat for unknown agent.");

    // Assert: Heartbeat for unknown agent fails
    assert_eq!(
        bad_heartbeat_response.status(),
        reqwest::StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_list_and_search_agents() {
    // Arrange
    let address = spawn_app().await;
    let client = reqwest::Client::new();

    // Agent 1: Translator
    let translator_card = AgentCard::builder()
        .name("translator".to_string())
        .description("Translates text".to_string())
        .url("http://translator.local".to_string())
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![AgentSkill::new(
            "translate".to_string(),
            "Translate Text".to_string(),
            "Translates text from one language to another.".to_string(),
            vec!["nlp".to_string()],
        )])
        .default_input_modes(vec!["text".to_string()])
        .default_output_modes(vec!["text".to_string()])
        .build();

    // Agent 2: Calculator
    let calculator_card = AgentCard::builder()
        .name("calculator".to_string())
        .description("Calculates math expressions".to_string())
        .url("http://calculator.local".to_string())
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![AgentSkill::new(
            "calculate".to_string(),
            "Calculate".to_string(),
            "Performs calculations.".to_string(),
            vec!["math".to_string()],
        )])
        .default_input_modes(vec!["text".to_string()])
        .default_output_modes(vec!["text".to_string()])
        .build();

    // Act: Register both agents
    client
        .post(&format!("{}/register", address))
        .json(&json!({ "agent_card": translator_card, "ttl_seconds": 60 }))
        .send()
        .await
        .unwrap();
    client
        .post(&format!("{}/register", address))
        .json(&json!({ "agent_card": calculator_card, "ttl_seconds": 60 }))
        .send()
        .await
        .unwrap();

    // Act: List all agents
    let list_response = client
        .get(&format!("{}/agents", address))
        .send()
        .await
        .unwrap();

    // Assert: List contains both agents
    assert_eq!(list_response.status(), reqwest::StatusCode::OK);
    let agents: Vec<AgentCard> = list_response.json().await.unwrap();
    assert_eq!(agents.len(), 2);
    assert!(agents.iter().any(|a| a.name == "translator"));
    assert!(agents.iter().any(|a| a.name == "calculator"));

    // Act: Search for agents with the 'translate' skill
    let search_response = client
        .get(&format!("{}/agents/search?skill=translate", address))
        .send()
        .await
        .unwrap();

    // Assert: Search finds only the translator agent
    assert_eq!(search_response.status(), reqwest::StatusCode::OK);
    let search_results: Vec<AgentCard> = search_response.json().await.unwrap();
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].name, "translator");
}
