use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use crate::{disk::DiskManager, error::EngineError, task::Task};

#[derive(Debug)]
pub struct DownloadTask {
    pub id: u64,
    pub url: String,
    pub total_size: u64,
}

pub struct Engine {
    disk_manager: Arc<DiskManager>,
    client: reqwest::Client,
    tasks: RwLock<HashMap<u64, Arc<Task>>>,
}

impl Engine {
    pub fn new(disk_manager: Arc<DiskManager>) -> Result<Self, EngineError> {
        let client = reqwest::Client::builder()
            .tcp_nodelay(true)
            .pool_max_idle_per_host(32)
            .user_agent(
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) rDM/0.1.0",
            )
            .build()?;

        Ok(Self {
            disk_manager,
            client,
            tasks: RwLock::new(HashMap::new()),
        })
    }
}
