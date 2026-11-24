#[cfg(target_os = "linux")]
use crate::linux_desktop as desktop;
#[cfg(windows)]
use crate::windows_desktop as desktop;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::{
    base::{Disableable, Scrollbar, ScrollbarMode},
    component::switch::Switch,
};
use std::{path::PathBuf, sync::Arc};

use crate::{
    components::{
        ActionTooltip, Button, ButtonVariants, IconButton, Input, InputState, Modal, ProgressBar,
    },
    lang::{ActiveLang, Lang},
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

#[cfg(any(windows, target_os = "linux"))]
struct DesktopPreferences(Entity<DesktopPreferencesState>);

#[cfg(any(windows, target_os = "linux"))]
impl Global for DesktopPreferences {}

#[cfg(any(windows, target_os = "linux"))]
struct DesktopPreferencesState {
    startup: Preference,
    close_to_tray: Preference,
}

#[cfg(any(windows, target_os = "linux"))]
struct Preference {
    value: Option<bool>,
    pending: bool,
    error: Option<String>,
}

#[cfg(any(windows, target_os = "linux"))]
impl Preference {
    fn new(value: Option<bool>) -> Self {
        Self {
            value,
            pending: false,
            error: None,
        }
    }
}

#[cfg(any(windows, target_os = "linux"))]
#[derive(Clone, Copy)]
enum DesktopPreference {
    Startup,
    CloseToTray,
}

#[cfg(any(windows, target_os = "linux"))]
impl DesktopPreference {
    fn select(self, state: &mut DesktopPreferencesState) -> &mut Preference {
        match self {
            Self::Startup => &mut state.startup,
            Self::CloseToTray => &mut state.close_to_tray,
        }
    }

    fn change(self, cx: &mut App) {
        let preferences = cx.global::<DesktopPreferences>().0.clone();
        let Some(previous) = preferences.update(cx, |state, cx| {
            let preference = self.select(state);
            if preference.pending {
                return None;
            }
            preference.pending = true;
            preference.error = None;
            cx.refresh_windows();
            Some(preference.value)
        }) else {
            return;
        };
        let config = cx.core().config().clone();
        let work = cx
            .tasks_model()
            .read(cx)
            .tokio_handle()
            .spawn_blocking(move || match self {
                Self::Startup => match previous {
                    Some(value) => desktop::set_startup_registered(!value)
                        .map(|()| !value)
                        .map_err(|error| error.to_string()),
                    None => desktop::startup_registered().map_err(|error| error.to_string()),
                },
                Self::CloseToTray => {
                    let current = previous.unwrap_or(true);
                    if let Err(error) = config.update_ui_config(|ui| ui.close_to_tray = !current) {
                        config.update_ui_config_in_memory(|ui| ui.close_to_tray = current);
                        return Err(error.to_string());
                    }
                    Ok(!current)
                }
            });
        cx.spawn(async move |cx| {
            let result = work
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result);
            preferences.update(cx, |state, cx| {
                let preference = self.select(state);
                preference.pending = false;
                match result {
                    Ok(value) => preference.value = Some(value),
                    Err(error) => preference.error = Some(error),
                }
                cx.refresh_windows();
            });
        })
        .detach();
    }
}

#[derive(Clone, Copy)]
struct StepperConfig {
    dec_id: &'static str,
    input_id: &'static str,
    inc_id: &'static str,
    input_width: Pixels,
    suffix: Option<&'static str>,
}

#[derive(Clone, Copy)]
struct ModalPalette {
    surface: Rgba,
    card_surface: Rgba,
    surface_hover: Rgba,
    border: Rgba,
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
    pub language_pending: bool,
    pub language_error: Option<String>,
    pub theme: String,
    pub accent_color: String,
}

