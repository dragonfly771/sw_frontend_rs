//! Info box "Potential Climb" (POT): what a circle here is worth
//!
//! One number for one question in every phase of flight, plus a yes / no
//! against the bar of the pilot (the MacCready value). Concept and numbers:
//! derived from 279 started circles of 11 flights. A Python reference
//! implementation, line by line, gave the expected values of the fixture
//! test (tests/pot_climb.rs).
//!
//! Straight flight: circle value = netto - sink in the own circle, with
//!   netto = vario + polar sink at this speed and load factor. Shown is
//!   C = min(KW, 0.41 * KW + 0.72): up to about 1.2 m/s the air as it is,
//!   above that only 41 % of the excess (what is left of a peak once the
//!   glider circles in it).
//! Roll-in: bank > 10 deg and the turn rate rising arms the box. The energy
//!   height (GNSS altitude + TAS^2 / 2g) 3 s before the roll-in is the
//!   reference. Once the circle stands (turn rate > 8 deg/s):
//!   EM = energy gained since the reference / time,
//!   shown = a(t) * EM + (1 - a(t)) * 0.95,  a(t) = 1 - 0.78 * exp(-t / 8.5 s).
//!   Early the measurement is damped towards the mean result of all
//!   started circles, later it stands alone. Faded in from C over 1 s.
//! Circle: from 300 to 400 deg the box changes to the climb measured over
//!   the last circle (altitude; the speed part runs out until 540 deg).
//! Exit: turn rate below 8 deg/s for 2 s. After a full circle the last value
//!   is held for 10 s, otherwise the box goes back to the straight value;
//!   circling again within that time continues the same thermal.
//!
//! No MacCready value, no average of the day and no memory in the number.
//! The yes / no is the comparison with the bar: yes from number >= bar, no
//! below bar - 0.7 m/s, nothing in between; only while the circle stands
//! (estimate or measured), hysteresis 0.2 m/s, 0.3 s dwell.
//!
//! The sink in the own circle is learned from the circles flown (mean speed
//! and load factor in the established circle, half of it taken over per
//! thermal). Before the first thermal: load factor 1.4 and the speed for a
//! lift coefficient of 1.3 at the current weight.

use crate::{
    flight_physics::Polar,
    model::{GpsState, PotVerdict},
    AirSpeed, CoreModel, FloatToSpeed,
};

#[allow(unused_imports)]
use micromath::F32Ext;

const TICK_S: f32 = 0.1; // 10 Hz (tick_100ms)
const G_M_S2: f32 = 9.80665;
const PT1_ALPHA: f32 = TICK_S / (1.5 + TICK_S); // bank and turn rate, 1.5 s
const N_ALPHA: f32 = TICK_S / 0.6; // load factor
const TAS_ALPHA: f32 = TICK_S / 0.3; // TAS for the kinetic height
const KW_ALPHA: f32 = TICK_S / (1.0 + TICK_S); // circle value, 1 s
const ROLL_ON_RAD: f32 = 10.0 * core::f32::consts::PI / 180.0;
const TR_EST_RAD_S: f32 = 8.0 * core::f32::consts::PI / 180.0;
const RISE_EPS_RAD_S: f32 = 0.2 * core::f32::consts::PI / 180.0;
const RISE_LAG_TICKS: usize = 10; // 1 s

// straight flight: of the circle value 41 % plus 0.72 m/s are left on
// average (259 started circles)
const K_A: f32 = 0.41;
const K_B: f32 = 0.72;
// roll-in: anchor (mean result of all started circles) and its weight
// 0.78 * exp(-t / 8.5 s). The roll-in is detected ~1 s after it began, so
// the weight starts at 0.78 * exp(-1 / 8.5) and decays every tick.
const ANCHOR_M_S: f32 = 0.95;
const W_ARM: f32 = 0.693_427_6;
const W_DECAY: f32 = 0.988_304_2;
/// energy reference: 4 s before the detected roll-in (3 s before its begin)
const PRE_TICKS: usize = 40;
const E_BUF_N: usize = 64;
const HANDOVER_TICKS: u32 = 10; // 1 s between straight and circle value
const SLIDE0_RAD: f32 = 300.0 * core::f32::consts::PI / 180.0;
const SLIDE1_RAD: f32 = 400.0 * core::f32::consts::PI / 180.0;
const FULL_CIRCLE_RAD: f32 = 2.0 * core::f32::consts::PI;
const EKIN_FADE_END_RAD: f32 = 540.0 * core::f32::consts::PI / 180.0;
const EKIN_FADE_SPAN_RAD: f32 = 180.0 * core::f32::consts::PI / 180.0;
const EXIT_TICKS: u32 = 20; // 2 s below the turn rate threshold
const HOLD_FROZEN_TICKS: u32 = 100; // 10 s with the last value
const HOLD_SHORT_TICKS: u32 = 60; // 6 s in which a re-entry continues
const ARM_LOW_ROLL_TICKS: u32 = 15;
const ARM_MAX_TICKS: u32 = 80;
const START_TICKS: u32 = 60; // nothing in the first 6 s
const HIST_N: usize = 150; // 1 Hz entries (turn angle, tick, altitude, kinetic height)
const TAS_MIN_M_S: f32 = 10.0; // below 36 km/h: not flying

