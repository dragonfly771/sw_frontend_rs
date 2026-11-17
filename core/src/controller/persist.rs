/// The Persistence Layer stores Data in EEPROM an distributes it to NMEA and Can Bus interfaces
///
/// Data points that can be processed must be recorded by the PersistenceId. The
/// restore_item() method writes the data read from the EEPROM to the CoreModel.
///
/// The persist_set() method receives data from the NMEA and CAN bus interfaces and from the editor,
/// saves it in the EEPROM if necessary and distributes the data to interfaces if required. The
/// distribution of the data to the interfaces is controlled via the enum Echo:
///   - Echo::None -> no distribution
///   - Echo::Nmea -> forwarding to the NMEA interface
///   - Echo::Can -> forwarding to the Can bus interface
///   - Echo::NmeaAndCan -> forwarding to NMEA and Can Bus
///
/// The module also ensures that the Nmea interface and the EEPROM are not overloaded by too much
/// data. This is achieved by initially storing the data in an index set and only forwarding it
/// after a pause of incoming data of at least 500 ms.
use heapless::Vec;
use num_enum::FromPrimitive;

use super::{
    helpers::{GearPins, InPinFunction, InTogglePinFunction, OutPinFunction},
    DataSource, VarioModeControl, MAX_PERS_IDS, REMOTE_GET_QUEUE,
};
use crate::{
    basic_config::PERSISTENCE_TIMEOUT,
    controller::{
        helpers::{CanConfigId, IntToDuration},
        RemoteConfig,
    },
    flight_physics::polar_store,
    model::{
        editable::Content,
        flarm_cfg::{FlashRate, FlasherActive, FlasherType, WarnView},
        AvgMarker, EditMode, GpsState, GustFilter, NmeaProtocol, UnitHeight, UnitHorizontalSpeed,
        UnitVerticalSpeed, VarioNeedle, NMEA_BAUDS, NMEA_BAUD_DEFAULT_IDX,
    },
    set_snd_spreading_factor,
    system_of_units::Speed,
    utils::Variant,
    view::viewable::{
        centerview::CenterView,
        vario_infoview::{Info3View, LineView},
    },
    CanFrame, CoreController, CoreError, CoreModel, DateTime, FloatToSpeed, Frame, GenericId,
    IdleEvent, Mass, PersistenceItem, Pressure, Rotation, VarioMode, Waveform,
};

/// It is not permitted to change the sequence or assignment, as the number references the memory
/// location in the EEPROM. New memory data may only ever be inserted before the LastItem.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, FromPrimitive, defmt::Format)]
#[repr(u16)]
pub enum PersistenceId {
    Volume = 0,
    McCready = 1,
    WaterBallast = 2,
    PilotWeight = 3,
    Glider = 4,
    VarioModeControl = 5,
    DisplayTheme = 6, // Dark = 0, Bright = 1
    Qnh = 7,
    Bugs = 8,
    Display = 9,
    TcClimbRate = 10,
    TcSpeedToFly = 11,
    Info1Vario = 12,
    Info2Vario = 13,
    Rotation = 14,
    CenterFrequency = 15,
    CenterViewCircling = 16,
    CenterViewStraight = 17,
    EmptyMass = 18,
    MaxBallast = 19,
    ReferenceWeight = 20,
    PolarValueV1 = 21,
    PolarValueV2 = 22,
    PolarValueV3 = 23,
    PolarValueSi1 = 24,
    PolarValueSi2 = 25,
    PolarValueSi3 = 26,
    GliderSymbol = 27,
    BatteryGood = 28,
    BatteryLow = 29,
    DrainPinConfig = 30,
    FlowEmpty = 31,
    FlowSlope = 32,
    FlashControl = 33,
    SpeedToFlyPinConfig = 34,
    GearPinConfig = 35,
    AirbrakesPinConfig = 36,
    GearAlarmMode = 37,
    AlarmVolume = 38,
    StfUpperLimit = 39,
    StfLowerLimit = 40,
    AvgClimbeRateSrc = 41,
    StfClimbrateAlt = 42, // no longer in use
    TcCircleHysteresis = 43,
    EnergyArrowMult = 44,
    VarioUpperLimit = 45,
    VarioLowerLimit = 46,
    Info1Stf = 47,
    Info2Stf = 48,
    Info3Vario = 49,
    Info3Stf = 50,
    UnitHorizontalSpeed = 51,
    UnitVerticalSpeed = 52,
    UnitHeight = 53,
    ClubMode = 54,
    Date = 55,
    Waveform = 56,
    SoundSpreading = 57,
    /// no longer used - the Stefly tone is the only tone. Id stays reserved
    ToneProfileUnused = 58,
    CenterFreqStefly = 59,
    SndFreqUpper = 60,
    SndFreqLower = 61,
    SndFullScale = 62,
    SinkToneMode = 63,
    SinkToneFrom = 64,
    TicksUntil = 65,
    AudioDamping = 66,
    LoudnessComp = 67,
    SpeedVolSlope = 68,
    SpeedVolRef = 69,
    BeepTempo = 70,
    SoundAttack = 71,
    /// reserved: the vario design is no longer selectable (ring only)
    VarioDesignUnused = 72,
    InfoR1 = 73,
    InfoR2 = 74,
    InfoR3 = 75,
    PotK = 76,
    /// no longer used - the POT box has no offset since 0.4.1.48. Id stays reserved
    PotC0Unused = 77,
    VarioNeedle = 78,
    GustFilter = 79,
    GustCircling = 80,
    RingAvgMarker = 81,
    StfSwitchMode = 82,
    StfSwitchSpeed = 83,
    FwDowngrade = 84,
    Brightness = 85,
    PageA0 = 86,
    PageA1 = 87,
    PageA2 = 88,
    PageA3 = 89,
    PageA4 = 90,
    PageA5 = 91,
    PageB0 = 92,
    PageB1 = 93,
    PageB2 = 94,
    PageB3 = 95,
    PageB4 = 96,
    PageB5 = 97,
    PageC0 = 98,
    PageC1 = 99,
    PageC2 = 100,
    PageC3 = 101,
    PageC4 = 102,
    PageC5 = 103,
    PageIdx = 104,
    BigEncoder = 105,
    /// no longer used - the POT box has no memory since 0.4.1.48. Id stays reserved
    PotMemoryUnused = 106,
    // flap table (plan_klappen_menue.md): settings word, one word and one
    // sensor position per stage
    FlapTable = 107,
    FlapStage0 = 108,
    FlapStage1 = 109,
    FlapStage2 = 110,
    FlapStage3 = 111,
    FlapStage4 = 112,
    FlapStage5 = 113,
    FlapStage6 = 114,
    FlapStage7 = 115,
    FlapStage8 = 116,
    FlapStage9 = 117,
    FlapStage10 = 118,
    FlapStage11 = 119,
    FlapPos0 = 120,
    FlapPos1 = 121,
    FlapPos2 = 122,
    FlapPos3 = 123,
    FlapPos4 = 124,
    FlapPos5 = 125,
    FlapPos6 = 126,
    FlapPos7 = 127,
    FlapPos8 = 128,
    FlapPos9 = 129,
    FlapPos10 = 130,
    FlapPos11 = 131,
    // FLARM interface and SteFly canopy flasher (plan_flarm_blitzer.md,
    // plan_flarm_warnung.md)
    FlarmBaud = 132,
    FlasherType = 133,
    FlasherActive = 134,
    FlashRate = 135,
    FlasherOnSpeed = 136,
    FlarmWarnView = 137,
    FlarmWarnLevel = 138,
    FlarmHeadOn = 139,
    FlarmTeamId = 140,
    // serial NMEA port: protocol (XCSoar / LXNAV) and baud rate (index into
    // NMEA_BAUDS)
    NmeaProtocol = 141,
    NmeaBaud = 142,
    /// volume of the cruise tone (143.. left free for the NMEA work)
    VolumeStf = 150,
    /// POT box: bar of the yes / no in percent of the MacCready value
    PotBar = 151,
    // LastItem was still 143 after VolumeStf (150) came in, so that value
    // was never written to the EEPROM; with 152 it is stored like the others.
    // 143..149 have no id: the restore at start skips numbers without one
    // (utils/persistence.rs, PersistenceIterator - it used to panic there)
    /// menu mode: 0 = Basic (delivery state), 1 = Expert (15.11.2026)
    MenuMode = 152,
    /// POT box: yes / no symbol on (delivery state) or off (16.11.2026)
    PotAdvice = 153,
    LastItem = 154, // Items smaller than this are stored in eeprom

    // Special function Ids
    VarioMode = 65532,
    UserProfile = 65533,
    DeleteAll = 65534,
    #[default]
    DoNotStore = 65535,
}

