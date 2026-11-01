//! A stream's tube table and the cache trait that hands it out (plan 10, P10.T6 and P10.T1).
//!
//! A tube table is measured, not assumed: about 2,000 tracers sprayed from the progenitor and
//! integrated in the potential ([`spray`]), reduced to about 256 knots per arm
//! (brainstorm, "Streams and accreted structure"). It costs about 0.2 s and 80 kB, so it is a
//! cache and never stored, and the cache is the caller's ("Runtime and code shape"): the sim asks
//! for a table through [`TubeLookup`] and keeps none itself. `pal5_like`'s takes 2.1 s sprayed
//! alone and 0.14 s on 16 threads, and holds 90 kB (`benches/global_list.rs`).
//!
//! # The arms
//!
//! A tracer belongs to the arm of its energy offset's sign at release
//! ([`Tracer::energy_arm`](spray::Tracer::energy_arm)): the leading arm for an offset below zero,
//! the debris more bound than the progenitor, and the trailing arm otherwise, a zero of either
//! sign included (main's ruling of 2026-10-30, after ψ's sign of 2026-10-29). A few tracers leave
//! one Lagrange point with the other arm's energy and drift with that arm ([`spray`], "The
//! arms"); split by their release point, a knot's kernel would average positions far apart at the
//! arm's root. Split by energy, each falls among the other arm's tracers at its ψ and keeps its
//! weight, so the arms' masses differ from half the tube's by those tracers' weight. ψ's sign
//! failed where the tracers have hardly drifted, as for an orphan on an eccentric orbit near its
//! apocentre, whose ψ swings through the orbit and hides the sign of the drift; the energy's sign
//! does not swing. Such a stream is a cloud, not a tube, whichever the split, a recorded limit
//! (plan 10, "P10.T6.a's arms by energy, as built").
//!
//! # The reduction
//!
//! The tube is ordered along by arm, the trailing first, and within an arm by ψ, from the trailing
//! arm's far end through the progenitor to the leading arm's, the order of the along coordinate
//! `u = Λ(ψ) ÷ Λ_total` (plan 10, Design note 7), so each arm's knots run in that order too: the
//! trailing arm's from its tip to its root, the leading arm's from its root to its tip. Where every
//! tracer's ψ has its energy arm's sign, that is the tube's order by ψ alone; in a cloud an arm's ψ
//! may cross zero, and ψ may step back between the arms' roots. Within an arm (Design note 6):
//!
//! - **Knots** sit at [`KNOTS_PER_ARM`] equal steps of the arm's cumulative weight, the first and
//!   last at its ends, each at the fractional rank where the cumulative weight, counted to each
//!   tracer's middle, reaches its step. A knot's ψ is the tracers' own at that rank, linear between
//!   the two about it: the inverse of the tracers' Λ(ψ), so the knots keep their arm's order.
//! - **Every other knot quantity is a Gaussian-kernel average** over the [`KERNEL_TRACERS`]
//!   tracers nearest the knot's rank, each weighted by its own weight times `exp(−½ (Δ ÷ σ)²)`,
//!   `Δ` its rank's distance from the knot's and `σ` [`KERNEL_WIDTH`] ranks.
//!   - The centre line and the mean velocity are the kernel's local-linear averages: the
//!     intercept at the knot's rank of the line fitted in rank by the kernel's weights, which is
//!     the kernel's mean where its tracers lie evenly about the knot. At an arm's end the kernel
//!     reaches one way only, and its mean would lie inside the end by about a dispersion in rank,
//!     and with it the mean velocity's direction, by about the angle the track turns there. The
//!     fit removes that first-order bias (Fan and Gijbels 1992, "Variable bandwidth and local
//!     linear regression smoothers", Ann. Statist. 20, 2008–2036), at the price of more noise
//!     there: an end knot's fit rests on an effective 7 tracers, against the one-sided mean's 22
//!     and an inner knot's 39. What it keeps is its second-order bias, about half the curvature
//!     of what it fits along the kernel times the kernel's width squared, of the other sign at an
//!     arm's end knot.
//!   - So each quantity is fitted where the track is nearly straight: in the kernel's orbital
//!     plane about the galactic centre, as its radius `R`, angle `φ` and height `z`, and the
//!     velocity's radial, azimuthal and vertical components, each tracer's along its own
//!     directions, mapped back at the fitted `φ`. An arc about the centre is constant in `R` and
//!     `z` and linear in `φ`, so the track's turn carries no bias. Fitted in x, y and z, the line
//!     was a chord inside the arc but at an arm's end knot, and at a tip, where the few old
//!     tracers make a kernel span 70° of the orbit and more, that put `pal5_like`'s trailing
//!     knots below every tracer and its shell's inner radius, with widths 6–22 times the median.
//!     What curvature is left is `R`'s and `z`'s along `φ`, largest at the orbit's turning points.
//!     (A stream lies near a segment of its progenitor's orbit, a straight line in angle-action
//!     coordinates: Sanders and Binney 2013, "Stream-orbit misalignment – I. The dangers of
//!     orbit-fitting", MNRAS 433, 1813–1825; Bovy 2014, "Dynamical modeling of tidal streams",
//!     ApJ 795, 95.) The plane's pole is the kernel's weighted angular momentum's direction, and
//!     its first axis the Cartesian fit's position projected on it. Where the angle is ill-posed
//!     (a kernel tracer more than three-eighths of a turn from that axis, within an eighth of a
//!     turn of `atan2`'s cut, or within a tenth of the kernel's mean `R` of the pole's line) the
//!     knot keeps the Cartesian fit (main's brief for P10.T6.a's tips, after research-leans-10's
//!     pilot). Over eight seeds `pal5_like`'s members outside its shell number 3 in all, against
//!     147 at the first seed alone in x, y and z, and every knot of its tube and `gd1_like`'s lies
//!     within its arm's tracers' radial range.
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
//! [`CELLS_PER_GAP_NODE`] mean cells apart in ψ, `8 (ψ_max − ψ_min) ÷ cells` from the least of the
//! knots' ψ to the greatest, the trailing arm's tip and the leading arm's where every tracer's ψ
//! has its arm's sign, so that no wavelength is shorter than eight mean cells. "Eight
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
//!
//! # Cells
//!
//! A member lies in a cell of the tube's own coordinates, a [`TubeCell`]: an along cell, and
//! across each of the normal and the binormal one of [`ACROSS_CELLS`] cells spanning
//! ±[`CUT_WIDTHS`] local widths, where the tube is cut (Design note 7).
//! [`TubeTable::cells_touching`] finds the cells that may hold a point within a radius of a
//! centre, conservatively: every cell holding such a point is found, and a few holding none may
//! be. For each knot segment whose reach (below) meets the sphere, and each along cell's stretch
//! of it, it bounds a point's distance from the centre from below in an orthonormal basis made
//! from the frame at the stretch's middle (the normal, the binormal made perpendicular to it, and
//! their cross product):
//!
//! - the centre line's motion over the stretch moves each coordinate by at most the chord's
//!   component times half the stretch, and the along coordinate's least magnitude is read over it;
//! - a point at across coordinates `a` and `b` moves, as the frame and the widths change over the
//!   stretch, by at most `|a|` times the normal's width change plus the larger normal width times
//!   the chord between the normal's unit vectors at the stretch's ends, and as much along the
//!   binormal for `|b|`, which widens the radius for the cell's farthest `a` and `b`; the stretch
//!   is cut into pieces over which the frame turns by a chord of at most 0.05;
//! - then each row of binormal cells is an interval test, and the normal cells a row allows an
//!   interval too, found for the cut's farthest `a` and then again for the farthest it allows.
//!
//! The rows are tested apart from the normal's coordinate, so the found cells cover the ball's
//! cross-section by a square's, not a disc's. Once the census sums the found cells' means (plan
//! 10, P10.T9), this test is output.
//!
//! # Finding the segments
//!
//! Every point of a knot segment's tube within the cut lies within the segment's *reach* of its
//! centre line: [`CUT_WIDTHS`] times `√(w_n² + w_b² + 2 w_n w_b c)`, with each width the larger at
//! the segment's two knots and `c` a bound on the cosine between the normal and the binormal as
//! they are interpolated between them, zero at the knots. Each arm keeps its segments' reaches,
//! and [`TubeTable::cells_touching`] scans the 510 segments' boxes, padded by their reaches, for
//! those whose reach about the centre line meets the sphere. A hash of the boxes on a grid of
//! cells sized to each tube (main's ruling of 2026-10-30, which left this scan as the fallback)
//! was measured against it on the three test streams and a drawn Milky Way galaxy's 306 tubes
//! (pod, 16 vCPU, P10.T6.c): the scan alone finds a 50 ly sphere's segments as fast, in
//! 2.3–7.1 µs against the hash's 2.1–9.9 µs on the test streams, since a cell of 2,048–65,536 ly holds many segments a
//! small sphere misses, and it keeps 4 kB a tube against the hash's 5.6–25 kB there and up to
//! 64 kB in the drawn galaxy.
//!

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity};
use crate::galaxy::{Galaxy, Population};
use crate::id::StreamMemberId;
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{KilometresPerSecond, LightYears, MetresPerSecond, Radians, SolarMasses};

