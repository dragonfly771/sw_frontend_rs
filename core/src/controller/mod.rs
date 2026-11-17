mod helpers;
pub use helpers::{
    can_frame::*,
    can_ids::{audio_legacy, frontend_legacy, sensor_legacy, GenericId, SpecialId},
    CanActive, CanConfigId, CircleStats, IntToDuration, NmeaBuffer, RemoteConfig, Scheduler, Tim,
};
pub(crate) use helpers::{
    DrainControl, FlashControl, GearAlarmControl, GearPins, InPinFunction, InTogglePinFunction,
    OutPinFunction, SpeedToFlyControl, ONE_PIN_MODE, PIN_IN_CLOSE, PIN_IN_OPEN, PIN_IN_TOGGLE,
    PIN_NONE, PIN_OUT_CLOSE, PIN_OUT_OPEN, TWO_PIN_MODE,
};

mod device_info;
pub use device_info::DeviceInfoControl;

mod editor;
#[cfg(test)]
pub(crate) use editor::activate_editable;
pub(crate) use editor::apply_page;
pub use editor::{close_edit_frame, Editor, PendingRemote};

pub(crate) mod menu;
pub use menu::{close_menu_display, MenuControl};

mod fw_update;
use fw_update::SwUpdateController;

mod sound;
pub use sound::{
    get_snd_spreading_factor, set_snd_spreading_factor, SoundControl, SoundParams, Waveform,
    SEQ_TICK_MS, SINK_TONE_CONTINUOUS, SINK_TONE_DOUBLE_BEEP, SINK_TONE_OFF, SND_BASE_AMPLITUDE,
    SND_EXP_MUL, WAVEFORM_RECTANGULAR, WAVEFORM_SAWTOOTH, WAVEFORM_SINE_WAVE, WAVEFORM_TRIANGULAR,
};

mod tone_profile;
pub use tone_profile::{
    anchors_from_spreading, db_to_factor, lerp_table, loudness_comp_db, speed_volume_db,
    stefly_beep_duty, stefly_beep_period_ms, stefly_climb_freq, stefly_sink_freq,
    stefly_tick_pause_ms, STEFLY_CENTER_FREQ, STEFLY_MAX_FREQ, STEFLY_MIN_FREQ,
};

mod pot_climb;
pub use pot_climb::{PotClimb, PotInput, PotMode};
mod needle_entry;
pub use needle_entry::NeedleEntry;

pub mod pitot_cal;

mod gust_filter;
pub use gust_filter::GustPt2;

pub mod flaps_ctl;

pub mod flasher;

pub mod flarm_ctl;

pub mod name_entry;

pub mod page_layout;

pub mod gear_alarm;
#[cfg(test)]
mod simple_tests;
pub mod volume_panel;
pub mod wizard;
pub use flarm_ctl::FlarmCtl;

mod tick_1s;

pub mod display_sync;
pub use display_sync::DisplaySync;
use tick_1s::*;

pub mod persist;
pub use persist::{profile_always_0, store_persistence_ids, Echo, PersistenceId};

use crate::{
    basic_config::{CONTROLLER_TICK_RATE, MAX_TX_FRAMES},
    common::PTxFrames,
    flight_physics::Polar,
    model::{
        editable::Content, CirclingDirection, DataSource, DisplayActive, EditMode, VarioModeControl,
    },
    system_of_units::{FloatToSpeed, Speed},
    utils::{KeyEvent, PIdleEvents, Pt1},
    CPersistenceItems, CoreModel, DeviceEvent, Editable, Event, IdleEvent, InputPinState,
    PersistenceItem, SdCardCmd, VarioMode,
};
use helpers::nmea_cyclic_200ms;

#[allow(unused_imports)]
use micromath::F32Ext;

use heapless::{FnvIndexMap, FnvIndexSet};

pub enum Direction {
    Forward,
    Backward,
}

pub enum ControlResult {
    Nothing,
    Edit(EditMode, Editable),
    NextDisplay(Direction),
}

pub type Callback = fn(cm: &mut CoreModel, cc: &mut CoreController);

pub enum Timer {
    Ticker1Hz,
    NmeaFast,
    PersistSetting,
    CloseEditFrame,
    CloseMenu,
}

/// settings waiting for the EEPROM (a power of two). 16 since 15.11.2026:
/// a sound preset sets eleven values at once, with 8 the last three were
/// not stored. The idle queue (utils/idle_events.rs) takes 19 events.
pub const MAX_PERS_IDS: usize = 16;
/// queued GET requests for sensor box configuration values
pub const REMOTE_GET_QUEUE: usize = 32;

