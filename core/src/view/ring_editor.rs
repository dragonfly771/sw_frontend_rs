//! Editor rendering for the Stefly (ring) design.
//!
//! One idiom in two sizes: the header in the dark blue cap, the value the
//! knob changes in a light blue bar across the circle (black number), grey
//! sub lines below. On the page it covers the black core (the vario scale
//! keeps running outside), in the full screen setup menu the whole display.
//! No arcs at the rim (Stefan 28.10.2026: no round progress bars).

use crate::{
    model::{
        editable::{Content, Params, MAX_ENUM_VARIANTS},
        CoreModel, DisplayActive, STF_SWITCH_MODE_BALLAST,
    },
    utils::Colors,
    CoreError, DrawImage, Editable,
};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, PrimitiveStyle},
};
#[allow(unused_imports)]
use micromath::F32Ext;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

/// geometry of the two editor sizes
struct Geo {
    center: Point,
    /// the circle the editor fills: the display (full screen) resp. the
    /// black core of the page
    radius: i32,
    full: bool,
}

impl Geo {
    fn new(cm: &CoreModel) -> Self {
        let h = cm.device_const.sizes.display.height as i32;
        let full = cm.config.display_active == DisplayActive::Menu;
        Geo {
            center: cm.device_const.sizes.display.center,
            radius: if full {
                h / 2
            } else {
                crate::view::ring_menu::overlay_radius(cm)
            },
            full,
        }
    }
    /// lower edge of the header cap
    fn cap(&self) -> i32 {
        if self.full {
            crate::view::ring_menu::CAP_FULL
        } else {
            crate::view::ring_menu::CAP_OVERLAY
        }
    }
    fn sub_y(&self, nr: i32) -> i32 {
        let base = if self.full { 66 } else { 56 };
        base + nr * 38
    }
    /// distance of the neighbour options above / below the bar
    fn row_dy(&self) -> i32 {
        BAR_H2 + 28
    }
}

/// short editor titles where the full name does not add anything
fn header_name(name: &str) -> &str {
    match name {
        "Water Ballast" => "Ballast",
        _ => name,
    }
}

fn gray_line<D>(
    display: &mut D,
    cm: &CoreModel,
    center: Point,
    y_offset: i32,
    text: &str,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    cm.device_const.value_font_small.render_aligned(
        text,
        Point::new(center.x, center.y + y_offset),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(Colors::DarkGray),
        display,
    )?;
    Ok(())
}

/// three line fisheye used by enum and list editors: the current option in
/// the light blue bar, the neighbours dim above and below
fn fisheye<D>(
    display: &mut D,
    cm: &CoreModel,
    g: &Geo,
    prev: &str,
    cur: &str,
    next: &str,
    counter: &str,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    let dy = g.row_dy();
    let cy = BAR_DY;
    if !cur.is_empty() {
        value_bar(display, cm, g)?;
        cm.device_const.value_font.render_aligned(
            cur,
            g.center + Point::new(0, cy),
            VerticalPosition::Center,
            HorizontalAlignment::Center,
            FontColor::Transparent(Colors::Black),
            display,
        )?;
    }
    for (txt, off) in [(prev, cy - dy), (next, cy + dy)] {
        if txt.is_empty() {
            continue;
        }
        cm.device_const.value_font_small.render_aligned(
            txt,
            g.center + Point::new(0, off),
            VerticalPosition::Center,
            HorizontalAlignment::Center,
            FontColor::Transparent(Colors::DimGray),
            display,
        )?;
    }
    if !counter.is_empty() {
        crate::view::ring_menu::position_text(display, g.center, cy + dy + 44, counter)?;
    }
    Ok(())
}

/// half height of the value bar and its centre (relative to the display
/// centre)
const BAR_H2: i32 = 36;
const BAR_DY: i32 = -4;

/// up / down marks of the value bar: 0 none, 1 black marks in the bar at
/// its right end, 2 blue marks above and below the number (build time
/// switch for the comparison renders: LARUS_VALUE_ARROWS=0/1/2)
const ARROWS: u8 = match option_env!("LARUS_VALUE_ARROWS") {
    Some(v) => v.as_bytes()[0] - b'0',
    None => 1,
};

/// y (relative to the centre) of the info lines below the bar
fn info_y(nr: i32) -> i32 {
    BAR_DY + BAR_H2 + 26 + if ARROWS == 2 { 16 } else { 0 } + nr * 28
}

/// the light blue bar across the whole circle (Stefan 28.10.2026: one bar,
/// no dark band, no field in it - the same blue as the value field of the
/// flight menu)
fn value_bar<D>(display: &mut D, cm: &CoreModel, g: &Geo) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    use embedded_graphics::primitives::Rectangle;
    let width = cm.device_const.sizes.display.width as i32;
    let r = g.radius;
    for dy in BAR_DY - BAR_H2..=BAR_DY + BAR_H2 {
        let h = crate::view::ring_menu::isqrt((r * r - dy * dy).max(0));
        let x0 = (g.center.x - h).max(0);
        let x1 = (g.center.x + h).min(width - 1);
        if x1 > x0 {
            display.fill_solid(
                &Rectangle::new(
                    Point::new(x0, g.center.y + dy),
                    Size::new((x1 - x0 + 1) as u32, 1),
                ),
                cm.palette().vario.ring_select,
            )?;
        }
    }
    Ok(())
}

