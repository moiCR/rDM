use gpui::{Rgba, rgb};

use super::Theme;

#[derive(Default, Clone, Copy)]
pub struct DarkTheme;

impl Theme for DarkTheme {
    fn background(&self) -> Rgba {
        rgb(0x101010)
    }

    fn surface(&self) -> Rgba {
        rgb(0x181818)
    }

    fn surface_hover(&self) -> Rgba {
        rgb(0x222222)
    }

    fn border(&self) -> Rgba {
        rgb(0x262626)
    }

    fn text_color(&self) -> Rgba {
        rgb(0xf5f5f5)
    }

    fn text_muted(&self) -> Rgba {
        rgb(0x8a8a8a)
    }

    fn accent(&self) -> Rgba {
        rgb(0x3b82f6)
    }

    fn downloading(&self) -> Rgba {
        rgb(0x0ea5e9)
    }

    fn paused(&self) -> Rgba {
        rgb(0xf59e0b)
    }

    fn completed(&self) -> Rgba {
        rgb(0x22c55e)
    }

    fn error(&self) -> Rgba {
        rgb(0xef4444)
    }
}
