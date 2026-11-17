//! Simple operation (Stefan 15.11.2026): Back as the first row, the short
//! flight menu, Basic / Expert / Club, presets, the setup wizard, the
//! guided sensor calibration. Mode decision of a new / updated device:
//! utils/persistence.rs (with the EEPROM).

extern crate std;
use crate::{
    controller::{editor::editor_timeout, menu::calibration_state, wizard},
    model::{
        editable::{
            simple::{self, *},
            Content,
        },
        menu::{
            basic, club, flight_menu, full, item_visible, settings_menu, step_visible,
            visual_order, Menu, MenuItemContent, ROOT,
        },
        DisplayActive, EditMode, GustFilter, OverlayActive, VarioModeControl,
    },
    test_harness,
    view::help::{help, HELP_MAX_CHARS},
    CoreController, CoreModel, Editable, Event, FloatToSpeed, KeyEvent,
};

fn key(cm: &mut CoreModel, cc: &mut CoreController, clock: &mut u16, k: KeyEvent) {
    cc.event_handler(Event::KeyItem(k), cm);
    run(cm, cc, clock, 20);
}

fn run(cm: &mut CoreModel, cc: &mut CoreController, clock: &mut u16, ms: u32) {
    for _ in 0..ms {
        *clock = clock.wrapping_add(1);
        cc.tick_1ms(*clock, cm);
    }
}

/// cursor on the item with this name in the current menu, push
fn enter(cm: &mut CoreModel, cc: &mut CoreController, clock: &mut u16, name: &str) {
    let menu = cm.control.menu_control.menu;
    let pos = menu
        .items
        .iter()
        .position(|it| it.name(cm) == name && item_visible(cm, it))
        .unwrap_or_else(|| panic!("{name} not in {}", menu.name));
    cm.control.menu_control.pos[menu.level] = pos;
    key(cm, cc, clock, KeyEvent::BtnEnc);
}

fn names(cm: &CoreModel, menu: &Menu) -> std::vec::Vec<&'static str> {
    visual_order(cm, menu)
        .iter()
        .map(|&i| menu.items[i].name(cm))
        .collect()
}

fn choice(cm: &CoreModel) -> std::string::String {
    match cm.control.editor.content {
        Content::Enum(v) => v.as_str().into(),
        _ => std::string::String::new(),
    }
}

/// turn the inner knob until the editor shows `want`
fn turn_to(cm: &mut CoreModel, cc: &mut CoreController, clock: &mut u16, want: &str) {
    for k in [KeyEvent::Rotary2Right, KeyEvent::Rotary2Left] {
        for _ in 0..8 {
            if choice(cm) == want {
                return;
            }
            key(cm, cc, clock, k);
        }
    }
    panic!("{want} not offered, editor shows {}", choice(cm));
}

fn basic_model() -> (CoreModel, CoreController, u16) {
    let (mut cm, cc, _tx) = test_harness!();
    cm.config.basic_mode = true;
    (cm, cc, 0)
}

// ------------------------------------------------------------------- menus

#[test]
fn back_is_the_first_row_of_every_menu_and_the_cursor_starts_below_it() {
    for basic_mode in [false, true] {
        let (mut cm, mut cc, _tx) = test_harness!();
        cm.config.basic_mode = basic_mode;
        let mut clock = 0u16;
        let lists: &[&'static Menu] = if basic_mode {
            &basic::MENU_LIST[..]
        } else {
            &full::MENU_LIST[..]
        }
        .iter()
        .collect::<std::vec::Vec<_>>()
        .leak();
        for menu in lists.iter().copied() {
            let n = menu.items.len();
            if n == 0 || menu.items[n - 1].content != MenuItemContent::EditItem(Editable::Return) {
                continue;
            }
            let shown = names(&cm, menu);
            assert_eq!(shown[0], "Back", "{}", menu.name);
            assert_eq!(
                shown.iter().filter(|s| **s == "Back").count(),
                1,
                "{}",
                menu.name
            );
        }
        // the setup opens with the cursor on its first setting; one click
        // back is "Back", a push there closes the setup
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
        assert_eq!(cm.config.display_active, DisplayActive::Menu);
        let menu = cm.control.menu_control.menu;
        assert!(menu == settings_menu(&cm));
        let order = visual_order(&cm, menu);
        assert_eq!(cm.control.menu_control.pos[menu.level], order[1]);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Left);
        assert_eq!(cm.control.menu_control.pos[menu.level], order[0]);
        // the first row is the end of the list
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Left);
        assert_eq!(cm.control.menu_control.pos[menu.level], order[0]);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert_eq!(cm.config.display_active, DisplayActive::Vario);
        assert!(cm.control.menu_control.menu == &ROOT);
    }
}