/// PersistenceId of a packed page word (0 = A, 1 = B, 2 = C) for page idx
pub fn page_id(word: u8, idx: usize) -> PersistenceId {
    const IDS: [[PersistenceId; 6]; 3] = [
        [
            PersistenceId::PageA0,
            PersistenceId::PageA1,
            PersistenceId::PageA2,
            PersistenceId::PageA3,
            PersistenceId::PageA4,
            PersistenceId::PageA5,
        ],
        [
            PersistenceId::PageB0,
            PersistenceId::PageB1,
            PersistenceId::PageB2,
            PersistenceId::PageB3,
            PersistenceId::PageB4,
            PersistenceId::PageB5,
        ],
        [
            PersistenceId::PageC0,
            PersistenceId::PageC1,
            PersistenceId::PageC2,
            PersistenceId::PageC3,
            PersistenceId::PageC4,
            PersistenceId::PageC5,
        ],
    ];
    IDS[(word as usize).min(2)][idx.min(5)]
}

/// This list defines which Ids are stored only in user profile 0
const PROFILE_0_IDS: &[PersistenceId] = &[
    PersistenceId::Glider,
    PersistenceId::EmptyMass,
    PersistenceId::MaxBallast,
    PersistenceId::ReferenceWeight,
    PersistenceId::PolarValueV1,
    PersistenceId::PolarValueV2,
    PersistenceId::PolarValueV3,
    PersistenceId::PolarValueSi1,
    PersistenceId::PolarValueSi2,
    PersistenceId::PolarValueSi3,
    PersistenceId::DrainPinConfig,
    PersistenceId::FlowEmpty,
    PersistenceId::FlowSlope,
    PersistenceId::FlashControl,
    PersistenceId::SpeedToFlyPinConfig,
    PersistenceId::GearPinConfig,
    PersistenceId::AirbrakesPinConfig,
    PersistenceId::GearAlarmMode,
    PersistenceId::ClubMode,
    PersistenceId::MenuMode,
    PersistenceId::Date,
    PersistenceId::FlapTable,
    PersistenceId::FlapStage0,
    PersistenceId::FlapStage1,
    PersistenceId::FlapStage2,
    PersistenceId::FlapStage3,
    PersistenceId::FlapStage4,
    PersistenceId::FlapStage5,
    PersistenceId::FlapStage6,
    PersistenceId::FlapStage7,
    PersistenceId::FlapStage8,
    PersistenceId::FlapStage9,
    PersistenceId::FlapStage10,
    PersistenceId::FlapStage11,
    PersistenceId::FlapPos0,
    PersistenceId::FlapPos1,
    PersistenceId::FlapPos2,
    PersistenceId::FlapPos3,
    PersistenceId::FlapPos4,
    PersistenceId::FlapPos5,
    PersistenceId::FlapPos6,
    PersistenceId::FlapPos7,
    PersistenceId::FlapPos8,
    PersistenceId::FlapPos9,
    PersistenceId::FlapPos10,
    PersistenceId::FlapPos11,
    PersistenceId::FlarmBaud,
    PersistenceId::FlasherType,
    PersistenceId::FlasherActive,
    PersistenceId::FlashRate,
    PersistenceId::FlasherOnSpeed,
    PersistenceId::FlarmWarnView,
    PersistenceId::FlarmWarnLevel,
    PersistenceId::FlarmHeadOn,
    PersistenceId::FlarmTeamId,
    PersistenceId::NmeaProtocol,
    PersistenceId::NmeaBaud,
];

/// This list defines which data is destroyed when a profile is deleted
const DELETE_CONFIG_LIST: &[PersistenceId] = &[
    PersistenceId::Volume,
    PersistenceId::VolumeStf,
    PersistenceId::McCready,
    PersistenceId::WaterBallast,
    PersistenceId::PilotWeight,
    PersistenceId::VarioModeControl,
    PersistenceId::DisplayTheme,
    PersistenceId::Qnh,
    PersistenceId::Bugs,
    PersistenceId::Display,
    PersistenceId::TcClimbRate,
    PersistenceId::TcSpeedToFly,
    PersistenceId::Info1Vario,
    PersistenceId::Info2Vario,
    PersistenceId::Rotation,
    PersistenceId::CenterFrequency,
    PersistenceId::CenterViewCircling,
    PersistenceId::CenterViewStraight,
    PersistenceId::GliderSymbol,
    PersistenceId::BatteryGood,
    PersistenceId::BatteryLow,
    PersistenceId::FlowEmpty,
    PersistenceId::FlowSlope,
    PersistenceId::AlarmVolume,
    PersistenceId::StfUpperLimit,
    PersistenceId::StfLowerLimit,
    PersistenceId::AvgClimbeRateSrc,
    PersistenceId::StfClimbrateAlt,
    PersistenceId::TcCircleHysteresis,
    PersistenceId::EnergyArrowMult,
    PersistenceId::Info1Stf,
    PersistenceId::Info2Stf,
    PersistenceId::Info3Vario,
    PersistenceId::Info3Stf,
    PersistenceId::Waveform,
    PersistenceId::SoundSpreading,
    PersistenceId::CenterFreqStefly,
    PersistenceId::SndFreqUpper,
    PersistenceId::SndFreqLower,
    PersistenceId::SndFullScale,
    PersistenceId::SinkToneMode,
    PersistenceId::SinkToneFrom,
    PersistenceId::TicksUntil,
    PersistenceId::AudioDamping,
    PersistenceId::LoudnessComp,
    PersistenceId::SpeedVolSlope,
    PersistenceId::SpeedVolRef,
    PersistenceId::BeepTempo,
    PersistenceId::SoundAttack,
    PersistenceId::InfoR1,
    PersistenceId::InfoR2,
    PersistenceId::InfoR3,
    PersistenceId::PotK,
    PersistenceId::PotC0Unused,
    PersistenceId::PotMemoryUnused,
    PersistenceId::PotBar,
    PersistenceId::PotAdvice,
    PersistenceId::VarioNeedle,
    PersistenceId::GustFilter,
    PersistenceId::GustCircling,
    PersistenceId::RingAvgMarker,
    PersistenceId::StfSwitchMode,
    PersistenceId::StfSwitchSpeed,
    PersistenceId::FwDowngrade,
    PersistenceId::Brightness,
    PersistenceId::PageA0,
    PersistenceId::PageA1,
    PersistenceId::PageA2,
    PersistenceId::PageA3,
    PersistenceId::PageA4,
    PersistenceId::PageA5,
    PersistenceId::PageB0,
    PersistenceId::PageB1,
    PersistenceId::PageB2,
    PersistenceId::PageB3,
    PersistenceId::PageB4,
    PersistenceId::PageB5,
    PersistenceId::PageC0,
    PersistenceId::PageC1,
    PersistenceId::PageC2,
    PersistenceId::PageC3,
    PersistenceId::PageC4,
    PersistenceId::PageC5,
    PersistenceId::PageIdx,
    PersistenceId::BigEncoder,
];

/// The following data is deleted when a new glider is selected
const SPECIFIC_POLAR_SETTINGS: &[PersistenceId] = &[
    PersistenceId::EmptyMass,
    PersistenceId::MaxBallast,
    PersistenceId::ReferenceWeight,
    PersistenceId::PolarValueV1,
    PersistenceId::PolarValueV2,
    PersistenceId::PolarValueV3,
    PersistenceId::PolarValueSi1,
    PersistenceId::PolarValueSi2,
    PersistenceId::PolarValueSi3,
];

/// Recalculate the anchor frequencies of the tone curve from the legacy spreading factor
///
/// The spreading factor is no longer a setting of its own (see concept 3.7). Devices updated
/// from an older firmware only have the centre frequency and the spreading factor in EEPROM;
/// this converts them once into the new anchors. Anchors that are stored themselves have higher
/// PersistenceIds and are restored afterwards, so they always win.
fn refresh_tone_anchors(cm: &mut CoreModel) {
    let spreading_factor = crate::get_snd_spreading_factor(cm);
    set_snd_spreading_factor(cm, spreading_factor);
}

/// Check if PersistenceId is in the list of profile 0
pub fn profile_always_0(id: PersistenceId) -> bool {
    let mut is_unique = false;
    for iter_id in PROFILE_0_IDS {
        if *iter_id == id {
            is_unique = true;
            break;
        }
    }
    is_unique
}

#[derive(PartialEq)]
pub enum Echo {
    None,
    Nmea,
    Can,
    NmeaAndCan,
}

