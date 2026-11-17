use embedded_graphics::draw_target::DrawTarget;

pub mod editor;
pub mod fw_update;
pub mod help;
pub(crate) mod thermal_data;
pub(crate) mod trail;

pub(crate) mod device_info;
pub(crate) mod flap_tape;
pub(crate) mod flarm_pages;
pub(crate) mod fonts_p1;
pub(crate) mod horizon;
pub(crate) mod horizon_k1;
pub(crate) mod info;
pub(crate) mod menu;
pub(crate) mod mini_horizon;
pub(crate) mod name_entry;
pub(crate) mod page_layout;
pub(crate) mod raster;
pub(crate) mod ring_editor;
pub(crate) mod ring_menu;
pub(crate) mod sprites;
pub(crate) mod vario;
pub(crate) mod vario_ring;
pub(crate) mod viewable;
pub(crate) mod volume_panel;

#[cfg(test)]
mod design_compare;
#[cfg(test)]
mod flarm_render_tests;
#[cfg(test)]
pub(crate) mod test_fixture;

use crate::{
    model::{CoreModel, DisplayActive, OverlayActive},
    utils::Colors,
    view::{
        device_info::DeviceInfo, editor::Edit, fw_update::SwUpdate, horizon::Horizon,
        info::InfoView, menu::MenuView, vario::Vario,
    },
    CoreError, DrawImage,
};

/// Horizon of the Stefly design K1 / H1 (true) or the former one with the
/// wallpaper (false)
pub(crate) const HORIZON_K1: bool = true;

// Debug build runs at 10 Hz
#[cfg(debug_assertions)]
pub const FRAME_RATE: u32 = 10;

// Release build runs at 30 Hz
#[cfg(not(debug_assertions))]
pub const FRAME_RATE: u32 = 20;

#[derive(PartialEq)]
enum PrimaryView {
    Vario(Vario),
    Horizon(Horizon),
    SwUpade(SwUpdate),
    MenuView(MenuView),
    DeviceInfo(DeviceInfo),
}

#[derive(PartialEq)]
enum SecondaryView {
    Edit(Edit),
    MenuView(MenuView),
    InfoView(InfoView),
}

pub struct CoreView<D>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    pub display: D,
    primary_view: PrimaryView,
    secondary_view: Option<SecondaryView>,
    /// Climb around the circle and the track through the air, for the
    /// centering aids. Recorded with every frame, whatever is on the
    /// display - the setup menu, the volume panel, another page (whenever the
    /// pilot opens the page, it looks as if it had been on the display all the
    /// time). Until 0.4.1.53 they belonged to the
    /// vario page: an overlay paused them, the setup menu cleared them.
    thermal_data: thermal_data::ThermalData,
    core_model: CoreModel,
    /// the snapshot the info boxes show: slow values once a second, the
    /// fast ones (`refresh_fast_values`) every FAST_TICKS
    core_model_1s: CoreModel,
    fast_tick: u32,
    last_display_active: DisplayActive,
}

/// refresh of the fast info box values in 100 ms ticks (5 Hz)
const FAST_TICKS: u32 = 2;

/// Copy the values that are read to fly into the 1 s snapshot: altitude,
/// flight level and QNH, airspeeds, attitude, g load, track, potential climb
fn refresh_fast_values(slow: &mut CoreModel, now: &CoreModel) {
    let (d, s) = (&mut slow.sensor, &now.sensor);
    d.pressure_altitude = s.pressure_altitude;
    d.pressure = s.pressure;
    d.airspeed = s.airspeed;
    d.euler_roll = s.euler_roll;
    d.euler_pitch = s.euler_pitch;
    d.euler_yaw = s.euler_yaw;
    d.nick_angle = s.nick_angle;
    d.slip_angle = s.slip_angle;
    d.turn_rate = s.turn_rate;
    d.g_force = s.g_force;
    d.vertical_g_force = s.vertical_g_force;
    d.gps_track = s.gps_track;
    d.gps_ground_speed = s.gps_ground_speed;
    d.gps_altitude = s.gps_altitude;
    slow.calculated.pot_climb = now.calculated.pot_climb;
    slow.calculated.pot_predictive = now.calculated.pot_predictive;
    slow.calculated.pot_verdict = now.calculated.pot_verdict;
}

