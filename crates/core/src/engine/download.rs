use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, atomic::Ordering},
};

use rdm_config::ConfigManager;
use tokio::{
    io::AsyncWriteExt,
    sync::{Mutex, RwLock},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    disk::DiskManager,
    error::EngineError,
    task::{
        DownloadSegment, PersistedTask, PostProcessAction, SegmentStatus, Task, TaskState,
        TaskStatus,
    },
};

use super::{
    EngineModule, HttpClientProvider, ResourceProbe, TaskOptions,
    multimedia::{
        MultimediaModule, normalize_post_process, staging_path, stream_user_agent, track_paths,
    },
    transfer::{
        apply_custom_headers, decode_and_sanitize_filename, download_to_path,
        extract_file_name_from_url, resolve_referer, stream_request,
    },
};

pub(super) struct DownloadModule {
    config: Arc<ConfigManager>,
    disk_manager: Arc<DiskManager>,
    client_provider: HttpClientProvider,
    tasks: RwLock<HashMap<Uuid, Arc<Task>>>,
    queue: RwLock<VecDeque<Uuid>>,
    dispatch: Mutex<()>,
    state_write: Mutex<()>,
    multimedia: Arc<MultimediaModule>,
}

impl EngineModule for DownloadModule {
    async fn probe(
        &self,
        url: &str,
        referer: Option<&str>,
    ) -> Result<Option<ResourceProbe>, EngineError> {
        self.probe_with_headers(url, referer, &[], None).await
    }
}

