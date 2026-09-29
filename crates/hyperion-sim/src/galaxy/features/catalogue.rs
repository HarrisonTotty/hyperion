//! The feature catalogue: features placed on a 4,096 ly grid by the same thinning as the systems
//! (plan 09, P09.T3 and P09.T4; Design notes 1, 3, 6, 19, 21 and 23).
//!
//! # Densities
//!
//! Each [`FeatureProcess`] has a closed-form number density of living features, features per cubic
//! light-year ([`FeatureCatalogue::density`]):
//!
//! - **Globulars**: 0 until phase 3 supplies their cored `r^−3.5` (P09.T12).
//! - **Old open clusters of sub-disc k**: the sub-disc's density times its living clusters per
//!   system, [`FeatureShares::clusters_alive_per_system`], `f_n Γ_b (m̄_f ÷ m̄_n) ∫ p_k S`. A
//!   candidate then draws its age from the sub-disc's ages *given* that it is alive, by rejection
//!   on its own stream, and its mass above the least mass alive at that age.
//! - **Nurseries**: the young disc's density, with its sharp arms, times the living nurseries per
//!   young system, [`NurseryRates::alive_per_system`]. A candidate draws its age, mass, bound mark,
//!   embedded duration and dissolution age given that it is alive at the epoch, by rejection.
//! - **Clouds**: plan 07's smooth gas, `ε n_n + 9 n_mol` ([`GasField::mean_cloud_gas`]), as a mass
//!   density over the mean cloud mass, in two parts thinned apart (below).
//!
//! The conditional draws make at most 256 attempts; a candidate that exhausts them, with
//! probability under 10⁻¹⁴, is rejected, the draws' only bias.
//!
//! # Proposal and bound (Design note 21)
//!
//! A 4,096 ly cell is many disc scale heights tall, so the disc processes propose a candidate's
//! height from `g(z) ∝ exp(−|z| ÷ H)` truncated to the cell; H is the process's largest scale
//! height, the disc component's effective height or the neutral layer's. A candidate is accepted
//! with probability `ρ ÷ (B′ exp(−(|z| − z₀) ÷ H))`, with `z₀` the cell's height nearest the plane
//! and `B′` a bound on `ρ exp((|z| − z₀) ÷ H)` over its support. That is thinning with a
//! non-uniform proposal, exact by the same theorem.
//!
//! - **Columns.** In x and y the disc processes and the neutral clouds propose from 256 columns of
//!   256 ly, each with its own bound: the column is chosen by its bound, the point is uniform in
//!   it. A whole cell's bound would take a sharp arm's ridge and the cell's nearest radius for all
//!   of it and waste some nine candidates in ten; the columns waste about one in three, which is
//!   what keeps the fullest cell of the heaviest galaxy under the 16,384 index.
//! - **The gas** is exponential in height (ruling 2 of 2026-09-22), so each layer's ratio to the
//!   proposal peaks at `z₀` when no layer is taller than H, and a column's `B′` is plan 07's cell
//!   bound ([`GasField::cloud_gas_bound`]) as the design note has it.
//! - **The stellar discs are cored in height** (the density rulings of 2026-09-21), flat at the
//!   plane, so the ratio first *rises* with height and the design note's "ratio at the height
//!   nearest the plane" is no bound for them (ruling 2 foresaw that plan 09 must re-bound what
//!   follows the stellar discs). A column's `B′` is instead plan 02's bound over the column at the
//!   plane times the greatest `exp((z − z₀) ÷ H − E(z))` over the cell's heights, which
//!   [`VerticalProfile::max_ratio_to_exponential`] finds exactly on the profile's table.
//! - **The molecular clouds** follow plan 07's molecular disc, `exp(−R ÷ L − |z| ÷ h)` of a few
//!   hundred light-years, nine times over (Design note 19). A cell's bound at its nearest corner
//!   would put thousands of candidates in each of the centre's cells, so this part is proposed
//!   exponentially in |x| and |y| as well, with scale √2 L, since `R ≥ (|x| + |y|) ÷ √2`; it wastes
//!   8 ÷ 2π of its candidates. The two parts' counts are drawn one after the other on the cloud
//!   process's cell stream, the neutral part's candidates first: a superposition of two Poisson
//!   processes, which is the process.
//!
//! Globulars will keep the uniform proposal. Every bound carries a relative margin of 10⁻⁹ over
//! the rounding of its factors, and debug builds assert that no density exceeds its envelope.
//!
//! # Index space (Design note 6)
//!
//! Each process draws its candidate count on its own cell tag, and a candidate's 14-bit index is
//! its number plus the counts of the processes before it, in [`FeatureProcess::ALL`]'s order.
//! Resolving an ID recomputes at most eight counts. The total is clamped to 16,384 with a debug
//! assertion, as plan 03 clamps a cell; P09.T3.b's test asserts the headroom.
//!
//! # Streams (Design note 23)
//!
//! A cell's counts are drawn on `ObjectKey::cell(word)`, the word being candidate 0's object word,
//! one tag per process. A candidate's position, acceptance and marks are drawn on
//! `ObjectKey::feature(FeatureRef::object_word())`, each on its own tag, so a candidate keeps its
//! streams whatever the other candidates do.

use std::sync::Arc;

use crate::coords::{GalacticPosition, GalacticVelocity, LyCell};
use crate::galaxy::ages::SubDisc;
use crate::galaxy::bounds::CellBox;
use crate::galaxy::fields::{Component, ComponentId, Shape};
use crate::galaxy::gas::SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY;
#[cfg(doc)]
use crate::galaxy::gas::field::GasField;
use crate::galaxy::gas::smooth::GasLayer;
use crate::galaxy::kinematics::draw_on;
use crate::galaxy::snr::window::{standard_normal_cdf, truncated_standard_normal};
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::{FeatureCell, FeatureRef};
use crate::math;
use crate::rng::{DomainTag, ObjectKey, PowerLaw, Stream, Threshold, tags};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{Dex, HydrogenPerCm3, KilometresPerSecond, LightYears, SolarMasses, Years};

use super::ids::{FeatureId, FeatureKind, FeatureProcess};
use super::kinds::cloud::{CLOUD_MASS_MAX, CLOUD_MASS_MIN, CLOUD_MASS_SLOPE, CloudMarks};
use super::kinds::globular::{
    ACCRETED_AGES, CORE_LAW, GlobularMarks, HALF_MASS_RADIUS_LAW, IN_SITU_AGE, METALLICITY_LAWS,
    origin_component,
};
use super::kinds::nursery::{
    AGE_SPREAD_MAX_MYR, BLOW_OUT_HEIGHTS, BOUND_FRACTION, BUBBLE_INTERIOR_MEDIAN,
    BUBBLE_INTERIOR_SIGMA_DEX, BUBBLE_INTERIOR_TRUNCATION, DISSOLUTION_AGE_MYR, EFFICIENCY_RANGE,
    EMBEDDED_DURATION_MYR, EXPANSION_SPEED_KM_S, NurseryMarks, NurseryStage, SIZE_RANGE_LY,
};
use super::kinds::open_cluster::{
    CONCENTRATION_RANGE, HALF_MASS_RADIUS_AT_1E4, NurseryMassFunction, OpenClusterMarks,
    half_mass_radius, least_surviving_mass, surviving_number_fraction,
};
#[cfg(doc)]
use super::shares::{FeatureShares, NurseryRates};
use crate::galaxy::consts::YEARS_PER_MEGAYEAR;
#[cfg(doc)]
use crate::galaxy::fields::VerticalProfile;

/// The farthest any catalogue feature reaches from its centre, 4,096 ly: a lookup visits the cells
/// within its radius plus this. No record's [`reach`](FeatureRecord::reach) exceeds it.
pub const MAX_FEATURE_REACH: LightYears = LightYears::new(4_096.0);

/// A feature cell's edge, ly.
const EDGE: f64 = 4_096.0;

/// A feature cell's index capacity: 2¹⁴ candidates.
pub const INDEX_CAPACITY: u32 = 1 << 14;

/// The relative margin every feature bound carries (module documentation).
const BOUND_MARGIN: f64 = 1e-9;

/// An open cluster's reach, in half-mass radii (ours), or its tidal radius where that is farther.
const CLUSTER_REACH_RADII: f64 = 10.0;

/// A cloud's reach, in core radii: its Plummer column there is 10⁻⁴ of the central one.
const CLOUD_REACH_CORES: f64 = 10.0;

/// A feature's marks, by the process that drew it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeatureMarks {
    /// A globular cluster's (P09.T12).
    Globular(GlobularMarks),
    /// An old open cluster's.
    OpenCluster(OpenClusterMarks),
    /// A nursery's, which is a star-forming region, a young cluster or an association by time.
    Nursery(NurseryMarks),
    /// A cloud's.
    Cloud(CloudMarks),
}

