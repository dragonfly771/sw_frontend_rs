//! Stacked info boxes of the ring design vario page
//!
//! Each box shows three rows: a short caption on top, the value in a big
//! font in the middle and the unit below. The content is selected with the
//! same [`LineView`] enum that is used by the classic info lines.

use super::vario_infoview::LineView;
use crate::view::ring_menu::isqrt;
use crate::{
    model::{DataSource, PotVerdict},
    tformat, Colors, CoreError, CoreModel, DrawImage, FloatToSpeed, Speed,
};
use embedded_graphics::{draw_target::DrawTarget, geometry::Point};
use heapless::String;
use u8g2_fonts::{
    types::{FontColor, HorizontalAlignment, VerticalPosition},
    FontRenderer,
};

#[allow(unused)]
use micromath::F32Ext;

/// Horizontal placement of a stacked info box relative to `pos`
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum BoxAlign {
    /// `pos.x` is the middle of the box
    Center,
    /// `pos.x` is the left edge - the box grows to the right
    Left,
    /// `pos.x` is the right edge - the box grows to the left. Needed on the
    /// light ring page, where a box running outward would end up as white
    /// text on the white outer band.
    Right,
}

/// Short caption of a viewable, suitable for the small caption font
pub fn caption(lv: LineView) -> &'static str {
    match lv {
        LineView::None => "",
        LineView::Altitude => "ALT",
        LineView::PotentialClimb => "POT",
        LineView::AverageClimbRate => "AVG",
        LineView::ThermalClimb => "THERMAL",
        LineView::FlightLevel => "FL",
        LineView::TrueCourse => "TRK",
        LineView::UtcTime => "UTC",
        LineView::WindAndDelta => "WIND",
        LineView::DriftAngle => "DRIFT",
        LineView::WindAndAvgWind => "AVG WIND",
        LineView::WindArrow => "WIND",
        LineView::AvgWindArrow => "AVG WIND",
        LineView::SpeedToFly => "STF",
        LineView::TrueAirSpeed => "TAS",
        LineView::BatteryVoltage => "BAT",
        LineView::GLoad => "G-LOAD",
        LineView::CircleDiameter => "CIRCLE",
        LineView::CircleMaxMin => "MAX-MIN",
        LineView::Heading => "HDG",
        LineView::BankAngle => "BANK",
        LineView::SlipAngle => "SLIP",
        LineView::IndicatedAirSpeed => "IAS",
        LineView::EquivalentAirspeed => "EAS",
        LineView::PitchAngle => "PITCH",
        LineView::LastElemntNotInUse => "",
    }
}

/// Value of the POT box: always a number (flight test feedback). In flight
/// what a circle here is worth (controller/pot_climb.rs): the circle value
/// of the air in straight flight, the energy balance while rolling in, the
/// measured climb in the circle. On the ground and without GNSS the normal
/// average climb like the AVG box. Same chain as the classic info line -
/// and as the ring marker with "AVG Marker = Potential", so marker and box
/// never disagree.
pub(crate) fn pot_box_value(cm: &CoreModel) -> Speed {
    match cm.calculated.pot_climb {
        Some(speed) => speed,
        None => match cm.control.avg_climb_rate_src {
            DataSource::Frontend => cm.calculated.av2_climb_rate,
            DataSource::Sensorbox => cm.sensor.average_climb_rate,
        },
    }
}

/// Value of the THERMAL box: the mean climb of the thermal, height gained
/// since circling began divided by the time (controller/tick_1s.rs). After
/// the thermal the value of that thermal stays until the next one begins -
/// in cruise the box says what the last thermal gave.
pub(crate) fn thermal_box_value(cm: &CoreModel) -> Speed {
    if cm.control.tcr_mode == crate::model::TcrMode::StraightFlight {
        cm.calculated.thermal_climb_last
    } else {
        cm.calculated.thermal_climb_rate
    }
}

