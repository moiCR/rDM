use std::{cell::Cell, f32::consts::TAU, rc::Rc, time::Duration};

use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::{
    base::{
        Button as BaseButton, ElementExt as _, Table, TableBody, TableCell, TableHead, TableHeader,
        TableRow,
    },
    component::{
        Icon, Sizable, Size as KitSize,
        button::{Button, ButtonCustomVariant, ButtonVariants},
        popover::{Popover, PopoverState},
        scroll::ScrollableElement,
    },
};
use rdm_core::task::TaskStatus;

use crate::{
    components::{ActionTooltip, ProgressBar},
    state::{CoreContext, QuickDownloadState, TaskSnapshot, TasksModel},
    theme::ActiveTheme,
};

const HEADER_HEIGHT: Pixels = px(40.0);
const ROW_HEIGHT: Pixels = px(48.0);
const ACTIONS_WIDTH: Pixels = px(40.0);
const COLUMN_COUNT: usize = DownloadColumn::ALL.len();

#[derive(Clone, Default)]
struct DownloadColumns {
    widths: Option<[Pixels; COLUMN_COUNT]>,
    measured: Rc<Cell<[Pixels; COLUMN_COUNT]>>,
    horizontal_scroll: ScrollHandle,
}

#[derive(Clone, Copy)]
struct RowLayout {
    widths: [Pixels; COLUMN_COUNT],
    minimum_width: Pixels,
    offset: Pixels,
}

