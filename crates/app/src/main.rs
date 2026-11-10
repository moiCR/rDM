#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod assets;
pub mod components;
pub mod ext;
pub mod layout;
pub mod router;
pub mod state;
pub mod theme;
pub mod views;

use std::sync::Arc;

use gpui::*;
use rdm_core::Core;

use crate::{
    assets::Assets,
    router::RootRouter,
    state::{AppState, TasksModel},
    theme::ThemeManager,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Arc::new(tokio::runtime::Runtime::new()?);
    let _guard = runtime.enter();

    let core = Arc::new(Core::new()?);
    let ui_config = core.config().ui_config();

    let core_server = Arc::clone(&core);
    runtime.spawn(async move {
        let _ = rdm_core::server::run_server(core_server).await;
    });

    let app = gpui_kit::application().with_assets(Assets::new());

    app.run(move |cx: &mut App| {
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

        let bounds = Bounds::centered(None, size(px(1024.0), px(680.0)), cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("rDM".into()),
                appears_transparent: true,
                traffic_light_position: None,
            }),
            ..Default::default()
        };

        let _ = gpui_kit::open_window(window_options, cx, |_window, cx| cx.new(RootRouter::new));
    });

    Ok(())
}
