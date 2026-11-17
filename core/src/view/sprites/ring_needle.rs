use super::{DrawColored, Rotate};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Triangle},
};
#[allow(unused_imports)]
use micromath::F32Ext;

use crate::utils::{Colors, CoreError};

/// Long thin needle of the ring design
///
/// Runs from just outside the center circle to the outer end of the scale
/// ticks. Geometry is defined in pixels of the 480x480 reference design and
/// scaled with `scl` (= min(display width, height) / 480).
pub struct RingNeedle {
    zero_pos: f32,
    rotation: f32,
    scl: f32,
    center: Point,
    r_in: f32,
    r_tip: f32,
    outline: Option<Colors>,
}

// reference geometry (480x480 design)
const R_IN: f32 = 82.0; // inner end (at the center circle)
/// tip: into the scale numbers (Stefly design P1: r 222; B3 had 212, before 200)
const R_TIP: f32 = 222.0;
/// tip when the middle of the page is taken by something large (horizon,
/// big boxes, FLARM radar and warnings): almost to the display edge, so the
/// short visible part of the needle still reads well (Stefan, 27.10.2026)
pub const R_TIP_LONG: f32 = 236.0;
const TAPER: f32 = 22.0; // length of the pointed tip
const HALF_W: f32 = 5.5; // half width of the needle body

impl RingNeedle {
    pub const fn new(scl: f32, center: Point) -> Self {
        Self {
            zero_pos: 0.0,
            rotation: 0.0,
            scl,
            center,
            r_in: R_IN,
            r_tip: R_TIP,
            outline: None,
        }
    }

    /// radius of the tip, in pixels of the 480x480 reference design
    pub fn tip(&mut self, r: f32) -> &mut Self {
        self.r_tip = r;
        self
    }

    /// Draw a two pixel border around the needle first. Needed where the
    /// needle runs over the light outer band - orange on white has barely
    /// any contrast. On a dark background it is invisible and harmless.
    #[allow(dead_code)]
    pub fn outline(&mut self, color: Option<Colors>) -> &mut Self {
        self.outline = color;
        self
    }

    /// Move the inner end outwards, so that the middle of the page stays
    /// free. `r` is the radius the ROUNDED CAP shall reach, in pixels of
    /// the 480x480 reference design.
    pub fn inner_end(&mut self, r: f32) -> &mut Self {
        self.r_in = r + HALF_W;
        self
    }
}

impl DrawColored for RingNeedle {
    fn draw_colored<D>(&self, color: Colors, display: &mut D) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError>,
    {
        let a = self.zero_pos + self.rotation;
        let (sin, cos) = (a.sin(), a.cos());
        let s = self.scl;

        // radial unit vector (alpha = 0 points to 12 o'clock)
        let pt = |r: f32, t: f32| {
            Point::new(
                self.center.x + (sin * r * s + cos * t * s) as i32,
                self.center.y + (-cos * r * s + sin * t * s) as i32,
            )
        };

        // body once in the border colour, two pixels larger, then the needle
        for (opt_col, grow) in [(self.outline, 2.0_f32), (Some(color), 0.0)] {
            let col = match opt_col {
                Some(c) => c,
                None => continue,
            };
            let hw = HALF_W + grow;
            let tip = pt(self.r_tip + grow, 0.0);
            let s1 = pt(self.r_tip - TAPER, hw);
            let s2 = pt(self.r_tip - TAPER, -hw);
            let b1 = pt(self.r_in - grow, hw);
            let b2 = pt(self.r_in - grow, -hw);

            let style = PrimitiveStyle::with_fill(col);
            Triangle::new(tip, s1, s2)
                .into_styled(style)
                .draw(display)?;
            Triangle::new(s1, b1, b2).into_styled(style).draw(display)?;
            Triangle::new(s1, b2, s2).into_styled(style).draw(display)?;

            // rounded cap at the inner end
            let base = pt(self.r_in - grow, 0.0);
            let dia = (2.0 * hw * s) as u32;
            Circle::with_center(base, dia)
                .into_styled(style)
                .draw(display)?;
        }

        Ok(())
    }
}

impl Rotate for RingNeedle {
    fn rotate(&mut self, rotation: f32) -> &mut Self {
        self.rotation = rotation;
        self
    }
    fn zero_pos(&mut self, zero_pos: f32) -> &mut Self {
        self.zero_pos = zero_pos;
        self
    }
}

