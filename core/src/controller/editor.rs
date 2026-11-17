use crate::{
    basic_config::{MENU_TIMEOUT, SECTION_EDITOR_TIMEOUT},
    controller::{close_menu_display, helpers::IntToDuration, KeyEvent, Timer},
    model::{editable::*, menu::ROOT, DisplayActive, EditMode, OverlayActive, PitotCalState},
    persist,
    utils::TString,
    utils::Variant,
    CanConfigId, CoreController, CoreModel, Echo, Editable, PersistenceId, RemoteConfig,
};
use num::clamp;

/// A changed sensor box value waiting for the user's confirmation
#[derive(Clone, Copy)]
pub struct PendingRemote {
    /// the edited item (for the dialog text)
    pub target: Editable,
    pub config_id: CanConfigId,
    pub old: Content,
    pub new: Content,
}

fn edit_cmd_content(
    _cm: &mut CoreModel,
    _cc: &mut CoreController,
    key_event: &mut KeyEvent,
    _target: Editable,
    _params: &CmdParams,
) {
    // There is nothing to do here, Cmd is sent when activating
    *key_event = KeyEvent::NoEvent
}

fn edit_date_content(
    cm: &mut CoreModel,
    cc: &mut CoreController,
    key_event: &mut KeyEvent,
    target: Editable,
    params: &DateParams,
) {
    if params.is_active {
        if let Content::Date(Some(mut date)) = cm.control.editor.content {
            if date >= params.min && date <= params.max {
                match key_event {
                    KeyEvent::Rotary2Left => date.add_days(params.small_inc_minus as i32),
                    KeyEvent::Rotary2Right => date.add_days(params.small_inc_plus as i32),
                    KeyEvent::Rotary1Left => date.add_days(params.big_inc_minus as i32),
                    KeyEvent::Rotary1Right => date.add_days(params.big_inc_plus as i32),
                    KeyEvent::BtnEnc => (),
                    _ => return,
                }
            } else {
                if *key_event == KeyEvent::BtnEnc {
                    return;
                } else {
                    date = params.min;
                }
            }
            let date = clamp(date, params.min, params.max);
            let content = Content::Date(Some(date));
            cm.control.editor.content = content;
            cm.control.editor.touched = true;
            target.set_content(cm, cc, content);
        }
    }
    *key_event = KeyEvent::NoEvent;
}

fn edit_enum_content(
    cm: &mut CoreModel,
    cc: &mut CoreController,
    key_event: &mut KeyEvent,
    target: Editable,
    params: &EnumParams,
) {
    if let Content::Enum(val) = cm.control.editor.content {
        let mut idx = MAX_ENUM_VARIANTS as isize;
        let mut max = 0_isize;
        while idx > 0 {
            idx -= 1;
            if max == 0 && !params.variants[idx as usize].is_empty() {
                max = idx;
            }
            if val.as_str() == params.variants[idx as usize] {
                break;
            }
        }
        match key_event {
            KeyEvent::Rotary2Left => idx -= 1,
            KeyEvent::Rotary2Right => idx += 1,
            KeyEvent::BtnEnc => (),
            _ => return,
        }
        let idx = clamp(idx, 0, max) as usize;
        let val = TString::<16>::from_str(params.variants[idx]);
        let content = Content::Enum(val);
        cm.control.editor.content = content;
        cm.control.editor.touched = true;
        target.set_content(cm, cc, content);
        *key_event = KeyEvent::NoEvent
    }
}

fn edit_f32_content(
    cm: &mut CoreModel,
    cc: &mut CoreController,
    key_event: &mut KeyEvent,
    target: Editable,
    params: &F32Params,
) {
    if let Content::F32(opt_val) = cm.control.editor.content {
        if let Some(mut val) = opt_val {
            match key_event {
                KeyEvent::Rotary2Left => val -= params.small_inc,
                KeyEvent::Rotary2Right => val += params.small_inc,
                KeyEvent::Rotary1Left => val -= params.big_inc,
                KeyEvent::Rotary1Right => val += params.big_inc,
                KeyEvent::BtnEnc => (),
                _ => return,
            }
            let mut val = clamp(val, params.min, params.max);
            // GNSS Config: the products Essential / Competition / Competition
            // Pro are 1 / 2 / 4, the F9P + F9H set-up (3) is not offered
            if target == Editable::GnssConfig
                && *key_event != KeyEvent::BtnEnc
                && (val - 3.0).abs() < 0.5
            {
                let old = opt_val.unwrap_or(val);
                val = if old < 3.0 { 4.0 } else { 2.0 };
            }
            let content = Content::F32(Some(val));
            cm.control.editor.content = content;
            cm.control.editor.touched = true;
            target.set_content(cm, cc, content);
        }
        *key_event = KeyEvent::NoEvent
    }
}

