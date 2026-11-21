use std::{cell::Cell, f32::consts::TAU, rc::Rc, time::Duration};

use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::component::button::ButtonRounded;
use gpui_pages::{Page, PageAnimation, PageOptions};
use rdm_core::task::TaskStatus;

use crate::{
    components::{
        ActionTooltip, Button, IconButton, ProgressBar,
        download_card::{download_card_surface, format_bytes, task_progress_percent},
        track_card_bounds,
    },
    lang::ActiveLang,
    modals::{DeleteDownloadModalState, DownloadInspectorModalState, NewDownloadModal},
    state::{CoreContext, TaskSnapshot, TasksModel},
    theme::ActiveTheme,
};

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
            TaskStatus::Downloading => ("status.downloading", self.downloading),
            TaskStatus::Paused => ("status.paused", self.paused),
            TaskStatus::Completed => ("status.completed", self.completed),
            TaskStatus::Failed(_) => ("status.failed", self.error),
            TaskStatus::ConversionFailed(_) => ("status.conversion_failed", self.error),
            TaskStatus::Processing { .. } => ("status.processing", self.downloading),
            TaskStatus::Connecting => ("status.connecting", self.downloading),
            TaskStatus::Pending | TaskStatus::ConversionQueued => {
                ("status.queued", self.text_muted)
            }
            TaskStatus::Cancelled => ("status.cancelled", self.text_muted),
        }
    }
}

fn truncate_file_name(name: &str, max_chars: usize) -> String {
    if name.chars().count() <= max_chars {
        return name.to_string();
    }
    let extension = name.rsplit_once('.').filter(|(stem, extension)| {
        !stem.is_empty() && !extension.is_empty() && extension.chars().count() + 4 <= max_chars
    });
    let suffix = extension
        .map(|(_, extension)| format!(".{extension}"))
        .unwrap_or_default();
    let keep = max_chars.saturating_sub(3 + suffix.chars().count());
    let prefix: String = name.chars().take(keep).collect();
    format!("{prefix}...{suffix}")
}

fn format_remaining(task: &TaskSnapshot, cx: &App) -> SharedString {
    let remaining = task.total_size.saturating_sub(task.downloaded_bytes);
    let speed = task.speed_bytes_per_sec;
    if remaining == 0 || speed == 0 {
        return cx.lang().get("downloads.calculating");
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

pub struct DownloadsPage;

impl Page for DownloadsPage {
    fn new(_: &mut Context<Self>) -> Self {
        Self
    }

    fn options() -> PageOptions {
        PageOptions::default().on_open(PageAnimation::SlideUpFade)
    }

    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = ViewPalette::from_theme(cx);
        let tasks_model = cx.tasks_model().clone();
        let tasks = tasks_model.read(cx).tasks().to_vec();
        let is_empty = tasks.is_empty();
        let transition = SpringAnimation::new(SpringConfig::new(300.0, 34.0, 1.0)).to(!is_empty);

        let cards =
            (!is_empty).then(|| {
                container_query(move |size, _window, cx| {
                    let columns =
                        (((f32::from(size.width) - 42.0 + 16.0) / 312.0).floor() as u16).max(1);
                    div()
                        .id("downloads-cards-scroll")
                        .size_full()
                        .overflow_y_scroll()
                        .px_5()
                        .pt_5()
                        .pb_12()
                        .child(
                            div()
                                .w_full()
                                .grid()
                                .grid_cols(columns)
                                .content_start()
                                .items_start()
                                .gap_4()
                                .children(tasks.iter().map(|task| {
                                    Self::render_card(task, &tasks_model, palette, cx)
                                })),
                        )
                })
            });
        let empty = Self::render_empty(palette, is_empty, cx);

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
                            .children(cards.map(|cards| cards.opacity(grid_opacity)))
                            .when(!is_empty, |panel| {
                                panel
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .left_0()
                                            .right_0()
                                            .h(px(20.0))
                                            .opacity(grid_opacity)
                                            .bg(linear_gradient(
                                                180.0,
                                                linear_color_stop(palette.surface, 0.0),
                                                linear_color_stop(
                                                    palette.surface.opacity(0.0),
                                                    1.0,
                                                ),
                                            )),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom_0()
                                            .left_0()
                                            .right_0()
                                            .h(px(56.0))
                                            .opacity(grid_opacity)
                                            .bg(linear_gradient(
                                                180.0,
                                                linear_color_stop(
                                                    palette.surface.opacity(0.0),
                                                    0.0,
                                                ),
                                                linear_color_stop(palette.surface, 1.0),
                                            )),
                                    )
                            })
                            .when(is_empty || grid_opacity < 0.99, |panel| {
                                panel.child(empty.opacity(1.0 - grid_opacity))
                            })
                    }),
            )
    }
}

