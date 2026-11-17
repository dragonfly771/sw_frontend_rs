//! Volume panel (controller/volume_panel.rs): three rows - pictogram,
//! slider, number - for circling, cruise and warnings. The cursor row has
//! the blue frame; while its value is changed the number field is blue.

use crate::{
    controller::volume_panel::{self as vp, ROWS, VOLUME_MAX},
    utils::Colors,
    view::{
        fonts_p1::{render_value, VALUE_21},
        ring_menu::header,
    },
    CoreError, CoreModel, DrawImage,
};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle, RoundedRectangle},
};
use u8g2_fonts::types::{HorizontalAlignment, VerticalPosition};

const ROW_PITCH: i32 = 64;
const SLIDER_X0: i32 = -96;
const SLIDER_X1: i32 = 74;

/// pictograms, about 30 px: circling arrow, arrow down to the right
/// (cruise), warning triangle
fn icon<D>(display: &mut D, row: u8, p: Point, col: Colors) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    match row {
        vp::ROW_CIRCLING => circling_icon(display, p, col, CIRCLING_ICON)?,
        vp::ROW_CRUISE => cruise_icon(display, p, col)?,
        _ => warning_icon(display, p, col)?,
    }
    Ok(())
}

/// geometry of the circling pictogram: radius and width of the ring, angle
/// (clockwise from 12 o'clock) where the ring starts and where the arrow
/// head sits, length and half width of the head
#[derive(Clone, Copy)]
pub struct CirclingIcon {
    pub r: f32,
    pub w: f32,
    pub start_deg: f32,
    pub end_deg: f32,
    pub head_len: f32,
    pub head_half: f32,
}

/// variant C of the sketch (Stefan 28.10.2026): head at half past one
pub const CIRCLING_ICON: CirclingIcon = CirclingIcon {
    r: 10.5,
    w: ICON_W,
    start_deg: 120.0,
    end_deg: 45.0,
    head_len: 10.0,
    head_half: 7.5,
};

/// stroke of all three pictograms
const ICON_W: f32 = 4.0;

