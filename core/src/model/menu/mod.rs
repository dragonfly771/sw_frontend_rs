#![allow(unused)]
pub mod basic;
pub mod club;
pub mod full;

use crate::{CoreModel, Editable, TString};

#[derive(Clone, Copy, PartialEq)]
pub struct Menu {
    pub name: &'static str,
    pub level: usize,
    pub items: &'static [MenuItem],
}

#[derive(Clone, Copy, PartialEq)]
pub enum MenuItemContent {
    EditItem(Editable),
    MenuItem(),
    /// entry of the PAGES list: opens the page editor for this page
    /// row of the flap stage table: opens the editor of this stage; rows
    /// beyond the table are hidden (item_visible)
    FlapItem(u8),
}

/// static names of the page entries

#[derive(Clone, Copy, PartialEq)]
pub struct MenuItem {
    pub content: MenuItemContent,
    pub next_menu_idx: usize,
}

impl MenuItem {
    pub fn name(&self, cm: &CoreModel) -> &'static str {
        match self.content {
            MenuItemContent::EditItem(editable) => editable.name(),
            MenuItemContent::MenuItem() => {
                let menu = menu_list(cm, self.next_menu_idx);
                menu.name
            }
            // the renderer shows the stage name from the table
            MenuItemContent::FlapItem(_) => "Flap",
        }
    }

    pub fn is_menu(&self) -> bool {
        match self.content {
            MenuItemContent::EditItem(_) => false,
            MenuItemContent::MenuItem() | MenuItemContent::FlapItem(_) => true,
        }
    }
}

/// Rows of the flap stage table exist only for existing stages
pub fn item_visible(cm: &CoreModel, item: &MenuItem) -> bool {
    match item.content {
        MenuItemContent::FlapItem(i) => (i as usize) < cm.flap_table().len(),
        // the live sensor value only with a flap sensor (sketch
        // skizze_klappen_menue.png 2 / 3)
        MenuItemContent::EditItem(crate::Editable::FlapSensorValue) => {
            cm.flap_table().sensor != crate::model::flaps::FlapSensorSource::None
        }
        // a stage has either an upper limit ("Up to") or, the fastest one,
        // only the speed it begins at ("From")
        MenuItemContent::EditItem(crate::Editable::FlapUpTo) => {
            (cm.config.flap_edit as usize) + 1 < cm.flap_table().len()
        }
        MenuItemContent::EditItem(crate::Editable::FlapFrom) => {
            let n = cm.flap_table().len();
            n >= 2 && (cm.config.flap_edit as usize) + 1 >= n
        }
        // the antenna geometry only matters with two antennas: Competition
        // (2), F9P + F9H (3), Competition Pro (4); hidden while the box has not told
        // its configuration
        MenuItemContent::EditItem(
            crate::Editable::AntBaselen
            | crate::Editable::AntSlaveDown
            | crate::Editable::AntSlaveRight,
        ) => gnss_dual_antenna(cm),
        // FLARM is parked: no entry in the setup (model/flarm_cfg.rs)
        MenuItemContent::MenuItem()
            if !crate::model::flarm_cfg::FLARM_FEATURE
                && !cm.config.club_mode
                && item.next_menu_idx == full::FLARM_IDX =>
        {
            false
        }
        // Basic menu: the switch speed only with "At speed", the pin only
        // with "Stick switch" (one question at a time)
        MenuItemContent::EditItem(crate::Editable::StfSwitchSpeed)
            if cm.config.basic_mode && !cm.config.club_mode =>
        {
            crate::model::editable::simple::cruise_switch(cm)
                == crate::model::editable::simple::CRUISE_SPEED
        }
        MenuItemContent::EditItem(crate::Editable::SpeedToFlyPinConfig)
            if cm.config.basic_mode && !cm.config.club_mode =>
        {
            crate::model::editable::simple::cruise_switch(cm)
                == crate::model::editable::simple::CRUISE_STICK
        }
        // the club setup has no page settings
        MenuItemContent::EditItem(crate::Editable::PageLayout) => !cm.config.club_mode,
        // the AoA factor belongs to the entry optimized needle only
        MenuItemContent::EditItem(crate::Editable::PotK) => {
            cm.config.vario_needle == crate::model::VarioNeedle::EntryOptimized
        }
        _ => true,
    }
}