use super::orbit::{BuildLeapfrogError, cross, dot};
use super::source::GlobalListModels;
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

/// The tube's cut, in widths either side of the centre line along each across axis, a parameter
/// of the generator version: 4 (brainstorm, "Streams and accreted structure": "the tube is cut at
/// four widths"; plan 10, Design note 7).
pub const CUT_WIDTHS: f64 = 4.0;

/// The largest internal speed a stream member's velocity draw can reach, in units of its tube's
/// greatest velocity dispersion: 8.58 (1 + √2) = 20.71 (P10.T9).
///
/// The draw is a Box–Muller pair and the first normal of a second, each times its own dispersion
/// along one of the local frame's three vectors ([`members`](super::members), P10.T8.c). A pair's
/// norm is at most 8.58 (plan 09's internal speed bound, 8.58 √2, over √2), so the pair's two
/// normals sum in magnitude to at most 8.58 √2 and the third is at most 8.58. The frame's vectors
/// are unit vectors, but between knots each is interpolated and made a unit vector again apart,
/// so they need not be orthogonal: the triangle inequality bounds the internal speed by the
/// dispersion times the three magnitudes' sum, not by their root sum of squares, 8.58 √2.
pub const MEMBER_SPEED_SIGMAS: f64 = 8.58 * (1.0 + std::f64::consts::SQRT_2);

/// The relative margin [`TubeTable::member_speed_bound`] is rounded up by, as plan 09's features'
/// bound is: a member's speed is the norm of a sum, which can pass the bound's by a few units in
/// the last place.
const SPEED_BOUND_MARGIN: f64 = 1e-9;

/// The cells across the tube along each across axis, spanning ±[`CUT_WIDTHS`]: 64 (plan 10,
/// Design note 7), which the 6 bits of each across field of a stream member's ID hold (plan 01's
/// `StreamMemberId`).
pub const ACROSS_CELLS: u8 = 64;

/// What the cells' tests add to every distance, ly: far above the rounding of the float
/// light-years they compare (69 km at 50,000 ly, `GalacticPosition::to_light_years_f64`), far
/// below a cell across (an eighth of a width).
const MARGIN_LY: f64 = 1e-3;

/// The farthest the frame may turn, as the chord between its unit vectors, over one piece of the
/// cells' test (module documentation, "Cells"): 0.05, about 2.9°.
const PIECE_CHORD: f64 = 0.05;

/// The most pieces one segment's stretch of an along cell is cut into for the cells' test.
const MAX_PIECES: u32 = 32;

