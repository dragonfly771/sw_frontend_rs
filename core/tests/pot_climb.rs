//! Fixture test of the info box "Potential Climb" (controller/pot_climb.rs)
//!
//! Concept and acceptance criteria: plan_potential_v3.md (numbers in the
//! test names of chapter 4 there).
//!
//! Fixtures are pieces of real flights at 10 Hz, columns: roll [rad],
//! turn_rate [rad/s], vario [m/s], gnss_alt [m], ias [m/s], tas [m/s],
//! g_force [m/s2].
//!   pot_v3_as33.csv  AS 33, 592.5 kg: 60 s cruise, a thermal of 108 s
//!                    (2.9 m/s over the first three circles), departure
//!   pot_v3_ls3.csv   LS3, 430.3 kg: a circle given up after 17 s
//!                    (-0.1 m/s), a turn, a weak thermal (0.3 m/s)
//! The expected values are the output of a Python mirror of this module,
//! which reproduces the reference of the analysis on all 13 flights: 259
//! started circles of the AS 33, error of the number against the result
//! 0.82 m/s after 8 s (reference 0.82), 0.56 after 18 s (0.56).

use corelib::{
    polar_store::POLARS, AirSpeed, Density, FloatToMass, GliderData, Polar, PotClimb, PotInput,
    PotMode, PotVerdict,
};

const G: f32 = 9.80665;
const AS33: &str = include_str!("fixtures/pot_v3_as33.csv");
const AS33_EXPECTED: &str = include_str!("fixtures/pot_v3_as33_expected.csv");
const LS3: &str = include_str!("fixtures/pot_v3_ls3.csv");
const LS3_EXPECTED: &str = include_str!("fixtures/pot_v3_ls3_expected.csv");

/// glider of the polar store with pilot and water for the given weight
fn glider(name: &str, weight_kg: f32) -> GliderData {
    let basic = *POLARS
        .iter()
        .find(|p| p.name == name)
        .expect("polar not in the store");
    let mut gd = GliderData::from_basic_glider_data(basic);
    gd.pilot_weight = 90.0.kg();
    gd.water_ballast = (weight_kg - basic.empty_mass - 90.0).kg();
    gd
}

#[derive(Clone, Copy)]
struct Row {
    roll: f32,
    tr: f32,
    vario: f32,
    alt: f32,
    ias: f32,
    tas: f32,
    g_force: f32,
}

fn rows(csv: &str) -> Vec<Row> {
    csv.lines()
        .skip(1)
        .map(|line| {
            let v: Vec<f32> = line.split(',').map(|s| s.parse().unwrap()).collect();
            Row {
                roll: v[0],
                tr: v[1],
                vario: v[2],
                alt: v[3],
                ias: v[4],
                tas: v[5],
                g_force: v[6],
            }
        })
        .collect()
}

struct Tick {
    pot: Option<f32>,
    mode: PotMode,
    verdict: PotVerdict,
    circle_sink: f32,
}

/// what the controller hands to the box for one row
fn input(r: &Row, gd: &GliderData, mc: f32, bar_factor: f32) -> PotInput {
    PotInput {
        roll_rad: r.roll,
        turn_rate_rad_s: r.tr,
        vario_m_s: r.vario,
        gnss_alt_m: r.alt,
        ias_m_s: r.ias,
        tas_m_s: r.tas,
        load_factor: r.g_force / G,
        mc_m_s: mc,
        bar_factor,
        wing_area_m2: gd.basic_glider_data.wing_area,
    }
}

/// TAS / IAS of a row, limited like in the box
fn density_factor(r: &Row) -> f32 {
    (r.tas / r.ias.max(10.0)).clamp(1.0, 1.3)
}

