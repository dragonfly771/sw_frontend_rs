//! Render checks of the FLARM and flap screens.
//!
//! `RENDER_DIR=/tmp/r cargo test --lib flarm_render -- --ignored --nocapture`
//! writes PPM pictures of the demo (radar page, warnings in the core and
//! full screen, flap tape, the new menus) for a visual check. The normal
//! tests below only check that the screens draw and put ink where expected.

use crate::{
    controller::menu::fill_values,
    flarm::logic::ViewKind,
    model::{
        demo::DemoMode,
        flarm_cfg::WarnView,
        menu::{full::MENU_LIST, Menu},
        DisplayActive, PageCore,
    },
    view::{flarm_pages, test_fixture::*},
    CoreController, CoreModel, CoreView,
};

fn run_ms(cm: &mut CoreModel, cc: &mut CoreController, t: &mut u32, ms: u32) {
    for _ in 0..ms {
        *t += 1;
        cc.tick_1ms((*t & 0xffff) as u16, cm);
    }
}

fn render(cm: &mut CoreModel) -> TestDisplay {
    let mut view = CoreView::new(TestDisplay::new(), cm);
    view.prepare(cm);
    view.draw().unwrap();
    view.display
}

fn set_core(cm: &mut CoreModel, core: PageCore) {
    let i = cm.config.page_idx as usize;
    cm.config.pages[i].core = core;
    cm.config.display_active = DisplayActive::Vario;
}

#[cfg(feature = "flarm")]
#[test]
fn radar_and_warnings_draw_during_the_whole_demo() {
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    set_core(&mut cm, PageCore::Radar);
    let mut t = 0u32;
    run_ms(&mut cm, &mut cc, &mut t, 1000);
    crate::controller::flaps_ctl::set_demo(&mut cm, DemoMode::Flarm);
    let mut kinds = std::vec::Vec::new();
    // the whole demo cycle: 300 s, a picture every 2 s
    for _ in 0..150 {
        run_ms(&mut cm, &mut cc, &mut t, 2000);
        let k = cm.calculated.flarm.view.kind;
        if !kinds.contains(&k) {
            kinds.push(k);
        }
        let d = render(&mut cm);
        let c = cm.device_const.sizes.display.center;
        // the own aircraft (magenta) is always there on the radar
        if !flarm_pages::is_warning(k) {
            assert!(d.painted_box(c.x - 20, c.y - 10, c.x + 20, c.y + 34) > 50);
        }
    }
    assert!(kinds.contains(&ViewKind::Idle), "{kinds:?}");
    assert!(kinds.contains(&ViewKind::Warn), "{kinds:?}");
}

#[cfg(feature = "flarm")]
#[test]
#[ignore]
fn flarm_render_demo() {
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    for (tag, view) in [("core", WarnView::Core), ("full", WarnView::FullScreen)] {
        let (mut cm, mut cc, _tx) = crate::test_harness!();
        set_core(&mut cm, PageCore::Radar);
        cm.config.flarm.warn_view = view;
        let mut t = 0u32;
        run_ms(&mut cm, &mut cc, &mut t, 1000);
        crate::controller::flaps_ctl::set_demo(&mut cm, DemoMode::Flarm);
        let mut last = ViewKind::Startup;
        let mut n = 0;
        for step in 0..3000 {
            run_ms(&mut cm, &mut cc, &mut t, 100);
            let v = cm.calculated.flarm.view;
            let periodic = view == WarnView::Core && step % 50 == 25;
            let new_kind = v.kind != last || (flarm_pages::is_warning(v.kind) && step % 20 == 0);
            last = v.kind;
            if periodic || new_kind {
                let d = render(&mut cm);
                dump_ppm(
                    &d,
                    &std::format!("{dir}/flarm_{tag}_{n:03}_{:?}_l{}.ppm", v.kind, v.level),
                );
                n += 1;
            }
        }
    }
}

