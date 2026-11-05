pub mod assets;
pub mod layout;
pub mod router;
pub mod state;
pub mod theme;
pub mod views;

use std::sync::Arc;

use gpui::*;
use rdm_core::Core;

use crate::{assets::Assets, router::RootRouter, state::AppState, theme::ThemeManager};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let core = Arc::new(Core::new()?);

    let app = gpui_kit::application().with_assets(Assets::new());

    app.run(move |cx: &mut App| {
        Assets::new().load_fonts(cx);
        gpui_router::init(cx);
        cx.set_global(ThemeManager::new());
        cx.set_global(AppState::new(core));

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

        let _ = cx.open_window(window_options, |_window, cx| cx.new(RootRouter::new));
    });

    Ok(())
}
