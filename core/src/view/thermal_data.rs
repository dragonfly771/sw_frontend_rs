use core::{default::Default, f32::consts::PI};

use super::trail::Trail;
use crate::{model::CirclingDirection, CanConfigId, Colors, CoreModel, FlyMode, VarioMode};

#[allow(unused_imports)]
use micromath::F32Ext;

pub const THERMAL_DATA_CNT: usize = 24;
pub const DELTA_ALPHA: f32 = 2.0 * PI / THERMAL_DATA_CNT as f32;

/// Colours of the spider assistant, from the inside to the outside: dark
/// red in the middle, over red and orange red to the orange of the ring
/// needle at the rim. The colour depends on the radius alone (see
/// `centerview::draw_spider`), so it is a fixed scale of the climb.
pub const HEAT: [Colors; 30] = [
    Colors::Heat0,
    Colors::Heat1,
    Colors::Heat2,
    Colors::Heat3,
    Colors::Heat4,
    Colors::Heat5,
    Colors::Heat6,
    Colors::Heat7,
    Colors::Heat8,
    Colors::Heat9,
    Colors::Heat10,
    Colors::Heat11,
    Colors::Heat12,
    Colors::Heat13,
    Colors::Heat14,
    Colors::Heat15,
    Colors::Heat16,
    Colors::Heat17,
    Colors::Heat18,
    Colors::Heat19,
    Colors::Heat20,
    Colors::Heat21,
    Colors::Heat22,
    Colors::Heat23,
    Colors::Heat24,
    Colors::Heat25,
    Colors::Heat26,
    Colors::Heat27,
    Colors::Heat28,
    Colors::Heat29,
];

/// The vario reading lags the air by about this fraction of the sensor
/// box's vario time constant (setup: "Vario Filter"): 0.6 s were measured at a TC of
/// 2 s (one flight, vario against the position based lift).
pub const VARIO_LAG_PER_TC: f32 = 0.3;
/// used until the sensor box has answered the request for its Vario TC
pub const VARIO_TC_DEFAULT: f32 = 2.0;
/// Reaction of pilot and glider: an action arc is reached this long before
/// the ideal moment.
pub const REACTION_TIME: f32 = 1.5;
/// Core offset (in circle radii) from which the arcs are shown; below
/// `CORE_OFF_OFFSET` they go again (hysteresis, so a value hovering around
/// the threshold does not blink). 15 flights / 1703 circles: a shift
/// towards the estimated core pays off (+0.2 m/s per radius in the next
/// circle) from 0.15 on, and no higher threshold separates better.
pub const CORE_MIN_OFFSET: f32 = 0.15;
const CORE_OFF_OFFSET: f32 = 0.11;
/// Half length of an action arc: 40 degrees at a small offset, growing to
/// 60 degrees at an offset of `ARC_OFFSET_FULL` circle radii and more.
/// Flattening moves the circle with cos(heading - core direction), so
/// within 60 degrees it still works at more than half; a pilot's action of
/// 5 s covers 85 degrees at 17 degrees per second.
const ARC_HALF_MIN: f32 = 40.0 * PI / 180.0;
const ARC_HALF_MAX: f32 = 60.0 * PI / 180.0;
const ARC_OFFSET_FULL: f32 = 0.6;
/// Bank change (degree seconds), projected on the direction of the core,
/// that shifts the circle by one radius: measured on 15 flights, 1226
/// circles, r = 0.79 (94..156 per flight; a kinematic simulation of a sine
/// modulation gives 90).
const BANK_SECONDS_PER_RADIUS: f32 = 118.0;
/// Fraction of the estimated offset one circle is meant to correct - half,
/// as the pilots do it (0.4 radii per circle at typical offsets); the rest
/// comes with the next circle and its fresh estimate.
const CORRECTION_PER_CIRCLE: f32 = 0.5;
/// The progress towards the correction fades with about one circle, so
/// only the recent action counts (per 100 ms tick).
const PROGRESS_DECAY: f32 = 0.995;
/// Sink of the glider in the circle, added to the mean climb of the circle
/// to get the strength of the thermal the harmonic is compared with.
const CIRCLING_SINK: f32 = 0.8;
/// best and worst sector are named from this many flown sectors on
const BEST_WORST_MIN_SECTORS: usize = 3;
/// sentinel of `best_pos` / `worst_pos`: no such sector yet
const NO_SECTOR: usize = usize::MAX;

