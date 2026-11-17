//! Serial NMEA port of the display
//!
//! Protocol `Larus` (XCSoar, OpenSoar, LK8000): GPS, vario, wind, attitude
//! and settings out, `$PLARS,H,..` and `$g,..` in.
//!
//! Protocol `Lxnav`: the display listens on the PDA/PC port of an LXNAV
//! main unit - MC, ballast and bugs from
//! `$LXWP2`, the circling/cruise state from `$PLXVS` - and sends nothing
//! (the main unit does not take settings from a vario, LXNAV installation
//! manual 5.2.2.7).
use crate::{
    controller::{persist, persist::set_vario_mode, Echo},
    model::{GpsState, NmeaProtocol, VarioModeControl},
    utils::ParseSlice,
    CoreController, CoreError, CoreModel, FloatToMass, FloatToPressure, FloatToSpeed,
    PersistenceId, Variant, VarioMode, STANDARD_GRAVITY,
};
use heapless::Vec;
use tfmt::uwrite;

impl CoreController {
    pub fn recv_u8(&mut self, cm: &mut CoreModel, b: u8) {
        if self.nmea_buffer.rx.recv_u8(b) {
            let _ = self.nmea_parse(cm);
        }
    }

    pub fn nmea_recv_slice(&mut self, cm: &mut CoreModel, bytes: &[u8]) {
        for b in bytes {
            self.recv_u8(cm, *b);
        }
    }

    fn nmea_parse(&mut self, cm: &mut CoreModel) -> Result<(), CoreError> {
        // check checksum
        self.nmea_buffer.rx.check()?;

        let head = self.nmea_buffer.rx.next_chunk()?;
        match cm.config.nmea_protocol {
            NmeaProtocol::Larus => match head {
                b"$PLARS" => self.nmea_parse_plars(cm),
                b"$g" => self.nmea_parse_g(cm),
                _ => Err(CoreError::ParseError),
            },
            NmeaProtocol::Lxnav => match head {
                b"$LXWP2" => self.nmea_parse_lxwp2(cm),
                b"$PLXVS" => self.nmea_parse_plxvs(cm),
                // GPS, LXWP0/1/3/5, FLARM: not needed, dropped
                _ => Err(CoreError::ParseError),
            },
        }
    }

    /// `$LXWP2,mc,ballast,bugs,polar_a,polar_b,polar_c,volume` of an LXNAV
    /// main unit: MC in m/s, ballast as overload (total mass / reference
    /// mass of the polar, 1.0 ..), bugs in percent. Empty fields are
    /// skipped; a value is only persisted when it differs from the current
    /// one, the sentence repeats every few seconds.
    fn nmea_parse_lxwp2(&mut self, cm: &mut CoreModel) -> Result<(), CoreError> {
        // MC
        let s = self.nmea_buffer.rx.next_chunk()?;
        if let Ok(mc) = f32::from_slice(s) {
            if (0.0..=9.9).contains(&mc) && (mc - cm.config.mc_cready.to_m_s()).abs() >= 0.05 {
                persist::persist_set(
                    self,
                    cm,
                    Variant::Speed(mc.m_s()),
                    PersistenceId::McCready,
                    Echo::Can,
                );
            }
        }
        // ballast: overload factor -> water ballast of our glider data; the
        // polar scaling sqrt(mass / reference) then equals the one of the main unit,
        // whatever the empty and pilot masses are in the two devices
        let s = self.nmea_buffer.rx.next_chunk()?;
        if let Ok(overload) = f32::from_slice(s) {
            if (1.0..=2.0).contains(&overload) {
                let bgd = &cm.glider_data.basic_glider_data;
                let total = overload * bgd.reference_weight;
                let water = (total - bgd.empty_mass - cm.glider_data.pilot_weight.to_kg())
                    .max(0.0)
                    .min(bgd.max_ballast);
                if (water - cm.glider_data.water_ballast.to_kg()).abs() >= 0.5 {
                    persist::persist_set(
                        self,
                        cm,
                        Variant::Mass(water.kg()),
                        PersistenceId::WaterBallast,
                        Echo::Can,
                    );
                }
            }
        }
        // bugs in percent (0, 5, 10 ..), our range ends at 50 %
        let s = self.nmea_buffer.rx.next_chunk()?;
        if let Ok(bugs) = f32::from_slice(s) {
            if (0.0..=100.0).contains(&bugs) {
                let factor = 1.0 + bugs.min(50.0) / 100.0;
                if (factor - cm.glider_data.bugs()).abs() >= 0.005 {
                    persist::persist_set(
                        self,
                        cm,
                        Variant::F32(factor),
                        PersistenceId::Bugs,
                        Echo::Can,
                    );
                }
            }
        }
        Ok(())
    }

