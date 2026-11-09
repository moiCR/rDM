use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_connections() -> usize {
    8
}

fn default_max_concurrent_downloads() -> usize {
    3
}

fn default_download_dir() -> PathBuf {
    dirs::download_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn default_state_dir() -> PathBuf {
    dirs::data_local_dir()
        .map(|dir| dir.join("rdm"))
        .unwrap_or_else(|| PathBuf::from(".rdm"))
}

fn default_max_retries() -> usize {
    8
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_server_enabled() -> bool {
    true
}

fn default_server_port() -> u16 {
    9654
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct EngineConfig {
    #[serde(default = "default_connections")]
    pub default_connections: usize,
    #[serde(default = "default_max_concurrent_downloads")]
    pub max_concurrent_downloads: usize,
    #[serde(default = "default_download_dir")]
    pub download_dir: PathBuf,
    #[serde(default = "default_state_dir")]
    pub state_dir: PathBuf,
    #[serde(default = "get_user_agent")]
    pub user_agent: String,
    #[serde(default)]
    pub max_speed_bytes_per_sec: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_server_enabled")]
    pub server_enabled: bool,
    #[serde(default = "default_server_port")]
    pub server_port: u16,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            default_connections: default_connections(),
            max_concurrent_downloads: default_max_concurrent_downloads(),
            download_dir: default_download_dir(),
            state_dir: default_state_dir(),
            user_agent: get_user_agent(),
            max_speed_bytes_per_sec: 0,
            max_retries: default_max_retries(),
            theme: default_theme(),
            server_enabled: default_server_enabled(),
            server_port: default_server_port(),
        }
    }
}

impl EngineConfig {
    pub fn new(
        default_connections: usize,
        download_dir: PathBuf,
        state_dir: PathBuf,
        user_agent: String,
    ) -> Self {
        Self {
            default_connections: default_connections.clamp(1, 32),
            max_concurrent_downloads: default_max_concurrent_downloads(),
            download_dir,
            state_dir,
            user_agent,
            max_speed_bytes_per_sec: 0,
            max_retries: default_max_retries(),
            theme: default_theme(),
            server_enabled: default_server_enabled(),
            server_port: default_server_port(),
        }
    }

    pub fn with_download_dir(mut self, download_dir: PathBuf) -> Self {
        self.download_dir = download_dir;
        self
    }

    pub fn with_state_dir(mut self, state_dir: PathBuf) -> Self {
        self.state_dir = state_dir;
        self
    }

    pub fn with_default_connections(mut self, connections: usize) -> Self {
        self.default_connections = connections.clamp(1, 32);
        self
    }

    pub fn with_max_concurrent_downloads(mut self, max_concurrent_downloads: usize) -> Self {
        self.max_concurrent_downloads = max_concurrent_downloads.clamp(1, 16);
        self
    }

    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = user_agent;
        self
    }

    pub fn with_max_speed_bytes_per_sec(mut self, max_speed_bytes_per_sec: u64) -> Self {
        self.max_speed_bytes_per_sec = max_speed_bytes_per_sec;
        self
    }

    pub fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries.max(1);
        self
    }

    pub fn with_theme(mut self, theme: impl Into<String>) -> Self {
        self.theme = theme.into();
        self
    }

    pub fn with_server_enabled(mut self, enabled: bool) -> Self {
        self.server_enabled = enabled;
        self
    }

    pub fn with_server_port(mut self, port: u16) -> Self {
        self.server_port = port;
        self
    }
}

pub fn get_user_agent() -> String {
    let os = if cfg!(target_os = "windows") {
        "Windows NT 10.0; Win64; x64"
    } else {
        "X11; Linux x86_64"
    };

    format!(
        "Mozilla/5.0 ({os}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 rDM/{}",
        env!("CARGO_PKG_VERSION")
    )
}
