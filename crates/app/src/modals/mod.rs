pub mod delete_download;
pub mod download_inspector;
pub mod new_download;
pub mod settings;

pub use delete_download::{DeleteDownloadModal, DeleteDownloadModalState};
pub use download_inspector::{DownloadInspectorModal, DownloadInspectorModalState};
pub use new_download::{NewDownloadModal, NewDownloadModalState};
pub use settings::{SettingsModal, SettingsModalState, SettingsTab};
