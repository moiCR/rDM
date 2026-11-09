use std::{path::PathBuf, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use uuid::Uuid;

use crate::{Core, error::EngineError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterceptedDownload {
    pub id: Uuid,
    pub url: String,
    pub file_name: String,
    pub total_size: u64,
    pub content_type: Option<String>,
    pub supports_ranges: bool,
    pub referer: Option<String>,
    pub connections: usize,
    pub download_dir: PathBuf,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExtensionDownloadRequest {
    pub url: String,
    pub filename: Option<String>,
    pub referer: Option<String>,
    pub connections: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtensionDownloadResponse {
    pub status: String,
    pub id: Uuid,
    pub file_name: String,
    pub total_size: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PingResponse {
    pub status: String,
    pub version: String,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct YouTubeFormatsResponse {
    pub status: String,
    pub formats: Vec<crate::engine::YouTubeFormat>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorResponse {
    pub status: String,
    pub message: String,
}

struct ParsedRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

pub async fn run_server(core: Arc<Core>) -> Result<(), EngineError> {
    let config = core.config().config();
    if !config.server_enabled {
        return Ok(());
    }

    let bind_address = format!("127.0.0.1:{}", config.server_port);
    let listener = TcpListener::bind(&bind_address).await?;

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(_) => continue,
        };

        let core_clone = Arc::clone(&core);
        tokio::spawn(async move {
            let _ = handle_connection(stream, core_clone).await;
        });
    }
}

async fn handle_connection(mut stream: TcpStream, core: Arc<Core>) -> Result<(), EngineError> {
    let parsed = match parse_http_request(&mut stream).await {
        Ok(req) => req,
        Err(_) => {
            send_json_response(
                &mut stream,
                400,
                &ErrorResponse {
                    status: "error".to_string(),
                    message: "Bad Request".to_string(),
                },
            )
            .await?;
            return Ok(());
        }
    };

    if parsed.method == "OPTIONS" {
        send_cors_preflight(&mut stream).await?;
        return Ok(());
    }

    match (parsed.method.as_str(), parsed.path.as_str()) {
        ("GET", "/api/v1/ping") | ("GET", "/ping") => {
            let response = PingResponse {
                status: "ok".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                name: "rDM".to_string(),
            };
            send_json_response(&mut stream, 200, &response).await?;
        }
        ("GET", path) if path.starts_with("/api/v1/youtube?v=") => {
            let video_id = path.trim_start_matches("/api/v1/youtube?v=").trim();
            match core.engine().resolve_youtube_formats(video_id).await {
                Ok(formats) => {
                    let response = YouTubeFormatsResponse {
                        status: "ok".to_string(),
                        formats,
                    };
                    send_json_response(&mut stream, 200, &response).await?;
                }
                Err(err) => {
                    send_json_response(
                        &mut stream,
                        500,
                        &ErrorResponse {
                            status: "error".to_string(),
                            message: err.to_string(),
                        },
                    )
                    .await?;
                }
            }
        }
        ("POST", "/api/v1/download") | ("POST", "/download") => {
            let download_req: ExtensionDownloadRequest = match serde_json::from_slice(&parsed.body)
            {
                Ok(req) => req,
                Err(err) => {
                    send_json_response(
                        &mut stream,
                        400,
                        &ErrorResponse {
                            status: "error".to_string(),
                            message: format!("Invalid JSON: {err}"),
                        },
                    )
                    .await?;
                    return Ok(());
                }
            };

            let trimmed_url = download_req.url.trim().to_string();
            if trimmed_url.is_empty() {
                send_json_response(
                    &mut stream,
                    400,
                    &ErrorResponse {
                        status: "error".to_string(),
                        message: "URL cannot be empty".to_string(),
                    },
                )
                .await?;
                return Ok(());
            }

            let probe_result = core
                .engine()
                .probe(&trimmed_url, download_req.referer.as_deref())
                .await;

            let requested_name = download_req
                .filename
                .as_ref()
                .and_then(|name| std::path::Path::new(name.trim()).file_name())
                .and_then(|os_name| os_name.to_str())
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string());

            let (final_url, file_name, total_size, content_type, supports_ranges) =
                match probe_result {
                    Ok(probe) => {
                        let resolved_name = requested_name.unwrap_or(probe.file_name);
                        (
                            probe.url,
                            resolved_name,
                            probe.total_size,
                            probe.content_type,
                            probe.supports_ranges,
                        )
                    }
                    Err(_) => {
                        let fallback_name = requested_name
                            .unwrap_or_else(|| fallback_filename_from_url(&trimmed_url));
                        (trimmed_url, fallback_name, 0, None, true)
                    }
                };

            let cfg = core.config().config();
            let connections = if supports_ranges {
                download_req
                    .connections
                    .unwrap_or(cfg.default_connections)
                    .clamp(1, 32)
            } else {
                1
            };

            let intercept_id = Uuid::new_v4();
            let intercepted = InterceptedDownload {
                id: intercept_id,
                url: final_url,
                file_name: file_name.clone(),
                total_size,
                content_type,
                supports_ranges,
                referer: download_req.referer,
                connections,
                download_dir: cfg.download_dir,
            };

            core.notify_intercepted(intercepted);

            let response = ExtensionDownloadResponse {
                status: "ok".to_string(),
                id: intercept_id,
                file_name,
                total_size,
            };
            send_json_response(&mut stream, 200, &response).await?;
        }
        _ => {
            send_json_response(
                &mut stream,
                404,
                &ErrorResponse {
                    status: "error".to_string(),
                    message: "Not Found".to_string(),
                },
            )
            .await?;
        }
    }

    Ok(())
}

fn fallback_filename_from_url(url_str: &str) -> String {
    if let Ok(parsed) = reqwest::Url::parse(url_str)
        && let Some(mut segments) = parsed.path_segments()
        && let Some(last) = segments.rfind(|s| !s.is_empty())
    {
        let decoded = percent_encoding::percent_decode_str(last).decode_utf8_lossy();
        let sanitized: String = decoded
            .chars()
            .map(|c| match c {
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
                c if c.is_control() => '_',
                c => c,
            })
            .collect();
        let trimmed = sanitized.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "download.bin".to_string()
}

async fn parse_http_request(stream: &mut TcpStream) -> Result<ParsedRequest, EngineError> {
    let mut buffer = [0u8; 4096];
    let mut header_bytes = Vec::new();

    loop {
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Connection closed before headers received",
            )
            .into());
        }

        header_bytes.extend_from_slice(&buffer[..bytes_read]);

        if let Some(pos) = find_header_end(&header_bytes) {
            let header_str = String::from_utf8_lossy(&header_bytes[..pos]);
            let mut lines = header_str.lines();

            let request_line = lines.next().unwrap_or_default();
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or("").to_uppercase();
            let path = parts.next().unwrap_or("").to_string();

            let mut content_length = 0usize;
            for line in lines {
                if let Some((key, val)) = line.split_once(':')
                    && key.trim().eq_ignore_ascii_case("content-length")
                    && let Ok(len) = val.trim().parse::<usize>()
                {
                    content_length = len.min(1024 * 1024);
                }
            }

            let initial_body = header_bytes[pos + 4..].to_vec();
            let mut body = initial_body;

            while body.len() < content_length {
                let remaining = content_length - body.len();
                let to_read = remaining.min(buffer.len());
                let n = stream.read(&mut buffer[..to_read]).await?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&buffer[..n]);
            }

            return Ok(ParsedRequest { method, path, body });
        }

        if header_bytes.len() > 16384 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "HTTP headers exceed max allowed size",
            )
            .into());
        }
    }
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|w| w == b"\r\n\r\n")
}

