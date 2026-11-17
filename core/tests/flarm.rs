// Tests of the FLARM port (crate corelib::flarm): parser, baud rate search,
// traffic model, warning logic, selection, demo, stress with 130 targets.
// The same tests run standalone with tools/flarm_standalone/test_flarm.rs.

use corelib::flarm;

use flarm::baud::{BaudScanner, BAUD_CANDIDATES};
use flarm::demo::{Demo, Scenario};
use flarm::logic::{
    compute, FollowResult, IdleKind, LogicConfig, LogicState, ViewKind, MAX_OTHERS,
};
use flarm::nmea::{checksum_ok, finish, parse_checked, NmeaMsg, NmeaReader, SentenceWriter};
use flarm::traffic::{since, Traffic, MAX_TARGETS, TARGET_SHOW_MS};

/* ---------------- helpers ---------------- */

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.below(1_000_000) as f32 / 1_000_000.0)
    }
}

/// Body ("$..." without checksum) -> full sentence with "*hh\r\n".
fn sentence(body: &str) -> Vec<u8> {
    let mut b = vec![0u8; body.len() + 8];
    b[..body.len()].copy_from_slice(body.as_bytes());
    let n = finish(&mut b, body.len());
    b.truncate(n);
    b
}

/// Body -> line with checksum but without CR/LF.
fn line(body: &str) -> Vec<u8> {
    let mut s = sentence(body);
    s.truncate(s.len() - 2);
    s
}

fn feed(rd: &mut NmeaReader, tr: &mut Traffic, bytes: &[u8], t: i64) {
    for &b in bytes {
        if let Some(m) = rd.feed(b) {
            tr.handle(&m, t);
        }
    }
}

fn send(rd: &mut NmeaReader, tr: &mut Traffic, body: &str, t: i64) {
    feed(rd, tr, &sentence(body), t);
}

/// Feed a full sentence into a fresh reader and return the message.
fn parse_body(body: &str) -> Option<NmeaMsg> {
    let mut rd = NmeaReader::new();
    let mut out = None;
    for b in sentence(body) {
        if let Some(m) = rd.feed(b) {
            out = Some(m);
        }
    }
    out
}