/// A tube table could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildTubeTableError {
    /// The spray could not be made ([`spray::spray`]).
    Spray(BuildLeapfrogError),
    /// An arm, by the energy's sign ([`Tracer::energy_arm`](spray::Tracer::energy_arm)), holds
    /// fewer tracers than a knot's kernel averages, [`KERNEL_TRACERS`].
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
    /// The tangent and the normal; the binormal is their cross product, recomputed when read, to
    /// the bits it was built with (P10.T6.c: 24 bytes a knot).
    frame: [[f64; 3]; 2],
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
    pub fn frame(&self) -> [[f64; 3]; 3] {
        let [tangent, normal] = self.frame;
        [tangent, normal, cross(tangent, normal)]
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
    /// Each segment's reach, from knot `k` to knot `k + 1` (module documentation, "Finding the
    /// segments").
    reaches_ly: Vec<f64>,
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

    /// The tracers it was reduced from, those of its energy arm
    /// ([`Tracer::energy_arm`](spray::Tracer::energy_arm)).
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
    /// documentation's. Tracers of one arm and equal ψ keep their order, so the table is a pure
    /// function of the seed, the mass, the stream's number and the list.
    ///
    /// # Errors
    ///
    /// [`BuildTubeTableError::TooFewTracers`] if either arm, by the energy's sign, holds fewer than
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
        let (mut trailing, mut leading): (Vec<&Tracer>, Vec<&Tracer>) = tracers
            .iter()
            .partition(|t| t.energy_arm() == Arm::Trailing);
        for arm in [&mut trailing, &mut leading] {
            // Stable, so tracers of equal ψ keep their index order. A tracer's ψ is finite
            // (`Spray::tracer`), so `total_cmp` reads no NaN's sign.
            arm.sort_by(|a, b| a.psi().value().total_cmp(&b.psi().value()));
        }
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
            count(Arm::Trailing, &trailing)?,
            count(Arm::Leading, &leading)?,
        ];
        let reduced = [Reduced::new(&trailing), Reduced::new(&leading)];
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

    /// The gap noise's span of ψ: the least and the greatest of the knots' ψ, each arm's knots
    /// being in order of ψ (module documentation, "Gaps").
    #[must_use]
    fn psi_span(&self) -> [Radians; 2] {
        let ends = |arm: &TubeArm| {
            let first = arm.knots.first().expect("an arm holds KNOTS_PER_ARM knots");
            let last = arm.knots.last().expect("an arm holds KNOTS_PER_ARM knots");
            [first.psi.value(), last.psi.value()]
        };
        let ([t0, t1], [l0, l1]) = (ends(&self.arms[0]), ends(&self.arms[1]));
        // `<` rather than `f64::min` or `max`, whose choice between zeros of either sign is not
        // portable (the sim-determinism skill's "Hazards across targets").
        let low = if l0 < t0 { l0 } else { t0 };
        let high = if t1 < l1 { l1 } else { t1 };
        [Radians::new(low), Radians::new(high)]
    }

    /// `exp(σ g − σ² ÷ 2)` at along cell `cell`'s middle, before it is divided by its mean over
    /// the cells, with node normals read from `nodes` (module documentation, "Gaps").
    #[must_use]
    fn undivided_noise(&self, nodes: &mut Stream, cell: u32) -> f64 {
        let cells = f64::from(self.along_cells);
        let psi = self.psi((f64::from(cell) + 0.5) / cells).value();
        // The trailing arm's tip lies at the least ψ and the leading arm's at the greatest where
        // every tracer's ψ has its arm's sign; in any case the span is positive.
        let [low, high] = self.psi_span().map(Radians::value);
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

    /// The arm of the energy's sign (module documentation, "The arms").
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
        let [_, normal, binormal] = interpolated_frame(&from.frame(), &to.frame(), f);
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

    /// The local frame at along coordinate `u`, `[tangent, normal, binormal]`: each vector linear
    /// between knots and made a unit vector again, as [`locate`](Self::locate) reads it.
    ///
    /// `u` is clamped to `[0, 1]`.
    ///
    /// # Panics
    ///
    /// If `u` is not finite.
    #[must_use]
    pub fn frame_at(&self, u: f64) -> [[f64; 3]; 3] {
        assert!(u.is_finite(), "a tube's along coordinate is finite: {u}");
        let (from, to, f) = self.segment(u);
        interpolated_frame(&from.frame(), &to.frame(), f)
    }

    /// The tracers' velocity dispersions at along coordinate `u`, rms along the local frame's
    /// tangent, normal and binormal ([`frame_at`](Self::frame_at)), linear between knots.
    ///
    /// `u` is clamped to `[0, 1]`.
    ///
    /// # Panics
    ///
    /// If `u` is not finite.
    #[must_use]
    pub fn dispersions_at(&self, u: f64) -> [MetresPerSecond; 3] {
        assert!(u.is_finite(), "a tube's along coordinate is finite: {u}");
        let (from, to, f) = self.segment(u);
        core::array::from_fn(|axis| {
            let (p, q) = (from.dispersions[axis].value(), to.dispersions[axis].value());
            MetresPerSecond::new(p + f * (q - p))
        })
    }

    /// ψ at along coordinate `u`, the inverse of `Λ(ψ) ÷ Λ_total` within each arm, linear between
    /// knots: in a cloud ψ may step back between the arms' roots (module documentation, "The
    /// reduction").
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

    /// A bound on the speed of every member placed in the tube, km/s, galactic (P10.T9).
    ///
    /// It is the knots' greatest mean speed plus [`MEMBER_SPEED_SIGMAS`] times their greatest
    /// velocity dispersion, rounded up by 1 + 10⁻⁹. A member's velocity is the mean at its along
    /// coordinate plus its three normals times the dispersions there along the local frame
    /// ([`members`](super::members)), each of the mean and the dispersions linear between knots, so
    /// at most their knots' greatest; what the range query pads a stream's cells by (P09.T23.b's
    /// rule).
    #[must_use]
    pub fn member_speed_bound(&self) -> KilometresPerSecond {
        let greater = |a: f64, b: f64| if b > a { b } else { a };
        let (speed, sigma) = self.knots().fold((0.0, 0.0), |(speed, sigma), k| {
            let v = k.velocity.metres_per_second();
            let s = k
                .dispersions
                .iter()
                .fold(sigma, |most, d| greater(most, d.value()));
            (greater(speed, dot(v, v).sqrt()), s)
        });
        let bound = speed + MEMBER_SPEED_SIGMAS * sigma;
        KilometresPerSecond::from(MetresPerSecond::new(bound * (1.0 + SPEED_BOUND_MARGIN)))
    }

    /// The bytes the table holds, its knots' and its segments' reaches included: what a caller's
    /// cache counts against its budget (plan 10, "Provides"). A `Vec`'s pointer and lengths are
    /// `usize`s, so the count differs between 32- and 64-bit targets; it is never output.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        let arms: usize = self
            .arms
            .iter()
            .map(|arm| {
                arm.knots.capacity() * size_of::<TubeKnot>()
                    + arm.reaches_ly.capacity() * size_of::<f64>()
            })
            .sum();
        size_of::<Self>() + arms
    }

    /// Every cell of the tube's own coordinates that may hold a member within `radius` of
    /// `centre`, in the order of [`TubeCell`]'s `Ord`: along, then across the normal, then
    /// across the binormal.
    ///
    /// A member is the point [`locate`](Self::locate) gives for its coordinates, within the cut.
    /// The answer is conservative (module documentation, "Cells"): every cell that holds such a
    /// point within `radius` is yielded, and a cell near the sphere that holds none may be. The
    /// knot segments to test are found by a scan (module documentation, "Finding the segments").
    ///
    /// # Panics
    ///
    /// If `radius` is negative or not finite.
    pub fn cells_touching(
        &self,
        centre: &GalacticPosition,
        radius: LightYears,
    ) -> impl Iterator<Item = TubeCell> + use<> {
        let radius_ly = radius.value();
        assert!(
            radius_ly.is_finite() && radius_ly >= 0.0,
            "a sphere's radius is finite and not negative: {radius_ly}"
        );
        let (cells, total) = (f64::from(self.along_cells), self.mass.value());
        let mut rows: BTreeMap<u32, [u64; ACROSS]> = BTreeMap::new();
        for segment in self.segments_near(centre, radius_ly) {
            let [from, to] = segment.knots;
            // The along cells whose share of the mass meets the segment's, one more at each end
            // where the segment ends on a cell's edge, since a member's `u Λ_total` rounds.
            let (low, high) = (from.cumulative_mass.value(), to.cumulative_mass.value());
            let fraction =
                |boundary: u32| (f64::from(boundary) / cells * total - low) / (high - low);
            let first = along_index(low / total * cells - EDGE, self.along_cells);
            let last = along_index(high / total * cells + EDGE, self.along_cells);
            for along in first..=last {
                let stretch = [
                    (fraction(along) - EDGE).clamp(0.0, 1.0),
                    (fraction(along + 1) + EDGE).clamp(0.0, 1.0),
                ];
                let mut bits = [0_u64; ACROSS];
                segment.mark(&mut bits, stretch);
                if bits.iter().any(|&row| row != 0) {
                    let row = rows.entry(along).or_insert([0; ACROSS]);
                    for (marked, new) in row.iter_mut().zip(bits) {
                        *marked |= new;
                    }
                }
            }
        }
        rows.into_iter().flat_map(|(along, bits)| {
            (0..ACROSS_CELLS).flat_map(move |a| {
                let row = bits[usize::from(a)];
                (0..ACROSS_CELLS)
                    .filter(move |&b| (row >> b) & 1 == 1)
                    .map(move |b| TubeCell {
                        along,
                        across: [a, b],
                    })
            })
        })
    }

    /// The knot segments whose reach about their centre lines meets the ball of `radius_ly` about
    /// `centre`, in the order of the along coordinate, each about the ball (module documentation,
    /// "Finding the segments").
    fn segments_near<'a>(
        &'a self,
        centre: &GalacticPosition,
        radius_ly: f64,
    ) -> impl Iterator<Item = NearSegment<'a>> + use<'a> {
        let (c, centre) = (centre.to_light_years_f64(), *centre);
        self.arms.iter().flat_map(move |arm| {
            let first = arm.knots[0].centre.to_light_years_f64();
            arm.knots
                .windows(2)
                .zip(&arm.reaches_ly)
                .scan(first, |p, (pair, &reach_ly)| {
                    let q = pair[1].centre.to_light_years_f64();
                    Some((pair, reach_ly, core::mem::replace(p, q), q))
                })
                .filter_map(move |(pair, reach_ly, p, q)| {
                    let pad = radius_ly + reach_ly + MARGIN_LY;
                    let boxed = (0..3).all(|axis| {
                        c[axis] >= p[axis].min(q[axis]) - pad
                            && c[axis] <= p[axis].max(q[axis]) + pad
                    });
                    if !boxed {
                        return None;
                    }
                    let near = NearSegment::new([&pair[0], &pair[1]], &centre, radius_ly);
                    (distance_to_segment(near.centre_m, near.chord_m)
                        <= near.radius_m + reach_ly * METRES_PER_LIGHT_YEAR)
                        .then_some(near)
                })
        })
    }
}

