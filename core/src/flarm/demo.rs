//! Demo traffic generator for presentation and tests.
//!
//! The scenarios are simulated physically and emitted as real NMEA sentences
//! (GPRMC, PFLAA, PFLAU) exactly like a FLARM sends them, so the demo runs
//! through the same code as in flight. Positions are f64 (metres), angles and
//! trigonometry f32 (works with micromath in `no_std`).

#[cfg(not(flarm_standalone))]
#[allow(unused_imports)]
use micromath::F32Ext;

use super::nmea::SentenceWriter;
use super::traffic::{cos_deg64, since, wrap360, DEG2RAD, RAD2DEG};

/// Maximum simulated objects per scenario.
pub const MAX_OBJ: usize = 10;
/// Integration step (ms).
const STEP_MS: i64 = 20;
const REF_LAT: f64 = 48.40;
const REF_LON: f64 = 11.40;

/// Demo scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// Crossing traffic from the right, rising alarm level.
    Crossing,
    /// Oncoming traffic straight ahead.
    HeadOn,
    /// Team mate approaching, later reception lost and regained.
    Team,
    /// Several gliders in the same thermal.
    Thermal,
    /// Cruise with varying speed, team mate behind.
    Cruise,
    /// Mixed traffic: all aircraft types and a transponder without position.
    Mixed,
    /// All scenarios one after another, repeating.
    All,
}

impl Scenario {
    /// The individual scenarios (without `All`).
    pub const LIST: [Scenario; 6] = [
        Scenario::Crossing,
        Scenario::HeadOn,
        Scenario::Team,
        Scenario::Thermal,
        Scenario::Cruise,
        Scenario::Mixed,
    ];

    /// English display name.
    pub fn name(self) -> &'static str {
        match self {
            Scenario::Crossing => "Crossing",
            Scenario::HeadOn => "Head-on",
            Scenario::Team => "Team",
            Scenario::Thermal => "Thermal",
            Scenario::Cruise => "Cruise",
            Scenario::Mixed => "Mixed",
            Scenario::All => "All",
        }
    }

    /// Scenario by index 0..=6 (6 = All); out of range gives `All`.
    pub fn from_index(i: usize) -> Scenario {
        Self::LIST.get(i).copied().unwrap_or(Scenario::All)
    }

    /// Next individual scenario (wraps; `All` gives `Crossing`).
    pub fn next(self) -> Scenario {
        let i = Self::LIST
            .iter()
            .position(|&s| s == self)
            .map_or(0, |i| (i + 1) % Self::LIST.len());
        Self::LIST[i]
    }
}

/// One simulated aircraft.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DemoObj {
    /// Currently received (false = simulated reception loss).
    pub active: bool,
    /// ID.
    pub id: u32,
    /// FLARM aircraft type.
    pub acft: u8,
    /// East (m).
    pub e: f64,
    /// North (m).
    pub n: f64,
    /// Altitude (m).
    pub alt: f64,
    /// Track (deg true).
    pub trk: f32,
    /// Ground speed (m/s).
    pub gs: f32,
    /// Turn rate (deg/s, right positive).
    pub turn: f32,
    /// Climb rate (m/s).
    pub climb: f32,
    /// Transponder only (Mode-S/C): emitted without direction.
    pub nondir: bool,
}

impl DemoObj {
    /// Inactive object at the origin.
    pub const EMPTY: DemoObj = DemoObj {
        active: false,
        id: 0,
        acft: 0,
        e: 0.0,
        n: 0.0,
        alt: 0.0,
        trk: 0.0,
        gs: 0.0,
        turn: 0.0,
        climb: 0.0,
        nondir: false,
    };
}