#[test]
#[ignore]
fn flarm_render_flaps_and_menus() {
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    set_core(&mut cm, PageCore::Boxes);
    let mut t = 0u32;
    run_ms(&mut cm, &mut cc, &mut t, 1000);
    crate::controller::flaps_ctl::set_demo(&mut cm, DemoMode::Flaps);
    for i in 0..12 {
        run_ms(&mut cm, &mut cc, &mut t, 5000);
        let d = render(&mut cm);
        dump_ppm(&d, &std::format!("{dir}/flaps_{i:02}.ppm"));
    }
    // menus of the new functions
    cm.config.display_active = DisplayActive::Menu;
    let names = [
        "FLARM",
        "SteFly Flasher",
        "Warnings",
        "Flaps",
        "Flap Stages",
        "Flap Stage",
        "System",
        "Aircraft",
        "Setup",
    ];
    for menu in MENU_LIST.iter() {
        if !names.contains(&menu.name) {
            continue;
        }
        let menu: &'static Menu = menu;
        cm.control.menu_control.menu = menu;
        cm.config.flap_edit = 3;
        fill_values(&mut cm, &mut cc);
        for (k, pos) in [0usize, menu.items.len() / 2].iter().enumerate() {
            cm.control.menu_control.pos[menu.level] =
                crate::model::menu::first_visible(&cm, menu, *pos);
            let mut d = TestDisplay::new();
            crate::view::ring_menu::draw_setup_list(&mut d, &cm).unwrap();
            let name: std::string::String = menu
                .name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '_'
                    }
                })
                .collect();
            dump_ppm(&d, &std::format!("{dir}/menu_{name}_{k}.ppm"));
        }
    }
}

fn key(cm: &mut CoreModel, cc: &mut CoreController, k: crate::KeyEvent) {
    cc.event_handler(crate::Event::KeyItem(k), cm);
}

/// move the cursor of the current menu onto the item with this name
fn goto(cm: &mut CoreModel, cc: &mut CoreController, name: &str) {
    // "Back" is the first row now: look down the list, then up
    for dir in [crate::KeyEvent::Rotary1Right, crate::KeyEvent::Rotary1Left] {
        for _ in 0..20 {
            let m = cm.control.menu_control.menu;
            let p = cm.control.menu_control.pos[m.level];
            let it = m.items[p];
            let n = match it.content {
                crate::model::menu::MenuItemContent::FlapItem(i) => {
                    std::string::String::from(cm.flap_table().name(i as usize))
                }
                _ => std::string::String::from(it.name(cm)),
            };
            if n == name {
                return;
            }
            key(cm, cc, dir);
        }
    }
    panic!("{name} not found in {}", cm.control.menu_control.menu.name);
}

