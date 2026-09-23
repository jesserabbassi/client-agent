use crate::ui::{ClientView, Page};
use slint::ComponentHandle;

pub(crate) fn bind(ui: &ClientView) {
    super::activity_controller::bind(ui);
    super::otp_controller::bind(ui);
    super::login_controller::bind(ui, super::otp_controller::begin);
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
            if ui.global::<crate::ui::Activity>().get_session_active() {
                ui.global::<crate::ui::Activity>()
                    .set_message("End your active session before logging out.".into());
                ui.set_page(Page::Dashboard);
                return;
            }
            super::otp_controller::reset(&ui);
            ui.set_authenticated(false);
            ui.set_page(Page::Dashboard);
            ui.set_player_name("".into());
            ui.set_password("".into());
            ui.set_message("".into());
        }
    });
}
