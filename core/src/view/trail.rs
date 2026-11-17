//! The track of the glider through the air, for the track view of the
//! centering page (entering a thermal, above all in the first circle, the
//! pilot sees where he flew before).
//!
//! The position is dead reckoning from true airspeed and heading, so it is
//! a track relative to the air: it drifts with the wind as the thermal
//! does, and needs no GNSS position. One point per second holds the place
//! at the start of the second and the mean climb of that second - the
//! vario reading lags the air by about 0.6 s (`VARIO_LAG_PER_TC`), so the
//! mean of the second belongs to its start rather than to its middle.
//!
//! The recording runs all the time the ring page is drawn, also in speed
//! to fly mode: when the circling detector fires, the approach is already
//! there.

use core::f32::consts::PI;

use crate::{CoreModel, FlyMode};

#[allow(unused_imports)]
use micromath::F32Ext;

/// Points kept: 5 s of approach and 1.5 circles of 50 s each (the widest
/// first circle of the flight the view was laid out with). The memory of
/// the track is part of the view, and the start of the device keeps several
/// copies of the view on the stack - so the points are stored packed, 16
/// bytes each (0.4.1.53 with 96 points of 24 bytes: black display).
pub const TRAIL_CNT: usize = 80;
/// one point per second (ticks of 100 ms)
const TICKS_PER_POINT: u32 = 10;
/// The approach shown before the turn began (ticks): only 5 s before the
/// circle began - further back the reading still holds the pull-up.
pub const APPROACH_TICKS: u32 = 50;
/// The track shows at most the last 1.5 circles.
pub const TRAIL_TURN: f32 = 3.0 * PI;
/// The first 1.5 circles of a thermal belong to the track view; then the
/// centering aid of the page takes over, with a ring that is closed
/// (switch over only after 1.5 circles).
pub const FIRST_TURN: f32 = 3.0 * PI;
/// mean turn rate of a second from which it counts as turning (rad/s)
const TURNING: f32 = 4.0 * PI / 180.0;
/// a pause of the recording longer than this starts the track afresh (the
/// menu was open, another page was shown)
const GAP_TICKS: u32 = 5;
/// shown while not circling (page editor): the last 30 s
const IDLE_TICKS: u32 = 300;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct TrailPoint {
    /// place in the air mass, metres
    pub east: f32,
    pub north: f32,
    /// mean climb of the second that began here, m/s
    pub climb: f32,
    /// heading here, radians
    pub yaw: f32,
    pub tick: u32,
    /// the second was flown turning
    pub turning: bool,
}

/// a point as it is stored: 16 bytes
#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct Stored {
    east: f32,
    north: f32,
    climb: f32,
    /// heading in hundredths of a degree, 0..35999
    yaw: u16,
    /// low 15 bits of the tick, bit 15: turning
    tick: u16,
}

const TURNING_BIT: u16 = 0x8000;

impl Stored {
    fn pack(p: &TrailPoint) -> Self {
        let deg = p.yaw.to_degrees() % 360.0;
        let deg = if deg < 0.0 { deg + 360.0 } else { deg };
        Stored {
            east: p.east,
            north: p.north,
            climb: p.climb,
            yaw: ((deg * 100.0 + 0.5) as u32 % 36000) as u16,
            tick: (p.tick & 0x7fff) as u16 | if p.turning { TURNING_BIT } else { 0 },
        }
    }

    /// `now`: a tick not more than 3276 s after the point
    fn unpack(&self, now: u32) -> TrailPoint {
        let age = now.wrapping_sub((self.tick & 0x7fff) as u32) & 0x7fff;
        TrailPoint {
            east: self.east,
            north: self.north,
            climb: self.climb,
            yaw: (self.yaw as f32 * 0.01).to_radians(),
            tick: now.wrapping_sub(age),
            turning: self.tick & TURNING_BIT != 0,
        }
    }
}

#[derive(PartialEq)]
pub struct Trail {
    points: [Stored; TRAIL_CNT],
    /// index of the next point to write
    next: usize,
    len: usize,
    east: f32,
    north: f32,
    last_tick: u32,
    last_yaw: f32,
    /// the point of the second in progress
    open: TrailPoint,
    sum_climb: f32,
    sum_turn: f32,
    samples: u32,
    last_fly_mode: FlyMode,
    /// tick at which the turn of the present circling began, 0 = not circling
    turn_start: u32,
    /// heading change since `turn_start`, radians, never negative
    turned: f32,
    /// scale step of the track view (kept here so it only changes when it
    /// has to, see `centerview::draw_track_at`)
    pub scale_step: u8,
}

