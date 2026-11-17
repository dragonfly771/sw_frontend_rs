//! Flap stages (camber flaps) of the glider: table, recommendation, sensor
//! position and the demo for the fair.
//!
//! Plan: plan_klappen_menue.md and plan_ring_design_p1.md chapter 3.
//!
//! The table is ordered from SLOW to FAST (index 0 is
//! the slowest stage, e.g. "L" on the AS 33). Every stage carries the upper
//! switch speed ("Up to") at the reference weight of the polar; the lower
//! limit is the "Up to" of the next slower stage. The fastest stage has no
//! upper limit.
//!
//! This module has no dependencies on the rest of the crate, so it can be
//! tested on its own (`rustc --test --cfg flaps_standalone`). All speeds
//! are km/h, masses kg.

#[cfg(not(flaps_standalone))]
use crate::utils::FClamp;
#[allow(unused_imports)]
use micromath::F32Ext;

/// maximum number of flap stages
pub const MAX_FLAPS: usize = 12;
/// maximum length of a flap name
pub const FLAP_NAME_LEN: usize = 3;

/// characters a flap name may use (code = index + 1, 0 = no character)
pub const FLAP_CHARSET: &[u8] = b"+-0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ./ ";

/// hysteresis at every switch speed (at reference weight)
pub const FLAP_HYSTERESIS_KMH: f32 = 3.0;

/// where the actual flap position comes from
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum FlapSensorSource {
    /// no flap sensor: the tape centres on the recommendation
    #[default]
    None = 0,
    /// LARUS flap sensor on the CAN bus (hardware in development)
    Larus = 1,
}

impl From<u8> for FlapSensorSource {
    fn from(v: u8) -> Self {
        match v {
            1 => FlapSensorSource::Larus,
            _ => FlapSensorSource::None,
        }
    }
}

impl FlapSensorSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            FlapSensorSource::None => "None",
            FlapSensorSource::Larus => "LARUS",
        }
    }
}

/// one flap stage
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FlapStage {
    /// name, ASCII from FLAP_CHARSET, 0 terminated if shorter
    pub name: [u8; FLAP_NAME_LEN],
    /// upper switch speed at reference weight, km/h; 0 = no limit
    pub up_to_kmh: u16,
    /// sensor position of this stage (degrees of the flap sensor), NaN = not set
    pub position: f32,
}

impl Default for FlapStage {
    fn default() -> Self {
        FlapStage {
            name: [0; FLAP_NAME_LEN],
            up_to_kmh: 0,
            position: f32::NAN,
        }
    }
}

fn char_code(c: u8) -> u32 {
    match FLAP_CHARSET.iter().position(|&x| x == c) {
        Some(i) => i as u32 + 1,
        None => 0,
    }
}

fn code_char(code: u32) -> u8 {
    if code == 0 {
        0
    } else {
        FLAP_CHARSET.get(code as usize - 1).copied().unwrap_or(0)
    }
}

impl FlapStage {
    pub fn new(name: &str, up_to_kmh: u16) -> Self {
        let mut s = FlapStage::default();
        s.set_name(name);
        s.up_to_kmh = up_to_kmh;
        s
    }

    /// set the name; characters outside the charset are dropped
    pub fn set_name(&mut self, name: &str) {
        self.name = [0; FLAP_NAME_LEN];
        let mut i = 0;
        for c in name.bytes() {
            let c = c.to_ascii_uppercase();
            if i < FLAP_NAME_LEN && char_code(c) != 0 {
                self.name[i] = c;
                i += 1;
            }
        }
    }

    /// the name as &str (trailing blanks removed)
    pub fn name_str(&self) -> &str {
        let n = self
            .name
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(FLAP_NAME_LEN);
        let s = core::str::from_utf8(&self.name[..n]).unwrap_or("");
        s.trim_end()
    }

    pub fn has_position(&self) -> bool {
        !self.position.is_nan()
    }

