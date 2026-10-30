//! A stream's tube table and the cache trait that hands it out (plan 10, P10.T6 and P10.T1).
//!
//! A tube table is measured, not assumed: about 2,000 tracers sprayed from the progenitor and
//! integrated in the potential ([`spray`]), reduced to about 256 knots per arm
//! (brainstorm, "Streams and accreted structure"). It costs about 0.2 s and 80 kB, so it is a
//! cache and never stored, and the cache is the caller's ("Runtime and code shape"): the sim asks
//! for a table through [`TubeLookup`] and keeps none itself.
//!
//! # The arms
//!
//! A tracer belongs to the arm of its ψ's sign, the angle it gained on the progenitor: the
//! leading arm for ψ ≥ 0 (a ψ of zero, either sign, included) and the trailing arm for ψ < 0
//! (main's ruling of 2026-10-29). A few tracers leave one Lagrange point and end on the other
//! arm's orbits ([`spray`], "The arms"); split by their release point, a knot's
//! kernel would average positions far apart at the arm's root. Split by sign, each falls among the
//! other arm's tracers at its ψ and keeps its weight, so the arms' masses differ from half the
//! tube's by those tracers' weight. Where the tracers have hardly drifted, as for an orphan on an
//! eccentric orbit near its apocentre, ψ's swing through the orbit hides the sign of the drift and
//! the split fails, a finding for the owner (plan 10, "P10.T6.a as built").
//!
//! # The reduction
//!
//! The tube is ordered along by ψ, from the trailing arm's far end through the progenitor to the
//! leading arm's, the order of the along coordinate `u = Λ(ψ) ÷ Λ_total` (plan 10, Design note 7),
//! so each arm's knots run in that order too: the trailing arm's from its tip to its root, the
//! leading arm's from its root to its tip. Within an arm (Design note 6):
//!
//! - **Knots** sit at [`KNOTS_PER_ARM`] equal steps of the arm's cumulative weight, the first and
//!   last at its ends, each at the fractional rank where the cumulative weight, counted to each
//!   tracer's middle, reaches its step. A knot's ψ is the tracers' own at that rank, linear between
//!   the two about it: the inverse of the tracers' Λ(ψ), so the knots keep their order and their
//!   arm's sign.
//! - **Every other knot quantity is a Gaussian-kernel average** over the [`KERNEL_TRACERS`]
//!   tracers nearest the knot's rank, each weighted by its own weight times `exp(−½ (Δ ÷ σ)²)`,
//!   `Δ` its rank's distance from the knot's and `σ` [`KERNEL_WIDTH`] ranks.
//!   - The centre line and the mean velocity are the kernel's local-linear averages: the
//!     intercept at the knot's rank of the line fitted in rank by the kernel's weights, which is
//!     the kernel's mean where its tracers lie evenly about the knot. At an arm's end the kernel
//!     reaches one way only, and its mean would lie inside the end by about a dispersion in rank,
//!     and with it the mean velocity's direction, by about the angle the track turns there; the
//!     fit carries no such bias to first order (Fan and Gijbels 1992, "Variable bandwidth and
//!     local linear regression smoothers", Ann. Statist. 20, 2008–2036: "the estimator does not
//!     have boundary effects"), at the price of more noise there: an end knot's fit rests on an
//!     effective 7 tracers, against the one-sided mean's 22 and an inner knot's 39.
//!   - The two widths (the rms offsets along the frame's normal and binormal) and the three
//!     velocity dispersions (the rms velocity offsets along its tangent, normal and binormal) are
//!     the kernel's means of the squared offsets, each tracer's measured about the centre line and
//!     the mean velocity interpolated at its own rank, so that the track's curvature and its
//!     velocity's gradient along it stay out.
//! - **The cumulative line density** Λ is the weight from the trailing arm's tip, so it rises by
//!   the same mass from knot to knot within an arm.
//!
//! The centre line is kept as a [`GalacticPosition`], integer light-years plus an offset.
//!
//! # The frame
//!
//! The frame is carried along the whole centre line by parallel transport, so that it never
//! flips: each knot's tangent is the direction of its centre line's fitted line, along increasing
//! ψ (on a synthetic stream of 600 tracers an arm the binormal then strays 2.8° from the pole at
//! most, against 9° with the chord through the neighbours, the worst at the arms' ends), and the
//! normal is carried from knot to knot by the double reflection of Wang, Jüttler, Zheng and Liu
//! (2008, "Computation of rotation minimizing frames", ACM Trans. Graph. 27(1), article 2,
//! doi:10.1145/1330511.1330513, §4 and Table I), then made exactly perpendicular to the tangent.
//! The method is fourth order in the knots' spacing for exact tangents (their Theorem 4.5); here
//! the tangents are the kernel fits' noisy directions, and their noise sets the frame's error.
//! The frame starts at the trailing arm's root, the knot nearest the progenitor, with the binormal
//! along the orbit's pole there, the mean angular momentum's direction, and is carried out to
//! both tips. So across a stream that keeps to one plane the normal stays in it, near the
//! galactocentric radius, and the binormal near the pole: the two widths are near the stream's
//! own in-plane and vertical widths.
//! `(tangent, normal, binormal)` is right-handed.
//!
//! # Between knots
//!
//! Every quantity is linear between knots ([`TubeTable::locate`], [`TubeTable::mean_velocity`],
//! [`TubeTable::psi`]), in the along coordinate, the frame's vectors then made unit vectors again,
//! so `u` maps back to ψ and to a point through the inverse of Λ: a uniform draw of `u` is a draw
//! by mass along the tube (Design note 7). Between
//! the two arms' roots Λ does not rise, so no `u` falls there: for an orphan, the gap where its
//! cluster was.
//!
//! # Gaps
//!
//! The along coordinate is cut into [`along_cells`](TubeTable::along_cells) cells of equal mass,
//! the power of two nearest the tube's mass over [`CROSS_SECTION_MEMBERS`] mean systems (Design
//! note 7), and each cell's mean count carries one factor of gap noise, [`TubeTable::noise`]
//! (Design note 8):
//!
//! ```text
//! F(cell) = exp(σ g(ψ) − σ² ÷ 2) ÷ m
//! ```
//!
//! with `σ` [`GAP_NOISE_AMPLITUDE`] and `ψ` the cell's middle's, [`TubeTable::psi`] at the middle's
//! share of the mass. `g` is one-dimensional lattice noise in ψ. Its nodes lie
//! [`CELLS_PER_GAP_NODE`] mean cells apart in ψ, `8 (ψ_max − ψ_min) ÷ cells` from the trailing
//! arm's tip to the leading arm's, so that no wavelength is shorter than eight mean cells. "Eight
//! cells" has to be read in ψ's mean, since the cells are equal in mass, not in ψ: where the
//! tracers lie sparse, at the tips, a cell is wider than the mean and a node spacing covers
//! fewer. Node `k` holds one standard normal, words `2k` and `2k + 1` of the
//! tube's [`tags::STREAM_TUBE_GAPS`] stream, so that any node is read without the ones before it.
//! Between two nodes `g` is the gas noise's in one dimension
//! ([`gas::noise`](crate::galaxy::gas::noise)): their normals weighted by `1 − s` and `s` of the
//! quintic fade and divided by the root of the weights' squares' sum, so that `g` is exactly
//! N(0, 1) at every ψ and `exp(σ g − σ² ÷ 2)` has expectation 1. `m` is that factor's exact mean
//! over the tube's cells, summed in cell order when the table is built, so the cells' factors sum
//! to the cell count and the tube's expected total is unchanged. The table keeps only `m`, and
//! a cell's factor is recomputed from the seed.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity};
use crate::galaxy::{Galaxy, Population};
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{LightYears, MetresPerSecond, Radians, SolarMasses};

