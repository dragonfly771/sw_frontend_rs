//! FLARM data port reader and parser.
//!
//! Sentences: PFLAU, PFLAA, xxRMC, PFLAE (decoded), GGA and PFLAV (recognised
//! only). Based solely on the public FLARM Data Port Specification. Lines
//! without a valid `*hh` checksum are rejected.
//!
//! Number parsing mimics the C `strtol`/`strtod` behaviour of the original
//! (leading whitespace and sign accepted, parsing stops at the first invalid
//! character, an empty field means "not present").

#[cfg(not(flarm_standalone))]
#[allow(unused_imports)]
use micromath::F32Ext;

/// Maximum accepted line length in bytes (from `$` up to the checksum, without CR/LF).
pub const MAX_LINE: usize = 100;
/// Maximum number of comma separated fields looked at (including the sentence type).
const MAX_FIELDS: usize = 24;

/// PFLAU: FLARM status and the most important alarm (FLARM decides the priority).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pflau {
    /// Number of devices received.
    pub rx: u8,
    /// Transmission status, 1 = ok.
    pub tx: u8,
    /// 0 = no GPS, 1 = 3D fix on ground, 2 = 3D fix airborne.
    pub gps: u8,
    /// Power status, 1 = ok.
    pub power: u8,
    /// Alarm level 0..3.
    pub alarm_level: u8,
    /// Relative bearing in degrees (-180..180) relative to own track; `None` for non-directional targets.
    pub rel_bearing: Option<i16>,
    /// Alarm type (hex): 0 none, 2 aircraft, 3 obstacle/alert zone, 4 traffic advisory, 0x41.. zones.
    pub alarm_type: u8,
    /// Relative vertical distance in metres, positive = target higher; `None` if the field is empty.
    pub rel_vertical: Option<i32>,
    /// Relative horizontal distance in metres (0 if empty).
    pub rel_distance: i32,
    /// Target ID (hex); `None` if the field is missing or empty.
    pub id: Option<u32>,
}

impl Pflau {
    /// All fields zero / not present.
    pub const EMPTY: Pflau = Pflau {
        rx: 0,
        tx: 0,
        gps: 0,
        power: 0,
        alarm_level: 0,
        rel_bearing: None,
        alarm_type: 0,
        rel_vertical: None,
        rel_distance: 0,
        id: None,
    };
}

impl Default for Pflau {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// PFLAA: one target in the vicinity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pflaa {
    /// Alarm level 0..3.
    pub alarm_level: u8,
    /// `false` if RelativeEast is empty: only distance known (Mode-S/C transponder).
    pub directional: bool,
    /// Relative north in metres; for non-directional targets the estimated distance.
    pub rel_north: i32,
    /// Relative east in metres (0 if empty).
    pub rel_east: i32,
    /// Relative vertical in metres, positive = target higher.
    pub rel_vertical: i32,
    /// ID type (0 random, 1 ICAO, 2 FLARM).
    pub id_type: u8,
    /// Target ID (hex).
    pub id: u32,
    /// Track over ground in degrees true, `None` if empty.
    pub track: Option<i16>,
    /// Turn rate in deg/s (right positive), `None` if empty (usually empty).
    pub turn_rate: Option<f32>,
    /// Ground speed in m/s, `None` if empty.
    pub ground_speed: Option<i16>,
    /// Climb rate in m/s, `None` if empty.
    pub climb: Option<f32>,
    /// FLARM aircraft type (hex), 1 = glider.
    pub acft_type: u8,
    /// NoTrack flag set (field equals 1).
    pub no_track: bool,
}

impl Pflaa {
    /// All fields zero / not present.
    pub const EMPTY: Pflaa = Pflaa {
        alarm_level: 0,
        directional: false,
        rel_north: 0,
        rel_east: 0,
        rel_vertical: 0,
        id_type: 0,
        id: 0,
        track: None,
        turn_rate: None,
        ground_speed: None,
        climb: None,
        acft_type: 0,
        no_track: false,
    };
}

