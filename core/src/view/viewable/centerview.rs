use crate::{
    model::CirclingDirection, Colors, CoreError, CoreModel, DrawImage, FloatToSpeed, FlyMode,
    VarioSizes,
};
use crate::{
    view::{
        sprites::{pos, Arrow, DrawColored, DrawStyled, PolarCoordinate, RingArrow, Rotate},
        thermal_data::{ActionArc, ThermalData, DELTA_ALPHA, HEAT, THERMAL_DATA_CNT},
        vario_ring::{CENTER_RING_IN, CENTER_RING_OUT},
    },
    Image,
};

use core::f32::consts::PI;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{AngleUnit, Point},
    prelude::{Angle, Primitive},
    primitives::{Arc, Circle, Line, PrimitiveStyle, PrimitiveStyleBuilder, Triangle},
    Drawable,
};
use num::clamp;

#[allow(unused_imports)]
use micromath::F32Ext;
use num_enum::FromPrimitive;

#[derive(Clone, Copy, PartialEq, FromPrimitive)]
#[repr(u8)]
pub enum CenterView {
    #[default]
    None,
    SingleArrowCircling,
    SingleArrowStraight,
    DoubleArrowCircling,
    DoubleArrowStraight,
    ThermalAssistant1,
    ThermalAssistant2,
    ThermalAssistant3,
    /// the track through the air (centering page only)
    ThermalAssistant4,
    LastElemntNotInUse,
}

const CIRCLING_CENTER_VIEW: [CenterView; 5] = [
    CenterView::SingleArrowCircling,
    CenterView::DoubleArrowCircling,
    CenterView::ThermalAssistant1,
    CenterView::ThermalAssistant2,
    CenterView::ThermalAssistant3,
];

const STRAIGHT_CENTER_VIEW: [CenterView; 2] = [
    CenterView::SingleArrowStraight,
    CenterView::DoubleArrowStraight,
];

// Limits of the wind arrow
const WIND_MIN: f32 = 10.0; // 10 km/h
const WIND_MAX: f32 = 30.0; // 30 km/h

pub enum CenterType {
    Circling,
    Straight,
}

impl CenterView {
    pub const fn max(center_type: CenterType) -> usize {
        match center_type {
            CenterType::Circling => CIRCLING_CENTER_VIEW.len() - 1,
            CenterType::Straight => STRAIGHT_CENTER_VIEW.len() - 1,
        }
    }

    // This method is used by the editor to obtain the correct viewables in the correct order
    pub fn from_sorted(value: usize, center_type: CenterType) -> CenterView {
        match center_type {
            CenterType::Circling => {
                if value < CIRCLING_CENTER_VIEW.len() {
                    return CIRCLING_CENTER_VIEW[value];
                }
            }
            CenterType::Straight => {
                if value < STRAIGHT_CENTER_VIEW.len() {
                    return STRAIGHT_CENTER_VIEW[value];
                }
            }
        }
        CenterView::None // should never happen
    }

    pub fn sorted_as_i32(&self, center_type: CenterType) -> i32 {
        match center_type {
            CenterType::Circling => {
                for (idx, view_item) in CIRCLING_CENTER_VIEW.iter().enumerate() {
                    if *self == *view_item {
                        return idx as i32;
                    };
                }
            }
            CenterType::Straight => {
                for (idx, view_item) in STRAIGHT_CENTER_VIEW.iter().enumerate() {
                    if *self == *view_item {
                        return idx as i32;
                    };
                }
            }
        }
        0 // should never happen
    }

    /// Get the name of a viewable
    pub fn name(&self) -> &'static str {
        match self {
            CenterView::SingleArrowCircling => "Single Arrow",
            CenterView::SingleArrowStraight => "Single Arrow",
            CenterView::DoubleArrowCircling => "Double Arrow",
            CenterView::DoubleArrowStraight => "Double Arrow",
            CenterView::ThermalAssistant1 => "Dotted Assistant",
            CenterView::ThermalAssistant2 => "Spider Assistant",
            CenterView::ThermalAssistant3 => "3D Ring Assistant",
            CenterView::ThermalAssistant4 => "Track Assistant",
            CenterView::None => "None",
            CenterView::LastElemntNotInUse => "",
        }
    }

    /// Draw a centering aid at another place than the centre circle (the
    /// large aid of the centering page). Anything else than a centering
    /// aid draws the 3D ring.
    pub fn draw_at<D>(
        &self,
        display: &mut D,
        cm: &CoreModel,
        thermal_data: &mut ThermalData,
        place: &AidPlace,
    ) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
    {
        match self {
            CenterView::ThermalAssistant1 => draw_dots_at(display, cm, thermal_data, place),
            CenterView::ThermalAssistant2 => draw_spider_aid_at(display, cm, thermal_data, place),
            CenterView::ThermalAssistant4 => draw_track_at(display, cm, thermal_data, place),
            _ => draw_ring3d_at(display, cm, thermal_data, place),
        }
    }

    /// a centering aid (dots, spider, 3D ring, track)
    pub fn is_centering_aid(&self) -> bool {
        matches!(
            self,
            CenterView::ThermalAssistant1
                | CenterView::ThermalAssistant2
                | CenterView::ThermalAssistant3
                | CenterView::ThermalAssistant4
        )
    }

    /// Draw viewable
    pub fn draw<D>(
        &self,
        display: &mut D,
        cm: &CoreModel,
        thermal_data: &mut ThermalData,
    ) -> Result<(), CoreError>
    where
        D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
    {
        match self {
            CenterView::None => Ok(()),
            CenterView::SingleArrowCircling => draw_single_arrow(display, cm),
            CenterView::SingleArrowStraight => draw_single_arrow(display, cm),
            CenterView::DoubleArrowCircling => draw_double_arrow(display, cm),
            CenterView::DoubleArrowStraight => draw_double_arrow(display, cm),
            CenterView::ThermalAssistant1 => draw_thermal_assitant1(display, cm, thermal_data),
            CenterView::ThermalAssistant2 => draw_thermal_assitant2(display, cm, thermal_data),
            CenterView::ThermalAssistant3 => draw_thermal_assitant3(display, cm, thermal_data),
            // the track is not offered for the centre circle (it would be
            // about 50 px wide there); a page that has it shows the 3D ring
            CenterView::ThermalAssistant4 => draw_thermal_assitant3(display, cm, thermal_data),
            CenterView::LastElemntNotInUse => Ok(()),
        }
    }
}

/// Where a centering aid is drawn and how large: in the centre circle of
/// the ring page, or large in the middle of the centering page (as large as
/// possible, it is easier to read then).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AidPlace {
    /// middle of the dots and of the spider
    pub center: Point,
    /// radius of the circle of dots and of the rim of the spider; the
    /// glider symbol sits on this circle
    pub radius: f32,
    /// diameter of the largest dot
    pub dot: u32,
    /// inner radius of the white ring around the spider (base of its pointer)
    pub ring: f32,
    /// the white ring around the spider has to be drawn (in the centre
    /// circle it is part of the scale picture)
    pub own_ring: bool,
    /// middle and radius of the circle the 3D ring fills
    pub ring3d_center: Point,
    pub ring3d_unit: f32,
    /// width of the edges of the 3D ring
    pub ring3d_edge: u32,
}

/// edges of the 3D ring: in the small centre circle and large
pub const RING_EDGE_SMALL: u32 = 2;
pub const RING_EDGE_LARGE: u32 = 3;
/// width of the white ring around a large spider
const OWN_RING_WIDTH: f32 = 3.0;

impl AidPlace {
    /// in the centre circle of the ring page
    pub fn centre_circle(cm: &CoreModel) -> Self {
        let sizes = &cm.device_const.sizes;
        let scl = sizes.display.width.min(sizes.display.height) as f32 / 480.0;
        AidPlace {
            center: sizes.display.center,
            radius: sizes.vario.ta_circle_radius as f32,
            dot: sizes.vario.ta_point_diameter,
            ring: CENTER_RING_IN * scl - 1.0,
            own_ring: false,
            ring3d_center: sizes.display.center,
            ring3d_unit: CENTER_RING_IN * scl - 2.0,
            ring3d_edge: RING_EDGE_SMALL,
        }
    }

    /// Large, between the heights `top` and `bottom` (display pixels) on
    /// the vertical through `x`: dots and spider in the circle that fits
    /// the band, with the proportions they have in the centre circle; the
    /// 3D ring, which is wider than high, fills the whole height of the
    /// band (its far wall and the arrow at the top, its near wall at the
    /// bottom).
    pub fn large(cm: &CoreModel, x: i32, top: f32, bottom: f32) -> Self {
        let small = Self::centre_circle(cm);
        let outer = 0.5 * (bottom - top);
        let ring = outer - OWN_RING_WIDTH;
        // the spider as far from its white ring as in the centre circle,
        // and the largest dot inside the band
        let k = (ring / small.ring).min(outer / (small.radius + 0.5 * small.dot as f32));
        // the 3D ring reaches RING_RY + RING_H above the middle of its
        // floor and RING_RY below it
        let unit = (bottom - top) / (2.0 * RING_RY + RING_H);
        let floor = top + (RING_RY + RING_H) * unit;
        AidPlace {
            center: Point::new(x, (0.5 * (top + bottom) + 0.5) as i32),
            radius: small.radius * k,
            dot: (small.dot as f32 * k + 0.5) as u32,
            ring,
            own_ring: true,
            ring3d_center: Point::new(x, (floor - RING_DROP * unit + 0.5) as i32),
            ring3d_unit: unit,
            ring3d_edge: RING_EDGE_LARGE,
        }
    }
}

/// Scale steps of the track view, metres per pixel of the large view on the
/// 480 px display. The first circles of 25 thermals of one flight (18 m
/// glider) were 229 to 696 m wide, the median 329 m, and took 2.0 to
/// 4.0 m per pixel with their 5 s of approach; the two finest steps are for
/// slow gliders in tight circles.
const TRACK_SCALES: [f32; 11] = [1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 12.0];
/// dot of the track view: the weakest and the strongest climb of the
/// track, and sink (pixels of the large view on the 480 px display)
const TRACK_DOT_MIN: f32 = 5.0;
const TRACK_DOT_MAX: f32 = 22.0;
const TRACK_DOT_SINK: f32 = 4.0;
/// radius of the ring of the large place on the 480 px display, the size
/// the constants of the track view refer to
const TRACK_REF_RING: f32 = 100.5;
/// the track view zooms in again when the track would fit the next finer
/// step with this much room to spare
const TRACK_ZOOM_IN: f32 = 0.8;
/// the colours of the track span at least this much climb (m/s)
const TRACK_MIN_SPAN: f32 = 1.0;

