mod controller;
mod flaps_flarm;
mod glider_data;
/// Elements that can be changed by the user
///
/// Editables are always saved in the model or controller and can be changed by the user. These
/// can be parameters, display selection, time constants or other data. With the help of this
/// module, the implemented editor is able to display and change such data, save it and, if
/// necessary, output it at the NMEA and CAN interfaces.
///
/// New elements are added with the following steps:
///   - First, the persistence layer is extended (src/controller/persist.rs)
///     - Extend PersistenceId
///     - Extend restore_item()
///   - Then the enum Editable is extended by the new element (see below)
///   - Create a empty struct with same name as enum variant and implement trait EditableFuncs
///   - Add reference to fn Editable::this()
///   - Add the new editable to the menu structure (src/model/menu)
mod model;
mod page;
mod sensorbox;
pub mod simple;

use controller::*;
use flaps_flarm::*;
use glider_data::*;
use model::*;
use page::*;
use sensorbox::*;
pub use sensorbox::{REMOTE_RESTART, REMOTE_SAVE};
use simple::*;

use crate::{utils::TString, CoreController, CoreModel, Date};
use tfmt::Convert;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Editable {
    // model
    AlarmVolume,
    AvgClimbRateSrc,
    BatteryGood,
    BatteryLow,
    CenterFrequency,
    CenterViewCircling,
    CenterViewStraight,
    Display,
    Glider,
    GliderSymbol,
    Info1Vario,
    Info2Vario,
    Info3Vario,
    Info1Stf,
    Info2Stf,
    Info3Stf,
    InfoR1,
    InfoR2,
    InfoR3,
    PotK,
    PotBar,
    PotAdvice,
    NeedleMode,
    GustFilterMode,
    GustCircling,
    AvgMarkerMode,
    McCready,
    SoundSpreading,
    StfUpperLimit,
    StfSwitchMode,
    StfSwitchSpeed,
    FwDowngrade,
    Brightness,
    Qnh,
    BigEncoder,

    StfLowerLimit,
    TcCircleHysteresis,
    TcClimbRate,
    TcSpeedToFly,
    Theme,
    SoundFreqUpper,
    SoundFreqLower,
    SoundFullScale,
    SinkTone,
    SinkToneFrom,
    TicksUntil,
    AudioDamping,
    SoundAttack,
    LoudnessComp,
    SpeedVolume,
    SpeedVolumeRef,
    BeepTempo,
    VarioUpperLimit,
    VarioLowerLimit,
    UnitHeight,
    UnitHorizontalSpeed,
    UnitVerticalSpeed,
    UsageCode,
    UsageMode,
    Volume,
    Waveform,

    // controller
    DrainPinConfig,
    EnergyArrowMult,
    FactoryReset,
    FlashControl,
    FlashTest,
    FlowEmpty,
    FlowSlope,
    GearPinConfig,
    AirbrakesPinConfig,
    GearAlarmModeConfig,
    ResetConfig,
    Rotation,
    SpeedToFlyPinConfig,
    UserProfile,
    VarioModeControl,
    /// opens the device info page (firmware versions, sensor box state)
    DeviceInfo,
    /// flight menu: page layout editor on the page itself
    PageLayout,

    // glider_data
    Bugs,
    PilotWeight,
    WaterBallast,
    EmptyMass,
    MaxBallast,
    ReferenceWeight,
    PolarValueV1,
    PolarValueV2,
    PolarValueV3,
    PolarValueSi1,
    PolarValueSi2,
    PolarValueSi3,

    // sensorbox
    SensTiltRoll,
    SensTiltPitch,
    SensTiltYaw,
    PitotOffset,
    PitotOffsetMeasure,
    PitotSpan,
    QnhDelta,
    VarioTc,
    VarioIntTc,
    WindTc,
    MeanWindTc,
    GnssConfig,
    AntBaselen,
    AntSlaveDown,
    AntSlaveRight,
    BlockHorizon,
    CmdMeas1,
    CmdMeas2,
    CmdMeas3,
    CmdCalcOrientation,
    CmdFineTuneOrientation,
    CmdResetSensorbox,
    CmdTestFunction,
    CmdTestFunctionNumber,
    /// dialogs of the sensor box settings: save old -> new?, restart the box?
    RemoteSave,
    RemoteRestart,

    // flaps, FLARM, canopy flasher, demo (flaps_flarm.rs)
    FlarmStatus,
    FlarmBaud,
    /// serial NMEA port (flaps_flarm.rs)
    NmeaProtocolSel,
    NmeaBaudSel,
    FlasherTypeSel,
    FlasherActiveSel,
    FlashRateSel,
    FlasherOnSpeed,
    FlasherTest,
    WarnViewSel,
    WarnLevelSel,
    HeadOnSel,
    DemoSel,
    FlapSensorSel,
    FlapSensorValue,
    FlapCircling,
    FlapAdd,
    FlapName,
    FlapUpTo,
    FlapFrom,
    FlapPosition,
    FlapSetPos,
    FlapDelete,

    // simple operation (simple.rs)
    VarioResponse,
    CruiseSwitch,
    SoundPreset,
    PageStyle,
    SetupWizard,
    // general
    None,
    Return,
}

