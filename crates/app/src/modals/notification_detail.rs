use chrono::{DateTime, Local};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::base::{Scrollbar, ScrollbarMode};
use uuid::Uuid;

use crate::{
    components::{Button, ButtonVariants, IconButton, Modal},
    lang::ActiveLang,
    modals::DownloadInspectorModalState,
    notifications::NotificationKind,
    state::CoreContext,
    theme::ActiveTheme,
};

#[derive(Default, Clone)]
pub struct NotificationDetailModalState {
    pub open: bool,
    pub notification_id: Option<Uuid>,
    pub origin: Option<Bounds<Pixels>>,
}

impl Global for NotificationDetailModalState {}

impl NotificationDetailModalState {
    pub fn open(
        notification_id: Uuid,
        origin: Option<Bounds<Pixels>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let notifications = cx.notifications().clone();
        notifications.update(cx, |model, cx| {
            model.mark_read(notification_id, cx);
        });
        cx.set_global(Self {
            open: true,
            notification_id: Some(notification_id),
            origin,
        });
        window.refresh();
    }

    pub fn close(window: &mut Window, cx: &mut App) {
        let prev = cx.try_global::<Self>().cloned().unwrap_or_default();
        cx.set_global(Self {
            open: false,
            notification_id: prev.notification_id,
            origin: prev.origin,
        });
        window.blur(cx);
        window.refresh();
    }
}

fn format_timestamp(ts: i64) -> String {
    DateTime::from_timestamp(ts, 0)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

#[derive(Clone, Copy)]
struct DetailPalette {
    card_surface: Rgba,
    surface_hover: Rgba,
    border: Rgba,
    text_color: Rgba,
    text_muted: Rgba,
    accent: Rgba,
    completed: Rgba,
    error: Rgba,
}

#[derive(IntoElement)]
pub struct NotificationDetailModal;

impl NotificationDetailModal {
    pub fn new() -> Self {
        Self
    }

    fn card(palette: DetailPalette) -> Div {
        div()
            .flex()
            .flex_col()
            .rounded(px(24.0))
            .bg(palette.card_surface)
            .border_1()
            .border_color(palette.border)
            .px_4()
            .py_3()
    }

    fn divider(palette: DetailPalette) -> Div {
        div().h(px(1.0)).w_full().bg(palette.border).opacity(0.6)
    }

    fn icon_action(id: impl Into<ElementId>, icon: &'static str, tooltip: SharedString) -> Button {
        Button::new(id)
            .size(px(34.0))
            .p_0()
            .rounded(px(24.0))
            .tooltip(tooltip.clone())
            .accessibility_label(tooltip)
            .child(svg().path(icon).size(px(15.0)))
    }

    fn render_header(
        title: SharedString,
        status_color: Option<Rgba>,
        palette: DetailPalette,
        cx: &App,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap_3()
            .pl_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .children(
                        status_color
                            .map(|color| div().size(px(8.0)).flex_none().rounded_full().bg(color)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.text_color)
                            .truncate()
                            .child(title),
                    ),
            )
            .child(
                IconButton::new("btn-close-notification-detail", "icons/x.svg")
                    .size(px(32.0))
                    .rounded(px(24.0))
                    .color(palette.text_muted)
                    .hover_bg(palette.surface_hover)
                    .tooltip(cx.lang().get("common.close"))
                    .on_click(|_, window, cx| NotificationDetailModalState::close(window, cx)),
            )
    }
}

impl Default for NotificationDetailModal {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for NotificationDetailModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let modal_state = cx
            .try_global::<NotificationDetailModalState>()
            .cloned()
            .unwrap_or_default();

        let notification = modal_state
            .notification_id
            .and_then(|id| cx.notifications().read(cx).get(id).cloned());
        let is_open = modal_state.open && notification.is_some();

        let theme = cx.theme();
        let palette = DetailPalette {
            card_surface: theme
                .input_background()
                .unwrap_or_else(|| theme.background()),
            surface_hover: theme.surface_hover(),
            border: theme.border(),
            text_color: theme.text_color(),
            text_muted: theme.text_muted(),
            accent: theme.accent(),
            completed: theme.completed(),
            error: theme.error(),
        };
        let viewport = window.viewport_size();
        let scroll_handle = window
            .use_keyed_state("notification-detail-scroll-handle", cx, |_, _| {
                ScrollHandle::new()
            })
            .read(cx)
            .clone();

