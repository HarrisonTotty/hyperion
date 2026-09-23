//! Column densities for the galaxy map: the pictures the `GALAXY` display draws (brainstorm,
//! "Visualiser"; plan 02, P02.T10).
//!
//! A map is the number of systems per square light-year along a line of sight, from the density
//! fields alone, so it never places a star. Two views:
//!
//! - **Face-on** ([`column_density_face_on`]), looking down the z axis at the point `(x, y)`: the
//!   whole column from `−∞` to `+∞`. Every disc and the bar integrate in closed form, since their
//!   vertical profiles integrate to twice an effective height; the bulge and each halo component
//!   take a 32-node quadrature after a substitution that maps the half-line, or the chord inside
//!   the halo's cut, onto the unit interval.
//! - **Edge-on** ([`column_density_edge_on`]), looking along `+y` at a pixel that spans `x` and the
//!   heights `z_lo` to `z_hi`: the column along `+y`, averaged over the pixel's height, so that a
//!   disc thinner than a pixel keeps its light instead of falling between samples. The height
//!   integral is closed-form for every disc and for the bar, and a 4-node rule on panels of at most
//!   [`PANEL_SCALES`] of the component's vertical scale for the bulge and the halo; along the line of
//!   sight the integral runs over fixed panels symmetric about `y = 0` with [`gl16`] on each.
//!
//! [`render_rows`] fills a range of a [`MapSpec`]'s rows, so that a pool can split a map into bands
//! and assemble them: the value of a pixel depends on the pixel alone, never on the band it was
//! rendered in (plan 04, P04.T11).
//!
//! The panel schemes, the substitutions and the node counts here are part of the generator version,
//! because the pictures a saved universe shows must not move under it.
//!
//! # Examples
//!
//! ```
//! use hyperion_sim::Seed;
//! use hyperion_sim::galaxy::Galaxy;
//! use hyperion_sim::galaxy::map::{MapSelection, MapSpec, MapView};
//! use hyperion_sim::galaxy::map::{column_density_face_on, render_rows};
//!
//! let galaxy = Galaxy::new(Seed::new(42));
//! let fields = galaxy.fields();
//! // The centre of the galaxy holds far more systems per square light-year than the solar circle.
//! let centre = column_density_face_on(fields, 0.0, 0.0, MapSelection::AllSystems);
//! let sun = column_density_face_on(fields, 22_516.7, 13_000.0, MapSelection::AllSystems);
//! assert!(centre > 100.0 * sun);
//! // A 64 × 64 picture of the whole root cube, rendered in two bands.
//! let spec = MapSpec::new(
//!     MapView::FaceOn,
//!     MapSelection::AllSystems,
//!     [64, 64],
//!     [0.0, 0.0],
//!     2_048.0,
//! )?;
//! let (mut top, mut bottom) = (Vec::new(), Vec::new());
//! render_rows(fields, &spec, 0..32, &mut top);
//! render_rows(fields, &spec, 32..64, &mut bottom);
//! assert_eq!(top.len() + bottom.len(), 64 * 64);
//! # Ok::<(), hyperion_sim::galaxy::map::BuildMapSpecError>(())
//! ```

use std::error::Error;
use std::fmt;
use std::ops::Range;

use super::Population;
use super::fields::arms::Arm;
use super::fields::bar::LongBar;
use super::fields::bulge::BoxyBulge;
use super::fields::disc::ExponentialDisc;
use super::fields::halo::HaloProfile;
use super::fields::{Component, Fields, MAX_COMPONENTS, Shape, VerticalProfile};
use super::quad::{gl4, gl16, gl32};
use crate::math;

/// The root cube's half-width in light-years: the far end of every line of sight, since no system
/// lies outside it (plan 01's `coords::ROOT_HALF_WIDTH_LY`, which a unit test holds this equal to).
const ROOT_HALF: f64 = 65_536.0;

/// The first panel of a line of sight runs from 0 to this height in light-years, and the
/// logarithmic panels beyond it, because `ln 0` has no value.
const INNER_PANEL: f64 = 16.0;

/// How many logarithmic panels a side a component without arms takes, beyond [`INNER_PANEL`].
const LOG_PANELS: u32 = 8;

/// The widest panel a disc with arms takes along a line of sight, light-years: narrow enough that
/// no panel spans an arm of the narrowest drawn width (250 ly) unresolved.
const ARM_PANEL: f64 = 512.0;

/// A disc whose profile integrates to less than this fraction of its whole column across a pixel's
/// height is left out of that pixel: its light is lost in the last bits of the sum anyway, and its
/// line of sight is the map's dearest integral (plan 02, P02.T10.b).
const DISC_FLOOR: f64 = 1e-9;

/// Which way a map looks at the galaxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MapView {
    /// From galactic north, down the z axis: the picture spans x and y.
    FaceOn,
    /// Along `+y`, so that the bar lies in the picture's plane: it spans x and z.
    EdgeOn,
}

/// Which systems a map counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MapSelection {
    /// Every population's systems.
    AllSystems,
    /// The young thin disc alone, which is where the arms show (brainstorm, "Visualiser").
    YoungOnly,
}

/// A [`MapSpec`] could not be built from what it was given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildMapSpecError {
    /// The map has no pixels: one of its dimensions is 0.
    EmptySize {
        /// The width in pixels.
        width_px: u32,
        /// The height in pixels.
        height_px: u32,
    },
    /// A pixel is not a positive, finite number of light-years across.
    PixelSize {
        /// The light-years per pixel given.
        ly_per_px: f64,
    },
    /// The centre is not finite, or the picture reaches outside the root cube, `[−65,536, 65,536]`
    /// ly, on the named axis.
    OutsideRootCube {
        /// Which axis of the picture, 0 across and 1 up.
        axis: usize,
        /// The picture's centre on that axis, light-years.
        centre: f64,
        /// The picture's extent on that axis, light-years.
        extent: f64,
    },
    /// The pixel is too small to tell the picture's rows apart this far up the picture: two
    /// consecutive row edges would round to the same light-year, leaving a row with no height and,
    /// edge-on, no mean column ([`MapSpec::pixel_span`]).
    UnresolvedRows {
        /// The light-years per pixel given.
        ly_per_px: f64,
        /// The picture's centre up the picture, light-years.
        centre: f64,
    },
}

impl fmt::Display for BuildMapSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySize {
                width_px,
                height_px,
            } => write!(f, "a map of {width_px} by {height_px} pixels has no pixels"),
            Self::PixelSize { ly_per_px } => {
                write!(f, "a map's pixel of {ly_per_px} ly is not a positive size")
            }
            Self::OutsideRootCube {
                axis,
                centre,
                extent,
            } => write!(
                f,
                "a map {extent} ly wide centred on {centre} ly reaches outside the root cube on \
                 axis {axis} of its picture"
            ),
            Self::UnresolvedRows { ly_per_px, centre } => write!(
                f,
                "a map's pixel of {ly_per_px} ly cannot tell its rows apart {centre} ly up the \
                 picture"
            ),
        }
    }
}

impl Error for BuildMapSpecError {}

/// One raster of the galaxy map: its view, what it counts, its size in pixels and where its pixels
/// lie in the galaxy.
///
/// Pixels are squares `ly_per_px` light-years across, as the wire's `DensityMap` has them (plan
/// 04), and `centre` is the point at the middle of the picture: `(x, y)` face-on and `(x, z)`
/// edge-on. Column `i` and row `j` have their centre at
///
/// ```text
/// across = centre[0] + (i + ½ − width_px ÷ 2) × ly_per_px
/// up     = centre[1] + (height_px ÷ 2 − j − ½) × ly_per_px
/// ```
///
/// so row 0 is the top of the picture, at the largest `y` or `z`. Edge-on, row `j` spans the
/// heights between the same formula's `j` and `j + 1` without the half, and the column is averaged
/// over them.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::map::{BuildMapSpecError, MapSelection, MapSpec, MapView};
///
/// // M1's face-on map: the whole root cube across, 512 pixels of 256 ly (plan 04).
/// let spec = MapSpec::new(
///     MapView::FaceOn,
///     MapSelection::AllSystems,
///     [512, 512],
///     [0.0, 0.0],
///     256.0,
/// )?;
/// assert_eq!(spec.extent(), [131_072.0, 131_072.0]);
/// assert_eq!(spec.pixel_centre(0, 0), [-65_408.0, 65_408.0]);
/// // A picture wider than the root cube is refused.
/// assert!(matches!(
///     MapSpec::new(MapView::FaceOn, MapSelection::AllSystems, [512, 512], [1.0, 0.0], 256.0),
///     Err(BuildMapSpecError::OutsideRootCube { axis: 0, .. })
/// ));
/// # Ok::<(), BuildMapSpecError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapSpec {
    view: MapView,
    selection: MapSelection,
    width_px: u32,
    height_px: u32,
    centre: [f64; 2],
    ly_per_px: f64,
}

