//! Traffic model.
//!
//! Holds own movement and all targets from PFLAA. FLARM sends about once per
//! second, so every target is extrapolated between messages with track, speed
//! and turn rate, and new messages are blended in softly instead of jumping.
//! This allows a smooth 30 fps display.

#[cfg(not(flarm_standalone))]
#[allow(unused_imports)]
use micromath::F32Ext;

use super::nmea::{Gprmc, NmeaMsg, Pflaa, Pflae, Pflau};

/// Degrees to radians.
pub const DEG2RAD: f32 = 0.017453292;
/// Radians to degrees.
pub const RAD2DEG: f32 = 57.29577951;
/// Knots to m/s.
pub const KT2MS: f32 = 0.514444;

/// Target table size. When full, the least important target is replaced
/// (never the team mate or a target with alarm).
///
/// Without the feature "flarm" (parked for the test versions) the table is
/// not used, and it must not cost its 11 KB: the controller holds it, and
/// the start of the device (RTIC init, driver::init::hw_init) keeps several
/// copies of the controller on the stack. 0.4.1.53 with the full table and
/// 2.4 KB more in the view needed 126 KB of stack plus the frames below in
/// 128 KB of RAM - the display stayed black (16.11.2026).
pub const MAX_TARGETS: usize = if cfg!(feature = "flarm") { 100 } else { 2 };
/// Stored non-directional targets (Mode-S/C without position).
pub const MAX_NONDIR: usize = 4;
/// A target without message for this long is no longer shown (ms).
pub const TARGET_SHOW_MS: i64 = 8000;
/// A target without message for this long is removed (ms).
pub const TARGET_TIMEOUT_MS: i64 = 20_000;
/// Extrapolation limit (ms).
pub const PREDICT_MAX_MS: i64 = 3000;
/// Team mate is extrapolated from the table while younger than this (ms), then team memory is used.
pub const TEAM_FRESH_MS: i64 = 5000;
/// External attitude is used for the heading while younger than this (ms).
pub const ATTITUDE_FRESH_MS: i64 = 1000;

const BLEND_TAU_S: f32 = 0.35; // blend time constant of new messages
const BLEND_MAX_M: f32 = 400.0; // larger jumps are not smoothed

/// `now - then` in ms without overflow.
#[inline]
pub fn since(now: i64, then: i64) -> i64 {
    now.saturating_sub(then)
}

#[inline]
fn secs(ms: i64) -> f32 {
    ms as f32 / 1000.0
}

/// Wrap an angle to (-180, 180]. Non-finite or absurdly large input gives 0.
pub fn wrap180(a: f32) -> f32 {
    if !a.is_finite() || a.abs() > 1.0e7 {
        return 0.0;
    }
    let mut a = a - (((a / 360.0) as i32) as f32) * 360.0;
    while a > 180.0 {
        a -= 360.0;
    }
    while a <= -180.0 {
        a += 360.0;
    }
    a
}

/// Wrap an angle to [0, 360). Non-finite or absurdly large input gives 0.
pub fn wrap360(a: f32) -> f32 {
    if !a.is_finite() || a.abs() > 1.0e7 {
        return 0.0;
    }
    let mut a = a - (((a / 360.0) as i32) as f32) * 360.0;
    while a >= 360.0 {
        a -= 360.0;
    }
    while a < 0.0 {
        a += 360.0;
    }
    if a >= 360.0 {
        0.0 // -tiny + 360 rounds to 360
    } else {
        a
    }
}

/// cos of an f64 angle in degrees, computed in f32 (no f64 math in `no_std`).
#[inline]
pub(super) fn cos_deg64(deg: f64) -> f64 {
    ((deg as f32) * DEG2RAD).cos() as f64
}