use super::orbit::{BuildLeapfrogError, cross, dot};
use super::spec::StreamSpec;
use super::spray::{self, Arm, Tracer};
use super::{GlobalList, StreamNumber};

/// Knots in each arm of a tube table, a parameter of the generator version: 256, the brainstorm's
/// "about 256 knots per arm" ("Streams and accreted structure").
pub const KNOTS_PER_ARM: u32 = 256;

/// The tracers a knot's kernel averages, the nearest in rank, a parameter of the generator
/// version: 48 (plan 10, Design note 6; "four tracers a knot would be noise").
///
/// An arm needs at least as many.
pub const KERNEL_TRACERS: u32 = 48;

/// The kernel's Gaussian dispersion in ranks, a parameter of the generator version: 12, so that
/// the [`KERNEL_TRACERS`] it averages span ±2 dispersions about the knot's rank (a choice of this
/// task, P10.T6.a, which the plan leaves to the generator version).
pub const KERNEL_WIDTH: f64 = 12.0;

/// The farthest across coordinate [`TubeTable::locate`] takes, in widths: 1,000, far beyond a
/// member's ±4 (plan 10, Design note 7), and near enough that a point that far from any knot stays
/// in the light-year cells' range.
pub const ACROSS_LIMIT: f64 = 1_000.0;

/// The members one cross-section of cells expects over all bands, which sets a tube's along
/// cells: 64 (plan 10, Design note 7).
pub const CROSS_SECTION_MEMBERS: f64 = 64.0;

/// The fewest along cells a tube is cut into, a parameter of the generator version: 16, two of
/// the gap noise's node spacings (a lean of P10.T6.b, 2026-10-30; Design note 7 sets no floor).
pub const MIN_ALONG_CELLS: u32 = 16;

/// The most along cells a tube is cut into: 16,384 (plan 10, Design note 7), which the 14 bits of
/// a stream member's ID's along cell hold (plan 01's `StreamMemberId`).
pub const MAX_ALONG_CELLS: u32 = 16_384;

/// The gap noise's node spacing in mean along cells, a parameter of the generator version: 8, so
/// that no wavelength is shorter than eight mean cells in ψ (plan 10, Design note 8).
pub const CELLS_PER_GAP_NODE: u32 = 8;

// The node lattice spans every tube with at least two intervals and ends on the leading tip, which
// `TubeTable::undivided_noise`'s `intervals − 1` needs.
const _: () = assert!(
    MIN_ALONG_CELLS >= 2 * CELLS_PER_GAP_NODE && MIN_ALONG_CELLS.is_multiple_of(CELLS_PER_GAP_NODE),
    "every tube's cells hold two node spacings or more, a whole number of them"
);

/// The gap noise's amplitude, the standard deviation of its factor's natural logarithm, a
/// parameter of the generator version: 0.3, so that the density contrast along a tube, the
/// factor less 1, has an rms of `√(exp(0.09) − 1)`, 0.31, and a factor of 0.5 lies 2.2 standard
/// deviations of the logarithm down (a lean of P10.T6.b, 2026-10-30, not a fit).
///
/// The noise stands for the gaps observed along streams such as GD-1 (Price-Whelan and Bonaca
/// 2018, ApJL 863, L20; de Boer, Erkal and Gieles 2020, MNRAS 494, 5315); the depth and the rms
/// are this lean's, not figures read from those papers.
pub const GAP_NOISE_AMPLITUDE: f64 = 0.3;

/// A tube table could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildTubeTableError {
    /// The spray could not be made ([`spray::spray`]).
    Spray(BuildLeapfrogError),
    /// An arm, by ψ's sign, holds fewer tracers than a knot's kernel averages,
    /// [`KERNEL_TRACERS`].
    TooFewTracers {
        /// The arm.
        arm: Arm,
        /// The tracers it holds.
        tracers: u32,
    },
}

