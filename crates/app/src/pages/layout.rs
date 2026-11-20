use gpui_pages::prelude::*;

use crate::{
    layout::{NavigationStyle, titlebar::Titlebar, toggle_sidebar},
    state::CoreContext,
    theme::ActiveTheme,
    views::NewDownloadModal,
};

pub(crate) struct RootLayout(AnyView);

impl PageLayout for RootLayout {
    fn new(child: AnyView) -> Self {
        Self(child)
    }

    fn set_child(&mut self, child: AnyView) {
        self.0 = child;
    }

    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dock = NavigationStyle::from_key(&cx.core().config().ui_config().sidebar_style)
            == NavigationStyle::Dock;

        let focus_handle = window
            .use_keyed_state("app-layout-focus", cx, |window, cx| {
                let handle = cx.focus_handle();
                let initial_focus = handle.clone();
                window.defer(cx, move |window, cx| {
                    if window.focused(cx).is_none() {
                        window.focus(&initial_focus, cx);
                    }
                });
                handle
            })
            .read(cx)
            .clone();

        div()
            .relative()
            .track_focus(&focus_handle)
            .key_context("AppLayout")
            .on_key_down(move |event, window, cx| {
                if !dock
                    && event.keystroke.modifiers.control
                    && event.keystroke.key.eq_ignore_ascii_case("b")
                {
                    toggle_sidebar(window, cx);
                } else if event.keystroke.modifiers.control
                    && event.keystroke.key.eq_ignore_ascii_case("n")
                {
                    NewDownloadModal::open(None, window, cx);
                    cx.stop_propagation();
                }
            })
            .flex()
            .flex_col()
            .size_full()
            .font_family("Geist")
            .bg(cx.theme().background())
            .text_color(cx.theme().text_color())
            .child(Titlebar::new().sidebar_toggle(!dock))
            .child(self.0.clone())
    }
}
