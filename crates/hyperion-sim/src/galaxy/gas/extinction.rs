//! The extinction line integral: visual extinction, hydrogen columns and per-band extinction
//! between any two points (plan 07, P07.T8 and Design notes 14–16).
//!
//! # The integral
//!
//! Along a segment the dust-bearing density is `n_disc × F × ζ` (Design notes 10 and 13) and one
//! magnitude of visual extinction is [`HYDROGEN_COLUMN_PER_MAG`] of it per cm². The segment is
//! clipped to the slab `|z| ≤ 8 h_w`, outside which nothing but the dust-free corona remains, whose
//! hydrogen column is added in closed form over the whole segment. Inside the slab it is marched
//! with two-point Gauss–Legendre steps.
//!
//! The segment is first cut where its smooth scale changes — into and out of the sphere of `3 R_c`
//! about the centre, where the molecular disc's height `h_c` is the scale; into and out of the slab
//! `|z| ≤ 5 h_n`, where the neutral layer's height or the lane width is; the warm layer's height
//! `h_w ÷ 4` elsewhere — and at every hole's surface. Each piece takes equal steps no longer than
//! `min(Δ_noise, Δ_smooth)`, with `Δ_noise = 32 ly × 2^lod` and `Δ_smooth` half the piece's scale.
//! [`Quality::Full`] is `lod = 0`. [`Quality::Budget`] takes the smallest `lod` whose step count is
//! at most its budget and reads the noise at `AtLeast(2 Δ_noise)`, so that octaves the steps cannot
//! resolve are replaced by their mean rather than aliased; so that a budget can always be met,
//! `Δ_smooth` doubles with `lod` too, and if even one step per piece is over the budget the pieces
//! between holes are merged and marched as one. [`NoiseMode::Mean`] skips the noise entirely, on
//! the same steps. The result is a pure function of its arguments, quality included, so a caller
//! that needs one answer uses one quality (Design note 14).
//!
//! Every sample is formed as an integer light-year cell plus an offset, from the segment's first
//! point and its exact displacement, never as one `f64` across the galaxy, and the field reads its
//! radius and height once per sample.
//!
//! # Symmetry
//!
//! [`sightline`] orders its end points canonically, by cell and then by offset, before it marches,
//! so `A(a, b)` and `A(b, a)` are the same bits: two-way visibility is exact (Design note 15).
//! [`horizon`] marches from its origin and is an instrument's range, not an identity.
//!
//! # Modifiers
//!
//! A hole replaces the field inside its sphere by its interior density, with no dust: its pieces
//! take no steps and add `interior × length`. A cloud is a Plummer ball whose column along a line
//! is algebraic — with `b` the line's impact parameter, `c² = a² + b²` and `s` the distance along
//! the line from the closest approach, `n_c a⁵ [s (3c² + 2s²) ÷ (3c⁴ (c² + s²)^(3/2))]` between the
//! segment's two values of `s` — so clouds cost no steps (Design note 16). A wall, a superbubble's
//! uniform shell of swept-up gas (plan 09, P09.T5), replaces the field where no hole covers it: the
//! segment is cut at both its surfaces too, and a piece inside a wall and no hole takes no steps
//! and adds the densest wall's density and dust times its length. The clouds' columns are added in
//! a canonical order, so the modifiers' order is immaterial; but a cloud adds a little at any
//! distance, so two-way visibility with modifiers needs their source to give the same set for `(a,
//! b)` as for `(b, a)`. With no modifiers every result is bit-identical to one computed without
//! them.
//!
//! # The neutral column
//!
//! Columns use the parcel mean (ruling 103 of 2026-09-22): the phase split inside a parcel
//! ([`PhaseMix`](crate::galaxy::gas::phase::PhaseMix)) keeps its neutral mass neutral, so the
//! neutral column is the integral of `n_neutral + n_mol` in [`NoiseMode::Mean`] and of
//! `(n_neutral + n_mol) F` in [`NoiseMode::Realised`], whose expectation over seeds it is. No draw
//! and no classification is needed. Holes are hot and add none; clouds are neutral and add all of
//! theirs, and so do walls, which are the H I supershells about superbubbles (McClure-Griffiths et
//! al. 2002, ApJ 578, 176).

use std::num::NonZeroU32;

use crate::coords::{GalacticDisplacement, GalacticPosition, UnitVector};
use crate::galaxy::gas::CENTIMETRES_PER_LIGHT_YEAR;
use crate::galaxy::gas::ccm::{Band, HYDROGEN_COLUMN_PER_MAG};
use crate::galaxy::gas::field::{GasField, Site};
use crate::galaxy::gas::modifiers::{GasModifier, densest};
use crate::galaxy::gas::noise::{NoiseCache, SmoothingScale};
use crate::galaxy::gas::smooth::GasLayer;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{LightYears, Magnitudes, PerCm2};

/// The noise's step at `lod = 0`, ly: half the finest octave's 64 ly (Design note 14).
const NOISE_STEP_LY: f64 = 32.0;

/// The slab outside which only the corona remains, in warm scale heights: `|z| ≤ 8 h_w`.
const SLAB_WARM_HEIGHTS: f64 = 8.0;

/// The slab about the plane whose smooth scale is the neutral layer's, in neutral scale heights:
/// `|z| < 5 h_n`.
const PLANE_NEUTRAL_HEIGHTS: f64 = 5.0;

/// The sphere about the centre whose smooth scale is the molecular disc's height, in its scale
/// lengths: `r < 3 R_c`.
const CENTRE_MOLECULAR_LENGTHS: f64 = 3.0;

/// The largest `lod` a budget tries: steps of `32 × 2¹⁶` ly, two million light-years, longer than
/// any segment of the root cube, so that at it every piece takes one step.
const LOD_MAX: u32 = 16;

/// The farthest a [`horizon`] looks, ly: 2¹⁹, over twice the root cube's diagonal, beyond which
/// nothing lies.
const HORIZON_LIMIT_LY: f64 = 524_288.0;

/// The two-point Gauss–Legendre rule's nodes, `±1 ÷ √3`, on `[−1, 1]`; its weights are 1.
const GL2_NODE: f64 = 0.577_350_269_189_625_7;

/// Whether the noise is read along the line or only its mean (Design note 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NoiseMode {
    /// The mean field: no noise, the expectation of [`Realised`](Self::Realised) over seeds.
    Mean,
    /// The seed's own clumpy gas, the lattice noise read at every sample.
    Realised,
}

/// How finely the line is marched (Design note 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Quality {
    /// Steps of at most 32 ly, and less where a smooth scale needs it: the full noise.
    Full,
    /// At most this many steps: the steps double in length until they fit, and the noise is
    /// smoothed to twice their length.
    Budget(NonZeroU32),
}

/// The budget lines of sight are marched at where a line's cost must be bounded whatever its
/// length: `Budget(256)` (plan 07, P07.T10.c; `decision-gg-port-d0.md` §5).
///
/// Two readers share it, so that the value has one owner:
///
/// - **the server's `extinction` request**, whose lines to at most 64 targets are then a few
///   milliseconds of one worker (plan 07's Risks, "Cost on long-range charts");
/// - **the sky band's ray profiles**, which `sky::band` marches by [`profile`] (rendering plan
///   R06, R06.T9.a), whose light lies within 3% of the same rays marched at [`Quality::Full`]
///   (`sky::band`'s test
///   `above_the_disc_the_band_is_brighter_towards_the_plane_by_the_models_own_integral`).
///
/// The sky census's per-star lines take a budget of their own, for another purpose (rendering plan
/// R06, Design note 10). A reader that wants another budget takes a constant of its own, with its
/// reason, rather than change this one.
pub const SIGHTLINE_QUALITY: Quality = Quality::Budget(NonZeroU32::new(256).expect("256 is not 0"));

/// What a line of sight passes through: its visual extinction, its hydrogen columns and the steps
/// it took.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sightline {
    /// `∫ n_disc F ζ dl`, cm⁻²: the hydrogen column weighted by the dust-to-gas ratio.
    dust: f64,
    /// `∫ n dl` over every phase, the corona included, cm⁻².
    hydrogen: f64,
    /// The neutral hydrogen's column, cm⁻².
    neutral: f64,
    steps: u32,
}

impl Sightline {
    /// Nothing: a segment of no length.
    pub const ZERO: Self = Self {
        dust: 0.0,
        hydrogen: 0.0,
        neutral: 0.0,
        steps: 0,
    };

    /// The visual extinction `A_V`: the dust-weighted hydrogen column over
    /// [`HYDROGEN_COLUMN_PER_MAG`].
    #[must_use]
    pub fn a_v(&self) -> Magnitudes {
        Magnitudes::new(self.dust / HYDROGEN_COLUMN_PER_MAG)
    }

    /// The extinction in `band`: `A_V` times the band's Cardelli–Clayton–Mathis ratio, 0 in radio.
    #[must_use]
    pub fn in_band(&self, band: Band) -> Magnitudes {
        self.a_v() * band.ratio()
    }

