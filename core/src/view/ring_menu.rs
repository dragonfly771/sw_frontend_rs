//! Menu renderer for the Stefly (ring) design: fisheye lists that follow
//! the round display, values shown directly in the list, the header in a
//! dark blue cap at the top, the position small at the bottom, and the
//! flight menu over the black core of the page while the vario scale keeps
//! running.
//!
//! Overflow policy (Stefan): a row never leaves the circle and never
//! collides with the header. If name and value do not fit side by side,
//! the VALUE is dropped completely (never partially cut); only the name
//! may be tail clipped as a last resort.

use crate::view::fonts_p1::{render_value, text_for, VALUE_16, VALUE_19, VALUE_21, VALUE_24};
use crate::Editable;
use crate::{
    controller::menu::MENU_VALUE_ROWS,
    model::{
        menu::{Menu, MenuItemContent},
        CoreModel,
    },
    utils::Colors,
    view::sprites::menu_icons::{draw_icon, icon_for, MenuIcon},
    CoreError, DrawImage,
};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle, RoundedRectangle, Triangle},
};
#[allow(unused_imports)]
use micromath::F32Ext;
use u8g2_fonts::{
    types::{FontColor, HorizontalAlignment, VerticalPosition},
    FontRenderer,
};

const SETUP_ROW_PITCH: i32 = 56;

/// Typography and geometry of the flight menu over the page
#[derive(Clone, Copy)]
pub struct FlightStyle {
    /// distance of the rows
    pub pitch: i32,
    /// centre of the selected row below the display centre
    pub sel_y: i32,
    /// names (Barlow) and values (B612)
    pub name: &'static FontRenderer,
    pub value: &'static FontRenderer,
    /// the outer rows (two away from the cursor), where the circle is narrow
    pub outer_name: &'static FontRenderer,
    pub outer_value: &'static FontRenderer,
    /// half the ink height of a name: rows that are not selected only need
    /// room for their text
    pub text_h2: i32,
    /// the long names ("Gust Filter") when they fit, else the short ones
    pub full_names: bool,
}

/// up to 0.4.1.44: Barlow 18 / B612 21, five rows, short names (kept for
/// the comparison renders)
#[allow(dead_code)]
pub const FLIGHT_STYLE_044: FlightStyle = FlightStyle {
    pitch: 54,
    sel_y: 26,
    name: &crate::view::fonts_p1::TEXT_18,
    value: &VALUE_21,
    outer_name: &crate::view::fonts_p1::TEXT_18,
    outer_value: &VALUE_21,
    text_h2: 12,
    full_names: false,
};

/// since 0.4.1.45 (Stefan 09.11.2026: larger, the black left and right is
/// wasted): Barlow 26 / B612 27 (+44 % / +29 %) for the cursor row and its
/// neighbours, Barlow 22 / B612 24 for the two outer rows where the circle
/// is narrow, five rows, full names where they fit
pub const FLIGHT_STYLE: FlightStyle = FlightStyle {
    pitch: 56,
    sel_y: 24,
    name: &crate::utils::larus_fonts::TEXT_26,
    value: &crate::utils::larus_fonts::VALUE_27,
    outer_name: &crate::utils::larus_fonts::TEXT_22,
    outer_value: &VALUE_24,
    text_h2: 15,
    full_names: true,
};
const NAME_VALUE_GAP: i32 = 16;
const CHEVRON_SLOT: i32 = 26;

/// The setup lists use the type sizes of the flight menu and the width of
/// the display (Stefan 16.11.2026: the setup was small next to the editors
/// and left the sides black; easier to read with weaker eyes). Before:
/// Barlow 18 / B612 19 between x = 76 and 404.
/// Cursor row and its neighbours Barlow 26 / B612 27, the outer rows Barlow
/// 22 / B612 24. No new font: all of them are in the flight menu already.
const SETUP_LEFT: i32 = 52;
const SETUP_RIGHT: i32 = 428;
/// text after an icon, and the icon itself
const SETUP_ICON_TEXT: i32 = 102;
const SETUP_ICON_X: i32 = 72;
/// The rows keep this distance to the round rim, about what the cursor row
/// has in the middle - so the left and right edge of the list follow the
/// circle and the lower rows move in (Stefan 16.11.2026: the margin must
/// look the same at the bottom as in the middle).
const SETUP_RIM: i32 = 46;

/// Left and right end of a setup row whose centre lies `dy` below the
/// display centre: the full width in the middle, following the round rim
/// further up and down.
fn setup_row_span(center_x: i32, dy: i32, half_height: i32) -> (i32, i32) {
    let r = center_x - 2;
    let h = dy.abs() + half_height;
    let w2 = (r * r - h * h).max(0);
    // integer square root (no float needed for a layout)
    let mut w = 0;
    while (w + 1) * (w + 1) <= w2 {
        w += 1;
    }
    (
        (center_x - w + SETUP_RIM).max(SETUP_LEFT),
        (center_x + w - SETUP_RIM).min(SETUP_RIGHT),
    )
}

