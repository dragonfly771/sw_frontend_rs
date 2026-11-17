use crate::model::{SINK_MODE_DOUBLE_BEEP, SINK_MODE_OFF};
use crate::{
    controller::tone_profile::{
        db_to_factor, double_beep_down_freq, double_beep_down_len_factor,
        double_beep_down_pause_ms, double_beep_up_freq, double_beep_up_pause_ms, loudness_comp_db,
        speed_volume_db, stefly_beep_duty, stefly_beep_period_ms, stefly_climb_freq,
        stefly_sink_freq, stefly_tick_pause_ms, DBD_GAP_MS, DBD_TONE1_MS, DBD_TONE2_MS, DBU_GAP_MS,
        DBU_TONE1_MS, DBU_TONE2_MS, MAJOR_THIRD, STEFLY_MAX_FREQ, STEFLY_MIN_FREQ,
        STEFLY_TICK_LEN_MS,
    },
    CoreModel, IdleEvent, VarioMode,
};
use num::clamp;
use num_enum::FromPrimitive;

#[allow(unused_imports)]
use micromath::F32Ext;

pub const SND_EXP_MUL: f32 = 0.138629; // -5 .. 5 two octaves

/// Base amplitude of the DAC output, 0.0 .. 1.0
///
/// The waveform tables are scaled with this before they go to the DAC. 0.5 keeps the loudness
/// of the original firmware: back then the negative half wave was clipped away by a saturating
/// cast, so the fundamental only ever reached half the amplitude. With the full wave restored,
/// half the amplitude gives the same fundamental - and leaves 6 dB of head room for the
/// airspeed dependent volume.
pub const SND_BASE_AMPLITUDE: f32 = 0.5;

/// Tick period of the beep sequencer in ms
pub const SEQ_TICK_MS: u16 = 10;
/// Tick rate of the beep sequencer in Hz
const SEQ_TICK_RATE: f32 = 1000.0 / SEQ_TICK_MS as f32;
/// Re-evaluation period of the states that hold a steady tone (silence, continuous sink)
const SEQ_STEADY_MS: f32 = 100.0;
/// Filter factor of the airspeed used for the airspeed dependent volume (tc = 2 s)
const IAS_FILTER_FACTOR: f32 = 1.0 / (2.0 * SEQ_TICK_RATE);

#[derive(Clone, Copy, PartialEq, Debug, FromPrimitive)]
#[repr(u8)]
pub enum Waveform {
    #[default]
    Triangular,
    Sawtooth,
    Rectangular,
    SineWave,
}

#[allow(unused)]
pub const WAVEFORM_TRIANGULAR: &str = "Triangular";
pub const WAVEFORM_SAWTOOTH: &str = "Sawtooth";
pub const WAVEFORM_RECTANGULAR: &str = "Rectangular";
pub const WAVEFORM_SINE_WAVE: &str = "Sine wave";

impl From<&str> for Waveform {
    fn from(value: &str) -> Self {
        match value {
            WAVEFORM_SAWTOOTH => Waveform::Sawtooth,
            WAVEFORM_RECTANGULAR => Waveform::Rectangular,
            WAVEFORM_SINE_WAVE => Waveform::SineWave,
            _ => Waveform::Triangular,
        }
    }
}

pub const SINK_TONE_CONTINUOUS: &str = "Continuous";
pub const SINK_TONE_DOUBLE_BEEP: &str = "Double Beep";
pub const SINK_TONE_OFF: &str = "Off";

#[derive(Clone, Copy)]
pub struct SoundParams {
    pub frequency: u16,
    /// `false`: the driver produces the beep rhythm itself by counting oscillations.
    /// `true`: the driver keeps the tone running and only follows `gate`.
    pub continuous: bool,
    /// Tone on/off, driven by the core sequencer. Always `true` for driver gated tones.
    pub gate: bool,
    pub gain: i8,
    /// Amplitude of the DAC output, 0.0 .. 1.0 - carries the loudness compensation and the
    /// airspeed dependent volume on top of [`SND_BASE_AMPLITUDE`].
    pub amplitude: f32,
    /// Attack time of the tone envelope in ms, the release is twice as long
    pub attack_ms: f32,
    /// Should the driver slide into the new pitch (`true`) or take it immediately (`false`)?
    ///
    /// The tone lab glides only where the pitch carries information that changes slowly - the
    /// climb tone and the continuous sink tone. Every tone of a *pattern* (zero zone ticks, both
    /// double beeps) gets its pitch as a step, otherwise the two tones of a double beep smear
    /// into one downward slide instead of sounding a major third apart.
    pub glide: bool,
    pub waveform: Waveform,
}

