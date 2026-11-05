use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, atomic::Ordering},
};

use futures_util::StreamExt;
use rdm_config::ConfigManager;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    disk::DiskManager,
    error::EngineError,
    task::{DownloadSegment, SegmentStatus, Task, TaskStatus},
};

#[derive(Debug, Clone)]
pub struct ResourceProbe {
    pub url: String,
    pub file_name: String,
    pub total_size: u64,
    pub supports_ranges: bool,
    pub content_type: Option<String>,
}

pub struct Engine {
    config: Arc<ConfigManager>,
    disk_manager: Arc<DiskManager>,
    client: reqwest::Client,
    tasks: RwLock<HashMap<Uuid, Arc<Task>>>,
}

impl Engine {
    pub fn new(
        config: Arc<ConfigManager>,
        disk_manager: Arc<DiskManager>,
    ) -> Result<Self, EngineError> {
        let engine_config = config.config();

        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(user_agent_value) =
            reqwest::header::HeaderValue::from_str(&engine_config.user_agent)
        {
            headers.insert(reqwest::header::USER_AGENT, user_agent_value);
        }

        let client = reqwest::Client::builder()
            .tcp_nodelay(true)
            .pool_max_idle_per_host(32)
            .default_headers(headers)
            .user_agent(&engine_config.user_agent)
            .build()?;

        Ok(Self {
            config,
            disk_manager,
            client,
            tasks: RwLock::new(HashMap::new()),
        })
    }

    pub fn config(&self) -> &Arc<ConfigManager> {
        &self.config
    }

    pub fn disk_manager(&self) -> &Arc<DiskManager> {
        &self.disk_manager
    }

    pub async fn probe(&self, url: &str) -> Result<ResourceProbe, EngineError> {
        let response = match self.client.head(url).send().await {
            Ok(res) if res.status().is_success() => res,
            _ => {
                self.client
                    .get(url)
                    .header(reqwest::header::RANGE, "bytes=0-0")
                    .send()
                    .await?
            }
        };

        let status = response.status();
        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(EngineError::HttpStatus(status));
        }

        let headers = response.headers();

        let supports_ranges = headers
            .get(reqwest::header::ACCEPT_RANGES)
            .and_then(|val| val.to_str().ok())
            .is_some_and(|val| val.eq_ignore_ascii_case("bytes"))
            || status == reqwest::StatusCode::PARTIAL_CONTENT;

        let total_size = if let Some(content_range) = headers.get(reqwest::header::CONTENT_RANGE) {
            content_range
                .to_str()
                .ok()
                .and_then(|s| s.rsplit('/').next())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        } else {
            headers
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|val| val.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        };

