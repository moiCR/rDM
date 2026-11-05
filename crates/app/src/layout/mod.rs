pub mod sidebar;
pub mod sidebar_item;
pub mod titlebar;

use gpui::*;
use gpui_router::Layout;

use crate::{
    layout::{sidebar::Sidebar, titlebar::Titlebar},
    theme::ActiveTheme,
};

#[derive(Default)]
pub struct AppLayout {
    outlet: Option<AnyElement>,
}

impl AppLayout {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Layout for AppLayout {
    fn outlet(&mut self, element: AnyElement) {
        self.outlet = Some(element);
    }

    fn render_layout(self: Box<Self>, _window: &mut Window, cx: &mut App) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background())
            .text_color(cx.theme().text_color())
            .child(Titlebar::new())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .size_full()
                    .child(Sidebar::new())
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .size_full()
                            .overflow_hidden()
                            .children(self.outlet),
                    ),
            )
            .into_any_element()
    }
}