#[derive(Clone, Copy)]
pub enum Content {
    Command(TString<16>),
    Date(Option<Date>),
    Enum(TString<16>),
    F32(Option<f32>),
    List(i32),
    String(TString<12>),
}

#[derive(Clone, Copy)]
pub struct F32Params {
    pub min: f32,
    pub max: f32,
    pub small_inc: f32,
    pub big_inc: f32,
    pub dec_places: u8,
    pub unit: &'static str,
}

#[derive(Clone, Copy)]
pub struct DateParams {
    pub min: Date,
    pub max: Date,
    pub small_inc_plus: i16,
    pub small_inc_minus: i16,
    pub big_inc_plus: i16,
    pub big_inc_minus: i16,
    pub is_active: bool,
}

pub const MAX_ENUM_VARIANTS: usize = 5;

#[derive(Clone, Copy)]
pub struct EnumParams {
    pub variants: [&'static str; MAX_ENUM_VARIANTS],
}

#[derive(Clone, Copy)]
pub struct StringParams {
    pub content: TString<16>,
}

#[derive(Clone, Copy)]
pub struct ListParams {
    pub max: i32,
}

#[derive(Clone, Copy)]
pub struct CmdParams {
    pub content: TString<16>,
}

#[derive(Clone, Copy)]
pub enum Params {
    Cmd(CmdParams),
    Date(DateParams),
    Enum(EnumParams),
    F32(F32Params),
    List(ListParams),
    String(StringParams),
}

struct EditableFptrs {
    name: fn() -> &'static str,
    content: fn(&mut CoreModel, &mut CoreController) -> Content,
    content_as_str: fn(&mut Convert<20>, i32),
    params: fn(&CoreModel) -> Params,
    set_content: fn(&mut CoreModel, &mut CoreController, Content),
}

trait EditableFuncs {
    fn name() -> &'static str {
        ""
    }

    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::String(TString::<12>::from_str(""))
    }

    fn content_as_str(_convert: &mut Convert<20>, _idx: i32) {}

    fn params(_cm: &CoreModel) -> Params {
        Params::String(StringParams {
            content: TString::<16>::from_str(""),
        })
    }

    fn set_content(_cm: &mut CoreModel, _cc: &mut CoreController, _content: Content) {}

    fn this() -> EditableFptrs {
        EditableFptrs {
            name: Self::name,
            content: Self::content,
            content_as_str: Self::content_as_str,
            params: Self::params,
            set_content: Self::set_content,
        }
    }
}

pub const COMMAND_SENT: &str = "Command sent";

struct None_;
impl EditableFuncs for None_ {
    fn name() -> &'static str {
        "None"
    }
}

struct Return;
impl EditableFuncs for Return {
    fn name() -> &'static str {
        "Back"
    }
}

