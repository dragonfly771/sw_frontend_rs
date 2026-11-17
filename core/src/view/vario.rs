use super::thermal_data::ThermalData;
use crate::{model::CoreModel, utils::Colors, CoreError, DrawImage};

use embedded_graphics::prelude::*;

/// The vario page. The thermal data (sectors of the circle, track through
/// the air) belong to `CoreView`, not to this page: they are recorded all
/// the time, whatever is on the display.
#[derive(PartialEq)]
pub struct Vario {}

impl Vario {
    pub fn new() -> Vario {
        Vario {}
    }

    pub fn draw<D>(
        &mut self,
        display: &mut D,
        cm: &CoreModel,
        cm_1s: &CoreModel,
        thermal_data: &mut ThermalData,
    ) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
    {
        super::vario_ring::draw_ring(display, cm, cm_1s, thermal_data)
    }
}
