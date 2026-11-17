use super::{Content, EditableFuncs, EnumParams, F32Params, ListParams, Params};
use crate::{
    controller::{
        Waveform, SINK_TONE_CONTINUOUS, SINK_TONE_DOUBLE_BEEP, SINK_TONE_OFF, WAVEFORM_RECTANGULAR,
        WAVEFORM_SAWTOOTH, WAVEFORM_SINE_WAVE, WAVEFORM_TRIANGULAR,
    },
    get_snd_spreading_factor, menu,
    model::{
        config::{
            UnitHeight, UnitHorizontalSpeed, UnitVerticalSpeed, BRIGHTNESS_MIN, DEVICE_INFO,
            HORIZON, SINK_MODE_CONTINUOUS, SINK_MODE_DOUBLE_BEEP, SINK_MODE_OFF,
            STF_SWITCH_BALLAST, STF_SWITCH_FIXED, STF_SWITCH_MODE_BALLAST, STF_SWITCH_MODE_FIXED,
            STF_SWITCH_MODE_POLAR, STF_SWITCH_POLAR, UNIT_FEET, UNIT_FPM, UNIT_KMPH, UNIT_KNOTS,
            UNIT_METER, UNIT_MPH, UNIT_MPS, VARIO,
        },
        control::{DATA_SOURCE_FRONTEND, DATA_SOURCE_SENSORBOX},
        AvgMarker, DataSource, DisplayActive, DisplayTheme, GustFilter, VarioNeedle,
    },
    persist::{self, persist_set},
    polar_store,
    utils::{TString, Variant},
    view::viewable::{
        centerview::{CenterType, CenterView},
        vario_infoview::{Info3View, LineView, Placement},
    },
    CoreController, CoreModel, Echo, PersistenceId,
};
use tfmt::Convert;

pub struct AlarmVolume;
const VOLUME_PARAMS: Params = Params::F32(F32Params {
    min: 0.0,
    max: 50.0,
    small_inc: 1.0,
    big_inc: 3.0,
    dec_places: 0,
    unit: "",
});

impl EditableFuncs for AlarmVolume {
    fn name() -> &'static str {
        "Alarm Volume"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.control.alarm_volume as f32))
    }

    fn params(_cm: &CoreModel) -> Params {
        VOLUME_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::I8(val as i8),
                PersistenceId::AlarmVolume,
                Echo::None,
            );
        }
    }
}

pub struct AvgClimbRateSrc;
impl EditableFuncs for AvgClimbRateSrc {
    fn name() -> &'static str {
        "Avg Source"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Enum(TString::<16>::from_str(
            cm.control.avg_climb_rate_src.as_str(),
        ))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [DATA_SOURCE_FRONTEND, DATA_SOURCE_SENSORBOX, "", "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let source = DataSource::from(val.as_str());
            persist::persist_set(
                cc,
                cm,
                Variant::U32(source as u32),
                PersistenceId::AvgClimbeRateSrc,
                Echo::None,
            );
        }
    }
}

pub struct BatteryGood;
const BAT_PARAMS: Params = Params::F32(F32Params {
    min: 7.0,
    max: 15.0,
    small_inc: 0.1,
    big_inc: 1.0,
    dec_places: 1,
    unit: "V",
});

impl EditableFuncs for BatteryGood {
    fn name() -> &'static str {
        "Battery Good"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.battery_good))
    }

    fn params(_cm: &CoreModel) -> Params {
        BAT_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::BatteryGood,
                Echo::None,
            );
        }
    }
}

pub struct BatteryLow;
impl EditableFuncs for BatteryLow {
    fn name() -> &'static str {
        "Battery Low"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.battery_low))
    }

    fn params(_cm: &CoreModel) -> Params {
        BAT_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::BatteryLow,
                Echo::None,
            );
        }
    }
}

pub struct CenterFrequency;

/// The zero tone frequency. The tables are written for 350 Hz and are stretched onto this
/// value and the two anchor frequencies, so the semitone spacing of the curve is kept.
impl EditableFuncs for CenterFrequency {
    fn name() -> &'static str {
        "Center Frequency"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_center_freq))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 250.0,
            max: 600.0,
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit: "Hz",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::CenterFreqStefly,
                Echo::None,
            );
        }
    }
}

pub struct SoundFreqUpper;
impl EditableFuncs for SoundFreqUpper {
    fn name() -> &'static str {
        "Upper Frequency"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_freq_upper))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 800.0,
            max: 3000.0,
            small_inc: 5.0,
            big_inc: 50.0,
            dec_places: 0,
            unit: "Hz",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::SndFreqUpper,
                Echo::None,
            );
        }
    }
}

pub struct SoundFreqLower;
impl EditableFuncs for SoundFreqLower {
    fn name() -> &'static str {
        "Lower Frequency"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_freq_lower))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 100.0,
            max: 500.0,
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit: "Hz",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::SndFreqLower,
                Echo::None,
            );
        }
    }
}

pub struct SoundFullScale;
impl EditableFuncs for SoundFullScale {
    fn name() -> &'static str {
        "Full Scale"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_full_scale))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 2.5,
            max: 10.0,
            small_inc: 0.1,
            big_inc: 0.5,
            dec_places: 1,
            unit: "m/s",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::SndFullScale,
                Echo::None,
            );
        }
    }
}