/// The track view (Stefan 16.11.2026): where the glider flew, seen from
/// above with the nose up, one dot per second. The stronger the climb, the
/// larger and the lighter the dot - dark red for the weakest climb of the
/// track, orange for the strongest; sink is a small grey dot. The best dot
/// since the turn began is yellow (not one of the approach: there the
/// reading still holds the pull-up).
///
/// The whole track always fits: its centre of gravity sits in the middle
/// of the place and the scale is the finest step that holds the farthest
/// point. The centre of gravity moves by a fraction of a pixel per new
/// dot, so the picture does not jump; in a steady circle it is the centre
/// of the circle and the glider sits at the side, as in the other aids.
fn draw_track_at<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &mut ThermalData,
    place: &AidPlace,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let trail = &mut thermal_data.trail;
    let n = trail.shown_len();
    let (here_e, here_n) = trail.position();

    // centre of gravity, range of the climb, best dot of the circle
    let (mut sum_e, mut sum_n) = (here_e, here_n);
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    let mut best: Option<usize> = None;
    let mut best_climb = 0.0_f32;
    for i in 0..n {
        let p = trail.point(i);
        sum_e += p.east;
        sum_n += p.north;
        low = low.min(p.climb);
        high = high.max(p.climb);
        if trail.in_circle(&p) && p.climb > best_climb {
            best_climb = p.climb;
            best = Some(i);
        }
    }
    let (mid_e, mid_n) = (sum_e / (n + 1) as f32, sum_n / (n + 1) as f32);
    let mut reach2 = (here_e - mid_e) * (here_e - mid_e) + (here_n - mid_n) * (here_n - mid_n);
    for i in 0..n {
        let p = trail.point(i);
        reach2 =
            reach2.max((p.east - mid_e) * (p.east - mid_e) + (p.north - mid_n) * (p.north - mid_n));
    }
    let reach = crate::math::sqrt(reach2);

    // the scale: out at once when the track does not fit, in again only
    // with room to spare
    let size = place.ring / TRACK_REF_RING;
    let fit = place.ring - 0.5 * TRACK_DOT_MAX * size - 1.0;
    let last = TRACK_SCALES.len() - 1;
    let need = TRACK_SCALES
        .iter()
        .position(|m| reach * size / m <= fit)
        .unwrap_or(last);
    let mut step = (trail.scale_step as usize).min(last);
    if need > step {
        step = need;
    } else if step > 0 && reach * size / TRACK_SCALES[step - 1] <= TRACK_ZOOM_IN * fit {
        step -= 1;
    }
    trail.scale_step = step as u8;
    let px_per_m = size / TRACK_SCALES[step];

    // nose up
    let yaw = cm.sensor.euler_yaw.to_radians();
    let (sin, cos) = (yaw.sin(), yaw.cos());
    let screen = |east: f32, north: f32| {
        let (de, dn) = (east - mid_e, north - mid_n);
        let right = de * cos - dn * sin;
        let ahead = de * sin + dn * cos;
        Point::new(
            place.center.x + (right * px_per_m + if right < 0.0 { -0.5 } else { 0.5 }) as i32,
            place.center.y - (ahead * px_per_m + if ahead < 0.0 { -0.5 } else { 0.5 }) as i32,
        )
    };

    let floor = low.max(0.0);
    let span = (high - floor).max(TRACK_MIN_SPAN);
    let dot = |display: &mut D, i: usize, gold: bool| -> Result<(), CoreError> {
        let p = trail.point(i);
        let (color, diameter) = if p.climb <= 0.0 {
            (Colors::DimGray, TRACK_DOT_SINK)
        } else {
            let t = clamp((p.climb - floor) / span, 0.0, 1.0);
            // more of the dark reds: the scale of HEAT turns orange early
            let shade = (29.0 * t * crate::math::sqrt(t) + 0.5) as usize;
            (
                HEAT[shade.min(HEAT.len() - 1)],
                TRACK_DOT_MIN + (TRACK_DOT_MAX - TRACK_DOT_MIN) * t,
            )
        };
        let color = if gold { Colors::Gold } else { color };
        Circle::with_center(
            screen(p.east, p.north),
            ((diameter * size + 0.5) as u32).max(2),
        )
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(display)
    };
    // the oldest first, so the newest lies on top where the track crosses
    // itself; the best one last of all
    for i in (0..n).rev() {
        if Some(i) != best {
            dot(display, i, false)?;
        }
    }
    if let Some(i) = best {
        dot(display, i, true)?;
    }

    let glider_img = Image::new(cm.device_const.images.small_glider);
    let p_gld = screen(here_e, here_n)
        - Point::new(
            (glider_img.width() / 2) as i32,
            (glider_img.height() / 2) as i32,
        );
    glider_img.draw(display, p_gld, Some(cm.palette().vario.scale))
}

/// The dot of the dotted assistant grows with the climb of its sector: the
/// smallest dot at `DOT_RANGE` m/s below the mean of the flown circle, the
/// largest at `DOT_RANGE` above it. (Before 0.4.1.52 the size followed the
/// amount of the difference, so a very bad sector was as large as a very
/// good one - Stefan 15.11.2026: "the worse, the smaller; the stronger,
/// the larger".) In a rough thermal the range widens to the largest
/// difference, so the best and the worst sector never look alike.
const DOT_RANGE: f32 = 2.0;
/// smallest dot as a fraction of the largest
const DOT_MIN: f32 = 0.24;

/// Diameter (px) of the dot of a sector `value` m/s above the mean of the
/// circle; `range` is the difference that gives the largest dot.
fn dot_diameter(value: f32, range: f32, largest: u32) -> u32 {
    let frac = clamp(0.5 + 0.5 * value / range.max(DOT_RANGE), 0.0, 1.0);
    let d = largest as f32 * (DOT_MIN + (1.0 - DOT_MIN) * frac);
    (d + 0.5) as u32
}

fn draw_thermal_assitant1<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &mut ThermalData,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    draw_dots_at(display, cm, thermal_data, &AidPlace::centre_circle(cm))
}

fn draw_dots_at<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &mut ThermalData,
    place: &AidPlace,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let mut pcoord = PolarCoordinate {
        alpha: 0.0,
        len: place.radius,
    };
    let delta = DELTA_ALPHA;

    let rotation = thermal_assistant_rotation(cm);
    thermal_data.prepare();
    let (low, high) = thermal_data.range();
    let range = high.max(-low);
    let largest = place.dot;
    for idx in 0..THERMAL_DATA_CNT {
        let center = pcoord.to_xy(1.0, rotation) + place.center;
        // a sector not flown yet in this circle: a small grey place holder
        let (fill_color, diameter) = match thermal_data.get_dotted_item(idx, cm) {
            Some((color, value)) => (color, dot_diameter(value, range, largest)),
            None => (
                Colors::DimGray,
                dot_diameter(-DOT_RANGE, DOT_RANGE, largest),
            ),
        };
        Circle::with_center(center, diameter)
            .into_styled(PrimitiveStyle::with_fill(fill_color))
            .draw(display)?;
        pcoord.rotate(delta);
    }
    // the pointer sits inside the ring of dots, so it hides none of them
    let base = place.radius - 0.5 * largest as f32 - 2.0;
    draw_core_pointer_at(display, cm, thermal_data, place, base)?;

    let glider_img = Image::new(cm.device_const.images.small_glider);
    let p_gld = thermal_assistant_glider_pos(cm, &glider_img, place.center, place.radius);
    glider_img.draw(display, p_gld, Some(cm.palette().vario.scale))
}

/// Sub steps per 15 degree sector of the spider outline
const SPIDER_STEPS: usize = 6;

/// Radius of the spider as a fraction of the circle radius: the ring of the
/// mean of the flown circle, the change per m/s and the smallest radius. At
/// `SPIDER_PER_MS` a sector 2 m/s better than the mean of the circle
/// reaches the rim, so the shape fills the centre circle (Stefan
/// 15.11.2026). It never exceeds the circle the glider symbol sits on.
const SPIDER_NULL: f32 = 0.7;
const SPIDER_PER_MS: f32 = 0.15;
const SPIDER_MIN: f32 = 0.3;

/// Radius change per m/s for the sectors of this circle: `SPIDER_PER_MS`,
/// less in a rough thermal so that the best sector just reaches the rim and
/// the worst the smallest radius. The shape is never cut off, and the
/// contour lines (1 m/s apart) move together to tell the larger scale.
fn spider_gain(low: f32, high: f32) -> f32 {
    let mut gain = SPIDER_PER_MS;
    if high > 0.0 {
        gain = gain.min((1.0 - SPIDER_NULL) / high);
    }
    if low < 0.0 {
        gain = gain.min((SPIDER_NULL - SPIDER_MIN) / -low);
    }
    gain
}

/// Radius of the spider for a climb `delta` (m/s relative to the mean of
/// the flown circle), as a fraction of the circle radius.
fn spider_scale(delta: f32, gain: f32) -> f32 {
    clamp(SPIDER_NULL + gain * delta, SPIDER_MIN, 1.0)
}

/// Outer edge (fraction of the circle radius) of colour band `k` of the
/// ramp `HEAT`: the bands share the radii from `SPIDER_MIN` to the rim, so
/// a colour always means the same climb.
fn band_edge(k: usize) -> f32 {
    SPIDER_MIN + (1.0 - SPIDER_MIN) * (k as f32 + 1.0) / HEAT.len() as f32
}

/// colour band a radius fraction lies in
fn band_of(scale: f32) -> usize {
    let k = ((scale - SPIDER_MIN) / (1.0 - SPIDER_MIN) * HEAT.len() as f32) as i32;
    k.clamp(0, HEAT.len() as i32 - 1) as usize
}

/// Contour lines every 1 m/s (relative to the mean of the circle): the
/// climb values, the ring of the mean itself is drawn dashed. Only the
/// lines between the smallest radius and the rim are drawn - three at the
/// usual scale, more when a rough thermal compresses it.
const SPIDER_CONTOURS: [f32; 8] = [-4.0, -3.0, -2.0, -1.0, 1.0, 2.0, 3.0, 4.0];
/// A contour line is drawn in the colour this many bands further in.
const CONTOUR_DARKER: usize = 6;
/// dashes of the average ring
const NULL_RING_DASHES: u32 = 24;

/// Catmull-Rom interpolation between `p1` and `p2` with the neighbours `p0`
/// and `p3`; the curve passes through the sector values, so an uneven part
/// of the circle stays visible as a bump.
fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t * t * t)
}

/// The spider assistant: the flown part of the circle as a smooth shape
/// around the centre, the radius following the climb (further out = better
/// than the mean of the flown circle). The colour depends on the radius alone,
/// like the bands of a map: dark red in the middle, the orange of the ring
/// needle where the shape reaches out. Only the sectors flown since the
/// circle began are shown; after a full turn the shape is closed and the
/// action arcs appear on the centre ring: white dots = fly flatter here,
/// orange = fly steeper here.
/// The action arcs of the spider (white dotted: fly flatter, orange: fly
/// steeper) are switched off: it has not proved itself in flight yet whether
/// they really work (Stefan 15.11.2026). The code and its tests stay.
pub const ACTION_ARCS: bool = false;

fn draw_thermal_assitant2<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &mut ThermalData,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    draw_spider_aid_at(display, cm, thermal_data, &AidPlace::centre_circle(cm))
}

fn draw_spider_aid_at<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &mut ThermalData,
    place: &AidPlace,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    draw_spider_at(display, cm, thermal_data, place)?;
    if place.own_ring {
        // the white ring the centre circle has around the spider
        let r = place.ring + 0.5 * OWN_RING_WIDTH;
        Circle::with_center(place.center, (2.0 * r) as u32 + 1)
            .into_styled(PrimitiveStyle::with_stroke(
                cm.palette().vario.scale,
                OWN_RING_WIDTH as u32,
            ))
            .draw(display)?;
    } else if ACTION_ARCS {
        draw_action_arcs(display, cm, thermal_data)?;
    }
    // the pointer reaches from the white ring around the spider into the shape
    draw_core_pointer_at(display, cm, thermal_data, place, place.ring)?;
    let glider_img = Image::new(cm.device_const.images.small_glider);
    let p_gld = thermal_assistant_glider_pos(cm, &glider_img, place.center, place.radius);
    glider_img.draw(display, p_gld, Some(cm.palette().vario.scale))
}

/// The 3D ring assistant (Stefan 16.11.2026): the circle seen from behind
/// and above, as a wall standing on its track. The higher the wall, the
/// better the climb in that part of the circle - where it goes up and
/// where it goes down is seen at a glance. Same data, same reference (mean
/// of the flown circle) and same orientation as the two other assistants:
/// straight ahead is at the back of the ring, the glider symbol sits on
/// the side of the ring.
///
/// The colours do not carry the climb, only the height of the wall does
/// (Stefan 16.11.2026, variant D of the sketches): one colour for the
/// outside of the wall and a dark one for its inside make the ring read
/// as a body, and a thick line in a third colour traces its upper edge.
///
/// Half width and half height of the track, the drop of its centre below
/// the centre of the display and the height of the best possible wall, all
/// as fractions of the inner radius of the centre ring.
const RING_RX: f32 = 0.80;
const RING_RY: f32 = 0.42;
const RING_DROP: f32 = 0.24;
const RING_H: f32 = 0.56;
/// height of the worst possible wall as a fraction of `RING_H`: the track
/// stays visible as a wall in sink too
const RING_H_MIN: f32 = 0.10;
/// Outside and inside of the wall and its edges. The upper edge carries
/// the shape; the lower edge shows where the wall stands, so the height of
/// the far wall reads as the distance between two lines (Stefan 16.11.2026,
/// variant c of the sketches). The width of the edges comes with the place:
/// `RING_EDGE_SMALL` in the centre circle, `RING_EDGE_LARGE` on the
/// centering page.
const RING_OUTSIDE: Colors = Colors::Heat20;
const RING_INSIDE: Colors = Colors::Heat1;
const RING_EDGE: Colors = Colors::Gold;
/// steps of the track line
const RING_TRACK_STEPS: usize = 48;
/// The pointer is an arrow on top of a dashed mast in the middle of the
/// ring (Stefan 16.11.2026). It floats above the highest possible wall, so
/// it never crosses the wall in front of it: its height, the end of its
/// shaft, its tip and the half widths of shaft and head, in fractions of
/// the inner radius of the centre ring (height) and of the track radius.
const RING_MAST_H: f32 = 0.62;
const RING_ARROW_SHAFT: f32 = 0.40;
const RING_ARROW_TIP: f32 = 0.84;
const RING_ARROW_SHAFT_HALF: f32 = 0.085;
const RING_ARROW_HEAD_HALF: f32 = 0.26;
/// dash and gap of the mast (fractions of the inner radius of the centre ring)
const RING_MAST_DASH: f32 = 0.07;
const RING_MAST_GAP: f32 = 0.05;