/// The value of a number editor: the light blue bar, in it the number
/// (black, B612) with its unit, the up / down marks (ARROWS). The size is
/// the largest in which `fit` (usually the value itself) fits the bar.
fn value_box<D>(
    display: &mut D,
    cm: &CoreModel,
    g: &Geo,
    value: &str,
    fit: &str,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    use crate::view::fonts_p1::{render_value, text_for, VALUE_27, VALUE_33, VALUE_42};
    use embedded_graphics::primitives::Triangle;
    value_bar(display, cm, g)?;
    let c = g.center + Point::new(0, BAR_DY);
    // number and unit ("31 kg" -> "31", "kg")
    let (num, unit) = match value.rfind(' ') {
        Some(i) if value[..i].chars().any(|ch| ch.is_ascii_digit()) => {
            (&value[..i], value[i + 1..].trim())
        }
        _ => (value, ""),
    };
    // unit next to the number: Barlow 18 (was 14)
    let uf = text_for(22).f;
    let w = |f: &u8g2_fonts::FontRenderer, t: &str| {
        f.get_rendered_dimensions(t, Point::zero(), VerticalPosition::Baseline)
            .map(|d| d.advance.x)
            .unwrap_or(0)
    };
    let uw = if unit.is_empty() { 0 } else { w(uf, unit) + 10 };
    // the largest size that leaves room for the marks in the bar - a long
    // word ("Competition Pro") steps down instead of running out of the bar
    let edge = crate::view::ring_menu::isqrt(g.radius * g.radius - (BAR_H2 + BAR_DY.abs()).pow(2));
    let room = 2 * (edge - 58);
    let (nf, h) = [(&VALUE_42, 42), (&VALUE_33, 32), (&VALUE_27, 27)]
        .into_iter()
        .find(|(f, _)| w(f, num).max(w(f, fit)) + uw <= room)
        .unwrap_or((&VALUE_27, 27));
    let nw = w(nf, num);
    // number and unit share the baseline, the pair centred
    let base = c.y + h / 2;
    let nx = c.x - (nw + uw) / 2;
    render_value(
        display,
        nf,
        num,
        Point::new(nx, base),
        VerticalPosition::Baseline,
        HorizontalAlignment::Left,
        Colors::Black,
    )?;
    if !unit.is_empty() {
        uf.render_aligned(
            unit,
            Point::new(nx + nw + 10, base),
            VerticalPosition::Baseline,
            HorizontalAlignment::Left,
            FontColor::Transparent(Colors::Black),
            display,
        )?;
    }
    // up / down marks: the knob changes this value
    let tri = |display: &mut D,
               x: i32,
               y: i32,
               up: bool,
               hw: i32,
               hh: i32,
               col: Colors|
     -> Result<(), CoreError> {
        let s = if up { -1 } else { 1 };
        Triangle::new(
            Point::new(x - hw, y - s * hh),
            Point::new(x + hw, y - s * hh),
            Point::new(x, y + s * hh),
        )
        .into_styled(PrimitiveStyle::with_fill(col))
        .draw(display)?;
        Ok(())
    };
    match ARROWS {
        1 => {
            let mx = (g.center.x + edge - 34)
                .min(nx + nw + uw + 44)
                .max(nx + nw + uw + 26);
            tri(display, mx, c.y - 12, true, 8, 6, Colors::Black)?;
            tri(display, mx, c.y + 12, false, 8, 6, Colors::Black)?;
        }
        2 => {
            let col = cm.palette().vario.ring_select;
            tri(display, c.x, c.y - BAR_H2 - 14, true, 11, 7, col)?;
            tri(display, c.x, c.y + BAR_H2 + 14, false, 11, 7, col)?;
        }
        _ => (),
    }
    Ok(())
}

/// wing loading (x10, kg/m2), best glide ratio and its speed (in the unit
/// of the display) with the masses that are set now
pub(crate) fn glider_info(cm: &CoreModel) -> Option<(i32, i32, i32)> {
    let gd = &cm.glider_data;
    let bgd = &gd.basic_glider_data;
    if bgd.wing_area <= 1.0 {
        return None;
    }
    let m = bgd.empty_mass + gd.pilot_weight.to_kg() + gd.water_ballast.to_kg();
    let wl10 = (m / bgd.wing_area * 10.0 + 0.5) as i32;
    let (ld, v) = crate::flight_physics::best_glide(gd)?;
    let v = cm
        .config
        .unit_horizontal_speed
        .as_f32(crate::Speed::from_km_h(v));
    Some((wl10, (ld + 0.5) as i32, (v + 0.5) as i32))
}

/// one line of grey text with white numbers, centred at (cx, y); `sup`
/// marks a part drawn small and raised ("2" of m2)
fn mixed_line<D>(
    display: &mut D,
    cx: i32,
    y: i32,
    parts: &[(&str, bool, bool)],
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    use crate::view::fonts_p1::text_for;
    // Barlow 18 with a raised 12 (was 14 / 11, Stefan 16.11.2026: larger)
    let f = text_for(22).f;
    let fs = text_for(16).f;
    let w = |font: &u8g2_fonts::FontRenderer, t: &str| {
        font.get_rendered_dimensions(t, Point::zero(), VerticalPosition::Baseline)
            .map(|d| d.advance.x)
            .unwrap_or(0)
    };
    let total: i32 = parts
        .iter()
        .map(|(t, _, sup)| if *sup { w(fs, t) } else { w(f, t) })
        .sum();
    let mut x = cx - total / 2;
    for (t, white, sup) in parts {
        let (font, dy) = if *sup { (fs, -10) } else { (f, 0) };
        let col = if *white {
            Colors::White
        } else {
            Colors::DarkGray
        };
        font.render_aligned(
            *t,
            Point::new(x, y + dy),
            VerticalPosition::Baseline,
            HorizontalAlignment::Left,
            FontColor::Transparent(col),
            display,
        )?;
        x += w(font, t);
    }
    Ok(())
}

