//! Setup menu of the normal mode (ring design), grouped by what the pilot
//! thinks about: Vario, Speed to Fly, Sound, Display, Units,
//! Aircraft, Sensorbox (installation and calibration), System.
use crate::{
    model::menu::{
        Menu, MenuItem, MenuItemContent, DISPLAY_IDX, FLIGHT_MENU, ROOT, ROOT_IDX, SETTINGS_IDX,
    },
    Editable,
};

// Indices into MENU_LIST. 0..4 are shared with the club list (mod.rs):
// ROOT, FLIGHT_MENU, the setup menu (SETTINGS_IDX) and the display menu
// (DISPLAY_IDX); index 4 is a free slot.
pub const DRAIN_SETTINGS_IDX: usize = 5;
pub const POLAR_SETTINGS_IDX: usize = 6;
pub const SENSORBOX_IDX: usize = 7;
pub const SENSORBOX_ORIENTATION_IDX: usize = 8;
pub const SPEED_TO_FLY_IDX: usize = 9;
pub const LANDING_GEAR_ALARM_IDX: usize = 10;
pub const VARIO_IDX: usize = 11;
pub const UNITS_IDX: usize = 12;
pub const VARIO_AVERAGE_IDX: usize = 13;
pub const VARIO_POT_IDX: usize = 14;
pub const STF_MODE_SWITCH_IDX: usize = 15;
pub const SYSTEM_BATTERY_IDX: usize = 16;
pub const SOUND_IDX: usize = 17;
pub const FLASH_CONTROL_IDX: usize = 18;
pub const SENSORBOX_SERVICE_IDX: usize = 19;
pub const SOUND_FREQUENCY_IDX: usize = 20;
pub const SOUND_STEFLY_IDX: usize = 21;
pub const SOUND_LOUDNESS_IDX: usize = 22;
pub const SENSORBOX_PITOT_IDX: usize = 23;
pub const SENSORBOX_GNSS_IDX: usize = 24;
pub const STEFLY_AIRCRAFT_IDX: usize = 25;
pub const STEFLY_SYSTEM_IDX: usize = 26;
pub const SYSTEM_FIRMWARE_IDX: usize = 30;
pub const SYSTEM_RESET_IDX: usize = 31;
pub const DISPLAY_WIND_IDX: usize = 32;
pub const FLARM_IDX: usize = 33;
pub const FLARM_WARNINGS_IDX: usize = 34;
pub const FLAPS_IDX: usize = 35;
pub const FLAP_STAGES_IDX: usize = 36;
pub const FLAP_EDIT_IDX: usize = 37;
pub const NMEA_PORT_IDX: usize = 38;
/// the former "Flash Control" slot holds the SteFly canopy flasher now
pub const STEFLY_FLASHER_IDX: usize = FLASH_CONTROL_IDX;

pub const MENU_LIST: &[Menu] = &[
    ROOT,
    FLIGHT_MENU,
    STEFLY_SETTINGS, // SETTINGS_IDX
    DISPLAY,         // DISPLAY_IDX
    ROOT,            // free slot
    DRAIN_SETTINGS,
    POLAR_SETTINGS,
    SENSORBOX,
    SENSORBOX_ORIENTATION,
    SPEED_TO_FLY,
    LANDING_GEAR_ALARM,
    VARIO,
    UNITS,
    VARIO_AVERAGE,
    VARIO_POT,
    STF_MODE_SWITCH,
    SYSTEM_BATTERY,
    SOUND,
    STEFLY_FLASHER, // FLASH_CONTROL_IDX
    SENSORBOX_SERVICE,
    SOUND_FREQUENCY,
    SOUND_STEFLY,
    SOUND_LOUDNESS,
    SENSORBOX_PITOT,
    SENSORBOX_GNSS,
    STEFLY_AIRCRAFT,
    STEFLY_SYSTEM,
    ROOT, // free slot (the pages are set up on the page: Page Layout)
    ROOT, // free slot
    ROOT, // free slot
    SYSTEM_FIRMWARE,
    SYSTEM_RESET,
    DISPLAY_WIND,
    FLARM,
    FLARM_WARNINGS,
    FLAPS,
    FLAP_STAGES,
    FLAP_EDIT,
    NMEA_PORT,
];

