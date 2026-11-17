use crate::{
    controller::SoundParams,
    model::UnitVerticalSpeed,
    system_of_units::{FloatToLength, FloatToSpeed, Length, Speed},
    AirSpeed, CoreModel,
};

/// Verdict of the POT box against the bar of the pilot (MacCready value)
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum PotVerdict {
    /// no statement: straight flight, the value is close below the bar, or the box holds
    #[default]
    None,
    /// the circle is expected to bring at least the bar
    Yes,
    /// clearly below the bar
    No,
}

/// Metastructure for calculated or set values
#[derive(Copy, Clone)]
pub struct Calculated {
    pub circle_hysteresis: i8,
    pub speed_to_fly: AirSpeed,
    pub av_speed_to_fly: Speed, // ref. IAS
    pub speed_to_fly_dif: Speed,
    pub speed_to_fly_1s: Speed, // ref. IAS
    pub thermal_climb_rate: Speed,
    /// mean climb of the last finished thermal (THERMAL box in cruise)
    pub thermal_climb_last: Speed,
    /// climb rate the sound preview plays right now (controller/sound.rs)
    pub sound_preview: Option<f32>,
    pub av2_climb_rate: Speed, // calculated by frontend
    pub circle_diameter: Length,
    pub circle_diameter_valid: bool,
    pub circle_max_min_last: Speed,
    pub circle_max_min_valid: bool,
    pub frequency: u16,
    pub continuous: bool,
    pub gain: i8,
    pub interpolated_climb_rate: InterpolatedClimbRate,
    /// Info box "Potential Climb" (POT): what a circle here is worth.
    /// None = not flying or no GNSS, the box shows the average climb then
    pub pot_climb: Option<Speed>,
    /// phase controlled AoA correction of the vario needle (0 if standard)
    pub needle_correction: Speed,
    /// true while the POT box shows an estimate (straight flight, roll-in,
    /// first circle) instead of a measured climb - drawn in magenta
    pub pot_predictive: bool,
    /// yes / no of the POT box against the MacCready value, only while circling
    pub pot_verdict: PotVerdict,
    /// recommended flap stage (index into the flap table), None = no flaps
    pub flap_rec: Option<u8>,
    /// actual flap position as fractional stage index, None = no sensor
    pub flap_actual: Option<f32>,
    /// FLARM: everything the warning, radar and info pages show
    pub flarm: crate::model::flarm_show::FlarmShow,
    /// gust filtered climb rate (PT2), source of needle and vario tone;
    /// with Gust Filter "Off" identical to sensor.climb_rate
    pub gust_filtered_climb: Speed,
    pub sound_params: SoundParams,
}

impl Default for Calculated {
    #[allow(unused)]
    fn default() -> Self {
        Calculated {
            circle_hysteresis: 0,
            speed_to_fly: AirSpeed::from_tas_at_nn(100.0.km_h()),
            av_speed_to_fly: Speed::from_km_h(0.0),
            speed_to_fly_dif: 0.0.km_h(),
            speed_to_fly_1s: 0.0.km_h(),
            thermal_climb_rate: 0.0.m_s(),
            thermal_climb_last: 0.0.m_s(),
            sound_preview: None,
            av2_climb_rate: 0.0.m_s(),
            circle_diameter: 0.0.m(),
            circle_diameter_valid: false,
            circle_max_min_last: 0.0.m_s(),
            circle_max_min_valid: false,
            frequency: 500,
            continuous: false,
            gain: 2,
            interpolated_climb_rate: InterpolatedClimbRate::default(),
            pot_climb: None,
            needle_correction: 0.0.m_s(),
            pot_predictive: false,
            pot_verdict: PotVerdict::None,
            flap_rec: None,
            flap_actual: None,
            flarm: crate::model::flarm_show::FlarmShow::EMPTY,
            gust_filtered_climb: 0.0.m_s(),
            sound_params: SoundParams::default(),
        }
    }
}