/// Store item content into data model
///
/// This method is also called directly from the idle-loop during start-up
pub fn restore_item(
    cc: &mut CoreController,
    cm: &mut CoreModel,
    item: PersistenceItem,
) -> Result<(), CoreError> {
    // A setting found in the EEPROM: this device was in use before (menu
    // mode of an updated device, `decide_menu_mode`). Not the user profile -
    // the restore sends it always, from an empty chip as well - and not the
    // date the device stores by itself.
    if !matches!(item.id, PersistenceId::UserProfile | PersistenceId::Date) {
        cm.control.items_restored = true;
    }
    match item.id {
        PersistenceId::Volume => cm.config.volume = item.to_i8(),
        PersistenceId::VolumeStf => cm.config.volume_stf = item.to_i8(),
        PersistenceId::McCready => cm.config.mc_cready = Speed::from_m_s(item.to_f32()?),
        PersistenceId::WaterBallast => {
            // an unwritten EEPROM item reads as NaN (0xffffffff)
            let kg = item.to_f32()?;
            cm.glider_data.water_ballast =
                Mass::from_kg(if kg.is_finite() { kg.max(0.0) } else { 0.0 })
        }
        PersistenceId::PilotWeight => cm.glider_data.pilot_weight = Mass::from_kg(item.to_f32()?),
        PersistenceId::Glider => {
            let raw_idx = if item.to_i32() as usize >= polar_store::POLARS.len() {
                0
            } else {
                item.to_i32()
            };
            cm.config.glider_idx = raw_idx;
            cm.glider_data.basic_glider_data = polar_store::POLARS[raw_idx as usize];
        }
        PersistenceId::VarioModeControl => {
            cm.control.vario_mode_control = VarioModeControl::from(item.to_u8())
        }
        PersistenceId::DisplayTheme => {
            cm.config.theme = if item.to_i32() == 0 {
                &cm.device_const.dark_theme
            } else {
                &cm.device_const.bright_theme
            }
        }
        PersistenceId::Qnh => {
            let qnh = Pressure::from_hpa(item.to_f32()?);
            cm.sensor.pressure_altitude.set_qnh(qnh)
        }
        PersistenceId::Bugs => cm.glider_data.set_bugs(item.to_f32()?),
        PersistenceId::Display => cm.config.display_active = item.to_u8().into(),
        PersistenceId::TcClimbRate => {
            let tc = item.to_f32()?;
            cm.config.av2_climb_rate_tc = tc;
            cc.av2_climb_rate.set_time_const(tc);
        }
        PersistenceId::TcSpeedToFly => {
            let tc = item.to_f32()?;
            cm.config.av_speed_to_fly_tc = tc;
            cc.av_speed_to_fly.set_time_const(tc);
        }
        // The legacy single page settings also feed page 1, so a device
        // updated from an older firmware looks exactly as before. The page
        // ids have higher numbers and win as soon as they are stored.
        PersistenceId::Info1Vario => {
            cm.config.info1_vario = LineView::from(item.to_u8());
            cm.config.pages[0].boxes[0] = cm.config.info1_vario;
        }
        PersistenceId::Info2Vario => {
            cm.config.info2_vario = LineView::from(item.to_u8());
            cm.config.pages[0].boxes[1] = cm.config.info2_vario;
        }
        // the vario design is no longer selectable - page 1 keeps the ring
        PersistenceId::VarioDesignUnused => (),
        PersistenceId::InfoR1 => {
            cm.config.info_r1 = LineView::from(item.to_u8());
            cm.config.pages[0].boxes[2] = cm.config.info_r1;
        }
        PersistenceId::InfoR2 => {
            cm.config.info_r2 = LineView::from(item.to_u8());
            cm.config.pages[0].boxes[3] = cm.config.info_r2;
        }
        PersistenceId::InfoR3 => {
            cm.config.info_r3 = LineView::from(item.to_u8());
            cm.config.pages[0].boxes[4] = cm.config.info_r3;
        }
        PersistenceId::PotK => cm.config.pot_k = item.to_f32()?,
        PersistenceId::PotC0Unused => (),
        PersistenceId::PotMemoryUnused => (),
        PersistenceId::PotBar => {
            // stored in percent; anything outside the menu range is ignored
            let pct = item.to_f32()?;
            if (50.0..=150.0).contains(&pct) {
                cm.config.pot_bar = pct / 100.0;
            }
        }
        PersistenceId::PotAdvice => cm.config.pot_advice = item.to_bool(),
        PersistenceId::VarioNeedle => cm.config.vario_needle = VarioNeedle::from(item.to_u8()),
        PersistenceId::GustFilter => cm.config.gust_filter = GustFilter::from(item.to_u8()),
        PersistenceId::GustCircling => cm.config.gust_filter_circling = item.to_bool(),
        PersistenceId::RingAvgMarker => cm.config.ring_avg_marker = AvgMarker::from(item.to_u8()),
        PersistenceId::Rotation => cm.control.rotation = Rotation::from(item.to_u8()),
        PersistenceId::CenterFrequency => {
            cm.config.snd_center_freq = item.to_f32()?;
            // keep the tone anchors in step with the centre frequency for devices that
            // still store the old spreading factor representation
            refresh_tone_anchors(cm);
        }
        PersistenceId::CenterViewCircling => {
            cm.config.center_circling = CenterView::from(item.to_u8());
            cm.config.pages[0].center_circling = cm.config.center_circling;
        }
        PersistenceId::CenterViewStraight => {
            cm.config.center_straight = CenterView::from(item.to_u8());
            cm.config.pages[0].center_straight = cm.config.center_straight;
        }
        PersistenceId::EmptyMass => cm.glider_data.basic_glider_data.empty_mass = item.to_f32()?,
        PersistenceId::MaxBallast => {
            cm.glider_data.basic_glider_data.max_ballast = item.to_f32()?
        }
        PersistenceId::ReferenceWeight => {
            cm.glider_data.basic_glider_data.reference_weight = item.to_f32()?
        }
        PersistenceId::PolarValueV1 => {
            cm.glider_data.basic_glider_data.polar_values[0][0] = item.to_f32()?
        }
        PersistenceId::PolarValueV2 => {
            cm.glider_data.basic_glider_data.polar_values[1][0] = item.to_f32()?
        }
        PersistenceId::PolarValueV3 => {
            cm.glider_data.basic_glider_data.polar_values[2][0] = item.to_f32()?
        }
        PersistenceId::PolarValueSi1 => {
            cm.glider_data.basic_glider_data.polar_values[0][1] = item.to_f32()?
        }
        PersistenceId::PolarValueSi2 => {
            cm.glider_data.basic_glider_data.polar_values[1][1] = item.to_f32()?
        }
        PersistenceId::PolarValueSi3 => {
            cm.glider_data.basic_glider_data.polar_values[2][1] = item.to_f32()?
        }
        PersistenceId::GliderSymbol => cm.config.glider_symbol = item.to_bool(),
        PersistenceId::BatteryGood => cm.config.battery_good = item.to_f32()?,
        PersistenceId::BatteryLow => cm.config.battery_low = item.to_f32()?,
        PersistenceId::DrainPinConfig => cc
            .drain_control
            .set_pin_function(InPinFunction::from(item.to_u8()), cm),
        PersistenceId::FlowEmpty => cc.drain_control.flow_rate_offset = item.to_f32()?,
        PersistenceId::FlowSlope => cc.drain_control.flow_rate_slope = item.to_f32()?,
        PersistenceId::FlashControl => cc
            .flash_control
            .set_pin_function(OutPinFunction::from(item.to_u8())),
        PersistenceId::SpeedToFlyPinConfig => cc
            .speed_to_fly_control
            .set_pin_function(InTogglePinFunction::from(item.to_u8())),
        PersistenceId::GearPinConfig => cc
            .gear_alarm_control
            .set_gear_pin_function(InPinFunction::from(item.to_u8())),
        PersistenceId::AirbrakesPinConfig => cc
            .gear_alarm_control
            .set_airbrakes_pin_function(InPinFunction::from(item.to_u8())),
        PersistenceId::GearAlarmMode => cc
            .gear_alarm_control
            .set_gear_pin_mode(GearPins::from(item.to_u8())),
        PersistenceId::AlarmVolume => cm.control.alarm_volume = item.to_i8(),
        PersistenceId::StfUpperLimit => cm.config.stf_upper_limit = item.to_f32()?.m_s(),
        PersistenceId::StfSwitchMode => cm.config.stf_switch_mode = item.to_u8().min(2),
        PersistenceId::StfSwitchSpeed => cm.config.stf_switch_speed = item.to_f32()?.m_s(),
        PersistenceId::FwDowngrade => cm.config.allow_fw_downgrade = item.to_bool(),
        PersistenceId::PageA0 => cm.config.pages[0].set_word_a(item.to_u32()),
        PersistenceId::PageA1 => cm.config.pages[1].set_word_a(item.to_u32()),
        PersistenceId::PageA2 => cm.config.pages[2].set_word_a(item.to_u32()),
        PersistenceId::PageA3 => cm.config.pages[3].set_word_a(item.to_u32()),
        PersistenceId::PageA4 => cm.config.pages[4].set_word_a(item.to_u32()),
        PersistenceId::PageA5 => cm.config.pages[5].set_word_a(item.to_u32()),
        PersistenceId::PageB0 => cm.config.pages[0].set_word_b(item.to_u32()),
        PersistenceId::PageB1 => cm.config.pages[1].set_word_b(item.to_u32()),
        PersistenceId::PageB2 => cm.config.pages[2].set_word_b(item.to_u32()),
        PersistenceId::PageB3 => cm.config.pages[3].set_word_b(item.to_u32()),
        PersistenceId::PageB4 => cm.config.pages[4].set_word_b(item.to_u32()),
        PersistenceId::PageB5 => cm.config.pages[5].set_word_b(item.to_u32()),
        PersistenceId::PageC0 => cm.config.pages[0].set_word_c(item.to_u32()),
        PersistenceId::PageC1 => cm.config.pages[1].set_word_c(item.to_u32()),
        PersistenceId::PageC2 => cm.config.pages[2].set_word_c(item.to_u32()),
        PersistenceId::PageC3 => cm.config.pages[3].set_word_c(item.to_u32()),
        PersistenceId::PageC4 => cm.config.pages[4].set_word_c(item.to_u32()),
        PersistenceId::PageC5 => cm.config.pages[5].set_word_c(item.to_u32()),
        PersistenceId::PageIdx => {
            cm.config.page_idx = item.to_u8().min((crate::model::MAX_PAGES - 1) as u8);
            crate::controller::editor::apply_page(cm);
        }
        PersistenceId::BigEncoder => cm.config.big_encoder_pages = item.to_bool(),
        PersistenceId::Brightness => {
            // lower limit, so the display always stays readable
            let v = item.to_u8().clamp(crate::model::BRIGHTNESS_MIN, 100);
            cm.config.display_brightness = v;
            cc.send_idle_event(IdleEvent::SetBacklight(v));
        }
        PersistenceId::StfLowerLimit => cm.config.stf_lower_limit = item.to_f32()?.m_s(),
        PersistenceId::AvgClimbeRateSrc => {
            cm.control.avg_climb_rate_src = DataSource::from(item.to_u8())
        }
        PersistenceId::StfClimbrateAlt => (),
        PersistenceId::TcCircleHysteresis => cm.config.circle_hysteresis_tc = item.to_i8(),
        PersistenceId::EnergyArrowMult => cm.control.energy_arrow_mult = item.to_f32()?,
        PersistenceId::VarioUpperLimit => cm.config.vario_upper_limit = item.to_f32()?.m_s(),
        PersistenceId::VarioLowerLimit => cm.config.vario_lower_limit = item.to_f32()?.m_s(),
        PersistenceId::Info1Stf => {
            cm.config.info1_stf = LineView::from(item.to_u8());
            cm.config.pages[0].stf_boxes[0] = cm.config.info1_stf;
        }
        PersistenceId::Info2Stf => {
            cm.config.info2_stf = LineView::from(item.to_u8());
            cm.config.pages[0].stf_boxes[1] = cm.config.info2_stf;
        }
        PersistenceId::Info3Vario => cm.config.info3_vario = Info3View::from(item.to_u8()),
        PersistenceId::Info3Stf => cm.config.info3_stf = Info3View::from(item.to_u8()),
        PersistenceId::UnitHorizontalSpeed => {
            cm.config.unit_horizontal_speed = UnitHorizontalSpeed::from(item.to_u8())
        }
        PersistenceId::UnitVerticalSpeed => {
            cm.config.unit_vertical_speed = UnitVerticalSpeed::from(item.to_u8())
        }
        PersistenceId::UnitHeight => cm.config.unit_height = UnitHeight::from(item.to_u8()),
        PersistenceId::ClubMode => cm.config.club_mode = item.to_bool(),
        PersistenceId::MenuMode => {
            cm.config.basic_mode = item.to_u8() == 0;
            cm.control.mode_checked = true;
        }
        PersistenceId::Date => {
            let data = item.to_4u8();
            cm.sensor.gps_date_time.mut_date().from_array_4u8(data);
        }
        PersistenceId::Waveform => {
            cm.calculated.sound_params.waveform = Waveform::from(item.to_u8())
        }
        PersistenceId::SoundSpreading => set_snd_spreading_factor(cm, item.to_f32()?),
        PersistenceId::ToneProfileUnused => (),
        PersistenceId::CenterFreqStefly => cm.config.snd_center_freq = item.to_f32()?,
        PersistenceId::SndFreqUpper => cm.config.snd_freq_upper = item.to_f32()?,
        PersistenceId::SndFreqLower => cm.config.snd_freq_lower = item.to_f32()?,
        PersistenceId::SndFullScale => cm.config.snd_full_scale = item.to_f32()?,
        PersistenceId::SinkToneMode => cm.config.snd_sink_mode = item.to_u8().min(2),
        PersistenceId::SinkToneFrom => cm.config.snd_sink_from = item.to_f32()?,
        PersistenceId::TicksUntil => cm.config.snd_ticks_until = item.to_f32()?,
        PersistenceId::AudioDamping => cm.config.snd_audio_tc = item.to_f32()?,
        PersistenceId::LoudnessComp => cm.config.snd_loudness_comp = item.to_f32()?,
        PersistenceId::SpeedVolSlope => cm.config.snd_speed_vol_slope = item.to_f32()?,
        PersistenceId::SpeedVolRef => cm.config.snd_speed_vol_ref = item.to_f32()?,
        PersistenceId::BeepTempo => cm.config.snd_beep_tempo = item.to_f32()?,
        PersistenceId::SoundAttack => cm.config.snd_attack_ms = item.to_f32()?,

        PersistenceId::FlapTable => cm.config.flaps.set_word(item.to_u32()),
        PersistenceId::FlapStage0 => cm.config.flaps.set_stage_word(0, item.to_u32()),
        PersistenceId::FlapStage1 => cm.config.flaps.set_stage_word(1, item.to_u32()),
        PersistenceId::FlapStage2 => cm.config.flaps.set_stage_word(2, item.to_u32()),
        PersistenceId::FlapStage3 => cm.config.flaps.set_stage_word(3, item.to_u32()),
        PersistenceId::FlapStage4 => cm.config.flaps.set_stage_word(4, item.to_u32()),
        PersistenceId::FlapStage5 => cm.config.flaps.set_stage_word(5, item.to_u32()),
        PersistenceId::FlapStage6 => cm.config.flaps.set_stage_word(6, item.to_u32()),
        PersistenceId::FlapStage7 => cm.config.flaps.set_stage_word(7, item.to_u32()),
        PersistenceId::FlapStage8 => cm.config.flaps.set_stage_word(8, item.to_u32()),
        PersistenceId::FlapStage9 => cm.config.flaps.set_stage_word(9, item.to_u32()),
        PersistenceId::FlapStage10 => cm.config.flaps.set_stage_word(10, item.to_u32()),
        PersistenceId::FlapStage11 => cm.config.flaps.set_stage_word(11, item.to_u32()),
        PersistenceId::FlapPos0 => cm.config.flaps.stages[0].position = item.to_f32()?,
        PersistenceId::FlapPos1 => cm.config.flaps.stages[1].position = item.to_f32()?,
        PersistenceId::FlapPos2 => cm.config.flaps.stages[2].position = item.to_f32()?,
        PersistenceId::FlapPos3 => cm.config.flaps.stages[3].position = item.to_f32()?,
        PersistenceId::FlapPos4 => cm.config.flaps.stages[4].position = item.to_f32()?,
        PersistenceId::FlapPos5 => cm.config.flaps.stages[5].position = item.to_f32()?,
        PersistenceId::FlapPos6 => cm.config.flaps.stages[6].position = item.to_f32()?,
        PersistenceId::FlapPos7 => cm.config.flaps.stages[7].position = item.to_f32()?,
        PersistenceId::FlapPos8 => cm.config.flaps.stages[8].position = item.to_f32()?,
        PersistenceId::FlapPos9 => cm.config.flaps.stages[9].position = item.to_f32()?,
        PersistenceId::FlapPos10 => cm.config.flaps.stages[10].position = item.to_f32()?,
        PersistenceId::FlapPos11 => cm.config.flaps.stages[11].position = item.to_f32()?,
        PersistenceId::FlarmBaud => cm.config.flarm.set_baud_word(item.to_u32()),
        PersistenceId::FlasherType => {
            cm.config.flarm.flasher_type = FlasherType::from(item.to_u8())
        }
        PersistenceId::FlasherActive => {
            cm.config.flarm.flasher_active = FlasherActive::from(item.to_u8())
        }
        PersistenceId::FlashRate => cm.config.flarm.flash_rate = FlashRate::from(item.to_u8()),
        PersistenceId::FlasherOnSpeed => {
            cm.config.flarm.on_speed_kmh = item.to_f32()?.clamp(20.0, 150.0)
        }
        PersistenceId::FlarmWarnView => cm.config.flarm.warn_view = WarnView::from(item.to_u8()),
        PersistenceId::FlarmWarnLevel => cm.config.flarm.warn_level = item.to_u8().clamp(1, 3),
        PersistenceId::FlarmHeadOn => cm.config.flarm.head_on = item.to_bool(),
        PersistenceId::FlarmTeamId => cm.config.flarm.team_id = item.to_u32(),
        PersistenceId::NmeaProtocol => cm.config.nmea_protocol = NmeaProtocol::from(item.to_u8()),
        PersistenceId::NmeaBaud => {
            let idx = item.to_u8();
            cm.config.nmea_baud_idx = if (idx as usize) < NMEA_BAUDS.len() {
                idx
            } else {
                NMEA_BAUD_DEFAULT_IDX
            };
            // the device reprograms the USART (at boot from the EEPROM, later
            // from the editor)
            cc.send_idle_event(IdleEvent::NmeaBaud(cm.config.nmea_baud()));
        }

        // Special function Ids
        PersistenceId::UserProfile => cm.config.user_profile = item.to_u8(),

        // Not stored in EEPROM
        PersistenceId::VarioMode => cm.control.vario_mode = VarioMode::from(item.to_u8()),
        PersistenceId::DeleteAll => (),
        PersistenceId::DoNotStore => (),
        PersistenceId::LastItem => (),
    }
    Ok(())
}

