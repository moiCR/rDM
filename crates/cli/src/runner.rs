use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use rdm_core::Core;
use rdm_core::task::{Task, TaskStatus};

use crate::{
    args::{CliArgs, CliCommand},
    error::CliError,
    format::{format_bytes, format_duration, format_speed},
    progress::ProgressReporter,
};

pub struct CliRunner {
    core: Core,
    reporter: ProgressReporter,
}

impl CliRunner {
    pub fn new() -> Result<Self, CliError> {
        let core = Core::new()?;
        let reporter = ProgressReporter::default();
        Ok(Self { core, reporter })
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

                let probe = self.core.engine().probe(&url, referer.as_deref()).await?;

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
                    .core
                    .engine()
                    .create_task(&url, output, connections, referer)
                    .await?;

                self.reporter
                    .info(&format!("Output path: {}", task.output_path.display()));
                self.reporter
                    .info(&format!("Active connections: {}", task.connections));

                self.core.engine().start_task(task.id).await?;
                self.monitor_task(task).await
            }
            CliCommand::Info { url, referer } => {
                self.reporter.info(&format!("Probing URL: {url}"));
                let probe = self.core.engine().probe(&url, referer.as_deref()).await?;

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
                let tasks = self.core.engine().all_tasks().await;
                if tasks.is_empty() {
                    self.reporter.info("No download tasks found in history.");
                    return Ok(());
                }

                println!();
                println!(
                    "{:<36}  {:<12}  {:<8}  {:<10}  FILE",
                    "TASK ID", "STATUS", "PROGRESS", "SIZE"
                );
                println!(
                    "{:-<36}  {:-<12}  {:-<8}  {:-<10}  {:-<20}",
                    "", "", "", "", ""
                );

                for task in tasks {
                    let status_str = match task.status() {
                        TaskStatus::Pending => "Pending".to_string(),
                        TaskStatus::Connecting => "Connecting".to_string(),
                        TaskStatus::Downloading => "Downloading".to_string(),
                        TaskStatus::Paused => "Paused".to_string(),
                        TaskStatus::Completed => "Completed".to_string(),
                        TaskStatus::Cancelled => "Cancelled".to_string(),
                        TaskStatus::Failed(_) => "Failed".to_string(),
                    };
                    let pct = format!("{:.1}%", task.progress_ratio() * 100.0);
                    let size_str = format_bytes(task.total_size);
                    let file_str = task
                        .output_path
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("unknown");

                    println!(
                        "{:<36}  {:<12}  {:<8}  {:<10}  {}",
                        task.id, status_str, pct, size_str, file_str
                    );
                }
                println!();
                Ok(())
            }
            CliCommand::Resume { id } => {
                let task = self.core.engine().get_task(id).await?;
                self.reporter.info(&format!(
                    "Resuming task {id}: {}",
                    task.output_path.display()
                ));
                self.core.engine().resume_task(id).await?;
                self.monitor_task(task).await
            }
            CliCommand::Delete { id, delete_file } => {
                self.core.engine().delete_task(id, delete_file).await?;
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

        loop {
            tokio::select! {
                _ = interval.tick() => {
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

                    match task.status() {
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
                    self.reporter.info("Interruption received (Ctrl+C). Pausing task...");
                    let _ = self.core.engine().pause_task(task.id).await;
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
