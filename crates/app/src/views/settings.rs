use gpui::*;
use rdm_config::EngineConfig;
use std::{path::PathBuf, sync::Arc};

use crate::{
    components::{Modal, ModalSubView},
    state::CoreContext,
    theme::{ActiveTheme, ThemeManager, dark::DarkTheme, light::LightTheme},
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
    border: Rgba,
    text_color: Rgba,
    text_muted: Rgba,
    accent: Rgba,
}

struct TextInputFieldParams<'a> {
    id_str: &'static str,
    value: &'a str,
    placeholder: &'static str,
    is_focused: bool,
    field_key: &'static str,
}

#[derive(Clone)]
pub struct SettingsFormState {
    pub default_connections_str: String,
    pub max_speed_mb_str: String,
    pub download_dir_str: String,
    pub user_agent_str: String,
    pub theme: String,
    pub sidebar_style: String,
    pub accent_color: String,
    pub focused_field: Option<String>,
}

impl SettingsFormState {
    pub fn from_config(config: &EngineConfig, ui_config: &rdm_config::UIConfig) -> Self {
        let max_speed_mb = if config.max_speed_bytes_per_sec > 0 {
            let mb = config.max_speed_bytes_per_sec as f64 / 1048576.0;
            format!("{mb:.1}")
        } else {
            "0".to_string()
        };

        Self {
            default_connections_str: config.default_connections.to_string(),
            max_speed_mb_str: max_speed_mb,
            download_dir_str: config.download_dir.to_string_lossy().to_string(),
            user_agent_str: config.user_agent.clone(),
            theme: ui_config.theme.clone(),
            sidebar_style: ui_config.sidebar_style.clone(),
            accent_color: ui_config.accent_color.clone(),
            focused_field: None,
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
            SettingsFormState::from_config(&current_cfg, &current_ui_cfg)
        });

        let form = form_state.read(cx).clone();

        let palette = ModalPalette {
            surface_hover: cx.theme().surface_hover(),
            border: cx.theme().border(),
            text_color: cx.theme().text_color(),
            text_muted: cx.theme().text_muted(),
            accent: cx.theme().accent(),
        };

