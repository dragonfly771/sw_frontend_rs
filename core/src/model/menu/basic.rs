//! Setup menu of the Basic mode (Stefan 15.11.2026): what a pilot needs who
//! never opens the manual - about 25 settings instead of more than 100.
//! Presets replace the single values (vario response, sound preset, page
//! style, cruise switch); everything else is in the Expert menu (full.rs).
//! Indices 0..3 are shared with the other lists (mod.rs).
use crate::{
    model::menu::{
        Menu, MenuItem, MenuItemContent, DISPLAY_IDX, FLIGHT_MENU, ROOT, ROOT_IDX, SETTINGS_IDX,
    },
    Editable,
};

pub const SOUND_IDX: usize = 4;
pub const UNITS_IDX: usize = 5;
pub const AIRCRAFT_IDX: usize = 6;
pub const SENSOR_IDX: usize = 7;
pub const SYSTEM_IDX: usize = 8;

pub const MENU_LIST: &[Menu] = &[
    ROOT,
    FLIGHT_MENU,
    SETTINGS, // SETTINGS_IDX
    DISPLAY,  // DISPLAY_IDX
    SOUND,
    UNITS,
    AIRCRAFT,
    SENSOR,
    SYSTEM,
];

/// Top level: the two questions of the vario (how lively, when to switch to cruise), then the sub menus. Switch Speed and StF Pin Config show only with the cruise switch that needs them (item_visible).
pub const SETTINGS: Menu = Menu {
    name: "Setup",
    level: 1,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::VarioResponse),
            next_menu_idx: SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CruiseSwitch),
            next_menu_idx: SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::StfSwitchSpeed),
            next_menu_idx: SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SpeedToFlyPinConfig),
            next_menu_idx: SETTINGS_IDX,
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
            next_menu_idx: AIRCRAFT_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SENSOR_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: ROOT_IDX,
        },
    ],
};

/// Sound: one preset and the sink tone; the volume is on the inner knob
pub const SOUND: Menu = Menu {
    name: "Sound",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SoundPreset),
            next_menu_idx: SOUND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SinkTone),
            next_menu_idx: SOUND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// Display: brightness, a ready made page, colours, mounting rotation
pub const DISPLAY: Menu = Menu {
    name: "Display",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Brightness),
            next_menu_idx: DISPLAY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PageStyle),
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
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// Units
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

/// Aircraft: the polar and the two masses that differ from glider to glider
pub const AIRCRAFT: Menu = Menu {
    name: "Aircraft",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Glider),
            next_menu_idx: AIRCRAFT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::EmptyMass),
            next_menu_idx: AIRCRAFT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::MaxBallast),
            next_menu_idx: AIRCRAFT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// Calibration of the sensor box mounting, in the order of the work; each row shows whether it is done
pub const SENSOR: Menu = Menu {
    name: "Sensor Setup",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdMeas1),
            next_menu_idx: SENSOR_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdMeas2),
            next_menu_idx: SENSOR_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdMeas3),
            next_menu_idx: SENSOR_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdCalcOrientation),
            next_menu_idx: SENSOR_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::CmdFineTuneOrientation),
            next_menu_idx: SENSOR_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

/// System: profile, the wizard again, device info and the way to the Expert menu
pub const SYSTEM: Menu = Menu {
    name: "System",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::UserProfile),
            next_menu_idx: SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::SetupWizard),
            next_menu_idx: SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::DeviceInfo),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::UsageMode),
            next_menu_idx: SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};
