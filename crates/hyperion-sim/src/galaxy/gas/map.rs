//! Extinction maps: the dust layer the galaxy map draws (plan 07, P07.T9 and Design note 19).
//!
//! A map pixel is the visual extinction through the whole galaxy along its line of sight, in
//! magnitudes, from the mean field: lanes included, noise not, since a pixel is hundreds of
//! light-years across and a column through the disc averages the noise out. The shape is plan 02's
//! [`galaxy::map`](crate::galaxy::map), so that plan 04's banded map service can drive both:
//!
//! - **Face-on** ([`extinction_face_on`]), looking down the z axis: every layer is exponential in
//!   height and the dust-to-gas ratio `ζ` reads the cylindrical radius alone, so the column at a
//!   point is the closed form `ζ Σ 2 h n(R, 0)` over the three layers, the lane factor on the
//!   neutral one. A lane is narrower than a pixel, so the pixel is the mean of that closed form
//!   over a fixed 4 × 4 sub-grid, which is why this function takes the pixel's size where plan
//!   02's face-on column does not.
//! - **Edge-on** ([`extinction_edge_on`]), looking along `+y` at a pixel that spans `x` and the
//!   heights `z_lo` to `z_hi`: the column along `+y` through the whole root cube, averaged over the
//!   pixel's height. Each layer is its in-plane density times `exp(−|z| ÷ h)`, so the double
//!   integral is, layer by layer, the line integral of `ζ n(R, θ, 0)` along `+y` — which depends on
//!   `x` alone — times the closed-form mean of `exp(−|z| ÷ h)` over the pixel's height, which
//!   depends on the row alone. Each mean is a difference of two exponentials clamped at 0 by
//!   comparison, never by `f64::max`. A band of rows computes each column's line integrals once
//!   and shares them across its rows, which changes no value (plan 02, P02.T10.b, whose private
//!   machinery this module writes afresh).
//!
//! The line integrals are the extinction integral's two-point Gauss–Legendre rule on steps of
//! 32 ly across the whole cube: the integral's noise step at full quality, which is also its
//! step in the plane, where the lane width and the neutral layer's height are the smooth scales and
//! both exceed 64 ly. The molecular disc's height, which the three-dimensional integral resolves
//! near the centre, is here in the closed-form mean, and its in-plane scale length is 200 ly or
//! more.
//!
//! The sub-grid, the step and the rule are part of the generator version, because the pictures a
//! saved universe shows must not move under it.

use std::ops::Range;

use crate::coords::ROOT_HALF_WIDTH_LY;
use crate::galaxy::PointLy;
use crate::galaxy::gas::CENTIMETRES_PER_LIGHT_YEAR;
use crate::galaxy::gas::ccm::HYDROGEN_COLUMN_PER_MAG;
use crate::galaxy::gas::field::{GasField, Site};
use crate::galaxy::gas::smooth::{self, GasLayer};
use crate::galaxy::map::{MapSpec, MapView};
use crate::units::Magnitudes;

/// The root cube's half-width, ly: where every edge-on line of sight starts and ends.
const ROOT_HALF: f64 = 65_536.0;

const _: () = assert!(
    ROOT_HALF_WIDTH_LY == 65_536,
    "ROOT_HALF is the root cube's half-width"
);

/// The edge-on line integral's step, ly.
const LINE_STEP_LY: f64 = 32.0;

/// The edge-on line integral's steps across the whole cube: `2 × 65,536 ÷ 32`.
const LINE_STEPS: u32 = 4_096;

/// The two-point Gauss–Legendre rule's nodes, `±1 ÷ √3`, on `[−1, 1]`.
const GL2_NODE: f64 = 0.577_350_269_189_625_7;

/// The face-on sub-grid's side: a pixel is the mean of 4 × 4 closed-form columns.
const SUB_GRID: u32 = 4;

/// Magnitudes of visual extinction per unit of dust-weighted density times light-years: cm per ly
/// over the hydrogen column per magnitude.
const MAG_PER_DENSITY_LY: f64 = CENTIMETRES_PER_LIGHT_YEAR / HYDROGEN_COLUMN_PER_MAG;

/// [`LINE_STEP_LY`] as a whole number, for the check that the steps tile the cube.
const LINE_STEP_WHOLE_LY: i64 = 32;

const _: () = assert!(
    LINE_STEP_WHOLE_LY * LINE_STEPS as i64 == 2 * ROOT_HALF_WIDTH_LY as i64,
    "the edge-on steps tile the cube's width"
);

