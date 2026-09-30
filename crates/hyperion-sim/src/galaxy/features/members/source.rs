//! Feature members in the range query: plan 03's merge hook for the catalogue features' nested
//! grids (plan 09, P09.T23; brainstorm, "The range query" and "Dense features").
//!
//! The range query treats a feature as one more stack of layers. For a query sphere
//! [`FeatureMemberSource`] takes the features whose reach touches it
//! ([`FeatureCatalogue::near`]), and for each the owned nested cells that touch it, band by band
//! from the coarsest mass band down:
//!
//! - **Census.** A feature's expected members in a band are the sum of the bound × volume of its
//!   cells touching the sphere's unpadded radius, which errs high, as [`SystemSource`]'s contract
//!   allows, and does not depend on the query's time. Each band counts in the layer of its band.
//! - **Hits.** The cells are chosen within the padded radius, since members move; each member is
//!   then tested at the sphere's time, drifting in a straight line from its epoch position at its
//!   velocity, the cluster's bulk motion plus its own (P09.T10), against the unpadded radius.
//!   Members not yet born at that time (a young nursery's, drawn with ages from −H) are dropped.
//!
//! The census and the hits are two calls of the query, so a caller that passes a keeping
//! [`FeatureInteriorCache`] ([`KeepInteriors`] for one query) builds each interior once; with
//! [`NoInteriorCache`] every call builds its own.
//!
//! Sums run in a fixed order: features in the catalogue's cell and index order, then cells in the
//! grid's level and cell order, so the census is a function of the galaxy and the sphere alone.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::query::{
    LayerCounts, LayerSet, QuerySphere, SystemHit, SystemSource, pad_for, pad_speed,
};
use crate::time::UniverseTime;
use crate::units::{LightYears, Seconds};

use super::super::FeatureId;

use super::super::catalogue::{FeatureCatalogue, FeatureCellCache, FeatureRecord};
use super::local_offset;
use super::placement::FeatureInterior;

/// Where a caller keeps feature interiors between queries (P09.T23).
///
/// Like [`FeatureCellCache`] it takes `&self`, so an implementation that keeps interiors uses
/// interior mutability, and it must lend each exactly as [`FeatureInterior::of`] builds it. An
/// implementation that keeps them between calls keys them by galaxy as well as by feature.
pub trait FeatureInteriorCache {
    /// The interior of `feature` in `galaxy`, or `None` for a feature with no members.
    fn interior(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Arc<FeatureInterior>>;
}

/// The cache that keeps nothing: every lookup builds its interior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoInteriorCache;

impl FeatureInteriorCache for NoInteriorCache {
    fn interior(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Arc<FeatureInterior>> {
        FeatureInterior::of(galaxy, feature).map(Arc::new)
    }
}

/// A cache that keeps every interior it builds for one galaxy, the one it is made for: what a
/// single query passes so that its census and its hits build each interior once (ruling 139.6), and
/// what a caller that resolves several members of one feature reuses. It grows without bound, so a
/// long-lived caller uses plan 09's server cache (P09.T40) instead.
///
/// # Panics
///
/// If it is asked about another galaxy than its own: a bare galaxy and its
/// [`with_full_potential`](Galaxy::with_full_potential) share a seed and differ in their members.
#[derive(Debug)]
pub struct KeepInteriors<'a> {
    galaxy: &'a Galaxy,
    kept: Mutex<BTreeMap<FeatureId, Option<Arc<FeatureInterior>>>>,
}

impl<'a> KeepInteriors<'a> {
    /// An empty cache for `galaxy`.
    #[must_use]
    pub fn new(galaxy: &'a Galaxy) -> Self {
        Self {
            galaxy,
            kept: Mutex::new(BTreeMap::new()),
        }
    }
}

impl FeatureInteriorCache for KeepInteriors<'_> {
    fn interior(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Arc<FeatureInterior>> {
        assert!(
            std::ptr::eq(galaxy, self.galaxy),
            "an interior cache is for the galaxy it was made for"
        );
        let mut kept = self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        kept.entry(feature.id())
            .or_insert_with(|| FeatureInterior::of(galaxy, feature).map(Arc::new))
            .clone()
    }
}

/// The catalogue features' members as a source of the range query (module documentation).
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
/// use hyperion_sim::galaxy::features::members::source::{FeatureMemberSource, NoInteriorCache};
/// use hyperion_sim::galaxy::placement::NoCache;
/// use hyperion_sim::galaxy::query::{RangeQuery, range_query};
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::units::LightYears;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let source = FeatureMemberSource::new(&NoFeatureCache, &NoInteriorCache);
/// let here = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
/// let query = RangeQuery::builder(here, LightYears::new(50.0)).build()?;
/// let result = range_query(&galaxy, &mut NoCache::new(), &[&source], &query);
/// # let _ = result;
/// # Ok::<(), hyperion_sim::galaxy::query::BuildRangeQueryError>(())
/// ```
#[derive(Clone, Copy)]
pub struct FeatureMemberSource<'a> {
    features: &'a dyn FeatureCellCache,
    interiors: &'a dyn FeatureInteriorCache,
}

