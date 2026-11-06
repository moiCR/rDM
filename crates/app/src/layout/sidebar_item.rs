use gpui::{prelude::FluentBuilder, *};

use crate::theme::ActiveTheme;

pub type SidebarItemAction = Box<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarVariant {
    Classic,
    Minimal,
}

#[derive(IntoElement)]
pub struct SidebarItem {
    label: &'static str,
    icon_path: &'static str,
    path: Option<&'static str>,
    active: bool,
    action: Option<SidebarItemAction>,
    variant: SidebarVariant,
}

pub fn sidebar_item(
    label: &'static str,
    icon_path: &'static str,
    path: Option<&'static str>,
    active: bool,
    action: Option<SidebarItemAction>,
) -> SidebarItem {
    SidebarItem {
        label,
        icon_path,
        path,
        active,
        action,
        variant: SidebarVariant::Classic,
    }
}

pub fn sidebar_item_with_variant(
    label: &'static str,
    icon_path: &'static str,
    path: Option<&'static str>,
    active: bool,
    action: Option<SidebarItemAction>,
    variant: SidebarVariant,
) -> SidebarItem {
    SidebarItem {
        label,
        icon_path,
        path,
        active,
        action,
        variant,
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
        let action = self.action;
        let variant = self.variant;

        if variant == SidebarVariant::Minimal {
            return div()
                .id(ElementId::Name(format!("minimal-{label}").into()))
                .flex()
                .items_center()
                .w_full()
                .py_2()
                .px_1()
                .cursor_pointer()
                .when_some(path, |this, target_path| {
                    this.on_click(move |_event, window, cx| {
                        gpui_router::use_navigate(window, cx).push(target_path);
                    })
                })
                .when_some(action, |this, on_click_action| {
                    this.on_click(move |_event, window, cx| {
                        (on_click_action)(window, cx);
                    })
                })
                .child(
                    div()
                        .text_size(px(18.0))
                        .line_height(relative(1.2))
                        .font_weight(if active {
                            FontWeight::BOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(if active { text_color } else { text_muted })
                        .hover(move |s| s.text_color(text_color))
                        .child(label),
                )
                .into_any_element();
        }

        let spring_config = SpringConfig::new(180.0, 26.0, 1.0);
        let color_spring = SpringAnimation::new(spring_config).to(highlighted);
        let scale_spring = SpringAnimation::new(spring_config).to(active);

        div()
            .id(ElementId::Name(label.into()))
            .flex()
            .items_center()
            .gap_1p5()
            .rounded(px(24.0))
            .line_height(relative(1.0))
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
            .when_some(action, |this, on_click_action| {
                this.on_click(move |_event, window, cx| {
                    (on_click_action)(window, cx);
                })
            })
            .with_spring(
                ElementId::Name(format!("{label}-color").into()),
                color_spring,
                move |this, phase| {
                    let fg = phase.interpolate_clamped(text_muted, text_color);
                    let bg = phase.interpolate_clamped(surface, surface_hover);

                    this.bg(bg)
                        .text_color(fg)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .flex_shrink_0()
                                .child(
                                    svg()
                                        .path(icon_path)
                                        .text_color(fg)
                                        .when(active, |icon| {
                                            icon.size(px(17.0)).with_transformation(
                                                Transformation::scale(size(1.05, 1.05)),
                                            )
                                        })
                                        .when(!active, |icon| icon.size_4()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .line_height(relative(1.0))
                                .pb(px(1.5))
                                .child(label),
                        )
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
            .into_any_element()
    }
}
