use gpui::*;

use crate::theme::ActiveTheme;

#[derive(IntoElement)]
pub struct DashboardView;

impl DashboardView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DashboardView {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DashboardView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background())
            .text_color(cx.theme().text_color())
            .p_8()
            .child("Dashboard")
    }
}