/// [`ACROSS_CELLS`] as an index.
const ACROSS: usize = ACROSS_CELLS as usize;

/// What [`TubeTable::cells_touching`] widens a fraction by at a cell's or a segment's edge, so
/// that a member whose `u Λ_total` rounds across the edge is still counted.
const EDGE: f64 = 1e-9;

/// One cell of a tube's own coordinates (plan 10, Design note 7): an along cell, one of the
/// tube's [`along_cells`](TubeTable::along_cells) of equal mass, and an across cell along each of
/// the frame's normal and binormal, one of [`ACROSS_CELLS`] spanning ±[`CUT_WIDTHS`] local widths.
///
/// A stream member's ID carries the three (plan 01's `StreamMemberId`: along in 14 bits, across
/// in 6 + 6). [`TubeTable::cells_touching`] yields them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TubeCell {
    along: u32,
    across: [u8; 2],
}

impl TubeCell {
    /// The along cell: it holds the along coordinates `[along, along + 1) ÷ cells` of a tube of
    /// `cells` along cells.
    #[must_use]
    pub const fn along(self) -> u32 {
        self.along
    }

    /// The across cells along the normal and the binormal, each below [`ACROSS_CELLS`].
    #[must_use]
    pub const fn across(self) -> [u8; 2] {
        self.across
    }

