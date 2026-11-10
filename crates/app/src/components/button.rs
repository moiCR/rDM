use std::rc::Rc;

use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::base::{Disableable, Selectable};
pub use gpui_kit::component::button::{
    Button, ButtonCustomVariant, ButtonRounded, ButtonVariant, ButtonVariants,
};

use super::tooltip::ActionTooltip;
use crate::theme::ActiveTheme;

pub type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon_path: SharedString,
    width: Option<Pixels>,
    height: Option<Pixels>,
    size: Pixels,
    icon_size: Pixels,
    rounded: ButtonRounded,
    tooltip: Option<SharedString>,
    shortcut: Option<SharedString>,
    color: Option<Rgba>,
    background: Option<Rgba>,
    hover_bg: Option<Rgba>,
    on_click: Option<ClickHandler>,
    selected: bool,
    disabled: bool,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon_path: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon_path: icon_path.into(),
            width: None,
            height: None,
            size: px(32.0),
            icon_size: px(16.0),
            rounded: ButtonRounded::Size(px(8.0)),
            tooltip: None,
            shortcut: None,
            color: None,
            background: None,
            hover_bg: None,
            on_click: None,
            selected: false,
            disabled: false,
        }
    }

    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Some(width.into());
        self
    }

    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.height = Some(height.into());
        self
    }

    pub fn icon_size(mut self, icon_size: impl Into<Pixels>) -> Self {
        self.icon_size = icon_size.into();
        self
    }

    pub fn rounded(mut self, rounded: impl Into<ButtonRounded>) -> Self {
        self.rounded = rounded.into();
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn color(mut self, color: impl Into<Rgba>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn background(mut self, background: Rgba) -> Self {
        self.background = Some(background);
        self
    }

    pub fn hover_bg(mut self, hover_bg: impl Into<Rgba>) -> Self {
        self.hover_bg = Some(hover_bg.into());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Selectable for IconButton {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Disableable for IconButton {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_muted = cx.theme().text_muted();
        let text_color = cx.theme().text_color();
        let surface_hover = self.hover_bg.unwrap_or_else(|| cx.theme().surface_hover());
        let icon_color = self.color.unwrap_or(text_muted);
        let group_name: SharedString = format!("icon-btn-{}", self.id).into();

        let rounding = match self.rounded {
            ButtonRounded::None => px(0.0),
            ButtonRounded::Small => px(4.0),
            ButtonRounded::Medium => px(8.0),
            ButtonRounded::Large => px(12.0),
            ButtonRounded::Size(px) => px,
        };

        let tooltip_label = self.tooltip;
        let tooltip_shortcut = self.shortcut;
        let is_disabled = self.disabled;
        let is_selected = self.selected;

        let w = self.width.unwrap_or(self.size);
        let h = self.height.unwrap_or(self.size);

        let mut el = div()
            .id(self.id)
            .group(group_name.clone())
            .flex()
            .items_center()
            .justify_center()
            .w(w)
            .h(h)
            .rounded(rounding)
            .when_some(self.background, |this, background| this.bg(background))
            .when(is_selected, |this| {
                this.bg(surface_hover).border_1().border_color(text_color)
            })
            .when(!is_disabled && !is_selected, |this| {
                this.cursor_pointer().hover(move |s| s.bg(surface_hover))
            })
            .when(!is_disabled && is_selected, |this| this.cursor_pointer())
            .when(is_disabled, |this| this.opacity(0.4).cursor_not_allowed())
            .child(
                svg()
                    .path(self.icon_path)
                    .w(self.icon_size)
                    .h(self.icon_size)
                    .text_color(icon_color)
                    .when(!is_disabled, |this| {
                        this.group_hover(group_name, move |s| s.text_color(text_color))
                    }),
            );

        if !is_disabled && let Some(on_click) = self.on_click {
            el = el.on_click(move |e, window, cx| {
                on_click(e, window, cx);
            });
        }

        if let Some(label) = tooltip_label {
            el = el.tooltip(move |_, cx| {
                let mut tip = ActionTooltip::new(label.clone());
                if let Some(ref sc) = tooltip_shortcut {
                    tip = tip.shortcut(sc.clone());
                }
                cx.new(|_| tip).into()
            });
        }

        el
    }
}

#[derive(IntoElement)]
pub struct PillButton {
    id: ElementId,
    label: SharedString,
    selected: bool,
    on_click: Option<ClickHandler>,
}

impl PillButton {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected: false,
            on_click: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Selectable for PillButton {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for PillButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let surface_hover = cx.theme().surface_hover();
        let border = cx.theme().border();
        let text_color = cx.theme().text_color();
        let text_muted = cx.theme().text_muted();

        let is_selected = self.selected;

        let mut btn = div()
            .id(self.id)
            .rounded(px(24.0))
            .h(px(36.0))
            .px_5()
            .border_1()
            .border_color(if is_selected { text_color } else { border })
            .bg(if is_selected { surface_hover } else { border })
            .text_color(if is_selected { text_color } else { text_muted })
            .font_weight(if is_selected {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |s| s.bg(surface_hover))
            .child(div().text_xs().child(self.label));

        if let Some(on_click) = self.on_click {
            btn = btn.on_click(move |e, window, cx| {
                on_click(e, window, cx);
            });
        }

        btn
    }
}
