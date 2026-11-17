//! Test fixture: a full `DeviceConst` for the 480x480 round display and a
//! memory display, so the views can be rendered and inspected in unit tests.
//!
//! The constants are a copy of `device/larus_frontend_v2/src/dev_const.rs`
//! with empty image slices - the views under test draw text and primitives,
//! not wallpapers. Keep it in sync when the device sizes change.

use crate::{
    utils::Colors, CoreError, CoreModel, DeviceConst, DisplaySizes, DrawImage, EditMode,
    HorizonSizes, HwVersion, Images, Misc, Palette, Rotation, Sizes, SwVersion, VarioSizes,
};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    Pixel,
};
use u8g2_fonts::FontRenderer;

pub const DISPLAY_HEIGHT: u32 = 480;
pub const DISPLAY_WIDTH: u32 = 480;

pub const DEVICE_CONST: DeviceConst = DeviceConst {
    dark_theme: Palette::default(),
    bright_theme: Palette::default(),
    big_font: BIG_FONT,
    small_font: SMALL_FONT,
    value_font: VALUE_FONT,
    value_font_mid: VALUE_FONT_MID,
    value_font_small: VALUE_FONT_SMALL,
    caption_font: CAPTION_FONT,
    header_font: HEADER_FONT,
    value_font_big: VALUE_FONT_BIG,
    value_font_big_mid: VALUE_FONT_BIG_MID,
    caption_font_big: CAPTION_FONT_BIG,
    images: images(),
    sizes: sizes(),
    misc: misc(),
};

const fn sizes() -> Sizes {
    Sizes {
        vario: vario_sizes(),
        horizon: horizon_sizes(),
        display: display_sizes(),
    }
}

const fn misc() -> Misc {
    Misc {
        sw_version: SwVersion {
            version: [0, 4, 1, 0],
        },
        hw_version: HwVersion {
            version: [1, 3, 1, 0],
        },
        edit_mode: EditMode::CircleArea,
    }
}

const fn display_sizes() -> DisplaySizes {
    DisplaySizes {
        height: 480,
        width: 480,
        margin: 2,
        radius: 238,
        center: Point::new(240, 240),
        screen_center: Point::new(240, 240),
    }
}

const fn horizon_sizes() -> HorizonSizes {
    HorizonSizes {
        pitch_scale_len: 20,
        lt_swing_len: (DISPLAY_HEIGHT * 65 / 100) as i32,
        lt_mid: (DISPLAY_HEIGHT * 25 / 100) as i32,
        lt_rad: DISPLAY_WIDTH * 70 / 1000,
        ring_wp_radius: (DISPLAY_HEIGHT * 5 / 16) as i32,
    }
}

const fn vario_sizes() -> VarioSizes {
    VarioSizes {
        stf_diameter: 392, // ring design B3: arc 192..200 (was 346 / 10: 168..178)
        stf_width: 8,
        indicator_len: 71,
        attention_pos: Point::new(90, 160),
        glider_pos: Point::new(129, 205),
        north_pos: Point::new(216, 14),
        bat_pos: Point::new(385, 130),
        sat_pos: Point::new(420, 130),
        profile_pos: Point::new(400, 175),
        unit_pos: Point::new(208, 432),
        info1_pos: Point::new(240, 120),
        info2_pos: Point::new(240, 360),
        info3_pos: Point::new(410, 280),
        pic_info3_pos: Point::new(380, 225),
        ta_circle_radius: 65,
        ta_point_diameter: 25,
        wind_len: 150,
        wind_len_min: 80,
        scale_factor: 25.0,
        ring_scale_factor: 36.0,
        ring_info_top_pos: Point::new(240, 108),
        ring_info_bottom_pos: Point::new(240, 372),
        ring_info_r1_pos: Point::new(302, 150),
        ring_info_r2_pos: Point::new(331, 240),
        ring_info_r3_pos: Point::new(312, 330),
        // P1: outer band between "4" and "3" (sketch skizze_status_symbole.png A),
        // centres at r 221.5 - half way between the scale line (r 203) and the
        // display edge (r 240), Stefan 28.10.2026 (before r 226 / 227)
        ring_bat_pos: Point::new(384, 390),
        ring_sat_pos: Point::new(337, 423),
        ring_profile_pos: Point::new(128, 288),
        ring_attention_pos: Point::new(90, 160),
    }
}

