//! Upper bounds on the densities over a cell, which make placement's thinning exact (brainstorm,
//! "Exact placement by thinning"; plan 02, P02.T8).
//!
//! Placement draws a cell's candidates from a Poisson distribution with mean bound × volume and
//! keeps each with probability density ÷ bound (plan 03). If the density exceeds the bound
//! anywhere, the acceptance saturates at 1 and the galaxy is silently short of systems in a
//! cell-shaped patch that no ordinary test would notice. So a bound is never below the density at
//! any point of its cell: a violation is a bug in the galaxy, not a tolerance.
//!
//! # The rule
//!
//! Every density is an envelope that never rises with |x|, |y| or |z|, times, for the discs with
//! arms, a factor that is bounded from the range of one scalar across the cell (brainstorm, "Exact
//! placement by thinning"; [`UnimodalFactor`]). No [`CellBox`] straddles the planes x = 0, y = 0
//! or z = 0, so every point of a cell lies at least as far from each plane as the corner nearest
//! the origin, and an envelope's maximum over the cell is its value there
//! ([`Component::envelope_bound`]). The arm factors are bounded from the cell's range of radius and
//! of arm phase ([`ArmGeometry::phase_range`], [`SharpArm::sup`], [`GentleArm::sup`]), and a
//! disc's bound is the product of the two ([`Component::bound`]).
//!
//! # Floating point
//!
//! The bound is never below the density as computed, not only as written. Three things stand
//! between the two, each covered by a margin that the bound carries:
//!
//! - **The envelopes.** They never rise analytically, and the discs', the bar's and the halo's do
//!   not rise in floating point either, step by step, given a monotone `libm`. A disc's vertical
//!   profile is a table, and between its knots it could rise by a rounding; it does not, because
//!   its exponent is linear between knots that are found exactly and made continuous bit for bit,
//!   so the exponent never falls with height as computed ([`vertical`](super::fields::vertical),
//!   "Floating point", with a test that steps across every knot). The bulge's envelope does rise,
//!   by up to 7 × 10⁻¹⁵ of itself, because its `M (1 + t^c∥)^(1 ÷ c∥)` rounds differently on
//!   either side of the switch between its two terms (plan 02, Risks, R16). Every envelope bound
//!   is therefore the value at the nearest corner times `1 +` [`BOUND_MARGIN`], 2⁻⁴⁰ or 9.1 ×
//!   10⁻¹³: over a hundred times the bulge's excess and far above a unit in the last place of any
//!   `libm` result, and still under the 10⁻¹² to which the bound equals the corner's value.
//! - **The arm phase.** A density computes `cos φ` by double angles from `x ÷ R` and `y ÷ R`
//!   ([`ArmGeometry::point`]), while a cell's range of phase comes from the phase at its centre;
//!   the two differ from the exact cosine by a few 10⁻¹⁴ at most (measured under 10⁻¹⁴ apart; the
//!   phase's own rounding, up to 150 radians in a 4 ly cell 20 ly from the axis, 4 × 10⁻¹⁴). The
//!   greatest `cos φ` over a range is therefore raised by `COS_SLACK`, 2⁻³⁶ or 1.5 × 10⁻¹¹, which
//!   loosens a sharp arm's bound by at most `k` × 2⁻³⁶ ≤ 5 × 10⁻⁸ of itself.
//! - **`I₀ₑ` and the radii.** The sharp arm's `I₀ₑ(k)` falls with `k` except where its sum
//!   changes form: across the switch between series at `k = 15` and wherever the asymptotic
//!   series takes one more term, it rises by up to 1.3 × 10⁻¹⁴ (at `k ≈ 15.004`); the bound
//!   divides by `I₀ₑ(k_max)` times `1 −` [`BOUND_MARGIN`]. A density's `k` and fade read
//!   `R² = x² + y²` and `R = √R²`, a bound's read `R` from the corners, so the radii are widened
//!   by `ROUNDING_SLACK`, 2⁻⁵⁰ relative, which covers the rounding of `R²` from `R`.
//!
//! Where an envelope is subnormal, below 2.2 × 10⁻³⁰⁸, the relative margin shrinks with the
//! precision left, and below about 3 × 10⁻³¹² it rounds away and the bound is the corner's value
//! itself. Inside the root cube only the long bar and the coldest discs get there, where the sum
//! of their exponent's terms passes about 710: the bar far out along x (from about 6.5
//! half-lengths) or across it (from about 38 widths), the young disc far above the plane (from
//! about 12,000–24,000 ly, 80–130 of its effective heights) and, in a few galaxies, the youngest
//! sub-disc near the cube's top. The discs are isothermal above 2.0 kpc and fall there only as the
//! potential rises, so the nuclear disc, whose exponent reaches about 110 at the cube's edge, no
//! longer gets there. All are monotone bit for bit. The bulge, the one envelope that rises by a
//! last bit, stays above 10⁻⁶⁵ of its centre everywhere in the cube (tested), and the halo's
//! components are 0 beyond their cut.
//!
//! With these, each step of the bound's arithmetic takes inputs no smaller than the density's
//! same step, and rounding is monotone, so the bound is not below the density bit for bit, given
//! a monotone `libm`. The envelope's relative margin covers `libm`'s last-bit steps wherever the
//! arm factor is well above 0; each arm factor's bound is also raised by 2⁻⁵⁰ absolute, four
//! units in the last place of 1, which covers them where the factor nears 0, as it can between
//! arms with an arm fraction near 1 and a relative margin cannot. The corners are whole
//! light-years, exact as `f64`s, and a cell's range of radius is computed from them by the
//! densities' own arithmetic, so no point of a cell lies nearer the origin than its nearest
//! corner, or outside its range of radius, even by a unit in the last place.
//!
//! The margins are part of the generated output: they move every candidate count by under one
//! part in 10⁷ (on a sharp arm's flank) and mostly under one in 10¹², which no test can see, and
//! they belong to the generator version like the rest of the bound.
//!
//! # A layer's bound
//!
//! A layer's density is `Σ share × density` over the components in their fixed order
//! ([`Fields::layer_density`]), and its bound is the same sum of `share × bound`
//! ([`Fields::layer_bound`]), in the same order. Each product and each partial sum is a monotone
//! rounding of inputs no smaller than the density's, so once every component's bound holds, the
//! layer's holds bit for bit: the weights plan 03 hands to `Mark::pick_weighted` never sum past
//! the layer's bound. Plan 03 (P03.T4.b) still pads the bound by `1 + 10⁻¹²` before that call and
//! debug-asserts the weighted density against the padded bound. The two do not overlap: the
//! margins here are inside the bound and make it true of the computed densities; plan 03's padding
//! is outside it, so its assertion fires only for a violation of more than 10⁻¹² beyond a bound
//! that already carries them, and the acceptance it changes, by at most 10⁻¹², is its own.

