//! Page layout editor on the page (plan_seiten_editor.md), keys and actions.
//! Model: model/page_layout.rs, drawing: view/page_layout.rs.
//!
//! Two knobs, one job each (flight test 28.10.2026):
//! - lower knob (`Rotary1`): next / previous zone - Scale, Flap tape, Core,
//!   Flight mode, Centre, the boxes, Page, Save & exit
//! - upper knob (`Rotary2`, the one with the push): changes the zone at
//!   once. The first click opens the choice list, every click shows the
//!   choice on the page; the list closes by itself after a short pause, a
//!   push or the lower knob. Esc while the list is open takes the choice back.
//!
//! Nothing is written while editing. "Save & exit" (last zone, a long push
//! of 3 s, Esc of a remote) asks: save and exit / discard changes / continue.
//! The display has no Esc key: a choice is taken back by turning back to
//! the row that is marked (the choice before).

use crate::{
    controller::{apply_page, persist},
    model::{
        page::{Page, PageCore, PageFrame, MAX_PAGES},
        page_layout::{
            zones, Choice, ListKind, Row, Zone, CENTRE_CENTERING, CENTRE_CIRCLING, CENTRE_CRUISE,
            CORES, MAX_ROWS,
        },
    },
    utils::Variant,
    view::viewable::vario_infoview::Placement,
    CoreController, CoreModel, Echo, KeyEvent, LineView, PersistenceId,
};
use heapless::Vec;

/// bit of the page index in `CoreController::page_save`
const SAVE_IDX_BIT: u32 = 1 << 31;
/// a list opened by turning closes 2.5 s after the last click
const LIST_AUTO_CLOSE: u8 = 25;

const PAGE_LABELS: [&str; MAX_PAGES] = ["Page 1", "Page 2", "Page 3", "Page 4", "Page 5", "Page 6"];

/// the lower knob: zones
fn zone_step(key: KeyEvent) -> Option<i32> {
    match key {
        KeyEvent::Rotary1Right => Some(1),
        KeyEvent::Rotary1Left => Some(-1),
        _ => None,
    }
}

/// the upper knob: values
fn value_step(key: KeyEvent) -> Option<i32> {
    match key {
        KeyEvent::Rotary2Right | KeyEvent::BtnEncPlusRotary2Right => Some(1),
        KeyEvent::Rotary2Left | KeyEvent::BtnEncPlusRotary2Left => Some(-1),
        _ => None,
    }
}

pub fn active(cm: &CoreModel) -> bool {
    cm.control.layout.active
}

/// flight menu "Page Layout": edit the page that is shown
pub fn start(cm: &mut CoreModel) {
    let pages = cm.config.pages;
    let idx = cm.config.page_idx;
    let l = &mut cm.control.layout;
    l.active = true;
    l.zone = 0;
    l.cruise = false;
    l.list = ListKind::None;
    l.sel = 0;
    l.timer = 0;
    l.start_pages = pages;
    l.start_idx = idx;
}

fn stop(cm: &mut CoreModel) {
    cm.control.layout.active = false;
    cm.control.layout.list = ListKind::None;
}

/// something differs from the pages at the start
pub fn changed(cm: &CoreModel) -> bool {
    cm.config.pages != cm.control.layout.start_pages
}

/// write what differs from the start (a few words per tick, save_tick)
fn save(cm: &mut CoreModel, cc: &mut CoreController) {
    let start = cm.control.layout.start_pages;
    for i in 0..MAX_PAGES {
        let (a, b) = (start[i], cm.config.pages[i]);
        if a.word_a() != b.word_a() || a.word_b() != b.word_b() || a.word_c() != b.word_c() {
            cc.page_save |= 0b111 << (i * 3);
        }
    }
    if cm.config.page_idx != cm.control.layout.start_idx {
        cc.page_save |= SAVE_IDX_BIT;
    }
}

fn discard(cm: &mut CoreModel) {
    cm.config.pages = cm.control.layout.start_pages;
    cm.config.page_idx = cm.control.layout.start_idx;
    apply_page(cm);
}

/// end the editor at once (FLARM warning): an open choice is taken back,
/// everything else is saved
pub fn abort(cm: &mut CoreModel, cc: &mut CoreController) {
    if !active(cm) {
        return;
    }
    if cm.control.layout.list == ListKind::Zone {
        undo_list(cm);
    }
    save(cm, cc);
    stop(cm);
}

fn cur_idx(cm: &CoreModel) -> usize {
    (cm.config.page_idx as usize).min(MAX_PAGES - 1)
}

fn page_mut(cm: &mut CoreModel) -> &mut Page {
    let i = cur_idx(cm);
    &mut cm.config.pages[i]
}

/// the zone the editor is on
pub fn zone(cm: &CoreModel) -> Zone {
    // while a choice is open the zones are those of the page as it was:
    // the preview of another core must not move the zone
    let page = if cm.control.layout.list == ListKind::Zone {
        &cm.control.layout.orig
    } else {
        cm.config.page()
    };
    let z = zones(cm, page);
    let i = (cm.control.layout.zone as usize).min(z.len().saturating_sub(1));
    z.get(i).copied().unwrap_or(Zone::Page)
}

