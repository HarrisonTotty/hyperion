//! The gas features add to plan 07's field: superbubbles as holes, clouds and embedded regions as
//! Plummer balls (plan 09, P09.T5).
//!
//! [`FeatureGas`] implements plan 07's [`GasModifierSource`]. For a segment it looks up the
//! catalogue's features whose reach touches it and yields a [`GasModifier::Hole`] for each nursery's
//! superbubble at the evaluation time ([`Superbubble`]: radius and interior density from P09.T4.b),
//! and a [`GasModifier::Cloud`] for each cloud (its Plummer core radius, central density and the
//! site's dust per hydrogen) and each embedded star-forming region (its gas mass in a Plummer ball
//! whose half-mass radius is half its size). Plan 07 integrates them. The supernova shell window of
//! phase 4 reads the smooth field only, never these.
//!
//! What a segment gets is a pure function of the unordered pair of its ends, as the trait asks: the
//! ends are put in a canonical order first, so that `a → b` and `b → a` look up the same features
//! and test them against the same line. Features sit at their epoch positions; their bulk motion,
//! at most some ten light-years over plan 01's source horizon, is not applied.
//!
//! Plan 09 has the server pass `FeatureGas` where it passed `NoModifiers`: the sky's sightlines and
//! the `extinction` request's (P09.T40.a), each at its own time, behind the server's gate that
//! holds plan 09's feature members back until P09.T2.c.
//!
//! # The index
//!
//! A segment's lookup reads every feature cell within its half-length and [`MAX_FEATURE_REACH`] of
//! its middle: some 3,000 cells for a ray from the Sun to the root cube's edge, of which it keeps the
//! nurseries and clouds near the line. A sky marches thousands of such rays, so
//! [`FeatureGasIndex`] holds a galaxy's nurseries and clouds once, binned by the feature cells their
//! reach touches, and [`FeatureGas::indexed`] looks a segment up in the bins it crosses alone
//! (P09.T40.a). It finds the same features, by the same two tests, and yields their modifiers in
//! the same order: the cells' order, then the index's. A feature whose reach touches the segment
//! has a point of the segment within its reach, so in a cell its padded reach overlaps, and the
//! segment crosses that cell; the lookup's own cells hold it too, since a nursery or cloud reaches
//! less than half [`MAX_FEATURE_REACH`] (`gas_index_finds_what_the_cells_find`).

use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::gas::modifiers::{GasModifier, GasModifierSource};
use crate::id::FeatureCell;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{HydrogenPerCm3, LightYears, Years};

use super::catalogue::{
    EDGE, FeatureCatalogue, FeatureCellCache, FeatureMarks, FeatureRecord, MAX_FEATURE_REACH,
};
use super::ids::FeatureProcess;
use super::kinds::cloud::{PLUMMER_HALF_MASS_RATIO, plummer_central_density};
#[cfg(doc)]
use super::kinds::nursery::Superbubble;
use super::kinds::nursery::{NurseryMarks, Superbubble as Bubble};

/// The features' gas at one evaluation time, as a source of plan 07's modifiers (P09.T5).
pub struct FeatureGas<'a> {
    galaxy: &'a Galaxy,
    lookup: Lookup<'a>,
    time: Years,
}

/// Where a [`FeatureGas`] finds a segment's features.
#[derive(Clone, Copy)]
enum Lookup<'a> {
    /// The catalogue's cells about the segment's middle, through a cache.
    Cells(&'a dyn FeatureCellCache),
    /// The galaxy's nurseries and clouds, in the bins the segment crosses.
    Index(&'a FeatureGasIndex),
}

impl std::fmt::Debug for FeatureGas<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeatureGas")
            .field("seed", &self.galaxy.seed())
            .field("time", &self.time)
            .finish_non_exhaustive()
    }
}

impl<'a> FeatureGas<'a> {
    /// The source of `galaxy`'s features `time` after the epoch, looking cells up through `cache`.
    #[must_use]
    pub fn new(galaxy: &'a Galaxy, cache: &'a dyn FeatureCellCache, time: Years) -> Self {
        Self {
            galaxy,
            lookup: Lookup::Cells(cache),
            time,
        }
    }

