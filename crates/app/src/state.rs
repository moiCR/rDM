use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use chrono::{DateTime, Datelike, Days, Local, NaiveDate};
use gpui::{App, AsyncApp, Context, Entity, Global, WeakEntity};
use rdm_core::{
    Core, Engine,
    task::{Task, TaskStatus},
    updater::{PackageKind, ReleaseInfo, UpdateProgress},
};
use uuid::Uuid;

use crate::views::NewDownloadModal;

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
    Downloaded {
        path: PathBuf,
        package: PackageKind,
        error: Option<String>,
    },
    Installing,
    Error(String),
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
}

pub struct DownloadStartResult {
    pub task_id: Option<Uuid>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
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
}

pub struct TasksModel {
    core: Arc<Core>,
    runtime: Arc<tokio::runtime::Runtime>,
    tasks: Vec<TaskSnapshot>,
    previous_bytes: Vec<(Uuid, u64)>,
    last_sample_at: Instant,
    download_history: Option<DownloadHistory>,
    updater_status: UpdaterStatus,
    active_update_progress: Option<Arc<UpdateProgress>>,
}

pub(crate) const HISTORY_WEEKS: usize = 53;
pub(crate) const HISTORY_WEEKDAYS: usize = 7;
const HISTORY_DAYS: usize = 365;

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
            last_sample_at: Instant::now(),
            download_history: None,
            updater_status: UpdaterStatus::Idle,
            active_update_progress: None,
        };

        if auto_check {
            cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let _ = entity.update(cx, |this, cx| {
                    this.check_for_updates(cx);
                });
            })
            .detach();
        }

        let mut intercept_rx = core.subscribe_intercepted();
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                match intercept_rx.recv().await {
                    Ok(intercepted) => {
                        let res = entity.update(cx, |_, cx| {
                            #[cfg(target_os = "linux")]
                            if let Err(error) = crate::linux_desktop::open_window(cx) {
                                eprintln!("No se pudo abrir la descarga interceptada: {error}");
                                return;
                            }
                            cx.activate(true);
                            if let Some(handle) = cx.windows().into_iter().next() {
                                let _ = handle.update(cx, |_, window, cx| {
                                    #[cfg(windows)]
                                    crate::windows_desktop::restore_window(window);
                                    #[cfg(not(windows))]
                                    window.activate_window();
                                    NewDownloadModal::open(Some(intercepted), window, cx);
                                    window.refresh();
                                });
                            }
                        });
                        if res.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        })
        .detach();

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

                let update_result = entity.update(cx, |this, cx| {
                    if let Some(progress) = &this.active_update_progress
                        && let UpdaterStatus::Downloading { ref info, .. } = this.updater_status
                    {
                        let downloaded = progress.downloaded_bytes.load(Ordering::Relaxed);
                        let total = progress.total_bytes.load(Ordering::Relaxed);
                        this.updater_status = UpdaterStatus::Downloading {
                            info: info.clone(),
                            downloaded_bytes: downloaded,
                            total_bytes: total,
                        };
                    }
                    this.download_history = history;
                    this.refresh_snapshots(tasks, true);
                    cx.notify();
                    cx.refresh_windows();
                });

                if update_result.is_err() {
                    break;
                }
            }
        })
        .detach();

        model
    }

    pub fn updater_status(&self) -> &UpdaterStatus {
        &self.updater_status
    }

    pub fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.updater_status,
            UpdaterStatus::Checking | UpdaterStatus::Downloading { .. } | UpdaterStatus::Installing
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
                Ok(Ok(None)) => UpdaterStatus::UpToDate,
                Ok(Err(e)) => UpdaterStatus::Error(e.to_string()),
                Err(e) => UpdaterStatus::Error(e.to_string()),
            };

            let _ = entity.update(cx, |this, cx| {
                this.updater_status = status;
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
        let Some(installer_url) = info.installer_url.clone() else {
            cx.open_url(&info.html_url);
            return;
        };

        let progress = Arc::new(UpdateProgress::default());
        self.active_update_progress = Some(Arc::clone(&progress));
        self.updater_status = UpdaterStatus::Downloading {
            info: info.clone(),
            downloaded_bytes: 0,
            total_bytes: info.installer_size,
        };
        cx.notify();
        cx.refresh_windows();

        let Some(installer_name) = info.installer_name.clone().filter(|name| {
            !name.contains(['/', '\\']) && !matches!(name.as_str(), "" | "." | "..")
        }) else {
            self.active_update_progress = None;
            self.updater_status = UpdaterStatus::Error("El nombre del paquete no es válido".into());
            cx.notify();
            return;
        };
        let temp_target = std::env::temp_dir()
            .join(format!("rdm-update-{}", Uuid::new_v4()))
            .join(installer_name);
        let expected_size = info.installer_size;
        #[cfg(target_os = "linux")]
        let package = info.package;

        let runtime = Arc::clone(&self.runtime);
        let progress_download = Arc::clone(&progress);
        let target_download = temp_target.clone();

        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let download_res = runtime
                .spawn(async move {
                    rdm_core::updater::download_installer(
                        &installer_url,
                        &target_download,
                        progress_download,
                        expected_size,
                    )
                    .await
                })
                .await;

            match download_res {
                Ok(Ok(())) => {
                    #[cfg(target_os = "linux")]
                    let _ = entity.update(cx, |this, cx| {
                        this.active_update_progress = None;
                        this.updater_status = UpdaterStatus::Downloaded {
                            path: temp_target,
                            package,
                            error: None,
                        };
                        cx.notify();
                        cx.refresh_windows();
                    });
                    #[cfg(not(target_os = "linux"))]
                    {
                        let _ = entity.update(cx, |this, cx| {
                            this.updater_status = UpdaterStatus::Installing;
                            cx.notify();
                            cx.refresh_windows();
                        });
                        if let Err(error) = rdm_core::updater::launch_installer(&temp_target) {
                            let _ = entity.update(cx, |this, cx| {
                                this.active_update_progress = None;
                                this.updater_status = UpdaterStatus::Error(error.to_string());
                                cx.notify();
                                cx.refresh_windows();
                            });
                        } else {
                            let _ = entity.update(cx, |_, cx| cx.quit());
                        }
                    }
                }
                Ok(Err(e)) => {
                    let _ = entity.update(cx, |this, cx| {
                        this.updater_status = UpdaterStatus::Error(e.to_string());
                        this.active_update_progress = None;
                        cx.notify();
                        cx.refresh_windows();
                    });
                }
                Err(e) => {
                    let _ = entity.update(cx, |this, cx| {
                        this.updater_status = UpdaterStatus::Error(e.to_string());
                        this.active_update_progress = None;
                        cx.notify();
                        cx.refresh_windows();
                    });
                }
            }
        })
        .detach();
    }

    pub fn open_downloaded_update(&mut self, cx: &mut Context<Self>) {
        let UpdaterStatus::Downloaded {
            path,
            package,
            error,
        } = &mut self.updater_status
        else {
            return;
        };
        if !matches!(package, PackageKind::Deb | PackageKind::Rpm) {
            cx.reveal_path(path);
            return;
        }
        *error = None;
        let child = match std::process::Command::new("xdg-open").arg(&path).spawn() {
            Ok(child) => child,
            Err(failure) => {
                *error = Some(failure.to_string());
                cx.notify();
                cx.refresh_windows();
                return;
            }
        };
        let target = path.clone();
        let work = self.runtime.spawn_blocking(move || {
            let mut child = child;
            let status = child.wait()?;
            if status.success() {
                Ok(())
            } else {
                Err(std::io::Error::other(
                    "No se pudo abrir el paquete. Puedes instalarlo desde su carpeta.",
                ))
            }
        });
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let result = work
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result.map_err(|error| error.to_string()));
            let _ = entity.update(cx, |this, cx| {
                if let UpdaterStatus::Downloaded { path, error, .. } = &mut this.updater_status
                    && *path == target
                {
                    *error = result.err();
                    cx.notify();
                    cx.refresh_windows();
                }
            });
        })
        .detach();
    }

    pub fn tasks(&self) -> &[TaskSnapshot] {
        &self.tasks
    }

    pub(crate) fn download_history(&self) -> Option<&DownloadHistory> {
        self.download_history.as_ref()
    }

    pub(crate) fn tokio_handle(&self) -> &tokio::runtime::Handle {
        self.runtime.handle()
    }

    fn refresh_snapshots(&mut self, live_tasks: Vec<Arc<Task>>, update_speed_sample: bool) {
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

            new_snapshots.push(TaskSnapshot {
                id: task.id,
                file_name,
                url: task.url.clone(),
                output_path: task.output_path.clone(),
                total_size: task.total_size,
                downloaded_bytes: downloaded,
                speed_bytes_per_sec: speed,
                status,
                progress: task.progress_ratio(),
            });
        }

        if update_speed_sample {
            self.previous_bytes = new_previous_bytes;
            self.last_sample_at = now;
        }
        self.tasks = new_snapshots;
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
                                .create_task_with_options(
                                    &request.url,
                                    request.output_path,
                                    request.connections,
                                    request.referer,
                                    request.post_process,
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
}

impl AppState {
    pub fn new(core: Arc<Core>, tasks_model: Entity<TasksModel>) -> Self {
        Self { core, tasks_model }
    }
}

impl Global for AppState {}

pub trait CoreContext {
    fn core(&self) -> &Arc<Core>;
    fn engine(&self) -> &Arc<Engine> {
        self.core().engine()
    }
    fn tasks_model(&self) -> &Entity<TasksModel>;
}

impl<'a, T> CoreContext for Context<'a, T> {
    fn core(&self) -> &Arc<Core> {
        &self.global::<AppState>().core
    }

    fn tasks_model(&self) -> &Entity<TasksModel> {
        &self.global::<AppState>().tasks_model
    }
}

impl CoreContext for App {
    fn core(&self) -> &Arc<Core> {
        &self.global::<AppState>().core
    }

    fn tasks_model(&self) -> &Entity<TasksModel> {
        &self.global::<AppState>().tasks_model
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