        let content_type = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|val| val.to_str().ok())
            .map(|s| s.to_string());

        let file_name = headers
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|val| val.to_str().ok())
            .and_then(extract_file_name_from_disposition)
            .unwrap_or_else(|| extract_file_name_from_url(url));

        Ok(ResourceProbe {
            url: url.to_string(),
            file_name,
            total_size,
            supports_ranges,
            content_type,
        })
    }

    pub async fn create_task(
        &self,
        url: &str,
        output_path: Option<PathBuf>,
        connections: Option<usize>,
    ) -> Result<Arc<Task>, EngineError> {
        let probe = self.probe(url).await?;

        let destination_path = match output_path {
            Some(path) => {
                if path.is_dir() {
                    resolve_unique_path(&path.join(&probe.file_name))
                } else {
                    resolve_unique_path(&path)
                }
            }
            None => {
                let target = self.config.config().download_dir.join(&probe.file_name);
                resolve_unique_path(&target)
            }
        };

        let active_connections = if probe.supports_ranges && probe.total_size > 0 {
            connections
                .unwrap_or(self.config.config().default_connections)
                .max(1)
        } else {
            1
        };

        let task_id = Uuid::new_v4();
        let task = Arc::new(Task::new(
            task_id,
            url.to_string(),
            destination_path,
            probe.total_size,
            active_connections,
        ));

        let mut tasks_guard = self.tasks.write().await;
        tasks_guard.insert(task_id, Arc::clone(&task));

        Ok(task)
    }

    pub async fn start_task(&self, task_id: Uuid) -> Result<(), EngineError> {
        let task = self.get_task(task_id).await?;

        let current_status = task.status();
        if current_status == TaskStatus::Downloading {
            return Ok(());
        }

        let cancellation_token = task.renew_cancellation_token();
        task.set_status(TaskStatus::Connecting);

        if task.total_size > 0 {
            self.disk_manager
                .allocate_file(&task.output_path, task.total_size)
                .await?;
        } else {
            let mut file = self.disk_manager.open_for_write(&task.output_path).await?;
            self.disk_manager.flush(&mut file).await?;
        }

        let segment_count = task.connections.max(1);
        let mut segments = Vec::with_capacity(segment_count);

        if segment_count <= 1 || task.total_size == 0 {
            let end_byte = if task.total_size > 0 {
                task.total_size - 1
            } else {
                0
            };
            segments.push(Arc::new(DownloadSegment::new(0, 0, end_byte)));
        } else {
            let chunk_size = task.total_size / segment_count as u64;
            for index in 0..segment_count {
                let start_byte = index as u64 * chunk_size;
                let end_byte = if index == segment_count - 1 {
                    task.total_size - 1
                } else {
                    (index as u64 + 1) * chunk_size - 1
                };
                segments.push(Arc::new(DownloadSegment::new(index, start_byte, end_byte)));
            }
        }

        task.set_segments(segments.clone());
        task.set_status(TaskStatus::Downloading);

        let mut worker_handles = Vec::with_capacity(segments.len());

        for segment in segments {
            let client = self.client.clone();
            let disk_manager = Arc::clone(&self.disk_manager);
            let task_ref = Arc::clone(&task);
            let token = cancellation_token.clone();

            let handle = tokio::spawn(async move {
                download_worker(client, disk_manager, task_ref, segment, token).await
            });

            worker_handles.push(handle);
        }

        let monitor_task = Arc::clone(&task);
        tokio::spawn(async move {
            let mut all_succeeded = true;
            let mut failure_reason = None;

            for handle in worker_handles {
                match handle.await {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => {
                        all_succeeded = false;
                        failure_reason = Some(err.to_string());
                    }
                    Err(join_error) => {
                        all_succeeded = false;
                        failure_reason = Some(join_error.to_string());
                    }
                }
            }

            let final_status = monitor_task.status();
            if final_status == TaskStatus::Paused || final_status == TaskStatus::Cancelled {
                return;
            }

            if all_succeeded {
                monitor_task.set_status(TaskStatus::Completed);
            } else {
                let error_text = match failure_reason {
                    Some(reason) => reason,
                    None => "Download worker failed".to_string(),
                };
                monitor_task.set_status(TaskStatus::Failed(error_text));
            }
        });

        Ok(())
    }

    pub async fn pause_task(&self, task_id: Uuid) -> Result<(), EngineError> {
        let task = self.get_task(task_id).await?;
        task.pause();
        Ok(())
    }

    pub async fn cancel_task(&self, task_id: Uuid) -> Result<(), EngineError> {
        let task = self.get_task(task_id).await?;
        task.cancel();
        Ok(())
    }

    pub async fn get_task(&self, task_id: Uuid) -> Result<Arc<Task>, EngineError> {
        let tasks_guard = self.tasks.read().await;
        tasks_guard
            .get(&task_id)
            .cloned()
            .ok_or(EngineError::TaskNotFound(task_id))
    }

    pub async fn all_tasks(&self) -> Vec<Arc<Task>> {
        let tasks_guard = self.tasks.read().await;
        tasks_guard.values().cloned().collect()
    }
}

async fn download_worker(
    client: reqwest::Client,
    disk_manager: Arc<DiskManager>,
    task: Arc<Task>,
    segment: Arc<DownloadSegment>,
    cancellation_token: CancellationToken,
) -> Result<(), EngineError> {
    segment.set_status(SegmentStatus::Downloading);

    let mut file = disk_manager.open_for_write(&task.output_path).await?;
    let downloaded = segment.downloaded_bytes.load(Ordering::Relaxed);
    let mut current_offset = segment.start_byte + downloaded;

    if segment.end_byte > 0 && current_offset > segment.end_byte {
        segment.set_status(SegmentStatus::Completed);
        return Ok(());
    }

    let mut request_builder = client.get(&task.url);

    if segment.end_byte > 0 {
        request_builder = request_builder.header(
            reqwest::header::RANGE,
            format!("bytes={current_offset}-{}", segment.end_byte),
        );
    }

    let response = request_builder.send().await?;
    let status = response.status();

    if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
        segment.set_status(SegmentStatus::Failed);
        return Err(EngineError::HttpStatus(status));
    }

    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        if cancellation_token.is_cancelled() {
            return Ok(());
        }

        let chunk = chunk_result?;
        disk_manager
            .write_at(&mut file, current_offset, &chunk)
            .await?;

        let bytes_written = chunk.len() as u64;
        current_offset += bytes_written;
        segment
            .downloaded_bytes
            .fetch_add(bytes_written, Ordering::Relaxed);
        task.downloaded_bytes
            .fetch_add(bytes_written, Ordering::Relaxed);
    }

    disk_manager.flush(&mut file).await?;
    segment.set_status(SegmentStatus::Completed);
    Ok(())
}