// limits of what a glider in flight can deliver; beyond them the value is broken
const SPEED_MAX_M_S: f32 = 150.0; // 540 km/h
const ROLL_MAX_RAD: f32 = 3.2; // the CAN reader wraps into +-180 deg
const TURN_RATE_MAX_RAD_S: f32 = 6.0; // 340 deg/s
const VARIO_MAX_M_S: f32 = 30.0;
const ALT_MAX_M: f32 = 30_000.0;
const LOAD_MAX: f32 = 15.0;
const OUT_LIMIT_M_S: f32 = 9.9;
/// circle values beyond this are not air but a broken input or polar
const KW_LIMIT_M_S: f32 = 30.0;
// sink in the own circle
const N_CIRCLE_DEFAULT: f32 = 1.4;
const LEARN_MIN_TICKS: u32 = 200; // a thermal counts from 20 s measured circling
const LEARN_EMA: f32 = 0.5;
// yes / no
const BAR_MIN_M_S: f32 = 0.5;
const NO_MARGIN_M_S: f32 = 0.7;
const HYST_M_S: f32 = 0.2;
const DWELL_CIRCLE_TICKS: u32 = 3;
const DWELL_STRAIGHT_TICKS: u32 = 10;

/// What the number is at the moment
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PotMode {
    /// straight flight: circle value of the air flown through
    Straight,
    /// roll-in and first circle: energy gained since before the roll-in
    Estimate,
    /// climb measured over the last circle
    Measured,
    /// the last measured value, held for 10 s after the exit
    Hold,
}

#[derive(PartialEq)]
enum State {
    Idle,
    /// roll-in detected, the circle does not stand yet
    Armed,
    Active,
    Hold,
}

/// Raw values of one 100 ms tick
#[derive(Clone, Copy)]
pub struct PotInput {
    pub roll_rad: f32,
    pub turn_rate_rad_s: f32,
    /// compensated vario of the sensor box
    pub vario_m_s: f32,
    pub gnss_alt_m: f32,
    pub ias_m_s: f32,
    pub tas_m_s: f32,
    /// g load (1.0 in level flight)
    pub load_factor: f32,
    pub mc_m_s: f32,
    /// bar = MacCready value (at least 0.5 m/s) times this factor
    pub bar_factor: f32,
    /// wing area of the glider, 0 = unknown
    pub wing_area_m2: f32,
}

/// State of the POT calculation (lives in the CoreController)
pub struct PotClimb {
    ticks: u32,
    n_f: f32,
    tas_f: f32,
    kw_s: f32,
    kw_valid: bool,
    roll_f: f32,
    tr_f: f32,
    trf_hist: [f32; RISE_LAG_TICKS],
    trf_idx: usize,
    // (altitude, kinetic height) of the last ticks, for the reference before the roll-in
    e_buf: [(f32, f32); E_BUF_N],
    e_len: usize,
    e_pos: usize,
    state: State,
    net_rot: f32,
    below: u32,
    arm: u32,
    low_roll: u32,
    hold: u32,
    w_anchor: f32,
    ref_alt: f32,
    ref_ekin: f32,
    ref_tick: u32,
    since_est: u32,
    since_out: u32,
    hist: [(f32, u32, f32, f32); HIST_N],
    hist_len: usize,
    hist_pos: usize,
    frozen: f32,
    show_frozen: bool,
    last_val: f32,
    mode: PotMode,
    // sink in the own circle: learned speed (IAS, 0 = not yet) and load factor
    v_c: f32,
    n_c: f32,
    learn_v: f32,
    learn_n: f32,
    learn_cnt: u32,
    // yes / no
    verdict: PotVerdict,
    cand: PotVerdict,
    cnt: u32,
}

impl Default for PotClimb {
    fn default() -> Self {
        Self::new()
    }
}

