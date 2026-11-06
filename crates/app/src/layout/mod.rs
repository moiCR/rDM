pub mod sidebar;
pub mod sidebar_item;
pub mod titlebar;

use gpui::*;
use gpui_router::Layout;

use crate::{
    layout::{sidebar::Sidebar, titlebar::Titlebar},
    state::QuickDownloadState,
    theme::ActiveTheme,
    views::SettingsModal,
};

#[derive(Default, Clone, Copy)]
pub struct SidebarState {
    pub collapsed: bool,
}

impl Global for SidebarState {}

#[derive(Default)]
pub struct AppLayout {
    outlet: Option<AnyElement>,
}

impl AppLayout {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Layout for AppLayout {
    fn outlet(&mut self, element: AnyElement) {
        self.outlet = Some(element);
    }

    fn render_layout(self: Box<Self>, window: &mut Window, cx: &mut App) -> AnyElement {
        let collapsed = cx
            .try_global::<SidebarState>()
            .map(|state| state.collapsed)
            .unwrap_or(false);

        let focus_handle = window
            .use_keyed_state("app-layout-focus", cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        if !focus_handle.is_focused(window) {
            window.focus(&focus_handle, cx);
        }

        let spring_config = SpringConfig::new(200.0, 24.0, 1.0);
        let collapse_spring = SpringAnimation::new(spring_config).to(!collapsed);

        div()
            .relative()
            .track_focus(&focus_handle)
            .key_context("AppLayout")
            .on_key_down(move |event, window, cx| {
                if event.keystroke.modifiers.control
                    && event.keystroke.key.eq_ignore_ascii_case("b")
                {
                    let current = cx
                        .try_global::<SidebarState>()
                        .map(|state| state.collapsed)
                        .unwrap_or(false);
                    cx.set_global(SidebarState {
                        collapsed: !current,
                    });
                    window.refresh();
                } else if event.keystroke.modifiers.control
                    && event.keystroke.key.eq_ignore_ascii_case("n")
                {
                    let current = cx
                        .try_global::<QuickDownloadState>()
                        .map(|state| state.expanded)
                        .unwrap_or(false);
                    cx.set_global(QuickDownloadState { expanded: !current });
                    window.refresh();
                }
            })
            .flex()
            .flex_col()
            .size_full()
            .font_family("Geist")
            .bg(cx.theme().background())
            .text_color(cx.theme().text_color())
            .child(Titlebar::new())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .size_full()
                    .child(
                        div()
                            .id("sidebar-container")
                            .overflow_hidden()
                            .h_full()
                            .child(Sidebar::new())
                            .with_spring(
                                "sidebar-collapse-animation",
                                collapse_spring,
                                move |this, phase| {
                                    let width = phase.interpolate_clamped(px(0.0), px(200.0));
                                    this.w(width)
                                },
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .size_full()
                            .overflow_hidden()
                            .children(self.outlet),
                    ),
            )
            .child(SettingsModal::new())
            .into_any_element()
    }
}
