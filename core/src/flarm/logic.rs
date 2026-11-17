//! Display decisions (port of `view.c` without LED ring and buzzer).
//!
//! [`compute`] turns the traffic model into a [`FlarmView`] once per frame:
//! which screen to show, the threat, team mate, selected/nearest target and
//! the other targets, all already rotated into the display frame
//! (x = right, y = forward, relative to the own heading).
//!
//! Selection for a rotary encoder lives in [`LogicState`]:
//! [`LogicState::select_step`] and [`LogicState::follow_selected`].

#[cfg(not(flarm_standalone))]
#[allow(unused_imports)]
use micromath::F32Ext;

use super::traffic::{
    since, wrap180, wrap360, Target, Traffic, DEG2RAD, MAX_NONDIR, RAD2DEG, TARGET_SHOW_MS,
};

/// Maximum number of other targets in a view (nearest first).
pub const MAX_OTHERS: usize = 24;
/// Maximum number of non-directional targets in a view.
pub const MAX_NONDIR_VIEW: usize = 2;
/// A PFLAU is used for warnings / GPS status while younger than this (ms).
pub const PFLAU_FRESH_MS: i64 = 3000;
/// No valid line for this long: "no FLARM" (ms).
pub const DATA_TIMEOUT_MS: i64 = 5000;
/// Team mate counts as lost after this many seconds without message.
pub const TEAM_LOST_S: f32 = 6.0;
/// Head-on view: threat within this relative bearing AND elevation angle (deg).
pub const HEADON_ANGLE: f32 = 10.0;
/// Head-on view is kept while the same target stays within this bearing and elevation (deg).
pub const HEADON_KEEP_ANGLE: f32 = 12.0;
/// Head-on view is kept at least this long once shown (ms).
pub const HEADON_HOLD_MS: i64 = 2000;
/// Minimum closing speed for the head-on view (m/s).
pub const HEADON_MIN_CLOSING: f32 = 3.0;
/// Target track must be within this angle of the reciprocal of the own heading (deg).
pub const HEADON_OPPOSITE_ANGLE: f32 = 45.0;
/// A manual selection falls back to automatic after this long without input (ms).
pub const SELECT_TIMEOUT_MS: i64 = 20_000;

/// Which screen to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewKind {
    /// No data received yet since start.
    Startup,
    /// No valid line from FLARM for 5 s.
    NoFlarm,
    /// FLARM reports no GPS fix.
    NoGps,
    /// No alarm: team mate, selected/nearest traffic or empty sky (see [`IdleKind`]).
    Idle,
    /// Directional warning.
    Warn,
    /// Head-on warning (threat straight ahead within +-10 deg bearing and elevation, approaching).
    HeadOn,
    /// Non-directional warning (transponder without position).
    WarnNonDir,
    /// Obstacle or alert zone.
    Obstacle,
}

/// What the idle screen focuses on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleKind {
    /// Team mate set: show the team mate (also while lost / searching).
    Team,
    /// No team mate: nearest traffic (front weighted).
    Nearest,
    /// Manually selected target (not the team mate).
    Selected,
    /// No traffic.
    Clear,
}

/// Everything a radar / warning screen needs about one target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetInfo {
    /// Entry filled.
    pub valid: bool,
    /// FLARM / ICAO ID (0 if unknown, e.g. PFLAU-only threat without ID).
    pub id: u32,
    /// Display frame: metres to the right of the own heading.
    pub x: f32,
    /// Display frame: metres forward along the own heading.
    pub y: f32,
    /// Bearing relative to own heading (deg, -180..180, right positive).
    pub bearing: f32,
    /// Horizontal distance (m).
    pub dist: f32,
    /// Relative vertical (m, positive = higher).
    pub rel_v: f32,
    /// Elevation angle above/below the horizon (deg).
    pub elev: f32,
    /// Target track relative to own heading (deg, -180..180), `None` without track.
    pub track_rel: Option<f32>,
    /// Climb rate (m/s) if known.
    pub climb: Option<f32>,
    /// Closing speed (m/s, positive = approaching), 0 if unknown.
    pub closing: f32,
    /// FLARM aircraft type (1 = glider, ...).
    pub acft_type: u8,
    /// Alarm level of this target (0..3).
    pub alarm_level: u8,
    /// This is the team mate.
    pub is_team: bool,
    /// This is the manually selected target.
    pub is_selected: bool,
    /// Target is circling (|turn rate| > 8 deg/s).
    pub circling: bool,
    /// Seconds since the last message.
    pub age_s: f32,
    /// Team mate no longer received (show "?" and `age_s`).
    pub lost: bool,
}