impl Editable {
    fn this(&self) -> EditableFptrs {
        match self {
            // model
            Editable::AlarmVolume => AlarmVolume::this(),
            Editable::AvgClimbRateSrc => AvgClimbRateSrc::this(),
            Editable::BatteryGood => BatteryGood::this(),
            Editable::BatteryLow => BatteryLow::this(),
            Editable::CenterFrequency => CenterFrequency::this(),
            Editable::CenterViewCircling => CenterViewCircling::this(),
            Editable::CenterViewStraight => CenterViewStraight::this(),
            Editable::Display => Display::this(),
            Editable::Glider => Glider::this(),
            Editable::GliderSymbol => GliderSymbol::this(),
            Editable::Info1Vario => Info1Vario::this(),
            Editable::Info2Vario => Info2Vario::this(),
            Editable::Info3Vario => Info3Vario::this(),
            Editable::Info1Stf => Info1Stf::this(),
            Editable::Info2Stf => Info2Stf::this(),
            Editable::Info3Stf => Info3Stf::this(),
            Editable::InfoR1 => InfoR1::this(),
            Editable::InfoR2 => InfoR2::this(),
            Editable::InfoR3 => InfoR3::this(),
            Editable::PotK => PotK::this(),
            Editable::PotBar => PotBar::this(),
            Editable::PotAdvice => PotAdvice::this(),
            Editable::NeedleMode => NeedleMode::this(),
            Editable::GustFilterMode => GustFilterMode::this(),
            Editable::GustCircling => GustCircling::this(),
            Editable::AvgMarkerMode => AvgMarkerMode::this(),
            Editable::McCready => McCready::this(),
            Editable::SoundSpreading => SoundSpreading::this(),
            Editable::StfUpperLimit => StfUpperLimit::this(),
            Editable::StfSwitchMode => StfSwitchMode::this(),
            Editable::StfSwitchSpeed => StfSwitchSpeed::this(),
            Editable::FwDowngrade => FwDowngrade::this(),
            Editable::Brightness => Brightness::this(),
            Editable::Qnh => Qnh::this(),
            Editable::BigEncoder => BigEncoder::this(),

            Editable::StfLowerLimit => StfLowerLimit::this(),
            Editable::TcCircleHysteresis => TcCircleHysteresis::this(),
            Editable::TcClimbRate => TcClimbRate::this(),
            Editable::TcSpeedToFly => TcSpeedToFly::this(),
            Editable::Theme => Theme::this(),
            Editable::SoundFreqUpper => SoundFreqUpper::this(),
            Editable::SoundFreqLower => SoundFreqLower::this(),
            Editable::SoundFullScale => SoundFullScale::this(),
            Editable::SinkTone => SinkTone::this(),
            Editable::SinkToneFrom => SinkToneFrom::this(),
            Editable::TicksUntil => TicksUntil::this(),
            Editable::AudioDamping => AudioDamping::this(),
            Editable::SoundAttack => SoundAttack::this(),
            Editable::LoudnessComp => LoudnessComp::this(),
            Editable::SpeedVolume => SpeedVolume::this(),
            Editable::SpeedVolumeRef => SpeedVolumeRef::this(),
            Editable::BeepTempo => BeepTempo::this(),
            Editable::VarioUpperLimit => VarioUpperLimit::this(),
            Editable::VarioLowerLimit => VarioLowerLimit::this(),
            Editable::UnitHeight => UnitHeigth_::this(),
            Editable::UnitHorizontalSpeed => UnitHorizontalSpeed_::this(),
            Editable::UnitVerticalSpeed => UnitVerticalSpeed_::this(),
            Editable::UsageCode => UsageCode::this(),
            Editable::UsageMode => UsageMode::this(),
            Editable::Volume => Volume::this(),
            Editable::Waveform => Waveform_::this(),

            // controller
            Editable::DrainPinConfig => DrainPinConfig::this(),
            Editable::EnergyArrowMult => EnergyArrowMult::this(),
            Editable::FactoryReset => FactoryReset::this(),
            Editable::FlashControl => FlashControl::this(),
            Editable::FlashTest => FlashTest::this(),
            Editable::FlowEmpty => FlowEmpty::this(),
            Editable::FlowSlope => FlowSlope::this(),
            Editable::GearPinConfig => GearPinConfig::this(),
            Editable::AirbrakesPinConfig => AirbrakesPinConfig::this(),
            Editable::GearAlarmModeConfig => GearAlarmModeConfig::this(),
            Editable::ResetConfig => ResetConfig::this(),
            Editable::Rotation => Rotation_::this(),
            Editable::SpeedToFlyPinConfig => SpeedToFlyPinConfig::this(),
            Editable::UserProfile => UserProfile::this(),
            Editable::VarioModeControl => VarioModeControl_::this(),
            Editable::DeviceInfo => DeviceInfo::this(),
            Editable::PageLayout => PageLayout::this(),

            // glider_data
            Editable::Bugs => Bugs::this(),
            Editable::PilotWeight => PilotWeight::this(),
            Editable::WaterBallast => WaterBallast::this(),
            Editable::EmptyMass => EmptyMass::this(),
            Editable::MaxBallast => MaxBallast::this(),
            Editable::ReferenceWeight => ReferenceWeight::this(),
            Editable::PolarValueV1 => PolarValueV1::this(),
            Editable::PolarValueV2 => PolarValueV2::this(),
            Editable::PolarValueV3 => PolarValueV3::this(),
            Editable::PolarValueSi1 => PolarValueSi1::this(),
            Editable::PolarValueSi2 => PolarValueSi2::this(),
            Editable::PolarValueSi3 => PolarValueSi3::this(),

            // sensorbox
            Editable::SensTiltRoll => SensTiltRoll::this(),
            Editable::SensTiltPitch => SensTiltPitch::this(),
            Editable::SensTiltYaw => SensTiltYaw::this(),
            Editable::PitotOffset => PitotOffset::this(),
            Editable::PitotOffsetMeasure => PitotOffsetMeasure::this(),
            Editable::PitotSpan => PitotSpan::this(),
            Editable::QnhDelta => QnhDelta::this(),
            Editable::VarioTc => VarioTc::this(),
            Editable::VarioIntTc => VarioIntTc::this(),
            Editable::WindTc => WindTc::this(),
            Editable::MeanWindTc => MeanWindTc::this(),
            Editable::GnssConfig => GnssConfig::this(),
            Editable::AntBaselen => AntBaselen::this(),
            Editable::AntSlaveDown => AntSlaveDown::this(),
            Editable::AntSlaveRight => AntSlaveRight::this(),
            Editable::BlockHorizon => BlockHorizon::this(),
            Editable::CmdMeas1 => CmdMeas1::this(),
            Editable::CmdMeas2 => CmdMeas2::this(),
            Editable::CmdMeas3 => CmdMeas3::this(),
            Editable::CmdCalcOrientation => CmdCalcOrientation::this(),
            Editable::CmdFineTuneOrientation => CmdFineTuneOrientation::this(),
            Editable::CmdResetSensorbox => CmdResetSensorbox::this(),
            Editable::CmdTestFunction => CmdTestFunction::this(),
            Editable::CmdTestFunctionNumber => CmdTestFunctionNumber::this(),

            Editable::RemoteSave => RemoteSave::this(),
            Editable::RemoteRestart => RemoteRestart::this(),

            // flaps, FLARM, canopy flasher, demo
            Editable::FlarmStatus => FlarmStatus::this(),
            Editable::FlarmBaud => FlarmBaud::this(),
            Editable::NmeaProtocolSel => NmeaProtocolSel::this(),
            Editable::NmeaBaudSel => NmeaBaudSel::this(),
            Editable::FlasherTypeSel => FlasherTypeSel::this(),
            Editable::FlasherActiveSel => FlasherActiveSel::this(),
            Editable::FlashRateSel => FlashRateSel::this(),
            Editable::FlasherOnSpeed => FlasherOnSpeed::this(),
            Editable::FlasherTest => FlasherTest::this(),
            Editable::WarnViewSel => WarnViewSel::this(),
            Editable::WarnLevelSel => WarnLevelSel::this(),
            Editable::HeadOnSel => HeadOnSel::this(),
            Editable::DemoSel => DemoSel::this(),
            Editable::FlapSensorSel => FlapSensorSel::this(),
            Editable::FlapSensorValue => FlapSensorValue::this(),
            Editable::FlapCircling => FlapCircling::this(),
            Editable::FlapAdd => FlapAdd::this(),
            Editable::FlapName => FlapName::this(),
            Editable::FlapUpTo => FlapUpTo::this(),
            Editable::FlapFrom => FlapFrom::this(),
            Editable::FlapPosition => FlapPosition::this(),
            Editable::FlapSetPos => FlapSetPos::this(),
            Editable::FlapDelete => FlapDelete::this(),

            // general
            Editable::None => None_::this(),
            Editable::VarioResponse => VarioResponse::this(),
            Editable::CruiseSwitch => CruiseSwitch::this(),
            Editable::SoundPreset => SoundPreset::this(),
            Editable::PageStyle => PageStyle::this(),
            Editable::SetupWizard => SetupWizard::this(),
            Editable::Return => Return::this(),
        }
    }

