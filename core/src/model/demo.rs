//! Demo mode for the fair (Deutscher Segelfliegertag 31.10.2026)
//!
//! Not stored: after a restart the device always runs with real data.
//! Only switched on at a standstill (IAS < 30 km/h) and switched off by
//! itself as soon as the real IAS exceeds 50 km/h. Demo values only go to
//! the display: the vario tone is silent, nothing is sent on NMEA.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum DemoMode {
    #[default]
    Off = 0,
    /// flight values and flap lever
    Flaps = 1,
    /// FLARM traffic scenarios
    Flarm = 2,
    /// both
    All = 3,
}

pub const DEMO_ON_MAX_KMH: f32 = 30.0;
pub const DEMO_OFF_KMH: f32 = 50.0;

impl From<u8> for DemoMode {
    fn from(v: u8) -> Self {
        match v {
            1 => DemoMode::Flaps,
            2 => DemoMode::Flarm,
            3 => DemoMode::All,
            _ => DemoMode::Off,
        }
    }
}

impl DemoMode {
    pub fn active(&self) -> bool {
        *self != DemoMode::Off
    }
    /// simulated flight values and flap lever
    pub fn flight(&self) -> bool {
        matches!(self, DemoMode::Flaps | DemoMode::All)
    }
    /// simulated FLARM traffic
    pub fn flarm(&self) -> bool {
        crate::model::flarm_cfg::FLARM_FEATURE && matches!(self, DemoMode::Flarm | DemoMode::All)
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            DemoMode::Off => "Off",
            DemoMode::Flaps => "Flaps",
            DemoMode::Flarm => "FLARM",
            DemoMode::All => "All",
        }
    }
}