impl TargetInfo {
    /// Empty / invalid entry.
    pub const EMPTY: TargetInfo = TargetInfo {
        valid: false,
        id: 0,
        x: 0.0,
        y: 0.0,
        bearing: 0.0,
        dist: 0.0,
        rel_v: 0.0,
        elev: 0.0,
        track_rel: None,
        climb: None,
        closing: 0.0,
        acft_type: 0,
        alarm_level: 0,
        is_team: false,
        is_selected: false,
        circling: false,
        age_s: 0.0,
        lost: false,
    };
}

/// Other target in the radar list (same content as [`TargetInfo`]).
pub type OtherTarget = TargetInfo;

/// Non-directional target (distance and height only).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NonDirInfo {
    /// Estimated distance (m).
    pub dist: f32,
    /// Relative vertical (m).
    pub rel_v: f32,
    /// Alarm level.
    pub alarm: u8,
}

impl NonDirInfo {
    /// Empty entry.
    pub const EMPTY: NonDirInfo = NonDirInfo {
        dist: 0.0,
        rel_v: 0.0,
        alarm: 0,
    };
}

/// Result of [`compute`]: everything the screen needs for one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlarmView {
    /// Screen to show.
    pub kind: ViewKind,
    /// Focus of the idle screen.
    pub idle: IdleKind,
    /// Alarm level of the shown warning (0 if no warning shown).
    pub level: u8,
    /// FLARM alarm type of the shown warning.
    pub alarm_type: u8,
    /// The threat (valid for Warn, HeadOn, WarnNonDir, Obstacle).
    pub threat: TargetInfo,
    /// Configured team mate ID (0 = none).
    pub team_id: u32,
    /// Team mate (`None` if none set or never seen; check `team_id` for "searching").
    pub team: Option<TargetInfo>,
    /// Target for the non-team field: the manual selection if any, otherwise the nearest (front weighted).
    pub selected: Option<TargetInfo>,
    /// `selected` is a manual selection.
    pub sel_manual: bool,
    /// Event: the manual selection was dropped in this frame because the target is gone ("Selection lost").
    pub sel_lost: bool,
    /// Nearest visible targets, sorted by distance; first `n_others` valid.
    pub others: [OtherTarget; MAX_OTHERS],
    /// Number of valid entries in `others`.
    pub n_others: usize,
    /// Nearest non-directional targets, nearest first; first `n_nondir` valid.
    pub nondir: [NonDirInfo; MAX_NONDIR_VIEW],
    /// Number of valid entries in `nondir`.
    pub n_nondir: usize,
    /// Display heading (deg true).
    pub own_heading: f32,
    /// Heading from the external attitude (otherwise GPS track).
    pub heading_from_attitude: bool,
    /// Roll (deg) with attitude source, else 0.
    pub roll: f32,
    /// Pitch (deg) with attitude source, else 0.
    pub pitch: f32,
    /// Own ground speed (m/s).
    pub own_gs: f32,
    /// Own circling.
    pub circling: bool,
    /// GPS ok (valid RMC and FLARM does not report "no GPS").
    pub gps_ok: bool,
    /// Number of devices received (PFLAU RX).
    pub rx_count: u8,
    /// Local time (hh, mm, ss) including the configured time zone offset.
    pub time: Option<(u8, u8, u8)>,
}

impl FlarmView {
    /// Empty view (startup).
    pub const EMPTY: FlarmView = FlarmView {
        kind: ViewKind::Startup,
        idle: IdleKind::Clear,
        level: 0,
        alarm_type: 0,
        threat: TargetInfo::EMPTY,
        team_id: 0,
        team: None,
        selected: None,
        sel_manual: false,
        sel_lost: false,
        others: [TargetInfo::EMPTY; MAX_OTHERS],
        n_others: 0,
        nondir: [NonDirInfo::EMPTY; MAX_NONDIR_VIEW],
        n_nondir: 0,
        own_heading: 0.0,
        heading_from_attitude: false,
        roll: 0.0,
        pitch: 0.0,
        own_gs: 0.0,
        circling: false,
        gps_ok: false,
        rx_count: 0,
        time: None,
    };

    /// The valid part of `others`.
    pub fn others(&self) -> &[OtherTarget] {
        self.others.get(..self.n_others).unwrap_or(&[])
    }

    /// The valid part of `nondir`.
    pub fn nondir(&self) -> &[NonDirInfo] {
        self.nondir.get(..self.n_nondir).unwrap_or(&[])
    }

