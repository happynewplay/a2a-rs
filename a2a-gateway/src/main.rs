use a2a_gateway::{
    config::Config,
    router,
    storage::{AgentRegistry, InMemoryRegistry},
};
use std::sync::Arc;
use tokio::{net::TcpListener, time};

/// The background task that periodically removes expired agents from the registry.
async fn sweeper_task(registry: Arc<dyn AgentRegistry>) {
    // Check every 10 seconds.
    let mut interval = time::interval(time::Duration::from_secs(10));
    loop {
        interval.tick().await;
        println!("Running sweeper task to prune expired agents...");
        registry.prune_expired();
    }
}

#[tokio::main]
async fn main() {
    // Load configuration from environment variables.
    let config = Config::new();

    // Initialize the in-memory registry.
    let registry = Arc::new(InMemoryRegistry::new());

    // Spawn the background sweeper task.
    let sweeper_registry = Arc::clone(&registry);
    tokio::spawn(sweeper_task(sweeper_registry));

    // Create the Axum router.
    let app = router::create_router(registry);

    // Define the server address from the loaded configuration.
    let address = format!("{}:{}", config.host, config.port);
    println!("A2A Gateway listening on {}", address);

    // Create a TCP listener.
    let listener = TcpListener::bind(&address)
        .await
        .expect("Failed to bind to address");

    // Start the server.
    axum::serve(listener, app)
        .await
        .expect("Server failed");
}