/// Run the box over rows like the controller does: the polar follows weight
/// and density once a second (tick_1s), the box runs every 100 ms.
fn run_with(
    pot: &mut PotClimb,
    polar: &mut Polar,
    rows: &[Row],
    gd: &GliderData,
    mc: f32,
    bar_factor: f32,
) -> Vec<Tick> {
    let mut out = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let k = density_factor(r);
        if i % 10 == 0 {
            polar.recalc(gd, Density::from_kg_m3(Density::AT_NN().0 / (k * k)));
        }
        let v = pot.tick_raw(&input(r, gd, mc, bar_factor), polar);
        out.push(Tick {
            pot: v,
            mode: pot.mode(),
            verdict: pot.verdict(),
            circle_sink: pot.circle_sink(polar, gd.basic_glider_data.wing_area, k),
        });
    }
    out
}

/// a new box and the polar of the glider over the rows
fn run(rows: &[Row], gd: &GliderData, mc: f32, bar_factor: f32) -> Vec<Tick> {
    let mut polar = Polar::default();
    polar.recalc_glider(gd);
    run_with(&mut PotClimb::new(), &mut polar, rows, gd, mc, bar_factor)
}

fn mode_no(m: PotMode) -> i32 {
    match m {
        PotMode::Straight => 0,
        PotMode::Estimate => 1,
        PotMode::Measured => 2,
        PotMode::Hold => 3,
    }
}

fn verdict_no(v: PotVerdict) -> i32 {
    match v {
        PotVerdict::None => 0,
        PotVerdict::Yes => 1,
        PotVerdict::No => -1,
    }
}

/// number, mode, yes / no and circle sink of every tick against the mirror
fn compare_with_mirror(name: &str, csv: &str, expected: &str, gd: &GliderData) {
    let out = run(&rows(csv), gd, 1.5, 1.0);
    let mut max_diff = 0.0f32;
    let mut max_sink = 0.0f32;
    let mut n = 0;
    for (line, t) in expected.lines().skip(1).zip(out.iter()) {
        let e: Vec<&str> = line.split(',').collect();
        let (pot, mode, verdict, sink): (f32, i32, i32, f32) = (
            e[1].parse().unwrap(),
            e[2].parse().unwrap(),
            e[3].parse().unwrap(),
            e[4].parse().unwrap(),
        );
        let tick = e[0];
        let got = t
            .pot
            .unwrap_or_else(|| panic!("{name} tick {tick}: no value"));
        max_diff = max_diff.max((got - pot).abs());
        max_sink = max_sink.max((t.circle_sink - sink).abs());
        assert_eq!(mode_no(t.mode), mode, "{name} tick {tick}: mode");
        assert_eq!(
            verdict_no(t.verdict),
            verdict,
            "{name} tick {tick}: yes / no"
        );
        n += 1;
    }
    assert_eq!(
        n,
        out.len(),
        "{name}: fixture and expectation differ in length"
    );
    assert!(
        max_diff < 0.01,
        "{name}: max deviation from the mirror {max_diff}"
    );
    assert!(
        max_sink < 0.005,
        "{name}: circle sink deviates by {max_sink}"
    );
}

#[test]
fn as33_thermal_matches_the_python_mirror() {
    compare_with_mirror("AS33", AS33, AS33_EXPECTED, &glider("AS 33 18m", 592.5));
}

#[test]
fn ls3_weak_air_matches_the_python_mirror() {
    compare_with_mirror("LS3", LS3, LS3_EXPECTED, &glider("LS-3", 430.27));
}

/// first tick with this mode
fn first(out: &[Tick], mode: PotMode) -> usize {
    out.iter()
        .position(|t| t.mode == mode)
        .expect("mode never reached")
}

