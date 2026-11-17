//! Flap tape of the ring page (plan_ring_design_p1.md chapter 3)
//!
//! Right side, centred at 3 o'clock, between r 167 and the inner edge of
//! the scale line (r 200.6). 15 degrees per stage, +/-2.5 stages visible,
//! fast stages on top. The band is one solid grey up to its ends (no fade
//! any more), the inner edge solid white, and it always keeps its full
//! length: beyond the first / last stage it stays grey and empty.
//!
//! - Actual position (flap sensor): orange arrow fixed at 3 o'clock at the
//!   inner edge plus an orange line through the band (gap at the digits).
//!   The band moves under it.
//! - Recommendation: the stage is filled magenta, white digit.
//! - Without sensor: no arrow, the band is centred on the recommendation.
//!
//! The band is drawn with one span per pixel row and a colour per pixel
//! from a short fade ramp of dedicated CLUT colours - the 8 bit display has
//! no alpha.

#[allow(unused_imports)]
use crate::utils::FClamp;
use crate::{
    model::flaps::FlapTable, utils::Colors, view::fonts_p1::text_for, view::ring_menu::isqrt,
    CoreError, CoreModel, DrawImage,
};
use embedded_graphics::{
    prelude::*,
    primitives::{Line, PrimitiveStyle, Triangle},
};
#[allow(unused_imports)]
use micromath::F32Ext;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

/// inner edge of the tape
pub const TAPE_R_IN: i32 = 167;
/// outer edge: inner edge of the ink of the scale line (r 200.75)
pub const TAPE_R_OUT: f32 = 200.6;
/// radius of the dark core (the tape crosses it)
const CORE_R: i32 = 192;
/// radius of the stage names
const R_TXT: f32 = 183.8;
/// white inner edge (2 px)
const EDGE_R_IN: i32 = 165;
/// degrees per stage
pub const PITCH_DEG: f32 = 15.0;
/// half of the visible span (2.5 stages)
pub const HALF_DEG: f32 = 37.5;
/// gap between the recommended field and its neighbours (degrees)
const REC_GAP_DEG: f32 = 0.6;

/// fade ramps, index 0 = full colour, last = background
const GREY_TO_BLACK: [Colors; 5] = [
    Colors::TapeBg,
    Colors::Grey36,
    Colors::Grey24,
    Colors::Grey12,
    Colors::Black,
];
const GREY_TO_WHITE: [Colors; 7] = [
    Colors::TapeBg,
    Colors::Grey64,
    Colors::Grey80,
    Colors::Gray,
    Colors::DarkGray,
    Colors::LightGray,
    Colors::White,
];
const MAG_TO_BLACK: [Colors; 5] = [
    Colors::Magenta,
    Colors::DarkMagenta,
    Colors::Magenta90,
    Colors::Magenta45,
    Colors::Black,
];
const MAG_TO_WHITE: [Colors; 5] = [
    Colors::Magenta,
    Colors::Violet,
    Colors::Plum,
    Colors::Thistle,
    Colors::White,
];
const EDGE_TO_BLACK: [Colors; 6] = [
    Colors::White,
    Colors::Silver,
    Colors::Gray,
    Colors::DimGray,
    Colors::Grey36,
    Colors::Black,
];
const TEXT_FADE: [Colors; 5] = [
    Colors::Gainsboro,
    Colors::Silver,
    Colors::Gray,
    Colors::DimGray,
    Colors::Grey64,
];

/// colour of a ramp for fade factor f (1 = full colour, 0 = background)
fn ramp(r: &[Colors], f: f32) -> Colors {
    let n = r.len() - 1;
    let i = ((1.0 - f.fclamp(0.0, 1.0)) * n as f32 + 0.5) as usize;
    r[i.min(n)]
}

/// colour factor at an angle from 3 o'clock (degrees, positive upwards):
/// full colour over the whole visible span, the ends are cut off without a
/// fade (the ramps only give the background for 0)
fn fade(phi: f32) -> f32 {
    if phi.abs() <= HALF_DEG {
        1.0
    } else {
        0.0
    }
}