/// the zone that is highlighted in the page: zone selection only - while a
/// list is open the page shows the choice as it will look
pub fn highlight(cm: &CoreModel) -> Option<Zone> {
    if active(cm) && cm.control.layout.list == ListKind::None {
        Some(zone(cm))
    } else {
        None
    }
}

/// the info box slot is highlighted
pub fn box_highlight(cm: &CoreModel, slot: usize) -> bool {
    highlight(cm) == Some(Zone::Box(slot as u8))
}

/// the cruise set is edited, but the page has none of its own yet (it is
/// made on the first change)
pub fn cruise_shared(cm: &CoreModel) -> bool {
    cm.control.layout.cruise && !cm.config.page().cruise_boxes
}

/// the rows of the open list
pub fn rows(cm: &CoreModel) -> Vec<Row, MAX_ROWS> {
    let mut v: Vec<Row, MAX_ROWS> = Vec::new();
    let mut push = |label: &'static str, choice: Choice| {
        let _ = v.push(Row { label, choice });
    };
    if cm.control.layout.list == ListKind::Exit {
        if changed(cm) {
            push("Save & exit", Choice::Save);
            push("Discard changes", Choice::Discard);
        } else {
            push("Exit", Choice::Save);
        }
        push("Continue", Choice::GoOn);
        return v;
    }
    let page = cm.config.page();
    let cruise = cm.control.layout.cruise;
    match zone(cm) {
        Zone::Scale => {
            push("Ring, white", Choice::Ring(true));
            push("Ring, black", Choice::Ring(false));
            if page.core == PageCore::Horizon {
                push("Full screen", Choice::FullScreen);
            }
        }
        Zone::FlapTape => {
            push("Auto", Choice::Tape(true));
            push("Off", Choice::Tape(false));
        }
        Zone::Core => {
            for (c, name) in CORES {
                if c == PageCore::Radar && !crate::model::flarm_cfg::FLARM_FEATURE {
                    continue;
                }
                push(name, Choice::Core(c));
            }
        }
        Zone::Mode => {
            push("Circling", Choice::Mode(false));
            push("Cruise", Choice::Mode(true));
            if cm.control.layout.orig.cruise_boxes {
                push("Cruise = circling", Choice::CruiseLikeCircling);
            }
        }
        Zone::Centre => {
            if cruise {
                for (c, name) in CENTRE_CRUISE {
                    push(name, Choice::Centre(c));
                }
            } else if page.core == PageCore::ThermalAssist {
                // the centering page: which aid is shown large
                for (c, name) in CENTRE_CENTERING {
                    push(name, Choice::Centre(c));
                }
            } else {
                for (c, name) in CENTRE_CIRCLING {
                    push(name, Choice::Centre(c));
                }
            }
        }
        Zone::Box(_) => {
            for i in 0..=LineView::max(Placement::Bottom) {
                let lv = LineView::from_sorted(i, Placement::Bottom);
                let name = if lv == LineView::None {
                    "Empty"
                } else {
                    lv.name()
                };
                push(name, Choice::Box(lv));
            }
        }
        Zone::Page => {
            // the pages first (turning shows them), then what can be done
            // with the page the list was opened on
            let on = on_pages(cm);
            for (k, &i) in on.iter().enumerate() {
                push(PAGE_LABELS[k], Choice::GoTo(i as u8));
            }
            let pos = on
                .iter()
                .position(|&i| i == cm.control.layout.orig_idx as usize)
                .unwrap_or(0);
            if on.len() < MAX_PAGES {
                push("Add page after", Choice::AddPage);
            }
            if pos > 0 {
                push("Move left", Choice::MoveLeft);
            }
            if pos + 1 < on.len() {
                push("Move right", Choice::MoveRight);
            }
            if on.len() > 1 {
                push("Delete page", Choice::Delete);
            }
        }
        Zone::Exit => (),
    }
    v
}

/// row of the list that matches the page as it is
fn current_row(cm: &CoreModel, rows: &[Row]) -> usize {
    let page = cm.config.page();
    let cruise = cm.control.layout.cruise;
    let slot = match zone(cm) {
        Zone::Box(s) => s as usize,
        _ => 0,
    };
    rows.iter()
        .position(|r| match r.choice {
            Choice::Ring(light) => page.frame == PageFrame::Ring360 && page.light_outer == light,
            Choice::FullScreen => page.frame == PageFrame::Full,
            Choice::Tape(on) => page.flap_tape_off != on,
            Choice::Core(c) => page.core == c,
            Choice::Mode(c) => cruise == c,
            Choice::Centre(c) => {
                if cruise {
                    page.center_straight == c
                } else if page.core == PageCore::ThermalAssist {
                    page.centering_view() == c
                } else {
                    page.center_circling == c
                }
            }
            Choice::Box(lv) => page.box_view(slot, !cruise) == lv,
            Choice::GoTo(i) => i == cm.config.page_idx,
            _ => false,
        })
        .unwrap_or(0)
}