/// Top level of the setup (ring design): what the pilot thinks about, most used first. The dynamic parent stack in MenuControl brings every Return back here.
pub const STEFLY_SETTINGS: Menu = Menu {
    name: "Setup",
    level: 1,
    items: &[
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: VARIO_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SPEED_TO_FLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SOUND_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: DISPLAY_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: UNITS_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: STEFLY_AIRCRAFT_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: FLARM_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SENSORBOX_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: ROOT_IDX,
        },
    ],
};

/// Vario signal: the sensor box filter (a box value, no restart), gust filter, needle signal, average and POT
pub const VARIO: Menu = Menu {
    name: "Vario",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::VarioTc),
            next_menu_idx: VARIO_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::GustFilterMode),
            next_menu_idx: VARIO_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::GustCircling),
            next_menu_idx: VARIO_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::NeedleMode),
            next_menu_idx: VARIO_IDX,
        },
        // only with Vario Signal = Entry Optimized (menu/mod.rs item_visible)
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PotK),
            next_menu_idx: VARIO_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: VARIO_AVERAGE_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: VARIO_POT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// Expert values of the vario average (15.11.2026: out of the pilot's way -
/// one clearly named value per box, the sources and time constants here):
/// its source and the averaging times of frontend and box
pub const VARIO_AVERAGE: Menu = Menu {
    name: "Expert",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::AvgClimbRateSrc),
            next_menu_idx: VARIO_AVERAGE_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::TcClimbRate),
            next_menu_idx: VARIO_AVERAGE_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::VarioIntTc),
            next_menu_idx: VARIO_AVERAGE_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: VARIO_IDX,
        },
    ],
};

/// Experimental: the potential climb (POT box) - what the marker on the
/// ring shows and the bar of the yes / no. For trying out, may change.
pub const VARIO_POT: Menu = Menu {
    name: "Experimental",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::AvgMarkerMode),
            next_menu_idx: VARIO_POT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PotBar),
            next_menu_idx: VARIO_POT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PotAdvice),
            next_menu_idx: VARIO_POT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: VARIO_IDX,
        },
    ],
};

/// Speed to fly with everything that switches between vario and speed to
/// fly (the former sub menu "Mode Switch", flattened 28.10.2026: one level
/// less for 5 items); the dead band of its tone lives with the beep pattern
/// (Sound)
pub const SPEED_TO_FLY: Menu = Menu {
    name: "Speed to Fly",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::TcSpeedToFly),
            next_menu_idx: SPEED_TO_FLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::VarioModeControl),
            next_menu_idx: SPEED_TO_FLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::StfSwitchMode),
            next_menu_idx: SPEED_TO_FLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::StfSwitchSpeed),
            next_menu_idx: SPEED_TO_FLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SpeedToFlyPinConfig),
            next_menu_idx: SPEED_TO_FLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::TcCircleHysteresis),
            next_menu_idx: SPEED_TO_FLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// not reachable any more (its items are in SPEED_TO_FLY), kept so the
/// indices of MENU_LIST stay
pub const STF_MODE_SWITCH: Menu = Menu {
    name: "Mode Switch",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::VarioModeControl),
            next_menu_idx: STF_MODE_SWITCH_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::StfSwitchMode),
            next_menu_idx: STF_MODE_SWITCH_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::StfSwitchSpeed),
            next_menu_idx: STF_MODE_SWITCH_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SpeedToFlyPinConfig),
            next_menu_idx: STF_MODE_SWITCH_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::TcCircleHysteresis),
            next_menu_idx: STF_MODE_SWITCH_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SPEED_TO_FLY_IDX,
        },
    ],
};

pub const SOUND: Menu = Menu {
    name: "Sound",
    level: 2,
    items: &[
        // 15.11.2026: three ready made sounds on top of the single values
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SoundPreset),
            next_menu_idx: SOUND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CenterFrequency),
            next_menu_idx: SOUND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Waveform),
            next_menu_idx: SOUND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::AudioDamping),
            next_menu_idx: SOUND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SoundAttack),
            next_menu_idx: SOUND_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SOUND_FREQUENCY_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SOUND_STEFLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SOUND_LOUDNESS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// Shape of the tone curve: two anchor frequencies plus the full scale value