/// wall height (fraction of the inner radius of the centre ring) for a
/// climb `delta` relative to the mean of the flown circle
fn ring_wall(delta: f32, gain: f32) -> f32 {
    let s = (spider_scale(delta, gain) - SPIDER_MIN) / (1.0 - SPIDER_MIN);
    RING_H * (RING_H_MIN + (1.0 - RING_H_MIN) * s)
}

/// Projection of the 3D ring: a point of the top view (screen angle
/// clockwise from straight ahead, radius as a fraction of the track radius)
/// lifted by `h` (fraction of `unit`). `unit` is the radius of the circle
/// the picture fills, `cx`/`cy` the middle of the floor of the ring.
struct RingProjection {
    cx: f32,
    cy: f32,
    unit: f32,
}

impl RingProjection {
    /// the ring inside the centre circle of the ring page
    #[cfg(test)]
    fn of(cm: &CoreModel) -> Self {
        let place = AidPlace::centre_circle(cm);
        Self::filling(place.ring3d_center, place.ring3d_unit)
    }

    /// the ring filling a circle of radius `unit` around `center`
    fn filling(center: Point, unit: f32) -> Self {
        RingProjection {
            cx: center.x as f32,
            cy: center.y as f32 + RING_DROP * unit,
            unit,
        }
    }

    fn at(&self, angle: f32, radius: f32, h: f32) -> Point {
        self.top_view(angle.sin() * radius, -angle.cos() * radius, h)
    }

    /// a point of the top view in track radii (`x` to the right, `y`
    /// towards the viewer), lifted by `h`
    fn top_view(&self, x: f32, y: f32, h: f32) -> Point {
        let px = self.cx + x * RING_RX * self.unit;
        let py = self.cy + y * RING_RY * self.unit - h * self.unit;
        Point::new((px + 0.5).floor() as i32, (py + 0.5).floor() as i32)
    }
}

/// Corners of the arrow on top of the mast for a screen angle (clockwise
/// from straight ahead): the four corners of the shaft (from the mast
/// outwards) and the head (tip and the two ends of its base).
fn ring_arrow_corners(proj: &RingProjection, angle: f32) -> ([Point; 4], [Point; 3]) {
    // a point of the top view at the height of the mast: along the arrow and across it
    let (sin, cos) = (angle.sin(), angle.cos());
    let corner = |along: f32, across: f32| {
        proj.top_view(
            along * sin + across * cos,
            -along * cos + across * sin,
            RING_MAST_H,
        )
    };
    (
        [
            corner(0.0, -RING_ARROW_SHAFT_HALF),
            corner(0.0, RING_ARROW_SHAFT_HALF),
            corner(RING_ARROW_SHAFT, RING_ARROW_SHAFT_HALF),
            corner(RING_ARROW_SHAFT, -RING_ARROW_SHAFT_HALF),
        ],
        [
            corner(RING_ARROW_TIP, 0.0),
            corner(RING_ARROW_SHAFT, -RING_ARROW_HEAD_HALF),
            corner(RING_ARROW_SHAFT, RING_ARROW_HEAD_HALF),
        ],
    )
}

/// The mast in the middle of the ring and the arrow on top of it, pointing
/// to where the circle should be moved.
fn draw_ring_pointer<D>(display: &mut D, proj: &RingProjection, angle: f32) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    // dashed mast from the floor to the arrow
    let mast = PrimitiveStyle::with_stroke(Colors::White, 2);
    let mut h = 0.0;
    while h < RING_MAST_H {
        let top = (h + RING_MAST_DASH).min(RING_MAST_H);
        Line::new(proj.top_view(0.0, 0.0, h), proj.top_view(0.0, 0.0, top))
            .into_styled(mast)
            .draw(display)?;
        h += RING_MAST_DASH + RING_MAST_GAP;
    }
    // arrow: a black rim first, then the white shape, so shaft and head
    // join without a line between them
    let ([s0, s1, s2, s3], [tip, b0, b1]) = ring_arrow_corners(proj, angle);
    let rim = PrimitiveStyleBuilder::new()
        .fill_color(Colors::Black)
        .stroke_color(Colors::Black)
        .stroke_width(3)
        .build();
    let fill = PrimitiveStyle::with_fill(Colors::White);
    for style in [rim, fill] {
        Triangle::new(s0, s1, s2).into_styled(style).draw(display)?;
        Triangle::new(s0, s2, s3).into_styled(style).draw(display)?;
        Triangle::new(tip, b0, b1)
            .into_styled(style)
            .draw(display)?;
    }
    Ok(())
}

fn draw_thermal_assitant3<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &mut ThermalData,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    draw_ring3d_at(display, cm, thermal_data, &AidPlace::centre_circle(cm))
}

fn draw_ring3d_at<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &ThermalData,
    place: &AidPlace,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let proj = RingProjection::filling(place.ring3d_center, place.ring3d_unit);
    draw_ring3d(display, cm, thermal_data, &proj, place.ring3d_edge)
}

fn draw_ring3d<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &ThermalData,
    proj: &RingProjection,
    edge_width: u32,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let rotation = thermal_assistant_rotation(cm);
    let (low, high) = thermal_data.range();
    let gain = spider_gain(low, high);

    // the track on the floor: the part not flown yet stays a grey line
    let track = PrimitiveStyle::with_stroke(Colors::DimGray, 1);
    let step = 2.0 * PI / RING_TRACK_STEPS as f32;
    for k in 0..RING_TRACK_STEPS {
        let a = k as f32 * step;
        Line::new(proj.at(a, 1.0, 0.0), proj.at(a + step, 1.0, 0.0))
            .into_styled(track)
            .draw(display)?;
    }

    // Wall pieces from sector to sector, the farthest first: the near half
    // of the ring covers the far half. `cos` of the screen angle is the
    // distance from the viewer (1 = straight ahead, at the back).
    let mut order = [0usize; THERMAL_DATA_CNT];
    for (k, o) in order.iter_mut().enumerate() {
        *o = k;
    }
    let depth = |idx: usize| ((idx as f32 + 0.5) * DELTA_ALPHA + rotation).cos();
    for i in 1..THERMAL_DATA_CNT {
        let mut j = i;
        while j > 0 && depth(order[j - 1]) < depth(order[j]) {
            order.swap(j - 1, j);
            j -= 1;
        }
    }
    // The pointer stands in the middle of the ring: in front of the far
    // half of the wall, behind the near half. The arrow floats above the
    // highest possible wall, so the near half never hides it - only the
    // foot of the mast disappears behind a high wall, as it should.
    let pointer = if CORE_POINTER {
        thermal_data.core_pointer()
    } else {
        None
    };
    let mut pointer_drawn = false;
    let edge = PrimitiveStyle::with_stroke(RING_EDGE, edge_width);
    for idx in order {
        let far = depth(idx) > 0.0;
        if !far && !pointer_drawn {
            if let Some(heading) = pointer {
                draw_ring_pointer(display, proj, heading + rotation)?;
            }
            pointer_drawn = true;
        }
        let next = (idx + 1) % THERMAL_DATA_CNT;
        if !thermal_data.visited(idx) || !thermal_data.visited(next) {
            continue;
        }
        let h0 = ring_wall(thermal_data.value(idx), gain);
        let h1 = ring_wall(thermal_data.value(next), gain);
        let a0 = idx as f32 * DELTA_ALPHA + rotation;
        let a1 = a0 + DELTA_ALPHA;
        let (p0, p1) = (proj.at(a0, 1.0, 0.0), proj.at(a1, 1.0, 0.0));
        let (q0, q1) = (proj.at(a0, 1.0, h0), proj.at(a1, 1.0, h1));
        let fill = PrimitiveStyle::with_fill(if far { RING_INSIDE } else { RING_OUTSIDE });
        Triangle::new(p0, p1, q1).into_styled(fill).draw(display)?;
        Triangle::new(p0, q1, q0).into_styled(fill).draw(display)?;
        // the lower edge shows where the wall stands, the upper edge
        // carries the shape
        Line::new(p0, p1).into_styled(edge).draw(display)?;
        Line::new(q0, q1).into_styled(edge).draw(display)?;
    }
    if !pointer_drawn {
        if let Some(heading) = pointer {
            draw_ring_pointer(display, proj, heading + rotation)?;
        }
    }

    // the glider on the track, at the side of the ring
    let glider_img = Image::new(cm.device_const.images.small_glider);
    let side = match cm.control.circling_direction {
        CirclingDirection::Left => pos::NINE_O_CLOCK,
        CirclingDirection::Right => pos::THREE_O_CLOCK,
    };
    let on_track = proj.at(side, 1.0, 0.0);
    let p_gld = on_track
        - Point::new(
            (glider_img.width() / 2) as i32,
            (glider_img.height() / 2) as i32,
        );
    glider_img.draw(display, p_gld, Some(cm.palette().vario.scale))
}

/// Samples of the spider outline: the radius scale at every sub step of
/// the flown run, from half a sector before the first sector centre to
/// half a sector after the last one (a closed circle: from the first
/// centre once around).
struct SpiderOutline {
    /// radius scale per sub step, `n` entries valid
    s: [f32; THERMAL_DATA_CNT * SPIDER_STEPS + 1],
    n: usize,
    /// run parameter (sectors) of the first sample and of the last one
    k_start: f32,
    k_end: f32,
    /// heading of the first sector centre and the sign of the flight direction
    alpha_first: f32,
    sign: f32,
    closed: bool,
    /// radius change per m/s of this outline
    gain: f32,
}

impl SpiderOutline {
    /// the outline of the sectors flown so far, None while nothing is recorded
    fn of(thermal_data: &ThermalData, cm: &CoreModel) -> Option<Self> {
        let direction = cm.control.circling_direction;
        let current = ThermalData::get_idx(cm.sensor.euler_yaw.to_radians());
        let (first, count) = thermal_data.flown_run(current, direction);
        if count == 0 {
            return None;
        }
        let closed = count == THERMAL_DATA_CNT;
        let (low, high) = thermal_data.range();
        let gain = spider_gain(low, high);
        // sector values in flight order
        let mut values = [0.0_f32; THERMAL_DATA_CNT];
        let mut idx = first;
        for value in values.iter_mut().take(count) {
            *value = thermal_data.value(idx);
            idx = ThermalData::next_idx(idx, direction);
        }
        let at = |k: i32| -> f32 {
            // value at sector k of the run, neighbours beyond the ends clamped
            // (open run) or wrapped (closed circle)
            let k = if closed {
                k.rem_euclid(count as i32)
            } else {
                k.clamp(0, count as i32 - 1)
            };
            values[k as usize]
        };
        let (k_start, k_end) = if closed {
            (0.0, count as f32)
        } else {
            (-0.5, count as f32 - 0.5)
        };
        let step = 1.0 / SPIDER_STEPS as f32;
        let mut out = SpiderOutline {
            s: [0.0; THERMAL_DATA_CNT * SPIDER_STEPS + 1],
            n: 0,
            k_start,
            k_end,
            alpha_first: (first as f32 + 0.5) * DELTA_ALPHA,
            sign: match direction {
                CirclingDirection::Left => 1.0,
                CirclingDirection::Right => -1.0,
            },
            closed,
            gain,
        };
        let n = ((k_end - k_start) * SPIDER_STEPS as f32 + 0.5) as usize + 1;
        for (j, slot) in out.s.iter_mut().enumerate().take(n) {
            let k = (k_start + j as f32 * step).min(k_end);
            let seg = k.floor() as i32;
            let t = k - seg as f32;
            // between two sector centres the value follows the spline; the
            // caps outside the run keep the value of their sector
            let value = if k < 0.0 || (!closed && k > count as f32 - 1.0) {
                at(seg)
            } else {
                catmull_rom(at(seg - 1), at(seg), at(seg + 1), at(seg + 2), t)
            };
            *slot = spider_scale(value, gain);
        }
        out.n = n;
        Some(out)
    }

    /// run parameter (sectors, counted in flight direction from the first
    /// sector centre) of a heading, None outside an open run
    fn param(&self, heading: f32) -> Option<f32> {
        let mut k = ((heading - self.alpha_first) * self.sign).rem_euclid(2.0 * PI) / DELTA_ALPHA;
        if self.closed {
            return Some(k);
        }
        if k >= THERMAL_DATA_CNT as f32 - 0.5 {
            // the half sector before the first centre
            k -= THERMAL_DATA_CNT as f32;
        }
        if k < self.k_start || k > self.k_end {
            None
        } else {
            Some(k)
        }
    }

    /// radius scale at a run parameter (linear between the samples)
    fn scale_at(&self, k: f32) -> f32 {
        let x = (k - self.k_start) * SPIDER_STEPS as f32;
        let j = (x as usize).min(self.n - 1);
        let t = x - j as f32;
        if j + 1 < self.n {
            self.s[j] + t * (self.s[j + 1] - self.s[j])
        } else {
            self.s[j]
        }
    }
}