#[test]
fn straight_flight_shows_the_circle_value_without_yes_or_no() {
    // A1: in cruise the box always shows a number and never a yes / no
    let out = run(&rows(AS33), &glider("AS 33 18m", 592.5), 1.5, 1.0);
    let roll_in = first(&out, PotMode::Estimate);
    assert!(
        roll_in > 600,
        "circle value ended before the roll-in at tick 600: {roll_in}"
    );
    for t in &out[..roll_in] {
        assert_eq!(t.mode, PotMode::Straight);
        assert_eq!(t.verdict, PotVerdict::None);
        assert!(t.pot.is_some());
    }
    // the cruise before this thermal went through sinking air at 190 km/h:
    // the circle value is negative there
    let v = out[300].pot.unwrap();
    assert!((-3.0..0.0).contains(&v), "circle value in the sink {v}");
}

#[test]
fn the_estimate_starts_when_the_circle_stands_and_says_yes_in_a_good_thermal() {
    // A2 / A4: roll-in of the fixture at tick 600; the circle stands 2 to 4 s
    // later, the yes follows within 0.5 s (dwell 0.3 s) and stays
    let out = run(&rows(AS33), &glider("AS 33 18m", 592.5), 1.5, 1.0);
    let est = first(&out, PotMode::Estimate);
    assert!((615..=645).contains(&est), "estimate from tick {est}");
    let yes = out
        .iter()
        .position(|t| t.verdict == PotVerdict::Yes)
        .expect("no yes");
    assert!(
        yes >= est && yes <= est + 5,
        "yes at {yes}, estimate from {est}"
    );
    let hold = first(&out, PotMode::Hold);
    assert!(out[yes..hold].iter().all(|t| t.verdict == PotVerdict::Yes));
    // result of this thermal over the first three circles: 2.9 m/s. 8 s
    // after the roll-in the box is within 0.5 m/s of it
    let v8 = out[680].pot.unwrap();
    assert!((v8 - 2.9).abs() < 0.5, "8 s after the roll-in: {v8}");
}

#[test]
fn after_one_circle_the_box_shows_the_measured_climb() {
    // A3: from 400 deg the number is the altitude gained over the last circle
    let rows = rows(AS33);
    let out = run(&rows, &glider("AS 33 18m", 592.5), 1.5, 1.0);
    let measured = first(&out, PotMode::Measured);
    let hold = first(&out, PotMode::Hold);
    assert!(hold - measured > 600, "less than 60 s measured");
    // against the altitude gained over exactly the last 360 deg of turn
    // (the box takes the newest 1 Hz entry at least one circle back, so its
    // window is up to 1 s longer)
    let mut rot = Vec::with_capacity(rows.len());
    let mut sum = 0.0f32;
    for r in &rows {
        sum += r.tr * 0.1;
        rot.push(sum);
    }
    let mut worst = 0.0f32;
    for i in (measured + 300..hold).step_by(25) {
        let j = (0..i)
            .rev()
            .find(|&j| (rot[i] - rot[j]).abs() >= 2.0 * core::f32::consts::PI)
            .unwrap();
        let climb = (rows[i].alt - rows[j].alt) / ((i - j) as f32 * 0.1);
        worst = worst.max((out[i].pot.unwrap() - climb).abs());
    }
    assert!(worst < 0.25, "measured climb off by {worst}");
}

#[test]
fn the_last_value_is_held_for_ten_seconds_after_the_exit() {
    // A5: no yes / no while the box holds, then back to the circle value
    let out = run(&rows(AS33), &glider("AS 33 18m", 592.5), 1.5, 1.0);
    let hold = first(&out, PotMode::Hold);
    let end = hold
        + out[hold..]
            .iter()
            .position(|t| t.mode != PotMode::Hold)
            .expect("hold never ends");
    assert_eq!(end - hold, 100);
    let frozen = out[hold].pot.unwrap();
    for t in &out[hold..end] {
        assert_eq!(t.pot.unwrap(), frozen);
        assert_eq!(t.verdict, PotVerdict::None);
    }
    assert_eq!(out[end].mode, PotMode::Straight);
}