/// One directional target from PFLAA.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    /// Slot in use.
    pub used: bool,
    /// FLARM / ICAO ID.
    pub id: u32,
    /// Last received message.
    pub raw: Pflaa,
    /// Time of the last message (ms).
    pub t_rx: i64,
    /// Measured relative north at `t_rx` (m).
    pub rn: f32,
    /// Measured relative east at `t_rx` (m).
    pub re: f32,
    /// Measured relative vertical at `t_rx` (m).
    pub rv: f32,
    /// Track and ground speed known.
    pub has_vel: bool,
    /// Target track (deg true).
    pub trk: f32,
    /// Target ground speed (m/s).
    pub gs: f32,
    /// Target turn rate (deg/s, right positive), from PFLAA or estimated from successive tracks.
    pub turn: f32,
    /// Blend correction north (m) at `t_corr`.
    pub cn: f32,
    /// Blend correction east (m) at `t_corr`.
    pub ce: f32,
    /// Blend correction vertical (m) at `t_corr`.
    pub cv: f32,
    /// Time of the blend correction (ms).
    pub t_corr: i64,
    /// Own track at `t_rx` (deg).
    pub own_trk_at_rx: f32,
}

impl Target {
    /// Unused slot.
    pub const EMPTY: Target = Target {
        used: false,
        id: 0,
        raw: Pflaa::EMPTY,
        t_rx: 0,
        rn: 0.0,
        re: 0.0,
        rv: 0.0,
        has_vel: false,
        trk: 0.0,
        gs: 0.0,
        turn: 0.0,
        cn: 0.0,
        ce: 0.0,
        cv: 0.0,
        t_corr: 0,
        own_trk_at_rx: 0.0,
    };
}

/// Memory of the team mate, also after reception is lost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeamMemory {
    /// Team mate was received at least once.
    pub known: bool,
    /// Time last seen (ms).
    pub t_seen: i64,
    /// Last known absolute latitude (deg), valid if `has_pos`.
    pub lat: f64,
    /// Last known absolute longitude (deg), valid if `has_pos`.
    pub lon: f64,
    /// Last relative vertical (m).
    pub rv: f32,
    /// Last climb rate (m/s, 0 if unknown).
    pub climb: f32,
    /// Absolute position stored (own position was known).
    pub has_pos: bool,
}

impl TeamMemory {
    /// Nothing known.
    pub const EMPTY: TeamMemory = TeamMemory {
        known: false,
        t_seen: 0,
        lat: 0.0,
        lon: 0.0,
        rv: 0.0,
        climb: 0.0,
        has_pos: false,
    };
}

/// Non-directional target (transponder Mode-S/C without position): distance and height only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NonDirTarget {
    /// Slot in use.
    pub used: bool,
    /// ID.
    pub id: u32,
    /// Estimated distance (m).
    pub dist: f32,
    /// Relative vertical (m).
    pub rv: f32,
    /// Alarm level.
    pub alarm: u8,
    /// Time of last message (ms).
    pub t_rx: i64,
}

impl NonDirTarget {
    /// Unused slot.
    pub const EMPTY: NonDirTarget = NonDirTarget {
        used: false,
        id: 0,
        dist: 0.0,
        rv: 0.0,
        alarm: 0,
        t_rx: 0,
    };
}

/// Relative position of the team mate (also from memory).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeamRel {
    /// Relative north (m).
    pub rn: f32,
    /// Relative east (m).
    pub re: f32,
    /// Relative vertical (m).
    pub rv: f32,
    /// Seconds since last message.
    pub age_s: f32,
}

/// Complete traffic state.
#[derive(Clone, Debug)]
pub struct Traffic {
    /// Last RMC had a valid fix.
    pub gps_valid: bool,
    /// Time of last valid RMC (ms).
    pub t_rmc: i64,
    /// Own ground speed (m/s).
    pub gs: f32,
    /// Own track (deg true).
    pub trk: f32,
    /// Own turn rate (deg/s), estimated from successive courses.
    pub turn: f32,
    /// Own track known.
    pub has_trk: bool,
    /// Own position (lat, lon) in degrees.
    pub pos: Option<(f64, f64)>,
    /// UTC time from RMC (hh, mm, ss) at `t_time`.
    pub time: Option<(u8, u8, u8)>,
    /// Receive time of `time` (ms).
    pub t_time: i64,
    /// External attitude available (vario sensor box), preferred for the heading.
    pub att_valid: bool,
    /// Time of the last attitude (ms).
    pub t_att: i64,
    /// External heading (deg true).
    pub att_heading: f32,
    /// External roll (deg).
    pub att_roll: f32,
    /// External pitch (deg).
    pub att_pitch: f32,
    /// Last PFLAU.
    pub pflau: Pflau,
    /// Time of the last PFLAU (ms), `None` if never received.
    pub t_pflau: Option<i64>,
    /// Last PFLAE.
    pub pflae: Pflae,
    /// Time of the last PFLAE (ms), `None` if never received.
    pub t_pflae: Option<i64>,
    /// Time of the last valid line of any type (ms), `None` if never received.
    pub t_last_data: Option<i64>,
    /// Own circling detected (with hysteresis).
    pub circling: bool,
    circ_cnt: i8,
    /// Directional targets.
    pub tg: [Target; MAX_TARGETS],
    /// Non-directional targets.
    pub nd: [NonDirTarget; MAX_NONDIR],
    /// Team mate ID, 0 = none. Use [`Traffic::set_team`] to change it.
    pub team_id: u32,
    /// Team mate memory.
    pub team: TeamMemory,
}