impl Default for Pflaa {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// xxRMC (GPRMC, GNRMC, ...): own movement and UTC time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gprmc {
    /// Status field is `A` (valid fix).
    pub valid: bool,
    /// UTC time (hh, mm, ss) if the time field has at least 6 characters.
    pub time: Option<(u8, u8, u8)>,
    /// Position (lat, lon) in degrees, `None` if latitude or longitude is empty.
    pub pos: Option<(f64, f64)>,
    /// Ground speed in knots (0 if empty).
    pub gs_kt: f32,
    /// Course over ground in degrees true, `None` if empty.
    pub course: Option<f32>,
}

impl Gprmc {
    /// All fields zero / not present.
    pub const EMPTY: Gprmc = Gprmc {
        valid: false,
        time: None,
        pos: None,
        gs_kt: 0.0,
        course: None,
    };
}

impl Default for Gprmc {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// PFLAE: FLARM self test result / error.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Pflae {
    /// Severity 0..3.
    pub severity: u8,
    /// Error code (hex).
    pub code: u16,
}

impl Pflae {
    /// No error.
    pub const EMPTY: Pflae = Pflae {
        severity: 0,
        code: 0,
    };
}

/// One decoded line with a valid checksum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NmeaMsg {
    /// PFLAU status / alarm.
    Pflau(Pflau),
    /// PFLAA target.
    Pflaa(Pflaa),
    /// xxRMC own movement.
    Gprmc(Gprmc),
    /// PFLAE error.
    Pflae(Pflae),
    /// xxGGA (recognised, not decoded).
    Gga,
    /// PFLAV version (recognised, not decoded).
    Pflav,
    /// Any other sentence with a valid checksum (still counts as "data received").
    Other,
}

/// Assembles lines from a byte stream, validates the checksum and decodes them.
#[derive(Clone, Debug)]
pub struct NmeaReader {
    buf: [u8; MAX_LINE],
    len: usize,
    /// Number of lines with a valid checksum.
    pub ok_count: u32,
    /// Number of rejected lines (bad/missing checksum or too long).
    pub bad_count: u32,
}

impl Default for NmeaReader {
    fn default() -> Self {
        Self::new()
    }
}

impl NmeaReader {
    /// Empty reader.
    pub const fn new() -> Self {
        NmeaReader {
            buf: [0; MAX_LINE],
            len: 0,
            ok_count: 0,
            bad_count: 0,
        }
    }

    /// Feed one byte. Returns the decoded message when a complete line with a
    /// valid checksum ends with this byte (CR or LF).
    ///
    /// `$` always starts a new line (an unfinished line is dropped silently),
    /// lines shorter than 6 bytes are ignored without counting, lines longer
    /// than [`MAX_LINE`] are dropped and counted as bad.
    pub fn feed(&mut self, b: u8) -> Option<NmeaMsg> {
        if b == b'$' {
            self.buf[0] = b;
            self.len = 1;
            return None;
        }
        if self.len == 0 {
            return None;
        }
        if b == b'\r' || b == b'\n' {
            let n = self.len;
            self.len = 0;
            if n < 6 {
                return None;
            }
            let line = self.buf.get(..n).unwrap_or(&[]);
            if checksum_ok(line) {
                self.ok_count = self.ok_count.wrapping_add(1);
                return Some(parse(line));
            }
            self.bad_count = self.bad_count.wrapping_add(1);
            return None;
        }
        match self.buf.get_mut(self.len) {
            Some(slot) => {
                *slot = b;
                self.len += 1;
            }
            None => {
                // too long: drop
                self.len = 0;
                self.bad_count = self.bad_count.wrapping_add(1);
            }
        }
        None
    }

    /// Feed a slice and call `f` for every decoded message.
    pub fn feed_slice<F: FnMut(NmeaMsg)>(&mut self, bytes: &[u8], mut f: F) {
        for &b in bytes {
            if let Some(m) = self.feed(b) {
                f(m);
            }
        }
    }
}

/// The SteFly C prototype works on 0-terminated strings: everything after a NUL byte is ignored.
fn cstr(line: &[u8]) -> &[u8] {
    match line.iter().position(|&c| c == 0) {
        Some(p) => line.get(..p).unwrap_or(&[]),
        None => line,
    }
}

fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'F' => Some(c - b'A' + 10),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

/// Checks `$...*hh`: XOR of all bytes between `$` and `*` must equal the hex
/// value `hh` (upper or lower case). Bytes after `hh` are ignored.
pub fn checksum_ok(line: &[u8]) -> bool {
    let line = cstr(line);
    if line.first() != Some(&b'$') {
        return false;
    }
    let mut cs = 0u8;
    let mut i = 1;
    while let Some(&c) = line.get(i) {
        if c == b'*' {
            break;
        }
        cs ^= c;
        i += 1;
    }
    if line.get(i) != Some(&b'*') {
        return false;
    }
    let h = line.get(i + 1).copied().and_then(hexval);
    let l = line.get(i + 2).copied().and_then(hexval);
    match (h, l) {
        (Some(h), Some(l)) => cs == ((h << 4) | l),
        _ => false,
    }
}

/// Appends `*hh\r\n` to the sentence `buf[..len]` (which starts with `$`) and
/// returns the new length. If the buffer is too small, nothing is appended and
/// `len` is returned unchanged.
pub fn finish(buf: &mut [u8], len: usize) -> usize {
    let len = len.min(buf.len());
    if buf.len() - len < 5 {
        return len;
    }
    let mut cs = 0u8;
    for &c in buf.get(1..len).unwrap_or(&[]) {
        cs ^= c;
    }
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    buf[len] = b'*';
    buf[len + 1] = HEX[(cs >> 4) as usize];
    buf[len + 2] = HEX[(cs & 15) as usize];
    buf[len + 3] = b'\r';
    buf[len + 4] = b'\n';
    len + 5
}

/* ---------------- field splitting and number parsing ---------------- */

struct Fields<'a> {
    line: &'a [u8],
    f: [(usize, usize); MAX_FIELDS],
    n: usize,
}

impl<'a> Fields<'a> {
    /// Split at commas after `$`, empty fields are kept, stops at `*` or end.
    fn split(line: &'a [u8]) -> Self {
        let mut fs = Fields {
            line,
            f: [(0, 0); MAX_FIELDS],
            n: 0,
        };
        let mut p = 1usize;
        let mut start = 1usize;
        while fs.n < MAX_FIELDS {
            let c = line.get(p).copied().unwrap_or(0);
            if c == b',' || c == b'*' || c == 0 {
                fs.f[fs.n] = (start, p);
                fs.n += 1;
                if c != b',' {
                    break;
                }
                start = p + 1;
            }
            p += 1;
        }
        fs
    }

    fn get(&self, i: usize) -> &'a [u8] {
        if i >= self.n {
            return &[];
        }
        match self.f.get(i) {
            Some(&(a, b)) => self.line.get(a..b).unwrap_or(&[]),
            None => &[],
        }
    }

    fn empty(&self, i: usize) -> bool {
        self.get(i).is_empty()
    }

    fn int(&self, i: usize, def: i64) -> i64 {
        let s = self.get(i);
        if s.is_empty() {
            return def;
        }
        strtol(s.get(..s.len().min(15)).unwrap_or(&[]), 10)
    }

    fn hex(&self, i: usize, def: i64) -> i64 {
        let s = self.get(i);
        if s.is_empty() {
            return def;
        }
        strtol(s.get(..s.len().min(15)).unwrap_or(&[]), 16)
    }

    fn dbl(&self, i: usize, def: f64) -> f64 {
        let s = self.get(i);
        if s.is_empty() {
            return def;
        }
        strtod(s.get(..s.len().min(23)).unwrap_or(&[]))
    }

    fn is(&self, i: usize, s: &[u8]) -> bool {
        let f = self.get(i);
        !f.is_empty() && f == s
    }
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// Like C `strtol`: whitespace, sign, (`0x` for base 16), digits; stops at the
/// first invalid character; saturates instead of overflowing.
fn strtol(s: &[u8], base: u32) -> i64 {
    let mut i = 0;
    while s.get(i).map_or(false, |&c| is_space(c)) {
        i += 1;
    }
    let mut neg = false;
    if let Some(&c) = s.get(i) {
        if c == b'+' || c == b'-' {
            neg = c == b'-';
            i += 1;
        }
    }
    if base == 16
        && s.get(i) == Some(&b'0')
        && matches!(s.get(i + 1), Some(b'x') | Some(b'X'))
        && s.get(i + 2).copied().and_then(hexval).is_some()
    {
        i += 2;
    }
    let mut v: i64 = 0;
    while let Some(&c) = s.get(i) {
        let d = match hexval(c) {
            Some(d) if (d as u32) < base => d,
            _ => break,
        };
        v = v.saturating_mul(base as i64).saturating_add(d as i64);
        i += 1;
    }
    if neg {
        -v
    } else {
        v
    }
}

