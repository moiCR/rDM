use gpui::*;

use crate::{layout::SidebarState, theme::ActiveTheme};

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
        let background = cx.theme().background();
        let surface_hover = cx.theme().surface_hover();
        let text_color = cx.theme().text_color();
        let text_muted = cx.theme().text_muted();
        let error = cx.theme().error();

        div()
            .flex()
            .items_center()
            .justify_between()
            .h_12()
            .w_full()
            .bg(background)
            .text_xs()
            .text_color(text_muted)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .px_4()
                    .h_full()
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("btn-collapse-sidebar")
                            .group("btn-collapse-sidebar")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_8()
                            .rounded_lg()
                            .cursor_pointer()
                            .hover(move |style| style.bg(surface_hover))
                            .on_click(|_event, window, cx| {
                                let current = cx
                                    .try_global::<SidebarState>()
                                    .map(|state| state.collapsed)
                                    .unwrap_or(false);
                                cx.set_global(SidebarState {
                                    collapsed: !current,
                                });
                                window.refresh();
                            })
                            .child(
                                svg()
                                    .path("icons/panel-left.svg")
                                    .size_4()
                                    .text_color(text_muted)
                                    .group_hover("btn-collapse-sidebar", move |style| {
                                        style.text_color(text_color)
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-create-task")
                            .group("btn-create-task")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_8()
                            .rounded_lg()
                            .cursor_pointer()
                            .hover(move |style| style.bg(surface_hover))
                            .on_click(|_event, window, cx| {
                                gpui_router::use_navigate(window, cx).push("/new");
                            })
                            .child(
                                svg()
                                    .path("icons/plus.svg")
                                    .size_4()
                                    .text_color(text_muted)
                                    .group_hover("btn-create-task", move |style| {
                                        style.text_color(text_color)
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .id("titlebar-drag")
                    .window_control_area(WindowControlArea::Drag)
                    .flex()
                    .items_center()
                    .flex_1()
                    .h_full(),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .h_full()
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("window-min")
                            .group("window-min")
                            .window_control_area(WindowControlArea::Min)
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(46.0))
                            .h_full()
                            .hover(move |style| style.bg(surface_hover))
                            .child(
                                svg()
                                    .path("icons/minus.svg")
                                    .size_3p5()
                                    .text_color(text_muted)
                                    .group_hover("window-min", move |style| {
                                        style.text_color(text_color)
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .id("window-max")
                            .group("window-max")
                            .window_control_area(WindowControlArea::Max)
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(46.0))
                            .h_full()
                            .hover(move |style| style.bg(surface_hover))
                            .child(
                                svg()
                                    .path("icons/square.svg")
                                    .size_3()
                                    .text_color(text_muted)
                                    .group_hover("window-max", move |style| {
                                        style.text_color(text_color)
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .id("window-close")
                            .group("window-close")
                            .window_control_area(WindowControlArea::Close)
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(46.0))
                            .h_full()
                            .hover(move |style| style.bg(error))
                            .child(
                                svg()
                                    .path("icons/x.svg")
                                    .size_3p5()
                                    .text_color(text_muted)
                                    .group_hover("window-close", |style| {
                                        style.text_color(rgb(0xffffff))
                                    }),
                            ),
                    ),
            )
    }
}
