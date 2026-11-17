//! Thermal entry optimized vario needle ("Vario Signal" = Entry Optimized)
//!
//! Phase controlled angle of attack correction of the instantaneous vario
//! (needle and tone). This is the needle part of the former pot_climb.rs,
//! moved here unchanged when the POT box got its own calculation
//! (0.4.1.48, plan_potential_v3.md). The POT box no longer uses any of it.
//!
//! The AoA proxy is built from signals the display already has, without a
//! CAN protocol change:
//!   flight path = atan2(baro climb rate [PT1 1.5 s], TAS)
//!   aoa_proxy   = pitch - flight path   [degrees, PT1 1.5 s]
//!   aoa_stat    = sliding median over ~30 s of established circling
//!   correction  = -k * (aoa_proxy - aoa_stat)
//! k is the "Needle Factor k" of the Vario menu (default -0.18 m/s per
//! degree, pooled over 5 installations; it can be calibrated per
//! installation from a flight log).
//!
//! Phases (all thresholds on the integrated NET turn angle):
//!   - roll-in: |bank| > 10 deg (PT1 1.5 s) and turn rate rising
//!   - full correction up to 1 circle, linear fade-out until 1.5 circles
//!   - exit: |turn rate| (PT1) below 8 deg/s for 2 s; after at least 45 deg
//!     the phase is held for 10 s, so re-centring in the same thermal
//!     continues instead of starting again
//!   - aoa_stat keeps its last value between thermals (start value 0)
//!
//! The phase factor is ramped and the effective deflection is rate limited,
//! so neither state changes nor baro glitches make the needle jump (at most
//! |k| * 8 deg/s). An entry smoothing of the needle itself (PT1 2.5 s)
//! existed from 0.4.1.2x to 0.4.1.31 and is gone since 0.4.1.32 (after a
//! flight test).

use crate::{model::VarioNeedle, CoreModel, FloatToSpeed};

#[allow(unused_imports)]
use micromath::F32Ext;

const TICK_S: f32 = 0.1; // 10 Hz (tick_100ms)
const PT1_TAU_S: f32 = 1.5;
const PT1_ALPHA: f32 = TICK_S / (PT1_TAU_S + TICK_S);
const RAD_TO_DEG: f32 = 180.0 / core::f32::consts::PI;
const ROLL_ON_RAD: f32 = 10.0 * core::f32::consts::PI / 180.0;
const TR_EST_RAD_S: f32 = 8.0 * core::f32::consts::PI / 180.0;
const SHOW_AT_RAD: f32 = 45.0 * core::f32::consts::PI / 180.0;
const RISE_LAG_TICKS: usize = 10; // 1 s
const RISE_EPS_RAD_S: f32 = 0.2 * core::f32::consts::PI / 180.0;
const EXIT_HOLD_TICKS: u32 = 20; // 2 s below the threshold
const FREEZE_TICKS: u32 = 100; // phase held for 10 s after the exit
const TAS_MIN_M_S: f32 = 15.0; // keeps atan2 stable
const MEDIAN_N: usize = 300; // ~30 s sliding median
const MEDIAN_MIN: usize = 50; // not before 5 s of circling data
const MEDIAN_EVERY: u32 = 10; // median once a second
const NEEDLE_FULL_RAD: f32 = 360.0 * core::f32::consts::PI / 180.0; // full correction up to 1 circle
const NEEDLE_FADE_END_RAD: f32 = 540.0 * core::f32::consts::PI / 180.0; // faded out at 1.5 circles

// Safety limit on the AoA deflection instead of a fixed value in m/s:
// real entries reach 12-16 deg (p99.9 of the logs), everything above is
// implausible (baro spike). The limit in m/s scales with the calibrated k.
const AOA_EX_LIMIT_DEG: f32 = 16.0;
const G_SLEW_PER_TICK: f32 = 0.1; // ramp 0 <-> 1 in 1 s
const EX_SLEW_DEG_PER_TICK: f32 = 0.8; // 8 deg/s rate limit of ex*g