/// value fallback: strip a trailing unit ("45 kg" -> "45") so at least
/// the number stays visible when the row is tight
fn strip_unit(val: &str) -> Option<&str> {
    let idx = val.rfind(' ')?;
    let (num, unit) = (&val[..idx], &val[idx + 1..]);
    let num_has_digit = num.chars().any(|c| c.is_ascii_digit());
    let unit_has_digit = unit.chars().any(|c| c.is_ascii_digit());
    if !num.is_empty() && num_has_digit && !unit_has_digit {
        Some(num)
    } else {
        None
    }
}

/// real rendered text width; falls back to a generous estimate
fn text_w(font: &FontRenderer, s: &str) -> i32 {
    if s.is_empty() {
        return 0;
    }
    font.get_rendered_dimensions(s, Point::zero(), VerticalPosition::Center)
        .map(|d| d.advance.x)
        .unwrap_or(s.chars().count() as i32 * 14)
}

/// tail clip so the REAL rendered width fits into max_px
pub(crate) fn clip_to(font: &FontRenderer, s: &str, max_px: i32) -> heapless::String<24> {
    let mut out: heapless::String<24> = heapless::String::new();
    for c in s.chars().take(24) {
        let _ = out.push(c);
    }
    while !out.is_empty() && text_w(font, out.as_str()) > max_px {
        let _ = out.pop();
    }
    out
}

/// Exact integer square root.
///
/// `f32::sqrt` is micromath's fast bit trick on the target and is off by
/// more than one percent - at a radius of 150 that is two pixels. Anything
/// that positions something against the round display edge must not use it.
pub(crate) fn isqrt(v: i32) -> i32 {
    if v <= 0 {
        return 0;
    }
    let mut x = v;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + v / x) / 2;
    }
    x
}

fn chord_half(r: i32, dy: i32) -> i32 {
    isqrt((r * r - dy * dy).max(0))
}

/// short labels for the narrow lists; the opened editor shows the full name
fn short_name(name: &str) -> &str {
    match name {
        "Water Ballast" => "Ballast",
        "Gust Filter" => "Gust",
        "Brightness" => "Dim",
        _ => name,
    }
}

fn value_str(cm: &CoreModel, idx: usize) -> &str {
    if idx < MENU_VALUE_ROWS {
        cm.control.menu_control.values[idx].as_str()
    } else {
        ""
    }
}

/// chevron of a sub menu (sketch: 18 px high selected, 14 px otherwise)
fn chevron<D>(display: &mut D, x: i32, y: i32, s: i32, color: Colors) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    Triangle::new(
        Point::new(x, y - s),
        Point::new(x + s + 3, y),
        Point::new(x, y + s),
    )
    .into_styled(PrimitiveStyle::with_fill(color))
    .draw(display)?;
    Ok(())
}

/// colour of a value that reports an error (sketch skizze_flarm_menue.png:
/// colour only for errors)
const ERR: Colors = Colors::Tomato;

/// a value of this menu row reports an error
fn value_error(cm: &CoreModel, item: &crate::model::menu::MenuItem) -> bool {
    use crate::model::flarm_show::FlarmPortStatus;
    let f = &cm.calculated.flarm;
    match item.content {
        MenuItemContent::EditItem(Editable::FlarmStatus) => {
            !f.demo && f.status != FlarmPortStatus::Ok
        }
        MenuItemContent::EditItem(Editable::FlarmBaud) => {
            !f.demo && cm.config.flarm.fixed_baud().is_none() && !f.baud_locked
        }
        _ => false,
    }
}

/// fill of the header cap (the dark band of the value editors before)
pub(crate) const CAP_FILL: Colors = Colors::SkyK1Deep;
/// lower edge of the header cap, relative to the centre: full screen lists
/// and editors (circle r 240) resp. the overlays on the page (black core)
pub(crate) const CAP_FULL: i32 = -156;
pub(crate) const CAP_OVERLAY: i32 = -118;

/// Header of every menu, list and editor (Stefan 28.10.2026): the top of the
/// circle `r` around `center` down to `bottom` (relative to the centre) is
/// filled dark blue, the title in it white, in mixed case, Barlow 22 px (18
/// px if it does not fit). No line below any more.
pub(crate) fn header<D>(
    display: &mut D,
    center: Point,
    r: i32,
    bottom: i32,
    text: &str,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    for dy in -r..=bottom {
        let h = chord_half(r, dy);
        if h > 0 {
            display.fill_solid(
                &Rectangle::new(
                    Point::new(center.x - h, center.y + dy),
                    Size::new((2 * h + 1) as u32, 1),
                ),
                CAP_FILL,
            )?;
        }
    }
    let big = &crate::utils::larus_fonts::TEXT_22;
    let small = &crate::utils::larus_fonts::TEXT_18;
    // baseline a little below the middle of the cap (the cap is narrow on top)
    let seg = r + bottom;
    let base = center.y + bottom - (seg - 22) * 2 / 5;
    let room = |cap: i32| 2 * chord_half(r, base - cap - center.y) - 32;
    let font = if text_w(big, text) <= room(22) {
        big
    } else {
        small
    };
    let t = clip_to(font, text, room(18));
    font.render_aligned(
        t.as_str(),
        Point::new(center.x, base),
        VerticalPosition::Baseline,
        HorizontalAlignment::Center,
        FontColor::Transparent(Colors::White),
        display,
    )?;
    Ok(())
}

