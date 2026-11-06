use gpui::prelude::FluentBuilder;
use gpui::*;
use rdm_config::EngineConfig;
use std::path::PathBuf;

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

        let form_save = form.clone();
        let config_save = config_manager.clone();

        let is_subview_open = modal_state.open && modal_state.subview.is_some();

        div()
            .id("settings-modal-root")
            .absolute()
            .inset_0()
            .size_full()
            .child(
                Modal::new("settings-modal")
                    .open(modal_state.open)
                    .title("Ajustes del Sistema")
                    .icon("icons/settings.svg")
                    .on_close(move |window, cx| {
                        cx.set_global(SettingsModalState {
                            open: false,
                            subview: None,
                        });
                        window.refresh();
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .text_xs()
                            .child(
                                div()
                                    .id("btn-appearance-row")
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .p_3()
                                    .rounded(px(16.0))
                                    .bg(palette.surface_hover)
                                    .border_1()
                                    .border_color(palette.border)
                                    .cursor_pointer()
                                    .hover(move |s| s.border_color(palette.accent))
                                    .on_click(move |_event, window, cx| {
                                        cx.set_global(SettingsModalState {
                                            open: true,
                                            subview: Some(SettingsSubView::Appearance),
                                        });
                                        window.refresh();
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .child(
                                                svg()
                                                    .path("icons/palette.svg")
                                                    .size_5()
                                                    .text_color(palette.accent),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_size(px(13.0))
                                                            .text_color(palette.text_color)
                                                            .child("Apariencia y Estilo"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_color(palette.text_muted)
                                                            .child(format!(
                                                                "Tema: {} · Sidebar: {}",
                                                                if form.theme == "light" {
                                                                    "Claro"
                                                                } else {
                                                                    "Oscuro"
                                                                },
                                                                if form.sidebar_style == "minimal" {
                                                                    "Texto Grande"
                                                                } else {
                                                                    "Clásica"
                                                                }
                                                            )),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_3()
                                            .py_1p5()
                                            .rounded(px(24.0))
                                            .bg(palette.border)
                                            .text_color(palette.text_color)
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Personalizar →"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(palette.text_color)
                                            .child("Límite de velocidad global (MB/s)"),
                                    )
                                    .child(
                                        div()
                                            .text_color(palette.text_muted)
                                            .child("Escribe la velocidad que desees o usa 0 para ilimitada."),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(Self::text_input_field(
                                                "input-speed",
                                                &speed_str,
                                                "MB/s",
                                                focused.as_deref() == Some("speed"),
                                                form_state.clone(),
                                                "speed",
                                                palette,
                                            ))
                                            .child(Self::speed_preset_button("0 (Ilim.)", 0, form_state.clone(), palette))
                                            .child(Self::speed_preset_button("10 MB/s", 10, form_state.clone(), palette))
                                            .child(Self::speed_preset_button("50 MB/s", 50, form_state.clone(), palette)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(palette.text_color)
                                            .child("Conexiones simultáneas por descarga"),
                                    )
                                    .child(
                                        div()
                                            .text_color(palette.text_muted)
                                            .child("Segmentos concurrentes (1 a 32)."),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(Self::text_input_field(
                                                "input-connections",
                                                &connections_str,
                                                "conex.",
                                                focused.as_deref() == Some("connections"),
                                                form_state.clone(),
                                                "connections",
                                                palette,
                                            ))
                                            .child(Self::connection_preset_button(4, form_state.clone(), palette))
                                            .child(Self::connection_preset_button(8, form_state.clone(), palette))
                                            .child(Self::connection_preset_button(16, form_state.clone(), palette))
                                            .child(Self::connection_preset_button(32, form_state.clone(), palette)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(palette.text_color)
                                            .child("Carpeta de descargas"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .h_9()
                                                    .px_3()
                                                    .rounded(px(24.0))
                                                    .bg(palette.surface_hover)
                                                    .border_1()
                                                    .border_color(palette.border)
                                                    .flex()
                                                    .items_center()
                                                    .truncate()
                                                    .text_color(palette.text_color)
                                                    .child(download_dir_str),
                                            )
                                            .child({
                                                let form_dir = form_state.clone();
                                                div()
                                                    .id("btn-browse-download-dir")
                                                    .h_9()
                                                    .px_3()
                                                    .rounded(px(24.0))
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
                                                        window.spawn(cx, async move |cx| {
                                                            if let Ok(Ok(Some(paths))) = prompt.await
                                                                && let Some(first) = paths.into_iter().next()
                                                            {
                                                                let path_str = first.to_string_lossy().to_string();
                                                                form_target.update(cx, |f, _| {
                                                                    f.download_dir_str = path_str;
                                                                });
                                                                let _ = cx.update(|window, _| {
                                                                    window.refresh();
                                                                });
                                                            }
                                                        }).detach();
                                                    })
                                                    .child("Cambiar...")
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(palette.text_color)
                                                    .child("User-Agent personalizado"),
                                            )
                                            .child({
                                                let form_reset = form_state.clone();
                                                div()
                                                    .id("btn-reset-user-agent")
                                                    .cursor_pointer()
                                                    .text_color(palette.accent)
                                                    .hover(move |s| s.underline())
                                                    .on_click(move |_event, window, cx| {
                                                        form_reset.update(cx, |f, _| {
                                                            f.user_agent_str = rdm_config::get_user_agent();
                                                        });
                                                        window.refresh();
                                                    })
                                                    .child("Restaurar por defecto")
                                            }),
                                    )
                                    .child(Self::text_input_field(
                                        "input-user-agent",
                                        &user_agent_str,
                                        "Escribe o pega User-Agent...",
                                        focused.as_deref() == Some("user_agent"),
                                        form_state.clone(),
                                        "user_agent",
                                        palette,
                                    )),
                            ),
                    )
                    .footer(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_3()
                            .pt_2()
                            .border_t_1()
                            .border_color(palette.border)
                            .child(
                                div()
                                    .id("btn-cancel-settings")
                                    .h_9()
                                    .px_4()
                                    .rounded(px(24.0))
                                    .bg(palette.surface_hover)
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .text_color(palette.text_color)
                                    .hover(move |s| s.border_1().border_color(palette.border))
                                    .on_click(move |_event, window, cx| {
                                        cx.set_global(SettingsModalState {
                                            open: false,
                                            subview: None,
                                        });
                                        window.refresh();
                                    })
                                    .child("Cancelar"),
                            )
                            .child(
                                div()
                                    .id("btn-save-settings")
                                    .h_9()
                                    .px_5()
                                    .rounded(px(24.0))
                                    .bg(palette.accent)
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xffffff))
                                    .on_click(move |_event, window, cx| {
                                        let speed_mb: f64 = form_save.max_speed_mb_str.parse().unwrap_or(0.0);
                                        let speed_bytes = (speed_mb * 1024.0 * 1024.0) as u64;
                                        let connections_val: usize = form_save.default_connections_str.parse().unwrap_or(8);
                                        let connections = connections_val.clamp(1, 32);
                                        let download_dir = PathBuf::from(&form_save.download_dir_str);
                                        let user_agent = form_save.user_agent_str.clone();

                                        let theme_choice = form_save.theme.clone();
                                        let sidebar_choice = form_save.sidebar_style.clone();
                                        let accent_choice = form_save.accent_color.clone();

                                        let _ = config_save.update_config(|cfg| {
                                            cfg.default_connections = connections;
                                            cfg.max_speed_bytes_per_sec = speed_bytes;
                                            if !download_dir.as_os_str().is_empty() {
                                                cfg.download_dir = download_dir;
                                            }
                                            if !user_agent.trim().is_empty() {
                                                cfg.user_agent = user_agent;
                                            }
                                        });

                                        let _ = config_save.update_ui_config(|ui| {
                                            ui.theme = theme_choice.clone();
                                            ui.sidebar_style = sidebar_choice;
                                            ui.accent_color = accent_choice;
                                        });

                                        if theme_choice == "light" {
                                            let mut tm = ThemeManager::new();
                                            tm.set_theme(Box::new(LightTheme));
                                            cx.set_global(tm);
                                        } else {
                                            let mut tm = ThemeManager::new();
                                            tm.set_theme(Box::new(DarkTheme));
                                            cx.set_global(tm);
                                        }

                                        cx.set_global(SettingsModalState {
                                            open: false,
                                            subview: None,
                                        });
                                        window.refresh();
                                    })
                                    .child("Guardar cambios"),
                            ),
                    ),
            )
            .child(
                ModalSubView::new("settings-subview-appearance")
                    .open(is_subview_open)
                    .title("Apariencia y Estilo")
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
                    )),
            )
    }
}

