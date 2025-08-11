//! Configuration manager with hot reload support

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock, watch};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::{GatewayConfig, Result, GatewayError};

/// Configuration change event
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// Configuration was reloaded
    Reloaded(Arc<GatewayConfig>),
    
    /// Configuration reload failed
    ReloadFailed(String),
    
    /// Configuration file was modified
    FileModified(PathBuf),
}

/// Configuration manager
#[derive(Debug)]
pub struct ConfigManager {
    /// Current configuration
    config: Arc<RwLock<GatewayConfig>>,
    
    /// Configuration file path
    config_path: PathBuf,
    
    /// Configuration change sender
    change_sender: broadcast::Sender<ConfigEvent>,
    
    /// Watch for configuration updates
    config_watch_sender: watch::Sender<Arc<GatewayConfig>>,
    
    /// Enable hot reload
    hot_reload_enabled: bool,
    
    /// Hot reload check interval
    hot_reload_interval: Duration,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub async fn new<P: AsRef<Path>>(
        config_path: P,
        hot_reload_enabled: bool,
        hot_reload_interval: Duration,
    ) -> Result<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        
        // Load initial configuration
        let initial_config = GatewayConfig::from_file(&config_path).await?;
        let config = Arc::new(RwLock::new(initial_config.clone()));
        
        // Create channels
        let (change_sender, _) = broadcast::channel(100);
        let (config_watch_sender, _) = watch::channel(Arc::new(initial_config));
        
        let manager = Self {
            config,
            config_path,
            change_sender,
            config_watch_sender,
            hot_reload_enabled,
            hot_reload_interval,
        };
        
        info!("Configuration manager initialized with file: {}", manager.config_path.display());
        
