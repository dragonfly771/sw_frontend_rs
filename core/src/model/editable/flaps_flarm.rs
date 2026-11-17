//! Editables of the flap table, the FLARM interface, the SteFly canopy
//! flasher, the FLARM warnings and the demo (Stefly design, 27.10.2026)

use super::{
    CmdParams, Content, EditableFuncs, EnumParams, F32Params, ListParams, Params, StringParams,
};
#[allow(unused_imports)]
use crate::utils::FClamp;
use crate::{
    controller::flaps_ctl,
    model::{
        demo::DemoMode,
        flaps::{FlapSensorSource, MAX_FLAPS},
        flarm_cfg::{FlashRate, FlasherActive, FlasherType, WarnView, FLARM_BAUDS},
        NmeaProtocol, NMEA_BAUDS, NMEA_PROTOCOL_LARUS, NMEA_PROTOCOL_LXNAV,
    },
    persist,
    utils::{TString, Variant},
    CoreController, CoreModel, Echo, PersistenceId,
};
use tfmt::Convert;

// ------------------------------------------------------------------ helpers

fn enum_of(s: &str) -> Content {
    Content::Enum(TString::<16>::from_str(s))
}

fn cmd_of(s: &str) -> Content {
    Content::Command(TString::<16>::from_str(s))
}

fn variants(v: [&'static str; 5]) -> Params {
    Params::Enum(EnumParams { variants: v })
}

fn set_u8(cc: &mut CoreController, cm: &mut CoreModel, id: PersistenceId, v: u8) {
    persist::persist_set(cc, cm, Variant::U8(v), id, Echo::None);
}

/// index of the stage being edited, clamped to the table in effect
fn edit_idx(cm: &CoreModel) -> usize {
    let n = cm.flap_table().len();
    (cm.config.flap_edit as usize).min(n.saturating_sub(1))
}

/// the table in effect (own table or the AS 33 placeholder) for display
fn table(cm: &CoreModel) -> crate::model::flaps::FlapTable {
    cm.flap_table()
}

// ------------------------------------------------------------------ FLARM

pub struct FlarmStatus;
impl EditableFuncs for FlarmStatus {
    fn name() -> &'static str {
        "Status"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        cmd_of(cm.calculated.flarm.status_str())
    }
    fn params(cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(cm.calculated.flarm.status_str()),
        })
    }
}

pub struct FlarmBaud;
impl EditableFuncs for FlarmBaud {
    fn name() -> &'static str {
        "Baud Rate"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.config.flarm.baud_setting as i32)
    }
    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        match FLARM_BAUDS.get(idx.max(0) as usize) {
            Some(0) | None => {
                let _ = convert.write_str("Auto");
            }
            Some(b) => {
                let _ = convert.u32(*b);
            }
        }
    }
    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: FLARM_BAUDS.len() as i32 - 1,
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(idx) = content {
            let mut cfg = cm.config.flarm;
            cfg.baud_setting = idx.clamp(0, FLARM_BAUDS.len() as i32 - 1) as u8;
            persist::persist_set(
                cc,
                cm,
                Variant::U32(cfg.baud_word()),
                PersistenceId::FlarmBaud,
                Echo::None,
            );
        }
    }
}

// ------------------------------------------------------------------ NMEA port

/// protocol of the serial NMEA port: Larus sentences for XCSoar & Co, or
/// receive only on the PDA/PC port of an LXNAV main unit
pub struct NmeaProtocolSel;
impl EditableFuncs for NmeaProtocolSel {
    fn name() -> &'static str {
        "Protocol"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(cm.config.nmea_protocol.as_str())
    }
    fn params(_cm: &CoreModel) -> Params {
        variants([NMEA_PROTOCOL_LARUS, NMEA_PROTOCOL_LXNAV, "", "", ""])
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(v) = content {
            let p = match v.as_str() {
                NMEA_PROTOCOL_LXNAV => NmeaProtocol::Lxnav,
                _ => NmeaProtocol::Larus,
            };
            set_u8(cc, cm, PersistenceId::NmeaProtocol, p as u8);
        }
    }
}

/// baud rate of the NMEA port (4800 .. 115200, default 38400)
pub struct NmeaBaudSel;
impl EditableFuncs for NmeaBaudSel {
    fn name() -> &'static str {
        "Baud Rate"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.config.nmea_baud_idx as i32)
    }
    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        let idx = (idx.max(0) as usize).min(NMEA_BAUDS.len() - 1);
        let _ = convert.u32(NMEA_BAUDS[idx]);
    }
    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: NMEA_BAUDS.len() as i32 - 1,
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(idx) = content {
            let idx = idx.clamp(0, NMEA_BAUDS.len() as i32 - 1) as u8;
            set_u8(cc, cm, PersistenceId::NmeaBaud, idx);
        }
    }
}

