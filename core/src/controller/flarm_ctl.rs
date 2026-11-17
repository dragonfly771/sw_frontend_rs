//! FLARM interface, canopy flasher and FLARM demo (plan_flarm_blitzer.md,
//! plan_flarm_warnung.md)
//!
//! Glue between the pure FLARM port in `crate::flarm` and the core:
//!
//! - bytes of the FLARM port (USART2 at the RJ45, Y cable) are fed with
//!   [`CoreController::flarm_recv`]; the device asks
//!   [`FlarmCtl::take_baud_request`] for a new UART rate
//! - every 100 ms: baud rate search, traffic housekeeping, attitude from the
//!   sensor box, warning logic, canopy flasher, result into
//!   `cm.calculated.flarm` (drawn by `view::flarm_pages`)
//! - the FLARM demo writes simulated NMEA sentences into the same traffic
//!   model; real FLARM data is still read, but only feeds the status and
//!   the canopy flasher meanwhile (the flasher NEVER flashes for the demo)
//!
//! Warning tones are not generated yet (decision of 27.10.2026).

use crate::{
    controller::{close_edit_frame, close_menu_display, flasher::*, persist},
    flarm::{
        baud::BaudScanner,
        demo::{Demo, Scenario},
        logic::{compute_into, FlarmView, FollowResult, LogicConfig, LogicState, ViewKind},
        nmea::{NmeaMsg, NmeaReader},
        traffic::Traffic,
    },
    model::{
        flarm_cfg::{FlasherType, WarnView},
        flarm_show::{FlarmPortStatus, FlarmToast},
        DisplayActive, EditMode, OverlayActive,
    },
    utils::Variant,
    CoreController, CoreModel, Echo, IdleEvent, PersistenceId,
};

/// no byte at all for this long: "No data"
const NO_DATA_MS: i64 = 5_000;
/// toast duration in 100 ms ticks
const TOAST_TICKS: u8 = 25;
/// buffer for one simulated second of the demo
const DEMO_BUF: usize = 1536;
/// a warning picture stays this long after the warning has ended
const WARN_HOLD_MS: i64 = 2_000;

fn is_warn_kind(kind: ViewKind) -> bool {
    matches!(
        kind,
        ViewKind::Warn | ViewKind::HeadOn | ViewKind::WarnNonDir | ViewKind::Obstacle
    )
}

pub struct FlarmCtl {
    reader: NmeaReader,
    scanner: BaudScanner,
    traffic: Traffic,
    state: LogicState,
    pub flasher: Flasher,
    demo: Option<Demo>,
    demo_reader: NmeaReader,
    /// baud rate the device has to switch the UART to
    baud_request: Option<u32>,
    /// baud rate the UART runs with (as requested last)
    baud: u32,
    /// config word the baud handling was set up for
    applied_baud_word: Option<u32>,
    /// last real byte
    t_bytes: Option<i64>,
    /// last real PFLAU and its content (for the flasher and the status)
    real: FlarmStatusInput,
    /// alarm level of the last warning picture (0 = none), to detect when a
    /// warning reaches level 2
    last_level: u8,
    /// team id applied to the traffic model (real data)
    applied_team: Option<u32>,
    /// last warning picture and its time: shown 2 s longer after the end
    held: Option<(FlarmView, i64)>,
    /// warning acknowledged with a push: (target id, alarm level). The
    /// picture stays hidden for this target until its level rises, another
    /// target warns or the warning has ended.
    ack: Option<(u32, u8)>,
}

impl Default for FlarmCtl {
    fn default() -> Self {
        Self::new()
    }
}

impl FlarmCtl {
    pub fn new() -> Self {
        FlarmCtl {
            reader: NmeaReader::new(),
            scanner: BaudScanner::new(19200),
            traffic: Traffic::new(),
            state: LogicState::new(),
            flasher: Flasher::new(),
            demo: None,
            demo_reader: NmeaReader::new(),
            baud_request: None,
            baud: 19200,
            applied_baud_word: None,
            t_bytes: None,
            real: FlarmStatusInput::default(),
            last_level: 0,
            applied_team: None,
            held: None,
            ack: None,
        }
    }