/// every pixel within `reach` of p for which `inside` holds (exact edges)
fn fill_shape<D, F>(
    display: &mut D,
    p: Point,
    reach: i32,
    col: Colors,
    inside: F,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
    F: Fn(f32, f32) -> bool,
{
    for y in -reach..=reach {
        for x in -reach..=reach {
            if inside(x as f32, y as f32) {
                display.draw_iter(core::iter::once(Pixel(p + Point::new(x, y), col)))?;
            }
        }
    }
    Ok(())
}

/// side of the point (x, y) to the line a -> b (cross product)
fn side(a: (f32, f32), b: (f32, f32), x: f32, y: f32) -> f32 {
    (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0)
}

fn in_triangle(t: [(f32, f32); 3], x: f32, y: f32) -> bool {
    let (d1, d2, d3) = (
        side(t[0], t[1], x, y),
        side(t[1], t[2], x, y),
        side(t[2], t[0], x, y),
    );
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

/// cruise: an arrow down to the right, the shaft like the circling ring,
/// the head about 60 deg (sharper than a right angle, Stefan 28.10.2026)
fn cruise_icon<D>(display: &mut D, p: Point, col: Colors) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    // along the diagonal u = (x + y) / sqrt 2, across v = (y - x) / sqrt 2
    const S: f32 = core::f32::consts::FRAC_1_SQRT_2;
    const TIP: f32 = 20.0;
    const BASE: f32 = 7.5;
    const HALF: f32 = 7.2;
    fill_shape(display, p, 16, col, |x, y| {
        let u = (x + y) * S;
        let v = (y - x) * S;
        let shaft = (-15.0..=BASE + 0.5).contains(&u) && v.abs() <= ICON_W / 2.0;
        let head = (BASE..=TIP).contains(&u) && v.abs() <= HALF * (TIP - u) / (TIP - BASE);
        shaft || head
    })
}

/// Mode symbol of the ring page (Stefan 15.11.2026, sketch A of
/// modus_symbol_vorschlaege.png): the circling ring in vario mode, the
/// arrow down to the right in speed to fly mode - the pictograms of the
/// volume panel, with a thicker stroke (the same for both) and the head of
/// the ring further round towards 3 o'clock.
pub const MODE_ICON_W: f32 = 5.0;
pub const MODE_CIRCLING_ICON: CirclingIcon = CirclingIcon {
    r: 12.0,
    w: MODE_ICON_W,
    start_deg: 185.0,
    end_deg: 75.0,
    head_len: 10.0,
    head_half: 8.5,
};

/// the same symbol at 3/4 size for the outer band of the scale (horizon page)
pub const MODE_CIRCLING_ICON_SMALL: CirclingIcon = CirclingIcon {
    r: 9.0,
    w: 4.0,
    start_deg: 185.0,
    end_deg: 75.0,
    head_len: 7.5,
    head_half: 6.5,
};

pub fn mode_icon<D>(
    display: &mut D,
    p: Point,
    col: Colors,
    cruise: bool,
    small: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    if !cruise {
        return circling_icon(
            display,
            p,
            col,
            if small {
                MODE_CIRCLING_ICON_SMALL
            } else {
                MODE_CIRCLING_ICON
            },
        );
    }
    // along the diagonal u, across v - like cruise_icon, a little shorter
    const S: f32 = core::f32::consts::FRAC_1_SQRT_2;
    let k = if small { 0.75 } else { 1.0 };
    let (tail, base, tip, half) = (-15.0 * k, 6.5 * k, 17.5 * k, 8.5 * k);
    let w = if small { 4.0 } else { MODE_ICON_W };
    fill_shape(display, p, 15, col, |x, y| {
        let u = (x + y) * S;
        let v = (y - x) * S;
        let shaft = (tail..=base + 0.5).contains(&u) && v.abs() <= w / 2.0;
        let head = (base..=tip).contains(&u) && v.abs() <= half * (tip - u) / (tip - base);
        shaft || head
    })
}

/// distance of (x, y) to the segment a - b
fn seg_dist(a: (f32, f32), b: (f32, f32), x: f32, y: f32) -> f32 {
    #[allow(unused_imports)]
    use micromath::F32Ext;
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = (((x - a.0) * dx + (y - a.1) * dy) / l2).clamp(0.0, 1.0);
    let (px, py) = (a.0 + t * dx - x, a.1 + t * dy - y);
    (px * px + py * py).sqrt()
}

/// warnings: a triangle outline with round corners and "!"
fn warning_icon<D>(display: &mut D, p: Point, col: Colors) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    #[allow(unused_imports)]
    use micromath::F32Ext;
    // outer triangle; the rounded shape is the triangle drawn RR smaller,
    // grown again by RR (round corners, the sides stay where they were)
    let t = [(0.0f32, -18.0f32), (20.0, 15.0), (-20.0, 15.0)];
    const RR: f32 = 5.0;
    let w = ICON_W - 0.5;
    let len = |a: (f32, f32), b: (f32, f32)| {
        ((a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)).sqrt()
    };
    let (la, lb, lc) = (len(t[1], t[2]), len(t[0], t[2]), len(t[0], t[1]));
    let per = la + lb + lc;
    let ic = (
        (la * t[0].0 + lb * t[1].0 + lc * t[2].0) / per,
        (la * t[0].1 + lb * t[1].1 + lc * t[2].1) / per,
    );
    let area =
        ((t[1].0 - t[0].0) * (t[2].1 - t[0].1) - (t[2].0 - t[0].0) * (t[1].1 - t[0].1)).abs() / 2.0;
    let r_in = area / (per / 2.0);
    let k = (r_in - RR) / r_in;
    let inner = [
        (ic.0 + (t[0].0 - ic.0) * k, ic.1 + (t[0].1 - ic.1) * k),
        (ic.0 + (t[1].0 - ic.0) * k, ic.1 + (t[1].1 - ic.1) * k),
        (ic.0 + (t[2].0 - ic.0) * k, ic.1 + (t[2].1 - ic.1) * k),
    ];
    // distance to the small triangle (0 inside)
    let dist = |x: f32, y: f32| {
        if in_triangle(inner, x, y) {
            0.0
        } else {
            seg_dist(inner[0], inner[1], x, y)
                .min(seg_dist(inner[1], inner[2], x, y))
                .min(seg_dist(inner[2], inner[0], x, y))
        }
    };
    fill_shape(display, p, 21, col, |x, y| {
        let d = dist(x, y);
        let rim = d <= RR
            && !(d <= RR - w && (RR - w) > 0.0
                || in_triangle(inner, x, y) && {
                    // inside the small triangle: inside the hole when farther than
                    // w - RR from its sides
                    let e = seg_dist(inner[0], inner[1], x, y)
                        .min(seg_dist(inner[1], inner[2], x, y))
                        .min(seg_dist(inner[2], inner[0], x, y));
                    e > w - RR
                });
        let bar = x.abs() <= 1.5 && (-6.0..=3.0).contains(&y);
        let dot = x * x + (y - 7.0) * (y - 7.0) <= 3.3;
        rim || bar || dot
    })
}

