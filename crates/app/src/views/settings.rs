use gpui::*;
use gpui_kit::{
    base::{Scrollbar, ScrollbarMode},
    component::button::{ButtonCustomVariant, ButtonRounded},
};
use std::{path::PathBuf, sync::Arc};

use crate::{
    components::{
        ActionTooltip, Button, ButtonVariants, IconButton, Input, InputState, Modal, PillButton,
        input::themed_input,
    },
    layout::NavigationStyle,
    state::{CoreContext, UpdaterStatus},
    theme::{ActiveTheme, PrimaryColors, ThemeManager, parse_hex_color},
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    #[default]
    Downloads,
    Appearance,
    Advanced,
}

#[derive(Default, Clone, Copy)]
pub struct SettingsModalState {
    pub open: bool,
    pub tab: SettingsTab,
}

impl Global for SettingsModalState {}

#[derive(Clone, Copy)]
struct ModalPalette {
    surface_hover: Rgba,
    control_surface: Rgba,
    text_color: Rgba,
    text_muted: Rgba,
    accent: Rgba,
    primary: PrimaryColors,
}

pub struct SettingsInputs {
    pub speed: Entity<InputState>,
    pub connections: Entity<InputState>,
    pub max_concurrent: Entity<InputState>,
    pub download_dir: Entity<InputState>,
    pub user_agent: Entity<InputState>,
    _subs: Vec<Subscription>,
}

#[derive(Clone)]
pub struct SettingsFormState {
    pub theme: String,
    pub accent_color: String,
}

impl SettingsFormState {
    pub fn from_ui_config(ui_config: &rdm_config::UIConfig) -> Self {
        Self {
            theme: ui_config.theme.clone(),
            accent_color: ui_config.accent_override().unwrap_or_default().to_owned(),
        }
    }
}

#[derive(IntoElement)]
pub struct SettingsModal;

impl SettingsModal {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SettingsModal {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for SettingsModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let modal_state = cx
            .try_global::<SettingsModalState>()
            .copied()
            .unwrap_or_default();

        let core = cx.core().clone();
        let config_manager = core.config().clone();
        let current_cfg = config_manager.config();
        let current_ui_cfg = config_manager.ui_config();

        let form_state = window.use_keyed_state("settings-form-state", cx, |_, _| {
            SettingsFormState::from_ui_config(&current_ui_cfg)
        });

        let form = form_state.read(cx).clone();

