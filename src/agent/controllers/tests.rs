//! Exercise the real callback, worker, and event-loop handoff without a desktop.
use crate::ui::ClientView;
use slint::platform::{
    software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    EventLoopProxy, Platform, WindowAdapter,
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
    use crate::ui::Activity;
    use slint::Model;
    ui.set_authenticated(true);
    ui.set_player_name("a-long-player-name@example.com".into());
    ui.global::<Activity>().set_session_active(true);
    ui.global::<Activity>()
        .set_session_label("Station 01 · 1h".into());
    ui.global::<Activity>()
        .set_session_elapsed("00:13:42".into());
    ui.global::<Activity>()
        .set_session_remaining("00:46:18".into());
    ui.global::<Activity>()
        .set_selected_game("Counter-Strike 2".into());
    crate::ui::enter_compact(&ui);
    assert!(ui.get_compact_mode());
    let compact_width = ui.get_compact_width().round() as u32;
    window.set_size(slint::PhysicalSize::new(compact_width, 820));
    window.request_redraw();
    let mut compact_pixels = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(compact_width, 820);
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(compact_pixels.make_mut_slice(), compact_width as usize);
    }));
    if let Some(directory) = std::env::var_os("NINETY_TEST_CAPTURE_DIR") {
        use std::io::Write;
        let path = std::path::PathBuf::from(directory).join("compact-session.ppm");
        let mut file = std::fs::File::create(path).expect("compact capture");
        write!(file, "P6\n{compact_width} 820\n255\n").expect("compact header");
        file.write_all(compact_pixels.as_bytes())
            .expect("compact pixels");
    }
    crate::ui::leave_compact(&ui);
    assert!(!ui.get_compact_mode());
    ui.set_authenticated(false);
    ui.set_player_name("".into());
    ui.global::<Activity>().set_session_active(false);
    ui.global::<Activity>().set_selected_game("".into());
    ui.global::<Activity>().invoke_start_session(1);
    assert!(!ui.global::<Activity>().get_session_active());
    ui.global::<Activity>().invoke_top_up("20".into());
    ui.global::<Activity>().invoke_reserve(0, 1);
    assert_eq!(ui.global::<Activity>().get_balance(), "0.00 €");
    assert_eq!(ui.global::<Activity>().get_reservations().row_count(), 0);

    for (width, height) in [(700, 600), (900, 700), (1920, 1080), (2560, 1440)] {
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
    ui.set_registering(true);
    window.set_size(slint::PhysicalSize::new(700, 600));
    window.request_redraw();
    let mut narrow_pixels = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(700, 600);
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(narrow_pixels.make_mut_slice(), 700);
    }));
    if let Some(directory) = std::env::var_os("NINETY_TEST_CAPTURE_DIR") {
        use std::io::Write;
        let path = std::path::PathBuf::from(directory).join("register-narrow.ppm");
        let mut file = std::fs::File::create(path).expect("register capture");
        write!(file, "P6\n700 600\n255\n").expect("register header");
        file.write_all(narrow_pixels.as_bytes())
            .expect("register pixels");
    }
    ui.set_registering(false);
    ui.set_authenticated(true);
    for page in [
        crate::ui::Page::Dashboard,
        crate::ui::Page::Games,
        crate::ui::Page::Wallet,
    ] {
        ui.set_page(page);
        window.request_redraw();
        assert!(window.draw_if_needed(|renderer| {
            renderer.render(narrow_pixels.make_mut_slice(), 700);
        }));
        if let Some(directory) = std::env::var_os("NINETY_TEST_CAPTURE_DIR") {
            use std::io::Write;
            let path = std::path::PathBuf::from(directory).join(format!("{page:?}-narrow.ppm"));
            let mut file = std::fs::File::create(path).expect("page capture");
            write!(file, "P6\n700 600\n255\n").expect("page header");
            file.write_all(narrow_pixels.as_bytes())
                .expect("page pixels");
        }
        window.set_size(slint::PhysicalSize::new(1120, 830));
        window.request_redraw();
        let mut wide_pixels = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(1120, 830);
        assert!(window.draw_if_needed(|renderer| {
            renderer.render(wide_pixels.make_mut_slice(), 1120);
        }));
        window.set_size(slint::PhysicalSize::new(700, 600));
    }
    ui.set_authenticated(false);
    ui.set_page(crate::ui::Page::Dashboard);
    window.set_size(slint::PhysicalSize::new(700, 600));
    crate::ui::enter_compact(&ui);
    crate::ui::leave_compact(&ui);
    assert_eq!(window.size(), slint::PhysicalSize::new(700, 600));
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
    let activity = ui.global::<Activity>();
    activity.invoke_choose_game("Valorant".into());
    assert!(activity.get_selected_game().is_empty());
    activity.invoke_reserve(0, 2);
    activity.invoke_reserve(0, 2);
    assert_eq!(activity.get_reservations().row_count(), 1);
    activity.invoke_start(1);
    assert!(!activity.get_session_active());
    activity.invoke_top_up("20.00".into());
    activity.invoke_start(1);
    activity.invoke_start(1);
    assert!(activity.get_session_active());
    assert_eq!(ui.get_page(), crate::ui::Page::Games);
    activity.invoke_choose_game("Valorant".into());
    assert_eq!(activity.get_selected_game(), "Valorant");
    activity.invoke_choose_game("Unknown game".into());
    assert_eq!(activity.get_selected_game(), "Valorant");
    activity.invoke_choose_game("Counter-Strike 2".into());
    assert_eq!(activity.get_selected_game(), "Counter-Strike 2");
    assert_eq!(activity.get_balance(), "8.00 €");
    assert_eq!(activity.get_transactions().row_count(), 2);
    ui.invoke_logout_requested();
    assert!(ui.get_authenticated());
    activity.invoke_end();
    assert!(activity.get_selected_game().is_empty());
    assert_eq!(ui.get_page(), crate::ui::Page::Dashboard);
    activity.invoke_end();
    activity.invoke_start(1);
    assert!(!activity.get_session_active());
    assert_eq!(activity.get_balance(), "8.00 €");
    assert_eq!(
        activity.get_reservations().row_data(0).unwrap().status,
        "Completed"
    );

    activity.invoke_start_session(3);
    assert!(!activity.get_session_active());
    assert_eq!(activity.get_reservations().row_count(), 1);
    activity.invoke_start_session(1);
    assert!(activity.get_session_active());
    assert_eq!(ui.get_page(), crate::ui::Page::Games);
    assert_eq!(activity.get_balance(), "2.00 €");
    activity.invoke_start_session(1);
    assert_eq!(activity.get_transactions().row_count(), 3);
    assert_eq!(activity.get_reservations().row_count(), 2);
    activity.invoke_choose_game("Valorant".into());
    assert_eq!(activity.get_selected_game(), "Valorant");
    activity.invoke_end();

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
