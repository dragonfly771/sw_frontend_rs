//! FLARM screens of the ring design (plan_flarm_warnung.md, sketch v7)
//!
//! - Warning in the core (warning view "Core"): the dark core r 192 is
//!   replaced, scale, needle, speed to fly and MC marker keep running
//!   outside. Values in the R2 style of the ring page (DIST on top, dALT
//!   below, white), the graphic in the middle band y +-106:
//!   - head-on: horizon blue / brown without a line, linear degree scale
//!     +-10 deg, target as a dot in the alarm colour with a simple black
//!     glider in front view
//!   - directional: top view with the own glider in the middle, range ring,
//!     dashed bearing line, target dot with the symbol, height tag
//!   - non-directional (transponder): dashed ring at the distance
//!   - obstacle: triangle symbol on the bearing
//! - Full screen (warning view "Full Screen", or a warning while a menu or
//!   another page covers the core): the same picture scaled to the display.
//! - Radar as a core page (`PageCore::Radar`): fisheye like the SteFly FLARM
//!   display 0.5.3, symbols 1:1 from its `ui.c` (style 3), selection with
//!   the small encoder (orange brackets), data block below.
//!
//! Colours only for the alarm (yellow / orange / red), the team mate
//! (turquoise), the own aircraft (magenta, as on the SteFly FLARM display prototype) and the
//! selection (orange, the colour of the knob actions).

#[allow(unused_imports)]
use crate::utils::FClamp;
use crate::view::raster::*;
use crate::{
    flarm::{
        demo::lookup_name,
        logic::{FlarmView, IdleKind, TargetInfo, ViewKind},
    },
    model::{
        flarm_cfg::WarnView, flarm_show::FlarmToast, CoreModel, DisplayActive, OverlayActive,
        PageCore, UnitHeight, UnitHorizontalSpeed, UnitVerticalSpeed,
    },
    tformat,
    utils::Colors,
    view::fonts_p1::{text_for, value_for, VALUE_16, VALUE_19, VALUE_24, VALUE_42},
    CoreError, DrawImage,
};
use embedded_graphics::{prelude::*, primitives::Rectangle};
use heapless::String;
#[allow(unused_imports)]
use micromath::F32Ext;
use u8g2_fonts::{
    types::{HorizontalAlignment, VerticalPosition},
    FontRenderer,
};

/// What the FLARM module puts into the core of the ring page
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CoreContent {
    Warning,
    Radar,
}

/// radius of the core the FLARM pictures replace (H1 / B3)
const CORE_R: f32 = 192.0;
/// half height of the middle band carrying the graphic
const BAND: f32 = 106.0;
/// head-on scale: pixels per degree (+-10 deg over the band)
const PPD: f32 = 16.5;

const WHITE: Colors = Colors::White;
const GREY: Colors = Colors::DarkGray;
const DGREY: Colors = Colors::DimGray;

// ------------------------------------------------------------ selection

pub fn is_warning(kind: ViewKind) -> bool {
    matches!(
        kind,
        ViewKind::Warn | ViewKind::HeadOn | ViewKind::WarnNonDir | ViewKind::Obstacle
    )
}

fn warning_shown(cm: &CoreModel) -> bool {
    is_warning(cm.calculated.flarm.view.kind) && cm.config.flarm.warn_view != WarnView::Off
}

/// Content of the core of the ring page, None = the page decides
pub fn core_content(cm: &CoreModel) -> Option<CoreContent> {
    if warning_shown(cm) && cm.config.flarm.warn_view == WarnView::Core {
        return Some(CoreContent::Warning);
    }
    if cm.config.page().core == PageCore::Radar {
        return Some(CoreContent::Radar);
    }
    None
}

/// The warning needs the whole display: warning view "Full Screen", or the
/// core of the ring page is covered (menu, editor, device info ...)
pub fn full_screen_needed(cm: &CoreModel) -> bool {
    if !warning_shown(cm) {
        return false;
    }
    if cm.config.flarm.warn_view == WarnView::FullScreen {
        return true;
    }
    cm.config.display_active != DisplayActive::Vario
        || cm.config.overlay_active != OverlayActive::None
}

// ------------------------------------------------------------ symbols
//
// Shapes 1:1 from the SteFly FLARM display prototype 0.5.3 (ui.c, ui_sym_style 3), all
// normalised to a span of 1.

const WING: [(f32, f32); 8] = [
    (-0.50, -0.06),
    (-0.08, -0.14),
    (0.08, -0.14),
    (0.50, -0.06),
    (0.50, 0.03),
    (0.08, 0.0),
    (-0.08, 0.0),
    (-0.50, 0.03),
];
const FUS: [(f32, f32); 7] = [
    (0.0, -0.36),
    (0.06, -0.27),
    (0.055, 0.05),
    (0.03, 0.42),
    (-0.03, 0.42),
    (-0.055, 0.05),
    (-0.06, -0.27),
];
const TAIL: [(f32, f32); 4] = [(-0.19, 0.34), (0.19, 0.34), (0.19, 0.42), (-0.19, 0.42)];
const P_WING: [(f32, f32); 4] = [(-0.50, -0.10), (0.50, -0.10), (0.50, 0.04), (-0.50, 0.04)];
const P_FUS: [(f32, f32); 7] = [
    (0.0, -0.40),
    (0.07, -0.33),
    (0.06, 0.10),
    (0.03, 0.42),
    (-0.03, 0.42),
    (-0.06, 0.10),
    (-0.07, -0.33),
];
const P_TAIL: [(f32, f32); 4] = [(-0.20, 0.32), (0.20, 0.32), (0.20, 0.42), (-0.20, 0.42)];
const P_PROP: [(f32, f32); 4] = [(-0.17, -0.47), (0.17, -0.47), (0.17, -0.42), (-0.17, -0.42)];
const OBST: [(f32, f32); 3] = [(0.0, -0.36), (0.32, 0.28), (-0.32, 0.28)];
const DIA: [(f32, f32); 4] = [(0.0, -0.30), (0.30, 0.0), (0.0, 0.30), (-0.30, 0.0)];
/// outline offsets (style 3)
const OFF: [(f32, f32); 8] = [
    (-1.5, 0.0),
    (1.5, 0.0),
    (0.0, -1.5),
    (0.0, 1.5),
    (-1.1, -1.1),
    (1.1, -1.1),
    (-1.1, 1.1),
    (1.1, 1.1),
];