    /// acknowledge the warning on the screen (push button)
    pub fn acknowledge(&mut self, view: &FlarmView) {
        self.ack = Some((view.threat.id, view.level));
    }

    /// New UART baud rate for the device (taken once)
    pub fn take_baud_request(&mut self) -> Option<u32> {
        self.baud_request.take()
    }

    /// baud rate the UART should run with now
    pub fn baud(&self) -> u32 {
        self.baud
    }

    fn request_baud(&mut self, baud: u32) {
        if baud != self.baud || self.applied_baud_word.is_none() {
            self.baud = baud;
            self.baud_request = Some(baud);
        }
    }

    pub fn demo_running(&self) -> bool {
        self.demo.is_some()
    }

    /// The FLARM demo was switched on or off (menu System > Demo)
    pub fn demo_changed(&mut self, on: bool, now_ms: i64) {
        if on == self.demo.is_some() {
            return;
        }
        self.traffic.clear();
        self.state = LogicState::new();
        self.demo_reader = NmeaReader::new();
        self.applied_team = None; // re-applied in the next tick
        self.demo = if on {
            let mut d = Demo::new(Scenario::All);
            let _ = d.step(now_ms, &mut []);
            self.traffic.set_team(d.team_id());
            Some(d)
        } else {
            None
        };
    }

    /// One received byte of the FLARM port
    fn recv_byte(&mut self, b: u8, now_ms: i64) {
        self.t_bytes = Some(now_ms);
        let bad_before = self.reader.bad_count;
        let msg = self.reader.feed(b);
        if self.reader.bad_count != bad_before {
            self.scanner.on_bad(now_ms);
        }
        if let Some(m) = msg {
            self.scanner.on_line_ok(now_ms);
            if let NmeaMsg::Pflau(p) = &m {
                self.real = FlarmStatusInput {
                    t_pflau: Some(now_ms),
                    gps: p.gps,
                    alarm_level: p.alarm_level,
                };
            }
            if self.demo.is_none() {
                self.traffic.handle(&m, now_ms);
            }
        }
    }

    /// Rotary on the radar page: select the next target clockwise / counter
    /// clockwise by bearing
    pub fn select_step(&mut self, cm: &mut CoreModel, clockwise: bool) -> u32 {
        let now = cm.control.now_ms;
        self.state.select_step(&self.traffic, now, clockwise)
    }

    /// a target is selected manually at the moment
    pub fn selection_active(&self) -> bool {
        self.state.selected_id() != 0
    }

    /// Push on the radar page: follow the selected target (team mate) or
    /// stop following it
    pub fn follow_selected(&mut self, cm: &mut CoreModel) -> FollowResult {
        let now = cm.control.now_ms;
        let r = self.state.follow_selected(&mut self.traffic, now);
        let show = &mut cm.calculated.flarm;
        match r {
            FollowResult::Followed(id) => {
                show.toast = FlarmToast::Following;
                show.toast_id = id;
                show.toast_ticks = TOAST_TICKS;
            }
            FollowResult::Unfollowed(id) => {
                show.toast = FlarmToast::NotFollowing;
                show.toast_id = id;
                show.toast_ticks = TOAST_TICKS;
            }
            FollowResult::NothingSelected => (),
        }
        r
    }
}

impl CoreController {
    /// Bytes from the FLARM port (call from the controller task, any amount)
    pub fn flarm_recv(&mut self, cm: &mut CoreModel, bytes: &[u8]) {
        if !crate::model::flarm_cfg::FLARM_FEATURE {
            return;
        }
        let now = cm.control.now_ms;
        for &b in bytes {
            self.flarm.recv_byte(b, now);
        }
    }
}

