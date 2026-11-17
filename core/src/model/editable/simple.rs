//! Editables of the simple operation (Stefan 15.11.2026): presets that set
//! several values at once, so the pilot answers one question instead of
//! many - vario response, cruise switch, sound preset, page style - and the
//! setup wizard.

use super::{CmdParams, Content, EditableFuncs, EnumParams, Params};
use crate::{
    controller::Waveform,
    model::{
        page::Page, GustFilter, PageCore, PageFrame, VarioModeControl, SINK_MODE_CONTINUOUS,
        SINK_MODE_DOUBLE_BEEP, SINK_MODE_OFF, STF_SWITCH_MODE_BALLAST, STF_SWITCH_MODE_FIXED,
        STF_SWITCH_MODE_POLAR,
    },
    persist,
    utils::{TString, Variant},
    view::viewable::vario_infoview::LineView,
    CenterView, CoreController, CoreModel, Echo, PersistenceId,
};

fn enum_of(s: &str) -> Content {
    Content::Enum(TString::<16>::from_str(s))
}

fn variants(v: [&'static str; 5]) -> Params {
    Params::Enum(EnumParams { variants: v })
}

// ------------------------------------------------------------ vario response

pub const RESPONSE_FAST: &str = "Fast";
pub const RESPONSE_NORMAL: &str = "Normal";
pub const RESPONSE_CALM: &str = "Calm";

/// How lively needle and tone are - one word for the gust filter: Fast =
/// off, Normal = Peak Cut (gust peaks cut, no delay), Calm = Medium
pub struct VarioResponse;
impl EditableFuncs for VarioResponse {
    fn name() -> &'static str {
        "Vario Response"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(match cm.config.gust_filter {
            GustFilter::Off => RESPONSE_FAST,
            GustFilter::PeakCut => RESPONSE_NORMAL,
            _ => RESPONSE_CALM,
        })
    }
    fn params(_cm: &CoreModel) -> Params {
        variants([RESPONSE_FAST, RESPONSE_NORMAL, RESPONSE_CALM, "", ""])
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let mode = match val.as_str() {
                RESPONSE_NORMAL => GustFilter::PeakCut,
                RESPONSE_CALM => GustFilter::Medium,
                _ => GustFilter::Off,
            };
            // Calm keeps a Weak / Strong chosen in the Expert menu
            if mode == GustFilter::Medium
                && matches!(cm.config.gust_filter, GustFilter::Weak | GustFilter::Strong)
            {
                return;
            }
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

// ------------------------------------------------------------- cruise switch

pub const CRUISE_AUTO: &str = "Auto";
pub const CRUISE_SPEED: &str = "At speed";
pub const CRUISE_STICK: &str = "Stick switch";
pub const CRUISE_NAV: &str = "Nav computer";
pub const CRUISE_CAN: &str = "2nd display";

/// what the cruise switch is set to, in the words of the menu
pub fn cruise_switch(cm: &CoreModel) -> &'static str {
    match cm.control.vario_mode_control {
        VarioModeControl::Auto if cm.config.stf_switch_mode == STF_SWITCH_MODE_POLAR => CRUISE_AUTO,
        VarioModeControl::Auto => CRUISE_SPEED,
        VarioModeControl::InputPin => CRUISE_STICK,
        VarioModeControl::Nmea => CRUISE_NAV,
        VarioModeControl::Can => CRUISE_CAN,
    }
}

/// When the display changes from the vario to the speed to fly: one choice
/// for Mode Control and Auto Switch of the Expert menu
pub struct CruiseSwitch;
impl EditableFuncs for CruiseSwitch {
    fn name() -> &'static str {
        "Cruise Switch"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(cruise_switch(cm))
    }
    fn params(_cm: &CoreModel) -> Params {
        variants([
            CRUISE_AUTO,
            CRUISE_SPEED,
            CRUISE_STICK,
            CRUISE_NAV,
            CRUISE_CAN,
        ])
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            if val.as_str() == cruise_switch(cm) {
                return;
            }
            let (control, switch) = match val.as_str() {
                CRUISE_SPEED => {
                    // keeps "IAS + Ballast" when that was set in the Expert menu
                    let s = if cm.config.stf_switch_mode == STF_SWITCH_MODE_BALLAST {
                        STF_SWITCH_MODE_BALLAST
                    } else {
                        STF_SWITCH_MODE_FIXED
                    };
                    (VarioModeControl::Auto, Some(s))
                }
                CRUISE_STICK => (VarioModeControl::InputPin, None),
                CRUISE_NAV => (VarioModeControl::Nmea, None),
                CRUISE_CAN => (VarioModeControl::Can, None),
                _ => (VarioModeControl::Auto, Some(STF_SWITCH_MODE_POLAR)),
            };
            persist::persist_set(
                cc,
                cm,
                Variant::U32(control as u32),
                PersistenceId::VarioModeControl,
                Echo::None,
            );
            if let Some(s) = switch {
                persist::persist_set(
                    cc,
                    cm,
                    Variant::U8(s),
                    PersistenceId::StfSwitchMode,
                    Echo::None,
                );
            }
        }
    }
}