#[test]
fn the_cursor_walks_the_rows_as_they_are_shown() {
    let (cm, _cc, _) = basic_model();
    let menu = settings_menu(&cm);
    let order = visual_order(&cm, menu);
    let mut pos = order[0];
    for want in order.iter().skip(1) {
        pos = step_visible(&cm, menu, pos, 1);
        assert_eq!(pos, *want);
    }
    assert_eq!(step_visible(&cm, menu, pos, 1), pos, "last row: stays");
}

#[test]
fn the_flight_menu_is_short() {
    let (cm, _cc, _tx) = test_harness!();
    assert_eq!(
        names(&cm, flight_menu(&cm)),
        [
            "Back",
            "MC",
            "Water Ballast",
            "Pilot Weight",
            "Bugs",
            "QNH",
            "Setup"
        ]
    );
}

#[test]
fn the_basic_setup_has_few_rows_and_every_one_explains_itself() {
    let (cm, _cc, _) = basic_model();
    let menu = settings_menu(&cm);
    assert!(menu == &basic::SETTINGS);
    let rows = names(&cm, menu);
    assert_eq!(
        rows,
        [
            "Back",
            "Vario Response",
            "Cruise Switch",
            "Sound",
            "Display",
            "Units",
            "Aircraft",
            "Sensor Setup",
            "System"
        ]
    );
    let mut settings = 0;
    // (the list starts with the root and the flight menu, whose values are
    // edited in their row without a help text)
    for menu in basic::MENU_LIST
        .iter()
        .skip(crate::model::menu::SETTINGS_IDX)
    {
        assert!(
            menu.items.len() <= 12,
            "{}: {} rows",
            menu.name,
            menu.items.len()
        );
        for item in menu.items.iter() {
            match item.content {
                MenuItemContent::MenuItem() => {
                    let next = &basic::MENU_LIST[item.next_menu_idx];
                    assert!(
                        !next.items.is_empty(),
                        "{} > {} leads nowhere",
                        menu.name,
                        item.name(&cm)
                    );
                }
                MenuItemContent::EditItem(e)
                    if e != Editable::Return && e != Editable::DeviceInfo =>
                {
                    settings += 1;
                    let h = help(e)
                        .unwrap_or_else(|| panic!("{} > {}: no help text", menu.name, e.name()));
                    assert!(
                        h.iter().all(|l| l.chars().count() <= HELP_MAX_CHARS),
                        "{}",
                        e.name()
                    );
                }
                _ => (),
            }
        }
    }
    assert!(
        (20..=32).contains(&settings),
        "{settings} settings in the Basic setup"
    );
}

#[test]
fn switch_speed_and_pin_show_only_with_their_cruise_switch() {
    let (mut cm, mut cc, mut clock) = basic_model();
    let menu = settings_menu(&cm);
    assert_eq!(cruise_switch(&cm), CRUISE_AUTO);
    assert!(!names(&cm, menu).contains(&"Switch Speed"));
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    enter(&mut cm, &mut cc, &mut clock, "Cruise Switch");
    turn_to(&mut cm, &mut cc, &mut clock, CRUISE_SPEED);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    assert_eq!(cruise_switch(&cm), CRUISE_SPEED);
    assert!(cm.control.vario_mode_control == VarioModeControl::Auto);
    let rows = names(&cm, menu);
    let at = rows.iter().position(|r| *r == "Cruise Switch").unwrap();
    assert_eq!(rows[at + 1], "Switch Speed", "{rows:?}");
    enter(&mut cm, &mut cc, &mut clock, "Cruise Switch");
    turn_to(&mut cm, &mut cc, &mut clock, CRUISE_STICK);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    assert!(cm.control.vario_mode_control == VarioModeControl::InputPin);
    let rows = names(&cm, menu);
    assert!(
        !rows.contains(&"Switch Speed") && rows.len() == 10,
        "{rows:?}"
    );
    // and back
    enter(&mut cm, &mut cc, &mut clock, "Cruise Switch");
    turn_to(&mut cm, &mut cc, &mut clock, CRUISE_AUTO);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    assert_eq!(cruise_switch(&cm), CRUISE_AUTO);
    assert_eq!(names(&cm, menu).len(), 9);
}

// ------------------------------------------------------------------ editors

