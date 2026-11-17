use crate::flight_physics::{polar_store::BasicGliderData, AirSpeed};
use crate::system_of_units::{Density, Float, FloatToMass, Mass, Speed};
use crate::utils::math;

#[allow(unused_imports)]
use micromath::F32Ext;
use num::clamp;

#[derive(Clone, Debug)]
pub struct PolarKoefs {
    pub a: Float,
    pub b: Float,
    pub c: Float,
    pub v_min: Float,
    pub weight: Float,
}

#[derive(Clone, Copy)]
pub struct GliderData {
    pub pilot_weight: Mass,
    pub water_ballast: Mass,
    bugs: f32,
    pub basic_glider_data: BasicGliderData,
}

impl Default for GliderData {
    fn default() -> Self {
        GliderData {
            pilot_weight: 90.0.kg(),
            water_ballast: 0.0.kg(),
            bugs: 1.0,
            basic_glider_data: BasicGliderData::default(),
        }
    }
}

impl GliderData {
    pub fn from_basic_glider_data(basic_glider_data: BasicGliderData) -> GliderData {
        GliderData {
            basic_glider_data,
            ..Default::default()
        }
    }

    /// Water ballast as fraction of the tank, always a number in 0..1.
    /// Polars with no water tanks use max_ballast = 0; never divide (NaN/Inf
    /// on the CAN ballast fraction crashes the sensorbox NMEA formatter, #185).
    pub fn ballast_fraction(&self) -> f32 {
        let max = self.basic_glider_data.max_ballast;
        let water = self.water_ballast.to_kg();
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if !(max > 0.0) || !water.is_finite() {
            return 0.0;
        }
        clamp(water / max, 0.0, 1.0)
    }

    pub fn set_ballast_fraction(&mut self, fraction: f32) {
        let fraction = if fraction.is_finite() {
            clamp(fraction, 0.0, 1.0)
        } else {
            0.0
        };
        let max = self.basic_glider_data.max_ballast.max(0.0);
        self.water_ballast = (fraction * max).kg();
    }

    pub fn bugs(&self) -> f32 {
        // This revised definition is based on XCSoar
        // XCSoar 1.0 .. 1.5 => Display Bugs 1.0 .. 2.0
        0.5 * (1.0 + self.bugs)
    }

    pub fn set_bugs(&mut self, bugs: f32) {
        // This revised definition is based on XCSoar
        // XCSoar 1.0 .. 1.5 => Display Bugs 1.0 .. 2.0
        let bugs = clamp(bugs, 1.0, 1.5);
        self.bugs = (bugs - 0.5) / 0.5;
    }
}
pub struct Polar {
    max_speed: Float,     // m/s
    density_ratio: Float, // -
    curr: PolarKoefs,     // current koefs
    refer: PolarKoefs,    // reference coefs
}

impl Default for Polar {
    fn default() -> Self {
        Self {
            curr: PolarKoefs {
                a: 0.0,
                b: 0.0,
                c: 0.0,
                v_min: 0.0,
                weight: 0.0,
            },
            refer: PolarKoefs {
                a: 0.0,
                b: 0.0,
                c: 0.0,
                v_min: 0.0,
                weight: 0.0,
            },
            max_speed: 0.0,
            density_ratio: 1.0,
        }
    }
}

/// Simple model of the polar curve for a glider
///
/// The polar of the glider is mapped with a simple model, a quadratic approximation. The
/// coefficients are calculated from 3 points of the polar curve.
///
/// If the pilot weight, water ballast, empty weight, pollution of the glider with mosquitoes
/// or air density changes, the polar curve is recalculated. Airspeeds are output as type
/// [AirSpeed], which contains both TAS and IAS.
impl Polar {
    /// calc polar coefficients
    pub fn recalc_glider(&mut self, glider_data: &GliderData) {
        let bgd = &glider_data.basic_glider_data;
        let (v1, w1) = (bgd.polar_values[0][0] / 3.6, bgd.polar_values[0][1]);
        let (v2, w2) = (bgd.polar_values[1][0] / 3.6, bgd.polar_values[1][1]);
        let (v3, w3) = (bgd.polar_values[2][0] / 3.6, bgd.polar_values[2][1]);

        let a = ((v2 - v3) * (w1 - w3) + (v3 - v1) * (w2 - w3))
            / (v1 * v1 * (v2 - v3) + v2 * v2 * (v3 - v1) + v3 * v3 * (v1 - v2));
        let b = (w2 - w3 - a * (v2 * v2 - v3 * v3)) / (v2 - v3);
        let c = w3 - a * v3 * v3 - b * v3;

        let v_min = -b / a / 2.0;
        let weight = bgd.reference_weight;

        self.curr = PolarKoefs {
            a,
            b,
            c,
            v_min,
            weight,
        };
        self.refer = self.curr.clone();
        self.max_speed = bgd.max_speed / 3.6;
        self.density_ratio = 1.0;
    }

