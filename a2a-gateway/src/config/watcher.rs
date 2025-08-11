//! Configuration file watcher for hot reload

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock, watch};
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

use crate::{config::GatewayConfig, Result, GatewayError};

/// Configuration change event
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// Configuration file changed
    FileChanged(PathBuf),
    
    /// Configuration reloaded successfully
    Reloaded(GatewayConfig),
    
    /// Configuration reload failed
    ReloadFailed(String),
    
    /// Configuration validation failed
    ValidationFailed(String),
}

/// Configuration watcher
#[derive(Debug)]
pub struct ConfigWatcher {
    config_path: PathBuf,
    current_config: Arc<RwLock<GatewayConfig>>,
    event_sender: mpsc::Sender<ConfigEvent>,
    config_sender: watch::Sender<GatewayConfig>,
    running: Arc<RwLock<bool>>,
}

impl ConfigWatcher {
    /// Create a new configuration watcher
    pub fn new(
        config_path: PathBuf,
        initial_config: GatewayConfig,
    ) -> (Self, mpsc::Receiver<ConfigEvent>, watch::Receiver<GatewayConfig>) {
        let (event_sender, event_receiver) = mpsc::channel(100);
        let (config_sender, config_receiver) = watch::channel(initial_config.clone());
        
        let watcher = Self {
            config_path,
            current_config: Arc::new(RwLock::new(initial_config)),
            event_sender,
            config_sender,
            running: Arc::new(RwLock::new(false)),
        };
        
        (watcher, event_receiver, config_receiver)
    }
    