#[test]
fn flap_stage_name_is_typed_in_the_ring_and_stored() {
    use crate::KeyEvent::*;
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0u32;
    run_ms(&mut cm, &mut cc, &mut t, 500);
    // Setup > Aircraft > Flaps > Flap Stages > "N" > Name
    key(&mut cm, &mut cc, BtnEncS3);
    goto(&mut cm, &mut cc, "Aircraft");
    key(&mut cm, &mut cc, BtnEnc);
    goto(&mut cm, &mut cc, "Flaps");
    key(&mut cm, &mut cc, BtnEnc);
    goto(&mut cm, &mut cc, "Flap Stages");
    key(&mut cm, &mut cc, BtnEnc);
    assert_eq!(cm.control.menu_control.menu.name, "Flap Stages");
    goto(&mut cm, &mut cc, "N");
    key(&mut cm, &mut cc, BtnEnc);
    assert_eq!(cm.control.menu_control.menu.name, "Flap Stage");
    assert_eq!(cm.config.flap_edit, 3);
    goto(&mut cm, &mut cc, "Name");
    key(&mut cm, &mut cc, BtnEnc);
    assert!(crate::controller::name_entry::active(&cm));
    // delete "N", type "N+" - the knob with the push moves the selection,
    // the other one switches ABC / 123
    for _ in 0..2 {
        key(&mut cm, &mut cc, Rotary2Left);
    }
    key(&mut cm, &mut cc, BtnEnc); // "<"
    for _ in 0..15 {
        key(&mut cm, &mut cc, Rotary2Right); // from '<' (27) round to 'N' (13)
    }
    key(&mut cm, &mut cc, BtnEnc);
    key(&mut cm, &mut cc, Rotary1Right);
    for _ in 0..10 {
        key(&mut cm, &mut cc, Rotary2Right);
    }
    key(&mut cm, &mut cc, BtnEnc);
    if let Ok(dir) = std::env::var("RENDER_DIR") {
        let d = render(&mut cm);
        dump_ppm(&d, &std::format!("{dir}/name_entry_123.ppm"));
    }
    key(&mut cm, &mut cc, BtnEncS3); // long push: take it
    assert!(!crate::controller::name_entry::active(&cm));
    assert_eq!(
        cm.config.flaps.name(3),
        "N+",
        "the template became the own table"
    );
    assert_eq!(cm.config.flaps.len(), 8);
    // the whole table is written to the EEPROM, a few items per second
    assert!(cc.flap_dirty != 0);
    for _ in 0..6 {
        run_ms(&mut cm, &mut cc, &mut t, 1000);
    }
    assert_eq!(cc.flap_dirty, 0);
    // back in the stage editor, its header shows the new name
    assert_eq!(cm.control.menu_control.menu.name, "Flap Stage");
    // delete the stage: back in the table, 7 stages
    goto(&mut cm, &mut cc, "Delete Stage");
    key(&mut cm, &mut cc, BtnEnc);
    run_ms(&mut cm, &mut cc, &mut t, 6000);
    assert_eq!(cm.config.flaps.len(), 7);
    assert_eq!(cm.control.menu_control.menu.name, "Flap Stages");
}

#[test]
#[ignore]
fn flarm_render_name_entry() {
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    cm.config.display_active = DisplayActive::Menu;
    crate::controller::activate_editable(crate::Editable::FlapName, &mut cm, &mut cc);
    cm.control.editor.content =
        crate::model::editable::Content::String(crate::utils::TString::from_str("L"));
    cm.control.editor.name.sel = 3;
    let d = render(&mut cm);
    dump_ppm(&d, &std::format!("{dir}/name_entry_abc.ppm"));
}

#[cfg(feature = "flarm")]
#[test]
fn radar_page_encoder_selects_and_follows() {
    use crate::KeyEvent::*;
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    set_core(&mut cm, PageCore::Radar);
    let mut t = 0u32;
    run_ms(&mut cm, &mut cc, &mut t, 500);
    crate::controller::flaps_ctl::set_demo(&mut cm, DemoMode::Flarm);
    // thermal scenario: several gliders around (Crossing 40 + HeadOn 30 + Team 70)
    run_ms(&mut cm, &mut cc, &mut t, 150_000);
    let vol = cm.config.volume;
    key(&mut cm, &mut cc, Rotary2Right);
    assert_eq!(
        cm.config.volume, vol,
        "the small encoder selects, no volume change"
    );
    run_ms(&mut cm, &mut cc, &mut t, 200);
    assert!(cm.calculated.flarm.view.sel_manual, "a target is selected");
    let id = cm.calculated.flarm.view.selected.unwrap().id;
    key(&mut cm, &mut cc, BtnEnc);
    assert_eq!(
        cm.calculated.flarm.toast,
        crate::model::flarm_show::FlarmToast::Following
    );
    run_ms(&mut cm, &mut cc, &mut t, 200);
    assert_eq!(cm.calculated.flarm.view.team_id, id);
    assert_eq!(
        cm.config.overlay_active,
        crate::model::OverlayActive::None,
        "no flight menu opened"
    );
    // in the demo the team mate is not stored
    assert_eq!(cm.config.flarm.team_id, 0);
}