    /// The reddening `E(B − V) = A_B − A_V`, the two taken at the bands' own wavelengths: about `A_V
    /// ÷ 3.1`.
    #[must_use]
    pub fn reddening(&self) -> Magnitudes {
        self.in_band(Band::B) - self.in_band(Band::V)
    }

    /// The hydrogen column of every phase, the corona and the modifiers' gas included: what an X-ray
    /// photoelectric absorption would read.
    #[must_use]
    pub fn hydrogen_column(&self) -> PerCm2 {
        PerCm2::new(self.hydrogen)
    }

    /// The neutral hydrogen's column: the 21 cm line's (module documentation, "The neutral
    /// column"). Never above [`hydrogen_column`](Self::hydrogen_column).
    #[must_use]
    pub fn neutral_hydrogen_column(&self) -> PerCm2 {
        PerCm2::new(self.neutral)
    }

    /// The Gauss–Legendre steps the integral took; 0 for a segment wholly outside the slab.
    #[must_use]
    pub fn steps(&self) -> u32 {
        self.steps
    }
}

/// What lies between `a` and `b` in `field`, with `modifiers` applied: visual extinction, hydrogen
/// columns and the steps taken (module documentation).
///
/// The same bits whichever way round the end points are given. `cache` changes what the call
/// costs, never what it returns; in [`NoiseMode::Mean`] it is not read.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::galaxy::gas::extinction::{NoiseMode, Quality, sightline};
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::Seed;
///
/// let galaxy = Galaxy::new(Seed::new(42));
/// let at = |ly| GalacticPosition::from_light_years(ly).expect("in the cube");
/// let (sun, star) = (at([0.0, 26_000.0, 0.0]), at([1_500.0, 26_000.0, 2_600.0]));
/// let mut cache = NoiseCache::with_capacity(4_096);
/// let line = sightline(galaxy.gas(), &sun, &star, NoiseMode::Mean, Quality::Full, &[], &mut cache);
/// // Out of the plane there is less than a magnitude, and far less still in the infrared.
/// assert!(line.a_v().value() > 0.0 && line.a_v().value() < 1.0);
/// assert!(line.in_band(Band::K).value() < 0.2 * line.a_v().value());
/// // Two-way visibility is exact.
/// let back = sightline(galaxy.gas(), &star, &sun, NoiseMode::Mean, Quality::Full, &[], &mut cache);
/// assert_eq!(line, back);
/// ```
#[must_use]
pub fn sightline(
    field: &GasField,
    a: &GalacticPosition,
    b: &GalacticPosition,
    mode: NoiseMode,
    quality: Quality,
    modifiers: &[GasModifier],
    cache: &mut NoiseCache,
) -> Sightline {
    let (a, b) = if precedes(b, a) { (b, a) } else { (a, b) };
    let line = Line::new(a, b);
    if line.length_ly.is_nan() || line.length_ly <= 0.0 {
        return Sightline::ZERO;
    }
    let plan = Plan::new(field, &line, modifiers, quality);
    let mut sums = Sums::default();
    for piece in &plan.pieces {
        let length_cm = (piece.t1 - piece.t0) * line.length_ly * CENTIMETRES_PER_LIGHT_YEAR;
        match piece.kind {
            PieceKind::Hole { interior } => sums.hydrogen += interior * length_cm,
            PieceKind::Wall { density, dust } => {
                sums.hydrogen += density * length_cm;
                sums.neutral += density * length_cm;
                sums.dust += dust * length_cm;
            }
            PieceKind::Outside => sums.hydrogen += field.smooth().corona_density() * length_cm,
            PieceKind::Stepped { .. } => {
                sums.hydrogen += field.smooth().corona_density() * length_cm;
            }
        }
    }
    let marcher = Marcher::new(field, &line, mode, plan.scale);
    for (piece, steps) in plan.stepped() {
        for step in 0..steps {
            marcher.step(piece, steps, step, cache, &mut sums);
        }
    }
    // Clouds in a canonical order, so that the columns add up to the same bits whatever order the
    // source listed them in: a Plummer ball adds a little everywhere, so the order is output.
    let mut clouds: Vec<Cloud> = modifiers
        .iter()
        .filter_map(|modifier| match *modifier {
            GasModifier::Cloud {
                centre,
                core_radius,
                central_density,
                dust_per_hydrogen,
            } => Some(Cloud {
                centre,
                core: core_radius.value(),
                central: central_density.value(),
                zeta: dust_per_hydrogen,
            }),
            GasModifier::Hole { .. } | GasModifier::Wall { .. } => None,
        })
        .collect();
    clouds.sort_by(Cloud::order);
    for cloud in &clouds {
        let column = plummer_column(&line, &cloud.centre, cloud.core, cloud.central);
        sums.hydrogen += column;
        sums.neutral += column;
        sums.dust += cloud.zeta * column;
    }
    // The neutral column is part of the whole, so it is held to it, by comparison rather than
    // `f64::min`: before ruling 103 a wholly neutral `Realised` sample carried the corona into the
    // neutral column step by step while the whole column took it in closed form, and the two could
    // differ in their last bit. Neither sum carries the corona that way now, but a line of clouds
    // alone still adds the same column to both.
    let neutral = if sums.neutral > sums.hydrogen {
        sums.hydrogen
    } else {
        sums.neutral
    };
    Sightline {
        dust: sums.dust,
        hydrogen: sums.hydrogen,
        neutral,
        steps: plan.steps,
    }
}

/// How far from `origin` along `direction` the extinction in `band` reaches `limit`, looking no
/// farther than `max_range`: the range of an instrument in that band (Design note 15).
///
/// The line is marched from the origin with the steps [`sightline`] would take over the whole range,
/// and the distance is interpolated linearly within the step that crosses the limit, so it may
/// differ in its last bits from a `sightline` to the point it names. It is `max_range` if the limit
/// is never reached — always in [`Band::Radio`] — and 0 for a limit that is not above 0 or a range
/// that is not positive. The range is capped at 2¹⁹ ly, beyond which nothing lies.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::{GalacticPosition, UnitVector};
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::galaxy::gas::extinction::{NoiseMode, Quality, horizon};
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::units::{LightYears, Magnitudes};
/// use hyperion_sim::Seed;
///
/// let galaxy = Galaxy::new(Seed::new(42));
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube");
/// let coreward = -UnitVector::Y;
/// let mut cache = NoiseCache::with_capacity(4_096);
/// let (limit, range) = (Magnitudes::new(5.0), LightYears::new(40_000.0));
/// let (mode, quality) = (NoiseMode::Mean, Quality::Full);
/// let mut reach = |band| {
///     horizon(galaxy.gas(), &sun, coreward, band, limit, range, mode, quality, &mut cache)
/// };
/// // The infrared sees much farther through the disc than the eye, and radio sees everything.
/// assert!(reach(Band::K) > reach(Band::V));
/// assert_eq!(reach(Band::Radio), LightYears::new(40_000.0));
/// ```
#[expect(
    clippy::too_many_arguments,
    reason = "plan 07's interface: a line, a band and its limit, a range, a mode, a quality and the \
              caller's cache"
)]
#[must_use]
pub fn horizon(
    field: &GasField,
    origin: &GalacticPosition,
    direction: UnitVector,
    band: Band,
    limit: Magnitudes,
    max_range: LightYears,
    mode: NoiseMode,
    quality: Quality,
    cache: &mut NoiseCache,
) -> LightYears {
    let range = max_range.value();
    if range.is_nan() || range <= 0.0 || limit.value().is_nan() || limit.value() <= 0.0 {
        return LightYears::ZERO;
    }
    let ratio = band.ratio();
    if ratio.is_nan() || ratio <= 0.0 {
        return max_range;
    }
    let range = if range < HORIZON_LIMIT_LY {
        range
    } else {
        HORIZON_LIMIT_LY
    };
    let metres = direction
        .components()
        .map(|c| c * range * METRES_PER_LIGHT_YEAR);
    let Some(end) = origin.translated(GalacticDisplacement::new(metres)) else {
        return max_range;
    };
    let line = Line::new(origin, &end);
    let plan = Plan::new(field, &line, &[], quality);
    let marcher = Marcher::new(field, &line, mode, plan.scale);
    // The limit as a dust-weighted column, so that the march compares like with like.
    let target = limit.value() / ratio * HYDROGEN_COLUMN_PER_MAG;
    let mut sums = Sums::default();
    for (piece, steps) in plan.stepped() {
        for step in 0..steps {
            let before = sums.dust;
            marcher.step(piece, steps, step, cache, &mut sums);
            if sums.dust >= target {
                let dt = (piece.t1 - piece.t0) / f64::from(steps);
                let start = piece.t0 + dt * f64::from(step);
                let within = (target - before) / (sums.dust - before);
                return LightYears::new((start + dt * within) * line.length_ly);
            }
        }
    }
    max_range
}