fn edit_list_content(
    cm: &mut CoreModel,
    cc: &mut CoreController,
    key_event: &mut KeyEvent,
    target: Editable,
    params: &ListParams,
) {
    if let Content::List(mut val) = cm.control.editor.content {
        match key_event {
            KeyEvent::Rotary2Left => val -= 1,
            KeyEvent::Rotary2Right => val += 1,
            KeyEvent::Rotary1Left => val -= 10,
            KeyEvent::Rotary1Right => val += 10,
            KeyEvent::BtnEnc => (),
            _ => return,
        }
        let val = clamp(val, 0, params.max);
        let content = Content::List(val);
        cm.control.editor.content = content;
        cm.control.editor.touched = true;
        target.set_content(cm, cc, content);
        *key_event = KeyEvent::NoEvent
    }
}

pub fn key_action(key_event: &mut KeyEvent, cm: &mut CoreModel, cc: &mut CoreController) {
    // the name entry has its own keys (alphabet in the ring)
    if crate::controller::name_entry::active(cm) {
        crate::controller::name_entry::key_action(key_event, cm, cc);
        return;
    }
    if cm.control.editor.mode != EditMode::Off {
        // a running pitot offset measurement ignores the push (a long push
        // aborts it)
        if *key_event == KeyEvent::BtnEnc
            && cm.control.editor.target == Editable::PitotOffsetMeasure
            && matches!(
                cm.control.pitot_cal.state,
                PitotCalState::Prepare(_) | PitotCalState::Settle(_) | PitotCalState::Measure(_)
            )
        {
            *key_event = KeyEvent::NoEvent;
            return;
        }
        if cm.control.editor.inline {
            // editing in the flight menu: the menu stays open meanwhile
            cc.scheduler.after(Timer::CloseMenu, MENU_TIMEOUT.secs());
            if *key_event == KeyEvent::BtnEsc {
                *key_event = KeyEvent::BtnEnc;
            }
        }
        match key_event {
            KeyEvent::BtnEnc => {
                cm.control.editor.enter_pushed = true;
                let _ = cc.scheduler.stop(Timer::CloseEditFrame, true); // finish edit session
            }
            KeyEvent::BtnEncS3 => {
                let _ = cc.scheduler.stop(Timer::CloseEditFrame, true); // finish edit session
                close_menu_display(cm, cc); // close also menu and return to standard display
                *key_event = KeyEvent::NoEvent;
            }
            _ => cc
                .scheduler
                .after(crate::Timer::CloseEditFrame, editor_timeout(cm).secs()),
        }

        let target = cm.control.editor.target;

        // change volume and mc cready independently of the selection -
        // but ONLY for the main page quick adjust; an editor opened from
        // a menu keeps both encoders on its own value (small/big steps)
        if !cm.control.editor.from_menu {
            match target {
                Editable::Volume => {
                    if *key_event == KeyEvent::Rotary1Left || *key_event == KeyEvent::Rotary1Right {
                        activate_editable(Editable::McCready, cm, cc);
                        *key_event = KeyEvent::NoEvent;
                    }
                }
                Editable::McCready => {
                    if *key_event == KeyEvent::Rotary2Left || *key_event == KeyEvent::Rotary2Right {
                        activate_editable(Editable::Volume, cm, cc);
                        *key_event = KeyEvent::NoEvent;
                    }
                }
                _ => (),
            }
        }

        match cm.control.editor.params {
            Params::Date(params) => edit_date_content(cm, cc, key_event, target, &params),
            Params::Enum(params) => edit_enum_content(cm, cc, key_event, target, &params),
            Params::String(_) => (),
            Params::List(params) => edit_list_content(cm, cc, key_event, target, &params),
            Params::F32(params) => edit_f32_content(cm, cc, key_event, target, &params),
            Params::Cmd(params) => edit_cmd_content(cm, cc, key_event, target, &params),
        }
        // in the menu row both knobs belong to the value: a knob the value
        // does not use (enums: the outer one) must not move the menu cursor
        if cm.control.editor.inline
            && matches!(
                *key_event,
                KeyEvent::Rotary1Left
                    | KeyEvent::Rotary1Right
                    | KeyEvent::Rotary2Left
                    | KeyEvent::Rotary2Right
            )
        {
            *key_event = KeyEvent::NoEvent;
        }
    }
    if cm.config.display_active != DisplayActive::Menu
        && cm.config.overlay_active != OverlayActive::Menu
    {
        match key_event {
            KeyEvent::Rotary1Left | KeyEvent::Rotary1Right => {
                // big encoder: page switching (MC then only via flight menu)
                if cm.config.big_encoder_pages {
                    switch_page(cm, cc, *key_event == KeyEvent::Rotary1Right);
                } else {
                    activate_editable(Editable::McCready, cm, cc);
                }
                *key_event = KeyEvent::NoEvent;
            }
            KeyEvent::Rotary2Left | KeyEvent::Rotary2Right => {
                if cm.config.display_active != DisplayActive::DeviceInfo {
                    // the volume panel, the slider of the tone heard now
                    // already changes with this click
                    let step = if *key_event == KeyEvent::Rotary2Right {
                        1
                    } else {
                        -1
                    };
                    crate::controller::volume_panel::open_with_step(cm, cc, step);
                    *key_event = KeyEvent::NoEvent;
                }
            }
            // push and turn keeps working as a second way to change pages
            KeyEvent::BtnEncPlusRotary2Left => switch_page(cm, cc, false),
            KeyEvent::BtnEncPlusRotary2Right => switch_page(cm, cc, true),
            KeyEvent::Btn1 => activate_editable(Editable::McCready, cm, cc),
            KeyEvent::Btn2 => activate_editable(Editable::WaterBallast, cm, cc),
            KeyEvent::Btn3 => activate_editable(Editable::PilotWeight, cm, cc),
            _ => (),
        }
    }
}

