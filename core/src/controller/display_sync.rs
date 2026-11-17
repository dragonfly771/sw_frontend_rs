//! Two displays, one glider: the aircraft settings are the same on both
//!
//! In a two-seater the front and the rear display sit on the same CAN bus.
//! MacCready, water ballast, bugs, QNH and pilot weight have always been
//! shared (SetSysSetting 1..5). This module shares the rest of what describes
//! the glider, so that both pilots see the same speed to fly, the same POT
//! value and the same flap band:
//!
//! - glider type and polar (empty mass, max. ballast, reference weight, the
//!   three polar points)
//! - switch between circling and cruise (mode and switch speed)
//! - flap table (stages, names, switch speeds, sensor positions)
//! - gust filter (also in circling), POT yes / no bar factor
//! - volumes (circling, cruise, warning): often only one display has a
//!   speaker, the pilot in the other seat still sets the volume
//!
//! Brightness and pages stay with each display.
//!
//! Gear alarm: the gear and airbrake switches are wired to one display only.
//! That display tells the other one about the alarm (`SYNC_GEAR`), the alarm
//! box shows on both, and a push on either display confirms it on both.
//!
//! Transport: SetSysSetting frames with config id `SYNC_BASE + PersistenceId`,
//! payload byte 0 = `SYNC_MARK`, byte 1 = number of gliders in the polar list
//! of the sender (a different firmware may number the gliders differently:
//! the glider index is then not taken, the polar values are), bytes 2..6 =
//! the 4 bytes of the EEPROM item. Older displays and the sensor box do not
//! know these ids and ignore them (sensor box: CAN_listener.cpp takes MC,
//! bugs, QNH, vario mode, ballast and the commands by id, everything else
//! only from 0x2000 on; checked 16.11.2026).
//!
//! Rules:
//! - Every display says "hello" every 10 s. Nothing else is sent before a
//!   second display has been heard, so a single display stays silent.
//! - A change made on a display (menu, wizard, preset, flap editor) goes out
//!   at once.
//! - The CAN master (the display that came up first) repeats all 43 items, one
//!   per second, as long as a second display is there. A display that was
//!   changed while the other one was off therefore takes over the settings of
//!   the master within a minute.
//! - A received item is taken only if it differs from the own value (no
//!   EEPROM writes in the steady state), not within 5 s after the pilot
//!   changed the same item here (a repetition that crossed the change must
//!   not undo it), and only while the EEPROM write buffer has room (the
//!   master repeats it).
//! - Received items are not sent on: no echo between the displays.

use crate::{
    controller::persist::{self, PersistenceId},
    flight_physics::polar_store,
    utils::PersistenceItem,
    CanFrame, CoreController, CoreModel, Frame, GenericId,
};
#[allow(unused_imports)]
use micromath::F32Ext;

/// config ids of the shared items: `SYNC_BASE + PersistenceId`
pub const SYNC_BASE: u16 = 0x1000;
/// "a display is here"
pub const SYNC_HELLO: u16 = 0x1FFF;
/// gear alarm of the display the switches are wired to: byte 4 = condition,
/// byte 5 = confirmed by the pilot on that display
pub const SYNC_GEAR: u16 = 0x1FFE;
/// a gear alarm of the other display counts for 3 s after its last frame
const GEAR_TICKS: u32 = 30;
/// first payload byte of every sync frame
pub const SYNC_MARK: u8 = 0xA5;

/// hello every 10 s (in 100 ms ticks)
const HELLO_TICKS: u32 = 100;
/// a second display counts as present for 30 s after its last frame
const PEER_TICKS: u32 = 300;
/// a received item does not undo a change made here within the last 5 s
const LOCAL_GUARD_TICKS: u32 = 50;

/// the shared items, in the order the master repeats them (glider first: on
/// a receiver it loads the stored polar, the values after it adjust it)
pub const SYNC_IDS: [PersistenceId; 43] = [
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
    PersistenceId::StfSwitchMode,
    PersistenceId::StfSwitchSpeed,
    PersistenceId::GustFilter,
    PersistenceId::GustCircling,
    PersistenceId::PotBar,
    PersistenceId::Volume,
    PersistenceId::VolumeStf,
    PersistenceId::AlarmVolume,
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
    // the table word (number of stages) last, as in the EEPROM
    PersistenceId::FlapTable,
];