pub struct CoreController {
    pub polar: Polar,
    pub drain_control: DrainControl,
    pub flash_control: FlashControl,
    pub speed_to_fly_control: SpeedToFlyControl,
    pub gear_alarm_control: GearAlarmControl,
    sw_update: SwUpdateController,
    sound_control: SoundControl,
    ms: u16,
    last_vario_mode: VarioMode,
    av2_climb_rate: Pt1<Speed>,
    av_speed_to_fly: Pt1<Speed>,
    circle_stats: CircleStats,
    pot_climb: PotClimb,
    needle_entry: NeedleEntry,
    gust_filter: GustPt2,
    pub nmea_buffer: NmeaBuffer,
    pub scheduler: Scheduler<5>,
    pub pers_vals: FnvIndexMap<PersistenceId, PersistenceItem, MAX_PERS_IDS>,
    pub nmea_vals: FnvIndexSet<PersistenceId, MAX_PERS_IDS>,
    /// sensor box value to write, frozen at the time of the change
    pub remote_val: Option<(CanConfigId, Content)>,
    /// sensor box values to ask for, sent one per 10 ms
    pub remote_gets: heapless::Deque<CanConfigId, REMOTE_GET_QUEUE>,
    /// FLARM port, canopy flasher, FLARM demo
    pub flarm: FlarmCtl,
    /// flap table items still to be written (see flaps_ctl::flush_flap_dirty)
    pub flap_dirty: u32,
    /// page words still to be written by the page layout editor
    /// (see page_layout::save_tick)
    pub page_save: u32,
    queue_to_idle_task: PIdleEvents,
    queue_from_idle_task: CPersistenceItems,
    p_tx_frames: PTxFrames<MAX_TX_FRAMES>,
    /// glider settings shared with the other display of a two-seater
    pub display_sync: DisplaySync,
}

impl CoreController {
    pub fn new(
        core_model: &mut CoreModel,
        queue_to_idle_task: PIdleEvents,
        queue_from_idle_task: CPersistenceItems,
        p_tx_frames: PTxFrames<MAX_TX_FRAMES>,
    ) -> Self {
        let av2_climb_rate = Pt1::new(
            0.0.m_s(),
            CONTROLLER_TICK_RATE,
            core_model.config.av2_climb_rate_tc,
        );
        let av_speed_to_fly = Pt1::new(
            0.0.m_s(),
            CONTROLLER_TICK_RATE,
            core_model.config.av_speed_to_fly_tc,
        );
        let mut scheduler = Scheduler::new([
            Tim::new(recalc_polar),
            Tim::new(nmea_cyclic_200ms),
            Tim::new(store_persistence_ids),
            Tim::new(close_edit_frame),
            Tim::new(close_menu_display),
        ]);
        scheduler.every(Timer::Ticker1Hz, 1.secs());
        scheduler.every(Timer::NmeaFast, 200.millis());
        Self {
            polar: Polar::default(),
            drain_control: DrainControl::default(),
            flash_control: FlashControl::default(),
            speed_to_fly_control: SpeedToFlyControl::default(),
            gear_alarm_control: GearAlarmControl::default(),
            sound_control: SoundControl::default(),
            ms: 0,
            last_vario_mode: VarioMode::Vario,
            sw_update: SwUpdateController::new(),
            av2_climb_rate,
            av_speed_to_fly,
            circle_stats: CircleStats::default(),
            pot_climb: PotClimb::new(),
            needle_entry: NeedleEntry::new(),
            gust_filter: GustPt2::new(),
            nmea_buffer: NmeaBuffer::new(),
            scheduler,
            nmea_vals: FnvIndexSet::new(),
            pers_vals: FnvIndexMap::new(),
            remote_val: None,
            remote_gets: heapless::Deque::new(),
            flarm: FlarmCtl::new(),
            flap_dirty: 0,
            page_save: 0,
            queue_to_idle_task,
            queue_from_idle_task,
            p_tx_frames,
            display_sync: DisplaySync::new(),
        }
    }

    pub fn event_handler(&mut self, event: Event, cm: &mut CoreModel) {
        match event {
            Event::KeyItem(key_event) => self.key_action(cm, key_event),
            Event::DeviceItem(device_event) => self.device_action(cm, &device_event),
            Event::InputItem(input_event) => self.input_action(cm, input_event),
        }
    }

