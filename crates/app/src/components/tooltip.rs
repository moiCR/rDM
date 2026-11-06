use gpui::*;

use crate::theme::ActiveTheme;

pub struct ActionTooltip {
    label: SharedString,
    shortcut: Option<SharedString>,
}

impl ActionTooltip {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
        }
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

impl Render for ActionTooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let surface = cx.theme().surface();
        let border = cx.theme().border();
        let text_color = cx.theme().text_color();
        let text_muted = cx.theme().text_muted();
        let surface_hover = cx.theme().surface_hover();

        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(surface)
            .border_1()
            .border_color(border)
            .shadow_md()
            .text_xs()
            .child(div().text_color(text_color).child(self.label.clone()))
            .children(self.shortcut.as_ref().map(|s| {
                div()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .bg(surface_hover)
                    .text_color(text_muted)
                    .child(s.clone())
            }))
    }
}