/// The cumulative visual extinction from `origin` along `direction` at each distance of `nodes`
/// (ascending, light-years), into `out` (cleared first): one march, as [`horizon`] marches, to the
/// last node, recording the running dust column, so that a profile of n nodes costs one
/// [`sightline`], not n (rendering plan R06, Design note 14).
///
/// Within a step the column is interpolated linearly. The march is from the origin outward, so,
/// as for [`horizon`], it is an instrument's integral and not symmetric: its last value equals a
/// [`sightline`] over the same segment to rounding, not bit for bit. Clouds among `modifiers` add
/// their Plummer column from the origin to each node; holes and walls are carved as [`sightline`]
/// carves them, a wall's column rising linearly across its piece. A node that is not finite or not
/// positive reads zero, and one beyond 2¹⁹ ly the column to there. `cache` changes the cost, never the values; in [`NoiseMode::Mean`] it is not read.
///
/// # Panics
///
/// In debug builds, if `nodes` is not ascending.
#[expect(
    clippy::too_many_arguments,
    reason = "plan 07's interface, as horizon's: a line, its nodes, a mode, a quality, the modifiers, the caller's cache and the output"
)]
pub fn profile(
    field: &GasField,
    origin: &GalacticPosition,
    direction: UnitVector,
    nodes: &[LightYears],
    mode: NoiseMode,
    quality: Quality,
    modifiers: &[GasModifier],
    cache: &mut NoiseCache,
    out: &mut Vec<Magnitudes>,
) {
    out.clear();
    debug_assert!(
        nodes.windows(2).all(|w| w[0].value() <= w[1].value()),
        "a profile's nodes are ascending"
    );
    let clamp = |d: f64| {
        if d.is_nan() || d <= 0.0 {
            0.0
        } else if d < HORIZON_LIMIT_LY {
            d
        } else {
            HORIZON_LIMIT_LY
        }
    };
    let Some(range) = nodes.last().map(|d| clamp(d.value())) else {
        return;
    };
    let metres = direction
        .components()
        .map(|c| c * range * METRES_PER_LIGHT_YEAR);
    let end = origin.translated(GalacticDisplacement::new(metres));
    let Some(end) = end.filter(|_| range > 0.0) else {
        out.extend(nodes.iter().map(|_| Magnitudes::ZERO));
        return;
    };
    let line = Line::new(origin, &end);
    let plan = Plan::new(field, &line, modifiers, quality);
    let marcher = Marcher::new(field, &line, mode, plan.scale);
    let fractions: Vec<f64> = nodes
        .iter()
        .map(|d| clamp(d.value()) / line.length_ly)
        .collect();
    let mut columns = NodeColumns::new(&fractions);
    let mut sums = Sums::default();
    let mut counts = plan.counts.iter().copied();
    for piece in &plan.pieces {
        let steps = match piece.kind {
            PieceKind::Stepped { .. } => counts
                .next()
                .expect("a plan holds a step count for every stepped piece"),
            PieceKind::Wall { dust, .. } => {
                // One stretch whose column rises linearly, as a step's is read.
                let (start, stop) = (piece.t0, piece.t1);
                let before = sums.dust;
                columns.reach(start, before);
                sums.dust += dust * (stop - start) * line.length_ly * CENTIMETRES_PER_LIGHT_YEAR;
                columns.rise((start, stop - start, stop), (before, sums.dust));
                continue;
            }
            PieceKind::Hole { .. } | PieceKind::Outside => continue,
        };
        let dt = (piece.t1 - piece.t0) / f64::from(steps);
        for step in 0..steps {
            let start = piece.t0 + dt * f64::from(step);
            let before = sums.dust;
            columns.reach(start, before);
            marcher.step(piece, steps, step, cache, &mut sums);
            columns.rise((start, dt, start + dt), (before, sums.dust));
        }
    }
    let mut columns = columns.finish(sums.dust);
    // Clouds in the canonical order, as sightline adds them, each from the origin to the node.
    let mut clouds: Vec<Cloud> = modifiers
        .iter()
        .filter_map(|modifier| match *modifier {
            GasModifier::Cloud {
                centre,
                core_radius,
                central_density,
                dust_per_hydrogen,
            } => Some(Cloud {
                centre,
                core: core_radius.value(),
                central: central_density.value(),
                zeta: dust_per_hydrogen,
            }),
            GasModifier::Hole { .. } | GasModifier::Wall { .. } => None,
        })
        .collect();
    clouds.sort_by(Cloud::order);
    if !clouds.is_empty() {
        for (column, &t) in columns.iter_mut().zip(&fractions) {
            if t <= 0.0 {
                continue;
            }
            let sub = Line::new(origin, &line.at(t.min(1.0)));
            for cloud in &clouds {
                *column +=
                    cloud.zeta * plummer_column(&sub, &cloud.centre, cloud.core, cloud.central);
            }
        }
    }
    out.extend(
        columns
            .iter()
            .map(|&dust| Magnitudes::new(dust / HYDROGEN_COLUMN_PER_MAG)),
    );
}

/// A profile's running columns at its nodes, filled in order along the line.
struct NodeColumns<'a> {
    /// The nodes' fractions of the line, ascending.
    fractions: &'a [f64],
    columns: Vec<f64>,
    /// The first node not yet given its column.
    next: usize,
}

impl<'a> NodeColumns<'a> {
    fn new(fractions: &'a [f64]) -> Self {
        Self {
            fractions,
            columns: vec![0.0; fractions.len()],
            next: 0,
        }
    }

    /// Every node at or before `start` takes the column so far, `column`.
    fn reach(&mut self, start: f64, column: f64) {
        while self.next < self.fractions.len() && self.fractions[self.next] <= start {
            self.columns[self.next] = column;
            self.next += 1;
        }
    }

    /// Every node before `stop` of the stretch from `start`, `width` long, takes the column
    /// interpolated linearly across it from `before` to `after`.
    fn rise(&mut self, (start, width, stop): (f64, f64, f64), (before, after): (f64, f64)) {
        while self.next < self.fractions.len() && self.fractions[self.next] < stop {
            let within = (self.fractions[self.next] - start) / width;
            self.columns[self.next] = before + (after - before) * within;
            self.next += 1;
        }
    }

    /// The columns, every node past the last stretch taking `column`, the whole line's.
    fn finish(mut self, column: f64) -> Vec<f64> {
        for rest in &mut self.columns[self.next..] {
            *rest = column;
        }
        self.columns
    }
}

/// A cloud's parts, as [`sightline`] sums its column.
#[derive(Debug, Clone, Copy)]
struct Cloud {
    centre: GalacticPosition,
    core: f64,
    central: f64,
    zeta: f64,
}

impl Cloud {
    /// The canonical order: by centre, as [`precedes`] orders points, then by core radius,
    /// central density and dust.
    fn order(a: &Self, b: &Self) -> core::cmp::Ordering {
        if precedes(&a.centre, &b.centre) {
            return core::cmp::Ordering::Less;
        }
        if precedes(&b.centre, &a.centre) {
            return core::cmp::Ordering::Greater;
        }
        a.core
            .total_cmp(&b.core)
            .then(a.central.total_cmp(&b.central))
            .then(a.zeta.total_cmp(&b.zeta))
    }
}

/// Whether `a` comes before `b` in the canonical order: by cell, then by offset, each
/// lexicographically.
fn precedes(a: &GalacticPosition, b: &GalacticPosition) -> bool {
    let (ca, cb) = (a.cell().to_array(), b.cell().to_array());
    if ca != cb {
        return ca < cb;
    }
    let (oa, ob) = (a.offset_metres(), b.offset_metres());
    for (x, y) in oa.iter().zip(&ob) {
        match x.total_cmp(y) {
            core::cmp::Ordering::Less => return true,
            core::cmp::Ordering::Greater => return false,
            core::cmp::Ordering::Equal => {}
        }
    }
    false
}

/// A segment from its first point, in light-years for its geometry and exactly for its samples.
#[derive(Debug, Clone, Copy)]
struct Line {
    start: GalacticPosition,
    /// The exact displacement to the end, metres.
    displacement: [f64; 3],
    /// The first point in float light-years, for the geometry of the pieces.
    start_ly: [f64; 3],
    /// The displacement in light-years.
    delta_ly: [f64; 3],
    length_ly: f64,
}

impl Line {
    fn new(a: &GalacticPosition, b: &GalacticPosition) -> Self {
        let displacement = a.displacement_to(b).metres();
        let delta_ly = displacement.map(|m| m / METRES_PER_LIGHT_YEAR);
        let length_ly = dot(delta_ly, delta_ly).sqrt();
        Self {
            start: *a,
            displacement,
            start_ly: a.to_light_years_f64(),
            delta_ly,
            length_ly,
        }
    }

    /// The point a fraction `t` of the way along, as a cell and an offset.
    fn at(&self, t: f64) -> GalacticPosition {
        self.start
            .translated(GalacticDisplacement::new(self.displacement.map(|m| m * t)))
            .expect("a point between two positions of the galaxy is a position of it")
    }

    /// The point a fraction `t` of the way along, in float light-years.
    fn at_ly(&self, t: f64) -> [f64; 3] {
        [0, 1, 2].map(|axis| self.start_ly[axis] + t * self.delta_ly[axis])
    }