use std::error::Error;
use std::fmt;

use super::PointLy;
use super::fields::arms::Arm;
#[cfg(doc)]
use super::fields::arms::{ArmAcross, ArmGeometry, GentleArm, SharpArm};
use super::fields::{Component, ComponentId, Fields, MAX_COMPONENTS, Site};
use super::imf::MassBand;
use super::shares::ShareMatrix;
use crate::coords::ROOT_HALF_WIDTH_LY;

/// The relative margin every envelope bound carries above the envelope's value at the nearest
/// corner: 2⁻⁴⁰, about 9.1 × 10⁻¹³ (module documentation, "Floating point").
pub const BOUND_MARGIN: f64 = 1.0 / 1_099_511_627_776.0;

/// How far the greatest `cos φ` over a range of arm phase is raised, absolutely: 2⁻³⁶, about
/// 1.5 × 10⁻¹¹ (module documentation, "Floating point").
pub(crate) const COS_SLACK: f64 = 1.0 / 68_719_476_736.0;

/// The relative widening of a range of radius before an arm factor reads it, and the absolute
/// amount an arm factor's bound is raised: 2⁻⁵⁰, about 8.9 × 10⁻¹⁶, four units in the last place
/// of 1 (module documentation, "Floating point").
pub(crate) const ROUNDING_SLACK: f64 = 1.0 / 1_125_899_906_842_624.0;

