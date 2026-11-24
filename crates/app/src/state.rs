use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use chrono::{DateTime, Datelike, Days, Local, NaiveDate};
use gpui::{App, AsyncApp, Context, Entity, Global, WeakEntity};
use rdm_core::{
    Core, Engine,
    task::{ExpectedChecksum, SegmentStatus, Task, TaskStatus},
    updater::{ReleaseInfo, UpdatePhase, UpdateProgress},
};
use uuid::Uuid;

use crate::{
    lang::ActiveLang,
    modals::NewDownloadModal,
    notifications::{NotificationKind, NotificationsModel, notify_entity},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdaterStatus {
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    Downloading {
        info: ReleaseInfo,
        downloaded_bytes: u64,
        total_bytes: u64,
    },
    Verifying {
        info: ReleaseInfo,
    },
    Installing {
        info: ReleaseInfo,
    },
    Error {
        message: String,
        release_url: Option<String>,
    },
}

#[derive(Default)]
pub struct DownloadRequest {
    pub url: String,
    pub output_path: Option<PathBuf>,
    pub directory: Option<PathBuf>,
    pub connections: Option<usize>,
    pub referer: Option<String>,
    pub post_process: Option<rdm_core::task::PostProcessAction>,
    pub retry_task: Option<Uuid>,
    pub network_options: rdm_core::engine::TaskOptions,
}

pub struct DownloadStartResult {
    pub task_id: Option<Uuid>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentSnapshot {
    pub index: usize,
    pub start_byte: u64,
    pub end_byte: u64,
    pub downloaded_bytes: u64,
    pub status: SegmentStatus,
}

impl SegmentSnapshot {
    pub fn total_bytes(&self) -> u64 {
        self.end_byte.saturating_sub(self.start_byte) + 1
    }

    pub fn progress_percent(&self) -> f32 {
        if self.status == SegmentStatus::Completed {
            return 100.0;
        }
        let total = self.total_bytes();
        if total == 0 {
            return 0.0;
        }
        ((self.downloaded_bytes as f64 / total as f64).clamp(0.0, 1.0) * 100.0) as f32
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskSnapshot {
    pub id: Uuid,
    pub file_name: String,
    pub url: String,
    pub output_path: PathBuf,
    pub total_size: u64,
    pub downloaded_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub status: TaskStatus,
    pub progress: f64,
    pub connections: usize,
    pub max_speed_bytes_per_sec: u64,
    pub checksum: Option<ExpectedChecksum>,
    pub referer: Option<String>,
    pub completed_at: Option<u64>,
    pub segments: Vec<SegmentSnapshot>,
}

pub struct TasksModel {
    core: Arc<Core>,
    runtime: Arc<tokio::runtime::Runtime>,
    tasks: Vec<TaskSnapshot>,
    previous_bytes: Vec<(Uuid, u64)>,
    previous_statuses: HashMap<Uuid, TaskStatus>,
    initialized_statuses: bool,
    last_sample_at: Instant,
    download_history: Option<DownloadHistory>,
    updater_status: UpdaterStatus,
    ffmpeg_status: rdm_core::engine::FfmpegStatus,
    ytdlp_status: rdm_core::engine::YtDlpStatus,
    active_update_progress: Option<Arc<UpdateProgress>>,
}

pub(crate) const HISTORY_WEEKS: usize = 53;
pub(crate) const HISTORY_WEEKDAYS: usize = 7;
const HISTORY_DAYS: usize = 365;

#[derive(PartialEq, Eq)]
pub(crate) struct DownloadHistory {
    pub first_week: NaiveDate,
    pub first_cell: usize,
    pub last_cell: usize,
    pub counts: [usize; HISTORY_WEEKS * HISTORY_WEEKDAYS],
    pub total: usize,
    pub undated: usize,
}

impl DownloadHistory {
    fn from_tasks(tasks: &[Arc<Task>], today: NaiveDate) -> Option<Self> {
        let start = today.checked_sub_days(Days::new((HISTORY_DAYS - 1) as u64))?;
        let first_cell = start.weekday().num_days_from_monday() as usize;
        let first_week = start.checked_sub_days(Days::new(first_cell as u64))?;
        let mut history = Self {
            first_week,
            first_cell,
            last_cell: first_cell + HISTORY_DAYS - 1,
            counts: [0; HISTORY_WEEKS * HISTORY_WEEKDAYS],
            total: 0,
            undated: 0,
        };

        for task in tasks {
            let (status, completed_at) = task.status_and_completion();
            if status != TaskStatus::Completed {
                continue;
            }
            let Some(date) = completed_at
                .and_then(|seconds| i64::try_from(seconds).ok())
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
                .map(|time| time.with_timezone(&Local).date_naive())
            else {
                history.undated += 1;
                continue;
            };
            if (start..=today).contains(&date) {
                let cell = date.signed_duration_since(first_week).num_days() as usize;
                history.counts[cell] += 1;
                history.total += 1;
            }
        }
        Some(history)
    }
}

impl TasksModel {
    pub fn new(
        core: Arc<Core>,
        runtime: Arc<tokio::runtime::Runtime>,
        cx: &mut Context<Self>,
    ) -> Self {
        let auto_check = core.config().ui_config().auto_check_updates;
        let model = Self {
            core: Arc::clone(&core),
            runtime: Arc::clone(&runtime),
            tasks: Vec::new(),
            previous_bytes: Vec::new(),
            previous_statuses: HashMap::new(),
            initialized_statuses: false,
            last_sample_at: Instant::now(),
            download_history: None,
            updater_status: UpdaterStatus::Idle,
            ffmpeg_status: rdm_core::engine::FfmpegStatus::Idle,
            ytdlp_status: rdm_core::engine::YtDlpStatus::Idle,
            active_update_progress: None,
        };

        if auto_check {
            cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let _ = entity.update(cx, |this, cx| {
                    this.check_for_updates_internal(true, cx);
                });
            })
            .detach();
        }

        let mut intercept_rx = core.subscribe_intercepted();
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                match intercept_rx.recv().await {
                    Ok(intercepted) => {
                        if entity.upgrade().is_none() {
                            break;
                        }
                        cx.update(|cx| {
                            #[cfg(target_os = "linux")]
                            let (had_window, handle) = {
                                let had_window = !cx.windows().is_empty();
                                match crate::linux_desktop::open_window(cx) {
                                    Ok(window) => (had_window, Some(window)),
                                    Err(error) => {
                                        eprintln!(
                                            "No se pudo abrir la descarga interceptada: {error}"
                                        );
                                        return;
                                    }
                                }
                            };
                            #[cfg(not(target_os = "linux"))]
                            let handle = cx.windows().into_iter().next();

                            cx.activate(true);
                            if let Some(handle) = handle {
                                let _ = handle.update(cx, |_, window, cx| {
                                    #[cfg(windows)]
                                    crate::windows_desktop::restore_window(window);
                                    #[cfg(target_os = "linux")]
                                    if !had_window {
                                        window.activate_window();
                                    }
                                    #[cfg(not(any(windows, target_os = "linux")))]
                                    window.activate_window();
                                    NewDownloadModal::open(Some(intercepted), window, cx);
                                    window.refresh();
                                });
                            }
                        });
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        })
        .detach();

        let mut ffmpeg_status = core.engine().subscribe_ffmpeg_status();
        let mut ytdlp_status = core.engine().subscribe_ytdlp_status();
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let core_init = Arc::clone(&core);
            let (initial_tasks, history) = runtime
                .spawn(async move {
                    let tasks = core_init.engine().all_tasks().await;
                    let history = DownloadHistory::from_tasks(&tasks, Local::now().date_naive());
                    (tasks, history)
                })
                .await
                .unwrap_or_default();

            let _ = entity.update(cx, |this, cx| {
                this.download_history = history;
                this.refresh_snapshots(initial_tasks, true);
                cx.notify();
                cx.refresh_windows();
            });

            let mut tick_count: u64 = 0;

            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;

                tick_count = tick_count.wrapping_add(1);
                let should_persist = tick_count.is_multiple_of(4);

                let core_loop = Arc::clone(&core);
                let (tasks, history) = runtime
                    .spawn(async move {
                        let all = core_loop.engine().all_tasks().await;
                        if should_persist
                            && all.iter().any(|t| t.status() == TaskStatus::Downloading)
                        {
                            let _ = core_loop.engine().save_state().await;
                        }
                        let history = DownloadHistory::from_tasks(&all, Local::now().date_naive());
                        (all, history)
                    })
                    .await
                    .unwrap_or_default();

                let latest_ffmpeg_status = ffmpeg_status.borrow_and_update().clone();
                let latest_ytdlp_status = ytdlp_status.borrow_and_update().clone();
                let update_result = entity.update(cx, |this, cx| {
                    let mut changed = this.ffmpeg_status != latest_ffmpeg_status
                        || this.ytdlp_status != latest_ytdlp_status;
                    this.ffmpeg_status = latest_ffmpeg_status;
                    this.ytdlp_status = latest_ytdlp_status;
                    if let Some(progress) = &this.active_update_progress {
                        let info = match &this.updater_status {
                            UpdaterStatus::Downloading { info, .. }
                            | UpdaterStatus::Verifying { info }
                            | UpdaterStatus::Installing { info } => Some(info.clone()),
                            _ => None,
                        };
                        if let Some(info) = info {
                            let next_status = match progress.phase() {
                                UpdatePhase::Downloading => UpdaterStatus::Downloading {
                                    info,
                                    downloaded_bytes: progress.downloaded(),
                                    total_bytes: progress.total(),
                                },
                                UpdatePhase::Verifying => UpdaterStatus::Verifying { info },
                                UpdatePhase::Installing => UpdaterStatus::Installing { info },
                            };
                            if this.updater_status != next_status {
                                this.updater_status = next_status;
                                changed = true;
                            }
                        }
                    }
                    if this.download_history != history {
                        this.download_history = history;
                        changed = true;
                    }
                    let (snapshots_changed, events) =
                        this.refresh_snapshots_with_events(tasks, true);
                    changed |= snapshots_changed;
                    this.emit_notifications(events, cx);
                    if changed {
                        cx.notify();
                        cx.refresh_windows();
                    }
                });

                if update_result.is_err() {
                    break;
                }
            }
        })
        .detach();

        model
    }

    pub fn ffmpeg_status(&self) -> &rdm_core::engine::FfmpegStatus {
        &self.ffmpeg_status
    }

    pub fn ytdlp_status(&self) -> &rdm_core::engine::YtDlpStatus {
        &self.ytdlp_status
    }

    pub fn prepare_ytdlp(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.ytdlp_status,
            rdm_core::engine::YtDlpStatus::Checking
                | rdm_core::engine::YtDlpStatus::Downloading { .. }
        ) {
            return;
        }
        self.ytdlp_status = rdm_core::engine::YtDlpStatus::Checking;
        let core = Arc::clone(&self.core);
        let runtime = Arc::clone(&self.runtime);
        cx.notify();
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let status = match runtime
                .spawn(async move { core.engine().ensure_ytdlp().await })
                .await
            {
                Ok(Ok(_)) => rdm_core::engine::YtDlpStatus::Ready,
                Ok(Err(error)) => rdm_core::engine::YtDlpStatus::Error(error),
                Err(error) => rdm_core::engine::YtDlpStatus::Error(error.to_string()),
            };
            let _ = entity.update(cx, |this, cx| {
                this.ytdlp_status = status;
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
    }

    pub fn prepare_ffmpeg(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.ffmpeg_status,
            rdm_core::engine::FfmpegStatus::Checking
                | rdm_core::engine::FfmpegStatus::Downloading { .. }
        ) {
            return;
        }
        self.ffmpeg_status = rdm_core::engine::FfmpegStatus::Checking;
        let core = Arc::clone(&self.core);
        let runtime = Arc::clone(&self.runtime);
        cx.notify();
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let result = runtime
                .spawn(async move { core.engine().ensure_ffmpeg().await })
                .await;
            let status = match result {
                Ok(Ok(_)) => rdm_core::engine::FfmpegStatus::Ready,
                Ok(Err(error)) => rdm_core::engine::FfmpegStatus::Error(error),
                Err(error) => rdm_core::engine::FfmpegStatus::Error(error.to_string()),
            };
            let _ = entity.update(cx, |this, cx| {
                this.ffmpeg_status = status;
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
    }

    pub fn updater_status(&self) -> &UpdaterStatus {
        &self.updater_status
    }

    pub fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        self.check_for_updates_internal(false, cx);
    }

    fn check_for_updates_internal(&mut self, silent: bool, cx: &mut Context<Self>) {
        if matches!(
            self.updater_status,
            UpdaterStatus::Checking
                | UpdaterStatus::Downloading { .. }
                | UpdaterStatus::Verifying { .. }
                | UpdaterStatus::Installing { .. }
        ) {
            return;
        }
        self.updater_status = UpdaterStatus::Checking;
        cx.notify();
        cx.refresh_windows();

        let runtime = Arc::clone(&self.runtime);
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let result = runtime
                .spawn(async move { rdm_core::updater::check_for_update().await })
                .await;

            let status = match result {
                Ok(Ok(Some(info))) => UpdaterStatus::Available(info),
                Ok(Ok(None)) => {
                    if silent {
                        UpdaterStatus::Idle
                    } else {
                        UpdaterStatus::UpToDate
                    }
                }
                Ok(Err(e)) => {
                    if silent {
                        UpdaterStatus::Idle
                    } else {
                        UpdaterStatus::Error {
                            message: format_update_error(e),
                            release_url: None,
                        }
                    }
                }
                Err(e) => {
                    if silent {
                        UpdaterStatus::Idle
                    } else {
                        UpdaterStatus::Error {
                            message: e.to_string(),
                            release_url: None,
                        }
                    }
                }
            };

            let _ = entity.update(cx, |this, cx| {
                let update_event = match &status {
                    UpdaterStatus::Available(info) => {
                        Some(NotificationKind::UpdateAvailable { info: info.clone() })
                    }
                    _ => None,
                };
                this.updater_status = status;
                if let Some(event) = update_event {
                    this.emit_notifications(vec![event], cx);
                }
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
    }

    pub fn install_update(&mut self, cx: &mut Context<Self>) {
        let UpdaterStatus::Available(info) = self.updater_status.clone() else {
            return;
        };
        if !info.can_auto_install() {
            cx.open_url(&info.html_url);
            return;
        }

        let progress = Arc::new(UpdateProgress::default());
        self.active_update_progress = Some(Arc::clone(&progress));
        self.updater_status = UpdaterStatus::Downloading {
            info: info.clone(),
            downloaded_bytes: 0,
            total_bytes: info.installer_size,
        };
        cx.notify();
        cx.refresh_windows();

        let runtime = Arc::clone(&self.runtime);
        let progress_download = Arc::clone(&progress);
        let info_download = info.clone();
        let fallback_url = info.html_url.clone();

        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let install_res = runtime
                .spawn(async move {
                    rdm_core::updater::download_and_install(&info_download, progress_download).await
                })
                .await;

            match install_res {
                Ok(Ok(())) => {
                    let _ = entity.update(cx, |this, cx| {
                        this.active_update_progress = None;
                        cx.quit();
                    });
                }
                Ok(Err(e)) => {
                    let _ = entity.update(cx, |this, cx| {
                        this.active_update_progress = None;
                        this.updater_status = UpdaterStatus::Error {
                            message: format_update_error(e),
                            release_url: Some(fallback_url),
                        };
                        cx.notify();
                        cx.refresh_windows();
                    });
                }
                Err(e) => {
                    let _ = entity.update(cx, |this, cx| {
                        this.active_update_progress = None;
                        this.updater_status = UpdaterStatus::Error {
                            message: e.to_string(),
                            release_url: Some(fallback_url),
                        };
                        cx.notify();
                        cx.refresh_windows();
                    });
                }
            }
        })
        .detach();
    }

    pub fn tasks(&self) -> &[TaskSnapshot] {
        &self.tasks
    }

    pub fn task(&self, id: Uuid) -> Option<&TaskSnapshot> {
        self.tasks.iter().find(|task| task.id == id)
    }

    pub(crate) fn download_history(&self) -> Option<&DownloadHistory> {
        self.download_history.as_ref()
    }

    pub(crate) fn tokio_handle(&self) -> &tokio::runtime::Handle {
        self.runtime.handle()
    }

    fn emit_notifications(&self, events: Vec<NotificationKind>, cx: &mut Context<Self>) {
        if events.is_empty() {
            return;
        }
        let Some(notifications) = cx
            .try_global::<AppState>()
            .map(|state| state.notifications.clone())
        else {
            return;
        };
        let os_enabled = self.core.config().ui_config().system_notifications;
        for kind in events {
            let (title, body) = match &kind {
                NotificationKind::DownloadStarted { file_name, .. } => (
                    cx.lang().get("notifications.download_started").to_string(),
                    file_name.clone(),
                ),
                NotificationKind::DownloadCompleted { file_name, .. } => (
                    cx.lang()
                        .get("notifications.download_completed")
                        .to_string(),
                    file_name.clone(),
                ),
                NotificationKind::DownloadFailed {
                    file_name, error, ..
                } => (
                    cx.lang().get("notifications.download_failed").to_string(),
                    format!("{file_name}: {error}"),
                ),
                NotificationKind::UpdateAvailable { info } => (
                    cx.lang().get("notifications.update_available").to_string(),
                    cx.lang()
                        .get_with("titlebar.update_available", &[("version", &info.version)])
                        .to_string(),
                ),
            };
            notify_entity(&notifications, kind, title, body, os_enabled, cx);
        }
    }

    fn refresh_snapshots(&mut self, live_tasks: Vec<Arc<Task>>, update_speed_sample: bool) -> bool {
        self.refresh_snapshots_with_events(live_tasks, update_speed_sample)
            .0
    }

    fn refresh_snapshots_with_events(
        &mut self,
        live_tasks: Vec<Arc<Task>>,
        update_speed_sample: bool,
    ) -> (bool, Vec<NotificationKind>) {
        let now = Instant::now();
        let elapsed_secs = now
            .duration_since(self.last_sample_at)
            .as_secs_f64()
            .max(0.1);

        let mut new_snapshots = Vec::with_capacity(live_tasks.len());
        let mut new_previous_bytes = Vec::with_capacity(live_tasks.len());

        for task in live_tasks {
            let downloaded = task.downloaded_bytes.load(Ordering::Relaxed);
            let status = task.status();

            let prev = self
                .previous_bytes
                .iter()
                .find(|(id, _)| *id == task.id)
                .map(|(_, b)| *b)
                .unwrap_or(downloaded);

            let prev_speed = self
                .tasks
                .iter()
                .find(|s| s.id == task.id)
                .map(|s| s.speed_bytes_per_sec)
                .unwrap_or(0);

            let speed = if status == TaskStatus::Downloading {
                if update_speed_sample {
                    let delta = downloaded.saturating_sub(prev) as f64;
                    (delta / elapsed_secs) as u64
                } else {
                    prev_speed
                }
            } else {
                0
            };

            if update_speed_sample {
                new_previous_bytes.push((task.id, downloaded));
            }

            let file_name = task
                .output_path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("download.bin")
                .to_string();

            let (status, completed_at) = task.status_and_completion();
            let segments = task
                .segments()
                .into_iter()
                .map(|segment| SegmentSnapshot {
                    index: segment.index,
                    start_byte: segment.start_byte,
                    end_byte: segment.end_byte,
                    downloaded_bytes: segment.downloaded_bytes.load(Ordering::Relaxed),
                    status: segment.status(),
                })
                .collect();

            new_snapshots.push(TaskSnapshot {
                id: task.id,
                file_name,
                url: task.url(),
                output_path: task.output_path.clone(),
                total_size: task.total_size,
                downloaded_bytes: downloaded,
                speed_bytes_per_sec: speed,
                status,
                progress: task.progress_ratio(),
                connections: task.connections,
                max_speed_bytes_per_sec: task.max_speed_bytes_per_sec(),
                checksum: task.checksum.clone(),
                referer: task.referer(),
                completed_at,
                segments,
            });
        }

        let mut events = Vec::new();
        let mut next_statuses = HashMap::with_capacity(new_snapshots.len());
        for snap in &new_snapshots {
            if self.initialized_statuses {
                let prev = self.previous_statuses.get(&snap.id);
                if prev != Some(&snap.status) {
                    match &snap.status {
                        TaskStatus::Downloading => {
                            events.push(NotificationKind::DownloadStarted {
                                task_id: snap.id,
                                file_name: snap.file_name.clone(),
                            });
                        }
                        TaskStatus::Completed => {
                            events.push(NotificationKind::DownloadCompleted {
                                task_id: snap.id,
                                file_name: snap.file_name.clone(),
                                output_path: snap.output_path.clone(),
                            });
                        }
                        TaskStatus::Failed(reason) | TaskStatus::ConversionFailed(reason) => {
                            events.push(NotificationKind::DownloadFailed {
                                task_id: snap.id,
                                file_name: snap.file_name.clone(),
                                url: snap.url.clone(),
                                output_path: snap.output_path.clone(),
                                error: reason.clone(),
                            });
                        }
                        _ => {}
                    }
                }
            }
            next_statuses.insert(snap.id, snap.status.clone());
        }
        self.previous_statuses = next_statuses;
        self.initialized_statuses = true;

        if update_speed_sample {
            self.previous_bytes = new_previous_bytes;
            self.last_sample_at = now;
        }
        let changed = if self.tasks == new_snapshots {
            false
        } else {
            self.tasks = new_snapshots;
            true
        };
        (changed, events)
    }

    pub fn start_download(
        &self,
        url: String,
        output_path: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.start_configured_download(
            DownloadRequest {
                url,
                output_path,
                ..Default::default()
            },
            cx,
        )
        .detach();
    }

    pub fn start_configured_download(
        &self,
        request: DownloadRequest,
        cx: &mut Context<Self>,
    ) -> gpui::Task<DownloadStartResult> {
        let retry_task = request.retry_task;
        let core = Arc::clone(&self.core);
        let runtime = Arc::clone(&self.runtime);
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let outcome = runtime
                .spawn(async move {
                    let mut task_id = request.retry_task;
                    let result = async {
                        if task_id.is_none() {
                            if let Some(directory) = &request.directory {
                                core.disk().ensure_directory(directory).await?;
                            }
                            let task = core
                                .engine()
                                .create_task_configured(
                                    &request.url,
                                    rdm_core::engine::TaskOptions {
                                        output_path: request.output_path,
                                        connections: request.connections,
                                        referer: request.referer,
                                        post_process: request.post_process,
                                        ..request.network_options
                                    },
                                )
                                .await?;
                            task_id = Some(task.id);
                        }
                        if let Some(id) = task_id {
                            core.engine().start_task(id).await?;
                        }
                        Ok::<(), rdm_core::EngineError>(())
                    }
                    .await;
                    (
                        DownloadStartResult {
                            task_id,
                            error: result.err().map(|error| error.to_string()),
                        },
                        core.engine().all_tasks().await,
                    )
                })
                .await;
            match outcome {
                Ok((result, all)) => {
                    let _ = entity.update(cx, |this, cx| {
                        this.refresh_snapshots(all, false);
                        cx.notify();
                        cx.refresh_windows();
                    });
                    result
                }
                Err(error) => DownloadStartResult {
                    task_id: retry_task,
                    error: Some(error.to_string()),
                },
            }
        })
    }

    pub fn pause_task(&self, task_id: Uuid, cx: &mut Context<Self>) {
        let core = Arc::clone(&self.core);
        let runtime = Arc::clone(&self.runtime);
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let core_task = Arc::clone(&core);
            let all = runtime
                .spawn(async move {
                    let _ = core_task.engine().pause_task(task_id).await;
                    core_task.engine().all_tasks().await
                })
                .await
                .unwrap_or_default();

            let _ = entity.update(cx, |this, cx| {
                this.refresh_snapshots(all, false);
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
    }

    pub fn resume_task(&self, task_id: Uuid, cx: &mut Context<Self>) {
        self.restart_task(task_id, false, cx);
    }

    pub fn retry_conversion(&self, task_id: Uuid, cx: &mut Context<Self>) {
        self.restart_task(task_id, true, cx);
    }

    fn restart_task(&self, task_id: Uuid, conversion_only: bool, cx: &mut Context<Self>) {
        let core = Arc::clone(&self.core);
        let runtime = Arc::clone(&self.runtime);
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let core_task = Arc::clone(&core);
            let all = runtime
                .spawn(async move {
                    let engine = core_task.engine();
                    let _ = if conversion_only {
                        engine.retry_conversion(task_id).await
                    } else {
                        engine.resume_task(task_id).await
                    };
                    core_task.engine().all_tasks().await
                })
                .await
                .unwrap_or_default();

            let _ = entity.update(cx, |this, cx| {
                this.refresh_snapshots(all, false);
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
    }

    pub fn delete_task(&self, task_id: Uuid, delete_file: bool, cx: &mut Context<Self>) {
        let core = Arc::clone(&self.core);
        let runtime = Arc::clone(&self.runtime);
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let core_task = Arc::clone(&core);
            let all = runtime
                .spawn(async move {
                    let _ = core_task.engine().delete_task(task_id, delete_file).await;
                    core_task.engine().all_tasks().await
                })
                .await
                .unwrap_or_default();

            let _ = entity.update(cx, |this, cx| {
                this.refresh_snapshots(all, false);
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
    }
}

pub struct AppState {
    pub core: Arc<Core>,
    pub tasks_model: Entity<TasksModel>,
    pub notifications: Entity<NotificationsModel>,
}

impl AppState {
    pub fn new(
        core: Arc<Core>,
        tasks_model: Entity<TasksModel>,
        notifications: Entity<NotificationsModel>,
    ) -> Self {
        Self {
            core,
            tasks_model,
            notifications,
        }
    }
}

impl Global for AppState {}

pub trait CoreContext {
    fn core(&self) -> &Arc<Core>;
    fn engine(&self) -> &Arc<Engine> {
        self.core().engine()
    }
    fn tasks_model(&self) -> &Entity<TasksModel>;
    fn notifications(&self) -> &Entity<NotificationsModel>;
}

impl<'a, T> CoreContext for Context<'a, T> {
    fn core(&self) -> &Arc<Core> {
        &self.global::<AppState>().core
    }

    fn tasks_model(&self) -> &Entity<TasksModel> {
        &self.global::<AppState>().tasks_model
    }

    fn notifications(&self) -> &Entity<NotificationsModel> {
        &self.global::<AppState>().notifications
    }
}

impl CoreContext for App {
    fn core(&self) -> &Arc<Core> {
        &self.global::<AppState>().core
    }

    fn tasks_model(&self) -> &Entity<TasksModel> {
        &self.global::<AppState>().tasks_model
    }

    fn notifications(&self) -> &Entity<NotificationsModel> {
        &self.global::<AppState>().notifications
    }
}

fn format_update_error(error: rdm_core::EngineError) -> String {
    match error {
        rdm_core::EngineError::UpdateError(message) => message,
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn download_history_counts_only_dated_completions_in_the_calendar_window()
    -> Result<(), Box<dyn std::error::Error>> {
        let today = NaiveDate::from_ymd_opt(2026, 11, 9).ok_or("invalid date")?;
        let make_task = |status, days_ago: Option<u64>| -> Option<Arc<Task>> {
            let task = Task::new(Uuid::new_v4(), String::new(), PathBuf::new(), 0, 1, None);
            let mut stored = task.to_persisted();
            stored.status = status;
            stored.completed_at = match days_ago {
                Some(days) => {
                    let time = today
                        .checked_sub_days(Days::new(days))?
                        .and_hms_opt(12, 0, 0)?;
                    Some(
                        u64::try_from(Local.from_local_datetime(&time).single()?.timestamp())
                            .ok()?,
                    )
                }
                None => None,
            };
            Some(Arc::new(Task::from_persisted(stored)))
        };
        let tasks = [
            make_task(TaskStatus::Completed, Some(0)).ok_or("missing completion")?,
            make_task(TaskStatus::Completed, Some(0)).ok_or("missing completion")?,
            make_task(TaskStatus::Completed, Some(364)).ok_or("missing oldest completion")?,
            make_task(TaskStatus::Completed, Some(365)).ok_or("missing excluded completion")?,
            make_task(TaskStatus::Completed, None).ok_or("missing legacy completion")?,
            make_task(TaskStatus::Paused, Some(0)).ok_or("missing paused task")?,
        ];
        let history =
            DownloadHistory::from_tasks(&tasks, today).ok_or("invalid calendar window")?;
        assert_eq!(history.counts[history.first_cell], 1);
        assert_eq!(history.counts[history.last_cell], 2);
        assert_eq!(history.total, 3);
        assert_eq!(history.undated, 1);
        Ok(())
    }
}