/// a title without the cap (name entry: inside the ring of letters)
pub(crate) fn title<D>(display: &mut D, center: Point, y: i32, text: &str) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    crate::utils::larus_fonts::TEXT_22.render_aligned(
        text,
        center + Point::new(0, y),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(Colors::White),
        display,
    )?;
    Ok(())
}

/// Radius of the overlays on the page (flight menu, value editors, volume,
/// Page Layout lists): the whole black core of the page (Stefan 28.10.2026:
/// no smaller circle with a grey line in it) - up to the scale line on a
/// dark page, up to the light band on a light one.
pub(crate) fn overlay_radius(cm: &CoreModel) -> i32 {
    if cm.config.page().light_outer {
        192
    } else {
        199
    }
}

/// the black disc of an overlay, returns its radius
pub(crate) fn overlay_disc<D>(display: &mut D, cm: &CoreModel) -> Result<i32, CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    let r = overlay_radius(cm);
    let c = cm.device_const.sizes.display.center;
    if r < 199 {
        // light page: the flap tape reaches into the white band - without
        // the core around it only a sliver of it would remain, so the band
        // is white there as on the rest of the circle
        let (r_in, r_out) = (r * r, 200 * 200);
        let dy_max = (200.0 * crate::view::flap_tape::HALF_DEG.to_radians().sin()) as i32 + 2;
        for dy in -dy_max..=dy_max {
            let x0 = isqrt((r_in - dy * dy).max(0));
            let x1 = isqrt(r_out - dy * dy - 1);
            if x1 >= x0 {
                display.fill_solid(
                    &Rectangle::new(
                        Point::new(c.x + x0, c.y + dy),
                        Size::new((x1 - x0 + 1) as u32, 1),
                    ),
                    Colors::White,
                )?;
            }
        }
    }
    for dy in -r..=r {
        let h = chord_half(r, dy);
        display.fill_solid(
            &Rectangle::new(
                Point::new(c.x - h, c.y + dy),
                Size::new((2 * h + 1) as u32, 1),
            ),
            cm.palette().vario.background,
        )?;
    }
    Ok(r)
}

/// Position in a long list, small at the bottom ("3 / 9") - instead of the
/// arc at the rim (Stefan 28.10.2026: no round progress bars)
pub(crate) fn list_position<D>(
    display: &mut D,
    center: Point,
    y: i32,
    pos: usize,
    len: usize,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    if len < 2 {
        return Ok(());
    }
    let t = crate::tformat!(12, "{} / {}", pos + 1, len).unwrap_or_default();
    position_text(display, center, y, t.as_str())
}

/// the small grey position text ("3 / 9") of lists and choice editors
pub(crate) fn position_text<D>(
    display: &mut D,
    center: Point,
    y: i32,
    t: &str,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    // Barlow 14 (was 11): readable, still clearly secondary
    text_for(19).f.render_aligned(
        t,
        Point::new(center.x, center.y + y),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(Colors::DimGray),
        display,
    )?;
    Ok(())
}

/// the "back" mark in front of Return (there is no Esc key)
fn back_chevron<D>(display: &mut D, x: i32, y: i32, s: i32, color: Colors) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    Triangle::new(
        Point::new(x + s + 3, y - s),
        Point::new(x, y),
        Point::new(x + s + 3, y + s),
    )
    .into_styled(PrimitiveStyle::with_fill(color))
    .draw(display)?;
    Ok(())
}

