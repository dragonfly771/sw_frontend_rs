//! Automatic measurement of the pitot offset of the sensor box
//!
//! Setup > Sensorbox > Pitot & Static > Measure Offset. The glider stands
//! still with no wind on the pitot (or the tube covered); the frontend
//! averages the dynamic pressure for ten seconds and offers the result as
//! the new offset in the usual "old > new, Save / Discard" dialog.
//!
//! What the box provides: the corrected pressure `(raw - offset) * span`
//! only as IAS = sqrt(p * 2 / rho0) on CAN 0x122 at 10 Hz, and a negative
//! corrected pressure is clamped to zero before that (navigator.h). The raw
//! pressure can therefore only be recovered while the corrected one is
//! clearly positive. So the measurement first stores `offset - SHIFT_PA` in
//! the box (applied at once: the box re-tunes its pressure gauges on every
//! configuration write, `tune_pressure_gauges()` since 0.7.1, a full
//! re-initialisation before), waits for the box to apply it, then averages
//! p = IAS^2 * rho0 / 2 and computes raw = mean / span + (offset - SHIFT_PA).
//! At rest raw is the new offset. Whatever ends the session without Save
//! (Discard, Esc, time-out) writes the old offset back.

use crate::{
    basic_config::SECTION_EDITOR_TIMEOUT,
    controller::{
        helpers::IntToDuration,
        persist::{queue_remote_get, remote_content},
        CanConfigId, PendingRemote, RemoteConfig, Timer,
    },
    model::{editable::Content, PitotCal, PitotCalError, PitotCalState},
    CoreController, CoreModel, Editable,
};
#[allow(unused_imports)]
use micromath::F32Ext;

/// the offset is lowered by this much for the measurement / Pa
/// (IAS about 33 km/h at rest; larger than any plausible offset error)
pub const SHIFT_PA: f32 = 50.0;
/// waiting for the cached offset and span of the box (100 ms ticks)
pub const PREPARE_TICKS: u8 = 20;
/// time for the box to apply the shifted offset and for the first shifted
/// airspeed frames to arrive (100 ms ticks)
pub const SETTLE_TICKS: u8 = 15;
/// samples at 100 ms = 10 s
pub const MEASURE_TICKS: u8 = 100;
/// above this IAS before the shift the glider is not standing still / m/s
pub const MAX_IAS_AT_REST: f32 = 12.0;
/// dynamic pressure from IAS, the inverse of the box's formula
/// IAS = sqrt(p * RECIP_STD_DENSITY_TIMES_2) with 1.632 (atmosphere.h)
const PA_PER_IAS_SQUARED: f32 = 1.0 / 1.632;

fn keep_editor_open(cc: &mut CoreController) {
    cc.scheduler
        .after(Timer::CloseEditFrame, SECTION_EDITOR_TIMEOUT.secs());
}

/// SET a box value right away, without the editor's persistence path and
/// without touching the cache (the cache keeps the value the pilot sees)
fn write_offset_now(cm: &mut CoreModel, cc: &mut CoreController, value: f32) {
    let frame = cm.can_frame_remote_config(
        CanConfigId::PitotOffset,
        RemoteConfig::Set,
        Content::F32(Some(value)),
    );
    if let Some(frame) = frame {
        let _ = cc.p_tx_frames.enqueue(frame);
    }
}

/// the old offset goes back into the box, the cache is refreshed
fn restore(cm: &mut CoreModel, cc: &mut CoreController) {
    if cm.control.pitot_cal.shifted {
        let old = cm.control.pitot_cal.old_offset;
        write_offset_now(cm, cc, old);
        queue_remote_get(cc, CanConfigId::PitotOffset, true);
        cm.control.pitot_cal.shifted = false;
    }
}

/// Measure Offset activated (the editor stays open while it runs)
pub fn start(cm: &mut CoreModel, cc: &mut CoreController) {
    // a session that was still running is torn down first
    restore(cm, cc);
    cm.control.pitot_cal = PitotCal {
        state: PitotCalState::Prepare(PREPARE_TICKS),
        ..PitotCal::default()
    };
    queue_remote_get(cc, CanConfigId::PitotOffset, true);
    queue_remote_get(cc, CanConfigId::PitotSpan, false);
}