#[derive(Clone, Copy, PartialEq)]
enum SymClass {
    Glider,
    Power,
    Obstacle,
    Unknown,
}

/// four classes like 0.5.3: glider | powered | slow / unknown | obstacle
fn sym_class(acft: u8, has_track: bool) -> SymClass {
    let acft = if acft == 0 { 1 } else { acft };
    match acft {
        1 if has_track => SymClass::Glider,
        2 | 3 | 5 | 8 | 9 if has_track => SymClass::Power,
        0xF => SymClass::Obstacle,
        _ => SymClass::Unknown,
    }
}

fn shape<D>(
    display: &mut D,
    g: &Geo,
    pts: &[(f32, f32)],
    cx: f32,
    cy: f32,
    scale: f32,
    ang_deg: f32,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let a = ang_deg.to_radians();
    let (s, c) = (a.sin(), a.cos());
    let mut q: heapless::Vec<(f32, f32), 8> = heapless::Vec::new();
    for &(x, y) in pts {
        let (x, y) = (x * scale, y * scale);
        let _ = q.push((cx + x * c - y * s, cy + x * s + y * c));
    }
    fill_poly(display, g, &q, col)
}

fn shapes_of<D>(
    display: &mut D,
    g: &Geo,
    class: SymClass,
    cx: f32,
    cy: f32,
    span: f32,
    ang: f32,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    match class {
        SymClass::Glider => {
            shape(display, g, &WING, cx, cy, span, ang, col)?;
            shape(display, g, &FUS, cx, cy, span, ang, col)?;
            shape(display, g, &TAIL, cx, cy, span, ang, col)
        }
        SymClass::Power => {
            let s = span * 0.85;
            shape(display, g, &P_WING, cx, cy, s, ang, col)?;
            shape(display, g, &P_FUS, cx, cy, s, ang, col)?;
            shape(display, g, &P_TAIL, cx, cy, s, ang, col)?;
            shape(display, g, &P_PROP, cx, cy, s, ang, col)
        }
        SymClass::Obstacle => shape(display, g, &OBST, cx, cy, span, 0.0, col),
        SymClass::Unknown => shape(display, g, &DIA, cx, cy, span * 1.1, 0.0, col),
    }
}

/// target symbol with dark outline (absolute position, span in pixels)
#[allow(clippy::too_many_arguments)]
fn symbol<D>(
    display: &mut D,
    g: &Geo,
    class: SymClass,
    cx: f32,
    cy: f32,
    span: f32,
    ang: f32,
    col: Colors,
    outline: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    if outline {
        for (ox, oy) in OFF {
            shapes_of(
                display,
                g,
                class,
                cx + ox * g.k,
                cy + oy * g.k,
                span,
                ang,
                Colors::Black,
            )?;
        }
    }
    shapes_of(display, g, class, cx, cy, span, ang, col)
}

fn target_class(t: &TargetInfo) -> SymClass {
    sym_class(t.acft_type, t.track_rel.is_some())
}

// ------------------------------------------------------------ formatting

pub fn level_color(level: u8) -> Colors {
    match level {
        3.. => Colors::FlarmL3,
        2 => Colors::FlarmL2,
        _ => Colors::FlarmL1,
    }
}

/// colour of a symbol / text on top of the alarm colour
fn on_level(level: u8) -> Colors {
    if level >= 3 {
        Colors::White
    } else {
        Colors::Black
    }
}

fn dist_str(cm: &CoreModel, m: f32) -> (String<12>, &'static str) {
    let (v, u) = match cm.config.unit_horizontal_speed {
        UnitHorizontalSpeed::Knots => (m / 1852.0, "NM"),
        UnitHorizontalSpeed::Mph => (m / 1609.34, "mi"),
        UnitHorizontalSpeed::Kmph => (m / 1000.0, "km"),
    };
    let s = if v < 1.0 {
        tformat!(12, "{:.2}", v)
    } else if v < 10.0 {
        tformat!(12, "{:.1}", v)
    } else {
        tformat!(12, "{:.0}", v)
    };
    (s.unwrap_or_default(), u)
}

fn signed(v: i32) -> String<12> {
    let r = if v > 0 {
        tformat!(12, "+{}", v)
    } else {
        tformat!(12, "{}", v)
    };
    r.unwrap_or_default()
}

fn alt_str(cm: &CoreModel, m: f32) -> (String<12>, &'static str) {
    match cm.config.unit_height {
        UnitHeight::Feet => (signed(round(m * 3.28084 / 10.0) * 10), "ft"),
        UnitHeight::Meter => (signed(round(m)), "m"),
    }
}

