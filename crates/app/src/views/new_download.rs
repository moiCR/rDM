use std::{collections::VecDeque, path::PathBuf};

use gpui::*;
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use rdm_core::{InterceptedDownload, task::PostProcessAction};
use uuid::Uuid;

use crate::{
    state::{CoreContext, DownloadRequest},
    theme::ActiveTheme,
};

mod widgets;

#[derive(Default)]
pub struct NewDownloadModalState {
    form: Option<Entity<DownloadForm>>,
    pending: VecDeque<InterceptedDownload>,
}

impl Global for NewDownloadModalState {}

struct DownloadForm {
    url: Entity<TextareaState>,
    file_name: Entity<InputState>,
    connections: Entity<InputState>,
    directory: PathBuf,
    referer: Option<String>,
    post_process: Option<PostProcessAction>,
    advanced: bool,
    submitting: bool,
    error: Option<String>,
    created_task: Option<Uuid>,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

fn validate_url(text: &str) -> Result<&str, &'static str> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Introduce una URL de descarga.");
    }
    if text
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err("Introduce una sola URL, sin espacios ni saltos de línea.");
    }
    let url = gpui::http_client::Url::parse(text).map_err(|_| "La URL no es válida.")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Usa una URL HTTP o HTTPS.");
    }
    Ok(text)
}

fn validate_file_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Ok(());
    }
    if name.trim().is_empty()
        || name.encode_utf16().count() > 255
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|character| character.is_control() || r#"<>:"/\|?*"#.contains(character))
    {
        return Err("Usa un nombre de archivo válido, sin rutas ni caracteres reservados.");
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    if reserved {
        return Err("Ese nombre está reservado por Windows.");
    }
    Ok(())
}

fn validate_connections(text: &str) -> Result<usize, &'static str> {
    text.trim()
        .parse::<usize>()
        .ok()
        .filter(|value| (1..=32).contains(value))
        .ok_or("Introduce un número de conexiones entre 1 y 32.")
}

