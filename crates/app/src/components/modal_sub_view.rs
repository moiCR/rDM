use gpui::prelude::FluentBuilder;
use gpui::*;
use std::rc::Rc;

use crate::theme::ActiveTheme;

pub type ModalSubViewCloseHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct ModalSubView {
    id: ElementId,
    open: bool,
    width: Option<Pixels>,
    max_height: Option<Pixels>,
    title: Option<SharedString>,
    icon_path: Option<SharedString>,
    on_back: Option<ModalSubViewCloseHandler>,
    header: Option<AnyElement>,
    footer: Option<AnyElement>,
    children: Vec<AnyElement>,
}

impl ModalSubView {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            open: false,
            width: None,
            max_height: None,
            title: None,
            icon_path: None,
            on_back: None,
            header: None,
            footer: None,
            children: Vec::new(),
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Some(width.into());
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

    pub fn on_back(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(Rc::new(handler));
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
}

impl ParentElement for ModalSubView {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ModalSubView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette_surface = cx.theme().surface();
        let palette_surface_hover = cx.theme().surface_hover();
        let palette_border = cx.theme().border();
        let palette_text = cx.theme().text_color();
        let palette_text_muted = cx.theme().text_muted();

        let card_w = self.width.unwrap_or_else(|| px(440.0));
        let card_max_h = self.max_height.unwrap_or_else(|| px(460.0));

        let on_back_backdrop = self.on_back.clone();
        let on_back_btn = self.on_back.clone();

        let header_element =
            if let Some(custom_header) = self.header {
                Some(custom_header)
            } else if self.title.is_some() || self.icon_path.is_some() || self.on_back.is_some() {
                let title_str = self.title.clone();
                let icon_str = self.icon_path.clone();

                Some(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2p5()
                                .children(icon_str.map(|path| {
                                    svg().path(path).size_5().text_color(rgb(0xffffff))
                                }))
                                .children(title_str.map(|title| {
                                    div()
                                        .text_xl()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(palette_text)
                                        .child(title)
                                })),
                        )
                        .children(on_back_btn.map(|handler| {
                            div()
                                .id("btn-subview-close")
                                .flex()
                                .items_center()
                                .justify_center()
                                .size_7()
                                .rounded_full()
                                .cursor_pointer()
                                .hover(move |s| s.bg(palette_surface_hover))
                                .on_click(move |_event, window, cx| {
                                    handler(window, cx);
                                })
                                .child(
                                    svg()
                                        .path("icons/x.svg")
                                        .size_3p5()
                                        .text_color(palette_text_muted),
                                )
                        }))
                        .into_any_element(),
                )
            } else {
                None
            };

        let footer_element = self.footer;

        let slide_spring = SpringAnimation::new(SpringConfig::new(260.0, 26.0, 1.0)).to(self.open);

        let card_id: ElementId = "modal-subview-card".into();
        let card_spring_id = ElementId::Name(format!("{}-spring", self.id).into());

        div()
            .id(self.id)
            .absolute()
            .inset_0()
            .size_full()
            .rounded(px(24.0))
            .with_spring(card_spring_id, slide_spring, move |this, phase| {
                let alpha = phase.interpolate_clamped(0.0, 1.0);
                if alpha <= 0.001 {
                    return this.invisible();
                }

                let backdrop_alpha = phase.interpolate_clamped(0.0, 0.70);
                let slide_y = phase.interpolate_clamped(px(30.0), px(0.0));

                this.visible()
                    .child(
                        div()
                            .id("modal-subview-backdrop")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .rounded(px(24.0))
                            .bg(rgb(0x000000))
                            .opacity(backdrop_alpha)
                            .when_some(on_back_backdrop.clone(), |el, handler| {
                                el.on_click(move |_event, window, cx| {
                                    handler(window, cx);
                                })
                            }),
                    )
                    .child(
                        div()
                            .id("modal-subview-positioner")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .id(card_id.clone())
                                    .relative()
                                    .top(slide_y)
                                    .opacity(alpha)
                                    .w(card_w)
                                    .max_h(card_max_h)
                                    .p_6()
                                    .gap_4()
                                    .rounded(px(20.0))
                                    .bg(palette_surface)
                                    .border_1()
                                    .border_color(palette_border)
                                    .shadow_2xl()
                                    .flex()
                                    .flex_col()
                                    .children(header_element)
                                    .children(self.children)
                                    .children(footer_element),
                            ),
                    )
            })
            .into_any_element()
    }
}
