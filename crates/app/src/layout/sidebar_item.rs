use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::base::Button;

use crate::{components::ActionTooltip, theme::ActiveTheme, views::SettingsModalState};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NavigationItem {
    Dashboard,
    Downloads,
    Settings,
}

impl NavigationItem {
    pub(super) const ALL: [Self; 3] = [Self::Dashboard, Self::Downloads, Self::Settings];

    fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Downloads => "Downloads",
            Self::Settings => "Ajustes",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Dashboard => "icons/dashboard.svg",
            Self::Downloads => "icons/download.svg",
            Self::Settings => "icons/settings.svg",
        }
    }

    pub(super) fn active(cx: &App) -> Option<Self> {
        if cx
            .try_global::<SettingsModalState>()
            .is_some_and(|state| state.open)
        {
            return Some(Self::Settings);
        }

        match gpui_router::use_location(cx).pathname.as_str() {
            "/" => Some(Self::Dashboard),
            "/downloads" => Some(Self::Downloads),
            _ => None,
        }
    }

    fn activate(self, window: &mut Window, cx: &mut App) {
        let path = match self {
            Self::Dashboard => "/",
            Self::Downloads => "/downloads",
            Self::Settings => {
                cx.set_global(SettingsModalState {
                    open: true,
                    ..Default::default()
                });
                window.refresh();
                return;
            }
        };

        if gpui_router::use_location(cx).pathname != path {
            gpui_router::use_navigate(window, cx).push(path);
        }
    }

    pub(super) fn sidebar_button(self, active: bool, cx: &App) -> Button {
        let theme = cx.theme();
        let text = theme.text_color();
        let color = if active { text } else { theme.text_muted() };

        Button::new(self.label())
            .group(self.label())
            .accessibility_label(self.label())
            .w_full()
            .justify_start()
            .gap_2p5()
            .py_2()
            .px_1()
            .cursor_pointer()
            .text_size(px(18.0))
            .line_height(relative(1.2))
            .font_weight(if active {
                FontWeight::BOLD
            } else {
                FontWeight::MEDIUM
            })
            .text_color(color)
            .hover(move |style| style.text_color(text))
            .on_click(move |_, window, cx| self.activate(window, cx))
            .child(
                svg()
                    .path(self.icon())
                    .size_4()
                    .flex_shrink_0()
                    .text_color(color)
                    .group_hover(self.label(), move |style| style.text_color(text)),
            )
            .child(self.label())
    }

    pub(super) fn dock_button(
        self,
        size: Pixels,
        color: Rgba,
        hover_color: Option<Rgba>,
    ) -> Button {
        Button::new(self.label())
            .group(self.label())
            .accessibility_label(self.label())
            .size(size)
            .flex_shrink_0()
            .rounded(px(12.0))
            .cursor_pointer()
            .tooltip(move |_, cx| cx.new(|_| ActionTooltip::new(self.label())).into())
            .on_click(move |_, window, cx| self.activate(window, cx))
            .child(
                svg()
                    .path(self.icon())
                    .size(px(19.0))
                    .text_color(color)
                    .when_some(hover_color, |icon, hover_color| {
                        icon.group_hover(self.label(), move |style| style.text_color(hover_color))
                    }),
            )
    }
}