#[test]
fn weak_air_gets_a_no_and_never_a_yes() {
    // A4: LS3, three times circling in 0.3 m/s and less - with a bar of 1.5
    // every circle gets its no within 1 s after it stands, never a yes
    let out = run(&rows(LS3), &glider("LS-3", 430.27), 1.5, 1.0);
    assert!(out.iter().all(|t| t.verdict != PotVerdict::Yes));
    let mut circles = 0;
    let mut i = 0;
    while i < out.len() {
        if out[i].mode == PotMode::Estimate && (i == 0 || out[i - 1].mode == PotMode::Straight) {
            circles += 1;
            let no = out[i..]
                .iter()
                .position(|t| t.verdict == PotVerdict::No)
                .expect("no 'no'");
            assert!(no <= 10, "circle from tick {i}: no only after {no} ticks");
        }
        i += 1;
    }
    assert_eq!(circles, 4);
    // no yes / no in straight flight (at most the 1 s it takes to clear)
    let mut straight_with = 0;
    for t in &out {
        if t.mode == PotMode::Straight && t.verdict != PotVerdict::None {
            straight_with += 1;
            assert!(straight_with <= 10);
        } else if t.mode != PotMode::Straight {
            straight_with = 0;
        }
    }
}

#[test]
fn the_bar_is_the_maccready_value_times_the_factor() {
    // A4: the same thermal (2.9 m/s) against other bars. MC 4: the estimate
    // of 2.4 to 3.4 is below the bar but not below bar - 0.7 all the time;
    // MC 5: a plain no. MC 4 with factor 0.5 is a bar of 2: yes again.
    let rows = rows(AS33);
    let gd = glider("AS 33 18m", 592.5);
    let at_8s = |mc: f32, factor: f32| run(&rows, &gd, mc, factor)[680].verdict;
    assert_eq!(at_8s(1.5, 1.0), PotVerdict::Yes);
    assert_eq!(at_8s(2.0, 1.0), PotVerdict::Yes);
    assert_eq!(at_8s(5.0, 1.0), PotVerdict::No);
    assert_eq!(at_8s(4.0, 0.5), PotVerdict::Yes);
    assert_eq!(at_8s(1.5, 1.5), PotVerdict::Yes);
    // MC 0 is a bar of 0.5 m/s, not "yes to everything"
    let weak = run(&self::rows(LS3), &glider("LS-3", 430.27), 0.0, 1.0);
    assert!(weak.iter().any(|t| t.verdict == PotVerdict::No));
}

#[test]
fn the_number_does_not_depend_on_the_maccready_value() {
    // A6: MC only moves the bar
    let rows = rows(AS33);
    let gd = glider("AS 33 18m", 592.5);
    let a = run(&rows, &gd, 0.5, 1.0);
    let b = run(&rows, &gd, 4.0, 1.3);
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.pot, y.pot);
        assert_eq!(x.mode, y.mode);
    }
}

/// synthetic flight: segments of (ticks, bank [rad], turn rate [rad/s],
/// climb [m/s]) at a constant 30 m/s
fn synthetic(seg: &[(usize, f32, f32, f32)]) -> Vec<Row> {
    let mut alt = 800.0f32;
    let mut out = Vec::new();
    for &(n, roll, tr, climb) in seg {
        for _ in 0..n {
            alt += climb * 0.1;
            // TE vario of a glider at 30 m/s: the air minus about 0.7 m/s own sink
            out.push(Row {
                roll,
                tr,
                vario: climb,
                alt,
                ias: 30.0,
                tas: 30.0,
                g_force: G,
            });
        }
    }
    out
}