/// short height tag like the SteFly FLARM display prototype: tens of metres / hundreds of feet
fn alt_tag(cm: &CoreModel, m: f32) -> String<12> {
    let v = match cm.config.unit_height {
        UnitHeight::Feet => round(m * 3.28084 / 100.0),
        UnitHeight::Meter => round(m / 10.0),
    }
    .clamp(-99, 99);
    let r = if v < 0 {
        tformat!(12, "-{}{}", (-v) / 10, (-v) % 10)
    } else {
        tformat!(12, "+{}{}", v / 10, v % 10)
    };
    r.unwrap_or_default()
}

fn vario_str(cm: &CoreModel, ms: f32) -> (String<12>, &'static str) {
    match cm.config.unit_vertical_speed {
        UnitVerticalSpeed::Knots => {
            let v = ms * 1.94384;
            let s = if v >= 0.0 {
                tformat!(12, "+{:.1}", v)
            } else {
                tformat!(12, "{:.1}", v)
            };
            (s.unwrap_or_default(), "kt")
        }
        UnitVerticalSpeed::Fpm => (signed(round(ms * 196.85 / 10.0) * 10), "fpm"),
        UnitVerticalSpeed::Mps => {
            let s = if ms >= 0.0 {
                tformat!(12, "+{:.1}", ms)
            } else {
                tformat!(12, "{:.1}", ms)
            };
            (s.unwrap_or_default(), "m/s")
        }
    }
}

/// name of a target: contest sign of the demo / name list, else the ID
pub fn name_of(id: u32) -> String<8> {
    let mut s: String<8> = String::new();
    match lookup_name(id) {
        Some(n) => {
            for c in n.chars().take(8) {
                let _ = s.push(c);
            }
        }
        None => {
            let hex = b"0123456789ABCDEF";
            for k in (0..6).rev() {
                let _ = s.push(hex[((id >> (4 * k)) & 0xf) as usize] as char);
            }
        }
    }
    s
}

fn pick_range(d: f32) -> f32 {
    for r in [250.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0] {
        if d < r * 0.85 {
            return r;
        }
    }
    20000.0
}

fn range_str(cm: &CoreModel, m: f32) -> String<12> {
    let (v, u) = dist_str(cm, m);
    // "2km", "0.5km": trailing zeros away, like the SteFly FLARM display prototype
    let mut s: String<12> = String::new();
    let t = v.as_str();
    let t = if t.contains('.') {
        t.trim_end_matches('0').trim_end_matches('.')
    } else {
        t
    };
    let _ = s.push_str(t);
    let _ = s.push_str(u);
    s
}

// ------------------------------------------------------------ public drawing

/// Draw the FLARM content into the core of the ring page
pub fn draw_core<D>(display: &mut D, cm: &CoreModel, content: CoreContent) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let g = geo(cm, 1.0, (CORE_R - 0.5) as i32);
    // black core (the speed to fly arc starts at 192)
    disc(display, &g, g.cx, g.cy, CORE_R - 0.5, Colors::Black)?;
    match content {
        CoreContent::Warning => draw_warning(display, cm, &g, false),
        CoreContent::Radar => draw_radar(display, cm, &g),
    }
}

/// Draw the warning on the whole display (no scale)
pub fn draw_full_screen<D>(display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    display.clear(Colors::Black)?;
    let half = cm.device_const.sizes.display.width as f32 / 2.0;
    let g = geo(cm, half / CORE_R, half as i32);
    draw_warning(display, cm, &g, true)
}

// ------------------------------------------------------------ warning

fn draw_warning<D>(display: &mut D, cm: &CoreModel, g: &Geo, full: bool) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let v: &FlarmView = &cm.calculated.flarm.view;
    let level = v.level.max(1);
    let col = level_color(level);
    let band = g.with_band(-BAND, BAND);
    match v.kind {
        ViewKind::HeadOn => headon(display, cm, &band, v, level)?,
        ViewKind::WarnNonDir => nondir(display, cm, &band, v, level)?,
        _ => directional(display, cm, &band, v, level, v.kind == ViewKind::Obstacle)?,
    }
    // DIST on top, dALT below (R2 style, white), positions of sketch v7:
    // values sketch inb46 (u8g2 inb33), captions sketch fub16 (fub11); the
    // top block ends 6 px above the band, the bottom one starts 6 px below.
    // Full screen: the same picture x1.25, values u8g2 inb42, captions fub14.
    let (d, du) = dist_str(cm, v.threat.dist);
    let (a, au) = alt_str(cm, v.threat.rel_v);
    let (font_v, font_c): (&FontRenderer, &FontRenderer) = if full {
        (&VALUE_42, text_for(20).f)
    } else {
        (value_for(46).f, text_for(16).f)
    };
    let k = g.k;
    let dig = 0.74 * 46.0;
    let cap_off = 6.0 + 0.93 * 16.0;
    let top_base = g.cy + (-BAND - 6.0 - (dig + 6.0 + 16.0) + dig) * k;
    let top_caption = top_base + cap_off * k;
    let bot_base = g.cy + (BAND + 6.0 + dig) * k;
    let bot_caption = bot_base + cap_off * k;
    let cx = round(g.cx);
    let mut cap: String<16> = String::new();
    let _ = cap.push_str("DIST  ");
    let _ = cap.push_str(du);
    value_text(
        display,
        font_v,
        d.as_str(),
        Point::new(cx, round(top_base)),
        VerticalPosition::Baseline,
        HorizontalAlignment::Center,
        WHITE,
    )?;
    text(
        display,
        font_c,
        cap.as_str(),
        Point::new(cx, round(top_caption)),
        VerticalPosition::Baseline,
        HorizontalAlignment::Center,
        WHITE,
    )?;
    let mut cap: String<16> = String::new();
    let _ = cap.push_str("dALT  ");
    let _ = cap.push_str(au);
    value_text(
        display,
        font_v,
        a.as_str(),
        Point::new(cx, round(bot_base)),
        VerticalPosition::Baseline,
        HorizontalAlignment::Center,
        WHITE,
    )?;
    text(
        display,
        font_c,
        cap.as_str(),
        Point::new(cx, round(bot_caption)),
        VerticalPosition::Baseline,
        HorizontalAlignment::Center,
        WHITE,
    )?;
    let _ = col;
    Ok(())
}

