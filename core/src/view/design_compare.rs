//! Screenshots of the P1 / FLARM / flap screens with the contents of the
//! sketches in ring_p1_skizzen/, for the side by side check against them.
//!
//! `RENDER_DIR=/tmp/s WALLPAPER_DIR=../device/larus_frontend_v2/assets \
//!  cargo test --lib design_screens -- --ignored`

use crate::{
    controller::{flaps_ctl, menu::fill_values},
    model::{
        demo::DemoMode,
        menu::{full::MENU_LIST, Menu},
        DisplayActive, OverlayActive, PageCore,
    },
    view::test_fixture::*,
    AirSpeed, CoreController, CoreModel, CoreView, FloatToPressure, FloatToSpeed, LineView,
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

/// the sketch values: ALT 1250 m, AVG +1.8 m/s, TAS 145 km/h, wind 270/25
fn sketch_values(cm: &mut CoreModel) {
    // ISA pressure for 1250 m at QNH 1013.25
    let p = 101_325.0 * (1.0 - 2.25577e-5 * 1250.0f32).powf(5.25588);
    cm.sensor.pressure = p.pa();
    cm.sensor.pressure_altitude.set_static_pressure(p.pa());
    cm.calculated.av2_climb_rate = 1.8.m_s();
    cm.sensor.average_climb_rate = 1.8.m_s();
    cm.sensor.airspeed = AirSpeed::from_speeds(130.0.km_h(), 145.0.km_h());
    cm.sensor.wind_vector.set_speed(25.0.km_h());
    cm.sensor
        .wind_vector
        .set_angle(embedded_graphics::geometry::AngleUnit::deg(270.0f32));
    cm.control.alive_ticks = 1000;
}

fn page(cm: &mut CoreModel, core: PageCore, light: bool, boxes: [LineView; 5]) {
    let i = cm.config.page_idx as usize;
    cm.config.pages[i].core = core;
    cm.config.pages[i].light_outer = light;
    cm.config.pages[i].boxes = boxes;
    cm.config.pages[i].stf_boxes = boxes;
    cm.config.display_active = DisplayActive::Vario;
    cm.config.overlay_active = OverlayActive::None;
}

macro_rules! harness_wp {
    () => {{
        let (_, cc, tx) = crate::test_harness!();
        let dir = std::env::var("WALLPAPER_DIR").unwrap_or_default();
        let mut cm = model_with_wallpapers(&dir);
        let cc = {
            let mut cc = cc;
            // the controller is fine with a fresh model
            cc.recalc_glider(&mut cm);
            cc
        };
        (cm, cc, tx)
    }};
}

#[test]
#[ignore]
fn design_screens() {
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let out = |d: &TestDisplay, name: &str| dump_ppm(d, &std::format!("{dir}/{name}.ppm"));

    // ---------- ring page R2 with flap tape (skizze_ring_schrift R2) ----------
    {
        let (mut cm, mut cc, _tx) = harness_wp!();
        cm.glider_data.basic_glider_data.name = "AS 33 18m";
        let mut t = 0;
        run_ms(&mut cm, &mut cc, &mut t, 300);
        let boxes = [
            LineView::Altitude,
            LineView::AverageClimbRate,
            LineView::WindAndDelta,
            LineView::TrueAirSpeed,
            LineView::None,
        ];
        for (light, name) in [(true, "ring_r2_light"), (false, "ring_r2_dark")] {
            page(&mut cm, PageCore::Boxes, light, boxes);
            sketch_values(&mut cm);
            cm.calculated.flap_rec = Some(5);
            cm.calculated.flap_actual = Some(4.6);
            out(&render(&mut cm), name);
        }
        // wind on top (skizze_windbox W1), L/D and TAS right
        let boxes = [
            LineView::WindAndDelta,
            LineView::AverageClimbRate,
            LineView::TrueAirSpeed,
            LineView::TrueAirSpeed,
            LineView::None,
        ];
        page(&mut cm, PageCore::Boxes, true, boxes);
        sketch_values(&mut cm);
        cm.calculated.flap_rec = Some(5);
        cm.calculated.flap_actual = Some(4.6);
        out(&render(&mut cm), "ring_wind_top");
        // without flaps
        cm.glider_data.basic_glider_data.name = "LS 8";
        let boxes = [
            LineView::Altitude,
            LineView::AverageClimbRate,
            LineView::WindAndDelta,
            LineView::TrueAirSpeed,
            LineView::None,
        ];
        page(&mut cm, PageCore::Boxes, true, boxes);
        sketch_values(&mut cm);
        cm.calculated.flap_rec = None;
        cm.calculated.flap_actual = None;
        out(&render(&mut cm), "ring_no_flaps");
        // big values (skizze_grosse_boxen_v4 C)
        cm.glider_data.basic_glider_data.name = "AS 33 18m";
        let boxes = [
            LineView::Altitude,
            LineView::FlightLevel,
            LineView::PotentialClimb,
            LineView::TrueAirSpeed,
            LineView::None,
        ];
        page(&mut cm, PageCore::BigBoxes, true, boxes);
        sketch_values(&mut cm);
        cm.calculated.flap_rec = Some(5);
        cm.calculated.flap_actual = Some(4.6);
        out(&render(&mut cm), "big_values");
        // horizon H1 in the ring
        use embedded_graphics::geometry::AngleUnit;
        cm.sensor.euler_roll = 25.0f32.deg();
        cm.sensor.euler_pitch = 6.0f32.deg();
        page(&mut cm, PageCore::Horizon, true, boxes);
        out(&render(&mut cm), "horizon_ring");
        cm.config.display_active = DisplayActive::Horizon;
        out(&render(&mut cm), "horizon_full");
    }

    // ---------- FLARM: radar and warnings from the demo ----------
    for (tag, view) in [
        ("core", crate::model::flarm_cfg::WarnView::Core),
        ("full", crate::model::flarm_cfg::WarnView::FullScreen),
    ] {
        let (mut cm, mut cc, _tx) = harness_wp!();
        let mut t = 0;
        run_ms(&mut cm, &mut cc, &mut t, 300);
        cm.config.flarm.warn_view = view;
        page(&mut cm, PageCore::Radar, false, [LineView::None; 5]);
        flaps_ctl::set_demo(&mut cm, DemoMode::Flarm);
        let mut seen: std::vec::Vec<std::string::String> = std::vec::Vec::new();
        for _ in 0..3000 {
            run_ms(&mut cm, &mut cc, &mut t, 100);
            let v = cm.calculated.flarm.view;
            let key = std::format!("{:?}{}", v.kind, v.level);
            let idle_shot = v.kind == crate::flarm::logic::ViewKind::Idle && (t / 100) % 400 == 0;
            if (!seen.contains(&key) && v.level >= 2) || (tag == "core" && idle_shot) {
                if !idle_shot {
                    seen.push(key.clone());
                }
                let d = render(&mut cm);
                out(&d, &std::format!("flarm_{tag}_{key}_{}", t / 1000));
            }
        }
    }

    // ---------- menus (skizze_flarm_menue, skizze_klappen_menue, skizze_menue_ueberschrift) ----------
    {
        let (mut cm, mut cc, _tx) = harness_wp!();
        cm.glider_data.basic_glider_data.name = "AS 33 18m";
        let mut t = 0;
        run_ms(&mut cm, &mut cc, &mut t, 300);
        cm.config.display_active = DisplayActive::Menu;
        let shots: [(&str, usize); 10] = [
            ("Setup", 7),
            ("FLARM", 1),
            ("SteFly Flasher", 1),
            ("Warnings", 0),
            ("Aircraft", 1),
            ("Flaps", 2),
            ("Flap Stages", 3),
            ("Flap Stage", 1),
            ("System", 1),
            ("Vario", 2),
        ];
        for (name, pos) in shots {
            let menu: &'static Menu = MENU_LIST.iter().find(|m| m.name == name).unwrap();
            cm.control.menu_control.menu = menu;
            cm.config.flap_edit = 3;
            fill_values(&mut cm, &mut cc);
            cm.control.menu_control.pos[menu.level] =
                crate::model::menu::first_visible(&cm, menu, pos);
            let mut d = TestDisplay::new();
            crate::view::ring_menu::draw_setup_list(&mut d, &cm).unwrap();
            let fname: std::string::String = name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '_'
                    }
                })
                .collect();
            out(&d, &std::format!("menu_{fname}"));
        }
        // flight menu over the ring page
        cm.config.display_active = DisplayActive::Vario;
        page(
            &mut cm,
            PageCore::Boxes,
            true,
            [
                LineView::Altitude,
                LineView::AverageClimbRate,
                LineView::WindAndDelta,
                LineView::TrueAirSpeed,
                LineView::None,
            ],
        );
        sketch_values(&mut cm);
        crate::controller::menu::activate_menu(
            crate::model::menu::flight_menu(&cm),
            &mut cm,
            &mut cc,
        );
        cm.control.menu_control.pos[1] = 2;
        fill_values(&mut cm, &mut cc);
        out(&render(&mut cm), "menu_flight");
        crate::controller::close_menu_display(&mut cm, &mut cc);
        // device info at the FLARM section
        cm.config.display_active = DisplayActive::DeviceInfo;
        cm.control.device_info_control.index = 7;
        out(&render(&mut cm), "device_info_flarm");
        cm.control.device_info_control.index = 0;
        out(&render(&mut cm), "device_info_top");
        // name entry
        cm.config.display_active = DisplayActive::Menu;
        cm.control.menu_control.menu = MENU_LIST.iter().find(|m| m.name == "Flap Stage").unwrap();
        crate::controller::activate_editable(crate::Editable::FlapName, &mut cm, &mut cc);
        cm.control.editor.content =
            crate::model::editable::Content::String(crate::utils::TString::from_str(""));
        cm.control.editor.name.sel = 11;
        out(&render(&mut cm), "name_entry_abc");
        cm.control.editor.content =
            crate::model::editable::Content::String(crate::utils::TString::from_str("S"));
        cm.control.editor.name.page = 1;
        cm.control.editor.name.sel = 2;
        out(&render(&mut cm), "name_entry_123");
    }
    let _ = 0.0.m_s();
}

