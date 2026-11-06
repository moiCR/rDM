use gpui::*;
use rdm_core::task::TaskStatus;
use uuid::Uuid;

use crate::{
    state::{CoreContext, TaskSnapshot},
    theme::ActiveTheme,
};

#[derive(Default)]
pub struct DownloadsViewState {
    pub active_menu_id: Option<Uuid>,
}

#[derive(Clone, Copy)]
struct ViewPalette {
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

struct RowContext<'a> {
    view_state: &'a Entity<DownloadsViewState>,
    tasks_model: &'a Entity<crate::state::TasksModel>,
}

fn format_bytes(bytes: u64) -> String {
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
        format!("{} B", bytes)
    }
}

fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec == 0 {
        "-".to_string()
    } else {
        format!("{}/s", format_bytes(bytes_per_sec))
    }
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let background = cx.theme().background();
        let palette = ViewPalette {
            surface: cx.theme().surface(),
            surface_hover: cx.theme().surface_hover(),
            border: cx.theme().border(),
            text_color: cx.theme().text_color(),
            text_muted: cx.theme().text_muted(),
            downloading: cx.theme().downloading(),
            paused: cx.theme().paused(),
            completed: cx.theme().completed(),
            error: cx.theme().error(),
        };

        let tasks_model = cx.tasks_model().clone();
        let tasks = tasks_model.read(cx).tasks().to_vec();

        let view_state = window.use_keyed_state("downloads-view-state", cx, |_, _| {
            DownloadsViewState::default()
        });

        let active_menu_id = view_state.read(cx).active_menu_id;

        let row_ctx = RowContext {
            view_state: &view_state,
            tasks_model: &tasks_model,
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
            .text_color(palette.text_color)
            .p_8()
            .gap_5()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Downloads"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .rounded(px(16.0))
                    .bg(palette.surface)
                    .border_1()
                    .border_color(palette.border)
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .w_full()
                            .h_10()
                            .px_5()
                            .border_b_1()
                            .border_color(palette.border)
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.text_muted)
                            .child(div().flex_1().min_w_0().child("Archivo"))
                            .child(div().w(px(130.0)).flex_shrink_0().child("Tamaño"))
                            .child(div().w(px(180.0)).flex_shrink_0().child("Progreso"))
                            .child(div().w(px(100.0)).flex_shrink_0().child("Velocidad"))
                            .child(div().w(px(110.0)).flex_shrink_0().child("Estado"))
                            .child(div().w(px(40.0)).flex_shrink_0()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .w_full()
                            .overflow_hidden()
                            .children(tasks.into_iter().map(|task| {
                                Self::render_task_row(task, active_menu_id, &row_ctx, palette)
                            })),
                    ),
            )
    }
}