/// half width of the one pixel lines (rim, contours, average ring), px
const SPIDER_LINE_HALF: f32 = 0.6;

/// the shape of the spider assistant, without glider symbol and arcs
///
/// Drawn as a scan of the disc, pixel by pixel classified by radius and
/// angle and written as runs of one colour: about 14 000 pixels with an
/// atan2 each. The former way - some 4000 tiny embedded-graphics triangles
/// per frame for the colour bands, lines for the contours and 24 arcs for
/// the average ring - took nine times the rest of the ring page on the host
/// and made the V2 skip display refreshes (flight test 16.09.: the needle
/// stuttered while circling).
#[cfg(test)]
fn draw_spider<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &ThermalData,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    draw_spider_at(display, cm, thermal_data, &AidPlace::centre_circle(cm))
}

fn draw_spider_at<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &ThermalData,
    place: &AidPlace,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let outline = match SpiderOutline::of(thermal_data, cm) {
        Some(outline) => outline,
        None => return Ok(()),
    };
    let sizes = &cm.device_const.sizes;
    let radius = place.radius;
    let center = place.center;
    let width = sizes.display.width as i32;
    let height = sizes.display.height as i32;
    let rotation = thermal_assistant_rotation(cm);
    let scale_color = cm.palette().vario.scale;

    // squared radii of everything that is compared per pixel
    let sq = |r: f32| r * r;
    let mut band_r2 = [0.0_f32; HEAT.len()];
    for (k, r2) in band_r2.iter_mut().enumerate() {
        *r2 = sq(band_edge(k) * radius);
    }
    // the contour lines that lie between the smallest radius and the rim
    let mut contour_buf = [(0.0, 0.0, 0.0, Colors::Black); SPIDER_CONTOURS.len()];
    let mut contour_cnt = 0;
    for delta in SPIDER_CONTOURS.iter() {
        let sc = SPIDER_NULL + outline.gain * delta;
        if sc <= SPIDER_MIN + 0.02 || sc >= 0.98 {
            continue;
        }
        let r = sc * radius;
        contour_buf[contour_cnt] = (
            sc,
            sq((r - SPIDER_LINE_HALF).max(0.0)),
            sq(r + SPIDER_LINE_HALF),
            HEAT[band_of(sc).saturating_sub(CONTOUR_DARKER)],
        );
        contour_cnt += 1;
    }
    let contours = &contour_buf[..contour_cnt];
    let null_r = SPIDER_NULL * radius;
    let (null_in2, null_out2) = (sq(null_r - SPIDER_LINE_HALF), sq(null_r + SPIDER_LINE_HALF));
    let dash_sweep = 2.0 * PI / NULL_RING_DASHES as f32;
    let edge_half2 = sq(SPIDER_LINE_HALF);
    // Beyond the farthest point of the shape (and the average ring) there
    // is nothing to draw; inside the nearest point of a closed shape the
    // colour depends on the radius alone - neither needs the angle, which
    // is the expensive part of a pixel.
    let (mut s_min, mut s_max) = (1.0_f32, 0.0_f32);
    for s in outline.s.iter().take(outline.n) {
        s_min = s_min.min(*s);
        s_max = s_max.max(*s);
    }
    let r_max2 = sq(s_max * radius + SPIDER_LINE_HALF)
        .max(null_out2)
        .min(sq(radius + 1.0));
    let r_plain2 = if outline.closed {
        sq((s_min * radius - SPIDER_LINE_HALF).max(0.0))
    } else {
        0.0
    };

    let y0 = (center.y - radius as i32 - 1).max(0);
    let y1 = (center.y + radius as i32 + 1).min(height - 1);
    for y in y0..=y1 {
        let dy = y - center.y;
        let dy2 = (dy * dy) as f32;
        if dy2 > r_max2 {
            continue;
        }
        let x0 = (center.x - radius as i32 - 1).max(0);
        let x1 = (center.x + radius as i32 + 1).min(width - 1);
        let mut run: Option<(i32, Colors)> = None;
        for x in x0..=x1 {
            let dx = x - center.x;
            let r2 = (dx * dx) as f32 + dy2;
            let mut color = None;
            let on_null_ring = r2 >= null_in2 && r2 <= null_out2;
            if r2 <= r_max2 && r2 < r_plain2 && !on_null_ring {
                // well inside a closed shape: contour or band by the radius
                for &(_, in2, out2, c) in contours {
                    if r2 >= in2 && r2 <= out2 {
                        color = Some(c);
                        break;
                    }
                }
                if color.is_none() {
                    let mut band = 0;
                    while band + 1 < HEAT.len() && r2 >= band_r2[band] {
                        band += 1;
                    }
                    color = Some(HEAT[band]);
                }
            } else if r2 <= r_max2 {
                // screen angle clockwise from 12 o'clock, as `to_xy` maps it
                let a = (dx as f32).atan2(-(dy as f32));
                // the dashed ring of the mean of the circle lies on top
                if on_null_ring {
                    let phase = a.rem_euclid(dash_sweep);
                    if phase < 0.5 * dash_sweep {
                        color = Some(scale_color);
                    }
                }
                if color.is_none() {
                    if let Some(k) = outline.param(a - rotation) {
                        let s = outline.scale_at(k);
                        let rs = s * radius;
                        let rim_in2 = sq((rs - SPIDER_LINE_HALF).max(0.0));
                        let rim_out2 = sq(rs + SPIDER_LINE_HALF);
                        if r2 >= rim_in2 && r2 <= rim_out2 {
                            color = Some(scale_color);
                        } else if r2 < rim_in2 {
                            // inside the shape: radial edge of an open run,
                            // contour line or the colour band of the radius
                            if !outline.closed {
                                let d = (k - outline.k_start).min(outline.k_end - k) * DELTA_ALPHA;
                                if d * d * r2 <= edge_half2 {
                                    color = Some(scale_color);
                                }
                            }
                            if color.is_none() {
                                for &(sc, in2, out2, c) in contours {
                                    if s >= sc && r2 >= in2 && r2 <= out2 {
                                        color = Some(c);
                                        break;
                                    }
                                }
                            }
                            if color.is_none() {
                                let mut band = 0;
                                while band + 1 < HEAT.len() && r2 >= band_r2[band] {
                                    band += 1;
                                }
                                color = Some(HEAT[band]);
                            }
                        }
                    }
                }
            }
            // extend, flush or start a run of one colour
            match (run, color) {
                (Some((_, rc)), Some(c)) if rc == c => {}
                (Some((xs, rc)), _) => {
                    // Safety: x and y are clamped to the display above
                    unsafe {
                        display.draw_line_unchecked(
                            (y * width + xs) as usize,
                            (x - xs) as usize,
                            rc,
                        );
                    }
                    run = color.map(|c| (x, c));
                }
                (None, Some(c)) => run = Some((x, c)),
                (None, None) => {}
            }
        }
        if let Some((xs, rc)) = run {
            // Safety: see above
            unsafe {
                display.draw_line_unchecked((y * width + xs) as usize, (x1 + 1 - xs) as usize, rc);
            }
        }
    }
    Ok(())
}

/// The white pointer of both assistants (Stefan 15.11.2026): a triangle on
/// the side of the circle that is nearest to the core, its tip towards the
/// centre. The picture is a top view with the heading up, so the pointer
/// shows the direction in which to move the circle; at the top the core
/// lies straight ahead. No pointer before the first full circle and none
/// in a centred circle (`ThermalData::core_pointer`).
pub const CORE_POINTER: bool = true;
/// length and half width of the pointer as fractions of the circle radius
const POINTER_LEN: f32 = 0.38;
const POINTER_HALF: f32 = 0.12;

/// corners of the pointer for a screen angle (clockwise from 12 o'clock):
/// the tip and the two ends of the base at radius `base`
fn pointer_corners(center: Point, angle: f32, base: f32, radius: f32) -> [Point; 3] {
    let at = |a: f32, r: f32| {
        center
            + Point::new(
                (a.sin() * r + 0.5).floor() as i32,
                (-a.cos() * r + 0.5).floor() as i32,
            )
    };
    let half = POINTER_HALF * radius / base;
    [
        at(angle, base - POINTER_LEN * radius),
        at(angle - half, base),
        at(angle + half, base),
    ]
}

#[cfg(test)]
fn draw_core_pointer<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &ThermalData,
    base: f32,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    draw_core_pointer_at(
        display,
        cm,
        thermal_data,
        &AidPlace::centre_circle(cm),
        base,
    )
}

fn draw_core_pointer_at<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &ThermalData,
    place: &AidPlace,
    base: f32,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    if !CORE_POINTER {
        return Ok(());
    }
    let heading = match thermal_data.core_pointer() {
        Some(heading) => heading,
        None => return Ok(()),
    };
    let [tip, a, b] = pointer_corners(
        place.center,
        heading + thermal_assistant_rotation(cm),
        base,
        place.radius,
    );
    // white with a black edge: readable on the orange of the spider too
    let style = PrimitiveStyleBuilder::new()
        .fill_color(Colors::White)
        .stroke_color(Colors::Black)
        .stroke_width(1)
        .build();
    Triangle::new(tip, a, b).into_styled(style).draw(display)?;
    Ok(())
}

/// Stroke of the action arcs in pixels of the 480x480 reference design
const ARC_STROKE: f32 = 5.0;
/// distance of the white dots, in strokes
const ARC_DOT_SPACING: f32 = 2.2;

/// The action arcs on the centre ring of the scale: a white dotted arc
/// where the glider is to fly flatter, an orange arc (the colour of the
/// ring needle) where it is to fly steeper. They turn with the picture; the
/// glider symbol runs through them.
fn draw_action_arcs<D>(
    display: &mut D,
    cm: &CoreModel,
    thermal_data: &ThermalData,
) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let arcs = thermal_data.action_arcs();
    if arcs.flatter.is_none() && arcs.steeper.is_none() {
        return Ok(());
    }
    let sizes = &cm.device_const.sizes;
    let scl = sizes.display.width.min(sizes.display.height) as f32 / 480.0;
    let radius = 0.5 * (CENTER_RING_IN + CENTER_RING_OUT) * scl;
    let stroke = ((ARC_STROKE * scl) as u32).max(3);
    let center = sizes.display.center;
    let rotation = thermal_assistant_rotation(cm);
    // screen angle (clockwise from 12 o'clock) of a heading
    let screen = |heading: f32| heading + rotation;

    if let Some(ActionArc { heading, half }) = arcs.steeper {
        // embedded-graphics counts its angles clockwise from 3 o'clock
        let start = (screen(heading) - half).to_degrees() - 90.0;
        Arc::with_center(
            center,
            (2.0 * radius) as u32,
            start.deg(),
            (2.0 * half).to_degrees().deg(),
        )
        .into_styled(PrimitiveStyle::with_stroke(
            cm.palette().vario.ring_needle,
            stroke,
        ))
        .draw(display)?;
    }
    if let Some(ActionArc { heading, half }) = arcs.flatter {
        let dots = ((2.0 * half * radius) / (ARC_DOT_SPACING * stroke as f32)).max(3.0) as usize;
        for i in 0..=dots {
            let alpha = screen(heading) - half + 2.0 * half * i as f32 / dots as f32;
            let p = PolarCoordinate { alpha, len: radius }.to_xy(1.0, 0.0) + center;
            Circle::with_center(p, stroke)
                .into_styled(PrimitiveStyle::with_fill(Colors::White))
                .draw(display)?;
        }
    }
    Ok(())
}

fn thermal_assistant_rotation(cm: &CoreModel) -> f32 {
    let yaw = -cm.sensor.euler_yaw.to_radians();
    match cm.control.circling_direction {
        CirclingDirection::Left => yaw + pos::NINE_O_CLOCK,
        CirclingDirection::Right => yaw + pos::THREE_O_CLOCK,
    }
}

/// upper left corner of the glider symbol on the circle `radius` around `center`
fn thermal_assistant_glider_pos(
    cm: &CoreModel,
    glider_img: &Image,
    center: Point,
    radius: f32,
) -> Point {
    let dy = (glider_img.height() / 2) as i32;
    let half = (glider_img.width() / 2) as i32;
    let r = (radius + 0.5) as i32;
    match cm.control.circling_direction {
        CirclingDirection::Left => center + Point::new(-r - half, -dy),
        CirclingDirection::Right => center + Point::new(r - half, -dy),
    }
}

