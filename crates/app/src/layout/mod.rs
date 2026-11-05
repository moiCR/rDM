pub mod sidebar;
pub mod sidebar_item;
pub mod titlebar;

use gpui::*;

use crate::{
    layout::{sidebar::Sidebar, titlebar::Titlebar},
    router::app_routes,
    theme::ActiveTheme,
};

pub struct AppLayout {
    titlebar: Entity<Titlebar>,
    sidebar: Entity<Sidebar>,
}

impl AppLayout {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let titlebar = cx.new(|_cx| Titlebar::new());
        let sidebar = cx.new(|_cx| Sidebar::new());
        Self { titlebar, sidebar }
    }
}

impl Render for AppLayout {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background())
            .text_color(cx.theme().text_color())
            .child(self.titlebar.clone())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .size_full()
                    .child(self.sidebar.clone())
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .size_full()
                            .overflow_hidden()
                            .child(app_routes()),
                    ),
            )
    }
}
