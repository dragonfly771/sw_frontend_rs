use crate::{model::CoreModel, utils::Colors, CoreError, DrawImage, EditMode};

use embedded_graphics::draw_target::DrawTarget;

#[derive(PartialEq)]
pub struct MenuView {}

impl MenuView {
    pub fn new() -> MenuView {
        MenuView {}
    }

    pub fn draw<D>(
        &self,
        display: &mut D,
        cm: &CoreModel,
        secondary_view: bool,
    ) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
    {
        if secondary_view && cm.device_const.misc.edit_mode == EditMode::CircleArea {
            // inner circle overlay, the scale keeps running
            crate::view::ring_menu::draw_flight_list(display, cm)
        } else {
            self.draw_fullscreen_menu(display, cm)
        }
    }

    /// fisheye list with icons and value preview
    pub fn draw_fullscreen_menu<D>(&self, display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
    {
        crate::view::ring_menu::draw_setup_list(display, cm)
    }
}
