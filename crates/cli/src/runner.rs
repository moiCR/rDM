use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use rdm_core::{
    Core, Engine, EngineError,
    task::{Task, TaskStatus},
};

use crate::{
    args::{CliArgs, CliCommand},
    error::CliError,
    format::{format_bytes, format_duration, format_speed},
    progress::ProgressReporter,
};

pub struct CliRunner {
    engine: Arc<Engine>,
    reporter: ProgressReporter,
}

impl CliRunner {
    pub fn new() -> Result<Self, CliError> {
        let core = Core::new()?;
        let engine = Arc::clone(core.engine());
        let reporter = ProgressReporter::default();
        Ok(Self { engine, reporter })
    }

    pub async fn run(&self, args: CliArgs) -> Result<(), CliError> {
        match args.command {
            CliCommand::Download {
                url,
                output,
                connections,
                referer,
            } => {
                self.reporter.info(&format!("Probing URL: {url}"));

                let probe = self.engine.probe(&url, referer.as_deref()).await?;

                self.reporter.info(&format!("File: {}", probe.file_name));
                self.reporter
                    .info(&format!("Size: {}", format_bytes(probe.total_size)));
                self.reporter.info(&format!(
                    "Multi-connection: {}",
                    if probe.supports_ranges {
                        "Supported"
                    } else {
                        "Not supported (single connection)"
                    }
                ));

                if let Some(content_type) = &probe.content_type
                    && content_type.contains("text/html")
                {
                    self.reporter.info(
                        "Warning: Content-Type is text/html. The URL may have expired or redirected to a web page.",
                    );
                }

                let task = self
                    .engine
                    .create_task(&url, output, connections, referer)
                    .await?;

                self.reporter
                    .info(&format!("Output path: {}", task.output_path.display()));
                self.reporter
                    .info(&format!("Active connections: {}", task.connections));

                self.engine.start_task(task.id).await?;
                self.monitor_task(task).await
            }
            CliCommand::Info { url, referer } => {
                self.reporter.info(&format!("Probing URL: {url}"));
                let probe = self.engine.probe(&url, referer.as_deref()).await?;

                println!();
                println!("Remote Resource Information:");
                println!("  File Name:        {}", probe.file_name);
                println!(
                    "  Total Size:       {} ({} bytes)",
                    format_bytes(probe.total_size),
                    probe.total_size
                );
                println!(
                    "  Range Requests:   {}",
                    if probe.supports_ranges {
                        "Supported (multi-connection enabled)"
                    } else {
                        "Not supported (single connection only)"
                    }
                );
                println!(
                    "  Content-Type:     {}",
                    probe.content_type.as_deref().unwrap_or("unspecified")
                );
                if let Some(ref ct) = probe.content_type
                    && ct.contains("text/html")
                {
                    println!(
                        "  Warning:          Content-Type is text/html. The URL may have expired or redirected to a web page."
                    );
                }
                println!();
                Ok(())
            }
            CliCommand::List => {
                let tasks = self.engine.all_tasks().await;
                if tasks.is_empty() {
                    self.reporter.info("No download tasks found in history.");
                    return Ok(());
                }

                println!();
                println!(
                    "{:<36}  {:<19}  {:<8}  {:<10}  FILE",
                    "TASK ID", "STATUS", "PROGRESS", "SIZE"
                );
                println!(
                    "{:-<36}  {:-<19}  {:-<8}  {:-<10}  {:-<20}",
                    "", "", "", "", ""
                );

                for task in tasks {
                    let status_str = match task.status() {
                        TaskStatus::Pending => "Pending".to_string(),
                        TaskStatus::Connecting => "Connecting".to_string(),
                        TaskStatus::Downloading => "Downloading".to_string(),
                        TaskStatus::ConversionQueued => "Queued (conversion)".to_string(),
                        TaskStatus::Processing { .. } => "Processing".to_string(),
                        TaskStatus::Paused => "Paused".to_string(),
                        TaskStatus::Completed => "Completed".to_string(),
                        TaskStatus::Cancelled => "Cancelled".to_string(),
                        TaskStatus::Failed(_) => "Failed".to_string(),
                        TaskStatus::ConversionFailed(_) => "Conversion failed".to_string(),
                    };
                    let pct = format!("{:.1}%", task.progress_ratio() * 100.0);
                    let size_str = format_bytes(task.total_size);
                    let file_str = task
                        .output_path
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("unknown");

                    println!(
                        "{:<36}  {:<19}  {:<8}  {:<10}  {}",
                        task.id, status_str, pct, size_str, file_str
                    );
                }
                println!();
                Ok(())
            }
            CliCommand::Resume { id } => {
                let task = self.engine.get_task(id).await?;
                self.reporter.info(&format!(
                    "Resuming task {id}: {}",
                    task.output_path.display()
                ));
                self.engine.resume_task(id).await?;
                self.monitor_task(task).await
            }
            CliCommand::Delete { id, delete_file } => {
                self.engine.delete_task(id, delete_file).await?;
                self.reporter
                    .success(&format!("Task {id} removed successfully"));
                Ok(())
            }
        }
    }