impl fmt::Display for BuildTubeTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spray(_) => f.write_str("the stream's spray could not be made"),
            Self::TooFewTracers { arm, tracers } => write!(
                f,
                "the {} arm holds {tracers} tracers, fewer than a knot's kernel of {KERNEL_TRACERS}",
                match arm {
                    Arm::Leading => "leading",
                    Arm::Trailing => "trailing",
                }
            ),
        }
    }
}

impl Error for BuildTubeTableError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spray(error) => Some(error),
            Self::TooFewTracers { .. } => None,
        }
    }
}

impl From<BuildLeapfrogError> for BuildTubeTableError {
    fn from(error: BuildLeapfrogError) -> Self {
        Self::Spray(error)
    }
}

/// One knot of a tube table (module documentation, "The reduction").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TubeKnot {
    centre: GalacticPosition,
    frame: [[f64; 3]; 3],
    velocity: GalacticVelocity,
    widths: [LightYears; 2],
    dispersions: [MetresPerSecond; 3],
    cumulative_mass: SolarMasses,
    psi: Radians,
}

impl TubeKnot {
    /// The centre line's point at the knot, in the galactic frame.
    #[must_use]
    pub const fn centre(&self) -> GalacticPosition {
        self.centre
    }

    /// The local frame, unit vectors in the galactic frame: the tangent, along the tube in the
    /// direction of increasing ψ, which is the direction of motion; the normal; and the binormal,
    /// tangent × normal (module documentation, "The frame").
    #[must_use]
    pub const fn frame(&self) -> [[f64; 3]; 3] {
        self.frame
    }

    /// The tracers' mean velocity at the knot, in the galactic frame.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The tube's widths, the tracers' rms offsets from the centre line along the frame's normal
    /// and binormal.
    #[must_use]
    pub const fn widths(&self) -> [LightYears; 2] {
        self.widths
    }

    /// The tracers' velocity dispersions about the mean velocity, rms along the frame's tangent,
    /// normal and binormal.
    #[must_use]
    pub const fn dispersions(&self) -> [MetresPerSecond; 3] {
        self.dispersions
    }

    /// Λ, the tube's mass from the trailing arm's tip to the knot.
    #[must_use]
    pub const fn cumulative_mass(&self) -> SolarMasses {
        self.cumulative_mass
    }

    /// ψ at the knot: its tracers' at its rank (module documentation, "The reduction").
    #[must_use]
    pub const fn psi(&self) -> Radians {
        self.psi
    }
}

/// One arm of a tube table: its knots, in the order of increasing ψ (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct TubeArm {
    knots: Vec<TubeKnot>,
    tracers: u32,
    mass: SolarMasses,
}

impl TubeArm {
    /// The knots, [`KNOTS_PER_ARM`] of them, in the order of increasing ψ: from the tip to the
    /// root for the trailing arm, from the root to the tip for the leading arm.
    #[must_use]
    pub fn knots(&self) -> &[TubeKnot] {
        &self.knots
    }

    /// The tracers it was reduced from, those of its ψ's sign.
    #[must_use]
    pub const fn tracers(&self) -> u32 {
        self.tracers
    }

    /// Its mass, its tracers' weights summed.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }
}

/// One stream's tube table (plan 10, P10.T6; module documentation): its two arms' knots.
///
/// # Examples
///
/// A caller's pool sprays the tracers in parallel and gathers them by index (ruling 146.3); here
/// two halves stand for it (not run: the debris needs the full potential, and the spray takes
/// some seconds):
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::global_list::spray::{Spray, SprayForce, SprayParams};
/// use hyperion_sim::galaxy::global_list::{Debris, TubeTable};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::{Galaxy, Population};
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?
///     .with_full_potential();
/// let debris = Debris::generate(&galaxy);
/// let stream = &debris.streams()[0];
/// let (seed, tables) = (galaxy.seed(), galaxy.potential());
/// let force = SprayForce::new(tables)?;
/// let spray = Spray::new(seed, &force, tables, stream, &SprayParams::default())?;
/// let half = spray.tracer_count() / 2;
/// let mut tracers: Vec<_> = (0..half).map(|i| spray.tracer(i)).collect();
/// tracers.extend((half..spray.tracer_count()).map(|i| spray.tracer(i)));
/// let mean_mass = galaxy.mean_system_mass(Population::Halo);
/// let tube = TubeTable::from_tracers(seed, mean_mass, stream, &tracers)?;
/// // The along coordinate is a share of the tube's mass: a uniform draw of it is a draw by mass.
/// let middle = tube.locate(0.5, 0.0, 0.0);
/// let edge = tube.locate(0.5, 1.0, 0.0);
/// assert!(middle.distance_to(&edge).value() > 0.0);
/// // Each along cell expects its share of the tube's mass times its gap noise, which sums to the
/// // cell count, so the tube's expected total is its mass.
/// let cells = tube.along_cells();
/// let noise: f64 = (0..cells).map(|cell| tube.noise(cell)).sum();
/// assert!((noise / f64::from(cells) - 1.0).abs() < 1e-12);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TubeTable {
    stream: StreamNumber,
    /// The trailing arm, then the leading arm: the order of the along coordinate.
    arms: [TubeArm; 2],
    mass: SolarMasses,
    /// The galaxy's seed, which the gap noise's nodes are drawn under.
    seed: Seed,
    along_cells: u32,
    /// The gap noise's mean over the along cells before it is divided out, `m` (module
    /// documentation, "Gaps").
    noise_mean: f64,
}