fn approx(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

/* ---------------- parser ---------------- */

#[test]
fn parser_pflaa() {
    let l = line("$PFLAA,0,-1234,1234,220,2,DD8F12,180,,30,-1.4,1");
    assert!(checksum_ok(&l));
    match parse_checked(&l) {
        Some(NmeaMsg::Pflaa(a)) => {
            assert_eq!(
                (a.rel_north, a.rel_east, a.rel_vertical),
                (-1234, 1234, 220)
            );
            assert!(a.directional);
            assert_eq!(a.id_type, 2);
            assert_eq!(a.id, 0xDD8F12);
            assert_eq!(a.track, Some(180));
            assert_eq!(a.turn_rate, None);
            assert_eq!(a.ground_speed, Some(30));
            assert!(approx(a.climb.unwrap(), -1.4, 0.01));
            assert_eq!(a.acft_type, 1);
            assert!(!a.no_track);
        }
        other => panic!("{:?}", other),
    }
    // non-directional, optional fields empty, NoTrack, extra Source/RSSI fields
    match parse_body("$PFLAA,1,850,,-40,1,4B1A2C,,,,,8,1,1,-74") {
        Some(NmeaMsg::Pflaa(a)) => {
            assert!(!a.directional);
            assert_eq!(
                (a.alarm_level, a.rel_north, a.rel_east, a.rel_vertical),
                (1, 850, 0, -40)
            );
            assert_eq!(
                (a.track, a.turn_rate, a.ground_speed, a.climb),
                (None, None, None, None)
            );
            assert_eq!(a.acft_type, 8);
            assert!(a.no_track);
        }
        other => panic!("{:?}", other),
    }
    // turn rate present, acft type hex
    match parse_body("$PFLAA,0,10,20,30,2,ABCDEF,90,12.5,40,0.3,D") {
        Some(NmeaMsg::Pflaa(a)) => {
            assert_eq!(a.turn_rate, Some(12.5));
            assert_eq!(a.acft_type, 0xD);
        }
        other => panic!("{:?}", other),
    }
    // truncated after ID (checksum valid): everything after is "not present"
    match parse_body("$PFLAA,0,10,20,30,2,ABCDEF") {
        Some(NmeaMsg::Pflaa(a)) => {
            assert_eq!(a.id, 0xABCDEF);
            assert_eq!((a.track, a.ground_speed, a.climb), (None, None, None));
            assert_eq!(a.acft_type, 0);
        }
        other => panic!("{:?}", other),
    }
}

#[test]
fn parser_pflau() {
    match parse_body("$PFLAU,3,1,2,1,2,-30,2,-32,755,DD8F12") {
        Some(NmeaMsg::Pflau(p)) => {
            assert_eq!((p.rx, p.tx, p.gps, p.power, p.alarm_level), (3, 1, 2, 1, 2));
            assert_eq!(p.rel_bearing, Some(-30));
            assert_eq!(p.alarm_type, 2);
            assert_eq!(p.rel_vertical, Some(-32));
            assert_eq!(p.rel_distance, 755);
            assert_eq!(p.id, Some(0xDD8F12));
        }
        other => panic!("{:?}", other),
    }
    // non-directional: bearing empty, no ID
    match parse_body("$PFLAU,1,1,2,1,1,,2,150,1200") {
        Some(NmeaMsg::Pflau(p)) => {
            assert_eq!(p.rel_bearing, None);
            assert_eq!(p.rel_distance, 1200);
            assert_eq!(p.rel_vertical, Some(150));
            assert_eq!(p.id, None);
        }
        other => panic!("{:?}", other),
    }
    // no alarm, all trailing fields empty
    match parse_body("$PFLAU,0,1,2,1,0,,0,,,") {
        Some(NmeaMsg::Pflau(p)) => {
            assert_eq!(
                (p.rel_bearing, p.rel_vertical, p.id, p.rel_distance),
                (None, None, None, 0)
            );
        }
        other => panic!("{:?}", other),
    }
    // alarm type as hex (alert zone), strtol-like partial numbers
    match parse_body("$PFLAU, 4,1,2,1,+3,12x,41,-5,300,0x1A") {
        Some(NmeaMsg::Pflau(p)) => {
            assert_eq!(p.rx, 4);
            assert_eq!(p.alarm_level, 3);
            assert_eq!(p.rel_bearing, Some(12));
            assert_eq!(p.alarm_type, 0x41);
            assert_eq!(p.id, Some(0x1A));
        }
        other => panic!("{:?}", other),
    }
}

#[test]
fn parser_rmc_pflae_other() {
    match parse_body("$GPRMC,123519.00,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W") {
        Some(NmeaMsg::Gprmc(r)) => {
            assert!(r.valid);
            assert_eq!(r.time, Some((12, 35, 19)));
            let (lat, lon) = r.pos.unwrap();
            assert!((lat - 48.1173).abs() < 1e-6, "{}", lat);
            assert!((lon - 11.516_666_6).abs() < 1e-6, "{}", lon);
            assert!(approx(r.gs_kt, 22.4, 1e-4));
            assert_eq!(r.course, Some(84.4));
        }
        other => panic!("{:?}", other),
    }
    match parse_body("$GNRMC,000102,A,3348.000,S,07030.000,W,10,,010126,,,A") {
        Some(NmeaMsg::Gprmc(r)) => {
            assert_eq!(r.time, Some((0, 1, 2)));
            let (lat, lon) = r.pos.unwrap();
            assert!(lat < -33.79 && lon < -70.49);
            assert_eq!(r.course, None);
        }
        other => panic!("{:?}", other),
    }
    match parse_body("$GPRMC,,V,,,,,,,,,,N") {
        Some(NmeaMsg::Gprmc(r)) => {
            assert!(!r.valid);
            assert_eq!((r.time, r.pos, r.course), (None, None, None));
        }
        other => panic!("{:?}", other),
    }
    assert_eq!(
        parse_body("$PFLAE,A,2,81"),
        Some(NmeaMsg::Pflae(flarm::nmea::Pflae {
            severity: 2,
            code: 0x81
        }))
    );
    assert_eq!(
        parse_body("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,"),
        Some(NmeaMsg::Gga)
    );
    assert_eq!(parse_body("$PFLAV,A,2.00,7.20,"), Some(NmeaMsg::Pflav));
    assert_eq!(parse_body("$PXYZW,1,2"), Some(NmeaMsg::Other));
    assert_eq!(parse_body("$PFLA,1,2"), Some(NmeaMsg::Other));
}

#[test]
fn parser_checksum_and_truncation() {
    // wrong checksum
    assert!(!checksum_ok(b"$PFLAU,0,1,2,1,0,,0,,*00"));
    // lower case checksum accepted
    let mut l = line("$PFLAU,0,1,2,1,0,,0,,,");
    let n = l.len();
    l[n - 2] = l[n - 2].to_ascii_lowercase();
    l[n - 1] = l[n - 1].to_ascii_lowercase();
    assert!(checksum_ok(&l));

    let mut rd = NmeaReader::new();
    let mut got = 0;
    let mut feed_all = |rd: &mut NmeaReader, s: &[u8]| {
        for &b in s {
            if rd.feed(b).is_some() {
                got += 1;
            }
        }
    };
    let good = sentence("$PFLAU,3,1,2,1,2,-30,2,-32,755,DD8F12");
    // bad checksum
    feed_all(&mut rd, b"$PFLAU,0,1,2,1,0,,0,,*00\r\n");
    assert_eq!((rd.ok_count, rd.bad_count), (0, 1));
    // no checksum at all
    feed_all(&mut rd, b"$PFLAU,3,1,2,1,2,-30,2,-32,755,DD8F12\r\n");
    assert_eq!(rd.bad_count, 2);
    // '*' but no / one hex digit
    feed_all(&mut rd, b"$PFLAU,3,1,2,1,2,-30,2,-32,755,DD8F12*\r\n");
    feed_all(&mut rd, b"$PFLAU,3,1,2,1,2,-30*4\r\n");
    assert_eq!(rd.bad_count, 4);
    // truncated line interrupted by a new '$': dropped silently, the next line is fine
    feed_all(&mut rd, b"$PFLAU,3,1,2");
    feed_all(&mut rd, &good);
    assert_eq!((rd.ok_count, rd.bad_count), (1, 4));
    // very short lines are ignored, not counted
    feed_all(&mut rd, b"$GP\r\n$\r\n\r\n");
    assert_eq!((rd.ok_count, rd.bad_count), (1, 4));
    // too long (> 100 bytes): dropped and counted, next line ok
    let mut long = b"$PFLAA,0".to_vec();
    long.extend(std::iter::repeat(b'1').take(150));
    long.extend_from_slice(b"*00\r\n");
    feed_all(&mut rd, &long);
    assert_eq!(rd.bad_count, 5);
    feed_all(&mut rd, &good);
    assert_eq!(rd.ok_count, 2);
    // garbage before '$' and bytes without '$' are ignored
    feed_all(&mut rd, b"garbage,*12\r\n");
    feed_all(&mut rd, &good);
    assert_eq!((rd.ok_count, rd.bad_count), (3, 5));
    // CR LF and LF only both end a line
    let mut lf = line("$PFLAU,0,1,2,1,0,,0,,,");
    lf.push(b'\n');
    feed_all(&mut rd, &lf);
    assert_eq!(rd.ok_count, 4);
    assert_eq!(got, 4);
    // exactly 100 bytes is accepted
    let mut body = String::from("$PFLAE,A,0,0,");
    while body.len() < 97 {
        body.push('x');
    }
    assert_eq!(line(&body).len(), 100);
    let mut rd2 = NmeaReader::new();
    assert!(sentence(&body).iter().filter_map(|&b| rd2.feed(b)).count() == 1);
}

#[test]
fn sentence_writer_format() {
    let mut buf = [0u8; 64];
    let n = {
        let mut w = SentenceWriter::new(&mut buf);
        w.str("$X,")
            .int(-12)
            .byte(b',')
            .int_pad(7, 2)
            .byte(b',')
            .hex(0xDD1A2B, 6)
            .byte(b',')
            .hex(0xD, 0);
        w.byte(b',')
            .fixed(-1.45, 1, 0)
            .byte(b',')
            .fixed(7.03856, 5, 8)
            .byte(b',')
            .fixed(64.75, 1, 0);
        w.finish()
    };
    let s = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(
        s.starts_with("$X,-12,07,DD1A2B,D,-1.5,07.03856,64.8*"),
        "{}",
        s
    );
    assert!(checksum_ok(&buf[..n]));
    // overflow: nothing usable
    let mut small = [0u8; 8];
    let mut w = SentenceWriter::new(&mut small);
    w.str("$PFLAU,1,2,3");
    assert_eq!(w.finish(), 0);
}

/* ---------------- extrapolation (port of test_core.c) ---------------- */

fn prediction_figures(sc: Scenario) -> (f64, f64, f64, usize) {
    let mut d = Demo::new(sc);
    let mut tr = Traffic::new();
    let mut rd = NmeaReader::new();
    let mut buf = [0u8; 2048];
    let (mut err_sum, mut err_max, mut jump_max) = (0.0f64, 0.0f64, 0.0f64);
    let mut cnt = 0usize;
    let mut last = [(0.0f32, 0.0f32, false); flarm::demo::MAX_OBJ];
    let dur = d.duration_ms();
    let mut t = 0i64;
    while t <= dur {
        let n = d.step(t, &mut buf);
        feed(&mut rd, &mut tr, &buf[..n], t);
        if t >= 3000 {
            let own = *d.own();
            for (k, o) in d.objects().iter().enumerate() {
                let gi = tr.find(o.id);
                let g = gi.and_then(|i| tr.target(i));
                if !o.active || g.map_or(true, |g| t - g.t_rx > 1500) {
                    last[k].2 = false;
                    continue;
                }
                let (rn, re, _) = tr.target_pos(gi.unwrap(), t);
                let (tn, te) = (o.n - own.n, o.e - own.e);
                let e = ((rn as f64 - tn).powi(2) + (re as f64 - te).powi(2)).sqrt();
                err_sum += e;
                cnt += 1;
                err_max = err_max.max(e);
                if last[k].2 {
                    let j = (((rn - last[k].0).powi(2) + (re - last[k].1).powi(2)) as f64).sqrt();
                    jump_max = jump_max.max(j);
                }
                last[k] = (rn, re, true);
            }
        }
        t += 33;
    }
    assert_eq!(rd.bad_count, 0);
    (
        if cnt > 0 { err_sum / cnt as f64 } else { 0.0 },
        err_max,
        jump_max,
        cnt,
    )
}

#[test]
fn extrapolation_error() {
    // C reference (test_core): mean 0.4..0.6 m, max 1.2..3.9 m, max move per frame 2.0..4.9 m
    for sc in Scenario::LIST {
        let (mean, max, jump, cnt) = prediction_figures(sc);
        println!(
            "scenario {:<9}: mean error {:.1} m, max {:.1} m, largest move per frame {:.1} m ({} samples)",
            sc.name(),
            mean,
            max,
            jump,
            cnt
        );
        assert!(cnt > 100, "{:?}: too few samples", sc);
        assert!(mean < 1.0, "{:?}: mean error {}", sc, mean);
        assert!(max < 6.0, "{:?}: max error {}", sc, max);
        assert!(jump < 8.0, "{:?}: jump {}", sc, jump);
    }
}

/* ---------------- stress (port of stress.c without drawing) ---------------- */

#[test]
fn stress_gaggle_eviction_and_noise() {
    const N_TGT: usize = 130;
    let mut rng = XorShift(4711);
    struct T {
        id: u32,
        n: f32,
        e: f32,
        v: f32,
        trk: f32,
        gs: f32,
        climb: f32,
        alarm: u8,
    }
    let mut tg: Vec<T> = (0..N_TGT)
        .map(|i| {
            let r = 300.0 + rng.below(7000) as f32;
            let a = rng.below(360) as f32 * 0.0174533;
            T {
                id: 0x400000 + i as u32,
                n: r * a.cos(),
                e: r * a.sin(),
                v: rng.below(600) as f32 - 300.0,
                trk: rng.below(360) as f32,
                gs: 25.0 + rng.below(15) as f32,
                climb: (rng.below(60) as f32 - 20.0) / 10.0,
                alarm: 0,
            }
        })
        .collect();
    // team mate = farthest target, target 1 = alarm
    let team = tg[0].id;
    tg[0].n = 9500.0;
    tg[0].e = 0.0;
    tg[0].v = 100.0;
    tg[1].n = 280.0;
    tg[1].e = 120.0;
    tg[1].v = 20.0;
    tg[1].alarm = 2;

    let mut tr = Traffic::new();
    tr.set_team(team);
    let mut rd = NmeaReader::new();
    let cfg = LogicConfig::DEFAULT;
    let mut st = LogicState::new();
    let (mut frames, mut team_missing, mut threat_missing, mut wrong_nearest, mut max_shown) =
        (0, 0, 0, 0, 0);
    let mut warn_frames = 0;

    let corrupt_feed =
        |rd: &mut NmeaReader, tr: &mut Traffic, s: &[u8], t: i64, rng: &mut XorShift| {
            for &b in s {
                let mut c = b;
                if rng.below(1000) < 1 {
                    c = rng.next() as u8; // noise byte, 0.1 %
                }
                if let Some(m) = rd.feed(c) {
                    tr.handle(&m, t);
                }
            }
        };

    let mut t = 0i64;
    while t <= 120_000 {
        if t % 1000 < 33 {
            let sec = t / 1000;
            let s = sentence(&format!(
                "$GPRMC,1200{:02}.00,A,4830.000,N,01130.000,E,50.0,0.0,221026,,,A",
                sec % 60
            ));
            corrupt_feed(&mut rd, &mut tr, &s, t, &mut rng);
            let mut ord: Vec<usize> = (0..N_TGT).collect();
            for i in (1..N_TGT).rev() {
                let j = rng.below(i as u64 + 1) as usize;
                ord.swap(i, j);
            }
            for &k in &ord {
                let p = &tg[k];
                let s = sentence(&format!(
                    "$PFLAA,{},{},{},{},2,{:06X},{},,{},{:.1},1",
                    p.alarm,
                    p.n as i32,
                    p.e as i32,
                    p.v as i32,
                    p.id,
                    p.trk as i32,
                    p.gs as i32,
                    p.climb
                ));
                corrupt_feed(&mut rd, &mut tr, &s, t, &mut rng);
            }
            let s = sentence(&format!(
                "$PFLAU,{},1,2,1,2,23,2,20,304,{:06X}",
                N_TGT, tg[1].id
            ));
            corrupt_feed(&mut rd, &mut tr, &s, t, &mut rng);
        }
        tr.housekeeping(t);
        let v = compute(&tr, &cfg, &mut st, t);
        if t >= 5000 {
            frames += 1;
            if v.kind == ViewKind::Warn || v.kind == ViewKind::HeadOn {
                warn_frames += 1;
            }
            max_shown = max_shown.max(v.n_others);
            if tr.find(team).is_none() {
                team_missing += 1;
            }
            if tr.find(tg[1].id).is_none() {
                threat_missing += 1;
            }
            // shown targets must be the nearest ones
            let max_in = v.others().iter().map(|o| o.dist).fold(0.0f32, f32::max);
            let mut closer_hidden = 0;
            for (i, g) in tr.tg.iter().enumerate() {
                if !g.used || t - g.t_rx > TARGET_SHOW_MS {
                    continue;
                }
                let (rn, re, _) = tr.target_pos(i, t);
                let d = (rn * rn + re * re).sqrt();
                let shown = v.others().iter().any(|o| o.id == g.id);
                if !shown && d < max_in - 1.0 {
                    closer_hidden += 1;
                }
            }
            if closer_hidden > 0 {
                wrong_nearest += 1;
            }
            // sorted by distance
            assert!(v.others().windows(2).all(|w| w[0].dist <= w[1].dist));
        }
        t += 33;
    }
    println!(
        "stress: frames {}, NMEA ok={} bad={} (noise), shown max {} (limit {}), team evicted {}, alarm evicted {}, wrong nearest {}, warn frames {}",
        frames, rd.ok_count, rd.bad_count, max_shown, MAX_OTHERS, team_missing, threat_missing, wrong_nearest, warn_frames
    );
    assert_eq!(team_missing, 0, "team mate evicted");
    assert_eq!(threat_missing, 0, "alarm target evicted");
    assert!(max_shown <= MAX_OTHERS);
    assert_eq!(max_shown, MAX_OTHERS);
    assert_eq!(wrong_nearest, 0, "not the nearest targets shown");
    assert!(rd.bad_count > 0 && rd.ok_count > 10_000);
    assert!(warn_frames > frames / 2);
    assert_eq!(tr.tg.iter().filter(|g| g.used).count(), MAX_TARGETS);
}

/* ---------------- fuzzing ---------------- */

#[test]
fn fuzz_reader_random_bytes() {
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let mut rd = NmeaReader::new();
    let mut tr = Traffic::new();
    let mut st = LogicState::new();
    let cfg = LogicConfig::DEFAULT;
    let alphabet = b"$*,\r\n0123456789ABCDEFPLUARMCGNV-.+ ";
    let mut t = 0i64;
    for i in 0..400_000u32 {
        // half fully random bytes, half from an NMEA-ish alphabet
        let b = if rng.below(2) == 0 {
            rng.next() as u8
        } else {
            alphabet[rng.below(alphabet.len() as u64) as usize]
        };
        if let Some(m) = rd.feed(b) {
            tr.handle(&m, t);
        }
        if i % 500 == 0 {
            t += rng.below(200) as i64;
            tr.housekeeping(t);
            let _ = compute(&tr, &cfg, &mut st, t);
        }
    }
    println!("fuzz random bytes: ok={} bad={}", rd.ok_count, rd.bad_count);
    assert!(rd.bad_count > 0);
}

/// Random but checksum-valid sentences: exercises the field parsers, the traffic model and the logic.
#[test]
fn fuzz_valid_checksum_sentences() {
    let mut rng = XorShift(12345);
    let mut rd = NmeaReader::new();
    let mut tr = Traffic::new();
    let mut st = LogicState::new();
    let mut cfg = LogicConfig::DEFAULT;
    let kinds = [
        "PFLAU", "PFLAA", "GPRMC", "GNRMC", "PFLAE", "GPGGA", "PFLAV", "XXXXX", "PFLA", "",
    ];
    let tokens = [
        "",
        "0",
        "1",
        "2",
        "3",
        "-1",
        "+7",
        "65535",
        "-2147483649",
        "99999999999999999999",
        "1e30",
        "-1e400",
        "1.5e",
        "0x1F",
        "DD8F12",
        "ffffffff",
        "A",
        "V",
        "N",
        "S",
        "E",
        "W",
        "4807.038",
        "01131.000",
        " 12",
        "--3",
        ".",
        "-.5",
        "123519.00",
        "12",
        "abc",
        "180",
        "-180",
        "720",
        "3.4e38",
        "nan",
        "inf",
    ];
    let mut t: i64 = 0;
    let mut msgs = 0u32;
    for i in 0..60_000u32 {
        let mut body = String::from("$");
        body.push_str(kinds[rng.below(kinds.len() as u64) as usize]);
        let nf = rng.below(16);
        for _ in 0..nf {
            body.push(',');
            if rng.below(4) == 0 {
                body.push_str(&format!(
                    "{}",
                    rng.next() as i32 as i64 * (rng.below(3) as i64 - 1)
                ));
            } else {
                body.push_str(tokens[rng.below(tokens.len() as u64) as usize]);
            }
        }
        if body.len() > 95 {
            body.truncate(95);
        }
        for b in sentence(&body) {
            if let Some(m) = rd.feed(b) {
                msgs += 1;
                tr.handle(&m, t);
            }
        }
        // time: mostly forward, sometimes backwards or huge jumps
        t = match rng.below(100) {
            0 => t.wrapping_sub(rng.below(100_000) as i64),
            1 => i64::MAX - rng.below(1000) as i64,
            2 => i64::MIN + rng.below(1000) as i64,
            3 => 0,
            _ => t.saturating_add(rng.below(300) as i64),
        };
        if i % 7 == 0 {
            tr.housekeeping(t);
            cfg.min_level = rng.below(5) as u8;
            cfg.headon_enabled = rng.below(2) == 0;
            cfg.tz_offset_min = rng.next() as i32;
            let v = compute(&tr, &cfg, &mut st, t);
            assert!(v.n_others <= MAX_OTHERS && v.n_nondir <= 2);
        }
        match rng.below(50) {
            0 => {
                st.select_step(&tr, t, rng.below(2) == 0);
            }
            1 => {
                st.follow_selected(&mut tr, t);
            }
            2 => tr.set_attitude(rng.f(-1e9, 1e9), rng.f(-90.0, 90.0), rng.f(-90.0, 90.0), t),
            _ => {}
        }
    }
    println!(
        "fuzz valid sentences: ok={} bad={} msgs={}",
        rd.ok_count, rd.bad_count, msgs
    );
    assert!(msgs > 50_000);
    assert_eq!(rd.bad_count, 0);
}

#[test]
fn extreme_timestamps_do_not_panic() {
    let mut rd = NmeaReader::new();
    let mut tr = Traffic::new();
    let mut st = LogicState::new();
    let cfg = LogicConfig::DEFAULT;
    tr.set_team(0xDD1);
    for &t in &[i64::MIN, -1, 0, 1, i64::MAX, i64::MIN + 1, i64::MAX - 1] {
        send(
            &mut rd,
            &mut tr,
            "$GPRMC,120000.00,A,4830.000,N,01130.000,E,50.0,10.0,221026,,,A",
            t,
        );
        send(
            &mut rd,
            &mut tr,
            "$PFLAA,2,100,10,5,2,000DD1,190,,30,1.0,1",
            t,
        );
        send(&mut rd, &mut tr, "$PFLAA,0,1000,10,5,1,000DD2,,,,,8", t);
        send(&mut rd, &mut tr, "$PFLAU,1,1,2,1,2,5,2,5,100,000DD1", t);
        tr.set_attitude(10.0, 1.0, 2.0, t);
        for &now in &[i64::MIN, 0, i64::MAX] {
            tr.housekeeping(now);
            let _ = compute(&tr, &cfg, &mut st, now);
            st.select_step(&tr, now, true);
            let _ = tr.team_rel(now);
            let _ = tr.own_heading(now);
        }
    }
    let mut b = BaudScanner::new(57600);
    for &now in &[i64::MIN, 0, i64::MAX, i64::MIN] {
        b.tick(now);
        b.on_line_ok(now);
        b.on_bad(now);
    }
    let mut d = Demo::new(Scenario::All);
    let mut buf = [0u8; 2048];
    for &now in &[i64::MIN, 0, 5000, i64::MAX, 0] {
        d.step(now, &mut buf);
    }
    // tiny output buffers
    let mut small = [0u8; 10];
    let mut d = Demo::new(Scenario::Mixed);
    for k in 0..100 {
        assert_eq!(d.step(k * 100, &mut small), 0);
    }
    let mut empty: [u8; 0] = [];
    d.step(20_000, &mut empty);
}

/* ---------------- logic: status, warnings, head-on ---------------- */

const RMC_NORTH: &str = "$GPRMC,120000.00,A,4830.000,N,01130.000,E,60.0,0.0,221026,,,A";

/// Traffic with own aircraft flying north at 60 kt (several messages so the track is known).
fn northbound(rd: &mut NmeaReader) -> (Traffic, i64) {
    let mut tr = Traffic::new();
    let mut t = 0;
    for _ in 0..3 {
        t += 1000;
        send(rd, &mut tr, RMC_NORTH, t);
    }
    (tr, t)
}

fn deg(x: f32) -> f32 {
    x * std::f32::consts::PI / 180.0
}

/// Single head-on test case: target at `brg` deg bearing, `elev` deg elevation,
/// 500 m away, flying `trk`.
fn headon_case(brg: f32, elev: f32, trk: i32, cfg: &LogicConfig) -> (ViewKind, f32, f32) {
    let mut rd = NmeaReader::new();
    let (mut tr, t) = northbound(&mut rd);
    let d = 500.0f32;
    let (n, e, v) = (d * deg(brg).cos(), d * deg(brg).sin(), d * deg(elev).tan());
    send(
        &mut rd,
        &mut tr,
        &format!(
            "$PFLAA,2,{:.0},{:.0},{:.0},2,DD0001,{},,30,0.0,1",
            n, e, v, trk
        ),
        t,
    );
    send(
        &mut rd,
        &mut tr,
        &format!("$PFLAU,2,1,2,1,2,{:.0},2,{:.0},500,DD0001", brg, v),
        t,
    );
    let mut st = LogicState::new();
    let v = compute(&tr, cfg, &mut st, t);
    (v.kind, v.threat.bearing, v.threat.elev)
}

/// plan_wurzel_genau.md (0.4.1.49): the distance shown is the true one. At 512 m north and
/// 512 m east (724 m) the root estimate of micromath was at its worst: 768 m.
#[test]
fn a_target_shows_its_true_distance() {
    let cfg = LogicConfig::DEFAULT;
    let mut st = LogicState::new();
    let mut rd = NmeaReader::new();
    let (mut tr, t) = northbound(&mut rd);
    send(
        &mut rd,
        &mut tr,
        "$PFLAA,0,512,512,0,2,DD0002,0,,0,0.0,1",
        t,
    );
    send(
        &mut rd,
        &mut tr,
        "$PFLAA,0,-300,400,0,2,DD0003,0,,0,0.0,1",
        t,
    );
    let v = compute(&tr, &cfg, &mut st, t);
    let dist = |id: u32| {
        v.others()
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.dist)
            .unwrap_or_else(|| panic!("target {id:X} not shown"))
    };
    assert!(approx(dist(0xDD0002), 724.08, 0.05), "{}", dist(0xDD0002));
    assert!(approx(dist(0xDD0003), 500.0, 0.05), "{}", dist(0xDD0003));
}