impl Default for SoundParams {
    fn default() -> Self {
        SoundParams {
            frequency: 500,
            continuous: false,
            gate: true,
            gain: 2,
            amplitude: SND_BASE_AMPLITUDE,
            attack_ms: 8.0,
            glide: true,
            waveform: Waveform::Triangular,
        }
    }
}

pub fn get_snd_spreading_factor(cm: &CoreModel) -> f32 {
    cm.config.snd_exp_mul / SND_EXP_MUL
}

/// Set the (legacy) spreading factor
///
/// The spreading factor is no longer a setting of its own - the shape of the tone curve comes
/// from the upper/lower anchor frequencies and the full scale value. It is still accepted over
/// NMEA and CAN and from old EEPROM contents, and converted into those anchors here.
pub fn set_snd_spreading_factor(cm: &mut CoreModel, spreading_factor: f32) {
    cm.config.snd_exp_mul = SND_EXP_MUL * spreading_factor;
    let (upper, lower) = crate::controller::tone_profile::anchors_from_spreading(
        cm.config.snd_center_freq,
        spreading_factor,
        cm.config.snd_full_scale,
    );
    cm.config.snd_freq_upper = upper;
    cm.config.snd_freq_lower = lower;
}

#[allow(unused)]
pub enum SoundScenario {
    Standard = 0b0000_0000,
    GearAlarm = 0b0000_1000,
}

impl core::ops::BitAnd<u8> for SoundScenario {
    type Output = bool;

    fn bitand(self, rhs: u8) -> Self::Output {
        self as u8 & rhs != 0
    }
}

/// States of the Stefly beep sequencer
///
/// `Off` means the sequencer is not in charge - the gear alarm lets the driver do the gating.
#[derive(Clone, Copy, PartialEq, Debug)]
enum SeqState {
    Off,
    ClimbOn,
    ClimbOff,
    TickBlip,
    TickPause,
    Silence,
    SinkCont,
    DblDownTone1,
    DblDownGap,
    DblDownTone2,
    DblDownPause,
    DblUpTone1,
    DblUpGap,
    DblUpTone2,
    DblUpPause,
}

/// One segment of the beep pattern: state, duration, frequency, tone on/off, glide into pitch
type Segment = (SeqState, f32, f32, bool, bool);

pub struct SoundControl {
    scenario: u8,
    tick: u16,
    /// climb rate after the audio damping low pass
    av_climb_rate: f32,
    /// indicated airspeed after a slow low pass, for the airspeed dependent volume
    av_ias_km_h: f32,
    state: SeqState,
    countdown_ms: i16,
    /// base frequency of the double beep currently being played
    beep_base: f32,
    /// tone length factor of the double beep currently being played
    beep_len_factor: f32,
    /// 10 ms ticks of the running sound preview (see `sound_preview`)
    preview_tick: u16,
}

impl Default for SoundControl {
    fn default() -> Self {
        SoundControl {
            scenario: SoundScenario::Standard as u8,
            tick: 0,
            av_climb_rate: 0.0,
            av_ias_km_h: 0.0,
            state: SeqState::Off,
            countdown_ms: 0,
            beep_base: 320.0,
            beep_len_factor: 1.0,
            preview_tick: 0,
        }
    }
}

impl SoundControl {
    pub fn activate_scenariio(&mut self, scenariio: SoundScenario) {
        self.scenario |= scenariio as u8;
    }

    pub fn clear_scenariio(&mut self, scenariio: SoundScenario) {
        self.scenario &= !(scenariio as u8);
    }

    /// a warning tone (gear alarm) is sounding
    pub fn alarm_active(&self) -> bool {
        SoundScenario::GearAlarm & self.scenario
    }