/// Small triangle marker of the ring design (average climb rate)
///
/// The long side sits on the scale ring, the tip points outwards.
pub struct RingMarker {
    zero_pos: f32,
    rotation: f32,
    scl: f32,
    center: Point,
    outline: Option<Colors>,
}

// The long side lies beyond the round display edge (r 240), so the marker
// always reaches the edge completely - a straight side at r 238 left a small
// free segment in its middle. Same flanks as before (half width 14 at r 238).
const M_BASE: f32 = 252.0; // long side (outer, outside the display)
const M_TIP: f32 = 214.0; // tip (inner, pointing at the scale)
const M_HALF_W: f32 = 14.0 * (M_BASE - M_TIP) / (238.0 - M_TIP); // half width of the long side

impl RingMarker {
    pub const fn new(scl: f32, center: Point) -> Self {
        Self {
            zero_pos: 0.0,
            rotation: 0.0,
            scl,
            center,
            outline: None,
        }
    }

    /// see [`RingNeedle::outline`]
    #[allow(dead_code)]
    pub fn outline(&mut self, color: Option<Colors>) -> &mut Self {
        self.outline = color;
        self
    }
}

impl DrawColored for RingMarker {
    fn draw_colored<D>(&self, color: Colors, display: &mut D) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError>,
    {
        let a = self.zero_pos + self.rotation;
        let (sin, cos) = (a.sin(), a.cos());
        let s = self.scl;
        let pt = |r: f32, t: f32| {
            Point::new(
                self.center.x + (sin * r * s + cos * t * s) as i32,
                self.center.y + (-cos * r * s + sin * t * s) as i32,
            )
        };

        if let Some(oc) = self.outline {
            Triangle::new(
                pt(M_TIP - 3.0, 0.0),
                pt(M_BASE, M_HALF_W + 3.0),
                pt(M_BASE, -M_HALF_W - 3.0),
            )
            .into_styled(PrimitiveStyle::with_fill(oc))
            .draw(display)?;
        }
        let tip = pt(M_TIP, 0.0);
        let b1 = pt(M_BASE, M_HALF_W);
        let b2 = pt(M_BASE, -M_HALF_W);
        Triangle::new(tip, b1, b2)
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(display)?;
        Ok(())
    }
}

impl Rotate for RingMarker {
    fn rotate(&mut self, rotation: f32) -> &mut Self {
        self.rotation = rotation;
        self
    }
    fn zero_pos(&mut self, zero_pos: f32) -> &mut Self {
        self.zero_pos = zero_pos;
        self
    }
}

/// Wind / energy arrow of the ring design
///
/// A long slim triangle with a rounded back, as in the approved template.
/// `len` is the distance from the rotation center to the tip; the rounded
/// back reaches about 0.76 * len behind the center.
pub struct RingArrow {
    zero_pos: f32,
    rotation: f32,
    len: i32,
    center: Point,
}

impl RingArrow {
    pub const fn new(len: i32, center: Point) -> Self {
        Self {
            zero_pos: 0.0,
            rotation: 0.0,
            len,
            center,
        }
    }
}

impl DrawColored for RingArrow {
    fn draw_colored<D>(&self, color: Colors, display: &mut D) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError>,
    {
        let a = self.zero_pos + self.rotation;
        let (sin, cos) = (a.sin(), a.cos());
        let l = self.len as f32;

        // longitudinal r (towards the tip), lateral t
        let pt = |r: f32, t: f32| {
            Point::new(
                self.center.x + (sin * r + cos * t) as i32,
                self.center.y + (-cos * r + sin * t) as i32,
            )
        };

        let tip = pt(l, 0.0);
        // rounded back, approximated with a short arc polygon
        let back = [
            pt(-0.69 * l, 0.44 * l),
            pt(-0.75 * l, 0.22 * l),
            pt(-0.76 * l, 0.0),
            pt(-0.75 * l, -0.22 * l),
            pt(-0.69 * l, -0.44 * l),
        ];

        let style = PrimitiveStyle::with_fill(color);
        for i in 0..back.len() - 1 {
            Triangle::new(tip, back[i], back[i + 1])
                .into_styled(style)
                .draw(display)?;
        }
        Ok(())
    }
}

impl Rotate for RingArrow {
    fn rotate(&mut self, rotation: f32) -> &mut Self {
        self.rotation = rotation;
        self
    }
    fn zero_pos(&mut self, zero_pos: f32) -> &mut Self {
        self.zero_pos = zero_pos;
        self
    }
}
