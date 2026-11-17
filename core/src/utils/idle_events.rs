use crate::{DateTime, PersistenceId, PersistenceItem};
use heapless::spsc::{Consumer, Producer, Queue};

#[derive(Debug, Copy, Clone)]
pub enum IdleEvent {
    SetEepromItem(PersistenceItem),
    ClearEepromItems(&'static [PersistenceId]),
    RestoreEepromItems,
    RestoreToSandardProfile,
    SdCardItem(SdCardCmd),
    FeedTheDog,
    SetGain(u8),
    SetBacklight(u8),
    DateTime(DateTime),
    ResetDevice(ResetReason),
    Output1(PinState),
    Output2(PinState),
    /// SteFly canopy flasher: sequence 0 (off) .. 9 as 20 Hz PWM on DO1
    /// (controller/flasher.rs::high_time_us)
    FlasherSeq(u8),
    /// baud rate of the serial NMEA port in bit/s (Setup -> System -> NMEA Port)
    NmeaBaud(u32),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PinState {
    Low,
    High,
}

impl core::ops::Not for PinState {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            PinState::High => PinState::Low,
            PinState::Low => PinState::High,
        }
    }
}

impl core::convert::From<bool> for PinState {
    fn from(is_high: bool) -> Self {
        if is_high {
            PinState::High
        } else {
            PinState::Low
        }
    }
}

impl PinState {
    pub fn as_str(&self) -> &'static str {
        match self {
            PinState::Low => "Low",
            PinState::High => "High",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ResetReason {
    NoReason,
    ConfigChanged,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SdCardCmd {
    SwUpdateAccepted,
    SwUpdateCanceled,
}

// This queue transports the configuration PersItems from controller to the idle loop.
const MAX_IDLE_EVENTS: usize = 20;
pub type QIdleEvents = Queue<IdleEvent, MAX_IDLE_EVENTS>;
pub type PIdleEvents = Producer<'static, IdleEvent, MAX_IDLE_EVENTS>;
pub type CIdleEvents = Consumer<'static, IdleEvent, MAX_IDLE_EVENTS>;