fn resolve_unique_path(target_path: &Path) -> PathBuf {
    if !target_path.exists() {
        return target_path.to_path_buf();
    }

    let parent = match target_path.parent() {
        Some(p) => p,
        None => Path::new("."),
    };

    let stem = match target_path.file_stem() {
        Some(s) => s.to_str().unwrap_or("download"),
        None => "download",
    };

    let extension = match target_path.extension() {
        Some(e) => e.to_str(),
        None => None,
    };

    let mut counter = 1usize;
    loop {
        let candidate_name = match extension {
            Some(ext) => format!("{stem} ({counter}).{ext}"),
            None => format!("{stem} ({counter})"),
        };
        let candidate_path = parent.join(candidate_name);
        if !candidate_path.exists() {
            return candidate_path;
        }
        counter += 1;
    }
}

fn decode_and_sanitize_filename(raw_name: &str) -> String {
    let replaced_plus = raw_name.replace('+', " ");
    let decoded = percent_encoding::percent_decode_str(&replaced_plus)
        .decode_utf8_lossy()
        .into_owned();

    let sanitized: String = decoded
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();

    let normalized: String = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");

    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        "download.bin".to_string()
    } else {
        trimmed.to_string()
    }
}

fn extract_file_name_from_disposition(disposition: &str) -> Option<String> {
    let mut fallback_filename = None;
    let mut star_filename = None;

    for part in disposition.split(';') {
        let trimmed = part.trim();
        if let Some(val) = trimmed.strip_prefix("filename*=") {
            let clean_val = val.trim_matches('"').trim();
            let encoded_part = if let Some((_, encoded)) = clean_val.split_once("''") {
                encoded
            } else if let Some(last_quote) = clean_val.rfind('\'') {
                &clean_val[last_quote + 1..]
            } else {
                clean_val
            };
            star_filename = Some(decode_and_sanitize_filename(encoded_part));
        } else if let Some(val) = trimmed.strip_prefix("filename=") {
            let clean_val = val.trim_matches('"').trim();
            fallback_filename = Some(decode_and_sanitize_filename(clean_val));
        }
    }

    star_filename.or(fallback_filename)
}

fn extract_file_name_from_url(url_str: &str) -> String {
    if let Ok(parsed_url) = reqwest::Url::parse(url_str)
        && let Some(mut segments) = parsed_url.path_segments()
        && let Some(file_name) = segments.rfind(|s| !s.is_empty())
    {
        return decode_and_sanitize_filename(file_name);
    }
    "download.bin".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_file_name_from_url() {
        assert_eq!(
            extract_file_name_from_url("https://example.com/archive.tar.gz"),
            "archive.tar.gz"
        );
        assert_eq!(
            extract_file_name_from_url("https://example.com/path/to/image.png?query=1"),
            "image.png"
        );
        assert_eq!(extract_file_name_from_url("invalid-url"), "download.bin");
    }

    #[test]
    fn test_extract_file_name_with_percent_encoding() {
        let raw_url = "https://example.com/files/%21++%C2%A74Demon+Red+%C2%A78%5Bx32%5D.zip";
        let extracted = extract_file_name_from_url(raw_url);
        assert_eq!(extracted, "! §4Demon Red §8[x32].zip");
    }

    #[test]
    fn test_extract_file_name_from_disposition() {
        assert_eq!(
            extract_file_name_from_disposition("attachment; filename=\"package.zip\""),
            Some("package.zip".to_string())
        );
        assert_eq!(
            extract_file_name_from_disposition("inline; filename=document.pdf"),
            Some("document.pdf".to_string())
        );
        assert_eq!(extract_file_name_from_disposition("inline"), None);
    }

    #[test]
    fn test_extract_file_name_from_disposition_rfc5987() {
        let disposition = "attachment; filename=\"fallback.zip\"; filename*=UTF-8''%21++%C2%A74Demon+Red+%C2%A78%5Bx32%5D.zip";
        let extracted = extract_file_name_from_disposition(disposition);
        assert_eq!(extracted, Some("! §4Demon Red §8[x32].zip".to_string()));
    }

    #[test]
    fn test_resolve_unique_path() {
        let temp_dir = std::env::temp_dir().join(format!("rdm_unique_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let base_file = temp_dir.join("sample.zip");

        assert_eq!(resolve_unique_path(&base_file), base_file);

        let _ = std::fs::write(&base_file, b"test");
        let first_duplicate = resolve_unique_path(&base_file);
        assert_eq!(first_duplicate, temp_dir.join("sample (1).zip"));

        let _ = std::fs::write(&first_duplicate, b"test");
        let second_duplicate = resolve_unique_path(&base_file);
        assert_eq!(second_duplicate, temp_dir.join("sample (2).zip"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_engine_initialization_reads_config_header() {
        let config = Arc::new(ConfigManager::new());
        let disk_manager = Arc::new(DiskManager::new());
        let engine = Engine::new(Arc::clone(&config), disk_manager);

        assert!(engine.is_ok());
        if let Ok(engine) = engine {
            assert_eq!(
                engine.config().config().user_agent,
                config.config().user_agent
            );
        }
    }
}
