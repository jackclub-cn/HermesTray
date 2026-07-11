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

/// Render a 32×32 tray icon: "H" letter + status dot at bottom-right
fn render_icon(dot_r: u8, dot_g: u8, dot_b: u8) -> Image<'static> {
    let size: u32 = 32;
    let w = size as i32;
    let mut pixels = Vec::with_capacity((w * w * 4) as usize);

    // "H" bitmap — bolder, centered
    let h_left = 7;
    let h_right = h_left + 7;
    let h_top = 6;
    let h_bot = 28;
    let h_mid = 17; // crossbar

    // Dot position (bottom-right corner)
    let d_cx = 24;
    let d_cy = 24;
    let d_radius = 5;

    for y in 0..w {
        for x in 0..w {
            let (mut px_r, mut px_g, mut px_b, mut alpha) = (0u8, 0u8, 0u8, 0u8);

            // ── H letter (white) ──
            let in_left = x >= h_left && x <= h_left + 2; // left stem
            let in_right = x >= h_right - 2 && x <= h_right; // right stem
            let in_cross = y >= h_mid - 1 && y <= h_mid + 1 && x >= h_left && x <= h_right;

            if (in_left || in_right || in_cross) && y >= h_top && y <= h_bot {
                (px_r, px_g, px_b) = (255, 255, 255);
                alpha = 255;
            }

            // ── Status dot (colored) ──
            let dx = (x - d_cx).abs();
            let dy = (y - d_cy).abs();
            let dist = ((dx * dx + dy * dy) as f64).sqrt();
            if dist < d_radius as f64 {
                (px_r, px_g, px_b) = (dot_r, dot_g, dot_b);
                alpha = 255;
            } else if dist < d_radius as f64 + 1.0 {
                // anti-alias edge
                let t = (dist - d_radius as f64);
                let a = ((1.0 - t) * 255.0) as u8;
                if a > alpha {
                    (px_r, px_g, px_b) = (dot_r, dot_g, dot_b);
                    alpha = a;
                }
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

/// Get a cached tray icon for the given status.
/// Also serves as the init icon (gray dot).
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