#[test]
fn recentering_continues_the_thermal() {
    // A5: 20 s straight, 60 s circling in 2 m/s, 6 s levelled (re-centring),
    // 30 s circling again. The box must neither fall back to the straight
    // value nor start a new estimate.
    let rows = synthetic(&[
        (200, 0.0, 0.0, 0.0),
        (600, 0.7, 0.26, 2.0),
        (60, 0.0, 0.0, 2.0),
        (300, 0.7, 0.26, 2.0),
    ]);
    let out = run(&rows, &glider("AS 33 18m", 500.0), 1.5, 1.0);
    let measured = first(&out, PotMode::Measured);
    assert!(measured < 200 + 320, "measured only from tick {measured}");
    for (i, t) in out.iter().enumerate().skip(measured) {
        assert!(
            t.mode == PotMode::Measured || t.mode == PotMode::Hold,
            "tick {i}: {:?}",
            t.mode
        );
        let v = t.pot.unwrap();
        assert!((v - 2.0).abs() < 0.15, "tick {i}: {v}");
    }
    assert_eq!(out.last().unwrap().mode, PotMode::Measured);
}

#[test]
fn a_pull_up_is_not_counted_as_climb() {
    // A2: 20 s straight at 40 m/s, then rolling in with an energy neutral
    // pull-up: the TAS falls from 40 to 25 m/s in 10 s, the altitude rises
    // by exactly the kinetic height lost (49.7 m), the TE vario shows 0.
    // The plain altitude rate would be 5 m/s. The energy balance stays near
    // the anchor share (0.95 m/s times its weight): below 0.7 m/s at 8 s,
    // below 0.3 m/s after two circles.
    let mut rows = Vec::new();
    let v0 = 40.0f32;
    for i in 0..(200 + 700) {
        let (roll, tr) = if i < 200 { (0.0, 0.0) } else { (0.7, 0.26) };
        let t_circ = (i as f32 - 200.0) * 0.1;
        let tas = if i < 200 {
            v0
        } else if t_circ < 10.0 {
            v0 - 1.5 * t_circ
        } else {
            25.0
        };
        let alt = 500.0 + (v0 * v0 - tas * tas) / (2.0 * G);
        rows.push(Row {
            roll,
            tr,
            vario: 0.0,
            alt,
            ias: tas,
            tas,
            g_force: G,
        });
    }
    let out = run(&rows, &glider("AS 33 18m", 500.0), 1.5, 1.0);
    let est = first(&out, PotMode::Estimate);
    assert!(est < 240, "estimate from tick {est}");
    for (i, t) in out.iter().enumerate().skip(est + 20) {
        let v = t.pot.unwrap();
        assert!(v.abs() < 0.9, "tick {i}: {v}");
    }
    let v8 = out[280].pot.unwrap();
    assert!(v8.abs() < 0.7, "8 s after the roll-in {v8}");
    let v_end = out.last().unwrap().pot.unwrap();
    assert!(v_end.abs() < 0.3, "after two circles {v_end}");
    assert!(out.iter().all(|t| t.verdict != PotVerdict::Yes));
}

#[test]
fn a_turn_without_circling_goes_back_to_the_circle_value() {
    // A5: a course change of 60 deg (4 s at 15 deg/s) is no thermal: no hold
    // with a frozen value, the box is back on the straight value 3 s later
    let rows = synthetic(&[
        (200, 0.0, 0.0, 0.0),
        (40, 0.5, 0.26, 0.0),
        (300, 0.0, 0.0, 0.0),
    ]);
    let out = run(&rows, &glider("AS 33 18m", 500.0), 1.5, 1.0);
    assert!(out
        .iter()
        .all(|t| t.mode != PotMode::Hold && t.mode != PotMode::Measured));
    let last_est = out
        .iter()
        .rposition(|t| t.mode == PotMode::Estimate)
        .expect("no estimate at all");
    assert!(last_est < 200 + 40 + 60, "estimate until tick {last_est}");
    let straight = out[200 - 1].pot.unwrap();
    let v = out[last_est + 30].pot.unwrap();
    assert!(
        (v - straight).abs() < 0.05,
        "3 s after the turn {v}, cruise value {straight}"
    );
}

