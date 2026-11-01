//! Feature members in the range query: plan 03's merge hook for the catalogue features' nested
//! grids (plan 09, P09.T23; brainstorm, "The range query" and "Dense features").
//!
//! The range query treats a feature as one more stack of layers. For a query sphere
//! [`FeatureMemberSource`] takes the features whose reach touches it
//! ([`FeatureCatalogue::near`]) and whose nested grid may, by a bound from the cluster model that
//! needs no interior ([`FeatureInteriorCache::grid_reach`], P09.T40.a), and for each the owned
//! nested cells that touch it, band by band from the coarsest mass band down:
//!
//! - **Census.** A feature's expected members in a band are the sum of the bound × volume of its
//!   cells touching the sphere's unpadded radius, which errs high, as [`SystemSource`]'s contract
//!   allows, and does not depend on the query's time. Each band counts in the layer of its band.
//! - **Hits.** The cells are chosen within the padded radius, since members move: band by band,
//!   padded at the larger of the band's layer's [`pad_speed_in`] and the feature's own members' bound
//!   ([`FeatureInterior::member_speed_bound`], P09.T23.b), so that where the bound is the lower the
//!   cells are the layer's. The features are looked up about the sphere padded at
//!   [`UNBOUND_PAD_SPEED`], above every feature's bound. Only the candidates placed within the
//!   radius padded at the bound are drawn (`FeatureInterior::members_in_cell_near`, P09.T40.a),
//!   since no other can reach the sphere. Each member is then tested at the sphere's time,
//!   drifting in a straight line from its epoch position at its velocity, the cluster's bulk
//!   motion plus its own (P09.T10), against the unpadded radius. Members not yet born at that time
//!   (a young nursery's, drawn with ages from −H) are dropped.
//!
//! - **The lists** (P09.T37). Under P09.T36's hold (`FEATURE_CLASSES_VERSION`) a feature's
//!   feature-level list holds its recent classes, the recently dead and the luminous blue
//!   variables its band-E cells give up, and an association, which has no nested grid, holds them
//!   nowhere else. Each feature whose reach touches the sphere adds to the census each recent
//!   class's expected candidates times the share of its profile in the shell about the feature's
//!   centre that holds the sphere (`FeatureLevelList::add_recent_expected_within`), in the layer
//!   its primaries are of. The list holds the recent classes alone, as the interior cache lends it
//!   ([`FeatureInteriorCache::recent_list`]), so a query's census and hits build it once. An entry moves on one straight line, through its epoch position at the
//!   feature's bulk velocity, on which its site lies at its death (P09.T36), before its death and
//!   after it. `member_speed_bound` does not cover the lists, so a list has a pad of its own: its
//!   entries lie within the feature's reach of its centre at the epoch and move at the bulk speed,
//!   below [`UNBOUND_PAD_SPEED`], so the features looked up for the hits hold every list that can
//!   reach the sphere. Each candidate's line is tested against the sphere before its class's test,
//!   a lifetime and a star, is asked, and a kept entry is a hit as a member is
//!   ([`SystemOrigin::FeatureMember`](crate::galaxy::placement::SystemOrigin::FeatureMember)), its
//!   ID resolved by [`resolve_member`](super::resolve_member). Below the hold's version no list
//!   is read, bit for bit.
//!
//! The census and the hits are two calls of the query, so a caller that passes a keeping
//! [`FeatureInteriorCache`] ([`KeepInteriors`] for one query) builds each interior once; with
//! [`NoInteriorCache`] every call builds its own.
//!
//! Sums run in a fixed order: features in the catalogue's cell and index order, each feature's list
//! before its cells, then cells in the grid's level and cell order, so the census is a function of
//! the galaxy and the sphere alone.

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::query::{
    LayerCounts, LayerSet, QuerySphere, RangeQuery, SystemHit, SystemSource, UNBOUND_PAD_SPEED,
    pad_for, pad_speed_in, widest_sphere,
};
use crate::id::{FeatureCell, Layer};
use crate::time::UniverseTime;
use crate::units::{KilometresPerSecond, LightYears, Seconds};

use super::super::{FeatureId, FeatureProcess};

use super::super::catalogue::{FeatureCatalogue, FeatureCellCache, FeatureRecord};
use super::super::interior::LocalCell;
use super::super::nested::{NestedCell, box_distance};
use super::level_list::FeatureLevelList;
use super::placement::FeatureInterior;
use super::recent::{FeatureClassesRules, RecentClass};
use super::{MemberRecord, local_offset};

