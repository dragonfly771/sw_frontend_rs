//! Ring design of the vario page (Stefly design)
//!
//! Full 360 degree scale: zero at the 9 o'clock position, positive climb
//! rates run clockwise over the top, negative ones counterclockwise over
//! the bottom, plus and minus full scale meet at the 3 o'clock position.
//! The info content is shown in stacked boxes (caption / value / unit)
//! inside the scale ring.

use super::{
    flap_tape::{draw_flap_tape, TapeState},
    fonts_p1,
    ring_menu::isqrt,
    sprites::*,
    thermal_data::ThermalData,
    viewable::ring_infobox::{draw_big_boxes, r2_caption, r2_value, BoxAlign},
};
use crate::{
    model::page_layout::Zone,
    model::{
        AvgMarker, CoreModel, DataSource, FlyMode, OverlayActive, PageCore, SystemState,
        UnitVerticalSpeed, VarioMode,
    },
    utils::Colors,
    view::page_layout::{box_frame, empty_slot, HL_FILL, HL_LINE},
    view::viewable::centerview::{AidPlace, CenterView},
    CoreError, DrawImage, Image,
};

use embedded_graphics::{
    geometry::AngleUnit,
    prelude::*,
    primitives::{Arc, Circle, PrimitiveStyle},
};
use num::clamp;

use crate::LineView;
#[allow(unused_imports)]
use micromath::F32Ext;
use u8g2_fonts::{
    types::{FontColor, HorizontalAlignment, VerticalPosition},
    FontRenderer,
};

/// Radius of the free middle on a Big Boxes page, in pixels of the 480x480
/// reference design. It is the outer ring of the small horizon - the ring
/// itself is not drawn here, the needle just stops there.
const RING_FREE: f32 = 150.0;

/// Radius that covers the small centre circle of the scale wallpaper
/// (it sits at 75..80 in the reference design).
const CENTER_CIRCLE: f32 = 84.0;

/// Light outer band: radius of the dark core on a page with info boxes.
///
/// Ring design B3 (27.10.2026, larger core): the scale ring of the
/// wallpaper sits at r = 202 (ink 201..203), the speed to fly arc runs from
/// 192 to 200 (`stf_diameter` 392, `stf_width` 8). At 192 the arc lands in
/// the light channel between core and ring. 192 is 80 % of the display
/// radius; until 0.4.1.33 it was 168 (70 %), the core area grew by 31 %.
/// The info boxes keep their positions for now - top and bottom reach out
/// to about 167, the right column aligns to the chord of this radius.
const CORE_BOXES: f32 = 192.0;

/// Light outer band: radius of the dark core when the middle is a disc
/// (small horizon, big boxes). Leaves a thin dark frame around it.
const CORE_DISC: f32 = 156.0;

/// Colours of the ring page that depend on the outer band
struct Skin {
    /// background outside the core
    outer: Colors,
    /// colour the scale wallpaper is drawn in
    scale: Colors,
    /// dark core radius, 0 = no separate core (everything dark)
    core: i32,
    /// outline for the markers, needed only on the light band
    outline: Option<Colors>,
    warning: Colors,
}

impl Skin {
    fn of(cm: &CoreModel, scl: f32) -> Self {
        // design P1: the dark core is r 192 on every page (horizon H1 and
        // the big values page included); the former disc remains for the
        // old horizon (HORIZON_K1 = false)
        let core_r = match cm.config.page().core {
            PageCore::Horizon if !crate::view::HORIZON_K1 => CORE_DISC,
            _ => CORE_BOXES,
        };
        if cm.config.page().light_outer {
            Skin {
                outer: Colors::White,
                scale: Colors::Black,
                core: (core_r * scl) as i32,
                outline: Some(Colors::Black),
                // yellow would vanish on white
                warning: Colors::DarkGoldenrod,
            }
        } else {
            Skin {
                outer: cm.palette().vario.background,
                scale: cm.palette().vario.scale,
                core: 0,
                outline: None,
                warning: cm.palette().signal.warning,
            }
        }
    }
    fn light(&self) -> bool {
        self.core > 0
    }
}

/// Centre circle of the scale wallpaper, measured from the image: its ink
/// runs from r = 75.2 to r = 78.6 in the reference design.
pub(crate) const CENTER_RING_OUT: f32 = 78.0;
pub(crate) const CENTER_RING_IN: f32 = 75.0;

/// Fill a ring (or a disc, with `r_in` = 0) with integer arithmetic.
///
/// A pixel belongs to the ring exactly when `r_in^2 <= dx^2 + dy^2 <=
/// r_out^2`. That is the mathematically exact discrete ring - there is no
/// square root anywhere. On the target `f32::sqrt` is micromath's fast bit
/// trick and off by more than one percent, which on a visible colour edge
/// shows up as dents. Both half widths are stepped from row to row instead,
/// so the whole fill costs O(radius) comparisons.
fn fill_ring<D>(
    display: &mut D,
    cm: &CoreModel,
    center: Point,
    r_out: i32,
    r_in: i32,
    color: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let width = cm.device_const.sizes.display.width as i32;
    let height = cm.device_const.sizes.display.height as i32;
    let ro2 = r_out * r_out;
    let ri2 = r_in * r_in;
    let y_min = (center.y - r_out).max(0);
    let y_max = (center.y + r_out).min(height - 1);
    let mut h_out: i32 = 0;
    let mut h_in: i32 = 0;
    // at most two spans per row: left and right of the inner circle
    let mut spans: [(i32, i32); 2];
    for y in y_min..=y_max {
        let dy = y - center.y;
        let d2o = ro2 - dy * dy;
        if d2o < 0 {
            continue;
        }
        while (h_out + 1) * (h_out + 1) <= d2o {
            h_out += 1;
        }
        while h_out > 0 && h_out * h_out > d2o {
            h_out -= 1;
        }
        let d2i = ri2 - dy * dy;
        if r_in > 0 && d2i > 0 {
            // largest half width that is STRICTLY inside the inner circle -
            // a pixel exactly on it belongs to the ring and has to stay
            while (h_in + 1) * (h_in + 1) < d2i {
                h_in += 1;
            }
            while h_in > 0 && h_in * h_in >= d2i {
                h_in -= 1;
            }
            // the row is cut in two by the inner circle
            spans = [
                (center.x - h_out, center.x - h_in - 1),
                (center.x + h_in + 1, center.x + h_out),
            ];
        } else {
            h_in = 0;
            spans = [(center.x - h_out, center.x + h_out), (0, -1)];
        }
        for (from, to) in spans {
            let x0 = from.max(0);
            let x1 = to.min(width - 1);
            if x1 < x0 {
                continue;
            }
            // Safety: coordinates are clamped to the display above
            unsafe {
                display.draw_line_unchecked(
                    y as usize * width as usize + x0 as usize,
                    (x1 - x0 + 1) as usize,
                    color,
                );
            }
        }
    }
    Ok(())
}