/// The visual extinction at `(x, y)` (ly) looking down the z axis through the whole galaxy, over a
/// pixel `pixel_ly` light-years square: the mean of the closed-form column `ζ Σ 2 h n(R, 0)` over a
/// 4 × 4 sub-grid of the pixel (module documentation).
///
/// A `pixel_ly` of 0 is the column at the point itself.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::map::extinction_face_on;
///
/// let galaxy = Galaxy::new(Seed::new(42));
/// // Straight through the disc near the Sun: about a magnitude (1.2 for this galaxy), against
/// // several times that through the centre.
/// let sun = extinction_face_on(galaxy.gas(), 0.0, 26_000.0, 256.0).value();
/// let centre = extinction_face_on(galaxy.gas(), 0.0, 0.0, 256.0).value();
/// assert!(sun < 2.0 && centre > 5.0 * sun, "{sun} mag at the Sun, {centre} at the centre");
/// ```
#[must_use]
pub fn extinction_face_on(field: &GasField, x: f64, y: f64, pixel_ly: f64) -> Magnitudes {
    let side = f64::from(SUB_GRID);
    let mut sum = 0.0;
    for i in 0..SUB_GRID {
        let dx = pixel_ly * ((f64::from(i) + 0.5) / side - 0.5);
        for j in 0..SUB_GRID {
            let dy = pixel_ly * ((f64::from(j) + 0.5) / side - 0.5);
            sum += face_on_column(field, x + dx, y + dy);
        }
    }
    Magnitudes::new(sum / (side * side) * MAG_PER_DENSITY_LY)
}

/// `ζ Σ 2 h n(R, 0)` at the in-plane point `(x, y)` (ly), cm⁻³ ly: the dust-weighted vertical
/// column in closed form.
fn face_on_column(field: &GasField, x: f64, y: f64) -> f64 {
    let site = Site::of_ly(&PointLy::new(x, y, 0.0));
    let layers = field.layers(&site);
    let gas = field.smooth();
    let column = 2.0 * gas.height(GasLayer::Neutral) * layers.neutral
        + 2.0 * gas.height(GasLayer::Warm) * layers.warm
        + 2.0 * gas.height(GasLayer::Molecular) * layers.molecular;
    field.zeta(site.r) * column
}

/// The visual extinction at `x` (ly) through the pixel spanning the heights `z_lo` to `z_hi` (ly),
/// looking along `+y` through the whole root cube: `∫∫ ζ n dy dz ÷ (z_hi − z_lo)` over the mean
/// field (module documentation).
///
/// It is the value [`render_extinction_rows`] gives that pixel, bit for bit, when given the row's
/// own heights, [`MapSpec::pixel_span`].
///
/// # Panics
///
/// In debug builds, if `z_hi` is not above `z_lo`: a pixel of no height has no mean column.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::map::extinction_edge_on;
///
/// let galaxy = Galaxy::new(Seed::new(42));
/// // Through the centre in the plane, tens of magnitudes; 8,000 ly above it, very little.
/// let plane = extinction_edge_on(galaxy.gas(), 128.0, 0.0, 256.0).value();
/// let above = extinction_edge_on(galaxy.gas(), 128.0, 8_000.0, 8_256.0).value();
/// assert!(plane > 10.0 && above < 0.1 * plane);
/// ```
#[must_use]
pub fn extinction_edge_on(field: &GasField, x: f64, z_lo: f64, z_hi: f64) -> Magnitudes {
    debug_assert!(
        z_hi > z_lo,
        "a pixel from {z_lo} to {z_hi} ly has no height"
    );
    pixel(&plane_lines(field, x), &vertical_means(field, z_lo, z_hi))
}