impl PotClimb {
    pub const fn new() -> Self {
        PotClimb {
            ticks: 0,
            n_f: 1.0,
            tas_f: 0.0,
            kw_s: 0.0,
            kw_valid: false,
            roll_f: 0.0,
            tr_f: 0.0,
            trf_hist: [0.0; RISE_LAG_TICKS],
            trf_idx: 0,
            e_buf: [(0.0, 0.0); E_BUF_N],
            e_len: 0,
            e_pos: 0,
            state: State::Idle,
            net_rot: 0.0,
            below: 0,
            arm: 0,
            low_roll: 0,
            hold: 0,
            w_anchor: 0.0,
            ref_alt: 0.0,
            ref_ekin: 0.0,
            ref_tick: 0,
            since_est: HANDOVER_TICKS,
            since_out: HANDOVER_TICKS,
            hist: [(0.0, 0, 0.0, 0.0); HIST_N],
            hist_len: 0,
            hist_pos: 0,
            frozen: 0.0,
            show_frozen: false,
            last_val: 0.0,
            mode: PotMode::Straight,
            v_c: 0.0,
            n_c: N_CIRCLE_DEFAULT,
            learn_v: 0.0,
            learn_n: 0.0,
            learn_cnt: 0,
            verdict: PotVerdict::None,
            cand: PotVerdict::None,
            cnt: 0,
        }
    }

    /// Not flying or no valid values: start again, keep what was learned
    /// about the own circles (`v_c`, `n_c`). Field by field instead of
    /// `*self = Self::new()`: on the ground this runs every 100 ms, and a
    /// temporary of the whole struct (3 kB with the buffers) has no business
    /// on the stack of the device. The buffers are only read up to their
    /// fill level, so their content can stay.
    fn reset(&mut self) {
        self.ticks = 0;
        self.n_f = 1.0;
        self.tas_f = 0.0;
        self.kw_s = 0.0;
        self.kw_valid = false;
        self.roll_f = 0.0;
        self.tr_f = 0.0;
        self.trf_hist = [0.0; RISE_LAG_TICKS];
        self.trf_idx = 0;
        self.e_len = 0;
        self.e_pos = 0;
        self.state = State::Idle;
        self.net_rot = 0.0;
        self.below = 0;
        self.arm = 0;
        self.low_roll = 0;
        self.hold = 0;
        self.w_anchor = 0.0;
        self.ref_alt = 0.0;
        self.ref_ekin = 0.0;
        self.ref_tick = 0;
        self.since_est = HANDOVER_TICKS;
        self.since_out = HANDOVER_TICKS;
        self.hist_len = 0;
        self.hist_pos = 0;
        self.frozen = 0.0;
        self.show_frozen = false;
        self.last_val = 0.0;
        self.mode = PotMode::Straight;
        self.learn_v = 0.0;
        self.learn_n = 0.0;
        self.learn_cnt = 0;
        self.verdict = PotVerdict::None;
        self.cand = PotVerdict::None;
        self.cnt = 0;
    }

    pub fn mode(&self) -> PotMode {
        self.mode
    }

    pub fn verdict(&self) -> PotVerdict {
        self.verdict
    }

    /// Sink in the own circle in m/s (positive) at the density given by `k` = TAS / IAS
    pub fn circle_sink(&self, polar: &Polar, wing_area_m2: f32, k: f32) -> f32 {
        let v_c = if self.v_c > 0.0 {
            self.v_c
        } else {
            polar.circling_ias(wing_area_m2, self.n_c).to_m_s()
        };
        -polar
            .sink_rate_g(AirSpeed::new(v_c, v_c * k), self.n_c)
            .to_m_s()
    }