#[test]
#[ignore]
fn flarm_render_flight_menu_and_device_info() {
    use crate::KeyEvent::*;
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    let mut t = 0u32;
    run_ms(&mut cm, &mut cc, &mut t, 500);
    crate::controller::flaps_ctl::set_demo(&mut cm, DemoMode::All);
    run_ms(&mut cm, &mut cc, &mut t, 3000);
    key(&mut cm, &mut cc, BtnEnc);
    for i in 0..3 {
        let d = render(&mut cm);
        dump_ppm(&d, &std::format!("{dir}/flight_menu_{i}.ppm"));
        key(&mut cm, &mut cc, Rotary1Right);
        key(&mut cm, &mut cc, Rotary1Right);
        key(&mut cm, &mut cc, Rotary1Right);
    }
    crate::controller::close_menu_display(&mut cm, &mut cc);
    set_core(&mut cm, PageCore::DeviceInfo);
    cm.config.display_active = DisplayActive::DeviceInfo;
    for i in 0..5 {
        cm.control.device_info_control.index = i * 6;
        let d = render(&mut cm);
        dump_ppm(&d, &std::format!("{dir}/device_info_{i}.ppm"));
    }
}

#[test]
#[ignore]
fn flarm_render_horizon() {
    use embedded_graphics::geometry::AngleUnit;
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    let mut t = 0u32;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    for (i, (roll, pitch, slip)) in [
        (25.0f32, 6.0f32, 4.0f32),
        (0.0, 0.0, 0.0),
        (-50.0, -12.0, -10.0),
    ]
    .iter()
    .enumerate()
    {
        cm.sensor.euler_roll = roll.deg();
        cm.sensor.euler_pitch = pitch.deg();
        cm.sensor.slip_angle = slip.deg();
        set_core(&mut cm, PageCore::Horizon);
        let d = render(&mut cm);
        dump_ppm(&d, &std::format!("{dir}/horizon_ring_{i}.ppm"));
        cm.config.display_active = DisplayActive::Horizon;
        let d = render(&mut cm);
        dump_ppm(&d, &std::format!("{dir}/horizon_full_{i}.ppm"));
    }
}

#[cfg(feature = "flarm")]
#[test]
#[ignore]
fn flarm_render_big_values() {
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    cm.config.page_idx = 3;
    crate::controller::apply_page(&mut cm);
    let mut t = 0u32;
    run_ms(&mut cm, &mut cc, &mut t, 500);
    let d = render(&mut cm);
    dump_ppm(&d, &std::format!("{dir}/big_values_0.ppm"));
    crate::controller::flaps_ctl::set_demo(&mut cm, DemoMode::Flaps);
    for i in 1..4 {
        run_ms(&mut cm, &mut cc, &mut t, 9000);
        let d = render(&mut cm);
        dump_ppm(&d, &std::format!("{dir}/big_values_{i}.ppm"));
    }
}

#[test]
#[ignore]
fn print_sizes() {
    std::println!("CoreModel {}", core::mem::size_of::<CoreModel>());
    std::println!("CoreController {}", core::mem::size_of::<CoreController>());
    std::println!(
        "FlarmCtl {}",
        core::mem::size_of::<crate::controller::FlarmCtl>()
    );
    std::println!(
        "FlarmView {}",
        core::mem::size_of::<crate::flarm::logic::FlarmView>()
    );
    std::println!(
        "Traffic {}",
        core::mem::size_of::<crate::flarm::traffic::Traffic>()
    );
}