pub fn draw_ring<D>(
    display: &mut D,
    cm: &CoreModel,
    cm_1s: &CoreModel,
    thermal_data: &mut ThermalData,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let sizes = &cm.device_const.sizes.vario;
    let d_sizes = &cm.device_const.sizes.display;

    // scale factor of the 480x480 reference design
    let scl = (d_sizes.width.min(d_sizes.height)) as f32 / 480.0;

    // Beyond full scale the needle swings on into the negative half; then
    // the wallpaper with the continued numbers (6..9 resp. 12..18) is shown,
    // so the reading stays unambiguous. Switching back is held for a
    // moment (see InterpolatedClimbRate) so the numbers do not flicker.
    let overrange = cm.calculated.interpolated_climb_rate.overrange;
    let img_background = match (cm.config.unit_vertical_speed, overrange) {
        (UnitVerticalSpeed::Mps, false) => Image::new(cm.device_const.images.wp_vario360_5),
        (UnitVerticalSpeed::Mps, true) => Image::new(cm.device_const.images.wp_vario360_5x),
        (_, false) => Image::new(cm.device_const.images.wp_vario360_10),
        (_, true) => Image::new(cm.device_const.images.wp_vario360_10x),
    };

    let skin = Skin::of(cm, scl);

    // Clear the display and draw the full circle scale. With a light outer
    // band the display is cleared white first and the dark core is filled
    // afterwards, so the visible colour edge is an exact circle.
    // page layout editor: the zone that is chosen is highlighted in blue
    let hl = crate::controller::page_layout::highlight(cm);
    let hl_core = hl == Some(Zone::Core) && cm.config.page().core.with_values();
    if hl == Some(Zone::Scale) {
        // the outer band blue, the scale keeps its colour
        display.clear(if skin.light() { HL_LINE } else { HL_FILL })?;
        fill_ring(
            display,
            cm,
            d_sizes.center,
            (CORE_BOXES * scl) as i32,
            0,
            cm.palette().vario.background,
        )?;
    } else {
        display.clear(skin.outer)?;
        if skin.light() {
            fill_ring(
                display,
                cm,
                d_sizes.center,
                skin.core,
                0,
                cm.palette().vario.background,
            )?;
        }
    }
    if hl_core {
        fill_ring(
            display,
            cm,
            d_sizes.center,
            (CORE_BOXES * scl) as i32,
            0,
            HL_FILL,
        )?;
    }
    img_background.draw(display, Point::new(0, 0), Some(skin.scale))?;

    // The scale line grows outwards by 2 px and the ticks get wider - first
    // for the light band (black on white looks thinner), then on the dark
    // ring as well (Stefan 28.10.2026)
    thicken_scale(display, cm, scl, skin.scale)?;

    // The wallpaper is one image in one colour. On the light band that
    // colour is dark, so its small centre circle would disappear in the
    // dark core. It is redrawn here in the core's foreground colour, as an
    // exact ring with the same radii the image has.
    let large_aid = centering_large(cm);
    if skin.light()
        && !large_aid
        && !matches!(
            cm.config.page().core,
            PageCore::BigBoxes | PageCore::TwoValues | PageCore::Horizon
        )
    {
        fill_ring(
            display,
            cm,
            d_sizes.center,
            (CENTER_RING_OUT * scl) as i32,
            (CENTER_RING_IN * scl) as i32,
            cm.palette().vario.scale,
        )?;
    }

    // The scale wallpaper carries a small centre circle, the anchor of the
    // needle and of the centre views. On a Big Boxes page the middle
    // belongs to the boxes, so it is painted over with the background. The
    // same on the centering page while its large centering aid is shown.
    if cm.config.page().core.big() || large_aid {
        let bg = if hl_core {
            HL_FILL
        } else {
            cm.palette().vario.background
        };
        Circle::with_center(d_sizes.center, (2.0 * CENTER_CIRCLE * scl) as u32)
            .into_styled(PrimitiveStyle::with_fill(bg))
            .draw(display)?;
    }

    // draw battery symbol - only when it needs attention (orange/red);
    // a good battery (green) shows nothing on the ring page. Position (device
    // constants): in the white outer band between "4" and "3" at the lower
    // right, r 226, clear of the scale ticks (sketch skizze_status_symbole.png,
    // variant A); visible on every page core.
    if cm_1s.device.supply_voltage <= cm.config.battery_good {
        if cm_1s.device.supply_voltage < cm.config.battery_low {
            display.draw_img(
                cm.device_const.images.bat_empty_small,
                sizes.ring_bat_pos,
                Some(cm.palette().signal.stop),
            )?;
        } else {
            display.draw_img(
                cm.device_const.images.bat_half_small,
                sizes.ring_bat_pos,
                Some(skin.warning),
            )?;
        }
    }

    // draw sat symbol - only when something is wrong (orange/red);
    // CAN and GPS ok (green) shows nothing on the ring page
    match cm.control.system_state {
        SystemState::NoCom => {
            display.draw_img(
                cm.device_const.images.sat_small,
                sizes.ring_sat_pos,
                Some(cm.palette().signal.stop),
            )?;
        }
        SystemState::CanOk => {
            display.draw_img(
                cm.device_const.images.sat_small,
                sizes.ring_sat_pos,
                Some(skin.warning),
            )?;
        }
        SystemState::CanAndGpsOk => {}
    }

    // to save computing power: Only draw the central elements when they are visible.
    // (no attention triangle on the ring page - by design)
    if cm.config.overlay_active == OverlayActive::None {
        // The core of the page decides what happens inside the ring. With
        // Horizon, Thermal Assist or Wind the info boxes stay away, so the
        // content has the whole inner circle for itself.
        let page_core = cm.config.page().core;
        thermal_data.update(cm);
        // FLARM warning (warning view "Core") or the radar page own the core
        let flarm_core = crate::view::flarm_pages::core_content(cm);
        if let Some(content) = flarm_core {
            crate::view::flarm_pages::draw_core(display, cm, content)?;
        }
        // with FLARM content in the core nothing else is drawn inside
        let core = if flarm_core.is_some() {
            PageCore::Radar
        } else {
            page_core
        };
        // The centering page (Stefan 16.11.2026): while circling the large
        // centering aid with one box above and one below it; in straight
        // flight it is the info boxes page, with its cruise set.
        let large_aid = core == PageCore::ThermalAssist && large_aid;
        let core = if core == PageCore::ThermalAssist && !large_aid {
            PageCore::Boxes
        } else {
            core
        };
        // flap tape on the pages with info boxes (not on horizon, radar ...)
        // ... unless the page has it switched off (the editor still shows
        // it while its zone is chosen)
        // ... and not while circling: which stage to circle with differs
        // from pilot to pilot, and the page gains the room (no flap display
        // needed while circling)
        let tape_on =
            (!cm.config.page().flap_tape_off && !circling_view(cm)) || hl == Some(Zone::FlapTape);
        let tape = if (core.with_values() || large_aid) && tape_on {
            TapeState::of(cm)
        } else {
            None
        };
        // the editor shows its HUD in the centre circle instead
        let layout = crate::controller::page_layout::active(cm);
        match core {
            // FLARM radar / warning: drawn above
            PageCore::Radar => (),
            // the horizon is drawn at the very end, over the needle
            PageCore::Horizon => (),
            // the three large boxes own the whole middle
            PageCore::BigBoxes | PageCore::TwoValues => (),
            PageCore::ThermalAssist => {
                let place = centering_place(cm);
                // The first 1.5 circles of a thermal: the track, whatever
                // aid the page has (Stefan 16.11.2026) - the aids need a
                // full circle, the track shows the way in. In the page
                // editor the aid that is being chosen.
                let view = if thermal_data.trail.first_circles()
                    && !crate::controller::page_layout::active(cm)
                {
                    CenterView::ThermalAssistant4
                } else {
                    cm.config.page().centering_view()
                };
                view.draw_at(display, cm, thermal_data, &place)?;
            }
            PageCore::Wind => {
                match cm.control.fly_mode {
                    FlyMode::Circling => {
                        CenterView::DoubleArrowCircling.draw(display, cm, thermal_data)
                    }
                    FlyMode::StraightFlight => {
                        CenterView::DoubleArrowStraight.draw(display, cm, thermal_data)
                    }
                }?;
            }
            // the editor: its HUD covers the centre, except when the centre
            // is the zone - then it shows the set that is edited
            _ if layout => {
                if hl == Some(Zone::Centre) {
                    let page = cm.config.page();
                    let view = if cm.control.layout.cruise {
                        page.center_straight
                    } else {
                        page.center_circling
                    };
                    view.draw(display, cm, thermal_data)?;
                }
            }
            _ => {
                // standard core: the configured center view plus info boxes
                match cm.control.fly_mode {
                    FlyMode::Circling => {
                        cm.config
                            .page()
                            .center_circling
                            .draw(display, cm, thermal_data)
                    }
                    FlyMode::StraightFlight => {
                        cm.config
                            .page()
                            .center_straight
                            .draw(display, cm, thermal_data)
                    }
                }?;
            }
        }

        let with_boxes = core == PageCore::Boxes;
        // the editor shows the box set it edits (circling / cruise)
        let vario_mode = if layout {
            !cm.control.layout.cruise
        } else {
            cm.control.vario_mode == VarioMode::Vario
        };
        // the centering page: the large view shows the circling set, the
        // info boxes view the cruise set - whatever the vario shows
        let box_mode = if page_core == PageCore::ThermalAssist {
            large_aid
        } else {
            vario_mode
        };
        if with_boxes {
            draw_boxes_p1(display, cm, cm_1s, tape.is_some(), box_mode)?;
        }
        if large_aid {
            draw_top_bottom_p1(display, cm, cm_1s, box_mode, true)?;
        }

        // Big Boxes core: three large boxes stacked in the middle. The
        // needle stops at the (invisible) inner ring, so the whole circle
        // of radius RING_FREE belongs to them.
        if core == PageCore::TwoValues {
            draw_two_values(display, cm, cm_1s, tape.is_some(), vario_mode)?;
        }
        if core == PageCore::BigBoxes {
            if BIG_VALUES_P1 {
                draw_big_values(display, cm, cm_1s, tape.is_some(), vario_mode)?;
            } else {
                draw_big_boxes(
                    display,
                    cm,
                    cm_1s,
                    d_sizes.center,
                    (RING_FREE * scl) as i32,
                    vario_mode,
                )?;
            }
        }

        // speed to fly arc - with the flap tape it ends at +/-3.9, where the
        // tape begins (plan_ring_design_p1.md chapter 3)
        // (in the page editor: when the cruise set is edited)
        if !vario_mode {
            let lim = if tape.is_some() { 3.9 } else { 5.0 };
            let stf = num::clamp(-cm.calculated.speed_to_fly_dif.to_km_h() / 10.0, -lim, lim);
            let angle_sweep = (sizes.ring_scale_factor * stf).deg();
            let col = cm.palette().vario.stf_arc;
            // on the light band orange alone has barely any contrast
            if let Some(oc) = skin.outline {
                Arc::with_center(d_sizes.center, sizes.stf_diameter, 180.0.deg(), angle_sweep)
                    .into_styled(PrimitiveStyle::with_stroke(oc, sizes.stf_width + 4))
                    .draw(display)?;
            }
            Arc::with_center(d_sizes.center, sizes.stf_diameter, 180.0.deg(), angle_sweep)
                .into_styled(PrimitiveStyle::with_stroke(col, sizes.stf_width))
                .draw(display)?;
        }

        // flap tape, then the scale line once more over it, so the line keeps
        // its width where the tape touches it
        if let Some(st) = &tape {
            draw_flap_tape(display, cm, st, skin.light())?;
            fill_ring(
                display,
                cm,
                d_sizes.center,
                (205.0 * scl) as i32,
                (200.0 * scl) as i32,
                skin.scale,
            )?;
        }
    }

    // Calculate the angles for the analog pointers. The needle follows the
    // smoothed, quantized value (entry optimized correction included).
    let needle_climb = cm.calculated.interpolated_climb_rate.shown;
    let avg_climb_rate = match cm.control.avg_climb_rate_src {
        DataSource::Frontend => cm.calculated.av2_climb_rate,
        DataSource::Sensorbox => cm.sensor.average_climb_rate,
    };
    // marker source: normal average or the value of the POT box - the very
    // same chain (POT while circling, pre-flight prognosis in straight
    // flight, otherwise the average), so the triangle always sits where the
    // POT box says. Flight test 16.09.: the marker skipped the prognosis
    // step and showed the average while the box showed magenta.
    let avg_climb_rate = match cm.config.ring_avg_marker {
        AvgMarker::Average => avg_climb_rate,
        AvgMarker::Potential => crate::view::viewable::ring_infobox::pot_box_value(cm),
    };

    // Needle overrange: on the full circle scale the needle keeps swinging
    // past the positive full scale mark (3 o'clock) into the negative half,
    // up to just short of a full circle (9.9 m/s, 19.8 kt, 1980 fpm). It
    // stops a hair BEFORE the zero position so a pegged needle can never be
    // mistaken for zero. Average marker and MC dot stay within full scale.
    let (mc_cready_angle, avg_climb_angle, climb_rate_angle) = match cm.config.unit_vertical_speed {
        UnitVerticalSpeed::Fpm => {
            let av_climb_rate = clamp(avg_climb_rate.to_ft_min(), -1000.0, 1000.0);
            let climb_rate = num::clamp(needle_climb.to_ft_min(), -1000.0, 1980.0);
            let scale_factor = sizes.ring_scale_factor / 200.0;
            (
                (cm.config.mc_cready.to_ft_min() * scale_factor).to_radians(),
                (av_climb_rate * scale_factor).to_radians(),
                (climb_rate * scale_factor).to_radians(),
            )
        }
        UnitVerticalSpeed::Knots => {
            let av_climb_rate = clamp(avg_climb_rate.to_kt(), -10.0, 10.0);
            let climb_rate = num::clamp(needle_climb.to_kt(), -10.0, 19.8);
            let scale_factor = sizes.ring_scale_factor / 2.0;
            (
                (cm.config.mc_cready.to_kt() * scale_factor).to_radians(),
                (av_climb_rate * scale_factor).to_radians(),
                (climb_rate * scale_factor).to_radians(),
            )
        }
        UnitVerticalSpeed::Mps => {
            let av_climb_rate = clamp(avg_climb_rate.to_m_s(), -5.0, 5.0);
            let climb_rate = num::clamp(needle_climb.to_m_s(), -5.0, 9.9);
            (
                (cm.config.mc_cready.to_m_s() * sizes.ring_scale_factor).to_radians(),
                (av_climb_rate * sizes.ring_scale_factor).to_radians(),
                (climb_rate * sizes.ring_scale_factor).to_radians(),
            )
        }
    };

    // MC dot (Stefly design P1): centred ON the scale line (r 202),
    // radius 10 so it touches the dark core, in the colour of the scale
    let a = pos::NINE_O_CLOCK + mc_cready_angle;
    let mc_center = Point::new(
        d_sizes.center.x + (a.sin() * 202.0 * scl) as i32,
        d_sizes.center.y + (-a.cos() * 202.0 * scl) as i32,
    );
    // in the blue of the pilot-set values (Stefan 28.10. / 15.11.2026)
    Circle::with_center(mc_center, (20.0 * scl) as u32 + 1)
        .into_styled(PrimitiveStyle::with_fill(cm.palette().vario.ring_mc_cready))
        .draw(display)?;

    // draw average climb rate marker (long side beyond the display edge, tip
    // inwards), without a black outline (Stefan 28.10.2026)
    RingMarker::new(scl, d_sizes.center)
        .zero_pos(pos::NINE_O_CLOCK)
        .rotate(avg_climb_angle)
        .draw_colored(cm.palette().vario.ring_avg_climb, display)?;

    // Draw climb rate needle (thin, from center circle to the ticks). On a
    // Big Boxes page it ends at the inner ring instead, so the middle of
    // the display stays free for the three large boxes.
    let mut needle = RingNeedle::new(scl, d_sizes.center);
    needle.zero_pos(pos::NINE_O_CLOCK).rotate(climb_rate_angle);
    // no black outline: it showed as a dark rim on the white outer band
    // something large in the middle: the needle runs almost to the display
    // edge, so its short visible part still reads well
    let flarm_core = crate::view::flarm_pages::core_content(cm);
    let page_core = cm.config.page().core;
    if page_core.big() {
        // big values page: the needle starts at r 130, just before the values
        needle.inner_end(if BIG_VALUES_P1 {
            BIG_VALUES_NEEDLE
        } else {
            RING_FREE
        });
    }
    // large centering aid: the needle stays outside of it and of the two boxes
    let large_aid = flarm_core.is_none() && centering_large(cm);
    if large_aid {
        needle.inner_end(CENTERING_NEEDLE);
    }
    // FLARM picture or horizon in the core: the needle only runs on the
    // scale outside the core. Otherwise it would show through where the
    // core is not painted over (the black band with the level tube below
    // the horizon) - and the short needle costs less drawing time.
    if flarm_core.is_some() || page_core == PageCore::Horizon {
        needle.inner_end(CORE_BOXES);
    }
    if flarm_core.is_some()
        || matches!(
            page_core,
            PageCore::BigBoxes | PageCore::TwoValues | PageCore::Horizon | PageCore::Radar
        )
        || large_aid
    {
        needle.tip(crate::view::sprites::ring_needle::R_TIP_LONG);
    }
    needle.draw_colored(cm.palette().vario.ring_needle, display)?;

    // Horizon core: drawn last; the needle starts outside the core (see above)
    if cm.config.overlay_active == OverlayActive::None
        && cm.config.page().core == PageCore::Horizon
        && flarm_core.is_none()
    {
        if crate::view::HORIZON_K1 {
            // H1: the horizon fills the core r 192, no rim
            crate::view::horizon_k1::draw_horizon_k1(display, cm, CORE_BOXES * scl, false)?;
        } else {
            let inner_radius = (d_sizes.height as i32) * 5 / 16;
            crate::view::mini_horizon::draw_mini_horizon(
                display,
                cm,
                d_sizes.center,
                inner_radius,
            )?;
        }
    }
    // gear alarm confirmed, gear still up with the airbrakes open: a small
    // reminder at the lower left, the mirror image of the satellite symbol
    if cm.control.gear_warning && cm.control.gear_alarm_ack {
        draw_gear_reminder(display, cm, sizes.ring_sat_pos, d_sizes.center)?;
    }
    // no data: say it in words in the middle of the page (the small symbol
    // in the band alone tells a new pilot nothing)
    if cm.config.overlay_active == OverlayActive::None
        && !crate::controller::page_layout::active(cm)
    {
        match cm.control.system_state {
            // the first seconds after power on are no fault; a push takes
            // the words away, the flight demo needs no sensor box
            SystemState::NoCom if no_box_banner(cm) => {
                draw_banner(
                    display,
                    d_sizes.center,
                    "NO SENSOR BOX",
                    "check CAN cable",
                    cm.palette().signal.stop,
                )?;
            }
            // without a fix the vario works: only name the symbol
            SystemState::CanOk => {
                // on black, readable over the horizon as well (this font
                // draws no background of its own)
                let font = crate::view::fonts_p1::text_for(15).f;
                let at = sizes.ring_sat_pos + NO_GPS_TEXT_OFFSET;
                let w = crate::view::raster::text_w(font, NO_GPS_TEXT);
                embedded_graphics::primitives::Rectangle::new(
                    Point::new(at.x - w - 3, at.y - 9),
                    Size::new(w as u32 + 6, 18),
                )
                .into_styled(PrimitiveStyle::with_fill(Colors::Black))
                .draw(display)?;
                font.render_aligned(
                    NO_GPS_TEXT,
                    at,
                    VerticalPosition::Center,
                    HorizontalAlignment::Right,
                    FontColor::Transparent(skin.warning),
                    display,
                )?;
            }
            _ => {}
        }
    }
    // mode symbol, drawn last (over the horizon): circling ring = vario
    // mode, arrow down right = speed to fly mode
    if cm.config.overlay_active == OverlayActive::None
        && flarm_core.is_none()
        && !crate::controller::page_layout::active(cm)
    {
        let place = mode_icon_place(cm.config.page().core);
        if let Some((off, small)) = place {
            // test (Stefan 15.11.2026): in the orange of the needle
            crate::view::volume_panel::mode_icon(
                display,
                d_sizes.center + off,
                cm.palette().vario.ring_needle,
                cm.control.vario_mode == crate::VarioMode::SpeedToFly,
                small,
            )?;
        }
    }
    Ok(())
}