impl MapSpec {
    /// A raster of `size` pixels, `[width, height]`, each `ly_per_px` light-years square, centred
    /// on `centre` in the picture's own axes.
    ///
    /// # Errors
    ///
    /// [`BuildMapSpecError`] if either dimension is 0, if a pixel is not a positive finite number
    /// of light-years, if the picture, centre included, is not finite or reaches outside the root
    /// cube, or if the pixel is too small to separate the picture's rows where it lies. The edges
    /// of the cube are allowed, as M1's maps span it exactly.
    pub fn new(
        view: MapView,
        selection: MapSelection,
        size: [u32; 2],
        centre: [f64; 2],
        ly_per_px: f64,
    ) -> Result<Self, BuildMapSpecError> {
        let [width_px, height_px] = size;
        if width_px == 0 || height_px == 0 {
            return Err(BuildMapSpecError::EmptySize {
                width_px,
                height_px,
            });
        }
        if !(ly_per_px.is_finite() && ly_per_px > 0.0) {
            return Err(BuildMapSpecError::PixelSize { ly_per_px });
        }
        let spec = Self {
            view,
            selection,
            width_px,
            height_px,
            centre,
            ly_per_px,
        };
        for (axis, (&middle, extent)) in centre.iter().zip(spec.extent()).enumerate() {
            let half = 0.5 * extent;
            let inside = middle.is_finite()
                && middle - half >= -ROOT_HALF
                && middle + half <= ROOT_HALF
                && extent.is_finite();
            if !inside {
                return Err(BuildMapSpecError::OutsideRootCube {
                    axis,
                    centre: middle,
                    extent,
                });
            }
        }
        // A pixel of the order of the last place of the picture's own coordinates would leave two
        // consecutive row edges on the same `f64`: the row would have no height and its mean column
        // would be 0 ÷ 0, a NaN pixel in a release build, where `column_density_edge_on`'s
        // assertion is gone. Row edges are spaced by `ly_per_px` and each is rounded by at most
        // half the last place of the largest of them, so twice that place is a sufficient pixel.
        // Which rows collapse is a quantisation pattern, not a monotone one — the outermost rows
        // miss about a tenth of the cases — so the test is on the pixel, not on a row.
        let up = spec.extent()[1];
        let top = (centre[1] + 0.5 * up)
            .abs()
            .max((centre[1] - 0.5 * up).abs());
        if ly_per_px <= 2.0 * top * f64::EPSILON {
            return Err(BuildMapSpecError::UnresolvedRows {
                ly_per_px,
                centre: centre[1],
            });
        }
        Ok(spec)
    }

    /// Which way the map looks at the galaxy.
    #[must_use]
    pub fn view(&self) -> MapView {
        self.view
    }

    /// Which systems the map counts.
    #[must_use]
    pub fn selection(&self) -> MapSelection {
        self.selection
    }

    /// The width in pixels, never 0.
    #[must_use]
    pub fn width_px(&self) -> u32 {
        self.width_px
    }

    /// The height in pixels, never 0.
    #[must_use]
    pub fn height_px(&self) -> u32 {
        self.height_px
    }

    /// The point at the middle of the picture: `(x, y)` face-on and `(x, z)` edge-on, light-years.
    #[must_use]
    pub fn centre(&self) -> [f64; 2] {
        self.centre
    }

    /// The size of a pixel, light-years square.
    #[must_use]
    pub fn ly_per_px(&self) -> f64 {
        self.ly_per_px
    }

    /// The picture's extent, light-years across and up: its size in pixels times
    /// [`ly_per_px`](Self::ly_per_px).
    #[must_use]
    pub fn extent(&self) -> [f64; 2] {
        [
            f64::from(self.width_px) * self.ly_per_px,
            f64::from(self.height_px) * self.ly_per_px,
        ]
    }

    /// The number of pixels the whole raster holds.
    #[must_use]
    pub fn pixel_count(&self) -> u64 {
        u64::from(self.width_px) * u64::from(self.height_px)
    }

    /// The centre of column `column` and row `row`, light-years in the picture's axes (the type's
    /// documentation). An index past the raster's edge follows the same formula.
    #[must_use]
    pub fn pixel_centre(&self, column: u32, row: u32) -> [f64; 2] {
        [
            self.across(f64::from(column) + 0.5),
            self.up(f64::from(row)),
        ]
    }

    /// The picture's horizontal coordinate, x in both views, at `column` pixels from the left edge.
    fn across(&self, column: f64) -> f64 {
        self.centre[0] + (column - 0.5 * f64::from(self.width_px)) * self.ly_per_px
    }

    /// The picture's vertical coordinate, y face-on and z edge-on, at the centre of row `row`.
    fn up(&self, row: f64) -> f64 {
        self.centre[1] + (0.5 * f64::from(self.height_px) - row - 0.5) * self.ly_per_px
    }

    /// The heights row `row` spans, edge-on: `[z_lo, z_hi]`, with the row's centre between them.
    ///
    /// Consecutive rows share an edge bit for bit, so a column of pixels covers its line without a
    /// gap or an overlap, and every row of a built [`MapSpec`] has a height above 0. These are the
    /// heights [`render_rows`] gives [`column_density_edge_on`], which reproduces a pixel only when
    /// given them.
    #[must_use]
    pub fn pixel_span(&self, row: u32) -> [f64; 2] {
        let edge =
            |row: f64| self.centre[1] + (0.5 * f64::from(self.height_px) - row) * self.ly_per_px;
        [edge(f64::from(row) + 1.0), edge(f64::from(row))]
    }
}

/// The column density at `(x, y)` (ly) looking down the z axis, systems per square light-year:
/// `∫ n dz` over the whole line, in component order (plan 02, P02.T10.a).
///
/// Every disc contributes `2 h n0 exp(−R ÷ L)` times its arm factor, with `h` the effective height
/// of its vertical profile, and the bar `2 h` times its density in the plane; the bulge and each
/// halo component take [`gl32`] after a substitution that maps the half-line, or the chord inside
/// the cut, onto the unit interval.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fields::Fields;
/// use hyperion_sim::galaxy::map::{MapSelection, column_density_face_on};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
///
/// let params = GalaxyParams::milky_way_like();
/// let fields = Fields::new(&params, &MassModel::new(&params));
/// let all = column_density_face_on(&fields, 22_516.7, 13_000.0, MapSelection::AllSystems);
/// let young = column_density_face_on(&fields, 22_516.7, 13_000.0, MapSelection::YoungOnly);
/// // The young thin disc is a small part of the whole column, and the arms are its own.
/// assert!(young > 0.0 && young < 0.2 * all);
/// ```
#[must_use]
pub fn column_density_face_on(fields: &Fields, x: f64, y: f64, selection: MapSelection) -> f64 {
    let arm = fields.arms().point(x, y);
    let r_sq = x * x + y * y;
    let r = r_sq.sqrt();
    fields
        .components()
        .iter()
        .filter(|c| counted(c, selection))
        .fold(0.0, |sum, component| {
            sum + match component.shape() {
                Shape::Disc(disc) => {
                    let factor = disc.arm().map_or(1.0, |arm_of| arm_of.factor(&arm));
                    2.0 * disc.height().value() * disc.envelope(r, 0.0) * factor
                }
                Shape::Bulge(bulge) => bulge_face_on(bulge, x, y),
                Shape::Bar(bar) => 2.0 * bar.height().value() * bar.envelope(x.abs(), y, 0.0),
                Shape::Halo(halo) => halo_face_on(halo, r_sq),
            }
        })
}

/// The column density at `x` (ly) through the pixel spanning the heights `z_lo` to `z_hi` (ly),
/// looking along `+y`: `∫∫ n dy dz ÷ (z_hi − z_lo)`, systems per square light-year (plan 02,
/// P02.T10.b).
///
/// Dividing by the pixel's height makes the value the mean column over the pixel, so a disc
/// thinner than the pixel keeps its light. It is the value [`render_rows`] gives that pixel, bit
/// for bit, when given the row's own heights, [`MapSpec::pixel_span`].
///
/// # Panics
///
/// In debug builds, if `z_hi` is not above `z_lo`: a pixel of no height has no mean column, and
/// dividing by its height would give a NaN.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fields::Fields;
/// use hyperion_sim::galaxy::map::{MapSelection, column_density_edge_on};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
///
/// let params = GalaxyParams::milky_way_like();
/// let fields = Fields::new(&params, &MassModel::new(&params));
/// // Through the solar circle: the pixel at the plane against one 8,000 ly above it.
/// let plane = column_density_edge_on(&fields, 26_000.0, 0.0, 256.0, MapSelection::AllSystems);
/// let above = column_density_edge_on(&fields, 26_000.0, 8_000.0, 8_256.0, MapSelection::AllSystems);
/// assert!(plane > 100.0 * above);
/// ```
#[must_use]
pub fn column_density_edge_on(
    fields: &Fields,
    x: f64,
    z_lo: f64,
    z_hi: f64,
    selection: MapSelection,
) -> f64 {
    debug_assert!(
        z_hi > z_lo,
        "a pixel from {z_lo} to {z_hi} ly has no height"
    );
    let plan = EdgeOnPlan::new(fields, selection);
    let mut means = [0.0; MAX_PARTS];
    plan.vertical_means(z_lo, z_hi, &mut means);
    let mut lines = [0.0; MAX_PARTS];
    plan.line_integrals(x, &wanted_parts(&[means]), &mut lines);
    plan.pixel(x, z_lo, z_hi, &means, &lines)
}

