use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Connecting,
    Downloading,
    ConversionQueued,
    Processing { staged_size: Option<u64> },
    Paused,
    Completed,
    Cancelled,
    Failed(String),
    ConversionFailed(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SegmentStatus {
    Pending,
    Downloading,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PostProcessAction {
    MuxVideoAudio {
        video_size: u64,
        audio_url: String,
        audio_size: u64,
    },
    ConvertMp3 {
        bitrate_kbps: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedSegment {
    pub index: usize,
    pub start_byte: u64,
    pub end_byte: u64,
    pub downloaded_bytes: u64,
    pub status: SegmentStatus,
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

    pub fn to_persisted(&self) -> PersistedSegment {
        PersistedSegment {
            index: self.index,
            start_byte: self.start_byte,
            end_byte: self.end_byte,
            downloaded_bytes: self.downloaded_bytes.load(Ordering::Relaxed),
            status: self.status(),
        }
    }

    pub fn from_persisted(persisted: PersistedSegment) -> Self {
        let is_incomplete = persisted.end_byte > 0
            && persisted.end_byte >= persisted.start_byte
            && persisted.downloaded_bytes < (persisted.end_byte - persisted.start_byte) + 1;

        let status = match persisted.status {
            SegmentStatus::Downloading => SegmentStatus::Pending,
            SegmentStatus::Completed if is_incomplete => SegmentStatus::Pending,
            other => other,
        };
        Self {
            index: persisted.index,
            start_byte: persisted.start_byte,
            end_byte: persisted.end_byte,
            downloaded_bytes: AtomicU64::new(persisted.downloaded_bytes),
            status: RwLock::new(status),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedTask {
    pub id: Uuid,
    pub url: String,
    pub output_path: PathBuf,
    pub total_size: u64,
    pub downloaded_bytes: u64,
    pub connections: usize,
    pub referer: Option<String>,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    pub segments: Vec<PersistedSegment>,
    #[serde(default)]
    pub post_process: Option<PostProcessAction>,
}

#[derive(Debug)]
pub struct Task {
    pub id: Uuid,
    pub url: String,
    pub output_path: PathBuf,
    pub total_size: u64,
    pub downloaded_bytes: AtomicU64,
    pub connections: usize,
    pub referer: Option<String>,
    pub post_process: Option<PostProcessAction>,
    state: RwLock<TaskState>,
    segments: RwLock<Vec<Arc<DownloadSegment>>>,
    cancellation_token: RwLock<CancellationToken>,
}

#[derive(Debug)]
pub(crate) struct TaskState {
    pub status: TaskStatus,
    pub completed_at: Option<u64>,
}

impl TaskState {
    pub(crate) fn new(status: TaskStatus) -> Self {
        let completed_at = if status == TaskStatus::Completed {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_secs())
        } else {
            None
        };
        Self {
            status,
            completed_at,
        }
    }
}

impl Task {
    pub fn new(
        id: Uuid,
        url: String,
        output_path: PathBuf,
        total_size: u64,
        connections: usize,
        referer: Option<String>,
    ) -> Self {
        Self::new_with_post_process(id, url, output_path, total_size, connections, referer, None)
    }

    pub fn new_with_post_process(
        id: Uuid,
        url: String,
        output_path: PathBuf,
        total_size: u64,
        connections: usize,
        referer: Option<String>,
        post_process: Option<PostProcessAction>,
    ) -> Self {
        Self {
            id,
            url,
            output_path,
            total_size,
            downloaded_bytes: AtomicU64::new(0),
            connections,
            referer,
            post_process,
            state: RwLock::new(TaskState::new(TaskStatus::Pending)),
            segments: RwLock::new(Vec::new()),
            cancellation_token: RwLock::new(CancellationToken::new()),
        }
    }

    pub fn status(&self) -> TaskStatus {
        self.status_and_completion().0
    }

    pub fn status_and_completion(&self) -> (TaskStatus, Option<u64>) {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (state.status.clone(), state.completed_at)
    }

    pub fn set_status(&self, new_status: TaskStatus) {
        self.set_state(TaskState::new(new_status));
    }

    pub(crate) fn set_state(&self, mut new_state: TaskState) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if new_state.status == TaskStatus::Completed && state.status == TaskStatus::Completed {
            new_state.completed_at = state.completed_at.or(new_state.completed_at);
        }
        *state = new_state;
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

    pub fn to_persisted(&self) -> PersistedTask {
        let (status, completed_at) = self.status_and_completion();
        let segments = self
            .segments()
            .into_iter()
            .map(|s| s.to_persisted())
            .collect();
        PersistedTask {
            id: self.id,
            url: self.url.clone(),
            output_path: self.output_path.clone(),
            total_size: self.total_size,
            downloaded_bytes: self.downloaded_bytes.load(Ordering::Relaxed),
            connections: self.connections,
            referer: self.referer.clone(),
            status,
            completed_at,
            segments,
            post_process: self.post_process.clone(),
        }
    }

    pub fn from_persisted(persisted: PersistedTask) -> Self {
        let is_incomplete =
            persisted.total_size > 0 && persisted.downloaded_bytes < persisted.total_size;
        let status = match persisted.status {
            TaskStatus::Downloading | TaskStatus::Connecting => TaskStatus::Paused,
            TaskStatus::Processing { .. } | TaskStatus::ConversionQueued => {
                TaskStatus::ConversionFailed("La conversión se interrumpió.".into())
            }
            TaskStatus::Completed if is_incomplete => TaskStatus::Paused,
            other => other,
        };
        let segments: Vec<Arc<DownloadSegment>> = persisted
            .segments
            .into_iter()
            .map(|s| Arc::new(DownloadSegment::from_persisted(s)))
            .collect();

        Self {
            id: persisted.id,
            url: persisted.url,
            output_path: persisted.output_path,
            total_size: persisted.total_size,
            downloaded_bytes: AtomicU64::new(persisted.downloaded_bytes),
            connections: persisted.connections,
            referer: persisted.referer,
            post_process: persisted.post_process,
            state: RwLock::new(TaskState {
                completed_at: (status == TaskStatus::Completed)
                    .then_some(persisted.completed_at)
                    .flatten(),
                status,
            }),
            segments: RwLock::new(segments),
            cancellation_token: RwLock::new(CancellationToken::new()),
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
            None,
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

    #[test]
    fn completion_date_survives_persistence_and_legacy_tasks_still_load()
    -> Result<(), serde_json::Error> {
        let task = Task::new(Uuid::new_v4(), String::new(), PathBuf::new(), 0, 1, None);
        task.set_status(TaskStatus::Completed);
        let completion = task.status_and_completion();
        assert!(completion.1.is_some());

        let snapshot = serde_json::to_value(task.to_persisted())?;
        let restored = Task::from_persisted(serde_json::from_value(snapshot.clone())?);
        assert_eq!(restored.status_and_completion(), completion);

        let mut legacy = snapshot;
        if let Some(fields) = legacy.as_object_mut() {
            fields.remove("completed_at");
        }
        let restored = Task::from_persisted(serde_json::from_value(legacy)?);
        assert_eq!(
            restored.status_and_completion(),
            (TaskStatus::Completed, None)
        );
        Ok(())
    }

    #[test]
    fn test_persisted_conversion() {
        let id = Uuid::new_v4();
        let task = Task::new(
            id,
            "https://example.com/file.zip".to_string(),
            PathBuf::from("file.zip"),
            1000,
            2,
            Some("https://example.com/".to_string()),
        );

        let s1 = Arc::new(DownloadSegment::new(0, 0, 499));
        s1.downloaded_bytes.store(200, Ordering::Relaxed);
        let s2 = Arc::new(DownloadSegment::new(1, 500, 999));
        task.set_segments(vec![s1, s2]);
        task.downloaded_bytes.store(200, Ordering::Relaxed);
        task.set_status(TaskStatus::Downloading);

        let persisted = task.to_persisted();
        assert_eq!(persisted.id, id);
        assert_eq!(persisted.downloaded_bytes, 200);
        assert_eq!(persisted.segments.len(), 2);
        assert_eq!(persisted.segments[0].downloaded_bytes, 200);

        let restored = Task::from_persisted(persisted);
        assert_eq!(restored.id, id);
        assert_eq!(restored.status(), TaskStatus::Paused);
        assert_eq!(restored.segments().len(), 2);
        assert_eq!(
            restored.segments()[0]
                .downloaded_bytes
                .load(Ordering::Relaxed),
            200
        );
    }
}
