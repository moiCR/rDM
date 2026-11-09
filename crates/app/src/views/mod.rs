pub mod dashboard;
pub mod downloads;
pub mod new_download;
pub mod settings;

pub use dashboard::DashboardView;
pub use downloads::{DeleteDownloadModal, DeleteDownloadModalState, DownloadsView};
pub use new_download::{NewDownloadModal, NewDownloadModalState, NewDownloadView};
pub use settings::{SettingsModal, SettingsModalState};