impl DownloadForm {
    fn new(item: Option<InterceptedDownload>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = cx.core().config().config();
        let url = cx.new(|cx| {
            let mut input =
                TextareaState::new(window, cx).placeholder("Introduce una URL de descarga…");
            if let Some(item) = &item {
                input.set_value(item.url.clone(), window, cx);
            }
            input.focus(window, cx);
            input
        });
        let file_name = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Nombre automático");
            if let Some(item) = &item {
                input.set_value(item.file_name.clone(), window, cx);
            }
            input
        });
        let connections = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value(
                item.as_ref()
                    .map_or(config.default_connections, |item| item.connections)
                    .to_string(),
                window,
                cx,
            );
            input
        });
        let subscriptions = vec![
            cx.subscribe(&url, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.error = None;
                    cx.notify();
                }
            }),
            cx.subscribe(&file_name, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.error = None;
                    cx.notify();
                }
            }),
            cx.subscribe(&connections, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.error = None;
                    cx.notify();
                }
            }),
        ];
        Self {
            url,
            file_name,
            connections,
            directory: item
                .as_ref()
                .map_or(config.download_dir, |item| item.download_dir.clone()),
            referer: item.as_ref().and_then(|item| item.referer.clone()),
            post_process: item.and_then(|item| item.post_process),
            advanced: false,
            submitting: false,
            error: None,
            created_task: None,
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.submitting {
            return;
        }
        let next = cx
            .try_global::<NewDownloadModalState>()
            .and_then(|state| state.pending.front().cloned());
        cx.update_global::<NewDownloadModalState, _>(|state, _| {
            state.form = None;
            state.pending.pop_front();
        });
        window.blur(cx);
        if let Some(item) = next {
            NewDownloadModal::open(Some(item), window, cx);
        }
        window.refresh();
    }

    fn choose_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.submitting || self.created_task.is_some() {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Selecciona la carpeta de descargas".into()),
        });
        cx.spawn_in(window, async move |entity, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = entity.update(cx, |this, cx| {
                    if !this.submitting && this.created_task.is_none() {
                        this.directory = path;
                        this.error = None;
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.submitting {
            return;
        }
        let url_text = self.url.read(cx).value().to_string();
        let name = self.file_name.read(cx).value().to_string();
        let connections_text = self.connections.read(cx).value().to_string();
        let (Ok(url), Ok(()), Ok(connections)) = (
            validate_url(&url_text),
            validate_file_name(&name),
            validate_connections(&connections_text),
        ) else {
            return;
        };
        if self.directory.as_os_str().is_empty() {
            return;
        }
        self.submitting = true;
        self.error = None;
        let request = DownloadRequest {
            url: url.to_owned(),
            output_path: Some(if name.is_empty() {
                self.directory.clone()
            } else {
                self.directory.join(name)
            }),
            directory: Some(self.directory.clone()),
            connections: Some(connections),
            referer: self.referer.clone(),
            post_process: self.post_process.clone(),
            retry_task: self.created_task,
        };
        let task = cx
            .tasks_model()
            .clone()
            .update(cx, |model, cx| model.start_configured_download(request, cx));
        cx.notify();
        cx.spawn_in(window, async move |entity, cx| {
            let result = task.await;
            let _ = entity.update_in(cx, |this, window, cx| {
                this.submitting = false;
                this.created_task = result.task_id;
                this.error = result.error;
                if this.error.is_none() {
                    this.close(window, cx);
                } else {
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

#[derive(IntoElement)]
pub struct NewDownloadModal;

impl NewDownloadModal {
    pub fn new() -> Self {
        Self
    }

    pub fn open(item: Option<InterceptedDownload>, window: &mut Window, cx: &mut App) {
        if let Some(form) = cx
            .try_global::<NewDownloadModalState>()
            .and_then(|state| state.form.clone())
        {
            if let Some(item) = item {
                cx.update_global::<NewDownloadModalState, _>(|state, _| {
                    state.pending.push_back(item)
                });
            } else {
                let input = form.read(cx).url.clone();
                input.update(cx, |input, cx| input.focus(window, cx));
            }
            return;
        }
        let form = cx.new(|cx| DownloadForm::new(item, window, cx));
        if cx.try_global::<NewDownloadModalState>().is_none() {
            cx.set_global(NewDownloadModalState::default());
        }
        cx.update_global::<NewDownloadModalState, _>(|state, _| state.form = Some(form));
        window.refresh();
    }
}

impl Default for NewDownloadModal {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for NewDownloadModal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div().absolute().inset_0().size_full().children(
            cx.try_global::<NewDownloadModalState>()
                .and_then(|state| state.form.clone()),
        )
    }
}

#[derive(IntoElement)]
pub struct NewDownloadView;

impl NewDownloadView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NewDownloadView {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for NewDownloadView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background())
            .text_color(cx.theme().text_color())
            .p_8()
            .child("New Download")
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_connections, validate_file_name, validate_url};

    #[test]
    fn accepts_one_http_url_and_preserves_encoded_path() {
        let url = "https://example.com/a%20b.zip?token=a%2Bb";
        assert_eq!(validate_url(&format!(" \n{url}\r\n")), Ok(url));
        for invalid in [
            "",
            "https://",
            "magnet:?xt=urn:btih:abc",
            "file:///C:/test",
            "https://a.test\nhttps://b.test",
            "https://a.test/a b",
            "https://a.test/\0b",
        ] {
            assert!(validate_url(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn rejects_paths_reserved_windows_names_and_invalid_file_names() {
        for invalid in [
            "../file",
            "..\\file",
            "C:\\file",
            "NUL.txt",
            "COM1.zip",
            "lpt²",
            "CON .txt",
            "file:stream",
            "file.",
            "file ",
            "\0",
            " ",
        ] {
            assert!(validate_file_name(invalid).is_err(), "{invalid:?}");
        }
        assert!(validate_file_name(&"a".repeat(256)).is_err());
        for valid in ["", "archivo.zip", "vídeo final.mp4", ".env", "COM10.txt"] {
            assert!(validate_file_name(valid).is_ok(), "{valid}");
        }
    }

    #[test]
    fn validates_connection_limits_before_submission() {
        assert_eq!(validate_connections(" 1 "), Ok(1));
        assert_eq!(validate_connections("32"), Ok(32));
        for invalid in ["", "0", "33", "-1", "1.5", "abc"] {
            assert!(validate_connections(invalid).is_err());
        }
    }
}