/// Colour of the value: magenta while the POT box shows an estimate
/// (straight flight, roll-in, first circle), the normal value colour once
/// the climb is measured
pub(crate) fn value_color(cm: &CoreModel, lv: LineView) -> Colors {
    if lv == LineView::PotentialClimb && cm.calculated.pot_predictive {
        Colors::Magenta
    } else {
        cm.palette().vario.value
    }
}

/// Yes / no of the box: only the POT box has one, and only in flight
/// (the fallback to the average climb on the ground has none). "POT Yes/No"
/// off: the box shows its value without the symbol (the advice whether to
/// circle, the check mark and the cross, can be switched off).
pub(crate) fn pot_verdict(cm: &CoreModel, lv: LineView) -> PotVerdict {
    if lv == LineView::PotentialClimb && cm.calculated.pot_climb.is_some() && cm.config.pot_advice {
        cm.calculated.pot_verdict
    } else {
        PotVerdict::None
    }
}

/// A wind arrow box: arrow rotation (like the arrows of the centre: north
/// up while circling, heading up in straight flight), its colour (live wind
/// sky blue, average wind grey like the centre) and the wind direction in
/// degrees (where the wind comes from) for the caption line.
pub(crate) struct WindArrowBox {
    pub rotation: f32,
    pub color: Colors,
    pub from_deg: f32,
}

pub(crate) fn wind_arrow(cm: &CoreModel, lv: LineView) -> Option<WindArrowBox> {
    let (vector, color) = match lv {
        LineView::WindArrow => (&cm.sensor.wind_vector, cm.palette().vario.ring_wind_fill),
        LineView::AvgWindArrow => (
            &cm.sensor.average_wind,
            cm.palette().vario.ring_avg_wind_fill,
        ),
        _ => return None,
    };
    let from = vector.angle();
    let slow = cm.sensor.airspeed.ias() < 30.0.km_h();
    let rotation = if slow {
        // on the ground: the arrow points down (as the centre arrows)
        core::f32::consts::PI
    } else {
        match cm.control.fly_mode {
            crate::FlyMode::Circling => from.to_radians(),
            crate::FlyMode::StraightFlight => (from - cm.sensor.euler_yaw).to_radians(),
        }
    };
    let mut from_deg = from.to_degrees() % 360.0;
    if from_deg < 0.0 {
        from_deg += 360.0;
    }
    Some(WindArrowBox {
        rotation,
        color,
        from_deg,
    })
}

