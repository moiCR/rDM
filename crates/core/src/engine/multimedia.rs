use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::Ordering},
};

use rdm_config::ConfigManager;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    disk::DiskManager,
    error::EngineError,
    task::{DownloadSegment, PersistedTask, PostProcessAction, Task, TaskStatus},
};

use super::{
    EngineModule, ResourceProbe,
    transfer::{
        decode_and_sanitize_filename, download_to_path, extract_file_name_from_url, stream_request,
    },
};

const YOUTUBE_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct YouTubeFormat {
    pub category: String,
    pub url: String,
    pub filename: String,
    pub quality: String,
    pub ext: String,
    pub size_bytes: u64,
    pub video_size: u64,
    pub audio_url: Option<String>,
    pub audio_size: u64,
    pub mp3_bitrate: Option<u32>,
}

pub(super) struct MultimediaModule {
    client: reqwest::Client,
    config: Arc<ConfigManager>,
    disk_manager: Arc<DiskManager>,
}

impl EngineModule for MultimediaModule {
    async fn probe(
        &self,
        url: &str,
        _referer: Option<&str>,
    ) -> Result<Option<ResourceProbe>, EngineError> {
        if let Some(video_id) = extract_youtube_video_id(url) {
            let formats = self.resolve_youtube_formats(&video_id).await?;
            if let Some(first) = formats.into_iter().next() {
                let post_process = match (first.audio_url, first.mp3_bitrate) {
                    (Some(audio_url), _) if first.audio_size > 0 => {
                        Some(PostProcessAction::MuxVideoAudio {
                            video_size: first.video_size,
                            audio_url,
                            audio_size: first.audio_size,
                        })
                    }
                    (_, Some(bitrate_kbps)) => Some(PostProcessAction::ConvertMp3 { bitrate_kbps }),
                    _ => None,
                };
                return Ok(Some(ResourceProbe {
                    url: first.url,
                    file_name: first.filename,
                    total_size: first.size_bytes,
                    supports_ranges: true,
                    content_type: Some("video/mp4".to_string()),
                    post_process,
                }));
            }
            return Err(EngineError::HttpStatus(reqwest::StatusCode::NOT_FOUND));
        }

        if url.contains("googlevideo.com/videoplayback")
            && let Some(clen) = extract_query_u64(url, "clen")
            && clen > 0
        {
            return Ok(Some(ResourceProbe {
                url: url.to_string(),
                file_name: extract_file_name_from_url(url),
                total_size: clen,
                supports_ranges: true,
                content_type: Some("video/mp4".to_string()),
                post_process: None,
            }));
        }

        Ok(None)
    }
}

impl MultimediaModule {
    pub(super) fn new(
        client: reqwest::Client,
        config: Arc<ConfigManager>,
        disk_manager: Arc<DiskManager>,
    ) -> Self {
        Self {
            client,
            config,
            disk_manager,
        }
    }

    pub(super) async fn resolve_youtube_formats(
        &self,
        video_id: &str,
    ) -> Result<Vec<YouTubeFormat>, EngineError> {
        let visitor_data = self.fetch_youtube_visitor_data(video_id).await;

        let mut client_obj = serde_json::json!({
            "clientName": "VISIONOS",
            "clientVersion": "1.02",
            "deviceMake": "Apple",
            "deviceModel": "RealityDevice17,1",
            "osName": "visionOS",
            "osVersion": "26.5.23O471",
            "userAgent": YOUTUBE_USER_AGENT
        });
        if let Some(ref vd) = visitor_data
            && let Some(map) = client_obj.as_object_mut()
        {
            map.insert(
                "visitorData".to_string(),
                serde_json::Value::String(vd.clone()),
            );
        }

        let payload = serde_json::json!({
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true,
            "context": {
                "client": client_obj
            }
        });

        let mut req = self
            .client
            .post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false")
            .header(reqwest::header::USER_AGENT, YOUTUBE_USER_AGENT)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ORIGIN, "https://www.youtube.com")
            .header("X-YouTube-Client-Name", "101")
            .header("X-YouTube-Client-Version", "1.02");

