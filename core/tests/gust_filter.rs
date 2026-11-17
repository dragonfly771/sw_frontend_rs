//! Test of the gust filter (PT2 cascade with IAS scaling + soft knee)
//!
//! Expected values from a Python reference: a synthetic sequence of a 1 s
//! triangle peak to 3 m/s (a typical short gust) and a 2 m/s step (a real
//! thermal entry), IAS 130 km/h = 36.1 m/s, straight flight (roll 0) or
//! circling (roll 40 deg).

use corelib::{GustFilter, GustPt2};

const IAS_REF: f32 = 36.1; // 130 km/h

fn sequence() -> Vec<f32> {
    let mut seq = vec![0.0; 50];
    for i in 1..=5 {
        seq.push(3.0 * i as f32 / 5.0); // Flanke hoch
    }
    for i in 1..=5 {
        seq.push(3.0 * (5 - i) as f32 / 5.0); // Flanke runter
    }
    seq.extend(vec![0.0; 45]);
    seq.extend(vec![2.0; 50]); // Sprung ab Tick 105
    seq
}

fn run(setting: GustFilter, ias: f32, roll: f32) -> Vec<f32> {
    run_opt(setting, ias, roll, false)
}

fn run_opt(setting: GustFilter, ias: f32, roll: f32, knee_circling: bool) -> Vec<f32> {
    let mut f = GustPt2::new();
    sequence()
        .iter()
        .map(|x| f.tick_raw(*x, ias, roll, setting, knee_circling))
        .collect()
}

#[test]
fn off_is_exact_passthrough() {
    let seq = sequence();
    let out = run(GustFilter::Off, IAS_REF, 0.0);
    assert_eq!(seq, out);
}

#[test]
fn spike_peaks_are_reduced_per_stage() {
    // Python reference: peak of the 3 m/s triangle (straight flight, knee active)
    for (setting, expect) in [
        (GustFilter::PeakCut, 1.74),
        (GustFilter::Weak, 1.24),
        (GustFilter::Medium, 1.13),
        (GustFilter::Strong, 0.94),
    ] {
        let out = run(setting, IAS_REF, 0.0);
        let peak = out[50..70].iter().fold(0.0f32, |m, v| m.max(*v));
        assert!(
            (peak - expect).abs() < 0.02,
            "{setting:?}: peak {peak}, expected ~{expect}"
        );
    }
}

#[test]
fn sustained_step_passes_with_bounded_delay() {
    // Python reference: 90 % of the 2 m/s step (measured from tick 110)
    for (setting, expect_ticks) in [
        (GustFilter::PeakCut, 6),
        (GustFilter::Weak, 12),
        (GustFilter::Medium, 14),
        (GustFilter::Strong, 18),
    ] {
        let out = run(setting, IAS_REF, 0.0);
        let t90 = out[110..].iter().position(|v| *v >= 1.8).unwrap();
        assert!(
            (t90 as i32 - expect_ticks).abs() <= 1,
            "{setting:?}: t90 {t90} ticks, expected ~{expect_ticks}"
        );
        // and the final value arrives in full
        assert!((out.last().unwrap() - 2.0).abs() < 0.01);
    }
}

#[test]
fn peak_cut_responds_instantly() {
    // Peak Cut: 0.5 s after the step about 77 % are there (1.55 of 2.0)
    let out = run(GustFilter::PeakCut, IAS_REF, 0.0);
    assert!(
        (out[110] - 1.55).abs() < 0.02,
        "peak cut step response {}, expected ~1.55",
        out[110]
    );
}

#[test]
fn knee_is_off_while_circling() {
    // circling (roll 40 deg): knee off -> the stages are pure PT2,
    // Peak Cut becomes pass-through (the thermal texture stays untouched)
    let med = run(GustFilter::Medium, IAS_REF, 0.7);
    let peak_med = med[50..70].iter().fold(0.0f32, |m, v| m.max(*v));
    assert!(
        (peak_med - 1.23).abs() < 0.02,
        "medium peak while circling {peak_med}, expected pure PT2 ~1.23"
    );
    let pc = run(GustFilter::PeakCut, IAS_REF, 0.7);
    let peak_pc = pc[50..70].iter().fold(0.0f32, |m, v| m.max(*v));
    assert!(
        (peak_pc - 3.0).abs() < 0.01,
        "peak cut while circling {peak_pc}, expected raw 3.0"
    );
}

#[test]
fn gust_circling_option_keeps_knee_active() {
    // option "Gust Circling" On: the knee works at 40 deg bank too ->
    // identical to the straight flight result
    for (setting, expect) in [(GustFilter::Medium, 1.13), (GustFilter::PeakCut, 1.74)] {
        let out = run_opt(setting, IAS_REF, 0.7, true);
        let peak = out[50..70].iter().fold(0.0f32, |m, v| m.max(*v));
        assert!(
            (peak - expect).abs() < 0.02,
            "{setting:?} circling+option: peak {peak}, expected ~{expect}"
        );
    }
}

#[test]
fn filter_scales_with_airspeed() {
    // Python reference (Medium): peak 1.21 at 100 km/h, 1.02 at 200 km/h
    let slow = run(GustFilter::Medium, 27.8, 0.0);
    let fast = run(GustFilter::Medium, 55.6, 0.0);
    let peak_slow = slow[50..70].iter().fold(0.0f32, |m, v| m.max(*v));
    let peak_fast = fast[50..70].iter().fold(0.0f32, |m, v| m.max(*v));
    assert!((peak_slow - 1.21).abs() < 0.02, "slow peak {peak_slow}");
    assert!((peak_fast - 1.02).abs() < 0.02, "fast peak {peak_fast}");
    assert!(peak_fast < peak_slow);
}
