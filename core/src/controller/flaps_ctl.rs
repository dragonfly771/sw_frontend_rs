//! Flap recommendation, actual flap position and the flight demo
//! (plan_klappen_menue.md, model/flaps.rs).

use crate::{
    model::{
        demo::{DemoMode, DEMO_OFF_KMH, DEMO_ON_MAX_KMH},
        flaps::FlapTable,
        FlyMode,
    },
    AirSpeed, CoreModel, FloatToAngularVelocity, FloatToSpeed,
};
#[allow(unused_imports)]
use crate::{utils::FClamp, FloatToLength, FloatToPressure};
use embedded_graphics::prelude::AngleUnit;
#[allow(unused_imports)]
use micromath::F32Ext;

impl CoreModel {
    /// The flap table in effect: the configured one, otherwise the AS 33
    /// placeholder while an AS 33 polar is selected or the flight demo runs.
    pub fn flap_table(&self) -> FlapTable {
        if !self.config.flaps.is_empty() {
            self.config.flaps
        } else if self.glider_data.basic_glider_data.name.starts_with("AS 33")
            || self.control.demo.flight()
        {
            FlapTable::as33_placeholder()
        } else {
            FlapTable::default()
        }
    }

    /// total mass / reference weight of the polar
    pub fn mass_ratio(&self) -> f32 {
        let bgd = &self.glider_data.basic_glider_data;
        let mass = bgd.empty_mass
            + self.glider_data.pilot_weight.to_kg()
            + self.glider_data.water_ballast.to_kg();
        if bgd.reference_weight > 10.0 && mass > 10.0 {
            mass / bgd.reference_weight
        } else {
            1.0
        }
    }
}

/// Switch the demo on or off. On only at a standstill.
/// Returns the mode that is set in the end.
pub fn set_demo(cm: &mut CoreModel, mode: DemoMode) -> DemoMode {
    if mode.active() && !cm.control.demo.active() {
        let ias = cm.sensor.airspeed.ias().to_km_h();
        if ias >= DEMO_ON_MAX_KMH {
            return cm.control.demo;
        }
        cm.control.demo_real_ias_kmh = ias;
        cm.control.flap_demo = crate::model::flaps::FlapDemo::new();
        if mode.flight() {
            // realistic pressure values: QNH of the demo, restored at the end
            cm.control.flap_demo.qnh_before_hpa = cm.sensor.pressure_altitude.qnh().to_hpa();
            cm.sensor
                .pressure_altitude
                .set_qnh(crate::model::flaps::DEMO_QNH_HPA.hpa());
            // the energy arrow (average - live wind) shows in the demo even
            // when it is switched off in the settings
            cm.control.flap_demo.energy_mult_before = cm.control.energy_arrow_mult;
            if cm.control.energy_arrow_mult <= 0.0 {
                cm.control.energy_arrow_mult = crate::model::flaps::DEMO_ENERGY_MULT;
            }
        }
        // start the lever where the first recommendation will be
        let t = FlapTable::as33_placeholder();
        if let Some(r) = t.recommend(95.0, 1.0, false, None) {
            cm.control.flap_demo.set_lever(r);
        }
    }
    if !mode.flight() && cm.control.demo.flight() {
        let q = cm.control.flap_demo.qnh_before_hpa;
        if q > 800.0 {
            cm.sensor.pressure_altitude.set_qnh(q.hpa());
        }
        cm.control.energy_arrow_mult = cm.control.flap_demo.energy_mult_before;
        // leave the simulated flight values behind
        cm.sensor.airspeed = AirSpeed::from_speeds(0.0.km_h(), 0.0.km_h());
        cm.sensor.climb_rate = 0.0.m_s();
        cm.sensor.average_climb_rate = 0.0.m_s();
        cm.sensor.turn_rate = 0.0.rad_s();
    }
    cm.control.demo = mode;
    mode
}

