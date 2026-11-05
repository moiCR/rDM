use gpui::*;
use gpui_router::{Route, Routes};

use crate::{
    layout::AppLayout,
    views::{DashboardView, DownloadsView, NewDownloadView},
};

pub struct RootRouter;

impl RootRouter {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for RootRouter {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        app_routes()
    }
}

pub fn app_routes() -> Routes {
    Routes::new().child(
        Route::new()
            .layout(AppLayout::new())
            .child(
                Route::new()
                    .index()
                    .element(|_window, _cx| DashboardView::new()),
            )
            .child(
                Route::new()
                    .path("downloads")
                    .element(|_window, _cx| DownloadsView::new()),
            )
            .child(
                Route::new()
                    .path("new")
                    .element(|_window, _cx| NewDownloadView::new()),
            ),
    )
}