    /// The fractions in `(0, 1)` at which `|z| = height`, appended to `out`.
    ///
    /// A level line gives an infinite or undefined fraction, which [`push_inside`] refuses.
    fn crossings_of_height(&self, height: f64, out: &mut Vec<f64>) {
        let (z0, dz) = (self.start_ly[2], self.delta_ly[2]);
        for level in [height, -height] {
            push_inside((level - z0) / dz, out);
        }
    }

    /// The fractions in `(0, 1)` at which the line meets the sphere of `radius` (ly) about the point
    /// `from_centre` ly behind its first point, appended to `out`.
    fn crossings_of_sphere(&self, from_centre: [f64; 3], radius: f64, out: &mut Vec<f64>) {
        let (a, half_b, c) = self.sphere_quadratic(from_centre, radius);
        let discriminant = half_b * half_b - a * c;
        if a > 0.0 && discriminant > 0.0 {
            let root = discriminant.sqrt();
            push_inside((-half_b - root) / a, out);
            push_inside((-half_b + root) / a, out);
        }
    }

    /// `|w + t d|² − r² = a t² + 2 (half_b) t + c` for `w` the first point's offset from the
    /// sphere's centre, ly.
    fn sphere_quadratic(&self, from_centre: [f64; 3], radius: f64) -> (f64, f64, f64) {
        let d = self.delta_ly;
        (
            dot(d, d),
            dot(from_centre, d),
            dot(from_centre, from_centre) - radius * radius,
        )
    }

    /// Whether the point a fraction `t` along lies inside the sphere of `radius` about the point
    /// `from_centre` ly behind the first point.
    fn inside_sphere(&self, from_centre: [f64; 3], radius: f64, t: f64) -> bool {
        let (a, half_b, c) = self.sphere_quadratic(from_centre, radius);
        (a * t + 2.0 * half_b) * t + c < 0.0
    }

    /// The fractions outside which [`inside_sphere`](Self::inside_sphere) is false at every `t` in
    /// `[0, 1]`, rounding and all, for the sphere of `radius` about the point `from_centre` ly
    /// behind the first point; `None` where it is false at every such `t`.
    ///
    /// The quadratic is `q(t) = a (t − v)² + m`, with `v = −half_b ÷ a` and `m = c − half_b² ÷ a`.
    /// For `|t| ≤ 1`, `inside_sphere`'s three roundings move `q` by at most `4u` of its terms'
    /// magnitudes, `S = a + 2|half_b| + |c| + half_b² ÷ a` bounding them, with `u` 2⁻⁵³; so `q` reads
    /// negative only where it is below `margin = 10⁻⁹ S`, which is within `√((margin − m) ÷ a)` of
    /// `v`. That width and `v` are widened by far more than their own roundings. A line of no
    /// length, or terms that are not finite, gives the whole line.
    #[must_use]
    fn inside_bounds(&self, from_centre: [f64; 3], radius: f64) -> Option<(f64, f64)> {
        let whole = Some((f64::NEG_INFINITY, f64::INFINITY));
        let (a, half_b, c) = self.sphere_quadratic(from_centre, radius);
        if !(a.is_finite() && a > 0.0) {
            return whole;
        }
        let vertex = -half_b / a;
        let lowest = c - half_b * half_b / a;
        let scale = a + 2.0 * half_b.abs() + c.abs() + half_b * half_b / a;
        let margin = INSIDE_MARGIN * scale;
        if !(vertex.is_finite() && lowest.is_finite() && margin.is_finite()) {
            return whole;
        }
        if lowest >= margin {
            return None;
        }
        let width = ((margin - lowest) / a).sqrt() * (1.0 + 1e-6) + 1e-9 * (1.0 + vertex.abs());
        Some((vertex - width, vertex + width))
    }
}

/// [`Line::inside_bounds`]' margin on the quadratic, relative to its terms' magnitudes: some 10⁶
/// times the `4u` its roundings can move it by.
const INSIDE_MARGIN: f64 = 1e-9;

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Appends `t` if it lies strictly inside `(0, 1)`.
fn push_inside(t: f64, out: &mut Vec<f64>) {
    if t > 0.0 && t < 1.0 {
        out.push(t);
    }
}

/// What a piece of the segment is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PieceKind {
    /// Inside a hole, whose lowest interior density (cm⁻³) replaces the field.
    Hole { interior: f64 },
    /// Inside a wall and no hole: the densest wall's hydrogen and dust-bearing densities (cm⁻³)
    /// replace the field.
    Wall { density: f64, dust: f64 },
    /// Outside the slab, where only the corona is.
    Outside,
    /// Inside the slab, marched with steps of at most `base × 2^lod` ly.
    Stepped { base: f64 },
}

/// A piece of the segment, from the fraction `t0` to `t1` of the way along.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Piece {
    t0: f64,
    t1: f64,
    kind: PieceKind,
}

/// The pieces of a segment and the steps each takes.
#[derive(Debug, Clone, PartialEq)]
struct Plan {
    pieces: Vec<Piece>,
    /// The steps of each stepped piece, in order.
    counts: Vec<u32>,
    /// Their sum.
    steps: u32,
    /// The noise's smoothing scale for the steps taken.
    scale: SmoothingScale,
}

impl Plan {
    /// The pieces of `line` in `field` with the holes of `mods`, and the steps `quality` gives them.
    fn new(field: &GasField, line: &Line, mods: &[GasModifier], quality: Quality) -> Self {
        let pieces = pieces(field, line, mods);
        let lengths: Vec<(f64, f64)> = pieces
            .iter()
            .filter_map(|piece| match piece.kind {
                PieceKind::Stepped { base } => Some(((piece.t1 - piece.t0) * line.length_ly, base)),
                PieceKind::Hole { .. } | PieceKind::Wall { .. } | PieceKind::Outside => None,
            })
            .collect();
        match quality {
            Quality::Full => Self::at_lod(pieces, &lengths, 0),
            Quality::Budget(budget) => {
                let budget = budget.get();
                if let Some(lod) = (0..=LOD_MAX).find(|&lod| total(&lengths, lod) <= budget) {
                    return Self::at_lod(pieces, &lengths, lod);
                }
                // Even one step per piece is over the budget: march each stretch between holes as
                // one piece, and take the steps it then allows.
                let merged = merge_runs(&pieces);
                let lengths: Vec<(f64, f64)> = merged
                    .iter()
                    .filter_map(|piece| match piece.kind {
                        PieceKind::Stepped { base } => {
                            Some(((piece.t1 - piece.t0) * line.length_ly, base))
                        }
                        PieceKind::Hole { .. } | PieceKind::Wall { .. } | PieceKind::Outside => {
                            None
                        }
                    })
                    .collect();
                let lod = (0..=LOD_MAX)
                    .find(|&lod| total(&lengths, lod) <= budget)
                    .unwrap_or(LOD_MAX);
                Self::at_lod(merged, &lengths, lod)
            }
        }
    }

    fn at_lod(pieces: Vec<Piece>, lengths: &[(f64, f64)], lod: u32) -> Self {
        let counts: Vec<u32> = lengths
            .iter()
            .map(|&(length, base)| steps_for(length, base, lod))
            .collect();
        let steps = counts.iter().fold(0_u32, |sum, &n| sum.saturating_add(n));
        let noise_step = NOISE_STEP_LY * f64::from(1_u32 << lod);
        let scale = if lod == 0 {
            SmoothingScale::Full
        } else {
            SmoothingScale::AtLeast(LightYears::new(2.0 * noise_step))
        };
        Self {
            pieces,
            counts,
            steps,
            scale,
        }
    }

    /// The stepped pieces with their step counts, in order along the segment.
    fn stepped(&self) -> impl Iterator<Item = (&Piece, u32)> {
        self.pieces
            .iter()
            .filter(|piece| matches!(piece.kind, PieceKind::Stepped { .. }))
            .zip(self.counts.iter().copied())
    }
}

/// The step count of a piece `length` ly long with base step `base` ly at `lod`: the least whole
/// number of equal steps no longer than `base × 2^lod`, at least one.
fn steps_for(length: f64, base: f64, lod: u32) -> u32 {
    let steps = (length / (base * f64::from(1_u32 << lod))).ceil();
    if steps.is_nan() || steps < 1.0 {
        return 1;
    }
    if steps >= f64::from(u32::MAX) {
        return u32::MAX;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a whole number between 1 and u32::MAX, checked above"
    )]
    let steps = steps as u32;
    steps
}

/// The total step count of pieces of these lengths and bases at `lod`.
fn total(lengths: &[(f64, f64)], lod: u32) -> u32 {
    lengths.iter().fold(0_u32, |sum, &(length, base)| {
        sum.saturating_add(steps_for(length, base, lod))
    })
}