/// Where the core of the thermal is, from the sector means of one circle
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CoreEstimate {
    /// heading (radians) at which the glider is nearest to the core, the
    /// vario latency taken out
    pub heading: f32,
    /// amplitude of the first harmonic of the sector means (m/s)
    pub amplitude: f32,
    /// offset of the core from the circle centre, in circle radii (0..1)
    pub offset: f32,
}

/// One of the two action arcs on the circle
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ActionArc {
    /// heading (radians) at the middle of the arc
    pub heading: f32,
    /// half length (radians)
    pub half: f32,
}

/// The two action arcs: fly flatter where the heading points at the core,
/// fly steeper where it points away. Both are gone for the rest of the
/// circle once enough has been done - flatter towards the core or steeper
/// away from it, wherever on the circle it happened.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct ActionArcs {
    pub flatter: Option<ActionArc>,
    pub steeper: Option<ActionArc>,
}

/// Climb around the circle in 24 sectors of 15 degrees of heading.
///
/// Every sector holds the mean climb rate of its latest pass and the tick
/// of that pass. Both assistants show a sector against the mean of the
/// sectors flown since the present circling began (`reference`): the mean
/// of the circle itself, not the average box. That box starts with the
/// vario reading of the moment the display changes to vario mode - in a
/// pull-up 6.9 m/s before a thermal of 2.8 m/s (one real flight) - and
/// needs a minute to come down, so for the first circles every sector was
/// "below average": a small spider, blue dots only (Stefan 15.11.2026).
///
/// Only the sectors flown since the present circling began (`phase_start`)
/// are shown, so on rolling into a thermal the part of the circle that has
/// not been flown stays empty. The phase starts a little before the
/// circling detector fires, so the roll-in itself is part of it.
///
/// From a full circle of sectors the core of the thermal is estimated
/// (first harmonic of the sector means) and the two action arcs are placed
/// on the circle.
#[derive(PartialEq)]
pub struct ThermalData {
    climb_data: [f32; THERMAL_DATA_CNT],
    /// mean of `climb_data` over the sectors flown in the present circling
    reference: f32,
    /// smallest and largest sector value relative to `reference`
    low: f32,
    high: f32,
    /// mean bank (degrees, absolute) of the latest pass of the sector
    roll_data: [f32; THERMAL_DATA_CNT],
    /// tick of the last update of the sector, 0 = never
    visit_tick: [u32; THERMAL_DATA_CNT],
    /// samples averaged into the sector on its current pass
    samples: [u8; THERMAL_DATA_CNT],
    /// first tick that counts as part of the present circling
    phase_start: u32,
    last_vario_mode: VarioMode,
    last_fly_mode: FlyMode,
    last_tick: u32,
    last_idx: usize,

    best_pos: usize,
    worst_pos: usize,

    estimate: Option<CoreEstimate>,
    arcs: ActionArcs,
    /// arcs shown (hysteresis on the offset)
    arcs_on: bool,
    /// bank change towards the core so far, degree seconds
    progress: f32,
    /// enough done: arcs stay away for a circle
    done: bool,
    /// sectors entered since `done`
    sectors_since_done: u8,
    /// the track through the air (track view of the centering page); it is
    /// recorded in speed to fly mode as well
    pub trail: Trail,
}

impl ThermalData {
    pub fn update(&mut self, cm: &CoreModel) {
        if cm.control.alive_ticks == self.last_tick {
            return;
        }
        self.last_tick = cm.control.alive_ticks;
        self.trail.update(cm);

        if cm.control.vario_mode != self.last_vario_mode {
            if cm.control.vario_mode == VarioMode::Vario {
                for climb_data in &mut self.climb_data {
                    *climb_data = 0.0;
                }
                self.visit_tick = [0; THERMAL_DATA_CNT];
            }
            self.last_vario_mode = cm.control.vario_mode;
        }
        if cm.control.vario_mode == VarioMode::SpeedToFly {
            self.clear_arcs();
            return;
        }

        // A new circling phase: the detector needs circle_hysteresis_tc
        // seconds of turning, so the phase begins that long ago (plus one
        // second) and the sectors of the roll-in belong to it.
        if cm.control.fly_mode == FlyMode::Circling && self.last_fly_mode != FlyMode::Circling {
            let lead = (cm.config.circle_hysteresis_tc.max(0) as u32 + 1) * 10;
            self.phase_start = cm.control.alive_ticks.saturating_sub(lead).max(1);
        }
        self.last_fly_mode = cm.control.fly_mode;

        let idx = Self::get_idx(cm.sensor.euler_yaw.to_radians());
        let value = cm.sensor.climb_rate.to_m_s();
        let roll = cm.sensor.euler_roll.to_degrees().abs();
        if idx != self.last_idx {
            // entering a sector starts a new pass: its mean starts afresh
            self.samples[idx] = 0;
            self.last_idx = idx;
            // a circle after the arcs went away the picture is all new and
            // they may come back
            if self.done {
                self.sectors_since_done = self.sectors_since_done.saturating_add(1);
                if self.sectors_since_done as usize >= THERMAL_DATA_CNT {
                    self.done = false;
                    self.progress = 0.0;
                }
            }
        }
        let n = self.samples[idx] as f32;
        if n == 0.0 {
            self.climb_data[idx] = value;
            self.roll_data[idx] = roll;
        } else {
            self.climb_data[idx] = (self.climb_data[idx] * n + value) / (n + 1.0);
            self.roll_data[idx] = (self.roll_data[idx] * n + roll) / (n + 1.0);
        }
        self.samples[idx] = self.samples[idx].saturating_add(1);
        self.visit_tick[idx] = cm.control.alive_ticks.max(1);

        self.update_reference();
        self.update_arcs(cm, idx);
    }