/// Battery and satellite symbol of the ring page at candidate positions
/// (sketch for the position decision): `RENDER_DIR=... WALLPAPER_DIR=...
/// cargo test --lib icon_positions -- --ignored`
#[test]
#[ignore]
fn icon_positions() {
    use embedded_graphics::prelude::Point;
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let wp = std::env::var("WALLPAPER_DIR").unwrap_or_default();
    // top left corners: battery 15 x 24 px, satellite 21 x 21 px
    let variants: [(&str, Point, Point); 6] = [
        ("v0", Point::new(371, 382), Point::new(333, 409)),
        ("v1", Point::new(387, 393), Point::new(340, 428)),
        ("v2", Point::new(249, 404), Point::new(216, 406)),
        ("v3", Point::new(249, 52), Point::new(216, 54)),
        ("v4", Point::new(247, 454), Point::new(212, 455)),
        ("v5", Point::new(384, 390), Point::new(337, 423)),
    ];
    for (tag, bat, sat) in variants {
        let (_, cc, _tx) = crate::test_harness!();
        let mut cm = model_with_wallpapers_at(&wp, bat, sat);
        let mut cc = cc;
        cc.recalc_glider(&mut cm);
        cm.glider_data.basic_glider_data.name = "AS 33 18m";
        let mut t = 0;
        run_ms(&mut cm, &mut cc, &mut t, 300);
        let shots = [
            (
                "ring",
                PageCore::Boxes,
                true,
                [
                    LineView::Altitude,
                    LineView::AverageClimbRate,
                    LineView::WindAndDelta,
                    LineView::TrueAirSpeed,
                    LineView::None,
                ],
            ),
            (
                "dark",
                PageCore::Boxes,
                false,
                [
                    LineView::Altitude,
                    LineView::AverageClimbRate,
                    LineView::WindAndDelta,
                    LineView::TrueAirSpeed,
                    LineView::None,
                ],
            ),
            (
                "big",
                PageCore::BigBoxes,
                true,
                [
                    LineView::Altitude,
                    LineView::FlightLevel,
                    LineView::PotentialClimb,
                    LineView::TrueAirSpeed,
                    LineView::None,
                ],
            ),
            ("hor", PageCore::Horizon, true, [LineView::None; 5]),
            ("radar", PageCore::Radar, false, [LineView::None; 5]),
        ];
        for (name, core, light, boxes) in shots {
            page(&mut cm, core, light, boxes);
            sketch_values(&mut cm);
            cm.calculated.flap_rec = Some(5);
            cm.calculated.flap_actual = Some(4.6);
            cm.device.supply_voltage = 10.8;
            cm.calculated.av2_climb_rate = (-2.6).m_s();
            cm.sensor.average_climb_rate = (-2.6).m_s();
            out_ppm(
                &render(&mut cm),
                &std::format!("{dir}/icons_{tag}_{name}.ppm"),
            );
        }
    }
}

fn out_ppm(d: &TestDisplay, path: &str) {
    dump_ppm(d, path)
}

/// needle over the level tube band of the horizon page (-2.6 m/s points
/// straight down through it): `cargo test --lib horizon_needle -- --ignored`
#[test]
#[ignore]
fn horizon_needle() {
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    for (v, name) in [
        (-2.4f32, "hor_needle_down"),
        (-3.4, "hor_needle_right"),
        (1.2, "hor_needle_up"),
    ] {
        page(&mut cm, PageCore::Horizon, true, [LineView::None; 5]);
        cm.calculated.gust_filtered_climb = v.m_s();
        cm.calculated.needle_correction = 0.0.m_s();
        cm.calculated.interpolated_climb_rate.value = v.m_s();
        cm.calculated.interpolated_climb_rate.shown = v.m_s();
        dump_ppm(&render(&mut cm), &std::format!("{dir}/{name}.ppm"));
    }
}

/// flight demo with wind, altitude and flaps on the ring page:
/// `cargo test --lib flight_demo_screens -- --ignored`
#[test]
#[ignore]
fn flight_demo_screens() {
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    page(
        &mut cm,
        PageCore::Boxes,
        true,
        [
            LineView::Altitude,
            LineView::AverageClimbRate,
            LineView::FlightLevel,
            LineView::TrueAirSpeed,
            LineView::None,
        ],
    );
    flaps_ctl::set_demo(&mut cm, DemoMode::Flaps);
    let mut s = 0;
    for shot in [8u32, 16, 30, 44, 52] {
        run_ms(&mut cm, &mut cc, &mut t, (shot - s) * 1000);
        s = shot;
        std::println!(
            "t {shot}s: alt {:.0} m, FL {:.0}, ias {:.0} tas {:.0}, wind {:.0}/{:.0} avg {:.0}/{:.0}, qnh {:.1}",
            cm.sensor.pressure_altitude.qnh_altitude().to_m(),
            cm.sensor.pressure_altitude.qne_altitude().to_m() / 0.3048 / 100.0,
            cm.sensor.airspeed.ias().to_km_h(),
            cm.sensor.airspeed.tas().to_km_h(),
            cm.sensor.wind_vector.angle().to_degrees(),
            cm.sensor.wind_vector.speed().to_km_h(),
            cm.sensor.average_wind.angle().to_degrees(),
            cm.sensor.average_wind.speed().to_km_h(),
            cm.sensor.pressure_altitude.qnh().to_hpa()
        );
        std::println!(
            "   climb {:.2} gust {:.2} corr {:.2} shown {:.2} roll {:.0} circ {}",
            cm.sensor.climb_rate.to_m_s(),
            cm.calculated.gust_filtered_climb.to_m_s(),
            cm.calculated.needle_correction.to_m_s(),
            cm.calculated.interpolated_climb_rate.shown.to_m_s(),
            cm.sensor.euler_roll.to_degrees(),
            cm.control.fly_mode == crate::model::FlyMode::Circling
        );
        // the needle interpolation runs per display frame, which this test
        // does not render: put it on its target
        let g = cm.calculated.gust_filtered_climb + cm.calculated.needle_correction;
        cm.calculated.interpolated_climb_rate.value = g;
        cm.calculated.interpolated_climb_rate.shown = g;
        dump_ppm(&render(&mut cm), &std::format!("{dir}/demo_{shot:02}s.ppm"));
    }
}

/// Page layout editor (plan_seiten_editor.md, sketch skizze_seiten_editor.png):
/// `RENDER_DIR=/tmp/s WALLPAPER_DIR=../device/larus_frontend_v2/assets \
///  cargo test --lib page_layout_screens -- --ignored`
#[test]
#[ignore]
fn page_layout_screens() {
    use crate::{model::page_layout::ListKind, utils::KeyEvent, Event};
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    let boxes = [
        LineView::Altitude,
        LineView::AverageClimbRate,
        LineView::WindAndDelta,
        LineView::TrueAirSpeed,
        LineView::None,
    ];
    page(&mut cm, PageCore::Boxes, true, boxes);
    let i = cm.config.page_idx as usize;
    cm.config.pages[i].cruise_boxes = false;
    let mut go = |cm: &mut CoreModel, cc: &mut CoreController, k: Option<KeyEvent>, ms: u32| {
        if let Some(k) = k {
            cc.event_handler(Event::KeyItem(k), cm);
        }
        run_ms(cm, cc, &mut t, ms);
    };
    let shot = |cm: &mut CoreModel, name: &str| {
        sketch_values(cm);
        cm.calculated.flap_rec = Some(5);
        cm.calculated.flap_actual = Some(4.6);
        dump_ppm(&render(cm), &std::format!("{dir}/layout_{name}.ppm"));
    };
    const ZONE: KeyEvent = KeyEvent::Rotary1Right;
    const VAL: KeyEvent = KeyEvent::Rotary2Right;
    // Page Layout is in Setup > Display since 0.4.1.52 (it was in the
    // flight menu): the editor is started directly
    fill_values(&mut cm, &mut cc);
    crate::controller::page_layout::start(&mut cm);
    go(&mut cm, &mut cc, None, 20);
    shot(&mut cm, "02_scale");
    // upper knob: the list opens at the first click and shows the choice
    go(&mut cm, &mut cc, Some(VAL), 20);
    shot(&mut cm, "03_scale_list_black");
    go(&mut cm, &mut cc, Some(KeyEvent::BtnEsc), 20);
    go(&mut cm, &mut cc, Some(ZONE), 20);
    shot(&mut cm, "04_flap_tape");
    go(&mut cm, &mut cc, Some(ZONE), 20);
    shot(&mut cm, "05_core");
    go(&mut cm, &mut cc, Some(VAL), 20);
    shot(&mut cm, "06_core_list");
    go(&mut cm, &mut cc, Some(KeyEvent::BtnEsc), 20);
    go(&mut cm, &mut cc, Some(ZONE), 20);
    shot(&mut cm, "07_mode");
    go(&mut cm, &mut cc, Some(VAL), 20);
    shot(&mut cm, "08_mode_list");
    go(&mut cm, &mut cc, Some(KeyEvent::BtnEsc), 20);
    go(&mut cm, &mut cc, Some(ZONE), 20);
    shot(&mut cm, "09_centre");
    go(&mut cm, &mut cc, Some(VAL), 20);
    shot(&mut cm, "10_centre_list");
    // pause: the list closes, the centre shows the choice
    go(&mut cm, &mut cc, None, 3000);
    shot(&mut cm, "11_centre_after");
    go(&mut cm, &mut cc, Some(KeyEvent::Rotary2Left), 20);
    go(&mut cm, &mut cc, None, 3000);
    go(&mut cm, &mut cc, Some(ZONE), 20);
    shot(&mut cm, "12_box_top");
    go(&mut cm, &mut cc, Some(VAL), 20);
    shot(&mut cm, "13_box_list");
    go(&mut cm, &mut cc, None, 3000);
    shot(&mut cm, "14_box_after");
    for name in ["15_box_bottom", "16_box_right1", "17_box_right2"] {
        go(&mut cm, &mut cc, Some(ZONE), 20);
        shot(&mut cm, name);
    }
    go(&mut cm, &mut cc, Some(ZONE), 20);
    shot(&mut cm, "18_page");
    go(&mut cm, &mut cc, Some(KeyEvent::BtnEnc), 20);
    shot(&mut cm, "19_page_list");
    go(&mut cm, &mut cc, Some(KeyEvent::BtnEsc), 20);
    go(&mut cm, &mut cc, Some(ZONE), 20);
    shot(&mut cm, "20_exit");
    go(&mut cm, &mut cc, Some(KeyEvent::BtnEnc), 20);
    assert!(cm.control.layout.list == ListKind::Exit);
    shot(&mut cm, "21_exit_list");
    go(&mut cm, &mut cc, Some(KeyEvent::BtnEsc), 20);
    // cruise: flight mode zone -> cruise, the boxes show "cruise = circling"
    cm.control.layout.cruise = true;
    cm.control.layout.zone = 5;
    shot(&mut cm, "22_cruise_box_top");
    cm.control.layout.cruise = false;
    // big values, horizon, status page
    cm.config.pages[i].core = PageCore::BigBoxes;
    cm.control.layout.zone = 4;
    shot(&mut cm, "23_big_value1");
    cm.control.layout.zone = 0;
    go(&mut cm, &mut cc, Some(KeyEvent::BtnEnc), 20);
    go(&mut cm, &mut cc, Some(KeyEvent::BtnEsc), 20);
}

