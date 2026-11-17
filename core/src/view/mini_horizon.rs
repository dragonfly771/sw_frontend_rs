//! Artificial horizon inside the inner circle of a ring or classic page.
//!
//! It is the very same instrument as the full screen horizon
//! (view/horizon.rs), only scaled to the smaller radius. The static part -
//! outer ring, roll scale, centre point, glider symbol and the outline of
//! the level tube - comes from `wp_horizon_ring`, a scaled copy of the big
//! horizon wallpaper, and the moving part - roll marker, pitch ladder and
//! the level tube ball - is drawn with the identical code, scaled by
//! `s = radius / (width / 2)`. Nothing is invented here.
//!
//! Devices without that wallpaper (`ring_wp_radius == 0`) and inner circles
//! of a different size fall back to a plain rim around the sky/earth fill.

use crate::{
    model::CoreModel,
    utils::Colors,
    view::sprites::{DrawColored, Rotate, SimpleIndicator},
    CoreError, DrawImage,
};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle},
};
#[allow(unused_imports)]
use micromath::F32Ext;
use num::clamp;

/// Draw the attitude indicator into a circle around `center`.
pub fn draw_mini_horizon<D>(
    display: &mut D,
    cm: &CoreModel,
    center: Point,
    radius: i32,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let width = cm.device_const.sizes.display.width as i32;
    let height = cm.device_const.sizes.display.height as i32;
    let sky = cm.palette().horizon.sky;
    let earth = cm.palette().horizon.earth;
    let cmh = &cm.device_const.sizes.horizon;
    // scale of this horizon compared to the full screen one
    let full_r = width as f32 / 2.0;
    let s = radius as f32 / full_r;

    // the scaled wallpaper fits exactly one radius and only in the centre
    let wallpaper = cmh.ring_wp_radius > 0
        && radius == cmh.ring_wp_radius
        && center == cm.device_const.sizes.display.center;

    // without valid attitude only the disc and the static scale are shown
    let blocked = cm.sensor.horizon_blocked();

    // same geometry as the full screen horizon, scaled to the inner circle
    let roll_angle = -clamp(cm.sensor.euler_roll.to_radians(), -1.55, 1.55);
    let m_roll = roll_angle.tan();
    let m_pitch = clamp(cm.sensor.euler_pitch.to_radians(), -1.55, 1.55).tan();
    let horizon_y = center.y + (m_pitch * radius as f32) as i32;

    // Sky and earth, clipped to the circle.
    //
    // The half width of each scan line is computed with INTEGERS, not with
    // a square root: on the target `f32::sqrt` comes from micromath and is
    // the fast bit trick, which is off by more than one percent. At radius
    // 150 that pushed the fill out to radius 152 at some rows - the colour
    // stuck out beyond the ring at a few places. `half` is stepped instead,
    // so every filled pixel satisfies half^2 + dy^2 <= r_fill^2 exactly and
    // the fill can never leave the circle.
    //
    // With the wallpaper the fill stops one pixel inside the radius and the
    // ring of the wallpaper - an exact annulus from radius-2.5 to
    // radius+0.5, 3 px wide like the one of the big horizon - lies on top
    // of it. So the visible border is a mathematically exact circle.
    // Without the wallpaper the fill is one pixel wider and the rim below
    // covers it.
    let r_fill = if wallpaper { radius - 1 } else { radius + 1 };
    let rr = r_fill * r_fill;
    let y_min = (center.y - r_fill).max(0);
    let y_max = (center.y + r_fill).min(height - 1);
    let mut half: i32 = 0;
    for y in y_min..=y_max {
        let dy = y - center.y;
        let d2 = rr - dy * dy;
        if d2 <= 0 {
            continue;
        }
        // largest half with half * half <= d2, stepped from the row before
        while (half + 1) * (half + 1) <= d2 {
            half += 1;
        }
        while half > 0 && half * half > d2 {
            half -= 1;
        }
        if half <= 0 {
            continue;
        }
        let x0 = (center.x - half).max(0);
        let x1 = (center.x + half).min(width - 1);
        if x1 < x0 {
            continue;
        }
        let (sky_from, sky_to, earth_from, earth_to) = if blocked {
            (x0, x1, 0, -1)
        } else if m_roll.abs() < 0.001 {
            if y > horizon_y {
                (0, -1, x0, x1)
            } else {
                (x0, x1, 0, -1)
            }
        } else {
            let xh = center.x + ((y - horizon_y) as f32 / m_roll) as i32;
            if m_roll > 0.0 {
                (xh.max(x0), x1, x0, (xh - 1).min(x1))
            } else {
                (x0, (xh - 1).min(x1), xh.max(x0), x1)
            }
        };
        let base = y as usize * width as usize;
        if sky_to >= sky_from {
            // Safety: coordinates are clamped to the display above
            unsafe {
                display.draw_line_unchecked(
                    base + sky_from as usize,
                    (sky_to - sky_from + 1) as usize,
                    sky,
                );
            }
        }
        if earth_to >= earth_from {
            unsafe {
                display.draw_line_unchecked(
                    base + earth_from as usize,
                    (earth_to - earth_from + 1) as usize,
                    earth,
                );
            }
        }
    }

    if wallpaper {
        // scale, ring, centre point, glider symbol and level tube - the
        // same picture as the big horizon, only smaller
        display.draw_img(
            cm.device_const.images.wp_horizon_ring,
            Point::new(0, 0),
            None,
        )?;
    } else {
        // rim: covers the scan line edge, so the outline is perfectly round
        Circle::with_center(center, (radius * 2) as u32)
            .into_styled(PrimitiveStyle::with_stroke(Colors::DimGray, 3))
            .draw(display)?;
    }

    if blocked {
        return Ok(());
    }

    // roll marker - same sprite as the big horizon
    SimpleIndicator::at_tip(radius - 3, center)
        .rotate(roll_angle)
        .draw_colored(cm.palette().horizon.needle, display)?;

    // pitch scale - identical construction, scaled lengths
    let sin_alpha = (roll_angle + core::f32::consts::FRAC_PI_2).sin();
    let cos_alpha = (roll_angle + core::f32::consts::FRAC_PI_2).cos();
    let scale_len = cmh.pitch_scale_len as f32 * s;
    let dx = (sin_alpha * scale_len) as i32;
    let dy = (cos_alpha * scale_len) as i32;
    let spacing = (width / 9) as f32 * s;
    let dcx = cos_alpha * spacing;
    let dcy = sin_alpha * spacing;
    let style = PrimitiveStyle::with_stroke(cm.palette().horizon.scale, 2);
    for mul in 1_i32..4 {
        let mul_dcx = (mul as f32 * dcx) as i32;
        let mul_dcy = (mul as f32 * dcy) as i32;
        let p1 = Point::new(center.x - dx - mul_dcx, center.y - mul_dcy + dy);
        let p2 = Point::new(center.x + dx - mul_dcx, center.y - mul_dcy - dy);
        Line::new(p1, p2).into_styled(style).draw(display)?;
        let p1 = Point::new(center.x - dx + mul_dcx, center.y + mul_dcy + dy);
        let p2 = Point::new(center.x + dx + mul_dcx, center.y + mul_dcy - dy);
        Line::new(p1, p2).into_styled(style).draw(display)?;
    }

    // level tube ball - same swing geometry, scaled
    const ALPHA_MAX: f32 = (16.5 / 180.0) * core::f32::consts::PI;
    let slip_angle = clamp(cm.sensor.slip_angle.to_radians(), -ALPHA_MAX, ALPHA_MAX);
    let swing = cmh.lt_swing_len as f32 * s;
    // pivot of the swing, relative to the centre of this horizon
    let pivot_y = center.y as f32 - (full_r - cmh.lt_mid as f32) * s;
    let x = center.x + (swing * slip_angle.sin()) as i32;
    let y = (pivot_y + swing * slip_angle.cos()) as i32;
    let ball = ((cmh.lt_rad as f32 * s) as u32).max(6);
    Circle::with_center(Point::new(x, y), ball)
        .into_styled(PrimitiveStyle::with_fill(Colors::Black))
        .draw(display)?;
    Ok(())
}
