use gpui::*;
use rdm_core::task::TaskStatus;

use crate::state::TaskSnapshot;

pub(crate) fn truncate_file_name(name: &str, max_chars: usize) -> String {
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