    /// The sensor box configuration value behind this editable, if any.
    /// These values are not stored in the frontend but requested from and
    /// written to the sensor box over CAN.
    pub fn remote_config_id(&self) -> Option<crate::CanConfigId> {
        use crate::CanConfigId;
        match self {
            Editable::SensTiltRoll => Some(CanConfigId::SensTiltRoll),
            Editable::SensTiltPitch => Some(CanConfigId::SensTiltPitch),
            Editable::SensTiltYaw => Some(CanConfigId::SensTiltYaw),
            Editable::PitotOffset => Some(CanConfigId::PitotOffset),
            Editable::PitotSpan => Some(CanConfigId::PitotSpan),
            Editable::QnhDelta => Some(CanConfigId::QnhDelta),
            Editable::VarioTc => Some(CanConfigId::VarioTc),
            Editable::VarioIntTc => Some(CanConfigId::VarioIntTc),
            Editable::WindTc => Some(CanConfigId::WindTc),
            Editable::MeanWindTc => Some(CanConfigId::MeanWindTc),
            Editable::GnssConfig => Some(CanConfigId::GnssConfig),
            Editable::AntBaselen => Some(CanConfigId::AntBaselen),
            Editable::AntSlaveDown => Some(CanConfigId::AntSlaveDown),
            Editable::AntSlaveRight => Some(CanConfigId::AntSlaveRight),
            Editable::BlockHorizon => Some(CanConfigId::BlockHorizon),
            _ => None,
        }
    }