    /// pack name and switch speed into one EEPROM word
    pub fn word(&self) -> u32 {
        let mut w = 0u32;
        for (i, c) in self.name.iter().enumerate() {
            w |= (char_code(*c) & 0x3f) << (6 * i);
        }
        w |= ((self.up_to_kmh as u32).min(511)) << 18;
        w | (1 << 27)
    }

    pub fn from_word(w: u32, position: f32) -> Self {
        let mut s = FlapStage::default();
        for i in 0..FLAP_NAME_LEN {
            s.name[i] = code_char((w >> (6 * i)) & 0x3f);
        }
        s.up_to_kmh = ((w >> 18) & 0x1ff) as u16;
        s.position = position;
        s
    }
}

/// the flap table of the glider
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FlapTable {
    /// number of stages in use (0 = glider without camber flaps)
    pub n: u8,
    pub stages: [FlapStage; MAX_FLAPS],
    /// slowest stage used for circling; while circling the recommendation
    /// stays on this stage or the next faster one
    pub circling: u8,
    pub sensor: FlapSensorSource,
}

impl Default for FlapTable {
    fn default() -> Self {
        FlapTable {
            n: 0,
            stages: [FlapStage::default(); MAX_FLAPS],
            circling: 0,
            sensor: FlapSensorSource::None,
        }
    }
}

impl FlapTable {
    /// Default table of the AS 33 18m (Stefan, 28.10.2026), fast to slow,
    /// "ab" = lower switch speed at the reference weight of the polar (400 kg):
    /// 1 from 185, 2 from 165, 3 from 150, 4 from 125, N from 118, 5 from 100
    /// (circling), 6 below; L is the landing stage only, below 70 km/h, so it
    /// is never recommended in flight. While circling the recommendation
    /// stays on 5 (or N when fast). Flap sensor LARUS preselected, with
    /// invented sensor positions until the sensor and the real table exist.
    pub fn as33_placeholder() -> Self {
        let mut t = FlapTable::default();
        // name, "up to" (= "from" of the next faster stage), sensor position
        let stages = [
            ("L", 70, 30.0),
            ("6", 100, 16.0),
            ("5", 118, 12.0),
            ("N", 125, 8.0),
            ("4", 150, 4.0),
            ("3", 165, 0.0),
            ("2", 185, -4.0),
            ("1", 0, -8.0),
        ];
        for (i, (name, up_to, pos)) in stages.iter().enumerate() {
            t.stages[i] = FlapStage::new(name, *up_to);
            t.stages[i].position = *pos;
        }
        t.n = stages.len() as u8;
        t.circling = 2; // "5": circling on 5 (or N)
        t.sensor = FlapSensorSource::Larus;
        t
    }

    /// the EEPROM word of the table settings
    pub fn word(&self) -> u32 {
        (self.n as u32 & 0x0f)
            | ((self.circling as u32 & 0x0f) << 4)
            | ((self.sensor as u32 & 0x03) << 8)
            | (1 << 10)
    }

    pub fn set_word(&mut self, w: u32) {
        self.n = ((w & 0x0f) as u8).min(MAX_FLAPS as u8);
        self.circling = ((w >> 4) & 0x0f) as u8;
        self.sensor = FlapSensorSource::from(((w >> 8) & 0x03) as u8);
    }

    /// restore one stage from its EEPROM word, the position stays
    pub fn set_stage_word(&mut self, i: usize, w: u32) {
        if let Some(st) = self.stages.get_mut(i) {
            let pos = st.position;
            *st = FlapStage::from_word(w, pos);
        }
    }

