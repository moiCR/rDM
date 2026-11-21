use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::component::button::ButtonRounded;
use rdm_core::task::{SegmentStatus, TaskStatus};
use uuid::Uuid;

use crate::{
    components::{
        Button, ButtonVariant, ButtonVariants, IconButton, Modal, ProgressBar,
        download_card::{format_bytes, task_progress_percent},
    },
    lang::ActiveLang,
    modals::DeleteDownloadModalState,
    state::{CoreContext, TaskSnapshot},
    theme::ActiveTheme,
};

#[derive(Default, Clone)]
pub struct DownloadInspectorModalState {
    pub open: bool,
    pub task_id: Option<Uuid>,
    pub origin: Option<Bounds<Pixels>>,
}

impl Global for DownloadInspectorModalState {}

impl DownloadInspectorModalState {
    pub fn open(
        task_id: Uuid,
        origin: Option<Bounds<Pixels>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        cx.set_global(Self {
            open: true,
            task_id: Some(task_id),
            origin,
        });
        window.refresh();
    }

    pub fn close(window: &mut Window, cx: &mut App) {
        let origin = cx
            .try_global::<Self>()
            .and_then(|state| state.origin);
        let task_id = cx
            .try_global::<Self>()
            .and_then(|state| state.task_id);
        cx.set_global(Self {
            open: false,
            task_id,
            origin,
        });
        window.refresh();
    }
}

#[derive(IntoElement)]
pub struct DownloadInspectorModal;

impl DownloadInspectorModal {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DownloadInspectorModal {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DownloadInspectorModal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let modal_state = cx
            .try_global::<DownloadInspectorModalState>()
            .cloned()
            .unwrap_or_default();

        let tasks_model = cx.tasks_model().clone();
        let task = modal_state
            .task_id
            .and_then(|id| tasks_model.read(cx).task(id).cloned());
        let is_open = modal_state.open && task.is_some();

        let theme = cx.theme();
        let background = theme.background();
        let surface_hover = theme.surface_hover();
        let border = theme.border();
        let text_color = theme.text_color();
        let text_muted = theme.text_muted();
        let accent = theme.accent();
        let downloading = theme.downloading();
        let paused = theme.paused();
        let completed = theme.completed();
        let error_color = theme.error();

        let (title, body, footer) = if let Some(task) = task {
            let task_id = task.id;
            let file_name = task.file_name.clone();
            let percent = task_progress_percent(&task);
            let (status_key, status_color) = match &task.status {
                TaskStatus::Downloading => ("status.downloading", downloading),
                TaskStatus::Paused => ("status.paused", paused),
                TaskStatus::Completed => ("status.completed", completed),
                TaskStatus::Failed(_) => ("status.failed", error_color),
                TaskStatus::ConversionFailed(_) => ("status.conversion_failed", error_color),
                TaskStatus::Processing { .. } => ("status.processing", downloading),
                TaskStatus::Connecting => ("status.connecting", downloading),
                TaskStatus::Pending | TaskStatus::ConversionQueued => ("status.queued", text_muted),
                TaskStatus::Cancelled => ("status.cancelled", text_muted),
            };
            let error_detail = match &task.status {
                TaskStatus::Failed(reason) | TaskStatus::ConversionFailed(reason) => {
                    Some(reason.clone())
                }
                _ => None,
            };

            let speed_text = if task.status == TaskStatus::Downloading {
                format!("{}/s", format_bytes(task.speed_bytes_per_sec))
            } else {
                "—".to_string()
            };
            let size_text = if task.total_size > 0 {
                format!(
                    "{} / {}",
                    format_bytes(task.downloaded_bytes),
                    format_bytes(task.total_size)
                )
            } else {
                format_bytes(task.downloaded_bytes)
            };
            let limit_text = if task.max_speed_bytes_per_sec == 0 {
                cx.lang().get("dashboard.unlimited").to_string()
            } else {
                format!("{}/s", format_bytes(task.max_speed_bytes_per_sec))
            };
            let checksum_text = task
                .checksum
                .as_ref()
                .map(|c| format!("{}:{}", c.algorithm.as_str(), c.value));
            let url_copy = task.url.clone();
            let path_str = task.output_path.to_string_lossy().to_string();

            let toggle_action = match task.status {
                TaskStatus::Downloading | TaskStatus::Connecting => {
                    Some((true, cx.lang().get("downloads.pause")))
                }
                TaskStatus::Paused | TaskStatus::Pending | TaskStatus::Failed(_) => {
                    Some((false, cx.lang().get("downloads.resume")))
                }
                _ => None,
            };

            let segments_section = (!task.segments.is_empty()).then(|| {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(text_muted)
                                    .child(cx.lang().get("inspector.segments")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text_muted)
                                    .child(format!("{}", task.segments.len())),
                            ),
                    )
                    .child(div().flex().gap_1().w_full().children(task.segments.iter().map(
                        |segment| {
                            let seg_color = match segment.status {
                                SegmentStatus::Completed => completed,
                                SegmentStatus::Downloading => downloading,
                                SegmentStatus::Failed => error_color,
                                SegmentStatus::Pending => text_muted,
                            };
                            div().flex_1().min_w_0().child(
                                ProgressBar::new(
                                    ("inspector-seg", (task_id.as_u64_pair().0, segment.index as u64)),
                                    segment.progress_percent(),
                                )
                                .height(px(6.0))
                                .color(seg_color)
                                .track_color(background),
                            )
                        },
                    )))
            });