pub struct SinkTone;
impl EditableFuncs for SinkTone {
    fn name() -> &'static str {
        "Sink Tone"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let s = match cm.config.snd_sink_mode {
            SINK_MODE_OFF => SINK_TONE_OFF,
            SINK_MODE_CONTINUOUS => SINK_TONE_CONTINUOUS,
            _ => SINK_TONE_DOUBLE_BEEP,
        };
        Content::Enum(TString::<16>::from_str(s))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [
                SINK_TONE_CONTINUOUS,
                SINK_TONE_DOUBLE_BEEP,
                SINK_TONE_OFF,
                "",
                "",
            ],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let mode = if val.as_str() == SINK_TONE_DOUBLE_BEEP {
                SINK_MODE_DOUBLE_BEEP
            } else if val.as_str() == SINK_TONE_OFF {
                SINK_MODE_OFF
            } else {
                SINK_MODE_CONTINUOUS
            };
            persist::persist_set(
                cc,
                cm,
                Variant::U8(mode),
                PersistenceId::SinkToneMode,
                Echo::None,
            );
        }
    }
}

pub struct SinkToneFrom;
impl EditableFuncs for SinkToneFrom {
    fn name() -> &'static str {
        "Sink Tone From"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_sink_from))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: -3.0,
            max: 0.0,
            small_inc: 0.1,
            big_inc: 0.5,
            dec_places: 1,
            unit: "m/s",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::SinkToneFrom,
                Echo::None,
            );
        }
    }
}

pub struct Brightness;
impl EditableFuncs for Brightness {
    fn name() -> &'static str {
        "Brightness"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.display_brightness as f32))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: BRIGHTNESS_MIN as f32,
            max: 100.0,
            small_inc: 5.0,
            big_inc: 20.0,
            dec_places: 0,
            unit: "%",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::U8(val as u8),
                PersistenceId::Brightness,
                Echo::None,
            );
        }
    }
}

pub struct Qnh;
impl EditableFuncs for Qnh {
    fn name() -> &'static str {
        "QNH"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.sensor.pressure_altitude.qnh().to_hpa()))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 940.0,
            max: 1070.0,
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit: "hPa",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Pressure(crate::Pressure::from_hpa(val)),
                PersistenceId::Qnh,
                Echo::NmeaAndCan,
            );
        }
    }
}

pub struct StfSwitchMode;
impl EditableFuncs for StfSwitchMode {
    fn name() -> &'static str {
        "Auto Switch"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let s = match cm.config.stf_switch_mode {
            STF_SWITCH_MODE_FIXED => STF_SWITCH_FIXED,
            STF_SWITCH_MODE_BALLAST => STF_SWITCH_BALLAST,
            _ => STF_SWITCH_POLAR,
        };
        Content::Enum(TString::<16>::from_str(s))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [
                STF_SWITCH_POLAR,
                STF_SWITCH_FIXED,
                STF_SWITCH_BALLAST,
                "",
                "",
            ],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let mode = if val.as_str() == STF_SWITCH_FIXED {
                STF_SWITCH_MODE_FIXED
            } else if val.as_str() == STF_SWITCH_BALLAST {
                STF_SWITCH_MODE_BALLAST
            } else {
                STF_SWITCH_MODE_POLAR
            };
            persist::persist_set(
                cc,
                cm,
                Variant::U8(mode),
                PersistenceId::StfSwitchMode,
                Echo::None,
            );
        }
    }
}

pub struct StfSwitchSpeed;
impl EditableFuncs for StfSwitchSpeed {
    fn name() -> &'static str {
        "Switch Speed"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let value = cm
            .config
            .unit_horizontal_speed
            .as_f32(cm.config.stf_switch_speed);
        Content::F32(Some(value))
    }

    fn params(cm: &CoreModel) -> Params {
        let unit = cm.config.unit_horizontal_speed.as_str();
        Params::F32(F32Params {
            min: 50.0,
            max: 200.0,
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(value)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Speed(cm.config.unit_horizontal_speed.as_speed(value)),
                PersistenceId::StfSwitchSpeed,
                Echo::None,
            )
        }
    }
}

pub struct FwDowngrade;
impl EditableFuncs for FwDowngrade {
    fn name() -> &'static str {
        "FW Downgrade"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        if cm.config.allow_fw_downgrade {
            Content::Enum(TString::<16>::from_str(ON))
        } else {
            Content::Enum(TString::<16>::from_str(OFF))
        }
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [OFF, ON, "", "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            // allows installing an OLDER firmware version from SD card on the
            // next start - for test versions the customers try out
            persist::persist_set(
                cc,
                cm,
                Variant::Bool(val.as_str() == ON),
                PersistenceId::FwDowngrade,
                Echo::None,
            );
        }
    }
}

pub struct TicksUntil;
impl EditableFuncs for TicksUntil {
    fn name() -> &'static str {
        "Ticks Until"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_ticks_until))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: -1.0,
            max: 0.0,
            small_inc: 0.1,
            big_inc: 0.5,
            dec_places: 1,
            unit: "m/s",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::TicksUntil,
                Echo::None,
            );
        }
    }
}

