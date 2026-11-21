use chrono::{DateTime, Local};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::component::button::ButtonRounded;
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

#[derive(IntoElement)]
pub struct NotificationDetailModal;

impl NotificationDetailModal {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NotificationDetailModal {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for NotificationDetailModal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let modal_state = cx
            .try_global::<NotificationDetailModalState>()
            .cloned()
            .unwrap_or_default();

        let notification = modal_state
            .notification_id
            .and_then(|id| cx.notifications().read(cx).get(id).cloned());
        let is_open = modal_state.open && notification.is_some();

        let theme = cx.theme();
        let background = theme.background();
        let surface_hover = theme.surface_hover();
        let border = theme.border();
        let text_color = theme.text_color();
        let text_muted = theme.text_muted();
        let accent = theme.accent();
        let completed = theme.completed();
        let error_color = theme.error();

        let (modal_w, modal_h) = match notification.as_ref().map(|item| &item.kind) {
            Some(NotificationKind::DownloadStarted { .. }) => (px(520.0), px(275.0)),
            Some(NotificationKind::DownloadCompleted { .. }) => (px(540.0), px(305.0)),
            Some(NotificationKind::DownloadFailed { .. }) => (px(580.0), px(430.0)),
            Some(NotificationKind::UpdateAvailable { .. }) | None => (px(620.0), px(540.0)),
        };

        let (title, body, footer) = if let Some(item) = notification {
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
                        .id("notif-update-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .p_3()
                                .rounded(px(12.0))
                                .bg(accent.opacity(0.12))
                                .border_1()
                                .border_color(accent.opacity(0.3))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(accent)
                                        .child(version_banner),
                                )
                                .child(div().text_xs().text_color(text_muted).child(timestamp)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(text_muted)
                                        .child(cx.lang().get("notifications.changelog")),
                                )
                                .child(
                                    div()
                                        .p_4()
                                        .rounded(px(14.0))
                                        .bg(background)
                                        .border_1()
                                        .border_color(border)
                                        .flex()
                                        .flex_col()
                                        .gap_1p5()
                                        .children(notes.lines().map(|line| {
                                            let trimmed = line.trim();
                                            let is_heading = trimmed.starts_with('#');
                                            div()
                                                .text_xs()
                                                .when(is_heading, |el| {
                                                    el.pt_1()
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(text_color)
                                                })
                                                .when(!is_heading, |el| el.text_color(text_muted))
                                                .child(
                                                    trimmed
                                                        .trim_start_matches('#')
                                                        .trim()
                                                        .to_string(),
                                                )
                                        })),
                                ),
                        )
                        .into_any_element();

                    let install_model = tasks_model.clone();
                    let footer_el = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .pt_2()
                        .child(
                            Button::new("notif-open-release")
                                .label(cx.lang().get("notifications.view_release"))
                                .ghost()
                                .rounded(px(24.0))
                                .on_click(move |_, _, cx| {
                                    cx.open_url(&html_url);
                                }),
                        )
                        .child(
                            Button::new("notif-install-update")
                                .label(cx.lang().get("notifications.download_update"))
                                .primary()
                                .rounded(px(24.0))
                                .on_click(move |_, window, cx| {
                                    install_model.update(cx, |m, cx| m.install_update(cx));
                                    NotificationDetailModalState::close(window, cx);
                                }),
                        )
                        .into_any_element();