pub fn persist_set(
    cc: &mut CoreController,
    cm: &mut CoreModel,
    variant: Variant,
    id: PersistenceId,
    echo: Echo,
) {
    let item = PersistenceItem::from_variant(id, variant);
    // a setting made now is no sign of a device that was in use before
    let restored = cm.control.items_restored;
    // silently ignore values like NAN, is_subnormal etc
    let _ = restore_item(cc, cm, item);
    cm.control.items_restored = restored;

    if id == PersistenceId::Glider {
        // When we choose a new glider polar, these settings are no longer usefull
        cc.send_idle_event(IdleEvent::ClearEepromItems(SPECIFIC_POLAR_SETTINGS));
    }

    if echo == Echo::Nmea || echo == Echo::NmeaAndCan {
        // Buffer NMEA datagrams in IndexSet
        let _ = cc.nmea_vals.insert(id); // send only last content
    }

    if echo == Echo::Can || echo == Echo::NmeaAndCan {
        // Queue directly to canbus
        let frame = cm.can_frame_sys_config(CanConfigId::from(id));
        if let Some(frame) = frame {
            let _ = cc.p_tx_frames.enqueue(frame);
        }
        // The sensorbox needs fraction of water ballast to be able to generate the NMEA sentence
        if id == PersistenceId::WaterBallast {
            let frame = cm
                .can_frame_sys_config(CanConfigId::WaterBallastFraction)
                .unwrap();
            let _ = cc.p_tx_frames.enqueue(frame);
        }
    }

    if (id as u16) < (PersistenceId::LastItem as u16) {
        let _ = cc.pers_vals.insert(id, item); // Buffer item to write it to EEPROM
    }

    cc.scheduler
        .after(crate::Timer::PersistSetting, PERSISTENCE_TIMEOUT.millis());

    // a glider setting changed here goes to the other display of a two-seater
    super::display_sync::on_local_change(cc, cm, id);
}