    /// Mean of the flown sectors and the range of the sectors around it
    fn update_reference(&mut self) {
        let (mut sum, mut n) = (0.0_f32, 0);
        for idx in 0..THERMAL_DATA_CNT {
            if self.visited(idx) {
                sum += self.climb_data[idx];
                n += 1;
            }
        }
        if n == 0 {
            self.reference = 0.0;
            self.low = 0.0;
            self.high = 0.0;
            return;
        }
        self.reference = sum / n as f32;
        let (mut low, mut high) = (0.0_f32, 0.0_f32);
        for idx in 0..THERMAL_DATA_CNT {
            if self.visited(idx) {
                let v = self.climb_data[idx] - self.reference;
                low = low.min(v);
                high = high.max(v);
            }
        }
        self.low = low;
        self.high = high;
    }

    /// flown since the present circling began
    pub fn visited(&self, idx: usize) -> bool {
        let tick = self.visit_tick[idx % THERMAL_DATA_CNT];
        tick != 0 && tick >= self.phase_start
    }

    /// mean climb (m/s) of the sectors flown in the present circling: the
    /// ring both assistants compare a sector with
    pub fn reference(&self) -> f32 {
        self.reference
    }

    /// worst and best sector relative to `reference` (m/s, <= 0 and >= 0)
    pub fn range(&self) -> (f32, f32) {
        (self.low, self.high)
    }

    pub fn get_idx(alpha: f32) -> usize {
        ((alpha.rem_euclid(2.0 * PI) / DELTA_ALPHA) as usize).clamp(0, THERMAL_DATA_CNT - 1)
    }

    /// mean climb of a sector relative to the mean of the flown circle
    pub fn value(&self, idx: usize) -> f32 {
        self.climb_data[idx % THERMAL_DATA_CNT] - self.reference
    }

    /// The next sector in flight direction.
    ///
    /// `CirclingDirection::Left` is the variant of a positive turn rate,
    /// which is a turn to the right with the heading increasing (the glider
    /// symbol sits at 9 o'clock, the circle on its right); `Right` is the
    /// turn to the left with the heading decreasing.
    pub fn next_idx(idx: usize, direction: CirclingDirection) -> usize {
        match direction {
            CirclingDirection::Left => (idx + 1) % THERMAL_DATA_CNT,
            CirclingDirection::Right => (idx + THERMAL_DATA_CNT - 1) % THERMAL_DATA_CNT,
        }
    }

    /// The run of sectors flown since the circle began, ending at `current`
    /// (the sector of the present heading) and reaching back against the
    /// turn direction as far as the sectors are marked visited.
    ///
    /// Returns the first sector of the run and its length in flight
    /// direction; a length of `THERMAL_DATA_CNT` means the whole circle has
    /// been flown, 0 that nothing has been recorded yet.
    pub fn flown_run(&self, current: usize, direction: CirclingDirection) -> (usize, usize) {
        let current = current % THERMAL_DATA_CNT;
        if !self.visited(current) {
            return (current, 0);
        }
        let back = match direction {
            CirclingDirection::Left => CirclingDirection::Right,
            CirclingDirection::Right => CirclingDirection::Left,
        };
        let mut first = current;
        let mut count = 1;
        while count < THERMAL_DATA_CNT {
            let prev = Self::next_idx(first, back);
            if !self.visited(prev) {
                break;
            }
            first = prev;
            count += 1;
        }
        (first, count)
    }