/// A feature of the catalogue: its ID, process, position at the epoch and marks (P09.T3.b).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatureRecord {
    id: FeatureId,
    process: FeatureProcess,
    position: GalacticPosition,
    component: Option<ComponentId>,
    marks: FeatureMarks,
    reach: LightYears,
}

impl FeatureRecord {
    /// Its ID.
    #[must_use]
    pub const fn id(&self) -> FeatureId {
        self.id
    }

    /// The process that placed it.
    #[must_use]
    pub const fn process(&self) -> FeatureProcess {
        self.process
    }

    /// Its centre at the epoch.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// The density component it follows: the sub-disc for an old cluster, the young disc for a
    /// nursery, none for a cloud.
    #[must_use]
    pub const fn component(&self) -> Option<ComponentId> {
        self.component
    }

    /// Its marks.
    #[must_use]
    pub const fn marks(&self) -> &FeatureMarks {
        &self.marks
    }

    /// The farthest it reaches from its centre at any time: for an old open cluster ten half-mass
    /// radii or its tidal radius, whichever is farther, for a globular four tidal radii, for a
    /// nursery the greater of its largest size and its bubble's blow-out radius, ten core radii for
    /// a cloud. At most [`MAX_FEATURE_REACH`]. Every member lies within it (P09.T21).
    #[must_use]
    pub const fn reach(&self) -> LightYears {
        self.reach
    }

    /// Its age at the epoch, or `None` for a cloud.
    #[must_use]
    pub const fn age_at_epoch(&self) -> Option<Years> {
        match &self.marks {
            FeatureMarks::Globular(marks) => Some(marks.age()),
            FeatureMarks::OpenCluster(marks) => Some(marks.age_at_epoch()),
            FeatureMarks::Nursery(marks) => Some(marks.age_at_epoch()),
            FeatureMarks::Cloud(_) => None,
        }
    }

    /// What it is `t` years after the epoch, or `None` once it has dissolved (Design note 1).
    #[must_use]
    pub fn kind_at(&self, t: Years) -> Option<FeatureKind> {
        match &self.marks {
            FeatureMarks::Globular(_) => Some(FeatureKind::Globular),
            FeatureMarks::OpenCluster(marks) => (marks.age_at_epoch().value() + t.value()
                < marks.dissolution_time().value())
            .then_some(FeatureKind::OpenCluster),
            FeatureMarks::Nursery(marks) => match marks.stage_at(t) {
                NurseryStage::Embedded => Some(FeatureKind::StarFormingRegion),
                NurseryStage::BoundCluster => Some(FeatureKind::OpenCluster),
                NurseryStage::Association => Some(FeatureKind::Association),
                NurseryStage::Unborn | NurseryStage::Dissolved => None,
            },
            FeatureMarks::Cloud(_) => Some(FeatureKind::MolecularCloud),
        }
    }
}

/// Everything a feature cell holds: its candidate counts per process and its features in index
/// order.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureCellContents {
    cell: FeatureCell,
    counts: [u32; 8],
    features: Vec<FeatureRecord>,
}

impl FeatureCellContents {
    /// The cell.
    #[must_use]
    pub const fn cell(&self) -> FeatureCell {
        self.cell
    }

    /// The candidate counts of each process, in [`FeatureProcess::ALL`]'s order.
    #[must_use]
    pub const fn counts(&self) -> [u32; 8] {
        self.counts
    }

    /// The accepted features, in index order.
    #[must_use]
    pub fn features(&self) -> &[FeatureRecord] {
        &self.features
    }
}

/// Where a caller keeps feature cells between lookups (P09.T3.b).
///
/// It takes `&self`, so an implementation that keeps cells uses interior mutability, and it must
/// lend each cell's contents exactly as [`FeatureCatalogue::cell`] makes them. **An implementation
/// that keeps cells between calls keys them by galaxy as well as by cell** (ruling 25 of
/// 2026-09-22, as plan 03's `CellCache` documents it): the trait does not check the galaxy.
pub trait FeatureCellCache {
    /// The contents of `cell` in `galaxy`.
    fn contents(&self, galaxy: &Galaxy, cell: FeatureCell) -> Arc<FeatureCellContents>;
}

/// The cache that keeps nothing: every lookup generates its cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoFeatureCache;

impl FeatureCellCache for NoFeatureCache {
    fn contents(&self, galaxy: &Galaxy, cell: FeatureCell) -> Arc<FeatureCellContents> {
        Arc::new(FeatureCatalogue::cell(galaxy, cell))
    }
}

/// The feature catalogue of a galaxy: pure functions of the galaxy and a cell (P09.T3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FeatureCatalogue;

/// One axis of a proposal: uniform over the cell, or exponential away from the axis's plane.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Axis {
    /// The exponential's scale, ly, or 0 for a uniform axis.
    scale: f64,
    /// The cell's least `|coordinate|` on this axis, ly.
    near: f64,
    /// Whether the cell lies on the negative side of the axis's plane.
    negative: bool,
}

impl Axis {
    /// The axis of `origin` (the cell's least coordinate, ly) with `scale`.
    fn new(origin: i32, scale: f64) -> Self {
        let negative = origin < 0;
        let near = if negative {
            f64::from(-(origin + 4_096))
        } else {
            f64::from(origin)
        };
        Self {
            scale,
            near,
            negative,
        }
    }

    /// `∫ exp(−lift ÷ scale)` over the cell's span, ly: the span itself for a uniform axis.
    fn weight(&self) -> f64 {
        if self.scale > 0.0 {
            self.scale * -math::exp_m1(-EDGE / self.scale)
        } else {
            EDGE
        }
    }
}

/// Which part of a process's density a proposal's term thins against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    /// The whole density.
    All,
    /// The clouds that follow the neutral layer.
    Neutral,
    /// The clouds that follow the molecular disc.
    Molecular,
}

/// One term of a proposal: a bound and the axes it is proposed on (module documentation).
#[derive(Debug, Clone, PartialEq)]
struct Term {
    /// `B′`: features per cubic light-year at the cell's point nearest the planes of its
    /// exponential axes, times the rest of the proposal at that point; for a column proposal, the
    /// greatest of its columns' bounds.
    bound: f64,
    axes: [Axis; 3],
    part: Part,
    /// The Poisson mean of the term's candidate count.
    mean: f64,
    /// For a column proposal, each column's bound and their running sums.
    columns: Option<Columns>,
}

/// A column proposal's columns: the cell's x–y face cut into [`COLUMNS_PER_AXIS`]² squares, each
/// with its own bound, x-major.
#[derive(Debug, Clone, PartialEq)]
struct Columns {
    bounds: Box<[f64]>,
    running: Box<[f64]>,
}

/// Columns per axis of a column proposal: 16, so 256 columns of 256 ly.
const COLUMNS_PER_AXIS: u32 = 16;

/// A column's edge, ly.
const COLUMN_EDGE: u32 = 256;

/// A finite, non-negative bound, or 0.
fn clean(bound: f64) -> f64 {
    if bound.is_finite() && bound > 0.0 {
        bound
    } else {
        0.0
    }
}

impl Term {
    fn new(bound: f64, axes: [Axis; 3], part: Part) -> Self {
        let bound = clean(bound);
        let mean = bound * axes.iter().map(Axis::weight).fold(1.0, |p, w| p * w);
        Self {
            bound,
            axes,
            part,
            mean,
            columns: None,
        }
    }

    /// A column proposal: uniform in x and y within each column, the column chosen by its bound,
    /// and `z` as `z_axis` proposes it. It follows arms and radial gradients that a whole cell's
    /// bound would flatten, so it wastes far fewer candidates (module documentation).
    fn with_columns(bounds: Vec<f64>, z_axis: Axis, x: Axis, y: Axis, part: Part) -> Self {
        let bounds: Box<[f64]> = bounds.into_iter().map(clean).collect();
        let mut sum = 0.0;
        let running: Box<[f64]> = bounds
            .iter()
            .map(|&b| {
                sum += b;
                sum
            })
            .collect();
        let face = f64::from(COLUMN_EDGE * COLUMN_EDGE);
        let mean = sum * face * z_axis.weight();
        let bound = bounds.iter().copied().fold(0.0, f64::max);
        Self {
            bound,
            axes: [x, y, z_axis],
            part,
            mean,
            columns: Some(Columns { bounds, running }),
        }
    }
}

/// How one process proposes candidates in one cell: one term, or two for the clouds, whose
/// neutral and molecular parts are thinned separately, a superposition of two Poisson processes
/// that is the process itself.
#[derive(Debug, Clone, PartialEq)]
struct Proposal {
    terms: [Option<Term>; 2],
}