pub struct AudioDamping;
impl EditableFuncs for AudioDamping {
    fn name() -> &'static str {
        "Audio Damping"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_audio_tc))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 0.0,
            max: 2.0,
            small_inc: 0.1,
            big_inc: 0.5,
            dec_places: 1,
            unit: "s",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::AudioDamping,
                Echo::None,
            );
        }
    }
}

pub struct SoundAttack;

/// Attack of the tone envelope, the release is always twice as long - same as in the tone lab.
///
/// A tone that starts and stops abruptly is heard as a hard edge even when it is switched at the
/// zero crossing. A few milliseconds of ramp take that edge away.
impl EditableFuncs for SoundAttack {
    fn name() -> &'static str {
        "Attack"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_attack_ms))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 1.0,
            max: 30.0,
            small_inc: 0.5,
            big_inc: 2.0,
            dec_places: 1,
            unit: "ms",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::SoundAttack,
                Echo::None,
            );
        }
    }
}

pub struct LoudnessComp;
impl EditableFuncs for LoudnessComp {
    fn name() -> &'static str {
        "Loudness Comp"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_loudness_comp * 100.0))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 0.0,
            max: 150.0,
            small_inc: 5.0,
            big_inc: 25.0,
            dec_places: 0,
            unit: "%",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val / 100.0),
                PersistenceId::LoudnessComp,
                Echo::None,
            );
        }
    }
}

pub struct SpeedVolume;
impl EditableFuncs for SpeedVolume {
    fn name() -> &'static str {
        "Speed Volume"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_speed_vol_slope))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 0.0,
            max: 3.0,
            small_inc: 0.1,
            big_inc: 0.5,
            dec_places: 1,
            unit: "dB",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::SpeedVolSlope,
                Echo::None,
            );
        }
    }
}

pub struct SpeedVolumeRef;
impl EditableFuncs for SpeedVolumeRef {
    fn name() -> &'static str {
        "Speed Reference"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_speed_vol_ref))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 80.0,
            max: 150.0,
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit: "km/h",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::SpeedVolRef,
                Echo::None,
            );
        }
    }
}

pub struct BeepTempo;
impl EditableFuncs for BeepTempo {
    fn name() -> &'static str {
        "Beep Tempo"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.snd_beep_tempo))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 0.6,
            max: 1.6,
            small_inc: 0.05,
            big_inc: 0.2,
            dec_places: 2,
            unit: "x",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::BeepTempo,
                Echo::None,
            );
        }
    }
}

pub struct CenterViewCircling;
impl EditableFuncs for CenterViewCircling {
    fn name() -> &'static str {
        "Center Content"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(
            cm.config
                .center_circling
                .sorted_as_i32(CenterType::Circling),
        )
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        convert
            .write_str(CenterView::from_sorted(idx as usize, CenterType::Circling).name())
            .unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: CenterView::max(CenterType::Circling) as i32,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(value) = content {
            let variant = CenterView::from_sorted(value as usize, CenterType::Circling) as i32;
            persist::persist_set(
                cc,
                cm,
                Variant::I32(variant),
                PersistenceId::CenterViewCircling,
                Echo::None,
            )
        }
    }
}

pub struct CenterViewStraight;
impl EditableFuncs for CenterViewStraight {
    fn name() -> &'static str {
        "Center Content"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(
            cm.config
                .center_straight
                .sorted_as_i32(CenterType::Straight),
        )
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        convert
            .write_str(CenterView::from_sorted(idx as usize, CenterType::Straight).name())
            .unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: CenterView::max(CenterType::Straight) as i32,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(value) = content {
            let variant = CenterView::from_sorted(value as usize, CenterType::Straight) as i32;
            persist::persist_set(
                cc,
                cm,
                Variant::I32(variant),
                PersistenceId::CenterViewStraight,
                Echo::None,
            )
        }
    }
}

pub struct Display;

impl EditableFuncs for Display {
    fn name() -> &'static str {
        "Display"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        match cm.config.last_display_active {
            DisplayActive::Horizon => Content::Enum(TString::<16>::from_str(HORIZON)),
            DisplayActive::DeviceInfo => Content::Enum(TString::<16>::from_str(DEVICE_INFO)),
            _ => Content::Enum(TString::<16>::from_str(VARIO)),
        }
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [VARIO, HORIZON, DEVICE_INFO, "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let display_active = DisplayActive::from(val.as_str());
            persist::persist_set(
                cc,
                cm,
                Variant::U32(display_active as u32),
                PersistenceId::Display,
                Echo::None,
            );
        }
    }
}

pub struct Glider;
impl EditableFuncs for Glider {
    fn name() -> &'static str {
        "Glider"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let sorted_idx = polar_store::to_sorted_idx(cm.config.glider_idx as usize);
        Content::List(sorted_idx as i32)
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        let raw_idx = polar_store::to_raw_idx(idx as usize);
        let name = polar_store::from_raw_idx(raw_idx).name;
        convert.write_str(name).unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: polar_store::size() as i32 - 1,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(sorted_idx) = content {
            let raw_idx = polar_store::to_raw_idx(sorted_idx as usize) as u32;
            // Taken with a push only, and only another glider (15.11.2026):
            // a new polar replaces empty mass, max ballast and the polar
            // points - turning through the list or opening it must not.
            if !cm.control.editor.enter_pushed || raw_idx == cm.config.glider_idx as u32 {
                return;
            }
            persist::persist_set(
                cc,
                cm,
                Variant::U32(raw_idx),
                PersistenceId::Glider,
                Echo::None,
            )
        }
    }
}