/// Competition numbers of the demo targets (for the name display).
pub fn lookup_name(id: u32) -> Option<&'static str> {
    const NAMES: [(u32, &str); 15] = [
        (0xDD1A2B, "KX"),
        (0xDD4F10, "D-1234"),
        (0xDDA5C3, "SL"),
        (0xDD77E1, "LS"),
        (0xDD9002, "M7"),
        (0xDD3B44, "WA"),
        (0xDDC0DE, "D-KEEE"),
        (0xDD5150, "S1"),
        (0xDDE001, "D-EFLY"),
        (0xDDE002, "JET"),
        (0xDDE003, "D-HELI"),
        (0xDDE004, "HG"),
        (0xDDE005, "PG"),
        (0xDDE006, "UAV"),
        (0xDDE007, "D-OBAL"),
    ];
    NAMES.iter().find(|(i, _)| *i == id).map(|(_, n)| *n)
}

/// Demo generator. Call [`Demo::step`] with the current time (any epoch);
/// the first call defines the scenario start.
#[derive(Clone, Debug)]
pub struct Demo {
    mode: Scenario,
    sc: Scenario,
    t0: Option<i64>,
    restarted: bool,
    t_ms: i64,
    next_out_ms: i64,
    own: DemoObj,
    obj: [DemoObj; MAX_OBJ],
    n_obj: usize,
    team_id: u32,
    duration_s: i64,
}

fn round_i64(x: f64) -> i64 {
    if !x.is_finite() {
        return 0;
    }
    if x >= 0.0 {
        (x + 0.5) as i64
    } else {
        (x - 0.5) as i64
    }
}

fn fabs(x: f64) -> f64 {
    if x < 0.0 {
        -x
    } else {
        x
    }
}

/// Put an object on a right-hand circle around (ce, cn).
fn put_on_circle(o: &mut DemoObj, ce: f64, cn: f64, r: f64, trk: f32, turn_dps: f32) {
    let th = trk * DEG2RAD;
    o.e = ce - r * th.cos() as f64;
    o.n = cn + r * th.sin() as f64;
    o.trk = trk;
    o.turn = turn_dps;
    o.gs = r as f32 * turn_dps * DEG2RAD;
}

/// Choose the start so that the object is at (me, mn) at time t_meet (s).
fn aim(o: &mut DemoObj, me: f64, mn: f64, trk: f32, gs: f32, t_meet: f32) {
    o.trk = trk;
    o.gs = gs;
    o.e = me - (gs * (trk * DEG2RAD).sin() * t_meet) as f64;
    o.n = mn - (gs * (trk * DEG2RAD).cos() * t_meet) as f64;
}

fn integrate(o: &mut DemoObj, dt: f64) {
    let th = o.trk * DEG2RAD;
    o.e += (o.gs * th.sin()) as f64 * dt;
    o.n += (o.gs * th.cos()) as f64 * dt;
    o.alt += o.climb as f64 * dt;
    o.trk = wrap360(o.trk + o.turn * dt as f32);
}

/// Alarm level from the predicted closest approach.
fn alarm_level(own: &DemoObj, o: &DemoObj) -> i64 {
    let (pe, pn, pu) = (o.e - own.e, o.n - own.n, o.alt - own.alt);
    let (to, tw) = (o.trk * DEG2RAD, own.trk * DEG2RAD);
    let ve = (o.gs * to.sin() - own.gs * tw.sin()) as f64;
    let vn = (o.gs * to.cos() - own.gs * tw.cos()) as f64;
    let vu = (o.climb - own.climb) as f64;
    let vv = ve * ve + vn * vn;
    if vv < 1.0 {
        return 0;
    }
    let tc = -(pe * ve + pn * vn) / vv;
    if !(0.0..=22.0).contains(&tc) {
        return 0;
    }
    let (de, dn, du) = (pe + ve * tc, pn + vn * tc, pu + vu * tc);
    if ((de * de + dn * dn) as f32).sqrt() > 110.0 || fabs(du) > 60.0 {
        return 0;
    }
    if tc < 8.0 {
        3
    } else if tc < 13.0 {
        2
    } else {
        1
    }
}

fn write_coord(w: &mut SentenceWriter, v: f64, lat: bool) {
    let a = fabs(v);
    let deg = a as i64;
    let min = (a - deg as f64) * 60.0;
    if lat {
        w.int_pad(deg, 2)
            .fixed(min, 5, 8)
            .byte(b',')
            .byte(if v >= 0.0 { b'N' } else { b'S' });
    } else {
        w.int_pad(deg, 3)
            .fixed(min, 5, 8)
            .byte(b',')
            .byte(if v >= 0.0 { b'E' } else { b'W' });
    }
}

