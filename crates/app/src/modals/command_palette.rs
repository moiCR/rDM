use std::{cell::Cell, rc::Rc};

use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_pages::router;
use uuid::Uuid;

use crate::{
    components::Input,
    lang::ActiveLang,
    layout::toggle_sidebar,
    modals::{DownloadInspectorModalState, NewDownloadModal, SettingsModalState, SettingsTab},
    state::CoreContext,
    theme::{ActiveTheme, ThemeManager},
};

#[derive(Default)]
pub struct CommandPaletteState {
    open: bool,
    form: Option<Entity<CommandPaletteForm>>,
    origin: Option<Bounds<Pixels>>,
}

impl Global for CommandPaletteState {}

#[derive(Default)]
pub struct TitlebarSearchAnchor(pub Rc<Cell<Option<Bounds<Pixels>>>>);

impl Global for TitlebarSearchAnchor {}

#[derive(Clone)]
enum PaletteAction {
    Navigate(&'static str),
    InspectDownload(Uuid),
    OpenSettings(SettingsTab),
    NewDownload,
    ToggleSidebar,
    ToggleTheme,
    CheckUpdates,
    MarkAllNotificationsRead,
}

#[derive(Clone)]
struct PaletteEntry {
    id: SharedString,
    category: SharedString,
    title: SharedString,
    subtitle: Option<SharedString>,
    icon: &'static str,
    action: PaletteAction,
    score: u32,
}

fn score_match(query: &str, title: &str, subtitle: Option<&str>, keywords: &str) -> Option<u32> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return Some(1);
    }
    let title_l = title.to_ascii_lowercase();
    if title_l == q {
        return Some(100);
    }
    if title_l.starts_with(&q) {
        return Some(80);
    }
    if title_l.contains(&q) {
        return Some(60);
    }
    if let Some(sub) = subtitle
        && sub.to_ascii_lowercase().contains(&q)
    {
        return Some(40);
    }
    if keywords.to_ascii_lowercase().contains(&q) {
        return Some(30);
    }
    None
}