    /// The source of `galaxy`'s features `time` after the epoch, looking them up in `index`: the
    /// same modifiers as [`new`](Self::new)'s, in the same order, for every segment, found from
    /// the bins the segment crosses rather than from every cell about its middle (P09.T40.a).
    ///
    /// # Panics
    ///
    /// If `index` is another galaxy's.
    #[must_use]
    pub fn indexed(galaxy: &'a Galaxy, index: &'a FeatureGasIndex, time: Years) -> Self {
        assert_eq!(
            index.seed,
            galaxy.seed(),
            "a features' gas index is read for its own galaxy"
        );
        debug_assert!(
            index.features.first().is_none_or(|first| {
                index.centre_gas[0] == CentreGas::of(galaxy, first.position())
            }),
            "a features' gas index is read for the galaxy it was built from"
        );
        Self {
            galaxy,
            lookup: Lookup::Index(index),
            time,
        }
    }

    /// The modifiers of one feature at the source's time, appended to `out`; a nursery's read the
    /// field at its centre from `centre_gas`.
    fn feature_modifiers(
        &self,
        feature: &FeatureRecord,
        centre_gas: impl FnOnce() -> CentreGas,
        out: &mut Vec<GasModifier>,
    ) {
        let centre = *feature.position();
        match feature.marks() {
            FeatureMarks::Cloud(cloud) => push_valid(
                out,
                GasModifier::Cloud {
                    centre,
                    core_radius: cloud.core_radius(),
                    central_density: cloud.central_density(),
                    dust_per_hydrogen: cloud.dust_per_hydrogen(),
                },
            ),
            FeatureMarks::Nursery(marks) => {
                self.nursery_modifiers(feature, marks, &centre_gas(), out);
            }
            FeatureMarks::OpenCluster(_) | FeatureMarks::Globular(_) => {}
        }
    }

    fn nursery_modifiers(
        &self,
        feature: &FeatureRecord,
        marks: &NurseryMarks,
        centre_gas: &CentreGas,
        out: &mut Vec<GasModifier>,
    ) {
        let centre = *feature.position();
        let age = marks.age_at_epoch() + self.time;
        let gas = self.galaxy.gas();
        if let Some(bubble) = Bubble::of(
            marks,
            age,
            centre_gas.mean_density,
            gas.params().neutral_height(),
            self.galaxy.feature_shares().supernova_clock(),
        ) {
            push_valid(
                out,
                GasModifier::Hole {
                    centre,
                    radius: bubble.radius(),
                    interior: bubble.interior_density(),
                },
            );
        }
        let mass = marks.gas_mass_at(age);
        if mass.value() > 0.0 {
            let core = marks.size_at(age) * (0.5 / PLUMMER_HALF_MASS_RATIO);
            push_valid(
                out,
                GasModifier::Cloud {
                    centre,
                    core_radius: core,
                    central_density: plummer_central_density(mass, core),
                    dust_per_hydrogen: centre_gas.dust_per_hydrogen,
                },
            );
        }
    }
}

/// What a nursery's gas reads of plan 07's smooth field at its centre: a pure function of the
/// centre, which [`FeatureGasIndex`] keeps beside each feature.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CentreGas {
    mean_density: HydrogenPerCm3,
    dust_per_hydrogen: f64,
}

impl CentreGas {
    /// The field's at `centre` in `galaxy`.
    #[must_use]
    fn of(galaxy: &Galaxy, centre: &GalacticPosition) -> Self {
        let gas = galaxy.gas();
        Self {
            mean_density: gas.mean_density(centre),
            dust_per_hydrogen: gas.dust_per_hydrogen(centre),
        }
    }
}

/// Pushes `modifier` if it is one plan 07 accepts: every modifier made here is, and the debug
/// build says so.
fn push_valid(out: &mut Vec<GasModifier>, modifier: GasModifier) {
    debug_assert!(
        modifier.is_valid(),
        "an invalid feature gas modifier {modifier:?}"
    );
    if modifier.is_valid() {
        out.push(modifier);
    }
}