/// A glider setting received from the other display of a two-seater
/// (display_sync): taken and stored like a change made here, but not sent
/// on - neither to the other display nor to NMEA.
pub fn persist_received(cc: &mut CoreController, cm: &mut CoreModel, item: PersistenceItem) {
    let restored = cm.control.items_restored;
    // silently ignore values like NAN, is_subnormal etc
    let _ = restore_item(cc, cm, item);
    cm.control.items_restored = restored;
    if item.id == PersistenceId::Glider {
        cc.send_idle_event(IdleEvent::ClearEepromItems(SPECIFIC_POLAR_SETTINGS));
    }
    if (item.id as u16) < (PersistenceId::LastItem as u16) {
        let _ = cc.pers_vals.insert(item.id, item);
    }
    cc.scheduler
        .after(crate::Timer::PersistSetting, PERSISTENCE_TIMEOUT.millis());
}

pub fn send_can_config_frame(
    cm: &mut CoreModel,
    cc: &mut CoreController,
    config_id: CanConfigId,
    get_set: RemoteConfig,
) {
    // get command (also used to trigger commands in the sensor box) is sent
    // immediately, set command after a while, if no new changes are made
    match get_set {
        RemoteConfig::Get => {
            let frame = cm.can_frame_remote_config(config_id, get_set, Content::F32(None));
            if let Some(frame) = frame {
                let _ = cc.p_tx_frames.enqueue(frame);
            }
        }
        RemoteConfig::Set => set_remote_config(cm, cc, config_id, cm.control.editor.content),
    }
}

/// Write a configuration value into the sensor box
///
/// The value is frozen here: it is sent after PERSISTENCE_TIMEOUT (so that a
/// turning knob does not flood the CAN bus) from this copy, not from the editor
/// slot that may hold something else by then. The cache is updated right away,
/// so that the value preview of the menus shows the new value.
pub fn set_remote_config(
    cm: &mut CoreModel,
    cc: &mut CoreController,
    config_id: CanConfigId,
    content: Content,
) {
    match content {
        Content::F32(Some(val)) => cm.sensor.remote.set(config_id.remote_slot(), val),
        Content::Date(Some(date)) if config_id == CanConfigId::BlockHorizon => {
            cm.sensor.remote.block_horizon = Some(date)
        }
        _ => return,
    }
    cm.control.editor.set_pending = Some(config_id);
    cc.remote_val = Some((config_id, content));
    cc.scheduler
        .after(crate::Timer::PersistSetting, PERSISTENCE_TIMEOUT.millis());
}

/// Ask the sensor box for a configuration value
///
/// Requests are queued and sent one per 10 ms tick: the value preview of a
/// ring menu asks for up to a dozen values at once, which would overflow the
/// CAN transmit queue if sent in a burst (requests would be dropped silently).
/// `first` puts the request in front of the queue - used for the value the
/// editor is opened for.
pub fn queue_remote_get(cc: &mut CoreController, config_id: CanConfigId, first: bool) {
    if cc.remote_gets.iter().any(|id| *id == config_id) {
        if !first {
            return;
        }
        // move it to the front: rebuild without the duplicate
        let mut rest = heapless::Deque::<CanConfigId, REMOTE_GET_QUEUE>::new();
        while let Some(id) = cc.remote_gets.pop_front() {
            if id != config_id {
                let _ = rest.push_back(id);
            }
        }
        cc.remote_gets = rest;
    }
    if first {
        let _ = cc.remote_gets.push_front(config_id);
    } else {
        let _ = cc.remote_gets.push_back(config_id);
    }
}

/// The cached content of a sensor box configuration value
pub fn remote_content(cm: &CoreModel, config_id: CanConfigId) -> Content {
    if config_id == CanConfigId::BlockHorizon {
        Content::Date(cm.sensor.remote.block_horizon)
    } else {
        Content::F32(cm.sensor.remote.get(config_id.remote_slot()))
    }
}

/// A configuration value arrived from the sensor box (answer to a GET)
///
/// Stored in the cache; the open editor takes it over only if it is the value
/// the editor works on and the user has not changed it yet; the value preview
/// of the menu is refreshed.
pub fn remote_value_received(cm: &mut CoreModel, config_id: CanConfigId, content: Content) {
    let editing_this = cm.control.editor.target.remote_config_id() == Some(config_id);
    // a value the user has just changed or confirmed: the answer still shows
    // the old one (the SET goes out after the persistence timeout) - keep
    // ours, the GET sent after the SET brings the confirmation
    if (editing_this && cm.control.editor.touched)
        || cm.control.editor.set_pending == Some(config_id)
    {
        return;
    }
    match content {
        Content::F32(Some(val)) => cm.sensor.remote.set(config_id.remote_slot(), val),
        Content::Date(Some(date)) if config_id == CanConfigId::BlockHorizon => {
            cm.sensor.remote.block_horizon = Some(date)
        }
        _ => return,
    }
    if editing_this && cm.control.editor.mode != EditMode::Off {
        cm.control.editor.content = content;
    }
    crate::controller::menu::refresh_remote_value(cm, config_id);
}

pub fn send_can_test_func(cc: &mut CoreController, no: u8) {
    // send test function with parameter no to sensorbox
    let data = [0u8; 5];
    let frame = Frame::generic(
        CanFrame::empty_from_id(0)
            .push_u16(CanConfigId::CmdTestFunction as u16)
            .push_u8(no)
            .push_slice(&data),
        GenericId::SetSysSetting as u16,
    );
    let _ = cc.p_tx_frames.enqueue(frame);
}

pub fn delete_config(cm: &mut CoreModel, cc: &mut CoreController) {
    cc.send_idle_event(IdleEvent::ClearEepromItems(DELETE_CONFIG_LIST));
    cc.send_idle_event(IdleEvent::RestoreEepromItems);
    cm.reset();
}

