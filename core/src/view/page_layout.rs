//! Page layout editor, drawing (plan_seiten_editor.md, sketch
//! skizze_seiten_editor.png). The page itself is drawn by its normal view;
//! the zone highlights inside the page come from vario_ring / flap_tape
//! (`controller::page_layout::highlight`). This module draws what lies on
//! top: the HUD in the centre circle and the choice list.

use crate::{
    controller::page_layout as pl,
    model::{
        page::{PageCore, PageFrame},
        page_layout::{Choice, ListKind, Zone},
        CoreModel,
    },
    utils::Colors,
    view::{
        fonts_p1::{render_value, text_for, VALUE_19},
        ring_menu::{clip_to, isqrt},
        viewable::ring_infobox::r2_value,
    },
    CoreError, DrawImage,
};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle, RoundedRectangle},
};
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

/// highlight colours: fill of a zone and its outline
pub const HL_FILL: Colors = Colors::SkyK1Dark;
pub const HL_LINE: Colors = Colors::SkyK1;

/// radius of the HUD in the centre circle (inside the wallpaper's ring)
const HUD_R: i32 = 74;
/// rows of the choice list
const ROW_PITCH: i32 = 52;

fn text_w(font: &u8g2_fonts::FontRenderer, s: &str) -> i32 {
    font.get_rendered_dimensions(s, Point::zero(), VerticalPosition::Center)
        .map(|d| d.advance.x)
        .unwrap_or(0)
}

/// "Page 2 of 4": position among the pages that are on
fn page_of(cm: &CoreModel) -> heapless::String<16> {
    let mut n = 0;
    let mut pos = 0;
    for (i, p) in cm.config.pages.iter().enumerate() {
        if p.is_on() {
            n += 1;
            if i as u8 == cm.config.page_idx {
                pos = n;
            }
        }
    }
    let mut s = heapless::String::new();
    let _ = core::fmt::write(&mut s, format_args!("Page {pos} of {n}"));
    s
}

fn mode_str(cm: &CoreModel) -> &'static str {
    if pl::cruise_shared(cm) {
        // no own cruise set yet: the first change makes it
        "cruise = circling"
    } else if cm.control.layout.cruise {
        "cruise"
    } else {
        "circling"
    }
}

/// a zone where the flight state (circling / cruise) matters
fn per_mode(zone: Zone) -> bool {
    matches!(zone, Zone::Mode | Zone::Centre | Zone::Box(_))
}

/// everything the editor draws on top of the page
pub fn draw<D>(display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    if !pl::active(cm) {
        return Ok(());
    }
    if cm.control.layout.list != ListKind::None {
        return draw_list(display, cm);
    }
    let c = cm.device_const.sizes.display.center;
    let page = cm.config.page();
    let zone = pl::zone(cm);
    let full = page.frame == PageFrame::Full || page.core == PageCore::DeviceInfo;

    // zones the page view cannot show itself: outlines on top
    let ring_core = page.core.with_values();
    match zone {
        Zone::Scale if full => {
            // full screen horizon: the scale would be around the edge
            Circle::with_center(c, 2 * 232)
                .into_styled(PrimitiveStyle::with_stroke(HL_LINE, 10))
                .draw(display)?;
        }
        Zone::Core if full || !ring_core => {
            // full screen: the horizon owns the edge (that is the scale
            // zone), the core outline sits further in; the status page
            // has no scale, its outline runs along the edge
            let r = match (full, page.core) {
                (true, PageCore::DeviceInfo) => 228,
                (true, _) => 196,
                _ => 190,
            };
            Circle::with_center(c, (2 * r) as u32)
                .into_styled(PrimitiveStyle::with_stroke(HL_LINE, 5))
                .draw(display)?;
        }
        _ => (),
    }

    // the big values own the middle: no HUD while one of them is chosen
    if page.core.big() && matches!(zone, Zone::Box(_)) {
        return Ok(());
    }
    // the centre shows what it will be (drawn by the page), only a line
    // around it
    if zone == Zone::Centre && crate::view::vario_ring::centering_large(cm) {
        // the large centering aid of the centering page: a line around it
        let place = crate::view::vario_ring::centering_place(cm);
        if page.centering_view() == crate::view::viewable::centerview::CenterView::ThermalAssistant3
        {
            // the 3D ring is wider than high: a frame around its band
            let (x, y) = (place.center.x, place.center.y);
            let half_w = (0.8 * place.ring3d_unit) as i32 + 12;
            let half_h = place.ring as i32 + 5;
            RoundedRectangle::with_equal_corners(
                Rectangle::new(
                    Point::new(x - half_w, y - half_h),
                    Size::new(2 * half_w as u32, 2 * half_h as u32),
                ),
                Size::new(10, 10),
            )
            .into_styled(PrimitiveStyle::with_stroke(HL_LINE, 4))
            .draw(display)?;
        } else {
            let r = place.ring + 6.0;
            Circle::with_center(place.center, (2.0 * r) as u32 + 1)
                .into_styled(PrimitiveStyle::with_stroke(HL_LINE, 4))
                .draw(display)?;
        }
        return Ok(());
    }
    if zone == Zone::Centre {
        Circle::with_center(c, (2 * HUD_R + 1) as u32)
            .into_styled(PrimitiveStyle::with_stroke(HL_LINE, 4))
            .draw(display)?;
        let view = if cm.control.layout.cruise {
            page.center_straight
        } else {
            page.center_circling
        };
        if view == crate::view::viewable::centerview::CenterView::None {
            text_for(19).f.render_aligned(
                "Off",
                c,
                VerticalPosition::Center,
                HorizontalAlignment::Center,
                FontColor::Transparent(Colors::DarkGray),
                display,
            )?;
        }
        return Ok(());
    }
    draw_hud(display, cm, zone)
}

