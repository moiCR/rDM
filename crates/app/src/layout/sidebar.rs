use gpui::*;

use crate::{
    layout::sidebar_item::{SidebarVariant, sidebar_item_with_variant},
    state::CoreContext,
    theme::ActiveTheme,
    views::SettingsModalState,
};

#[derive(IntoElement)]
pub struct Sidebar;

impl Sidebar {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let location = gpui_router::use_location(cx);
        let path = location.pathname.as_str();

        let core = cx.core().clone();
        let ui_cfg = core.config().ui_config();
        let variant = if ui_cfg.sidebar_style == "minimal" {
            SidebarVariant::Minimal
        } else {
            SidebarVariant::Classic
        };

        div()
            .flex()
            .flex_col()
            .items_start()
            .justify_between()
            .w(px(200.0))
            .h_full()
            .bg(cx.theme().background())
            .px_4()
            .pb_5()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_start()
                    .justify_center()
                    .gap_2()
                    .child(sidebar_item_with_variant(
                        "Dashboard",
                        "icons/dashboard.svg",
                        Some("/"),
                        path == "/",
                        None,
                        variant,
                    ))
                    .child(sidebar_item_with_variant(
                        "Downloads",
                        "icons/download.svg",
                        Some("/downloads"),
                        path == "/downloads",
                        None,
                        variant,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_start()
                    .pt_2()
                    .child(sidebar_item_with_variant(
                        "Ajustes",
                        "icons/settings.svg",
                        None,
                        false,
                        Some(Box::new(|window, cx| {
                            cx.set_global(SettingsModalState {
                                open: true,
                                subview: None,
                            });
                            window.refresh();
                        })),
                        variant,
                    )),
            )
    }
}
