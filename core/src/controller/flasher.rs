//! SteFly canopy flasher (plan_flarm_blitzer.md)
//!
//! Logic 1:1 from the SteFly Control Box firmware 2.0.33
//! (`Controlbox-FW-2.0.4.ino`): the PIC in the flasher reads a 20 Hz PWM
//! signal, the duty cycle selects the flash sequence 1..9, a constant LOW
//! line is "off".
//!
//! | Flash Rate | level 0 | 1 | 2 | 3 |
//! |------------|---------|---|---|---|
//! | Low        | 1       | 2 | 3 | 4 |
//! | Medium     | 5       | 6 | 7 | 8 |
//! | High       | 6       | 7 | 8 | 9 |
//!
//! "Airborne": FLARM reports airborne (PFLAU GPS = 2) -> sequence of the
//! alarm level, also level 0. "Warning only": airborne and level > 0.
//! No PFLAU for 5 s -> off. Test: 30 s with level 3 of the chosen rate.
//! New: "Airspeed" mode without FLARM, level 0 sequence above the "On Speed".
//!
//! No dependencies on the rest of the crate except the settings enums.

use crate::model::flarm_cfg::{FlashRate, FlasherActive, FlasherType};

/// PWM period of the Control Box (LEDC 20 Hz)
pub const FLASH_PERIOD_US: u32 = 50_000;
/// no PFLAU for this long -> off (Control Box blinkTimeout)
pub const PFLAU_TIMEOUT_MS: i64 = 5_000;
/// duration of the flash test
pub const TEST_MS: i64 = 30_000;
/// hysteresis of the Airspeed mode
const SPEED_HYST_KMH: f32 = 5.0;

/// sequence for an alarm level
pub fn sequence(rate: FlashRate, level: u8) -> u8 {
    let base = match rate {
        FlashRate::Low => 1,
        FlashRate::Medium => 5,
        FlashRate::High => 6,
    };
    base + level.min(3)
}

/// Time the ESP32 pin of the Control Box is HIGH within one period, in µs.
///
/// `dn = map(n, 1, 9, 26, 230)` (Arduino integer map), the LEDC duty is
/// `di = 255 - dn` steps of 256 per period. Sequence 0 = always LOW (0),
/// sequence 10 = always HIGH (whole period).
pub fn high_time_us(seq: u8) -> u32 {
    match seq {
        0 => 0,
        1..=9 => {
            let n = seq as u32;
            let dn = (n - 1) * (230 - 26) / (9 - 1) + 26;
            let di = 255 - dn;
            di * FLASH_PERIOD_US / 256
        }
        _ => FLASH_PERIOD_US,
    }
}

/// state of the flasher logic
#[derive(Clone, Copy, Debug, Default)]
pub struct Flasher {
    /// end of a running test
    test_until: Option<i64>,
    /// airspeed mode: currently on (for the hysteresis)
    speed_on: bool,
    /// last output
    seq: u8,
}

/// FLARM status the flasher works with (only REAL FLARM data, never the demo)
#[derive(Clone, Copy, Debug, Default)]
pub struct FlarmStatusInput {
    /// time of the last PFLAU sentence
    pub t_pflau: Option<i64>,
    /// PFLAU GPS field: 0 no fix, 1 on ground, 2 airborne
    pub gps: u8,
    /// PFLAU alarm level 0..3
    pub alarm_level: u8,
}

impl Flasher {
    pub const fn new() -> Self {
        Flasher {
            test_until: None,
            speed_on: false,
            seq: 0,
        }
    }

    pub fn start_test(&mut self, now_ms: i64) {
        self.test_until = Some(now_ms.saturating_add(TEST_MS));
    }

    pub fn test_running(&self, now_ms: i64) -> bool {
        matches!(self.test_until, Some(t) if now_ms < t)
    }

    /// current output sequence (0 = off)
    pub fn seq(&self) -> u8 {
        self.seq
    }