    /// One 100 ms tick with raw values; returns the value to show in m/s,
    /// None while not flying
    pub fn tick_raw(&mut self, inp: &PotInput, polar: &Polar) -> Option<f32> {
        let (roll, tr, alt) = (inp.roll_rad, inp.turn_rate_rad_s, inp.gnss_alt_m);
        let (ias, tas) = (inp.ias_m_s, inp.tas_m_s);
        // A glider in flight, with values it can have. Everything else (and
        // not-a-number: every comparison with it is false) is no flight. One
        // wild value in a filter would be shown for a minute, NaN for good.
        let plausible = tas > TAS_MIN_M_S
            && tas < SPEED_MAX_M_S
            && ias.abs() < SPEED_MAX_M_S
            && roll.abs() <= ROLL_MAX_RAD
            && tr.abs() < TURN_RATE_MAX_RAD_S
            && inp.vario_m_s.abs() < VARIO_MAX_M_S
            && alt.abs() < ALT_MAX_M
            && inp.load_factor.abs() < LOAD_MAX;
        if !plausible {
            self.reset();
            return None;
        }
        self.ticks = self.ticks.wrapping_add(1);
        let k = (tas / ias.max(10.0)).clamp(1.0, 1.3);

        // ---- the air: netto, circle value, straight flight value
        self.n_f += N_ALPHA * (inp.load_factor - self.n_f);
        if self.tas_f <= 0.0 {
            self.tas_f = tas;
        } else {
            self.tas_f += TAS_ALPHA * (tas - self.tas_f);
        }
        let ekin = self.tas_f * self.tas_f / (2.0 * G_M_S2);
        let sink = polar
            .sink_rate_g(AirSpeed::new(ias, tas), self.n_f.clamp(0.2, 3.0))
            .to_m_s();
        let netto = inp.vario_m_s - sink;
        let kw = netto - self.circle_sink(polar, inp.wing_area_m2, k);
        if !(kw.abs() < KW_LIMIT_M_S) {
            // the polar gives no usable number (not initialised yet, or an
            // air density of 0 from the box: NaN with exact arithmetic, 1e19
            // with the square root of the device): no value either
            self.reset();
            return None;
        }
        if self.kw_valid {
            self.kw_s += KW_ALPHA * (kw - self.kw_s);
        } else {
            self.kw_s = kw;
            self.kw_valid = true;
        }
        let c = self.kw_s.min(K_A * self.kw_s + K_B);

        // ---- state of flight
        self.roll_f += PT1_ALPHA * (roll.abs() - self.roll_f);
        self.tr_f += PT1_ALPHA * (tr - self.tr_f);
        let trf = self.tr_f.abs();
        let trf_old = self.trf_hist[self.trf_idx];
        self.trf_hist[self.trf_idx] = trf;
        self.trf_idx = (self.trf_idx + 1) % RISE_LAG_TICKS;

        let mut just_established = false;
        if self.state != State::Idle {
            self.w_anchor *= W_DECAY;
        }
        if self.since_est < HANDOVER_TICKS {
            self.since_est += 1;
        }
        if self.since_out < HANDOVER_TICKS {
            self.since_out += 1;
        }
        if self.state == State::Hold {
            self.hold += 1;
            let limit = if self.show_frozen {
                HOLD_FROZEN_TICKS
            } else {
                HOLD_SHORT_TICKS
            };
            if self.hold > limit {
                self.state = State::Idle;
            } else if self.roll_f > ROLL_ON_RAD && trf > TR_EST_RAD_S {
                // circling again: the same thermal goes on
                self.state = State::Active;
                self.below = 0;
                if !self.show_frozen {
                    // the box was back on the straight value: fade in again
                    self.since_est = 0;
                    just_established = true;
                }
            }
        }
        if self.state == State::Idle
            && self.roll_f > ROLL_ON_RAD
            && trf - trf_old > RISE_EPS_RAD_S
            && self.ticks > START_TICKS
        {
            // roll-in detected (the smoothing sees it ~1 s after it began)
            self.state = State::Armed;
            self.arm = 0;
            self.low_roll = 0;
            self.net_rot = 0.0;
            self.hist_len = 0;
            self.hist_pos = 0;
            self.w_anchor = W_ARM;
            let back = PRE_TICKS.min(self.e_len);
            if back > 0 {
                let (a0, e0) = self.e_buf[(self.e_pos + E_BUF_N - back) % E_BUF_N];
                self.ref_alt = a0;
                self.ref_ekin = e0;
                self.ref_tick = self.ticks.wrapping_sub(back as u32);
            } else {
                self.ref_alt = alt;
                self.ref_ekin = ekin;
                self.ref_tick = self.ticks;
            }
            self.since_est = HANDOVER_TICKS;
        } else if self.state == State::Armed {
            self.arm += 1;
            self.net_rot += tr * TICK_S;
            if self.roll_f < ROLL_ON_RAD {
                self.low_roll += 1;
            } else {
                self.low_roll = 0;
            }
            if trf > TR_EST_RAD_S {
                // the circle stands
                self.state = State::Active;
                self.below = 0;
                self.since_est = 0;
                just_established = true;
            } else if self.low_roll >= ARM_LOW_ROLL_TICKS || self.arm > ARM_MAX_TICKS {
                self.state = State::Idle;
            }
        }

        let out;
        let mode;
        if self.state == State::Active {
            if !just_established {
                self.net_rot += tr * TICK_S;
            }
            let rot = self.net_rot.abs();
            if self.ticks % 10 == 0 {
                self.hist[self.hist_pos] = (rot, self.ticks, alt, ekin);
                self.hist_pos = (self.hist_pos + 1) % HIST_N;
                if self.hist_len < HIST_N {
                    self.hist_len += 1;
                }
            }
            // energy gained since before the roll-in, damped towards the anchor
            let age = self.ticks.wrapping_sub(self.ref_tick);
            let em =
                ((alt - self.ref_alt) + (ekin - self.ref_ekin)) / (age as f32 * TICK_S).max(1.0);
            let mut val = (1.0 - self.w_anchor) * em + self.w_anchor * ANCHOR_M_S;
            let mut md = PotMode::Estimate;
            if rot > SLIDE0_RAD {
                // climb over the last circle. While the window still reaches
                // back before the roll-in it takes the full energy; after
                // that the newest entry at least one circle back, and the
                // speed part runs out
                let want = rot - FULL_CIRCLE_RAD;
                let (mut m_alt, mut m_ekin, mut m_age, mut f) =
                    (self.ref_alt, self.ref_ekin, age, 1.0);
                for i in 0..self.hist_len {
                    let idx = (self.hist_pos + HIST_N - 1 - i) % HIST_N;
                    let (r, tk, al, ek) = self.hist[idx];
                    if r <= want {
                        m_alt = al;
                        m_ekin = ek;
                        m_age = self.ticks.wrapping_sub(tk);
                        f = ((EKIN_FADE_END_RAD - rot) / EKIN_FADE_SPAN_RAD).clamp(0.0, 1.0);
                        break;
                    }
                }
                let dtm = (m_age as f32 * TICK_S).max(1.0);
                let slide = (alt - m_alt) / dtm + f * (ekin - m_ekin) / dtm;
                let w2 = ((rot - SLIDE0_RAD) / (SLIDE1_RAD - SLIDE0_RAD)).clamp(0.0, 1.0);
                val = (1.0 - w2) * val + w2 * slide;
                if w2 >= 1.0 {
                    md = PotMode::Measured;
                }
            }
            let u = (self.since_est as f32 / HANDOVER_TICKS as f32).min(1.0);
            out = (1.0 - u) * c + u * val;
            self.last_val = out;
            mode = md;
            if md == PotMode::Measured {
                // how this pilot circles: running means over the thermal
                self.learn_cnt += 1;
                self.learn_v += (ias - self.learn_v) / self.learn_cnt as f32;
                self.learn_n += (self.n_f - self.learn_n) / self.learn_cnt as f32;
            }
            if trf < TR_EST_RAD_S {
                self.below += 1;
                if self.below >= EXIT_TICKS {
                    self.state = State::Hold;
                    self.hold = 0;
                    self.frozen = val;
                    self.since_out = 0;
                    self.show_frozen = rot >= FULL_CIRCLE_RAD;
                    if self.learn_cnt >= LEARN_MIN_TICKS {
                        let n_m = self.learn_n.clamp(1.05, 2.5);
                        if self.v_c > 0.0 {
                            self.v_c += LEARN_EMA * (self.learn_v - self.v_c);
                            self.n_c += LEARN_EMA * (n_m - self.n_c);
                        } else {
                            self.v_c = self.learn_v;
                            self.n_c = n_m;
                        }
                    }
                    self.learn_v = 0.0;
                    self.learn_n = 0.0;
                    self.learn_cnt = 0;
                }
            } else {
                self.below = 0;
            }
        } else if self.state == State::Hold && self.show_frozen {
            out = self.frozen;
            mode = PotMode::Hold;
        } else {
            let u = (self.since_out as f32 / HANDOVER_TICKS as f32).min(1.0);
            out = (1.0 - u) * self.last_val + u * c;
            mode = PotMode::Straight;
        }

        // energy buffer after the evaluation: the reference lies whole ticks back
        self.e_buf[self.e_pos] = (alt, ekin);
        self.e_pos = (self.e_pos + 1) % E_BUF_N;
        if self.e_len < E_BUF_N {
            self.e_len += 1;
        }

        if !out.is_finite() {
            // last line of defence: never show "NaN", never keep it
            self.reset();
            return None;
        }
        let out = out.clamp(-OUT_LIMIT_M_S, OUT_LIMIT_M_S);
        self.mode = mode;
        self.update_verdict(out, mode, inp.mc_m_s.max(BAR_MIN_M_S) * inp.bar_factor);
        Some(out)
    }