#[test]
fn status_views() {
    let cfg = LogicConfig::DEFAULT;
    let mut st = LogicState::new();
    let mut rd = NmeaReader::new();
    let mut tr = Traffic::new();
    assert_eq!(compute(&tr, &cfg, &mut st, 0).kind, ViewKind::Startup);
    send(&mut rd, &mut tr, "$GPRMC,,V,,,,,,,,,,N", 0);
    assert_eq!(compute(&tr, &cfg, &mut st, 100).kind, ViewKind::NoGps);
    send(&mut rd, &mut tr, RMC_NORTH, 1000);
    assert_eq!(compute(&tr, &cfg, &mut st, 1100).kind, ViewKind::Idle);
    // FLARM says no GPS
    send(&mut rd, &mut tr, "$PFLAU,0,1,0,1,0,,0,,,", 1200);
    assert_eq!(compute(&tr, &cfg, &mut st, 1300).kind, ViewKind::NoGps);
    send(&mut rd, &mut tr, "$PFLAU,0,1,2,1,0,,0,,,", 1400);
    let v = compute(&tr, &cfg, &mut st, 1500);
    assert_eq!((v.kind, v.idle), (ViewKind::Idle, IdleKind::Clear));
    assert_eq!(v.time, Some((12, 0, 0)));
    // stale data
    assert_eq!(
        compute(&tr, &cfg, &mut st, 1400 + 5001).kind,
        ViewKind::NoFlarm
    );
    // obstacle
    send(&mut rd, &mut tr, RMC_NORTH, 7000);
    send(&mut rd, &mut tr, "$PFLAU,0,1,2,1,3,-20,3,-50,300", 7000);
    let v = compute(&tr, &cfg, &mut st, 7000);
    assert_eq!((v.kind, v.level), (ViewKind::Obstacle, 3));
    assert!(approx(v.threat.bearing, -20.0, 0.5));
    // non-directional
    send(&mut rd, &mut tr, "$PFLAA,1,800,,40,1,4B1A2C,,,,,8", 8000);
    send(&mut rd, &mut tr, "$PFLAU,1,1,2,1,1,,2,40,800,4B1A2C", 8000);
    let v = compute(&tr, &cfg, &mut st, 8000);
    assert_eq!(v.kind, ViewKind::WarnNonDir);
    assert_eq!(v.n_nondir, 1);
    assert!(approx(v.nondir[0].dist, 800.0, 0.1));
    // level 1 hidden with min_level 2
    let cfg2 = LogicConfig {
        min_level: 2,
        ..cfg
    };
    assert_eq!(compute(&tr, &cfg2, &mut st, 8000).kind, ViewKind::Idle);
    // directional warning from PFLAU only (target unknown): bearing relative to track
    send(
        &mut rd,
        &mut tr,
        "$PFLAU,1,1,2,1,2,60,2,10,400,ABC123",
        9000,
    );
    let v = compute(&tr, &cfg, &mut st, 9000);
    assert_eq!((v.kind, v.threat.id), (ViewKind::Warn, 0xABC123));
    assert!(approx(v.threat.bearing, 60.0, 0.5));
    assert!(approx(v.threat.x, 400.0 * deg(60.0).sin(), 1.0) && approx(v.threat.y, 200.0, 1.0));
}

