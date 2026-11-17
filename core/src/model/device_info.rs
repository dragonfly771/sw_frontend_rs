use crate::view::viewable::device_lineview::DeviceLineView;

/// sections follow each other directly, the headers carry a line (sketch
/// skizze_menue_info.png "Vorschlag: Geraeteinfo")
pub const DEVICE_INFO_CONTENT: &[DeviceLineView] = &[
    // Vario Display
    DeviceLineView::VarioDisplay,
    DeviceLineView::DisplayVersion,
    DeviceLineView::SupplyVoltage,
    // DeviceLineView::IluminationVoltage,
    // DeviceLineView::TempPcb,
    DeviceLineView::InPinBreaks,
    DeviceLineView::InPinDrain,
    DeviceLineView::InPinGear,
    DeviceLineView::InPinSpeedToFly,
    // FLARM port and SteFly canopy flasher (on the former flash output);
    // only with the crate feature `flarm` (parked, model/flarm_cfg.rs)
    #[cfg(feature = "flarm")]
    DeviceLineView::Flarm,
    #[cfg(feature = "flarm")]
    DeviceLineView::FlarmData,
    #[cfg(feature = "flarm")]
    DeviceLineView::FlarmBaud,
    #[cfg(feature = "flarm")]
    DeviceLineView::FlarmGps,
    #[cfg(feature = "flarm")]
    DeviceLineView::FlarmAlarm,
    #[cfg(feature = "flarm")]
    DeviceLineView::Flasher,
    // Sensor Box
    DeviceLineView::SensorBox,
    DeviceLineView::SensorboxVersion,
    DeviceLineView::GnssAccuracityOk,
    DeviceLineView::MagneticDisturbanceOk,
    DeviceLineView::AhrsPitch,
    DeviceLineView::AhrsRoll,
    DeviceLineView::AhrsYaw,
    DeviceLineView::Ias,
    DeviceLineView::Tas,
    DeviceLineView::Density,
    DeviceLineView::Gforce,
    DeviceLineView::GpsAltitude,
    DeviceLineView::GpsGroundSpeed,
    DeviceLineView::GpsTrack,
    DeviceLineView::GpsSats,
    DeviceLineView::GpsState,
    DeviceLineView::NickAngle,
    DeviceLineView::Pressure,
    DeviceLineView::SlipAngle,
    DeviceLineView::TurnRate,
    DeviceLineView::HorizonAvailable,
];