    /// `$PLXVS,oat,mode,voltage,..`: mode 0 = circling (vario), 1 = cruise
    /// (speed to fly). Taken over like `$g,s0/s1` from XCSoar, so it needs
    /// Mode Switch = NMEA.
    fn nmea_parse_plxvs(&mut self, cm: &mut CoreModel) -> Result<(), CoreError> {
        let _oat = self.nmea_buffer.rx.next_chunk()?;
        match self.nmea_buffer.rx.next_chunk()? {
            b"0" => set_vario_mode(cm, self, VarioMode::Vario, VarioModeControl::Nmea),
            b"1" => set_vario_mode(cm, self, VarioMode::SpeedToFly, VarioModeControl::Nmea),
            _ => return Err(CoreError::ParseError),
        }
        Ok(())
    }

    fn nmea_parse_g(&mut self, cm: &mut CoreModel) -> Result<(), CoreError> {
        match self.nmea_buffer.rx.next_chunk()? {
            b"s0" => set_vario_mode(cm, self, VarioMode::Vario, VarioModeControl::Nmea),
            b"s1" => set_vario_mode(cm, self, VarioMode::SpeedToFly, VarioModeControl::Nmea),
            b"rp" => self.key_action(cm, crate::KeyEvent::BtnEnc),
            b"rl" => self.key_action(cm, crate::KeyEvent::BtnEncS3),
            b"ru" => self.key_action(cm, crate::KeyEvent::Rotary2Left),
            b"rd" => self.key_action(cm, crate::KeyEvent::Rotary2Right),
            _ => return Err(CoreError::ParseError),
        }
        Ok(())
    }

    fn nmea_parse_plars(&mut self, cm: &mut CoreModel) -> Result<(), CoreError> {
        fn in_range(val: f32, lower: f32, upper: f32) -> Result<f32, CoreError> {
            if val >= lower && val <= upper {
                Ok(val)
            } else {
                Err(CoreError::ParseError)
            }
        }

        self.nmea_buffer.rx.compare_chunk(b"H")?;

        let cmd: Vec<u8, 10> = Vec::from_slice(self.nmea_buffer.rx.next_chunk()?)
            .map_err(|_| CoreError::ParseError)?;

        let s = self.nmea_buffer.rx.next_chunk()?;
        let val = f32::from_slice(s)?;

        match cmd.as_slice() {
            b"MC" => {
                let val = in_range(val, 0.0, 9.9)?.m_s();
                persist::persist_set(
                    self,
                    cm,
                    Variant::Speed(val),
                    PersistenceId::McCready,
                    Echo::Can,
                )
            }
            b"BAL" => {
                cm.glider_data
                    .set_ballast_fraction(in_range(val, 0.00, 1.00)?);
                let val = cm.glider_data.water_ballast;
                persist::persist_set(
                    self,
                    cm,
                    Variant::Mass(val),
                    PersistenceId::WaterBallast,
                    Echo::Can,
                );
            }
            b"BUGS" => {
                let val = 1.0 + in_range(val, 0.0, 50.0)? / 100.0;
                persist::persist_set(self, cm, Variant::F32(val), PersistenceId::Bugs, Echo::Can);
            }
            b"QNH" => {
                let val = in_range(val, 900.0, 1100.0)?.hpa();
                persist::persist_set(
                    self,
                    cm,
                    Variant::Pressure(val),
                    PersistenceId::Qnh,
                    Echo::Can,
                );
            }
            b"CIR" => match val as i32 {
                0 => set_vario_mode(cm, self, VarioMode::SpeedToFly, VarioModeControl::Nmea),
                1 => set_vario_mode(cm, self, VarioMode::Vario, VarioModeControl::Nmea),
                _ => return Err(CoreError::ParseError),
            },
            _ => return Err(CoreError::ParseError),
        }
        Ok(())
    }