/// A valid but empty image (format version 4, display width, height 0, no
/// colours): `draw_img` accepts it and paints nothing, so whole pages can
/// be rendered in tests without the real wallpapers.
pub const EMPTY_IMG: &[u8] = &[
    4,
    0,
    (DISPLAY_WIDTH & 0xff) as u8,
    (DISPLAY_WIDTH >> 8) as u8,
    0,
    0,
    0,
    0,
    0,
];

const fn images() -> Images {
    Images {
        attention: EMPTY_IMG,
        avg_climb_rate: EMPTY_IMG,
        bat_empty: EMPTY_IMG,
        bat_full: EMPTY_IMG,
        bat_half: EMPTY_IMG,
        battery: EMPTY_IMG,
        club_0: EMPTY_IMG,
        club_1: EMPTY_IMG,
        club_2: EMPTY_IMG,
        club_3: EMPTY_IMG,
        circle_delta: EMPTY_IMG,
        circle_diameter: EMPTY_IMG,
        drift_angle: EMPTY_IMG,
        flight_level: EMPTY_IMG,
        fpm: EMPTY_IMG,
        fpm_100: EMPTY_IMG,
        ft: EMPTY_IMG,
        g: EMPTY_IMG,
        g_load: EMPTY_IMG,
        gear: EMPTY_IMG,
        glider: EMPTY_IMG,
        ias: EMPTY_IMG,
        km_h: EMPTY_IMG,
        kt: EMPTY_IMG,
        mph: EMPTY_IMG,
        m: EMPTY_IMG,
        m_s: EMPTY_IMG,
        pitch: EMPTY_IMG,
        roll_left: EMPTY_IMG,
        roll_right: EMPTY_IMG,
        slip: EMPTY_IMG,
        v: EMPTY_IMG,
        ve: EMPTY_IMG,
        yaw: EMPTY_IMG,
        normal_0: EMPTY_IMG,
        normal_1: EMPTY_IMG,
        normal_2: EMPTY_IMG,
        normal_3: EMPTY_IMG,
        north: EMPTY_IMG,
        sat: EMPTY_IMG,
        small_glider: EMPTY_IMG,
        speed_to_fly: EMPTY_IMG,
        spiral: EMPTY_IMG,
        straight: EMPTY_IMG,
        tas: EMPTY_IMG,
        true_course: EMPTY_IMG,
        wp_editor: EMPTY_IMG,
        wp_horizon: EMPTY_IMG,
        wp_horizon_ring: EMPTY_IMG,
        wp_vario360_5: EMPTY_IMG,
        wp_vario360_10: EMPTY_IMG,
        wp_vario360_5x: EMPTY_IMG,
        wp_vario360_10x: EMPTY_IMG,
        sat_small: EMPTY_IMG,
        bat_full_small: EMPTY_IMG,
        bat_half_small: EMPTY_IMG,
        bat_empty_small: EMPTY_IMG,
    }
}

pub const SMALL_FONT: FontRenderer = crate::utils::larus_fonts::TEXT_26;
pub const VALUE_FONT: FontRenderer = crate::utils::larus_fonts::VALUE_27;
pub const VALUE_FONT_MID: FontRenderer = crate::utils::larus_fonts::VALUE_24;
pub const VALUE_FONT_SMALL: FontRenderer = crate::utils::larus_fonts::VALUE_21;
pub const CAPTION_FONT: FontRenderer = crate::utils::larus_fonts::TEXT_11;
pub const HEADER_FONT: FontRenderer = crate::utils::larus_fonts::TEXT_18;
pub const BIG_FONT: FontRenderer = crate::utils::larus_fonts::TEXT_30;
/// value font of the three large info boxes
pub const VALUE_FONT_BIG: FontRenderer = crate::utils::larus_fonts::VALUE_38;
pub const VALUE_FONT_BIG_MID: FontRenderer = crate::utils::larus_fonts::VALUE_32;
pub const CAPTION_FONT_BIG: FontRenderer = crate::utils::larus_fonts::TEXT_14;