#[test]
fn headon_rule() {
    let cfg = LogicConfig::DEFAULT;
    // inside +-10 deg -> head-on
    for &(b, e) in &[(0.0, 0.0), (5.0, 0.0), (-9.0, 3.0), (2.0, -9.0), (9.5, 9.5)] {
        let (k, brg, el) = headon_case(b, e, 180, &cfg);
        assert_eq!(
            k,
            ViewKind::HeadOn,
            "bearing {} elev {} -> {} / {}",
            b,
            e,
            brg,
            el
        );
    }
    // outside -> directional warning
    for &(b, e) in &[
        (15.0, 0.0),
        (-15.0, 0.0),
        (0.0, 15.0),
        (0.0, -12.0),
        (11.0, 0.0),
        (45.0, 0.0),
    ] {
        let (k, brg, el) = headon_case(b, e, 180, &cfg);
        assert_eq!(
            k,
            ViewKind::Warn,
            "bearing {} elev {} -> {} / {}",
            b,
            e,
            brg,
            el
        );
    }
    // not opposite (same direction, slower: overtaking) -> warn
    assert_eq!(headon_case(0.0, 0.0, 0, &cfg).0, ViewKind::Warn);
    // track 140 deg (40 deg off reciprocal) still counts as oncoming, 120 not
    assert_eq!(headon_case(0.0, 0.0, 140, &cfg).0, ViewKind::HeadOn);
    assert_eq!(headon_case(0.0, 0.0, 120, &cfg).0, ViewKind::Warn);
    // disabled
    let off = LogicConfig {
        headon_enabled: false,
        ..cfg
    };
    assert_eq!(headon_case(0.0, 0.0, 180, &off).0, ViewKind::Warn);
    // min level above the alarm level: no warning at all
    let hi = LogicConfig {
        min_level: 3,
        ..cfg
    };
    assert_eq!(headon_case(0.0, 0.0, 180, &hi).0, ViewKind::Idle);

    // circling -> no head-on
    let mut rd = NmeaReader::new();
    let mut tr = Traffic::new();
    let mut t = 0;
    for k in 0..5 {
        t += 1000;
        send(
            &mut rd,
            &mut tr,
            &format!(
                "$GPRMC,1200{:02}.00,A,4830.000,N,01130.000,E,50.0,{}.0,221026,,,A",
                k,
                k * 20
            ),
            t,
        );
    }
    assert!(tr.circling);
    let (h, _) = tr.own_heading(t);
    let (n, e) = (500.0 * deg(h).cos(), 500.0 * deg(h).sin());
    let trk = (h as i32 + 180) % 360;
    send(
        &mut rd,
        &mut tr,
        &format!("$PFLAA,2,{:.0},{:.0},0,2,DD0001,{},,30,0.0,1", n, e, trk),
        t,
    );
    send(&mut rd, &mut tr, "$PFLAU,2,1,2,1,2,0,2,0,500,DD0001", t);
    let mut st = LogicState::new();
    let v = compute(&tr, &cfg, &mut st, t);
    assert!(v.threat.bearing.abs() < 1.0);
    assert_eq!(v.kind, ViewKind::Warn);
}

