//! Simple menu icons for the Stefly (ring) design, drawn with
//! embedded-graphics primitives so no bitmap flash space is needed.

use crate::{utils::Colors, CoreError};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle},
};
#[allow(unused_imports)]
use micromath::F32Ext;

#[derive(Clone, Copy, PartialEq)]
pub enum MenuIcon {
    Vario,
    SpeedToFly,
    Sound,
    Display,
    Units,
    Aircraft,
    Sensorbox,
    Flarm,
    System,
    Setup,
    None,
}

/// map a menu (item) name to its icon
pub fn icon_for(name: &str) -> MenuIcon {
    match name {
        "Vario" => MenuIcon::Vario,
        "Speed to Fly" => MenuIcon::SpeedToFly,
        "Sound" => MenuIcon::Sound,
        "Display" => MenuIcon::Display,
        "Units" => MenuIcon::Units,
        "Aircraft" => MenuIcon::Aircraft,
        "Sensorbox" => MenuIcon::Sensorbox,
        "FLARM" => MenuIcon::Flarm,
        "System" => MenuIcon::System,
        "Settings" | "Setup" => MenuIcon::Setup,
        _ => MenuIcon::None,
    }
}

/// glider top view, normalised to a span of 1 (from the FLARM symbols in
/// flarm_pages.rs, slightly heavier wing so it reads at 22 px)
const GLIDER_WING: [(f32, f32); 8] = [
    (-0.50, -0.07),
    (-0.08, -0.16),
    (0.08, -0.16),
    (0.50, -0.07),
    (0.50, 0.03),
    (0.08, 0.0),
    (-0.08, 0.0),
    (-0.50, 0.03),
];
const GLIDER_FUS: [(f32, f32); 7] = [
    (0.0, -0.36),
    (0.065, -0.27),
    (0.06, 0.05),
    (0.035, 0.42),
    (-0.035, 0.42),
    (-0.06, 0.05),
    (-0.065, -0.27),
];
const GLIDER_TAIL: [(f32, f32); 4] = [(-0.19, 0.33), (0.19, 0.33), (0.19, 0.42), (-0.19, 0.42)];

/// filled polygon (scan lines), points scaled by `k` around `c`
fn fill_polygon<D>(
    display: &mut D,
    pts: &[(f32, f32)],
    c: Point,
    k: f32,
    color: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    let n = pts.len();
    let p = |i: usize| (c.x as f32 + pts[i].0 * k, c.y as f32 + pts[i].1 * k);
    let (mut ymin, mut ymax) = (f32::MAX, f32::MIN);
    for i in 0..n {
        ymin = ymin.min(p(i).1);
        ymax = ymax.max(p(i).1);
    }
    for y in (ymin.floor() as i32)..=(ymax.ceil() as i32) {
        let yc = y as f32 + 0.5;
        let mut xs: heapless::Vec<f32, 8> = heapless::Vec::new();
        for i in 0..n {
            let (x0, y0) = p(i);
            let (x1, y1) = p((i + 1) % n);
            if (y0 <= yc && y1 > yc) || (y1 <= yc && y0 > yc) {
                let _ = xs.push(x0 + (yc - y0) / (y1 - y0) * (x1 - x0));
            }
        }
        for i in 1..xs.len() {
            let mut j = i;
            while j > 0 && xs[j - 1] > xs[j] {
                xs.swap(j - 1, j);
                j -= 1;
            }
        }
        let mut i = 0;
        while i + 1 < xs.len() {
            let a = (xs[i] - 0.5).ceil() as i32;
            let b = (xs[i + 1] - 0.5).floor() as i32;
            if b >= a {
                display.fill_solid(
                    &Rectangle::new(Point::new(a, y), Size::new((b - a + 1) as u32, 1)),
                    color,
                )?;
            }
            i += 2;
        }
    }
    Ok(())
}

