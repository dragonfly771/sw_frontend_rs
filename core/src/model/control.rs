use crate::{
    controller::{CanActive, DeviceInfoControl, Editor},
    system_of_units::{FloatToLength, FloatToSpeed, Length, Speed},
    utils::DeviceEvent,
    MenuControl, PinState, Rotation,
};

/// Flymode display variants
///
/// Flymode::Climbing is displayed while circling upwind. In addition to vario signal and wind,
/// the average climb is also displayed.
///
/// In Flymode::Gliding the mean climb is hidden and the speed command is displayed
/// graphically and as a number. Flymode::Transition is treatet like Flymode::Gliding
#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
pub enum FlyMode {
    Circling,
    StraightFlight,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CirclingDirection {
    Left,
    Right,
}

impl CirclingDirection {
    pub fn from_turn_rate(turn_rate_rad_s: f32, current: Self) -> Self {
        const MIN_TURN_RATE_RAD_S: f32 = 0.03;

        if turn_rate_rad_s > MIN_TURN_RATE_RAD_S {
            Self::Left
        } else if turn_rate_rad_s < -MIN_TURN_RATE_RAD_S {
            Self::Right
        } else {
            current
        }
    }
}

/// This enum is relevant for the View component. During Vario mode, information needed to
/// optimize climbing in thermals is displayed. SppedToFly, on the other hand, is intended
/// for optimal pre-flight.
#[repr(u8)]
#[derive(PartialEq, Clone, Copy)]
pub enum VarioMode {
    Vario,
    SpeedToFly,
}

impl From<u8> for VarioMode {
    fn from(value: u8) -> Self {
        if value == 1 {
            VarioMode::SpeedToFly
        } else {
            VarioMode::Vario
        }
    }
}

impl core::ops::Not for VarioMode {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            VarioMode::Vario => VarioMode::SpeedToFly,
            VarioMode::SpeedToFly => VarioMode::Vario,
        }
    }
}

/// This determines how to switch between the two modes of VarioMode Vario and SpeedToFly:
/// automatic, manual Vario or manual SpeedToFly.
#[repr(u8)]
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum VarioModeControl {
    Auto,
    InputPin,
    Nmea,
    Can,
}

pub const VARIO_MODE_CONTROL_AUTO: &str = "Auto";
pub const VARIO_MODE_CONTROL_PIN: &str = "Input Pin";
pub const VARIO_MODE_CONTROL_NMEA: &str = "NMEA";
pub const VARIO_MODE_CONTROL_CAN: &str = "CAN";

impl From<u8> for VarioModeControl {
    fn from(value: u8) -> Self {
        match value {
            1 => VarioModeControl::InputPin,
            2 => VarioModeControl::Nmea,
            3 => VarioModeControl::Can,
            _ => VarioModeControl::Auto,
        }
    }
}

impl From<&str> for VarioModeControl {
    fn from(value: &str) -> Self {
        match value {
            VARIO_MODE_CONTROL_NMEA => VarioModeControl::Nmea,
            VARIO_MODE_CONTROL_PIN => VarioModeControl::InputPin,
            VARIO_MODE_CONTROL_CAN => VarioModeControl::Can,
            _ => VarioModeControl::Auto,
        }
    }
}

impl VarioModeControl {
    pub fn as_str(&self) -> &'static str {
        match self {
            VarioModeControl::Auto => VARIO_MODE_CONTROL_AUTO,
            VarioModeControl::InputPin => VARIO_MODE_CONTROL_PIN,
            VarioModeControl::Nmea => VARIO_MODE_CONTROL_NMEA,
            VarioModeControl::Can => VARIO_MODE_CONTROL_CAN,
        }
    }
}

#[repr(u8)]
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum DataSource {
    Frontend,
    Sensorbox,
}

pub const DATA_SOURCE_FRONTEND: &str = "Frontend";
pub const DATA_SOURCE_SENSORBOX: &str = "Sensorbox";

impl From<u8> for DataSource {
    fn from(value: u8) -> Self {
        match value {
            1 => DataSource::Sensorbox,
            _ => DataSource::Frontend,
        }
    }
}

impl From<&str> for DataSource {
    fn from(value: &str) -> Self {
        match value {
            DATA_SOURCE_SENSORBOX => DataSource::Sensorbox,
            _ => DataSource::Frontend,
        }
    }
}

impl DataSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            DataSource::Frontend => DATA_SOURCE_FRONTEND,
            DataSource::Sensorbox => DATA_SOURCE_SENSORBOX,
        }
    }
}

/// Enum mode controls whether the background should be visible or not when editing a data
/// point.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
pub enum EditMode {
    Off,
    Window,
    Fullscreen,
    CircleArea,
}

/// Enum for calculation of thermal climb rate
#[derive(Clone, Copy, PartialEq)]
pub enum TcrMode {
    StraightFlight,
    Transition,
    Climbing,
}