/// A factor of a density that is bounded over a cell from the range one scalar takes across it
/// (brainstorm, "Exact placement by thinning").
///
/// This is the rule behind every bound: a density is an envelope that never rises with |x|, |y|
/// or |z|, optionally times a factor that is unimodal in one scalar (the arm phase, the radius R
/// or the distance r) and whose supremum over a cell follows from that scalar's range there: its
/// peak if the range reaches the peak, the value at the nearer end otherwise. The bound is the
/// envelope at the nearest corner times that supremum.
///
/// The first implementors are the arms ([`ArmAcross`]): over a band of radii, an arm factor is a
/// function of the phase that peaks on every ridge, once a turn. The intended second is the
/// flared layers of displaced remnants (plan 08): `exp(−(z ÷ h)^β) ÷ h` is unimodal in the scale
/// height `h`, with its peak at `h = z β^(1 ÷ β)`, so a flared layer's bound is its radial
/// envelope at the nearest corner times that peak if it falls inside the cell's range of `h`, and
/// the value at the nearer end otherwise. A peanut bulge would use it the same way.
pub trait UnimodalFactor {
    /// An upper bound on the factor at every point whose scalar lies in `range`: never below the
    /// factor there as computed, bit for bit.
    #[must_use]
    fn sup(&self, range: ScalarRange) -> f64;
}

/// A closed range `[lo, hi]` of one scalar across a cell, such as the cylindrical radius or the arm
/// phase, with `lo ≤ hi`.
///
/// Its ends may be infinite: a range of phase from −∞ to ∞ holds every phase.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScalarRange {
    /// The least value.
    pub lo: f64,
    /// The greatest value.
    pub hi: f64,
}

impl ScalarRange {
    /// The range `[lo, hi]`.
    ///
    /// # Panics
    ///
    /// In debug builds, unless `lo ≤ hi`, which also rules out a NaN end.
    #[must_use]
    pub fn new(lo: f64, hi: f64) -> Self {
        debug_assert!(lo <= hi, "a scalar range needs lo ≤ hi, got [{lo}, {hi}]");
        Self { lo, hi }
    }

    /// Whether `value` lies in the range, ends included.
    #[must_use]
    pub fn contains(&self, value: f64) -> bool {
        self.lo <= value && value <= self.hi
    }
}

/// The names of the axes, for messages.
const AXES: [&str; 3] = ["x", "y", "z"];

/// A [`CellBox`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildCellBoxError {
    /// The edge is not a power of two light-years (0 included).
    EdgeNotPowerOfTwo {
        /// The edge, light-years.
        edge: u32,
    },
    /// The box straddles the plane where the named coordinate is 0: its low corner is negative
    /// and its high corner positive.
    StraddlesPlane {
        /// Which axis, 0 for x, 1 for y, 2 for z.
        axis: usize,
        /// The low corner on that axis, light-years.
        min: i32,
        /// The edge, light-years.
        edge: u32,
    },
    /// The box reaches outside the root cube, `[−65,536, 65,536]` ly, on the named axis.
    OutsideRootCube {
        /// Which axis, 0 for x, 1 for y, 2 for z.
        axis: usize,
        /// The low corner on that axis, light-years.
        min: i32,
        /// The edge, light-years.
        edge: u32,
    },
}

impl fmt::Display for BuildCellBoxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = |axis: usize| AXES.get(axis).copied().unwrap_or("?");
        match self {
            Self::EdgeNotPowerOfTwo { edge } => {
                write!(f, "a cell box's edge of {edge} ly is not a power of two")
            }
            Self::StraddlesPlane { axis, min, edge } => write!(
                f,
                "a cell box from {} = {min} ly with an edge of {edge} ly straddles the plane {} = 0",
                name(*axis),
                name(*axis)
            ),
            Self::OutsideRootCube { axis, min, edge } => write!(
                f,
                "a cell box from {} = {min} ly with an edge of {edge} ly reaches outside the root \
                 cube",
                name(*axis)
            ),
        }
    }
}