/// Where a caller keeps feature interiors between queries (P09.T23).
///
/// Like [`FeatureCellCache`] it takes `&self`, so an implementation that keeps interiors uses
/// interior mutability, and it must lend each exactly as [`FeatureInterior::of`] builds it. An
/// implementation that keeps them between calls keys them by galaxy as well as by feature.
pub trait FeatureInteriorCache {
    /// The interior of `feature` in `galaxy`, or `None` for a feature with no members.
    fn interior(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Arc<FeatureInterior>>;

    /// A bound on the reach of `feature`'s nested grid in `galaxy`, ly, or `None` for a feature
    /// with no members: [`FeatureInterior::grid_reach_bound`], which an implementation may keep,
    /// or the reach of an interior it holds (P09.T40.a). An implementation never returns less
    /// than the reach of the grid [`interior`](Self::interior) lends, and `None` only where it
    /// lends none.
    fn grid_reach(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<LightYears> {
        FeatureInterior::grid_reach_bound(galaxy, feature)
    }

    /// The list of `feature`'s recent classes in `galaxy` (P09.T37), which an implementation may
    /// keep: exactly what `FeatureLevelList::recent_of` builds, `None` for a feature that registers
    /// none. A range query's census and its hits both read it.
    fn recent_list(
        &self,
        galaxy: &Galaxy,
        feature: &FeatureRecord,
    ) -> Option<Arc<FeatureLevelList>> {
        FeatureLevelList::recent_of(galaxy, feature).map(Arc::new)
    }
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
    reaches: Mutex<BTreeMap<FeatureId, Option<LightYears>>>,
    lists: Mutex<BTreeMap<FeatureId, Option<Arc<FeatureLevelList>>>>,
}

impl<'a> KeepInteriors<'a> {
    /// An empty cache for `galaxy`.
    #[must_use]
    pub fn new(galaxy: &'a Galaxy) -> Self {
        Self {
            galaxy,
            kept: Mutex::new(BTreeMap::new()),
            reaches: Mutex::new(BTreeMap::new()),
            lists: Mutex::new(BTreeMap::new()),
        }
    }

    /// Checks that `galaxy` is the cache's own.
    ///
    /// # Panics
    ///
    /// If it is not.
    fn check(&self, galaxy: &Galaxy) {
        assert!(
            std::ptr::eq(galaxy, self.galaxy),
            "an interior cache is for the galaxy it was made for"
        );
    }
}

impl FeatureInteriorCache for KeepInteriors<'_> {
    fn interior(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Arc<FeatureInterior>> {
        self.check(galaxy);
        let mut kept = self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        kept.entry(feature.id())
            .or_insert_with(|| FeatureInterior::of(galaxy, feature).map(Arc::new))
            .clone()
    }

    /// The reach of the interior it keeps, or the bound, kept.
    fn grid_reach(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<LightYears> {
        self.check(galaxy);
        if let Some(interior) = self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&feature.id())
        {
            return interior.as_ref().map(|interior| interior.grid().reach());
        }
        *self
            .reaches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(feature.id())
            .or_insert_with(|| FeatureInterior::grid_reach_bound(galaxy, feature))
    }

    /// The list it keeps, or the list built and kept.
    fn recent_list(
        &self,
        galaxy: &Galaxy,
        feature: &FeatureRecord,
    ) -> Option<Arc<FeatureLevelList>> {
        self.check(galaxy);
        self.lists
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(feature.id())
            .or_insert_with(|| FeatureLevelList::recent_of(galaxy, feature).map(Arc::new))
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

    /// The feature cells the source reads for `query` in `galaxy`, each with the last process it
    /// reads it through (P09.T40.a).
    ///
    /// They are [`FeatureCatalogue::cells_near`]'s about the query's centre at the radius its hits
    /// are looked up at, padded for its members' motion to its time, which is at least the radius
    /// its counts are looked up at: so no lookup of the query reads another cell, or one through a
    /// later process. A server generates them ahead of the query, on several threads; what the
    /// source finds does not depend on it.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::features::catalogue::FeatureCatalogue;
    /// use hyperion_sim::galaxy::features::members::source::FeatureMemberSource;
    /// use hyperion_sim::galaxy::query::RangeQuery;
    /// use hyperion_sim::units::LightYears;
    ///
    /// let galaxy = Galaxy::new(Seed::new(9));
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
    /// let query = RangeQuery::builder(sun, LightYears::new(50.0)).build()?;
    /// // At the epoch the members have not moved, so the cells are the lookup's own.
    /// assert_eq!(
    ///     FeatureMemberSource::feature_cells(&galaxy, &query),
    ///     FeatureCatalogue::cells_near(&galaxy, &sun, LightYears::new(50.0)),
    /// );
    /// # Ok::<(), hyperion_sim::galaxy::query::BuildRangeQueryError>(())
    /// ```
    #[must_use]
    pub fn feature_cells(
        galaxy: &Galaxy,
        query: &RangeQuery,
    ) -> Vec<(FeatureCell, FeatureProcess)> {
        let sphere = widest_sphere(galaxy, query);
        FeatureCatalogue::cells_near(galaxy, sphere.centre(), lookup_radius(&sphere))
    }

    /// Reads `cell` through `last` for `query` as its lookups read it, through the caches this
    /// source was made with, and for each feature there that they find, its grid's reach bound
    /// and, where that grid may meet the query's sphere, its interior (P09.T40.a).
    ///
    /// It is what a server does for each of a query's [`feature_cells`](Self::feature_cells) on
    /// several threads before the query runs, so that the query's lookups find all of it held.
    /// What it reads and builds is each a pure function of the galaxy and the feature, so it
    /// changes no answer, and it is lost if the caches keep nothing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
    /// use hyperion_sim::galaxy::features::members::source::{FeatureMemberSource, KeepInteriors};
    /// use hyperion_sim::galaxy::placement::NoCache;
    /// use hyperion_sim::galaxy::query::{RangeQuery, range_query};
    /// use hyperion_sim::units::LightYears;
    ///
    /// let galaxy = Galaxy::new(Seed::new(9));
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
    /// let query = RangeQuery::builder(sun, LightYears::new(50.0)).build()?;
    /// let interiors = KeepInteriors::new(&galaxy);
    /// let source = FeatureMemberSource::new(&NoFeatureCache, &interiors);
    /// // Each cell may be read ahead on a thread of its own; the interiors are kept for the query.
    /// for cell in FeatureMemberSource::feature_cells(&galaxy, &query) {
    ///     source.read_ahead(&galaxy, &query, cell);
    /// }
    /// let result = range_query(&galaxy, &mut NoCache::new(), &[&source], &query);
    /// # let _ = result;
    /// # Ok::<(), hyperion_sim::galaxy::query::BuildRangeQueryError>(())
    /// ```
    pub fn read_ahead(
        &self,
        galaxy: &Galaxy,
        query: &RangeQuery,
        (cell, last): (FeatureCell, FeatureProcess),
    ) {
        let sphere = widest_sphere(galaxy, query);
        let (centre, radius) = (sphere.centre(), lookup_radius(&sphere));
        let mut found = Vec::new();
        FeatureCatalogue::found_in(
            galaxy,
            self.features,
            (cell, last),
            (centre, radius),
            &mut found,
        );
        for feature in &found {
            let touches = self
                .interiors
                .grid_reach(galaxy, feature)
                .is_some_and(|reach| grid_may_touch(feature, centre, reach, radius));
            if touches {
                drop(self.interiors.interior(galaxy, feature));
            }
        }
    }

    /// The features whose reach touches the sphere of `radius` about `centre`, in the catalogue's
    /// order, each with its interior where its nested grid may meet the sphere: those whose grid's
    /// reach bound does ([`FeatureInteriorCache::grid_reach`]; P09.T40.a). A grid that misses the
    /// sphere has no cell touching it, so its feature adds nothing from its cells to the census or
    /// the hits, and its interior is not built. Under P09.T36's held arm, `lists`, only the
    /// features with an interior are given, since nothing else of theirs is read.
    fn near(
        &self,
        galaxy: &Galaxy,
        centre: &GalacticPosition,
        radius: LightYears,
        lists: FeatureClassesRules,
    ) -> impl Iterator<Item = (FeatureRecord, Option<Arc<FeatureInterior>>)> {
        FeatureCatalogue::near(galaxy, centre, radius, self.features).filter_map(move |f| {
            let interior = self
                .interiors
                .grid_reach(galaxy, &f)
                .is_some_and(|reach| grid_may_touch(&f, centre, reach, radius))
                .then(|| self.interiors.interior(galaxy, &f))
                .flatten();
            let given = match lists {
                FeatureClassesRules::Unlisted => interior.is_some(),
                FeatureClassesRules::Listed => true,
            };
            given.then_some((f, interior))
        })
    }

    /// Each member of `band` that may lie between `inner` and `outer` of `centre` at some time of
    /// `window`, born by its end, given to `each` in the source's order: features as the
    /// catalogue lists them, then cells in the grid's order, then members in index order. It may
    /// give a member beyond the shell, never leave one out. What the sky's census reads, a shell
    /// at a time (rendering plan R06, R06.T16.a), in `parts` parts: the part `part` gives the
    /// members of the cells whose own key falls to it ([`part_of_cell`]), so that the parts give
    /// each member once between them, whatever their number and whatever the caches hold.
    ///
    /// Each feature is padded by its own members' speed bound
    /// ([`FeatureInterior::member_speed_bound`], P09.T23.b), never by a layer's pad: a member
    /// within the shell at some time of the window lies, at the epoch, within that pad of it over
    /// the window's farthest time from the epoch. Only the candidates within `outer` so padded are
    /// drawn, and a cell that lies wholly within `inner` less the pad is not visited.
    pub(crate) fn members_in_shell_over(
        &self,
        galaxy: &Galaxy,
        centre: &GalacticPosition,
        window: (UniverseTime, UniverseTime),
        ((band, inner, outer), (part, parts)): (
            (MassBand, LightYears, LightYears),
            (u32, NonZeroU32),
        ),
        each: &mut dyn FnMut(&MemberRecord),
    ) {
        let (from, to) = window;
        let farthest = if from.since_epoch().as_seconds_f64().abs()
            > to.since_epoch().as_seconds_f64().abs()
        {
            from
        } else {
            to
        };
        let lookup = outer + pad_for(farthest, UNBOUND_PAD_SPEED);
        let mut members = Vec::new();
        // Only the features with an interior have members in cells: `Unlisted` gives just those.
        let interiors = self
            .near(galaxy, centre, lookup, FeatureClassesRules::Unlisted)
            .filter_map(|(_, interior)| interior);
        for interior in interiors {
            let local = local_offset(interior.feature().position(), centre);
            let near = [local.x, local.y, local.z];
            let bound = interior.member_speed_bound();
            let within = member_reach(outer, farthest, bound);
            // The margins `member_reach` adds, taken off the inner edge too.
            let inside = inner.value() - (within.value() - outer.value());
            let mut memo = interior.bound_memo();
            for cell in interior.grid().cells_touching(&local, within) {
                if box_farthest(&interior.grid().local_cell(cell), near) < inside {
                    continue;
                }
                if part_of_cell(interior.feature().id(), (band, cell), parts) != part {
                    continue;
                }
                interior.members_in_cell_near(
                    galaxy,
                    band,
                    cell,
                    &local,
                    within,
                    (&mut members, &mut memo),
                );
                for (member, _) in &members {
                    use crate::galaxy::placement::Existence;
                    if matches!(member.record().existence_at(to), Existence::Exists) {
                        each(member);
                    }
                }
            }
        }
    }
}

/// Which of `parts` parts takes the members of `band` in `feature`'s nested cell `cell`: a fixed
/// mix of the feature's object word, the band, the cell's level and coordinates (a `SplitMix64`
/// step at each, Stafford's Mix13; Vigna 2015, `splitmix64.c`), modulo the parts. A
/// function of the cell alone, so no visit's order or cache changes which part holds a member.
#[must_use]
fn part_of_cell(
    feature: FeatureId,
    (band, cell): (MassBand, NestedCell),
    parts: NonZeroU32,
) -> u32 {
    let [x, y, z] = cell.cell();
    let band = u64::try_from(band.index()).expect("a band's index is below seven");
    let key = [
        band,
        u64::from(cell.level()),
        u64::from(x),
        u64::from(y),
        u64::from(z),
    ]
    .into_iter()
    .fold(feature.object_word(), |word, part| mix(word ^ part));
    u32::try_from(key % u64::from(parts.get())).expect("below a u32's parts")
}

/// One `SplitMix64` step from the state `word`: the golden gamma, then Stafford's Mix13 (Stafford
/// 2011, "Better bit mixing"), as Vigna's 2015 `splitmix64.c` gives it.
#[must_use]
const fn mix(word: u64) -> u64 {
    let mut z = word.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The distance from the local point `p` to the farthest point of `cell`'s box, ly.
#[must_use]
fn box_farthest(cell: &LocalCell, p: [f64; 3]) -> f64 {
    let mut sum = 0.0;
    for (&lo, &c) in cell.min.iter().zip(&p) {
        let d = (c - lo).abs().max((lo + cell.edge - c).abs());
        sum += d * d;
    }
    sum.sqrt()
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
        let lists = galaxy.rules().feature_classes;
        let mut cells = Vec::new();
        for (feature, interior) in self.near(galaxy, sphere.centre(), radius, lists) {
            if lists == FeatureClassesRules::Listed
                && let Some(list) = self.interiors.recent_list(galaxy, &feature)
            {
                list.add_recent_expected_within(sphere.centre(), radius, &mut counts);
            }
            let Some(interior) = interior else {
                continue;
            };
            let local = local_offset(interior.feature().position(), sphere.centre());
            cells.clear();
            cells.extend(interior.grid().cells_touching(&local, radius));
            // Most cells' bounds repeat a mirror's (`BoundMemo`): two fifths of a globular core's
            // 50 ly query outside its remnant draws were these, measured on a pod of 16 vCPU.
            let mut memo = interior.bound_memo();
            for band in BANDS_COARSEST_FIRST {
                let expected = cells.iter().fold(0.0, |sum, &cell| {
                    sum + interior.expected_candidates_with(band, cell, &mut memo)
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
        let padded = lookup_radius(sphere);
        let lists = galaxy.rules().feature_classes;
        let mut members = Vec::new();
        for (feature, interior) in self.near(galaxy, sphere.centre(), padded, lists) {
            if lists == FeatureClassesRules::Listed
                && let Some(list) = self.interiors.recent_list(galaxy, &feature)
            {
                recent_hits(galaxy, (&feature, &list), sphere, layers, out);
            }
            let Some(interior) = interior else {
                continue;
            };
            let local = local_offset(interior.feature().position(), sphere.centre());
            let mut memo = interior.bound_memo();
            let bound = interior.member_speed_bound();
            let within = member_reach(sphere.radius(), t, bound);
            for band in BANDS_COARSEST_FIRST {
                if !layers.contains(band.layer()) {
                    continue;
                }
                let reach = band_reach(galaxy, sphere.radius(), t, band.layer(), bound);
                for cell in interior.grid().cells_touching(&local, reach) {
                    interior.members_in_cell_near(
                        galaxy,
                        band,
                        cell,
                        &local,
                        within,
                        (&mut members, &mut memo),
                    );
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

/// How far from `sphere`'s centre the source looks up features for its hits: its radius padded by
/// [`UNBOUND_PAD_SPEED`] to its time, which every feature's members move below
/// ([`FeatureInterior::of`] asserts it).
#[must_use]
fn lookup_radius(sphere: &QuerySphere) -> LightYears {
    sphere.radius() + pad_for(sphere.time(), UNBOUND_PAD_SPEED)
}

/// Whether a grid of `reach` about `feature`'s centre may hold a cell that the ball of `radius`
/// about `centre` touches: whether the ball meets the grid's cube, `[−reach, reach]³`.
///
/// Every owned cell lies in the cube, with its outer faces on the cube's, so the distance to the
/// cube, taken as [`NestedGrid::cells_touching`](super::super::nested::NestedGrid::cells_touching)
/// takes a cell's, is never more than a cell's in floating point either: where this is false, no
/// cell touches the ball.
#[must_use]
fn grid_may_touch(
    feature: &FeatureRecord,
    centre: &GalacticPosition,
    reach: LightYears,
    radius: LightYears,
) -> bool {
    let local = local_offset(feature.position(), centre);
    let reach = reach.value();
    let cube = LocalCell {
        min: [-reach; 3],
        edge: 2.0 * reach,
    };
    box_distance(&cube, [local.x, local.y, local.z]) <= radius.value()
}

/// How far from a query's centre a feature's cells of `layer` are visited, for a sphere of
/// `radius` at `t` (P09.T23.b).
///
/// The sphere is padded at the larger of the layer's pad in `galaxy` ([`pad_speed_in`]) and the
/// feature's members' `bound` ([`FeatureInterior::member_speed_bound`]). Where the bound is the
/// lower, the reach is the layer's, bit for bit.
#[must_use]
pub(crate) fn band_reach(
    galaxy: &Galaxy,
    radius: LightYears,
    t: UniverseTime,
    layer: Layer,
    bound: KilometresPerSecond,
) -> LightYears {
    let speed = pad_speed_in(galaxy, layer).value().max(bound.value());
    radius + pad_for(t, KilometresPerSecond::new(speed))
}

/// How far from a query's centre a member at the epoch may lie and still be inside its sphere of
/// `radius` at `t`, for members moving below `bound` (P09.T40.a): the radius plus the farthest
/// such a member drifts, with margins over the rounding.
///
/// A member's local position, which this is compared with, and its galactic one, which
/// [`hit_at`] tests, differ by the rounding of a translation and a distance, some 10⁻¹² ly at a
/// feature's 4,096 ly reach; the margins are far above it, and above the rounding of the drift, so
/// that no member [`hit_at`] keeps is left undrawn.
#[must_use]
fn member_reach(radius: LightYears, t: UniverseTime, bound: KilometresPerSecond) -> LightYears {
    /// The relative margin on the drift.
    const DRIFT_MARGIN: f64 = 1e-9;
    /// The absolute margin, ly.
    const NEAR_MARGIN_LY: f64 = 1e-6;
    LightYears::new(
        radius.value() + pad_for(t, bound).value() * (1.0 + DRIFT_MARGIN) + NEAR_MARGIN_LY,
    )
}

/// The hit a member makes in `sphere`, drifting at `velocity` for `elapsed` since the epoch, or
/// `None` if it is outside the unpadded radius then or not yet born.
#[must_use]
pub(crate) fn hit_at(
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
    let position = drifted_at(record, velocity, sphere.time(), elapsed);
    let distance = LightYears::from(sphere.centre().distance_to(&position));
    (distance.value() <= sphere.radius().value())
        .then(|| SystemHit::new(*record, position, distance))
}

/// Where a member drifting at `velocity` from `record`'s epoch position is at `t`, `elapsed` since
/// the epoch: the position its hit carries ([`hit_at`]), its epoch position at the epoch.
///
/// # Panics
///
/// If the drift leaves the addressable range, which nothing below light's speed does from the
/// root cube within the source horizon.
#[must_use]
pub(crate) fn drifted_at(
    record: &SystemRecord,
    velocity: GalacticVelocity,
    t: UniverseTime,
    elapsed: Seconds,
) -> GalacticPosition {
    drifted_from(record.epoch_position(), velocity, t, elapsed)
}

/// Where a member at `epoch_position` at the epoch is at `t`, drifting at `velocity` for `elapsed`
/// since the epoch: [`drifted_at`]'s position, bit for bit, from the position alone (P09.T37: a
/// list's candidate is tested on its line before its record is drawn).
///
/// # Panics
///
/// As [`drifted_at`].
#[must_use]
fn drifted_from(
    epoch_position: &GalacticPosition,
    velocity: GalacticVelocity,
    t: UniverseTime,
    elapsed: Seconds,
) -> GalacticPosition {
    if t == UniverseTime::EPOCH {
        return *epoch_position;
    }
    epoch_position
        .translated(velocity.displacement_over(elapsed))
        .expect("a member moving under 3,000 km/s over the clock window stays addressable")
}

/// The hits in `sphere` among `layers` of `feature`'s recent entries, its `list` (module
/// documentation, "The lists"), appended to `out` in index order.
///
/// # Panics
///
/// In debug builds, if the feature's bulk speed is not below [`UNBOUND_PAD_SPEED`], which the
/// features' lookup assumes, or an entry's member is not on its candidate's line.
fn recent_hits(
    galaxy: &Galaxy,
    (feature, list): (&FeatureRecord, &FeatureLevelList),
    sphere: &QuerySphere,
    layers: LayerSet,
    out: &mut Vec<SystemHit>,
) {
    let Some(stars) = list.recent_stars() else {
        return;
    };
    let velocity = stars.velocity();
    let speed = KilometresPerSecond::from(velocity.speed());
    debug_assert!(
        speed.value() < UNBOUND_PAD_SPEED.value(),
        "{:?}: a feature's bulk speed {speed:?} is below the features' lookup pad",
        feature.id()
    );
    // The list's pad: its entries lie within the reach of the feature's centre at the epoch.
    let within = member_reach(sphere.radius() + feature.reach(), sphere.time(), speed);
    if LightYears::from(sphere.centre().distance_to(feature.position())).value() > within.value() {
        return;
    }
    let elapsed = Seconds::new(sphere.time().since_epoch().as_seconds_f64());
    for entry in list.entries() {
        let Some(class) = RecentClass::of(entry.class()) else {
            continue;
        };
        if !layers.contains(class.layer()) {
            continue;
        }
        let at = drifted_from(entry.position(), velocity, sphere.time(), elapsed);
        if LightYears::from(sphere.centre().distance_to(&at)).value() > sphere.radius().value() {
            continue;
        }
        let Some(recent) = list.recent_entry(galaxy, entry.index()) else {
            continue;
        };
        let member = recent.member();
        debug_assert!(
            member.record().epoch_position() == entry.position() && member.velocity() == velocity,
            "{:?}: an entry's member moves on its candidate's line",
            member.record().id()
        );
        if layers.contains(member.record().layer()) {
            out.extend(hit_at(member.record(), velocity, sphere, elapsed));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};

    use hyperion_testkit::float::{assert_same_bits, bits};
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::galaxy::PointLy;
    use crate::galaxy::features::FeatureId;
    use crate::galaxy::features::FeatureProcess;
    use crate::galaxy::features::catalogue::{FeatureMarks, NoFeatureCache};
    use crate::galaxy::features::cluster::ClusterParameters;
    use crate::galaxy::features::ids::PackedFeature;
    use crate::galaxy::features::interior::counts::{TailReach, tail_window};
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
        skipping_the_grids_that_miss_the_sphere_changes_nothing();
        the_answers_do_not_depend_on_the_order_queries_are_asked_in();
        the_feature_cells_cover_every_lookup_and_reading_them_ahead_changes_nothing();
        drawing_only_the_near_members_keeps_every_near_member();
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

    /// A cache that builds every interior and bounds no grid: the source as it was before it
    /// tested the grids' reach (P09.T40.a).
    struct EveryInterior<'a>(KeepInteriors<'a>);

    impl FeatureInteriorCache for EveryInterior<'_> {
        fn interior(
            &self,
            galaxy: &Galaxy,
            feature: &FeatureRecord,
        ) -> Option<Arc<FeatureInterior>> {
            self.0.interior(galaxy, feature)
        }

        fn grid_reach(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<LightYears> {
            self.interior(galaxy, feature).map(|_| LightYears::new(1e9))
        }
    }

    /// Run on its one build of the fixture by
    /// [`the_member_source_finds_and_counts_what_a_scan_finds`].
    fn skipping_the_grids_that_miss_the_sphere_changes_nothing() {
        let galaxy = galaxy();
        let [x, y, z] = open_cluster().feature().position().to_light_years_f64();
        let centre = GalacticPosition::from_light_years([x + 1.5, y, z - 1.0]).unwrap();
        let every = EveryInterior(KeepInteriors::new(galaxy));
        let skipping = KeepInteriors::new(galaxy);
        for years in [0, -600] {
            let sphere = sphere_at(centre, 40.0, years);
            let old = FeatureMemberSource::new(&NoFeatureCache, &every);
            let new = FeatureMemberSource::new(&NoFeatureCache, &skipping);
            let (before, after) = (
                old.expected_in_sphere(galaxy, &sphere),
                new.expected_in_sphere(galaxy, &sphere),
            );
            for layer in Layer::ALL {
                assert_same_bits(after.get(layer), before.get(layer));
            }
            let (mut before, mut after) = (Vec::new(), Vec::new());
            old.systems_in_sphere(galaxy, &sphere, all_layers(), &mut before);
            new.systems_in_sphere(galaxy, &sphere, all_layers(), &mut after);
            assert!(before.len() > 20, "{} hits", before.len());
            assert_eq!(after, before, "at {years} years");
        }
        // The bound never falls short of a grid, is `None` just where there is no interior, and
        // keeps some grids clear of the sphere.
        let near = FeatureCatalogue::near(galaxy, &centre, LightYears::new(40.0), &NoFeatureCache);
        let mut skipped = 0;
        for feature in near {
            let bound = FeatureInterior::grid_reach_bound(galaxy, &feature);
            let interior = every.interior(galaxy, &feature);
            assert_eq!(bound.is_some(), interior.is_some(), "{:?}", feature.id());
            if let (Some(bound), Some(interior)) = (bound, interior) {
                assert!(bound.value() >= interior.grid().reach().value());
                // The bound reads the model's tidal radius and tail without the model.
                let parameters = ClusterParameters::of_record(galaxy, &feature).unwrap();
                assert_eq!(parameters, *interior.model().parameters());
                let tidal = parameters.tidal_radius(galaxy);
                assert_same_bits(tidal.value(), interior.model().tidal_radius().value());
                let tail = TailReach::of(
                    galaxy,
                    &parameters.position,
                    tidal,
                    parameters.age,
                    feature.reach(),
                );
                let window = tail_window(galaxy, interior.model(), feature.reach());
                assert_same_bits(tail.length.value(), window.length.value());
                if !grid_may_touch(&feature, &centre, bound, LightYears::new(40.0)) {
                    skipped += 1;
                }
            }
        }
        assert!(skipped > 0, "no grid is kept clear");
    }

    /// Run on its one build of the fixture by
    /// [`the_member_source_finds_and_counts_what_a_scan_finds`]: the census, bit for bit, and the
    /// hits of spheres near the open cluster, asked through one keeping cache in every order, and
    /// through a fresh one each, whether a feature is tested by its bound or its kept grid's reach
    /// (P09.T40.a).
    fn the_answers_do_not_depend_on_the_order_queries_are_asked_in() {
        let galaxy = galaxy();
        let [x, y, z] = open_cluster().feature().position().to_light_years_f64();
        let spheres = [
            sphere_at(
                GalacticPosition::from_light_years([x + 1.5, y, z - 1.0]).unwrap(),
                12.0,
                0,
            ),
            sphere_at(
                GalacticPosition::from_light_years([x - 30.0, y, z]).unwrap(),
                25.0,
                0,
            ),
            sphere_at(
                GalacticPosition::from_light_years([x, y + 4.0, z]).unwrap(),
                6.0,
                -600,
            ),
        ];
        let answer = |interiors: &dyn FeatureInteriorCache, sphere: &QuerySphere| {
            let source = FeatureMemberSource::new(&NoFeatureCache, interiors);
            let census = source.expected_in_sphere(galaxy, sphere);
            let mut hits = Vec::new();
            source.systems_in_sphere(galaxy, sphere, all_layers(), &mut hits);
            let bits: Vec<u64> = Layer::ALL.iter().map(|&l| bits(census.get(l))).collect();
            (bits, hits)
        };
        let kept = KeepInteriors::new(galaxy);
        assert_order_independent(&spheres, |sphere| answer(&kept, sphere));
        assert_order_independent(&spheres, |sphere| {
            answer(&KeepInteriors::new(galaxy), sphere)
        });
    }

    /// A feature-cell cache that keeps nothing and records every cell asked for, with the last
    /// process it is asked through.
    #[derive(Default)]
    struct Recording(Mutex<Vec<(FeatureCell, FeatureProcess)>>);

    impl FeatureCellCache for Recording {
        fn contents(
            &self,
            galaxy: &Galaxy,
            cell: FeatureCell,
        ) -> Arc<crate::galaxy::features::catalogue::FeatureCellContents> {
            self.0.lock().unwrap().push((cell, FeatureProcess::Cloud));
            Arc::new(FeatureCatalogue::cell(galaxy, cell))
        }

        fn contents_through(
            &self,
            galaxy: &Galaxy,
            cell: FeatureCell,
            last: FeatureProcess,
        ) -> Arc<crate::galaxy::features::catalogue::FeatureCellContents> {
            self.0.lock().unwrap().push((cell, last));
            Arc::new(FeatureCatalogue::cell_through(galaxy, cell, last))
        }
    }

    /// Run on its one build of the fixture by
    /// [`the_member_source_finds_and_counts_what_a_scan_finds`]: every feature cell a query's
    /// census and hits look up is among its [`FeatureMemberSource::feature_cells`], through the
    /// same or a later process, and reading those ahead, in either order, changes neither the
    /// census, bit for bit, nor the hits (P09.T40.a). Away from the epoch, so that the hits' lookup
    /// is padded past the census's.
    fn the_feature_cells_cover_every_lookup_and_reading_them_ahead_changes_nothing() {
        let galaxy = galaxy();
        let [x, y, z] = open_cluster().feature().position().to_light_years_f64();
        let centre = GalacticPosition::from_light_years([x + 1.5, y, z - 1.0]).unwrap();
        let query = RangeQuery::builder(centre, LightYears::new(12.0))
            .time(UniverseTime::from_julian_years(400).unwrap())
            .build()
            .unwrap();
        let sphere = widest_sphere(galaxy, &query);
        let answer = |source: &FeatureMemberSource<'_>| {
            let census = source.expected_in_sphere(galaxy, &sphere);
            let mut hits = Vec::new();
            source.systems_in_sphere(galaxy, &sphere, all_layers(), &mut hits);
            let bits: Vec<u64> = Layer::ALL.iter().map(|&l| bits(census.get(l))).collect();
            (bits, hits)
        };
        let expected = answer(&FeatureMemberSource::new(&NoFeatureCache, &NoInteriorCache));
        assert!(
            !expected.1.is_empty(),
            "the query finds the cluster's members"
        );
        let cells = FeatureMemberSource::feature_cells(galaxy, &query);
        let recording = Recording::default();
        assert_eq!(
            answer(&FeatureMemberSource::new(&recording, &NoInteriorCache)),
            expected
        );
        let asked = recording.0.into_inner().unwrap();
        assert_ne!(
            asked.len(),
            0,
            "the census and the hits look up feature cells"
        );
        for (cell, last) in asked {
            assert!(
                cells
                    .iter()
                    .any(|&(c, l)| c == cell && l.index() >= last.index()),
                "{:?} through {last} is looked up, but not read ahead",
                cell.to_array()
            );
        }
        for order in [cells.clone(), cells.iter().rev().copied().collect()] {
            let (keep, interiors) = (Keep::default(), KeepInteriors::new(galaxy));
            let source = FeatureMemberSource::new(&keep, &interiors);
            for &cell in &order {
                source.read_ahead(galaxy, &query, cell);
            }
            assert_eq!(answer(&source), expected);
        }
    }

    /// Run on its one build of the fixture by
    /// [`the_member_source_finds_and_counts_what_a_scan_finds`].
    fn drawing_only_the_near_members_keeps_every_near_member() {
        let galaxy = galaxy();
        let interior = open_cluster();
        let near = PointLy::new(1.5, -0.5, 1.0);
        let within = 3.0;
        let (mut all, mut kept) = (Vec::new(), Vec::new());
        let mut found = 0;
        for band in MassBand::ALL {
            for cell in interior
                .grid()
                .cells_touching(&near, LightYears::new(within))
            {
                interior.members_in_cell(galaxy, band, cell, &mut all);
                let reach = LightYears::new(within);
                interior.members_in_cell_near(
                    galaxy,
                    band,
                    cell,
                    &near,
                    reach,
                    (&mut kept, &mut interior.bound_memo()),
                );
                all.retain(|(_, p)| {
                    let (dx, dy, dz) = (p.x - near.x, p.y - near.y, p.z - near.z);
                    (dx * dx + dy * dy + dz * dz).sqrt() <= within
                });
                assert_eq!(kept, all, "band {band:?}, cell {cell:?}");
                found += kept.len();
            }
        }
        assert!(found > 5, "{found} members within {within} ly");
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
            pad_for(t, crate::galaxy::query::pad_speed(Layer::E)),
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
        let galaxy = galaxy();
        let radius = LightYears::new(12.0);
        for years in [-1_000, -600, 1, 1_000] {
            let t = UniverseTime::from_julian_years(years).unwrap();
            for layer in [Layer::A, Layer::D, Layer::E] {
                let pad = pad_speed_in(galaxy, layer);
                let today = radius + pad_for(t, pad);
                for below in [0.0, 164.0, 0.5 * pad.value(), pad.value()] {
                    let below = KilometresPerSecond::new(below);
                    let reach = band_reach(galaxy, radius, t, layer, below);
                    assert_same_bits(reach.value(), today.value());
                }
                for above in [pad.value() * (1.0 + 1e-9), 1.2 * pad.value(), 4_000.0] {
                    let bound = KilometresPerSecond::new(above);
                    let reach = band_reach(galaxy, radius, t, layer, bound);
                    assert_same_bits(reach.value(), (radius + pad_for(t, bound)).value());
                    assert!(reach.value() > today.value(), "{layer:?} at {years} years");
                }
            }
        }
        // At the epoch nothing has moved, however fast.
        let epoch = band_reach(
            galaxy,
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