    /// Yes / no against the bar, only while the circle stands
    fn update_verdict(&mut self, v: f32, mode: PotMode, bar: f32) {
        let circle = mode == PotMode::Estimate || mode == PotMode::Measured;
        let cur = self.verdict;
        let mut want = cur;
        if mode == PotMode::Hold {
            want = PotVerdict::None;
        } else {
            match cur {
                PotVerdict::Yes => {
                    if v < bar - HYST_M_S || !circle {
                        want = PotVerdict::None;
                    }
                }
                PotVerdict::No => {
                    if v >= bar - NO_MARGIN_M_S + HYST_M_S || !circle {
                        want = PotVerdict::None;
                    }
                }
                PotVerdict::None => (),
            }
            if want == PotVerdict::None && circle {
                if v >= bar {
                    want = PotVerdict::Yes;
                } else if v < bar - NO_MARGIN_M_S {
                    want = PotVerdict::No;
                }
            }
        }
        if want == cur {
            self.cnt = 0;
        } else {
            if want == self.cand {
                self.cnt += 1;
            } else {
                self.cand = want;
                self.cnt = 1;
            }
            let dwell = if circle {
                DWELL_CIRCLE_TICKS
            } else {
                DWELL_STRAIGHT_TICKS
            };
            if self.cnt >= dwell || mode == PotMode::Hold {
                self.verdict = want;
                self.cnt = 0;
            }
        }
    }