/// words of the gear reminder (after the alarm was confirmed)
pub const GEAR_REMINDER_TEXT: &str = "GEAR UP";

/// Small wheel in the outer band at the lower left and "GEAR UP" inside the
/// ring above it - where the satellite symbol and "NO GPS" are on the right.
fn draw_gear_reminder<D>(
    display: &mut D,
    cm: &CoreModel,
    sat_pos: Point,
    c: Point,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    let alarm = cm.palette().signal.alarm;
    // centre of the satellite symbol (20 px), mirrored at the vertical axis
    let wheel = Point::new(2 * c.x - (sat_pos.x + 10), sat_pos.y + 10);
    Circle::with_center(wheel, 19)
        .into_styled(PrimitiveStyle::with_stroke(alarm, 4))
        .draw(display)?;
    Circle::with_center(wheel, 5)
        .into_styled(PrimitiveStyle::with_fill(alarm))
        .draw(display)?;
    let font = crate::view::fonts_p1::text_for(15).f;
    let at = Point::new(
        2 * c.x - (sat_pos.x + NO_GPS_TEXT_OFFSET.x),
        sat_pos.y + NO_GPS_TEXT_OFFSET.y,
    );
    let w = crate::view::raster::text_w(font, GEAR_REMINDER_TEXT);
    embedded_graphics::primitives::Rectangle::new(
        Point::new(at.x - 3, at.y - 9),
        Size::new(w as u32 + 6, 18),
    )
    .into_styled(PrimitiveStyle::with_fill(Colors::Black))
    .draw(display)?;
    font.render_aligned(
        GEAR_REMINDER_TEXT,
        at,
        VerticalPosition::Center,
        HorizontalAlignment::Left,
        FontColor::Transparent(alarm),
        display,
    )?;
    Ok(())
}

/// seconds after the start until a missing sensor box is reported in words
pub const NO_BOX_BANNER_AFTER_S: u16 = 5;

/// "NO SENSOR BOX - check CAN cable" stands in the middle of the page: no
/// CAN device heard, 5 s after power on. Not after a push on it (the pilot
/// can acknowledge it - the red symbol in the band stays) and not while the flight demo runs, which
/// feeds the page itself.
pub(crate) fn no_box_banner(cm: &CoreModel) -> bool {
    cm.config.display_active == crate::model::DisplayActive::Vario
        && cm.config.overlay_active == OverlayActive::None
        && !crate::controller::page_layout::active(cm)
        && cm.control.system_state == SystemState::NoCom
        && cm.control.boot_seconds >= NO_BOX_BANNER_AFTER_S
        && !cm.control.no_box_ack
        && !cm.control.demo.flight()
}
const NO_GPS_TEXT: &str = "NO GPS";
/// "NO GPS" stands inside the ring above the satellite symbol (the band
/// itself has no room for a word): right end and middle of the text,
/// relative to the symbol
const NO_GPS_TEXT_OFFSET: Point = Point::new(-7, -19);

/// message box in the middle of the page: black, a frame in the colour of
/// the message, two lines
fn draw_banner<D>(
    display: &mut D,
    c: Point,
    line1: &str,
    line2: &str,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    use embedded_graphics::primitives::{Rectangle, RoundedRectangle};
    let (w, h) = (236, 76);
    let r = Rectangle::new(
        Point::new(c.x - w / 2, c.y - h / 2),
        Size::new(w as u32, h as u32),
    );
    RoundedRectangle::with_equal_corners(r, Size::new(12, 12))
        .into_styled(
            embedded_graphics::primitives::PrimitiveStyleBuilder::new()
                .fill_color(Colors::Black)
                .stroke_color(col)
                .stroke_width(3)
                .build(),
        )
        .draw(display)?;
    crate::view::fonts_p1::text_for(22).f.render_aligned(
        line1,
        Point::new(c.x, c.y - 14),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(col),
        display,
    )?;
    crate::view::fonts_p1::text_for(19).f.render_aligned(
        line2,
        Point::new(c.x, c.y + 16),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(Colors::White),
        display,
    )?;
    Ok(())
}

/// the small mode symbol in the outer band of the scale: lower right,
/// between the tick of -4.5 and the number 4 (23.5 deg below 3 o'clock, r 222)
pub const MODE_ICON_BAND: (Point, bool) = (Point::new(204, 88), true);

/// Where the mode symbol sits on a page (offset from the display centre)
/// and whether it is the small one. Stefan 15.11.2026: always in the outer
/// band of the scale, the same place on every page with the ring - places
/// inside the core collided with long numbers. The status page has none.
pub fn mode_icon_place(core: PageCore) -> Option<(Point, bool)> {
    match core {
        PageCore::Radar | PageCore::DeviceInfo => None,
        _ => Some(MODE_ICON_BAND),
    }
}

/// free middle of the ring page: centre circle 78 plus 12 px air
const R_FREE_P1: i32 = 90;
/// info boxes keep 8 px away from the core edge
const R_USE_P1: i32 = 184;
/// the POT pictogram in front of a caption: this many px larger than the letters
const CAPTION_MARK_EXTRA: i32 = 4;
/// right hand boxes end 8 px before the flap tape
const R_RIGHT_TAPE: i32 = 159;
/// the two blocks top/bottom end at y = -/+92, the right ones keep +/-12
/// free around 3 o'clock for the flap arrow
const Y_TOP_BOTTOM: i32 = 92;
/// (22 instead of the 12 of the sketch: about two caption heights more air
/// between the two right boxes, Stefan 27./28.10.2026)
const Y_RIGHT_GAP: i32 = 22;
/// the right boxes move this much to the left (see Y_RIGHT_GAP)
const RIGHT_SHIFT_IN: f32 = 8.0;

/// One P1 block: value at its baseline, below it ONE line "LABEL  unit".
/// `sketch` is the value size of the sketch, the caption baseline follows
/// the sketch rule (see fonts_p1::caption_offset). `room`: width the
/// caption may use - the unit is dropped first, then the label is kept.
#[allow(clippy::too_many_arguments)]
fn draw_p1_block<D>(
    display: &mut D,
    cm: &CoreModel,
    lv: LineView,
    value: &str,
    x: i32,
    baseline: i32,
    sketch: u8,
    font: &FontRenderer,
    room: i32,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    use crate::view::viewable::pot_symbol;
    use crate::view::viewable::ring_infobox::{
        pot_verdict, render_value_tight_v, tight_width, value_color, wind_arrow,
    };
    // wind arrow boxes: the arrow left of the speed, the pair centred
    let arrow = wind_arrow(cm, lv);
    let aw = arrow_w(lv, sketch);
    let vx = x + aw / 2;
    render_value_tight_v(
        display,
        font,
        value,
        Point::new(vx, baseline),
        BoxAlign::Center,
        value_color(cm, lv),
        VerticalPosition::Baseline,
    )?;
    // POT box: its yes / no left of the number, in the free space of a
    // centred block (the number stays where it is). The side boxes have no
    // room there - they get a small mark in front of the caption.
    let verdict = pot_verdict(cm, lv);
    let mut caption_mark = false;
    if verdict != crate::model::PotVerdict::None {
        let c = cm.device_const.sizes.display.center;
        let dig = fonts_p1::sketch_digit(sketch) as i32;
        let vw = tight_width(font, value);
        // as high as the digits; one or two steps smaller where the
        // circle is narrow (top box), so the disc stays in the core
        caption_mark = true;
        if x == c.x {
            for size in [dig, dig * 4 / 5, dig * 13 / 20] {
                let gap = (size / 4).max(5);
                let (sx, sy) = (vx - vw / 2 - gap - size / 2, baseline - dig / 2);
                let (dx, dy) = (c.x - (sx - size / 2), (sy - c.y).abs() + size / 3);
                if size >= 10 && dx * dx + dy * dy <= R_USE_P1 * R_USE_P1 {
                    pot_symbol::draw(display, verdict, Point::new(sx, sy), size)?;
                    caption_mark = false;
                    break;
                }
            }
        }
    }
    if let Some(a) = &arrow {
        let l = arrow_len(sketch);
        let dig = fonts_p1::sketch_digit(sketch) as i32;
        let cx = vx - tight_width(font, value) / 2 - ARROW_GAP - l;
        RingArrow::new(l, Point::new(cx, baseline - dig / 2))
            .zero_pos(pos::SIX_O_CLOCK)
            .rotate(a.rotation)
            .draw_colored(a.color, display)?;
    }
    let cap_s = fonts_p1::sketch_caption(sketch);
    let cap_f = fonts_p1::text_for(cap_s).f;
    if let Some(a) = &arrow {
        return draw_wind_caption(
            display,
            cm,
            lv,
            a.from_deg,
            x,
            baseline + fonts_p1::caption_offset(cap_s),
            cap_s,
            room,
        );
    }
    let mut cap = r2_caption(lv, units_of(cm, lv));
    let w = |t: &str| {
        cap_f
            .get_rendered_dimensions(t, Point::zero(), VerticalPosition::Baseline)
            .map(|d| d.advance.x)
            .unwrap_or(0)
    };
    if w(cap.as_str()) > room || caption_mark {
        cap = r2_caption(lv, "");
    }
    let cap_y = baseline + fonts_p1::caption_offset(cap_s);
    let mut cap_x = x;
    if caption_mark {
        // pictogram and label as a pair, centred: the pictogram takes the place
        // of the unit, a little higher than the letters
        let m = fonts_p1::text_for(cap_s).h as i32 + CAPTION_MARK_EXTRA;
        let total = m + 4 + w(cap.as_str());
        let cy = cap_y - fonts_p1::text_for(cap_s).h as i32 / 2;
        pot_symbol::draw(display, verdict, Point::new(x - total / 2 + m / 2, cy), m)?;
        cap_x = x + (m + 4) / 2;
    }
    cap_f.render_aligned(
        cap.as_str(),
        Point::new(cap_x, cap_y),
        VerticalPosition::Baseline,
        HorizontalAlignment::Center,
        FontColor::Transparent(cm.palette().vario.unit),
        display,
    )?;
    Ok(())
}

/// gap between a wind arrow and its value
const ARROW_GAP: i32 = 6;

