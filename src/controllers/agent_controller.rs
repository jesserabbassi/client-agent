use crate::ui::{ClientView, Page};
use slint::ComponentHandle;

pub(crate) fn bind(ui: &ClientView) {
    super::login_controller::bind(ui, show_dashboard);
    let weak = ui.as_weak();
    ui.on_navigate(move |page| {
        if let Some(ui) = weak.upgrade() {
            if ui.get_authenticated() {
                ui.set_page(page);
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_logout_requested(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_authenticated(false);
            ui.set_page(Page::Dashboard);
            ui.set_player_name("".into());
            ui.set_password("".into());
            ui.set_message("".into());
        }
    });
}
fn show_dashboard(ui: &ClientView, username: String) {
    ui.set_page(Page::Dashboard);
    ui.set_player_name(username.into());
    ui.set_authenticated(true);
}