/// round-ended tick like the main ticks of the outer scale
fn cap_tick<D>(display: &mut D, g: &Geo, x: f32, ln: f32, w: f32) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let k = g.k;
    let (x0, y0) = g.p(x, 0.0);
    let half = w * k / 2.0;
    let yb = y0 + (ln - w / 2.0) * k;
    fill_poly(
        display,
        g,
        &[
            (x0 - half, y0),
            (x0 + half, y0),
            (x0 + half, yb),
            (x0 - half, yb),
        ],
        WHITE,
    )?;
    disc(display, g, x0, yb, half, WHITE)
}

fn headon<D>(
    display: &mut D,
    _cm: &CoreModel,
    g: &Geo,
    v: &FlarmView,
    level: u8,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    // horizon K1: only blue and brown, no line
    let hy = round(g.cy);
    for y in g.y0..=g.y1 {
        if let Some((a, b)) = g.row(y) {
            span(
                display,
                g,
                y,
                a,
                b,
                if y < hy {
                    Colors::SkyK1
                } else {
                    Colors::EarthK1
                },
            )?;
        }
    }
    // linear degree scale below the horizon
    for a in 1..=10 {
        for s in [-1.0f32, 1.0] {
            let x = s * a as f32 * PPD;
            if a == 5 || a == 10 {
                cap_tick(display, g, x, 13.0, 5.0)?;
                let mut l: String<4> = String::new();
                let _ = tfmt::uwrite!(&mut l, "{}", a);
                let font = if g.k > 1.1 {
                    text_for(25).f
                } else {
                    text_for(22).f
                };
                // number centred on the tick, the degree sign as a small ring
                // behind it (the ASCII text font has none)
                let p = g.pt(x, 32.0);
                text(
                    display,
                    font,
                    l.as_str(),
                    p,
                    VerticalPosition::Center,
                    HorizontalAlignment::Center,
                    WHITE,
                )?;
                let w = text_w(font, l.as_str());
                let base = crate::view::fonts_p1::baseline_of(font, p.y, VerticalPosition::Center);
                let hh = font.get_ascent() as i32;
                crate::view::fonts_p1::degree_ring(
                    display,
                    p.x + w / 2 - 1,
                    10,
                    base,
                    hh - 1,
                    WHITE,
                )?;
            } else {
                line(
                    display,
                    g,
                    g.p(x, 0.0),
                    g.p(x, 6.0),
                    2.0 * g.k,
                    WHITE,
                    false,
                )?;
            }
        }
    }
    cap_tick(display, g, 0.0, 16.0, 6.0)?;
    // target: dot in the alarm colour with a simple black glider in front view
    let t = &v.threat;
    let tx = t.bearing.fclamp(-10.5, 10.5) * PPD;
    let ty = (-t.elev * PPD).fclamp(-BAND + 20.0, BAND - 20.0);
    let (px, py) = g.p(tx, ty);
    // steady, no blinking (Stefan 28.10.2026: not done in avionics)
    disc(display, g, px, py, 46.0 * g.k, level_color(level))?;
    front_glider(display, g, tx, ty + 8.0, 130.0, Colors::Black)
}

/// glider from the front, as simple as possible: wing with a slight V,
/// fuselage dot, fin and tailplane (T tail)
fn front_glider<D>(
    display: &mut D,
    g: &Geo,
    cx: f32,
    cy: f32,
    span: f32,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let h = span / 2.0;
    let w = (span * 0.045).max(4.0);
    let k = g.k;
    let tip_l = g.p(cx - h, cy - span * 0.04);
    let root = g.p(cx, cy);
    let tip_r = g.p(cx + h, cy - span * 0.04);
    line(display, g, tip_l, root, w * k, col, true)?;
    line(display, g, root, tip_r, w * k, col, true)?;
    let ft = span * 0.20;
    line(display, g, root, g.p(cx, cy - ft), w * 0.85 * k, col, false)?;
    line(
        display,
        g,
        g.p(cx - span * 0.13, cy - ft),
        g.p(cx + span * 0.13, cy - ft),
        w * 0.85 * k,
        col,
        true,
    )?;
    disc(display, g, root.0, root.1, w * 1.5 * k, col)
}