pub struct GliderSymbol;
const ON: &str = "On";
const OFF: &str = "Off";

impl EditableFuncs for GliderSymbol {
    fn name() -> &'static str {
        "Glider Symbol"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        if cm.config.glider_symbol {
            Content::Enum(TString::<16>::from_str(ON))
        } else {
            Content::Enum(TString::<16>::from_str(OFF))
        }
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [ON, OFF, "", "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Bool(val.as_str() == ON),
                PersistenceId::GliderSymbol,
                Echo::None,
            );
        }
    }
}

pub struct Info1Vario;
impl EditableFuncs for Info1Vario {
    fn name() -> &'static str {
        "Info 1 Content"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.config.info1_vario.sorted_as_i32(Placement::Top))
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        convert
            .write_str(LineView::from_sorted(idx as usize, Placement::Top).name())
            .unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: LineView::max(Placement::Top) as i32,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(value) = content {
            let variant = LineView::from_sorted(value as usize, Placement::Top) as i32;
            persist::persist_set(
                cc,
                cm,
                Variant::I32(variant),
                PersistenceId::Info1Vario,
                Echo::None,
            )
        }
    }
}

pub struct Info1Stf;
impl EditableFuncs for Info1Stf {
    fn name() -> &'static str {
        "Info 1 Content"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.config.info1_stf.sorted_as_i32(Placement::Top))
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        convert
            .write_str(LineView::from_sorted(idx as usize, Placement::Top).name())
            .unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: LineView::max(Placement::Top) as i32,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(value) = content {
            let variant = LineView::from_sorted(value as usize, Placement::Top) as i32;
            persist::persist_set(
                cc,
                cm,
                Variant::I32(variant),
                PersistenceId::Info1Stf,
                Echo::None,
            )
        }
    }
}

pub struct Info2Vario;
impl EditableFuncs for Info2Vario {
    fn name() -> &'static str {
        "Info 2 Content"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.config.info2_vario.sorted_as_i32(Placement::Bottom))
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        convert
            .write_str(LineView::from_sorted(idx as usize, Placement::Bottom).name())
            .unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: LineView::max(Placement::Bottom) as i32,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(value) = content {
            let variant = LineView::from_sorted(value as usize, Placement::Bottom) as i32;
            persist::persist_set(
                cc,
                cm,
                Variant::I32(variant),
                PersistenceId::Info2Vario,
                Echo::None,
            )
        }
    }
}

pub struct Info2Stf;
impl EditableFuncs for Info2Stf {
    fn name() -> &'static str {
        "Info 2 Content"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.config.info2_stf.sorted_as_i32(Placement::Bottom))
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        convert
            .write_str(LineView::from_sorted(idx as usize, Placement::Bottom).name())
            .unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: LineView::max(Placement::Bottom) as i32,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(value) = content {
            let variant = LineView::from_sorted(value as usize, Placement::Bottom) as i32;
            persist::persist_set(
                cc,
                cm,
                Variant::I32(variant),
                PersistenceId::Info2Stf,
                Echo::None,
            )
        }
    }
}

pub struct Info3Vario;
impl EditableFuncs for Info3Vario {
    fn name() -> &'static str {
        "Info 3 Content"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.config.info3_vario as i32)
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        convert
            .write_str(Info3View::from(idx as u8).name())
            .unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: Info3View::max(),
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(value) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::I32(value),
                PersistenceId::Info3Vario,
                Echo::None,
            )
        }
    }
}

pub struct Info3Stf;
impl EditableFuncs for Info3Stf {
    fn name() -> &'static str {
        "Info 3 Content"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.config.info3_stf as i32)
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        convert
            .write_str(Info3View::from(idx as u8).name())
            .unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: Info3View::max(),
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(value) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::I32(value),
                PersistenceId::Info3Stf,
                Echo::None,
            )
        }
    }
}

pub struct McCready;
impl EditableFuncs for McCready {
    fn name() -> &'static str {
        "MC"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let value = cm.config.unit_vertical_speed.as_f32(cm.config.mc_cready);
        Content::F32(Some(value))
    }

    fn params(cm: &CoreModel) -> Params {
        let unit = cm.config.unit_vertical_speed.as_str();
        match cm.config.unit_vertical_speed {
            UnitVerticalSpeed::Fpm => Params::F32(F32Params {
                min: 0.0,
                max: 1000.0,
                small_inc: 20.0,
                big_inc: 20.0,
                dec_places: 0,
                unit,
            }),
            UnitVerticalSpeed::Knots => Params::F32(F32Params {
                min: 0.0,
                max: 10.0,
                small_inc: 0.2,
                big_inc: 0.2,
                dec_places: 1,
                unit,
            }),
            UnitVerticalSpeed::Mps => Params::F32(F32Params {
                min: 0.0,
                max: 5.0,
                small_inc: 0.1,
                big_inc: 0.1,
                dec_places: 1,
                unit,
            }),
        }
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(value)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Speed(cm.config.unit_vertical_speed.as_speed(value)),
                PersistenceId::McCready,
                Echo::NmeaAndCan,
            );
        }
    }
}