                    (title, Some(body_el), Some(footer_el))
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
                        .id("notif-error-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(text_color)
                                        .truncate()
                                        .child(file_name),
                                )
                                .child(div().text_xs().text_color(text_muted).child(timestamp)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_4()
                                .rounded(px(14.0))
                                .bg(error_color.opacity(0.1))
                                .border_1()
                                .border_color(error_color.opacity(0.35))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(error_color)
                                                .child(cx.lang().get("inspector.error_log")),
                                        )
                                        .child(
                                            IconButton::new("notif-copy-error", "icons/copy.svg")
                                                .size(px(24.0))
                                                .icon_size(px(12.0))
                                                .rounded(ButtonRounded::Size(px(99.0)))
                                                .color(error_color)
                                                .hover_bg(error_color.opacity(0.15))
                                                .on_click(move |_, _, cx| {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(
                                                            error_copy.clone(),
                                                        ),
                                                    );
                                                }),
                                        ),
                                )
                                .child(div().text_xs().text_color(text_color).child(error)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1p5()
                                .p_3()
                                .rounded(px(12.0))
                                .bg(background)
                                .border_1()
                                .border_color(border)
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text_muted)
                                        .truncate()
                                        .child(format!("URL: {url}")),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text_muted)
                                        .truncate()
                                        .child(format!("Path: {}", output_path.display())),
                                ),
                        )
                        .into_any_element();

                    let retry_model = tasks_model.clone();
                    let footer_el = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .pt_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .when(task_exists, |row| {
                                    row.child(
                                        Button::new("notif-retry-download")
                                            .label(cx.lang().get("downloads.resume"))
                                            .primary()
                                            .rounded(px(24.0))
                                            .on_click(move |_, window, cx| {
                                                retry_model.update(cx, |m, cx| {
                                                    m.resume_task(task_id, cx);
                                                });
                                                NotificationDetailModalState::close(window, cx);
                                            }),
                                    )
                                    .child(
                                        Button::new("notif-inspect-failed")
                                            .label(cx.lang().get("notifications.inspect"))
                                            .ghost()
                                            .rounded(px(24.0))
                                            .on_click(move |_, window, cx| {
                                                NotificationDetailModalState::close(window, cx);
                                                DownloadInspectorModalState::open(
                                                    task_id, None, window, cx,
                                                );
                                            }),
                                    )
                                }),
                        )
                        .child(
                            Button::new("notif-delete-item")
                                .label(cx.lang().get("common.delete"))
                                .ghost()
                                .rounded(px(24.0))
                                .on_click(move |_, window, cx| {
                                    let notifications = cx.notifications().clone();
                                    notifications.update(cx, |m, cx| m.remove(notif_id, cx));
                                    NotificationDetailModalState::close(window, cx);
                                }),
                        )
                        .into_any_element();

                    (title, Some(body_el), Some(footer_el))
                }
                NotificationKind::DownloadCompleted {
                    task_id,
                    file_name,
                    output_path,
                } => {
                    let title = cx.lang().get("notifications.download_completed");
                    let task_exists = tasks_model.read(cx).task(task_id).is_some();

                    let body_el = div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .rounded(px(14.0))
                        .bg(background)
                        .border_1()
                        .border_color(border)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(completed)
                                        .truncate()
                                        .child(file_name),
                                )
                                .child(div().text_xs().text_color(text_muted).child(timestamp)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(text_muted)
                                .truncate()
                                .child(output_path.to_string_lossy().to_string()),
                        )
                        .into_any_element();

                    let file_to_open = output_path.clone();
                    let file_to_reveal = output_path.clone();
                    let footer_el = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .pt_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new("notif-open-file")
                                        .label(cx.lang().get("downloads.open_file"))
                                        .primary()
                                        .rounded(px(24.0))
                                        .on_click(move |_, _, cx| {
                                            cx.open_with_system(&file_to_open);
                                        }),
                                )
                                .child(
                                    Button::new("notif-open-folder")
                                        .label(cx.lang().get("downloads.reveal_file"))
                                        .ghost()
                                        .rounded(px(24.0))
                                        .on_click(move |_, _, cx| {
                                            cx.reveal_path(&file_to_reveal);
                                        }),
                                )
                                .when(task_exists, |row| {
                                    row.child(
                                        Button::new("notif-inspect-completed")
                                            .label(cx.lang().get("notifications.inspect"))
                                            .ghost()
                                            .rounded(px(24.0))
                                            .on_click(move |_, window, cx| {
                                                NotificationDetailModalState::close(window, cx);
                                                DownloadInspectorModalState::open(
                                                    task_id, None, window, cx,
                                                );
                                            }),
                                    )
                                }),
                        )
                        .child(
                            Button::new("notif-delete-completed")
                                .label(cx.lang().get("common.delete"))
                                .ghost()
                                .rounded(px(24.0))
                                .on_click(move |_, window, cx| {
                                    let notifications = cx.notifications().clone();
                                    notifications.update(cx, |m, cx| m.remove(notif_id, cx));
                                    NotificationDetailModalState::close(window, cx);
                                }),
                        )
                        .into_any_element();

                    (title, Some(body_el), Some(footer_el))
                }
                NotificationKind::DownloadStarted { task_id, file_name } => {
                    let title = cx.lang().get("notifications.download_started");
                    let task_exists = tasks_model.read(cx).task(task_id).is_some();

                    let body_el = div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_4()
                        .rounded(px(14.0))
                        .bg(background)
                        .border_1()
                        .border_color(border)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(text_color)
                                        .truncate()
                                        .child(file_name),
                                )
                                .child(div().text_xs().text_color(text_muted).child(timestamp)),
                        )
                        .into_any_element();

                    let footer_el = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .pt_2()
                        .child(div().when(task_exists, |row| {
                            row.child(
                                Button::new("notif-inspect-started")
                                    .label(cx.lang().get("notifications.inspect"))
                                    .primary()
                                    .rounded(px(24.0))
                                    .on_click(move |_, window, cx| {
                                        NotificationDetailModalState::close(window, cx);
                                        DownloadInspectorModalState::open(
                                            task_id, None, window, cx,
                                        );
                                    }),
                            )
                        }))
                        .child(
                            Button::new("notif-delete-started")
                                .label(cx.lang().get("common.delete"))
                                .ghost()
                                .rounded(px(24.0))
                                .on_click(move |_, window, cx| {
                                    let notifications = cx.notifications().clone();
                                    notifications.update(cx, |m, cx| m.remove(notif_id, cx));
                                    NotificationDetailModalState::close(window, cx);
                                }),
                        )
                        .into_any_element();

                    (title, Some(body_el), Some(footer_el))
                }
            }
        } else {
            (cx.lang().get("notifications.title"), None, None)
        };

        let _ = surface_hover;

        div()
            .id("notification-detail-modal-root")
            .absolute()
            .inset_0()
            .size_full()
            .child(
                Modal::new("notification-detail-modal")
                    .open(is_open)
                    .from_bounds(modal_state.origin)
                    .width(modal_w)
                    .max_height(modal_h)
                    .title(title)
                    .on_close(|window, cx| {
                        NotificationDetailModalState::close(window, cx);
                    })
                    .when_some(footer, |modal, footer| modal.footer(footer))
                    .children(body),
            )
    }
}
