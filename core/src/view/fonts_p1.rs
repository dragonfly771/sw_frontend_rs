//! Typography of the Stefly design P1 (plan_ring_design_p1.md and the
//! sketches in ring_p1_skizzen/)
//!
//! IMPORTANT - sizes: the sketches were drawn with TrueType fonts (DejaVu
//! Sans Mono Bold for the "inbNN" values, DejaVu Sans Bold for the "fubNN"
//! texts) at a size of NN. For these fonts NN is the em size: a digit is
//! 0.74 x NN high, a capital 0.73 x NN. In u8g2, NN is the pixel height
//! of the glyphs themselves (inb42: digit "0" 42 px, fub20: "H" 20 px,
//! measured with `print_u8g2_metrics`). Using the same number would make
//! everything about 1.35 x larger than the sketches. All screens therefore
//! go through `value_for()` / `text_for()`: they take the size named in the
//! plan / the sketch and return the u8g2 font with the same visual height
//! (and about the same width).
//!
//!   sketch inb  21 24 27 30 33 38 42 46 49 53 57 63 72
//!   u8g2  inb   16 19 19 21 24 27 33 33 38 38 42 42 53
//!   sketch fub  13 15 16 18 20 21 22 23 25 26 30
//!   u8g2  luBS  10 10 12 12 14 14 18 18 18 18 24
//!
//! Up to 0.4.1.38 the texts used luBS (Lucida Sans Bold), the values
//! Inconsolata Bold (inb) with a double strike. Since 0.4.1.39 the values
//! are B612 Bold and the texts Barlow Semi Condensed SemiBold
//! (`utils::larus_fonts`), generated at the same ink heights as the u8g2
//! fonts of the table, so the mapping and all layouts stay.

use u8g2_fonts::FontRenderer;

// Since 0.4.1.39 all fonts come from `utils::larus_fonts` (B612 Bold for the
// values, Barlow Semi Condensed SemiBold for the texts, generated at the ink
// heights of the u8g2 fonts listed above). The old names stay, so the
// screens keep their sizes; VALUE_33 is the 32 px cut (inb33 had a 32 px
// digit as well), TEXT_10 / TEXT_24 are the 11 / 26 px cuts (Barlow has no
// 10 and 25 px, see larus_fonts).
pub use crate::utils::larus_fonts::TEXT_11 as TEXT_10;
pub use crate::utils::larus_fonts::TEXT_26 as TEXT_24;
pub use crate::utils::larus_fonts::VALUE_32 as VALUE_33;
pub use crate::utils::larus_fonts::{TEXT_12, TEXT_14, TEXT_18};
pub use crate::utils::larus_fonts::{
    VALUE_16, VALUE_19, VALUE_21, VALUE_24, VALUE_27, VALUE_38, VALUE_42, VALUE_53, VALUE_57,
};

/// a font with its measured glyph height (digit resp. capital, px)
#[derive(Clone, Copy)]
pub struct Font {
    pub f: &'static FontRenderer,
    /// height of a digit (values) resp. a capital letter (texts)
    #[allow(dead_code)]
    pub h: i32,
}

/// u8g2 value font that looks like the sketch size `sketch` ("inbNN" in
/// the plans)
pub fn value_for(sketch: u8) -> Font {
    match sketch {
        0..=22 => Font {
            f: &VALUE_16,
            h: 16,
        },
        23..=28 => Font {
            f: &VALUE_19,
            h: 19,
        },
        29..=31 => Font {
            f: &VALUE_21,
            h: 21,
        },
        32..=35 => Font {
            f: &VALUE_24,
            h: 24,
        },
        36..=40 => Font {
            f: &VALUE_27,
            h: 27,
        },
        41..=47 => Font {
            f: &VALUE_33,
            h: 32,
        },
        48..=55 => Font {
            f: &VALUE_38,
            h: 38,
        },
        56..=68 => Font {
            f: &VALUE_42,
            h: 42,
        },
        _ => Font {
            f: &VALUE_53,
            h: 53,
        },
    }
}

/// u8g2 text font that looks like the sketch size `sketch` ("fubNN" in the
/// plans)
pub fn text_for(sketch: u8) -> Font {
    match sketch {
        0..=15 => Font { f: &TEXT_10, h: 11 },
        16..=18 => Font { f: &TEXT_12, h: 12 },
        19..=21 => Font { f: &TEXT_14, h: 14 },
        22..=28 => Font { f: &TEXT_18, h: 18 },
        _ => Font { f: &TEXT_24, h: 26 },
    }
}