    /// The across coordinates the cell spans along the normal and the binormal, in local widths:
    /// `[low, high]` each, an eighth of a width across, from −[`CUT_WIDTHS`] for cell 0 to
    /// +[`CUT_WIDTHS`] for the last.
    #[must_use]
    pub fn across_widths(self) -> [[f64; 2]; 2] {
        self.across.map(across_range)
    }
}

impl From<StreamMemberId> for TubeCell {
    /// The cell a member's ID names. The ID's fields hold 14 and 6 + 6 bits, so the across cells
    /// are below [`ACROSS_CELLS`]; whether the along cell is below its tube's
    /// [`along_cells`](TubeTable::along_cells) is the tube's to check.
    fn from(id: StreamMemberId) -> Self {
        Self {
            along: u32::from(id.along()),
            across: id.across(),
        }
    }
}

/// The across coordinates across cell `j` spans, in widths: `[−4 + j ÷ 8, −4 + (j + 1) ÷ 8]` at the
/// generator's cut and cells, each exact in binary.
#[must_use]
fn across_range(j: u8) -> [f64; 2] {
    let step = 2.0 * CUT_WIDTHS / f64::from(ACROSS_CELLS);
    [
        -CUT_WIDTHS + f64::from(j) * step,
        -CUT_WIDTHS + f64::from(j + 1) * step,
    ]
}

/// The along cell holding the fractional cell index `x`, held to the tube's `cells`.
#[must_use]
fn along_index(x: f64, cells: u32) -> u32 {
    debug_assert!(x.is_finite(), "a share of a finite mass is finite: {x}");
    let floor = x.floor().clamp(0.0, f64::from(cells - 1));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "floor is a whole number clamped to [0, cells − 1], cells at most 16,384"
    )]
    let cell = floor as u32;
    cell
}

/// The farthest a point of the segment `[from, to]` within the cut lies from its centre line, ly:
/// [`CUT_WIDTHS`] times `√(w_n² + w_b² + 2 w_n w_b c)`, each width the larger at the segment's
/// ends and `c` a bound on the cosine between the normal and the binormal interpolated between
/// them (module documentation, "Finding the segments").
#[must_use]
fn segment_reach_ly(from: &TubeKnot, to: &TubeKnot) -> f64 {
    let [n, b] = [0, 1].map(|axis| from.widths[axis].value().max(to.widths[axis].value()));
    let ([_, n0, b0], [_, n1, b1]) = (from.frame(), to.frame());
    // Between orthonormal ends, (1 − f) n₀ + f n₁ and the like for b have the dot product
    // f (1 − f) (n₀·b₁ + n₁·b₀), and lengths of at least half their ends' sums, at f = ½.
    let sums = [
        length(core::array::from_fn(|i| n0[i] + n1[i])),
        length(core::array::from_fn(|i| b0[i] + b1[i])),
    ];
    let skew = if sums[0] > 0.0 && sums[1] > 0.0 {
        ((dot(n0, b1) + dot(n1, b0)).abs() / (sums[0] * sums[1]) + 1e-12).min(1.0)
    } else {
        1.0
    };
    CUT_WIDTHS * (n * n + b * b + 2.0 * n * b * skew).sqrt()
}

/// The distance from the point `q` to the segment from the origin to `d`.
#[must_use]
fn distance_to_segment(q: [f64; 3], d: [f64; 3]) -> f64 {
    let square = dot(d, d);
    let t = if square > 0.0 {
        (dot(q, d) / square).clamp(0.0, 1.0)
    } else {
        0.0
    };
    length(core::array::from_fn(|i| q[i] - t * d[i]))
}

/// The length of `v`.
#[must_use]
fn length(v: [f64; 3]) -> f64 {
    dot(v, v).sqrt()
}

/// One knot segment about a sphere's centre, for [`TubeTable::cells_touching`]'s test of the
/// across cells (module documentation, "Cells"), from the segment's first knot.
struct NearSegment<'a> {
    knots: [&'a TubeKnot; 2],
    frames: [[[f64; 3]; 3]; 2],
    /// The sphere's centre.
    centre_m: [f64; 3],
    /// The centre line from the first knot to the second.
    chord_m: [f64; 3],
    /// The sphere's radius with the margin.
    radius_m: f64,
}