/// centre circle: "Page 1 of 4", the zone, circling / cruise
fn draw_hud<D>(display: &mut D, cm: &CoreModel, zone: Zone) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let c = cm.device_const.sizes.display.center;
    let page = cm.config.page();
    let fill = if zone == Zone::Exit {
        HL_FILL
    } else {
        cm.palette().vario.background
    };
    Circle::with_center(c, (2 * HUD_R + 1) as u32)
        .into_styled(PrimitiveStyle::with_fill(fill))
        .draw(display)?;
    if matches!(zone, Zone::Page | Zone::Exit | Zone::Mode) {
        Circle::with_center(c, (2 * HUD_R + 1) as u32)
            .into_styled(PrimitiveStyle::with_stroke(HL_LINE, 3))
            .draw(display)?;
    }
    let p = page_of(cm);
    text_for(14).f.render_aligned(
        p.as_str(),
        c + Point::new(0, -34),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(Colors::DarkGray),
        display,
    )?;
    // the zone name, one size smaller when it is long ("Box right 1")
    let name = zone.name_on(page);
    let big = text_for(22).f;
    let f = if text_w(big, name) <= 2 * HUD_R - 16 {
        big
    } else {
        text_for(19).f
    };
    f.render_aligned(
        name,
        c,
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(cm.palette().vario.scale),
        display,
    )?;
    // last line: what a push does on "Save & exit", else circling / cruise
    // where the page has two sets
    let line = if zone == Zone::Exit {
        "push"
    } else if page.core.two_sets() {
        mode_str(cm)
    } else {
        return Ok(());
    };
    text_for(14).f.render_aligned(
        line,
        c + Point::new(0, 33),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(HL_LINE),
        display,
    )?;
    Ok(())
}

/// value preview of a box choice ("1250")
fn row_value(cm: &CoreModel, choice: Choice) -> heapless::String<20> {
    match choice {
        Choice::Box(lv) if lv != crate::LineView::None => r2_value(cm, lv),
        _ => heapless::String::new(),
    }
}