    async fn monitor_task(&self, task: Arc<Task>) -> Result<(), CliError> {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        let start_time = Instant::now();
        let mut last_instant = Instant::now();
        let mut last_bytes = task.downloaded_bytes.load(Ordering::Relaxed);

        let mut processing_reported = false;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let status = task.status();
                    if matches!(status, TaskStatus::Processing { .. }) {
                        if !processing_reported {
                            self.reporter.info("Processing with FFmpeg...");
                            processing_reported = true;
                        }
                        continue;
                    }
                    let now = Instant::now();
                    let current_bytes = task.downloaded_bytes.load(Ordering::Relaxed);
                    let delta_bytes = current_bytes.saturating_sub(last_bytes);
                    let elapsed_seconds = now.duration_since(last_instant).as_secs_f64();

                    let speed = if elapsed_seconds > 0.0 {
                        delta_bytes as f64 / elapsed_seconds
                    } else {
                        0.0
                    };

                    let eta = if speed > 0.0 && task.total_size > current_bytes {
                        let remaining = task.total_size - current_bytes;
                        Some(Duration::from_secs_f64(remaining as f64 / speed))
                    } else {
                        None
                    };

                    self.reporter.render(current_bytes, task.total_size, speed, eta)?;

                    last_instant = now;
                    last_bytes = current_bytes;

                    match status {
                        TaskStatus::Completed => {
                            self.reporter.finish();
                            let total_elapsed = start_time.elapsed();
                            let avg_speed = if total_elapsed.as_secs_f64() > 0.0 {
                                task.total_size as f64 / total_elapsed.as_secs_f64()
                            } else {
                                0.0
                            };
                            self.reporter.success(&format!(
                                "Download finished in {} (avg {}) -> {}",
                                format_duration(total_elapsed),
                                format_speed(avg_speed),
                                task.output_path.display()
                            ));
                            return Ok(());
                        }
                        TaskStatus::Failed(reason) => {
                            self.reporter.finish();
                            self.reporter.error(&format!("Download failed: {reason}"));
                            return Err(CliError::InvalidArgument(reason));
                        }
                        TaskStatus::ConversionFailed(reason) => {
                            self.reporter.finish();
                            return Err(CliError::ConversionFailed(reason));
                        }
                        TaskStatus::Cancelled => {
                            self.reporter.finish();
                            self.reporter.info("Download was cancelled");
                            return Ok(());
                        }
                        TaskStatus::Paused => {
                            self.reporter.finish();
                            self.reporter.info(&format!(
                                "Download paused. Resume anytime with: rdm-cli resume {}",
                                task.id
                            ));
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    self.reporter.finish();
                    match self.engine.pause_task(task.id).await {
                        Ok(()) if task.status() == TaskStatus::Paused => {}
                        Ok(()) => continue,
                        Err(EngineError::ConversionInProgress(_)) => {
                            self.reporter.info("Waiting for FFmpeg to finish...");
                            continue;
                        }
                        Err(error) => return Err(error.into()),
                    }
                    self.reporter.info(&format!(
                        "Download paused. Resume anytime with: rdm-cli resume {}",
                        task.id
                    ));
                    return Ok(());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rdm_config::{ConfigManager, EngineConfig};
    use rdm_core::{DiskManager, task::PostProcessAction};
    use uuid::Uuid;

    #[tokio::test]
    async fn resume_reports_conversion_error_without_redownloading()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = std::env::temp_dir().join(format!("rdm_cli_resume_{}", Uuid::new_v4()));
        let config = EngineConfig::default()
            .with_state_dir(directory.clone())
            .with_download_dir(directory.clone());
        let engine = Arc::new(Engine::new(
            Arc::new(ConfigManager::from_config(config)),
            Arc::new(DiskManager::new()),
        )?);
        let task = engine
            .create_task_with_options(
                "https://example.invalid/googlevideo.com/videoplayback?clen=4",
                Some(directory.join("audio.mp3")),
                None,
                None,
                Some(PostProcessAction::ConvertMp3 { bitrate_kbps: 128 }),
            )
            .await?;
        task.set_status(TaskStatus::ConversionFailed("previous failure".into()));
        let runner = CliRunner {
            engine,
            reporter: ProgressReporter::default(),
        };
        let result = runner
            .run(CliArgs {
                command: CliCommand::Resume { id: task.id },
            })
            .await;
        assert!(
            matches!(result, Err(CliError::ConversionFailed(reason)) if reason.contains("audio.part"))
        );
        assert!(!tokio::fs::try_exists(&task.output_path).await?);
        tokio::fs::remove_dir_all(directory).await?;
        Ok(())
    }
}