/// tip radius of the wind arrow of a box: a little more than half the digit
/// height, so the turning arrow stays within the height of the value
fn arrow_len(sketch: u8) -> i32 {
    (fonts_p1::sketch_digit(sketch) * 0.58) as i32
}

/// width the arrow of a wind arrow box takes in front of the value
fn arrow_w(lv: LineView, sketch: u8) -> i32 {
    if matches!(lv, LineView::WindArrow | LineView::AvgWindArrow) {
        2 * arrow_len(sketch) + ARROW_GAP
    } else {
        0
    }
}

/// caption line of a wind arrow box: "270°  km/h" (the direction the wind
/// comes from), "AVG 270°  km/h" for the average; the unit goes first, then
/// the AVG, when the room is short
#[allow(clippy::too_many_arguments)]
fn draw_wind_caption<D>(
    display: &mut D,
    cm: &CoreModel,
    lv: LineView,
    from_deg: f32,
    x: i32,
    baseline: i32,
    cap_s: u8,
    room: i32,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let font = fonts_p1::text_for(cap_s);
    let f = font.f;
    let col = cm.palette().vario.unit;
    let w = |t: &str| {
        f.get_rendered_dimensions(t, Point::zero(), VerticalPosition::Baseline)
            .map(|d| d.advance.x)
            .unwrap_or(0)
    };
    let deg: heapless::String<8> = crate::tformat!(8, "{:03.0}", from_deg).unwrap_or_default();
    let unit = units_of(cm, lv);
    let unit = if unit == "knots" { "kt" } else { unit };
    let pre = if lv == LineView::AvgWindArrow {
        "AVG "
    } else {
        ""
    };
    // the degree ring takes about half a digit
    let ring = (font.h as i32 * 6) / 10 + 2;
    let mut parts: (&str, &str) = (pre, unit);
    let total = |p: (&str, &str)| {
        w(p.0) + w(deg.as_str()) + ring + if p.1.is_empty() { 0 } else { w("  ") + w(p.1) }
    };
    if total(parts) > room {
        parts.1 = "";
    }
    if total(parts) > room {
        parts.0 = "";
    }
    let mut cx = x - total(parts) / 2;
    for t in [parts.0, deg.as_str()] {
        f.render_aligned(
            t,
            Point::new(cx, baseline),
            VerticalPosition::Baseline,
            HorizontalAlignment::Left,
            FontColor::Transparent(col),
            display,
        )?;
        cx += w(t);
    }
    fonts_p1::degree_ring(display, cx, ring, baseline, font.h as i32, col)?;
    cx += ring;
    if !parts.1.is_empty() {
        let t = crate::tformat!(12, "  {}", parts.1).unwrap_or_default();
        f.render_aligned(
            t.as_str(),
            Point::new(cx, baseline),
            VerticalPosition::Baseline,
            HorizontalAlignment::Left,
            FontColor::Transparent(col),
            display,
        )?;
    }
    Ok(())
}

/// sketch value sizes the boxes may step down through
const P1_LADDER: [u8; 8] = [57, 53, 49, 46, 42, 38, 33, 30];

/// height of a sketch block: digits, 6 px, caption font
fn p1_block_h(sketch: u8) -> f32 {
    fonts_p1::sketch_digit(sketch) + 6.0 + fonts_p1::sketch_caption(sketch) as f32
}

/// Bottom box of the large centering page: one size smaller than the top
/// box, so the centering aid between the two boxes gets more room (a large
/// info box at the top, a small one below).
const CENTERING_BOTTOM_LADDER: [u8; 4] = [38, 33, 30, 27];
/// free pixels between the centering aid and the box above / below it
/// Inner end of the needle on the large centering page: r 150, the edge of
/// the room the large aid may use, so the visible needle is 86 px long. It
/// runs over the two boxes where they reach further out (top 156 to 175,
/// bottom 172 to 176) - in the foreground, as on the info boxes page (the
/// usual UI convention; a needle ending at r 178, outside of the boxes, was
/// too short).
const CENTERING_NEEDLE: f32 = 150.0;
const CENTERING_GAP_TOP: f32 = 4.0;
const CENTERING_GAP_BOTTOM: f32 = 6.0;

/// The small bottom box ends this far below the middle of the display:
/// further out than the large bottom box of the info boxes page (which
/// ends at about 156), because the smaller value still fits the chord
/// there. That moves the box away from the centering aid.
const CENTERING_BOTTOM_END: f32 = 172.0;

/// inner edge of the small bottom box
fn centering_bottom_y() -> f32 {
    CENTERING_BOTTOM_END - p1_block_h(CENTERING_BOTTOM_LADDER[0])
}

/// The centering page (core "Centering aid") shows its centering aid large
/// while circling: no centre circle, a short needle, a large box at the top
/// and a small one at the bottom. In straight flight it is the info boxes
/// page with its cruise set. In the page editor the edited set decides.
pub(crate) fn centering_large(cm: &CoreModel) -> bool {
    cm.config.page().core == PageCore::ThermalAssist && circling_view(cm)
}

/// The page shows what it shows while circling: the glider circles - or,
/// in the page editor, the circling set is the one that is edited.
pub(crate) fn circling_view(cm: &CoreModel) -> bool {
    if crate::controller::page_layout::active(cm) {
        !cm.control.layout.cruise
    } else {
        cm.control.fly_mode == FlyMode::Circling
    }
}

/// where the large centering aid is drawn: the band between the two boxes
pub(crate) fn centering_place(cm: &CoreModel) -> AidPlace {
    let c = cm.device_const.sizes.display.center;
    let top = c.y as f32 - Y_TOP_BOTTOM as f32 + CENTERING_GAP_TOP;
    let bottom = c.y as f32 + centering_bottom_y() - CENTERING_GAP_BOTTOM;
    AidPlace::large(cm, c.x, top, bottom)
}

/// The box at the top and the box at the bottom of the ring page.
/// `small_bottom`: the bottom box of the large centering page.
fn draw_top_bottom_p1<D>(
    display: &mut D,
    cm: &CoreModel,
    cm_1s: &CoreModel,
    vario_mode: bool,
    small_bottom: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    use crate::view::viewable::ring_infobox::tight_width;
    let c = cm.device_const.sizes.display.center;
    let chord = |r: f32, dy: f32| {
        let v = r * r - dy * dy;
        if v > 0.0 {
            isqrt(v as i32) as f32
        } else {
            0.0
        }
    };
    let r_use = R_USE_P1 as f32;
    let ytb = Y_TOP_BOTTOM as f32;

    for (slot, top) in [(0_usize, true), (1, false)] {
        // the small bottom box hangs below its own inner edge
        let small = small_bottom && !top;
        let ladder: &[u8] = if small {
            &CENTERING_BOTTOM_LADDER
        } else {
            &P1_LADDER
        };
        let y_in = if small { centering_bottom_y() } else { ytb };
        let lv = cm.config.box_view(slot, vario_mode);
        let hl = crate::controller::page_layout::box_highlight(cm, slot);
        if lv == LineView::None {
            if hl {
                // the empty slot: where a value would be
                let h = p1_block_h(ladder[0]) as i32;
                let y0 = if top {
                    c.y - Y_TOP_BOTTOM - h
                } else {
                    c.y + y_in as i32
                };
                empty_slot(display, c.x, y0 - 4, 80, h + 8)?;
            }
            continue;
        }
        // the software version is shown in the top box during the first seconds
        if top && cm.control.alive_ticks <= 70 {
            let s = cm.device_const.misc.sw_version.as_string();
            fonts_p1::text_for(22).f.render_aligned(
                s.as_str(),
                Point::new(c.x, c.y - Y_TOP_BOTTOM - 20),
                VerticalPosition::Center,
                HorizontalAlignment::Center,
                FontColor::Transparent(cm.palette().vario.scale),
                display,
            )?;
            continue;
        }
        let value = r2_value(cm_1s, lv);
        let mut sketch = ladder[ladder.len() - 1];
        for &s in ladder {
            let f = fonts_p1::value_for(s);
            let h = p1_block_h(s);
            // the sketch rule: half the value width + 4 inside the core
            // chord at the outer edge of the block
            if ((tight_width(f.f, value.as_str()) + arrow_w(lv, s)) / 2 + 4) as f32
                <= chord(r_use, y_in + h)
            {
                sketch = s;
                break;
            }
        }
        let f = fonts_p1::value_for(sketch);
        let dig = fonts_p1::sketch_digit(sketch);
        let baseline = if top {
            -ytb - p1_block_h(sketch) + dig
        } else {
            y_in + dig
        };
        let room = 2 * chord(r_use, y_in + p1_block_h(sketch)) as i32;
        if hl {
            let half = ((tight_width(f.f, value.as_str()) + arrow_w(lv, sketch)) / 2 + 14).max(64);
            let y0 = c.y + baseline as i32 - dig as i32 - 7;
            box_frame(
                display,
                c.x - half,
                y0,
                c.x + half,
                y0 + p1_block_h(sketch) as i32 + 12,
            )?;
        }
        draw_p1_block(
            display,
            cm_1s,
            lv,
            value.as_str(),
            c.x,
            c.y + baseline as i32,
            sketch,
            f.f,
            room,
        )?;
    }

    Ok(())
}

/// Info boxes of the ring page, Stefly design P1 (layout B, text R2), laid
/// out exactly like the sketch skizze_ring_schrift.png (R2) and
/// skizze_windbox.png (W1): top and bottom "inb57" of the sketch (u8g2
/// inb42), the top block ends at y -92, the bottom one starts at y +92;
/// a value that does not fit takes the next smaller size in its own box
/// (wind "270/25": sketch inb53). Right two boxes above and below 3 o'clock
/// (gap +-12 for the flap arrow) between the centre circle (r 90) and the
/// flap tape (r 159): sketch inb24 (u8g2 inb19), without flaps up to r 184:
/// sketch inb33 (u8g2 inb24). Captions fub11 (sketch 13 / 16).
fn draw_boxes_p1<D>(
    display: &mut D,
    cm: &CoreModel,
    cm_1s: &CoreModel,
    with_tape: bool,
    vario_mode: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    use crate::view::viewable::ring_infobox::tight_width;
    let c = cm.device_const.sizes.display.center;
    let chord = |r: f32, dy: f32| {
        let v = r * r - dy * dy;
        if v > 0.0 {
            isqrt(v as i32) as f32
        } else {
            0.0
        }
    };
    let r_use = R_USE_P1 as f32;

    draw_top_bottom_p1(display, cm, cm_1s, vario_mode, false)?;

    // ---------------- right hand side: two boxes around 3 o'clock ----------------
    let (r_lim, ladder): (f32, &[u8]) = if with_tape {
        (R_RIGHT_TAPE as f32, &[24, 21])
    } else {
        // without the tape the boxes have room for one size more than the
        // sketch (inb27 instead of inb24), so the two layouts differ clearly
        (r_use, &[38, 33, 30, 27])
    };
    let gap = Y_RIGHT_GAP as f32;
    for (slot, upper) in [(2_usize, true), (3, false)] {
        let lv = cm.config.box_view(slot, vario_mode);
        let hl = crate::controller::page_layout::box_highlight(cm, slot);
        if lv == LineView::None {
            if hl {
                let s = ladder[ladder.len() - 1];
                let h = p1_block_h(s);
                let xl = chord(R_FREE_P1 as f32, gap) + 4.0;
                let xr = chord(r_lim, gap + h) - 4.0;
                let xm = (xl + xr) / 2.0 - RIGHT_SHIFT_IN;
                let ytop = if upper { -gap - h } else { gap };
                empty_slot(
                    display,
                    c.x + xm as i32,
                    c.y + ytop as i32 - 4,
                    ((xr - xl) / 2.0) as i32,
                    h as i32 + 8,
                )?;
            }
            continue;
        }
        let mut value = r2_value(cm_1s, lv);
        let mut chosen = ladder[ladder.len() - 1];
        let mut fits = false;
        let fit = |s: u8, v: &str| {
            let xl = chord(R_FREE_P1 as f32, gap) + 4.0;
            let xr = chord(r_lim, gap + p1_block_h(s)) - 4.0;
            (tight_width(fonts_p1::value_for(s).f, v) + arrow_w(lv, s)) as f32 <= xr - xl
        };
        for &s in ladder {
            if fit(s, value.as_str()) {
                chosen = s;
                fits = true;
                break;
            }
        }
        // wind "270/25" is too wide for the side: the speed alone (sketch
        // R2: "25 WIND"), the direction shows the wind arrow in the middle
        if !fits {
            if let Some(i) = value.find('/') {
                let speed: heapless::String<20> =
                    heapless::String::try_from(&value.as_str()[i + 1..]).unwrap_or_default();
                value = speed;
                chosen = ladder
                    .iter()
                    .copied()
                    .find(|&s| fit(s, value.as_str()))
                    .unwrap_or(ladder[ladder.len() - 1]);
            }
        }
        let h = p1_block_h(chosen);
        let dig = fonts_p1::sketch_digit(chosen);
        let xl = chord(R_FREE_P1 as f32, gap) + 4.0;
        let xr = chord(r_lim, gap + h) - 4.0;
        // RIGHT_SHIFT_IN towards the middle: the boxes keep the same air to the
        // flap tape / core edge now that they sit further from 3 o'clock
        let xm = (xl + xr) / 2.0 - RIGHT_SHIFT_IN;
        let ytop = if upper { -gap - h } else { gap };
        if hl {
            let half = ((xr - xl) / 2.0) as i32 + 2;
            let y0 = c.y + ytop as i32 - 5;
            box_frame(
                display,
                c.x + xm as i32 - half,
                y0,
                c.x + xm as i32 + half,
                y0 + h as i32 + 9,
            )?;
        }
        draw_p1_block(
            display,
            cm_1s,
            lv,
            value.as_str(),
            c.x + xm as i32,
            c.y + (ytop + dig) as i32,
            chosen,
            fonts_p1::value_for(chosen).f,
            (xr - xl) as i32,
        )?;
    }
    Ok(())
}

