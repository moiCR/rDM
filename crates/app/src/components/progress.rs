use gpui::*;
pub use gpui_kit::component::progress::{Progress, ProgressCircle};

use crate::theme::ActiveTheme;

#[derive(IntoElement)]
pub struct ProgressBar {
    id: ElementId,
    value: f32,
    color: Option<Rgba>,
    track_color: Option<Rgba>,
    height: Pixels,
}

impl ProgressBar {
    pub fn new(id: impl Into<ElementId>, value: f32) -> Self {
        Self {
            id: id.into(),
            value: value.clamp(0.0, 100.0),
            color: None,
            track_color: None,
            height: px(5.0),
        }
    }

    pub fn color(mut self, color: impl Into<Rgba>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn track_color(mut self, track_color: impl Into<Rgba>) -> Self {
        self.track_color = Some(track_color.into());
        self
    }

    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.height = height.into();
        self
    }
}

impl RenderOnce for ProgressBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let fill_color = self.color.unwrap_or_else(|| cx.theme().accent());
        let track = self
            .track_color
            .unwrap_or_else(|| cx.theme().surface_hover());

        let ratio = self.value / 100.0;

        div()
            .id(self.id)
            .w_full()
            .h(self.height)
            .rounded_full()
            .bg(track)
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .rounded_full()
                    .bg(fill_color)
                    .w(relative(ratio)),
            )
    }
}
