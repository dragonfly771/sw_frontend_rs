use eeprom::ADR_USER_PROFILE;
use heapless::spsc::{Consumer, Producer, Queue};
use num::clamp;

use crate::{eeprom::PAGE_SIZE, CoreError, PersistenceId, Variant};

#[cfg(feature = "eeprom_size_8192")]
pub mod eeprom {
    // size of eeprom
    pub const SIZE: u32 = 8192;
    // size of a page
    pub const PAGE_SIZE: u32 = 32;

    // Fixed Address Storage

    // address, where magic number is stored 8 bytes
    pub const ADR_IDENTIFICATION_BLOCK: u32 = 0;
    // address, where active user profile is stored
    pub const ADR_USER_PROFILE: u32 = 8;
    // address, where reset reason is stored
    pub const ADR_RESET_REASON: u32 = 9;
    //...

    // start adress of data allocation table
    pub const ADR_DAT: u32 = 32;
    // len of DAT in bytes
    pub const DAT_LEN: u32 = SIZE / 8 / 4;
    // start address of stored data
    pub const ADR_DATA_STORAGE: u32 = ADR_DAT + DAT_LEN;
    // maximum of possible items to store in eeprom
    pub const MAX_ITEM_COUNT: u32 = (SIZE - ADR_DATA_STORAGE) / 4;
    // magic number to identify, if eeprom is initialized
    pub const MAGIC: [u8; 8] = [0x1e, 0xf9, 0xb4, 0xaf, 0x22, 0xe1, 0xe5, 0xeb];
}

pub enum EepromTopic {
    ConfigValues,
}

pub const MAX_USER_VALUES: u32 = 256;

const MAX_PERSISTENCE_ITEMS: usize = 20;
pub type QPersistenceItems = Queue<PersistenceItem, MAX_PERSISTENCE_ITEMS>;
pub type PPersistenceItems = Producer<'static, PersistenceItem, MAX_PERSISTENCE_ITEMS>;
pub type CPersistenceItems = Consumer<'static, PersistenceItem, MAX_PERSISTENCE_ITEMS>;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PersistenceItem {
    pub id: PersistenceId,
    pub data: [u8; 4],
}

impl PersistenceItem {
    pub fn do_not_store() -> Self {
        PersistenceItem {
            id: PersistenceId::DoNotStore,
            data: [0, 0, 0, 0],
        }
    }

    pub fn from_array_4u8(id: PersistenceId, data: [u8; 4]) -> Self {
        PersistenceItem { id, data }
    }

    pub fn from_bool(id: PersistenceId, value: bool) -> Self {
        let data: u32 = if value { 1 } else { 0 };
        PersistenceItem {
            id,
            data: data.to_le_bytes(),
        }
    }

    pub fn from_i8(id: PersistenceId, value: i8) -> Self {
        PersistenceItem {
            id,
            data: (value as i32).to_le_bytes(),
        }
    }

    pub fn from_i32(id: PersistenceId, value: i32) -> Self {
        PersistenceItem {
            id,
            data: value.to_le_bytes(),
        }
    }

    pub fn from_u8(id: PersistenceId, value: u8) -> Self {
        PersistenceItem {
            id,
            data: (value as u32).to_le_bytes(),
        }
    }

    pub fn from_u16(id: PersistenceId, value: u16) -> Self {
        PersistenceItem {
            id,
            data: (value as u32).to_le_bytes(),
        }
    }

    pub fn from_u32(id: PersistenceId, value: u32) -> Self {
        PersistenceItem {
            id,
            data: value.to_le_bytes(),
        }
    }

    pub fn from_f32(id: PersistenceId, value: f32) -> Self {
        PersistenceItem {
            id,
            data: value.to_le_bytes(),
        }
    }

    pub fn from_variant(id: PersistenceId, variant: Variant) -> Self {
        match variant {
            Variant::Bool(bool) => Self::from_bool(id, bool),
            Variant::I8(i8) => Self::from_i8(id, i8),
            Variant::I32(i32) => Self::from_i32(id, i32),
            Variant::F32(f32) => Self::from_f32(id, f32),
            Variant::U8(u8) => Self::from_u8(id, u8),
            Variant::U32(u32) => Self::from_u32(id, u32),

            Variant::Date(date) => Self::from_array_4u8(id, date.as_array_4u8()),
            Variant::Mass(mass) => Self::from_f32(id, mass.to_kg()),
            Variant::Pressure(pressure) => Self::from_f32(id, pressure.to_hpa()),
            Variant::Speed(speed) => Self::from_f32(id, speed.to_m_s()),
        }
    }

