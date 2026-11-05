use gpui::{prelude::FluentBuilder, *};

use crate::theme::ActiveTheme;

#[derive(IntoElement)]
pub struct SidebarItem {
    label: &'static str,
    icon_path: &'static str,
    path: Option<&'static str>,
    active: bool,
}

pub fn sidebar_item(
    label: &'static str,
    icon_path: &'static str,
    path: Option<&'static str>,
    active: bool,
) -> SidebarItem {
    SidebarItem {
        label,
        icon_path,
        path,
        active,
    }
}

impl RenderOnce for SidebarItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let surface = cx.theme().surface();
        let surface_hover = cx.theme().surface_hover();
        let text_color = cx.theme().text_color();
        let text_muted = cx.theme().text_muted();

        let hover_state = window.use_keyed_state(self.label, cx, |_, _| false);
        let hovered = *hover_state.read(cx);
        let highlighted = self.active || hovered;
        let active = self.active;
        let label = self.label;
        let icon_path = self.icon_path;
        let path = self.path;

        let spring_config = SpringConfig::new(180.0, 26.0, 1.0);
        let color_spring = SpringAnimation::new(spring_config).to(highlighted);
        let scale_spring = SpringAnimation::new(spring_config).to(active);
        let icon_color_spring = SpringAnimation::new(spring_config).to(highlighted);

        div()
            .id(ElementId::Name(label.into()))
            .flex()
            .items_center()
            .gap_2p5()
            .rounded(px(24.0))
            .cursor_pointer()
            .when(active, |this| this.font_weight(FontWeight::MEDIUM))
            .on_hover(move |is_hovered, _window, cx| {
                hover_state.update(cx, |state, cx| {
                    if *state != *is_hovered {
                        *state = *is_hovered;
                        cx.notify();
                    }
                });
            })
            .when_some(path, |this, target_path| {
                this.on_click(move |_event, window, cx| {
                    gpui_router::use_navigate(window, cx).push(target_path);
                })
            })
            .child(
                svg()
                    .path(icon_path)
                    .when(active, |this| {
                        this.size(px(17.0))
                            .with_transformation(Transformation::scale(size(1.05, 1.05)))
                    })
                    .when(!active, |this| this.size_4())
                    .with_spring(
                        ElementId::Name(format!("{label}-icon-color").into()),
                        icon_color_spring,
                        move |this, phase| {
                            this.text_color(phase.interpolate_clamped(text_muted, text_color))
                        },
                    ),
            )
            .child(label)
            .with_spring(
                ElementId::Name(format!("{label}-color").into()),
                color_spring,
                move |this, phase| {
                    this.bg(phase.interpolate_clamped(surface, surface_hover))
                        .text_color(phase.interpolate_clamped(text_muted, text_color))
                },
            )
            .with_spring(
                ElementId::Name(format!("{label}-scale").into()),
                scale_spring,
                move |this, phase| {
                    let pad_x = phase.interpolate_clamped(px(16.0), px(20.0));
                    let pad_y = phase.interpolate_clamped(px(9.0), px(11.5));
                    let font_px = phase.interpolate_clamped(px(14.0), px(15.0));

                    this.map_element(|div| div.px(pad_x).py(pad_y).text_size(font_px))
                },
            )
    }
}