        let (modal_w, modal_h, fit_content) = match notification.as_ref().map(|item| &item.kind) {
            Some(NotificationKind::DownloadStarted { .. }) => (px(480.0), px(164.0), true),
            Some(NotificationKind::DownloadCompleted { .. }) => (px(520.0), px(188.0), true),
            Some(NotificationKind::DownloadFailed { .. }) => (px(560.0), px(360.0), false),
            Some(NotificationKind::UpdateAvailable { .. }) | None => (px(580.0), px(460.0), false),
        };

        let (title, status_color, body, footer) = if let Some(item) = notification {
            let notif_id = item.id;
            let timestamp = format_timestamp(item.created_at);
            let tasks_model = cx.tasks_model().clone();

            match item.kind {
                NotificationKind::UpdateAvailable { info } => {
                    let title = cx
                        .lang()
                        .get_with("titlebar.update_available", &[("version", &info.version)]);
                    let notes = if info.release_notes.trim().is_empty() {
                        cx.lang().get("notifications.no_changelog").to_string()
                    } else {
                        info.release_notes.clone()
                    };
                    let html_url = info.html_url.clone();
                    let version_banner = format!(
                        "v{} → v{}",
                        rdm_core::updater::current_version(),
                        info.version
                    );

                    let body_el = div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .child(
                            div()
                                .id("notif-update-scroll")
                                .size_full()
                                .overflow_y_scroll()
                                .track_scroll(&scroll_handle)
                                .pr_2()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_3()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .px_4()
                                                .py_2p5()
                                                .rounded(px(24.0))
                                                .bg(palette.accent.opacity(0.12))
                                                .border_1()
                                                .border_color(palette.accent.opacity(0.3))
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(palette.accent)
                                                        .child(version_banner),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(palette.text_muted)
                                                        .child(timestamp),
                                                ),
                                        )
                                        .child(
                                            Self::card(palette)
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(palette.text_muted)
                                                        .child(
                                                            cx.lang()
                                                                .get("notifications.changelog"),
                                                        ),
                                                )
                                                .child(Self::divider(palette))
                                                .child(div().flex().flex_col().gap_1p5().children(
                                                    notes.lines().map(|line| {
                                                        let trimmed = line.trim();
                                                        let is_heading = trimmed.starts_with('#');
                                                        div()
                                                            .text_xs()
                                                            .when(is_heading, |el| {
                                                                el.pt_1()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(palette.text_color)
                                                            })
                                                            .when(!is_heading, |el| {
                                                                el.text_color(palette.text_muted)
                                                            })
                                                            .child(
                                                                trimmed
                                                                    .trim_start_matches('#')
                                                                    .trim()
                                                                    .to_string(),
                                                            )
                                                    }),
                                                )),
                                        ),
                                ),
                        )
                        .child(Scrollbar::vertical(&scroll_handle).mode(ScrollbarMode::Always))
                        .into_any_element();

                    let install_model = tasks_model.clone();
                    let footer_el = div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .pt_1()
                        .child(
                            Self::icon_action(
                                "notif-open-release",
                                "icons/external-link.svg",
                                cx.lang().get("notifications.view_release"),
                            )
                            .on_click(move |_, _, cx| {
                                cx.open_url(&html_url);
                            }),
                        )
                        .child(
                            Self::icon_action(
                                "notif-install-update",
                                "icons/download.svg",
                                cx.lang().get("notifications.download_update"),
                            )
                            .primary()
                            .on_click(move |_, window, cx| {
                                install_model.update(cx, |m, cx| m.install_update(cx));
                                NotificationDetailModalState::close(window, cx);
                            }),
                        )
                        .into_any_element();

                    (title, Some(palette.accent), Some(body_el), Some(footer_el))
                }
                NotificationKind::DownloadFailed {
                    task_id,
                    file_name,
                    url,
                    output_path,
                    error,
                } => {
                    let title = cx.lang().get("notifications.download_failed");
                    let error_copy = error.clone();
                    let task_exists = tasks_model.read(cx).task(task_id).is_some();

                    let body_el = div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .child(
                            div()
                                .id("notif-error-scroll")
                                .size_full()
                                .overflow_y_scroll()
                                .track_scroll(&scroll_handle)
                                .pr_2()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_3()
                                        .child(
                                            Self::card(palette)
                                                .py_1()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_3()
                                                        .py_2()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(palette.text_color)
                                                                .truncate()
                                                                .child(file_name),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_none()
                                                                .text_xs()
                                                                .text_color(palette.text_muted)
                                                                .child(timestamp),
                                                        ),
                                                )
                                                .child(Self::divider(palette))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_1()
                                                        .py_2()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(palette.text_muted)
                                                                .truncate()
                                                                .child(format!("URL: {url}")),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(palette.text_muted)
                                                                .truncate()
                                                                .child(format!(
                                                                    "Path: {}",
                                                                    output_path.display()
                                                                )),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_2()
                                                .px_4()
                                                .py_3()
                                                .rounded(px(24.0))
                                                .bg(palette.error.opacity(0.1))
                                                .border_1()
                                                .border_color(palette.error.opacity(0.3))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(palette.error)
                                                                .child(
                                                                    cx.lang()
                                                                        .get("inspector.error_log"),
                                                                ),
                                                        )
                                                        .child(
                                                            IconButton::new(
                                                                "notif-copy-error",
                                                                "icons/copy.svg",
                                                            )
                                                            .size(px(24.0))
                                                            .icon_size(px(12.0))
                                                            .rounded(px(20.0))
                                                            .color(palette.error)
                                                            .hover_bg(palette.error.opacity(0.15))
                                                            .on_click(move |_, _, cx| {
                                                                cx.write_to_clipboard(
                                                                    ClipboardItem::new_string(
                                                                        error_copy.clone(),
                                                                    ),
                                                                );
                                                            }),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(palette.text_color)
                                                        .child(error),
                                                ),
                                        ),
                                ),
                        )
                        .child(Scrollbar::vertical(&scroll_handle).mode(ScrollbarMode::Always))
                        .into_any_element();

                    let retry_model = tasks_model.clone();
                    let footer_el = div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .pt_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .when(task_exists, |row| {
                                    row.child(
                                        Self::icon_action(
                                            "notif-retry-download",
                                            "icons/resume.svg",
                                            cx.lang().get("downloads.resume"),
                                        )
                                        .primary()
                                        .on_click(
                                            move |_, window, cx| {
                                                retry_model.update(cx, |m, cx| {
                                                    m.resume_task(task_id, cx);
                                                });
                                                NotificationDetailModalState::close(window, cx);
                                            },
                                        ),
                                    )
                                    .child(
                                        Self::icon_action(
                                            "notif-inspect-failed",
                                            "icons/info.svg",
                                            cx.lang().get("notifications.inspect"),
                                        )
                                        .on_click(
                                            move |_, window, cx| {
                                                NotificationDetailModalState::close(window, cx);
                                                DownloadInspectorModalState::open(
                                                    task_id, None, window, cx,
                                                );
                                            },
                                        ),
                                    )
                                }),
                        )
                        .child(
                            Self::icon_action(
                                "notif-delete-item",
                                "icons/trash.svg",
                                cx.lang().get("common.delete"),
                            )
                            .danger()
                            .on_click(move |_, window, cx| {
                                let notifications = cx.notifications().clone();
                                notifications.update(cx, |m, cx| m.remove(notif_id, cx));
                                NotificationDetailModalState::close(window, cx);
                            }),
                        )
                        .into_any_element();

                    (title, Some(palette.error), Some(body_el), Some(footer_el))
                }
                NotificationKind::DownloadCompleted {
                    task_id,
                    file_name,
                    output_path,
                } => {
                    let title = cx.lang().get("notifications.download_completed");
                    let task_exists = tasks_model.read(cx).task(task_id).is_some();

                    let body_el = Self::card(palette)
                        .gap_1p5()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.text_color)
                                        .truncate()
                                        .child(file_name),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .text_xs()
                                        .text_color(palette.text_muted)
                                        .child(timestamp),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(palette.text_muted)
                                .truncate()
                                .child(output_path.to_string_lossy().to_string()),
                        )
                        .into_any_element();

                    let file_to_open = output_path.clone();
                    let file_to_reveal = output_path.clone();
                    let footer_el = div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .pt_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Self::icon_action(
                                        "notif-open-file",
                                        "icons/external-link.svg",
                                        cx.lang().get("downloads.open_file"),
                                    )
                                    .primary()
                                    .on_click(
                                        move |_, _, cx| {
                                            cx.open_with_system(&file_to_open);
                                        },
                                    ),
                                )
                                .child(
                                    Self::icon_action(
                                        "notif-open-folder",
                                        "icons/folder-open.svg",
                                        cx.lang().get("downloads.reveal_file"),
                                    )
                                    .on_click(
                                        move |_, _, cx| {
                                            cx.reveal_path(&file_to_reveal);
                                        },
                                    ),
                                )
                                .when(task_exists, |row| {
                                    row.child(
                                        Self::icon_action(
                                            "notif-inspect-completed",
                                            "icons/info.svg",
                                            cx.lang().get("notifications.inspect"),
                                        )
                                        .on_click(
                                            move |_, window, cx| {
                                                NotificationDetailModalState::close(window, cx);
                                                DownloadInspectorModalState::open(
                                                    task_id, None, window, cx,
                                                );
                                            },
                                        ),
                                    )
                                }),
                        )
                        .child(
                            Self::icon_action(
                                "notif-delete-completed",
                                "icons/trash.svg",
                                cx.lang().get("common.delete"),
                            )
                            .danger()
                            .on_click(move |_, window, cx| {
                                let notifications = cx.notifications().clone();
                                notifications.update(cx, |m, cx| m.remove(notif_id, cx));
                                NotificationDetailModalState::close(window, cx);
                            }),
                        )
                        .into_any_element();

                    (
                        title,
                        Some(palette.completed),
                        Some(body_el),
                        Some(footer_el),
                    )
                }
                NotificationKind::DownloadStarted { task_id, file_name } => {
                    let title = cx.lang().get("notifications.download_started");
                    let task_exists = tasks_model.read(cx).task(task_id).is_some();

                    let body_el = Self::card(palette)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.text_color)
                                        .truncate()
                                        .child(file_name),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .text_xs()
                                        .text_color(palette.text_muted)
                                        .child(timestamp),
                                ),
                        )
                        .into_any_element();

                    let footer_el = div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .pt_1()
                        .child(div().when(task_exists, |row| {
                            row.child(
                                Self::icon_action(
                                    "notif-inspect-started",
                                    "icons/info.svg",
                                    cx.lang().get("notifications.inspect"),
                                )
                                .primary()
                                .on_click(move |_, window, cx| {
                                    NotificationDetailModalState::close(window, cx);
                                    DownloadInspectorModalState::open(task_id, None, window, cx);
                                }),
                            )
                        }))
                        .child(
                            Self::icon_action(
                                "notif-delete-started",
                                "icons/trash.svg",
                                cx.lang().get("common.delete"),
                            )
                            .danger()
                            .on_click(move |_, window, cx| {
                                let notifications = cx.notifications().clone();
                                notifications.update(cx, |m, cx| m.remove(notif_id, cx));
                                NotificationDetailModalState::close(window, cx);
                            }),
                        )
                        .into_any_element();

                    (title, Some(palette.accent), Some(body_el), Some(footer_el))
                }
            }
        } else {
            (cx.lang().get("notifications.title"), None, None, None)
        };

        let modal = Modal::new("notification-detail-modal")
            .open(is_open)
            .compact()
            .when(fit_content, |modal| modal.fit_content())
            .from_bounds(modal_state.origin)
            .width((viewport.width - px(32.0)).clamp(px(0.0), modal_w))
            .max_height((viewport.height - px(32.0)).clamp(px(0.0), modal_h))
            .header(Self::render_header(title, status_color, palette, cx))
            .on_close(|window, cx| {
                NotificationDetailModalState::close(window, cx);
            })
            .when_some(footer, |modal, footer| modal.footer(footer))
            .children(body);

        div()
            .id("notification-detail-modal-root")
            .absolute()
            .inset_0()
            .size_full()
            .capture_key_down(move |event: &KeyDownEvent, window, cx| {
                if is_open && event.keystroke.key == "escape" {
                    NotificationDetailModalState::close(window, cx);
                    cx.stop_propagation();
                }
            })
            .child(modal)
    }
}