/// "L 31.2 kg/m2" and "E 34 at 128 km/h" (wing loading, best glide) at the
/// baselines y1 / y2 - for ballast, pilot weight and bugs
pub(crate) fn draw_glider_info<D>(
    display: &mut D,
    cm: &CoreModel,
    cx: i32,
    y1: i32,
    y2: i32,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    if let Some((wl10, ld, v)) = glider_info(cm) {
        let wl = crate::tformat!(12, "{}.{}", wl10 / 10, wl10 % 10).unwrap_or_default();
        mixed_line(
            display,
            cx,
            y1,
            &[
                ("L ", false, false),
                (wl.as_str(), true, false),
                (" kg/m", false, false),
                ("2", false, true),
            ],
        )?;
        let lds = crate::tformat!(8, "{}", ld).unwrap_or_default();
        let vs = crate::tformat!(8, "{}", v).unwrap_or_default();
        let unit = cm.config.unit_horizontal_speed.as_str();
        let unit = if unit == "knots" { "kt" } else { unit };
        let u = crate::tformat!(12, " {}", unit).unwrap_or_default();
        mixed_line(
            display,
            cx,
            y2,
            &[
                ("E ", false, false),
                (lds.as_str(), true, false),
                (" at ", false, false),
                (vs.as_str(), true, false),
                (u.as_str(), false, false),
            ],
        )?;
    }
    Ok(())
}

/// the editables whose change moves wing loading and best glide
/// y of a help line (view/help.rs) relative to the centre: below the
/// neighbour option and the counter of a choice, right below the bar of a
/// number - and below the two glider info lines where a value has them
pub(crate) fn help_line_y(params: &Params, target: Editable, nr: i32) -> i32 {
    let y0 = match params {
        // (15.11.2026: 10 px higher and the lines closer - the display is
        // round, lower lines have less room)
        Params::Enum(_) | Params::List(_) => BAR_DY + BAR_H2 + 28 + 68,
        _ if shows_glider_info(target) => info_y(1) + 34,
        _ => info_y(0) + 22,
    };
    y0 + nr * HELP_LINE_PITCH
}
const HELP_LINE_PITCH: i32 = 27;

/// The help lines are set in the text face, Barlow 22 (Stefan 16.11.2026:
/// smaller, so the two lines stand clear of each other - they are read on
/// the ground and shall look good). Before: the small value font, whose
/// lines of 24 px almost touched at the pitch of 27.
pub(crate) fn help_font() -> &'static u8g2_fonts::FontRenderer {
    &crate::utils::larus_fonts::TEXT_22
}

/// width a help line may have at this y: the chord of the display circle
/// at the far edge of the line, with a margin
#[cfg(test)]
pub(crate) fn help_line_max_px(radius: i32, y: i32) -> i32 {
    let edge = y.abs() + 9;
    let half_sq = (radius * radius - edge * edge).max(0);
    let mut half = 0;
    while (half + 1) * (half + 1) <= half_sq {
        half += 1;
    }
    2 * half - 6
}

pub(crate) fn shows_glider_info(e: Editable) -> bool {
    matches!(
        e,
        Editable::WaterBallast | Editable::PilotWeight | Editable::Bugs
    )
}

