//! One or two lines of help below the value of a setup editor (Stefan
//! 15.11.2026: every setting says what it does, so the device can be set up
//! without the manual). Plain words, what changes when the value changes.
//! A line has to fit the display circle at its height (test below).

use crate::Editable;

/// longest help line in characters (checked against the pixels in the test)
#[cfg(test)]
pub const HELP_MAX_CHARS: usize = 30;

pub fn help(e: Editable) -> Option<[&'static str; 2]> {
    use Editable::*;
    Some(match e {
        // ---- sound ----
        CenterFrequency => [
            "Pitch of the tone at 0 m/s.",
            "Lower = deeper, calmer sound",
        ],
        Waveform => ["Timbre: Sine soft and pure,", "Rectangular sharp, loud"],
        AudioDamping => ["Smooths the pitch. Higher =", "calmer, but follows later"],
        SoundAttack => ["How soft a beep starts.", "Higher = softer, no clicks"],
        SoundFreqUpper => ["Pitch at full scale climb.", "Higher = more pitch per m/s"],
        SoundFreqLower => ["Pitch at full scale sink.", "Lower = deeper sink tone"],
        SoundFullScale => [
            "Climb where the top pitch is",
            "reached. Smaller = livelier",
        ],
        SinkTone => ["Tone in sink: steady, falling", "double beep, or silent"],
        SinkToneFrom => [
            "Sink tone starts below this.",
            "Above it the vario is quiet",
        ],
        TicksUntil => [
            "Short ticks between this and",
            "0 m/s: lift is near. 0 = off",
        ],
        BeepTempo => ["Speed of the climb beeps.", "Above 1 = faster beeping"],
        StfUpperLimit => ["Speed to fly tone: silent up", "to this much too fast"],
        StfLowerLimit => ["Speed to fly tone: silent up", "to this much too slow"],
        LoudnessComp => ["Makes low tones as loud as", "high ones. 0 = no balancing"],
        SpeedVolume => ["Louder when flying faster:", "dB per 10 km/h. 0 = off"],
        SpeedVolumeRef => ["Airspeed at which the volume", "is exactly the set volume"],
        Volume => ["Volume of the vario tone", ""],
        AlarmVolume => ["Volume of the gear warning", ""],
        // ---- vario ----
        VarioTc => ["Response of needle and tone.", "Higher = calmer, slower"],
        GustFilterMode => ["Cuts short gust peaks.", "Peak Cut adds no delay"],
        GustCircling => ["Gust filter while circling", "too (calmer in thermals)"],
        NeedleMode => ["Entry Optimized: the needle", "does not overshoot"],
        PotK => ["Strength of the entry", "correction. Default -0.18"],
        AvgClimbRateSrc => ["Vario Average comes from", "this display or the box"],
        TcClimbRate => ["Vario Average looks back", "this long (display source)"],
        VarioIntTc => ["Vario Average looks back", "this long (box source)"],
        AvgMarkerMode => ["Magenta mark on the scale:", "average or POT estimate"],
        PotBar => ["POT says yes above this", "share of the MC value"],
        PotAdvice => ["Tick or cross in the POT", "box: circle here or not"],
        // ---- speed to fly ----
        TcSpeedToFly => ["Smooths the speed to fly.", "Higher = calmer commands"],
        VarioModeControl => ["What switches from vario", "to speed to fly mode"],
        StfSwitchMode => ["Auto: switch by the polar or", "at a fixed airspeed"],
        StfSwitchSpeed => ["Above this airspeed: speed", "to fly, below: vario"],
        SpeedToFlyPinConfig => ["How the switch on stick or", "flap lever is wired"],
        TcCircleHysteresis => [
            "Turning this long counts as",
            "circling (and straight again)",
        ],
        // ---- display, wind ----
        EnergyArrowMult => [
            "Red arrow: wind change near",
            "lift. Length factor, 0 = off",
        ],
        WindTc => ["Live wind looks back this", "long. Shorter = more nervous"],
        MeanWindTc => ["Mean wind looks back this", "long (grey arrow)"],
        GliderSymbol => ["Glider outline in the wind", "view in straight flight"],
        BigEncoder => ["Outer knob on a page:", "next page or set MC"],
        Theme => ["Dark or bright colours", ""],
        Brightness => ["Backlight. Lower saves power", ""],
        // ---- simple operation ----
        VarioResponse => ["How lively needle and tone", "are. Calm: gusts cut off"],
        CruiseSwitch => ["What switches from vario to", "speed to fly (cruise)"],
        SoundPreset => ["A ready made vario sound.", "Push to take it"],
        PageStyle => ["A ready made vario page", "(page 1). Push to take it"],
        Glider => ["Push to take it. Replaces", "masses and polar points"],
        PilotWeight => ["Both pilots in a two-seater.", "Goes into the speed to fly"],
        EmptyMass => ["Glider without water and", "pilot (weighing report)"],
        MaxBallast => ["Full water tanks. 0 for a", "glider without tanks"],
        UserProfile => ["Own settings for each pilot.", "Push to change over"],
        UsageMode => ["Basic: few settings. Expert:", "all. Push to take it"],
        Rotation => ["Turn the picture to match", "how the display sits"],
        UnitHorizontalSpeed => ["Unit of airspeed and wind", ""],
        UnitVerticalSpeed => ["Unit of the vario scale", "and of the climb rates"],
        UnitHeight => ["Unit of the altitude", ""],
        CmdMeas1 => [
            "Measured with the left wing",
            "on the ground. Repeat if not",
        ],
        CmdMeas2 => [
            "Measured with the right wing",
            "on the ground. Repeat if not",
        ],
        CmdMeas3 => ["Measured with the wings", "level. Repeat if not"],
        CmdCalcOrientation => [
            "Needs the three steps above.",
            "Then check the horizon page",
        ],
        CmdFineTuneOrientation => ["In calm air: straight, wings", "level, normal speed. Push"],
        SetupWizard => ["Asks for units, glider,", "weight and rotation again"],
        _ => return Option::None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_lines_are_short_enough() {
        // every Editable that has a help text: both lines within the limit
        for v in 0..=255u8 {
            // Editable is repr(u8) without gaps up to Return
            if v > Editable::Return as u8 {
                break;
            }
            let e: Editable = unsafe { core::mem::transmute(v) };
            if let Some(lines) = help(e) {
                for l in lines {
                    assert!(
                        l.len() <= HELP_MAX_CHARS,
                        "{:?}: '{}' has {} chars",
                        e,
                        l,
                        l.len()
                    );
                }
            }
        }
    }

    /// The display is round: a help line must fit the chord at its height.
    /// Measured with the font of the editor, for every setting.
    #[test]
    fn help_lines_fit_the_round_display() {
        extern crate std;
        use crate::view::ring_editor::{help_line_max_px, help_line_y};
        let (cm, _cc, _tx) = crate::test_harness!();
        let radius = cm.device_const.sizes.display.height as i32 / 2;
        let mut bad = std::vec::Vec::new();
        for v in 0..=Editable::Return as u8 {
            let e: Editable = unsafe { core::mem::transmute(v) };
            let Some(lines) = help(e) else { continue };
            let params = e.params(&cm);
            for (nr, l) in lines.iter().enumerate() {
                let y = help_line_y(&params, e, nr as i32);
                let w = crate::view::raster::text_w(crate::view::ring_editor::help_font(), l);
                let max = help_line_max_px(radius, y);
                if w > max {
                    bad.push(std::format!(
                        "{:?} line {}: '{}' {} px > {} px",
                        e,
                        nr + 1,
                        l,
                        w,
                        max
                    ));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