impl SettingsFormState {
    pub fn from_ui_config(ui_config: &rdm_config::UIConfig) -> Self {
        Self {
            language_pending: false,
            language_error: None,
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

    #[cfg(any(windows, target_os = "linux"))]
    pub(crate) fn initialize_desktop_preferences(close_to_tray: bool, cx: &mut App) {
        let preferences = cx.new(|_| DesktopPreferencesState {
            startup: Preference::new(None),
            close_to_tray: Preference::new(Some(close_to_tray)),
        });
        cx.set_global(DesktopPreferences(preferences));
        DesktopPreference::Startup.change(cx);
    }

    #[cfg(any(windows, target_os = "linux"))]
    pub(crate) fn close_to_tray(cx: &App) -> bool {
        cx.global::<DesktopPreferences>()
            .0
            .read(cx)
            .close_to_tray
            .value
            .unwrap_or(true)
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
                let mut s = InputState::new(window, cx)
                    .placeholder(cx.lang().get("settings.directory_placeholder"));
                s.set_value(
                    current_cfg.download_dir.to_string_lossy().to_string(),
                    window,
                    cx,
                );
                s
            });

            let user_agent = cx.new(|cx| {
                let mut s = InputState::new(window, cx)
                    .placeholder(cx.lang().get("settings.user_agent_placeholder"));
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

            let user_agent_placeholder = user_agent.clone();
            let directory_placeholder = download_dir.clone();
            subs.push(window.observe_global::<Lang>(cx, move |window, cx| {
                let user_agent = cx.lang().get("settings.user_agent_placeholder");
                let directory = cx.lang().get("settings.directory_placeholder");
                user_agent_placeholder.update(cx, |input, cx| {
                    input.set_placeholder(user_agent, window, cx)
                });
                directory_placeholder
                    .update(cx, |input, cx| input.set_placeholder(directory, window, cx));
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

        let theme = cx.theme();
        let palette = ModalPalette {
            surface: theme.surface(),
            card_surface: theme
                .input_background()
                .unwrap_or_else(|| theme.background()),
            surface_hover: theme.surface_hover(),
            border: theme.border(),
            text_color: theme.text_color(),
            text_muted: theme.text_muted(),
            accent: theme.accent(),
            primary: theme.primary(),
        };
        let viewport = window.viewport_size();
        let is_open = modal_state.open;
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
                cx,
            ),
            SettingsTab::Advanced => Self::render_advanced(
                inputs,
                palette,
                &config_manager,
                current_ui_cfg.auto_check_updates,
                cx,
            ),
        };

        div()
            .id("settings-modal-root")
            .absolute()
            .inset_0()
            .size_full()
            .capture_key_down(move |event: &KeyDownEvent, window, cx| {
                if is_open && event.keystroke.key == "escape" {
                    Self::close(window, cx);
                    cx.stop_propagation();
                }
            })
            .child(
                Modal::new("settings-modal")
                    .open(is_open)
                    .compact()
                    .width((viewport.width - px(32.0)).clamp(px(0.0), px(580.0)))
                    .max_height((viewport.height - px(32.0)).clamp(px(0.0), px(460.0)))
                    .header(Self::render_header(modal_state.tab, palette, cx))
                    .on_close(Self::close)
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .pt_1()
                            .child(
                                div()
                                    .id(("settings-content", tab_index))
                                    .size_full()
                                    .overflow_y_scroll()
                                    .track_scroll(&scroll_handle)
                                    .pr_2()
                                    .child(content),
                            )
                            .child(Scrollbar::vertical(&scroll_handle).mode(ScrollbarMode::Always)),
                    ),
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
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .p_1()
                    .rounded(px(24.0))
                    .bg(palette.card_surface)
                    .border_1()
                    .border_color(palette.border)
                    .children(
                        [
                            (SettingsTab::Downloads, "settings.downloads"),
                            (SettingsTab::Appearance, "settings.appearance"),
                            (SettingsTab::Advanced, "settings.advanced"),
                        ]
                        .map(|(section, label)| {
                            let selected = section == tab;
                            div()
                                .id(("settings-tab", section as usize))
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(28.0))
                                .px_3()
                                .rounded(px(20.0))
                                .text_xs()
                                .font_weight(if selected {
                                    FontWeight::MEDIUM
                                } else {
                                    FontWeight::NORMAL
                                })
                                .text_color(if selected {
                                    palette.text_color
                                } else {
                                    palette.text_muted
                                })
                                .when(selected, |el| el.bg(palette.surface_hover))
                                .cursor_pointer()
                                .when(!selected, |el| {
                                    el.hover(move |s| s.text_color(palette.text_color))
                                })
                                .on_click(move |_, window, cx| {
                                    if !selected {
                                        cx.set_global(SettingsModalState {
                                            open: true,
                                            tab: section,
                                        });
                                        window.refresh();
                                    }
                                })
                                .child(cx.lang().get(label))
                        }),
                    ),
            )
            .child(
                div().flex_none().child(
                    IconButton::new("btn-close-settings", "icons/x.svg")
                        .size(px(32.0))
                        .rounded(px(24.0))
                        .color(palette.text_muted)
                        .hover_bg(palette.surface_hover)
                        .tooltip(cx.lang().get("common.close"))
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

    fn card(palette: ModalPalette) -> Div {
        div()
            .flex()
            .flex_col()
            .rounded(px(24.0))
            .bg(palette.card_surface)
            .border_1()
            .border_color(palette.border)
            .px_4()
            .py_1()
    }

    fn divider(palette: ModalPalette) -> Div {
        div().h(px(1.0)).w_full().bg(palette.border).opacity(0.6)
    }

    fn row_label(title: SharedString, hint: SharedString, palette: ModalPalette) -> Div {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.text_color)
                    .child(title),
            )
            .child(div().text_xs().text_color(palette.text_muted).child(hint))
    }

    fn stepper(
        config: StepperConfig,
        state: &Entity<InputState>,
        palette: ModalPalette,
        on_dec: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_inc: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .flex_none()
            .gap_1()
            .h(px(32.0))
            .px_1()
            .rounded(px(24.0))
            .bg(palette.surface)
            .border_1()
            .border_color(palette.border)
            .child(
                IconButton::new(config.dec_id, "icons/minus.svg")
                    .size(px(24.0))
                    .icon_size(px(12.0))
                    .rounded(px(20.0))
                    .color(palette.text_muted)
                    .hover_bg(palette.surface_hover)
                    .on_click(on_dec),
            )
            .child(
                div().w(config.input_width).child(
                    Input::new(state)
                        .id(config.input_id)
                        .appearance(false)
                        .text_size(px(13.0))
                        .text_color(palette.text_color)
                        .text_align(TextAlign::Center)
                        .h(px(24.0))
                        .w_full(),
                ),
            )
            .children(config.suffix.map(|unit| {
                div()
                    .text_xs()
                    .text_color(palette.text_muted)
                    .pr_1()
                    .child(unit)
            }))
            .child(
                IconButton::new(config.inc_id, "icons/plus.svg")
                    .size(px(24.0))
                    .icon_size(px(12.0))
                    .rounded(px(20.0))
                    .color(palette.text_muted)
                    .hover_bg(palette.surface_hover)
                    .on_click(on_inc),
            )
    }

    fn segmented_track(palette: ModalPalette) -> Div {
        div()
            .flex()
            .items_center()
            .gap_0p5()
            .p_0p5()
            .rounded(px(24.0))
            .bg(palette.surface)
            .border_1()
            .border_color(palette.border)
    }

    fn action_button(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        primary: bool,
    ) -> Button {
        let button = Button::new(id).label(label).rounded(px(24.0)).h(px(30.0));
        if primary { button.primary() } else { button }
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
            .gap_3()
            .child(
                Self::card(palette)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .py_2p5()
                            .child(Self::row_label(
                                cx.lang().get("settings.speed_limit"),
                                cx.lang().get("settings.speed_limit_hint"),
                                palette,
                            ))
                            .child(Self::stepper(
                                StepperConfig {
                                    dec_id: "btn-speed-dec",
                                    input_id: "input-speed-limit",
                                    inc_id: "btn-speed-inc",
                                    input_width: px(44.0),
                                    suffix: Some("MB/s"),
                                },
                                &inputs.speed,
                                palette,
                                {
                                    let speed = inputs.speed.clone();
                                    move |_, window, cx| {
                                        speed.update(cx, |state, cx| {
                                            let current =
                                                state.value().trim().parse::<f64>().unwrap_or(0.0);
                                            let next = (current - 1.0).max(0.0);
                                            state.set_value(format!("{next:.1}"), window, cx);
                                        });
                                        window.refresh();
                                    }
                                },
                                {
                                    let speed = inputs.speed.clone();
                                    move |_, window, cx| {
                                        speed.update(cx, |state, cx| {
                                            let current =
                                                state.value().trim().parse::<f64>().unwrap_or(0.0);
                                            let next = (current + 1.0).min(1000.0);
                                            state.set_value(format!("{next:.1}"), window, cx);
                                        });
                                        window.refresh();
                                    }
                                },
                            )),
                    )
                    .child(Self::divider(palette))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .py_2p5()
                            .child(Self::row_label(
                                cx.lang().get("settings.connections"),
                                cx.lang().get("settings.connections_hint"),
                                palette,
                            ))
                            .child(Self::stepper(
                                StepperConfig {
                                    dec_id: "btn-conn-dec",
                                    input_id: "input-connections",
                                    inc_id: "btn-conn-inc",
                                    input_width: px(38.0),
                                    suffix: None,
                                },
                                &inputs.connections,
                                palette,
                                {
                                    let conn = inputs.connections.clone();
                                    move |_, window, cx| {
                                        conn.update(cx, |state, cx| {
                                            let current =
                                                state.value().trim().parse::<usize>().unwrap_or(8);
                                            let next = current.saturating_sub(1).clamp(1, 32);
                                            state.set_value(next.to_string(), window, cx);
                                        });
                                        window.refresh();
                                    }
                                },
                                {
                                    let conn = inputs.connections.clone();
                                    move |_, window, cx| {
                                        conn.update(cx, |state, cx| {
                                            let current =
                                                state.value().trim().parse::<usize>().unwrap_or(8);
                                            let next = (current + 1).clamp(1, 32);
                                            state.set_value(next.to_string(), window, cx);
                                        });
                                        window.refresh();
                                    }
                                },
                            )),
                    )
                    .child(Self::divider(palette))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .py_2p5()
                            .child(Self::row_label(
                                cx.lang().get("settings.concurrent_downloads"),
                                cx.lang().get("settings.concurrent_downloads_hint"),
                                palette,
                            ))
                            .child(Self::stepper(
                                StepperConfig {
                                    dec_id: "btn-concurrent-dec",
                                    input_id: "input-max-concurrent",
                                    inc_id: "btn-concurrent-inc",
                                    input_width: px(38.0),
                                    suffix: None,
                                },
                                &inputs.max_concurrent,
                                palette,
                                {
                                    let conc = inputs.max_concurrent.clone();
                                    move |_, window, cx| {
                                        conc.update(cx, |state, cx| {
                                            let current =
                                                state.value().trim().parse::<usize>().unwrap_or(3);
                                            let next = current.saturating_sub(1).clamp(1, 16);
                                            state.set_value(next.to_string(), window, cx);
                                        });
                                        window.refresh();
                                    }
                                },
                                {
                                    let conc = inputs.max_concurrent.clone();
                                    move |_, window, cx| {
                                        conc.update(cx, |state, cx| {
                                            let current =
                                                state.value().trim().parse::<usize>().unwrap_or(3);
                                            let next = (current + 1).clamp(1, 16);
                                            state.set_value(next.to_string(), window, cx);
                                        });
                                        window.refresh();
                                    }
                                },
                            )),
                    ),
            )
            .child(
                Self::card(palette)
                    .py_3()
                    .gap_2p5()
                    .child(Self::row_label(
                        cx.lang().get("settings.directory"),
                        cx.lang().get("settings.directory_hint"),
                        palette,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .h(px(34.0))
                            .pl_3()
                            .pr_1()
                            .rounded(px(24.0))
                            .bg(palette.surface)
                            .border_1()
                            .border_color(palette.border)
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
                                        Input::new(&inputs.download_dir)
                                            .id("input-download-dir")
                                            .appearance(false)
                                            .cleanable(false)
                                            .text_size(px(13.0))
                                            .text_color(palette.text_color)
                                            .h(px(26.0))
                                            .w_full(),
                                    ),
                            )
                            .child({
                                let dir_entity = inputs.download_dir.clone();
                                let cfg_dir = config_manager.clone();
                                IconButton::new("btn-browse-download-dir", "icons/folder-open.svg")
                                    .size(px(26.0))
                                    .icon_size(px(14.0))
                                    .rounded(px(20.0))
                                    .hover_bg(palette.surface_hover)
                                    .color(palette.text_muted)
                                    .tooltip(cx.lang().get("new_download.choose_directory"))
                                    .on_click(move |_event, window, cx| {
                                        let prompt = cx.prompt_for_paths(PathPromptOptions {
                                            files: false,
                                            directories: true,
                                            multiple: false,
                                            prompt: Some(
                                                cx.lang().get("new_download.select_directory"),
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
        let user_agent_card = Self::card(palette)
            .py_3()
            .gap_2p5()
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_sm()
                            .text_color(palette.text_color)
                            .child(cx.lang().get("settings.user_agent")),
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
                            .child(cx.lang().get("settings.restore_default"))
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(34.0))
                    .px_3()
                    .rounded(px(24.0))
                    .bg(palette.surface)
                    .border_1()
                    .border_color(palette.border)
                    .on_mouse_down(MouseButton::Left, {
                        let ua = inputs.user_agent.clone();
                        move |_, window, cx| {
                            ua.update(cx, |s, cx| s.focus(window, cx));
                        }
                    })
                    .child(
                        Input::new(&inputs.user_agent)
                            .id("input-user-agent")
                            .appearance(false)
                            .cleanable(true)
                            .text_size(px(13.0))
                            .text_color(palette.text_color)
                            .h(px(26.0))
                            .w_full(),
                    ),
            );

        let system_card = {
            let cfg_auto_check = config_manager.clone();
            let card = Self::card(palette).child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py_2p5()
                    .child(Self::row_label(
                        cx.lang().get("settings.auto_updates"),
                        cx.lang().get("settings.auto_updates_hint"),
                        palette,
                    ))
                    .child(
                        Switch::new("btn-toggle-auto-check")
                            .checked(auto_check)
                            .accessibility_label(cx.lang().get("settings.auto_updates"))
                            .on_change(move |checked, window, _cx| {
                                let _ = cfg_auto_check.update_ui_config(|u| {
                                    u.auto_check_updates = *checked;
                                });
                                window.refresh();
                            }),
                    ),
            );

            #[cfg(any(windows, target_os = "linux"))]
            let card = card
                .child(Self::divider(palette))
                .child(Self::render_desktop_preferences(palette, cx));

            card
        };

        let tools_card = Self::card(palette)
            .child({
                use rdm_core::engine::FfmpegStatus;
                let status = cx.tasks_model().read(cx).ffmpeg_status();
                let message: SharedString = match status {
                    FfmpegStatus::Idle => cx.lang().get("ffmpeg.hint"),
                    FfmpegStatus::Checking => cx.lang().get("ffmpeg.checking"),
                    FfmpegStatus::Ready => cx.lang().get("ffmpeg.ready"),
                    FfmpegStatus::Error(error) => {
                        format!("{} {error}", cx.lang().get("ffmpeg.error")).into()
                    }
                    FfmpegStatus::Downloading {
                        downloaded_bytes,
                        total_bytes,
                    } => {
                        if *total_bytes > 0 {
                            format!(
                                "{} {}%",
                                cx.lang().get("ffmpeg.downloading"),
                                downloaded_bytes.saturating_mul(100) / total_bytes
                            )
                            .into()
                        } else {
                            format!(
                                "{} {} KiB",
                                cx.lang().get("ffmpeg.downloading"),
                                downloaded_bytes / 1024
                            )
                            .into()
                        }
                    }
                };
                let busy = matches!(
                    status,
                    FfmpegStatus::Checking | FfmpegStatus::Downloading { .. }
                );
                let ready = matches!(status, FfmpegStatus::Ready);
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py_2p5()
                    .child(Self::row_label("FFmpeg".into(), message, palette))
                    .child({
                        let tasks_model = cx.tasks_model().clone();
                        Self::action_button(
                            "settings-ffmpeg-download",
                            cx.lang().get(if busy {
                                "ffmpeg.checking"
                            } else if ready {
                                "ffmpeg.check"
                            } else {
                                "ffmpeg.download"
                            }),
                            false,
                        )
                        .disabled(busy)
                        .on_click(move |_, window, cx| {
                            tasks_model.update(cx, |model, cx| model.prepare_ffmpeg(cx));
                            window.refresh();
                        })
                    })
            })
            .child(Self::divider(palette))
            .child({
                use rdm_core::engine::YtDlpStatus;
                let status = cx.tasks_model().read(cx).ytdlp_status();
                let message: SharedString = match status {
                    YtDlpStatus::Idle => cx.lang().get("ytdlp.hint"),
                    YtDlpStatus::Checking => cx.lang().get("ytdlp.checking"),
                    YtDlpStatus::Ready => cx.lang().get("ytdlp.ready"),
                    YtDlpStatus::Error(error) => {
                        format!("{} {error}", cx.lang().get("ytdlp.error")).into()
                    }
                    YtDlpStatus::Downloading {
                        downloaded_bytes,
                        total_bytes,
                    } => {
                        if *total_bytes > 0 {
                            format!(
                                "{} {}%",
                                cx.lang().get("ytdlp.downloading"),
                                downloaded_bytes.saturating_mul(100) / total_bytes
                            )
                            .into()
                        } else {
                            format!(
                                "{} {} KiB",
                                cx.lang().get("ytdlp.downloading"),
                                downloaded_bytes / 1024
                            )
                            .into()
                        }
                    }
                };
                let busy = matches!(
                    status,
                    YtDlpStatus::Checking | YtDlpStatus::Downloading { .. }
                );
                let ready = matches!(status, YtDlpStatus::Ready);
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py_2p5()
                    .child(Self::row_label("yt-dlp".into(), message, palette))
                    .child({
                        let tasks_model = cx.tasks_model().clone();
                        Self::action_button(
                            "settings-ytdlp-download",
                            cx.lang().get(if busy {
                                "ytdlp.checking"
                            } else if ready {
                                "ytdlp.check"
                            } else {
                                "ytdlp.download"
                            }),
                            false,
                        )
                        .disabled(busy)
                        .on_click(move |_, window, cx| {
                            tasks_model.update(cx, |model, cx| model.prepare_ytdlp(cx));
                            window.refresh();
                        })
                    })
            })
            .child(Self::divider(palette))
            .child({
                let enabled = cx.core().config().ui_config().system_notifications;
                let config = cx.core().config().clone();
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py_2p5()
                    .child(Self::row_label(
                        cx.lang().get("settings.system_notifications"),
                        cx.lang().get("settings.system_notifications_hint"),
                        palette,
                    ))
                    .child(
                        Switch::new("settings-toggle-system-notifications")
                            .checked(enabled)
                            .accessibility_label(cx.lang().get("settings.system_notifications"))
                            .on_change(move |checked, window, _cx| {
                                let _ = config.update_ui_config(|ui| {
                                    ui.system_notifications = *checked;
                                });
                                window.refresh();
                            }),
                    )
            })
            .child(Self::divider(palette))
            .child({
                let updater_status = cx.tasks_model().read(cx).updater_status().clone();
                let status_text: SharedString = match &updater_status {
                    UpdaterStatus::Idle => cx.lang().get("updater.idle"),
                    UpdaterStatus::Checking => cx.lang().get("updater.checking"),
                    UpdaterStatus::UpToDate => cx.lang().get("updater.up_to_date"),
                    UpdaterStatus::Available(info) => cx
                        .lang()
                        .get_with("updater.available", &[("version", &info.version)]),
                    UpdaterStatus::Downloading {
                        downloaded_bytes,
                        total_bytes,
                        ..
                    } => {
                        if *total_bytes > 0 {
                            let percent = ((*downloaded_bytes as f64 / *total_bytes as f64)
                                * 100.0)
                                .clamp(0.0, 100.0);
                            cx.lang().get_with(
                                "updater.progress",
                                &[
                                    (
                                        "downloaded",
                                        &format!("{:.1}", *downloaded_bytes as f64 / 1048576.0),
                                    ),
                                    ("total", &format!("{:.1}", *total_bytes as f64 / 1048576.0)),
                                    ("percent", &format!("{percent:.0}")),
                                ],
                            )
                        } else {
                            cx.lang().get_with(
                                "updater.progress_unknown",
                                &[(
                                    "downloaded",
                                    &format!("{:.1}", *downloaded_bytes as f64 / 1048576.0),
                                )],
                            )
                        }
                    }
                    UpdaterStatus::Verifying { .. } => cx.lang().get("updater.verifying"),
                    UpdaterStatus::Installing { .. } => cx.lang().get("updater.installing"),
                    UpdaterStatus::Error { message, .. } => cx
                        .lang()
                        .get_with("updater.error", &[("error", &cx.lang().get(message))]),
                };
                let download_progress = match &updater_status {
                    UpdaterStatus::Downloading {
                        downloaded_bytes,
                        total_bytes,
                        ..
                    } if *total_bytes > 0 => Some(
                        ((*downloaded_bytes as f64 / *total_bytes as f64) * 100.0).clamp(0.0, 100.0)
                            as f32,
                    ),
                    _ => None,
                };
                let release_link = match &updater_status {
                    UpdaterStatus::Available(info) if info.can_auto_install() => {
                        Some(info.html_url.clone())
                    }
                    UpdaterStatus::Error { release_url, .. } => release_url.clone(),
                    _ => None,
                };

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .py_2p5()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .flex_wrap()
                            .gap_3()
                            .child(Self::row_label(
                                cx.lang().get_with(
                                    "updater.title",
                                    &[("version", rdm_core::updater::current_version())],
                                ),
                                status_text,
                                palette,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .children(release_link.map(|url| {
                                        Self::action_button(
                                            "btn-view-release",
                                            cx.lang().get("updater.view_release"),
                                            false,
                                        )
                                        .on_click(move |_, _, cx| cx.open_url(&url))
                                    }))
                                    .child(match &updater_status {
                                        UpdaterStatus::Available(info) => {
                                            if info.can_auto_install() {
                                                let tasks_model = cx.tasks_model().clone();
                                                Self::action_button(
                                                    "btn-install-update",
                                                    cx.lang().get("updater.install"),
                                                    true,
                                                )
                                                .on_click(move |_, window, cx| {
                                                    tasks_model.update(cx, |model, cx| {
                                                        model.install_update(cx);
                                                    });
                                                    window.refresh();
                                                })
                                            } else {
                                                let url = info.html_url.clone();
                                                Self::action_button(
                                                    "btn-view-release-primary",
                                                    cx.lang().get("updater.view_release"),
                                                    true,
                                                )
                                                .on_click(move |_, _, cx| cx.open_url(&url))
                                            }
                                        }
                                        UpdaterStatus::Checking
                                        | UpdaterStatus::Downloading { .. }
                                        | UpdaterStatus::Verifying { .. }
                                        | UpdaterStatus::Installing { .. } => Self::action_button(
                                            "btn-checking-update",
                                            cx.lang().get("updater.busy"),
                                            false,
                                        )
                                        .disabled(true),
                                        UpdaterStatus::Idle
                                        | UpdaterStatus::UpToDate
                                        | UpdaterStatus::Error { .. } => {
                                            let tasks_model = cx.tasks_model().clone();
                                            Self::action_button(
                                                "btn-check-updates",
                                                cx.lang().get("updater.check"),
                                                false,
                                            )
                                            .on_click(
                                                move |_, window, cx| {
                                                    tasks_model.update(cx, |model, cx| {
                                                        model.check_for_updates(cx);
                                                    });
                                                    window.refresh();
                                                },
                                            )
                                        }
                                    }),
                            ),
                    )
                    .children(download_progress.map(|percent| {
                        ProgressBar::new("settings-update-progress", percent).color(palette.accent)
                    }))
            });

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(user_agent_card)
            .child(system_card)
            .child(tools_card)
    }

    #[cfg(any(windows, target_os = "linux"))]
    fn render_desktop_preferences(palette: ModalPalette, cx: &App) -> Div {
        let preferences = cx.global::<DesktopPreferences>().0.read(cx);
        let error_color = cx.theme().error();
        let rows = [
            (
                DesktopPreference::Startup,
                &preferences.startup,
                "startup",
                if cfg!(windows) {
                    cx.lang().get("settings.startup_windows")
                } else {
                    cx.lang().get("settings.startup")
                },
                cx.lang().get("settings.startup_hint"),
            ),
            (
                DesktopPreference::CloseToTray,
                &preferences.close_to_tray,
                "close-to-tray",
                cx.lang().get("settings.close_to_tray"),
                cx.lang().get("settings.close_to_tray_hint"),
            ),
        ];

        let mut content = div().flex().flex_col();
        for (index, (setting, preference, id, title, description)) in rows.into_iter().enumerate() {
            if index > 0 {
                content = content.child(Self::divider(palette));
            }
            let control: AnyElement = match (preference.value, preference.pending) {
                (Some(checked), pending) => Switch::new(id)
                    .checked(checked)
                    .disabled(pending)
                    .accessibility_label(title.clone())
                    .on_change(move |_, _, cx| setting.change(cx))
                    .into_any_element(),
                (None, true) => Self::action_button(id, cx.lang().get("common.processing"), false)
                    .disabled(true)
                    .into_any_element(),
                (None, false) => Self::action_button(id, cx.lang().get("common.retry"), false)
                    .on_click(move |_, _, cx| setting.change(cx))
                    .into_any_element(),
            };
            content =
                content.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .py_2p5()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .child(Self::row_label(title, description, palette))
                                .child(control),
                        )
                        .children(preference.error.as_ref().map(|error| {
                            div().text_xs().text_color(error_color).child(error.clone())
                        })),
                );
        }

        #[cfg(target_os = "linux")]
        if !crate::linux_desktop::available(cx) {
            content = content.child(
                div()
                    .pb_2()
                    .text_xs()
                    .text_color(palette.text_muted)
                    .child(cx.lang().get("settings.tray_unavailable")),
            );
        }

        content
    }

    fn render_appearance(
        current_theme: &str,
        navigation_style: NavigationStyle,
        current_accent: &str,
        state: Entity<SettingsFormState>,
        palette: ModalPalette,
        config: Arc<rdm_config::ConfigManager>,
        cx: &App,
    ) -> Div {
        let accents = [
            "", "#9e9e9e", "#1e1e1e", "#ef4444", "#f97316", "#10b981", "#4422ff", "#9900ff",
            "#ff0099", "#e07070",
        ];

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                Self::card(palette)
                    .child(Self::render_language(
                        state.clone(),
                        config.clone(),
                        palette,
                        cx,
                    ))
                    .child(Self::divider(palette))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .py_2p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.text_color)
                                    .child(cx.lang().get("settings.navigation")),
                            )
                            .child(
                                Self::segmented_track(palette)
                                    .child(Self::navigation_button(
                                        NavigationStyle::Normal,
                                        cx.lang().get("settings.normal"),
                                        navigation_style,
                                        palette,
                                        config.clone(),
                                    ))
                                    .child(Self::navigation_button(
                                        NavigationStyle::Dock,
                                        cx.lang().get("settings.dock"),
                                        navigation_style,
                                        palette,
                                        config.clone(),
                                    )),
                            ),
                    )
                    .child(Self::divider(palette))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .py_2p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.text_color)
                                    .child(cx.lang().get("settings.theme")),
                            )
                            .child(
                                Self::segmented_track(palette)
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
                                        config.clone(),
                                    )),
                            ),
                    ),
            )
            .child(
                Self::card(palette)
                    .py_3()
                    .gap_2p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.text_color)
                            .child(cx.lang().get("settings.theme_color")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .children(accents.iter().map(|&hex| {
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
    }

    fn render_language(
        state: Entity<SettingsFormState>,
        config: Arc<rdm_config::ConfigManager>,
        palette: ModalPalette,
        cx: &App,
    ) -> Div {
        let pending = state.read(cx).language_pending;
        let error = state.read(cx).language_error.clone();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .py_2p5()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_wrap()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.text_color)
                            .child(cx.lang().get("settings.language")),
                    )
                    .child(
                        Self::segmented_track(palette).children(cx.lang().languages().map(
                            |(code, name)| {
                                let code = code.to_owned();
                                let selected = cx.lang().code() == code;
                                let state = state.clone();
                                let config = config.clone();
                                div()
                                    .id(SharedString::from(format!("language-{code}")))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .h(px(28.0))
                                    .px_3()
                                    .rounded(px(20.0))
                                    .text_xs()
                                    .font_weight(if selected {
                                        FontWeight::MEDIUM
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(if selected {
                                        palette.text_color
                                    } else {
                                        palette.text_muted
                                    })
                                    .when(selected, |el| el.bg(palette.surface_hover))
                                    .when(!pending, |el| {
                                        el.cursor_pointer()
                                            .when(!selected, |el| {
                                                el.hover(move |s| s.text_color(palette.text_color))
                                            })
                                            .on_click(move |_, _, cx| {
                                                Self::change_language(
                                                    &code,
                                                    state.clone(),
                                                    config.clone(),
                                                    cx,
                                                )
                                            })
                                    })
                                    .when(pending, |el| el.opacity(0.5))
                                    .child(name.to_owned())
                            },
                        )),
                    ),
            )
            .children(pending.then(|| {
                div()
                    .text_xs()
                    .text_color(palette.text_muted)
                    .child(cx.lang().get("settings.saving_language"))
            }))
            .children(error.map(|error| {
                div().text_xs().text_color(cx.theme().error()).child(
                    cx.lang()
                        .get_with("settings.language_error", &[("error", &error)]),
                )
            }))
    }

    fn change_language(
        code: &str,
        state: Entity<SettingsFormState>,
        config: Arc<rdm_config::ConfigManager>,
        cx: &mut App,
    ) {
        if state.read(cx).language_pending || cx.lang().code() == code {
            return;
        }
        let code = code.to_owned();
        state.update(cx, |state, _| {
            state.language_pending = true;
            state.language_error = None;
        });
        let work = cx
            .tasks_model()
            .read(cx)
            .tokio_handle()
            .spawn_blocking(move || {
                let previous = config.ui_config().language;
                if let Err(error) = config.update_ui_config(|ui| ui.language = code.clone()) {
                    config.update_ui_config_in_memory(|ui| ui.language = previous);
                    return Err(error.to_string());
                }
                Ok(code)
            });
        cx.refresh_windows();
        cx.spawn(async move |cx| {
            let result = work
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result);
            state.update(cx, |state, cx| {
                state.language_pending = false;
                state.language_error = match result {
                    Ok(code) => cx
                        .global_mut::<Lang>()
                        .set_language(&code)
                        .err()
                        .map(|error| error.to_string()),
                    Err(error) => Some(error),
                };
                cx.refresh_windows();
            });
        })
        .detach();
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
        let selection = outline.opacity(0.8);

        let swatch = div()
            .id(if is_default {
                ElementId::Name("color-theme-default".into())
            } else {
                ElementId::Name(format!("color-{hex}").into())
            })
            .size(px(32.0))
            .rounded_full()
            .border_2()
            .border_color(selection.opacity(if is_selected { 1.0 } else { 0.0 }))
            .p(px(2.0))
            .cursor_pointer()
            .hover(move |s| {
                s.border_color(if is_selected {
                    selection
                } else {
                    outline.opacity(0.4)
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
            swatch.tooltip(|_, cx| {
                let label = cx.lang().get("settings.theme_color");
                cx.new(|_| ActionTooltip::new(label)).into()
            })
        } else {
            swatch
        }
    }

    fn navigation_button(
        style: NavigationStyle,
        label: SharedString,
        current: NavigationStyle,
        palette: ModalPalette,
        config: Arc<rdm_config::ConfigManager>,
    ) -> impl IntoElement {
        let style_key = style.key();
        let selected = current == style;
        div()
            .id(ElementId::Name(format!("btn-style-{style_key}").into()))
            .flex()
            .items_center()
            .justify_center()
            .h(px(28.0))
            .px_3()
            .rounded(px(20.0))
            .text_xs()
            .font_weight(if selected {
                FontWeight::MEDIUM
            } else {
                FontWeight::NORMAL
            })
            .text_color(if selected {
                palette.text_color
            } else {
                palette.text_muted
            })
            .when(selected, |el| el.bg(palette.surface_hover))
            .cursor_pointer()
            .when(!selected, |el| {
                el.hover(move |s| s.text_color(palette.text_color))
            })
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
            .child(label)
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
            .flex()
            .items_center()
            .justify_center()
            .h(px(28.0))
            .w(px(36.0))
            .rounded(px(20.0))
            .when(is_selected, |el| el.bg(palette.surface_hover))
            .cursor_pointer()
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
            .child(
                svg()
                    .path(icon_path)
                    .size(px(15.0))
                    .text_color(if is_selected {
                        palette.text_color
                    } else {
                        palette.text_muted
                    }),
            )
    }
}
