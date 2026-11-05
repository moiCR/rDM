use gpui::*;

use crate::{layout::sidebar_item::sidebar_item, theme::ActiveTheme};

#[derive(IntoElement)]
pub struct Sidebar;

impl Sidebar {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let location = gpui_router::use_location(cx);
        let path = location.pathname.as_str();

        div()
            .flex()
            .flex_col()
            .items_start()
            .justify_between()
            .w(px(200.0))
            .h_full()
            .bg(cx.theme().background())
            .px_4()
            .pb_5()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_start()
                    .justify_center()
                    .gap_2()
                    .child(sidebar_item(
                        "Dashboard",
                        "icons/dashboard.svg",
                        Some("/"),
                        path == "/",
                    ))
                    .child(sidebar_item(
                        "Downloads",
                        "icons/download.svg",
                        Some("/downloads"),
                        path == "/downloads",
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_start()
                    .pt_2()
                    .child(sidebar_item("Ajustes", "icons/settings.svg", None, false)),
            )
    }
}