fn draw_and_calc_wind_basics<D>(
    display: &mut D,
    cm: &CoreModel,
) -> Result<(Angle, Angle), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let sizes = &cm.device_const.sizes.vario;
    let (mut angle, mut av_angle) = match cm.control.fly_mode {
        FlyMode::Circling => {
            // draw north symbol
            display.draw_img(
                cm.device_const.images.north,
                sizes.north_pos,
                Some(cm.palette().vario.background),
            )?;
            // return absolut wind vector
            (
                cm.sensor.wind_vector.angle(),
                cm.sensor.average_wind.angle(),
            )
        }
        FlyMode::StraightFlight => {
            // draw glider symbol
            if cm.config.glider_symbol {
                display.draw_img(
                    cm.device_const.images.glider,
                    sizes.glider_pos,
                    Some(cm.palette().vario.scale),
                )?;
            }
            (
                // return relativ wind vector
                cm.sensor.wind_vector.angle() - cm.sensor.euler_yaw,
                cm.sensor.average_wind.angle() - cm.sensor.euler_yaw,
            )
        }
    };

    if cm.sensor.airspeed.ias() < 30.0.km_h() {
        angle = 180.0.deg(); // The sensor box should actually do this
        av_angle = 180.0.deg();
    }
    Ok((angle, av_angle))
}

fn calc_len(cm: &CoreModel, wind_speed: f32, sizes: &VarioSizes) -> i32 {
    let len = match wind_speed {
        x if x < WIND_MIN => sizes.wind_len_min, // Light wind is set to a minimum size
        x if x > WIND_MAX => sizes.wind_len,     // Strong wind is set to a maximum size
        _ => {
            sizes.wind_len_min
                + ((sizes.wind_len - sizes.wind_len_min) as f32 * (wind_speed - WIND_MIN)
                    / (WIND_MAX - WIND_MIN)) as i32
        }
    };
    // the ring design center circle is smaller than the old center area
    let _ = cm;
    len * 50 / 100
}

fn draw_single_arrow<D>(display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let sizes = &cm.device_const.sizes.vario;
    let d_sizes = &cm.device_const.sizes.display;

    // draw wind arrow
    let wind_speed = cm.sensor.wind_vector.speed().to_km_h();
    let (angle, av_angle) = draw_and_calc_wind_basics(display, cm)?;

    let _ = av_angle;
    let len = calc_len(cm, wind_speed, sizes);
    RingArrow::new(len, d_sizes.center)
        .zero_pos(pos::SIX_O_CLOCK)
        .rotate(angle.to_radians())
        .draw_colored(cm.palette().vario.ring_wind_fill, display)?;
    draw_energy_arrow(display, cm)
}

fn draw_double_arrow<D>(display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    let sizes = &cm.device_const.sizes.vario;
    let d_sizes = &cm.device_const.sizes.display;

    // draw wind arrow
    let (angle, av_angle) = draw_and_calc_wind_basics(display, cm)?;

    let len = calc_len(cm, cm.sensor.average_wind.speed().to_km_h(), sizes);
    RingArrow::new(len, d_sizes.center)
        .zero_pos(pos::SIX_O_CLOCK)
        .rotate(av_angle.to_radians())
        .draw_colored(cm.palette().vario.ring_avg_wind_fill, display)?;

    let len = calc_len(cm, cm.sensor.wind_vector.speed().to_km_h(), sizes);
    RingArrow::new(len, d_sizes.center)
        .zero_pos(pos::SIX_O_CLOCK)
        .rotate(angle.to_radians())
        .draw_colored(cm.palette().vario.ring_wind_fill, display)?;

    draw_energy_arrow(display, cm)
}

fn draw_energy_arrow<D>(display: &mut D, cm: &CoreModel) -> Result<(), CoreError>
where
    D: DrawTarget<Color = Colors, Error = CoreError> + DrawImage,
{
    // Basic mode: two wind arrows are enough (Stefan 15.11.2026)
    if cm.config.basic_mode && !cm.config.club_mode {
        return Ok(());
    }
    let energy_arrow = cm.sensor.average_wind - cm.sensor.wind_vector;
    let wind_speed = energy_arrow.speed().to_km_h() * cm.control.energy_arrow_mult;
    let len = if wind_speed > WIND_MAX {
        1.0
    } else {
        wind_speed / WIND_MAX
    };
    let len = (len * cm.device_const.sizes.vario.wind_len as f32) as i32;

    if len <= cm.device_const.sizes.vario.wind_len / 4 {
        return Ok(());
    }

    let angle = match cm.control.fly_mode {
        FlyMode::Circling => energy_arrow.angle(),
        FlyMode::StraightFlight => energy_arrow.angle() - cm.sensor.euler_yaw,
    };
    // the energy arrow must stay inside the center circle
    let len = len * 50 / 100;
    let style = PrimitiveStyle::with_fill(Colors::Red);
    Arrow::new(len, cm.device_const.sizes.display.center)
        .zero_pos(pos::SIX_O_CLOCK)
        .rotate(angle.to_radians())
        .draw_styled(style, display)
}

#[cfg(test)]
mod spider_tests {
    use super::*;
    use crate::view::test_fixture::{test_model, TestDisplay, DISPLAY_HEIGHT, DISPLAY_WIDTH};
    use crate::{CanConfigId, FloatToAngularVelocity, VarioMode};
    use embedded_graphics::pixelcolor::Rgb888;
    use embedded_graphics::prelude::RgbColor;

    fn dump_ppm(display: &TestDisplay, path: &str) {
        use std::io::Write;
        let mut f = std::fs::File::create(path).unwrap();
        write!(f, "P6 {} {} 255\n", DISPLAY_WIDTH, DISPLAY_HEIGHT).unwrap();
        for p in display.buf.iter() {
            let rgb: [u8; 3] = match p {
                None => [0, 0, 0],
                Some(c) => {
                    let c = Rgb888::from(*c);
                    [c.r(), c.g(), c.b()]
                }
            };
            f.write_all(&rgb).unwrap();
        }
    }

    /// Fly `ticks` steps of a circle (3 degrees of heading per 100 ms tick)
    /// with a climb that has its best part around heading `best_deg`.
    fn fly(
        cm: &mut CoreModel,
        td: &mut ThermalData,
        direction: CirclingDirection,
        ticks: usize,
        best_deg: f32,
    ) {
        cm.control.vario_mode = VarioMode::Vario;
        cm.control.fly_mode = FlyMode::Circling;
        cm.control.circling_direction = direction;
        cm.calculated.av2_climb_rate = 1.5.m_s();
        cm.sensor.average_climb_rate = 1.5.m_s();
        let mut yaw = 90.0_f32;
        for _ in 0..ticks {
            let rel = (yaw - best_deg).to_radians();
            // a broad maximum, a narrow dip 100 degrees further and a bump
            let climb = 1.5 + 1.6 * rel.cos()
                - 1.2 * (-((yaw - best_deg - 100.0).rem_euclid(360.0) - 180.0).abs() / 12.0).exp()
                + 0.7 * (-((yaw - best_deg + 140.0).rem_euclid(360.0) - 180.0).abs() / 8.0).exp();
            cm.sensor.euler_yaw = yaw.rem_euclid(360.0).deg();
            cm.sensor.climb_rate = climb.m_s();
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            td.update(cm);
            // `Left` is the variant of a positive turn rate: heading increasing
            yaw += match direction {
                CirclingDirection::Left => 3.0,
                CirclingDirection::Right => -3.0,
            };
        }
    }

    fn render(cm: &CoreModel, td: &mut ThermalData) -> TestDisplay {
        let mut display = TestDisplay::new();
        draw_spider(&mut display, cm, td).expect("the spider must draw");
        display
    }

