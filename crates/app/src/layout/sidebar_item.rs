use gpui::{prelude::FluentBuilder, *};

use crate::{ext::transitions::*, theme::ActiveTheme};

pub type SidebarItemAction = Box<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarVariant {
    Classic,
    Modern,
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
        let accent = cx.theme().accent();

        let hover_state = window.use_keyed_state(self.label, cx, |_, _| false);
        let hovered = *hover_state.read(cx);
        let active = self.active;
        let label = self.label;
        let icon_path = self.icon_path;
        let path = self.path;
        let action = self.action;
        let variant = self.variant;

        if variant == SidebarVariant::Classic {
            let hover_state_classic = hover_state.clone();
            return div()
                .id(ElementId::Name(format!("classic-{label}").into()))
                .flex()
                .items_center()
                .w_full()
                .py_2()
                .px_1()
                .cursor_pointer()
                .on_hover(move |is_hovered, window, cx| {
                    hover_state_classic.update(cx, |state, cx| {
                        if *state != *is_hovered {
                            *state = *is_hovered;
                            cx.notify();
                        }
                    });
                    window.refresh();
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
                .child(
                    div()
                        .id(ElementId::Name(format!("classic-text-{label}").into()))
                        .with_transitions(format!("classic-text-trans-{label}"), |t| {
                            t.all(millis(300))
                        })
                        .text_size(px(18.0))
                        .line_height(relative(1.2))
                        .font_weight(if active {
                            FontWeight::BOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(if active || hovered {
                            text_color
                        } else {
                            text_muted
                        })
                        .hover_style(|s| s.text_color(text_color))
                        .child(label),
                )
                .into_any_element();
        }

        let highlighted = active || hovered;
        let spring_config = SpringConfig::new(180.0, 26.0, 1.0);
        let color_spring = SpringAnimation::new(spring_config).to(highlighted);

        div()
            .id(ElementId::Name(format!("modern-{label}").into()))
            .flex()
            .items_center()
            .gap_3()
            .w_full()
            .px_3()
            .py_2()
            .rounded(px(14.0))
            .cursor_pointer()
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
                ElementId::Name(format!("{label}-modern-bg").into()),
                color_spring,
                move |this, phase| {
                    let bg = if active {
                        surface_hover
                    } else {
                        phase.interpolate_clamped(surface.opacity(0.0), surface_hover.opacity(0.6))
                    };
                    let icon_fg = if active { accent } else { text_muted };
                    let fg = if active { text_color } else { text_muted };

                    this.bg(bg)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size_5()
                                .child(svg().path(icon_path).size_4().text_color(icon_fg)),
                        )
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(if active {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .text_color(fg)
                                .child(label),
                        )
                },
            )
            .into_any_element()
    }
}