// -------------------------------------------------------------- sound preset

pub const PRESET_STANDARD: &str = "Standard";
pub const PRESET_SOFT: &str = "Soft";
pub const PRESET_SHARP: &str = "Sharp";
pub const PRESET_CUSTOM: &str = "Custom";

/// the values a sound preset sets
#[derive(Clone, Copy)]
pub struct SoundSet {
    pub waveform: Waveform,
    pub center: f32,
    pub upper: f32,
    pub lower: f32,
    pub full_scale: f32,
    pub sink_mode: u8,
    pub sink_from: f32,
    pub ticks_until: f32,
    pub tempo: f32,
    pub damping: f32,
    pub attack: f32,
}

/// Standard = the delivery values. Soft: pure, deeper, slower, no sink tone.
/// Sharp: brighter, higher, faster, a steady sink tone - cuts through wind noise.
pub const SOUND_PRESETS: [(&str, SoundSet); 3] = [
    (
        PRESET_STANDARD,
        SoundSet {
            waveform: Waveform::Triangular,
            center: 500.0,
            upper: 1930.0,
            lower: 318.0,
            full_scale: 5.0,
            sink_mode: SINK_MODE_DOUBLE_BEEP,
            sink_from: -1.0,
            ticks_until: -0.5,
            tempo: 1.0,
            damping: 0.2,
            attack: 8.0,
        },
    ),
    (
        PRESET_SOFT,
        SoundSet {
            waveform: Waveform::SineWave,
            center: 400.0,
            upper: 1500.0,
            lower: 250.0,
            full_scale: 5.0,
            sink_mode: SINK_MODE_OFF,
            sink_from: -1.0,
            ticks_until: -0.5,
            tempo: 0.9,
            damping: 0.4,
            attack: 16.0,
        },
    ),
    (
        PRESET_SHARP,
        SoundSet {
            waveform: Waveform::Sawtooth,
            center: 600.0,
            upper: 2400.0,
            lower: 318.0,
            full_scale: 4.0,
            sink_mode: SINK_MODE_CONTINUOUS,
            sink_from: -1.0,
            ticks_until: -0.5,
            tempo: 1.2,
            damping: 0.1,
            attack: 4.0,
        },
    ),
];

fn sound_now(cm: &CoreModel) -> SoundSet {
    SoundSet {
        waveform: cm.calculated.sound_params.waveform,
        center: cm.config.snd_center_freq,
        upper: cm.config.snd_freq_upper,
        lower: cm.config.snd_freq_lower,
        full_scale: cm.config.snd_full_scale,
        sink_mode: cm.config.snd_sink_mode,
        sink_from: cm.config.snd_sink_from,
        ticks_until: cm.config.snd_ticks_until,
        tempo: cm.config.snd_beep_tempo,
        damping: cm.config.snd_audio_tc,
        attack: cm.config.snd_attack_ms,
    }
}

fn same(a: &SoundSet, b: &SoundSet) -> bool {
    let near = |x: f32, y: f32| (x - y).abs() < 0.01 + 0.002 * y.abs();
    a.waveform as u8 == b.waveform as u8
        && near(a.center, b.center)
        && near(a.upper, b.upper)
        && near(a.lower, b.lower)
        && near(a.full_scale, b.full_scale)
        && a.sink_mode == b.sink_mode
        && near(a.sink_from, b.sink_from)
        && near(a.ticks_until, b.ticks_until)
        && near(a.tempo, b.tempo)
        && near(a.damping, b.damping)
        && near(a.attack, b.attack)
}

