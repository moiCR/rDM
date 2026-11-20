pub mod sidebar;
pub mod sidebar_item;
pub mod titlebar;

use gpui::*;

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

pub(crate) fn toggle_sidebar(window: &mut Window, cx: &mut App) {
    let collapsed = cx
        .try_global::<SidebarState>()
        .is_some_and(|state| state.collapsed);
    cx.set_global(SidebarState {
        collapsed: !collapsed,
    });
    window.refresh();
}