impl<D> CoreView<D>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    pub fn new(display: D, core_model: &CoreModel) -> Self {
        let primary_view = PrimaryView::Vario(Vario::new());
        let cm = *core_model;
        CoreView {
            display,
            primary_view,
            secondary_view: None,
            thermal_data: thermal_data::ThermalData::default(),
            core_model: cm,
            core_model_1s: cm,
            fast_tick: 0,
            last_display_active: DisplayActive::Vario,
        }
    }

    /// Prepare a frame of the default frame rate (timer driven targets)
    pub fn prepare(&mut self, core_model: &mut CoreModel) {
        self.prepare_dt(core_model, 1.0 / FRAME_RATE as f32);
    }

    /// Prepare a frame that is `dt_s` seconds after the previous one - the
    /// V2 renders synchronized to the display refresh (33.5 Hz), so the
    /// needle interpolation gets the real frame period
    pub fn prepare_dt(&mut self, core_model: &mut CoreModel, dt_s: f32) {
        // interpolate climb rate
        core_model.interpolate_climb_rate_dt(dt_s);

        // the page indicator fades out on its own
        if core_model.control.page_indicator > 0 {
            core_model.control.page_indicator -= 1;
        }

        // take a snapshot
        self.core_model = *core_model;
        self.thermal_data.update(&self.core_model);

        // every second take another snapshop
        if core_model.control.alive_ticks.is_multiple_of(10)
            && core_model.control.alive_ticks != self.core_model_1s.control.alive_ticks
        {
            self.core_model_1s = *core_model;
        } else if core_model.control.alive_ticks.is_multiple_of(FAST_TICKS)
            && core_model.control.alive_ticks != self.fast_tick
        {
            // the values a pilot reads to fly (altitude, speeds, attitude)
            // five times a second; averages, wind, battery, time stay at 1 s
            // (Stefan 09.11.2026: the altitude moved only once a second)
            refresh_fast_values(&mut self.core_model_1s, core_model);
        }
        self.fast_tick = core_model.control.alive_ticks;

        // set the orientation
        self.display.set_rotation(core_model.control.rotation);

        if core_model.config.display_active != self.last_display_active {
            self.last_display_active = core_model.config.display_active;

            self.primary_view = match core_model.config.display_active {
                DisplayActive::Horizon => PrimaryView::Horizon(Horizon::new()),
                DisplayActive::DeviceInfo => PrimaryView::DeviceInfo(DeviceInfo::new()),
                DisplayActive::FirmwareUpdate => {
                    let update_state = core_model.control.firmware_update_state;
                    PrimaryView::SwUpade(SwUpdate::new(update_state))
                }
                DisplayActive::Menu => PrimaryView::MenuView(MenuView::new()),
                _ => PrimaryView::Vario(Vario::new()),
            };
        }

        self.secondary_view = match core_model.config.overlay_active {
            OverlayActive::Editor => Some(SecondaryView::Edit(Edit::new(core_model))),
            OverlayActive::Menu => Some(SecondaryView::MenuView(MenuView::new())),
            OverlayActive::Info => {
                InfoView::new(core_model.config.type_of_info).map(SecondaryView::InfoView)
            }
            OverlayActive::None => None,
        };
    }

    pub fn draw(&mut self) -> Result<(), CoreError> {
        // a FLARM warning that cannot be shown in the core of the ring page
        // (warning view "Full Screen", menu or editor open, other page)
        // takes the whole display
        if crate::view::flarm_pages::full_screen_needed(&self.core_model) {
            return crate::view::flarm_pages::draw_full_screen(&mut self.display, &self.core_model);
        }
        match &mut self.primary_view {
            PrimaryView::Vario(vario) => vario.draw(
                &mut self.display,
                &self.core_model,
                &self.core_model_1s,
                &mut self.thermal_data,
            )?,
            PrimaryView::Horizon(horizon) => horizon.draw(&mut self.display, &self.core_model)?,
            PrimaryView::DeviceInfo(device_info) => {
                device_info.draw(&mut self.display, &self.core_model, &self.core_model_1s)?
            }
            PrimaryView::MenuView(menu_view) => {
                menu_view.draw(&mut self.display, &self.core_model, false)?
            }
            PrimaryView::SwUpade(sw_update) => {
                sw_update.draw(&mut self.display, &self.core_model)?
            }
        }

        // page indicator: shown for a moment after a page change
        if self.core_model.control.page_indicator > 0 {
            crate::view::ring_menu::draw_page_indicator(&mut self.display, &self.core_model)?;
        }

        // page layout editor: HUD in the centre, choice lists
        crate::view::page_layout::draw(&mut self.display, &self.core_model)?;
        // volume panel (circling / cruise / warnings)
        crate::view::volume_panel::draw(&mut self.display, &self.core_model)?;

        if let Some(secondary_view) = &mut self.secondary_view {
            match secondary_view {
                SecondaryView::Edit(edit) => edit.draw(&mut self.display, &self.core_model)?,
                SecondaryView::MenuView(menu) => {
                    menu.draw(&mut self.display, &self.core_model, true)?
                }
                SecondaryView::InfoView(info_view) => {
                    if self.core_model.config.is_base_display() {
                        info_view.draw(&mut self.display, &self.core_model)?
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod snapshot_tests {
    //! Stefan 09.11.2026: the altitude box moved only once a second. The
    //! values read to fly follow five times a second, the battery voltage
    //! (and the other slow values) still once a second.
    use super::*;
    use crate::view::test_fixture::{test_model, TestDisplay};

    #[test]
    fn altitude_five_times_a_second_battery_once() {
        let mut cm = test_model();
        let mut view = CoreView::new(TestDisplay::new(), &cm);
        let mut alt_changes = 0;
        let mut bat_changes = 0;
        let mut alt = view
            .core_model_1s
            .sensor
            .pressure_altitude
            .qnh_altitude()
            .to_m();
        let mut bat = view.core_model_1s.device.supply_voltage;
        // 3 s of 100 ms ticks, three frames per tick
        for tick in 1..=30u32 {
            cm.control.alive_ticks = tick;
            let p = 95_000.0 - tick as f32 * 2.0;
            cm.sensor
                .pressure_altitude
                .set_static_pressure(crate::system_of_units::FloatToPressure::pa(p));
            cm.device.supply_voltage = 12.0 + tick as f32 * 0.01;
            for _ in 0..3 {
                view.prepare(&mut cm);
                let a = view
                    .core_model_1s
                    .sensor
                    .pressure_altitude
                    .qnh_altitude()
                    .to_m();
                let b = view.core_model_1s.device.supply_voltage;
                if a != alt {
                    alt_changes += 1;
                    alt = a;
                }
                if b != bat {
                    bat_changes += 1;
                    bat = b;
                }
            }
        }
        assert_eq!(bat_changes, 3, "battery once a second");
        assert_eq!(alt_changes, 15, "altitude five times a second");
    }
}

#[cfg(test)]
mod recording_tests {
    //! Stefan 16.11.2026: the track must not be lost while the volume is
    //! set or the menu is open - the centering page looks as if it had been
    //! on the display all the time.
    use super::*;
    use crate::view::test_fixture::{test_model, TestDisplay};
    use crate::{AirSpeed, FloatToAngularVelocity, FloatToSpeed, FlyMode};
    use embedded_graphics::geometry::AngleUnit;

    fn fly(
        view: &mut CoreView<TestDisplay>,
        cm: &mut CoreModel,
        yaw: &mut f32,
        seconds: f32,
        rate: f32,
    ) {
        for _ in 0..(seconds * 10.0 + 0.5) as u32 {
            *yaw += rate * 0.1;
            cm.control.alive_ticks += 1;
            cm.sensor.euler_yaw = yaw.rem_euclid(360.0).deg();
            cm.sensor.turn_rate = rate.to_radians().rad_s();
            cm.sensor.airspeed = AirSpeed::new(28.0, 30.0);
            cm.sensor.climb_rate = (2.0 + yaw.to_radians().cos()).m_s();
            view.prepare(cm);
            view.draw().unwrap();
        }
    }

    #[test]
    fn menu_overlay_and_other_views_do_not_stop_the_recording() {
        let run = |interrupt: fn(&mut CoreModel)| {
            let mut cm = test_model();
            cm.control.alive_ticks = 1000;
            cm.control.fly_mode = FlyMode::StraightFlight;
            let mut view = CoreView::new(TestDisplay::new(), &cm);
            let mut yaw = 0.0_f32;
            fly(&mut view, &mut cm, &mut yaw, 15.0, 0.0);
            fly(&mut view, &mut cm, &mut yaw, 7.0, 18.0);
            cm.control.fly_mode = FlyMode::Circling;
            fly(&mut view, &mut cm, &mut yaw, 4.0, 18.0);
            // 10 s with something else on the display
            let keep = (cm.config.display_active, cm.config.overlay_active);
            interrupt(&mut cm);
            fly(&mut view, &mut cm, &mut yaw, 10.0, 18.0);
            cm.config.display_active = keep.0;
            cm.config.overlay_active = keep.1;
            fly(&mut view, &mut cm, &mut yaw, 0.2, 18.0);
            (
                view.thermal_data.trail.turned().to_degrees(),
                view.thermal_data.trail.shown_len(),
                (0..crate::view::thermal_data::THERMAL_DATA_CNT)
                    .filter(|i| view.thermal_data.visited(*i))
                    .count(),
            )
        };
        let undisturbed = run(|_| ());
        // 21.2 s of turning at 18 degrees per second, 5 s of approach
        assert!(
            (undisturbed.0 - 381.6).abs() < 20.0,
            "{} degrees",
            undisturbed.0
        );
        assert!(
            (25..=27).contains(&undisturbed.1),
            "{} points",
            undisturbed.1
        );
        assert_eq!(undisturbed.2, 24, "every sector of the circle flown");
        assert_eq!(
            run(|cm| cm.config.overlay_active = OverlayActive::Menu),
            undisturbed,
            "flight menu"
        );
        assert_eq!(
            run(|cm| cm.config.overlay_active = OverlayActive::Editor),
            undisturbed,
            "editor, volume panel"
        );
        assert_eq!(
            run(|cm| cm.config.display_active = DisplayActive::Menu),
            undisturbed,
            "setup menu"
        );
        assert_eq!(
            run(|cm| cm.config.display_active = DisplayActive::Horizon),
            undisturbed,
            "horizon"
        );
        assert_eq!(
            run(|cm| cm.config.display_active = DisplayActive::DeviceInfo),
            undisturbed,
            "device info"
        );
    }
}

#[cfg(test)]
mod size_tests {
    //! The start of the device (RTIC init and driver::init::hw_init of
    //! device/larus_frontend_v2) keeps several copies of the controller, of
    //! the view and of the model on the stack: in the image of 0.4.1.53 the
    //! two frames were 89,776 + 36,600 bytes, in 128 KB of RAM that also hold
    //! 38 KB of statics - the display stayed black (16.11.2026). 0.4.1.49
    //! needed 78,880 + 33,392 bytes and ran. Every byte added to one of
    //! these three types costs two to four bytes of stack at the start.
    //!
    //! The limits are sizes on the host (64 bit), a little above the present
    //! ones. A test that fails here means: look at the frames of the built
    //! image before it is flashed, do not just raise the limit.
    use super::*;
    use crate::view::test_fixture::TestDisplay;
    use crate::CoreController;
    use core::mem::size_of;

    #[test]
    fn controller_view_and_model_stay_small() {
        let view = size_of::<CoreView<TestDisplay>>() - size_of::<TestDisplay>();
        let (controller, model) = (size_of::<CoreController>(), size_of::<CoreModel>());
        assert!(
            controller <= LIMIT_CONTROLLER,
            "CoreController has {controller} bytes"
        );
        assert!(view <= LIMIT_VIEW, "CoreView has {view} bytes");
        assert!(model <= LIMIT_MODEL, "CoreModel has {model} bytes");
    }

    /// 16.11.2026: 9,232 (with the feature "flarm" 20,208 - its target table.
    /// An image with FLARM must not be built before the start of the device
    /// no longer copies the controller on the stack.)
    const LIMIT_CONTROLLER: usize = if cfg!(feature = "flarm") {
        21_000
    } else {
        10_000
    };
    /// 16.11.2026: 9,768 (two models, thermal data 1,768)
    const LIMIT_VIEW: usize = 10_200;
    /// 16.11.2026: 3,968
    const LIMIT_MODEL: usize = 4_200;
}