/// State of the exchange (lives in the CoreController)
pub struct DisplaySync {
    /// tick of the last frame of another display, 0 = never
    peer_tick: u32,
    /// next item the master repeats
    rr_idx: usize,
    /// tick of the last change made here, per item
    local_tick: [u32; SYNC_IDS.len()],
    /// an item received from the other display is being applied
    receiving: bool,
    /// gear alarm condition reported by the other display, and when
    remote_gear: bool,
    remote_gear_tick: u32,
    /// own condition last reported
    gear_sent: bool,
}

impl Default for DisplaySync {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplaySync {
    pub const fn new() -> Self {
        DisplaySync {
            peer_tick: 0,
            rr_idx: 0,
            local_tick: [0; SYNC_IDS.len()],
            receiving: false,
            remote_gear: false,
            remote_gear_tick: 0,
            gear_sent: false,
        }
    }

    /// the other display reports a gear alarm (fresh within 3 s)
    pub fn remote_gear(&self, now: u32) -> bool {
        self.remote_gear && now.wrapping_sub(self.remote_gear_tick) < GEAR_TICKS
    }

    /// a second display has been heard within the last 30 s
    pub fn peer_present(&self, now: u32) -> bool {
        self.peer_tick != 0 && now.wrapping_sub(self.peer_tick) < PEER_TICKS
    }
}

fn sync_idx(id: PersistenceId) -> Option<usize> {
    SYNC_IDS.iter().position(|i| *i == id)
}

fn polar_count() -> u8 {
    polar_store::POLARS.len().min(255) as u8
}

/// the EEPROM item of a shared setting as it is now in the model
pub fn current_item(cm: &CoreModel, id: PersistenceId) -> Option<PersistenceItem> {
    let g = &cm.glider_data.basic_glider_data;
    let flaps = &cm.config.flaps;
    let f = |v: f32| Some(PersistenceItem::from_f32(id, v));
    match id {
        PersistenceId::Glider => Some(PersistenceItem::from_u32(id, cm.config.glider_idx as u32)),
        PersistenceId::EmptyMass => f(g.empty_mass),
        PersistenceId::MaxBallast => f(g.max_ballast),
        PersistenceId::ReferenceWeight => f(g.reference_weight),
        PersistenceId::PolarValueV1 => f(g.polar_values[0][0]),
        PersistenceId::PolarValueV2 => f(g.polar_values[1][0]),
        PersistenceId::PolarValueV3 => f(g.polar_values[2][0]),
        PersistenceId::PolarValueSi1 => f(g.polar_values[0][1]),
        PersistenceId::PolarValueSi2 => f(g.polar_values[1][1]),
        PersistenceId::PolarValueSi3 => f(g.polar_values[2][1]),
        PersistenceId::StfSwitchMode => {
            Some(PersistenceItem::from_u8(id, cm.config.stf_switch_mode))
        }
        PersistenceId::StfSwitchSpeed => f(cm.config.stf_switch_speed.to_m_s()),
        PersistenceId::GustFilter => {
            Some(PersistenceItem::from_u8(id, cm.config.gust_filter as u8))
        }
        PersistenceId::GustCircling => Some(PersistenceItem::from_bool(
            id,
            cm.config.gust_filter_circling,
        )),
        // stored in whole percent: no float noise between the displays
        PersistenceId::PotBar => f((cm.config.pot_bar * 100.0 + 0.5).floor()),
        PersistenceId::Volume => Some(PersistenceItem::from_i8(id, cm.config.volume)),
        PersistenceId::VolumeStf => Some(PersistenceItem::from_i8(id, cm.config.volume_stf)),
        PersistenceId::AlarmVolume => Some(PersistenceItem::from_i8(id, cm.control.alarm_volume)),
        PersistenceId::FlapTable => Some(PersistenceItem::from_u32(id, flaps.word())),
        _ => {
            let raw = id as u16;
            let stage0 = PersistenceId::FlapStage0 as u16;
            let pos0 = PersistenceId::FlapPos0 as u16;
            if (stage0..stage0 + 12).contains(&raw) {
                let st = flaps.stages[(raw - stage0) as usize];
                Some(PersistenceItem::from_u32(id, st.word()))
            } else if (pos0..pos0 + 12).contains(&raw) {
                f(flaps.stages[(raw - pos0) as usize].position)
            } else {
                None
            }
        }
    }
}

fn item_frame(item: &PersistenceItem) -> Frame {
    Frame::generic(
        CanFrame::empty_from_id(0)
            .push_u16(SYNC_BASE + item.id as u16)
            .push_u8(SYNC_MARK)
            .push_u8(polar_count())
            .push_slice(&item.data),
        GenericId::SetSysSetting as u16,
    )
}

fn hello_frame() -> Frame {
    Frame::generic(
        CanFrame::empty_from_id(0)
            .push_u16(SYNC_HELLO)
            .push_u8(SYNC_MARK)
            .push_u8(polar_count())
            .push_slice(&[0, 0, 0, 0]),
        GenericId::SetSysSetting as u16,
    )
}

fn send(cc: &mut CoreController, frame: Frame) {
    let _ = cc.p_tx_frames.enqueue(frame);
}

/// Gear alarm state of this display to the other one: the own condition
/// (from the switches wired here) and whether it is confirmed here. Sent
/// when the condition changes, when the pilot confirms, and every second
/// while the condition holds.
pub fn send_gear(cc: &mut CoreController, cm: &CoreModel, local: bool) {
    cc.display_sync.gear_sent = local;
    if !cc.display_sync.peer_present(cm.control.alive_ticks) {
        return;
    }
    let frame = Frame::generic(
        CanFrame::empty_from_id(0)
            .push_u16(SYNC_GEAR)
            .push_u8(SYNC_MARK)
            .push_u8(polar_count())
            .push_slice(&[local as u8, cm.control.gear_alarm_ack as u8, 0, 0]),
        GenericId::SetSysSetting as u16,
    );
    send(cc, frame);
}

/// the own gear condition changed since it was last reported
pub fn gear_changed(cc: &CoreController, local: bool) -> bool {
    cc.display_sync.gear_sent != local
}

/// called by `persist_set` for every setting changed on this display
pub fn on_local_change(cc: &mut CoreController, cm: &CoreModel, id: PersistenceId) {
    if cc.display_sync.receiving {
        return;
    }
    let Some(i) = sync_idx(id) else { return };
    let now = cm.control.alive_ticks;
    cc.display_sync.local_tick[i] = now.max(1);
    if cc.display_sync.peer_present(now) {
        if let Some(item) = current_item(cm, id) {
            send(cc, item_frame(&item));
        }
    }
}

/// once per second (tick_1s)
pub fn tick_1s(cm: &CoreModel, cc: &mut CoreController) {
    let now = cm.control.alive_ticks;
    if (now / 10).is_multiple_of(HELLO_TICKS / 10) {
        send(cc, hello_frame());
    }
    let local_gear = cc.gear_alarm_control.condition();
    if local_gear {
        send_gear(cc, cm, true);
    }
    if cm.control.is_can_master && cc.display_sync.peer_present(now) {
        let id = SYNC_IDS[cc.display_sync.rr_idx % SYNC_IDS.len()];
        cc.display_sync.rr_idx = (cc.display_sync.rr_idx + 1) % SYNC_IDS.len();
        if let Some(item) = current_item(cm, id) {
            send(cc, item_frame(&item));
        }
    }
}

/// a SetSysSetting frame with a config id of this module; false = not ours
pub fn on_frame(
    cc: &mut CoreController,
    cm: &mut CoreModel,
    config_id: u16,
    frame: &CanFrame,
) -> bool {
    if !(SYNC_BASE..=SYNC_HELLO).contains(&config_id) {
        return false;
    }
    if frame.data().len() < 8 || frame.read_u8(2) != SYNC_MARK {
        return true;
    }
    let now = cm.control.alive_ticks;
    cc.display_sync.peer_tick = now.max(1);
    if config_id == SYNC_HELLO {
        return true;
    }
    if config_id == SYNC_GEAR {
        let condition = frame.read_u8(4) != 0;
        let confirmed = frame.read_u8(5) != 0;
        cc.display_sync.remote_gear = condition;
        cc.display_sync.remote_gear_tick = now.max(1);
        let combined = cc.gear_alarm_control.condition() || cc.display_sync.remote_gear(now);
        if confirmed && combined && !cm.control.gear_alarm_ack {
            // confirmed in the other seat
            crate::controller::gear_alarm::confirm_from_other(cm, cc);
        }
        crate::controller::gear_alarm::update(cm, cc);
        return true;
    }
    let id = PersistenceId::from(config_id - SYNC_BASE);
    let Some(i) = sync_idx(id) else { return true };
    if id == PersistenceId::Glider && frame.read_u8(3) != polar_count() {
        // another polar list: the index means another glider here; the
        // polar values that follow are taken
        return true;
    }
    let lt = cc.display_sync.local_tick[i];
    if lt != 0 && now.wrapping_sub(lt) < LOCAL_GUARD_TICKS {
        return true;
    }
    let mut data = [0u8; 4];
    data.copy_from_slice(&frame.data()[4..8]);
    let item = PersistenceItem::from_array_4u8(id, data);
    if current_item(cm, id).map(|c| c.data) == Some(item.data) {
        return true;
    }
    if cc.pers_vals.len() + 1 >= crate::controller::MAX_PERS_IDS / 2 {
        // no room in the EEPROM write buffer now: the master repeats it
        return true;
    }
    cc.display_sync.receiving = true;
    persist::persist_received(cc, cm, item);
    cc.display_sync.receiving = false;
    true
}

#[cfg(test)]
mod tests {
    //! Two displays on one bus: what one sends, the other reads.
    use super::*;
    use crate::{
        basic_config::MAX_TX_FRAMES, common::CTxFrames, controller::persist::persist_set,
        test_harness, utils::Variant, Echo, FloatToSpeed,
    };