#[derive(PartialEq)]
enum State {
    Idle,
    Active,
    Hold,
}

/// State of the needle correction (lives in the CoreController)
pub struct NeedleEntry {
    state: State,
    roll_f: f32,
    tr_f: f32,
    trf_hist: [f32; RISE_LAG_TICKS],
    hist_idx: usize,
    ticks: u32,
    // AoA proxy
    last_baro_alt: Option<f32>,
    baro_climb_f: f32,
    aoa_f: f32,
    aoa_stat: f32,
    med_buf: [f32; MEDIAN_N],
    med_len: usize,
    med_pos: usize,
    // phase
    net_rot: f32,
    below_ticks: u32,
    shown: bool,
    hold_ticks: u32,
    // continuous correction
    g_smooth: f32,
    ex_out: f32,
}

impl Default for NeedleEntry {
    fn default() -> Self {
        Self::new()
    }
}

impl NeedleEntry {
    pub const fn new() -> Self {
        NeedleEntry {
            state: State::Idle,
            roll_f: 0.0,
            tr_f: 0.0,
            trf_hist: [0.0; RISE_LAG_TICKS],
            hist_idx: 0,
            ticks: 0,
            last_baro_alt: None,
            baro_climb_f: 0.0,
            aoa_f: 0.0,
            aoa_stat: 0.0,
            med_buf: [0.0; MEDIAN_N],
            med_len: 0,
            med_pos: 0,
            net_rot: 0.0,
            below_ticks: 0,
            shown: false,
            hold_ticks: 0,
            g_smooth: 0.0,
            ex_out: 0.0,
        }
    }

    /// Median of the ring buffer (insertion sort on a copy, once a second)
    fn median(&self) -> f32 {
        let n = self.med_len;
        let mut buf = [0.0f32; MEDIAN_N];
        buf[..n].copy_from_slice(&self.med_buf[..n]);
        let s = &mut buf[..n];
        for i in 1..n {
            let v = s[i];
            let mut j = i;
            while j > 0 && s[j - 1] > v {
                s[j] = s[j - 1];
                j -= 1;
            }
            s[j] = v;
        }
        s[n / 2]
    }

