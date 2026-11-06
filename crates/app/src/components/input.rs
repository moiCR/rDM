use gpui::*;
pub use gpui_kit::component::input::{
    Input, InputContent, InputContentType, InputEditsEvent, InputEvent, InputState, NumberInput,
    NumberInputEvent, NumberStep, StepAction,
};

use crate::theme::ActiveTheme;

#[derive(IntoElement)]
pub struct TextInput {
    id: ElementId,
    state: Entity<InputState>,
    placeholder: Option<SharedString>,
    cleanable: bool,
    width: Option<Pixels>,
    height: Pixels,
    rounded: Pixels,
    bg: Option<Rgba>,
    border_color: Option<Rgba>,
    text_color: Option<Rgba>,
    disabled: bool,
}

impl TextInput {
    pub fn new(id: impl Into<ElementId>, state: &Entity<InputState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            placeholder: None,
            cleanable: false,
            width: None,
            height: px(36.0),
            rounded: px(24.0),
            bg: None,
            border_color: None,
            text_color: None,
            disabled: false,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn cleanable(mut self, cleanable: bool) -> Self {
        self.cleanable = cleanable;
        self
    }

    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Some(width.into());
        self
    }

    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.height = height.into();
        self
    }

    pub fn rounded(mut self, rounded: impl Into<Pixels>) -> Self {
        self.rounded = rounded.into();
        self
    }

    pub fn bg(mut self, bg: impl Into<Rgba>) -> Self {
        self.bg = Some(bg.into());
        self
    }

    pub fn border_color(mut self, border_color: impl Into<Rgba>) -> Self {
        self.border_color = Some(border_color.into());
        self
    }

    pub fn text_color(mut self, text_color: impl Into<Rgba>) -> Self {
        self.text_color = Some(text_color.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for TextInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let bg = self.bg.unwrap_or_else(|| cx.theme().surface());
        let border = self.border_color.unwrap_or_else(|| cx.theme().border());
        let text_color = self.text_color.unwrap_or_else(|| cx.theme().text_color());

        if let Some(placeholder) = self.placeholder {
            self.state.update(cx, |s, cx| {
                s.set_placeholder(placeholder, window, cx);
            });
        }

        let mut input = Input::new(&self.state)
            .id(self.id)
            .appearance(false)
            .text_size(px(13.0))
            .text_color(text_color)
            .w_full()
            .h_full()
            .flex_1();

        if self.cleanable {
            input = input.cleanable(true);
        }

        let mut container = div()
            .flex()
            .items_center()
            .px_3()
            .h(self.height)
            .rounded(self.rounded)
            .bg(bg)
            .border_1()
            .border_color(border)
            .overflow_hidden()
            .child(input);

        if let Some(w) = self.width {
            container = container.w(w);
        } else {
            container = container.w_full();
        }

        container
    }
}