/// the ring style editor - returns false if the caller should fall back
pub fn draw_editor<D>(
    display: &mut D,
    cm: &CoreModel,
    header: &str,
    value: &str,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    // a text: name entry with the alphabet in the ring
    if let Params::String(_) = cm.control.editor.params {
        if cm.config.overlay_active == crate::model::OverlayActive::Editor {
            return crate::view::name_entry::draw_name_entry(display, cm, header);
        }
    }
    let g = Geo::new(cm);
    let pal_needle = cm.palette().vario.ring_select;
    let background = cm.palette().vario.background;

    // background: full screen clears, the overlay covers the black core of
    // the page (no line around it)
    if g.full {
        display.clear(background)?;
    } else {
        crate::view::ring_menu::overlay_disc(display, cm)?;
    }

    // the header like in every menu: the dark blue cap; the values of a flap
    // stage say which stage they belong to ("Flap 4 - Up to")
    let mut head: heapless::String<24> = heapless::String::new();
    if matches!(
        cm.control.editor.target,
        Editable::FlapUpTo | Editable::FlapFrom | Editable::FlapPosition
    ) {
        let t = cm.flap_table();
        let i = (cm.config.flap_edit as usize).min(t.len().saturating_sub(1));
        let _ = head.push_str("Flap ");
        let _ = head.push_str(t.name(i));
        let _ = head.push_str(" - ");
    }
    let _ = head.push_str(header_name(header));
    crate::view::ring_menu::header(display, g.center, g.radius, g.cap(), head.as_str())?;

    let target = cm.control.editor.target;

    // info popups (e.g. ballast changed via CAN) are not an edit session:
    // show the plain value, the editor params may belong to something else
    if cm.config.overlay_active == crate::model::OverlayActive::Info {
        cm.device_const.value_font.render_aligned(
            value,
            g.center + Point::new(0, -4),
            VerticalPosition::Center,
            HorizontalAlignment::Center,
            FontColor::Transparent(pal_needle),
            display,
        )?;
        return Ok(());
    }

    match cm.control.editor.params {
        // ---------------- numbers ----------------
        Params::F32(p) => {
            // GNSS Config: the three product names in one size, the size the
            // longest one needs (Stefan 09.11.2026)
            let fit = if target == Editable::GnssConfig {
                "Competition Pro"
            } else {
                value
            };
            value_box(display, cm, &g, value, fit)?;
            // no gauge arc for the other numbers: the value alone (Stefan
            // 28.10.2026 - the arc read like a progress bar)
            let _ = p;
        }
        // ---------------- enums: one segment per option ----------------
        Params::Enum(p) => {
            let mut n = 0usize;
            let mut idx = 0usize;
            if let Content::Enum(val) = cm.control.editor.content {
                for i in 0..MAX_ENUM_VARIANTS {
                    if !p.variants[i].is_empty() {
                        n = i + 1;
                        if val.as_str() == p.variants[i] {
                            idx = i;
                        }
                    }
                }
            }
            if n > 1 {
                // The value goes on the display FIRST. Everything in this
                // function propagates errors with `?`, so a failing piece of
                // decoration would take the value down with it and leave
                // nothing but the header on the screen.
                if target == Editable::RemoteSave || target == Editable::RemoteRestart {
                    // dialog of the sensor box settings: item and old > new
                    // above the choice, the other option dimmed below it
                    let dy = g.row_dy();
                    cm.device_const.value_font.render_aligned(
                        value,
                        g.center + Point::new(0, 14),
                        VerticalPosition::Center,
                        HorizontalAlignment::Center,
                        FontColor::Transparent(Colors::White),
                        display,
                    )?;
                    let other = if idx == 0 {
                        p.variants[1]
                    } else {
                        p.variants[0]
                    };
                    cm.device_const.value_font_small.render_aligned(
                        other,
                        g.center + Point::new(0, dy + 14),
                        VerticalPosition::Center,
                        HorizontalAlignment::Center,
                        FontColor::Transparent(Colors::DimGray),
                        display,
                    )?;
                    if let Some(pd) = cm.control.editor.pending {
                        gray_line(display, cm, g.center, -dy - 6, pd.target.name())?;
                        let old = pd.target.content_as_str(pd.old, cm);
                        let new = pd.target.content_as_str(pd.new, cm);
                        let line = if target == Editable::RemoteSave {
                            crate::tformat!(44, "{} > {}", old.as_str().trim(), new.as_str().trim())
                        } else {
                            crate::tformat!(44, "{}: restart needed", new.as_str().trim())
                        };
                        if let Ok(txt) = line {
                            gray_line(display, cm, g.center, -dy + 24, txt.as_str())?;
                        }
                    }
                } else {
                    let prev = if idx > 0 { p.variants[idx - 1] } else { "" };
                    let next = if idx + 1 < n { p.variants[idx + 1] } else { "" };
                    let cnt = crate::tformat!(20, "{} / {}", idx + 1, n)
                        .unwrap_or_else(|_| heapless::String::new());
                    fisheye(display, cm, &g, prev, value, next, cnt.as_str())?;
                }
            } else {
                fisheye(display, cm, &g, "", value, "", "")?;
            }
        }
        // ---------------- lists: fisheye picker ----------------
        Params::List(p) => {
            let idx = if let Content::List(v) = cm.control.editor.content {
                v
            } else {
                0
            };
            let prev = if idx > 0 {
                target.content_as_str(Content::List(idx - 1), cm)
            } else {
                crate::utils::TString::<20>::new()
            };
            let next = if idx < p.max {
                target.content_as_str(Content::List(idx + 1), cm)
            } else {
                crate::utils::TString::<20>::new()
            };
            let cnt = crate::tformat!(20, "{} / {}", idx + 1, p.max + 1)
                .unwrap_or_else(|_| heapless::String::new());
            fisheye(
                display,
                cm,
                &g,
                prev.as_str(),
                value,
                next.as_str(),
                cnt.as_str(),
            )?;
        }
        // ---------------- pitot offset measurement ----------------
        Params::Cmd(_) if target == Editable::PitotOffsetMeasure => {
            use crate::controller::pitot_cal;
            use crate::model::PitotCalState;
            let cal = cm.control.pitot_cal;
            let note = pitot_cal::note(&cal);
            match cal.state {
                PitotCalState::Settle(_) | PitotCalState::Measure(_) | PitotCalState::Confirm => {
                    // the running mean of the raw pressure in the value bar,
                    // one decimal like the offset editor
                    let shown = crate::tformat!(20, "{:.1} Pa", cal.raw)
                        .unwrap_or_else(|_| heapless::String::new());
                    fisheye(display, cm, &g, "", shown.as_str(), "", "")?;
                    progress_line(display, cm, &g, info_y(0) + 4, pitot_cal::percent(&cal))?;
                    gray_line(display, cm, g.center, info_y(1) + 16, note)?;
                }
                PitotCalState::Prepare(_) => {
                    // waiting for the box: the busy segment runs to and fro
                    progress_line(display, cm, &g, info_y(0) + 4, None)?;
                    gray_line(display, cm, g.center, info_y(1) + 16, note)?;
                }
                PitotCalState::Failed(why) => {
                    // the reason, large and white where the value would be,
                    // what to do about it below
                    crate::view::fonts_p1::text_for(30).f.render_aligned(
                        note,
                        g.center + Point::new(0, BAR_DY),
                        VerticalPosition::Center,
                        HorizontalAlignment::Center,
                        FontColor::Transparent(Colors::White),
                        display,
                    )?;
                    let hint = match why {
                        crate::model::PitotCalError::NoSensorbox => "check the CAN cable",
                        crate::model::PitotCalError::NotOnGround => "cover the pitot",
                    };
                    gray_line(display, cm, g.center, info_y(0) + 10, hint)?;
                }
                PitotCalState::Idle => {
                    fisheye(display, cm, &g, "", value, "", "")?;
                }
            }
        }
        // ---------------- date, command, string ----------------
        _ => {
            fisheye(display, cm, &g, "", value, "", "")?;
        }
    }

    // ---------------- context sub lines ----------------
    if target == Editable::Qnh {
        let alt = cm.sensor.pressure_altitude.qnh_altitude();
        if let Ok(txt) = crate::tformat!(
            20,
            "{} {}",
            cm.config.unit_height.as_f32(alt) as i32,
            cm.config.unit_height.as_str()
        ) {
            gray_line(display, cm, g.center, info_y(0), txt.as_str())?;
        }
    }
    // GNSS: what the configuration number means and, for the antenna
    // geometry, the value in words - the names alone cannot carry the sign
    if let (Some(lines), true) = (
        gnss_lines(target, cm.control.editor.content),
        cm.control.editor.pending.is_none(),
    ) {
        for (nr, l) in lines.iter().enumerate() {
            if !l.is_empty() {
                gray_line(
                    display,
                    cm,
                    g.center,
                    info_y(0) + 6 + nr as i32 * 36,
                    l.as_str(),
                )?;
            }
        }
    }
    // MC: the glide ratio at the speed to fly for this MC in still air
    if target == Editable::McCready {
        if let Some((ld, v)) = crate::flight_physics::mc_glide(&cm.glider_data, cm.config.mc_cready)
        {
            let v = cm
                .config
                .unit_horizontal_speed
                .as_f32(crate::Speed::from_km_h(v));
            let lds = crate::tformat!(8, "{}", (ld + 0.5) as i32).unwrap_or_default();
            let vs = crate::tformat!(8, "{}", (v + 0.5) as i32).unwrap_or_default();
            let unit = cm.config.unit_horizontal_speed.as_str();
            let unit = if unit == "knots" { "kt" } else { unit };
            let u = crate::tformat!(12, " {}", unit).unwrap_or_default();
            mixed_line(
                display,
                g.center.x,
                g.center.y + info_y(0) + 8,
                &[
                    ("E ", false, false),
                    (lds.as_str(), true, false),
                    (" at ", false, false),
                    (vs.as_str(), true, false),
                    (u.as_str(), false, false),
                ],
            )?;
        }
    }
    // setup wizard: which step of how many
    if let Some((n, of)) = crate::controller::wizard::step(cm) {
        if let Ok(txt) = crate::tformat!(32, "Setup {} of {} - push: next", n, of) {
            gray_line(display, cm, g.center, g.cap() + 34, txt.as_str())?;
        }
    }
    // sound preview running: say so and which climb rate is heard
    if let Some(v) = cm.calculated.sound_preview {
        let speed = crate::FloatToSpeed::m_s(v);
        let s = cm.config.unit_vertical_speed.value_str(speed);
        let sign = if v >= 0.05 { "+" } else { "" };
        if let Ok(txt) = crate::tformat!(
            32,
            "Preview {}{} {}",
            sign,
            s.as_str(),
            cm.config.unit_vertical_speed.as_str()
        ) {
            gray_line(display, cm, g.center, g.cap() + 34, txt.as_str())?;
        }
    }
    // what the setting does (view/help.rs): two grey lines in the setup
    // editor, below the neighbour option and the counter of a choice, right
    // below the bar of a number. Not while a save / restart dialog asks.
    if g.full
        && cm.control.editor.pending.is_none()
        && gnss_lines(target, cm.control.editor.content).is_none()
    {
        if let Some(lines) = crate::view::help::help(target) {
            for (nr, l) in lines.iter().enumerate() {
                if !l.is_empty() {
                    help_font().render_aligned(
                        *l,
                        Point::new(
                            g.center.x,
                            g.center.y + help_line_y(&cm.control.editor.params, target, nr as i32),
                        ),
                        VerticalPosition::Center,
                        HorizontalAlignment::Center,
                        FontColor::Transparent(Colors::Gray),
                        display,
                    )?;
                }
            }
        }
    }
    if shows_glider_info(target) {
        draw_glider_info(
            display,
            cm,
            g.center.x,
            g.center.y + info_y(0) + 4,
            g.center.y + info_y(1) + 5,
        )?;
    }
    if target == Editable::WaterBallast {
        let bgd = &cm.glider_data.basic_glider_data;
        let m_dry = bgd.empty_mass + cm.glider_data.pilot_weight.to_kg();
        let m_act = m_dry + cm.glider_data.water_ballast.to_kg();
        if cm.config.stf_switch_mode == STF_SWITCH_MODE_BALLAST && m_dry > 1.0 {
            let v = cm.config.stf_switch_speed * crate::utils::math::sqrt(m_act / m_dry);
            if let Ok(txt) = crate::tformat!(
                20,
                "switch {} {}",
                cm.config.unit_horizontal_speed.as_f32(v) as i32,
                cm.config.unit_horizontal_speed.as_str()
            ) {
                gray_line(display, cm, g.center, info_y(2), txt.as_str())?;
            }
        }
    }
    Ok(())
}