pub struct SoundSpreading;
impl EditableFuncs for SoundSpreading {
    fn name() -> &'static str {
        "Spreading Factor"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let value = get_snd_spreading_factor(cm);
        Content::F32(Some(value))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 0.5,
            max: 2.0,
            small_inc: 0.1,
            big_inc: 0.2,
            dec_places: 1,
            unit: "",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(value)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(value),
                PersistenceId::SoundSpreading,
                Echo::None,
            );
        }
    }
}

pub struct StfUpperLimit;
impl EditableFuncs for StfUpperLimit {
    fn name() -> &'static str {
        "StF Upper Limit"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let value = cm
            .config
            .unit_horizontal_speed
            .as_f32(cm.config.stf_upper_limit);
        Content::F32(Some(value))
    }

    fn params(cm: &CoreModel) -> Params {
        let unit = cm.config.unit_horizontal_speed.as_str();
        Params::F32(F32Params {
            min: 0.0,
            max: 50.0,
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(value)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Speed(cm.config.unit_horizontal_speed.as_speed(value)),
                PersistenceId::StfUpperLimit,
                Echo::None,
            )
        }
    }
}

pub struct StfLowerLimit;
impl EditableFuncs for StfLowerLimit {
    fn name() -> &'static str {
        "StF Lower Limit"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let value = cm
            .config
            .unit_horizontal_speed
            .as_f32(cm.config.stf_lower_limit);
        Content::F32(Some(value))
    }

    fn params(cm: &CoreModel) -> Params {
        let unit = cm.config.unit_horizontal_speed.as_str();
        Params::F32(F32Params {
            min: -50.0,
            max: 0.0,
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(value)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Speed(cm.config.unit_horizontal_speed.as_speed(value)),
                PersistenceId::StfLowerLimit,
                Echo::None,
            )
        }
    }
}

pub struct TcCircleHysteresis;
impl EditableFuncs for TcCircleHysteresis {
    fn name() -> &'static str {
        "Circling Delay"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.circle_hysteresis_tc as f32))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 3.0,
            max: 30.0,
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit: "s",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::I8(val as i8),
                PersistenceId::TcCircleHysteresis,
                Echo::None,
            )
        }
    }
}

pub struct TcClimbRate;
impl EditableFuncs for TcClimbRate {
    fn name() -> &'static str {
        "Avg Time"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.av2_climb_rate_tc))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 15.0,
            max: 120.0,
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit: "s",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::TcClimbRate,
                Echo::Can,
            )
        }
    }
}

pub struct TcSpeedToFly;
impl EditableFuncs for TcSpeedToFly {
    fn name() -> &'static str {
        "StF Time"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.av_speed_to_fly_tc))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 1.0,
            max: 60.0,
            small_inc: 1.0,
            big_inc: 10.0,
            dec_places: 0,
            unit: "s",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::F32(val),
                PersistenceId::TcSpeedToFly,
                Echo::Can,
            )
        }
    }
}

pub struct Theme;
const DARK: &str = "Dark";
const BRIGHT: &str = "Bright";

impl EditableFuncs for Theme {
    fn name() -> &'static str {
        "Theme"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        if cm.config.theme == &cm.device_const.dark_theme {
            Content::Enum(TString::<16>::from_str(DARK))
        } else {
            Content::Enum(TString::<16>::from_str(BRIGHT))
        }
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [DARK, BRIGHT, "", "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let theme = match val.as_str() {
                BRIGHT => DisplayTheme::Bright,
                _ => DisplayTheme::Dark,
            };
            persist::persist_set(
                cc,
                cm,
                Variant::U32(theme as u32),
                PersistenceId::DisplayTheme,
                Echo::None,
            );
        }
    }
}

pub struct VarioUpperLimit;
impl EditableFuncs for VarioUpperLimit {
    fn name() -> &'static str {
        "Vario Upper Limit"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let value = cm
            .config
            .unit_vertical_speed
            .as_f32(cm.config.vario_upper_limit);
        Content::F32(Some(value))
    }

    fn params(cm: &CoreModel) -> Params {
        let unit = cm.config.unit_vertical_speed.as_str();
        match cm.config.unit_vertical_speed {
            UnitVerticalSpeed::Fpm => Params::F32(F32Params {
                min: -2000.0,
                max: 2000.0,
                small_inc: 20.0,
                big_inc: 200.0,
                dec_places: 0,
                unit,
            }),
            UnitVerticalSpeed::Knots => Params::F32(F32Params {
                min: -20.0,
                max: 20.0,
                small_inc: 0.2,
                big_inc: 2.0,
                dec_places: 1,
                unit,
            }),
            UnitVerticalSpeed::Mps => Params::F32(F32Params {
                min: -10.0,
                max: 10.0,
                small_inc: 0.1,
                big_inc: 1.0,
                dec_places: 1,
                unit,
            }),
        }
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(value)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Speed(cm.config.unit_vertical_speed.as_speed(value)),
                PersistenceId::VarioUpperLimit,
                Echo::None,
            )
        }
    }
}