#[test]
fn on_the_ground_there_is_no_value_and_the_start_is_clean() {
    // A7: below 10 m/s TAS the box gives nothing (the display shows the
    // average then); after the take-off it needs 6 s before a roll-in counts
    let mut rows = synthetic(&[
        (100, 0.0, 0.0, 0.0),
        (30, 0.7, 0.26, 3.0),
        (400, 0.7, 0.26, 3.0),
    ]);
    for r in rows.iter_mut().take(100) {
        r.tas = 5.0;
        r.ias = 5.0;
    }
    let out = run(&rows, &glider("AS 33 18m", 500.0), 1.5, 1.0);
    assert!(out[..100]
        .iter()
        .all(|t| t.pot.is_none() && t.verdict == PotVerdict::None));
    assert!(out[100..160].iter().all(|t| t.mode == PotMode::Straight));
    assert!(out[100..].iter().all(|t| t.pot.is_some()));
    // not a number in, nothing out, and no panic
    let mut bad = synthetic(&[(300, 0.0, 0.0, 0.0)]);
    bad[150].alt = f32::NAN;
    bad[200].g_force = f32::INFINITY;
    let out = run(&bad, &glider("AS 33 18m", 500.0), 1.5, 1.0);
    assert!(out[150].pot.is_none() && out[200].pot.is_none());
    assert!(out[149].pot.unwrap().is_finite() && out[151].pot.unwrap().is_finite());
}

#[test]
fn after_a_stop_the_box_starts_like_a_new_one() {
    // A7: below 10 m/s TAS the box is reset - whatever it was doing. What
    // follows must be the same, bit by bit, as on a box that never ran: no
    // filter, buffer or counter may survive. (What it learned about the own
    // circles stays; here no thermal was finished before the stop.)
    let ls3 = (rows(LS3), glider("LS-3", 430.27));
    let as33 = (rows(AS33), glider("AS 33 18m", 592.5));
    // 0.3 s after a circle was given up: the hand-over back to the straight
    // value is running
    let m = run(&ls3.0, &ls3.1, 1.5, 1.0);
    let given_up = (1..m.len())
        .find(|&i| m[i - 1].mode == PotMode::Estimate && m[i].mode == PotMode::Straight)
        .expect("no circle given up in the LS3 fixture")
        + 3;
    // stop in cruise, in the estimate of the first circle, in the measured
    // climb (a thermal half learned), right after giving up
    let stops: [(&[Row], usize, PotMode); 4] = [
        (&as33.0, 300, PotMode::Straight),
        (&as33.0, 700, PotMode::Estimate),
        (&as33.0, 990, PotMode::Measured),
        (&ls3.0, given_up, PotMode::Straight),
    ];
    for (name, (flight, gd)) in [("LS3", &ls3), ("AS33", &as33)] {
        let fresh = run(flight, gd, 1.5, 1.0);
        for (before, cut, mode) in stops {
            let mut all: Vec<Row> = before[..cut].to_vec();
            let mut ground = before[cut];
            ground.tas = 5.0;
            ground.ias = 5.0;
            // at least 1 s, and the flight starts on a full second: the
            // polar of `run` keeps its rhythm
            let pad = 10 + (10 - cut % 10) % 10;
            all.extend(std::iter::repeat(ground).take(pad));
            all.extend(flight.iter().copied());
            let out = run(&all, gd, 1.5, 1.0);
            let at = format!("{name} after a stop at {cut}");
            assert_eq!(
                out[cut - 1].mode,
                mode,
                "{at}: the box was doing something else"
            );
            assert!(
                out[cut..cut + pad].iter().all(|t| t.pot.is_none()
                    && t.mode == PotMode::Straight
                    && t.verdict == PotVerdict::None),
                "{at}: not silent while stopped"
            );
            for (i, (x, y)) in fresh.iter().zip(&out[cut + pad..]).enumerate() {
                assert_eq!(
                    x.pot.map(f32::to_bits),
                    y.pot.map(f32::to_bits),
                    "{at}, tick {i}: {:?} / {:?}",
                    x.pot,
                    y.pot
                );
                assert_eq!(x.mode, y.mode, "{at}, tick {i}: mode");
                assert_eq!(x.verdict, y.verdict, "{at}, tick {i}: yes / no");
                assert_eq!(
                    x.circle_sink.to_bits(),
                    y.circle_sink.to_bits(),
                    "{at}, tick {i}: circle sink {} / {}",
                    x.circle_sink,
                    y.circle_sink
                );
            }
        }
    }
}