impl Proposal {
    const NONE: Self = Self {
        terms: [None, None],
    };

    fn one(term: Term) -> Self {
        Self {
            terms: [Some(term), None],
        }
    }

    fn mean(&self) -> f64 {
        self.terms.iter().flatten().fold(0.0, |sum, t| sum + t.mean)
    }
}

impl FeatureCatalogue {
    /// The candidate density of `process` at `p`, features per cubic light-year (module
    /// documentation).
    #[must_use]
    pub fn density(galaxy: &Galaxy, process: FeatureProcess, p: &GalacticPosition) -> f64 {
        let point = PointLy::from(p);
        match process {
            FeatureProcess::Globular => galaxy.feature_shares().globulars().density(&point),
            FeatureProcess::OldOpenCluster(_) | FeatureProcess::Nursery => {
                let factor = disc_factor(galaxy, process);
                disc_components(galaxy, process)
                    .map(|(_, component)| component.density(&point) * factor)
                    .fold(0.0, |sum, d| sum + d)
            }
            FeatureProcess::Cloud => cloud_factor(galaxy) * cloud_gas(galaxy, p).value(),
        }
    }

    /// The expected number of candidates of `process` in `cell`: the Poisson mean its count is
    /// drawn from.
    #[must_use]
    pub fn expected_candidates(galaxy: &Galaxy, process: FeatureProcess, cell: FeatureCell) -> f64 {
        proposal(galaxy, process, cell).mean()
    }

    /// The candidate counts of every process in `cell`, in [`FeatureProcess::ALL`]'s order,
    /// clamped so that they sum to at most [`INDEX_CAPACITY`].
    ///
    /// # Panics
    ///
    /// In debug builds, if the counts would overflow the index.
    #[must_use]
    pub fn counts(galaxy: &Galaxy, cell: FeatureCell) -> [u32; 8] {
        let mut counts = [0; 8];
        let mut total = 0_u32;
        for process in FeatureProcess::ALL {
            let terms = term_counts(galaxy, process, cell, &proposal(galaxy, process, cell));
            counts[process.index()] = clamp_count(cell, process, terms, &mut total);
        }
        counts
    }

    /// Every feature of `cell`, in index order (P09.T3.b).
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::features::catalogue::FeatureCatalogue;
    /// use hyperion_sim::id::FeatureCell;
    ///
    /// let galaxy = Galaxy::new(Seed::new(9));
    /// // The feature cell above the Sun-like point: the young disc's nurseries and the clouds.
    /// let cell = FeatureCatalogue::cell(&galaxy, FeatureCell::new([0, 6, 0])?);
    /// assert!(!cell.features().is_empty());
    /// for feature in cell.features() {
    ///     assert_eq!(FeatureCatalogue::resolve(&galaxy, feature.id()).as_ref(), Some(feature));
    /// }
    /// # Ok::<(), hyperion_sim::id::BuildSystemIdError>(())
    /// ```
    #[must_use]
    pub fn cell(galaxy: &Galaxy, cell: FeatureCell) -> FeatureCellContents {
        let mut counts = [0; 8];
        let mut features = Vec::new();
        let mut total = 0_u32;
        for process in FeatureProcess::ALL {
            let proposal = proposal(galaxy, process, cell);
            let terms = term_counts(galaxy, process, cell, &proposal);
            let offset = total;
            let n = clamp_count(cell, process, terms, &mut total);
            counts[process.index()] = n;
            features.extend((0..n).filter_map(|number| {
                evaluate(
                    galaxy,
                    process,
                    cell,
                    offset + number,
                    number,
                    &proposal,
                    terms,
                )
            }));
        }
        FeatureCellContents {
            cell,
            counts,
            features,
        }
    }

    /// The feature `id` names, or `None` if its index is past its cell's candidates or its
    /// candidate was rejected: the check every ID read from a save or the protocol goes through.
    #[must_use]
    pub fn resolve(galaxy: &Galaxy, id: FeatureId) -> Option<FeatureRecord> {
        let cell = id.cell();
        let index = u32::from(id.index());
        let mut total = 0_u32;
        for process in FeatureProcess::ALL {
            let proposal = proposal(galaxy, process, cell);
            let terms = term_counts(galaxy, process, cell, &proposal);
            let offset = total;
            let n = clamp_count(cell, process, terms, &mut total);
            if index < offset + n {
                return evaluate(
                    galaxy,
                    process,
                    cell,
                    index,
                    index - offset,
                    &proposal,
                    terms,
                );
            }
        }
        None
    }

    /// The features whose reach touches the sphere of `radius` about `centre`, from the cells
    /// within `radius` + [`MAX_FEATURE_REACH`], through `cache`, in cell and index order.
    pub fn near(
        galaxy: &Galaxy,
        centre: &GalacticPosition,
        radius: LightYears,
        cache: &dyn FeatureCellCache,
    ) -> impl Iterator<Item = FeatureRecord> + use<> {
        let [cx, cy, cz] = centre.to_light_years_f64();
        let reach = radius.value() + MAX_FEATURE_REACH.value();
        let range = |c: f64| {
            let lo = (((c - reach) / EDGE).floor()).max(-16.0);
            let hi = (((c + reach) / EDGE).floor()).min(15.0);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "both ends are whole numbers clamped to −16..=15"
            )]
            (lo as i32, hi as i32)
        };
        let (x, y, z) = (range(cx), range(cy), range(cz));
        let mut found = Vec::new();
        for i in x.0..=x.1 {
            for j in y.0..=y.1 {
                for k in z.0..=z.1 {
                    let Ok(cell) = FeatureCell::new([i, j, k]) else {
                        continue;
                    };
                    if box_distance(cell, [cx, cy, cz]) > reach {
                        continue;
                    }
                    let contents = cache.contents(galaxy, cell);
                    found.extend(contents.features().iter().copied().filter(|f| {
                        f.position.distance_to(centre).value() / METRES_PER_LIGHT_YEAR
                            <= radius.value() + f.reach.value()
                    }));
                }
            }
        }
        found.into_iter()
    }

    /// Every feature of `process` in the galaxy, cell by cell over all 32,768 cells in the order
    /// x, then y, then z, lazily (plan 10 walks the globulars with it).
    ///
    /// # Panics
    ///
    /// Never: every coordinate it visits is a feature cell.
    pub fn walk_process(
        galaxy: &Galaxy,
        process: FeatureProcess,
    ) -> impl Iterator<Item = FeatureRecord> + '_ {
        (-16..16).flat_map(move |i| {
            (-16..16).flat_map(move |j| {
                (-16..16).flat_map(move |k| {
                    let cell = FeatureCell::new([i, j, k])
                        .expect("every coordinate of −16..16 is a feature cell");
                    process_features(galaxy, process, cell)
                })
            })
        })
    }
}

/// The bulk velocity of a cluster or nursery: the law of the population it follows at its centre,
/// on its own `feature.velocity` stream (P09.T4.a), or `None` for a cloud or a galaxy without
/// kinematic tables ([`Galaxy::with_full_potential`]).
#[must_use]
pub fn bulk_velocity(galaxy: &Galaxy, feature: &FeatureRecord) -> Option<GalacticVelocity> {
    let component = feature.component?;
    galaxy.kinematics()?;
    let mut stream = feature_stream(galaxy, tags::FEATURE_VELOCITY, feature.id);
    Some(draw_on(galaxy, component, &feature.position, &mut stream).velocity())
}

/// The features of one process in one cell, in index order.
fn process_features(
    galaxy: &Galaxy,
    process: FeatureProcess,
    cell: FeatureCell,
) -> Vec<FeatureRecord> {
    let mut total = 0_u32;
    for earlier in FeatureProcess::ALL {
        let proposal = proposal(galaxy, earlier, cell);
        let terms = term_counts(galaxy, earlier, cell, &proposal);
        let offset = total;
        let n = clamp_count(cell, earlier, terms, &mut total);
        if earlier == process {
            return (0..n)
                .filter_map(|number| {
                    evaluate(
                        galaxy,
                        process,
                        cell,
                        offset + number,
                        number,
                        &proposal,
                        terms,
                    )
                })
                .collect();
        }
    }
    Vec::new()
}

/// A process's candidate count from its terms' counts, clamped to the room left in the index,
/// with `total` the candidates before it, which it advances.
fn clamp_count(
    cell: FeatureCell,
    process: FeatureProcess,
    terms: [u32; 2],
    total: &mut u32,
) -> u32 {
    let n = terms[0].saturating_add(terms[1]);
    let room = INDEX_CAPACITY - *total;
    debug_assert!(
        n <= room,
        "feature cell {:?} overflows its index: {n} {process} candidates after {total}",
        cell.to_array()
    );
    let n = n.min(room);
    *total += n;
    n
}

