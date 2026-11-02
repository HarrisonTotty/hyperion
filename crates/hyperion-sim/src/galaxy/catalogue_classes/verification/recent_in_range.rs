//! P09.T37's merge of the recently dead (P09.T36): every recent entry of a catalogue feature's list
//! and of the galactic centre's whose present position lies in a range query's sphere is returned
//! exactly once, by ID, and none outside it, under every 26 hold and every 27 hold.
//!
//! An entry moves on one straight line, its member's: through its epoch position at its velocity,
//! its feature's bulk motion or, at the centre, its tracer's draw, on which its site lies at its
//! death (P09.T36's lean). So its present position at a sphere's time is that line's, before its
//! death and after it.
//!
//! The sources are those the server registers beside the grid (`compute::range::beside_the_grid`):
//! the features' members, and from 27 the catalogue classes and the global list, which holds the
//! centre's source. Below 27 no source places the centre (`LIST_SOURCE_VERSION`), so the centre's
//! entries are asked under every 27 hold alone.
//!
//! Each entry returned is also placed by its ID, as the server places a range row's system (plan
//! 10's P10.T10.b): a feature's on its member's line, to the rounding plan 03's drift and the
//! line's differ by, and a centre entry on its `CentrePath`, bit for bit, both at its velocity.
//!
//! Each sphere's entries are found by brute force: every feature whose reach comes within 100 ly of
//! the sphere, far beyond any feature's motion over the clock window, and its whole list.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use hyperion_testkit::float::bits;
use hyperion_testkit::order::assert_order_independent;

use super::{KeepCells, KeepClassCells, Tubes, milky_way};
use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::ages::SubDisc;
use crate::galaxy::catalogue_classes::CatalogueClassSource;
use crate::galaxy::features::catalogue::{FeatureCatalogue, FeatureRecord, NoFeatureCache};
use crate::galaxy::features::centre::events::victims::victim_count;
use crate::galaxy::features::centre::members::resolve_centre_member_with;
use crate::galaxy::features::centre::recent::CentreRecent;
use crate::galaxy::features::centre::{CentreMemberSource, CentrePath, KeepCentreOrbits};
use crate::galaxy::features::ids::{FeatureKind, FeatureProcess};
use crate::galaxy::features::members::level_list::registered_classes;
use crate::galaxy::features::members::recent::{FeatureClassesRules, RecentClass, moved};
use crate::galaxy::features::members::{
    FeatureLevelList, FeatureMemberSource, KeepInteriors, resolve_member,
};
use crate::galaxy::global_list::GlobalListSource;
use crate::galaxy::motion::StateAt;
use crate::galaxy::placement::{Existence, SystemRecord};
use crate::galaxy::query::{
    LayerCounts, LayerSet, MassFloor, QuerySphere, RangePlan, RangeQuery, SystemSource, range_query,
};
use crate::galaxy::rules::GalaxyRules;
use crate::id::{Layer, MemberSlot, SystemId, SystemIdKind};
use crate::observe::{Drift, Trajectory};
use crate::planetary::testing::{with_every_26_hold_with_its_retargets, with_every_27_hold};
use crate::time::UniverseTime;
use crate::units::{KilometresPerSecond, LightYears, Years};

/// The spheres' times, years from the epoch: the clock window's ends and the epoch.
const TIMES_YR: [i64; 3] = [-1_000, 0, 1_000];

/// How far beyond a sphere the brute force looks for features, light-years: some 30,000 km/s over
/// the clock window, far above any feature's motion.
const SCAN_MARGIN_LY: f64 = 100.0;

/// The radius of a sphere about one entry, light-years.
const ABOUT_AN_ENTRY_LY: f64 = 0.5;

/// The entries of each feature about which a sphere is asked.
const ENTRY_SPHERES: usize = 4;

/// The nursery features walked for one with entries, of each kind.
const NURSERIES_WALKED: usize = 4_000;

