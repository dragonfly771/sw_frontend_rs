use super::{CmdParams, Content, EditableFuncs, EnumParams, F32Params, Params, COMMAND_SENT};
use crate::{
    controller::{
        persist::{queue_remote_get, remote_content, send_can_config_frame},
        CanConfigId, RemoteConfig,
    },
    model::SystemState,
    persist::send_can_test_func,
    utils::TString,
    CoreController, CoreModel, Date, SwVersion,
};

/// Content of a value that lives in the sensor box: the cached value (None
/// until the first answer arrived), and a GET request is queued so that the
/// cache is refreshed. Called for the editor as well as for the value preview
/// of the menus.
fn remote(cm: &mut CoreModel, cc: &mut CoreController, config_id: CanConfigId) -> Content {
    // the editor asks with its target set - that request goes first
    let first = cm.control.editor.target.remote_config_id() == Some(config_id);
    queue_remote_get(cc, config_id, first);
    remote_content(cm, config_id)
}

/// A changed sensor box value stays in the editor until the user confirms it:
/// Enter opens the "save old -> new?" dialog (controller/editor.rs,
/// remote_confirm_step), only then the value goes to the box. Turning the
/// knob therefore writes nothing here.
fn keep_in_editor(_cm: &mut CoreModel, _cc: &mut CoreController, _content: Content) {}

pub struct SensTiltRoll;
const SENS_TILT_PARAMS: Params = Params::F32(F32Params {
    min: -179.0,
    max: 359.0,
    small_inc: 0.1,
    big_inc: 1.0,
    dec_places: 1,
    unit: "°",
});

impl EditableFuncs for SensTiltRoll {
    fn name() -> &'static str {
        "Tilt Roll"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::SensTiltRoll)
    }

    fn params(_cm: &CoreModel) -> Params {
        SENS_TILT_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

pub struct SensTiltPitch;
impl EditableFuncs for SensTiltPitch {
    fn name() -> &'static str {
        "Tilt Pitch"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::SensTiltPitch)
    }

    fn params(_cm: &CoreModel) -> Params {
        SENS_TILT_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

pub struct SensTiltYaw;
impl EditableFuncs for SensTiltYaw {
    fn name() -> &'static str {
        "Tilt Yaw"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::SensTiltYaw)
    }

    fn params(_cm: &CoreModel) -> Params {
        SENS_TILT_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

pub struct PitotOffset;
const PA_PARAMS: Params = Params::F32(F32Params {
    min: -50.0,
    max: 50.0,
    small_inc: 0.1,
    big_inc: 1.0,
    dec_places: 1,
    unit: "Pa",
});

impl EditableFuncs for PitotOffset {
    fn name() -> &'static str {
        "Pitot Offset"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::PitotOffset)
    }

    fn params(_cm: &CoreModel) -> Params {
        PA_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

/// Measure Offset: the automatic pitot offset measurement
/// (controller/pitot_cal.rs). The editor stays open while it runs and shows
/// the progress; the result is offered in the save dialog of the box values.
pub struct PitotOffsetMeasure;
impl EditableFuncs for PitotOffsetMeasure {
    fn name() -> &'static str {
        "Measure Offset"
    }

    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Command(TString::<16>::from_str(""))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(""),
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        crate::controller::pitot_cal::start(cm, cc);
    }
}

pub struct PitotSpan;
impl EditableFuncs for PitotSpan {
    fn name() -> &'static str {
        "Pitot Span"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::PitotSpan)
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 0.7,
            max: 1.3,
            small_inc: 0.001,
            big_inc: 0.01,
            dec_places: 3,
            unit: "",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

