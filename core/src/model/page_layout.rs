//! Page layout editor on the page itself (plan_seiten_editor.md): the flight
//! menu item "Page Layout" switches the shown page into an edit mode; one
//! encoder walks through its areas (zones), the other one changes the zone
//! at once. Logic in controller/page_layout.rs, drawing in
//! view/page_layout.rs.

use crate::{
    model::page::{Page, PageCore, MAX_PAGES},
    view::viewable::centerview::CenterView,
    CoreModel, LineView,
};
use heapless::Vec;

/// one area of the page that can be edited
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Zone {
    /// the 360 degree scale outside (white / black / full screen)
    #[default]
    Scale,
    /// flap tape on the right (auto / off)
    FlapTape,
    /// what the page is: info boxes, big values, horizon, radar ...
    Core,
    /// which set of boxes / centre is edited: circling or cruise
    Mode,
    /// the small centre circle: wind / centering aid (info boxes page)
    Centre,
    /// info box slot
    Box(u8),
    /// the page itself: other page, add, move, delete
    Page,
    /// save and leave the editor
    Exit,
}

impl Zone {
    pub fn name(&self) -> &'static str {
        match self {
            Zone::Scale => "Scale",
            Zone::FlapTape => "Flap tape",
            Zone::Core => "Core",
            Zone::Mode => "Flight mode",
            Zone::Centre => "Centre",
            Zone::Box(0) => "Box top",
            Zone::Box(1) => "Box bottom",
            Zone::Box(2) => "Box right 1",
            Zone::Box(_) => "Box right 2",
            Zone::Page => "Page",
            Zone::Exit => "Save & exit",
        }
    }

    /// name on the big values page (four stacked values, top to bottom:
    /// slots 0, 2, 3, 1)
    pub fn name_big(&self) -> &'static str {
        match self {
            Zone::Box(0) => "Value 1",
            Zone::Box(2) => "Value 2",
            Zone::Box(3) => "Value 3",
            Zone::Box(_) => "Value 4",
            other => other.name(),
        }
    }

    /// name of the zone on this page
    pub fn name_on(&self, page: &Page) -> &'static str {
        match (page.core, self) {
            (PageCore::BigBoxes, _) => self.name_big(),
            (PageCore::TwoValues, Zone::Box(2)) => "Value 1",
            (PageCore::TwoValues, Zone::Box(_)) => "Value 2",
            _ => self.name(),
        }
    }
}

/// which list is open
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum ListKind {
    #[default]
    None,
    /// the choice of the current zone
    Zone,
    /// leaving the editor: save, discard or go on editing
    Exit,
}

/// state of the page layout editor (part of Control, never stored)
#[derive(Clone, Copy, PartialEq, Default)]
pub struct LayoutEdit {
    pub active: bool,
    /// index into `zones()` of the page
    pub zone: u8,
    /// editing the cruise set (boxes, centre) instead of the circling set
    pub cruise: bool,
    pub list: ListKind,
    /// selected row of the open list
    pub sel: u8,
    /// the page as it was when the list was opened (Esc restores it)
    pub orig: Page,
    /// the page index when the list was opened (page preview)
    pub orig_idx: u8,
    /// circling / cruise when the list was opened
    pub orig_cruise: bool,
    /// row of the list that was the choice when it opened (marked)
    pub orig_sel: u8,
    /// 100 ms ticks until a choice list closes by itself (0 = stays)
    pub timer: u8,
    /// all pages as they were when the editor started ("Discard")
    pub start_pages: [Page; MAX_PAGES],
    pub start_idx: u8,
}

pub const MAX_ZONES: usize = 12;