/// full screen setup menu in the ring design (sketches skizze_flarm_menue,
/// skizze_klappen_menue, skizze_menue_ueberschrift): rows 56 px, names
/// sketch fub25 (u8g2 fub20, the outer rows fub17), values sketch inb27
/// (u8g2 inb21), header sketch fub22 (fub17) in mixed case
pub fn draw_setup_list<D>(display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let pal = &cm.palette().vario;
    let center = cm.device_const.sizes.display.center;
    display.clear(pal.background)?;

    let menu: &Menu = cm.control.menu_control.menu;
    let pos = cm.control.menu_control.pos[menu.level];
    let with_icons = icon_for(menu.name) == MenuIcon::Setup;
    let ftab = cm.flap_table();
    // the flap table has its own layout (columns)
    if menu.name == "Flap Stages" {
        return draw_flap_table(display, cm, menu, pos);
    }
    let vis = crate::model::menu::visual_order(cm, menu);
    let vpos = vis.iter().position(|&i| i == pos).unwrap_or(0) as i32;

    // header: the stage editor shows the name of its stage ("Flap N")
    let mut head: heapless::String<24> = heapless::String::new();
    if menu.name == "Flap Stage" {
        let _ = head.push_str("Flap ");
        let _ = head.push_str(ftab.name(cm.config.flap_edit as usize));
    } else {
        let _ = head.push_str(menu.name);
    }
    header(display, center, 240, CAP_FULL, head.as_str())?;

    let n = vis.len() as i32;
    for rel in -3i32..=3 {
        let vi = vpos + rel;
        if vi < 0 || vi >= n {
            continue;
        }
        let idx = vis[vi as usize] as i32;
        let item = menu.items[idx as usize];
        let dy = rel * SETUP_ROW_PITCH;
        let y = center.y + dy - 10;
        // never collide with the header or leave the lower rim
        if y - SETUP_ROW_PITCH / 2 < center.y + CAP_FULL + 4
            || y + SETUP_ROW_PITCH / 2 > center.y + 212
        {
            continue;
        }
        let selected = rel == 0;
        let near = rel.abs() <= 1;
        let val = value_str(cm, idx as usize);
        let icon = if with_icons {
            icon_for(item.name(cm))
        } else {
            MenuIcon::None
        };
        let big: &FontRenderer = &crate::utils::larus_fonts::TEXT_26;
        let mid: &FontRenderer = &crate::utils::larus_fonts::TEXT_22;
        let v27: &FontRenderer = &crate::utils::larus_fonts::VALUE_27;
        // the row follows the round rim
        let (row_l, row_r) = setup_row_span(center.x, y - center.y, if near { 15 } else { 13 });
        let text_x = if icon == MenuIcon::None {
            row_l
        } else {
            row_l + (SETUP_ICON_TEXT - SETUP_LEFT)
        };
        let inner_w = row_r - text_x;
        // Return: a back mark in front (there is no Esc key)
        let back = matches!(item.content, MenuItemContent::EditItem(Editable::Return));
        let back_w = if back { 28 } else { 0 };

        // Fit with REAL text widths. Tried in this order: the full name in
        // the large type, the full name one size smaller, the short name.
        // For each: the value in falling sizes, with its unit first - a
        // value is never cut. A sub menu keeps room for its chevron.
        let long = item.name(cm);
        let name_fonts: [&FontRenderer; 2] = if near {
            [big, mid]
        } else {
            [mid, &crate::view::fonts_p1::TEXT_18]
        };
        let value_fonts: [&FontRenderer; 4] = if near {
            [v27, &VALUE_24, &VALUE_21, &VALUE_19]
        } else {
            [&VALUE_24, &VALUE_21, &VALUE_19, &VALUE_16]
        };
        let wants_value = !val.is_empty() && !item.is_menu();
        let num = strip_unit(val);
        let mut name: &str = long;
        let mut name_font: &FontRenderer = name_fonts[0];
        let mut val_show: &str = "";
        let mut vf: &FontRenderer = value_fonts[0];
        let mut found = !wants_value;
        if wants_value {
            'fit: for nm in [long, short_name(long)] {
                for nf in name_fonts {
                    let nw = text_w(nf, nm) + back_w;
                    // the unit is kept down to the third size ("500 Hz"
                    // one size smaller rather than a large bare "500")
                    let [f0, f1, f2, f3] = value_fonts;
                    for (f, t) in [
                        (f0, Some(val)),
                        (f1, Some(val)),
                        (f2, Some(val)),
                        (f0, num),
                        (f1, num),
                        (f2, num),
                        (f3, Some(val)),
                        (f3, num),
                    ] {
                        if let Some(t) = t {
                            if nw + text_w(f, t) + NAME_VALUE_GAP <= inner_w {
                                name = nm;
                                name_font = nf;
                                val_show = t;
                                vf = f;
                                found = true;
                                break 'fit;
                            }
                        }
                    }
                }
            }
        }
        // without a value (or when none fits): the largest name that fits
        let menu_room = if item.is_menu() { CHEVRON_SLOT } else { 0 };
        if !wants_value || !found {
            let room = inner_w - menu_room - back_w;
            let pick = [
                (long, name_fonts[0]),
                (long, name_fonts[1]),
                (short_name(long), name_fonts[0]),
                (short_name(long), name_fonts[1]),
            ]
            .into_iter()
            .find(|(nm, nf)| text_w(nf, nm) <= room);
            (name, name_font) = pick.unwrap_or((short_name(long), name_fonts[1]));
        }
        let name_w = text_w(name_font, name);
        let show_val = !val_show.is_empty();
        // a sub menu may carry a short value before its chevron ("8 >")
        let menu_vf: &FontRenderer = value_fonts[1];
        let menu_val: &str = if item.is_menu()
            && !val.is_empty()
            && name_w + text_w(menu_vf, val) + CHEVRON_SLOT + NAME_VALUE_GAP <= inner_w
        {
            val
        } else {
            ""
        };
        let name_max = if show_val {
            inner_w - text_w(vf, val_show) - NAME_VALUE_GAP
        } else if item.is_menu() {
            inner_w
                - CHEVRON_SLOT
                - if menu_val.is_empty() {
                    0
                } else {
                    text_w(menu_vf, menu_val) + NAME_VALUE_GAP
                }
        } else {
            inner_w
        };
        let name_clipped = clip_to(name_font, name, name_max - back_w);
        let err = value_error(cm, &item);

        let (col, vcol) = if selected {
            (pal.scale, if err { ERR } else { pal.ring_select })
        } else {
            // aviation look: colour only where the knob acts or for errors
            let c = if rel.abs() == 1 {
                Colors::DarkGray
            } else {
                Colors::DimGray
            };
            (c, if err { ERR } else { c })
        };
        if selected {
            RoundedRectangle::with_equal_corners(
                Rectangle::new(
                    Point::new(row_l - 16, y - SETUP_ROW_PITCH / 2),
                    Size::new((row_r - row_l + 32) as u32, SETUP_ROW_PITCH as u32),
                ),
                Size::new(8, 8),
            )
            .into_styled(PrimitiveStyle::with_stroke(pal.ring_select, 2))
            .draw(display)?;
        }
        if icon != MenuIcon::None {
            let size = if selected {
                36
            } else if near {
                28
            } else {
                24
            };
            draw_icon(
                display,
                icon,
                Point::new(row_l + (SETUP_ICON_X - SETUP_LEFT), y),
                size,
                col,
            )?;
        }
        if back {
            back_chevron(
                display,
                text_x,
                y,
                if selected { 10 } else { 8 },
                if selected { pal.ring_select } else { col },
            )?;
        }
        name_font.render_aligned(
            name_clipped.as_str(),
            Point::new(text_x + back_w, y),
            VerticalPosition::Center,
            HorizontalAlignment::Left,
            FontColor::Transparent(col),
            display,
        )?;
        if item.is_menu() {
            let (s, c) = if selected {
                (11, pal.ring_select)
            } else {
                (9, col)
            };
            chevron(display, row_r - 8, y, s, c)?;
            if !menu_val.is_empty() {
                render_value(
                    display,
                    menu_vf,
                    menu_val,
                    Point::new(row_r - CHEVRON_SLOT, y),
                    VerticalPosition::Center,
                    HorizontalAlignment::Right,
                    vcol,
                )?;
            }
        } else if show_val {
            render_value(
                display,
                vf,
                val_show,
                Point::new(row_r, y),
                VerticalPosition::Center,
                HorizontalAlignment::Right,
                vcol,
            )?;
        }
    }
    list_position(display, center, 216, vpos.max(0) as usize, vis.len())
}

