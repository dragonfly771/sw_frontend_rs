use crate::{utils::Colors, CoreModel, HwVersion, SwVersion};
use embedded_graphics::geometry::Point;
use u8g2_fonts::FontRenderer;

use super::EditMode;

pub struct DeviceConst {
    pub dark_theme: Palette,
    pub bright_theme: Palette,
    pub big_font: FontRenderer,
    pub small_font: FontRenderer,
    /// Big value font of the ring design info boxes
    pub value_font: FontRenderer,
    /// One step smaller value font, used when a value gets 4 or more digits
    pub value_font_mid: FontRenderer,
    /// Smaller value font (top box) of the ring design
    pub value_font_small: FontRenderer,
    /// Small caption/unit font of the ring design info boxes
    pub caption_font: FontRenderer,
    /// Menu/editor header font of the ring design (between caption and small)
    pub header_font: FontRenderer,
    /// Value font of the three large info boxes (Big Boxes page core)
    pub value_font_big: FontRenderer,
    /// One step smaller, used when the large value would not fit
    pub value_font_big_mid: FontRenderer,
    /// Caption/unit font of the large info boxes - used when it fits
    pub caption_font_big: FontRenderer,
    pub images: Images,
    pub sizes: Sizes,
    pub misc: Misc,
}

impl CoreModel {
    pub fn palette(&self) -> &'static Palette {
        self.config.theme
    }
}

pub struct Sizes {
    pub vario: VarioSizes,
    pub horizon: HorizonSizes,
    pub display: DisplaySizes,
}

pub struct Misc {
    pub sw_version: SwVersion,
    pub hw_version: HwVersion,
    pub edit_mode: EditMode,
}

pub struct DisplaySizes {
    pub height: u32,
    pub width: u32,
    pub margin: u32,
    pub radius: u32,
    pub center: Point,
    pub screen_center: Point,
}

pub struct HorizonSizes {
    pub pitch_scale_len: i32,
    pub lt_swing_len: i32,
    pub lt_mid: i32,
    pub lt_rad: u32,
    /// radius the scaled horizon wallpaper `wp_horizon_ring` was drawn for.
    /// 0 = this device has no small horizon wallpaper.
    pub ring_wp_radius: i32,
}

pub struct VarioSizes {
    pub stf_diameter: u32, // stf speed to fly
    pub stf_width: u32,
    pub indicator_len: u32,
    pub attention_pos: Point,
    pub glider_pos: Point,
    pub north_pos: Point,
    pub bat_pos: Point,
    pub sat_pos: Point,
    pub profile_pos: Point,
    pub unit_pos: Point,
    pub info1_pos: Point,
    pub info2_pos: Point,
    pub info3_pos: Point,
    pub pic_info3_pos: Point,
    pub ta_circle_radius: u32, // ta thermal assistant
    pub ta_point_diameter: u32,
    pub wind_len: i32,
    pub wind_len_min: i32,
    pub scale_factor: f32, // angle in degrees betwenn 0 and the first entry
    // ring design (full 360 degree scale)
    pub ring_scale_factor: f32, // angle in degrees per unit, +/- full scale meet at 3 o'clock
    pub ring_info_top_pos: Point,
    pub ring_info_bottom_pos: Point,
    pub ring_info_r1_pos: Point,
    pub ring_info_r2_pos: Point,
    pub ring_info_r3_pos: Point,
    pub ring_bat_pos: Point,
    pub ring_sat_pos: Point,
    pub ring_profile_pos: Point,
    pub ring_attention_pos: Point,
}

pub struct Images {
    pub attention: &'static [u8],
    pub avg_climb_rate: &'static [u8],
    pub bat_empty: &'static [u8],
    pub bat_full: &'static [u8],
    pub bat_half: &'static [u8],
    pub battery: &'static [u8],
    pub club_0: &'static [u8],
    pub club_1: &'static [u8],
    pub club_2: &'static [u8],
    pub club_3: &'static [u8],
    pub circle_delta: &'static [u8],
    pub circle_diameter: &'static [u8],
    pub drift_angle: &'static [u8],
    pub flight_level: &'static [u8],
    pub fpm: &'static [u8],
    pub fpm_100: &'static [u8],
    pub ft: &'static [u8],
    pub g: &'static [u8],
    pub g_load: &'static [u8],
    pub gear: &'static [u8],
    pub glider: &'static [u8],
    pub ias: &'static [u8],
    pub km_h: &'static [u8],
    pub kt: &'static [u8],
    pub mph: &'static [u8],
    pub m: &'static [u8],
    pub m_s: &'static [u8],
    pub pitch: &'static [u8],
    pub roll_left: &'static [u8],
    pub roll_right: &'static [u8],
    pub slip: &'static [u8],
    pub v: &'static [u8],
    pub ve: &'static [u8],
    pub yaw: &'static [u8],
    pub normal_0: &'static [u8],
    pub normal_1: &'static [u8],
    pub normal_2: &'static [u8],
    pub normal_3: &'static [u8],
    pub north: &'static [u8],
    pub sat: &'static [u8],
    pub small_glider: &'static [u8],
    pub speed_to_fly: &'static [u8],
    pub spiral: &'static [u8],
    pub straight: &'static [u8],
    pub tas: &'static [u8],
    pub true_course: &'static [u8],
    pub wp_editor: &'static [u8],
    pub wp_horizon: &'static [u8],
    /// scaled horizon wallpaper for the inner circle of the ring design
    pub wp_horizon_ring: &'static [u8],
    pub wp_vario360_5: &'static [u8],
    pub wp_vario360_10: &'static [u8],
    /// overrange variants: the numbers of the negative half continue the
    /// positive scale (6..9 resp. 12..18), shown while the needle is beyond
    /// full scale
    pub wp_vario360_5x: &'static [u8],
    pub wp_vario360_10x: &'static [u8],
    // small status icons of the ring design
    pub sat_small: &'static [u8],
    pub bat_full_small: &'static [u8],
    pub bat_half_small: &'static [u8],
    pub bat_empty_small: &'static [u8],
}