/// Value and unit strings of a viewable
pub(crate) fn value_and_unit(cm: &CoreModel, lv: LineView) -> (String<20>, &'static str) {
    let mut unit: &'static str = "";
    let value: String<20> = match lv {
        LineView::None | LineView::LastElemntNotInUse => String::new(),
        LineView::Altitude => {
            unit = cm.config.unit_height.as_str();
            let altitude = cm.sensor.pressure_altitude.qnh_altitude();
            tformat!(20, "{:.0}", cm.config.unit_height.as_f32(altitude)).unwrap()
        }
        LineView::PotentialClimb => {
            unit = cm.config.unit_vertical_speed.as_str();
            let speed = pot_box_value(cm);
            let s = cm.config.unit_vertical_speed.value_str(speed);
            if speed.to_m_s() >= 0.05 {
                tformat!(20, "+{}", s.as_str()).unwrap()
            } else {
                tformat!(20, "{}", s.as_str()).unwrap()
            }
        }
        LineView::AverageClimbRate => {
            unit = cm.config.unit_vertical_speed.as_str();
            let speed = match cm.control.avg_climb_rate_src {
                DataSource::Frontend => cm.calculated.av2_climb_rate,
                DataSource::Sensorbox => cm.sensor.average_climb_rate,
            };
            let s = cm.config.unit_vertical_speed.value_str(speed);
            if speed.to_m_s() >= 0.05 {
                tformat!(20, "+{}", s.as_str()).unwrap()
            } else {
                tformat!(20, "{}", s.as_str()).unwrap()
            }
        }
        LineView::ThermalClimb => {
            unit = cm.config.unit_vertical_speed.as_str();
            let speed = thermal_box_value(cm);
            let s = cm.config.unit_vertical_speed.value_str(speed);
            if speed.to_m_s() >= 0.05 {
                tformat!(20, "+{}", s.as_str()).unwrap()
            } else {
                tformat!(20, "{}", s.as_str()).unwrap()
            }
        }
        LineView::FlightLevel => {
            let mut altitude = cm.sensor.pressure_altitude.qne_altitude().to_ft() / 100.0;
            if altitude < 0.0 {
                altitude = 0.0
            }
            tformat!(20, "{:03.0}", altitude).unwrap()
        }
        LineView::TrueCourse => tformat!(20, "{:.0}°", cm.sensor.gps_track.to_degrees()).unwrap(),
        LineView::UtcTime => {
            let s = cm.sensor.gps_date_time.to_time_string();
            tformat!(20, "{}", s.as_str()).unwrap()
        }
        LineView::WindAndDelta | LineView::WindAndAvgWind => {
            unit = cm.config.unit_horizontal_speed.as_str();
            // "Wind, avg Wind" is the mean wind in numbers (caption AVG
            // WIND); before 0.4.1.52 both boxes showed the live wind
            // (Stefan 15.11.2026: the mean wind without an arrow)
            let wind = if lv == LineView::WindAndAvgWind {
                &cm.sensor.average_wind
            } else {
                &cm.sensor.wind_vector
            };
            let angle = if cm.sensor.airspeed.ias() < 30.0.km_h() {
                cm.sensor.euler_yaw
            } else {
                wind.angle()
            };
            let speed_str = cm.config.unit_horizontal_speed.value_str(wind.speed());
            // Stefly design P1: "270/25" (direction / speed), no degree sign
            tformat!(20, "{:03.0}/{}", angle.to_degrees(), speed_str.as_str()).unwrap()
        }
        LineView::WindArrow | LineView::AvgWindArrow => {
            // the speed only - the arrow shows the direction, the degrees
            // stand in the caption line (wind_arrow)
            unit = cm.config.unit_horizontal_speed.as_str();
            let speed = if lv == LineView::WindArrow {
                cm.sensor.wind_vector.speed()
            } else {
                cm.sensor.average_wind.speed()
            };
            let s = cm.config.unit_horizontal_speed.value_str(speed);
            tformat!(20, "{}", s.as_str()).unwrap()
        }
        LineView::DriftAngle => {
            let track = cm.sensor.gps_track.to_degrees();
            let heading = cm.sensor.euler_yaw.to_degrees();
            let mut drift_angle = track - heading;
            if drift_angle.abs() > 360.0 {
                drift_angle = 0.0
            }
            while drift_angle > 180.0 {
                drift_angle -= 360.0
            }
            while drift_angle < -180.0 {
                drift_angle += 360.0
            }
            if drift_angle > 0.0 {
                tformat!(20, "+{:.0}°", drift_angle).unwrap()
            } else {
                tformat!(20, "{:.0}°", drift_angle).unwrap()
            }
        }
        LineView::SpeedToFly => {
            unit = cm.config.unit_horizontal_speed.as_str();
            let s = cm
                .config
                .unit_horizontal_speed
                .value_str(cm.calculated.speed_to_fly_1s);
            tformat!(20, "{}", s.as_str()).unwrap()
        }
        LineView::TrueAirSpeed => {
            unit = cm.config.unit_horizontal_speed.as_str();
            let s = cm
                .config
                .unit_horizontal_speed
                .value_str(cm.sensor.airspeed.tas());
            tformat!(20, "{}", s.as_str()).unwrap()
        }
        LineView::IndicatedAirSpeed => {
            unit = cm.config.unit_horizontal_speed.as_str();
            let s = cm
                .config
                .unit_horizontal_speed
                .value_str(cm.sensor.airspeed.ias());
            tformat!(20, "{}", s.as_str()).unwrap()
        }
        LineView::EquivalentAirspeed => {
            unit = cm.config.unit_horizontal_speed.as_str();
            if let Some(eas) = cm.sensor.airspeed.eas(cm.sensor.g_force) {
                let s = cm.config.unit_horizontal_speed.value_str(eas);
                tformat!(20, "{}", s.as_str()).unwrap()
            } else {
                tformat!(20, "--").unwrap()
            }
        }
        LineView::BatteryVoltage => {
            unit = "V";
            let voltage = cm.device.supply_voltage;
            if voltage > 0.1 {
                tformat!(20, "{:.1}", voltage).unwrap()
            } else {
                tformat!(20, "--").unwrap()
            }
        }
        LineView::GLoad => {
            unit = "g";
            let g = cm.sensor.g_force.to_m_s2() / 9.81;
            tformat!(20, "{:.2}", g).unwrap()
        }
        LineView::CircleDiameter => {
            unit = cm.config.unit_height.as_str();
            if cm.calculated.circle_diameter_valid {
                let s = cm
                    .config
                    .unit_height
                    .value_str(cm.calculated.circle_diameter);
                tformat!(20, "{}", s.as_str()).unwrap()
            } else {
                tformat!(20, "--").unwrap()
            }
        }
        LineView::CircleMaxMin => {
            unit = cm.config.unit_vertical_speed.as_str();
            if cm.calculated.circle_max_min_valid {
                let s = cm
                    .config
                    .unit_vertical_speed
                    .value_str(cm.calculated.circle_max_min_last);
                tformat!(20, "{}", s.as_str()).unwrap()
            } else {
                tformat!(20, "--").unwrap()
            }
        }
        LineView::Heading => tformat!(20, "{:.0}°", cm.sensor.euler_yaw.to_degrees()).unwrap(),
        LineView::BankAngle => {
            if cm.sensor.roll_pitch_available() {
                tformat!(20, "{:.0}°", cm.sensor.euler_roll.to_degrees()).unwrap()
            } else {
                tformat!(20, "--").unwrap()
            }
        }
        LineView::SlipAngle => tformat!(20, "{:.0}°", cm.sensor.slip_angle.to_degrees()).unwrap(),
        LineView::PitchAngle => {
            if cm.sensor.roll_pitch_available() {
                tformat!(20, "{:.0}°", cm.sensor.euler_pitch.to_degrees()).unwrap()
            } else {
                tformat!(20, "--").unwrap()
            }
        }
    };
    (value, unit)
}

