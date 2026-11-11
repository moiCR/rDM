use std::collections::BTreeMap;

use gpui::{App, Context, Global, SharedString};
use thiserror::Error;

pub const DEFAULT_LANGUAGE: &str = "es";

#[derive(Debug, Error)]
pub enum LangError {
    #[error("Invalid language TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("Missing language metadata: {0}")]
    MissingMetadata(&'static str),
    #[error("Language value must be a string: {0}")]
    InvalidValue(String),
    #[error("Duplicate language key: {0}")]
    DuplicateKey(String),
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),
}

struct LanguageBundle {
    code: String,
    name: String,
    strings: BTreeMap<String, SharedString>,
}

impl LanguageBundle {
    fn parse(content: &str) -> Result<Self, LangError> {
        let mut table = toml::from_str::<toml::Table>(content)?;
        let mut metadata = |key| {
            table
                .remove(key)
                .and_then(|value| value.as_str().map(str::to_owned))
                .filter(|value| !value.trim().is_empty())
                .ok_or(LangError::MissingMetadata(key))
        };
        let code = metadata("code")?;
        let name = metadata("name")?;
        let mut strings = BTreeMap::new();
        Self::flatten(table, "", &mut strings)?;
        Ok(Self {
            code,
            name,
            strings,
        })
    }

    fn flatten(
        table: toml::Table,
        prefix: &str,
        strings: &mut BTreeMap<String, SharedString>,
    ) -> Result<(), LangError> {
        for (key, value) in table {
            let path = if prefix.is_empty() {
                key
            } else {
                format!("{prefix}.{key}")
            };
            match value {
                toml::Value::String(value) => {
                    if strings.insert(path.clone(), value.into()).is_some() {
                        return Err(LangError::DuplicateKey(path));
                    }
                }
                toml::Value::Table(table) => Self::flatten(table, &path, strings)?,
                _ => return Err(LangError::InvalidValue(path)),
            }
        }
        Ok(())
    }
}

pub struct Lang {
    languages: BTreeMap<String, LanguageBundle>,
    current: String,
}

impl Lang {
    pub fn new(language: &str) -> Result<Self, LangError> {
        let mut languages = BTreeMap::new();
        for content in [include_str!("es.toml"), include_str!("en.toml")] {
            let bundle = LanguageBundle::parse(content)?;
            languages.insert(bundle.code.clone(), bundle);
        }
        let mut lang = Self {
            languages,
            current: DEFAULT_LANGUAGE.to_owned(),
        };
        let _ = lang.set_language(language);
        Ok(lang)
    }

    pub fn code(&self) -> &str {
        &self.current
    }

    pub fn languages(&self) -> impl Iterator<Item = (&str, &str)> {
        self.languages
            .values()
            .map(|bundle| (bundle.code.as_str(), bundle.name.as_str()))
    }

    pub fn set_language(&mut self, code: &str) -> Result<(), LangError> {
        if !self.languages.contains_key(code) {
            return Err(LangError::UnsupportedLanguage(code.to_owned()));
        }
        self.current = code.to_owned();
        Ok(())
    }

    pub fn get(&self, key: &str) -> SharedString {
        self.languages
            .get(&self.current)
            .and_then(|bundle| bundle.strings.get(key))
            .or_else(|| {
                self.languages
                    .get(DEFAULT_LANGUAGE)
                    .and_then(|bundle| bundle.strings.get(key))
            })
            .cloned()
            .unwrap_or_else(|| key.to_owned().into())
    }

    pub fn get_with(&self, key: &str, arguments: &[(&str, &str)]) -> SharedString {
        let template = self.get(key);
        let mut remaining = template.as_ref();
        let mut output = String::with_capacity(template.len());
        while let Some(start) = remaining.find('{') {
            output.push_str(&remaining[..start]);
            let Some(end) = remaining[start..].find('}').map(|end| end + start) else {
                output.push_str(&remaining[start..]);
                return output.into();
            };
            let name = &remaining[start + 1..end];
            match arguments.iter().find(|(key, _)| *key == name) {
                Some((_, value)) => output.push_str(value),
                None => output.push_str(&remaining[start..=end]),
            }
            remaining = &remaining[end + 1..];
        }
        output.push_str(remaining);
        output.into()
    }
}

impl Global for Lang {}

pub trait ActiveLang {
    fn lang(&self) -> &Lang;
}

impl ActiveLang for App {
    fn lang(&self) -> &Lang {
        self.global::<Lang>()
    }
}

