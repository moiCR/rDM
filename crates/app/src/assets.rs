use std::{borrow::Cow, path::PathBuf};

use gpui::{AssetSource, SharedString};

pub struct Assets {
    base_dir: PathBuf,
}

impl Default for Assets {
    fn default() -> Self {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("public");
        let base_dir = if manifest_dir.exists() {
            manifest_dir
        } else {
            PathBuf::from("public")
        };
        Self { base_dir }
    }
}

impl Assets {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let clean_path = path.trim_start_matches('/');
        let file_path = self.base_dir.join(clean_path);
        if file_path.exists() {
            let bytes = std::fs::read(&file_path)?;
            Ok(Some(Cow::Owned(bytes)))
        } else {
            Ok(None)
        }
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let clean_path = path.trim_start_matches('/');
        let dir_path = self.base_dir.join(clean_path);
        let mut results = Vec::new();

        if dir_path.is_dir() {
            for entry in std::fs::read_dir(dir_path)? {
                let entry = entry?;
                if let Some(name) = entry.file_name().to_str() {
                    results.push(SharedString::from(name.to_string()));
                }
            }
        }

        Ok(results)
    }
}
