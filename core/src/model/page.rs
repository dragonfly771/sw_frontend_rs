//! Freely configurable display pages (Stefly design).
//!
//! A page is no longer a hard coded screen type but a small data record:
//! FRAME (what runs on the outside) + CORE (what happens inside) + the
//! info boxes. Everything that exists today is a special case of it, and
//! the planned combinations (several vario pages with different boxes,
//! centering aid in the core, later ring scale plus artificial horizon)
//! fit in without changing the model again.
//!
//! The whole set has to stay small: CoreModel is Copy and snapshotted
//! twice per frame.

use crate::{view::viewable::centerview::CenterView, LineView};

pub const MAX_PAGES: usize = 6;
/// number of info box slots: top, bottom, right 1..3
pub const PAGE_BOXES: usize = 5;

/// what runs on the outside of a page
#[derive(Clone, Copy, PartialEq, Default, Debug)]
#[repr(u8)]
pub enum PageFrame {
    /// page is switched off and skipped when paging
    Off = 0,
    #[default]
    Ring360 = 1,
    /// no instrument frame, the core uses the whole display
    Full = 3,
}

impl From<u8> for PageFrame {
    fn from(v: u8) -> Self {
        match v {
            1 => PageFrame::Ring360,
            // 2 was the old Classic frame - devices that still have it stored get the ring
            2 => PageFrame::Ring360,
            3 => PageFrame::Full,
            _ => PageFrame::Off,
        }
    }
}

pub const FRAME_OFF: &str = "Off";
pub const FRAME_RING: &str = "Ring 360";
pub const FRAME_FULL: &str = "Full";

impl PageFrame {
    pub fn as_str(&self) -> &'static str {
        match self {
            PageFrame::Off => FRAME_OFF,
            PageFrame::Ring360 => FRAME_RING,
            PageFrame::Full => FRAME_FULL,
        }
    }
}

/// what happens inside a page
#[derive(Clone, Copy, PartialEq, Default, Debug)]
#[repr(u8)]
pub enum PageCore {
    #[default]
    Boxes = 0,
    Horizon = 1,
    DeviceInfo = 2,
    /// centering aid / thermal assistant in the core (stage 3)
    ThermalAssist = 3,
    /// wind in the core (stage 3)
    Wind = 4,
    /// three large info boxes in the middle, needle shortened to the inner
    /// circle so that they have the whole centre for themselves
    BigBoxes = 5,
    /// FLARM radar in the core (plan_flarm_warnung.md), scale outside
    Radar = 6,
    /// two very large values, right of the middle (slots 2 and 3)
    TwoValues = 7,
}

impl From<u8> for PageCore {
    fn from(v: u8) -> Self {
        match v {
            1 => PageCore::Horizon,
            2 => PageCore::DeviceInfo,
            3 => PageCore::ThermalAssist,
            4 => PageCore::Wind,
            5 => PageCore::BigBoxes,
            6 if crate::model::flarm_cfg::FLARM_FEATURE => PageCore::Radar,
            7 => PageCore::TwoValues,
            _ => PageCore::Boxes,
        }
    }
}

pub const CORE_BOXES: &str = "Info Boxes";
pub const CORE_HORIZON: &str = "Horizon";
pub const CORE_INFO: &str = "Device Info";
pub const CORE_THERMAL: &str = "Thermal Assist";
pub const CORE_WIND: &str = "Wind";
pub const CORE_BIG_BOXES: &str = "Big Boxes";
pub const CORE_RADAR: &str = "FLARM Radar";
pub const CORE_TWO_VALUES: &str = "2 Big Values";

