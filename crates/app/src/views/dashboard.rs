mod calendar;

use gpui::{prelude::FluentBuilder as _, *};
use rdm_core::task::TaskStatus;
use smallvec::SmallVec;

use crate::{
    components::{ButtonRounded, IconButton, PillButton, ProgressBar},
    state::{CoreContext, DownloadHistory, TaskSnapshot},
    theme::{ActiveTheme, Theme},
    views::{NewDownloadModal, SettingsModalState},
};

use super::downloads::{download_card_surface, format_bytes, task_progress_percent};
use calendar::download_calendar;

const ACTIVITY_LIMIT: usize = 5;

#[derive(Default)]
struct DashboardSummary<'a> {
    speed: u64,
    transferred: u64,
    downloading: usize,
    queued: usize,
    processing: usize,
    failed: usize,
    active: SmallVec<[&'a TaskSnapshot; 16]>,
}

impl<'a> DashboardSummary<'a> {
    fn from_tasks(tasks: &'a [TaskSnapshot]) -> Self {
        let mut summary = Self::default();

        for task in tasks {
            summary.transferred = summary.transferred.saturating_add(task.downloaded_bytes);
            match task.status {
                TaskStatus::Downloading | TaskStatus::Connecting => {
                    summary.downloading += 1;
                    summary.speed = summary.speed.saturating_add(task.speed_bytes_per_sec);
                    summary.active.push(task);
                }
                TaskStatus::Processing { .. } => {
                    summary.processing += 1;
                    summary.active.push(task);
                }
                TaskStatus::Pending | TaskStatus::ConversionQueued => summary.queued += 1,
                TaskStatus::Failed(_) | TaskStatus::ConversionFailed(_) => summary.failed += 1,
                TaskStatus::Paused | TaskStatus::Completed | TaskStatus::Cancelled => {}
            }
        }
        summary.active.sort_unstable_by(|left, right| {
            left.file_name
                .cmp(&right.file_name)
                .then(left.id.cmp(&right.id))
        });
        summary
    }
}

#[derive(IntoElement)]
pub struct DashboardView;

impl DashboardView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DashboardView {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DashboardView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let tasks = cx.tasks_model().read(cx);
        let summary = DashboardSummary::from_tasks(tasks.tasks());
        let speed_limit = cx.core().config().config().max_speed_bytes_per_sec;

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background())
            .text_color(theme.text_color())
            .py_5()
            .child(
                div()
                    .id("dashboard-scroll")
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .w_full()
                            .flex_1()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .px_5()
                            .child(Self::header())
                            .child(Self::metrics(&summary, speed_limit, theme))
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .flex_grow_1()
                                    .flex_shrink_0()
                                    .gap_3()
                                    .child(Self::task_panel(&summary, theme))
                                    .child(Self::transfer_panel(&summary, theme)),
                            )
                            .child(Self::history_panel(tasks.download_history(), theme)),
                    ),
            )
    }
}

