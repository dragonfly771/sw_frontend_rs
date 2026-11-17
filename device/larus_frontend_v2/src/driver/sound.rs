use super::H7_HCLK;
use crate::{utils::samples::*, SoundParams, Waveform};
/// Sound Modul
///
/// The sound module provides the necessary components to realize the sound output of the Vario.
/// This is implemented as follows:
/// - A timer cyclically generates events that are passed on to the digital-to-analog converter.
///   The sound frequency of the output is set by adjusting the timer start value.
/// - The digital-to-analog converter outputs the currently specified amplitude value and signals
///   to the DMA that the next value is required.
/// - The DMA copies the next amplitude value from a waveform table to the DAC. The DMA works
///   cyclically and sends an interrupt request after each cycle.
/// - The interrupt service routine checks whether changes to the sound are pending. The sound
///   signal could be interrupted (continuous / not continuous), it could be switched off (mute)
///   or a new waveform could be specified.
///
/// Great importance was attached to ensuring that acoustically relevant switching operations are
/// always carried out at the zero crossing of the oscillation, so that no cracking noises are
/// generated. This applies to non-continuous tones, mute function and switching the desired
/// waveform.
///
/// Switching at the zero crossing removes the step in the signal, but not the kink: a tone that
/// starts and stops abruptly still spreads energy over the whole spectrum and is heard as a hard
/// edge. The output therefore runs through an exponential attack/release envelope, matching the
/// tone lab (attack = the configured time, release twice as long). The envelope is interpolated
/// across the samples of one period, so even the lowest tones get a real ramp instead of a
/// single step.
///
/// In this module, unsafe is used several times to access the controller peripherals. This is
/// unavoidable, necessary and tests did not reveal any problems.
use stm32h7xx_hal::{
    dac::{Disabled, C1},
    dma::dma::Stream0,
    pac,
    pac::{DAC, DMA1},
};

#[link_section = ".axisram.AXISRAM"]
static mut SAMPLES: [u16; SAMPLES_COUNT] = [2047; SAMPLES_COUNT];
const SILENCE: [u16; SAMPLES_COUNT] = [2047; SAMPLES_COUNT];

/// Lowest frequency the DAC timer can serve.
///
/// The reload value of the timer is a u16, so it overflows below
/// H7_HCLK / SAMPLES_COUNT / u16::MAX. A little head room is added on top.
const MIN_FREQ: f32 = (H7_HCLK / SAMPLES_COUNT as u32 / u16::MAX as u32) as f32 + 10.0;
/// Highest frequency handed to the DAC timer (covers the whole configurable range)
const MAX_FREQ: f32 = 3100.0;

#[allow(unused)]
pub struct Sound {
    dac: C1<DAC, Disabled>,
    tim: pac::TIM6,
    dma1_stream0: Stream0<DMA1>,
    cycle_counter: u16,
    duty_cycle: u16,
    wave_ctr: u32,
    next_f: f32,
    curr_f: f32,
    delta_f: f32,
    /// should the tone sound in the current period?
    on: bool,
    /// current value of the attack/release envelope, 0.0 .. 1.0
    env: f32,
    /// the sample buffer has to be written to the DMA area again
    dirty: bool,
    params: SoundParams,
    /// waveform scaled by volume and amplitude, without the DAC offset
    buffer: [f32; SAMPLES_COUNT],
}

/// Below this distance the envelope is snapped onto its target
const ENV_EPSILON: f32 = 0.01;