    /// One 100 ms tick with raw values
    pub fn tick_raw(
        &mut self,
        roll_rad: f32,
        turn_rate_rad_s: f32,
        pitch_rad: f32,
        tas_m_s: f32,
        baro_alt_m: f32,
    ) {
        self.ticks = self.ticks.wrapping_add(1);

        // smoothing (PT1 ~1.5 s)
        self.roll_f += PT1_ALPHA * (roll_rad.abs() - self.roll_f);
        self.tr_f += PT1_ALPHA * (turn_rate_rad_s - self.tr_f);
        let trf_abs = self.tr_f.abs();
        let trf_old = self.trf_hist[self.hist_idx];
        self.trf_hist[self.hist_idx] = trf_abs;
        self.hist_idx = (self.hist_idx + 1) % RISE_LAG_TICKS;

        // AoA proxy from pitch and the baro flight path
        let baro_climb = match self.last_baro_alt {
            Some(last) => (baro_alt_m - last) / TICK_S,
            None => 0.0,
        };
        self.last_baro_alt = Some(baro_alt_m);
        self.baro_climb_f += PT1_ALPHA * (baro_climb - self.baro_climb_f);
        let tas = if tas_m_s > TAS_MIN_M_S {
            tas_m_s
        } else {
            TAS_MIN_M_S
        };
        let path_rad = self.baro_climb_f.atan2(tas);
        let aoa_deg = (pitch_rad - path_rad) * RAD_TO_DEG;
        self.aoa_f += PT1_ALPHA * (aoa_deg - self.aoa_f);

        // sliding median in established circling
        if trf_abs > TR_EST_RAD_S {
            self.med_buf[self.med_pos] = self.aoa_f;
            self.med_pos = (self.med_pos + 1) % MEDIAN_N;
            if self.med_len < MEDIAN_N {
                self.med_len += 1;
            }
            if self.med_len >= MEDIAN_MIN && self.ticks % MEDIAN_EVERY == 0 {
                self.aoa_stat = self.median();
            }
        }

        match self.state {
            State::Idle | State::Hold => {
                if self.state == State::Hold {
                    self.hold_ticks += 1;
                    if self.hold_ticks > FREEZE_TICKS {
                        self.state = State::Idle;
                    }
                }
                // re-entry during the hold (re-centring in the same
                // thermal): continue, the net turn angle and the median stay.
                // The condition is established circling again, not the early
                // roll-in trigger, so rolling on the way out does not
                // continue the phase by mistake.
                if self.state == State::Hold && self.roll_f > ROLL_ON_RAD && trf_abs > TR_EST_RAD_S
                {
                    self.state = State::Active;
                    self.below_ticks = 0;
                }
                // roll-in: |bank| > 10 deg and turn rate rising
                let rising = trf_abs - trf_old > RISE_EPS_RAD_S;
                if self.state == State::Idle && self.roll_f > ROLL_ON_RAD && rising {
                    self.state = State::Active;
                    self.net_rot = 0.0;
                    self.below_ticks = 0;
                    self.shown = false;
                    // rebuild the median buffer; aoa_stat keeps the last
                    // thermal as start value
                    self.med_len = 0;
                    self.med_pos = 0;
                }
            }
            State::Active => {
                // net turn angle: keep the sign, absolute value on compare
                self.net_rot += turn_rate_rad_s * TICK_S;
                if self.net_rot.abs() >= SHOW_AT_RAD {
                    self.shown = true;
                }
                // exit: turn rate below the threshold for 2 s
                if trf_abs < TR_EST_RAD_S {
                    self.below_ticks += 1;
                    if self.below_ticks >= EXIT_HOLD_TICKS {
                        if self.shown {
                            self.hold_ticks = 0;
                            self.state = State::Hold;
                        } else {
                            // given up before 45 deg
                            self.state = State::Idle;
                        }
                    }
                } else {
                    self.below_ticks = 0;
                }
            }
        }

        // follow the correction continuously (every tick): phase factor g
        // -> ramp, effective deflection ex*g -> rate limit
        let g_target = if self.state == State::Active {
            let rot = self.net_rot.abs();
            if rot <= NEEDLE_FULL_RAD {
                1.0
            } else if rot < NEEDLE_FADE_END_RAD {
                (NEEDLE_FADE_END_RAD - rot) / (NEEDLE_FADE_END_RAD - NEEDLE_FULL_RAD)
            } else {
                0.0
            }
        } else {
            0.0
        };
        self.g_smooth += (g_target - self.g_smooth).clamp(-G_SLEW_PER_TICK, G_SLEW_PER_TICK);
        let aoa_ex = (self.aoa_f - self.aoa_stat).clamp(-AOA_EX_LIMIT_DEG, AOA_EX_LIMIT_DEG);
        let p = aoa_ex * self.g_smooth;
        self.ex_out += (p - self.ex_out).clamp(-EX_SLEW_DEG_PER_TICK, EX_SLEW_DEG_PER_TICK);
    }

    /// Phase controlled AoA correction of the vario needle in m/s (a delta
    /// to the raw vario), without any delay of the vario signal itself.
    /// `k` in m/s per degree AoA.
    pub fn needle_correction(&self, k: f32) -> f32 {
        -k * self.ex_out
    }

    /// 10 Hz tick from the controller: reads the sensor values, writes
    /// `calculated.needle_correction`
    pub fn tick(&mut self, cm: &mut CoreModel) {
        self.tick_raw(
            cm.sensor.euler_roll.to_radians(),
            cm.sensor.turn_rate.to_rad_s(),
            cm.sensor.euler_pitch.to_radians(),
            cm.sensor.airspeed.tas().to_m_s(),
            cm.sensor.pressure_altitude.qne_altitude().to_m(),
        );
        let corr = if cm.config.vario_needle == VarioNeedle::EntryOptimized {
            self.needle_correction(cm.config.pot_k)
        } else {
            0.0
        };
        cm.calculated.needle_correction = corr.m_s();
    }
}