    /// Recalculate the output. Returns Some(sequence) when it changed.
    pub fn update(
        &mut self,
        flasher_type: FlasherType,
        active: FlasherActive,
        rate: FlashRate,
        on_speed_kmh: f32,
        flarm: FlarmStatusInput,
        ias_kmh: f32,
        now_ms: i64,
    ) -> Option<u8> {
        let seq = self.calc(
            flasher_type,
            active,
            rate,
            on_speed_kmh,
            flarm,
            ias_kmh,
            now_ms,
        );
        if seq != self.seq {
            self.seq = seq;
            Some(seq)
        } else {
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn calc(
        &mut self,
        flasher_type: FlasherType,
        active: FlasherActive,
        rate: FlashRate,
        on_speed_kmh: f32,
        flarm: FlarmStatusInput,
        ias_kmh: f32,
        now_ms: i64,
    ) -> u8 {
        if self.test_running(now_ms) {
            // the test works in every mode except Off
            return if flasher_type == FlasherType::Off {
                0
            } else {
                sequence(rate, 3)
            };
        }
        self.test_until = None;
        match flasher_type {
            FlasherType::Off => 0,
            FlasherType::Flarm => {
                let fresh = match flarm.t_pflau {
                    Some(t) => now_ms.saturating_sub(t) <= PFLAU_TIMEOUT_MS,
                    None => false,
                };
                if !fresh || flarm.gps != 2 {
                    return 0;
                }
                match active {
                    FlasherActive::Airborne => sequence(rate, flarm.alarm_level),
                    FlasherActive::WarningOnly => {
                        if flarm.alarm_level > 0 {
                            sequence(rate, flarm.alarm_level)
                        } else {
                            0
                        }
                    }
                }
            }
            FlasherType::Airspeed => {
                if self.speed_on {
                    if ias_kmh < on_speed_kmh - SPEED_HYST_KMH {
                        self.speed_on = false;
                    }
                } else if ias_kmh >= on_speed_kmh {
                    self.speed_on = true;
                }
                if self.speed_on {
                    sequence(rate, 0)
                } else {
                    0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pwm_times_match_the_control_box() {
        // HIGH times in ms from the Arduino code (plan_flarm_blitzer.md)
        let expect = [44.73, 39.84, 34.77, 29.88, 24.80, 19.92, 14.84, 9.96, 4.88];
        for (i, e) in expect.iter().enumerate() {
            let us = high_time_us(i as u8 + 1) as f32;
            assert!(
                (us / 1000.0 - e).abs() < 0.02,
                "seq {} {} ms",
                i + 1,
                us / 1000.0
            );
        }
        assert_eq!(high_time_us(0), 0);
        assert_eq!(high_time_us(10), FLASH_PERIOD_US);
    }

    #[test]
    fn control_box_table() {
        assert_eq!(sequence(FlashRate::Low, 0), 1);
        assert_eq!(sequence(FlashRate::Low, 3), 4);
        assert_eq!(sequence(FlashRate::Medium, 0), 5);
        assert_eq!(sequence(FlashRate::Medium, 3), 8);
        assert_eq!(sequence(FlashRate::High, 0), 6);
        assert_eq!(sequence(FlashRate::High, 3), 9);
    }

    #[test]
    fn flarm_modes_timeout_and_test() {
        let mut f = Flasher::new();
        let st = |gps, lvl, t| FlarmStatusInput {
            t_pflau: Some(t),
            gps,
            alarm_level: lvl,
        };
        let m = FlashRate::Medium;
        // on ground: off
        assert_eq!(
            f.update(
                FlasherType::Flarm,
                FlasherActive::Airborne,
                m,
                40.0,
                st(1, 0, 0),
                0.0,
                100
            ),
            None
        );
        // airborne: level 0 pattern 5
        assert_eq!(
            f.update(
                FlasherType::Flarm,
                FlasherActive::Airborne,
                m,
                40.0,
                st(2, 0, 0),
                0.0,
                200
            ),
            Some(5)
        );
        // warning level 2 -> 7
        assert_eq!(
            f.update(
                FlasherType::Flarm,
                FlasherActive::Airborne,
                m,
                40.0,
                st(2, 2, 300),
                0.0,
                300
            ),
            Some(7)
        );
        // warning only, level 0 -> off
        assert_eq!(
            f.update(
                FlasherType::Flarm,
                FlasherActive::WarningOnly,
                m,
                40.0,
                st(2, 0, 400),
                0.0,
                400
            ),
            Some(0)
        );
        // no PFLAU for more than 5 s -> off
        f.update(
            FlasherType::Flarm,
            FlasherActive::Airborne,
            m,
            40.0,
            st(2, 1, 1000),
            0.0,
            1000,
        );
        assert_eq!(f.seq(), 6);
        f.update(
            FlasherType::Flarm,
            FlasherActive::Airborne,
            m,
            40.0,
            st(2, 1, 1000),
            0.0,
            6_100,
        );
        assert_eq!(f.seq(), 0);
        // test 30 s with level 3
        f.start_test(10_000);
        f.update(
            FlasherType::Flarm,
            FlasherActive::Airborne,
            FlashRate::High,
            40.0,
            FlarmStatusInput::default(),
            0.0,
            10_000,
        );
        assert_eq!(f.seq(), 9);
        f.update(
            FlasherType::Flarm,
            FlasherActive::Airborne,
            FlashRate::High,
            40.0,
            FlarmStatusInput::default(),
            0.0,
            40_001,
        );
        assert_eq!(f.seq(), 0);
    }

    #[test]
    fn airspeed_mode_with_hysteresis() {
        let mut f = Flasher::new();
        let none = FlarmStatusInput::default();
        let r = FlashRate::Low;
        f.update(
            FlasherType::Airspeed,
            FlasherActive::Airborne,
            r,
            40.0,
            none,
            39.0,
            0,
        );
        assert_eq!(f.seq(), 0);
        f.update(
            FlasherType::Airspeed,
            FlasherActive::Airborne,
            r,
            40.0,
            none,
            41.0,
            0,
        );
        assert_eq!(f.seq(), 1);
        f.update(
            FlasherType::Airspeed,
            FlasherActive::Airborne,
            r,
            40.0,
            none,
            36.0,
            0,
        );
        assert_eq!(f.seq(), 1, "hysteresis");
        f.update(
            FlasherType::Airspeed,
            FlasherActive::Airborne,
            r,
            40.0,
            none,
            34.0,
            0,
        );
        assert_eq!(f.seq(), 0);
    }
}
