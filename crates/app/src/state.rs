use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use gpui::{App, AsyncApp, Context, Entity, Global, WeakEntity};
use rdm_core::{
    Core, Engine,
    task::{Task, TaskStatus},
};
use uuid::Uuid;

#[derive(Default, Clone, Copy)]
pub struct QuickDownloadState {
    pub expanded: bool,
}

impl Global for QuickDownloadState {}

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
}

impl TasksModel {
    pub fn new(
        core: Arc<Core>,
        runtime: Arc<tokio::runtime::Runtime>,
        cx: &mut Context<Self>,
    ) -> Self {
        let model = Self {
            core: Arc::clone(&core),
            runtime: Arc::clone(&runtime),
            tasks: Vec::new(),
            previous_bytes: Vec::new(),
        };

        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let core_init = Arc::clone(&core);
            let initial_tasks = runtime
                .spawn(async move { core_init.engine().all_tasks().await })
                .await
                .unwrap_or_default();

            let _ = entity.update(cx, |this, cx| {
                this.refresh_snapshots(initial_tasks);
                cx.notify();
                cx.refresh_windows();
            });

            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;

                let core_loop = Arc::clone(&core);
                let tasks = runtime
                    .spawn(async move { core_loop.engine().all_tasks().await })
                    .await
                    .unwrap_or_default();

                let update_result = entity.update(cx, |this, cx| {
                    this.refresh_snapshots(tasks);
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

    pub fn tasks(&self) -> &[TaskSnapshot] {
        &self.tasks
    }

    fn refresh_snapshots(&mut self, live_tasks: Vec<Arc<Task>>) {
        let mut new_snapshots = Vec::with_capacity(live_tasks.len());
        let mut new_previous_bytes = Vec::with_capacity(live_tasks.len());

        for task in live_tasks {
            let downloaded = task.downloaded_bytes.load(Ordering::Relaxed);
            let prev = self
                .previous_bytes
                .iter()
                .find(|(id, _)| *id == task.id)
                .map(|(_, b)| *b)
                .unwrap_or(downloaded);

            let speed = downloaded.saturating_sub(prev) * 2;
            new_previous_bytes.push((task.id, downloaded));

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
                status: task.status(),
                progress: task.progress_ratio(),
            });
        }

        self.previous_bytes = new_previous_bytes;
        self.tasks = new_snapshots;
    }

    pub fn start_download(
        &self,
        url: String,
        output_path: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let core = Arc::clone(&self.core);
        let runtime = Arc::clone(&self.runtime);
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let core_task = Arc::clone(&core);
            let all = runtime
                .spawn(async move {
                    if let Ok(task) = core_task
                        .engine()
                        .create_task(&url, output_path, None, None)
                        .await
                    {
                        let _ = core_task.engine().start_task(task.id).await;
                    }
                    core_task.engine().all_tasks().await
                })
                .await
                .unwrap_or_default();

            let _ = entity.update(cx, |this, cx| {
                this.refresh_snapshots(all);
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
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
                this.refresh_snapshots(all);
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();
    }

    pub fn resume_task(&self, task_id: Uuid, cx: &mut Context<Self>) {
        let core = Arc::clone(&self.core);
        let runtime = Arc::clone(&self.runtime);
        cx.spawn(async move |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            let core_task = Arc::clone(&core);
            let all = runtime
                .spawn(async move {
                    let _ = core_task.engine().resume_task(task_id).await;
                    core_task.engine().all_tasks().await
                })
                .await
                .unwrap_or_default();

            let _ = entity.update(cx, |this, cx| {
                this.refresh_snapshots(all);
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
                this.refresh_snapshots(all);
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