struct CommandPaletteForm {
    query: Entity<InputState>,
    selected: usize,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl CommandPaletteForm {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholder = cx.lang().get("search.placeholder");
        let query = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let sub = cx.subscribe(&query, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.selected = 0;
                cx.notify();
            }
        });
        let query_focus = query.clone();
        window.defer(cx, move |window, cx| {
            query_focus.update(cx, |input, cx| input.focus(window, cx));
        });
        Self {
            query,
            selected: 0,
            scroll: ScrollHandle::new(),
            _subscriptions: vec![sub],
        }
    }

    fn entries(&self, cx: &App) -> Vec<PaletteEntry> {
        let query = self.query.read(cx).value().to_string();
        let mut results = Vec::new();

        let page_cat = cx.lang().get("search.category_pages");
        let pages = [
            (
                "page-dashboard",
                cx.lang().get("navigation.dashboard"),
                "/",
                "icons/dashboard.svg",
                "home inicio dashboard panel",
            ),
            (
                "page-downloads",
                cx.lang().get("navigation.downloads"),
                "/downloads",
                "icons/download.svg",
                "downloads descargas lista cola queue",
            ),
            (
                "page-notifications",
                cx.lang().get("navigation.notifications"),
                "/notifications",
                "icons/bell.svg",
                "notifications notificaciones alertas updates changelog",
            ),
        ];
        for (id, title, path, icon, kw) in pages {
            if let Some(score) = score_match(&query, &title, Some(path), kw) {
                results.push(PaletteEntry {
                    id: id.into(),
                    category: page_cat.clone(),
                    title,
                    subtitle: Some(path.into()),
                    icon,
                    action: PaletteAction::Navigate(path),
                    score: score + 10,
                });
            }
        }

        let dl_cat = cx.lang().get("search.category_downloads");
        for task in cx.tasks_model().read(cx).tasks() {
            if let Some(score) = score_match(&query, &task.file_name, Some(&task.url), "") {
                results.push(PaletteEntry {
                    id: format!("dl-{}", task.id).into(),
                    category: dl_cat.clone(),
                    title: task.file_name.clone().into(),
                    subtitle: Some(task.url.clone().into()),
                    icon: "icons/download.svg",
                    action: PaletteAction::InspectDownload(task.id),
                    score: score + 5,
                });
            }
        }

        let settings_cat = cx.lang().get("search.category_settings");
        let settings = [
            (
                "set-downloads",
                cx.lang().get("settings.downloads"),
                SettingsTab::Downloads,
                "downloads folder speed connections proxy headers cookies",
            ),
            (
                "set-appearance",
                cx.lang().get("settings.appearance"),
                SettingsTab::Appearance,
                "theme dark light accent language sidebar dock idioma tema",
            ),
            (
                "set-advanced",
                cx.lang().get("settings.advanced"),
                SettingsTab::Advanced,
                "ffmpeg ytdlp yt-dlp updates tray startup notifications",
            ),
        ];
        for (id, title, tab, kw) in settings {
            if let Some(score) = score_match(&query, &title, None, kw) {
                results.push(PaletteEntry {
                    id: id.into(),
                    category: settings_cat.clone(),
                    title,
                    subtitle: Some(cx.lang().get("navigation.settings")),
                    icon: "icons/settings.svg",
                    action: PaletteAction::OpenSettings(tab),
                    score,
                });
            }
        }

        let action_cat = cx.lang().get("search.category_actions");
        let actions = [
            (
                "act-new",
                cx.lang().get("titlebar.new_download"),
                Some(SharedString::from("Ctrl+N")),
                "icons/plus.svg",
                PaletteAction::NewDownload,
                "new download nueva descarga add url",
            ),
            (
                "act-sidebar",
                cx.lang().get("titlebar.toggle_sidebar"),
                Some(SharedString::from("Ctrl+B")),
                "icons/panel-left.svg",
                PaletteAction::ToggleSidebar,
                "sidebar barra lateral collapse hide show",
            ),
            (
                "act-theme",
                cx.lang().get("search.action_toggle_theme"),
                None,
                "icons/palette.svg",
                PaletteAction::ToggleTheme,
                "theme dark light mode tema oscuro claro",
            ),
            (
                "act-updates",
                cx.lang().get("search.action_check_updates"),
                None,
                "icons/info.svg",
                PaletteAction::CheckUpdates,
                "update actualizacion version check",
            ),
            (
                "act-mark-read",
                cx.lang().get("notifications.mark_all_read"),
                None,
                "icons/bell.svg",
                PaletteAction::MarkAllNotificationsRead,
                "notifications read leidas limpiar",
            ),
        ];
        for (id, title, subtitle, icon, action, kw) in actions {
            if let Some(score) = score_match(&query, &title, subtitle.as_deref(), kw) {
                results.push(PaletteEntry {
                    id: id.into(),
                    category: action_cat.clone(),
                    title,
                    subtitle,
                    icon,
                    action,
                    score,
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        results.truncate(30);
        results
    }

    fn execute(action: PaletteAction, window: &mut Window, cx: &mut App) {
        CommandPaletteModal::close(window, cx);
        match action {
            PaletteAction::Navigate(path) => {
                if let Some(manager) = router(window, cx) {
                    manager.update(cx, |manager, cx| {
                        let _ = manager.navigate(path, cx);
                    });
                }
            }
            PaletteAction::InspectDownload(task_id) => {
                DownloadInspectorModalState::open(task_id, None, window, cx);
            }
            PaletteAction::OpenSettings(tab) => {
                cx.set_global(SettingsModalState { open: true, tab });
                window.refresh();
            }
            PaletteAction::NewDownload => {
                NewDownloadModal::open(None, window, cx);
            }
            PaletteAction::ToggleSidebar => {
                toggle_sidebar(window, cx);
            }
            PaletteAction::ToggleTheme => {
                let current = cx.core().config().ui_config().theme;
                let next = if current == "dark" { "light" } else { "dark" };
                let _ = cx.core().config().update_ui_config(|ui| {
                    ui.theme = next.to_string();
                });
                ThemeManager::apply_mode(next, cx);
                window.refresh();
            }
            PaletteAction::CheckUpdates => {
                let model = cx.tasks_model().clone();
                model.update(cx, |m, cx| m.check_for_updates(cx));
            }
            PaletteAction::MarkAllNotificationsRead => {
                let notifications = cx.notifications().clone();
                notifications.update(cx, |m, cx| m.mark_all_read(cx));
            }
        }
    }
}

impl Render for CommandPaletteForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = cx.try_global::<CommandPaletteState>();
        let is_open = state.is_some_and(|s| s.open);
        let origin = state.and_then(|s| s.origin);

        let entries = self.entries(cx);
        let count = entries.len();
        if count > 0 && self.selected >= count {
            self.selected = count - 1;
        }
        let selected_index = self.selected;
        let selected_action = entries
            .get(selected_index)
            .map(|entry| entry.action.clone());

        let theme = cx.theme();
        let surface = theme.surface();
        let surface_hover = theme.surface_hover();
        let border = theme.border();
        let text_color = theme.text_color();
        let text_muted = theme.text_muted();
        let accent = theme.accent();

        let viewport = window.viewport_size();
        let origin_w = origin.map(|b| b.size.width).unwrap_or_else(|| px(280.0));
        let origin_h = origin.map(|b| b.size.height).unwrap_or_else(|| px(30.0));
        let origin_x = origin
            .map(|b| b.origin.x)
            .unwrap_or_else(|| (viewport.width - origin_w) / 2.0);
        let origin_y = origin.map(|b| b.origin.y).unwrap_or_else(|| px(9.0));

        let target_w = px(460.0).min((viewport.width - px(48.0)).max(px(300.0)));
        let target_x = ((viewport.width - target_w) / 2.0).max(px(16.0));
        let estimated_rows = if count == 0 {
            px(72.0)
        } else {
            px((count.min(6) as f32) * 52.0 + 16.0)
        };
        let target_h = (px(44.0) + estimated_rows).min((viewport.height - px(48.0)).min(px(390.0)));

        let spring = SpringAnimation::new(SpringConfig::new(320.0, 36.0, 1.0))
            .to(is_open)
            .from(false);

        let query_input = Input::new(&self.query)
            .id("command-palette-input")
            .appearance(false)
            .text_size(px(14.0))
            .text_color(text_color)
            .w_full();

        let no_results_label = cx.lang().get("search.no_results");
        let scroll = self.scroll.clone();

        let key_handler = cx.listener(move |this, event: &KeyDownEvent, window, cx| {
            match event.keystroke.key.as_str() {
                "escape" => {
                    CommandPaletteModal::close(window, cx);
                    cx.stop_propagation();
                }
                "down" if count > 0 => {
                    this.selected = (this.selected + 1) % count;
                    cx.notify();
                    cx.stop_propagation();
                }
                "up" if count > 0 => {
                    this.selected = (this.selected + count - 1) % count;
                    cx.notify();
                    cx.stop_propagation();
                }
                "enter" => {
                    if let Some(action) = selected_action.clone() {
                        Self::execute(action, window, cx);
                        cx.stop_propagation();
                    }
                }
                _ => {}
            }
        });

        div()
            .id("command-palette-overlay")
            .absolute()
            .inset_0()
            .size_full()
            .with_spring("command-palette-expand", spring, move |overlay, phase| {
                let alpha = phase.interpolate_clamped(0.0, 1.0);
                if alpha <= 0.005 {
                    return overlay.invisible();
                }
                let left = phase.interpolate_clamped(origin_x, target_x);
                let width = phase.interpolate_clamped(origin_w, target_w);
                let height = phase.interpolate_clamped(origin_h, target_h);
                let header_h = phase.interpolate_clamped(origin_h, px(42.0));
                let list_alpha = ((alpha - 0.25) / 0.75).clamp(0.0, 1.0);
                let card_alpha = (alpha / 0.15).clamp(0.0, 1.0);

                overlay
                    .visible()
                    .when(is_open, |el| {
                        el.occlude()
                            .on_click(|_, window, cx| {
                                CommandPaletteModal::close(window, cx);
                            })
                            .on_key_down(key_handler)
                    })
                    .child(
                        div()
                            .id("command-palette-card")
                            .occlude()
                            .absolute()
                            .left(left)
                            .top(origin_y)
                            .w(width)
                            .h(height)
                            .opacity(card_alpha)
                            .rounded(px(16.0))
                            .bg(surface)
                            .border_1()
                            .border_color(border)
                            .shadow_xl()
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .on_click(|_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2p5()
                                    .px_3p5()
                                    .h(header_h)
                                    .flex_shrink_0()
                                    .child(
                                        svg()
                                            .path("icons/search.svg")
                                            .size_4()
                                            .flex_shrink_0()
                                            .text_color(text_muted),
                                    )
                                    .child(div().flex_1().min_w_0().child(query_input)),
                            )
                            .child(
                                div()
                                    .id("command-palette-results")
                                    .track_scroll(&scroll)
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .px_2()
                                    .pb_2()
                                    .opacity(list_alpha)
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .when(entries.is_empty(), |list| {
                                        list.child(
                                            div()
                                                .py_6()
                                                .flex()
                                                .justify_center()
                                                .text_xs()
                                                .text_color(text_muted)
                                                .child(no_results_label),
                                        )
                                    })
                                    .children(entries.into_iter().enumerate().map(
                                        |(index, entry)| {
                                            let is_selected = index == selected_index;
                                            let action = entry.action.clone();
                                            div()
                                                .id(ElementId::Name(entry.id.clone()))
                                                .w_full()
                                                .px_2p5()
                                                .py_2()
                                                .rounded(px(10.0))
                                                .cursor_pointer()
                                                .when(is_selected, |row| row.bg(surface_hover))
                                                .hover(move |style| style.bg(surface_hover))
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .gap_3()
                                                .on_click(move |_, window, cx| {
                                                    Self::execute(action.clone(), window, cx);
                                                })
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_2p5()
                                                        .min_w_0()
                                                        .child(
                                                            svg()
                                                                .path(entry.icon)
                                                                .size_4()
                                                                .flex_shrink_0()
                                                                .text_color(if is_selected {
                                                                    accent
                                                                } else {
                                                                    text_muted
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .min_w_0()
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .font_weight(
                                                                            FontWeight::MEDIUM,
                                                                        )
                                                                        .text_color(text_color)
                                                                        .truncate()
                                                                        .child(entry.title),
                                                                )
                                                                .when_some(
                                                                    entry.subtitle,
                                                                    |col, sub| {
                                                                        col.child(
                                                                            div()
                                                                                .text_xs()
                                                                                .text_color(
                                                                                    text_muted,
                                                                                )
                                                                                .truncate()
                                                                                .child(sub),
                                                                        )
                                                                    },
                                                                ),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .text_xs()
                                                        .text_color(text_muted)
                                                        .child(entry.category),
                                                )
                                        },
                                    )),
                            ),
                    )
            })
    }
}

#[derive(IntoElement)]
pub struct CommandPaletteModal;

impl CommandPaletteModal {
    pub fn new() -> Self {
        Self
    }

    pub fn anchor_slot(cx: &mut App) -> Rc<Cell<Option<Bounds<Pixels>>>> {
        if cx.try_global::<TitlebarSearchAnchor>().is_none() {
            cx.set_global(TitlebarSearchAnchor::default());
        }
        Rc::clone(&cx.global::<TitlebarSearchAnchor>().0)
    }

    pub fn open(window: &mut Window, cx: &mut App) {
        let origin = cx
            .try_global::<TitlebarSearchAnchor>()
            .and_then(|anchor| anchor.0.get());
        if let Some(form) = cx
            .try_global::<CommandPaletteState>()
            .and_then(|state| state.form.clone())
        {
            cx.set_global(CommandPaletteState {
                open: true,
                form: Some(form.clone()),
                origin,
            });
            let input = form.read(cx).query.clone();
            input.update(cx, |input, cx| {
                input.set_value(String::new(), window, cx);
                input.focus(window, cx);
            });
            window.refresh();
            return;
        }
        let form = cx.new(|cx| CommandPaletteForm::new(window, cx));
        cx.set_global(CommandPaletteState {
            open: true,
            form: Some(form),
            origin,
        });
        window.refresh();
    }

    pub fn close(window: &mut Window, cx: &mut App) {
        let form = cx
            .try_global::<CommandPaletteState>()
            .and_then(|state| state.form.clone());
        let origin = cx
            .try_global::<CommandPaletteState>()
            .and_then(|state| state.origin);
        cx.set_global(CommandPaletteState {
            open: false,
            form,
            origin,
        });
        window.blur(cx);
        window.refresh();
    }
}

impl Default for CommandPaletteModal {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for CommandPaletteModal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div().absolute().inset_0().size_full().children(
            cx.try_global::<CommandPaletteState>()
                .and_then(|state| state.form.clone()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::score_match;

    #[test]
    fn ranks_exact_and_prefix_matches_above_substring_and_keywords() {
        let exact = score_match("downloads", "Downloads", None, "");
        let prefix = score_match("down", "Downloads", None, "");
        let substring = score_match("load", "Downloads", None, "");
        let keyword = score_match("cola", "Downloads", None, "cola queue");
        assert!(exact > prefix);
        assert!(prefix > substring);
        assert!(substring > keyword);
        assert_eq!(score_match("zzz", "Downloads", None, "cola"), None);
    }
}