/// Called every 100 ms BEFORE the normal processing: feeds the simulated
/// flight values into the model, as if they came from the sensor box
/// (the CAN reader drops the real ones while the flight demo runs).
pub fn demo_tick_100ms(cm: &mut CoreModel) {
    if !cm.control.demo.active() {
        return;
    }
    if !cm.control.demo.flight() {
        // FLARM demo only: the airspeed in the model is the real one
        cm.control.demo_real_ias_kmh = cm.sensor.airspeed.ias().to_km_h();
    }
    // safety: real flight ends the demo
    if cm.control.demo_real_ias_kmh > DEMO_OFF_KMH {
        set_demo(cm, DemoMode::Off);
        return;
    }
    if !cm.control.demo.flight() {
        return;
    }
    let f = cm.control.flap_demo.frame();
    // altitude from the simulated climb, static pressure for it at the QNH
    // of the demo (qnh_altitude = altitude(p - (qnh - p_nn))), so ALT and
    // FL read like in flight
    cm.control.flap_demo.climb(100, f.climb_ms);
    let alt = cm.control.flap_demo.alt_m;
    let qnh = cm.sensor.pressure_altitude.qnh();
    let p = crate::flight_physics::pressure(alt.m()) + (qnh - crate::Pressure::AT_NN());
    cm.sensor.pressure = p;
    cm.sensor.pressure_altitude.set_static_pressure(p);
    let rho = crate::flight_physics::density(alt.m());
    cm.sensor.density = rho;
    // TAS from the density at that altitude
    let sigma = (rho.to_kg_m3() / crate::Density::AT_NN().to_kg_m3()).fclamp(0.3, 1.2);
    let ias = f.ias_kmh.km_h();
    cm.sensor.airspeed = AirSpeed::from_speeds(ias, (f.ias_kmh / sigma.sqrt()).km_h());
    // bank angle: 40 deg in the circle (ramps in over 3 s, out over 2 s),
    // nearly level in cruise - the gust filter and the horizon see a real turn
    let tl = cm.control.flap_demo.t_ms % crate::model::flaps::DEMO_LOOP_MS;
    let cruise_ms = crate::model::flaps::DEMO_CRUISE_MS;
    let bank = if f.circling {
        40.0 * ((tl - cruise_ms) as f32 / 3_000.0).min(1.0)
    } else if tl < 2_000 {
        40.0 * (1.0 - tl as f32 / 2_000.0)
    } else {
        0.0
    };
    cm.sensor.euler_roll = bank.deg(); // right turn (turn rate > 0): right wing down
    cm.sensor.euler_pitch = (if f.circling {
        3.0
    } else {
        1.0 - (f.ias_kmh - 95.0) * 0.04
    })
    .deg();
    // live wind swinging around the average wind -> energy arrow
    let (dir, spd, adir, aspd) = crate::model::flaps::demo_wind(cm.control.flap_demo.t_ms);
    cm.sensor.wind_vector.set_angle(dir.deg());
    cm.sensor.wind_vector.set_speed(spd.km_h());
    cm.sensor.average_wind.set_angle(adir.deg());
    cm.sensor.average_wind.set_speed(aspd.km_h());
    cm.sensor.climb_rate = f.climb_ms.m_s();
    cm.sensor.average_climb_rate = f.avg_ms.m_s();
    cm.sensor.turn_rate = f.turn_rate_dps.to_radians().rad_s();
    if f.circling {
        // turn the heading so the wind arrow and the centering aid move
        let yaw = cm.sensor.euler_yaw.to_radians() + f.turn_rate_dps.to_radians() * 0.1;
        cm.sensor.euler_yaw = crate::into_range_0_360(crate::Angle::from_radians(yaw));
    }
}