    pub fn nmea_send_config_data(&mut self, id: PersistenceId) {
        // no error if deque is full
        match id {
            PersistenceId::Bugs
            | PersistenceId::McCready
            | PersistenceId::WaterBallast
            | PersistenceId::Qnh
            | PersistenceId::VarioMode => {
                let _ = self.nmea_buffer.pers_id.push_back(id);
            }
            _ => (),
        }
    }

    pub fn nmea_cyclic_1s(&mut self) {
        let _ = self.nmea_buffer.to_send.extend_from_slice(&[0, 1, 2, 3, 4]);
    }

    pub fn nmea_cyclic_200ms(&mut self) {
        let _ = self.nmea_buffer.to_send.extend_from_slice(&[5, 6, 7]);
    }

    pub fn nmea_next(&mut self, cm: &mut CoreModel) -> Option<&[u8]> {
        if cm.config.nmea_protocol == NmeaProtocol::Lxnav {
            // receive only behind an LXNAV main unit
            self.nmea_buffer.to_send.clear();
            self.nmea_buffer.pers_id.clear();
            return None;
        }
        if !self.nmea_buffer.pers_id.is_empty() {
            let id = self.nmea_buffer.pers_id.pop_front().unwrap();
            return self.nmea_plars(cm, id);
        }
        let roll_pitch_available = cm.sensor.roll_pitch_available();
        while let Some(idx) = self.nmea_buffer.to_send.pop() {
            if idx == 6 && !roll_pitch_available {
                continue;
            }

            return match idx {
                // rarely sent
                0 => Some(self.nmea_gprmc(cm)),
                1 => Some(self.nmea_gpgga(cm)),
                2 => Some(self.nmea_plarw(cm, true)),
                3 => Some(self.nmea_plard(cm)),
                4 => Some(self.nmea_plarb(cm)),

                // often sent
                5 => Some(self.nmea_plarw(cm, false)),
                6 => Some(self.nmea_plara(cm)),
                7 => Some(self.nmea_plarv(cm)),

                // not known
                _ => None,
            };
        }
        None
    }

    fn nmea_gprmc(&mut self, cm: &mut CoreModel) -> &[u8] {
        self.nmea_buffer.tx.reset();
        let _ = uwrite!(
            self.nmea_buffer.tx,
            "$GPRMC,{:n},A,{:n},{:n},{:.1},{:.1},{:n},,,A",
            cm.sensor.gps_date_time.time(),
            cm.sensor.gps_lat,
            cm.sensor.gps_lon,
            cm.sensor.gps_ground_speed.to_kt(),
            cm.sensor.gps_track.to_degrees(),
            cm.sensor.gps_date_time.date(),
        );
        self.nmea_buffer.tx.finish()
    }

    fn nmea_gpgga(&mut self, cm: &mut CoreModel) -> &[u8] {
        self.nmea_buffer.tx.reset();
        let gps_quality_indicator = match cm.sensor.gps_state {
            GpsState::PosAvail => 1,
            GpsState::HeadingAvail => 2,
            _ => 0,
        };
        let _ = uwrite!(
            self.nmea_buffer.tx,
            "$GPGGA,{:n},{:n},{:n},{},{},1.0,{:.1},M,{:.1},M,,",
            cm.sensor.gps_date_time.time(),
            cm.sensor.gps_lat,
            cm.sensor.gps_lon,
            gps_quality_indicator,
            cm.sensor.gps_sats,
            cm.sensor.gps_altitude.to_m(),
            cm.sensor.gps_geo_seperation.to_m(),
        );
        self.nmea_buffer.tx.finish()
    }