        Ok(manager)
    }
    
    /// Get current configuration
    pub async fn get_config(&self) -> Arc<GatewayConfig> {
        Arc::new(self.config.read().await.clone())
    }
    
    /// Subscribe to configuration changes
    pub fn subscribe(&self) -> broadcast::Receiver<ConfigEvent> {
        self.change_sender.subscribe()
    }
    
    /// Watch for configuration updates
    pub fn watch(&self) -> watch::Receiver<Arc<GatewayConfig>> {
        self.config_watch_sender.subscribe()
    }
    
    /// Reload configuration from file
    pub async fn reload(&self) -> Result<()> {
        info!("Reloading configuration from: {}", self.config_path.display());
        
        match GatewayConfig::from_file(&self.config_path).await {
            Ok(new_config) => {
                // Update current configuration
                {
                    let mut config = self.config.write().await;
                    if let Err(e) = config.merge(&new_config) {
                        error!("Failed to merge configuration: {}", e);
                        let _ = self.change_sender.send(ConfigEvent::ReloadFailed(e.to_string()));
                        return Err(e);
                    }
                    *config = new_config.clone();
                }
                
                // Notify watchers
                let config_arc = Arc::new(new_config);
                if let Err(e) = self.config_watch_sender.send(config_arc.clone()) {
                    warn!("Failed to notify config watchers: {}", e);
                }
                
                // Send reload event
                if let Err(e) = self.change_sender.send(ConfigEvent::Reloaded(config_arc)) {
                    warn!("Failed to send reload event: {}", e);
                }
                
                info!("Configuration reloaded successfully");
                Ok(())
            }
            Err(e) => {
                error!("Failed to reload configuration: {}", e);
                let _ = self.change_sender.send(ConfigEvent::ReloadFailed(e.to_string()));
                Err(e)
            }
        }
    }
    
    /// Update configuration programmatically
    pub async fn update_config(&self, new_config: GatewayConfig) -> Result<()> {
        info!("Updating configuration programmatically");
        
        // Validate new configuration
        new_config.validate()?;
        
        // Update current configuration
        {
            let mut config = self.config.write().await;
            *config = new_config.clone();
        }
        
        // Notify watchers
        let config_arc = Arc::new(new_config);
        if let Err(e) = self.config_watch_sender.send(config_arc.clone()) {
            warn!("Failed to notify config watchers: {}", e);
        }
        
        // Send reload event
        if let Err(e) = self.change_sender.send(ConfigEvent::Reloaded(config_arc)) {
            warn!("Failed to send reload event: {}", e);
        }
        
        info!("Configuration updated successfully");
        Ok(())
    }
    
    /// Start hot reload monitoring
    pub async fn start_hot_reload(&self) -> Result<()> {
        if !self.hot_reload_enabled {
            debug!("Hot reload is disabled");
            return Ok();
        }
        
        info!("Starting hot reload monitoring");
        
        let config_path = self.config_path.clone();
        let change_sender = self.change_sender.clone();
        let manager = self.clone_for_task();
        
        tokio::spawn(async move {
            let mut interval_timer = interval(manager.hot_reload_interval);
            let mut last_modified = get_file_modified_time(&config_path).await.unwrap_or(0);
            
            loop {
                interval_timer.tick().await;
                
                match get_file_modified_time(&config_path).await {
                    Ok(modified_time) => {
                        if modified_time > last_modified {
                            debug!("Configuration file modified, reloading...");
                            last_modified = modified_time;
                            
                            // Send file modified event
                            if let Err(e) = change_sender.send(ConfigEvent::FileModified(config_path.clone())) {
                                warn!("Failed to send file modified event: {}", e);
                            }
                            
                            // Reload configuration
                            if let Err(e) = manager.reload().await {
                                error!("Hot reload failed: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to check file modification time: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Clone for use in async tasks
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            config_path: self.config_path.clone(),
            change_sender: self.change_sender.clone(),
            config_watch_sender: self.config_watch_sender.clone(),
            hot_reload_enabled: self.hot_reload_enabled,
            hot_reload_interval: self.hot_reload_interval,
        }
    }
    
    /// Save current configuration to file
    pub async fn save(&self) -> Result<()> {
        let config = self.config.read().await;
        config.to_file(&self.config_path).await?;
        info!("Configuration saved to: {}", self.config_path.display());
        Ok(())
    }
    
    /// Get configuration file path
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }
    
    /// Check if hot reload is enabled
    pub fn is_hot_reload_enabled(&self) -> bool {
        self.hot_reload_enabled
    }
}

impl Clone for ConfigManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            config_path: self.config_path.clone(),
            change_sender: self.change_sender.clone(),
            config_watch_sender: self.config_watch_sender.clone(),
            hot_reload_enabled: self.hot_reload_enabled,
            hot_reload_interval: self.hot_reload_interval,
        }
    }
}

/// Get file modification time
async fn get_file_modified_time(path: &Path) -> Result<u64> {
    let metadata = tokio::fs::metadata(path).await
        .map_err(|e| GatewayError::config(format!("Failed to get file metadata: {}", e)))?;
    
    let modified = metadata.modified()
        .map_err(|e| GatewayError::config(format!("Failed to get modification time: {}", e)))?;
    
    let duration = modified.duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| GatewayError::config(format!("Invalid modification time: {}", e)))?;
    
    Ok(duration.as_secs())
}

/// Configuration builder for easier setup
#[derive(Debug)]
pub struct ConfigManagerBuilder {
    config_path: Option<PathBuf>,
    hot_reload_enabled: bool,
    hot_reload_interval: Duration,
}

impl ConfigManagerBuilder {
    /// Create a new configuration manager builder
    pub fn new() -> Self {
        Self {
            config_path: None,
            hot_reload_enabled: true,
            hot_reload_interval: Duration::from_secs(5),
        }
    }
    
    /// Set configuration file path
    pub fn config_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.config_path = Some(path.as_ref().to_path_buf());
        self
    }
    
    /// Enable or disable hot reload
    pub fn hot_reload(mut self, enabled: bool) -> Self {
        self.hot_reload_enabled = enabled;
        self
    }
    
    /// Set hot reload check interval
    pub fn hot_reload_interval(mut self, interval: Duration) -> Self {
        self.hot_reload_interval = interval;
        self
    }
    
    /// Build the configuration manager
    pub async fn build(self) -> Result<ConfigManager> {
        let config_path = self.config_path
            .ok_or_else(|| GatewayError::config("Configuration file path is required"))?;
        
        ConfigManager::new(config_path, self.hot_reload_enabled, self.hot_reload_interval).await
    }
}

impl Default for ConfigManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
