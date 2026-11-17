use crate::{
    basic_config::MENU_TIMEOUT,
    controller::{editor, helpers::IntToDuration, EditMode, KeyEvent, Timer},
    model::{
        menu::{
            first_visible, flight_menu, menu_list, settings_menu, step_visible, Menu,
            MenuItemContent, FLIGHT_MENU, FLIGHT_MENU_STEFLY, ROOT,
        },
        DisplayActive, OverlayActive,
    },
    utils::TString,
    CoreController, CoreModel, Editable,
};

/// number of cached value strings for the ring style menu renderer
pub const MENU_VALUE_ROWS: usize = 16;

#[derive(Clone, Copy)]
pub struct MenuControl {
    pub pos: [usize; 6],
    pub return_idx: usize,
    pub menu: &'static Menu,
    /// dynamic parent stack (menu + its cursor position): submenus can be
    /// reused under new parents, Return comes back to where the user came
    /// from and restores the cursor - even when parent and child share the
    /// same menu level
    pub parents: [(&'static Menu, usize); 6],
    pub depth: usize,
    /// cached value strings of the current menu items (ring design shows
    /// the values directly in the list)
    pub values: [TString<20>; MENU_VALUE_ROWS],
}

impl Default for MenuControl {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuControl {
    pub fn new() -> Self {
        MenuControl {
            pos: [0; 6],
            return_idx: 0,
            menu: &FLIGHT_MENU,
            parents: [(&ROOT, 0); 6],
            depth: 0,
            values: [TString::new(); MENU_VALUE_ROWS],
        }
    }
}

fn is_flight(menu: &'static Menu) -> bool {
    menu == &FLIGHT_MENU || menu == &FLIGHT_MENU_STEFLY
}

/// refresh the cached value strings for the current menu
pub fn fill_values(cm: &mut CoreModel, cc: &mut CoreController) {
    let menu = cm.control.menu_control.menu;
    let n = menu.items.len().min(MENU_VALUE_ROWS);
    for i in 0..n {
        let s = match menu.items[i].content {
            // baud rate: "Auto 19200" with the detected rate, "Scanning"
            // while the search runs (sketch skizze_flarm_menue.png)
            MenuItemContent::EditItem(Editable::FlarmBaud)
                if cm.config.flarm.fixed_baud().is_none() =>
            {
                let f = &cm.calculated.flarm;
                let mut t: heapless::String<20> = heapless::String::new();
                if f.baud_locked {
                    let _ = tfmt::uwrite!(&mut t, "Auto {}", f.baud);
                } else {
                    let _ = t.push_str("Scanning");
                }
                TString::from_str(t.as_str())
            }
            // sensor calibration: each step says whether it is done (this
            // session), the calculation what it still needs
            MenuItemContent::EditItem(
                e @ (Editable::CmdMeas1
                | Editable::CmdMeas2
                | Editable::CmdMeas3
                | Editable::CmdCalcOrientation
                | Editable::CmdFineTuneOrientation),
            ) => TString::from_str(calibration_state(cm.control.cal_done, e)),
            MenuItemContent::EditItem(e) if e != Editable::Return => {
                let content = e.content(cm, cc);
                e.content_as_str(content, cm)
            }
            MenuItemContent::FlapItem(i) => flap_row_value(cm, i as usize),
            // "Flap Stages  8 >": number of stages before the chevron
            MenuItemContent::MenuItem()
                if menu_list(cm, menu.items[i].next_menu_idx).name == "Flap Stages" =>
            {
                let mut t: heapless::String<20> = heapless::String::new();
                let _ = tfmt::uwrite!(&mut t, "{}", cm.flap_table().len());
                TString::from_str(t.as_str())
            }
            _ => TString::new(),
        };
        cm.control.menu_control.values[i] = s;
    }
}

/// value column of a calibration step (bits: model/control.rs cal_done)
pub fn calibration_state(done: u8, e: Editable) -> &'static str {
    let bit = |b: u8| done & (1 << b) != 0;
    match e {
        Editable::CmdMeas1 => {
            if bit(0) {
                "done"
            } else {
                "to do"
            }
        }
        Editable::CmdMeas2 => {
            if bit(1) {
                "done"
            } else {
                "to do"
            }
        }
        Editable::CmdMeas3 => {
            if bit(2) {
                "done"
            } else {
                "to do"
            }
        }
        Editable::CmdCalcOrientation => {
            if bit(3) {
                "done"
            } else if done & 0b111 == 0b111 {
                "ready"
            } else {
                "not yet"
            }
        }
        Editable::CmdFineTuneOrientation => {
            if bit(4) {
                "done"
            } else {
                "in flight"
            }
        }
        _ => "",
    }
}

/// value column of a flap stage row: speed range (at reference weight) and,
/// with a flap sensor, the stored position - "75-88 km/h  31.5°"
fn flap_row_value(cm: &CoreModel, i: usize) -> TString<20> {
    let t = cm.flap_table();
    if i >= t.len() {
        return TString::new();
    }
    let u = cm.config.unit_horizontal_speed;
    let conv = |kmh: u16| u.as_f32(crate::Speed::from_km_h(kmh as f32)) as i32;
    let (lo, hi) = t.range(i);
    let mut s: heapless::String<20> = heapless::String::new();
    let _ = match (lo, hi) {
        (0, 0) => tfmt::uwrite!(&mut s, "all"),
        (0, h) => tfmt::uwrite!(&mut s, "<{}", conv(h)),
        (l, 0) => tfmt::uwrite!(&mut s, "{}+", conv(l)),
        (l, h) => tfmt::uwrite!(&mut s, "{}-{}", conv(l), conv(h)),
    };
    let _ = tfmt::uwrite!(&mut s, " {}", u.as_str());
    if t.sensor != crate::model::flaps::FlapSensorSource::None {
        let p = t.stages[i].position;
        if p.is_nan() {
            let _ = tfmt::uwrite!(&mut s, "  -");
        } else {
            let _ = tfmt::uwrite!(
                &mut s,
                "  {}°",
                (if p >= 0.0 { p + 0.5 } else { p - 0.5 }) as i32
            );
        }
    }
    TString::from_str(s.as_str())
}

/// A sensor box value arrived: update the value preview of the current menu
/// for the item(s) showing it, without asking the box again
pub fn refresh_remote_value(cm: &mut CoreModel, config_id: crate::CanConfigId) {
    if cm.config.display_active != DisplayActive::Menu
        && cm.config.overlay_active != OverlayActive::Menu
    {
        return;
    }
    let menu = cm.control.menu_control.menu;
    let n = menu.items.len().min(MENU_VALUE_ROWS);
    for i in 0..n {
        if let MenuItemContent::EditItem(e) = menu.items[i].content {
            if e.remote_config_id() == Some(config_id) {
                let content = crate::persist::remote_content(cm, config_id);
                let s = e.content_as_str(content, cm);
                cm.control.menu_control.values[i] = s;
            }
        }
    }
}

pub fn key_action(key_event: &mut KeyEvent, cm: &mut CoreModel, cc: &mut CoreController) {
    fn menu_action(cm: &mut CoreModel, cc: &mut CoreController, pos: usize) {
        let cur = cm.control.menu_control.menu;
        if pos >= cur.items.len() {
            return;
        }
        let menu_item = cur.items[pos];
        let is_return = menu_item.content == MenuItemContent::EditItem(Editable::Return);

        // a row of the flap table selects the stage the editor works on
        if let MenuItemContent::FlapItem(idx) = menu_item.content {
            cm.config.flap_edit = idx;
        }

        // flight menu: a number or a choice is edited right in its row -
        // the menu stays where it is, the row turns orange, push ends it
        if let MenuItemContent::EditItem(editable) = menu_item.content {
            // volume: the panel with the three sliders
            if is_flight(cur) && editable == Editable::Volume {
                crate::controller::volume_panel::open(cm, cc, false, true);
                return;
            }
            let in_row = matches!(
                editable.params(cm),
                crate::model::editable::Params::F32(_)
                    | crate::model::editable::Params::Enum(_)
                    | crate::model::editable::Params::List(_)
            );
            if is_flight(cur) && in_row && editable != Editable::Return {
                editor::activate_editable(editable, cm, cc);
                cm.control.editor.from_menu = true;
                cm.control.editor.inline = true;
                cm.config.overlay_active = OverlayActive::Menu;
                return;
            }
        }

        // resolve the next menu: Return pops the dynamic parent stack
        // first and restores the cursor position of the parent
        let mut restore_pos: Option<usize> = None;
        let next_menu = if is_return && cm.control.menu_control.depth > 0 {
            cm.control.menu_control.depth -= 1;
            let (parent, ppos) = cm.control.menu_control.parents[cm.control.menu_control.depth];
            restore_pos = Some(ppos);
            parent
        } else {
            menu_list(cm, menu_item.next_menu_idx)
        };

        // entering a submenu: push the current menu (with its cursor
        // position) and start at the top of the new one
        if menu_item.is_menu() && next_menu != cur {
            if cm.control.menu_control.depth < 6 {
                let depth = cm.control.menu_control.depth;
                cm.control.menu_control.parents[depth] =
                    (cur, cm.control.menu_control.pos[cur.level]);
                cm.control.menu_control.depth = depth + 1;
            }
            cm.control.menu_control.pos[next_menu.level] = 0;

            // leaving the flight overlay towards the big setup menu
            if cm.config.overlay_active == OverlayActive::Menu && !is_flight(next_menu) {
                cm.config.overlay_active = OverlayActive::None;
                if cm.config.display_active != DisplayActive::Menu {
                    cm.config.last_display_active = cm.config.display_active;
                }
                cm.config.display_active = DisplayActive::Menu;
                cm.control.menu_control.depth = 0; // the setup starts a fresh stack
            }
        }

        // activate next_menu
        cm.control.menu_control.menu = next_menu;
        if next_menu != &ROOT {
            // restore or clamp the cursor - a stale position from a menu
            // sharing the same level must never point beyond the items
            let level = next_menu.level;
            let len = next_menu.items.len();
            if let Some(p) = restore_pos {
                cm.control.menu_control.pos[level] = p;
            }
            if len > 0 && cm.control.menu_control.pos[level] >= len {
                cm.control.menu_control.pos[level] = len - 1;
            }
            // never rest on a hidden row (flap table)
            cm.control.menu_control.pos[level] =
                first_visible(cm, next_menu, cm.control.menu_control.pos[level]);
        }
        if next_menu == &ROOT {
            close_menu_display(cm, cc);
        } else {
            fill_values(cm, cc);
        }

        // open editor if menu item contains something to edit
        if let MenuItemContent::EditItem(editable) = menu_item.content {
            if editable != Editable::Return {
                let stages_before = cm.config.flaps.len();
                editor::activate_editable(editable, cm, cc);
                if editable == Editable::FlapAdd && cm.config.flaps.len() > stages_before {
                    // a new stage: straight into it, its name first (there
                    // was only a blank "added" screen before, 29.10.2026)
                    open_new_stage(cm, cc, cur);
                    return;
                }
                // both encoders edit the value when coming from a menu
                cm.control.editor.from_menu = true;

                if editable == Editable::FlapDelete {
                    // the stage is gone: back to the table
                    menu_back(cm, cc);
                }
            }
        }
    }

    if cm.control.editor.mode == EditMode::Window {
        return;
    }
    if cm.config.display_active == DisplayActive::Menu
        || cm.config.overlay_active == OverlayActive::Menu
    {
        cc.scheduler
            .after(crate::Timer::CloseMenu, MENU_TIMEOUT.secs());
        let level = cm.control.menu_control.menu.level;
        match key_event {
            KeyEvent::Rotary1Left | KeyEvent::Rotary2Left => {
                let menu = cm.control.menu_control.menu;
                let p = cm.control.menu_control.pos[level];
                cm.control.menu_control.pos[level] = step_visible(cm, menu, p, -1);
                *key_event = KeyEvent::NoEvent
            }
            KeyEvent::Rotary1Right | KeyEvent::Rotary2Right => {
                let menu = cm.control.menu_control.menu;
                let p = cm.control.menu_control.pos[level];
                cm.control.menu_control.pos[level] = step_visible(cm, menu, p, 1);
                *key_event = KeyEvent::NoEvent
            }
            KeyEvent::BtnEnc => {
                let pos = cm.control.menu_control.pos[level];
                menu_action(cm, cc, pos);
            }
            KeyEvent::BtnEsc => {
                let pos = cm.control.menu_control.menu.items.len() - 1;
                menu_action(cm, cc, pos);
            }
            KeyEvent::BtnEncS3 => close_menu_display(cm, cc),
            _ => (),
        };
    } else {
        match key_event {
            KeyEvent::BtnEnc => {
                activate_menu(flight_menu(cm), cm, cc);
                *key_event = KeyEvent::NoEvent;
            }
            KeyEvent::BtnEncS3 => {
                let settings_menu = settings_menu(cm);
                activate_menu(settings_menu, cm, cc);
                *key_event = KeyEvent::NoEvent;
            }
            _ => (),
        };
    }
}

/// one level up, like Return (used after a command removed the entry the
/// pilot came from)
/// After "Add Stage": the stage menu of the new (fastest) stage opens with
/// the name entry; Return there leads back to the flap table
fn open_new_stage(cm: &mut CoreModel, cc: &mut CoreController, table: &'static Menu) {
    use crate::model::menu::full::FLAP_EDIT;
    // no "added" screen: the command is done
    cm.control.editor.mode = EditMode::Off;
    cm.config.overlay_active = OverlayActive::None;
    let n = cm.config.flaps.len();
    cm.config.flap_edit = (n - 1) as u8;
    let mc = &mut cm.control.menu_control;
    if mc.depth < 6 {
        // the table is the parent, its cursor on the new stage
        let pos = table
            .items
            .iter()
            .position(|it| it.content == MenuItemContent::FlapItem((n - 1) as u8))
            .unwrap_or(mc.pos[table.level]);
        mc.parents[mc.depth] = (table, pos);
        mc.depth += 1;
    }
    mc.menu = &FLAP_EDIT;
    mc.pos[FLAP_EDIT.level] = 0;
    fill_values(cm, cc);
    editor::activate_editable(Editable::FlapName, cm, cc);
    cm.control.editor.from_menu = true;
}

pub fn menu_back(cm: &mut CoreModel, cc: &mut CoreController) {
    let mc = &mut cm.control.menu_control;
    if mc.depth == 0 {
        return;
    }
    mc.depth -= 1;
    let (parent, ppos) = mc.parents[mc.depth];
    mc.menu = parent;
    mc.pos[parent.level] = ppos;
    let p = first_visible(cm, parent, ppos);
    cm.control.menu_control.pos[parent.level] = p;
    fill_values(cm, cc);
}

pub fn activate_menu(menu: &'static Menu, cm: &mut CoreModel, cc: &mut CoreController) {
    cm.control.menu_control.pos[menu.level] = 0;
    cm.control.menu_control.menu = menu;
    cm.control.menu_control.depth = 0;

    if cm.config.display_active != DisplayActive::Menu {
        cm.config.last_display_active = cm.config.display_active;
    }

    if is_flight(menu) {
        cm.config.overlay_active = OverlayActive::Menu;
    } else {
        cm.config.display_active = DisplayActive::Menu;
    }
    fill_values(cm, cc);
    cc.scheduler
        .after(crate::Timer::CloseMenu, MENU_TIMEOUT.secs());
}

pub fn close_menu_display(cm: &mut CoreModel, cc: &mut CoreController) {
    if cm.config.display_active == DisplayActive::Menu {
        cm.config.display_active = cm.config.last_display_active;
    }
    if cm.config.overlay_active == OverlayActive::Menu {
        cm.config.overlay_active = OverlayActive::None;
    }
    cm.control.menu_control.menu = &ROOT;
    cm.control.menu_control.depth = 0;
    crate::controller::wizard::stop(cm);
    let _ = cc.scheduler.stop(Timer::CloseMenu, false);
}

#[cfg(test)]
mod setup_flow_tests {
    //! The setup menu as the pilot walks it: Device Info opens from
    //! System, Esc brings the page back; switching the usage mode swaps the
    //! menu list underneath the open System menu.
    use super::*;
    use crate::{
        model::{editable::Content, menu::club, menu::full},
        test_harness, Event,
    };

    /// a key through the whole dispatch chain (editor, menu, device info),
    /// followed by 20 ms so that queued callbacks run like on the device
    fn key(cm: &mut CoreModel, cc: &mut CoreController, clock: &mut u16, k: KeyEvent) {
        cc.event_handler(Event::KeyItem(k), cm);
        for _ in 0..2 {
            *clock = clock.wrapping_add(10);
            cc.tick_1ms(*clock, cm);
        }
    }

    /// select the item with this name in the current menu and press Enter
    fn enter(cm: &mut CoreModel, cc: &mut CoreController, clock: &mut u16, name: &str) {
        let menu = cm.control.menu_control.menu;
        let pos = menu
            .items
            .iter()
            .position(|it| it.name(cm) == name)
            .unwrap_or_else(|| panic!("{name} not in {}", menu.name));
        cm.control.menu_control.pos[menu.level] = pos;
        key(cm, cc, clock, KeyEvent::BtnEnc);
    }

    #[test]
    fn device_info_opens_from_the_system_menu_and_esc_returns_to_the_page() {
        let (mut cm, mut cc, _tx) = test_harness!();
        let mut clock = 0u16;
        assert_eq!(cm.config.display_active, DisplayActive::Vario);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
        assert_eq!(cm.config.display_active, DisplayActive::Menu);
        enter(&mut cm, &mut cc, &mut clock, "System");
        assert!(cm.control.menu_control.menu == &full::STEFLY_SYSTEM);
        enter(&mut cm, &mut cc, &mut clock, "Device Info");
        assert_eq!(cm.config.display_active, DisplayActive::DeviceInfo);
        assert_eq!(cm.config.overlay_active, OverlayActive::None);
        assert!(
            cm.control.editor.mode == EditMode::Off,
            "no editor frame over the info page"
        );
        assert!(cm.control.menu_control.menu == &ROOT);
        // scrolling works, Esc goes back to the page
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Right);
        assert_eq!(cm.config.display_active, DisplayActive::DeviceInfo);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEsc);
        assert_eq!(cm.config.display_active, DisplayActive::Vario);
        assert!(cm.control.editor.mode == EditMode::Off);
    }