pub fn factory_reset(cm: &mut CoreModel, cc: &mut CoreController) {
    let item = PersistenceItem::from_i8(PersistenceId::DeleteAll, 0);
    cc.send_idle_event(IdleEvent::SetEepromItem(item));
    cc.send_idle_event(IdleEvent::RestoreEepromItems);
    cm.reset();
}

pub fn user_profile(cm: &mut CoreModel, cc: &mut CoreController) {
    let item = PersistenceItem::from_u8(PersistenceId::UserProfile, cm.config.user_profile);
    cc.send_idle_event(IdleEvent::SetEepromItem(item));
    let item = PersistenceItem::from_bool(PersistenceId::ClubMode, cm.config.club_mode);
    cc.send_idle_event(IdleEvent::SetEepromItem(item));
    cc.send_idle_event(IdleEvent::RestoreEepromItems);
    cm.reset();
}

pub fn restore_stanard_proflile(cm: &mut CoreModel, cc: &mut CoreController) {
    cc.send_idle_event(IdleEvent::RestoreToSandardProfile);
    cc.send_idle_event(IdleEvent::RestoreEepromItems);
    cm.reset();
}

pub fn set_date_time(cm: &mut CoreModel, cc: &mut CoreController, date_time: DateTime) {
    if cm.control.alive_ticks < 10 || cm.sensor.gps_state == GpsState::NoGps {
        return;
    }
    let new_date = *date_time.date();
    let old_date = *cm.sensor.gps_date_time.date();
    cm.sensor.gps_date_time = date_time;

    if new_date != old_date {
        let data = new_date.as_array_4u8();
        let item = PersistenceItem::from_array_4u8(PersistenceId::Date, data);
        cc.send_idle_event(IdleEvent::SetEepromItem(item));

        if cm.config.club_mode {
            let item = PersistenceItem::from_u8(PersistenceId::UserProfile, 1);
            cc.send_idle_event(IdleEvent::SetEepromItem(item));
            cc.send_idle_event(IdleEvent::RestoreToSandardProfile);
            cc.send_idle_event(IdleEvent::RestoreEepromItems);
            cm.reset();
        }
    }
}

// This function is called by Timer::PersistSetting after a short period to avoid
// flooting EEPROM and NMEA Interface with to much data
pub fn store_persistence_ids(cm: &mut CoreModel, cc: &mut CoreController) {
    let mut items = Vec::<PersistenceItem, MAX_PERS_IDS>::new();

    // We must first copy the ids into a Vec, because we can't borrow cc twice
    for (_id, item) in cc.pers_vals.iter() {
        let _ = items.push(*item);
    }
    cc.pers_vals.clear();
    while let Some(item) = items.pop() {
        // Store data in EEPROM
        cc.send_idle_event(IdleEvent::SetEepromItem(item));
    }

    let mut ids = Vec::<PersistenceId, MAX_PERS_IDS>::new();
    // We must first copy the ids into a Vec, because we can't borrow cc twice
    for id in cc.nmea_vals.iter() {
        let _ = ids.push(*id);
    }
    cc.nmea_vals.clear();
    while let Some(id) = ids.pop() {
        // Send data via NMEA
        cc.nmea_send_config_data(id);
    }

    // send remote value if necessary - from the frozen copy, not from the editor
    if let Some((config_id, content)) = cc.remote_val {
        let frame = cm.can_frame_remote_config(config_id, RemoteConfig::Set, content);
        if let Some(frame) = frame {
            let _ = cc.p_tx_frames.enqueue(frame);
        }
        cc.remote_val = None;
        // the edit session is over for the cache: read back what the box
        // has stored, so that the value preview shows the truth
        if cm.control.editor.target.remote_config_id() == Some(config_id) {
            cm.control.editor.touched = false;
        }
        if cm.control.editor.set_pending == Some(config_id) {
            cm.control.editor.set_pending = None;
        }
        queue_remote_get(cc, config_id, true);
    }

    // We don't know, if someone has changed glider polar settings
    cc.recalc_glider(cm);
}

pub fn set_vario_mode(
    cm: &mut CoreModel,
    cc: &mut CoreController,
    vario_mode: VarioMode,
    source: VarioModeControl,
) {
    #[allow(clippy::if_same_then_else)]
    let vario_mode = if source == cm.control.vario_mode_control {
        vario_mode
    } else if source == VarioModeControl::Can
        && cm.control.vario_mode_control == VarioModeControl::Nmea
    {
        vario_mode
    } else {
        cm.control.vario_mode
    };

    if vario_mode != cm.control.vario_mode {
        let echo = match source {
            VarioModeControl::Auto | VarioModeControl::InputPin => Echo::NmeaAndCan,
            VarioModeControl::Nmea => Echo::Can,
            VarioModeControl::Can => Echo::Nmea,
        };
        persist_set(
            cc,
            cm,
            Variant::U8(vario_mode as u8),
            PersistenceId::VarioMode,
            echo,
        );
    }
}

#[cfg(test)]
mod remote_config_tests {
    //! Sensor box settings in the ring menu: the value preview asks the box
    //! for a dozen values at once. Before, every answer was written into the
    //! single editor slot (the editor showed whichever value came last, a
    //! turn of the knob clamped it into the range of the edited item - GNSS
    //! Config showed "1" while the box had 2), the burst of requests overran
    //! the CAN transmit queue (the editor's own request was dropped) and the
    //! SET frame was built from the editor slot half a second later, when it
    //! held something else. These tests walk through that use case, with the
    //! confirmation dialogs (save old -> new?, restart the box?) on top.
    use super::*;
    use crate::{
        basic_config::MAX_TX_FRAMES,
        controller::{
            editor,
            helpers::{can_ids::object_id, can_ids::sensor},
            menu,
        },
        model::{
            editable::{Content, REMOTE_RESTART, REMOTE_SAVE},
            menu::{Menu, MenuItem, MenuItemContent},
            DisplayActive, EditMode,
        },
        test_harness,
        utils::KeyEvent,
        CanFrame, Editable, Frame, GenericId,
    };
    use heapless::spsc::Consumer;

    type TxConsumer = Consumer<'static, Frame, MAX_TX_FRAMES>;

    /// a menu with all eleven sensor box values in one list - the burst of
    /// requests this test is about (the setup spreads them over Orientation,
    /// Pitot & Static, GNSS and Vario, the mechanism is the same)
    const fn item(e: Editable) -> MenuItem {
        MenuItem {
            content: MenuItemContent::EditItem(e),
            next_menu_idx: 0,
        }
    }
    const SENSOR_BOX_SETTINGS: Menu = Menu {
        name: "Box Values",
        level: 3,
        items: &[
            item(Editable::SensTiltRoll),
            item(Editable::SensTiltPitch),
            item(Editable::SensTiltYaw),
            item(Editable::PitotOffset),
            item(Editable::PitotSpan),
            item(Editable::QnhDelta),
            item(Editable::VarioTc),
            item(Editable::GnssConfig),
            item(Editable::AntBaselen),
            item(Editable::AntSlaveDown),
            item(Editable::AntSlaveRight),
            item(Editable::Return),
        ],
    };

    struct Clock(u16);
    impl Clock {
        /// advance the controller in 10 ms steps, draining the transmit queue
        fn run(
            &mut self,
            cc: &mut CoreController,
            cm: &mut CoreModel,
            rx: &mut TxConsumer,
            ms: u16,
            out: &mut heapless::Vec<Frame, 64>,
        ) {
            for _ in 0..ms / 10 {
                self.0 = self.0.wrapping_add(10);
                cc.tick_1ms(self.0, cm);
                while let Some(f) = rx.dequeue() {
                    let _ = out.push(f);
                }
            }
        }
    }

    fn answer(cm: &mut CoreModel, cc: &mut CoreController, id: CanConfigId, val: f32) {
        let frame = Frame::specific(
            CanFrame::empty_from_id(0).push_u32(id as u32).push_f32(val),
            sensor::CONFIG_VALUE,
            object_id::SENSOR,
        );
        cc.read_can_frame(cm, &frame);
    }

    /// (config id, is_set, value) of a SetSysSetting frame
    fn cfg(frame: &Frame) -> Option<(CanConfigId, bool, f32)> {
        if let Frame::Generic(g) = frame {
            if g.generic_id == GenericId::SetSysSetting as u16 {
                let id = CanConfigId::from(g.can_frame.read_u16(0));
                let is_set = g.can_frame.read_u8(2) == 1;
                return Some((id, is_set, g.can_frame.read_f32(4)));
            }
        }
        None
    }

    fn sets(frames: &[Frame]) -> heapless::Vec<(CanConfigId, f32), 8> {
        let mut v = heapless::Vec::new();
        for f in frames {
            if let Some((id, true, val)) = cfg(f) {
                let _ = v.push((id, val));
            }
        }
        v
    }

    fn gets(frames: &[Frame]) -> heapless::Vec<CanConfigId, 32> {
        let mut v = heapless::Vec::new();
        for f in frames {
            if let Some((id, false, _)) = cfg(f) {
                let _ = v.push(id);
            }
        }
        v
    }

