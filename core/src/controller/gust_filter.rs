//! Gust filter for the instant vario (needle and tone)
//!
//! Two mechanisms that complement each other (from a log analysis):
//!
//! 1. PT2 cascade (two equal PT1 stages) - calms the fast jitter; the time
//!    constant per stage scales with the IAS (reference 130 km/h, factor
//!    0.6 .. 1.3): agile while circling, filtered more in fast cruise.
//!    Stages: Weak 0.25 / Medium 0.35 / Strong 0.50 s.
//! 2. Soft knee compression - cuts the high gust peaks WITHOUT additional
//!    delay: deviations from a 2 s reference up to +-0.8 m/s pass unchanged,
//!    only the excess is compressed to 30 %. Active in straight flight only
//!    (roll blend: full below 10 deg, off above 20 deg) - while circling the
//!    real thermal texture would lie above the knee 24 - 31 % of the time and
//!    the core maxima would be damped.
//!
//! Selection (Advanced -> Vario -> "Gust Filter"):
//!   Off      = exact pass-through (behaviour as before)
//!   Peak Cut = compression only (no delay, maxima -36 .. -45 %)
//!   Weak / Medium / Strong = PT2 + compression (large peaks -50 .. -67 %)
//!
//! Only the source of the needle (ring) and the vario tone is filtered;
//! averages, speed to fly and NMEA stay untouched.

use crate::{model::GustFilter, CoreModel, FloatToSpeed};

const TICK_S: f32 = 0.1; // 10 Hz (tick_100ms)
const TAU_WEAK_S: f32 = 0.25;
const TAU_MEDIUM_S: f32 = 0.35;
const TAU_STRONG_S: f32 = 0.50;
const IAS_REF_M_S: f32 = 36.1; // 130 km/h
const IAS_FACTOR_MIN: f32 = 0.6;
const IAS_FACTOR_MAX: f32 = 1.3;
// soft knee compression
const KNEE_M_S: f32 = 0.8; // unchanged up to here
const KNEE_RATIO: f32 = 0.30; // the excess is compressed to 30 %
const REF_TAU_S: f32 = 2.0; // slow reference
const ROLL_TAU_S: f32 = 1.5; // smoothing of |roll| for the blend
const ROLL_FULL_RAD: f32 = 10.0 * core::f32::consts::PI / 180.0;
const ROLL_OFF_RAD: f32 = 20.0 * core::f32::consts::PI / 180.0;

pub struct GustPt2 {
    y1: f32,
    y2: f32,
    ref_v: f32,
    roll_f: f32,
}

impl GustPt2 {
    pub const fn new() -> Self {
        GustPt2 {
            y1: 0.0,
            y2: 0.0,
            ref_v: 0.0,
            roll_f: 0.0,
        }
    }

    /// One 100 ms tick on raw values, returns the filtered climb rate in m/s
    ///
    /// `knee_circling`: apply the knee compression while circling too
    /// (menu "Gust Circling"); otherwise the roll blend fades it out.
    pub fn tick_raw(
        &mut self,
        climb: f32,
        ias: f32,
        roll_rad: f32,
        setting: GustFilter,
        knee_circling: bool,
    ) -> f32 {
        // roll blend for the knee (always tracked)
        self.roll_f += (TICK_S / (ROLL_TAU_S + TICK_S)) * (roll_rad.abs() - self.roll_f);

        let tau = match setting {
            GustFilter::Off => {
                // passthrough; keep the stages primed for a bumpless enable
                self.y1 = climb;
                self.y2 = climb;
                self.ref_v += (TICK_S / (REF_TAU_S + TICK_S)) * (climb - self.ref_v);
                return climb;
            }
            GustFilter::PeakCut => 0.0,
            GustFilter::Weak => TAU_WEAK_S,
            GustFilter::Medium => TAU_MEDIUM_S,
            GustFilter::Strong => TAU_STRONG_S,
        };

        // stage 1: PT2 cascade (not with Peak Cut)
        let mut x = climb;
        if tau > 0.0 {
            let factor = if ias > 1.0 {
                (ias / IAS_REF_M_S).clamp(IAS_FACTOR_MIN, IAS_FACTOR_MAX)
            } else {
                1.0 // no airspeed (ground): neutral scaling
            };
            let alpha = TICK_S / (tau * factor + TICK_S);
            self.y1 += alpha * (x - self.y1);
            self.y2 += alpha * (self.y1 - self.y2);
            x = self.y2;
        } else {
            self.y1 = x;
            self.y2 = x;
        }

        // stage 2: soft knee compression around the slow reference. Without
        // the option "Gust Circling" it fades out with the bank angle
        // (straight flight only); with the option it works while circling too.
        self.ref_v += (TICK_S / (REF_TAU_S + TICK_S)) * (x - self.ref_v);
        let s = if knee_circling {
            1.0
        } else {
            (1.0 - (self.roll_f - ROLL_FULL_RAD) / (ROLL_OFF_RAD - ROLL_FULL_RAD)).clamp(0.0, 1.0)
        };
        let d = x - self.ref_v;
        let ad = d.abs();
        if ad > KNEE_M_S {
            let cut = (ad - KNEE_M_S) * (1.0 - KNEE_RATIO) * s;
            x = self.ref_v + (ad - cut) * d.signum();
        }
        x
    }

    /// Tick within `tick_100ms()`, result goes to `calculated.gust_filtered_climb`
    pub fn tick(&mut self, cm: &mut CoreModel) {
        let filtered = self.tick_raw(
            cm.sensor.climb_rate.to_m_s(),
            cm.sensor.airspeed.ias().to_m_s(),
            cm.sensor.euler_roll.to_radians(),
            cm.config.gust_filter,
            cm.config.gust_filter_circling,
        );
        cm.calculated.gust_filtered_climb = filtered.m_s();
    }
}

impl Default for GustPt2 {
    fn default() -> Self {
        Self::new()
    }
}