/// Copy a finished line into `out` if it fits completely.
fn append(out: &mut [u8], len: &mut usize, line: &[u8]) {
    if line.is_empty() {
        return;
    }
    if let Some(dst) = out.get_mut(*len..*len + line.len()) {
        dst.copy_from_slice(line);
        *len += line.len();
    }
}

impl Demo {
    /// New demo for `scenario` (`All` starts with `Crossing` and cycles).
    pub fn new(scenario: Scenario) -> Self {
        let mut d = Demo {
            mode: scenario,
            sc: Scenario::Crossing,
            t0: None,
            restarted: false,
            t_ms: 0,
            next_out_ms: 0,
            own: DemoObj::EMPTY,
            obj: [DemoObj::EMPTY; MAX_OBJ],
            n_obj: 0,
            team_id: 0,
            duration_s: 0,
        };
        let first = if scenario == Scenario::All {
            Scenario::Crossing
        } else {
            scenario
        };
        d.init(first);
        d
    }

    /// Mode given to [`Demo::new`].
    pub fn mode(&self) -> Scenario {
        self.mode
    }

    /// Scenario currently running.
    pub fn scenario(&self) -> Scenario {
        self.sc
    }

    /// Team mate ID of the running scenario (0 = none). Set it in the traffic model on (re)start.
    pub fn team_id(&self) -> u32 {
        self.team_id
    }

    /// Duration of the running scenario (ms). After it, the scenario restarts (or the next one starts in `All`).
    pub fn duration_ms(&self) -> i64 {
        self.duration_s * 1000
    }

    /// Simulated time within the running scenario (ms).
    pub fn sim_time_ms(&self) -> i64 {
        self.t_ms
    }

    /// Own aircraft (truth).
    pub fn own(&self) -> &DemoObj {
        &self.own
    }

    /// Simulated objects (truth).
    pub fn objects(&self) -> &[DemoObj] {
        self.obj.get(..self.n_obj).unwrap_or(&[])
    }

    /// `true` once after a scenario (re)start: clear the traffic model and set [`Demo::team_id`].
    pub fn take_restart(&mut self) -> bool {
        core::mem::replace(&mut self.restarted, false)
    }

    /// Advance the simulation to `now_ms`. When a full second is reached, the
    /// NMEA sentences (GPRMC, PFLAA..., PFLAU) are written to `out` and the
    /// length is returned, otherwise 0. Lines that do not fit are dropped.
    pub fn step(&mut self, now_ms: i64, out: &mut [u8]) -> usize {
        let t0 = *self.t0.get_or_insert(now_ms);
        let mut local = since(now_ms, t0);
        if local < 0 || local > self.duration_ms() {
            let next = if self.mode == Scenario::All {
                self.sc.next()
            } else {
                self.sc
            };
            self.init(next);
            self.t0 = Some(now_ms);
            self.restarted = true;
            local = 0;
        }
        let mut len = 0;
        while self.t_ms < local {
            let dt = (local - self.t_ms).min(STEP_MS);
            let dts = dt as f64 / 1000.0;
            integrate(&mut self.own, dts);
            for o in self.obj.iter_mut().take(self.n_obj) {
                integrate(o, dts);
            }
            self.t_ms += dt;
            self.scenario_events();
            if self.t_ms >= self.next_out_ms {
                len = self.emit(out); // at most one output per call
                self.next_out_ms += 1000;
            }
        }
        len
    }

    fn add(&mut self, id: u32, acft: u8) -> Option<&mut DemoObj> {
        let o = self.obj.get_mut(self.n_obj)?;
        self.n_obj += 1;
        *o = DemoObj::EMPTY;
        o.active = true;
        o.id = id;
        o.acft = acft;
        Some(o)
    }

