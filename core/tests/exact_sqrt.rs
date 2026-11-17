//! The square roots of the firmware as the device computes them (plan_wurzel_genau.md,
//! 0.4.1.49).
//!
//! The core is no_std. An integration test links it like the device does, so `x.sqrt()` in the
//! library is the approximation of micromath here (exact at powers of 4, up to 6 % too large in
//! between); the unit tests have std and with it the exact root - they never saw the
//! difference. Up to 0.4.1.48 the speed to fly was up to 17 km/h too high because of it.

use corelib::{
    math, polar_store::POLARS, AirSpeed, Angle, Density, FloatToAcceleration, FloatToMass,
    FloatToSpeed, GliderData, Polar, WindVector,
};

const RHO0: f64 = 1.225;

/// ISA density at an altitude
fn rho(h: f64) -> f64 {
    RHO0 * (1.0 - 2.25577e-5 * h).powf(4.2559)
}

/// coefficients of the store polar (the same three point fit as Polar::recalc_glider)
fn fit(p: &corelib::polar_store::BasicGliderData) -> (f64, f64, f64, f64) {
    let pv = p.polar_values;
    let (v1, w1) = (pv[0][0] as f64 / 3.6, pv[0][1] as f64);
    let (v2, w2) = (pv[1][0] as f64 / 3.6, pv[1][1] as f64);
    let (v3, w3) = (pv[2][0] as f64 / 3.6, pv[2][1] as f64);
    let a = ((v2 - v3) * (w1 - w3) + (v3 - v1) * (w2 - w3))
        / (v1 * v1 * (v2 - v3) + v2 * v2 * (v3 - v1) + v3 * v3 * (v1 - v2));
    let b = (w2 - w3 - a * (v2 * v2 - v3 * v3)) / (v2 - v3);
    let c = w3 - a * v3 * v3 - b * v3;
    (a, b, c, -b / a / 2.0)
}

/// the polar of the firmware for a glider, weight and altitude
fn polar(p: &corelib::polar_store::BasicGliderData, w: f64, h: f64) -> Polar {
    let mut gd = GliderData::from_basic_glider_data(*p);
    gd.pilot_weight = 90.0.kg();
    gd.water_ballast = ((w - p.empty_mass as f64 - 90.0) as f32).kg();
    let mut polar = Polar::default();
    polar.recalc_glider(&gd);
    polar.recalc(&gd, Density::from_kg_m3(rho(h) as f32));
    polar
}

/// every glider of the store at three weights, four altitudes
fn cases() -> Vec<(&'static corelib::polar_store::BasicGliderData, f64, f64)> {
    let mut out = Vec::new();
    for p in POLARS
        .iter()
        .filter(|p| p.wing_area > 1.0 && p.reference_weight > 100.0)
    {
        let dry = p.empty_mass as f64 + 90.0;
        for w in [
            dry,
            dry + 0.5 * p.max_ballast as f64,
            dry + p.max_ballast as f64,
        ] {
            for h in [0.0, 1000.0, 2000.0, 3000.0] {
                out.push((p, w, h));
            }
        }
    }
    out
}

#[test]
fn the_root_is_exact_with_the_arithmetic_of_the_device() {
    // the estimate of micromath at its worst point: 1.5 instead of 1.414
    assert!((micromath::F32Ext::sqrt(2.0f32) - 1.5).abs() < 1e-6);
    let mut worst = 0.0f64;
    let mut x = 1.0e-20f32;
    while x < 1.0e20 {
        let want = (x as f64).sqrt();
        worst = worst.max(((math::sqrt(x) as f64 - want) / want).abs());
        x *= 1.013;
    }
    assert!(worst < 2.5e-7, "relative error {worst}");
}

#[test]
fn the_polar_follows_weight_and_density_exactly() {
    for (p, w, h) in cases() {
        let (a0, b0, c0, vmin0) = fit(p);
        let r = (w / p.reference_weight as f64).sqrt() * (RHO0 / rho(h)).sqrt();
        let pol = polar(p, w, h);
        for v in [25.0f64, 40.0, 55.0] {
            // the polar holds its value below the speed of minimum sink and above VNE
            let vc = v.max(vmin0 * r).min(p.max_speed as f64 / 3.6);
            let want = a0 / r * vc * vc + b0 * vc + c0 * r;
            let got = pol.sink_rate(AirSpeed::new(v as f32, v as f32)).to_m_s() as f64;
            assert!(
                (got - want).abs() < 0.002,
                "{} {w:.0} kg {h:.0} m, {v} m/s: sink {got:.4}, exact {want:.4}",
                p.name
            );
        }
    }
}

#[test]
fn the_speed_to_fly_is_the_maccready_speed_of_the_polar() {
    // up to 0.4.1.48: median +3 km/h, 90 % below +8, at most +17 (never too low)
    let mut worst = 0.0f64;
    for (p, w, h) in cases() {
        let (a0, _, c0, vmin0) = fit(p);
        let dr = (RHO0 / rho(h)).sqrt();
        let r = (w / p.reference_weight as f64).sqrt() * dr;
        let pol = polar(p, w, h);
        for netto in [-2.0f64, -1.0, 0.0, 1.0] {
            for mc in [0.0f64, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0] {
                let val = (c0 * r + netto - mc) / (a0 / r);
                let tas = if val > 0.0 { val.sqrt() } else { 0.0 };
                let tas = tas.min(p.max_speed as f64 / 3.6).max(vmin0 * r);
                let want = tas / dr * 3.6;
                let got = pol
                    .speed_to_fly((netto as f32).m_s(), (mc as f32).m_s())
                    .ias()
                    .to_km_h() as f64;
                worst = worst.max((got - want).abs());
                assert!(
                    (got - want).abs() < 0.05,
                    "{} {w:.0} kg {h:.0} m, netto {netto}, MC {mc}: {got:.2} km/h, exact {want:.2}",
                    p.name
                );
            }
        }
    }
    println!("largest deviation {worst:.4} km/h");
}

