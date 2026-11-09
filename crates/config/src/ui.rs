use serde::{Deserialize, Serialize};

fn default_theme() -> String {
    "dark".to_string()
}

fn default_sidebar_style() -> String {
    "classic".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct UIConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_sidebar_style")]
    pub sidebar_style: String,
    pub accent_color: String,
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            sidebar_style: default_sidebar_style(),
            accent_color: String::new(),
        }
    }
}

impl UIConfig {
    pub fn accent_override(&self) -> Option<&str> {
        let color = self.accent_color.trim();
        let is_legacy_default = color.trim_start_matches('#').eq_ignore_ascii_case("0088ff");
        (!color.is_empty() && !is_legacy_default).then_some(color)
    }
}

#[cfg(test)]
mod tests {
    use super::UIConfig;

    #[test]
    fn accent_override_distinguishes_theme_defaults_from_custom_colors()
    -> Result<(), serde_json::Error> {
        let implicit: UIConfig = serde_json::from_str(r#"{"theme":"light"}"#)?;
        let legacy: UIConfig = serde_json::from_str(r##"{"accent_color":"#0088FF"}"##)?;
        let custom: UIConfig = serde_json::from_str(r##"{"accent_color":"#ef4444"}"##)?;

        assert_eq!(implicit.accent_override(), None);
        assert_eq!(legacy.accent_override(), None);
        assert_eq!(custom.accent_override(), Some("#ef4444"));
        Ok(())
    }
}
