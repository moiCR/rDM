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
            .justify_between()
            .w(px(220.0))
            .h_full()
            .bg(cx.theme().surface())
            .border_r_1()
            .border_color(cx.theme().border())
            .p_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_3()
                            .font_weight(FontWeight::BOLD)
                            .text_base()
                            .text_color(cx.theme().accent())
                            .child("rDM"),
                    )
                    .child(sidebar_item("Descargas", "/", path == "/", None))
                    .child(sidebar_item("Nueva descarga", "/new", path == "/new", None)),
            )
            .child(div().flex().flex_col().gap_1().child(sidebar_item(
                "Ajustes",
                "/settings",
                path == "/settings",
                None,
            )))
    }
}
