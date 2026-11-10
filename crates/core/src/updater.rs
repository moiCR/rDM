use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::error::EngineError;

const REPO_OWNER: &str = "moiCR";
const REPO_NAME: &str = "rDM";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag_name: String,
    pub release_notes: String,
    pub html_url: String,
    pub installer_url: Option<String>,
    pub installer_name: Option<String>,
    pub installer_size: u64,
}

#[derive(Default)]
pub struct UpdateProgress {
    pub downloaded_bytes: AtomicU64,
    pub total_bytes: AtomicU64,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn is_newer_version(current: &str, remote: &str) -> bool {
    fn parse_semver(v: &str) -> Option<(u32, u32, u32)> {
        let clean = v.trim().trim_start_matches(['v', 'V']);
        let core = clean.split(['-', '+']).next()?;
        let mut parts = core.split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
        let patch = parts.next().unwrap_or("0").parse::<u32>().ok()?;
        Some((major, minor, patch))
    }

    match (parse_semver(current), parse_semver(remote)) {
        (Some(c), Some(r)) => r > c,
        _ => false,
    }
}

pub async fn check_for_update() -> Result<Option<ReleaseInfo>, EngineError> {
    let client = reqwest::Client::builder()
        .user_agent(format!("rDM-Updater/{}", current_version()))
        .build()?;

    let url = format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/latest");
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(EngineError::HttpStatus(response.status()));
    }

    let body = response.bytes().await?;
    let release = serde_json::from_slice::<GitHubRelease>(&body)?;
    let remote_ver = release
        .tag_name
        .trim()
        .trim_start_matches(['v', 'V'])
        .to_string();

    if !is_newer_version(current_version(), &remote_ver) {
        return Ok(None);
    }

    let installer_asset = release.assets.iter().find(|a| {
        let lower = a.name.to_lowercase();
        lower.ends_with(".exe") && (lower.contains("setup") || lower.contains("rdm"))
    });

    let (installer_url, installer_name, installer_size) = match installer_asset {
        Some(asset) => (
            Some(asset.browser_download_url.clone()),
            Some(asset.name.clone()),
            asset.size,
        ),
        None => (None, None, 0),
    };

    Ok(Some(ReleaseInfo {
        version: remote_ver,
        tag_name: release.tag_name,
        release_notes: release.body.unwrap_or_default(),
        html_url: release.html_url,
        installer_url,
        installer_name,
        installer_size,
    }))
}

pub async fn download_installer(
    installer_url: &str,
    target_path: &Path,
    progress: Arc<UpdateProgress>,
) -> Result<(), EngineError> {
    let client = reqwest::Client::builder()
        .user_agent(format!("rDM-Updater/{}", current_version()))
        .build()?;

    let response = client.get(installer_url).send().await?;
    if !response.status().is_success() {
        return Err(EngineError::HttpStatus(response.status()));
    }

    if let Some(total) = response.content_length() {
        progress.total_bytes.store(total, Ordering::Relaxed);
    }

    if let Some(parent) = target_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut file = tokio::fs::File::create(target_path).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        file.write_all(&chunk).await?;
        progress
            .downloaded_bytes
            .fetch_add(chunk.len() as u64, Ordering::Relaxed);
    }

    file.flush().await?;
    Ok(())
}

pub fn launch_installer(installer_path: &Path) -> Result<(), EngineError> {
    #[cfg(windows)]
    {
        std::process::Command::new(installer_path)
            .args([
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/CLOSEAPPLICATIONS",
            ])
            .spawn()
            .map_err(|e| EngineError::UpdateError(e.to_string()))?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = installer_path;
        Err(EngineError::UpdateError("Unsupported platform".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.1.0", "0.2.0"));
        assert!(is_newer_version("0.1.0", "v0.1.1"));
        assert!(is_newer_version("0.1.0", "1.0.0"));
        assert!(is_newer_version("0.1.9", "0.1.10"));
        assert!(!is_newer_version("0.2.0", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("invalid", "0.1.0"));
    }
}