        let cfg_for_inputs = config_manager.clone();
        let core_for_inputs = core.clone();
        let inputs_state = window.use_keyed_state("settings-inputs-state", cx, |window, cx| {
            let speed = cx.new(|cx| {
                let mb = if current_cfg.max_speed_bytes_per_sec > 0 {
                    format!(
                        "{:.1}",
                        current_cfg.max_speed_bytes_per_sec as f64 / 1048576.0
                    )
                } else {
                    "0".to_string()
                };
                let mut s = InputState::new(window, cx)
                    .placeholder("0")
                    .validate(|text, _| {
                        text.is_empty()
                            || text.parse::<f64>().is_ok()
                            || (text.ends_with('.')
                                && text[..text.len() - 1].parse::<f64>().is_ok())
                    });
                s.set_value(mb, window, cx);
                s
            });

            let connections = cx.new(|cx| {
                let mut s = InputState::new(window, cx)
                    .placeholder("8")
                    .validate(|text, _| {
                        text.is_empty()
                            || (text.chars().all(|c| c.is_ascii_digit())
                                && text.parse::<usize>().map(|v| v <= 32).unwrap_or(false))
                    });
                s.set_value(current_cfg.default_connections.to_string(), window, cx);
                s
            });

            let max_concurrent = cx.new(|cx| {
                let mut s = InputState::new(window, cx)
                    .placeholder("3")
                    .validate(|text, _| {
                        text.is_empty()
                            || (text.chars().all(|c| c.is_ascii_digit())
                                && text.parse::<usize>().map(|v| v <= 16).unwrap_or(false))
                    });
                s.set_value(current_cfg.max_concurrent_downloads.to_string(), window, cx);
                s
            });

            let download_dir = cx.new(|cx| {
                let mut s = InputState::new(window, cx).placeholder("C:\\Downloads");
                s.set_value(
                    current_cfg.download_dir.to_string_lossy().to_string(),
                    window,
                    cx,
                );
                s
            });

            let user_agent = cx.new(|cx| {
                let mut s = InputState::new(window, cx).placeholder("Escribe o pega User-Agent...");
                s.set_value(current_cfg.user_agent.clone(), window, cx);
                s
            });

            let mut subs = Vec::new();

            let c1 = cfg_for_inputs.clone();
            subs.push(window.observe(&speed, cx, move |entity, _, cx| {
                let text = entity.read(cx).text().to_string();
                if let Ok(mb) = text.trim().parse::<f64>() {
                    let bytes = (mb * 1024.0 * 1024.0) as u64;
                    let _ = c1.update_config(|c| c.max_speed_bytes_per_sec = bytes);
                }
            }));

            let c2 = cfg_for_inputs.clone();
            subs.push(window.observe(&connections, cx, move |entity, _, cx| {
                let text = entity.read(cx).text().to_string();
                if let Ok(val) = text.trim().parse::<usize>() {
                    let _ = c2.update_config(|c| c.default_connections = val.clamp(1, 32));
                }
            }));

            let c_concurrent = cfg_for_inputs.clone();
            let core_queue = core_for_inputs.clone();
            subs.push(window.observe(&max_concurrent, cx, move |entity, _, cx| {
                let text = entity.read(cx).text().to_string();
                if let Ok(val) = text.trim().parse::<usize>() {
                    let _ = c_concurrent
                        .update_config(|c| c.max_concurrent_downloads = val.clamp(1, 16));
                    let engine = core_queue.engine().clone();
                    cx.tasks_model().read(cx).tokio_handle().spawn(async move {
                        engine.process_queue().await;
                    });
                }
            }));

            let c3 = cfg_for_inputs.clone();
            subs.push(window.observe(&download_dir, cx, move |entity, _, cx| {
                let text = entity.read(cx).text().to_string();
                let p = PathBuf::from(text.trim());
                if !p.as_os_str().is_empty() {
                    let _ = c3.update_config(|c| c.download_dir = p);
                }
            }));

            let c4 = cfg_for_inputs.clone();
            subs.push(window.observe(&user_agent, cx, move |entity, _, cx| {
                let text = entity.read(cx).text().to_string();
                if !text.trim().is_empty() {
                    let _ = c4.update_config(|c| c.user_agent = text);
                }
            }));

            SettingsInputs {
                speed,
                connections,
                max_concurrent,
                download_dir,
                user_agent,
                _subs: subs,
            }
        });

        let palette = ModalPalette {
            surface_hover: cx.theme().surface_hover(),
            control_surface: cx
                .theme()
                .input_background()
                .unwrap_or_else(|| cx.theme().surface_hover()),
            text_color: cx.theme().text_color(),
            text_muted: cx.theme().text_muted(),
            accent: cx.theme().accent(),
            primary: cx.theme().primary(),
        };
        let viewport = window.viewport_size();
        let tab_index = modal_state.tab as usize;
        let scroll_handle = window
            .use_keyed_state(("settings-scroll-handle", tab_index), cx, |_, _| {
                ScrollHandle::new()
            })
            .read(cx)
            .clone();
        let inputs = inputs_state.read(cx);

        let content = match modal_state.tab {
            SettingsTab::Downloads => Self::render_downloads(inputs, palette, &config_manager, cx),
            SettingsTab::Appearance => Self::render_appearance(
                &form.theme,
                NavigationStyle::from_key(&current_ui_cfg.sidebar_style),
                &form.accent_color,
                form_state,
                palette,
                config_manager,
            ),
            SettingsTab::Advanced => Self::render_advanced(
                inputs,
                palette,
                &config_manager,
                current_ui_cfg.auto_check_updates,
                cx,
            ),
        };

        Modal::new("settings-modal")
            .open(modal_state.open)
            .compact()
            .width((viewport.width - px(32.0)).clamp(px(0.0), px(620.0)))
            .max_height((viewport.height - px(32.0)).clamp(px(0.0), px(400.0)))
            .header(Self::render_header(modal_state.tab, palette, cx))
            .on_close(Self::close)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id(("settings-content", tab_index))
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&scroll_handle)
                            .pr_3()
                            .child(content),
                    )
                    .child(Scrollbar::vertical(&scroll_handle).mode(ScrollbarMode::Always)),
            )
    }
}