    /// the user presses Enter: key handling queues the close callback, which
    /// the controller runs at the next tick (like on the device)
    fn press_enter(
        cm: &mut CoreModel,
        cc: &mut CoreController,
        rx: &mut TxConsumer,
        clock: &mut Clock,
    ) -> heapless::Vec<Frame, 64> {
        let mut key = KeyEvent::BtnEnc;
        editor::key_action(&mut key, cm, cc);
        let mut out = heapless::Vec::new();
        clock.run(cc, cm, rx, 20, &mut out);
        out
    }

    fn turn(cm: &mut CoreModel, cc: &mut CoreController, right: bool) {
        let mut key = if right {
            KeyEvent::Rotary2Right
        } else {
            KeyEvent::Rotary2Left
        };
        editor::key_action(&mut key, cm, cc);
    }

    fn choice(cm: &CoreModel) -> &str {
        if let Content::Enum(v) = &cm.control.editor.content {
            v.as_str()
        } else {
            ""
        }
    }

    /// the box value list open, Enter on `item`, the box has answered everything
    fn open_item(
        cm: &mut CoreModel,
        cc: &mut CoreController,
        rx: &mut TxConsumer,
        clock: &mut Clock,
        item: Editable,
    ) -> heapless::Vec<Frame, 64> {
        let mut out = heapless::Vec::new();
        clock.run(cc, cm, rx, 50, &mut out);
        out.clear();
        cm.config.display_active = DisplayActive::Menu;
        cm.control.menu_control.menu = &SENSOR_BOX_SETTINGS;
        menu::fill_values(cm, cc);
        editor::activate_editable(item, cm, cc);
        cm.control.editor.from_menu = true;
        clock.run(cc, cm, rx, 300, &mut out);
        answer(cm, cc, CanConfigId::SensTiltRoll, -1.51815);
        answer(cm, cc, CanConfigId::GnssConfig, 2.0);
        answer(cm, cc, CanConfigId::PitotOffset, -1.0);
        answer(cm, cc, CanConfigId::PitotSpan, 1.0);
        answer(cm, cc, CanConfigId::AntBaselen, 1.84);
        answer(cm, cc, CanConfigId::AntSlaveDown, -0.13);
        answer(cm, cc, CanConfigId::AntSlaveRight, 0.0);
        out
    }