pub fn draw_icon<D>(
    display: &mut D,
    icon: MenuIcon,
    c: Point,
    s: i32,
    color: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    let w = ((s / 12).max(2)) as u32;
    let r = s / 2;
    let stroke = PrimitiveStyle::with_stroke(color, w);
    let fill = PrimitiveStyle::with_fill(color);
    match icon {
        MenuIcon::Vario => {
            Circle::with_center(c, s as u32)
                .into_styled(stroke)
                .draw(display)?;
            Line::new(c, Point::new(c.x + r * 5 / 8, c.y - r * 5 / 8))
                .into_styled(stroke)
                .draw(display)?;
            Circle::with_center(c, w * 2)
                .into_styled(fill)
                .draw(display)?;
        }
        MenuIcon::SpeedToFly => {
            Line::new(Point::new(c.x - r, c.y), Point::new(c.x + r / 4, c.y))
                .into_styled(stroke)
                .draw(display)?;
            Triangle::new(
                Point::new(c.x + r, c.y),
                Point::new(c.x + r / 5, c.y - r / 2),
                Point::new(c.x + r / 5, c.y + r / 2),
            )
            .into_styled(fill)
            .draw(display)?;
        }
        MenuIcon::Sound => {
            Rectangle::new(
                Point::new(c.x - r, c.y - r / 3),
                Size::new((r / 2) as u32, (2 * r / 3) as u32),
            )
            .into_styled(fill)
            .draw(display)?;
            Triangle::new(
                Point::new(c.x - r / 2, c.y),
                Point::new(c.x, c.y - r * 3 / 4),
                Point::new(c.x, c.y + r * 3 / 4),
            )
            .into_styled(fill)
            .draw(display)?;
            Line::new(
                Point::new(c.x + r / 3, c.y - r / 3),
                Point::new(c.x + r / 3, c.y + r / 3),
            )
            .into_styled(stroke)
            .draw(display)?;
            Line::new(
                Point::new(c.x + 2 * r / 3, c.y - r / 2),
                Point::new(c.x + 2 * r / 3, c.y + r / 2),
            )
            .into_styled(stroke)
            .draw(display)?;
        }
        MenuIcon::Display => {
            // the round display with its inner circle
            Circle::with_center(c, s as u32)
                .into_styled(stroke)
                .draw(display)?;
            Circle::with_center(c, (s / 3) as u32)
                .into_styled(stroke)
                .draw(display)?;
        }
        MenuIcon::Units => {
            // a ruler: a flat bar with scale marks along the upper edge
            let h = r * 5 / 8;
            Rectangle::new(
                Point::new(c.x - r, c.y - h / 2),
                Size::new((2 * r) as u32, h as u32),
            )
            .into_styled(stroke)
            .draw(display)?;
            for k in 1..=5 {
                let x = c.x - r + (2 * r * k) / 6;
                let len = if k % 2 == 1 { h / 2 } else { h / 4 };
                Line::new(Point::new(x, c.y - h / 2), Point::new(x, c.y - h / 2 + len))
                    .into_styled(stroke)
                    .draw(display)?;
            }
        }
        MenuIcon::Aircraft => {
            // glider from above, the shapes of the FLARM radar symbol
            // (flarm_pages, SteFly FLARM display prototype 0.5.3), nose up, span 1.3 x size
            let span = s as f32 * 1.3;
            for poly in [&GLIDER_WING[..], &GLIDER_FUS[..], &GLIDER_TAIL[..]] {
                fill_polygon(display, poly, c, span, color)?;
            }
        }
        MenuIcon::Sensorbox => {
            Rectangle::new(
                Point::new(c.x - r / 2, c.y - r / 2),
                Size::new(r as u32, r as u32),
            )
            .into_styled(stroke)
            .draw(display)?;
            for k in [-1, 0, 1] {
                let o = k * r / 3;
                Line::new(
                    Point::new(c.x - r, c.y + o),
                    Point::new(c.x - r / 2, c.y + o),
                )
                .into_styled(stroke)
                .draw(display)?;
                Line::new(
                    Point::new(c.x + r / 2, c.y + o),
                    Point::new(c.x + r, c.y + o),
                )
                .into_styled(stroke)
                .draw(display)?;
            }
        }
        MenuIcon::System | MenuIcon::Setup => {
            Circle::with_center(c, (2 * s / 3) as u32)
                .into_styled(stroke)
                .draw(display)?;
            for k in 0..8 {
                let a = (k as f32) * core::f32::consts::PI / 4.0;
                let (sin, cos) = (a.sin(), a.cos());
                let p1 = Point::new(
                    c.x + ((r as f32 * 0.62 * cos) as i32),
                    c.y + ((r as f32 * 0.62 * sin) as i32),
                );
                let p2 = Point::new(
                    c.x + ((r as f32 * cos) as i32),
                    c.y + ((r as f32 * sin) as i32),
                );
                Line::new(p1, p2).into_styled(stroke).draw(display)?;
            }
        }
        MenuIcon::Flarm => {
            // radar: two range rings, own aircraft in the middle, one target
            Circle::with_center(c, (2 * r) as u32)
                .into_styled(stroke)
                .draw(display)?;
            Circle::with_center(c, r as u32)
                .into_styled(stroke)
                .draw(display)?;
            Circle::with_center(c, (w * 2) as u32)
                .into_styled(fill)
                .draw(display)?;
            Circle::with_center(Point::new(c.x + r / 2, c.y - 2 * r / 3), (w * 3) as u32)
                .into_styled(fill)
                .draw(display)?;
        }
        MenuIcon::None => (),
    }
    Ok(())
}
