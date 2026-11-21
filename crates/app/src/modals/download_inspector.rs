use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::base::{Scrollbar, ScrollbarMode};
use rdm_core::task::{SegmentStatus, TaskStatus};
use uuid::Uuid;

use crate::{
    components::{
        Button, ButtonVariants, IconButton, Modal, ProgressBar,
        download_card::{format_bytes, task_progress_percent, truncate_file_name},
    },
    lang::ActiveLang,
    modals::DeleteDownloadModalState,
    state::CoreContext,
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
    pub fn open(task_id: Uuid, origin: Option<Bounds<Pixels>>, window: &mut Window, cx: &mut App) {
        cx.set_global(Self {
            open: true,
            task_id: Some(task_id),
            origin,
        });
        window.refresh();
    }

    pub fn close(window: &mut Window, cx: &mut App) {
        let origin = cx.try_global::<Self>().and_then(|state| state.origin);
        let task_id = cx.try_global::<Self>().and_then(|state| state.task_id);
        cx.set_global(Self {
            open: false,
            task_id,
            origin,
        });
        window.blur(cx);
        window.refresh();
    }
}

#[derive(Clone, Copy)]
struct InspectorPalette {
    surface: Rgba,
    card_surface: Rgba,
    surface_hover: Rgba,
    border: Rgba,
    text_color: Rgba,
    text_muted: Rgba,
    accent: Rgba,
    downloading: Rgba,
    paused: Rgba,
    completed: Rgba,
    error: Rgba,
}

#[derive(IntoElement)]
pub struct DownloadInspectorModal;

impl DownloadInspectorModal {
    pub fn new() -> Self {
        Self
    }

    fn card(palette: InspectorPalette) -> Div {
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

    fn divider(palette: InspectorPalette) -> Div {
        div().h(px(1.0)).w_full().bg(palette.border).opacity(0.6)
    }

    fn detail_row(label: SharedString, value: String, palette: InspectorPalette) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .py_2()
            .child(
                div()
                    .flex_shrink_0()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.text_muted)
                    .child(label),
            )
            .child(
                div()
                    .min_w_0()
                    .text_xs()
                    .text_color(palette.text_color)
                    .truncate()
                    .child(value),
            )
    }

    fn render_header(
        title: SharedString,
        status_color: Option<Rgba>,
        palette: InspectorPalette,
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
                IconButton::new("btn-close-download-inspector", "icons/x.svg")
                    .size(px(32.0))
                    .rounded(px(24.0))
                    .color(palette.text_muted)
                    .hover_bg(palette.surface_hover)
                    .tooltip(cx.lang().get("common.close"))
                    .on_click(|_, window, cx| DownloadInspectorModalState::close(window, cx)),
            )
    }
}

impl Default for DownloadInspectorModal {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DownloadInspectorModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
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
        let palette = InspectorPalette {
            surface: theme.surface(),
            card_surface: theme
                .input_background()
                .unwrap_or_else(|| theme.background()),
            surface_hover: theme.surface_hover(),
            border: theme.border(),
            text_color: theme.text_color(),
            text_muted: theme.text_muted(),
            accent: theme.accent(),
            downloading: theme.downloading(),
            paused: theme.paused(),
            completed: theme.completed(),
            error: theme.error(),
        };
        let viewport = window.viewport_size();
        let scroll_handle = window
            .use_keyed_state("download-inspector-scroll-handle", cx, |_, _| {
                ScrollHandle::new()
            })
            .read(cx)
            .clone();

        let modal_h = match task.as_ref() {
            Some(task) => {
                let mut h = 408.0_f32;
                if !task.segments.is_empty() {
                    h += 54.0;
                }
                if matches!(
                    task.status,
                    TaskStatus::Failed(_) | TaskStatus::ConversionFailed(_)
                ) {
                    h += 104.0;
                }
                if task.checksum.is_some() {
                    h += 38.0;
                }
                if task.referer.is_some() {
                    h += 38.0;
                }
                px(h.min(580.0))
            }
            None => px(408.0),
        };

