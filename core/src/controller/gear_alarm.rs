//! Gear alarm on the display, with a way to acknowledge it so that it goes
//! away.
//!
//! The condition comes from the inputs (helpers/hw_pins.rs GearAlarmControl:
//! gear up and airbrakes open, or one line with both switches in series).
//! While it holds:
//! - the alarm box is shown over the page and the alarm tone sounds. A
//!   flight menu or a value being set on the page gives way to it; the
//!   setup (on the ground) and the page layout editor do not.
//! - a PUSH (short or long) confirms it: the box goes, the tone stops, a
//!   small "GEAR UP" stays at the lower left of the ring page as long as
//!   the condition holds. The knobs keep working meanwhile (volume, pages).
//! - when the condition ends (gear down or airbrakes in) everything is
//!   reset; the next time the alarm comes again.
//! Drawing: view/ring_editor.rs draw_alarm, reminder: view/vario_ring.rs.
//!
//! Two-seater (Stefan 16.11.2026): the switches are wired to one display.
//! The condition of the other display counts as well (display_sync,
//! SYNC_GEAR): the box shows on both, a push on either confirms it on both.

use super::{display_sync, editor, menu, page_layout, sound::SoundScenario, volume_panel, Timer};
use crate::{
    model::{DisplayActive, EditMode, OverlayActive, TypeOfInfo},
    CoreController, CoreModel, KeyEvent,
};

/// the alarm box is on the display
pub fn shown(cm: &CoreModel) -> bool {
    cm.config.overlay_active == OverlayActive::Info
        && cm.config.type_of_info == TypeOfInfo::GearAlarm
}

/// Called with every change of the inputs and once per second. Returns
/// true while the alarm tone sounds.
pub fn update(cm: &mut CoreModel, cc: &mut CoreController) -> bool {
    let local = cc.gear_alarm_control.condition();
    if display_sync::gear_changed(cc, local) {
        display_sync::send_gear(cc, cm, local);
    }
    let condition = local || cc.display_sync.remote_gear(cm.control.alive_ticks);
    cm.control.gear_warning = condition;
    if !condition {
        cm.control.gear_alarm_ack = false;
        hide(cm);
    } else if !cm.control.gear_alarm_ack {
        show(cm, cc);
    }
    let sounding = condition && !cm.control.gear_alarm_ack;
    cc.sound_control
        .set_scenario(SoundScenario::GearAlarm, sounding);
    sounding
}

fn show(cm: &mut CoreModel, cc: &mut CoreController) {
    if shown(cm) {
        return;
    }
    // In flight the alarm comes first: the flight menu and a value that is
    // being set on the page (MC, ballast ... - all taken while turning)
    // close. Not the setup and not the page layout editor.
    if cm.config.display_active != DisplayActive::Menu && !page_layout::active(cm) {
        if cm.control.editor.mode != EditMode::Off {
            let _ = cc.scheduler.stop(Timer::CloseEditFrame, false);
            editor::close_edit_frame(cm, cc);
        }
        if cm.config.overlay_active == OverlayActive::Menu {
            menu::close_menu_display(cm, cc);
        }
    }
    // (the ballast info of a running dump gives way as well)
    if matches!(
        cm.config.overlay_active,
        OverlayActive::None | OverlayActive::Info
    ) {
        cm.config.overlay_active = OverlayActive::Info;
        cm.config.type_of_info = TypeOfInfo::GearAlarm;
    }
}

fn hide(cm: &mut CoreModel) {
    if cm.config.type_of_info == TypeOfInfo::GearAlarm {
        cm.config.type_of_info = TypeOfInfo::None;
        if cm.config.overlay_active == OverlayActive::Info {
            cm.config.overlay_active = OverlayActive::None;
        }
    }
}

/// the pilot has seen it
pub fn confirm(cm: &mut CoreModel, cc: &mut CoreController) {
    confirm_from_other(cm, cc);
    // and the other display of a two-seater takes it down as well
    let local = cc.gear_alarm_control.condition();
    display_sync::send_gear(cc, cm, local);
}

/// confirmed on the other display of a two-seater
pub fn confirm_from_other(cm: &mut CoreModel, cc: &mut CoreController) {
    cm.control.gear_alarm_ack = true;
    hide(cm);
    cc.sound_control
        .set_scenario(SoundScenario::GearAlarm, false);
}

