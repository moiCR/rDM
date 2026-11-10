use std::sync::Arc;

pub use crate::{
    disk::DiskManager, engine::Engine, error::EngineError, server::InterceptedDownload,
};
use rdm_config::ConfigManager;
use tokio::sync::broadcast;

pub mod disk;
pub mod engine;
pub mod error;
pub mod server;
pub mod task;
pub mod updater;

#[derive(Clone)]
pub struct Core {
    disk_manager: Arc<DiskManager>,
    engine: Arc<Engine>,
    config: Arc<ConfigManager>,
    intercept_tx: broadcast::Sender<InterceptedDownload>,
}

impl Core {
    pub fn new() -> Result<Self, EngineError> {
        let config = Arc::new(ConfigManager::new());
        let disk_manager = Arc::new(DiskManager::new());
        let engine = Arc::new(Engine::new(Arc::clone(&config), Arc::clone(&disk_manager))?);
        let (intercept_tx, _) = broadcast::channel(32);

        Ok(Self {
            disk_manager,
            config,
            engine,
            intercept_tx,
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

    pub fn notify_intercepted(&self, item: InterceptedDownload) {
        let _ = self.intercept_tx.send(item);
    }

    pub fn subscribe_intercepted(&self) -> broadcast::Receiver<InterceptedDownload> {
        self.intercept_tx.subscribe()
    }
}