#[test]
fn eas_takes_the_exact_root() {
    // EAS = IAS / sqrt(n): at 2 g the estimate was 6 % off
    let eas = AirSpeed::new(30.0, 33.0)
        .eas((2.0 * 9.80665).m_s2())
        .unwrap()
        .to_m_s();
    assert!((eas - 30.0 / 2.0f32.powf(0.5)).abs() < 1e-4, "EAS {eas}");
}

/// the difference of two winds from their components: "from" directions, the result points
/// where the difference blows from - the convention of WindVector::sub (see its unit test)
fn wind_difference(s1: f64, a1: f64, s2: f64, a2: f64) -> (f64, f64) {
    let (x, y) = (
        s2 * a2.to_radians().sin() - s1 * a1.to_radians().sin(),
        s2 * a2.to_radians().cos() - s1 * a1.to_radians().cos(),
    );
    (
        (x * x + y * y).sqrt(),
        x.atan2(y).to_degrees().rem_euclid(360.0),
    )
}

#[test]
fn the_wind_difference_is_exact_also_for_parallel_winds() {
    // parallel winds of different strength: up to 0.4.1.48 the arc cosine gave NaN in
    // some of them and the difference was 0
    let mut zeros = 0;
    for deg in (0..360).step_by(5) {
        let d = deg as f32;
        let v = WindVector::new(10.0.m_s(), Angle::from_degrees(d))
            - WindVector::new(6.0.m_s(), Angle::from_degrees(d));
        if v.speed().to_m_s() < 1.0 {
            zeros += 1;
        }
        assert!(
            (v.speed().to_m_s() - 4.0).abs() < 0.01,
            "{deg} deg: {} m/s",
            v.speed().to_m_s()
        );
    }
    assert_eq!(zeros, 0);
    // a grid of winds against the components. The cosine of micromath stays (0.002): its
    // effect on the length is a * b * 0.002 / length
    for s1 in [3.0f64, 8.0, 15.0] {
        for s2 in [2.0f64, 6.0, 12.0] {
            for a1 in (0..360).step_by(30) {
                for a2 in (0..360).step_by(20) {
                    let (want_s, want_a) = wind_difference(s1, a1 as f64, s2, a2 as f64);
                    let v = WindVector::new((s1 as f32).m_s(), Angle::from_degrees(a1 as f32))
                        - WindVector::new((s2 as f32).m_s(), Angle::from_degrees(a2 as f32));
                    let got_s = v.speed().to_m_s() as f64;
                    let tol = 0.01 + 0.0025 * s1 * s2 / want_s.max(0.5);
                    assert!(
                        (got_s - want_s).abs() < tol,
                        "{s1} @ {a1} - {s2} @ {a2}: {got_s:.3} m/s, exact {want_s:.3}"
                    );
                    if want_s > 2.0 {
                        let da = (v.angle().to_degrees() as f64 - want_a + 540.0).rem_euclid(360.0)
                            - 180.0;
                        assert!(
                            da.abs() < 1.5,
                            "{s1} @ {a1} - {s2} @ {a2}: angle off by {da:.2} deg"
                        );
                    }
                }
            }
        }
    }
}

/// Where the library may keep the fast root: places that only position pixels, a demo, and a
/// ranking the estimate cannot change (it rises monotonically). Everything else feeds a number
/// the pilot reads or a decision and takes `math::sqrt` - also via micromath's acos, asin,
/// hypot and invsqrt, which use the estimate inside.
#[test]
fn every_root_that_feeds_a_number_is_exact() {
    const FAST_ROOT_ALLOWED: &[(&str, &str)] = &[
        ("src/utils/math.rs", "the start value of the Newton steps"),
        (
            "src/view/raster.rs",
            "isqrt_f: start value of its own Newton steps (drawing)",
        ),
        ("src/view/volume_panel.rs", "geometry of the pictograms"),
        ("src/flarm/demo.rs", "demo traffic"),
        (
            "src/flarm/traffic.rs",
            "ranking for eviction: order unchanged by the estimate",
        ),
        ("src/controller/flaps_ctl.rs", "flap demo: simulated TAS"),
    ];
    // compiled only for the unit tests (std)
    const TEST_ONLY: &[&str] = &[
        "src/view/test_fixture.rs",
        "src/view/flarm_render_tests.rs",
        "src/view/design_compare.rs",
    ];
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map_or(false, |x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(std::path::Path::new("src"), &mut files);
    assert!(files.len() > 50, "source not found");
    let mut found = Vec::new();
    for f in files {
        let name = f.to_string_lossy().replace('\\', "/");
        if TEST_ONLY.contains(&name.as_str()) {
            continue;
        }
        let allowed = FAST_ROOT_ALLOWED.iter().any(|(n, _)| *n == name);
        let text = std::fs::read_to_string(&f).unwrap();
        for (i, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if line.trim() == "#[cfg(test)]"
                && text
                    .lines()
                    .nth(i + 1)
                    .map_or(false, |l| l.trim_start().starts_with("mod "))
            {
                break; // the unit tests of the file
            }
            let fast = code.contains(".sqrt()") || code.contains("F32Ext::sqrt(");
            let hidden = code.contains(".acos()")
                || code.contains(".asin()")
                || code.contains(".hypot(")
                || code.contains(".invsqrt()");
            if hidden || (fast && !allowed) {
                found.push(format!("{name}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "approximate root where a number depends on it - use math::sqrt:\n{}",
        found.join("\n")
    );
}
