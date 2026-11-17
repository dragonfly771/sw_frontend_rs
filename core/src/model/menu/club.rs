//! Setup menu of the club mode: the same structure as the full menu, reduced
//! to what a club pilot may change - no aircraft, sensor box, page or
//! reset entries. Indices 0..4 are shared with the full list (mod.rs).
use crate::{
    model::menu::{
        Menu, MenuItem, MenuItemContent, DISPLAY_IDX, FLIGHT_MENU, ROOT, ROOT_IDX, SETTINGS_IDX,
    },
    Editable,
};

pub const VARIO_IDX: usize = 5;
pub const SPEED_TO_FLY_IDX: usize = 6;
pub const SOUND_IDX: usize = 7;
pub const UNITS_IDX: usize = 8;
pub const SYSTEM_IDX: usize = 9;

pub const MENU_LIST: &[Menu] = &[
    ROOT,
    FLIGHT_MENU,
    SETTINGS, // SETTINGS_IDX
    DISPLAY,  // DISPLAY_IDX
    ROOT,     // free slot
    VARIO,
    SPEED_TO_FLY,
    SOUND,
    UNITS,
    SYSTEM,
];

pub const SETTINGS: Menu = Menu {
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
            next_menu_idx: SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: ROOT_IDX,
        },
    ],
};

pub const VARIO: Menu = Menu {
    name: "Vario",
    level: 2,
    items: &[
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
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

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
            content: MenuItemContent::EditItem(Editable::TcCircleHysteresis),
            next_menu_idx: SPEED_TO_FLY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

pub const SOUND: Menu = Menu {
    name: "Sound",
    level: 2,
    items: &[
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
            content: MenuItemContent::EditItem(Editable::StfUpperLimit),
            next_menu_idx: SOUND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::StfLowerLimit),
            next_menu_idx: SOUND_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};

pub const DISPLAY: Menu = Menu {
    name: "Display",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Brightness),
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
            content: MenuItemContent::EditItem(Editable::EnergyArrowMult),
            next_menu_idx: DISPLAY_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
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

pub const SYSTEM: Menu = Menu {
    name: "System",
    level: 2,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::UserProfile),
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
            content: MenuItemContent::EditItem(Editable::UsageCode),
            next_menu_idx: SYSTEM_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: SETTINGS_IDX,
        },
    ],
};