/// wind arrow boxes (live sky blue, average grey) in the slots of the ring
/// page and the big values page:
/// `RENDER_DIR=/tmp/s WALLPAPER_DIR=... cargo test --lib wind_arrow_boxes -- --ignored`
#[test]
#[ignore]
fn wind_arrow_boxes() {
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    use embedded_graphics::geometry::AngleUnit;
    let (mut cm, mut cc, _tx) = harness_wp!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    let set = |cm: &mut CoreModel| {
        sketch_values(cm);
        cm.sensor.average_wind.set_speed(18.0.km_h());
        cm.sensor.average_wind.set_angle(250.0f32.deg());
        cm.control.fly_mode = crate::model::FlyMode::Circling;
        cm.calculated.flap_rec = Some(5);
        cm.calculated.flap_actual = Some(4.6);
    };
    let shots: [(&str, PageCore, bool, [LineView; 5]); 4] = [
        (
            "ring_top_bottom",
            PageCore::Boxes,
            true,
            [
                LineView::WindArrow,
                LineView::AvgWindArrow,
                LineView::Altitude,
                LineView::TrueAirSpeed,
                LineView::None,
            ],
        ),
        (
            "ring_right",
            PageCore::Boxes,
            true,
            [
                LineView::Altitude,
                LineView::AverageClimbRate,
                LineView::WindArrow,
                LineView::AvgWindArrow,
                LineView::None,
            ],
        ),
        (
            "ring_dark",
            PageCore::Boxes,
            false,
            [
                LineView::WindArrow,
                LineView::AverageClimbRate,
                LineView::AvgWindArrow,
                LineView::TrueAirSpeed,
                LineView::None,
            ],
        ),
        (
            "big_values",
            PageCore::BigBoxes,
            true,
            [
                LineView::Altitude,
                LineView::FlightLevel,
                LineView::WindArrow,
                LineView::AvgWindArrow,
                LineView::None,
            ],
        ),
    ];
    for (name, core, light, boxes) in shots {
        page(&mut cm, core, light, boxes);
        set(&mut cm);
        dump_ppm(&render(&mut cm), &std::format!("{dir}/wind_{name}.ppm"));
    }
}

/// two values page (Stefan 28.10.2026):
/// `RENDER_DIR=/tmp/s WALLPAPER_DIR=... cargo test --lib two_values_page -- --ignored`
#[test]
#[ignore]
fn two_values_page() {
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    use embedded_graphics::geometry::AngleUnit;
    let (mut cm, mut cc, _tx) = harness_wp!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    let n = LineView::None;
    let shots: [(&str, bool, LineView, LineView, bool); 5] = [
        (
            "avg_alt_light",
            true,
            LineView::AverageClimbRate,
            LineView::Altitude,
            true,
        ),
        (
            "avg_alt_dark",
            false,
            LineView::AverageClimbRate,
            LineView::Altitude,
            true,
        ),
        (
            "pot_tas_noflaps",
            true,
            LineView::PotentialClimb,
            LineView::TrueAirSpeed,
            false,
        ),
        (
            "wind_fl",
            true,
            LineView::WindArrow,
            LineView::FlightLevel,
            true,
        ),
        (
            "alt_big",
            true,
            LineView::AverageClimbRate,
            LineView::Altitude,
            true,
        ),
    ];
    for (name, light, a, b, flaps) in shots {
        cm.glider_data.basic_glider_data.name = if flaps { "AS 33 18m" } else { "LS 8" };
        page(&mut cm, PageCore::TwoValues, light, [n, n, a, b, n]);
        sketch_values(&mut cm);
        if name == "alt_big" {
            // 4 digit altitude, climb with sign and decimal
            cm.calculated.av2_climb_rate = (-12.5).m_s();
            cm.sensor.average_climb_rate = (-12.5).m_s();
        }
        cm.sensor.average_wind.set_speed(18.0.km_h());
        cm.sensor.average_wind.set_angle(250.0f32.deg());
        cm.calculated.flap_rec = if flaps { Some(5) } else { None };
        cm.calculated.flap_actual = if flaps { Some(4.6) } else { None };
        dump_ppm(&render(&mut cm), &std::format!("{dir}/two_{name}.ppm"));
    }
}

/// every core, light and dark, for the check of the scale line:
/// `RENDER_DIR=/tmp/s WALLPAPER_DIR=... cargo test --lib all_cores_scale -- --ignored`
#[test]
#[ignore]
fn all_cores_scale() {
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    let b = [
        LineView::Altitude,
        LineView::AverageClimbRate,
        LineView::WindAndDelta,
        LineView::TrueAirSpeed,
        LineView::None,
    ];
    for (name, core) in [
        ("boxes", PageCore::Boxes),
        ("big", PageCore::BigBoxes),
        ("two", PageCore::TwoValues),
        ("horizon", PageCore::Horizon),
        ("radar", PageCore::Radar),
        ("thermal", PageCore::ThermalAssist),
        ("wind", PageCore::Wind),
    ] {
        for light in [true, false] {
            page(&mut cm, core, light, b);
            sketch_values(&mut cm);
            cm.calculated.flap_rec = Some(5);
            cm.calculated.flap_actual = Some(4.6);
            let tag = if light { "light" } else { "dark" };
            dump_ppm(
                &render(&mut cm),
                &std::format!("{dir}/core_{name}_{tag}.ppm"),
            );
        }
    }
}

/// flight menu with the value edited in its row:
/// `RENDER_DIR=/tmp/s WALLPAPER_DIR=... cargo test --lib flight_menu_inline -- --ignored`
#[test]
#[ignore]
fn flight_menu_inline() {
    use crate::{utils::KeyEvent, Event};
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    page(
        &mut cm,
        PageCore::Boxes,
        true,
        [
            LineView::Altitude,
            LineView::AverageClimbRate,
            LineView::WindAndDelta,
            LineView::TrueAirSpeed,
            LineView::None,
        ],
    );
    let mut go = |cm: &mut CoreModel, cc: &mut CoreController, k: KeyEvent| {
        cc.event_handler(Event::KeyItem(k), cm);
        run_ms(cm, cc, &mut t, 20);
    };
    let shot = |cm: &mut CoreModel, name: &str| {
        sketch_values(cm);
        dump_ppm(&render(cm), &std::format!("{dir}/fm_{name}.ppm"));
    };
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    let menu = crate::model::menu::flight_menu(&cm);
    let to = |cm: &mut CoreModel, name: &str| {
        // (items that left the flight menu with 0.4.1.52 keep the cursor)
        let pos = menu
            .items
            .iter()
            .position(|it| it.name(cm) == name)
            .unwrap_or(cm.control.menu_control.pos[menu.level]);
        cm.control.menu_control.pos[menu.level] = pos;
    };
    to(&mut cm, "Volume");
    fill_values(&mut cm, &mut cc);
    shot(&mut cm, "1_volume");
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    shot(&mut cm, "2_volume_edit");
    for _ in 0..3 {
        go(&mut cm, &mut cc, KeyEvent::Rotary2Right);
    }
    shot(&mut cm, "3_volume_turned");
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    to(&mut cm, "Brightness");
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    shot(&mut cm, "4_brightness_edit");
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    to(&mut cm, "Gust Filter");
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    shot(&mut cm, "5_gust_edit");
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    // quick adjust on the page (small knob): the editor without gauge arc
    go(&mut cm, &mut cc, KeyEvent::BtnEncS3);
    crate::controller::close_menu_display(&mut cm, &mut cc);
    go(&mut cm, &mut cc, KeyEvent::Rotary2Right);
    shot(&mut cm, "6_quick_volume");
}

/// number editors with the value field and the glider info:
/// `RENDER_DIR=... cargo test --lib value_editors -- --ignored`
#[test]
#[ignore]
fn value_editors() {
    use crate::{utils::KeyEvent, Editable, Event};
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    page(
        &mut cm,
        PageCore::Boxes,
        true,
        [
            LineView::Altitude,
            LineView::AverageClimbRate,
            LineView::WindAndDelta,
            LineView::TrueAirSpeed,
            LineView::None,
        ],
    );
    cm.glider_data.water_ballast = crate::FloatToMass::kg(60.0f32);
    cc.recalc_glider(&mut cm);
    let shot = |cm: &mut CoreModel, name: &str| {
        sketch_values(cm);
        dump_ppm(&render(cm), &std::format!("{dir}/ve_{name}.ppm"));
    };
    // on the page: ballast (button 2), MC, volume, QNH
    for (e, name) in [
        (Editable::WaterBallast, "ballast"),
        (Editable::McCready, "mc"),
        (Editable::Volume, "volume"),
        (Editable::Qnh, "qnh"),
    ] {
        crate::controller::activate_editable(e, &mut cm, &mut cc);
        shot(&mut cm, &std::format!("page_{name}"));
        crate::controller::close_edit_frame(&mut cm, &mut cc);
    }
    // flight menu: ballast edited in its row
    cc.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut cm);
    run_ms(&mut cm, &mut cc, &mut t, 20);
    let menu = crate::model::menu::flight_menu(&cm);
    let pos = menu
        .items
        .iter()
        .position(|it| it.name(&cm) == "Water Ballast")
        .unwrap();
    cm.control.menu_control.pos[menu.level] = pos;
    fill_values(&mut cm, &mut cc);
    cc.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut cm);
    run_ms(&mut cm, &mut cc, &mut t, 20);
    shot(&mut cm, "menu_ballast");
}

