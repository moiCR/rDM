use std::sync::Arc;

use gpui::{App, Context};
use rdm_core::{Core, Engine};

pub struct AppState {
    pub core: Arc<Core>,
}

impl AppState {
    pub fn new(core: Arc<Core>) -> Self {
        Self { core }
    }
}

impl gpui::Global for AppState {}

pub trait CoreContext {
    fn core(&self) -> &Arc<Core>;
    fn engine(&self) -> &Arc<Engine> {
        self.core().engine()
    }
}

impl<'a, T> CoreContext for Context<'a, T> {
    fn core(&self) -> &Arc<Core> {
        &self.global::<AppState>().core
    }
}

impl CoreContext for App {
    fn core(&self) -> &Arc<Core> {
        &self.global::<AppState>().core
    }
}
