//! Name entry: alphabet in the outer ring (plan_klappen_menue.md ch. 2,
//! sketch skizze_namenseingabe.png). Logic in controller/name_entry.rs.

use crate::{
    controller::name_entry::{field, n_fields, Field},
    model::editable::Content,
    utils::Colors,
    view::fonts_p1::{text_for, VALUE_53},
    CoreError, CoreModel, DrawImage,
};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle, RoundedRectangle},
};
#[allow(unused_imports)]
use micromath::F32Ext;
use u8g2_fonts::{
    types::{FontColor, HorizontalAlignment, VerticalPosition},
    FontRenderer,
};

/// radius of the character ring (sketch skizze_namenseingabe.png)
const RING_R: f32 = 210.0;
/// light grey of the keys' labels
const CTRL: Colors = Colors::Gray;

fn w(font: &FontRenderer, s: &str) -> i32 {
    font.get_rendered_dimensions(s, Point::zero(), VerticalPosition::Baseline)
        .map(|d| d.advance.x)
        .unwrap_or(0)
}

fn put<D>(
    display: &mut D,
    font: &FontRenderer,
    s: &str,
    p: Point,
    h: HorizontalAlignment,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    font.render_aligned(
        s,
        p,
        VerticalPosition::Center,
        h,
        FontColor::Transparent(col),
        display,
    )?;
    Ok(())
}

fn put_v<D>(
    display: &mut D,
    font: &FontRenderer,
    s: &str,
    p: Point,
    h: HorizontalAlignment,
    col: Colors,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError>,
{
    crate::view::fonts_p1::render_value(display, font, s, p, VerticalPosition::Center, h, col)
}

fn key_label(f: Field) -> &'static str {
    match f {
        Field::Page(0) => "ABC",
        Field::Page(_) => "123",
        Field::Back => "<",
        Field::Ok => "OK",
        Field::Char(_) => "",
    }
}