/// Which parts any row of a band wants: the parts whose vertical mean over some row is above 0, and
/// so whose line of sight must be integrated.
///
/// A mask, not the sum of the means, so that what a band computes cannot depend on the value of a
/// sum taken over the band's rows: a pixel's value depends on the pixel alone (plan 04, P04.T11).
fn wanted_parts(means: &[[f64; MAX_PARTS]]) -> [bool; MAX_PARTS] {
    let mut wanted = [false; MAX_PARTS];
    for row in means {
        for (slot, &mean) in wanted.iter_mut().zip(row.iter()) {
            debug_assert!(mean >= 0.0, "a vertical mean of {mean} is below 0");
            *slot |= mean > 0.0;
        }
    }
    wanted
}

/// Fills `out` with the rows `rows` of the raster `spec`, row by row and left to right, in systems
/// per square light-year: [`column_density_face_on`] at each pixel's centre face-on and
/// [`column_density_edge_on`] over each pixel's height edge-on, bit for bit.
///
/// `out` is cleared first and ends with `rows.len() × spec.width_px()` values, so a pool can render
/// the bands of one `spec` into buffers of their own and assemble them in order: a pixel's value
/// depends on the pixel alone, never on the band it was rendered in or on how many bands there are
/// (plan 04, P04.T11).
///
/// # Panics
///
/// If `rows` reaches past `spec.height_px()`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fields::Fields;
/// use hyperion_sim::galaxy::map::{MapSelection, MapSpec, MapView, render_rows};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
///
/// let params = GalaxyParams::milky_way_like();
/// let fields = Fields::new(&params, &MassModel::new(&params));
/// let spec = MapSpec::new(
///     MapView::EdgeOn,
///     MapSelection::AllSystems,
///     [32, 16],
///     [0.0, 0.0],
///     4_096.0,
/// )?;
/// let mut rows = Vec::new();
/// render_rows(&fields, &spec, 7..9, &mut rows);
/// assert_eq!(rows.len(), 2 * 32);
/// // The two rows either side of the plane hold the same columns: the galaxy is symmetric in z.
/// for (near, far) in rows[..32].iter().zip(&rows[32..]) {
///     assert!((near / far - 1.0).abs() < 1e-12);
/// }
/// # Ok::<(), hyperion_sim::galaxy::map::BuildMapSpecError>(())
/// ```
pub fn render_rows(fields: &Fields, spec: &MapSpec, rows: Range<u32>, out: &mut Vec<f64>) {
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
    match spec.view() {
        MapView::FaceOn => {
            for (row, line) in rows.clone().zip(out.chunks_mut(width)) {
                let y = spec.up(f64::from(row));
                for (column, pixel) in line.iter_mut().enumerate() {
                    let column = u32::try_from(column).expect("a column of the map's width");
                    let x = spec.across(f64::from(column) + 0.5);
                    *pixel = column_density_face_on(fields, x, y, spec.selection());
                }
            }
        }
        MapView::EdgeOn => render_edge_on_rows(fields, spec, rows, out),
    }
}

/// [`render_rows`] for the edge-on view, which computes each disc's and the bar's line of sight
/// once per column of the band instead of once per pixel.
///
/// A separable component contributes the integral along `+y`, which depends on `x` alone, times the
/// mean of its vertical profile over the pixel, which depends on the row alone. Reusing the first
/// across a band's rows changes no value: it is the same number, computed once.
fn render_edge_on_rows(fields: &Fields, spec: &MapSpec, rows: Range<u32>, out: &mut [f64]) {
    let plan = EdgeOnPlan::new(fields, spec.selection());
    let width = usize::try_from(spec.width_px()).expect("a map's width fits the address space");
    let heights: Vec<[f64; 2]> = rows.clone().map(|row| spec.pixel_span(row)).collect();
    let mut means = vec![[0.0; MAX_PARTS]; heights.len()];
    for (row, &[z_lo, z_hi]) in heights.iter().enumerate() {
        plan.vertical_means(z_lo, z_hi, &mut means[row]);
    }
    let wanted = wanted_parts(&means);
    let mut lines = [0.0; MAX_PARTS];
    for column in 0..spec.width_px() {
        let x = spec.across(f64::from(column) + 0.5);
        plan.line_integrals(x, &wanted, &mut lines);
        let column = usize::try_from(column).expect("a column of the map's width");
        for (row, &[z_lo, z_hi]) in heights.iter().enumerate() {
            out[row * width + column] = plan.pixel(x, z_lo, z_hi, &means[row], &lines);
        }
    }
}

/// Whether a selection counts a component's systems.
fn counted(component: &Component, selection: MapSelection) -> bool {
    match selection {
        MapSelection::AllSystems => true,
        MapSelection::YoungOnly => component.population() == Population::YoungThinDisc,
    }
}

/// `∫ₐᵇ f` by [`gl32`], or exactly 0 for an empty panel, which a split at a kink can leave.
fn gl32_panel(f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
    if b > a { gl32(f, a, b) } else { 0.0 }
}

/// The bulge's face-on column, `2 ∫₀^∞ n0 exp(−m) dz`, by [`gl32`] in `u` with
/// `z = s u ÷ (1 − u)`, `s = c (1 + P)`.
///
/// The substitution maps the half-line onto `[0, 1)`. Its scale is the bulge's vertical scale
/// length times `1 + P`, `P` the dimensionless in-plane radius, because the density is level in `z`
/// out to `z ≈ c P` and falls over a width that grows with `P`: a fixed scale would push the whole
/// fall-off into the last few nodes far from the centre. The transformed integrand falls off faster
/// than any power towards `u = 1`, where the rule has no node, so the tail costs nothing. Its worst
/// relative error against a brute-force integral over the map's 200 points is 2.8 × 10⁻⁸ (unit
/// tests; the figure is the brute force's own limit, since an independent knot-exact quadrature puts
/// it under 10⁻⁹), against the plan's 10⁻³.
fn bulge_face_on(bulge: &BoxyBulge, x: f64, y: f64) -> f64 {
    let scale = bulge.scale_z().value() * (1.0 + bulge.radius(x, y, 0.0));
    2.0 * gl32(
        |u| {
            let rest = 1.0 - u;
            bulge.envelope(x, y, scale * u / rest) * scale / (rest * rest)
        },
        0.0,
        1.0,
    )
}

/// A halo component's face-on column at the in-plane radius squared `r_sq` (ly²),
/// `2 ∫₀^Z n0 f(m) dz` with `Z` the chord inside the cut, by [`gl32`] in `u` with
/// `z = s u ÷ (1 − c u)`.
///
/// The chord is `√(r_c² − R²)`, since the cut is a sphere. The substitution's scale `s` is
/// `q √(a² + R²)`, the height at which the cored power law has fallen by `2^(−γ ÷ 2)` on this line
/// of sight, and `c = 1 − s ÷ Z` carries `u = 1` to the chord's end: the transformed integrand then
/// runs from `s` at `u = 0` to `s (s ÷ Z)^(γ − 2)`-ish at `u = 1` without a peak between, which the
/// rule resolves to the last bits. The dominant merger's break, a kink in the integrand, is a panel
/// edge: with it the quadrature sits 5 × 10⁻¹⁶ from a brute force of the broken profile, without it
/// 1.6 × 10⁻⁴. The worst relative error over the map's 200 points is 3.4 × 10⁻⁵ (unit tests), which
/// is the 4,000-step brute force's own limit at the kink and not the map's: an independent
/// quadrature with the break and the cut as panel edges puts the columns under 10⁻⁹.
fn halo_face_on(halo: &HaloProfile, r_sq: f64) -> f64 {
    let cut = halo.cut_radius().value();
    let chord_sq = cut * cut - r_sq;
    if chord_sq <= 0.0 {
        return 0.0;
    }
    let chord = chord_sq.sqrt();
    let scale = halo.flattening() * (halo.core().value() * halo.core().value() + r_sq).sqrt();
    let bend = 1.0 - scale / chord;
    let integrand = |u: f64| {
        let rest = 1.0 - bend * u;
        halo.envelope(r_sq, scale * u / rest) * scale / (rest * rest)
    };
    // The break is at m = r_b, which this line of sight crosses at z = q √(r_b² − R²).
    let kink = match halo.break_radius() {
        Some(radius) => {
            let above = radius.value() * radius.value() - r_sq;
            if above > 0.0 {
                let z = halo.flattening() * above.sqrt();
                (z / (scale + bend * z)).min(1.0)
            } else {
                1.0
            }
        }
        None => 1.0,
    };
    2.0 * (gl32_panel(integrand, 0.0, kink) + gl32_panel(integrand, kink, 1.0))
}