    /// A warning screen is shown.
    pub fn is_warning(&self) -> bool {
        matches!(
            self.kind,
            ViewKind::Warn | ViewKind::HeadOn | ViewKind::WarnNonDir | ViewKind::Obstacle
        )
    }
}

/// Configuration of the display logic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicConfig {
    /// Warnings are shown from this FLARM alarm level on (clamped to 1..3; 1 = like the SteFly C prototype).
    pub min_level: u8,
    /// Head-on view enabled (otherwise always the directional warning).
    pub headon_enabled: bool,
    /// Time zone offset for the clock (minutes).
    pub tz_offset_min: i32,
}

impl LogicConfig {
    /// min_level 1, head-on enabled, UTC.
    pub const DEFAULT: LogicConfig = LogicConfig {
        min_level: 1,
        headon_enabled: true,
        tz_offset_min: 0,
    };
}

impl Default for LogicConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Result of [`LogicState::follow_selected`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowResult {
    /// The selected target is now the team mate (ID). Persist it if wanted.
    Followed(u32),
    /// The selected target was the team mate and is no longer (ID).
    Unfollowed(u32),
    /// Nothing selected, nothing changed.
    NothingSelected,
}

/// State kept between frames: manual selection and head-on hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicState {
    /// Manually selected target ID, 0 = automatic.
    pub sel_id: u32,
    /// Time of the last selection input (ms).
    pub sel_t: i64,
    headon_id: u32,
    headon_until: Option<i64>,
}

impl Default for LogicState {
    fn default() -> Self {
        Self::new()
    }
}

/* ---------------- target info ---------------- */

/// Build the info for a target at relative (rn, re, rv) with the display heading.
fn make_info(
    t: &Traffic,
    g: Option<&Target>,
    rn: f32,
    re: f32,
    rv: f32,
    heading: f32,
    now: i64,
) -> TargetInfo {
    let mut ti = TargetInfo::EMPTY;
    ti.valid = true;
    ti.dist = crate::utils::math::sqrt(rn * rn + re * re);
    ti.rel_v = rv;
    ti.bearing = wrap180(re.atan2(rn) * RAD2DEG - heading);
    let h = heading * DEG2RAD;
    let (sh, ch) = (h.sin(), h.cos());
    ti.x = re * ch - rn * sh;
    ti.y = rn * ch + re * sh;
    ti.elev = rv.atan2(ti.dist.max(1.0)) * RAD2DEG;
    if let Some(g) = g {
        ti.id = g.id;
        ti.acft_type = g.raw.acft_type;
        ti.alarm_level = g.raw.alarm_level;
        ti.age_s = since(now, g.t_rx) as f32 / 1000.0;
        ti.climb = g.raw.climb;
        ti.circling = g.turn.abs() > 8.0;
        if g.has_vel {
            let dt = ti.age_s.max(0.0).min(3.0);
            let trk = g.trk + g.turn * dt;
            ti.track_rel = Some(wrap180(trk - heading));
            // closing speed
            let (vtn, vte) = (g.gs * (trk * DEG2RAD).cos(), g.gs * (trk * DEG2RAD).sin());
            let (mut von, mut voe) = (0.0, 0.0);
            if t.has_trk {
                von = t.gs * (t.trk * DEG2RAD).cos();
                voe = t.gs * (t.trk * DEG2RAD).sin();
            }
            let (vn, ve) = (vtn - von, vte - voe);
            if ti.dist > 1.0 {
                ti.closing = -(rn * vn + re * ve) / ti.dist;
            }
        }
    }
    ti
}

/// Info for a PFLAU-only position (bearing relative to the heading, distance, vertical).
fn polar_info(bearing: f32, dist: f32, rv: f32) -> TargetInfo {
    let mut ti = TargetInfo::EMPTY;
    ti.valid = true;
    ti.bearing = bearing;
    ti.dist = dist;
    ti.rel_v = rv;
    ti.x = dist * (bearing * DEG2RAD).sin();
    ti.y = dist * (bearing * DEG2RAD).cos();
    ti.elev = rv.atan2(dist.max(1.0)) * RAD2DEG;
    ti
}

struct Collected {
    others: [TargetInfo; MAX_OTHERS],
    n: usize,
    nearest: Option<TargetInfo>,
    sel: Option<TargetInfo>,
}

