//! Small raster helpers for the round display: scanline fills of
//! polygons, discs and rings clipped to a circle and a band of rows,
//! thick lines, text on a black box. Integer spans through
//! `draw_line_unchecked`, so they are fast on the target.
//!
//! Used by the FLARM screens and the horizon K1.

use crate::{utils::Colors, CoreError, CoreModel, DrawImage};
use embedded_graphics::prelude::*;
#[allow(unused_imports)]
use micromath::F32Ext;
use u8g2_fonts::{
    types::{FontColor, HorizontalAlignment, VerticalPosition},
    FontRenderer,
};

/// drawing area: centre, scale factor against the 480 px reference, clip
#[derive(Clone, Copy)]
pub(crate) struct Geo {
    pub(crate) cx: f32,
    pub(crate) cy: f32,
    pub(crate) k: f32,
    /// clip circle radius (pixels)
    pub(crate) r_clip: i32,
    /// clip band (absolute pixel rows, inclusive)
    pub(crate) y0: i32,
    pub(crate) y1: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

impl Geo {
    pub(crate) fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (self.cx + x * self.k, self.cy + y * self.k)
    }
    pub(crate) fn pt(&self, x: f32, y: f32) -> Point {
        let (a, b) = self.p(x, y);
        Point::new(round(a), round(b))
    }
    pub(crate) fn with_band(&self, y0: f32, y1: f32) -> Geo {
        let mut g = *self;
        g.y0 = (self.cy + y0 * self.k).ceil() as i32;
        g.y1 = (self.cy + y1 * self.k).floor() as i32;
        g
    }
    /// x range of the clip circle in row y (absolute), None outside
    pub(crate) fn row(&self, y: i32) -> Option<(i32, i32)> {
        if y < self.y0 || y > self.y1 || y < 0 || y >= self.height {
            return None;
        }
        let dy = y as f32 + 0.5 - self.cy;
        let r = self.r_clip as f32;
        let h2 = r * r - dy * dy;
        if h2 <= 0.0 {
            return None;
        }
        let h = isqrt_f(h2);
        let x0 = ((self.cx - h).ceil() as i32).max(0);
        let x1 = ((self.cx + h).floor() as i32 - 1).min(self.width - 1);
        if x1 < x0 {
            None
        } else {
            Some((x0, x1))
        }
    }
}

pub(crate) fn round(v: f32) -> i32 {
    if v >= 0.0 {
        (v + 0.5) as i32
    } else {
        (v - 0.5) as i32
    }
}

/// exact square root for positioning against the round edge (see ring_menu::isqrt)
pub(crate) fn isqrt_f(v: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    let mut x = v.sqrt();
    // two Newton steps make the fast micromath estimate exact enough
    x = 0.5 * (x + v / x);
    x = 0.5 * (x + v / x);
    x
}

pub(crate) fn span<D>(
    display: &mut D,
    g: &Geo,
    y: i32,
    x0: i32,
    x1: i32,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    if let Some((c0, c1)) = g.row(y) {
        let a = x0.max(c0);
        let b = x1.min(c1);
        if b >= a {
            // Safety: row() clamps x and y to the display
            unsafe {
                display.draw_line_unchecked(
                    y as usize * g.width as usize + a as usize,
                    (b - a + 1) as usize,
                    col,
                );
            }
        }
    }
    Ok(())
}

/// filled polygon (even-odd), points in display pixels (absolute)
pub(crate) fn fill_poly<D>(
    display: &mut D,
    g: &Geo,
    pts: &[(f32, f32)],
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let n = pts.len();
    if n < 3 {
        return Ok(());
    }
    let mut ymin = f32::MAX;
    let mut ymax = f32::MIN;
    for p in pts {
        ymin = ymin.min(p.1);
        ymax = ymax.max(p.1);
    }
    let ys = (ymin.floor() as i32).max(g.y0);
    let ye = (ymax.ceil() as i32).min(g.y1);
    for y in ys..=ye {
        let yc = y as f32 + 0.5;
        let mut xs: heapless::Vec<f32, 32> = heapless::Vec::new();
        for i in 0..n {
            let (x0, y0) = pts[i];
            let (x1, y1) = pts[(i + 1) % n];
            if (y0 <= yc && y1 > yc) || (y1 <= yc && y0 > yc) {
                let t = (yc - y0) / (y1 - y0);
                let _ = xs.push(x0 + t * (x1 - x0));
            }
        }
        // sort (tiny lists)
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
                span(display, g, y, a, b, col)?;
            }
            i += 2;
        }
    }
    Ok(())
}

/// filled circle (absolute centre)
pub(crate) fn disc<D>(
    display: &mut D,
    g: &Geo,
    cx: f32,
    cy: f32,
    r: f32,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let ys = ((cy - r).floor() as i32).max(g.y0);
    let ye = ((cy + r).ceil() as i32).min(g.y1);
    for y in ys..=ye {
        let dy = y as f32 + 0.5 - cy;
        let h2 = r * r - dy * dy;
        if h2 <= 0.0 {
            continue;
        }
        let h = isqrt_f(h2);
        let a = (cx - h - 0.5).ceil() as i32;
        let b = (cx + h - 0.5).floor() as i32;
        if b >= a {
            span(display, g, y, a, b, col)?;
        }
    }
    Ok(())
}