fn directional<D>(
    display: &mut D,
    cm: &CoreModel,
    g: &Geo,
    v: &FlarmView,
    level: u8,
    obstacle: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let k = g.k;
    let col = level_color(level);
    // range ring with a tick every 30 degrees
    let rr = 90.0;
    ring(
        display,
        g,
        g.cx,
        g.cy,
        (rr - 1.0) * k,
        (rr + 1.0) * k,
        DGREY,
    )?;
    for i in 0..12 {
        let a = (i as f32 * 30.0).to_radians();
        let l = if i % 3 == 0 { 10.0 } else { 6.0 };
        let (s, c) = (a.sin(), a.cos());
        line(
            display,
            g,
            g.p(s * (rr - l), -c * (rr - l)),
            g.p(s * rr, -c * rr),
            2.0 * k,
            if i % 3 == 0 { GREY } else { DGREY },
            false,
        )?;
    }
    let t = &v.threat;
    let range = pick_range(t.dist);
    let font_l = if k > 1.1 {
        text_for(22).f
    } else {
        text_for(18).f
    };
    text(
        display,
        font_l,
        range_str(cm, range).as_str(),
        g.pt(rr + 24.0, 74.0),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        GREY,
    )?;
    // never on top of the own glider: the dot keeps clear of the middle
    let r = (t.dist / range * rr).fclamp(58.0, rr * 0.86);
    let a = t.bearing.to_radians();
    let tx = a.sin() * r;
    let ty = -a.cos() * r;
    // dashed bearing line
    let n = 7;
    for i in 0..n {
        let f0 = (i as f32 + 0.2) / n as f32;
        let f1 = (i as f32 + 0.65) / n as f32;
        line(
            display,
            g,
            g.p(tx * f0, ty * f0),
            g.p(tx * f1, ty * f1),
            3.0 * k,
            col,
            false,
        )?;
    }
    // own glider
    let (ox, oy) = g.p(0.0, 0.0);
    symbol(
        display,
        g,
        SymClass::Glider,
        ox,
        oy,
        40.0 * k,
        0.0,
        Colors::FlarmOwn,
        true,
    )?;
    // target
    let (px, py) = g.p(tx, ty);
    let class = if obstacle {
        SymClass::Obstacle
    } else {
        target_class(t)
    };
    // steady dot, no blinking and no height tag at the dot (Stefan
    // 28.10.2026): the height difference stands large in the dALT block
    disc(display, g, px, py, 35.0 * k, col)?;
    symbol(
        display,
        g,
        class,
        px,
        py,
        52.0 * k,
        t.track_rel.unwrap_or(0.0),
        on_level(level),
        false,
    )
}

fn dashed_circle<D>(
    display: &mut D,
    g: &Geo,
    cx: f32,
    cy: f32,
    r: f32,
    w: f32,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let n = 28;
    for i in 0..n {
        let a0 = i as f32 * core::f32::consts::TAU / n as f32;
        let a1 = a0 + core::f32::consts::TAU / n as f32 * 0.55;
        line(
            display,
            g,
            (cx + a0.sin() * r, cy - a0.cos() * r),
            (cx + a1.sin() * r, cy - a1.cos() * r),
            w,
            col,
            false,
        )?;
    }
    Ok(())
}

fn nondir<D>(
    display: &mut D,
    cm: &CoreModel,
    g: &Geo,
    v: &FlarmView,
    level: u8,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let k = g.k;
    let col = level_color(level);
    let rr = 90.0;
    ring(
        display,
        g,
        g.cx,
        g.cy,
        (rr - 1.0) * k,
        (rr + 1.0) * k,
        DGREY,
    )?;
    let t = &v.threat;
    let range = pick_range(t.dist);
    let r = (t.dist / range * rr).fclamp(28.0, rr * 0.9);
    dashed_circle(display, g, g.cx, g.cy, r * k, 4.0 * k, col)?;
    symbol(
        display,
        g,
        SymClass::Glider,
        g.cx,
        g.cy,
        40.0 * k,
        0.0,
        Colors::FlarmOwn,
        true,
    )?;
    let font_l = if k > 1.1 {
        text_for(22).f
    } else {
        text_for(18).f
    };
    text(
        display,
        font_l,
        range_str(cm, range).as_str(),
        g.pt(rr + 24.0, 74.0),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        GREY,
    )?;
    // no height tag at the ring (dALT block, like the directional warning)
    text(
        display,
        font_l,
        "XPDR",
        g.pt(0.0, -rr + 22.0),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        col,
    )
}

// ------------------------------------------------------------ radar

/// radar geometry in reference pixels (relative to the display centre)
const RADAR_OY: f32 = 12.0;
const RADAR_R: f32 = 172.0;
/// share of the radius of the linear inner part (inner ring at r 110)
const FISH_IN: f32 = 110.0 / 172.0;
const FISH_MAX: f32 = 10_000.0;
/// upper edge of the data block
const DATA_Y: f32 = 66.0;
const SYM_OWN: f32 = 36.0;
const SYM_MAIN: f32 = 50.0;
const SYM_NEAR: f32 = 44.0;
const SYM_FAR: f32 = 32.0;
const RADAR_MAX_SYM: usize = 10;

struct Radar {
    range: f32,
}

impl Radar {
    /// distance in reference pixels from the own aircraft (fisheye)
    fn rmap(&self, dist: f32) -> f32 {
        let rl = self.range;
        if dist <= rl {
            return FISH_IN * RADAR_R * dist / rl;
        }
        let d = dist.min(FISH_MAX);
        let kk = if rl < FISH_MAX {
            (1.0 + (d - rl) / rl).ln() / (1.0 + (FISH_MAX - rl) / rl).ln()
        } else {
            1.0
        };
        FISH_IN * RADAR_R + (1.0 - FISH_IN) * RADAR_R * kk
    }