/// What the tape shows: the table, the stage under the 3 o'clock mark
/// (fractional) and the recommended stage
pub struct TapeState {
    pub table: FlapTable,
    /// fractional stage index at 3 o'clock
    pub reference: f32,
    pub recommended: Option<usize>,
    /// true: actual position known, arrow and line are drawn
    pub with_sensor: bool,
}

impl TapeState {
    /// None when the glider has no flaps or nothing is recommended yet
    pub fn of(cm: &CoreModel) -> Option<Self> {
        let table = cm.flap_table();
        if table.is_empty() {
            return None;
        }
        let rec = cm.calculated.flap_rec.map(|r| r as usize);
        let actual = cm.calculated.flap_actual;
        let reference = match (actual, rec) {
            (Some(a), _) => a,
            (None, Some(r)) => r as f32,
            (None, None) => return None,
        };
        Some(TapeState {
            table,
            reference,
            recommended: rec,
            with_sensor: actual.is_some(),
        })
    }

    /// angle of stage i from 3 o'clock (degrees, positive = upwards)
    fn stage_phi(&self, i: usize) -> f32 {
        (i as f32 - self.reference) * PITCH_DEG
    }
}

/// Draw the tape. `light_outer`: the channel between core and scale line is
/// light (white outer band) - the outer part fades into white then.
pub fn draw_flap_tape<D>(
    display: &mut D,
    cm: &CoreModel,
    st: &TapeState,
    light_outer: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let center = cm.device_const.sizes.display.center;
    let width = cm.device_const.sizes.display.width as i32;
    let r_out = TAPE_R_OUT;
    let ro2 = (r_out * r_out) as i32;
    let ri2 = TAPE_R_IN * TAPE_R_IN;
    let e2 = EDGE_R_IN * EDGE_R_IN;
    let c2 = CORE_R * CORE_R;

    // vertical extent: half span + a little
    let half_rad = HALF_DEG.to_radians();
    let dy_max = (r_out * half_rad.sin()) as i32 + 2;

    let rec_phi = st.recommended.map(|r| st.stage_phi(r));
    let hl = crate::controller::page_layout::highlight(cm)
        == Some(crate::model::page_layout::Zone::FlapTape);
    let outer_grey: &[Colors] = if light_outer {
        &GREY_TO_WHITE
    } else {
        &GREY_TO_BLACK
    };
    let outer_mag: &[Colors] = if light_outer {
        &MAG_TO_WHITE
    } else {
        &MAG_TO_BLACK
    };

    for dy in -dy_max..=dy_max {
        let y = center.y + dy;
        if y < 0 || y >= cm.device_const.sizes.display.height as i32 {
            continue;
        }
        let d2 = dy * dy;
        if d2 > ro2 {
            continue;
        }
        // x span of the ring at this row (right side only)
        let x_out = isqrt(ro2 - d2);
        let x_in = if e2 > d2 { isqrt(e2 - d2) } else { 0 };
        let mut prev_col: Option<Colors> = None;
        let mut run_start = 0;
        let flush = |display: &mut D, from: i32, to: i32, col: Colors| -> Result<(), CoreError> {
            let x0 = (center.x + from).max(0);
            let x1 = (center.x + to).min(width - 1);
            if x1 >= x0 {
                // Safety: x and y are clamped to the display
                unsafe {
                    display.draw_line_unchecked(
                        y as usize * width as usize + x0 as usize,
                        (x1 - x0 + 1) as usize,
                        col,
                    );
                }
            }
            Ok(())
        };
        for dx in x_in..=x_out {
            let r2 = dx * dx + d2;
            if r2 < e2 || r2 > ro2 {
                continue;
            }
            // angle from 3 o'clock, positive upwards (screen y runs down)
            let phi = ((-dy) as f32).atan2(dx as f32).to_degrees();
            if phi.abs() > HALF_DEG {
                if let Some(pc) = prev_col.take() {
                    flush(display, run_start, dx - 1, pc)?;
                }
                continue;
            }
            // the band keeps its full outline: beyond the first / last stage it
            // is just grey and empty (no name, no magenta)
            let f = fade(phi);
            let col = if r2 < ri2 {
                // white inner edge
                ramp(&EDGE_TO_BLACK, f)
            } else {
                let is_rec = match rec_phi {
                    Some(rp) => (phi - rp).abs() < PITCH_DEG / 2.0 - REC_GAP_DEG,
                    None => false,
                };
                if r2 < c2 {
                    ramp(
                        if is_rec {
                            &MAG_TO_BLACK
                        } else {
                            &GREY_TO_BLACK
                        },
                        f,
                    )
                } else {
                    ramp(if is_rec { outer_mag } else { outer_grey }, f)
                }
            };
            // page layout editor: the tape is the chosen zone
            let col = if hl && col == Colors::TapeBg {
                crate::view::page_layout::HL_FILL
            } else {
                col
            };
            match prev_col {
                Some(pc) if pc == col => (),
                Some(pc) => {
                    flush(display, run_start, dx - 1, pc)?;
                    prev_col = Some(col);
                    run_start = dx;
                }
                None => {
                    prev_col = Some(col);
                    run_start = dx;
                }
            }
        }
        if let Some(pc) = prev_col {
            flush(display, run_start, x_out, pc)?;
        }
    }

    // stage names
    for i in 0..st.table.len() {
        let phi = st.stage_phi(i);
        if phi.abs() > HALF_DEG - 2.0 {
            continue;
        }
        let f = fade(phi);
        let is_rec = st.recommended == Some(i);
        let col = if is_rec {
            Colors::White
        } else {
            if f < 0.15 {
                continue;
            }
            ramp(&TEXT_FADE, f.powf(0.8))
        };
        let a = phi.to_radians();
        let p = Point::new(
            center.x + (R_TXT * a.cos()) as i32,
            center.y - (R_TXT * a.sin()) as i32,
        );
        // sketch fub18 -> u8g2 fub14
        text_for(18).f.render_aligned(
            st.table.name(i),
            p,
            VerticalPosition::Center,
            HorizontalAlignment::Center,
            FontColor::Transparent(col),
            display,
        )?;
    }

    // actual position: orange line through the band (gap at the digits)
    // and the arrow at the inner edge, both fixed at 3 o'clock
    if st.with_sensor {
        let orange = cm.palette().vario.ring_needle;
        for (r0, r1) in [
            (TAPE_R_IN, (R_TXT - 10.0) as i32),
            ((R_TXT + 10.0) as i32, r_out as i32),
        ] {
            let p0 = Point::new(center.x + r0, center.y);
            let p1 = Point::new(center.x + r1, center.y);
            Line::new(p0, p1)
                .into_styled(PrimitiveStyle::with_stroke(Colors::Black, 5))
                .draw(display)?;
            Line::new(p0, p1)
                .into_styled(PrimitiveStyle::with_stroke(orange, 3))
                .draw(display)?;
        }
        let tip = Point::new(center.x + TAPE_R_IN - 1, center.y);
        let hw = 12;
        let b1 = Point::new(center.x + TAPE_R_IN - 17, center.y - hw);
        let b2 = Point::new(center.x + TAPE_R_IN - 17, center.y + hw);
        Triangle::new(
            Point::new(tip.x + 1, tip.y),
            Point::new(b1.x - 2, b1.y - 2),
            Point::new(b2.x - 2, b2.y + 2),
        )
        .into_styled(PrimitiveStyle::with_fill(Colors::Black))
        .draw(display)?;
        Triangle::new(tip, b1, b2)
            .into_styled(PrimitiveStyle::with_fill(orange))
            .draw(display)?;
    }
    Ok(())
}