/// Switch to the next/previous configured page (big encoder).
///
/// The page decides what is shown; DisplayActive keeps its meaning as the
/// KIND of screen, so all existing logic (menus, key handling, editors)
/// works unchanged.
pub fn switch_page(cm: &mut CoreModel, cc: &mut CoreController, forward: bool) {
    let idx = cm.config.next_page(forward);
    if idx == cm.config.page_idx {
        return;
    }
    cm.config.page_idx = idx;
    apply_page(cm);
    // show the page indicator for a moment
    cm.control.page_indicator = crate::basic_config::PAGE_INDICATOR_TICKS;
    persist::persist_set(cc, cm, Variant::U8(idx), PersistenceId::PageIdx, Echo::None);
}

/// Set the screen kind from the active page
pub fn apply_page(cm: &mut CoreModel) {
    use crate::model::{PageCore, PageFrame, MAX_PAGES};
    // never sit on a page that is switched off
    if !cm.config.page().is_on() {
        let mut found = false;
        for i in 0..MAX_PAGES {
            if cm.config.pages[i].is_on() {
                cm.config.page_idx = i as u8;
                found = true;
                break;
            }
        }
        if !found {
            cm.config.pages[0].frame = PageFrame::Ring360;
            cm.config.page_idx = 0;
        }
    }
    // The FRAME decides which screen is used, the core only fills it:
    // with a Ring frame the vario view draws the scale and puts
    // the core inside; only "Full" hands the whole display to the core.
    let core = cm.config.page().core;
    let frame = cm.config.page().frame;
    cm.config.display_active = match core {
        // a list of values cannot live in a circle - always full screen
        PageCore::DeviceInfo => DisplayActive::DeviceInfo,
        PageCore::Horizon if frame == PageFrame::Full => DisplayActive::Horizon,
        _ => DisplayActive::Vario,
    };
}

/// Seconds without a key until an editor closes by itself: the quick
/// editors on the page 4 s as before; an editor opened in the setup menu
/// stays as long as the menu (30 s) - there is a help text to read and the
/// sound preview to listen to (15.11.2026: after 4 s both were gone).
pub fn editor_timeout(cm: &CoreModel) -> u16 {
    if cm.config.display_active == DisplayActive::Menu {
        MENU_TIMEOUT
    } else {
        SECTION_EDITOR_TIMEOUT
    }
}