impl<'a> NearSegment<'a> {
    /// The segment `knots` about the sphere of `radius_ly` about `centre`.
    #[must_use]
    fn new(knots: [&'a TubeKnot; 2], centre: &GalacticPosition, radius_ly: f64) -> Self {
        let [from, to] = knots;
        let centre_m = from.centre.displacement_to(centre).metres();
        let chord_m = from.centre.displacement_to(&to.centre).metres();
        let scale = length(centre_m) + length(chord_m);
        Self {
            knots,
            frames: [from.frame(), to.frame()],
            centre_m,
            chord_m,
            radius_m: (radius_ly + MARGIN_LY) * METRES_PER_LIGHT_YEAR + 1e-12 * scale,
        }
    }

    /// Marks in `bits`, row `a` and bit `b`, every across cell `[a, b]` that may hold a point of
    /// the segment within the sphere at fractions `[f₀, f₁]` of the way along it: the stretch cut
    /// into pieces over which the frame turns by at most [`PIECE_CHORD`].
    fn mark(&self, bits: &mut [u64; ACROSS], [f0, f1]: [f64; 2]) {
        let ([_, n0, b0], [_, n1, b1]) = (self.frame(f0), self.frame(f1));
        let turn = distance(n0, n1).max(distance(b0, b1));
        debug_assert!(
            turn.is_finite(),
            "unit vectors lie a finite chord apart: {turn}"
        );
        let pieces = (turn / PIECE_CHORD)
            .ceil()
            .clamp(1.0, f64::from(MAX_PIECES));
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "pieces is a whole number clamped to [1, MAX_PIECES]; a chord is finite"
        )]
        let pieces = pieces as u32;
        let at = |k: u32| f0 + (f1 - f0) * (f64::from(k) / f64::from(pieces));
        for k in 0..pieces {
            self.mark_piece(bits, [at(k), at(k + 1)]);
        }
    }

    /// [`mark`](Self::mark) over one piece, `[g₀, g₁]`.
    fn mark_piece(&self, bits: &mut [u64; ACROSS], [g0, g1]: [f64; 2]) {
        let [from, to] = self.knots;
        let ([_, n0, b0], [_, n1, b1]) = (self.frame(g0), self.frame(g1));
        let (middle, half) = (f64::midpoint(g0, g1), 0.5 * (g1 - g0));
        let [_, normal, binormal] = self.frame(middle);
        let width = |axis: usize, f: f64| {
            let (p, q) = (from.widths[axis].value(), to.widths[axis].value());
            (p + f * (q - p)) * METRES_PER_LIGHT_YEAR
        };
        // How far a point at across coordinate `c` moves, per unit of |c|, from where the frame
        // and the widths at the middle put it: the width's change over half the piece, and the
        // larger width times the chord between the unit vectors at the piece's ends, which bounds
        // the chord between any two of them on the arc they sweep.
        let drift = |axis: usize, ends: [[f64; 3]; 2]| {
            (to.widths[axis].value() - from.widths[axis].value()).abs()
                * METRES_PER_LIGHT_YEAR
                * half
                + width(axis, g0).max(width(axis, g1)) * distance(ends[0], ends[1])
        };
        let (drift_n, drift_b) = (drift(0, [n0, n1]), drift(1, [b0, b1]));
        let (w_n, w_b) = (width(0, middle), width(1, middle));
        // An orthonormal basis from the middle's frame: the normal, the binormal made
        // perpendicular to it, and their cross product, nearly the tangent.
        let along_normal = dot(binormal, normal);
        let Some(e2) = unit(core::array::from_fn(|i| {
            binormal[i] - along_normal * normal[i]
        })) else {
            *bits = [u64::MAX; ACROSS];
            return;
        };
        let (e1, e3) = (normal, cross(normal, e2));
        let (c1, c2, c3) = (along_normal, dot(binormal, e2), dot(binormal, e3));
        // The centre line at the middle, from the sphere's centre, in the basis; and how far each
        // coordinate moves with the centre line over half the piece.
        let offset: [f64; 3] =
            core::array::from_fn(|i| middle * self.chord_m[i] - self.centre_m[i]);
        let coordinate = |e: [f64; 3]| (dot(offset, e), dot(self.chord_m, e).abs() * half);
        let ((x1, s1), (x2, s2), (x3, s3)) = (coordinate(e1), coordinate(e2), coordinate(e3));
        // Along the tube no cell of the piece comes within the widest limit, any a and b's.
        let nearest = (x3.abs() - s3 - CUT_WIDTHS * w_b * c3.abs()).max(0.0);
        if nearest > self.radius_m + CUT_WIDTHS * (drift_n + drift_b) {
            return;
        }
        for b in 0..ACROSS_CELLS {
            let [low, high] = across_range(b);
            let farthest = low.abs().max(high.abs());
            let along = (x3.abs() - s3 - farthest * w_b * c3.abs()).max(0.0);
            // The binormal's coordinate over the row and the piece; c2 is not negative.
            let (lowest, highest) = (x2 - s2 + low * w_b * c2, x2 + s2 + high * w_b * c2);
            let gap = if lowest > 0.0 {
                lowest
            } else if highest < 0.0 {
                -highest
            } else {
                0.0
            };
            let skew = [low * w_b * c1, high * w_b * c1];
            // At a signed zero either zero is returned; the bounds below differ from it by the
            // finite `allowed`, so the cell indices do not depend on which.
            let (skew_low, skew_high) = (skew[0].min(skew[1]), skew[0].max(skew[1]));
            // The normal's coordinate is x1 + a w_n + b w_b c1, within what the radius, widened by
            // the drift of the farthest a and b, leaves of it.
            let span = |a: f64| {
                let limit = self.radius_m + a * drift_n + farthest * drift_b;
                let left = limit * limit - along * along - gap * gap;
                if left < 0.0 {
                    return None;
                }
                let allowed = left.sqrt();
                across_span(
                    -allowed - x1 - s1 - skew_high,
                    allowed - x1 + s1 - skew_low,
                    w_n,
                )
            };
            // First for any a within the cut, then again for the farthest a that allows.
            let Some([first, last]) = span(CUT_WIDTHS) else {
                continue;
            };
            let a = across_range(cell_u8(first))[0]
                .abs()
                .max(across_range(cell_u8(last))[1].abs());
            if let Some([first, last]) = span(a) {
                for row in &mut bits[first..=last] {
                    *row |= 1 << b;
                }
            }
        }
    }

    /// The frame a fraction `f` of the way along the segment, as [`TubeTable::locate`] reads it.
    #[must_use]
    fn frame(&self, f: f64) -> [[f64; 3]; 3] {
        interpolated_frame(&self.frames[0], &self.frames[1], f)
    }
}