        if let Some(ref vd) = visitor_data {
            req = req.header("X-Goog-Visitor-Id", vd.as_str());
        }

        let response = req.body(payload.to_string()).send().await?;

        if !response.status().is_success() {
            return Err(EngineError::HttpStatus(response.status()));
        }

        let body_bytes = response.bytes().await?;
        let json: serde_json::Value = serde_json::from_slice(&body_bytes)?;
        let raw_title = json
            .get("videoDetails")
            .and_then(|v| v.get("title"))
            .and_then(|t| t.as_str())
            .unwrap_or("youtube_video");
        let clean_title = decode_and_sanitize_filename(raw_title);

        let mut results = Vec::new();
        let adaptive = json
            .get("streamingData")
            .and_then(|s| s.get("adaptiveFormats"))
            .and_then(|f| f.as_array());

        let mut best_m4a: Option<(String, u64, bool)> = None;
        let mut best_audio_for_mp3: Option<(String, u64, bool)> = None;

        if let Some(adaptive_list) = adaptive {
            for fmt in adaptive_list {
                let itag = fmt.get("itag").and_then(|i| i.as_u64()).unwrap_or(0);
                let Some(url) = fmt.get("url").and_then(|u| u.as_str()) else {
                    continue;
                };
                let size_bytes = fmt
                    .get("contentLength")
                    .and_then(|c| c.as_str())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                let is_original = !url.contains("dubbed-auto")
                    && fmt
                        .get("audioTrack")
                        .and_then(|a| a.get("audioIsDefault"))
                        .and_then(|d| d.as_bool())
                        .unwrap_or(true);

                if itag == 140
                    && best_m4a
                        .as_ref()
                        .is_none_or(|(_, existing_sz, existing_orig)| {
                            (is_original && !*existing_orig)
                                || (is_original == *existing_orig && size_bytes > *existing_sz)
                        })
                {
                    best_m4a = Some((url.to_string(), size_bytes, is_original));
                }

                if (itag == 140 || itag == 251)
                    && best_audio_for_mp3
                        .as_ref()
                        .is_none_or(|(_, existing_sz, existing_orig)| {
                            (is_original && !*existing_orig)
                                || (is_original == *existing_orig && size_bytes > *existing_sz)
                        })
                {
                    best_audio_for_mp3 = Some((url.to_string(), size_bytes, is_original));
                }
            }

            if best_m4a.is_none() {
                for fmt in adaptive_list {
                    let itag = fmt.get("itag").and_then(|i| i.as_u64()).unwrap_or(0);
                    let Some(url) = fmt.get("url").and_then(|u| u.as_str()) else {
                        continue;
                    };
                    let size_bytes = fmt
                        .get("contentLength")
                        .and_then(|c| c.as_str())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    if itag == 139 {
                        best_m4a = Some((url.to_string(), size_bytes, true));
                        break;
                    }
                }
            }
        }
        let best_m4a = best_m4a.map(|(u, s, _)| (u, s));
        let best_audio_for_mp3 = best_audio_for_mp3.map(|(u, s, _)| (u, s));

        let target_resolutions = [
            ("2160p", "2160p 4K"),
            ("1440p", "1440p 2K"),
            ("1080p", "1080p HD"),
            ("720p", "720p HD"),
            ("480p", "480p"),
            ("360p", "360p"),
            ("240p", "240p"),
            ("144p", "144p"),
        ];

