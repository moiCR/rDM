use gpui::*;
use gpui_kit::{
    base::{Disableable, Scrollbar, ScrollbarMode},
    component::{button::ButtonVariants, input::Textarea},
};

use super::{
    DownloadForm, parse_network_options, validate_connections, validate_file_name, validate_url,
};
use crate::{
    components::{Button, IconButton, Modal, input::themed_input},
    lang::ActiveLang,
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
        let network_error = parse_network_options(
            &self.speed_limit.read(cx).value(),
            &self.checksum.read(cx).value(),
            &self.headers.read(cx).value(),
            &self.cookies.read(cx).value(),
        )
        .err();
        let invalid = network_error.is_some()
            || url_error.is_some()
            || name_error.is_some()
            || connections_error.is_some()
            || self.directory.as_os_str().is_empty();
        let locked = self.submitting || self.created_task.is_some() || self.closing;
        let viewport = window.viewport_size();
        let form = cx.entity().downgrade();
        let advanced_animation =
            SpringAnimation::new(SpringConfig::new(300.0, 34.0, 1.0)).to(self.advanced);
        let advanced_height = px(224.0
            + if self.network_expanded { 200.0 } else { 0.0 }
            + if name_error.is_some() { 24.0 } else { 0.0 }
            + if connections_error.is_some() {
                24.0
            } else {
                0.0
            });

        let section_height = if self.advanced {
            f32::from(advanced_height)
        } else {
            0.0
        };

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
                    .open(!self.closing)
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
                                                    .aria_label(cx.lang().get("new_download.url_label"))
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
.child(cx.lang().get(error))
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
.child(cx.lang().get("new_download.save_to")))
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
.tooltip(cx.lang().get("new_download.choose_directory"))
                                                        .on_click(cx.listener(|this, _, window, cx| this.choose_directory(window, cx)))),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .gap_2()
                                                    .child(
                                                        Button::new("new-download-advanced")
                                                            .ghost()
                                                            .rounded(px(24.0))
                                                            .accessibility_label(
                                                                cx.lang()
                                                                    .get("new_download.advanced"),
                                                            )
                                                            .toggled(self.advanced)
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .items_center()
                                                                    .gap_2()
                                                                    .child(
                                                                        svg()
                                                                            .path(
                                                                                "icons/chevron-right.svg",
                                                                            )
                                                                            .size_3()
                                                                            .text_color(muted)
.with_spring("new-download-chevron-rotation", advanced_animation.clone(), |icon, phase| {
    icon.with_transformation(Transformation::rotate(radians(phase.interpolate_clamped(0.0, std::f32::consts::FRAC_PI_2))))
}))
                                                        .child(cx.lang().get("new_download.advanced")))
                                                    .on_click(cx.listener(|this, _, _, cx| { this.advanced = !this.advanced; cx.notify(); })))
                                                    .child(
                                                        Button::new("new-download-probe-media")
                                                            .ghost()
                                                            .rounded(px(24.0))
                                                            .disabled(locked || url_error.is_some() || self.probing_media)
                                                            .label(cx.lang().get(if self.probing_media {
                                                                "new_download.media_probing"
                                                            } else {
                                                                "new_download.media_probe"
                                                            }))
                                                            .on_click(cx.listener(|this, _, window, cx| {
                                                                this.probe_media(window, cx);
                                                            })),
                                                    ),
                                            )
                                            .children(self.media_probe.as_ref().map(|probe| {
                                                let selected = self.selected_format;
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_2()
                                                    .p_3()
                                                    .rounded(px(16.0))
                                                    .bg(field_background)
                                                    .border_1()
                                                    .border_color(border)
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(foreground)
                                                            .truncate()
                                                            .child(probe.title.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_wrap()
                                                            .gap_1p5()
                                                            .children(probe.formats.iter().enumerate().map(|(index, fmt)| {
                                                                let is_selected = selected == Some(index);
                                                                Button::new(("media-format", index))
                                                                    .ghost()
                                                                    .toggled(is_selected)
                                                                    .rounded(px(24.0))
                                                                    .label(fmt.label.clone())
                                                                    .on_click(cx.listener(move |this, _, window, cx| {
                                                                        this.select_media_format(index, window, cx);
                                                                    }))
                                                            })),
                                                    )
                                            }))
                                            .child(
                                                div()
.overflow_hidden()
.with_spring("new-download-advanced-height", SpringAnimation::new(SpringConfig::new(300.0, 34.0, 1.0)).to(section_height).from(0.0), |section, phase| {
    section.h(px(phase.max(0.0)))
})
.child(div()
.flex()
.flex_col()
.flex_shrink_0()
.gap_2()
.line_height(px(16.0))
                                                    .child(div()
.text_sm()
.text_color(muted)
.child(cx.lang().get("new_download.file_name")))
                                                    .child(themed_input(&self.file_name, cx)
                                                        .id("new-download-name")
.aria_label(cx.lang().get("new_download.optional_name"))
                                                        .rounded(px(24.0))
.h(px(32.0))
.w_full()
.disabled(locked))
                                                    .children(name_error.map(|error| div()
.text_xs()
.text_color(error_color)
.child(cx.lang().get(error))))
                                                    .child(div()
.flex()
.items_center()
.justify_between()
.gap_2()
                                                        .child(div()
.text_sm()
.text_color(muted)
.child(cx.lang().get("new_download.connections")))
                                                        .child(div()
.w(px(72.0))
.child(themed_input(&self.connections, cx)
                                                            .id("new-download-connections")
.aria_label(cx.lang().get("new_download.connections_label"))
                                                            .rounded(px(24.0))
.h(px(32.0))
.text_align(TextAlign::Center)
.disabled(locked))))
 .child(div().flex().items_center().gap_2().h(px(32.0))
 .child(div().w(px(150.0)).flex_shrink_0().text_sm().text_color(muted).child(cx.lang().get("new_download.speed_limit")))
 .child(themed_input(&self.speed_limit, cx).rounded(px(24.0)).id("new-download-speed_limit").aria_label(cx.lang().get("new_download.speed_limit")).h(px(32.0)).w_full().disabled(locked)))
 .child(div().flex().items_center().gap_2().h(px(32.0))
 .child(div().w(px(150.0)).flex_shrink_0().text_sm().text_color(muted).child(cx.lang().get("new_download.checksum")))
 .child(themed_input(&self.checksum, cx).rounded(px(24.0)).id("new-download-checksum").aria_label(cx.lang().get("new_download.checksum")).h(px(32.0)).w_full().disabled(locked)))
                                                    .children(connections_error.map(|error| div()
.text_xs()
.text_color(error_color)
.child(cx.lang().get(error))))
 .child(Button::new("new-download-network").ghost().h(px(32.0)).disabled(locked)
 .child(div().flex().items_center().gap_2()
 .child(svg().path("icons/chevron-right.svg").size_3().text_color(muted)
 .with_spring("new-download-network-chevron", SpringAnimation::new(SpringConfig::new(300.0,34.0,1.0)).to(self.network_expanded), |icon, phase| icon.with_transformation(Transformation::rotate(radians(phase.interpolate_clamped(0.0,std::f32::consts::FRAC_PI_2))))))
 .child(cx.lang().get("new_download.network")))
 .on_click(cx.listener(|this, _, _, cx| { this.network_expanded = !this.network_expanded; cx.notify(); })))
 .child(div().overflow_hidden().h(px(200.0)).flex_shrink_0()
 .child(div().flex().flex_col().gap_2().line_height(px(16.0))
 .child(div().text_sm().text_color(muted).child(cx.lang().get("new_download.headers")))
 .child(Textarea::new(&self.headers).rounded(px(24.0)).aria_label(cx.lang().get("new_download.headers")).h(px(112.0)).w_full().bg(field_background).border_color(border).text_color(foreground).disabled(locked))
 .child(div().text_sm().text_color(muted).child(cx.lang().get("new_download.cookies")))
 .child(themed_input(&self.cookies, cx).rounded(px(24.0)).id("new-download-cookies").aria_label(cx.lang().get("new_download.cookies")).h(px(32.0)).w_full().disabled(locked))))

                                            ))
                                            .children((!self.advanced)
.then_some(name_error.or(connections_error))
.flatten()
                                                .map(|error| div()
.text_xs()
.text_color(error_color)
.child(cx.lang().get(error))))
                                            .children(network_error.map(|error| div().text_xs().text_color(error_color).child(cx.lang().get(error))))
                                            .children(self.error.as_deref()
.map(|error| div()
.text_sm()
.text_color(error_color)
.child(cx.lang().get(error)))),
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
.label(cx.lang().get("common.cancel"))
.rounded(px(24.0))
                                .disabled(self.submitting)
                                .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))))
                            .child(Button::new("new-download-submit")
                                .label(if self.submitting { cx.lang().get("new_download.adding") } else if self.created_task.is_some() { cx.lang().get("common.retry") } else { cx.lang().get("new_download.download") })
                                .primary()
.rounded(px(24.0))
.disabled(invalid || self.submitting || self.closing)
                                .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)))),
                    ),
            )
    }
}
