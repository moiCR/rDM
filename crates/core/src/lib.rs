use std::sync::Arc;

pub use crate::{disk::DiskManager, engine::Engine, error::EngineError};
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
        let config = Arc::new(ConfigManager::new());
        let disk_manager = Arc::new(DiskManager::new());
        let engine = Arc::new(Engine::new(Arc::clone(&config), Arc::clone(&disk_manager))?);

        Ok(Self {
            disk_manager,
            config,
            engine,
        })
    }

    #[inline]
    pub fn disk(&self) -> &Arc<DiskManager> {
        &self.disk_manager
    }

    #[inline]
    pub fn engine(&self) -> &Arc<Engine> {
        &self.engine
    }

    #[inline]
    pub fn config(&self) -> &Arc<ConfigManager> {
        &self.config
    }
}