/// baud handling: fixed rate from the setting or automatic search
fn baud_tick(cm: &mut CoreModel, cc: &mut CoreController, now: i64) {
    let word = cm.config.flarm.baud_word();
    if cc.flarm.applied_baud_word != Some(word) {
        // setting changed (menu or restored from the EEPROM)
        let start = cm
            .config
            .flarm
            .fixed_baud()
            .unwrap_or(cm.config.flarm.last_baud);
        cc.flarm.scanner = BaudScanner::new(start);
        let b = cc.flarm.scanner.current();
        cc.flarm.request_baud(b);
        cc.flarm.applied_baud_word = Some(word);
        return;
    }
    if cm.config.flarm.fixed_baud().is_some() {
        return;
    }
    let was_locked = cc.flarm.scanner.locked();
    if let Some(b) = cc.flarm.scanner.tick(now) {
        cc.flarm.request_baud(b);
    }
    if !was_locked && cc.flarm.scanner.locked() {
        let b = cc.flarm.scanner.current();
        if b != cm.config.flarm.last_baud {
            // remember the rate: next start begins with it
            let mut cfg = cm.config.flarm;
            cfg.last_baud = b;
            let w = cfg.baud_word();
            cc.flarm.applied_baud_word = Some(w);
            persist::persist_set(
                cc,
                cm,
                Variant::U32(w),
                PersistenceId::FlarmBaud,
                Echo::None,
            );
        }
    }
}

