use gpui::prelude::FluentBuilder;
use gpui::*;
use std::{cell::Cell, rc::Rc};

use super::button::{ButtonRounded, IconButton};
use crate::theme::ActiveTheme;

pub type ModalCloseHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

pub fn track_card_bounds(slot: Rc<Cell<Option<Bounds<Pixels>>>>) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            slot.set(Some(bounds));
        },
        |_, (), _, _| {},
    )
    .absolute()
    .inset_0()
}

#[derive(IntoElement)]
pub struct Modal {
    id: ElementId,
    open: bool,
    compact: bool,
    fit_content: bool,
    width: Option<Pixels>,
    max_height: Option<Pixels>,
    from_bounds: Option<Bounds<Pixels>>,
    title: Option<SharedString>,
    icon_path: Option<SharedString>,
    on_close: Option<ModalCloseHandler>,
    header: Option<AnyElement>,
    footer: Option<AnyElement>,
    subview: Option<AnyElement>,
    children: Vec<AnyElement>,
}

impl Modal {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            open: false,
            compact: false,
            fit_content: false,
            width: None,
            max_height: None,
            from_bounds: None,
            title: None,
            icon_path: None,
            on_close: None,
            header: None,
            footer: None,
            subview: None,
            children: Vec::new(),
        }
    }

    pub fn from_bounds(mut self, origin: Option<Bounds<Pixels>>) -> Self {
        self.from_bounds = origin;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Some(width.into());
        self
    }

    pub fn fit_content(mut self) -> Self {
        self.fit_content = true;
        self
    }

    pub fn max_height(mut self, max_height: impl Into<Pixels>) -> Self {
        self.max_height = Some(max_height.into());
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn icon(mut self, icon_path: impl Into<SharedString>) -> Self {
        self.icon_path = Some(icon_path.into());
        self
    }

    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    pub fn subview(mut self, subview: impl IntoElement) -> Self {
        self.subview = Some(subview.into_any_element());
        self
    }
}

impl ParentElement for Modal {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Modal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette_surface = cx.theme().surface();
        let palette_surface_hover = cx.theme().surface_hover();
        let palette_border = cx.theme().border();
        let palette_text = cx.theme().text_color();
        let palette_text_muted = cx.theme().text_muted();
        let palette_accent = cx.theme().accent();

        let viewport = window.viewport_size();
        let card_w = self.width.unwrap_or_else(|| px(880.0));
        let card_max_h = self.max_height.unwrap_or_else(|| px(620.0));
        let from_bounds = self.from_bounds;

        let on_close_backdrop = self.on_close.clone();
        let on_close_btn = self.on_close.clone();