/// The components a disc process follows, with their IDs.
fn disc_components(
    galaxy: &Galaxy,
    process: FeatureProcess,
) -> impl Iterator<Item = (ComponentId, &Component)> {
    let fields = galaxy.fields();
    fields
        .component_ids()
        .map(move |id| (id, fields.component(id)))
        .filter(move |(_, component)| match process {
            FeatureProcess::OldOpenCluster(sub_disc) => component.sub_disc() == Some(sub_disc),
            FeatureProcess::Nursery => component.population() == Population::YoungThinDisc,
            FeatureProcess::Globular | FeatureProcess::Cloud => false,
        })
}

/// Features per system of a disc process's components.
fn disc_factor(galaxy: &Galaxy, process: FeatureProcess) -> f64 {
    let shares = galaxy.feature_shares();
    match process {
        FeatureProcess::OldOpenCluster(sub_disc) => shares.clusters_alive_per_system(sub_disc),
        FeatureProcess::Nursery => shares.nurseries().alive_per_system(),
        FeatureProcess::Globular | FeatureProcess::Cloud => 0.0,
    }
}

/// Clouds per cubic light-year per hydrogen per cm³ of the gas they follow.
fn cloud_factor(galaxy: &Galaxy) -> f64 {
    SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY * galaxy.feature_shares().clouds_per_solar_mass()
}

/// The gas the clouds follow at `p`.
fn cloud_gas(galaxy: &Galaxy, p: &GalacticPosition) -> HydrogenPerCm3 {
    let (share, multiple) = galaxy.feature_shares().cloud_weights();
    galaxy.gas().mean_cloud_gas(p, share, multiple)
}

/// The cell's least `|z|`, ly: the height of its face nearer the plane.
fn z_near(cell: FeatureCell) -> f64 {
    let oz = cell.origin().to_array()[2];
    if oz < 0 {
        f64::from(-(oz + 4_096))
    } else {
        f64::from(oz)
    }
}

/// The proposal of `process` in `cell` (module documentation).
fn proposal(galaxy: &Galaxy, process: FeatureProcess, cell: FeatureCell) -> Proposal {
    let z_near = z_near(cell);
    let [ox, oy, oz] = cell.origin().to_array();
    match process {
        FeatureProcess::Globular => {
            // The globulars' law never rises with |x|, |y| or |z| and its cuts are indicators that
            // do not either, so the cell's nearest corner bounds it (module documentation).
            let cube = CellBox::new(cell.origin().to_array(), FeatureCell::EDGE_LY)
                .expect("a feature cell is a cell box of the root cube");
            let bound = galaxy
                .feature_shares()
                .globulars()
                .density(&cube.nearest_corner())
                * (1.0 + BOUND_MARGIN);
            let axes = [Axis::new(ox, 0.0), Axis::new(oy, 0.0), Axis::new(oz, 0.0)];
            Proposal::one(Term::new(bound, axes, Part::All))
        }
        FeatureProcess::OldOpenCluster(_) | FeatureProcess::Nursery => {
            let factor = disc_factor(galaxy, process);
            let height = disc_components(galaxy, process)
                .map(|(_, c)| disc_height(c))
                .fold(0.0, f64::max);
            if height <= 0.0 || factor <= 0.0 {
                return Proposal::NONE;
            }
            let rate = 1.0 / height;
            let ratios: Vec<(&Component, f64)> = disc_components(galaxy, process)
                .map(|(_, c)| {
                    let Shape::Disc(disc) = c.shape() else {
                        unreachable!("a disc process follows discs")
                    };
                    let ratio =
                        disc.profile()
                            .max_ratio_to_exponential(z_near, z_near + EDGE, rate)
                            * math::exp(-rate * z_near);
                    (c, ratio)
                })
                .collect();
            let plane_z = if oz < 0 { -COLUMN_EDGE_I32 } else { 0 };
            let bounds = column_boxes(ox, oy, plane_z)
                .map(|column| {
                    ratios
                        .iter()
                        .fold(0.0, |sum, (c, ratio)| sum + c.bound(&column) * ratio)
                        * factor
                        * (1.0 + BOUND_MARGIN)
                })
                .collect();
            Proposal::one(Term::with_columns(
                bounds,
                Axis::new(oz, height),
                Axis::new(ox, 0.0),
                Axis::new(oy, 0.0),
                Part::All,
            ))
        }
        FeatureProcess::Cloud => {
            let (share, multiple) = galaxy.feature_shares().cloud_weights();
            let gas = galaxy.gas();
            let factor = cloud_factor(galaxy) * (1.0 + BOUND_MARGIN);
            let neutral_height = gas.smooth().height(GasLayer::Neutral);
            let near_z = if oz < 0 {
                oz + 4_096 - COLUMN_EDGE_I32
            } else {
                oz
            };
            let bounds = column_boxes(ox, oy, near_z)
                .map(|column| gas.cloud_gas_bound(&column, share, 0.0).value() * factor)
                .collect();
            let neutral = Term::with_columns(
                bounds,
                Axis::new(oz, neutral_height),
                Axis::new(ox, 0.0),
                Axis::new(oy, 0.0),
                Part::Neutral,
            );
            // The molecular disc is exp(−R ÷ L − |z| ÷ h), and R ≥ (|x| + |y|) ÷ √2, so an
            // exponential proposal in |x| and |y| of scale √2 L and in |z| of scale h bounds it
            // everywhere, wasting only 8 ÷ 2π of its candidates.
            let length = gas.params().molecular_disc().length().value();
            let height = gas.smooth().height(GasLayer::Molecular);
            let scale = core::f64::consts::SQRT_2 * length;
            let axes = [
                Axis::new(ox, scale),
                Axis::new(oy, scale),
                Axis::new(oz, height),
            ];
            let amplitude = gas.smooth().plane_density(GasLayer::Molecular, 0.0);
            let lift = (axes[0].near + axes[1].near) / scale + axes[2].near / height;
            let molecular = Term::new(
                multiple * amplitude * math::exp(-lift) * factor * (1.0 + BOUND_MARGIN),
                axes,
                Part::Molecular,
            );
            Proposal {
                terms: [Some(neutral), Some(molecular)],
            }
        }
    }
}

/// A column's edge as an `i32`, ly.
const COLUMN_EDGE_I32: i32 = 256;

/// The column boxes of a cell whose least x and y are `ox` and `oy`, each a 256 ly cube whose
/// least z is `z`, x-major: the boxes a column proposal bounds.
fn column_boxes(ox: i32, oy: i32, z: i32) -> impl Iterator<Item = CellBox> {
    let n = i32::try_from(COLUMNS_PER_AXIS).expect("sixteen fits in i32");
    (0..n).flat_map(move |i| {
        (0..n).map(move |j| {
            CellBox::new(
                [ox + COLUMN_EDGE_I32 * i, oy + COLUMN_EDGE_I32 * j, z],
                COLUMN_EDGE,
            )
            .expect("a column of a feature cell is a cell box of the root cube")
        })
    })
}

/// A disc component's effective height, ly, or 0 for another shape.
fn disc_height(component: &Component) -> f64 {
    match component.shape() {
        Shape::Disc(disc) => disc.profile().effective_height().value(),
        Shape::Bulge(_) | Shape::Bar(_) | Shape::Halo(_) => 0.0,
    }
}

/// The cell tag of a process's candidate count.
fn count_tag(process: FeatureProcess) -> DomainTag {
    match process {
        FeatureProcess::Globular => tags::FEATURE_CELL_GLOBULAR,
        FeatureProcess::OldOpenCluster(SubDisc::First) => tags::FEATURE_CELL_OPEN_CLUSTER_1,
        FeatureProcess::OldOpenCluster(SubDisc::Second) => tags::FEATURE_CELL_OPEN_CLUSTER_2,
        FeatureProcess::OldOpenCluster(SubDisc::Third) => tags::FEATURE_CELL_OPEN_CLUSTER_3,
        FeatureProcess::OldOpenCluster(SubDisc::Fourth) => tags::FEATURE_CELL_OPEN_CLUSTER_4,
        FeatureProcess::OldOpenCluster(SubDisc::Fifth) => tags::FEATURE_CELL_OPEN_CLUSTER_5,
        FeatureProcess::Nursery => tags::FEATURE_CELL_NURSERY,
        FeatureProcess::Cloud => tags::FEATURE_CELL_CLOUD,
    }
}

