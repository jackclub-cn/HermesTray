use crate::api::ConnectionStatus;
use std::sync::OnceLock;
use tauri::image::Image;

/// Pre-rendered tray icons — cached once, reused forever
struct TrayIcons {
    disconnected: Image<'static>,
    idle: Image<'static>,
    busy: Image<'static>,
}

static TRAY_ICONS: OnceLock<TrayIcons> = OnceLock::new();

/// Render a 32×32 tray icon: "H" letter on a colored circular background
fn render_icon(bg_r: u8, bg_g: u8, bg_b: u8) -> Image<'static> {
    let size: i32 = 32;
    let w = size as usize;
    let mut pixels = Vec::with_capacity(w * w * 4);

    // Bounding box for the H letter — wider than before
    let h_left = 5;
    let h_right = 26;
    let h_top = 7;
    let h_bot = 26;
    let h_mid = 16; // crossbar

    // Circle background — centered
    let cc = 15.5; // center (15,15) with anti-aliasing
    let bg_radius = 15.0;
    let aa = 1.2;

    for y in 0..size {
        for x in 0..size {
            let (mut px_r, mut px_g, mut px_b, mut alpha) = (0u8, 0u8, 0u8, 0u8);

            // ── Circular background ──
            let dx = (x as f64 - cc).powi(2);
            let dy = (y as f64 - cc).powi(2);
            let dist = (dx + dy).sqrt();

            if dist < bg_radius {
                alpha = 255;
                (px_r, px_g, px_b) = (bg_r, bg_g, bg_b);
            } else if dist < bg_radius + aa {
                let t = (dist - bg_radius) / aa;
                let a = ((1.0 - t) * 255.0) as u8;
                alpha = a;
                (px_r, px_g, px_b) = (bg_r, bg_g, bg_b);
            }

            // ── H letter (white, overlay on background) ──
            let stem_w = 3; // width of each vertical stem
            let in_left = x >= h_left && x <= h_left + stem_w;
            let in_right = x >= h_right - stem_w && x <= h_right;
            let in_cross = y >= h_mid - 1 && y <= h_mid + 1 && x >= h_left && x <= h_right;

            if (in_left || in_right || in_cross) && y >= h_top && y <= h_bot {
                (px_r, px_g, px_b) = (255, 255, 255);
                alpha = 255; // solid white letter, no AA needed
            }

            pixels.push(px_r);
            pixels.push(px_g);
            pixels.push(px_b);
            pixels.push(alpha);
        }
    }

    let static_pixels: &'static [u8] = Box::leak(pixels.into_boxed_slice());
    Image::new(static_pixels, size as u32, size as u32)
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