/// name of the preset the sound settings equal, "Custom" if none
pub fn sound_preset(cm: &CoreModel) -> &'static str {
    let now = sound_now(cm);
    SOUND_PRESETS
        .iter()
        .find(|(_, p)| same(&now, p))
        .map(|(n, _)| *n)
        .unwrap_or(PRESET_CUSTOM)
}

/// set the sound values; `store`: for good (EEPROM), else only in the
/// model, to be heard in the preview
fn apply_sound(cm: &mut CoreModel, cc: &mut CoreController, p: SoundSet, store: bool) {
    let items = [
        (Variant::U8(p.waveform as u8), PersistenceId::Waveform),
        (Variant::F32(p.center), PersistenceId::CenterFreqStefly),
        (Variant::F32(p.upper), PersistenceId::SndFreqUpper),
        (Variant::F32(p.lower), PersistenceId::SndFreqLower),
        (Variant::F32(p.full_scale), PersistenceId::SndFullScale),
        (Variant::U8(p.sink_mode), PersistenceId::SinkToneMode),
        (Variant::F32(p.sink_from), PersistenceId::SinkToneFrom),
        (Variant::F32(p.ticks_until), PersistenceId::TicksUntil),
        (Variant::F32(p.tempo), PersistenceId::BeepTempo),
        (Variant::F32(p.damping), PersistenceId::AudioDamping),
        (Variant::F32(p.attack), PersistenceId::SoundAttack),
    ];
    for (v, id) in items {
        if store {
            persist::persist_set(cc, cm, v, id, Echo::None);
        } else {
            let restored = cm.control.items_restored;
            let _ = persist::restore_item(cc, cm, crate::PersistenceItem::from_variant(id, v));
            cm.control.items_restored = restored;
        }
    }
}

/// The preset editor has closed (or is opened again): a preset that was
/// only listened to gives way to the sound that was set before.
pub fn sound_preset_closed(cm: &mut CoreModel, cc: &mut CoreController) {
    if let Some(before) = cm.control.sound_backup.take() {
        apply_sound(cm, cc, before, false);
    }
}

/// One of three ready made sounds. Turning through the list plays each one
/// in the sound preview (on the ground); only a push takes it - without a
/// push the sound set before comes back, values set one by one in the
/// Expert menu included ("Custom").
pub struct SoundPreset;
impl EditableFuncs for SoundPreset {
    fn name() -> &'static str {
        "Sound Preset"
    }
    fn content(cm: &mut CoreModel, cc: &mut CoreController) -> Content {
        // left over from an editor that did not close the normal way
        sound_preset_closed(cm, cc);
        enum_of(sound_preset(cm))
    }
    fn params(cm: &CoreModel) -> Params {
        if sound_preset(cm) == PRESET_CUSTOM {
            variants([
                PRESET_STANDARD,
                PRESET_SOFT,
                PRESET_SHARP,
                PRESET_CUSTOM,
                "",
            ])
        } else {
            variants([PRESET_STANDARD, PRESET_SOFT, PRESET_SHARP, "", ""])
        }
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            let before = match cm.control.sound_backup {
                Some(b) => b,
                Option::None => sound_now(cm),
            };
            // "Custom" in the list: the values that were set before
            let chosen = SOUND_PRESETS
                .iter()
                .find(|(n, _)| *n == val.as_str())
                .map(|(_, p)| *p)
                .unwrap_or(before);
            if cm.control.editor.enter_pushed {
                cm.control.sound_backup = Option::None;
                // nothing new: nothing to write
                apply_sound(cm, cc, chosen, !same(&chosen, &before));
            } else {
                cm.control.sound_backup = Some(before);
                apply_sound(cm, cc, chosen, false);
            }
        }
    }
}

// ---------------------------------------------------------------- page style

pub const STYLE_STANDARD: &str = "Standard";
pub const STYLE_SIMPLE: &str = "Simple";
pub const STYLE_BIG: &str = "Big numbers";
pub const STYLE_COMPETITION: &str = "Competition";
pub const STYLE_CUSTOM: &str = "Custom";