impl Error for BuildCellBoxError {}

/// A cube of the galactic frame, with whole-light-year corners and a power-of-two edge, inside the
/// root cube and never straddling an axis plane: the region a bound holds over.
///
/// Its low corner is `min` and its edge `edge` light-years, so it spans `[min, min + edge]` on each
/// axis; a bound over the closed box also holds over any half-open cell inside it. Every grid of
/// the brainstorm has power-of-two cells aligned to their size, 4 ly to 128 ly for the layers and
/// 4,096 ly for the feature catalogue, so no cell of any grid straddles a plane: the planes x, y,
/// z = 0 are cell faces. A power-of-two edge also makes [`in_plane_half_diagonal`] never less than
/// the true `edge ÷ √2` (the rounding of `1 ÷ √2` is upwards, and scaling by a power of two is
/// exact), which the arm's phase range relies on.
///
/// [`in_plane_half_diagonal`]: Self::in_plane_half_diagonal
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::PointLy;
/// use hyperion_sim::galaxy::bounds::{BuildCellBoxError, CellBox};
///
/// // A layer-E cell below the plane, on the far side of the y axis.
/// let cell = CellBox::new([-256, 25_984, -128], 128)?;
/// assert_eq!(cell.nearest_corner(), PointLy::new(-128.0, 25_984.0, 0.0));
/// assert_eq!(cell.farthest_corner(), PointLy::new(-256.0, 26_112.0, -128.0));
/// let radii = cell.r_cyl_range();
/// assert!(radii.lo < 26_000.0 && 26_000.0 < radii.hi);
///
/// // A box across the plane z = 0 is not a cell of any grid.
/// assert!(matches!(
///     CellBox::new([0, 0, -64], 128),
///     Err(BuildCellBoxError::StraddlesPlane { axis: 2, .. })
/// ));
/// # Ok::<(), BuildCellBoxError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellBox {
    min: [i32; 3],
    edge: u32,
}

impl CellBox {
    /// The box with the low corner `min` and the edge `edge`, both in light-years.
    ///
    /// # Errors
    ///
    /// - [`BuildCellBoxError::EdgeNotPowerOfTwo`] unless `edge` is a power of two.
    /// - [`BuildCellBoxError::OutsideRootCube`] unless `−65,536 ≤ min` and `min + edge ≤ 65,536`
    ///   on every axis.
    /// - [`BuildCellBoxError::StraddlesPlane`] if `min < 0 < min + edge` on some axis.
    pub fn new(min: [i32; 3], edge: u32) -> Result<Self, BuildCellBoxError> {
        if !edge.is_power_of_two() {
            return Err(BuildCellBoxError::EdgeNotPowerOfTwo { edge });
        }
        let half = i64::from(ROOT_HALF_WIDTH_LY);
        for (axis, low) in min.into_iter().enumerate() {
            let high = i64::from(low) + i64::from(edge);
            if i64::from(low) < -half || high > half {
                return Err(BuildCellBoxError::OutsideRootCube {
                    axis,
                    min: low,
                    edge,
                });
            }
            if low < 0 && high > 0 {
                return Err(BuildCellBoxError::StraddlesPlane {
                    axis,
                    min: low,
                    edge,
                });
            }
        }
        Ok(Self { min, edge })
    }

    /// The low corner, light-years: the least coordinate on each axis.
    #[must_use]
    pub fn min_corner(&self) -> [i32; 3] {
        self.min
    }

    /// The edge, light-years.
    #[must_use]
    pub fn edge(&self) -> u32 {
        self.edge
    }

    /// The low and high ends of the box on each axis, light-years, exact as `f64`s.
    #[must_use]
    fn ends(&self) -> [(f64, f64); 3] {
        let edge = f64::from(self.edge);
        self.min.map(|low| {
            let low = f64::from(low);
            (low, low + edge)
        })
    }