/// the zones of a page, in the order of the small encoder
pub fn zones(cm: &CoreModel, page: &Page) -> Vec<Zone, MAX_ZONES> {
    let mut v: Vec<Zone, MAX_ZONES> = Vec::new();
    let has_flaps = !cm.flap_table().is_empty();
    match page.core {
        PageCore::DeviceInfo => {
            let _ = v.push(Zone::Core);
        }
        PageCore::Boxes => {
            let _ = v.push(Zone::Scale);
            if has_flaps {
                let _ = v.push(Zone::FlapTape);
            }
            let _ = v.push(Zone::Core);
            let _ = v.push(Zone::Mode);
            let _ = v.push(Zone::Centre);
            for s in 0..4 {
                let _ = v.push(Zone::Box(s));
            }
        }
        PageCore::ThermalAssist => {
            // the centering page: while circling the large centering aid
            // with the box above and the box below it, in cruise the info
            // boxes page with its four boxes
            let _ = v.push(Zone::Scale);
            if has_flaps {
                let _ = v.push(Zone::FlapTape);
            }
            let _ = v.push(Zone::Core);
            let _ = v.push(Zone::Mode);
            let _ = v.push(Zone::Centre);
            let boxes = if cm.control.layout.cruise { 4 } else { 2 };
            for s in 0..boxes {
                let _ = v.push(Zone::Box(s));
            }
        }
        PageCore::BigBoxes => {
            let _ = v.push(Zone::Scale);
            if has_flaps {
                let _ = v.push(Zone::FlapTape);
            }
            let _ = v.push(Zone::Core);
            let _ = v.push(Zone::Mode);
            // top to bottom as on the page
            for s in [0, 2, 3, 1] {
                let _ = v.push(Zone::Box(s));
            }
        }
        PageCore::TwoValues => {
            let _ = v.push(Zone::Scale);
            if has_flaps {
                let _ = v.push(Zone::FlapTape);
            }
            let _ = v.push(Zone::Core);
            let _ = v.push(Zone::Mode);
            // the big values of the 4 values page
            let _ = v.push(Zone::Box(2));
            let _ = v.push(Zone::Box(3));
        }
        _ => {
            let _ = v.push(Zone::Scale);
            let _ = v.push(Zone::Core);
        }
    }
    let _ = v.push(Zone::Page);
    let _ = v.push(Zone::Exit);
    v
}

/// what a row of a choice list does
#[derive(Clone, Copy, PartialEq)]
pub enum Choice {
    /// ring with white (true) or black outer band
    Ring(bool),
    FullScreen,
    /// flap tape shown (true) / off
    Tape(bool),
    Core(PageCore),
    Centre(CenterView),
    Box(LineView),
    /// edit the cruise set (true) or the circling set
    Mode(bool),
    /// show page idx (preview while turning)
    GoTo(u8),
    /// the cruise set is the circling set again
    CruiseLikeCircling,
    AddPage,
    MoveLeft,
    MoveRight,
    Delete,
    Save,
    Discard,
    GoOn,
}

/// one row of a choice list
#[derive(Clone, Copy, PartialEq)]
pub struct Row {
    pub label: &'static str,
    pub choice: Choice,
}

pub const MAX_ROWS: usize = 28;

pub const CORES: [(PageCore, &str); 8] = [
    (PageCore::Boxes, "Info boxes"),
    (PageCore::BigBoxes, "Big values"),
    (PageCore::TwoValues, "2 big values"),
    (PageCore::Horizon, "Horizon"),
    (PageCore::Radar, "FLARM radar"),
    (PageCore::ThermalAssist, "Centering aid"),
    (PageCore::Wind, "Wind"),
    (PageCore::DeviceInfo, "Status page"),
];

pub const CENTRE_CIRCLING: [(CenterView, &str); 6] = [
    (CenterView::SingleArrowCircling, "Wind, 1 arrow"),
    (CenterView::DoubleArrowCircling, "Wind, 2 arrows"),
    (CenterView::ThermalAssistant1, "Centering, dots"),
    (CenterView::ThermalAssistant2, "Centering, spider"),
    (CenterView::ThermalAssistant3, "Centering, 3D ring"),
    (CenterView::None, "Off"),
];

/// the centering aids of the centering page (shown large while circling)
pub const CENTRE_CENTERING: [(CenterView, &str); 4] = [
    (CenterView::ThermalAssistant1, "Centering, dots"),
    (CenterView::ThermalAssistant2, "Centering, spider"),
    (CenterView::ThermalAssistant3, "Centering, 3D ring"),
    (CenterView::ThermalAssistant4, "Centering, track"),
];

pub const CENTRE_CRUISE: [(CenterView, &str); 3] = [
    (CenterView::SingleArrowStraight, "Wind, 1 arrow"),
    (CenterView::DoubleArrowStraight, "Wind, 2 arrows"),
    (CenterView::None, "Off"),
];