    /// recalc polar to adopt weight and density changes
    pub fn recalc(&mut self, glider_data: &GliderData, density: Density) {
        let weight = (glider_data.basic_glider_data.empty_mass.kg()
            + glider_data.pilot_weight
            + glider_data.water_ballast)
            .to_kg();
        // exact roots: the micromath one made the polar heavier and the speed to fly
        // faster than they are (up to 17 km/h, 0.4.1.49)
        let ratio_weight = math::sqrt(weight / self.refer.weight);
        self.density_ratio = math::sqrt(Density::AT_NN().0 / density.to_kg_m3());
        let ratio = ratio_weight * self.density_ratio;

        self.curr.a = glider_data.bugs * self.refer.a / ratio;
        self.curr.b = glider_data.bugs * self.refer.b;
        self.curr.c = glider_data.bugs * self.refer.c * ratio;

        self.curr.v_min = self.refer.v_min * ratio;
        self.curr.weight = weight;
    }

    /// Returns the sink rate of the glider
    ///
    /// It is checked that the speed is within the permissible range.
    pub fn sink_rate(&self, speed: AirSpeed) -> Speed {
        let v = self.clamp_speed(speed.tas().to_m_s());
        let sink_rate = v * v * self.curr.a + v * self.curr.b + self.curr.c;
        Speed(sink_rate)
    }

    /// Returns the sink rate at a load factor (turn, pull-up)
    ///
    /// The polar holds for 1 g. The part of the sink that grows with the lift is taken from
    /// the polar itself: at best glide half of the sink depends on the lift, so with
    /// B = -sink(v_bg) * v_bg / 2 the additional sink is B * (n * n - 1) / v. No aspect
    /// ratio or span is needed (the polar store has none).
    pub fn sink_rate_g(&self, speed: AirSpeed, load_factor: Float) -> Speed {
        let sink = self.sink_rate(speed).to_m_s();
        if self.curr.a >= 0.0 || self.curr.c >= 0.0 {
            // no polar loaded
            return Speed(sink);
        }
        let v_bg = math::sqrt(self.curr.c / self.curr.a);
        let w_bg = 2.0 * self.curr.c + self.curr.b * v_bg;
        let b_lift = -0.5 * w_bg * v_bg;
        let v_floor = 0.8 * self.curr.v_min;
        let tas = speed.tas().to_m_s();
        let v = if tas > v_floor { tas } else { v_floor };
        Speed(sink - b_lift * (load_factor * load_factor - 1.0) / v)
    }

    /// Returns the indicated airspeed a glider of the current weight circles at with the
    /// given load factor: lift coefficient 1.3. Without a wing area 0.83 times best glide.
    pub fn circling_ias(&self, wing_area_m2: Float, load_factor: Float) -> Speed {
        const CL_CIRCLE: Float = 1.3;
        const G: Float = 9.80665;
        if wing_area_m2 > 1.0 && self.curr.weight > 1.0 {
            let rho = Density::AT_NN().0;
            Speed(math::sqrt(
                2.0 * load_factor * self.curr.weight * G / (rho * wing_area_m2 * CL_CIRCLE),
            ))
        } else if self.curr.a < 0.0 && self.curr.c < 0.0 {
            Speed(0.83 * math::sqrt(self.curr.c / self.curr.a) / self.density_ratio)
        } else {
            Speed(25.0)
        }
    }

    /// Returns the speed for minimal sink
    pub fn min_sink_speed(&self) -> AirSpeed {
        let v = self.clamp_speed(-self.curr.b / self.curr.a / 2.0);
        self.airspeed_from_tas(v)
    }

    /// Returns the speed to fly, which is a function of the expected climb and the metereological
    /// sink.
    pub fn speed_to_fly(&self, si_met: Speed, st_mc_cready: Speed) -> AirSpeed {
        let (met, mc_cready) = (si_met.to_m_s(), st_mc_cready.to_m_s());
        let val = (self.curr.c + met - mc_cready) / self.curr.a;
        let stf = if val > 0.0 { math::sqrt(val) } else { 0.0 };
        let stf = self.clamp_speed(stf);
        self.airspeed_from_tas(stf)
    }