#[test]
fn a_broken_polar_gives_no_value_and_leaves_no_trace() {
    // A7, found with the simulator: it runs its first second without a
    // polar, and the box showed "NaN" from then on - one such value in a
    // filter stays there for the rest of the flight. An air density of 0
    // from the sensor box does the same to the polar (exact arithmetic: not
    // a number; square root of the device: 1e19). No value then - the
    // display shows the average climb - and afterwards the box is like new.
    let gd = glider("LS-3", 430.27);
    let flight = rows(LS3);
    let fresh = run(&flight, &gd, 1.5, 1.0);
    let before = rows(AS33);
    for density in [0.0, f32::NAN] {
        let mut polar = Polar::default();
        polar.recalc_glider(&gd);
        let mut pot = PotClimb::new();
        // into the measured climb of a thermal: every filter and buffer in use
        let ok = run_with(&mut pot, &mut polar, &before[..990], &gd, 1.5, 1.0);
        assert_eq!(ok[989].mode, PotMode::Measured);
        // one second with the broken polar
        polar.recalc(&gd, Density::from_kg_m3(density));
        for r in &before[990..1000] {
            let v = pot.tick_raw(&input(r, &gd, 1.5, 1.0), &polar);
            assert!(v.is_none(), "density {density}: the box shows {v:?}");
            assert_eq!(pot.mode(), PotMode::Straight);
            assert_eq!(pot.verdict(), PotVerdict::None);
        }
        // the polar is back: the same as on a box that never ran
        let out = run_with(&mut pot, &mut polar, &flight, &gd, 1.5, 1.0);
        for (i, (x, y)) in fresh.iter().zip(&out).enumerate() {
            assert_eq!(
                x.pot.map(f32::to_bits),
                y.pot.map(f32::to_bits),
                "density {density}, tick {i}: {:?} / {:?}",
                x.pot,
                y.pot
            );
            assert_eq!(x.mode, y.mode, "density {density}, tick {i}: mode");
            assert_eq!(
                x.verdict, y.verdict,
                "density {density}, tick {i}: yes / no"
            );
        }
    }
}