        let (title, status_color, body, footer) = if let Some(task) = task {
            let task_id = task.id;
            let file_name = task.file_name.clone();
            let percent = task_progress_percent(&task);
            let (status_key, status_color) = match &task.status {
                TaskStatus::Downloading => ("status.downloading", palette.downloading),
                TaskStatus::Paused => ("status.paused", palette.paused),
                TaskStatus::Completed => ("status.completed", palette.completed),
                TaskStatus::Failed(_) => ("status.failed", palette.error),
                TaskStatus::ConversionFailed(_) => ("status.conversion_failed", palette.error),
                TaskStatus::Processing { .. } => ("status.processing", palette.downloading),
                TaskStatus::Connecting => ("status.connecting", palette.downloading),
                TaskStatus::Pending | TaskStatus::ConversionQueued => {
                    ("status.queued", palette.text_muted)
                }
                TaskStatus::Cancelled => ("status.cancelled", palette.text_muted),
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

            let progress_card = Self::card(palette)
                .gap_2p5()
                .child(
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
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(status_color)
                                        .child(cx.lang().get(status_key)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.text_color)
                                        .child(format!("{percent:.1}%")),
                                ),
                        )
                        .child(
                            ProgressBar::new(("inspector-total", task_id.as_u64_pair().0), percent)
                                .height(px(6.0))
                                .color(status_color)
                                .track_color(palette.surface),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .text_xs()
                                .text_color(palette.text_muted)
                                .child(size_text)
                                .child(speed_text),
                        ),
                )
                .when(!task.segments.is_empty(), |card| {
                    card.child(Self::divider(palette)).child(
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
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(palette.text_muted)
                                            .child(cx.lang().get("inspector.segments")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(palette.text_muted)
                                            .child(format!("{}", task.segments.len())),
                                    ),
                            )
                            .child(div().flex().gap_1().w_full().children(
                                task.segments.iter().map(|segment| {
                                    let seg_color = match segment.status {
                                        SegmentStatus::Completed => palette.completed,
                                        SegmentStatus::Downloading => palette.downloading,
                                        SegmentStatus::Failed => palette.error,
                                        SegmentStatus::Pending => palette.text_muted,
                                    };
                                    div().flex_1().min_w_0().child(
                                        ProgressBar::new(
                                            (
                                                "inspector-seg",
                                                task_id
                                                    .as_u64_pair()
                                                    .0
                                                    .wrapping_add(segment.index as u64),
                                            ),
                                            segment.progress_percent(),
                                        )
                                        .height(px(6.0))
                                        .color(seg_color)
                                        .track_color(palette.surface),
                                    )
                                }),
                            )),
                    )
                });

            let details_card = Self::card(palette)
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
                                .flex_shrink_0()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(palette.text_muted)
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
                                        .text_color(palette.accent)
                                        .truncate()
                                        .child(task.url.clone()),
                                )
                                .child(
                                    IconButton::new("inspector-copy-url", "icons/copy.svg")
                                        .size(px(24.0))
                                        .icon_size(px(12.0))
                                        .rounded(px(20.0))
                                        .color(palette.text_muted)
                                        .hover_bg(palette.surface_hover)
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                url_copy.clone(),
                                            ));
                                        }),
                                ),
                        ),
                )
                .child(Self::divider(palette))
                .child(Self::detail_row(
                    cx.lang().get("inspector.path"),
                    path_str,
                    palette,
                ))
                .child(Self::divider(palette))
                .child(Self::detail_row(
                    cx.lang().get("inspector.connections"),
                    task.connections.to_string(),
                    palette,
                ))
                .child(Self::divider(palette))
                .child(Self::detail_row(
                    cx.lang().get("inspector.speed_limit"),
                    limit_text,
                    palette,
                ))
                .when_some(checksum_text, |list, checksum| {
                    list.child(Self::divider(palette)).child(Self::detail_row(
                        cx.lang().get("inspector.checksum"),
                        checksum,
                        palette,
                    ))
                })
                .when_some(task.referer.clone(), |list, referer| {
                    list.child(Self::divider(palette)).child(Self::detail_row(
                        cx.lang().get("inspector.referer"),
                        referer,
                        palette,
                    ))
                });

            let body_el = div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("inspector-scroll")
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&scroll_handle)
                        .pr_2()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(progress_card)
                                .when_some(error_detail, |container, reason| {
                                    let reason_copy = reason.clone();
                                    container.child(
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
                                                            "inspector-copy-error",
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
                                                                    reason_copy.clone(),
                                                                ),
                                                            );
                                                        }),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(palette.text_color)
                                                    .child(reason),
                                            ),
                                    )
                                })
                                .child(details_card),
                        ),
                )
                .child(Scrollbar::vertical(&scroll_handle).mode(ScrollbarMode::Always))
                .into_any_element();

            let is_completed = task.status == TaskStatus::Completed;
            let can_delete = !matches!(task.status, TaskStatus::Processing { .. });
            let is_conversion_failed = matches!(task.status, TaskStatus::ConversionFailed(_));
            let file_name_delete = file_name.clone();

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
                        .when(is_completed, |row| {
                            let file_to_open = task.output_path.clone();
                            let file_to_reveal = task.output_path.clone();
                            row.child(
                                Button::new("inspector-open-file")
                                    .label(cx.lang().get("downloads.open_file"))
                                    .primary()
                                    .rounded(px(24.0))
                                    .h(px(32.0))
                                    .on_click(move |_, _, cx| {
                                        cx.open_with_system(&file_to_open);
                                    }),
                            )
                            .child(
                                Button::new("inspector-open-folder")
                                    .label(cx.lang().get("downloads.reveal_file"))
                                    .rounded(px(24.0))
                                    .h(px(32.0))
                                    .on_click(move |_, _, cx| {
                                        cx.reveal_path(&file_to_reveal);
                                    }),
                            )
                        })
                        .when(is_conversion_failed, |row| {
                            let retry_model = tasks_model.clone();
                            row.child(
                                Button::new("inspector-retry-conversion")
                                    .label(cx.lang().get("downloads.retry_conversion"))
                                    .primary()
                                    .rounded(px(24.0))
                                    .h(px(32.0))
                                    .on_click(move |_, _, cx| {
                                        retry_model
                                            .update(cx, |m, cx| m.retry_conversion(task_id, cx));
                                    }),
                            )
                        })
                        .when_some(toggle_action, |row, (is_running, label)| {
                            let toggle_model = tasks_model.clone();
                            row.child(
                                Button::new("inspector-toggle-task")
                                    .label(label)
                                    .primary()
                                    .rounded(px(24.0))
                                    .h(px(32.0))
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
                            .danger()
                            .rounded(px(24.0))
                            .h(px(32.0))
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

            (
                SharedString::from(truncate_file_name(&file_name, 36)),
                Some(status_color),
                Some(body_el),
                Some(footer_el),
            )
        } else {
            (cx.lang().get("inspector.title"), None, None, None)
        };

        div()
            .id("download-inspector-modal-root")
            .absolute()
            .inset_0()
            .size_full()
            .capture_key_down(move |event: &KeyDownEvent, window, cx| {
                if is_open && event.keystroke.key == "escape" {
                    DownloadInspectorModalState::close(window, cx);
                    cx.stop_propagation();
                }
            })
            .child(
                Modal::new("download-inspector-modal")
                    .open(is_open)
                    .compact()
                    .from_bounds(modal_state.origin)
                    .width((viewport.width - px(32.0)).clamp(px(0.0), px(580.0)))
                    .max_height((viewport.height - px(32.0)).clamp(px(0.0), modal_h))
                    .header(Self::render_header(title, status_color, palette, cx))
                    .on_close(|window, cx| {
                        DownloadInspectorModalState::close(window, cx);
                    })
                    .when_some(footer, |modal, footer| modal.footer(footer))
                    .children(body),
            )
    }
}