pub struct FlasherTypeSel;
impl EditableFuncs for FlasherTypeSel {
    fn name() -> &'static str {
        "Flasher Type"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(cm.config.flarm.flasher_type.as_str())
    }
    fn params(_cm: &CoreModel) -> Params {
        variants(["Off", "FLARM", "Airspeed", "", ""])
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(v) = content {
            let t = match v.as_str() {
                "FLARM" => FlasherType::Flarm,
                "Airspeed" => FlasherType::Airspeed,
                _ => FlasherType::Off,
            };
            set_u8(cc, cm, PersistenceId::FlasherType, t as u8);
        }
    }
}

pub struct FlasherActiveSel;
impl EditableFuncs for FlasherActiveSel {
    fn name() -> &'static str {
        "Active"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(cm.config.flarm.flasher_active.as_str())
    }
    fn params(_cm: &CoreModel) -> Params {
        variants(["Airborne", "Warning only", "", "", ""])
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(v) = content {
            let a = match v.as_str() {
                "Warning only" => FlasherActive::WarningOnly,
                _ => FlasherActive::Airborne,
            };
            set_u8(cc, cm, PersistenceId::FlasherActive, a as u8);
        }
    }
}

pub struct FlashRateSel;
impl EditableFuncs for FlashRateSel {
    fn name() -> &'static str {
        "Flash Rate"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(cm.config.flarm.flash_rate.as_str())
    }
    fn params(_cm: &CoreModel) -> Params {
        variants(["Low", "Medium", "High", "", ""])
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(v) = content {
            let r = match v.as_str() {
                "Low" => FlashRate::Low,
                "High" => FlashRate::High,
                _ => FlashRate::Medium,
            };
            set_u8(cc, cm, PersistenceId::FlashRate, r as u8);
        }
    }
}

pub struct FlasherOnSpeed;
impl EditableFuncs for FlasherOnSpeed {
    fn name() -> &'static str {
        "On Speed"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let v = cm
            .config
            .unit_horizontal_speed
            .as_f32(crate::Speed::from_km_h(cm.config.flarm.on_speed_kmh));
        Content::F32(Some(v))
    }
    fn params(cm: &CoreModel) -> Params {
        let u = cm.config.unit_horizontal_speed;
        Params::F32(F32Params {
            min: u.as_f32(crate::Speed::from_km_h(20.0)),
            max: u.as_f32(crate::Speed::from_km_h(150.0)),
            small_inc: u.as_f32(crate::Speed::from_km_h(5.0)),
            big_inc: u.as_f32(crate::Speed::from_km_h(20.0)),
            dec_places: 0,
            unit: u.as_str(),
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(v)) = content {
            let kmh = cm.config.unit_horizontal_speed.as_speed(v).to_km_h();
            persist::persist_set(
                cc,
                cm,
                Variant::F32(kmh),
                PersistenceId::FlasherOnSpeed,
                Echo::None,
            );
        }
    }
}

pub struct FlasherTest;
impl EditableFuncs for FlasherTest {
    fn name() -> &'static str {
        "Flash Test"
    }
    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        // sketch skizze_flarm_menue.png: the row offers "Start" (the test
        // runs 30 s)
        cmd_of("Start")
    }
    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str("Start"),
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        cc.flarm.flasher.start_test(cm.control.now_ms);
    }
}

pub struct WarnViewSel;
impl EditableFuncs for WarnViewSel {
    fn name() -> &'static str {
        "Warning View"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(cm.config.flarm.warn_view.as_str())
    }
    fn params(_cm: &CoreModel) -> Params {
        variants(["Core", "Full Screen", "Off", "", ""])
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(v) = content {
            let w = match v.as_str() {
                "Full Screen" => WarnView::FullScreen,
                "Off" => WarnView::Off,
                _ => WarnView::Core,
            };
            set_u8(cc, cm, PersistenceId::FlarmWarnView, w as u8);
        }
    }
}

pub struct WarnLevelSel;
impl EditableFuncs for WarnLevelSel {
    fn name() -> &'static str {
        "From Level"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(match cm.config.flarm.warn_level {
            2 => "2",
            3 => "3",
            _ => "1",
        })
    }
    fn params(_cm: &CoreModel) -> Params {
        variants(["1", "2", "3", "", ""])
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(v) = content {
            let l = match v.as_str() {
                "2" => 2,
                "3" => 3,
                _ => 1,
            };
            set_u8(cc, cm, PersistenceId::FlarmWarnLevel, l);
        }
    }
}

pub struct HeadOnSel;
impl EditableFuncs for HeadOnSel {
    fn name() -> &'static str {
        "Head-on View"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(if cm.config.flarm.head_on { "On" } else { "Off" })
    }
    fn params(_cm: &CoreModel) -> Params {
        variants(["On", "Off", "", "", ""])
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(v) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Bool(v.as_str() == "On"),
                PersistenceId::FlarmHeadOn,
                Echo::None,
            );
        }
    }
}