impl std::fmt::Debug for FeatureMemberSource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeatureMemberSource")
            .finish_non_exhaustive()
    }
}

impl<'a> FeatureMemberSource<'a> {
    /// The source that reads feature cells through `features` and interiors through `interiors`.
    #[must_use]
    pub fn new(
        features: &'a dyn FeatureCellCache,
        interiors: &'a dyn FeatureInteriorCache,
    ) -> Self {
        Self {
            features,
            interiors,
        }
    }

    /// The interiors of the features whose reach touches the sphere of `radius` about `centre`,
    /// in the catalogue's order.
    fn interiors_near(
        &self,
        galaxy: &Galaxy,
        centre: &GalacticPosition,
        radius: LightYears,
    ) -> impl Iterator<Item = Arc<FeatureInterior>> {
        FeatureCatalogue::near(galaxy, centre, radius, self.features)
            .filter_map(|f| self.interiors.interior(galaxy, &f))
    }
}

/// The bands from the coarsest down, the order the query walks its layers.
const BANDS_COARSEST_FIRST: [MassBand; 5] = [
    MassBand::E,
    MassBand::D,
    MassBand::C,
    MassBand::B,
    MassBand::A,
];

impl SystemSource for FeatureMemberSource<'_> {
    fn expected_in_sphere(&self, galaxy: &Galaxy, sphere: &QuerySphere) -> LayerCounts {
        let mut counts = LayerCounts::ZERO;
        let radius = sphere.radius();
        let mut cells = Vec::new();
        for interior in self.interiors_near(galaxy, sphere.centre(), radius) {
            let local = local_offset(interior.feature().position(), sphere.centre());
            cells.clear();
            cells.extend(interior.grid().cells_touching(&local, radius));
            for band in BANDS_COARSEST_FIRST {
                let expected = cells.iter().fold(0.0, |sum, &cell| {
                    sum + interior.expected_candidates(band, cell)
                });
                let layer = band.layer();
                counts.set(layer, counts.get(layer) + expected);
            }
        }
        counts
    }

    fn systems_in_sphere(
        &self,
        galaxy: &Galaxy,
        sphere: &QuerySphere,
        layers: LayerSet,
        out: &mut Vec<SystemHit>,
    ) {
        let t = sphere.time();
        let elapsed = Seconds::new(t.since_epoch().as_seconds_f64());
        let fastest = BANDS_COARSEST_FIRST
            .iter()
            .filter(|b| layers.contains(b.layer()))
            .map(|b| pad_for(t, pad_speed(b.layer())).value())
            .fold(0.0, f64::max);
        let padded = LightYears::new(sphere.radius().value() + fastest);
        let mut members = Vec::new();
        for interior in self.interiors_near(galaxy, sphere.centre(), padded) {
            let local = local_offset(interior.feature().position(), sphere.centre());
            for band in BANDS_COARSEST_FIRST {
                if !layers.contains(band.layer()) {
                    continue;
                }
                let reach = sphere.radius() + pad_for(t, pad_speed(band.layer()));
                for cell in interior.grid().cells_touching(&local, reach) {
                    interior.members_in_cell(galaxy, band, cell, &mut members);
                    for (member, _) in &members {
                        if let Some(hit) =
                            hit_at(member.record(), member.velocity(), sphere, elapsed)
                        {
                            out.push(hit);
                        }
                    }
                }
            }
        }
    }

    fn suppresses(&self, _galaxy: &Galaxy, _record: &SystemRecord, _t: UniverseTime) -> bool {
        false
    }
}