impl Default for Traffic {
    fn default() -> Self {
        Self::new()
    }
}

/// Displacement (east, north) on a circular arc: start track `trk` (deg),
/// speed `gs`, turn rate `turn` (deg/s), time `dt` (s).
fn arc_disp(trk: f32, gs: f32, turn: f32, dt: f32) -> (f32, f32) {
    let th = trk * DEG2RAD;
    let w = turn * DEG2RAD;
    if w.abs() < 0.005 {
        return (gs * dt * th.sin(), gs * dt * th.cos());
    }
    let r = gs / w;
    let th1 = th + w * dt;
    (r * (th.cos() - th1.cos()), r * (th1.sin() - th.sin()))
}

/// Flat earth offset for the team memory.
fn latlon_offset(lat: f64, lon: f64, dn: f32, de: f32) -> (f64, f64) {
    (
        lat + dn as f64 / 111_320.0,
        lon + de as f64 / (111_320.0 * cos_deg64(lat)),
    )
}

fn latlon_diff(lat0: f64, lon0: f64, lat1: f64, lon1: f64) -> (f32, f32) {
    (
        ((lat1 - lat0) * 111_320.0) as f32,
        ((lon1 - lon0) * 111_320.0 * cos_deg64(lat0)) as f32,
    )
}

impl Traffic {
    /// Empty state, no team mate.
    pub const fn new() -> Self {
        Traffic {
            gps_valid: false,
            t_rmc: 0,
            gs: 0.0,
            trk: 0.0,
            turn: 0.0,
            has_trk: false,
            pos: None,
            time: None,
            t_time: 0,
            att_valid: false,
            t_att: 0,
            att_heading: 0.0,
            att_roll: 0.0,
            att_pitch: 0.0,
            pflau: Pflau::EMPTY,
            t_pflau: None,
            pflae: Pflae::EMPTY,
            t_pflae: None,
            t_last_data: None,
            circling: false,
            circ_cnt: 0,
            tg: [Target::EMPTY; MAX_TARGETS],
            nd: [NonDirTarget::EMPTY; MAX_NONDIR],
            team_id: 0,
            team: TeamMemory::EMPTY,
        }
    }

    /// Reset everything except the team mate ID (team memory is cleared).
    pub fn clear(&mut self) {
        let team = self.team_id;
        *self = Traffic::new();
        self.team_id = team;
    }

    /// Set the team mate (0 = none) and clear the team memory.
    pub fn set_team(&mut self, id: u32) {
        self.team_id = id;
        self.team = TeamMemory::EMPTY;
    }

    /// Process one decoded message received at `now_ms`.
    pub fn handle(&mut self, m: &NmeaMsg, now_ms: i64) {
        self.t_last_data = Some(now_ms);
        match m {
            NmeaMsg::Pflau(p) => {
                self.pflau = *p;
                self.t_pflau = Some(now_ms);
            }
            NmeaMsg::Pflaa(a) => self.handle_pflaa(a, now_ms),
            NmeaMsg::Gprmc(r) => self.handle_rmc(r, now_ms),
            NmeaMsg::Pflae(e) => {
                self.pflae = *e;
                self.t_pflae = Some(now_ms);
            }
            _ => {}
        }
    }