impl DownloadModule {
    pub(super) async fn probe_with_headers(
        &self,
        url: &str,
        referer: Option<&str>,
        custom_headers: &[(String, String)],
        cookies: Option<&str>,
    ) -> Result<Option<ResourceProbe>, EngineError> {
        let client = self.client_provider.client()?;
        let resolved_referer = resolve_referer(url, referer);

        let mut head_builder = client.head(url);
        if let Some(ref r) = resolved_referer
            && let Ok(val) = reqwest::header::HeaderValue::from_str(r)
        {
            head_builder = head_builder.header(reqwest::header::REFERER, val);
        }
        head_builder = apply_custom_headers(head_builder, custom_headers, cookies);

        let response = match head_builder.send().await {
            Ok(res) if res.status().is_success() => res,
            _ => {
                let mut get_builder = client.get(url).header(reqwest::header::RANGE, "bytes=0-0");
                if let Some(ref r) = resolved_referer
                    && let Ok(val) = reqwest::header::HeaderValue::from_str(r)
                {
                    get_builder = get_builder.header(reqwest::header::REFERER, val);
                }
                get_builder = apply_custom_headers(get_builder, custom_headers, cookies);
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
        client_provider: HttpClientProvider,
        multimedia: Arc<MultimediaModule>,
    ) -> Self {
        let engine_config = config.config();
        let mut initial_tasks = HashMap::new();
        let mut initial_queue = VecDeque::new();
        let state_file = engine_config.state_dir.join("tasks.json");
        if let Ok(content) = std::fs::read_to_string(&state_file)
            && let Ok(persisted_tasks) = serde_json::from_str::<Vec<PersistedTask>>(&content)
        {
            for mut persisted in persisted_tasks {
                MultimediaModule::recover_conversion(&mut persisted);
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
            client_provider,
            tasks: RwLock::new(initial_tasks),
            queue: RwLock::new(initial_queue),
            dispatch: Mutex::new(()),
            state_write: Mutex::new(()),
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
        let _write = self.state_write.lock().await;
        self.persist_snapshot(None).await
    }

    async fn persist_snapshot(
        &self,
        state_update: Option<(Uuid, &TaskState)>,
    ) -> Result<(), EngineError> {
        let state_dir = self.config.config().state_dir;
        let tasks: Vec<_> = self
            .tasks
            .read()
            .await
            .values()
            .map(|task| {
                let mut snapshot = task.to_persisted();
                if let Some((id, state)) = state_update
                    && id == task.id
                {
                    snapshot.status = state.status.clone();
                    snapshot.completed_at = state.completed_at;
                }
                snapshot
            })
            .collect();
        persist_tasks_to_disk(&state_dir, &tasks).await
    }

    pub(super) async fn load_state(&self) -> Result<(), EngineError> {
        let _dispatch = self.dispatch.lock().await;
        let state_file = self.config.config().state_dir.join("tasks.json");
        if !tokio::fs::try_exists(&state_file).await.unwrap_or(false) {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&state_file).await?;
        let mut persisted_tasks: Vec<PersistedTask> = serde_json::from_str(&content)?;
        let tasks = self.tasks.read().await;
        persisted_tasks.retain(|task| {
            !tasks
                .get(&task.id)
                .is_some_and(|live| is_active(&live.status()))
        });
        drop(tasks);
        let persisted_tasks = tokio::task::spawn_blocking(move || {
            for task in &mut persisted_tasks {
                MultimediaModule::recover_conversion(task);
            }
            persisted_tasks
        })
        .await?;

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

    #[cfg(test)]
    pub(super) async fn create_task(
        &self,
        probe: ResourceProbe,
        output_path: Option<PathBuf>,
        connections: Option<usize>,
        referer: Option<String>,
        explicit_post_process: Option<PostProcessAction>,
    ) -> Result<Arc<Task>, EngineError> {
        self.create_task_configured(
            probe,
            TaskOptions {
                output_path,
                connections,
                referer,
                post_process: explicit_post_process,
                ..Default::default()
            },
        )
        .await
    }

    pub(super) async fn create_task_configured(
        &self,
        probe: ResourceProbe,
        options: TaskOptions,
    ) -> Result<Arc<Task>, EngineError> {
        let current_cfg = self.config.config();

        let requested_path = match options.output_path {
            Some(path) => {
                if tokio::fs::metadata(&path)
                    .await
                    .is_ok_and(|metadata| metadata.is_dir())
                {
                    path.join(&probe.file_name)
                } else {
                    path
                }
            }
            None => current_cfg.download_dir.join(&probe.file_name),
        };

        let (total_size, final_post_process) = normalize_post_process(
            probe.total_size,
            options.post_process.or(probe.post_process),
        );

        let active_connections = if probe.supports_ranges && total_size > 0 {
            options
                .connections
                .unwrap_or(current_cfg.default_connections)
                .max(1)
        } else {
            1
        };

        let task_id = Uuid::new_v4();
        let _dispatch = self.dispatch.lock().await;
        let reserved = self
            .tasks
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let destination_path = tokio::task::spawn_blocking({
            let post_process = final_post_process.clone();
            move || {
                resolve_unique_path(
                    &requested_path,
                    total_size,
                    post_process.as_ref(),
                    &reserved,
                )
            }
        })
        .await??;
        let task = Arc::new(Task::new_configured(
            task_id,
            probe.url,
            destination_path,
            total_size,
            active_connections,
            options.referer,
            final_post_process,
            options.checksum,
            options.headers,
            options.cookies,
            options.max_speed_bytes_per_sec.unwrap_or(0),
        ));

        {
            let mut tasks_guard = self.tasks.write().await;
            tasks_guard.insert(task_id, Arc::clone(&task));
        }

        let _ = self.save_state().await;

        Ok(task)
    }

    async fn active_downloads_count(&self) -> usize {
        let tasks_guard = self.tasks.read().await;
        tasks_guard
            .values()
            .filter(|task| is_active(&task.status()))
            .count()
    }

    async fn remove_from_queue(&self, task_id: Uuid) {
        let mut queue_guard = self.queue.write().await;
        queue_guard.retain(|&id| id != task_id);
    }

    pub(super) async fn start_task(self: &Arc<Self>, task_id: Uuid) -> Result<(), EngineError> {
        self.request_start(task_id, false).await
    }

    pub(super) async fn retry_conversion(
        self: &Arc<Self>,
        task_id: Uuid,
    ) -> Result<(), EngineError> {
        self.request_start(task_id, true).await
    }

    async fn request_start(
        self: &Arc<Self>,
        task_id: Uuid,
        conversion_only: bool,
    ) -> Result<(), EngineError> {
        let dispatch = self.dispatch.lock().await;
        let task = self.get_task(task_id).await?;

        let current_status = task.status();
        let is_conversion = matches!(
            current_status,
            TaskStatus::ConversionFailed(_)
                | TaskStatus::ConversionQueued
                | TaskStatus::Processing { .. }
        );
        if conversion_only && (!is_conversion || task.post_process.is_none()) {
            return Err(EngineError::ConversionNotRetryable(task_id));
        }
        if is_active(&current_status) || current_status == TaskStatus::Completed {
            return Ok(());
        }

        let max_concurrent = self.config.config().max_concurrent_downloads.max(1);
        if is_conversion || self.active_downloads_count().await >= max_concurrent {
            task.set_status(if is_conversion {
                TaskStatus::ConversionQueued
            } else {
                TaskStatus::Pending
            });
            {
                let mut queue_guard = self.queue.write().await;
                if !queue_guard.contains(&task_id) {
                    queue_guard.push_back(task_id);
                }
            }
            let _ = self.save_state().await;
            drop(dispatch);
            if is_conversion {
                self.process_queue().await;
            }
            return Ok(());
        }

        self.remove_from_queue(task_id).await;
        let result = self.launch_download(task).await;
        drop(dispatch);
        if result.is_err() {
            self.process_queue().await;
        }
        result
    }

    pub(super) fn process_queue(
        self: &Arc<Self>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            loop {
                let _dispatch = self.dispatch.lock().await;
                let max_concurrent = self.config.config().max_concurrent_downloads.max(1);
                let active_count = self.active_downloads_count().await;
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

                if let Ok(task) = self.get_task(task_id).await {
                    match task.status() {
                        TaskStatus::Pending => {
                            let _ = self.launch_download(task).await;
                        }
                        TaskStatus::ConversionQueued => self.spawn_conversion(task),
                        _ => {}
                    }
                }
            }
        })
    }

    async fn launch_download(self: &Arc<Self>, task: Arc<Task>) -> Result<(), EngineError> {
        let result = self.launch_task(Arc::clone(&task)).await;
        if let Err(error) = &result {
            task.set_status(TaskStatus::Failed(error.to_string()));
            let _ = self.save_state().await;
        }
        result
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

        let client = self.client_provider.client()?;
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

            let client = client.clone();
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

        if task.post_process.is_none() && worker_handles.is_empty() {
            let downloaded = task.downloaded_bytes.load(Ordering::Relaxed);
            if task.total_size == 0 || downloaded >= task.total_size {
                if let Some(ref expected) = task.checksum
                    && let Err(err) = self
                        .disk_manager
                        .verify_checksum(&task.output_path, expected)
                        .await
                {
                    task.set_status(TaskStatus::Failed(err.to_string()));
                } else {
                    task.set_status(TaskStatus::Completed);
                }
                let _ = self.save_state().await;
                let download = Arc::clone(self);
                tokio::spawn(async move {
                    download.process_queue().await;
                });
                return Ok(());
            }
        }

        let monitor_task = Arc::clone(&task);
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

            let dispatch = download.dispatch.lock().await;
            if cancellation_token.is_cancelled() || monitor_task.status() != TaskStatus::Downloading
            {
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
                if monitor_task.post_process.is_some() {
                    download.spawn_conversion(monitor_task);
                    return;
                }
                if let Some(ref expected) = monitor_task.checksum
                    && let Err(err) = download
                        .disk_manager
                        .verify_checksum(&monitor_task.output_path, expected)
                        .await
                {
                    monitor_task.set_status(TaskStatus::Failed(err.to_string()));
                } else {
                    monitor_task.set_status(TaskStatus::Completed);
                }
            } else {
                let error_text = match failure_reason {
                    Some(reason) => reason,
                    None => "Download incomplete".to_string(),
                };
                monitor_task.set_status(TaskStatus::Failed(error_text));
            }

            let _ = download.save_state().await;
            drop(dispatch);
            download.process_queue().await;
        });

        Ok(())
    }

    fn spawn_conversion(self: &Arc<Self>, task: Arc<Task>) {
        task.set_status(TaskStatus::Processing { staged_size: None });
        let download = Arc::clone(self);
        tokio::spawn(async move {
            let result = download.convert(&task).await;
            {
                let _dispatch = download.dispatch.lock().await;
                let was_staged = matches!(
                    task.status(),
                    TaskStatus::Processing {
                        staged_size: Some(_)
                    }
                );
                let status = match result {
                    Ok(()) => {
                        download.multimedia.remove_parts(&task).await;
                        if let Some(ref expected) = task.checksum
                            && let Err(err) = download
                                .disk_manager
                                .verify_checksum(&task.output_path, expected)
                                .await
                        {
                            TaskStatus::Failed(err.to_string())
                        } else {
                            TaskStatus::Completed
                        }
                    }
                    Err(error) => {
                        if !was_staged {
                            download.multimedia.discard_staging(&task).await;
                        }
                        TaskStatus::ConversionFailed(error)
                    }
                };
                let state = TaskState::new(status);
                let _write = download.state_write.lock().await;
                let saved = download.persist_snapshot(Some((task.id, &state))).await;
                if was_staged
                    && saved.is_ok()
                    && matches!(state.status, TaskStatus::ConversionFailed(_))
                {
                    download.multimedia.discard_staging(&task).await;
                }
                task.set_state(state);
            }
            download.process_queue().await;
        });
    }

    async fn convert(&self, task: &Task) -> Result<(), String> {
        self.save_state().await.map_err(|error| error.to_string())?;
        let size = self.multimedia.prepare_conversion(task).await?;
        task.set_status(TaskStatus::Processing {
            staged_size: Some(size),
        });
        self.save_state().await.map_err(|error| error.to_string())?;
        self.multimedia.publish_conversion(task).await
    }

    pub(super) async fn pause_task(self: &Arc<Self>, task_id: Uuid) -> Result<(), EngineError> {
        let dispatch = self.dispatch.lock().await;
        let task = self.get_task(task_id).await?;
        if !download_control_available(&task)? {
            return Ok(());
        }
        self.remove_from_queue(task_id).await;
        task.pause();
        let _ = self.save_state().await;
        drop(dispatch);
        self.process_queue().await;
        Ok(())
    }

    pub(super) async fn cancel_task(self: &Arc<Self>, task_id: Uuid) -> Result<(), EngineError> {
        let dispatch = self.dispatch.lock().await;
        let task = self.get_task(task_id).await?;
        if !download_control_available(&task)? {
            return Ok(());
        }
        self.remove_from_queue(task_id).await;
        task.cancel();
        let _ = self.save_state().await;
        drop(dispatch);
        self.process_queue().await;
        Ok(())
    }

    pub(super) async fn delete_task(
        self: &Arc<Self>,
        task_id: Uuid,
        delete_file: bool,
    ) -> Result<(), EngineError> {
        let dispatch = self.dispatch.lock().await;
        let task = self.get_task(task_id).await?;
        if matches!(task.status(), TaskStatus::Processing { .. }) {
            return Err(EngineError::ConversionInProgress(task_id));
        }
        self.remove_from_queue(task_id).await;
        self.tasks.write().await.remove(&task_id);
        task.cancel();

        if task.post_process.is_some() {
            self.multimedia.discard_staging(&task).await;
            self.multimedia.remove_parts(&task).await;
        }

        if delete_file
            && tokio::fs::try_exists(&task.output_path)
                .await
                .unwrap_or(false)
        {
            let _ = tokio::fs::remove_file(&task.output_path).await;
        }

        let _ = self.save_state().await;
        drop(dispatch);
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

    pub(super) async fn refresh_task_url(
        &self,
        task_id: Uuid,
        new_url: &str,
        new_referer: Option<String>,
    ) -> Result<(), EngineError> {
        let trimmed_url = new_url.trim();
        let parsed = reqwest::Url::parse(trimmed_url)
            .map_err(|_| EngineError::InvalidUrl(trimmed_url.to_string()))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(EngineError::InvalidUrl(trimmed_url.to_string()));
        }

        let (headers, cookies, effective_referer, expected_size) = {
            let task = self.get_task(task_id).await?;
            let status = task.status();
            if is_active(&status)
                || matches!(
                    status,
                    TaskStatus::Completed
                        | TaskStatus::ConversionQueued
                        | TaskStatus::ConversionFailed(_)
                )
            {
                return Err(EngineError::InvalidTaskState(format!(
                    "Cannot refresh URL while task {task_id} is in state {status:?}"
                )));
            }
            (
                task.headers.clone(),
                task.cookies.clone(),
                new_referer.clone().or_else(|| task.referer()),
                task.total_size,
            )
        };

        let probe = self
            .probe_with_headers(
                trimmed_url,
                effective_referer.as_deref(),
                &headers,
                cookies.as_deref(),
            )
            .await?
            .ok_or(EngineError::HttpStatus(reqwest::StatusCode::NOT_FOUND))?;

        if expected_size > 0 && probe.total_size > 0 && probe.total_size != expected_size {
            return Err(EngineError::UrlSizeMismatch {
                expected: expected_size,
                actual: probe.total_size,
            });
        }

        let _dispatch = self.dispatch.lock().await;
        let task = self.get_task(task_id).await?;
        let status = task.status();
        if is_active(&status)
            || matches!(
                status,
                TaskStatus::Completed
                    | TaskStatus::ConversionQueued
                    | TaskStatus::ConversionFailed(_)
            )
        {
            return Err(EngineError::InvalidTaskState(format!(
                "Cannot refresh URL while task {task_id} is in state {status:?}"
            )));
        }

        task.set_url(probe.url);
        if let Some(referer) = new_referer {
            task.set_referer(Some(referer));
        }

        for segment in task.segments() {
            if segment.status() == SegmentStatus::Failed {
                segment.set_status(SegmentStatus::Pending);
            }
        }

        if matches!(status, TaskStatus::Failed(_) | TaskStatus::Cancelled) {
            task.set_status(TaskStatus::Paused);
        }

        self.save_state().await?;
        Ok(())
    }

    pub(super) async fn set_task_speed_limit(
        &self,
        task_id: Uuid,
        bytes_per_sec: u64,
    ) -> Result<(), EngineError> {
        let task = self.get_task(task_id).await?;
        task.set_max_speed_bytes_per_sec(bytes_per_sec);
        self.save_state().await?;
        Ok(())
    }
}

fn is_active(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Connecting | TaskStatus::Downloading | TaskStatus::Processing { .. }
    )
}

