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
//! - **Hits.** The cells are chosen within the padded radius, since members move: band by band,
//!   padded at the larger of the band's layer's [`pad_speed`] and the feature's own members' bound
//!   ([`FeatureInterior::member_speed_bound`], P09.T23.b), so that where the bound is the lower the
//!   cells are the layer's. The features are looked up about the sphere padded at
//!   [`UNBOUND_PAD_SPEED`], above every feature's bound. Each member is then tested at the sphere's
//!   time, drifting in a straight line from its epoch position at its velocity, the cluster's bulk
//!   motion plus its own (P09.T10), against the unpadded radius. Members not yet born at that time
//!   (a young nursery's, drawn with ages from −H) are dropped.
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
    LayerCounts, LayerSet, QuerySphere, SystemHit, SystemSource, UNBOUND_PAD_SPEED, pad_for,
    pad_speed,
};
use crate::id::Layer;
use crate::time::UniverseTime;
use crate::units::{KilometresPerSecond, LightYears, Seconds};

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
        // Every feature's members move below this (`FeatureInterior::of` asserts it in debug
        // builds), so a member that can reach the sphere by `t` lies in a feature whose reach
        // touches it padded so.
        let padded = sphere.radius() + pad_for(t, UNBOUND_PAD_SPEED);
        let mut members = Vec::new();
        for interior in self.interiors_near(galaxy, sphere.centre(), padded) {
            let local = local_offset(interior.feature().position(), sphere.centre());
            let bound = interior.member_speed_bound();
            for band in BANDS_COARSEST_FIRST {
                if !layers.contains(band.layer()) {
                    continue;
                }
                let reach = band_reach(sphere.radius(), t, band.layer(), bound);
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

/// How far from a query's centre a feature's cells of `layer` are visited, for a sphere of
/// `radius` at `t` (P09.T23.b).
///
/// The sphere is padded at the larger of the layer's [`pad_speed`] and the feature's members'
/// `bound` ([`FeatureInterior::member_speed_bound`]). Where the bound is the lower, the reach is
/// the layer's, bit for bit.
#[must_use]
fn band_reach(
    radius: LightYears,
    t: UniverseTime,
    layer: Layer,
    bound: KilometresPerSecond,
) -> LightYears {
    let speed = pad_speed(layer).value().max(bound.value());
    radius + pad_for(t, KilometresPerSecond::new(speed))
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

    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::features::FeatureId;
    use crate::galaxy::features::FeatureProcess;
    use crate::galaxy::features::catalogue::{FeatureMarks, NoFeatureCache};
    use crate::galaxy::features::ids::PackedFeature;
    use crate::galaxy::features::members::{MemberRecord, equipartition_factor};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::SystemOrigin;
    use crate::galaxy::query::PAD_SPEED;
    use crate::rng::Seed;

    /// The Milky Way fixture with its kinematic tables, built once per process.
    ///
    /// The build takes some 5 s in a test build, and nextest runs each test in a process of its
    /// own, so the fast properties that read it or the clusters below run in one test,
    /// [`the_member_source_finds_and_counts_what_a_scan_finds`].
    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(0x0923_0000), GalaxyParams::milky_way_like())
                .unwrap()
                .with_full_potential()
        })
    }

    /// The feature-member source (plan 09), each property below on one build of the fixture
    /// ([`galaxy`]) and of its clusters ([`open_cluster`], [`globular`]), in turn.
    #[test]
    fn the_member_source_finds_and_counts_what_a_scan_finds() {
        a_query_in_an_open_cluster_finds_every_member_a_scan_finds();
        the_census_does_not_depend_on_what_is_cached();
        hits_count_in_their_band_s_layer_and_the_unborn_are_dropped();
        a_cluster_s_speed_bound_is_its_bulk_and_larger_term_and_holds_its_members();
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

    /// Every member of `interior`, band by band over every owned cell.
    fn every_member(interior: &FeatureInterior) -> Vec<MemberRecord> {
        let galaxy = galaxy();
        let mut all = Vec::new();
        let mut members = Vec::new();
        for band in MassBand::ALL {
            for cell in interior.grid().owned_cells() {
                interior.members_in_cell(galaxy, band, cell, &mut members);
                all.extend(members.iter().map(|(m, _)| *m));
            }
        }
        all
    }

    /// Every member of the shared open cluster ([`open_cluster`]), enumerated once.
    fn open_cluster_members() -> &'static [MemberRecord] {
        static MEMBERS: OnceLock<Vec<MemberRecord>> = OnceLock::new();
        MEMBERS.get_or_init(|| every_member(open_cluster()))
    }

    /// The members of the shared globular's ([`globular`]) owned cell where band C expects the
    /// most candidates.
    fn globular_s_busiest_c_members() -> &'static [MemberRecord] {
        static MEMBERS: OnceLock<Vec<MemberRecord>> = OnceLock::new();
        MEMBERS.get_or_init(|| {
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
            interior.members_in_cell(galaxy(), MassBand::C, cell, &mut members);
            members.into_iter().map(|(m, _)| m).collect()
        })
    }

    /// The hits `members` make in `sphere`, by a scan of every one, in ID order.
    fn brute_force(members: &[MemberRecord], sphere: &QuerySphere) -> Vec<SystemHit> {
        let elapsed = Seconds::new(sphere.time().since_epoch().as_seconds_f64());
        let mut brute: Vec<SystemHit> = members
            .iter()
            .filter_map(|m| hit_at(m.record(), m.velocity(), sphere, elapsed))
            .collect();
        brute.sort_by_key(SystemHit::id);
        brute
    }

    /// Run on its one build of the fixture by
    /// [`the_member_source_finds_and_counts_what_a_scan_finds`].
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
            let brute = brute_force(open_cluster_members(), &sphere);
            assert!(brute.len() > 20, "{} members inside 12 ly", brute.len());
            assert_eq!(found, brute, "at {years} years");
            // A source asked for band C alone returns only its members, and all of them: the
            // feature lookup without layer E is padded as the one with it (P09.T23.b).
            let mut c_only = Vec::new();
            source.systems_in_sphere(galaxy, &sphere, LayerSet::EMPTY.with(Layer::C), &mut c_only);
            assert!(c_only.iter().all(|h| h.record().layer() == Layer::C));
            let mut c_found: Vec<SystemHit> =
                c_only.into_iter().filter(|h| of_feature(h, id)).collect();
            c_found.sort_by_key(SystemHit::id);
            let c_brute: Vec<SystemHit> = brute
                .into_iter()
                .filter(|h| h.record().layer() == Layer::C)
                .collect();
            assert!(!c_brute.is_empty(), "no band-C member inside 12 ly");
            assert_eq!(c_found, c_brute, "band C at {years} years");
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
        let members = every_member(interior);
        for years in [0, 400] {
            let sphere = sphere_at(centre, 50.0, years);
            let source = FeatureMemberSource::new(&NoFeatureCache, &NoInteriorCache);
            let mut hits = Vec::new();
            source.systems_in_sphere(galaxy, &sphere, all_layers(), &mut hits);
            let mut from_source: Vec<SystemHit> =
                hits.into_iter().filter(|h| of_feature(h, id)).collect();
            from_source.sort_by_key(SystemHit::id);
            let brute = brute_force(&members, &sphere);
            assert!(brute.len() > 1_000, "{} members inside 50 ly", brute.len());
            assert_eq!(from_source, brute, "at {years} years");
        }
    }

    /// Run on its one build of the fixture by
    /// [`the_member_source_finds_and_counts_what_a_scan_finds`].
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

    /// Run on its one build of the fixture by
    /// [`the_member_source_finds_and_counts_what_a_scan_finds`].
    fn hits_count_in_their_band_s_layer_and_the_unborn_are_dropped() {
        let member = globular_s_busiest_c_members()
            .first()
            .expect("the busiest cell has members");
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

    /// The two terms of `interior`'s members' speed bound, km/s, from its model's getters: the
    /// escape speed at the centre and 8.58 √2 times the largest σ of a tail class at the tidal
    /// radius (0 with no tail).
    fn bound_terms(interior: &FeatureInterior) -> (f64, f64) {
        let model = interior.model();
        let core = model.escape_speed_at(LightYears::new(0.0)).value();
        let largest = MassBand::ALL
            .iter()
            .filter_map(|&band| interior.table().tail(band))
            .map(|tail| {
                model.sigma(model.tidal_radius()).value()
                    * equipartition_factor(tail.class().mean_mass, model.turn_off_mass())
            })
            .fold(0.0, f64::max);
        (core, 8.58 * std::f64::consts::SQRT_2 * largest)
    }

    /// The fastest of `members`, km/s.
    fn top_speed(members: &[MemberRecord]) -> f64 {
        members
            .iter()
            .map(|m| KilometresPerSecond::from(m.velocity().speed()).value())
            .fold(0.0, f64::max)
    }

    /// Run on its one build of the fixture by
    /// [`the_member_source_finds_and_counts_what_a_scan_finds`].
    fn a_cluster_s_speed_bound_is_its_bulk_and_larger_term_and_holds_its_members() {
        for (name, interior, members) in [
            ("open cluster", open_cluster(), open_cluster_members()),
            ("globular", globular(), globular_s_busiest_c_members()),
        ] {
            let bound = interior.member_speed_bound().value();
            let (core, tail) = bound_terms(interior);
            let bulk = KilometresPerSecond::from(interior.bulk_velocity().speed()).value();
            let sum = bulk + core.max(tail);
            let fastest = top_speed(members);
            println!(
                "{name}: bound {bound:.3} km/s = bulk {bulk:.3} + max(core {core:.3}, tail \
                 {tail:.3}); fastest of {} members {fastest:.3}",
                members.len()
            );
            // Rounded up by 10⁻⁹ and no more.
            assert!(
                sum <= bound && bound <= sum * (1.0 + 2e-9),
                "{name}: {bound} against {sum}"
            );
            assert!(
                !members.is_empty() && fastest < bound,
                "{name}: {fastest} against {bound}"
            );
            assert!(bound < UNBOUND_PAD_SPEED.value(), "{name}: {bound}");
        }
    }

    /// A feature's cells of a layer are padded as the layer's wherever its members' bound is below
    /// the layer's pad speed, and by the bound where it is above.
    #[test]
    fn a_feature_s_reach_is_its_layer_s_unless_its_bound_is_faster() {
        let radius = LightYears::new(12.0);
        for years in [-1_000, -600, 1, 1_000] {
            let t = UniverseTime::from_julian_years(years).unwrap();
            for layer in [Layer::A, Layer::D, Layer::E] {
                let pad = pad_speed(layer);
                let today = radius + pad_for(t, pad);
                for below in [0.0, 164.0, 0.5 * pad.value(), pad.value()] {
                    let reach = band_reach(radius, t, layer, KilometresPerSecond::new(below));
                    assert_same_bits(reach.value(), today.value());
                }
                for above in [pad.value() * (1.0 + 1e-9), 1.2 * pad.value(), 4_000.0] {
                    let bound = KilometresPerSecond::new(above);
                    let reach = band_reach(radius, t, layer, bound);
                    assert_same_bits(reach.value(), (radius + pad_for(t, bound)).value());
                    assert!(reach.value() > today.value(), "{layer:?} at {years} years");
                }
            }
        }
        // At the epoch nothing has moved, however fast.
        let epoch = band_reach(
            radius,
            UniverseTime::EPOCH,
            Layer::A,
            KilometresPerSecond::new(2_000.0),
        );
        assert_same_bits(epoch.value(), radius.value());
    }

    /// The distance from the galactic centre within which the speed bound's slow tests take every
    /// feature with members.
    const INNER_RADIUS: LightYears = LightYears::new(2_000.0);

    /// The fewest features the speed bound's slow tests take: the inner set, topped up to this
    /// from the walks.
    const SAMPLED: usize = 200;

    /// The features the speed bound's slow tests read (P09.T23.b), built once per process.
    struct Sample {
        /// The inner set, then the rest.
        interiors: Vec<FeatureInterior>,
        /// How many of them are the inner set.
        inner: usize,
    }

    /// The inner set, every feature with an interior whose centre lies within [`INNER_RADIUS`] of
    /// the galactic centre, in [`FeatureCatalogue::near`]'s order; then, to [`SAMPLED`] in all,
    /// features with interiors outside it from [`FeatureCatalogue::walk_process`], a third of the
    /// rest each from the globulars, the old open clusters (the five sub-discs in turn) and the
    /// nurseries, in that order. Clouds have no interior.
    fn sample() -> &'static Sample {
        static SAMPLE: OnceLock<Sample> = OnceLock::new();
        SAMPLE.get_or_init(|| {
            let galaxy = galaxy();
            let centre = GalacticPosition::from_light_years([0.0; 3]).unwrap();
            let inside = move |f: &FeatureRecord| {
                LightYears::from(centre.distance_to(f.position())).value() <= INNER_RADIUS.value()
            };
            let mut interiors: Vec<FeatureInterior> =
                FeatureCatalogue::near(galaxy, &centre, INNER_RADIUS, &NoFeatureCache)
                    .filter(inside)
                    .filter_map(|f| FeatureInterior::of(galaxy, &f))
                    .collect();
            let inner = interiors.len();
            let rest = SAMPLED.saturating_sub(inner);
            let walk = move |process| {
                FeatureCatalogue::walk_process(galaxy, process)
                    .filter(move |f| !inside(f))
                    .filter_map(move |f| FeatureInterior::of(galaxy, &f))
            };
            interiors.extend(walk(FeatureProcess::Globular).take(rest / 3));
            let mut sub_discs: Vec<_> = crate::galaxy::ages::SubDisc::ALL
                .into_iter()
                .map(|s| walk(FeatureProcess::OldOpenCluster(s)))
                .collect();
            let mut open = Vec::new();
            let mut live = vec![true; sub_discs.len()];
            while open.len() < rest / 3 && live.contains(&true) {
                for (walk, live) in sub_discs.iter_mut().zip(live.iter_mut()) {
                    if *live && open.len() < rest / 3 {
                        match walk.next() {
                            Some(interior) => open.push(interior),
                            None => *live = false,
                        }
                    }
                }
            }
            interiors.extend(open);
            let nurseries = SAMPLED.saturating_sub(interiors.len());
            interiors.extend(walk(FeatureProcess::Nursery).take(nurseries));
            assert_eq!(interiors.len(), SAMPLED.max(inner));
            Sample { interiors, inner }
        })
    }

    /// The sampled feature whose members' bound is the largest.
    fn fastest_feature(sample: &Sample) -> &FeatureInterior {
        sample
            .interiors
            .iter()
            .max_by(|a, b| {
                a.member_speed_bound()
                    .value()
                    .total_cmp(&b.member_speed_bound().value())
            })
            .expect("the sample has features")
    }

    /// Every member of every sampled feature ([`sample`]): the inner set alone is 225 features at
    /// version 22, so the sample is the inner set. Plan 09's "P09.T23.b as built" gives the
    /// run-time rule it is kept whole under.
    #[test]
    #[ignore = "slow: the members of 225 features, 20 minutes"]
    fn every_member_of_the_sampled_features_moves_below_its_feature_s_bound() {
        let galaxy = galaxy();
        let sample = sample();
        let mut enumerated = 0_usize;
        let mut worst: Option<(f64, FeatureId)> = None;
        let mut members = Vec::new();
        for interior in &sample.interiors {
            let id = interior.feature().id();
            let bound = interior.member_speed_bound();
            let bound_m_s = crate::units::MetresPerSecond::from(bound).value();
            let mut n = 0_usize;
            for band in MassBand::ALL {
                for cell in interior.grid().owned_cells() {
                    interior.members_in_cell(galaxy, band, cell, &mut members);
                    for (member, _) in &members {
                        let speed = member.velocity().speed().value();
                        assert!(
                            speed < bound_m_s,
                            "{:x} of {} moves at {speed} m/s, its feature's bound {} km/s",
                            member.record().id(),
                            id.designation(),
                            bound.value()
                        );
                        let ratio = speed / bound_m_s;
                        if worst.is_none_or(|(r, _)| ratio > r) {
                            worst = Some((ratio, id));
                        }
                    }
                    n += members.len();
                }
            }
            enumerated += n;
            println!(
                "{} {:?}: {n} members, bound {:.3} km/s",
                id.designation(),
                interior.feature().process(),
                bound.value()
            );
        }
        let (ratio, id) = worst.expect("the features have members");
        println!(
            "{enumerated} members of {} features ({} inner); the largest speed over bound \
             {ratio:.6}, of {}",
            sample.interiors.len(),
            sample.inner,
            id.designation()
        );
    }

    /// Every sampled feature's members' bound is below the feature lookup's [`UNBOUND_PAD_SPEED`],
    /// and those at or above [`PAD_SPEED`] are counted and printed with the largest bound: none at
    /// version 22, the largest 688.5 km/s.
    #[test]
    #[ignore = "slow: the interiors of 225 features, 7 s"]
    fn the_sampled_features_bounds_stay_below_the_unbound_pad_and_the_fast_ones_are_counted() {
        let sample = sample();
        let mut by_process = BTreeMap::new();
        for (index, interior) in sample.interiors.iter().enumerate() {
            let process = interior.feature().process();
            let entry = by_process
                .entry(format!("{process:?}"))
                .or_insert((0_usize, 0_usize));
            if index < sample.inner {
                entry.0 += 1;
            } else {
                entry.1 += 1;
            }
            let bound = interior.member_speed_bound().value();
            assert!(
                bound < UNBOUND_PAD_SPEED.value(),
                "{}: {bound} km/s",
                interior.feature().id().designation()
            );
        }
        // A feature at or above the pad speed is a finding, not a failure: the query pads it by
        // its own bound, which the brute-force test at ±H checks.
        let fast = sample
            .interiors
            .iter()
            .filter(|i| i.member_speed_bound().value() >= PAD_SPEED.value())
            .count();
        let largest = fastest_feature(sample);
        let (core, tail) = bound_terms(largest);
        // What a feature outside the inner set could add to its bulk, and what bounds the bulk
        // there: the internal terms the sample takes, and the escape speed in the plane at the
        // inner set's edge.
        let internal = sample
            .interiors
            .iter()
            .map(|i| {
                let (core, tail) = bound_terms(i);
                core.max(tail)
            })
            .fold(0.0, f64::max);
        println!(
            "{} features, {} inner, by process (inner, rest) {by_process:?}; the largest internal \
             term {internal:.3} km/s; the escape speed in the plane at {} ly {:.3} km/s",
            sample.interiors.len(),
            sample.inner,
            INNER_RADIUS.value(),
            galaxy()
                .potential()
                .escape_speed_in_plane(INNER_RADIUS)
                .value()
        );
        println!(
            "{fast} at or above {} km/s; the largest bound {:.3} km/s, of {} ({:?}): bulk {:.3}, \
             core {core:.3} (the fitted central escape speed {:.3}), tail {tail:.3}",
            PAD_SPEED.value(),
            largest.member_speed_bound().value(),
            largest.feature().id().designation(),
            largest.feature().process(),
            KilometresPerSecond::from(largest.bulk_velocity().speed()).value(),
            largest.model().escape_speed_central().value()
        );
    }

    /// How many sampled features the brute-force test at ±H queries.
    const CENTRES: usize = 20;

    /// P09.T23's brute-force comparison at both ends of the clock window, in the 20 sampled
    /// features whose members' bound is the largest, the sample's fastest feature among
    /// them: a sphere of the half-mass radius off each centre, where the bulk motion alone carries
    /// the cluster some 2 ly by ±H.
    #[test]
    #[ignore = "slow: 40 queries and every member of 20 features, 10 minutes"]
    fn queries_at_plus_and_minus_h_find_what_a_scan_finds_in_the_20_fastest_sampled_features() {
        let galaxy = galaxy();
        let mut by_bound: Vec<&FeatureInterior> = sample().interiors.iter().collect();
        by_bound.sort_by(|a, b| {
            b.member_speed_bound()
                .value()
                .total_cmp(&a.member_speed_bound().value())
                .then(a.feature().id().cmp(&b.feature().id()))
        });
        by_bound.truncate(CENTRES);
        // The queries' own cache, so each interior they touch is built once.
        let keep = KeepInteriors::new(galaxy);
        let source = FeatureMemberSource::new(&NoFeatureCache, &keep);
        let mut found_any = 0;
        for interior in by_bound {
            let id = interior.feature().id();
            let members = every_member(interior);
            // A sphere of the half-mass radius, off the centre, whose edge cuts the cluster.
            let r_h = interior.model().half_mass_radius().value();
            let [x, y, z] = interior.feature().position().to_light_years_f64();
            let centre =
                GalacticPosition::from_light_years([x + 0.25 * r_h, y, z - 0.25 * r_h]).unwrap();
            let mut counts = Vec::new();
            for years in [-1_000, 1_000] {
                let sphere = sphere_at(centre, r_h, years);
                let mut hits = Vec::new();
                source.systems_in_sphere(galaxy, &sphere, all_layers(), &mut hits);
                let mut found: Vec<SystemHit> =
                    hits.into_iter().filter(|h| of_feature(h, id)).collect();
                found.sort_by_key(SystemHit::id);
                let brute = brute_force(&members, &sphere);
                assert_eq!(found, brute, "{} at {years} years", id.designation());
                counts.push(brute.len());
            }
            found_any += usize::from(counts.iter().any(|&n| n > 0));
            println!(
                "{} {:?}: bound {:.3} km/s, {} members, r_h {r_h:.2} ly, hits at −H and +H \
                 {counts:?}",
                id.designation(),
                interior.feature().process(),
                interior.member_speed_bound().value(),
                members.len()
            );
        }
        assert!(
            found_any >= CENTRES - 2,
            "{found_any} centres found members"
        );
    }
}
