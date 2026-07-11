use crate::api::ConnectionStatus;
use tauri::{image::Image, AppHandle};

/// Generate a colored 32x32 RGBA tray icon
pub fn make_tray_icon(r: u8, g: u8, b: u8) -> Image<'static> {
    let size: u32 = 32;
    let usize_sz = size as usize;
    let mut pixels = Vec::with_capacity(usize_sz * usize_sz * 4);
    for y in 0..size {
        for x in 0..size {
            let cx = 16i32;
            let cy = 16i32;
            let dx = (x as i32 - cx).abs();
            let dy = (y as i32 - cy).abs();
            let dist = ((dx * dx + dy * dy) as f64).sqrt();
            let radius = 13.0f64;
            let aa = 1.5f64;
            let alpha = if dist < radius {
                255u8
            } else if dist < radius + aa {
                let t = (dist - radius) / aa;
                ((1.0 - t) * 255.0) as u8
            } else {
                0u8
            };
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(alpha);
        }
    }
    // Leak to get 'static lifetime required by Image::new
    let static_pixels: &'static [u8] = Box::leak(pixels.into_boxed_slice());
    Image::new(static_pixels, size, size)
}

/// Update the tray icon and tooltip based on connection status
pub fn update_tray(app: &AppHandle, status: &ConnectionStatus) {
    let (r, g, b, label) = match status {
        ConnectionStatus::Disconnected => (160u8, 160u8, 160u8, "HermesTray — disconnected"),
        ConnectionStatus::Idle => (80u8, 220u8, 100u8, "HermesTray — idle"),
        ConnectionStatus::Busy => (255u8, 170u8, 50u8, "HermesTray — busy"),
    };

    if let Some(tray) = app.tray_by_id("main") {
        let icon = make_tray_icon(r, g, b);
        let _ = tray.set_icon(Some(icon));
        let _ = tray.set_tooltip(Some(label));
    }
}