/// volume panel and the headers in capitals:
/// `RENDER_DIR=... cargo test --lib volume_panel_screens -- --ignored`
#[test]
#[ignore]
fn volume_panel_screens() {
    use crate::{utils::KeyEvent, Editable, Event, VarioMode};
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    page(
        &mut cm,
        PageCore::Boxes,
        true,
        [
            LineView::Altitude,
            LineView::AverageClimbRate,
            LineView::WindAndDelta,
            LineView::TrueAirSpeed,
            LineView::None,
        ],
    );
    cm.config.volume = 17;
    cm.config.volume_stf = 12;
    cm.control.alarm_volume = 30;
    let mut go = |cm: &mut CoreModel, cc: &mut CoreController, k: KeyEvent| {
        cc.event_handler(Event::KeyItem(k), cm);
        run_ms(cm, cc, &mut t, 20);
    };
    let shot = |cm: &mut CoreModel, name: &str| {
        sketch_values(cm);
        dump_ppm(&render(cm), &std::format!("{dir}/vp_{name}.ppm"));
    };
    cm.control.vario_mode = VarioMode::Vario;
    go(&mut cm, &mut cc, KeyEvent::Rotary2Right);
    shot(&mut cm, "1_page_circling");
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    cm.control.vario_mode = VarioMode::SpeedToFly;
    go(&mut cm, &mut cc, KeyEvent::Rotary2Left);
    shot(&mut cm, "2_page_cruise");
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    cm.control.vario_mode = VarioMode::Vario;
    // flight menu > Volume: picking
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    shot(&mut cm, "3_flight_menu");
    let menu = crate::model::menu::flight_menu(&cm);
    // the volume left the flight menu with 0.4.1.52 (inner knob on the page)
    let Some(pos) = menu.items.iter().position(|it| it.name(&cm) == "Volume") else {
        return;
    };
    cm.control.menu_control.pos[menu.level] = pos;
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    go(&mut cm, &mut cc, KeyEvent::Rotary2Right);
    go(&mut cm, &mut cc, KeyEvent::Rotary2Right);
    shot(&mut cm, "4_menu_pick_warning");
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    shot(&mut cm, "5_menu_change_warning");
    go(&mut cm, &mut cc, KeyEvent::BtnEnc);
    go(&mut cm, &mut cc, KeyEvent::BtnEncS3);
    crate::controller::close_menu_display(&mut cm, &mut cc);
    // MC editor with its glide line, ballast
    crate::controller::activate_editable(Editable::McCready, &mut cm, &mut cc);
    shot(&mut cm, "6_mc");
    crate::controller::close_edit_frame(&mut cm, &mut cc);
    crate::controller::activate_editable(Editable::WaterBallast, &mut cm, &mut cc);
    shot(&mut cm, "7_ballast");
    crate::controller::close_edit_frame(&mut cm, &mut cc);
}

/// menu redesign 28.10.2026 (cap header, value bar, no rim arcs, glider
/// icon): `RENDER_DIR=... cargo test --lib menu_design_screens -- --ignored`
#[test]
#[ignore]
fn menu_design_screens() {
    use crate::model::menu::full::{STEFLY_SETTINGS, STEFLY_SYSTEM};
    use crate::{DisplayActive, Editable};
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    page(
        &mut cm,
        PageCore::Boxes,
        true,
        [
            LineView::Altitude,
            LineView::AverageClimbRate,
            LineView::WindAndDelta,
            LineView::TrueAirSpeed,
            LineView::None,
        ],
    );
    let out = |d: &TestDisplay, name: &str| dump_ppm(d, &std::format!("{dir}/md_{name}.ppm"));
    // setup root on Aircraft, System in the middle
    cm.config.display_active = DisplayActive::Menu;
    for (menu, item, name) in [
        (&STEFLY_SETTINGS, "Aircraft", "setup_aircraft"),
        (&STEFLY_SYSTEM, "Battery Good", "system"),
    ] {
        cm.control.menu_control.menu = menu;
        fill_values(&mut cm, &mut cc);
        let pos = menu
            .items
            .iter()
            .position(|it| it.name(&cm) == item)
            .unwrap();
        cm.control.menu_control.pos[menu.level] = pos;
        let mut d = TestDisplay::new();
        crate::view::ring_menu::draw_setup_list(&mut d, &cm).unwrap();
        out(&d, name);
    }
    // full screen editors: number, choice, list
    for (e, name) in [
        (Editable::StfSwitchSpeed, "edit_number"),
        (Editable::Theme, "edit_enum"),
        (Editable::Glider, "edit_list"),
    ] {
        crate::controller::activate_editable(e, &mut cm, &mut cc);
        let v = cm.control.editor.get_value_line(&cm);
        let mut d = TestDisplay::new();
        crate::view::ring_editor::draw_editor(&mut d, &cm, e.name(), v.as_str()).unwrap();
        out(&d, name);
        crate::controller::close_edit_frame(&mut cm, &mut cc);
    }
    // progress
    let mut d = TestDisplay::new();
    crate::view::ring_editor::draw_progress(
        &mut d,
        &cm,
        "Firmware Update",
        Some(40),
        "do not switch off",
    )
    .unwrap();
    out(&d, "progress");
    // on the page: the ballast editor over the dark core
    cm.config.display_active = DisplayActive::Vario;
    page(
        &mut cm,
        PageCore::Boxes,
        false,
        [
            LineView::Altitude,
            LineView::AverageClimbRate,
            LineView::WindAndDelta,
            LineView::TrueAirSpeed,
            LineView::None,
        ],
    );
    cm.glider_data.water_ballast = crate::FloatToMass::kg(60.0f32);
    cc.recalc_glider(&mut cm);
    crate::controller::activate_editable(Editable::WaterBallast, &mut cm, &mut cc);
    sketch_values(&mut cm);
    out(&render(&mut cm), "page_ballast_dark");
}

/// GNSS menu 09.11.2026: antenna items only with two antennas, plain names
/// and the value explained below the bar:
/// `RENDER_DIR=... cargo test --lib gnss_menu_screens -- --ignored`
#[test]
#[ignore]
fn gnss_menu_screens() {
    use crate::model::menu::full::SENSORBOX_GNSS;
    use crate::{CanConfigId, DisplayActive, Editable};
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    let out = |d: &TestDisplay, name: &str| dump_ppm(d, &std::format!("{dir}/gnss_{name}.ppm"));
    cm.config.display_active = DisplayActive::Menu;
    cm.control.menu_control.menu = &SENSORBOX_GNSS;
    for (cfg, tag) in [(1.0, "menu_cfg1"), (2.0, "menu_cfg2")] {
        cm.sensor
            .remote
            .set(CanConfigId::GnssConfig.remote_slot(), cfg);
        cm.sensor
            .remote
            .set(CanConfigId::AntBaselen.remote_slot(), 1.84);
        cm.sensor
            .remote
            .set(CanConfigId::AntSlaveDown.remote_slot(), 0.20);
        cm.sensor
            .remote
            .set(CanConfigId::AntSlaveRight.remote_slot(), -0.06);
        crate::controller::menu::fill_values(&mut cm, &mut cc);
        cm.control.menu_control.pos[SENSORBOX_GNSS.level] = if cfg > 1.5 { 2 } else { 0 };
        let mut d = TestDisplay::new();
        crate::view::ring_menu::draw_setup_list(&mut d, &cm).unwrap();
        out(&d, tag);
    }
    for (v, tag) in [(1.0, "edit_config_essential"), (4.0, "edit_config_pro")] {
        crate::controller::activate_editable(Editable::GnssConfig, &mut cm, &mut cc);
        cm.control.editor.content = crate::model::editable::Content::F32(Some(v));
        let val = cm.control.editor.get_value_line(&cm);
        let mut d = TestDisplay::new();
        crate::view::ring_editor::draw_editor(
            &mut d,
            &cm,
            Editable::GnssConfig.name(),
            val.as_str(),
        )
        .unwrap();
        out(&d, tag);
        crate::controller::close_edit_frame(&mut cm, &mut cc);
    }
    // the save dialog: Competition > Competition Pro
    crate::controller::activate_editable(Editable::RemoteSave, &mut cm, &mut cc);
    cm.control.editor.pending = Some(crate::controller::PendingRemote {
        target: Editable::GnssConfig,
        config_id: CanConfigId::GnssConfig,
        old: crate::model::editable::Content::F32(Some(2.0)),
        new: crate::model::editable::Content::F32(Some(4.0)),
    });
    {
        let val = cm.control.editor.get_value_line(&cm);
        let mut d = TestDisplay::new();
        crate::view::ring_editor::draw_editor(
            &mut d,
            &cm,
            Editable::RemoteSave.name(),
            val.as_str(),
        )
        .unwrap();
        out(&d, "save_dialog");
    }
    cm.control.editor.pending = None;
    crate::controller::close_edit_frame(&mut cm, &mut cc);
    for (e, tag) in [
        (Editable::GnssConfig, "edit_config"),
        (Editable::AntBaselen, "edit_distance"),
        (Editable::AntSlaveDown, "edit_lower"),
        (Editable::AntSlaveRight, "edit_right"),
    ] {
        crate::controller::activate_editable(e, &mut cm, &mut cc);
        let v = cm.control.editor.get_value_line(&cm);
        let mut d = TestDisplay::new();
        crate::view::ring_editor::draw_editor(&mut d, &cm, e.name(), v.as_str()).unwrap();
        out(&d, tag);
        crate::controller::close_edit_frame(&mut cm, &mut cc);
    }
}

