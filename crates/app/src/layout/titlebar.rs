use gpui::*;

use crate::theme::ActiveTheme;

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

impl Render for Titlebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