/// Fills `out` with the rows `rows` of the raster `spec`, row by row and left to right, in
/// magnitudes: [`extinction_face_on`] at each pixel's centre over the pixel's size face-on and
/// [`extinction_edge_on`] over each pixel's height edge-on, bit for bit. `spec`'s selection is not
/// read: the dust is one quantity whatever systems a density map counts.
///
/// `out` is cleared first and ends with `rows.len() × spec.width_px()` values, so a pool can render
/// the bands of one `spec` into buffers of their own and assemble them in order: a pixel's value
/// depends on the pixel alone, never on the band it was rendered in (plan 04, P04.T11).
///
/// # Panics
///
/// If `rows` reaches past `spec.height_px()`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::map::render_extinction_rows;
/// use hyperion_sim::galaxy::map::{MapSelection, MapSpec, MapView};
///
/// let galaxy = Galaxy::new(Seed::new(42));
/// let (view, selection) = (MapView::EdgeOn, MapSelection::AllSystems);
/// let spec = MapSpec::new(view, selection, [32, 16], [0.0, 0.0], 4_096.0)?;
/// let mut rows = Vec::new();
/// render_extinction_rows(galaxy.gas(), &spec, 7..9, &mut rows);
/// assert_eq!(rows.len(), 2 * 32);
/// // The two rows either side of the plane are mirror images: the dust is symmetric in z.
/// assert_eq!(rows[..32], rows[32..]);
/// # Ok::<(), hyperion_sim::galaxy::map::BuildMapSpecError>(())
/// ```
pub fn render_extinction_rows(
    field: &GasField,
    spec: &MapSpec,
    rows: Range<u32>,
    out: &mut Vec<f64>,
) {
    assert!(
        rows.end <= spec.height_px(),
        "rows {rows:?} reach past the map's {} rows",
        spec.height_px()
    );
    let width = usize::try_from(spec.width_px()).expect("a map's width fits the address space");
    let band = usize::try_from(rows.end.saturating_sub(rows.start))
        .expect("a band's rows fit the address space");
    out.clear();
    out.resize(
        band.checked_mul(width)
            .expect("a band of a map fits memory"),
        0.0,
    );
    let columns = || (0..spec.width_px()).map(|column| spec.pixel_centre(column, 0)[0]);
    match spec.view() {
        MapView::FaceOn => {
            for (row, line) in rows.zip(out.chunks_mut(width)) {
                for (column, pixel) in (0..spec.width_px()).zip(line.iter_mut()) {
                    let [x, y] = spec.pixel_centre(column, row);
                    *pixel = extinction_face_on(field, x, y, spec.ly_per_px()).value();
                }
            }
        }
        MapView::EdgeOn => {
            let lines: Vec<[f64; 3]> = columns().map(|x| plane_lines(field, x)).collect();
            for (row, line) in rows.zip(out.chunks_mut(width)) {
                let [z_lo, z_hi] = spec.pixel_span(row);
                let means = vertical_means(field, z_lo, z_hi);
                for (pixel, lines) in line.iter_mut().zip(&lines) {
                    *pixel = pixel_value(lines, &means);
                }
            }
        }
    }
}

/// An edge-on pixel from its column's line integrals and its row's vertical means.
fn pixel(lines: &[f64; 3], means: &[f64; 3]) -> Magnitudes {
    Magnitudes::new(pixel_value(lines, means))
}

/// `Σ line × mean` over the layers, in magnitudes.
fn pixel_value(lines: &[f64; 3], means: &[f64; 3]) -> f64 {
    (lines[0] * means[0] + lines[1] * means[1] + lines[2] * means[2]) * MAG_PER_DENSITY_LY
}

/// Each layer's `∫ ζ n(R, θ, 0) dy` along `+y` at `x` (ly) across the whole root cube, cm⁻³ ly, in
/// [`GasLayer::ALL`]'s order: two-point Gauss–Legendre on steps of 32 ly.
fn plane_lines(field: &GasField, x: f64) -> [f64; 3] {
    let mut sums = [0.0; 3];
    let half = 0.5 * LINE_STEP_LY;
    for step in 0..LINE_STEPS {
        let middle = -ROOT_HALF + LINE_STEP_LY * (f64::from(step) + 0.5);
        for y in [middle - half * GL2_NODE, middle + half * GL2_NODE] {
            let site = Site::of_ly(&PointLy::new(x, y, 0.0));
            let layers = field.layers(&site);
            let zeta = field.zeta(site.r);
            sums[0] += half * zeta * layers.neutral;
            sums[1] += half * zeta * layers.warm;
            sums[2] += half * zeta * layers.molecular;
        }
    }
    sums
}

/// Each layer's mean of `exp(−|z| ÷ h)` over the heights `z_lo` to `z_hi` (ly), in
/// [`GasLayer::ALL`]'s order: its closed-form integral over the pixel, a difference clamped at 0 by
/// comparison, over the pixel's height.
fn vertical_means(field: &GasField, z_lo: f64, z_hi: f64) -> [f64; 3] {
    let height = z_hi - z_lo;
    GasLayer::ALL
        .map(|layer| smooth::layer_thickness(field.smooth().height(layer), z_lo, z_hi) / height)
}