fn download_control_available(task: &Task) -> Result<bool, EngineError> {
    match task.status() {
        TaskStatus::Processing { .. } | TaskStatus::ConversionQueued => {
            Err(EngineError::ConversionInProgress(task.id))
        }
        TaskStatus::ConversionFailed(_) => Ok(false),
        TaskStatus::Completed if task.post_process.is_some() => Ok(false),
        _ => Ok(true),
    }
}

async fn persist_tasks_to_disk(
    state_dir: &Path,
    tasks: &[PersistedTask],
) -> Result<(), EngineError> {
    let json = serde_json::to_vec_pretty(tasks)?;
    tokio::fs::create_dir_all(state_dir).await?;
    let temp = state_dir.join(format!(".tasks-{}.tmp", Uuid::new_v4()));
    let result = async {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .await?;
        file.write_all(&json).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temp, state_dir.join("tasks.json")).await
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temp).await;
    }
    result?;
    Ok(())
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
        let task_url = task.url();
        let task_referer = task.referer();
        let request = stream_request(
            &client,
            &task_url,
            task_referer.as_deref(),
            stream_user_agent(&task_url),
            &task.headers,
            task.cookies.as_deref(),
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

fn resolve_unique_path(
    target_path: &Path,
    total_size: u64,
    post_process: Option<&PostProcessAction>,
    tasks: &[Arc<Task>],
) -> std::io::Result<PathBuf> {
    let reserved: HashSet<_> = tasks
        .iter()
        .flat_map(|task| {
            std::iter::once(task.output_path.clone())
                .chain(
                    track_paths(
                        &task.output_path,
                        task.total_size,
                        task.post_process.as_ref(),
                    )
                    .map(|(path, _)| path),
                )
                .chain(
                    task.post_process
                        .as_ref()
                        .map(|_| staging_path(&task.output_path, task.id)),
                )
        })
        .map(|path| reservation_key(&path))
        .collect();
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

    let mut counter = 0usize;
    loop {
        let candidate = if counter == 0 {
            target_path.to_path_buf()
        } else {
            let name = match extension {
                Some(ext) => format!("{stem} ({counter}).{ext}"),
                None => format!("{stem} ({counter})"),
            };
            parent.join(name)
        };
        let mut available = true;
        let candidates = std::iter::once(candidate.clone())
            .chain(track_paths(&candidate, total_size, post_process).map(|(path, _)| path));
        for path in candidates {
            if reserved.contains(&reservation_key(&path)) {
                available = false;
                break;
            }
            match std::fs::symlink_metadata(path) {
                Ok(_) => {
                    available = false;
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        if available {
            return Ok(candidate);
        }
        counter += 1;
    }
}

fn reservation_key(path: &Path) -> PathBuf {
    let normalized = path
        .parent()
        .zip(path.file_name())
        .and_then(|(parent, filename)| {
            let parent = if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            };
            std::fs::canonicalize(parent)
                .ok()
                .map(|directory| directory.join(filename))
        })
        .or_else(|| std::path::absolute(path).ok())
        .unwrap_or_else(|| path.to_path_buf());
    #[cfg(windows)]
    let normalized = PathBuf::from(normalized.to_string_lossy().to_lowercase());
    normalized
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
mod conversion_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{engine::Engine, task::ExpectedChecksum};
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
    fn test_resolve_unique_path() -> std::io::Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("rdm_unique_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let base_file = temp_dir.join("sample.zip");

        assert_eq!(resolve_unique_path(&base_file, 0, None, &[])?, base_file);

        let _ = std::fs::write(&base_file, b"test");
        let first_duplicate = resolve_unique_path(&base_file, 0, None, &[])?;
        assert_eq!(first_duplicate, temp_dir.join("sample (1).zip"));

        let _ = std::fs::write(&first_duplicate, b"test");
        let second_duplicate = resolve_unique_path(&base_file, 0, None, &[])?;
        assert_eq!(second_duplicate, temp_dir.join("sample (2).zip"));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
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

    async fn spawn_mock_http_server(
        payload: &'static [u8],
        required_header: Option<(&'static str, &'static str)>,
    ) -> std::io::Result<std::net::SocketAddr> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let Ok(n) = socket.read(&mut buf).await else {
                        return;
                    };
                    let req = String::from_utf8_lossy(&buf[..n]);
                    if let Some((name, val)) = required_header {
                        let expected_line = format!("{name}: {val}").to_ascii_lowercase();
                        if !req.to_ascii_lowercase().contains(&expected_line) {
                            let _ = socket
                                .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
                                .await;
                            return;
                        }
                    }

                    let is_head = req.starts_with("HEAD ");
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        payload.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    if !is_head {
                        let _ = socket.write_all(payload).await;
                    }
                });
            }
        });

        Ok(addr)
    }

    #[tokio::test]
    async fn download_verifies_checksum_sends_headers_and_refreshes_url()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("rdm_net_test_{}", Uuid::new_v4()));
        let config_obj = EngineConfig::default()
            .with_state_dir(temp_dir.clone())
            .with_download_dir(temp_dir.clone());
        let config = Arc::new(ConfigManager::from_config(config_obj));
        let disk_manager = Arc::new(DiskManager::new());
        let engine = Engine::new(Arc::clone(&config), Arc::clone(&disk_manager))?;

        let addr = spawn_mock_http_server(b"abc", Some(("x-api-key", "secret123"))).await?;
        let url = format!("http://{addr}/file.bin");

        let task = engine
            .create_task_configured(
                &url,
                TaskOptions {
                    connections: Some(1),
                    headers: vec![("X-Api-Key".to_string(), "secret123".to_string())],
                    cookies: Some("session=ok".to_string()),
                    checksum: Some(ExpectedChecksum::parse(
                        "md5:900150983cd24fb0d6963f7d28e17f72",
                    )?),
                    max_speed_bytes_per_sec: Some(0),
                    ..Default::default()
                },
            )
            .await?;

        engine.set_task_speed_limit(task.id, 50_000).await?;
        assert_eq!(task.max_speed_bytes_per_sec(), 50_000);

        engine.start_task(task.id).await?;
        for _ in 0..50 {
            if task.status() == TaskStatus::Completed {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        }
        assert_eq!(task.status(), TaskStatus::Completed);

        let mismatch_task = engine
            .create_task_configured(
                &url,
                TaskOptions {
                    connections: Some(1),
                    headers: vec![("X-Api-Key".to_string(), "secret123".to_string())],
                    checksum: Some(ExpectedChecksum::parse(
                        "md5:00000000000000000000000000000000",
                    )?),
                    ..Default::default()
                },
            )
            .await?;
        engine.start_task(mismatch_task.id).await?;
        for _ in 0..50 {
            if matches!(mismatch_task.status(), TaskStatus::Failed(_)) {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        }
        assert!(matches!(mismatch_task.status(), TaskStatus::Failed(_)));

        let addr2 = spawn_mock_http_server(b"abc", Some(("x-api-key", "secret123"))).await?;
        let new_url = format!("http://{addr2}/refreshed.bin");
        engine
            .refresh_task_url(mismatch_task.id, &new_url, None)
            .await?;
        assert_eq!(mismatch_task.url(), new_url);
        assert_eq!(mismatch_task.status(), TaskStatus::Paused);

        let wrong_size_addr =
            spawn_mock_http_server(b"abcdef", Some(("x-api-key", "secret123"))).await?;
        let wrong_size_url = format!("http://{wrong_size_addr}/wrong.bin");
        let err = engine
            .refresh_task_url(mismatch_task.id, &wrong_size_url, None)
            .await;
        assert!(matches!(err, Err(EngineError::UrlSizeMismatch { .. })));

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        Ok(())
    }
}