    /// Returns the current possible minimum speed
    pub fn v_min(&self) -> AirSpeed {
        self.airspeed_from_tas(self.curr.v_min)
    }

    /// Returns the gliding ratio
    pub fn gliding_ratio(&self, speed: AirSpeed) -> Float {
        let v = self.clamp_speed(speed.tas().to_m_s());
        let sink_rate = v * v * self.curr.a + v * self.curr.b + self.curr.c;
        -v / sink_rate
    }

    fn clamp_speed(&self, speed: Float) -> Float {
        match speed {
            v if v > self.max_speed => self.max_speed,
            v if v < self.curr.v_min => self.curr.v_min,
            v if v.is_nan() => self.curr.v_min,
            _ => speed,
        }
    }

    fn airspeed_from_tas(&self, tas: Float) -> AirSpeed {
        let ias = tas / self.density_ratio;
        AirSpeed::new(ias, tas)
    }
}

#[cfg(test)]
mod tests {
    /// a glider without water tank and an unwritten (NaN) value never give a
    /// NaN ballast fraction - the sensor box crashes on it (box 0.7.x)
    #[test]
    fn ballast_fraction_is_always_a_number() {
        use crate::system_of_units::FloatToMass;
        let mut gd = super::GliderData::default();
        gd.basic_glider_data.max_ballast = 0.0;
        gd.water_ballast = 0.0.kg();
        assert_eq!(gd.ballast_fraction(), 0.0);
        gd.water_ballast = 50.0.kg();
        assert_eq!(gd.ballast_fraction(), 0.0);
        gd.basic_glider_data.max_ballast = 100.0;
        gd.water_ballast = f32::from_bits(0xffff_ffff).kg(); // erased EEPROM
        assert_eq!(gd.ballast_fraction(), 0.0);
        gd.water_ballast = 150.0.kg();
        assert_eq!(gd.ballast_fraction(), 1.0);
        gd.water_ballast = 25.0.kg();
        assert!((gd.ballast_fraction() - 0.25).abs() < 1e-6);
        gd.set_ballast_fraction(f32::NAN);
        assert_eq!(gd.water_ballast.to_kg(), 0.0);
    }

    use super::*;
    use crate::assert_float_eq;
    use crate::{FloatToDensity, FloatToMass, FloatToSpeed};
    use std::{fs::File, io::*, vec::Vec};

    const LS3_GLIDER_DATA: BasicGliderData = BasicGliderData {
        // 0
        name: "LS-3 WL", // first winglets
        wing_area: 10.5,
        max_speed: 270.0,
        empty_mass: 280.0,
        max_ballast: 121.0,
        reference_weight: 396.0,
        handicap: 107,
        polar_values: [[80.0, -0.604], [105.0, -0.700], [180.0, -1.939]],
    };

    const AS33_GLIDER_DATA: BasicGliderData = BasicGliderData {
        // No 167,  Manufacturer's data interpreted by Andreas Westkamp
        name: "AS 33 18m",
        wing_area: 10.00,
        max_speed: 270.0,
        empty_mass: 285.0,
        max_ballast: 220.0,
        reference_weight: 400.0,
        handicap: 122,
        polar_values: [[97.2, -0.511], [111.6, -0.556], [180.0, -1.369]],
    };

    fn write_stf_to_csv(file_name: &str, polar: &mut Polar) {
        fn write_stf_for_mc<W: Write>(f: &mut W, mc: f32, polar: &mut Polar) {
            let mut si_met = 0.0;
            while si_met > -3.1 {
                let stf = polar.speed_to_fly(si_met.m_s(), mc.m_s()).tas().to_km_h();
                writeln!(f, "{:.1};{:.1};{:.0}", mc, si_met, stf).unwrap();
                si_met -= 0.5;
            }
        }

        //let mut now_calculated = File::create(file_name).unwrap();
        let mut now_calculated = Vec::new();
        now_calculated.write(b"mc;si_met;speed_to_fly\n").unwrap();
        write_stf_for_mc(&mut now_calculated, 0.0, polar);
        write_stf_for_mc(&mut now_calculated, 1.0, polar);
        write_stf_for_mc(&mut now_calculated, 2.0, polar);
        write_stf_for_mc(&mut now_calculated, 3.0, polar);

        let mut r = File::open(file_name).unwrap();
        let mut should_be = Vec::new();
        let _ = r.read_to_end(&mut should_be).unwrap();
        assert_eq!(now_calculated, should_be);
    }

