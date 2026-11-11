use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::{
    components::IconButton,
    lang::ActiveLang,
    layout::toggle_sidebar,
    state::{CoreContext, UpdaterStatus},
    theme::ActiveTheme,
    views::{NewDownloadModal, SettingsModalState, SettingsTab},
};

#[derive(IntoElement)]
pub struct Titlebar {
    show_sidebar_toggle: bool,
}

impl Titlebar {
    pub fn new() -> Self {
        Self {
            show_sidebar_toggle: true,
        }
    }

    pub(super) fn sidebar_toggle(mut self, show: bool) -> Self {
        self.show_sidebar_toggle = show;
        self
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

        let updater_status = cx.tasks_model().read(cx).updater_status().clone();
        let update_badge = match updater_status {
            UpdaterStatus::Available(info) => Some((
                cx.lang()
                    .get_with("titlebar.update_available", &[("version", &info.version)]),
                false,
            )),
            UpdaterStatus::Downloading {
                downloaded_bytes,
                total_bytes,
                ..
            } => {
                let pct = if total_bytes > 0 {
                    (downloaded_bytes as f64 / total_bytes as f64 * 100.0) as u32
                } else {
                    0
                };
                Some((
                    cx.lang()
                        .get_with("titlebar.updating", &[("percent", &pct.to_string())]),
                    true,
                ))
            }
            _ => None,
        };
        let accent = cx.theme().accent();

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
                    .children(self.show_sidebar_toggle.then(|| {
                        IconButton::new("btn-collapse-sidebar", "icons/panel-left.svg")
                            .tooltip(cx.lang().get("titlebar.toggle_sidebar"))
                            .shortcut("Ctrl+B")
                            .on_click(|_, window, cx| toggle_sidebar(window, cx))
                    }))
                    .child(
                        IconButton::new("btn-new-download", "icons/plus.svg")
                            .rounded(px(24.0))
                            .tooltip(cx.lang().get("titlebar.new_download"))
                            .shortcut("Ctrl+N")
                            .on_click(|_, window, cx| NewDownloadModal::open(None, window, cx)),
                    ),
            )
            .child(
                div()
                    .id("titlebar-drag")
                    .window_control_area(WindowControlArea::Drag)
                    .when(cfg!(target_os = "linux"), |element| {
                        let should_move = _window.use_state(cx, |_, _| false);
                        element
                            .on_mouse_down_out(
                                _window.listener_for(&should_move, |should_move, _, _, _| {
                                    *should_move = false
                                }),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                _window.listener_for(
                                    &should_move,
                                    |should_move, event: &MouseDownEvent, window, _| {
                                        *should_move = event.click_count == 1;
                                        if event.click_count == 2 {
                                            window.zoom_window();
                                        }
                                    },
                                ),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                _window.listener_for(&should_move, |should_move, _, _, _| {
                                    *should_move = false;
                                }),
                            )
                            .on_mouse_move(_window.listener_for(
                                &should_move,
                                |should_move, event: &MouseMoveEvent, window, _| {
                                    if *should_move {
                                        *should_move = false;
                                        if event.pressed_button == Some(MouseButton::Left) {
                                            window.start_window_move();
                                        }
                                    }
                                },
                            ))
                    })
                    .flex()
                    .items_center()
                    .flex_1()
                    .h_full(),
            )
            .children(update_badge.map(|(label, is_downloading)| {
                div()
                    .id("btn-titlebar-update-badge")
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .px_2p5()
                    .py_1()
                    .mr_2()
                    .rounded_full()
                    .bg(accent.opacity(0.15))
                    .border_1()
                    .border_color(accent.opacity(0.35))
                    .text_color(accent)
                    .text_xs()
                    .cursor_pointer()
                    .hover(move |s| s.bg(accent.opacity(0.25)))
                    .on_click(move |_event, window, cx| {
                        cx.set_global(SettingsModalState {
                            open: true,
                            tab: SettingsTab::Advanced,
                        });
                        window.refresh();
                    })
                    .child(
                        svg()
                            .path(if is_downloading {
                                "icons/loader.svg"
                            } else {
                                "icons/download.svg"
                            })
                            .size_3()
                            .text_color(accent),
                    )
                    .child(label)
            }))
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
                            .when(cfg!(target_os = "linux"), |element| {
                                element.on_click(|_, window, _| window.minimize_window())
                            })
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
                            .when(cfg!(target_os = "linux"), |element| {
                                element.on_click(|_, window, _| window.zoom_window())
                            })
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
                            .when(cfg!(target_os = "linux"), |element| {
                                element.on_click(|_, window, _| window.remove_window())
                            })
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
