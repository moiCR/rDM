#[cfg(windows)]
use std::path::PathBuf;
use std::{io::Read, path::Path};

use md5::Md5;
use sha2::{Digest, Sha256};
use tokio::{
    fs::{File, OpenOptions, create_dir_all},
    io::{AsyncSeekExt, AsyncWriteExt, SeekFrom},
};

use crate::{
    error::EngineError,
    task::{ChecksumAlgorithm, ExpectedChecksum},
};

#[derive(Default)]
pub struct DiskManager {}

impl DiskManager {
    pub async fn ensure_directory(&self, path: &Path) -> Result<(), EngineError> {
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    pub fn new() -> Self {
        Self {}
    }

    pub fn get_available_space(path: &Path) -> Result<u64, EngineError> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;

            let mut target_dir = if path.is_dir() {
                path.to_path_buf()
            } else {
                path.parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."))
            };

            while !target_dir.exists() {
                if let Some(parent) = target_dir.parent() {
                    target_dir = parent.to_path_buf();
                } else {
                    break;
                }
            }

            let mut wide: Vec<u16> = target_dir.as_os_str().encode_wide().collect();
            wide.push(0);

            let mut free_bytes_available: u64 = 0;
            let mut total_number_of_bytes: u64 = 0;
            let mut total_number_of_free_bytes: u64 = 0;

            unsafe extern "system" {
                fn GetDiskFreeSpaceExW(
                    lpDirectoryName: *const u16,
                    lpFreeBytesAvailableToCaller: *mut u64,
                    lpTotalNumberOfBytes: *mut u64,
                    lpTotalNumberOfFreeBytes: *mut u64,
                ) -> i32;
            }

            let success = unsafe {
                GetDiskFreeSpaceExW(
                    wide.as_ptr(),
                    &mut free_bytes_available,
                    &mut total_number_of_bytes,
                    &mut total_number_of_free_bytes,
                )
            };

            if success != 0 {
                Ok(free_bytes_available)
            } else {
                Err(EngineError::Io(std::io::Error::last_os_error()))
            }
        }

        #[cfg(target_os = "linux")]
        {
            let mut target = path.to_path_buf();
            loop {
                match rustix::fs::statvfs(&target) {
                    Ok(space) => {
                        return Ok(space.f_bavail.saturating_mul(space.f_frsize));
                    }
                    Err(rustix::io::Errno::NOENT) => {
                        if !target.pop() || target.as_os_str().is_empty() {
                            target = std::env::current_dir()?;
                        }
                    }
                    Err(error) => return Err(std::io::Error::from(error).into()),
                }
            }
        }

        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = path;
            Ok(u64::MAX)
        }
    }

    pub fn check_available_space(path: &Path, required_bytes: u64) -> Result<(), EngineError> {
        let available = Self::get_available_space(path)?;
        if available < required_bytes {
            return Err(EngineError::InsufficientDiskSpace {
                required: required_bytes,
                available,
            });
        }
        Ok(())
    }

    pub async fn allocate_file(&self, path: &Path, size: u64) -> Result<File, EngineError> {
        if let Some(parent) = path.parent() {
            create_dir_all(parent).await?;
        }

        let existing_len = if let Ok(metadata) = tokio::fs::metadata(path).await {
            metadata.len()
        } else {
            0
        };

        let additional_required = size.saturating_sub(existing_len);
        if additional_required > 0 {
            let target = path.to_path_buf();
            tokio::task::spawn_blocking(move || {
                Self::check_available_space(&target, additional_required)
            })
            .await??;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
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

    pub async fn compute_checksum(
        &self,
        path: &Path,
        algorithm: ChecksumAlgorithm,
    ) -> Result<String, EngineError> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let mut file = std::fs::File::open(&path)?;
            match algorithm {
                ChecksumAlgorithm::Sha256 => hash_file::<Sha256>(&mut file),
                ChecksumAlgorithm::Md5 => hash_file::<Md5>(&mut file),
            }
        })
        .await?
    }

    pub async fn verify_checksum(
        &self,
        path: &Path,
        expected: &ExpectedChecksum,
    ) -> Result<(), EngineError> {
        let actual = self.compute_checksum(path, expected.algorithm).await?;
        if actual != expected.value {
            return Err(EngineError::ChecksumMismatch {
                algorithm: expected.algorithm.as_str().to_string(),
                expected: expected.value.clone(),
                actual,
            });
        }
        Ok(())
    }
}

fn hash_file<D: Digest>(file: &mut std::fs::File) -> Result<String, EngineError> {
    let mut hasher = D::new();
    let mut buffer = [0u8; 65_536];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
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

    #[test]
    fn test_disk_space_check() {
        let temp_dir = std::env::temp_dir();
        let space = DiskManager::get_available_space(&temp_dir);
        assert!(space.is_ok());
        assert!(space.unwrap() > 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn missing_destination_uses_existing_ancestor() -> Result<(), EngineError> {
        let parent = std::env::temp_dir();
        let destination = parent
            .join(uuid::Uuid::new_v4().to_string())
            .join("download.bin");
        let available = DiskManager::get_available_space(&destination)?;
        assert!(available > 0);
        assert!(available < u64::MAX);
        assert!(matches!(
            DiskManager::check_available_space(&destination, u64::MAX),
            Err(EngineError::InsufficientDiskSpace { .. })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn verify_checksum_validates_sha256_and_md5() -> Result<(), Box<dyn std::error::Error>> {
        let disk_manager = DiskManager::new();
        let temp_dir = std::env::temp_dir().join(format!("rdm_hash_test_{}", uuid::Uuid::new_v4()));
        disk_manager.ensure_directory(&temp_dir).await?;
        let file_path = temp_dir.join("payload.txt");
        tokio::fs::write(&file_path, b"abc").await?;

        let sha = ExpectedChecksum::parse(
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )?;
        disk_manager.verify_checksum(&file_path, &sha).await?;

        let md5 = ExpectedChecksum::parse("md5:900150983cd24fb0d6963f7d28e17f72")?;
        disk_manager.verify_checksum(&file_path, &md5).await?;

        let wrong = ExpectedChecksum::parse("md5:00000000000000000000000000000000")?;
        let err = disk_manager.verify_checksum(&file_path, &wrong).await;
        assert!(matches!(err, Err(EngineError::ChecksumMismatch { .. })));

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        Ok(())
    }
}