/// flight menu typography 09.11.2026, old (0.4.1.44) against new, light and
/// dark page, a few cursor positions and a value being edited:
/// `RENDER_DIR=... cargo test --lib flight_menu_sizes -- --ignored`
#[test]
#[ignore]
fn flight_menu_sizes() {
    use crate::utils::KeyEvent;
    use crate::view::ring_menu::{draw_flight_list_styled, FLIGHT_STYLE, FLIGHT_STYLE_044};
    use crate::Event;
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    for light in [true, false] {
        let l = if light { "light" } else { "dark" };
        page(
            &mut cm,
            PageCore::Boxes,
            light,
            [
                LineView::Altitude,
                LineView::AverageClimbRate,
                LineView::WindAndDelta,
                LineView::TrueAirSpeed,
                LineView::None,
            ],
        );
        cc.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut cm);
        run_ms(&mut cm, &mut cc, &mut t, 20);
        let menu = crate::model::menu::flight_menu(&cm);
        for item in ["MC", "Volume", "Gust Filter", "Page Layout"] {
            let pos = match menu.items.iter().position(|it| it.name(&cm) == item) {
                Some(p) => p,
                None => continue,
            };
            cm.control.menu_control.pos[menu.level] = pos;
            fill_values(&mut cm, &mut cc);
            sketch_values(&mut cm);
            for (tag, st) in [("old", &FLIGHT_STYLE_044), ("new", &FLIGHT_STYLE)] {
                let mut d = render(&mut cm);
                draw_flight_list_styled(&mut d, &cm, st).unwrap();
                let it: std::string::String =
                    item.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
                dump_ppm(&d, &std::format!("{dir}/fs_{l}_{it}_{tag}.ppm"));
            }
        }
        // editing QNH in its row
        let pos = menu
            .items
            .iter()
            .position(|it| it.name(&cm) == "QNH")
            .unwrap();
        cm.control.menu_control.pos[menu.level] = pos;
        cc.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut cm);
        run_ms(&mut cm, &mut cc, &mut t, 20);
        for (tag, st) in [("old", &FLIGHT_STYLE_044), ("new", &FLIGHT_STYLE)] {
            let mut d = render(&mut cm);
            draw_flight_list_styled(&mut d, &cm, st).unwrap();
            dump_ppm(&d, &std::format!("{dir}/fs_{l}_QNHedit_{tag}.ppm"));
        }
        cc.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut cm);
        run_ms(&mut cm, &mut cc, &mut t, 20);
        crate::controller::close_menu_display(&mut cm, &mut cc);
        run_ms(&mut cm, &mut cc, &mut t, 20);
    }
}

/// POT box with its yes / no (green disc with check mark, white disc with cross) in every
/// place a box can be:
/// `RENDER_DIR=/tmp/s WALLPAPER_DIR=../device/larus_frontend_v2/assets cargo test --lib pot_symbol_screens -- --ignored`
#[test]
#[ignore]
fn pot_symbol_screens() {
    use crate::model::PotVerdict;
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    let n = LineView::None;
    let pot = LineView::PotentialClimb;
    // (name, core, boxes [top, bottom, right up, right low, -])
    let layouts: [(&str, PageCore, [LineView; 5]); 5] = [
        (
            "bottom",
            PageCore::Boxes,
            [
                LineView::Altitude,
                pot,
                LineView::WindAndDelta,
                LineView::TrueAirSpeed,
                n,
            ],
        ),
        (
            "top",
            PageCore::Boxes,
            [
                pot,
                LineView::Altitude,
                LineView::WindAndDelta,
                LineView::TrueAirSpeed,
                n,
            ],
        ),
        (
            "right",
            PageCore::Boxes,
            [
                LineView::Altitude,
                LineView::AverageClimbRate,
                pot,
                LineView::TrueAirSpeed,
                n,
            ],
        ),
        (
            "big",
            PageCore::BigBoxes,
            [
                LineView::Altitude,
                LineView::TrueAirSpeed,
                pot,
                LineView::AverageClimbRate,
                n,
            ],
        ),
        (
            "two",
            PageCore::TwoValues,
            [n, n, pot, LineView::Altitude, n],
        ),
    ];
    // (name, value, estimate, yes / no)
    let states: [(&str, f32, bool, PotVerdict); 5] = [
        ("cruise", -0.6, true, PotVerdict::None),
        ("yes_est", 2.3, true, PotVerdict::Yes),
        ("yes", 2.6, false, PotVerdict::Yes),
        ("open", 1.1, true, PotVerdict::None),
        ("no", 0.4, true, PotVerdict::No),
    ];
    for (lname, core, boxes) in layouts {
        for light in [true, false] {
            for (sname, v, est, verdict) in states {
                page(&mut cm, core, light, boxes);
                sketch_values(&mut cm);
                cm.config.mc_cready = 1.5.m_s();
                cm.calculated.flap_rec = Some(5);
                cm.calculated.flap_actual = Some(4.6);
                cm.calculated.pot_climb = Some(v.m_s());
                cm.calculated.pot_predictive = est;
                cm.calculated.pot_verdict = verdict;
                // in flight with GNSS (without it the box shows the average and no yes / no)
                cm.sensor.gps_state = crate::model::GpsState::HeadingAvail;
                cm.sensor.gps_sats = 14;
                cm.control.system_state = crate::model::SystemState::CanAndGpsOk;
                let th = if light { "light" } else { "dark" };
                dump_ppm(
                    &render(&mut cm),
                    &std::format!("{dir}/pot_{lname}_{th}_{sname}.ppm"),
                );
            }
        }
    }
}

/// the thermal average box (15.11.2026) on the ring page, circling and
/// cruise: `RENDER_DIR=... WALLPAPER_DIR=... cargo test --lib thermal_box_page -- --ignored`
#[test]
#[ignore]
fn thermal_box_page() {
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    page(
        &mut cm,
        PageCore::Boxes,
        false,
        [
            LineView::Altitude,
            LineView::ThermalClimb,
            LineView::WindAndDelta,
            LineView::AverageClimbRate,
            LineView::None,
        ],
    );
    sketch_values(&mut cm);
    cm.control.tcr_mode = crate::model::TcrMode::Climbing;
    cm.calculated.thermal_climb_rate = 2.3.m_s();
    dump_ppm(&render(&mut cm), &std::format!("{dir}/thermal_box.ppm"));
}

/// mode symbol on every kind of page, vario and speed to fly mode:
/// `RENDER_DIR=... WALLPAPER_DIR=... cargo test --lib mode_symbol_pages -- --ignored`
#[test]
#[ignore]
fn mode_symbol_pages() {
    use crate::CenterView;
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let b = [
        LineView::Altitude,
        LineView::AverageClimbRate,
        LineView::WindAndDelta,
        LineView::TrueAirSpeed,
        LineView::None,
    ];
    // (name, core, flap tape, centre circle, full screen)
    let cases = [
        ("1_boxes", PageCore::Boxes, true, true, false),
        ("2_boxes_notape", PageCore::Boxes, false, true, false),
        ("3_boxes_nocentre", PageCore::Boxes, true, false, false),
        (
            "4_boxes_notape_nocentre",
            PageCore::Boxes,
            false,
            false,
            false,
        ),
        ("5_big", PageCore::BigBoxes, true, true, false),
        ("6_big_notape", PageCore::BigBoxes, false, true, false),
        ("7_two", PageCore::TwoValues, true, true, false),
        ("8_centering", PageCore::ThermalAssist, false, true, false),
        ("9_wind", PageCore::Wind, false, true, false),
        ("10_horizon", PageCore::Horizon, false, true, false),
        ("11_horizon_light", PageCore::Horizon, false, true, true),
    ];
    // long numbers in the boxes next to the symbol (bottom, right 1, right 2)
    let long = [
        (
            "20_long_wind_bottom",
            true,
            [
                LineView::Altitude,
                LineView::WindAndDelta,
                LineView::TrueAirSpeed,
                LineView::Altitude,
                LineView::None,
            ],
            false,
        ),
        (
            "21_long_wind_bottom_notape",
            false,
            [
                LineView::Altitude,
                LineView::WindAndDelta,
                LineView::TrueAirSpeed,
                LineView::Altitude,
                LineView::None,
            ],
            false,
        ),
        (
            "22_long_fpm_ft",
            true,
            [
                LineView::Altitude,
                LineView::AverageClimbRate,
                LineView::WindAndDelta,
                LineView::Altitude,
                LineView::None,
            ],
            true,
        ),
        (
            "23_long_fpm_ft_notape",
            false,
            [
                LineView::Altitude,
                LineView::AverageClimbRate,
                LineView::WindAndDelta,
                LineView::Altitude,
                LineView::None,
            ],
            true,
        ),
    ];
    for (name, tape, boxes, imperial) in long {
        let (mut cm, mut cc, _tx) = harness_wp!();
        cm.glider_data.basic_glider_data.name = "AS 33 18m";
        let mut t = 0;
        run_ms(&mut cm, &mut cc, &mut t, 300);
        page(&mut cm, PageCore::Boxes, false, boxes);
        let i = cm.config.page_idx as usize;
        cm.config.pages[i].flap_tape_off = !tape;
        if imperial {
            cm.config.unit_vertical_speed = crate::UnitVerticalSpeed::Fpm;
            cm.config.unit_height = crate::UnitHeight::Feet;
            cm.config.unit_horizontal_speed = crate::UnitHorizontalSpeed::Knots;
        }
        sketch_values(&mut cm);
        // 3600 m = 11811 ft, -6.4 m/s = -1260 fpm
        let p = 101_325.0 * (1.0 - 2.25577e-5 * 3600.0f32).powf(5.25588);
        cm.sensor.pressure = p.pa();
        cm.sensor.pressure_altitude.set_static_pressure(p.pa());
        cm.calculated.av2_climb_rate = (-6.4).m_s();
        cm.sensor.average_climb_rate = (-6.4).m_s();
        cm.sensor.wind_vector.set_speed(118.0.km_h());
        for (mode, m) in [
            (crate::VarioMode::Vario, "vario"),
            (crate::VarioMode::SpeedToFly, "stf"),
        ] {
            cm.control.vario_mode = mode;
            dump_ppm(
                &render(&mut cm),
                &std::format!("{dir}/modepg_{name}_{m}.ppm"),
            );
        }
    }
    for (name, core, tape, centre, full) in cases {
        let (mut cm, mut cc, _tx) = harness_wp!();
        cm.glider_data.basic_glider_data.name = "AS 33 18m";
        let mut t = 0;
        run_ms(&mut cm, &mut cc, &mut t, 300);
        page(&mut cm, core, false, b);
        let i = cm.config.page_idx as usize;
        cm.config.pages[i].flap_tape_off = !tape;
        if !centre {
            cm.config.pages[i].center_circling = CenterView::None;
            cm.config.pages[i].center_straight = CenterView::None;
        }
        if full {
            // (here: the light outer band)
            cm.config.pages[i].light_outer = true;
        }
        sketch_values(&mut cm);
        for (mode, m) in [
            (crate::VarioMode::Vario, "vario"),
            (crate::VarioMode::SpeedToFly, "stf"),
        ] {
            cm.control.vario_mode = mode;
            dump_ppm(
                &render(&mut cm),
                &std::format!("{dir}/modepg_{name}_{m}.ppm"),
            );
        }
    }
}

