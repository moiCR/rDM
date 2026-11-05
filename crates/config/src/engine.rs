use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub default_connections: usize,
    pub download_dir: PathBuf,
    pub user_agent: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        let download_dir = match dirs::download_dir() {
            Some(dir) => dir,
            None => PathBuf::from("."),
        };

        Self {
            default_connections: 8,
            download_dir,
            user_agent: get_user_agent(),
        }
    }
}

impl EngineConfig {
    pub fn new(default_connections: usize, download_dir: PathBuf, user_agent: String) -> Self {
        Self {
            default_connections,
            download_dir,
            user_agent,
        }
    }

    pub fn with_download_dir(mut self, download_dir: PathBuf) -> Self {
        self.download_dir = download_dir;
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