impl GasModifierSource for FeatureGas<'_> {
    fn modifiers_near_segment(
        &self,
        a: &GalacticPosition,
        b: &GalacticPosition,
        out: &mut Vec<GasModifier>,
    ) {
        let (a, b) = canonical(a, b);
        let [ax, ay, az] = a.to_light_years_f64();
        let d = a
            .displacement_to(b)
            .metres()
            .map(|m| m / METRES_PER_LIGHT_YEAR);
        let half = 0.5 * (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let middle =
            GalacticPosition::from_light_years([ax + 0.5 * d[0], ay + 0.5 * d[1], az + 0.5 * d[2]]);
        let Some(middle) = middle else {
            return;
        };
        let radius = LightYears::new(half);
        match self.lookup {
            Lookup::Cells(cache) => {
                for feature in FeatureCatalogue::near(self.galaxy, &middle, radius, cache) {
                    if segment_distance(a, &d, feature.position()) <= feature.reach().value() {
                        let centre_gas = || CentreGas::of(self.galaxy, feature.position());
                        self.feature_modifiers(&feature, centre_gas, out);
                    }
                }
            }
            Lookup::Index(index) => {
                // `near`'s test, then the line's, as above.
                let touches = |feature: &FeatureRecord| {
                    feature.touches_sphere(&middle, radius)
                        && segment_distance(a, &d, feature.position()) <= feature.reach().value()
                };
                let mut found = Vec::new();
                index.found_along(a, &d, touches, &mut found);
                for rank in found {
                    let centre_gas = || index.centre_gas[rank];
                    self.feature_modifiers(&index.features[rank], centre_gas, out);
                }
            }
        }
    }
}

/// A galaxy's nurseries and clouds, the features that carry gas, binned by the feature cells their
/// reach touches (P09.T40.a): what [`FeatureGas::indexed`] looks a segment up in.
///
/// Built once per galaxy from the whole catalogue, as [`build`](Self::build) makes it or a caller
/// [`assemble`](Self::assemble)s it from its [`slab`](Self::slab)s, made in any order or at once;
/// some 215,000 features and 52 MB for the Milky Way-like galaxy, whose cells take some 7 s to
/// make on one thread of a test build (pod, 2026-11-05).
///
/// # Examples
///
/// A server makes the slabs on its workers, then reads the index for every sightline of a sky:
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
/// use hyperion_sim::galaxy::features::gas_overlay::{FeatureGas, FeatureGasIndex};
/// use hyperion_sim::galaxy::gas::modifiers::GasModifierSource;
/// use hyperion_sim::units::Years;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// // Each slab on a thread of its own, then assembled in their order.
/// let slabs = FeatureGasIndex::SLABS.map(|x| FeatureGasIndex::slab(&galaxy, &NoFeatureCache, x));
/// let index = FeatureGasIndex::assemble(&galaxy, slabs.collect::<Vec<_>>());
/// // What the cells' lookup finds, as the cells' lookup would order it, along a ray coreward.
/// let gas = FeatureGas::indexed(&galaxy, &index, Years::ZERO);
/// let at = |ly| GalacticPosition::from_light_years(ly).expect("in the cube");
/// let mut modifiers = Vec::new();
/// gas.modifiers_near_segment(&at([0.0, 26_000.0, 0.0]), &at([0.0, -60_000.0, 0.0]), &mut modifiers);
/// assert!(!modifiers.is_empty());
/// ```
#[derive(Clone, PartialEq)]
pub struct FeatureGasIndex {
    seed: Seed,
    /// The features, in the order [`FeatureCatalogue::near`] gives them: by cell, x then y then z,
    /// then by index.
    features: Vec<FeatureRecord>,
    /// The field at each feature's centre, as its gas reads it.
    centre_gas: Vec<CentreGas>,
    /// Each feature's centre, ly, and its reach padded by [`BIN_PAD_LY`]: a cheap first test.
    spheres: Vec<[f64; 4]>,
    /// Bin `b`'s features are `entries[starts[b]..starts[b + 1]]`, their places in `features`,
    /// ascending.
    starts: Vec<u32>,
    entries: Vec<u32>,
}

impl std::fmt::Debug for FeatureGasIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeatureGasIndex")
            .field("seed", &self.seed)
            .field("features", &self.features.len())
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}

/// The nurseries and clouds of the feature cells of one x coordinate, in the index's order: a
/// [`FeatureGasIndex`]'s part, which a caller makes on a thread of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureGasSlab {
    seed: Seed,
    x: i32,
    features: Vec<FeatureRecord>,
    centre_gas: Vec<CentreGas>,
}

/// The feature cells along each axis, and the least cell coordinate.
const CELLS_PER_AXIS: i32 = 32;
const LEAST_CELL: i32 = -16;

/// How far beyond its reach a feature is binned, and beyond a bin's faces a segment is taken to
/// cross it, ly: far more than the roundings of either, so that no feature the tests find is
/// missed.
const BIN_PAD_LY: f64 = 1.0;

