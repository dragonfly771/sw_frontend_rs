//! FLARM traffic display core logic.
//!
//! Dependency-free, `no_std`, no allocation. Port of the SteFly C prototype of a FLARM display
//! (`components/core`): NMEA reader/parser, traffic model with extrapolation,
//! display decisions, demo generator and automatic baud rate detection.
//!
//! Typical use per frame (about 30 Hz):
//! 1. feed received UART bytes into [`nmea::NmeaReader::feed`] and pass every
//!    returned message to [`traffic::Traffic::handle`] (and the line result to
//!    [`baud::BaudScanner`]),
//! 2. call [`traffic::Traffic::housekeeping`],
//! 3. call [`logic::compute`] and draw the returned [`logic::FlarmView`].

#[cfg(not(flarm_standalone))]
#[allow(unused_imports)]
use micromath::F32Ext;

pub mod baud;
pub mod demo;
pub mod logic;
pub mod nmea;
pub mod traffic;
