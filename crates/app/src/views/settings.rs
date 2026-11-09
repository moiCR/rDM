use gpui::*;
use gpui_kit::component::button::ButtonRounded;
use gpui_kit::component::input::{Input, InputState};
use std::{path::PathBuf, sync::Arc};

use crate::{
    components::{ActionTooltip, IconButton, Modal, ModalSubView, PillButton},
    layout::NavigationStyle,
    state::CoreContext,
    theme::{ActiveTheme, PrimaryColors, ThemeManager, dark::DarkTheme, light::LightTheme},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSubView {
    Appearance,
}

#[derive(Default, Clone, Copy)]
pub struct SettingsModalState {
    pub open: bool,
    pub subview: Option<SettingsSubView>,
}

impl Global for SettingsModalState {}

#[derive(Clone, Copy)]
struct ModalPalette {
    surface_hover: Rgba,
    _border: Rgba,
    text_color: Rgba,
    text_muted: Rgba,
    accent: Rgba,
    primary: PrimaryColors,
}

pub struct SettingsInputs {
    pub speed: Entity<InputState>,
    pub connections: Entity<InputState>,
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
                download_dir,
                user_agent,
                _subs: subs,
            }
        });

        let palette = ModalPalette {
            surface_hover: cx.theme().surface_hover(),
            _border: cx.theme().border(),
            text_color: cx.theme().text_color(),
            text_muted: cx.theme().text_muted(),
            accent: cx.theme().accent(),
            primary: cx.theme().primary(),
        };

        let current_theme = form.theme.clone();
        let navigation_style = NavigationStyle::from_key(&current_ui_cfg.sidebar_style);
        let current_accent = form.accent_color.clone();

        let inputs = inputs_state.read(cx);

        let is_subview_open = modal_state.open && modal_state.subview.is_some();

        div()
            .id("settings-modal-root")
            .absolute()
            .inset_0()
            .size_full()
            .child(
                Modal::new("settings-modal")
                    .open(modal_state.open)
                    .width(px(900.0))
                    .max_height(px(640.0))
                    .title("Configuración")
                    .on_close(move |window, cx| {
                        cx.set_global(SettingsModalState {
                            open: false,
                            subview: None,
                        });
                        window.refresh();
                    })
                    .subview(
                        ModalSubView::new("settings-subview-appearance")
                            .open(is_subview_open)
                            .width(px(380.0))
                            .max_height(px(350.0))
                            .title("Apariencia")
                            .icon("icons/palette.svg")
                            .on_back(move |window, cx| {
                                cx.set_global(SettingsModalState {
                                    open: true,
                                    subview: None,
                                });
                                window.refresh();
                            })
                            .child(Self::render_appearance_subview(
                                &current_theme,
                                navigation_style,
                                &current_accent,
                                form_state.clone(),
                                palette,
                                config_manager.clone(),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(
                                div()
                                    .id("row-appearance")
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .py_2()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(15.0))
                                                    .text_color(palette.text_color)
                                                    .child("Apariencia y Estilo"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(palette.text_muted)
                                                    .child("Cambie el tema, el estilo del panel de navegacion y más"),
                                            ),
                                    )
                                    .child(
                                        IconButton::new(
                                            "btn-open-appearance-subview",
                                            "icons/chevron-right.svg",
                                        )
                                        .size(px(32.0))
                                        .icon_size(px(20.0))
                                        .color(palette.text_muted)
                                        .hover_bg(palette.surface_hover)
                                        .on_click(move |_event, window, cx| {
                                            cx.set_global(SettingsModalState {
                                                open: true,
                                                subview: Some(SettingsSubView::Appearance),
                                            });
                                            window.refresh();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_6()
                                    .py_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap_1p5()
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
                                            .gap_1p5()
                                            .child(
                                                IconButton::new("btn-speed-dec", "icons/minus.svg")
                                                    .size(px(32.0))
                                                    .icon_size(px(14.0))
                                                    .color(palette.text_muted)
                                                    .hover_bg(palette.surface_hover)
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
                                                                state.set_value(
                                                                    format!("{next:.1}"),
                                                                    window,
                                                                    cx,
                                                                );
                                                            });
                                                            window.refresh();
                                                        }
                                                    }),
                                            )
                                            .child(
                                                div().w(px(80.0)).child(
                                                    Input::new(&inputs.speed)
                                                        .id("input-speed-limit")
                                                        .text_align(TextAlign::Center)
                                                        .h(px(36.0))
                                                        .w_full(),
                                                ),
                                            )
                                            .child(
                                                IconButton::new("btn-speed-inc", "icons/plus.svg")
                                                    .size(px(32.0))
                                                    .icon_size(px(14.0))
                                                    .color(palette.text_muted)
                                                    .hover_bg(palette.surface_hover)
                                                    .on_click({
                                                        let speed = inputs.speed.clone();
                                                        move |_, window, cx| {
                                                            speed.update(cx, |state, cx| {
                                                                let current = state
                                                                    .value()
                                                                    .trim()
                                                                    .parse::<f64>()
                                                                    .unwrap_or(0.0);
                                                                let next =
                                                                    (current + 1.0).min(1000.0);
                                                                state.set_value(
                                                                    format!("{next:.1}"),
                                                                    window,
                                                                    cx,
                                                                );
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
                                    .gap_6()
                                    .py_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap_1p5()
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
                                            .gap_1p5()
                                            .child(
                                                IconButton::new("btn-conn-dec", "icons/minus.svg")
                                                    .size(px(32.0))
                                                    .icon_size(px(14.0))
                                                    .color(palette.text_muted)
                                                    .hover_bg(palette.surface_hover)
                                                    .on_click({
                                                        let conn = inputs.connections.clone();
                                                        move |_, window, cx| {
                                                            conn.update(cx, |state, cx| {
                                                                let current = state
                                                                    .value()
                                                                    .trim()
                                                                    .parse::<usize>()
                                                                    .unwrap_or(8);
                                                                let next = current
                                                                    .saturating_sub(1)
                                                                    .clamp(1, 32);
                                                                state.set_value(
                                                                    next.to_string(),
                                                                    window,
                                                                    cx,
                                                                );
                                                            });
                                                            window.refresh();
                                                        }
                                                    }),
                                            )
                                            .child(
                                                div().w(px(64.0)).child(
                                                    Input::new(&inputs.connections)
                                                        .id("input-connections")
                                                        .text_align(TextAlign::Center)
                                                        .h(px(36.0))
                                                        .w_full(),
                                                ),
                                            )
                                            .child(
                                                IconButton::new("btn-conn-inc", "icons/plus.svg")
                                                    .size(px(32.0))
                                                    .icon_size(px(14.0))
                                                    .color(palette.text_muted)
                                                    .hover_bg(palette.surface_hover)
                                                    .on_click({
                                                        let conn = inputs.connections.clone();
                                                        move |_, window, cx| {
                                                            conn.update(cx, |state, cx| {
                                                                let current = state
                                                                    .value()
                                                                    .trim()
                                                                    .parse::<usize>()
                                                                    .unwrap_or(8);
                                                                let next =
                                                                    (current + 1).clamp(1, 32);
                                                                state.set_value(
                                                                    next.to_string(),
                                                                    window,
                                                                    cx,
                                                                );
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
                                    .gap_6()
                                    .py_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_size(px(14.0))
                                                    .text_color(palette.text_color)
                                                    .child("Carpeta de descargas"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(palette.text_muted)
                                                    .child("Directorio predeterminado donde se guardarán los archivos."),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .w(px(280.0))
                                                    .on_mouse_down(MouseButton::Left, {
                                                        let dir = inputs.download_dir.clone();
                                                        move |_, window, cx| {
                                                            dir.update(cx, |s, cx| {
                                                                s.focus(window, cx)
                                                            });
                                                        }
                                                    })
                                                    .child(
                                                        Input::new(&inputs.download_dir)
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
                                                    .rounded(ButtonRounded::Size(px(99.0)))
                                                    .hover_bg(palette.surface_hover)
                                                    .color(palette.text_muted)
                                                    .on_click(move |_event, window, cx| {
                                                        let prompt = cx.prompt_for_paths(PathPromptOptions {
                                                            files: false,
                                                            directories: true,
                                                            multiple: false,
                                                            prompt: Some("Selecciona la carpeta de descargas".into()),
                                                        });
                                                        let dir_target = dir_entity.clone();
                                                        let cfg_target = cfg_dir.clone();
                                                        window.spawn(cx, async move |cx| {
                                                            if let Ok(Ok(Some(paths))) = prompt.await
                                                                && let Some(first) = paths.into_iter().next()
                                                            {
                                                                let path_str = first.to_string_lossy().to_string();
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
                                                        }).detach();
                                                    })
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_6()
                                    .py_2()
                                    .child(
                                        div()
                                            .w(px(260.0))
                                            .flex()
                                            .flex_col()
                                            .gap_1p5()
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
                                                Input::new(&inputs.user_agent)
                                                    .id("input-user-agent")
                                                    .cleanable(true)
                                                    .h(px(36.0))
                                                    .w_full(),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

impl SettingsModal {
    fn render_appearance_subview(
        current_theme: &str,
        navigation_style: NavigationStyle,
        current_accent: &str,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
        config: Arc<rdm_config::ConfigManager>,
    ) -> impl IntoElement {
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
                            .items_center()
                            .gap_2p5()
                            .children(row1.iter().map(|&hex| {
                                Self::figma_color_squircle(
                                    hex,
                                    current_accent,
                                    palette.primary,
                                    state.clone(),
                                    config.clone(),
                                )
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2p5()
                            .children(row2.iter().map(|&hex| {
                                Self::figma_color_squircle(
                                    hex,
                                    current_accent,
                                    palette.primary,
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

    fn figma_color_squircle(
        hex: &'static str,
        current: &str,
        primary: PrimaryColors,
        state: Entity<SettingsFormState>,
        config: Arc<rdm_config::ConfigManager>,
    ) -> impl IntoElement {
        let is_selected = current.eq_ignore_ascii_case(hex);
        let is_default = hex.is_empty();
        let (base_color, darker_border) = match hex {
            "#9e9e9e" => (rgb(0x9e9e9e), rgb(0x737373)),
            "#1e1e1e" => (rgb(0x1e1e1e), rgb(0x0a0a0a)),
            "#ef4444" => (rgb(0xef4444), rgb(0xb91c1c)),
            "#f97316" => (rgb(0xf97316), rgb(0xc2410c)),
            "#10b981" => (rgb(0x10b981), rgb(0x047857)),
            "#4422ff" => (rgb(0x4422ff), rgb(0x280dbb)),
            "#9900ff" => (rgb(0x9900ff), rgb(0x6b00b3)),
            "#ff0099" => (rgb(0xff0099), rgb(0xb8006e)),
            "#e07070" => (rgb(0xe07070), rgb(0xb84848)),
            _ => (primary.fill, primary.border),
        };
        let state_click = state.clone();

        let swatch = div()
            .id(if is_default {
                ElementId::Name("color-theme-default".into())
            } else {
                ElementId::Name(format!("color-{hex}").into())
            })
            .size(px(46.0))
            .relative()
            .overflow_hidden()
            .rounded(if is_selected { px(16.0) } else { px(99.0) })
            .bg(base_color)
            .border_2()
            .border_color(darker_border)
            .cursor_pointer()
            .hover(move |s| s.opacity(0.9))
            .on_click(move |_event, window, cx| {
                state_click.update(cx, |f, _| {
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
                    .absolute()
                    .top(px(2.0))
                    .left(px(5.0))
                    .right(px(5.0))
                    .h(px(16.0))
                    .rounded_full()
                    .bg(rgba(0xffffff45)),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(12.0))
                    .rounded_b(if is_selected { px(14.0) } else { px(99.0) })
                    .bg(rgba(0x00000028)),
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
            cx.update_global::<ThemeManager, _>(|tm, _| {
                if mode == "light" {
                    tm.set_theme(Box::new(LightTheme));
                } else {
                    tm.set_theme(Box::new(DarkTheme));
                }
            });
            if mode == "light" {
                gpui_kit::component::Theme::change(gpui_kit::component::ThemeMode::Light, None, cx);
            } else {
                gpui_kit::component::Theme::change(gpui_kit::component::ThemeMode::Dark, None, cx);
            }
            window.refresh();
        })
    }
}
