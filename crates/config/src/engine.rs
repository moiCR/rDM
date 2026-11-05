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
        Self {
            default_connections: 8,
            download_dir: PathBuf::from("."),
            user_agent: get_user_agent(),
        }
    }
}

pub fn get_user_agent() -> String {
    let os = if cfg!(target_os = "windows") {
        "Windows NT 10.0; Win64; x64"
    } else {
        "X11; Linux x86_64"
    };

    format!(
        "Mozilla/5.0 ({}) AppleWebKit/537.36 (KHTML, like Gecko) rDM/{}",
        os,
        env!("CARGO_PKG_VERSION")
    )
}