impl PageCore {
    pub fn as_str(&self) -> &'static str {
        match self {
            PageCore::Boxes => CORE_BOXES,
            PageCore::Horizon => CORE_HORIZON,
            PageCore::DeviceInfo => CORE_INFO,
            PageCore::ThermalAssist => CORE_THERMAL,
            PageCore::Wind => CORE_WIND,
            PageCore::BigBoxes => CORE_BIG_BOXES,
            PageCore::Radar => CORE_RADAR,
            PageCore::TwoValues => CORE_TWO_VALUES,
        }
    }

    /// a big values page (4 or 2 values): no centre circle, short needle
    pub fn big(&self) -> bool {
        matches!(self, PageCore::BigBoxes | PageCore::TwoValues)
    }

    /// a page with info boxes / values (flap tape, circling / cruise sets)
    pub fn with_values(&self) -> bool {
        matches!(
            self,
            PageCore::Boxes | PageCore::BigBoxes | PageCore::TwoValues
        )
    }

    /// a page with a circling and a cruise set: the pages with values and
    /// the centering page (large centering aid while circling, info boxes
    /// in straight flight)
    pub fn two_sets(&self) -> bool {
        self.with_values() || *self == PageCore::ThermalAssist
    }

    /// order in the selection list of the page editor
    pub const ALL: [PageCore; 8] = [
        PageCore::Boxes,
        PageCore::BigBoxes,
        PageCore::TwoValues,
        PageCore::Radar,
        PageCore::Horizon,
        PageCore::ThermalAssist,
        PageCore::Wind,
        PageCore::DeviceInfo,
    ];

    pub fn as_idx(&self) -> i32 {
        Self::ALL.iter().position(|c| c == self).unwrap_or(0) as i32
    }

    pub fn from_idx(idx: i32) -> PageCore {
        let i = idx.max(0) as usize;
        if i < Self::ALL.len() {
            Self::ALL[i]
        } else {
            PageCore::Boxes
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Page {
    pub frame: PageFrame,
    pub core: PageCore,
    /// info box content: 0 top, 1 bottom, 2..4 numbered boxes (right hand
    /// side of the ring page, the three large boxes of a Big Boxes page)
    pub boxes: [LineView; PAGE_BOXES],
    /// second set, used in speed to fly mode when cruise_boxes is set
    pub stf_boxes: [LineView; PAGE_BOXES],
    /// true: own box set in speed to fly mode (today's behaviour)
    pub cruise_boxes: bool,
    /// true: white background outside the scale ring, dark core with the
    /// info boxes. Per page, so one page can be the bright weather page.
    pub light_outer: bool,
    pub center_circling: CenterView,
    pub center_straight: CenterView,
    /// no flap tape on this page (default: the tape shows with a flap table)
    pub flap_tape_off: bool,
}

impl Default for Page {
    fn default() -> Self {
        Page {
            frame: PageFrame::Off,
            core: PageCore::Boxes,
            // same defaults as the single page had before
            boxes: [
                LineView::None,
                LineView::WindAndDelta,
                LineView::Altitude,
                LineView::AverageClimbRate,
                LineView::FlightLevel,
            ],
            stf_boxes: [
                LineView::None,
                LineView::WindAndDelta,
                LineView::Altitude,
                LineView::AverageClimbRate,
                LineView::FlightLevel,
            ],
            cruise_boxes: false,
            light_outer: false,
            center_circling: CenterView::SingleArrowCircling,
            center_straight: CenterView::SingleArrowStraight,
            flap_tape_off: false,
        }
    }
}

impl Page {
    /// content of one info box slot, depending on the flight mode
    pub fn box_view(&self, slot: usize, vario_mode: bool) -> LineView {
        let slot = slot.min(PAGE_BOXES - 1);
        if self.cruise_boxes && !vario_mode {
            self.stf_boxes[slot]
        } else {
            self.boxes[slot]
        }
    }

    pub fn is_on(&self) -> bool {
        self.frame != PageFrame::Off
    }

    /// The centering aid of the centering page: the one chosen for circling;
    /// the 3D ring if that is not a centering aid (a page stored by an
    /// older version, where this page always showed the dots in the small
    /// centre circle).
    pub fn centering_view(&self) -> CenterView {
        if self.center_circling.is_centering_aid() {
            self.center_circling
        } else {
            CenterView::ThermalAssistant3
        }
    }

    /// short description for the menu ("Ring 360 / Info Boxes")
    pub fn kind_str(&self) -> &'static str {
        if self.frame == PageFrame::Off {
            FRAME_OFF
        } else {
            match self.core {
                PageCore::Boxes => self.frame.as_str(),
                other => other.as_str(),
            }
        }
    }

    // ---- eeprom packing: three u32 per page ----
    pub fn word_a(&self) -> u32 {
        (self.frame as u32)
            | ((self.core as u32) << 3)
            | ((self.center_circling as u32) << 6)
            | ((self.center_straight as u32) << 10)
            | ((self.cruise_boxes as u32) << 14)
            | ((self.light_outer as u32) << 15)
            | ((self.flap_tape_off as u32) << 16)
    }

    pub fn set_word_a(&mut self, w: u32) {
        self.frame = PageFrame::from((w & 0x7) as u8);
        self.core = PageCore::from(((w >> 3) & 0x7) as u8);
        self.center_circling = CenterView::from(((w >> 6) & 0xf) as u8);
        self.center_straight = CenterView::from(((w >> 10) & 0xf) as u8);
        self.cruise_boxes = (w >> 14) & 1 == 1;
        self.light_outer = (w >> 15) & 1 == 1;
        self.flap_tape_off = (w >> 16) & 1 == 1;
    }

    fn pack_boxes(set: &[LineView; PAGE_BOXES]) -> u32 {
        let mut w = 0_u32;
        for (i, lv) in set.iter().enumerate() {
            w |= ((*lv as u32) & 0x3f) << (i * 6);
        }
        w
    }

    fn unpack_boxes(w: u32) -> [LineView; PAGE_BOXES] {
        let mut set = [LineView::None; PAGE_BOXES];
        for (i, lv) in set.iter_mut().enumerate() {
            *lv = LineView::from(((w >> (i * 6)) & 0x3f) as u8);
        }
        set
    }

    pub fn word_b(&self) -> u32 {
        Self::pack_boxes(&self.boxes)
    }

    pub fn set_word_b(&mut self, w: u32) {
        self.boxes = Self::unpack_boxes(w);
    }

    pub fn word_c(&self) -> u32 {
        Self::pack_boxes(&self.stf_boxes)
    }

    pub fn set_word_c(&mut self, w: u32) {
        self.stf_boxes = Self::unpack_boxes(w);
    }
}

/// The default page set of a new device (15.11.2026): the vario ring and
/// the horizon - two pages, so a turn of the outer knob never ends on a page
/// the pilot does not know. The status page is in Setup > System > Device
/// Info, more pages are added with Page Layout. A device with stored pages
/// keeps them.
pub fn default_pages() -> [Page; MAX_PAGES] {
    let mut pages = [Page::default(); MAX_PAGES];
    pages[0].frame = PageFrame::Ring360;
    pages[0].core = PageCore::Boxes;
    pages[0].cruise_boxes = true;
    pages[1].frame = PageFrame::Full;
    pages[1].core = PageCore::Horizon;
    pages
}

#[cfg(test)]
mod centering_page_tests {
    use super::*;

    #[test]
    fn the_centering_page_shows_a_centering_aid_whatever_was_stored() {
        let mut p = Page::default();
        p.core = PageCore::ThermalAssist;
        // a page of an older version: the centre circle held a wind arrow
        p.center_circling = CenterView::SingleArrowCircling;
        assert!(p.centering_view() == CenterView::ThermalAssistant3);
        p.center_circling = CenterView::None;
        assert!(p.centering_view() == CenterView::ThermalAssistant3);
        for aid in [
            CenterView::ThermalAssistant1,
            CenterView::ThermalAssistant2,
            CenterView::ThermalAssistant3,
        ] {
            p.center_circling = aid;
            assert!(p.centering_view() == aid);
            // the choice survives the EEPROM word
            let mut q = Page::default();
            q.set_word_a(p.word_a());
            assert!(q.core == PageCore::ThermalAssist && q.center_circling == aid);
        }
        assert!(PageCore::ThermalAssist.two_sets() && !PageCore::ThermalAssist.with_values());
        assert!(
            PageCore::Boxes.two_sets()
                && !PageCore::Horizon.two_sets()
                && !PageCore::Wind.two_sets()
        );
    }
}