            let detail_row = |label: SharedString, value: String| {
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap_4()
                    .py_1p5()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text_muted)
                            .child(label),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .text_xs()
                            .text_color(text_color)
                            .truncate()
                            .child(value),
                    )
            };

            let body_el = div()
                .id("inspector-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_4()
                        .rounded(px(16.0))
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
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(status_color)
                                        .child(cx.lang().get(status_key)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(text_color)
                                        .child(format!("{percent:.1}%")),
                                ),
                        )
                        .child(
                            ProgressBar::new(("inspector-total", task_id.as_u64_pair().0), percent)
                                .height(px(6.0))
                                .color(status_color)
                                .track_color(surface_hover),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .text_xs()
                                .text_color(text_muted)
                                .child(size_text)
                                .child(speed_text),
                        ),
                )
                .children(segments_section)
                .when_some(error_detail, |container, reason| {
                    let reason_copy = reason.clone();
                    container.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_3()
                            .rounded(px(12.0))
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
                                        IconButton::new("inspector-copy-error", "icons/copy.svg")
                                            .size(px(24.0))
                                            .icon_size(px(12.0))
                                            .rounded(ButtonRounded::Size(px(99.0)))
                                            .color(error_color)
                                            .hover_bg(error_color.opacity(0.15))
                                            .on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    reason_copy.clone(),
                                                ));
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text_color)
                                    .child(reason),
                            ),
                    )
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .py_1p5()
                                .border_b_1()
                                .border_color(border)
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(text_muted)
                                        .child(cx.lang().get("inspector.url")),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .min_w_0()
                                                .text_xs()
                                                .text_color(accent)
                                                .truncate()
                                                .child(task.url.clone()),
                                        )
                                        .child(
                                            IconButton::new("inspector-copy-url", "icons/copy.svg")
                                                .size(px(24.0))
                                                .icon_size(px(12.0))
                                                .rounded(ButtonRounded::Size(px(99.0)))
                                                .color(text_muted)
                                                .hover_bg(surface_hover)
                                                .on_click(move |_, _, cx| {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(url_copy.clone()),
                                                    );
                                                }),
                                        ),
                                ),
                        )
                        .child(detail_row(cx.lang().get("inspector.path"), path_str))
                        .child(detail_row(
                            cx.lang().get("inspector.connections"),
                            task.connections.to_string(),
                        ))
                        .child(detail_row(
                            cx.lang().get("inspector.speed_limit"),
                            limit_text,
                        ))
                        .when_some(checksum_text, |list, checksum| {
                            list.child(detail_row(cx.lang().get("inspector.checksum"), checksum))
                        })
                        .when_some(task.referer.clone(), |list, referer| {
                            list.child(detail_row(cx.lang().get("inspector.referer"), referer))
                        }),
                )
                .into_any_element();

            let is_completed = task.status == TaskStatus::Completed;
            let can_delete = !matches!(task.status, TaskStatus::Processing { .. });
            let is_conversion_failed = matches!(task.status, TaskStatus::ConversionFailed(_));
            let file_name_delete = file_name.clone();

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
                        .when(is_completed, |row| {
                            let open_model = tasks_model.clone();
                            let folder_model = tasks_model.clone();
                            row.child(
                                Button::new("inspector-open-file")
                                    .label(cx.lang().get("downloads.open_file"))
                                    .variant(ButtonVariant::Secondary)
                                    .rounded(px(24.0))
                                    .on_click(move |_, _, cx| {
                                        open_model.read(cx).open_file(task_id);
                                    }),
                            )
                            .child(
                                Button::new("inspector-open-folder")
                                    .label(cx.lang().get("downloads.open_folder"))
                                    .variant(ButtonVariant::Ghost)
                                    .rounded(px(24.0))
                                    .on_click(move |_, _, cx| {
                                        folder_model.read(cx).open_folder(task_id);
                                    }),
                            )
                        })
                        .when(is_conversion_failed, |row| {
                            let retry_model = tasks_model.clone();
                            row.child(
                                Button::new("inspector-retry-conversion")
                                    .label(cx.lang().get("downloads.retry_conversion"))
                                    .variant(ButtonVariant::Primary)
                                    .rounded(px(24.0))
                                    .on_click(move |_, _, cx| {
                                        retry_model.update(cx, |m, cx| m.retry_conversion(task_id, cx));
                                    }),
                            )
                        })
                        .when_some(toggle_action, |row, (is_running, label)| {
                            let toggle_model = tasks_model.clone();
                            row.child(
                                Button::new("inspector-toggle-task")
                                    .label(label)
                                    .variant(ButtonVariant::Primary)
                                    .rounded(px(24.0))
                                    .on_click(move |_, _, cx| {
                                        toggle_model.update(cx, |m, cx| {
                                            if is_running {
                                                m.pause_task(task_id, cx);
                                            } else {
                                                m.resume_task(task_id, cx);
                                            }
                                        });
                                    }),
                            )
                        }),
                )
                .when(can_delete, |row| {
                    row.child(
                        Button::new("inspector-delete-task")
                            .label(cx.lang().get("common.delete"))
                            .variant(ButtonVariant::Ghost)
                            .rounded(px(24.0))
                            .on_click(move |_, window, cx| {
                                DownloadInspectorModalState::close(window, cx);
                                cx.set_global(DeleteDownloadModalState {
                                    open: true,
                                    task_id: Some(task_id),
                                    file_name: file_name_delete.clone(),
                                    delete_file: false,
                                });
                                window.refresh();
                            }),
                    )
                })
                .into_any_element();

            (SharedString::from(file_name), Some(body_el), Some(footer_el))
        } else {
            (cx.lang().get("inspector.title"), None, None)
        };

        div()
            .id("download-inspector-modal-root")
            .absolute()
            .inset_0()
            .size_full()
            .child(
                Modal::new("download-inspector-modal")
                    .open(is_open)
                    .from_bounds(modal_state.origin)
                    .width(px(640.0))
                    .max_height(px(580.0))
                    .title(title)
                    .on_close(|window, cx| {
                        DownloadInspectorModalState::close(window, cx);
                    })
                    .when_some(footer, |modal, footer| modal.footer(footer))
                    .children(body),
            )
    }
}