/// put a choice into the page (RAM only; preview while turning)
fn apply(cm: &mut CoreModel, choice: Choice) {
    let cruise = cm.control.layout.cruise;
    let slot = match zone(cm) {
        Zone::Box(s) => s as usize,
        _ => 0,
    };
    match choice {
        Choice::Mode(c) => {
            cm.control.layout.cruise = c;
            return;
        }
        Choice::GoTo(i) => {
            if cm.config.pages[(i as usize).min(MAX_PAGES - 1)].is_on() {
                cm.config.page_idx = i;
                cm.control.page_indicator = crate::basic_config::PAGE_INDICATOR_TICKS;
            }
            apply_page(cm);
            return;
        }
        _ => (),
    }
    // a preview of another page is left again by any other choice
    let orig_centre = cm.control.layout.orig.center_circling;
    let p = page_mut(cm);
    match choice {
        Choice::Ring(light) => {
            p.frame = PageFrame::Ring360;
            p.light_outer = light;
        }
        Choice::FullScreen => p.frame = PageFrame::Full,
        Choice::Tape(on) => p.flap_tape_off = !on,
        Choice::Core(c) => {
            p.core = c;
            // the status page has no scale, the others need a ring except
            // the horizon, which may stay full screen
            if c == PageCore::DeviceInfo {
                p.frame = PageFrame::Full;
            } else if c != PageCore::Horizon {
                p.frame = PageFrame::Ring360;
            }
            // The centering page shows a centering aid, whatever the
            // centre circle of the page showed before; turning on to
            // another core gives the page its own centre view back.
            p.center_circling = orig_centre;
            if c == PageCore::ThermalAssist {
                p.center_circling = p.centering_view();
            } else if p.center_circling == crate::CenterView::ThermalAssistant4 {
                // the track is an aid of the centering page only
                p.center_circling = crate::CenterView::ThermalAssistant3;
            }
        }
        Choice::Centre(c) => {
            if cruise {
                p.center_straight = c;
            } else {
                p.center_circling = c;
            }
        }
        Choice::Box(lv) => {
            if cruise {
                // first change of a cruise box: own cruise set, a copy of
                // the circling one
                if !p.cruise_boxes {
                    p.stf_boxes = p.boxes;
                    p.cruise_boxes = true;
                }
                p.stf_boxes[slot] = lv;
            } else {
                p.boxes[slot] = lv;
            }
        }
        _ => return,
    }
    apply_page(cm);
}

/// a row that shows itself on the page while turning; the others are
/// actions, done on a push only
fn is_preview(choice: Choice) -> bool {
    matches!(
        choice,
        Choice::Ring(_)
            | Choice::FullScreen
            | Choice::Tape(_)
            | Choice::Core(_)
            | Choice::Centre(_)
            | Choice::Box(_)
            | Choice::Mode(_)
            | Choice::GoTo(_)
    )
}

/// indices of the pages that are on, in their order
fn on_pages(cm: &CoreModel) -> Vec<usize, MAX_PAGES> {
    let mut v: Vec<usize, MAX_PAGES> = Vec::new();
    for i in 0..MAX_PAGES {
        if cm.config.pages[i].is_on() {
            let _ = v.push(i);
        }
    }
    v
}

/// the pages in a new order (on pages first, then the off ones), showing
/// page `show` of the new order
fn reorder(cm: &mut CoreModel, order: &[Page], show: usize) {
    let mut new = [Page::default(); MAX_PAGES];
    for (k, p) in order.iter().take(MAX_PAGES).enumerate() {
        new[k] = *p;
    }
    cm.config.pages = new;
    cm.config.page_idx = show.min(order.len().saturating_sub(1)) as u8;
    cm.control.page_indicator = crate::basic_config::PAGE_INDICATOR_TICKS;
    apply_page(cm);
}

/// an action of the Page / Flight mode list, on the page the list was
/// opened on
fn action(cm: &mut CoreModel, choice: Choice) {
    if choice == Choice::CruiseLikeCircling {
        let p = page_mut(cm);
        p.cruise_boxes = false;
        p.stf_boxes = p.boxes;
        cm.control.layout.cruise = true;
        apply_page(cm);
        return;
    }
    // back to the page of the list (the rows may have shown another one)
    cm.config.page_idx = cm.control.layout.orig_idx;
    let on = on_pages(cm);
    let cur = cur_idx(cm);
    let pos = on.iter().position(|&i| i == cur).unwrap_or(0);
    let mut order: Vec<Page, MAX_PAGES> = on.iter().map(|&i| cm.config.pages[i]).collect();
    match choice {
        Choice::AddPage => {
            let copy = cm.config.pages[cur];
            if order.insert(pos + 1, copy).is_ok() {
                reorder(cm, &order, pos + 1);
            }
        }
        Choice::MoveLeft if pos > 0 => {
            order.swap(pos, pos - 1);
            reorder(cm, &order, pos - 1);
        }
        Choice::MoveRight if pos + 1 < order.len() => {
            order.swap(pos, pos + 1);
            reorder(cm, &order, pos + 1);
        }
        Choice::Delete if order.len() > 1 => {
            order.remove(pos);
            reorder(cm, &order, pos.saturating_sub(1));
        }
        _ => apply_page(cm),
    }
    cm.control.layout.cruise = false;
    // the zone stays "Page" (of the page shown now)
    cm.control.layout.orig = *cm.config.page();
}

