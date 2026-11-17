//! The vario tone
//!
//! Frequency, beep period and duty cycle are three *independent* curves, given as support
//! points and linearly interpolated (derived from `vario_tone_lab_15.html`). The beep pattern
//! is produced by the 10 ms sequencer in `controller::sound`, not by the driver.
//!
//! All tables are pure data, the interpolation is allocation free and `no_std` safe.
//!
//! The tables are written for the reference centre frequency [`STEFLY_CENTER_FREQ`] (350 Hz)
//! and are stretched onto the three user anchors - centre frequency at zero, maximum
//! frequency at +reference, minimum frequency at -reference. The stretch happens in log
//! space, so the shape of the curve and with it the musical character are preserved; with
//! the default anchors it reproduces the unstretched table to within 3 cent.

use num::clamp;

#[allow(unused_imports)]
use micromath::F32Ext;

/// Centre frequency the Stefly tables are written for
pub const STEFLY_CENTER_FREQ: f32 = 350.0;

/// Frequency window the Stefly profile is clamped to.
///
/// The lower bound is a hardware limit: the DAC timer reload value is a `u16`, which overflows
/// below roughly 153 Hz (200 MHz / 20 samples / 65535). The upper bound covers the highest
/// table value (1900 Hz) plus head room for a raised centre frequency.
pub const STEFLY_MIN_FREQ: f32 = 175.0;
pub const STEFLY_MAX_FREQ: f32 = 2100.0;

/// Climb tone frequency: (climb rate [m/s], frequency [Hz])
///
/// One full octave between 0 and 2 m/s (6 semitones per m/s - that is where centring happens),
/// flatter above (about 2 semitones per m/s) up to 1900 Hz at 7 m/s, constant from there.
#[rustfmt::skip]
pub const STEFLY_CLIMB_FREQ: &[(f32, f32)] = &[
    (0.0, 350.0), (0.5, 450.0), (1.0, 550.0), (1.5, 650.0), (2.0, 750.0), (3.0, 950.0),
    (4.0, 1150.0), (5.0, 1350.0), (6.0, 1600.0), (7.0, 1900.0), (10.0, 1900.0),
];

/// Beep period of the climb tone: (climb rate [m/s], period [ms])
#[rustfmt::skip]
pub const STEFLY_BEEP_PERIOD: &[(f32, f32)] = &[
    (0.0, 650.0), (0.5, 560.0), (1.0, 480.0), (1.5, 420.0), (2.0, 380.0), (3.0, 330.0),
    (4.0, 300.0), (5.0, 270.0), (6.0, 250.0), (8.0, 200.0), (10.0, 150.0),
];

/// Duty cycle of the climb tone: (climb rate [m/s], fraction of the period the tone is on)
#[rustfmt::skip]
pub const STEFLY_BEEP_DUTY: &[(f32, f32)] = &[
    (0.0, 0.45), (1.0, 0.50), (2.0, 0.55), (4.0, 0.60), (6.0, 0.64), (8.0, 0.67), (10.0, 0.70),
];

/// Continuous sink tone: (climb rate [m/s], frequency [Hz]), ascending in x
#[rustfmt::skip]
pub const STEFLY_SINK_FREQ: &[(f32, f32)] = &[
    (-10.0, 180.0), (-3.0, 240.0), (-1.5, 290.0), (0.0, 350.0),
];

/// Length of a single zero zone tick ("the lift sniffer") in ms
pub const STEFLY_TICK_LEN_MS: f32 = 35.0;
/// Pause between two ticks at 0 m/s
pub const STEFLY_TICK_PAUSE_ZERO_MS: f32 = 600.0;
/// Pause between two ticks at the lower end of the tick range
pub const STEFLY_TICK_PAUSE_EDGE_MS: f32 = 800.0;

/// Interval of a major third, used by both double beep patterns
pub const MAJOR_THIRD: f32 = 1.26;

// ---------------------------------------------------------------------------
// Double beep downwards - sink tone option and speed to fly "push"
// ---------------------------------------------------------------------------

/// Length factor of the two tones: (excess over the threshold, factor)
#[rustfmt::skip]
pub const DBD_LEN_FACTOR: &[(f32, f32)] = &[
    (0.0, 1.0), (1.0, 0.8), (2.0, 0.65), (4.0, 0.5),
];