/// The sub lines of the GNSS editors (Stefan 09.11.2026: "Ant Slave Down"
/// is understood by nobody). The box's secondary antenna is the front one,
/// its values are kept as they are - only explained.
fn gnss_lines(target: Editable, content: Content) -> Option<[heapless::String<28>; 2]> {
    let v = match content {
        Content::F32(Some(v)) => v,
        _ => return None,
    };
    let mut l: [heapless::String<28>; 2] = [heapless::String::new(), heapless::String::new()];
    let a = if v < 0.0 { -v } else { v };
    let same = a < 0.005;
    match target {
        Editable::GnssConfig => {
            let (x, y) = match (v + 0.5) as i32 {
                1 => ("1 antenna", "GNSS M9N"),
                2 => ("2 antennas", "D-GNSS 2x F9P"),
                3 => ("2 antennas", "D-GNSS F9P + F9H"),
                4 => ("2 antennas", "D-GNSS X20D"),
                _ => ("", ""),
            };
            let _ = l[0].push_str(x);
            let _ = l[1].push_str(y);
        }
        Editable::AntBaselen => {
            let _ = l[0].push_str("front to rear antenna");
            let _ = l[1].push_str("along the fuselage");
        }
        Editable::AntSlaveDown => {
            if same {
                let _ = l[0].push_str("both antennas");
                let _ = l[1].push_str("at the same height");
            } else {
                let _ = crate::tformat!(28, "front antenna {:.2} m", a).map(|t| l[0] = t);
                let _ = l[1].push_str(if v > 0.0 {
                    "lower than the rear"
                } else {
                    "higher than the rear"
                });
            }
        }
        Editable::AntSlaveRight => {
            if same {
                let _ = l[0].push_str("both antennas");
                let _ = l[1].push_str("on the centre line");
            } else {
                let _ = crate::tformat!(28, "front antenna {:.2} m", a).map(|t| l[0] = t);
                let _ = l[1].push_str(if v > 0.0 {
                    "right of the rear"
                } else {
                    "left of the rear"
                });
            }
        }
        _ => return None,
    }
    Some(l)
}