/// The pieces of `line`: cut at the slab's faces, at the smooth scales' boundaries, at every hole's
/// surface and at both of every wall's, each labelled by its middle.
fn pieces(field: &GasField, line: &Line, mods: &[GasModifier]) -> Vec<Piece> {
    let gas = field.smooth();
    let slab = SLAB_WARM_HEIGHTS * gas.height(GasLayer::Warm);
    let plane = PLANE_NEUTRAL_HEIGHTS * gas.height(GasLayer::Neutral);
    let centre = CENTRE_MOLECULAR_LENGTHS * field.params().molecular_disc().length().value();
    let mut cuts = Vec::with_capacity(8 + 2 * mods.len());
    line.crossings_of_height(slab, &mut cuts);
    line.crossings_of_height(plane, &mut cuts);
    let from_origin = line.start_ly;
    line.crossings_of_sphere(from_origin, centre, &mut cuts);
    let holes: Vec<([f64; 3], f64, f64)> = mods
        .iter()
        .filter_map(|modifier| match *modifier {
            GasModifier::Hole {
                centre,
                radius,
                interior,
            } => Some((
                centre
                    .displacement_to(&line.start)
                    .metres()
                    .map(|m| m / METRES_PER_LIGHT_YEAR),
                radius.value(),
                interior.value(),
            )),
            GasModifier::Cloud { .. } | GasModifier::Wall { .. } => None,
        })
        .collect();
    for &(from_centre, radius, _) in &holes {
        line.crossings_of_sphere(from_centre, radius, &mut cuts);
    }
    let mut walls = WallSweep::new(line, mods);
    walls.cut(line, &mut cuts);
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    // The pieces' middles rise along the line, so a sweep keeps only the holes whose bounds hold
    // the middle (`active`, in the list's order), and each piece tests those alone: the same holes
    // pass, in the same order, as when every piece tested every hole. A profile from the Sun-like
    // point through the centre, past 10,597 bubbles, took 174 ms so and takes 13.7 ms (P09.T40.a;
    // pod, test build, 2026-11-05).
    let mut rising: Vec<(usize, (f64, f64))> = holes
        .iter()
        .enumerate()
        .filter_map(|(hole, &(from_centre, radius, _))| {
            line.inside_bounds(from_centre, radius)
                .map(|bounds| (hole, bounds))
        })
        .collect();
    rising.sort_by(|(_, (left, _)), (_, (right, _))| left.total_cmp(right));
    let mut rising = rising.into_iter().peekable();
    let mut active: Vec<(usize, f64)> = Vec::new();
    let smooth = smooth_scales(field);
    let mut pieces = Vec::with_capacity(cuts.len() + 1);
    let mut t0 = 0.0;
    for t1 in cuts.into_iter().chain(core::iter::once(1.0)) {
        let middle = f64::midpoint(t0, t1);
        while let Some((hole, (_, high))) = rising.next_if(|(_, (low, _))| *low <= middle) {
            let at = active.partition_point(|&(earlier, _)| earlier < hole);
            active.insert(at, (hole, high));
        }
        active.retain(|&(_, high)| high >= middle);
        let lowest = active
            .iter()
            .map(|&(hole, _)| holes[hole])
            .filter(|&(from_centre, radius, _)| line.inside_sphere(from_centre, radius, middle))
            .map(|(_, _, interior)| interior)
            .reduce(|lowest, interior| if interior < lowest { interior } else { lowest });
        let [_, _, z] = line.at_ly(middle);
        let wall = walls.at(line, middle);
        let kind = if let Some(interior) = lowest {
            PieceKind::Hole { interior }
        } else if let Some((density, dust)) = wall {
            PieceKind::Wall { density, dust }
        } else if z.is_nan() || z.abs() > slab {
            PieceKind::Outside
        } else {
            let base = if line.inside_sphere(from_origin, centre, middle) {
                smooth.centre
            } else if z.abs() < plane {
                smooth.plane
            } else {
                smooth.elsewhere
            };
            PieceKind::Stepped { base }
        };
        pieces.push(Piece { t0, t1, kind });
        t0 = t1;
    }
    pieces
}

/// The walls of a line's modifiers, swept along it as [`pieces`] sweeps the holes: by their outer
/// spheres' bounds ([`Line::inside_bounds`]), so that each piece tests only the walls that can hold
/// its middle. The densest wins whatever their order, so the active walls keep none.
struct WallSweep {
    /// Each wall's centre's offset behind the line's first point (ly), its inner and outer radii
    /// (ly), and its hydrogen and dust-bearing densities (cm⁻³).
    walls: Vec<([f64; 3], f64, f64, f64, f64)>,
    /// The walls not yet reached, with their bounds, by their low bounds.
    rising: core::iter::Peekable<std::vec::IntoIter<(usize, (f64, f64))>>,
    /// The walls reached, with their high bounds.
    active: Vec<(usize, f64)>,
}

impl WallSweep {
    fn new(line: &Line, mods: &[GasModifier]) -> Self {
        let walls: Vec<([f64; 3], f64, f64, f64, f64)> = mods
            .iter()
            .filter_map(|modifier| match *modifier {
                GasModifier::Wall {
                    centre,
                    inner,
                    outer,
                    density,
                    dust_per_hydrogen,
                } => Some((
                    centre
                        .displacement_to(&line.start)
                        .metres()
                        .map(|m| m / METRES_PER_LIGHT_YEAR),
                    inner.value(),
                    outer.value(),
                    density.value(),
                    dust_per_hydrogen * density.value(),
                )),
                GasModifier::Hole { .. } | GasModifier::Cloud { .. } => None,
            })
            .collect();
        let mut rising: Vec<(usize, (f64, f64))> = walls
            .iter()
            .enumerate()
            .filter_map(|(wall, &(from_centre, _, outer, _, _))| {
                line.inside_bounds(from_centre, outer)
                    .map(|bounds| (wall, bounds))
            })
            .collect();
        rising.sort_by(|(_, (left, _)), (_, (right, _))| left.total_cmp(right));
        Self {
            walls,
            rising: rising.into_iter().peekable(),
            active: Vec::new(),
        }
    }

    /// The fractions at which `line` meets every wall's two spheres, appended to `cuts`.
    fn cut(&self, line: &Line, cuts: &mut Vec<f64>) {
        for &(from_centre, inner, outer, _, _) in &self.walls {
            line.crossings_of_sphere(from_centre, inner, cuts);
            line.crossings_of_sphere(from_centre, outer, cuts);
        }
    }

    /// The densest wall whose shell holds the point a fraction `middle` along `line`, as its
    /// hydrogen and dust-bearing densities; the middles asked for must rise.
    fn at(&mut self, line: &Line, middle: f64) -> Option<(f64, f64)> {
        while let Some((wall, (_, high))) = self.rising.next_if(|(_, (low, _))| *low <= middle) {
            self.active.push((wall, high));
        }
        self.active.retain(|&(_, high)| high >= middle);
        densest(self.active.iter().filter_map(|&(wall, _)| {
            let (from_centre, inner, outer, density, dust) = self.walls[wall];
            (line.inside_sphere(from_centre, outer, middle)
                && !line.inside_sphere(from_centre, inner, middle))
            .then_some((density, dust))
        }))
    }
}

/// The base step of each smooth region, ly: half its smallest smooth scale, capped at the noise's
/// step (Design note 14).
struct SmoothScales {
    centre: f64,
    plane: f64,
    elsewhere: f64,
}

fn smooth_scales(field: &GasField) -> SmoothScales {
    let gas = field.smooth();
    let cap = |scale: f64| {
        let half = 0.5 * scale;
        if half < NOISE_STEP_LY {
            half
        } else {
            NOISE_STEP_LY
        }
    };
    let neutral = gas.height(GasLayer::Neutral);
    let lane = field.params().lane().width().value();
    SmoothScales {
        centre: cap(gas.height(GasLayer::Molecular)),
        plane: cap(if lane < neutral { lane } else { neutral }),
        elsewhere: cap(0.25 * gas.height(GasLayer::Warm)),
    }
}

/// The stretches of `pieces` between holes and outside the slab, each as one stepped piece with the
/// noise's step as its base: what a budget below one step per piece marches.
fn merge_runs(pieces: &[Piece]) -> Vec<Piece> {
    let mut merged: Vec<Piece> = Vec::with_capacity(pieces.len());
    for piece in pieces {
        let stepped = matches!(piece.kind, PieceKind::Stepped { .. });
        match merged.last_mut() {
            Some(last) if stepped && matches!(last.kind, PieceKind::Stepped { .. }) => {
                last.t1 = piece.t1;
            }
            _ => merged.push(if stepped {
                Piece {
                    kind: PieceKind::Stepped {
                        base: NOISE_STEP_LY,
                    },
                    ..*piece
                }
            } else {
                *piece
            }),
        }
    }
    merged
}

/// The running columns, cm⁻².
#[derive(Debug, Clone, Copy, Default)]
struct Sums {
    dust: f64,
    hydrogen: f64,
    neutral: f64,
}

/// What one step's samples read, and how.
struct Marcher<'a> {
    field: &'a GasField,
    line: &'a Line,
    mode: NoiseMode,
    scale: SmoothingScale,
}

impl<'a> Marcher<'a> {
    fn new(field: &'a GasField, line: &'a Line, mode: NoiseMode, scale: SmoothingScale) -> Self {
        Self {
            field,
            line,
            mode,
            scale,
        }
    }