#[test]
fn a_setup_editor_stays_30_s_a_value_on_the_page_4_s() {
    let (mut cm, mut cc, _tx) = test_harness!();
    let mut clock = 0u16;
    assert_eq!(
        editor_timeout(&cm),
        crate::basic_config::SECTION_EDITOR_TIMEOUT
    );
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    assert_eq!(editor_timeout(&cm), crate::basic_config::MENU_TIMEOUT);
    enter(&mut cm, &mut cc, &mut clock, "Sound");
    enter(&mut cm, &mut cc, &mut clock, "Sound Preset");
    assert!(cm.control.editor.mode != EditMode::Off);
    run(&mut cm, &mut cc, &mut clock, 20_000);
    assert!(
        cm.control.editor.mode != EditMode::Off,
        "the help text is still there after 20 s"
    );
    run(&mut cm, &mut cc, &mut clock, 12_000);
    assert!(cm.control.editor.mode == EditMode::Off);
}

#[test]
fn vario_response_is_the_gust_filter_in_one_word() {
    let (mut cm, mut cc, mut clock) = basic_model();
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    for (word, filter) in [
        (RESPONSE_FAST, GustFilter::Off),
        (RESPONSE_CALM, GustFilter::Medium),
        (RESPONSE_NORMAL, GustFilter::PeakCut),
    ] {
        enter(&mut cm, &mut cc, &mut clock, "Vario Response");
        turn_to(&mut cm, &mut cc, &mut clock, word);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert!(cm.config.gust_filter == filter, "{word}");
    }
}

#[test]
fn a_sound_preset_is_heard_while_turning_and_taken_with_a_push_only() {
    let (mut cm, mut cc, mut clock) = basic_model();
    assert_eq!(
        sound_preset(&cm),
        PRESET_STANDARD,
        "the defaults are the Standard preset"
    );
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    enter(&mut cm, &mut cc, &mut clock, "Sound");
    enter(&mut cm, &mut cc, &mut clock, "Sound Preset");
    assert!(
        crate::controller::sound::sound_preview_wanted(&cm),
        "the preview plays"
    );
    turn_to(&mut cm, &mut cc, &mut clock, PRESET_SOFT);
    assert_eq!(
        sound_preset(&cm),
        PRESET_SOFT,
        "the preview plays the preset under the cursor"
    );
    // 30 s without a push: nothing taken, the sound from before is back
    run(&mut cm, &mut cc, &mut clock, 31_000);
    assert!(cm.control.editor.mode == EditMode::Off);
    assert_eq!(sound_preset(&cm), PRESET_STANDARD);
    assert!(cm.control.sound_backup.is_none());
    // a long push leaves the setup: nothing taken either
    run(&mut cm, &mut cc, &mut clock, 31_000);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    enter(&mut cm, &mut cc, &mut clock, "Sound");
    enter(&mut cm, &mut cc, &mut clock, "Sound Preset");
    turn_to(&mut cm, &mut cc, &mut clock, PRESET_SHARP);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    assert_eq!(cm.config.display_active, DisplayActive::Vario);
    assert_eq!(sound_preset(&cm), PRESET_STANDARD);
    // push: taken
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    enter(&mut cm, &mut cc, &mut clock, "Sound");
    enter(&mut cm, &mut cc, &mut clock, "Sound Preset");
    turn_to(&mut cm, &mut cc, &mut clock, PRESET_SHARP);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    assert_eq!(sound_preset(&cm), PRESET_SHARP);
    run(&mut cm, &mut cc, &mut clock, 31_000);
    assert_eq!(sound_preset(&cm), PRESET_SHARP);
}

#[test]
fn a_sound_set_by_hand_survives_listening_to_the_presets() {
    let (mut cm, mut cc, mut clock) = basic_model();
    cm.config.snd_center_freq = 540.0;
    cm.config.snd_beep_tempo = 1.1;
    assert_eq!(sound_preset(&cm), PRESET_CUSTOM);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    enter(&mut cm, &mut cc, &mut clock, "Sound");
    enter(&mut cm, &mut cc, &mut clock, "Sound Preset");
    assert_eq!(choice(&cm), PRESET_CUSTOM);
    turn_to(&mut cm, &mut cc, &mut clock, PRESET_SOFT);
    assert_eq!(cm.config.snd_center_freq, 400.0);
    // Custom in the list is the own sound again, a push keeps it
    turn_to(&mut cm, &mut cc, &mut clock, PRESET_CUSTOM);
    assert_eq!(cm.config.snd_center_freq, 540.0);
    turn_to(&mut cm, &mut cc, &mut clock, PRESET_SHARP);
    turn_to(&mut cm, &mut cc, &mut clock, PRESET_CUSTOM);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    assert_eq!(cm.config.snd_center_freq, 540.0);
    assert_eq!(cm.config.snd_beep_tempo, 1.1);
    assert_eq!(sound_preset(&cm), PRESET_CUSTOM);
}

