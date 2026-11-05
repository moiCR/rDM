pub mod dark;
pub mod light;

use gpui::{App, Context, Rgba};

pub trait Theme: 'static + Send + Sync {
    fn background(&self) -> Rgba;
    fn surface(&self) -> Rgba;
    fn surface_hover(&self) -> Rgba;
    fn border(&self) -> Rgba;
    fn text_color(&self) -> Rgba;
    fn text_muted(&self) -> Rgba;
    fn accent(&self) -> Rgba;

    fn downloading(&self) -> Rgba;
    fn paused(&self) -> Rgba;
    fn completed(&self) -> Rgba;
    fn error(&self) -> Rgba;
}

pub struct ThemeManager {
    active_theme: Box<dyn Theme>,
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self {
            active_theme: Box::new(dark::DarkTheme),
        }
    }
}

impl ThemeManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active(&self) -> &dyn Theme {
        self.active_theme.as_ref()
    }

    pub fn set_theme(&mut self, theme: Box<dyn Theme>) {
        self.active_theme = theme;
    }
}

impl gpui::Global for ThemeManager {}

pub trait ActiveTheme {
    fn theme(&self) -> &dyn Theme;
    fn themes(&self) -> &dyn Theme {
        self.theme()
    }
}

impl<'a, T> ActiveTheme for Context<'a, T> {
    fn theme(&self) -> &dyn Theme {
        self.global::<ThemeManager>().active()
    }
}

impl ActiveTheme for App {
    fn theme(&self) -> &dyn Theme {
        self.global::<ThemeManager>().active()
    }
}
