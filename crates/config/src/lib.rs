use std::sync::Arc;

use crate::engine::EngineConfig;

pub mod engine;

#[derive(Clone, Default)]
pub struct ConfigManager {
    engine_config: Arc<EngineConfig>,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_config(engine_config: EngineConfig) -> Self {
        Self {
            engine_config: Arc::new(engine_config),
        }
    }

    pub fn config(&self) -> &Arc<EngineConfig> {
        &self.engine_config
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
}
