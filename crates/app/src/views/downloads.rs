use std::{f32::consts::TAU, time::Duration};

use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::component::button::ButtonRounded;
use rdm_core::task::TaskStatus;
use uuid::Uuid;

use crate::{
    components::{ActionTooltip, IconButton, Modal, PillButton, ProgressBar},
    state::{CoreContext, TaskSnapshot, TasksModel},
    theme::ActiveTheme,
    views::NewDownloadModal,
};

#[derive(Default, Clone)]
pub struct DeleteDownloadModalState {
    pub open: bool,
    pub task_id: Option<Uuid>,
    pub file_name: String,
    pub delete_file: bool,
}

impl Global for DeleteDownloadModalState {}

#[derive(Clone, Copy)]
struct ViewPalette {
    background: Rgba,
    surface: Rgba,
    surface_hover: Rgba,
    border: Rgba,
    text_color: Rgba,
    text_muted: Rgba,
    downloading: Rgba,
    paused: Rgba,
    completed: Rgba,
    error: Rgba,
}

impl ViewPalette {
    fn from_theme(cx: &App) -> Self {
        let theme = cx.theme();
        Self {
            background: theme.background(),
            surface: theme.surface(),
            surface_hover: theme.surface_hover(),
            border: theme.border(),
            text_color: theme.text_color(),
            text_muted: theme.text_muted(),
            downloading: theme.downloading(),
            paused: theme.paused(),
            completed: theme.completed(),
            error: theme.error(),
        }
    }

    fn status(self, status: &TaskStatus) -> (&'static str, Rgba) {
        match status {
            TaskStatus::Downloading => ("Descargando", self.downloading),
            TaskStatus::Paused => ("Pausado", self.paused),
            TaskStatus::Completed => ("Completado", self.completed),
            TaskStatus::Failed(_) => ("Error", self.error),
            TaskStatus::ConversionFailed(_) => ("Error de conversión", self.error),
            TaskStatus::Processing { .. } => ("Procesando", self.downloading),
            TaskStatus::Connecting => ("Conectando", self.downloading),
            TaskStatus::Pending | TaskStatus::ConversionQueued => ("En cola", self.text_muted),
            TaskStatus::Cancelled => ("Cancelado", self.text_muted),
        }
    }
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

fn truncate_file_name(name: &str, max_chars: usize) -> String {
    let char_count = name.chars().count();
    if char_count <= max_chars {
        name.to_string()
    } else {
        let keep = max_chars.saturating_sub(3);
        let prefix: String = name.chars().take(keep).collect();
        format!("{prefix}...")
    }
}

pub(super) fn task_progress_percent(task: &TaskSnapshot) -> f32 {
    match (task.total_size, &task.status) {
        (
            _,
            TaskStatus::Completed
            | TaskStatus::Processing { .. }
            | TaskStatus::ConversionFailed(_)
            | TaskStatus::ConversionQueued,
        ) => 100.0,
        (0, _) => 0.0,
        _ => (task.progress.clamp(0.0, 1.0) * 100.0) as f32,
    }
}

pub(super) fn download_card_surface(surface: Rgba, raised: Rgba, border: Rgba) -> Div {
    div()
        .rounded(px(24.0))
        .bg(linear_gradient(
            180.0,
            linear_color_stop(raised, 0.0),
            linear_color_stop(surface, 1.0),
        ))
        .border_1()
        .border_color(border)
        .shadow_md()
}

fn format_remaining(task: &TaskSnapshot) -> SharedString {
    let remaining = task.total_size.saturating_sub(task.downloaded_bytes);
    let speed = task.speed_bytes_per_sec;
    if remaining == 0 || speed == 0 {
        return "Calculando...".into();
    }

    let seconds = remaining.div_ceil(speed);
    match seconds {
        0..60 => format!("{seconds} s"),
        60..3600 => format!("{} min {} s", seconds / 60, seconds % 60),
        3600..86400 => format!("{} h {} min", seconds / 3600, (seconds / 60) % 60),
        _ => format!("{} d {} h", seconds / 86400, (seconds / 3600) % 24),
    }
    .into()
}

#[derive(IntoElement)]
pub struct DownloadsView;

impl DownloadsView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DownloadsView {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DownloadsView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = ViewPalette::from_theme(cx);
        let tasks_model = cx.tasks_model().clone();
        let tasks = tasks_model.read(cx).tasks().to_vec();
        let is_empty = tasks.is_empty();
        let transition = SpringAnimation::new(SpringConfig::new(300.0, 34.0, 1.0)).to(!is_empty);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(palette.background)
            .text_color(palette.text_color)
            .p_5()
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .rounded(px(16.0))
                    .bg(palette.surface)
                    .border_1()
                    .border_color(palette.border)
                    .overflow_hidden()
                    .child(Self::render_glow(palette.surface))
                    .with_spring("downloads-panel-state", transition, move |panel, phase| {
                        let grid_opacity = phase.interpolate_clamped(0.0, 1.0);

                        panel
                            .when(!is_empty, |panel| {
                                panel.child(
                                    div()
                                        .id("downloads-cards-scroll")
                                        .size_full()
                                        .overflow_y_scroll()
                                        .p_5()
                                        .opacity(grid_opacity)
                                        .child(
                                            div()
                                                .w_full()
                                                .flex()
                                                .flex_wrap()
                                                .content_start()
                                                .items_start()
                                                .gap_4()
                                                .children(tasks.iter().map(|task| {
                                                    Self::render_card(task, &tasks_model, palette)
                                                })),
                                        ),
                                )
                            })
                            .when(is_empty || grid_opacity < 0.99, |panel| {
                                panel.child(
                                    Self::render_empty(palette, is_empty)
                                        .opacity(1.0 - grid_opacity),
                                )
                            })
                    }),
            )
    }
}