/// Enum for Larus System State
#[derive(Clone, Copy, PartialEq)]
pub enum SystemState {
    NoCom,
    CanOk,
    CanAndGpsOk,
}

/// Struct to store pin states
#[derive(Clone, Copy)]
pub struct HwPins {
    pub in_breakes: PinState,
    pub in_drain: PinState,
    pub in_gear: PinState,
    pub in_speed_to_fly: PinState,
    pub out_flash: PinState,
}

impl Default for HwPins {
    fn default() -> Self {
        // Input Pins are set by hardware, simulator sets them to High
        // Output pins are set during initialization to low
        HwPins {
            in_breakes: PinState::High,
            in_drain: PinState::High,
            in_gear: PinState::High,
            in_speed_to_fly: PinState::High,
            out_flash: PinState::Low,
        }
    }
}

/// Metastructure for different control variables
#[derive(Clone, Copy)]
pub struct Control {
    /// Count ticks the firmware is alive (100ms)
    pub alive_ticks: u32,
    /// Count ticks average climb rates slave mode is active (1s)
    pub avg_climb_slave_ticks: u16,
    /// State of the Larus system
    pub system_state: SystemState,
    /// Bit pattern of all can bus devices
    pub can_devices: u32,
    /// FlyMode::Circling, FlyMode::StraightFlight
    pub fly_mode: FlyMode,
    /// Last detected circling direction
    pub circling_direction: CirclingDirection,
    /// VarioMode::Vario, VarioMode::SpeedToFly
    pub vario_mode: VarioMode,
    /// VarioMode::Vario, VarioMode::SpeedToFly, VarioMode::Auto
    pub vario_mode_control: VarioModeControl,
    /// Sets the switching point Vario/SpeedToFly in relation to speed of the best l/d ratio
    pub vario_mode_switch_ratio: f32,
    /// Speed limit above which SpeedToFly is activated
    pub speed_to_fly_limit: Speed,
    /// EditMode::Section, EditMode::Fullscreen
    pub pers_ticks: u32,
    /// DeviceEvent::FwAvailable, PrepareFwUpload, ...
    pub firmware_update_state: DeviceEvent,
    /// TcrMode::StraightFlight, TcrMode::Transition, TcrMode::Climbing
    pub tcr_mode: TcrMode,
    /// Measurement of time climbing
    pub tcr_1s_climb_ticks: u32,
    /// Measurement of time transient climb <-> straigt flight
    pub tcr_1s_transient_ticks: u32,
    /// Height at the beginning of the climb
    pub tcr_start: Length,
    /// General Editor
    pub editor: Editor,
    /// Menu Control
    pub menu_control: MenuControl,
    /// counts down while the page indicator is shown
    pub page_indicator: u8,
    /// Screen Rotation
    pub rotation: Rotation,
    /// Reset Config
    pub reset_config: i8,
    /// Volume for alarms
    pub alarm_volume: i8,
    /// volume panel (circling / cruise / warnings, controller/volume_panel.rs)
    pub volume_panel: crate::controller::volume_panel::VolumePanel,
    /// Source average climb rate
    pub avg_climb_rate_src: DataSource,
    /// Energy arrow multiplying factor
    pub energy_arrow_mult: f32,
    /// the menu mode is known (stored, or decided 10 s after the start)
    pub mode_checked: bool,
    /// the EEPROM brought settings: the device was in use before
    pub items_restored: bool,
    /// seconds since the start / the last reset of the model
    pub boot_seconds: u16,
    /// sensor calibration of this session: bits 0..2 = left wing, right
    /// wing, wings straight measured, bit 3 = orientation calculated,
    /// bit 4 = level flight set
    pub cal_done: u8,
    /// gear alarm: the condition holds (gear up, airbrakes open) ...
    pub gear_warning: bool,
    /// ... and the pilot has confirmed the alarm with a push (controller/gear_alarm.rs)
    pub gear_alarm_ack: bool,
    /// "NO SENSOR BOX" on the page was confirmed with a push; the red
    /// symbol in the band stays. Cleared when the box is heard again.
    pub no_box_ack: bool,
    /// setup wizard: 0 = off, else the step (1 = first)
    pub wizard_step: u8,
    /// sound preset editor: the sound that was set before a preset is
    /// listened to (model/editable/simple.rs)
    pub sound_backup: Option<crate::model::editable::simple::SoundSet>,
    /// Information about CAN Master (uses device can base address or not)
    pub is_can_master: bool,
    /// Data for Device Info Control
    pub device_info_control: DeviceInfoControl,
    /// Store pin states of input and output pins
    pub hw_pins: HwPins,
    /// counts down (100 ms) to the automatic Calc Orientation after
    /// Level Flight, 0 = nothing pending (editable/sensorbox.rs)
    pub level_flight_calc: u8,
    /// demo for the fair (model/demo.rs), never stored
    pub demo: crate::model::demo::DemoMode,
    /// state of the flight / flap demo
    pub flap_demo: crate::model::flaps::FlapDemo,
    /// real IAS from the sensor box while the demo overrides it (km/h)
    pub demo_real_ias_kmh: f32,
    /// milliseconds since start (controller tick), time base of FLARM and flasher
    pub now_ms: i64,
    /// page layout editor on the page (flight menu "Page Layout")
    pub layout: crate::model::page_layout::LayoutEdit,
    /// automatic pitot offset measurement (controller/pitot_cal.rs)
    pub pitot_cal: PitotCal,
}

