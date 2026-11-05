use gpui::*;

use crate::theme::ActiveTheme;

#[derive(IntoElement)]
pub struct DownloadsView;

impl DownloadsView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DownloadsView {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DownloadsView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background())
            .text_color(cx.theme().text_color())
            .p_8()
            .child("Downloads")
    }
}