/// Draw one stacked info box (caption / value / unit) centered at `pos`
#[allow(dead_code)]
pub fn draw_ring_infobox<D>(
    display: &mut D,
    cm: &CoreModel,
    pos: Point,
    lv: LineView,
    align: BoxAlign,
    small: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    if lv == LineView::None || lv == LineView::LastElemntNotInUse {
        return Ok(());
    }
    let (value, _) = value_and_unit(cm, lv);
    let digits = value.chars().filter(|c| c.is_ascii_digit()).count();
    let value_font = if small {
        &cm.device_const.value_font_small
    } else if digits >= 4 {
        // long values (e.g. a 4 digit altitude) drop one font size
        &cm.device_const.value_font_mid
    } else {
        &cm.device_const.value_font
    };
    draw_box(
        display,
        cm,
        pos,
        lv,
        align,
        value_font,
        &cm.device_const.caption_font,
    )
}

/// Width of one value string in a given font (monospaced value fonts)
fn value_width(font: &FontRenderer, s: &str) -> i32 {
    let adv = font
        .get_rendered_dimensions('0', Point::zero(), VerticalPosition::Center)
        .map(|d| d.advance.x)
        .unwrap_or(12);
    s.chars().count() as i32 * adv
}

/// Height one stacked box really occupies on the display
///
/// Not the nominal line height of the fonts but exactly what `draw_box`
/// below produces: the rendered extents of the value plus the two gaps
/// plus a caption line with ascender and descender. So the caller can
/// decide which font combination fits without any guessing.
pub fn box_height(value_font: &FontRenderer, caption_font: &FontRenderer) -> i32 {
    let caption_height = caption_font.get_default_line_height() as i32;
    let value_height = value_font.get_default_line_height() as i32;
    let gap = caption_height * 85 / 100 - 20;
    let (v_top, v_bottom) = glyph_extents(value_font, "0", value_height / 2);
    let (c_top, c_bottom) = glyph_extents(caption_font, "Ag", caption_height / 2);
    (v_bottom - v_top) + 2 * gap + 2 * (c_bottom - c_top)
}