impl Sound {
    pub fn new(dac: C1<DAC, Disabled>, tim: pac::TIM6, dma1_stream0: Stream0<DMA1>) -> Self {
        unsafe {
            tim.cr2.write(|w| w.mms().bits(0b010)); // update event trigger output
            tim.arr.write(|w| w.arr().bits(11363)); // preload register
            tim.cr1.modify(|_, w| w.cen().set_bit()); // start timer

            let dac1 = &(*pac::DAC::ptr());
            dac1.mcr.modify(|_, w| w.mode1().bits(0));
            // CR 0x3    0    1    7
            //    0b0011_0000_0001_0111
            // Bit 0    en1 enable
            // Bit 1    ten1 enable trigger enable
            // Bit 5:2  tsel1 trigger select 00101 dac1_ch1_trig5 (Tim6_tgo)
            // Bit 12   dmaen1 DAC chan1 DMA enable
            // Bit 13   dmaudrie1 DAC chan1 DMA underrun ie
            dac1.cr.modify(|_, w| w.ten1().set_bit());
            dac1.cr.modify(|_, w| w.tsel1().bits(5));
            dac1.cr.modify(|_, w| w.dmaen1().set_bit());
            dac1.cr.modify(|_, w| w.dmaudrie1().set_bit());
            dac1.cr.modify(|_, w| w.en1().set_bit());

            let dmamux1 = &(*pac::DMAMUX1::ptr());
            dmamux1.ccr[0].write(|w| w.dmareq_id().bits(0x43));

            let dma1 = &(*pac::DMA1::ptr());
            dma1.st[0].ndtr.write(|w| w.bits(SAMPLES_COUNT as u32)); // cnt dma
            dma1.st[0].par.write(|w| w.bits(0x4000_7408)); // dst 12 Bit DAC register
            dma1.st[0].m0ar.write(|w| w.bits(&raw const SAMPLES as u32));
            // Bit 4     transfer complete ie
            // Bit 7:6   0b01 memory to peripheral
            // Bit 8     circular mode
            // Bit 10    memory increment after transfer
            // Bit 12:11 0b01 peripheral data size 16 Bit
            // Bit 14:13 0b01 memory data size 16 Bit
            dma1.st[0].cr.write(|w| w.bits(0b0010_1101_0101_0000));
            dma1.st[0].cr.modify(|_, w| w.en().set_bit()); // start dma transfer
        }
        Sound {
            dac,
            tim,
            dma1_stream0,
            cycle_counter: 0,
            duty_cycle: 120,
            wave_ctr: 0,
            next_f: 1000.0,
            curr_f: 1000.0,
            delta_f: 0.0,
            on: false,
            env: 0.0,
            dirty: true,
            params: SoundParams::default(),
            buffer: [0.0; SAMPLES_COUNT],
        }
    }

    pub fn set_duty_cycle(&mut self, duty_cycle: u16) {
        self.duty_cycle = duty_cycle;
    }

    pub fn set_params(&mut self, params: &SoundParams) {
        self.params = *params;
        let devider = self.tim.arr.read().bits();
        self.wave_ctr = 0;
        self.curr_f = H7_HCLK as f32 / SAMPLES_COUNT as f32 / devider as f32;
        self.next_f = self.params.frequency as f32;
        if self.params.glide {
            // Calculate delta frequency asume 10 Hz tick rate
            self.delta_f = (self.next_f - self.curr_f) / self.curr_f * 10.0;
        } else {
            // Pattern tones take their pitch at once. The timer reload is written in the
            // interrupt, so the change still lands on a period boundary and stays phase clean.
            self.curr_f = self.next_f;
            self.delta_f = 0.0;
        }

        // get source pointer of sound samples
        let wave = match self.params.waveform {
            Waveform::Triangular => TRIANGULAR_WAVE,
            Waveform::Sawtooth => SAWTOOTH_WAVE,
            Waveform::Rectangular => RECTANGULAR_WAVE,
            Waveform::SineWave => SINE_WAVE,
        };

        // get volume_factor
        let volume_factor = if self.params.gain > 30 && self.params.gain <= 50 {
            VOLUME_FACTORS[self.params.gain as usize - 31] // 31..
        } else {
            0.5
        };

        // `amplitude` carries the base level, the loudness compensation and the airspeed
        // dependent volume; it is calculated in the core.
        let factor = volume_factor * self.params.amplitude;
        for idx in 0..(SAMPLES_COUNT) {
            self.buffer[idx] = wave[idx] * factor;
        }
        self.dirty = true;
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        let params = SoundParams {
            waveform,
            ..self.params
        };
        self.set_params(&params);
    }

