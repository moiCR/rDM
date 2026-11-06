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
    tasks: Arc<RwLock<HashMap<Uuid, Arc<Task>>>>,
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

        let mut initial_tasks = HashMap::new();
        let state_file = engine_config.state_dir.join("tasks.json");
        if let Ok(content) = std::fs::read_to_string(&state_file)
            && let Ok(persisted_tasks) =
                serde_json::from_str::<Vec<crate::task::PersistedTask>>(&content)
        {
            for persisted in persisted_tasks {
                let id = persisted.id;
                initial_tasks.insert(id, Arc::new(Task::from_persisted(persisted)));
            }
        }

        Ok(Self {
            config,
            disk_manager,
            client,
            tasks: Arc::new(RwLock::new(initial_tasks)),
        })
    }

    pub fn config(&self) -> &Arc<ConfigManager> {
        &self.config
    }

    pub fn disk_manager(&self) -> &Arc<DiskManager> {
        &self.disk_manager
    }

    pub async fn save_state(&self) -> Result<(), EngineError> {
        let state_dir = &self.config.config().state_dir;
        persist_tasks_to_disk(state_dir, &self.tasks).await;
        Ok(())
    }

    pub async fn load_state(&self) -> Result<(), EngineError> {
        let state_file = self.config.config().state_dir.join("tasks.json");
        if !tokio::fs::try_exists(&state_file).await.unwrap_or(false) {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&state_file).await?;
        let persisted_tasks: Vec<crate::task::PersistedTask> = serde_json::from_str(&content)?;

        let mut tasks_guard = self.tasks.write().await;
        for persisted in persisted_tasks {
            let task_id = persisted.id;
            let task = Arc::new(Task::from_persisted(persisted));
            tasks_guard.insert(task_id, task);
        }
        Ok(())
    }

    pub async fn probe(
        &self,
        url: &str,
        referer: Option<&str>,
    ) -> Result<ResourceProbe, EngineError> {
        let resolved_referer = resolve_referer(url, referer);

        let mut head_builder = self.client.head(url);
        if let Some(ref r) = resolved_referer
            && let Ok(val) = reqwest::header::HeaderValue::from_str(r)
        {
            head_builder = head_builder.header(reqwest::header::REFERER, val);
        }

        let response = match head_builder.send().await {
            Ok(res) if res.status().is_success() => res,
            _ => {
                let mut get_builder = self
                    .client
                    .get(url)
                    .header(reqwest::header::RANGE, "bytes=0-0");
                if let Some(ref r) = resolved_referer
                    && let Ok(val) = reqwest::header::HeaderValue::from_str(r)
                {
                    get_builder = get_builder.header(reqwest::header::REFERER, val);
                }
                get_builder.send().await?
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
        referer: Option<String>,
    ) -> Result<Arc<Task>, EngineError> {
        let probe = self.probe(url, referer.as_deref()).await?;

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
            referer,
        ));

        {
            let mut tasks_guard = self.tasks.write().await;
            tasks_guard.insert(task_id, Arc::clone(&task));
        }

        let _ = self.save_state().await;

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

        let mut segments = task.segments();
        if segments.is_empty() {
            let segment_count = task.connections.max(1);
            let mut new_segments = Vec::with_capacity(segment_count);

            if segment_count <= 1 || task.total_size == 0 {
                let end_byte = if task.total_size > 0 {
                    task.total_size - 1
                } else {
                    0
                };
                new_segments.push(Arc::new(DownloadSegment::new(0, 0, end_byte)));
            } else {
                let chunk_size = task.total_size / segment_count as u64;
                for index in 0..segment_count {
                    let start_byte = index as u64 * chunk_size;
                    let end_byte = if index == segment_count - 1 {
                        task.total_size - 1
                    } else {
                        (index as u64 + 1) * chunk_size - 1
                    };
                    new_segments.push(Arc::new(DownloadSegment::new(index, start_byte, end_byte)));
                }
            }

            task.set_segments(new_segments.clone());
            segments = new_segments;
        }

        for segment in &segments {
            if segment.end_byte > 0
                && segment.downloaded_bytes.load(Ordering::Relaxed) < segment.total_bytes()
                && segment.status() == SegmentStatus::Completed
            {
                segment.set_status(SegmentStatus::Pending);
            }
        }

        if !segments.is_empty() {
            let synced_downloaded: u64 = segments
                .iter()
                .map(|s| s.downloaded_bytes.load(Ordering::Relaxed))
                .sum();
            task.downloaded_bytes
                .store(synced_downloaded, Ordering::Relaxed);
        }

        task.set_status(TaskStatus::Downloading);
        let _ = self.save_state().await;

        let mut worker_handles = Vec::with_capacity(segments.len());

        for segment in segments {
            let is_complete = if segment.end_byte > 0 {
                segment.downloaded_bytes.load(Ordering::Relaxed) >= segment.total_bytes()
                    && segment.status() == SegmentStatus::Completed
            } else {
                segment.status() == SegmentStatus::Completed
            };
            if is_complete {
                continue;
            }

            let client = self.client.clone();
            let disk_manager = Arc::clone(&self.disk_manager);
            let task_ref = Arc::clone(&task);
            let token = cancellation_token.clone();

            let handle = tokio::spawn(async move {
                download_worker(client, disk_manager, task_ref, segment, token).await
            });

            worker_handles.push(handle);
        }

        if worker_handles.is_empty() {
            let downloaded = task.downloaded_bytes.load(Ordering::Relaxed);
            if task.total_size == 0 || downloaded >= task.total_size {
                task.set_status(TaskStatus::Completed);
                let _ = self.save_state().await;
                return Ok(());
            }
        }

        let monitor_task = Arc::clone(&task);
        let monitor_state_dir = self.config.config().state_dir.clone();
        let monitor_tasks = Arc::clone(&self.tasks);

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
                persist_tasks_to_disk(&monitor_state_dir, &monitor_tasks).await;
                return;
            }

            let downloaded = monitor_task.downloaded_bytes.load(Ordering::Relaxed);
            let total = monitor_task.total_size;
            let segments_complete = monitor_task.segments().iter().all(|s| {
                if s.end_byte > 0 {
                    s.downloaded_bytes.load(Ordering::Relaxed) >= s.total_bytes()
                        && s.status() == SegmentStatus::Completed
                } else {
                    s.status() == SegmentStatus::Completed
                }
            });
            let size_complete = total == 0 || downloaded >= total;

            if all_succeeded && segments_complete && size_complete {
                monitor_task.set_status(TaskStatus::Completed);
            } else {
                let error_text = match failure_reason {
                    Some(reason) => reason,
                    None => "Download incomplete".to_string(),
                };
                monitor_task.set_status(TaskStatus::Failed(error_text));
            }

            persist_tasks_to_disk(&monitor_state_dir, &monitor_tasks).await;
        });

        Ok(())
    }

    pub async fn resume_task(&self, task_id: Uuid) -> Result<(), EngineError> {
        self.start_task(task_id).await
    }

    pub async fn pause_task(&self, task_id: Uuid) -> Result<(), EngineError> {
        let task = self.get_task(task_id).await?;
        task.pause();
        let _ = self.save_state().await;
        Ok(())
    }

    pub async fn cancel_task(&self, task_id: Uuid) -> Result<(), EngineError> {
        let task = self.get_task(task_id).await?;
        task.cancel();
        let _ = self.save_state().await;
        Ok(())
    }

    pub async fn delete_task(&self, task_id: Uuid, delete_file: bool) -> Result<(), EngineError> {
        let task = {
            let mut tasks_guard = self.tasks.write().await;
            tasks_guard
                .remove(&task_id)
                .ok_or(EngineError::TaskNotFound(task_id))?
        };

        task.cancel();

        if delete_file
            && tokio::fs::try_exists(&task.output_path)
                .await
                .unwrap_or(false)
        {
            let _ = tokio::fs::remove_file(&task.output_path).await;
        }

        let _ = self.save_state().await;
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

async fn persist_tasks_to_disk(state_dir: &Path, tasks_map: &RwLock<HashMap<Uuid, Arc<Task>>>) {
    if let Ok(()) = tokio::fs::create_dir_all(state_dir).await {
        let tasks_guard = tasks_map.read().await;
        let persisted_tasks: Vec<crate::task::PersistedTask> =
            tasks_guard.values().map(|t| t.to_persisted()).collect();
        if let Ok(json) = serde_json::to_string_pretty(&persisted_tasks) {
            let _ = tokio::fs::write(state_dir.join("tasks.json"), json).await;
        }
    }
}

async fn download_worker(
    client: reqwest::Client,
    disk_manager: Arc<DiskManager>,
    task: Arc<Task>,
    segment: Arc<DownloadSegment>,
    cancellation_token: CancellationToken,
) -> Result<(), EngineError> {
    let max_retries = 8;
    let mut attempt = 0;

    loop {
        if cancellation_token.is_cancelled() {
            return Ok(());
        }

        let bytes_before = segment.downloaded_bytes.load(Ordering::Relaxed);
        let result =
            download_worker_stream(&client, &disk_manager, &task, &segment, &cancellation_token)
                .await;

        match result {
            Ok(()) => return Ok(()),
            Err(err) => {
                if cancellation_token.is_cancelled() {
                    return Ok(());
                }
                let bytes_after = segment.downloaded_bytes.load(Ordering::Relaxed);
                if bytes_after > bytes_before {
                    attempt = 0;
                    if matches!(err, EngineError::IncompleteSegment { .. }) {
                        continue;
                    }
                } else {
                    attempt += 1;
                    if attempt > max_retries {
                        segment.set_status(SegmentStatus::Failed);
                        return Err(err);
                    }
                }
                let backoff_ms = (200 * (1 << attempt.min(5))).min(5000);
                tokio::select! {
                    _ = cancellation_token.cancelled() => return Ok(()),
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)) => {}
                }
            }
        }
    }
}

