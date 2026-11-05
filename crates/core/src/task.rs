use std::{path::PathBuf, sync::atomic::AtomicU64};

use uuid::Uuid;

#[derive(Debug)]
pub struct Task {
    pub id: Uuid,
    pub url: String,
    pub output_path: PathBuf,
    pub total_size: u64,
    pub downloaded_bytes: AtomicU64,
    pub connections: usize,
}

impl Task {
    pub fn new(
        id: Uuid,
        url: String,
        output_path: PathBuf,
        total_size: u64,
        connections: usize,
    ) -> Self {
        Self {
            id,
            url,
            output_path,
            total_size,
            downloaded_bytes: AtomicU64::new(0),
            connections,
        }
    }
}
