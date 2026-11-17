use crate::PinState;
#[allow(unused_imports)]
use micromath::F32Ext;

/// samples of the supply voltage the median is taken of (10 Hz: one second)
const SUPPLY_WINDOW: usize = 10;
/// the shown voltage follows the filter only when it moved this much / V -
/// a value at a 0.1 V boundary does not toggle the last digit
const SUPPLY_HYSTERESIS: f32 = 0.06;

/// Smoothing of the supply voltage (Stefan 09.11.2026: the battery box
/// jumps, the canopy flasher pulls the voltage down in short pulses):
/// median of the last second - short dips do not reach it - then a PT1 and
/// a small hysteresis for the shown value.
#[derive(Clone, Copy, Default)]
pub struct SupplyFilter {
    buf: [f32; SUPPLY_WINDOW],
    n: u8,
    idx: u8,
    pt1: f32,
}

impl SupplyFilter {
    fn median(&self) -> f32 {
        let n = self.n as usize;
        let mut v = self.buf;
        let v = &mut v[..n];
        // insertion sort, ten values at most
        for i in 1..n {
            let mut j = i;
            while j > 0 && v[j - 1] > v[j] {
                v.swap(j - 1, j);
                j -= 1;
            }
        }
        if n % 2 == 1 {
            v[n / 2]
        } else {
            (v[n / 2 - 1] + v[n / 2]) * 0.5
        }
    }
}

#[derive(Clone, Copy)]
pub struct Device {
    /// supply voltage as shown and used (filtered, see `supply_sample`) / V
    pub supply_voltage: f32,
    pub illumination_voltage: f32,
    pub temperature_pcb: f32,
    pub out1: PinState,
    pub out2: PinState,
    pub supply_filter: SupplyFilter,
}

impl Default for Device {
    fn default() -> Self {
        Device {
            supply_voltage: 13.0,
            illumination_voltage: 0.0,
            temperature_pcb: 0.0,
            out1: PinState::High,
            out2: PinState::High,
            supply_filter: SupplyFilter::default(),
        }
    }
}

impl Device {
    /// A new reading of the supply voltage (every 100 ms), `tc_s` time
    /// constant of the PT1 (config `av_supply_voltage_tc`)
    pub fn supply_sample(&mut self, raw: f32, tc_s: f32) {
        const DT: f32 = 0.1;
        let f = &mut self.supply_filter;
        f.buf[f.idx as usize] = raw;
        f.idx = ((f.idx as usize + 1) % SUPPLY_WINDOW) as u8;
        let first = f.n == 0;
        if (f.n as usize) < SUPPLY_WINDOW {
            f.n += 1;
        }
        let med = f.median();
        if (f.n as usize) < SUPPLY_WINDOW {
            // the first second: no history to smooth with yet
            f.pt1 = med;
        } else {
            f.pt1 += (med - f.pt1) * DT / (tc_s.max(0.0) + DT);
        }
        let pt1 = f.pt1;
        if first || (pt1 - self.supply_voltage).abs() >= SUPPLY_HYSTERESIS {
            self.supply_voltage = pt1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(d: &mut Device, secs: f32, sample: impl Fn(usize) -> f32) -> (f32, f32) {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..(secs * 10.0) as usize {
            d.supply_sample(sample(i), 3.0);
            lo = lo.min(d.supply_voltage);
            hi = hi.max(d.supply_voltage);
        }
        (lo, hi)
    }

    /// The canopy flasher pulls the voltage down by 1.5 V in every fourth
    /// sample - the shown value does not move.
    #[test]
    fn flasher_dips_do_not_reach_the_shown_voltage() {
        let mut d = Device::default();
        run(&mut d, 5.0, |_| 12.6);
        let (lo, hi) = run(&mut d, 30.0, |i| if i % 4 == 0 { 11.1 } else { 12.6 });
        assert!(lo > 12.55 && hi < 12.65, "shown {lo} .. {hi}");
    }

    /// ADC noise of +-0.08 V at a 0.1 V boundary: the last digit stays.
    #[test]
    fn noise_at_a_digit_boundary_does_not_toggle() {
        let mut d = Device::default();
        let noise =
            |i: usize| 12.45 + if i % 2 == 0 { 0.08 } else { -0.08 } + (i % 7) as f32 * 0.01 - 0.03;
        run(&mut d, 5.0, noise);
        let shown = std::format!("{:.1}", d.supply_voltage);
        for i in 50..600 {
            d.supply_sample(noise(i), 3.0);
            assert_eq!(std::format!("{:.1}", d.supply_voltage), shown, "sample {i}");
        }
    }

    /// A real change (switch to the second battery) shows within seconds,
    /// the first reading at once.
    #[test]
    fn real_change_follows_within_seconds() {
        let mut d = Device::default();
        d.supply_sample(12.2, 3.0);
        assert!(
            (d.supply_voltage - 12.2).abs() < 0.01,
            "first reading at once"
        );
        run(&mut d, 5.0, |_| 12.2);
        run(&mut d, 10.0, |_| 13.1);
        assert!(
            (d.supply_voltage - 13.1).abs() < 0.1,
            "after 10 s: {}",
            d.supply_voltage
        );
    }
}
