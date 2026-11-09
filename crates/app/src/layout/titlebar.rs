use gpui::*;
use gpui_kit::component::input::{Input, InputState};

use crate::{
    components::{ActionTooltip, IconButton},
    layout::toggle_sidebar,
    state::{CoreContext, QuickDownloadState},
    theme::ActiveTheme,
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let background = cx.theme().background();
        let surface_hover = cx.theme().surface_hover();
        let text_color = cx.theme().text_color();
        let text_muted = cx.theme().text_muted();
        let error = cx.theme().error();

        let is_expanded = cx
            .try_global::<QuickDownloadState>()
            .map(|s| s.expanded)
            .unwrap_or(false);

        let quick_input = window.use_keyed_state("quick-download-input-state", cx, |window, cx| {
            InputState::new(window, cx).placeholder("Pega URL y presiona Enter...")
        });

        let spring_config = SpringConfig::new(220.0, 24.0, 1.0);
        let quick_spring = SpringAnimation::new(spring_config).to(is_expanded);

        let quick_input_key = quick_input.clone();
        let q_submit = quick_input.clone();
        let q_clear = quick_input.clone();
        let current_text = quick_input.read(cx).text().to_string();
        let has_text = !current_text.trim().is_empty();

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
                            .tooltip("Alternar barra lateral")
                            .shortcut("Ctrl+B")
                            .on_click(|_, window, cx| toggle_sidebar(window, cx))
                    }))
                    .child(
                        div()
                            .id("quick-download-morph")
                            .overflow_hidden()
                            .flex()
                            .items_center()
                            .h_8()
                            .on_click({
                                let q_input = quick_input.clone();
                                move |_event, window, cx| {
                                    if !is_expanded {
                                        cx.set_global(QuickDownloadState { expanded: true });
                                        q_input.update(cx, |s, cx| s.focus(window, cx));
                                        window.refresh();
                                    }
                                }
                            })
                            .on_key_down({
                                let q_state = quick_input_key.clone();
                                move |event, window, cx| {
                                    if !is_expanded {
                                        return;
                                    }
                                    if event.keystroke.key == "escape" {
                                        q_state.update(cx, |s, cx| {
                                            s.set_value("", window, cx);
                                        });
                                        cx.set_global(QuickDownloadState { expanded: false });
                                        window.refresh();
                                    } else if event.keystroke.key == "enter" {
                                        let text = q_state.read(cx).text().to_string();
                                        if !text.trim().is_empty() {
                                            let model = cx.tasks_model().clone();
                                            model.update(cx, |model, cx| {
                                                model.start_download(
                                                    text.trim().to_string(),
                                                    None,
                                                    cx,
                                                );
                                            });
                                            q_state.update(cx, |s, cx| {
                                                s.set_value("", window, cx);
                                            });
                                            cx.set_global(QuickDownloadState { expanded: false });
                                            window.refresh();
                                        }
                                    }
                                }
                            })
                            .child(
                                div()
                                    .id("btn-create-task-icon")
                                    .group("quick-download-icon")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size_8()
                                    .flex_shrink_0()
                                    .rounded(px(8.0))
                                    .cursor_pointer()
                                    .hover(move |style| style.bg(surface_hover))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| {
                                            ActionTooltip::new("Nueva descarga").shortcut("Ctrl+N")
                                        })
                                        .into()
                                    })
                                    .child(
                                        svg()
                                            .path(if is_expanded {
                                                "icons/download.svg"
                                            } else {
                                                "icons/plus.svg"
                                            })
                                            .size_4()
                                            .text_color(text_muted)
                                            .group_hover("quick-download-icon", move |style| {
                                                style.text_color(text_color)
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .w(px(328.0))
                                    .pr_2p5()
                                    .flex_shrink_0()
                                    .child(
                                        div().flex_1().h_full().flex().items_center().child(
                                            Input::new(&quick_input)
                                                .id("quick-download-text-input")
                                                .appearance(false)
                                                .text_size(px(12.0))
                                                .text_color(text_color)
                                                .w_full()
                                                .h_full(),
                                        ),
                                    )
                                    .children(if has_text {
                                        Some(
                                            IconButton::new(
                                                "btn-submit-quick-download",
                                                "icons/plus.svg",
                                            )
                                            .size(px(20.0))
                                            .icon_size(px(12.0))
                                            .on_click(
                                                move |_event, window, cx| {
                                                    let text = q_submit.read(cx).text().to_string();
                                                    if !text.trim().is_empty() {
                                                        let model = cx.tasks_model().clone();
                                                        model.update(cx, |model, cx| {
                                                            model.start_download(
                                                                text.trim().to_string(),
                                                                None,
                                                                cx,
                                                            );
                                                        });
                                                        q_submit.update(cx, |s, cx| {
                                                            s.set_value("", window, cx);
                                                        });
                                                        cx.set_global(QuickDownloadState {
                                                            expanded: false,
                                                        });
                                                        window.refresh();
                                                    }
                                                },
                                            ),
                                        )
                                    } else {
                                        None
                                    })
                                    .child(
                                        IconButton::new("btn-close-quick-download", "icons/x.svg")
                                            .size(px(20.0))
                                            .icon_size(px(12.0))
                                            .on_click(move |_event, window, cx| {
                                                q_clear.update(cx, |s, cx| {
                                                    s.set_value("", window, cx);
                                                });
                                                cx.set_global(QuickDownloadState {
                                                    expanded: false,
                                                });
                                                window.refresh();
                                            }),
                                    ),
                            )
                            .with_spring("quick-download-spring", quick_spring, |this, phase| {
                                this.w(phase.interpolate_clamped(px(32.0), px(360.0)))
                            }),
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
