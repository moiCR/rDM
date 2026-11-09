use gpui::{Rgba, rgb};

use super::{PrimaryColors, Theme};

#[derive(Default, Clone, Copy)]
pub struct LightTheme;

impl Theme for LightTheme {
    fn background(&self) -> Rgba {
        rgb(0xf0f4fa)
    }

    fn surface(&self) -> Rgba {
        rgb(0xffffff)
    }

    fn surface_hover(&self) -> Rgba {
        rgb(0xe7effa)
    }

    fn border(&self) -> Rgba {
        rgb(0xd3dfee)
    }

    fn text_color(&self) -> Rgba {
        rgb(0x152840)
    }

    fn text_muted(&self) -> Rgba {
        rgb(0x516780)
    }

    fn accent(&self) -> Rgba {
        self.primary().fill
    }

    fn primary(&self) -> PrimaryColors {
        let fill = rgb(0x19365f);
        PrimaryColors {
            fill,
            border: fill,
            text: rgb(0xffffff),
        }
    }

    fn downloading(&self) -> Rgba {
        rgb(0x006d83)
    }

    fn paused(&self) -> Rgba {
        rgb(0x8f530c)
    }

    fn completed(&self) -> Rgba {
        rgb(0x166b50)
    }

    fn error(&self) -> Rgba {
        rgb(0xac3443)
    }
}