/// Circling pictogram: a ring running clockwise, the arrow head between 12
/// and 3 o'clock pointing on clockwise; every pixel tested exactly (no
/// thick arc of embedded-graphics, whose ends are ragged)
pub fn circling_icon<D>(
    display: &mut D,
    p: Point,
    col: Colors,
    g: CirclingIcon,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    use core::f32::consts::PI;
    #[allow(unused_imports)]
    use micromath::F32Ext;
    let (cx, cy) = (p.x as f32, p.y as f32);
    let e = g.end_deg.to_radians();
    // arrow head: base on the ring at the end angle, tip along the tangent
    let (ex, ey) = (cx + g.r * e.sin(), cy - g.r * e.cos());
    let (tx, ty) = (e.cos(), e.sin()); // clockwise tangent
    let (nx, ny) = (e.sin(), -e.cos()); // outwards
    let tip = (ex + tx * g.head_len, ey + ty * g.head_len);
    let b1 = (ex + nx * g.head_half, ey + ny * g.head_half);
    let b2 = (ex - nx * g.head_half, ey - ny * g.head_half);
    let inside_tri = |x: f32, y: f32| {
        let s = |a: (f32, f32), b: (f32, f32)| (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0);
        let (d1, d2, d3) = (s(tip, b1), s(b1, b2), s(b2, tip));
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    };
    // the ring runs clockwise from start to end (through 6 and 12 o'clock)
    let span = (g.end_deg - g.start_deg).rem_euclid(360.0);
    let reach = (g.r + g.head_len + 2.0) as i32;
    for y in p.y - reach..=p.y + reach {
        for x in p.x - reach..=p.x + reach {
            let (fx, fy) = (x as f32, y as f32);
            let (dx, dy) = (fx - cx, fy - cy);
            let d = (dx * dx + dy * dy).sqrt();
            let mut on = false;
            if (d - g.r).abs() <= g.w / 2.0 {
                // clockwise from 12 o'clock
                let a = dx.atan2(-dy) * 180.0 / PI;
                let rel = (a - g.start_deg).rem_euclid(360.0);
                on = rel <= span;
            }
            if on || inside_tri(fx, fy) {
                display.draw_iter(core::iter::once(Pixel(Point::new(x, y), col)))?;
            }
        }
    }
    Ok(())
}