    /// everything A has put on the bus goes to B; returns the sync item frames
    fn pass(
        rx_a: &mut CTxFrames<MAX_TX_FRAMES>,
        b_cm: &mut CoreModel,
        b_cc: &mut CoreController,
    ) -> usize {
        let mut items = 0;
        while let Some(f) = rx_a.dequeue() {
            if let Frame::Generic(g) = &f {
                let id = g.can_frame.read_u16(0);
                if (SYNC_BASE..SYNC_HELLO).contains(&id) {
                    items += 1;
                }
            }
            b_cc.read_can_frame(b_cm, &f);
        }
        items
    }

    /// one second for both: hello / repetition, then the bus
    fn second(
        a: (
            &mut CoreModel,
            &mut CoreController,
            &mut CTxFrames<MAX_TX_FRAMES>,
        ),
        b: (
            &mut CoreModel,
            &mut CoreController,
            &mut CTxFrames<MAX_TX_FRAMES>,
        ),
    ) -> (usize, usize) {
        a.0.control.alive_ticks += 10;
        b.0.control.alive_ticks += 10;
        tick_1s(a.0, a.1);
        tick_1s(b.0, b.1);
        let ab = pass(a.2, b.0, b.1);
        let ba = pass(b.2, a.0, a.1);
        (ab, ba)
    }