async fn download_worker_stream(
    client: &reqwest::Client,
    disk_manager: &Arc<DiskManager>,
    task: &Arc<Task>,
    segment: &Arc<DownloadSegment>,
    cancellation_token: &CancellationToken,
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

    let resolved_ref = resolve_referer(&task.url, task.referer.as_deref());
    if let Some(ref r) = resolved_ref
        && let Ok(val) = reqwest::header::HeaderValue::from_str(r)
    {
        request_builder = request_builder.header(reqwest::header::REFERER, val);
    }

    if segment.end_byte > 0 {
        request_builder = request_builder.header(
            reqwest::header::RANGE,
            format!("bytes={current_offset}-{}", segment.end_byte),
        );
    } else if current_offset > 0 {
        request_builder =
            request_builder.header(reqwest::header::RANGE, format!("bytes={current_offset}-"));
    }

    let response = request_builder.send().await?;
    let status = response.status();

    if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(EngineError::HttpStatus(status));
    }

    if current_offset > 0 && status != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(EngineError::HttpStatus(status));
    }

    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        if cancellation_token.is_cancelled() {
            return Ok(());
        }

        if segment.end_byte > 0 && current_offset > segment.end_byte {
            break;
        }

        let chunk = chunk_result?;
        let write_slice = if segment.end_byte > 0 {
            let remaining = (segment.end_byte + 1).saturating_sub(current_offset) as usize;
            let take_len = chunk.len().min(remaining);
            &chunk[..take_len]
        } else {
            &chunk[..]
        };

        if write_slice.is_empty() {
            break;
        }

        disk_manager
            .write_at(&mut file, current_offset, write_slice)
            .await?;

        let bytes_written = write_slice.len() as u64;
        current_offset += bytes_written;
        segment
            .downloaded_bytes
            .fetch_add(bytes_written, Ordering::Relaxed);
        task.downloaded_bytes
            .fetch_add(bytes_written, Ordering::Relaxed);

        if segment.end_byte > 0 && current_offset > segment.end_byte {
            break;
        }
    }

    disk_manager.flush(&mut file).await?;

    if segment.end_byte > 0 {
        let actual = segment.downloaded_bytes.load(Ordering::Relaxed);
        let expected = segment.total_bytes();
        if actual < expected {
            return Err(EngineError::IncompleteSegment { expected, actual });
        }
    }

    segment.set_status(SegmentStatus::Completed);
    Ok(())
}

