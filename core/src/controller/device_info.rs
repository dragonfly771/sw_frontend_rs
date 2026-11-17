use crate::{
    model::{device_info::DEVICE_INFO_CONTENT, DisplayActive},
    view::{device_info::device_info_lines, viewable::device_lineview::DeviceLineView},
    CoreController, CoreModel, KeyEvent,
};

#[derive(Clone, Copy)]
pub struct DeviceInfoControl {
    pub index: u32,
    pub content: &'static [DeviceLineView],
}

impl Default for DeviceInfoControl {
    fn default() -> Self {
        DeviceInfoControl {
            index: 0,
            content: DEVICE_INFO_CONTENT,
        }
    }
}

pub fn key_action(key_event: &mut KeyEvent, cm: &mut CoreModel, _cc: &mut CoreController) {
    if cm.config.display_active != DisplayActive::DeviceInfo {
        return;
    }

    match key_event {
        KeyEvent::Rotary2Left => {
            if cm.control.device_info_control.index > 0 {
                cm.control.device_info_control.index -= 1;
            }
        }
        KeyEvent::Rotary2Right => {
            if cm.control.device_info_control.index
                < (DEVICE_INFO_CONTENT.len() - device_info_lines(cm)) as u32
            {
                cm.control.device_info_control.index += 1;
            }
        }
        // opened from Setup > System > Device Info: Esc goes back to the
        // active page (a page showing the device info itself stays)
        KeyEvent::BtnEsc => {
            crate::controller::editor::apply_page(cm);
            *key_event = KeyEvent::NoEvent;
        }
        _ => (),
    }
}
