use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, atomic::Ordering},
};

use rdm_config::ConfigManager;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    disk::DiskManager,
    error::EngineError,
    task::{DownloadSegment, PostProcessAction, SegmentStatus, Task, TaskStatus},
};

use super::{
    EngineModule, ResourceProbe,
    multimedia::{MultimediaModule, normalize_post_process, stream_user_agent},
    transfer::{
        decode_and_sanitize_filename, download_to_path, extract_file_name_from_url,
        resolve_referer, stream_request,
    },
};

pub(super) struct DownloadModule {
    config: Arc<ConfigManager>,
    disk_manager: Arc<DiskManager>,
    client: reqwest::Client,
    tasks: RwLock<HashMap<Uuid, Arc<Task>>>,
    queue: RwLock<VecDeque<Uuid>>,
    multimedia: Arc<MultimediaModule>,
}

impl EngineModule for DownloadModule {
    async fn probe(
        &self,
        url: &str,
        referer: Option<&str>,
    ) -> Result<Option<ResourceProbe>, EngineError> {
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

        Ok(Some(ResourceProbe {
            url: url.to_string(),
            file_name,
            total_size,
            supports_ranges,
            content_type,
            post_process: None,
        }))
    }
}

impl DownloadModule {
    pub(super) fn new(
        config: Arc<ConfigManager>,
        disk_manager: Arc<DiskManager>,
        client: reqwest::Client,
        multimedia: Arc<MultimediaModule>,
    ) -> Self {
        let engine_config = config.config();
        let mut initial_tasks = HashMap::new();
        let mut initial_queue = VecDeque::new();
        let state_file = engine_config.state_dir.join("tasks.json");
        if let Ok(content) = std::fs::read_to_string(&state_file)
            && let Ok(persisted_tasks) =
                serde_json::from_str::<Vec<crate::task::PersistedTask>>(&content)
        {
            for persisted in persisted_tasks {
                let id = persisted.id;
                let task = Arc::new(Task::from_persisted(persisted));
                if task.status() == TaskStatus::Pending {
                    initial_queue.push_back(id);
                }
                initial_tasks.insert(id, task);
            }
        }

        Self {
            config,
            disk_manager,
            client,
            tasks: RwLock::new(initial_tasks),
            queue: RwLock::new(initial_queue),
            multimedia,
        }
    }

    pub(super) fn config(&self) -> &Arc<ConfigManager> {
        &self.config
    }

    pub(super) fn disk_manager(&self) -> &Arc<DiskManager> {
        &self.disk_manager
    }

    pub(super) async fn save_state(&self) -> Result<(), EngineError> {
        let state_dir = self.config.config().state_dir;
        persist_tasks_to_disk(&state_dir, &self.tasks).await;
        Ok(())
    }

    pub(super) async fn load_state(&self) -> Result<(), EngineError> {
        let state_file = self.config.config().state_dir.join("tasks.json");
        if !tokio::fs::try_exists(&state_file).await.unwrap_or(false) {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&state_file).await?;
        let persisted_tasks: Vec<crate::task::PersistedTask> = serde_json::from_str(&content)?;

        let mut tasks_guard = self.tasks.write().await;
        let mut queue_guard = self.queue.write().await;
        for persisted in persisted_tasks {
            let task_id = persisted.id;
            let task = Arc::new(Task::from_persisted(persisted));
            if task.status() == TaskStatus::Pending && !queue_guard.contains(&task_id) {
                queue_guard.push_back(task_id);
            }
            tasks_guard.insert(task_id, task);
        }
        Ok(())
    }

