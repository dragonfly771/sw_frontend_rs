use crate::{
    controller::SND_EXP_MUL,
    system_of_units::{FloatToSpeed, Speed},
    tformat,
    view::viewable::{
        centerview::CenterView,
        vario_infoview::{Info3View, LineView},
    },
    CoreModel, Image, Length, Palette,
};
use core::convert::From;
use heapless::String;
use num::clamp;
use num_enum::FromPrimitive;

/// Possible displays
#[derive(Clone, Copy, PartialEq, FromPrimitive, Debug)]
#[repr(u8)]
pub enum DisplayActive {
    #[default]
    Vario,
    Horizon,
    DeviceInfo,
    Menu,
    FirmwareUpdate,
}

pub const DEVICE_INFO: &str = "Device Info";
pub const HORIZON: &str = "Horizon";
#[allow(unused)]
pub const VARIO: &str = "Vario";
pub const FIRMWARE_UPDATE: &str = "Firmware Update";
pub const MENU: &str = "Menu";

impl From<&str> for DisplayActive {
    fn from(value: &str) -> Self {
        match value {
            HORIZON => DisplayActive::Horizon,
            DEVICE_INFO => DisplayActive::DeviceInfo,
            FIRMWARE_UPDATE => DisplayActive::FirmwareUpdate,
            MENU => DisplayActive::Menu,
            _ => DisplayActive::Vario,
        }
    }
}

#[derive(Clone, Copy, PartialEq, FromPrimitive)]
#[repr(u8)]
pub enum DisplayTheme {
    #[default]
    Dark,
    Bright,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TypeOfInfo {
    None,
    WaterBallast,
    GearAlarm,
}

/// Behaviour of the instantaneous vario signal (needle and audio tone)
#[derive(Clone, Copy, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]
pub enum VarioNeedle {
    /// Needle and tone show the compensated vario unchanged
    #[default]
    Standard,
    /// Thermal entry optimized: phase controlled AoA correction from
    /// roll-in until ~1 circle, then fading out (Needle Factor k);
    /// applies to the needle and the audio vario alike
    EntryOptimized,
}

/// Gust filter for the instantaneous vario signal (needle and audio tone)
///
/// Two combined mechanisms (see plan_gust_filter.md): a PT2 cascade whose
/// time constant scales with the indicated airspeed (reference 130 km/h,
/// factor 0.6..1.3), and a soft knee compression that cuts high gust peaks
/// without added delay (deviations from a 2 s reference pass unchanged up
/// to 0.8 m/s, the excess is compressed to 30 %; straight flight only).
#[derive(Clone, Copy, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]
pub enum GustFilter {
    /// Passthrough, behaviour as before
    #[default]
    Off,
    /// PT2 2 x 0.25 s + knee: big gust peaks -50..-57 %
    Weak,
    /// PT2 2 x 0.35 s + knee: big gust peaks -58..-60 %
    Medium,
    /// PT2 2 x 0.50 s + knee: big gust peaks -63..-67 %
    Strong,
    /// Knee compression only: no added delay, big gust peaks -36..-45 %
    PeakCut,
}

/// Source of the red average marker on the ring scale
#[derive(Clone, Copy, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]
pub enum AvgMarker {
    /// Average climb rate (source per "Avg Source")
    #[default]
    Average,
    /// Potential climb (POT); falls back to the average while POT
    /// shows no value (outside circling)
    Potential,
}

/// Protocol spoken on the serial NMEA port of the display
///
/// `Larus`: the Larus sentences for a navigation computer (XCSoar, OpenSoar,
/// LK8000): GPS, vario, wind, attitude out, `$PLARS,H,..` settings in.
/// `Lxnav`: receive only, on the PDA/PC port of an LXNAV main unit: MC,
/// ballast and bugs from `$LXWP2`, the circling /
/// cruise state from `$PLXVS` (effective with Mode Switch = NMEA). Nothing
/// is sent, the main unit stays the master.
#[derive(Clone, Copy, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]
pub enum NmeaProtocol {
    #[default]
    Larus,
    Lxnav,
}

pub const NMEA_PROTOCOL_LARUS: &str = "XCSoar";
pub const NMEA_PROTOCOL_LXNAV: &str = "LXNAV";

impl NmeaProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            NmeaProtocol::Larus => NMEA_PROTOCOL_LARUS,
            NmeaProtocol::Lxnav => NMEA_PROTOCOL_LXNAV,
        }
    }
}