        if let Some(adaptive_list) = adaptive
            && let Some((ref m4a_url, m4a_size)) = best_m4a
        {
            for (res_key, res_label) in target_resolutions {
                let mut selected_fmt: Option<(&str, u64, bool)> = None;

                for fmt in adaptive_list {
                    let mime = fmt.get("mimeType").and_then(|m| m.as_str()).unwrap_or("");
                    if !mime.starts_with("video/mp4") {
                        continue;
                    }
                    let q_label = fmt
                        .get("qualityLabel")
                        .and_then(|q| q.as_str())
                        .unwrap_or("");
                    if !q_label.starts_with(res_key) {
                        continue;
                    }
                    let Some(url) = fmt.get("url").and_then(|u| u.as_str()) else {
                        continue;
                    };
                    let video_size = fmt
                        .get("contentLength")
                        .and_then(|c| c.as_str())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    if video_size == 0 {
                        continue;
                    }
                    let is_avc1 = mime.contains("avc1");

                    match selected_fmt {
                        None => selected_fmt = Some((url, video_size, is_avc1)),
                        Some((_, existing_sz, existing_avc1)) => {
                            if (is_avc1 && !existing_avc1)
                                || (is_avc1 == existing_avc1 && video_size > existing_sz)
                            {
                                selected_fmt = Some((url, video_size, is_avc1));
                            }
                        }
                    }
                }

                if let Some((video_url, video_size, _)) = selected_fmt {
                    results.push(YouTubeFormat {
                        category: "video".to_string(),
                        url: video_url.to_string(),
                        filename: format!("{clean_title} ({res_key}).mp4"),
                        quality: res_label.to_string(),
                        ext: "MP4".to_string(),
                        size_bytes: video_size + m4a_size,
                        video_size,
                        audio_url: Some(m4a_url.clone()),
                        audio_size: m4a_size,
                        mp3_bitrate: None,
                    });
                }
            }
        }