/// Flap Stages like the sketch skizze_klappen_menue.png (4): column heads
/// Flap / km/h / Sensor (sketch fub15 -> fub11), rows 52 px: name fub20,
/// speed range inb21 in the middle, stored position inb16 right; the
/// recommended stage carries a magenta dot
fn draw_flap_table<D>(
    display: &mut D,
    cm: &CoreModel,
    menu: &Menu,
    pos: usize,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let pal = &cm.palette().vario;
    let center = cm.device_const.sizes.display.center;
    let t = cm.flap_table();
    let with_sensor = t.sensor != crate::model::flaps::FlapSensorSource::None;
    header(display, center, 240, CAP_FULL, menu.name)?;
    let u = cm.config.unit_horizontal_speed;
    for (x, label, h) in [
        (92, "Flap", HorizontalAlignment::Left),
        (226, u.as_str(), HorizontalAlignment::Center),
        (
            400,
            if with_sensor { "Sensor" } else { "" },
            HorizontalAlignment::Right,
        ),
    ] {
        text_for(15).f.render_aligned(
            label,
            Point::new(x, center.y - 136),
            VerticalPosition::Center,
            h,
            FontColor::Transparent(Colors::DimGray),
            display,
        )?;
    }
    let vis = crate::model::menu::visual_order(cm, menu);
    let vpos = vis.iter().position(|&i| i == pos).unwrap_or(0) as i32;
    let conv = |kmh: u16| u.as_f32(crate::Speed::from_km_h(kmh as f32)) as i32;
    const PITCH: i32 = 52;
    for rel in -2i32..=2 {
        let vi = vpos + rel;
        if vi < 0 || vi >= vis.len() as i32 {
            continue;
        }
        let item = menu.items[vis[vi as usize]];
        let y = center.y + rel * PITCH + 12;
        let col = if rel == 0 {
            pal.scale
        } else if rel.abs() == 1 {
            Colors::DarkGray
        } else {
            Colors::DimGray
        };
        let vcol = if rel == 0 { pal.ring_select } else { col };
        if rel == 0 {
            RoundedRectangle::with_equal_corners(
                Rectangle::new(
                    Point::new(center.x - 184, y - PITCH / 2 + 1),
                    Size::new(368, (PITCH - 2) as u32),
                ),
                Size::new(8, 8),
            )
            .into_styled(PrimitiveStyle::with_stroke(pal.ring_select, 2))
            .draw(display)?;
        }
        let name_font: &FontRenderer = text_for(25).f; // all rows alike, only the colour fades
        match item.content {
            MenuItemContent::FlapItem(i) => {
                let i = i as usize;
                if cm.calculated.flap_rec == Some(i as u8) {
                    Circle::with_center(Point::new(74, y), 10)
                        .into_styled(PrimitiveStyle::with_fill(Colors::Magenta))
                        .draw(display)?;
                }
                name_font.render_aligned(
                    t.name(i),
                    Point::new(92, y),
                    VerticalPosition::Center,
                    HorizontalAlignment::Left,
                    FontColor::Transparent(col),
                    display,
                )?;
                let (lo, hi) = t.range(i);
                let mut r: heapless::String<16> = heapless::String::new();
                let _ = match (lo, hi) {
                    (0, 0) => tfmt::uwrite!(&mut r, "all"),
                    (0, h) => tfmt::uwrite!(&mut r, "<{}", conv(h)),
                    (l, 0) => tfmt::uwrite!(&mut r, "{}+", conv(l)),
                    (l, h) => tfmt::uwrite!(&mut r, "{}-{}", conv(l), conv(h)),
                };
                let vfont: &FontRenderer = &VALUE_19;
                render_value(
                    display,
                    vfont,
                    r.as_str(),
                    Point::new(226, y),
                    VerticalPosition::Center,
                    HorizontalAlignment::Center,
                    vcol,
                )?;
                if with_sensor {
                    let p = t.stages[i].position;
                    let mut sv: heapless::String<12> = heapless::String::new();
                    if p.is_nan() {
                        let _ = sv.push('-');
                    } else {
                        let tenths = (p * 10.0 + if p >= 0.0 { 0.5 } else { -0.5 }) as i32;
                        let _ = tfmt::uwrite!(
                            &mut sv,
                            "{}{}.{}°",
                            if tenths < 0 { "-" } else { "" },
                            tenths.abs() / 10,
                            tenths.abs() % 10
                        );
                    }
                    render_value(
                        display,
                        &VALUE_16,
                        sv.as_str(),
                        Point::new(400, y),
                        VerticalPosition::Center,
                        HorizontalAlignment::Right,
                        col,
                    )?;
                }
            }
            _ => {
                let back = matches!(item.content, MenuItemContent::EditItem(Editable::Return));
                let x = if back {
                    back_chevron(
                        display,
                        92,
                        y,
                        if rel == 0 { 8 } else { 6 },
                        if rel == 0 { pal.ring_select } else { col },
                    )?;
                    116
                } else {
                    92
                };
                name_font.render_aligned(
                    item.name(cm),
                    Point::new(x, y),
                    VerticalPosition::Center,
                    HorizontalAlignment::Left,
                    FontColor::Transparent(col),
                    display,
                )?;
            }
        }
    }
    list_position(display, center, 216, vpos.max(0) as usize, vis.len())
}