pub struct QnhDelta;
impl EditableFuncs for QnhDelta {
    fn name() -> &'static str {
        "QNH Delta"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::QnhDelta)
    }

    fn params(_cm: &CoreModel) -> Params {
        PA_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

/// Time constant of the vario filter in the sensor box (ini: Vario_TC).
/// The box accepts 1..100 s, but the needle is meant to follow the air:
/// everything above a few seconds only hides what happens, so the editor
/// stops at 5 s.
pub struct VarioTc;
const TC_PARAMS: Params = Params::F32(F32Params {
    min: 1.0,
    max: 5.0,
    small_inc: 0.1,
    big_inc: 1.0,
    dec_places: 1,
    unit: "s",
});

impl EditableFuncs for VarioTc {
    fn name() -> &'static str {
        "Vario Filter"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::VarioTc)
    }

    fn params(_cm: &CoreModel) -> Params {
        TC_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

/// Averaging time of the climb rate the sensor box computes itself - the
/// value the frontend shows with Avg Source = Sensorbox
pub struct VarioIntTc;
impl EditableFuncs for VarioIntTc {
    fn name() -> &'static str {
        "Avg Time (Box)"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::VarioIntTc)
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 5.0,
            max: 120.0,
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit: "s",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

/// Time constant of the instantaneous wind of the sensor box (wind arrow)
pub struct WindTc;
impl EditableFuncs for WindTc {
    fn name() -> &'static str {
        "Wind Time"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::WindTc)
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 1.0,
            max: 60.0,
            small_inc: 1.0,
            big_inc: 5.0,
            dec_places: 0,
            unit: "s",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

/// Time constant of the averaged wind of the sensor box
pub struct MeanWindTc;
impl EditableFuncs for MeanWindTc {
    fn name() -> &'static str {
        "Mean Wind Time"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::MeanWindTc)
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 10.0,
            max: 600.0,
            small_inc: 5.0,
            big_inc: 30.0,
            dec_places: 0,
            unit: "s",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

pub struct GnssConfig;
impl EditableFuncs for GnssConfig {
    fn name() -> &'static str {
        "GNSS Config"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::GnssConfig)
    }

    fn params(_cm: &CoreModel) -> Params {
        // GNSS_CONFIG of the box: 1 single M9N, 2 two F9P, 3 F9P + F9H
        // heading module, 4 X20D (sw_sensor configuration/README.md). The
        // editor offers the products 1 / 2 / 4 and skips 3 (shown if a box
        // reports it) - controller/editor.rs edit_f32_content
        Params::F32(F32Params {
            min: 1.0,
            max: 4.0,
            small_inc: 1.0,
            big_inc: 1.0,
            dec_places: 0,
            unit: "",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

/// the LARUS product that goes with a GNSS_CONFIG value of the box
pub fn gnss_config_name(v: f32) -> &'static str {
    match (v + 0.5) as i32 {
        1 => "Essential",
        2 => "Competition",
        3 => "F9P + F9H",
        4 => "Competition Pro",
        _ => "-",
    }
}

pub struct AntBaselen;
const ANT_PARAMS: Params = Params::F32(F32Params {
    min: -10.0,
    max: 10.0,
    small_inc: 0.01,
    big_inc: 0.1,
    dec_places: 2,
    unit: "m",
});

impl EditableFuncs for AntBaselen {
    fn name() -> &'static str {
        // ANT_BASELEN of the box: distance front to rear antenna along the
        // fuselage (the box's "secondary" antenna is the front one: its
        // heading is the vector main -> secondary, NAV_Algorithms/AHRS.cpp)
        "Antenna Distance"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::AntBaselen)
    }

    fn params(_cm: &CoreModel) -> Params {
        ANT_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

pub struct AntSlaveDown;
impl EditableFuncs for AntSlaveDown {
    fn name() -> &'static str {
        // ANT_SLAVE_DOWN: positive = the front (secondary) antenna sits
        // lower than the rear one - value and sign unchanged, only the name
        "Front Ant. Lower"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::AntSlaveDown)
    }

    fn params(_cm: &CoreModel) -> Params {
        ANT_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

pub struct AntSlaveRight;
impl EditableFuncs for AntSlaveRight {
    fn name() -> &'static str {
        // ANT_SLAVE_RIGHT: positive = the front (secondary) antenna sits
        // further right (seen in flight direction) than the rear one
        "Front Ant. Right"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::AntSlaveRight)
    }

    fn params(_cm: &CoreModel) -> Params {
        ANT_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

pub struct BlockHorizon;
impl EditableFuncs for BlockHorizon {
    fn name() -> &'static str {
        "Block Horizon"
    }

    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        remote(cm, cc, CanConfigId::BlockHorizon)
    }

    fn params(cm: &CoreModel) -> Params {
        if cm.control.system_state == SystemState::CanAndGpsOk {
            let min = *cm.sensor.gps_date_time.date();
            let mut max = min;
            max.add_days(21);
            Params::Date(super::DateParams {
                min,
                max,
                small_inc_plus: 1,
                small_inc_minus: 0,
                big_inc_plus: 10,
                big_inc_minus: 0,
                is_active: true,
            })
        } else {
            Params::Date(super::DateParams {
                min: Date::new(2000, 1, 1),
                max: Date::new(2000, 1, 1),
                small_inc_plus: 0,
                small_inc_minus: 0,
                big_inc_plus: 0,
                big_inc_minus: 0,
                is_active: false,
            })
        }
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        keep_in_editor(cm, cc, content);
    }
}

pub struct CmdMeas1;

impl EditableFuncs for CmdMeas1 {
    fn name() -> &'static str {
        "Left Wing Down"
    }

    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Command(TString::<16>::from_str(COMMAND_SENT))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(COMMAND_SENT),
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        send_can_config_frame(cm, cc, crate::CanConfigId::CmdMeasure1, RemoteConfig::Get);
        cm.control.cal_done |= 1 << 0;
    }
}

pub struct CmdMeas2;
impl EditableFuncs for CmdMeas2 {
    fn name() -> &'static str {
        "Right Wing Down"
    }

    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Command(TString::<16>::from_str(COMMAND_SENT))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(COMMAND_SENT),
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        send_can_config_frame(cm, cc, crate::CanConfigId::CmdMeasure2, RemoteConfig::Get);
        cm.control.cal_done |= 1 << 1;
    }
}

pub struct CmdMeas3;
impl EditableFuncs for CmdMeas3 {
    fn name() -> &'static str {
        "Wings Straight"
    }

    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Command(TString::<16>::from_str(COMMAND_SENT))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(COMMAND_SENT),
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        send_can_config_frame(cm, cc, crate::CanConfigId::CmdMeasure3, RemoteConfig::Get);
        cm.control.cal_done |= 1 << 2;
    }
}