    /// Call this latest after 1 ms
    ///
    /// time_ms is the absolute time. The internal counter is updated tick by tick until the time
    /// is caught up. A maximum of one callback routine is started in one call.
    pub fn tick_1ms(&mut self, time_ms: u16, cm: &mut CoreModel) -> bool {
        let mut recalc = false;
        while self.ms != time_ms {
            self.ms = self.ms.wrapping_add(1);
            cm.control.now_ms = cm.control.now_ms.wrapping_add(1);
            match self.ms % 100 {
                0 => {
                    // call scheduler every 100ms
                    let _ = self.scheduler.tick_100ms();
                }
                1 => {
                    self.tick_100ms(cm); // call 100ms tick routine
                    recalc = true;
                }
                ms_in_100 => {
                    if ms_in_100 % 10 == 5 {
                        // call the beep sequencer every 10 ms
                        if self.tick_10ms(cm) {
                            recalc = true;
                        }
                        self.remote_get_tick(cm);
                    } else if let Some(callback) = self.scheduler.next_callback() {
                        // alternatively: execute a callback every ms as long as available
                        callback(cm, self);
                    }
                }
            }
        }
        // Check queue everey time, tick_1ms is called
        if let Some(item) = self.queue_from_idle_task.dequeue() {
            // silently ignores values like NAN, is_subnormal etc
            let _ = persist::restore_item(self, cm, item);
        }
        recalc
    }

    pub fn set_ms(&mut self, time_ms: u16) {
        self.ms = time_ms;
    }

    pub fn recalc_glider(&mut self, cm: &mut CoreModel) {
        self.polar.recalc_glider(&cm.glider_data);
    }

    /// Beep sequencer of the Stefly tone profile
    ///
    /// Returns true when the sound parameters changed, so that the caller hands them to the
    /// sound driver. The state of an external CAN tone generator is updated as well.
    fn tick_10ms(&mut self, core_model: &mut CoreModel) -> bool {
        if self.sound_control.tick_10ms(core_model) {
            // send the sequenced state to external tone generators right away
            let can_frame = core_model.can_frame_sound();
            let _ = self.p_tx_frames.enqueue(can_frame); // ignore when queue is full
            true
        } else {
            false
        }
    }

    /// Send the next queued GET request for a sensor box value (one per 10 ms,
    /// see persist::queue_remote_get). Stays in the queue if the CAN transmit
    /// queue is full at the moment.
    fn remote_get_tick(&mut self, cm: &mut CoreModel) {
        if let Some(config_id) = self.remote_gets.pop_front() {
            let frame =
                cm.can_frame_remote_config(config_id, RemoteConfig::Get, Content::F32(None));
            if let Some(frame) = frame {
                if self.p_tx_frames.enqueue(frame).is_err() {
                    let _ = self.remote_gets.push_front(config_id);
                }
            }
        }
    }