/// the page a style makes of `page` (outer band and flap tape stay)
pub fn styled_page(style: &str, page: Page) -> Option<Page> {
    use LineView::*;
    let mut p = page;
    p.frame = PageFrame::Ring360;
    match style {
        // the page of a new device (model/page.rs default_pages)
        STYLE_STANDARD => {
            let d = crate::model::page::default_pages()[0];
            p.core = d.core;
            p.boxes = d.boxes;
            p.stf_boxes = d.stf_boxes;
            p.cruise_boxes = d.cruise_boxes;
            p.center_circling = d.center_circling;
            p.center_straight = d.center_straight;
        }
        STYLE_SIMPLE => {
            p.core = PageCore::Boxes;
            p.boxes = [Altitude, AverageClimbRate, None, None, None];
            p.cruise_boxes = false;
            p.center_circling = CenterView::ThermalAssistant1;
            p.center_straight = CenterView::SingleArrowStraight;
        }
        STYLE_BIG => {
            p.core = PageCore::TwoValues;
            p.boxes = [None, None, AverageClimbRate, Altitude, None];
            p.cruise_boxes = false;
        }
        STYLE_COMPETITION => {
            p.core = PageCore::Boxes;
            p.boxes = [Altitude, ThermalClimb, WindAndDelta, AverageClimbRate, None];
            p.stf_boxes = [Altitude, AverageClimbRate, WindAndDelta, SpeedToFly, None];
            p.cruise_boxes = true;
            p.center_circling = CenterView::ThermalAssistant2;
            p.center_straight = CenterView::DoubleArrowStraight;
        }
        _ => return Option::None,
    }
    p.stf_boxes = if p.cruise_boxes { p.stf_boxes } else { p.boxes };
    Some(p)
}

/// The style belongs to the first page, the vario page - not to the page
/// that happens to be shown (a style taken on the horizon page would
/// replace the horizon).
pub const STYLE_PAGE: usize = 0;

/// name of the style the first page equals, "Custom" if none
pub fn page_style(cm: &CoreModel) -> &'static str {
    let page = cm.config.pages[STYLE_PAGE];
    for s in [STYLE_STANDARD, STYLE_SIMPLE, STYLE_BIG, STYLE_COMPETITION] {
        if styled_page(s, page) == Some(page) {
            return s;
        }
    }
    STYLE_CUSTOM
}

/// A ready made layout for the first page. Taken with a push only.
pub struct PageStyle;
impl EditableFuncs for PageStyle {
    fn name() -> &'static str {
        "Page Style"
    }
    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        enum_of(page_style(cm))
    }
    fn params(cm: &CoreModel) -> Params {
        if page_style(cm) == STYLE_CUSTOM {
            variants([
                STYLE_STANDARD,
                STYLE_SIMPLE,
                STYLE_BIG,
                STYLE_COMPETITION,
                STYLE_CUSTOM,
            ])
        } else {
            variants([
                STYLE_STANDARD,
                STYLE_SIMPLE,
                STYLE_BIG,
                STYLE_COMPETITION,
                "",
            ])
        }
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            if !cm.control.editor.enter_pushed {
                return;
            }
            let i = STYLE_PAGE;
            if let Some(p) = styled_page(val.as_str(), cm.config.pages[i]) {
                if p != cm.config.pages[i] {
                    cm.config.pages[i] = p;
                    // the three words of this page go to the EEPROM (page_layout::save_tick)
                    cc.page_save |= 0b111 << (i * 3);
                }
                // and it is the page that shows when the setup closes
                cm.config.page_idx = i as u8;
            }
        }
    }
}

// -------------------------------------------------------------- setup wizard

pub struct SetupWizard;
impl EditableFuncs for SetupWizard {
    fn name() -> &'static str {
        "Setup Wizard"
    }
    fn content(_cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        Content::Command(TString::<16>::from_str(""))
    }
    fn params(_cm: &CoreModel) -> Params {
        Params::Cmd(CmdParams {
            content: TString::<16>::from_str(""),
        })
    }
    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, _content: Content) {
        crate::controller::wizard::start(cm, cc);
    }
}
