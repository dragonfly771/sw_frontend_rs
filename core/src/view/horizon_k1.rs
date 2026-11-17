//! Artificial horizon K1 / H1 (plan_ring_design_p1.md ch. 8, sketches
//! skizze_horizont_v4.png and skizze_horizont_sollfahrt.png)
//!
//! - colours K1: sky (90,170,230), earth (150,85,35); the roll band on top
//!   is the same colour x 0.6 (two CLUT colours, no alpha)
//! - full screen r 240 with a white rim 2.5 px; in the ring (H1) r 192 =
//!   exactly the core, without rim - blue / brown end at the speed to fly
//!   channel
//! - pitch linear in degrees: 6 px/deg full screen, 4.6 px/deg in the ring;
//!   horizon line white; ladder every 5 deg (long with numbers at 10/20/30)
//!   below the roll band
//! - roll band: darkened ring segment +-62 deg, 46 px wide,
//!   ticks at its inner edge every 5 deg (long + number at 15/30/45/60),
//!   fixed zero index (white triangle, tip inwards), split roll pointer
//!   (blunt triangle + bar, always centred) turning with the horizon
//! - glider symbol: wings with upturned winglets and a dot, white with a
//!   thin black outline; level tube at the bottom (variant A)
//!
//! Each row of the fill is split into at most a handful of spans (sky,
//! dark sky, dark earth, earth) with exact boundaries, so the picture
//! costs about one span write per pixel row and colour.

#[allow(unused_imports)]
use crate::utils::FClamp;
use crate::{
    utils::Colors,
    view::{fonts_p1::text_for, raster::*},
    CoreError, CoreModel, DrawImage,
};
use embedded_graphics::prelude::*;
use heapless::Vec;
#[allow(unused_imports)]
use micromath::F32Ext;
use u8g2_fonts::types::{HorizontalAlignment, VerticalPosition};

/// half width of the roll band in degrees
const BAND_DEG: f32 = 62.0;
const LADDER: Colors = Colors::AliceBlue;