    pub(super) async fn create_task(
        &self,
        probe: ResourceProbe,
        output_path: Option<PathBuf>,
        connections: Option<usize>,
        referer: Option<String>,
        explicit_post_process: Option<PostProcessAction>,
    ) -> Result<Arc<Task>, EngineError> {
        let current_cfg = self.config.config();

        let destination_path = match output_path {
            Some(path) => {
                if path.is_dir() {
                    resolve_unique_path(&path.join(&probe.file_name))
                } else {
                    resolve_unique_path(&path)
                }
            }
            None => {
                let target = current_cfg.download_dir.join(&probe.file_name);
                resolve_unique_path(&target)
            }
        };

        let (total_size, final_post_process) = normalize_post_process(
            probe.total_size,
            explicit_post_process.or(probe.post_process),
        );

        let active_connections = if probe.supports_ranges && total_size > 0 {
            connections
                .unwrap_or(current_cfg.default_connections)
                .max(1)
        } else {
            1
        };

        let task_id = Uuid::new_v4();
        let task = Arc::new(Task::new_with_post_process(
            task_id,
            probe.url,
            destination_path,
            total_size,
            active_connections,
            referer,
            final_post_process,
        ));

        {
            let mut tasks_guard = self.tasks.write().await;
            tasks_guard.insert(task_id, Arc::clone(&task));
        }

        let _ = self.save_state().await;

        Ok(task)
    }

    async fn active_downloads_count(&self, exclude_id: Option<Uuid>) -> usize {
        let tasks_guard = self.tasks.read().await;
        tasks_guard
            .values()
            .filter(|t| {
                Some(t.id) != exclude_id
                    && matches!(t.status(), TaskStatus::Downloading | TaskStatus::Connecting)
            })
            .count()
    }

    async fn remove_from_queue(&self, task_id: Uuid) {
        let mut queue_guard = self.queue.write().await;
        queue_guard.retain(|&id| id != task_id);
    }

    pub(super) async fn start_task(self: &Arc<Self>, task_id: Uuid) -> Result<(), EngineError> {
        let task = self.get_task(task_id).await?;

        let current_status = task.status();
        if matches!(
            current_status,
            TaskStatus::Downloading | TaskStatus::Connecting
        ) {
            return Ok(());
        }

        let max_concurrent = self.config.config().max_concurrent_downloads.max(1);
        let active_count = self.active_downloads_count(Some(task_id)).await;

        if active_count >= max_concurrent {
            task.set_status(TaskStatus::Pending);
            {
                let mut queue_guard = self.queue.write().await;
                if !queue_guard.contains(&task_id) {
                    queue_guard.push_back(task_id);
                }
            }
            let _ = self.save_state().await;
            return Ok(());
        }

        self.remove_from_queue(task_id).await;
        self.launch_task(task).await
    }