/// Simulated encounter: own north at `own_gs`, target south at `tgt_gs` with
/// lateral offset `lat` (m) and height `dv`. Returns (time, kind, bearing) per 100 ms frame.
fn encounter(
    own_gs: f32,
    tgt_gs: f32,
    lat: f32,
    start: f32,
    dv: f32,
    secs: i64,
) -> Vec<(i64, ViewKind, f32)> {
    let cfg = LogicConfig::DEFAULT;
    let mut st = LogicState::new();
    let mut rd = NmeaReader::new();
    let mut tr = Traffic::new();
    let mut out = Vec::new();
    let mut t = 0i64;
    while t <= secs * 1000 {
        if t % 1000 == 0 {
            let ts = t as f32 / 1000.0;
            let n = start - (own_gs + tgt_gs) * ts;
            send(
                &mut rd,
                &mut tr,
                &format!(
                    "$GPRMC,1200{:02}.00,A,4830.000,N,01130.000,E,{:.1},0.0,221026,,,A",
                    (t / 1000) % 60,
                    own_gs / 0.514444
                ),
                t,
            );
            send(
                &mut rd,
                &mut tr,
                &format!(
                    "$PFLAA,3,{:.0},{:.0},{:.0},2,DD0002,180,,{:.0},0.0,1",
                    n, lat, dv, tgt_gs
                ),
                t,
            );
            let brg = lat.atan2(n).to_degrees();
            send(
                &mut rd,
                &mut tr,
                &format!(
                    "$PFLAU,1,1,2,1,3,{:.0},2,{:.0},{:.0},DD0002",
                    brg,
                    dv,
                    (n * n + lat * lat).sqrt()
                ),
                t,
            );
        }
        if t >= 3000 {
            let v = compute(&tr, &cfg, &mut st, t);
            if v.threat.dist > 30.0 && v.threat.y > 0.0 {
                out.push((t, v.kind, v.threat.bearing));
            }
        }
        t += 100;
    }
    out
}