/// The entries a feature's list holds for the walk to stop at it.
const MANY_ENTRIES: usize = 3;

/// The old open clusters walked for one with entries.
const OLD_CLUSTERS_WALKED: usize = 2_000;

/// One recent entry as the brute force sees it: its record and its line.
#[derive(Debug, Clone, Copy)]
struct Listed {
    record: SystemRecord,
    velocity: GalacticVelocity,
}

impl Listed {
    /// Where it is at `t`: its epoch position moved straight along its velocity.
    fn at(&self, t: UniverseTime) -> GalacticPosition {
        if t == UniverseTime::EPOCH {
            *self.record.epoch_position()
        } else {
            moved(self.record.epoch_position(), self.velocity, t)
        }
    }

    /// Whether it exists at `t` and lies inside the sphere of `radius` about `centre` then.
    fn inside(&self, centre: &GalacticPosition, radius: LightYears, t: UniverseTime) -> bool {
        self.record.existence_at(t) == Existence::Exists
            && LightYears::from(centre.distance_to(&self.at(t))).value() <= radius.value()
    }
}

/// `feature`'s recent entries in `galaxy`, by ID.
fn feature_entries(galaxy: &Galaxy, feature: &FeatureRecord) -> BTreeMap<SystemId, Listed> {
    FeatureLevelList::of(galaxy, feature)
        .recent_entries(galaxy)
        .map(|entry| {
            let member = entry.member();
            (
                member.record().id(),
                Listed {
                    record: *member.record(),
                    velocity: member.velocity(),
                },
            )
        })
        .collect()
}

/// The centre's recent entries in `galaxy`, by ID: none where it lists no class.
fn centre_entries(galaxy: &Galaxy) -> BTreeMap<SystemId, Listed> {
    let model = galaxy.centre().expect("the Milky Way's centre builds");
    let victims = victim_count(galaxy).expect("the Milky Way's black hole has its disruptions");
    let recent = CentreRecent::new(galaxy, model, victims);
    recent
        .list(galaxy)
        .entries()
        .filter_map(|entry| {
            let class = RecentClass::of(entry.class())?;
            let kept = recent.entry(galaxy, model, class, entry.index())?;
            let member = kept.member();
            Some((
                member.record().id(),
                Listed {
                    record: *member.record(),
                    velocity: member.velocity(),
                },
            ))
        })
        .collect()
}

/// Whether `id` names an entry of a recent class on a list, a catalogue feature's or the centre's:
/// a feature-level index past the centre's black hole and its victims, of a recent class.
fn is_recent_slot(galaxy: &Galaxy, id: SystemId) -> bool {
    match id.kind() {
        SystemIdKind::FeatureMember(member) => {
            let MemberSlot::FeatureLevel { index } = member.slot() else {
                return false;
            };
            FeatureCatalogue::resolve(galaxy, member.feature().into()).is_some_and(|feature| {
                FeatureLevelList::of(galaxy, &feature)
                    .entry(index)
                    .is_some_and(|entry| RecentClass::of(entry.class()).is_some())
            })
        }
        SystemIdKind::Centre(member) => {
            let MemberSlot::FeatureLevel { index } = member.slot() else {
                return false;
            };
            let model = galaxy.centre().expect("the Milky Way's centre builds");
            let victims = victim_count(galaxy).expect("the disruptions build");
            CentreRecent::new(galaxy, model, victims)
                .list(galaxy)
                .entry(index)
                .is_some_and(|entry| RecentClass::of(entry.class()).is_some())
        }
        SystemIdKind::Grid(_)
        | SystemIdKind::Stream(_)
        | SystemIdKind::DwarfCore(_)
        | SystemIdKind::Pinned(_)
        | SystemIdKind::Catalogue(_) => false,
    }
}

/// What a placement by ID reads, as the server's `convert::system_motion` reads it: the features'
/// interiors and the centre's orbits, the ones the sources read.
#[derive(Clone, Copy)]
struct Reads<'a> {
    interiors: &'a KeepInteriors<'a>,
    orbits: &'a KeepCentreOrbits<'a>,
}

