use chrono::{Datelike, Days};
use gpui::*;

use crate::{
    components::ActionTooltip,
    lang::Lang,
    state::{DownloadHistory, HISTORY_WEEKDAYS, HISTORY_WEEKS},
    theme::Theme,
};

const MONTHS: [&str; 12] = [
    "calendar.jan",
    "calendar.feb",
    "calendar.mar",
    "calendar.apr",
    "calendar.may",
    "calendar.jun",
    "calendar.jul",
    "calendar.aug",
    "calendar.sep",
    "calendar.oct",
    "calendar.nov",
    "calendar.dec",
];
const WEEKDAY_WIDTH: Pixels = px(30.0);
const DAY_STRIDE: f32 = 14.0;

pub(super) fn download_calendar(history: &DownloadHistory, theme: &dyn Theme, lang: &Lang) -> Div {
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
    let title = lang.get_with(
        if count == 1 {
            "calendar.completed_one"
        } else {
            "calendar.completed_many"
        },
        &[("count", &count.to_string())],
    );

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
                        .child(lang.get("calendar.title")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(lang.get_with("calendar.period", &[("title", &title)])),
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
                    .child(month_labels(history, muted, lang))
                    .child(
                        div()
                            .h(px(DAY_STRIDE * HISTORY_WEEKDAYS as f32))
                            .flex()
                            .child(weekday_labels(muted, lang))
                            .child(day_grid(history, colors, lang)),
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
                    lang.get_with(
                        "calendar.undated",
                        &[("count", &history.undated.to_string())],
                    )
                } else {
                    SharedString::default()
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(lang.get("common.less"))
                        .children(colors.map(|color| div().size_2p5().rounded(px(2.0)).bg(color)))
                        .child(lang.get("common.more")),
                ),
        )
}

fn month_labels(history: &DownloadHistory, color: Rgba, lang: &Lang) -> Div {
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
                        .child(lang.get(MONTHS[day.month0() as usize]))
                })
            })),
    )
}

fn weekday_labels(color: Rgba, lang: &Lang) -> Div {
    div()
        .w(WEEKDAY_WIDTH)
        .flex_shrink_0()
        .flex()
        .flex_col()
        .text_xs()
        .text_color(color)
        .children(
            [
                "calendar.mon",
                "",
                "calendar.wed",
                "",
                "calendar.fri",
                "",
                "",
            ]
            .map(|label| div().flex_1().flex().items_center().child(lang.get(label))),
        )
}

fn day_grid(history: &DownloadHistory, colors: [Rgba; 5], lang: &Lang) -> Div {
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
            let label = lang.get_with(
                if count == 1 {
                    "calendar.day_one"
                } else {
                    "calendar.day_many"
                },
                &[
                    ("day", &date.day().to_string()),
                    ("month", &lang.get(MONTHS[date.month0() as usize])),
                    ("year", &date.year().to_string()),
                    ("count", &count.to_string()),
                ],
            );
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