/// Why an automatic pitot offset measurement did not run
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PitotCalError {
    /// offset or span of the box unknown: no answer on the CAN bus
    NoSensorbox,
    /// the airspeed says the glider is flying (or the wind is on the pitot)
    NotOnGround,
}

/// State of the automatic pitot offset measurement; the numbers are 100 ms
/// ticks. The controller drives it, the ring editor draws it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PitotCalState {
    Idle,
    /// waiting for the cached offset and span of the box (ticks left)
    Prepare(u8),
    /// the shifted offset is on its way to the box, samples not valid yet
    Settle(u8),
    /// averaging the dynamic pressure (samples taken so far)
    Measure(u8),
    /// the result waits in the save dialog
    Confirm,
    /// did not run, the reason stays on the display until the editor closes
    Failed(PitotCalError),
}

/// Automatic measurement of the pitot offset of the sensor box
///
/// The box does not put the raw pitot pressure on the CAN bus, only the
/// airspeed computed from it, and a negative corrected pressure is clamped
/// to zero. So the offset stored in the box is shifted down by `SHIFT_PA`
/// for the measurement (the corrected pressure is then safely positive),
/// the dynamic pressure is recovered from the IAS, averaged, and the raw
/// pressure follows as mean / span + shifted offset. That raw pressure at
/// rest is the new offset. Discard, Esc or a time-out restore the old value.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PitotCal {
    pub state: PitotCalState,
    /// offset stored in the box before the measurement
    pub old_offset: f32,
    pub span: f32,
    /// the box currently holds `old_offset - SHIFT_PA`
    pub shifted: bool,
    /// sum of the dynamic pressure samples / Pa
    pub sum: f32,
    /// the running mean of the raw pressure / Pa; the result once measured
    pub raw: f32,
}

impl Default for PitotCal {
    fn default() -> Self {
        PitotCal {
            state: PitotCalState::Idle,
            old_offset: 0.0,
            span: 1.0,
            shifted: false,
            sum: 0.0,
            raw: 0.0,
        }
    }
}

impl Default for Control {
    fn default() -> Self {
        Self {
            alive_ticks: 0,
            avg_climb_slave_ticks: 0,
            system_state: SystemState::NoCom,
            can_devices: CanActive::None as u32,
            fly_mode: FlyMode::StraightFlight,
            circling_direction: CirclingDirection::Right,
            vario_mode: VarioMode::Vario,
            vario_mode_control: VarioModeControl::Auto,
            vario_mode_switch_ratio: 1.10,
            speed_to_fly_limit: 105.0.km_h(),
            pers_ticks: 0,
            firmware_update_state: DeviceEvent::UploadFinished,
            tcr_mode: TcrMode::StraightFlight,
            tcr_1s_climb_ticks: 0,
            tcr_1s_transient_ticks: 0,
            tcr_start: 0.0.m(),
            editor: Editor::new(),
            menu_control: MenuControl::new(),
            page_indicator: 0,
            rotation: Rotation::Rotate0,
            reset_config: 0,
            alarm_volume: 15,
            volume_panel: crate::controller::volume_panel::VolumePanel::new(),
            avg_climb_rate_src: DataSource::Sensorbox, // the box averages over the circle (Stefan 16.11.2026)
            energy_arrow_mult: 2.0, // visible by default (Stefan 15.11.2026); 0 = off
            mode_checked: false,
            items_restored: false,
            boot_seconds: 0,
            cal_done: 0,
            gear_warning: false,
            gear_alarm_ack: false,
            no_box_ack: false,
            wizard_step: 0,
            sound_backup: None,
            is_can_master: false,
            device_info_control: DeviceInfoControl::default(),
            hw_pins: HwPins::default(),
            level_flight_calc: 0,
            pitot_cal: PitotCal::default(),
            demo: crate::model::demo::DemoMode::Off,
            flap_demo: crate::model::flaps::FlapDemo::new(),
            demo_real_ias_kmh: 0.0,
            now_ms: 0,
            layout: crate::model::page_layout::LayoutEdit::default(),
        }
    }
}
