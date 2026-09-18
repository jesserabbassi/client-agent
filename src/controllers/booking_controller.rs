use crate::services::booking_service;

use crate::ui::ClientView;
use slint::ComponentHandle;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn clear(ui: &ClientView) {
    ui.set_booking_reference("".into());
    ui.set_booking_total("".into());
    ui.set_booking_summary("".into());
    ui.set_booking_error("".into());
}

pub(crate) fn bind(ui: &ClientView) {
    let weak = ui.as_weak();
    ui.on_booking_confirm(move |date, start, end| {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.get_authenticated()
            || !ui.get_show_booking()
            || !ui.get_station_detail().available
            || !ui.get_booking_reference().is_empty()
            || ui.get_booking_total().is_empty()
        {
            return;
        }
        let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            clear(&ui);
            ui.set_booking_error("Unable to read the system clock.".into());
            return;
        };
        match booking_service::quote(
            date.as_str(),
            start,
            end,
            ui.get_station_detail().rate_cents,
            now.as_secs(),
        ) {
            Ok(cents)
                if ui.get_booking_summary() == summary(&date, start, end)
                    && ui.get_booking_total() == price(cents) =>
            {
                ui.set_booking_reference(format!("DEMO-{}", now.as_nanos()).into());
            }
            Ok(_) => {
                clear(&ui);
                ui.set_booking_error("Your details changed. Review the booking again.".into());
            }
            Err(message) => {
                clear(&ui);
                ui.set_booking_error(message.into());
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_booking_changed(move || {
        if let Some(ui) = weak.upgrade() {
            clear(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.on_booking_review(move |date, start, end| {
        let Some(ui) = weak.upgrade() else { return };
        clear(&ui);
        if !ui.get_authenticated() || !ui.get_show_booking() || !ui.get_station_detail().available {
            return;
        }
        let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            ui.set_booking_error("Unable to read the system clock.".into());
            return;
        };
        match booking_service::quote(
            date.as_str(),
            start,
            end,
            ui.get_station_detail().rate_cents,
            now.as_secs(),
        ) {
            Ok(cents) => {
                ui.set_booking_total(price(cents).into());
                ui.set_booking_summary(summary(&date, start, end).into());
            }
            Err(message) => ui.set_booking_error(message.into()),
        }
    });
}

fn price(cents: i32) -> String {
    format!("{}.{:02} €", cents / 100, cents % 100)
}

fn summary(date: &str, start: i32, end: i32) -> String {
    format!(
        "{date}\n{start:02}:00 – {end:02}:00 UTC\n{} hours",
        end - start
    )
}
