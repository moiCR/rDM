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

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PackageKind {
    Windows,
    Deb,
    Rpm,
    AppImage,
    #[default]
    Manual,
}

impl PackageKind {
    fn matches_asset(self, name: &str, architecture: &str) -> bool {
        if name.contains(['/', '\\']) || name.chars().any(char::is_control) {
            return false;
        }
        let name = name.to_ascii_lowercase();
        if self != Self::Windows && !name.contains("rdm") {
            return false;
        }
        match self {
            Self::Windows => {
                name.ends_with(".exe") && (name.contains("rdm") || name.contains("setup"))
            }
            Self::Deb => architecture == "x86_64" && name.ends_with("_amd64.deb"),
            Self::Rpm => architecture == "x86_64" && name.ends_with(".x86_64.rpm"),
            Self::AppImage => architecture == "x86_64" && name.ends_with("x86_64.appimage"),
            Self::Manual => false,
        }
    }
}

pub async fn installed_package() -> Result<PackageKind, EngineError> {
    #[cfg(windows)]
    {
        Ok(PackageKind::Windows)
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("APPIMAGE").is_some_and(|path| !path.is_empty()) {
            return Ok(PackageKind::AppImage);
        }
        let executable = std::env::current_exe()?;
        let Some(directory) = executable.parent() else {
            return Ok(PackageKind::Manual);
        };
        match tokio::fs::read_to_string(directory.join("package-format")).await {
            Ok(marker) => Ok(package_from_marker(&marker)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PackageKind::Manual),
            Err(error) => Err(error.into()),
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Ok(PackageKind::Manual)
    }
}

#[cfg(any(target_os = "linux", test))]
fn package_from_marker(marker: &str) -> PackageKind {
    match marker.trim() {
        "deb" => PackageKind::Deb,
        "rpm" => PackageKind::Rpm,
        _ => PackageKind::Manual,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag_name: String,
    pub release_notes: String,
    pub html_url: String,
    pub installer_url: Option<String>,
    pub installer_name: Option<String>,
    pub installer_size: u64,
    #[serde(default)]
    pub package: PackageKind,
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

    let package = installed_package().await?;
    let installer_asset = release
        .assets
        .iter()
        .find(|asset| package.matches_asset(&asset.name, std::env::consts::ARCH));

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
        package,
    }))
}

pub async fn download_installer(
    installer_url: &str,
    target_path: &Path,
    progress: Arc<UpdateProgress>,
    expected_size: u64,
) -> Result<(), EngineError> {
    progress.downloaded_bytes.store(0, Ordering::Relaxed);
    progress.total_bytes.store(expected_size, Ordering::Relaxed);
    let client = reqwest::Client::builder()
        .user_agent(format!("rDM-Updater/{}", current_version()))
        .build()?;

    let response = client.get(installer_url).send().await?;
    if !response.status().is_success() {
        return Err(EngineError::HttpStatus(response.status()));
    }

    let content_length = response.content_length();
    if let Some(total) = content_length {
        if expected_size != 0 && expected_size != total {
            return Err(EngineError::UpdateError(
                "El tamaño del paquete no coincide con la versión publicada".into(),
            ));
        }
        progress.total_bytes.store(total, Ordering::Relaxed);
    }

    if let Some(parent) = target_path.parent() {
        let mut directory = tokio::fs::DirBuilder::new();
        directory.recursive(true);
        #[cfg(unix)]
        directory.mode(0o700);
        directory.create(parent).await?;
    }

    let temporary = target_path.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
    let result = async {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await?;
        let mut stream = response.bytes_stream();
        let mut downloaded = 0_u64;
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            if expected_size != 0 && downloaded > expected_size {
                return Err(EngineError::UpdateError(
                    "El paquete excede el tamaño publicado".into(),
                ));
            }
            file.write_all(&chunk).await?;
            progress
                .downloaded_bytes
                .store(downloaded, Ordering::Relaxed);
        }
        let required = if expected_size != 0 {
            Some(expected_size)
        } else {
            content_length
        };
        if required.is_some_and(|total| total != downloaded) {
            return Err(EngineError::UpdateError(
                "La descarga del paquete está incompleta".into(),
            ));
        }
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temporary, target_path).await?;
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
    }
    result
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
    fn selects_only_matching_linux_package_and_architecture() {
        let names = [
            "rDM-Setup-v1.0.0-x64.exe",
            "rdm_1.0.0_amd64.deb",
            "rdm-1.0.0-1.x86_64.rpm",
            "rDM-1.0.0-x86_64.AppImage",
        ];
        for (package, expected) in [
            (PackageKind::Windows, names[0]),
            (PackageKind::Deb, names[1]),
            (PackageKind::Rpm, names[2]),
            (PackageKind::AppImage, names[3]),
        ] {
            let matching: Vec<_> = names
                .iter()
                .filter(|name| package.matches_asset(name, "x86_64"))
                .copied()
                .collect();
            assert_eq!(matching, vec![expected]);
            assert!(!package.matches_asset("../rDM-1.0.0-x86_64.AppImage", "x86_64"));
        }
        assert!(!PackageKind::Deb.matches_asset(names[1], "aarch64"));
        assert!(!PackageKind::Rpm.matches_asset(names[2], "aarch64"));
        assert!(!PackageKind::AppImage.matches_asset(names[3], "aarch64"));
        assert!(
            !names
                .iter()
                .any(|name| PackageKind::Manual.matches_asset(name, "x86_64"))
        );
        assert_eq!(package_from_marker("deb\n"), PackageKind::Deb);
        assert_eq!(package_from_marker("rpm\n"), PackageKind::Rpm);
        assert_eq!(package_from_marker("unknown"), PackageKind::Manual);
    }

    async fn serve_package(response: &'static [u8]) -> Result<String, EngineError> {
        use tokio::{io::AsyncReadExt, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await?;
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).await?;
            stream.write_all(response).await?;
            stream.shutdown().await
        });
        Ok(format!("http://{address}/package"))
    }

    #[tokio::test]
    async fn publishes_complete_update_and_removes_partial_downloads() -> Result<(), EngineError> {
        let directory =
            std::env::temp_dir().join(format!("rdm-updater-test-{}", uuid::Uuid::new_v4()));
        let target = directory.join("rdm.deb");
        let progress = Arc::new(UpdateProgress::default());
        let url =
            serve_package(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndata")
                .await?;
        download_installer(&url, &target, Arc::clone(&progress), 4).await?;
        assert_eq!(tokio::fs::read(&target).await?, b"data");
        assert_eq!(progress.downloaded_bytes.load(Ordering::Relaxed), 4);
        tokio::fs::remove_file(&target).await?;

        for (response, expected) in [
            (
                &b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nshort"[..],
                10,
            ),
            (
                &b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndata"[..],
                10,
            ),
            (
                &b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\ntoo-long"[..],
                4,
            ),
            (
                &b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"[..],
                4,
            ),
        ] {
            let url = serve_package(response).await?;
            assert!(
                download_installer(&url, &target, Arc::new(UpdateProgress::default()), expected)
                    .await
                    .is_err()
            );
            assert!(!tokio::fs::try_exists(&target).await?);
            assert!(
                tokio::fs::read_dir(&directory)
                    .await?
                    .next_entry()
                    .await?
                    .is_none()
            );
        }
        tokio::fs::remove_dir(directory).await?;
        Ok(())
    }

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