    /// The corner nearest the origin: on each axis, the end of smaller magnitude.
    ///
    /// Every point of the box lies at least as far from each axis plane as this corner, so an
    /// envelope that never rises with |x|, |y| or |z| is greatest here.
    #[must_use]
    pub fn nearest_corner(&self) -> PointLy {
        let [x, y, z] = self
            .ends()
            .map(|(low, high)| if low >= 0.0 { low } else { high });
        PointLy::new(x, y, z)
    }

    /// The corner farthest from the origin: on each axis, the end of greater magnitude.
    #[must_use]
    pub fn farthest_corner(&self) -> PointLy {
        let [x, y, z] = self
            .ends()
            .map(|(low, high)| if low >= 0.0 { high } else { low });
        PointLy::new(x, y, z)
    }

    /// The range of the cylindrical radius `R = √(x² + y²)` over the box, light-years: from the
    /// nearest corner to the farthest, as the brainstorm states for a box that straddles no plane.
    ///
    /// Each end is `R` as the densities compute it at that corner, so no point of the box has an
    /// `R` outside the range, bit for bit.
    #[must_use]
    pub fn r_cyl_range(&self) -> ScalarRange {
        ScalarRange::new(
            Site::new(&self.nearest_corner()).r,
            Site::new(&self.farthest_corner()).r,
        )
    }

    /// The centre, light-years; exact as `f64`s.
    #[must_use]
    pub fn centre(&self) -> PointLy {
        let half = 0.5 * f64::from(self.edge);
        let [x, y, z] = self.min.map(|low| f64::from(low) + half);
        PointLy::new(x, y, z)
    }

    /// Half the diagonal of the box's square face in the plane, `edge ÷ √2` light-years: how far a
    /// point of the box can lie from the centre's vertical line. Never below the exact value.
    #[must_use]
    pub fn in_plane_half_diagonal(&self) -> f64 {
        f64::from(self.edge) * core::f64::consts::FRAC_1_SQRT_2
    }

    /// Whether `p` lies in the closed box.
    #[must_use]
    pub fn contains(&self, p: &PointLy) -> bool {
        let [(x0, x1), (y0, y1), (z0, z1)] = self.ends();
        (x0..=x1).contains(&p.x) && (y0..=y1).contains(&p.y) && (z0..=z1).contains(&p.z)
    }
}

impl Component {
    /// An upper bound on the component's envelope over `cell`, systems per cubic light-year: its
    /// value at the nearest corner times `1 +` [`BOUND_MARGIN`].
    ///
    /// The envelope is the density without its arm factor ([`Component::envelope`]), which never
    /// rises with |x|, |y| or |z|; for a component without arms this bounds the density itself. A
    /// halo component's envelope is 0 beyond its cut, a sphere, so its bound is 0 over a cell whose
    /// nearest corner lies outside it.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::bounds::CellBox;
    /// use hyperion_sim::galaxy::fields::{Fields, Shape};
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    /// use hyperion_sim::galaxy::potential::MassModel;
    ///
    /// let params = GalaxyParams::milky_way_like();
    /// let fields = Fields::new(&params, &MassModel::new(&params));
    /// let bulge = fields
    ///     .components()
    ///     .iter()
    ///     .find(|c| matches!(c.shape(), Shape::Bulge(_)))
    ///     .expect("every galaxy has a bulge");
    /// // A layer-A cell in the bulge: the bound sits a hair above the nearest corner's density.
    /// let cell = CellBox::new([-808, 400, 96], 8)?;
    /// let corner = bulge.envelope(&cell.nearest_corner());
    /// let bound = bulge.envelope_bound(&cell);
    /// assert!(bound > corner && bound / corner - 1.0 < 1e-12);
    /// assert!(bulge.envelope(&cell.centre()) < bound);
    /// # Ok::<(), hyperion_sim::galaxy::bounds::BuildCellBoxError>(())
    /// ```
    #[must_use]
    pub fn envelope_bound(&self, cell: &CellBox) -> f64 {
        self.envelope(&cell.nearest_corner()) * (1.0 + BOUND_MARGIN)
    }

