use chrono::{Datelike, Days};
use gpui::*;

use crate::{
    components::ActionTooltip,
    state::{DownloadHistory, HISTORY_WEEKDAYS, HISTORY_WEEKS},
    theme::Theme,
};

const MONTHS: [&str; 12] = [
    "Ene", "Feb", "Mar", "Abr", "May", "Jun", "Jul", "Ago", "Sep", "Oct", "Nov", "Dic",
];
const WEEKDAY_WIDTH: Pixels = px(30.0);
const DAY_STRIDE: f32 = 14.0;

pub(super) fn download_calendar(history: &DownloadHistory, theme: &dyn Theme) -> Div {
    let muted = theme.text_muted();
    let accent = theme.accent();
    let colors = [
        muted.opacity(0.12),
        accent.opacity(0.32),
        accent.opacity(0.51),
        accent.opacity(0.73),
        accent,
    ];
    let count = history.total;
    let title = if count == 1 {
        "1 descarga completada".to_owned()
    } else {
        format!("{count} descargas completadas")
    };

    div()
        .flex()
        .flex_col()
        .gap_1p5()
        .child(
            div()
                .flex()
                .flex_wrap()
                .justify_between()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(muted)
                        .child("HISTORIAL DE DESCARGAS"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(format!("{title} · Últimos 365 días")),
                ),
        )
        .child(
            div().flex().justify_center().child(
                div()
                    .w_full()
                    .max_w(WEEKDAY_WIDTH + px(DAY_STRIDE * HISTORY_WEEKS as f32))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(month_labels(history, muted))
                    .child(
                        div()
                            .h(px(DAY_STRIDE * HISTORY_WEEKDAYS as f32))
                            .flex()
                            .child(weekday_labels(muted))
                            .child(day_grid(history, colors)),
                    ),
            ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .text_xs()
                .text_color(muted)
                .child(if history.undated > 0 {
                    format!("Sin fecha registrada: {}", history.undated)
                } else {
                    String::new()
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child("Menos")
                        .children(colors.map(|color| div().size_2p5().rounded(px(2.0)).bg(color)))
                        .child("Más"),
                ),
        )
}

fn month_labels(history: &DownloadHistory, color: Rgba) -> Div {
    div().pl(WEEKDAY_WIDTH).child(
        div()
            .relative()
            .h(px(16.0))
            .text_xs()
            .text_color(color)
            .children((history.first_cell..=history.last_cell).filter_map(|cell| {
                let day = history
                    .first_week
                    .checked_add_days(Days::new(cell as u64))?;
                let week = cell / HISTORY_WEEKDAYS;
                let first_partial_month = cell == history.first_cell && day.day() < 25;
                ((day.day() == 1 || first_partial_month) && week < HISTORY_WEEKS - 1).then(|| {
                    div()
                        .absolute()
                        .left(relative(week as f32 / HISTORY_WEEKS as f32))
                        .child(MONTHS[day.month0() as usize])
                })
            })),
    )
}

fn weekday_labels(color: Rgba) -> Div {
    div()
        .w(WEEKDAY_WIDTH)
        .flex_shrink_0()
        .flex()
        .flex_col()
        .text_xs()
        .text_color(color)
        .children(
            ["Lun", "", "Mié", "", "Vie", "", ""]
                .map(|label| div().flex_1().flex().items_center().child(label)),
        )
}

fn day_grid(history: &DownloadHistory, colors: [Rgba; 5]) -> Div {
    let counts = history.counts;
    let days = history.first_cell..=history.last_cell;
    let painted_days = days.clone();

    div()
        .relative()
        .flex_1()
        .min_w_0()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    let column = bounds.size.width / HISTORY_WEEKS as f32;
                    let row = bounds.size.height / HISTORY_WEEKDAYS as f32;
                    let stride = column.min(row);
                    let side = stride - px(2.0).min(stride / 4.0);
                    let cell = size(side, side);
                    let offset = point((column - side) / 2.0, (row - side) / 2.0);
                    for index in painted_days.clone() {
                        let x = index / HISTORY_WEEKDAYS;
                        let y = index % HISTORY_WEEKDAYS;
                        let origin =
                            bounds.origin + offset + point(column * x as f32, row * y as f32);
                        let level = match counts[index] {
                            0 => 0,
                            1 => 1,
                            2..=3 => 2,
                            4..=7 => 3,
                            _ => 4,
                        };
                        window.paint_quad(
                            fill(Bounds::new(origin, cell), colors[level]).corner_radii(px(2.0)),
                        );
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
        .children(days.filter_map(|index| {
            let count = counts[index];
            if count == 0 {
                return None;
            }
            let date = history
                .first_week
                .checked_add_days(Days::new(index as u64))?;
            let label = SharedString::from(format!(
                "{} {} {} · {} {}",
                date.day(),
                MONTHS[date.month0() as usize],
                date.year(),
                count,
                if count == 1 { "descarga" } else { "descargas" },
            ));
            Some(
                div()
                    .id(("download-history-day", date.num_days_from_ce() as u64))
                    .absolute()
                    .left(relative(
                        (index / HISTORY_WEEKDAYS) as f32 / HISTORY_WEEKS as f32,
                    ))
                    .top(relative(
                        (index % HISTORY_WEEKDAYS) as f32 / HISTORY_WEEKDAYS as f32,
                    ))
                    .w(relative(1.0 / HISTORY_WEEKS as f32))
                    .h(relative(1.0 / HISTORY_WEEKDAYS as f32))
                    .tooltip(move |_, cx| cx.new(|_| ActionTooltip::new(label.clone())).into()),
            )
        }))
}