    pub fn set_scenario(&mut self, scenariio: SoundScenario, active: bool) {
        if active {
            self.activate_scenariio(scenariio);
        } else {
            self.clear_scenariio(scenariio);
        }
    }

    // is called every 100ms
    pub fn sound(&mut self, cm: &mut CoreModel) -> Option<IdleEvent> {
        let gain = if SoundScenario::GearAlarm & self.scenario {
            let (frequency, continuous, gain) = self.gear_alarm_sound(cm);
            self.set_driver_gated_params(cm, frequency, continuous);
            gain
        } else {
            self.tick = 0;
            // Frequency, continuous and gate belong to the 10 ms sequencer. Silence is
            // produced by the gate, so the amplifier gain simply stays at the user volume
            // - circling (vario) or cruise (speed to fly) volume
            match cm.control.vario_mode {
                VarioMode::SpeedToFly => cm.config.stf_volume(),
                VarioMode::Vario => cm.config.volume,
            }
        };

        self.update_amplitude(cm);

        if gain != cm.calculated.sound_params.gain {
            cm.calculated.sound_params.gain = gain;
            let event = IdleEvent::SetGain(gain as u8);

            // send event to the idle loop, which handles the amplifier via i2c
            Some(event)
        } else {
            None
        }
    }

    /// SOUND PREVIEW (Stefan 15.11.2026: hear what a sound setting does)
    /// lives in `sound_preview_wanted` / `sound_preview_climb` below.
    ///
    /// Beep sequencer of the Stefly profile, called every 10 ms
    ///
    /// Returns `true` when the sound parameters changed and have to be handed to the driver.
    pub fn tick_10ms(&mut self, cm: &mut CoreModel) -> bool {
        // audio damping: one pole low pass, takes the wobble out of the gliding tone
        let time_const = if cm.config.snd_audio_tc < 0.02 {
            0.02
        } else {
            cm.config.snd_audio_tc
        };
        let factor = clamp(1.0 / (time_const * SEQ_TICK_RATE), 0.0, 1.0);
        // needle and tone follow the same source: gust filtered climb rate
        // (equals sensor.climb_rate when the gust filter is off) plus the
        // entry optimized correction (0 in standard mode / outside the entry)
        let mut climb_src =
            cm.calculated.gust_filtered_climb.to_m_s() + cm.calculated.needle_correction.to_m_s();
        // sound preview: on the ground, with a sound setting open in the
        // editor, the tone runs through the whole range instead
        cm.calculated.sound_preview = if sound_preview_wanted(cm) {
            let v = sound_preview_climb(self.preview_tick);
            self.preview_tick = (self.preview_tick + 1) % PREVIEW_PERIOD_TICKS;
            climb_src = v;
            Some(v)
        } else {
            self.preview_tick = 0;
            None
        };
        self.av_climb_rate += (climb_src - self.av_climb_rate) * factor;

        // slow low pass on the airspeed - no pumping of the volume in gusts
        let ias = cm.sensor.airspeed.ias().to_km_h();
        self.av_ias_km_h += (ias - self.av_ias_km_h) * IAS_FILTER_FACTOR;

        // the gear alarm owns the tone for as long as it sounds
        if SoundScenario::GearAlarm & self.scenario {
            self.state = SeqState::Off;
            self.countdown_ms = 0;
            // the driver does the gating here, so the gate has to stay open
            if !cm.calculated.sound_params.gate {
                cm.calculated.sound_params.gate = true;
                return true;
            }
            return false;
        }

        // demo for the fair: the vario stays silent
        if cm.control.demo.active() {
            self.state = SeqState::Off;
            self.countdown_ms = 0;
            if cm.calculated.sound_params.gate {
                cm.calculated.sound_params.gate = false;
                return true;
            }
            return false;
        }

        self.countdown_ms -= SEQ_TICK_MS as i16;
        if self.countdown_ms > 0 {
            return false;
        }
        self.next_segment(cm)
    }

