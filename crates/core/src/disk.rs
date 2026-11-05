use std::path::Path;

use tokio::{
    fs::{File, OpenOptions, create_dir_all},
    io::{AsyncSeekExt, AsyncWriteExt, SeekFrom},
};

use crate::error::EngineError;

#[derive(Default)]
pub struct DiskManager {}

impl DiskManager {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn allocate_file(&self, path: &Path, size: u64) -> Result<File, EngineError> {
        if let Some(parent) = path.parent() {
            create_dir_all(parent).await?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .await?;

        file.set_len(size).await?;
        Ok(file)
    }

    pub async fn open_for_write(&self, path: &Path) -> Result<File, EngineError> {
        if let Some(parent) = path.parent() {
            create_dir_all(parent).await?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .await?;

        Ok(file)
    }

    pub async fn write_at(
        &self,
        file: &mut File,
        offset: u64,
        data: &[u8],
    ) -> Result<(), EngineError> {
        file.seek(SeekFrom::Start(offset)).await?;
        file.write_all(data).await?;
        Ok(())
    }

    pub async fn flush(&self, file: &mut File) -> Result<(), EngineError> {
        file.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn test_allocate_and_write_at() -> Result<(), Box<dyn std::error::Error>> {
        let disk_manager = DiskManager::new();
        let temp_dir = std::env::temp_dir().join(format!("rdm_test_{}", uuid::Uuid::new_v4()));
        let file_path = temp_dir.join("test_file.bin");

        let mut file = disk_manager.allocate_file(&file_path, 100).await?;
        assert_eq!(file.metadata().await?.len(), 100);

        disk_manager.write_at(&mut file, 10, b"hello").await?;
        disk_manager.write_at(&mut file, 20, b"world").await?;
        disk_manager.flush(&mut file).await?;
        drop(file);

        let mut read_file = OpenOptions::new().read(true).open(&file_path).await?;
        let mut buffer = vec![0u8; 100];
        read_file.read_exact(&mut buffer).await?;

        assert_eq!(&buffer[10..15], b"hello");
        assert_eq!(&buffer[20..25], b"world");

        let _ = tokio::fs::remove_file(&file_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_open_for_write_creates_file_if_not_exists()
    -> Result<(), Box<dyn std::error::Error>> {
        let disk_manager = DiskManager::new();
        let temp_dir =
            std::env::temp_dir().join(format!("rdm_test_create_{}", uuid::Uuid::new_v4()));
        let file_path = temp_dir.join("new_file.bin");

        let mut file = disk_manager.open_for_write(&file_path).await?;
        disk_manager.write_at(&mut file, 0, b"data").await?;
        disk_manager.flush(&mut file).await?;
        drop(file);

        assert!(tokio::fs::try_exists(&file_path).await?);

        let _ = tokio::fs::remove_file(&file_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;

        Ok(())
    }
}