/// The hit a member makes in `sphere`, drifting at `velocity` for `elapsed` since the epoch, or
/// `None` if it is outside the unpadded radius then or not yet born.
#[must_use]
fn hit_at(
    record: &SystemRecord,
    velocity: GalacticVelocity,
    sphere: &QuerySphere,
    elapsed: Seconds,
) -> Option<SystemHit> {
    use crate::galaxy::placement::Existence;
    match record.existence_at(sphere.time()) {
        Existence::NoSystemYet => return None,
        Existence::Exists => {}
    }
    let position = if sphere.time() == UniverseTime::EPOCH {
        *record.epoch_position()
    } else {
        record
            .epoch_position()
            .translated(velocity.displacement_over(elapsed))
            .expect("a member moving under 3,000 km/s over the clock window stays addressable")
    };
    let distance = LightYears::from(sphere.centre().distance_to(&position));
    (distance.value() <= sphere.radius().value())
        .then(|| SystemHit::new(*record, position, distance))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};

    use super::*;
    use crate::galaxy::features::FeatureId;
    use crate::galaxy::features::FeatureProcess;
    use crate::galaxy::features::catalogue::{FeatureMarks, NoFeatureCache};
    use crate::galaxy::features::ids::PackedFeature;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::SystemOrigin;
    use crate::id::Layer;
    use crate::rng::Seed;

    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(0x0923_0000), GalaxyParams::milky_way_like())
                .unwrap()
                .with_full_potential()
        })
    }

    /// A cache that keeps everything it is asked for, keyed by feature cell and feature.
    #[derive(Default)]
    struct Keep {
        cells:
            Mutex<BTreeMap<[i32; 3], Arc<crate::galaxy::features::catalogue::FeatureCellContents>>>,
        interiors: Mutex<BTreeMap<FeatureId, Option<Arc<FeatureInterior>>>>,
    }

    impl FeatureCellCache for Keep {
        fn contents(
            &self,
            galaxy: &Galaxy,
            cell: crate::id::FeatureCell,
        ) -> Arc<crate::galaxy::features::catalogue::FeatureCellContents> {
            let mut cells = self.cells.lock().unwrap();
            Arc::clone(
                cells
                    .entry(cell.to_array())
                    .or_insert_with(|| Arc::new(FeatureCatalogue::cell(galaxy, cell))),
            )
        }
    }

    impl FeatureInteriorCache for Keep {
        fn interior(
            &self,
            galaxy: &Galaxy,
            feature: &FeatureRecord,
        ) -> Option<Arc<FeatureInterior>> {
            let mut interiors = self.interiors.lock().unwrap();
            interiors
                .entry(feature.id())
                .or_insert_with(|| FeatureInterior::of(galaxy, feature).map(Arc::new))
                .clone()
        }
    }

    /// A globular of 5,000–20,000 M☉ that has not collapsed, shared by the tests.
    fn globular() -> &'static FeatureInterior {
        static GLOBULAR: OnceLock<FeatureInterior> = OnceLock::new();
        GLOBULAR.get_or_init(|| {
            let galaxy = galaxy();
            FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
                .filter(|f| match f.marks() {
                    FeatureMarks::Globular(m) => (5e3..2e4).contains(&m.mass().value()),
                    _ => false,
                })
                .filter_map(|f| FeatureInterior::of(galaxy, &f))
                .find(|i| !i.model().is_core_collapsed())
                .expect("the fixture has such a globular")
        })
    }

    /// An old open cluster of a few thousand solar masses at birth, shared by the tests.
    fn open_cluster() -> &'static FeatureInterior {
        static OPEN: OnceLock<FeatureInterior> = OnceLock::new();
        OPEN.get_or_init(|| {
            let galaxy = galaxy();
            FeatureCatalogue::cell(galaxy, crate::id::FeatureCell::new([0, 6, 0]).unwrap())
                .features()
                .iter()
                .filter(|f| match f.marks() {
                    FeatureMarks::OpenCluster(m) => (3e3..3e4).contains(&m.initial_mass().value()),
                    _ => false,
                })
                .find_map(|f| FeatureInterior::of(galaxy, f))
                .expect("the fixture has such an open cluster")
        })
    }

    /// The hits of `interior`'s members in `sphere`, by a scan of every member.
    fn brute_force(interior: &FeatureInterior, sphere: &QuerySphere) -> Vec<SystemHit> {
        let galaxy = galaxy();
        let elapsed = Seconds::new(sphere.time().since_epoch().as_seconds_f64());
        let mut brute = Vec::new();
        let mut members = Vec::new();
        for band in MassBand::ALL {
            for cell in interior.grid().owned_cells() {
                interior.members_in_cell(galaxy, band, cell, &mut members);
                brute.extend(
                    members
                        .iter()
                        .filter_map(|(m, _)| hit_at(m.record(), m.velocity(), sphere, elapsed)),
                );
            }
        }
        brute.sort_by_key(SystemHit::id);
        brute
    }

    #[test]
    fn a_query_in_an_open_cluster_finds_every_member_a_scan_finds() {
        let galaxy = galaxy();
        let interior = open_cluster();
        let id = interior.feature().id();
        let [x, y, z] = interior.feature().position().to_light_years_f64();
        let centre = GalacticPosition::from_light_years([x + 1.5, y, z - 1.0]).unwrap();
        // The query's own cache: its census and hits build each interior once.
        let keep = KeepInteriors::new(galaxy);
        for years in [0, -600] {
            let sphere = sphere_at(centre, 12.0, years);
            let source = FeatureMemberSource::new(&NoFeatureCache, &keep);
            let mut hits = Vec::new();
            source.systems_in_sphere(galaxy, &sphere, all_layers(), &mut hits);
            let mut found: Vec<SystemHit> =
                hits.into_iter().filter(|h| of_feature(h, id)).collect();
            found.sort_by_key(SystemHit::id);
            let brute = brute_force(interior, &sphere);
            assert!(brute.len() > 20, "{} members inside 12 ly", brute.len());
            assert_eq!(found, brute, "at {years} years");
            // A source asked for band C alone returns only its members.
            let mut c_only = Vec::new();
            source.systems_in_sphere(galaxy, &sphere, LayerSet::EMPTY.with(Layer::C), &mut c_only);
            assert!(c_only.iter().all(|h| h.record().layer() == Layer::C));
        }
    }

    fn all_layers() -> LayerSet {
        [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
            .into_iter()
            .collect()
    }

    fn sphere_at(centre: GalacticPosition, radius: f64, years: i64) -> QuerySphere {
        let t = UniverseTime::from_julian_years(years).unwrap();
        QuerySphere::new(
            centre,
            LightYears::new(radius),
            t,
            pad_for(t, pad_speed(Layer::E)),
        )
        .unwrap()
    }

    fn of_feature(hit: &SystemHit, feature: FeatureId) -> bool {
        matches!(
            hit.record().origin(),
            SystemOrigin::FeatureMember { feature: f, .. } if f == PackedFeature::from(feature)
        )
    }

    #[test]
    #[ignore = "slow: every member of a globular of some ten thousand"]
    fn a_50_ly_query_in_a_globular_s_core_is_a_brute_force_enumeration() {
        let galaxy = galaxy();
        let interior = globular();
        let id = interior.feature().id();
        let [x, y, z] = interior.feature().position().to_light_years_f64();
        let centre = GalacticPosition::from_light_years([x + 3.0, y - 2.0, z + 1.0]).unwrap();
        for years in [0, 400] {
            let sphere = sphere_at(centre, 50.0, years);
            let source = FeatureMemberSource::new(&NoFeatureCache, &NoInteriorCache);
            let mut hits = Vec::new();
            source.systems_in_sphere(galaxy, &sphere, all_layers(), &mut hits);
            let mut from_source: Vec<SystemHit> =
                hits.into_iter().filter(|h| of_feature(h, id)).collect();
            from_source.sort_by_key(SystemHit::id);
            let brute = brute_force(interior, &sphere);
            assert!(brute.len() > 1_000, "{} members inside 50 ly", brute.len());
            assert_eq!(from_source, brute, "at {years} years");
        }
    }

    #[test]
    fn the_census_does_not_depend_on_what_is_cached() {
        let galaxy = galaxy();
        let interior = globular();
        let [x, y, z] = interior.feature().position().to_light_years_f64();
        let centre = GalacticPosition::from_light_years([x, y + 10.0, z]).unwrap();
        let sphere = sphere_at(centre, 30.0, 100);
        let cold = FeatureMemberSource::new(&NoFeatureCache, &NoInteriorCache)
            .expected_in_sphere(galaxy, &sphere);
        let keep = Keep::default();
        let interiors = KeepInteriors::new(galaxy);
        let warm = FeatureMemberSource::new(&keep, &interiors);
        // Warm the caches with another query first.
        let other = sphere_at(
            GalacticPosition::from_light_years([x + 40.0, y, z]).unwrap(),
            20.0,
            0,
        );
        let _ = warm.expected_in_sphere(galaxy, &other);
        assert_eq!(warm.expected_in_sphere(galaxy, &sphere), cold);
        assert_eq!(warm.expected_in_sphere(galaxy, &sphere), cold);
        // Every band counts in its own layer, and the census sees the cluster.
        assert!(cold.get(Layer::A) > cold.get(Layer::E));
        assert!(cold.get(Layer::A) > 100.0, "{cold:?}");
        assert!(cold.get(Layer::BrownDwarf) <= 0.0);
    }

    #[test]
    fn hits_count_in_their_band_s_layer_and_the_unborn_are_dropped() {
        let galaxy = galaxy();
        let interior = globular();
        let cell = interior
            .grid()
            .owned_cells()
            .max_by(|a, b| {
                interior
                    .expected_candidates(MassBand::C, *a)
                    .total_cmp(&interior.expected_candidates(MassBand::C, *b))
            })
            .unwrap();
        let mut members = Vec::new();
        interior.members_in_cell(galaxy, MassBand::C, cell, &mut members);
        let (member, _) = members.first().expect("the busiest cell has members");
        assert_eq!(member.record().layer(), Layer::C);
        let centre = *member.record().epoch_position();
        let at = |years| sphere_at(centre, 1.0, years);
        let zero = Seconds::new(0.0);
        assert!(hit_at(member.record(), member.velocity(), &at(0), zero).is_some());
        // A record born a century after the epoch is not there before then.
        let unborn = SystemRecord::from_parts(
            member.record().id(),
            centre,
            member.record().origin(),
            member.record().population(),
            member.record().primary_initial_mass(),
            crate::units::Years::new(-100.0),
        );
        let before = at(50);
        let elapsed = Seconds::new(before.time().since_epoch().as_seconds_f64());
        assert!(hit_at(&unborn, member.velocity(), &before, elapsed).is_none());
        let after = at(150);
        let elapsed = Seconds::new(after.time().since_epoch().as_seconds_f64());
        assert!(hit_at(&unborn, GalacticVelocity::default(), &after, elapsed).is_some());
    }
}
