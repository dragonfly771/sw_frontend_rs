//! Fixture test of the entry optimized vario needle (controller/needle_entry.rs)
//!
//! Fixture: one thermal of a 10 Hz flight log with 15 s of straight
//! flight before and after. Columns: roll_angle [rad], turn_rate [rad/s],
//! vario [m/s], position_up [m], pitch_angle [rad], tas [m/s],
//! pressure_altitude [m]. Default k = -0.18 (cold start: aoa_stat begins at
//! 0, there is no previous thermal).
//!
//! These are the needle tests of the former tests/pot_climb.rs with
//! unchanged limits: the needle correction did not change when the POT box
//! got its own calculation (0.4.1.48).

use corelib::NeedleEntry;

const FIXTURE: &str = include_str!("fixtures/needle_entry_thermal.csv");

fn run() -> Vec<f32> {
    let mut needle = NeedleEntry::new();
    let mut corr = Vec::new();
    for line in FIXTURE.lines().skip(1) {
        let mut it = line.split(',');
        let roll: f32 = it.next().unwrap().parse().unwrap();
        let tr: f32 = it.next().unwrap().parse().unwrap();
        let _vario: f32 = it.next().unwrap().parse().unwrap();
        let _alt: f32 = it.next().unwrap().parse().unwrap();
        let pitch: f32 = it.next().unwrap().parse().unwrap();
        let tas: f32 = it.next().unwrap().parse().unwrap();
        let baro_alt: f32 = it.next().unwrap().parse().unwrap();
        needle.tick_raw(roll, tr, pitch, tas, baro_alt);
        corr.push(needle.needle_correction(-0.18));
    }
    corr
}

#[test]
fn needle_correction_is_phase_controlled() {
    let corr = run();
    // straight flight before the roll-in: no correction
    assert!(corr[..100].iter().all(|c| *c == 0.0));
    // entry: pure AoA correction (Python reference: max ~1.0 m/s)
    let max_entry = corr[150..300].iter().fold(0.0f32, |m, c| m.max(c.abs()));
    assert!(
        (0.90..=1.10).contains(&max_entry),
        "entry correction max {max_entry}, expected ~1.0"
    );
    // plausibility limit: AoA limit +-16 deg * 0.18
    assert!(corr.iter().all(|c| c.abs() <= 16.0 * 0.18 + 1e-4));
    // after the gate (straight flight at the end): exactly no correction
    assert!(corr[corr.len() - 50..].iter().all(|c| *c == 0.0));
}

#[test]
fn recentering_keeps_the_needle_continuous() {
    // synthetic: 10 s straight, 40 s circling, 6 s levelled (re-centring),
    // 30 s circling again. The needle correction must not jump.
    let mut needle = NeedleEntry::new();
    let mut corr = Vec::new();
    let seg: [(usize, f32, f32); 4] = [
        (100, 0.0, 0.0), // (ticks, bank [rad], turn rate [rad/s])
        (400, 0.7, 0.26),
        (60, 0.0, 0.0),
        (300, 0.7, 0.26),
    ];
    let mut alt = 500.0f32;
    for (n, roll, tr) in seg {
        for _ in 0..n {
            alt += 0.2; // 2 m/s climb
            needle.tick_raw(roll, tr, 0.05, 25.0, alt);
            corr.push(needle.needle_correction(-0.18));
        }
    }
    // no jumps: max |delta| per tick = |k| * 0.8 deg
    let max_step = corr
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(max_step <= 0.18 * 0.8 + 1e-4, "corr step {max_step}");
}