/// the sensor box has a two antenna GNSS (GNSS Config 2, 3 or 4), as the box
/// reported it (a saved new configuration is in the cache at once)
pub fn gnss_dual_antenna(cm: &CoreModel) -> bool {
    use crate::{controller::persist::remote_content, model::editable::Content, CanConfigId};
    matches!(remote_content(cm, CanConfigId::GnssConfig), Content::F32(Some(v)) if v > 1.5)
}

/// The visible items of a menu in the order they are SHOWN (Stefan
/// 15.11.2026: there is no Esc key - "Back" is the first row of every list,
/// not the last). The menus keep "Return" as their last item, only the
/// order on the display and of the cursor differs; an opened menu starts
/// with the cursor on its first setting, "Back" one row above it.
pub fn visual_order(cm: &CoreModel, menu: &Menu) -> heapless::Vec<usize, 32> {
    let mut v: heapless::Vec<usize, 32> = heapless::Vec::new();
    let n = menu.items.len();
    let is_back = |i: usize| menu.items[i].content == MenuItemContent::EditItem(Editable::Return);
    if n > 0 && is_back(n - 1) && item_visible(cm, &menu.items[n - 1]) {
        let _ = v.push(n - 1);
    }
    for i in 0..n {
        if (i + 1 == n && is_back(i)) || !item_visible(cm, &menu.items[i]) {
            continue;
        }
        let _ = v.push(i);
    }
    v
}

/// next visible item from `pos` in direction `step` (+1 / -1) in the order
/// shown (see `visual_order`); stays if none
pub fn step_visible(cm: &CoreModel, menu: &Menu, pos: usize, step: i32) -> usize {
    let order = visual_order(cm, menu);
    match order.iter().position(|&i| i == pos) {
        Some(v) => {
            let nv = v as i32 + step;
            if nv >= 0 && (nv as usize) < order.len() {
                order[nv as usize]
            } else {
                pos
            }
        }
        // the cursor sits on a hidden item: the nearest item in list order
        None => {
            let n = menu.items.len() as i32;
            let mut p = pos as i32 + step;
            while p >= 0 && p < n {
                if item_visible(cm, &menu.items[p as usize]) {
                    return p as usize;
                }
                p += step;
            }
            pos
        }
    }
}

/// `pos` if visible, else the nearest visible item before it, else after it
pub fn first_visible(cm: &CoreModel, menu: &Menu, pos: usize) -> usize {
    let n = menu.items.len();
    if n == 0 {
        return 0;
    }
    let pos = pos.min(n - 1);
    if item_visible(cm, &menu.items[pos]) {
        return pos;
    }
    // the row before first (after deleting the last stage the cursor stays
    // on the table), then the one after
    for p in (0..pos).rev() {
        if item_visible(cm, &menu.items[p]) {
            return p;
        }
    }
    for p in pos + 1..n {
        if item_visible(cm, &menu.items[p]) {
            return p;
        }
    }
    pos
}

pub fn menu_list(cm: &CoreModel, idx: usize) -> &'static Menu {
    if cm.config.club_mode {
        &club::MENU_LIST[idx]
    } else if cm.config.basic_mode {
        &basic::MENU_LIST[idx]
    } else {
        &full::MENU_LIST[idx]
    }
}

/// The flight menu (short encoder press)
pub fn flight_menu(_cm: &CoreModel) -> &'static Menu {
    &FLIGHT_MENU_STEFLY
}

pub fn settings_menu(cm: &CoreModel) -> &'static Menu {
    menu_list(cm, SETTINGS_IDX)
}

/// Indices into the MENU_LISTs shared by the full and the club menu:
/// root, flight menu, setup and display; index 4 is a free slot, the lists
/// continue with their own menus from 5 on.
pub const ROOT_IDX: usize = 0;
pub const FLIGHT_MENU_IDX: usize = 1;
pub const SETTINGS_IDX: usize = 2;
pub const DISPLAY_IDX: usize = 3;

pub const ROOT: Menu = Menu {
    name: "Root",
    level: 0,
    items: &[],
};