/// both centering aids as the page shows them while circling (in the centre
/// of an info box page and as the core "Centering aid"):
/// `RENDER_DIR=... WALLPAPER_DIR=... cargo test --lib centering_pages -- --ignored`
#[test]
#[ignore]
fn centering_pages() {
    use crate::{CenterView, FlyMode, VarioMode};
    use embedded_graphics::geometry::AngleUnit;
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let b = [
        LineView::Altitude,
        LineView::AverageClimbRate,
        LineView::WindAndDelta,
        LineView::TrueAirSpeed,
        LineView::None,
    ];
    for (name, core, centre, ticks) in [
        (
            "spider_boxes",
            PageCore::Boxes,
            CenterView::ThermalAssistant2,
            150,
        ),
        (
            "spider_boxes_part",
            PageCore::Boxes,
            CenterView::ThermalAssistant2,
            60,
        ),
        (
            "dots_boxes",
            PageCore::Boxes,
            CenterView::ThermalAssistant1,
            150,
        ),
        (
            "dots_core",
            PageCore::ThermalAssist,
            CenterView::ThermalAssistant1,
            200,
        ),
        (
            "ring3d_boxes",
            PageCore::Boxes,
            CenterView::ThermalAssistant3,
            150,
        ),
        (
            "ring3d_boxes_part",
            PageCore::Boxes,
            CenterView::ThermalAssistant3,
            60,
        ),
    ] {
        let (mut cm, mut cc, _tx) = harness_wp!();
        let mut t = 0;
        run_ms(&mut cm, &mut cc, &mut t, 300);
        page(&mut cm, core, false, b);
        let i = cm.config.page_idx as usize;
        cm.config.pages[i].center_circling = centre;
        sketch_values(&mut cm);
        cm.control.vario_mode = VarioMode::Vario;
        cm.control.fly_mode = FlyMode::Circling;
        cm.control.circling_direction = crate::model::CirclingDirection::Left;
        cm.calculated.av2_climb_rate = 1.5.m_s();
        cm.sensor.average_climb_rate = 1.5.m_s();
        cm.sensor.euler_roll = 40.0f32.deg();
        let mut view = CoreView::new(TestDisplay::new(), &cm);
        let mut yaw = 90.0f32;
        for _ in 0..ticks {
            let rel = (yaw - 30.0).to_radians();
            let climb = 1.5 + 1.6 * rel.cos()
                - 1.2 * (-((yaw - 130.0).rem_euclid(360.0) - 180.0).abs() / 12.0).exp();
            cm.sensor.euler_yaw = yaw.rem_euclid(360.0).deg();
            cm.sensor.climb_rate = climb.m_s();
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            view.prepare(&mut cm);
            view.draw().unwrap();
            yaw += 3.0;
        }
        dump_ppm(&view.display, &std::format!("{dir}/centering_{name}.ppm"));
    }
}

/// title picture of the manual: needle at 1.4 m/s, both wind arrows and the
/// energy arrow, no satellite symbol, no glider outline:
/// `RENDER_DIR=... WALLPAPER_DIR=... cargo test --lib manual_cover -- --ignored`
#[test]
#[ignore]
fn manual_cover() {
    use crate::{CenterView, FlyMode, VarioMode};
    use embedded_graphics::geometry::AngleUnit;
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    cm.glider_data.basic_glider_data.name = "AS 33 18m";
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    page(
        &mut cm,
        PageCore::Boxes,
        false,
        [
            LineView::Altitude,
            LineView::AverageClimbRate,
            LineView::WindAndDelta,
            LineView::TrueAirSpeed,
            LineView::None,
        ],
    );
    let i = cm.config.page_idx as usize;
    cm.config.pages[i].center_straight = CenterView::DoubleArrowStraight;
    cm.config.pages[i].center_circling = CenterView::DoubleArrowCircling;
    cm.config.glider_symbol = false;
    sketch_values(&mut cm);
    cm.control.system_state = crate::model::SystemState::CanAndGpsOk;
    cm.control.vario_mode = VarioMode::Vario;
    cm.control.fly_mode = FlyMode::StraightFlight;
    cm.control.energy_arrow_mult = 2.0;
    cm.sensor.average_wind.set_speed(20.0.km_h());
    cm.sensor.average_wind.set_angle(235.0f32.deg());
    cm.sensor.climb_rate = 1.4.m_s();
    cm.calculated.gust_filtered_climb = 1.4.m_s();
    cm.calculated.interpolated_climb_rate.value = 1.4.m_s();
    cm.calculated.interpolated_climb_rate.shown = 1.4.m_s();
    dump_ppm(&render(&mut cm), &std::format!("{dir}/cover.ppm"));
}

/// Screens of the simple operation (15.11.2026) for the manual and for a
/// look at them: error texts, short flight menu, Basic setup with Back as
/// the first row, presets, wizard, guided calibration, page styles
/// (`RENDER_DIR=/tmp/si WALLPAPER_DIR=... cargo test --lib simple_screens -- --ignored`)
#[test]
#[ignore]
fn simple_screens() {
    use crate::{
        controller::{activate_editable, close_menu_display, wizard},
        model::{editable::simple::*, menu::basic, SystemState},
        Editable, Event, KeyEvent,
    };
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let key = |cm: &mut CoreModel, cc: &mut CoreController, t: &mut u32, k: KeyEvent| {
        cc.event_handler(Event::KeyItem(k), cm);
        run_ms(cm, cc, t, 20);
    };
    let shot = |cm: &mut CoreModel, name: &str| {
        dump_ppm(&render(cm), &std::format!("{dir}/sim_{name}.ppm"))
    };
    // cursor on the row with this name in the current menu
    let cursor = |cm: &mut CoreModel, name: &str| {
        let menu = cm.control.menu_control.menu;
        let pos = menu
            .items
            .iter()
            .position(|it| it.name(cm) == name)
            .unwrap_or_else(|| panic!("{name} not in {}", menu.name));
        cm.control.menu_control.pos[menu.level] = pos;
    };
    let b = [
        LineView::Altitude,
        LineView::AverageClimbRate,
        LineView::WindAndDelta,
        LineView::TrueAirSpeed,
        LineView::None,
    ];
    let fresh = |basic_mode: bool| {
        let (mut cm, mut cc, _tx) = harness_wp!();
        cm.glider_data.basic_glider_data.name = "AS 33 18m";
        cm.config.basic_mode = basic_mode;
        let mut t = 0u32;
        run_ms(&mut cm, &mut cc, &mut t, 300);
        page(&mut cm, PageCore::Boxes, false, b);
        sketch_values(&mut cm);
        cm.control.system_state = SystemState::CanAndGpsOk;
        (cm, cc, t)
    };

    // ---- the page: all well, no GPS, no sensor box
    let (mut cm, _cc, _t) = fresh(true);
    cm.control.vario_mode = crate::VarioMode::Vario;
    shot(&mut cm, "page_ok");
    cm.control.system_state = SystemState::CanOk;
    shot(&mut cm, "page_nogps");
    for (core, name) in [
        (PageCore::BigBoxes, "big"),
        (PageCore::TwoValues, "two"),
        (PageCore::Horizon, "horizon"),
        (PageCore::ThermalAssist, "centering"),
        (PageCore::Wind, "wind"),
    ] {
        let i = cm.config.page_idx as usize;
        cm.config.pages[i].core = core;
        shot(&mut cm, &std::format!("page_nogps_{name}"));
        cm.control.system_state = SystemState::NoCom;
        cm.control.boot_seconds = 60;
        shot(&mut cm, &std::format!("page_nobox_{name}"));
        cm.control.system_state = SystemState::CanOk;
    }
    let i = cm.config.page_idx as usize;
    cm.config.pages[i].core = PageCore::Boxes;
    cm.control.system_state = SystemState::NoCom;
    cm.control.boot_seconds = 60;
    shot(&mut cm, "page_nobox");

    // ---- flight menu (with the page of a new device: Page Style "Standard")
    let (mut cm, mut cc, mut t) = fresh(true);
    cm.config.pages = crate::model::page::default_pages();
    key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
    shot(&mut cm, "flight_menu");
    key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
    key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary1Right);
    shot(&mut cm, "flight_menu_mc");
    key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
    close_menu_display(&mut cm, &mut cc);

    // ---- Basic setup: every menu
    cm.sensor.airspeed = AirSpeed::from_speeds(0.0.km_h(), 0.0.km_h());
    key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEncS3);
    shot(&mut cm, "basic_setup");
    key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary2Left);
    shot(&mut cm, "basic_setup_back");
    for (row, name) in [
        ("Sound", "sound"),
        ("Display", "display"),
        ("Units", "units"),
        ("Aircraft", "aircraft"),
        ("Sensor Setup", "sensor"),
        ("System", "system"),
    ] {
        cursor(&mut cm, row);
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        shot(&mut cm, &std::format!("basic_{name}"));
        if name == "sensor" {
            // two measurements done
            for (i, row) in ["Measure 1", "Measure 2", "Measure 3"].iter().enumerate() {
                let menu = cm.control.menu_control.menu;
                cm.control.menu_control.pos[menu.level] = i;
                let _ = row;
                key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
                if i == 0 {
                    shot(&mut cm, "basic_sensor_cmd");
                }
                key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
                if i == 1 {
                    shot(&mut cm, "basic_sensor_2done");
                }
            }
            shot(&mut cm, "basic_sensor_ready");
        }
        cursor(&mut cm, "Back");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
    }
    // the cruise switch at a speed: its row appears
    cursor(&mut cm, "Cruise Switch");
    key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
    key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary2Right);
    shot(&mut cm, "edit_cruise_switch");
    key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
    shot(&mut cm, "basic_setup_at_speed");
    close_menu_display(&mut cm, &mut cc);

    // ---- editors with their help text
    for (e, name, turns) in [
        (Editable::VarioResponse, "vario_response", 0),
        (Editable::SoundPreset, "sound_preset", 1),
        (Editable::PageStyle, "page_style", 1),
        (Editable::UsageMode, "usage_mode", 0),
        (Editable::Glider, "glider", 0),
        (Editable::PilotWeight, "pilot_weight", 0),
        (Editable::UserProfile, "user_profile", 0),
    ] {
        cm.config.display_active = DisplayActive::Menu;
        activate_editable(e, &mut cm, &mut cc);
        cm.control.editor.from_menu = true;
        for _ in 0..turns {
            key(&mut cm, &mut cc, &mut t, KeyEvent::Rotary2Right);
        }
        run_ms(&mut cm, &mut cc, &mut t, 4200);
        shot(&mut cm, &std::format!("edit_{name}"));
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEncS3);
        run_ms(&mut cm, &mut cc, &mut t, 100);
    }

    // ---- wizard
    let (mut cm, mut cc, mut t) = fresh(true);
    cm.sensor.airspeed = AirSpeed::from_speeds(0.0.km_h(), 0.0.km_h());
    wizard::start(&mut cm, &mut cc);
    run_ms(&mut cm, &mut cc, &mut t, 50);
    for n in 1..=6 {
        shot(&mut cm, &std::format!("wizard_{n}"));
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
    }
    assert!(cm.control.menu_control.menu == &basic::SENSOR);
    shot(&mut cm, "wizard_end");

    // ---- page styles
    for (style, name) in [
        (STYLE_STANDARD, "standard"),
        (STYLE_SIMPLE, "simple"),
        (STYLE_BIG, "big"),
        (STYLE_COMPETITION, "competition"),
    ] {
        let (mut cm, _cc, _t) = fresh(true);
        let i = cm.config.page_idx as usize;
        cm.config.pages[i] = styled_page(style, cm.config.pages[i]).unwrap();
        cm.control.vario_mode = crate::VarioMode::Vario;
        cm.control.fly_mode = crate::FlyMode::Circling;
        shot(&mut cm, &std::format!("style_{name}"));
        cm.control.vario_mode = crate::VarioMode::SpeedToFly;
        cm.control.fly_mode = crate::FlyMode::StraightFlight;
        shot(&mut cm, &std::format!("style_{name}_cruise"));
    }

    // ---- Expert setup
    let (mut cm, mut cc, mut t) = fresh(false);
    cm.sensor.airspeed = AirSpeed::from_speeds(0.0.km_h(), 0.0.km_h());
    key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEncS3);
    shot(&mut cm, "expert_setup");
    for (row, name) in [
        ("Sound", "sound"),
        ("Display", "display"),
        ("System", "system"),
    ] {
        cursor(&mut cm, row);
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        shot(&mut cm, &std::format!("expert_{name}"));
        cursor(&mut cm, "Back");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
    }
}