/// review 27.10.: the throttled flap flush must leave room in the EEPROM
/// write buffer, and a model reset must not restart the time base
#[test]
fn flap_flush_leaves_room_and_reset_keeps_the_time() {
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0u32;
    run_ms(&mut cm, &mut cc, &mut t, 500);
    crate::controller::flaps_ctl::adopt_template(&mut cm, &mut cc);
    for _ in 0..20 {
        crate::controller::flaps_ctl::flush_flap_dirty(&mut cm, &mut cc);
        assert!(cc.pers_vals.len() <= crate::controller::MAX_PERS_IDS / 2);
    }
    crate::persist::persist_set(
        &mut cc,
        &mut cm,
        crate::utils::Variant::I8(3),
        crate::PersistenceId::Volume,
        crate::Echo::None,
    );
    assert!(cc.pers_vals.contains_key(&crate::PersistenceId::Volume));
    run_ms(&mut cm, &mut cc, &mut t, 2000);
    let before = cm.control.now_ms;
    cm.reset();
    assert_eq!(cm.control.now_ms, before);
}

#[test]
#[ignore]
fn print_u8g2_metrics() {
    use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
    use u8g2_fonts::{fonts, FontRenderer};
    fn m(name: &str, f: FontRenderer, ch: &str) {
        let mut d = TestDisplay::new();
        if f.render_aligned(
            ch,
            embedded_graphics::prelude::Point::new(240, 240),
            VerticalPosition::Baseline,
            HorizontalAlignment::Left,
            FontColor::Transparent(crate::utils::Colors::White),
            &mut d,
        )
        .is_err()
        {
            return;
        }
        let (mut t, mut b, mut l, mut r) = (999, -999, 999, -999);
        for y in 0..480 {
            for x in 0..480 {
                if d.painted_box(x, y, x, y) > 0 {
                    t = t.min(y);
                    b = b.max(y);
                    l = l.min(x);
                    r = r.max(x);
                }
            }
        }
        let adv = f
            .get_rendered_dimensions(
                ch,
                embedded_graphics::prelude::Point::zero(),
                VerticalPosition::Baseline,
            )
            .map(|d| d.advance.x)
            .unwrap_or(0);
        std::println!("{name} '{ch}': h {} w {} adv {}", b - t + 1, r - l + 1, adv);
    }
    macro_rules! f {
        ($n:literal, $t:ty, $c:literal) => {
            m($n, FontRenderer::new::<$t>(), $c)
        };
    }
    f!("inb16", fonts::u8g2_font_inb16_mf, "0");
    f!("inb19", fonts::u8g2_font_inb19_mf, "0");
    f!("inb21", fonts::u8g2_font_inb21_mf, "0");
    f!("inb24", fonts::u8g2_font_inb24_mf, "0");
    f!("inb27", fonts::u8g2_font_inb27_mf, "0");
    f!("inb30", fonts::u8g2_font_inb30_mf, "0");
    f!("inb33", fonts::u8g2_font_inb33_mf, "0");
    f!("inb38", fonts::u8g2_font_inb38_mf, "0");
    f!("inb42", fonts::u8g2_font_inb42_mf, "0");
    f!("inb46", fonts::u8g2_font_inb46_mf, "0");
    f!("inb49", fonts::u8g2_font_inb49_mf, "0");
    f!("inb53", fonts::u8g2_font_inb53_mf, "0");
    f!("inb57n", fonts::u8g2_font_inb57_mn, "0");
    f!("inb63n", fonts::u8g2_font_inb63_mn, "0");
    f!("fub11", fonts::u8g2_font_fub11_tf, "H");
    f!("fub14", fonts::u8g2_font_fub14_tf, "H");
    f!("fub17", fonts::u8g2_font_fub17_tf, "H");
    f!("fub20", fonts::u8g2_font_fub20_tf, "H");
    f!("fub25", fonts::u8g2_font_fub25_tf, "H");
    f!("fub30", fonts::u8g2_font_fub30_tf, "H");
    f!("fub35", fonts::u8g2_font_fub35_tf, "H");
    f!("fub42", fonts::u8g2_font_fub42_tf, "H");
    f!("fub11", fonts::u8g2_font_fub11_tf, "0");
    f!("fub14", fonts::u8g2_font_fub14_tf, "0");
    f!("fub17", fonts::u8g2_font_fub17_tf, "0");
    f!("fub20", fonts::u8g2_font_fub20_tf, "0");
    f!("fub25", fonts::u8g2_font_fub25_tf, "0");
    f!("fub30", fonts::u8g2_font_fub30_tf, "0");
    // other font families available for small text
    f!("helvB08", fonts::u8g2_font_helvB08_tf, "H");
    f!("helvB10", fonts::u8g2_font_helvB10_tf, "H");
    f!("helvB12", fonts::u8g2_font_helvB12_tf, "H");
    f!("helvB14", fonts::u8g2_font_helvB14_tf, "H");
    f!("helvB18", fonts::u8g2_font_helvB18_tf, "H");
    f!("helvB24", fonts::u8g2_font_helvB24_tf, "H");
    f!("ncenB14", fonts::u8g2_font_ncenB14_tf, "H");
    f!("logisoso16", fonts::u8g2_font_logisoso16_tf, "0");
    f!("logisoso18", fonts::u8g2_font_logisoso18_tf, "0");
    f!("logisoso20", fonts::u8g2_font_logisoso20_tf, "0");
    f!("logisoso22", fonts::u8g2_font_logisoso22_tf, "0");
    f!("logisoso24", fonts::u8g2_font_logisoso24_tf, "0");
    f!("logisoso26", fonts::u8g2_font_logisoso26_tf, "0");
    f!("logisoso28", fonts::u8g2_font_logisoso28_tf, "0");
    f!("logisoso30", fonts::u8g2_font_logisoso30_tf, "0");
    f!("logisoso32", fonts::u8g2_font_logisoso32_tf, "0");
    f!("logisoso34", fonts::u8g2_font_logisoso34_tf, "0");
    f!("logisoso38", fonts::u8g2_font_logisoso38_tf, "0");
    f!("logisoso42", fonts::u8g2_font_logisoso42_tf, "0");
    f!("logisoso46", fonts::u8g2_font_logisoso46_tf, "0");
    f!("logisoso50", fonts::u8g2_font_logisoso50_tf, "0");
    f!("logisoso54", fonts::u8g2_font_logisoso54_tf, "0");
    f!("logisoso58", fonts::u8g2_font_logisoso58_tf, "0");
    f!("logisoso62", fonts::u8g2_font_logisoso62_tn, "0");
    f!("logisoso78", fonts::u8g2_font_logisoso78_tn, "0");
}