/// Draw the three large info boxes of the Big Boxes page core
///
/// `free` is the radius of the middle that the shortened needle leaves
/// empty. The three boxes are stacked over the diameter and use the very
/// same three rows, colours and spacing rules as every other info box -
/// only in the largest fonts that really fit. Font choice is made once for
/// all three boxes, so they stay the same size, and it is made from the
/// measured extents, so nothing can ever run out of the circle or into the
/// box above.
pub fn draw_big_boxes<D>(
    display: &mut D,
    cm: &CoreModel,
    cm_1s: &CoreModel,
    center: Point,
    free: i32,
    vario_mode: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    // three boxes over the diameter, with some air above and below
    let pitch = free * 5 / 8;
    // half the height of a value line - the width of the circle has to be
    // taken at the lower edge of the line, not at its middle
    let v_half = free / 5;
    let slots = [(2_usize, -pitch), (3, 0), (4, pitch)];

    // captions in the larger font as long as a whole box still fits
    let cap_big = &cm.device_const.caption_font_big;
    let caption_font = if box_height(&cm.device_const.value_font_big, cap_big) <= pitch {
        cap_big
    } else {
        &cm.device_const.caption_font
    };

    // width of the circle at the lower edge of the value line of a box
    let width_at = |dy: i32| {
        let edge = dy.abs() + v_half;
        2 * isqrt((free * free - edge * edge).max(0))
    };

    // largest value font that fits every one of the three boxes
    let mut value_font = &cm.device_const.value_font;
    for font in [
        &cm.device_const.value_font_big,
        &cm.device_const.value_font_big_mid,
    ] {
        if box_height(font, caption_font) > pitch {
            continue;
        }
        let mut fits = true;
        for (slot, dy) in slots {
            let (value, _) = value_and_unit(cm_1s, cm.config.box_view(slot, vario_mode));
            if value_width(font, value.as_str()) > width_at(dy) {
                fits = false;
            }
        }
        if fits {
            value_font = font;
            break;
        }
    }

    for (slot, dy) in slots {
        draw_box(
            display,
            cm_1s,
            Point::new(center.x, center.y + dy),
            cm.config.box_view(slot, vario_mode),
            BoxAlign::Center,
            value_font,
            caption_font,
        )?;
    }
    Ok(())
}