/// flight menu as inner circle overlay r 184 - the ring scale keeps
/// running (sketch skizze_menue_ueberschrift.png): rows 54 px, names
/// sketch fub25 (u8g2 fub20), values sketch inb30 (u8g2 inb21), header
/// "Flight" sketch fub22 (fub17)
pub fn draw_flight_list<D>(display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    draw_flight_list_styled(display, cm, &FLIGHT_STYLE)
}

/// the flight menu in a given typography (comparison renders)
pub fn draw_flight_list_styled<D>(
    display: &mut D,
    cm: &CoreModel,
    st: &FlightStyle,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let pal = &cm.palette().vario;
    let center = cm.device_const.sizes.display.center;
    // the whole black core, no line around it (Stefan 28.10.2026)
    let radius = overlay_disc(display, cm)?;

    let menu: &Menu = cm.control.menu_control.menu;
    let pos = cm.control.menu_control.pos[menu.level];

    header(display, center, radius, CAP_OVERLAY, "Flight")?;
    let top_lim = center.y + CAP_OVERLAY + 4;

    let n = menu.items.len() as i32;
    let h2 = st.pitch / 2 - 1;
    // ballast / bugs being edited: wing loading and best glide take the two
    // rows below
    let info = cm.control.editor.inline
        && cm.control.editor.mode != crate::EditMode::Off
        && crate::view::ring_editor::shows_glider_info(cm.control.editor.target);
    if info {
        let y = |rel: i32| center.y + rel * st.pitch + st.sel_y + 8;
        crate::view::ring_editor::draw_glider_info(display, cm, center.x, y(1), y(2))?;
    }
    let vis = crate::model::menu::visual_order(cm, menu);
    let vpos = vis.iter().position(|&i| i == pos).unwrap_or(0) as i32;
    let _ = n;
    for rel in -2i32..=2 {
        let vi = vpos + rel;
        if vi < 0 || vi >= vis.len() as i32 || (info && rel > 0) {
            continue;
        }
        let idx = vis[vi as usize] as i32;
        let item = menu.items[idx as usize];
        let y = center.y + rel * st.pitch + st.sel_y;
        let selected = rel == 0;
        let (name_f, value_f) = if rel.abs() >= 2 {
            (st.outer_name, st.outer_value)
        } else {
            (st.name, st.value)
        };
        // rows never collide with the header and never leave the circle
        // (the selected one with its frame, the others with their text)
        let ext = if selected { h2 } else { st.text_h2 };
        if y - ext < top_lim || y + ext > center.y + radius - 8 {
            continue;
        }
        // the value of this row is being edited right here (push - turn -
        // push): orange row, black text, the live value
        let editing =
            selected && cm.control.editor.inline && cm.control.editor.mode != crate::EditMode::Off;
        // the frame of the selected row must stay inside the circle, the
        // other rows only need room for their text (sketch: "Volume" whole)
        let edge = if selected { h2 } else { st.text_h2 };
        let half = chord_half(radius - 10, (y - center.y).abs() + edge) - 6;
        let live = cm.control.editor.get_value_line(cm);
        let val = if editing {
            live.as_str()
        } else {
            value_str(cm, idx as usize)
        };
        // the full name ("Gust Filter") when it fits with the whole value
        // (or the chevron), else the short one ("Gust")
        let long = item.name(cm);
        let tail = if item.is_menu() {
            CHEVRON_SLOT
        } else if val.is_empty() {
            0
        } else {
            text_w(value_f, val) + NAME_VALUE_GAP
        };
        // decided at the narrowest of the three large rows, so a name does
        // not change while it scrolls past the cursor
        let ref_half = if rel.abs() >= 2 {
            half
        } else {
            let dy = (st.sel_y - st.pitch).abs().max(st.sel_y + st.pitch) + st.text_h2;
            half.min(chord_half(radius - 10, dy) - 6)
        };
        let name = if st.full_names && text_w(name_f, long) + tail <= 2 * ref_half - 28 {
            long
        } else {
            short_name(long)
        };
        let (col, vcol) = if editing {
            (pal.scale, pal.background)
        } else if selected {
            (pal.scale, pal.ring_select)
        } else if rel.abs() == 1 {
            (Colors::DarkGray, Colors::DarkGray)
        } else {
            (Colors::DimGray, Colors::DimGray)
        };

        // fit check with REAL text widths: first drop the unit (keep the
        // number), then drop the value completely - never cut it partially
        let inner_w = 2 * half - 28;
        let name_w = text_w(name_f, name);
        let mut val_show: &str = "";
        if !val.is_empty() && !item.is_menu() {
            if name_w + text_w(value_f, val) + NAME_VALUE_GAP <= inner_w {
                val_show = val;
            } else if let Some(num) = strip_unit(val) {
                if name_w + text_w(value_f, num) + NAME_VALUE_GAP <= inner_w {
                    val_show = num;
                }
            }
        }
        let show_val = !val_show.is_empty();
        let name_max = if show_val {
            inner_w - text_w(value_f, val_show) - NAME_VALUE_GAP
        } else if item.is_menu() {
            inner_w - CHEVRON_SLOT
        } else {
            inner_w
        };
        let back = matches!(item.content, MenuItemContent::EditItem(Editable::Return));
        let back_w = if back { 24 } else { 0 };
        let name_clipped = clip_to(name_f, name, name_max - back_w);

        if selected {
            RoundedRectangle::with_equal_corners(
                Rectangle::new(
                    Point::new(center.x - half, y - h2 + 1),
                    Size::new((2 * half) as u32, (2 * h2 - 2) as u32),
                ),
                Size::new(8, 8),
            )
            .into_styled(PrimitiveStyle::with_stroke(pal.ring_select, 2))
            .draw(display)?;
        }
        // editing: only the value field at the end of the row is filled
        // orange, the number black (Stefan 28.10.2026: a whole orange row was
        // too heavy)
        if editing && show_val {
            // at least three digits wide, so the field does not jump at 9 -> 10
            let vw = text_w(value_f, val_show).max(text_w(value_f, "000"));
            let x0 = center.x + half - 14 - vw - 12;
            RoundedRectangle::with_equal_corners(
                Rectangle::new(
                    Point::new(x0, y - h2 + 1),
                    Size::new((center.x + half - x0) as u32, (2 * h2 - 2) as u32),
                ),
                Size::new(8, 8),
            )
            .into_styled(PrimitiveStyle::with_fill(pal.ring_select))
            .draw(display)?;
        }
        if back {
            back_chevron(
                display,
                center.x - half + 14,
                y,
                if selected { 8 } else { 6 },
                if selected { pal.ring_select } else { col },
            )?;
        }
        name_f.render_aligned(
            name_clipped.as_str(),
            Point::new(center.x - half + 14 + back_w, y),
            VerticalPosition::Center,
            HorizontalAlignment::Left,
            FontColor::Transparent(col),
            display,
        )?;
        if item.is_menu() {
            chevron(display, center.x + half - 28, y, 8, vcol)?;
        } else if show_val {
            render_value(
                display,
                value_f,
                val_show,
                Point::new(center.x + half - 14, y),
                VerticalPosition::Center,
                HorizontalAlignment::Right,
                vcol,
            )?;
        }
    }
    Ok(())
}