/// The pilot adds a stage and sets it up (Stefan 29.10.2026: "Add Stage"
/// showed a black screen with the header only, the speed of the new stage
/// could not be changed). Walks the menus like the pilot does and checks
/// what is on the screen after every step.
#[test]
fn flap_stage_is_added_and_set_up_like_a_pilot_does() {
    use crate::KeyEvent::*;
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0u32;
    run_ms(&mut cm, &mut cc, &mut t, 500);
    let dir = std::env::var("RENDER_DIR").ok();
    let mut shot = |cm: &mut CoreModel, name: &str| {
        let d = render(cm);
        if let Some(dir) = &dir {
            dump_ppm(&d, &std::format!("{dir}/flapflow_{name}.ppm"));
        }
        d
    };
    key(&mut cm, &mut cc, BtnEncS3);
    for m in ["Aircraft", "Flaps", "Flap Stages"] {
        goto(&mut cm, &mut cc, m);
        key(&mut cm, &mut cc, BtnEnc);
    }
    assert_eq!(cm.control.menu_control.menu.name, "Flap Stages");
    let n0 = cm.flap_table().len();
    shot(&mut cm, "1_table");

    // Add Stage: the new stage is opened right away with the name entry
    goto(&mut cm, &mut cc, "Add Stage");
    key(&mut cm, &mut cc, BtnEnc);
    run_ms(&mut cm, &mut cc, &mut t, 50);
    shot(&mut cm, "2_after_add");
    assert_eq!(cm.config.flaps.len(), n0 + 1);
    assert_eq!(
        cm.control.menu_control.menu.name, "Flap Stage",
        "the new stage is open"
    );
    assert_eq!(cm.config.flap_edit as usize, n0);
    assert!(
        crate::controller::name_entry::active(&cm),
        "its name is asked for at once"
    );
    // type one character and take it
    key(&mut cm, &mut cc, BtnEnc);
    key(&mut cm, &mut cc, BtnEncS3);
    run_ms(&mut cm, &mut cc, &mut t, 50);
    assert!(!crate::controller::name_entry::active(&cm));
    let name = std::string::String::from(cm.config.flaps.name(n0));
    assert!(!name.is_empty() && name != "-", "name stored: {name:?}");
    shot(&mut cm, "3_stage_menu");

    // the fastest stage has no upper limit: its speed is set as "From"
    let from0 = cm.config.flaps.range(n0).0;
    goto(&mut cm, &mut cc, "From");
    key(&mut cm, &mut cc, BtnEnc);
    shot(&mut cm, "4_from_editor");
    for _ in 0..3 {
        key(&mut cm, &mut cc, Rotary2Right);
    }
    shot(&mut cm, "5_from_turned");
    key(&mut cm, &mut cc, BtnEnc);
    run_ms(&mut cm, &mut cc, &mut t, 50);
    assert_eq!(
        cm.config.flaps.range(n0).0,
        from0 + 3,
        "From moved by 3 km/h"
    );
    assert_eq!(cm.control.menu_control.menu.name, "Flap Stage");

    // a stage in the middle: "Up to" changes with the knob
    goto(&mut cm, &mut cc, "Back");
    key(&mut cm, &mut cc, BtnEnc);
    assert_eq!(cm.control.menu_control.menu.name, "Flap Stages");
    let mid = 3usize;
    let mname = std::string::String::from(cm.config.flaps.name(mid));
    for _ in 0..20 {
        key(&mut cm, &mut cc, Rotary1Left); // back up the list
    }
    goto(&mut cm, &mut cc, &mname);
    key(&mut cm, &mut cc, BtnEnc);
    assert_eq!(cm.config.flap_edit as usize, mid);
    let up0 = cm.config.flaps.range(mid).1;
    goto(&mut cm, &mut cc, "Up to");
    key(&mut cm, &mut cc, BtnEnc);
    for _ in 0..2 {
        key(&mut cm, &mut cc, Rotary2Left);
    }
    shot(&mut cm, "6_upto_turned");
    key(&mut cm, &mut cc, BtnEnc);
    run_ms(&mut cm, &mut cc, &mut t, 50);
    assert_eq!(
        cm.config.flaps.range(mid).1,
        up0 - 2,
        "Up to moved by -2 km/h"
    );
    shot(&mut cm, "7_stage_after");
}