    pub fn len(&self) -> usize {
        (self.n as usize).min(MAX_FLAPS)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn stage(&self, i: usize) -> Option<&FlapStage> {
        if i < self.len() {
            self.stages.get(i)
        } else {
            None
        }
    }

    pub fn name(&self, i: usize) -> &str {
        self.stage(i).map(|s| s.name_str()).unwrap_or("")
    }

    /// lower and upper switch speed of stage i (0 = open end)
    pub fn range(&self, i: usize) -> (u16, u16) {
        let lo = if i == 0 {
            0
        } else {
            self.stage(i - 1).map(|s| s.up_to_kmh).unwrap_or(0)
        };
        let hi = if i + 1 >= self.len() {
            0
        } else {
            self.stage(i).map(|s| s.up_to_kmh).unwrap_or(0)
        };
        (lo, hi)
    }

    /// allowed range of the "Up to" value of stage i: above the slower
    /// neighbour, below the faster one
    pub fn up_to_limits(&self, i: usize) -> (u16, u16) {
        let lo = if i == 0 {
            40
        } else {
            self.stage(i - 1).map(|s| s.up_to_kmh + 1).unwrap_or(40)
        };
        let hi = if i + 2 >= self.len() {
            400
        } else {
            self.stage(i + 1)
                .map(|s| s.up_to_kmh.saturating_sub(1))
                .unwrap_or(400)
        };
        (lo, hi.max(lo))
    }

    /// add a stage at the fast end (copy of the fastest one, 20 km/h faster)
    pub fn add_stage(&mut self) -> bool {
        let n = self.len();
        if n >= MAX_FLAPS {
            return false;
        }
        let mut s = FlapStage::new("-", 0);
        if n > 0 {
            // the former fastest stage gets an upper limit now
            let prev_lo = self.range(n - 1).0;
            let up = if prev_lo > 0 { prev_lo + 20 } else { 120 };
            self.stages[n - 1].up_to_kmh = up;
            s.up_to_kmh = 0;
        }
        self.stages[n] = s;
        self.n += 1;
        true
    }

    /// remove stage i, the following stages move up
    pub fn delete_stage(&mut self, i: usize) -> bool {
        let n = self.len();
        if i >= n {
            return false;
        }
        for k in i..n - 1 {
            self.stages[k] = self.stages[k + 1];
        }
        self.stages[n - 1] = FlapStage::default();
        self.n -= 1;
        if self.circling as usize > i {
            self.circling -= 1;
        }
        let n = self.len();
        if n > 0 {
            self.stages[n - 1].up_to_kmh = 0;
        }
        if self.circling as usize >= n {
            self.circling = n.saturating_sub(1) as u8;
        }
        true
    }

    /// recommended stage
    ///
    /// `ias_kmh`: indicated airspeed, `mass_ratio`: actual mass / reference
    /// weight (switch speeds scale with sqrt of it), `prev`: the stage
    /// recommended last time (hysteresis). Returns None without a table.
    pub fn recommend(
        &self,
        ias_kmh: f32,
        mass_ratio: f32,
        circling: bool,
        prev: Option<usize>,
    ) -> Option<usize> {
        let n = self.len();
        if n == 0 {
            return None;
        }
        let ratio = if mass_ratio > 0.2 && mass_ratio < 5.0 {
            mass_ratio
        } else {
            1.0
        };
        // speed at reference weight, compared squared (no sqrt needed)
        let v2 = ias_kmh.max(0.0) * ias_kmh.max(0.0) / ratio;
        let below = |limit: f32| limit <= 0.0 || v2 < limit * limit;

        let mut rec = n - 1;
        for i in 0..n {
            let (_, hi) = self.range(i);
            if below(hi as f32) {
                rec = i;
                break;
            }
        }
        // hysteresis: stay on the previous stage while within its range +/- 3 km/h
        if let Some(p) = prev {
            if p < n && p != rec {
                let (lo, hi) = self.range(p);
                let lo_h = lo as f32 - FLAP_HYSTERESIS_KMH;
                let hi_h = hi as f32 + FLAP_HYSTERESIS_KMH;
                let above_lo = lo == 0 || v2 >= lo_h.max(0.0) * lo_h.max(0.0);
                let below_hi = hi == 0 || v2 < hi_h * hi_h;
                if above_lo && below_hi {
                    rec = p;
                }
            }
        }
        if circling {
            let c = (self.circling as usize).min(n - 1);
            let c_fast = (c + 1).min(n - 1);
            rec = rec.clamp(c, c_fast);
        }
        Some(rec)
    }

    /// actual flap position as fractional stage index from the sensor value
    ///
    /// Linear between the stored positions of neighbouring stages, clamped
    /// at both ends. Works for rising and falling sensor values. None if
    /// fewer than two stages have a stored position.
    pub fn position_index(&self, sensor_deg: f32) -> Option<f32> {
        if sensor_deg.is_nan() {
            return None;
        }
        let n = self.len();
        let mut last: Option<(usize, f32)> = None;
        let mut first: Option<(usize, f32)> = None;
        let mut best: Option<f32> = None;
        let mut best_err = f32::MAX;
        for i in 0..n {
            let p = self.stages[i].position;
            if p.is_nan() {
                continue;
            }
            if first.is_none() {
                first = Some((i, p));
            }
            if let Some((j, q)) = last {
                let (a, b) = if q <= p { (q, p) } else { (p, q) };
                if sensor_deg >= a && sensor_deg <= b && b > a {
                    let f = (sensor_deg - q) / (p - q);
                    return Some(j as f32 + f * (i - j) as f32);
                }
                // outside: remember the nearest end
                for (k, v) in [(j, q), (i, p)] {
                    let e = (sensor_deg - v).abs();
                    if e < best_err {
                        best_err = e;
                        best = Some(k as f32);
                    }
                }
            }
            last = Some((i, p));
        }
        if first.is_some() && last.is_some() && first.map(|f| f.0) != last.map(|l| l.0) {
            best
        } else {
            None
        }
    }
}

/// state of the demo for the fair (see plan_klappen_menue.md chapter 4)
///
/// One loop of 60 s: 35 s cruise with the speed gliding 95 -> 185 -> 120
/// km/h, then 25 s circling at 95 km/h. The simulated flap lever follows
/// the recommendation 1.5 s later with 0.8 s travel per stage.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct FlapDemo {
    /// ms since the demo was started
    pub t_ms: u32,
    /// simulated lever, fractional stage index
    pub lever: f32,
    lever_target: f32,
    /// ms the recommendation has been different from the lever target
    pending_ms: u32,
    last_rec: Option<usize>,
    /// simulated altitude above sea level (QNH), m
    pub alt_m: f32,
    /// mean climb of one loop, m/s: taken off the altitude so it stays
    /// around its start value however long the demo runs
    drift_ms: f32,
    /// QNH before the demo, restored when it ends (hPa, 0 = not set)
    pub qnh_before_hpa: f32,
    /// energy arrow factor before the demo (it is switched on for the demo
    /// when it was off), restored when it ends
    pub energy_mult_before: f32,
}