fn resolve_referer(url_str: &str, custom_referer: Option<&str>) -> Option<String> {
    if let Some(r) = custom_referer {
        return Some(r.to_string());
    }

    if let Ok(parsed_url) = reqwest::Url::parse(url_str)
        && let Some(host) = parsed_url.host_str()
    {
        let scheme = parsed_url.scheme();
        return Some(format!("{scheme}://{host}/"));
    }

    None
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
    use rdm_config::EngineConfig;

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
    fn test_resolve_referer_default() {
        let referer = resolve_referer("https://us1.akirabox.com/uploads/file.zip", None);
        assert_eq!(referer, Some("https://us1.akirabox.com/".to_string()));
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

    #[tokio::test]
    async fn test_persistence_save_and_load() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("rdm_persist_test_{}", Uuid::new_v4()));
        let config_obj = EngineConfig::default()
            .with_state_dir(temp_dir.clone())
            .with_download_dir(temp_dir.clone());
        let config = Arc::new(ConfigManager::from_config(config_obj));
        let disk_manager = Arc::new(DiskManager::new());

        let engine = Engine::new(Arc::clone(&config), Arc::clone(&disk_manager))?;
        let task_id = Uuid::new_v4();
        let task = Arc::new(Task::new(
            task_id,
            "https://example.com/test.zip".to_string(),
            temp_dir.join("test.zip"),
            1024,
            2,
            None,
        ));

        {
            let mut tasks = engine.tasks.write().await;
            tasks.insert(task_id, task);
        }

        engine.save_state().await?;

        let engine2 = Engine::new(Arc::clone(&config), Arc::clone(&disk_manager))?;
        let restored_task = engine2.get_task(task_id).await?;
        assert_eq!(restored_task.id, task_id);
        assert_eq!(restored_task.total_size, 1024);

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        Ok(())
    }
}
