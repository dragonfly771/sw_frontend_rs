//! Name entry with the alphabet in the outer ring (plan_klappen_menue.md ch. 2)
//!
//! Used for every `Params::String` editable (today: the flap stage name).
//! Two pages: the main page holds only the alphabet, numbers and special
//! characters are on the second one. The ring is fixed: the first character
//! (A resp. 0) sits at 9 o'clock like the zero of the vario scale, the
//! others follow clockwise; the keys sit just before 9 o'clock.
//!
//! - knob with the push (`Rotary2`): move the selection around the ring
//! - the other knob (`Rotary1`): switch the page (selection on its first
//!   character)
//! - push: add the character / execute the key (`123` `ABC` `<` `OK`)
//! - long push: take the name (like OK), Esc: leave without change
//! - when the name is full, the selection jumps to OK
//! - the placeholder name "-" of a new stage is not kept: the entry starts
//!   empty (the first letter came after the "-" before, 28.10.2026)

use crate::{
    controller::{close_edit_frame, helpers::IntToDuration, KeyEvent, Timer},
    model::editable::{Content, Params},
    utils::TString,
    CoreController, CoreModel, Editable,
};

/// characters of the main page
pub const PAGE_ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// characters of the second page (the last one is the blank)
pub const PAGE_123: &[u8] = b"0123456789+-./ ";
/// the editor stays open this long without input
pub const NAME_TIMEOUT_S: u16 = 30;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Char(u8),
    /// switch to page 0 (ABC) or 1 (123)
    Page(u8),
    Back,
    Ok,
}

fn chars(page: u8) -> &'static [u8] {
    if page == 0 {
        PAGE_ABC
    } else {
        PAGE_123
    }
}

/// number of fields of a page (characters + 3 keys): 29 resp. 18
pub fn n_fields(page: u8) -> usize {
    chars(page).len() + 3
}

pub fn field(page: u8, i: usize) -> Field {
    let c = chars(page);
    if i < c.len() {
        Field::Char(c[i])
    } else {
        match i - c.len() {
            0 => Field::Page(1 - page.min(1)),
            1 => Field::Back,
            _ => Field::Ok,
        }
    }
}

pub fn ok_index(page: u8) -> usize {
    n_fields(page) - 1
}

/// state of the name entry (part of the editor)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NameEntry {
    pub page: u8,
    pub sel: u8,
    /// maximum length of the name
    pub max_len: u8,
}

impl NameEntry {
    pub const fn new() -> Self {
        NameEntry {
            page: 0,
            sel: 0,
            max_len: 3,
        }
    }

    /// start a new entry for `target`
    pub fn start(target: Editable) -> Self {
        let max_len = match target {
            Editable::FlapName => crate::model::flaps::FLAP_NAME_LEN as u8,
            _ => 11,
        };
        NameEntry {
            page: 0,
            sel: 0,
            max_len,
        }
    }

    fn step(&mut self, d: i32) {
        let n = n_fields(self.page) as i32;
        self.sel = (self.sel as i32 + d).rem_euclid(n) as u8;
    }

    fn set_page(&mut self, page: u8) {
        self.page = page.min(1);
        self.sel = 0;
    }
}

impl Default for NameEntry {
    fn default() -> Self {
        Self::new()
    }
}

fn text_of(cm: &CoreModel) -> heapless::String<12> {
    let mut s = heapless::String::new();
    if let Content::String(t) = cm.control.editor.content {
        let _ = s.push_str(t.as_str());
    }
    s
}

/// the text a new entry starts with: the name as it is, but not the
/// placeholder of an unnamed stage
pub fn start_text(name: &str) -> &str {
    if name.trim() == "-" {
        ""
    } else {
        name
    }
}

fn set_text(cm: &mut CoreModel, s: &str) {
    cm.control.editor.content = Content::String(TString::<12>::from_str(s));
    cm.control.editor.touched = true;
}

/// true while a name entry is open
pub fn active(cm: &CoreModel) -> bool {
    cm.control.editor.mode != crate::EditMode::Off
        && matches!(cm.control.editor.params, Params::String(_))
}