    /// 10 Hz tick from the controller: reads the sensor values, writes
    /// `calculated.pot_climb`, `pot_predictive` and `pot_verdict`
    pub fn tick(&mut self, cm: &mut CoreModel, polar: &Polar) {
        let pot = if cm.sensor.gps_state == GpsState::NoGps {
            // no GNSS altitude, no energy balance: the box falls back to
            // the average climb
            self.reset();
            None
        } else {
            let inp = PotInput {
                roll_rad: cm.sensor.euler_roll.to_radians(),
                turn_rate_rad_s: cm.sensor.turn_rate.to_rad_s(),
                vario_m_s: cm.sensor.climb_rate.to_m_s(),
                gnss_alt_m: cm.sensor.gps_altitude.to_m(),
                ias_m_s: cm.sensor.airspeed.ias().to_m_s(),
                tas_m_s: cm.sensor.airspeed.tas().to_m_s(),
                load_factor: cm.sensor.g_force.load(),
                mc_m_s: cm.config.mc_cready.to_m_s(),
                bar_factor: cm.config.pot_bar,
                wing_area_m2: cm.glider_data.basic_glider_data.wing_area,
            };
            self.tick_raw(&inp, polar)
        };
        cm.calculated.pot_climb = pot.map(|v| v.m_s());
        // magenta = estimate (straight flight, roll-in, first circle),
        // normal colour once the climb is measured
        cm.calculated.pot_predictive =
            pot.is_some() && matches!(self.mode, PotMode::Straight | PotMode::Estimate);
        cm.calculated.pot_verdict = if pot.is_some() {
            self.verdict
        } else {
            PotVerdict::None
        };
    }
}

#[cfg(test)]
mod tests {
    //! The box inside the controller: fed from the sensor values every
    //! 100 ms, with the polar of the glider and the settings of the pilot.
    use crate::{
        controller::persist::{self, Echo, PersistenceId},
        model::{menu, GpsState, PotVerdict},
        test_harness,
        utils::Variant,
        AirSpeed, CoreController, CoreModel, Density, Editable, FloatToAcceleration, FloatToLength,
        FloatToSpeed, VarioNeedle,
    };
    use embedded_graphics::geometry::AngleUnit;

    /// fly `seconds`: bank and turn rate in rad, climb of glider and air in m/s
    fn fly(
        cm: &mut CoreModel,
        cc: &mut CoreController,
        clock: &mut u16,
        seconds: u32,
        roll: f32,
        tr: f32,
        climb: f32,
    ) {
        use crate::FloatToAngularVelocity;
        for _ in 0..seconds * 10 {
            let alt = cm.sensor.gps_altitude.to_m() + climb * 0.1;
            cm.sensor.gps_altitude = alt.m();
            cm.sensor.climb_rate = climb.m_s();
            cm.sensor.euler_roll = roll.rad();
            cm.sensor.turn_rate = tr.rad_s();
            cm.sensor.airspeed = AirSpeed::from_speeds(30.0.m_s(), 30.0.m_s());
            cm.sensor.g_force = 9.81.m_s2();
            // a sensor box is on the bus (without one the 1 s tick declares the GNSS lost)
            cm.control.can_devices = 1;
            for _ in 0..10 {
                *clock = clock.wrapping_add(10);
                cc.tick_1ms(*clock, cm);
            }
        }
    }

