use gpui::{Hsla, hsla};

use super::Theme;

#[derive(Default, Clone, Copy)]
pub struct DarkTheme;

impl Theme for DarkTheme {
    fn background(&self) -> Hsla {
        hsla(220.0 / 360.0, 0.14, 0.08, 1.0)
    }

    fn surface(&self) -> Hsla {
        hsla(220.0 / 360.0, 0.13, 0.12, 1.0)
    }

    fn surface_hover(&self) -> Hsla {
        hsla(220.0 / 360.0, 0.13, 0.16, 1.0)
    }

    fn border(&self) -> Hsla {
        hsla(220.0 / 360.0, 0.12, 0.20, 1.0)
    }

    fn text_color(&self) -> Hsla {
        hsla(0.0, 0.0, 0.96, 1.0)
    }

    fn text_muted(&self) -> Hsla {
        hsla(220.0 / 360.0, 0.08, 0.60, 1.0)
    }

    fn accent(&self) -> Hsla {
        hsla(217.0 / 360.0, 0.91, 0.60, 1.0)
    }

    fn downloading(&self) -> Hsla {
        hsla(199.0 / 360.0, 0.89, 0.48, 1.0)
    }

    fn paused(&self) -> Hsla {
        hsla(38.0 / 360.0, 0.92, 0.50, 1.0)
    }

    fn completed(&self) -> Hsla {
        hsla(142.0 / 360.0, 0.71, 0.45, 1.0)
    }

    fn error(&self) -> Hsla {
        hsla(0.0, 0.84, 0.60, 1.0)
    }
}