/// the choice list: covers the core like the flight menu, the page goes on
/// around it (scale preview), the selected row in the orange frame
fn draw_list<D>(display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let pal = &cm.palette().vario;
    let c = cm.device_const.sizes.display.center;
    // the whole black core, no line around it (like the flight menu)
    let radius = crate::view::ring_menu::overlay_disc(display, cm)?;

    let l = &cm.control.layout;
    let zone = pl::zone(cm);
    let page = cm.config.page();
    let p = page_of(cm);
    let (title, sub): (&str, Option<&str>) = match l.list {
        ListKind::Exit if pl::changed(cm) => ("Page Layout", Some("not saved yet")),
        ListKind::Exit => ("Page Layout", None),
        _ if zone == Zone::Page => (p.as_str(), None),
        _ if per_mode(zone) => (zone.name_on(page), Some(mode_str(cm))),
        _ => (zone.name_on(page), None),
    };
    let cap = crate::view::ring_menu::CAP_OVERLAY;
    crate::view::ring_menu::header(display, c, radius, cap, title)?;
    let mut top_lim = c.y + cap + 4;
    if let Some(s) = sub {
        text_for(14).f.render_aligned(
            s,
            c + Point::new(0, cap + 16),
            VerticalPosition::Center,
            HorizontalAlignment::Center,
            FontColor::Transparent(HL_LINE),
            display,
        )?;
        top_lim += 26;
    }

    let rows = pl::rows(cm);
    let n = rows.len() as i32;
    let sel = l.sel as i32;
    let h2 = ROW_PITCH / 2;
    for rel in -3i32..=3 {
        let idx = sel + rel;
        if idx < 0 || idx >= n {
            continue;
        }
        let row = rows[idx as usize];
        let y = c.y + rel * ROW_PITCH + 30;
        if y - h2 < top_lim || y + h2 > c.y + radius - 8 {
            continue;
        }
        let selected = rel == 0;
        let edge = if selected { h2 } else { 12 };
        let half =
            isqrt(((radius - 10) * (radius - 10) - ((y - c.y).abs() + edge).pow(2)).max(0)) - 6;
        let (col, vcol) = if selected {
            (pal.scale, pal.ring_select)
        } else if rel.abs() == 1 {
            (Colors::DarkGray, Colors::DarkGray)
        } else {
            (Colors::DimGray, Colors::DimGray)
        };
        if selected {
            RoundedRectangle::with_equal_corners(
                Rectangle::new(
                    Point::new(c.x - half, y - h2 + 1),
                    Size::new((2 * half) as u32, (2 * h2 - 2) as u32),
                ),
                Size::new(8, 8),
            )
            .into_styled(PrimitiveStyle::with_stroke(pal.ring_select, 2))
            .draw(display)?;
        }
        // the choice the list was opened with: white with a small dot in
        // front, so it can be found again (there is no Esc on the display).
        // Not blue - blue is the selection frame now.
        let prev = !selected && l.list == ListKind::Zone && idx == l.orig_sel as i32;
        let col = if prev { pal.scale } else { col };
        let nf = text_for(25).f;
        let dot_w = if prev { 14 } else { 0 };
        if prev {
            Circle::with_center(Point::new(c.x - half + 16, y), 7)
                .into_styled(PrimitiveStyle::with_fill(pal.scale))
                .draw(display)?;
        }
        let inner = 2 * half - 28 - dot_w;
        let val = row_value(cm, row.choice);
        let vw = if val.is_empty() {
            0
        } else {
            text_w(&VALUE_19, val.as_str())
        };
        let show_val = vw > 0 && text_w(nf, row.label) + vw + 16 <= inner;
        // a long name near the rim is cut at the end, never beyond the circle
        let label = clip_to(nf, row.label, inner);
        nf.render_aligned(
            label.as_str(),
            Point::new(c.x - half + 14 + dot_w, y),
            VerticalPosition::Center,
            HorizontalAlignment::Left,
            FontColor::Transparent(col),
            display,
        )?;
        // page rows: what the page is, in the text font
        if let Choice::GoTo(i) = row.choice {
            let core = cm.config.pages[(i as usize).min(crate::model::MAX_PAGES - 1)].core;
            let name = crate::model::page_layout::CORES
                .iter()
                .find(|(c, _)| *c == core)
                .map(|(_, n)| *n)
                .unwrap_or("");
            let tf = text_for(19).f;
            if text_w(nf, row.label) + text_w(tf, name) + 16 <= inner {
                tf.render_aligned(
                    name,
                    Point::new(c.x + half - 14, y),
                    VerticalPosition::Center,
                    HorizontalAlignment::Right,
                    FontColor::Transparent(if selected { vcol } else { col }),
                    display,
                )?;
            }
        }
        if show_val {
            render_value(
                display,
                &VALUE_19,
                val.as_str(),
                Point::new(c.x + half - 14, y),
                VerticalPosition::Center,
                HorizontalAlignment::Right,
                vcol,
            )?;
        }
    }
    Ok(())
}

/// a highlighted info box: blue field with a light blue line (drawn under
/// the value). `x0..x1`, `y0..y1` in display coordinates.
pub fn box_frame<D>(display: &mut D, x0: i32, y0: i32, x1: i32, y1: i32) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    let r = RoundedRectangle::with_equal_corners(
        Rectangle::new(
            Point::new(x0, y0),
            Size::new((x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32),
        ),
        Size::new(10, 10),
    );
    r.into_styled(PrimitiveStyle::with_fill(HL_FILL))
        .draw(display)?;
    r.into_styled(PrimitiveStyle::with_stroke(HL_LINE, 3))
        .draw(display)?;
    Ok(())
}

/// an empty slot while it is highlighted: a frame with "Empty"
pub fn empty_slot<D>(display: &mut D, x: i32, y0: i32, half: i32, h: i32) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    box_frame(display, x - half, y0, x + half, y0 + h)?;
    text_for(18).f.render_aligned(
        "Empty",
        Point::new(x, y0 + h / 2),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(Colors::LightGray),
        display,
    )?;
    Ok(())
}