/// Draw the horizon into a circle of `radius` around the display centre.
/// `full`: full screen variant (white rim, 6 px/deg), else the H1 core of
/// the ring page (4.6 px/deg, no rim).
pub fn draw_horizon_k1<D>(
    display: &mut D,
    cm: &CoreModel,
    radius: f32,
    full: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let r = radius;
    let s = r / 240.0;
    let kp = if full { 6.0 } else { 4.6 };
    let g = geo(cm, 1.0, r.ceil() as i32);
    let (cx, cy) = (g.cx, g.cy);
    let blocked = cm.sensor.horizon_blocked();
    let (roll, pitch) = if blocked {
        (0.0, 0.0)
    } else {
        (
            cm.sensor.euler_roll.to_degrees().fclamp(-89.0, 89.0),
            cm.sensor.euler_pitch.to_degrees().fclamp(-60.0, 60.0),
        )
    };
    let band_w = 46.0 * s + 4.0;
    let rb = r - band_w;
    let tube_y = 145.0 * s;
    let phi = roll.to_radians();
    let (sn, cs) = (phi.sin(), phi.cos());
    let off = kp * pitch;
    let tan_band = BAND_DEG.to_radians().tan();

    // ---------------- sky / earth / roll band ----------------
    let y_start = (cy - r).floor() as i32;
    let y_end = ((cy + tube_y).ceil() as i32).min((cy + r).ceil() as i32);
    for y in y_start..y_end {
        let dy = y as f32 + 0.5 - cy;
        if dy >= tube_y {
            break;
        }
        let ho2 = r * r - dy * dy;
        if ho2 <= 0.0 {
            continue;
        }
        let ho = isqrt_f(ho2);
        let mut xs: Vec<f32, 8> = Vec::new();
        let _ = xs.push(-ho);
        let _ = xs.push(ho);
        // roll band boundaries in this row (top half only)
        let in_band = |x: f32| -> bool {
            if dy >= 0.0 {
                return false;
            }
            let rr = x * x + dy * dy;
            rr >= rb * rb && x.abs() <= tan_band * (-dy)
        };
        if dy < 0.0 && !blocked {
            let amax = tan_band * (-dy);
            if amax < ho {
                let _ = xs.push(-amax);
                let _ = xs.push(amax);
            }
            if dy.abs() < rb {
                let hi = isqrt_f(rb * rb - dy * dy);
                let _ = xs.push(-hi);
                let _ = xs.push(hi);
            }
        }
        // horizon crossing
        if !blocked && sn.abs() > 1e-4 {
            let _ = xs.push((off - dy * cs) / sn);
        }
        // sort, keep inside the chord
        for i in 1..xs.len() {
            let mut j = i;
            while j > 0 && xs[j - 1] > xs[j] {
                xs.swap(j - 1, j);
                j -= 1;
            }
        }
        for w in xs.windows(2) {
            let (xa, xb) = (w[0].max(-ho), w[1].min(ho));
            if xb <= xa {
                continue;
            }
            let xm = 0.5 * (xa + xb);
            let earth = !blocked && xm * sn + dy * cs > off;
            let col = match (earth, in_band(xm) && !blocked) {
                (false, false) => Colors::SkyK1,
                (false, true) => Colors::SkyK1Dark,
                (true, false) => Colors::EarthK1,
                (true, true) => Colors::EarthK1Dark,
            };
            let a = (cx + xa - 0.5).ceil() as i32;
            let b = (cx + xb - 0.5).ceil() as i32 - 1;
            span(display, &g, y, a, b, col)?;
        }
    }

    let gt = g.with_band(-r, tube_y - 1.0);
    // ---------------- horizon line and pitch ladder ----------------
    // along the horizon: t = (cos, -sin), across: n = (sin, cos)
    let p = |along: f32, across: f32| -> (f32, f32) {
        (cx + cs * along + sn * across, cy - sn * along + cs * across)
    };
    if !blocked {
        let mut gl = gt;
        gl.r_clip = (r - 1.0) as i32;
        line(
            display,
            &gl,
            p(-r, off),
            p(r, off),
            2.0 * s.max(1.0),
            Colors::White,
            false,
        )?;
        let r_lad = rb - 8.0;
        let font = text_for(15).f; // sketch fub15 (x0.85 in the ring)
        for deg in (-30i32..=30).step_by(5) {
            if deg == 0 {
                continue;
            }
            let a = off - kp * deg as f32;
            let half = if deg % 10 == 0 { 40.0 * s } else { 18.0 * s };
            let e = half + 26.0 * s;
            if isqrt_f(e * e + a * a) > r_lad {
                continue;
            }
            line(display, &gt, p(-half, a), p(half, a), 2.0, LADDER, false)?;
            if deg % 10 == 0 {
                let mut b = [0u8; 4];
                let t = num_str(deg.unsigned_abs(), &mut b);
                for side in [-1.0f32, 1.0] {
                    let (x, y) = p(side * (half + 16.0 * s), a);
                    if y < cy + tube_y - 8.0 {
                        value_text(
                            display,
                            font,
                            t,
                            Point::new(round(x), round(y)),
                            VerticalPosition::Center,
                            HorizontalAlignment::Center,
                            Colors::White,
                        )?;
                    }
                }
            }
        }
    }

    // ---------------- roll scale in the band ----------------
    let font = text_for(15).f; // sketch fub15
    for deg in (5..=60).step_by(5) {
        for sg in [-1.0f32, 1.0] {
            let a = (sg * deg as f32).to_radians();
            let major = deg % 15 == 0;
            let ln = if major { 15.0 } else { 7.0 } * s + 2.0;
            let wd = if major { 3.0 } else { 2.0 };
            let p0 = (cx + a.sin() * (rb + 1.0), cy - a.cos() * (rb + 1.0));
            let p1 = (
                cx + a.sin() * (rb + 1.0 + ln),
                cy - a.cos() * (rb + 1.0 + ln),
            );
            line(display, &g, p0, p1, wd, Colors::White, false)?;
            if major {
                let rt = rb + ln + 11.0 * s + 3.0;
                let mut b = [0u8; 4];
                let t = num_str(deg, &mut b);
                let q = Point::new(round(cx + a.sin() * rt), round(cy - a.cos() * rt));
                value_text(
                    display,
                    font,
                    t,
                    q,
                    VerticalPosition::Center,
                    HorizontalAlignment::Center,
                    Colors::White,
                )?;
            }
        }
    }
    // fixed zero index: triangle at the inner edge, tip inwards
    let w = 8.0 * s + 2.0;
    fill_poly(
        display,
        &g,
        &[
            (cx, cy - rb + 2.0),
            (cx - w, cy - rb - 13.0 * s - 2.0),
            (cx + w, cy - rb - 13.0 * s - 2.0),
        ],
        Colors::White,
    )?;

    // ---------------- roll pointer ----------------
    if !blocked {
        let rz = rb - 1.0;
        let ang = (-roll).to_radians();
        let (ux, uy) = (ang.sin(), -ang.cos());
        let (vx, vy) = (ang.cos(), ang.sin());
        let rot = |px: f32, py: f32| -> (f32, f32) {
            (cx + ux * (rz - py) + vx * px, cy + uy * (rz - py) + vy * px)
        };
        let h1 = 17.0 * s + 3.0;
        let w1 = 11.0 * s + 3.0;
        let gap = 3.0 * s + 1.0;
        let h2 = 6.0 * s + 2.0;
        let tri = [rot(0.0, 0.0), rot(-w1, h1), rot(w1, h1)];
        let bar = [
            rot(-w1, h1 + gap),
            rot(w1, h1 + gap),
            rot(w1 + 1.5 * s, h1 + gap + h2),
            rot(-w1 - 1.5 * s, h1 + gap + h2),
        ];
        for poly in [&tri[..], &bar[..]] {
            outlined_poly(display, &g, poly, Colors::White)?;
        }
    }

    // ---------------- glider symbol ----------------
    for (col, wd) in [(Colors::Black, 5.0 * s + 3.0), (Colors::White, 5.0 * s)] {
        for side in [-1.0f32, 1.0] {
            line(
                display,
                &g,
                (cx + side * 48.0 * s, cy),
                (cx + side * 192.0 * s, cy),
                wd,
                col,
                true,
            )?;
            line(
                display,
                &g,
                (cx + side * 192.0 * s, cy),
                (cx + side * 202.0 * s, cy),
                wd,
                col,
                true,
            )?;
            line(
                display,
                &g,
                (cx + side * 200.0 * s, cy),
                (cx + side * 206.0 * s, cy - 34.0 * s),
                wd,
                col,
                true,
            )?;
        }
    }
    disc(display, &g, cx, cy, 8.0 * s + 1.0, Colors::Black)?;
    disc(display, &g, cx, cy, 8.0 * s, Colors::White)?;

    // ---------------- level tube ----------------
    level_tube(display, cm, &g, s)?;

    if blocked {
        text(
            display,
            text_for(18).f,
            "no attitude",
            Point::new(round(cx), round(cy + 70.0 * s)),
            VerticalPosition::Center,
            HorizontalAlignment::Center,
            Colors::Black,
        )?;
    }

    // ---------------- rim (full screen only) ----------------
    if full {
        ring(display, &g, cx, cy, r - 2.5, r, Colors::White)?;
    }
    Ok(())
}