/// energy arrow factor of the flight demo when the setting is 0 (off)
pub const DEMO_ENERGY_MULT: f32 = 2.0;

/// start altitude of the flight demo, m QNH
pub const DEMO_ALT_M: f32 = 1250.0;
/// QNH of the flight demo, hPa
pub const DEMO_QNH_HPA: f32 = 1018.0;

/// wind of the flight demo: (live direction deg, live km/h, average
/// direction deg, average km/h). The live wind swings a little around the
/// average one, so the energy arrow (difference of the two) moves.
pub fn demo_wind(t_ms: u32) -> (f32, f32, f32, f32) {
    let avg_dir = 265.0 + 3.0 * wiggle(t_ms, 61_000);
    let avg_spd = 22.0 + 1.0 * wiggle(t_ms, 47_000);
    let dir = avg_dir + 12.0 * wiggle(t_ms, 7_300);
    let spd = avg_spd + 5.0 * wiggle(t_ms, 5_100) + 1.5;
    (dir, spd, avg_dir, avg_spd)
}

/// one frame of the demo
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FlapDemoFrame {
    pub ias_kmh: f32,
    pub climb_ms: f32,
    pub circling: bool,
    /// deg/s, sign = circling direction
    pub turn_rate_dps: f32,
    /// what the sensor box would send as its average climb
    pub avg_ms: f32,
}