/// baud rates of the NMEA port; the index is what the EEPROM stores
pub const NMEA_BAUDS: [u32; 6] = [4800, 9600, 19200, 38400, 57600, 115200];
/// index of 38 400 Bd, the rate the port had before it became a setting
pub const NMEA_BAUD_DEFAULT_IDX: u8 = 3;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum OverlayActive {
    None,
    Editor,
    Menu,
    Info,
}

#[derive(Clone, Copy, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]
pub enum UnitHorizontalSpeed {
    #[default]
    Kmph,
    Mph,
    Knots,
}

pub const UNIT_KMPH: &str = "km/h";
pub const UNIT_KNOTS: &str = "knots";
pub const UNIT_MPH: &str = "mph";

impl UnitHorizontalSpeed {
    pub fn from_str(name: &str) -> Self {
        match name {
            UNIT_KNOTS => UnitHorizontalSpeed::Knots,
            UNIT_MPH => UnitHorizontalSpeed::Mph,
            _ => UnitHorizontalSpeed::Kmph,
        }
    }

    pub fn as_f32(&self, speed: Speed) -> f32 {
        match self {
            UnitHorizontalSpeed::Kmph => speed.to_km_h(),
            UnitHorizontalSpeed::Mph => speed.to_mph(),
            UnitHorizontalSpeed::Knots => speed.to_kt(),
        }
    }

    pub fn as_speed(&self, value: f32) -> Speed {
        match self {
            UnitHorizontalSpeed::Kmph => Speed::from_km_h(value),
            UnitHorizontalSpeed::Knots => Speed::from_kt(value),
            UnitHorizontalSpeed::Mph => Speed::from_mph(value),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            UnitHorizontalSpeed::Kmph => UNIT_KMPH,
            UnitHorizontalSpeed::Knots => UNIT_KNOTS,
            UnitHorizontalSpeed::Mph => UNIT_MPH,
        }
    }

    pub fn image(&self, cm: &CoreModel) -> Image {
        let data = match self {
            UnitHorizontalSpeed::Kmph => cm.device_const.images.km_h,
            UnitHorizontalSpeed::Mph => cm.device_const.images.mph,
            UnitHorizontalSpeed::Knots => cm.device_const.images.kt,
        };
        Image::new(data)
    }

    pub fn value_str(&self, speed: Speed) -> String<3> {
        let s = clamp(self.as_f32(speed), -99.0, 999.0);
        tformat!(3, "{:.0}", s).unwrap()
    }
}

#[derive(Clone, Copy, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]
pub enum UnitVerticalSpeed {
    #[default]
    Mps,
    Fpm,
    Knots,
}

pub const UNIT_MPS: &str = "m/s";
pub const UNIT_FPM: &str = "fpm";

impl UnitVerticalSpeed {
    pub fn from_str(name: &str) -> Self {
        match name {
            UNIT_KNOTS => UnitVerticalSpeed::Knots,
            UNIT_FPM => UnitVerticalSpeed::Fpm,
            _ => UnitVerticalSpeed::Mps,
        }
    }

    pub fn as_f32(&self, speed: Speed) -> f32 {
        match self {
            UnitVerticalSpeed::Mps => speed.to_m_s(),
            UnitVerticalSpeed::Fpm => speed.to_ft_min(),
            UnitVerticalSpeed::Knots => speed.to_kt(),
        }
    }

    pub fn as_speed(&self, value: f32) -> Speed {
        match self {
            UnitVerticalSpeed::Mps => Speed::from_m_s(value),
            UnitVerticalSpeed::Fpm => Speed::from_ft_min(value),
            UnitVerticalSpeed::Knots => Speed::from_kt(value),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            UnitVerticalSpeed::Mps => UNIT_MPS,
            UnitVerticalSpeed::Fpm => UNIT_FPM,
            UnitVerticalSpeed::Knots => UNIT_KNOTS,
        }
    }

    pub fn image(&self, cm: &CoreModel) -> Image {
        let data = match self {
            UnitVerticalSpeed::Mps => cm.device_const.images.m_s,
            UnitVerticalSpeed::Fpm => cm.device_const.images.fpm,
            UnitVerticalSpeed::Knots => cm.device_const.images.kt,
        };
        Image::new(data)
    }