#[test]
fn the_three_sound_presets_differ() {
    for (i, (a, pa)) in simple::SOUND_PRESETS.iter().enumerate() {
        for (b, pb) in simple::SOUND_PRESETS.iter().skip(i + 1) {
            assert!(pa.center != pb.center && pa.tempo != pb.tempo, "{a} = {b}");
        }
    }
}

#[test]
fn a_page_style_sets_the_first_page_with_a_push() {
    let (mut cm, mut cc, mut clock) = basic_model();
    assert_eq!(
        page_style(&cm),
        STYLE_STANDARD,
        "a new device shows the Standard page"
    );
    // the horizon page is shown: the style is still the one of the vario page
    cm.config.page_idx = 1;
    let before = cm.config.pages;
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    enter(&mut cm, &mut cc, &mut clock, "Display");
    enter(&mut cm, &mut cc, &mut clock, "Page Style");
    assert_eq!(choice(&cm), STYLE_STANDARD);
    turn_to(&mut cm, &mut cc, &mut clock, STYLE_BIG);
    assert!(cm.config.pages == before, "turning changes nothing");
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    assert_eq!(page_style(&cm), STYLE_BIG);
    assert!(cm.config.pages[0].core == crate::model::PageCore::TwoValues);
    assert_eq!(cm.config.page_idx, 0, "the styled page shows");
    // (stored: utils/persistence.rs a_page_style_reaches_the_chip)
    for style in [STYLE_SIMPLE, STYLE_COMPETITION, STYLE_STANDARD] {
        enter(&mut cm, &mut cc, &mut clock, "Page Style");
        turn_to(&mut cm, &mut cc, &mut clock, style);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
        assert_eq!(page_style(&cm), style);
    }
    // Standard is the page of a new device again, the horizon was never touched
    assert!(cm.config.pages == before);
    // a page changed with Page Layout is "Custom", and stays offered
    cm.config.pages[0].boxes[0] = crate::LineView::TrueAirSpeed;
    assert_eq!(page_style(&cm), STYLE_CUSTOM);
}

#[test]
fn a_glider_is_taken_with_a_push_not_by_scrolling_past_it() {
    let (mut cm, mut cc, mut clock) = basic_model();
    let glider = cm.config.glider_idx;
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    enter(&mut cm, &mut cc, &mut clock, "Aircraft");
    enter(&mut cm, &mut cc, &mut clock, "Glider");
    for _ in 0..5 {
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Right);
    }
    assert_eq!(
        cm.config.glider_idx, glider,
        "scrolling does not load polars"
    );
    run(&mut cm, &mut cc, &mut clock, 31_000);
    assert_eq!(cm.config.glider_idx, glider, "no push: the glider stays");
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    enter(&mut cm, &mut cc, &mut clock, "Aircraft");
    enter(&mut cm, &mut cc, &mut clock, "Glider");
    for _ in 0..5 {
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Right);
    }
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    assert_ne!(cm.config.glider_idx, glider, "the push takes it");
}

// -------------------------------------------------------------- usage mode

#[test]
fn basic_and_expert_switch_without_a_code_club_needs_one() {
    let (mut cm, mut cc, mut clock) = basic_model();
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    enter(&mut cm, &mut cc, &mut clock, "System");
    assert!(cm.control.menu_control.menu == &basic::SYSTEM);
    enter(&mut cm, &mut cc, &mut clock, "Usage Mode");
    assert_eq!(choice(&cm), "Basic");
    // no code: Club is not offered
    for _ in 0..4 {
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Right);
    }
    assert_eq!(choice(&cm), "Expert");
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    assert!(!cm.config.basic_mode && !cm.config.club_mode);
    assert!(
        cm.control.menu_control.menu == &full::STEFLY_SYSTEM,
        "{}",
        cm.control.menu_control.menu.name
    );
    assert_eq!(cm.config.display_active, DisplayActive::Menu);
    // Back leads to the Expert setup
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEsc);
    assert!(cm.control.menu_control.menu == &full::STEFLY_SETTINGS);
    // and back to Basic
    enter(&mut cm, &mut cc, &mut clock, "System");
    enter(&mut cm, &mut cc, &mut clock, "Usage Mode");
    turn_to(&mut cm, &mut cc, &mut clock, "Basic");
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    assert!(cm.config.basic_mode);
    assert!(cm.control.menu_control.menu == &basic::SYSTEM);
    // with the code Club is the third choice
    cm.config.usage_code = cm.device_const.misc.sw_version.lower_as_u16();
    cm.config.user_profile = 1;
    enter(&mut cm, &mut cc, &mut clock, "Usage Mode");
    turn_to(&mut cm, &mut cc, &mut clock, "Club");
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    assert!(cm.config.club_mode);
    assert!(cm.control.menu_control.menu == &club::SYSTEM);
}

