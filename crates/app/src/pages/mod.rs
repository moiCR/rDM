use gpui_pages::prelude::*;

#[derive(Default)]
pub struct HomePage;

impl Page for HomePage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        crate::views::DashboardView::new()
    }
}