/// Like C `strtod` for decimal input (`[ws][sign]digits[.digits][e[sign]digits]`).
/// Hex floats, `inf` and `nan` are not supported and parse as 0.
fn strtod(s: &[u8]) -> f64 {
    let mut i = 0;
    while s.get(i).map_or(false, |&c| is_space(c)) {
        i += 1;
    }
    let mut neg = false;
    if let Some(&c) = s.get(i) {
        if c == b'+' || c == b'-' {
            neg = c == b'-';
            i += 1;
        }
    }
    let mut mant: u64 = 0;
    let mut exp10: i32 = 0;
    let mut any = false;
    while let Some(&c) = s.get(i) {
        if !c.is_ascii_digit() {
            break;
        }
        any = true;
        if mant < 100_000_000_000_000_000 {
            mant = mant * 10 + (c - b'0') as u64;
        } else {
            exp10 += 1;
        }
        i += 1;
    }
    if s.get(i) == Some(&b'.') {
        i += 1;
        while let Some(&c) = s.get(i) {
            if !c.is_ascii_digit() {
                break;
            }
            any = true;
            if mant < 100_000_000_000_000_000 {
                mant = mant * 10 + (c - b'0') as u64;
                exp10 -= 1;
            }
            i += 1;
        }
    }
    if !any {
        return 0.0;
    }
    if matches!(s.get(i), Some(b'e') | Some(b'E')) {
        let mut j = i + 1;
        let mut eneg = false;
        if let Some(&c) = s.get(j) {
            if c == b'+' || c == b'-' {
                eneg = c == b'-';
                j += 1;
            }
        }
        if s.get(j).map_or(false, |c| c.is_ascii_digit()) {
            let mut e: i32 = 0;
            while let Some(&c) = s.get(j) {
                if !c.is_ascii_digit() {
                    break;
                }
                e = (e * 10 + (c - b'0') as i32).min(10_000);
                j += 1;
            }
            exp10 += if eneg { -e } else { e };
        }
    }
    if mant == 0 {
        return if neg { -0.0 } else { 0.0 };
    }
    let e = exp10.clamp(-400, 400);
    let mut p = 1.0f64;
    for _ in 0..e.unsigned_abs() {
        p *= 10.0;
    }
    let v = if e < 0 {
        mant as f64 / p
    } else {
        mant as f64 * p
    };
    if neg {
        -v
    } else {
        v
    }
}

/* ---------------- sentence decoding ---------------- */

/// PFLAU,<RX>,<TX>,<GPS>,<Power>,<AlarmLevel>,<RelativeBearing>,<AlarmType>,
/// <RelativeVertical>,<RelativeDistance>[,<ID>]
fn parse_pflau(fs: &Fields) -> Pflau {
    Pflau {
        rx: fs.int(1, 0) as u8,
        tx: fs.int(2, 0) as u8,
        gps: fs.int(3, 0) as u8,
        power: fs.int(4, 0) as u8,
        alarm_level: fs.int(5, 0) as u8,
        rel_bearing: if fs.empty(6) {
            None
        } else {
            Some(fs.int(6, 0) as i16)
        },
        alarm_type: fs.hex(7, 0) as u8,
        rel_vertical: if fs.empty(8) {
            None
        } else {
            Some(fs.int(8, 0) as i32)
        },
        rel_distance: fs.int(9, 0) as i32,
        id: if fs.empty(10) {
            None
        } else {
            Some(fs.hex(10, 0) as u32)
        },
    }
}