pub struct VarioLowerLimit;
impl EditableFuncs for VarioLowerLimit {
    fn name() -> &'static str {
        "Vario Lower Limit"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let value = cm
            .config
            .unit_vertical_speed
            .as_f32(cm.config.vario_lower_limit);
        Content::F32(Some(value))
    }

    fn params(cm: &CoreModel) -> Params {
        let unit = cm.config.unit_vertical_speed.as_str();
        match cm.config.unit_vertical_speed {
            UnitVerticalSpeed::Fpm => Params::F32(F32Params {
                min: -2000.0,
                max: 2000.0,
                small_inc: 20.0,
                big_inc: 200.0,
                dec_places: 0,
                unit,
            }),
            UnitVerticalSpeed::Knots => Params::F32(F32Params {
                min: -20.0,
                max: 20.0,
                small_inc: 0.2,
                big_inc: 2.0,
                dec_places: 1,
                unit,
            }),
            UnitVerticalSpeed::Mps => Params::F32(F32Params {
                min: -10.0,
                max: 10.0,
                small_inc: 0.1,
                big_inc: 1.0,
                dec_places: 1,
                unit,
            }),
        }
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(value)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Speed(cm.config.unit_vertical_speed.as_speed(value)),
                PersistenceId::VarioLowerLimit,
                Echo::None,
            )
        }
    }
}

pub struct UnitHeigth_;

impl EditableFuncs for UnitHeigth_ {
    fn name() -> &'static str {
        "Height"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        match cm.config.unit_height {
            UnitHeight::Feet => Content::Enum(TString::<16>::from_str(UNIT_FEET)),
            _ => Content::Enum(TString::<16>::from_str(UNIT_METER)),
        }
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [UNIT_METER, UNIT_FEET, "", "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let unit = UnitHeight::from_str(val.as_str());
            persist::persist_set(
                cc,
                cm,
                Variant::I32(unit as i32),
                PersistenceId::UnitHeight,
                Echo::None,
            );
        }
    }
}

pub struct UnitHorizontalSpeed_;

impl EditableFuncs for UnitHorizontalSpeed_ {
    fn name() -> &'static str {
        "Horizontal Speed"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        match cm.config.unit_horizontal_speed {
            UnitHorizontalSpeed::Mph => Content::Enum(TString::<16>::from_str(UNIT_MPH)),
            UnitHorizontalSpeed::Knots => Content::Enum(TString::<16>::from_str(UNIT_KNOTS)),
            _ => Content::Enum(TString::<16>::from_str(UNIT_KMPH)),
        }
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [UNIT_KMPH, UNIT_MPH, UNIT_KNOTS, "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let unit = UnitHorizontalSpeed::from_str(val.as_str());
            persist::persist_set(
                cc,
                cm,
                Variant::I32(unit as i32),
                PersistenceId::UnitHorizontalSpeed,
                Echo::None,
            );
        }
    }
}

pub struct UnitVerticalSpeed_;

impl EditableFuncs for UnitVerticalSpeed_ {
    fn name() -> &'static str {
        "Vertical Speed"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        match cm.config.unit_vertical_speed {
            UnitVerticalSpeed::Fpm => Content::Enum(TString::<16>::from_str(UNIT_FPM)),
            UnitVerticalSpeed::Knots => Content::Enum(TString::<16>::from_str(UNIT_KNOTS)),
            _ => Content::Enum(TString::<16>::from_str(UNIT_MPS)),
        }
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [UNIT_MPS, UNIT_FPM, UNIT_KNOTS, "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let unit = UnitVerticalSpeed::from_str(val.as_str());
            persist::persist_set(
                cc,
                cm,
                Variant::I32(unit as i32),
                PersistenceId::UnitVerticalSpeed,
                Echo::None,
            );
        }
    }
}

pub struct UsageCode;
impl EditableFuncs for UsageCode {
    fn name() -> &'static str {
        "Code"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.usage_code as f32))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 0.0,
            max: 9999.0,
            small_inc: 1.0,
            big_inc: 100.0,
            dec_places: 0,
            unit: "",
        })
    }

    fn set_content(cm: &mut CoreModel, _cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            cm.config.usage_code = val as u16;
        }
    }
}

pub const USAGE_MODE_CLUB: &str = "Club";
pub struct UsageMode;
pub const USAGE_MODE_BASIC: &str = "Basic";
pub const USAGE_MODE_EXPERT: &str = "Expert";

/// name of the mode in effect
pub fn usage_mode(cm: &CoreModel) -> &'static str {
    if cm.config.club_mode {
        USAGE_MODE_CLUB
    } else if cm.config.basic_mode {
        USAGE_MODE_BASIC
    } else {
        USAGE_MODE_EXPERT
    }
}