/// polygon with a 1 px black outline
fn outlined_poly<D>(
    display: &mut D,
    g: &Geo,
    pts: &[(f32, f32)],
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let mut q: Vec<(f32, f32), 8> = Vec::new();
    for (ox, oy) in [(-1.2f32, 0.0f32), (1.2, 0.0), (0.0, -1.2), (0.0, 1.2)] {
        q.clear();
        for &(x, y) in pts {
            let _ = q.push((x + ox, y + oy));
        }
        fill_poly(display, g, &q, Colors::Black)?;
    }
    fill_poly(display, g, pts, col)
}

fn num_str(v: u32, buf: &mut [u8; 4]) -> &str {
    let v = v.min(999);
    let mut n = 0;
    if v >= 100 {
        buf[n] = b'0' + (v / 100) as u8;
        n += 1;
    }
    if v >= 10 {
        buf[n] = b'0' + ((v / 10) % 10) as u8;
        n += 1;
    }
    buf[n] = b'0' + (v % 10) as u8;
    n += 1;
    core::str::from_utf8(&buf[..n]).unwrap_or("")
}

/// Level tube (slip indicator) at the bottom: the geometry of the old
/// wallpaper (swing radius, pivot, ball size from the device constants),
/// drawn as a light tube with two centre marks and the black ball
fn level_tube<D>(display: &mut D, cm: &CoreModel, g: &Geo, s: f32) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let cmh = &cm.device_const.sizes.horizon;
    let full_r = cm.device_const.sizes.display.width as f32 / 2.0;
    let swing = cmh.lt_swing_len as f32 * s;
    let pivot_y = g.cy - (full_r - cmh.lt_mid as f32) * s;
    let ball_r = (cmh.lt_rad as f32 * s / 2.0).max(4.0);
    let alpha_max = 16.5f32.to_radians();
    let tube_half = ball_r + 3.0 * s + 1.0;
    let a_end = alpha_max + (ball_r + 4.0) / swing;
    let sector = |r_in: f32, r_out: f32, a: f32| -> Vec<(f32, f32), 32> {
        let mut v: Vec<(f32, f32), 32> = Vec::new();
        let n = 12;
        for i in 0..=n {
            let t = -a + 2.0 * a * i as f32 / n as f32;
            let _ = v.push((g.cx + r_out * t.sin(), pivot_y + r_out * t.cos()));
        }
        for i in (0..=n).rev() {
            let t = -a + 2.0 * a * i as f32 / n as f32;
            let _ = v.push((g.cx + r_in * t.sin(), pivot_y + r_in * t.cos()));
        }
        v
    };
    let outer = sector(
        swing - tube_half - 1.5,
        swing + tube_half + 1.5,
        a_end + 0.006,
    );
    fill_poly(display, g, &outer, Colors::Black)?;
    // rounded ends like the sketch
    for t in [-a_end, a_end] {
        disc(
            display,
            g,
            g.cx + swing * t.sin(),
            pivot_y + swing * t.cos(),
            tube_half + 1.5,
            Colors::Black,
        )?;
    }
    let inner = sector(swing - tube_half, swing + tube_half, a_end);
    fill_poly(display, g, &inner, Colors::LightYellow)?;
    for t in [-a_end, a_end] {
        disc(
            display,
            g,
            g.cx + swing * t.sin(),
            pivot_y + swing * t.cos(),
            tube_half,
            Colors::LightYellow,
        )?;
    }
    // centre marks one ball width apart
    let am = (ball_r + 1.5) / swing;
    for t in [-am, am] {
        line(
            display,
            g,
            (
                g.cx + (swing - tube_half) * t.sin(),
                pivot_y + (swing - tube_half) * t.cos(),
            ),
            (
                g.cx + (swing + tube_half) * t.sin(),
                pivot_y + (swing + tube_half) * t.cos(),
            ),
            2.0,
            Colors::Black,
            false,
        )?;
    }
    if !cm.sensor.horizon_blocked() {
        let slip = cm
            .sensor
            .slip_angle
            .to_radians()
            .fclamp(-alpha_max, alpha_max);
        disc(
            display,
            g,
            g.cx + swing * slip.sin(),
            pivot_y + swing * slip.cos(),
            ball_r,
            Colors::Black,
        )?;
    }
    Ok(())
}