impl TubeTable {
    /// The tube table of `spec`'s stream in `galaxy`, its tracers sprayed one after another at the
    /// generator's [`SprayParams`](spray::SprayParams) ([`spray::spray`]); a caller with a pool
    /// sprays them in parallel and calls [`from_tracers`](Self::from_tracers) with the galaxy's
    /// seed and its halo's mean system mass, as this does.
    ///
    /// # Errors
    ///
    /// [`BuildTubeTableError::Spray`] as [`spray::spray`]; as
    /// [`from_tracers`](Self::from_tracers) otherwise.
    pub fn build(galaxy: &Galaxy, spec: &StreamSpec) -> Result<Self, BuildTubeTableError> {
        let tracers = spray::spray(galaxy, spec)?;
        Self::from_tracers(
            galaxy.seed(),
            galaxy.mean_system_mass(Population::Halo),
            spec,
            &tracers,
        )
    }

    /// The tube table reduced from `tracers`, the spray of `spec`'s stream in index order, with
    /// the gap noise of `seed` and along cells for members of `mean_system_mass`, the halo's
    /// (module documentation, "Gaps").
    ///
    /// The tracers are [`Spray::tracer`](spray::Spray::tracer)'s, as a pool gathers them by index
    /// (main's ruling of 2026-10-29, after ruling 146.3), and the reduction is the module
    /// documentation's. Tracers of equal ψ keep their order, so the table is a pure function of
    /// the seed, the mass, the stream's number and the list.
    ///
    /// # Errors
    ///
    /// [`BuildTubeTableError::TooFewTracers`] if either arm, by ψ's sign, holds fewer than
    /// [`KERNEL_TRACERS`] tracers, which none of a drawn Milky Way galaxy's 309 streams does at
    /// the generator's settings (P10.T6.a, as built).
    ///
    /// # Panics
    ///
    /// If a tracer's weight is not positive and finite, which a spray's, each half its stratum's
    /// share of its tube's positive mass, always is; or if `mean_system_mass` is not, which a
    /// galaxy's never is.
    pub fn from_tracers(
        seed: Seed,
        mean_system_mass: SolarMasses,
        spec: &StreamSpec,
        tracers: &[Tracer],
    ) -> Result<Self, BuildTubeTableError> {
        let mut along: Vec<&Tracer> = tracers.iter().collect();
        // Stable, so tracers of equal ψ keep their index order. A tracer's ψ is finite
        // (`Spray::tracer`), so `total_cmp` reads no NaN's sign.
        along.sort_by(|a, b| a.psi().value().total_cmp(&b.psi().value()));
        // −0.0 sorts before +0.0 and compares equal to zero: both are the leading arm's.
        let split = along.partition_point(|t| t.psi().value() < 0.0);
        let (trailing, leading) = along.split_at(split);
        assert!(
            tracers
                .iter()
                .all(|t| t.weight().value() > 0.0 && t.weight().value().is_finite()),
            "a spray's tracers weigh their tube's positive mass"
        );
        let count = |arm, on: &[&Tracer]| {
            // A spray's tracers number a `u32` (`Spray::tracer_count`), and no other list of them
            // can be made, so the saturation is never reached.
            let tracers = u32::try_from(on.len()).unwrap_or(u32::MAX);
            if tracers < KERNEL_TRACERS {
                Err(BuildTubeTableError::TooFewTracers { arm, tracers })
            } else {
                Ok(tracers)
            }
        };
        let counts = [
            count(Arm::Trailing, trailing)?,
            count(Arm::Leading, leading)?,
        ];
        let reduced = [Reduced::new(trailing), Reduced::new(leading)];
        let frames = frames(&reduced);
        // Λ from the trailing arm's tip: the leading arm's starts at the trailing arm's mass.
        let bases = [0.0, reduced[0].mass];
        let arms = core::array::from_fn(|side| {
            reduced[side].finish(&frames[side], bases[side], counts[side])
        });
        let mass = SolarMasses::new(reduced[0].mass + reduced[1].mass);
        let mut table = Self {
            stream: spec.number(),
            arms,
            mass,
            seed,
            along_cells: along_cells(mass, mean_system_mass),
            noise_mean: 1.0,
        };
        let mut nodes = table.gap_nodes();
        let sum = (0..table.along_cells).fold(0.0, |sum, cell| {
            sum + table.undivided_noise(&mut nodes, cell)
        });
        table.noise_mean = sum / f64::from(table.along_cells);
        assert!(
            table.noise_mean > 0.0 && table.noise_mean.is_finite(),
            "a mean of exponentials of finite normals is positive and finite: {}",
            table.noise_mean
        );
        Ok(table)
    }

    /// The stream whose tube this is.
    #[must_use]
    pub const fn stream(&self) -> StreamNumber {
        self.stream
    }

    /// The tube's mass, `Λ_total`: its tracers' weights summed, the arms' masses added.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The number of along cells the tube is cut into, cells of equal mass: the power of two
    /// nearest `Λ_total ÷ (m̄ × 64)`, nearest in its logarithm, with `m̄` the mean system mass it
    /// was built with and 64 [`CROSS_SECTION_MEMBERS`] (plan 10, Design note 7), held to
    /// [`MIN_ALONG_CELLS`] (this task's lean) and [`MAX_ALONG_CELLS`] (the note's). Cell `i` holds
    /// the along coordinates `[i, i + 1) ÷ cells`; the table does not keep `m̄`.
    #[must_use]
    pub const fn along_cells(&self) -> u32 {
        self.along_cells
    }

    /// The gap noise of along cell `cell`, the factor on its mean count (module documentation,
    /// "Gaps"): positive, constant within the cell, and summing over the cells to
    /// [`along_cells`](Self::along_cells), up to rounding.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`along_cells`](Self::along_cells).
    #[must_use]
    pub fn noise(&self, cell: u32) -> f64 {
        assert!(
            cell < self.along_cells,
            "the tube has {} along cells, not {cell}",
            self.along_cells
        );
        self.undivided_noise(&mut self.gap_nodes(), cell) / self.noise_mean
    }