    fn flaps_differ(a: &CoreModel, b: &CoreModel) -> bool {
        SYNC_IDS
            .iter()
            .any(|id| current_item(a, *id).map(|i| i.data) != current_item(b, *id).map(|i| i.data))
    }

    #[test]
    fn a_change_on_one_display_shows_on_the_other_and_comes_not_back() {
        let (mut am, mut ac, mut ar) = test_harness!();
        let (mut bm, mut bc, mut br) = test_harness!();
        am.control.is_can_master = true;
        bm.control.is_can_master = false;
        // both come up: after the first hello each knows the other
        for _ in 0..10 {
            second((&mut am, &mut ac, &mut ar), (&mut bm, &mut bc, &mut br));
        }
        assert!(ac.display_sync.peer_present(am.control.alive_ticks));
        assert!(bc.display_sync.peer_present(bm.control.alive_ticks));
        // front pilot picks another glider: the rear display has it at once
        let idx = 3u32;
        assert!(bm.config.glider_idx as u32 != idx);
        persist_set(
            &mut ac,
            &mut am,
            Variant::U32(idx),
            PersistenceId::Glider,
            Echo::None,
        );
        let n = pass(&mut ar, &mut bm, &mut bc);
        assert_eq!(n, 1, "one frame for one change");
        assert_eq!(bm.config.glider_idx as u32, idx);
        assert_eq!(
            bm.glider_data.basic_glider_data.polar_values,
            am.glider_data.basic_glider_data.polar_values
        );
        // nothing is echoed back
        assert_eq!(pass(&mut br, &mut am, &mut ac), 0);
        // rear pilot changes the empty mass and the switch speed: the front has them
        persist_set(
            &mut bc,
            &mut bm,
            Variant::F32(355.0),
            PersistenceId::EmptyMass,
            Echo::None,
        );
        persist_set(
            &mut bc,
            &mut bm,
            Variant::F32(28.0),
            PersistenceId::StfSwitchSpeed,
            Echo::None,
        );
        assert_eq!(pass(&mut br, &mut am, &mut ac), 2);
        assert_eq!(am.glider_data.basic_glider_data.empty_mass, 355.0);
        assert!((am.config.stf_switch_speed.to_m_s() - 28.0).abs() < 1e-6);
        assert_eq!(pass(&mut ar, &mut bm, &mut bc), 0);
        assert!(!flaps_differ(&am, &bm));
    }