/// 100 ms tick
pub fn tick(cm: &mut CoreModel, cc: &mut CoreController) {
    let ias = cm.sensor.airspeed.ias().to_m_s();
    let cal = cm.control.pitot_cal;
    match cal.state {
        PitotCalState::Idle | PitotCalState::Confirm | PitotCalState::Failed(_) => (),
        PitotCalState::Prepare(left) => {
            keep_editor_open(cc);
            if ias > MAX_IAS_AT_REST {
                cm.control.pitot_cal.state = PitotCalState::Failed(PitotCalError::NotOnGround);
                return;
            }
            let offset = remote_content(cm, CanConfigId::PitotOffset);
            let span = remote_content(cm, CanConfigId::PitotSpan);
            if let (Content::F32(Some(offset)), Content::F32(Some(span))) = (offset, span) {
                // a span of zero would make the box report IAS 0 for ever;
                // measure against the nominal one then
                let span = if span.abs() < 0.1 { 1.0 } else { span };
                cm.control.pitot_cal.old_offset = offset;
                cm.control.pitot_cal.span = span;
                cm.control.pitot_cal.raw = offset;
                write_offset_now(cm, cc, offset - SHIFT_PA);
                cm.control.pitot_cal.shifted = true;
                cm.control.pitot_cal.state = PitotCalState::Settle(SETTLE_TICKS);
            } else if left == 0 {
                cm.control.pitot_cal.state = PitotCalState::Failed(PitotCalError::NoSensorbox);
            } else {
                cm.control.pitot_cal.state = PitotCalState::Prepare(left - 1);
            }
        }
        PitotCalState::Settle(left) => {
            keep_editor_open(cc);
            cm.control.pitot_cal.state = if left == 0 {
                PitotCalState::Measure(0)
            } else {
                PitotCalState::Settle(left - 1)
            };
        }
        PitotCalState::Measure(n) => {
            keep_editor_open(cc);
            let p = ias * ias * PA_PER_IAS_SQUARED;
            let n = n + 1;
            let sum = cal.sum + p;
            let raw = sum / n as f32 / cal.span + (cal.old_offset - SHIFT_PA);
            cm.control.pitot_cal.sum = sum;
            cm.control.pitot_cal.raw = raw;
            if n >= MEASURE_TICKS {
                // the editor shows one decimal - the value offered is the
                // one the pilot reads
                let new = (raw * 10.0 + if raw >= 0.0 { 0.5 } else { -0.5 }) as i32 as f32 / 10.0;
                cm.control.pitot_cal.raw = new;
                cm.control.pitot_cal.state = PitotCalState::Confirm;
                crate::controller::editor::open_remote_save(
                    cm,
                    cc,
                    PendingRemote {
                        target: Editable::PitotOffset,
                        config_id: CanConfigId::PitotOffset,
                        old: Content::F32(Some(cal.old_offset)),
                        new: Content::F32(Some(new)),
                    },
                );
            } else {
                cm.control.pitot_cal.state = PitotCalState::Measure(n);
            }
        }
    }
}

/// The save dialog of the measurement was answered (or timed out)
pub fn dialog_closed(cm: &mut CoreModel, cc: &mut CoreController, saved: bool) {
    if cm.control.pitot_cal.state != PitotCalState::Confirm {
        return;
    }
    if saved {
        // set_remote_config() has queued the new value; it overrides the
        // shifted one in the box after the persistence timeout
        cm.control.pitot_cal.shifted = false;
    } else {
        restore(cm, cc);
    }
    cm.control.pitot_cal.state = PitotCalState::Idle;
}

/// The measurement editor closes (Esc, time-out, menu closed) - anything
/// still running is aborted and the box gets its old offset back
pub fn editor_closed(cm: &mut CoreModel, cc: &mut CoreController) {
    match cm.control.pitot_cal.state {
        PitotCalState::Idle | PitotCalState::Confirm => (),
        _ => {
            restore(cm, cc);
            cm.control.pitot_cal.state = PitotCalState::Idle;
        }
    }
}

