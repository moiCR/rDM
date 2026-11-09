use gpui::{Rgba, rgb};

use super::{PrimaryColors, Theme};

#[derive(Default, Clone, Copy)]
pub struct DarkTheme;

impl Theme for DarkTheme {
    fn background(&self) -> Rgba {
        rgb(0x060c16)
    }

    fn surface(&self) -> Rgba {
        rgb(0x0b1424)
    }

    fn surface_hover(&self) -> Rgba {
        rgb(0x121f33)
    }

    fn border(&self) -> Rgba {
        rgb(0x1b2a40)
    }

    fn text_color(&self) -> Rgba {
        rgb(0xeaf2ff)
    }

    fn text_muted(&self) -> Rgba {
        rgb(0xaabbd2)
    }

    fn accent(&self) -> Rgba {
        rgb(0x91add4)
    }

    fn primary(&self) -> PrimaryColors {
        PrimaryColors {
            fill: rgb(0x223c61),
            border: rgb(0x536f94),
            text: rgb(0xf2f6fc),
        }
    }

    fn downloading(&self) -> Rgba {
        rgb(0x4e909c)
    }

    fn paused(&self) -> Rgba {
        rgb(0xf4bc64)
    }

    fn completed(&self) -> Rgba {
        rgb(0x6ed9ad)
    }

    fn error(&self) -> Rgba {
        rgb(0xff9c9c)
    }
}