/// How far short of [`MAX_FEATURE_REACH`] every nursery's and cloud's reach must stop, ly: so that
/// the cells [`FeatureCatalogue::near`] reads about a segment's middle hold every feature the
/// index finds for it, with the bins' pad and far more than the tests' roundings to spare. Their
/// reaches stop near 1,750 ly.
const REACH_MARGIN_LY: f64 = 2.0 * BIN_PAD_LY;

impl FeatureGasIndex {
    /// The x coordinates of the feature cells, one slab each, in the order
    /// [`assemble`](Self::assemble) takes them.
    pub const SLABS: std::ops::Range<i32> = LEAST_CELL..LEAST_CELL + CELLS_PER_AXIS;

    /// `galaxy`'s index, its cells read through `cache`: [`assemble`](Self::assemble) of every
    /// [`slab`](Self::slab).
    #[must_use]
    pub fn build(galaxy: &Galaxy, cache: &dyn FeatureCellCache) -> Self {
        Self::assemble(galaxy, Self::SLABS.map(|x| Self::slab(galaxy, cache, x)))
    }

    /// The nurseries and clouds of `galaxy`'s feature cells whose x coordinate is `x`, read
    /// through `cache`.
    ///
    /// # Panics
    ///
    /// If `x` is not one of [`SLABS`](Self::SLABS).
    #[must_use]
    pub fn slab(galaxy: &Galaxy, cache: &dyn FeatureCellCache, x: i32) -> FeatureGasSlab {
        assert!(Self::SLABS.contains(&x), "{x} is no feature cell's x");
        let mut features = Vec::new();
        for y in Self::SLABS {
            for z in Self::SLABS {
                let cell = FeatureCell::new([x, y, z]).expect("a coordinate of every feature cell");
                features.extend(
                    cache
                        .contents(galaxy, cell)
                        .features()
                        .iter()
                        .filter(|feature| carries_gas(feature.process()))
                        .copied(),
                );
            }
        }
        let centre_gas = features
            .iter()
            .map(|feature| CentreGas::of(galaxy, feature.position()))
            .collect();
        FeatureGasSlab {
            seed: galaxy.seed(),
            x,
            features,
            centre_gas,
        }
    }

    /// The index of `galaxy` from its `slabs`, one for each of [`SLABS`](Self::SLABS) in order.
    ///
    /// # Panics
    ///
    /// - If the slabs are not `galaxy`'s, or not every x in order.
    /// - If a feature reaches within [`REACH_MARGIN_LY`] of [`MAX_FEATURE_REACH`], where the
    ///   index's lookup is no longer the cells' (module documentation).
    /// - If the slabs hold more than `u32::MAX` features, or the bins more than `u32::MAX` entries.
    #[must_use]
    pub fn assemble(galaxy: &Galaxy, slabs: impl IntoIterator<Item = FeatureGasSlab>) -> Self {
        let (mut features, mut centre_gas) = (Vec::new(), Vec::new());
        let mut xs = Self::SLABS;
        for slab in slabs {
            assert_eq!(
                slab.seed,
                galaxy.seed(),
                "a features' gas index takes its own galaxy's slabs"
            );
            assert_eq!(
                Some(slab.x),
                xs.next(),
                "a features' gas index takes its slabs in x order"
            );
            features.extend(slab.features);
            centre_gas.extend(slab.centre_gas);
        }
        assert_eq!(xs.next(), None, "a features' gas index takes every slab");
        let farthest = MAX_FEATURE_REACH.value() - REACH_MARGIN_LY;
        assert!(
            features
                .iter()
                .all(|feature| feature.reach().value() < farthest),
            "a nursery or cloud reaches within {REACH_MARGIN_LY} ly of the catalogue's farthest"
        );
        features.shrink_to_fit();
        centre_gas.shrink_to_fit();
        let spheres = features
            .iter()
            .map(|feature| {
                let [x, y, z] = feature.position().to_light_years_f64();
                [x, y, z, feature.reach().value() + BIN_PAD_LY]
            })
            .collect();
        let ranks = u32::try_from(features.len()).expect("a galaxy's gas features fit a u32");
        let mut counts = vec![0_u32; BINS];
        for feature in &features {
            for_each_bin(feature, |bin| counts[bin] += 1);
        }
        let mut starts = Vec::with_capacity(BINS + 1);
        let mut total = 0_u32;
        starts.push(0);
        for count in &counts {
            total = total
                .checked_add(*count)
                .expect("a galaxy's binned gas features fit a u32");
            starts.push(total);
        }
        let mut entries = vec![0_u32; usize::try_from(total).expect("u32 fits a usize")];
        let mut next: Vec<u32> = starts[..BINS].to_vec();
        for (rank, feature) in (0..ranks).zip(&features) {
            for_each_bin(feature, |bin| {
                entries[usize::try_from(next[bin]).expect("u32 fits a usize")] = rank;
                next[bin] += 1;
            });
        }
        Self {
            seed: galaxy.seed(),
            features,
            centre_gas,
            spheres,
            starts,
            entries,
        }
    }