pub struct CmdCalcOrientation;
impl EditableFuncs for CmdCalcOrientation {
    fn name() -> &'static str {
        "Calc Orientation"
    }

    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Command(TString::<16>::from_str(COMMAND_SENT))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(COMMAND_SENT),
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        send_can_config_frame(
            cm,
            cc,
            crate::CanConfigId::CmdCalcSensorOrientation,
            RemoteConfig::Get,
        );
        // a calculation without the three measurements does not count
        if cm.control.cal_done & 0b111 == 0b111 {
            cm.control.cal_done |= 1 << 3;
        }
    }
}

/// Level Flight: the neutral attitude. The box averages its acceleration
/// vector for one second (100 samples) and takes that direction as "down"
/// - so the glider has to fly straight, wings level, at constant normal
/// speed, or stand in the weighing attitude on the ground. Since sensor
/// firmware 0.7.3 the box applies the result only with the next
/// "Calc Orientation" (sw_algorithms_lib 06.05.2026); older boxes apply
/// it themselves. For a box that needs it the frontend sends the calc
/// command 2.5 s later (controller tick), so one press does the job on
/// every box and the tilt values in the list refresh afterwards.
pub struct CmdFineTuneOrientation;

/// first sensor box firmware whose fine tune waits for "Calc Orientation"
pub const LEVEL_FLIGHT_NEEDS_CALC_FROM: SwVersion = SwVersion {
    version: [0, 7, 3, 0],
};
/// 100 ms ticks between Level Flight and the automatic Calc Orientation:
/// the box averages for 1 s after it has received the command
pub const LEVEL_FLIGHT_CALC_TICKS: u8 = 25;

impl EditableFuncs for CmdFineTuneOrientation {
    fn name() -> &'static str {
        "Level Flight"
    }

    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Command(TString::<16>::from_str(COMMAND_SENT))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(COMMAND_SENT),
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        send_can_config_frame(
            cm,
            cc,
            crate::CanConfigId::CmdFineTuneCalibration,
            RemoteConfig::Get,
        );
        cm.control.cal_done |= 1 << 4;
        // an unknown version (no heartbeat yet) gets no automatic calc -
        // the pilot can still press Calc Orientation himself
        let version = cm.sensor.sw_version;
        cm.control.level_flight_calc = if version >= LEVEL_FLIGHT_NEEDS_CALC_FROM {
            LEVEL_FLIGHT_CALC_TICKS
        } else {
            0
        };
    }
}

pub struct CmdResetSensorbox;
impl EditableFuncs for CmdResetSensorbox {
    fn name() -> &'static str {
        "Reset Sensorbox"
    }

    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Command(TString::<16>::from_str(COMMAND_SENT))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(COMMAND_SENT),
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        send_can_config_frame(cm, cc, crate::CanConfigId::CmdReset, RemoteConfig::Get);
    }
}

pub struct CmdTestFunction;

impl EditableFuncs for CmdTestFunction {
    fn name() -> &'static str {
        "Test Function"
    }

    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Command(TString::<16>::from_str(COMMAND_SENT))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(COMMAND_SENT),
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        send_can_test_func(cc, cm.sensor.test_no);
    }
}

pub struct CmdTestFunctionNumber;

impl EditableFuncs for CmdTestFunctionNumber {
    fn name() -> &'static str {
        "Test Parameter"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.sensor.test_no as f32))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 0.0,
            max: 9.0,
            small_inc: 1.0,
            big_inc: 1.0,
            dec_places: 0,
            unit: "",
        })
    }

    fn set_content(cm: &mut CoreModel, _cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            cm.sensor.test_no = val as u8;
        }
    }
}

pub const REMOTE_SAVE: &str = "Save";
pub const REMOTE_DISCARD: &str = "Discard";
pub const REMOTE_RESTART: &str = "Restart";
pub const REMOTE_LATER: &str = "Later";

/// Dialog after editing a sensor box value: shows the item, old and new value
/// (sub lines of the ring editor) and asks whether to save
pub struct RemoteSave;
impl EditableFuncs for RemoteSave {
    fn name() -> &'static str {
        "Save Setting"
    }

    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Enum(TString::<16>::from_str(REMOTE_SAVE))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [REMOTE_SAVE, REMOTE_DISCARD, "", "", ""],
        })
    }

    // the decision is taken on Enter (controller/editor.rs)
    fn set_content(_cm: &mut CoreModel, _cc: &mut CoreController, _content: Content) {}
}

/// Dialog after saving a value the box reads only at start-up: restart now?
pub struct RemoteRestart;
impl EditableFuncs for RemoteRestart {
    fn name() -> &'static str {
        "Restart Sensorbox"
    }

    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Enum(TString::<16>::from_str(REMOTE_RESTART))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [REMOTE_RESTART, REMOTE_LATER, "", "", ""],
        })
    }

    fn set_content(_cm: &mut CoreModel, _cc: &mut CoreController, _content: Content) {}
}