    #[test]
    fn test_stf_table() {
        let mut glider_data = GliderData::default();
        glider_data.basic_glider_data = LS3_GLIDER_DATA;
        let mut polar = Polar::default();
        polar.recalc_glider(&glider_data);
        polar.recalc(&glider_data, Density::AT_NN());

        write_stf_to_csv("tests/ls3_wl.csv", &mut polar);

        glider_data.basic_glider_data = AS33_GLIDER_DATA;
        let mut polar = Polar::default();
        polar.recalc_glider(&glider_data);
        polar.recalc(&glider_data, Density::AT_NN());

        write_stf_to_csv("tests/as33.csv", &mut polar);
    }

    #[test]
    fn test_ballast_fraction_max_zero() {
        let mut glider_data = GliderData::default();
        glider_data.basic_glider_data.max_ballast = 0.0;
        glider_data.water_ballast = 50.0.kg();

        assert_eq!(glider_data.ballast_fraction(), 0.0);

        glider_data.set_ballast_fraction(0.5);
        assert_eq!(glider_data.water_ballast.to_kg(), 0.0);
        assert_eq!(glider_data.ballast_fraction(), 0.0);

        glider_data.basic_glider_data.max_ballast = 100.0;
        glider_data.water_ballast = 25.0.kg();
        assert_eq!(glider_data.ballast_fraction(), 0.25);
    }

