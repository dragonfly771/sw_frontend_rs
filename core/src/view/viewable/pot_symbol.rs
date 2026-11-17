//! Yes / no of the POT box as a pictogram next to its value
//!
//! The POT box compares its number with the bar of the pilot (MacCready value,
//! controller/pot_climb.rs). While the circle stands the result is shown as a pictogram:
//! yes = the circle is expected to bring at least the bar, no = clearly below it. Nothing in
//! between and in straight flight.
//!
//! The pictogram (Stefan, 10.11.2026): yes = a green disc with a check mark, no = a white disc
//! with a cross. The marks are cut out of the disc in the colour of the core (black) and have
//! the same stroke width. The edges are smoothed with the neighbouring colours of the palette
//! (green: ForestGreen, DarkGreen; white: the greys), from the exact distance of every pixel
//! centre to the edge - a disc of 30 to 44 px looks round only that way.
//!
//! Centred boxes (top, bottom, big values, two values) show the pictogram left of the number,
//! in the free space - the number never moves. The side boxes of the ring page have no room
//! there: a small pictogram in front of the caption instead.

use crate::{
    model::PotVerdict,
    utils::{math, Colors},
    CoreError,
};
use embedded_graphics::{draw_target::DrawTarget, geometry::Point, Pixel};

pub(crate) const YES_COLOR: Colors = Colors::LimeGreen;
pub(crate) const NO_COLOR: Colors = Colors::White;
/// colour of the marks: the core behind the disc
const MARK_COLOR: Colors = Colors::Black;

/// edge colours between the fill and black: (share of the fill, colour)
const YES_RAMP: [(f32, Colors); 3] = [
    (1.0, YES_COLOR),
    (0.68, Colors::ForestGreen),
    (0.49, Colors::DarkGreen),
];
const NO_RAMP: [(f32, Colors); 6] = [
    (1.0, NO_COLOR),
    (0.86, Colors::Gainsboro),
    (0.75, Colors::Silver),
    (0.66, Colors::DarkGray),
    (0.50, Colors::Gray),
    (0.41, Colors::DimGray),
];
/// below this share of the fill a pixel is black
const MIN_SHARE: f32 = 0.22;

/// stroke width of both marks, in disc radii
const STROKE: f32 = 0.25;
/// thinnest stroke in px (small pictograms)
const STROKE_MIN_PX: f32 = 2.0;
/// check mark: left end, corner, right end (x to the right, y down), in disc radii
const CHECK: [(f32, f32); 3] = [(-0.55, 0.0), (-0.16, 0.38), (0.56, -0.36)];
/// cross: half the extent of its arms in x and y, in disc radii
const CROSS: f32 = 0.38;

/// fill colour of the disc of a verdict
#[cfg(test)]
pub(crate) fn color(verdict: PotVerdict) -> Colors {
    if verdict == PotVerdict::Yes {
        YES_COLOR
    } else {
        NO_COLOR
    }
}

fn ramp(verdict: PotVerdict) -> &'static [(f32, Colors)] {
    if verdict == PotVerdict::Yes {
        &YES_RAMP
    } else {
        &NO_RAMP
    }
}

/// radius in px of a pictogram `size` px across (its edge pixels stay inside)
fn radius(size: i32) -> f32 {
    size as f32 / 2.0 - 0.5
}

/// stroke width in px of the marks in a pictogram `size` px across - the same for both
pub(crate) fn stroke_width(size: i32) -> f32 {
    (STROKE * radius(size)).max(STROKE_MIN_PX)
}

/// the two strokes of the mark of a verdict, in px from the centre
pub(crate) fn strokes(verdict: PotVerdict, size: i32) -> [((f32, f32), (f32, f32)); 2] {
    let r = radius(size);
    let s = |(x, y): (f32, f32)| (x * r, y * r);
    if verdict == PotVerdict::Yes {
        [(s(CHECK[0]), s(CHECK[1])), (s(CHECK[1]), s(CHECK[2]))]
    } else {
        let c = CROSS * r;
        [((-c, -c), (c, c)), ((-c, c), (c, -c))]
    }
}

/// squared distance of (x, y) to the segment a - b
fn seg_dist2(a: (f32, f32), b: (f32, f32), x: f32, y: f32) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((x - a.0) * dx + (y - a.1) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (px, py) = (a.0 + t * dx - x, a.1 + t * dy - y);
    px * px + py * py
}

/// share (0..1) of an area in a pixel whose centre lies `d` px inside its edge (negative:
/// outside), for an edge of `limit` px (radius, half stroke) and the squared distance `d2`
fn share(d2: f32, limit: f32) -> f32 {
    if d2 <= (limit - 0.5).max(0.0) * (limit - 0.5).max(0.0) {
        1.0
    } else if d2 >= (limit + 0.5) * (limit + 0.5) {
        0.0
    } else {
        (0.5 + limit - math::sqrt(d2)).clamp(0.0, 1.0)
    }
}

/// For the pixel whose centre is (x, y) px from the centre of the pictogram: (share of the
/// disc, share of the fill colour = disc without the mark)
pub(crate) fn coverage(verdict: PotVerdict, size: i32, x: f32, y: f32) -> (f32, f32) {
    let disc = share(x * x + y * y, radius(size));
    if disc <= 0.0 {
        return (0.0, 0.0);
    }
    let [s0, s1] = strokes(verdict, size);
    let d2 = seg_dist2(s0.0, s0.1, x, y).min(seg_dist2(s1.0, s1.1, x, y));
    let mark = share(d2, stroke_width(size) / 2.0);
    (disc, disc * (1.0 - mark))
}