    /// Switch to the next segment of the beep pattern
    fn next_segment(&mut self, cm: &mut CoreModel) -> bool {
        // the preview is always the vario tone, whatever the mode switch says
        let mode = if cm.calculated.sound_preview.is_some() {
            VarioMode::Vario
        } else {
            cm.control.vario_mode
        };
        let (state, duration_ms, frequency, gate, glide) = match mode {
            VarioMode::Vario => self.next_vario_segment(cm),
            VarioMode::SpeedToFly => self.next_stf_segment(cm),
        };

        self.state = state;
        self.countdown_ms = clamp(duration_ms, SEQ_TICK_MS as f32, i16::MAX as f32) as i16;

        let frequency = clamp(frequency, STEFLY_MIN_FREQ, STEFLY_MAX_FREQ) as u16;
        let sound_params = &mut cm.calculated.sound_params;
        let changed = sound_params.frequency != frequency
            || sound_params.gate != gate
            || sound_params.glide != glide
            || !sound_params.continuous;
        sound_params.frequency = frequency;
        sound_params.gate = gate;
        sound_params.glide = glide;
        // the driver must not add its own duty gating on top of the sequencer
        sound_params.continuous = true;
        self.update_amplitude(cm);
        changed
    }

    /// Next segment in vario mode: climb beep, zero zone ticks, silence or sink tone
    fn next_vario_segment(&mut self, cm: &CoreModel) -> Segment {
        let climb_rate = self.av_climb_rate;
        let center_freq = cm.config.snd_center_freq;
        let ticks_until = cm.config.snd_ticks_until;
        let sink_from = cm.config.snd_sink_from;
        let last_freq = cm.calculated.sound_params.frequency as f32;

        if climb_rate > 0.0 {
            let tempo = clamp(cm.config.snd_beep_tempo, 0.4, 2.5);
            let period = stefly_beep_period_ms(climb_rate) / tempo;
            let duty = stefly_beep_duty(climb_rate);
            if self.state == SeqState::ClimbOn {
                // pause of the climb beep, keep the pitch of the beep just played
                (
                    SeqState::ClimbOff,
                    period * (1.0 - duty),
                    last_freq,
                    false,
                    true,
                )
            } else {
                // the climb tone glides: its pitch tracks a slowly changing climb rate
                (
                    SeqState::ClimbOn,
                    period * duty,
                    stefly_climb_freq(
                        climb_rate,
                        center_freq,
                        cm.config.snd_freq_upper,
                        cm.config.snd_full_scale,
                    ),
                    true,
                    true,
                )
            }
        } else if ticks_until < 0.0 && climb_rate > ticks_until {
            // zero zone: short blips at the zero tone frequency - the lift sniffer
            if self.state == SeqState::TickBlip {
                (
                    SeqState::TickPause,
                    stefly_tick_pause_ms(climb_rate, ticks_until),
                    last_freq,
                    false,
                    false,
                )
            } else {
                // a 35 ms blip has no time to glide - it takes its pitch at once
                (
                    SeqState::TickBlip,
                    STEFLY_TICK_LEN_MS,
                    stefly_climb_freq(
                        0.0,
                        center_freq,
                        cm.config.snd_freq_upper,
                        cm.config.snd_full_scale,
                    ),
                    true,
                    false,
                )
            }
        } else if climb_rate > sink_from || cm.config.snd_sink_mode == SINK_MODE_OFF {
            // silence zone - or the sink tone is switched off entirely
            (SeqState::Silence, SEQ_STEADY_MS, last_freq, false, false)
        } else if cm.config.snd_sink_mode == SINK_MODE_DOUBLE_BEEP {
            self.double_beep_down_segment(sink_from - climb_rate, center_freq, last_freq)
        } else {
            // the continuous sink tone glides, like the climb tone
            (
                SeqState::SinkCont,
                SEQ_STEADY_MS,
                stefly_sink_freq(
                    climb_rate,
                    center_freq,
                    cm.config.snd_freq_lower,
                    cm.config.snd_full_scale,
                ),
                true,
                true,
            )
        }
    }