/// A 480x480 memory display that records every pixel it is asked to paint.
pub struct TestDisplay {
    pub buf: std::vec::Vec<Option<Colors>>,
}

impl TestDisplay {
    pub fn new() -> Self {
        TestDisplay {
            buf: std::vec![None; (DISPLAY_WIDTH * DISPLAY_HEIGHT) as usize],
        }
    }

    /// number of foreground pixels in the box `x0..=x1 / y0..=y1`
    ///
    /// The background fill of the page does not count - only pixels that
    /// carry ink, which is what "is something on the screen" means here.
    pub fn painted_box(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> usize {
        let mut n = 0;
        for y in y0.max(0)..=y1.min(DISPLAY_HEIGHT as i32 - 1) {
            for x in x0.max(0)..=x1.min(DISPLAY_WIDTH as i32 - 1) {
                match self.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] {
                    Some(Colors::Black) | None => (),
                    Some(_) => n += 1,
                }
            }
        }
        n
    }

    /// number of pixels of one colour in the rows `y0 ..= y1`
    #[allow(unused)]
    pub fn colored_rows(&self, y0: i32, y1: i32, color: Colors) -> usize {
        let mut n = 0;
        for y in y0.max(0)..=y1.min(DISPLAY_HEIGHT as i32 - 1) {
            for x in 0..DISPLAY_WIDTH as i32 {
                if self.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] == Some(color) {
                    n += 1;
                }
            }
        }
        n
    }
}

impl OriginDimensions for TestDisplay {
    fn size(&self) -> Size {
        Size::new(DISPLAY_WIDTH, DISPLAY_HEIGHT)
    }
}

impl DrawTarget for TestDisplay {
    type Color = Colors;
    type Error = CoreError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, c) in pixels {
            if p.x >= 0 && p.y >= 0 && p.x < DISPLAY_WIDTH as i32 && p.y < DISPLAY_HEIGHT as i32 {
                self.buf[(p.y * DISPLAY_WIDTH as i32 + p.x) as usize] = Some(c);
            }
        }
        Ok(())
    }
}

impl DrawImage for TestDisplay {
    const DISPLAY_WIDTH: u32 = DISPLAY_WIDTH;
    const DISPLAY_HEIGHT: u32 = DISPLAY_HEIGHT;

    unsafe fn draw_line_unchecked(&mut self, idx: usize, len: usize, color: Colors) {
        for i in idx..(idx + len).min(self.buf.len()) {
            self.buf[i] = Some(color);
        }
    }

    fn set_rotation(&mut self, _rotation: Rotation) {}
}

/// a model with the device constants of the V2 frontend
pub fn test_model() -> CoreModel {
    let mut cm = CoreModel::new(&DEVICE_CONST, 1234);
    // tests run with the full menu (Expert) unless they say otherwise
    cm.control.mode_checked = true;
    cm
}

/// debugging aid for view tests: write a test display as a PPM image
pub fn dump_ppm(display: &TestDisplay, path: &str) {
    use embedded_graphics::pixelcolor::{Rgb888, RgbColor};
    use std::io::Write;
    let mut f = std::fs::File::create(path).unwrap();
    write!(f, "P6 {} {} 255\n", DISPLAY_WIDTH, DISPLAY_HEIGHT).unwrap();
    for p in display.buf.iter() {
        let rgb: [u8; 3] = match p {
            None => [0, 0, 0],
            Some(c) => {
                let c = Rgb888::from(*c);
                [c.r(), c.g(), c.b()]
            }
        };
        f.write_all(&rgb).unwrap();
    }
}