#[test]
fn headon_hold() {
    // fast encounter: bearing passes 10..12 deg in < 2 s -> held as head-on until 12 deg
    let fr = encounter(30.0, 30.0, 50.0, 1500.0, 0.0, 24);
    let mut held = 0;
    for &(t, k, b) in &fr {
        let b = b.abs();
        if b < 9.7 {
            assert_eq!(k, ViewKind::HeadOn, "t={} brg={}", t, b);
        } else if b > 10.3 && b < 11.7 {
            assert_eq!(k, ViewKind::HeadOn, "hold t={} brg={}", t, b);
            held += 1;
        } else if b > 12.3 {
            assert_eq!(k, ViewKind::Warn, "t={} brg={}", t, b);
        }
    }
    assert!(held >= 3, "hold frames {}", held);
    assert!(fr.iter().filter(|f| f.1 == ViewKind::HeadOn).count() > 50);

    // slow encounter: bearing stays between 10 and 12 deg much longer than 2 s -> back to warn after 2 s
    let fr = encounter(3.5, 2.0, 100.0, 650.0, 0.0, 40);
    let t10 = fr
        .iter()
        .find(|f| f.2.abs() > 10.0)
        .map(|f| f.0)
        .expect("crosses 10 deg");
    let mut hold_seen = false;
    let mut warn_after = false;
    for &(t, k, b) in &fr {
        if b.abs() > 10.05 && b.abs() < 11.95 {
            if t < t10 + 1900 {
                assert_eq!(k, ViewKind::HeadOn, "t={} brg={}", t, b);
                hold_seen = true;
            } else if t > t10 + 2200 {
                assert_eq!(k, ViewKind::Warn, "t={} brg={}", t, b);
                warn_after = true;
            }
        }
    }
    assert!(hold_seen && warn_after);
}

/* ---------------- selection ---------------- */

/// Own north, four targets without velocity at bearings 30, 90, 180, -45 (ids 1..4).
fn selection_setup(rd: &mut NmeaReader, tr: &mut Traffic, t: i64, skip: Option<u32>) {
    send(rd, tr, RMC_NORTH, t);
    for &(id, b) in &[(1u32, 30.0f32), (2, 90.0), (3, 180.0), (4, -45.0)] {
        if Some(id) == skip {
            continue;
        }
        let (n, e) = (1000.0 * deg(b).cos(), 1000.0 * deg(b).sin());
        send(
            rd,
            tr,
            &format!("$PFLAA,0,{:.0},{:.0},0,2,{:06X},,,,,1", n, e, id),
            t,
        );
    }
    send(rd, tr, "$PFLAU,4,1,2,1,0,,0,,,", t);
}