#[derive(PartialEq)]
pub struct Palette {
    pub horizon: HorizonPalette,
    pub list_edit: ListEditorPalette,
    pub signal: SignalPalette,
    pub vario: VarioPalette,
}

impl Palette {
    pub const fn default() -> Self {
        Self {
            horizon: HorizonPalette::default(),
            list_edit: ListEditorPalette::default(),
            signal: SignalPalette::default(),
            vario: VarioPalette::default(),
        }
    }
}

#[derive(PartialEq)]
pub struct ListEditorPalette {
    pub background: Colors,
    pub header: Colors,
    pub item: Colors,
    pub frame: Colors,
    pub name: Colors,
    pub value: Colors,
}

impl ListEditorPalette {
    pub const fn default() -> Self {
        Self {
            background: Colors::Black,
            header: Colors::LightBlue,
            item: Colors::Bisque,
            frame: Colors::White,
            name: Colors::Bisque,
            value: Colors::White,
        }
    }
}

#[derive(PartialEq)]
pub struct HorizonPalette {
    pub sky: Colors,
    pub earth: Colors,
    pub scale: Colors,
    pub needle: Colors,
}

impl HorizonPalette {
    pub const fn default() -> Self {
        Self {
            sky: Colors::LightSkyBlue,
            earth: Colors::Sienna,
            scale: Colors::White,
            needle: Colors::Red,
        }
    }
}

#[derive(PartialEq)]
pub struct SignalPalette {
    pub stop: Colors,
    pub warning: Colors,
    pub go: Colors,
    pub alarm: Colors,
}

impl SignalPalette {
    pub const fn default() -> Self {
        Self {
            stop: Colors::Red,
            warning: Colors::Yellow,
            go: Colors::LimeGreen,
            alarm: Colors::OrangeRed,
        }
    }
}

#[derive(PartialEq)]
pub struct VarioPalette {
    pub background: Colors,
    pub scale: Colors,

    pub needle: Colors,
    pub avg_climb: Colors,
    pub mc_cready: Colors,
    pub stf_arc: Colors,

    // ring design indicator colors
    pub ring_needle: Colors,
    /// selection / editing in the menus (cursor frame, value field, lists):
    /// the blue of the pilot-set values, like the MC dot (AC 25-11: selected
    /// data cyan; orange is too close to the amber of cautions)
    pub ring_select: Colors,
    pub ring_avg_climb: Colors,
    pub ring_mc_cready: Colors,
    pub ring_wind_fill: Colors,
    pub ring_avg_wind_fill: Colors,

    pub wind_stroke: Colors,
    pub wind_fill: Colors,
    pub avg_wind_stroke: Colors,
    pub avg_wind_fill: Colors,
    pub wind_diff: Colors,

    pub icon: Colors,
    pub value: Colors,
    pub unit: Colors,

    pub therm_ass_best: Colors,
    pub therm_ass_good: Colors,
    pub therm_ass_bad: Colors,
    pub therm_ass_worst: Colors,
    pub therm2_ass_best: Colors,
    pub therm2_ass_good: Colors,
    pub therm2_ass_bad: Colors,
    pub therm2_ass_worst: Colors,
}

impl VarioPalette {
    pub const fn default() -> Self {
        Self {
            background: Colors::Black,
            scale: Colors::White,

            needle: Colors::DarkRed,
            avg_climb: Colors::Green,
            mc_cready: Colors::Red,
            stf_arc: Colors::Orange,

            ring_needle: Colors::Orange,
            ring_select: Colors::SkyK1,
            ring_avg_climb: Colors::Magenta,
            // MC in the blue of the pilot-set values (15.11.2026: the dot uses
            // its own palette entry now, same tone as the wind); the wind blue
            // (Stefan 28.10.2026: wind belongs to blue) is the sky of the
            // horizon - softer than DodgerBlue, the orange needle stays first;
            // the mean wind behind it grey as before P1
            ring_mc_cready: Colors::SkyK1,
            ring_wind_fill: Colors::SkyK1,
            ring_avg_wind_fill: Colors::DarkGray,

            wind_stroke: Colors::White,
            wind_fill: Colors::DodgerBlue,
            avg_wind_stroke: Colors::Blue,
            avg_wind_fill: Colors::LightGray,
            wind_diff: Colors::LightPink,

            icon: Colors::Orange,
            value: Colors::White,
            unit: Colors::DarkGray,

            therm_ass_best: Colors::Yellow,
            therm_ass_good: Colors::Red,
            therm_ass_bad: Colors::DeepSkyBlue,
            // white, not black: a black dot does not show on the black core
            // (Stefan 15.11.2026)
            therm_ass_worst: Colors::White,
            therm2_ass_best: Colors::Yellow,
            therm2_ass_good: Colors::Red,
            therm2_ass_bad: Colors::Blue,
            therm2_ass_worst: Colors::Black,
        }
    }
}
