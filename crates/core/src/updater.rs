#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::path::PathBuf;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf as StdPathBuf},
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
};

use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use tokio::process::Command;

use crate::error::EngineError;

const RELEASE_URL: &str = "https://github.com/moiCR/rDM/releases/latest/download/latest.json";
const RELEASE_DOWNLOAD_PREFIX: &str = "https://github.com/moiCR/rDM/releases/download/";
const RELEASE_PAGE_PREFIX: &str = "https://github.com/moiCR/rDM/releases/tag/v";
const DEFAULT_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDM3MUE0OTA4N0ZEQTYzNzMKUldSelk5cC9DRWthTjY4Q09JSEhXa2pOYU1CUDAxVVhiYjRqYW4wTGlkL2Z1ck00Y25GWEJHR2UK";
const PUBLIC_KEY_OVERRIDE: Option<&str> = option_env!("RDM_UPDATER_PUBLIC_KEY");
const MAX_UPDATE_BYTES: u64 = 1_073_741_824;

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
    fn manifest_keys(self) -> &'static [&'static str] {
        match self {
            Self::Windows => &["windows-x86_64-inno", "windows-x86_64"],
            Self::AppImage => &["linux-x86_64-appimage", "linux-x86_64"],
            Self::Deb => &["linux-x86_64-deb"],
            Self::Rpm => &["linux-x86_64-rpm"],
            Self::Manual => &[],
        }
    }

    fn resolve_platform(
        self,
        platforms: &std::collections::HashMap<String, PlatformRelease>,
    ) -> Option<&PlatformRelease> {
        self.manifest_keys()
            .iter()
            .find_map(|key| platforms.get(*key))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UpdatePhase {
    #[default]
    Downloading = 0,
    Verifying = 1,
    Installing = 2,
}

#[derive(Default)]
pub struct UpdateProgress {
    pub downloaded_bytes: AtomicU64,
    pub total_bytes: AtomicU64,
    phase: AtomicU8,
}

impl UpdateProgress {
    pub fn downloaded(&self) -> u64 {
        self.downloaded_bytes.load(Ordering::Relaxed)
    }

    pub fn total(&self) -> u64 {
        self.total_bytes.load(Ordering::Relaxed)
    }

    pub fn phase(&self) -> UpdatePhase {
        match self.phase.load(Ordering::Relaxed) {
            1 => UpdatePhase::Verifying,
            2 => UpdatePhase::Installing,
            _ => UpdatePhase::Downloading,
        }
    }

    pub fn set_phase(&self, phase: UpdatePhase) {
        self.phase.store(phase as u8, Ordering::Relaxed);
    }

    pub fn reset(&self, total: u64) {
        self.downloaded_bytes.store(0, Ordering::Relaxed);
        self.total_bytes.store(total, Ordering::Relaxed);
        self.set_phase(UpdatePhase::Downloading);
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
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub appimage_path: Option<StdPathBuf>,
}

impl ReleaseInfo {
    pub fn can_auto_install(&self) -> bool {
        self.package != PackageKind::Manual
            && self.installer_url.is_some()
            && self.installer_name.as_deref().is_some_and(is_safe_filename)
            && self
                .signature
                .as_ref()
                .is_some_and(|signature| !signature.trim().is_empty())
    }
}

#[derive(Deserialize)]
struct ReleaseManifest {
    version: String,
    notes: Option<String>,
    platforms: std::collections::HashMap<String, PlatformRelease>,
}

#[derive(Deserialize)]
struct PlatformRelease {
    url: String,
    signature: String,
    #[serde(default)]
    size: Option<u64>,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn is_newer_version(current: &str, remote: &str) -> bool {
    let parse = |value: &str| {
        let clean = value.trim().trim_start_matches(['v', 'V']);
        semver::Version::parse(clean).ok()
    };

    match (parse(current), parse(remote)) {
        (Some(c), Some(r)) => r > c,
        _ => false,
    }
}

fn select_public_key(raw: Option<&str>) -> Result<&str, EngineError> {
    raw.map(str::trim)
        .filter(|key| !key.is_empty())
        .ok_or_else(|| EngineError::UpdateError("updater.missing_public_key".into()))
}

fn configured_public_key() -> Result<&'static str, EngineError> {
    select_public_key(Some(
        PUBLIC_KEY_OVERRIDE
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .unwrap_or(DEFAULT_PUBLIC_KEY),
    ))
}

#[cfg(any(target_os = "linux", test))]
fn package_from_marker(marker: &str) -> PackageKind {
    match marker.trim() {
        "deb" => PackageKind::Deb,
        "rpm" => PackageKind::Rpm,
        _ => PackageKind::Manual,
    }
}

pub async fn installed_package() -> Result<PackageKind, EngineError> {
    Ok(detect_target().await?.0)
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
async fn detect_target() -> Result<(PackageKind, Option<StdPathBuf>), EngineError> {
    Ok((PackageKind::Windows, None))
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
async fn detect_target() -> Result<(PackageKind, Option<StdPathBuf>), EngineError> {
    if let Some(appimage) = std::env::var_os("APPIMAGE").filter(|path| !path.is_empty()) {
        let path = PathBuf::from(appimage);
        let has_extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("AppImage"));
        let is_file = tokio::fs::metadata(&path)
            .await
            .is_ok_and(|metadata| metadata.is_file());
        if has_extension || is_file {
            return Ok((PackageKind::AppImage, Some(path)));
        }
    }

    let executable = std::env::current_exe()?;
    if let Some(directory) = executable.parent() {
        match tokio::fs::read_to_string(directory.join("package-format")).await {
            Ok(marker) => {
                let package = package_from_marker(&marker);
                if package != PackageKind::Manual {
                    return Ok((package, None));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    if package_manager_owns("dpkg-query", &["-S"], &executable).await? {
        return Ok((PackageKind::Deb, None));
    }
    if package_manager_owns("rpm", &["-qf"], &executable).await? {
        return Ok((PackageKind::Rpm, None));
    }
    Ok((PackageKind::Manual, None))
}

#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86_64")
)))]
async fn detect_target() -> Result<(PackageKind, Option<StdPathBuf>), EngineError> {
    Ok((PackageKind::Manual, None))
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
async fn package_manager_owns(
    manager: &str,
    arguments: &[&str],
    executable: &Path,
) -> Result<bool, EngineError> {
    match Command::new(manager)
        .args(arguments)
        .arg(executable)
        .output()
        .await
    {
        Ok(output) => Ok(output.status.success()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn is_safe_filename(filename: &str) -> bool {
    !filename.is_empty()
        && !matches!(filename, "." | "..")
        && !filename.contains(['/', '\\', '?', '#'])
        && !filename.chars().any(char::is_control)
}

fn validate_release_url(url: &str, version: &str) -> Result<String, EngineError> {
    let prefix = format!("{RELEASE_DOWNLOAD_PREFIX}v{version}/");
    let Some(filename) = url.strip_prefix(&prefix) else {
        return Err(EngineError::UpdateError(
            "Update package URL does not match the release version".into(),
        ));
    };
    if !is_safe_filename(filename) {
        return Err(EngineError::UpdateError(
            "Update package URL contains an invalid file name".into(),
        ));
    }
    Ok(filename.to_owned())
}

fn release_info_from_manifest(
    manifest: ReleaseManifest,
    package: PackageKind,
    appimage_path: Option<StdPathBuf>,
) -> Result<Option<ReleaseInfo>, EngineError> {
    let clean_remote = manifest.version.trim().trim_start_matches(['v', 'V']);
    semver::Version::parse(clean_remote).map_err(|_| {
        EngineError::UpdateError("Update manifest contains an invalid version".into())
    })?;
    if !is_newer_version(current_version(), clean_remote) {
        return Ok(None);
    }

    let html_url = format!("{RELEASE_PAGE_PREFIX}{clean_remote}");
    let (installer_url, installer_name, installer_size, signature, appimage_path) =
        if let Some(platform) = package.resolve_platform(&manifest.platforms) {
            let filename = validate_release_url(&platform.url, clean_remote)?;
            if platform.signature.trim().is_empty() {
                return Err(EngineError::UpdateError(
                    "Update manifest has no package signature".into(),
                ));
            }
            if platform
                .size
                .is_some_and(|size| size == 0 || size > MAX_UPDATE_BYTES)
            {
                return Err(EngineError::UpdateError(
                    "Update manifest contains an invalid package size".into(),
                ));
            }
            (
                Some(platform.url.clone()),
                Some(filename),
                platform.size.unwrap_or(0),
                Some(platform.signature.clone()),
                appimage_path,
            )
        } else {
            (None, None, 0, None, None)
        };

    Ok(Some(ReleaseInfo {
        version: clean_remote.to_owned(),
        tag_name: format!("v{clean_remote}"),
        release_notes: manifest.notes.unwrap_or_default(),
        html_url,
        installer_url,
        installer_name,
        installer_size,
        package,
        signature,
        appimage_path,
    }))
}

pub async fn check_for_update() -> Result<Option<ReleaseInfo>, EngineError> {
    let _ = configured_public_key()?;

    let client = reqwest::Client::builder()
        .user_agent(format!("rDM-Updater/{}", current_version()))
        .build()?;

    let response = client.get(RELEASE_URL).send().await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(EngineError::HttpStatus(response.status()));
    }

    let body = response.bytes().await?;
    let manifest = serde_json::from_slice::<ReleaseManifest>(&body)?;
    let (package, appimage_path) = detect_target().await?;
    release_info_from_manifest(manifest, package, appimage_path)
}

async fn create_private_dir(directory: &Path) -> Result<(), EngineError> {
    let mut builder = tokio::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(directory).await?;
    Ok(())
}

async fn cleanup_stale_files(directory: &Path) {
    let Ok(mut entries) = tokio::fs::read_dir(directory).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if tokio::fs::metadata(&path)
            .await
            .is_ok_and(|metadata| metadata.is_file())
        {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }
}

async fn download_stream_to_part(
    client: &reqwest::Client,
    installer_url: &str,
    temporary: &Path,
    expected_size: Option<u64>,
    progress: &UpdateProgress,
) -> Result<(), EngineError> {
    let response = client.get(installer_url).send().await?;
    if !response.status().is_success() {
        return Err(EngineError::HttpStatus(response.status()));
    }

    let content_length = response.content_length();
    if let Some(total) = content_length {
        if total > MAX_UPDATE_BYTES {
            return Err(EngineError::UpdateError(
                "Update package exceeds the maximum supported size".into(),
            ));
        }
        if let Some(expected) = expected_size
            && expected != total
        {
            return Err(EngineError::UpdateError(
                "Update package size does not match the manifest".into(),
            ));
        }
        progress.total_bytes.store(total, Ordering::Relaxed);
    }

    let max_allowed = expected_size
        .unwrap_or(MAX_UPDATE_BYTES)
        .min(MAX_UPDATE_BYTES);
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
        .await?;
    let mut stream = response.bytes_stream();
    let mut downloaded = 0_u64;
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > max_allowed {
            return Err(EngineError::UpdateError(
                "Update package exceeds the expected size".into(),
            ));
        }
        file.write_all(&chunk).await?;
        progress
            .downloaded_bytes
            .store(downloaded, Ordering::Relaxed);
    }

    let required = expected_size.or(content_length);
    if required.is_some_and(|total| total != downloaded) {
        return Err(EngineError::UpdateError(
            "Update download is incomplete".into(),
        ));
    }

    file.sync_all().await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn download_and_verify_to_path(
    client: &reqwest::Client,
    installer_url: &str,
    package_path: &Path,
    public_key_text: &str,
    signature_text: &str,
    version: &str,
    expected_filename: &str,
    expected_size: Option<u64>,
    progress: Arc<UpdateProgress>,
) -> Result<(), EngineError> {
    progress.reset(expected_size.unwrap_or(0));

    if let Some(parent) = package_path.parent() {
        create_private_dir(parent).await?;
    }

    let temporary = package_path.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
    let stage_result = async {
        download_stream_to_part(client, installer_url, &temporary, expected_size, &progress)
            .await?;

        progress.set_phase(UpdatePhase::Verifying);
        let verify_path = temporary.clone();
        let key = public_key_text.to_owned();
        let signature = signature_text.to_owned();
        let version = version.to_owned();
        let filename = expected_filename.to_owned();
        tokio::task::spawn_blocking(move || {
            verify_signature(&verify_path, &key, &signature, &version, &filename)
        })
        .await??;

        tokio::fs::rename(&temporary, package_path).await?;
        Ok(())
    }
    .await;

    if stage_result.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
    }
    stage_result
}

pub async fn download_and_install(
    info: &ReleaseInfo,
    progress: Arc<UpdateProgress>,
) -> Result<(), EngineError> {
    let public_key = configured_public_key()?;
    if info.package == PackageKind::Manual {
        return Err(EngineError::UpdateError(
            "Automatic updates are unavailable for this installation".into(),
        ));
    }
    let Some(installer_url) = info.installer_url.as_deref() else {
        return Err(EngineError::NoUpdateInstaller);
    };
    let Some(installer_name) = info
        .installer_name
        .as_deref()
        .filter(|name| is_safe_filename(name))
    else {
        return Err(EngineError::UpdateError("updater.invalid_package".into()));
    };
    let Some(signature) = info
        .signature
        .as_deref()
        .filter(|signature| !signature.trim().is_empty())
    else {
        return Err(EngineError::UpdateError(
            "Update manifest has no package signature".into(),
        ));
    };
    let clean_version = info.version.trim().trim_start_matches(['v', 'V']);
    validate_release_url(installer_url, clean_version)?;
    decode_public_key(public_key)?;
    decode_signature(signature)?;

    if info.installer_size > MAX_UPDATE_BYTES {
        return Err(EngineError::UpdateError(
            "Update package exceeds the maximum supported size".into(),
        ));
    }
    let expected_size = (info.installer_size > 0).then_some(info.installer_size);

    let client = reqwest::Client::builder()
        .user_agent(format!("rDM-Updater/{}", current_version()))
        .build()?;

    let directory = std::env::temp_dir().join("rDM").join("updates");
    create_private_dir(&directory).await?;
    cleanup_stale_files(&directory).await;

    let package_path = directory.join(installer_name);
    download_and_verify_to_path(
        &client,
        installer_url,
        &package_path,
        public_key,
        signature,
        clean_version,
        installer_name,
        expected_size,
        progress.clone(),
    )
    .await?;

    progress.set_phase(UpdatePhase::Installing);
    let install_result = install_package(info, &package_path).await;
    #[cfg(not(windows))]
    let _ = tokio::fs::remove_file(&package_path).await;
    #[cfg(windows)]
    if install_result.is_err() {
        let _ = tokio::fs::remove_file(&package_path).await;
    }
    install_result
}

async fn install_package(info: &ReleaseInfo, package_path: &Path) -> Result<(), EngineError> {
    match info.package {
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        PackageKind::Windows => {
            std::process::Command::new(package_path)
                .args([
                    "/VERYSILENT",
                    "/SUPPRESSMSGBOXES",
                    "/NORESTART",
                    "/CLOSEAPPLICATIONS",
                ])
                .spawn()
                .map_err(|error| EngineError::UpdateError(error.to_string()))?;
            Ok(())
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        PackageKind::AppImage => {
            let appimage = info
                .appimage_path
                .clone()
                .or_else(|| std::env::var_os("APPIMAGE").map(PathBuf::from))
                .ok_or_else(|| {
                    EngineError::UpdateError("Could not locate the current AppImage file".into())
                })?;
            replace_appimage(package_path, &appimage).await
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        PackageKind::Deb => {
            run_elevated_installer("apt-get", &["install", "--yes"], package_path).await?;
            restart_application().await
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        PackageKind::Rpm => {
            let (manager, arguments): (&str, &[&str]) = if executable_available("dnf").await {
                ("dnf", &["install", "--assumeyes", "--nogpgcheck"])
            } else if executable_available("yum").await {
                ("yum", &["install", "--assumeyes", "--nogpgcheck"])
            } else if executable_available("zypper").await {
                (
                    "zypper",
                    &["--no-gpg-checks", "--non-interactive", "install"],
                )
            } else {
                return Err(EngineError::UpdateError(
                    "No supported RPM package manager is installed".into(),
                ));
            };
            run_elevated_installer(manager, arguments, package_path).await?;
            restart_application().await
        }
        _ => {
            let _ = package_path;
            Err(EngineError::UpdateError("Unsupported platform".into()))
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
async fn replace_appimage(download: &Path, appimage: &Path) -> Result<(), EngineError> {
    use std::os::unix::fs::PermissionsExt;

    let destination = tokio::fs::canonicalize(appimage)
        .await
        .map_err(|error| EngineError::UpdateError(format!("AppImage error: {error}")))?;
    let parent = destination
        .parent()
        .ok_or_else(|| EngineError::UpdateError("AppImage path has no parent directory".into()))?;
    let update_id = uuid::Uuid::new_v4();
    let staged = parent.join(format!(".rDM-{update_id}.AppImage"));
    let backup = parent.join(format!(".rDM-{update_id}.backup"));

    tokio::fs::copy(download, &staged).await?;
    if let Err(error) =
        tokio::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).await
    {
        let _ = tokio::fs::remove_file(&staged).await;
        return Err(error.into());
    }

    if let Err(error) = tokio::fs::rename(&destination, &backup).await {
        let _ = tokio::fs::remove_file(&staged).await;
        return Err(error.into());
    }
    if let Err(error) = tokio::fs::rename(&staged, &destination).await {
        let restore_result = tokio::fs::rename(&backup, &destination).await;
        let _ = tokio::fs::remove_file(&staged).await;
        if let Err(restore_error) = restore_result {
            return Err(EngineError::UpdateError(format!(
                "Could not restore previous AppImage after replacement failed: {error} ({restore_error})"
            )));
        }
        return Err(error.into());
    }

    if let Err(error) = Command::new(&destination).spawn() {
        let _ = tokio::fs::remove_file(&destination).await;
        let restore_result = tokio::fs::rename(&backup, &destination).await;
        if let Err(restore_error) = restore_result {
            return Err(EngineError::UpdateError(format!(
                "Could not restore previous AppImage after restart failed: {error} ({restore_error})"
            )));
        }
        return Err(error.into());
    }
    let _ = tokio::fs::remove_file(&backup).await;
    Ok(())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
async fn executable_available(executable: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    for directory in std::env::split_paths(&path) {
        if tokio::fs::metadata(directory.join(executable))
            .await
            .is_ok_and(|metadata| metadata.is_file())
        {
            return true;
        }
    }
    false
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
async fn run_elevated_installer(
    manager: &str,
    arguments: &[&str],
    package_path: &Path,
) -> Result<(), EngineError> {
    if !executable_available("pkexec").await {
        return Err(EngineError::UpdateError(
            "System authentication helper pkexec is not installed".into(),
        ));
    }
    if !executable_available(manager).await {
        return Err(EngineError::UpdateError(format!(
            "Package manager {manager} is not installed"
        )));
    }
    let status = Command::new("pkexec")
        .arg(manager)
        .args(arguments)
        .arg(package_path)
        .status()
        .await?;
    if !status.success() {
        return Err(EngineError::UpdateError(
            "System package installation was canceled or failed".into(),
        ));
    }
    Ok(())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
async fn restart_application() -> Result<(), EngineError> {
    let executable = std::env::current_exe()?;
    Command::new(executable).spawn()?;
    Ok(())
}

fn decode_public_key(key_text: &str) -> Result<PublicKey, EngineError> {
    let trimmed = key_text.trim();
    if trimmed.is_empty() {
        return Err(EngineError::UpdateError(
            "updater.missing_public_key".into(),
        ));
    }
    if trimmed.starts_with("untrusted comment:") {
        return PublicKey::decode(trimmed).map_err(|error| {
            EngineError::UpdateError(format!("Invalid updater public key: {error}"))
        });
    }
    if let Ok(public_key) = PublicKey::from_base64(trimmed) {
        return Ok(public_key);
    }
    let decoded_bytes = STANDARD.decode(trimmed).map_err(|error| {
        EngineError::UpdateError(format!("Invalid updater public key: {error}"))
    })?;
    let decoded_text = String::from_utf8(decoded_bytes).map_err(|error| {
        EngineError::UpdateError(format!("Updater public key is not UTF-8: {error}"))
    })?;
    let decoded_trimmed = decoded_text.trim();
    if decoded_trimmed.starts_with("untrusted comment:") {
        PublicKey::decode(decoded_trimmed)
    } else {
        PublicKey::from_base64(decoded_trimmed)
    }
    .map_err(|error| EngineError::UpdateError(format!("Invalid updater public key: {error}")))
}

fn decode_signature(signature_text: &str) -> Result<Signature, EngineError> {
    let trimmed = signature_text.trim();
    if trimmed.starts_with("untrusted comment:") {
        return Signature::decode(trimmed).map_err(|error| {
            EngineError::UpdateError(format!("Invalid package signature: {error}"))
        });
    }
    let decoded_bytes = STANDARD
        .decode(trimmed)
        .map_err(|error| EngineError::UpdateError(format!("Invalid package signature: {error}")))?;
    let decoded_text = String::from_utf8(decoded_bytes).map_err(|error| {
        EngineError::UpdateError(format!("Package signature is not UTF-8: {error}"))
    })?;
    Signature::decode(decoded_text.trim())
        .map_err(|error| EngineError::UpdateError(format!("Invalid package signature: {error}")))
}

fn validate_trusted_comment(
    trusted_comment: &str,
    version: &str,
    expected_filename: &str,
) -> Result<(), EngineError> {
    let tokens: Vec<&str> = trusted_comment.split_whitespace().collect();
    let expected_version = format!("version:{version}");
    let expected_app_version = format!("app-version:{version}");
    if !tokens
        .iter()
        .any(|field| *field == expected_version || *field == expected_app_version)
    {
        return Err(EngineError::UpdateError(
            "Update signature does not match the manifest version".into(),
        ));
    }
    let expected_file = format!("file:{expected_filename}");
    if tokens
        .iter()
        .filter(|field| field.starts_with("file:"))
        .any(|field| *field != expected_file)
    {
        return Err(EngineError::UpdateError(
            "Update signature does not match the expected package file".into(),
        ));
    }
    Ok(())
}

fn verify_signature_stream(
    package_path: &Path,
    public_key: &PublicKey,
    signature: &Signature,
) -> Result<(), EngineError> {
    let mut verifier = public_key
        .verify_stream(signature)
        .map_err(|error| EngineError::UpdateError(format!("Update signature rejected: {error}")))?;
    let mut file = File::open(package_path)?;
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        verifier.update(&buffer[..read]);
    }
    verifier.finalize().map_err(|error| {
        EngineError::UpdateError(format!("Update signature verification failed: {error}"))
    })?;
    Ok(())
}

fn verify_signature(
    package_path: &Path,
    public_key_text: &str,
    signature_text: &str,
    version: &str,
    expected_filename: &str,
) -> Result<(), EngineError> {
    let public_key = decode_public_key(public_key_text)?;
    let signature = decode_signature(signature_text)?;
    validate_trusted_comment(signature.trusted_comment(), version, expected_filename)?;
    verify_signature_stream(package_path, &public_key, &signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PUBLIC_KEY_B64: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const TEST_SIGNATURE_RAW: &str = "untrusted comment: signature from minisign secret key\n\
        RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\n\
        trusted comment: timestamp:1556193335\tfile:test\n\
        y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==";

    #[test]
    fn resolves_platform_keys_and_manifest_fallbacks() -> Result<(), EngineError> {
        let manifest_json = r#"{
            "version": "99.0.0",
            "notes": "Release notes",
            "platforms": {
                "windows-x86_64-inno": {
                    "url": "https://github.com/moiCR/rDM/releases/download/v99.0.0/rDM-Setup-v99.0.0-x64.exe",
                    "signature": "sig-win",
                    "size": 1024
                },
                "linux-x86_64-appimage": {
                    "url": "https://github.com/moiCR/rDM/releases/download/v99.0.0/rDM-99.0.0-x86_64.AppImage",
                    "signature": "sig-appimage",
                    "size": 2048
                },
                "linux-x86_64-deb": {
                    "url": "https://github.com/moiCR/rDM/releases/download/v99.0.0/rdm_99.0.0_amd64.deb",
                    "signature": "sig-deb",
                    "size": 3072
                },
                "linux-x86_64-rpm": {
                    "url": "https://github.com/moiCR/rDM/releases/download/v99.0.0/rdm-99.0.0-1.x86_64.rpm",
                    "signature": "sig-rpm",
                    "size": 4096
                }
            }
        }"#;
        let manifest: ReleaseManifest = serde_json::from_str(manifest_json)?;
        let win = release_info_from_manifest(manifest, PackageKind::Windows, None)?
            .ok_or_else(|| EngineError::UpdateError("expected release info".into()))?;
        assert!(win.can_auto_install());
        assert_eq!(
            win.installer_name.as_deref(),
            Some("rDM-Setup-v99.0.0-x64.exe")
        );
        assert_eq!(win.installer_size, 1024);

        let manifest: ReleaseManifest = serde_json::from_str(manifest_json)?;
        let appimage = release_info_from_manifest(
            manifest,
            PackageKind::AppImage,
            Some(StdPathBuf::from("/tmp/rDM.AppImage")),
        )?
        .ok_or_else(|| EngineError::UpdateError("expected release info".into()))?;
        assert!(appimage.can_auto_install());
        assert_eq!(
            appimage.installer_name.as_deref(),
            Some("rDM-99.0.0-x86_64.AppImage")
        );

        let manifest: ReleaseManifest = serde_json::from_str(manifest_json)?;
        let manual = release_info_from_manifest(manifest, PackageKind::Manual, None)?
            .ok_or_else(|| EngineError::UpdateError("expected release info".into()))?;
        assert!(!manual.can_auto_install());
        assert!(manual.installer_url.is_none());

        assert_eq!(package_from_marker("deb\n"), PackageKind::Deb);
        assert_eq!(package_from_marker("rpm\n"), PackageKind::Rpm);
        assert_eq!(package_from_marker("unknown"), PackageKind::Manual);
        Ok(())
    }

    #[test]
    fn validates_public_key_presence_decoding_and_trusted_comments() -> Result<(), EngineError> {
        assert!(select_public_key(None).is_err());
        assert!(select_public_key(Some("   ")).is_err());
        assert_eq!(select_public_key(Some("  RWabc  "))?, "RWabc");
        let embedded = decode_public_key(DEFAULT_PUBLIC_KEY)?;
        assert!(
            embedded
                .untrusted_comment()
                .is_some_and(|comment| comment.contains("371A49087FDA6373"))
        );

        let raw_two_line = format!(
            "untrusted comment: minisign public key: E7620F1842B4E81F\n{TEST_PUBLIC_KEY_B64}"
        );
        let from_raw_lines = decode_public_key(&raw_two_line)?;
        let from_raw_b64 = decode_public_key(TEST_PUBLIC_KEY_B64)?;
        let from_wrapped = decode_public_key(&STANDARD.encode(raw_two_line.as_bytes()))?;
        assert_eq!(from_raw_lines, from_wrapped);
        assert!(from_raw_b64.untrusted_comment().is_none());

        let sig_raw = decode_signature(TEST_SIGNATURE_RAW)?;
        let sig_b64 = decode_signature(&STANDARD.encode(TEST_SIGNATURE_RAW.as_bytes()))?;
        assert_eq!(sig_raw.trusted_comment(), sig_b64.trusted_comment());

        let dir = std::env::temp_dir().join(format!("rdm-updater-sig-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir)?;
        let file = dir.join("payload.bin");
        std::fs::write(&file, b"test")?;
        verify_signature_stream(&file, &from_raw_b64, &sig_b64)?;
        std::fs::write(&file, b"tampered")?;
        assert!(verify_signature_stream(&file, &from_raw_b64, &sig_b64).is_err());
        std::fs::remove_dir_all(&dir)?;

        assert!(
            validate_trusted_comment(
                "timestamp:1790570130\tfile:rDM-Setup-v1.4.0-x64.exe\tversion:1.4.0",
                "1.4.0",
                "rDM-Setup-v1.4.0-x64.exe",
            )
            .is_ok()
        );
        assert!(
            validate_trusted_comment(
                "timestamp:1790570130\tfile:rDM-Setup-v1.4.0-x64.exe\tapp-version:1.4.0",
                "1.4.0",
                "rDM-Setup-v1.4.0-x64.exe",
            )
            .is_ok()
        );
        assert!(
            validate_trusted_comment(
                "timestamp:1790570130\tfile:Other.exe\tversion:1.4.0",
                "1.4.0",
                "rDM-Setup-v1.4.0-x64.exe",
            )
            .is_err()
        );
        assert!(
            validate_trusted_comment(
                "timestamp:1790570130\tfile:rDM-Setup-v1.4.0-x64.exe\tversion:1.3.0",
                "1.4.0",
                "rDM-Setup-v1.4.0-x64.exe",
            )
            .is_err()
        );
        Ok(())
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
        create_private_dir(&directory).await?;
        let part_path = directory.join("rdm.deb.part");
        let target = directory.join("rdm.deb");
        let client = reqwest::Client::new();
        let progress = Arc::new(UpdateProgress::default());

        let url =
            serve_package(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ntest")
                .await?;
        download_stream_to_part(&client, &url, &part_path, Some(4), &progress).await?;
        assert_eq!(tokio::fs::read(&part_path).await?, b"test");
        assert_eq!(progress.downloaded(), 4);
        assert_eq!(progress.total(), 4);
        tokio::fs::remove_file(&part_path).await?;

        for (response, expected) in [
            (
                &b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nshort"[..],
                Some(10),
            ),
            (
                &b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ntest"[..],
                Some(10),
            ),
            (
                &b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\ntoo-long"[..],
                Some(4),
            ),
            (
                &b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"[..],
                Some(4),
            ),
        ] {
            let url = serve_package(response).await?;
            assert!(
                download_and_verify_to_path(
                    &client,
                    &url,
                    &target,
                    TEST_PUBLIC_KEY_B64,
                    TEST_SIGNATURE_RAW,
                    "1.4.0",
                    "test",
                    expected,
                    Arc::new(UpdateProgress::default()),
                )
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