/// Where the system `record` is at `t` placed by its ID, as the server places a range row's
/// system (plan 10's P10.T10.b): a feature's member on its line (`Drift::of_record_in`), a centre
/// member on its path (`CentrePath`), with its velocity there; `None` for a system it does not
/// place then.
fn placed_by_id(
    galaxy: &Galaxy,
    reads: Reads<'_>,
    record: &SystemRecord,
    t: UniverseTime,
) -> Option<(GalacticPosition, GalacticVelocity)> {
    match record.id().kind() {
        SystemIdKind::FeatureMember(_) => {
            let line = Drift::of_record_in(galaxy, reads.interiors, record).ok()?;
            Some((line.position_at(t), line.velocity()))
        }
        SystemIdKind::Centre(id) => {
            match CentrePath::of(galaxy, reads.orbits, id).ok()?.state_at(t) {
                StateAt::Exists(state) => Some((*state.position(), state.velocity())),
                StateAt::NoSuchSystem => None,
            }
        }
        SystemIdKind::Grid(_)
        | SystemIdKind::Stream(_)
        | SystemIdKind::DwarfCore(_)
        | SystemIdKind::Pinned(_)
        | SystemIdKind::Catalogue(_) => None,
    }
}

/// How far a feature member's placement by ID may lie from its range row, m: plan 03 rounds its
/// row's drift and `Drift`'s line differently, by up to a few metres (the server's
/// `SystemLine::at`).
const LINE_ROUNDING_M: f64 = 10.0;