    #[test]
    fn usage_mode_switch_continues_in_the_system_menu_of_the_other_list() {
        let (mut cm, mut cc, _tx) = test_harness!();
        let mut clock = 0u16;
        cm.config.usage_code = cm.device_const.misc.sw_version.lower_as_u16();
        // from the owner's profile 0 the switch reloads profile 1 with a
        // model reset (the pages come back) - this test walks the menu path
        cm.config.user_profile = 1;
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
        enter(&mut cm, &mut cc, &mut clock, "System");
        enter(&mut cm, &mut cc, &mut clock, "Usage Mode");
        assert!(cm.control.editor.mode != EditMode::Off);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Right);
        assert!(matches!(cm.control.editor.content, Content::Enum(v) if v.as_str() == "Club"));
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert!(cm.config.club_mode);
        assert!(
            cm.control.editor.mode == EditMode::Off,
            "Enter closes the editor"
        );
        assert!(
            cm.control.menu_control.menu == &club::SYSTEM,
            "System of the club list, not {} (depth {})",
            cm.control.menu_control.menu.name,
            cm.control.menu_control.depth
        );
        assert_eq!(cm.config.display_active, DisplayActive::Menu);
        // Return lands in the club setup, its Return closes the menu
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEsc);
        assert!(cm.control.menu_control.menu == &club::SETTINGS);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEsc);
        assert!(cm.control.menu_control.menu == &ROOT);
        assert_eq!(cm.config.display_active, DisplayActive::Vario);
        // and back to normal the same way
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
        enter(&mut cm, &mut cc, &mut clock, "System");
        enter(&mut cm, &mut cc, &mut clock, "Usage Mode");
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Left);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert!(!cm.config.club_mode);
        assert!(cm.control.menu_control.menu == &full::STEFLY_SYSTEM);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEsc);
        assert!(cm.control.menu_control.menu == &full::STEFLY_SETTINGS);
    }

    #[test]
    fn flight_menu_values_are_edited_in_their_row() {
        let (mut cm, mut cc, _tx) = test_harness!();
        let mut clock = 0u16;
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert_eq!(cm.config.overlay_active, OverlayActive::Menu);
        enter(&mut cm, &mut cc, &mut clock, "MC");
        // no editor screen: the menu stays, the row is edited
        assert_eq!(cm.config.overlay_active, OverlayActive::Menu);
        assert!(cm.control.editor.inline && cm.control.editor.mode != EditMode::Off);
        let v0 = cm.config.mc_cready.to_m_s();
        let pos = cm.control.menu_control.pos[cm.control.menu_control.menu.level];
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Right);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary1Right);
        assert!(
            cm.config.mc_cready.to_m_s() > v0,
            "both knobs change the value at once"
        );
        assert_eq!(
            cm.control.menu_control.pos[cm.control.menu_control.menu.level], pos,
            "the cursor stays"
        );
        // push: done, back to moving through the menu
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert!(!cm.control.editor.inline && cm.control.editor.mode == EditMode::Off);
        assert_eq!(cm.config.overlay_active, OverlayActive::Menu);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Right);
        assert_eq!(
            cm.control.menu_control.pos[cm.control.menu_control.menu.level],
            pos + 1
        );
        // the next number (bugs) the same way; the outer knob does not move the cursor
        enter(&mut cm, &mut cc, &mut clock, "Bugs");
        assert!(cm.control.editor.inline);
        let p2 = cm.control.menu_control.pos[cm.control.menu_control.menu.level];
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary1Right);
        assert_eq!(
            cm.control.menu_control.pos[cm.control.menu_control.menu.level],
            p2
        );
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert!(!cm.control.editor.inline);
    }
}