    /// The stream the gap noise's nodes are drawn on: [`tags::STREAM_TUBE_GAPS`] keyed by the
    /// stream's number (plan 10, Design note 14).
    #[must_use]
    fn gap_nodes(&self) -> Stream {
        Stream::open(
            self.seed,
            tags::STREAM_TUBE_GAPS,
            ObjectKey::galaxy_item(u64::from(self.stream.get())),
        )
    }

    /// `exp(σ g − σ² ÷ 2)` at along cell `cell`'s middle, before it is divided by its mean over
    /// the cells, with node normals read from `nodes` (module documentation, "Gaps").
    #[must_use]
    fn undivided_noise(&self, nodes: &mut Stream, cell: u32) -> f64 {
        let cells = f64::from(self.along_cells);
        let psi = self.psi((f64::from(cell) + 0.5) / cells).value();
        let low = self.arms[0].knots[0].psi.value();
        let high = self.arms[1].knots[self.arms[1].knots.len() - 1].psi.value();
        // The trailing arm's tip lies at a negative ψ and the leading arm's at zero or more, so
        // the span is positive.
        let spacing = f64::from(CELLS_PER_GAP_NODE) * (high - low) / cells;
        let intervals = self.along_cells / CELLS_PER_GAP_NODE;
        let t = ((psi - low) / spacing).clamp(0.0, f64::from(intervals));
        debug_assert!(t.is_finite(), "ψ is finite and the span positive: {t}");
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "t is clamped to [0, intervals − 1] first, intervals at most 2,048"
        )]
        let below = t.min(f64::from(intervals - 1)).floor() as u32;
        let s = fade(t - f64::from(below));
        let mut normal = |node: u32| {
            nodes.seek(2 * u64::from(node));
            nodes.standard_normal()
        };
        let (first, second) = (normal(below), normal(below + 1));
        let g = ((1.0 - s) * first + s * second) / ((1.0 - s) * (1.0 - s) + s * s).sqrt();
        math::exp(GAP_NOISE_AMPLITUDE * g - 0.5 * GAP_NOISE_AMPLITUDE * GAP_NOISE_AMPLITUDE)
    }

    /// The arm of ψ's sign (module documentation, "The arms").
    #[must_use]
    pub const fn arm(&self, arm: Arm) -> &TubeArm {
        match arm {
            Arm::Trailing => &self.arms[0],
            Arm::Leading => &self.arms[1],
        }
    }

    /// Every knot in the order of the along coordinate: the trailing arm's, then the leading
    /// arm's.
    pub fn knots(&self) -> impl Iterator<Item = &TubeKnot> {
        self.arms.iter().flat_map(|arm| arm.knots.iter())
    }

    /// The centre line's length, from the trailing arm's tip to the leading arm's through the
    /// segment between the arms' roots.
    #[must_use]
    pub fn length(&self) -> LightYears {
        let metres = self
            .knots()
            .zip(self.knots().skip(1))
            .fold(0.0, |sum, (from, to)| {
                sum + from.centre.distance_to(&to.centre).value()
            });
        LightYears::new(metres / METRES_PER_LIGHT_YEAR)
    }

    /// The point at along coordinate `u` and across coordinates `a` and `b`.
    ///
    /// It is the centre line's point where the share `u` of the tube's mass lies behind it, moved
    /// `a` widths along the normal and `b` widths along the binormal, each quantity linear between
    /// knots and the normal and binormal then made unit vectors (module documentation, "Between
    /// knots"). `u` is clamped to `[0, 1]`; a member's across coordinates lie within ±4 widths
    /// (plan 10, Design note 7).
    ///
    /// # Panics
    ///
    /// If `u`, `a` or `b` is not finite, or `a` or `b` is beyond [`ACROSS_LIMIT`] widths, so far
    /// out that the point could leave the light-year cells' range.
    #[must_use]
    pub fn locate(&self, u: f64, a: f64, b: f64) -> GalacticPosition {
        assert!(
            u.is_finite() && a.abs() <= ACROSS_LIMIT && b.abs() <= ACROSS_LIMIT,
            "a tube's coordinates are finite and within {ACROSS_LIMIT} widths: {u}, {a}, {b}"
        );
        let (from, to, f) = self.segment(u);
        let along = from.centre.displacement_to(&to.centre).metres();
        let [_, normal, binormal] = interpolated_frame(&from.frame, &to.frame, f);
        let lerp = |p: f64, q: f64| p + f * (q - p);
        let (width_n, width_b) = (
            lerp(from.widths[0].value(), to.widths[0].value()) * METRES_PER_LIGHT_YEAR,
            lerp(from.widths[1].value(), to.widths[1].value()) * METRES_PER_LIGHT_YEAR,
        );
        let offset: [f64; 3] = core::array::from_fn(|i| {
            f * along[i] + a * width_n * normal[i] + b * width_b * binormal[i]
        });
        from.centre
            .translated(GalacticDisplacement::new(offset))
            .expect(
                "a point within ACROSS_LIMIT widths of a knot, every width a tracer's rms offset, \
             fits a light-year cell",
            )
    }

    /// The tracers' mean velocity at along coordinate `u`, linear between knots.
    ///
    /// `u` is clamped to `[0, 1]`.
    ///
    /// # Panics
    ///
    /// If `u` is not finite.
    #[must_use]
    pub fn mean_velocity(&self, u: f64) -> GalacticVelocity {
        assert!(u.is_finite(), "a tube's along coordinate is finite: {u}");
        let (from, to, f) = self.segment(u);
        let (p, q) = (
            from.velocity.metres_per_second(),
            to.velocity.metres_per_second(),
        );
        GalacticVelocity::new(core::array::from_fn(|i| p[i] + f * (q[i] - p[i])))
    }

    /// ψ at along coordinate `u`, the inverse of `Λ(ψ) ÷ Λ_total`, linear between knots.
    ///
    /// `u` is clamped to `[0, 1]`.
    ///
    /// # Panics
    ///
    /// If `u` is not finite.
    #[must_use]
    pub fn psi(&self, u: f64) -> Radians {
        assert!(u.is_finite(), "a tube's along coordinate is finite: {u}");
        let (from, to, f) = self.segment(u);
        let (p, q) = (from.psi.value(), to.psi.value());
        Radians::new(p + f * (q - p))
    }

    /// The knots on either side of along coordinate `u`, clamped to `[0, 1]`, and its fraction of
    /// the way between them: the segment of the arm whose Λ rises through `u Λ_total`, never the
    /// one between the arms' roots, where Λ does not rise.
    #[must_use]
    fn segment(&self, u: f64) -> (&TubeKnot, &TubeKnot, f64) {
        let target = u.clamp(0.0, 1.0) * self.mass.value();
        let arm = if target < self.arms[0].mass.value() {
            &self.arms[0]
        } else {
            &self.arms[1]
        };
        let knots = &arm.knots;
        let after = knots.partition_point(|k| k.cumulative_mass.value() <= target);
        match after {
            0 => (&knots[0], &knots[0], 0.0),
            n if n == knots.len() => (&knots[n - 1], &knots[n - 1], 0.0),
            n => {
                let (from, to) = (&knots[n - 1], &knots[n]);
                let (low, high) = (from.cumulative_mass.value(), to.cumulative_mass.value());
                (from, to, (target - low) / (high - low))
            }
        }
    }
}