/// half period of the alarm blink in 100 ms ticks (1 Hz)
pub const ALARM_BLINK_TICKS: u32 = 5;
/// width of the alarm ring around the box
const ALARM_RING_W: i32 = 10;
/// the alarm bar: centre (relative to the display centre) and half height
const ALARM_BAR_DY: i32 = 22;
const ALARM_BAR_H2: i32 = 34;

/// Alarm box in the ring style (15.11.2026, landing gear): the black core
/// of the page with a thick ring and a bar in the alarm colour, in the bar
/// what to do, above it the symbol, below the reason and how to confirm.
/// It blinks between the alarm colour and a dark red - nothing ever
/// disappears, the text stays readable in both phases.
pub fn draw_alarm<D>(
    display: &mut D,
    cm: &CoreModel,
    title: &str,
    reason: &str,
    img: &'static [u8],
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    use embedded_graphics::primitives::Rectangle;
    let c = cm.device_const.sizes.display.center;
    let width = cm.device_const.sizes.display.width as i32;
    let r = crate::view::ring_menu::overlay_disc(display, cm)?;
    let bright = (cm.control.alive_ticks / ALARM_BLINK_TICKS) % 2 == 0;
    let alarm = cm.palette().signal.alarm;
    let (fill, ink) = if bright {
        (alarm, Colors::Black)
    } else {
        (Colors::DarkRed, Colors::White)
    };
    // ring at the rim of the core
    // (2 px inside the core, so it never touches the scale line)
    Circle::with_center(c, (2 * r - ALARM_RING_W - 3) as u32)
        .into_styled(PrimitiveStyle::with_stroke(fill, ALARM_RING_W as u32))
        .draw(display)?;
    // bar across the core, inside the ring
    let ri = r - ALARM_RING_W - 1;
    for dy in ALARM_BAR_DY - ALARM_BAR_H2..=ALARM_BAR_DY + ALARM_BAR_H2 {
        let h = crate::view::ring_menu::isqrt((ri * ri - dy * dy).max(0));
        let x0 = (c.x - h).max(0);
        let x1 = (c.x + h).min(width - 1);
        if x1 > x0 {
            display.fill_solid(
                &Rectangle::new(Point::new(x0, c.y + dy), Size::new((x1 - x0 + 1) as u32, 1)),
                fill,
            )?;
        }
    }
    cm.device_const.value_font.render_aligned(
        title,
        c + Point::new(0, ALARM_BAR_DY),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(ink),
        display,
    )?;
    // the symbol above the bar (steady)
    let alarm_img = crate::Image::new(img);
    let (iw, ih) = (alarm_img.width() as i32, alarm_img.height() as i32);
    let top = ALARM_BAR_DY - ALARM_BAR_H2;
    alarm_img.draw(display, c + Point::new(-iw / 2, top - 14 - ih), None)?;
    // the reason, and how it goes away
    cm.device_const.value_font_small.render_aligned(
        reason,
        c + Point::new(0, ALARM_BAR_DY + ALARM_BAR_H2 + 28),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(Colors::White),
        display,
    )?;
    crate::view::fonts_p1::text_for(22).f.render_aligned(
        "push = OK",
        c + Point::new(0, ALARM_BAR_DY + ALARM_BAR_H2 + 62),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(Colors::Gray),
        display,
    )?;
    Ok(())
}

/// straight progress line under the value bar (pitot offset measurement):
/// grey track, the filled part in the selection blue; without a percentage
/// a short segment runs to and fro (busy)
fn progress_line<D>(
    display: &mut D,
    cm: &CoreModel,
    g: &Geo,
    y: i32,
    percent: Option<u8>,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    use embedded_graphics::primitives::{Rectangle, RoundedRectangle};
    const H: i32 = 12;
    // as wide as the circle allows at that height, minus a margin
    let r = g.radius;
    let half = crate::view::ring_menu::isqrt((r * r - (y + H) * (y + H)).max(0)) - 40;
    let w_all = (2 * half).clamp(60, 300);
    let x0 = g.center.x - w_all / 2;
    let y0 = g.center.y + y;
    let blue = cm.palette().vario.ring_select;
    let mut bar = |x: i32, w: i32, col: Colors| -> Result<(), CoreError> {
        if w > 0 {
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(x, y0), Size::new(w as u32, H as u32)),
                Size::new(H as u32 / 2, H as u32 / 2),
            )
            .into_styled(PrimitiveStyle::with_fill(col))
            .draw(display)?;
        }
        Ok(())
    };
    bar(x0, w_all, Colors::Grey36)?;
    match percent {
        Some(pc) => {
            let pc = pc.min(100) as i32;
            if pc > 0 {
                bar(x0, (w_all * pc / 100).max(H), blue)?;
            }
        }
        None => {
            let seg = 60;
            let span = w_all - seg;
            let t = ((cm.control.alive_ticks * 8) % (2 * span as u32)) as i32;
            let x = if t < span { t } else { 2 * span - t };
            bar(x0 + x, seg, blue)?;
        }
    }
    Ok(())
}