#[test]
fn a_wild_input_is_no_flight_and_gone_a_moment_later() {
    // A7: one value a glider cannot have - not a number, or a number far
    // out - must not get into a filter: NaN would stay for good, 1e30 would
    // show as 9.9 m/s for a minute. The tick gives nothing, the box starts
    // again and is back on the undisturbed values within 5 s.
    let gd = glider("AS 33 18m", 500.0);
    let calm = synthetic(&[(300, 0.0, 0.0, 0.4), (300, 0.0, 0.0, -1.2)]);
    let good = run(&calm, &gd, 1.5, 1.0);
    let pokes: [(&str, fn(&mut Row, f32)); 7] = [
        ("bank", |r, x| r.roll = x),
        ("turn rate", |r, x| r.tr = x),
        ("vario", |r, x| r.vario = x),
        ("altitude", |r, x| r.alt = x),
        ("ias", |r, x| r.ias = x),
        ("tas", |r, x| r.tas = x),
        ("g load", |r, x| r.g_force = x),
    ];
    for (what, poke) in pokes {
        for x in [f32::NAN, f32::INFINITY, -1.0e30, 1.0e30] {
            let mut rows = calm.clone();
            // (not on a full second: there the test takes the air density from the row)
            poke(&mut rows[205], x);
            let out = run(&rows, &gd, 1.5, 1.0);
            assert!(out[205].pot.is_none(), "{what} = {x}: {:?}", out[205].pot);
            assert_eq!(out[205].verdict, PotVerdict::None);
            for (i, (a, b)) in good.iter().zip(&out).enumerate() {
                if i == 205 {
                    continue;
                }
                let (a, b) = (a.pot.unwrap(), b.pot.expect("no value after the wild one"));
                assert!(
                    b.is_finite() && b.abs() <= 9.9,
                    "{what} = {x}, tick {i}: {b}"
                );
                if !(205..255).contains(&i) {
                    assert!(
                        (a - b).abs() < 0.02,
                        "{what} = {x}, tick {i}: {b}, undisturbed {a}"
                    );
                }
            }
        }
    }
    // in a circle: whatever comes in, never a value that is not a number
    let mut wild = synthetic(&[(100, 0.0, 0.0, 0.0), (500, 0.7, 0.26, 3.0)]);
    wild[300].alt = 1.0e30;
    wild[350].vario = -1.0e30;
    wild[400].tas = 3.0e19;
    wild[450].g_force = f32::NAN;
    let out = run(&wild, &gd, 1.5, 1.0);
    assert!(out
        .iter()
        .all(|t| t.pot.map_or(true, |v| v.is_finite() && v.abs() <= 9.9)));
    for i in [300, 350, 400, 450] {
        assert!(out[i].pot.is_none(), "tick {i}: {:?}", out[i].pot);
    }
    assert!(out[599].pot.is_some());
}

#[test]
fn every_glider_of_the_store_gets_a_usable_circle_sink() {
    // A8: before the first thermal the box assumes a circle at 1.4 g and a
    // lift coefficient of 1.3 (four entries of the store have no wing area:
    // 0.83 times best glide). For all 170 polars that is between 0.5 m/s
    // (ASW 22) and 1.8 m/s (Dimona) - a broken entry would show here.
    for p in POLARS.iter() {
        let gd = GliderData::from_basic_glider_data(*p);
        let mut polar = Polar::default();
        polar.recalc_glider(&gd);
        polar.recalc(&gd, Density::AT_NN());
        let sink = PotClimb::new().circle_sink(&polar, p.wing_area, 1.0);
        assert!((0.4..2.0).contains(&sink), "{}: circle sink {sink}", p.name);
        // and the polar with a load factor stays a number in the whole range the box uses
        for n in [0.2, 1.0, 3.0] {
            for v in [15.0, 30.0, 70.0] {
                let s = polar.sink_rate_g(AirSpeed::new(v, v), n).to_m_s();
                assert!(s.is_finite(), "{}: sink {s} at {v} m/s, {n} g", p.name);
            }
        }
    }
}

#[test]
fn the_sink_in_the_own_circle_is_learned_from_the_circles_flown() {
    // A8: default load factor 1.4. A pilot who circles flat (bank 30 deg,
    // n = 1.15) at 27 m/s gets a lower circle sink after his first thermal.
    let gd = glider("AS 33 18m", 500.0);
    let mut rows = synthetic(&[
        (200, 0.0, 0.0, 0.0),
        (900, 0.52, 0.2, 1.5),
        (300, 0.0, 0.0, 0.0),
    ]);
    for r in rows.iter_mut().skip(200).take(900) {
        r.g_force = 1.15 * G;
        r.ias = 27.0;
        r.tas = 27.0;
    }
    let out = run(&rows, &gd, 1.5, 1.0);
    let before = out[100].circle_sink;
    let after = out.last().unwrap().circle_sink;
    assert!(
        (0.85..1.25).contains(&before),
        "default circle sink {before}"
    );
    assert!(after < before - 0.15, "circle sink {before} -> {after}");
    assert!(after > 0.5, "circle sink {after}");
}