/// Pure key logic on the entry state and the text; returns true when the
/// name is to be taken, false otherwise. `None` for keys it does not use.
pub fn apply_key(
    ne: &mut NameEntry,
    text: &mut heapless::String<12>,
    key: KeyEvent,
) -> Option<bool> {
    match key {
        KeyEvent::Rotary2Left => ne.step(-1),
        KeyEvent::Rotary2Right => ne.step(1),
        KeyEvent::Rotary1Left | KeyEvent::Rotary1Right => {
            let p = 1 - ne.page.min(1);
            ne.set_page(p);
        }
        KeyEvent::BtnEnc => match field(ne.page, ne.sel as usize) {
            Field::Char(c) => {
                if text.len() < ne.max_len as usize {
                    let _ = text.push(c as char);
                }
                if text.len() >= ne.max_len as usize {
                    ne.sel = ok_index(ne.page) as u8;
                }
            }
            Field::Page(p) => ne.set_page(p),
            Field::Back => {
                let _ = text.pop();
            }
            Field::Ok => return Some(true),
        },
        KeyEvent::BtnEncS3 => return Some(true),
        _ => return None,
    }
    Some(false)
}

/// Key handling while the name entry is open. Consumes every key.
pub fn key_action(key_event: &mut KeyEvent, cm: &mut CoreModel, cc: &mut CoreController) {
    let key = *key_event;
    *key_event = KeyEvent::NoEvent;
    if key == KeyEvent::BtnEsc {
        // leave without a change
        cm.control.editor.touched = false;
        let _ = cc.scheduler.stop(Timer::CloseEditFrame, false);
        close_edit_frame(cm, cc);
        return;
    }
    cc.scheduler
        .after(Timer::CloseEditFrame, NAME_TIMEOUT_S.secs());
    let mut ne = cm.control.editor.name;
    let mut text = text_of(cm);
    let take = apply_key(&mut ne, &mut text, key);
    cm.control.editor.name = ne;
    set_text(cm, text.as_str());
    if take == Some(true) {
        let target = cm.control.editor.target;
        target.set_content(cm, cc, cm.control.editor.content);
        let _ = cc.scheduler.stop(Timer::CloseEditFrame, false);
        close_edit_frame(cm, cc);
        crate::controller::menu::fill_values(cm, cc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_and_keys() {
        assert_eq!(n_fields(0), 29);
        assert_eq!(n_fields(1), 18);
        assert_eq!(field(0, 0), Field::Char(b'A'));
        assert_eq!(field(0, 26), Field::Page(1));
        assert_eq!(field(0, 28), Field::Ok);
        assert_eq!(field(1, 0), Field::Char(b'0'));
        assert_eq!(field(1, 15), Field::Page(0));
        assert_eq!(field(1, 16), Field::Back);
    }

    #[test]
    fn a_new_stage_starts_without_the_placeholder() {
        assert_eq!(start_text("-"), "");
        assert_eq!(start_text("L"), "L");
    }

    #[test]
    fn typing_a_flap_name() {
        let mut ne = NameEntry::start(Editable::FlapName);
        let mut t: heapless::String<12> = heapless::String::new();
        // "L": 11 steps right, push
        for _ in 0..11 {
            apply_key(&mut ne, &mut t, KeyEvent::Rotary2Right);
        }
        apply_key(&mut ne, &mut t, KeyEvent::BtnEnc);
        assert_eq!(t.as_str(), "L");
        // to the number page, "+" is index 10
        apply_key(&mut ne, &mut t, KeyEvent::Rotary1Right);
        assert_eq!((ne.page, ne.sel), (1, 0));
        for _ in 0..10 {
            apply_key(&mut ne, &mut t, KeyEvent::Rotary2Right);
        }
        apply_key(&mut ne, &mut t, KeyEvent::BtnEnc);
        assert_eq!(t.as_str(), "L+");
        // left from 0 wraps to OK
        apply_key(&mut ne, &mut t, KeyEvent::Rotary1Left);
        apply_key(&mut ne, &mut t, KeyEvent::Rotary2Left);
        assert_eq!(field(ne.page, ne.sel as usize), Field::Ok);
        // back key deletes
        apply_key(&mut ne, &mut t, KeyEvent::Rotary2Left);
        apply_key(&mut ne, &mut t, KeyEvent::BtnEnc);
        assert_eq!(t.as_str(), "L");
        // two more chars: full -> selection on OK, OK takes it
        ne.sel = 0;
        apply_key(&mut ne, &mut t, KeyEvent::BtnEnc);
        apply_key(&mut ne, &mut t, KeyEvent::BtnEnc);
        assert_eq!(t.as_str(), "LAA");
        assert_eq!(field(ne.page, ne.sel as usize), Field::Ok);
        ne.sel = 0;
        apply_key(&mut ne, &mut t, KeyEvent::BtnEnc);
        assert_eq!(t.as_str(), "LAA", "no fourth character");
        assert_eq!(field(ne.page, ne.sel as usize), Field::Ok);
        assert_eq!(apply_key(&mut ne, &mut t, KeyEvent::BtnEnc), Some(true));
    }
}