/// every stage of the table: its speed can be changed with the knob (Up to,
/// the fastest one From), and the stage menu shows the right row
#[test]
fn every_flap_stage_speed_can_be_changed() {
    use crate::KeyEvent::*;
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0u32;
    run_ms(&mut cm, &mut cc, &mut t, 500);
    key(&mut cm, &mut cc, BtnEncS3);
    for m in ["Aircraft", "Flaps", "Flap Stages"] {
        goto(&mut cm, &mut cc, m);
        key(&mut cm, &mut cc, BtnEnc);
    }
    let n = cm.flap_table().len();
    for i in 0..n {
        let name = std::string::String::from(cm.flap_table().name(i));
        for _ in 0..20 {
            key(&mut cm, &mut cc, Rotary1Left);
        }
        // the names of the template are unique, the cursor finds the row
        goto(&mut cm, &mut cc, &name);
        key(&mut cm, &mut cc, BtnEnc);
        assert_eq!(cm.config.flap_edit as usize, i);
        let last = i + 1 == n;
        let row = if last { "From" } else { "Up to" };
        let speed = |cm: &CoreModel| {
            if last {
                cm.flap_table().range(i).0
            } else {
                cm.flap_table().range(i).1
            }
        };
        let v0 = speed(&cm);
        goto(&mut cm, &mut cc, row);
        key(&mut cm, &mut cc, BtnEnc);
        key(&mut cm, &mut cc, Rotary2Right);
        let up = speed(&cm);
        key(&mut cm, &mut cc, Rotary2Left);
        key(&mut cm, &mut cc, Rotary2Left);
        let down = speed(&cm);
        assert!(up != v0 || down != v0, "stage {name}: {row} stays at {v0}");
        key(&mut cm, &mut cc, BtnEnc);
        run_ms(&mut cm, &mut cc, &mut t, 50);
        goto(&mut cm, &mut cc, "Back");
        key(&mut cm, &mut cc, BtnEnc);
        assert_eq!(cm.control.menu_control.menu.name, "Flap Stages");
    }
}