impl DownloadsView {
    fn render_card(
        task: &TaskSnapshot,
        tasks_model: &Entity<TasksModel>,
        palette: ViewPalette,
    ) -> impl IntoElement {
        let task_id = task.id;
        let task_key = task_id.as_u64_pair().0;
        let display_name = truncate_file_name(&task.file_name, 32);
        let size_text = if task.total_size > 0 {
            format_bytes(task.total_size)
        } else {
            format_bytes(task.downloaded_bytes)
        };
        let header_line = format!("{display_name}  •  {size_text}");
        let (status_label, status_color) = palette.status(&task.status);
        let percent = task_progress_percent(task);
        let percent_text = format!("{}%", percent as u32);
        let (bottom_left_text, bottom_left_color): (SharedString, Rgba) =
            if task.status == TaskStatus::Downloading {
                (format_remaining(task), palette.text_muted)
            } else {
                (status_label.into(), status_color)
            };
        let url_str = task.url.clone();
        let url_for_copy = task.url.clone();
        let file_name_for_modal = task.file_name.clone();
        let conversion_error = match &task.status {
            TaskStatus::ConversionFailed(reason) => Some(SharedString::from(reason.clone())),
            _ => None,
        };
        let can_delete = !matches!(task.status, TaskStatus::Processing { .. });
        let retry_conversion = conversion_error.as_ref().map(|_| {
            let model = tasks_model.clone();
            PillButton::new(
                ("download-card-retry-conversion", task_key),
                "Reintentar conversión",
            )
            .on_click(move |_event, _window, cx| {
                model.update(cx, |m, cx| m.retry_conversion(task_id, cx));
            })
        });

        let toggle_action = match task.status {
            TaskStatus::Downloading | TaskStatus::Connecting => {
                Some((true, "icons/pause.svg", "Pausar descarga"))
            }
            TaskStatus::Paused | TaskStatus::Pending | TaskStatus::Failed(_) => {
                Some((false, "icons/resume.svg", "Reanudar descarga"))
            }
            _ => None,
        };

        download_card_surface(palette.surface, palette.surface_hover, palette.border)
            .id(("download-card", task_key))
            .w(px(296.0))
            .p_4()
            .flex()
            .flex_col()
            .justify_between()
            .gap_3p5()
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_1p5()
                    .child(
                        div()
                            .w_full()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.text_color)
                            .truncate()
                            .child(header_line),
                    )
                    .child(
                        div()
                            .id(("download-card-url", task_key))
                            .w_full()
                            .text_xs()
                            .text_color(palette.text_muted)
                            .truncate()
                            .cursor_pointer()
                            .hover(move |s| s.text_color(palette.text_color).underline())
                            .tooltip(|_, cx| {
                                cx.new(|_| ActionTooltip::new("Clic para copiar URL"))
                                    .into()
                            })
                            .on_click(move |_event, _window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    url_for_copy.clone(),
                                ));
                            })
                            .child(url_str),
                    )
                    .child(
                        div()
                            .w_full()
                            .pt_1()
                            .flex()
                            .items_center()
                            .gap_2p5()
                            .child(
                                div().flex_1().child(
                                    ProgressBar::new(("download-card-progress", task_key), percent)
                                        .height(px(4.0))
                                        .color(status_color)
                                        .track_color(palette.background),
                                ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.text_muted)
                                    .child(percent_text),
                            ),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .id(("download-card-status", task_key))
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(bottom_left_color)
                            .truncate()
                            .when_some(conversion_error, |status, reason| {
                                status.tooltip(move |_, cx| {
                                    cx.new(|_| ActionTooltip::new(reason.clone())).into()
                                })
                            })
                            .child(bottom_left_text),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_1()
                            .children(retry_conversion)
                            .when(task.status == TaskStatus::Completed, |actions| {
                                let file_button = |id, icon, tooltip| {
                                    IconButton::new((id, task_key), icon)
                                        .size(px(30.0))
                                        .icon_size(px(14.0))
                                        .rounded(ButtonRounded::Size(px(99.0)))
                                        .color(palette.text_muted)
                                        .hover_bg(palette.surface_hover)
                                        .tooltip(tooltip)
                                };
                                let file_to_open = task.output_path.clone();
                                let file_to_reveal = task.output_path.clone();
                                actions
                                    .child(
                                        file_button(
                                            "download-card-open-file",
                                            "icons/external-link.svg",
                                            "Abrir archivo",
                                        )
                                        .on_click(
                                            move |_, _, cx| {
                                                cx.open_with_system(&file_to_open);
                                            },
                                        ),
                                    )
                                    .child(
                                        file_button(
                                            "download-card-reveal-file",
                                            "icons/folder-open.svg",
                                            "Mostrar en carpeta",
                                        )
                                        .on_click(
                                            move |_, _, cx| {
                                                cx.reveal_path(&file_to_reveal);
                                            },
                                        ),
                                    )
                            })
                            .children(toggle_action.map(|(is_pause, icon_path, tooltip_text)| {
                                let model = tasks_model.clone();
                                IconButton::new(("download-card-toggle", task_key), icon_path)
                                    .size(px(30.0))
                                    .icon_size(px(14.0))
                                    .rounded(ButtonRounded::Size(px(99.0)))
                                    .color(palette.text_muted)
                                    .hover_bg(palette.surface_hover)
                                    .tooltip(tooltip_text)
                                    .on_click(move |_event, _window, cx| {
                                        model.update(cx, |m, cx| {
                                            if is_pause {
                                                m.pause_task(task_id, cx);
                                            } else {
                                                m.resume_task(task_id, cx);
                                            }
                                        });
                                    })
                            }))
                            .children(can_delete.then(|| {
                                IconButton::new(
                                    ("download-card-delete", task_key),
                                    "icons/trash.svg",
                                )
                                .size(px(30.0))
                                .icon_size(px(14.0))
                                .rounded(ButtonRounded::Size(px(99.0)))
                                .color(palette.error)
                                .hover_bg(palette.error.opacity(0.14))
                                .tooltip("Eliminar descarga")
                                .on_click(
                                    move |_event, window, cx| {
                                        cx.set_global(DeleteDownloadModalState {
                                            open: true,
                                            task_id: Some(task_id),
                                            file_name: file_name_for_modal.clone(),
                                            delete_file: false,
                                        });
                                        window.refresh();
                                    },
                                )
                            })),
                    ),
            )
    }

    fn render_empty(palette: ViewPalette, interactive: bool) -> Div {
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Aún no hay descargas"),
            )
            .child(
                div()
                    .id("downloads-empty-add")
                    .mt_1()
                    .px_2()
                    .py_1()
                    .text_sm()
                    .text_color(palette.text_muted)
                    .when(interactive, |action| {
                        action
                            .cursor_pointer()
                            .hover(move |style| style.text_color(palette.text_color).underline())
                            .on_click(|_, window, cx| {
                                NewDownloadModal::open(None, window, cx);
                            })
                    })
                    .child("Pulsa + para añadir una"),
            )
    }

    fn render_glow(surface: Rgba) -> impl IntoElement {
        div().absolute().inset_0().with_animation(
            "downloads-glow",
            Animation::new(Duration::from_secs(10))
                .repeat()
                .with_max_fps(24.0),
            move |glow, phase| {
                glow.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, (), window, _| {
                            Self::paint_glow(bounds, surface, phase, window)
                        },
                    )
                    .size_full(),
                )
            },
        )
    }

    fn paint_glow(bounds: Bounds<Pixels>, surface: Rgba, phase: f32, window: &mut Window) {
        const COLUMNS: usize = 31;
        const ROWS: usize = 15;
        let cell = (bounds.size.width * 0.68 / COLUMNS as f32)
            .min(bounds.size.height * 0.48 / ROWS as f32);
        if cell <= px(0.0) {
            return;
        }

        let angle = phase * TAU;
        let pulse = angle.cos();
        let side_space = (bounds.size.width - cell * COLUMNS as f32) / 2.0;
        let travel = (bounds.size.width * 0.12).min((side_space - px(16.0)).max(px(0.0)));
        let left = bounds.left() + side_space + travel * angle.sin();
        let top = bounds.bottom() - cell * (ROWS as f32 - 0.5);
        let opacity = (0.64 - surface.g * 0.36) * (0.92 + 0.08 * pulse);
        let glow_strength = |x: f32, y: f32| (1.0 - x * x - y * y).max(0.0).powi(2);

        for row in 0..ROWS {
            let y = row as f32 / (ROWS - 1) as f32;
            for column in 0..COLUMNS {
                let x = column as f32 / (COLUMNS - 1) as f32;
                let blue = glow_strength((x - 0.37 - 0.04 * pulse) / 0.36, (y - 0.81) / 0.77);
                let violet = glow_strength((x - 0.65 + 0.04 * pulse) / 0.31, (y - 0.88) / 0.70);
                let strength = blue + violet;
                if strength < 0.035 {
                    continue;
                }

                let blend = violet / strength;
                let color = Rgba {
                    r: 0.19 + 0.20 * blend,
                    g: 0.39 - 0.10 * blend,
                    b: 0.66 + 0.04 * blend,
                    a: opacity * strength.min(1.0),
                };
                let origin = point(left + cell * column as f32, top + cell * row as f32);
                window.paint_quad(fill(Bounds::new(origin, size(cell, cell)), color));
            }
        }
    }
}

