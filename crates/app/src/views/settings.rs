use gpui::*;
use rdm_config::EngineConfig;
use std::path::PathBuf;

use crate::{
    components::Modal,
    state::CoreContext,
    theme::{ActiveTheme, ThemeManager, dark::DarkTheme, light::LightTheme},
};

#[derive(Default, Clone, Copy)]
pub struct SettingsModalState {
    pub open: bool,
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
    pub default_connections: usize,
    pub max_speed_mb_str: String,
    pub download_dir_str: String,
    pub user_agent_str: String,
    pub max_retries: usize,
    pub theme: String,
}

impl Default for SettingsFormState {
    fn default() -> Self {
        Self {
            default_connections: 8,
            max_speed_mb_str: "0".to_string(),
            download_dir_str: String::new(),
            user_agent_str: String::new(),
            max_retries: 8,
            theme: "dark".to_string(),
        }
    }
}

impl SettingsFormState {
    pub fn from_config(config: &EngineConfig) -> Self {
        let max_speed_mb = if config.max_speed_bytes_per_sec > 0 {
            let mb = config.max_speed_bytes_per_sec as f64 / 1048576.0;
            format!("{mb:.1}")
        } else {
            "0".to_string()
        };

        Self {
            default_connections: config.default_connections,
            max_speed_mb_str: max_speed_mb,
            download_dir_str: config.download_dir.to_string_lossy().to_string(),
            user_agent_str: config.user_agent.clone(),
            max_retries: config.max_retries,
            theme: config.theme.clone(),
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
        let is_open = cx
            .try_global::<SettingsModalState>()
            .map(|s| s.open)
            .unwrap_or(false);

        let core = cx.core().clone();
        let config_manager = core.config().clone();
        let current_cfg = config_manager.config();

        let form_state = window.use_keyed_state("settings-form-state", cx, |_, _| {
            SettingsFormState::from_config(&current_cfg)
        });

        let form = form_state.read(cx).clone();

        let palette = ModalPalette {
            surface_hover: cx.theme().surface_hover(),
            border: cx.theme().border(),
            text_color: cx.theme().text_color(),
            text_muted: cx.theme().text_muted(),
            accent: cx.theme().accent(),
        };

        let connections = form.default_connections;
        let speed_str = form.max_speed_mb_str.clone();
        let download_dir_str = form.download_dir_str.clone();
        let user_agent_str = form.user_agent_str.clone();
        let current_theme = form.theme.clone();

        let form_save = form.clone();
        let config_save = config_manager.clone();

        Modal::new("settings-modal")
            .open(is_open)
            .title("Ajustes del Sistema")
            .icon("icons/settings.svg")
            .on_close(move |window, cx| {
                cx.set_global(SettingsModalState { open: false });
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
                                    .child("Usa 0 para velocidad ilimitada."),
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
                                            .child(div().text_color(palette.text_color).child(
                                                if speed_str.is_empty() {
                                                    "0 (Ilimitado)".to_string()
                                                } else {
                                                    format!("{speed_str} MB/s")
                                                },
                                            )),
                                    )
                                    .child(Self::speed_preset_button(
                                        "0 MB/s",
                                        0,
                                        form_state.clone(),
                                        palette,
                                    ))
                                    .child(Self::speed_preset_button(
                                        "10 MB/s",
                                        10,
                                        form_state.clone(),
                                        palette,
                                    ))
                                    .child(Self::speed_preset_button(
                                        "50 MB/s",
                                        50,
                                        form_state.clone(),
                                        palette,
                                    )),
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
                                    .child("Cantidad de segmentos concurrentes (1 a 32)."),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(Self::connection_preset_button(
                                        4,
                                        connections,
                                        form_state.clone(),
                                        palette,
                                    ))
                                    .child(Self::connection_preset_button(
                                        8,
                                        connections,
                                        form_state.clone(),
                                        palette,
                                    ))
                                    .child(Self::connection_preset_button(
                                        16,
                                        connections,
                                        form_state.clone(),
                                        palette,
                                    ))
                                    .child(Self::connection_preset_button(
                                        32,
                                        connections,
                                        form_state.clone(),
                                        palette,
                                    )),
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
                                                let prompt =
                                                    cx.prompt_for_paths(PathPromptOptions {
                                                        files: false,
                                                        directories: true,
                                                        multiple: false,
                                                        prompt: Some(
                                                            "Selecciona la carpeta de descargas"
                                                                .into(),
                                                        ),
                                                    });
                                                let form_target = form_dir.clone();
                                                window
                                                    .spawn(cx, async move |cx| {
                                                        if let Ok(Ok(Some(paths))) = prompt.await
                                                            && let Some(first) =
                                                                paths.into_iter().next()
                                                        {
                                                            let path_str =
                                                                first.to_string_lossy().to_string();
                                                            form_target.update(cx, |f, _| {
                                                                f.download_dir_str = path_str;
                                                            });
                                                            let _ = cx.update(|window, _| {
                                                                window.refresh();
                                                            });
                                                        }
                                                    })
                                                    .detach();
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
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(palette.text_color)
                                    .child("Tema de la interfaz"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(Self::theme_button(
                                        "dark",
                                        "Oscuro",
                                        &current_theme,
                                        form_state.clone(),
                                        palette,
                                    ))
                                    .child(Self::theme_button(
                                        "light",
                                        "Claro",
                                        &current_theme,
                                        form_state.clone(),
                                        palette,
                                    )),
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
                                    .child("User-Agent personalizado"),
                            )
                            .child(
                                div()
                                    .h_9()
                                    .px_3()
                                    .rounded(px(24.0))
                                    .bg(palette.surface_hover)
                                    .border_1()
                                    .border_color(palette.border)
                                    .flex()
                                    .items_center()
                                    .truncate()
                                    .text_color(palette.text_muted)
                                    .child(user_agent_str),
                            ),
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
                                cx.set_global(SettingsModalState { open: false });
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
                                let speed_mb: f64 =
                                    form_save.max_speed_mb_str.parse().unwrap_or(0.0);
                                let speed_bytes = (speed_mb * 1024.0 * 1024.0) as u64;
                                let connections = form_save.default_connections.clamp(1, 32);
                                let download_dir = PathBuf::from(&form_save.download_dir_str);
                                let theme_choice = form_save.theme.clone();

                                let _ = config_save.update_config(|cfg| {
                                    cfg.default_connections = connections;
                                    cfg.max_speed_bytes_per_sec = speed_bytes;
                                    if !download_dir.as_os_str().is_empty() {
                                        cfg.download_dir = download_dir;
                                    }
                                    cfg.theme = theme_choice.clone();
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

                                cx.set_global(SettingsModalState { open: false });
                                window.refresh();
                            })
                            .child("Guardar cambios"),
                    ),
            )
    }
}

impl SettingsModal {
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
        current: usize,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
    ) -> impl IntoElement {
        let is_selected = count == current;
        let state_click = state.clone();
        div()
            .id(ElementId::Name(format!("btn-conn-{}", count).into()))
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
                    f.default_connections = count;
                });
                window.refresh();
            })
            .child(format!("{count} conex."))
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