/// progress in percent for the ring editor, None while waiting for the box
pub fn percent(cal: &PitotCal) -> Option<u8> {
    match cal.state {
        PitotCalState::Settle(_) => Some(0),
        PitotCalState::Measure(n) => Some(((n as u16 * 100) / MEASURE_TICKS as u16) as u8),
        PitotCalState::Confirm => Some(100),
        _ => None,
    }
}

/// the line under the value in the ring editor
pub fn note(cal: &PitotCal) -> &'static str {
    match cal.state {
        PitotCalState::Idle => "",
        PitotCalState::Prepare(_) => "asking sensorbox",
        PitotCalState::Settle(_) | PitotCalState::Measure(_) => "keep still, no wind",
        PitotCalState::Confirm => "",
        PitotCalState::Failed(PitotCalError::NoSensorbox) => "box not answering",
        PitotCalState::Failed(PitotCalError::NotOnGround) => "airspeed not zero",
    }
}

#[cfg(test)]
mod tests {
    //! The measurement as the pilot sees it on the CAN bus: shift, ten
    //! seconds of averaging, the dialog, and the box ends up with the new
    //! offset (Save) or the old one (Discard, Esc); no box, no write.
    use super::*;
    use crate::{
        basic_config::MAX_TX_FRAMES,
        controller::{
            editor,
            helpers::can_ids::{object_id, sensor},
        },
        flight_physics::AirSpeed,
        model::{editable::Content, DisplayActive},
        system_of_units::FloatToSpeed,
        test_harness,
        utils::KeyEvent,
        CanFrame, Editable, Frame, GenericId,
    };
    use heapless::spsc::Consumer;

    type TxConsumer = Consumer<'static, Frame, MAX_TX_FRAMES>;

