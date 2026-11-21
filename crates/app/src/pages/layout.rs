use gpui::*;
use gpui_pages::PageLayout;

use crate::{
    layout::{
        NavigationStyle, SidebarState,
        sidebar::{Dock, Sidebar},
        titlebar::Titlebar,
        toggle_sidebar,
    },
    modals::{
        CommandPaletteModal, DeleteDownloadModal, DownloadInspectorModal, NewDownloadModal,
        NotificationDetailModal, SettingsModal,
    },
    state::CoreContext,
    theme::ActiveTheme,
};

pub(crate) struct RootLayout(AnyView);

impl PageLayout for RootLayout {
    fn new(child: AnyView, _: &mut Context<Self>) -> Self {
        Self(child)
    }

    fn set_child(&mut self, child: AnyView) {
        self.0 = child;
    }

    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dock = NavigationStyle::from_key(&cx.core().config().ui_config().sidebar_style)
            == NavigationStyle::Dock;
        let collapsed = cx
            .try_global::<SidebarState>()
            .map(|state| state.collapsed)
            .unwrap_or(false);

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

        let spring_config = SpringConfig::new(200.0, 24.0, 1.0);
        let collapse_spring = SpringAnimation::new(spring_config).to(!collapsed);

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
                } else if event.keystroke.modifiers.control
                    && event.keystroke.key.eq_ignore_ascii_case("k")
                {
                    CommandPaletteModal::open(window, cx);
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
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .children((!dock).then(|| {
                        div()
                            .id("sidebar-container")
                            .overflow_hidden()
                            .h_full()
                            .child(Sidebar)
                            .with_spring(
                                "sidebar-collapse-animation",
                                collapse_spring,
                                move |this, phase| {
                                    let width = phase.interpolate_clamped(px(0.0), px(200.0));
                                    this.w(width)
                                },
                            )
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .size_full()
                            .overflow_hidden()
                            .child(self.0.clone()),
                    ),
            )
            .children(dock.then_some(Dock))
            .child(SettingsModal::new())
            .child(NewDownloadModal::new())
            .child(DeleteDownloadModal::new())
            .child(DownloadInspectorModal::new())
            .child(NotificationDetailModal::new())
            .child(CommandPaletteModal::new())
    }
}