pub fn draw_name_entry<D>(display: &mut D, cm: &CoreModel, header: &str) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let c = cm.device_const.sizes.display.center;
    let orange = cm.palette().vario.ring_select;
    display.clear(cm.palette().vario.background)?;
    let ne = cm.control.editor.name;
    let n = n_fields(ne.page);
    let sel = (ne.sel as usize).min(n - 1);

    // ring of fields, fixed: the first character (A resp. 0) at 9 o'clock like
    // the zero of the vario scale, then clockwise; the keys end the round just
    // before 9 o'clock. Only the selection moves (Stefan 28.10.2026: the
    // turning ring of the sketch was hard to follow).
    for i in 0..n {
        let a = (180.0 + i as f32 * 360.0 / n as f32).to_radians();
        let p = Point::new(
            c.x + (RING_R * a.cos()) as i32,
            c.y + (RING_R * a.sin()) as i32,
        );
        let f = field(ne.page, i);
        let selected = i == sel;
        match f {
            Field::Char(ch) => {
                let mut b = [0u8; 4];
                let s = (ch as char).encode_utf8(&mut b);
                let label: &str = if ch == b' ' { "_" } else { s };
                // sketch: characters fub26 (u8g2 fub20), the selected one
                // fub30 (fub25) orange in an orange circle r 26
                if selected {
                    Circle::with_center(p, 52)
                        .into_styled(PrimitiveStyle::with_stroke(orange, 3))
                        .draw(display)?;
                    put(
                        display,
                        text_for(30).f,
                        label,
                        p + Point::new(0, 1),
                        HorizontalAlignment::Center,
                        orange,
                    )?;
                } else {
                    put(
                        display,
                        text_for(26).f,
                        label,
                        p + Point::new(0, 1),
                        HorizontalAlignment::Center,
                        Colors::White,
                    )?;
                }
            }
            _ => {
                // keys: grey frames with a smaller font, set apart from the characters
                // sketch: grey frames 44 / 32 x 30 px, labels fub16 / fub18
                // (u8g2 fub11 / fub14); selected: orange circle, fub20 (fub14)
                let label = key_label(f);
                let three = label.len() == 3;
                if selected {
                    Circle::with_center(p, 52)
                        .into_styled(PrimitiveStyle::with_stroke(orange, 3))
                        .draw(display)?;
                    put(
                        display,
                        text_for(20).f,
                        label,
                        p + Point::new(0, 1),
                        HorizontalAlignment::Center,
                        orange,
                    )?;
                } else {
                    let bw = if three { 44 } else { 32 };
                    let r = Rectangle::new(
                        Point::new(p.x - bw / 2, p.y - 15),
                        Size::new(bw as u32, 30),
                    );
                    RoundedRectangle::with_equal_corners(r, Size::new(6, 6))
                        .into_styled(PrimitiveStyle::with_stroke(Colors::DimGray, 2))
                        .draw(display)?;
                    let font = if three {
                        text_for(16).f
                    } else {
                        text_for(18).f
                    };
                    put(
                        display,
                        font,
                        label,
                        p + Point::new(0, 1),
                        HorizontalAlignment::Center,
                        CTRL,
                    )?;
                }
            }
        }
    }

    // header ("Flap Name" instead of the short list name), sketch fub22 in
    // mixed case (u8g2 fub17)
    let header = if cm.control.editor.target == crate::Editable::FlapName {
        "Flap Name"
    } else {
        header
    };
    crate::view::ring_menu::title(display, c, -114, header)?;
    // page indicator "ABC  123" (sketch fub18 -> fub14), the active page
    // orange and underlined
    for (k, label) in ["ABC", "123"].iter().enumerate() {
        let x = c.x + if k == 0 { -34 } else { 34 };
        let active = ne.page as usize == k;
        let p = Point::new(x, c.y - 74);
        put(
            display,
            text_for(18).f,
            label,
            p,
            HorizontalAlignment::Center,
            if active { orange } else { Colors::DimGray },
        )?;
        if active {
            Line::new(Point::new(x - 20, c.y - 60), Point::new(x + 20, c.y - 60))
                .into_styled(PrimitiveStyle::with_stroke(orange, 3))
                .draw(display)?;
        }
    }

    // the name (sketch inb72 -> u8g2 inb53): it ends at the centre, the
    // selected character follows as an orange preview, underlined
    let name = match cm.control.editor.content {
        Content::String(t) => t,
        _ => crate::utils::TString::new(),
    };
    let mut pb = [0u8; 4];
    let preview: &str = match field(ne.page, sel) {
        Field::Char(b' ') if name.len() < ne.max_len as usize => "_",
        Field::Char(ch) if name.len() < ne.max_len as usize => (ch as char).encode_utf8(&mut pb),
        _ => "",
    };
    let y = c.y + 6;
    if preview.is_empty() {
        put_v(
            display,
            &VALUE_53,
            name.as_str(),
            Point::new(c.x, y),
            HorizontalAlignment::Center,
            Colors::White,
        )?;
    } else {
        put_v(
            display,
            &VALUE_53,
            name.as_str(),
            Point::new(c.x, y),
            HorizontalAlignment::Right,
            Colors::White,
        )?;
        put_v(
            display,
            &VALUE_53,
            preview,
            Point::new(c.x + 2, y),
            HorizontalAlignment::Left,
            orange,
        )?;
        let wc = w(&VALUE_53, "N");
        Line::new(
            Point::new(c.x + 4, c.y + 46),
            Point::new(c.x + 2 + wc, c.y + 46),
        )
        .into_styled(PrimitiveStyle::with_stroke(orange, 4))
        .draw(display)?;
    }
    // operating hints (sketch fub18 -> fub14)
    put(
        display,
        text_for(18).f,
        "turn: select   push: add",
        c + Point::new(0, 92),
        HorizontalAlignment::Center,
        Colors::DimGray,
    )?;
    put(
        display,
        text_for(18).f,
        "outer knob: ABC / 123",
        c + Point::new(0, 118),
        HorizontalAlignment::Center,
        Colors::DimGray,
    )?;
    Ok(())
}
