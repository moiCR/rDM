use std::{
    path::{Path, PathBuf},
    sync::RwLock,
};
use thiserror::Error;

pub use crate::engine::{EngineConfig, get_user_agent};

pub mod engine;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("I/O error while reading or writing config: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub struct ConfigManager {
    engine_config: RwLock<EngineConfig>,
    config_file_path: PathBuf,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigManager {
    pub fn new() -> Self {
        let default_config = EngineConfig::default();
        let config_file_path = default_config.state_dir.join("settings.json");
        let initial_config = Self::load_from_path(&config_file_path).unwrap_or(default_config);

        Self {
            engine_config: RwLock::new(initial_config),
            config_file_path,
        }
    }

    pub fn from_config(engine_config: EngineConfig) -> Self {
        let config_file_path = engine_config.state_dir.join("settings.json");
        Self {
            engine_config: RwLock::new(engine_config),
            config_file_path,
        }
    }

    pub fn config(&self) -> EngineConfig {
        match self.engine_config.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn config_path(&self) -> &Path {
        &self.config_file_path
    }

    pub fn update_config<F>(&self, update_fn: F) -> Result<EngineConfig, ConfigError>
    where
        F: FnOnce(&mut EngineConfig),
    {
        let mut guard = match self.engine_config.write() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        update_fn(&mut guard);
        let updated = guard.clone();
        drop(guard);

        self.save()?;
        Ok(updated)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let current = self.config();
        if let Some(parent) = self.config_file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_string_pretty(&current)?;
        std::fs::write(&self.config_file_path, serialized)?;
        Ok(())
    }

    pub fn load_from_path(path: &Path) -> Result<EngineConfig, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Config file does not exist",
            )));
        }
        let raw = std::fs::read_to_string(path)?;
        let parsed = serde_json::from_str::<EngineConfig>(&raw)?;
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_manager_default_download_dir() {
        let manager = ConfigManager::new();
        let config = manager.config();

        assert_eq!(config.default_connections, 8);
        assert!(!config.download_dir.as_os_str().is_empty());
    }

    #[test]
    fn test_config_manager_custom_config() {
        let custom =
            EngineConfig::default().with_download_dir(std::path::PathBuf::from("/custom/path"));
        let manager = ConfigManager::from_config(custom);

        assert_eq!(
            manager.config().download_dir,
            std::path::PathBuf::from("/custom/path")
        );
    }

    #[test]
    fn test_config_manager_save_and_update() -> Result<(), Box<dyn std::error::Error>> {
        let test_nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("rdm_cfg_{test_nonce}"));
        let custom = EngineConfig {
            state_dir: temp_dir.clone(),
            ..EngineConfig::default()
        };
        let manager = ConfigManager::from_config(custom);

        manager.update_config(|c| {
            c.default_connections = 16;
            c.max_speed_bytes_per_sec = 1024 * 1024 * 5;
        })?;

        let loaded = manager.config();
        assert_eq!(loaded.default_connections, 16);
        assert_eq!(loaded.max_speed_bytes_per_sec, 5242880);

        let from_disk = ConfigManager::load_from_path(manager.config_path())?;
        assert_eq!(from_disk.default_connections, 16);
        assert_eq!(from_disk.max_speed_bytes_per_sec, 5242880);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