/// unit string of a viewable as shown in the boxes
fn units_of(cm: &CoreModel, lv: LineView) -> &'static str {
    crate::view::viewable::ring_infobox::unit_of(cm, lv)
}

#[cfg(test)]
mod ring_page_tests {
    use super::*;
    use crate::view::test_fixture::{test_model, TestDisplay, DISPLAY_HEIGHT, DISPLAY_WIDTH};
    use crate::{FloatToSpeed, LineView, VarioMode};

    /// rows that carry ink of the value colour inside a horizontal band
    /// around `y`, as (first, last) row - the height of the value glyphs
    fn value_rows(d: &TestDisplay, y: i32, color: Colors) -> (i32, i32) {
        let (mut first, mut last) = (i32::MAX, i32::MIN);
        for row in (y - 40).max(0)..(y + 40).min(DISPLAY_HEIGHT as i32) {
            for x in 180..300 {
                if d.buf[(row * DISPLAY_WIDTH as i32 + x) as usize] == Some(color) {
                    first = first.min(row);
                    last = last.max(row);
                }
            }
        }
        (first, last)
    }

    #[test]
    fn top_and_bottom_box_use_the_same_value_font() {
        let mut cm = test_model();
        cm.control.alive_ticks = 1000;
        cm.control.vario_mode = VarioMode::Vario;
        cm.control.fly_mode = FlyMode::StraightFlight;
        cm.config.overlay_active = OverlayActive::None;
        // top and bottom box with a three digit value each
        let idx = cm.config.page_idx as usize;
        let page = &mut cm.config.pages[idx];
        page.core = PageCore::Boxes;
        page.boxes[0] = LineView::AverageClimbRate;
        page.boxes[1] = LineView::AverageClimbRate;
        cm.calculated.av2_climb_rate = 2.3.m_s();
        cm.sensor.average_climb_rate = 2.3.m_s();
        let cm_1s = cm;
        let mut td = ThermalData::default();
        let mut d = TestDisplay::new();
        draw_ring(&mut d, &cm, &cm_1s, &mut td).unwrap();
        let color = cm.palette().vario.value;
        let c = cm.device_const.sizes.display.center.y;
        // P1: the top block ends at -92, the bottom block starts at +92
        let (t0, t1) = value_rows(&d, c - 92 - 40, color);
        let (b0, b1) = value_rows(&d, c + 92 + 30, color);
        if let Ok(path) = std::env::var("RING_DUMP") {
            crate::view::test_fixture::dump_ppm(&d, &path);
        }
        assert!(
            t1 > t0 && b1 > b0,
            "boxes not drawn: top {t0}..{t1}, bottom {b0}..{b1}"
        );
        assert_eq!(
            t1 - t0,
            b1 - b0,
            "value glyph height top {} vs bottom {}",
            t1 - t0,
            b1 - b0
        );
        // nothing of the blocks crosses into the middle band +/-92 (1 px for
        // the overshoot of the round B612 digits over the flat "0")
        assert!(t1 < c - 92 + 2, "top block reaches {t1}");
        assert!(b0 > c + 92 - 2, "bottom block starts {b0}");
    }

    /// pixels of `color` in the middle of the page (the message box)
    fn middle_ink(cm: &CoreModel, color: Colors) -> usize {
        let cm_1s = *cm;
        let mut td = ThermalData::default();
        let mut d = TestDisplay::new();
        draw_ring(&mut d, cm, &cm_1s, &mut td).unwrap();
        let c = cm.device_const.sizes.display.center;
        let mut n = 0;
        for y in c.y - 38..c.y + 38 {
            for x in c.x - 118..c.x + 118 {
                if d.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] == Some(color) {
                    n += 1;
                }
            }
        }
        n
    }

    /// A display without its sensor box says so in words - but not in the
    /// first seconds after power on, and not over the setup or a menu.
    #[test]
    fn a_missing_sensor_box_is_reported_in_words() {
        let mut cm = test_model();
        cm.control.alive_ticks = 1000;
        cm.config.overlay_active = OverlayActive::None;
        cm.control.system_state = SystemState::NoCom;
        let red = cm.palette().signal.stop;
        let base = middle_ink(&cm, red);
        cm.control.boot_seconds = NO_BOX_BANNER_AFTER_S;
        let with = middle_ink(&cm, red);
        assert!(with > base + 1500, "no message box: {base} -> {with} px");
        // the flight menu lies over the page: no box
        cm.config.overlay_active = OverlayActive::Menu;
        assert_eq!(middle_ink(&cm, red), base);
        cm.config.overlay_active = OverlayActive::None;
        // with the box but without a GPS fix the middle stays free
        cm.control.system_state = SystemState::CanOk;
        assert_eq!(middle_ink(&cm, red), base);
        cm.control.system_state = SystemState::CanAndGpsOk;
        assert_eq!(middle_ink(&cm, red), base);
    }

    /// a new device has two pages: the vario and the horizon
    #[test]
    fn a_new_device_has_the_vario_and_the_horizon_page() {
        let pages = crate::model::page::default_pages();
        let on: std::vec::Vec<_> = pages
            .iter()
            .filter(|p| p.frame != crate::model::PageFrame::Off)
            .collect();
        assert_eq!(on.len(), 2);
        assert!(on[0].core == PageCore::Boxes && on[1].core == PageCore::Horizon);
    }

    /// ring page with the POT box in `slot` (0 top, 1 bottom, 2 / 3 right)
    fn pot_page(slot: usize, verdict: crate::model::PotVerdict) -> (TestDisplay, CoreModel) {
        let mut cm = test_model();
        cm.control.alive_ticks = 1000;
        cm.control.vario_mode = VarioMode::Vario;
        cm.control.fly_mode = FlyMode::Circling;
        cm.config.overlay_active = OverlayActive::None;
        let idx = cm.config.page_idx as usize;
        let page = &mut cm.config.pages[idx];
        page.core = PageCore::Boxes;
        page.boxes = [
            LineView::Altitude,
            LineView::Altitude,
            LineView::TrueAirSpeed,
            LineView::TrueAirSpeed,
            LineView::None,
        ];
        page.boxes[slot] = LineView::PotentialClimb;
        cm.calculated.pot_climb = Some(2.3.m_s());
        cm.calculated.pot_predictive = true;
        cm.calculated.pot_verdict = verdict;
        let cm_1s = cm;
        let mut td = ThermalData::default();
        let mut d = TestDisplay::new();
        draw_ring(&mut d, &cm, &cm_1s, &mut td).unwrap();
        (d, cm)
    }

    /// "POT Yes/No" off: the box shows its value without tick or cross,
    /// exactly as when there is no verdict
    #[test]
    fn the_yes_no_symbol_of_the_pot_box_can_be_switched_off() {
        use crate::model::PotVerdict;
        for slot in 0..4 {
            let (none, _) = pot_page(slot, PotVerdict::None);
            for verdict in [PotVerdict::Yes, PotVerdict::No] {
                let (on, mut cm) = pot_page(slot, verdict);
                assert!(on.buf != none.buf, "the symbol is drawn (slot {slot})");
                cm.config.pot_advice = false;
                let cm_1s = cm;
                let mut d = TestDisplay::new();
                draw_ring(&mut d, &cm, &cm_1s, &mut ThermalData::default()).unwrap();
                assert!(
                    d.buf == none.buf,
                    "no symbol with the setting off (slot {slot})"
                );
            }
        }
    }

    /// pixels of one colour in the box x0..x1 / y0..y1, as (count, leftmost, rightmost)
    fn ink(
        d: &TestDisplay,
        (x0, x1): (i32, i32),
        (y0, y1): (i32, i32),
        color: Colors,
    ) -> (usize, i32, i32) {
        let (mut n, mut left, mut right) = (0, i32::MAX, i32::MIN);
        for y in y0..y1 {
            for x in x0..x1 {
                if d.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] == Some(color) {
                    n += 1;
                    left = left.min(x);
                    right = right.max(x);
                }
            }
        }
        (n, left, right)
    }

    /// pixels where two renders differ: (count, left, right, top, bottom, pixels of `fill`)
    fn diff(a: &TestDisplay, b: &TestDisplay, fill: Colors) -> (usize, i32, i32, i32, i32, usize) {
        let (mut n, mut l, mut r, mut t, mut bo, mut f) =
            (0, i32::MAX, i32::MIN, i32::MAX, i32::MIN, 0);
        for (i, (pa, pb)) in a.buf.iter().zip(b.buf.iter()).enumerate() {
            if pa != pb {
                let (x, y) = (
                    i as i32 % DISPLAY_WIDTH as i32,
                    i as i32 / DISPLAY_WIDTH as i32,
                );
                n += 1;
                (l, r, t, bo) = (l.min(x), r.max(x), t.min(y), bo.max(y));
                if *pb == Some(fill) {
                    f += 1;
                }
            }
        }
        (n, l, r, t, bo, f)
    }

    /// plan_potential_v3.md A9: the yes / no stands left of the number, in
    /// the free space; the number does not move when it comes or goes.
    /// Yes: a green disc with a check mark, no: a white disc with a cross.
    #[test]
    fn pot_box_shows_its_yes_and_no_left_of_the_number() {
        use crate::model::PotVerdict;
        use crate::view::viewable::pot_symbol::color;
        for (slot, rows) in [(1usize, (92 - 4, 92 + 46)), (0, (-92 - 70, -92 - 16))] {
            let (none, cm) = pot_page(slot, PotVerdict::None);
            let c = cm.device_const.sizes.display.center;
            let (xs, ys) = ((c.x - 184, c.x + 184), (c.y + rows.0, c.y + rows.1));
            // the number (an estimate: magenta)
            let number = ink(&none, xs, ys, Colors::Magenta);
            assert!(number.0 > 500, "slot {slot}: no number");
            for verdict in [PotVerdict::Yes, PotVerdict::No] {
                let (d, _) = pot_page(slot, verdict);
                // the number is the same
                assert_eq!(
                    ink(&d, xs, ys, Colors::Magenta),
                    number,
                    "slot {slot} {verdict:?}: the number moved"
                );
                // a disc left of the number, mostly in its fill colour
                let (n, l, r, t, b, fill) = diff(&none, &d, color(verdict));
                let (w, h) = (r - l + 1, b - t + 1);
                assert!(
                    (w - h).abs() <= 1 && w >= 24,
                    "slot {slot} {verdict:?}: {w} x {h} px"
                );
                assert!(
                    n as f32 > 0.7 * 0.785 * (w * h) as f32,
                    "slot {slot} {verdict:?}: {n} px in {w} x {h}"
                );
                assert!(
                    fill as f32 > 0.5 * n as f32,
                    "slot {slot} {verdict:?}: fill {fill} of {n}"
                );
                assert!(
                    r < number.1 && number.1 - r >= 4,
                    "slot {slot} {verdict:?}: pictogram {l}..{r}, number from {}",
                    number.1
                );
                assert!(
                    t >= ys.0 && b < ys.1,
                    "slot {slot} {verdict:?}: rows {t}..{b}"
                );
                // nothing of it outside the core (r 192)
                for (x, y) in [(l, t), (l, b), (r, t), (r, b)] {
                    let (dx, dy) = (x - c.x, y - c.y);
                    assert!(
                        dx * dx + dy * dy <= 192 * 192 || (x > l && x < r),
                        "slot {slot}: corner {x},{y} outside the core"
                    );
                }
                let (cx, cy) = ((l + r) / 2, (t + b) / 2);
                let (dx, dy) = (cx - c.x, cy - c.y);
                let rad = w / 2;
                assert!(
                    (dx * dx + dy * dy) as f32 <= ((192 - rad) * (192 - rad)) as f32,
                    "slot {slot}: disc reaches out of the core"
                );
            }
        }
    }

    /// plan_potential_v3.md A9: a side box has no room next to the number -
    /// a small pictogram in front of the caption, the number untouched
    #[test]
    fn pot_in_a_side_box_marks_its_caption() {
        use crate::model::PotVerdict;
        use crate::view::viewable::pot_symbol::color;
        let (none, cm) = pot_page(2, PotVerdict::None);
        let c = cm.device_const.sizes.display.center;
        let (xs, ys) = ((c.x + 60, c.x + 190), (c.y - 80, c.y));
        let number = ink(&none, xs, ys, Colors::Magenta);
        assert!(number.0 > 100, "no number in the side box");
        for verdict in [PotVerdict::Yes, PotVerdict::No] {
            let (d, _) = pot_page(2, verdict);
            assert_eq!(ink(&d, xs, ys, Colors::Magenta), number);
            // the pictogram and the caption moved right to make room: changes only below the number
            let (n, l, r, t, b, fill) = diff(&none, &d, color(verdict));
            assert!(n > 40 && fill > 20, "{verdict:?}: {n} px, fill {fill}");
            assert!(
                l >= xs.0 && r < xs.1 && t >= ys.0 && b < ys.1,
                "{verdict:?}: {l}..{r} / {t}..{b}"
            );
            assert!(b - t <= 18, "{verdict:?}: {} px high", b - t + 1);
        }
    }
}