/// The across cells, first and last, whose coordinates `c` in widths, within the cut, put
/// `c × width` in `[low, high]`, or `None` if none does.
#[must_use]
fn across_span(low: f64, high: f64, width: f64) -> Option<[usize; 2]> {
    if low > high {
        return None;
    }
    let (from, to) = if width > 0.0 {
        (low / width, high / width)
    } else if low <= 0.0 && high >= 0.0 {
        (-CUT_WIDTHS, CUT_WIDTHS)
    } else {
        return None;
    };
    if to < -CUT_WIDTHS || from > CUT_WIDTHS {
        return None;
    }
    let cell = |c: f64| {
        debug_assert!(!c.is_nan(), "a bound of finite coordinates is not NaN");
        let scaled = ((c.clamp(-CUT_WIDTHS, CUT_WIDTHS) + CUT_WIDTHS) * f64::from(ACROSS_CELLS)
            / (2.0 * CUT_WIDTHS))
            .floor()
            .min(f64::from(ACROSS_CELLS - 1));
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "scaled is a whole number in [0, ACROSS_CELLS − 1]"
        )]
        let cell = scaled as usize;
        cell
    };
    Some([cell(from), cell(to)])
}

/// An across cell's index as the `u8` it is, below [`ACROSS_CELLS`].
#[must_use]
fn cell_u8(cell: usize) -> u8 {
    u8::try_from(cell).expect("an across cell is below ACROSS_CELLS, 64")
}

/// The distance between two points.
#[must_use]
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    length(core::array::from_fn(|i| a[i] - b[i]))
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
        // Each knot's kernel, its tracers' indices and weights, gathered once for its five passes.
        let mut near: Vec<(usize, f64)> = Vec::with_capacity(index(KERNEL_TRACERS));
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
                near.clear();
                near.extend(kernel(rank, tracers));
                // The kernel's moments in rank, for its weights corrected to first order.
                let [mut s0, mut s1, mut s2] = [0.0; 3];
                for &(i, k) in &near {
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
                let fit = LocalLinear {
                    moments: [s0, s1, s2],
                    determinant,
                };
                let mut position_m = [0.0; 3];
                let mut slope_m = [0.0; 3];
                let mut velocity_m_s = [0.0; 3];
                for &(i, k) in &near {
                    let (level, rise) = fit.weights(k, index_f64(i) - rank);
                    let t = tracers[i];
                    let (x, v) = (t.position().metres(), t.velocity().metres_per_second());
                    for axis in 0..3 {
                        position_m[axis] += level * x[axis];
                        slope_m[axis] += rise * x[axis];
                        velocity_m_s[axis] += level * v[axis];
                    }
                }
                // The same fit in the orbital plane, where it is well posed: the Cartesian one
                // only sets the plane's first axis there (module documentation, "The reduction").
                match polar_fit(rank, tracers, &near, &fit, position_m) {
                    Some(polar) => [position_m, slope_m, velocity_m_s] = polar,
                    None => count_cartesian_knot(),
                }
                // ψ at the rank itself, between the two tracers about it: the inverse of the
                // tracers' own Λ(ψ), which keeps the knots in their arm's order of ψ.
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
                    frame: [frame[0], frame[1]],
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
            .collect::<Vec<_>>();
        let reaches_ly = knots
            .windows(2)
            .map(|pair| segment_reach_ly(&pair[0], &pair[1]))
            .collect();
        TubeArm {
            knots,
            reaches_ly,
            tracers,
            mass: SolarMasses::new(self.mass),
        }
    }
}

/// The kernel's local-linear weights about a knot's rank (module documentation, "The reduction"):
/// its moments in rank, `s0`, `s1` and `s2` of the tracers' kernel weights times 1, `d` and `d²`,
/// and their determinant `s0 s2 − s1²`, positive.
#[derive(Debug, Clone, Copy)]
struct LocalLinear {
    moments: [f64; 3],
    determinant: f64,
}

impl LocalLinear {
    /// The shares, of the fitted line's level at the knot and of its slope in rank, of the kernel
    /// tracer of kernel weight `k` at `d` ranks from the knot.
    #[must_use]
    fn weights(&self, k: f64, d: f64) -> (f64, f64) {
        let [s0, s1, s2] = self.moments;
        (
            k * (s2 - s1 * d) / self.determinant,
            k * (s0 * d - s1) / self.determinant,
        )
    }
}

/// The share of the kernel's mean radius in its orbital plane under which a kernel tracer makes
/// the plane's angle ill-posed ([`polar_fit`]): a tenth (research-leans-10's pilot, as main's
/// brief for P10.T6.a's tips sets it).
const POLAR_RADIUS_FLOOR: f64 = 0.1;

