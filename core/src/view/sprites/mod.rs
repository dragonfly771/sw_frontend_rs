pub(crate) mod arrow;
pub(crate) mod menu_icons;
pub(crate) mod polar_defs;
pub(crate) mod ring_needle;
pub(crate) mod simple_indicator;

pub(crate) use arrow::Arrow;
pub(crate) use polar_defs::*;
pub(crate) use ring_needle::{RingArrow, RingMarker, RingNeedle};
pub(crate) use simple_indicator::SimpleIndicator;

use crate::{utils::Colors, CoreError};
use embedded_graphics::{
    prelude::{DrawTarget, Point},
    primitives::PrimitiveStyle,
};

#[allow(unused_imports)]
use micromath::F32Ext;

#[derive(Clone, Copy)]
pub struct PolarCoordinate {
    pub len: f32,
    pub alpha: f32,
}

impl PolarCoordinate {
    #[allow(clippy::wrong_self_convention)]
    pub fn to_xy(&self, scale: f32, rotation: f32) -> Point {
        let a = self.alpha + rotation;
        let l = self.len * scale;
        let x = a.sin() * l;
        let y = -a.cos() * l;
        Point::new(x as i32, y as i32)
    }

    pub fn rotate(&mut self, rotation: f32) {
        self.alpha += rotation;
    }
}

pub trait Rotate {
    fn rotate(&mut self, rotation: f32) -> &mut Self;
    #[allow(unused)]
    fn zero_pos(&mut self, zero_pos: f32) -> &mut Self;
}

pub trait DrawStyled {
    fn draw_styled<D>(
        &self,
        style: PrimitiveStyle<Colors>,
        display: &mut D,
    ) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError>;
}

pub trait DrawColored {
    fn draw_colored<D>(&self, color: Colors, display: &mut D) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError>;
}

pub mod pos {
    use core::f32::consts::PI;

    #[allow(unused)]
    pub const TWELVE_O_CLOCK: f32 = 0.0;
    #[allow(unused)]
    pub const THREE_O_CLOCK: f32 = 0.5 * PI;
    #[allow(unused)]
    pub const SIX_O_CLOCK: f32 = PI;
    #[allow(unused)]
    pub const NINE_O_CLOCK: f32 = 1.5 * PI;
}