async fn send_cors_preflight(stream: &mut TcpStream) -> Result<(), EngineError> {
    let headers = "HTTP/1.1 204 No Content\r\n\
Access-Control-Allow-Origin: *\r\n\
Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
Access-Control-Max-Age: 86400\r\n\
Content-Length: 0\r\n\
Connection: close\r\n\r\n";

    stream.write_all(headers.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

async fn send_json_response<T: serde::Serialize>(
    stream: &mut TcpStream,
    status_code: u16,
    data: &T,
) -> Result<(), EngineError> {
    let status_text = match status_code {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Status",
    };

    let body = serde_json::to_vec(data)?;
    let header = format!(
        "HTTP/1.1 {status_code} {status_text}\r\n\
Content-Type: application/json\r\n\
Access-Control-Allow-Origin: *\r\n\
Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\r\n",
        body.len()
    );

    stream.write_all(header.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_header_end() {
        let raw = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\nbody";
        assert_eq!(find_header_end(raw), Some(31));

        let invalid = b"GET / HTTP/1.1\r\nHost: localhost\r\n";
        assert_eq!(find_header_end(invalid), None);
    }

    #[test]
    fn test_deserialize_download_request() {
        let json = r#"{"url":"https://example.com/file.zip","filename":"custom.zip","referer":"https://example.com"}"#;
        let req: Result<ExtensionDownloadRequest, _> = serde_json::from_str(json);
        assert!(req.is_ok());
        let req = req.expect("deserialization should succeed");
        assert_eq!(req.url, "https://example.com/file.zip");
        assert_eq!(req.filename, Some("custom.zip".to_string()));
        assert_eq!(req.referer, Some("https://example.com".to_string()));
    }

    #[test]
    fn test_fallback_filename_from_url() {
        assert_eq!(
            fallback_filename_from_url("https://example.com/files/archive%20v1.zip?token=123"),
            "archive v1.zip"
        );
        assert_eq!(
            fallback_filename_from_url("https://example.com/"),
            "download.bin"
        );
    }
}