        let speed_str = form.max_speed_mb_str.clone();
        let connections_str = form.default_connections_str.clone();
        let download_dir_str = form.download_dir_str.clone();
        let user_agent_str = form.user_agent_str.clone();
        let current_theme = form.theme.clone();
        let current_sidebar = form.sidebar_style.clone();
        let current_accent = form.accent_color.clone();
        let focused = form.focused_field.clone();

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
                                &current_sidebar,
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
                                        div()
                                            .id("btn-open-appearance-subview")
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size_8()
                                            .rounded_lg()
                                            .cursor_pointer()
                                            .hover(move |s| s.bg(palette.surface_hover))
                                            .on_click(move |_event, window, cx| {
                                                cx.set_global(SettingsModalState {
                                                    open: true,
                                                    subview: Some(SettingsSubView::Appearance),
                                                });
                                                window.refresh();
                                            })
                                            .child(
                                                svg()
                                                    .path("icons/chevron-right.svg")
                                                    .size_5()
                                                    .text_color(palette.text_muted),
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
                                            .gap_2()
                                            .child(
                                                div()
                                                    .w(px(140.0))
                                                    .child(Self::text_input_field(
                                                        TextInputFieldParams {
                                                            id_str: "input-speed",
                                                            value: &speed_str,
                                                            placeholder: "0",
                                                            is_focused: focused.as_deref() == Some("speed"),
                                                            field_key: "speed",
                                                        },
                                                        form_state.clone(),
                                                        palette,
                                                        config_manager.clone(),
                                                    )),
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
                                            .w(px(100.0))
                                            .child(Self::text_input_field(
                                                TextInputFieldParams {
                                                    id_str: "input-connections",
                                                    value: &connections_str,
                                                    placeholder: "8",
                                                    is_focused: focused.as_deref() == Some("connections"),
                                                    field_key: "connections",
                                                },
                                                form_state.clone(),
                                                palette,
                                                config_manager.clone(),
                                            )),
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
                                                    .child(Self::text_input_field(
                                                        TextInputFieldParams {
                                                            id_str: "input-download-dir",
                                                            value: &download_dir_str,
                                                            placeholder: "C:\\Downloads",
                                                            is_focused: focused.as_deref() == Some("download_dir"),
                                                            field_key: "download_dir",
                                                        },
                                                        form_state.clone(),
                                                        palette,
                                                        config_manager.clone(),
                                                    )),
                                            )
                                            .child({
                                                let form_dir = form_state.clone();
                                                let cfg_dir = config_manager.clone();
                                                div()
                                                    .id("btn-browse-download-dir")
                                                    .size_9()
                                                    .rounded_full()
                                                    .bg(palette.surface_hover)
                                                    .border_1()
                                                    .border_color(palette.border)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .cursor_pointer()
                                                    .hover(move |s| s.border_color(palette.text_color))
                                                    .on_click(move |_event, window, cx| {
                                                        let prompt = cx.prompt_for_paths(PathPromptOptions {
                                                            files: false,
                                                            directories: true,
                                                            multiple: false,
                                                            prompt: Some("Selecciona la carpeta de descargas".into()),
                                                        });
                                                        let form_target = form_dir.clone();
                                                        let cfg_target = cfg_dir.clone();
                                                        window.spawn(cx, async move |cx| {
                                                            if let Ok(Ok(Some(paths))) = prompt.await
                                                                && let Some(first) = paths.into_iter().next()
                                                            {
                                                                let path_str = first.to_string_lossy().to_string();
                                                                let path_buf = first.clone();
                                                                form_target.update(cx, |f, _| {
                                                                    f.download_dir_str = path_str;
                                                                });
                                                                let _ = cfg_target.update_config(|c| {
                                                                    c.download_dir = path_buf;
                                                                });
                                                                let _ = cx.update(|window, _| {
                                                                    window.refresh();
                                                                });
                                                            }
                                                        }).detach();
                                                    })
                                                    .child(
                                                        svg()
                                                            .path("icons/folder-open.svg")
                                                            .size_4()
                                                            .text_color(palette.text_muted),
                                                    )
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
                                                let form_reset = form_state.clone();
                                                let cfg_reset = config_manager.clone();
                                                div()
                                                    .id("btn-reset-user-agent")
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(palette.accent)
                                                    .hover(move |s| s.underline())
                                                    .on_click(move |_event, window, cx| {
                                                        let default_ua = rdm_config::get_user_agent();
                                                        form_reset.update(cx, |f, _| {
                                                            f.user_agent_str = default_ua.clone();
                                                        });
                                                        let _ = cfg_reset.update_config(|c| {
                                                            c.user_agent = default_ua;
                                                        });
                                                        window.refresh();
                                                    })
                                                    .child("Restaurar por defecto")
                                            }),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Self::text_input_field(
                                                TextInputFieldParams {
                                                    id_str: "input-user-agent",
                                                    value: &user_agent_str,
                                                    placeholder: "Escribe o pega User-Agent...",
                                                    is_focused: focused.as_deref() == Some("user_agent"),
                                                    field_key: "user_agent",
                                                },
                                                form_state.clone(),
                                                palette,
                                                config_manager.clone(),
                                            )),
                                    ),
                            ),
                    ),
            )
    }
}

