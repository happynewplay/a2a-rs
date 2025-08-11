//! A multi-agent example demonstrating the use of the A2A Gateway.
//!
//! This example simulates a complete A2A flow where an "Orchestrator" agent
//! receives a request, discovers a "Calculator" tool agent via the gateway,
//! and then communicates with it directly to fulfill the request.
//!
//! # Prerequisites
//!
//! 1. **Run the Gateway Service:**
//!    In a separate terminal, start the gateway service first:
//!    ```sh
//!    cargo run --bin a2a-gateway
//!    ```
//!
//! 2. **Run this Example:**
//!    In another terminal, run this example:
//!    ```sh
//!    cargo run --example multi_agent_gateway_demo
//!    ```

use a2a_gateway::client::GatewayClient;
use a2a_rs::{
    adapter::{DefaultRequestProcessor, HttpServer, InMemoryTaskStorage, NoopPushNotificationSender, SimpleAgentInfo},
    port::AsyncMessageHandler,
    services::AsyncA2AClient,
    AgentCard, AgentCapabilities, AgentSkill, HttpClient, Message, Part, Role, Task, TaskState,
    A2AError,
};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

// --- Main Simulation Logic ---

#[tokio::main]
async fn main() -> Result<()> {
    println!("--- Multi-Agent Gateway Demo ---");
    let gateway_url = "http://127.0.0.1:3000".to_string();
    let gateway_client = Arc::new(GatewayClient::new(gateway_url.clone())?);

    // 1. Start the CalculatorAgent (Tool Agent).
    let calculator_card = AgentCard::builder()
        .name("calculator".to_string())
        .description("An agent that can perform addition.".to_string())
        .url("".to_string()) // URL will be set by the helper
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![AgentSkill::new(
            "add".to_string(),
            "Addition".to_string(),
            "Adds two numbers.".to_string(),
            vec!["math".to_string()],
        )])
        .default_input_modes(vec!["text".to_string()])
        .default_output_modes(vec!["text".to_string()])
        .build();

    let calculator_agent = CalculatorAgent;
    start_agent(calculator_agent, calculator_card, gateway_client.clone()).await?;
    println!("✅ CalculatorAgent started and registered.");

    // 2. Start the OrchestratorAgent.
    let orchestrator_card = AgentCard::builder()
        .name("orchestrator".to_string())
        .description("An agent that coordinates tasks.".to_string())
        .url("".to_string()) // URL will be set by the helper
        .version("1.0.0".to_string())
        .capabilities(AgentCapabilities::default())
        .skills(vec![])
        .default_input_modes(vec!["text".to_string()])
        .default_output_modes(vec!["text".to_string()])
        .build();

    let orchestrator_agent = OrchestratorAgent::new(gateway_client.clone());
    let orchestrator_url =
        start_agent(orchestrator_agent, orchestrator_card, gateway_client.clone()).await?;
    println!("✅ OrchestratorAgent started and registered at {}", orchestrator_url);

    // 3. Wait for a moment for agents to be discoverable.
    sleep(Duration::from_secs(1)).await;

    // 4. Simulate a client request to the OrchestratorAgent.
    println!("\n🚀 Simulating client request to OrchestratorAgent...");
    let client = HttpClient::new(orchestrator_url);
    let user_message = Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("add 5 3".to_string())])
        .message_id(Uuid::new_v4().to_string())
        .build();

    let final_task = client
        .send_task_message("user-task-1", &user_message, None, None)
        .await?;

    // 5. Print the final result.
    println!("\n🏁 Final Result from OrchestratorAgent:");
    if let Some(history) = final_task.history {
        if let Some(last_message) = history.last() {
            if let Some(last_part) = last_message.parts.first() {
                if let Part::Text { text, .. } = last_part {
                    println!("✅ Success! Final response: \"{}\"", text);
                }
            }
        }
    }

    println!("\n--- Demo Complete ---");
    Ok(())
}

