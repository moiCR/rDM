pub mod dark;
pub mod light;

use gpui::{App, Context, Rgba, rgb};
use gpui_kit::component::{Theme as ComponentTheme, ThemeMode};

#[derive(Clone, Copy)]
pub struct PrimaryColors {
    pub fill: Rgba,
    pub border: Rgba,
    pub text: Rgba,
}

pub trait Theme: 'static + Send + Sync {
    fn background(&self) -> Rgba;
    fn surface(&self) -> Rgba;
    fn input_background(&self) -> Option<Rgba> {
        None
    }
    fn surface_hover(&self) -> Rgba;
    fn border(&self) -> Rgba;
    fn text_color(&self) -> Rgba;
    fn text_muted(&self) -> Rgba;
    fn accent(&self) -> Rgba;
    fn primary(&self) -> PrimaryColors;

    fn downloading(&self) -> Rgba;
    fn paused(&self) -> Rgba;
    fn completed(&self) -> Rgba;
    fn error(&self) -> Rgba;
}

pub fn parse_hex_color(hex: &str) -> Option<Rgba> {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() != 6 {
        return None;
    }
    u32::from_str_radix(clean, 16).ok().map(rgb)
}

pub struct ThemeManager {
    active_theme: Box<dyn Theme>,
    accent_override: Option<Rgba>,
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self {
            active_theme: Box::new(dark::DarkTheme),
            accent_override: None,
        }
    }
}

impl Theme for ThemeManager {
    fn background(&self) -> Rgba {
        self.active_theme.background()
    }

    fn surface(&self) -> Rgba {
        self.active_theme.surface()
    }

    fn input_background(&self) -> Option<Rgba> {
        self.active_theme.input_background()
    }

    fn surface_hover(&self) -> Rgba {
        self.active_theme.surface_hover()
    }

    fn border(&self) -> Rgba {
        self.active_theme.border()
    }

    fn text_color(&self) -> Rgba {
        self.active_theme.text_color()
    }

    fn text_muted(&self) -> Rgba {
        self.active_theme.text_muted()
    }

    fn accent(&self) -> Rgba {
        self.accent_override
            .unwrap_or_else(|| self.active_theme.accent())
    }

    fn primary(&self) -> PrimaryColors {
        self.active_theme.primary()
    }

    fn downloading(&self) -> Rgba {
        self.active_theme.downloading()
    }

    fn paused(&self) -> Rgba {
        self.active_theme.paused()
    }

    fn completed(&self) -> Rgba {
        self.active_theme.completed()
    }

    fn error(&self) -> Rgba {
        self.active_theme.error()
    }
}

impl ThemeManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active(&self) -> &dyn Theme {
        self
    }

    pub fn apply_mode(mode: &str, cx: &mut App) {
        let (theme, component_mode): (Box<dyn Theme>, _) = if mode == "light" {
            (Box::new(light::LightTheme), ThemeMode::Light)
        } else {
            (Box::new(dark::DarkTheme), ThemeMode::Dark)
        };
        cx.global_mut::<Self>().active_theme = theme;
        ComponentTheme::change(component_mode, None, cx);

        if component_mode.is_dark() {
            let dark = dark::DarkTheme;
            ComponentTheme::update(cx, |theme| {
                theme.colors.background = dark.background().into();
                theme.colors.popover = dark.surface().into();
                theme.colors.list_hover = dark.surface_hover().into();
                theme.colors.border = dark.border().into();
                theme.colors.input = dark.border().into();
                theme.colors.foreground = dark.text_color().into();
                theme.colors.muted_foreground = dark.text_muted().into();
            });
        }
    }

    pub fn set_accent(&mut self, color: Option<Rgba>) {
        self.accent_override = color;
    }

    pub fn set_accent_hex(&mut self, hex: &str) {
        self.accent_override = parse_hex_color(hex);
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
