pub mod sidebar;
pub mod sidebar_item;
pub mod titlebar;

use gpui::*;
use gpui_router::Layout;

use crate::{
    layout::{
        sidebar::{Dock, Sidebar},
        titlebar::Titlebar,
    },
    state::CoreContext,
    theme::ActiveTheme,
    views::{DeleteDownloadModal, NewDownloadModal, SettingsModal},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavigationStyle {
    Normal,
    Dock,
}

impl NavigationStyle {
    pub(crate) fn from_key(key: &str) -> Self {
        match key {
            "dock" => Self::Dock,
            _ => Self::Normal,
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Normal => "classic",
            Self::Dock => "dock",
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct SidebarState {
    pub collapsed: bool,
}

impl Global for SidebarState {}

pub(super) fn toggle_sidebar(window: &mut Window, cx: &mut App) {
    let collapsed = cx
        .try_global::<SidebarState>()
        .is_some_and(|state| state.collapsed);
    cx.set_global(SidebarState {
        collapsed: !collapsed,
    });
    window.refresh();
}

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
                            .children(self.outlet),
                    ),
            )
            .children(dock.then_some(Dock))
            .child(SettingsModal::new())
            .child(NewDownloadModal::new())
            .child(DeleteDownloadModal::new())
            .into_any_element()
    }
}