/// write the changed page words a few at a time (the persistence buffer
/// takes 8 ids) and close a list that was left alone; every 100 ms
pub fn save_tick(cm: &mut CoreModel, cc: &mut CoreController) {
    if active(cm) && cm.control.layout.list == ListKind::Zone && cm.control.layout.timer > 0 {
        cm.control.layout.timer -= 1;
        if cm.control.layout.timer == 0 {
            close_list(cm);
        }
    }
    let mut n = 0;
    while cc.page_save != 0 && n < 3 && cc.pers_vals.len() < crate::controller::MAX_PERS_IDS - 2 {
        if cc.page_save & SAVE_IDX_BIT != 0 {
            cc.page_save &= !SAVE_IDX_BIT;
            let idx = cm.config.page_idx;
            persist::persist_set(cc, cm, Variant::U8(idx), PersistenceId::PageIdx, Echo::None);
        } else {
            let bit = cc.page_save.trailing_zeros() as usize;
            cc.page_save &= !(1 << bit);
            let (idx, word) = (bit / 3, bit % 3);
            let p = cm.config.pages[idx.min(MAX_PAGES - 1)];
            let w = match word {
                0 => p.word_a(),
                1 => p.word_b(),
                _ => p.word_c(),
            };
            persist::persist_set(
                cc,
                cm,
                Variant::U32(w),
                persist::page_id(word as u8, idx),
                Echo::None,
            );
        }
        n += 1;
    }
}

fn open_list(cm: &mut CoreModel, kind: ListKind, timer: u8) {
    let l = &mut cm.control.layout;
    l.list = kind;
    l.timer = timer;
    l.orig_cruise = l.cruise;
    l.orig = *cm.config.page();
    l.orig_idx = cm.config.page_idx;
    let r = rows(cm);
    cm.control.layout.sel = if kind == ListKind::Zone {
        current_row(cm, &r) as u8
    } else {
        0
    };
    cm.control.layout.orig_sel = cm.control.layout.sel;
}

/// close the list, the choice shown stays
fn close_list(cm: &mut CoreModel) {
    let prev = zone(cm);
    let other_page = cm.config.page_idx != cm.control.layout.orig_idx;
    cm.control.layout.list = ListKind::None;
    cm.control.layout.timer = 0;
    if other_page {
        // went to another page: its own flight mode starts with circling
        cm.control.layout.cruise = false;
    }
    // the zones may have changed with the core: stay on the same zone
    let z = zones(cm, cm.config.page());
    let n = z.len().max(1);
    cm.control.layout.zone = match z.iter().position(|&x| x == prev) {
        Some(i) => i as u8,
        None => (cm.control.layout.zone as usize).min(n - 1) as u8,
    };
}

/// take the choice of the open list back
fn undo_list(cm: &mut CoreModel) {
    let l = cm.control.layout;
    cm.config.page_idx = l.orig_idx;
    *page_mut(cm) = l.orig;
    cm.control.layout.cruise = l.orig_cruise;
    apply_page(cm);
}

fn step_zone(cm: &mut CoreModel, step: i32) {
    let n = zones(cm, cm.config.page()).len() as i32;
    let z = (cm.control.layout.zone as i32 + step).rem_euclid(n.max(1));
    cm.control.layout.zone = z as u8;
}

/// Keys while the editor is on: it takes all of them
pub fn key_action(key_event: &mut KeyEvent, cm: &mut CoreModel, cc: &mut CoreController) {
    if !active(cm) {
        return;
    }
    let key = *key_event;
    *key_event = KeyEvent::NoEvent;
    // long push (3 s): leave - the question first, an open choice is taken back
    if key == KeyEvent::BtnEncS3 {
        if cm.control.layout.list == ListKind::Zone {
            undo_list(cm);
            close_list(cm);
        }
        if cm.control.layout.list == ListKind::None {
            open_list(cm, ListKind::Exit, 0);
        }
        return;
    }
    match cm.control.layout.list {
        ListKind::None => {
            let exit_zone = zone(cm) == Zone::Exit;
            if let Some(step) = zone_step(key) {
                step_zone(cm, step);
            } else if let Some(step) = value_step(key) {
                if exit_zone {
                    open_list(cm, ListKind::Exit, 0);
                } else {
                    // the first click opens the list and already moves on
                    open_list(cm, ListKind::Zone, LIST_AUTO_CLOSE);
                    select(cm, step);
                }
            } else if key == KeyEvent::BtnEnc {
                if exit_zone {
                    open_list(cm, ListKind::Exit, 0);
                } else {
                    // opened with a push: stays until push, Esc or the lower knob
                    open_list(cm, ListKind::Zone, 0);
                }
            } else if key == KeyEvent::BtnEsc {
                open_list(cm, ListKind::Exit, 0);
            }
        }
        ListKind::Zone => {
            if let Some(step) = value_step(key) {
                if cm.control.layout.timer > 0 {
                    cm.control.layout.timer = LIST_AUTO_CLOSE;
                }
                select(cm, step);
            } else if let Some(step) = zone_step(key) {
                close_list(cm);
                step_zone(cm, step);
            } else if key == KeyEvent::BtnEnc {
                let r = rows(cm);
                if let Some(row) = r.get(cm.control.layout.sel as usize) {
                    if !is_preview(row.choice) {
                        action(cm, row.choice);
                    }
                }
                close_list(cm);
            } else if key == KeyEvent::BtnEsc {
                undo_list(cm);
                close_list(cm);
            }
        }
        ListKind::Exit => {
            if let Some(step) = value_step(key).or(zone_step(key)) {
                let n = rows(cm).len().max(1) as i32;
                cm.control.layout.sel = (cm.control.layout.sel as i32 + step).clamp(0, n - 1) as u8;
            } else if key == KeyEvent::BtnEnc {
                let choice = rows(cm)
                    .get(cm.control.layout.sel as usize)
                    .map(|r| r.choice);
                match choice {
                    Some(Choice::Save) => {
                        save(cm, cc);
                        stop(cm);
                    }
                    Some(Choice::Discard) => {
                        discard(cm);
                        stop(cm);
                    }
                    _ => cm.control.layout.list = ListKind::None,
                }
            } else if key == KeyEvent::BtnEsc {
                cm.control.layout.list = ListKind::None;
            }
        }
    }
}