        if results.is_empty()
            && let Some(formats) = json
                .get("streamingData")
                .and_then(|s| s.get("formats"))
                .and_then(|f| f.as_array())
        {
            for fmt in formats {
                let Some(url) = fmt.get("url").and_then(|u| u.as_str()) else {
                    continue;
                };
                let quality = fmt
                    .get("qualityLabel")
                    .and_then(|q| q.as_str())
                    .unwrap_or("360p");
                let size_bytes = fmt
                    .get("contentLength")
                    .and_then(|c| c.as_str())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);

                results.push(YouTubeFormat {
                    category: "video".to_string(),
                    url: url.to_string(),
                    filename: format!("{clean_title} ({quality}).mp4"),
                    quality: quality.to_string(),
                    ext: "MP4".to_string(),
                    size_bytes,
                    video_size: size_bytes,
                    audio_url: None,
                    audio_size: 0,
                    mp3_bitrate: None,
                });
            }
        }

        let length_secs = json
            .get("videoDetails")
            .and_then(|v| v.get("lengthSeconds"))
            .and_then(|l| l.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        if let Some((audio_url, audio_size)) = best_audio_for_mp3.or(best_m4a) {
            for (bitrate, label) in [
                (320u32, "320 kbps (Alta)"),
                (192u32, "192 kbps (Media)"),
                (128u32, "128 kbps (Estándar)"),
            ] {
                let estimated_mp3_size = if length_secs > 0 {
                    (length_secs * u64::from(bitrate) * 1000) / 8
                } else {
                    audio_size
                };
                results.push(YouTubeFormat {
                    category: "audio".to_string(),
                    url: audio_url.clone(),
                    filename: format!("{clean_title} ({bitrate}kbps).mp3"),
                    quality: label.to_string(),
                    ext: "MP3".to_string(),
                    size_bytes: estimated_mp3_size,
                    video_size: audio_size,
                    audio_url: None,
                    audio_size: 0,
                    mp3_bitrate: Some(bitrate),
                });
            }
        }

        Ok(results)
    }

    async fn fetch_youtube_visitor_data(&self, video_id: &str) -> Option<String> {
        let watch_url = format!("https://www.youtube.com/watch?v={video_id}");
        if let Ok(resp) = self
            .client
            .get(&watch_url)
            .header(
                reqwest::header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await
            && let Ok(text) = resp.text().await
        {
            let marker = "\"VISITOR_DATA\":\"";
            if let Some(start) = text.find(marker) {
                let rest = &text[start + marker.len()..];
                if let Some(end) = rest.find('"') {
                    let vd = rest[..end].trim();
                    if !vd.is_empty() {
                        return Some(vd.to_string());
                    }
                }
            }
        }
        None
    }

    pub(super) async fn infer_post_process(
        &self,
        url: &str,
        filename: Option<&str>,
        referer: Option<&str>,
    ) -> Option<PostProcessAction> {
        if !url.contains("googlevideo.com/videoplayback") {
            return None;
        }
        if filename.is_some_and(|name| name.to_lowercase().ends_with(".mp3")) {
            return Some(PostProcessAction::ConvertMp3 { bitrate_kbps: 192 });
        }
        if !url.contains("mime=video") {
            return None;
        }

        let video_id = extract_youtube_video_id(referer?)?;
        let format = self
            .resolve_youtube_formats(&video_id)
            .await
            .ok()?
            .into_iter()
            .find(|format| format.audio_url.is_some())?;
        Some(PostProcessAction::MuxVideoAudio {
            video_size: 0,
            audio_url: format.audio_url?,
            audio_size: format.audio_size,
        })
    }

    pub(super) fn primary_target(&self, task: &Task) -> (PathBuf, u64) {
        match &task.post_process {
            Some(PostProcessAction::MuxVideoAudio { video_size, .. }) => {
                (temp_part_path(&task.output_path, "video.part"), *video_size)
            }
            Some(PostProcessAction::ConvertMp3 { .. }) => (
                temp_part_path(&task.output_path, "audio.part"),
                task.total_size,
            ),
            None => (task.output_path.clone(), task.total_size),
        }
    }

    pub(super) fn spawn_companion_stream(
        &self,
        task: &Arc<Task>,
        cancellation_token: &CancellationToken,
    ) -> Option<JoinHandle<Result<(), EngineError>>> {
        let Some(PostProcessAction::MuxVideoAudio {
            audio_url,
            audio_size,
            ..
        }) = &task.post_process
        else {
            return None;
        };

        let audio_target_path = temp_part_path(&task.output_path, "audio.part");
        let client = self.client.clone();
        let disk_manager = Arc::clone(&self.disk_manager);
        let config = Arc::clone(&self.config);
        let task = Arc::clone(task);
        let audio_url = audio_url.clone();
        let audio_size = *audio_size;
        let token = cancellation_token.clone();

        Some(tokio::spawn(async move {
            download_companion_stream(
                client,
                disk_manager,
                config,
                task,
                audio_url,
                audio_target_path,
                audio_size,
                token,
            )
            .await
        }))
    }

    pub(super) async fn prepare_conversion(&self, task: &Task) -> Result<u64, String> {
        self.discard_staging(task).await;
        let state_dir = self.config.config().state_dir;
        let staged_path = staging_path(&task.output_path, task.id);
        for (path, expected_size) in track_paths(
            &task.output_path,
            task.total_size,
            task.post_process.as_ref(),
        ) {
            let metadata = tokio::fs::metadata(&path).await.map_err(|error| {
                format!("No se puede leer la pista {}: {error}", path.display())
            })?;
            if !metadata.is_file()
                || metadata.len() == 0
                || (expected_size > 0 && metadata.len() != expected_size)
            {
                return Err(format!("La pista {} está incompleta.", path.display()));
            }
        }

        match &task.post_process {
            Some(PostProcessAction::MuxVideoAudio { .. }) => {
                let video = temp_part_path(&task.output_path, "video.part");
                let audio = temp_part_path(&task.output_path, "audio.part");
                run_ffmpeg_mux(&state_dir, &video, &audio, &staged_path).await?;
            }
            Some(PostProcessAction::ConvertMp3 { bitrate_kbps }) => {
                let audio = temp_part_path(&task.output_path, "audio.part");
                run_ffmpeg_mp3(&state_dir, &audio, &staged_path, *bitrate_kbps).await?;
            }
            None => return Err("La descarga no tiene una conversión pendiente.".into()),
        }

        let output = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&staged_path)
            .await
            .map_err(|error| format!("No se puede abrir la salida de FFmpeg: {error}"))?;
        let metadata = output.metadata().await.map_err(|error| error.to_string())?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err("FFmpeg produjo un archivo vacío.".into());
        }
        output.sync_all().await.map_err(|error| error.to_string())?;
        Ok(metadata.len())
    }

    pub(super) async fn publish_conversion(&self, task: &Task) -> Result<(), String> {
        match tokio::fs::symlink_metadata(&task.output_path).await {
            Ok(_) => {
                return Err(format!(
                    "El destino {} ya existe.",
                    task.output_path.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("No se puede comprobar el destino: {error}")),
        }
        tokio::fs::rename(staging_path(&task.output_path, task.id), &task.output_path)
            .await
            .map_err(|error| format!("No se puede publicar la conversión: {error}"))
    }

    pub(super) async fn discard_staging(&self, task: &Task) {
        let _ = tokio::fs::remove_file(staging_path(&task.output_path, task.id)).await;
    }

    pub(super) async fn remove_parts(&self, task: &Task) {
        for (path, _) in track_paths(
            &task.output_path,
            task.total_size,
            task.post_process.as_ref(),
        ) {
            let _ = tokio::fs::remove_file(path).await;
        }
    }

    pub(super) fn recover_conversion(persisted: &mut PersistedTask) {
        let TaskStatus::Processing { staged_size } = persisted.status else {
            return;
        };
        let staged_path = staging_path(&persisted.output_path, persisted.id);
        let was_published = persisted.post_process.is_some()
            && staged_size.is_some_and(|size| {
                size > 0
                    && std::fs::symlink_metadata(&staged_path)
                        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
                    && std::fs::symlink_metadata(&persisted.output_path)
                        .is_ok_and(|metadata| metadata.is_file() && metadata.len() == size)
            });
        if was_published {
            for (path, _) in track_paths(
                &persisted.output_path,
                persisted.total_size,
                persisted.post_process.as_ref(),
            ) {
                let _ = std::fs::remove_file(path);
            }
            persisted.status = TaskStatus::Completed;
        }
    }
}

pub(super) fn track_paths(
    path: &Path,
    total_size: u64,
    action: Option<&PostProcessAction>,
) -> impl Iterator<Item = (PathBuf, u64)> {
    let (video, audio) = match action {
        Some(PostProcessAction::MuxVideoAudio {
            video_size,
            audio_size,
            ..
        }) => (Some(*video_size), Some(*audio_size)),
        Some(PostProcessAction::ConvertMp3 { .. }) => (None, Some(total_size)),
        None => (None, None),
    };
    video
        .map(|size| (temp_part_path(path, "video.part"), size))
        .into_iter()
        .chain(audio.map(|size| (temp_part_path(path, "audio.part"), size)))
}

fn staging_path(output_path: &Path, task_id: Uuid) -> PathBuf {
    output_path.with_file_name(format!(".rdm-{task_id}.conversion.part"))
}

pub(super) fn stream_user_agent(url: &str) -> Option<&'static str> {
    url.contains("googlevideo.com")
        .then_some(YOUTUBE_USER_AGENT)
}