/// Needle smoothing (ring design): the 10 Hz climb rate is followed with an
/// exponential approach (tau 0.23 s), and the DRAWN value only moves in
/// steps of about one pixel at the needle tip. Without the step the needle
/// crawled sub-pixel every frame and the unsmoothed edges shimmered.
#[derive(Clone, Copy)]
pub struct InterpolatedClimbRate {
    /// continuous, smoothed value
    pub value: Speed,
    /// value the needle is drawn with (quantized with hysteresis)
    pub shown: Speed,
    /// the needle is beyond full scale (ring design): the scale numbers of
    /// the negative half are shown continued (6..9 resp. 12..18)
    pub overrange: bool,
    /// seconds the overrange display is still held after the needle came
    /// back below full scale
    overrange_hold_s: f32,
}

impl Default for InterpolatedClimbRate {
    fn default() -> Self {
        InterpolatedClimbRate {
            value: 0.0.m_s(),
            shown: 0.0.m_s(),
            overrange: false,
            overrange_hold_s: 0.0,
        }
    }
}

/// hold time of the continued scale after the needle dropped back below
/// full scale - a needle oscillating around full scale must not flip the
/// numbers back and forth
const OVERRANGE_HOLD_S: f32 = 2.0;

/// time constant of the needle approach
const NEEDLE_TAU_S: f32 = 0.23;
/// closer than this the needle snaps onto the target (m/s)
const NEEDLE_SNAP_M_S: f32 = 0.002;
/// minimum change of the drawn value: 0.006 m/s = 0.22 deg on the ring
/// scale (36 deg per m/s) = 0.75 px at the needle tip (r = 200 px)
const NEEDLE_STEP_M_S: f32 = 0.006;

impl CoreModel {
    /// Needle interpolation for one frame of the default frame rate
    pub fn interpolate_climb_rate(&mut self) {
        self.interpolate_climb_rate_dt(1.0 / crate::view::FRAME_RATE as f32);
    }