/// the gear alarm box (both blink phases, dark / light page, horizon), the
/// reminder after the confirmation, the ballast info
#[test]
#[ignore]
fn alarm_screens() {
    use crate::model::{SystemState, TypeOfInfo};
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let b = [
        LineView::Altitude,
        LineView::AverageClimbRate,
        LineView::WindAndDelta,
        LineView::TrueAirSpeed,
        LineView::None,
    ];
    for (core, light, cname) in [
        (PageCore::Boxes, false, "boxes"),
        (PageCore::Boxes, true, "light"),
        (PageCore::Horizon, false, "horizon"),
    ] {
        let (mut cm, mut cc, _tx) = harness_wp!();
        cm.glider_data.basic_glider_data.name = "AS 33 18m";
        let mut t = 0;
        run_ms(&mut cm, &mut cc, &mut t, 300);
        page(&mut cm, core, light, b);
        sketch_values(&mut cm);
        cm.control.system_state = SystemState::CanAndGpsOk;
        cm.config.overlay_active = OverlayActive::Info;
        cm.config.type_of_info = TypeOfInfo::GearAlarm;
        cm.control.gear_warning = true;
        cm.control.alive_ticks = 1000; // bright phase
        dump_ppm(
            &render(&mut cm),
            &std::format!("{dir}/alarm_gear_{cname}_on.ppm"),
        );
        cm.control.alive_ticks = 1005; // dark phase
        dump_ppm(
            &render(&mut cm),
            &std::format!("{dir}/alarm_gear_{cname}_off.ppm"),
        );
        // confirmed: the page with the reminder
        cm.config.overlay_active = OverlayActive::None;
        cm.config.type_of_info = TypeOfInfo::None;
        cm.control.gear_alarm_ack = true;
        dump_ppm(
            &render(&mut cm),
            &std::format!("{dir}/alarm_gear_{cname}_confirmed.ppm"),
        );
        cm.control.gear_warning = false;
        cm.control.gear_alarm_ack = false;
        cm.config.overlay_active = OverlayActive::Info;
        cm.config.type_of_info = TypeOfInfo::WaterBallast;
        dump_ppm(
            &render(&mut cm),
            &std::format!("{dir}/info_ballast_{cname}.ppm"),
        );
    }
}

/// The POT box at five moments of the real AS 33 piece of tests/pot_climb.rs
/// (193 s) - for the explanation of the box. The numbers are the output of
/// PotClimb at 510 kg over a replay of the raw log with the recommended
/// configuration of the box and a corrected pitot.
#[test]
#[ignore]
fn pot_example_screens() {
    use crate::model::{PotVerdict, SystemState};
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    // name, MC, vario, GNSS altitude, TAS km/h, POT, estimate?, verdict, circling, 20 s average
    let moments: [(&str, f32, f32, f32, f32, f32, bool, PotVerdict, bool, f32); 5] = [
        (
            "a_straight",
            2.0,
            5.853,
            2206.0,
            137.3,
            3.1948,
            true,
            PotVerdict::None,
            false,
            2.176,
        ),
        (
            "b_rollin_mc2",
            2.0,
            5.264,
            2248.0,
            139.6,
            2.6464,
            true,
            PotVerdict::Yes,
            true,
            3.521,
        ),
        (
            "c_rollin_mc4",
            4.0,
            5.264,
            2248.0,
            139.6,
            2.6464,
            true,
            PotVerdict::No,
            true,
            3.521,
        ),
        (
            "d_measured",
            2.0,
            1.085,
            2406.0,
            132.2,
            3.3137,
            false,
            PotVerdict::Yes,
            true,
            2.703,
        ),
        (
            "e_hold",
            2.0,
            3.505,
            2551.0,
            142.8,
            3.1321,
            false,
            PotVerdict::None,
            false,
            2.765,
        ),
    ];
    for (name, mc, vario, alt, tas, pot, estimate, verdict, circling, avg) in moments {
        let (mut cm, mut cc, _tx) = harness_wp!();
        cm.glider_data.basic_glider_data.name = "AS 33 18m";
        let mut t = 0;
        run_ms(&mut cm, &mut cc, &mut t, 300);
        page(
            &mut cm,
            PageCore::Boxes,
            false,
            [
                LineView::Altitude,
                LineView::PotentialClimb,
                LineView::AverageClimbRate,
                LineView::TrueAirSpeed,
                LineView::None,
            ],
        );
        sketch_values(&mut cm);
        cm.control.system_state = SystemState::CanAndGpsOk;
        let p = 101_325.0 * (1.0 - 2.25577e-5 * alt).powf(5.25588);
        cm.sensor.pressure = p.pa();
        cm.sensor.pressure_altitude.set_static_pressure(p.pa());
        cm.sensor.airspeed = AirSpeed::from_speeds((tas / 1.11).km_h(), tas.km_h());
        cm.sensor.climb_rate = vario.m_s();
        cm.calculated.needle_correction = 0.0.m_s();
        cm.calculated.interpolated_climb_rate.value = vario.clamp(-5.0, 5.0).m_s();
        cm.calculated.interpolated_climb_rate.shown = vario.clamp(-5.0, 5.0).m_s();
        cm.calculated.gust_filtered_climb = vario.clamp(-5.0, 5.0).m_s();
        // the wind is not part of the fixture: the centre shows nothing
        let i = cm.config.page_idx as usize;
        cm.config.pages[i].center_circling = crate::CenterView::None;
        cm.config.pages[i].center_straight = crate::CenterView::None;
        cm.calculated.av2_climb_rate = avg.m_s();
        cm.sensor.average_climb_rate = avg.m_s();
        cm.config.mc_cready = mc.m_s();
        cm.calculated.pot_climb = Some(pot.m_s());
        cm.calculated.pot_predictive = estimate;
        cm.calculated.pot_verdict = verdict;
        cm.control.vario_mode = crate::VarioMode::Vario;
        cm.control.fly_mode = if circling {
            crate::FlyMode::Circling
        } else {
            crate::FlyMode::StraightFlight
        };
        dump_ppm(&render(&mut cm), &std::format!("{dir}/pot_{name}.ppm"));
    }
}