#[derive(IntoElement)]
pub struct DeleteDownloadModal;

impl DeleteDownloadModal {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeleteDownloadModal {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DeleteDownloadModal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let modal_state = cx
            .try_global::<DeleteDownloadModalState>()
            .cloned()
            .unwrap_or_default();

        let is_open = modal_state.open && modal_state.task_id.is_some();
        let task_id = modal_state.task_id;
        let file_name = modal_state.file_name.clone();
        let delete_file = modal_state.delete_file;

        let palette = ViewPalette::from_theme(cx);
        let toggle_file_name = modal_state.file_name.clone();

        div()
            .id("delete-download-modal-root")
            .absolute()
            .inset_0()
            .size_full()
            .child(
                Modal::new("delete-download-modal")
                    .open(is_open)
                    .width(px(520.0))
                    .max_height(px(320.0))
                    .title("Eliminar descarga")
                    .on_close(move |window, cx| {
                        cx.set_global(DeleteDownloadModalState {
                            open: false,
                            task_id: None,
                            file_name: String::new(),
                            delete_file: false,
                        });
                        window.refresh();
                    })
                    .footer(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2p5()
                            .pt_2()
                            .child(
                                PillButton::new("btn-cancel-delete-download", "Cancelar")
                                    .selected(false)
                                    .on_click(move |_event, window, cx| {
                                        cx.set_global(DeleteDownloadModalState {
                                            open: false,
                                            task_id: None,
                                            file_name: String::new(),
                                            delete_file: false,
                                        });
                                        window.refresh();
                                    }),
                            )
                            .child(
                                PillButton::new("btn-confirm-delete-download", "Eliminar")
                                    .selected(true)
                                    .on_click(move |_event, window, cx| {
                                        if let Some(id) = task_id {
                                            let model = cx.tasks_model().clone();
                                            model.update(cx, |m, cx| {
                                                m.delete_task(id, delete_file, cx);
                                            });
                                        }
                                        cx.set_global(DeleteDownloadModalState {
                                            open: false,
                                            task_id: None,
                                            file_name: String::new(),
                                            delete_file: false,
                                        });
                                        window.refresh();
                                    }),
                            ),
                    )
                    .child(
                        div().flex().flex_col().gap_5().child(
                            div()
                                .id("checkbox-delete-file-from-disk")
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_6()
                                .py_2()
                                .cursor_pointer()
                                .on_click(move |_event, window, cx| {
                                    cx.set_global(DeleteDownloadModalState {
                                        open: true,
                                        task_id,
                                        file_name: toggle_file_name.clone(),
                                        delete_file: !delete_file,
                                    });
                                    window.refresh();
                                })
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap_1p5()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_size(px(14.0))
                                                .text_color(palette.text_color)
                                                .truncate()
                                                .child(file_name),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(palette.text_muted)
                                                .child("Eliminar también el archivo del disco."),
                                        ),
                                )
                                .child(
                                    div()
                                        .size(px(18.0))
                                        .rounded(px(5.0))
                                        .border_1()
                                        .border_color(if delete_file {
                                            palette.text_color
                                        } else {
                                            palette.border
                                        })
                                        .bg(if delete_file {
                                            palette.surface_hover
                                        } else {
                                            palette.background
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .flex_shrink_0()
                                        .children(delete_file.then(|| {
                                            div()
                                                .size(px(8.0))
                                                .rounded(px(2.0))
                                                .bg(palette.text_color)
                                        })),
                                ),
                        ),
                    ),
            )
    }
}