    /// The galaxy's nurseries and clouds, in the order [`FeatureCatalogue::near`] gives them.
    #[must_use]
    pub fn features(&self) -> &[FeatureRecord] {
        &self.features
    }

    /// The bytes the index owns on the heap: what a caller's byte-bounded cache charges it.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.features.capacity() * size_of::<FeatureRecord>()
            + self.centre_gas.capacity() * size_of::<CentreGas>()
            + self.spheres.capacity() * size_of::<[f64; 4]>()
            + (self.starts.capacity() + self.entries.capacity()) * size_of::<u32>()
    }

    /// The places of the features that pass `touches` among those binned in every bin the
    /// segment from `start` along `along` (ly) crosses, ascending, into `found`: each feature is
    /// tested once, and only the found are sorted.
    ///
    /// A feature whose padded sphere misses the segment in float light-years is not tested: the
    /// pad, a light-year, is far more than that float test's error, so `touches`, which asks the
    /// segment within the reach itself, would fail it too. It halves the lookup of a ray from the
    /// Sun-like point to the centre and beyond, 12.5 ms to 6.7 ms (P09.T40.a; pod, test build,
    /// 2026-11-05).
    fn found_along(
        &self,
        start: &GalacticPosition,
        along: &[f64; 3],
        touches: impl Fn(&FeatureRecord) -> bool,
        found: &mut Vec<usize>,
    ) {
        let origin = start.to_light_years_f64();
        let length_sq = dot(along, along);
        let near = |[x, y, z, padded]: [f64; 4]| {
            let offset = [x - origin[0], y - origin[1], z - origin[2]];
            let t = if length_sq > 0.0 {
                (dot(&offset, along) / length_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let miss = [0, 1, 2].map(|k| offset[k] - t * along[k]);
            dot(&miss, &miss) <= padded * padded
        };
        let mut seen = vec![0_u64; self.features.len().div_ceil(64)];
        crossed_bins(origin, *along, |bin| {
            let entries = &self.entries[to_usize(self.starts[bin])..to_usize(self.starts[bin + 1])];
            for rank in entries.iter().map(|&rank| to_usize(rank)) {
                let (word, bit) = (rank / 64, 1_u64 << (rank % 64));
                if seen[word] & bit == 0 {
                    seen[word] |= bit;
                    if near(self.spheres[rank]) && touches(&self.features[rank]) {
                        found.push(rank);
                    }
                }
            }
        });
        found.sort_unstable();
    }
}

/// Whether `process`'s features carry gas: the nurseries and the clouds.
#[must_use]
fn carries_gas(process: FeatureProcess) -> bool {
    match process {
        FeatureProcess::Nursery | FeatureProcess::Cloud => true,
        FeatureProcess::Globular | FeatureProcess::OldOpenCluster(_) => false,
    }
}

/// The bins, one per feature cell.
const BINS: usize = 1 << 15;

#[must_use]
fn to_usize(value: u32) -> usize {
    usize::try_from(value).expect("u32 fits a usize")
}

/// The bin of the feature cell `[x, y, z]`, each coordinate one of [`FeatureGasIndex::SLABS`], in
/// x, then y, then z order.
#[must_use]
fn bin([x, y, z]: [i32; 3]) -> usize {
    let place =
        |c: i32| to_usize(u32::try_from(c - LEAST_CELL).expect("a feature cell's coordinate"));
    let per_axis = to_usize(CELLS_PER_AXIS.unsigned_abs());
    (place(x) * per_axis + place(y)) * per_axis + place(z)
}

/// The feature cell coordinate that holds `ly` on an axis, clamped to the cells.
#[must_use]
fn cell_of(ly: f64) -> i32 {
    let cell = (ly / EDGE).floor();
    if cell.is_nan() || cell < f64::from(LEAST_CELL) {
        return LEAST_CELL;
    }
    if cell >= f64::from(LEAST_CELL + CELLS_PER_AXIS - 1) {
        return LEAST_CELL + CELLS_PER_AXIS - 1;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number within the cells' coordinates, checked above"
    )]
    let cell = cell as i32;
    cell
}

