use crate::api::ConnectionStatus;
use std::sync::OnceLock;
use tauri::image::Image;

/// Pre-rendered tray icons (only 3 possible colors, cached forever)
struct TrayIcons {
    disconnected: Image<'static>,
    idle: Image<'static>,
    busy: Image<'static>,
}

static TRAY_ICONS: OnceLock<TrayIcons> = OnceLock::new();

fn render_icon(r: u8, g: u8, b: u8) -> Image<'static> {
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
    // Leak once — only 3 images total, ~4KB each, lives for app lifetime
    let static_pixels: &'static [u8] = Box::leak(pixels.into_boxed_slice());
    Image::new(static_pixels, size, size)
}

fn get_icons() -> &'static TrayIcons {
    TRAY_ICONS.get_or_init(|| TrayIcons {
        disconnected: render_icon(160, 160, 160),
        idle: render_icon(80, 220, 100),
        busy: render_icon(255, 170, 50),
    })
}

/// Get a cached tray icon for the given status
pub fn make_tray_icon(r: u8, g: u8, b: u8) -> Image<'static> {
    // Used only for the initial gray icon at startup
    let icons = get_icons();
    if r == 160 && g == 160 && b == 160 {
        icons.disconnected.clone()
    } else if r == 80 && g == 220 && b == 100 {
        icons.idle.clone()
    } else if r == 255 && g == 170 && b == 50 {
        icons.busy.clone()
    } else {
        render_icon(r, g, b)
    }
}

/// Update the tray icon and tooltip based on connection status
pub fn update_tray(app: &tauri::AppHandle, status: &ConnectionStatus) {
    let (icon, label) = match status {
        ConnectionStatus::Disconnected => (&get_icons().disconnected, "HermesTray — disconnected"),
        ConnectionStatus::Idle => (&get_icons().idle, "HermesTray — idle"),
        ConnectionStatus::Busy => (&get_icons().busy, "HermesTray — busy"),
    };

    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_icon(Some(icon.clone()));
        let _ = tray.set_tooltip(Some(label));
    }
}
