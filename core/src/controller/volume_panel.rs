//! Volume panel: three sliders - circling (vario tone), cruise (speed to
//! fly tone) and warnings (Stefan 28.10.2026). Drawing: view/volume_panel.rs.
//!
//! - on the page, a turn of the inner knob opens it with the slider that
//!   is heard now (warning sounding -> warnings, speed to fly -> cruise,
//!   else circling) and already changes it; the outer knob picks another
//!   slider; a push or a pause ends it
//! - from the flight menu ("Volume") it opens with the cursor on that
//!   slider: turn to pick one, push to change it, push again: done

use crate::{
    basic_config::MENU_TIMEOUT,
    controller::{helpers::IntToDuration, persist, Timer},
    model::{menu::ROOT, OverlayActive},
    utils::Variant,
    CoreController, CoreModel, Echo, KeyEvent, PersistenceId, VarioMode,
};

pub const ROWS: u8 = 3;
pub const ROW_CIRCLING: u8 = 0;
pub const ROW_CRUISE: u8 = 1;
pub const ROW_WARNING: u8 = 2;
pub const VOLUME_MAX: i8 = 50;

/// 100 ms ticks without a key until the panel closes
const TIMEOUT_EDIT: u8 = 40;
const TIMEOUT_SELECT: u8 = 80;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct VolumePanel {
    pub active: bool,
    /// slider the cursor is on
    pub sel: u8,
    /// the knob changes the slider (false: the knob moves the cursor)
    pub edit: bool,
    /// opened from the flight menu (it comes back when the panel closes)
    pub from_menu: bool,
    pub timer: u8,
}

impl VolumePanel {
    pub const fn new() -> Self {
        VolumePanel {
            active: false,
            sel: 0,
            edit: false,
            from_menu: false,
            timer: 0,
        }
    }
}

pub fn active(cm: &CoreModel) -> bool {
    cm.control.volume_panel.active
}

/// volume of a slider
pub fn value(cm: &CoreModel, row: u8) -> i8 {
    match row {
        ROW_CIRCLING => cm.config.volume,
        ROW_CRUISE => cm.config.stf_volume(),
        _ => cm.control.alarm_volume,
    }
}

fn set(cm: &mut CoreModel, cc: &mut CoreController, row: u8, v: i8) {
    let v = v.clamp(0, VOLUME_MAX);
    match row {
        // the circling volume is the volume of the other displays and the
        // sensor box as before
        ROW_CIRCLING => persist::persist_set(
            cc,
            cm,
            Variant::I8(v),
            PersistenceId::Volume,
            Echo::NmeaAndCan,
        ),
        ROW_CRUISE => {
            persist::persist_set(cc, cm, Variant::I8(v), PersistenceId::VolumeStf, Echo::None)
        }
        _ => persist::persist_set(
            cc,
            cm,
            Variant::I8(v),
            PersistenceId::AlarmVolume,
            Echo::None,
        ),
    }
}

/// the slider of the tone that is heard now
pub fn auto_row(cm: &CoreModel, cc: &CoreController) -> u8 {
    if cc.sound_control.alarm_active() {
        ROW_WARNING
    } else if cm.control.vario_mode == VarioMode::SpeedToFly {
        ROW_CRUISE
    } else {
        ROW_CIRCLING
    }
}

/// open the panel; `edit`: the knob changes the slider at once
pub fn open(cm: &mut CoreModel, cc: &mut CoreController, edit: bool, from_menu: bool) {
    let sel = auto_row(cm, cc);
    let p = &mut cm.control.volume_panel;
    p.active = true;
    p.sel = sel;
    p.edit = edit;
    p.from_menu = from_menu;
    p.timer = if edit { TIMEOUT_EDIT } else { TIMEOUT_SELECT };
    if from_menu {
        // the menu is hidden meanwhile and comes back afterwards
        cm.config.overlay_active = OverlayActive::None;
        let _ = cc.scheduler.stop(Timer::CloseMenu, false);
    }
}

/// a turn of the inner knob on the page: open and change at once
pub fn open_with_step(cm: &mut CoreModel, cc: &mut CoreController, step: i32) {
    open(cm, cc, true, false);
    let row = cm.control.volume_panel.sel;
    let v = value(cm, row) as i32 + step;
    set(cm, cc, row, v as i8);
}

fn close(cm: &mut CoreModel, cc: &mut CoreController) {
    let from_menu = cm.control.volume_panel.from_menu;
    cm.control.volume_panel = VolumePanel::new();
    if from_menu && cm.control.menu_control.menu != &ROOT {
        cm.config.overlay_active = OverlayActive::Menu;
        cc.scheduler.after(Timer::CloseMenu, MENU_TIMEOUT.secs());
        crate::controller::menu::fill_values(cm, cc);
    }
}

pub fn tick_100ms(cm: &mut CoreModel, cc: &mut CoreController) {
    if !active(cm) {
        return;
    }
    if cm.control.volume_panel.timer > 0 {
        cm.control.volume_panel.timer -= 1;
    }
    if cm.control.volume_panel.timer == 0 {
        close(cm, cc);
    }
}