    /// Start watching for configuration changes
    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.running.write().await;
            if *running {
                return Ok(());
            }
            *running = true;
        }
        
        info!("Starting configuration watcher for: {}", self.config_path.display());
        
        let config_path = self.config_path.clone();
        let current_config = self.current_config.clone();
        let event_sender = self.event_sender.clone();
        let config_sender = self.config_sender.clone();
        let running = self.running.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5)); // Check every 5 seconds
            let mut last_modified = None;
            
            loop {
                interval.tick().await;
                
                // Check if we should stop
                {
                    let running_guard = running.read().await;
                    if !*running_guard {
                        break;
                    }
                }
                
                // Check if file has been modified
                match tokio::fs::metadata(&config_path).await {
                    Ok(metadata) => {
                        let modified = metadata.modified().ok();
                        
                        if last_modified.is_none() {
                            last_modified = modified;
                            continue;
                        }
                        
                        if let Some(current_modified) = modified {
                            if let Some(last) = last_modified {
                                if current_modified > last {
                                    debug!("Configuration file changed: {}", config_path.display());
                                    
                                    // Send file changed event
                                    if event_sender.send(ConfigEvent::FileChanged(config_path.clone())).await.is_err() {
                                        debug!("Event receiver dropped");
                                        break;
                                    }
                                    
                                    // Try to reload configuration
                                    match Self::reload_config(&config_path, &current_config, &event_sender, &config_sender).await {
                                        Ok(_) => {
                                            info!("Configuration reloaded successfully");
                                        }
                                        Err(e) => {
                                            error!("Failed to reload configuration: {}", e);
                                        }
                                    }
                                    
                                    last_modified = Some(current_modified);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to check config file metadata: {}", e);
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
            
            info!("Configuration watcher stopped");
        });
        
        Ok(())
    }
    
    /// Stop watching for configuration changes
    pub async fn stop(&self) -> Result<()> {
        {
            let mut running = self.running.write().await;
            *running = false;
        }
        
        info!("Stopping configuration watcher");
        Ok(())
    }
    
    /// Manually reload configuration
    pub async fn reload(&self) -> Result<()> {
        Self::reload_config(
            &self.config_path,
            &self.current_config,
            &self.event_sender,
            &self.config_sender,
        ).await
    }
    
    /// Get current configuration
    pub async fn get_config(&self) -> GatewayConfig {
        let config = self.current_config.read().await;
        config.clone()
    }
    
    /// Update configuration programmatically
    pub async fn update_config(&self, new_config: GatewayConfig) -> Result<()> {
        // Validate new configuration
        new_config.validate()?;
        
        // Update current configuration
        {
            let mut config = self.current_config.write().await;
            *config = new_config.clone();
        }
        
        // Send update notification
        if self.config_sender.send(new_config.clone()).is_err() {
            warn!("No config receivers listening");
        }
        
        // Send reload event
        if self.event_sender.send(ConfigEvent::Reloaded(new_config)).await.is_err() {
            debug!("Event receiver dropped");
        }
        
        info!("Configuration updated programmatically");
        Ok(())
    }
    
    /// Internal method to reload configuration from file
    async fn reload_config(
        config_path: &Path,
        current_config: &Arc<RwLock<GatewayConfig>>,
        event_sender: &mpsc::Sender<ConfigEvent>,
        config_sender: &watch::Sender<GatewayConfig>,
    ) -> Result<()> {
        // Load new configuration
        let new_config = match GatewayConfig::from_file(config_path).await {
            Ok(config) => config,
            Err(e) => {
                let error_msg = format!("Failed to load config: {}", e);
                if event_sender.send(ConfigEvent::ReloadFailed(error_msg.clone())).await.is_err() {
                    debug!("Event receiver dropped");
                }
                return Err(e);
            }
        };
        
        // Validate new configuration
        if let Err(e) = new_config.validate() {
            let error_msg = format!("Config validation failed: {}", e);
            if event_sender.send(ConfigEvent::ValidationFailed(error_msg.clone())).await.is_err() {
                debug!("Event receiver dropped");
            }
            return Err(e);
        }
        
        // Update current configuration
        {
            let mut config = current_config.write().await;
            if let Err(e) = config.merge(&new_config) {
                let error_msg = format!("Config merge failed: {}", e);
                if event_sender.send(ConfigEvent::ReloadFailed(error_msg.clone())).await.is_err() {
                    debug!("Event receiver dropped");
                }
                return Err(e);
            }
        }
        
        // Send update notification
        if config_sender.send(new_config.clone()).is_err() {
            warn!("No config receivers listening");
        }
        
        // Send reload event
        if event_sender.send(ConfigEvent::Reloaded(new_config)).await.is_err() {
            debug!("Event receiver dropped");
        }
        
        Ok(())
    }
}

/// Configuration manager that handles hot reload and validation
#[derive(Debug)]
pub struct ConfigManager {
    watcher: ConfigWatcher,
    event_receiver: Arc<RwLock<Option<mpsc::Receiver<ConfigEvent>>>>,
    config_receiver: watch::Receiver<GatewayConfig>,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub async fn new<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        
        // Load initial configuration
        let initial_config = GatewayConfig::from_file(&config_path).await?;
        
        // Create watcher
        let (watcher, event_receiver, config_receiver) = ConfigWatcher::new(config_path, initial_config);
        
        Ok(Self {
            watcher,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            config_receiver,
        })
    }
    
    /// Start the configuration manager
    pub async fn start(&self) -> Result<()> {
        self.watcher.start().await
    }
    
    /// Stop the configuration manager
    pub async fn stop(&self) -> Result<()> {
        self.watcher.stop().await
    }
    
    /// Get current configuration
    pub async fn get_config(&self) -> GatewayConfig {
        self.watcher.get_config().await
    }
    
    /// Get configuration receiver for watching changes
    pub fn config_receiver(&self) -> watch::Receiver<GatewayConfig> {
        self.config_receiver.clone()
    }
    
    /// Take event receiver (can only be called once)
    pub async fn take_event_receiver(&self) -> Option<mpsc::Receiver<ConfigEvent>> {
        let mut receiver = self.event_receiver.write().await;
        receiver.take()
    }
    
    /// Manually reload configuration
    pub async fn reload(&self) -> Result<()> {
        self.watcher.reload().await
    }
    
    /// Update configuration programmatically
    pub async fn update_config(&self, new_config: GatewayConfig) -> Result<()> {
        self.watcher.update_config(new_config).await
    }
}
