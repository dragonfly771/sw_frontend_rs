//! Automatic baud rate detection for the FLARM data port.
//!
//! Same procedure and timings as the SteFly C prototype (`flarm_task` in `app_main.c`):
//! - start with the last good baud rate (stored by the caller),
//! - lock after 3 valid lines at the current rate,
//! - while not locked, try the next candidate after 2.5 s,
//! - when locked and no valid line arrived for 10 s, unlock and search again
//!   (starting with the current rate for another 2.5 s).

#[cfg(not(flarm_standalone))]
#[allow(unused_imports)]
use micromath::F32Ext;

/// Candidate baud rates in search order.
pub const BAUD_CANDIDATES: [u32; 6] = [19200, 38400, 57600, 115200, 9600, 4800];
/// Dwell time per candidate while searching (ms).
pub const DWELL_MS: i64 = 2500;
/// Valid lines needed at one rate to lock.
pub const LOCK_LINES: u32 = 3;
/// Locked rate is given up after this long without a valid line (ms).
pub const LOST_MS: i64 = 10_000;

/// Baud rate search state machine. Feed it the reader results and call
/// [`BaudScanner::tick`] regularly (e.g. every UART read, 20 ms).
#[derive(Clone, Debug)]
pub struct BaudScanner {
    idx: usize,
    locked: bool,
    ok: u32,
    ok_at_switch: u32,
    bad: u32,
    bad_at_switch: u32,
    t_switch: Option<i64>,
    t_good: i64,
}

impl BaudScanner {
    /// Start searching at `start` (the last good rate); unknown rates start at 19200.
    pub fn new(start: u32) -> Self {
        let idx = BAUD_CANDIDATES
            .iter()
            .position(|&b| b == start)
            .unwrap_or(0);
        BaudScanner {
            idx,
            locked: false,
            ok: 0,
            ok_at_switch: 0,
            bad: 0,
            bad_at_switch: 0,
            t_switch: None,
            t_good: 0,
        }
    }

    /// Baud rate the UART should currently use.
    pub fn current(&self) -> u32 {
        BAUD_CANDIDATES
            .get(self.idx)
            .copied()
            .unwrap_or(BAUD_CANDIDATES[0])
    }

    /// `true` once 3 valid lines were seen at the current rate (until 10 s silence).
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// Rejected lines since the last switch (diagnostics only, does not influence the search).
    pub fn bad_since_switch(&self) -> u32 {
        self.bad.wrapping_sub(self.bad_at_switch)
    }

    /// Report a line with a valid checksum. Returns `true` when this line made
    /// the scanner lock (the caller should then persist [`BaudScanner::current`]).
    pub fn on_line_ok(&mut self, now_ms: i64) -> bool {
        if self.t_switch.is_none() {
            self.t_switch = Some(now_ms);
        }
        self.ok = self.ok.wrapping_add(1);
        self.t_good = now_ms;
        if !self.locked && self.ok.wrapping_sub(self.ok_at_switch) >= LOCK_LINES {
            self.locked = true;
            return true;
        }
        false
    }

    /// Report a rejected line (bad or missing checksum, too long). Like the C
    /// firmware this only counts; the switching is purely time based.
    pub fn on_bad(&mut self, now_ms: i64) {
        if self.t_switch.is_none() {
            self.t_switch = Some(now_ms);
        }
        self.bad = self.bad.wrapping_add(1);
    }

    /// Advance the timers. Returns the new baud rate when the UART has to be switched.
    pub fn tick(&mut self, now_ms: i64) -> Option<u32> {
        let t_switch = match self.t_switch {
            Some(t) => t,
            None => {
                self.t_switch = Some(now_ms);
                return None;
            }
        };
        if self.locked && now_ms.saturating_sub(self.t_good) > LOST_MS {
            self.locked = false;
            self.restart_dwell(now_ms);
            return None;
        }
        let since = now_ms.saturating_sub(t_switch);
        if !self.locked && (since > DWELL_MS || since < -DWELL_MS) {
            self.idx = (self.idx + 1) % BAUD_CANDIDATES.len();
            self.restart_dwell(now_ms);
            return Some(self.current());
        }
        None
    }

    fn restart_dwell(&mut self, now_ms: i64) {
        self.t_switch = Some(now_ms);
        self.ok_at_switch = self.ok;
        self.bad_at_switch = self.bad;
    }
}