impl SettingsModal {
    fn render_header(tab: SettingsTab, palette: ModalPalette, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div().flex().flex_wrap().gap_1().children(
                    [
                        (SettingsTab::Downloads, "Descargas"),
                        (SettingsTab::Appearance, "Apariencia"),
                        (SettingsTab::Advanced, "Avanzado"),
                    ]
                    .map(|(section, label)| {
                        let selected = section == tab;
                        let background = if selected {
                            palette.surface_hover.into()
                        } else {
                            transparent_black()
                        };
                        let foreground = if selected {
                            palette.text_color
                        } else {
                            palette.text_muted
                        };

                        Button::new(("settings-tab", section as usize))
                            .label(label)
                            .toggled(selected)
                            .rounded(px(24.0))
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(background)
                                    .hover(background)
                                    .active(background)
                                    .foreground(foreground.into()),
                            )
                            .on_click(move |_, window, cx| {
                                if !selected {
                                    cx.set_global(SettingsModalState {
                                        open: true,
                                        tab: section,
                                    });
                                    window.refresh();
                                }
                            })
                    }),
                ),
            )
            .child(
                div().flex_none().child(
                    IconButton::new("btn-close-settings", "icons/x.svg")
                        .rounded(px(99.0))
                        .color(palette.text_muted)
                        .hover_bg(palette.surface_hover)
                        .tooltip("Cerrar")
                        .on_click(|_, window, cx| Self::close(window, cx)),
                ),
            )
    }

    fn close(window: &mut Window, cx: &mut App) {
        let tab = cx
            .try_global::<SettingsModalState>()
            .map_or(SettingsTab::default(), |state| state.tab);
        cx.set_global(SettingsModalState { open: false, tab });
        window.blur(cx);
        window.refresh();
    }

    fn settings_input(state: &Entity<InputState>, cx: &App) -> Input {
        themed_input(state, cx)
            .rounded(px(24.0))
            .border_color(transparent_black())
    }

    fn step_button(id: &'static str, icon: &'static str, palette: ModalPalette) -> IconButton {
        IconButton::new(id, icon)
            .size(px(34.0))
            .icon_size(px(14.0))
            .rounded(px(24.0))
            .color(palette.text_muted)
            .background(palette.control_surface)
            .hover_bg(palette.surface_hover)
    }

    fn render_downloads(
        inputs: &SettingsInputs,
        palette: ModalPalette,
        config_manager: &Arc<rdm_config::ConfigManager>,
        cx: &App,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap_3()
                    .py_1()
                    .child(
                        div()
                            .flex_1()
                            .flex_basis(px(172.0))
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(14.0))
                                    .text_color(palette.text_color)
                                    .child("Límite de velocidad global"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(palette.text_muted)
                                    .child("Usa 0 para velocidad sin límites."),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_none()
                            .gap_1()
                            .child(
                                Self::step_button("btn-speed-dec", "icons/minus.svg", palette)
                                    .on_click({
                                        let speed = inputs.speed.clone();
                                        move |_, window, cx| {
                                            speed.update(cx, |state, cx| {
                                                let current = state
                                                    .value()
                                                    .trim()
                                                    .parse::<f64>()
                                                    .unwrap_or(0.0);
                                                let next = (current - 1.0).max(0.0);
                                                state.set_value(format!("{next:.1}"), window, cx);
                                            });
                                            window.refresh();
                                        }
                                    }),
                            )
                            .child(
                                div().w(px(72.0)).child(
                                    Self::settings_input(&inputs.speed, cx)
                                        .id("input-speed-limit")
                                        .text_align(TextAlign::Center)
                                        .h(px(36.0))
                                        .w_full(),
                                ),
                            )
                            .child(
                                Self::step_button("btn-speed-inc", "icons/plus.svg", palette)
                                    .on_click({
                                        let speed = inputs.speed.clone();
                                        move |_, window, cx| {
                                            speed.update(cx, |state, cx| {
                                                let current = state
                                                    .value()
                                                    .trim()
                                                    .parse::<f64>()
                                                    .unwrap_or(0.0);
                                                let next = (current + 1.0).min(1000.0);
                                                state.set_value(format!("{next:.1}"), window, cx);
                                            });
                                            window.refresh();
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.text_muted)
                                    .child("MB/s"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap_3()
                    .py_1()
                    .child(
                        div()
                            .flex_1()
                            .flex_basis(px(172.0))
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(14.0))
                                    .text_color(palette.text_color)
                                    .child("Conexiones simultáneas"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(palette.text_muted)
                                    .child("Segmentos concurrentes por descarga (1 a 32)."),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_none()
                            .gap_1()
                            .child(
                                Self::step_button("btn-conn-dec", "icons/minus.svg", palette)
                                    .on_click({
                                        let conn = inputs.connections.clone();
                                        move |_, window, cx| {
                                            conn.update(cx, |state, cx| {
                                                let current = state
                                                    .value()
                                                    .trim()
                                                    .parse::<usize>()
                                                    .unwrap_or(8);
                                                let next = current.saturating_sub(1).clamp(1, 32);
                                                state.set_value(next.to_string(), window, cx);
                                            });
                                            window.refresh();
                                        }
                                    }),
                            )
                            .child(
                                div().w(px(54.0)).child(
                                    Self::settings_input(&inputs.connections, cx)
                                        .id("input-connections")
                                        .text_align(TextAlign::Center)
                                        .h(px(36.0))
                                        .w_full(),
                                ),
                            )
                            .child(
                                Self::step_button("btn-conn-inc", "icons/plus.svg", palette)
                                    .on_click({
                                        let conn = inputs.connections.clone();
                                        move |_, window, cx| {
                                            conn.update(cx, |state, cx| {
                                                let current = state
                                                    .value()
                                                    .trim()
                                                    .parse::<usize>()
                                                    .unwrap_or(8);
                                                let next = (current + 1).clamp(1, 32);
                                                state.set_value(next.to_string(), window, cx);
                                            });
                                            window.refresh();
                                        }
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap_3()
                    .py_1()
                    .child(
                        div()
                            .flex_1()
                            .flex_basis(px(172.0))
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(14.0))
                                    .text_color(palette.text_color)
                                    .child("Descargas simultáneas"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(palette.text_muted)
                                    .child("Descargas activas al mismo tiempo (1 a 16)."),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_none()
                            .gap_1()
                            .child(
                                Self::step_button("btn-concurrent-dec", "icons/minus.svg", palette)
                                    .on_click({
                                        let conc = inputs.max_concurrent.clone();
                                        move |_, window, cx| {
                                            conc.update(cx, |state, cx| {
                                                let current = state
                                                    .value()
                                                    .trim()
                                                    .parse::<usize>()
                                                    .unwrap_or(3);
                                                let next = current.saturating_sub(1).clamp(1, 16);
                                                state.set_value(next.to_string(), window, cx);
                                            });
                                            window.refresh();
                                        }
                                    }),
                            )
                            .child(
                                div().w(px(54.0)).child(
                                    Self::settings_input(&inputs.max_concurrent, cx)
                                        .id("input-max-concurrent")
                                        .text_align(TextAlign::Center)
                                        .h(px(36.0))
                                        .w_full(),
                                ),
                            )
                            .child(
                                Self::step_button("btn-concurrent-inc", "icons/plus.svg", palette)
                                    .on_click({
                                        let conc = inputs.max_concurrent.clone();
                                        move |_, window, cx| {
                                            conc.update(cx, |state, cx| {
                                                let current = state
                                                    .value()
                                                    .trim()
                                                    .parse::<usize>()
                                                    .unwrap_or(3);
                                                let next = (current + 1).clamp(1, 16);
                                                state.set_value(next.to_string(), window, cx);
                                            });
                                            window.refresh();
                                        }
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .py_1()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(14.0))
                                    .text_color(palette.text_color)
                                    .child("Carpeta de descargas"),
                            )
                            .child(div().text_xs().text_color(palette.text_muted).child(
                                "Directorio predeterminado donde se guardarán los archivos.",
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .on_mouse_down(MouseButton::Left, {
                                        let dir = inputs.download_dir.clone();
                                        move |_, window, cx| {
                                            dir.update(cx, |s, cx| s.focus(window, cx));
                                        }
                                    })
                                    .child(
                                        Self::settings_input(&inputs.download_dir, cx)
                                            .id("input-download-dir")
                                            .cleanable(false)
                                            .h(px(36.0))
                                            .w_full(),
                                    ),
                            )
                            .child({
                                let dir_entity = inputs.download_dir.clone();
                                let cfg_dir = config_manager.clone();
                                IconButton::new("btn-browse-download-dir", "icons/folder-open.svg")
                                    .size(px(36.0))
                                    .icon_size(px(16.0))
                                    .rounded(px(24.0))
                                    .hover_bg(palette.surface_hover)
                                    .color(palette.text_muted)
                                    .on_click(move |_event, window, cx| {
                                        let prompt = cx.prompt_for_paths(PathPromptOptions {
                                            files: false,
                                            directories: true,
                                            multiple: false,
                                            prompt: Some(
                                                "Selecciona la carpeta de descargas".into(),
                                            ),
                                        });
                                        let dir_target = dir_entity.clone();
                                        let cfg_target = cfg_dir.clone();
                                        window
                                            .spawn(cx, async move |cx| {
                                                if let Ok(Ok(Some(paths))) = prompt.await
                                                    && let Some(first) = paths.into_iter().next()
                                                {
                                                    let path_str =
                                                        first.to_string_lossy().to_string();
                                                    let path_buf = first.clone();
                                                    let _ = cfg_target.update_config(|c| {
                                                        c.download_dir = path_buf;
                                                    });
                                                    let _ = cx.update(|window, cx| {
                                                        dir_target.update(cx, |s, cx| {
                                                            s.set_value(path_str, window, cx);
                                                        });
                                                        window.refresh();
                                                    });
                                                }
                                            })
                                            .detach();
                                    })
                            }),
                    ),
            )
    }

    fn render_advanced(
        inputs: &SettingsInputs,
        palette: ModalPalette,
        config_manager: &Arc<rdm_config::ConfigManager>,
        auto_check: bool,
        cx: &App,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .py_1()
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(14.0))
                                    .text_color(palette.text_color)
                                    .child("User-Agent"),
                            )
                            .child({
                                let ua_entity = inputs.user_agent.clone();
                                let cfg_reset = config_manager.clone();
                                div()
                                    .id("btn-reset-user-agent")
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(palette.accent)
                                    .hover(move |s| s.underline())
                                    .on_click(move |_event, window, cx| {
                                        let default_ua = rdm_config::get_user_agent();
                                        let _ = cfg_reset.update_config(|c| {
                                            c.user_agent = default_ua.clone();
                                        });
                                        ua_entity.update(cx, |s, cx| {
                                            s.set_value(default_ua, window, cx);
                                        });
                                        window.refresh();
                                    })
                                    .child("Restaurar por defecto")
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .on_mouse_down(MouseButton::Left, {
                                let ua = inputs.user_agent.clone();
                                move |_, window, cx| {
                                    ua.update(cx, |s, cx| s.focus(window, cx));
                                }
                            })
                            .child(
                                Self::settings_input(&inputs.user_agent, cx)
                                    .id("input-user-agent")
                                    .cleanable(true)
                                    .h(px(36.0))
                                    .w_full(),
                            ),
                    ),
            )
            .child({
                let updater_status = cx.tasks_model().read(cx).updater_status().clone();
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap_3()
                    .py_1()
                    .child(
                        div()
                            .flex_1()
                            .flex_basis(px(175.0))
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(14.0))
                                    .text_color(palette.text_color)
                                    .child(format!(
                                        "Actualizaciones (v{})",
                                        rdm_core::updater::current_version()
                                    )),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(palette.text_muted)
                                    .child(match &updater_status {
                                        UpdaterStatus::Idle => {
                                            "Comprueba si hay nuevas versiones de rDM disponibles.".to_string()
                                        }
                                        UpdaterStatus::Checking => {
                                            "Buscando actualizaciones en GitHub...".to_string()
                                        }
                                        UpdaterStatus::UpToDate => {
                                            "rDM está actualizado a la versión más reciente.".to_string()
                                        }
                                        UpdaterStatus::Available(info) => {
                                            format!("¡Versión v{} disponible para instalar!", info.version)
                                        }
                                        UpdaterStatus::Downloading {
                                            downloaded_bytes,
                                            total_bytes,
                                            ..
                                        } => {
                                            if *total_bytes > 0 {
                                                format!(
                                                    "Descargando instalador: {:.1} MB / {:.1} MB ({:.0}%)",
                                                    *downloaded_bytes as f64 / 1048576.0,
                                                    *total_bytes as f64 / 1048576.0,
                                                    (*downloaded_bytes as f64 / *total_bytes as f64) * 100.0
                                                )
                                            } else {
                                                format!(
                                                    "Descargando instalador: {:.1} MB...",
                                                    *downloaded_bytes as f64 / 1048576.0
                                                )
                                            }
                                        }
                                        UpdaterStatus::Installing => {
                                            "Instalando actualización y reiniciando rDM...".to_string()
                                        }
                                        UpdaterStatus::Error(e) => {
                                            format!("Error de actualización: {e}")
                                        }
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(match &updater_status {
                                UpdaterStatus::Available(_) => {
                                    let tasks_model = cx.tasks_model().clone();
                                    PillButton::new("btn-install-update", "Actualizar ahora")
                                        .selected(true)
                                        .on_click(move |_, window, cx| {
                                            tasks_model.update(cx, |model, cx| {
                                                model.install_update(cx);
                                            });
                                            window.refresh();
                                        })
                                }
                                UpdaterStatus::Checking
                                | UpdaterStatus::Downloading { .. }
                                | UpdaterStatus::Installing => {
                                    PillButton::new("btn-checking-update", "Comprobando...")
                                        .selected(false)
                                }
                                _ => {
                                    let tasks_model = cx.tasks_model().clone();
                                    PillButton::new("btn-check-updates", "Buscar actualizaciones")
                                        .selected(false)
                                        .on_click(move |_, window, cx| {
                                            tasks_model.update(cx, |model, cx| {
                                                model.check_for_updates(cx);
                                            });
                                            window.refresh();
                                        })
                                }
                            }),
                    )
            })
            .child({
                let cfg_auto_check = config_manager.clone();
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap_3()
                    .py_1()
                    .child(
                        div()
                            .flex_1()
                            .flex_basis(px(175.0))
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(14.0))
                                    .text_color(palette.text_color)
                                    .child("Búsqueda automática"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(palette.text_muted)
                                    .child("Comprobar nuevas versiones automáticamente al iniciar la app."),
                            ),
                    )
                    .child(
                        PillButton::new(
                            "btn-toggle-auto-check",
                            if auto_check { "Activado" } else { "Desactivado" },
                        )
                        .selected(auto_check)
                        .on_click(move |_, window, _cx| {
                            let next = !auto_check;
                            let _ = cfg_auto_check.update_ui_config(|u| {
                                u.auto_check_updates = next;
                            });
                            window.refresh();
                        }),
                    )
            })
    }

    fn render_appearance(
        current_theme: &str,
        navigation_style: NavigationStyle,
        current_accent: &str,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
        config: Arc<rdm_config::ConfigManager>,
    ) -> Div {
        let row1 = ["#9e9e9e", "#1e1e1e", "#ef4444", "#f97316", "#10b981"];
        let row2 = ["", "#4422ff", "#9900ff", "#ff0099", "#e07070"];

        div()
            .flex()
            .flex_col()
            .gap_3p5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2p5()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_center()
                            .gap_2p5()
                            .children(row1.iter().map(|&hex| {
                                Self::accent_swatch(
                                    hex,
                                    current_accent,
                                    palette,
                                    state.clone(),
                                    config.clone(),
                                )
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_center()
                            .gap_2p5()
                            .children(row2.iter().map(|&hex| {
                                Self::accent_swatch(
                                    hex,
                                    current_accent,
                                    palette,
                                    state.clone(),
                                    config.clone(),
                                )
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.text_color)
                            .child("Navegación"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Self::navigation_button(
                                NavigationStyle::Normal,
                                "Normal",
                                navigation_style,
                                config.clone(),
                            ))
                            .child(Self::navigation_button(
                                NavigationStyle::Dock,
                                "Dock",
                                navigation_style,
                                config.clone(),
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.text_color)
                            .child("Tema"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Self::theme_icon_button(
                                "dark",
                                "icons/moon.svg",
                                current_theme,
                                state.clone(),
                                palette,
                                config.clone(),
                            ))
                            .child(Self::theme_icon_button(
                                "light",
                                "icons/sun.svg",
                                current_theme,
                                state.clone(),
                                palette,
                                config,
                            )),
                    ),
            )
    }

    fn accent_swatch(
        hex: &'static str,
        current: &str,
        palette: ModalPalette,
        state: Entity<SettingsFormState>,
        config: Arc<rdm_config::ConfigManager>,
    ) -> impl IntoElement {
        let is_selected = current.eq_ignore_ascii_case(hex);
        let is_default = hex.is_empty();
        let color = parse_hex_color(hex).unwrap_or(palette.primary.fill);
        let outline = palette.text_color;
        let surface = linear_gradient(
            145.0,
            linear_color_stop(color.blend(rgb(0xffffff).opacity(0.18)), 0.0),
            linear_color_stop(color.blend(rgb(0x000000).opacity(0.20)), 1.0),
        );
        let edge = color.blend(outline.opacity(0.24));
        let selection = outline.opacity(0.75);

        let swatch = div()
            .id(if is_default {
                ElementId::Name("color-theme-default".into())
            } else {
                ElementId::Name(format!("color-{hex}").into())
            })
            .size(px(46.0))
            .rounded_full()
            .border_2()
            .border_color(selection.opacity(if is_selected { 1.0 } else { 0.0 }))
            .p(px(1.0))
            .cursor_pointer()
            .hover(move |s| {
                s.border_color(if is_selected {
                    selection
                } else {
                    outline.opacity(0.38)
                })
            })
            .on_click(move |_event, window, cx| {
                state.update(cx, |f, _| {
                    f.accent_color = hex.to_string();
                });
                let _ = config.update_ui_config(|ui| {
                    ui.accent_color = hex.to_string();
                });
                cx.update_global::<ThemeManager, _>(|mgr, _| {
                    mgr.set_accent_hex(hex);
                });
                window.refresh();
            })
            .child(
                div()
                    .size_full()
                    .rounded_full()
                    .bg(surface)
                    .border_1()
                    .border_color(edge),
            );

        if is_default {
            swatch.tooltip(|_, cx| cx.new(|_| ActionTooltip::new("Color del tema")).into())
        } else {
            swatch
        }
    }

    fn navigation_button(
        style: NavigationStyle,
        label: &'static str,
        current: NavigationStyle,
        config: Arc<rdm_config::ConfigManager>,
    ) -> impl IntoElement {
        let style_key = style.key();
        PillButton::new(
            ElementId::Name(format!("btn-style-{style_key}").into()),
            label,
        )
        .selected(current == style)
        .on_click(move |_event, window, cx| {
            if NavigationStyle::from_key(&config.ui_config().sidebar_style) == style {
                return;
            }
            config.update_ui_config_in_memory(|ui| {
                ui.sidebar_style = style_key.to_string();
            });
            let config = config.clone();
            cx.tasks_model()
                .read(cx)
                .tokio_handle()
                .spawn_blocking(move || {
                    if let Err(error) = config.save() {
                        eprintln!("No se pudo guardar el modo de navegación: {error}");
                    }
                });
            window.refresh();
        })
    }

    fn theme_icon_button(
        mode: &'static str,
        icon_path: &'static str,
        current: &str,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
        config: Arc<rdm_config::ConfigManager>,
    ) -> impl IntoElement {
        let is_selected = current == mode;
        let state_click = state.clone();
        IconButton::new(
            ElementId::Name(format!("btn-theme-{mode}").into()),
            icon_path,
        )
        .size(px(36.0))
        .icon_size(px(16.0))
        .rounded(ButtonRounded::Size(px(24.0)))
        .selected(is_selected)
        .color(if is_selected {
            palette.text_color
        } else {
            palette.text_muted
        })
        .hover_bg(palette.surface_hover)
        .on_click(move |_event, window, cx| {
            state_click.update(cx, |f, _| {
                f.theme = mode.to_string();
            });
            let _ = config.update_ui_config(|ui| {
                ui.theme = mode.to_string();
            });
            ThemeManager::apply_mode(mode, cx);
            window.refresh();
        })
    }
}
