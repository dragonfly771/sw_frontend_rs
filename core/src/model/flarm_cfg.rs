//! Settings of the FLARM interface, the SteFly canopy flasher and the
//! FLARM warning pages (plan_flarm_blitzer.md, plan_flarm_warnung.md).
//!
//! No dependencies on the rest of the crate.

/// baud rates of the FLARM port, index 0 = automatic detection
pub const FLARM_BAUDS: [u32; 7] = [0, 4800, 9600, 19200, 38400, 57600, 115200];

/// what drives the SteFly canopy flasher (output DO1, D-Sub pin 9)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum FlasherType {
    #[default]
    Off = 0,
    /// flash pattern by FLARM alarm level (like the SteFly Control Box)
    Flarm = 1,
    /// without FLARM: level 0 pattern above the "On Speed"
    Airspeed = 2,
}

impl From<u8> for FlasherType {
    fn from(v: u8) -> Self {
        match v {
            1 => FlasherType::Flarm,
            2 => FlasherType::Airspeed,
            _ => FlasherType::Off,
        }
    }
}

impl FlasherType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FlasherType::Off => "Off",
            FlasherType::Flarm => "FLARM",
            FlasherType::Airspeed => "Airspeed",
        }
    }
}

/// when the flasher runs in FLARM mode (Control Box "Flasher Active")
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum FlasherActive {
    /// FLARM reports "airborne": pattern of the alarm level, also level 0
    #[default]
    Airborne = 0,
    /// airborne and alarm level > 0 only
    WarningOnly = 1,
}

impl From<u8> for FlasherActive {
    fn from(v: u8) -> Self {
        match v {
            1 => FlasherActive::WarningOnly,
            _ => FlasherActive::Airborne,
        }
    }
}

impl FlasherActive {
    pub fn as_str(&self) -> &'static str {
        match self {
            FlasherActive::Airborne => "Airborne",
            FlasherActive::WarningOnly => "Warning only",
        }
    }
}

/// Control Box "Flash Frequency"
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum FlashRate {
    Low = 0,
    #[default]
    Medium = 1,
    High = 2,
}

impl From<u8> for FlashRate {
    fn from(v: u8) -> Self {
        match v {
            0 => FlashRate::Low,
            2 => FlashRate::High,
            _ => FlashRate::Medium,
        }
    }
}

impl FlashRate {
    pub fn as_str(&self) -> &'static str {
        match self {
            FlashRate::Low => "Low",
            FlashRate::Medium => "Medium",
            FlashRate::High => "High",
        }
    }
}

/// how a FLARM warning is shown
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum WarnView {
    /// in the core (r 192), the scale keeps running outside
    #[default]
    Core = 0,
    /// whole display
    FullScreen = 1,
    /// no warning picture (cockpit with its own FLARM display)
    Off = 2,
}

impl From<u8> for WarnView {
    fn from(v: u8) -> Self {
        match v {
            1 => WarnView::FullScreen,
            2 => WarnView::Off,
            _ => WarnView::Core,
        }
    }
}

impl WarnView {
    pub fn as_str(&self) -> &'static str {
        match self {
            WarnView::Core => "Core",
            WarnView::FullScreen => "Full Screen",
            WarnView::Off => "Off",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FlarmConfig {
    /// index into FLARM_BAUDS, 0 = automatic
    pub baud_setting: u8,
    /// last baud rate that delivered valid sentences (start value of the scan)
    pub last_baud: u32,
    pub flasher_type: FlasherType,
    pub flasher_active: FlasherActive,
    pub flash_rate: FlashRate,
    /// Airspeed mode: flasher on above this IAS
    pub on_speed_kmh: f32,
    pub warn_view: WarnView,
    /// warnings are shown from this alarm level on (1..3)
    pub warn_level: u8,
    /// frontal view for head-on threats
    pub head_on: bool,
    /// FLARM id of the team mate, 0 = none
    pub team_id: u32,
}

impl Default for FlarmConfig {
    fn default() -> Self {
        FlarmConfig {
            baud_setting: 0,
            last_baud: 19200,
            flasher_type: FlasherType::Off,
            flasher_active: FlasherActive::Airborne,
            flash_rate: FlashRate::Medium,
            on_speed_kmh: 40.0,
            warn_view: WarnView::Core,
            warn_level: 1,
            head_on: true,
            team_id: 0,
        }
    }
}

impl FlarmConfig {
    /// EEPROM word: setting index in bits 0..3, index of the last good
    /// baud rate in bits 4..7
    pub fn baud_word(&self) -> u32 {
        let last = FLARM_BAUDS
            .iter()
            .position(|&b| b == self.last_baud)
            .unwrap_or(3) as u32;
        (self.baud_setting as u32 & 0x0f) | ((last & 0x0f) << 4)
    }

    pub fn set_baud_word(&mut self, w: u32) {
        let s = (w & 0x0f) as usize;
        self.baud_setting = if s < FLARM_BAUDS.len() { s as u8 } else { 0 };
        let l = ((w >> 4) & 0x0f) as usize;
        self.last_baud = match FLARM_BAUDS.get(l) {
            Some(&b) if b > 0 => b,
            _ => 19200,
        };
    }

    /// fixed baud rate, None = automatic
    pub fn fixed_baud(&self) -> Option<u32> {
        match FLARM_BAUDS.get(self.baud_setting as usize) {
            Some(&b) if b > 0 => Some(b),
            _ => None,
        }
    }
}

/// FLARM, radar page and canopy flasher are compiled in but switched off
/// unless the crate feature `flarm` is set (parked for the test versions,
/// flarm_feature_parked.md). Everything that shows or runs FLARM asks here.
pub const FLARM_FEATURE: bool = cfg!(feature = "flarm");