    /// plan_potential_v3.md A1, A4, A7: number, estimate flag and yes / no
    /// arrive in the model; without GNSS the box gives nothing
    #[test]
    fn the_controller_feeds_the_box_and_writes_its_results() {
        let (mut cm, mut cc, _tx) = test_harness!();
        cc.recalc_glider(&mut cm);
        let mut clock = 0u16;
        cm.sensor.gps_altitude = 1000.0.m();
        cm.config.mc_cready = 1.5.m_s();

        // no GNSS: no value, the display falls back to the average climb
        cm.sensor.gps_state = GpsState::NoGps;
        fly(&mut cm, &mut cc, &mut clock, 3, 0.0, 0.0, -0.7);
        assert!(cm.calculated.pot_climb.is_none());
        assert!(!cm.calculated.pot_predictive);
        assert_eq!(cm.calculated.pot_verdict, PotVerdict::None);

        // cruise in still air: the circle value (netto minus the sink in a
        // circle) is negative, an estimate, no yes / no
        cm.sensor.gps_state = GpsState::PosAvail;
        fly(&mut cm, &mut cc, &mut clock, 10, 0.0, 0.0, -0.7);
        let cruise = cm
            .calculated
            .pot_climb
            .expect("no value in cruise")
            .to_m_s();
        assert!(
            (-1.6..-0.3).contains(&cruise),
            "circle value in still air {cruise}"
        );
        assert!(cm.calculated.pot_predictive);
        assert_eq!(cm.calculated.pot_verdict, PotVerdict::None);

        // circling in 3 m/s: yes within 10 s, measured after two circles
        fly(&mut cm, &mut cc, &mut clock, 10, 0.7, 0.26, 3.0);
        assert_eq!(cm.calculated.pot_verdict, PotVerdict::Yes);
        assert!(cm.calculated.pot_predictive);
        fly(&mut cm, &mut cc, &mut clock, 40, 0.7, 0.26, 3.0);
        let v = cm.calculated.pot_climb.unwrap().to_m_s();
        assert!((v - 3.0).abs() < 0.1, "measured {v}");
        assert!(!cm.calculated.pot_predictive);
        assert_eq!(cm.calculated.pot_verdict, PotVerdict::Yes);

        // a bar of 150 % of MC 3: 4.5 m/s - the same 3 m/s is a no now
        cm.config.mc_cready = 3.0.m_s();
        persist::persist_set(
            &mut cc,
            &mut cm,
            Variant::F32(150.0),
            PersistenceId::PotBar,
            Echo::None,
        );
        assert!((cm.config.pot_bar - 1.5).abs() < 1e-6);
        fly(&mut cm, &mut cc, &mut clock, 2, 0.7, 0.26, 3.0);
        assert_eq!(cm.calculated.pot_verdict, PotVerdict::No);
    }

    /// plan_potential_v3.md A7, found with the simulator: it runs its first
    /// moments without a polar (not a number after the first
    /// recalculation), and the box showed "NaN" for the rest of the flight.
    /// An air density of 0 from the sensor box does the same to the polar.
    /// The box is silent then (the display shows the average climb), never
    /// shows a value that is not a number, and is back as soon as the polar is.
    #[test]
    fn without_a_usable_polar_the_box_is_silent_and_comes_back() {
        let (mut cm, mut cc, _tx) = test_harness!();
        let mut clock = 0u16;
        cm.sensor.gps_altitude = 1000.0.m();
        cm.sensor.gps_state = GpsState::PosAvail;
        // every 100 ms: a number or nothing
        fn checked(cm: &mut CoreModel, cc: &mut CoreController, clock: &mut u16, seconds: u32) {
            for _ in 0..seconds * 10 {
                cm.sensor.gps_altitude = (cm.sensor.gps_altitude.to_m() - 0.07).m();
                cm.sensor.climb_rate = (-0.7).m_s();
                cm.sensor.airspeed = AirSpeed::from_speeds(30.0.m_s(), 30.0.m_s());
                cm.sensor.g_force = 9.81.m_s2();
                cm.control.can_devices = 1;
                for _ in 0..10 {
                    *clock = clock.wrapping_add(10);
                    cc.tick_1ms(*clock, cm);
                }
                if let Some(v) = cm.calculated.pot_climb {
                    assert!(v.to_m_s().is_finite(), "the box shows {}", v.to_m_s());
                }
            }
        }

        // the controller was never told the glider (the simulator at start):
        // whatever the polar is in these seconds, the box shows no nonsense
        checked(&mut cm, &mut cc, &mut clock, 5);

        // the glider is known: the circle value of still air
        cc.recalc_glider(&mut cm);
        checked(&mut cm, &mut cc, &mut clock, 3);
        let v = cm
            .calculated
            .pot_climb
            .expect("no value with a polar")
            .to_m_s();
        assert!((-1.6..-0.3).contains(&v), "circle value in still air {v}");

        // air density 0 from the sensor box: silent as long as it lasts
        cm.sensor.density = Density::from_kg_m3(0.0);
        checked(&mut cm, &mut cc, &mut clock, 3);
        assert!(
            cm.calculated.pot_climb.is_none(),
            "with density 0 the box shows {:?}",
            cm.calculated.pot_climb.map(|v| v.to_m_s())
        );
        assert!(!cm.calculated.pot_predictive);
        assert_eq!(cm.calculated.pot_verdict, PotVerdict::None);
        cm.sensor.density = Density::AT_NN();
        checked(&mut cm, &mut cc, &mut clock, 3);
        let back = cm
            .calculated
            .pot_climb
            .expect("no value after the density came back")
            .to_m_s();
        assert!((back - v).abs() < 0.05, "{back} after, {v} before");
    }

