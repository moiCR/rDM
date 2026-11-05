use gpui::{prelude::FluentBuilder, *};

pub fn sidebar_item(
    label: &'static str,
    path: &'static str,
    active: bool,
    badge: Option<usize>,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(label.into()))
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .when(active, |this| {
            this.bg(hsla(220.0 / 360.0, 0.13, 0.18, 1.0))
                .font_weight(FontWeight::SEMIBOLD)
        })
        .hover(|this| this.bg(hsla(220.0 / 360.0, 0.13, 0.15, 1.0)))
        .on_click(move |_event, window, cx| {
            gpui_router::use_navigate(window, cx).push(path);
        })
        .child(label)
        .children(badge.map(|count| {
            div()
                .text_xs()
                .px_2()
                .py_0p5()
                .rounded_full()
                .bg(hsla(217.0 / 360.0, 0.91, 0.60, 0.2))
                .text_color(hsla(217.0 / 360.0, 0.91, 0.60, 1.0))
                .child(count.to_string())
        }))
}