/// Page indicator: one dot per configured page, the active one filled in
/// the interaction colour. Shown for a moment after a page change.
pub fn draw_page_indicator<D>(display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let pages = &cm.config.pages;
    let n = pages.iter().filter(|p| p.is_on()).count();
    if n < 2 {
        return Ok(());
    }
    // the dots follow the circle right at the rim: one gray dot diameter
    // (8 px) of air between dot and display edge, centred at 6 o'clock in
    // the gap between the -2 and -3 scale numbers
    const DOT_ACTIVE: u32 = 12;
    const DOT_IDLE: u32 = 8;
    let center = cm.device_const.sizes.display.center;
    let radius =
        (cm.device_const.sizes.display.height as f32) / 2.0 - (DOT_IDLE + DOT_ACTIVE / 2) as f32; // 226 at 480
                                                                                                  // tighter spacing with many pages, so the outer dots stay clear of the
                                                                                                  // scale numbers left and right
    let pitch_deg = if n <= 4 { 7.0_f32 } else { 4.8_f32 };
    // 90 deg = 6 o'clock; page 1 sits on the left, the last one on the right
    let start = 90.0 + pitch_deg * (n as f32 - 1.0) / 2.0;
    let mut slot = 0_f32;
    for (i, p) in pages.iter().enumerate() {
        if !p.is_on() {
            continue;
        }
        let a = (start - pitch_deg * slot) * core::f32::consts::PI / 180.0;
        let pos = Point::new(
            center.x + (radius * a.cos()) as i32,
            center.y + (radius * a.sin()) as i32,
        );
        let active = i as u8 == cm.config.page_idx;
        let (style, d) = if active {
            (
                PrimitiveStyle::with_fill(cm.palette().vario.ring_select),
                DOT_ACTIVE,
            )
        } else {
            (PrimitiveStyle::with_fill(Colors::DimGray), DOT_IDLE)
        };
        Circle::with_center(pos, d)
            .into_styled(style)
            .draw(display)?;
        slot += 1.0;
    }
    Ok(())
}

