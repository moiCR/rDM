use serde::{Deserialize, Serialize};

fn default_theme() -> String {
    "dark".to_string()
}

fn default_sidebar_style() -> String {
    "classic".to_string()
}

fn default_accent_color() -> String {
    "#3b82f6".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct UIConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_sidebar_style")]
    pub sidebar_style: String,
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            sidebar_style: default_sidebar_style(),
            accent_color: default_accent_color(),
        }
    }
}