/// Big values page of design P1 (plan_ring_design_p1.md ch. 7) instead of
/// the three large boxes (false: the former layout)
const BIG_VALUES_P1: bool = true;
/// inner end of the needle on the big values page
const BIG_VALUES_NEEDLE: f32 = 130.0;

/// Big values page (variant C, sketch skizze_grosse_boxen_v4.png): all
/// centred, two main values of the sketch size 72 (u8g2 inb53), above and
/// below one small value of the sketch size 33 (u8g2 inb24), every value
/// with its "LABEL  unit" line. 12 px between the two big blocks, 16 px to
/// the small ones, the stack symmetrical about y 0 - positions computed
/// with the sketch formulas. Slots: top = "Box Top", the two big ones =
/// "Box 1" and "Box 2", bottom = "Box Bottom". A big value that is too
/// wide takes the next smaller size.
fn draw_big_values<D>(
    display: &mut D,
    cm: &CoreModel,
    cm_1s: &CoreModel,
    with_tape: bool,
    vario_mode: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    use crate::view::viewable::ring_infobox::tight_width;
    let c = cm.device_const.sizes.display.center;
    let views = [
        cm.config.box_view(0, vario_mode),
        cm.config.box_view(2, vario_mode),
        cm.config.box_view(3, vario_mode),
        cm.config.box_view(1, vario_mode),
    ];
    // the two big values in the size of the top / bottom box of the ring
    // page (sketch 57), more air between the blocks (Stefan 28.10.2026)
    let (big, small, gap_mid, gap_out) = (57u8, 33u8, 26.0f32, 22.0f32);
    let hb = p1_block_h(big);
    let hs = p1_block_h(small);
    // block tops like the sketch
    let y_b1 = -gap_mid / 2.0 - hb;
    let y_b2 = gap_mid / 2.0;
    let y_s1 = y_b1 - gap_out - hs;
    let y_s2 = y_b2 + hb + gap_out;
    let right = if with_tape { R_RIGHT_TAPE } else { R_USE_P1 };
    const SLOTS: [usize; 4] = [0, 2, 3, 1];
    for (k, (lv, ytop, size)) in [
        (views[0], y_s1, small),
        (views[1], y_b1, big),
        (views[2], y_b2, big),
        (views[3], y_s2, small),
    ]
    .iter()
    .enumerate()
    {
        let hl = crate::controller::page_layout::box_highlight(cm, SLOTS[k]);
        let h = p1_block_h(*size) as i32;
        if *lv == LineView::None || *lv == LineView::LastElemntNotInUse {
            if hl {
                empty_slot(display, c.x, c.y + *ytop as i32 - 5, 90, h + 10)?;
            }
            continue;
        }
        let value = r2_value(cm_1s, *lv);
        let mut sz = *size;
        if k == 1 || k == 2 {
            // too wide (with the flap tape on the right): step down
            for s in [57u8, 53, 49, 46] {
                sz = s;
                let half =
                    (tight_width(fonts_p1::value_for(s).f, value.as_str()) + arrow_w(*lv, s)) / 2;
                if half + 6 <= right.min(isqrt(R_USE_P1 * R_USE_P1 - 60 * 60)) {
                    break;
                }
            }
        }
        // the block keeps its place, a smaller value sits on the same baseline
        let baseline = ytop + fonts_p1::sketch_digit(*size);
        if hl {
            let half =
                ((tight_width(fonts_p1::value_for(sz).f, value.as_str()) + arrow_w(*lv, sz)) / 2
                    + 14)
                    .max(64);
            let y0 = c.y + *ytop as i32 - 5;
            box_frame(display, c.x - half, y0, c.x + half, y0 + h + 10)?;
        }
        draw_p1_block(
            display,
            cm_1s,
            *lv,
            value.as_str(),
            c.x,
            c.y + baseline as i32,
            *size,
            fonts_p1::value_for(sz).f,
            2 * right,
        )?;
    }
    Ok(())
}

/// right edge of the two values (a bit right of the middle,
/// Stefan 28.10.2026), stays clear of the flap tape (r 165)
const TWO_RIGHT: i32 = 112;
/// left limit: the needle ends at r 130 there
const TWO_LEFT: i32 = -124;
/// digit height of the two values (u8g2 inb57, a little smaller than the
/// first try with inb63) and the caption font
const TWO_DIGIT: i32 = 57;
const TWO_CAPTION: u8 = 21;
/// gap between the two blocks (40 instead of the first 24, Stefan)
const TWO_GAP: i32 = 40;

/// Two values page: two very large values, right aligned at TWO_RIGHT,
/// each with its caption line right aligned below. Slots 2 and 3 (the big
/// values of the 4 values page). A value that does not fit takes the next
/// smaller font, on the same baseline.
fn draw_two_values<D>(
    display: &mut D,
    cm: &CoreModel,
    cm_1s: &CoreModel,
    _with_tape: bool,
    vario_mode: bool,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    use crate::view::viewable::ring_infobox::{
        render_value_tight_v, tight_width, value_color, wind_arrow,
    };
    let c = cm.device_const.sizes.display.center;
    let cap = fonts_p1::text_for(TWO_CAPTION);
    let block_h = TWO_DIGIT + 8 + cap.h;
    let xr = c.x + TWO_RIGHT;
    let ladder: [(&FontRenderer, i32); 3] = [
        (&fonts_p1::VALUE_57, TWO_DIGIT),
        (&fonts_p1::VALUE_53, 53),
        (&fonts_p1::VALUE_42, 42),
    ];
    for (slot, top) in [(2usize, -TWO_GAP / 2 - block_h), (3, TWO_GAP / 2)] {
        let lv = cm.config.box_view(slot, vario_mode);
        let y0 = c.y + top;
        let hl = crate::controller::page_layout::box_highlight(cm, slot);
        if lv == LineView::None || lv == LineView::LastElemntNotInUse {
            if hl {
                empty_slot(display, xr - 100, y0 - 6, 104, block_h + 12)?;
            }
            continue;
        }
        let value = r2_value(cm_1s, lv);
        let arrow = wind_arrow(cm_1s, lv);
        let arrow_l = (TWO_DIGIT as f32 * 0.5) as i32;
        let aw = if arrow.is_some() {
            2 * arrow_l + ARROW_GAP
        } else {
            0
        };
        let (font, dig) = ladder
            .iter()
            .copied()
            .find(|(f, _)| xr - tight_width(f, value.as_str()) - aw >= c.x + TWO_LEFT)
            .unwrap_or(ladder[2]);
        let vw = tight_width(font, value.as_str());
        let baseline = y0 + TWO_DIGIT;
        if hl {
            box_frame(
                display,
                xr - vw - aw - 12,
                y0 - 6,
                xr + 12,
                y0 + block_h + 8,
            )?;
        }
        render_value_tight_v(
            display,
            font,
            value.as_str(),
            Point::new(xr, baseline),
            BoxAlign::Right,
            value_color(cm_1s, lv),
            VerticalPosition::Baseline,
        )?;
        // POT box: its yes / no left of the number (the number stays)
        let verdict = crate::view::viewable::ring_infobox::pot_verdict(cm_1s, lv);
        if verdict != crate::model::PotVerdict::None {
            use crate::view::viewable::pot_symbol;
            let size = dig * 3 / 4;
            pot_symbol::draw(
                display,
                verdict,
                Point::new(xr - vw - size / 4 - size / 2, baseline - dig * 37 / 100),
                size,
            )?;
        }
        let cap_y = baseline + 8 + cap.h;
        if let Some(a) = &arrow {
            RingArrow::new(
                arrow_l,
                Point::new(xr - vw - ARROW_GAP - arrow_l, baseline - dig / 2),
            )
            .zero_pos(pos::SIX_O_CLOCK)
            .rotate(a.rotation)
            .draw_colored(a.color, display)?;
            // the caption ("270°  km/h") centred under the speed
            draw_wind_caption(
                display,
                cm_1s,
                lv,
                a.from_deg,
                xr - vw / 2,
                cap_y,
                TWO_CAPTION,
                vw + aw,
            )?;
            continue;
        }
        let mut text = r2_caption(lv, units_of(cm_1s, lv));
        let room = xr - (c.x + TWO_LEFT);
        let w = cap
            .f
            .get_rendered_dimensions(text.as_str(), Point::zero(), VerticalPosition::Baseline)
            .map(|d| d.advance.x)
            .unwrap_or(0);
        if w > room {
            text = r2_caption(lv, "");
        }
        cap.f.render_aligned(
            text.as_str(),
            Point::new(xr, cap_y),
            VerticalPosition::Baseline,
            HorizontalAlignment::Right,
            FontColor::Transparent(cm.palette().vario.unit),
            display,
        )?;
    }
    Ok(())
}

/// The scale line of the wallpaper (r 202, 2.5 px wide) is drawn as one
/// exact ring r 200 ... 205 (grows outwards), the minor ticks (3 px) become
/// 5 px, the major ones (6 px, rounded cap at r 212) 7 px with the cap at
/// r 213 (MAJOR_R1). Geometry of
/// assets/create_wallpaper_vario.py: 21 ticks every 18 deg from 9 o'clock.
fn thicken_scale<D>(display: &mut D, cm: &CoreModel, scl: f32, col: Colors) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let c = cm.device_const.sizes.display.center;
    // the whole line is drawn here, r 200 ... 205: the wallpaper's line is
    // anti-aliased at the edges and lies at r 200..201 on some angles, a
    // ring from 203 on left a dark gap there (seen on the horizon page)
    fill_ring(
        display,
        cm,
        c,
        (205.0 * scl) as i32,
        (200.0 * scl) as i32,
        col,
    )?;
    // The ticks as exact shapes, pixel by pixel: embedded-graphics' thick
    // line plus a circle for the cap gave lumpy, uneven ends (the circle of
    // an even diameter sits half a pixel off, the line ends are cut
    // square). The wallpaper was drawn around the pixel corner (239.5,
    // 239.5), the ticks use the same centre so they cover its ticks fully.
    let (cx, cy) = (c.x as f32 - 0.5, c.y as f32 - 0.5);
    for (k, (cos, sin)) in TICK_DIRS.iter().enumerate() {
        // k even: major tick (every 36 deg from 9 o'clock)
        let (r0, r1, half, round) = if k % 2 == 0 {
            (203.0, MAJOR_R1, MAJOR_HALF, true)
        } else {
            (203.0, 212.0, 2.5, false)
        };
        #[cfg(test)]
        let (r1, half) = if k % 2 == 0 {
            let v = |n: &str, d: f32| {
                std::env::var(n)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(d)
            };
            (v("TICK_R1", r1), v("TICK_HALF", half))
        } else {
            (r1, half)
        };
        fill_tick(
            display,
            cm,
            (cx, cy),
            (*cos, *sin),
            r0 * scl,
            r1 * scl,
            half * scl,
            round,
            col,
        )?;
    }
    Ok(())
}

