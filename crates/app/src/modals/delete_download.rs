use gpui::*;
use gpui_kit::component::{button::ButtonVariants, switch::Switch};
use uuid::Uuid;

use crate::{
    components::{Button, Modal},
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

    fn close(window: &mut Window, cx: &mut App) {
        cx.set_global(DeleteDownloadModalState {
            open: false,
            task_id: None,
            file_name: String::new(),
            delete_file: false,
        });
        window.blur(cx);
        window.refresh();
    }

    fn set_delete_file(
        task_id: Option<Uuid>,
        file_name: String,
        delete_file: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        cx.set_global(DeleteDownloadModalState {
            open: true,
            task_id,
            file_name,
            delete_file,
        });
        window.refresh();
    }
}

impl Default for DeleteDownloadModal {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DeleteDownloadModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let modal_state = cx
            .try_global::<DeleteDownloadModalState>()
            .cloned()
            .unwrap_or_default();

        let is_open = modal_state.open && modal_state.task_id.is_some();
        let task_id = modal_state.task_id;
        let file_name = modal_state.file_name.clone();
        let delete_file = modal_state.delete_file;
        let row_file_name = modal_state.file_name.clone();
        let switch_file_name = modal_state.file_name;

        let theme = cx.theme();
        let field_background = theme
            .input_background()
            .unwrap_or_else(|| theme.surface_hover());
        let border = theme.border();
        let text_color = theme.text_color();
        let text_muted = theme.text_muted();
        let viewport = window.viewport_size();

        div()
            .id("delete-download-modal-root")
            .absolute()
            .inset_0()
            .size_full()
            .capture_key_down(move |event: &KeyDownEvent, window, cx| {
                if is_open && event.keystroke.key == "escape" {
                    Self::close(window, cx);
                    cx.stop_propagation();
                }
            })
            .child(
                Modal::new("delete-download-modal")
                    .open(is_open)
                    .compact()
                    .fit_content()
                    .width((viewport.width - px(32.0)).clamp(px(0.0), px(480.0)))
                    .max_height((viewport.height - px(32.0)).max(px(0.0)))
                    .header(
                        div().flex().flex_none().items_center().px_1().child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text_color)
                                .child(cx.lang().get("downloads.delete")),
                        ),
                    )
                    .on_close(Self::close)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .rounded(px(24.0))
                            .bg(field_background)
                            .border_1()
                            .border_color(border)
                            .px_4()
                            .py_3()
                            .child(
                                div()
                                    .id("delete-download-file-label")
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .cursor_pointer()
                                    .on_click(move |_event, window, cx| {
                                        Self::set_delete_file(
                                            task_id,
                                            row_file_name.clone(),
                                            !delete_file,
                                            window,
                                            cx,
                                        );
                                    })
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
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
                                Switch::new("checkbox-delete-file-from-disk")
                                    .checked(delete_file)
                                    .accessibility_label(cx.lang().get("downloads.delete_file"))
                                    .on_change(move |checked, window, cx| {
                                        Self::set_delete_file(
                                            task_id,
                                            switch_file_name.clone(),
                                            *checked,
                                            window,
                                            cx,
                                        );
                                    }),
                            ),
                    )
                    .footer(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("btn-cancel-delete-download")
                                    .label(cx.lang().get("common.cancel"))
                                    .rounded(px(24.0))
                                    .on_click(move |_event, window, cx| {
                                        Self::close(window, cx);
                                    }),
                            )
                            .child(
                                Button::new("btn-confirm-delete-download")
                                    .label(cx.lang().get("common.delete"))
                                    .danger()
                                    .rounded(px(24.0))
                                    .on_click(move |_event, window, cx| {
                                        if let Some(id) = task_id {
                                            let model = cx.tasks_model().clone();
                                            model.update(cx, |m, cx| {
                                                m.delete_task(id, delete_file, cx);
                                            });
                                        }
                                        Self::close(window, cx);
                                    }),
                            ),
                    ),
            )
    }
}