/// The word that keys a cell's streams: candidate 0's object word.
fn cell_word(cell: FeatureCell) -> u64 {
    FeatureRef::new(cell, 0)
        .expect("index 0 is in every feature cell")
        .object_word()
}

/// The candidate counts of a process's terms in `cell`, before the index's clamp: Poisson draws
/// on the process's cell stream, one per term in order.
fn term_counts(
    galaxy: &Galaxy,
    process: FeatureProcess,
    cell: FeatureCell,
    proposal: &Proposal,
) -> [u32; 2] {
    if proposal.mean() <= 0.0 {
        return [0, 0];
    }
    let mut stream = Stream::open(
        galaxy.seed(),
        count_tag(process),
        ObjectKey::cell(cell_word(cell)),
    );
    proposal.terms.each_ref().map(|term| match term {
        Some(term) if term.mean > 0.0 => {
            u32::try_from(stream.poisson(term.mean)).unwrap_or(u32::MAX)
        }
        Some(_) | None => 0,
    })
}

/// A candidate's stream on `tag`.
fn feature_stream(galaxy: &Galaxy, tag: DomainTag, id: FeatureId) -> Stream {
    Stream::open(galaxy.seed(), tag, ObjectKey::feature(id.object_word()))
}

/// `2⁻⁵²`.
const TWO_POW_MINUS_52: f64 = 1.0 / 4_503_599_627_370_496.0;

/// `2⁻⁵³`.
const TWO_POW_MINUS_53: f64 = 1.0 / 9_007_199_254_740_992.0;

/// A whole light-year of a span of `2^log2` ly and its fraction from one word: the top `log2`
/// bits and the next 52, as plan 01's `GenCell::position_from_words` lays out a generation cell.
fn uniform_ly(word: u64, log2: u32) -> (i32, f64) {
    let ly = i32::try_from(word >> (64 - log2)).expect("at most twelve bits fit in i32");
    #[expect(
        clippy::cast_precision_loss,
        reason = "a 52-bit fraction is exact in f64"
    )]
    let fraction = ((word << log2) >> 12) as f64 * TWO_POW_MINUS_52;
    (ly, fraction)
}

/// A candidate's position under `term`, its lifts along the term's exponential axes, ly, and the
/// bound of the column it lies in (the term's own bound if it has no columns).
///
/// Words 0, 1 and 2 of `feature.position` give x, y and z, and for a column proposal word 3 a
/// 53-bit uniform that picks the column by the running sum of their bounds. A uniform axis takes
/// the top twelve bits (eight inside a column) as the whole light-year and the next 52 as the
/// fraction; an exponential one takes a 53-bit
/// uniform `u` to the lift `−s ln(1 + u (e^(−Δ ÷ s) − 1))` away from the axis's plane, in [0, Δ)
/// on the positive side and, with `1 − u`, in (0, Δ] on the negative side, so that the cell's face
/// that belongs to its neighbour is never reached.
fn candidate_position(
    galaxy: &Galaxy,
    cell: FeatureCell,
    id: FeatureId,
    term: &Term,
) -> (GalacticPosition, [f64; 3], f64) {
    let mut stream = feature_stream(galaxy, tags::FEATURE_POSITION, id);
    let mut origin = cell.origin().to_array();
    let words = [stream.next_u64(), stream.next_u64(), stream.next_u64()];
    let mut bound = term.bound;
    let mut column_log2 = 12;
    if let Some(columns) = &term.columns {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a 53-bit integer is exact in f64"
        )]
        let u = (stream.next_u64() >> 11) as f64 * TWO_POW_MINUS_53;
        let total = columns.running.last().copied().unwrap_or(0.0);
        let target = u * total;
        let i = columns
            .running
            .partition_point(|&c| c <= target)
            .min(columns.running.len() - 1);
        bound = columns.bounds[i];
        let n = usize::try_from(COLUMNS_PER_AXIS).expect("sixteen fits in usize");
        let (ci, cj) = (i / n, i % n);
        origin[0] += COLUMN_EDGE_I32 * i32::try_from(ci).expect("a column index fits in i32");
        origin[1] += COLUMN_EDGE_I32 * i32::try_from(cj).expect("a column index fits in i32");
        column_log2 = 8;
    }
    let mut ly = [0_i32; 3];
    let mut fraction = [0.0; 3];
    let mut lifts = [0.0; 3];
    let cell_origin = cell.origin().to_array();
    for axis in 0..3 {
        let word = words[axis];
        let spec = term.axes[axis];
        if spec.scale > 0.0 {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a 53-bit integer is exact in f64"
            )]
            let u = (word >> 11) as f64 * TWO_POW_MINUS_53;
            let u = if spec.negative { 1.0 - u } else { u };
            let s = spec.scale;
            let lift = (-s * math::ln_1p(u * math::exp_m1(-EDGE / s))).clamp(0.0, EDGE);
            let c = if spec.negative {
                -(spec.near + lift)
            } else {
                spec.near + lift
            };
            let whole = c.floor();
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a whole light-year inside the root cube fits in i32"
            )]
            let mut whole_ly = whole as i32;
            let mut part = c - whole;
            let top = cell_origin[axis] + 4_095;
            if whole_ly > top {
                whole_ly = top;
                part = 1.0 - f64::EPSILON;
            } else if whole_ly < cell_origin[axis] {
                whole_ly = cell_origin[axis];
                part = 0.0;
            }
            ly[axis] = whole_ly;
            fraction[axis] = part;
            lifts[axis] = lift;
        } else {
            let log2 = if axis < 2 { column_log2 } else { 12 };
            let (whole, part) = uniform_ly(word, log2);
            ly[axis] = origin[axis] + whole;
            fraction[axis] = part;
        }
    }
    let offset = fraction
        .map(|f| (f * METRES_PER_LIGHT_YEAR).min(METRES_PER_LIGHT_YEAR * (1.0 - f64::EPSILON)));
    let position = GalacticPosition::new(LyCell::new(ly), offset)
        .expect("a light-year of the root cube and an offset inside it is a position");
    (position, lifts, bound)
}

/// The density a term thins against at `p`, features per cubic light-year.
fn term_density(galaxy: &Galaxy, process: FeatureProcess, part: Part, p: &GalacticPosition) -> f64 {
    let (share, multiple) = galaxy.feature_shares().cloud_weights();
    match part {
        Part::All => FeatureCatalogue::density(galaxy, process, p),
        Part::Neutral => cloud_factor(galaxy) * galaxy.gas().mean_cloud_gas(p, share, 0.0).value(),
        Part::Molecular => {
            cloud_factor(galaxy) * galaxy.gas().mean_cloud_gas(p, 0.0, multiple).value()
        }
    }
}

/// Candidate `index` of `cell`, number `number` of `process`, or `None` if the thinning or its
/// marks reject it. `terms` are the process's terms' counts: the first term's candidates come
/// first.
fn evaluate(
    galaxy: &Galaxy,
    process: FeatureProcess,
    cell: FeatureCell,
    index: u32,
    number: u32,
    proposal: &Proposal,
    terms: [u32; 2],
) -> Option<FeatureRecord> {
    let term = if number < terms[0] {
        proposal.terms[0].as_ref()
    } else {
        proposal.terms[1].as_ref()
    }?;
    if term.bound <= 0.0 {
        return None;
    }
    let id = FeatureId::of(cell, u16::try_from(index).ok()?)?;
    let (position, lifts, bound) = candidate_position(galaxy, cell, id, term);
    let density = term_density(galaxy, process, term.part, &position);
    let lift: f64 = term
        .axes
        .iter()
        .zip(lifts)
        .filter(|(axis, _)| axis.scale > 0.0)
        .map(|(axis, lift)| lift / axis.scale)
        .sum();
    let envelope = bound * math::exp(-lift);
    debug_assert!(
        density <= envelope,
        "the {process} density {density} at {:?} is above its bound {envelope} in feature cell {:?}",
        position.to_light_years_f64(),
        cell.to_array()
    );
    if envelope <= 0.0 {
        return None;
    }
    let mark = feature_stream(galaxy, tags::FEATURE_ACCEPT, id).mark();
    if !mark.is_below(Threshold::from_ratio(density.min(envelope), envelope)) {
        return None;
    }
    let point = PointLy::from(&position);
    match process {
        FeatureProcess::Globular => Some(globular(galaxy, id, position, &point)),
        FeatureProcess::OldOpenCluster(sub_disc) => {
            old_cluster(galaxy, sub_disc, id, position, &point)
        }
        FeatureProcess::Nursery => nursery(galaxy, id, position, &point),
        FeatureProcess::Cloud => Some(cloud(galaxy, id, position)),
    }
}