    pub fn waveform(&self) -> Waveform {
        self.params.waveform
    }

    pub fn on_interrupt(&mut self) {
        // we have accept the dam interrupt request, so unsafe is necessary here
        let dma1 = unsafe { &(*pac::DMA1::ptr()) };
        dma1.lifcr.write(|w| w.ctcif0().set_bit());

        // calc next frequency
        self.curr_f += self.delta_f;
        self.wave_ctr += 1;
        if self.curr_f > MIN_FREQ && self.curr_f < MAX_FREQ {
            let devider = (H7_HCLK / SAMPLES_COUNT as u32 / self.curr_f as u32) as u16;
            self.tim.arr.write(|w| w.arr().bits(devider)); // preload register
        }

        self.cycle_counter += 1;

        // Decide whether the tone should sound in the period that is starting now.
        let sound_on = if self.params.gain == 0 {
            // muted
            self.cycle_counter = 0;
            false
        } else if self.params.continuous {
            // The driver does not gate on its own here. Either the tone is continuous (classic
            // sink tone, gate is always true) or the beep pattern comes from the sequencer in
            // the core, which toggles `gate`.
            self.cycle_counter = 0;
            self.params.gate
        } else if self.cycle_counter >= self.duty_cycle {
            // classic beep: the driver counts full oscillations and toggles the tone itself
            self.cycle_counter = 0;
            !self.on
        } else {
            self.on
        };
        self.on = sound_on;

        let target = if sound_on { 1.0 } else { 0.0 };
        let env = self.env;
        if env != target {
            let mut next = env + (target - env) * self.env_factor();
            if (target - next) < ENV_EPSILON && (next - target) < ENV_EPSILON {
                next = target;
            }
            self.write_samples(env, next);
            self.env = next;
            self.dirty = false;
        } else if self.dirty {
            self.write_samples(env, env);
            self.dirty = false;
        }
    }

    /// Step of the exponential envelope for one period of the tone
    ///
    /// The tone lab uses a time constant of attack/3 for the attack and twice that for the
    /// release, so the envelope has reached about 95 % after the configured time.
    fn env_factor(&self) -> f32 {
        let attack_ms = if self.params.attack_ms < 0.5 {
            0.5
        } else {
            self.params.attack_ms
        };
        let time_const_ms = if self.on {
            attack_ms / 3.0
        } else {
            2.0 * attack_ms / 3.0
        };
        // one period of the tone currently being played
        let frequency = if self.curr_f < MIN_FREQ {
            MIN_FREQ
        } else {
            self.curr_f
        };
        let period_ms = 1000.0 / frequency;
        let factor = period_ms / time_const_ms;
        if factor > 1.0 {
            1.0
        } else {
            factor
        }
    }

    /// Write one period into the DMA area, interpolating the envelope across the samples
    ///
    /// Doing the interpolation inside the period matters: the interrupt only fires once per
    /// oscillation, so without it the shortest possible ramp would be one whole period - almost
    /// 3 ms at the 350 Hz zero tone, which is exactly the hard edge we want to get rid of.
    fn write_samples(&mut self, from_env: f32, to_env: f32) {
        if from_env <= 0.0 && to_env <= 0.0 {
            // unsafe is ok here because the access is synchronized to the dma access
            unsafe {
                SAMPLES = SILENCE;
            }
            return;
        }
        let step = (to_env - from_env) / SAMPLES_COUNT as f32;
        let mut env = from_env;
        let mut period = [2047_u16; SAMPLES_COUNT];
        for idx in 0..(SAMPLES_COUNT) {
            env += step;
            period[idx] = (2047.0 + self.buffer[idx] * env) as u16;
        }
        // unsafe is ok here because the access is synchronized to the dma access
        unsafe {
            SAMPLES = period;
        }
    }
}