/// `∫_{z_lo}^{z_hi} g(|z|) dz` for a profile whose integral from the plane, `∫₀^{|z|} g`, is
/// `from_plane`.
///
/// On one side of the plane it is a difference of two of those integrals, clamped at 0: the
/// difference is not monotone in the last bit, so it can come out a unit in the last place below
/// zero (plan 02, Risks, R18). Across the plane it is their sum, since `from_plane` reads `|z|`.
///
/// The clamp is written out rather than taken as `max`, which may return either of two inputs that
/// compare equal: `−0.0` clamped by `f64::max(0.0)` is `−0.0` or `+0.0` as the target pleases.
fn across_pixel(from_plane: impl Fn(f64) -> f64, z_lo: f64, z_hi: f64) -> f64 {
    let clamped = |difference: f64| if difference > 0.0 { difference } else { 0.0 };
    if z_lo >= 0.0 {
        clamped(from_plane(z_hi) - from_plane(z_lo))
    } else if z_hi <= 0.0 {
        clamped(from_plane(z_lo) - from_plane(z_hi))
    } else {
        from_plane(z_lo) + from_plane(z_hi)
    }
}

/// `∫₀^{|z|} exp(−z′ ÷ h) dz′ = h (1 − exp(−|z| ÷ h))`, the bar's vertical profile from the plane.
fn bar_from_plane(height: f64, z: f64) -> f64 {
    -height * math::exp_m1(-z.abs() / height)
}

/// Whether an integrand along a line of sight is even in `y`, which halves its panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Symmetry {
    /// `f(−y) = f(y)`: every component but a disc with arms.
    EvenInY,
    /// Neither even nor odd: a disc with arms, whose arms wind one way.
    Any,
}

/// No kink: 0 lies on the first panel's edge, never inside a panel.
const NO_KINK: f64 = 0.0;

/// The panels of one side of a line of sight, ascending: `edge(0)` is 0 and `edge(count())` the
/// reach, the far end of the line.
///
/// The scheme is a constant of the generator version (plan 02, P02.T10.b).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Panels {
    /// One panel from 0 to [`INNER_PANEL`] and [`LOG_PANELS`] log-spaced panels beyond it: what
    /// every component without arms takes, whose density along a line of sight falls smoothly over
    /// decades.
    Logarithmic {
        /// The far end of the line, ly.
        reach: f64,
        /// `ln` of the ratio between consecutive edges above [`INNER_PANEL`].
        ln_ratio: f64,
    },
    /// Equal panels no wider than [`ARM_PANEL`]: what a disc with arms takes, so that no panel
    /// spans an arm unresolved.
    Uniform {
        /// The far end of the line, ly.
        reach: f64,
        /// The width of every panel but the last, ly.
        width: f64,
        /// How many panels, at least 1.
        count: u32,
    },
}

impl Panels {
    /// [`LOG_PANELS`] panels log-spaced from [`INNER_PANEL`] to `reach`, after one from 0.
    fn logarithmic(reach: f64) -> Self {
        let ln_ratio = if reach > INNER_PANEL {
            math::ln(reach / INNER_PANEL) / f64::from(LOG_PANELS)
        } else {
            0.0
        };
        Self::Logarithmic { reach, ln_ratio }
    }

    /// Panels of at most [`ARM_PANEL`] out to `reach`, all of one width but the last.
    fn uniform(reach: f64) -> Self {
        let wanted = (reach / ARM_PANEL).ceil().max(1.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a reach of at most 65,536 ly in panels of 512 gives at most 128"
        )]
        let count = wanted as u32;
        Self::Uniform {
            reach,
            width: reach / wanted,
            count,
        }
    }

    /// How many panels the side holds.
    fn count(self) -> u32 {
        match self {
            Self::Logarithmic { reach, .. } => {
                if reach > INNER_PANEL {
                    LOG_PANELS + 1
                } else {
                    1
                }
            }
            Self::Uniform { count, .. } => count,
        }
    }

    /// Edge `k`, from `edge(0) = 0` to `edge(count()) = reach`, ly.
    fn edge(self, k: u32) -> f64 {
        match self {
            Self::Logarithmic { reach, ln_ratio } => {
                if k == 0 {
                    0.0
                } else if k >= self.count() {
                    reach
                } else {
                    INNER_PANEL * math::exp(f64::from(k - 1) * ln_ratio)
                }
            }
            Self::Uniform {
                reach,
                width,
                count,
            } => {
                if k >= count {
                    reach
                } else {
                    f64::from(k) * width
                }
            }
        }
    }
}

/// `∫ f(y) dy` along a whole line of sight: [`gl16`] on every panel above 0, and on their mirror
/// below it unless `f` is even, summed panel by panel.
///
/// A positive `kink` inside a panel splits it, so that a kink in the integrand never falls inside
/// one.
fn along_line(panels: Panels, symmetry: Symmetry, kink: f64, mut f: impl FnMut(f64) -> f64) -> f64 {
    let mut plus = 0.0;
    for k in 0..panels.count() {
        let (lo, hi) = (panels.edge(k), panels.edge(k + 1));
        plus += if lo < kink && kink < hi {
            gl16(&mut f, lo, kink) + gl16(&mut f, kink, hi)
        } else {
            gl16(&mut f, lo, hi)
        };
    }
    match symmetry {
        Symmetry::EvenInY => 2.0 * plus,
        Symmetry::Any => {
            let mut minus = 0.0;
            for k in 0..panels.count() {
                let (lo, hi) = (panels.edge(k), panels.edge(k + 1));
                minus += if lo < kink && kink < hi {
                    gl16(&mut f, -hi, -kink) + gl16(&mut f, -kink, -lo)
                } else {
                    gl16(&mut f, -hi, -lo)
                };
            }
            plus + minus
        }
    }
}

/// The most components an edge-on plan holds, the buffers' size: [`MAX_COMPONENTS`], since every
/// component has one part and at most one line of sight of its own.
const MAX_PARTS: usize = MAX_COMPONENTS;

/// The integrand a component integrates along `+y`, at `z = 0` for the separable ones.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LineKind {
    /// `exp(−R ÷ L)` times an arm factor: every disc of this scale length and arm shares it, and
    /// each multiplies the integral by its own `n0`.
    Disc {
        /// `1 ÷ L`, ly⁻¹.
        inv_length: f64,
        /// The disc's arm, if it has one.
        arm: Option<Arm>,
    },
    /// The bar's `n0 L(|x|) exp(−y² ÷ 2σ_y²)`.
    Bar(LongBar),
}

/// One line of sight: what to integrate and over which panels.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Line {
    kind: LineKind,
    panels: Panels,
    symmetry: Symmetry,
}

impl Line {
    /// The line of sight of a disc: `exp(−R ÷ L)` times its arm factor, without `n0`.
    fn of_disc(disc: &ExponentialDisc) -> Self {
        let arm = disc.arm().copied();
        let (panels, symmetry) = match arm {
            // The panels reach the cube's edge, not the eight scale lengths of plan 02's P02.T10.b,
            // so that no disc's column is cut short; 512 ly is what resolves the arms.
            Some(_) => (Panels::uniform(ROOT_HALF), Symmetry::Any),
            None => (Panels::logarithmic(ROOT_HALF), Symmetry::EvenInY),
        };
        Self {
            kind: LineKind::Disc {
                inv_length: 1.0 / disc.length().value(),
                arm,
            },
            panels,
            symmetry,
        }
    }

    /// The line of sight of the bar.
    fn of_bar(bar: &LongBar) -> Self {
        Self {
            kind: LineKind::Bar(*bar),
            panels: Panels::logarithmic(ROOT_HALF),
            symmetry: Symmetry::EvenInY,
        }
    }

    /// `∫ f(y) dy` at `x` (ly).
    fn integral(&self, x: f64) -> f64 {
        match self.kind {
            LineKind::Disc { inv_length, arm } => {
                along_line(self.panels, self.symmetry, NO_KINK, |y| {
                    let r = (x * x + y * y).sqrt();
                    let envelope = math::exp(-(r * inv_length));
                    match arm {
                        Some(arm) => envelope * arm.factor(&arm.geometry().point(x, y)),
                        None => envelope,
                    }
                })
            }
            LineKind::Bar(bar) => {
                let abs_x = x.abs();
                along_line(self.panels, self.symmetry, NO_KINK, |y| {
                    bar.envelope(abs_x, y, 0.0)
                })
            }
        }
    }
}