/// palette colour for a share of the fill colour (None: leave the pixel to the core)
fn pixel_color(verdict: PotVerdict, disc: f32, fill: f32) -> Option<Colors> {
    if fill < MIN_SHARE {
        // inside the disc: the mark; at its edge: the core shows
        return if disc >= 0.5 { Some(MARK_COLOR) } else { None };
    }
    let mut best = ramp(verdict)[0];
    for level in ramp(verdict) {
        if (level.0 - fill).abs() < (best.0 - fill).abs() {
            best = *level;
        }
    }
    Some(best.1)
}

/// Draw the pictogram of a verdict, `size` px across, centred on the pixel `p`
pub(crate) fn draw<D>(
    display: &mut D,
    verdict: PotVerdict,
    p: Point,
    size: i32,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    if verdict == PotVerdict::None {
        return Ok(());
    }
    let reach = size / 2 + 1;
    for y in -reach..=reach {
        for x in -reach..=reach {
            let (disc, fill) = coverage(verdict, size, x as f32, y as f32);
            if disc <= 0.0 {
                continue;
            }
            if let Some(col) = pixel_color(verdict, disc, fill) {
                display.draw_iter(core::iter::once(Pixel(p + Point::new(x, y), col)))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::test_fixture::{TestDisplay, DISPLAY_WIDTH};

    /// share of the fill colour of a drawn pixel (0 = black or not drawn)
    fn level(c: Option<Colors>) -> f32 {
        match c {
            None | Some(Colors::Black) => 0.0,
            Some(c) => YES_RAMP
                .iter()
                .chain(NO_RAMP.iter())
                .find(|l| l.1 == c)
                .map(|l| l.0)
                .unwrap_or_else(|| panic!("colour {c:?} is not part of the pictogram")),
        }
    }

    /// The stroke width as it ends up on the display: across a stroke the missing fill is
    /// summed (nearest pixel, 0.02 px steps) at 30, 50 and 70 % of its length, away from the
    /// ends, the corner of the check and the crossing.
    fn drawn_stroke_width(verdict: PotVerdict, size: i32) -> f32 {
        let mut d = TestDisplay::new();
        let c = Point::new(240, 240);
        draw(&mut d, verdict, c, size).unwrap();
        let px = |x: f32, y: f32| {
            let (ix, iy) = (c.x + x.round() as i32, c.y + y.round() as i32);
            level(d.buf[(iy * DISPLAY_WIDTH as i32 + ix) as usize])
        };
        let mut sum = 0.0;
        let mut n = 0;
        for (a, b) in strokes(verdict, size) {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let l = math::sqrt(dx * dx + dy * dy);
            let (nx, ny) = (-dy / l, dx / l);
            for t in [0.3, 0.5, 0.7] {
                if verdict == PotVerdict::No && t == 0.5 {
                    continue; // the crossing
                }
                let (mx, my) = (a.0 + t * dx, a.1 + t * dy);
                // across the stroke and 1.5 px beyond its edges
                let reach = stroke_width(size) / 2.0 + 1.5;
                let mut w = 0.0;
                let mut s = -reach;
                while s <= reach {
                    w += (1.0 - px(mx + s * nx, my + s * ny)) * 0.02;
                    s += 0.02;
                }
                sum += w;
                n += 1;
            }
        }
        sum / n as f32
    }

    /// Stefan: the check mark and the cross with a similar stroke width
    #[test]
    fn check_and_cross_have_the_same_stroke_width_on_the_display() {
        for size in [16, 20, 24, 28, 32, 35, 36, 40, 44, 48] {
            let w = stroke_width(size);
            let yes = drawn_stroke_width(PotVerdict::Yes, size);
            let no = drawn_stroke_width(PotVerdict::No, size);
            assert!(
                (yes - no).abs() < 0.35,
                "{size} px: check {yes:.2} px, cross {no:.2} px"
            );
            assert!(
                (yes - w).abs() < 0.6 && (no - w).abs() < 0.6,
                "{size} px: {yes:.2} / {no:.2}, drawn {w:.2}"
            );
        }
    }

    /// a disc: every pixel of the pictogram lies inside the size, the fill dominates, the
    /// marks are cut out
    #[test]
    fn a_disc_with_a_mark_inside_its_size() {
        for verdict in [PotVerdict::Yes, PotVerdict::No] {
            for size in [16, 29, 35, 44] {
                let mut d = TestDisplay::new();
                let c = Point::new(100, 100);
                draw(&mut d, verdict, c, size).unwrap();
                let (mut fill, mut mark, mut edge) = (0, 0, 0);
                for (i, px) in d.buf.iter().enumerate() {
                    let Some(col) = px else { continue };
                    let (x, y) = (
                        i as i32 % DISPLAY_WIDTH as i32,
                        i as i32 / DISPLAY_WIDTH as i32,
                    );
                    let (dx, dy) = ((x - c.x) as f32, (y - c.y) as f32);
                    assert!(
                        dx * dx + dy * dy <= (size as f32 / 2.0 + 0.1).powi(2),
                        "{size}: pixel at {dx},{dy}"
                    );
                    match *col {
                        Colors::Black => mark += 1,
                        c if c == color(verdict) => fill += 1,
                        _ => edge += 1,
                    }
                }
                let area = core::f32::consts::PI * (size as f32 / 2.0).powi(2);
                assert!(
                    fill as f32 > 0.45 * area,
                    "{verdict:?} {size}: fill {fill} of {area:.0}"
                );
                assert!(mark as f32 > 0.08 * area, "{verdict:?} {size}: mark {mark}");
                assert!(edge > 0, "{verdict:?} {size}: no smoothed edge");
            }
        }
        // nothing for no verdict
        let mut d = TestDisplay::new();
        draw(&mut d, PotVerdict::None, Point::new(100, 100), 40).unwrap();
        assert!(d.buf.iter().all(|p| p.is_none()));
    }
}