    pub fn to_4u8(&self) -> [u8; 4] {
        self.data
    }

    pub fn to_bool(&self) -> bool {
        i32::from_le_bytes(self.data) == 1
    }

    pub fn to_i8(&self) -> i8 {
        i32::from_le_bytes(self.data) as i8
    }

    pub fn to_i32(&self) -> i32 {
        i32::from_le_bytes(self.data)
    }

    pub fn to_u8(&self) -> u8 {
        i32::from_le_bytes(self.data) as u8
    }

    pub fn to_u16(&self) -> u16 {
        i32::from_le_bytes(self.data) as u16
    }

    pub fn to_u32(&self) -> u32 {
        u32::from_le_bytes(self.data)
    }

    pub fn to_f32(&self) -> Result<f32, CoreError> {
        let result = f32::from_le_bytes(self.data);
        if result.is_normal() || result == 0.0 {
            Ok(result)
        } else {
            Err(CoreError::ConversionError)
        }
    }
}

pub trait EepromTrait {
    /// Write a single byte in an address.
    ///
    /// After writing a byte, the EEPROM enters an internally-timed write cycle
    /// to the nonvolatile memory.
    /// During this time all inputs are disabled and the EEPROM will not
    /// respond until the write is complete.
    fn write_byte(&mut self, address: u32, data: u8) -> Result<(), CoreError>;

    /// Write up to a page starting in an address.
    ///
    /// The maximum amount of data that can be written depends on the page
    /// size of the device and its overall capacity. If too much data is passed,
    /// the error `Error::TooMuchData` will be returned.
    ///
    /// After writing a byte, the EEPROM enters an internally-timed write cycle
    /// to the nonvolatile memory.
    /// During this time all inputs are disabled and the EEPROM will not
    /// respond until the write is complete.
    fn write_page(&mut self, address: u32, data: &[u8]) -> Result<(), CoreError>;

    /// Read a single byte from an address.
    fn read_byte(&mut self, address: u32) -> Result<u8, CoreError>;

    /// Read starting in an address as many bytes as necessary to fill the data array provided.
    fn read_data(&mut self, address: u32, data: &mut [u8]) -> Result<(), CoreError>;

    /// Clears the dat (data allocation table) and other data
    fn clear_all_data(&mut self) -> Result<(), CoreError> {
        let mut address = eeprom::ADR_DAT;
        let data = [0_u8; 8];
        while address < (eeprom::ADR_DAT + eeprom::DAT_LEN) {
            self.write_page(address, &data)?;
            address += 8;
        }
        self.write_byte(ADR_USER_PROFILE, 0)
    }

    /// check magic number, if not ok, eeprom will be deleted
    fn check_magic(&mut self) -> Result<(), CoreError> {
        let mut magic = [0_u8; 8];
        self.read_data(eeprom::ADR_IDENTIFICATION_BLOCK, &mut magic)?;
        if magic != eeprom::MAGIC {
            // Write magic number
            self.write_page(eeprom::ADR_IDENTIFICATION_BLOCK, &eeprom::MAGIC)?;
            self.clear_all_data()?;
        }
        Ok(())
    }
}

pub struct Eeprom<S>
where
    S: EepromTrait,
{
    user_profile: u8,
    eeprom: S,
    is_unique: fn(PersistenceId) -> bool,
}