/// All visible targets: nearest 24 by distance, nearest (front weighted), selected.
fn collect(t: &Traffic, heading: f32, now: i64, sel_id: u32) -> Collected {
    let mut c = Collected {
        others: [TargetInfo::EMPTY; MAX_OTHERS],
        n: 0,
        nearest: None,
        sel: None,
    };
    let mut best = f32::MAX;
    for g in t.tg.iter() {
        if !g.used || since(now, g.t_rx) > TARGET_SHOW_MS {
            continue;
        }
        let (rn, re, rv) = t.pos_of(g, now);
        let mut ti = make_info(t, Some(g), rn, re, rv, heading, now);
        ti.is_team = t.team_id != 0 && g.id == t.team_id;
        ti.is_selected = sel_id != 0 && g.id == sel_id;
        // "nearest": front counts more than behind (behind up to factor 1.5 farther)
        let d3 = crate::utils::math::sqrt(ti.dist * ti.dist + rv * rv)
            * (1.0 + 0.25 * (1.0 - (ti.bearing * DEG2RAD).cos()));
        if d3 < best {
            best = d3;
            c.nearest = Some(ti);
        }
        if ti.is_selected {
            c.sel = Some(ti);
        }
        // keep only the nearest MAX_OTHERS (bounds drawing time in large gaggles)
        if c.n < MAX_OTHERS {
            c.others[c.n] = ti;
            c.n += 1;
        } else {
            let mut far = 0;
            for k in 1..MAX_OTHERS {
                if c.others[k].dist > c.others[far].dist {
                    far = k;
                }
            }
            if ti.dist < c.others[far].dist {
                c.others[far] = ti;
            }
        }
    }
    // sort by distance (insertion sort, at most 24 entries)
    for i in 1..c.n {
        let mut j = i;
        while j > 0 && c.others[j - 1].dist > c.others[j].dist {
            c.others.swap(j - 1, j);
            j -= 1;
        }
    }
    c
}

/* ---------------- compute ---------------- */

/// Compute the view for this frame. Also maintains the selection fallback in `st`.
pub fn compute(t: &Traffic, cfg: &LogicConfig, st: &mut LogicState, now_ms: i64) -> FlarmView {
    let mut v = FlarmView::EMPTY;
    compute_into(&mut v, t, cfg, st, now_ms);
    v
}