/// The one and only box renderer - every info box goes through here, so
/// the large boxes cannot drift away from the small ones in style.
fn draw_box<D>(
    display: &mut D,
    cm: &CoreModel,
    pos: Point,
    lv: LineView,
    align: BoxAlign,
    value_font: &FontRenderer,
    caption_font: &FontRenderer,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    if lv == LineView::None || lv == LineView::LastElemntNotInUse {
        return Ok(());
    }
    let (value, unit) = value_and_unit(cm, lv);
    // keep the units short in the boxes
    let unit = if unit == "knots" { "kt" } else { unit };

    let h_align = match align {
        BoxAlign::Center => HorizontalAlignment::Center,
        BoxAlign::Left => HorizontalAlignment::Left,
        BoxAlign::Right => HorizontalAlignment::Right,
    };
    let caption_height = caption_font.get_default_line_height() as i32;
    let value_height = value_font.get_default_line_height() as i32;

    // Position caption and unit by the real glyph extents, so that the
    // perceived gap above and below the value is identical (glyphs like a
    // lone "m" have no ascender and would otherwise appear farther away).
    let gap = caption_height * 85 / 100 - 20; // calibrated: perceived gap ~6 px on the 480 display
    let (v_top, v_bottom) = glyph_extents(value_font, "0", value_height / 2);
    let caption_txt = caption(lv);
    let (_, c_bottom) = glyph_extents(caption_font, caption_txt, caption_height / 2);
    let (u_top, _) = glyph_extents(caption_font, unit, caption_height / 2);
    let caption_y = pos.y + v_top - gap - c_bottom;
    let unit_y = pos.y + v_bottom + gap - u_top;

    caption_font.render_aligned(
        caption_txt,
        Point::new(pos.x, caption_y),
        VerticalPosition::Center,
        h_align,
        FontColor::Transparent(cm.palette().vario.unit),
        display,
    )?;

    render_value_tight(
        display,
        value_font,
        value.as_str(),
        pos,
        align,
        value_color(cm, lv),
    )?;

    if !unit.is_empty() {
        caption_font.render_aligned(
            unit,
            Point::new(pos.x, unit_y),
            VerticalPosition::Center,
            h_align,
            FontColor::Transparent(cm.palette().vario.unit),
            display,
        )?;
    }
    Ok(())
}

/// Render a value string aligned to `pos`.
///
/// Up to 0.4.1.38 the values used the monospaced Inconsolata, where the
/// decimal point took a full cell; this drew character by character and
/// pulled the point together. B612 is proportional with tabular digits, the
/// point is narrow by itself, so the string is drawn as it is (the name
/// stays for the callers).
pub(crate) fn render_value_tight<D>(
    display: &mut D,
    font: &FontRenderer,
    s: &str,
    pos: Point,
    align: BoxAlign,
    color: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    render_value_tight_v(
        display,
        font,
        s,
        pos,
        align,
        color,
        VerticalPosition::Center,
    )
}

/// width of a value as `render_value_tight` draws it (sum of the advances)
pub(crate) fn tight_width(font: &FontRenderer, s: &str) -> i32 {
    font.get_rendered_dimensions(s, Point::zero(), VerticalPosition::Baseline)
        .map(|d| d.advance.x)
        .unwrap_or(0)
}

/// `render_value_tight` with a free vertical anchor (Baseline for the P1
/// layouts, which place values by their baseline like the sketches)
pub(crate) fn render_value_tight_v<D>(
    display: &mut D,
    font: &FontRenderer,
    s: &str,
    pos: Point,
    align: BoxAlign,
    color: Colors,
    vpos: VerticalPosition,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let width = tight_width(font, s);
    let x = match align {
        BoxAlign::Center => pos.x - width / 2,
        BoxAlign::Left => pos.x,
        BoxAlign::Right => pos.x - width,
    };
    font.render(
        s,
        Point::new(x, pos.y),
        vpos,
        FontColor::Transparent(color),
        display,
    )?;
    Ok(())
}

/// the caption line of an R2 box: "ALT  m", "WIND  km/h"
pub fn r2_caption(lv: LineView, unit: &str) -> String<24> {
    let unit = if unit == "knots" { "kt" } else { unit };
    let cap = caption(lv);
    let mut s: String<24> = String::new();
    let _ = s.push_str(cap);
    if !unit.is_empty() {
        let _ = s.push_str("  ");
        let _ = s.push_str(unit);
    }
    s
}

/// unit of a viewable as shown in the boxes ("" for none)
pub fn unit_of(cm: &CoreModel, lv: LineView) -> &'static str {
    value_and_unit(cm, lv).1
}

/// value string of a viewable (for the font fit of the caller)
pub fn r2_value(cm: &CoreModel, lv: LineView) -> String<20> {
    value_and_unit(cm, lv).0
}

