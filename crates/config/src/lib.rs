use std::sync::Arc;

use crate::engine::EngineConfig;

pub mod engine;

#[derive(Clone)]
pub struct ConfigManager {
    engine_config: Arc<EngineConfig>,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            engine_config: Arc::new(EngineConfig::default()),
        }
    }

    pub fn config(&self) -> &Arc<EngineConfig> {
        &self.engine_config
    }
}