/// major tick: straight part up to this radius, then the round cap - 7 px
/// wide, outer end r 213 (8 px wide to r 212 looked like a lump: a cap that
/// is nearly all of the stub; Stefan 28.10.2026)
const MAJOR_R1: f32 = 209.5;
const MAJOR_HALF: f32 = 3.5;

/// directions of the 20 ticks, every 18 deg clockwise from 9 o'clock, as
/// (x, y) on the display (exact values - micromath's sin / cos are rough)
const TICK_DIRS: [(f32, f32); 20] = [
    (-1.0, 0.0),
    (-0.951_056_5, -0.309_017),
    (-0.809_017, -0.587_785_3),
    (-0.587_785_3, -0.809_017),
    (-0.309_017, -0.951_056_5),
    (0.0, -1.0),
    (0.309_017, -0.951_056_5),
    (0.587_785_3, -0.809_017),
    (0.809_017, -0.587_785_3),
    (0.951_056_5, -0.309_017),
    (1.0, 0.0),
    (0.951_056_5, 0.309_017),
    (0.809_017, 0.587_785_3),
    (0.587_785_3, 0.809_017),
    (0.309_017, 0.951_056_5),
    (0.0, 1.0),
    (-0.309_017, 0.951_056_5),
    (-0.587_785_3, 0.809_017),
    (-0.809_017, 0.587_785_3),
    (-0.951_056_5, 0.309_017),
];