/// One arm's knots before their frames: the kernel's means, and what the widths and dispersions
/// need of the tracers.
struct Reduced<'a> {
    tracers: &'a [&'a Tracer],
    /// The arm's mass, its tracers' weights summed in order.
    mass: f64,
    knots: Vec<KernelMean>,
}

/// A knot's rank, its kernel's local-linear means of position (m) and velocity (m/s) in the
/// galactic frame, and ψ at the rank.
#[derive(Debug, Clone, Copy)]
struct KernelMean {
    rank: f64,
    position_m: [f64; 3],
    /// The fitted line's slope in rank, m.
    slope_m: [f64; 3],
    velocity_m_s: [f64; 3],
    psi: f64,
}

impl<'a> Reduced<'a> {
    /// The knots of the arm of `tracers`, in the order of increasing ψ, at least
    /// [`KERNEL_TRACERS`] of them.
    #[must_use]
    fn new(tracers: &'a [&'a Tracer]) -> Self {
        // Each tracer's cumulative weight to its middle.
        let mut mass = 0.0;
        let middles: Vec<f64> = tracers
            .iter()
            .map(|t| {
                let w = t.weight().value();
                let middle = mass + 0.5 * w;
                mass += w;
                middle
            })
            .collect();
        let last = index_f64(tracers.len() - 1);
        let steps = f64::from(KNOTS_PER_ARM - 1);
        let knots = (0..KNOTS_PER_ARM)
            .map(|j| {
                let target = mass * (f64::from(j) / steps);
                let after = middles.partition_point(|&m| m <= target);
                let rank = match after {
                    0 => 0.0,
                    n if n == middles.len() => last,
                    n => {
                        let (low, high) = (middles[n - 1], middles[n]);
                        index_f64(n - 1) + (target - low) / (high - low)
                    }
                };
                let rank = rank.clamp(0.0, last);
                // The kernel's moments in rank, for its weights corrected to first order.
                let [mut s0, mut s1, mut s2] = [0.0; 3];
                for (i, k) in kernel(rank, tracers) {
                    let d = index_f64(i) - rank;
                    s0 += k;
                    s1 += k * d;
                    s2 += k * d * d;
                }
                let determinant = s0 * s2 - s1 * s1;
                assert!(
                    determinant > 0.0 && determinant.is_finite(),
                    "a spray's tracers weigh their tube's positive mass: {determinant}"
                );
                let mut position_m = [0.0; 3];
                let mut slope_m = [0.0; 3];
                let mut velocity_m_s = [0.0; 3];
                for (i, k) in kernel(rank, tracers) {
                    let d = index_f64(i) - rank;
                    let (level, rise) = (
                        k * (s2 - s1 * d) / determinant,
                        k * (s0 * d - s1) / determinant,
                    );
                    let t = tracers[i];
                    let (x, v) = (t.position().metres(), t.velocity().metres_per_second());
                    for axis in 0..3 {
                        position_m[axis] += level * x[axis];
                        slope_m[axis] += rise * x[axis];
                        velocity_m_s[axis] += level * v[axis];
                    }
                }
                // ψ at the rank itself, between the two tracers about it: the inverse of the
                // tracers' own Λ(ψ), which keeps the knots in order and on their arm's side.
                let below = rank.floor();
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "rank is finite and in [0, n − 1] for an arm of n tracers, n a u32"
                )]
                let (lower, upper) = (below as usize, rank.ceil() as usize);
                let (p, q) = (tracers[lower].psi().value(), tracers[upper].psi().value());
                KernelMean {
                    rank,
                    position_m,
                    slope_m,
                    velocity_m_s,
                    psi: p + (rank - below) * (q - p),
                }
            })
            .collect();
        Self {
            tracers,
            mass,
            knots,
        }
    }

    /// The arm's knots with their `frames`, each knot's Λ `base` plus its share of the arm's mass,
    /// its widths and dispersions measured about the centre line at each tracer's rank (module
    /// documentation, "The reduction"); `tracers` the arm's count.
    #[must_use]
    fn finish(&self, frames: &[[[f64; 3]; 3]], base: f64, tracers: u32) -> TubeArm {
        let ranks: Vec<f64> = self.knots.iter().map(|k| k.rank).collect();
        // Each tracer's offsets from the centre line at its own rank: across, along the normal and
        // the binormal (m), and of its velocity from the mean along the frame (m/s).
        let offsets: Vec<[f64; 5]> = self
            .tracers
            .iter()
            .enumerate()
            .map(|(i, tracer)| {
                let rank = index_f64(i);
                let after = ranks.partition_point(|&r| r <= rank);
                let below = after.clamp(1, ranks.len() - 1) - 1;
                let span = ranks[below + 1] - ranks[below];
                let fraction = if span > 0.0 {
                    (rank - ranks[below]) / span
                } else {
                    0.0
                };
                let (from, to) = (&self.knots[below], &self.knots[below + 1]);
                let lerp = |start: [f64; 3], end: [f64; 3]| -> [f64; 3] {
                    core::array::from_fn(|axis| start[axis] + fraction * (end[axis] - start[axis]))
                };
                let centre = lerp(from.position_m, to.position_m);
                let mean_velocity = lerp(from.velocity_m_s, to.velocity_m_s);
                let [tangent, normal, binormal] =
                    interpolated_frame(&frames[below], &frames[below + 1], fraction);
                let (position, velocity) = (
                    tracer.position().metres(),
                    tracer.velocity().metres_per_second(),
                );
                let offset: [f64; 3] = core::array::from_fn(|axis| position[axis] - centre[axis]);
                let drift: [f64; 3] =
                    core::array::from_fn(|axis| velocity[axis] - mean_velocity[axis]);
                [
                    dot(offset, normal),
                    dot(offset, binormal),
                    dot(drift, tangent),
                    dot(drift, normal),
                    dot(drift, binormal),
                ]
            })
            .collect();
        let steps = f64::from(KNOTS_PER_ARM - 1);
        let knots = self
            .knots
            .iter()
            .zip(frames)
            .zip(0_u32..)
            .map(|((mean, frame), j)| {
                let mut squares = [0.0; 5];
                let mut total = 0.0;
                for (i, k) in kernel(mean.rank, self.tracers) {
                    for (s, o) in squares.iter_mut().zip(offsets[i]) {
                        *s += k * o * o;
                    }
                    total += k;
                }
                let rms = squares.map(|s| (s / total).sqrt());
                TubeKnot {
                    centre: GalacticPosition::ORIGIN
                        .translated(GalacticDisplacement::new(mean.position_m))
                        .expect(
                            "a tracer's position, and so a mean of them, fits a light-year cell",
                        ),
                    frame: *frame,
                    velocity: GalacticVelocity::new(mean.velocity_m_s),
                    widths: [
                        LightYears::new(rms[0] / METRES_PER_LIGHT_YEAR),
                        LightYears::new(rms[1] / METRES_PER_LIGHT_YEAR),
                    ],
                    dispersions: [
                        MetresPerSecond::new(rms[2]),
                        MetresPerSecond::new(rms[3]),
                        MetresPerSecond::new(rms[4]),
                    ],
                    cumulative_mass: SolarMasses::new(base + self.mass * (f64::from(j) / steps)),
                    psi: Radians::new(mean.psi),
                }
            })
            .collect();
        TubeArm {
            knots,
            tracers,
            mass: SolarMasses::new(self.mass),
        }
    }
}

