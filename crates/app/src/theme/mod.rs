pub mod dark;
pub mod light;

use gpui::{App, Context, Hsla};

pub trait Theme: 'static + Send + Sync {
    fn background(&self) -> Hsla;
    fn surface(&self) -> Hsla;
    fn surface_hover(&self) -> Hsla;
    fn border(&self) -> Hsla;
    fn text_color(&self) -> Hsla;
    fn text_muted(&self) -> Hsla;
    fn accent(&self) -> Hsla;

    fn downloading(&self) -> Hsla;
    fn paused(&self) -> Hsla;
    fn completed(&self) -> Hsla;
    fn error(&self) -> Hsla;
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
