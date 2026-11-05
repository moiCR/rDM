use gpui_router::{Route, Routes};

use crate::views::{DownloadsView, NewDownloadView, SettingsView};

pub fn app_routes() -> Routes {
    Routes::new()
        .child(
            Route::new()
                .path("/")
                .element(|_window, _cx| DownloadsView::new()),
        )
        .child(
            Route::new()
                .path("/new")
                .element(|_window, _cx| NewDownloadView::new()),
        )
        .child(
            Route::new()
                .path("/settings")
                .element(|_window, _cx| SettingsView::new()),
        )
}