/// Store configuration data in an EEPROM
///
/// The storage of configuration and other data, which should bridge a power failure, is done in an
/// EEPROM, which is connected via an i2c interface.  Conceptually, the memory area is divided into
/// three areas:
///
/// - Identification of the initialization with a signature (Identification Block).
/// - Table of contents (data allocation table)
/// - Data storage
///
/// The data are divided into four-byte blocks in the data store. They are addressed with Ids,
/// where each Id addresses one such block. Each ID is assigned to a bit in the table of contents.
///
/// When a block is written, the corresponding bit in the table of contents is written first and
/// then the block. When reading it is the other way round. First it is checked whether the
/// corresponding ID is stored. If this is not the case, an error message is issued, otherwise
/// the block is returned.
///
/// Another function is the possibility to define topic areas and to assign them to Id areas.
/// An iterator makes it possible to search out all stored data of such a topic area. This can be
/// useful to load all initialization data, log eventc, etc..
///
/// Summarizing features of the solution:
/// - Individual data is stored one by one
/// - When changes are made, only these are written to the EEPROM
/// - The solution is extensible and version independent - new data points can always be added
///   without losing the old data
/// - The solution is independent of a specific EEPROM type.
/// - At the moment it is limited to data up to 4 bytes, but can be extended to data with more
///   blocks, if needed.
/// - The blocks of this structure are well suited to be transported via queues.
/// - The data to be stored must be defined individually
///
#[allow(dead_code)]
impl<S> Eeprom<S>
where
    S: EepromTrait,
{
    /// Create a Persistence Instance
    pub fn new(mut eeprom: S, is_unique: fn(PersistenceId) -> bool) -> Result<Self, CoreError> {
        eeprom.check_magic()?;

        let user_profile = eeprom.read_byte(ADR_USER_PROFILE)?;
        let user_profile = num::clamp(user_profile, 0, 3);

        Ok(Eeprom {
            eeprom,
            user_profile,
            is_unique,
        })
    }

    /// Write a PersistentItem into the data store
    ///
    /// The data is stored at the desired location defined by the ID. An entry is made in the data
    /// allocation table (DAT), if desired (dat_bit in PersitentItem).
    pub fn write_item(&mut self, item: PersistenceItem) -> Result<(), CoreError> {
        match item.id {
            PersistenceId::DoNotStore => Ok(()),
            PersistenceId::DeleteAll => self.eeprom.clear_all_data(),
            PersistenceId::UserProfile => {
                self.user_profile = clamp(item.data[0], 0, 3);
                self.eeprom
                    .write_byte(eeprom::ADR_USER_PROFILE, self.user_profile)
            }
            _ => {
                let address = self.item_address(item.id);
                self.set_id(item.id)?;
                self.eeprom.write_page(address, &item.data)
            }
        }
    }

    /// Read data from storage - do not check the DAT
    pub fn read_item_unchecked(&mut self, id: PersistenceId) -> Result<PersistenceItem, CoreError> {
        let address = self.item_address(id);
        let mut data = [0_u8; 4];
        self.eeprom
            .read_data(address, &mut data)
            .map_err(|_| CoreError::EepromOrI2c1)?;
        Ok(PersistenceItem { id, data })
    }

    /// Read data from storage - return error if DAT bit is not set
    pub fn read_item(&mut self, id: PersistenceId) -> Result<PersistenceItem, CoreError> {
        if self.test_id(id) {
            self.read_item_unchecked(id)
        } else {
            Err(CoreError::NoItemAvailable)
        }
    }

    /// Returns an iterator to the desired topic area
    pub fn iter_over(&mut self, p_type: EepromTopic) -> PersistenceIterator<'_, S> {
        let _ = self.set_user_profile();
        let (start_id, end_id) = match p_type {
            EepromTopic::ConfigValues => (0, PersistenceId::LastItem as u16),
        };
        PersistenceIterator::new(start_id, end_id, self)
    }

    /// Delete all items of the given list
    pub fn delete_items_list(&mut self, items_list: &[PersistenceId]) -> Result<(), CoreError> {
        for item_id in items_list {
            self.clear_id(*item_id)?;
        }
        Ok(())
    }

    pub fn restore_standard(&mut self) -> Result<(), CoreError> {
        if self.user_profile < 1 || self.user_profile > 3 {
            return Err(CoreError::OutOfRange);
        }

        // copy DAT
        let src = eeprom::ADR_DAT;
        let dst = eeprom::ADR_DAT + (self.user_profile as u32 * MAX_USER_VALUES) / 8;
        let len = MAX_USER_VALUES / 8;
        self.copy(src, dst, len)?;

        // copy Data
        let src = eeprom::ADR_DATA_STORAGE;
        let dst = eeprom::ADR_DATA_STORAGE + (self.user_profile as u32 * MAX_USER_VALUES) * 4;
        let len = MAX_USER_VALUES * 4;
        self.copy(src, dst, len)?;

        Ok(())
    }

    fn copy(&mut self, src: u32, dst: u32, len: u32) -> Result<(), CoreError> {
        let mut data = [0_u8; PAGE_SIZE as usize];

        let mut chunk_size = PAGE_SIZE - dst % PAGE_SIZE;
        let mut idx = 0;

        while idx < len {
            if chunk_size > len - idx {
                chunk_size = len - idx
            }

            self.eeprom
                .read_data(src + idx, &mut data[..chunk_size as usize])?;
            self.eeprom
                .write_page(dst + idx, &data[..chunk_size as usize])?;

            idx += chunk_size;
            chunk_size = PAGE_SIZE;
        }
        Ok(())
    }

    fn set_user_profile(&mut self) -> Result<(), CoreError> {
        let user_profile = self.eeprom.read_byte(ADR_USER_PROFILE)?;
        self.user_profile = num::clamp(user_profile, 0, 3);
        Ok(())
    }

    fn profile(&self, id: PersistenceId) -> u32 {
        if (self.is_unique)(id) {
            0
        } else {
            self.user_profile as u32
        }
    }

    /// returns the address of an item
    fn item_address(&mut self, id: PersistenceId) -> u32 {
        eeprom::ADR_DATA_STORAGE + (self.profile(id) * MAX_USER_VALUES + id as u32) * 4
    }

    /// returns address of the byte of id in DAT
    fn data_byte_adr_from_id(&mut self, id: PersistenceId) -> u32 {
        eeprom::ADR_DAT + (id as u32 + self.profile(id) * MAX_USER_VALUES) / 8
    }

    /// Returns a byte of the DAT
    fn read_bitfield_byte(&mut self, id: PersistenceId) -> Result<u8, CoreError> {
        if id as u32 >= PersistenceId::LastItem as u32 {
            return Err(CoreError::PersistenceIdNotInDat);
        }
        let byte_adr = self.data_byte_adr_from_id(id);
        self.eeprom.read_byte(byte_adr)
    }

    /// Tests a id, if coresponing dat_bit is set
    fn test_id(&mut self, id: PersistenceId) -> bool {
        let byte_adr = self.data_byte_adr_from_id(id);
        let bit_pattern: u8 = 1 << ((id as u32) % 8);
        match self.eeprom.read_byte(byte_adr) {
            Ok(found) => (found & bit_pattern) != 0,
            Err(_) => false,
        }
    }

    /// Set dat_bit in table of contentspub fn iter_over(&mut self, p_type: PersistType) -> PersistenceIterator
    fn set_id(&mut self, id: PersistenceId) -> Result<(), CoreError> {
        let byte_adr = self.data_byte_adr_from_id(id);
        let bit_pattern: u8 = 1 << ((id as u32) % 8);
        let found = self.eeprom.read_byte(byte_adr)?;
        let target = found | bit_pattern;
        if found != target {
            self.eeprom.write_byte(byte_adr, target)?;
        }
        Ok(())
    }

    /// Deletes the bit in DAT => deletes the item in store
    fn clear_id(&mut self, id: PersistenceId) -> Result<(), CoreError> {
        let byte_adr = self.data_byte_adr_from_id(id);
        let bit_pattern: u8 = !(1 << ((id as u32) % 8));
        let found = self.eeprom.read_byte(byte_adr)?;
        let target = found & bit_pattern;
        if found != target {
            self.eeprom.write_byte(byte_adr, target)?;
        }
        Ok(())
    }
}