/// Basic (reduced menus, the delivery state), Expert (everything) - both
/// free to choose - and Club, which needs the code to enter and to leave.
impl EditableFuncs for UsageMode {
    fn name() -> &'static str {
        "Usage Mode"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Enum(TString::<16>::from_str(usage_mode(cm)))
    }

    fn params(cm: &CoreModel) -> Params {
        let code_ok = cm.config.usage_code == cm.device_const.misc.sw_version.lower_as_u16();
        let variants = if code_ok {
            [USAGE_MODE_BASIC, USAGE_MODE_EXPERT, USAGE_MODE_CLUB, "", ""]
        } else if cm.config.club_mode {
            [USAGE_MODE_CLUB, "", "", "", ""]
        } else {
            [USAGE_MODE_BASIC, USAGE_MODE_EXPERT, "", "", ""]
        };
        Params::Enum(EnumParams { variants })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            if cm.control.editor.enter_pushed && val.as_str() != usage_mode(cm) {
                let club_mode = val.as_str() == USAGE_MODE_CLUB;
                let basic_mode = val.as_str() == USAGE_MODE_BASIC;
                let club_changed = club_mode != cm.config.club_mode;
                cm.config.club_mode = club_mode;
                cm.config.basic_mode = basic_mode;
                // Hard intervention necessary because the menu structure
                // is being changed: continue in the System menu of the
                // other list. Without a parent stack, Back uses the
                // static index and lands in the setup of that list.
                cm.control.menu_control.menu = if club_mode {
                    &menu::club::SYSTEM
                } else if basic_mode {
                    &menu::basic::SYSTEM
                } else {
                    &menu::full::STEFLY_SYSTEM
                };
                let level = cm.control.menu_control.menu.level;
                cm.control.menu_control.pos[level] = 0;
                cm.control.menu_control.depth = 0;
                if club_mode && cm.config.user_profile == 0 {
                    cm.config.user_profile = 1;
                    persist::user_profile(cm, cc);
                }
                if club_changed {
                    persist_set(
                        cc,
                        cm,
                        Variant::Bool(club_mode),
                        PersistenceId::ClubMode,
                        Echo::None,
                    );
                }
                if !club_mode {
                    persist::store_menu_mode(cm, cc);
                }
            }
        }
    }
}

pub struct Volume;
impl EditableFuncs for Volume {
    fn name() -> &'static str {
        "Volume"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.volume as f32))
    }

    fn params(_cm: &CoreModel) -> Params {
        VOLUME_PARAMS
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::I8(val as i8),
                PersistenceId::Volume,
                Echo::NmeaAndCan,
            );
        }
    }
}

pub struct Waveform_;

impl EditableFuncs for Waveform_ {
    fn name() -> &'static str {
        "Waveform"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        match cm.calculated.sound_params.waveform {
            Waveform::Rectangular => Content::Enum(TString::<16>::from_str(WAVEFORM_RECTANGULAR)),
            Waveform::Sawtooth => Content::Enum(TString::<16>::from_str(WAVEFORM_SAWTOOTH)),
            Waveform::SineWave => Content::Enum(TString::<16>::from_str(WAVEFORM_SINE_WAVE)),
            Waveform::Triangular => Content::Enum(TString::<16>::from_str(WAVEFORM_TRIANGULAR)),
        }
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [
                WAVEFORM_RECTANGULAR,
                WAVEFORM_SAWTOOTH,
                WAVEFORM_SINE_WAVE,
                WAVEFORM_TRIANGULAR,
                "",
            ],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let waveform = Waveform::from(val.as_str());
            persist::persist_set(
                cc,
                cm,
                Variant::U8(waveform as u8),
                PersistenceId::Waveform,
                Echo::None,
            );
        }
    }
}

pub struct InfoR1;
impl EditableFuncs for InfoR1 {
    fn name() -> &'static str {
        "Ring Info 1 Content"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.config.info_r1.sorted_as_i32(Placement::Bottom))
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        convert
            .write_str(LineView::from_sorted(idx as usize, Placement::Bottom).name())
            .unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: LineView::max(Placement::Bottom) as i32,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(value) = content {
            let variant = LineView::from_sorted(value as usize, Placement::Bottom) as i32;
            persist::persist_set(
                cc,
                cm,
                Variant::I32(variant),
                PersistenceId::InfoR1,
                Echo::None,
            )
        }
    }
}

pub struct InfoR2;
impl EditableFuncs for InfoR2 {
    fn name() -> &'static str {
        "Ring Info 2 Content"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.config.info_r2.sorted_as_i32(Placement::Bottom))
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        convert
            .write_str(LineView::from_sorted(idx as usize, Placement::Bottom).name())
            .unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: LineView::max(Placement::Bottom) as i32,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(value) = content {
            let variant = LineView::from_sorted(value as usize, Placement::Bottom) as i32;
            persist::persist_set(
                cc,
                cm,
                Variant::I32(variant),
                PersistenceId::InfoR2,
                Echo::None,
            )
        }
    }
}

pub struct InfoR3;
impl EditableFuncs for InfoR3 {
    fn name() -> &'static str {
        "Ring Info 3 Content"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::List(cm.config.info_r3.sorted_as_i32(Placement::Bottom))
    }

    fn content_as_str(convert: &mut Convert<20>, idx: i32) {
        convert
            .write_str(LineView::from_sorted(idx as usize, Placement::Bottom).name())
            .unwrap()
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::List(ListParams {
            max: LineView::max(Placement::Bottom) as i32,
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::List(value) = content {
            let variant = LineView::from_sorted(value as usize, Placement::Bottom) as i32;
            persist::persist_set(
                cc,
                cm,
                Variant::I32(variant),
                PersistenceId::InfoR3,
                Echo::None,
            )
        }
    }
}

pub struct PotK;
impl EditableFuncs for PotK {
    fn name() -> &'static str {
        // AoA factor of the entry optimized needle (Vario Signal). Up to
        // 0.4.1.47 "POT Factor k": the POT box used it too
        "Needle Factor k"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.pot_k))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: -1.0,
            max: 1.0,
            small_inc: 0.01,
            big_inc: 0.05,
            dec_places: 2,
            unit: "m/s/deg",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(cc, cm, Variant::F32(val), PersistenceId::PotK, Echo::None);
        }
    }
}