// --- Agent Definitions ---

/// A simple tool agent that can perform addition.
#[derive(Debug, Clone)]
struct CalculatorAgent;

/// The data structure for the 'add' skill's input.
#[derive(Debug, Serialize, Deserialize)]
struct AddRequest {
    a: i64,
    b: i64,
}

/// The data structure for the 'add' skill's output.
#[derive(Debug, Serialize, Deserialize)]
struct AddResponse {
    result: i64,
}

#[async_trait]
impl AsyncMessageHandler for CalculatorAgent {
    async fn process_message<'a>(
        &self,
        task_id: &'a str,
        message: &'a Message,
        _session_id: Option<&'a str>,
    ) -> Result<Task, A2AError> {
        println!("\n🧮 CalculatorAgent: Received a message for task {}", task_id);
        let data_part = message.parts.iter().find_map(|part| match part {
            Part::Data { data, .. } => Some(data),
            _ => None,
        });
        if let Some(data) = data_part {
            if let Ok(add_request) = serde_json::from_value::<AddRequest>(data.clone().into()) {
                let result = add_request.a + add_request.b;
                let response_data = AddResponse { result };
                let response_json = json!(response_data);
                let response_map = response_json.as_object().unwrap().clone();
                let response_part = Part::data(response_map);
                let response_message = Message::builder()
                    .role(Role::Agent)
                    .parts(vec![response_part])
                    .message_id(Uuid::new_v4().to_string())
                    .build();
                let context_id = message.context_id.clone().unwrap_or_default();
                let mut task = Task::new(task_id.to_string(), context_id);
                task.status.state = TaskState::Completed;
                task.history = Some(vec![message.clone(), response_message]);
                println!("🧮 CalculatorAgent: Calculation complete.");
                return Ok(task);
            }
        }
        Err(A2AError::InvalidRequest(
            "Missing or invalid DataPart for addition".to_string(),
        ))
    }
}

/// An agent that coordinates with other agents to fulfill requests.
#[derive(Debug, Clone)]
struct OrchestratorAgent {
    gateway_client: Arc<GatewayClient>,
}

impl OrchestratorAgent {
    fn new(gateway_client: Arc<GatewayClient>) -> Self {
        Self { gateway_client }
    }
}