    fn nmea_plara(&mut self, cm: &mut CoreModel) -> &[u8] {
        self.nmea_buffer.tx.reset();
        let _ = uwrite!(
            self.nmea_buffer.tx,
            "$PLARA,{:.1},{:.1},{:.1}",
            cm.sensor.euler_roll.to_degrees(),
            cm.sensor.euler_pitch.to_degrees(),
            cm.sensor.euler_yaw.to_degrees(),
        );
        self.nmea_buffer.tx.finish()
    }

    fn nmea_plarb(&mut self, cm: &mut CoreModel) -> &[u8] {
        self.nmea_buffer.tx.reset();
        let _ = uwrite!(
            self.nmea_buffer.tx,
            "$PLARB,{:.2}",
            cm.device.supply_voltage,
        );
        self.nmea_buffer.tx.finish()
    }

    fn nmea_plard(&mut self, cm: &mut CoreModel) -> &[u8] {
        self.nmea_buffer.tx.reset();
        let _ = uwrite!(
            self.nmea_buffer.tx,
            "$PLARD,{:.2},M",
            cm.sensor.density.to_g_m3(),
        );
        self.nmea_buffer.tx.finish()
    }

    fn nmea_plars(&mut self, cm: &mut CoreModel, id: PersistenceId) -> Option<&[u8]> {
        self.nmea_buffer.tx.reset();
        let _ = match id {
            PersistenceId::McCready => uwrite!(
                self.nmea_buffer.tx,
                "$PLARS,L,MC,{:.1}",
                cm.config.mc_cready.to_m_s()
            ),
            PersistenceId::WaterBallast => uwrite!(
                self.nmea_buffer.tx,
                "$PLARS,L,BAL,{:.3}",
                cm.glider_data.ballast_fraction(),
            ),
            PersistenceId::Bugs => uwrite!(
                self.nmea_buffer.tx,
                "$PLARS,L,BUGS,{:.0}",
                (cm.glider_data.bugs() - 1.0) * 100.0
            ),
            PersistenceId::Qnh => uwrite!(
                self.nmea_buffer.tx,
                "$PLARS,L,QNH,{:.1}",
                cm.sensor.pressure_altitude.qnh().to_hpa()
            ),
            PersistenceId::VarioMode => uwrite!(
                self.nmea_buffer.tx,
                "$PLARS,L,CIR,{}",
                if cm.control.vario_mode == VarioMode::Vario {
                    1
                } else {
                    0
                }
            ),
            _ => return None,
        };
        Some(self.nmea_buffer.tx.finish())
    }

    fn nmea_plarv(&mut self, cm: &mut CoreModel) -> &[u8] {
        self.nmea_buffer.tx.reset();
        let _ = uwrite!(
            self.nmea_buffer.tx,
            "$PLARV,{:.2},{:.2},{:.0},{:.0},{:.2}",
            cm.sensor.climb_rate.to_m_s(),
            cm.sensor.average_climb_rate.to_m_s(),
            cm.sensor.pressure_altitude.qne_altitude().to_m(),
            cm.sensor.airspeed.tas().to_km_h(),
            cm.sensor.g_force.to_m_s2() / STANDARD_GRAVITY,
        );
        self.nmea_buffer.tx.finish()
    }

    fn nmea_plarw(&mut self, cm: &mut CoreModel, average: bool) -> &[u8] {
        self.nmea_buffer.tx.reset();
        let (kind, wind) = if average {
            ("A", cm.sensor.average_wind)
        } else {
            ("I", cm.sensor.wind_vector)
        };
        let _ = uwrite!(
            self.nmea_buffer.tx,
            "$PLARW,{:.0},{:.0},{},A",
            wind.angle().to_degrees(),
            wind.speed().to_km_h(),
            kind,
        );
        self.nmea_buffer.tx.finish()
    }
}

pub fn nmea_cyclic_200ms(cm: &mut CoreModel, cc: &mut CoreController) {
    // demo values never go to the navigation computer
    if cm.control.demo.active() {
        return;
    }
    cc.nmea_cyclic_200ms();
}