/// A component whose density does not separate into a function of `z` times one of `(x, y)`: its
/// pixel takes a two-dimensional quadrature.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Spheroid<'a> {
    /// The boxy bulge.
    Bulge(&'a BoxyBulge),
    /// A halo component, whose line of sight ends at the chord inside its cut.
    Halo(&'a HaloProfile),
}

impl Spheroid<'_> {
    /// `∫ n dy` at `x` and the height `z` (ly).
    fn along(self, x: f64, z: f64) -> f64 {
        match self {
            Self::Bulge(bulge) => along_line(
                Panels::logarithmic(ROOT_HALF),
                Symmetry::EvenInY,
                NO_KINK,
                |y| bulge.envelope(x, y, z),
            ),
            Self::Halo(halo) => {
                let cut = halo.cut_radius().value();
                let chord_sq = cut * cut - x * x - z * z;
                if chord_sq <= 0.0 {
                    return 0.0;
                }
                // The break, where m² = x² + y² + z² ÷ q² reaches r_b², is a kink: a panel edge.
                let kink = halo.break_radius().map_or(NO_KINK, |radius| {
                    let across = radius.value() * radius.value() - halo.radius_sq(x * x, z);
                    if across > 0.0 { across.sqrt() } else { NO_KINK }
                });
                along_line(
                    Panels::logarithmic(chord_sq.sqrt()),
                    Symmetry::EvenInY,
                    kink,
                    |y| halo.envelope(x * x + y * y, z),
                )
            }
        }
    }

    /// The vertical scale over which this component's column falls off nearest the plane, ly: the
    /// bulge's `c`, and a halo component's core times its flattening, which is the scale of
    /// `(1 + m² ÷ a²)^(−γ÷2)` in `z` on the axis.
    ///
    /// It is the smallest such scale over the picture, since both columns flatten in `z` as `x`
    /// grows, so a panel of [`PANEL_SCALES`] of it is never too wide anywhere along the row.
    fn vertical_scale(self) -> f64 {
        match self {
            Self::Bulge(bulge) => bulge.scale_z().value(),
            Self::Halo(halo) => halo.flattening() * halo.core().value(),
        }
    }

    /// The mean of `∫ n dy` over the pixel's height, `∫∫ n dy dz ÷ (z_hi − z_lo)`, by [`gl4`] on
    /// panels of `z`, split at the plane where the pixel straddles it, since `n` reads `|z|`.
    ///
    /// Four nodes are what plan 02's P02.T10.b asks for, and they hold the plan's 3 × 10⁻³ only
    /// while a panel is no more than some eight scale heights tall: alone across a pixel they
    /// under-read a column by 1.9 × 10⁻³ at 4,096 ly and 8 × 10⁻³ at 8,192 ly, the height of a
    /// 16 × 16 raster of the whole cube, which `galaxy_map.golden` pins. A stated accuracy has to
    /// hold over the range the code is used over, and a golden pinning a raster outside its own
    /// tolerance makes the tolerance untestable, so the height is panelled (plan 02, ruling 17 of
    /// 2026-09-22): panels of at most [`PANEL_SCALES`] vertical scales of the component, at most
    /// [`MAX_PIXEL_PANELS`] of them.
    ///
    /// Every raster plan 04 renders (128 to 1,024 ly per pixel, against vertical scales of some 700
    /// to 4,000 ly) takes one panel, and one panel is `gl4` across the whole pixel bit for bit, so
    /// the rule costs those rasters nothing.
    fn pixel(self, x: f64, z_lo: f64, z_hi: f64) -> f64 {
        let integral = if z_lo < 0.0 && z_hi > 0.0 {
            self.panelled(x, z_lo, 0.0) + self.panelled(x, 0.0, z_hi)
        } else {
            self.panelled(x, z_lo, z_hi)
        };
        integral / (z_hi - z_lo)
    }

    /// The height above which the column at `x` is 0, ly: a halo component's cut sphere reaches
    /// `√(cut² − x²)`, and the bulge has no cut.
    fn reach_in_z(self, x: f64) -> f64 {
        match self {
            Self::Bulge(_) => f64::INFINITY,
            Self::Halo(halo) => {
                let cut = halo.cut_radius().value();
                let across = cut * cut - x * x;
                if across > 0.0 { across.sqrt() } else { 0.0 }
            }
        }
    }

    /// `∫ (∫ n dy) dz` from `lo` to `hi > lo`, neither crossing the plane, by [`gl4`] on equal
    /// panels of at most [`PANEL_SCALES`] vertical scales.
    ///
    /// A halo component's column falls to 0 at the height where its cut sphere ends, with a
    /// square-root cusp there, so the interval is first clipped to that height: otherwise a pixel
    /// the cut crosses reads it only if some node happens to fall below the cut, as a single panel
    /// across the top row of a raster of the whole cube did not (plan 02, Risks, R22). A pixel
    /// wholly inside the cut is unchanged by the clip, bit for bit.
    ///
    /// The panel count is the fewest whose width meets that, found by counting up rather than by
    /// rounding a quotient, so that it is one fixed comparison chain and not a cast; the last
    /// panel's far edge is the interval's own, so no panel reaches past the pixel.
    fn panelled(self, x: f64, lo: f64, hi: f64) -> f64 {
        let reach = self.reach_in_z(x);
        let (lo, hi) = (lo.max(-reach), hi.min(reach));
        if hi <= lo {
            return 0.0;
        }
        let widest = PANEL_SCALES * self.vertical_scale();
        let span = hi - lo;
        let mut count = 1_u32;
        while count < MAX_PIXEL_PANELS && f64::from(count) * widest < span {
            count += 1;
        }
        if count == 1 {
            return gl4(|z| self.along(x, z), lo, hi);
        }
        let width = span / f64::from(count);
        (0..count).fold(0.0, |sum, k| {
            let a = lo + f64::from(k) * width;
            let b = if k + 1 == count {
                hi
            } else {
                lo + f64::from(k + 1) * width
            };
            sum + gl4(|z| self.along(x, z), a, b)
        })
    }
}

/// The most vertical scales of a spheroid ([`Spheroid::vertical_scale`]) that one panel of a
/// pixel's height spans.
///
/// Four, half the eight scale heights [`gl4`] is demonstrably enough for, because at eight the rule
/// is already within a factor of two of the plan's 3 × 10⁻³, and every raster plan 04 renders still
/// takes a single panel. Measured on the plane for the Milky Way fixture against Simpson's rule on
/// 4,000 steps in `z` (`tests::the_spheroids_height_integral_holds_its_tolerance_against_simpsons_rule`):
/// 1.3 × 10⁻⁵ at 1,024 ly, 3.6 × 10⁻⁴ at 4,096, 1.5 × 10⁻⁴ at 8,192 and 1.2 × 10⁻³ from 16,384 ly
/// to the cube's 65,536, where one `gl4` read 4.2 × 10⁻⁵, 1.9 × 10⁻³ and 8.0 × 10⁻³ at the first
/// three (plan 02, Risks, R21 and R22).
const PANEL_SCALES: f64 = 4.0;

/// The most panels one side of a pixel's height is split into.
///
/// A bound, not a working limit: the smallest vertical scale the drawn ranges allow is 510 ly (a
/// bulge of `c ÷ a` 0.3 on the 1,700 ly clamp), so the tallest pixel the root cube holds, 131,072
/// ly, wants 65 panels. Past the bound the panels widen, which costs accuracy rather than time.
const MAX_PIXEL_PANELS: u32 = 256;

/// What one component contributes to an edge-on pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Part<'a> {
    /// A disc: `n0` times its line of sight's integral times the mean of its vertical profile over
    /// the pixel.
    Disc {
        /// Which line of sight, an index into the plan's lines.
        line: usize,
        /// The disc's central density, systems per ly³.
        n0: f64,
        /// Its vertical profile, whose integral across the pixel is exact for its table.
        profile: &'a VerticalProfile,
        /// The profile's integral across the pixel below which the disc is left out:
        /// [`DISC_FLOOR`] times its whole column.
        floor: f64,
    },
    /// The bar: its line of sight's integral times the mean of `exp(−|z| ÷ h)` over the pixel.
    Bar {
        /// Which line of sight, an index into the plan's lines.
        line: usize,
        /// The bar's scale height, ly.
        height: f64,
    },
    /// The bulge or a halo component, whose pixel takes a two-dimensional quadrature.
    Spheroid(Spheroid<'a>),
    /// A component the selection does not count.
    Dropped,
}

/// One edge-on map's plan: what every component contributes, in component order, and the lines of
/// sight the discs and the bar integrate along.
///
/// Building it costs a handful of divisions and no quadrature, and it holds no state that depends
/// on which pixels have been rendered: it is a pure function of the fields and the selection.
#[derive(Debug)]
struct EdgeOnPlan<'a> {
    parts: Vec<Part<'a>>,
    lines: Vec<Line>,
}

impl<'a> EdgeOnPlan<'a> {
    /// The plan of `fields` under `selection`.
    fn new(fields: &'a Fields, selection: MapSelection) -> Self {
        let mut plan = Self {
            parts: Vec::with_capacity(fields.components().len()),
            lines: Vec::with_capacity(fields.components().len()),
        };
        for component in fields.components() {
            let part = if counted(component, selection) {
                match component.shape() {
                    Shape::Disc(disc) => Part::Disc {
                        line: plan.line(Line::of_disc(disc)),
                        n0: disc.n0(),
                        profile: disc.profile(),
                        floor: DISC_FLOOR * 2.0 * disc.height().value(),
                    },
                    Shape::Bar(bar) => Part::Bar {
                        line: plan.line(Line::of_bar(bar)),
                        height: bar.height().value(),
                    },
                    Shape::Bulge(bulge) => Part::Spheroid(Spheroid::Bulge(bulge)),
                    Shape::Halo(halo) => Part::Spheroid(Spheroid::Halo(halo)),
                }
            } else {
                Part::Dropped
            };
            plan.parts.push(part);
        }
        plan
    }

    /// The index of `line`, adding it if no earlier component integrates the same integrand over
    /// the same panels. The five sub-discs share one scale length and one arm, so they share a
    /// line.
    fn line(&mut self, line: Line) -> usize {
        if let Some(at) = self.lines.iter().position(|held| *held == line) {
            return at;
        }
        self.lines.push(line);
        self.lines.len() - 1
    }