impl Default for Trail {
    fn default() -> Self {
        Trail {
            points: [Stored {
                east: 0.0,
                north: 0.0,
                climb: 0.0,
                yaw: 0,
                tick: 0,
            }; TRAIL_CNT],
            next: 0,
            len: 0,
            east: 0.0,
            north: 0.0,
            last_tick: 0,
            last_yaw: 0.0,
            open: TrailPoint::default(),
            sum_climb: 0.0,
            sum_turn: 0.0,
            samples: 0,
            last_fly_mode: FlyMode::StraightFlight,
            turn_start: 0,
            turned: 0.0,
            scale_step: 0,
        }
    }
}

/// difference of two headings, -PI..PI
fn wrap(a: f32) -> f32 {
    let mut a = a % (2.0 * PI);
    if a > PI {
        a -= 2.0 * PI;
    } else if a < -PI {
        a += 2.0 * PI;
    }
    a
}

impl Trail {
    pub fn update(&mut self, cm: &CoreModel) {
        let tick = cm.control.alive_ticks;
        if tick == self.last_tick {
            return;
        }
        let yaw = cm.sensor.euler_yaw.to_radians();
        let mut ticks = tick.wrapping_sub(self.last_tick);
        if self.last_tick == 0 || ticks > GAP_TICKS {
            // afresh, without a copy of the whole track on the stack
            self.len = 0;
            self.next = 0;
            self.east = 0.0;
            self.north = 0.0;
            self.sum_climb = 0.0;
            self.sum_turn = 0.0;
            self.samples = 0;
            self.last_fly_mode = FlyMode::StraightFlight;
            self.turn_start = 0;
            self.turned = 0.0;
            self.scale_step = 0;
            self.last_yaw = yaw;
            ticks = 0;
        }
        self.last_tick = tick;

        // the second in progress starts where the glider is now
        if self.samples == 0 {
            self.open = TrailPoint {
                east: self.east,
                north: self.north,
                climb: 0.0,
                yaw,
                tick,
                turning: false,
            };
        }
        let dt = ticks as f32 * 0.1;
        let tas = cm.sensor.airspeed.tas().to_m_s();
        self.east += tas * yaw.sin() * dt;
        self.north += tas * yaw.cos() * dt;
        self.sum_climb += cm.sensor.climb_rate.to_m_s();
        self.sum_turn += cm.sensor.turn_rate.to_rad_s().abs();
        self.samples += 1;

        let circling = cm.control.fly_mode == FlyMode::Circling;
        if circling && self.last_fly_mode != FlyMode::Circling {
            self.begin_circling(tick, yaw);
        } else if circling {
            self.turned += wrap(yaw - self.last_yaw).abs();
        } else {
            self.turn_start = 0;
            self.turned = 0.0;
        }
        self.last_fly_mode = cm.control.fly_mode;
        self.last_yaw = yaw;

        if tick.wrapping_sub(self.open.tick) + 1 >= TICKS_PER_POINT {
            let n = self.samples as f32;
            self.open.climb = self.sum_climb / n;
            self.open.turning = self.sum_turn / n >= TURNING;
            self.points[self.next] = Stored::pack(&self.open);
            self.next = (self.next + 1) % TRAIL_CNT;
            self.len = (self.len + 1).min(TRAIL_CNT);
            self.sum_climb = 0.0;
            self.sum_turn = 0.0;
            self.samples = 0;
        }
    }

    /// The circling detector has fired: the turn began where the seconds
    /// of turning before it began.
    fn begin_circling(&mut self, tick: u32, yaw: f32) {
        let mut start = if self.samples > 0 {
            self.open.tick
        } else {
            tick
        };
        let mut turned = if self.samples > 0 {
            wrap(yaw - self.open.yaw).abs()
        } else {
            0.0
        };
        let mut later_yaw = if self.samples > 0 { self.open.yaw } else { yaw };
        for i in 0..self.len {
            let p = self.point(i);
            if !p.turning {
                break;
            }
            turned += wrap(later_yaw - p.yaw).abs();
            later_yaw = p.yaw;
            start = p.tick;
        }
        self.turn_start = start.max(1);
        self.turned = turned;
        self.scale_step = 0;
    }

    /// the i-th point back from the newest one (0 = newest)
    pub fn point(&self, i: usize) -> TrailPoint {
        self.points[(self.next + TRAIL_CNT - 1 - i % TRAIL_CNT) % TRAIL_CNT].unpack(self.last_tick)
    }