pub const SOUND_FREQUENCY: Menu = Menu {
    name: "Frequency",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SoundFreqUpper),
            next_menu_idx: SOUND_FREQUENCY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SoundFreqLower),
            next_menu_idx: SOUND_FREQUENCY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SoundFullScale),
            next_menu_idx: SOUND_FREQUENCY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SOUND_IDX,
        },
    ],
};

/// Beep pattern of the vario tone, plus the dead band of the speed to fly double beep
pub const SOUND_STEFLY: Menu = Menu {
    name: "Beep Pattern",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SinkTone),
            next_menu_idx: SOUND_STEFLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SinkToneFrom),
            next_menu_idx: SOUND_STEFLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::TicksUntil),
            next_menu_idx: SOUND_STEFLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::BeepTempo),
            next_menu_idx: SOUND_STEFLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::StfUpperLimit),
            next_menu_idx: SOUND_STEFLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::StfLowerLimit),
            next_menu_idx: SOUND_STEFLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SOUND_IDX,
        },
    ],
};

/// Level settings
pub const SOUND_LOUDNESS: Menu = Menu {
    name: "Loudness",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::LoudnessComp),
            next_menu_idx: SOUND_LOUDNESS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SpeedVolume),
            next_menu_idx: SOUND_LOUDNESS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SpeedVolumeRef),
            next_menu_idx: SOUND_LOUDNESS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SOUND_IDX,
        },
    ],
};

/// Basic settings of the display. The old per view contents (Circling / Straight / Ring Design) are gone: the pages hold them.
pub const DISPLAY: Menu = Menu {
    name: "Display",
    level: 2,
    items: &[
        // 15.11.2026: from the flight menu (brightness, page layout) + page styles
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Brightness),
            next_menu_idx: DISPLAY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PageStyle),
            next_menu_idx: DISPLAY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PageLayout),
            next_menu_idx: DISPLAY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Theme),
            next_menu_idx: DISPLAY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Rotation),
            next_menu_idx: DISPLAY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::GliderSymbol),
            next_menu_idx: DISPLAY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::BigEncoder),
            next_menu_idx: DISPLAY_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: DISPLAY_WIND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// Wind arrows: energy arrow gain (frontend) and the wind filter times of the sensor box
pub const DISPLAY_WIND: Menu = Menu {
    name: "Wind",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::EnergyArrowMult),
            next_menu_idx: DISPLAY_WIND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::WindTc),
            next_menu_idx: DISPLAY_WIND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::MeanWindTc),
            next_menu_idx: DISPLAY_WIND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: DISPLAY_IDX,
        },
    ],
};

pub const UNITS: Menu = Menu {
    name: "Units",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::UnitHorizontalSpeed),
            next_menu_idx: UNITS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::UnitVerticalSpeed),
            next_menu_idx: UNITS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::UnitHeight),
            next_menu_idx: UNITS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// This glider: polar, flaps, and everything wired to the frontend (drain valve, gear and airbrakes).
/// The flash output moved to FLARM > SteFly Flasher.
pub const STEFLY_AIRCRAFT: Menu = Menu {
    name: "Aircraft",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: POLAR_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: FLAPS_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: DRAIN_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: LANDING_GEAR_ALARM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

pub const POLAR_SETTINGS: Menu = Menu {
    name: "Polar Settings",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Glider),
            next_menu_idx: POLAR_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::EmptyMass),
            next_menu_idx: POLAR_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::MaxBallast),
            next_menu_idx: POLAR_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::ReferenceWeight),
            next_menu_idx: POLAR_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PolarValueV1),
            next_menu_idx: POLAR_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PolarValueV2),
            next_menu_idx: POLAR_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PolarValueV3),
            next_menu_idx: POLAR_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PolarValueSi1),
            next_menu_idx: POLAR_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PolarValueSi2),
            next_menu_idx: POLAR_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PolarValueSi3),
            next_menu_idx: POLAR_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: STEFLY_AIRCRAFT_IDX,
        },
    ],
};