    pub fn value_str(&self, speed: Speed) -> String<5> {
        match self {
            UnitVerticalSpeed::Mps => {
                let s = clamp(speed.to_m_s(), -99.0, 99.0);
                tformat!(5, "{:.1}", s).unwrap()
            }
            UnitVerticalSpeed::Fpm => {
                let s = clamp(speed.to_ft_min(), -9999.0, 9999.0);
                tformat!(5, "{:.0}", s).unwrap()
            }
            UnitVerticalSpeed::Knots => {
                let s = clamp(speed.to_kt(), -99.0, 99.0);
                tformat!(5, "{:.1}", s).unwrap()
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]
pub enum UnitHeight {
    #[default]
    Meter,
    Feet,
}

pub const UNIT_METER: &str = "m";
pub const UNIT_FEET: &str = "ft";

impl UnitHeight {
    pub fn from_str(name: &str) -> Self {
        match name {
            UNIT_FEET => UnitHeight::Feet,
            _ => UnitHeight::Meter,
        }
    }

    pub fn as_f32(&self, height: Length) -> f32 {
        match self {
            UnitHeight::Meter => height.to_m(),
            UnitHeight::Feet => height.to_ft(),
        }
    }

    pub fn as_length(&self, value: f32) -> Length {
        match self {
            UnitHeight::Meter => Length::from_m(value),
            UnitHeight::Feet => Length::from_ft(value),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            UnitHeight::Meter => UNIT_METER,
            UnitHeight::Feet => UNIT_FEET,
        }
    }

    pub fn image(&self, cm: &CoreModel) -> Image {
        let data = match self {
            UnitHeight::Meter => cm.device_const.images.m,
            UnitHeight::Feet => cm.device_const.images.ft,
        };
        Image::new(data)
    }

    pub fn value_str(&self, height: Length) -> String<5> {
        let h = match self {
            UnitHeight::Meter => height.to_m(),
            UnitHeight::Feet => height.to_ft(),
        };
        let h = clamp(h, -9999.0, 99999.0);
        tformat!(5, "{:.0}", h).unwrap()
    }
}

/// Auto switch mode Vario <-> SpeedToFly (Editable "Auto Switch")
pub const STF_SWITCH_MODE_POLAR: u8 = 0;
pub const STF_SWITCH_MODE_FIXED: u8 = 1;
pub const STF_SWITCH_MODE_BALLAST: u8 = 2;
pub const STF_SWITCH_POLAR: &str = "Polar";
pub const STF_SWITCH_FIXED: &str = "Fixed IAS";
pub const STF_SWITCH_BALLAST: &str = "IAS + Ballast";

/// The display backlight can never be dimmed below this (stays readable)
pub const BRIGHTNESS_MIN: u8 = 10;

/// Sink tone mode (eeprom compatible with the old bool 0/1)
pub const SINK_MODE_CONTINUOUS: u8 = 0;
pub const SINK_MODE_DOUBLE_BEEP: u8 = 1;
pub const SINK_MODE_OFF: u8 = 2;

/// Metastructur for config variables
#[derive(Clone, Copy)]
pub struct Config {
    pub circle_hysteresis_tc: i8,
    pub display_active: DisplayActive,
    pub overlay_active: OverlayActive,
    pub type_of_info: TypeOfInfo,
    pub last_display_active: DisplayActive,
    pub user_profile: u8,
    pub glider_idx: i32,
    pub volume: i8,
    /// volume in speed to fly mode (cruise); -1 = like `volume` (not set yet)
    pub volume_stf: i8,
    pub mc_cready: Speed,
    pub snd_min_freq: f32,
    pub snd_center_freq: f32,
    pub snd_max_freq: f32,
    pub snd_exp_mul: f32,
    pub snd_duty_cycle: u16, // Oscillations, symetric on/off
    /// Frequency the tone reaches at +reference (anchor)
    pub snd_freq_upper: f32,
    /// Frequency the tone reaches at -reference (anchor)
    pub snd_freq_lower: f32,
    /// Climb rate the two anchors above refer to, in m/s
    pub snd_full_scale: f32,
    /// Climb rate (negative) down to which the zero zone ticks are produced, 0.0 = off
    pub snd_ticks_until: f32,
    /// Climb rate (negative) from which the sink tone starts
    pub snd_sink_from: f32,
    /// Sink tone mode: SINK_MODE_CONTINUOUS, SINK_MODE_DOUBLE_BEEP or SINK_MODE_OFF
    pub snd_sink_mode: u8,
    /// Tempo multiplier of the Stefly climb beep, 1.0 = as tabulated
    pub snd_beep_tempo: f32,
    /// Time constant of the audio damping in s
    pub snd_audio_tc: f32,
    /// Attack time of the tone envelope in ms, the release is twice as long
    pub snd_attack_ms: f32,
    /// Strength of the frequency dependent loudness compensation, 0.0 = off, 1.0 = the curve
    pub snd_loudness_comp: f32,
    /// Airspeed dependent volume: level increase in dB per 10 km/h above the reference, 0 = off
    pub snd_speed_vol_slope: f32,
    /// Airspeed dependent volume: reference IAS in km/h, below it nothing happens
    pub snd_speed_vol_ref: f32,
    pub av2_climb_rate_tc: f32,
    pub av_speed_to_fly_tc: f32,
    pub av_supply_voltage_tc: f32,
    pub theme: &'static Palette,
    pub uuid: u32,
    pub info1_vario: LineView,
    pub info2_vario: LineView,
    pub info3_vario: Info3View,
    pub info1_stf: LineView,
    pub info2_stf: LineView,
    pub info3_stf: Info3View,
    pub center_circling: CenterView,
    pub center_straight: CenterView,
    pub glider_symbol: bool,
    /// Instantaneous vario needle: standard or thermal entry optimized
    pub vario_needle: VarioNeedle,
    /// Gust filter strength for needle and audio tone
    pub gust_filter: GustFilter,
    /// Ring design: source of the red average marker on the scale
    pub ring_avg_marker: AvgMarker,
    /// Apply the knee compression of the gust filter while circling too
    /// (default off: while circling the thermal texture stays untouched)
    pub gust_filter_circling: bool,
    /// Entry optimized needle: correction factor k in m/s per degree AoA
    /// (the name is from the time the POT box used it too)
    pub pot_k: f32,
    /// POT info box: bar of the yes / no as a factor of the MacCready value
    /// (1.0 = the MacCready value itself, at least 0.5 m/s)
    pub pot_bar: f32,
    /// the POT box shows its yes / no symbol (tick, cross)
    pub pot_advice: bool,
    /// Ring design: content of the three info boxes in the right half
    pub info_r1: LineView,
    pub info_r2: LineView,
    pub info_r3: LineView,
    pub battery_good: f32,
    pub battery_low: f32,
    pub stf_upper_limit: Speed,
    pub stf_lower_limit: Speed,
    /// Auto switch mode: STF_SWITCH_MODE_POLAR / _FIXED / _BALLAST
    pub stf_switch_mode: u8,
    /// Pilot selected switch speed (IAS) for the fixed auto switch modes;
    /// in the ballast mode it refers to the glider without water ballast
    pub stf_switch_speed: Speed,
    /// Allow installing an older firmware version from SD card at boot
    pub allow_fw_downgrade: bool,
    /// Display backlight brightness in percent (BRIGHTNESS_MIN..=100)
    pub display_brightness: u8,
    /// freely configurable pages (Stefly design)
    pub pages: [crate::model::page::Page; crate::model::page::MAX_PAGES],
    /// index of the page currently shown
    pub page_idx: u8,
    /// flap table of the glider (plan_klappen_menue.md)
    pub flaps: crate::model::flaps::FlapTable,
    /// stage the flap stage editor works on
    pub flap_edit: u8,
    /// FLARM interface, canopy flasher, warning pages
    pub flarm: crate::model::flarm_cfg::FlarmConfig,
    /// protocol and baud rate of the serial NMEA port (XCSoar / LXNAV)
    pub nmea_protocol: NmeaProtocol,
    /// index into NMEA_BAUDS
    pub nmea_baud_idx: u8,
    /// big encoder switches pages (true) or adjusts MC as before (false)
    pub big_encoder_pages: bool,
    pub vario_lower_limit: Speed,
    pub vario_upper_limit: Speed,
    pub unit_horizontal_speed: UnitHorizontalSpeed,
    pub unit_vertical_speed: UnitVerticalSpeed,
    pub unit_height: UnitHeight,
    pub club_mode: bool,
    /// reduced menus for the pilot who never opens the manual (delivery
    /// state of a new device; Expert = false)
    pub basic_mode: bool,
    pub usage_code: u16,
}

impl Config {
    /// baud rate of the NMEA port in bit/s
    pub fn nmea_baud(&self) -> u32 {
        NMEA_BAUDS[(self.nmea_baud_idx as usize).min(NMEA_BAUDS.len() - 1)]
    }

    pub fn default(theme: &'static Palette, uuid: u32) -> Self {
        Self {
            circle_hysteresis_tc: 7,
            display_active: DisplayActive::Vario,
            overlay_active: OverlayActive::None,
            type_of_info: TypeOfInfo::None,
            last_display_active: DisplayActive::Vario,
            user_profile: 0,
            glider_idx: crate::flight_physics::polar_store::DEFAULT_GLIDER_IDX,
            volume: 2,
            volume_stf: -1,
            mc_cready: 0.7.m_s(),
            snd_min_freq: 200.0,
            snd_center_freq: 500.0,
            snd_max_freq: 3000.0,
            snd_exp_mul: SND_EXP_MUL,
            snd_duty_cycle: 200,
            // The three anchors are chosen so that the curve is NOT stretched
            // at all by default: 1930 Hz and 318 Hz are exactly what the tone
            // table produces at +-5 m/s with a 500 Hz centre frequency.
            snd_freq_upper: 1930.0,
            snd_freq_lower: 318.0,
            snd_full_scale: 5.0,
            snd_ticks_until: -0.5,
            snd_sink_from: -1.0,
            snd_sink_mode: SINK_MODE_DOUBLE_BEEP,
            snd_beep_tempo: 1.0,
            snd_audio_tc: 0.2,
            snd_attack_ms: 8.0,
            snd_loudness_comp: 1.0,
            snd_speed_vol_slope: 1.0,
            snd_speed_vol_ref: 100.0,
            av2_climb_rate_tc: 30.0,
            av_speed_to_fly_tc: 5.0,
            av_supply_voltage_tc: 3.0,
            theme,
            uuid,
            info1_vario: LineView::None,
            info2_vario: LineView::WindAndDelta,
            info3_vario: Info3View::Climbing,
            info1_stf: LineView::None,
            info2_stf: LineView::WindAndDelta,
            info3_stf: Info3View::SpeedToFly,
            center_circling: CenterView::SingleArrowCircling,
            center_straight: CenterView::SingleArrowStraight,
            glider_symbol: false, // wind arrow without the outline behind it (Stefan 16.11.2026)
            vario_needle: VarioNeedle::Standard,
            gust_filter: GustFilter::Off,
            gust_filter_circling: false,
            ring_avg_marker: AvgMarker::Average,
            pot_k: -0.18,
            pot_bar: 1.0,
            pot_advice: true,
            info_r1: LineView::Altitude,
            info_r2: LineView::AverageClimbRate,
            info_r3: LineView::FlightLevel,
            battery_good: 11.5,
            battery_low: 10.0,
            stf_upper_limit: 10.0.km_h(),
            stf_lower_limit: -10.0.km_h(),
            stf_switch_mode: STF_SWITCH_MODE_POLAR,
            stf_switch_speed: 100.0.km_h(),
            allow_fw_downgrade: false,
            display_brightness: 100,
            pages: crate::model::page::default_pages(),
            page_idx: 0,
            flaps: crate::model::flaps::FlapTable::default(),
            flap_edit: 0,
            flarm: crate::model::flarm_cfg::FlarmConfig::default(),
            nmea_protocol: NmeaProtocol::Larus,
            nmea_baud_idx: NMEA_BAUD_DEFAULT_IDX,
            big_encoder_pages: true,
            vario_upper_limit: 0.0.m_s(),
            vario_lower_limit: 0.0.m_s(),
            unit_horizontal_speed: UnitHorizontalSpeed::Kmph,
            unit_vertical_speed: UnitVerticalSpeed::Mps,
            unit_height: UnitHeight::Meter,
            club_mode: false,
            basic_mode: false,
            usage_code: 0,
        }
    }

    /// the page currently shown
    /// volume of the cruise (speed to fly) tone
    pub fn stf_volume(&self) -> i8 {
        if self.volume_stf < 0 {
            self.volume
        } else {
            self.volume_stf
        }
    }

    pub fn page(&self) -> &crate::model::page::Page {
        &self.pages[(self.page_idx as usize).min(crate::model::page::MAX_PAGES - 1)]
    }

    /// content of an info box slot of the active page
    pub fn box_view(&self, slot: usize, vario_mode: bool) -> crate::LineView {
        self.page().box_view(slot, vario_mode)
    }

    /// next page index in the given direction, skipping pages that are off
    pub fn next_page(&self, forward: bool) -> u8 {
        // club mode has two fixed pages and no page editor
        let n = if self.club_mode {
            2
        } else {
            crate::model::page::MAX_PAGES
        };
        let cur = (self.page_idx as usize).min(n - 1);
        for step in 1..=n {
            let idx = if forward {
                (cur + step) % n
            } else {
                (cur + n - step) % n
            };
            if self.pages[idx].is_on() {
                return idx as u8;
            }
        }
        self.page_idx
    }

    pub fn is_base_display(&self) -> bool {
        self.display_active == DisplayActive::Vario
            || self.display_active == DisplayActive::Horizon
            || self.display_active == DisplayActive::DeviceInfo
    }
}
