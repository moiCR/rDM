use std::{cell::Cell, rc::Rc};

use chrono::{DateTime, Local};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_pages::{Page, PageAnimation, PageOptions};

use crate::{
    components::{
        Button, ButtonVariants, download_card::download_card_surface, track_card_bounds,
    },
    lang::ActiveLang,
    modals::NotificationDetailModalState,
    notifications::{AppNotification, NotificationKind},
    state::CoreContext,
    theme::ActiveTheme,
};

pub struct NotificationsPage;

impl Page for NotificationsPage {
    fn new(_: &mut Context<Self>) -> Self {
        Self
    }

    fn options() -> PageOptions {
        PageOptions::default().on_open(PageAnimation::SlideUpFade)
    }

    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let background = theme.background();
        let surface = theme.surface();
        let surface_hover = theme.surface_hover();
        let border = theme.border();
        let text_color = theme.text_color();
        let text_muted = theme.text_muted();
        let accent = theme.accent();
        let completed = theme.completed();
        let error_color = theme.error();
        let downloading = theme.downloading();

        let notifications_model = cx.notifications().clone();
        let items = notifications_model.read(cx).items().to_vec();
        let unread = notifications_model.read(cx).unread_count();
        let is_empty = items.is_empty();

        let mark_all_model = notifications_model.clone();
        let clear_model = notifications_model.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
            .text_color(text_color)
            .p_5()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2p5()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child(cx.lang().get("notifications.title")),
                            )
                            .when(unread > 0, |row| {
                                row.child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(accent.opacity(0.18))
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(accent)
                                        .child(unread.to_string()),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(unread > 0, |row| {
                                row.child(
                                    Button::new("notifications-mark-all-read")
                                        .label(cx.lang().get("notifications.mark_all_read"))
                                        .ghost()
                                        .rounded(px(24.0))
                                        .on_click(move |_, _, cx| {
                                            mark_all_model.update(cx, |m, cx| m.mark_all_read(cx));
                                        }),
                                )
                            })
                            .when(!is_empty, |row| {
                                row.child(
                                    Button::new("notifications-clear-all")
                                        .label(cx.lang().get("notifications.clear_all"))
                                        .ghost()
                                        .rounded(px(24.0))
                                        .on_click(move |_, _, cx| {
                                            clear_model.update(cx, |m, cx| m.clear(cx));
                                        }),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .rounded(px(16.0))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .overflow_hidden()
                    .when(is_empty, |panel| {
                        panel.items_center().justify_center().child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .child(
                                    svg()
                                        .path("icons/bell.svg")
                                        .size_8()
                                        .text_color(text_muted),
                                )
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(text_color)
                                        .child(cx.lang().get("notifications.empty_title")),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text_muted)
                                        .child(cx.lang().get("notifications.empty_subtitle")),
                                ),
                        )
                    })
                    .when(!is_empty, |panel| {
                        panel.child(container_query(move |size, _window, cx| {
                            let columns = (((f32::from(size.width) - 42.0 + 16.0) / 312.0).floor()
                                as u16)
                                .max(1);
                            div()
                                .id("notifications-cards-scroll")
                                .size_full()
                                .overflow_y_scroll()
                                .p_5()
                                .child(
                                    div()
                                        .w_full()
                                        .grid()
                                        .grid_cols(columns)
                                        .content_start()
                                        .items_start()
                                        .gap_4()
                                        .children(items.iter().map(|item| {
                                            Self::render_card(
                                                item,
                                                surface,
                                                surface_hover,
                                                border,
                                                text_color,
                                                text_muted,
                                                accent,
                                                completed,
                                                downloading,
                                                error_color,
                                                cx,
                                            )
                                        })),
                                )
                        }))
                    }),
            )
    }
}

impl NotificationsPage {
    #[allow(clippy::too_many_arguments)]
    fn render_card(
        item: &AppNotification,
        surface: Rgba,
        surface_hover: Rgba,
        border: Rgba,
        text_color: Rgba,
        text_muted: Rgba,
        accent: Rgba,
        completed: Rgba,
        downloading: Rgba,
        error_color: Rgba,
        cx: &App,
    ) -> impl IntoElement {
        let notif_id = item.id;
        let notif_key = notif_id.as_u64_pair().0;
        let card_bounds = Rc::new(Cell::new(None));
        let card_bounds_click = Rc::clone(&card_bounds);

        let (badge_color, title, subtitle) = match &item.kind {
            NotificationKind::DownloadStarted { file_name, .. } => (
                downloading,
                cx.lang().get("notifications.download_started"),
                file_name.clone(),
            ),
            NotificationKind::DownloadCompleted { file_name, .. } => (
                completed,
                cx.lang().get("notifications.download_completed"),
                file_name.clone(),
            ),
            NotificationKind::DownloadFailed {
                file_name, error, ..
            } => (
                error_color,
                cx.lang().get("notifications.download_failed"),
                format!("{file_name} — {error}"),
            ),
            NotificationKind::UpdateAvailable { info } => (
                accent,
                cx.lang()
                    .get_with("titlebar.update_available", &[("version", &info.version)]),
                cx.lang().get("notifications.view_changelog").to_string(),
            ),
        };

        let timestamp = DateTime::from_timestamp(item.created_at, 0)
            .map(|dt| dt.with_timezone(&Local).format("%H:%M · %d/%m").to_string())
            .unwrap_or_default();

        download_card_surface(surface, surface_hover, border)
            .id(("notification-card", notif_key))
            .relative()
            .cursor_pointer()
            .w_full()
            .min_w_0()
            .p_4()
            .flex()
            .flex_col()
            .gap_2p5()
            .on_click(move |_, window, cx| {
                NotificationDetailModalState::open(notif_id, card_bounds_click.get(), window, cx);
            })
            .child(track_card_bounds(card_bounds))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .min_w_0()
                            .child(div().size_2().rounded_full().flex_shrink_0().bg(badge_color))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(text_color)
                                    .truncate()
                                    .child(title),
                            ),
                    )
                    .when(!item.read, |row| {
                        row.child(div().size_2().rounded_full().flex_shrink_0().bg(accent))
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(text_muted)
                    .truncate()
                    .child(subtitle),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_1()
                    .text_xs()
                    .text_color(text_muted)
                    .child(timestamp)
                    .child(cx.lang().get("notifications.open_detail")),
            )
    }
}
