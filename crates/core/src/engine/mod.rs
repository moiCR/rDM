mod download;
mod multimedia;
mod transfer;

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use rdm_config::{ConfigManager, EngineConfig, ProxyConfig};
use uuid::Uuid;

use crate::{
    disk::DiskManager,
    error::EngineError,
    task::{ExpectedChecksum, PostProcessAction, Task},
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

#[derive(Debug, Clone, Default)]
pub struct TaskOptions {
    pub output_path: Option<PathBuf>,
    pub connections: Option<usize>,
    pub referer: Option<String>,
    pub post_process: Option<PostProcessAction>,
    pub checksum: Option<ExpectedChecksum>,
    pub headers: Vec<(String, String)>,
    pub cookies: Option<String>,
    pub max_speed_bytes_per_sec: Option<u64>,
}

#[derive(Clone)]
pub(super) struct HttpClientProvider {
    config: Arc<ConfigManager>,
    cached: Arc<std::sync::RwLock<(ProxyConfig, String, reqwest::Client)>>,
}

impl HttpClientProvider {
    pub(super) fn new(config: Arc<ConfigManager>) -> Result<Self, EngineError> {
        let engine_config = config.config();
        let client = build_http_client(&engine_config)?;
        Ok(Self {
            config,
            cached: Arc::new(std::sync::RwLock::new((
                engine_config.proxy,
                engine_config.user_agent,
                client,
            ))),
        })
    }

    pub(super) fn client(&self) -> Result<reqwest::Client, EngineError> {
        let engine_config = self.config.config();
        {
            let guard = self
                .cached
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if guard.0 == engine_config.proxy && guard.1 == engine_config.user_agent {
                return Ok(guard.2.clone());
            }
        }

        let new_client = build_http_client(&engine_config)?;
        let mut guard = self
            .cached
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = (
            engine_config.proxy,
            engine_config.user_agent,
            new_client.clone(),
        );
        Ok(new_client)
    }
}

pub(super) fn build_http_client(
    engine_config: &EngineConfig,
) -> Result<reqwest::Client, EngineError> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Ok(user_agent_value) = reqwest::header::HeaderValue::from_str(&engine_config.user_agent)
    {
        headers.insert(reqwest::header::USER_AGENT, user_agent_value);
    }

    let mut builder = reqwest::Client::builder()
        .tcp_nodelay(true)
        .pool_max_idle_per_host(32)
        .default_headers(headers)
        .user_agent(&engine_config.user_agent);

    if engine_config.proxy.enabled && !engine_config.proxy.url.trim().is_empty() {
        let mut proxy = reqwest::Proxy::all(engine_config.proxy.url.trim())?;
        if let (Some(username), Some(password)) =
            (&engine_config.proxy.username, &engine_config.proxy.password)
            && !username.is_empty()
        {
            proxy = proxy.basic_auth(username, password);
        }
        builder = builder.proxy(proxy);
    }

    Ok(builder.build()?)
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
        let client_provider = HttpClientProvider::new(Arc::clone(&config))?;
        let client = client_provider.client()?;

        let multimedia = Arc::new(MultimediaModule::new(
            client,
            Arc::clone(&config),
            Arc::clone(&disk_manager),
        ));
        let download = Arc::new(DownloadModule::new(
            config,
            disk_manager,
            client_provider,
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
        self.probe_with_headers(url, referer, &[], None).await
    }

    pub async fn probe_with_headers(
        &self,
        url: &str,
        referer: Option<&str>,
        headers: &[(String, String)],
        cookies: Option<&str>,
    ) -> Result<ResourceProbe, EngineError> {
        if let Some(probe) = self.multimedia.probe(url, referer).await? {
            return Ok(probe);
        }
        self.download
            .probe_with_headers(url, referer, headers, cookies)
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
        self.create_task_configured(
            url,
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

    pub async fn create_task_configured(
        &self,
        url: &str,
        options: TaskOptions,
    ) -> Result<Arc<Task>, EngineError> {
        let probe = self
            .probe_with_headers(
                url,
                options.referer.as_deref(),
                &options.headers,
                options.cookies.as_deref(),
            )
            .await?;
        self.download.create_task_configured(probe, options).await
    }

    pub async fn refresh_task_url(
        &self,
        task_id: Uuid,
        new_url: &str,
        new_referer: Option<String>,
    ) -> Result<(), EngineError> {
        self.download
            .refresh_task_url(task_id, new_url, new_referer)
            .await
    }

    pub async fn set_task_speed_limit(
        &self,
        task_id: Uuid,
        bytes_per_sec: u64,
    ) -> Result<(), EngineError> {
        self.download
            .set_task_speed_limit(task_id, bytes_per_sec)
            .await
    }

    pub fn set_global_speed_limit(&self, bytes_per_sec: u64) -> Result<(), EngineError> {
        self.config().update_config(|cfg| {
            cfg.max_speed_bytes_per_sec = bytes_per_sec;
        })?;
        Ok(())
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
