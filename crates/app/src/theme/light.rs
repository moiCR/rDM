use gpui::{Rgba, rgb};

use super::Theme;

#[derive(Default, Clone, Copy)]
pub struct LightTheme;

impl Theme for LightTheme {
    fn background(&self) -> Rgba {
        rgb(0xfaf8f6)
    }

    fn surface(&self) -> Rgba {
        rgb(0xf0eeeb)
    }

    fn surface_hover(&self) -> Rgba {
        rgb(0xe2dfdc)
    }

    fn border(&self) -> Rgba {
        rgb(0xd6d3d0)
    }

    fn text_color(&self) -> Rgba {
        rgb(0x171717)
    }

    fn text_muted(&self) -> Rgba {
        rgb(0x85827e)
    }

    fn accent(&self) -> Rgba {
        rgb(0x2563eb)
    }

    fn downloading(&self) -> Rgba {
        rgb(0x0284c7)
    }

    fn paused(&self) -> Rgba {
        rgb(0xd97706)
    }

    fn completed(&self) -> Rgba {
        rgb(0x16a34a)
    }

    fn error(&self) -> Rgba {
        rgb(0xdc2626)
    }
}