#[test]
fn selection_cycling() {
    let mut rd = NmeaReader::new();
    let (mut tr, t) = northbound(&mut rd);
    selection_setup(&mut rd, &mut tr, t, None);
    let mut st = LogicState::new();
    // clockwise from straight ahead: 30, 90, 180, 315 (-45), wrap to 30
    let seq: Vec<u32> = (0..6).map(|_| st.select_step(&tr, t, true)).collect();
    assert_eq!(seq, vec![1, 2, 3, 4, 1, 2]);
    // counter-clockwise from 90: 30, -45, 180, 90
    let seq: Vec<u32> = (0..4).map(|_| st.select_step(&tr, t, false)).collect();
    assert_eq!(seq, vec![1, 4, 3, 2]);
    // counter-clockwise from straight ahead: first target to the left (-45)
    let mut st2 = LogicState::new();
    assert_eq!(st2.select_step(&tr, t, false), 4);
    // view reflects the manual selection
    let cfg = LogicConfig::DEFAULT;
    let v = compute(&tr, &cfg, &mut st2, t);
    assert!(v.sel_manual);
    assert_eq!(v.idle, IdleKind::Selected);
    assert_eq!(v.selected.unwrap().id, 4);
    assert_eq!(v.others().iter().filter(|o| o.is_selected).count(), 1);
    let s = v.selected.unwrap();
    assert!(approx(s.bearing, -45.0, 0.5) && approx(s.x, -707.1, 2.0) && approx(s.y, 707.1, 2.0));
    // no targets: nothing selected
    let mut st3 = LogicState::new();
    assert_eq!(st3.select_step(&Traffic::new(), t, true), 0);
    // display frame follows the external heading: heading 90 -> target 2 (east) straight ahead
    tr.set_attitude(90.0, 0.0, 0.0, t);
    let mut st4 = LogicState::new();
    assert_eq!(st4.select_step(&tr, t, true), 2);
    let v = compute(&tr, &cfg, &mut st4, t);
    let s = v.selected.unwrap();
    assert!(v.heading_from_attitude && approx(s.bearing, 0.0, 0.5) && approx(s.y, 1000.0, 2.0));
}

#[test]
fn follow_and_unfollow() {
    let cfg = LogicConfig::DEFAULT;
    let mut rd = NmeaReader::new();
    let (mut tr, t) = northbound(&mut rd);
    selection_setup(&mut rd, &mut tr, t, None);
    let mut st = LogicState::new();
    assert_eq!(
        st.follow_selected(&mut tr, t),
        FollowResult::NothingSelected
    );
    st.select_step(&tr, t, true);
    st.select_step(&tr, t, true); // id 2
    assert_eq!(st.follow_selected(&mut tr, t), FollowResult::Followed(2));
    assert_eq!(tr.team_id, 2);
    assert_eq!(st.sel_id, 0);
    let v = compute(&tr, &cfg, &mut st, t);
    assert_eq!(v.idle, IdleKind::Team);
    let tm = v.team.unwrap();
    assert!(tm.is_team && !tm.lost && approx(tm.bearing, 90.0, 0.5));
    assert!(v.others().iter().any(|o| o.id == 2 && o.is_team));
    // select the team mate again and press: unfollow
    st.select_step(&tr, t, true);
    assert_eq!(st.select_step(&tr, t, true), 2);
    assert_eq!(compute(&tr, &cfg, &mut st, t).idle, IdleKind::Team); // selecting the team mate keeps the team view
    assert_eq!(st.follow_selected(&mut tr, t), FollowResult::Unfollowed(2));
    assert_eq!(tr.team_id, 0);
    let v = compute(&tr, &cfg, &mut st, t);
    assert_eq!(v.team, None);
    assert_eq!(v.idle, IdleKind::Nearest);
    // switching directly to another target
    st.select_step(&tr, t, true);
    assert_eq!(st.follow_selected(&mut tr, t), FollowResult::Followed(1));
    st.select_step(&tr, t, false);
    assert_eq!(st.follow_selected(&mut tr, t), FollowResult::Followed(4));
    assert_eq!(tr.team_id, 4);
}

#[test]
fn selection_fallback_and_lost() {
    let cfg = LogicConfig::DEFAULT;
    let mut rd = NmeaReader::new();
    let (mut tr, t0) = northbound(&mut rd);
    selection_setup(&mut rd, &mut tr, t0, None);
    let mut st = LogicState::new();
    st.select_step(&tr, t0, true); // id 1
    let mut t = t0;
    // keep everything alive for 25 s; selection must end after 20 s without input, silently
    let mut ended_at = None;
    while t < t0 + 25_000 {
        t += 100;
        if t % 1000 == 0 {
            selection_setup(&mut rd, &mut tr, t, None);
        }
        let v = compute(&tr, &cfg, &mut st, t);
        assert!(!v.sel_lost);
        if since(t, t0) <= 20_000 {
            assert!(v.sel_manual && v.selected.unwrap().id == 1, "t={}", t - t0);
        } else if ended_at.is_none() {
            assert!(!v.sel_manual);
            ended_at = Some(t - t0);
        }
    }
    assert_eq!(ended_at, Some(20_100));

    // target lost: selection dropped with the event flag once
    st.select_step(&tr, t, true);
    st.select_step(&tr, t, true); // id 2
    assert_eq!(st.sel_id, 2);
    let t1 = t;
    let mut lost_events = 0;
    while t < t1 + 10_000 {
        t += 100;
        if t % 1000 == 0 {
            selection_setup(&mut rd, &mut tr, t, Some(2));
        }
        let v = compute(&tr, &cfg, &mut st, t);
        if v.sel_lost {
            lost_events += 1;
            assert!(since(t, t1) > TARGET_SHOW_MS);
            assert!(!v.sel_manual);
        }
    }
    assert_eq!(lost_events, 1);
    assert_eq!(st.sel_id, 0);
}

#[test]
fn team_memory_lost_and_found() {
    let cfg = LogicConfig::DEFAULT;
    let mut st = LogicState::new();
    let mut d = Demo::new(Scenario::Team);
    let mut tr = Traffic::new();
    tr.set_team(d.team_id());
    let mut rd = NmeaReader::new();
    let mut buf = [0u8; 2048];
    let (mut lost_seen, mut max_age, mut found_again) = (false, 0.0f32, false);
    let mut t = 0;
    while t <= d.duration_ms() {
        let n = d.step(t, &mut buf);
        feed(&mut rd, &mut tr, &buf[..n], t);
        tr.housekeeping(t);
        let v = compute(&tr, &cfg, &mut st, t);
        if let Some(tm) = v.team {
            if tm.lost {
                lost_seen = true;
                max_age = max_age.max(tm.age_s);
                assert!(tm.climb.is_some() && tm.track_rel.is_none());
            } else if lost_seen {
                found_again = true;
            }
        }
        t += 33;
    }
    println!("team: lost max age {:.1} s", max_age);
    assert!(lost_seen && found_again);
    assert!(max_age > 12.0 && max_age < 16.5);
}