/// A push on the alarm box confirms it. The knobs are left alone: the inner
/// one opens the volume panel on the warning slider, the outer one changes
/// the page.
pub fn key_action(key_event: &mut KeyEvent, cm: &mut CoreModel, cc: &mut CoreController) {
    if !shown(cm) || volume_panel::active(cm) {
        return;
    }
    if matches!(
        *key_event,
        KeyEvent::BtnEnc | KeyEvent::BtnEncS3 | KeyEvent::BtnEsc
    ) {
        confirm(cm, cc);
        *key_event = KeyEvent::NoEvent;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::{
        controller::InPinFunction, model::menu::ROOT, test_harness, Event, InputPinState, PinState,
    };

    fn run(cm: &mut CoreModel, cc: &mut CoreController, clock: &mut u16, ms: u32) {
        for _ in 0..ms {
            *clock = clock.wrapping_add(1);
            cc.tick_1ms(*clock, cm);
        }
    }
    fn key(cm: &mut CoreModel, cc: &mut CoreController, clock: &mut u16, k: KeyEvent) {
        cc.event_handler(Event::KeyItem(k), cm);
        run(cm, cc, clock, 20);
    }
    /// two switches: gear up when its contact is closed, airbrakes open when theirs is
    fn wired() -> (CoreModel, CoreController, u16) {
        let (mut cm, mut cc, _tx) = test_harness!();
        cc.gear_alarm_control
            .set_gear_pin_function(InPinFunction::OnClose);
        cc.gear_alarm_control
            .set_airbrakes_pin_function(InPinFunction::OnClose);
        let mut clock = 0;
        gear(&mut cm, &mut cc, true);
        brakes(&mut cm, &mut cc, false);
        run(&mut cm, &mut cc, &mut clock, 50);
        (cm, cc, clock)
    }
    fn gear(cm: &mut CoreModel, cc: &mut CoreController, up: bool) {
        let s = if up { PinState::Low } else { PinState::High };
        cc.event_handler(Event::InputItem(InputPinState::Io3(s)), cm);
    }
    fn brakes(cm: &mut CoreModel, cc: &mut CoreController, open: bool) {
        let s = if open { PinState::Low } else { PinState::High };
        cc.event_handler(Event::InputItem(InputPinState::Io4(s)), cm);
    }

    #[test]
    fn a_push_confirms_the_alarm_and_it_stays_away_until_the_condition_ends() {
        let (mut cm, mut cc, mut clock) = wired();
        assert!(
            !shown(&cm) && !cc.sound_control.alarm_active(),
            "gear up alone is no alarm"
        );
        brakes(&mut cm, &mut cc, true);
        assert!(shown(&cm) && cc.sound_control.alarm_active());
        assert!(cm.control.gear_warning && !cm.control.gear_alarm_ack);
        // it stays as long as nobody pushes
        run(&mut cm, &mut cc, &mut clock, 20_000);
        assert!(shown(&cm) && cc.sound_control.alarm_active());
        // push: gone, quiet - and it does not come back by itself
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert!(!shown(&cm) && !cc.sound_control.alarm_active());
        assert_eq!(
            cm.config.overlay_active,
            OverlayActive::None,
            "no flight menu opened by that push"
        );
        assert!(
            cm.control.gear_warning && cm.control.gear_alarm_ack,
            "the reminder stays"
        );
        run(&mut cm, &mut cc, &mut clock, 30_000);
        assert!(!shown(&cm) && !cc.sound_control.alarm_active());
        // the display works as usual meanwhile (this bench has no sensor
        // box: the next push first takes "NO SENSOR BOX" off the page)
        assert!(crate::view::vario_ring::no_box_banner(&cm));
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert_eq!(cm.config.overlay_active, OverlayActive::None);
        assert!(cm.control.no_box_ack && !crate::view::vario_ring::no_box_banner(&cm));
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert_eq!(cm.config.overlay_active, OverlayActive::Menu);
        menu::close_menu_display(&mut cm, &mut cc);
        // gear down: all reset
        gear(&mut cm, &mut cc, false);
        assert!(!cm.control.gear_warning && !cm.control.gear_alarm_ack);
        // the next time (gear up again, brakes still open) it comes again
        gear(&mut cm, &mut cc, true);
        assert!(shown(&cm) && cc.sound_control.alarm_active());
        // airbrakes in: gone without a push
        brakes(&mut cm, &mut cc, false);
        assert!(!shown(&cm) && !cc.sound_control.alarm_active());
        assert_eq!(cm.config.overlay_active, OverlayActive::None);
    }

    #[test]
    fn a_long_push_confirms_too_and_does_not_open_the_setup() {
        let (mut cm, mut cc, mut clock) = wired();
        brakes(&mut cm, &mut cc, true);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
        assert!(!shown(&cm) && cm.control.gear_alarm_ack);
        assert_eq!(cm.config.display_active, DisplayActive::Vario);
    }

    #[test]
    fn the_knobs_do_not_confirm_the_inner_one_sets_the_warning_volume() {
        let (mut cm, mut cc, mut clock) = wired();
        brakes(&mut cm, &mut cc, true);
        let page = cm.config.page_idx;
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary1Right);
        assert!(
            cm.config.page_idx != page,
            "the outer knob changes the page"
        );
        assert!(shown(&cm) && cc.sound_control.alarm_active());
        let v = cm.control.alarm_volume;
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Left);
        assert!(volume_panel::active(&cm));
        assert_eq!(cm.control.volume_panel.sel, volume_panel::ROW_WARNING);
        assert_eq!(cm.control.alarm_volume, v - 1);
        assert!(cc.sound_control.alarm_active(), "quieter, not confirmed");
        // the push that closes the panel is not the confirmation
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert!(!volume_panel::active(&cm));
        assert!(shown(&cm) && !cm.control.gear_alarm_ack);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert!(!shown(&cm) && cm.control.gear_alarm_ack);
    }

    #[test]
    fn the_flight_menu_gives_way_to_the_alarm() {
        let (mut cm, mut cc, mut clock) = wired();
        // the pilot is setting MC in the flight menu
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Right);
        assert!(cm.control.editor.inline && cm.control.editor.mode != EditMode::Off);
        let mc = cm.config.mc_cready;
        brakes(&mut cm, &mut cc, true);
        assert!(shown(&cm), "the alarm is on the display at once");
        assert!(cm.control.editor.mode == EditMode::Off && !cm.control.editor.inline);
        assert!(cm.control.menu_control.menu == &ROOT);
        assert!(cm.config.mc_cready == mc, "the MC set so far stays");
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert!(!shown(&cm));
        assert_eq!(cm.config.overlay_active, OverlayActive::None);
        // no late timer closes or opens anything
        run(&mut cm, &mut cc, &mut clock, 35_000);
        assert_eq!(cm.config.overlay_active, OverlayActive::None);
        assert_eq!(cm.config.display_active, DisplayActive::Vario);
    }

    #[test]
    fn the_setup_is_not_interrupted_the_alarm_shows_when_it_is_left() {
        let (mut cm, mut cc, mut clock) = wired();
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
        assert_eq!(cm.config.display_active, DisplayActive::Menu);
        crate::controller::activate_editable(crate::Editable::Theme, &mut cm, &mut cc);
        brakes(&mut cm, &mut cc, true);
        assert!(cc.sound_control.alarm_active(), "the tone sounds");
        assert!(
            !shown(&cm) && cm.control.editor.mode != EditMode::Off,
            "the editor stays"
        );
        // back on the page the box is there within a second
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
        run(&mut cm, &mut cc, &mut clock, 1_100);
        assert_eq!(cm.config.display_active, DisplayActive::Vario);
        assert!(shown(&cm));
    }

    #[test]
    fn one_line_with_both_switches_in_series_works_the_same() {
        let (mut cm, mut cc, _tx) = test_harness!();
        let mut clock = 0u16;
        cc.gear_alarm_control
            .set_gear_pin_mode(crate::controller::GearPins::OnePinMode);
        cc.gear_alarm_control
            .set_gear_pin_function(InPinFunction::OnClose);
        gear(&mut cm, &mut cc, true);
        assert!(shown(&cm) && cc.sound_control.alarm_active());
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert!(!shown(&cm) && !cc.sound_control.alarm_active());
        gear(&mut cm, &mut cc, false);
        gear(&mut cm, &mut cc, true);
        assert!(shown(&cm));
    }

    /// pixels of a colour on the whole display
    fn ink(cm: &mut CoreModel, col: crate::Colors) -> usize {
        use crate::view::test_fixture::TestDisplay;
        let mut view = crate::CoreView::new(TestDisplay::new(), cm);
        view.prepare(cm);
        view.draw().unwrap();
        view.display.buf.iter().filter(|p| **p == Some(col)).count()
    }

    /// The box is a box in both blink phases (alarm colour / dark red, the
    /// words always there); after the push the page is back with the small
    /// reminder, and that goes with the condition.
    #[test]
    fn the_box_blinks_without_disappearing_and_leaves_a_reminder() {
        use crate::Colors;
        let (mut cm, mut cc, mut clock) = wired();
        cm.control.alive_ticks = 1000;
        let alarm = cm.palette().signal.alarm;
        let before = ink(&mut cm, alarm);
        brakes(&mut cm, &mut cc, true);
        cm.control.alive_ticks = 1000;
        let bright = ink(&mut cm, alarm);
        assert!(
            bright > before + 20_000,
            "bar and ring in the alarm colour: {before} -> {bright}"
        );
        cm.control.alive_ticks = 1000 + crate::view::ring_editor::ALARM_BLINK_TICKS;
        assert!(
            ink(&mut cm, Colors::DarkRed) > 20_000,
            "dark phase: the bar stays, in dark red"
        );
        assert!(
            ink(&mut cm, Colors::White) > 500,
            "... with its words in white"
        );
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        let reminder = ink(&mut cm, alarm);
        assert!(
            reminder > before + 150 && reminder < before + 1500,
            "small reminder: {before} -> {reminder}"
        );
        assert_eq!(ink(&mut cm, Colors::DarkRed), 0);
        brakes(&mut cm, &mut cc, false);
        assert_eq!(ink(&mut cm, alarm), before, "airbrakes in: nothing left");
    }
}