    /// advance the controller in 10 ms steps, collecting the configuration
    /// frames (the cyclic ones would fill the vector within a second)
    fn run(
        cm: &mut CoreModel,
        cc: &mut CoreController,
        rx: &mut TxConsumer,
        clock: &mut u16,
        ms: u16,
        out: &mut heapless::Vec<Frame, 64>,
    ) {
        for _ in 0..ms / 10 {
            *clock = clock.wrapping_add(10);
            cc.tick_1ms(*clock, cm);
            while let Some(f) = rx.dequeue() {
                if cfg(&f).is_some() {
                    out.push(f).expect("frame vector full");
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

    fn offset_sets(frames: &[Frame]) -> heapless::Vec<f32, 8> {
        let mut v = heapless::Vec::new();
        for f in frames {
            if let Some((CanConfigId::PitotOffset, true, val)) = cfg(f) {
                let _ = v.push(val);
            }
        }
        v
    }

    fn offset_gets(frames: &[Frame]) -> usize {
        frames
            .iter()
            .filter(|f| matches!(cfg(f), Some((CanConfigId::PitotOffset, false, _))))
            .count()
    }

    /// the airspeed the box reports for a raw pressure with a given offset
    fn ias_for(raw_pa: f32, offset_pa: f32, span: f32, cm: &mut CoreModel) {
        let p = ((raw_pa - offset_pa) * span).max(0.0);
        let ias = (p * 1.632).sqrt();
        cm.sensor.airspeed = AirSpeed::from_speeds(ias.m_s(), ias.m_s());
    }

    fn key(cm: &mut CoreModel, cc: &mut CoreController, k: KeyEvent) {
        let mut k = k;
        editor::key_action(&mut k, cm, cc);
    }

    fn cached_offset(cm: &CoreModel) -> Option<f32> {
        match remote_content(cm, CanConfigId::PitotOffset) {
            Content::F32(v) => v,
            _ => None,
        }
    }

    fn pending_new(cm: &CoreModel) -> Option<f32> {
        match cm.control.editor.pending {
            Some(p) => match p.new {
                Content::F32(v) => v,
                _ => None,
            },
            None => None,
        }
    }

    /// box: offset -1.0, span 1.0, true raw pressure at rest -6 Pa; the
    /// measurement is started and runs up to the dialog
    fn measure_up_to_dialog(
        cm: &mut CoreModel,
        cc: &mut CoreController,
        rx: &mut TxConsumer,
        clock: &mut u16,
    ) -> heapless::Vec<Frame, 64> {
        let mut out = heapless::Vec::new();
        run(cm, cc, rx, clock, 50, &mut out);
        answer(cm, cc, CanConfigId::PitotOffset, -1.0);
        answer(cm, cc, CanConfigId::PitotSpan, 1.0);
        out.clear();
        ias_for(-6.0, -1.0, 1.0, cm); // IAS 0: the raw pressure is below the offset
        cm.config.display_active = DisplayActive::Menu;
        editor::activate_editable(Editable::PitotOffsetMeasure, cm, cc);
        run(cm, cc, rx, clock, 200, &mut out);
        // the shifted offset went out, nothing else was written
        assert!(
            offset_sets(&out).as_slice() == [-1.0 - SHIFT_PA],
            "{:?}",
            offset_sets(&out)
        );
        assert!(matches!(
            cm.control.pitot_cal.state,
            PitotCalState::Settle(_)
        ));
        // the box applies it: IAS now reflects raw - shifted offset
        ias_for(-6.0, -1.0 - SHIFT_PA, 1.0, cm);
        out.clear();
        run(
            cm,
            cc,
            rx,
            clock,
            1500 + 100 * MEASURE_TICKS as u16 + 200,
            &mut out,
        );
        assert!(cm.control.pitot_cal.state == PitotCalState::Confirm);
        assert!(cm.control.editor.target == Editable::RemoteSave);
        let new = pending_new(cm).expect("dialog holds the result");
        assert!((new - (-6.0)).abs() < 0.05, "measured {new}");
        assert!(
            offset_sets(&out).is_empty(),
            "no write during the measurement"
        );
        out.clear();
        out
    }

    #[test]
    fn measurement_offers_the_raw_pressure_and_saves_it() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = 0u16;
        let mut out = measure_up_to_dialog(&mut cm, &mut cc, &mut rx, &mut clock);
        // Enter on Save: the new offset goes out after the persistence
        // timeout, followed by a read-back
        key(&mut cm, &mut cc, KeyEvent::BtnEnc);
        run(&mut cm, &mut cc, &mut rx, &mut clock, 700, &mut out);
        assert!(
            offset_sets(&out).as_slice() == [-6.0],
            "{:?}",
            offset_sets(&out)
        );
        assert!(offset_gets(&out) >= 1);
        assert!(cm.control.pitot_cal.state == PitotCalState::Idle);
        assert!(!cm.control.pitot_cal.shifted);
        // the cache shows the new value for the menu list
        assert!(cached_offset(&cm) == Some(-6.0));
    }

    #[test]
    fn discard_puts_the_old_offset_back() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = 0u16;
        let mut out = measure_up_to_dialog(&mut cm, &mut cc, &mut rx, &mut clock);
        key(&mut cm, &mut cc, KeyEvent::Rotary2Right); // Discard
        key(&mut cm, &mut cc, KeyEvent::BtnEnc);
        run(&mut cm, &mut cc, &mut rx, &mut clock, 700, &mut out);
        assert!(
            offset_sets(&out).as_slice() == [-1.0],
            "{:?}",
            offset_sets(&out)
        );
        assert!(offset_gets(&out) >= 1);
        assert!(cm.control.pitot_cal.state == PitotCalState::Idle);
        assert!(cached_offset(&cm) == Some(-1.0));
    }

    #[test]
    fn dialog_time_out_puts_the_old_offset_back() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = 0u16;
        let mut out = measure_up_to_dialog(&mut cm, &mut cc, &mut rx, &mut clock);
        run(&mut cm, &mut cc, &mut rx, &mut clock, 31000, &mut out); // editor time-out (setup: 30 s)
        assert!(
            offset_sets(&out).as_slice() == [-1.0],
            "{:?}",
            offset_sets(&out)
        );
        assert!(cm.control.pitot_cal.state == PitotCalState::Idle);
    }

    #[test]
    fn escape_during_the_measurement_restores_and_enter_is_ignored() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = 0u16;
        let mut out = heapless::Vec::new();
        run(&mut cm, &mut cc, &mut rx, &mut clock, 50, &mut out);
        answer(&mut cm, &mut cc, CanConfigId::PitotOffset, 2.5);
        answer(&mut cm, &mut cc, CanConfigId::PitotSpan, 1.0);
        ias_for(4.0, 2.5, 1.0, &mut cm);
        cm.config.display_active = DisplayActive::Menu;
        editor::activate_editable(Editable::PitotOffsetMeasure, &mut cm, &mut cc);
        run(&mut cm, &mut cc, &mut rx, &mut clock, 3000, &mut out);
        assert!(matches!(
            cm.control.pitot_cal.state,
            PitotCalState::Measure(_)
        ));
        // Enter does not end a running measurement
        key(&mut cm, &mut cc, KeyEvent::BtnEnc);
        run(&mut cm, &mut cc, &mut rx, &mut clock, 100, &mut out);
        assert!(matches!(
            cm.control.pitot_cal.state,
            PitotCalState::Measure(_)
        ));
        assert!(cm.control.editor.target == Editable::PitotOffsetMeasure);
        // Esc (the editor closes) aborts and restores
        out.clear();
        editor::close_edit_frame(&mut cm, &mut cc);
        run(&mut cm, &mut cc, &mut rx, &mut clock, 100, &mut out);
        assert!(
            offset_sets(&out).as_slice() == [2.5],
            "{:?}",
            offset_sets(&out)
        );
        assert!(cm.control.pitot_cal.state == PitotCalState::Idle);
        assert!(!cm.control.pitot_cal.shifted);
    }