/// PFLAA,<AlarmLevel>,<RelativeNorth>,<RelativeEast>,<RelativeVertical>,<IDType>,
/// <ID>,<Track>,<TurnRate>,<GroundSpeed>,<ClimbRate>,<AcftType>[,<NoTrack>[,<Source>,<RSSI>]]
fn parse_pflaa(fs: &Fields) -> Pflaa {
    Pflaa {
        alarm_level: fs.int(1, 0) as u8,
        rel_north: fs.int(2, 0) as i32,
        directional: !fs.empty(3),
        rel_east: fs.int(3, 0) as i32,
        rel_vertical: fs.int(4, 0) as i32,
        id_type: fs.int(5, 0) as u8,
        id: fs.hex(6, 0) as u32,
        track: if fs.empty(7) {
            None
        } else {
            Some(fs.int(7, 0) as i16)
        },
        turn_rate: if fs.empty(8) {
            None
        } else {
            Some(fs.dbl(8, 0.0) as f32)
        },
        ground_speed: if fs.empty(9) {
            None
        } else {
            Some(fs.int(9, 0) as i16)
        },
        climb: if fs.empty(10) {
            None
        } else {
            Some(fs.dbl(10, 0.0) as f32)
        },
        acft_type: fs.hex(11, 0) as u8,
        no_track: fs.int(12, 0) == 1,
    }
}

fn nmea_coord(v: f64, neg: bool) -> f64 {
    let deg = ((v / 100.0) as i32) as f64;
    let r = deg + (v - deg * 100.0) / 60.0;
    if neg {
        -r
    } else {
        r
    }
}

/// xxRMC,hhmmss.ss,A,llll.ll,a,yyyyy.yy,a,x.x,x.x,ddmmyy,...
fn parse_rmc(fs: &Fields) -> Gprmc {
    let mut o = Gprmc::EMPTY;
    o.valid = fs.is(2, b"A");
    let t = fs.get(1);
    if t.len() >= 6 {
        // like the SteFly C prototype: no digit check, wrapping byte arithmetic
        let d = |i: usize| t.get(i).copied().unwrap_or(b'0').wrapping_sub(b'0');
        let two = |i: usize| d(i).wrapping_mul(10).wrapping_add(d(i + 1));
        o.time = Some((two(0), two(2), two(4)));
    }
    if !fs.empty(3) && !fs.empty(5) {
        o.pos = Some((
            nmea_coord(fs.dbl(3, 0.0), fs.is(4, b"S")),
            nmea_coord(fs.dbl(5, 0.0), fs.is(6, b"W")),
        ));
    }
    o.gs_kt = fs.dbl(7, 0.0) as f32;
    o.course = if fs.empty(8) {
        None
    } else {
        Some(fs.dbl(8, 0.0) as f32)
    };
    o
}

/// Decode a line (`$...*hh`, checksum is not checked here; see [`checksum_ok`]).
/// Never fails: unknown sentence types give [`NmeaMsg::Other`].
pub fn parse(line: &[u8]) -> NmeaMsg {
    let line = cstr(line);
    let fs = Fields::split(line);
    let t = fs.get(0);
    if t.len() != 5 {
        return NmeaMsg::Other;
    }
    let tail = t.get(2..5).unwrap_or(&[]);
    if t == b"PFLAU" {
        NmeaMsg::Pflau(parse_pflau(&fs))
    } else if t == b"PFLAA" {
        NmeaMsg::Pflaa(parse_pflaa(&fs))
    } else if tail == b"RMC" {
        NmeaMsg::Gprmc(parse_rmc(&fs))
    } else if tail == b"GGA" {
        NmeaMsg::Gga
    } else if t == b"PFLAE" {
        NmeaMsg::Pflae(Pflae {
            severity: fs.int(2, 0) as u8,
            code: fs.hex(3, 0) as u16,
        })
    } else if t == b"PFLAV" {
        NmeaMsg::Pflav
    } else {
        NmeaMsg::Other
    }
}