pub(crate) fn normalize_post_process(
    probed_size: u64,
    post_process: Option<PostProcessAction>,
) -> (u64, Option<PostProcessAction>) {
    match post_process {
        Some(PostProcessAction::MuxVideoAudio {
            video_size,
            audio_url,
            audio_size,
        }) => {
            let video_size = if video_size > 0 {
                video_size
            } else {
                probed_size
            };
            (
                video_size + audio_size,
                Some(PostProcessAction::MuxVideoAudio {
                    video_size,
                    audio_url,
                    audio_size,
                }),
            )
        }
        other => (probed_size, other),
    }
}

fn temp_part_path(output_path: &Path, suffix: &str) -> PathBuf {
    let mut os = output_path.as_os_str().to_os_string();
    os.push(format!(".{suffix}"));
    PathBuf::from(os)
}

fn resolve_ffmpeg_binary(state_dir: &Path) -> PathBuf {
    let bin_name = if cfg!(target_os = "windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let local_bin = state_dir.join("bin").join(bin_name);
    if local_bin.exists() {
        return local_bin;
    }
    PathBuf::from(bin_name)
}

async fn run_ffmpeg_mux(
    state_dir: &Path,
    video_path: &Path,
    audio_path: &Path,
    output_path: &Path,
) -> Result<(), String> {
    let ffmpeg = resolve_ffmpeg_binary(state_dir);
    let mut cmd = tokio::process::Command::new(ffmpeg);
    cmd.kill_on_drop(true)
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
        .arg("-i")
        .arg(video_path)
        .arg("-i")
        .arg(audio_path)
        .arg("-c")
        .arg("copy")
        .arg("-movflags")
        .arg("+faststart")
        .args(["-f", "mp4"])
        .arg(output_path);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("FFmpeg error: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg mux failed: {stderr}"));
    }
    Ok(())
}