/// A model plus a controller with fresh (static) queues - one expansion per
/// test, because a spsc queue may be split only once.
/// `let (mut cm, mut cc, mut tx) = test_harness!();` - `tx` is the consumer
/// side of the CAN transmit queue.
#[macro_export]
macro_rules! test_harness {
    () => {{
        use heapless::spsc::Queue;
        let (p_tx_frames, c_tx_frames) = {
            static mut Q_TX_FRAMES: $crate::QTxFrames<{ $crate::basic_config::MAX_TX_FRAMES }> =
                Queue::new();
            unsafe { (*core::ptr::addr_of_mut!(Q_TX_FRAMES)).split() }
        };
        let (p_idle_events, _c_idle_events) = {
            static mut Q_IDLE_EVENTS: $crate::utils::QIdleEvents = Queue::new();
            unsafe { (*core::ptr::addr_of_mut!(Q_IDLE_EVENTS)).split() }
        };
        let (_p_pers, c_pers) = {
            static mut Q_PERS: $crate::QPersistenceItems = Queue::new();
            unsafe { (*core::ptr::addr_of_mut!(Q_PERS)).split() }
        };
        let mut cm = $crate::view::test_fixture::test_model();
        let cc = $crate::CoreController::new(&mut cm, p_idle_events, c_pers, p_tx_frames);
        (cm, cc, c_tx_frames)
    }};
}

/// a model whose device constants carry the real scale wallpapers, loaded
/// from `dir` (device/larus_frontend_v2/assets), for screenshots that look
/// like the device. Missing files fall back to the empty image.
pub fn model_with_wallpapers(dir: &str) -> CoreModel {
    model_with_wallpapers_at(
        dir,
        DEVICE_CONST.sizes.vario.ring_bat_pos,
        DEVICE_CONST.sizes.vario.ring_sat_pos,
    )
}

/// like `model_with_wallpapers`, battery and satellite symbol of the ring
/// page at other positions (top left corner of the images)
pub fn model_with_wallpapers_at(dir: &str, bat: Point, sat: Point) -> CoreModel {
    fn load(dir: &str, name: &str) -> &'static [u8] {
        match std::fs::read(std::format!("{dir}/{name}")) {
            Ok(v) => std::boxed::Box::leak(v.into_boxed_slice()),
            Err(_) => EMPTY_IMG,
        }
    }
    let images = Images {
        wp_vario360_5: load(dir, "wp_vario360-5.lif"),
        wp_vario360_10: load(dir, "wp_vario360-10.lif"),
        wp_vario360_5x: load(dir, "wp_vario360-5x.lif"),
        wp_vario360_10x: load(dir, "wp_vario360-10x.lif"),
        sat_small: load(dir, "sat_s.lif"),
        bat_half_small: load(dir, "bat_half_s.lif"),
        bat_empty_small: load(dir, "bat_empty_s.lif"),
        bat_full_small: load(dir, "bat_full_s.lif"),
        // the glider symbols of the centre views (manual screenshots)
        gear: load(dir, "gear.lif"),
        glider: load(dir, "glider.lif"),
        small_glider: load(dir, "small_glider.lif"),
        north: load(dir, "north.lif"),
        ..images()
    };
    let sizes = crate::model::Sizes {
        vario: crate::model::VarioSizes {
            ring_bat_pos: bat,
            ring_sat_pos: sat,
            ..DEVICE_CONST.sizes.vario
        },
        ..DEVICE_CONST.sizes
    };
    let dc: &'static DeviceConst = std::boxed::Box::leak(std::boxed::Box::new(DeviceConst {
        images,
        sizes,
        ..DEVICE_CONST
    }));
    let mut cm = CoreModel::new(dc, 1234);
    cm.control.mode_checked = true;
    cm
}