    /// Adds step `step` of the `steps` equal steps of `piece` to `sums`, by the two-point
    /// Gauss–Legendre rule.
    fn step(&self, piece: &Piece, steps: u32, step: u32, cache: &mut NoiseCache, sums: &mut Sums) {
        let dt = (piece.t1 - piece.t0) / f64::from(steps);
        let middle = piece.t0 + dt * (f64::from(step) + 0.5);
        let half = 0.5 * dt;
        let weight = half * self.line.length_ly * CENTIMETRES_PER_LIGHT_YEAR;
        for t in [middle - half * GL2_NODE, middle + half * GL2_NODE] {
            let sample = self.sample(t, cache);
            sums.dust += weight * sample.dust;
            sums.hydrogen += weight * sample.gas;
            sums.neutral += weight * sample.neutral;
        }
    }

    /// The field at the point a fraction `t` along: its dust-bearing, disc and neutral densities.
    fn sample(&self, t: f64, cache: &mut NoiseCache) -> Sample {
        let p = self.line.at(t);
        let site = Site::of(&p);
        let layers = self.field.layers(&site);
        let zeta = self.field.zeta(site.r);
        match self.mode {
            NoiseMode::Mean => {
                let gas = layers.disc();
                Sample {
                    dust: gas * zeta,
                    gas,
                    neutral: layers.neutral + layers.molecular,
                }
            }
            NoiseMode::Realised => {
                let factor = self.field.noise(&p, self.scale, cache);
                let gas = layers.disc() * factor;
                Sample {
                    dust: gas * zeta,
                    gas,
                    neutral: (layers.neutral + layers.molecular) * factor,
                }
            }
        }
    }
}

/// One sample's densities, cm⁻³.
#[derive(Debug, Clone, Copy)]
struct Sample {
    /// `n_disc F ζ`.
    dust: f64,
    /// `n_disc F`, the corona apart.
    gas: f64,
    /// The neutral hydrogen.
    neutral: f64,
}