impl<T> ActiveLang for Context<'_, T> {
    fn lang(&self) -> &Lang {
        self.global::<Lang>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_keys_and_interpolation_variables_exist_in_every_catalog() -> Result<(), LangError> {
        let lang = Lang::new("es")?;
        let sources = [
            include_str!("../views/dashboard.rs"),
            include_str!("../views/dashboard/calendar.rs"),
            include_str!("../views/downloads.rs"),
            include_str!("../views/new_download.rs"),
            include_str!("../views/new_download/widgets.rs"),
            include_str!("../views/settings.rs"),
            include_str!("../layout/sidebar_item.rs"),
            include_str!("../layout/titlebar.rs"),
            include_str!("../state.rs"),
        ];
        for source in sources {
            let source = source.split("#[cfg(test)]").next().unwrap_or_default();
            for prefix in [
                "common",
                "navigation",
                "titlebar",
                "settings",
                "dashboard",
                "downloads",
                "new_download",
                "validation",
                "status",
                "updater",
                "calendar",
            ] {
                let needle = format!("\"{prefix}.");
                let mut remaining = source;
                while let Some(start) = remaining.find(&needle) {
                    let tail = &remaining[start + 1..];
                    let Some(end) = tail.find('"') else {
                        break;
                    };
                    let key = &tail[..end];
                    for bundle in lang.languages.values() {
                        assert!(
                            bundle.strings.contains_key(key),
                            "{} is missing {key}",
                            bundle.code
                        );
                    }
                    remaining = &tail[end + 1..];
                }
            }
        }
        let placeholders = |text: &str| {
            text.split('{')
                .skip(1)
                .filter_map(|part| part.split_once('}').map(|(key, _)| key.to_owned()))
                .collect::<std::collections::BTreeSet<_>>()
        };
        let spanish = lang
            .languages
            .get("es")
            .ok_or_else(|| LangError::UnsupportedLanguage("es".into()))?;
        for bundle in lang.languages.values() {
            for (key, value) in &bundle.strings {
                let fallback = spanish
                    .strings
                    .get(key)
                    .ok_or_else(|| LangError::InvalidValue(key.clone()))?;
                assert_eq!(
                    placeholders(value),
                    placeholders(fallback),
                    "{}: {key}",
                    bundle.code
                );
            }
        }
        Ok(())
    }

    #[test]
    fn catalogs_have_the_same_keys() -> Result<(), LangError> {
        let lang = Lang::new("es")?;
        let spanish = lang
            .languages
            .get("es")
            .ok_or_else(|| LangError::UnsupportedLanguage("es".into()))?;
        let english = lang
            .languages
            .get("en")
            .ok_or_else(|| LangError::UnsupportedLanguage("en".into()))?;
        assert_eq!(
            spanish.strings.keys().collect::<Vec<_>>(),
            english.strings.keys().collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn selects_language_and_falls_back_per_key() -> Result<(), LangError> {
        let mut lang = Lang::new("en")?;
        assert_eq!(lang.get("common.cancel").as_ref(), "Cancel");
        let english = lang
            .languages
            .get_mut("en")
            .ok_or_else(|| LangError::UnsupportedLanguage("en".into()))?;
        english.strings.remove("common.cancel");
        english.strings.insert("common.close".into(), "".into());
        assert_eq!(lang.get("common.cancel").as_ref(), "Cancelar");
        assert_eq!(lang.get("common.close").as_ref(), "");
        assert_eq!(lang.get("cancel").as_ref(), "cancel");
        assert_eq!(lang.get("missing.key").as_ref(), "missing.key");
        assert!(lang.set_language("unknown").is_err());
        assert_eq!(lang.code(), "en");
        lang.set_language("es")?;
        assert_eq!(lang.get("common.close").as_ref(), "Cerrar");
        assert_eq!(Lang::new("unknown")?.code(), DEFAULT_LANGUAGE);
        Ok(())
    }

    #[test]
    fn interpolates_without_reinterpreting_argument_values() -> Result<(), LangError> {
        let lang = Lang::new("es")?;
        assert_eq!(
            lang.get_with("titlebar.update_available", &[("version", "1.2.3")])
                .as_ref(),
            "v1.2.3 disponible"
        );
        assert_eq!(
            lang.get_with("titlebar.update_available", &[]).as_ref(),
            "v{version} disponible"
        );
        assert_eq!(
            lang.get_with(
                "titlebar.update_available",
                &[("version", "{percent}"), ("percent", "90")]
            )
            .as_ref(),
            "v{percent} disponible"
        );
        assert_eq!(
            lang.get_with("titlebar.updating", &[("percent", "90")])
                .as_ref(),
            "Actualizando... 90%"
        );
        Ok(())
    }

    #[test]
    fn rejects_malformed_or_ambiguous_catalogs() {
        assert!(matches!(
            LanguageBundle::parse("invalid = ["),
            Err(LangError::Parse(_))
        ));
        assert!(matches!(
            LanguageBundle::parse("code = 'es'"),
            Err(LangError::MissingMetadata("name"))
        ));
        assert!(
            matches!(LanguageBundle::parse("code = 'es'\nname = 'Español'\n[panel]\ntitle = 42"), Err(LangError::InvalidValue(key)) if key == "panel.title")
        );
        assert!(
            matches!(LanguageBundle::parse("code = 'es'\nname = 'Español'\n'panel.title' = 'One'\n[panel]\ntitle = 'Two'"), Err(LangError::DuplicateKey(key)) if key == "panel.title")
        );
    }
}