/// Long pause after the pattern: (excess over the threshold, pause [ms])
#[rustfmt::skip]
pub const DBD_PAUSE_MS: &[(f32, f32)] = &[
    (0.0, 900.0), (0.5, 650.0), (1.0, 450.0), (1.5, 330.0), (2.0, 250.0), (3.0, 170.0), (4.0, 120.0),
];

/// Lowest frequency the pattern is allowed to reach
///
/// The concept puts the floor on the base tone (180 Hz), but the *second* tone sits a major
/// third below it and would land at 143 Hz - below what the DAC timer can produce (its reload
/// value is a u16). The floor therefore applies to the lower of the two tones, so the interval
/// survives at strong sink instead of collapsing against the hardware limit.
pub const DBD_MIN_FREQ: f32 = 180.0;
/// Anchor of the push/sink double beep relative to the center frequency
/// (320 Hz at the 500 Hz default center)
pub const DBD_ANCHOR_RATIO: f32 = 0.64;

/// Length of the first tone at length factor 1.0
pub const DBD_TONE1_MS: f32 = 90.0;
/// Gap between the two tones
pub const DBD_GAP_MS: f32 = 45.0;
/// Length of the second tone at length factor 1.0
pub const DBD_TONE2_MS: f32 = 120.0;

// ---------------------------------------------------------------------------
// Double beep upwards - speed to fly "pull"
// ---------------------------------------------------------------------------

pub const DBU_TONE1_MS: f32 = 70.0;
pub const DBU_GAP_MS: f32 = 50.0;
pub const DBU_TONE2_MS: f32 = 90.0;

// ---------------------------------------------------------------------------
// Loudness compensation
// ---------------------------------------------------------------------------

/// Level correction over pitch: (frequency [Hz], correction [dB])
///
/// High tones come out of the small speaker louder, and the ear is most sensitive around
/// 1-3 kHz. Without this the high climb tones sound shrill next to the low zero tone.
#[rustfmt::skip]
pub const LOUDNESS_COMP_DB: &[(f32, f32)] = &[
    (500.0, 0.0), (1000.0, -1.5), (1500.0, -3.0), (2000.0, -4.0),
];

/// Maximum level increase of the airspeed dependent volume in dB
pub const SPEED_VOLUME_MAX_DB: f32 = 9.0;

/// Linear interpolation over a table of support points.
///
/// The table must be sorted ascending by x and must not be empty. Outside the table the first
/// resp. last y value is returned (no extrapolation) - this is what `lerpTable` in the tone lab
/// does as well.
pub fn lerp_table(table: &[(f32, f32)], x: f32) -> f32 {
    match table.first() {
        None => 0.0,
        Some(&(x_first, y_first)) => {
            if x <= x_first {
                return y_first;
            }
            for idx in 1..table.len() {
                let (x1, y1) = table[idx];
                if x <= x1 {
                    let (x0, y0) = table[idx - 1];
                    let dx = x1 - x0;
                    return if dx > 0.0 {
                        y0 + (y1 - y0) * (x - x0) / dx
                    } else {
                        y1
                    };
                }
            }
            // x is beyond the last support point
            table[table.len() - 1].1
        }
    }
}

/// Stretch a table frequency onto the user anchors.
///
/// `f_tab` is the table value for the wanted climb rate, `f_ref_tab` the table value at the
/// reference climb rate. Both are relative to [`STEFLY_CENTER_FREQ`]. In log space the table
/// shape is scaled so that zero lands exactly on `center` and the reference exactly on
/// `anchor`. Degenerate settings fall back to the plain multiplicative shift.
fn stretch(f_tab: f32, f_ref_tab: f32, center: f32, anchor: f32) -> f32 {
    let plain = f_tab * center / STEFLY_CENTER_FREQ;
    if center <= 0.0 || anchor <= 0.0 || f_tab <= 0.0 || f_ref_tab <= 0.0 {
        return plain;
    }
    let l_ref = (f_ref_tab / STEFLY_CENTER_FREQ).ln();
    if l_ref.abs() < 0.01 {
        return plain;
    }
    let l = (f_tab / STEFLY_CENTER_FREQ).ln();
    center * ((anchor / center).ln() * l / l_ref).exp()
}

/// Frequency of the climb tone, stretched onto centre and maximum frequency
pub fn stefly_climb_freq(
    climb_rate: f32,
    center_freq: f32,
    freq_upper: f32,
    full_scale: f32,
) -> f32 {
    let reference = if full_scale > 0.1 { full_scale } else { 0.1 };
    stretch(
        lerp_table(STEFLY_CLIMB_FREQ, climb_rate),
        lerp_table(STEFLY_CLIMB_FREQ, reference),
        center_freq,
        freq_upper,
    )
}