pub const FLIGHT_MENU: Menu = Menu {
    name: "Flight Menu",
    level: 1,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::WaterBallast),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Bugs),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PilotWeight),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Display),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::UserProfile),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: ROOT_IDX,
        },
    ],
};

/// Flight menu (15.11.2026, short): what is set in flight - MC, ballast,
/// pilot weight, bugs, QNH - and the way to the setup. The volume is on the
/// inner knob; gust filter, brightness and page layout are in the setup.
/// Editing an item closes the menu and opens the overlay editor, so the
/// vario scale stays visible the whole time.
pub const FLIGHT_MENU_STEFLY: Menu = Menu {
    name: "Flight Menu",
    level: 1,
    items: &[
        MenuItem {
            content: MenuItemContent::EditItem(Editable::McCready),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::WaterBallast),
            next_menu_idx: ROOT_IDX,
        },
        // back again (15.11.2026): it goes into wing loading and speed to fly
        MenuItem {
            content: MenuItemContent::EditItem(Editable::PilotWeight),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Bugs),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Qnh),
            next_menu_idx: ROOT_IDX,
        },
        MenuItem {
            content: MenuItemContent::MenuItem(),
            next_menu_idx: SETTINGS_IDX,
        },
        MenuItem {
            content: MenuItemContent::EditItem(Editable::Return),
            next_menu_idx: ROOT_IDX,
        },
    ],
};

#[cfg(test)]
mod structure_tests {
    //! Both menu lists: every index resolves, levels fit the cursor array,
    //! Return goes up, editing stays in its menu, and a setting has exactly
    //! one place in the setup.
    use super::*;
    use crate::view::test_fixture::test_model;

    fn check_list(cm: &CoreModel, list: &[Menu]) {
        let n = list.len();
        assert!(list[ROOT_IDX] == ROOT && list[FLIGHT_MENU_IDX] == FLIGHT_MENU);
        assert_eq!(list[SETTINGS_IDX].name, "Setup");
        assert_eq!(list[DISPLAY_IDX].name, "Display");
        for (idx, menu) in list.iter().enumerate() {
            assert!(
                menu.level < 6,
                "{}: level {} beyond the cursor array",
                menu.name,
                menu.level
            );
            for item in menu.items {
                let target = item.next_menu_idx;
                assert!(target < n, "{}: index {target} outside the list", menu.name);
                let next = &list[target];
                match item.content {
                    MenuItemContent::EditItem(Editable::Return) => {
                        assert!(
                            target == ROOT_IDX || next.level + 1 == menu.level,
                            "{}: Return leads to {} (level {})",
                            menu.name,
                            next.name,
                            next.level
                        );
                    }
                    MenuItemContent::EditItem(_) => {
                        assert!(
                            target == idx || target == ROOT_IDX,
                            "{}: {} leaves its menu towards {}",
                            menu.name,
                            item.name(cm),
                            next.name
                        );
                    }
                    MenuItemContent::MenuItem() | MenuItemContent::FlapItem(_) => {
                        assert!(
                            next.level == menu.level + 1,
                            "{}: submenu {} has level {} instead of {}",
                            menu.name,
                            next.name,
                            next.level,
                            menu.level + 1
                        );
                        assert!(
                            next.items.last().map(|it| it.content)
                                == Some(MenuItemContent::EditItem(Editable::Return)),
                            "{}: no Return at the end",
                            next.name
                        );
                    }
                }
            }
        }
    }

    /// editables of the setup tree, each menu counted once
    fn setup_editables(cm: &CoreModel, list: &[Menu]) -> std::vec::Vec<Editable> {
        let mut seen: std::vec::Vec<usize> = std::vec![SETTINGS_IDX];
        let mut out = std::vec::Vec::new();
        let mut i = 0;
        while i < seen.len() {
            let menu = &list[seen[i]];
            for item in menu.items {
                match item.content {
                    MenuItemContent::EditItem(Editable::Return) => {}
                    MenuItemContent::EditItem(e) => out.push(e),
                    _ => {
                        if !seen.contains(&item.next_menu_idx) {
                            seen.push(item.next_menu_idx);
                        }
                    }
                }
            }
            i += 1;
        }
        let _ = cm;
        out
    }