// ------------------------------------------------------------------ demo

pub struct DemoSel;
impl EditableFuncs for DemoSel {
    fn name() -> &'static str {
        "Demo"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(cm.control.demo.as_str())
    }
    fn params(_cm: &CoreModel) -> Params {
        if !crate::model::flarm_cfg::FLARM_FEATURE {
            return variants(["Off", "Flaps", "", "", ""]);
        }
        variants(["Off", "Flaps", "FLARM", "All", ""])
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(v) = content {
            let m = match v.as_str() {
                "Flaps" => DemoMode::Flaps,
                "FLARM" => DemoMode::Flarm,
                "All" => DemoMode::All,
                _ => DemoMode::Off,
            };
            let set = flaps_ctl::set_demo(cm, m);
            cc.flarm.demo_changed(set.flarm(), cm.control.now_ms);
        }
    }
}

// ------------------------------------------------------------------ flaps

pub struct FlapSensorSel;
impl EditableFuncs for FlapSensorSel {
    fn name() -> &'static str {
        "Flap Sensor"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(table(cm).sensor.as_str())
    }
    fn params(_cm: &CoreModel) -> Params {
        variants(["None", "LARUS", "", "", ""])
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(v) = content {
            flaps_ctl::adopt_template(cm, cc);
            cm.config.flaps.sensor = if v.as_str() == "LARUS" {
                FlapSensorSource::Larus
            } else {
                FlapSensorSource::None
            };
            cc.flap_dirty |= flaps_ctl::DIRTY_TABLE;
        }
    }
}

/// live value of the flap sensor (read only)
pub struct FlapSensorValue;
impl EditableFuncs for FlapSensorValue {
    fn name() -> &'static str {
        "Sensor"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        cmd_of(flaps_ctl::sensor_str(cm).as_str())
    }
    fn params(cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(flaps_ctl::sensor_str(cm).as_str()),
        })
    }
}

/// circling stage, the list shows the stage names (see Editable::content_as_str)
pub struct FlapCircling;
impl EditableFuncs for FlapCircling {
    fn name() -> &'static str {
        "Circling"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.flap_table().circling as i32)
    }
    fn params(cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: (cm.flap_table().len() as i32 - 1).max(0),
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(idx) = content {
            flaps_ctl::adopt_template(cm, cc);
            let n = cm.config.flaps.len() as i32;
            cm.config.flaps.circling = idx.clamp(0, (n - 1).max(0)) as u8;
            cc.flap_dirty |= flaps_ctl::DIRTY_TABLE;
        }
    }
}

pub struct FlapAdd;
impl EditableFuncs for FlapAdd {
    fn name() -> &'static str {
        "Add Stage"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        // no value in the list; the editor reports a full table
        cmd_of(if table(cm).len() >= MAX_FLAPS {
            "table full"
        } else {
            ""
        })
    }
    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str("added"),
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        flaps_ctl::adopt_template(cm, cc);
        if cm.config.flaps.add_stage() {
            let n = cm.config.flaps.len();
            cc.flap_dirty |= flaps_ctl::DIRTY_TABLE;
            flaps_ctl::mark_stage(cc, n - 1);
            if n >= 2 {
                flaps_ctl::mark_stage(cc, n - 2);
            }
            crate::controller::menu::fill_values(cm, cc);
        }
    }
}