    /// Core of the thermal from the first harmonic of the 24 sector means.
    ///
    /// The heading of the maximum is the heading at which the vario showed
    /// the best climb; the glider was there `VARIO_LAG_PER_TC` times the
    /// vario time constant earlier, so the heading is turned back by that
    /// much. The offset follows from the amplitude relative to the strength
    /// of the thermal (von Mises model, thermal radius about the circle
    /// radius): a rough number, good for the length of the arcs.
    pub fn core_estimate(&self, cm: &CoreModel) -> CoreEstimate {
        let (mut a, mut b) = (0.0_f32, 0.0_f32);
        for (idx, value) in self.climb_data.iter().enumerate() {
            let alpha = (idx as f32 + 0.5) * DELTA_ALPHA;
            a += value * alpha.cos();
            b += value * alpha.sin();
        }
        let scale = 2.0 / THERMAL_DATA_CNT as f32;
        a *= scale;
        b *= scale;
        let amplitude = crate::utils::math::sqrt(a * a + b * b);
        let vario_tc = cm
            .sensor
            .remote
            .get(CanConfigId::VarioTc.remote_slot())
            .unwrap_or(VARIO_TC_DEFAULT);
        let lag = cm.sensor.turn_rate.to_rad_s() * VARIO_LAG_PER_TC * vario_tc;
        let heading = (b.atan2(a) - lag).rem_euclid(2.0 * PI);
        let strength = self.reference.abs() + CIRCLING_SINK;
        let offset = (1.3 * amplitude / strength).clamp(0.0, 1.0);
        CoreEstimate {
            heading,
            amplitude,
            offset,
        }
    }

    /// The circle's usual bank: mean over the sectors (degrees)
    pub fn nominal_bank(&self) -> f32 {
        self.roll_data.iter().sum::<f32>() / THERMAL_DATA_CNT as f32
    }

    fn clear_arcs(&mut self) {
        self.estimate = None;
        self.arcs = ActionArcs::default();
        self.arcs_on = false;
        self.progress = 0.0;
        self.done = false;
        self.sectors_since_done = 0;
    }

    /// Place the action arcs and follow the pilot's bank.
    ///
    /// The circle drifts towards the heading at which the bank is flattest
    /// (kinematics of a turn with modulated bank, confirmed on 15 flights),
    /// so the flatter arc sits where the heading points at the core - a
    /// quarter circle before the best climb - and the steeper arc opposite.
    /// Both are pulled forward against the flight direction by the
    /// reaction time. The bank change against the circle's usual bank,
    /// projected on the direction of the core, is integrated over the whole
    /// circle (flatter towards the core and steeper away from it both
    /// count); once it covers half the estimated offset, both arcs
    /// disappear for a circle - the "well done" the pilot gets to see.
    fn update_arcs(&mut self, cm: &CoreModel, current: usize) {
        let direction = cm.control.circling_direction;
        let (_, count) = self.flown_run(current, direction);
        if cm.control.fly_mode != FlyMode::Circling || count < THERMAL_DATA_CNT {
            self.clear_arcs();
            return;
        }
        let estimate = self.core_estimate(cm);
        self.estimate = Some(estimate);
        // hysteresis on the offset
        if self.arcs_on {
            if estimate.offset < CORE_OFF_OFFSET {
                self.arcs_on = false;
            }
        } else if estimate.offset >= CORE_MIN_OFFSET {
            self.arcs_on = true;
        }
        if !self.arcs_on {
            self.arcs = ActionArcs::default();
            self.progress = 0.0;
            self.done = false;
            return;
        }
        let rate = cm.sensor.turn_rate.to_rad_s();
        let sgn = match direction {
            CirclingDirection::Left => 1.0,
            CirclingDirection::Right => -1.0,
        };
        // heading that points at the core: flying flatter here moves the
        // circle towards it
        let towards = (estimate.heading - sgn * 0.5 * PI).rem_euclid(2.0 * PI);
        // earlier by the reaction time: against the sense of the heading
        let lead = rate * REACTION_TIME;
        let half = ARC_HALF_MIN
            + (ARC_HALF_MAX - ARC_HALF_MIN) * (estimate.offset / ARC_OFFSET_FULL).min(1.0);
        let h_flatter = (towards - lead).rem_euclid(2.0 * PI);
        let h_steeper = (h_flatter + PI).rem_euclid(2.0 * PI);

        if !self.done {
            // one tick is 100 ms; flatter than usual while heading towards
            // the core counts, and so does steeper while heading away
            let yaw = cm.sensor.euler_yaw.to_radians();
            let bank = cm.sensor.euler_roll.to_degrees().abs();
            let gain = (self.nominal_bank() - bank) * angle_diff(yaw, towards).cos() * 0.1;
            self.progress = (self.progress * PROGRESS_DECAY + gain).max(0.0);
            let required = BANK_SECONDS_PER_RADIUS * CORRECTION_PER_CIRCLE * estimate.offset;
            if self.progress >= required {
                self.done = true;
                self.sectors_since_done = 0;
            }
        }
        self.arcs = if self.done {
            ActionArcs::default()
        } else {
            ActionArcs {
                flatter: Some(ActionArc {
                    heading: h_flatter,
                    half,
                }),
                steeper: Some(ActionArc {
                    heading: h_steeper,
                    half,
                }),
            }
        };
    }