/// ring (annulus) between r_in and r_out (absolute centre)
pub(crate) fn ring<D>(
    display: &mut D,
    g: &Geo,
    cx: f32,
    cy: f32,
    r_in: f32,
    r_out: f32,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let ys = ((cy - r_out).floor() as i32).max(g.y0);
    let ye = ((cy + r_out).ceil() as i32).min(g.y1);
    for y in ys..=ye {
        let dy = y as f32 + 0.5 - cy;
        let ho2 = r_out * r_out - dy * dy;
        if ho2 <= 0.0 {
            continue;
        }
        let ho = isqrt_f(ho2);
        let hi2 = r_in * r_in - dy * dy;
        let ao = (cx - ho - 0.5).ceil() as i32;
        let bo = (cx + ho - 0.5).floor() as i32;
        if hi2 <= 0.0 {
            span(display, g, y, ao, bo, col)?;
        } else {
            let hi = isqrt_f(hi2);
            let ai = (cx - hi - 0.5).ceil() as i32;
            let bi = (cx + hi - 0.5).floor() as i32;
            span(display, g, y, ao, ai - 1, col)?;
            span(display, g, y, bi + 1, bo, col)?;
        }
    }
    Ok(())
}

/// thick line as a quad, optional round ends
pub(crate) fn line<D>(
    display: &mut D,
    g: &Geo,
    a: (f32, f32),
    b: (f32, f32),
    w: f32,
    col: Colors,
    round_ends: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len = isqrt_f(dx * dx + dy * dy);
    if len < 0.01 {
        return disc(display, g, a.0, a.1, w / 2.0, col);
    }
    let nx = -dy / len * w / 2.0;
    let ny = dx / len * w / 2.0;
    fill_poly(
        display,
        g,
        &[
            (a.0 + nx, a.1 + ny),
            (b.0 + nx, b.1 + ny),
            (b.0 - nx, b.1 - ny),
            (a.0 - nx, a.1 - ny),
        ],
        col,
    )?;
    if round_ends {
        disc(display, g, a.0, a.1, w / 2.0, col)?;
        disc(display, g, b.0, b.1, w / 2.0, col)?;
    }
    Ok(())
}

pub(crate) fn text<D>(
    display: &mut D,
    font: &FontRenderer,
    s: &str,
    p: Point,
    v: VerticalPosition,
    h: HorizontalAlignment,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    font.render_aligned(s, p, v, h, FontColor::Transparent(col), display)?;
    Ok(())
}

pub(crate) fn text_w(font: &FontRenderer, s: &str) -> i32 {
    font.get_rendered_dimensions(s, Point::zero(), VerticalPosition::Baseline)
        .map(|d| d.advance.x)
        .unwrap_or(s.len() as i32 * 12)
}

/// text on a black box (labels on the radar)
pub(crate) fn text_boxed<D>(
    display: &mut D,
    g: &Geo,
    font: &FontRenderer,
    s: &str,
    p: Point,
    h: HorizontalAlignment,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    boxed(display, g, font, s, p, h, col, false)
}

/// a value (inb, P1 weight) on a black box
pub(crate) fn value_boxed<D>(
    display: &mut D,
    g: &Geo,
    font: &FontRenderer,
    s: &str,
    p: Point,
    h: HorizontalAlignment,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    boxed(display, g, font, s, p, h, col, true)
}

/// a value in the value font (B612 Bold)
pub(crate) fn value_text<D>(
    display: &mut D,
    font: &FontRenderer,
    s: &str,
    p: Point,
    v: VerticalPosition,
    h: HorizontalAlignment,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    crate::view::fonts_p1::render_value(display, font, s, p, v, h, col)
}

#[allow(clippy::too_many_arguments)]
fn boxed<D>(
    display: &mut D,
    g: &Geo,
    font: &FontRenderer,
    s: &str,
    p: Point,
    h: HorizontalAlignment,
    col: Colors,
    bold: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let w = text_w(font, s);
    let x0 = match h {
        HorizontalAlignment::Left => p.x,
        HorizontalAlignment::Center => p.x - w / 2,
        HorizontalAlignment::Right => p.x - w,
    };
    let hh = font
        .get_rendered_dimensions(s, Point::zero(), VerticalPosition::Center)
        .ok()
        .and_then(|d| d.bounding_box)
        .map(|b| b.size.height as i32)
        .unwrap_or(16);
    for y in p.y - hh / 2 - 2..=p.y + hh / 2 + 1 {
        span(display, g, y, x0 - 3, x0 + w + 2, Colors::Black)?;
    }
    if bold {
        value_text(display, font, s, p, VerticalPosition::Center, h, col)
    } else {
        text(display, font, s, p, VerticalPosition::Center, h, col)
    }
}

/// drawing area of the whole display around its centre, scale k against
/// the 480 px reference, clip circle r_clip
pub(crate) fn geo(cm: &CoreModel, k: f32, r_clip: i32) -> Geo {
    let c = cm.device_const.sizes.display.center;
    let width = cm.device_const.sizes.display.width as i32;
    let height = cm.device_const.sizes.display.height as i32;
    Geo {
        cx: c.x as f32,
        cy: c.y as f32,
        k,
        r_clip,
        y0: 0,
        y1: height - 1,
        width,
        height,
    }
}