    /// target position (reference pixels, relative to the display centre)
    /// and whether it was pushed to the edge of the visible area
    fn target_at(&self, bearing: f32, dist: f32) -> (f32, f32, bool) {
        let mut d = self.rmap(dist).min(RADAR_R - 6.0).max(20.0);
        let a = bearing.to_radians();
        let (dx, dy) = (a.sin(), -a.cos());
        let full = d;
        // visible area: above the data block, inside the core
        let y1 = DATA_Y - 14.0 - RADAR_OY;
        if dy > 0.01 && dy * d > y1 {
            d = y1 / dy;
        }
        let x = dx * d;
        let y = RADAR_OY + dy * d;
        // keep the symbol inside the core circle
        let r = isqrt_f(x * x + y * y);
        let lim = CORE_R - 16.0;
        let (x, y) = if r > lim {
            (x * lim / r, y * lim / r)
        } else {
            (x, y)
        };
        (x, y, d < full - 0.5)
    }
}

fn draw_radar<D>(display: &mut D, cm: &CoreModel, g: &Geo) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let show = &cm.calculated.flarm;
    let v = &show.view;
    let k = g.k;
    let threat = if is_warning(v.kind) && v.kind != ViewKind::WarnNonDir && v.threat.valid {
        Some(v.threat)
    } else {
        None
    };
    // focus target: manual selection, else team mate, else nearest
    let sel: Option<TargetInfo> = match v.idle {
        IdleKind::Selected => v.selected,
        IdleKind::Team => v.team.or(v.selected),
        _ => v.selected,
    };
    let sel_col = |t: &TargetInfo| -> Colors {
        if t.is_selected || v.sel_manual && !t.is_team {
            Colors::Orange
        } else if t.is_team {
            Colors::FlarmTeam
        } else {
            WHITE
        }
    };

    // range of the linear part: three nearest, team mate up to 2 km, threat
    let mut d3 = [f32::MAX; 3];
    for o in v.others.iter().take(v.n_others) {
        let mut m = 0;
        for i in 1..3 {
            if d3[i] > d3[m] {
                m = i;
            }
        }
        if o.dist < d3[m] {
            d3[m] = o.dist;
        }
    }
    let mut dl: f32 = 150.0;
    if threat.is_none() {
        for d in d3 {
            if d < f32::MAX && d > dl {
                dl = d;
            }
        }
    }
    if let Some(s) = sel {
        if s.dist < 2000.0 && s.dist > dl {
            dl = s.dist;
        }
    }
    if let Some(t) = threat {
        if t.dist > dl {
            dl = t.dist;
        }
    }
    let rad = Radar {
        range: pick_range(dl),
    };

    let top = g.with_band(-CORE_R, DATA_Y - 1.0);
    let (ocx, ocy) = g.p(0.0, RADAR_OY);
    // rings: inner = end of the linear part (labelled), outer without number
    ring(
        display,
        &top,
        ocx,
        ocy,
        (FISH_IN * RADAR_R - 1.0) * k,
        (FISH_IN * RADAR_R + 1.0) * k,
        Colors::Grey80,
    )?;
    ring(
        display,
        &top,
        ocx,
        ocy,
        (RADAR_R - 1.0) * k,
        (RADAR_R + 1.0) * k,
        Colors::Grey64,
    )?;

    // non directional targets: dashed circle at their distance
    for nd in v.nondir.iter().take(v.n_nondir) {
        let r = rad.rmap(nd.dist).fclamp(20.0, RADAR_R - 3.0) * k;
        let c = if nd.alarm > 0 {
            level_color(nd.alarm)
        } else {
            Colors::Gainsboro
        };
        dashed_circle(display, &top, ocx, ocy, r, 3.0 * k, c)?;
    }

    // status without data
    match v.kind {
        ViewKind::Startup | ViewKind::NoFlarm => {
            let msg = if show.demo { "Demo" } else { "No FLARM" };
            text_boxed(
                display,
                &top,
                text_for(22).f,
                msg,
                g.pt(0.0, -70.0),
                HorizontalAlignment::Center,
                GREY,
            )?;
        }
        ViewKind::NoGps => {
            text_boxed(
                display,
                &top,
                text_for(22).f,
                "FLARM: no GPS",
                g.pt(0.0, -70.0),
                HorizontalAlignment::Center,
                Colors::FlarmL1,
            )?;
        }
        _ => (),
    }

    // other targets: large symbols, only the nearest and only as many as fit
    // without overlap (logic of ui.c). Priority: own, threat, focus, team, distance.
    let mut occ: heapless::Vec<(f32, f32, f32), 20> = heapless::Vec::new();
    let _ = occ.push((0.0, RADAR_OY, SYM_OWN * 0.5));
    if let Some(t) = threat {
        let (x, y, _) = rad.target_at(t.bearing, t.dist);
        let _ = occ.push((x, y, SYM_MAIN * 0.5));
        let _ = occ.push((x + if x > 120.0 { -38.0 } else { 38.0 }, y + 10.0, 18.0));
    }
    if let Some(s) = sel {
        let (x, y, _) = rad.target_at(s.bearing, s.dist);
        let _ = occ.push((x, y, SYM_MAIN * 0.5));
        let _ = occ.push((x + if x > 110.0 { -44.0 } else { 44.0 }, y, 22.0));
    }
    let same =
        |a: &TargetInfo, b: &Option<TargetInfo>| b.is_some_and(|b| b.id == a.id && a.id != 0);
    let mut ord: heapless::Vec<usize, 24> = heapless::Vec::new();
    for (i, o) in v.others.iter().enumerate().take(v.n_others) {
        if same(o, &sel) || same(o, &threat) {
            continue;
        }
        let _ = ord.push(i);
    }
    let key = |o: &TargetInfo| if o.is_team { -1.0 } else { o.dist };
    for i in 1..ord.len() {
        let mut j = i;
        while j > 0 && key(&v.others[ord[j - 1]]) > key(&v.others[ord[j]]) {
            ord.swap(j - 1, j);
            j -= 1;
        }
    }
    let mut acc: heapless::Vec<(usize, f32, f32, f32, bool), RADAR_MAX_SYM> = heapless::Vec::new();
    for &i in ord.iter() {
        if acc.is_full() {
            break;
        }
        let o = &v.others[i];
        let rr = rad.rmap(o.dist);
        if rr > RADAR_R - 3.0 {
            continue;
        }
        let (x, y, edge) = rad.target_at(o.bearing, o.dist);
        let outer = rr > FISH_IN * RADAR_R;
        let av = o.rel_v.abs();
        let spn = if outer || edge {
            SYM_FAR
        } else {
            SYM_NEAR - if av > 500.0 { 8.0 } else { av * 8.0 / 500.0 }
        };
        let mut free = true;
        if !o.is_team {
            for &(qx, qy, qr) in occ.iter() {
                let (dx, dy) = (x - qx, y - qy);
                let md = (qr + spn * 0.5) * 0.8;
                if dx * dx + dy * dy < md * md {
                    free = false;
                    break;
                }
            }
        }
        if !free {
            continue;
        }
        let _ = acc.push((i, x, y, spn, edge));
        let _ = occ.push((x, y, spn * 0.5));
    }
    // farther ones first, the nearest lie on top
    for &(i, x, y, spn, edge) in acc.iter().rev() {
        let o = &v.others[i];
        let c = if o.is_team {
            Colors::FlarmTeam
        } else if edge {
            GREY
        } else {
            WHITE
        };
        let (px, py) = g.p(x, y);
        symbol(
            display,
            &top,
            target_class(o),
            px,
            py,
            spn * k,
            o.track_rel.unwrap_or(0.0),
            c,
            true,
        )?;
        if o.is_team && !edge {
            let lx = if x > 120.0 { x - 30.0 } else { x + 26.0 };
            let al = if x > 120.0 {
                HorizontalAlignment::Right
            } else {
                HorizontalAlignment::Left
            };
            text_boxed(
                display,
                &top,
                text_for(18).f,
                name_of(o.id).as_str(),
                g.pt(lx, y - 12.0),
                al,
                Colors::FlarmTeam,
            )?;
        }
    }

    // own aircraft
    symbol(
        display,
        &top,
        SymClass::Glider,
        ocx,
        ocy,
        SYM_OWN * k,
        0.0,
        Colors::FlarmOwn,
        true,
    )?;

    // focus target with name and height tag, selection brackets
    if let Some(s) = sel {
        if !same(&s, &threat) {
            let c = sel_col(&s);
            let (x, y, _) = rad.target_at(s.bearing, s.dist);
            let (px, py) = g.p(x, y);
            if s.lost {
                let unc = (rad.rmap(s.dist + s.age_s * 30.0) - rad.rmap(s.dist)).min(RADAR_R);
                ring(display, &top, px, py, (unc + 9.0) * k, (unc + 10.5) * k, c)?;
                text(
                    display,
                    text_for(25).f,
                    "?",
                    Point::new(round(px), round(py)),
                    VerticalPosition::Center,
                    HorizontalAlignment::Center,
                    c,
                )?;
            } else {
                symbol(
                    display,
                    &top,
                    target_class(&s),
                    px,
                    py,
                    SYM_MAIN * k,
                    s.track_rel.unwrap_or(0.0),
                    c,
                    true,
                )?;
            }
            if s.is_selected || (v.sel_manual && !s.is_team) {
                brackets(display, &top, px, py, 33.0 * k, Colors::Orange)?;
            }
            label(display, cm, &top, g, x, y, &s, c)?;
        }
    }

    // threat in the alarm colour
    if let Some(t) = threat {
        let c = level_color(t.alarm_level.max(1));
        let (x, y, _) = rad.target_at(t.bearing, t.dist);
        let (px, py) = g.p(x, y);
        symbol(
            display,
            &top,
            target_class(&t),
            px,
            py,
            SYM_MAIN * k,
            t.track_rel.unwrap_or(0.0),
            c,
            true,
        )?;
        let tag = alt_tag(cm, t.rel_v);
        let lx = if x > 120.0 { x - 30.0 } else { x + 30.0 };
        let al = if x > 120.0 {
            HorizontalAlignment::Right
        } else {
            HorizontalAlignment::Left
        };
        value_boxed(
            display,
            &top,
            &VALUE_19,
            tag.as_str(),
            g.pt(lx, y + 18.0),
            al,
            c,
        )?;
    }

    // range at the inner ring, upper left (only with traffic data)
    let rl = FISH_IN * RADAR_R * 0.71;
    if matches!(
        v.kind,
        ViewKind::Startup | ViewKind::NoFlarm | ViewKind::NoGps
    ) {
        return toast(display, cm, g);
    }
    text_boxed(
        display,
        &top,
        text_for(18).f,
        range_str(cm, rad.range).as_str(),
        g.pt(-rl, RADAR_OY - rl),
        HorizontalAlignment::Center,
        GREY,
    )?;

    // data block below a grey line
    data_block(display, cm, g, sel.or(threat), &sel_col)?;

    // toast after an encoder action
    toast(display, cm, g)
}