/// Helper struct for Iteration
pub struct PersistenceIterator<'a, S>
where
    S: EepromTrait,
{
    cur_id: u16,
    end_id: u16,
    user_profile_sent: bool,
    persistence: &'a mut Eeprom<S>,
}

impl<'a, S> PersistenceIterator<'a, S>
where
    S: EepromTrait,
{
    /// Creates a iteration helper struct
    pub fn new(start_id: u16, end_id: u16, persistence: &'a mut Eeprom<S>) -> Self {
        PersistenceIterator {
            cur_id: start_id,
            end_id,
            user_profile_sent: false,
            persistence,
        }
    }
}

impl<S> Iterator for PersistenceIterator<'_, S>
where
    S: EepromTrait,
{
    type Item = PersistenceItem;
    fn next(&mut self) -> Option<Self::Item> {
        if self.user_profile_sent {
            while self.cur_id < self.end_id {
                // The ids are not contiguous (143..149 are left free). A
                // number without an id converts to the default DoNotStore,
                // which is not in the table of contents: the unwrap below
                // would stop the device at start. Nothing is stored there.
                if PersistenceId::from(self.cur_id) as u16 != self.cur_id {
                    self.cur_id += 1;
                    continue;
                }
                let cur_byte = self
                    .persistence
                    .read_bitfield_byte(self.cur_id.into())
                    .unwrap();
                let cur_bit = 0x01 << (self.cur_id % 8);
                let id_exists = (cur_bit & cur_byte) != 0;

                if id_exists {
                    let r = self
                        .persistence
                        .read_item_unchecked(self.cur_id.into())
                        .unwrap();
                    self.cur_id += 1;
                    return Some(r);
                } else {
                    self.cur_id += 1;
                }
            }
            None
        } else {
            let value = self.persistence.eeprom.read_byte(ADR_USER_PROFILE).unwrap();
            let value = num::clamp(value, 0, 3);
            let item = PersistenceItem::from_u8(PersistenceId::UserProfile, value);
            self.user_profile_sent = true;
            Some(item)
        }
    }
}

