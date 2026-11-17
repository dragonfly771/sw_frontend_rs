//! What the FLARM screens show - filled by the controller every 100 ms
//! (controller/flarm_ctl.rs), drawn by view/flarm_pages.rs.

use crate::flarm::logic::FlarmView;

/// state of the FLARM data port (Control Box: data status LED)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlarmPortStatus {
    /// no bytes at all for 5 s
    NoData,
    /// bytes, but no PFLAU for 5 s (Control Box: "Check Baud Rate")
    NoPflau,
    Ok,
}

/// short message over the picture after an encoder action
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlarmToast {
    None,
    Following,
    NotFollowing,
    SelectionLost,
}

#[derive(Clone, Copy, Debug)]
pub struct FlarmShow {
    pub view: FlarmView,
    pub status: FlarmPortStatus,
    /// baud rate the port runs with
    pub baud: u32,
    /// automatic detection has found valid sentences at this rate
    pub baud_locked: bool,
    /// last real PFLAU: GPS 0/1/2, 255 = none yet
    pub gps: u8,
    pub alarm_level: u8,
    /// output sequence of the canopy flasher (0 = off)
    pub flasher_seq: u8,
    pub flasher_test: bool,
    /// the FLARM demo feeds the screens
    pub demo: bool,
    pub toast: FlarmToast,
    pub toast_id: u32,
    /// 100 ms ticks the toast is still shown
    pub toast_ticks: u8,
}

impl FlarmShow {
    pub const EMPTY: FlarmShow = FlarmShow {
        view: FlarmView::EMPTY,
        status: FlarmPortStatus::NoData,
        baud: 19200,
        baud_locked: false,
        gps: 255,
        alarm_level: 0,
        flasher_seq: 0,
        flasher_test: false,
        demo: false,
        toast: FlarmToast::None,
        toast_id: 0,
        toast_ticks: 0,
    };

    /// status line of the FLARM menu and the device info
    pub fn status_str(&self) -> &'static str {
        if self.demo {
            return "Demo";
        }
        match self.status {
            FlarmPortStatus::NoData => "No data",
            FlarmPortStatus::NoPflau => "No PFLAU",
            FlarmPortStatus::Ok => "OK",
        }
    }

    pub fn gps_str(&self) -> &'static str {
        match self.gps {
            0 => "No fix",
            1 => "Ground",
            2 => "Airborne",
            _ => "-",
        }
    }
}

impl Default for FlarmShow {
    fn default() -> Self {
        FlarmShow::EMPTY
    }
}