#[cfg(test)]
mod lxnav_protocol_tests {
    use crate::{
        model::{NmeaProtocol, VarioModeControl},
        test_harness, FloatToMass, FloatToSpeed, VarioMode,
    };

    /// the NMEA checksum of the body, as a string "$body*CS\r\n"
    fn sentence(body: &str) -> heapless::String<90> {
        let cs = body.bytes().fold(0u8, |a, b| a ^ b);
        let mut s: heapless::String<90> = heapless::String::new();
        let _ = tfmt::uwrite!(&mut s, "${}*{:02X}\r\n", body, cs);
        s
    }

    fn lx_setup(cm: &mut crate::CoreModel) {
        cm.config.nmea_protocol = NmeaProtocol::Lxnav;
        // ASG 29 like test glider: reference 450 kg, empty 280, pilot 90,
        // max water 121
        cm.glider_data.basic_glider_data.reference_weight = 450.0;
        cm.glider_data.basic_glider_data.empty_mass = 280.0;
        cm.glider_data.basic_glider_data.max_ballast = 121.0;
        cm.glider_data.pilot_weight = 90.0.kg();
        cm.glider_data.water_ballast = 0.0.kg();
        cm.glider_data.set_bugs(1.0);
        cm.config.mc_cready = 0.0.m_s();
    }

    #[test]
    fn lxwp2_sets_mc_ballast_bugs() {
        let (mut cm, mut cc, _tx) = test_harness!();
        lx_setup(&mut cm);
        // overload 1.2 -> 540 kg total -> 170 kg water, capped at 121
        cc.nmea_recv_slice(
            &mut cm,
            sentence("LXWP2,2.5,1.2,10,1.21,-2.13,2.8,50").as_bytes(),
        );
        assert!((cm.config.mc_cready.to_m_s() - 2.5).abs() < 0.01);
        assert!((cm.glider_data.water_ballast.to_kg() - 121.0).abs() < 0.01);
        assert!((cm.glider_data.bugs() - 1.10).abs() < 0.001);
        // overload 1.0 -> 450 kg total -> 80 kg water
        cc.nmea_recv_slice(
            &mut cm,
            sentence("LXWP2,0.0,1.0,0,1.21,-2.13,2.8,50").as_bytes(),
        );
        assert!((cm.config.mc_cready.to_m_s() - 0.0).abs() < 0.01);
        assert!((cm.glider_data.water_ballast.to_kg() - 80.0).abs() < 0.01);
        assert!((cm.glider_data.bugs() - 1.0).abs() < 0.001);
    }

    #[test]
    fn lxwp2_empty_fields_keep_values() {
        let (mut cm, mut cc, _tx) = test_harness!();
        lx_setup(&mut cm);
        cc.nmea_recv_slice(&mut cm, sentence("LXWP2,1.5,1.1,5,,,,").as_bytes());
        assert!((cm.config.mc_cready.to_m_s() - 1.5).abs() < 0.01);
        cc.nmea_recv_slice(&mut cm, sentence("LXWP2,,,,,,,").as_bytes());
        assert!((cm.config.mc_cready.to_m_s() - 1.5).abs() < 0.01);
        assert!((cm.glider_data.water_ballast.to_kg() - 125.0_f32.min(121.0)).abs() < 1.0);
        assert!((cm.glider_data.bugs() - 1.05).abs() < 0.001);
    }

    #[test]
    fn lxwp2_ignored_in_larus_protocol() {
        let (mut cm, mut cc, _tx) = test_harness!();
        lx_setup(&mut cm);
        cm.config.nmea_protocol = NmeaProtocol::Larus;
        cc.nmea_recv_slice(
            &mut cm,
            sentence("LXWP2,2.5,1.2,10,1.21,-2.13,2.8,50").as_bytes(),
        );
        assert!(cm.config.mc_cready.to_m_s().abs() < 0.01);
        // and the Larus sentences are not parsed behind an LXNAV main unit
        cm.config.nmea_protocol = NmeaProtocol::Lxnav;
        cc.nmea_recv_slice(&mut cm, sentence("PLARS,H,MC,3.0").as_bytes());
        assert!(cm.config.mc_cready.to_m_s().abs() < 0.01);
    }