#[cfg(test)]
mod tests {
    //! The settings in the EEPROM over a power cycle, with the calls of the
    //! device (device/larus_frontend_v2/src/idle_loop.rs): at start every id
    //! below `PersistenceId::LastItem` is looked up - twice, the first time
    //! before the update check; changes arrive as `IdleEvent::SetEepromItem`.
    extern crate std;
    use super::*;
    use crate::{
        basic_config::MAX_TX_FRAMES,
        controller::persist::{self, Echo},
        profile_always_0,
        utils::{CIdleEvents, IdleEvent, QIdleEvents},
        view::test_fixture::test_model,
        CoreController, CoreModel, QTxFrames,
    };
    use std::boxed::Box;

    /// EEPROM in RAM. A new chip reads 0xFF everywhere.
    struct Ram {
        data: [u8; eeprom::SIZE as usize],
    }

    impl Ram {
        fn blank() -> Self {
            Ram {
                data: [0xFF; eeprom::SIZE as usize],
            }
        }
    }

    impl EepromTrait for Ram {
        fn write_byte(&mut self, address: u32, data: u8) -> Result<(), CoreError> {
            self.write_page(address, &[data])
        }

        fn write_page(&mut self, address: u32, data: &[u8]) -> Result<(), CoreError> {
            let (start, end) = (address as usize, address as usize + data.len());
            if end > self.data.len() {
                return Err(CoreError::OutOfRange);
            }
            self.data[start..end].copy_from_slice(data);
            Ok(())
        }

        fn read_byte(&mut self, address: u32) -> Result<u8, CoreError> {
            let mut b = [0u8];
            self.read_data(address, &mut b)?;
            Ok(b[0])
        }

        fn read_data(&mut self, address: u32, data: &mut [u8]) -> Result<(), CoreError> {
            let (start, end) = (address as usize, address as usize + data.len());
            if end > self.data.len() {
                return Err(CoreError::OutOfRange);
            }
            data.copy_from_slice(&self.data[start..end]);
            Ok(())
        }
    }

    fn new_chip() -> Eeprom<Ram> {
        Eeprom::new(Ram::blank(), profile_always_0).unwrap()
    }

    /// power off and on again: the same chip, read from the start
    fn power_cycle(e: Eeprom<Ram>) -> Eeprom<Ram> {
        Eeprom::new(e.eeprom, profile_always_0).unwrap()
    }

    /// what the device restores at start (without the user profile item that always comes first)
    fn restored(e: &mut Eeprom<Ram>) -> std::vec::Vec<PersistenceItem> {
        let all: std::vec::Vec<PersistenceItem> = e.iter_over(EepromTopic::ConfigValues).collect();
        assert_eq!(all[0].id, PersistenceId::UserProfile);
        all[1..].to_vec()
    }

    /// The ids are not contiguous (143..149 are left free). The restore at
    /// start walks all numbers below LastItem and must get past the gaps:
    /// a panic there is a device that does not start any more.
    #[test]
    fn the_restore_at_start_gets_past_the_gaps_in_the_ids() {
        let mut e = new_chip();
        assert!(restored(&mut e).is_empty());
        // every number below LastItem is either an id or skipped
        for n in 0..PersistenceId::LastItem as u16 {
            let id: PersistenceId = n.into();
            assert!(id as u16 == n || id == PersistenceId::DoNotStore, "{n}");
        }
        assert_eq!(
            PersistenceId::from(145u16),
            PersistenceId::DoNotStore,
            "145 is a gap"
        );
    }