/// orange corner brackets around the selected target
fn brackets<D>(
    display: &mut D,
    g: &Geo,
    cx: f32,
    cy: f32,
    h: f32,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let l = h * 0.45;
    let w = 3.0 * g.k;
    for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let x = cx + sx * h;
        let y = cy + sy * h;
        line(display, g, (x, y), (x - sx * l, y), w, col, false)?;
        line(display, g, (x, y), (x, y - sy * l), w, col, false)?;
    }
    Ok(())
}

/// name and height tag next to a target (reference coordinates)
#[allow(clippy::too_many_arguments)]
fn label<D>(
    display: &mut D,
    cm: &CoreModel,
    clip: &Geo,
    g: &Geo,
    x: f32,
    y: f32,
    t: &TargetInfo,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let right = x > 110.0;
    let lx = if right { x - 40.0 } else { x + 40.0 };
    let al = if right {
        HorizontalAlignment::Right
    } else {
        HorizontalAlignment::Left
    };
    let y = if y > DATA_Y - 40.0 { y - 26.0 } else { y };
    text_boxed(
        display,
        clip,
        text_for(18).f,
        name_of(t.id).as_str(),
        g.pt(lx, y - 11.0),
        al,
        col,
    )?;
    let tag: String<12> = if t.lost {
        let a = (t.age_s as i32).min(5999);
        if a < 60 {
            tformat!(12, "{}s", a).unwrap_or_default()
        } else {
            tformat!(12, "{}:{}{}", a / 60, (a % 60) / 10, a % 10).unwrap_or_default()
        }
    } else {
        alt_tag(cm, t.rel_v)
    };
    value_boxed(
        display,
        clip,
        &VALUE_16,
        tag.as_str(),
        g.pt(lx, y + 11.0),
        al,
        col,
    )
}