    /// Next segment in speed to fly mode: double beep up = pull, double beep down = push
    fn next_stf_segment(&mut self, cm: &CoreModel) -> Segment {
        // positive means faster than the speed to fly, so the pilot has to pull
        let stf_dif = -cm.calculated.speed_to_fly_dif.to_km_h() / 10.0;
        let upper = cm.config.stf_upper_limit.to_km_h() / 10.0;
        let lower = cm.config.stf_lower_limit.to_km_h() / 10.0;
        let center_freq = cm.config.snd_center_freq;
        let last_freq = cm.calculated.sound_params.frequency as f32;

        if stf_dif > upper {
            // too fast - pull: major third upwards
            self.double_beep_up_segment(stf_dif - upper, cm, last_freq)
        } else if stf_dif < lower {
            // too slow - push: major third downwards, same language as the sink tone
            self.double_beep_down_segment(lower - stf_dif, center_freq, last_freq)
        } else {
            (SeqState::Silence, SEQ_STEADY_MS, last_freq, false, false)
        }
    }

    /// Descending double beep - sink tone option and speed to fly "push"
    fn double_beep_down_segment(
        &mut self,
        excess: f32,
        center_freq: f32,
        last_freq: f32,
    ) -> Segment {
        match self.state {
            SeqState::DblDownTone1 => (SeqState::DblDownGap, DBD_GAP_MS, last_freq, false, false),
            SeqState::DblDownGap => (
                SeqState::DblDownTone2,
                DBD_TONE2_MS * self.beep_len_factor,
                self.beep_base / MAJOR_THIRD,
                true,
                false,
            ),
            SeqState::DblDownTone2 => (
                SeqState::DblDownPause,
                double_beep_down_pause_ms(excess),
                last_freq,
                false,
                false,
            ),
            _ => {
                // start a new pattern - the strength is sampled once here, so the two tones of
                // one pattern always belong together
                self.beep_base = double_beep_down_freq(excess, center_freq);
                self.beep_len_factor = double_beep_down_len_factor(excess);
                (
                    SeqState::DblDownTone1,
                    DBD_TONE1_MS * self.beep_len_factor,
                    self.beep_base,
                    true,
                    false,
                )
            }
        }
    }

    /// Rising double beep - speed to fly "pull"
    fn double_beep_up_segment(&mut self, excess: f32, cm: &CoreModel, last_freq: f32) -> Segment {
        match self.state {
            SeqState::DblUpTone1 => (SeqState::DblUpGap, DBU_GAP_MS, last_freq, false, false),
            SeqState::DblUpGap => (
                SeqState::DblUpTone2,
                DBU_TONE2_MS,
                self.beep_base * MAJOR_THIRD,
                true,
                false,
            ),
            SeqState::DblUpTone2 => (
                SeqState::DblUpPause,
                double_beep_up_pause_ms(excess),
                last_freq,
                false,
                false,
            ),
            _ => {
                self.beep_base = double_beep_up_freq(
                    excess,
                    cm.config.snd_center_freq,
                    cm.config.snd_freq_upper,
                    cm.config.snd_full_scale,
                );
                (
                    SeqState::DblUpTone1,
                    DBU_TONE1_MS,
                    self.beep_base,
                    true,
                    false,
                )
            }
        }
    }

    /// Recalculate the DAC amplitude from the current pitch and airspeed
    fn update_amplitude(&self, cm: &mut CoreModel) {
        let frequency = cm.calculated.sound_params.frequency as f32;
        let comp_db = loudness_comp_db(frequency, cm.config.snd_loudness_comp);
        let speed_db = speed_volume_db(
            self.av_ias_km_h,
            cm.config.snd_speed_vol_ref,
            cm.config.snd_speed_vol_slope,
        );
        let amplitude = SND_BASE_AMPLITUDE * db_to_factor(comp_db + speed_db);
        cm.calculated.sound_params.amplitude = clamp(amplitude, 0.05, 1.0);
        cm.calculated.sound_params.attack_ms = clamp(cm.config.snd_attack_ms, 0.5, 30.0);
    }

    /// Hand over parameters for the driver gated tone generation (gear alarm)
    fn set_driver_gated_params(&mut self, cm: &mut CoreModel, frequency: u16, continuous: bool) {
        cm.calculated.sound_params.frequency = clamp(
            frequency,
            cm.config.snd_min_freq as u16,
            cm.config.snd_max_freq as u16,
        );
        cm.calculated.sound_params.continuous = continuous;
        cm.calculated.sound_params.gate = true;
        // the driver gated tone has always glided between two commands - that stays
        cm.calculated.sound_params.glide = true;
    }