/// One tick: the pixels within `half` of the radial segment r0 ... r1 in
/// direction `dir` (a capsule with a round outer end when `round`, else a
/// straight end at r1). Pixel centres are tested, so the edge is exact.
#[allow(clippy::too_many_arguments)]
fn fill_tick<D>(
    display: &mut D,
    cm: &CoreModel,
    centre: (f32, f32),
    dir: (f32, f32),
    r0: f32,
    r1: f32,
    half: f32,
    round: bool,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let width = cm.device_const.sizes.display.width as i32;
    let height = cm.device_const.sizes.display.height as i32;
    let reach = r1 + half + 1.0;
    let (ax, ay) = (centre.0 + dir.0 * r0, centre.1 + dir.1 * r0);
    let (bx, by) = (centre.0 + dir.0 * reach, centre.1 + dir.1 * reach);
    let x0 = (ax.min(bx) - half - 1.0).floor().max(0.0) as i32;
    let x1 = (ax.max(bx) + half + 1.0).ceil().min((width - 1) as f32) as i32;
    let y0 = (ay.min(by) - half - 1.0).floor().max(0.0) as i32;
    let y1 = (ay.max(by) + half + 1.0).ceil().min((height - 1) as f32) as i32;
    let h2 = half * half;
    for y in y0..=y1 {
        // one run per row (the shape is convex)
        let mut run: Option<(i32, i32)> = None;
        for x in x0..=x1 {
            let (px, py) = (x as f32 - centre.0, y as f32 - centre.1);
            // along the tick (radius) and across it
            let along = px * dir.0 + py * dir.1;
            let across = -px * dir.1 + py * dir.0;
            let inside = if along < r0 {
                false
            } else if along <= r1 {
                across * across <= h2
            } else if round {
                let d = along - r1;
                d * d + across * across <= h2
            } else {
                false
            };
            if inside {
                run = Some(match run {
                    Some((a, _)) => (a, x),
                    None => (x, x),
                });
            }
        }
        if let Some((a, b)) = run {
            // Safety: x and y are clamped to the display above
            unsafe {
                display.draw_line_unchecked(
                    y as usize * width as usize + a as usize,
                    (b - a + 1) as usize,
                    col,
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod mode_icon_tests {
    use super::*;

    #[test]
    fn the_mode_symbol_sits_in_the_outer_band_on_every_ring_page() {
        // between the scale line (r 205) and the edge of the display (r 240)
        let (p, small) = MODE_ICON_BAND;
        let r = ((p.x * p.x + p.y * p.y) as f32).sqrt();
        assert!(small && (218.0..=226.0).contains(&r), "r {r}");
        for core in [
            PageCore::Boxes,
            PageCore::BigBoxes,
            PageCore::TwoValues,
            PageCore::Horizon,
            PageCore::ThermalAssist,
            PageCore::Wind,
        ] {
            assert!(mode_icon_place(core) == Some(MODE_ICON_BAND));
        }
        assert!(mode_icon_place(PageCore::DeviceInfo).is_none());
    }
}

#[cfg(test)]
mod centering_page_tests {
    use super::*;
    use crate::view::test_fixture::{test_model, TestDisplay, DISPLAY_HEIGHT, DISPLAY_WIDTH};
    use crate::view::thermal_data::{ThermalData, HEAT};
    use crate::VarioMode;

    /// a model on a centering page with different circling and cruise sets
    fn model(core: PageCore) -> CoreModel {
        let mut cm = test_model();
        cm.control.alive_ticks = 1000;
        cm.config.overlay_active = OverlayActive::None;
        let i = cm.config.page_idx as usize;
        let p = &mut cm.config.pages[i];
        p.core = core;
        p.cruise_boxes = true;
        p.boxes = [
            LineView::AverageClimbRate,
            LineView::WindAndAvgWind,
            LineView::Altitude,
            LineView::TrueAirSpeed,
            LineView::None,
        ];
        p.stf_boxes = [
            LineView::Altitude,
            LineView::WindAndDelta,
            LineView::AverageClimbRate,
            LineView::SpeedToFly,
            LineView::None,
        ];
        p.center_circling = CenterView::ThermalAssistant3;
        p.center_straight = CenterView::DoubleArrowStraight;
        cm.sensor.average_climb_rate = crate::FloatToSpeed::m_s(1.5_f32);
        cm.control.fly_mode = FlyMode::StraightFlight;
        cm.control.vario_mode = VarioMode::SpeedToFly;
        cm
    }

    fn draw(cm: &CoreModel) -> TestDisplay {
        let mut d = TestDisplay::new();
        let cm_1s = *cm;
        let mut td = ThermalData::default();
        draw_ring(&mut d, cm, &cm_1s, &mut td).expect("the ring page must draw");
        d
    }

    /// pixels of a colour closer to the display centre than `r`
    fn ink_inside(d: &TestDisplay, color: Colors, r: f32) -> usize {
        let mut n = 0;
        for y in 0..DISPLAY_HEIGHT as i32 {
            for x in 0..DISPLAY_WIDTH as i32 {
                let (dx, dy) = ((x - 240) as f32, (y - 240) as f32);
                if dx * dx + dy * dy < r * r
                    && d.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] == Some(color)
                {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn the_centering_page_is_large_only_while_circling() {
        let mut cm = model(PageCore::ThermalAssist);
        assert!(
            !centering_large(&cm),
            "straight flight: the info boxes view"
        );
        cm.control.fly_mode = FlyMode::Circling;
        assert!(centering_large(&cm));
        // the vario mode does not decide: slow straight flight stays small
        cm.control.fly_mode = FlyMode::StraightFlight;
        cm.control.vario_mode = VarioMode::Vario;
        assert!(!centering_large(&cm));
        // another page never
        let mut other = model(PageCore::Boxes);
        other.control.fly_mode = FlyMode::Circling;
        assert!(!centering_large(&other));
        // the page editor: the set that is edited decides, not the flight
        cm.control.layout.active = true;
        cm.control.layout.cruise = false;
        assert!(centering_large(&cm));
        cm.control.fly_mode = FlyMode::Circling;
        cm.control.layout.cruise = true;
        assert!(!centering_large(&cm));
    }

    /// In straight flight back to the standard picture - the same picture,
    /// pixel for pixel
    #[test]
    fn in_straight_flight_the_centering_page_is_the_info_boxes_page() {
        let centering = model(PageCore::ThermalAssist);
        let boxes = model(PageCore::Boxes);
        assert!(
            draw(&centering).buf == draw(&boxes).buf,
            "speed to fly mode: the info boxes page with the cruise set"
        );
        // vario mode in straight flight (slow): still the cruise set, as an
        // info boxes page would show a page whose only set is that one
        let mut centering = model(PageCore::ThermalAssist);
        centering.control.vario_mode = VarioMode::Vario;
        let mut boxes = model(PageCore::Boxes);
        boxes.control.vario_mode = VarioMode::Vario;
        let i = boxes.config.page_idx as usize;
        boxes.config.pages[i].boxes = boxes.config.pages[i].stf_boxes;
        assert!(
            draw(&centering).buf == draw(&boxes).buf,
            "vario mode in straight flight: the cruise set as well"
        );
    }

    #[test]
    fn the_large_view_has_two_boxes_and_a_short_needle() {
        let mut cm = model(PageCore::ThermalAssist);
        cm.control.fly_mode = FlyMode::Circling;
        cm.control.vario_mode = VarioMode::Vario;
        let large = draw(&cm);
        // the right hand boxes of the circling set are not on the page
        let mut other = cm;
        let i = other.config.page_idx as usize;
        other.config.pages[i].boxes[2] = LineView::FlightLevel;
        other.config.pages[i].boxes[3] = LineView::None;
        assert!(
            draw(&other).buf == large.buf,
            "slots 2 and 3 do not show in the large view"
        );
        // top and bottom are
        for slot in [0, 1] {
            let mut other = cm;
            other.config.pages[i].boxes[slot] = LineView::None;
            assert!(
                draw(&other).buf != large.buf,
                "slot {slot} shows in the large view"
            );
        }
        // the cruise set does not matter while circling
        let mut other = cm;
        other.config.pages[i].stf_boxes = [LineView::None; 5];
        assert!(draw(&other).buf == large.buf);

        // the needle stays outside the large aid; on the info boxes view
        // it runs to the centre circle
        let needle = cm.palette().vario.ring_needle;
        assert_eq!(
            ink_inside(&large, needle, CENTERING_NEEDLE - 1.0),
            0,
            "needle inside the large aid"
        );
        let mut straight = cm;
        straight.control.fly_mode = FlyMode::StraightFlight;
        assert!(ink_inside(&draw(&straight), needle, RING_FREE - 1.0) > 200);
    }

    /// largest distance from the display centre of a pixel that differs
    fn reach_of_difference(a: &TestDisplay, b: &TestDisplay) -> f32 {
        let mut far = 0.0_f32;
        for y in 0..DISPLAY_HEIGHT as i32 {
            for x in 0..DISPLAY_WIDTH as i32 {
                let i = (y * DISPLAY_WIDTH as i32 + x) as usize;
                if a.buf[i] != b.buf[i] {
                    let (dx, dy) = ((x - 240) as f32, (y - 240) as f32);
                    far = far.max(crate::math::sqrt(dx * dx + dy * dy));
                }
            }
        }
        far
    }

    /// The needle of the large view begins at r 150: it never enters the
    /// room of the aid, whatever the climb rate, and it runs over the boxes
    /// in the foreground, as on the info boxes page.
    #[test]
    fn the_needle_of_the_large_view_stays_out_of_the_aid_and_runs_over_the_boxes() {
        let mut cm = model(PageCore::ThermalAssist);
        cm.control.fly_mode = FlyMode::Circling;
        cm.control.vario_mode = VarioMode::Vario;
        let i = cm.config.page_idx as usize;
        cm.calculated.pot_climb = Some(crate::FloatToSpeed::m_s(2.8_f32));
        cm.calculated.pot_verdict = crate::model::PotVerdict::Yes;
        cm.config.pages[i].boxes[0] = LineView::PotentialClimb;
        // the boxes reach further out than the inner end of the needle
        let with_boxes = draw(&cm);
        let mut empty = cm;
        empty.config.pages[i].boxes[0] = LineView::None;
        empty.config.pages[i].boxes[1] = LineView::None;
        let reach = reach_of_difference(&with_boxes, &draw(&empty));
        assert!(
            reach > CENTERING_NEEDLE && reach < 178.0,
            "the boxes reach out to r {reach}"
        );

        let at_zero = draw(&cm);
        let needle = cm.palette().vario.ring_needle;
        let mut over_a_box = false;
        for tenth in -50..=99 {
            let mut c = cm;
            c.calculated.interpolated_climb_rate.shown =
                crate::FloatToSpeed::m_s(tenth as f32 * 0.1);
            let d = draw(&c);
            assert_eq!(
                ink_inside(&d, needle, CENTERING_NEEDLE - 1.0),
                0,
                "needle inside at {tenth}"
            );
            // nothing but the needle moves: no change in the room of the aid
            for y in 0..DISPLAY_HEIGHT as i32 {
                for x in 0..DISPLAY_WIDTH as i32 {
                    let (dx, dy) = ((x - 240) as f32, (y - 240) as f32);
                    let k = (y * DISPLAY_WIDTH as i32 + x) as usize;
                    let r2 = dx * dx + dy * dy;
                    if d.buf[k] != at_zero.buf[k] {
                        assert!(
                            r2 >= (CENTERING_NEEDLE - 1.0) * (CENTERING_NEEDLE - 1.0),
                            "the needle at {tenth} changes the aid"
                        );
                    }
                    // foreground: where the needle is, it is not hidden by a box
                    if d.buf[k] == Some(needle)
                        && r2 < reach * reach
                        && with_boxes.buf[k] != Some(Colors::Black)
                        && with_boxes.buf[k] != Some(needle)
                        && with_boxes.buf[k].is_some()
                    {
                        over_a_box = true;
                    }
                }
            }
        }
        assert!(
            over_a_box,
            "the needle runs over the boxes, in the foreground"
        );
        // 86 px of needle: from r 150 to the long tip at r 236
        assert!(
            (crate::view::sprites::ring_needle::R_TIP_LONG - CENTERING_NEEDLE - 86.0).abs() < 0.5
        );
    }

    /// fly `seconds` on the page: heading, speed and climb go into the
    /// thermal data as on the device (one update per 100 ms tick)
    fn fly(
        cm: &mut CoreModel,
        td: &mut ThermalData,
        yaw: &mut f32,
        seconds: f32,
        rate: f32,
        climb: impl Fn(f32) -> f32,
    ) {
        use embedded_graphics::geometry::AngleUnit;
        for _ in 0..(seconds * 10.0 + 0.5) as u32 {
            *yaw += rate * 0.1;
            cm.control.alive_ticks += 1;
            cm.sensor.euler_yaw = yaw.rem_euclid(360.0).deg();
            cm.sensor.turn_rate = crate::FloatToAngularVelocity::rad_s(rate.to_radians());
            cm.sensor.airspeed = crate::AirSpeed::new(28.0, 30.0);
            cm.sensor.climb_rate = crate::FloatToSpeed::m_s(climb(*yaw));
            td.update(cm);
        }
    }

    fn draw_with(cm: &CoreModel, td: &mut ThermalData) -> TestDisplay {
        let mut d = TestDisplay::new();
        let cm_1s = *cm;
        draw_ring(&mut d, cm, &cm_1s, td).expect("the ring page must draw");
        d
    }

    /// Stefan 16.11.2026: the track for the first 1.5 circles of a thermal,
    /// then the aid of the page; a page with the track keeps it.
    #[test]
    fn the_first_one_and_a_half_circles_show_the_track_then_the_aid_of_the_page() {
        let gold = Colors::Gold;
        for aid in [
            CenterView::ThermalAssistant1,
            CenterView::ThermalAssistant2,
            CenterView::ThermalAssistant3,
        ] {
            let mut cm = model(PageCore::ThermalAssist);
            let i = cm.config.page_idx as usize;
            cm.config.pages[i].center_circling = aid;
            cm.control.vario_mode = VarioMode::Vario;
            let (mut td, mut yaw) = (ThermalData::default(), 0.0_f32);
            let climb = |yaw: f32| 2.0 + (yaw.to_radians()).cos();
            fly(&mut cm, &mut td, &mut yaw, 20.0, 0.0, |_| 0.5);
            fly(&mut cm, &mut td, &mut yaw, 7.0, 18.0, climb);
            cm.control.fly_mode = FlyMode::Circling;
            // one circle after the turn began (360 degrees take 20 s)
            fly(&mut cm, &mut td, &mut yaw, 13.5, 18.0, climb);
            assert!(td.trail.first_circles());
            let track = draw_with(&cm, &mut td);
            // the same picture as a page that has the track as its aid
            let mut as_track = cm;
            as_track.config.pages[i].center_circling = CenterView::ThermalAssistant4;
            assert!(
                track.buf == draw_with(&as_track, &mut td).buf,
                "the track, whatever aid the page has"
            );
            // 1.5 circles and a bit: the aid of the page; the track page stays
            fly(&mut cm, &mut td, &mut yaw, 10.5, 18.0, climb);
            assert!(!td.trail.first_circles());
            let after = draw_with(&cm, &mut td);
            let mut as_track = cm;
            as_track.config.pages[i].center_circling = CenterView::ThermalAssistant4;
            let still_track = draw_with(&as_track, &mut td);
            assert!(
                after.buf != still_track.buf,
                "after 1.5 circles the aid of the page"
            );
            // the yellow dot of the track; the edges of the 3D ring are
            // yellow too, dots and spider have none of it
            assert!(ink_inside(&still_track, gold, 176.0) > 50);
            if aid != CenterView::ThermalAssistant3 {
                assert_eq!(ink_inside(&after, gold, 176.0), 0);
            }
            // the page editor shows the aid that is chosen, also in the first circle
            let mut cm = model(PageCore::ThermalAssist);
            cm.config.pages[i].center_circling = aid;
            let (mut td, mut yaw) = (ThermalData::default(), 0.0_f32);
            cm.control.fly_mode = FlyMode::Circling;
            fly(&mut cm, &mut td, &mut yaw, 5.0, 18.0, climb);
            assert!(td.trail.first_circles());
            let flying = draw_with(&cm, &mut td);
            cm.control.layout.active = true;
            cm.control.layout.cruise = false;
            assert!(draw_with(&cm, &mut td).buf != flying.buf);
        }
    }

    /// A clearly larger circle: the whole track must still fit on the
    /// display.
    #[test]
    fn the_whole_track_fits_between_the_boxes_whatever_the_size_of_the_circle() {
        // without the boxes the middle of the page holds nothing but the aid
        for (rate, seconds) in [
            (22.0_f32, 30.0_f32),
            (12.0, 45.0),
            (6.0, 80.0),
            (4.5, 110.0),
        ] {
            let mut cm = model(PageCore::ThermalAssist);
            let i = cm.config.page_idx as usize;
            cm.config.pages[i].center_circling = CenterView::ThermalAssistant4;
            cm.config.pages[i].boxes = [LineView::None; 5];
            cm.control.vario_mode = VarioMode::Vario;
            let (mut td, mut yaw) = (ThermalData::default(), 40.0_f32);
            let climb = |yaw: f32| 1.5 + 2.0 * (yaw.to_radians()).sin();
            fly(&mut cm, &mut td, &mut yaw, 15.0, 0.0, |_| -1.0);
            fly(&mut cm, &mut td, &mut yaw, 7.0, rate, climb);
            cm.control.fly_mode = FlyMode::Circling;
            let place = centering_place(&cm);
            let (top, bottom) = (
                place.center.y as f32 - place.ring - 3.0,
                place.center.y as f32 + place.ring + 3.0,
            );
            // changes of direction of the scale (out, then in again, ...)
            let mut reversals = 0;
            let mut last = 0;
            let mut going_out = true;
            let mut t = 0.0;
            while t < seconds {
                fly(&mut cm, &mut td, &mut yaw, 1.0, rate, climb);
                t += 1.0;
                let d = draw_with(&cm, &mut td);
                let with_aid = &d.buf;
                let mut dots = 0;
                for y in 0..DISPLAY_HEIGHT as i32 {
                    for x in 0..DISPLAY_WIDTH as i32 {
                        let (dx, dy) = ((x - 240) as f32, (y - 240) as f32);
                        let c = with_aid[(y * DISPLAY_WIDTH as i32 + x) as usize];
                        let track = matches!(c, Some(col) if col == Colors::Gold || col == Colors::DimGray || HEAT.contains(&col));
                        if track && dx * dx + dy * dy < 176.0 * 176.0 {
                            dots += 1;
                            assert!(
                                (y as f32) >= top && (y as f32) <= bottom,
                                "a dot at row {y} at {rate} degrees per second, {t} s"
                            );
                        }
                    }
                }
                assert!(
                    dots > 20,
                    "the track is on the page ({dots} pixels at {t} s)"
                );
                if td.trail.scale_step != last {
                    let out = td.trail.scale_step > last;
                    if out != going_out {
                        reversals += 1;
                        going_out = out;
                    }
                    last = td.trail.scale_step;
                }
            }
            // The scale does not pump: out while the track grows, in again
            // at most once, when the approach has left the track.
            assert!(
                reversals <= 1,
                "{reversals} reversals of the scale at {rate} degrees per second"
            );
        }
    }

    /// No flap display needed while circling.
    /// While circling a page is drawn as if its flap tape were switched off.
    #[test]
    fn no_flap_tape_while_circling() {
        for core in [
            PageCore::Boxes,
            PageCore::ThermalAssist,
            PageCore::BigBoxes,
            PageCore::TwoValues,
        ] {
            let mut cm = model(core);
            let basic = *crate::polar_store::POLARS
                .iter()
                .find(|p| p.name == "AS 33 18m")
                .unwrap();
            cm.glider_data = crate::GliderData::from_basic_glider_data(basic);
            cm.calculated.flap_rec = Some(5);
            cm.calculated.flap_actual = Some(4.6);
            assert!(
                crate::view::flap_tape::TapeState::of(&cm).is_some(),
                "the glider has a flap tape"
            );
            let i = cm.config.page_idx as usize;
            let without = |cm: &CoreModel| {
                let mut off = *cm;
                off.config.pages[i].flap_tape_off = true;
                draw(&off)
            };
            // straight flight: the tape is on the page
            assert!(
                draw(&cm).buf != without(&cm).buf,
                "{core:?}: tape in straight flight"
            );
            // circling: no tape
            cm.control.fly_mode = FlyMode::Circling;
            assert!(
                draw(&cm).buf == without(&cm).buf,
                "{core:?}: no tape while circling"
            );
            // the page editor: the set that is edited decides
            cm.control.layout.active = true;
            cm.control.layout.cruise = true;
            assert!(
                draw(&cm).buf != without(&cm).buf,
                "{core:?}: tape with the cruise set in the editor"
            );
            cm.control.layout.cruise = false;
            assert!(
                draw(&cm).buf == without(&cm).buf,
                "{core:?}: no tape with the circling set in the editor"
            );
        }
    }

    #[test]
    fn the_small_bottom_box_sits_further_out_and_the_aid_gains_the_room() {
        let standard_end = Y_TOP_BOTTOM as f32 + p1_block_h(P1_LADDER[0]);
        let small_end = centering_bottom_y() + p1_block_h(CENTERING_BOTTOM_LADDER[0]);
        assert!((small_end - CENTERING_BOTTOM_END).abs() < 0.01);
        assert!(
            small_end > standard_end + 10.0,
            "further out than the large bottom box ({standard_end})"
        );
        // inside the area the info boxes may use, with room for a value
        // of five digits (about 110 px in the 38 px font)
        let r_use = R_USE_P1 as f32;
        let half = (r_use * r_use - small_end * small_end).sqrt();
        assert!(
            small_end < r_use && half >= 59.0,
            "half chord at the end of the box: {half}"
        );
        assert!(
            centering_bottom_y() > Y_TOP_BOTTOM as f32 + 25.0,
            "the band for the aid gains from the smaller box"
        );
        // every size of the small box is smaller than the large top box
        assert!(CENTERING_BOTTOM_LADDER.iter().all(|s| *s < P1_LADDER[0]));
        // the band: below the top box, above the bottom box
        let cm = model(PageCore::ThermalAssist);
        let place = centering_place(&cm);
        let top = 240.0 - Y_TOP_BOTTOM as f32;
        let bottom = 240.0 + centering_bottom_y();
        let (c, r) = (place.center.y as f32, place.ring + 3.0);
        assert!(
            c - r >= top + 3.0 && c + r <= bottom - 5.0,
            "dots and spider between the boxes: {} .. {}",
            c - r,
            c + r
        );
        assert!(
            place.radius > 1.3 * AidPlace::centre_circle(&cm).radius,
            "larger than in the centre circle"
        );
        assert!(
            place.ring3d_unit > 1.9 * AidPlace::centre_circle(&cm).ring3d_unit,
            "3D ring twice as large"
        );
        assert!(place.ring3d_edge == crate::view::viewable::centerview::RING_EDGE_LARGE);
        assert!(
            AidPlace::centre_circle(&cm).ring3d_edge
                == crate::view::viewable::centerview::RING_EDGE_SMALL
        );
    }
}