struct ColumnResize {
    table_id: EntityId,
    index: usize,
    start: Cell<Option<(Pixels, [Pixels; COLUMN_COUNT])>>,
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

impl ViewPalette {
    fn from_theme(cx: &App) -> Self {
        let theme = cx.theme();
        Self {
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
            TaskStatus::Connecting => ("Conectando", self.downloading),
            TaskStatus::Paused => ("En pausa", self.paused),
            TaskStatus::Completed => ("Completada", self.completed),
            TaskStatus::Failed(_) => ("Error", self.error),
            TaskStatus::Pending => ("En cola", self.text_muted),
            TaskStatus::Cancelled => ("Cancelado", self.text_muted),
        }
    }
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

fn format_speed(bytes_per_sec: u64) -> SharedString {
    if bytes_per_sec == 0 {
        "—".into()
    } else {
        format!("{}/s", format_bytes(bytes_per_sec)).into()
    }
}

fn format_remaining(task: &TaskSnapshot) -> SharedString {
    let remaining = task.total_size.saturating_sub(task.downloaded_bytes);
    let speed = task.speed_bytes_per_sec;
    if task.status != TaskStatus::Downloading || remaining == 0 || speed == 0 {
        return "—".into();
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let background = cx.theme().background();
        let palette = ViewPalette::from_theme(cx);
        let tasks_model = cx.tasks_model().clone();
        let task_count = tasks_model.read(cx).tasks().len();
        let is_empty = task_count == 0;
        let columns = window.use_keyed_state("downloads-column-widths", cx, |_, _| {
            DownloadColumns::default()
        });
        let layout = columns.read(cx).clone();
        let transition = SpringAnimation::new(SpringConfig::new(300.0, 34.0, 1.0)).to(!is_empty);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
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
                    .with_spring("downloads-panel-state", transition, move |panel, phase| {
                        let table_opacity = phase.interpolate_clamped(0.0, 1.0);
                        panel
                            .when(!is_empty, |panel| {
                                panel.child(
                                    div()
                                        .size_full()
                                        .relative()
                                        .opacity(table_opacity)
                                        .child(
                                            div()
                                                .absolute()
                                                .top_0()
                                                .left_0()
                                                .right_0()
                                                .h(HEADER_HEIGHT)
                                                .rounded_t(px(15.0))
                                                .bg(palette.surface_hover),
                                        )
                                        .child(
                                            DownloadsTable { tasks_model }
                                                .render(task_count, palette, columns, layout),
                                        ),
                                )
                            })
                            .when(is_empty || table_opacity < 0.99, |panel| {
                                panel.child(
                                    Self::render_empty(palette, is_empty)
                                        .opacity(1.0 - table_opacity),
                                )
                            })
                    }),
            )
    }
}

impl DownloadsView {
    fn render_empty(palette: ViewPalette, interactive: bool) -> Div {
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .child(Self::render_glow(palette.surface))
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
                                cx.set_global(QuickDownloadState { expanded: true });
                                window.refresh();
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

#[derive(Clone, Copy)]
enum DownloadColumn {
    File,
    Size,
    Progress,
    Remaining,
    Network,
    Written,
    Status,
}

impl DownloadColumn {
    const ALL: [Self; 7] = [
        Self::File,
        Self::Size,
        Self::Progress,
        Self::Remaining,
        Self::Network,
        Self::Written,
        Self::Status,
    ];

    fn layout(self) -> (&'static str, Pixels, f32) {
        match self {
            Self::File => ("Archivo", px(166.0), 4.0),
            Self::Size => ("Tamaño", px(84.0), 1.0),
            Self::Progress => ("Progreso", px(100.0), 2.0),
            Self::Remaining => ("Restante", px(85.0), 1.0),
            Self::Network => ("Red", px(100.0), 1.0),
            Self::Written => ("Escrito", px(86.0), 1.0),
            Self::Status => ("Estado", px(110.0), 1.0),
        }
    }

    fn minimum_width(self) -> Pixels {
        px(match self {
            Self::File => 96.0,
            Self::Size => 76.0,
            Self::Progress => 80.0,
            Self::Remaining => 82.0,
            Self::Network => 96.0,
            Self::Written => 82.0,
            Self::Status => 106.0,
        })
    }

    fn progress(task: &TaskSnapshot) -> Option<f32> {
        match (task.total_size, &task.status) {
            (_, TaskStatus::Completed) => Some(100.0),
            (0, _) => None,
            _ => Some((task.progress.clamp(0.0, 1.0) * 100.0) as f32),
        }
    }

    fn label(self, task: &TaskSnapshot, palette: ViewPalette) -> SharedString {
        match self {
            Self::File => task.file_name.clone().into(),
            Self::Size if task.total_size > 0 => format_bytes(task.total_size).into(),
            Self::Size => "—".into(),
            Self::Progress => Self::progress(task).map_or_else(
                || "—".into(),
                |percent| format!("{}%", percent as u32).into(),
            ),
            Self::Remaining => format_remaining(task),
            Self::Network if task.status == TaskStatus::Downloading => {
                format_speed(task.speed_bytes_per_sec)
            }
            Self::Network => "—".into(),
            Self::Written => format_bytes(task.downloaded_bytes).into(),
            Self::Status => palette.status(&task.status).0.into(),
        }
    }

    fn numeric(self) -> bool {
        matches!(
            self,
            Self::Size | Self::Remaining | Self::Network | Self::Written
        )
    }
}

struct DownloadsTable {
    tasks_model: Entity<TasksModel>,
}

impl DownloadsTable {
    fn render(
        self,
        task_count: usize,
        palette: ViewPalette,
        columns: Entity<DownloadColumns>,
        layout: DownloadColumns,
    ) -> impl IntoElement {
        let widths = layout
            .widths
            .unwrap_or_else(|| DownloadColumn::ALL.map(|column| column.layout().1));
        let minimum_width: Pixels = DownloadColumn::ALL
            .into_iter()
            .map(DownloadColumn::minimum_width)
            .sum();

        let horizontal_scroll = layout.horizontal_scroll;
        let header = Self::render_header(
            palette,
            columns,
            widths,
            minimum_width,
            layout.measured,
            &horizontal_scroll,
        );
        let row_scroll = horizontal_scroll.clone();
        let rows = uniform_list("download-table-rows", task_count, move |range, _, cx| {
            let tasks = self.tasks_model.read(cx).tasks();
            let row_layout = RowLayout {
                widths,
                minimum_width,
                offset: row_scroll.offset().x,
            };
            range
                .filter_map(|index| {
                    tasks
                        .get(index)
                        .map(|task| self.render_row(task, index, palette, row_layout, cx))
                })
                .collect()
        })
        .size_full()
        .min_h_0()
        .flex_1();

        let body_scroll = horizontal_scroll.clone();
        div()
            .relative()
            .size_full()
            .child(
                Table::new("download-table")
                    .accessibility_label("Descargas")
                    .row_count(task_count + 1)
                    .column_count(COLUMN_COUNT + 1)
                    .size_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(header)
                    .child(
                        TableBody::new("download-table-body")
                            .w_full()
                            .flex_1()
                            .min_h_0()
                            .pb_4()
                            .on_scroll(move |event, window, cx| {
                                let delta = event.delta.pixel_delta(ROW_HEIGHT).x;
                                if delta == px(0.0) {
                                    return;
                                }
                                let mut offset = body_scroll.offset();
                                let next =
                                    (offset.x + delta).clamp(-body_scroll.max_offset().x, px(0.0));
                                if next != offset.x {
                                    offset.x = next;
                                    body_scroll.set_offset(offset);
                                    cx.stop_propagation();
                                    window.refresh();
                                }
                            })
                            .child(rows),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right(ACTIONS_WIDTH)
                    .h_4()
                    .horizontal_scrollbar(&horizontal_scroll),
            )
    }

    fn render_header(
        palette: ViewPalette,
        columns: Entity<DownloadColumns>,
        widths: [Pixels; COLUMN_COUNT],
        minimum_width: Pixels,
        measured: Rc<Cell<[Pixels; COLUMN_COUNT]>>,
        horizontal_scroll: &ScrollHandle,
    ) -> TableHeader {
        let table_id = columns.entity_id();
        let row = TableRow::new("download-table-heading-row", 1)
            .flex()
            .w_full()
            .h(HEADER_HEIGHT)
            .border_b_1()
            .border_color(palette.border)
            .on_drag_move(move |event: &DragMoveEvent<ColumnResize>, _, cx| {
                let drag = event.drag(cx);
                if drag.table_id != table_id {
                    return;
                }
                let Some((start_x, mut widths)) = drag.start.get() else {
                    return;
                };
                let index = drag.index;
                let next = index + 1;
                let total = widths[index] + widths[next];
                let minimum = DownloadColumn::ALL[index].minimum_width();
                let maximum = (total - DownloadColumn::ALL[next].minimum_width()).max(minimum);
                widths[index] =
                    (widths[index] + event.event.position.x - start_x).clamp(minimum, maximum);
                widths[next] = total - widths[index];
                columns.update(cx, |state, cx| {
                    if state.widths != Some(widths) {
                        state.widths = Some(widths);
                        cx.notify();
                    }
                });
            });
        let data = DownloadColumn::ALL.into_iter().enumerate().fold(
            div().flex().h_full().w_full().min_w(minimum_width),
            |data, (index, column)| {
                let (label, _, _) = column.layout();
                let measured = measured.clone();
                let measurements = measured.clone();
                let cell = TableHead::new(("download-header", index), index + 1)
                    .relative()
                    .h_full()
                    .px_3()
                    .flex()
                    .items_center()
                    .when(column.numeric(), |cell| cell.justify_end())
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.text_muted)
                    .aria_label(label)
                    .when(matches!(column, DownloadColumn::Network), |cell| {
                        cell.tooltip(|_, cx| {
                            cx.new(|_| ActionTooltip::new("Velocidad efectiva de descarga"))
                                .into()
                        })
                    })
                    .on_prepaint(move |bounds, _, _| {
                        let mut widths = measured.get();
                        widths[index] = bounds.size.width;
                        measured.set(widths);
                    })
                    .child(div().truncate().child(label))
                    .when(index + 1 < COLUMN_COUNT, |cell| {
                        cell.child(
                            div()
                                .id(("download-column-resize", index))
                                .absolute()
                                .right_0()
                                .top_0()
                                .h_full()
                                .w_2()
                                .flex()
                                .items_center()
                                .justify_end()
                                .occlude()
                                .cursor_col_resize()
                                .on_drag(
                                    ColumnResize {
                                        table_id,
                                        index,
                                        start: Cell::new(None),
                                    },
                                    move |drag, _, window, cx| {
                                        drag.start.set(Some((
                                            window.mouse_position().x,
                                            measurements.get(),
                                        )));
                                        cx.stop_propagation();
                                        cx.new(|_| Empty)
                                    },
                                )
                                .child(div().h_4().w_px().bg(palette.border)),
                        )
                    });
                data.child(Self::style_column(cell, column, widths[index]))
            },
        );

        TableHeader::new("download-table-header")
            .flex_shrink_0()
            .child(
                row.child(
                    div()
                        .id("download-horizontal-scroll")
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_x_scroll()
                        .track_scroll(horizontal_scroll)
                        .child(data),
                )
                .child(
                    TableHead::new("download-actions-header", COLUMN_COUNT + 1)
                        .w(ACTIONS_WIDTH)
                        .flex_shrink_0()
                        .aria_label("Acciones"),
                ),
            )
    }

    fn style_column<T: Styled>(cell: T, column: DownloadColumn, width: Pixels) -> T {
        cell.flex_basis(width)
            .min_w(column.minimum_width())
            .flex_shrink_1()
            .flex_grow(column.layout().2)
    }

    fn render_row(
        &self,
        task: &TaskSnapshot,
        index: usize,
        palette: ViewPalette,
        layout: RowLayout,
        cx: &App,
    ) -> TableRow {
        let row = TableRow::new(task.id, index + 2)
            .flex()
            .w_full()
            .h(ROW_HEIGHT)
            .border_b_1()
            .border_color(palette.border)
            .hover(move |row| row.bg(palette.surface_hover));
        let data = DownloadColumn::ALL.into_iter().enumerate().fold(
            div()
                .relative()
                .left(layout.offset)
                .flex()
                .h_full()
                .w_full()
                .min_w(layout.minimum_width),
            |data, (index, column)| {
                let label = column.label(task, palette);
                let cell = TableCell::new(("download-cell", index), index + 1)
                    .h_full()
                    .px_3()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .aria_label(label.clone())
                    .text_xs()
                    .text_color(palette.text_muted)
                    .when(column.numeric(), |cell| cell.text_right());
                let cell = Self::style_column(cell, column, layout.widths[index]);
                let text = div().truncate().child(label);
                let cell = match column {
                    DownloadColumn::File => cell
                        .text_color(palette.text_color)
                        .font_weight(FontWeight::MEDIUM)
                        .child(text),
                    DownloadColumn::Progress => {
                        cell.gap_1()
                            .child(text)
                            .children(DownloadColumn::progress(task).map(|percent| {
                                ProgressBar::new(task.id, percent)
                                    .height(px(4.0))
                                    .color(palette.status(&task.status).1)
                                    .track_color(palette.surface_hover)
                            }))
                    }
                    DownloadColumn::Remaining => cell.text_color(palette.text_color).child(text),
                    DownloadColumn::Status => {
                        cell.text_color(palette.status(&task.status).1).child(text)
                    }
                    _ => cell.child(text),
                };
                data.child(cell)
            },
        );

        row.child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .overflow_hidden()
                .child(data),
        )
        .child(
            TableCell::new("download-row-actions", COLUMN_COUNT + 1)
                .w(ACTIONS_WIDTH)
                .h_full()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .child(self.render_actions(task.id, palette, cx)),
        )
    }
    fn render_actions(&self, task_id: uuid::Uuid, palette: ViewPalette, cx: &App) -> Popover {
        let tasks_model = self.tasks_model.clone();
        let trigger_style = ButtonCustomVariant::new(cx)
            .foreground(palette.text_color.into())
            .hover(palette.surface_hover.into())
            .active(palette.border.into());

        Popover::new(task_id)
            .anchor(Anchor::TopRight)
            .bg(palette.surface)
            .border_color(palette.border)
            .text_color(palette.text_color)
            .rounded(px(10.0))
            .p_1p5()
            .w(px(212.0))
            .trigger(
                Button::new("download-actions-trigger")
                    .custom(trigger_style)
                    .with_size(KitSize::Size(px(28.0)))
                    .icon(Icon::default().path("icons/ellipsis.svg"))
                    .accessibility_label("Acciones de descarga"),
            )
            .content(move |_, _, cx| {
                let menu = div().flex().flex_col();
                let Some(task) = tasks_model
                    .read(cx)
                    .tasks()
                    .iter()
                    .find(|task| task.id == task_id)
                else {
                    return menu;
                };
                let control = match task.status {
                    TaskStatus::Downloading | TaskStatus::Connecting => Some(DownloadAction::Pause),
                    TaskStatus::Paused | TaskStatus::Failed(_) | TaskStatus::Pending => {
                        Some(DownloadAction::Resume)
                    }
                    _ => None,
                };
                let popover = cx.entity().downgrade();
                let item = |action: DownloadAction| {
                    let tasks_model = tasks_model.clone();
                    let popover = popover.clone();
                    let (label, icon) = action.presentation();
                    BaseButton::new(label)
                        .accessibility_label(label)
                        .w_full()
                        .h_8()
                        .px_2p5()
                        .gap_2()
                        .justify_start()
                        .rounded_md()
                        .cursor_pointer()
                        .text_size(px(13.0))
                        .line_height(px(18.0))
                        .hover(move |row| row.bg(palette.surface_hover))
                        .active(move |row| row.bg(palette.border))
                        .child(
                            svg()
                                .path(icon)
                                .size(px(14.0))
                                .flex_shrink_0()
                                .text_color(palette.text_muted),
                        )
                        .child(label)
                        .on_click(move |_, window, cx| {
                            let _ = popover.update(cx, |state: &mut PopoverState, cx| {
                                state.dismiss(window, cx);
                            });
                            action.run(task_id, &tasks_model, cx);
                        })
                };

                menu.children(control.map(item))
                    .child(item(DownloadAction::OpenFolder))
                    .child(item(DownloadAction::CopyLink))
                    .child(div().h_px().mx_2p5().my_1().bg(palette.border))
                    .child(item(DownloadAction::RemoveFromList))
                    .child(item(DownloadAction::RemoveFile))
            })
    }
}

#[derive(Clone, Copy)]
enum DownloadAction {
    Pause,
    Resume,
    OpenFolder,
    CopyLink,
    RemoveFromList,
    RemoveFile,
}

impl DownloadAction {
    fn presentation(self) -> (&'static str, &'static str) {
        match self {
            Self::Pause => ("Pausar descarga", "icons/pause.svg"),
            Self::Resume => ("Reanudar descarga", "icons/resume.svg"),
            Self::OpenFolder => ("Abrir carpeta", "icons/folder-open.svg"),
            Self::CopyLink => ("Copiar enlace", "icons/copy.svg"),
            Self::RemoveFromList => ("Eliminar de la lista", "icons/x.svg"),
            Self::RemoveFile => ("Eliminar archivo", "icons/trash.svg"),
        }
    }

    fn run(self, id: uuid::Uuid, tasks_model: &Entity<TasksModel>, cx: &mut App) {
        match self {
            Self::Pause => tasks_model.update(cx, |model, cx| model.pause_task(id, cx)),
            Self::Resume => tasks_model.update(cx, |model, cx| model.resume_task(id, cx)),
            Self::RemoveFromList | Self::RemoveFile => tasks_model.update(cx, |model, cx| {
                model.delete_task(id, matches!(self, Self::RemoveFile), cx);
            }),
            Self::OpenFolder | Self::CopyLink => {
                let Some(task) = tasks_model
                    .read(cx)
                    .tasks()
                    .iter()
                    .find(|task| task.id == id)
                else {
                    return;
                };
                match self {
                    Self::OpenFolder => {
                        let selection = format!("/select,{}", task.output_path.display());
                        let _ = std::process::Command::new("explorer.exe")
                            .arg(selection)
                            .spawn();
                    }
                    Self::CopyLink => {
                        cx.write_to_clipboard(ClipboardItem::new_string(task.url.clone()))
                    }
                    _ => {}
                }
            }
        }
    }
}