    /// An upper bound on the component's density over `cell`, systems per cubic light-year: the
    /// [envelope bound](Self::envelope_bound) times, for a disc with arms, the bound of its arm
    /// factor over the cell's ranges of radius and phase (plan 02, P02.T8.b).
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::PointLy;
    /// use hyperion_sim::galaxy::bounds::CellBox;
    /// use hyperion_sim::galaxy::fields::Fields;
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    /// use hyperion_sim::galaxy::potential::MassModel;
    ///
    /// let params = GalaxyParams::milky_way_like();
    /// let fields = Fields::new(&params, &MassModel::new(&params));
    /// let young = &fields.components()[0];
    /// // A layer-E cell that a ridge crosses at 26,000 ly: the corners miss the ridge, the bound
    /// // does not, and it stays within a few per cent of the ridge's density.
    /// let theta = fields.arms().ridge_azimuth(26_000.0, 0);
    /// let (sin, cos) = hyperion_sim::math::sin_cos(theta);
    /// let ridge = PointLy::new(26_000.0 * cos, 26_000.0 * sin, 0.0);
    /// let cell = CellBox::new([-17_024, -19_712, 0], 128)?;
    /// assert!(cell.contains(&ridge));
    /// let on_ridge = young.density(&ridge);
    /// assert!(on_ridge <= young.bound(&cell) && young.bound(&cell) < 1.05 * on_ridge);
    /// # Ok::<(), hyperion_sim::galaxy::bounds::BuildCellBoxError>(())
    /// ```
    #[must_use]
    pub fn bound(&self, cell: &CellBox) -> f64 {
        let envelope = self.envelope_bound(cell);
        match self.arm() {
            Some(arm) => {
                let phase = arm.geometry().phase_range(cell);
                envelope * arm.across(cell.r_cyl_range()).sup(phase)
            }
            None => envelope,
        }
    }
}

impl Fields {
    /// An upper bound on the density of the component `id` over `cell`, systems per cubic
    /// light-year: [`Component::bound`].
    ///
    /// # Panics
    ///
    /// If `id` does not index these fields' components ([`Fields::component`]).
    #[must_use]
    pub fn component_bound(&self, id: ComponentId, cell: &CellBox) -> f64 {
        self.component(id).bound(cell)
    }

    /// Every component's bound over `cell` into `out`, in component order, the same bits as
    /// [`component_bound`](Self::component_bound); entries past the last component are set to 0.
    /// Systems per cubic light-year, before layer shares.
    ///
    /// It reads the nearest corner and the cell's ranges of radius and phase once for every
    /// component, and the arm factor's bound once for the sub-discs, which share one arm.
    pub fn component_bounds(&self, cell: &CellBox, out: &mut [f64; MAX_COMPONENTS]) {
        let corner = Site::new(&cell.nearest_corner());
        let radii = cell.r_cyl_range();
        // Every arm follows the galaxy's geometry, which `Fields::new` asserts.
        let phase = self.arms().phase_range(cell);
        let mut last: Option<(&Arm, f64)> = None;
        let (used, rest) = out.split_at_mut(self.components().len());
        for (slot, component) in used.iter_mut().zip(self.components()) {
            let envelope = component.shape().envelope_at(&corner) * (1.0 + BOUND_MARGIN);
            *slot = match component.arm() {
                Some(arm) => {
                    let sup = match last {
                        Some((previous, sup)) if previous == arm => sup,
                        _ => {
                            let sup = arm.sup(radii, phase);
                            last = Some((arm, sup));
                            sup
                        }
                    };
                    envelope * sup
                }
                None => envelope,
            };
        }
        rest.fill(0.0);
    }