    #[test]
    fn no_answer_from_the_box_writes_nothing() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = 0u16;
        let mut out = heapless::Vec::new();
        run(&mut cm, &mut cc, &mut rx, &mut clock, 50, &mut out);
        cm.config.display_active = DisplayActive::Menu;
        editor::activate_editable(Editable::PitotOffsetMeasure, &mut cm, &mut cc);
        run(&mut cm, &mut cc, &mut rx, &mut clock, 3000, &mut out);
        assert!(cm.control.pitot_cal.state == PitotCalState::Failed(PitotCalError::NoSensorbox));
        assert!(offset_sets(&out).is_empty());
        assert!(offset_gets(&out) >= 1, "it did ask");
        assert!(note(&cm.control.pitot_cal) == "box not answering");
    }

    #[test]
    fn airspeed_means_not_on_the_ground() {
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = 0u16;
        let mut out = heapless::Vec::new();
        run(&mut cm, &mut cc, &mut rx, &mut clock, 50, &mut out);
        answer(&mut cm, &mut cc, CanConfigId::PitotOffset, 0.0);
        answer(&mut cm, &mut cc, CanConfigId::PitotSpan, 1.0);
        cm.sensor.airspeed = AirSpeed::from_speeds(25.0.m_s(), 25.0.m_s());
        cm.config.display_active = DisplayActive::Menu;
        editor::activate_editable(Editable::PitotOffsetMeasure, &mut cm, &mut cc);
        run(&mut cm, &mut cc, &mut rx, &mut clock, 500, &mut out);
        assert!(cm.control.pitot_cal.state == PitotCalState::Failed(PitotCalError::NotOnGround));
        assert!(offset_sets(&out).is_empty());
    }

    #[test]
    fn span_is_taken_into_account() {
        // raw -6 Pa, span 1.05: the box reports (raw - offset) * span
        let (mut cm, mut cc, mut rx) = test_harness!();
        let mut clock = 0u16;
        let mut out = heapless::Vec::new();
        run(&mut cm, &mut cc, &mut rx, &mut clock, 50, &mut out);
        answer(&mut cm, &mut cc, CanConfigId::PitotOffset, 3.0);
        answer(&mut cm, &mut cc, CanConfigId::PitotSpan, 1.05);
        ias_for(-6.0, 3.0, 1.05, &mut cm);
        cm.config.display_active = DisplayActive::Menu;
        editor::activate_editable(Editable::PitotOffsetMeasure, &mut cm, &mut cc);
        run(&mut cm, &mut cc, &mut rx, &mut clock, 200, &mut out);
        ias_for(-6.0, 3.0 - SHIFT_PA, 1.05, &mut cm);
        run(
            &mut cm,
            &mut cc,
            &mut rx,
            &mut clock,
            1500 + 100 * MEASURE_TICKS as u16 + 200,
            &mut out,
        );
        let new = pending_new(&cm).expect("dialog");
        assert!((new - (-6.0)).abs() < 0.05, "measured {new}");
    }
}