/// Top and bottom edge of the rendered glyphs, relative to a
/// VerticalPosition::Center anchor
fn glyph_extents(font: &FontRenderer, s: &str, fallback_half: i32) -> (i32, i32) {
    if !s.is_empty() {
        if let Ok(d) = font.get_rendered_dimensions(s, Point::zero(), VerticalPosition::Center) {
            if let Some(bb) = d.bounding_box {
                return (bb.top_left.y, bb.top_left.y + bb.size.height as i32);
            }
        }
    }
    (-fallback_half, fallback_half)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::test_fixture::{test_model, TestDisplay};
    use crate::FloatToSpeed;

    fn render(cm: &CoreModel) -> TestDisplay {
        let mut display = TestDisplay::new();
        draw_ring_infobox(
            &mut display,
            cm,
            Point::new(240, 240),
            LineView::PotentialClimb,
            BoxAlign::Center,
            false,
        )
        .unwrap();
        display
    }

    /// "Wind and Delta" is the live wind, "Wind, avg Wind" the mean wind, both in numbers
    #[test]
    fn the_two_wind_boxes_in_numbers_show_live_and_mean_wind() {
        use embedded_graphics::geometry::AngleUnit;
        let mut cm = crate::view::test_fixture::test_model();
        cm.sensor.airspeed = crate::AirSpeed::from_speeds(120.0.km_h(), 130.0.km_h());
        cm.sensor.wind_vector.set_angle(270.0f32.deg());
        cm.sensor.wind_vector.set_speed(25.0.km_h());
        cm.sensor.average_wind.set_angle(23.0f32.deg());
        cm.sensor.average_wind.set_speed(7.0.km_h());
        assert_eq!(
            value_and_unit(&cm, LineView::WindAndDelta).0.as_str(),
            "270/25"
        );
        assert_eq!(
            value_and_unit(&cm, LineView::WindAndAvgWind).0.as_str(),
            "023/7"
        );
        assert_eq!(caption(LineView::WindAndDelta), "WIND");
        assert_eq!(caption(LineView::WindAndAvgWind), "AVG WIND");
        // no arrow in either of them
        assert!(wind_arrow(&cm, LineView::WindAndDelta).is_none());
        assert!(wind_arrow(&cm, LineView::WindAndAvgWind).is_none());
    }

    /// The small wind arrow of a wind info box is north up while circling and
    /// track up in straight flight. The arrow is drawn from 6 o'clock,
    /// turned by `rotation`: 0 = it points down, the wind comes from the top.
    #[test]
    fn the_wind_arrow_of_a_box_is_north_up_while_circling_and_heading_up_in_straight_flight() {
        use embedded_graphics::geometry::AngleUnit;
        let mut cm = crate::view::test_fixture::test_model();
        cm.sensor.airspeed = crate::AirSpeed::from_speeds(120.0.km_h(), 130.0.km_h());
        cm.sensor.wind_vector.set_angle(270.0f32.deg());
        cm.sensor.average_wind.set_angle(40.0f32.deg());
        cm.sensor.euler_yaw = 100.0f32.deg();
        let deg = |cm: &CoreModel, lv| {
            wind_arrow(cm, lv)
                .unwrap()
                .rotation
                .to_degrees()
                .rem_euclid(360.0)
        };
        // circling: north is up, whatever the heading - wind from the west
        // comes from the left
        cm.control.fly_mode = crate::FlyMode::Circling;
        assert!((deg(&cm, LineView::WindArrow) - 270.0).abs() < 0.01);
        assert!((deg(&cm, LineView::AvgWindArrow) - 40.0).abs() < 0.01);
        cm.sensor.euler_yaw = 200.0f32.deg();
        assert!(
            (deg(&cm, LineView::WindArrow) - 270.0).abs() < 0.01,
            "the heading does not turn it"
        );
        // straight flight: the nose is up - heading 200, wind from 270:
        // from 70 degrees right of the nose
        cm.control.fly_mode = crate::FlyMode::StraightFlight;
        assert!((deg(&cm, LineView::WindArrow) - 70.0).abs() < 0.01);
        assert!((deg(&cm, LineView::AvgWindArrow) - 200.0).abs() < 0.01);
        // head wind: from the top, in both boxes the same rule as the centre arrows
        cm.sensor.euler_yaw = 270.0f32.deg();
        assert!(deg(&cm, LineView::WindArrow).min(360.0 - deg(&cm, LineView::WindArrow)) < 0.01);
        // on the ground the arrow points down, as the arrows of the centre
        cm.sensor.airspeed = crate::AirSpeed::from_speeds(5.0.km_h(), 5.0.km_h());
        assert!((deg(&cm, LineView::WindArrow) - 180.0).abs() < 0.01);
    }

    #[test]
    fn a_new_device_takes_the_vario_average_from_the_sensor_box() {
        let mut cm = test_model();
        assert!(cm.control.avg_climb_rate_src == DataSource::Sensorbox);
        cm.calculated.av2_climb_rate = 0.4.m_s();
        cm.sensor.average_climb_rate = 2.3.m_s();
        let (value, _) = value_and_unit(&cm, LineView::AverageClimbRate);
        assert!(value.contains("2.3") || value.contains("2,3"), "{value}");
    }

    #[test]
    fn pot_box_always_shows_a_number_and_magenta_only_for_estimates() {
        let mut cm = test_model();
        // on the ground / no GNSS: the AVG fallback in the normal colour -
        // never "--"
        cm.calculated.pot_climb = None;
        cm.calculated.pot_predictive = false;
        cm.calculated.av2_climb_rate = (-1.2).m_s();
        cm.sensor.average_climb_rate = (-1.2).m_s();
        let (value, _) = value_and_unit(&cm, LineView::PotentialClimb);
        assert!(value.contains("1.2") || value.contains("1,2"), "{value}");
        assert!(!value.contains("--"));
        let d = render(&cm);
        assert_eq!(d.colored_rows(200, 280, Colors::Magenta), 0);

        // straight flight: the circle value of the air, an estimate -> magenta
        cm.calculated.pot_climb = Some(1.5.m_s());
        cm.calculated.pot_predictive = true;
        let (value, _) = value_and_unit(&cm, LineView::PotentialClimb);
        assert!(value.starts_with('+'), "{value}");
        let d = render(&cm);
        assert!(d.colored_rows(200, 280, Colors::Magenta) > 50);

        // roll-in and first circle: energy balance, still an estimate -> magenta
        cm.calculated.pot_climb = Some(2.1.m_s());
        cm.calculated.pot_predictive = true;
        let d = render(&cm);
        assert!(d.colored_rows(200, 280, Colors::Magenta) > 50);

        // measured (after one circle): normal colour
        cm.calculated.pot_predictive = false;
        let d = render(&cm);
        assert_eq!(d.colored_rows(200, 280, Colors::Magenta), 0);
        assert!(d.painted_box(180, 200, 300, 280) > 50);
    }
}

#[cfg(test)]
mod thermal_box_tests {
    use super::*;
    use crate::{model::TcrMode, test_harness};

    #[test]
    fn the_thermal_box_shows_the_running_thermal_and_then_keeps_the_last_one() {
        let (mut cm, _cc, _tx) = test_harness!();
        cm.control.tcr_mode = TcrMode::Climbing;
        cm.calculated.thermal_climb_rate = 2.3.m_s();
        assert!((thermal_box_value(&cm).to_m_s() - 2.3).abs() < 1e-6);
        // 30 s after the exit the running value is cleared, the box keeps it
        cm.control.tcr_mode = TcrMode::StraightFlight;
        cm.calculated.thermal_climb_last = cm.calculated.thermal_climb_rate;
        cm.calculated.thermal_climb_rate = 0.0.m_s();
        assert!((thermal_box_value(&cm).to_m_s() - 2.3).abs() < 1e-6);
        let (value, unit) = value_and_unit(&cm, LineView::ThermalClimb);
        assert_eq!(value.as_str(), "+2.3");
        assert_eq!(unit, "m/s");
        assert_eq!(caption(LineView::ThermalClimb), "THERMAL");
    }
}