pub const DRAIN_SETTINGS: Menu = Menu {
    name: "Drain Control",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::DrainPinConfig),
            next_menu_idx: DRAIN_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlowEmpty),
            next_menu_idx: DRAIN_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlowSlope),
            next_menu_idx: DRAIN_SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: STEFLY_AIRCRAFT_IDX,
        },
    ],
};

pub const LANDING_GEAR_ALARM: Menu = Menu {
    name: "Gear Alarm",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::AlarmVolume),
            next_menu_idx: LANDING_GEAR_ALARM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::GearAlarmModeConfig),
            next_menu_idx: LANDING_GEAR_ALARM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::GearPinConfig),
            next_menu_idx: LANDING_GEAR_ALARM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::AirbrakesPinConfig),
            next_menu_idx: LANDING_GEAR_ALARM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: STEFLY_AIRCRAFT_IDX,
        },
    ],
};

/// SteFly canopy flasher (plan_flarm_blitzer.md), signal 1:1 as the Control Box
pub const STEFLY_FLASHER: Menu = Menu {
    name: "SteFly Flasher",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlasherTypeSel),
            next_menu_idx: STEFLY_FLASHER_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlasherActiveSel),
            next_menu_idx: STEFLY_FLASHER_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlasherOnSpeed),
            next_menu_idx: STEFLY_FLASHER_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlashRateSel),
            next_menu_idx: STEFLY_FLASHER_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlasherTest),
            next_menu_idx: STEFLY_FLASHER_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: FLARM_IDX,
        },
    ],
};

/// Sensor box: installation and one time calibration, grouped by topic
pub const SENSORBOX: Menu = Menu {
    name: "Sensorbox",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SENSORBOX_ORIENTATION_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SENSORBOX_PITOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SENSORBOX_GNSS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::BlockHorizon),
            next_menu_idx: SENSORBOX_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SENSORBOX_SERVICE_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// Orientation of the box in the glider, in working order: the three measurements, the calculation, the fine tune in flight, and the resulting tilt values to check or correct
pub const SENSORBOX_ORIENTATION: Menu = Menu {
    name: "Orientation",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdMeas1),
            next_menu_idx: SENSORBOX_ORIENTATION_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdMeas2),
            next_menu_idx: SENSORBOX_ORIENTATION_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdMeas3),
            next_menu_idx: SENSORBOX_ORIENTATION_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdCalcOrientation),
            next_menu_idx: SENSORBOX_ORIENTATION_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdFineTuneOrientation),
            next_menu_idx: SENSORBOX_ORIENTATION_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SensTiltRoll),
            next_menu_idx: SENSORBOX_ORIENTATION_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SensTiltPitch),
            next_menu_idx: SENSORBOX_ORIENTATION_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SensTiltYaw),
            next_menu_idx: SENSORBOX_ORIENTATION_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SENSORBOX_IDX,
        },
    ],
};

pub const SENSORBOX_PITOT: Menu = Menu {
    name: "Pitot & Static",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PitotOffset),
            next_menu_idx: SENSORBOX_PITOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PitotOffsetMeasure),
            next_menu_idx: SENSORBOX_PITOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PitotSpan),
            next_menu_idx: SENSORBOX_PITOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::QnhDelta),
            next_menu_idx: SENSORBOX_PITOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SENSORBOX_IDX,
        },
    ],
};

pub const SENSORBOX_GNSS: Menu = Menu {
    name: "GNSS",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::GnssConfig),
            next_menu_idx: SENSORBOX_GNSS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::AntBaselen),
            next_menu_idx: SENSORBOX_GNSS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::AntSlaveDown),
            next_menu_idx: SENSORBOX_GNSS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::AntSlaveRight),
            next_menu_idx: SENSORBOX_GNSS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SENSORBOX_IDX,
        },
    ],
};

/// Reset of the box and the developer test function
pub const SENSORBOX_SERVICE: Menu = Menu {
    name: "Service",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdResetSensorbox),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdTestFunctionNumber),
            next_menu_idx: SENSORBOX_SERVICE_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdTestFunction),
            next_menu_idx: SENSORBOX_SERVICE_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SENSORBOX_IDX,
        },
    ],
};

