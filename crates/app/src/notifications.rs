use std::{
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use gpui::{App, Context, Entity, WeakEntity};
use rdm_core::updater::ReleaseInfo;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_NOTIFICATIONS: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NotificationKind {
    DownloadStarted {
        task_id: Uuid,
        file_name: String,
    },
    DownloadCompleted {
        task_id: Uuid,
        file_name: String,
        output_path: PathBuf,
    },
    DownloadFailed {
        task_id: Uuid,
        file_name: String,
        url: String,
        output_path: PathBuf,
        error: String,
    },
    UpdateAvailable {
        info: ReleaseInfo,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppNotification {
    pub id: Uuid,
    pub kind: NotificationKind,
    pub created_at: i64,
    pub read: bool,
}

impl AppNotification {
    pub fn new(kind: NotificationKind) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Self {
            id: Uuid::new_v4(),
            kind,
            created_at,
            read: false,
        }
    }
}

pub struct NotificationsModel {
    items: Vec<AppNotification>,
    storage_path: PathBuf,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl NotificationsModel {
    pub fn new(
        state_dir: PathBuf,
        runtime: Arc<tokio::runtime::Runtime>,
        cx: &mut Context<Self>,
    ) -> Self {
        let storage_path = state_dir.join("notifications.json");
        let load_path = storage_path.clone();
        let load_runtime = Arc::clone(&runtime);
        cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            let loaded = load_runtime
                .spawn(async move { load_notifications(&load_path).await })
                .await
                .unwrap_or_default();
            if loaded.is_empty() {
                return;
            }
            let _ = entity.update(cx, |this, cx| {
                for item in loaded {
                    if !this.items.iter().any(|existing| existing.id == item.id) {
                        this.items.push(item);
                    }
                }
                this.items.sort_by_key(|a| std::cmp::Reverse(a.created_at));
                this.items.truncate(MAX_NOTIFICATIONS);
                cx.notify();
                cx.refresh_windows();
            });
        })
        .detach();

        Self {
            items: Vec::new(),
            storage_path,
            runtime,
        }
    }

    pub fn items(&self) -> &[AppNotification] {
        &self.items
    }

    pub fn get(&self, id: Uuid) -> Option<&AppNotification> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn unread_count(&self) -> usize {
        self.items.iter().filter(|item| !item.read).count()
    }

    pub fn push(
        &mut self,
        kind: NotificationKind,
        system_title: &str,
        system_body: &str,
        send_os_notification: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.insert_deduplicated(kind) {
            return false;
        }
        if send_os_notification {
            send_native_notification(system_title, system_body);
        }
        self.persist();
        cx.notify();
        cx.refresh_windows();
        true
    }

    fn insert_deduplicated(&mut self, kind: NotificationKind) -> bool {
        let duplicate = match &kind {
            NotificationKind::UpdateAvailable { info } => self.items.iter().any(|item| {
                matches!(
                    &item.kind,
                    NotificationKind::UpdateAvailable { info: existing }
                        if existing.tag_name == info.tag_name
                )
            }),
            NotificationKind::DownloadStarted { task_id, .. } => self.items.iter().any(|item| {
                matches!(
                    &item.kind,
                    NotificationKind::DownloadStarted { task_id: existing, .. }
                        if existing == task_id
                )
            }),
            _ => false,
        };
        if duplicate {
            return false;
        }
        self.items.insert(0, AppNotification::new(kind));
        self.items.truncate(MAX_NOTIFICATIONS);
        true
    }

    pub fn mark_read(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id)
            && !item.read
        {
            item.read = true;
            self.persist();
            cx.notify();
            cx.refresh_windows();
        }
    }

    pub fn mark_all_read(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        for item in &mut self.items {
            if !item.read {
                item.read = true;
                changed = true;
            }
        }
        if changed {
            self.persist();
            cx.notify();
            cx.refresh_windows();
        }
    }

    pub fn remove(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let before = self.items.len();
        self.items.retain(|item| item.id != id);
        if self.items.len() != before {
            self.persist();
            cx.notify();
            cx.refresh_windows();
        }
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if !self.items.is_empty() {
            self.items.clear();
            self.persist();
            cx.notify();
            cx.refresh_windows();
        }
    }

    fn persist(&self) {
        let path = self.storage_path.clone();
        let items = self.items.clone();
        self.runtime.spawn(async move {
            let _ = save_notifications(&path, &items).await;
        });
    }
}

pub fn notify_entity(
    notifications: &Entity<NotificationsModel>,
    kind: NotificationKind,
    title: String,
    body: String,
    enabled: bool,
    cx: &mut App,
) {
    notifications.update(cx, |model, cx| {
        model.push(kind, &title, &body, enabled, cx);
    });
}

fn send_native_notification(title: &str, body: &str) {
    #[cfg(windows)]
    crate::windows_desktop::notify(title, body);
    #[cfg(target_os = "linux")]
    crate::linux_desktop::notify(title, body);
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = (title, body);
}

async fn load_notifications(path: &std::path::Path) -> Vec<AppNotification> {
    let Ok(bytes) = tokio::fs::read(path).await else {
        return Vec::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

async fn save_notifications(
    path: &std::path::Path,
    items: &[AppNotification],
) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let data = serde_json::to_vec_pretty(items).map_err(std::io::Error::other)?;
    let temp = path.with_extension("json.tmp");
    tokio::fs::write(&temp, data).await?;
    tokio::fs::rename(&temp, path).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_release(tag: &str) -> ReleaseInfo {
        ReleaseInfo {
            version: tag.trim_start_matches('v').to_string(),
            tag_name: tag.to_string(),
            release_notes: "## Changes\n* Item 1".to_string(),
            html_url: "https://github.com/moiCR/rDM/releases".to_string(),
            installer_url: None,
            installer_name: None,
            installer_size: 0,
            package: rdm_core::updater::PackageKind::Manual,
        }
    }

    #[test]
    fn deduplicates_started_downloads_and_updates_by_key() {
        let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
        let mut model = NotificationsModel {
            items: Vec::new(),
            storage_path: std::env::temp_dir().join("rdm-test-notifications.json"),
            runtime,
        };
        let id = Uuid::new_v4();
        assert!(
            model.insert_deduplicated(NotificationKind::DownloadStarted {
                task_id: id,
                file_name: "a.zip".into(),
            })
        );
        assert!(
            !model.insert_deduplicated(NotificationKind::DownloadStarted {
                task_id: id,
                file_name: "a.zip".into(),
            })
        );
        assert!(
            model.insert_deduplicated(NotificationKind::UpdateAvailable {
                info: sample_release("v1.3.0"),
            })
        );
        assert!(
            !model.insert_deduplicated(NotificationKind::UpdateAvailable {
                info: sample_release("v1.3.0"),
            })
        );
        assert!(
            model.insert_deduplicated(NotificationKind::UpdateAvailable {
                info: sample_release("v1.4.0"),
            })
        );
        assert_eq!(model.items().len(), 3);
        assert_eq!(model.unread_count(), 3);
    }
}