/// 35 s of cruise and 85 s in the thermal: a good four circles of 20 s, so
/// the centering page shows the track of the first one and a half circles
/// and then its aid for more than two (until 0.4.1.53: 25 s of circling
/// with a climb that did not depend on the heading - the centering aids had
/// nothing to show; the simulator data should be closer to a real flight).
pub const DEMO_LOOP_MS: u32 = 120_000;
/// roll into the circle and out of it (ms)
pub const DEMO_ROLL_IN_MS: u32 = 3_000;
pub const DEMO_ROLL_OUT_MS: u32 = 2_000;
/// turn rate in the circle, degrees per second
pub const DEMO_TURN_DPS: f32 = 18.0;
pub const DEMO_CRUISE_MS: u32 = 35_000;

fn smooth(a: f32, b: f32, f: f32) -> f32 {
    let f = f.fclamp(0.0, 1.0);
    let s = f * f * (3.0 - 2.0 * f);
    a + (b - a) * s
}

/// climb of the demo in straight flight at `t` ms of the loop
fn cruise_climb(t: u32, ias: f32) -> f32 {
    let hat = |centre: f32, half: f32| (1.0 - ((t as f32 - centre) / half).abs()).max(0.0);
    -0.8 - (ias - 95.0) * 0.018
        + 0.4 * wiggle(t, 3_700)
        + 2.2 * hat(26_000.0, 2_000.0)
        + 3.6 * hat(33_300.0, 2_800.0)
}

/// small deterministic wiggle (no sin table needed, period ~ 4 s)
fn wiggle(t_ms: u32, period_ms: u32) -> f32 {
    let p = (t_ms % period_ms) as f32 / period_ms as f32; // 0..1
                                                          // triangle -> smoothed to a sine like shape
    let tri = if p < 0.5 {
        4.0 * p - 1.0
    } else {
        3.0 - 4.0 * p
    };
    tri * (1.5 - 0.5 * tri * tri)
}

impl FlapDemo {
    pub fn new() -> Self {
        let mut d = FlapDemo {
            alt_m: DEMO_ALT_M,
            ..FlapDemo::default()
        };
        // mean climb over one loop
        let mut sum = 0.0;
        let n = DEMO_LOOP_MS / 100;
        for k in 0..n {
            d.t_ms = k * 100;
            sum += d.frame().climb_ms;
        }
        d.t_ms = 0;
        d.drift_ms = sum / n as f32;
        d
    }

    /// advance the simulated altitude by dt_ms with the climb of the frame
    pub fn climb(&mut self, dt_ms: u32, climb_ms: f32) {
        self.alt_m += (climb_ms - self.drift_ms) * dt_ms as f32 * 0.001;
        self.alt_m = self.alt_m.fclamp(300.0, 4000.0);
    }

    /// flight values at the current demo time
    pub fn frame(&self) -> FlapDemoFrame {
        let t = self.t_ms % DEMO_LOOP_MS;
        if t < DEMO_CRUISE_MS {
            // 0..12 s accelerate 95 -> 185, 12..20 hold, 20..32 slow to 120, 32..35 to 95
            let ias = if t < 12_000 {
                smooth(95.0, 185.0, t as f32 / 12_000.0)
            } else if t < 20_000 {
                185.0
            } else if t < 32_000 {
                smooth(185.0, 120.0, (t - 20_000) as f32 / 12_000.0)
            } else {
                smooth(120.0, 95.0, (t - 32_000) as f32 / 3_000.0)
            };
            // sinking air while fast, a short bump at 26 s, and the lift of
            // the thermal on the way in: from 30.5 s, strongest 1.7 s
            // before the turn begins
            let climb = cruise_climb(t, ias);
            FlapDemoFrame {
                ias_kmh: ias,
                climb_ms: climb,
                circling: false,
                turn_rate_dps: 0.0,
                avg_ms: -0.9,
            }
        } else {
            let tc = t - DEMO_CRUISE_MS;
            let left = DEMO_LOOP_MS - t;
            // roll in over 3 s, out over the last 2 s
            let turn = DEMO_TURN_DPS
                * (tc as f32 / DEMO_ROLL_IN_MS as f32)
                    .min(1.0)
                    .min(left as f32 / DEMO_ROLL_OUT_MS as f32);
            // heading turned since the turn began (degrees)
            let s = tc as f32 * 0.001;
            let roll_in = DEMO_ROLL_IN_MS as f32 * 0.001;
            let turned = if s < roll_in {
                0.5 * DEMO_TURN_DPS * s * s / roll_in
            } else {
                DEMO_TURN_DPS * (s - 0.5 * roll_in)
            };
            // A thermal with its core to one side: the climb goes around
            // with the heading, strongest 60 degrees after the entry
            // heading. Over the circles the pilot centres - the difference
            // around the circle shrinks from 1.6 to 0.4 m/s and the mean
            // climb grows from 1.8 to 2.9 m/s. A little gust on top.
            let amp = smooth(1.6, 0.4, s / 70.0);
            let mean = smooth(1.8, 2.9, s / 60.0);
            let thermal =
                mean + amp * (turned - 60.0).to_radians().cos() + 0.25 * wiggle(tc, 2_300);
            // the first 2 s lead over from the last value of the approach,
            // the last 2 s to the first value of the next cruise
            let lead = (s / 2.0).min(1.0);
            let climb = cruise_climb(DEMO_CRUISE_MS, 95.0) * (1.0 - lead) + thermal * lead;
            let out = (left as f32 / DEMO_ROLL_OUT_MS as f32).min(1.0);
            let climb = cruise_climb(0, 95.0) * (1.0 - out) + climb * out;
            FlapDemoFrame {
                ias_kmh: 95.0 + 2.0 * wiggle(tc, 5_300),
                climb_ms: climb,
                circling: true,
                turn_rate_dps: turn,
                avg_ms: mean,
            }
        }
    }

