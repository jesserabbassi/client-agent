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
    assert!(!ui.get_authenticated());
    assert!(ui.get_otp_pending());
    ui.invoke_navigate(crate::ui::Page::Games);
    assert_eq!(ui.get_page(), crate::ui::Page::Dashboard);
    for (width, height) in [(1120, 830), (1920, 1080), (2560, 1440)] {
        window.set_size(slint::PhysicalSize::new(width, height));
        window.request_redraw();
        let mut pixels = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(width, height);
        assert!(window.draw_if_needed(|renderer| {
            renderer.render(pixels.make_mut_slice(), width as usize);
        }));
        if let Some(directory) = std::env::var_os("NINETY_TEST_CAPTURE_DIR") {
            use std::io::Write;
            let path = std::path::PathBuf::from(directory).join(format!("otp-{width}.ppm"));
            let mut file = std::fs::File::create(path).expect("OTP capture");
            write!(file, "P6\n{width} {height}\n255\n").unwrap();
            file.write_all(pixels.as_bytes()).unwrap();
        }
    }
    ui.set_otp_code("000000".into());
    ui.invoke_otp_verify();
    assert!(!ui.get_authenticated());
    assert!(!ui.get_otp_error().is_empty());
    ui.set_otp_code("123456".into());
    ui.invoke_otp_verify();
    assert!(!ui.get_otp_pending());
    assert!(ui.get_otp_code().is_empty());
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
    for page in [
        crate::ui::Page::Games,
        crate::ui::Page::Wallet,
        crate::ui::Page::Dashboard,
    ] {
        ui.invoke_navigate(page);
        assert_eq!(ui.get_page(), page);
        for (width, height) in [(1120, 830), (1920, 1080), (2560, 1440)] {
            window.set_size(slint::PhysicalSize::new(width, height));
            window.request_redraw();
            let mut pixels = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(width, height);
            assert!(window.draw_if_needed(|renderer| {
                renderer.render(pixels.make_mut_slice(), width as usize);
            }));
            if let Some(directory) = std::env::var_os("NINETY_TEST_CAPTURE_DIR") {
                use std::io::Write;
                let path =
                    std::path::PathBuf::from(directory).join(format!("{page:?}-{width}.ppm"));
                let mut file = std::fs::File::create(path).expect("capture");
                write!(file, "P6\n{width} {height}\n255\n").unwrap();
                file.write_all(pixels.as_bytes()).unwrap();
            }
        }
    }
    ui.invoke_navigate(crate::ui::Page::Wallet);
    ui.invoke_logout_requested();
    assert!(!ui.get_authenticated());
    assert_eq!(ui.get_page(), crate::ui::Page::Dashboard);
    ui.invoke_navigate(crate::ui::Page::Games);
    assert_eq!(ui.get_page(), crate::ui::Page::Dashboard);
    assert!(ui.get_player_name().is_empty());
    assert!(ui.get_password().is_empty());
    ui.invoke_login_requested("player".into(), "password".into(), false);
    events
        .recv_timeout(Duration::from_secs(5))
        .expect("login after logout")();
    assert!(!ui.get_authenticated());
    assert!(ui.get_otp_pending());
    for _ in 0..5 {
        ui.set_otp_code("000000".into());
        ui.invoke_otp_verify();
    }
    ui.set_otp_code("123456".into());
    ui.invoke_otp_verify();
    assert!(!ui.get_authenticated());
    assert_eq!(ui.get_otp_attempts(), 5);
    ui.invoke_otp_cancel();
    assert!(!ui.get_otp_pending());
    assert!(ui.get_otp_code().is_empty());
    assert!(ui.get_player_name().is_empty());
    ui.invoke_otp_verify();
    assert!(!ui.get_authenticated());
}