/// The component a disc candidate at `point` belongs to: the first of the process's components by
/// the odds of their densities there, with a uniform `u`.
fn pick_component<'g>(
    galaxy: &'g Galaxy,
    process: FeatureProcess,
    point: &PointLy,
    u: f64,
) -> Option<(ComponentId, &'g Component)> {
    let total = disc_components(galaxy, process)
        .map(|(_, c)| c.density(point))
        .fold(0.0, |sum, d| sum + d);
    let mut running = 0.0;
    let mut last = None;
    for (id, c) in disc_components(galaxy, process) {
        running += c.density(point);
        last = Some((id, c));
        if u * total < running {
            return last;
        }
    }
    last
}

/// The attempts a candidate's conditional draw makes before it gives up: 256. The draw that is
/// conditioned on living accepts at least one attempt in eight at every sub-disc and age (the
/// nurseries about seven in ten), so a candidate exhausts them with probability under 10⁻¹⁴, and
/// that is the only bias of the conditional draws: such a candidate is rejected.
const CONDITIONAL_ATTEMPTS: u64 = 256;

/// The words an old cluster's age attempt takes: the age and its acceptance mark.
const CLUSTER_ATTEMPT_WORDS: u64 = 2;

/// The words a nursery's life attempt takes: age, mass, bound mark, embedded duration and
/// dissolution age.
const NURSERY_ATTEMPT_WORDS: u64 = 5;

/// An old open cluster's marks (P09.T4.a).
///
/// Word 0 of `feature.open_cluster` picks the component, if the process has several. The age is
/// then drawn from the sub-disc's ages conditional on the cluster's being alive, by rejection:
/// attempt `k` reads words `1 + 2k` and `2 + 2k`, an age from the sub-disc's distribution and a
/// mark that keeps it with odds `S(a) ÷ S(a_lo)`. The mass is drawn above the least surviving mass
/// at that age, so the cluster is alive, and the other marks follow after the attempts' words.
fn old_cluster(
    galaxy: &Galaxy,
    sub_disc: SubDisc,
    id: FeatureId,
    position: GalacticPosition,
    point: &PointLy,
) -> Option<FeatureRecord> {
    let process = FeatureProcess::OldOpenCluster(sub_disc);
    let mut stream = feature_stream(galaxy, tags::FEATURE_OPEN_CLUSTER, id);
    let pick = stream.uniform();
    let (component_id, component) = pick_component(galaxy, process, point, pick)?;
    let ceiling = galaxy.feature_shares().youngest_survival(sub_disc);
    if ceiling <= 0.0 {
        return None;
    }
    let age = (0..CONDITIONAL_ATTEMPTS).find_map(|k| {
        stream.seek(1 + CLUSTER_ATTEMPT_WORDS * k);
        let age = component.ages().sample(&mut stream);
        let survival = surviving_number_fraction(age).min(ceiling);
        stream
            .mark()
            .is_below(Threshold::from_ratio(survival, ceiling))
            .then_some(age)
    })?;
    stream.seek(1 + CLUSTER_ATTEMPT_WORDS * CONDITIONAL_ATTEMPTS);
    let mass =
        NurseryMassFunction::STANDARD.quantile_above(least_surviving_mass(age), stream.uniform());
    let radius = half_mass_radius(mass, stream.standard_normal());
    let concentration = stream.uniform_in(CONCENTRATION_RANGE.0, CONCENTRATION_RANGE.1);
    let metallicity = component.metallicity(point, age);
    let marks = OpenClusterMarks::new(
        age,
        mass,
        (radius, concentration),
        &metallicity,
        stream.standard_normal(),
    );
    Some(FeatureRecord {
        id,
        process,
        position,
        component: Some(component_id),
        marks: FeatureMarks::OpenCluster(marks),
        reach: LightYears::new(
            (CLUSTER_REACH_RADII * radius.value())
                .max(present_tidal_radius_ly(galaxy, &position, mass, age))
                .min(MAX_FEATURE_REACH.value()),
        ),
    })
}

/// An open cluster's tidal radius today, ly: its present mass's at its centre, the radius its
/// members' profiles end at ([`ClusterModel`](super::cluster::ClusterModel)'s, computed the same
/// way), which its reach must hold for the members' lookup (P09.T23).
#[must_use]
fn present_tidal_radius_ly(
    galaxy: &Galaxy,
    position: &GalacticPosition,
    initial_mass: SolarMasses,
    age: Years,
) -> f64 {
    let mass = super::kinds::open_cluster::present_mass(initial_mass, age);
    if mass.value() <= 0.0 {
        return 0.0;
    }
    galaxy
        .potential()
        .tidal_radius(mass, &PointLy::from(position))
        .value()
        / METRES_PER_LIGHT_YEAR
}

/// A nursery's marks (P09.T4.b).
///
/// Word 0 of `feature.nursery` picks the component. Its life is then drawn conditional on its
/// being alive at the epoch, by rejection: attempt `k` reads five words from `1 + 5k`, its age on
/// the young disc's born ages, its mass, its bound mark, its embedded duration and its dissolution
/// age, and is kept if the nursery has not dissolved by the epoch. The other marks follow after
/// the attempts' words: expansion speed, age spread, efficiency, metallicity, bubble interior,
/// and the half-mass radius and concentration it has as a bound cluster.
fn nursery(
    galaxy: &Galaxy,
    id: FeatureId,
    position: GalacticPosition,
    point: &PointLy,
) -> Option<FeatureRecord> {
    let process = FeatureProcess::Nursery;
    let mut stream = feature_stream(galaxy, tags::FEATURE_NURSERY, id);
    let pick = stream.uniform();
    let (component_id, component) = pick_component(galaxy, process, point, pick)?;
    let ages = component.ages();
    let unborn = ages.cdf(Years::ZERO);
    let myr = |value: f64| Years::new(value * YEARS_PER_MEGAYEAR);
    let life = (0..CONDITIONAL_ATTEMPTS).find_map(|k| {
        stream.seek(1 + NURSERY_ATTEMPT_WORDS * k);
        let age = ages.quantile(unborn + stream.uniform() * (1.0 - unborn));
        let age = Years::new(age.value().max(0.0));
        let mass =
            NurseryMassFunction::STANDARD.quantile_above(SolarMasses::new(0.0), stream.uniform());
        let bound = stream
            .mark()
            .is_below(Threshold::from_probability(BOUND_FRACTION));
        let embedded = myr(stream.uniform_in(EMBEDDED_DURATION_MYR.0, EMBEDDED_DURATION_MYR.1));
        let dissolution = myr(stream.uniform_in(DISSOLUTION_AGE_MYR.0, DISSOLUTION_AGE_MYR.1));
        let marks = NurseryMarks {
            age_at_epoch: age,
            mass,
            bound,
            embedded_duration: embedded,
            dissolution_age: dissolution,
            expansion_speed: KilometresPerSecond::new(0.0),
            age_spread: Years::ZERO,
            efficiency: EFFICIENCY_RANGE.0,
            fe_h: Dex::new(0.0),
            bubble_interior: BUBBLE_INTERIOR_MEDIAN,
            half_mass_radius: HALF_MASS_RADIUS_AT_1E4,
            concentration: CONCENTRATION_RANGE.0,
        };
        (marks.stage_at(Years::ZERO) != NurseryStage::Dissolved).then_some(marks)
    })?;
    stream.seek(1 + NURSERY_ATTEMPT_WORDS * CONDITIONAL_ATTEMPTS);
    let speed =
        KilometresPerSecond::new(stream.uniform_in(EXPANSION_SPEED_KM_S.0, EXPANSION_SPEED_KM_S.1));
    let spread = myr(stream.uniform() * AGE_SPREAD_MAX_MYR);
    let efficiency = stream.uniform_in(EFFICIENCY_RANGE.0, EFFICIENCY_RANGE.1);
    let metallicity = component.metallicity(point, life.age_at_epoch);
    let fe_h = Dex::new(
        metallicity.mean().value() + metallicity.sigma().value() * stream.standard_normal(),
    );
    // The drawn normal's rank, mapped into the truncated law on the same words (ruling 136.3).
    let interior_z = truncated_standard_normal(
        standard_normal_cdf(stream.standard_normal()),
        BUBBLE_INTERIOR_TRUNCATION,
    );
    let interior = HydrogenPerCm3::new(
        BUBBLE_INTERIOR_MEDIAN.value() * math::exp10(BUBBLE_INTERIOR_SIGMA_DEX * interior_z),
    );
    let radius = half_mass_radius(life.mass, stream.standard_normal());
    let concentration = stream.uniform_in(CONCENTRATION_RANGE.0, CONCENTRATION_RANGE.1);
    let marks = NurseryMarks {
        expansion_speed: speed,
        age_spread: spread,
        efficiency,
        fe_h,
        bubble_interior: interior,
        half_mass_radius: radius,
        concentration,
        ..life
    };
    let bubble_cap = BLOW_OUT_HEIGHTS * galaxy.gas().params().neutral_height().value();
    let reach = (SIZE_RANGE_LY.1 / 2.0)
        .max(bubble_cap)
        .min(MAX_FEATURE_REACH.value());
    Some(FeatureRecord {
        id,
        process,
        position,
        component: Some(component_id),
        marks: FeatureMarks::Nursery(marks),
        reach: LightYears::new(reach),
    })
}