/// The kernel about fractional `rank` over an arm's `tracers`, of [`KERNEL_TRACERS`] or more:
/// the indices of the [`KERNEL_TRACERS`] nearest in rank, in rank order, each with its weight
/// times the Gaussian's (module documentation, "The reduction").
fn kernel<'a>(rank: f64, tracers: &'a [&'a Tracer]) -> impl Iterator<Item = (usize, f64)> + 'a {
    let width = index(KERNEL_TRACERS);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rank is finite and in [0, n − 1] for an arm of n tracers, n a u32"
    )]
    let nearest = (rank + 0.5).floor() as usize;
    let start = nearest.saturating_sub(width / 2).min(tracers.len() - width);
    (start..start + width).map(move |i| {
        let d = (index_f64(i) - rank) / KERNEL_WIDTH;
        (i, tracers[i].weight().value() * math::exp(-0.5 * d * d))
    })
}

/// The frames of both arms' knots (module documentation, "The frame"): the trailing arm's, then
/// the leading arm's, each `[tangent, normal, binormal]`.
#[must_use]
fn frames(arms: &[Reduced<'_>; 2]) -> [Vec<[[f64; 3]; 3]>; 2] {
    let knots: Vec<&KernelMean> = arms.iter().flat_map(|arm| arm.knots.iter()).collect();
    let n = knots.len();
    // The fitted line's direction, along increasing rank, which is increasing ψ in either arm.
    // A kernel of a spray's tracers never fits a line of no slope; the x axis only keeps one
    // finite.
    let tangents: Vec<[f64; 3]> = knots
        .iter()
        .map(|k| unit(k.slope_m).unwrap_or([1.0, 0.0, 0.0]))
        .collect();
    // The trailing arm's root, the knot nearest the progenitor.
    let start = arms[0].knots.len() - 1;
    let mut normals = vec![[0.0; 3]; n];
    let root = knots[start];
    let pole = cross(root.position_m, root.velocity_m_s);
    normals[start] = perpendicular(cross(pole, tangents[start]), tangents[start]);
    for k in start..n - 1 {
        normals[k + 1] = transported(
            normals[k],
            [knots[k].position_m, knots[k + 1].position_m],
            [tangents[k], tangents[k + 1]],
        );
    }
    for k in (1..=start).rev() {
        normals[k - 1] = transported(
            normals[k],
            [knots[k].position_m, knots[k - 1].position_m],
            [tangents[k], tangents[k - 1]],
        );
    }
    let mut all = tangents
        .into_iter()
        .zip(normals)
        .map(|(t, normal)| [t, normal, cross(t, normal)]);
    let trailing = all.by_ref().take(start + 1).collect();
    [trailing, all.collect()]
}

/// `normal`, at points `x[0]` with tangent `t[0]`, carried to `x[1]` with tangent `t[1]` by the
/// double reflection (Wang et al. 2008, "Computation of rotation minimizing frames", §4), then made
/// a unit vector perpendicular to `t[1]`.
#[must_use]
fn transported(normal: [f64; 3], x: [[f64; 3]; 2], t: [[f64; 3]; 2]) -> [f64; 3] {
    let reflect = |r: [f64; 3], about: [f64; 3]| -> [f64; 3] {
        let square = dot(about, about);
        if square > 0.0 {
            let k = 2.0 * dot(about, r) / square;
            core::array::from_fn(|axis| r[axis] - k * about[axis])
        } else {
            r
        }
    };
    let chord: [f64; 3] = core::array::from_fn(|axis| x[1][axis] - x[0][axis]);
    let (normal, tangent) = (reflect(normal, chord), reflect(t[0], chord));
    let second: [f64; 3] = core::array::from_fn(|axis| t[1][axis] - tangent[axis]);
    perpendicular(reflect(normal, second), t[1])
}

/// `v` less its component along the unit `tangent`, made a unit vector, or, if that leaves
/// nothing, a unit vector perpendicular to `tangent` from the axis farthest from it.
#[must_use]
fn perpendicular(v: [f64; 3], tangent: [f64; 3]) -> [f64; 3] {
    let along = dot(v, tangent);
    unit(core::array::from_fn(|axis| v[axis] - along * tangent[axis])).unwrap_or_else(|| {
        let axis = if tangent[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        unit(cross(tangent, axis))
            .expect("a unit vector crossed with an axis 26° or more from it is not zero")
    })
}

/// The frame a fraction `f` of the way from `from` to `to`: each vector linear between them, then
/// made a unit vector, so that where the frame turns between knots the across axes keep their
/// length (by 0.89 of it at mid-segment for a turn of 54°, else).
#[must_use]
fn interpolated_frame(from: &[[f64; 3]; 3], to: &[[f64; 3]; 3], f: f64) -> [[f64; 3]; 3] {
    core::array::from_fn(|axis| {
        let v: [f64; 3] =
            core::array::from_fn(|i| from[axis][i] + f * (to[axis][i] - from[axis][i]));
        unit(v).unwrap_or(from[axis])
    })
}

/// `v` over its length, or `None` for a zero or non-finite vector.
#[must_use]
fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(v, v).sqrt();
    (length > 0.0 && length.is_finite()).then(|| v.map(|c| c / length))
}

/// The along cells of a tube of `mass` whose members' mean mass is `mean_system_mass`
/// ([`TubeTable::along_cells`]): doubled from [`MIN_ALONG_CELLS`] while the mass in a
/// cross-section of [`CROSS_SECTION_MEMBERS`] lies at or above the geometric middle between this
/// count and the next, up to [`MAX_ALONG_CELLS`].
///
/// # Panics
///
/// If `mean_system_mass` is not positive and finite.
#[must_use]
fn along_cells(mass: SolarMasses, mean_system_mass: SolarMasses) -> u32 {
    let mean = mean_system_mass.value();
    assert!(
        mean > 0.0 && mean.is_finite(),
        "a galaxy's mean system mass is positive and finite: {mean}"
    );
    let ideal = mass.value() / (mean * CROSS_SECTION_MEMBERS);
    let mut cells = MIN_ALONG_CELLS;
    while cells < MAX_ALONG_CELLS && ideal >= f64::from(cells) * core::f64::consts::SQRT_2 {
        cells *= 2;
    }
    cells
}

/// The quintic fade `s(t) = t³ (6t² − 15t + 10)`, which rises from 0 to 1 over `[0, 1]` with its
/// first and second derivatives 0 at both ends, as the gas noise's
/// ([`gas::noise`](crate::galaxy::gas::noise)).
#[must_use]
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// A `u32` as an index.
#[must_use]
fn index(i: u32) -> usize {
    usize::try_from(i)
        .expect("a u32 fits a usize on every target the sim builds for, of 32 bits or more")
}

/// An index into an arm's tracers, at most a spray's `u32` count, as a float, exactly.
#[must_use]
fn index_f64(i: usize) -> f64 {
    f64::from(u32::try_from(i).expect("an arm holds at most a spray's u32 count of tracers"))
}

/// What a caller's cache of tube tables implements: the galaxy's global list, and each stream's
/// table on request (plan 10, "Provides").
///
/// The server's `TubeTableCache` (P10.T10.b) builds a table on a miss inside the job that asked
/// and evicts by bytes; eviction is safe, because a table is a pure function of the galaxy and
/// the stream number and is rebuilt to the same bits. An implementation must hand out the list
/// of the galaxy it is asked about, and the table of `stream` in it.
///
/// # Examples
///
/// A lookup for a caller that has not built the list yet, which no stream or core member can
/// resolve through:
///
/// ```
/// use std::sync::Arc;
///
/// use hyperion_sim::galaxy::global_list::{GlobalList, StreamNumber, TubeLookup, TubeTable};
///
/// struct Empty(GlobalList);
///
/// impl TubeLookup for Empty {
///     fn global_list(&self) -> &GlobalList {
///         &self.0
///     }
///
///     fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
///         unreachable!("an empty list has no stream {}", stream.get())
///     }
/// }
///
/// let lookup = Empty(GlobalList::default());
/// assert_eq!(lookup.global_list().kinds().count(), 0);
/// ```
pub trait TubeLookup {
    /// The galaxy's global list.
    fn global_list(&self) -> &GlobalList;

    /// The tube table of `stream`, built if the cache does not hold it. `stream` is below the
    /// list's stream count; callers check that against [`global_list`](Self::global_list) first.
    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable>;
}

#[cfg(test)]
mod tests;
