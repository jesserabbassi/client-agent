//! Presentation boundary for the GPUI Supervisor application.
//!
//! Views receive state/actions from ViewModels only. They must not access
//! `ApiClient`, `SignalRClient`, or local storage directly.

use super::{models::StationTelemetry, viewmodels::MonitoringViewModel};

pub struct MonitoringView {
    pub view_model: MonitoringViewModel,
}

#[cfg(feature = "gpui")]
pub mod gpui_shell {
    use gpui::{App, Application, Context, Window, WindowOptions, div, prelude::*, px, rgb};

    pub struct SupervisorShell;

    impl Render for SupervisorShell {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .bg(rgb(0x071522))
                .text_color(rgb(0xe8f1f8))
                .flex()
                .flex_col()
                .child(div().h(px(72.)).bg(rgb(0x0b2032)).p_4().child("NINETY  |  SUPERVISOR APPLICATION"))
                .child(div().flex_1().flex().child(div().w(px(220.)).bg(rgb(0x091c2c)).p_4().child("Dashboard\nGaming Stations\nSessions\nReservations\nCustomers\nWallet / Payments\nMonitoring\nAlerts\nReports\nSettings")))
        }
    }

    pub fn run() {
        Application::new().run(|cx: &mut App| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|_| SupervisorShell)
            })
            .unwrap();
            cx.activate(true);
        });
    }
}

impl MonitoringView {
    pub fn new(view_model: MonitoringViewModel) -> Self {
        Self { view_model }
    }

    pub fn stations(&self) -> impl Iterator<Item = &StationTelemetry> {
        self.view_model.stations()
    }
}