/// A globular's reach, in tidal radii: its tails' extent until P09.T21's grid (ours).
const GLOBULAR_REACH_TIDAL_RADII: f64 = 4.0;

/// A globular's marks (P09.T12.b; [`globular`](super::kinds::globular)'s documentation).
///
/// Words of `feature.globular`: 0 the part (metal-rich or metal-poor) by the parts' odds at the
/// position, 1 the origin within the part, 2 the mass, 3–4 the half-mass radius's normal, 5–6 the
/// core's normal, 7–8 the metallicity's normal, 9 an accreted cluster's age.
fn globular(
    galaxy: &Galaxy,
    id: FeatureId,
    position: GalacticPosition,
    point: &PointLy,
) -> FeatureRecord {
    let system = galaxy.feature_shares().globulars();
    let mut stream = feature_stream(galaxy, tags::FEATURE_GLOBULAR, id);
    let (part, within) = (stream.mark(), stream.mark());
    let origin = system.origin(point, part, within);
    let mass = system.mass_function().quantile(stream.uniform());
    let r_kpc = (point.x * point.x + point.y * point.y + point.z * point.z).sqrt()
        / crate::galaxy::consts::LIGHT_YEARS_PER_KILOPARSEC;
    let (r0, slope, scatter) = HALF_MASS_RADIUS_LAW;
    let r_h =
        r0 * math::powf(r_kpc.max(0.05), slope) * math::exp10(scatter * stream.standard_normal());
    let (u_mean, u_sigma, (u_lo, u_hi)) = CORE_LAW;
    let u = (u_mean + u_sigma * stream.standard_normal()).clamp(u_lo, u_hi);
    let ((rich_mean, rich_sigma), (poor_mean, poor_sigma)) = METALLICITY_LAWS;
    let (mean, sigma) = if origin.is_metal_rich() {
        (rich_mean, rich_sigma)
    } else {
        (poor_mean, poor_sigma)
    };
    let fe_h = Dex::new(mean + sigma * stream.standard_normal());
    let accreted_age = stream.uniform_in(ACCRETED_AGES.0, ACCRETED_AGES.1);
    let age = Years::new(if origin.is_in_situ() {
        IN_SITU_AGE
    } else {
        accreted_age
    });
    let marks = GlobularMarks {
        origin,
        mass,
        half_mass_radius: LightYears::new(r_h * crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC),
        log_half_mass_over_core: u,
        fe_h,
        age,
    };
    let tidal = galaxy.potential().tidal_radius(mass, point).value() / METRES_PER_LIGHT_YEAR;
    FeatureRecord {
        id,
        process: FeatureProcess::Globular,
        position,
        component: origin_component(galaxy, origin),
        marks: FeatureMarks::Globular(marks),
        reach: LightYears::new(
            (GLOBULAR_REACH_TIDAL_RADII * tidal)
                .max(marks.half_mass_radius().value())
                .min(MAX_FEATURE_REACH.value()),
        ),
    }
}

/// A cloud's marks (P09.T4.c).
fn cloud(galaxy: &Galaxy, id: FeatureId, position: GalacticPosition) -> FeatureRecord {
    let mut stream = feature_stream(galaxy, tags::FEATURE_CLOUD, id);
    let law = PowerLaw::new(
        CLOUD_MASS_SLOPE,
        CLOUD_MASS_MIN.value(),
        CLOUD_MASS_MAX.value(),
    )
    .expect("the cloud mass function's limits and slope are valid");
    let mass = SolarMasses::new(stream.power_law(&law));
    let marks = CloudMarks::new(mass, galaxy.gas().dust_per_hydrogen(&position));
    let reach = (CLOUD_REACH_CORES * marks.core_radius().value()).min(MAX_FEATURE_REACH.value());
    FeatureRecord {
        id,
        process: FeatureProcess::Cloud,
        position,
        component: None,
        marks: FeatureMarks::Cloud(marks),
        reach: LightYears::new(reach),
    }
}