    fn gear_alarm_sound(&mut self, cm: &mut CoreModel) -> (u16, bool, i8) {
        const START_FREQ: u16 = 700;
        const INC_FREQ: u16 = 150;

        self.tick += 1;

        match self.tick {
            0..=4 => (START_FREQ, false, 0), // silence
            5 => (START_FREQ, true, cm.control.alarm_volume),
            6..=10 => (
                cm.calculated.sound_params.frequency + INC_FREQ,
                true,
                cm.control.alarm_volume,
            ),
            11 => (START_FREQ, false, 0), // silence
            12 => (START_FREQ, true, cm.control.alarm_volume),
            13..=17 => (
                cm.calculated.sound_params.frequency + INC_FREQ,
                true,
                cm.control.alarm_volume,
            ),
            18 => (START_FREQ, false, 0), // silence
            19 => (START_FREQ, true, cm.control.alarm_volume),
            20..=24 => (
                cm.calculated.sound_params.frequency + INC_FREQ,
                true,
                cm.control.alarm_volume,
            ),
            25..=40 => (START_FREQ, false, 0), // silence
            _ => {
                self.tick = 0;
                (START_FREQ, false, 0)
            }
        }
    }
}

/// one run of the sound preview: 16 s (10 ms ticks)
pub const PREVIEW_PERIOD_TICKS: u16 = 1600;

/// The sound preview runs while a setting of the vario tone is open in the
/// editor and the glider stands on the ground (below 30 km/h IAS) - in
/// flight the real vario is never replaced. Not for the dead band of the
/// speed to fly tone and not while the fair demo keeps the vario silent.
pub fn sound_preview_wanted(cm: &CoreModel) -> bool {
    use crate::Editable::*;
    if cm.control.editor.mode == crate::model::EditMode::Off || cm.control.demo.active() {
        return false;
    }
    if cm.sensor.airspeed.ias().to_km_h() >= 30.0 {
        return false;
    }
    matches!(
        cm.control.editor.target,
        CenterFrequency
            | Waveform
            | AudioDamping
            | SoundAttack
            | SoundFreqUpper
            | SoundFreqLower
            | SoundFullScale
            | SinkTone
            | SinkToneFrom
            | TicksUntil
            | BeepTempo
            | LoudnessComp
            | SoundPreset
    )
}

/// Climb rate of the preview at a tick: from -3 m/s steadily up to +5 m/s in
/// 10 s (0 m/s after 3.75 s), then back down in 6 s. Every part of the
/// tone is heard once per run: sink tone, silence, ticks, climb beeps.
pub fn sound_preview_climb(tick: u16) -> f32 {
    let t = (tick % PREVIEW_PERIOD_TICKS) as f32 / 100.0;
    if t < 10.0 {
        -3.0 + 0.8 * t
    } else {
        5.0 - (t - 10.0) * 8.0 / 6.0
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;
    use crate::{test_harness, Editable};

    #[test]
    fn the_preview_sweeps_the_whole_range_once_per_run() {
        assert!((sound_preview_climb(0) + 3.0).abs() < 1e-3);
        assert!(sound_preview_climb(375).abs() < 1e-2, "0 m/s after 3.75 s");
        assert!((sound_preview_climb(1000) - 5.0).abs() < 1e-3);
        assert!((sound_preview_climb(1599) + 3.0).abs() < 0.02);
        for t in 0..PREVIEW_PERIOD_TICKS {
            let v = sound_preview_climb(t);
            assert!((-3.01..=5.01).contains(&v));
        }
    }

    #[test]
    fn the_preview_runs_only_on_the_ground_with_a_sound_setting_open() {
        let (mut cm, mut cc, _tx) = test_harness!();
        assert!(!sound_preview_wanted(&cm), "no editor open");
        crate::controller::activate_editable(Editable::CenterFrequency, &mut cm, &mut cc);
        assert!(sound_preview_wanted(&cm));
        // in flight the real vario is never replaced
        cm.sensor.airspeed = crate::flight_physics::AirSpeed::from_speeds(
            crate::FloatToSpeed::km_h(100.0f32),
            crate::FloatToSpeed::km_h(100.0f32),
        );
        assert!(!sound_preview_wanted(&cm));
    }
}
