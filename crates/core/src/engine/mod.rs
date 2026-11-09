mod download;
mod multimedia;
mod transfer;

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use rdm_config::ConfigManager;
use uuid::Uuid;

use crate::{
    disk::DiskManager,
    error::EngineError,
    task::{PostProcessAction, Task},
};

use download::DownloadModule;
use multimedia::MultimediaModule;
pub(crate) use multimedia::normalize_post_process;
pub use multimedia::{YouTubeFormat, extract_youtube_video_id};

#[derive(Debug, Clone)]
pub struct ResourceProbe {
    pub url: String,
    pub file_name: String,
    pub total_size: u64,
    pub supports_ranges: bool,
    pub content_type: Option<String>,
    pub post_process: Option<PostProcessAction>,
}

pub trait EngineModule {
    fn probe(
        &self,
        url: &str,
        referer: Option<&str>,
    ) -> impl Future<Output = Result<Option<ResourceProbe>, EngineError>> + Send;
}

#[derive(Clone)]
pub struct Engine {
    download: Arc<DownloadModule>,
    multimedia: Arc<MultimediaModule>,
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

        let multimedia = Arc::new(MultimediaModule::new(
            client.clone(),
            Arc::clone(&config),
            Arc::clone(&disk_manager),
        ));
        let download = Arc::new(DownloadModule::new(
            config,
            disk_manager,
            client,
            Arc::clone(&multimedia),
        ));

        Ok(Self {
            download,
            multimedia,
        })
    }

    pub fn config(&self) -> &Arc<ConfigManager> {
        self.download.config()
    }

    pub fn disk_manager(&self) -> &Arc<DiskManager> {
        self.download.disk_manager()
    }

    pub async fn save_state(&self) -> Result<(), EngineError> {
        self.download.save_state().await
    }

    pub async fn load_state(&self) -> Result<(), EngineError> {
        self.download.load_state().await
    }

    pub async fn probe(
        &self,
        url: &str,
        referer: Option<&str>,
    ) -> Result<ResourceProbe, EngineError> {
        if let Some(probe) = self.multimedia.probe(url, referer).await? {
            return Ok(probe);
        }
        self.download
            .probe(url, referer)
            .await?
            .ok_or(EngineError::HttpStatus(reqwest::StatusCode::NOT_FOUND))
    }

    pub async fn resolve_youtube_formats(
        &self,
        video_id: &str,
    ) -> Result<Vec<YouTubeFormat>, EngineError> {
        self.multimedia.resolve_youtube_formats(video_id).await
    }

    pub(crate) async fn infer_post_process(
        &self,
        url: &str,
        filename: Option<&str>,
        referer: Option<&str>,
    ) -> Option<PostProcessAction> {
        self.multimedia
            .infer_post_process(url, filename, referer)
            .await
    }

    pub async fn create_task(
        &self,
        url: &str,
        output_path: Option<PathBuf>,
        connections: Option<usize>,
        referer: Option<String>,
    ) -> Result<Arc<Task>, EngineError> {
        self.create_task_with_options(url, output_path, connections, referer, None)
            .await
    }

    pub async fn create_task_with_options(
        &self,
        url: &str,
        output_path: Option<PathBuf>,
        connections: Option<usize>,
        referer: Option<String>,
        explicit_post_process: Option<PostProcessAction>,
    ) -> Result<Arc<Task>, EngineError> {
        let probe = self.probe(url, referer.as_deref()).await?;
        self.download
            .create_task(
                probe,
                output_path,
                connections,
                referer,
                explicit_post_process,
            )
            .await
    }

    pub async fn start_task(&self, task_id: Uuid) -> Result<(), EngineError> {
        self.download.start_task(task_id).await
    }

    pub fn process_queue(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.download.process_queue()
    }

    pub async fn resume_task(&self, task_id: Uuid) -> Result<(), EngineError> {
        self.start_task(task_id).await
    }

    pub async fn retry_conversion(&self, task_id: Uuid) -> Result<(), EngineError> {
        self.download.retry_conversion(task_id).await
    }

    pub async fn pause_task(&self, task_id: Uuid) -> Result<(), EngineError> {
        self.download.pause_task(task_id).await
    }

    pub async fn cancel_task(&self, task_id: Uuid) -> Result<(), EngineError> {
        self.download.cancel_task(task_id).await
    }

    pub async fn delete_task(&self, task_id: Uuid, delete_file: bool) -> Result<(), EngineError> {
        self.download.delete_task(task_id, delete_file).await
    }

    pub async fn get_task(&self, task_id: Uuid) -> Result<Arc<Task>, EngineError> {
        self.download.get_task(task_id).await
    }

    pub async fn all_tasks(&self) -> Vec<Arc<Task>> {
        self.download.all_tasks().await
    }
}