    #[test]
    fn plxvs_switches_the_mode_with_nmea_control() {
        let (mut cm, mut cc, _tx) = test_harness!();
        lx_setup(&mut cm);
        cm.control.vario_mode_control = VarioModeControl::Nmea;
        cm.control.vario_mode = VarioMode::Vario;
        cc.nmea_recv_slice(&mut cm, sentence("PLXVS,23.1,1,12.3,").as_bytes());
        assert!(cm.control.vario_mode == VarioMode::SpeedToFly);
        cc.nmea_recv_slice(&mut cm, sentence("PLXVS,,0,,").as_bytes());
        assert!(cm.control.vario_mode == VarioMode::Vario);
        // not with the automatic switch
        cm.control.vario_mode_control = VarioModeControl::Auto;
        cc.nmea_recv_slice(&mut cm, sentence("PLXVS,23.1,1,12.3,").as_bytes());
        assert!(cm.control.vario_mode == VarioMode::Vario);
    }

    #[test]
    fn nothing_is_sent_behind_an_lx() {
        let (mut cm, mut cc, _tx) = test_harness!();
        lx_setup(&mut cm);
        cc.nmea_cyclic_1s();
        cc.nmea_cyclic_200ms();
        cc.nmea_send_config_data(crate::PersistenceId::McCready);
        assert!(cc.nmea_next(&mut cm).is_none());
        cm.config.nmea_protocol = NmeaProtocol::Larus;
        cc.nmea_cyclic_200ms();
        assert!(cc.nmea_next(&mut cm).is_some());
    }
}