/* ---------------- demo ---------------- */

/// Run one scenario start to end: returns (ok, bad, headon longest ms, warn frames, nondir seen, kinds seen).
fn run_demo(sc: Scenario) -> (u32, u32, i64, u32, bool) {
    let cfg = LogicConfig::DEFAULT;
    let mut st = LogicState::new();
    let mut d = Demo::new(sc);
    let mut tr = Traffic::new();
    tr.set_team(d.team_id());
    let mut rd = NmeaReader::new();
    let mut buf = [0u8; 2048];
    let (mut ho_start, mut ho_best) = (None, 0i64);
    let mut warn = 0;
    let mut nondir = false;
    let mut types = [false; 3];
    let mut t = 0;
    while t <= d.duration_ms() {
        let n = d.step(t, &mut buf);
        for &b in &buf[..n] {
            if let Some(m) = rd.feed(b) {
                match m {
                    NmeaMsg::Gprmc(_) => types[0] = true,
                    NmeaMsg::Pflaa(_) => types[1] = true,
                    NmeaMsg::Pflau(_) => types[2] = true,
                    _ => {}
                }
                tr.handle(&m, t);
            }
        }
        tr.housekeeping(t);
        let v = compute(&tr, &cfg, &mut st, t);
        assert!(
            !matches!(v.kind, ViewKind::NoFlarm | ViewKind::NoGps) || t < 100,
            "{:?} at {}",
            v.kind,
            t
        );
        if v.kind == ViewKind::HeadOn {
            let s = *ho_start.get_or_insert(t);
            ho_best = ho_best.max(t - s);
        } else {
            ho_start = None;
        }
        if v.kind == ViewKind::Warn || v.kind == ViewKind::HeadOn {
            warn += 1;
        }
        nondir |= v.n_nondir > 0;
        t += 33;
    }
    assert!(types.iter().all(|&x| x), "{:?}: missing sentence type", sc);
    assert!(!d.take_restart());
    (rd.ok_count, rd.bad_count, ho_best, warn, nondir)
}

#[test]
fn demo_scenarios_run() {
    for sc in Scenario::LIST {
        let (ok, bad, ho, warn, nondir) = run_demo(sc);
        println!(
            "demo {:<9}: ok={} bad={} head-on longest {:.1} s, warning frames {}, transponder {}",
            sc.name(),
            ok,
            bad,
            ho as f32 / 1000.0,
            warn,
            nondir
        );
        assert!(ok > 0);
        assert_eq!(bad, 0, "{:?}", sc);
        match sc {
            Scenario::HeadOn => assert!(ho >= 5000, "head-on only {} ms", ho),
            Scenario::Crossing => assert!(warn > 100 && ho == 0),
            Scenario::Mixed => assert!(nondir),
            _ => {}
        }
    }
}

#[test]
fn demo_all_cycles() {
    let mut d = Demo::new(Scenario::All);
    let mut tr = Traffic::new();
    let mut rd = NmeaReader::new();
    let mut buf = [0u8; 2048];
    let mut seen = vec![d.scenario()];
    let total: i64 = 40 + 30 + 70 + 40 + 60 + 60;
    let mut t = 1_000_000; // arbitrary epoch
    let end = t + total * 1000 + 10_000;
    while t < end {
        let n = d.step(t, &mut buf);
        if d.take_restart() {
            tr.clear();
            tr.set_team(d.team_id());
            seen.push(d.scenario());
        }
        feed(&mut rd, &mut tr, &buf[..n], t);
        t += 50;
    }
    assert_eq!(
        &seen[..7],
        &[
            Scenario::Crossing,
            Scenario::HeadOn,
            Scenario::Team,
            Scenario::Thermal,
            Scenario::Cruise,
            Scenario::Mixed,
            Scenario::Crossing
        ]
    );
    assert!(rd.ok_count > 1000 && rd.bad_count == 0);
    assert_eq!(flarm::demo::lookup_name(0xDDA5C3), Some("SL"));
    assert_eq!(flarm::demo::lookup_name(1), None);
}

/* ---------------- baud ---------------- */

#[test]
fn baud_scanner() {
    let mut b = BaudScanner::new(57600);
    assert_eq!(b.current(), 57600);
    assert!(!b.locked());
    assert_eq!(b.tick(0), None); // starts the dwell timer
    assert_eq!(b.tick(2500), None);
    assert_eq!(b.tick(2501), Some(115200));
    assert_eq!(b.tick(5002), Some(9600));
    // bad lines do not change the timing
    b.on_bad(5100);
    assert_eq!(b.bad_since_switch(), 1);
    assert_eq!(b.tick(7503), Some(4800));
    assert_eq!(b.tick(10004), Some(19200)); // wrap-around
    assert_eq!(b.bad_since_switch(), 0);
    // three good lines lock
    assert!(!b.on_line_ok(10100));
    assert!(!b.on_line_ok(10200));
    assert!(b.on_line_ok(10300));
    assert!(!b.on_line_ok(10400));
    assert!(b.locked());
    assert_eq!(b.tick(20_000), None);
    assert_eq!(b.current(), 19200);
    // 10 s silence: unlocked, current rate gets another 2.5 s, then the search continues
    assert_eq!(b.tick(20_401), None);
    assert!(!b.locked());
    assert_eq!(b.tick(22_901), None);
    assert_eq!(b.tick(22_902), Some(38400));
    // two lines are not enough after a switch
    b.on_line_ok(23_000);
    b.on_line_ok(23_100);
    assert!(!b.locked());
    assert_eq!(b.tick(25_403), Some(57600));
    // unknown start rate
    let b = BaudScanner::new(12345);
    assert_eq!(b.current(), BAUD_CANDIDATES[0]);
}

/// The scanner on a simulated line: data only decodes at the right rate.
#[test]
fn baud_scanner_finds_rate() {
    let flarm_baud = 115200;
    let mut b = BaudScanner::new(19200);
    let mut rd = NmeaReader::new();
    let mut rng = XorShift(99);
    let good = sentence("$PFLAU,0,1,2,1,0,,0,,,");
    let mut t = 0i64;
    let mut lock_t = None;
    while t < 30_000 {
        let bytes: Vec<u8> = if b.current() == flarm_baud {
            good.clone()
        } else {
            (0..40).map(|_| rng.next() as u8).collect()
        };
        for x in bytes {
            let (ok0, bad0) = (rd.ok_count, rd.bad_count);
            rd.feed(x);
            if rd.ok_count != ok0 && b.on_line_ok(t) && lock_t.is_none() {
                lock_t = Some(t);
            }
            if rd.bad_count != bad0 {
                b.on_bad(t);
            }
        }
        b.tick(t);
        t += 200;
    }
    assert!(b.locked());
    assert_eq!(b.current(), flarm_baud);
    let lt = lock_t.unwrap();
    assert!(lt > 7000 && lt < 8500, "locked at {}", lt);
}