pub fn draw<D>(display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    if !vp::active(cm) {
        return Ok(());
    }
    let pal = &cm.palette().vario;
    let blue = pal.ring_select;
    let c = cm.device_const.sizes.display.center;
    // the whole black core, no line around it, the cap header like all menus
    let radius = crate::view::ring_menu::overlay_disc(display, cm)?;
    header(
        display,
        c,
        radius,
        crate::view::ring_menu::CAP_OVERLAY,
        "Volume",
    )?;

    let p = cm.control.volume_panel;
    for row in 0..ROWS {
        let y = c.y + (row as i32 - 1) * ROW_PITCH + 10;
        let sel = row == p.sel;
        let v = vp::value(cm, row);
        // frame of the cursor row
        if sel {
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(c.x - 156, y - 26), Size::new(312, 52)),
                Size::new(10, 10),
            )
            .into_styled(PrimitiveStyle::with_stroke(blue, 2))
            .draw(display)?;
        }
        let col = if sel { Colors::White } else { Colors::DarkGray };
        icon(display, row, Point::new(c.x - 126, y), col)?;
        // slider: track, filled part, knob
        let x0 = c.x + SLIDER_X0;
        let x1 = c.x + SLIDER_X1;
        let xv = x0 + (x1 - x0) * v.clamp(0, VOLUME_MAX) as i32 / VOLUME_MAX as i32;
        RoundedRectangle::with_equal_corners(
            Rectangle::new(Point::new(x0, y - 4), Size::new((x1 - x0) as u32, 8)),
            Size::new(4, 4),
        )
        .into_styled(PrimitiveStyle::with_fill(Colors::Grey64))
        .draw(display)?;
        let fill = if sel { blue } else { Colors::Gray };
        if xv > x0 {
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(x0, y - 4), Size::new((xv - x0) as u32, 8)),
                Size::new(4, 4),
            )
            .into_styled(PrimitiveStyle::with_fill(fill))
            .draw(display)?;
        }
        Circle::with_center(Point::new(xv, y), 18)
            .into_styled(PrimitiveStyle::with_fill(if sel {
                Colors::White
            } else {
                Colors::Gray
            }))
            .draw(display)?;
        // the number; changing: black on the blue field like in the menus
        let s = crate::tformat!(6, "{}", v).unwrap_or_default();
        let editing = sel && p.edit;
        if editing {
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(c.x + 96, y - 25), Size::new(59, 50)),
                Size::new(10, 10),
            )
            .into_styled(PrimitiveStyle::with_fill(blue))
            .draw(display)?;
        }
        let vcol = if editing {
            pal.background
        } else if sel {
            blue
        } else {
            Colors::DarkGray
        };
        render_value(
            display,
            &VALUE_21,
            s.as_str(),
            Point::new(c.x + 146, y),
            VerticalPosition::Center,
            HorizontalAlignment::Right,
            vcol,
        )?;
    }
    // what the knob does
    let hint = if p.edit { "push: done" } else { "push: change" };
    crate::view::fonts_p1::text_for(16).f.render_aligned(
        hint,
        c + Point::new(0, 2 * ROW_PITCH + 6),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        u8g2_fonts::types::FontColor::Transparent(Colors::DimGray),
        display,
    )?;
    Ok(())
}

#[cfg(test)]
mod icon_tests {
    use super::*;
    use crate::view::test_fixture::{dump_ppm, TestDisplay};

    /// `RENDER_DIR=... cargo test --lib circling_icon_variants -- --ignored`
    #[test]
    #[ignore]
    fn circling_icon_variants() {
        let dir = match std::env::var("RENDER_DIR") {
            Ok(d) => d,
            Err(_) => return,
        };
        let vs = [
            (
                "a",
                CirclingIcon {
                    r: 10.5,
                    w: 3.5,
                    start_deg: 135.0,
                    end_deg: 60.0,
                    head_len: 9.0,
                    head_half: 6.5,
                },
            ),
            (
                "b",
                CirclingIcon {
                    r: 11.0,
                    w: 3.0,
                    start_deg: 150.0,
                    end_deg: 75.0,
                    head_len: 9.0,
                    head_half: 6.0,
                },
            ),
            (
                "c",
                CirclingIcon {
                    r: 10.5,
                    w: 4.0,
                    start_deg: 120.0,
                    end_deg: 45.0,
                    head_len: 10.0,
                    head_half: 7.5,
                },
            ),
            (
                "d",
                CirclingIcon {
                    r: 11.5,
                    w: 3.0,
                    start_deg: 160.0,
                    end_deg: 90.0,
                    head_len: 8.0,
                    head_half: 6.0,
                },
            ),
        ];
        for (tag, g) in vs {
            let mut d = TestDisplay::new();
            circling_icon(&mut d, Point::new(40, 40), Colors::White, g).unwrap();
            dump_ppm(&d, &std::format!("{dir}/icon_{tag}.ppm"));
        }
    }
}