    #[test]
    fn test_basic_functions() {
        let mut glider_data = GliderData::default();
        glider_data.basic_glider_data = LS3_GLIDER_DATA;

        #[allow(unused_mut)]
        let mut polar = Polar::default();
        polar.recalc_glider(&glider_data);
        polar.recalc(&glider_data, Density::AT_NN());

        assert_float_eq!(
            polar.gliding_ratio(AirSpeed::from_tas_at_nn(101.86.km_h())),
            41.68
        );
        assert_float_eq!(polar.min_sink_speed().tas().to_km_h(), 74.77);
        assert_float_eq!(
            polar.gliding_ratio(AirSpeed::from_tas_at_nn(180.0.km_h())),
            24.56
        );

        assert_float_eq!(
            polar
                .sink_rate(AirSpeed::from_tas_at_nn(90.0.km_h()))
                .to_m_s(),
            -0.613
        );
        assert_float_eq!(
            polar
                .sink_rate(AirSpeed::from_tas_at_nn(135.0.km_h()))
                .to_m_s(),
            -1.059
        );
        assert_float_eq!(
            polar
                .sink_rate(AirSpeed::from_tas_at_nn(200.0.km_h()))
                .to_m_s(),
            -2.64
        );

        assert_float_eq!(
            polar.speed_to_fly(0.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            100.2
        );
        assert_float_eq!(
            polar.speed_to_fly(0.62.m_s(), 0.0.m_s()).tas().to_km_h(),
            74.77
        );
        assert_float_eq!(
            polar.speed_to_fly(1.0.m_s(), 1.0.m_s()).tas().to_km_h(),
            100.2
        );
        assert_float_eq!(
            polar.speed_to_fly(-1.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            132.9
        );
        assert_float_eq!(
            polar.speed_to_fly(-2.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            159.0
        );
        assert_float_eq!(
            polar.speed_to_fly(-3.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            181.4
        );
        assert_float_eq!(
            polar.speed_to_fly(10.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            74.77
        );
        assert_float_eq!(
            polar.speed_to_fly(-99.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            270.0
        );

        glider_data.water_ballast = 121.0.kg();
        polar.recalc(&glider_data, Density::AT_NN());
        assert_float_eq!(
            polar.speed_to_fly(0.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            115.4
        );
        assert_float_eq!(
            polar.speed_to_fly(-3.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            199.15
        );

        glider_data.water_ballast = 0.0.kg();
        polar.recalc(&glider_data, Density::AT_NN());
        assert_float_eq!(
            polar.speed_to_fly(0.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            100.2
        );

        glider_data.pilot_weight = 120.0.kg();
        polar.recalc(&glider_data, Density::AT_NN());
        assert_float_eq!(
            polar.speed_to_fly(0.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            104.2
        );

        glider_data.pilot_weight = 90.0.kg();
        polar.recalc(&glider_data, Density::AT_NN());
        assert_float_eq!(
            polar.speed_to_fly(0.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            100.2
        );

        glider_data.basic_glider_data.empty_mass = 260.0;
        polar.recalc(&glider_data, Density::AT_NN());
        assert_float_eq!(
            polar.speed_to_fly(0.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            97.4
        );

        glider_data.basic_glider_data.empty_mass = 280.0;
        polar.recalc(&glider_data, Density::AT_NN());
        assert_float_eq!(
            polar.speed_to_fly(0.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            100.2
        );

        glider_data.set_bugs(1.05);
        polar.recalc(&glider_data, Density::AT_NN());
        assert_float_eq!(
            polar.gliding_ratio(AirSpeed::from_tas_at_nn(105.0.km_h())),
            37.7
        );

        glider_data.set_bugs(1.5);
        polar.recalc(&glider_data, Density::AT_NN());
        assert_float_eq!(
            polar.gliding_ratio(AirSpeed::from_tas_at_nn(105.0.km_h())),
            20.75
        );

        glider_data.set_bugs(1.0);
        polar.recalc(&glider_data, Density::AT_NN());
        assert_float_eq!(
            polar.gliding_ratio(AirSpeed::from_tas_at_nn(105.0.km_h())),
            41.5
        );

        polar.recalc(&glider_data, 0.913.kg_m3());
        let speed = polar.speed_to_fly(0.0.m_s(), 0.0.m_s());
        assert_float_eq!(speed.ias().to_km_h(), 100.2);
        assert_float_eq!(speed.tas().to_km_h(), 116.0);

        polar.recalc(&glider_data, Density::AT_NN());
        assert_float_eq!(
            polar.speed_to_fly(0.0.m_s(), 0.0.m_s()).tas().to_km_h(),
            100.2
        );
    }
}

/// Best glide ratio and the speed for it (km/h, ISA sea level - IAS = TAS)
/// with the masses and bugs of `glider_data`; None without a polar
pub fn best_glide(glider_data: &GliderData) -> Option<(Float, Float)> {
    if glider_data.basic_glider_data.reference_weight <= 0.0 {
        return None;
    }
    let mut p = Polar::default();
    p.recalc_glider(glider_data);
    p.recalc(glider_data, Density::AT_NN());
    let (a, b, c) = (p.curr.a, p.curr.b, p.curr.c);
    // L/D = -v / (a v^2 + b v + c) is largest where c = a v^2
    if a == 0.0 || c / a <= 0.0 {
        return None;
    }
    let v = p.clamp_speed((c / a).sqrt());
    let sink = v * v * a + v * b + c;
    if sink >= 0.0 {
        return None;
    }
    Some((-v / sink, v * 3.6))
}

/// Glide ratio at the speed to fly for `mc` in still air and that speed
/// (km/h, ISA sea level); for MC 0 it is the best glide
pub fn mc_glide(
    glider_data: &GliderData,
    mc: crate::system_of_units::Speed,
) -> Option<(Float, Float)> {
    if glider_data.basic_glider_data.reference_weight <= 0.0 {
        return None;
    }
    let mut p = Polar::default();
    p.recalc_glider(glider_data);
    p.recalc(glider_data, Density::AT_NN());
    let v = p
        .speed_to_fly(crate::system_of_units::Speed(0.0), mc)
        .tas()
        .to_m_s();
    let sink = p
        .sink_rate(crate::flight_physics::AirSpeed::new(v, v))
        .to_m_s();
    if sink >= 0.0 {
        return None;
    }
    Some((-v / sink, v * 3.6))
}

#[cfg(test)]
mod best_glide_tests {
    use super::*;
    use crate::flight_physics::polar_store;

    #[test]
    fn best_glide_of_the_default_glider_is_plausible_and_moves_with_the_ballast() {
        let mut gd = GliderData::from_basic_glider_data(*polar_store::from_raw_idx(
            polar_store::DEFAULT_GLIDER_IDX as usize,
        ));
        let (e0, v0) = best_glide(&gd).expect("polar");
        assert!(e0 > 35.0 && e0 < 70.0, "L/D {e0}");
        assert!(v0 > 80.0 && v0 < 140.0, "speed {v0}");
        gd.water_ballast = 150.0.kg();
        let (e1, v1) = best_glide(&gd).unwrap();
        assert!((e1 - e0).abs() < 0.5, "same L/D with ballast: {e0} {e1}");
        assert!(v1 > v0 + 3.0, "faster with ballast: {v0} -> {v1}");
        // MC 0 is the best glide, a higher MC flies faster at a lower L/D
        let (m0, s0) = mc_glide(&gd, crate::system_of_units::Speed(0.0)).unwrap();
        assert!(
            (m0 - e1).abs() < 0.5 && (s0 - v1).abs() < 2.0,
            "MC 0 {m0}/{s0} vs {e1}/{v1}"
        );
        let (m2, s2) = mc_glide(&gd, crate::system_of_units::Speed(2.0)).unwrap();
        assert!(m2 < m0 && s2 > s0 + 20.0, "MC 2: {m2} at {s2}");
    }
}