impl DownloadsView {
    fn render_task_row(
        task: TaskSnapshot,
        active_menu_id: Option<Uuid>,
        ctx: &RowContext,
        palette: ViewPalette,
    ) -> AnyElement {
        let is_menu_open = active_menu_id == Some(task.id);
        let task_id = task.id;

        let (status_label, status_color) = match task.status {
            TaskStatus::Downloading | TaskStatus::Connecting => {
                ("Descargando", palette.downloading)
            }
            TaskStatus::Paused => ("Pausado", palette.paused),
            TaskStatus::Completed => ("Completado", palette.completed),
            TaskStatus::Failed(_) => ("Error", palette.error),
            TaskStatus::Pending => ("En cola", palette.text_muted),
            TaskStatus::Cancelled => ("Cancelado", palette.text_muted),
        };

        let progress_val = (task.progress.clamp(0.0, 1.0) * 100.0) as u32;
        let progress_ratio = task.progress.clamp(0.0, 1.0) as f32;

        let downloaded_str = format_bytes(task.downloaded_bytes);
        let size_str = if task.total_size > 0 {
            format!("{downloaded_str} / {}", format_bytes(task.total_size))
        } else {
            downloaded_str
        };
        let speed_str = format_speed(task.speed_bytes_per_sec);

        let view_state_toggle = ctx.view_state.clone();

        let appear_spring = SpringAnimation::new(SpringConfig::new(220.0, 24.0, 1.0))
            .to(true)
            .from(false);

        div()
            .id(ElementId::Name(format!("task-row-{}", task.id).into()))
            .relative()
            .flex()
            .items_center()
            .w_full()
            .px_5()
            .border_b_1()
            .border_color(palette.border)
            .text_xs()
            .overflow_hidden()
            .hover(move |s| s.bg(palette.surface_hover))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .flex_1()
                    .min_w_0()
                    .pr_4()
                    .child(
                        svg()
                            .path("icons/download.svg")
                            .size_4()
                            .text_color(status_color)
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(palette.text_color)
                            .truncate()
                            .child(task.file_name.clone()),
                    ),
            )
            .child(
                div()
                    .w(px(130.0))
                    .flex_shrink_0()
                    .text_color(palette.text_muted)
                    .truncate()
                    .child(size_str),
            )
            .child(
                div()
                    .w(px(180.0))
                    .flex_shrink_0()
                    .pr_4()
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .child(
                        div()
                            .flex_1()
                            .h(px(5.0))
                            .rounded_full()
                            .bg(palette.surface_hover)
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(status_color)
                                    .w(relative(progress_ratio)),
                            ),
                    )
                    .child(
                        div()
                            .w(px(34.0))
                            .text_color(palette.text_muted)
                            .child(format!("{progress_val}%")),
                    ),
            )
            .child(
                div()
                    .w(px(100.0))
                    .flex_shrink_0()
                    .text_color(palette.text_muted)
                    .child(speed_str),
            )
            .child(
                div()
                    .w(px(110.0))
                    .flex_shrink_0()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(status_color)
                    .child(status_label),
            )
            .child(
                div()
                    .id(ElementId::Name(
                        format!("btn-menu-anchor-{}", task.id).into(),
                    ))
                    .w(px(40.0))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .relative()
                    .child(
                        div()
                            .id(ElementId::Name(format!("btn-ellipsis-{}", task.id).into()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_7()
                            .rounded_lg()
                            .cursor_pointer()
                            .hover(move |s| s.bg(palette.surface_hover))
                            .on_click(move |_event, window, cx| {
                                view_state_toggle.update(cx, |s, _| {
                                    if s.active_menu_id == Some(task_id) {
                                        s.active_menu_id = None;
                                    } else {
                                        s.active_menu_id = Some(task_id);
                                    }
                                });
                                window.refresh();
                            })
                            .child(
                                svg()
                                    .path("icons/ellipsis.svg")
                                    .size_4()
                                    .text_color(palette.text_muted),
                            ),
                    )
                    .children(if is_menu_open {
                        Some(deferred(Self::render_context_menu(&task, ctx, palette)))
                    } else {
                        None
                    }),
            )
            .with_spring(
                ElementId::Name(format!("task-row-appear-{}", task.id).into()),
                appear_spring,
                |this, phase| {
                    let height = phase.interpolate_clamped(px(0.0), px(48.0));
                    let alpha = phase.interpolate_clamped(0.0, 1.0);
                    this.h(height).opacity(alpha)
                },
            )
            .into_any_element()
    }

    fn render_context_menu(
        task: &TaskSnapshot,
        ctx: &RowContext,
        palette: ViewPalette,
    ) -> AnyElement {
        let task_id = task.id;
        let url = task.url.clone();
        let output_path = task.output_path.clone();

        let view_state_pause = ctx.view_state.clone();
        let view_state_resume = ctx.view_state.clone();
        let view_state_folder = ctx.view_state.clone();
        let view_state_copy = ctx.view_state.clone();
        let view_state_del_list = ctx.view_state.clone();
        let view_state_del_file = ctx.view_state.clone();

        let tasks_model_pause = ctx.tasks_model.clone();
        let tasks_model_resume = ctx.tasks_model.clone();
        let tasks_model_del_list = ctx.tasks_model.clone();
        let tasks_model_del_file = ctx.tasks_model.clone();

        div()
            .absolute()
            .top(px(32.0))
            .right_0()
            .w(px(210.0))
            .p_1p5()
            .rounded(px(16.0))
            .bg(palette.surface)
            .border_1()
            .border_color(palette.border)
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_1()
            .children(match task.status {
                TaskStatus::Downloading | TaskStatus::Connecting => Some(
                    div()
                        .id(ElementId::Name(format!("menu-pause-{}", task_id).into()))
                        .flex()
                        .items_center()
                        .gap_2p5()
                        .px_3()
                        .h_8()
                        .rounded_lg()
                        .cursor_pointer()
                        .hover(move |s| s.bg(palette.surface_hover))
                        .on_click(move |_event, window, cx| {
                            tasks_model_pause.update(cx, |m, cx| m.pause_task(task_id, cx));
                            view_state_pause.update(cx, |s, _| s.active_menu_id = None);
                            window.refresh();
                        })
                        .child(
                            svg()
                                .path("icons/pause.svg")
                                .size_3p5()
                                .text_color(palette.text_muted),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(palette.text_color)
                                .child("Pausar descarga"),
                        ),
                ),
                TaskStatus::Paused | TaskStatus::Failed(_) | TaskStatus::Pending => Some(
                    div()
                        .id(ElementId::Name(format!("menu-resume-{}", task_id).into()))
                        .flex()
                        .items_center()
                        .gap_2p5()
                        .px_3()
                        .h_8()
                        .rounded_lg()
                        .cursor_pointer()
                        .hover(move |s| s.bg(palette.surface_hover))
                        .on_click(move |_event, window, cx| {
                            tasks_model_resume.update(cx, |m, cx| m.resume_task(task_id, cx));
                            view_state_resume.update(cx, |s, _| s.active_menu_id = None);
                            window.refresh();
                        })
                        .child(
                            svg()
                                .path("icons/resume.svg")
                                .size_3p5()
                                .text_color(palette.text_muted),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(palette.text_color)
                                .child("Reanudar descarga"),
                        ),
                ),
                _ => None,
            })
            .child(
                div()
                    .id(ElementId::Name(format!("menu-folder-{}", task_id).into()))
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .px_3()
                    .h_8()
                    .rounded_lg()
                    .cursor_pointer()
                    .hover(move |s| s.bg(palette.surface_hover))
                    .on_click(move |_event, window, cx| {
                        let _ = std::process::Command::new("explorer.exe")
                            .arg(format!("/select,{}", output_path.display()))
                            .spawn();
                        view_state_folder.update(cx, |s, _| s.active_menu_id = None);
                        window.refresh();
                    })
                    .child(
                        svg()
                            .path("icons/folder-open.svg")
                            .size_3p5()
                            .text_color(palette.text_muted),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(palette.text_color)
                            .child("Abrir carpeta"),
                    ),
            )
            .child(
                div()
                    .id(ElementId::Name(format!("menu-copy-{}", task_id).into()))
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .px_3()
                    .h_8()
                    .rounded_lg()
                    .cursor_pointer()
                    .hover(move |s| s.bg(palette.surface_hover))
                    .on_click(move |_event, window, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
                        view_state_copy.update(cx, |s, _| s.active_menu_id = None);
                        window.refresh();
                    })
                    .child(
                        svg()
                            .path("icons/copy.svg")
                            .size_3p5()
                            .text_color(palette.text_muted),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(palette.text_color)
                            .child("Copiar enlace"),
                    ),
            )
            .child(div().h(px(1.0)).bg(palette.border).my_0p5())
            .child(
                div()
                    .id(ElementId::Name(format!("menu-del-list-{}", task_id).into()))
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .px_3()
                    .h_8()
                    .rounded_lg()
                    .cursor_pointer()
                    .hover(move |s| s.bg(palette.surface_hover))
                    .on_click(move |_event, window, cx| {
                        tasks_model_del_list.update(cx, |m, cx| m.delete_task(task_id, false, cx));
                        view_state_del_list.update(cx, |s, _| s.active_menu_id = None);
                        window.refresh();
                    })
                    .child(
                        svg()
                            .path("icons/x.svg")
                            .size_3p5()
                            .text_color(palette.text_muted),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(palette.text_color)
                            .child("Eliminar de la lista"),
                    ),
            )
            .child(
                div()
                    .id(ElementId::Name(format!("menu-del-file-{}", task_id).into()))
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .px_3()
                    .h_8()
                    .rounded_lg()
                    .cursor_pointer()
                    .hover(move |s| s.bg(palette.surface_hover))
                    .on_click(move |_event, window, cx| {
                        tasks_model_del_file.update(cx, |m, cx| m.delete_task(task_id, true, cx));
                        view_state_del_file.update(cx, |s, _| s.active_menu_id = None);
                        window.refresh();
                    })
                    .child(
                        svg()
                            .path("icons/trash.svg")
                            .size_3p5()
                            .text_color(palette.error),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(palette.error)
                            .child("Eliminar archivo"),
                    ),
            )
            .into_any_element()
    }
}