    #[test]
    fn a_display_changed_while_the_other_was_off_takes_the_settings_of_the_master() {
        let (mut am, mut ac, mut ar) = test_harness!();
        let (mut bm, mut bc, mut br) = test_harness!();
        am.control.is_can_master = true;
        // the rear display was set up alone: other glider, own flap table
        persist_set(
            &mut bc,
            &mut bm,
            Variant::U32(5),
            PersistenceId::Glider,
            Echo::None,
        );
        persist_set(
            &mut bc,
            &mut bm,
            Variant::F32(410.0),
            PersistenceId::EmptyMass,
            Echo::None,
        );
        bm.config.flaps.set_word(3 | (1 << 10));
        while br.dequeue().is_some() {}
        while ar.dequeue().is_some() {}
        assert!(flaps_differ(&am, &bm));
        // together on the bus: within one round of the master all is equal
        let mut writes = 0;
        for _ in 0..(10 + SYNC_IDS.len() + 2) {
            second((&mut am, &mut ac, &mut ar), (&mut bm, &mut bc, &mut br));
            writes += bc.pers_vals.len();
            crate::controller::persist::store_persistence_ids(&mut bm, &mut bc);
        }
        assert!(
            !flaps_differ(&am, &bm),
            "same glider, polar, switch speed and flap table"
        );
        assert!(writes > 0);
        // steady state: the repetitions write nothing more
        for _ in 0..SYNC_IDS.len() {
            second((&mut am, &mut ac, &mut ar), (&mut bm, &mut bc, &mut br));
            assert!(
                bc.pers_vals.is_empty() && ac.pers_vals.is_empty(),
                "no EEPROM writes when equal"
            );
        }
    }

    #[test]
    fn a_repetition_that_crosses_a_change_does_not_undo_it() {
        let (mut am, mut ac, mut ar) = test_harness!();
        let (mut bm, mut bc, mut br) = test_harness!();
        am.control.is_can_master = true;
        for _ in 0..10 {
            second((&mut am, &mut ac, &mut ar), (&mut bm, &mut bc, &mut br));
        }
        // the master is about to repeat the empty mass with its old value
        let old = am.glider_data.basic_glider_data.empty_mass;
        let stale = current_item(&am, PersistenceId::EmptyMass).unwrap();
        persist_set(
            &mut bc,
            &mut bm,
            Variant::F32(old + 20.0),
            PersistenceId::EmptyMass,
            Echo::None,
        );
        bm.control.alive_ticks += 5;
        bc.read_can_frame(&mut bm, &item_frame(&stale));
        assert_eq!(bm.glider_data.basic_glider_data.empty_mass, old + 20.0);
        // and the change reaches the master
        pass(&mut br, &mut am, &mut ac);
        assert_eq!(am.glider_data.basic_glider_data.empty_mass, old + 20.0);
    }

    #[test]
    fn a_single_display_sends_only_its_hello() {
        let (mut am, mut ac, mut ar) = test_harness!();
        am.control.is_can_master = true;
        persist_set(
            &mut ac,
            &mut am,
            Variant::U32(2),
            PersistenceId::Glider,
            Echo::None,
        );
        let mut frames = 0;
        for _ in 0..60 {
            am.control.alive_ticks += 10;
            tick_1s(&am, &mut ac);
        }
        while let Some(Frame::Generic(g)) = ar.dequeue() {
            let id = g.can_frame.read_u16(0);
            if (SYNC_BASE..=SYNC_HELLO).contains(&id) {
                assert_eq!(id, SYNC_HELLO);
                frames += 1;
            }
        }
        assert_eq!(frames, 6, "one hello per 10 s, nothing else");
    }