    pub(super) fn process_queue(
        self: &Arc<Self>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            loop {
                let max_concurrent = self.config.config().max_concurrent_downloads.max(1);
                let active_count = self.active_downloads_count(None).await;
                if active_count >= max_concurrent {
                    break;
                }

                let next_id = {
                    let mut queue_guard = self.queue.write().await;
                    queue_guard.pop_front()
                };

                let Some(task_id) = next_id else {
                    break;
                };

                if let Ok(task) = self.get_task(task_id).await
                    && task.status() == TaskStatus::Pending
                {
                    let _ = self.launch_task(task).await;
                }
            }
        })
    }

    async fn launch_task(self: &Arc<Self>, task: Arc<Task>) -> Result<(), EngineError> {
        let cancellation_token = task.renew_cancellation_token();
        task.set_status(TaskStatus::Connecting);

        let (primary_target_path, primary_total_size) = self.multimedia.primary_target(&task);

        if primary_total_size > 0 {
            self.disk_manager
                .allocate_file(&primary_target_path, primary_total_size)
                .await?;
        } else {
            let mut file = self
                .disk_manager
                .open_for_write(&primary_target_path)
                .await?;
            self.disk_manager.flush(&mut file).await?;
        }

        let mut segments = task.segments();
        if segments.is_empty() {
            let segment_count = task.connections.max(1);
            let mut new_segments = Vec::with_capacity(segment_count);

            if segment_count <= 1 || primary_total_size == 0 {
                let end_byte = if primary_total_size > 0 {
                    primary_total_size - 1
                } else {
                    0
                };
                new_segments.push(Arc::new(DownloadSegment::new(0, 0, end_byte)));
            } else {
                let chunk_size = primary_total_size / segment_count as u64;
                for index in 0..segment_count {
                    let start_byte = index as u64 * chunk_size;
                    let end_byte = if index == segment_count - 1 {
                        primary_total_size - 1
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

        let active_segment_count = segments
            .iter()
            .filter(|s| {
                if s.end_byte > 0 {
                    s.downloaded_bytes.load(Ordering::Relaxed) < s.total_bytes()
                        || s.status() != SegmentStatus::Completed
                } else {
                    s.status() != SegmentStatus::Completed
                }
            })
            .count()
            .max(1);

        let mut worker_handles = Vec::with_capacity(segments.len() + 1);

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
            let config_mgr = Arc::clone(&self.config);
            let task_ref = Arc::clone(&task);
            let target_file = primary_target_path.clone();
            let token = cancellation_token.clone();

            let handle = tokio::spawn(async move {
                download_worker(
                    client,
                    disk_manager,
                    config_mgr,
                    task_ref,
                    target_file,
                    segment,
                    token,
                    active_segment_count,
                )
                .await
            });

            worker_handles.push(handle);
        }
        if let Some(handle) = self
            .multimedia
            .spawn_companion_stream(&task, &cancellation_token)
        {
            worker_handles.push(handle);
        }

        if worker_handles.is_empty() {
            let downloaded = task.downloaded_bytes.load(Ordering::Relaxed);
            if task.total_size == 0 || downloaded >= task.total_size {
                task.set_status(TaskStatus::Completed);
                let _ = self.save_state().await;
                let download = Arc::clone(self);
                tokio::spawn(async move {
                    download.process_queue().await;
                });
                return Ok(());
            }
        }

        let monitor_task = Arc::clone(&task);
        let monitor_state_dir = self.config.config().state_dir;
        let download = Arc::clone(self);

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
                persist_tasks_to_disk(&monitor_state_dir, &download.tasks).await;
                download.process_queue().await;
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
                match download
                    .multimedia
                    .finish(&monitor_state_dir, &monitor_task)
                    .await
                {
                    Ok(()) => monitor_task.set_status(TaskStatus::Completed),
                    Err(err_msg) => monitor_task.set_status(TaskStatus::Failed(err_msg)),
                }
            } else {
                let error_text = match failure_reason {
                    Some(reason) => reason,
                    None => "Download incomplete".to_string(),
                };
                monitor_task.set_status(TaskStatus::Failed(error_text));
            }

            persist_tasks_to_disk(&monitor_state_dir, &download.tasks).await;
            download.process_queue().await;
        });

        Ok(())
    }

    pub(super) async fn pause_task(self: &Arc<Self>, task_id: Uuid) -> Result<(), EngineError> {
        self.remove_from_queue(task_id).await;
        let task = self.get_task(task_id).await?;
        task.pause();
        let _ = self.save_state().await;
        self.process_queue().await;
        Ok(())
    }

    pub(super) async fn cancel_task(self: &Arc<Self>, task_id: Uuid) -> Result<(), EngineError> {
        self.remove_from_queue(task_id).await;
        let task = self.get_task(task_id).await?;
        task.cancel();
        let _ = self.save_state().await;
        self.process_queue().await;
        Ok(())
    }

    pub(super) async fn delete_task(
        self: &Arc<Self>,
        task_id: Uuid,
        delete_file: bool,
    ) -> Result<(), EngineError> {
        self.remove_from_queue(task_id).await;
        let task = {
            let mut tasks_guard = self.tasks.write().await;
            tasks_guard
                .remove(&task_id)
                .ok_or(EngineError::TaskNotFound(task_id))?
        };

        task.cancel();

        self.multimedia.remove_parts(&task.output_path).await;

        if delete_file
            && tokio::fs::try_exists(&task.output_path)
                .await
                .unwrap_or(false)
        {
            let _ = tokio::fs::remove_file(&task.output_path).await;
        }

        let _ = self.save_state().await;
        self.process_queue().await;
        Ok(())
    }

    pub(super) async fn get_task(&self, task_id: Uuid) -> Result<Arc<Task>, EngineError> {
        let tasks_guard = self.tasks.read().await;
        tasks_guard
            .get(&task_id)
            .cloned()
            .ok_or(EngineError::TaskNotFound(task_id))
    }

    pub(super) async fn all_tasks(&self) -> Vec<Arc<Task>> {
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

#[allow(clippy::too_many_arguments)]
async fn download_worker(
    client: reqwest::Client,
    disk_manager: Arc<DiskManager>,
    config: Arc<ConfigManager>,
    task: Arc<Task>,
    target_path: PathBuf,
    segment: Arc<DownloadSegment>,
    cancellation_token: CancellationToken,
    active_segment_count: usize,
) -> Result<(), EngineError> {
    let max_retries = config.config().max_retries.max(1);
    let mut attempt = 0;

    loop {
        if cancellation_token.is_cancelled() {
            return Ok(());
        }

        let bytes_before = segment.downloaded_bytes.load(Ordering::Relaxed);
        let request = stream_request(
            &client,
            &task.url,
            task.referer.as_deref(),
            stream_user_agent(&task.url),
        );
        let result = download_to_path(
            request,
            &disk_manager,
            &config,
            &task,
            &target_path,
            &segment,
            &cancellation_token,
            active_segment_count,
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
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
            let mut tasks = engine.download.tasks.write().await;
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

    #[tokio::test]
    async fn test_concurrent_queue_limit_and_advancement() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = std::env::temp_dir().join(format!("rdm_queue_test_{}", Uuid::new_v4()));
        let config_obj = EngineConfig::default()
            .with_state_dir(temp_dir.clone())
            .with_download_dir(temp_dir.clone())
            .with_max_concurrent_downloads(1);
        let config = Arc::new(ConfigManager::from_config(config_obj));
        let disk_manager = Arc::new(DiskManager::new());

        let engine = Engine::new(Arc::clone(&config), Arc::clone(&disk_manager))?;

        let active_id = Uuid::new_v4();
        let active_task = Arc::new(Task::new(
            active_id,
            "https://example.com/active.bin".to_string(),
            temp_dir.join("active.bin"),
            1024,
            1,
            None,
        ));
        active_task.set_status(TaskStatus::Downloading);

        let queued_id = Uuid::new_v4();
        let queued_task = Arc::new(Task::new(
            queued_id,
            "https://example.com/queued.bin".to_string(),
            temp_dir.join("queued.bin"),
            100,
            1,
            None,
        ));
        let completed_seg = Arc::new(DownloadSegment::new(0, 0, 99));
        completed_seg.downloaded_bytes.store(100, Ordering::Relaxed);
        completed_seg.set_status(SegmentStatus::Completed);
        queued_task.set_segments(vec![completed_seg]);
        queued_task.downloaded_bytes.store(100, Ordering::Relaxed);
        queued_task.set_status(TaskStatus::Paused);

        {
            let mut tasks = engine.download.tasks.write().await;
            tasks.insert(active_id, Arc::clone(&active_task));
            tasks.insert(queued_id, Arc::clone(&queued_task));
        }

        engine.start_task(queued_id).await?;
        assert_eq!(queued_task.status(), TaskStatus::Pending);

        engine.pause_task(active_id).await?;
        assert_eq!(active_task.status(), TaskStatus::Paused);
        assert_eq!(queued_task.status(), TaskStatus::Completed);

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        Ok(())
    }
}