/// every 100 ms (controller tick)
pub fn flarm_tick_100ms(cm: &mut CoreModel, cc: &mut CoreController) {
    if !crate::model::flarm_cfg::FLARM_FEATURE {
        return;
    }
    let now = cm.control.now_ms;

    // demo state follows the demo setting (also switched off automatically)
    let want_demo = cm.control.demo.flarm();
    if want_demo != cc.flarm.demo.is_some() {
        cc.flarm.demo_changed(want_demo, now);
    }

    baud_tick(cm, cc, now);

    // demo: simulated NMEA into the traffic model
    if let Some(demo) = cc.flarm.demo.as_mut() {
        let mut buf = [0u8; DEMO_BUF];
        let n = demo.step(now, &mut buf);
        if demo.take_restart() {
            let team = demo.team_id();
            cc.flarm.traffic.clear();
            cc.flarm.traffic.set_team(team);
            // no "no data" picture between two scenarios
            cc.flarm.traffic.t_last_data = Some(now);
            cc.flarm.traffic.gps_valid = true;
            cc.flarm.state = LogicState::new();
        }
        let f = &mut cc.flarm;
        for &b in buf.get(..n).unwrap_or(&[]) {
            if let Some(m) = f.demo_reader.feed(b) {
                f.traffic.handle(&m, now);
            }
        }
    } else if cc.flarm.applied_team != Some(cm.config.flarm.team_id) {
        cc.flarm.traffic.set_team(cm.config.flarm.team_id);
        cc.flarm.applied_team = Some(cm.config.flarm.team_id);
    }

    // own attitude from the sensor box (heading of the radar, horizon of the
    // head-on view); without sensor box the GPS track of the FLARM is used
    if cm.control.can_devices != 0 || cm.control.demo.flight() {
        cc.flarm.traffic.set_attitude(
            cm.sensor.euler_yaw.to_degrees(),
            cm.sensor.euler_roll.to_degrees(),
            cm.sensor.euler_pitch.to_degrees(),
            now,
        );
    }
    cc.flarm.traffic.housekeeping(now);

    // warning logic
    let cfg = LogicConfig {
        min_level: cm.config.flarm.warn_level.clamp(1, 3),
        headon_enabled: cm.config.flarm.head_on,
        tz_offset_min: 0,
    };
    {
        let f = &mut cc.flarm;
        let view = &mut cm.calculated.flarm.view;
        compute_into(view, &f.traffic, &cfg, &mut f.state, now);
        // after the end of a warning its picture stays 2 s (plan ch. 2)
        if is_warn_kind(view.kind) {
            f.held = Some((*view, now));
        } else if let Some((hv, t)) = f.held {
            if view.kind == ViewKind::Idle && now.saturating_sub(t) < WARN_HOLD_MS {
                *view = hv;
            } else {
                f.held = None;
            }
        }
        // acknowledged warning: hidden while it is the same target at the same
        // or a lower level; a higher level or another target shows again
        if is_warn_kind(view.kind) {
            if let Some((id, lvl)) = f.ack {
                if view.threat.id == id && view.level <= lvl {
                    view.kind = ViewKind::Idle;
                    view.level = 0;
                } else {
                    f.ack = None;
                }
            }
        } else {
            f.ack = None;
        }
    }

    // status of the real port
    let show = &mut cm.calculated.flarm;
    show.demo = cc.flarm.demo.is_some();
    show.baud = cc.flarm.baud;
    show.baud_locked = cm.config.flarm.fixed_baud().is_some() || cc.flarm.scanner.locked();
    let bytes_fresh = cc
        .flarm
        .t_bytes
        .is_some_and(|t| now.saturating_sub(t) < NO_DATA_MS);
    let pflau_fresh = cc
        .flarm
        .real
        .t_pflau
        .is_some_and(|t| now.saturating_sub(t) < PFLAU_TIMEOUT_MS);
    show.status = if !bytes_fresh {
        FlarmPortStatus::NoData
    } else if !pflau_fresh {
        FlarmPortStatus::NoPflau
    } else {
        FlarmPortStatus::Ok
    };
    if pflau_fresh {
        show.gps = cc.flarm.real.gps;
        show.alarm_level = cc.flarm.real.alarm_level;
    } else {
        show.gps = 255;
        show.alarm_level = 0;
    }
    if show.view.sel_lost {
        show.toast = FlarmToast::SelectionLost;
        show.toast_ticks = TOAST_TICKS;
    }
    if show.toast_ticks > 0 {
        show.toast_ticks -= 1;
        if show.toast_ticks == 0 {
            show.toast = FlarmToast::None;
        }
    }

    // canopy flasher: real FLARM data only
    let fc = cm.config.flarm;
    let ias = if cm.control.demo.flight() {
        cm.control.demo_real_ias_kmh
    } else {
        cm.sensor.airspeed.ias().to_km_h()
    };
    if let Some(seq) = cc.flarm.flasher.update(
        fc.flasher_type,
        fc.flasher_active,
        fc.flash_rate,
        fc.on_speed_kmh,
        cc.flarm.real,
        ias,
        now,
    ) {
        cc.send_idle_event(IdleEvent::FlasherSeq(seq));
    }
    let show = &mut cm.calculated.flarm;
    show.flasher_seq = cc.flarm.flasher.seq();
    show.flasher_test = cc.flarm.flasher.test_running(now) && fc.flasher_type != FlasherType::Off;

    // a real warning of level 2 or more closes menus and editors, the
    // warning picture must be visible
    let kind = show.view.kind;
    let level = show.view.level;
    let is_warn = is_warn_kind(kind);
    // also a warning that starts at level 1 and rises to 2 counts
    let cur_level = if is_warn { level } else { 0 };
    let reached = cur_level >= 2 && cc.flarm.last_level < 2;
    cc.flarm.last_level = cur_level;
    if reached && !show.demo && fc.warn_view != WarnView::Off {
        if cm.control.editor.mode != EditMode::Off {
            close_edit_frame(cm, cc);
        }
        if cm.config.display_active == DisplayActive::Menu
            || cm.config.overlay_active == OverlayActive::Menu
        {
            close_menu_display(cm, cc);
        }
        // the page layout editor ends like a menu (an open choice is dropped)
        crate::controller::page_layout::abort(cm, cc);
    }
}

/// A team mate chosen with the encoder is kept (persisted, profile 0)
pub fn persist_team(cm: &mut CoreModel, cc: &mut CoreController, id: u32) {
    cc.flarm.applied_team = Some(id);
    persist::persist_set(
        cc,
        cm,
        Variant::U32(id),
        PersistenceId::FlarmTeamId,
        Echo::None,
    );
}

