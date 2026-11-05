use std::{
    io::{Write, stdout},
    time::Duration,
};

use crate::format::{format_bytes, format_duration, format_speed};

pub struct ProgressReporter {
    bar_width: usize,
}

impl Default for ProgressReporter {
    fn default() -> Self {
        Self::new(30)
    }
}

impl ProgressReporter {
    pub fn new(bar_width: usize) -> Self {
        Self { bar_width }
    }

    pub fn info(&self, message: &str) {
        println!("\r[INFO] {message}");
    }

    pub fn success(&self, message: &str) {
        println!("\r[OK]   {message}");
    }

    pub fn error(&self, message: &str) {
        eprintln!("\r[ERR]  {message}");
    }

    pub fn render(
        &self,
        downloaded: u64,
        total: u64,
        speed: f64,
        eta: Option<Duration>,
    ) -> std::io::Result<()> {
        let mut out = stdout().lock();

        if total > 0 {
            let ratio = (downloaded as f64 / total as f64).clamp(0.0, 1.0);
            let percent = ratio * 100.0;
            let filled = (ratio * self.bar_width as f64).round() as usize;
            let empty = self.bar_width.saturating_sub(filled);

            let bar_filled = "=".repeat(filled);
            let bar_empty = " ".repeat(empty);

            let eta_text = match eta {
                Some(duration) => format!("ETA: {}", format_duration(duration)),
                None => "ETA: --:--".to_string(),
            };

            write!(
                out,
                "\r[{bar_filled}>{bar_empty}] {:>5.1}% ({:>9} / {:>9}) @ {:>10} | {:>10}  ",
                percent,
                format_bytes(downloaded),
                format_bytes(total),
                format_speed(speed),
                eta_text
            )?;
        } else {
            write!(
                out,
                "\r[Streaming] {:>9} downloaded @ {:>10}  ",
                format_bytes(downloaded),
                format_speed(speed)
            )?;
        }

        out.flush()?;
        Ok(())
    }

    pub fn finish(&self) {
        println!();
    }
}