/// The degree sign as a ring at the top of a character cell (the ASCII
/// fonts have none): `cell_x` left edge of the cell, `adv` its width,
/// `baseline` and `h` (digit height) of the text it belongs to.
pub fn degree_ring<D>(
    display: &mut D,
    cell_x: i32,
    adv: i32,
    baseline: i32,
    h: i32,
    col: crate::utils::Colors,
) -> Result<(), crate::CoreError>
where
    D: embedded_graphics::draw_target::DrawTarget<
        Color = crate::utils::Colors,
        Error = crate::CoreError,
    >,
{
    use embedded_graphics::{
        prelude::{Point, Primitive},
        primitives::{Circle, PrimitiveStyle},
        Drawable,
    };
    let d = ((h * 3 + 5) / 10).max(5) as u32; // outer diameter 0.3 x digit height
    let w = ((h + 6) / 9).clamp(2, 6) as u32;
    let x = cell_x + adv / 2 - d as i32 / 2;
    Circle::new(Point::new(x, baseline - h), d)
        .into_styled(PrimitiveStyle::with_stroke(col, w))
        .draw(display)?;
    Ok(())
}

/// baseline of a text drawn with `v` at `y` (as u8g2-fonts computes it)
pub fn baseline_of(font: &FontRenderer, y: i32, v: u8g2_fonts::types::VerticalPosition) -> i32 {
    use u8g2_fonts::types::VerticalPosition as V;
    let a = font.get_ascent() as i32;
    let d = font.get_descent() as i32;
    y + match v {
        V::Baseline => 0,
        V::Top => a + 1,
        V::Center => (a - d + 1) / 2 + d,
        V::Bottom => d,
    }
}

/// a value in the value font (B612 Bold, the degree sign is in the font)
pub fn render_value<D>(
    display: &mut D,
    font: &FontRenderer,
    s: &str,
    p: embedded_graphics::prelude::Point,
    v: u8g2_fonts::types::VerticalPosition,
    h: u8g2_fonts::types::HorizontalAlignment,
    col: crate::utils::Colors,
) -> Result<(), crate::CoreError>
where
    D: embedded_graphics::draw_target::DrawTarget<
        Color = crate::utils::Colors,
        Error = crate::CoreError,
    >,
{
    font.render_aligned(
        s,
        p,
        v,
        h,
        u8g2_fonts::types::FontColor::Transparent(col),
        display,
    )?;
    Ok(())
}

/// Position helpers for a "value + caption" block as the sketches build it:
/// the sketch puts the caption (font size `cap`) with its TOP 6 px below
/// the baseline of the value. The visible capital starts 0.2 x cap lower,
/// so the caption BASELINE lies 6 + 0.93 x cap below the value baseline.
pub fn caption_offset(cap_sketch: u8) -> i32 {
    (6.0 + 0.93 * cap_sketch as f32 + 0.5) as i32
}

/// digit height the sketch assumed for its layout (0.74 x size)
pub fn sketch_digit(sketch: u8) -> f32 {
    0.74 * sketch as f32
}

/// caption size the sketches use under a value of size `sketch`
pub fn sketch_caption(sketch: u8) -> u8 {
    if sketch < 46 {
        13
    } else {
        16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use u8g2_fonts::types::VerticalPosition;

    #[test]
    fn mapped_fonts_match_the_sketch_heights() {
        // digit height of the sketch font (DejaVu Sans Mono Bold: 0.755 x size)
        for s in [24u8, 33, 46, 57, 72] {
            let want = 0.755 * s as f32;
            let got = value_for(s).h as f32;
            assert!((got - want).abs() <= 3.5, "inb{s}: {got} px for {want} px");
        }
        // the digit height stored in the table is the real ink height
        for s in [24u8, 33, 57, 72] {
            let f = value_for(s);
            let mut d = crate::view::test_fixture::TestDisplay::new();
            f.f.render(
                "0",
                embedded_graphics::prelude::Point::new(100, 200),
                VerticalPosition::Baseline,
                u8g2_fonts::types::FontColor::Transparent(crate::utils::Colors::White),
                &mut d,
            )
            .unwrap();
            let rows = (0..480)
                .filter(|&y| d.painted_box(0, y, 479, y) > 0)
                .count() as i32;
            assert!(
                (rows - f.h).abs() <= 1,
                "inb{s}: glyph {rows} px, table {}",
                f.h
            );
        }
    }
}
