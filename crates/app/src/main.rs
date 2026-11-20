#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod assets;
pub mod components;
pub mod ext;
pub mod lang;
pub mod layout;
#[rustfmt::skip]
#[path = "pages_modules.rs"]
pub mod pages;
pub mod router;
pub mod state;
pub mod theme;
pub mod views;

#[cfg(target_os = "linux")]
mod linux_desktop;
#[cfg(windows)]
mod windows_desktop;

#[cfg(not(target_os = "linux"))]
use crate::router::RootRouter;

use std::sync::Arc;

use gpui::*;
use gpui_pages::{ApplicationExt, PageManager};
use rdm_core::Core;

gpui_pages::include_pages!();

use crate::{
    assets::Assets,
    state::{AppState, TasksModel},
    theme::ThemeManager,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(any(windows, target_os = "linux"))]
    let startup = std::env::args_os().any(|arg| arg == "--startup");
    #[cfg(windows)]
    let Some(_instance) = windows_desktop::Instance::acquire(startup)? else {
        return Ok(());
    };

    #[cfg(target_os = "linux")]
    let Some(mut _instance) = linux_desktop::Instance::acquire(startup)? else {
        return Ok(());
    };
    #[cfg(target_os = "linux")]
    let activation_listener = _instance.take_listener()?;

    let runtime = Arc::new(tokio::runtime::Runtime::new()?);
    let _guard = runtime.enter();

    let core = Arc::new(Core::new()?);
    let ui_config = core.config().ui_config();
    let language = lang::Lang::new(&ui_config.language)?;

    let core_server = Arc::clone(&core);
    runtime.spawn(async move {
        let _ = rdm_core::server::run_server(core_server).await;
    });

    let app = gpui_kit::application().with_assets(Assets::new());

    let launch = move |cx: &mut App| {
        cx.set_global(language);
        #[cfg(windows)]
        let desktop_events = windows_desktop::Desktop::install(cx)
            .inspect_err(|error| eprintln!("No se pudo crear la bandeja de rDM: {error}"))
            .ok();

        Assets::new().load_fonts(cx);
        gpui_kit::init(cx);
        gpui_router::init(cx);

        let mut theme_mgr = ThemeManager::new();
        if let Some(accent) = ui_config.accent_override() {
            theme_mgr.set_accent_hex(accent);
        }
        cx.set_global(theme_mgr);
        ThemeManager::apply_mode(&ui_config.theme, cx);

        let tasks_model = cx.new(|cx| TasksModel::new(Arc::clone(&core), Arc::clone(&runtime), cx));
        cx.set_global(AppState::new(core, tasks_model));

        if cfg!(pages_test) {
            if let Err(error) = open_pages_test(cx) {
                eprintln!("No se pudo abrir el modo PAGES_TEST de rDM: {error}");
                cx.quit();
            }
            return;
        }

        #[cfg(windows)]
        views::SettingsModal::initialize_desktop_preferences(ui_config.close_to_tray, cx);

        #[cfg(target_os = "linux")]
        {
            views::SettingsModal::initialize_desktop_preferences(ui_config.close_to_tray, cx);
            if let Err(error) =
                linux_desktop::install(activation_listener, runtime.handle(), startup, cx)
            {
                eprintln!("No se pudo iniciar la integración Linux: {error}");
                cx.quit();
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            let bounds = Bounds::centered(None, size(px(1024.0), px(680.0)), cx);
            #[cfg(windows)]
            let show = !(startup && windows_desktop::Desktop::available(cx));
            #[cfg(not(windows))]
            let show = true;

            let window_options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                show,
                focus: show,
                titlebar: Some(TitlebarOptions {
                    title: Some("rDM".into()),
                    appears_transparent: true,
                    traffic_light_position: None,
                }),
                ..Default::default()
            };

            match gpui_kit::open_window(window_options, cx, |_window, cx| {
                #[cfg(windows)]
                _window.on_window_should_close(cx, |window, cx| {
                    !(views::SettingsModal::close_to_tray(cx)
                        && windows_desktop::Desktop::available(cx)
                        && windows_desktop::hide_window(window))
                });
                cx.new(RootRouter::new)
            }) {
                Ok((_window, _)) =>
                {
                    #[cfg(windows)]
                    if let Some(events) = desktop_events {
                        events.listen(_window, cx);
                    }
                }
                Err(error) => {
                    eprintln!("No se pudo abrir la ventana de rDM: {error}");
                    cx.quit();
                }
            }
        }
    };

    if cfg!(pages_test) {
        app.use_pages().run_with(launch);
    } else {
        app.run(launch);
    }

    Ok(())
}

struct PagesTestRouter(Entity<PageManager>);

impl Global for PagesTestRouter {}

pub fn pages_test_router(cx: &App) -> Option<Entity<PageManager>> {
    cx.try_global::<PagesTestRouter>()
        .map(|router| router.0.clone())
}

fn open_pages_test(cx: &mut App) -> anyhow::Result<()> {
    let manager = PageManager::new(PAGE_ROUTES, "/", cx)?;
    let manager = cx.new(|_| manager);
    cx.set_global(PagesTestRouter(manager.clone()));

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1024.0), px(680.0)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("rDM - Pages Test".into()),
            appears_transparent: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    gpui_kit::open_window(options, cx, |_, _| manager)?;
    cx.activate(true);
    Ok(())
}