/// Like [`compute`], but writes into an existing view (avoids a large copy).
pub fn compute_into(
    v: &mut FlarmView,
    t: &Traffic,
    cfg: &LogicConfig,
    st: &mut LogicState,
    now_ms: i64,
) {
    let now = now_ms;
    *v = FlarmView::EMPTY;
    let (heading, from_att) = t.own_heading(now);
    v.own_heading = heading;
    v.heading_from_attitude = from_att;
    if from_att {
        v.roll = t.att_roll;
        v.pitch = t.att_pitch;
    }
    v.own_gs = t.gs;
    v.circling = t.circling;
    v.rx_count = t.pflau.rx;
    v.team_id = t.team_id;

    if let Some((hh, mm, ss)) = t.time {
        let secs = (hh as i64 * 3600 + mm as i64 * 60 + ss as i64)
            .saturating_add(since(now, t.t_time) / 1000)
            .saturating_add(cfg.tz_offset_min as i64 * 60)
            .rem_euclid(86_400);
        v.time = Some((
            (secs / 3600) as u8,
            ((secs / 60) % 60) as u8,
            (secs % 60) as u8,
        ));
    }

    /* --- targets and selection --- */
    let mut col = collect(t, heading, now, st.sel_id);
    let mut sel_info = None;
    if st.sel_id != 0 {
        match col.sel {
            None => {
                v.sel_lost = true;
                st.sel_id = 0;
            }
            Some(s) => {
                let dt = since(now, st.sel_t);
                if dt > SELECT_TIMEOUT_MS || dt < -SELECT_TIMEOUT_MS {
                    st.sel_id = 0;
                } else {
                    sel_info = Some(s);
                }
            }
        }
        if st.sel_id == 0 {
            for o in col.others.iter_mut() {
                o.is_selected = false;
            }
            if let Some(n) = col.nearest.as_mut() {
                n.is_selected = false;
            }
        }
    }
    v.others = col.others;
    v.n_others = col.n;
    v.sel_manual = sel_info.is_some();
    v.selected = sel_info.or(col.nearest);

    /* --- non-directional targets: the two nearest --- */
    for nd in t.nd.iter().take(MAX_NONDIR) {
        if !nd.used || since(now, nd.t_rx) > TARGET_SHOW_MS {
            continue;
        }
        let e = NonDirInfo {
            dist: nd.dist,
            rel_v: nd.rv,
            alarm: nd.alarm,
        };
        if v.n_nondir < MAX_NONDIR_VIEW {
            v.nondir[v.n_nondir] = e;
            v.n_nondir += 1;
        } else if e.dist < v.nondir[1].dist {
            v.nondir[1] = e;
        }
        if v.n_nondir == 2 && v.nondir[1].dist < v.nondir[0].dist {
            v.nondir.swap(0, 1);
        }
    }

    /* --- team mate --- */
    if t.team_id != 0 {
        if let Some(r) = t.team_rel(now) {
            let g = t.find(t.team_id).and_then(|i| t.target(i));
            let g = if r.age_s < TEAM_LOST_S { g } else { None };
            let mut ti = make_info(t, g, r.rn, r.re, r.rv, heading, now);
            ti.id = t.team_id;
            ti.age_s = r.age_s;
            ti.lost = r.age_s >= TEAM_LOST_S;
            ti.is_team = true;
            ti.is_selected = st.sel_id != 0 && st.sel_id == t.team_id;
            if ti.lost {
                ti.climb = Some(t.team.climb);
                ti.track_rel = None;
            }
            v.team = Some(ti);
        }
    }

    v.idle = if v.sel_manual && st.sel_id != t.team_id {
        IdleKind::Selected
    } else if t.team_id != 0 {
        IdleKind::Team
    } else if v.selected.is_some() {
        IdleKind::Nearest
    } else {
        IdleKind::Clear
    };

    /* --- status --- */
    let pflau_fresh = t
        .t_pflau
        .map_or(false, |tp| since(now, tp) < PFLAU_FRESH_MS);
    let data_ok = t
        .t_last_data
        .map_or(false, |td| since(now, td) < DATA_TIMEOUT_MS);
    v.gps_ok = t.gps_valid && !(pflau_fresh && t.pflau.gps == 0);

    if !data_ok {
        v.kind = if t.t_last_data.is_none() {
            ViewKind::Startup
        } else {
            ViewKind::NoFlarm
        };
        return;
    }
    if !v.gps_ok {
        v.kind = ViewKind::NoGps;
        return;
    }
    v.kind = ViewKind::Idle;
    let p = &t.pflau;
    let min_level = cfg.min_level.max(1).min(3);
    if !(pflau_fresh && p.alarm_level > 0 && p.alarm_level >= min_level) {
        return;
    }
    let rv_p = p.rel_vertical.unwrap_or(0) as f32;
    let dist_p = p.rel_distance as f32;
    if p.alarm_type == 3 || p.alarm_type >= 0x40 {
        v.kind = ViewKind::Obstacle;
        let brg = match p.rel_bearing {
            Some(b) => wrap180(b as f32 + t.trk - heading),
            None => 0.0,
        };
        v.threat = polar_info(brg, dist_p, rv_p);
    } else if p.alarm_type == 2 {
        let g = p.id.and_then(|id| t.find(id)).and_then(|i| t.target(i));
        match (g, p.rel_bearing) {
            (Some(g), _) if since(now, g.t_rx) < PFLAU_FRESH_MS => {
                let (rn, re, rv) = t.pos_of(g, now);
                v.threat = make_info(t, Some(g), rn, re, rv, heading, now);
                v.kind = ViewKind::Warn;
            }
            (_, Some(b)) => {
                let base = if t.has_trk { t.trk } else { heading };
                v.threat = polar_info(wrap180(b as f32 + base - heading), dist_p, rv_p);
                v.threat.id = p.id.unwrap_or(0);
                v.kind = ViewKind::Warn;
            }
            _ => {
                v.threat = TargetInfo::EMPTY;
                v.threat.valid = true;
                v.threat.dist = dist_p;
                v.threat.rel_v = rv_p;
                v.threat.id = p.id.unwrap_or(0);
                v.kind = ViewKind::WarnNonDir;
            }
        }
    } else {
        // traffic advisory or other types: no warning screen (like the SteFly C prototype)
        return;
    }
    v.level = p.alarm_level;
    v.alarm_type = p.alarm_type;
    v.threat.alarm_level = p.alarm_level;
    v.threat.is_team = t.team_id != 0 && v.threat.id == t.team_id;
    v.threat.is_selected = st.sel_id != 0 && v.threat.id == st.sel_id;

    /* --- head-on --- */
    if v.kind == ViewKind::Warn && cfg.headon_enabled {
        let th = &v.threat;
        let mut headon = false;
        // straight ahead within +-10 deg bearing and elevation, approaching, flying towards us.
        // Not while circling: the bearing changes too fast there, the top view is clearer.
        if !t.circling
            && th.bearing.abs() <= HEADON_ANGLE
            && th.elev.abs() <= HEADON_ANGLE
            && th.closing >= HEADON_MIN_CLOSING
        {
            let opposite = match th.track_rel {
                Some(tr) => wrap180(tr - 180.0).abs() <= HEADON_OPPOSITE_ANGLE,
                None => th.closing > t.gs * 1.3,
            };
            if opposite {
                headon = true;
                st.headon_id = th.id;
                st.headon_until = Some(now.saturating_add(HEADON_HOLD_MS));
            }
        }
        // once head-on: stay at least 2 s while it is the same target within +-12 deg
        if !headon {
            if let Some(until) = st.headon_until {
                if th.id == st.headon_id
                    && now < until
                    && now > until.saturating_sub(HEADON_HOLD_MS + 100)
                    && th.bearing.abs() <= HEADON_KEEP_ANGLE
                    && th.elev.abs() <= HEADON_KEEP_ANGLE
                {
                    headon = true;
                }
            }
        }
        if headon {
            v.kind = ViewKind::HeadOn;
        }
    }
}