    pub fn content_as_str(&self, content: Content, cm: &CoreModel) -> TString<20> {
        let mut conv = Convert::<20>::new(b' ');
        let params = self.params(cm);

        match params {
            Params::Cmd(_params) => {
                if let Content::Command(msg) = content {
                    conv.write_str(msg.as_str()).unwrap();
                }
            }
            Params::Date(params) => {
                if params.is_active {
                    if let Content::Date(Some(date)) = content {
                        let date_string = date.as_string();
                        conv.write_str(date_string.as_str()).unwrap();
                    } else {
                        conv.write_str("-").unwrap();
                    }
                } else {
                    conv.write_str("-").unwrap();
                }
            }
            Params::Enum(_params) => {
                if let Content::Enum(val) = content {
                    conv.write_str(val.as_str()).unwrap();
                }
            }
            Params::F32(params) => {
                if let Content::F32(opt_val) = content {
                    // the converter fills from the right: unit, blank, number
                    if let (Editable::GnssConfig, Some(val)) = (*self, opt_val) {
                        // the product instead of the number (Stefan 09.11.2026)
                        conv.write_str(sensorbox::gnss_config_name(val)).unwrap();
                    } else if let Some(val) = opt_val {
                        conv.write_str(params.unit).unwrap();
                        // "65.4°" like the sketches, other units with a blank
                        if params.unit != "°" {
                            conv.write_u8(b' ').unwrap();
                        }
                        conv.f32(val, params.dec_places as usize).unwrap();
                    } else {
                        // a sensor box value the box has not answered yet
                        conv.write_str("-").unwrap();
                    }
                }
            }
            Params::List(_params) => {
                if let Content::List(val) = content {
                    if *self == Editable::FlapCircling {
                        // the list shows the names of the flap stages
                        let t = cm.flap_table();
                        conv.write_str(t.name(val.max(0) as usize)).unwrap();
                    } else {
                        ((self.this()).content_as_str)(&mut conv, val);
                    }
                }
            }
            Params::String(_params) => {
                if let Content::String(val) = content {
                    conv.write_str(val.as_str()).unwrap();
                }
            }
        }
        TString::<20>::from_str(conv.as_str())
    }

    pub fn content(&self, cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        ((self.this()).content)(cm, cc)
    }

    pub fn name(&self) -> &'static str {
        ((self.this()).name)()
    }

    pub fn params(&self, cm: &CoreModel) -> Params {
        ((self.this()).params)(cm)
    }

    pub fn set_content(&self, cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        ((self.this()).set_content)(cm, cc, content)
    }
}