#[cfg(test)]
mod setup_list_tests {
    use super::*;
    use crate::model::menu::full::STEFLY_SETTINGS;
    use crate::view::test_fixture::{test_model, TestDisplay};

    /// every submenu of the ring setup list carries an icon (the Pages entry
    /// had none, flight test 16.09.) - in the club list as well
    #[test]
    fn every_submenu_of_the_setup_list_has_an_icon() {
        let mut cm = test_model();
        for item in STEFLY_SETTINGS.items {
            if item.is_menu() {
                let name = item.name(&cm);
                assert!(
                    icon_for(name) != MenuIcon::None,
                    "setup entry {name} has no icon"
                );
            }
        }
        cm.config.club_mode = true;
        for item in crate::model::menu::club::SETTINGS.items {
            if item.is_menu() {
                let name = item.name(&cm);
                assert!(
                    icon_for(name) != MenuIcon::None,
                    "club setup entry {name} has no icon"
                );
            }
        }
    }

    /// `RING_DUMP_DIR=/tmp/menus cargo test --lib dump_setup_menus -- --ignored`
    /// renders every menu of the full setup (cursor on the first, the middle
    /// and the last row) with the value preview of a model without sensor box
    #[test]
    #[ignore]
    fn dump_setup_menus() {
        use crate::model::menu::full::MENU_LIST;
        let dir = match std::env::var("RING_DUMP_DIR") {
            Ok(d) => d,
            Err(_) => return,
        };
        let (mut cm, mut cc, _tx) = crate::test_harness!();
        cm.config.display_active = crate::model::DisplayActive::Menu;
        for (idx, menu) in MENU_LIST.iter().enumerate() {
            if menu.items.is_empty() || idx == 1 {
                continue;
            }
            cm.control.menu_control.menu = menu;
            crate::controller::menu::fill_values(&mut cm, &mut cc);
            let mid = menu.items.len() / 2;
            for (tag, pos) in [("a", 0usize), ("m", mid), ("b", menu.items.len() - 1)] {
                cm.control.menu_control.pos[menu.level] = pos;
                let mut d = TestDisplay::new();
                draw_setup_list(&mut d, &cm).unwrap();
                let name: std::string::String = menu
                    .name
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() {
                            c.to_ascii_lowercase()
                        } else {
                            '_'
                        }
                    })
                    .collect();
                crate::view::test_fixture::dump_ppm(
                    &d,
                    &std::format!("{dir}/{idx:02}_{name}_{tag}.ppm"),
                );
            }
        }
    }

    #[test]
    fn setup_list_draws_the_display_icon() {
        let mut cm = test_model();
        cm.control.menu_control.menu = &STEFLY_SETTINGS;
        let pos = STEFLY_SETTINGS
            .items
            .iter()
            .position(|it| it.name(&cm) == "Display")
            .expect("Display in the setup list");
        cm.control.menu_control.pos[STEFLY_SETTINGS.level] = pos;
        let mut d = TestDisplay::new();
        draw_setup_list(&mut d, &cm).unwrap();
        if let Ok(path) = std::env::var("RING_DUMP") {
            crate::view::test_fixture::dump_ppm(&d, &path);
        }
        // the icon of the selected row sits at x = 92, y = centre - 10, 32 px
        let c = cm.device_const.sizes.display.center;
        let ink = d.painted_box(92 - 18, c.y - 10 - 18, 92 + 18, c.y - 10 + 18);
        assert!(
            ink > 60,
            "no icon ink in the selected Display row: {ink} px"
        );
    }
}