/// progress of long running actions (firmware update, calibration): a
/// straight bar in the selection blue (no ring any more), the percentage
/// above it; without a percentage a short segment runs to and fro
pub fn draw_progress<D>(
    display: &mut D,
    cm: &CoreModel,
    header: &str,
    percent: Option<u8>,
    note: &str,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    use embedded_graphics::primitives::{Rectangle, RoundedRectangle};
    let g = Geo::new(cm);
    let blue = cm.palette().vario.ring_select;
    display.clear(cm.palette().vario.background)?;
    crate::view::ring_menu::header(
        display,
        g.center,
        240,
        crate::view::ring_menu::CAP_FULL,
        header,
    )?;
    const W: i32 = 300;
    const H: i32 = 14;
    let x0 = g.center.x - W / 2;
    let y0 = g.center.y + 24;
    let bar = |display: &mut D, x: i32, w: i32, col: Colors| -> Result<(), CoreError> {
        if w > 0 {
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(x, y0), Size::new(w as u32, H as u32)),
                Size::new(H as u32 / 2, H as u32 / 2),
            )
            .into_styled(PrimitiveStyle::with_fill(col))
            .draw(display)?;
        }
        Ok(())
    };
    bar(display, x0, W, Colors::Grey36)?;
    if let Some(pc) = percent {
        let pc = pc.min(100) as i32;
        bar(display, x0, (W * pc / 100).max(H), blue)?;
        if let Ok(txt) = crate::tformat!(20, "{} %", pc) {
            cm.device_const.value_font.render_aligned(
                txt.as_str(),
                g.center + Point::new(0, -20),
                VerticalPosition::Center,
                HorizontalAlignment::Center,
                FontColor::Transparent(Colors::White),
                display,
            )?;
        }
    } else {
        // no percentage available: a segment runs to and fro (busy)
        let seg = 70;
        let span = W - seg;
        let t = ((cm.control.alive_ticks * 8) % (2 * span as u32)) as i32;
        let x = if t < span { t } else { 2 * span - t };
        bar(display, x0 + x, seg, blue)?;
    }
    if !note.is_empty() {
        crate::view::fonts_p1::text_for(21).f.render_aligned(
            note,
            g.center + Point::new(0, g.sub_y(0) + 20),
            VerticalPosition::Center,
            HorizontalAlignment::Center,
            FontColor::Transparent(Colors::DarkGray),
            display,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::editable::{Content, EnumParams, F32Params, Params};
    use crate::model::OverlayActive;
    use crate::utils::TString;
    use crate::view::test_fixture::{test_model, TestDisplay};
    use crate::{DisplayActive, EditMode};

    /// the box around the centre of the display where the value line sits -
    /// narrow enough that neither the circle outline nor the status arc
    /// reaches into it
    const VAL_X0: i32 = 150;
    const VAL_Y0: i32 = 212;
    const VAL_X1: i32 = 330;
    const VAL_Y1: i32 = 268;

    fn render(target: Editable, params: Params, content: Content) -> (TestDisplay, usize) {
        let mut cm = test_model();
        cm.config.overlay_active = OverlayActive::Editor;
        cm.config.display_active = DisplayActive::Vario;
        cm.control.editor.target = target;
        cm.control.editor.params = params;
        cm.control.editor.content = content;
        cm.control.editor.mode = EditMode::CircleArea;
        let value = cm.control.editor.get_value_line(&cm);
        let mut display = TestDisplay::new();
        draw_editor(&mut display, &cm, target.name(), value.as_str())
            .expect("the editor must not fail to draw");
        let painted = display.painted_box(VAL_X0, VAL_Y0, VAL_X1, VAL_Y1);
        (display, painted)
    }

    /// An enum editor must put its value on the screen. The failure mode this
    /// guards against is a screen with nothing but the header on it.
    #[test]
    fn enum_editor_draws_its_value() {
        let params = Params::Enum(EnumParams {
            variants: ["Dark", "Light", "", "", ""],
        });
        let content = Content::Enum(TString::<16>::from_str("Light"));
        let (_d, painted) = render(Editable::Theme, params, content);
        assert!(painted > 100, "only {painted} ink pixels on the value line");
    }

    /// The same for a two option enum whose content does not match any
    /// variant - the value must still be readable.
    #[test]
    fn enum_editor_draws_unknown_value() {
        let params = Params::Enum(EnumParams {
            variants: ["Dark", "Light", "", "", ""],
        });
        let content = Content::Enum(TString::<16>::from_str("Other"));
        let (_d, painted) = render(Editable::Theme, params, content);
        assert!(painted > 100, "only {painted} ink pixels on the value line");
    }

    /// Control: the pixel count really measures the value line. With an
    /// empty value the band around the centre stays empty - that is exactly
    /// the "nothing but the header" picture the tests above rule out.
    #[test]
    fn empty_value_leaves_the_centre_empty() {
        let mut cm = test_model();
        cm.config.overlay_active = OverlayActive::Editor;
        cm.config.display_active = DisplayActive::Vario;
        cm.control.editor.target = Editable::Theme;
        cm.control.editor.params = Params::Enum(EnumParams {
            variants: ["Dark", "Light", "", "", ""],
        });
        // a content variant the params do not expect - content_as_str
        // returns an empty string for it
        cm.control.editor.content = Content::F32(Some(1.0));
        cm.control.editor.mode = EditMode::CircleArea;
        let value = cm.control.editor.get_value_line(&cm);
        assert!(value.as_str().is_empty());
        let mut display = TestDisplay::new();
        draw_editor(&mut display, &cm, "Outer Ring", value.as_str()).unwrap();
        assert_eq!(display.painted_box(VAL_X0, VAL_Y0, VAL_X1, VAL_Y1), 0);
    }

    /// The save dialog of the sensor box settings names the item and shows
    /// old > new below the choice - both sub lines carry ink.
    #[test]
    fn remote_save_dialog_shows_item_and_values() {
        use crate::controller::PendingRemote;
        let mut cm = test_model();
        cm.config.overlay_active = OverlayActive::Editor;
        cm.config.display_active = DisplayActive::Menu;
        cm.control.editor.target = Editable::RemoteSave;
        cm.control.editor.params = Editable::RemoteSave.params(&cm);
        cm.control.editor.content = Content::Enum(TString::<16>::from_str("Save"));
        cm.control.editor.mode = EditMode::Fullscreen;
        cm.control.editor.pending = Some(PendingRemote {
            target: Editable::GnssConfig,
            config_id: crate::CanConfigId::GnssConfig,
            old: Content::F32(Some(2.0)),
            new: Content::F32(Some(3.0)),
        });
        let value = cm.control.editor.get_value_line(&cm);
        assert_eq!(value.as_str(), "Save");
        let mut display = TestDisplay::new();
        draw_editor(
            &mut display,
            &cm,
            Editable::RemoteSave.name(),
            value.as_str(),
        )
        .unwrap();
        if let Ok(path) = std::env::var("RING_EDITOR_DUMP") {
            dump_ppm(&display, &path);
        }
        // full screen geometry: info lines at center - 64 and - 34
        let c = cm.device_const.sizes.display.center;
        let with = display.painted_box(c.x - 150, c.y - 80, c.x + 150, c.y - 20);
        // control: without pending value the info lines stay empty
        cm.control.editor.pending = None;
        let mut display = TestDisplay::new();
        draw_editor(
            &mut display,
            &cm,
            Editable::RemoteSave.name(),
            value.as_str(),
        )
        .unwrap();
        let without = display.painted_box(c.x - 150, c.y - 80, c.x + 150, c.y - 20);
        assert!(
            with > 200 && without == 0,
            "info lines: {with} vs {without} ink pixels"
        );
    }

    /// debugging aid: write the test display as a PPM image
    fn dump_ppm(display: &TestDisplay, path: &str) {
        use crate::view::test_fixture::{DISPLAY_HEIGHT, DISPLAY_WIDTH};
        use std::io::Write;
        let mut f = std::fs::File::create(path).unwrap();
        write!(f, "P6 {} {} 255\n", DISPLAY_WIDTH, DISPLAY_HEIGHT).unwrap();
        for p in display.buf.iter() {
            let rgb: [u8; 3] = match p {
                None | Some(Colors::Black) => [0, 0, 0],
                Some(Colors::White) => [255, 255, 255],
                Some(Colors::DarkGray) => [90, 90, 90],
                Some(Colors::DimGray) => [130, 130, 130],
                Some(_) => [255, 140, 0],
            };
            f.write_all(&rgb).unwrap();
        }
    }

    /// The pitot offset measurement shows the running mean in the value bar
    /// while it runs, the busy line while the box is asked and the reason
    /// in the middle when it failed. Dumps the pictures with RING_DUMP_DIR.
    #[test]
    fn pitot_measurement_editor_shows_progress_value_or_note() {
        use crate::model::{PitotCal, PitotCalError, PitotCalState};
        let dir = std::env::var("RING_DUMP_DIR").ok();
        let mut cm = test_model();
        cm.config.overlay_active = OverlayActive::Editor;
        cm.config.display_active = DisplayActive::Menu;
        cm.control.editor.target = Editable::PitotOffsetMeasure;
        cm.control.editor.params = Editable::PitotOffsetMeasure.params(&cm);
        cm.control.editor.content = Content::Command(TString::<16>::from_str(""));
        cm.control.editor.mode = EditMode::Fullscreen;
        let c = cm.device_const.sizes.display.center;
        let cases = [
            ("prepare", PitotCalState::Prepare(10), false),
            ("measure", PitotCalState::Measure(40), true),
            (
                "failed",
                PitotCalState::Failed(PitotCalError::NoSensorbox),
                true,
            ),
        ];
        for (tag, state, centre_ink) in cases {
            cm.control.pitot_cal = PitotCal {
                state,
                old_offset: -1.0,
                span: 1.0,
                shifted: true,
                sum: 0.0,
                raw: -6.3,
            };
            let mut display = TestDisplay::new();
            draw_editor(&mut display, &cm, "Measure Offset", "").unwrap();
            if let Some(d) = &dir {
                crate::view::test_fixture::dump_ppm(&display, &std::format!("{d}/pitot_{tag}.ppm"));
            }
            let painted = display.painted_box(VAL_X0, VAL_Y0, VAL_X1, VAL_Y1);
            assert!(
                (painted > 100) == centre_ink,
                "{tag}: {painted} ink pixels on the value line"
            );
            // below the bar: the progress line or the note says what is going on
            let below = display.painted_box(c.x - 150, c.y + 40, c.x + 150, c.y + 130);
            assert!(
                below > 100,
                "{tag}: only {below} ink pixels below the value"
            );
        }
    }

    #[test]
    fn f32_editor_draws_its_value() {
        let params = Params::F32(F32Params {
            min: 0.0,
            max: 5.0,
            small_inc: 0.1,
            big_inc: 0.5,
            dec_places: 1,
            unit: "m/s",
        });
        let content = Content::F32(Some(2.5));
        let (_d, painted) = render(Editable::McCready, params, content);
        assert!(painted > 100, "only {painted} ink pixels on the value line");
    }
}
