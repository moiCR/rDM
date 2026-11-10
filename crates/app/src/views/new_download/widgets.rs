use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::{
    base::{Disableable, Scrollbar, ScrollbarMode},
    component::{button::ButtonVariants, input::Textarea},
};

use super::{DownloadForm, validate_connections, validate_file_name, validate_url};
use crate::{
    components::{Button, IconButton, Modal, input::themed_input},
    theme::ActiveTheme,
};

impl Render for DownloadForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let foreground = theme.text_color();
        let muted = theme.text_muted();
        let field_background = theme
            .input_background()
            .unwrap_or_else(|| theme.surface_hover());
        let border = theme.border();
        let error_color = theme.error();
        let url = self.url.read(cx).value().to_string();
        let name = self.file_name.read(cx).value().to_string();
        let connections = self.connections.read(cx).value().to_string();
        let url_error = validate_url(&url).err();
        let name_error = validate_file_name(&name).err();
        let connections_error = validate_connections(&connections).err();
        let invalid = url_error.is_some()
            || name_error.is_some()
            || connections_error.is_some()
            || self.directory.as_os_str().is_empty();
        let locked = self.submitting || self.created_task.is_some();
        let viewport = window.viewport_size();
        let form = cx.entity().downgrade();

        div()
            .size_full()
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.close(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                Modal::new("new-download-modal")
                    .open(true)
                    .compact()
                    .fit_content()
                    .width((viewport.width - px(32.0))
.clamp(px(0.0), px(620.0)))
                    .max_height((viewport.height - px(32.0))
.max(px(0.0)))
                    .header(div()
.hidden())
                    .on_close(move |window, cx| {
                        let _ = form.update(cx, |this, cx| this.close(window, cx));
                    })
                    .child(
                        div()
                            .relative()
                            .min_h_0()
                            .child(
                                div()
                                    .id("new-download-scroll")
                                    .max_h((viewport.height - px(112.0))
.max(px(0.0)))
                                    .overflow_y_scroll()
                                    .track_scroll(&self.scroll)
                                    .pr_2()
                                    .child(
                                        div()
.flex()
.flex_col()
.gap_3()
                                            .child(
                                                Textarea::new(&self.url)
                                                    .aria_label("URL de descarga")
                                                    .h(px(125.0))
                                                    .w_full()
                                                    .rounded(px(24.0))
                                                    .bg(field_background)
                                                    .border_color(border)
                                                    .text_color(foreground)
                                                    .disabled(locked),
                                            )
                                            .children(url_error.filter(|_| !url.trim()
.is_empty())
.map(|error| {
                                                div()
.text_xs()
.text_color(error_color)
.child(error)
                                            }))
                                            .child(
                                                div()
.flex()
.items_center()
.gap_2()
                                                    .rounded(px(24.0))
.bg(field_background)
.px_3()
.py_1()
                                                    .child(div()
.flex_none()
.text_xs()
.text_color(muted)
.child("Guardar en"))
                                                    .child(div()
.flex_1()
.min_w_0()
.truncate()
.text_sm()
.text_color(foreground)
                                                        .child(self.directory.to_string_lossy()
.to_string()))
                                                    .child(IconButton::new("new-download-directory", "icons/folder-open.svg")
                                                        .rounded(px(24.0))
.disabled(locked)
.tooltip("Elegir carpeta")
                                                        .on_click(cx.listener(|this, _, window, cx| this.choose_directory(window, cx)))),
                                            )
                                            .child(
                                                div()
.flex()
.child(Button::new("new-download-advanced")
                                                    .ghost()
.rounded(px(24.0))
.accessibility_label("Avanzado")
                                                    .toggled(self.advanced)
                                                    .child(div()
.flex()
.items_center()
.gap_2()
                                                        .child(svg()
.path(if self.advanced { "icons/chevron-down.svg" } else { "icons/chevron-right.svg" })
.size_3())
                                                        .child("Avanzado"))
                                                    .on_click(cx.listener(|this, _, _, cx| { this.advanced = !this.advanced; cx.notify(); }))),
                                            )
                                            .when(self.advanced, |content| content.child(
                                                div()
.flex()
.flex_col()
.gap_2()
                                                    .child(div()
.text_sm()
.text_color(muted)
.child("Nombre de archivo"))
                                                    .child(themed_input(&self.file_name, cx)
                                                        .id("new-download-name")
.aria_label("Nombre de archivo opcional")
                                                        .rounded(px(24.0))
.h(px(36.0))
.w_full()
.disabled(locked))
                                                    .children(name_error.map(|error| div()
.text_xs()
.text_color(error_color)
.child(error)))
                                                    .child(div()
.flex()
.items_center()
.justify_between()
.gap_2()
                                                        .child(div()
.text_sm()
.text_color(muted)
.child("Conexiones por descarga"))
                                                        .child(div()
.w(px(72.0))
.child(themed_input(&self.connections, cx)
                                                            .id("new-download-connections")
.aria_label("Conexiones por descarga, entre 1 y 32")
                                                            .rounded(px(24.0))
.h(px(36.0))
.text_align(TextAlign::Center)
.disabled(locked))))
                                                    .children(connections_error.map(|error| div()
.text_xs()
.text_color(error_color)
.child(error)))
                                            ))
                                            .children((!self.advanced)
.then_some(name_error.or(connections_error))
.flatten()
                                                .map(|error| div()
.text_xs()
.text_color(error_color)
.child(error)))
                                            .children(self.error.clone()
.map(|error| div()
.text_sm()
.text_color(error_color)
.child(error))),
                                    ),
                            )
                            .child(Scrollbar::vertical(&self.scroll)
.mode(ScrollbarMode::Always)),
                    )
                    .footer(
                        div()
.flex()
.flex_none()
.items_center()
.justify_end()
.gap_2()
                            .child(Button::new("new-download-cancel")
.label("Cancelar")
.rounded(px(24.0))
                                .disabled(self.submitting)
                                .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))))
                            .child(Button::new("new-download-submit")
                                .label(if self.submitting { "Añadiendo…" } else if self.created_task.is_some() { "Reintentar" } else { "Descargar" })
                                .primary()
.rounded(px(24.0))
.disabled(invalid || self.submitting)
                                .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)))),
                    ),
            )
    }
}