/// The distance from `p` (ly) to the nearest point of `cell`, ly.
fn box_distance(cell: FeatureCell, p: [f64; 3]) -> f64 {
    let origin = cell.origin().to_array();
    let mut sum = 0.0;
    for axis in 0..3 {
        let lo = f64::from(origin[axis]);
        let hi = lo + EDGE;
        let d = if p[axis] < lo {
            lo - p[axis]
        } else if p[axis] > hi {
            p[axis] - hi
        } else {
            0.0
        };
        sum += d * d;
    }
    sum.sqrt()
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::galaxy::params::GalaxyParams;
    use crate::rng::Seed;

    /// The seed of the galaxies these tests place features in.
    const SEED: u64 = 0x0900_0003_0000_0000;

    fn milky_way(n: u64) -> Galaxy {
        Galaxy::from_params(Seed::new(SEED | n), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    fn cell(c: [i32; 3]) -> FeatureCell {
        FeatureCell::new(c).unwrap()
    }

    /// The cell above the Sun-like point, one below it, cells an arm ridge crosses in the inner
    /// disc, one at the bar's end, the centre's and one far above the plane.
    const CELLS: [[i32; 3]; 8] = [
        [0, 6, 0],
        [0, 6, -1],
        [-1, 0, 0],
        [-2, 0, -1],
        [3, 0, 0],
        [-4, -1, -1],
        [-1, -1, -1],
        [0, 3, 2],
    ];

    #[test]
    fn resolve_agrees_with_the_cell_and_refuses_rejected_and_uncounted_indices() {
        let galaxy = milky_way(0);
        for c in [[0, 6, 0], [0, 6, -1], [0, 3, 2]] {
            let contents = FeatureCatalogue::cell(&galaxy, cell(c));
            let total: u32 = contents.counts().iter().sum();
            let mut accepted = contents.features().iter().peekable();
            for index in 0..total {
                let id = FeatureId::of(cell(c), u16::try_from(index).unwrap()).unwrap();
                let resolved = FeatureCatalogue::resolve(&galaxy, id);
                if accepted.peek().is_some_and(|f| f.id() == id) {
                    assert_eq!(resolved.as_ref(), accepted.next(), "{c:?} {index}");
                } else {
                    assert_eq!(resolved, None, "{c:?} {index} was rejected");
                }
            }
            assert!(accepted.next().is_none());
            for index in [total, total + 1, 16_383] {
                let id = FeatureId::of(cell(c), u16::try_from(index).unwrap()).unwrap();
                assert_eq!(
                    FeatureCatalogue::resolve(&galaxy, id),
                    None,
                    "{c:?} {index}"
                );
            }
        }
    }

    #[test]
    fn a_dense_cell_resolves_every_tenth_candidate_as_it_generates_it() {
        let galaxy = milky_way(0);
        let contents = FeatureCatalogue::cell(&galaxy, cell([-1, 0, 0]));
        for feature in contents.features().iter().step_by(10) {
            assert_eq!(
                FeatureCatalogue::resolve(&galaxy, feature.id()).as_ref(),
                Some(feature)
            );
        }
        assert!(
            contents.features().len() > 500,
            "{}",
            contents.features().len()
        );
    }

    #[test]
    fn cells_do_not_depend_on_what_was_generated_before() {
        let galaxy = milky_way(1);
        let keys: Vec<FeatureCell> = CELLS[..4].iter().map(|&c| cell(c)).collect();
        assert_order_independent(&keys, |&c| FeatureCatalogue::cell(&galaxy, c));
    }

    /// The bound hunt of P09.T3.a and T3.b: 10⁵ points per process spread over the cells, each
    /// uniform in a column of a column proposal and in the cell otherwise, never above the term's
    /// envelope `B′ exp(−lift ÷ s)` there.
    #[test]
    fn no_density_rises_above_its_proposal_over_random_points() {
        let galaxy = milky_way(2);
        let mut stream = Stream::open(Seed::new(SEED), tags::SELFTEST_STREAM, ObjectKey::galaxy());
        let per_cell = 100_000 / CELLS.len();
        for process in FeatureProcess::ALL {
            for &c in &CELLS {
                let proposal = proposal(&galaxy, process, cell(c));
                for term in proposal.terms.iter().flatten() {
                    for _ in 0..per_cell {
                        let (p, envelope) = random_point(&mut stream, cell(c), term);
                        let density = term_density(&galaxy, process, term.part, &p);
                        assert!(
                            density <= envelope,
                            "{process} at {:?}: {density} above {envelope}",
                            p.to_light_years_f64()
                        );
                    }
                }
            }
        }
    }

    /// A uniform point of `term`'s support in `cell` and the term's envelope there.
    fn random_point(
        stream: &mut Stream,
        cell: FeatureCell,
        term: &Term,
    ) -> (GalacticPosition, f64) {
        let origin = cell.origin().to_array().map(f64::from);
        let mut ly = [0.0; 3];
        let mut bound = term.bound;
        let mut lo = origin;
        let mut span = [EDGE; 3];
        if let Some(columns) = &term.columns {
            let n = usize::try_from(COLUMNS_PER_AXIS).unwrap();
            let i =
                usize::try_from(stream.below(core::num::NonZeroU64::new(256).unwrap())).unwrap();
            bound = columns.bounds[i];
            lo[0] += f64::from(COLUMN_EDGE) * f64::from(u32::try_from(i / n).unwrap());
            lo[1] += f64::from(COLUMN_EDGE) * f64::from(u32::try_from(i % n).unwrap());
            span[0] = f64::from(COLUMN_EDGE);
            span[1] = f64::from(COLUMN_EDGE);
        }
        let mut lift = 0.0;
        for axis in 0..3 {
            ly[axis] = lo[axis] + span[axis] * stream.uniform() * (1.0 - 1e-12);
            let spec = term.axes[axis];
            if spec.scale > 0.0 {
                lift += (ly[axis].abs() - spec.near).max(0.0) / spec.scale;
            }
        }
        let p = GalacticPosition::from_light_years(ly).unwrap();
        (p, bound * math::exp(-lift))
    }

    #[test]
    fn accepted_heights_follow_the_vertical_law_not_the_proposal() {
        // Nurseries and the first sub-disc's clusters in the plane cells of a ring about the Sun's
        // radius, over several galaxies: their |z| inside the cell follows the component's
        // profile, 0–4,096 ly.
        for process in [
            FeatureProcess::Nursery,
            FeatureProcess::OldOpenCluster(SubDisc::First),
        ] {
            let mut heights = Vec::new();
            let mut profile = None;
            for n in 0..6 {
                let galaxy = milky_way(10 + n);
                for c in [
                    [0, 6, 0],
                    [0, 5, 0],
                    [1, 5, 0],
                    [-1, 5, 0],
                    [-2, 4, 0],
                    [5, 0, 0],
                    [-6, 0, 0],
                ] {
                    let contents = FeatureCatalogue::cell(&galaxy, cell(c));
                    heights.extend(
                        contents
                            .features()
                            .iter()
                            .filter(|f| f.process() == process)
                            .map(|f| PointLy::from(f.position()).z),
                    );
                }
                if profile.is_none() {
                    let (_, c) = disc_components(&galaxy, process).next().unwrap();
                    let Shape::Disc(disc) = c.shape() else {
                        unreachable!()
                    };
                    profile = Some(disc.profile().clone());
                }
            }
            let profile = profile.unwrap();
            let top = profile.integral_to(EDGE).value();
            let ks = ks_one_sample(&mut heights, |z| profile.integral_to(z).value() / top);
            assert!(heights.len() > 300, "{process}: {}", heights.len());
            assert_p_value(&format!("{process} heights"), ks.p_value, ALPHA);
        }
        // The clouds of the same cells follow the neutral layer, exponential in height (the
        // molecular disc is at the centre, far from them).
        let mut heights = Vec::new();
        let mut scale = 0.0;
        for n in 0..6 {
            let galaxy = milky_way(10 + n);
            scale = galaxy.gas().smooth().height(GasLayer::Neutral);
            for c in [
                [0, 6, 0],
                [0, 5, 0],
                [1, 5, 0],
                [-1, 5, 0],
                [-2, 4, 0],
                [5, 0, 0],
                [-6, 0, 0],
            ] {
                heights.extend(
                    FeatureCatalogue::cell(&galaxy, cell(c))
                        .features()
                        .iter()
                        .filter(|f| f.process() == FeatureProcess::Cloud)
                        .map(|f| PointLy::from(f.position()).z),
                );
            }
        }
        let top = -math::exp_m1(-EDGE / scale);
        let ks = ks_one_sample(&mut heights, |z| -math::exp_m1(-z / scale) / top);
        assert!(heights.len() > 300, "clouds: {}", heights.len());
        assert_p_value("cloud heights", ks.p_value, ALPHA);
    }

    /// One inner cell and one outer cell, every feature's ID, process, position and first marks.
    #[test]
    fn feature_cells_golden() {
        let galaxy = milky_way(0);
        let mut writer = GoldenWriter::new();
        writer.header(GENERATOR_VERSION.get());
        for c in [[-1, 0, 0], [0, 6, 0]] {
            let contents = FeatureCatalogue::cell(&galaxy, cell(c));
            writer.line(&format!("cell {c:?} counts {:?}", contents.counts()));
            for f in contents
                .features()
                .iter()
                .step_by(if c[0] == -1 { 25 } else { 1 })
            {
                let [x, y, z] = f.position().to_light_years_f64();
                let marks = match f.marks() {
                    FeatureMarks::OpenCluster(m) => format!(
                        "age {:e} mass {:e} r_h {:e}",
                        m.age_at_epoch().value(),
                        m.initial_mass().value(),
                        m.half_mass_radius().value()
                    ),
                    FeatureMarks::Nursery(m) => format!(
                        "age {:e} mass {:e} bound {}",
                        m.age_at_epoch().value(),
                        m.mass().value(),
                        m.is_bound()
                    ),
                    FeatureMarks::Cloud(m) => format!("mass {:e}", m.mass().value()),
                    FeatureMarks::Globular(m) => format!(
                        "mass {:e} r_h {:e} {:?}",
                        m.mass().value(),
                        m.half_mass_radius().value(),
                        m.origin()
                    ),
                };
                writer.line(&format!(
                    "{} {} ({x:?}, {y:?}, {z:?}) {marks}",
                    f.id().designation(),
                    f.process()
                ));
            }
        }
        golden!("galaxy/features/cells", writer.as_str());
    }

    #[test]
    fn an_old_open_cluster_reaches_at_least_its_tidal_radius() {
        use super::super::cluster::ClusterModel;
        let galaxy = milky_way(0);
        let mut checked = 0;
        for f in FeatureCatalogue::cell(&galaxy, cell([0, 6, 0])).features() {
            if !matches!(f.marks(), FeatureMarks::OpenCluster(_)) || checked >= 3 {
                continue;
            }
            // A cluster model costs seconds, so three of them.
            let model = ClusterModel::from_record(&galaxy, f).expect("an old cluster is alive");
            assert!(f.reach().value() >= model.tidal_radius().value(), "{f:?}");
            checked += 1;
        }
        assert_eq!(checked, 3);
    }

    #[test]
    fn no_record_reaches_past_the_catalogue_s_reach() {
        let galaxy = milky_way(0);
        for &c in &CELLS {
            for f in FeatureCatalogue::cell(&galaxy, cell(c)).features() {
                assert!(f.reach().value() > 0.0 && f.reach().value() <= MAX_FEATURE_REACH.value());
                assert!(
                    f.kind_at(Years::ZERO).is_some(),
                    "{:?} is dissolved",
                    f.id()
                );
            }
        }
    }

    #[test]
    fn near_finds_what_a_scan_of_the_neighbouring_cells_finds() {
        let galaxy = milky_way(0);
        let centre = GalacticPosition::from_light_years([100.0, 26_000.0, 20.0]).unwrap();
        let radius = LightYears::new(300.0);
        let found: Vec<FeatureRecord> =
            FeatureCatalogue::near(&galaxy, &centre, radius, &NoFeatureCache).collect();
        let mut expected = Vec::new();
        for i in -2..=2 {
            for j in 4..=8 {
                for k in -2..=1 {
                    for f in FeatureCatalogue::cell(&galaxy, cell([i, j, k])).features() {
                        let d = f.position().distance_to(&centre).value() / METRES_PER_LIGHT_YEAR;
                        if d <= radius.value() + f.reach().value() {
                            expected.push(*f);
                        }
                    }
                }
            }
        }
        let key = |f: &FeatureRecord| f.id();
        let mut a: Vec<_> = found.iter().map(key).collect();
        let mut b: Vec<_> = expected.iter().map(key).collect();
        a.sort();
        b.sort();
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }
}