    /// the arcs as placed by the last `update`
    pub fn action_arcs(&self) -> ActionArcs {
        self.arcs
    }

    /// the core estimate of the last `update`, once a full circle is flown
    pub fn estimate(&self) -> Option<CoreEstimate> {
        self.estimate
    }

    /// Where to move the circle: the heading (radians) at which the glider
    /// is nearest to the core, None while a full circle has not been flown
    /// or the circle is centred.
    ///
    /// It is the direction of the centre of gravity of all 24 sectors (the
    /// first harmonic of `core_estimate`), turned back by the lag of the
    /// vario - not the single best sector, which a gust moves around. Shown
    /// from an offset of `CORE_MIN_OFFSET` circle radii on, the offset from
    /// which a shift towards the estimate paid off in the flights analysed
    /// (see there), and until it falls below `CORE_OFF_OFFSET`.
    pub fn core_pointer(&self) -> Option<f32> {
        if self.arcs_on {
            self.estimate.map(|e| e.heading)
        } else {
            None
        }
    }

    /// Find the best and the worst of the flown sectors
    pub fn prepare(&mut self) {
        self.best_pos = NO_SECTOR;
        self.worst_pos = NO_SECTOR;
        let (mut best_value, mut worst_value) = (f32::MIN, f32::MAX);
        let mut n = 0;
        for idx in 0..THERMAL_DATA_CNT {
            if !self.visited(idx) {
                continue;
            }
            n += 1;
            if self.climb_data[idx] > best_value {
                best_value = self.climb_data[idx];
                self.best_pos = idx;
            }
            if self.climb_data[idx] < worst_value {
                worst_value = self.climb_data[idx];
                self.worst_pos = idx;
            }
        }
        if n < BEST_WORST_MIN_SECTORS {
            self.best_pos = NO_SECTOR;
            self.worst_pos = NO_SECTOR;
        }
    }

    /// Colour and value (m/s relative to the mean of the flown circle) of a
    /// dot of the dotted assistant, None for a sector that has not been
    /// flown in the present circling. Red is above the mean of the circle,
    /// blue below, the best sector yellow, the worst white.
    pub fn get_dotted_item(&mut self, idx: usize, cm: &CoreModel) -> Option<(Colors, f32)> {
        if !self.visited(idx) {
            return None;
        }
        let value = self.value(idx);
        let color = if idx == self.best_pos {
            cm.palette().vario.therm_ass_best
        } else if idx == self.worst_pos {
            cm.palette().vario.therm_ass_worst
        } else if value > 0.0 {
            cm.palette().vario.therm_ass_good
        } else {
            cm.palette().vario.therm_ass_bad
        };
        Some((color, value))
    }
}

/// difference of two angles (radians) brought into -PI..PI
pub fn angle_diff(a: f32, b: f32) -> f32 {
    (a - b + PI).rem_euclid(2.0 * PI) - PI
}

impl Default for ThermalData {
    fn default() -> Self {
        ThermalData {
            climb_data: [0.0; THERMAL_DATA_CNT],
            reference: 0.0,
            low: 0.0,
            high: 0.0,
            roll_data: [0.0; THERMAL_DATA_CNT],
            visit_tick: [0; THERMAL_DATA_CNT],
            samples: [0; THERMAL_DATA_CNT],
            phase_start: 1,
            last_vario_mode: VarioMode::SpeedToFly,
            last_fly_mode: FlyMode::StraightFlight,
            last_tick: 0,
            last_idx: usize::MAX,
            best_pos: 0,
            worst_pos: 0,
            estimate: None,
            arcs: ActionArcs::default(),
            arcs_on: false,
            progress: 0.0,
            done: false,
            sectors_since_done: 0,
            trail: Trail::default(),
        }
    }
}