    /// Remove targets without message for [`TARGET_TIMEOUT_MS`].
    pub fn housekeeping(&mut self, now_ms: i64) {
        for g in self.tg.iter_mut() {
            if g.used && since(now_ms, g.t_rx) > TARGET_TIMEOUT_MS {
                g.used = false;
            }
        }
    }

    /// External attitude (heading deg true, roll, pitch in deg) from the vario
    /// sensor box. Used for the display heading while younger than 1 s.
    /// Non-finite values are ignored.
    pub fn set_attitude(&mut self, heading_deg: f32, roll_deg: f32, pitch_deg: f32, now_ms: i64) {
        if !(heading_deg.is_finite() && roll_deg.is_finite() && pitch_deg.is_finite()) {
            return;
        }
        self.att_valid = true;
        self.t_att = now_ms;
        self.att_heading = wrap360(heading_deg);
        self.att_roll = roll_deg;
        self.att_pitch = pitch_deg;
    }

    /// Heading the display is aligned to (deg true) and whether it comes from
    /// the external attitude (otherwise: extrapolated GPS track).
    pub fn own_heading(&self, now_ms: i64) -> (f32, bool) {
        if self.att_valid && since(now_ms, self.t_att) < ATTITUDE_FRESH_MS {
            return (self.att_heading, true);
        }
        (self.own_trk_at(now_ms), false)
    }

    /// Index of the target with `id`.
    pub fn find(&self, id: u32) -> Option<usize> {
        self.tg.iter().position(|g| g.used && g.id == id)
    }

    /// Target in slot `idx` if used.
    pub fn target(&self, idx: usize) -> Option<&Target> {
        self.tg.get(idx).filter(|g| g.used)
    }

    /// Extrapolated relative position (north, east, up in m) of the target in
    /// slot `idx` at `now_ms`. Unused or invalid slots give (0, 0, 0).
    pub fn target_pos(&self, idx: usize, now_ms: i64) -> (f32, f32, f32) {
        match self.target(idx) {
            Some(g) => self.pos_of(g, now_ms),
            None => (0.0, 0.0, 0.0),
        }
    }

    /// Extrapolated relative position of a target (north, east, up in m).
    pub fn pos_of(&self, g: &Target, now_ms: i64) -> (f32, f32, f32) {
        let (mut rn, mut re) = self.predict_raw(g, now_ms);
        let mut rv = g.rv;
        if let Some(climb) = g.raw.climb {
            let dt = secs(since(now_ms, g.t_rx)).min(secs(PREDICT_MAX_MS));
            if dt > 0.0 {
                rv += climb * dt; // own climb unknown -> target only
            }
        }
        let age = secs(since(now_ms, g.t_corr));
        if (0.0..3.0).contains(&age) {
            let k = (-age / BLEND_TAU_S).exp();
            rn += g.cn * k;
            re += g.ce * k;
            rv += g.cv * k;
        }
        (rn, re, rv)
    }

    /// Team mate relative position (also from memory). `None` if no team mate
    /// is set or it was never seen.
    pub fn team_rel(&self, now_ms: i64) -> Option<TeamRel> {
        if self.team_id == 0 {
            return None;
        }
        let gi = self.find(self.team_id);
        if let Some(i) = gi {
            let g = &self.tg[i];
            if since(now_ms, g.t_rx) < TEAM_FRESH_MS {
                let (rn, re, rv) = self.pos_of(g, now_ms);
                return Some(TeamRel {
                    rn,
                    re,
                    rv,
                    age_s: secs(since(now_ms, g.t_rx)),
                });
            }
        }
        let own = match self.pos {
            Some(p) if self.team.known && self.team.has_pos => p,
            _ => {
                // without position: last relative position
                return gi.map(|i| {
                    let g = &self.tg[i];
                    TeamRel {
                        rn: g.rn,
                        re: g.re,
                        rv: g.rv,
                        age_s: secs(since(now_ms, g.t_rx)),
                    }
                });
            }
        };
        let (rn, re) = latlon_diff(own.0, own.1, self.team.lat, self.team.lon);
        Some(TeamRel {
            rn,
            re,
            rv: self.team.rv,
            age_s: secs(since(now_ms, self.team.t_seen)),
        })
    }

    /* ---------------- internals ---------------- */

