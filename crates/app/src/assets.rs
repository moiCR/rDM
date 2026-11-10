use std::{borrow::Cow, path::PathBuf};

use gpui::{App, AssetSource, SharedString};

pub struct Assets {
    base_dir: PathBuf,
}

impl Default for Assets {
    fn default() -> Self {
        let exe_public = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|dir| dir.join("public")));

        let base_dir = if let Some(exe_dir) = exe_public.filter(|path| path.is_dir()) {
            exe_dir
        } else {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("public");
            if manifest_dir.exists() {
                manifest_dir
            } else {
                PathBuf::from("public")
            }
        };
        Self { base_dir }
    }
}

impl Assets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_fonts(&self, cx: &App) {
        let Ok(font_files) = self.list("fonts") else {
            return;
        };

        let mut fonts = Vec::new();
        for file in font_files {
            if file.ends_with(".ttf")
                && let Ok(Some(bytes)) = self.load(&format!("fonts/{file}"))
            {
                fonts.push(bytes);
            }
        }

        if !fonts.is_empty() {
            let _ = cx.text_system().add_fonts(fonts);
        }
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
