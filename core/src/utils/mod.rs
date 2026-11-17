mod colors16;
mod colors8;
mod crc;
mod date;
mod date_time;
mod draw_image;
mod error;
mod events;
mod filter;
mod idle_events;
mod image;
pub mod larus_fonts;
pub mod math;
mod metadata;
mod parse;
mod persistence;
mod rgb565_colors;
mod tstring;
mod variant;
mod version;
mod version_check;

#[cfg(feature = "colors_rgb565")]
pub use colors16::Colors;
#[cfg(feature = "colors_8_indexed")]
pub use colors8::Colors;
pub use colors8::Colors as Colors8;
pub use crc::*;
pub use date::*;
pub use date_time::*;
pub use draw_image::*;
pub use error::CoreError;
pub use events::*;
pub use filter::*;
pub use idle_events::*;
pub use image::*;
pub use metadata::*;
pub use parse::*;
pub use persistence::*;
pub use rgb565_colors::RGB565_COLORS;
pub use tstring::*;
pub use variant::*;
pub use version::*;
pub use version_check::*;

use num::clamp;

pub fn val_manip<T>(val: T, key: &KeyEvent, inc1: T, inc2: T, min: T, max: T) -> T
where
    T: core::ops::Add<Output = T> + core::ops::Sub<Output = T> + core::cmp::PartialOrd,
{
    match key {
        KeyEvent::Rotary1Left => clamp(val - inc2, min, max),
        KeyEvent::Rotary1Right => clamp(val + inc2, min, max),
        KeyEvent::Rotary2Left => clamp(val - inc1, min, max),
        KeyEvent::Rotary2Right => clamp(val + inc1, min, max),
        _ => val,
    }
}

/*#[cfg(test)]
pub(crate) mod tests {
    use crate::{
        basic_config::MAX_TX_FRAMES, CoreController, CoreModel, DeviceConst, HwVersion, Images, Palette, QIdleEvents, QTxFrames, SwVersion
    };
    use heapless::spsc::Queue;
    use u8g2_fonts::FontRenderer;

    const HW_VERSION: HwVersion = HwVersion::from_bytes([1, 3, 1, 0]);
    const SW_VERSION: SwVersion = SwVersion {
        version: [0, 0, 0, 0],
    };

    #[allow(unused)]
    pub(crate) fn cores() -> (CoreModel, CoreController) {
        let (p_tx_frames, _c_tx_frames) = {
            static mut Q_TX_FRAMES: QTxFrames<MAX_TX_FRAMES> = Queue::new();
            // Note: unsafe is ok here, because [heapless::spsc] queue protects against UB
            unsafe { Q_TX_FRAMES.split() }
        };

        // This queue routes the StorageItems from the controller to the idle loop.
        let (p_idle_events, _c_idle_events) = {
            static mut Q_IDLE_EVENTS: QIdleEvents = Queue::new();
            // Note: unsafe is ok here, because [heapless::spsc] queue protects against UB
            unsafe { Q_IDLE_EVENTS.split() }
        };

        const DEVICE_CONST: DeviceConst = DeviceConst {
            dark_theme: Palette::default(),
            bright_theme: Palette::default(),
            big_font: crate::utils::larus_fonts::TEXT_18,
            small_font: crate::utils::larus_fonts::TEXT_18,
            images: Images::new(),
        };

        let mut model = CoreModel::new(1234_u32, HW_VERSION, SW_VERSION, &DEVICE_CONST);
        let controller = CoreController::new(&mut model, p_idle_events, p_tx_frames);
        (model, controller)
    }
}*/

/// `f32::clamp` without the panic for min > max: the panic message formats
/// floats and pulls about 10 KB of float formatting into the firmware
pub trait FClamp {
    fn fclamp(self, lo: f32, hi: f32) -> f32;
}

impl FClamp for f32 {
    #[inline]
    fn fclamp(self, lo: f32, hi: f32) -> f32 {
        if self < lo {
            lo
        } else if self > hi {
            hi
        } else {
            self
        }
    }
}

/// The device keeps its own colour table (CLUT of the V2 LTDC). A colour
/// that is added to `Colors` but not to the CLUT shows up in whatever the
/// CLUT RAM holds (28.10.2026: the menu header cap was light green on the
/// display). Every index must be in all three tables with the same RGB.
#[cfg(test)]
mod color_table_tests {
    use super::{Colors8, RGB565_COLORS};
    use embedded_graphics::pixelcolor::{Rgb888, RgbColor};

    #[test]
    fn device_clut_matches_the_colors() {
        let path = std::format!(
            "{}/../device/larus_frontend_v2/src/driver/clut_colors.rs",
            env!("CARGO_MANIFEST_DIR")
        );
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => return, // core checked out alone
        };
        let mut clut: std::vec::Vec<u32> = std::vec::Vec::new();
        for line in src.lines() {
            let t = line.trim();
            if let Some(hex) = t.strip_prefix("0x") {
                let v = u32::from_str_radix(&hex[..8], 16).unwrap();
                clut.push(v);
            }
        }
        let n = RGB565_COLORS.len();
        assert_eq!(
            clut.len(),
            n,
            "device CLUT has {} entries, Colors has {}",
            clut.len(),
            n
        );
        for (i, v) in clut.iter().enumerate() {
            assert_eq!(
                (v >> 24) as usize,
                i,
                "CLUT entry {i} carries index {}",
                v >> 24
            );
            let c: Rgb888 = Colors8::from(i as u8).into();
            let rgb = (c.r() as u32) << 16 | (c.g() as u32) << 8 | c.b() as u32;
            assert_eq!(
                v & 0xff_ffff,
                rgb,
                "CLUT {i}: {:06x}, Colors {:?}: {:06x}",
                v & 0xff_ffff,
                Colors8::from(i as u8),
                rgb
            );
            let q = ((c.r() as u16 >> 3) << 11) | ((c.g() as u16 >> 2) << 5) | (c.b() as u16 >> 3);
            assert_eq!(
                RGB565_COLORS[i],
                q,
                "RGB565 table {i} {:?}",
                Colors8::from(i as u8)
            );
        }
    }
}
