use gpui::*;
use uuid::Uuid;

use crate::{
    components::{Modal, PillButton},
    lang::ActiveLang,
    state::CoreContext,
    theme::ActiveTheme,
};

#[derive(Default, Clone)]
pub struct DeleteDownloadModalState {
    pub open: bool,
    pub task_id: Option<Uuid>,
    pub file_name: String,
    pub delete_file: bool,
}

impl Global for DeleteDownloadModalState {}

#[derive(IntoElement)]
pub struct DeleteDownloadModal;

impl DeleteDownloadModal {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeleteDownloadModal {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DeleteDownloadModal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let modal_state = cx
            .try_global::<DeleteDownloadModalState>()
            .cloned()
            .unwrap_or_default();

        let is_open = modal_state.open && modal_state.task_id.is_some();
        let task_id = modal_state.task_id;
        let file_name = modal_state.file_name.clone();
        let delete_file = modal_state.delete_file;
        let toggle_file_name = modal_state.file_name.clone();

        let theme = cx.theme();
        let background = theme.background();
        let surface_hover = theme.surface_hover();
        let border = theme.border();
        let text_color = theme.text_color();
        let text_muted = theme.text_muted();

        div()
            .id("delete-download-modal-root")
            .absolute()
            .inset_0()
            .size_full()
            .child(
                Modal::new("delete-download-modal")
                    .open(is_open)
                    .width(px(520.0))
                    .max_height(px(320.0))
                    .title(cx.lang().get("downloads.delete"))
                    .on_close(move |window, cx| {
                        cx.set_global(DeleteDownloadModalState {
                            open: false,
                            task_id: None,
                            file_name: String::new(),
                            delete_file: false,
                        });
                        window.refresh();
                    })
                    .footer(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2p5()
                            .pt_2()
                            .child(
                                PillButton::new(
                                    "btn-cancel-delete-download",
                                    cx.lang().get("common.cancel"),
                                )
                                .selected(false)
                                .on_click(
                                    move |_event, window, cx| {
                                        cx.set_global(DeleteDownloadModalState {
                                            open: false,
                                            task_id: None,
                                            file_name: String::new(),
                                            delete_file: false,
                                        });
                                        window.refresh();
                                    },
                                ),
                            )
                            .child(
                                PillButton::new(
                                    "btn-confirm-delete-download",
                                    cx.lang().get("common.delete"),
                                )
                                .selected(true)
                                .on_click(
                                    move |_event, window, cx| {
                                        if let Some(id) = task_id {
                                            let model = cx.tasks_model().clone();
                                            model.update(cx, |m, cx| {
                                                m.delete_task(id, delete_file, cx);
                                            });
                                        }
                                        cx.set_global(DeleteDownloadModalState {
                                            open: false,
                                            task_id: None,
                                            file_name: String::new(),
                                            delete_file: false,
                                        });
                                        window.refresh();
                                    },
                                ),
                            ),
                    )
                    .child(
                        div().flex().flex_col().gap_5().child(
                            div()
                                .id("checkbox-delete-file-from-disk")
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_6()
                                .py_2()
                                .cursor_pointer()
                                .on_click(move |_event, window, cx| {
                                    cx.set_global(DeleteDownloadModalState {
                                        open: true,
                                        task_id,
                                        file_name: toggle_file_name.clone(),
                                        delete_file: !delete_file,
                                    });
                                    window.refresh();
                                })
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
                                                .text_color(text_color)
                                                .truncate()
                                                .child(file_name),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(text_muted)
                                                .child(cx.lang().get("downloads.delete_file")),
                                        ),
                                )
                                .child(
                                    div()
                                        .size(px(18.0))
                                        .rounded(px(5.0))
                                        .border_1()
                                        .border_color(if delete_file { text_color } else { border })
                                        .bg(if delete_file {
                                            surface_hover
                                        } else {
                                            background
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .flex_shrink_0()
                                        .children(delete_file.then(|| {
                                            div().size(px(8.0)).rounded(px(2.0)).bg(text_color)
                                        })),
                                ),
                        ),
                    ),
            )
    }
}
