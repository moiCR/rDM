use std::{
    path::PathBuf,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Connecting,
    Downloading,
    Paused,
    Completed,
    Cancelled,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentStatus {
    Pending,
    Downloading,
    Completed,
    Failed,
}

#[derive(Debug)]
pub struct DownloadSegment {
    pub index: usize,
    pub start_byte: u64,
    pub end_byte: u64,
    pub downloaded_bytes: AtomicU64,
    status: RwLock<SegmentStatus>,
}

impl DownloadSegment {
    pub fn new(index: usize, start_byte: u64, end_byte: u64) -> Self {
        Self {
            index,
            start_byte,
            end_byte,
            downloaded_bytes: AtomicU64::new(0),
            status: RwLock::new(SegmentStatus::Pending),
        }
    }

    pub fn total_bytes(&self) -> u64 {
        if self.end_byte >= self.start_byte {
            (self.end_byte - self.start_byte) + 1
        } else {
            0
        }
    }

    pub fn status(&self) -> SegmentStatus {
        match self.status.read() {
            Ok(guard) => *guard,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    pub fn set_status(&self, status: SegmentStatus) {
        match self.status.write() {
            Ok(mut guard) => *guard = status,
            Err(poisoned) => *poisoned.into_inner() = status,
        }
    }
}

#[derive(Debug)]
pub struct Task {
    pub id: Uuid,
    pub url: String,
    pub output_path: PathBuf,
    pub total_size: u64,
    pub downloaded_bytes: AtomicU64,
    pub connections: usize,
    status: RwLock<TaskStatus>,
    segments: RwLock<Vec<Arc<DownloadSegment>>>,
    cancellation_token: RwLock<CancellationToken>,
}

impl Task {
    pub fn new(
        id: Uuid,
        url: String,
        output_path: PathBuf,
        total_size: u64,
        connections: usize,
    ) -> Self {
        Self {
            id,
            url,
            output_path,
            total_size,
            downloaded_bytes: AtomicU64::new(0),
            connections,
            status: RwLock::new(TaskStatus::Pending),
            segments: RwLock::new(Vec::new()),
            cancellation_token: RwLock::new(CancellationToken::new()),
        }
    }

    pub fn status(&self) -> TaskStatus {
        match self.status.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn set_status(&self, new_status: TaskStatus) {
        match self.status.write() {
            Ok(mut guard) => *guard = new_status,
            Err(poisoned) => *poisoned.into_inner() = new_status,
        }
    }

    pub fn progress_ratio(&self) -> f64 {
        if self.total_size == 0 {
            return 0.0;
        }
        let downloaded = self.downloaded_bytes.load(Ordering::Relaxed) as f64;
        (downloaded / self.total_size as f64).clamp(0.0, 1.0)
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        match self.cancellation_token.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn renew_cancellation_token(&self) -> CancellationToken {
        let new_token = CancellationToken::new();
        match self.cancellation_token.write() {
            Ok(mut guard) => *guard = new_token.clone(),
            Err(poisoned) => *poisoned.into_inner() = new_token.clone(),
        }
        new_token
    }

    pub fn pause(&self) {
        self.cancellation_token().cancel();
        self.set_status(TaskStatus::Paused);
    }

    pub fn cancel(&self) {
        self.cancellation_token().cancel();
        self.set_status(TaskStatus::Cancelled);
    }

    pub fn set_segments(&self, segments: Vec<Arc<DownloadSegment>>) {
        match self.segments.write() {
            Ok(mut guard) => *guard = segments,
            Err(poisoned) => *poisoned.into_inner() = segments,
        }
    }

    pub fn segments(&self) -> Vec<Arc<DownloadSegment>> {
        match self.segments.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_lifecycle_and_progress() {
        let task = Task::new(
            Uuid::new_v4(),
            "https://example.com/file.zip".to_string(),
            PathBuf::from("file.zip"),
            1000,
            4,
        );

        assert_eq!(task.status(), TaskStatus::Pending);
        assert_eq!(task.progress_ratio(), 0.0);

        task.downloaded_bytes.store(500, Ordering::Relaxed);
        assert!((task.progress_ratio() - 0.5).abs() < f64::EPSILON);

        task.pause();
        assert_eq!(task.status(), TaskStatus::Paused);
        assert!(task.cancellation_token().is_cancelled());

        let new_token = task.renew_cancellation_token();
        assert!(!new_token.is_cancelled());

        task.cancel();
        assert_eq!(task.status(), TaskStatus::Cancelled);
        assert!(task.cancellation_token().is_cancelled());
    }

    #[test]
    fn test_download_segment_total_bytes() {
        let segment = DownloadSegment::new(0, 0, 99);
        assert_eq!(segment.total_bytes(), 100);
        assert_eq!(segment.status(), SegmentStatus::Pending);

        segment.set_status(SegmentStatus::Downloading);
        assert_eq!(segment.status(), SegmentStatus::Downloading);
    }
}