#[async_trait]
impl AsyncMessageHandler for OrchestratorAgent {
    async fn process_message<'a>(
        &self,
        task_id: &'a str,
        message: &'a Message,
        _session_id: Option<&'a str>,
    ) -> Result<Task, A2AError> {
        println!("\n🤖 OrchestratorAgent: Received a message for task {}", task_id);
        let user_text = message
            .parts
            .iter()
            .find_map(|part| match part {
                Part::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .unwrap_or("");
        let parts: Vec<&str> = user_text.split_whitespace().collect();
        if parts.len() != 3 || parts[0] != "add" {
            return Err(A2AError::InvalidRequest(
                "Request format should be: 'add <num1> <num2>'".to_string(),
            ));
        }
        let a: i64 = parts[1]
            .parse()
            .map_err(|_| A2AError::InvalidRequest("Invalid number 'a'".to_string()))?;
        let b: i64 = parts[2]
            .parse()
            .map_err(|_| A2AError::InvalidRequest("Invalid number 'b'".to_string()))?;
        println!("🤖 OrchestratorAgent: Parsed request to add {} and {}", a, b);
        println!("🤖 OrchestratorAgent: Discovering agents with 'add' skill...");
        let calculator_agents = self
            .gateway_client
            .find_by_skill("add".to_string())
            .await
            .map_err(|e| A2AError::Internal(format!("Gateway discovery failed: {}", e)))?;
        let calculator_card = match calculator_agents.first() {
            Some(card) => card,
            None => {
                return Err(A2AError::Internal(
                    "No calculator agent found".to_string(),
                ));
            }
        };
        println!(
            "🤖 OrchestratorAgent: Found CalculatorAgent at {}",
            calculator_card.url
        );
        let tool_agent_client = HttpClient::new(calculator_card.url.clone());
        let add_request = AddRequest { a, b };
        let request_part = Part::data(json!(add_request).as_object().unwrap().clone());
        let request_message_to_tool = Message::builder()
            .role(Role::User)
            .parts(vec![request_part])
            .message_id(Uuid::new_v4().to_string())
            .build();
        println!("🤖 OrchestratorAgent: Sending request to CalculatorAgent...");
        let tool_task = tool_agent_client
            .send_task_message("tool-task", &request_message_to_tool, None, None)
            .await
            .map_err(|e| A2AError::Internal(format!("Failed to call tool agent: {}", e)))?;
        let tool_response_message = tool_task.history.as_ref().and_then(|h| h.last());
        let result = if let Some(msg) = tool_response_message {
            msg.parts.iter().find_map(|part| match part {
                Part::Data { data, .. } => {
                    serde_json::from_value::<AddResponse>(data.clone().into()).ok()
                }
                _ => None,
            })
        } else {
            None
        };
        let final_result_text = match result {
            Some(add_response) => {
                format!("The result is {}", add_response.result)
            }
            None => "Failed to get result from calculator agent.".to_string(),
        };
        let final_response_part = Part::text(final_result_text);
        let final_response_message = Message::builder()
            .role(Role::Agent)
            .parts(vec![final_response_part])
            .message_id(Uuid::new_v4().to_string())
            .build();
        let context_id = message.context_id.clone().unwrap_or_default();
        let mut final_task = Task::new(task_id.to_string(), context_id);
        final_task.status.state = TaskState::Completed;
        final_task.history = Some(vec![message.clone(), final_response_message]);
        println!(
            "🤖 OrchestratorAgent: Responding to original request for task {}.",
            task_id
        );
        Ok(final_task)
    }
}

// --- Helper Functions ---

/// A helper to start an agent server, register it, and manage heartbeats.
async fn start_agent(
    handler: impl AsyncMessageHandler + Clone + Send + Sync + 'static,
    mut agent_card: AgentCard,
    gateway_client: Arc<GatewayClient>,
) -> Result<String> {
    // 1. Find a free port to run the agent on.
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let url = format!("http://{}", addr);
    agent_card.url = url.clone();
    drop(listener); // Drop the listener so the port is free again for the server to bind.

    // 2. Create the server components.
    let task_manager = InMemoryTaskStorage::new();
    let notification_manager = NoopPushNotificationSender;
    let processor = DefaultRequestProcessor::new(handler, task_manager, notification_manager);
    let agent_info = SimpleAgentInfo::new(agent_card.name.clone(), agent_card.version.clone());

    // 3. Create the HttpServer instance. It will bind to the address itself in `start()`.
    let server = HttpServer::new(processor, agent_info, addr.to_string());

    // 4. Spawn the server to run in the background.
    let server_card = agent_card.clone();
    tokio::spawn(async move {
        if let Err(e) = server.start().await {
            eprintln!("Server for agent {} failed: {}", server_card.name, e);
        }
    });

    // 5. Register the agent with the gateway.
    let ttl_seconds = 30;
    gateway_client
        .register(agent_card.clone(), ttl_seconds)
        .await?;

    // 6. Spawn a background task to send heartbeats.
    let heartbeat_card = agent_card; // Move ownership to the task
    tokio::spawn(async move {
        let heartbeat_interval = Duration::from_secs(ttl_seconds / 2);
        loop {
            sleep(heartbeat_interval).await;
            println!("❤️ Sending heartbeat for agent: {}", heartbeat_card.name);
            if let Err(e) = gateway_client
                .heartbeat(heartbeat_card.name.clone(), ttl_seconds)
                .await
            {
                eprintln!(
                    "Failed to send heartbeat for agent {}: {}",
                    heartbeat_card.name, e
                );
            }
        }
    });

    Ok(url)
}