    /// The mean of each separable component's vertical profile over the pixel's height, in
    /// component order, and 0 for the components that have none or are left out.
    fn vertical_means(&self, z_lo: f64, z_hi: f64, out: &mut [f64; MAX_PARTS]) {
        let depth = z_hi - z_lo;
        for (slot, part) in out.iter_mut().zip(&self.parts) {
            *slot = match part {
                Part::Disc { profile, floor, .. } => {
                    let column = across_pixel(|z| profile.integral_to(z).value(), z_lo, z_hi);
                    if column < *floor { 0.0 } else { column / depth }
                }
                Part::Bar { height, .. } => {
                    across_pixel(|z| bar_from_plane(*height, z), z_lo, z_hi) / depth
                }
                Part::Spheroid(_) | Part::Dropped => 0.0,
            };
        }
        for slot in out.iter_mut().skip(self.parts.len()) {
            *slot = 0.0;
        }
    }

    /// Each line of sight's integral at `x` (ly), for the lines a part `wanted` marks reads, and 0
    /// for the rest, whose parts contribute 0 whatever their line gives.
    ///
    /// `wanted` covers a band's rows together, so which lines are computed depends on the band;
    /// zeroing the others keeps the buffer from ever carrying a value belonging to another column,
    /// and the parts that read them are multiplied by a mean of 0 in any case.
    fn line_integrals(&self, x: f64, wanted: &[bool; MAX_PARTS], out: &mut [f64; MAX_PARTS]) {
        let mut needed = [false; MAX_PARTS];
        for (part, &asked) in self.parts.iter().zip(wanted.iter()) {
            if !asked {
                continue;
            }
            match part {
                Part::Disc { line, .. } | Part::Bar { line, .. } => needed[*line] = true,
                Part::Spheroid(_) | Part::Dropped => {}
            }
        }
        for (at, line) in self.lines.iter().enumerate() {
            out[at] = if needed[at] { line.integral(x) } else { 0.0 };
        }
    }