    #[test]
    fn both_lists_are_consistent() {
        let cm = test_model();
        check_list(&cm, full::MENU_LIST);
        check_list(&cm, club::MENU_LIST);
    }

    #[test]
    fn a_setting_has_one_place_in_the_setup() {
        let cm = test_model();
        for list in [full::MENU_LIST, club::MENU_LIST] {
            let all = setup_editables(&cm, list);
            for (i, e) in all.iter().enumerate() {
                assert!(
                    !all[i + 1..].contains(e),
                    "{} appears twice in the setup",
                    e.name()
                );
            }
        }
    }

    #[test]
    fn the_dead_entries_are_gone_and_the_box_values_have_moved() {
        let cm = test_model();
        let all = setup_editables(&cm, full::MENU_LIST);
        for dead in [
            Editable::VarioUpperLimit,
            Editable::VarioLowerLimit,
            Editable::CenterViewCircling,
            Editable::CenterViewStraight,
            Editable::Info1Vario,
            Editable::Info3Stf,
            Editable::InfoR1,
        ] {
            assert!(!all.contains(&dead), "{} is still offered", dead.name());
        }
        for wanted in [
            Editable::VarioTc,
            Editable::VarioIntTc,
            Editable::WindTc,
            Editable::MeanWindTc,
            Editable::DeviceInfo,
            Editable::AvgMarkerMode,
            Editable::BlockHorizon,
        ] {
            assert!(all.contains(&wanted), "{} missing", wanted.name());
        }
        // Vario TC sits in the Vario menu, first entry
        assert!(full::VARIO.items[0].content == MenuItemContent::EditItem(Editable::VarioTc));
        // the club list offers no aircraft, sensor box or page entries
        let club_all = setup_editables(&cm, club::MENU_LIST);
        for e in [
            Editable::Glider,
            Editable::SensTiltRoll,
            Editable::PageLayout,
            Editable::FactoryReset,
        ] {
            assert!(!club_all.contains(&e), "club mode offers {}", e.name());
        }
    }
}

#[cfg(test)]
mod tree_dump {
    use super::*;
    use crate::model::editable::Params;
    use crate::view::test_fixture::test_model;

    fn describe(cm: &CoreModel, e: Editable) -> std::string::String {
        match e.params(cm) {
            Params::F32(p) => std::format!(
                "Zahl {}..{} {} (Schritt {}/{})",
                p.min,
                p.max,
                p.unit,
                p.small_inc,
                p.big_inc
            ),
            Params::Enum(p) => {
                let v: std::vec::Vec<&str> = p
                    .variants
                    .iter()
                    .copied()
                    .filter(|s| !s.is_empty())
                    .collect();
                std::format!("Auswahl: {}", v.join(" / "))
            }
            Params::List(p) => {
                let mut names: std::vec::Vec<std::string::String> = std::vec::Vec::new();
                for i in 0..=p.max {
                    let t = e.content_as_str(crate::model::editable::Content::List(i), cm);
                    names.push(t.as_str().trim().into());
                }
                std::format!("Liste: {}", names.join(" / "))
            }
            Params::Cmd(_) => "Befehl".into(),
            Params::String(_) => "Text/Anzeige".into(),
            Params::Date(_) => "Datum".into(),
        }
    }

    fn walk(
        cm: &CoreModel,
        menu: &Menu,
        depth: usize,
        out: &mut std::string::String,
        seen: &mut std::vec::Vec<&'static str>,
    ) {
        for item in menu.items {
            let name = item.name(cm);
            let pad = "  ".repeat(depth);
            match item.content {
                MenuItemContent::EditItem(Editable::Return) => {}
                MenuItemContent::EditItem(e) => {
                    out.push_str(&std::format!("{pad}- {name}  [{}]\n", describe(cm, e)));
                }
                MenuItemContent::MenuItem() => {
                    let sub = menu_list(cm, item.next_menu_idx);
                    out.push_str(&std::format!("{pad}- {name} >\n"));
                    if !seen.contains(&sub.name) {
                        seen.push(sub.name);
                        walk(cm, sub, depth + 1, out, seen);
                    }
                }
                MenuItemContent::FlapItem(_) => {
                    let sub = menu_list(cm, item.next_menu_idx);
                    out.push_str(&std::format!("{pad}- {name} >\n"));
                    if !seen.contains(&sub.name) {
                        seen.push(sub.name);
                        walk(cm, sub, depth + 1, out, seen);
                    }
                }
            }
        }
    }