/// The column of the Plummer ball of core radius `core` (ly) and central density `central` (cm⁻³)
/// about `centre` along `line`, cm⁻² (Design note 16).
///
/// With `u` the first point's offset from the centre and `e` the line's direction, the closest
/// approach is `b = |u × e|` — the cross product, which keeps its digits where `|u|² − (u·e)²`
/// would cancel — and the distances along the line from it are `s₀ = u·e` and `s₁ = s₀ + L`.
fn plummer_column(line: &Line, centre: &GalacticPosition, core: f64, central: f64) -> f64 {
    if core.is_nan() || core <= 0.0 {
        return 0.0;
    }
    let u = centre
        .displacement_to(&line.start)
        .metres()
        .map(|m| m / METRES_PER_LIGHT_YEAR);
    let e = line.delta_ly.map(|d| d / line.length_ly);
    let cross = [
        u[1] * e[2] - u[2] * e[1],
        u[2] * e[0] - u[0] * e[2],
        u[0] * e[1] - u[1] * e[0],
    ];
    let c_sq = core * core + dot(cross, cross);
    let s0 = dot(u, e);
    let s1 = s0 + line.length_ly;
    let antiderivative = |s: f64| {
        let r_sq = c_sq + s * s;
        s * (3.0 * c_sq + 2.0 * s * s) / (3.0 * c_sq * c_sq * r_sq * r_sq.sqrt())
    };
    let core_sq = core * core;
    central
        * core_sq
        * core_sq
        * core
        * (antiderivative(s1) - antiderivative(s0))
        * CENTIMETRES_PER_LIGHT_YEAR
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;

    #[test]
    fn a_profile_ends_on_the_sightline_and_rises_with_distance() {
        use crate::galaxy::Galaxy;
        let galaxy = Galaxy::new(crate::Seed::new(42));
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube");
        let direction = UnitVector::from_components([-0.3, -0.9, 0.05]).expect("a direction");
        let nodes: Vec<LightYears> = (0..=48)
            .map(|k| LightYears::new(crate::math::exp10(f64::from(k) / 12.0)))
            .collect();
        for mode in [NoiseMode::Mean, NoiseMode::Realised] {
            let mut cache = NoiseCache::with_capacity(1 << 14);
            let mut out = Vec::new();
            profile(
                galaxy.gas(),
                &sun,
                direction,
                &nodes,
                mode,
                Quality::Full,
                &[],
                &mut cache,
                &mut out,
            );
            assert_eq!(out.len(), nodes.len());
            assert!(
                out.windows(2).all(|w| w[0].value() <= w[1].value()),
                "{mode:?}"
            );
            let last = nodes[nodes.len() - 1].value();
            let end = sun
                .translated(GalacticDisplacement::new(
                    direction
                        .components()
                        .map(|c| c * last * METRES_PER_LIGHT_YEAR),
                ))
                .expect("in the cube");
            let line = sightline(
                galaxy.gas(),
                &sun,
                &end,
                mode,
                Quality::Full,
                &[],
                &mut cache,
            );
            let (a, b) = (out[out.len() - 1].value(), line.a_v().value());
            assert!(((a - b) / b).abs() < 1e-12, "{mode:?}: {a} against {b}");
            if mode == NoiseMode::Mean {
                // The mean field reads no cache: an empty one gives the same bits.
                let mut again = Vec::new();
                profile(
                    galaxy.gas(),
                    &sun,
                    direction,
                    &nodes,
                    mode,
                    Quality::Full,
                    &[],
                    &mut NoiseCache::with_capacity(0),
                    &mut again,
                );
                assert_eq!(again, out);
            }
        }
    }
    use crate::Seed;
    use crate::galaxy::fields::Fields;
    use crate::galaxy::gas::params::GasParams;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::potential::MassModel;
    use crate::units::HydrogenPerCm3;

    fn milky_way(seed: Seed) -> GasField {
        let params = GalaxyParams::milky_way_like();
        let fields = Fields::new(&params, &MassModel::new(&params));
        GasField::with_params(seed, GasParams::milky_way_like(), &fields)
    }

    fn at(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).expect("inside the root cube")
    }

    fn budget(steps: u32) -> Quality {
        Quality::Budget(NonZeroU32::new(steps).expect("a budget of at least one step"))
    }

    /// The pieces cover the segment without a gap, in order, and take their labels from where
    /// they lie: the slab's faces, the plane's zone and the centre's sphere cut an in-plane line
    /// through the centre and a steep one out of the disc.
    #[test]
    fn the_pieces_tile_the_segment_and_follow_the_smooth_scales() {
        let field = milky_way(Seed::new(1));
        for (a, b) in [
            ([26_000.0, 10.0, 0.0], [-3_000.0, -5.0, 0.0]),
            ([0.0, 26_000.0, -30_000.0], [300.0, 25_000.0, 40_000.0]),
            ([100.0, 50.0, -2_000.0], [-80.0, -40.0, 2_000.0]),
        ] {
            let line = Line::new(&at(a), &at(b));
            let pieces = pieces(&field, &line, &[]);
            assert_same_bits(pieces[0].t0, 0.0);
            assert_same_bits(pieces[pieces.len() - 1].t1, 1.0);
            for pair in pieces.windows(2) {
                assert_same_bits(pair[0].t1, pair[1].t0);
                assert!(pair[0].t0 < pair[0].t1);
            }
        }
        let scales = smooth_scales(&field);
        // The fixture's molecular disc is 58 ly tall, so its sphere's step is 29 ly; the lanes are
        // 200 ly wide and the neutral layer 700 ly tall, and the warm layer 3,000 ly, so both other
        // regions take the noise's 32 ly.
        assert_same_bits(scales.centre, 29.0);
        assert_same_bits(scales.plane, NOISE_STEP_LY);
        assert_same_bits(scales.elsewhere, NOISE_STEP_LY);
        let line = Line::new(&at([2_000.0, 0.0, 0.0]), &at([-2_000.0, 0.0, 0.0]));
        let kinds: Vec<PieceKind> = pieces(&field, &line, &[]).iter().map(|p| p.kind).collect();
        assert_eq!(
            kinds,
            [
                PieceKind::Stepped { base: 32.0 },
                PieceKind::Stepped { base: 29.0 },
                PieceKind::Stepped { base: 32.0 },
            ]
        );
    }

    /// A budget is honoured whatever it is: from one step to thousands, on lines of every length
    /// and through holes, the steps taken never exceed it, and a budget that one step per piece
    /// already meets takes the finest steps it allows.
    #[test]
    fn a_budget_is_never_exceeded() {
        let field = milky_way(Seed::new(2));
        let mut lcg = Lcg::new(0x0708_b0d6);
        let hole = GasModifier::Hole {
            centre: at([13_000.0, 0.0, 0.0]),
            radius: LightYears::new(400.0),
            interior: HydrogenPerCm3::new(0.005),
        };
        for _ in 0..400 {
            let [a, b] = [0; 2].map(|_| {
                at([0; 3].map(|_| 60_000.0 * (2.0 * lcg.next_f64() - 1.0) * lcg.next_f64()))
            });
            let line = Line::new(&a, &b);
            for steps in [1, 2, 3, 7, 16, 64, 256, 4_096] {
                for mods in [&[][..], &[hole][..]] {
                    let plan = Plan::new(&field, &line, mods, budget(steps));
                    let runs = merge_runs(&plan.pieces)
                        .iter()
                        .filter(|p| matches!(p.kind, PieceKind::Stepped { .. }))
                        .count();
                    let runs = u32::try_from(runs).unwrap();
                    assert!(
                        plan.steps <= steps.max(runs),
                        "{} steps over a budget of {steps}",
                        plan.steps
                    );
                    if mods.is_empty() {
                        assert!(plan.steps <= steps, "{} steps over {steps}", plan.steps);
                    }
                }
            }
        }
    }

    /// The sweep labels every piece with the hole every hole's own test gives it (P09.T40.a): the
    /// same lowest interior, bit for bit, or none. The lines are long and short, and their holes
    /// many, near and far, holding the whole line, and grazing it at a tangent, where the test's
    /// roundings decide.
    #[test]
    fn the_sweep_labels_each_piece_as_testing_every_hole_does() {
        let field = milky_way(Seed::new(3));
        let mut lcg = Lcg::new(0x0940_a0d6);
        let uniform = |lcg: &mut Lcg, scale: f64| scale * (2.0 * lcg.next_f64() - 1.0);
        let mut grazing = 0;
        for case in 0..60 {
            let length = [10.0, 900.0, 30_000.0][case % 3];
            let start = [0; 3].map(|_| uniform(&mut lcg, 3_000.0));
            let direction = [0; 3].map(|_| uniform(&mut lcg, 1.0));
            let norm = direction.iter().map(|c| c * c).sum::<f64>().sqrt();
            let end: [f64; 3] = [0, 1, 2].map(|k| start[k] + length * direction[k] / norm);
            let line = Line::new(&at(start), &at(end));
            let mut mods = Vec::new();
            for _ in 0..300 {
                let t = 1.4 * lcg.next_f64() - 0.2;
                let on = line.at_ly(t);
                let offset = [0; 3].map(|_| uniform(&mut lcg, 1.0));
                let reach = uniform(&mut lcg, 0.5 * length);
                let miss = [0, 1, 2].map(|k| on[k] + reach * offset[k]);
                let centre = at(miss);
                let from_centre = centre
                    .displacement_to(&line.start)
                    .metres()
                    .map(|m| m / METRES_PER_LIGHT_YEAR);
                // A third graze the line: the radius is the centre's distance from it.
                let (a, half_b, c) = line.sphere_quadratic(from_centre, 0.0);
                let distance = (c - half_b * half_b / a).max(0.0).sqrt();
                let radius = match lcg.next_below(3) {
                    0 => {
                        grazing += 1;
                        distance
                    }
                    1 => distance * (0.5 + lcg.next_f64()),
                    _ => length * (0.01 + 3.0 * lcg.next_f64()),
                };
                if radius > 0.0 {
                    mods.push(GasModifier::Hole {
                        centre,
                        radius: LightYears::new(radius),
                        interior: HydrogenPerCm3::new(0.001 + lcg.next_f64()),
                    });
                }
            }
            let holes: Vec<([f64; 3], f64, f64)> = mods
                .iter()
                .map(|modifier| match *modifier {
                    GasModifier::Hole {
                        centre,
                        radius,
                        interior,
                    } => (
                        centre
                            .displacement_to(&line.start)
                            .metres()
                            .map(|m| m / METRES_PER_LIGHT_YEAR),
                        radius.value(),
                        interior.value(),
                    ),
                    GasModifier::Cloud { .. } | GasModifier::Wall { .. } => {
                        unreachable!("only holes")
                    }
                })
                .collect();
            let swept = pieces(&field, &line, &mods);
            assert!(swept.len() > 2, "case {case}: the holes cut the line");
            for piece in &swept {
                let middle = f64::midpoint(piece.t0, piece.t1);
                let every = holes
                    .iter()
                    .filter(|&&(from_centre, radius, _)| {
                        line.inside_sphere(from_centre, radius, middle)
                    })
                    .map(|&(_, _, interior)| interior)
                    .reduce(|lowest, interior| if interior < lowest { interior } else { lowest });
                match (every, piece.kind) {
                    (Some(lowest), PieceKind::Hole { interior }) => {
                        assert_same_bits(interior, lowest);
                    }
                    (None, PieceKind::Outside | PieceKind::Stepped { .. }) => {}
                    (every, kind) => panic!("case {case} at {middle}: {kind:?} against {every:?}"),
                }
            }
        }
        assert!(grazing > 5_000, "{grazing} grazing holes");
    }

    /// The walls' sweep labels every piece with the wall testing every hole and every wall gives
    /// it (P09.T5): a hole first, then the densest wall holding the middle, bit for bit, or the
    /// field. The walls are many, some grazing the line, some lining a hole.
    #[test]
    fn the_sweep_labels_each_wall_piece_as_testing_every_wall_does() {
        let field = milky_way(Seed::new(4));
        let mut lcg = Lcg::new(0x0905_ba11);
        let uniform = |lcg: &mut Lcg, scale: f64| scale * (2.0 * lcg.next_f64() - 1.0);
        let mut walled = 0;
        for case in 0..30 {
            let length = [10.0, 900.0, 30_000.0][case % 3];
            let start = [0; 3].map(|_| uniform(&mut lcg, 3_000.0));
            let direction = [0; 3].map(|_| uniform(&mut lcg, 1.0));
            let norm = direction.iter().map(|c| c * c).sum::<f64>().sqrt();
            let end: [f64; 3] = [0, 1, 2].map(|k| start[k] + length * direction[k] / norm);
            let line = Line::new(&at(start), &at(end));
            let mut mods = Vec::new();
            for _ in 0..200 {
                let on = line.at_ly(1.4 * lcg.next_f64() - 0.2);
                let reach = uniform(&mut lcg, 0.5 * length);
                let offset = [0; 3].map(|_| uniform(&mut lcg, 1.0));
                let centre = at([0, 1, 2].map(|k| on[k] + reach * offset[k]));
                let from_centre = centre
                    .displacement_to(&line.start)
                    .metres()
                    .map(|m| m / METRES_PER_LIGHT_YEAR);
                let (a, half_b, c) = line.sphere_quadratic(from_centre, 0.0);
                let distance = (c - half_b * half_b / a).max(0.0).sqrt();
                let outer = if lcg.next_below(3) == 0 {
                    distance
                } else {
                    length * (0.01 + 2.0 * lcg.next_f64())
                };
                if outer <= 0.0 {
                    continue;
                }
                let inner = 0.9 * outer;
                // Some small bubbles' holes, which leave most of the line to the walls.
                if outer < 0.2 * length && lcg.next_below(2) == 0 {
                    mods.push(GasModifier::Hole {
                        centre,
                        radius: LightYears::new(inner),
                        interior: HydrogenPerCm3::new(0.001 + 0.01 * lcg.next_f64()),
                    });
                }
                mods.push(GasModifier::Wall {
                    centre,
                    inner: LightYears::new(inner),
                    outer: LightYears::new(outer),
                    density: HydrogenPerCm3::new(0.1 + lcg.next_f64()),
                    dust_per_hydrogen: lcg.next_f64(),
                });
            }
            let inside = |centre: &GalacticPosition, radius: f64, t: f64| {
                let from_centre = centre
                    .displacement_to(&line.start)
                    .metres()
                    .map(|m| m / METRES_PER_LIGHT_YEAR);
                line.inside_sphere(from_centre, radius, t)
            };
            for piece in &pieces(&field, &line, &mods) {
                let middle = f64::midpoint(piece.t0, piece.t1);
                let holed = mods.iter().any(|m| match *m {
                    GasModifier::Hole { centre, radius, .. } => {
                        inside(&centre, radius.value(), middle)
                    }
                    GasModifier::Wall { .. } | GasModifier::Cloud { .. } => false,
                });
                let every = densest(mods.iter().filter_map(|m| {
                    match *m {
                        GasModifier::Wall {
                            centre,
                            inner,
                            outer,
                            density,
                            dust_per_hydrogen,
                        } => (inside(&centre, outer.value(), middle)
                            && !inside(&centre, inner.value(), middle))
                        .then_some((density.value(), dust_per_hydrogen * density.value())),
                        GasModifier::Hole { .. } | GasModifier::Cloud { .. } => None,
                    }
                }));
                match (holed, every, piece.kind) {
                    (true, _, PieceKind::Hole { .. })
                    | (false, None, PieceKind::Outside | PieceKind::Stepped { .. }) => {}
                    (false, Some((n, d)), PieceKind::Wall { density, dust }) => {
                        assert_same_bits(density, n);
                        assert_same_bits(dust, d);
                        walled += 1;
                    }
                    (holed, every, kind) => {
                        panic!("case {case} at {middle}: {kind:?} against {holed} {every:?}")
                    }
                }
            }
        }
        assert!(walled > 1_000, "{walled} wall pieces");
    }

    /// Full quality steps no longer than 32 ly, and a budget's noise is smoothed to twice its step.
    #[test]
    fn full_quality_takes_the_finest_steps() {
        let field = milky_way(Seed::new(3));
        let line = Line::new(&at([26_000.0, 0.0, 0.0]), &at([0.0, 0.0, 0.0]));
        let full = Plan::new(&field, &line, &[], Quality::Full);
        assert_eq!(full.scale, SmoothingScale::Full);
        for (piece, steps) in full.stepped() {
            let step = (piece.t1 - piece.t0) * line.length_ly / f64::from(steps);
            assert!(step <= NOISE_STEP_LY, "a step of {step} ly");
        }
        // The line from the Sun to the centre in 64 steps takes steps of 512 ly and reads the
        // noise's coarsest octave alone.
        let coarse = Plan::new(&field, &line, &[], budget(64));
        assert!(
            coarse.steps <= 64 && coarse.steps > 32,
            "{} steps",
            coarse.steps
        );
        assert_eq!(
            coarse.scale,
            SmoothingScale::AtLeast(LightYears::new(1_024.0))
        );
    }

    /// A cloud's column along a line is the integral of its Plummer density: to 10⁻⁶ of a
    /// brute-force midpoint sum on 100 random segments near it, the same whichever way the segment
    /// runs, and `4 a n_c ÷ 3` along a long line through its centre.
    #[test]
    fn a_clouds_column_is_the_integral_of_its_density() {
        let mut lcg = Lcg::new(0x0708_c10d);
        let centre = at([12_345.6, -789.0, 42.0]);
        let (core, central) = (7.5, 400.0);
        for _ in 0..100 {
            let [start, end] = [0; 2].map(|_| {
                at([12_345.6, -789.0, 42.0].map(|c| c + 60.0 * (2.0 * lcg.next_f64() - 1.0)))
            });
            let forward = Line::new(&start, &end);
            let column = plummer_column(&forward, &centre, core, central);
            let backward = plummer_column(&Line::new(&end, &start), &centre, core, central);
            assert!(
                (column / backward - 1.0).abs() < 1e-12,
                "{column} against {backward}"
            );
            let points = 200_000_u32;
            let step = 1.0 / f64::from(points);
            let mut sum = 0.0;
            let [cx, cy, cz] = centre.to_light_years_f64();
            for i in 0..points {
                let [px, py, pz] = forward.at_ly((f64::from(i) + 0.5) * step);
                let r_sq = (px - cx) * (px - cx) + (py - cy) * (py - cy) + (pz - cz) * (pz - cz);
                sum += crate::galaxy::gas::modifiers::plummer(central, core, r_sq.sqrt());
            }
            let brute = sum * step * forward.length_ly * CENTIMETRES_PER_LIGHT_YEAR;
            assert!(
                (column / brute - 1.0).abs() < 1e-6,
                "{column} against {brute}"
            );
        }
        let through = Line::new(
            &at([12_345.6, -789.0, -40_000.0]),
            &at([12_345.6, -789.0, 40_000.0]),
        );
        let column = plummer_column(&through, &centre, core, central) / CENTIMETRES_PER_LIGHT_YEAR;
        assert!(
            (column / (4.0 / 3.0 * core * central) - 1.0).abs() < 1e-9,
            "{column}"
        );
    }

    /// Far above the disc, where only the corona is, a wall puts its density and dust in place of
    /// the corona's along its shell's chords: twice its thickness through the centre, the outer
    /// sphere's chord where the line misses the inner, the part inside the shell for a segment
    /// that ends in it, nothing for a line that misses it, and nothing inside a hole. Its column is
    /// neutral, the same both ways, and a profile reads it rising across the wall.
    #[test]
    fn a_wall_replaces_the_field_along_its_shells_chords() {
        let field = milky_way(Seed::new(5));
        let corona = field.smooth().corona_density();
        let centre = [1_000.0, 26_000.0, 40_000.0];
        let wall = GasModifier::Wall {
            centre: at(centre),
            inner: LightYears::new(90.0),
            outer: LightYears::new(100.0),
            density: HydrogenPerCm3::new(3.0),
            dust_per_hydrogen: 0.8,
        };
        let cm = CENTIMETRES_PER_LIGHT_YEAR;
        let [cx, cy, cz] = centre;
        let half = |miss: f64, r: f64| (r * r - miss * miss).sqrt();
        let tolerance = 1e-9 * 3.0 * 20.0 * cm;
        for (a, b, chord) in [
            ([cx - 500.0, cy, cz], [cx + 500.0, cy, cz], 20.0),
            (
                [cx - 500.0, cy + 95.0, cz],
                [cx + 500.0, cy + 95.0, cz],
                2.0 * half(95.0, 100.0),
            ),
            (
                [cx - 500.0, cy + 60.0, cz],
                [cx + 500.0, cy + 60.0, cz],
                2.0 * (half(60.0, 100.0) - half(60.0, 90.0)),
            ),
            ([cx - 500.0, cy, cz], [cx + 95.0, cy, cz], 15.0),
            (
                [cx - 500.0, cy + 101.0, cz],
                [cx + 500.0, cy + 101.0, cz],
                0.0,
            ),
        ] {
            let mut cache = NoiseCache::with_capacity(64);
            let mut line = |a, b, mods: &[GasModifier]| {
                sightline(
                    &field,
                    &at(a),
                    &at(b),
                    NoiseMode::Mean,
                    Quality::Full,
                    mods,
                    &mut cache,
                )
            };
            let (with, without) = (line(a, b, &[wall]), line(a, b, &[]));
            assert_eq!(with, line(b, a, &[wall]));
            let gained = with.hydrogen_column().value() - without.hydrogen_column().value();
            let expected = (3.0 - corona) * chord * cm;
            assert!(
                (gained - expected).abs() <= tolerance,
                "{gained} against {expected}"
            );
            let neutral = with.neutral_hydrogen_column().value();
            assert!((neutral - 3.0 * chord * cm).abs() <= tolerance, "{neutral}");
            let dust = with.a_v().value() * HYDROGEN_COLUMN_PER_MAG;
            assert!((dust - 0.8 * 3.0 * chord * cm).abs() <= tolerance, "{dust}");
            assert_same_bits(without.a_v().value(), 0.0);
        }
    }

    /// A hole over half a wall's near chord takes that half out, and a profile reads the wall's
    /// column rising across it to the line's.
    #[test]
    fn a_hole_takes_out_the_wall_it_covers_and_a_profile_reads_the_rest() {
        let field = milky_way(Seed::new(5));
        let centre = [1_000.0, 26_000.0, 40_000.0];
        let wall = GasModifier::Wall {
            centre: at(centre),
            inner: LightYears::new(90.0),
            outer: LightYears::new(100.0),
            density: HydrogenPerCm3::new(3.0),
            dust_per_hydrogen: 0.8,
        };
        let cm = CENTIMETRES_PER_LIGHT_YEAR;
        let [cx, cy, cz] = centre;
        let tolerance = 1e-9 * 3.0 * 20.0 * cm;
        let hole = GasModifier::Hole {
            centre: at([cx - 97.5, cy, cz]),
            radius: LightYears::new(2.5),
            interior: HydrogenPerCm3::new(0.004),
        };
        let mut cache = NoiseCache::with_capacity(64);
        let (a, b) = (at([cx - 500.0, cy, cz]), at([cx - 50.0, cy, cz]));
        let holed = sightline(
            &field,
            &a,
            &b,
            NoiseMode::Mean,
            Quality::Full,
            &[wall, hole],
            &mut cache,
        );
        let dust = holed.a_v().value() * HYDROGEN_COLUMN_PER_MAG;
        assert!((dust - 0.8 * 3.0 * 5.0 * cm).abs() <= tolerance, "{dust}");
        let from = at([cx - 500.0, cy, cz]);
        let mut out = Vec::new();
        profile(
            &field,
            &from,
            UnitVector::X,
            &[
                LightYears::new(300.0),
                LightYears::new(405.0),
                LightYears::new(1_000.0),
            ],
            NoiseMode::Mean,
            Quality::Full,
            &[wall],
            &mut cache,
            &mut out,
        );
        let whole = sightline(
            &field,
            &from,
            &at([cx + 500.0, cy, cz]),
            NoiseMode::Mean,
            Quality::Full,
            &[wall],
            &mut cache,
        );
        assert_same_bits(out[0].value(), 0.0);
        let into = 0.8 * 3.0 * 5.0 * cm / HYDROGEN_COLUMN_PER_MAG;
        assert!(((out[1].value() - into) / into).abs() < 1e-12, "{out:?}");
        let (last, all) = (out[2].value(), whole.a_v().value());
        assert!(((last - all) / all).abs() < 1e-12, "{last} against {all}");
    }

    /// The canonical order is by cell, then by offset, and a point never precedes itself.
    #[test]
    fn the_canonical_order_is_by_cell_then_offset() {
        let a = at([10.0, 20.0, 30.5]);
        let b = at([10.0, 20.0, 30.75]);
        let c = at([-5.0, 99.0, 0.0]);
        assert!(precedes(&a, &b) && !precedes(&b, &a));
        assert!(precedes(&c, &a) && !precedes(&a, &c));
        assert!(!precedes(&a, &a));
    }
}
