use crate::Display;
use corelib::*;

/// Display refresh period of the V2 panel: 527 x 509 pixel clocks at 9 MHz
/// (LTDC timing registers) = 29.8 ms, 33.5 Hz. The view task runs once per
/// refresh, spawned by the LTDC line interrupt.
pub const LTDC_FRAME_S: f32 = 527.0 * 509.0 / 9.0e6;

pub struct DevView {
    core_view: CoreView<Display>,
}

impl DevView {
    pub fn new(display: Display, core_model: &CoreModel) -> Self {
        let core_view = CoreView::new(display, core_model);
        DevView { core_view }
    }

    pub fn core(&mut self) -> &mut CoreView<Display> {
        &mut self.core_view
    }
}