    /// One pixel's column density, summed in component order, from the vertical means `means` and
    /// the line integrals `lines` of its row and column.
    fn pixel(
        &self,
        x: f64,
        z_lo: f64,
        z_hi: f64,
        means: &[f64; MAX_PARTS],
        lines: &[f64; MAX_PARTS],
    ) -> f64 {
        let mut sum = 0.0;
        for (part, &mean) in self.parts.iter().zip(means.iter()) {
            sum += match part {
                Part::Disc { line, n0, .. } => (n0 * lines[*line]) * mean,
                Part::Bar { line, .. } => lines[*line] * mean,
                Part::Spheroid(spheroid) => spheroid.pixel(x, z_lo, z_hi),
                Part::Dropped => 0.0,
            };
        }
        sum
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::Seed;
    use crate::coords::ROOT_HALF_WIDTH_LY;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::potential::MassModel;

    /// Steps of the brute-force integral every quadrature here is checked against (plan 02,
    /// P02.T10.a).
    const BRUTE_STEPS: u32 = 4_000;

    /// `∫ₐᵇ f` by Simpson's rule on `steps` panels: the brute force, which needs no substitution
    /// and no panel scheme, only many evaluations.
    fn brute_force(f: impl Fn(f64) -> f64, a: f64, b: f64, steps: u32) -> f64 {
        let h = (b - a) / f64::from(steps);
        let mut sum = f(a) + f(b);
        for i in 1..steps {
            let weight = if i % 2 == 0 { 2.0 } else { 4.0 };
            sum += weight * f(a + f64::from(i) * h);
        }
        sum * h / 3.0
    }

    fn fixture() -> Fields {
        let params = GalaxyParams::milky_way_like();
        Fields::new(&params, &MassModel::new(&params))
    }

    /// Points spread over the face-on map: the centre, a lattice out to the cube's edge and random
    /// points, 200 in all.
    fn face_on_points() -> Vec<(f64, f64)> {
        let mut points = vec![(0.0, 0.0)];
        for i in 0..7_u32 {
            for j in 0..7_u32 {
                let at = |n: u32| -64_000.0 + 64_000.0 * f64::from(n) / 3.0;
                points.push((at(i), at(j)));
            }
        }
        let mut lcg = Lcg::new(0x0210_a000_0000_0001);
        while points.len() < 200 {
            let coordinate = |lcg: &mut Lcg| 131_000.0 * (lcg.next_f64() - 0.5);
            let (x, y) = (coordinate(&mut lcg), coordinate(&mut lcg));
            points.push((x, y));
        }
        points
    }

    #[test]
    fn the_root_cubes_half_width_is_the_frames() {
        assert_same_bits(ROOT_HALF, f64::from(ROOT_HALF_WIDTH_LY));
    }

    /// A pixel too small to separate the picture's rows is refused, because a row of no height has
    /// no mean column: `0 ÷ 0` would give a NaN pixel in a release build, where
    /// [`column_density_edge_on`]'s assertion is gone.
    #[test]
    fn a_map_whose_rows_would_collapse_is_refused() {
        let all = MapSelection::AllSystems;
        // 32,000 ly up, one light-year is 2⁻³⁸ of a step, so a pixel of 10⁻¹² ly leaves consecutive
        // row edges on the same `f64`.
        let spec = MapSpec::new(MapView::EdgeOn, all, [4, 256], [0.0, 32_000.0], 1e-12);
        assert!(matches!(
            spec,
            Err(BuildMapSpecError::UnresolvedRows { .. })
        ));
        // The collapse can come from a row anywhere in the picture, below the plane as above it.
        assert!(matches!(
            MapSpec::new(MapView::EdgeOn, all, [4, 256], [0.0, -32_000.0], 1e-12),
            Err(BuildMapSpecError::UnresolvedRows { .. })
        ));
        // At the cube's edge the last place is 1.5 × 10⁻¹¹ ly, so a pixel of 10⁻¹¹ goes too.
        assert!(matches!(
            MapSpec::new(MapView::EdgeOn, all, [4, 256], [0.0, 65_535.0], 1e-11),
            Err(BuildMapSpecError::UnresolvedRows { .. })
        ));
        // A pixel of a thousandth of a light-year at the cube's edge is still resolved, and every
        // row of an accepted spec has a height above 0.
        for (centre, ly_per_px, rows) in [
            ([0.0, 65_535.0], 1e-3, 256_u32),
            ([0.0, 0.0], 1e-9, 64),
            ([0.0, 0.0], 8_192.0, 16),
            ([0.0, 0.0], 8_192.0, 15),
        ] {
            let spec = MapSpec::new(MapView::EdgeOn, all, [4, rows], centre, ly_per_px)
                .expect("a resolved raster");
            for row in 0..rows {
                let [lo, hi] = spec.pixel_span(row);
                assert!(hi > lo, "row {row} of {spec:?} spans {lo} to {hi}");
            }
        }
    }

    /// The band's mask marks a part exactly when some row of the band wants its line of sight, so
    /// that nothing about the band but which lines are computed can differ.
    #[test]
    fn the_bands_mask_is_the_union_of_its_rows() {
        let mut rows = [[0.0; MAX_PARTS]; 3];
        rows[0][2] = 1.5;
        rows[1][2] = 0.0;
        rows[1][5] = f64::MIN_POSITIVE;
        rows[2][0] = 0.25;
        let wanted = wanted_parts(&rows);
        let mut expected = [false; MAX_PARTS];
        expected[0] = true;
        expected[2] = true;
        expected[5] = true;
        assert_eq!(wanted, expected);
        // A band of one row is the mask of that row alone, which is what a single pixel takes.
        assert_eq!(wanted_parts(&rows[1..2]), {
            let mut only = [false; MAX_PARTS];
            only[5] = true;
            only
        });
        assert_eq!(wanted_parts(&[]), [false; MAX_PARTS]);
    }

    /// The height integral holds plan 02's 3 × 10⁻³ at every pixel height the root cube allows,
    /// which is what panelling the height buys (plan 02, ruling 17 of 2026-09-22).
    ///
    /// The bulge and the halo are the only components the height integral is not exact for, and a
    /// pixel resting on the plane holds their peak between the plane and the rule's innermost node,
    /// so on one panel `gl4` under-read a column by 1.9 × 10⁻³ at 4,096 ly and 8 × 10⁻³ at 8,192 ly,
    /// past the tolerance at the height `galaxy_map.golden`'s 16 × 16 raster of the whole cube uses.
    /// The brackets below are the measured errors of the panelled rule; the figures are printed, and
    /// plan 02's Risks record them.
    #[test]
    fn edge_on_columns_hold_their_tolerance_at_every_pixel_height() {
        let fields = fixture();
        // A reference that resolves the height: the same lines of sight, over sixteen panels of the
        // pixel instead of one, which the closed forms leave unchanged and the spheroids do not.
        let refined = |x: f64, z_lo: f64, z_hi: f64| {
            let step = (z_hi - z_lo) / 16.0;
            let plan = EdgeOnPlan::new(&fields, MapSelection::AllSystems);
            let mut total = 0.0;
            for k in 0..16 {
                let (lo, hi) = (z_lo + f64::from(k) * step, z_lo + f64::from(k + 1) * step);
                let mut means = [0.0; MAX_PARTS];
                plan.vertical_means(lo, hi, &mut means);
                let mut lines = [0.0; MAX_PARTS];
                plan.line_integrals(x, &wanted_parts(&[means]), &mut lines);
                total += plan.pixel(x, lo, hi, &means, &lines) * (hi - lo);
            }
            total / (z_hi - z_lo)
        };
        let brackets = [
            (256.0, 1e-6),
            (1_024.0, 1e-4),
            (4_096.0, 3e-4),
            (8_192.0, 3e-4),
            (16_384.0, 1e-3),
            (32_768.0, 1.5e-3),
            (65_536.0, 1.5e-3),
        ];
        for (height, bracket) in brackets {
            let mut worst = 0.0_f64;
            for x in [0.0, 512.0, 2_048.0, 4_096.0, 8_192.0, 26_000.0] {
                let ours =
                    column_density_edge_on(&fields, x, 0.0, height, MapSelection::AllSystems);
                let theirs = refined(x, 0.0, height);
                worst = worst.max((ours / theirs - 1.0).abs());
            }
            println!("edge-on, a pixel {height} ly tall on the plane: {worst:e}");
            assert!(worst < bracket, "{height} ly tall: {worst:e}");
        }
    }

    /// Where a halo component's cut sphere ends inside a pixel's height, the pixel reads the light
    /// below it: the height integral is clipped to the cut, so it no longer depends on whether a node
    /// happens to fall inside (plan 02, Risks, R22). Measured against Simpson's rule on 4,000 steps
    /// up to the cut, for pixels 8,192 ly tall that the cut crosses near their bottom, middle and
    /// top, where the one-panel rule read 0, 0 and a few per cent.
    #[test]
    fn a_pixel_the_halos_cut_crosses_reads_the_light_inside_it() {
        let fields = fixture();
        let x = -36_864.0;
        let mut worst = 0.0_f64;
        for component in fields.components() {
            let Shape::Halo(halo) = component.shape() else {
                continue;
            };
            let spheroid = Spheroid::Halo(halo);
            let reach = spheroid.reach_in_z(x);
            for below in [287.0, 4_096.0, 8_000.0] {
                let (z_lo, z_hi) = (reach - below, reach - below + 8_192.0);
                let ours = spheroid.pixel(x, z_lo, z_hi) * (z_hi - z_lo);
                let theirs = brute_force(|z| spheroid.along(x, z), z_lo, reach, BRUTE_STEPS);
                assert!(
                    ours > 0.0,
                    "{:?}: nothing read {below} ly below the cut",
                    halo.kind()
                );
                worst = worst.max((ours / theirs - 1.0).abs());
            }
        }
        println!("edge-on, a pixel the halo's cut crosses: worst relative error {worst:e}");
        assert!(worst < 3e-3, "{worst:e}");
    }

    /// The spheroids' height integral on the plane against an independent reference, Simpson's rule
    /// on 4,000 steps in `z` over the same lines of sight, which shares no node with [`gl4`]: the
    /// figures ruling 17 of 2026-09-22 asks plan 02 to record (Risks, R22), from 1,024 ly to the
    /// cube's full height. Before the panels, one `gl4` across the pixel read 4.2 × 10⁻⁵ at 1,024
    /// ly, 1.9 × 10⁻³ at 4,096 and 8.0 × 10⁻³ at 8,192 (R21).
    #[test]
    fn the_spheroids_height_integral_holds_its_tolerance_against_simpsons_rule() {
        let fields = fixture();
        let spheroids: Vec<Spheroid<'_>> = fields
            .components()
            .iter()
            .filter_map(|c| match c.shape() {
                Shape::Bulge(bulge) => Some(Spheroid::Bulge(bulge)),
                Shape::Halo(halo) => Some(Spheroid::Halo(halo)),
                Shape::Disc(_) | Shape::Bar(_) => None,
            })
            .collect();
        for height in [1_024.0, 4_096.0, 8_192.0, 16_384.0, 32_768.0, 65_536.0] {
            let mut worst = 0.0_f64;
            for x in [0.0, 2_048.0, 8_192.0, 26_000.0] {
                let (ours, theirs) = spheroids.iter().fold((0.0, 0.0), |(o, r), s| {
                    let reach = s.reach_in_z(x).min(height);
                    (
                        o + s.pixel(x, 0.0, height) * height,
                        r + brute_force(|z| s.along(x, z), 0.0, reach, BRUTE_STEPS),
                    )
                });
                worst = worst.max((ours / theirs - 1.0).abs());
            }
            println!("edge-on spheroids, {height} ly on the plane, against Simpson: {worst:e}");
            assert!(worst < 3e-3, "{height} ly: {worst:e}");
        }
    }

    /// The bulge's face-on column against the brute force over the half-line, which is taken out to
    /// where `exp(−m)` has fallen by `e^−80` from its value on the axis.
    #[test]
    fn the_bulges_face_on_column_matches_a_brute_force_integral() {
        let fields = fixture();
        let Some(Shape::Bulge(bulge)) = fields
            .components()
            .iter()
            .map(Component::shape)
            .find(|shape| matches!(shape, Shape::Bulge(_)))
        else {
            panic!("the fixture has a bulge");
        };
        let mut worst = 0.0_f64;
        for (x, y) in face_on_points() {
            let ours = bulge_face_on(bulge, x, y);
            let far = bulge.scale_z().value() * (bulge.radius(x, y, 0.0) + 80.0);
            let theirs = 2.0 * brute_force(|z| bulge.envelope(x, y, z), 0.0, far, BRUTE_STEPS);
            let error = (ours / theirs - 1.0).abs();
            assert!(error < 1e-3, "({x}, {y}): {ours} against {theirs}");
            worst = worst.max(error);
        }
        println!("bulge face-on: worst relative error {worst:e}");
    }

    /// Every halo component's face-on column against the brute force over the chord inside its cut,
    /// with the break as a panel edge of the brute force too, since Simpson's rule is no better
    /// than the map's quadrature at a kink.
    #[test]
    fn the_halos_face_on_columns_match_a_brute_force_integral() {
        let fields = fixture();
        let halos: Vec<&HaloProfile> = fields
            .components()
            .iter()
            .filter_map(|c| match c.shape() {
                Shape::Halo(halo) => Some(halo),
                Shape::Disc(_) | Shape::Bulge(_) | Shape::Bar(_) => None,
            })
            .collect();
        assert!(
            halos.len() >= 3,
            "the fixture's halo has three or more parts"
        );
        let mut worst = 0.0_f64;
        for (x, y) in face_on_points() {
            let r_sq = x * x + y * y;
            for halo in &halos {
                let ours = halo_face_on(halo, r_sq);
                let cut = halo.cut_radius().value();
                let chord_sq = cut * cut - r_sq;
                if chord_sq <= 0.0 {
                    assert_same_bits(ours, 0.0);
                    continue;
                }
                let chord = chord_sq.sqrt();
                let along = |z| halo.envelope(r_sq, z);
                let split = halo.break_radius().map_or(0.0, |radius| {
                    let above = radius.value() * radius.value() - r_sq;
                    if above > 0.0 {
                        (halo.flattening() * above.sqrt()).min(chord)
                    } else {
                        0.0
                    }
                });
                let theirs = 2.0
                    * (brute_force(along, 0.0, split, BRUTE_STEPS)
                        + brute_force(along, split, chord, BRUTE_STEPS));
                let error = (ours / theirs - 1.0).abs();
                assert!(error < 1e-3, "({x}, {y}): {ours} against {theirs}");
                worst = worst.max(error);
            }
        }
        println!("halo face-on: worst relative error {worst:e}");
    }

    /// The closed forms: every disc's and the bar's face-on column against the exact integral of
    /// its vertical profile, to 10⁻¹², and against the brute force, to 10⁻⁵.
    ///
    /// The exact integral of a disc's profile is the profile's own
    /// [`integral_to`](VerticalProfile::integral_to), which is exact for its table, so the tight
    /// comparison is against that; the brute force cannot do better than about 10⁻⁶, because the
    /// table's 705 knots break the profile's slope and Simpson's rule crosses them (plan 02,
    /// P02.T10.a as built). The bar's exponential is smooth, so its brute force takes four panels
    /// of a thousand steps and reaches 10⁻¹².
    #[test]
    fn the_closed_form_face_on_columns_match_the_exact_vertical_integral() {
        let fields = fixture();
        let (mut worst, mut worst_brute) = (0.0_f64, 0.0_f64);
        // The discs alone, whose `exact` really is an exact integral: for the bar `exact` is its
        // brute force, so the two figures would otherwise be reported as one (plan 02, R20).
        let mut worst_disc = 0.0_f64;
        for (x, y) in face_on_points() {
            let r = (x * x + y * y).sqrt();
            let arm = fields.arms().point(x, y);
            for component in fields.components() {
                // The column as the map takes it, the exact integral of the vertical profile, the
                // brute force, and the tolerance the brute force can hold to.
                let (ours, exact, brute, tolerance) = match component.shape() {
                    Shape::Disc(disc) => {
                        let profile = disc.profile();
                        let factor = disc.arm().map_or(1.0, |arm_of| arm_of.factor(&arm));
                        let plane = disc.envelope(r, 0.0) * factor;
                        let ours = 2.0 * disc.height().value() * disc.envelope(r, 0.0) * factor;
                        // Either side of the plane, and the tail beyond the table's end with it.
                        let exact =
                            profile.integral_to(1e12).value() + profile.integral_to(-1e12).value();
                        // Far enough that the profile has nothing left, in steps of a sixteenth of
                        // the disc's height, so that Simpson's rule resolves its fall.
                        let far = 256.0 * disc.height().value();
                        let inside = brute_force(|z| profile.value(z), 0.0, far, BRUTE_STEPS);
                        (ours, plane * exact, plane * 2.0 * inside, 1e-5)
                    }
                    Shape::Bar(bar) => {
                        let height = bar.height().value();
                        let plane = bar.envelope(x.abs(), y, 0.0);
                        let along = |z| bar.envelope(x.abs(), y, z);
                        let steps = BRUTE_STEPS / 4;
                        let edges = [0.0, 2.0, 6.0, 20.0, 60.0].map(|e: f64| e * height);
                        let brute = 2.0
                            * edges
                                .windows(2)
                                .fold(0.0, |sum, p| sum + brute_force(along, p[0], p[1], steps));
                        // The bar's closed form has nothing to compare against but the brute
                        // force, which its smooth exponential lets reach 10⁻¹².
                        (2.0 * height * plane, brute, brute, 1e-12)
                    }
                    Shape::Bulge(_) | Shape::Halo(_) => continue,
                };
                // Subnormal columns, which the bar has at the cube's far corners, carry too few
                // bits for either comparison.
                if exact < f64::MIN_POSITIVE {
                    continue;
                }
                let error = (ours / exact - 1.0).abs();
                let from_brute = (ours / brute - 1.0).abs();
                assert!(
                    error < 1e-12 && from_brute < tolerance,
                    "{:?} at ({x}, {y}): {ours:e} against {exact:e} exactly and {brute:e} by brute \
                     force",
                    component.population()
                );
                worst = worst.max(error);
                worst_brute = worst_brute.max(from_brute);
                if matches!(component.shape(), Shape::Disc(_)) {
                    worst_disc = worst_disc.max(error);
                }
            }
        }
        println!(
            "closed forms face-on: worst relative error {worst:e} against the exact integral \
             ({worst_disc:e} for the discs, whose integral is exact; the rest is the bar against \
             its own brute force), {worst_brute:e} against the brute force"
        );
    }

    /// Every panel scheme covers its line of sight once, with ascending edges from 0 to the reach.
    #[test]
    fn panel_schemes_cover_their_line_of_sight() {
        let reaches = [1.0, 16.0, 17.0, 512.0, 4_096.0, 50_000.0, ROOT_HALF];
        for reach in reaches {
            for panels in [Panels::logarithmic(reach), Panels::uniform(reach)] {
                assert!(panels.count() >= 1, "{panels:?}");
                assert_same_bits(panels.edge(0), 0.0);
                assert_same_bits(panels.edge(panels.count()), reach);
                for k in 0..panels.count() {
                    assert!(panels.edge(k) < panels.edge(k + 1), "{panels:?} at {k}");
                }
                if let Panels::Uniform { width, .. } = panels {
                    assert!(width <= ARM_PANEL, "{panels:?}");
                }
            }
        }
        // The logarithmic scheme is one panel below 16 ly and eight above it, ratio by ratio.
        let panels = Panels::logarithmic(ROOT_HALF);
        assert_eq!(panels.count(), LOG_PANELS + 1);
        assert_same_bits(panels.edge(1), INNER_PANEL);
        let ratio = panels.edge(3) / panels.edge(2);
        assert!((panels.edge(2) / panels.edge(1) - ratio).abs() < 1e-12);
        assert!((ratio - 2.0 * core::f64::consts::SQRT_2).abs() < 1e-12);
        // The uniform scheme fills the cube with 128 panels of exactly 512 ly.
        assert_eq!(Panels::uniform(ROOT_HALF).count(), 128);
        assert_same_bits(Panels::uniform(ROOT_HALF).edge(1), ARM_PANEL);
    }

    /// A line of sight's integral over a whole line, against the brute force, even and not.
    #[test]
    fn a_line_of_sight_matches_a_brute_force_integral() {
        let even = |y: f64| math::exp(-(y * y) / (2.0 * 4_000.0 * 4_000.0));
        let whole = brute_force(even, -ROOT_HALF, ROOT_HALF, 40_000);
        for panels in [Panels::logarithmic(ROOT_HALF), Panels::uniform(ROOT_HALF)] {
            let ours = along_line(panels, Symmetry::EvenInY, NO_KINK, even);
            assert!((ours / whole - 1.0).abs() < 1e-9, "{panels:?}: {ours}");
            let either = along_line(panels, Symmetry::Any, NO_KINK, even);
            assert!((either / whole - 1.0).abs() < 1e-9, "{panels:?}: {either}");
        }
        // A kink splits the panel that holds it, and the split is exact for a broken line.
        let kinked = |y: f64| if y.abs() < 3_000.0 { 1.0 } else { 0.5 };
        let panels = Panels::logarithmic(ROOT_HALF);
        let split = along_line(panels, Symmetry::EvenInY, 3_000.0, kinked);
        assert!((split / (2.0 * (3_000.0 + 0.5 * (ROOT_HALF - 3_000.0))) - 1.0).abs() < 1e-12);
    }

    /// The integral across a pixel: a difference on one side of the plane, a sum across it, never
    /// below 0.
    #[test]
    fn a_pixel_takes_its_share_of_a_vertical_profile() {
        let height = 500.0;
        let from_plane = |z: f64| bar_from_plane(height, z);
        let whole = 2.0 * height;
        assert!((across_pixel(from_plane, -1e9, 1e9) / whole - 1.0).abs() < 1e-12);
        let above = across_pixel(from_plane, 0.0, 256.0);
        let below = across_pixel(from_plane, -256.0, 0.0);
        assert_same_bits(above, below);
        assert!((across_pixel(from_plane, -256.0, 256.0) - (above + below)).abs() < 1e-12);
        // Far above the plane the difference underflows to nothing, never to a negative.
        assert!(across_pixel(from_plane, 60_000.0, 60_256.0) >= 0.0);
        assert!(across_pixel(|_| 1.0, 5.0, 7.0) >= 0.0);
    }

    /// The five sub-discs share one scale length and one arm, so the plan gives them one line of
    /// sight; the young disc's differs, and the bar's is its own.
    #[test]
    fn an_edge_on_plan_shares_the_sub_discs_line_of_sight() {
        let fields = fixture();
        let plan = EdgeOnPlan::new(&fields, MapSelection::AllSystems);
        assert_eq!(plan.parts.len(), fields.components().len());
        let discs = fields
            .components()
            .iter()
            .filter(|c| matches!(c.shape(), Shape::Disc(_)))
            .count();
        // Young, the five sub-discs, thick and nuclear: eight discs over four lines of sight,
        // plus the bar's.
        assert_eq!(discs, 8);
        assert_eq!(plan.lines.len(), 5);
        // A shared line gives each disc that reads it the same bits as its own line would: the
        // deduplication stands in for the direct computation. The fixture and the three seeds the
        // goldens pin, so that a galaxy whose discs do not share a scale length is covered too.
        let mut wanted = [true; MAX_PARTS];
        let mut lines = [0.0; MAX_PARTS];
        let seeds = [
            0x0000_0000_0000_0001,
            0x5eed_0000_c0ff_ee00,
            0xdead_beef_cafe_f00d,
        ];
        let others: Vec<Fields> = seeds
            .into_iter()
            .map(|s| {
                let params = GalaxyParams::from_seed(Seed::new(s), MassFunctionKind::default());
                Fields::new(&params, &MassModel::new(&params))
            })
            .collect();
        for their_fields in std::iter::once(&fields).chain(&others) {
            let plan = EdgeOnPlan::new(their_fields, MapSelection::AllSystems);
            for x in [0.0, 4_096.0, 26_000.0, -60_000.0] {
                plan.line_integrals(x, &wanted, &mut lines);
                for (part, component) in plan.parts.iter().zip(their_fields.components()) {
                    let Part::Disc { line, .. } = part else {
                        continue;
                    };
                    let Shape::Disc(disc) = component.shape() else {
                        panic!("a disc part holds a disc")
                    };
                    assert_same_bits(lines[*line], Line::of_disc(disc).integral(x));
                }
            }
        }
        wanted[0] = false;
        plan.line_integrals(0.0, &wanted, &mut lines);
        assert_same_bits(lines[0], 0.0);
        let young = MapSelection::YoungOnly;
        let narrow = EdgeOnPlan::new(&fields, young);
        assert_eq!(narrow.lines.len(), 1);
        assert_eq!(
            narrow
                .parts
                .iter()
                .filter(|part| matches!(part, Part::Dropped))
                .count(),
            fields.components().len() - 1
        );
    }
}