impl DownloadsPage {
    fn render_card(
        task: &TaskSnapshot,
        tasks_model: &Entity<TasksModel>,
        palette: ViewPalette,
        cx: &App,
    ) -> impl IntoElement {
        let task_id = task.id;
        let task_key = task_id.as_u64_pair().0;
        let display_name = truncate_file_name(&task.file_name, 24);
        let size_text = if task.total_size > 0 {
            format_bytes(task.total_size)
        } else {
            format_bytes(task.downloaded_bytes)
        };

        let (status_label, status_color) = palette.status(&task.status);
        let percent = task_progress_percent(task);
        let percent_text = format!("{}%", percent as u32);
        let (bottom_left_text, bottom_left_color): (SharedString, Rgba) =
            if task.status == TaskStatus::Downloading {
                (format_remaining(task, cx), palette.text_muted)
            } else if matches!(task.status, TaskStatus::Processing { .. })
                && matches!(
                    tasks_model.read(cx).ffmpeg_status(),
                    rdm_core::engine::FfmpegStatus::Checking
                        | rdm_core::engine::FfmpegStatus::Downloading { .. }
                )
            {
                (cx.lang().get("ffmpeg.preparing"), status_color)
            } else {
                (cx.lang().get(status_label), status_color)
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
            Button::new(("download-card-retry-conversion", task_key))
                .label(cx.lang().get("downloads.retry_conversion"))
                .rounded(px(24.0))
                .h(px(32.0))
                .on_click(move |_event, _window, cx| {
                model.update(cx, |m, cx| m.retry_conversion(task_id, cx));
            })
        });

        let toggle_action = match task.status {
            TaskStatus::Downloading | TaskStatus::Connecting => {
                Some((true, "icons/pause.svg", cx.lang().get("downloads.pause")))
            }
            TaskStatus::Paused | TaskStatus::Pending | TaskStatus::Failed(_) => {
                Some((false, "icons/resume.svg", cx.lang().get("downloads.resume")))
            }
            _ => None,
        };

        let card_bounds = Rc::new(Cell::new(None));
        let card_bounds_click = Rc::clone(&card_bounds);
        let is_morphed = DownloadInspectorModalState::is_card_morphed(task_id, cx);
        let reveal_spring =
            SpringAnimation::new(SpringConfig::new(360.0, 38.0, 1.0)).to(!is_morphed);

        download_card_surface(palette.surface, palette.surface_hover, palette.border)
            .id(("download-card", task_key))
            .relative()
            .cursor_pointer()
            .w_full()
            .min_w_0()
            .p_4()
            .flex()
            .flex_col()
            .justify_between()
            .gap_3p5()
            .on_click(move |_event, window, cx| {
                DownloadInspectorModalState::open(task_id, card_bounds_click.get(), window, cx);
            })
            .child(track_card_bounds(card_bounds))
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
                            .flex()
                            .gap_2()
                            .child({
                                let (prefix, suffix) = display_name
                                    .split_once("...")
                                    .map(|(prefix, suffix)| {
                                        (prefix.to_string(), format!("...{suffix}"))
                                    })
                                    .unwrap_or_else(|| (display_name.clone(), String::new()));
                                div()
                                    .flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().min_w_0().truncate().child(prefix))
                                    .child(div().flex_shrink_0().child(suffix))
                            })
                            .child(div().flex_shrink_0().child(format!("• {size_text}"))),
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
                                {
                                    let label = cx.lang().get("downloads.copy_url");
                                    cx.new(|_| ActionTooltip::new(label))
                                }
                                .into()
                            })
                            .on_click(move |_event, _window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    url_for_copy.clone(),
                                ));
                                cx.stop_propagation();
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
                            .id(("download-card-actions", task_key))
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_1()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .on_click(|_, _, cx| {
                                cx.stop_propagation();
                            })
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
                                            cx.lang().get("downloads.open_file"),
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
                                            cx.lang().get("downloads.reveal_file"),
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
                                .tooltip(cx.lang().get("downloads.delete"))
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
            .with_spring(
                ("download-card-reveal", task_key),
                reveal_spring,
                |card, phase| card.opacity(phase.interpolate_clamped(0.0, 1.0)),
            )
    }

    fn render_empty(palette: ViewPalette, interactive: bool, cx: &App) -> Div {
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
                    .child(cx.lang().get("downloads.empty")),
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
                    .child(cx.lang().get("downloads.empty_hint")),
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

#[cfg(test)]
mod filename_tests {
    use super::truncate_file_name;

    #[test]
    fn preserves_short_names_and_extensions() {
        assert_eq!(truncate_file_name("rDM.AppImage", 24), "rDM.AppImage");
        let name = truncate_file_name("rDM-1.0.0-x86_64-release.AppImage", 24);
        assert_eq!(name.chars().count(), 24);
        assert!(name.ends_with("...AppImage"));
        assert!(truncate_file_name("á".repeat(30).as_str(), 24).ends_with("..."));
        assert_eq!(
            truncate_file_name("a".repeat(24).as_str(), 24),
            "a".repeat(24)
        );
    }
}
