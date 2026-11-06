pub mod dashboard;
pub mod downloads;
pub mod new_download;
pub mod settings;

pub use dashboard::DashboardView;
pub use downloads::DownloadsView;
pub use new_download::NewDownloadView;
pub use settings::{SettingsModal, SettingsModalState};