impl SettingsModal {
    fn text_input_field(
        id_str: &'static str,
        value: &str,
        placeholder: &'static str,
        is_focused: bool,
        state: Entity<SettingsFormState>,
        field_key: &'static str,
        palette: ModalPalette,
    ) -> impl IntoElement {
        let state_key = state.clone();
        let state_click = state.clone();
        let val_disp = if value.is_empty() {
            placeholder.to_string()
        } else {
            value.to_string()
        };

        div()
            .id(ElementId::Name(id_str.into()))
            .flex_1()
            .h_9()
            .px_3()
            .rounded(px(24.0))
            .bg(palette.surface_hover)
            .border_1()
            .border_color(if is_focused {
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
                        "user_agent" => {
                            f.user_agent_str.pop();
                        }
                        _ => {}
                    });
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
                        "user_agent" => {
                            f.user_agent_str.push_str(text.trim());
                        }
                        _ => {}
                    });
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
                        "user_agent" => {
                            f.user_agent_str.push_str(ch);
                        }
                        _ => {}
                    });
                    window.refresh();
                }
            })
            .child(
                div()
                    .truncate()
                    .text_color(if value.is_empty() {
                        palette.text_muted
                    } else {
                        palette.text_color
                    })
                    .child(val_disp),
            )
    }

    fn render_appearance_subview(
        current_theme: &str,
        current_sidebar: &str,
        current_accent: &str,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_5()
            .text_xs()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.text_color)
                            .child("Tema de la interfaz"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Self::theme_button("dark", "Oscuro", current_theme, state.clone(), palette))
                            .child(Self::theme_button("light", "Claro", current_theme, state.clone(), palette)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.text_color)
                            .child("Estilo de Navegación (Sidebar)"),
                    )
                    .child(
                        div()
                            .text_color(palette.text_muted)
                            .child("Selecciona entre navegación compacta con chips o minimalista con texto grande."),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Self::sidebar_style_button("classic", "Clásica", current_sidebar, state.clone(), palette))
                            .child(Self::sidebar_style_button("minimal", "Texto Grande", current_sidebar, state.clone(), palette)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2p5()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.text_color)
                            .child("Color de Acento"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_3()
                            .child(Self::color_circle("#3b82f6", current_accent, state.clone()))
                            .child(Self::color_circle("#2563eb", current_accent, state.clone()))
                            .child(Self::color_circle("#6366f1", current_accent, state.clone()))
                            .child(Self::color_circle("#8b5cf6", current_accent, state.clone()))
                            .child(Self::color_circle("#d946ef", current_accent, state.clone()))
                            .child(Self::color_circle("#ec4899", current_accent, state.clone()))
                            .child(Self::color_circle("#ef4444", current_accent, state.clone()))
                            .child(Self::color_circle("#f97316", current_accent, state.clone()))
                            .child(Self::color_circle("#10b981", current_accent, state.clone()))
                            .child(Self::color_circle("#71717a", current_accent, state)),
                    ),
            )
    }

    fn color_circle(
        hex: &'static str,
        current: &str,
        state: Entity<SettingsFormState>,
    ) -> impl IntoElement {
        let is_selected = current.eq_ignore_ascii_case(hex);
        let parsed_color = match hex {
            "#3b82f6" => rgb(0x3b82f6),
            "#2563eb" => rgb(0x2563eb),
            "#6366f1" => rgb(0x6366f1),
            "#8b5cf6" => rgb(0x8b5cf6),
            "#d946ef" => rgb(0xd946ef),
            "#ec4899" => rgb(0xec4899),
            "#ef4444" => rgb(0xef4444),
            "#f97316" => rgb(0xf97316),
            "#10b981" => rgb(0x10b981),
            _ => rgb(0x71717a),
        };
        let state_click = state.clone();

        div()
            .id(ElementId::Name(format!("color-{hex}").into()))
            .size_8()
            .rounded_full()
            .bg(parsed_color)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .when(is_selected, |s| s.border_2().border_color(rgb(0xffffff)))
            .on_click(move |_event, window, cx| {
                state_click.update(cx, |f, _| {
                    f.accent_color = hex.to_string();
                });
                window.refresh();
            })
    }

    fn sidebar_style_button(
        style_key: &'static str,
        label: &'static str,
        current: &str,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
    ) -> impl IntoElement {
        let is_selected = current == style_key;
        let state_click = state.clone();
        div()
            .id(ElementId::Name(format!("btn-style-{style_key}").into()))
            .flex_1()
            .h_9()
            .rounded(px(24.0))
            .bg(if is_selected {
                palette.accent
            } else {
                palette.surface_hover
            })
            .border_1()
            .border_color(if is_selected {
                palette.accent
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
                palette.text_color
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
                window.refresh();
            })
            .child(label)
    }

    fn speed_preset_button(
        label: &'static str,
        mb: u64,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
    ) -> impl IntoElement {
        let state_click = state.clone();
        div()
            .id(ElementId::Name(format!("btn-speed-{}", mb).into()))
            .h_9()
            .px_3()
            .rounded(px(24.0))
            .bg(palette.surface_hover)
            .border_1()
            .border_color(palette.border)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |s| s.border_color(palette.text_color))
            .on_click(move |_event, window, cx| {
                state_click.update(cx, |f, _| {
                    f.max_speed_mb_str = mb.to_string();
                });
                window.refresh();
            })
            .child(label)
    }

    fn connection_preset_button(
        count: usize,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
    ) -> impl IntoElement {
        let state_click = state.clone();
        div()
            .id(ElementId::Name(format!("btn-conn-{}", count).into()))
            .h_9()
            .px_3()
            .rounded(px(24.0))
            .bg(palette.surface_hover)
            .border_1()
            .border_color(palette.border)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |s| s.border_color(palette.text_color))
            .on_click(move |_event, window, cx| {
                state_click.update(cx, |f, _| {
                    f.default_connections_str = count.to_string();
                });
                window.refresh();
            })
            .child(format!("{count}"))
    }

    fn theme_button(
        mode: &'static str,
        label: &'static str,
        current: &str,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
    ) -> impl IntoElement {
        let is_selected = current == mode;
        let state_click = state.clone();
        div()
            .id(ElementId::Name(format!("btn-theme-{}", mode).into()))
            .flex_1()
            .h_9()
            .rounded(px(24.0))
            .bg(if is_selected {
                palette.accent
            } else {
                palette.surface_hover
            })
            .border_1()
            .border_color(if is_selected {
                palette.accent
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
                palette.text_color
            })
            .font_weight(if is_selected {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .on_click(move |_event, window, cx| {
                state_click.update(cx, |f, _| {
                    f.theme = mode.to_string();
                });
                window.refresh();
            })
            .child(label)
    }
}
