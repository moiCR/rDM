use gpui::*;
use rdm_core::task::TaskStatus;

use crate::state::TaskSnapshot;

pub(crate) fn format_bytes(bytes: u64) -> String {
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

pub(crate) fn task_progress_percent(task: &TaskSnapshot) -> f32 {
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

pub(crate) fn download_card_surface(surface: Rgba, raised: Rgba, border: Rgba) -> Div {
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
