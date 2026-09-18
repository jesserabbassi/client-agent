//! Exercise the real callback, worker, and event-loop handoff without a desktop.
use crate::ui::ClientView;
use slint::platform::{
    EventLoopProxy, Platform, WindowAdapter,
    software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
};
use std::{rc::Rc, sync::mpsc, time::Duration};

type Event = Box<dyn FnOnce() + Send>;
struct TestPlatform(mpsc::Sender<Event>, Rc<MinimalSoftwareWindow>);
struct Proxy(mpsc::Sender<Event>);
impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.1.clone())
    }
    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(Proxy(self.0.clone())))
    }
}
impl EventLoopProxy for Proxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        Ok(())
    }
    fn invoke_from_event_loop(&self, event: Event) -> Result<(), slint::EventLoopError> {
        self.0
            .send(event)
            .map_err(|_| slint::EventLoopError::EventLoopTerminated)
    }
}

#[test]
fn login_callback_validates_and_delivers_worker_results() {
    let (sender, events) = mpsc::channel();
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(TestPlatform(sender, window.clone())))
        .expect("test platform");
    let ui = ClientView::new().expect("UI creation");
    super::bind(&ui);
    use slint::ComponentHandle;
    ui.show().expect("show UI");
    for (width, height) in [(1920, 1080), (2560, 1440)] {
        window.set_size(slint::PhysicalSize::new(width, height));
        window.request_redraw();
        let mut pixels = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(width, height);
        assert!(window.draw_if_needed(|renderer| {
            renderer.render(pixels.make_mut_slice(), width as usize);
        }));
        // Optional visual-review artifact, without adding an image dependency.
        if let Some(directory) = std::env::var_os("NINETY_TEST_CAPTURE_DIR") {
            use std::io::Write;
            let path = std::path::PathBuf::from(directory).join(format!("ninety-{width}.ppm"));
            let mut file = std::fs::File::create(path).expect("capture file");
            write!(file, "P6\n{width} {height}\n255\n").expect("capture header");
            file.write_all(pixels.as_bytes()).expect("capture pixels");
        }
    }
    ui.invoke_login_requested(" ".into(), "password".into(), false);
    assert!(!ui.get_busy());
    assert_eq!(ui.get_message(), "Enter your username or email.");
    ui.invoke_login_requested("player".into(), "".into(), false);
    assert_eq!(ui.get_message(), "Enter your password.");

    ui.set_password("wrong".into());
    ui.invoke_login_requested("player".into(), "wrong".into(), false);
    assert!(ui.get_busy());
    assert!(ui.get_password().is_empty());
    // A repeated submission must not start a second worker.
    ui.invoke_login_requested("player".into(), "password".into(), false);
    events
        .recv_timeout(Duration::from_secs(5))
        .expect("worker result")();
    assert!(!ui.get_busy());
    assert!(!ui.get_authenticated());
    let fault = std::env::var("NINETY_MOCK_MODE").unwrap_or_default();
    if fault == "unavailable" || fault == "network-error" || !cfg!(feature = "mock-auth") {
        assert!(ui.get_connection_problem());
        return;
    }
    assert!(ui.get_message().contains("Incorrect"));
    ui.invoke_login_requested("player".into(), "password".into(), true);
    assert!(ui.get_busy());
    events
        .recv_timeout(Duration::from_secs(5))
        .expect("worker result")();
    assert!(ui.get_authenticated());
    assert!(!ui.get_busy());
    assert!(events.try_recv().is_err());
    assert_eq!(ui.get_player_name(), "player");
    for (width, height) in [(1120, 830), (1920, 1080), (2560, 1440)] {
        window.set_size(slint::PhysicalSize::new(width, height));
        window.request_redraw();
        let mut pixels = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(width, height);
        assert!(window.draw_if_needed(|renderer| {
            renderer.render(pixels.make_mut_slice(), width as usize);
        }));
        if let Some(directory) = std::env::var_os("NINETY_TEST_CAPTURE_DIR") {
            use std::io::Write;
            let path = std::path::PathBuf::from(directory).join(format!("dashboard-{width}.ppm"));
            let mut file = std::fs::File::create(path).expect("dashboard capture");
            write!(file, "P6\n{width} {height}\n255\n").unwrap();
            file.write_all(pixels.as_bytes()).unwrap();
        }
    }
    ui.invoke_stations_requested();
    assert!(ui.get_show_stations());
    assert_eq!(ui.get_selected_station(), -1);
    for invalid in [-1, 3, 5, 7, 99] {
        ui.invoke_station_selected(invalid);
        assert_eq!(ui.get_selected_station(), -1);
    }
    ui.invoke_station_selected(2);
    assert_eq!(ui.get_station_detail().name, "PC-03");
    assert_eq!(ui.get_station_detail().gpu, "RTX 4090");
    ui.invoke_station_selected(3);
    assert_eq!(ui.get_selected_station(), 2);
    for (width, height) in [(1120, 830), (1920, 1080), (2560, 1440)] {
        window.set_size(slint::PhysicalSize::new(width, height));
        window.request_redraw();
        let mut pixels = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(width, height);
        assert!(window.draw_if_needed(|renderer| {
            renderer.render(pixels.make_mut_slice(), width as usize);
        }));
        if let Some(directory) = std::env::var_os("NINETY_TEST_CAPTURE_DIR") {
            use std::io::Write;
            let path = std::path::PathBuf::from(directory).join(format!("stations-{width}.ppm"));
            let mut file = std::fs::File::create(path).expect("stations capture");
            write!(file, "P6\n{width} {height}\n255\n").unwrap();
            file.write_all(pixels.as_bytes()).unwrap();
        }
    }
    ui.invoke_booking_requested();
    assert!(ui.get_show_booking());
    ui.invoke_booking_review("2099-02-30".into(), 14, 18);
    assert!(!ui.get_booking_error().is_empty());
    assert!(ui.get_booking_total().is_empty());
    ui.invoke_booking_review("2099-12-31".into(), 14, 18);
    assert_eq!(ui.get_booking_total(), "12.00 €");
    assert_eq!(
        ui.get_booking_summary(),
        "2099-12-31\n14:00 – 18:00 UTC\n4 hours"
    );
    assert!(ui.get_booking_error().is_empty());
    for (width, height) in [(1120, 830), (1920, 1080), (2560, 1440)] {
        window.set_size(slint::PhysicalSize::new(width, height));
        window.request_redraw();
        let mut pixels = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(width, height);
        assert!(window.draw_if_needed(|renderer| {
            renderer.render(pixels.make_mut_slice(), width as usize);
        }));
        if let Some(directory) = std::env::var_os("NINETY_TEST_CAPTURE_DIR") {
            use std::io::Write;
            let path = std::path::PathBuf::from(directory).join(format!("booking-{width}.ppm"));
            let mut file = std::fs::File::create(path).expect("booking capture");
            write!(file, "P6\n{width} {height}\n255\n").unwrap();
            file.write_all(pixels.as_bytes()).unwrap();
        }
    }
    ui.invoke_booking_confirm("2099-12-31".into(), 14, 18);
    let reference = ui.get_booking_reference();
    assert!(reference.starts_with("DEMO-"));
    ui.invoke_booking_confirm("2099-12-31".into(), 14, 18);
    assert_eq!(ui.get_booking_reference(), reference);
    ui.invoke_booking_changed();
    assert!(ui.get_booking_reference().is_empty());
    ui.invoke_booking_confirm("2099-12-31".into(), 14, 18);
    assert!(ui.get_booking_reference().is_empty());
    assert!(ui.get_booking_total().is_empty());
    ui.invoke_stations_requested();
    assert!(!ui.get_show_booking());
    ui.invoke_station_selected(0);
    ui.invoke_booking_requested();
    assert!(ui.get_booking_total().is_empty());
    ui.invoke_stations_requested();
    ui.invoke_station_selected(2);
    ui.invoke_dashboard_requested();
    assert!(!ui.get_show_stations());
    ui.invoke_station_selected(0);
    assert_eq!(ui.get_selected_station(), 2);
    ui.invoke_stations_requested();
    ui.invoke_logout_requested();
    assert!(!ui.get_authenticated());
    assert!(!ui.get_show_stations());
    assert_eq!(ui.get_selected_station(), -1);
    ui.invoke_stations_requested();
    ui.invoke_station_selected(0);
    assert!(!ui.get_show_stations());
    assert_eq!(ui.get_selected_station(), -1);
    ui.invoke_booking_requested();
    ui.invoke_booking_review("2099-12-31".into(), 14, 18);
    assert!(!ui.get_show_booking());
    assert!(ui.get_booking_total().is_empty());
    assert!(ui.get_player_name().is_empty());
    assert!(ui.get_password().is_empty());
    ui.invoke_login_requested("player".into(), "password".into(), false);
    events
        .recv_timeout(Duration::from_secs(5))
        .expect("login after logout")();
    assert!(ui.get_authenticated());
}