    /// a bit in the table of contents where no id lives (worn chip, a later
    /// firmware that used the number) is not an item and not a crash
    #[test]
    fn a_stray_bit_at_an_unused_number_is_ignored() {
        let mut e = new_chip();
        e.write_item(PersistenceItem::from_u8(PersistenceId::NmeaBaud, 3))
            .unwrap();
        let stray = 145u32;
        let adr = (eeprom::ADR_DAT + stray / 8) as usize;
        e.eeprom.data[adr] |= 1 << (stray % 8);
        let mut e = power_cycle(e);
        let items = restored(&mut e);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, PersistenceId::NmeaBaud);
    }

    /// the newest ids lie behind the gap: they are written and come back
    #[test]
    fn items_behind_the_gap_come_back_after_a_power_cycle() {
        let mut e = new_chip();
        let written = [
            PersistenceItem::from_u8(PersistenceId::Volume, 5),
            PersistenceItem::from_u8(PersistenceId::NmeaBaud, 3),
            PersistenceItem::from_i8(PersistenceId::VolumeStf, 7),
            PersistenceItem::from_f32(PersistenceId::PotBar, 80.0),
        ];
        for item in written {
            e.write_item(item).unwrap();
        }
        let mut e = power_cycle(e);
        assert_eq!(restored(&mut e), written);
        // a second pass (the device reads twice) gives the same
        assert_eq!(restored(&mut e), written);
    }

    /// model and controller with the queues of the device
    fn device() -> (CoreModel, CoreController, CIdleEvents, PPersistenceItems) {
        let q_tx: &'static mut QTxFrames<MAX_TX_FRAMES> = Box::leak(Box::new(Queue::new()));
        let q_idle: &'static mut QIdleEvents = Box::leak(Box::new(Queue::new()));
        let q_pers: &'static mut QPersistenceItems = Box::leak(Box::new(Queue::new()));
        let (p_tx, _c_tx) = q_tx.split();
        let (p_idle, c_idle) = q_idle.split();
        let (p_pers, c_pers) = q_pers.split();
        let mut cm = test_model();
        let cc = CoreController::new(&mut cm, p_idle, c_pers, p_tx);
        (cm, cc, c_idle, p_pers)
    }

    /// The whole way of a setting: menu -> controller -> idle loop -> EEPROM,
    /// power off, EEPROM -> idle loop -> controller -> model. With the POT
    /// bar (id 151) and the cruise volume (id 150, never stored up to
    /// 0.4.1.47 because LastItem was still 143).
    #[test]
    fn a_setting_goes_through_the_device_and_survives_the_power_cycle() {
        let (mut cm, mut cc, mut c_idle, _p_pers) = device();
        let mut e = new_chip();
        persist::persist_set(
            &mut cc,
            &mut cm,
            Variant::F32(80.0),
            PersistenceId::PotBar,
            Echo::None,
        );
        persist::persist_set(
            &mut cc,
            &mut cm,
            Variant::I8(7),
            PersistenceId::VolumeStf,
            Echo::None,
        );
        // after the persistence delay the controller hands the items to the idle loop
        let mut clock = 0u16;
        let mut ids = std::vec::Vec::new();
        for _ in 0..1500 {
            clock = clock.wrapping_add(1);
            cc.tick_1ms(clock, &mut cm);
            while let Some(event) = c_idle.dequeue() {
                if let IdleEvent::SetEepromItem(item) = event {
                    e.write_item(item).unwrap();
                    ids.push(item.id);
                }
            }
        }
        assert!(
            ids.contains(&PersistenceId::PotBar),
            "bar not written: {ids:?}"
        );
        assert!(
            ids.contains(&PersistenceId::VolumeStf),
            "cruise volume not written: {ids:?}"
        );

        // power off and on: a new model with the defaults, the same chip
        let (mut cm, mut cc, _c_idle, mut p_pers) = device();
        assert_eq!(cm.config.pot_bar, 1.0);
        assert_eq!(cm.config.volume_stf, -1);
        let mut e = power_cycle(e);
        // first pass of the device: only the downgrade flag is looked at
        assert!(e.iter_over(EepromTopic::ConfigValues).count() >= 3);
        // second pass: every item goes to the controller, one per call
        for item in e.iter_over(EepromTopic::ConfigValues) {
            p_pers.enqueue(item).unwrap();
            clock = clock.wrapping_add(1);
            cc.tick_1ms(clock, &mut cm);
        }
        assert!(
            (cm.config.pot_bar - 0.8).abs() < 1e-6,
            "bar {}",
            cm.config.pot_bar
        );
        assert_eq!(cm.config.volume_stf, 7);
    }

    /// 10 s of the device: ticks, and what goes to the idle loop is written
    fn run_10s(
        cm: &mut CoreModel,
        cc: &mut CoreController,
        c_idle: &mut CIdleEvents,
        e: &mut Eeprom<Ram>,
        clock: &mut u16,
    ) -> std::vec::Vec<PersistenceItem> {
        let mut written = std::vec::Vec::new();
        for _ in 0..10_500 {
            *clock = clock.wrapping_add(1);
            cc.tick_1ms(*clock, cm);
            while let Some(event) = c_idle.dequeue() {
                if let IdleEvent::SetEepromItem(item) = event {
                    e.write_item(item).unwrap();
                    written.push(item);
                }
            }
        }
        written
    }

    /// the restore of the device at start, one item per tick
    fn restore_all(
        cm: &mut CoreModel,
        cc: &mut CoreController,
        p_pers: &mut PPersistenceItems,
        e: &mut Eeprom<Ram>,
        clock: &mut u16,
    ) {
        for item in e.iter_over(EepromTopic::ConfigValues) {
            p_pers.enqueue(item).unwrap();
            *clock = clock.wrapping_add(1);
            cc.tick_1ms(*clock, cm);
        }
    }

    /// Menu mode (15.11.2026). A new device - empty chip, the restore sends
    /// only the user profile - becomes Basic 10 s after the start and opens
    /// the setup wizard; the mode is stored and found again at the next
    /// start, where no wizard comes any more.
    #[test]
    fn a_new_device_starts_basic_with_the_wizard_and_stays_basic() {
        let (mut cm, mut cc, mut c_idle, mut p_pers) = device();
        cm.control.mode_checked = false; // the fixture skips the decision
        let mut e = new_chip();
        let mut clock = 0u16;
        restore_all(&mut cm, &mut cc, &mut p_pers, &mut e, &mut clock);
        assert!(
            !cm.control.items_restored,
            "the user profile item is no sign of use"
        );
        assert!(!cm.config.basic_mode && !cm.control.mode_checked);
        let written = run_10s(&mut cm, &mut cc, &mut c_idle, &mut e, &mut clock);
        assert!(cm.config.basic_mode, "new device: Basic");
        assert!(
            crate::controller::wizard::step(&cm) == Some((1, 6)),
            "the wizard asks its first question"
        );
        assert!(
            written
                .iter()
                .any(|i| i.id == PersistenceId::MenuMode && i.to_u8() == 0),
            "{written:?}"
        );

        // next start (the clock of the device starts at 0 again)
        let (mut cm, mut cc, mut c_idle, mut p_pers) = device();
        cm.control.mode_checked = false;
        let mut clock = 0u16;
        let mut e = power_cycle(e);
        restore_all(&mut cm, &mut cc, &mut p_pers, &mut e, &mut clock);
        assert!(cm.config.basic_mode && cm.control.mode_checked);
        run_10s(&mut cm, &mut cc, &mut c_idle, &mut e, &mut clock);
        assert!(cm.config.basic_mode);
        assert!(
            crate::controller::wizard::step(&cm).is_none(),
            "the wizard comes once"
        );
    }

    /// A device that was in use (any setting in the chip, no menu mode yet:
    /// update from an older firmware) keeps the full menu: Expert, no wizard.
    #[test]
    fn an_updated_device_keeps_the_full_menu() {
        let mut e = new_chip();
        e.write_item(PersistenceItem::from_i8(PersistenceId::Volume, 5))
            .unwrap();
        let mut e = power_cycle(e);
        let (mut cm, mut cc, mut c_idle, mut p_pers) = device();
        cm.control.mode_checked = false;
        let mut clock = 0u16;
        restore_all(&mut cm, &mut cc, &mut p_pers, &mut e, &mut clock);
        assert!(cm.control.items_restored);
        let written = run_10s(&mut cm, &mut cc, &mut c_idle, &mut e, &mut clock);
        assert!(!cm.config.basic_mode, "Expert");
        assert!(crate::controller::wizard::step(&cm).is_none());
        assert_eq!(cm.config.display_active, crate::model::DisplayActive::Vario);
        assert!(
            written
                .iter()
                .any(|i| i.id == PersistenceId::MenuMode && i.to_u8() == 1),
            "{written:?}"
        );
    }

    /// a setting made in the first seconds (volume knob) does not turn a
    /// new device into a used one
    #[test]
    fn a_knob_turn_before_the_decision_does_not_count_as_use() {
        let (mut cm, mut cc, mut c_idle, mut p_pers) = device();
        cm.control.mode_checked = false;
        let mut e = new_chip();
        let mut clock = 0u16;
        restore_all(&mut cm, &mut cc, &mut p_pers, &mut e, &mut clock);
        persist::persist_set(
            &mut cc,
            &mut cm,
            Variant::I8(9),
            PersistenceId::Volume,
            Echo::None,
        );
        run_10s(&mut cm, &mut cc, &mut c_idle, &mut e, &mut clock);
        assert!(cm.config.basic_mode);
    }

    /// the factory reset empties the chip: Basic and the wizard again
    #[test]
    fn after_a_factory_reset_the_device_is_new_again() {
        let mut e = new_chip();
        e.write_item(PersistenceItem::from_i8(PersistenceId::Volume, 5))
            .unwrap();
        e.write_item(PersistenceItem::from_u8(PersistenceId::MenuMode, 1))
            .unwrap();
        let mut e = power_cycle(e);
        let (mut cm, mut cc, mut c_idle, mut p_pers) = device();
        cm.control.mode_checked = false;
        let mut clock = 0u16;
        restore_all(&mut cm, &mut cc, &mut p_pers, &mut e, &mut clock);
        assert!(!cm.config.basic_mode && cm.control.mode_checked);
        persist::factory_reset(&mut cm, &mut cc);
        // the idle loop: delete all, then restore
        let mut restore = false;
        while let Some(event) = c_idle.dequeue() {
            match event {
                IdleEvent::SetEepromItem(item) => e.write_item(item).unwrap(),
                IdleEvent::RestoreEepromItems => restore = true,
                _ => (),
            }
        }
        assert!(restore);
        restore_all(&mut cm, &mut cc, &mut p_pers, &mut e, &mut clock);
        assert!(!cm.control.mode_checked, "nothing left in the chip");
        run_10s(&mut cm, &mut cc, &mut c_idle, &mut e, &mut clock);
        assert!(cm.config.basic_mode);
        assert!(crate::controller::wizard::step(&cm) == Some((1, 6)));
    }

    /// Sound preset: listening to a preset writes nothing to the chip, the
    /// push writes its eleven values - and they are there after the restart.
    #[test]
    fn a_sound_preset_reaches_the_chip_with_the_push_only() {
        use crate::{
            model::editable::simple::{sound_preset, PRESET_SHARP, PRESET_STANDARD},
            Editable, Event, KeyEvent,
        };
        let (mut cm, mut cc, mut c_idle, _p_pers) = device();
        let mut e = new_chip();
        let mut clock = 0u16;
        let mut pump =
            |cm: &mut CoreModel, cc: &mut CoreController, e: &mut Eeprom<Ram>, ms: u32| {
                let mut n = 0;
                for _ in 0..ms {
                    clock = clock.wrapping_add(1);
                    cc.tick_1ms(clock, cm);
                    while let Some(event) = c_idle.dequeue() {
                        if let IdleEvent::SetEepromItem(item) = event {
                            e.write_item(item).unwrap();
                            n += 1;
                        }
                    }
                }
                n
            };
        cm.config.display_active = crate::model::DisplayActive::Menu;
        crate::controller::activate_editable(Editable::SoundPreset, &mut cm, &mut cc);
        cc.event_handler(Event::KeyItem(KeyEvent::Rotary2Right), &mut cm);
        cc.event_handler(Event::KeyItem(KeyEvent::Rotary2Right), &mut cm);
        assert_eq!(sound_preset(&cm), PRESET_SHARP);
        assert_eq!(
            pump(&mut cm, &mut cc, &mut e, 31_000),
            0,
            "listening wrote to the chip"
        );
        assert_eq!(sound_preset(&cm), PRESET_STANDARD);
        crate::controller::activate_editable(Editable::SoundPreset, &mut cm, &mut cc);
        cc.event_handler(Event::KeyItem(KeyEvent::Rotary2Right), &mut cm);
        cc.event_handler(Event::KeyItem(KeyEvent::Rotary2Right), &mut cm);
        cc.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut cm);
        assert_eq!(pump(&mut cm, &mut cc, &mut e, 3_000), 11);
        // restart
        let (mut cm, mut cc, _c_idle, mut p_pers) = device();
        let mut e = power_cycle(e);
        let mut clock = 0u16;
        restore_all(&mut cm, &mut cc, &mut p_pers, &mut e, &mut clock);
        assert_eq!(sound_preset(&cm), PRESET_SHARP);
    }

    /// a page style taken with a push is in the chip and back after the restart
    #[test]
    fn a_page_style_reaches_the_chip() {
        use crate::{
            model::editable::simple::{page_style, STYLE_BIG, STYLE_STANDARD},
            Editable, Event, KeyEvent,
        };
        let (mut cm, mut cc, mut c_idle, _p_pers) = device();
        let mut e = new_chip();
        let mut clock = 0u16;
        cm.config.display_active = crate::model::DisplayActive::Menu;
        crate::controller::activate_editable(Editable::PageStyle, &mut cm, &mut cc);
        for _ in 0..2 {
            cc.event_handler(Event::KeyItem(KeyEvent::Rotary2Right), &mut cm);
        }
        cc.event_handler(Event::KeyItem(KeyEvent::BtnEnc), &mut cm);
        assert_eq!(page_style(&cm), STYLE_BIG);
        run_10s(&mut cm, &mut cc, &mut c_idle, &mut e, &mut clock);
        let (mut cm, mut cc, _c_idle, mut p_pers) = device();
        assert_eq!(page_style(&cm), STYLE_STANDARD);
        let mut e = power_cycle(e);
        let mut clock = 0u16;
        restore_all(&mut cm, &mut cc, &mut p_pers, &mut e, &mut clock);
        assert_eq!(page_style(&cm), STYLE_BIG);
    }
}
