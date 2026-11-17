use crate::{model::CoreModel, utils::Colors, CoreError, DeviceEvent, DrawImage};

use embedded_graphics::draw_target::DrawTarget;

#[derive(PartialEq)]
pub struct SwUpdate {}

impl SwUpdate {
    pub fn new(_update_state: DeviceEvent) -> SwUpdate {
        SwUpdate {}
    }

    pub fn draw<D>(&self, display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
    {
        // progress ring instead of a dialog box
        let note = match cm.control.firmware_update_state {
            DeviceEvent::UploadInProgress => "do not power off",
            DeviceEvent::PrepareFwUpload => "preparing",
            _ => "",
        };
        crate::view::ring_editor::draw_progress(display, cm, "Firmware Update", None, note)
    }
}