impl DashboardView {
    fn header() -> impl IntoElement {
        div()
            .flex()
            .flex_wrap()
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .gap_2()
            .pb_1()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Dashboard"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        PillButton::new("dashboard-all-downloads", "Ver descargas").on_click(
                            |_, window, cx| {
                                gpui_router::use_navigate(window, cx).push("/downloads");
                            },
                        ),
                    )
                    .child(
                        PillButton::new("dashboard-new-download", "Nueva descarga")
                            .selected(true)
                            .on_click(|_, window, cx| {
                                NewDownloadModal::open(None, window, cx);
                            }),
                    ),
            )
    }

    fn panel(theme: &dyn Theme) -> Div {
        download_card_surface(theme.surface(), theme.surface_hover(), theme.border())
            .flex()
            .flex_col()
    }

    fn label(text: &'static str, theme: &dyn Theme) -> Div {
        div()
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.text_muted())
            .child(text)
    }

    fn value(value: impl Into<SharedString>) -> Div {
        div()
            .text_size(px(32.0))
            .font_weight(FontWeight::SEMIBOLD)
            .child(value.into())
    }

    fn metric(theme: &dyn Theme) -> Div {
        Self::panel(theme)
            .flex_1()
            .min_w(px(156.0))
            .min_h(px(112.0))
            .justify_between()
            .gap_2()
            .p_4()
    }

    fn metrics(summary: &DashboardSummary<'_>, speed_limit: u64, theme: &dyn Theme) -> Div {
        let muted = theme.text_muted();
        let speed_limit = if speed_limit == 0 {
            "Sin límite".to_owned()
        } else {
            format!("{}/s", format_bytes(speed_limit))
        };

        div()
            .flex()
            .flex_wrap()
            .flex_grow_1()
            .flex_shrink_0()
            .gap_3()
            .child(
                Self::metric(theme)
                    .child(Self::label("MOTOR", theme))
                    .child(Self::value("Listo"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(concat!("rDM ", env!("CARGO_PKG_VERSION"))),
                            )
                            .child(div().size_2().rounded_full().bg(theme.completed())),
                    ),
            )
            .child(
                Self::metric(theme)
                    .child(Self::label("LÍMITE", theme))
                    .child(Self::value(speed_limit))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_xs().text_color(muted).child("Velocidad global"))
                            .child(
                                IconButton::new("dashboard-speed-settings", "icons/settings.svg")
                                    .size(px(22.0))
                                    .icon_size(px(13.0))
                                    .rounded(ButtonRounded::Size(px(5.0)))
                                    .color(muted)
                                    .hover_bg(theme.surface_hover())
                                    .tooltip("Ajustar límite de velocidad")
                                    .on_click(|_, window, cx| {
                                        cx.set_global(SettingsModalState {
                                            open: true,
                                            ..Default::default()
                                        });
                                        window.refresh();
                                    }),
                            ),
                    ),
            )
            .child(
                Self::metric(theme)
                    .child(Self::label("DESCARGA", theme))
                    .child(Self::value(format!("{}/s", format_bytes(summary.speed))))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .h(px(2.0))
                                    .w(px(14.0))
                                    .rounded_full()
                                    .bg(theme.accent()),
                            )
                            .child(div().text_xs().text_color(muted).child("Velocidad actual")),
                    ),
            )
            .child(
                Self::metric(theme)
                    .child(Self::label("ERRORES", theme))
                    .child(
                        Self::value(summary.failed.to_string())
                            .when(summary.failed > 0, |value| value.text_color(theme.error())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(if summary.failed > 0 {
                                "Tareas con error"
                            } else {
                                "Sin errores"
                            }),
                    ),
            )
    }

    fn task_panel(summary: &DashboardSummary<'_>, theme: &dyn Theme) -> Div {
        let counts = [
            ("En descarga", summary.downloading),
            ("En cola", summary.queued),
            ("Procesando", summary.processing),
        ];
        let hidden_count = summary.active.len().saturating_sub(ACTIVITY_LIMIT);
        Self::panel(theme)
            .flex_1()
            .min_w(px(265.0))
            .justify_between()
            .gap_3()
            .p_4()
            .child(Self::label("TAREAS ACTIVAS", theme))
            .child(Self::value(summary.active.len().to_string()))
            .child(div().flex().gap_3().children(counts.map(|(label, count)| {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(count.to_string()),
                    )
                    .child(div().text_xs().text_color(theme.text_muted()).child(label))
            })))
            .when(!summary.active.is_empty(), |panel| {
                panel.child(
                    div()
                        .id("dashboard-active-tasks")
                        .flex_1()
                        .min_h(px(54.0))
                        .max_h(px(260.0))
                        .overflow_y_scroll()
                        .children(
                            summary
                                .active
                                .iter()
                                .take(ACTIVITY_LIMIT)
                                .map(|task| Self::activity_row(task, theme)),
                        )
                        .when(hidden_count > 0, |list| {
                            list.child(
                                div()
                                    .py_2()
                                    .text_xs()
                                    .text_color(theme.text_muted())
                                    .child(format!("+{hidden_count} en Descargas")),
                            )
                        }),
                )
            })
    }

    fn transfer_panel(summary: &DashboardSummary<'_>, theme: &dyn Theme) -> Div {
        Self::panel(theme)
            .flex_1()
            .min_w(px(265.0))
            .justify_between()
            .gap_3()
            .p_4()
            .child(Self::label("TRANSFERENCIA", theme))
            .child(Self::value(format_bytes(summary.transferred)))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted())
                    .child("Datos descargados en las tareas del historial"),
            )
    }

    fn history_panel(history: Option<&DownloadHistory>, theme: &dyn Theme) -> Div {
        Self::panel(theme)
            .flex_shrink_0()
            .px_4()
            .py_3()
            .when_some(history, |panel, history| {
                panel.child(download_calendar(history, theme))
            })
    }

    fn activity_row(task: &TaskSnapshot, theme: &dyn Theme) -> Div {
        let muted = theme.text_muted();
        let progress = task_progress_percent(task);
        let status = match task.status {
            TaskStatus::Connecting => "Conectando".to_owned(),
            TaskStatus::Processing { .. } => "Procesando".to_owned(),
            _ => format!("{}/s", format_bytes(task.speed_bytes_per_sec)),
        };
        let show_percent = task.total_size > 0 && matches!(task.status, TaskStatus::Downloading);

        div()
            .border_t_1()
            .border_color(theme.border())
            .py_2()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(task.file_name.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(muted)
                            .child(status),
                    ),
            )
            .when(
                task.total_size > 0 || matches!(task.status, TaskStatus::Processing { .. }),
                |row| {
                    row.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div().flex_1().child(
                                    ProgressBar::new(
                                        ("dashboard-progress", task.id.as_u64_pair().0),
                                        progress,
                                    )
                                    .height(px(4.0))
                                    .color(theme.accent()),
                                ),
                            )
                            .when(show_percent, |progress_row| {
                                progress_row.child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(format!("{progress:.0}%")),
                                )
                            }),
                    )
                },
            )
    }
}