/// Keys while the panel is shown: it takes all of them
pub fn key_action(key_event: &mut KeyEvent, cm: &mut CoreModel, cc: &mut CoreController) {
    if !active(cm) || *key_event == KeyEvent::NoEvent {
        return;
    }
    let key = *key_event;
    *key_event = KeyEvent::NoEvent;
    let edit = cm.control.volume_panel.edit;
    cm.control.volume_panel.timer = if edit { TIMEOUT_EDIT } else { TIMEOUT_SELECT };
    let step = |k: KeyEvent| match k {
        KeyEvent::Rotary1Right | KeyEvent::Rotary2Right | KeyEvent::BtnEncPlusRotary2Right => 1,
        KeyEvent::Rotary1Left | KeyEvent::Rotary2Left | KeyEvent::BtnEncPlusRotary2Left => -1,
        _ => 0,
    };
    let move_sel = |cm: &mut CoreModel, d: i32| {
        let s = (cm.control.volume_panel.sel as i32 + d).clamp(0, ROWS as i32 - 1);
        cm.control.volume_panel.sel = s as u8;
    };
    match key {
        // inner knob: the value (or the cursor while picking)
        KeyEvent::Rotary2Left
        | KeyEvent::Rotary2Right
        | KeyEvent::BtnEncPlusRotary2Left
        | KeyEvent::BtnEncPlusRotary2Right => {
            if edit {
                let row = cm.control.volume_panel.sel;
                let v = value(cm, row) as i32 + step(key);
                set(cm, cc, row, v as i8);
            } else {
                move_sel(cm, step(key));
            }
        }
        // outer knob: the other sliders
        KeyEvent::Rotary1Left | KeyEvent::Rotary1Right => move_sel(cm, step(key)),
        KeyEvent::BtnEnc => {
            if edit {
                close(cm, cc);
            } else {
                cm.control.volume_panel.edit = true;
                cm.control.volume_panel.timer = TIMEOUT_EDIT;
            }
        }
        KeyEvent::BtnEsc => close(cm, cc),
        KeyEvent::BtnEncS3 => {
            close(cm, cc);
            crate::controller::close_menu_display(cm, cc);
        }
        _ => (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{test_harness, Event};

    fn key(cm: &mut CoreModel, cc: &mut CoreController, t: &mut u16, k: KeyEvent) {
        cc.event_handler(Event::KeyItem(k), cm);
        for _ in 0..20 {
            *t = t.wrapping_add(1);
            cc.tick_1ms(*t, cm);
        }
    }

    #[test]
    fn a_turn_on_the_page_changes_the_tone_that_is_heard() {
        let (mut cm, mut cc, _tx) = test_harness!();
        let mut t = 0u16;
        key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary1Left); // nothing
        cm.control.vario_mode = VarioMode::Vario;
        let v0 = cm.config.volume;
        key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary2Right);
        assert!(active(&cm));
        assert_eq!(cm.control.volume_panel.sel, ROW_CIRCLING);
        assert_eq!(cm.config.volume, v0 + 1, "the first click already counts");
        // the outer knob picks the cruise slider, the inner one changes it
        key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary1Right);
        assert_eq!(cm.control.volume_panel.sel, ROW_CRUISE);
        key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary2Left);
        assert_eq!(
            cm.config.stf_volume(),
            v0,
            "cruise: from the circling volume one down"
        );
        assert_eq!(cm.config.volume, v0 + 1);
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert!(!active(&cm));
        // in speed to fly mode the cruise slider is the one
        cm.control.vario_mode = VarioMode::SpeedToFly;
        key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary2Right);
        assert_eq!(cm.control.volume_panel.sel, ROW_CRUISE);
        // a pause closes it
        for _ in 0..300 {
            key(&mut cm, &mut cc, &mut t, KeyEvent::NoEvent);
        }
        assert!(!active(&cm));
    }

    #[test]
    fn from_the_flight_menu_pick_push_turn_push() {
        let (mut cm, mut cc, _tx) = test_harness!();
        let mut t = 0u16;
        cm.control.vario_mode = VarioMode::Vario;
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        // 15.11.2026: the flight menu has no Volume row any more (the inner
        // knob on the page is the way); the path from a menu stays tested
        let menu = crate::model::menu::flight_menu(&cm);
        assert!(menu.items.iter().all(|it| it.name(&cm) != "Volume"));
        open(&mut cm, &mut cc, false, true);
        assert!(active(&cm) && !cm.control.volume_panel.edit);
        assert_eq!(cm.config.overlay_active, OverlayActive::None);
        // pick warnings, push, turn
        key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary2Right);
        key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary2Right);
        assert_eq!(cm.control.volume_panel.sel, ROW_WARNING);
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        let a0 = cm.control.alarm_volume;
        key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary2Right);
        assert_eq!(cm.control.alarm_volume, a0 + 1);
        // push: done, back in the flight menu
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert!(!active(&cm));
        assert_eq!(cm.config.overlay_active, OverlayActive::Menu);
    }
}
