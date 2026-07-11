use crate::api::ConnectionStatus;
use std::sync::OnceLock;
use tauri::image::Image;

/// Pre-rendered tray icons — Hermes' winged sandal motif
struct TrayIcons {
    disconnected: Image<'static>,
    idle: Image<'static>,
    busy: Image<'static>,
}

static TRAY_ICONS: OnceLock<TrayIcons> = OnceLock::new();

/// Render a 32×32 tray icon: a pair of stylized wings (Hermes motif) on a colored circle
fn render_icon(bg_r: u8, bg_g: u8, bg_b: u8) -> Image<'static> {
    let sz: i32 = 32;
    let mut pixels = Vec::with_capacity((sz * sz * 4) as usize);

    // Circle background centered at (15.5, 15.5), radius 14.5
    let cc = 15.5f64;
    let bg_r2 = 14.5f64 * 14.5f64;
    let bg_aa = 1.2f64;

    // Wings: two curved shapes spreading from center
    // Each wing is a set of diagonal feather lines
    // Left wing: from (13,16) sweeping up-left
    // Right wing: from (19,16) sweeping up-right

    for y in 0..sz {
        for x in 0..sz {
            let (mut px_r, mut px_g, mut px_b, mut alpha) = (0u8, 0u8, 0u8, 0u8);

            // ── Background circle ──
            let d2 = (x as f64 - cc).powi(2) + (y as f64 - cc).powi(2);
            if d2 <= bg_r2 {
                alpha = 255;
                (px_r, px_g, px_b) = (bg_r, bg_g, bg_b);
            } else {
                let dist = d2.sqrt();
                if dist < cc + bg_aa {
                    let t = (dist - (bg_r2.sqrt())) / bg_aa;
                    let a = ((1.0 - t).clamp(0.0, 1.0) * 255.0) as u8;
                    alpha = a;
                    (px_r, px_g, px_b) = (bg_r, bg_g, bg_b);
                }
            }

            // ── Wings (white, Hermes messenger motif) ──
            let on_wing = if y >= 8 && y <= 22 {
                // Wing center at y=15, x ranges differently per row
                let mid = 15.5f64;
                let wing_y = y as f64 - mid; // -7..7

                // Left wing feathers (top → bottom: fanning out)
                let left_feather =
                    // Top feather: sweeping left-up
                    (wing_y >= -7.0 && wing_y <= -3.0 && x as f64 >= mid - 15.0 + wing_y.abs() && x as f64 <= mid - 2.0)
                    // Middle feathers
                    || (wing_y >= -3.0 && wing_y <= 0.0 && x as f64 >= mid - 12.0 && x as f64 <= mid - 1.0)
                    // Bottom feathers
                    || (wing_y >= 0.0 && wing_y <= 5.0 && x as f64 >= mid - 10.0 + wing_y * 0.5 && x as f64 <= mid - 1.0)
                    // Core
                    || (wing_y >= -2.0 && wing_y <= 2.0 && x as f64 >= mid - 6.0 && x as f64 <= mid - 2.0);

                // Right wing feathers (mirror)
                let right_feather =
                    (wing_y >= -7.0 && wing_y <= -3.0 && x as f64 >= mid + 2.0 && x as f64 <= mid + 15.0 - wing_y.abs())
                    || (wing_y >= -3.0 && wing_y <= 0.0 && x as f64 >= mid + 1.0 && x as f64 <= mid + 12.0)
                    || (wing_y >= 0.0 && wing_y <= 5.0 && x as f64 >= mid + 1.0 && x as f64 <= mid + 10.0 - wing_y * 0.5)
                    || (wing_y >= -2.0 && wing_y <= 2.0 && x as f64 >= mid + 2.0 && x as f64 <= mid + 6.0);

                left_feather || right_feather
            } else {
                false
            };

            if on_wing {
                (px_r, px_g, px_b) = (255, 255, 255);
                alpha = 255;
            }

            pixels.push(px_r);
            pixels.push(px_g);
            pixels.push(px_b);
            pixels.push(alpha);
        }
    }

    let static_pixels: &'static [u8] = Box::leak(pixels.into_boxed_slice());
    Image::new(static_pixels, sz as u32, sz as u32)
}

fn get_icons() -> &'static TrayIcons {
    TRAY_ICONS.get_or_init(|| TrayIcons {
        disconnected: render_icon(160, 160, 160),
        idle: render_icon(80, 220, 100),
        busy: render_icon(255, 170, 50),
    })
}

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