    /// advance the demo by dt_ms; `rec` is the recommendation for the new
    /// frame, the lever follows it
    pub fn step(&mut self, dt_ms: u32, rec: Option<usize>) {
        self.t_ms = (self.t_ms + dt_ms) % (DEMO_LOOP_MS * 1000);
        if let Some(r) = rec {
            let r = r as f32;
            if self.last_rec != rec {
                self.pending_ms = 0;
                self.last_rec = rec;
            }
            if (r - self.lever_target).abs() > 0.01 {
                self.pending_ms += dt_ms;
                // the pilot reacts after 1.5 s
                if self.pending_ms >= 1_500 {
                    self.lever_target = r;
                    self.pending_ms = 0;
                }
            }
        }
        // 0.8 s per stage
        let max_step = dt_ms as f32 / 800.0;
        let d = self.lever_target - self.lever;
        if d.abs() <= max_step {
            self.lever = self.lever_target;
        } else {
            self.lever += max_step * d.signum();
        }
    }

    /// start the lever on a stage (no travel at the beginning)
    pub fn set_lever(&mut self, idx: usize) {
        self.lever = idx as f32;
        self.lever_target = idx as f32;
        self.last_rec = Some(idx);
        self.pending_ms = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_and_unpack() {
        let s = FlapStage::new("+2", 90);
        let t = FlapStage::from_word(s.word(), 12.5);
        assert_eq!(t.name_str(), "+2");
        assert_eq!(t.up_to_kmh, 90);
        assert_eq!(t.position, 12.5);
        let s = FlapStage::new("s1x", 511);
        let t = FlapStage::from_word(s.word(), f32::NAN);
        assert_eq!(t.name_str(), "S1X");
        assert_eq!(t.up_to_kmh, 511);
        assert!(!t.has_position());
        let tab = FlapTable::as33_placeholder();
        assert_eq!(tab.sensor, FlapSensorSource::Larus);
        let mut u = FlapTable::default();
        u.set_word(tab.word());
        assert_eq!((u.n, u.circling, u.sensor), (8, 2, FlapSensorSource::Larus));
    }

    #[test]
    fn as33_recommendation() {
        let t = FlapTable::as33_placeholder();
        let name = |v: f32, circ: bool| t.name(t.recommend(v, 1.0, circ, None).unwrap());
        assert_eq!(name(65.0, false), "L");
        assert_eq!(name(90.0, false), "6");
        assert_eq!(name(110.0, false), "5");
        assert_eq!(name(120.0, false), "N");
        assert_eq!(name(140.0, false), "4");
        assert_eq!(name(155.0, false), "3");
        assert_eq!(name(175.0, false), "2");
        assert_eq!(name(230.0, false), "1");
        // circling: 5, when fast N - never 6 or L
        assert_eq!(name(70.0, true), "5");
        assert_eq!(name(95.0, true), "5");
        assert_eq!(name(130.0, true), "N");
        // the invented sensor positions give the stages back
        for i in 0..t.len() {
            assert_eq!(t.position_index(t.stages[i].position), Some(i as f32));
        }
    }

    #[test]
    fn mass_scaling_and_hysteresis() {
        let t = FlapTable::as33_placeholder();
        // 30 % heavier: switch 4 -> 3 at 150 * sqrt(1.3) = 171.0 km/h
        let r = t.recommend(168.0, 1.3, false, None).unwrap();
        assert_eq!(t.name(r), "4");
        let r = t.recommend(173.0, 1.3, false, None).unwrap();
        assert_eq!(t.name(r), "3");
        // hysteresis: 5 reaches up to 118, stays until 121
        let i5 = 2;
        assert_eq!(t.recommend(120.0, 1.0, false, Some(i5)), Some(i5));
        assert_eq!(t.recommend(122.0, 1.0, false, Some(i5)), Some(3));
        assert_eq!(t.recommend(116.0, 1.0, false, Some(3)), Some(3));
        assert_eq!(t.recommend(114.0, 1.0, false, Some(3)), Some(2));
    }

    #[test]
    fn sensor_interpolation() {
        let mut t = FlapTable::as33_placeholder();
        for i in 0..8 {
            t.stages[i].position = f32::NAN;
        }
        assert_eq!(t.position_index(10.0), None);
        for i in 0..8 {
            t.stages[i].position = 100.0 - 10.0 * i as f32; // falling values
        }
        assert_eq!(t.position_index(100.0), Some(0.0));
        assert_eq!(t.position_index(85.0), Some(1.5));
        assert_eq!(t.position_index(30.0), Some(7.0));
        assert_eq!(t.position_index(150.0), Some(0.0));
        assert_eq!(t.position_index(-20.0), Some(7.0));
    }

    #[test]
    fn add_and_delete() {
        let mut t = FlapTable::default();
        assert!(t.add_stage());
        assert!(t.add_stage());
        assert_eq!(t.len(), 2);
        assert_eq!(t.stages[0].up_to_kmh, 120);
        assert_eq!(t.stages[1].up_to_kmh, 0);
        let mut t = FlapTable::as33_placeholder();
        assert!(t.delete_stage(7));
        assert_eq!(t.len(), 7);
        assert_eq!(t.stages[6].up_to_kmh, 0, "new fastest stage is open ended");
        assert_eq!(t.up_to_limits(3), (119, 149));
    }

    #[test]
    fn demo_runs_through_both_phases() {
        let t = FlapTable::as33_placeholder();
        let mut d = FlapDemo::new();
        d.set_lever(2);
        let mut seen_fast = false;
        let mut seen_circling = false;
        let mut rec = None;
        for _ in 0..(DEMO_LOOP_MS / 100) {
            let f = d.frame();
            rec = t.recommend(f.ias_kmh, 1.0, f.circling, rec);
            d.step(100, rec);
            assert!(f.ias_kmh >= 90.0 && f.ias_kmh <= 190.0);
            assert!(d.lever >= 0.0 && d.lever <= 7.0);
            if t.name(rec.unwrap()) == "2" {
                seen_fast = true;
            }
            if f.circling {
                seen_circling = true;
                assert!(matches!(t.name(rec.unwrap()), "5" | "N"));
            }
        }
        assert!(seen_fast && seen_circling);
    }

    /// The simulation data should be close to a real flight, so that the
    /// centering aid shows something.
    #[test]
    fn the_demo_flies_into_a_thermal_with_its_core_to_one_side_and_centres_it() {
        let mut d = FlapDemo::new();
        let at = |d: &mut FlapDemo, ms: u32| {
            d.t_ms = ms;
            d.frame()
        };
        // lift on the way in: more than 2 m/s within the 5 s before the turn
        let approach = (300..350)
            .map(|k| at(&mut d, k * 100).climb_ms)
            .fold(f32::MIN, f32::max);
        assert!(approach > 2.0, "{approach} m/s on the approach");
        assert!(!at(&mut d, DEMO_CRUISE_MS - 100).circling && at(&mut d, DEMO_CRUISE_MS).circling);
        // the turn rolls in and out, and no value jumps from one frame to the next
        assert!(at(&mut d, DEMO_CRUISE_MS + 100).turn_rate_dps < 1.0);
        assert!((at(&mut d, DEMO_CRUISE_MS + 5_000).turn_rate_dps - DEMO_TURN_DPS).abs() < 0.01);
        assert!(at(&mut d, DEMO_LOOP_MS - 100).turn_rate_dps < 1.0);
        let mut last = at(&mut d, 0);
        let mut turned = 0.0;
        for k in 1..(DEMO_LOOP_MS / 100) {
            let f = at(&mut d, k * 100);
            assert!(
                (f.climb_ms - last.climb_ms).abs() < 0.45,
                "climb jumps at {} s",
                k as f32 / 10.0
            );
            assert!(
                (f.turn_rate_dps - last.turn_rate_dps).abs() < 1.0,
                "turn rate jumps at {} s",
                k as f32 / 10.0
            );
            turned += f.turn_rate_dps * 0.1;
            last = f;
        }
        // more than four circles: 1.5 for the track, the rest for the aid of the page
        assert!(turned > 4.0 * 360.0, "{turned} degrees");
        // around a circle the climb differs: by about 3 m/s in the first
        // circle, by about 1 m/s in the last - the pilot has centred
        let spread = |d: &mut FlapDemo, from_s: u32| {
            let v: std::vec::Vec<f32> = (0..200)
                .map(|k| at(d, DEMO_CRUISE_MS + from_s * 1000 + k * 100).climb_ms)
                .collect();
            v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
        };
        let (first, late) = (spread(&mut d, 3), spread(&mut d, 62));
        assert!(first > 2.4 && first < 4.0, "first circle {first}");
        assert!(late > 0.6 && late < 1.6, "last circle {late}");
        // and it is the heading that decides, not the time: one circle
        // later (20 s) the same side is the good one again
        let good = at(&mut d, DEMO_CRUISE_MS + 5_000).climb_ms;
        let bad = at(&mut d, DEMO_CRUISE_MS + 15_000).climb_ms;
        let good_again = at(&mut d, DEMO_CRUISE_MS + 25_000).climb_ms;
        assert!(
            good > bad + 1.5 && good_again > bad + 1.0,
            "{good} {bad} {good_again}"
        );
        // the average of the box follows the climb of the thermal
        assert!(
            at(&mut d, DEMO_CRUISE_MS + 70_000).avg_ms
                > at(&mut d, DEMO_CRUISE_MS + 5_000).avg_ms + 0.8
        );
    }

    #[test]
    fn demo_altitude_and_wind() {
        let mut d = FlapDemo::new();
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for _ in 0..(10 * DEMO_LOOP_MS / 100) {
            let f = d.frame();
            d.climb(100, f.climb_ms);
            d.t_ms += 100;
            lo = lo.min(d.alt_m);
            hi = hi.max(d.alt_m);
        }
        // ten loops: stays around the start, but really climbs and sinks
        assert!((d.alt_m - DEMO_ALT_M).abs() < 5.0, "{}", d.alt_m);
        assert!(hi - lo > 30.0 && hi - lo < 250.0, "{lo} {hi}");
        let mut diff_max: f32 = 0.0;
        for t in (0..60_000).step_by(500) {
            let (dir, spd, adir, aspd) = demo_wind(t);
            assert!((dir - adir).abs() <= 13.0 && (spd - aspd).abs() <= 7.0);
            diff_max = diff_max.max((spd - aspd).abs());
        }
        assert!(diff_max > 3.0);
    }
}