/// Calls `each` with every bin that `feature`'s reach, padded by [`BIN_PAD_LY`], overlaps.
fn for_each_bin(feature: &FeatureRecord, mut each: impl FnMut(usize)) {
    let centre = feature.position().to_light_years_f64();
    let reach = feature.reach().value() + BIN_PAD_LY;
    let [x, y, z] = centre.map(|c| (cell_of(c - reach), cell_of(c + reach)));
    for i in x.0..=x.1 {
        for j in y.0..=y.1 {
            for k in z.0..=z.1 {
                each(bin([i, j, k]));
            }
        }
    }
}

/// Calls `each` with every bin whose cell, padded by [`BIN_PAD_LY`], the segment from `start` along
/// `along` (ly) crosses: x first, then within each x the y, then within each y the z.
fn crossed_bins(start: [f64; 3], along: [f64; 3], mut each: impl FnMut(usize)) {
    // The fractions of the segment, within `within`, whose `axis` lies in the padded cell `cell`.
    let clip = |axis: usize, cell: i32, within: (f64, f64)| -> Option<(f64, f64)> {
        let low = f64::from(cell) * EDGE - BIN_PAD_LY;
        let high = low + EDGE + 2.0 * BIN_PAD_LY;
        let (from, step) = (start[axis], along[axis]);
        if step.abs() > 0.0 {
            let (u, v) = ((low - from) / step, (high - from) / step);
            let (u, v) = if u < v { (u, v) } else { (v, u) };
            let first = if u > within.0 { u } else { within.0 };
            let last = if v < within.1 { v } else { within.1 };
            (first <= last).then_some((first, last))
        } else {
            (low <= from && from <= high).then_some(within)
        }
    };
    // The cells on `axis` the segment's fractions `within` reach.
    let cells = |axis: usize, within: (f64, f64)| {
        let ends = [within.0, within.1].map(|t| start[axis] + t * along[axis]);
        let (low, high) = if ends[0] < ends[1] {
            (ends[0], ends[1])
        } else {
            (ends[1], ends[0])
        };
        cell_of(low - BIN_PAD_LY)..=cell_of(high + BIN_PAD_LY)
    };
    let whole = (0.0, 1.0);
    for i in cells(0, whole) {
        let Some(on_i) = clip(0, i, whole) else {
            continue;
        };
        for j in cells(1, on_i) {
            let Some(on_j) = clip(1, j, on_i) else {
                continue;
            };
            for k in cells(2, on_j) {
                if clip(2, k, on_j).is_some() {
                    each(bin([i, j, k]));
                }
            }
        }
    }
}

/// The two ends in a fixed order, by light-year cell and then offset, whichever end was first.
fn canonical<'p>(
    a: &'p GalacticPosition,
    b: &'p GalacticPosition,
) -> (&'p GalacticPosition, &'p GalacticPosition) {
    let key = |p: &GalacticPosition| (p.cell().to_array(), p.offset_metres());
    let (ka, kb) = (key(a), key(b));
    let order = ka.0.cmp(&kb.0).then_with(|| {
        ka.1.iter()
            .zip(&kb.1)
            .map(|(x, y)| x.total_cmp(y))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if order.is_gt() { (b, a) } else { (a, b) }
}

#[must_use]
fn dot(u: &[f64; 3], v: &[f64; 3]) -> f64 {
    u[0] * v[0] + u[1] * v[1] + u[2] * v[2]
}

/// The distance, ly, from `point` to the segment from `start` along `along` (ly).
fn segment_distance(start: &GalacticPosition, along: &[f64; 3], point: &GalacticPosition) -> f64 {
    let offset = start
        .displacement_to(point)
        .metres()
        .map(|m| m / METRES_PER_LIGHT_YEAR);
    let length2 = dot(along, along);
    let t = if length2 > 0.0 {
        (dot(&offset, along) / length2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let miss = [0, 1, 2].map(|k| offset[k] - t * along[k]);
    dot(&miss, &miss).sqrt()
}
