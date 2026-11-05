use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

use rdm_core::Core;
use rdm_core::task::TaskStatus;

use crate::{
    args::CliArgs,
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
        self.reporter.info(&format!("Probing URL: {}", args.url));

        let probe = self
            .core
            .engine()
            .probe(&args.url, args.referer.as_deref())
            .await?;

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
            .create_task(&args.url, args.output, args.connections, args.referer)
            .await?;

        self.reporter
            .info(&format!("Output path: {}", task.output_path.display()));
        self.reporter
            .info(&format!("Active connections: {}", task.connections));

        self.core.engine().start_task(task.id).await?;

        let mut interval = tokio::time::interval(Duration::from_millis(200));
        let start_time = Instant::now();
        let mut last_instant = Instant::now();
        let mut last_bytes = 0u64;

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
                            self.reporter.info("Download was paused");
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    self.reporter.finish();
                    self.reporter.info("Interruption received (Ctrl+C). Cancelling task...");
                    let _ = self.core.engine().cancel_task(task.id).await;
                    return Ok(());
                }
            }
        }
    }
}