/// Screens for the manual: every setup menu, the sound editors with their
/// help text and the preview line, the thermal average box
/// (`RENDER_DIR=/tmp/m WALLPAPER_DIR=... cargo test --lib manual_screens -- --ignored`)
#[test]
#[ignore]
fn manual_screens() {
    use crate::{view::viewable::vario_infoview::LineView, Editable, FloatToSpeed};
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = crate::test_harness!();
    set_core(&mut cm, PageCore::Boxes);
    let mut t = 0u32;
    run_ms(&mut cm, &mut cc, &mut t, 1000);
    // all menus, cursor on the first row
    cm.config.display_active = DisplayActive::Menu;
    for menu in MENU_LIST.iter() {
        if menu.name == "Root" || menu.name.is_empty() {
            continue;
        }
        let menu: &'static Menu = menu;
        cm.control.menu_control.menu = menu;
        fill_values(&mut cm, &mut cc);
        cm.control.menu_control.pos[menu.level] = crate::model::menu::first_visible(&cm, menu, 0);
        let mut d = TestDisplay::new();
        if crate::view::ring_menu::draw_setup_list(&mut d, &cm).is_err() {
            continue;
        }
        let name: std::string::String = menu
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        dump_ppm(&d, &std::format!("{dir}/man_menu_{name}.ppm"));
    }
    // editors with help text; the sound ones with the running preview
    for (e, name) in [
        (Editable::CenterFrequency, "center_frequency"),
        (Editable::SinkTone, "sink_tone"),
        (Editable::TicksUntil, "ticks_until"),
        (Editable::SoundFullScale, "full_scale"),
        (Editable::GustFilterMode, "gust_filter"),
        (Editable::NeedleMode, "vario_signal"),
        (Editable::EnergyArrowMult, "energy_arrow"),
        (Editable::StfSwitchMode, "auto_switch"),
        (Editable::AvgMarkerMode, "avg_marker"),
    ] {
        cm.config.display_active = DisplayActive::Menu;
        crate::controller::activate_editable(e, &mut cm, &mut cc);
        run_ms(&mut cm, &mut cc, &mut t, 2500);
        let v = cm.control.editor.get_value_line(&cm);
        let mut d = TestDisplay::new();
        crate::view::ring_editor::draw_editor(&mut d, &cm, e.name(), v.as_str()).unwrap();
        dump_ppm(&d, &std::format!("{dir}/man_edit_{name}.ppm"));
        crate::controller::close_edit_frame(&mut cm, &mut cc);
    }
    let _ = (LineView::ThermalClimb, 1.0.m_s());
}
