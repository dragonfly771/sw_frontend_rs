//! The fonts of the display (since 0.4.1.39): B612 Bold for all values,
//! Barlow Semi Condensed SemiBold for all texts (menus, captions, headers).
//! Both SIL Open Font License 1.1, generated as u8g2 fonts by
//! tools/fontgen/gen.py (the TrueType files are in tools/fonts_ofl).
//!
//! The number in the name is the ink height in pixels: of the digit "0" for
//! the values, of the capital "H" for the texts. The sizes replace the u8g2
//! fonts used before at the same height (inbNN resp. luBS / fubNN). The
//! texts have no 10, 17 and 25 px cut (the hinting of Barlow jumps from 9
//! to 11, 16 to 18 and 24 to 26 px), the neighbours take their place.
//!
//! Only ASCII plus degree, squared and cubed (all the firmware strings use),
//! the values with tabular digits. Together about 52 KB of flash, the fonts
//! replaced were about 114 KB (all Latin-1 for the older screens).

#[path = "larus_fonts_data.rs"]
mod data;

use u8g2_fonts::FontRenderer;

macro_rules! font {
    ($name:ident, $data:ident) => {
        pub const $name: FontRenderer =
            FontRenderer::new::<data::$data>().with_ignore_unknown_chars(true);
    };
}

font!(VALUE_16, b612_value_16);
font!(VALUE_19, b612_value_19);
font!(VALUE_21, b612_value_21);
font!(VALUE_24, b612_value_24);
font!(VALUE_27, b612_value_27);
font!(VALUE_32, b612_value_32);
font!(VALUE_38, b612_value_38);
font!(VALUE_42, b612_value_42);
font!(VALUE_53, b612_value_53);
// the two values page only: blank * + , - . / 0..9 : and degree (1.2 KB)
font!(VALUE_57, b612_value_57_num);

font!(TEXT_11, barlow_text_11);
font!(TEXT_12, barlow_text_12);
font!(TEXT_14, barlow_text_14);
font!(TEXT_18, barlow_text_18);
// the menu headers (since 0.4.1.40)
font!(TEXT_22, barlow_text_22);
font!(TEXT_26, barlow_text_26);
font!(TEXT_30, barlow_text_30);