    #[test]
    fn another_polar_list_keeps_the_glider_but_takes_the_polar() {
        let (mut am, mut ac, _ar) = test_harness!();
        let (mut bm, mut bc, _br) = test_harness!();
        let _ = (&mut am, &mut ac);
        bm.control.alive_ticks = 100;
        let before = bm.config.glider_idx;
        let item = PersistenceItem::from_u32(PersistenceId::Glider, 4);
        let f = Frame::generic(
            CanFrame::empty_from_id(0)
                .push_u16(SYNC_BASE + PersistenceId::Glider as u16)
                .push_u8(SYNC_MARK)
                .push_u8(polar_count().wrapping_add(1))
                .push_slice(&item.data),
            GenericId::SetSysSetting as u16,
        );
        bc.read_can_frame(&mut bm, &f);
        assert_eq!(bm.config.glider_idx, before);
        let item = PersistenceItem::from_f32(PersistenceId::PolarValueSi2, -0.77);
        bc.read_can_frame(&mut bm, &item_frame(&item));
        assert_eq!(bm.glider_data.basic_glider_data.polar_values[1][1], -0.77);
        let _ = 0.0.m_s();
    }
    /// both displays run `ms` milliseconds; the bus is passed every 10 ms
    fn run_both(
        a: (
            &mut CoreModel,
            &mut CoreController,
            &mut CTxFrames<MAX_TX_FRAMES>,
        ),
        b: (
            &mut CoreModel,
            &mut CoreController,
            &mut CTxFrames<MAX_TX_FRAMES>,
        ),
        clock: &mut u16,
        ms: u32,
    ) {
        for k in 0..ms {
            *clock = clock.wrapping_add(1);
            a.1.tick_1ms(*clock, a.0);
            b.1.tick_1ms(*clock, b.0);
            if k % 10 == 0 {
                pass(a.2, b.0, b.1);
                pass(b.2, a.0, a.1);
            }
        }
    }