pub fn activate_editable(editable: Editable, cm: &mut CoreModel, cc: &mut CoreController) {
    cm.control.editor.from_menu = false; // the menu controller overrides this
    cm.control.editor.inline = false; // the flight menu sets it after this
    cm.control.editor.target = editable;
    cm.control.editor.params = editable.params(cm);
    cm.control.editor.content = editable.content(cm, cc);
    cm.control.editor.enter_pushed = false;
    cm.control.editor.touched = false;
    if cm.config.display_active == DisplayActive::Menu {
        // the setup editors use the round inner circle overlay as well,
        // so everything looks alike
        cm.control.editor.mode = if cm.device_const.misc.edit_mode == EditMode::CircleArea {
            EditMode::CircleArea
        } else {
            EditMode::Fullscreen
        }
    } else {
        cm.control.editor.mode = cm.device_const.misc.edit_mode;
    }
    cm.config.overlay_active = OverlayActive::Editor;
    cc.scheduler
        .after(crate::Timer::CloseEditFrame, editor_timeout(cm).secs());

    // a text opens the name entry: longer time-out, alphabet in the ring
    if let Params::String(_) = cm.control.editor.params {
        cm.control.editor.name = crate::controller::name_entry::NameEntry::start(editable);
        if let crate::model::editable::Content::String(t) = cm.control.editor.content {
            let s = crate::controller::name_entry::start_text(t.as_str());
            cm.control.editor.content =
                crate::model::editable::Content::String(crate::utils::TString::<12>::from_str(s));
        }
        cc.scheduler.after(
            crate::Timer::CloseEditFrame,
            crate::controller::name_entry::NAME_TIMEOUT_S.secs(),
        );
    }

    // a command is executed when activating it - not during edit session
    if let Params::Cmd(_content) = cm.control.editor.params {
        editable.set_content(cm, cc, cm.control.editor.content);
    }
}

/// Open one of the confirmation dialogs of the sensor box settings in place
/// of the editor that is being closed
fn open_dialog(dialog: Editable, cm: &mut CoreModel, cc: &mut CoreController) {
    let from_menu = cm.control.editor.from_menu;
    activate_editable(dialog, cm, cc);
    cm.control.editor.from_menu = from_menu;
}

/// The "save old > new?" dialog for a value that was not typed in but
/// measured (pitot offset): the controller opens it once the result is in
pub fn open_remote_save(cm: &mut CoreModel, cc: &mut CoreController, pending: PendingRemote) {
    cm.control.editor.pending = Some(pending);
    open_dialog(Editable::RemoteSave, cm, cc);
}

fn contents_differ(a: Content, b: Content) -> bool {
    match (a, b) {
        (Content::F32(Some(x)), Content::F32(Some(y))) => x != y,
        (Content::F32(None), Content::F32(None)) => false,
        (Content::Date(Some(x)), Content::Date(Some(y))) => x != y,
        (Content::Date(None), Content::Date(None)) => false,
        _ => true,
    }
}

/// Sensor box settings are not written while turning the knob. Enter on a
/// changed value opens the "save old -> new?" dialog; only Save sends the
/// value to the box, and for values the box reads at start-up only (sensor
/// tilt, GNSS config) a second dialog offers to restart the box. A time-out
/// or Discard leaves the box untouched. Returns true when a dialog was
/// opened - the edit frame then stays open.
fn remote_confirm_step(cm: &mut CoreModel, cc: &mut CoreController) -> bool {
    let target = cm.control.editor.target;
    let entered = cm.control.editor.enter_pushed;
    let choice = if let Content::Enum(v) = cm.control.editor.content {
        v
    } else {
        TString::<16>::new()
    };
    match target {
        Editable::RemoteSave => {
            let pending = cm.control.editor.pending.take();
            let saved = entered && choice.as_str() == REMOTE_SAVE && pending.is_some();
            if let Some(p) = pending {
                if saved {
                    persist::set_remote_config(cm, cc, p.config_id, p.new);
                }
            }
            // a measured pitot offset: the box holds the shifted value until
            // it is saved or restored
            crate::controller::pitot_cal::dialog_closed(cm, cc, saved);
            if let Some(p) = pending {
                if saved && p.config_id.needs_restart() {
                    cm.control.editor.pending = Some(p);
                    open_dialog(Editable::RemoteRestart, cm, cc);
                    return true;
                }
            }
            false
        }
        Editable::RemoteRestart => {
            cm.control.editor.pending = None;
            if entered && choice.as_str() == REMOTE_RESTART {
                // the SET waits for the persistence timeout - it has to be on
                // the bus before the reset command, or the box restarts with
                // the old value
                if cc.remote_val.is_some() {
                    persist::store_persistence_ids(cm, cc);
                }
                persist::send_can_config_frame(cm, cc, CanConfigId::CmdReset, RemoteConfig::Get);
            }
            false
        }
        _ => {
            // a value editable of the sensor box (commands have no config id)
            if let Some(config_id) = target.remote_config_id() {
                let old = persist::remote_content(cm, config_id);
                let new = cm.control.editor.content;
                if entered && contents_differ(old, new) {
                    cm.control.editor.pending = Some(PendingRemote {
                        target,
                        config_id,
                        old,
                        new,
                    });
                    open_dialog(Editable::RemoteSave, cm, cc);
                    return true;
                }
            }
            false
        }
    }
}