#[test]
fn the_energy_arrow_is_an_expert_thing() {
    use crate::view::test_fixture::TestDisplay;
    use crate::Colors;
    let reds = |basic: bool| {
        let (mut cm, _cc, _tx) = test_harness!();
        cm.config.basic_mode = basic;
        cm.control.alive_ticks = 1000;
        cm.control.fly_mode = crate::FlyMode::StraightFlight;
        cm.config.overlay_active = OverlayActive::None;
        cm.sensor.wind_vector.set_speed(10.0.km_h());
        cm.sensor.average_wind.set_speed(40.0.km_h());
        let cm_1s = cm;
        let mut td = crate::view::thermal_data::ThermalData::default();
        let mut d = TestDisplay::new();
        crate::view::vario_ring::draw_ring(&mut d, &cm, &cm_1s, &mut td).unwrap();
        let c = cm.device_const.sizes.display.center;
        let mut n = 0;
        for y in c.y - 60..c.y + 60 {
            for x in c.x - 60..c.x + 60 {
                if d.buf[(y * crate::view::test_fixture::DISPLAY_WIDTH as i32 + x) as usize]
                    == Some(Colors::Red)
                {
                    n += 1;
                }
            }
        }
        n
    };
    assert!(
        reds(false) > 50,
        "Expert: the red arrow is drawn ({} px)",
        reds(false)
    );
    assert_eq!(reds(true), 0, "Basic: no red arrow");
}

// ------------------------------------------------------------------ wizard

#[test]
fn the_wizard_asks_six_things_and_ends_in_the_sensor_setup() {
    let (mut cm, mut cc, mut clock) = basic_model();
    wizard::start(&mut cm, &mut cc);
    run(&mut cm, &mut cc, &mut clock, 20);
    assert_eq!(cm.config.display_active, DisplayActive::Menu);
    for (n, e) in wizard::STEPS.iter().enumerate() {
        assert_eq!(wizard::step(&cm), Some((n as u8 + 1, 6)));
        assert!(
            cm.control.editor.mode != EditMode::Off && cm.control.editor.target == *e,
            "step {}: {}",
            n + 1,
            e.name()
        );
        assert!(help(*e).is_some(), "{}: no help text", e.name());
        key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Right);
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    }
    assert!(wizard::step(&cm).is_none());
    assert!(cm.control.editor.mode == EditMode::Off);
    assert!(
        cm.control.menu_control.menu == &basic::SENSOR,
        "{}",
        cm.control.menu_control.menu.name
    );
    assert_eq!(cm.config.display_active, DisplayActive::Menu);
    // Back: the setup, with the cursor on Sensor Setup
    enter(&mut cm, &mut cc, &mut clock, "Back");
    let menu = cm.control.menu_control.menu;
    assert!(menu == &basic::SETTINGS);
    assert_eq!(
        menu.items[cm.control.menu_control.pos[menu.level]].name(&cm),
        "Sensor Setup"
    );
}

#[test]
fn the_wizard_takes_what_was_set() {
    let (mut cm, mut cc, mut clock) = basic_model();
    let (speed, weight) = (cm.config.unit_horizontal_speed, cm.glider_data.pilot_weight);
    wizard::start(&mut cm, &mut cc);
    run(&mut cm, &mut cc, &mut clock, 20);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary2Right);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc); // 1 speed unit
    assert!(cm.config.unit_horizontal_speed != speed);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc); // 2
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc); // 3
    let glider = cm.config.glider_idx;
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc); // 4 glider: a push without a turn keeps it
    assert_eq!(cm.config.glider_idx, glider);
    assert_eq!(wizard::step(&cm), Some((5, 6)));
    key(&mut cm, &mut cc, &mut clock, KeyEvent::Rotary1Right);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc); // 5 pilot weight
    assert!(cm.glider_data.pilot_weight != weight);
}

