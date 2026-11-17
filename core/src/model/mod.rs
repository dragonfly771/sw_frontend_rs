mod calculated;
mod config;
mod control;
pub mod demo;
mod device;
mod device_const;
pub mod device_info;
pub mod editable;
pub mod flaps;
pub mod flarm_cfg;
pub mod flarm_show;
pub mod menu;
pub mod page;
pub mod page_layout;
mod sensor;

use crate::flight_physics::{polar_store, GliderData};
use calculated::Calculated;
pub use calculated::PotVerdict;
pub use config::{
    AvgMarker, Config, DisplayActive, DisplayTheme, GustFilter, NmeaProtocol, OverlayActive,
    TypeOfInfo, UnitHeight, UnitHorizontalSpeed, UnitVerticalSpeed, VarioNeedle, BRIGHTNESS_MIN,
    NMEA_BAUDS, NMEA_BAUD_DEFAULT_IDX, NMEA_PROTOCOL_LARUS, NMEA_PROTOCOL_LXNAV,
    SINK_MODE_CONTINUOUS, SINK_MODE_DOUBLE_BEEP, SINK_MODE_OFF, STF_SWITCH_MODE_BALLAST,
    STF_SWITCH_MODE_FIXED, STF_SWITCH_MODE_POLAR,
};
pub use control::{
    CirclingDirection, Control, DataSource, EditMode, FlyMode, PitotCal, PitotCalError,
    PitotCalState, SystemState, TcrMode, VarioMode, VarioModeControl,
};
use device::Device;
pub use device_const::{
    DeviceConst, DisplaySizes, HorizonSizes, Images, Misc, Palette, Sizes, VarioSizes,
};
pub use editable::Editable;
pub use page::{PageCore, PageFrame, MAX_PAGES};
pub use sensor::{GpsState, Sensor};

/// Data model for the entire device
///
/// In the CoreModel all variables of the display are kept in a structure, which is used by
/// different modules. The CoreModel is filled by the CoreController, which holds the different
/// channels to the data sources like Larus, controls, sensors, inputs and time. The View
/// modules display the data contents. The LCD display and the sound system are the most
/// important ones.
#[derive(Clone, Copy)]
pub struct CoreModel {
    pub calculated: Calculated,
    pub config: Config,
    pub control: Control,
    pub device: Device,
    pub device_const: &'static DeviceConst,
    pub glider_data: GliderData,
    pub sensor: Sensor,
}

impl CoreModel {
    pub fn new(device_const: &'static DeviceConst, uuid: u32) -> Self {
        let calculated = Calculated::default();
        let config = Config::default(&device_const.dark_theme, uuid);
        let control = Control::default();
        let device = Device::default();
        let glider_data = GliderData::from_basic_glider_data(*polar_store::from_raw_idx(
            config.glider_idx as usize,
        ));

        let sensor = Sensor::default();
        CoreModel {
            calculated,
            config,
            control,
            device,
            device_const,
            glider_data,
            sensor,
        }
    }

    pub fn reset(&mut self) {
        self.calculated = Calculated::default();
        self.config = Config::default(&self.device_const.dark_theme, self.config.uuid);
        // the time base runs on (FLARM, flasher and demo keep their timestamps)
        let now_ms = self.control.now_ms;
        self.control = Control::default();
        self.control.now_ms = now_ms;
        self.device = Device::default();
        self.glider_data = GliderData::from_basic_glider_data(*polar_store::from_raw_idx(
            self.config.glider_idx as usize,
        ));
        self.sensor = Sensor::default();
    }
}