    /// Own track at `ms`, extrapolated with the turn rate.
    fn own_trk_at(&self, ms: i64) -> f32 {
        let dt = secs(since(ms, self.t_rmc)).max(-1.0).min(2.0);
        wrap360(self.trk + self.turn * dt)
    }

    /// Own displacement (east, north) between `ms0` and `ms1`.
    fn own_disp(&self, ms0: i64, ms1: i64) -> (f32, f32) {
        let dt = secs(since(ms1, ms0));
        if !self.has_trk || self.gs < 1.0 {
            return (0.0, 0.0);
        }
        arc_disp(self.own_trk_at(ms0), self.gs, self.turn, dt)
    }

    /// Prediction without blend correction (north, east).
    fn predict_raw(&self, g: &Target, now: i64) -> (f32, f32) {
        let mut ms = now;
        if since(ms, g.t_rx) > PREDICT_MAX_MS {
            ms = g.t_rx.saturating_add(PREDICT_MAX_MS);
        }
        let dt = secs(since(ms, g.t_rx));
        let (mut ten, mut tnn, mut oe, mut on) = (0.0, 0.0, 0.0, 0.0);
        if g.has_vel && dt > 0.0 {
            let (e, n) = arc_disp(g.trk, g.gs, g.turn, dt);
            ten = e;
            tnn = n;
        }
        if dt > 0.0 {
            let (e, n) = self.own_disp(g.t_rx, ms);
            oe = e;
            on = n;
        }
        (g.rn + tnn - on, g.re + ten - oe)
    }

    /// Importance when evicting: smaller = replace first. Expired targets
    /// first, then the farthest; team mate and alarm targets never.
    fn keep_score(&self, id: u32, raw: &Pflaa, t_rx: i64, now: i64) -> f32 {
        if (self.team_id != 0 && id == self.team_id) || raw.alarm_level > 0 {
            return 1e12;
        }
        let n = raw.rel_north as f32;
        let e = raw.rel_east as f32;
        let d = (n * n + e * e).sqrt();
        let age = since(now, t_rx) as f32;
        if age > TARGET_SHOW_MS as f32 {
            return -age; // expired: free immediately
        }
        1e8 - d // otherwise: farther away = less important
    }

    /// Slot for `id`: (index, freshly allocated). `None` if the table is full
    /// of more important targets.
    fn find_or_alloc(&mut self, id: u32, now: i64, a: &Pflaa) -> Option<(usize, bool)> {
        let mut free_slot: Option<usize> = None;
        let mut victim: Option<(usize, f32)> = None;
        for (i, g) in self.tg.iter().enumerate() {
            if g.used && g.id == id {
                return Some((i, false));
            }
            if !g.used {
                if free_slot.is_none() {
                    free_slot = Some(i);
                }
                continue;
            }
            let sc = self.keep_score(g.id, &g.raw, g.t_rx, now);
            match victim {
                Some((_, vs)) if sc >= vs => {}
                _ => victim = Some((i, sc)),
            }
        }
        let slot = match (free_slot, victim) {
            (Some(f), _) => f,
            (None, Some((v, vs))) => {
                // full: only take the new target if it is more important than the weakest entry
                if self.keep_score(id, a, now, now) <= vs {
                    return None;
                }
                v
            }
            (None, None) => return None,
        };
        let g = self.tg.get_mut(slot)?;
        *g = Target::EMPTY;
        g.used = true;
        g.id = id;
        Some((slot, true))
    }

