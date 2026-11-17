use crate::{
    model::{device_info::DEVICE_INFO_CONTENT, CoreModel},
    utils::Colors,
    CoreError, DrawImage,
};

use embedded_graphics::draw_target::DrawTarget;
#[allow(unused_imports)]
use micromath::F32Ext;

/// number of visible device info lines - they have to stay inside the circle
pub const DEVICE_INFO_LINES: usize = 7;

/// number of visible device info lines for the active design
pub fn device_info_lines(_cm: &CoreModel) -> usize {
    DEVICE_INFO_LINES
}

#[derive(PartialEq)]
pub struct DeviceInfo {}

impl DeviceInfo {
    pub fn new() -> DeviceInfo {
        DeviceInfo {}
    }

    pub fn draw<D>(
        &mut self,
        display: &mut D,
        cm: &CoreModel,
        cm_1s: &CoreModel,
    ) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
    {
        // same look as the menus - name left, value right, rows follow the
        // round display
        display.clear(cm.palette().vario.background)?;
        let center = cm.device_const.sizes.display.center;
        // geometry of the sketch: r 232, 7 rows of 50 px
        let radius = cm.device_const.sizes.display.height as i32 / 2 - 8;
        let pitch = 50;
        for pos in 0..DEVICE_INFO_LINES {
            let dy = (pos as i32 - (DEVICE_INFO_LINES as i32 - 1) / 2) * pitch;
            let r6 = radius - 6;
            let inner = (r6 * r6 - (dy.abs() + 22) * (dy.abs() + 22)).max(0);
            let half = crate::view::ring_menu::isqrt(inner) - 16;
            if half < 70 {
                continue;
            }
            let index = pos + cm.control.device_info_control.index as usize;
            if index >= DEVICE_INFO_CONTENT.len() {
                break;
            }
            DEVICE_INFO_CONTENT[index].draw_ring(display, cm_1s, center.x, center.y + dy, half)?;
        }
        // position like in the setup lists (no arc at the rim any more)
        let n = DEVICE_INFO_CONTENT.len().saturating_sub(DEVICE_INFO_LINES) + 1;
        crate::view::ring_menu::list_position(
            display,
            center,
            216,
            cm.control.device_info_control.index as usize,
            n,
        )?;
        Ok(())
    }
}