async fn run_ffmpeg_mp3(
    state_dir: &Path,
    audio_path: &Path,
    output_path: &Path,
    bitrate_kbps: u32,
) -> Result<(), String> {
    let ffmpeg = resolve_ffmpeg_binary(state_dir);
    let mut cmd = tokio::process::Command::new(ffmpeg);
    cmd.kill_on_drop(true)
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
        .arg("-i")
        .arg(audio_path)
        .arg("-vn")
        .arg("-c:a")
        .arg("libmp3lame")
        .arg("-b:a")
        .arg(format!("{bitrate_kbps}k"))
        .args(["-f", "mp3"])
        .arg(output_path);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("FFmpeg error: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg MP3 conversion failed: {stderr}"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn download_companion_stream(
    client: reqwest::Client,
    disk_manager: Arc<DiskManager>,
    config: Arc<ConfigManager>,
    task: Arc<Task>,
    url: String,
    target_path: PathBuf,
    total_size: u64,
    cancellation_token: CancellationToken,
) -> Result<(), EngineError> {
    if total_size > 0 {
        disk_manager.allocate_file(&target_path, total_size).await?;
    } else {
        let mut f = disk_manager.open_for_write(&target_path).await?;
        disk_manager.flush(&mut f).await?;
    }

    let end_byte = if total_size > 0 { total_size - 1 } else { 0 };
    let seg = Arc::new(DownloadSegment::new(999, 0, end_byte));

    let max_retries = config.config().max_retries.max(1);
    let mut attempt = 0;

    loop {
        if cancellation_token.is_cancelled() {
            return Ok(());
        }

        let bytes_before = seg.downloaded_bytes.load(Ordering::Relaxed);
        let request = stream_request(
            &client,
            &url,
            task.referer.as_deref(),
            stream_user_agent(&url),
        );
        let result = download_to_path(
            request,
            &disk_manager,
            &config,
            &task,
            &target_path,
            &seg,
            &cancellation_token,
            1,
        )
        .await;

        match result {
            Ok(()) => return Ok(()),
            Err(err) => {
                if cancellation_token.is_cancelled() {
                    return Ok(());
                }
                let bytes_after = seg.downloaded_bytes.load(Ordering::Relaxed);
                if bytes_after > bytes_before {
                    attempt = 0;
                    if matches!(err, EngineError::IncompleteSegment { .. }) {
                        continue;
                    }
                } else {
                    attempt += 1;
                    if attempt > max_retries {
                        return Err(err);
                    }
                }
                let backoff_ms = (200 * (1 << attempt.min(5))).min(5000);
                tokio::select! {
                    _ = cancellation_token.cancelled() => return Ok(()),
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)) => {}
                }
            }
        }
    }
}

fn extract_query_u64(url_str: &str, key: &str) -> Option<u64> {
    let parsed = reqwest::Url::parse(url_str).ok()?;
    for (k, v) in parsed.query_pairs() {
        if k == key {
            return v.trim().parse::<u64>().ok();
        }
    }
    None
}

pub fn extract_youtube_video_id(url_str: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url_str).ok()?;
    let host = parsed.host_str()?.to_lowercase();

    if host == "youtu.be" {
        let mut segs = parsed.path_segments()?;
        let id = segs.next()?.trim();
        if !id.is_empty() {
            return Some(id.to_string());
        }
        return None;
    }

    if host == "youtube.com" || host == "www.youtube.com" || host == "m.youtube.com" {
        let path = parsed.path();
        if path == "/watch" {
            for (k, v) in parsed.query_pairs() {
                if k == "v" && !v.trim().is_empty() {
                    return Some(v.trim().to_string());
                }
            }
        } else if let Some(rest) = path
            .strip_prefix("/shorts/")
            .or_else(|| path.strip_prefix("/embed/"))
            && let Some(id) = rest.split('/').next()
            && !id.trim().is_empty()
        {
            return Some(id.trim().to_string());
        }
    }

    None
}