#[cfg(test)]
thread_local! {
    /// The knots whose centre line kept the Cartesian fit, where [`polar_fit`] was ill-posed,
    /// counted on this thread: a test's counter, never a process-wide one, since tests run on
    /// threads of one process.
    static CARTESIAN_KNOTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Counts a knot that kept the Cartesian fit on this thread's counter ([`cartesian_knots`]) in a
/// test build, and does nothing otherwise.
fn count_cartesian_knot() {
    #[cfg(test)]
    CARTESIAN_KNOTS.with(|count| count.set(count.get() + 1));
}

/// The knots this thread's reductions have kept the Cartesian fit for since it started, where the
/// polar fit was ill-posed (module documentation, "The reduction"): a count for the tests, which
/// no output reads.
#[cfg(test)]
#[must_use]
pub(crate) fn cartesian_knots() -> u64 {
    CARTESIAN_KNOTS.with(std::cell::Cell::get)
}

/// A knot's centre line, its slope in rank (both m) and its mean velocity (m/s), galactic, fitted
/// by `fit`'s weights over `near`, the kernel about `rank` ([`kernel`]'s indices into `tracers`
/// and weights), in the kernel's orbital plane about the galactic centre (module documentation,
/// "The reduction"); `cartesian_m` is the same fit's position in x, y and z, which sets the
/// plane's first axis.
///
/// The plane's pole is the direction of the kernel's weighted angular momentum, its first axis
/// `cartesian_m` projected on it, and its second the pole crossed with the first. Each tracer
/// gives its radius `R`, angle `φ` and height `z` in that basis, and its velocity's components
/// along its own radial and azimuthal directions and the pole; the fit's levels and slopes of
/// each map back at the fitted `φ`, the slope by the chain rule.
///
/// `None` where the plane's angle is ill-posed, so that the Cartesian fit stands: a pole of no
/// direction, a first axis of none, a kernel tracer more than three-eighths of a turn from that
/// axis, within an eighth of a turn of `atan2`'s cut at ±π, or one nearer the pole's line than
/// [`POLAR_RADIUS_FLOOR`] of the kernel's mean radius. A quarter turn from the axis, the brief's
/// first limit, refused `pal5_like`'s trailing tips in 3 of 8 seeds, whose kernels reach 114°
/// from it, and left 6, 54 and 11 of its members outside its shell there (lean, 2026-10-31).
#[must_use]
fn polar_fit(
    rank: f64,
    tracers: &[&Tracer],
    near: &[(usize, f64)],
    fit: &LocalLinear,
    cartesian_m: [f64; 3],
) -> Option<[[f64; 3]; 3]> {
    let mut momentum = [0.0; 3];
    for &(i, k) in near {
        let tracer = tracers[i];
        let own = cross(
            tracer.position().metres(),
            tracer.velocity().metres_per_second(),
        );
        for axis in 0..3 {
            momentum[axis] += k * own[axis];
        }
    }
    let pole = unit(momentum)?;
    let height = dot(cartesian_m, pole);
    let first = unit(core::array::from_fn(|axis| {
        cartesian_m[axis] - height * pole[axis]
    }))?;
    let second = cross(pole, first);
    // A vector's components along the plane's first and second axes.
    let in_plane = |vector: [f64; 3]| (dot(vector, first), dot(vector, second));
    // The kernel's mean radius, and each tracer within three-eighths of a turn of the first axis
    // (|φ| ≤ 3π ÷ 4, where c₁ ≥ −|c₂|), so that no two lie across `atan2`'s cut at ±π.
    let (mut weighed, mut total) = (0.0, 0.0);
    for &(i, k) in near {
        let (c1, c2) = in_plane(tracers[i].position().metres());
        if c1 < -c2.abs() {
            return None;
        }
        weighed += k * (c1 * c1 + c2 * c2).sqrt();
        total += k;
    }
    let floor = POLAR_RADIUS_FLOOR * weighed / total;
    // The fitted line's levels at the knot and slopes in rank of R, φ and z, and the levels of the
    // velocity's radial, azimuthal and vertical components.
    let [mut radius_level, mut angle_level, mut height_level] = [0.0; 3];
    let [mut radius_rise, mut angle_rise, mut height_rise] = [0.0; 3];
    let mut velocity_level = [0.0; 3];
    for &(i, k) in near {
        let (level, rise) = fit.weights(k, index_f64(i) - rank);
        let tracer = tracers[i];
        let (position, velocity) = (
            tracer.position().metres(),
            tracer.velocity().metres_per_second(),
        );
        let (c1, c2) = in_plane(position);
        let radius = (c1 * c1 + c2 * c2).sqrt();
        if radius < floor {
            return None;
        }
        let angle = math::atan2(c2, c1);
        let up = dot(position, pole);
        radius_level += level * radius;
        angle_level += level * angle;
        height_level += level * up;
        radius_rise += rise * radius;
        angle_rise += rise * angle;
        height_rise += rise * up;
        // Along the tracer's own radial direction, (c₁, c₂) ÷ R in the plane, and its azimuthal
        // one, (−c₂, c₁) ÷ R: no sine or cosine of its angle (the bench, P10.T6.a's tips).
        let (v1, v2) = in_plane(velocity);
        velocity_level[0] += level * (v1 * c1 + v2 * c2) / radius;
        velocity_level[1] += level * (v2 * c1 - v1 * c2) / radius;
        velocity_level[2] += level * dot(velocity, pole);
    }
    let [radial, around] = plane_axes(first, second, angle_level);
    let [v_radial, v_around, v_up] = velocity_level;
    Some([
        core::array::from_fn(|axis| radius_level * radial[axis] + height_level * pole[axis]),
        core::array::from_fn(|axis| {
            radius_rise * radial[axis]
                + radius_level * angle_rise * around[axis]
                + height_rise * pole[axis]
        }),
        core::array::from_fn(|axis| {
            v_radial * radial[axis] + v_around * around[axis] + v_up * pole[axis]
        }),
    ])
}

/// The radial and azimuthal unit vectors at angle `phi` (rad) in the plane of the orthonormal
/// `first` and `second` axes.
#[must_use]
fn plane_axes(first: [f64; 3], second: [f64; 3], phi: f64) -> [[f64; 3]; 2] {
    let (sin, cos) = math::sin_cos(phi);
    [
        core::array::from_fn(|axis| cos * first[axis] + sin * second[axis]),
        core::array::from_fn(|axis| cos * second[axis] - sin * first[axis]),
    ]
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
/// The server's `ListedTubes`, one galaxy's view of its byte-bounded `TubeTableCache` (P10.T10.b),
/// builds a table on a miss inside the job that asked; eviction is safe, because a table is a pure
/// function of the galaxy and the stream number and is rebuilt to the same bits. An implementation
/// must hand out the list of the galaxy it is asked about, and the table of `stream` in it.
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

    /// The models of the list ([`GlobalListModels`]), if the caller keeps them for every query of
    /// the galaxy, made for that galaxy and this list: `None`, the default, and each
    /// [`GlobalListSource`](super::GlobalListSource) makes its own (P09.T37's bench).
    fn models(&self) -> Option<&GlobalListModels> {
        None
    }
}

#[cfg(test)]
mod tests;