/// Called every 100 ms AFTER the sensor values are in: recommendation and
/// actual position for the flap tape
pub fn flaps_tick_100ms(cm: &mut CoreModel) {
    let table = cm.flap_table();
    if table.is_empty() {
        cm.calculated.flap_rec = None;
        cm.calculated.flap_actual = None;
        return;
    }
    let ias = cm.sensor.airspeed.ias().to_km_h();
    let circling = cm.control.fly_mode == FlyMode::Circling;
    let prev = cm.calculated.flap_rec.map(|r| r as usize);
    let rec = table.recommend(ias, cm.mass_ratio(), circling, prev);
    cm.calculated.flap_rec = rec.map(|r| r as u8);

    cm.calculated.flap_actual = if cm.control.demo.flight() {
        cm.control.flap_demo.step(100, rec);
        Some(cm.control.flap_demo.lever)
    } else {
        match table.sensor {
            crate::model::flaps::FlapSensorSource::Larus => {
                table.position_index(cm.sensor.flap_sensor_deg)
            }
            crate::model::flaps::FlapSensorSource::None => None,
        }
    };
}

// ------------------------------------------------------------ persistence
//
// A table edit can touch up to 12 stages at once (delete, template). The
// EEPROM write buffer (`pers_vals`) holds 8 items only, so changed stages
// are only marked here and written a few per second from `tick_1s`
// (`flush_flap_dirty`).

/// bit of `CoreController::flap_dirty`: table word (count, circling, sensor)
pub const DIRTY_TABLE: u32 = 1 << 31;

/// stage i (name + up-to speed and position) has to be written
pub fn mark_stage(cc: &mut crate::CoreController, i: usize) {
    if i < crate::model::flaps::MAX_FLAPS {
        cc.flap_dirty |= 1 << i;
    }
}

/// The pilot edits the table while the AS 33 placeholder is in effect: the
/// placeholder becomes his own table first (all of it is written).
pub fn adopt_template(cm: &mut CoreModel, cc: &mut crate::CoreController) {
    if cm.config.flaps.is_empty() {
        let t = cm.flap_table();
        if !t.is_empty() {
            cm.config.flaps = t;
            cc.flap_dirty |= DIRTY_TABLE;
            for i in 0..t.len() {
                mark_stage(cc, i);
            }
        }
    }
}

/// live value of the flap sensor for the menu
pub fn sensor_str(cm: &CoreModel) -> crate::utils::TString<16> {
    let v = cm.sensor.flap_sensor_deg;
    if v.is_nan() {
        crate::utils::TString::<16>::from_str("no sensor")
    } else {
        match crate::tformat!(16, "{:.1}°", v) {
            Ok(s) => crate::utils::TString::<16>::from_str(s.as_str()),
            Err(_) => crate::utils::TString::<16>::from_str("-"),
        }
    }
}

/// Write marked flap items, as many as fit into the EEPROM write buffer
/// (called once per second)
pub fn flush_flap_dirty(cm: &mut CoreModel, cc: &mut crate::CoreController) {
    use crate::{
        controller::MAX_PERS_IDS, persist::persist_set, utils::Variant, Echo, PersistenceId,
    };
    const STAGE_IDS: [PersistenceId; 12] = [
        PersistenceId::FlapStage0,
        PersistenceId::FlapStage1,
        PersistenceId::FlapStage2,
        PersistenceId::FlapStage3,
        PersistenceId::FlapStage4,
        PersistenceId::FlapStage5,
        PersistenceId::FlapStage6,
        PersistenceId::FlapStage7,
        PersistenceId::FlapStage8,
        PersistenceId::FlapStage9,
        PersistenceId::FlapStage10,
        PersistenceId::FlapStage11,
    ];
    const POS_IDS: [PersistenceId; 12] = [
        PersistenceId::FlapPos0,
        PersistenceId::FlapPos1,
        PersistenceId::FlapPos2,
        PersistenceId::FlapPos3,
        PersistenceId::FlapPos4,
        PersistenceId::FlapPos5,
        PersistenceId::FlapPos6,
        PersistenceId::FlapPos7,
        PersistenceId::FlapPos8,
        PersistenceId::FlapPos9,
        PersistenceId::FlapPos10,
        PersistenceId::FlapPos11,
    ];
    if cc.flap_dirty == 0 {
        return;
    }
    // stages first, the table word (count) last: after a power loss in
    // between, the old count still matches written stages. At most two
    // stages per call and only into a half empty buffer, so a setting the
    // pilot changes meanwhile always finds room (the buffer drops items
    // silently when it is full).
    const ROOM: usize = MAX_PERS_IDS / 2;
    let mut written = 0;
    for i in 0..12 {
        if cc.flap_dirty & (1 << i) == 0 {
            continue;
        }
        if written >= 2 || cc.pers_vals.len() + 2 > ROOM {
            return;
        }
        written += 1;
        let st = cm.config.flaps.stages[i];
        persist_set(cc, cm, Variant::U32(st.word()), STAGE_IDS[i], Echo::None);
        persist_set(cc, cm, Variant::F32(st.position), POS_IDS[i], Echo::None);
        cc.flap_dirty &= !(1 << i);
    }
    if cc.flap_dirty & DIRTY_TABLE != 0 && cc.pers_vals.len() < ROOM {
        let w = cm.config.flaps.word();
        persist_set(
            cc,
            cm,
            Variant::U32(w),
            PersistenceId::FlapTable,
            Echo::None,
        );
        cc.flap_dirty &= !DIRTY_TABLE;
    }
}