pub struct FlapName;
impl EditableFuncs for FlapName {
    fn name() -> &'static str {
        "Name"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let i = edit_idx(cm);
        Content::String(TString::<12>::from_str(table(cm).name(i)))
    }
    fn params(cm: &CoreModel) -> Params {
        let i = edit_idx(cm);
        Params::String(StringParams {
            content: TString::<16>::from_str(table(cm).name(i)),
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::String(s) = content {
            flaps_ctl::adopt_template(cm, cc);
            let i = edit_idx(cm);
            if let Some(st) = cm.config.flaps.stages.get_mut(i) {
                st.set_name(s.as_str());
                flaps_ctl::mark_stage(cc, i);
            }
        }
    }
}

pub struct FlapUpTo;
impl EditableFuncs for FlapUpTo {
    fn name() -> &'static str {
        "Up to"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let i = edit_idx(cm);
        let t = table(cm);
        if i + 1 >= t.len() {
            // the fastest stage has no upper limit
            return Content::F32(None);
        }
        let kmh = t.stages[i].up_to_kmh as f32;
        Content::F32(Some(
            cm.config
                .unit_horizontal_speed
                .as_f32(crate::Speed::from_km_h(kmh)),
        ))
    }
    fn params(cm: &CoreModel) -> Params {
        let i = edit_idx(cm);
        let (lo, hi) = table(cm).up_to_limits(i);
        let u = cm.config.unit_horizontal_speed;
        Params::F32(F32Params {
            min: u.as_f32(crate::Speed::from_km_h(lo as f32)),
            max: u.as_f32(crate::Speed::from_km_h(hi as f32)),
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit: u.as_str(),
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(v)) = content {
            flaps_ctl::adopt_template(cm, cc);
            let i = edit_idx(cm);
            let (lo, hi) = cm.config.flaps.up_to_limits(i);
            let kmh = cm.config.unit_horizontal_speed.as_speed(v).to_km_h();
            let kmh = (kmh + 0.5).fclamp(lo as f32, hi as f32) as u16;
            if i + 1 < cm.config.flaps.len() {
                cm.config.flaps.stages[i].up_to_kmh = kmh;
                flaps_ctl::mark_stage(cc, i);
            }
        }
    }
}

/// The fastest stage has no upper limit - its speed is where it begins:
/// "From" edits the "Up to" of the stage before it (the only speed of the
/// stage that can be set; before 29.10.2026 its "Up to" showed "-" and the
/// knob did nothing)
pub struct FlapFrom;
impl EditableFuncs for FlapFrom {
    fn name() -> &'static str {
        "From"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let i = edit_idx(cm);
        let t = table(cm);
        if i == 0 || i >= t.len() {
            return Content::F32(None);
        }
        let kmh = t.range(i).0 as f32;
        Content::F32(Some(
            cm.config
                .unit_horizontal_speed
                .as_f32(crate::Speed::from_km_h(kmh)),
        ))
    }
    fn params(cm: &CoreModel) -> Params {
        let i = edit_idx(cm).max(1);
        let (lo, hi) = table(cm).up_to_limits(i - 1);
        let u = cm.config.unit_horizontal_speed;
        Params::F32(F32Params {
            min: u.as_f32(crate::Speed::from_km_h(lo as f32)),
            max: u.as_f32(crate::Speed::from_km_h(hi as f32)),
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit: u.as_str(),
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(v)) = content {
            flaps_ctl::adopt_template(cm, cc);
            let i = edit_idx(cm);
            if i == 0 || i >= cm.config.flaps.len() {
                return;
            }
            let (lo, hi) = cm.config.flaps.up_to_limits(i - 1);
            let kmh = cm.config.unit_horizontal_speed.as_speed(v).to_km_h();
            cm.config.flaps.stages[i - 1].up_to_kmh =
                (kmh + 0.5).fclamp(lo as f32, hi as f32) as u16;
            flaps_ctl::mark_stage(cc, i - 1);
        }
    }
}

pub struct FlapPosition;
impl EditableFuncs for FlapPosition {
    fn name() -> &'static str {
        "Position"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let i = edit_idx(cm);
        // no stored position yet: start from the sensor (or 0) so it can be
        // set by hand as well
        let p = table(cm).stages[i].position;
        let live = cm.sensor.flap_sensor_deg;
        Content::F32(Some(if !p.is_nan() {
            p
        } else if !live.is_nan() {
            live
        } else {
            0.0
        }))
    }
    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: -180.0,
            max: 360.0,
            small_inc: 0.1,
            big_inc: 5.0,
            dec_places: 1,
            unit: "°",
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(v)) = content {
            flaps_ctl::adopt_template(cm, cc);
            let i = edit_idx(cm);
            cm.config.flaps.stages[i].position = v;
            flaps_ctl::mark_stage(cc, i);
        }
    }
}

/// store the live sensor value as position of this stage
pub struct FlapSetPos;
impl EditableFuncs for FlapSetPos {
    fn name() -> &'static str {
        "Set Position"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        cmd_of(flaps_ctl::sensor_str(cm).as_str())
    }
    fn params(cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(flaps_ctl::sensor_str(cm).as_str()),
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        let v = cm.sensor.flap_sensor_deg;
        if !v.is_nan() {
            flaps_ctl::adopt_template(cm, cc);
            let i = edit_idx(cm);
            cm.config.flaps.stages[i].position = v;
            flaps_ctl::mark_stage(cc, i);
        }
    }
}

pub struct FlapDelete;
impl EditableFuncs for FlapDelete {
    fn name() -> &'static str {
        "Delete Stage"
    }
    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        cmd_of("")
    }
    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str("deleted"),
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        flaps_ctl::adopt_template(cm, cc);
        let i = edit_idx(cm);
        let n_before = cm.config.flaps.len();
        if cm.config.flaps.delete_stage(i) {
            cc.flap_dirty |= flaps_ctl::DIRTY_TABLE;
            // the new fastest stage lost its upper limit: write it as well
            for k in i.saturating_sub(1)..n_before {
                flaps_ctl::mark_stage(cc, k);
            }
        }
    }
}