    fn init(&mut self, sc: Scenario) {
        self.sc = sc;
        self.t_ms = 0;
        self.next_out_ms = 0;
        self.own = DemoObj::EMPTY;
        self.own.alt = 1500.0;
        self.obj = [DemoObj::EMPTY; MAX_OBJ];
        self.n_obj = 0;
        self.team_id = 0;
        match sc {
            Scenario::Crossing => {
                self.duration_s = 40;
                self.own.trk = 0.0;
                self.own.gs = 33.3;
                if let Some(o) = self.add(0xDD1A2B, 1) {
                    // from the right, 30 m higher
                    aim(o, 35.0, 33.3 * 24.0, 245.0, 30.0, 24.0);
                    o.alt = 1530.0;
                }
                if let Some(o) = self.add(0xDD4F10, 1) {
                    // far left, circling
                    put_on_circle(o, -2000.0, 900.0, 90.0, 30.0, 17.0);
                    o.alt = 1650.0;
                    o.climb = 1.8;
                }
                if let Some(o) = self.add(0xDD3B44, 1) {
                    // behind left, same direction
                    o.e = -900.0;
                    o.n = -1200.0;
                    o.trk = 10.0;
                    o.gs = 31.0;
                    o.alt = 1420.0;
                }
            }
            Scenario::HeadOn => {
                self.duration_s = 30;
                self.own.trk = 0.0;
                self.own.gs = 33.3;
                if let Some(o) = self.add(0xDD9002, 1) {
                    aim(o, 25.0, 33.3 * 20.0, 180.0, 34.0, 20.0);
                    o.alt = 1488.0;
                }
                if let Some(o) = self.add(0xDD4F10, 1) {
                    put_on_circle(o, 1800.0, 2500.0, 95.0, 300.0, 16.0);
                    o.alt = 1620.0;
                    o.climb = 1.5;
                }
            }
            Scenario::Team => {
                self.duration_s = 70;
                put_on_circle(&mut self.own, 0.0, 0.0, 85.0, 0.0, 18.0);
                self.own.climb = 2.0;
                self.team_id = 0xDDA5C3;
                if let Some(o) = self.add(0xDDA5C3, 1) {
                    // team mate approaching
                    o.e = 2300.0;
                    o.n = 1700.0;
                    o.trk = 233.0;
                    o.gs = 41.0;
                    o.alt = 1680.0;
                    o.climb = -1.1;
                }
                if let Some(o) = self.add(0xDD77E1, 1) {
                    // same thermal, higher
                    put_on_circle(o, 0.0, 0.0, 90.0, 180.0, 17.0);
                    o.alt = 1720.0;
                    o.climb = 2.1;
                }
                if let Some(o) = self.add(0xDD5150, 1) {
                    // same thermal, lower
                    put_on_circle(o, 15.0, 10.0, 80.0, 90.0, 19.0);
                    o.alt = 1380.0;
                    o.climb = 2.3;
                }
                if let Some(o) = self.add(0xDDC0DE, 2) {
                    // powered aircraft far away
                    o.e = -3500.0;
                    o.n = -500.0;
                    o.trk = 80.0;
                    o.gs = 55.0;
                    o.alt = 1300.0;
                }
            }
            Scenario::Cruise => {
                self.duration_s = 60;
                self.own.trk = 40.0;
                self.own.gs = 34.0;
                self.team_id = 0xDDA5C3;
                if let Some(o) = self.add(0xDDA5C3, 1) {
                    // team mate behind left, same course
                    o.e = -700.0;
                    o.n = -350.0;
                    o.trk = 42.0;
                    o.gs = 35.0;
                    o.alt = 1460.0;
                    o.climb = -0.9;
                }
                if let Some(o) = self.add(0xDD4F10, 1) {
                    // far right, circling
                    put_on_circle(o, 2600.0, 2200.0, 90.0, 30.0, 17.0);
                    o.alt = 1750.0;
                    o.climb = 2.2;
                }
                if let Some(o) = self.add(0xDD3B44, 1) {
                    o.e = 1500.0;
                    o.n = -900.0;
                    o.trk = 220.0;
                    o.gs = 30.0;
                    o.alt = 1300.0;
                    o.climb = -1.0;
                }
            }
            Scenario::Mixed => {
                self.duration_s = 60;
                self.own.trk = 0.0;
                self.own.gs = 30.0;
                // (id, type, e, n, trk, gs, alt)
                let straight: [(u32, u8, f64, f64, f32, f32, f64); 5] = [
                    (0xDDE001, 8, -1400.0, 1800.0, 120.0, 50.0, 1650.0), // piston aircraft
                    (0xDDE002, 9, 3000.0, 3500.0, 250.0, 110.0, 1900.0), // jet
                    (0xDDE003, 3, 1600.0, 500.0, 300.0, 40.0, 1350.0),   // helicopter
                    (0xDDE006, 0xD, 350.0, 600.0, 90.0, 8.0, 1420.0),    // drone
                    (0xDDE007, 0xB, 2200.0, -400.0, 60.0, 4.0, 1600.0),  // balloon
                ];
                for (k, &(id, ty, e, n, trk, gs, alt)) in straight.iter().enumerate() {
                    if k == 3 {
                        // hang glider and paraglider circling (same order as the C demo)
                        if let Some(o) = self.add(0xDDE004, 6) {
                            put_on_circle(o, -800.0, 900.0, 70.0, 0.0, 14.0);
                            o.alt = 1560.0;
                            o.climb = 1.2;
                        }
                        if let Some(o) = self.add(0xDDE005, 7) {
                            put_on_circle(o, -650.0, 1050.0, 45.0, 200.0, 18.0);
                            o.alt = 1480.0;
                            o.climb = 1.4;
                        }
                    }
                    if let Some(o) = self.add(id, ty) {
                        o.e = e;
                        o.n = n;
                        o.trk = trk;
                        o.gs = gs;
                        o.alt = alt;
                    }
                }
                if let Some(o) = self.add(0xDDE008, 8) {
                    // transponder without position
                    o.e = 900.0;
                    o.n = 2600.0;
                    o.trk = 180.0;
                    o.gs = 45.0;
                    o.alt = 1650.0;
                    o.nondir = true;
                }
            }
            Scenario::Thermal | Scenario::All => {
                self.duration_s = 40;
                put_on_circle(&mut self.own, 0.0, 0.0, 80.0, 0.0, 20.0);
                self.own.climb = 2.5;
                if let Some(o) = self.add(0xDD77E1, 1) {
                    put_on_circle(o, 0.0, 0.0, 85.0, 120.0, 19.0);
                    o.alt = 1540.0;
                    o.climb = 2.4;
                }
                if let Some(o) = self.add(0xDD5150, 1) {
                    put_on_circle(o, 0.0, 0.0, 78.0, 240.0, 20.5);
                    o.alt = 1440.0;
                    o.climb = 2.6;
                }
                if let Some(o) = self.add(0xDD9002, 1) {
                    put_on_circle(o, 10.0, -5.0, 82.0, 200.0, 20.0);
                    o.alt = 1512.0;
                    o.climb = 2.5;
                }
            }
        }
    }