        let header_element = if let Some(custom_header) = self.header {
            Some(custom_header)
        } else if self.title.is_some() || self.icon_path.is_some() || self.on_close.is_some() {
            let title_str = self.title.clone();
            let icon_str = self.icon_path.clone();

            Some(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .w_full()
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap_3()
                            .children(icon_str.map(|path| {
                                svg()
                                    .path(path)
                                    .size_6()
                                    .flex_shrink_0()
                                    .text_color(palette_accent)
                            }))
                            .children(title_str.map(|title| {
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(palette_text)
                                    .child(title)
                            })),
                    )
                    .children(on_close_btn.map(|handler| {
                        div().flex_shrink_0().child(
                            IconButton::new("btn-close-modal", "icons/x.svg")
                                .size(px(32.0))
                                .icon_size(px(16.0))
                                .rounded(ButtonRounded::Size(px(99.0)))
                                .color(palette_text_muted)
                                .hover_bg(palette_surface_hover)
                                .on_click(move |_event, window, cx| {
                                    handler(window, cx);
                                }),
                        )
                    }))
                    .into_any_element(),
            )
        } else {
            None
        };

        let footer_element = self.footer;
        let subview_element = self.subview;

        let slide_spring = if from_bounds.is_some() {
            SpringAnimation::new(SpringConfig::new(320.0, 36.0, 1.0))
                .to(self.open)
                .from(false)
        } else {
            SpringAnimation::new(SpringConfig::new(260.0, 26.0, 1.0))
                .to(self.open)
                .from(false)
        };

        let card_id: ElementId = "modal-card".into();
        let card_spring_id = ElementId::Name(format!("{}-spring", self.id).into());

        div()
            .id(self.id)
            .absolute()
            .inset_0()
            .size_full()
            .with_spring(card_spring_id, slide_spring, move |this, phase| {
                let alpha = phase.interpolate_clamped(0.0, 1.0);
                if alpha <= 0.005 {
                    return this.invisible();
                }

                let backdrop_alpha = phase.interpolate_clamped(0.0, 0.65);

                let card = if let Some(origin) = from_bounds {
                    let target_w = card_w.min((viewport.width - px(48.0)).max(px(320.0)));
                    let target_h = card_max_h.min((viewport.height - px(64.0)).max(px(140.0)));
                    let target_x = ((viewport.width - target_w) / 2.0).max(px(16.0));
                    let target_y = ((viewport.height - target_h) / 2.0).max(px(16.0));
                    let left = phase.interpolate_clamped(origin.origin.x, target_x);
                    let top = phase.interpolate_clamped(origin.origin.y, target_y);
                    let width = phase.interpolate_clamped(origin.size.width, target_w);
                    let height = phase.interpolate_clamped(origin.size.height, target_h);
                    let content_alpha = ((alpha - 0.12) / 0.88).clamp(0.0, 1.0);

                    div()
                        .id(card_id.clone())
                        .absolute()
                        .left(left)
                        .top(top)
                        .w(width)
                        .h(height)
                        .overflow_hidden()
                        .occlude()
                        .rounded(px(24.0))
                        .bg(linear_gradient(
                            180.0,
                            linear_color_stop(palette_surface_hover, 0.0),
                            linear_color_stop(palette_surface, 1.0),
                        ))
                        .when(!self.compact, |el| {
                            el.border_1().border_color(palette_border)
                        })
                        .shadow_xl()
                        .flex()
                        .flex_col()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .on_click(|_, _, cx| {
                            cx.stop_propagation();
                        })
                        .child(
                            div()
                                .size_full()
                                .flex()
                                .flex_col()
                                .min_h_0()
                                .opacity(content_alpha)
                                .when(self.compact, |inner| inner.p_4().gap_2())
                                .when(!self.compact, |inner| inner.p_8().gap_6())
                                .children(header_element)
                                .children(self.children)
                                .children(footer_element)
                                .children(subview_element),
                        )
                } else {
                    let slide_y = phase.interpolate_clamped(px(60.0), px(0.0));
                    div()
                        .id(card_id.clone())
                        .relative()
                        .overflow_hidden()
                        .occlude()
                        .top(slide_y)
                        .opacity(alpha)
                        .w(card_w)
                        .max_h(card_max_h)
                        .when(self.compact, |card| {
                            card.p_4()
                                .gap_2()
                                .when(!self.fit_content, |card| card.h(card_max_h))
                        })
                        .when(!self.compact, |card| {
                            card.p_8().gap_6().border_1().border_color(palette_border)
                        })
                        .rounded(px(24.0))
                        .bg(palette_surface)
                        .shadow_xl()
                        .flex()
                        .flex_col()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .on_click(|_, _, cx| {
                            cx.stop_propagation();
                        })
                        .children(header_element)
                        .children(self.children)
                        .children(footer_element)
                        .children(subview_element)
                };

                this.visible()
                    .child(
                        div()
                            .id("modal-backdrop")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .bg(rgb(0x000000))
                            .opacity(backdrop_alpha),
                    )
                    .child(
                        div()
                            .id("modal-positioner")
                            .occlude()
                            .absolute()
                            .inset_0()
                            .size_full()
                            .when(from_bounds.is_none(), |pos| {
                                pos.flex().items_center().justify_center()
                            })
                            .when_some(on_close_backdrop.clone(), |el, handler| {
                                el.on_click(move |_event, window, cx| {
                                    handler(window, cx);
                                })
                            })
                            .child(card),
                    )
            })
            .into_any_element()
    }
}
