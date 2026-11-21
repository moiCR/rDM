use gpui::{prelude::FluentBuilder as _, *};
use gpui_kit::base::Button;
use gpui_pages::router;

use crate::{
    components::ActionTooltip, lang::ActiveLang, modals::SettingsModalState, state::CoreContext,
    theme::ActiveTheme,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NavigationItem {
    Dashboard,
    Downloads,
    Notifications,
    Settings,
}

impl NavigationItem {
    pub(super) const ALL: [Self; 4] = [
        Self::Dashboard,
        Self::Downloads,
        Self::Notifications,
        Self::Settings,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::Dashboard => "navigation.dashboard",
            Self::Downloads => "navigation.downloads",
            Self::Notifications => "navigation.notifications",
            Self::Settings => "navigation.settings",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Dashboard => "icons/dashboard.svg",
            Self::Downloads => "icons/download.svg",
            Self::Notifications => "icons/bell.svg",
            Self::Settings => "icons/settings.svg",
        }
    }

    pub(super) fn label(self, cx: &App) -> SharedString {
        cx.lang().get(self.key())
    }

    pub(super) fn active(window: &Window, cx: &App) -> Option<Self> {
        if cx
            .try_global::<SettingsModalState>()
            .is_some_and(|state| state.open)
        {
            return Some(Self::Settings);
        }

        let path = router(window, cx).map(|manager| manager.read(cx).current_path().to_owned());
        match path.as_deref() {
            Some("/") => Some(Self::Dashboard),
            Some("/downloads") => Some(Self::Downloads),
            Some("/notifications") => Some(Self::Notifications),
            _ => None,
        }
    }

    fn activate(self, window: &mut Window, cx: &mut App) {
        let path = match self {
            Self::Dashboard => "/",
            Self::Downloads => "/downloads",
            Self::Notifications => "/notifications",
            Self::Settings => {
                cx.set_global(SettingsModalState {
                    open: true,
                    ..Default::default()
                });
                window.refresh();
                return;
            }
        };

        if let Some(manager) = router(window, cx) {
            manager.update(cx, |manager, cx| {
                if manager.current_path() != path
                    && let Err(error) = manager.navigate(path, cx)
                {
                    eprintln!("{error}");
                }
            });
        }
    }

    pub(super) fn sidebar_button(self, active: bool, cx: &App) -> Button {
        let theme = cx.theme();
        let text = theme.text_color();
        let color = if active { text } else { theme.text_muted() };

        let unread = if self == Self::Notifications {
            cx.notifications().read(cx).unread_count()
        } else {
            0
        };
        let accent = theme.accent();

        Button::new(self.key())
            .group(self.key())
            .accessibility_label(cx.lang().get(self.key()))
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
                    .group_hover(self.key(), move |style| style.text_color(text)),
            )
            .child(div().flex_1().truncate().child(cx.lang().get(self.key())))
            .when(unread > 0, |button| {
                button.child(
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_full()
                        .bg(accent.opacity(0.18))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(accent)
                        .child(if unread > 99 {
                            "99+".to_string()
                        } else {
                            unread.to_string()
                        }),
                )
            })
    }

    pub(super) fn dock_button(
        self,
        size: Pixels,
        color: Rgba,
        hover_color: Option<Rgba>,
        label: SharedString,
    ) -> Button {
        Button::new(self.key())
            .group(self.key())
            .accessibility_label(label)
            .size(size)
            .flex_shrink_0()
            .rounded(px(12.0))
            .cursor_pointer()
            .tooltip(move |_, cx| {
                let label = cx.lang().get(self.key());
                cx.new(|_| ActionTooltip::new(label)).into()
            })
            .on_click(move |_, window, cx| self.activate(window, cx))
            .child(
                svg()
                    .path(self.icon())
                    .size(px(19.0))
                    .text_color(color)
                    .when_some(hover_color, |icon, hover_color| {
                        icon.group_hover(self.key(), move |style| style.text_color(hover_color))
                    }),
            )
    }
}