    /// `cargo test --lib tree_dump -- --ignored --nocapture` prints the
    /// menu tree of the ring design (flight menu and setup)
    #[test]
    #[ignore]
    fn print_menu_tree() {
        let cm = test_model();
        let mut out = std::string::String::new();
        let mut seen = std::vec::Vec::new();
        out.push_str("FLIGHT MENU (kurzer Druck)\n");
        walk(&cm, flight_menu(&cm), 1, &mut out, &mut seen);
        out.push_str("\nSETUP\n");
        let mut seen = std::vec::Vec::new();
        walk(&cm, settings_menu(&cm), 1, &mut out, &mut seen);
        std::println!("{out}");
    }
}

#[cfg(test)]
mod tree_dump_en {
    //! The menus for the manual, in English, Basic and Expert:
    //! `cargo test --lib print_menu_tree_en -- --ignored --nocapture`
    use super::*;
    use crate::model::editable::Params;
    use crate::view::test_fixture::test_model;

    fn describe(cm: &CoreModel, e: Editable) -> std::string::String {
        match e.params(cm) {
            Params::F32(p) => std::format!(
                "{}..{} {} (step {}/{})",
                p.min,
                p.max,
                p.unit,
                p.small_inc,
                p.big_inc
            ),
            Params::Enum(p) => {
                let v: std::vec::Vec<&str> = p
                    .variants
                    .iter()
                    .copied()
                    .filter(|s| !s.is_empty())
                    .collect();
                std::format!("choice: {}", v.join(" / "))
            }
            Params::List(p) if p.max > 20 => "list of polars".into(),
            Params::List(p) => {
                let mut names: std::vec::Vec<std::string::String> = std::vec::Vec::new();
                for i in 0..=p.max {
                    let t = e.content_as_str(crate::model::editable::Content::List(i), cm);
                    names.push(t.as_str().trim().into());
                }
                std::format!("list: {}", names.join(" / "))
            }
            Params::Cmd(_) => "command".into(),
            Params::String(_) => "text".into(),
            Params::Date(_) => "date".into(),
        }
    }

    fn walk(
        cm: &CoreModel,
        menu: &Menu,
        depth: usize,
        out: &mut std::string::String,
        seen: &mut std::vec::Vec<&'static str>,
    ) {
        for item in menu.items {
            let name = item.name(cm);
            let pad = "  ".repeat(depth);
            match item.content {
                MenuItemContent::EditItem(Editable::Return) => {}
                MenuItemContent::EditItem(e) => {
                    out.push_str(&std::format!("{pad}- {name}  [{}]\n", describe(cm, e)));
                }
                // parked menus (FLARM) are not shown
                MenuItemContent::MenuItem() if !item_visible(cm, item) => {}
                MenuItemContent::MenuItem() | MenuItemContent::FlapItem(_) => {
                    let sub = menu_list(cm, item.next_menu_idx);
                    out.push_str(&std::format!("{pad}- {name} >\n"));
                    if !seen.contains(&sub.name) {
                        seen.push(sub.name);
                        walk(cm, sub, depth + 1, out, seen);
                    }
                }
            }
        }
    }

    #[test]
    #[ignore]
    fn print_menu_tree_en() {
        let mut cm = test_model();
        let mut out = std::string::String::new();
        out.push_str("FLIGHT MENU (short push on a page)\n");
        for item in flight_menu(&cm).items {
            match item.content {
                MenuItemContent::EditItem(Editable::Return) => {}
                MenuItemContent::EditItem(e) => out.push_str(&std::format!(
                    "  - {}  [{}]\n",
                    item.name(&cm),
                    describe(&cm, e)
                )),
                _ => out.push_str(&std::format!(
                    "  - {} >  (the setup menu below)\n",
                    item.name(&cm)
                )),
            }
        }
        for (basic, title) in [
            (true, "SETUP, BASIC (long push on a page)"),
            (false, "SETUP, EXPERT (long push on a page)"),
        ] {
            cm.config.basic_mode = basic;
            out.push_str(&std::format!("\n{title}\n"));
            let mut seen = std::vec::Vec::new();
            walk(&cm, settings_menu(&cm), 1, &mut out, &mut seen);
        }
        std::println!("TREE-BEGIN\n{out}TREE-END");
    }
}