/// The sources the server registers in `galaxy` beside the grid, handed to `run` with what a
/// placement by ID reads.
fn with_served<R>(galaxy: &Galaxy, run: impl FnOnce(&[&dyn SystemSource], Reads<'_>) -> R) -> R {
    let interiors = KeepInteriors::new(galaxy);
    let features = FeatureMemberSource::new(&NoFeatureCache, &interiors);
    let cells = KeepClassCells::default();
    let classes =
        CatalogueClassSource::registered_in(galaxy).map(|classes| classes.with_cells(&cells));
    let orbits = KeepCentreOrbits::new(galaxy);
    let tubes = Tubes::new(galaxy);
    let listed = GlobalListSource::is_registered_in(galaxy).then(|| {
        GlobalListSource::new(galaxy, &tubes, &orbits).expect("the Milky Way's centre builds")
    });
    let mut sources: Vec<&dyn SystemSource> = vec![&features];
    if let Some(listed) = &listed {
        sources.push(listed);
    }
    if let Some(classes) = &classes {
        sources.push(classes);
    }
    let reads = Reads {
        interiors: &interiors,
        orbits: &orbits,
    };
    run(&sources, reads)
}

/// A query of the sphere of `radius` about `centre` at `t`, down to layer D, where every recent
/// entry's primary lies (a core collapse's and a variable's in E, a Type Ia's in D).
fn query(centre: &GalacticPosition, radius: LightYears, t: UniverseTime) -> RangeQuery {
    RangeQuery::builder(*centre, radius)
        .time(t)
        .mass_floor(MassFloor::LayerD)
        .limit(NonZeroU32::new(1 << 20).expect("non-zero"))
        .build()
        .expect("a sphere in the cube within the clock window")
}

/// What one sphere found wrong: each entry inside returned other than once, each outside returned,
/// each hit of a recent slot that names no such entry, and each returned entry that its placement
/// by ID ([`placed_by_id`]) puts elsewhere or moving otherwise, as text.
fn check_sphere(
    (galaxy, cells, reads): (&Galaxy, &mut KeepCells, Reads<'_>),
    sources: &[&dyn SystemSource],
    listed: &BTreeMap<SystemId, Listed>,
    (centre, radius, t): (&GalacticPosition, LightYears, UniverseTime),
) -> (usize, Vec<String>) {
    let q = query(centre, radius, t);
    let result = range_query(galaxy, cells, sources, &q);
    assert!(
        result.census().layers().contains(Layer::D),
        "{radius:?} at {t:?}: the census admits layers D and E"
    );
    let mut seen: BTreeMap<SystemId, usize> = BTreeMap::new();
    let mut faults = Vec::new();
    for hit in result.systems() {
        if let Some(entry) = listed.get(&hit.id()) {
            *seen.entry(hit.id()).or_default() += 1;
            if hit.record() != &entry.record || hit.position() != &entry.at(t) {
                faults.push(format!(
                    "{:?}: returned at {:?} as {:?}, not as listed",
                    hit.id(),
                    hit.position(),
                    hit.record()
                ));
            }
            let placed = placed_by_id(galaxy, reads, hit.record(), t);
            let agrees = placed.is_some_and(|(position, velocity)| {
                let off = hit.position().distance_to(&position).value();
                velocity == entry.velocity
                    && match hit.id().kind() {
                        SystemIdKind::Centre(_) => position == *hit.position(),
                        _ => off <= LINE_ROUNDING_M,
                    }
            });
            if !agrees {
                faults.push(format!(
                    "{:?}: returned at {:?}, placed by its ID at {placed:?}",
                    hit.id(),
                    hit.position()
                ));
            }
        } else if is_recent_slot(galaxy, hit.id()) {
            faults.push(format!("{:?}: a recent slot no list keeps", hit.id()));
        }
    }
    let mut inside = 0;
    for (id, entry) in listed {
        let returned = seen.get(id).copied().unwrap_or(0);
        let want = usize::from(entry.inside(centre, radius, t));
        inside += want;
        if returned != want {
            faults.push(format!(
                "{id:?} ({:?}, {:.3} ly from the centre at {t:?}): returned {returned} times, \
                 inside {}",
                entry.record.layer(),
                LightYears::from(centre.distance_to(&entry.at(t))).value(),
                want == 1
            ));
        }
    }
    (inside, faults)
}

/// The entries of every feature whose reach comes within [`SCAN_MARGIN_LY`] of the sphere of
/// `radius` about `centre`, by ID.
fn entries_near(
    galaxy: &Galaxy,
    centre: &GalacticPosition,
    radius: LightYears,
) -> BTreeMap<SystemId, Listed> {
    FeatureCatalogue::near(
        galaxy,
        centre,
        LightYears::new(radius.value() + SCAN_MARGIN_LY),
        &NoFeatureCache,
    )
    .flat_map(|feature| feature_entries(galaxy, &feature))
    .collect()
}

/// The features asked in `galaxy`: of each kind of nursery, a star-forming region, a young bound
/// cluster and an association, and of the youngest sub-disc's old open clusters, the first of
/// those walked that lists [`MANY_ENTRIES`] entries, or else the one that lists the most, if any
/// lists one.
fn features_asked(galaxy: &Galaxy) -> Vec<FeatureRecord> {
    let pick = |walked: &mut dyn Iterator<Item = FeatureRecord>| {
        let mut best: Option<(usize, FeatureRecord)> = None;
        for feature in walked {
            let n = feature_entries(galaxy, &feature).len();
            if n >= MANY_ENTRIES {
                return Some(feature);
            }
            if n > best.as_ref().map_or(0, |b| b.0) {
                best = Some((n, feature));
            }
        }
        best.map(|(_, feature)| feature)
    };
    let mut asked = Vec::new();
    for kind in [
        FeatureKind::StarFormingRegion,
        FeatureKind::OpenCluster,
        FeatureKind::Association,
    ] {
        asked.extend(pick(
            &mut FeatureCatalogue::walk_process(galaxy, FeatureProcess::Nursery)
                .take(NURSERIES_WALKED)
                .filter(|f| f.kind_at(Years::ZERO) == Some(kind)),
        ));
    }
    asked.extend(pick(
        &mut FeatureCatalogue::walk_process(galaxy, FeatureProcess::OldOpenCluster(SubDisc::First))
            .take(OLD_CLUSTERS_WALKED),
    ));
    asked
}

/// The spheres about `centre` and the entries of `own` at `t`: one holding every entry, one of
/// half the median entry's distance, which leaves at least half of them outside, and one of
/// [`ABOUT_AN_ENTRY_LY`] about each of the first [`ENTRY_SPHERES`].
fn spheres_about(
    centre: &GalacticPosition,
    own: &BTreeMap<SystemId, Listed>,
    t: UniverseTime,
) -> Vec<(GalacticPosition, LightYears)> {
    let mut distances: Vec<f64> = own
        .values()
        .map(|entry| LightYears::from(centre.distance_to(&entry.at(t))).value())
        .collect();
    distances.sort_by(f64::total_cmp);
    let farthest = distances.last().copied().unwrap_or(1.0);
    let median = distances.get(distances.len() / 2).copied().unwrap_or(1.0);
    let mut spheres = vec![
        (*centre, LightYears::new(farthest + 1.0)),
        (
            *centre,
            LightYears::new((0.5 * median).max(ABOUT_AN_ENTRY_LY)),
        ),
    ];
    spheres.extend(
        own.values()
            .take(ENTRY_SPHERES)
            .map(|entry| (entry.at(t), LightYears::new(ABOUT_AN_ENTRY_LY))),
    );
    spheres
}

/// The features' half of the test in `galaxy`, about the features `asked`: the entries inside the
/// spheres, and the faults.
fn features_in_range(
    (galaxy, reads): (&Galaxy, Reads<'_>),
    sources: &[&dyn SystemSource],
    asked: &[FeatureRecord],
) -> (usize, Vec<String>) {
    assert!(asked.len() >= 3, "the nurseries of each kind list entries");
    let (mut inside, mut faults) = (0, Vec::new());
    let mut cells = KeepCells::default();
    for feature in asked {
        let own = feature_entries(galaxy, feature);
        eprintln!(
            "{:?}, {:?}: {} recent entries",
            feature.id(),
            feature.kind_at(Years::ZERO),
            own.len()
        );
        // The census holds the whole list of a feature inside the sphere.
        let whole = QuerySphere::new(
            *feature.position(),
            LightYears::new(feature.reach().value() + 1.0),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .expect("a positive radius");
        let census = sources[0].expected_in_sphere(galaxy, &whole);
        let mut listed = LayerCounts::ZERO;
        for class in registered_classes(galaxy, feature) {
            if let Some(recent) = RecentClass::of(class.class()) {
                let layer = recent.layer();
                listed.set(layer, listed.get(layer) + class.expected());
            }
        }
        for layer in [Layer::D, Layer::E] {
            assert!(
                census.get(layer) >= listed.get(layer),
                "{:?}, {layer:?}: the census {census:?} holds the list's {listed:?}",
                feature.id()
            );
        }
        for years in TIMES_YR {
            let t = UniverseTime::from_julian_years(years).expect("in the clock window");
            for (centre, radius) in spheres_about(feature.position(), &own, t) {
                let listed = entries_near(galaxy, &centre, radius);
                let (n, mut found) = check_sphere(
                    (galaxy, &mut cells, reads),
                    sources,
                    &listed,
                    (&centre, radius, t),
                );
                inside += n;
                faults.append(&mut found);
            }
        }
    }
    (inside, faults)
}

/// The radius about `centre` at `t`, from `start` halved, at which the census admits layer D in
/// `galaxy` with `sources`.
fn admitting_radius(
    galaxy: &Galaxy,
    sources: &[&dyn SystemSource],
    centre: &GalacticPosition,
    start: f64,
    t: UniverseTime,
) -> LightYears {
    let mut radius = start;
    for _ in 0..24 {
        let plan = RangePlan::new(galaxy, sources, &query(centre, LightYears::new(radius), t));
        if plan.census().layers().contains(Layer::D) {
            return LightYears::new(radius);
        }
        radius *= 0.5;
    }
    panic!("no radius about {centre:?} under {start} ly admits layer D");
}

/// The centre's half of the test in `galaxy`: about the black hole and each entry at the epoch,
/// and about the black hole and the first [`ENTRY_SPHERES`] entries at the clock window's ends.
fn centre_in_range(
    (galaxy, reads): (&Galaxy, Reads<'_>),
    sources: &[&dyn SystemSource],
) -> (usize, Vec<String>) {
    let listed = centre_entries(galaxy);
    eprintln!("the centre: {} recent entries", listed.len());
    assert!(
        !listed.is_empty(),
        "the Milky Way's centre lists its recent dead"
    );
    let model = galaxy.centre().expect("the Milky Way's centre builds");
    for (id, entry) in &listed {
        eprintln!(
            "  {id:?}: {:?}, {:.4} ly from the black hole, {:.0} km/s",
            entry.record.layer(),
            LightYears::from(GalacticPosition::ORIGIN.distance_to(entry.record.epoch_position()))
                .value(),
            KilometresPerSecond::from(entry.velocity.speed()).value()
        );
        let SystemIdKind::Centre(member) = id.kind() else {
            panic!("{id:?}: a centre entry carries a centre ID");
        };
        assert_eq!(
            resolve_centre_member_with(galaxy, model, member).map(|m| *m.record()),
            Ok(entry.record),
            "{id:?} resolves to its entry"
        );
    }
    let (mut inside, mut faults) = (0, Vec::new());
    let mut cells = KeepCells::default();
    for years in TIMES_YR {
        let t = UniverseTime::from_julian_years(years).expect("in the clock window");
        let mut spheres = vec![GalacticPosition::ORIGIN];
        let about = if years == 0 {
            listed.len()
        } else {
            ENTRY_SPHERES
        };
        spheres.extend(listed.values().take(about).map(|entry| entry.at(t)));
        for centre in spheres {
            let radius = admitting_radius(galaxy, sources, &centre, 1.0, t);
            let (n, mut found) = check_sphere(
                (galaxy, &mut cells, reads),
                sources,
                &listed,
                (&centre, radius, t),
            );
            inside += n;
            faults.append(&mut found);
        }
    }
    (inside, faults)
}

/// The centre's source answers the same whatever it has built (its list and its places are built
/// when a sphere first needs them): the census and the hits of spheres about the black hole and
/// about three entries, at the epoch and at +H, in every order through one source, warmed, and
/// through a source made afresh for each, cold.
fn the_centre_s_source_is_order_independent(
    (galaxy, reads): (&Galaxy, Reads<'_>),
    listed: &BTreeMap<SystemId, Listed>,
) {
    let mut keys = Vec::new();
    for years in [0, 1_000] {
        let t = UniverseTime::from_julian_years(years).expect("in the clock window");
        keys.push((GalacticPosition::ORIGIN, LightYears::new(0.05), t));
        keys.extend(
            listed
                .values()
                .take(3)
                .map(|entry| (entry.at(t), LightYears::new(ABOUT_AN_ENTRY_LY), t)),
        );
    }
    let stellar = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .fold(LayerSet::EMPTY, LayerSet::with);
    let ask = |source: &CentreMemberSource<'_>,
               &(centre, radius, t): &(GalacticPosition, LightYears, UniverseTime)| {
        let sphere =
            QuerySphere::new(centre, radius, t, LightYears::ZERO).expect("a positive radius");
        let mut hits = Vec::new();
        source.systems_in_sphere(galaxy, &sphere, stellar, &mut hits);
        let census = source
            .expected_in_sphere(galaxy, &sphere)
            .to_array()
            .map(bits);
        (census, hits)
    };
    let warmed = CentreMemberSource::new(galaxy, reads.orbits).expect("the centre builds");
    assert_order_independent(&keys, |key| ask(&warmed, key));
    assert_order_independent(&keys, |key| {
        ask(
            &CentreMemberSource::new(galaxy, reads.orbits).expect("the centre builds"),
            key,
        )
    });
    for key in &keys {
        let cold = CentreMemberSource::new(galaxy, reads.orbits).expect("the centre builds");
        assert_eq!(ask(&cold, key), ask(&warmed, key), "{key:?}");
    }
}

/// Prints the entries inside the spheres and the faults, the first twenty shown.
fn report(what: &str, inside: usize, faults: &[String]) {
    eprintln!(
        "{what}: {inside} entries inside the spheres, {} faults",
        faults.len()
    );
    for fault in faults.iter().take(20) {
        eprintln!("  {fault}");
    }
}

/// Fails with the faults, the first twenty shown, if there are any.
fn assert_no_faults(what: &str, inside: usize, faults: &[String]) {
    report(what, inside, faults);
    assert!(inside > 0, "{what}: some entry lies inside a sphere");
    assert!(faults.is_empty(), "{what}: {} faults", faults.len());
}

/// Every 26 hold: the features' recent entries, through the features' members, the one source
/// registered beside the grid at 26 (module documentation). Their IDs resolve through
/// `resolve_member`. Should the features' lists be held to 27 (`FEATURE_CLASSES_VERSION`), no
/// feature lists an entry at 26, and none is lost.
#[test]
#[ignore = "slow: range queries about four features' lists at three times under every 26 hold"]
fn every_recent_entry_in_a_sphere_is_returned_once_under_every_26_hold() {
    with_every_26_hold_with_its_retargets(|| {
        let galaxy = milky_way(GalaxyRules::every_26_hold_with_its_retargets());
        let asked = features_asked(&galaxy);
        if galaxy.rules().feature_classes == FeatureClassesRules::Unlisted {
            assert_eq!(asked, Vec::new(), "no feature lists a recent class");
            return;
        }
        with_served(&galaxy, |sources, reads| {
            assert_eq!(sources.len(), 1, "at 26 the features' members alone");
            let (inside, faults) = features_in_range((&galaxy, reads), sources, &asked);
            assert_no_faults("the features under every 26 hold", inside, &faults);
        });
        let interiors = KeepInteriors::new(&galaxy);
        for feature in &asked {
            for (id, entry) in feature_entries(&galaxy, feature) {
                let SystemIdKind::FeatureMember(member) = id.kind() else {
                    panic!("{id:?}: a feature's entry carries a member ID");
                };
                assert_eq!(
                    resolve_member(&galaxy, &interiors, member).map(|m| *m.record()),
                    Ok(entry.record)
                );
            }
        }
    });
}

/// Every 27 hold: the features' recent entries and the centre's, through every source the server
/// registers (module documentation), each placed by its ID where it was returned; and the
/// centre's source answers the same whatever it has built.
#[test]
#[ignore = "slow: range queries about four features' lists and the centre's at three times under \
            every 27 hold"]
fn every_recent_entry_in_a_sphere_is_returned_once_under_every_27_hold() {
    with_every_27_hold(|| {
        let galaxy = milky_way(GalaxyRules::every_27_hold());
        with_served(&galaxy, |sources, reads| {
            assert_eq!(
                sources.len(),
                3,
                "at 27 the members, the list and the classes"
            );
            let features = features_in_range((&galaxy, reads), sources, &features_asked(&galaxy));
            let centre = centre_in_range((&galaxy, reads), sources);
            the_centre_s_source_is_order_independent((&galaxy, reads), &centre_entries(&galaxy));
            report("the centre under every 27 hold", centre.0, &centre.1);
            assert_no_faults("the features under every 27 hold", features.0, &features.1);
            assert_no_faults("the centre under every 27 hold", centre.0, &centre.1);
        });
    });
}