    fn scenario_events(&mut self) {
        let t = self.t_ms as f32 / 1000.0;
        if self.sc == Scenario::Cruise {
            // speed oscillates between about 95 and 185 km/h
            self.own.gs = 38.5 + 12.5 * (t * 2.0 * core::f32::consts::PI / 30.0).sin();
        }
        if self.sc == Scenario::Team {
            if let Some(tp) = self.obj.get_mut(0) {
                // from 32 s circling in the thermal, 40..55 s no reception
                if t >= 32.0 && tp.turn == 0.0 {
                    tp.turn = 17.0;
                    tp.gs = 27.0;
                    tp.climb = 1.6;
                }
                tp.active = !(40.0..55.0).contains(&t);
            }
        }
    }

    fn emit(&self, out: &mut [u8]) -> usize {
        let mut len = 0;
        let mut line = [0u8; 128];
        let sec = self.t_ms / 1000;
        let (hh, mm, ss) = (12 + sec / 3600, (sec / 60) % 60, sec % 60);
        let lat = REF_LAT + self.own.n / 111_320.0;
        let lon = REF_LON + self.own.e / (111_320.0 * cos_deg64(lat));
        let n = {
            let mut w = SentenceWriter::new(&mut line);
            w.str("$GPRMC,")
                .int_pad(hh, 2)
                .int_pad(mm, 2)
                .int_pad(ss, 2)
                .str(".00,A,");
            write_coord(&mut w, lat, true);
            w.byte(b',');
            write_coord(&mut w, lon, false);
            w.byte(b',').fixed(self.own.gs as f64 / 0.514444, 1, 0);
            w.byte(b',')
                .fixed(self.own.trk as f64, 1, 0)
                .str(",210526,,,A");
            w.finish()
        };
        append(out, &mut len, line.get(..n).unwrap_or(&[]));

        let mut best_lvl: i64 = -1;
        let mut best_d = f64::MAX;
        let mut best: Option<&DemoObj> = None;
        let mut rx = 0;
        for o in self.objects() {
            if !o.active {
                continue;
            }
            let (rn, re, rv) = (o.n - self.own.n, o.e - self.own.e, o.alt - self.own.alt);
            let dist = ((rn * rn + re * re) as f32).sqrt() as f64;
            if dist > 6000.0 || fabs(rv) > 1000.0 {
                continue;
            }
            rx += 1;
            let mut lvl = alarm_level(&self.own, o);
            let n = {
                let mut w = SentenceWriter::new(&mut line);
                if o.nondir {
                    // transponder without position: only estimated distance in RelativeNorth
                    lvl = if dist < 700.0 { 1 } else { 0 };
                    w.str("$PFLAA,")
                        .int(lvl)
                        .byte(b',')
                        .int(round_i64(dist))
                        .str(",,")
                        .int(round_i64(rv));
                    w.str(",1,").hex(o.id, 6).str(",,,,,").hex(o.acft as u32, 0);
                } else {
                    w.str("$PFLAA,")
                        .int(lvl)
                        .byte(b',')
                        .int(round_i64(rn))
                        .byte(b',')
                        .int(round_i64(re));
                    w.byte(b',')
                        .int(round_i64(rv))
                        .str(",2,")
                        .hex(o.id, 6)
                        .byte(b',');
                    w.int(round_i64(o.trk as f64) % 360)
                        .str(",,")
                        .int(round_i64(o.gs as f64));
                    w.byte(b',')
                        .fixed(o.climb as f64, 1, 0)
                        .byte(b',')
                        .hex(o.acft as u32, 0);
                }
                w.finish()
            };
            append(out, &mut len, line.get(..n).unwrap_or(&[]));
            if lvl > best_lvl || (lvl == best_lvl && dist < best_d) {
                best_lvl = lvl;
                best_d = dist;
                best = Some(o);
            }
        }
        let n = {
            let mut w = SentenceWriter::new(&mut line);
            match best {
                Some(b) => {
                    let (rn, re) = ((b.n - self.own.n) as f32, (b.e - self.own.e) as f32);
                    let brg = round_i64(
                        (wrap360(re.atan2(rn) * RAD2DEG - self.own.trk + 540.0) - 180.0) as f64,
                    );
                    w.str("$PFLAU,")
                        .int(rx)
                        .str(",1,2,1,")
                        .int(best_lvl)
                        .byte(b',');
                    if !b.nondir {
                        w.int(brg);
                    }
                    w.byte(b',')
                        .int(if best_lvl > 0 { 2 } else { 0 })
                        .byte(b',');
                    w.int(round_i64(b.alt - self.own.alt))
                        .byte(b',')
                        .int(round_i64(best_d))
                        .byte(b',')
                        .hex(b.id, 6);
                }
                None => {
                    w.str("$PFLAU,0,1,2,1,0,,0,,,");
                }
            }
            w.finish()
        };
        append(out, &mut len, line.get(..n).unwrap_or(&[]));
        len
    }
}