/// Checks the checksum and decodes one line; `None` if the checksum is wrong or missing.
pub fn parse_checked(line: &[u8]) -> Option<NmeaMsg> {
    if checksum_ok(line) {
        Some(parse(line))
    } else {
        None
    }
}

/* ---------------- sentence writer (demo, tests) ---------------- */

/// Minimal printf replacement for building NMEA sentences in a fixed buffer.
/// Output that does not fit is dropped and marks the writer as overflowed.
pub struct SentenceWriter<'a> {
    buf: &'a mut [u8],
    len: usize,
    overflow: bool,
}

impl<'a> SentenceWriter<'a> {
    /// Start writing at the beginning of `buf`.
    pub fn new(buf: &'a mut [u8]) -> Self {
        SentenceWriter {
            buf,
            len: 0,
            overflow: false,
        }
    }

    /// Bytes written so far.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Nothing written yet.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Append one byte.
    pub fn byte(&mut self, b: u8) -> &mut Self {
        match self.buf.get_mut(self.len) {
            Some(s) => {
                *s = b;
                self.len += 1;
            }
            None => self.overflow = true,
        }
        self
    }

    /// Append a string.
    pub fn str(&mut self, s: &str) -> &mut Self {
        for &b in s.as_bytes() {
            self.byte(b);
        }
        self
    }

    fn digits(&mut self, mut v: u64, base: u64, width: usize) {
        let mut tmp = [0u8; 20];
        let mut n = 0;
        loop {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            tmp[n] = HEX[(v % base) as usize];
            n += 1;
            v /= base;
            if v == 0 || n >= tmp.len() {
                break;
            }
        }
        for _ in n..width {
            self.byte(b'0');
        }
        while n > 0 {
            n -= 1;
            self.byte(tmp[n]);
        }
    }

    /// Append a signed integer (`%d`).
    pub fn int(&mut self, v: i64) -> &mut Self {
        self.int_pad(v, 0)
    }

    /// Append a signed integer zero padded to `width` digits (`%0Nd`).
    pub fn int_pad(&mut self, v: i64, width: usize) -> &mut Self {
        if v < 0 {
            self.byte(b'-');
        }
        self.digits(v.unsigned_abs(), 10, width);
        self
    }

    /// Append an upper case hex number zero padded to `width` digits (`%0NX`, width 0 = `%X`).
    pub fn hex(&mut self, v: u32, width: usize) -> &mut Self {
        self.digits(v as u64, 16, width);
        self
    }

    /// Append a fixed point number with `decimals` (0..9) decimals, total width
    /// `width` zero padded (`%0W.Df`). Non-finite values are written as 0.
    pub fn fixed(&mut self, v: f64, decimals: u32, width: usize) -> &mut Self {
        let decimals = decimals.min(9);
        let v = if v.is_finite() { v } else { 0.0 };
        let neg = v < 0.0;
        let a = if neg { -v } else { v };
        let mut scale: u64 = 1;
        for _ in 0..decimals {
            scale *= 10;
        }
        let r = a * scale as f64 + 0.5;
        let n = if r >= 1.8e19 { u64::MAX / 10 } else { r as u64 };
        let ip = n / scale;
        let fp = n % scale;
        let mut ilen = 1;
        let mut x = ip / 10;
        while x > 0 {
            ilen += 1;
            x /= 10;
        }
        let total =
            ilen + if decimals > 0 {
                1 + decimals as usize
            } else {
                0
            } + neg as usize;
        if neg {
            self.byte(b'-');
        }
        self.digits(ip, 10, ilen + width.saturating_sub(total));
        if decimals > 0 {
            self.byte(b'.');
            self.digits(fp, 10, decimals as usize);
        }
        self
    }

    /// Append `*hh\r\n` and return the total length, or 0 if anything did not fit.
    pub fn finish(&mut self) -> usize {
        if self.overflow {
            return 0;
        }
        let n = finish(self.buf, self.len);
        if n != self.len + 5 {
            self.overflow = true;
            return 0;
        }
        self.len = n;
        n
    }
}