#[cfg(test)]
mod gnss_tests {
    //! Stefan 09.11.2026: the antenna geometry appears only with a two
    //! antenna GNSS (Competition, Competition Pro, F9P + F9H).
    use super::*;
    use crate::{model::menu::full::SENSORBOX_GNSS, view::test_fixture::test_model, CanConfigId};

    fn visible(cm: &CoreModel) -> std::vec::Vec<&'static str> {
        SENSORBOX_GNSS
            .items
            .iter()
            .filter(|it| item_visible(cm, it))
            .map(|it| it.name(cm))
            .collect()
    }

    #[test]
    fn antenna_items_only_with_two_antennas() {
        let mut cm = test_model();
        // the box has not answered yet: only the configuration
        assert_eq!(visible(&cm), ["GNSS Config", "Back"]);
        cm.sensor
            .remote
            .set(CanConfigId::GnssConfig.remote_slot(), 1.0);
        assert_eq!(visible(&cm), ["GNSS Config", "Back"]);
        for c in [2.0, 3.0, 4.0] {
            cm.sensor
                .remote
                .set(CanConfigId::GnssConfig.remote_slot(), c);
            assert_eq!(
                visible(&cm),
                [
                    "GNSS Config",
                    "Antenna Distance",
                    "Front Ant. Lower",
                    "Front Ant. Right",
                    "Back"
                ],
                "GNSS Config {c}"
            );
        }
        // the cursor never rests on a hidden item
        cm.sensor
            .remote
            .set(CanConfigId::GnssConfig.remote_slot(), 1.0);
        // shown: Back (item 4) above GNSS Config (item 0)
        assert_eq!(step_visible(&cm, &SENSORBOX_GNSS, 0, -1), 4);
        assert_eq!(step_visible(&cm, &SENSORBOX_GNSS, 0, 1), 0);
        assert_eq!(step_visible(&cm, &SENSORBOX_GNSS, 4, 1), 0);
        assert_eq!(first_visible(&cm, &SENSORBOX_GNSS, 2), 0);
    }
}

#[cfg(test)]
mod parked_and_restored_tests {
    use super::*;
    use crate::{model::flarm_cfg::FLARM_FEATURE, test_harness};

    /// 15.11.2026: FLARM is parked - without the crate feature no menu leads to it
    #[test]
    fn flarm_shows_in_the_setup_only_with_its_feature() {
        let (cm, _cc, _tx) = test_harness!();
        let setup = settings_menu(&cm);
        let shown = setup
            .items
            .iter()
            .filter(|it| item_visible(&cm, it))
            .any(|it| it.name(&cm) == "FLARM");
        assert_eq!(shown, FLARM_FEATURE);
        assert!(
            setup.items.iter().any(|it| it.name(&cm) == "FLARM"),
            "the entry itself stays"
        );
    }

    /// pilot weight in the flight menu, user profile in System (full and club)
    #[test]
    fn pilot_weight_and_user_profile_can_be_reached() {
        let (mut cm, _cc, _tx) = test_harness!();
        assert!(flight_menu(&cm)
            .items
            .iter()
            .any(|it| it.content == MenuItemContent::EditItem(Editable::PilotWeight)));
        for club in [false, true] {
            cm.config.club_mode = club;
            let found = (0..if club {
                club::MENU_LIST.len()
            } else {
                full::MENU_LIST.len()
            })
                .map(|i| menu_list(&cm, i))
                .filter(|m| m.name == "System")
                .any(|m| {
                    m.items
                        .iter()
                        .any(|it| it.content == MenuItemContent::EditItem(Editable::UserProfile))
                });
            assert!(found, "club {club}");
        }
    }
}