/// Frequency of the continuous sink tone, stretched onto centre and minimum frequency
pub fn stefly_sink_freq(
    climb_rate: f32,
    center_freq: f32,
    freq_lower: f32,
    full_scale: f32,
) -> f32 {
    let reference = if full_scale > 0.1 { full_scale } else { 0.1 };
    stretch(
        lerp_table(STEFLY_SINK_FREQ, climb_rate),
        lerp_table(STEFLY_SINK_FREQ, -reference),
        center_freq,
        freq_lower,
    )
}

/// Beep period of the climb tone in ms
pub fn stefly_beep_period_ms(climb_rate: f32) -> f32 {
    lerp_table(STEFLY_BEEP_PERIOD, climb_rate)
}

/// Fraction of the beep period the tone is on
pub fn stefly_beep_duty(climb_rate: f32) -> f32 {
    lerp_table(STEFLY_BEEP_DUTY, climb_rate)
}

/// Pause between two zero zone ticks in ms.
///
/// `climb_rate` is expected to be within `threshold ..= 0.0`, `threshold` is the (negative)
/// climb rate where the ticks stop.
pub fn stefly_tick_pause_ms(climb_rate: f32, threshold: f32) -> f32 {
    if threshold >= 0.0 {
        return STEFLY_TICK_PAUSE_ZERO_MS;
    }
    let ratio = clamp(climb_rate / threshold, 0.0, 1.0);
    STEFLY_TICK_PAUSE_ZERO_MS + (STEFLY_TICK_PAUSE_EDGE_MS - STEFLY_TICK_PAUSE_ZERO_MS) * ratio
}

/// Anchor frequencies that reproduce a given spreading factor.
///
/// Used once when migrating an existing device: the spreading factor is no longer a setting
/// of its own, it is implied by the anchor points.
pub fn anchors_from_spreading(
    center_freq: f32,
    spreading_factor: f32,
    full_scale: f32,
) -> (f32, f32) {
    let exponent = crate::controller::SND_EXP_MUL * spreading_factor * full_scale;
    let upper = center_freq * exponent.exp();
    let lower = center_freq * (-exponent).exp();
    (upper, lower)
}

/// Base frequency of the downwards double beep - the stronger, the deeper
pub fn double_beep_down_freq(excess: f32, center_freq: f32) -> f32 {
    let floor = DBD_MIN_FREQ * MAJOR_THIRD;
    // the push/sink beep anchor follows the adjustable center frequency
    let value = center_freq * DBD_ANCHOR_RATIO * (-excess / 4.0 * core::f32::consts::LN_2).exp();
    if value < floor {
        floor
    } else {
        value
    }
}

/// Length factor of the two tones of the downwards double beep
pub fn double_beep_down_len_factor(excess: f32) -> f32 {
    lerp_table(DBD_LEN_FACTOR, excess)
}

/// Long pause after the downwards double beep
pub fn double_beep_down_pause_ms(excess: f32) -> f32 {
    lerp_table(DBD_PAUSE_MS, excess)
}

/// Base frequency of the upwards double beep, taken from the climb curve
pub fn double_beep_up_freq(excess: f32, center_freq: f32, freq_upper: f32, full_scale: f32) -> f32 {
    stefly_climb_freq(excess * 0.6, center_freq, freq_upper, full_scale)
}

/// Long pause after the upwards double beep
pub fn double_beep_up_pause_ms(excess: f32) -> f32 {
    let value = 800.0 - 140.0 * excess;
    if value < 180.0 {
        180.0
    } else {
        value
    }
}

/// Level correction for the given pitch in dB (never positive)
///
/// `strength` scales the whole curve: 0.0 switches the compensation off, 1.0 is the curve as
/// tabulated, 1.5 applies one and a half times the attenuation.
pub fn loudness_comp_db(frequency: f32, strength: f32) -> f32 {
    lerp_table(LOUDNESS_COMP_DB, frequency) * strength
}

/// Level increase from the airspeed in dB
///
/// Below the reference speed nothing happens; above it the level rises linearly in dB, which is
/// physically already the curved shape - sound pressure grows exponentially with airspeed, which
/// matches the roughly exponentially growing wind noise. Capped at [`SPEED_VOLUME_MAX_DB`].
pub fn speed_volume_db(ias_km_h: f32, reference_km_h: f32, slope_db_per_10km_h: f32) -> f32 {
    if slope_db_per_10km_h <= 0.0 || ias_km_h <= reference_km_h {
        return 0.0;
    }
    let db = (ias_km_h - reference_km_h) / 10.0 * slope_db_per_10km_h;
    clamp(db, 0.0, SPEED_VOLUME_MAX_DB)
}