    /// farthest ink pixel from the centre, and the ink in each half (left/right of the centre)
    fn ink_stats(d: &TestDisplay) -> (f32, usize, usize) {
        let (cx, cy) = (240.0_f32, 240.0_f32);
        let mut rmax = 0.0_f32;
        let (mut left, mut right) = (0, 0);
        for y in 0..DISPLAY_HEIGHT as i32 {
            for x in 0..DISPLAY_WIDTH as i32 {
                if let Some(c) = d.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] {
                    if c == Colors::Black {
                        continue;
                    }
                    let r = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                    rmax = rmax.max(r);
                    if x < 240 {
                        left += 1
                    } else {
                        right += 1
                    }
                }
            }
        }
        (rmax, left, right)
    }

    /// pixels whose colour satisfies `pred`, and their radius range
    fn ink_where(d: &TestDisplay, pred: impl Fn(Colors) -> bool) -> (usize, f32, f32) {
        let (cx, cy) = (240.0_f32, 240.0_f32);
        let (mut n, mut rmin, mut rmax) = (0, f32::MAX, 0.0_f32);
        for y in 0..DISPLAY_HEIGHT as i32 {
            for x in 0..DISPLAY_WIDTH as i32 {
                if let Some(c) = d.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] {
                    if pred(c) {
                        n += 1;
                        let r = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                        rmin = rmin.min(r);
                        rmax = rmax.max(r);
                    }
                }
            }
        }
        (n, rmin, rmax)
    }

    fn is_heat(c: Colors, from: usize, to: usize) -> bool {
        HEAT[from..=to].contains(&c)
    }

    #[test]
    fn spider_stays_inside_the_circle_and_shows_only_the_flown_part() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        // 40 ticks = 120 degrees of a circle
        fly(&mut cm, &mut td, CirclingDirection::Left, 40, 30.0);
        let d = render(&cm, &mut td);
        let (rmax, left, right) = ink_stats(&d);
        if std::env::var("SPIDER_DUMP").is_ok() {
            dump_ppm(&d, "/tmp/spider_part.ppm");
        }
        let radius = cm.device_const.sizes.vario.ta_circle_radius as f32;
        assert!(
            rmax <= radius + 1.5,
            "ink out to {rmax} px, circle radius {radius}"
        );
        // a third of the circle is flown: most of the centre is still empty
        let total = left + right;
        assert!(total > 500, "hardly any ink: {total}");
        let full = std::f32::consts::PI * radius * radius;
        assert!(
            (total as f32) < 0.5 * full,
            "{total} ink pixels for a third of the circle"
        );
        // no arcs before the circle is closed
        assert!(td.action_arcs().flatter.is_none() && td.action_arcs().steeper.is_none());

        // after a full turn the shape is closed and larger
        fly(&mut cm, &mut td, CirclingDirection::Left, 90, 30.0);
        let d2 = render(&cm, &mut td);
        let (rmax2, l2, r2) = ink_stats(&d2);
        if std::env::var("SPIDER_DUMP").is_ok() {
            dump_ppm(&d2, "/tmp/spider_full.ppm");
        }
        assert!(rmax2 <= radius + 1.5, "ink out to {rmax2} px");
        assert!(
            l2 + r2 > total,
            "the closed spider must cover more than the third"
        );
        // the colour follows the radius: dark red in the middle, the outer
        // bands only where the shape reaches beyond the average ring
        let (n_core, _, r_core) = ink_where(&d2, |c| is_heat(c, 0, 0));
        assert!(n_core > 300, "dark red middle missing: {n_core} px");
        // the innermost band plus the contour line at -2 m/s drawn in its colour
        assert!(r_core < 0.45 * radius, "Heat0 out to {r_core} px");
        let (n_out, r_out_min, _) = ink_where(&d2, |c| is_heat(c, 16, 29));
        assert!(n_out > 100, "outer bands missing: {n_out} px");
        assert!(
            r_out_min > 0.6 * radius,
            "outer band starts at {r_out_min} px"
        );
    }

    #[test]
    fn core_estimate_and_arcs_follow_the_best_climb() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        cm.sensor.euler_roll = 45.0.deg();
        fly(&mut cm, &mut td, CirclingDirection::Left, 130, 30.0);
        let est = td.estimate().expect("a full circle gives an estimate");
        let deg = est.heading.to_degrees();
        assert!(
            (deg - 30.0).abs() < 20.0,
            "core at {deg} degrees, best climb at 30"
        );
        assert!(est.offset >= 0.15, "offset {}", est.offset);
        let arcs = td.action_arcs();
        let fl = arcs.flatter.expect("flatter arc");
        let st = arcs.steeper.expect("steeper arc");
        // turn rate 0 in the test model: no lead. The flatter arc sits a
        // quarter circle before the best climb (heading increasing)
        let fl_deg = fl.heading.to_degrees();
        assert!(
            ((fl_deg - 300.0 + 180.0).rem_euclid(360.0) - 180.0).abs() < 20.0,
            "flatter arc at {fl_deg}"
        );
        let st_deg = st.heading.to_degrees();
        assert!(
            ((st_deg - 120.0 + 180.0).rem_euclid(360.0) - 180.0).abs() < 20.0,
            "steeper arc at {st_deg}"
        );
        assert!(fl.half >= 30.0_f32.to_radians() && fl.half <= 60.0_f32.to_radians());

        // with a turn rate the arcs come earlier: the vario lag and the
        // reaction time are taken out against the sense of the heading
        cm.sensor.turn_rate = 0.3.rad_s();
        cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
        td.update(&cm);
        let fl2 = td
            .action_arcs()
            .flatter
            .expect("flatter arc")
            .heading
            .to_degrees();
        let shift = (fl_deg - fl2 + 180.0).rem_euclid(360.0) - 180.0;
        // 0.3 rad/s * (0.3 * 2 s + 1.5 s) = 0.63 rad = 36 degrees
        assert!(shift > 25.0 && shift < 47.0, "lead of {shift} degrees");
    }

    #[test]
    fn arcs_mirror_in_a_turn_to_the_left_and_follow_the_vario_tc() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        cm.sensor.euler_roll = 40.0.deg();
        // `Right` is the variant of a negative turn rate: heading decreasing
        fly(&mut cm, &mut td, CirclingDirection::Right, 130, 200.0);
        let est = td.estimate().expect("estimate");
        let deg = est.heading.to_degrees();
        assert!(
            ((deg - 200.0 + 180.0).rem_euclid(360.0) - 180.0).abs() < 20.0,
            "core at {deg}"
        );
        // the flatter arc a quarter circle before the best climb: heading + 90
        let fl = td
            .action_arcs()
            .flatter
            .expect("flatter arc")
            .heading
            .to_degrees();
        assert!(
            ((fl - 290.0 + 180.0).rem_euclid(360.0) - 180.0).abs() < 20.0,
            "flatter arc at {fl}"
        );

        // with a turn rate the lead goes the other way round (heading decreasing),
        // and a shorter vario time constant of the box makes it smaller
        cm.sensor.turn_rate = (-0.3).rad_s();
        cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
        td.update(&cm);
        let fl2 = td
            .action_arcs()
            .flatter
            .expect("flatter arc")
            .heading
            .to_degrees();
        let shift2 = (fl2 - fl + 180.0).rem_euclid(360.0) - 180.0;
        assert!(
            shift2 > 25.0 && shift2 < 47.0,
            "lead of {shift2} degrees (TC default 2 s)"
        );
        cm.sensor
            .remote
            .set(CanConfigId::VarioTc.remote_slot(), 1.0);
        cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
        td.update(&cm);
        let fl3 = td
            .action_arcs()
            .flatter
            .expect("flatter arc")
            .heading
            .to_degrees();
        let shift3 = (fl3 - fl + 180.0).rem_euclid(360.0) - 180.0;
        // 0.3 rad/s * 0.3 s less = 5 degrees less
        assert!(
            shift3 < shift2 - 3.0 && shift3 > shift2 - 8.0,
            "lead {shift2} -> {shift3} with TC 1 s"
        );
    }

    #[test]
    fn arcs_disappear_after_enough_bank_change_and_come_back_a_circle_later() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        cm.sensor.euler_roll = 45.0.deg();
        fly(&mut cm, &mut td, CirclingDirection::Left, 130, 30.0);
        let fl = td.action_arcs().flatter.expect("flatter arc");
        let est = td.estimate().expect("estimate");
        // sit where the heading points at the core (no turn rate in the
        // fixture, so no lead), 15 degrees flatter than usual: 1.5 degree
        // seconds per tick towards the core
        let required = 118.0 * 0.5 * est.offset;
        let ticks_needed = (required / 1.5) as usize;
        cm.sensor.euler_roll = 30.0.deg();
        cm.sensor.euler_yaw = fl.heading.to_degrees().rem_euclid(360.0).deg();
        for _ in 0..ticks_needed * 2 / 3 {
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            td.update(&cm);
        }
        assert!(td.action_arcs().flatter.is_some(), "arcs gone too early");
        for _ in 0..ticks_needed {
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            td.update(&cm);
        }
        assert!(
            td.action_arcs().flatter.is_none(),
            "flatter arc still there after enough opening"
        );
        assert!(
            td.action_arcs().steeper.is_none(),
            "the steeper arc goes with it"
        );
        // back at the usual bank the arcs stay away for the rest of the circle ...
        cm.sensor.euler_roll = 45.0.deg();
        fly(&mut cm, &mut td, CirclingDirection::Left, 60, 30.0);
        assert!(
            td.action_arcs().flatter.is_none(),
            "arcs back within the same circle"
        );
        // ... and are there again once a whole circle of sectors is new
        fly(&mut cm, &mut td, CirclingDirection::Left, 70, 30.0);
        assert!(
            td.action_arcs().flatter.is_some() && td.action_arcs().steeper.is_some(),
            "arcs must come back a circle later"
        );
    }

    #[test]
    fn steeper_away_from_the_core_counts_as_well() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        cm.sensor.euler_roll = 45.0.deg();
        fly(&mut cm, &mut td, CirclingDirection::Left, 130, 30.0);
        let st = td.action_arcs().steeper.expect("steeper arc");
        let est = td.estimate().expect("estimate");
        // a little more than the plain count: the progress fades a bit
        let ticks_needed = (118.0 * 0.5 * est.offset / 1.5) as usize + 15;
        cm.sensor.euler_roll = 60.0.deg();
        cm.sensor.euler_yaw = st.heading.to_degrees().rem_euclid(360.0).deg();
        for _ in 0..ticks_needed {
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            td.update(&cm);
        }
        assert!(
            td.action_arcs().steeper.is_none(),
            "steepening away from the core must count"
        );
    }

    #[test]
    fn opposite_action_does_not_count() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        cm.sensor.euler_roll = 45.0.deg();
        fly(&mut cm, &mut td, CirclingDirection::Left, 130, 30.0);
        let st = td.action_arcs().steeper.expect("steeper arc");
        // flatter while heading away from the core: wrong, progress stays at zero
        cm.sensor.euler_roll = 30.0.deg();
        cm.sensor.euler_yaw = st.heading.to_degrees().rem_euclid(360.0).deg();
        for _ in 0..200 {
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            td.update(&cm);
        }
        assert!(td.action_arcs().flatter.is_some() && td.action_arcs().steeper.is_some());
    }

    #[test]
    fn no_arcs_in_a_centred_circle() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        cm.control.vario_mode = VarioMode::Vario;
        cm.control.fly_mode = FlyMode::Circling;
        cm.control.circling_direction = CirclingDirection::Right;
        cm.calculated.av2_climb_rate = 2.0.m_s();
        cm.sensor.average_climb_rate = 2.0.m_s();
        let mut yaw = 0.0_f32;
        for _ in 0..130 {
            cm.sensor.euler_yaw = yaw.rem_euclid(360.0).deg();
            cm.sensor.climb_rate = 2.1.m_s();
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            td.update(&cm);
            yaw -= 3.0;
        }
        let est = td.estimate().expect("estimate");
        assert!(
            est.offset < 0.15,
            "offset {} in an even thermal",
            est.offset
        );
        assert!(td.action_arcs().flatter.is_none() && td.action_arcs().steeper.is_none());
    }

    #[test]
    fn action_arcs_sit_on_the_centre_ring() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        cm.sensor.euler_roll = 45.0.deg();
        fly(&mut cm, &mut td, CirclingDirection::Left, 130, 30.0);
        let mut d = TestDisplay::new();
        draw_action_arcs(&mut d, &cm, &td).expect("arcs must draw");
        if std::env::var("SPIDER_DUMP").is_ok() {
            dump_ppm(&d, "/tmp/spider_arcs.ppm");
            let mut both = TestDisplay::new();
            draw_spider(&mut both, &cm, &td).expect("spider");
            draw_action_arcs(&mut both, &cm, &td).expect("arcs");
            dump_ppm(&both, "/tmp/spider_full_arcs.ppm");
        }
        let orange = cm.palette().vario.ring_needle;
        let (n_o, r_o_min, r_o_max) = ink_where(&d, |c| c == orange);
        let (n_w, r_w_min, r_w_max) = ink_where(&d, |c| c == Colors::White);
        assert!(n_o > 100 && n_w > 60, "orange {n_o} px, white {n_w} px");
        for r in [r_o_min, r_o_max, r_w_min, r_w_max] {
            assert!(r > 72.0 && r < 82.0, "arc ink at radius {r}");
        }
    }

    #[test]
    fn spider_starts_empty_in_a_new_circle() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        fly(&mut cm, &mut td, CirclingDirection::Right, 130, 200.0);
        // a minute of straight flight in between, then a new circle: the old
        // data must not show
        cm.control.fly_mode = FlyMode::StraightFlight;
        for _ in 0..600 {
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            td.update(&cm);
        }
        fly(&mut cm, &mut td, CirclingDirection::Right, 4, 200.0);
        let d = render(&cm, &mut td);
        if std::env::var("SPIDER_DUMP").is_ok() {
            dump_ppm(&d, "/tmp/spider_new.ppm");
        }
        let (_, left, right) = ink_stats(&d);
        let radius = cm.device_const.sizes.vario.ta_circle_radius as f32;
        let full = std::f32::consts::PI * radius * radius;
        assert!(
            ((left + right) as f32) < 0.12 * full,
            "{} ink pixels right after rolling in",
            left + right
        );
    }

    /// The circling detector fires only after circle_hysteresis_tc seconds
    /// of turning. The sectors flown meanwhile are part of the picture the
    /// moment it appears.
    #[test]
    fn spider_includes_the_roll_in_before_the_detector_fires() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        cm.control.vario_mode = VarioMode::Vario;
        cm.control.fly_mode = FlyMode::StraightFlight;
        cm.control.circling_direction = CirclingDirection::Left;
        cm.calculated.av2_climb_rate = 1.0.m_s();
        cm.sensor.average_climb_rate = 1.0.m_s();
        // 7 s of turning (the default hysteresis) while the detector still says straight
        let mut yaw = 0.0_f32;
        for _ in 0..70 {
            cm.sensor.euler_yaw = yaw.deg();
            cm.sensor.climb_rate = 1.5.m_s();
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            td.update(&cm);
            yaw += 3.0;
        }
        cm.control.fly_mode = FlyMode::Circling;
        cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
        td.update(&cm);
        let current = ThermalData::get_idx(cm.sensor.euler_yaw.to_radians());
        let (_, count) = td.flown_run(current, CirclingDirection::Left);
        // 210 degrees of heading = 14 sectors, plus the one of the present heading
        assert!(count >= 14, "only {count} sectors of the roll-in are shown");
    }

    /// A circle with a harmonic of `amp` m/s around `mean`, the average box
    /// standing at `avg` (heading increasing, 3 degrees per tick).
    fn fly_harmonic(
        cm: &mut CoreModel,
        td: &mut ThermalData,
        ticks: usize,
        mean: f32,
        amp: f32,
        avg: f32,
    ) {
        cm.control.vario_mode = VarioMode::Vario;
        cm.control.fly_mode = FlyMode::Circling;
        cm.control.circling_direction = CirclingDirection::Left;
        cm.calculated.av2_climb_rate = avg.m_s();
        cm.sensor.average_climb_rate = avg.m_s();
        let mut yaw = 90.0_f32;
        for _ in 0..ticks {
            cm.sensor.euler_yaw = yaw.rem_euclid(360.0).deg();
            cm.sensor.climb_rate = (mean + amp * (yaw - 30.0).to_radians().cos()).m_s();
            cm.control.alive_ticks = cm.control.alive_ticks.wrapping_add(1);
            td.update(cm);
            yaw += 3.0;
        }
    }

    /// The average box starts far too high after a pull-up (6.9 m/s before
    /// a thermal of 2.8 m/s in a real flight). The assistants compare with
    /// the mean of the circle itself, so the picture is the same.
    #[test]
    fn the_sectors_are_compared_with_the_circle_not_with_the_average_box() {
        let mut pictures = std::vec::Vec::new();
        for avg in [2.5_f32, 6.9] {
            let mut cm = test_model();
            let mut td = ThermalData::default();
            fly_harmonic(&mut cm, &mut td, 130, 2.5, 1.5, avg);
            assert!(
                (td.reference() - 2.5).abs() < 0.1,
                "reference {} at an average box of {avg}",
                td.reference()
            );
            let sum: f32 = (0..THERMAL_DATA_CNT).map(|i| td.value(i)).sum();
            assert!(
                sum.abs() < 0.05,
                "the sectors must scatter around their mean: sum {sum}"
            );
            let (low, high) = td.range();
            assert!(
                low < -1.3 && low > -1.6 && high > 1.3 && high < 1.6,
                "range {low} .. {high}"
            );
            let d = render(&cm, &mut td);
            // the shape reaches beyond the ring of the mean
            let (n_out, _, _) = ink_where(&d, |c| is_heat(c, 20, 29));
            assert!(
                n_out > 300,
                "outer bands missing at an average box of {avg}: {n_out} px"
            );
            pictures.push(d.buf.iter().filter(|p| p.is_some()).count());
            // dots: about half of them above the mean (red or yellow)
            let mut above = 0;
            td.prepare();
            for idx in 0..THERMAL_DATA_CNT {
                let (color, value) = td
                    .get_dotted_item(idx, &cm)
                    .expect("a full circle is flown");
                if value > 0.0 {
                    above += 1;
                    assert!(color == Colors::Red || color == Colors::Yellow);
                } else {
                    assert!(color == Colors::DeepSkyBlue || color == Colors::White);
                }
            }
            assert!(
                (10..=14).contains(&above),
                "{above} of 24 sectors above the mean"
            );
        }
        assert_eq!(
            pictures[0], pictures[1],
            "the average box must not change the spider"
        );
    }

    /// From the first moment: the part flown so far is compared with its own mean
    #[test]
    fn a_partial_circle_is_compared_with_its_own_mean() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        fly_harmonic(&mut cm, &mut td, 40, 2.5, 1.5, 6.9);
        let visited: std::vec::Vec<usize> =
            (0..THERMAL_DATA_CNT).filter(|&i| td.visited(i)).collect();
        assert!(
            visited.len() >= 8 && visited.len() <= 10,
            "{} sectors after 120 degrees",
            visited.len()
        );
        let sum: f32 = visited.iter().map(|&i| td.value(i)).sum();
        assert!(sum.abs() < 0.05, "sum of the flown sectors {sum}");
        td.prepare();
        for idx in 0..THERMAL_DATA_CNT {
            assert_eq!(
                td.get_dotted_item(idx, &cm).is_some(),
                visited.contains(&idx)
            );
        }
    }

    /// 2 m/s better than the mean of the circle reaches the rim; a rough
    /// thermal is scaled down instead of being cut off at the rim.
    #[test]
    fn the_spider_fills_the_centre_circle_and_is_never_cut_off() {
        let radius = test_model().device_const.sizes.vario.ta_circle_radius as f32;
        assert!((spider_scale(2.0, spider_gain(-2.0, 2.0)) - 1.0).abs() < 1e-3);
        assert!(
            (spider_gain(-1.0, 1.0) - SPIDER_PER_MS).abs() < 1e-6,
            "a smooth circle keeps the usual scale"
        );
        for amp in [2.0_f32, 4.0] {
            let mut cm = test_model();
            let mut td = ThermalData::default();
            fly_harmonic(&mut cm, &mut td, 130, 3.0, amp, 3.0);
            let (low, high) = td.range();
            let gain = spider_gain(low, high);
            // best sector at the rim, worst not below the smallest radius
            let best = SPIDER_NULL + gain * high;
            let worst = SPIDER_NULL + gain * low;
            assert!(
                best > 0.93 && best <= 1.0 + 1e-4,
                "amplitude {amp}: best sector at {best}"
            );
            assert!(
                worst >= SPIDER_MIN - 1e-4,
                "amplitude {amp}: worst sector at {worst}"
            );
            let d = render(&cm, &mut td);
            let (rmax, _, _) = ink_stats(&d);
            assert!(
                rmax > 0.9 * radius && rmax <= radius + 1.5,
                "amplitude {amp}: ink out to {rmax} px"
            );
            // one best direction only: the rim is touched on a short arc, not
            // all around as a shape cut off at the rim would be
            let (n_rim, _, _) = ink_where(&d, |c| is_heat(c, 28, 29));
            let (n_all, _, _) = ink_where(&d, |c| is_heat(c, 0, 29));
            assert!(
                (n_rim as f32) < 0.06 * n_all as f32,
                "amplitude {amp}: {n_rim} of {n_all} px in the outermost bands"
            );
        }
        // the contour lines move together when the scale is compressed
        assert!(spider_gain(-4.0, 4.0) < 0.6 * SPIDER_PER_MS);
    }

    /// The worse, the smaller - the stronger, the larger (Stefan 15.11.2026)
    #[test]
    fn a_dot_grows_with_the_climb_of_its_sector() {
        let largest = 25;
        let mut last = 0;
        for tenth in -30..=30 {
            let d = dot_diameter(tenth as f32 * 0.1, 0.0, largest);
            assert!(d >= last, "dot shrinks at {} m/s", tenth as f32 * 0.1);
            last = d;
        }
        assert_eq!(dot_diameter(2.0, 0.0, largest), largest);
        assert_eq!(dot_diameter(-2.0, 0.0, largest), 6);
        assert!(dot_diameter(-1.5, 0.0, largest) < dot_diameter(0.0, 0.0, largest));
        assert!(dot_diameter(0.0, 0.0, largest) < dot_diameter(1.5, 0.0, largest));
        // a rough thermal: the range widens, +2 m/s is no longer the largest
        assert!(dot_diameter(2.0, 4.0, largest) < dot_diameter(4.0, 4.0, largest));
        assert_eq!(dot_diameter(4.0, 4.0, largest), largest);

        // on the display: the best dot (yellow) is the largest, the worst
        // (white) the smallest, and nothing is grey after a full circle
        let mut cm = test_model();
        let mut td = ThermalData::default();
        fly_harmonic(&mut cm, &mut td, 130, 2.5, 1.5, 6.9);
        let mut d = TestDisplay::new();
        draw_thermal_assitant1(&mut d, &cm, &mut td).expect("dots must draw");
        if std::env::var("SPIDER_DUMP").is_ok() {
            dump_ppm(&d, "/tmp/dots_full.ppm");
        }
        let (yellow, _, _) = ink_where(&d, |c| c == Colors::Yellow);
        let (red, _, _) = ink_where(&d, |c| c == Colors::Red);
        let (blue, _, _) = ink_where(&d, |c| c == Colors::DeepSkyBlue);
        let (grey, _, _) = ink_where(&d, |c| c == Colors::DimGray);
        assert_eq!(grey, 0, "grey place holders after a full circle");
        assert!(yellow > 150, "best dot {yellow} px");
        assert!(
            red > 2 * blue,
            "red {red} px, blue {blue} px: the strong half must be the big one"
        );

        // a third of a circle: the rest are small grey place holders
        let mut cm = test_model();
        let mut td = ThermalData::default();
        fly_harmonic(&mut cm, &mut td, 40, 2.5, 1.5, 6.9);
        let mut d = TestDisplay::new();
        draw_thermal_assitant1(&mut d, &cm, &mut td).expect("dots must draw");
        if std::env::var("SPIDER_DUMP").is_ok() {
            dump_ppm(&d, "/tmp/dots_part.ppm");
        }
        let (grey, _, _) = ink_where(&d, |c| c == Colors::DimGray);
        assert!(
            grey > 200,
            "place holders of the 15 sectors not flown: {grey} px"
        );
    }

    /// mean screen angle (degrees clockwise from 12 o'clock) and the radius
    /// range of the white ink
    fn white_ink(d: &TestDisplay) -> (usize, f32, f32, f32) {
        let (mut n, mut sx, mut sy) = (0, 0.0_f32, 0.0_f32);
        let (mut rmin, mut rmax) = (f32::MAX, 0.0_f32);
        for y in 0..DISPLAY_HEIGHT as i32 {
            for x in 0..DISPLAY_WIDTH as i32 {
                if d.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] == Some(Colors::White) {
                    let (dx, dy) = (x as f32 - 240.0, y as f32 - 240.0);
                    n += 1;
                    sx += dx;
                    sy += dy;
                    let r = (dx * dx + dy * dy).sqrt();
                    rmin = rmin.min(r);
                    rmax = rmax.max(r);
                }
            }
        }
        (n, sx.atan2(-sy).to_degrees().rem_euclid(360.0), rmin, rmax)
    }

    /// The white pointer: on the side of the best climb, from the white ring
    /// inwards, in the heading-up picture (the glider symbol of a turn with
    /// increasing heading sits at 9 o'clock = 270 degrees on the screen).
    #[test]
    fn the_pointer_shows_where_to_move_the_circle() {
        for (direction, best) in [
            (CirclingDirection::Left, 30.0_f32),
            (CirclingDirection::Right, 200.0),
        ] {
            let mut cm = test_model();
            let mut td = ThermalData::default();
            // no pointer before the circle is closed
            fly(&mut cm, &mut td, direction, 60, best);
            assert!(td.core_pointer().is_none(), "pointer after half a circle");
            let mut d = TestDisplay::new();
            draw_core_pointer(&mut d, &cm, &td, 74.0).expect("pointer must draw");
            assert_eq!(white_ink(&d).0, 0);

            // (`fly` starts at the same heading each time: a new circle)
            let mut td = ThermalData::default();
            fly(&mut cm, &mut td, direction, 130, best);
            let heading = td
                .core_pointer()
                .expect("pointer after a full circle with an offset")
                .to_degrees();
            assert!(
                ((heading - best + 180.0).rem_euclid(360.0) - 180.0).abs() < 20.0,
                "pointer at {heading}, best climb at {best}"
            );
            let mut d = TestDisplay::new();
            draw_core_pointer(&mut d, &cm, &td, 74.0).expect("pointer must draw");
            let (n, angle, rmin, rmax) = white_ink(&d);
            assert!(n > 120 && n < 400, "pointer of {n} px");
            assert!(
                rmin > 44.0 && rmax < 76.0,
                "pointer from {rmin} to {rmax} px"
            );
            // screen angle of a heading: heading - yaw + the place of the glider symbol
            let yaw = cm.sensor.euler_yaw.to_degrees();
            let symbol = if direction == CirclingDirection::Left {
                270.0
            } else {
                90.0
            };
            let want = (heading - yaw + symbol).rem_euclid(360.0);
            assert!(
                ((angle - want + 180.0).rem_euclid(360.0) - 180.0).abs() < 4.0,
                "pointer drawn at {angle}, wanted {want}"
            );

            // on the dotted assistant it lies inside the ring of dots
            let mut d = TestDisplay::new();
            draw_thermal_assitant1(&mut d, &cm, &mut td).expect("dots must draw");
            let radius = cm.device_const.sizes.vario.ta_circle_radius as f32;
            let mut inner_white = 0;
            for y in 0..DISPLAY_HEIGHT as i32 {
                for x in 0..DISPLAY_WIDTH as i32 {
                    let r = ((x as f32 - 240.0).powi(2) + (y as f32 - 240.0).powi(2)).sqrt();
                    if r < radius - 14.0
                        && d.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] == Some(Colors::White)
                    {
                        inner_white += 1;
                    }
                }
            }
            assert!(
                inner_white > 120,
                "pointer inside the dots: {inner_white} px"
            );
        }
    }

    #[test]
    fn no_pointer_in_a_centred_circle() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        fly_harmonic(&mut cm, &mut td, 130, 2.5, 0.1, 2.5);
        assert!(td.estimate().is_some());
        assert!(
            td.core_pointer().is_none(),
            "pointer at an offset of {}",
            td.estimate().unwrap().offset
        );
        // ... and it comes with an offset
        fly_harmonic(&mut cm, &mut td, 130, 2.5, 1.5, 2.5);
        assert!(td.core_pointer().is_some());
    }

    fn render_ring(cm: &CoreModel, td: &mut ThermalData) -> TestDisplay {
        let mut display = TestDisplay::new();
        draw_thermal_assitant3(&mut display, cm, td).expect("the 3D ring must draw");
        display
    }

    /// The higher the wall, the better the climb (Stefan 16.11.2026)
    #[test]
    fn the_wall_of_the_3d_ring_grows_with_the_climb() {
        let gain = spider_gain(-2.0, 2.0);
        let mut last = 0.0;
        for step in 0..=40 {
            let h = ring_wall(-2.0 + 0.1 * step as f32, gain);
            assert!(h > last, "the wall must grow with the climb");
            last = h;
        }
        assert!(
            (ring_wall(2.0, gain) - RING_H).abs() < 1e-3,
            "the best sector reaches the full height"
        );
        assert!(
            ring_wall(-10.0, gain) >= RING_H * RING_H_MIN - 1e-4,
            "a wall stays in sink too"
        );

        // best part straight ahead at the end of the circle: the wall at the
        // back is high; best part behind: the back is low
        let top_of = |best_deg: f32| {
            let mut cm = test_model();
            let mut td = ThermalData::default();
            fly(&mut cm, &mut td, CirclingDirection::Left, 130, best_deg);
            let d = render_ring(&cm, &mut td);
            let mut top = DISPLAY_HEIGHT as i32;
            for y in 0..DISPLAY_HEIGHT as i32 {
                for x in 0..DISPLAY_WIDTH as i32 {
                    if let Some(c) = d.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] {
                        if is_heat(c, 0, 29) {
                            top = top.min(y);
                        }
                    }
                }
            }
            (top, cm.sensor.euler_yaw.to_degrees())
        };
        // heading at the end of the 130 ticks
        let (_, yaw) = top_of(0.0);
        // `Left`: the glider sits at 9 o'clock with the circle on its right,
        // so the part flown with the heading 90 degrees further lies at the back
        let (far_best, _) = top_of(yaw + 90.0);
        let (near_best, _) = top_of(yaw + 270.0);
        assert!(
            far_best < near_best - 12,
            "best part at the back: the ring must reach higher ({far_best} / {near_best})"
        );
    }

    #[test]
    fn the_3d_ring_stays_inside_the_centre_ring_and_shows_only_the_flown_part() {
        let limit = CENTER_RING_IN - 0.5;
        let mut heat_px = [0usize; 3];
        for (k, ticks) in [0usize, 60, 130].into_iter().enumerate() {
            for amp in [1.0_f32, 4.0] {
                let mut cm = test_model();
                let mut td = ThermalData::default();
                if ticks > 0 {
                    fly_harmonic(&mut cm, &mut td, ticks, 3.0, amp, 3.0);
                }
                let d = render_ring(&cm, &mut td);
                let (rmax, _, _) = ink_stats(&d);
                assert!(
                    rmax <= limit,
                    "{ticks} ticks, amplitude {amp}: ink out to {rmax} px"
                );
                let (n, _, _) = ink_where(&d, |c| is_heat(c, 0, 29));
                heat_px[k] = n;
            }
        }
        assert_eq!(heat_px[0], 0, "no wall before the first sector is flown");
        assert!(
            heat_px[1] > 200 && heat_px[2] > heat_px[1] + 200,
            "the wall grows with the circle: {heat_px:?}"
        );
    }

    #[test]
    fn the_arrow_of_the_3d_ring_points_to_the_core_and_clears_every_wall() {
        let cm = test_model();
        let proj = RingProjection::of(&cm);
        let top = proj.top_view(0.0, 0.0, RING_MAST_H);
        assert!(
            RING_MAST_H > RING_H,
            "the arrow must float above the highest wall"
        );
        for deg in [0.0_f32, 60.0, 135.0, 180.0, 250.0, 300.0] {
            let a = deg.to_radians();
            let (shaft, [tip, b0, b1]) = ring_arrow_corners(&proj, a);
            let (dx, dy) = ((tip.x - top.x) as f32, (tip.y - top.y) as f32);
            // the floor is squeezed in depth: compare in the top view
            let shown = (dx / RING_RX)
                .atan2(-dy / RING_RY)
                .to_degrees()
                .rem_euclid(360.0);
            assert!(
                crate::view::thermal_data::angle_diff(shown.to_radians(), a).abs() < 0.12,
                "arrow for {deg} deg shown at {shown}"
            );
            for p in shaft.iter().chain([tip, b0, b1].iter()) {
                // inside the track, seen from above
                let x = (p.x - top.x) as f32 / (RING_RX * proj.unit);
                let y = (p.y - top.y) as f32 / (RING_RY * proj.unit);
                assert!(
                    (x * x + y * y).sqrt() < 0.9,
                    "arrow for {deg} deg leaves the track"
                );
                // above the highest wall that can stand in front of it: the
                // near wall below the same x
                let reach = (1.0 - x * x).max(0.0).sqrt();
                let wall_top = proj.cy + reach * RING_RY * proj.unit - RING_H * proj.unit;
                assert!(
                    (p.y as f32) < wall_top,
                    "arrow for {deg} deg dips behind the near wall"
                );
                // and inside the centre ring
                let (rx, ry) = (p.x as f32 - 240.0, p.y as f32 - 240.0);
                assert!(
                    (rx * rx + ry * ry).sqrt() < CENTER_RING_IN - 1.0,
                    "arrow for {deg} deg leaves the centre ring"
                );
            }
        }
    }

    /// One colour outside, one inside, a thick edge - no colour for the climb (Stefan 16.11.2026)
    #[test]
    fn the_3d_ring_has_two_wall_colours_and_an_edge() {
        let mut cm = test_model();
        let mut td = ThermalData::default();
        fly(&mut cm, &mut td, CirclingDirection::Left, 130, 30.0);
        let d = render_ring(&cm, &mut td);
        let (outside, _, _) = ink_where(&d, |c| c == RING_OUTSIDE);
        let (inside, _, _) = ink_where(&d, |c| c == RING_INSIDE);
        let (edge, _, _) = ink_where(&d, |c| c == RING_EDGE);
        let (other, _, _) = ink_where(&d, |c| {
            is_heat(c, 0, 29) && c != RING_OUTSIDE && c != RING_INSIDE
        });
        assert!(
            outside > 500 && inside > 300 && edge > 300,
            "outside {outside}, inside {inside}, edge {edge}"
        );
        assert_eq!(other, 0, "no other wall colour");
    }

    /// The large aids of the centering page stay in the band between its
    /// two boxes and clear of the short needle - whatever the thermal.
    #[test]
    fn the_large_aids_stay_between_the_boxes_and_clear_of_the_needle() {
        let small = AidPlace::centre_circle(&test_model());
        for view in [
            CenterView::ThermalAssistant1,
            CenterView::ThermalAssistant2,
            CenterView::ThermalAssistant3,
        ] {
            for (ticks, amp) in [
                (0usize, 1.0_f32),
                (60, 1.0),
                (130, 1.0),
                (130, 4.0),
                (260, 4.0),
            ] {
                let mut cm = test_model();
                let i = cm.config.page_idx as usize;
                cm.config.pages[i].core = crate::model::PageCore::ThermalAssist;
                let mut td = ThermalData::default();
                if ticks > 0 {
                    fly_harmonic(&mut cm, &mut td, ticks, 3.0, amp, 3.0);
                }
                let place = crate::view::vario_ring::centering_place(&cm);
                let mut d = TestDisplay::new();
                view.draw_at(&mut d, &cm, &mut td, &place)
                    .expect("the large aid must draw");
                // the band of the page: 4 px below the top box (y 148), 6 px
                // above the small bottom box
                let (mut top, mut bottom, mut rmax, mut n) = (i32::MAX, 0, 0.0_f32, 0usize);
                for y in 0..DISPLAY_HEIGHT as i32 {
                    for x in 0..DISPLAY_WIDTH as i32 {
                        if let Some(c) = d.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] {
                            if c == Colors::Black {
                                continue;
                            }
                            n += 1;
                            top = top.min(y);
                            bottom = bottom.max(y);
                            let (dx, dy) = ((x - 240) as f32, (y - 240) as f32);
                            rmax = rmax.max((dx * dx + dy * dy).sqrt());
                        }
                    }
                }
                assert!(
                    n > 0,
                    "view {} with {ticks} ticks draws nothing",
                    view.name()
                );
                let band_top = place.center.y as f32 - place.ring - OWN_RING_WIDTH;
                let band_bottom = place.center.y as f32 + place.ring + OWN_RING_WIDTH;
                assert!(
                    top as f32 >= band_top - 1.5 && bottom as f32 <= band_bottom + 1.5,
                    "{} with {ticks} ticks, amplitude {amp}: rows {top} .. {bottom}, band {band_top} .. {band_bottom}",
                    view.name()
                );
                // the needle of the large view ends at r 150
                assert!(
                    rmax < 148.0,
                    "{} with {ticks} ticks, amplitude {amp}: ink out to {rmax} px",
                    view.name()
                );
            }
        }
        // dots and spider keep the proportions of the centre circle
        let cm = test_model();
        let place = crate::view::vario_ring::centering_place(&cm);
        assert!(place.radius / place.ring <= small.radius / small.ring + 1e-3);
        assert!(place.radius / place.ring > 0.95 * small.radius / small.ring);
        assert!((place.dot as f32 / place.radius - small.dot as f32 / small.radius).abs() < 0.02);
        assert!(place.own_ring && !small.own_ring);
    }

    /// The lower edge shows where the wall stands (Stefan 16.11.2026); the
    /// edges are thinner in the small centre circle than on the centering page
    #[test]
    fn the_3d_ring_has_a_lower_edge_and_thinner_edges_when_small() {
        let edge_px = |place_of: &dyn Fn(&CoreModel) -> AidPlace| {
            let mut cm = test_model();
            let mut td = ThermalData::default();
            fly_harmonic(&mut cm, &mut td, 130, 3.0, 0.0, 3.0);
            let place = place_of(&cm);
            let mut d = TestDisplay::new();
            draw_ring3d_at(&mut d, &cm, &td, &place).expect("the 3D ring must draw");
            // a column through the near wall, left of the mast
            let x = place.ring3d_center.x - (0.3 * place.ring3d_unit) as i32;
            let mut runs: heapless::Vec<(i32, i32), 8> = heapless::Vec::new();
            let mut start = None;
            for y in 0..DISPLAY_HEIGHT as i32 {
                let gold = d.buf[(y * DISPLAY_WIDTH as i32 + x) as usize] == Some(RING_EDGE);
                match (gold, start) {
                    (true, None) => start = Some(y),
                    (false, Some(s)) => {
                        let _ = runs.push((s, y - s));
                        start = None;
                    }
                    _ => {}
                }
            }
            runs
        };
        // from the top: upper and lower edge of the far wall, upper and
        // lower edge of the near wall
        let small = edge_px(&|cm| AidPlace::centre_circle(cm));
        let large = edge_px(&|cm| crate::view::vario_ring::centering_place(cm));
        assert_eq!(small.len(), 4, "small ring: {small:?}");
        assert_eq!(large.len(), 4, "large ring: {large:?}");
        let width =
            |runs: &[(i32, i32)]| runs.iter().map(|r| r.1).sum::<i32>() as f32 / runs.len() as f32;
        assert!(
            width(&small) >= 1.5 && width(&small) <= 3.2,
            "edges of the small ring: {small:?}"
        );
        assert!(
            width(&large) > width(&small),
            "edges of the large ring are wider: {large:?} / {small:?}"
        );
    }

    /// Pictures of the 3D ring alone, for a look:
    /// `RENDER_DIR=... cargo test --lib ring3d_views -- --ignored`
    #[test]
    #[ignore]
    fn ring3d_views() {
        let dir = match std::env::var("RENDER_DIR") {
            Ok(d) => d,
            Err(_) => return,
        };
        for (name, direction, ticks, best_off) in [
            ("best_far", CirclingDirection::Left, 130, 90.0_f32),
            ("best_near", CirclingDirection::Left, 130, 270.0),
            ("best_opposite", CirclingDirection::Left, 130, 180.0),
            ("best_here", CirclingDirection::Left, 130, 0.0),
            ("part", CirclingDirection::Left, 70, 180.0),
            ("other_direction", CirclingDirection::Right, 130, 180.0),
        ] {
            // heading at the end of the run: 90 deg at the start, 3 deg per tick
            let turn = match direction {
                CirclingDirection::Left => 3.0,
                CirclingDirection::Right => -3.0,
            };
            let yaw_end = 90.0 + turn * (ticks as f32 - 1.0);
            let mut cm = test_model();
            let mut td = ThermalData::default();
            fly(&mut cm, &mut td, direction, ticks, yaw_end + best_off);
            let d = render_ring(&cm, &mut td);
            dump_ppm(&d, &std::format!("{dir}/ring3d_{name}.ppm"));
        }
    }

    /// timing aid (host): `cargo test --release spider_bench -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn spider_bench() {
        use std::time::Instant;
        let mut cm = test_model();
        let mut td = ThermalData::default();
        cm.sensor.euler_roll = 45.0.deg();
        fly(&mut cm, &mut td, CirclingDirection::Left, 130, 30.0);
        let n = 200;
        let mut d = TestDisplay::new();
        let t = Instant::now();
        for _ in 0..n {
            draw_spider(&mut d, &cm, &td).unwrap();
        }
        let spider = t.elapsed().as_secs_f64() / n as f64;
        let t = Instant::now();
        for _ in 0..n {
            draw_action_arcs(&mut d, &cm, &td).unwrap();
        }
        let arcs = t.elapsed().as_secs_f64() / n as f64;
        let t = Instant::now();
        for _ in 0..n {
            draw_thermal_assitant3(&mut d, &cm, &mut td).unwrap();
        }
        let ring = t.elapsed().as_secs_f64() / n as f64;
        // reference: the whole ring page without a centre view
        let mut cm2 = cm;
        cm2.control.alive_ticks = 1000;
        cm2.config.overlay_active = crate::model::OverlayActive::None;
        let idx = cm2.config.page_idx as usize;
        cm2.config.pages[idx].core = crate::model::PageCore::Boxes;
        cm2.config.pages[idx].center_circling = CenterView::None;
        let cm_1s = cm2;
        let t = Instant::now();
        for _ in 0..n {
            crate::view::vario_ring::draw_ring(&mut d, &cm2, &cm_1s, &mut td).unwrap();
        }
        let page = t.elapsed().as_secs_f64() / n as f64;
        eprintln!(
            "host timing per frame: spider {:.3} ms, arcs {:.3} ms, 3D ring {:.3} ms, ring page without centre view {:.3} ms",
            spider * 1e3, arcs * 1e3, ring * 1e3, page * 1e3
        );
    }
}