pub fn close_edit_frame(cm: &mut CoreModel, cc: &mut CoreController) {
    if remote_confirm_step(cm, cc) {
        return;
    }
    // a pitot offset measurement that is still running is aborted
    crate::controller::pitot_cal::editor_closed(cm, cc);
    // Close Editor if open
    cm.control.editor.mode = EditMode::Off;
    cm.control.editor.inline = false;
    cm.config.overlay_active = OverlayActive::None;

    // an editor opened from a menu returns to THAT menu, not to the vario
    // page: the setup menu is the primary view anyway, the flight menu is
    // an overlay and has to be switched back on
    if cm.control.editor.from_menu && cm.control.menu_control.menu != &ROOT {
        if cm.config.display_active != DisplayActive::Menu {
            cm.config.overlay_active = OverlayActive::Menu;
        }
        cc.scheduler.after(Timer::CloseMenu, MENU_TIMEOUT.secs());
        crate::controller::menu::fill_values(cm, cc);
    } else if cm.config.display_active == DisplayActive::Menu {
        // ring style menus show the values in the list - refresh them
        crate::controller::menu::fill_values(cm, cc);
    }
    // a sound preset that was only listened to
    if cm.control.editor.target == Editable::SoundPreset {
        crate::model::editable::simple::sound_preset_closed(cm, cc);
    }
    // setup wizard: a push takes the value and opens the next step
    let pushed = cm.control.editor.enter_pushed;
    crate::controller::wizard::editor_closed(cm, cc, pushed);
}

#[derive(Clone, Copy)]
pub struct Editor {
    pub target: Editable,
    pub mode: EditMode,
    pub params: Params,
    pub content: Content,
    pub enter_pushed: bool,
    /// editor was opened from a menu: both encoders edit THIS value
    /// (small = small steps, big = big steps) and the main page quick
    /// adjust switching between MC and volume is disabled
    pub from_menu: bool,
    /// the user has changed the value in this edit session - an answer of
    /// the sensor box arriving now must not overwrite it
    pub touched: bool,
    /// a changed sensor box value waiting for confirmation (dialogs)
    pub pending: Option<PendingRemote>,
    /// a sensor box value whose SET is still on its way: answers of the box
    /// (with the old value) must not overwrite the cache meanwhile
    pub set_pending: Option<CanConfigId>,
    /// state of the name entry (Params::String)
    pub name: crate::controller::name_entry::NameEntry,
    /// the value is edited right in its row of the flight menu (no editor
    /// screen): push - turn - push (Stefan 28.10.2026)
    pub inline: bool,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            target: Editable::None,
            mode: EditMode::Off,
            params: Params::String(StringParams {
                content: TString::<16>::from_str(""),
            }),
            content: Content::String(TString::new()),
            enter_pushed: false,
            from_menu: false,
            touched: false,
            pending: None,
            set_pending: None,
            name: crate::controller::name_entry::NameEntry::new(),
            inline: false,
        }
    }

    pub fn get_head_line(&self) -> &'static str {
        self.target.name()
    }

    pub fn get_value_line(&self, cm: &CoreModel) -> TString<20> {
        self.target.content_as_str(self.content, cm)
    }
}

/// the F32 step of the editor for tests in other modules
#[cfg(test)]
pub fn edit_f32_for_test(cm: &mut CoreModel, cc: &mut CoreController, key_event: &mut KeyEvent) {
    if let crate::model::editable::Params::F32(p) = cm.control.editor.params {
        let target = cm.control.editor.target;
        edit_f32_content(cm, cc, key_event, target, &p);
    }
}