/// Convert a level in dB into a linear amplitude factor
pub fn db_to_factor(db: f32) -> f32 {
    // 10^(db/20) == e^(db * ln(10) / 20)
    (db * 0.115_129_25).exp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_float_eq;

    #[test]
    fn test_lerp_table_bounds() {
        // below / above the table: no extrapolation
        assert_float_eq!(lerp_table(STEFLY_CLIMB_FREQ, -5.0), 350.0);
        assert_float_eq!(lerp_table(STEFLY_CLIMB_FREQ, 20.0), 1900.0);
        assert_float_eq!(lerp_table(STEFLY_SINK_FREQ, -20.0), 180.0);
        assert_float_eq!(lerp_table(STEFLY_SINK_FREQ, 3.0), 350.0);
    }

    #[test]
    fn test_lerp_table_support_points() {
        for (x, y) in STEFLY_CLIMB_FREQ.iter() {
            assert_float_eq!(lerp_table(STEFLY_CLIMB_FREQ, *x), *y);
        }
        for (x, y) in STEFLY_BEEP_PERIOD.iter() {
            assert_float_eq!(lerp_table(STEFLY_BEEP_PERIOD, *x), *y);
        }
        for (x, y) in STEFLY_BEEP_DUTY.iter() {
            assert_float_eq!(lerp_table(STEFLY_BEEP_DUTY, *x), *y);
        }
    }

    #[test]
    fn test_lerp_table_between() {
        // half way between 0.0 -> 350 Hz and 0.5 -> 450 Hz
        assert_float_eq!(lerp_table(STEFLY_CLIMB_FREQ, 0.25), 400.0);
        // half way between 2.0 -> 750 Hz and 3.0 -> 950 Hz
        assert_float_eq!(lerp_table(STEFLY_CLIMB_FREQ, 2.5), 850.0);
        // duty between 2.0 -> 0.55 and 4.0 -> 0.60
        assert_float_eq!(lerp_table(STEFLY_BEEP_DUTY, 3.0), 0.575);
    }

    #[test]
    fn test_center_frequency_shift() {
        // shifting the centre frequency keeps the ratios (and therefore the semitones)
        // anchor = table value at the reference, scaled with the centre frequency,
        // so the stretch is neutral and only the shift is measured
        let f_350 = stefly_climb_freq(2.0, 350.0, 1350.0, 5.0);
        let f_500 = stefly_climb_freq(2.0, 500.0, 1350.0 * 500.0 / 350.0, 5.0);
        assert_float_eq!(f_350, 750.0);
        assert_float_eq!(f_500 / f_350, 500.0 / 350.0);
    }

    #[test]
    fn test_octave_between_zero_and_two() {
        // the character of the profile: exactly one octave between 0 and 2 m/s
        let f0 = stefly_climb_freq(0.0, STEFLY_CENTER_FREQ, 1350.0, 5.0);
        let f2 = stefly_climb_freq(2.0, STEFLY_CENTER_FREQ, 1350.0, 5.0);
        assert!((f2 / f0 - 2.142857).abs() < 0.001);
    }

    #[test]
    fn test_tick_pause() {
        assert_float_eq!(stefly_tick_pause_ms(0.0, -0.5), 600.0);
        assert_float_eq!(stefly_tick_pause_ms(-0.25, -0.5), 700.0);
        assert_float_eq!(stefly_tick_pause_ms(-0.5, -0.5), 800.0);
        // ticks switched off -> no division by zero
        assert_float_eq!(stefly_tick_pause_ms(0.0, 0.0), 600.0);
    }

    #[test]
    fn test_default_anchors_reproduce_the_table() {
        // With the default anchors the stretch has to be a no-op: the curve
        // must still be the plain table shifted to the centre frequency.
        let c = 500.0_f32;
        for v in [0.0_f32, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0] {
            let plain = lerp_table(STEFLY_CLIMB_FREQ, v) * c / STEFLY_CENTER_FREQ;
            let got = stefly_climb_freq(v, c, 1930.0, 5.0);
            assert!(
                (got - plain).abs() < plain * 0.005,
                "v={} plain={} got={}",
                v,
                plain,
                got
            );
        }
        for v in [0.0_f32, -1.0, -3.0, -5.0, -8.0] {
            let plain = lerp_table(STEFLY_SINK_FREQ, v) * c / STEFLY_CENTER_FREQ;
            let got = stefly_sink_freq(v, c, 318.0, 5.0);
            assert!(
                (got - plain).abs() < plain * 0.005,
                "v={} plain={} got={}",
                v,
                plain,
                got
            );
        }
    }

    #[test]
    fn test_anchors_are_hit_exactly() {
        let c = 500.0_f32;
        assert!((stefly_climb_freq(0.0, c, 1200.0, 4.0) - c).abs() < 1.0);
        assert!((stefly_climb_freq(4.0, c, 1200.0, 4.0) - 1200.0).abs() < 5.0);
        assert!((stefly_sink_freq(-4.0, c, 380.0, 4.0) - 380.0).abs() < 3.0);
        // degenerate settings must not blow up
        assert!(stefly_climb_freq(2.0, c, 0.0, 0.0).is_finite());
        assert!(stefly_sink_freq(-2.0, 0.0, 380.0, 4.0).is_finite());
    }

    #[test]
    fn test_anchor_migration() {
        let (upper, lower) = anchors_from_spreading(659.0, 1.0, 5.0);
        assert!((upper - 1318.0).abs() < 15.0, "upper={}", upper);
        assert!((lower - 330.0).abs() < 5.0, "lower={}", lower);
    }

    #[test]
    fn test_double_beep_down() {
        // base tone gets deeper with excess and never falls below 180 Hz
        assert!((double_beep_down_freq(0.0, 500.0) - 320.0).abs() < 3.0);
        // the anchor follows the center frequency
        assert!((double_beep_down_freq(0.0, 700.0) - 448.0).abs() < 5.0);
        // 320 * 2^-1 = 160 Hz would put the second tone below the hardware limit
        assert_float_eq!(
            double_beep_down_freq(4.0, 500.0),
            DBD_MIN_FREQ * MAJOR_THIRD
        );
        assert!(double_beep_down_freq(20.0, 500.0) / MAJOR_THIRD >= DBD_MIN_FREQ - 0.5);
        assert!(double_beep_down_freq(2.0, 500.0) < double_beep_down_freq(1.0, 500.0));
        // tones get shorter and the pause gets tighter
        assert_float_eq!(double_beep_down_len_factor(1.0), 0.8);
        assert_float_eq!(double_beep_down_pause_ms(1.0), 450.0);
        assert_float_eq!(double_beep_down_pause_ms(0.0), 900.0);
    }

    #[test]
    fn test_double_beep_up() {
        // pause shrinks with excess but stops at 180 ms
        assert_float_eq!(double_beep_up_pause_ms(0.0), 800.0);
        assert_float_eq!(double_beep_up_pause_ms(2.0), 520.0);
        assert_float_eq!(double_beep_up_pause_ms(10.0), 180.0);
        // base tone comes from the climb curve at 0.6 * excess
        assert_float_eq!(double_beep_up_freq(0.0, 350.0, 1930.0, 5.0), 350.0);
        assert!(
            double_beep_up_freq(3.0, 350.0, 1930.0, 5.0)
                > double_beep_up_freq(1.0, 350.0, 1930.0, 5.0)
        );
    }

    #[test]
    fn test_loudness_and_speed_volume() {
        assert_eq!(loudness_comp_db(400.0, 1.0), 0.0);
        assert_float_eq!(loudness_comp_db(1000.0, 1.0), -1.5);
        assert_float_eq!(loudness_comp_db(1000.0, 1.5), -2.25);
        assert_float_eq!(loudness_comp_db(3000.0, 1.0), -4.0);
        assert_eq!(loudness_comp_db(1000.0, 0.0), 0.0);

        assert_eq!(speed_volume_db(90.0, 100.0, 1.0), 0.0);
        assert_float_eq!(speed_volume_db(150.0, 100.0, 1.0), 5.0);
        assert_float_eq!(speed_volume_db(300.0, 100.0, 3.0), SPEED_VOLUME_MAX_DB);
        assert_eq!(speed_volume_db(200.0, 100.0, 0.0), 0.0);

        // -6 dB is half the amplitude
        assert!((db_to_factor(-6.0) - 0.5).abs() < 0.01);
        assert!((db_to_factor(0.0) - 1.0).abs() < 0.01);
    }
}
