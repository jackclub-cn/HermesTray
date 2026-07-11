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

/// Render a 32×32 tray icon: bold "H" on colored circular background, clean & minimal
fn render_icon(bg_r: u8, bg_g: u8, bg_b: u8) -> Image<'static> {
    const SZ: i32 = 32;
    let w = SZ as usize;
    let mut pixels = Vec::with_capacity(w * w * 4);

    // ── Circle background ──
    let cc = 15.5f64;
    let bg_r2 = 14.5f64.powi(2); // radius² for fast inner check
    let bg_aa = 1.2f64; // anti-alias band

    // ── H glyph (bold, compact, slightly off-center towards top for visual balance) ──
    // Before: h_left=5 h_right=26 h_top=7 h_bot=26  (span 21×19)
    // Now:    compact + bolder
    let h_left = 6;
    let h_right = 25;
    let h_top = 7;
    let h_bot = 26;
    let h_mid = 16; // crossbar
    let stem_w = 4; // wider stems for bold look

    for y in 0..SZ {
        for x in 0..SZ {
            let (mut px_r, mut px_g, mut px_b, mut alpha) = (0u8, 0u8, 0u8, 0u8);

            // ── Background circle ──
            let d2 = (x as f64 - cc).powi(2) + (y as f64 - cc).powi(2);
            if d2 <= bg_r2 {
                // Inside solid circle
                alpha = 255;
                (px_r, px_g, px_b) = (bg_r, bg_g, bg_b);
            } else {
                let dist = d2.sqrt();
                if dist < cc + bg_aa {
                    // AA border
                    let t = (dist - (bg_r2.sqrt())) / bg_aa;
                    let a = ((1.0 - t).clamp(0.0, 1.0) * 255.0) as u8;
                    alpha = a;
                    (px_r, px_g, px_b) = (bg_r, bg_g, bg_b);
                }
                // else fully transparent
            }

            // ── H glyph (white, bold) ──
            let in_left = x >= h_left && x <= h_left + stem_w;
            let in_right = x >= h_right - stem_w && x <= h_right;
            let in_cross = y >= h_mid - 2 && y <= h_mid + 2 && x >= h_left && x <= h_right;

            if (in_left || in_right || in_cross) && y >= h_top && y <= h_bot {
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
    Image::new(static_pixels, SZ as u32, SZ as u32)
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