    /// Needle interpolation for a frame of `dt_s` seconds (the V2 renders
    /// synchronized to the display refresh, other targets with a timer)
    pub fn interpolate_climb_rate_dt(&mut self, dt_s: f32) {
        // needle source: gust filtered climb rate (equals sensor.climb_rate
        // when the gust filter is off) plus the entry optimized correction,
        // both smoothed together so the 10 Hz steps of the correction do
        // not show either
        let target = self.calculated.gust_filtered_climb.to_m_s()
            + self.calculated.needle_correction.to_m_s();
        let icr = &mut self.calculated.interpolated_climb_rate;
        let mut value = icr.value.to_m_s();
        let diff = target - value;
        let snapped = diff.abs() < NEEDLE_SNAP_M_S;
        if snapped {
            value = target;
        } else {
            let alpha = dt_s / (NEEDLE_TAU_S + dt_s);
            value += diff * alpha;
        }
        icr.value = value.m_s();
        // redraw only for a visible step - and once more exactly on the
        // target, so the resting needle is never a fraction off
        if (value - icr.shown.to_m_s()).abs() >= NEEDLE_STEP_M_S
            || (snapped && icr.shown.to_m_s() != value)
        {
            icr.shown = value.m_s();
        }

        // continued scale numbers while the needle is beyond full scale
        let full_scale = match self.config.unit_vertical_speed {
            UnitVerticalSpeed::Mps => 5.0.m_s(),
            UnitVerticalSpeed::Knots => 10.0.kt(),
            UnitVerticalSpeed::Fpm => 1000.0.ft_min(),
        };
        if icr.shown >= full_scale {
            icr.overrange = true;
            icr.overrange_hold_s = OVERRANGE_HOLD_S;
        } else if icr.overrange {
            icr.overrange_hold_s -= dt_s;
            if icr.overrange_hold_s <= 0.0 {
                icr.overrange = false;
                icr.overrange_hold_s = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod needle_tests {
    use crate::{CoreModel, FloatToSpeed};

    fn model() -> CoreModel {
        crate::view::test_fixture::test_model()
    }

    #[test]
    fn needle_settles_exactly_and_moves_in_pixel_steps() {
        let mut cm = model();
        cm.calculated.gust_filtered_climb = 2.0.m_s();
        cm.calculated.needle_correction = 0.0.m_s();
        let mut last_shown = cm.calculated.interpolated_climb_rate.shown.to_m_s();
        let mut steps = 0;
        let mut min_step = f32::MAX;
        for _ in 0..200 {
            cm.interpolate_climb_rate_dt(1.0 / 33.5);
            let shown = cm.calculated.interpolated_climb_rate.shown.to_m_s();
            if shown != last_shown {
                steps += 1;
                // the final settling step onto the target may be smaller
                if shown != 2.0 {
                    min_step = min_step.min((shown - last_shown).abs());
                }
                last_shown = shown;
            }
        }
        // converged onto the target - no residual crawl
        assert_eq!(cm.calculated.interpolated_climb_rate.value.to_m_s(), 2.0);
        assert_eq!(cm.calculated.interpolated_climb_rate.shown.to_m_s(), 2.0);
        // every redraw moved the needle by at least ~0.75 px at the tip
        assert!(min_step >= 0.006 - 1e-6, "smallest step {min_step}");
        // a 2 m/s swing takes roughly 1 s and no more than ~40 redraws
        assert!(steps > 10 && steps < 45, "{steps} steps");
        // the entry optimized correction is part of the target
        cm.calculated.needle_correction = (-0.5).m_s();
        for _ in 0..200 {
            cm.interpolate_climb_rate_dt(1.0 / 33.5);
        }
        assert!((cm.calculated.interpolated_climb_rate.shown.to_m_s() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn overrange_scale_switches_with_hold() {
        use crate::model::UnitVerticalSpeed;
        let mut cm = model();
        cm.config.unit_vertical_speed = UnitVerticalSpeed::Mps;
        let dt = 1.0 / 33.5;
        // 4.9 m/s: within full scale
        cm.calculated.gust_filtered_climb = 4.9.m_s();
        for _ in 0..100 {
            cm.interpolate_climb_rate_dt(dt);
        }
        assert!(!cm.calculated.interpolated_climb_rate.overrange);
        // 5.5 m/s: beyond full scale -> continued numbers
        cm.calculated.gust_filtered_climb = 5.5.m_s();
        for _ in 0..100 {
            cm.interpolate_climb_rate_dt(dt);
        }
        assert!(cm.calculated.interpolated_climb_rate.overrange);
        // back to 4.0: held for 2 s, then normal numbers again
        cm.calculated.gust_filtered_climb = 4.0.m_s();
        let mut frames_held = 0;
        for _ in 0..200 {
            cm.interpolate_climb_rate_dt(dt);
            if cm.calculated.interpolated_climb_rate.overrange {
                frames_held += 1;
            }
        }
        assert!(!cm.calculated.interpolated_climb_rate.overrange);
        let held_s = frames_held as f32 * dt;
        assert!(held_s > 1.9 && held_s < 2.4, "held {held_s} s");
        // knots: full scale 10 kt (5.14 m/s)
        cm.config.unit_vertical_speed = UnitVerticalSpeed::Knots;
        cm.calculated.gust_filtered_climb = 5.05.m_s();
        for _ in 0..200 {
            cm.interpolate_climb_rate_dt(dt);
        }
        assert!(!cm.calculated.interpolated_climb_rate.overrange);
        cm.calculated.gust_filtered_climb = 5.3.m_s();
        for _ in 0..100 {
            cm.interpolate_climb_rate_dt(dt);
        }
        assert!(cm.calculated.interpolated_climb_rate.overrange);
    }

    #[test]
    fn needle_time_constant_is_independent_of_frame_rate() {
        // one second of approach at 20 Hz and at 33.5 Hz ends up at the same place
        let mut a = model();
        let mut b = model();
        a.calculated.gust_filtered_climb = 3.0.m_s();
        b.calculated.gust_filtered_climb = 3.0.m_s();
        for _ in 0..20 {
            a.interpolate_climb_rate_dt(0.05);
        }
        for _ in 0..34 {
            b.interpolate_climb_rate_dt(1.0 / 34.0);
        }
        let va = a.calculated.interpolated_climb_rate.value.to_m_s();
        let vb = b.calculated.interpolated_climb_rate.value.to_m_s();
        assert!((va - vb).abs() < 0.1, "{va} vs {vb}");
        // after 1 s (~4 tau) the needle is within 3 % of the target
        assert!(va > 2.9, "{va}");
    }
}