    /// where the glider is now
    pub fn position(&self) -> (f32, f32) {
        (self.east, self.north)
    }

    /// Number of points the track view shows, counted back from the newest:
    /// from `APPROACH_TICKS` before the turn began, and not more than the
    /// last `TRAIL_TURN` of turning.
    pub fn shown_len(&self) -> usize {
        let first = if self.turn_start > 0 {
            self.turn_start.saturating_sub(APPROACH_TICKS)
        } else {
            self.last_tick.saturating_sub(IDLE_TICKS)
        };
        let mut turned = 0.0;
        let mut later_yaw = self.last_yaw;
        for i in 0..self.len {
            let p = self.point(i);
            turned += wrap(later_yaw - p.yaw).abs();
            later_yaw = p.yaw;
            if p.tick < first || turned > TRAIL_TURN {
                return i;
            }
        }
        self.len
    }

    /// the point was flown after the turn of the present circling began
    pub fn in_circle(&self, p: &TrailPoint) -> bool {
        self.turn_start > 0 && p.tick >= self.turn_start
    }

    /// Circling, and less than `FIRST_TURN` turned since the turn began:
    /// the centering page shows the track whatever aid it has.
    pub fn first_circles(&self) -> bool {
        self.turn_start > 0 && self.turned < FIRST_TURN
    }