/// The centering page (core "Centering aid"): the large centering aid while
/// circling with the two boxes, the info boxes view in straight flight, and
/// the page in the layout editor.
/// `RENDER_DIR=... WALLPAPER_DIR=... cargo test --lib centering_page_screens -- --ignored`
#[test]
#[ignore]
fn centering_page_screens() {
    use crate::{utils::KeyEvent, CenterView, Event, FlyMode, VarioMode};
    use embedded_graphics::geometry::AngleUnit;
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let circling = [
        LineView::AverageClimbRate,
        LineView::WindAndAvgWind,
        LineView::None,
        LineView::None,
        LineView::None,
    ];
    let cruise = [
        LineView::Altitude,
        LineView::WindAndAvgWind,
        LineView::AverageClimbRate,
        LineView::SpeedToFly,
        LineView::None,
    ];
    // a model on the centering page, `ticks` of a circle flown
    let scene = |glider: &'static str, centre: CenterView, top: LineView, ticks: usize| {
        let (mut cm, mut cc, _tx) = harness_wp!();
        cm.glider_data.basic_glider_data.name = glider;
        let mut t = 0;
        run_ms(&mut cm, &mut cc, &mut t, 300);
        let mut boxes = circling;
        boxes[0] = top;
        page(&mut cm, PageCore::ThermalAssist, false, boxes);
        let i = cm.config.page_idx as usize;
        cm.config.pages[i].stf_boxes = cruise;
        cm.config.pages[i].cruise_boxes = true;
        cm.config.pages[i].center_circling = centre;
        cm.config.pages[i].center_straight = CenterView::DoubleArrowStraight;
        sketch_values(&mut cm);
        cm.sensor.average_wind.set_speed(22.0.km_h());
        cm.sensor.average_wind.set_angle(265.0f32.deg());
        cm.calculated.thermal_climb_rate = 2.1.m_s();
        cm.control.vario_mode = VarioMode::Vario;
        cm.control.fly_mode = FlyMode::Circling;
        cm.control.circling_direction = crate::model::CirclingDirection::Left;
        cm.calculated.av2_climb_rate = 1.5.m_s();
        cm.sensor.average_climb_rate = 1.5.m_s();
        cm.sensor.euler_roll = 40.0f32.deg();
        cm.calculated.flap_rec = Some(5);
        cm.calculated.flap_actual = Some(4.6);
        let mut view = CoreView::new(TestDisplay::new(), &cm);
        let mut yaw = 90.0f32;
        for _ in 0..ticks {
            let rel = (yaw - 30.0).to_radians();
            let climb = 1.5 + 1.6 * rel.cos()
                - 1.2 * (-((yaw - 130.0).rem_euclid(360.0) - 180.0).abs() / 12.0).exp();
            cm.sensor.euler_yaw = yaw.rem_euclid(360.0).deg();
            cm.sensor.climb_rate = climb.m_s();
            cm.calculated.gust_filtered_climb = climb.m_s();
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            view.prepare(&mut cm);
            view.draw().unwrap();
            yaw += 3.0;
        }
        (cm, cc, view)
    };
    // 200 ticks of 3 degrees: more than the 1.5 circles the track view takes
    // on the large page; "ring_part" (half a circle) shows that track
    for (name, glider, centre, top, ticks) in [
        (
            "ring",
            "LS 8",
            CenterView::ThermalAssistant3,
            LineView::AverageClimbRate,
            200,
        ),
        (
            "ring_part",
            "LS 8",
            CenterView::ThermalAssistant3,
            LineView::AverageClimbRate,
            60,
        ),
        (
            "ring_thermal",
            "LS 8",
            CenterView::ThermalAssistant3,
            LineView::ThermalClimb,
            200,
        ),
        (
            "ring_tape",
            "AS 33 18m",
            CenterView::ThermalAssistant3,
            LineView::AverageClimbRate,
            200,
        ),
        (
            "spider",
            "LS 8",
            CenterView::ThermalAssistant2,
            LineView::AverageClimbRate,
            200,
        ),
        (
            "spider_tape",
            "AS 33 18m",
            CenterView::ThermalAssistant2,
            LineView::AverageClimbRate,
            200,
        ),
        (
            "dots",
            "LS 8",
            CenterView::ThermalAssistant1,
            LineView::AverageClimbRate,
            200,
        ),
        // a page of an older version: the centre of the page was a wind arrow
        (
            "old_page",
            "LS 8",
            CenterView::SingleArrowCircling,
            LineView::AverageClimbRate,
            200,
        ),
    ] {
        let (_cm, _cc, view) = scene(glider, centre, top, ticks);
        dump_ppm(&view.display, &std::format!("{dir}/cpage_{name}.ppm"));
    }
    // straight flight: the info boxes view with the cruise set
    for (name, glider, vario) in [
        ("cruise", "LS 8", false),
        ("cruise_tape", "AS 33 18m", false),
        ("straight_vario", "LS 8", true),
    ] {
        let (mut cm, _cc, mut view) = scene(
            glider,
            CenterView::ThermalAssistant3,
            LineView::AverageClimbRate,
            200,
        );
        cm.control.fly_mode = FlyMode::StraightFlight;
        cm.control.vario_mode = if vario {
            VarioMode::Vario
        } else {
            VarioMode::SpeedToFly
        };
        cm.sensor.euler_roll = 0.0f32.deg();
        cm.calculated.gust_filtered_climb = (-0.8).m_s();
        cm.sensor.climb_rate = (-0.8).m_s();
        for _ in 0..30 {
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            view.prepare(&mut cm);
            view.draw().unwrap();
        }
        dump_ppm(&view.display, &std::format!("{dir}/cpage_{name}.ppm"));
    }
    // the layout editor on the centering page
    const ZONE: KeyEvent = KeyEvent::Rotary1Right;
    const VAL: KeyEvent = KeyEvent::Rotary2Right;
    let (mut cm, mut cc, mut view) = scene(
        "LS 8",
        CenterView::ThermalAssistant3,
        LineView::AverageClimbRate,
        200,
    );
    crate::controller::page_layout::start(&mut cm);
    let mut t = 0;
    let mut go = |cm: &mut CoreModel,
                  cc: &mut CoreController,
                  view: &mut CoreView<TestDisplay>,
                  k: Option<KeyEvent>,
                  name: Option<&str>| {
        if let Some(k) = k {
            cc.event_handler(Event::KeyItem(k), cm);
        }
        run_ms(cm, cc, &mut t, 20);
        cm.control.alive_ticks = 1000;
        view.prepare(cm);
        view.draw().unwrap();
        if let Some(name) = name {
            dump_ppm(&view.display, &std::format!("{dir}/cpage_edit_{name}.ppm"));
        }
    };
    go(&mut cm, &mut cc, &mut view, None, Some("1_scale"));
    go(&mut cm, &mut cc, &mut view, Some(ZONE), Some("2_core"));
    go(&mut cm, &mut cc, &mut view, Some(VAL), Some("2_core_list"));
    go(&mut cm, &mut cc, &mut view, Some(KeyEvent::BtnEsc), None);
    go(&mut cm, &mut cc, &mut view, Some(ZONE), Some("3_mode"));
    go(&mut cm, &mut cc, &mut view, Some(ZONE), Some("4_centre"));
    go(
        &mut cm,
        &mut cc,
        &mut view,
        Some(VAL),
        Some("4_centre_list"),
    );
    go(&mut cm, &mut cc, &mut view, Some(KeyEvent::BtnEsc), None);
    go(&mut cm, &mut cc, &mut view, Some(ZONE), Some("5_box_top"));
    go(
        &mut cm,
        &mut cc,
        &mut view,
        Some(ZONE),
        Some("6_box_bottom"),
    );
    go(
        &mut cm,
        &mut cc,
        &mut view,
        Some(VAL),
        Some("6_box_bottom_list"),
    );
    go(&mut cm, &mut cc, &mut view, Some(KeyEvent::BtnEsc), None);
    go(
        &mut cm,
        &mut cc,
        &mut view,
        Some(ZONE),
        Some("7_after_boxes"),
    );
    // back to the flight mode zone, the cruise set
    for _ in 0..4 {
        go(
            &mut cm,
            &mut cc,
            &mut view,
            Some(KeyEvent::Rotary1Left),
            None,
        );
    }
    // the first click opens the list and moves to "Cruise"
    go(&mut cm, &mut cc, &mut view, Some(VAL), Some("8_mode_list"));
    go(
        &mut cm,
        &mut cc,
        &mut view,
        Some(KeyEvent::BtnEnc),
        Some("9_mode_cruise"),
    );
    go(
        &mut cm,
        &mut cc,
        &mut view,
        Some(ZONE),
        Some("9_cruise_centre"),
    );
    go(&mut cm, &mut cc, &mut view, Some(ZONE), None);
    go(&mut cm, &mut cc, &mut view, Some(ZONE), None);
    go(
        &mut cm,
        &mut cc,
        &mut view,
        Some(ZONE),
        Some("9_cruise_box_right1"),
    );
}

/// The track view of the centering page on a simulated entry: 12 s
/// straight, then a circle of 20 s with the best climb at a heading of 40
/// degrees. `cpage_track_entry`: half a circle flown, `cpage_track`: one
/// and a quarter, `cpage_track_after`: after 1.5 circles the aid of the
/// page (3D ring).
#[test]
#[ignore]
fn centering_track_screens() {
    use crate::{CenterView, FlyMode, VarioMode};
    use embedded_graphics::geometry::AngleUnit;
    let dir = match std::env::var("RENDER_DIR") {
        Ok(d) => d,
        Err(_) => return,
    };
    let (mut cm, mut cc, _tx) = harness_wp!();
    let mut t = 0;
    run_ms(&mut cm, &mut cc, &mut t, 300);
    page(
        &mut cm,
        PageCore::ThermalAssist,
        false,
        [
            LineView::AverageClimbRate,
            LineView::WindAndAvgWind,
            LineView::None,
            LineView::None,
            LineView::None,
        ],
    );
    let i = cm.config.page_idx as usize;
    cm.config.pages[i].center_circling = CenterView::ThermalAssistant3;
    sketch_values(&mut cm);
    cm.sensor.average_wind.set_speed(22.0.km_h());
    cm.sensor.average_wind.set_angle(265.0f32.deg());
    cm.sensor.average_climb_rate = 1.5.m_s();
    cm.control.vario_mode = VarioMode::Vario;
    cm.control.fly_mode = FlyMode::StraightFlight;
    cm.control.circling_direction = crate::model::CirclingDirection::Left;
    let mut view = CoreView::new(TestDisplay::new(), &cm);
    let mut yaw = 300.0f32;
    let mut fly =
        |cm: &mut CoreModel, view: &mut CoreView<TestDisplay>, seconds: f32, rate: f32| {
            for _ in 0..(seconds * 10.0 + 0.5) as u32 {
                yaw += rate * 0.1;
                let climb = if rate == 0.0 {
                    0.4
                } else {
                    2.0 + 1.5 * (yaw - 40.0).to_radians().cos()
                };
                cm.sensor.euler_yaw = yaw.rem_euclid(360.0).deg();
                cm.sensor.euler_roll = if rate == 0.0 {
                    0.0f32.deg()
                } else {
                    40.0f32.deg()
                };
                cm.sensor.turn_rate = crate::FloatToAngularVelocity::rad_s(rate.to_radians());
                cm.sensor.airspeed = crate::AirSpeed::new(27.0, 29.0);
                cm.sensor.climb_rate = climb.m_s();
                cm.calculated.gust_filtered_climb = climb.m_s();
                cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
                view.prepare(cm);
                view.draw().unwrap();
            }
        };
    fly(&mut cm, &mut view, 12.0, 0.0);
    fly(&mut cm, &mut view, 7.0, 18.0);
    cm.control.fly_mode = FlyMode::Circling;
    fly(&mut cm, &mut view, 3.0, 18.0);
    dump_ppm(&view.display, &std::format!("{dir}/cpage_track_entry.ppm"));
    fly(&mut cm, &mut view, 15.0, 18.0);
    dump_ppm(&view.display, &std::format!("{dir}/cpage_track.ppm"));
    fly(&mut cm, &mut view, 7.0, 18.0);
    dump_ppm(&view.display, &std::format!("{dir}/cpage_track_after.ppm"));
    // a page with the track as its aid keeps it
    cm.config.pages[i].center_circling = CenterView::ThermalAssistant4;
    fly(&mut cm, &mut view, 5.0, 18.0);
    dump_ppm(&view.display, &std::format!("{dir}/cpage_track_stays.ppm"));
}
