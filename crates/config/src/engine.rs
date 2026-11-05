use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub default_connections: usize,
    pub download_dir: PathBuf,
    pub state_dir: PathBuf,
    pub user_agent: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        let download_dir = dirs::download_dir().unwrap_or_else(|| PathBuf::from("."));
        let state_dir = dirs::data_local_dir()
            .map(|dir| dir.join("rdm"))
            .unwrap_or_else(|| PathBuf::from(".rdm"));

        Self {
            default_connections: 8,
            download_dir,
            state_dir,
            user_agent: get_user_agent(),
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
            default_connections,
            download_dir,
            state_dir,
            user_agent,
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
