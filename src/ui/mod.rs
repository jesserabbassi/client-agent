slint::include_modules!();

pub(crate) fn enter_compact(ui: &ClientView) {
    use slint::ComponentHandle;
    if ui.get_compact_mode() {
        return;
    }
    let window = ui.window();
    let position = window.position();
    let size = window.size();
    ui.set_full_window_x(position.x);
    ui.set_full_window_y(position.y);
    ui.set_full_window_width(size.width as i32);
    ui.set_full_window_height(size.height as i32);
    ui.set_full_window_maximized(window.is_maximized());
    ui.set_full_window_fullscreen(window.is_fullscreen());

    if window.is_fullscreen() {
        window.set_fullscreen(false);
    }
    if window.is_maximized() {
        window.set_maximized(false);
    }

    let scale = window.scale_factor();
    let (logical_width, x, y, physical_height) = compact_geometry(scale, position, size);
    let width = (logical_width * scale).round() as u32;
    ui.set_compact_width(logical_width);
    ui.set_compact_mode(true);
    window.set_size(slint::PhysicalSize::new(width, physical_height));
    window.set_position(slint::PhysicalPosition::new(x, y));
    window.request_redraw();

    // Windows may apply old native size limits during the same event-loop pass.
    // Repeat after Slint updates the window constraints and surface.
    #[cfg(windows)]
    {
        let weak = ui.as_weak();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = weak.upgrade() {
                if ui.get_compact_mode() {
                    let window = ui.window();
                    window.set_size(slint::PhysicalSize::new(width, physical_height));
                    window.set_position(slint::PhysicalPosition::new(x, y));
                    window.request_redraw();
                }
            }
        });
    }
}

fn compact_geometry(
    scale: f32,
    position: slint::PhysicalPosition,
    size: slint::PhysicalSize,
) -> (f32, i32, i32, u32) {
    #[cfg(target_os = "macos")]
    {
        let display = core_graphics::display::CGDisplay::main();
        let bounds = display.bounds();
        let millimeters = display.screen_size().width;
        if millimeters > 0.0 && bounds.size.width > 0.0 {
            let logical_width = (30.0 * bounds.size.width / millimeters) as f32;
            let physical_width = (logical_width * scale).round() as i32;
            let right = ((bounds.origin.x + bounds.size.width) * f64::from(scale)).round() as i32;
            let top = ((bounds.origin.y + 28.0) * f64::from(scale)).round() as i32;
            return (
                logical_width,
                right - physical_width - (8.0 * scale).round() as i32,
                top,
                (820.0 * scale).round() as u32,
            );
        }
    }
    let logical_width = 150.0;
    let physical_width = (logical_width * scale).round() as u32;
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        };

        let center = POINT {
            x: position.x + (size.width / 2) as i32,
            y: position.y + (size.height / 2) as i32,
        };
        let monitor = unsafe { MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            let margin = (8.0 * scale).round() as i32;
            let x = info.rcWork.right - physical_width as i32 - margin;
            let y = info.rcWork.top + margin;
            let available_height = (info.rcWork.bottom - y - margin).max(480);
            let height = ((820.0 * scale).round() as i32).min(available_height) as u32;
            return (logical_width, x, y, height);
        }
    }
    (
        logical_width,
        position.x + size.width.saturating_sub(physical_width) as i32,
        position.y,
        (820.0 * scale).round() as u32,
    )
}

pub(crate) fn leave_compact(ui: &ClientView) {
    use slint::ComponentHandle;
    if !ui.get_compact_mode() {
        return;
    }
    ui.set_compact_mode(false);
    let window = ui.window();
    window.set_size(slint::PhysicalSize::new(
        ui.get_full_window_width().max(140) as u32,
        ui.get_full_window_height().max(480) as u32,
    ));
    window.set_position(slint::PhysicalPosition::new(
        ui.get_full_window_x(),
        ui.get_full_window_y(),
    ));
    if ui.get_full_window_maximized() {
        window.set_maximized(true);
    }
    if ui.get_full_window_fullscreen() {
        window.set_fullscreen(true);
    }
    window.request_redraw();
}
