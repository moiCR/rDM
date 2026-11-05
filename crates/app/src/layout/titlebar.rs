use gpui::*;

use crate::theme::ActiveTheme;

#[derive(IntoElement)]
pub struct Titlebar;

impl Titlebar {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Titlebar {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Titlebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .h_9()
            .w_full()
            .bg(cx.theme().surface())
            .border_b_1()
            .border_color(cx.theme().border())
            .px_4()
            .text_xs()
            .text_color(cx.theme().text_muted())
            .child("rDM — Internet Download Manager")
    }
}