/// move the selection of the open choice list, the page shows it at once
fn select(cm: &mut CoreModel, step: i32) {
    let r = rows(cm);
    let n = r.len().max(1) as i32;
    let s = (cm.control.layout.sel as i32 + step).clamp(0, n - 1);
    cm.control.layout.sel = s as u8;
    if let Some(row) = r.get(s as usize) {
        if is_preview(row.choice) {
            apply(cm, row.choice);
        } else if cm.config.page_idx != cm.control.layout.orig_idx {
            // an action row after a page preview: back to the list's page
            cm.config.page_idx = cm.control.layout.orig_idx;
            apply_page(cm);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{menu::flight_menu, DisplayActive, OverlayActive},
        test_harness, Event,
    };

    const ZONE_FWD: KeyEvent = KeyEvent::Rotary1Right;
    const ZONE_BACK: KeyEvent = KeyEvent::Rotary1Left;
    const VAL_FWD: KeyEvent = KeyEvent::Rotary2Right;
    const VAL_BACK: KeyEvent = KeyEvent::Rotary2Left;

    fn key(cm: &mut CoreModel, cc: &mut CoreController, t: &mut u16, k: KeyEvent) {
        cc.event_handler(Event::KeyItem(k), cm);
        tick(cm, cc, t, 20);
    }

    fn tick(cm: &mut CoreModel, cc: &mut CoreController, t: &mut u16, ms: u32) {
        for _ in 0..ms {
            *t = t.wrapping_add(1);
            cc.tick_1ms(*t, cm);
        }
    }

    fn setup() -> (CoreModel, CoreController, u16) {
        let (mut cm, mut cc, _tx) = test_harness!();
        let mut t = 0;
        tick(&mut cm, &mut cc, &mut t, 300);
        (cm, cc, t)
    }

    /// Setup > Display > Page Layout (15.11.2026, before: flight menu)
    fn open(cm: &mut CoreModel, cc: &mut CoreController, t: &mut u16) {
        assert!(
            flight_menu(cm)
                .items
                .iter()
                .all(|it| it.name(cm) != "Page Layout"),
            "the flight menu is short"
        );
        key(cm, cc, t, KeyEvent::BtnEncS3);
        assert!(
            cm.config.display_active == DisplayActive::Menu,
            "setup open"
        );
        for name in ["Display", "Page Layout"] {
            let menu = cm.control.menu_control.menu;
            let pos = menu
                .items
                .iter()
                .position(|it| it.name(cm) == name)
                .unwrap_or_else(|| panic!("{name} not in {}", menu.name));
            cm.control.menu_control.pos[menu.level] = pos;
            key(cm, cc, t, KeyEvent::BtnEnc);
        }
        assert!(active(cm));
        assert!(
            cm.config.display_active == DisplayActive::Vario,
            "the page shows, not the setup"
        );
    }

    /// lower knob to the zone
    fn go_to(cm: &mut CoreModel, cc: &mut CoreController, t: &mut u16, z: Zone) {
        for _ in 0..14 {
            if zone(cm) == z {
                return;
            }
            key(cm, cc, t, ZONE_FWD);
        }
        panic!("zone {:?} not reached", z);
    }

    /// upper knob: turn the choice of the zone to the row with this label
    fn turn_to(cm: &mut CoreModel, cc: &mut CoreController, t: &mut u16, label: &str) {
        if cm.control.layout.list == ListKind::None {
            // the first click opens the list (and moves on one row)
            key(cm, cc, t, VAL_FWD);
        }
        let r = rows(cm);
        let want = r
            .iter()
            .position(|r| r.label == label)
            .unwrap_or_else(|| panic!("no row {label}"));
        while (cm.control.layout.sel as usize) < want {
            key(cm, cc, t, VAL_FWD);
        }
        while (cm.control.layout.sel as usize) > want {
            key(cm, cc, t, VAL_BACK);
        }
    }

    /// Esc, then the row of the exit question
    fn exit(cm: &mut CoreModel, cc: &mut CoreController, t: &mut u16, label: &str) {
        key(cm, cc, t, KeyEvent::BtnEsc);
        assert!(cm.control.layout.list == ListKind::Exit);
        turn_to(cm, cc, t, label);
        key(cm, cc, t, KeyEvent::BtnEnc);
    }

    #[test]
    fn the_lower_knob_walks_the_zones_and_the_upper_one_changes_them() {
        let (mut cm, mut cc, mut t) = setup();
        open(&mut cm, &mut cc, &mut t);
        assert_eq!(cm.config.overlay_active, OverlayActive::None);
        assert_eq!(zone(&cm), Zone::Scale);
        let n = zones(&cm, cm.config.page()).len();
        for _ in 0..n {
            key(&mut cm, &mut cc, &mut t, ZONE_FWD);
        }
        assert_eq!(zone(&cm), Zone::Scale, "the round wraps");
        key(&mut cm, &mut cc, &mut t, ZONE_BACK);
        assert_eq!(zone(&cm), Zone::Exit, "the last zone is Save & exit");
        // upper knob on Core: the list opens and the page shows the next core
        go_to(&mut cm, &mut cc, &mut t, Zone::Core);
        assert!(cm.config.page().core == PageCore::Boxes);
        key(&mut cm, &mut cc, &mut t, VAL_FWD);
        assert_eq!(cm.control.layout.list, ListKind::Zone);
        assert!(
            cm.config.page().core == PageCore::BigBoxes,
            "live at the first click"
        );
        assert_eq!(zone(&cm), Zone::Core);
        // a pause closes the list, the choice stays
        tick(&mut cm, &mut cc, &mut t, 3000);
        assert_eq!(cm.control.layout.list, ListKind::None);
        assert!(cm.config.page().core == PageCore::BigBoxes);
        assert_eq!(zone(&cm), Zone::Core);
        // the lower knob closes an open list as well and moves on
        key(&mut cm, &mut cc, &mut t, VAL_BACK);
        assert!(cm.config.page().core == PageCore::Boxes);
        key(&mut cm, &mut cc, &mut t, ZONE_FWD);
        assert_eq!(cm.control.layout.list, ListKind::None);
        assert_eq!(zone(&cm), Zone::Mode);
        // nothing written while editing
        assert_eq!(cc.page_save, 0);
    }

    /// The centering page (Stefan 16.11.2026): while circling the large
    /// centering aid with the box above and the box below it, in cruise
    /// the info boxes page with its four boxes.
    #[test]
    fn the_centering_page_has_two_boxes_circling_and_four_in_cruise() {
        use crate::view::viewable::centerview::CenterView;
        let (mut cm, mut cc, mut t) = setup();
        open(&mut cm, &mut cc, &mut t);
        let before = cm.config.page().center_circling;
        assert!(
            !before.is_centering_aid(),
            "the test starts from a wind arrow in the centre"
        );
        go_to(&mut cm, &mut cc, &mut t, Zone::Core);
        turn_to(&mut cm, &mut cc, &mut t, "Centering aid");
        assert!(cm.config.page().core == PageCore::ThermalAssist);
        assert!(
            cm.config.page().center_circling == CenterView::ThermalAssistant3,
            "the 3D ring to begin with"
        );
        // turning on to another core gives the page its centre view back
        turn_to(&mut cm, &mut cc, &mut t, "Wind");
        assert!(cm.config.page().center_circling == before);
        turn_to(&mut cm, &mut cc, &mut t, "Centering aid");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert_eq!(cm.control.layout.list, ListKind::None);
        assert_eq!(
            zone(&cm),
            Zone::Core,
            "the zone stays when the core changes"
        );

        // circling set: the two boxes around the large aid
        let boxes = |cm: &CoreModel| {
            zones(cm, cm.config.page())
                .iter()
                .filter(|z| matches!(z, Zone::Box(_)))
                .count()
        };
        assert!(!cm.control.layout.cruise);
        assert_eq!(boxes(&cm), 2);
        assert!(zones(&cm, cm.config.page()).contains(&Zone::Mode));
        assert!(
            crate::view::vario_ring::centering_large(&cm),
            "the editor shows the large aid for the circling set"
        );

        // the centre: one of the four centering aids, no wind arrow, no "Off"
        go_to(&mut cm, &mut cc, &mut t, Zone::Centre);
        key(&mut cm, &mut cc, &mut t, VAL_FWD);
        let labels: Vec<&str, MAX_ROWS> = rows(&cm).iter().map(|r| r.label).collect();
        assert_eq!(
            labels.as_slice(),
            [
                "Centering, dots",
                "Centering, spider",
                "Centering, 3D ring",
                "Centering, track"
            ]
        );
        assert_eq!(
            cm.control.layout.orig_sel, 2,
            "the 3D ring is marked as the choice"
        );
        // the track is an aid of the centering page only: another core
        // takes the 3D ring for its centre circle
        turn_to(&mut cm, &mut cc, &mut t, "Centering, track");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert!(cm.config.page().center_circling == CenterView::ThermalAssistant4);
        go_to(&mut cm, &mut cc, &mut t, Zone::Core);
        key(&mut cm, &mut cc, &mut t, VAL_FWD);
        turn_to(&mut cm, &mut cc, &mut t, "Info boxes");
        assert!(
            cm.config.page().center_circling == CenterView::ThermalAssistant3,
            "no track in the centre circle"
        );
        turn_to(&mut cm, &mut cc, &mut t, "Centering aid");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert!(
            cm.config.page().center_circling == CenterView::ThermalAssistant4,
            "back on the centering page: the track again"
        );
        go_to(&mut cm, &mut cc, &mut t, Zone::Centre);
        key(&mut cm, &mut cc, &mut t, VAL_FWD);
        turn_to(&mut cm, &mut cc, &mut t, "Centering, spider");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert!(cm.config.page().center_circling == CenterView::ThermalAssistant2);

        // cruise set: the info boxes page with four boxes and a wind arrow
        go_to(&mut cm, &mut cc, &mut t, Zone::Mode);
        turn_to(&mut cm, &mut cc, &mut t, "Cruise");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert!(cm.control.layout.cruise);
        assert_eq!(zone(&cm), Zone::Mode);
        assert_eq!(boxes(&cm), 4);
        assert!(
            !crate::view::vario_ring::centering_large(&cm),
            "cruise: the info boxes view"
        );
        go_to(&mut cm, &mut cc, &mut t, Zone::Centre);
        key(&mut cm, &mut cc, &mut t, VAL_FWD);
        assert!(
            rows(&cm).iter().all(|r| !r.label.starts_with("Centering")),
            "no centering aid in cruise"
        );
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEsc);
        go_to(&mut cm, &mut cc, &mut t, Zone::Box(3));

        // back to the circling set from the last cruise box: the zone
        // index stays valid
        go_to(&mut cm, &mut cc, &mut t, Zone::Mode);
        turn_to(&mut cm, &mut cc, &mut t, "Circling");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert_eq!(boxes(&cm), 2);
        let n = zones(&cm, cm.config.page()).len();
        for _ in 0..n {
            key(&mut cm, &mut cc, &mut t, ZONE_FWD);
            assert!(
                !matches!(zone(&cm), Zone::Box(2) | Zone::Box(3)),
                "no right boxes around the large aid"
            );
        }
    }

    #[test]
    fn esc_in_the_list_takes_the_choice_back() {
        let (mut cm, mut cc, mut t) = setup();
        open(&mut cm, &mut cc, &mut t);
        go_to(&mut cm, &mut cc, &mut t, Zone::Core);
        turn_to(&mut cm, &mut cc, &mut t, "Status page");
        assert_eq!(cm.config.display_active, DisplayActive::DeviceInfo);
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEsc);
        assert!(cm.config.page().core == PageCore::Boxes);
        assert_eq!(cm.config.display_active, DisplayActive::Vario);
        assert_eq!(cm.control.layout.list, ListKind::None);
        assert!(!changed(&cm));
    }

    #[test]
    fn save_writes_the_pages_discard_restores_them() {
        let (mut cm, mut cc, mut t) = setup();
        let before = cm.config.pages;
        open(&mut cm, &mut cc, &mut t);
        go_to(&mut cm, &mut cc, &mut t, Zone::Box(0));
        turn_to(&mut cm, &mut cc, &mut t, LineView::TrueAirSpeed.name());
        tick(&mut cm, &mut cc, &mut t, 3000);
        assert!(changed(&cm));
        exit(&mut cm, &mut cc, &mut t, "Discard changes");
        assert!(!active(&cm));
        assert!(cm.config.pages == before, "discarded");
        assert_eq!(cc.page_save, 0);
        // again, now saved
        open(&mut cm, &mut cc, &mut t);
        go_to(&mut cm, &mut cc, &mut t, Zone::Box(0));
        turn_to(&mut cm, &mut cc, &mut t, LineView::TrueAirSpeed.name());
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        exit(&mut cm, &mut cc, &mut t, "Save & exit");
        assert!(!active(&cm));
        assert!(
            cm.config.pages[0].boxes[0] == LineView::TrueAirSpeed,
            "got {}",
            cm.config.pages[0].boxes[0].name()
        );
        // (the mark itself is not asserted: the 20 ms after the key may already
        // cross the 100 ms tick that writes it - it depends on the number of
        // rows turned through, one more since the Thermal Average box)
        tick(&mut cm, &mut cc, &mut t, 500);
        assert_eq!(cc.page_save, 0, "written");
        // keys reach the page again: a push opens the flight menu
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert_eq!(cm.config.overlay_active, OverlayActive::Menu);
    }

    #[test]
    fn the_save_and_exit_zone_asks_and_go_on_stays() {
        let (mut cm, mut cc, mut t) = setup();
        open(&mut cm, &mut cc, &mut t);
        go_to(&mut cm, &mut cc, &mut t, Zone::Exit);
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert_eq!(cm.control.layout.list, ListKind::Exit);
        // nothing changed: Exit / Go on editing
        assert_eq!(rows(&cm).len(), 2);
        turn_to(&mut cm, &mut cc, &mut t, "Continue");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert!(active(&cm) && cm.control.layout.list == ListKind::None);
        key(&mut cm, &mut cc, &mut t, VAL_FWD);
        assert_eq!(
            cm.control.layout.list,
            ListKind::Exit,
            "turning on it asks as well"
        );
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert!(!active(&cm));
    }

    #[test]
    fn the_flight_mode_zone_switches_to_the_cruise_set() {
        let (mut cm, mut cc, mut t) = setup();
        let i = cm.config.page_idx as usize;
        cm.config.pages[i].cruise_boxes = false;
        let circling = cm.config.pages[i].boxes;
        open(&mut cm, &mut cc, &mut t);
        go_to(&mut cm, &mut cc, &mut t, Zone::Mode);
        turn_to(&mut cm, &mut cc, &mut t, "Cruise");
        assert!(cm.control.layout.cruise);
        assert!(cruise_shared(&cm), "no own cruise set yet");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        go_to(&mut cm, &mut cc, &mut t, Zone::Box(1));
        turn_to(&mut cm, &mut cc, &mut t, LineView::SpeedToFly.name());
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        let p = cm.config.pages[i];
        assert!(p.cruise_boxes, "own cruise set");
        assert!(p.stf_boxes[1] == LineView::SpeedToFly);
        assert!(p.stf_boxes[0] == circling[0], "copy of the circling set");
        assert!(p.boxes[1] == circling[1], "circling set unchanged");
        // back to "cruise = circling"
        go_to(&mut cm, &mut cc, &mut t, Zone::Mode);
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        turn_to(&mut cm, &mut cc, &mut t, "Cruise = circling");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert!(!cm.config.pages[i].cruise_boxes);
        assert!(cruise_shared(&cm));
    }

    #[test]
    fn the_page_zone_shows_the_other_pages_and_moves_them() {
        let (mut cm, mut cc, mut t) = setup();
        let n0 = on_pages(&cm).len();
        let first = cm.config.pages[0];
        open(&mut cm, &mut cc, &mut t);
        go_to(&mut cm, &mut cc, &mut t, Zone::Page);
        // turning goes through the pages
        turn_to(&mut cm, &mut cc, &mut t, "Page 2");
        assert_eq!(cm.config.page_idx, 1);
        assert!(cm.config.page().core == PageCore::Horizon);
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEsc);
        assert_eq!(cm.config.page_idx, 0, "Esc: back to the first page");
        // add a copy after it, move it to the front, delete it
        let page_cmd = |cm: &mut CoreModel, cc: &mut CoreController, t: &mut u16, label: &str| {
            go_to(cm, cc, t, Zone::Page);
            key(cm, cc, t, KeyEvent::BtnEnc);
            turn_to(cm, cc, t, label);
            key(cm, cc, t, KeyEvent::BtnEnc);
        };
        page_cmd(&mut cm, &mut cc, &mut t, "Add page after");
        assert_eq!(on_pages(&cm).len(), n0 + 1);
        assert_eq!(cm.config.page_idx, 1, "the copy is shown");
        assert!(cm.config.pages[1] == first);
        assert_eq!(zone(&cm), Zone::Page);
        cm.config.pages[1].light_outer = !first.light_outer;
        page_cmd(&mut cm, &mut cc, &mut t, "Move left");
        assert_eq!(cm.config.page_idx, 0);
        assert!(cm.config.pages[0].light_outer != first.light_outer);
        page_cmd(&mut cm, &mut cc, &mut t, "Delete page");
        assert_eq!(on_pages(&cm).len(), n0);
        assert!(cm.config.pages[0] == first);
        assert!(!changed(&cm), "back as it was");
        // the lower knob still walks the zones on another page
        page_cmd(&mut cm, &mut cc, &mut t, "Page 2");
        assert_eq!(cm.config.page_idx, 1);
        assert_eq!(
            zones(&cm, cm.config.page()).as_slice(),
            &[Zone::Scale, Zone::Core, Zone::Page, Zone::Exit]
        );
        assert_eq!(zone(&cm), Zone::Page);
    }

    #[test]
    fn without_esc_a_long_push_asks_and_turning_back_undoes() {
        let (mut cm, mut cc, mut t) = setup();
        open(&mut cm, &mut cc, &mut t);
        go_to(&mut cm, &mut cc, &mut t, Zone::Core);
        key(&mut cm, &mut cc, &mut t, VAL_FWD);
        assert!(cm.config.page().core == PageCore::BigBoxes);
        assert_eq!(cm.control.layout.orig_sel, 0, "the former choice is marked");
        // a long push in the open list: choice back, the exit question
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEncS3);
        assert!(cm.config.page().core == PageCore::Boxes);
        assert_eq!(cm.control.layout.list, ListKind::Exit);
        assert_eq!(rows(&cm).len(), 2, "nothing changed: Exit / Continue");
        key(&mut cm, &mut cc, &mut t, KeyEvent::BtnEnc);
        assert!(!active(&cm));
    }

    #[test]
    fn the_radar_page_gives_both_knobs_to_the_editor() {
        let (mut cm, mut cc, mut t) = setup();
        let i = cm.config.page_idx as usize;
        cm.config.pages[i].core = PageCore::Radar;
        open(&mut cm, &mut cc, &mut t);
        key(&mut cm, &mut cc, &mut t, ZONE_FWD);
        assert_eq!(zone(&cm), Zone::Core);
        key(&mut cm, &mut cc, &mut t, VAL_FWD);
        assert_eq!(cm.control.layout.list, ListKind::Zone);
    }
}