/// Device info page, NMEA port, battery thresholds, demo, usage mode (normal
/// / club, code protected), firmware downgrade and the resets. Battery and
/// Firmware were sub menus with two resp. one item (flattened 28.10.2026).
pub const STEFLY_SYSTEM: Menu = Menu {
    name: "System",
    level: 2,
    items: &[
        // back again (15.11.2026)
        MenuItem {
            content: MenuItemContent::EditItem(Editable::UserProfile),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SetupWizard),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::DeviceInfo),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: NMEA_PORT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::BatteryGood),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::BatteryLow),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::DemoSel),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::UsageMode),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::UsageCode),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FwDowngrade),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SYSTEM_RESET_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// not reachable any more (items in STEFLY_SYSTEM), kept for the indices
pub const SYSTEM_BATTERY: Menu = Menu {
    name: "Battery",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::BatteryGood),
            next_menu_idx: SYSTEM_BATTERY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::BatteryLow),
            next_menu_idx: SYSTEM_BATTERY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
    ],
};

/// not reachable any more (item in STEFLY_SYSTEM), kept for the indices
pub const SYSTEM_FIRMWARE: Menu = Menu {
    name: "Firmware",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FwDowngrade),
            next_menu_idx: SYSTEM_FIRMWARE_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
    ],
};

/// Serial NMEA port of the display: XCSoar & Co (Larus sentences, both
/// directions) or LXNAV main unit (receive MC / ballast / bugs / mode),
/// and the baud rate
pub const NMEA_PORT: Menu = Menu {
    name: "NMEA Port",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::NmeaProtocolSel),
            next_menu_idx: NMEA_PORT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::NmeaBaudSel),
            next_menu_idx: NMEA_PORT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
    ],
};

pub const SYSTEM_RESET: Menu = Menu {
    name: "Reset",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::ResetConfig),
            next_menu_idx: SYSTEM_RESET_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FactoryReset),
            next_menu_idx: SYSTEM_RESET_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: STEFLY_SYSTEM_IDX,
        },
    ],
};

/// FLARM at the second serial port (RJ45, Y cable): data status, baud rate,
/// canopy flasher and the collision warning pages
pub const FLARM: Menu = Menu {
    name: "FLARM",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlarmStatus),
            next_menu_idx: FLARM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlarmBaud),
            next_menu_idx: FLARM_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: STEFLY_FLASHER_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: FLARM_WARNINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// FLARM warning pages (plan_flarm_warnung.md); no warning tones yet
pub const FLARM_WARNINGS: Menu = Menu {
    name: "Warnings",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::WarnViewSel),
            next_menu_idx: FLARM_WARNINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::WarnLevelSel),
            next_menu_idx: FLARM_WARNINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::HeadOnSel),
            next_menu_idx: FLARM_WARNINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: FLARM_IDX,
        },
    ],
};

/// Flaps: sensor, stage table, circling stage (plan_klappen_menue.md)
pub const FLAPS: Menu = Menu {
    name: "Flaps",
    level: 3,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlapSensorSel),
            next_menu_idx: FLAPS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlapSensorValue),
            next_menu_idx: FLAPS_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: FLAP_STAGES_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlapCircling),
            next_menu_idx: FLAPS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: STEFLY_AIRCRAFT_IDX,
        },
    ],
};

/// Table of the flap stages, slow on top. Only the rows of existing stages
/// are shown (model::menu::item_visible).
pub const FLAP_STAGES: Menu = Menu {
    name: "Flap Stages",
    level: 4,
    items: &[
        MenuItem {
            content: MenuItemContent::FlapItem(0),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::FlapItem(1),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::FlapItem(2),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::FlapItem(3),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::FlapItem(4),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::FlapItem(5),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::FlapItem(6),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::FlapItem(7),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::FlapItem(8),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::FlapItem(9),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::FlapItem(10),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::FlapItem(11),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlapAdd),
            next_menu_idx: FLAP_STAGES_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: FLAPS_IDX,
        },
    ],
};

/// one flap stage (config.flap_edit), the header shows its name
pub const FLAP_EDIT: Menu = Menu {
    name: "Flap Stage",
    level: 5,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlapName),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlapUpTo),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlapFrom),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlapPosition),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlapSetPos),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::FlapDelete),
            next_menu_idx: FLAP_EDIT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: FLAP_STAGES_IDX,
        },
    ],
};