    #[test]
    fn requests_are_queued_and_the_editor_keeps_its_own_value() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = Clock(0);
        // nothing known yet - the editor shows no value instead of a wrong one
        cm.config.display_active = DisplayActive::Menu;
        cm.control.menu_control.menu = &SENSOR_BOX_SETTINGS;
        menu::fill_values(&mut cm, &mut cc);
        editor::activate_editable(Editable::GnssConfig, &mut cm, &mut cc);
        assert!(cm.control.editor.mode == EditMode::CircleArea);
        assert!(matches!(cm.control.editor.content, Content::F32(None)));
        // the requests go out one per 10 ms, none is lost, the editor's first
        let mut out = heapless::Vec::new();
        clock.run(&mut cc, &mut cm, &mut rx, 300, &mut out);
        let g = gets(&out);
        assert_eq!(
            g.len(),
            11,
            "one request per item, no duplicates, none dropped"
        );
        assert!(
            g[0] == CanConfigId::GnssConfig,
            "the editor's own request goes first"
        );
        // the box answers, GNSS Config not last: editor, cache and list are right
        answer(&mut cm, &mut cc, CanConfigId::SensTiltRoll, -1.51815);
        answer(&mut cm, &mut cc, CanConfigId::GnssConfig, 2.0);
        answer(&mut cm, &mut cc, CanConfigId::AntBaselen, 1.84);
        answer(&mut cm, &mut cc, CanConfigId::AntSlaveDown, -0.13);
        assert!(matches!(cm.control.editor.content, Content::F32(Some(v)) if v == 2.0));
        assert_eq!(
            cm.sensor.remote.get(CanConfigId::AntBaselen.remote_slot()),
            Some(1.84)
        );
        assert_eq!(
            cm.control.menu_control.values[7].as_str().trim(),
            "Competition"
        );
        assert!(cm.control.menu_control.values[0].as_str().contains("-87.0"));
        assert!(cm.control.menu_control.values[9].as_str().contains("-0.13"));
        // one click up: Competition Pro (4); a late answer must not undo it
        turn(&mut cm, &mut cc, true);
        assert!(matches!(cm.control.editor.content, Content::F32(Some(v)) if v == 4.0));
        answer(&mut cm, &mut cc, CanConfigId::GnssConfig, 2.0);
        assert!(matches!(cm.control.editor.content, Content::F32(Some(v)) if v == 4.0));
        // turning the knob writes nothing to the box
        out.clear();
        clock.run(&mut cc, &mut cm, &mut rx, 700, &mut out);
        assert!(sets(&out).is_empty());
        assert_eq!(
            cm.sensor.remote.get(CanConfigId::GnssConfig.remote_slot()),
            Some(2.0)
        );
    }

    #[test]
    fn save_dialog_then_restart_dialog_for_gnss_config() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = Clock(0);
        open_item(&mut cm, &mut cc, &mut rx, &mut clock, Editable::GnssConfig);
        turn(&mut cm, &mut cc, true); // Competition (2) -> Competition Pro (4), 3 skipped
                                      // Enter: the save dialog shows GNSS Config, Competition > Competition Pro, "Save" preselected
        press_enter(&mut cm, &mut cc, &mut rx, &mut clock);
        assert!(cm.control.editor.target == Editable::RemoteSave);
        assert!(cm.control.editor.mode != EditMode::Off);
        assert_eq!(choice(&cm), REMOTE_SAVE);
        let p = cm.control.editor.pending.expect("pending value");
        assert!(p.target == Editable::GnssConfig);
        assert!(matches!(p.old, Content::F32(Some(v)) if v == 2.0));
        assert!(matches!(p.new, Content::F32(Some(v)) if v == 4.0));
        // still nothing sent, cache untouched
        let mut out = heapless::Vec::new();
        clock.run(&mut cc, &mut cm, &mut rx, 700, &mut out);
        assert!(sets(&out).is_empty());
        assert_eq!(
            cm.sensor.remote.get(CanConfigId::GnssConfig.remote_slot()),
            Some(2.0)
        );
        // Enter on Save: the value goes to the box, the restart dialog opens
        press_enter(&mut cm, &mut cc, &mut rx, &mut clock);
        assert!(cm.control.editor.target == Editable::RemoteRestart);
        assert_eq!(choice(&cm), REMOTE_RESTART);
        assert_eq!(
            cm.sensor.remote.get(CanConfigId::GnssConfig.remote_slot()),
            Some(4.0)
        );
        out.clear();
        clock.run(&mut cc, &mut cm, &mut rx, 700, &mut out);
        let s = sets(&out);
        assert_eq!(s.len(), 1);
        assert!(s[0].0 == CanConfigId::GnssConfig);
        assert_eq!(s[0].1, 4.0);
        // the SET is followed by a read back; a stale answer in between is ignored
        assert!(gets(&out).iter().any(|id| *id == CanConfigId::GnssConfig));
        answer(&mut cm, &mut cc, CanConfigId::GnssConfig, 4.0);
        assert_eq!(
            cm.sensor.remote.get(CanConfigId::GnssConfig.remote_slot()),
            Some(4.0)
        );
        // Enter on Restart: reset command, dialog closed, list shows the product
        let out = press_enter(&mut cm, &mut cc, &mut rx, &mut clock);
        assert!(cm.control.editor.mode == EditMode::Off);
        assert!(out
            .iter()
            .any(|f| matches!(cfg(f), Some((CanConfigId::CmdReset, false, _)))));
        assert_eq!(
            cm.control.menu_control.values[7].as_str().trim(),
            "Competition Pro"
        );
    }

    /// GNSS Config shows the products and skips the F9P + F9H set-up (3);
    /// a box that reports 3 keeps it as long as the knob is not turned
    #[test]
    fn gnss_config_offers_the_products() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = Clock(0);
        open_item(&mut cm, &mut cc, &mut rx, &mut clock, Editable::GnssConfig);
        let name = |cm: &CoreModel| {
            std::string::String::from(cm.control.editor.get_value_line(cm).as_str().trim())
        };
        assert_eq!(name(&cm), "Competition");
        turn(&mut cm, &mut cc, true);
        assert_eq!(name(&cm), "Competition Pro");
        turn(&mut cm, &mut cc, true);
        assert_eq!(name(&cm), "Competition Pro", "4 is the end");
        turn(&mut cm, &mut cc, false);
        assert_eq!(name(&cm), "Competition", "back over 3");
        turn(&mut cm, &mut cc, false);
        assert_eq!(name(&cm), "Essential");
        // a box with F9P + F9H
        cm.control.editor.content = Content::F32(Some(3.0));
        assert_eq!(name(&cm), "F9P + F9H");
        let mut k = crate::utils::KeyEvent::BtnEnc;
        crate::controller::editor::edit_f32_for_test(&mut cm, &mut cc, &mut k);
        assert_eq!(name(&cm), "F9P + F9H", "Enter keeps it");
    }

    /// Restart pressed right after Save: the SET must be on the bus before
    /// the reset command, otherwise the box restarts with the old value
    #[test]
    fn restart_right_after_save_sends_the_value_first() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = Clock(0);
        open_item(
            &mut cm,
            &mut cc,
            &mut rx,
            &mut clock,
            Editable::SensTiltPitch,
        );
        turn(&mut cm, &mut cc, false); // -87.0 -> -87.1 (roll answered, pitch not: use roll's item)
        press_enter(&mut cm, &mut cc, &mut rx, &mut clock);
        // the pitch value never arrived, so the editor could not be changed:
        // no dialog, plain close
        assert!(cm.control.editor.mode == EditMode::Off);
        // now with a known value
        answer(&mut cm, &mut cc, CanConfigId::SensTiltPitch, -0.164915);
        editor::activate_editable(Editable::SensTiltPitch, &mut cm, &mut cc);
        cm.control.editor.from_menu = true;
        turn(&mut cm, &mut cc, false);
        press_enter(&mut cm, &mut cc, &mut rx, &mut clock); // save dialog
        press_enter(&mut cm, &mut cc, &mut rx, &mut clock); // Save -> restart dialog
        assert!(cm.control.editor.target == Editable::RemoteRestart);
        let out = press_enter(&mut cm, &mut cc, &mut rx, &mut clock); // Restart at once
        let mut order = heapless::Vec::<(CanConfigId, bool), 8>::new();
        for f in out.iter() {
            if let Some((id, is_set, _)) = cfg(f) {
                if is_set || id == CanConfigId::CmdReset {
                    let _ = order.push((id, is_set));
                }
            }
        }
        assert_eq!(order.len(), 2, "SET and reset");
        assert!(order[0] == (CanConfigId::SensTiltPitch, true));
        assert!(order[1] == (CanConfigId::CmdReset, false));
    }

    #[test]
    fn discard_and_timeout_leave_the_box_untouched() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = Clock(0);
        open_item(&mut cm, &mut cc, &mut rx, &mut clock, Editable::AntBaselen);
        turn(&mut cm, &mut cc, true); // 1.84 -> 1.85
        press_enter(&mut cm, &mut cc, &mut rx, &mut clock);
        assert!(cm.control.editor.target == Editable::RemoteSave);
        turn(&mut cm, &mut cc, true); // Save -> Discard
        assert_eq!(choice(&cm), "Discard");
        press_enter(&mut cm, &mut cc, &mut rx, &mut clock);
        assert!(cm.control.editor.mode == EditMode::Off);
        assert!(cm.control.editor.pending.is_none());
        let mut out = heapless::Vec::new();
        clock.run(&mut cc, &mut cm, &mut rx, 700, &mut out);
        assert!(sets(&out).is_empty());
        assert_eq!(
            cm.sensor.remote.get(CanConfigId::AntBaselen.remote_slot()),
            Some(1.84)
        );
        assert!(cm.control.menu_control.values[8].as_str().contains("1.84"));

        // change again, but the editor times out without Enter: no dialog, no SET
        editor::activate_editable(Editable::AntBaselen, &mut cm, &mut cc);
        cm.control.editor.from_menu = true;
        turn(&mut cm, &mut cc, true);
        editor::close_edit_frame(&mut cm, &mut cc);
        assert!(cm.control.editor.mode == EditMode::Off);
        out.clear();
        clock.run(&mut cc, &mut cm, &mut rx, 700, &mut out);
        assert!(sets(&out).is_empty());
    }

    #[test]
    fn values_taken_over_live_get_no_restart_dialog() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = Clock(0);
        open_item(
            &mut cm,
            &mut cc,
            &mut rx,
            &mut clock,
            Editable::AntSlaveDown,
        );
        // -0.13 -> +0.20 with the big encoder (0.1 steps) and the small one
        let mut key = KeyEvent::Rotary1Right;
        for _ in 0..3 {
            editor::key_action(&mut key, &mut cm, &mut cc);
            key = KeyEvent::Rotary1Right;
        }
        for _ in 0..3 {
            turn(&mut cm, &mut cc, true);
        }
        press_enter(&mut cm, &mut cc, &mut rx, &mut clock);
        assert!(cm.control.editor.target == Editable::RemoteSave);
        press_enter(&mut cm, &mut cc, &mut rx, &mut clock);
        // no restart needed for the antenna geometry: back to the menu right away
        assert!(cm.control.editor.mode == EditMode::Off);
        assert!(cm.control.editor.pending.is_none());
        let mut out = heapless::Vec::new();
        clock.run(&mut cc, &mut cm, &mut rx, 700, &mut out);
        let s = sets(&out);
        assert_eq!(s.len(), 1);
        assert!(s[0].0 == CanConfigId::AntSlaveDown);
        assert!((s[0].1 - 0.20).abs() < 1e-5, "{}", s[0].1);
        assert!(!out
            .iter()
            .any(|f| matches!(cfg(f), Some((CanConfigId::CmdReset, _, _)))));
    }

    /// Level Flight: the box averages for a second; since sensor firmware
    /// 0.7.3 it applies the result only with Calc Orientation, which the
    /// frontend sends 2.5 s later, followed by requests for the tilt angles.
    /// An older box applies the fine tune itself and gets no calc command.
    #[test]
    fn level_flight_sends_calc_orientation_only_to_a_box_that_needs_it() {
        use crate::SwVersion;
        fn run(
            version: [u8; 4],
        ) -> (
            heapless::Vec<CanConfigId, 32>,
            heapless::Vec<CanConfigId, 32>,
        ) {
            let (mut cm, mut cc, mut rx) = test_harness!();
            let mut clock = Clock(0);
            let mut out = heapless::Vec::new();
            clock.run(&mut cc, &mut cm, &mut rx, 50, &mut out);
            out.clear();
            cm.sensor.sw_version = SwVersion { version };
            cm.config.display_active = DisplayActive::Menu;
            editor::activate_editable(Editable::CmdFineTuneOrientation, &mut cm, &mut cc);
            clock.run(&mut cc, &mut cm, &mut rx, 200, &mut out);
            let first = gets(&out);
            out.clear();
            clock.run(&mut cc, &mut cm, &mut rx, 3000, &mut out);
            (first, gets(&out))
        }
        // new box: fine tune at once, calc plus the three tilt requests later
        let (first, later) = run([0, 7, 6, 0]);
        assert!(first.as_slice() == [CanConfigId::CmdFineTuneCalibration]);
        assert!(
            later.as_slice()
                == [
                    CanConfigId::CmdCalcSensorOrientation,
                    CanConfigId::SensTiltRoll,
                    CanConfigId::SensTiltPitch,
                    CanConfigId::SensTiltYaw
                ]
        );
        // the first firmware that needs it
        let (_, later) = run([0, 7, 3, 0]);
        assert!(later.first() == Some(&CanConfigId::CmdCalcSensorOrientation));
        // old box and unknown version (no heartbeat yet): nothing follows
        for version in [[0, 7, 2, 0], [0, 0, 0, 0]] {
            let (first, later) = run(version);
            assert!(first.as_slice() == [CanConfigId::CmdFineTuneCalibration]);
            assert!(later.is_empty(), "{version:?}: {} frames", later.len());
        }
    }
}

/// Menu mode of a device that has none stored yet, decided once, 10 s after
/// the start (the EEPROM restore is long done then): a device that brought
/// settings of an older firmware keeps the full menu (Expert); a new device
/// or one after a factory reset starts in Basic and with the setup wizard.
/// The mode is stored right away, so the question never comes up again.
pub fn decide_menu_mode(cm: &mut CoreModel, cc: &mut CoreController) {
    if cm.control.mode_checked {
        return;
    }
    cm.control.mode_checked = true;
    let fresh = !cm.control.items_restored;
    cm.config.basic_mode = fresh;
    let item = PersistenceItem::from_u8(PersistenceId::MenuMode, if fresh { 0 } else { 1 });
    cc.send_idle_event(IdleEvent::SetEepromItem(item));
    if fresh {
        crate::controller::wizard::start(cm, cc);
    }
}

/// Basic / Expert chosen by the pilot (the club mode has its own item)
pub fn store_menu_mode(cm: &mut CoreModel, cc: &mut CoreController) {
    cm.control.mode_checked = true;
    let item = PersistenceItem::from_u8(
        PersistenceId::MenuMode,
        if cm.config.basic_mode { 0 } else { 1 },
    );
    cc.send_idle_event(IdleEvent::SetEepromItem(item));
}
