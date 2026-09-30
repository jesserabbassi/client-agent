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

    let scale = window.scale_factor();
    let (logical_width, x, y) = compact_geometry(scale, position, size);
    let width = (logical_width * scale).round() as u32;
    let height = (820.0 * scale).round() as u32;
    ui.set_compact_width(logical_width);
    ui.set_compact_mode(true);
    window.set_size(slint::PhysicalSize::new(width, height));
    window.set_position(slint::PhysicalPosition::new(x, y));
}

fn compact_geometry(
    scale: f32,
    position: slint::PhysicalPosition,
    size: slint::PhysicalSize,
) -> (f32, i32, i32) {
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
            );
        }
    }
    let logical_width = 150.0;
    let physical_width = (logical_width * scale).round() as u32;
    (
        logical_width,
        position.x + size.width.saturating_sub(physical_width) as i32,
        position.y,
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
        ui.get_full_window_width().max(1120) as u32,
        ui.get_full_window_height().max(830) as u32,
    ));
    window.set_position(slint::PhysicalPosition::new(
        ui.get_full_window_x(),
        ui.get_full_window_y(),
    ));
}