/*#[cfg(test)]
mod tests {
    use crate::{
        model::GpsState, utils::tests::cores, AirSpeed, Coord, FloatToDensity, FloatToLength,
        FloatToMass, FloatToPressure, FloatToSpeed, Latitude, Longitude, WindVector,
    };
    use embedded_graphics::geometry::AngleUnit;

    #[test]
    fn gpgga() {
        let (mut cm, mut cc) = cores();
        cm.sensor
            .gps_date_time
            .set_date_time(2023, 06, 23, 12, 05, 20);
        cm.sensor.gps_lon = Longitude(Coord(-0.1498276674644056));
        cm.sensor.gps_lat = Latitude(Coord(-0.8672530930250163));
        cm.sensor.gps_state = GpsState::HeadingAvail;
        cm.sensor.gps_sats = 23;
        cm.sensor.gps_altitude = 2745.9.m();
        cm.sensor.gps_geo_seperation = 12.3.m();

        let s = cc.nmea_gpgga(&mut cm);
        assert_eq!(
            s,
            b"$GPGGA,120520.00,4941.39652,S,835.06958,W,2,23,1.0,2745.9,M,12.3,M,,*56\r\n"
        );
    }

    #[test]
    fn gprmc() {
        let (mut cm, mut cc) = cores();
        cm.sensor
            .gps_date_time
            .set_date_time(2023, 06, 23, 12, 05, 20);
        cm.sensor.gps_lon = Longitude(Coord(0.1498276674644056));
        cm.sensor.gps_lat = Latitude(Coord(0.8672530930250163));
        cm.sensor.gps_state = GpsState::HeadingAvail;
        cm.sensor.gps_ground_speed = 123.4.kt();
        cm.sensor.gps_track = 321.4_f32.deg();

        let s = cc.nmea_gprmc(&mut cm);
        assert_eq!(
            s,
            b"$GPRMC,120520.00,A,4941.39652,N,835.06958,E,123.4,321.4,230623,,,A*53\r\n"
        );
    }

    #[test]
    fn plara() {
        let (mut cm, mut cc) = cores();
        cm.sensor.euler_roll = 123.4_f32.deg();
        cm.sensor.euler_pitch = 98.7_f32.deg();
        cm.sensor.euler_yaw = 12.3_f32.deg();
        let s = cc.nmea_plara(&mut cm);
        assert_eq!(s, b"$PLARA,123.4,98.7,12.3*4E\r\n");
    }

    #[test]
    fn plarb() {
        let (mut cm, mut cc) = cores();
        cm.device.supply_voltage = 13.12;
        let s = cc.nmea_plarb(&mut cm);
        assert_eq!(s, b"$PLARB,13.12*4E\r\n");
    }

    #[test]
    fn plard() {
        let (mut cm, mut cc) = cores();
        cm.sensor.density = 922.54.g_m3();
        let s = cc.nmea_plard(&mut cm);
        assert_eq!(s, b"$PLARD,922.54,M*10\r\n");
    }

    #[test]
    fn plars() {
        let (mut cm, mut cc) = cores();
        cm.config.mc_cready = 1.7.m_s();
        cm.config.glider_idx = 105;
        cm.glider_data.basic_glider_data.empty_mass = 295.0;
        cm.glider_data.pilot_weight = 90.0.kg();
        cm.glider_data.water_ballast = 100.0.kg();
        cm.glider_data.bugs = 1.23;
        cm.sensor.pressure_altitude.set_qnh(1031.37.hpa());

        let s = cc.nmea_plars(&mut cm, crate::PersistenceId::McCready);
        assert_eq!(s.unwrap(), b"$PLARS,L,MC,1.7*1A\r\n");

        let s = cc.nmea_plars(&mut cm, crate::PersistenceId::WaterBallast);
        assert_eq!(s.unwrap(), b"$PLARS,L,BAL,0.826*51\r\n");

        let s = cc.nmea_plars(&mut cm, crate::PersistenceId::Bugs);
        assert_eq!(s.unwrap(), b"$PLARS,L,BUGS,23*3E\r\n");

        let s = cc.nmea_plars(&mut cm, crate::PersistenceId::Qnh);
        assert_eq!(s.unwrap(), b"$PLARS,L,QNH,1031.4*72\r\n");
    }

    #[test]
    fn plarv() {
        let (mut cm, mut cc) = cores();
        cm.sensor.climb_rate = 2.50.m_s();
        cm.sensor.average_climb_rate = 1.25.m_s();
        cm.sensor
            .pressure_altitude
            .set_static_pressure(97_717.0_f32.pa());
        cm.sensor.airspeed = AirSpeed::from_tas_at_nn(111.1.km_h());
        let s = cc.nmea_plarv(&mut cm);
        assert_eq!(s, b"$PLARV,2.50,1.25,305,111*5F\r\n");
    }

    #[test]
    fn plarw() {
        let (mut cm, mut cc) = cores();
        cm.sensor.average_wind = WindVector::new(45.6.km_h(), 321.0_f32.deg());
        let s = cc.nmea_plarw(&mut cm, true);
        assert_eq!(s, b"$PLARW,321,46,A,A*6A\r\n");

        cm.sensor.wind_vector = WindVector::new(45.6.km_h(), 321.0_f32.deg());
        let s = cc.nmea_plarw(&mut cm, false);
        assert_eq!(s, b"$PLARW,321,46,I,A*62\r\n");
    }

    /*fn cores() -> (CoreModel, CoreController) {
        let (p_tx_frames, _c_tx_frames) = {
            static mut Q_TX_FRAMES: QTxFrames<MAX_TX_FRAMES> = Queue::new();
            // Note: unsafe is ok here, because [heapless::spsc] queue protects against UB
            unsafe { Q_TX_FRAMES.split() }
        };

        // This queue routes the StorageItems from the controller to the idle loop.
        let (p_idle_events, _c_idle_events) = {
            static mut Q_IDLE_EVENTS: QIdleEvents = Queue::new();
            // Note: unsafe is ok here, because [heapless::spsc] queue protects against UB
            unsafe { Q_IDLE_EVENTS.split() }
        };

        let mut model = CoreModel::new(1234_u32, HW_VERSION, SW_VERSION);
        let controller = CoreController::new(&mut model, p_idle_events, p_tx_frames);
        (model, controller)
    }*/
}*/