#[test]
fn a_long_push_or_a_pause_ends_the_wizard() {
    let (mut cm, mut cc, mut clock) = basic_model();
    wizard::start(&mut cm, &mut cc);
    run(&mut cm, &mut cc, &mut clock, 20);
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc);
    assert_eq!(wizard::step(&cm), Some((2, 6)));
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    assert!(wizard::step(&cm).is_none());
    assert_eq!(cm.config.display_active, DisplayActive::Vario);
    assert!(cm.control.editor.mode == EditMode::Off);
    // a pause: the editor closes after 30 s, the wizard with it, no next step
    wizard::start(&mut cm, &mut cc);
    run(&mut cm, &mut cc, &mut clock, 31_000);
    assert!(wizard::step(&cm).is_none());
    assert!(cm.control.editor.mode == EditMode::Off);
    run(&mut cm, &mut cc, &mut clock, 31_000);
    assert_eq!(
        cm.config.display_active,
        DisplayActive::Vario,
        "the setup closes as usual"
    );
}

#[test]
fn no_wizard_in_flight_and_it_starts_again_from_the_system_menu() {
    let (mut cm, mut cc, mut clock) = basic_model();
    cm.sensor.airspeed = crate::flight_physics::AirSpeed::from_speeds(100.0.km_h(), 100.0.km_h());
    wizard::start(&mut cm, &mut cc);
    assert!(wizard::step(&cm).is_none());
    assert_eq!(cm.config.display_active, DisplayActive::Vario);
    cm.sensor.airspeed = crate::flight_physics::AirSpeed::from_speeds(0.0.km_h(), 0.0.km_h());
    for expert in [false, true] {
        cm.config.basic_mode = !expert;
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
        enter(&mut cm, &mut cc, &mut clock, "System");
        // a command runs with the push that picks it
        enter(&mut cm, &mut cc, &mut clock, "Setup Wizard");
        assert_eq!(wizard::step(&cm), Some((1, 6)), "expert {expert}");
        assert!(
            cm.control.editor.target == wizard::STEPS[0] && cm.control.editor.mode != EditMode::Off
        );
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
        assert!(wizard::step(&cm).is_none());
    }
}

// ------------------------------------------------------------- calibration

#[test]
fn the_calibration_rows_say_what_is_done_and_what_is_missing() {
    let (mut cm, mut cc, mut clock) = basic_model();
    use Editable::*;
    assert_eq!(calibration_state(0, CmdMeas1), "to do");
    assert_eq!(calibration_state(0, CmdCalcOrientation), "not yet");
    assert_eq!(calibration_state(0b011, CmdCalcOrientation), "not yet");
    assert_eq!(calibration_state(0b111, CmdCalcOrientation), "ready");
    assert_eq!(calibration_state(0b1111, CmdCalcOrientation), "done");
    assert_eq!(
        calibration_state(0b1111, CmdFineTuneOrientation),
        "in flight"
    );
    // through the menu
    key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEncS3);
    enter(&mut cm, &mut cc, &mut clock, "Sensor Setup");
    let menu = cm.control.menu_control.menu;
    assert!(menu == &basic::SENSOR);
    let value = |cm: &CoreModel, e: Editable| -> std::string::String {
        let i = menu
            .items
            .iter()
            .position(|it| it.content == MenuItemContent::EditItem(e))
            .unwrap();
        cm.control.menu_control.values[i].as_str().into()
    };
    assert_eq!(value(&cm, CmdMeas1), "to do");
    for (k, e) in [CmdMeas1, CmdMeas2, CmdMeas3].into_iter().enumerate() {
        let i = menu
            .items
            .iter()
            .position(|it| it.content == MenuItemContent::EditItem(e))
            .unwrap();
        cm.control.menu_control.pos[menu.level] = i;
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc); // the push runs the command
        assert!(
            cm.control.editor.mode != EditMode::Off,
            "its text stays to read"
        );
        key(&mut cm, &mut cc, &mut clock, KeyEvent::BtnEnc); // back to the list
        run(&mut cm, &mut cc, &mut clock, 100);
        assert!(cm.control.menu_control.menu == menu && cm.control.editor.mode == EditMode::Off);
        assert!(cm.control.cal_done & (1 << k) != 0, "{}", e.name());
        assert_eq!(value(&cm, e), "done", "{}", e.name());
    }
    assert_eq!(value(&cm, CmdCalcOrientation), "ready");
}