pub struct PotBar;
impl EditableFuncs for PotBar {
    fn name() -> &'static str {
        // POT box: bar of the yes / no in percent of the MacCready value
        "POT Bar"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::F32(Some(cm.config.pot_bar * 100.0))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::F32(F32Params {
            min: 50.0,
            max: 150.0,
            small_inc: 5.0,
            big_inc: 10.0,
            dec_places: 0,
            unit: "% MC",
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::F32(Some(val)) = content {
            persist::persist_set(cc, cm, Variant::F32(val), PersistenceId::PotBar, Echo::None);
        }
    }
}

pub struct PotAdvice;
impl EditableFuncs for PotAdvice {
    fn name() -> &'static str {
        // POT box: the yes / no symbol (tick, cross)
        "POT Yes/No"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Enum(TString::<16>::from_str(if cm.config.pot_advice {
            ON
        } else {
            OFF
        }))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [ON, OFF, "", "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Bool(val.as_str() == ON),
                PersistenceId::PotAdvice,
                Echo::None,
            );
        }
    }
}

pub struct NeedleMode;
const NEEDLE_STANDARD: &str = "Standard";
const NEEDLE_OPTIMIZED: &str = "Entry Optimized";

impl EditableFuncs for NeedleMode {
    fn name() -> &'static str {
        // applies to the needle AND the audio vario
        "Vario Signal"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        match cm.config.vario_needle {
            VarioNeedle::Standard => Content::Enum(TString::<16>::from_str(NEEDLE_STANDARD)),
            VarioNeedle::EntryOptimized => Content::Enum(TString::<16>::from_str(NEEDLE_OPTIMIZED)),
        }
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [NEEDLE_STANDARD, NEEDLE_OPTIMIZED, "", "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let mode = match val.as_str() {
                NEEDLE_OPTIMIZED => VarioNeedle::EntryOptimized,
                _ => VarioNeedle::Standard,
            };
            persist::persist_set(
                cc,
                cm,
                Variant::U32(mode as u32),
                PersistenceId::VarioNeedle,
                Echo::None,
            );
        }
    }
}

pub struct GustFilterMode;
const GUST_OFF: &str = "Off";
const GUST_WEAK: &str = "Weak";
const GUST_MEDIUM: &str = "Medium";
const GUST_STRONG: &str = "Strong";
const GUST_PEAK_CUT: &str = "Peak Cut";

impl EditableFuncs for GustFilterMode {
    fn name() -> &'static str {
        // PT2 gust filter, applies to the needle AND the audio vario
        "Gust Filter"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let s = match cm.config.gust_filter {
            GustFilter::Off => GUST_OFF,
            GustFilter::Weak => GUST_WEAK,
            GustFilter::Medium => GUST_MEDIUM,
            GustFilter::Strong => GUST_STRONG,
            GustFilter::PeakCut => GUST_PEAK_CUT,
        };
        Content::Enum(TString::<16>::from_str(s))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [GUST_OFF, GUST_WEAK, GUST_MEDIUM, GUST_STRONG, GUST_PEAK_CUT],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let mode = match val.as_str() {
                GUST_WEAK => GustFilter::Weak,
                GUST_MEDIUM => GustFilter::Medium,
                GUST_STRONG => GustFilter::Strong,
                GUST_PEAK_CUT => GustFilter::PeakCut,
                _ => GustFilter::Off,
            };
            persist::persist_set(
                cc,
                cm,
                Variant::U32(mode as u32),
                PersistenceId::GustFilter,
                Echo::None,
            );
        }
    }
}

pub struct GustCircling;
impl EditableFuncs for GustCircling {
    fn name() -> &'static str {
        // knee compression of the gust filter while circling too
        "Gust Circling"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        if cm.config.gust_filter_circling {
            Content::Enum(TString::<16>::from_str(ON))
        } else {
            Content::Enum(TString::<16>::from_str(OFF))
        }
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [ON, OFF, "", "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Bool(val.as_str() == ON),
                PersistenceId::GustCircling,
                Echo::None,
            );
        }
    }
}

pub struct AvgMarkerMode;
const AVG_MARKER_AVERAGE: &str = "Average";
const AVG_MARKER_POTENTIAL: &str = "Potential";

impl EditableFuncs for AvgMarkerMode {
    fn name() -> &'static str {
        // red triangle on the ring scale: normal average or POT value
        "Avg Marker"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let s = match cm.config.ring_avg_marker {
            AvgMarker::Average => AVG_MARKER_AVERAGE,
            AvgMarker::Potential => AVG_MARKER_POTENTIAL,
        };
        Content::Enum(TString::<16>::from_str(s))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [AVG_MARKER_AVERAGE, AVG_MARKER_POTENTIAL, "", "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let mode = match val.as_str() {
                AVG_MARKER_POTENTIAL => AvgMarker::Potential,
                _ => AvgMarker::Average,
            };
            persist::persist_set(
                cc,
                cm,
                Variant::U32(mode as u32),
                PersistenceId::RingAvgMarker,
                Echo::None,
            );
        }
    }
}