    /// An upper bound on the density of the layer of `band` over `cell`, systems per cubic
    /// light-year: `Σ share × bound` over the components in order, the sum
    /// [`layer_density`](Self::layer_density) takes of the densities (module documentation, "A
    /// layer's bound"). This is the bound placement thins against (plan 03).
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::PointLy;
    /// use hyperion_sim::galaxy::bounds::CellBox;
    /// use hyperion_sim::galaxy::fields::Fields;
    /// use hyperion_sim::galaxy::imf::{BandShares, Kroupa, MassBand};
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    /// use hyperion_sim::galaxy::potential::MassModel;
    /// use hyperion_sim::galaxy::shares::ShareMatrix;
    ///
    /// let params = GalaxyParams::milky_way_like();
    /// let fields = Fields::new(&params, &MassModel::new(&params));
    /// let shares = ShareMatrix::uniform(&BandShares::of(&Kroupa));
    /// // A layer-A cell at the solar circle: the mean number of candidates is bound × 8³.
    /// let cell = CellBox::new([22_512, 12_992, 48], 8)?;
    /// let bound = fields.layer_bound(&shares, MassBand::A, &cell);
    /// let centre = fields.layer_density(&shares, MassBand::A, &cell.centre());
    /// assert!(centre <= bound && bound < 1.2 * centre);
    /// assert!((0.5..3.0).contains(&(bound * 512.0)));
    /// # Ok::<(), hyperion_sim::galaxy::bounds::BuildCellBoxError>(())
    /// ```
    #[must_use]
    pub fn layer_bound(&self, shares: &ShareMatrix, band: MassBand, cell: &CellBox) -> f64 {
        let mut bounds = [0.0; MAX_COMPONENTS];
        self.component_bounds(cell, &mut bounds);
        self.components()
            .iter()
            .zip(bounds)
            .fold(0.0, |sum, (c, bound)| {
                sum + shares.component_share(band, c) * bound
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_touching_a_plane_are_cells_and_boxes_across_one_are_not() {
        for (min, edge) in [
            ([0, 0, 0], 8),
            ([-8, -8, -8], 8),
            ([-8, 0, 8], 8),
            ([64, -128, -64], 64),
        ] {
            assert_eq!(CellBox::new(min, edge).map(|c| c.min_corner()), Ok(min));
        }
        assert_eq!(
            CellBox::new([-4, 0, 0], 8),
            Err(BuildCellBoxError::StraddlesPlane {
                axis: 0,
                min: -4,
                edge: 8
            })
        );
        assert_eq!(
            CellBox::new([0, -1, 0], 2),
            Err(BuildCellBoxError::StraddlesPlane {
                axis: 1,
                min: -1,
                edge: 2
            })
        );
        assert_eq!(
            CellBox::new([0, 0, -2_048], 4_096),
            Err(BuildCellBoxError::StraddlesPlane {
                axis: 2,
                min: -2_048,
                edge: 4_096
            })
        );
    }

    #[test]
    fn edges_are_powers_of_two() {
        for edge in [1, 4, 8, 128, 4_096, 65_536] {
            assert!(CellBox::new([0, 0, 0], edge).is_ok(), "{edge}");
        }
        for edge in [0, 3, 12, 100, 4_095] {
            assert_eq!(
                CellBox::new([0, 0, 0], edge),
                Err(BuildCellBoxError::EdgeNotPowerOfTwo { edge })
            );
        }
    }

    #[test]
    fn boxes_lie_inside_the_root_cube() {
        assert!(CellBox::new([65_536 - 128, -65_536, 0], 128).is_ok());
        assert!(CellBox::new([-65_536, -65_536, -65_536], 65_536).is_ok());
        assert_eq!(
            CellBox::new([0, 65_536 - 64, 0], 128),
            Err(BuildCellBoxError::OutsideRootCube {
                axis: 1,
                min: 65_536 - 64,
                edge: 128
            })
        );
        assert_eq!(
            CellBox::new([0, 0, -65_544], 8),
            Err(BuildCellBoxError::OutsideRootCube {
                axis: 2,
                min: -65_544,
                edge: 8
            })
        );
        assert!(CellBox::new([i32::MAX - 7, 0, 0], 8).is_err());
    }

    #[test]
    fn error_text_is_lowercase_without_trailing_punctuation_and_names_the_plane() {
        let straddles = BuildCellBoxError::StraddlesPlane {
            axis: 1,
            min: -4,
            edge: 8,
        };
        assert!(straddles.to_string().contains("plane y = 0"), "{straddles}");
        for error in [
            straddles,
            BuildCellBoxError::EdgeNotPowerOfTwo { edge: 3 },
            BuildCellBoxError::OutsideRootCube {
                axis: 0,
                min: 70_000,
                edge: 8,
            },
        ] {
            let text = error.to_string();
            assert!(text.starts_with('a') && !text.ends_with('.'), "{text}");
        }
    }

    /// The corners and the centre in every octant, with a box touching each plane.
    #[test]
    fn corners_and_centre_in_every_octant() {
        for sx in [1, -1] {
            for sy in [1, -1] {
                for sz in [1, -1] {
                    let low =
                        |s: i32, magnitude: i32| if s > 0 { magnitude } else { -magnitude - 16 };
                    let cell = CellBox::new([low(sx, 32), low(sy, 0), low(sz, 48)], 16).unwrap();
                    let (fx, fy, fz) = (f64::from(sx), f64::from(sy), f64::from(sz));
                    assert_eq!(
                        cell.nearest_corner(),
                        PointLy::new(32.0 * fx, 0.0, 48.0 * fz)
                    );
                    assert_eq!(
                        cell.farthest_corner(),
                        PointLy::new(48.0 * fx, 16.0 * fy, 64.0 * fz)
                    );
                    assert_eq!(cell.centre(), PointLy::new(40.0 * fx, 8.0 * fy, 56.0 * fz));
                    assert_eq!(
                        cell.r_cyl_range(),
                        ScalarRange::new(32.0, (48.0_f64 * 48.0 + 16.0 * 16.0).sqrt())
                    );
                    for corner in [cell.nearest_corner(), cell.farthest_corner(), cell.centre()] {
                        assert!(cell.contains(&corner));
                    }
                    assert!(!cell.contains(&PointLy::new(0.0, 0.0, 0.0)));
                }
            }
        }
        let cell = CellBox::new([0, 0, 0], 4_096).unwrap();
        assert!(
            (cell.in_plane_half_diagonal() - 2_048.0 * core::f64::consts::SQRT_2).abs() < 1e-12
        );
        assert_eq!(
            cell.r_cyl_range(),
            ScalarRange::new(0.0, 4_096.0 * core::f64::consts::SQRT_2)
        );
        assert!(ScalarRange::new(1.0, 2.0).contains(2.0));
        assert!(!ScalarRange::new(1.0, 2.0).contains(0.5));
    }

    /// Every point of a box lies within the radii the box reports, bit for bit, with `R` as the
    /// densities compute it.
    #[test]
    fn points_of_a_box_lie_within_its_radii() {
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0xce11);
        for _ in 0..10_000 {
            let edge = 1_u32 << (2 + lcg.next_below(11));
            let reach = u64::try_from(ROOT_HALF_WIDTH_LY).unwrap() - u64::from(edge);
            let mut low = || {
                let magnitude = i32::try_from(lcg.next_below(reach)).unwrap();
                if lcg.next_f64() < 0.5 {
                    magnitude
                } else {
                    -magnitude - i32::try_from(edge).unwrap()
                }
            };
            let cell = CellBox::new([low(), low(), low()], edge).unwrap();
            let radii = cell.r_cyl_range();
            let min = cell.min_corner().map(f64::from);
            for _ in 0..8 {
                let [x, y] = [min[0], min[1]].map(|m| m + f64::from(edge) * lcg.next_f64());
                let r = Site::new(&PointLy::new(x, y, 0.0)).r;
                assert!(radii.contains(r), "{r} outside {radii:?} of {cell:?}");
            }
        }
    }
}
