use std::path::PathBuf;

use gpui::*;
use gpui_kit::component::input::{Input, InputState};
use rdm_core::InterceptedDownload;
use uuid::Uuid;

use crate::{
    components::{ActionTooltip, Modal, PillButton},
    state::CoreContext,
    theme::ActiveTheme,
};

#[derive(Default, Clone)]
pub struct NewDownloadModalState {
    pub open: bool,
    pub pending: Option<InterceptedDownload>,
}

impl Global for NewDownloadModalState {}

struct NewDownloadInputs {
    file_name: Entity<InputState>,
}

#[derive(Clone, Copy)]
struct ModalPalette {
    text_color: Rgba,
    text_muted: Rgba,
}

fn format_file_size(bytes: u64) -> String {
    if bytes == 0 {
        return "Tamaño desconocido".to_string();
    }
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

fn extract_host_or_url(url: &str) -> String {
    if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        && let Some(host) = rest.split('/').next()
        && !host.is_empty()
    {
        return host.to_string();
    }
    url.to_string()
}

#[derive(IntoElement)]
pub struct NewDownloadModal;

impl NewDownloadModal {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NewDownloadModal {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for NewDownloadModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let modal_state = cx
            .try_global::<NewDownloadModalState>()
            .cloned()
            .unwrap_or_default();

        let is_open = modal_state.open && modal_state.pending.is_some();
        let item = modal_state
            .pending
            .clone()
            .unwrap_or_else(|| InterceptedDownload {
                id: Uuid::nil(),
                url: String::new(),
                file_name: "download.bin".to_string(),
                total_size: 0,
                content_type: None,
                supports_ranges: true,
                referer: None,
                connections: 8,
                download_dir: PathBuf::from("."),
            });

        let item_for_inputs = item.clone();
        let inputs_state = window.use_keyed_state(
            ("new-download-modal-inputs", item.id.as_u64_pair().0),
            cx,
            move |window, cx| {
                let file_name = cx.new(|cx| {
                    let mut state = InputState::new(window, cx).placeholder("archivo.bin");
                    state.set_value(item_for_inputs.file_name.clone(), window, cx);
                    state
                });

                NewDownloadInputs { file_name }
            },
        );

        let inputs = inputs_state.read(cx);

        let palette = ModalPalette {
            text_color: cx.theme().text_color(),
            text_muted: cx.theme().text_muted(),
        };

        let size_text = format_file_size(item.total_size);
        let host_text = extract_host_or_url(&item.url);
        let info_subtitle = if host_text.is_empty() {
            size_text
        } else {
            format!("{size_text}  •  {host_text}")
        };

        let url_for_copy = item.url.clone();
        let confirm_url = item.url.clone();
        let confirm_referer = item.referer.clone();
        let confirm_default_name = item.file_name.clone();
        let confirm_default_dir = item.download_dir.clone();
        let confirm_default_conn = item.connections;
        let confirm_fn_entity = inputs.file_name.clone();

        div()
            .id("new-download-modal-root")
            .absolute()
            .inset_0()
            .size_full()
            .child(
                Modal::new("new-download-modal")
                    .open(is_open)
                    .width(px(500.0))
                    .max_height(px(300.0))
                    .title("Nueva descarga")
                    .on_close(move |window, cx| {
                        cx.set_global(NewDownloadModalState {
                            open: false,
                            pending: None,
                        });
                        window.refresh();
                    })
                    .footer(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2p5()
                            .pt_1()
                            .child(
                                PillButton::new("btn-cancel-new-download", "Cancelar")
                                    .selected(false)
                                    .on_click(move |_event, window, cx| {
                                        cx.set_global(NewDownloadModalState {
                                            open: false,
                                            pending: None,
                                        });
                                        window.refresh();
                                    }),
                            )
                            .child(
                                PillButton::new("btn-confirm-new-download", "Descargar")
                                    .selected(true)
                                    .on_click(move |_event, window, cx| {
                                        let raw_name =
                                            confirm_fn_entity.read(cx).value().trim().to_string();
                                        let final_name = if raw_name.is_empty() {
                                            confirm_default_name.clone()
                                        } else {
                                            raw_name
                                        };

                                        let output_path = confirm_default_dir.join(final_name);
                                        let model = cx.tasks_model().clone();
                                        model.update(cx, |m, cx| {
                                            m.start_configured_download(
                                                confirm_url.clone(),
                                                Some(output_path),
                                                Some(confirm_default_conn),
                                                confirm_referer.clone(),
                                                cx,
                                            );
                                        });

                                        cx.set_global(NewDownloadModalState {
                                            open: false,
                                            pending: None,
                                        });
                                        window.refresh();
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_6()
                            .py_1()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_size(px(14.0))
                                            .text_color(palette.text_color)
                                            .child("Archivo"),
                                    )
                                    .child(
                                        div()
                                            .id("new-download-copy-url")
                                            .text_xs()
                                            .text_color(palette.text_muted)
                                            .truncate()
                                            .cursor_pointer()
                                            .hover(move |s| {
                                                s.text_color(palette.text_color).underline()
                                            })
                                            .tooltip(|_, cx| {
                                                cx.new(|_| {
                                                    ActionTooltip::new("Clic para copiar enlace")
                                                })
                                                .into()
                                            })
                                            .on_click(move |_event, _window, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    url_for_copy.clone(),
                                                ));
                                            })
                                            .child(info_subtitle),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(210.0))
                                    .on_mouse_down(MouseButton::Left, {
                                        let fn_entity = inputs.file_name.clone();
                                        move |_, window, cx| {
                                            fn_entity.update(cx, |s, cx| s.focus(window, cx));
                                        }
                                    })
                                    .child(
                                        Input::new(&inputs.file_name)
                                            .id("input-new-download-filename")
                                            .cleanable(false)
                                            .h(px(36.0))
                                            .w_full(),
                                    ),
                            ),
                    ),
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