/// Keys on the radar page (plan_flarm_warnung.md ch. 4): the small encoder
/// selects targets clockwise / counter clockwise by bearing instead of the
/// volume (volume stays in the flight menu), a push follows the selected
/// target (team mate) or stops following it. Without a selection the push
/// opens the flight menu as usual.
pub fn key_action(key_event: &mut crate::KeyEvent, cm: &mut CoreModel, cc: &mut CoreController) {
    use crate::{model::PageCore, KeyEvent};
    if !crate::model::flarm_cfg::FLARM_FEATURE {
        return;
    }
    // a warning on the screen: one push of any button acknowledges it and
    // removes the picture (core or full screen), the push does nothing else
    let view = cm.calculated.flarm.view;
    if is_warn_kind(view.kind)
        && cm.config.flarm.warn_view != WarnView::Off
        && matches!(
            *key_event,
            KeyEvent::BtnEnc | KeyEvent::BtnEsc | KeyEvent::Btn1 | KeyEvent::Btn2 | KeyEvent::Btn3
        )
    {
        cc.flarm.acknowledge(&view);
        // hide it at once, not only with the next 100 ms tick
        cm.calculated.flarm.view.kind = ViewKind::Idle;
        cm.calculated.flarm.view.level = 0;
        *key_event = KeyEvent::NoEvent;
        return;
    }
    if cm.control.editor.mode != EditMode::Off
        || cm.control.layout.active
        || cm.config.display_active != DisplayActive::Vario
        || cm.config.overlay_active != OverlayActive::None
        || cm.config.page().core != PageCore::Radar
    {
        return;
    }
    match *key_event {
        KeyEvent::Rotary2Left | KeyEvent::Rotary2Right => {
            let cw = *key_event == KeyEvent::Rotary2Right;
            cc.flarm.select_step(cm, cw);
            *key_event = KeyEvent::NoEvent;
        }
        KeyEvent::BtnEnc if cc.flarm.selection_active() => {
            let demo = cc.flarm.demo_running();
            match cc.flarm.follow_selected(cm) {
                FollowResult::Followed(id) if !demo => persist_team(cm, cc, id),
                FollowResult::Unfollowed(_) if !demo => persist_team(cm, cc, 0),
                _ => (),
            }
            *key_event = KeyEvent::NoEvent;
        }
        _ => (),
    }
}

#[cfg(test)]
mod ack_tests {
    use crate::{
        controller::flaps_ctl, flarm::logic::ViewKind, model::demo::DemoMode, Event, KeyEvent,
    };

    #[cfg(feature = "flarm")]
    #[test]
    fn push_acknowledges_the_warning() {
        let (mut cm, mut cc, _tx) = crate::test_harness!();
        flaps_ctl::set_demo(&mut cm, DemoMode::Flarm);
        let mut t: u32 = 0;
        let mut tick = |cm: &mut crate::CoreModel, cc: &mut crate::CoreController, ms: u32| {
            for _ in 0..ms {
                t += 1;
                cc.tick_1ms((t & 0xffff) as u16, cm);
            }
        };
        // run the demo until a warning is on the screen
        let mut found = false;
        for _ in 0..3000 {
            tick(&mut cm, &mut cc, 100);
            if super::is_warn_kind(cm.calculated.flarm.view.kind) {
                found = true;
                break;
            }
        }
        assert!(found, "the demo shows a warning");
        let warned = cm.calculated.flarm.view;
        cc.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut cm);
        assert_eq!(
            cm.calculated.flarm.view.kind,
            ViewKind::Idle,
            "gone at once"
        );
        // the push did not open the flight menu
        assert_eq!(cm.config.overlay_active, crate::model::OverlayActive::None);
        // stays hidden while it is the same target at the same level
        for _ in 0..30 {
            tick(&mut cm, &mut cc, 100);
            let v = cm.calculated.flarm.view;
            if super::is_warn_kind(v.kind) {
                assert!(v.threat.id != warned.threat.id || v.level > warned.level);
                break;
            }
        }
    }
}