    /// heading change since the turn began, radians
    pub fn turned(&self) -> f32 {
        self.turned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::test_fixture::test_model;
    use crate::{AirSpeed, FloatToSpeed};
    use embedded_graphics::geometry::AngleUnit;

    /// fly `seconds` with the turn rate `rate` (degrees per second, + = to the right)
    fn fly(
        trail: &mut Trail,
        cm: &mut CoreModel,
        yaw: &mut f32,
        seconds: f32,
        rate: f32,
        climb: f32,
    ) {
        for _ in 0..(seconds * 10.0 + 0.5) as u32 {
            *yaw += rate * 0.1;
            cm.control.alive_ticks += 1;
            cm.sensor.euler_yaw = yaw.rem_euclid(360.0).deg();
            cm.sensor.turn_rate = crate::FloatToAngularVelocity::rad_s(rate.to_radians());
            cm.sensor.airspeed = AirSpeed::new(28.0, 30.0);
            cm.sensor.climb_rate = climb.m_s();
            trail.update(cm);
        }
    }

    fn model() -> CoreModel {
        let mut cm = test_model();
        cm.control.alive_ticks = 1000;
        cm.control.fly_mode = FlyMode::StraightFlight;
        cm
    }

    #[test]
    fn the_position_follows_speed_and_heading() {
        let (mut trail, mut cm, mut yaw) = (Trail::default(), model(), 90.0_f32);
        fly(&mut trail, &mut cm, &mut yaw, 10.0, 0.0, 1.0);
        let (e, n) = trail.position();
        // the first update only starts the track: 9.9 s at 30 m/s to the east
        assert!(
            (e - 297.0).abs() < 0.5 && n.abs() < 0.5,
            "east {e} north {n}"
        );
        // one point per second, the newest holds the last complete second
        assert_eq!(trail.len, 10);
        assert!(
            (trail.point(0).east - 270.0).abs() < 3.1 && (trail.point(0).climb - 1.0).abs() < 1e-4
        );
        assert!(!trail.point(0).turning);
    }

    #[test]
    fn the_turn_begins_where_the_turning_began_and_five_seconds_of_approach_show() {
        let (mut trail, mut cm, mut yaw) = (Trail::default(), model(), 0.0_f32);
        fly(&mut trail, &mut cm, &mut yaw, 30.0, 0.0, 0.5);
        let turn_begin = cm.control.alive_ticks;
        // 7 s of turning at 18 degrees per second, then the detector fires
        fly(&mut trail, &mut cm, &mut yaw, 7.0, 18.0, 2.0);
        assert!(
            !trail.first_circles(),
            "not before the display has detected circling"
        );
        cm.control.fly_mode = FlyMode::Circling;
        fly(&mut trail, &mut cm, &mut yaw, 0.1, 18.0, 2.0);
        assert!(trail.first_circles());
        assert!(
            (trail.turn_start as i64 - turn_begin as i64).abs() <= 10,
            "turn start {} against {}",
            trail.turn_start,
            turn_begin
        );
        assert!(
            (trail.turned().to_degrees() - 7.1 * 18.0).abs() < 20.0,
            "turned {}",
            trail.turned().to_degrees()
        );
        // 7 points of the turn and 5 of the approach (the second in which
        // the turn began may count to either side)
        let n = trail.shown_len();
        assert!((12..=13).contains(&n), "{n} points");
        let in_circle = (0..n).filter(|i| trail.in_circle(&trail.point(*i))).count();
        assert!(
            (7..=8).contains(&in_circle),
            "{in_circle} points since the turn began"
        );
    }

    #[test]
    fn after_one_and_a_half_circles_the_aid_of_the_page_takes_over() {
        let (mut trail, mut cm, mut yaw) = (Trail::default(), model(), 0.0_f32);
        fly(&mut trail, &mut cm, &mut yaw, 20.0, 0.0, 0.5);
        fly(&mut trail, &mut cm, &mut yaw, 7.0, -18.0, 2.0);
        cm.control.fly_mode = FlyMode::Circling;
        // 540 degrees at 18 degrees per second are 30 s, 7 s are flown
        fly(&mut trail, &mut cm, &mut yaw, 21.0, -18.0, 2.0);
        assert!(
            trail.first_circles(),
            "{} degrees",
            trail.turned().to_degrees()
        );
        fly(&mut trail, &mut cm, &mut yaw, 3.0, -18.0, 2.0);
        assert!(
            !trail.first_circles(),
            "{} degrees",
            trail.turned().to_degrees()
        );
        // the track itself goes on: the last 1.5 circles, the approach is gone
        fly(&mut trail, &mut cm, &mut yaw, 40.0, -18.0, 2.0);
        let n = trail.shown_len();
        assert!(
            (29..=31).contains(&n),
            "{n} points for 540 degrees at 18 degrees per second"
        );
        assert!((0..n).all(|i| trail.in_circle(&trail.point(i))));
        // straight flight ends it
        cm.control.fly_mode = FlyMode::StraightFlight;
        fly(&mut trail, &mut cm, &mut yaw, 1.0, 0.0, 0.0);
        assert!(!trail.first_circles() && trail.turned() == 0.0);
    }

    /// the track must stay small: see TRAIL_CNT
    #[test]
    fn a_point_is_stored_in_16_bytes_and_comes_back_as_it_was() {
        assert_eq!(core::mem::size_of::<Stored>(), 16);
        assert!(
            core::mem::size_of::<Trail>() <= 16 * TRAIL_CNT + 80,
            "{} bytes",
            core::mem::size_of::<Trail>()
        );
        for (yaw, tick, turning) in [
            (0.0_f32, 1_000_u32, false),
            (359.99, 40_000, true),
            (123.45, 70_123, true),
            (-90.0, 5, false),
        ] {
            let p = TrailPoint {
                east: -1234.5,
                north: 98765.4,
                climb: -2.35,
                yaw: yaw.to_radians(),
                tick,
                turning,
            };
            // read back up to 100 s later
            for later in [0_u32, 9, 1_000] {
                let q = Stored::pack(&p).unpack(tick + later);
                assert_eq!(
                    (q.east, q.north, q.climb, q.tick, q.turning),
                    (p.east, p.north, p.climb, p.tick, p.turning)
                );
                assert!(
                    wrap(q.yaw - p.yaw).abs() < 0.0002,
                    "yaw {} -> {}",
                    p.yaw,
                    q.yaw
                );
            }
        }
    }

    #[test]
    fn a_pause_of_the_recording_starts_the_track_afresh() {
        let (mut trail, mut cm, mut yaw) = (Trail::default(), model(), 0.0_f32);
        fly(&mut trail, &mut cm, &mut yaw, 20.0, 0.0, 0.5);
        assert_eq!(trail.len, 20);
        // the menu was open for 10 s: the place of the glider is not known
        cm.control.alive_ticks += 100;
        fly(&mut trail, &mut cm, &mut yaw, 2.0, 0.0, 0.5);
        assert_eq!(trail.len, 2);
        let (e, n) = trail.position();
        assert!(
            e.abs() < 0.5 && (n - 57.0).abs() < 0.5,
            "east {e} north {n}"
        );
    }

    #[test]
    fn a_very_slow_circle_keeps_what_fits_the_memory() {
        let (mut trail, mut cm, mut yaw) = (Trail::default(), model(), 0.0_f32);
        fly(&mut trail, &mut cm, &mut yaw, 10.0, 0.0, 0.5);
        cm.control.fly_mode = FlyMode::Circling;
        // 4.5 degrees per second: 1.5 circles take 120 s
        fly(&mut trail, &mut cm, &mut yaw, 150.0, 4.5, 1.0);
        assert_eq!(trail.shown_len(), TRAIL_CNT);
    }
}