    #[test]
    fn the_gear_alarm_of_the_front_display_shows_in_the_rear_and_either_push_confirms_it() {
        use crate::{
            controller::{gear_alarm, InPinFunction},
            Event, InputPinState, KeyEvent, PinState,
        };
        let (mut am, mut ac, mut ar) = test_harness!();
        let (mut bm, mut bc, mut br) = test_harness!();
        am.control.is_can_master = true;
        // the switches are wired to the front display only
        ac.gear_alarm_control
            .set_gear_pin_function(InPinFunction::OnClose);
        ac.gear_alarm_control
            .set_airbrakes_pin_function(InPinFunction::OnClose);
        let pin = |cm: &mut CoreModel, cc: &mut CoreController, gear_pin: bool, closed: bool| {
            let s = if closed {
                PinState::Low
            } else {
                PinState::High
            };
            let e = if gear_pin {
                InputPinState::Io3(s)
            } else {
                InputPinState::Io4(s)
            };
            cc.event_handler(Event::InputItem(e), cm);
        };
        pin(&mut am, &mut ac, true, true); // gear up
        pin(&mut am, &mut ac, false, false); // brakes in
        let mut clock = 0u16;
        run_both(
            (&mut am, &mut ac, &mut ar),
            (&mut bm, &mut bc, &mut br),
            &mut clock,
            12_000,
        );
        assert!(ac.display_sync.peer_present(am.control.alive_ticks));
        assert!(!gear_alarm::shown(&am) && !gear_alarm::shown(&bm));
        // airbrakes out with the gear up: the alarm on both displays
        pin(&mut am, &mut ac, false, true);
        run_both(
            (&mut am, &mut ac, &mut ar),
            (&mut bm, &mut bc, &mut br),
            &mut clock,
            50,
        );
        assert!(
            gear_alarm::shown(&am) && gear_alarm::shown(&bm),
            "box on both"
        );
        assert!(
            ac.sound_control.alarm_active() && bc.sound_control.alarm_active(),
            "tone on both"
        );
        // it stays on the rear display (repeated every second)
        run_both(
            (&mut am, &mut ac, &mut ar),
            (&mut bm, &mut bc, &mut br),
            &mut clock,
            10_000,
        );
        assert!(gear_alarm::shown(&bm));
        // the rear pilot pushes: gone on both, the reminder stays on both
        bc.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut bm);
        run_both(
            (&mut am, &mut ac, &mut ar),
            (&mut bm, &mut bc, &mut br),
            &mut clock,
            50,
        );
        assert!(!gear_alarm::shown(&am) && !gear_alarm::shown(&bm));
        assert!(!ac.sound_control.alarm_active() && !bc.sound_control.alarm_active());
        assert!(am.control.gear_warning && bm.control.gear_warning);
        run_both(
            (&mut am, &mut ac, &mut ar),
            (&mut bm, &mut bc, &mut br),
            &mut clock,
            5_000,
        );
        assert!(
            !gear_alarm::shown(&am) && !gear_alarm::shown(&bm),
            "it does not come back by itself"
        );
        // airbrakes in: all reset on both
        pin(&mut am, &mut ac, false, false);
        run_both(
            (&mut am, &mut ac, &mut ar),
            (&mut bm, &mut bc, &mut br),
            &mut clock,
            50,
        );
        assert!(!am.control.gear_warning && !bm.control.gear_warning);
        assert!(!am.control.gear_alarm_ack && !bm.control.gear_alarm_ack);
        // next time: on both again, the front pilot confirms for both
        pin(&mut am, &mut ac, false, true);
        run_both(
            (&mut am, &mut ac, &mut ar),
            (&mut bm, &mut bc, &mut br),
            &mut clock,
            50,
        );
        assert!(gear_alarm::shown(&am) && gear_alarm::shown(&bm));
        ac.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut am);
        run_both(
            (&mut am, &mut ac, &mut ar),
            (&mut bm, &mut bc, &mut br),
            &mut clock,
            50,
        );
        assert!(!gear_alarm::shown(&am) && !gear_alarm::shown(&bm));
        // the front display goes off with the alarm still pending: the rear
        // one lets it go after 3 s
        pin(&mut am, &mut ac, false, false);
        pin(&mut am, &mut ac, false, true);
        run_both(
            (&mut am, &mut ac, &mut ar),
            (&mut bm, &mut bc, &mut br),
            &mut clock,
            50,
        );
        assert!(gear_alarm::shown(&bm));
        for _ in 0..5000 {
            clock = clock.wrapping_add(1);
            bc.tick_1ms(clock, &mut bm);
        }
        assert!(!gear_alarm::shown(&bm) && !bm.control.gear_warning);
    }

    #[test]
    fn volume_gust_filter_and_bar_are_shared() {
        let (mut am, mut ac, mut ar) = test_harness!();
        let (mut bm, mut bc, mut br) = test_harness!();
        am.control.is_can_master = true;
        for _ in 0..10 {
            second((&mut am, &mut ac, &mut ar), (&mut bm, &mut bc, &mut br));
        }
        // the rear pilot turns the cruise and warning volume, sets the gust
        // filter and the bar: the front display has them
        persist_set(
            &mut bc,
            &mut bm,
            Variant::I8(3),
            PersistenceId::VolumeStf,
            Echo::None,
        );
        persist_set(
            &mut bc,
            &mut bm,
            Variant::I8(7),
            PersistenceId::AlarmVolume,
            Echo::None,
        );
        persist_set(
            &mut bc,
            &mut bm,
            Variant::U8(2),
            PersistenceId::GustFilter,
            Echo::None,
        );
        persist_set(
            &mut bc,
            &mut bm,
            Variant::Bool(true),
            PersistenceId::GustCircling,
            Echo::None,
        );
        persist_set(
            &mut bc,
            &mut bm,
            Variant::F32(80.0),
            PersistenceId::PotBar,
            Echo::None,
        );
        persist_set(
            &mut bc,
            &mut bm,
            Variant::I8(9),
            PersistenceId::Volume,
            Echo::NmeaAndCan,
        );
        pass(&mut br, &mut am, &mut ac);
        assert_eq!(am.config.volume_stf, 3);
        assert_eq!(am.control.alarm_volume, 7);
        assert_eq!(am.config.gust_filter as u8, 2);
        assert!(am.config.gust_filter_circling);
        assert!((am.config.pot_bar - 0.8).abs() < 1e-6);
        assert_eq!(am.config.volume, 9);
        // the master repeats; nothing changes and nothing is written any more
        crate::controller::persist::store_persistence_ids(&mut am, &mut ac);
        crate::controller::persist::store_persistence_ids(&mut bm, &mut bc);
        for _ in 0..(SYNC_IDS.len() + 2) {
            second((&mut am, &mut ac, &mut ar), (&mut bm, &mut bc, &mut br));
            assert!(ac.pers_vals.is_empty() && bc.pers_vals.is_empty());
        }
        assert!(!flaps_differ(&am, &bm));
    }
}