fn data_block<D, F>(
    display: &mut D,
    cm: &CoreModel,
    g: &Geo,
    t: Option<TargetInfo>,
    sel_col: &F,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
    F: Fn(&TargetInfo) -> Colors,
{
    let k = g.k;
    let yb = DATA_Y;
    let xb = isqrt_f(CORE_R * CORE_R - yb * yb) - 6.0;
    line(display, g, g.p(-xb, yb), g.p(xb, yb), 2.0 * k, DGREY, false)?;
    let t = match t {
        Some(t) => t,
        None => {
            let msg = if cm.calculated.flarm.view.kind == ViewKind::Idle {
                "no traffic"
            } else {
                ""
            };
            return text(
                display,
                text_for(18).f,
                msg,
                g.pt(0.0, yb + 40.0),
                VerticalPosition::Center,
                HorizontalAlignment::Center,
                GREY,
            );
        }
    };
    let c = if t.alarm_level > 0 && !t.is_team && !t.is_selected {
        level_color(t.alarm_level)
    } else {
        sel_col(&t)
    };
    // data block of the sketch: call sign fub20 (u8g2 fub14) at yb + 16,
    // values inb34 (u8g2 inb24), units fub16 (fub11)
    text(
        display,
        text_for(18).f,
        name_of(t.id).as_str(),
        g.pt(0.0, yb + 16.0),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        c,
    )?;
    let (d, du) = dist_str(cm, t.dist);
    let (a, au) = alt_str(cm, t.rel_v);
    let base = yb + 30.0 + 36.0 * 0.74;
    let mut cols: heapless::Vec<(f32, String<12>, &'static str), 3> = heapless::Vec::new();
    // three columns (team mate with its climb) in inb27, otherwise two in inb33
    let font = match (t.is_team, t.climb) {
        (true, Some(cl)) => {
            let (vv, vu) = vario_str(cm, cl);
            let _ = cols.push((-102.0, d, du));
            let _ = cols.push((0.0, a, au));
            let _ = cols.push((102.0, vv, vu));
            &VALUE_24
        }
        _ => {
            let _ = cols.push((-60.0, d, du));
            let _ = cols.push((60.0, a, au));
            &VALUE_24
        }
    };
    for (x, v, u) in cols.iter() {
        value_text(
            display,
            font,
            v.as_str(),
            g.pt(*x, base),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            WHITE,
        )?;
        text(
            display,
            text_for(16).f,
            u,
            g.pt(*x, base + 8.0 + 0.93 * 16.0),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            GREY,
        )?;
    }
    Ok(())
}

fn toast<D>(display: &mut D, cm: &CoreModel, g: &Geo) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let show = &cm.calculated.flarm;
    if show.toast_ticks == 0 {
        return Ok(());
    }
    let mut s: String<24> = String::new();
    match show.toast {
        FlarmToast::None => return Ok(()),
        FlarmToast::Following => {
            let _ = s.push_str("Following ");
            let _ = s.push_str(name_of(show.toast_id).as_str());
        }
        FlarmToast::NotFollowing => {
            let _ = s.push_str("Not following");
        }
        FlarmToast::SelectionLost => {
            let _ = s.push_str("Selection lost");
        }
    }
    let p = g.pt(0.0, -140.0);
    let w = text_w(text_for(18).f, s.as_str());
    let r = Rectangle::new(
        Point::new(p.x - w / 2 - 10, p.y - 15),
        Size::new((w + 20) as u32, 30),
    );
    display.fill_solid(&r, Colors::Black)?;
    let (x0, x1) = (
        r.top_left.x as f32,
        (r.top_left.x + r.size.width as i32 - 1) as f32,
    );
    line(
        display,
        g,
        (x0, (p.y - 15) as f32),
        (x1, (p.y - 15) as f32),
        2.0,
        Colors::Orange,
        false,
    )?;
    line(
        display,
        g,
        (x0, (p.y + 14) as f32),
        (x1, (p.y + 14) as f32),
        2.0,
        Colors::Orange,
        false,
    )?;
    text(
        display,
        text_for(18).f,
        s.as_str(),
        p,
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        WHITE,
    )
}