#[cfg(test)]
mod no_box_tests {
    //! The message NO SENSOR BOX - check CAN cable can be acknowledged
    //! (useful when testing with the simulation only).
    extern crate std;
    use super::*;
    use crate::model::OverlayActive;
    use crate::view::vario_ring::no_box_banner;
    use crate::{test_harness, CoreController, Event, KeyEvent};

    fn run(cm: &mut CoreModel, cc: &mut CoreController, clock: &mut u16, ms: u32) {
        for _ in 0..ms {
            *clock = clock.wrapping_add(1);
            cc.tick_1ms(*clock, cm);
        }
    }

    #[test]
    fn a_push_takes_no_sensor_box_off_the_page_until_the_box_was_heard_again() {
        let (mut cm, mut cc, _tx) = test_harness!();
        let mut clock = 0;
        assert!(!no_box_banner(&cm), "not in the first seconds");
        run(&mut cm, &mut cc, &mut clock, 6_500);
        assert!(no_box_banner(&cm));
        // the first push confirms and opens nothing
        cc.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut cm);
        run(&mut cm, &mut cc, &mut clock, 20);
        assert!(!no_box_banner(&cm) && cm.control.no_box_ack);
        assert_eq!(cm.config.overlay_active, OverlayActive::None);
        // it stays away, and the display works as usual
        run(&mut cm, &mut cc, &mut clock, 20_000);
        assert!(!no_box_banner(&cm));
        cc.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut cm);
        run(&mut cm, &mut cc, &mut clock, 20);
        assert_eq!(cm.config.overlay_active, OverlayActive::Menu);
        crate::controller::menu::close_menu_display(&mut cm, &mut cc);
        // the box is heard, then lost again: the words are back
        cm.control.can_devices = 1;
        run(&mut cm, &mut cc, &mut clock, 1_100);
        assert!(!cm.control.no_box_ack && !no_box_banner(&cm));
        cm.control.can_devices = 0;
        run(&mut cm, &mut cc, &mut clock, 1_100);
        assert!(no_box_banner(&cm));
    }

    #[test]
    fn the_flight_demo_shows_no_sensor_box_message() {
        let (mut cm, mut cc, _tx) = test_harness!();
        let mut clock = 0;
        run(&mut cm, &mut cc, &mut clock, 6_500);
        assert!(no_box_banner(&cm));
        set_demo(&mut cm, DemoMode::Flaps);
        assert!(!no_box_banner(&cm));
        // the demo flies: after its cruise the display detects the circle
        // by itself, and the heading turns more than four times
        let mut circling_ms = 0;
        for _ in 0..120 {
            run(&mut cm, &mut cc, &mut clock, 1_000);
            if cm.control.fly_mode == FlyMode::Circling {
                circling_ms += 1_000;
            }
            assert!(!no_box_banner(&cm));
        }
        assert!(
            circling_ms > 60_000,
            "{circling_ms} ms of detected circling in one loop of the demo"
        );
        set_demo(&mut cm, DemoMode::Off);
        assert!(
            no_box_banner(&cm),
            "the demo is over: the sensor box is still missing"
        );
    }
}
