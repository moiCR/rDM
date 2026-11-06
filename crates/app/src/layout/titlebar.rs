use gpui::*;

use crate::{
    layout::SidebarState,
    state::{CoreContext, QuickDownloadState},
    theme::ActiveTheme,
};

#[derive(Default)]
pub struct QuickDownloadInputState {
    pub text: String,
}

struct ActionTooltip {
    label: SharedString,
    shortcut: Option<SharedString>,
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let background = cx.theme().background();
        let surface_hover = cx.theme().surface_hover();
        let border = cx.theme().border();
        let text_color = cx.theme().text_color();
        let text_muted = cx.theme().text_muted();
        let accent = cx.theme().accent();
        let error = cx.theme().error();

        let is_expanded = cx
            .try_global::<QuickDownloadState>()
            .map(|s| s.expanded)
            .unwrap_or(false);

        let input_focus = window
            .use_keyed_state("quick-download-input-focus", cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();

        let input_state = window.use_keyed_state("quick-download-input-state", cx, |_, _| {
            QuickDownloadInputState::default()
        });

        if is_expanded && !input_focus.is_focused(window) {
            window.focus(&input_focus, cx);
        }

        let spring_config = SpringConfig::new(220.0, 24.0, 1.0);
        let quick_spring = SpringAnimation::new(spring_config).to(is_expanded);

        let current_text = input_state.read(cx).text.clone();

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
                            .tooltip(|_, cx| {
                                cx.new(|_| ActionTooltip {
                                    label: "Alternar barra lateral".into(),
                                    shortcut: Some("Ctrl+B".into()),
                                })
                                .into()
                            })
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
                            .id("quick-download-container")
                            .overflow_hidden()
                            .h_8()
                            .with_spring(
                                "quick-download-spring",
                                quick_spring,
                                move |this, phase| {
                                    let width = phase.interpolate_clamped(px(32.0), px(360.0));
                                    this.w(width)
                                },
                            )
                            .child(if !is_expanded {
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
                                    .tooltip(|_, cx| {
                                        cx.new(|_| ActionTooltip {
                                            label: "Nueva descarga".into(),
                                            shortcut: Some("Ctrl+N".into()),
                                        })
                                        .into()
                                    })
                                    .on_click(|_event, window, cx| {
                                        cx.set_global(QuickDownloadState { expanded: true });
                                        window.refresh();
                                    })
                                    .child(
                                        svg()
                                            .path("icons/plus.svg")
                                            .size_4()
                                            .text_color(text_muted)
                                            .group_hover("btn-create-task", move |style| {
                                                style.text_color(text_color)
                                            }),
                                    )
                                    .into_any_element()
                            } else {
                                let input_state_clone = input_state.clone();
                                let input_state_submit = input_state.clone();
                                let input_state_clear = input_state.clone();

                                div()
                                    .id("quick-download-input-wrapper")
                                    .track_focus(&input_focus)
                                    .key_context("QuickDownloadInput")
                                    .flex()
                                    .items_center()
                                    .h_8()
                                    .w(px(360.0))
                                    .px_3()
                                    .rounded(px(24.0))
                                    .bg(surface_hover)
                                    .border_1()
                                    .border_color(border)
                                    .cursor_text()
                                    .on_key_down(move |event, window, cx| {
                                        if event.keystroke.key == "escape" {
                                            input_state_clone.update(cx, |s, _| s.text.clear());
                                            cx.set_global(QuickDownloadState { expanded: false });
                                            window.refresh();
                                        } else if event.keystroke.key == "enter" {
                                            let text =
                                                input_state_clone.read(cx).text.trim().to_string();
                                            if !text.is_empty() {
                                                let model = cx.tasks_model().clone();
                                                model.update(cx, |model, cx| {
                                                    model.start_download(text, None, cx);
                                                });
                                                input_state_clone.update(cx, |s, _| s.text.clear());
                                                cx.set_global(QuickDownloadState {
                                                    expanded: false,
                                                });
                                                window.refresh();
                                            }
                                        } else if event.keystroke.key == "backspace" {
                                            input_state_clone.update(cx, |s, _| {
                                                s.text.pop();
                                            });
                                            window.refresh();
                                        } else if event.keystroke.modifiers.control
                                            && event.keystroke.key.eq_ignore_ascii_case("v")
                                            && let Some(item) = cx.read_from_clipboard()
                                            && let Some(text) = item.text()
                                        {
                                            input_state_clone.update(cx, |s, _| {
                                                s.text.push_str(text.trim());
                                            });
                                            window.refresh();
                                        } else if !event.keystroke.modifiers.control
                                            && !event.keystroke.modifiers.alt
                                            && !event.keystroke.modifiers.platform
                                            && let Some(ref ch) = event.keystroke.key_char
                                        {
                                            input_state_clone.update(cx, |s, _| {
                                                s.text.push_str(ch);
                                            });
                                            window.refresh();
                                        }
                                    })
                                    .child(
                                        svg()
                                            .path("icons/download.svg")
                                            .size_3p5()
                                            .text_color(accent)
                                            .mr_2()
                                            .flex_shrink_0(),
                                    )
                                    .child(div().flex_1().overflow_hidden().text_xs().child(
                                        if current_text.is_empty() {
                                            div()
                                                .text_color(text_muted)
                                                .child("Pega URL y presiona Enter...")
                                                .into_any_element()
                                        } else {
                                            div()
                                                .flex()
                                                .items_center()
                                                .text_color(text_color)
                                                .child(current_text)
                                                .child(
                                                    div()
                                                        .w(px(1.5))
                                                        .h(px(13.0))
                                                        .bg(accent)
                                                        .ml_0p5(),
                                                )
                                                .into_any_element()
                                        },
                                    ))
                                    .children(if !input_state.read(cx).text.trim().is_empty() {
                                        Some(
                                            div()
                                                .id("btn-submit-quick-download")
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .size_5()
                                                .rounded_full()
                                                .cursor_pointer()
                                                .hover(move |s| s.bg(surface_hover))
                                                .on_click(move |_event, window, cx| {
                                                    let text = input_state_submit
                                                        .read(cx)
                                                        .text
                                                        .trim()
                                                        .to_string();
                                                    if !text.is_empty() {
                                                        let model = cx.tasks_model().clone();
                                                        model.update(cx, |model, cx| {
                                                            model.start_download(text, None, cx);
                                                        });
                                                        input_state_submit
                                                            .update(cx, |s, _| s.text.clear());
                                                        cx.set_global(QuickDownloadState {
                                                            expanded: false,
                                                        });
                                                        window.refresh();
                                                    }
                                                })
                                                .child(
                                                    svg()
                                                        .path("icons/plus.svg")
                                                        .size_3()
                                                        .text_color(accent),
                                                ),
                                        )
                                    } else {
                                        None
                                    })
                                    .child(
                                        div()
                                            .id("btn-close-quick-download")
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size_5()
                                            .rounded_full()
                                            .cursor_pointer()
                                            .hover(move |s| s.bg(surface_hover))
                                            .on_click(move |_event, window, cx| {
                                                input_state_clear.update(cx, |s, _| s.text.clear());
                                                cx.set_global(QuickDownloadState {
                                                    expanded: false,
                                                });
                                                window.refresh();
                                            })
                                            .child(
                                                svg()
                                                    .path("icons/x.svg")
                                                    .size_3()
                                                    .text_color(text_muted),
                                            ),
                                    )
                                    .into_any_element()
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