/* ---------------- selection ---------------- */

/// Ordering key for cycling: bearing in [0, 360) clockwise from straight ahead, then ID.
fn key_lt(a: (f32, u32), b: (f32, u32)) -> bool {
    a.0 < b.0 || (a.0 == b.0 && a.1 < b.1)
}

impl LogicState {
    /// No selection.
    pub const fn new() -> Self {
        LogicState {
            sel_id: 0,
            sel_t: 0,
            headon_id: 0,
            headon_until: None,
        }
    }

    /// Currently selected ID (0 = automatic).
    pub fn selected_id(&self) -> u32 {
        self.sel_id
    }

    /// Drop the manual selection.
    pub fn clear_selection(&mut self) {
        self.sel_id = 0;
    }

    /// Rotary step: select the next visible target by relative bearing
    /// (clockwise = increasing bearing, wraps around), starting from the
    /// current selection or, without one, from straight ahead. Candidates are
    /// the targets shown in the radar list (nearest 24). Returns the new
    /// selected ID (0 if there is no target).
    pub fn select_step(&mut self, t: &Traffic, now_ms: i64, clockwise: bool) -> u32 {
        self.sel_t = now_ms;
        let (heading, _) = t.own_heading(now_ms);
        let col = collect(t, heading, now_ms, self.sel_id);
        let cands = col.others.get(..col.n).unwrap_or(&[]);
        if cands.is_empty() {
            self.sel_id = 0;
            return 0;
        }
        let cur = col.sel.map(|s| (wrap360(s.bearing), s.id));
        let mut best: Option<(f32, u32)> = None; // next in direction
        let mut wrap: Option<(f32, u32)> = None; // first / last overall
        for c in cands {
            let k = (wrap360(c.bearing), c.id);
            if cur.map_or(false, |cu| cu.1 == k.1) {
                continue;
            }
            if clockwise {
                if wrap.map_or(true, |w| key_lt(k, w)) {
                    wrap = Some(k);
                }
                if let Some(cu) = cur {
                    if key_lt(cu, k) && best.map_or(true, |b| key_lt(k, b)) {
                        best = Some(k);
                    }
                }
            } else {
                if wrap.map_or(true, |w| key_lt(w, k)) {
                    wrap = Some(k);
                }
                if let Some(cu) = cur {
                    if key_lt(k, cu) && best.map_or(true, |b| key_lt(b, k)) {
                        best = Some(k);
                    }
                }
            }
        }
        // only the current selection itself is visible: keep it
        self.sel_id = match best.or(wrap) {
            Some(k) => k.1,
            None => cur.map_or(0, |c| c.1),
        };
        self.sel_id
    }

    /// Make the selected target the team mate, or clear the team mate if the
    /// selected target already is it. The selection is dropped afterwards.
    pub fn follow_selected(&mut self, t: &mut Traffic, now_ms: i64) -> FollowResult {
        self.sel_t = now_ms;
        let id = self.sel_id;
        if id == 0 {
            return FollowResult::NothingSelected;
        }
        self.sel_id = 0;
        if t.team_id == id {
            t.set_team(0);
            FollowResult::Unfollowed(id)
        } else {
            t.set_team(id);
            FollowResult::Followed(id)
        }
    }
}