    /// compatibility: the bar is stored in percent, nonsense is ignored;
    /// the retired ids of offset and memory do nothing
    #[test]
    fn the_bar_setting_is_range_checked_and_the_old_ids_are_ignored() {
        let (mut cm, mut cc, _tx) = test_harness!();
        assert!((cm.config.pot_bar - 1.0).abs() < 1e-6);
        persist::persist_set(
            &mut cc,
            &mut cm,
            Variant::F32(80.0),
            PersistenceId::PotBar,
            Echo::None,
        );
        assert!((cm.config.pot_bar - 0.8).abs() < 1e-6);
        for nonsense in [0.0, -20.0, 400.0, f32::NAN] {
            persist::persist_set(
                &mut cc,
                &mut cm,
                Variant::F32(nonsense),
                PersistenceId::PotBar,
                Echo::None,
            );
            assert!(
                (cm.config.pot_bar - 0.8).abs() < 1e-6,
                "{nonsense} was taken"
            );
        }
        let before = cm.config.pot_k;
        persist::persist_set(
            &mut cc,
            &mut cm,
            Variant::F32(1.5),
            PersistenceId::PotC0Unused,
            Echo::None,
        );
        persist::persist_set(
            &mut cc,
            &mut cm,
            Variant::Bool(false),
            PersistenceId::PotMemoryUnused,
            Echo::None,
        );
        assert_eq!(cm.config.pot_k, before);
        assert!((cm.config.pot_bar - 0.8).abs() < 1e-6);
        // the bar and the cruise volume lie below LastItem: both are written to the EEPROM
        assert!((PersistenceId::PotBar as u16) < (PersistenceId::LastItem as u16));
        assert!((PersistenceId::VolumeStf as u16) < (PersistenceId::LastItem as u16));
    }

    /// menu: Vario > POT holds the bar; the AoA factor sits next to the
    /// needle mode it belongs to and shows only with that mode
    #[test]
    fn the_menu_offers_the_bar_and_the_needle_factor_with_its_mode() {
        let (mut cm, _cc, _tx) = test_harness!();
        let pot = &menu::full::VARIO_POT;
        // 15.11.2026: the POT menu is "Experimental" - marker source, bar,
        // (16.11.2026) yes / no symbol, Return
        assert_eq!(pot.name, "Experimental");
        assert!(pot.items[0].content == menu::MenuItemContent::EditItem(Editable::AvgMarkerMode));
        assert!(pot.items[1].content == menu::MenuItemContent::EditItem(Editable::PotBar));
        assert!(pot.items[2].content == menu::MenuItemContent::EditItem(Editable::PotAdvice));
        assert_eq!(
            pot.items.len(),
            4,
            "Experimental: marker, bar, yes / no and Return"
        );
        assert_eq!(Editable::PotAdvice.name(), "POT Yes/No");
        // on in the delivery state; the setting is stored like the others
        let (mut cm2, mut cc2, _tx2) = test_harness!();
        assert!(cm2.config.pot_advice);
        persist::persist_set(
            &mut cc2,
            &mut cm2,
            Variant::Bool(false),
            PersistenceId::PotAdvice,
            Echo::None,
        );
        assert!(!cm2.config.pot_advice);
        assert!(
            (PersistenceId::PotAdvice as u16) < (PersistenceId::LastItem as u16),
            "stored in the EEPROM"
        );
        let vario = &menu::full::VARIO;
        let k = vario
            .items
            .iter()
            .find(|i| i.content == menu::MenuItemContent::EditItem(Editable::PotK))
            .expect("needle factor not in the Vario menu");
        assert_eq!(Editable::PotK.name(), "Needle Factor k");
        cm.config.vario_needle = VarioNeedle::Standard;
        assert!(!menu::item_visible(&cm, k));
        cm.config.vario_needle = VarioNeedle::EntryOptimized;
        assert!(menu::item_visible(&cm, k));
    }
}
