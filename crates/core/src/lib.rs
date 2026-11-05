use std::sync::{Arc, RwLock};

use crate::{disk::DiskManager, engine::Engine, error::EngineError};
use rdm_config::ConfigManager;

pub mod disk;
pub mod engine;
pub mod error;
pub mod task;

#[derive(Clone)]
pub struct Core {
    disk_manager: Arc<DiskManager>,
    engine: Arc<Engine>,
    config: Arc<ConfigManager>,
}

impl Core {
    pub fn new() -> Result<Self, EngineError> {
        let disk_manager = Arc::new(DiskManager::new());
        let engine = Arc::new(Engine::new(Arc::clone(&disk_manager))?);

        Ok((Self {
            disk_manager,
            config: Arc::new(ConfigManager::new()),
            engine,
        }))
    }

    #[inline]
    fn disk(&self) -> &DiskManager {
        &self.disk_manager
    }

    #[inline]
    fn engine(&self) -> &Engine {
        &self.engine
    }

    #[inline]
    fn config(&self) -> &ConfigManager{
        &self.config
    }
}
