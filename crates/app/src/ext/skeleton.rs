use gpui::*;

pub trait DivSkeletonExt {
    fn skeleton(self, loading: bool) -> Self;
}

impl DivSkeletonExt for Div {
    fn skeleton(self, loading: bool) -> Self {
        if loading {
            self.bg(rgb(0x2a2a2a))
                .text_color(transparent_black())
                .border_color(rgb(0x333333))
                .opacity(0.7)
        } else {
            self
        }
    }
}