impl SettingsModal {
    fn text_input_field(
        params: TextInputFieldParams<'_>,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
        config: Arc<rdm_config::ConfigManager>,
    ) -> impl IntoElement {
        let field_key = params.field_key;
        let state_key = state.clone();
        let state_click = state.clone();
        let val_disp = if params.value.is_empty() {
            params.placeholder.to_string()
        } else {
            params.value.to_string()
        };

        div()
            .id(ElementId::Name(params.id_str.into()))
            .flex_1()
            .h_9()
            .px_3()
            .rounded(px(24.0))
            .bg(palette.surface_hover)
            .border_1()
            .border_color(if params.is_focused {
                palette.accent
            } else {
                palette.border
            })
            .flex()
            .items_center()
            .cursor_text()
            .on_click(move |_event, window, cx| {
                state_click.update(cx, |f, _| {
                    f.focused_field = Some(field_key.to_string());
                });
                window.refresh();
            })
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" || event.keystroke.key == "enter" {
                    state_key.update(cx, |f, _| {
                        f.focused_field = None;
                    });
                    window.refresh();
                } else if event.keystroke.key == "backspace" {
                    state_key.update(cx, |f, _| match field_key {
                        "speed" => {
                            f.max_speed_mb_str.pop();
                        }
                        "connections" => {
                            f.default_connections_str.pop();
                        }
                        "download_dir" => {
                            f.download_dir_str.pop();
                        }
                        "user_agent" => {
                            f.user_agent_str.pop();
                        }
                        _ => {}
                    });
                    Self::save_current_form(&state_key, &config, cx);
                    window.refresh();
                } else if event.keystroke.modifiers.control
                    && event.keystroke.key.eq_ignore_ascii_case("v")
                    && let Some(item) = cx.read_from_clipboard()
                    && let Some(text) = item.text()
                {
                    state_key.update(cx, |f, _| match field_key {
                        "speed" => {
                            f.max_speed_mb_str.push_str(text.trim());
                        }
                        "connections" => {
                            f.default_connections_str.push_str(text.trim());
                        }
                        "download_dir" => {
                            f.download_dir_str.push_str(text.trim());
                        }
                        "user_agent" => {
                            f.user_agent_str.push_str(text.trim());
                        }
                        _ => {}
                    });
                    Self::save_current_form(&state_key, &config, cx);
                    window.refresh();
                } else if !event.keystroke.modifiers.control
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.platform
                    && let Some(ch) = event.keystroke.key_char.as_deref()
                {
                    state_key.update(cx, |f, _| match field_key {
                        "speed" => {
                            if ch.chars().all(|c| c.is_ascii_digit() || c == '.') {
                                f.max_speed_mb_str.push_str(ch);
                            }
                        }
                        "connections" => {
                            if ch.chars().all(|c| c.is_ascii_digit()) {
                                f.default_connections_str.push_str(ch);
                            }
                        }
                        "download_dir" => {
                            f.download_dir_str.push_str(ch);
                        }
                        "user_agent" => {
                            f.user_agent_str.push_str(ch);
                        }
                        _ => {}
                    });
                    Self::save_current_form(&state_key, &config, cx);
                    window.refresh();
                }
            })
            .child(
                div()
                    .truncate()
                    .text_xs()
                    .text_color(if params.value.is_empty() {
                        palette.text_muted
                    } else {
                        palette.text_color
                    })
                    .child(val_disp),
            )
    }

    fn save_current_form(
        state: &Entity<SettingsFormState>,
        config: &Arc<rdm_config::ConfigManager>,
        cx: &mut App,
    ) {
        let f = state.read(cx);
        let speed_mb: f64 = f.max_speed_mb_str.parse().unwrap_or(0.0);
        let speed_bytes = (speed_mb * 1024.0 * 1024.0) as u64;
        let connections_val: usize = f.default_connections_str.parse().unwrap_or(8);
        let connections = connections_val.clamp(1, 32);
        let download_dir = PathBuf::from(&f.download_dir_str);
        let user_agent = f.user_agent_str.clone();

        let _ = config.update_config(|cfg| {
            cfg.default_connections = connections;
            cfg.max_speed_bytes_per_sec = speed_bytes;
            if !download_dir.as_os_str().is_empty() {
                cfg.download_dir = download_dir;
            }
            if !user_agent.trim().is_empty() {
                cfg.user_agent = user_agent;
            }
        });
    }

    fn render_appearance_subview(
        current_theme: &str,
        current_sidebar: &str,
        current_accent: &str,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
        config: Arc<rdm_config::ConfigManager>,
    ) -> impl IntoElement {
        let row1 = ["#9e9e9e", "#1e1e1e", "#ef4444", "#f97316", "#10b981"];
        let row2 = ["#0088ff", "#4422ff", "#9900ff", "#ff0099", "#e07070"];

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
                            .child(Self::pill_button(
                                "classic",
                                "Normal",
                                current_sidebar,
                                state.clone(),
                                palette,
                                config.clone(),
                            ))
                            .child(Self::pill_button(
                                "modern",
                                "Moderna",
                                current_sidebar,
                                state.clone(),
                                palette,
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
        state: Entity<SettingsFormState>,
        config: Arc<rdm_config::ConfigManager>,
    ) -> impl IntoElement {
        let is_selected = current.eq_ignore_ascii_case(hex);
        let (base_color, darker_border) = match hex {
            "#9e9e9e" => (rgb(0x9e9e9e), rgb(0x737373)),
            "#1e1e1e" => (rgb(0x1e1e1e), rgb(0x0a0a0a)),
            "#ef4444" => (rgb(0xef4444), rgb(0xb91c1c)),
            "#f97316" => (rgb(0xf97316), rgb(0xc2410c)),
            "#10b981" => (rgb(0x10b981), rgb(0x047857)),
            "#0088ff" => (rgb(0x0088ff), rgb(0x0055cc)),
            "#4422ff" => (rgb(0x4422ff), rgb(0x280dbb)),
            "#9900ff" => (rgb(0x9900ff), rgb(0x6b00b3)),
            "#ff0099" => (rgb(0xff0099), rgb(0xb8006e)),
            "#e07070" => (rgb(0xe07070), rgb(0xb84848)),
            _ => (rgb(0x0088ff), rgb(0x0055cc)),
        };
        let state_click = state.clone();

        div()
            .id(ElementId::Name(format!("color-{hex}").into()))
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
            )
    }

    fn pill_button(
        style_key: &'static str,
        label: &'static str,
        current: &str,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
        config: Arc<rdm_config::ConfigManager>,
    ) -> impl IntoElement {
        let is_selected = current == style_key;
        let state_click = state.clone();
        div()
            .id(ElementId::Name(format!("btn-style-{style_key}").into()))
            .px_5()
            .h_9()
            .rounded(px(24.0))
            .bg(if is_selected {
                palette.surface_hover
            } else {
                palette.border
            })
            .border_1()
            .border_color(if is_selected {
                palette.text_color
            } else {
                palette.border
            })
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(if is_selected {
                rgb(0xffffff)
            } else {
                palette.text_muted
            })
            .font_weight(if is_selected {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .on_click(move |_event, window, cx| {
                state_click.update(cx, |f, _| {
                    f.sidebar_style = style_key.to_string();
                });
                let _ = config.update_ui_config(|ui| {
                    ui.sidebar_style = style_key.to_string();
                });
                window.refresh();
            })
            .child(div().text_xs().child(label))
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
        div()
            .id(ElementId::Name(format!("btn-theme-{mode}").into()))
            .w(px(58.0))
            .h_9()
            .rounded(px(24.0))
            .bg(if is_selected {
                palette.surface_hover
            } else {
                palette.border
            })
            .border_1()
            .border_color(if is_selected {
                palette.text_color
            } else {
                palette.border
            })
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_click(move |_event, window, cx| {
                state_click.update(cx, |f, _| {
                    f.theme = mode.to_string();
                });
                let _ = config.update_ui_config(|ui| {
                    ui.theme = mode.to_string();
                });
                if mode == "light" {
                    let mut tm = ThemeManager::new();
                    tm.set_theme(Box::new(LightTheme));
                    cx.set_global(tm);
                } else {
                    let mut tm = ThemeManager::new();
                    tm.set_theme(Box::new(DarkTheme));
                    cx.set_global(tm);
                }
                window.refresh();
            })
            .child(svg().path(icon_path).size_4().text_color(if is_selected {
                rgb(0xffffff)
            } else {
                palette.text_muted
            }))
    }
}