    fn tick_100ms(&mut self, core_model: &mut CoreModel) {
        core_model.control.alive_ticks = core_model.control.alive_ticks.wrapping_add(1);
        // demo for the fair: simulated flight values first, as if they came
        // from the sensor box
        flaps_ctl::demo_tick_100ms(core_model);
        // The spider assistant places its action arcs by the vario time
        // constant of the sensor box: ask for it 3 s after start-up and
        // then once a minute, in case it was changed.
        if core_model.control.alive_ticks % 600 == 30 {
            persist::queue_remote_get(self, CanConfigId::VarioTc, false);
        }
        // Level Flight on a sensor box that applies the fine tune only
        // with Calc Orientation: send it once the box has averaged, then
        // ask for the tilt angles so the Orientation list shows the result
        if core_model.control.level_flight_calc > 0 {
            core_model.control.level_flight_calc -= 1;
            if core_model.control.level_flight_calc == 0 {
                persist::send_can_config_frame(
                    core_model,
                    self,
                    CanConfigId::CmdCalcSensorOrientation,
                    RemoteConfig::Get,
                );
                for id in [
                    CanConfigId::SensTiltRoll,
                    CanConfigId::SensTiltPitch,
                    CanConfigId::SensTiltYaw,
                ] {
                    persist::queue_remote_get(self, id, false);
                }
            }
        }
        pitot_cal::tick(core_model, self);
        self.needle_entry.tick(core_model);
        self.pot_climb.tick(core_model, &self.polar);
        self.gust_filter.tick(core_model);
        core_model.control.circling_direction = CirclingDirection::from_turn_rate(
            core_model.sensor.turn_rate.to_rad_s(),
            core_model.control.circling_direction,
        );

        if core_model.control.vario_mode == VarioMode::Vario {
            self.av2_climb_rate.tick(core_model.sensor.climb_rate);
            if core_model.control.avg_climb_slave_ticks == 0 {
                core_model.calculated.av2_climb_rate = self.av2_climb_rate.value();
            }
        }

        // Calculate speed_to_fly and speed_to_fly_dif
        let climb_rate = core_model.sensor.climb_rate;
        let mc_cready = core_model.config.mc_cready;
        let sink_rate = self.polar.sink_rate(core_model.sensor.airspeed);
        core_model.calculated.speed_to_fly =
            self.polar.speed_to_fly(climb_rate - sink_rate, mc_cready);
        self.av_speed_to_fly
            .tick(core_model.calculated.speed_to_fly.ias());
        core_model.calculated.av_speed_to_fly = self.av_speed_to_fly.value();
        core_model.calculated.speed_to_fly_dif =
            core_model.calculated.av_speed_to_fly - core_model.sensor.airspeed.ias();

        let can_frame = core_model.can_frame_avg_climb_rates();
        let _ = self.p_tx_frames.enqueue(can_frame); // ignore when queue is full

        // flap recommendation and actual flap position (flap tape)
        flaps_ctl::flaps_tick_100ms(core_model);

        // FLARM: baud rate, traffic, warnings, canopy flasher, demo
        flarm_ctl::flarm_tick_100ms(core_model, self);

        // page layout editor: changed pages are written a few per tick
        page_layout::save_tick(core_model, self);
        // volume panel: closes by itself after a pause
        volume_panel::tick_100ms(core_model, self);

        // calc sound params
        if let Some(event) = self.sound_control.sound(core_model) {
            self.send_idle_event(event);
        }

        // create frame for external CAN loudspeaker
        let can_frame = core_model.can_frame_sound();
        let _ = self.p_tx_frames.enqueue(can_frame); // ignore when queue is full
    }

    pub fn send_idle_event(&mut self, idle_event: IdleEvent) {
        let _ = self.queue_to_idle_task.enqueue(idle_event);
    }

    // Event handler for reactions to inputs
    fn input_action(&mut self, cm: &mut CoreModel, input_event: InputPinState) {
        match input_event {
            InputPinState::Io1(state) => self.drain_control.set_state(cm, state),
            InputPinState::Io2(state) => self.speed_to_fly_control.set_state(cm, state),
            InputPinState::Io3(state) => {
                self.gear_alarm_control.set_gear_pin_state(cm, state);
                gear_alarm::update(cm, self);
            }
            InputPinState::Io4(state) => {
                self.gear_alarm_control.set_airbrakes_pin_state(cm, state);
                gear_alarm::update(cm, self);
            }
        }
    }

    // Event handler for events generated by the device
    fn device_action(&mut self, core_model: &mut CoreModel, device_event: &DeviceEvent) {
        match device_event {
            DeviceEvent::FwAvailable(_) => {
                core_model.config.last_display_active = core_model.config.display_active;
                core_model.config.display_active = DisplayActive::FirmwareUpdate;
            }
            DeviceEvent::UploadFinished => {
                core_model.config.display_active = core_model.config.last_display_active
            }

            _ => (),
        }
        if core_model.config.display_active == DisplayActive::FirmwareUpdate {
            self.sw_update.device_action(core_model, device_event);
            self.send_idle_event(IdleEvent::SdCardItem(SdCardCmd::SwUpdateAccepted))
        }
    }

    // Event handler for keystrokes
    fn key_action(&mut self, cm: &mut CoreModel, mut key_event: KeyEvent) {
        // the radar page takes the small encoder for the target selection
        flarm_ctl::key_action(&mut key_event, cm, self);
        // the page layout editor takes all keys while it is on
        page_layout::key_action(&mut key_event, cm, self);
        // the volume panel takes all keys while it is shown
        volume_panel::key_action(&mut key_event, cm, self);
        // a push on the gear alarm confirms it
        gear_alarm::key_action(&mut key_event, cm, self);
        // a push on "NO SENSOR BOX" takes the words off the page
        if crate::view::vario_ring::no_box_banner(cm)
            && matches!(key_event, KeyEvent::BtnEnc | KeyEvent::BtnEsc)
        {
            cm.control.no_box_ack = true;
            key_event = KeyEvent::NoEvent;
        }
        editor::key_action(&mut key_event, cm, self);
        menu::key_action(&mut key_event, cm, self);
        device_info::key_action(&mut key_event, cm, self);
    }
}