    fn handle_pflaa(&mut self, a: &Pflaa, now: i64) {
        if !a.directional {
            // non-directional: RelativeNorth holds the estimated distance
            let mut slot: Option<usize> = None;
            let mut oldest = 0usize;
            for i in 0..MAX_NONDIR {
                let nd = &self.nd[i];
                if nd.used && nd.id == a.id {
                    slot = Some(i);
                    break;
                }
                if slot.is_none() && !nd.used {
                    slot = Some(i);
                }
                if nd.t_rx < self.nd[oldest].t_rx {
                    oldest = i;
                }
            }
            let s = &mut self.nd[slot.unwrap_or(oldest)];
            s.used = true;
            s.id = a.id;
            s.dist = a.rel_north.unsigned_abs() as f32;
            s.rv = a.rel_vertical as f32;
            s.alarm = a.alarm_level;
            s.t_rx = now;
            return;
        }
        let (i, fresh) = match self.find_or_alloc(a.id, now, a) {
            Some(x) => x,
            None => return, // table full of more important targets
        };
        // blend correction: difference between current display and new measurement
        if !fresh && since(now, self.tg[i].t_rx) < PREDICT_MAX_MS {
            let (pn, pe, pv) = self.pos_of(&self.tg[i], now);
            let dn = pn - a.rel_north as f32;
            let de = pe - a.rel_east as f32;
            let g = &mut self.tg[i];
            if dn.abs() < BLEND_MAX_M && de.abs() < BLEND_MAX_M {
                g.cn = dn;
                g.ce = de;
                g.cv = pv - a.rel_vertical as f32;
                g.t_corr = now;
            } else {
                g.cn = 0.0;
                g.ce = 0.0;
                g.cv = 0.0;
            }
        }
        let own_trk = self.own_trk_at(now);
        let g = &mut self.tg[i];
        // turn rate: FLARM mostly leaves the field empty, then estimate from two tracks
        let mut turn_est = 0.0f32;
        if let Some(tr) = a.turn_rate {
            turn_est = tr;
        } else if let (false, true, Some(trk), Some(gs)) =
            (fresh, g.has_vel, a.track, a.ground_speed)
        {
            if gs > 5 {
                let dt = secs(since(now, g.t_rx));
                if dt > 0.3 && dt < 4.0 {
                    let w = wrap180(trk as f32 - g.trk) / dt;
                    turn_est = 0.5 * g.turn + 0.5 * w;
                }
            }
        }
        g.raw = *a;
        g.t_rx = now;
        g.rn = a.rel_north as f32;
        g.re = a.rel_east as f32;
        g.rv = a.rel_vertical as f32;
        g.has_vel = a.track.is_some() && a.ground_speed.is_some();
        g.trk = a.track.unwrap_or(0) as f32;
        g.gs = a.ground_speed.unwrap_or(0) as f32;
        g.turn = turn_est.max(-30.0).min(30.0);
        g.own_trk_at_rx = own_trk;
        let (grn, gre, grv) = (g.rn, g.re, g.rv);

        if self.team_id != 0 && a.id == self.team_id {
            self.team.known = true;
            self.team.t_seen = now;
            self.team.rv = grv;
            self.team.climb = a.climb.unwrap_or(0.0);
            if let Some((lat, lon)) = self.pos {
                let (la, lo) = latlon_offset(lat, lon, grn, gre);
                self.team.lat = la;
                self.team.lon = lo;
                self.team.has_pos = true;
            }
        }
    }

    fn handle_rmc(&mut self, r: &Gprmc, now: i64) {
        if let Some(t) = r.time {
            self.time = Some(t);
            self.t_time = now;
        }
        self.gps_valid = r.valid;
        if !r.valid {
            return;
        }
        let gs = r.gs_kt * KT2MS;
        match r.course {
            Some(course) if gs > 3.0 => {
                if self.has_trk && now > self.t_rmc {
                    let dt = secs(since(now, self.t_rmc));
                    if dt > 0.2 && dt < 3.0 {
                        let w = (wrap180(course - self.trk) / dt).max(-40.0).min(40.0);
                        self.turn = 0.5 * self.turn + 0.5 * w;
                    }
                }
                self.trk = wrap360(course);
                self.has_trk = true;
            }
            _ => self.turn = 0.0,
        }
        self.gs = gs;
        self.t_rmc = now;
        // circling: in above 8 deg/s for 2 messages, out below 4 deg/s for 3 messages
        let aw = self.turn.abs();
        if !self.circling {
            self.circ_cnt = if aw > 8.0 {
                self.circ_cnt.saturating_add(1)
            } else {
                0
            };
            if self.circ_cnt >= 2 {
                self.circling = true;
                self.circ_cnt = 0;
            }
        } else {
            self.circ_cnt = if aw < 4.0 {
                self.circ_cnt.saturating_add(1)
            } else {
                0
            };
            if self.circ_cnt >= 3 {
                self.circling = false;
                self.circ_cnt = 0;
            }
        }
        if let Some(p) = r.pos {
            self.pos = Some(p);
        }
    }
}
