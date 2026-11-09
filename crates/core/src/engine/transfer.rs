use std::{
    path::Path,
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use futures_util::StreamExt;
use rdm_config::ConfigManager;
use tokio_util::sync::CancellationToken;

use crate::{
    disk::DiskManager,
    error::EngineError,
    task::{DownloadSegment, SegmentStatus, Task},
};

pub(super) fn stream_request(
    client: &reqwest::Client,
    url: &str,
    referer: Option<&str>,
    user_agent: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut request = client.get(url);
    if let Some(user_agent) = user_agent {
        request = request.header(reqwest::header::USER_AGENT, user_agent);
    } else if let Some(referer) = resolve_referer(url, referer)
        && let Ok(value) = reqwest::header::HeaderValue::from_str(&referer)
    {
        request = request.header(reqwest::header::REFERER, value);
    }
    request
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn download_to_path(
    mut request: reqwest::RequestBuilder,
    disk_manager: &Arc<DiskManager>,
    config: &Arc<ConfigManager>,
    task: &Arc<Task>,
    target_path: &Path,
    segment: &Arc<DownloadSegment>,
    cancellation_token: &CancellationToken,
    active_segment_count: usize,
) -> Result<(), EngineError> {
    segment.set_status(SegmentStatus::Downloading);

    let mut file = disk_manager.open_for_write(target_path).await?;
    let downloaded = segment.downloaded_bytes.load(Ordering::Relaxed);
    let mut current_offset = segment.start_byte + downloaded;

    if segment.end_byte > 0 && current_offset > segment.end_byte {
        segment.set_status(SegmentStatus::Completed);
        return Ok(());
    }

    if segment.end_byte > 0 {
        request = request.header(
            reqwest::header::RANGE,
            format!("bytes={current_offset}-{}", segment.end_byte),
        );
    } else if current_offset > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={current_offset}-"));
    }

    let response = request.send().await?;
    let status = response.status();

    if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(EngineError::HttpStatus(status));
    }

    if current_offset > 0 && status != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(EngineError::HttpStatus(status));
    }

    let mut stream = response.bytes_stream();
    let mut rate_window_start = Instant::now();
    let mut rate_window_bytes: u64 = 0;

    while let Some(chunk_result) = stream.next().await {
        if cancellation_token.is_cancelled() {
            return Ok(());
        }

        if segment.end_byte > 0 && current_offset > segment.end_byte {
            break;
        }

        let chunk = chunk_result?;
        let write_slice = if segment.end_byte > 0 {
            let remaining = (segment.end_byte + 1).saturating_sub(current_offset) as usize;
            let take_len = chunk.len().min(remaining);
            &chunk[..take_len]
        } else {
            &chunk[..]
        };

        if write_slice.is_empty() {
            break;
        }

        disk_manager
            .write_at(&mut file, current_offset, write_slice)
            .await?;

        let bytes_written = write_slice.len() as u64;
        current_offset += bytes_written;
        segment
            .downloaded_bytes
            .fetch_add(bytes_written, Ordering::Relaxed);
        task.downloaded_bytes
            .fetch_add(bytes_written, Ordering::Relaxed);

        let max_speed_global = config.config().max_speed_bytes_per_sec;
        if max_speed_global > 0 {
            rate_window_bytes += bytes_written;
            let per_worker_speed =
                (max_speed_global / active_segment_count.max(1) as u64).max(1024);
            let elapsed = rate_window_start.elapsed();
            let expected_duration = std::time::Duration::from_secs_f64(
                rate_window_bytes as f64 / per_worker_speed as f64,
            );
            if expected_duration > elapsed {
                let sleep_time = expected_duration - elapsed;
                if sleep_time > std::time::Duration::from_millis(5) {
                    tokio::select! {
                        _ = cancellation_token.cancelled() => return Ok(()),
                        _ = tokio::time::sleep(sleep_time) => {}
                    }
                    rate_window_start = Instant::now();
                    rate_window_bytes = 0;
                }
            } else if elapsed >= std::time::Duration::from_millis(500) {
                rate_window_start = Instant::now();
                rate_window_bytes = 0;
            }
        }

        if segment.end_byte > 0 && current_offset > segment.end_byte {
            break;
        }
    }

    disk_manager.flush(&mut file).await?;

    if segment.end_byte > 0 {
        let actual = segment.downloaded_bytes.load(Ordering::Relaxed);
        let expected = segment.total_bytes();
        if actual < expected {
            return Err(EngineError::IncompleteSegment { expected, actual });
        }
    }

    segment.set_status(SegmentStatus::Completed);
    Ok(())
}

pub(super) fn resolve_referer(url_str: &str, custom_referer: Option<&str>) -> Option<String> {
    if let Some(r) = custom_referer {
        return Some(r.to_string());
    }

    if let Ok(parsed_url) = reqwest::Url::parse(url_str)
        && let Some(host) = parsed_url.host_str()
    {
        let scheme = parsed_url.scheme();
        return Some(format!("{scheme}://{host}/"));
    }

    None
}
pub(super) fn decode_and_sanitize_filename(raw_name: &str) -> String {
    let replaced_plus = raw_name.replace('+', " ");
    let decoded = percent_encoding::percent_decode_str(&replaced_plus)
        .decode_utf8_lossy()
        .into_owned();

    let sanitized: String = decoded
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();

    let normalized: String = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");

    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        "download.bin".to_string()
    } else {
        trimmed.to_string()
    }
}
pub(super) fn extract_file_name_from_url(url_str: &str) -> String {
    if let Ok(parsed_url) = reqwest::Url::parse(url_str)
        && let Some(mut segments) = parsed_url.path_segments()
        && let Some(file_name) = segments.rfind(|s| !s.is_empty())
    {
        return decode_and_sanitize_filename(file_name);
    }
    "download.bin".to_string()
}
