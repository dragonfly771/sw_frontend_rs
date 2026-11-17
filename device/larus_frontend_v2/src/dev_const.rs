use crate::{HW_VERSION, SW_VERSION};
use corelib::{
    DeviceConst, DisplaySizes, EditMode, HorizonSizes, Images, Misc, Palette, Sizes, VarioSizes,
};
use embedded_graphics::geometry::Point;
use u8g2_fonts::FontRenderer;

pub const DISPLAY_HEIGHT: u32 = 480;
pub const DISPLAY_WIDTH: u32 = 480;

#[macro_export]
macro_rules! include_bytes_aligned {
    ($align_to:expr, $path:expr) => {{
        #[repr(C, align($align_to))]
        struct __Aligned<T: ?Sized>(T);
        const __DATA: &'static __Aligned<[u8]> = &__Aligned(*include_bytes!($path));
        &__DATA.0
    }};
}

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
        sw_version: SW_VERSION,
        hw_version: HW_VERSION,
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

const fn images() -> Images {
    Images {
        attention: include_bytes_aligned!(4, "../assets/attention.lif"),
        avg_climb_rate: include_bytes_aligned!(4, "../assets/avg_climb_rate.lif"),
        bat_empty: include_bytes_aligned!(4, "../assets/bat_empty.lif"),
        bat_full: include_bytes_aligned!(4, "../assets/bat_full.lif"),
        bat_half: include_bytes_aligned!(4, "../assets/bat_half.lif"),
        battery: include_bytes_aligned!(4, "../assets/battery.lif"),
        club_0: include_bytes_aligned!(4, "../assets/club-0.lif"),
        club_1: include_bytes_aligned!(4, "../assets/club-1.lif"),
        club_2: include_bytes_aligned!(4, "../assets/club-2.lif"),
        club_3: include_bytes_aligned!(4, "../assets/club-3.lif"),
        circle_delta: include_bytes_aligned!(4, "../assets/circle_delta.lif"),
        circle_diameter: include_bytes_aligned!(4, "../assets/circle_diameter.lif"),
        drift_angle: include_bytes_aligned!(4, "../assets/drift_angle.lif"),
        flight_level: include_bytes_aligned!(4, "../assets/flight_level.lif"),
        fpm: include_bytes_aligned!(4, "../assets/fpm.lif"),
        fpm_100: include_bytes_aligned!(4, "../assets/fpm-100.lif"),
        ft: include_bytes_aligned!(4, "../assets/ft.lif"),
        g: include_bytes_aligned!(4, "../assets/g.lif"),
        g_load: include_bytes_aligned!(4, "../assets/g-load.lif"),
        gear: include_bytes_aligned!(4, "../assets/gear.lif"),
        glider: include_bytes_aligned!(4, "../assets/glider.lif"),
        ias: include_bytes_aligned!(4, "../assets/ias.lif"),
        km_h: include_bytes_aligned!(4, "../assets/km_h.lif"),
        kt: include_bytes_aligned!(4, "../assets/kt.lif"),
        mph: include_bytes_aligned!(4, "../assets/mph.lif"),
        m: include_bytes_aligned!(4, "../assets/m.lif"),
        m_s: include_bytes_aligned!(4, "../assets/m_s.lif"),
        pitch: include_bytes_aligned!(4, "../assets/pitch.lif"),
        roll_left: include_bytes_aligned!(4, "../assets/roll_left.lif"),
        roll_right: include_bytes_aligned!(4, "../assets/roll_right.lif"),
        slip: include_bytes_aligned!(4, "../assets/slip.lif"),
        v: include_bytes_aligned!(4, "../assets/V.lif"),
        ve: include_bytes_aligned!(4, "../assets/Ve.lif"),
        yaw: include_bytes_aligned!(4, "../assets/yaw.lif"),
        normal_0: include_bytes_aligned!(4, "../assets/normal-0.lif"),
        normal_1: include_bytes_aligned!(4, "../assets/normal-1.lif"),
        normal_2: include_bytes_aligned!(4, "../assets/normal-2.lif"),
        normal_3: include_bytes_aligned!(4, "../assets/normal-3.lif"),
        north: include_bytes_aligned!(4, "../assets/north.lif"),
        sat: include_bytes_aligned!(4, "../assets/sat.lif"),
        small_glider: include_bytes_aligned!(4, "../assets/small_glider.lif"),
        speed_to_fly: include_bytes_aligned!(4, "../assets/speed_to_fly.lif"),
        spiral: include_bytes_aligned!(4, "../assets/spiral.lif"),
        straight: include_bytes_aligned!(4, "../assets/straight.lif"),
        tas: include_bytes_aligned!(4, "../assets/tas.lif"),
        true_course: include_bytes_aligned!(4, "../assets/true_course.lif"),
        wp_editor: include_bytes_aligned!(4, "../assets/wp_editor.lif"),
        wp_horizon: include_bytes_aligned!(4, "../assets/wp_horizon.lif"),
        wp_horizon_ring: include_bytes_aligned!(4, "../assets/wp_horizon_ring.lif"),
        wp_vario360_5: include_bytes_aligned!(4, "../assets/wp_vario360-5.lif"),
        wp_vario360_10: include_bytes_aligned!(4, "../assets/wp_vario360-10.lif"),
        wp_vario360_5x: include_bytes_aligned!(4, "../assets/wp_vario360-5x.lif"),
        wp_vario360_10x: include_bytes_aligned!(4, "../assets/wp_vario360-10x.lif"),
        sat_small: include_bytes_aligned!(4, "../assets/sat_s.lif"),
        bat_full_small: include_bytes_aligned!(4, "../assets/bat_full_s.lif"),
        bat_half_small: include_bytes_aligned!(4, "../assets/bat_half_s.lif"),
        bat_empty_small: include_bytes_aligned!(4, "../assets/bat_empty_s.lif"),
    }
}

pub const SMALL_FONT: FontRenderer = corelib::larus_fonts::TEXT_26;
pub const VALUE_FONT: FontRenderer = corelib::larus_fonts::VALUE_27;
pub const VALUE_FONT_MID: FontRenderer = corelib::larus_fonts::VALUE_24;
pub const VALUE_FONT_SMALL: FontRenderer = corelib::larus_fonts::VALUE_21;
pub const CAPTION_FONT: FontRenderer = corelib::larus_fonts::TEXT_11;
pub const HEADER_FONT: FontRenderer = corelib::larus_fonts::TEXT_18;
pub const BIG_FONT: FontRenderer = corelib::larus_fonts::TEXT_30;
/// value font of the three large info boxes
pub const VALUE_FONT_BIG: FontRenderer = corelib::larus_fonts::VALUE_38;
pub const VALUE_FONT_BIG_MID: FontRenderer = corelib::larus_fonts::VALUE_32;
pub const CAPTION_FONT_BIG: FontRenderer = corelib::larus_fonts::TEXT_14;
