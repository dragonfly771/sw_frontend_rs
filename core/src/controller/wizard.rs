//! Setup wizard (Stefan 15.11.2026: a pilot who never opens the manual).
//!
//! A new device (and one after a factory reset) asks for the few things it
//! cannot know, one after the other, with the normal setup editors: units,
//! glider, pilot weight, mounting rotation. Then the sensor calibration menu
//! opens. A push takes a value and goes on; a long push or 30 s without a
//! key end the wizard, everything set so far stays. It can be started again
//! with Setup > System > Setup Wizard.

use super::{editor, menu};
use crate::{
    model::{
        menu::{basic, full, settings_menu, Menu, MenuItemContent},
        DisplayActive,
    },
    CoreController, CoreModel, Editable,
};

pub const STEPS: [Editable; 6] = [
    Editable::UnitHorizontalSpeed,
    Editable::UnitVerticalSpeed,
    Editable::UnitHeight,
    Editable::Glider,
    Editable::PilotWeight,
    Editable::Rotation,
];

/// the wizard runs and shows this step (1 ..= STEPS.len())
pub fn step(cm: &CoreModel) -> Option<(u8, u8)> {
    match cm.control.wizard_step {
        0 => None,
        n => Some((n, STEPS.len() as u8)),
    }
}

pub fn start(cm: &mut CoreModel, cc: &mut CoreController) {
    // on the ground only
    if cm.sensor.airspeed.ias().to_km_h() >= 30.0 {
        return;
    }
    // the setup menu underneath: its editors are full screen and stay 30 s
    if cm.config.display_active != DisplayActive::Menu {
        menu::activate_menu(settings_menu(cm), cm, cc);
    }
    cm.control.wizard_step = 1;
    open_step(cm, cc);
}

fn open_step(cm: &mut CoreModel, cc: &mut CoreController) {
    let e = STEPS[(cm.control.wizard_step as usize - 1).min(STEPS.len() - 1)];
    editor::activate_editable(e, cm, cc);
    cm.control.editor.from_menu = true;
}

pub fn stop(cm: &mut CoreModel) {
    cm.control.wizard_step = 0;
}

/// An editor has closed. `pushed`: with a push (the value is taken) - then
/// the next step opens and `true` is returned. Else the wizard ends.
pub fn editor_closed(cm: &mut CoreModel, cc: &mut CoreController, pushed: bool) -> bool {
    if cm.control.wizard_step == 0 {
        return false;
    }
    if !pushed {
        stop(cm);
        return false;
    }
    cm.control.wizard_step += 1;
    if cm.control.wizard_step as usize > STEPS.len() {
        stop(cm);
        finish(cm, cc);
        return false;
    }
    open_step(cm, cc);
    true
}

/// after the last step: the sensor calibration list, its Back leads to the setup
fn finish(cm: &mut CoreModel, cc: &mut CoreController) {
    let target: Option<&'static Menu> = if cm.config.club_mode {
        None
    } else if cm.config.basic_mode {
        Some(&basic::SENSOR)
    } else {
        Some(&full::SENSORBOX_ORIENTATION)
    };
    let Some(target) = target else { return };
    let setup = settings_menu(cm);
    let mc = &mut cm.control.menu_control;
    // the setup as the parent, its cursor on the entry that leads there
    let ppos = setup
        .items
        .iter()
        .position(|it| {
            matches!(it.content, MenuItemContent::MenuItem())
                && it.next_menu_idx == basic::SENSOR_IDX
        })
        .unwrap_or(0);
    mc.menu = setup;
    mc.pos[setup.level] = if cm.config.basic_mode { ppos } else { 0 };
    mc.depth = 0;
    if mc.depth < 6 {
        mc.parents[mc.depth] = (setup, mc.pos[setup.level]);
        mc.depth += 1;
    }
    mc.menu = target;
    mc.pos[target.level] = 0;
    menu::fill_values(cm, cc);
}
