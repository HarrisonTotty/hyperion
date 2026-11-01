//! The global list source's unit tests (P10.T9): the three test streams of `global_list::testing`
//! and a small dwarf core on a list of their own ([`GlobalList::of_parts`]), the streams' tubes
//! reduced from 64 tracers an arm, so that the sim's fast suite runs them on `wasm32-wasip1` too.
//! The brute-force comparisons, at the generator's 2,000 tracers, are slow and ignored.

use std::num::NonZeroU32;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use super::*;
use crate::coords::{GalacticDisplacement, GalacticVelocity};
use crate::galaxy::Population;
use crate::galaxy::features::centre::NoCentreOrbitCache;
use crate::galaxy::global_list::spec::{DwarfCoreSpec, SphericalOrbit, StreamSpec};
use crate::galaxy::global_list::spray::{Spray, SprayForce, SprayParams};
use crate::galaxy::global_list::testing::{gd1_like, pal5_like, sagittarius_like};
use crate::galaxy::global_list::{DwarfCoreNumber, GlobalEntry};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{NoCache, SystemOrigin};
use crate::galaxy::potential::{MassModel, PotentialTables};
use crate::galaxy::query::{RangeQuery, range_query};
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::rng::Seed;
use crate::units::SolarMasses;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{LightYears, Years};

/// The galaxies' seed, the tubes' gap noise's and the sprays' too.
const SEED: Seed = Seed::new(0x0a10_9a00_0000_0000);

/// The stellar layers, every band a member can be in.
fn stellar() -> LayerSet {
    [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .fold(LayerSet::EMPTY, LayerSet::with)
}

/// The Milky Way fixture's full tables, shared.
fn tables() -> &'static PotentialTables {
    static TABLES: OnceLock<PotentialTables> = OnceLock::new();
    TABLES.get_or_init(|| PotentialTables::full(&MassModel::new(&GalaxyParams::milky_way_like())))
}

/// The fixture galaxy under the version's rules and under every 26 hold.
fn galaxy(at_26: bool) -> &'static Galaxy {
    static HELD: OnceLock<Galaxy> = OnceLock::new();
    static AT_26: OnceLock<Galaxy> = OnceLock::new();
    let build = |rules| {
        Galaxy::from_params_with_rules(SEED, GalaxyParams::milky_way_like(), rules)
            .expect("the Milky Way fixture builds")
    };
    if at_26 {
        AT_26.get_or_init(|| build(GalaxyRules::every_26_hold()))
    } else {
        HELD.get_or_init(|| build(GalaxyRules::of_version(crate::GENERATOR_VERSION)))
    }
}

/// The three test streams, numbered 0–2.
fn streams() -> Vec<StreamSpec> {
    let tables = tables();
    vec![
        pal5_like(tables),
        gd1_like(tables),
        sagittarius_like(tables),
    ]
}

/// A small dwarf core, number 0: 2,000 M☉ still bound of 6,000 at an accretion 3 Gyr ago, at
/// (30,000, 20,000, 5,000) ly on an orbit between 30,000 and 120,000 ly.
fn core() -> DwarfCoreSpec {
    let galaxy = galaxy(false);
    let orbit = SphericalOrbit::between(
        tables(),
        LightYears::new(30_000.0),
        LightYears::new(120_000.0),
    )
    .expect("turning points that bound an orbit");
    let mass = 2e3;
    DwarfCoreSpec {
        number: DwarfCoreNumber::new(0).expect("below the limit"),
        progenitor: 2,
        mass: SolarMasses::new(mass),
        systems: mass / galaxy.mean_system_mass(Population::Halo).value(),
        accreted: Years::new(3e9),
        accreted_mass: SolarMasses::new(3.0 * mass),
        position: GalacticDisplacement::new(
            [30_000.0, 20_000.0, 5_000.0].map(|x| x * METRES_PER_LIGHT_YEAR),
        ),
        velocity: GalacticVelocity::new([1.2e5, -8e4, 3e4]),
        orbit,
        stripping_fraction: 0.3,
        pericentres: 4,
    }
}

/// The tube of `spec` reduced from `per_arm` tracers an arm.
fn tube_of(spec: &StreamSpec, per_arm: u32) -> TubeTable {
    let tables = tables();
    let force = SprayForce::new(tables).expect("the full tables");
    let params = SprayParams::new(NonZeroU32::new(per_arm).expect("not zero"));
    let tracers = Spray::new(SEED, &force, tables, spec, &params)
        .expect("a finite stripping time")
        .tracers();
    let mean = galaxy(false).mean_system_mass(Population::Halo);
    TubeTable::from_tracers(SEED, mean, spec, &tracers).expect("enough tracers an arm")
}

/// A lookup of a list and its streams' tables, counting the tables it hands out; if `rebuild`, it
/// makes each table afresh on every call, as a cache that keeps none would.
struct Lookup {
    list: GlobalList,
    tubes: Vec<Arc<TubeTable>>,
    per_arm: u32,
    rebuild: bool,
    handed: AtomicUsize,
}

impl Lookup {
    fn new(list: GlobalList, per_arm: u32, rebuild: bool) -> Self {
        let tubes = list
            .debris()
            .streams()
            .iter()
            .map(|spec| Arc::new(tube_of(spec, per_arm)))
            .collect();
        Self {
            list,
            tubes,
            per_arm,
            rebuild,
            handed: AtomicUsize::new(0),
        }
    }

    fn handed(&self) -> usize {
        self.handed.load(Ordering::Relaxed)
    }
}

impl TubeLookup for Lookup {
    fn global_list(&self) -> &GlobalList {
        &self.list
    }

    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
        self.handed.fetch_add(1, Ordering::Relaxed);
        let index = usize::from(stream.get());
        if self.rebuild {
            let spec = &self.list.debris().streams()[index];
            Arc::new(tube_of(spec, self.per_arm))
        } else {
            Arc::clone(&self.tubes[index])
        }
    }
}

/// The fast fixture's lookup: the three test streams' 64-tracer tubes and the small core.
fn lookup() -> &'static Lookup {
    static LOOKUP: OnceLock<Lookup> = OnceLock::new();
    LOOKUP.get_or_init(|| {
        let list = GlobalList::of_parts(galaxy(false), tables(), streams(), vec![core()]);
        Lookup::new(list, 64, false)
    })
}

/// The sphere of `radius` ly about `centre` at `years` from the epoch, padded as the range query
/// pads the sources' sphere.
fn sphere(centre: GalacticPosition, radius: f64, years: i64) -> QuerySphere {
    let t = UniverseTime::from_julian_years(years).expect("in the clock window");
    QuerySphere::new(
        centre,
        LightYears::new(radius),
        t,
        pad_for(t, UNBOUND_PAD_SPEED),
    )
    .expect("a positive radius")
}

/// The source's hits in `sphere`, sorted by ID.
fn hits(source: &GlobalListSource<'_>, galaxy: &Galaxy, sphere: &QuerySphere) -> Vec<SystemHit> {
    let mut out = Vec::new();
    source.systems_in_sphere(galaxy, sphere, stellar(), &mut out);
    out.sort_by_key(|hit| hit.id().raw());
    out
}

/// The hits' raw IDs, in order.
fn ids(hits: &[SystemHit]) -> Vec<u64> {
    hits.iter().map(|hit| hit.id().raw()).collect()
}

/// P10.T9: a query whose sphere meets no stream's shell builds no table, from version 26, and one
/// on a stream's axis builds its stream's alone; below 26 the source visits every stream, since a
/// member may lie outside its shell then. The list here is `pal5_like`'s and `gd1_like`'s, since
/// `sagittarius_like`'s shell holds the whole cube.
#[test]
fn a_query_that_touches_no_shell_builds_no_table() {
    let streams = streams()[..2].to_vec();
    for at_26 in [true, false] {
        let galaxy = galaxy(at_26);
        let list = GlobalList::of_parts(galaxy, tables(), streams.clone(), Vec::new());
        let lookup = Lookup::new(list, 64, false);
        let source = GlobalListSource::new(galaxy, &lookup, &NoCentreOrbitCache)
            .expect("no centre to build");
        // 2,000 ly from the centre, inside both shells' holes.
        let inner = GalacticPosition::from_light_years([2_000.0, 0.0, 0.0]).expect("in range");
        for years in [0, 1_000] {
            let s = sphere(inner, 50.0, years);
            let census = source.expected_in_sphere(galaxy, &s);
            assert_eq!(census, LayerCounts::ZERO, "{at_26} {years}");
            assert_eq!(hits(&source, galaxy, &s), Vec::new(), "{at_26} {years}");
        }
        let expected = if at_26 { 0 } else { 2 * 2 * 2 };
        assert_eq!(lookup.handed(), expected, "{at_26}: tables handed out");
        // A query on pal5's axis, inside its shell and not gd1's: below gd1's inner radius.
        let gd1_inner = lookup.list.entries()[1].shell().inner().value();
        let on_axis = (1..100)
            .map(|k| lookup.tubes[0].locate(f64::from(k) / 100.0, 0.0, 0.0))
            .find(|p| {
                let [x, y, z] = p.to_light_years_f64();
                (x * x + y * y + z * z).sqrt() < gd1_inner - 1_000.0
            })
            .expect("pal5's tube comes within gd1's hole");
        let s = sphere(on_axis, 50.0, 0);
        assert!(!lookup.list.entries()[1].shell().touches(&s));
        let before = lookup.handed();
        let census = source.expected_in_sphere(galaxy, &s);
        assert!(census.get(Layer::A) > 0.0, "{census:?}");
        let handed = lookup.handed() - before;
        assert_eq!(handed, if at_26 { 1 } else { 2 }, "{at_26}");
    }
}

/// P10.T9: the census is the exact sum of the touched cells' means band by band, the same through
/// a lookup that keeps its tables and through one that rebuilds them on every call, and at any
/// time the query is asked at (the census is the epoch's); it is the cells' sum by hand.
#[test]
fn the_census_is_the_cells_sum_and_independent_of_the_cache() {
    let galaxy = galaxy(true);
    let kept = lookup();
    let rebuilt = Lookup::new(kept.list.clone(), 64, true);
    let a = GlobalListSource::new(galaxy, kept, &NoCentreOrbitCache).expect("no centre");
    let b = GlobalListSource::new(galaxy, &rebuilt, &NoCentreOrbitCache).expect("no centre");
    let gd1 = &kept.tubes[1];
    let centre = gd1.locate(0.3, 1.0, -0.5);
    let census = a.expected_in_sphere(galaxy, &sphere(centre, 200.0, 0));
    for years in [0, -500, 3_000] {
        let s = sphere(centre, 200.0, years);
        assert_eq!(a.expected_in_sphere(galaxy, &s), census, "{years}");
        assert_eq!(b.expected_in_sphere(galaxy, &s), census, "{years}");
    }
    assert!(rebuilt.handed() > 0);
    // By hand: the streams whose shells the sphere meets; the core's cells lie far away.
    let shares = |n: usize| StreamBandShares::of(galaxy, &kept.list.debris().streams()[n]);
    let mut by_hand = LayerCounts::ZERO;
    for n in [0, 1, 2] {
        let entry = kept.list.entries()[n];
        if !entry.shell().touches(&sphere(centre, 200.0, 0)) {
            continue;
        }
        let tube = &kept.tubes[n];
        let shares = shares(n);
        let cells: Vec<_> = tube
            .cells_touching(&centre, LightYears::new(200.0))
            .collect();
        for band in BANDS_COARSEST_FIRST {
            let sum = cells
                .iter()
                .fold(0.0, |s, &c| s + expected_members(tube, &shares, c, band));
            by_hand.set(band.layer(), by_hand.get(band.layer()) + sum);
        }
    }
    assert_eq!(census, by_hand);
    assert!(census.get(Layer::A) > 1.0, "{census:?}");
}

/// P10.T9: the hits and the census of each sphere are the same whatever was asked before them,
/// through one source whose band shares fill as the queries reach their streams, and equal a
/// fresh source's (the sim-determinism skill's order check).
#[test]
fn the_source_s_answers_are_order_independent() {
    use hyperion_testkit::order::assert_order_independent;
    let galaxy = galaxy(true);
    let lookup = lookup();
    let core = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache)
        .expect("no centre")
        .core(DwarfCoreNumber::new(0).expect("0"))
        .expect("played")
        .clone();
    let mut members = Vec::new();
    let in_core = core
        .grid()
        .owned_cells()
        .find_map(|cell| {
            core.members_in_cell(galaxy, MassBand::A, cell, &mut members);
            members.first().map(|(m, _)| *m.record().epoch_position())
        })
        .expect("a core with members");
    // About a member of each stream near its axis, from `u` on, so that each sphere holds one
    // whatever the tube's centre line; a stream's members outside the root cube are dropped.
    let in_stream = |n: usize, u: f64| {
        let tube = &lookup.tubes[n];
        let shares = StreamBandShares::of(galaxy, &lookup.list.debris().streams()[n]);
        let mut members = Vec::new();
        (0..20)
            .map(|k| (u + 0.05 * f64::from(k)).fract())
            .find_map(|at| {
                tube.cells_touching(&tube.locate(at, 0.0, 0.0), LightYears::new(100.0))
                    .find_map(|cell| {
                        stream_cell_members(galaxy, tube, &shares, cell, MassBand::A, &mut members);
                        members.first().map(|m| *m.record().epoch_position())
                    })
            })
            .expect("a member near the axis")
    };
    let inner = GalacticPosition::from_light_years([2_000.0, 0.0, 0.0]).expect("in range");
    let spheres = [
        sphere(in_stream(0, 0.45), 300.0, 200),
        sphere(in_stream(1, 0.3), 200.0, 0),
        sphere(in_stream(2, 0.6), 50.0, -1_000),
        sphere(in_core, 30.0, 1_000),
        sphere(inner, 50.0, 0),
    ];
    let answer = |source: &GlobalListSource<'_>, s: &QuerySphere| {
        (
            hits(source, galaxy, s),
            source.expected_in_sphere(galaxy, s),
        )
    };
    let warm = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
    assert_order_independent(&spheres, |s| answer(&warm, s));
    let fresh = |s: &QuerySphere| {
        let cold = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
        answer(&cold, s)
    };
    assert_order_independent(&spheres, fresh);
    for (k, s) in spheres.iter().enumerate() {
        assert_eq!(answer(&warm, s), fresh(s), "sphere {k}");
    }
    let counts: Vec<usize> = spheres
        .iter()
        .map(|s| hits(&warm, galaxy, s).len())
        .collect();
    assert!(counts[..4].iter().all(|&n| n > 0), "{counts:?}");
}

/// P10.T9: the hits of a query about the small core's members are exactly the members a scan of
/// every owned cell finds in the sphere at its time, at the epoch and a millennium either way;
/// every core member lies inside its entry's shell; and the census counts the cells it touches.
#[test]
fn a_core_s_hits_are_every_member_inside_the_sphere() {
    let galaxy = galaxy(true);
    let lookup = lookup();
    let source = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
    let model = source
        .core(DwarfCoreNumber::new(0).expect("0"))
        .expect("played");
    let shell = lookup.list.entries()[3].shell();
    let mut all = Vec::new();
    let mut cell_members = Vec::new();
    for band in BANDS_COARSEST_FIRST {
        for cell in model.grid().owned_cells() {
            model.members_in_cell(galaxy, band, cell, &mut cell_members);
            all.extend(cell_members.iter().map(|(m, _)| *m));
        }
    }
    assert!(all.len() > 1_000, "{} members", all.len());
    for member in &all {
        let at = GalacticPosition::ORIGIN.displacement_to(member.record().epoch_position());
        assert!(shell.contains(&at), "{shell:?} misses {at:?}");
    }
    let near = all[all.len() / 3].record().epoch_position();
    for years in [0, 1_000, -1_000] {
        let s = sphere(*near, 30.0, years);
        let elapsed = Seconds::new(s.time().since_epoch().as_seconds_f64());
        let mut want: Vec<SystemHit> = all
            .iter()
            .filter_map(|m| hit_at(m.record(), m.velocity(), &s, elapsed))
            .collect();
        want.sort_by_key(|hit| hit.id().raw());
        let got = hits(&source, galaxy, &s);
        // The streams' hits, if any, sort apart: their IDs are of another sub-kind.
        let cores: Vec<SystemHit> = got
            .into_iter()
            .filter(|hit| matches!(hit.id().kind(), crate::id::SystemIdKind::DwarfCore(_)))
            .collect();
        assert_eq!(cores, want, "{years}");
        assert_ne!(want.len(), 0, "{years}");
    }
    let census = source.expected_in_sphere(galaxy, &sphere(*near, 30.0, 0));
    assert!(census.get(Layer::A) > 0.0, "{census:?}");
}

/// P10.T9: every hit is a member of the global list in a requested layer, of the halo's
/// population and no density component (plan 10, "Members as `SystemRecord`s"), inside the
/// sphere at its time; a query asking for layer A alone gets layer A's hits alone.
#[test]
fn hits_are_global_list_members_of_the_layers_asked() {
    let galaxy = galaxy(true);
    let lookup = lookup();
    let source = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
    let centre = lookup.tubes[0].locate(0.45, 0.0, 0.0);
    let s = sphere(centre, 300.0, 200);
    let all = hits(&source, galaxy, &s);
    assert!(all.len() > 10, "{} hits", all.len());
    let mut layers = std::collections::BTreeSet::new();
    for hit in &all {
        let record = hit.record();
        assert!(matches!(
            record.origin(),
            SystemOrigin::GlobalListMember { .. }
        ));
        assert_eq!(record.population(), Population::Halo);
        assert_eq!(record.component(), None);
        assert!(hit.distance().value() <= 300.0);
        layers.insert(record.layer());
    }
    assert!(layers.len() >= 2, "{layers:?}");
    let mut only_a = Vec::new();
    source.systems_in_sphere(galaxy, &s, LayerSet::EMPTY.with(Layer::A), &mut only_a);
    only_a.sort_by_key(|hit| hit.id().raw());
    let want: Vec<SystemHit> = all
        .into_iter()
        .filter(|hit| hit.record().layer() == Layer::A)
        .collect();
    assert_eq!(only_a, want);
}

/// P10.T9: merged with the grid by `range_query`, the source's hits keep their IDs apart from the
/// grid's, and the census counts its members.
#[test]
fn the_range_query_merges_the_list_s_members() {
    let galaxy = galaxy(true);
    let lookup = lookup();
    let source = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
    // About a member of the small core, which lies dense.
    let model = source
        .core(DwarfCoreNumber::new(0).expect("0"))
        .expect("played");
    let mut members = Vec::new();
    let centre = model
        .grid()
        .owned_cells()
        .find_map(|cell| {
            model.members_in_cell(galaxy, MassBand::A, cell, &mut members);
            members.first().map(|(m, _)| *m.record().epoch_position())
        })
        .expect("a core with members");
    let query = RangeQuery::builder(centre, LightYears::new(20.0))
        .build()
        .expect("a valid query");
    let alone = range_query(galaxy, &mut NoCache::new(), &[], &query);
    let merged = range_query(galaxy, &mut NoCache::new(), &[&source], &query);
    let is_member =
        |hit: &&SystemHit| matches!(hit.record().origin(), SystemOrigin::GlobalListMember { .. });
    assert!(merged.systems().iter().any(|hit| is_member(&hit)));
    // The grid's hits are the query's alone, but for the layers the members' counts stopped.
    let grid: Vec<u64> = alone.systems().iter().map(|hit| hit.id().raw()).collect();
    for hit in merged.systems().iter().filter(|hit| !is_member(hit)) {
        assert!(grid.contains(&hit.id().raw()), "{hit:?}");
    }
    let a = merged.census().expected().get(Layer::A);
    assert!(a > alone.census().expected().get(Layer::A), "{a}");
}

/// The stream shell's hold, P10.T9: until the bump to 26 every galaxy built by `from_params`
/// takes the held arm, and the tests reach the new one under every 26 hold.
#[test]
fn the_stream_shell_is_held_only_until_the_26_bump() {
    assert!(
        crate::GENERATOR_VERSION < crate::galaxy::global_list::STREAM_SHELL_VERSION,
        "the 25 → 26 bump puts P10.T9's shell on the record: a stream's member outside its \
         bounding shell is none, so `resolve_with`'s answers for those members and the global list \
         source's hits and census move. Delete `STREAM_SHELL_VERSION`, `StreamShellRules` with \
         its `Unbounded` arm, `GalaxyRules::stream_shell`, the `match` in `in_stream_shell` and \
         in `resolve_with`, `GlobalListSource::entries_near`'s `every_stream` and this test, in \
         the bump's own commit; `tests/global_list_tube.rs`'s \
         `resolve_with_reaches_a_drawn_stream_s_members` then needs its galaxy's list, not \
         `GlobalList::default()`, and to expect `NoSuchSystem` for a member outside the shell"
    );
    let built = Galaxy::from_params(SEED, GalaxyParams::milky_way_like()).expect("builds");
    assert_eq!(built.rules().stream_shell, StreamShellRules::Unbounded);
    assert_eq!(galaxy(true).rules().stream_shell, StreamShellRules::Shell);
}

/// Every member of every cell of `tube`, whose stream's band shares are `shares`, in every band.
fn every_member(
    galaxy: &Galaxy,
    tube: &TubeTable,
    shares: &StreamBandShares,
) -> Vec<GlobalMemberRecord> {
    use crate::galaxy::global_list::tube::ACROSS_CELLS;
    use crate::id::StreamMemberId;
    let mut all = Vec::new();
    let mut members = Vec::new();
    for along in 0..tube.along_cells() {
        let along = u16::try_from(along).expect("at most 16,384 along cells");
        for a in 0..ACROSS_CELLS {
            for b in 0..ACROSS_CELLS {
                let cell = StreamMemberId::new(tube.stream().get(), Layer::A, along, [a, b], 0)
                    .expect("a tube's cell")
                    .into();
                for band in BANDS_COARSEST_FIRST {
                    stream_cell_members(galaxy, tube, shares, cell, band, &mut members);
                    all.extend_from_slice(&members);
                }
            }
        }
    }
    all
}

/// The hits a brute-force scan of `all` makes in `sphere`, those inside `shell` under the
/// galaxy's rules, sorted by ID.
fn brute_force(
    galaxy: &Galaxy,
    all: &[GlobalMemberRecord],
    shell: &BoundingShell,
    sphere: &QuerySphere,
) -> Vec<SystemHit> {
    let elapsed = Seconds::new(sphere.time().since_epoch().as_seconds_f64());
    let mut want: Vec<SystemHit> = all
        .iter()
        .filter(|m| in_stream_shell(galaxy, shell, m.record().epoch_position()))
        .filter_map(|m| hit_at(m.record(), m.velocity(), sphere, elapsed))
        .collect();
    want.sort_by_key(|hit| hit.id().raw());
    want
}

/// The slow fixture's lookup: `pal5_like`'s and `gd1_like`'s tubes at the generator's 2,000
/// tracers, on a list of the two.
fn generator_lookup() -> &'static Lookup {
    static LOOKUP: OnceLock<Lookup> = OnceLock::new();
    LOOKUP.get_or_init(|| {
        let list =
            GlobalList::of_parts(galaxy(false), tables(), streams()[..2].to_vec(), Vec::new());
        Lookup::new(list, 1_000, false)
    })
}

/// P10.T9: a query on the axis of `gd1_like`, and on `pal5_like`'s, at the generator's 2,000
/// tracers, equals a brute-force enumeration of every member of its tube, at the epoch and a
/// millennium either way, under both arms of the stream shell's hold (from 26 the members inside
/// the shell); every member of `gd1_like` lies inside its shell, and of `pal5_like`'s some lie
/// outside, which 26 drops (printed); and no member moves faster than its tube's speed bound.
#[test]
#[ignore = "slow: sprays two test streams of 2,000 tracers and enumerates every member of their tubes"]
fn a_query_on_a_test_stream_s_axis_equals_every_member_of_its_tube() {
    let lookup = generator_lookup();
    for (n, name) in [(1, "gd1"), (0, "pal5")] {
        let tube = &lookup.tubes[n];
        let shell = lookup.list.entries()[n].shell();
        let held = galaxy(false);
        let shares = StreamBandShares::of(held, &lookup.list.debris().streams()[n]);
        let all = every_member(held, tube, &shares);
        let outside = all
            .iter()
            .filter(|m| {
                !shell.contains(
                    &GalacticPosition::ORIGIN.displacement_to(m.record().epoch_position()),
                )
            })
            .count();
        let bound = tube.member_speed_bound().value();
        let fastest = all
            .iter()
            .map(|m| crate::units::KilometresPerSecond::from(m.velocity().speed()).value())
            .fold(0.0, f64::max);
        eprintln!(
            "{name}: {} members, {outside} outside the shell; fastest {fastest:.1} km/s against \
             the bound's {bound:.1}",
            all.len()
        );
        assert!(fastest <= bound, "{name}: {fastest} against {bound}");
        if name == "gd1" {
            assert_eq!(outside, 0, "{name}");
        }
        for at_26 in [false, true] {
            let galaxy = galaxy(at_26);
            let source =
                GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
            for (u, radius, occupied) in
                [(0.2, 60.0, false), (0.5, 300.0, true), (0.9, 150.0, false)]
            {
                for years in [0, 1_000, -1_000] {
                    let s = sphere(tube.locate(u, 0.0, 0.0), radius, years);
                    let got = hits(&source, galaxy, &s);
                    let mine: Vec<SystemHit> = got
                        .into_iter()
                        .filter(|hit| {
                            matches!(
                                hit.id().kind(),
                                crate::id::SystemIdKind::Stream(id) if usize::from(id.number()) == n
                            )
                        })
                        .collect();
                    let want = brute_force(galaxy, &all, &shell, &s);
                    assert_eq!(ids(&mine), ids(&want), "{name} {at_26} {u} {years}");
                    assert_eq!(mine, want, "{name} {at_26} {u} {years}");
                    if occupied {
                        assert_ne!(want.len(), 0, "{name}: no member on the axis");
                    }
                }
            }
        }
    }
}

/// The most of `pal5_like`'s members that lie outside its shell under any of the eight seeds of
/// [`over_eight_seeds_the_test_tubes_lie_within_their_tracers_and_shells`], as measured
/// (2026-10-31, x86-64) with its tips fitted in the orbital plane (P10.T6.a): 2, of 3 over the
/// eight, against 147 at the first seed when they were fitted in x, y and z.
const PAL5_OUTSIDE_PINNED: usize = 2;

/// P10.T6.a's tips, fitted in the orbital plane: over eight seeds of the generator's 2,000
/// tracers, every knot of `pal5_like` and `gd1_like` lies within its arm's tracers' radial range,
/// and of their members outside the list's shell `gd1_like` has none and `pal5_like` at most
/// [`PAL5_OUTSIDE_PINNED`] a seed (each seed printed, with the knots that kept the Cartesian fit,
/// none as measured).
#[test]
#[ignore = "slow: sprays two test streams under eight seeds and enumerates their tubes"]
fn over_eight_seeds_the_test_tubes_lie_within_their_tracers_and_shells() {
    use crate::galaxy::global_list::spray::Arm;
    let held = galaxy(false);
    let list = GlobalList::of_parts(held, tables(), streams()[..2].to_vec(), Vec::new());
    let force = SprayForce::new(tables()).expect("the full tables");
    let params = SprayParams::new(NonZeroU32::new(1_000).expect("not zero"));
    let mean = held.mean_system_mass(Population::Halo);
    let radius = |x: GalacticDisplacement| {
        let [a, b, c] = x.metres();
        (a * a + b * b + c * c).sqrt() / METRES_PER_LIGHT_YEAR
    };
    for (n, name) in [(0, "pal5"), (1, "gd1")] {
        let spec = &list.debris().streams()[n];
        let shell = list.entries()[n].shell();
        let shares = StreamBandShares::of(held, spec);
        for k in 0..8 {
            let seed = Seed::new(SEED.get() + k);
            let tracers = Spray::new(seed, &force, tables(), spec, &params)
                .expect("a finite stripping time")
                .tracers();
            let before = crate::galaxy::global_list::tube::cartesian_knots();
            let tube = TubeTable::from_tracers(seed, mean, spec, &tracers).expect("enough");
            let fallbacks = crate::galaxy::global_list::tube::cartesian_knots() - before;
            let range = |arm: Arm| {
                tracers
                    .iter()
                    .filter(|t| t.energy_arm() == arm)
                    .map(|t| radius(t.position()))
                    .fold((f64::INFINITY, 0.0_f64), |(a, b), r| (a.min(r), b.max(r)))
            };
            for arm in [Arm::Trailing, Arm::Leading] {
                let (low, high) = range(arm);
                for (j, knot) in tube.arm(arm).knots().iter().enumerate() {
                    let r = radius(GalacticPosition::ORIGIN.displacement_to(&knot.centre()));
                    assert!(
                        (low..=high).contains(&r),
                        "{name} +{k} {arm:?} knot {j}: {r:.0} ly, its arm's tracers \
                         {low:.0}–{high:.0}"
                    );
                }
            }
            let all = every_member(held, &tube, &shares);
            let outside = all
                .iter()
                .filter(|m| {
                    !shell.contains(
                        &GalacticPosition::ORIGIN.displacement_to(m.record().epoch_position()),
                    )
                })
                .count();
            eprintln!(
                "{name} +{k}: {} members, {outside} outside the shell; {fallbacks} knots kept the \
                 Cartesian fit",
                all.len()
            );
            let most = if n == 0 { PAL5_OUTSIDE_PINNED } else { 0 };
            assert!(
                outside <= most,
                "{name} +{k}: {outside} outside, against {most}"
            );
        }
    }
}

/// P10.T9, the bench's figure: a 50 ly query on `pal5_like`'s axis generates every member of the
/// cells it touches and returns those in the sphere (measured 2026-10-31, the tips fitted in the
/// orbital plane: 135 generated and 22 returned over the 19 queries, about 6 a hit, where the plan
/// estimated about 160 to find two).
#[test]
#[ignore = "slow: sprays a test stream of 2,000 tracers"]
fn a_query_on_a_globular_stream_s_axis_generates_its_cells_members() {
    let lookup = generator_lookup();
    let galaxy = galaxy(true);
    let source = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("no centre");
    let tube = &lookup.tubes[0];
    let shares = StreamBandShares::of(galaxy, &lookup.list.debris().streams()[0]);
    let (mut generated, mut returned) = (0, 0);
    for k in 1..20 {
        let centre = tube.locate(f64::from(k) / 20.0, 0.0, 0.0);
        let s = sphere(centre, 50.0, 0);
        let got = hits(&source, galaxy, &s);
        returned += got.len();
        let mut members = Vec::new();
        for cell in tube.cells_touching(&centre, LightYears::new(50.0)) {
            for band in BANDS_COARSEST_FIRST {
                stream_cell_members(galaxy, tube, &shares, cell, band, &mut members);
                generated += members.len();
            }
        }
    }
    eprintln!(
        "pal5: {generated} members generated, {returned} returned over 19 queries of 50 ly: {:.0} \
         a hit",
        f64::from(u32::try_from(generated).expect("small"))
            / f64::from(u32::try_from(returned.max(1)).expect("small"))
    );
    assert!(returned > 0 && generated > returned);
}

/// A lookup of a list and one table, which it hands out for every stream.
struct One(GlobalList, Arc<TubeTable>);

impl TubeLookup for One {
    fn global_list(&self) -> &GlobalList {
        &self.0
    }

    fn tube(&self, _: StreamNumber) -> Arc<TubeTable> {
        Arc::clone(&self.1)
    }
}

/// P10.T9: from version 26 `resolve_with` refuses a drawn stream's member outside its entry's
/// shell, as the source leaves it out, and below 26 resolves it; with the list's own shell both
/// resolve it. The shell is the empty one here, so that every member is outside.
#[test]
#[ignore = "slow: builds a Milky Way galaxy's kinematic tables, its list and a tube"]
fn resolve_with_refuses_a_stream_member_outside_its_shell_from_26() {
    use crate::galaxy::placement::resolve_with;
    for at_26 in [false, true] {
        let rules = if at_26 {
            GalaxyRules::every_26_hold()
        } else {
            GalaxyRules::of_version(crate::GENERATOR_VERSION)
        };
        let galaxy = Galaxy::from_params_with_rules(
            Seed::new(0x0a10_8b01),
            GalaxyParams::milky_way_like(),
            rules,
        )
        .expect("builds")
        .with_full_potential();
        let list = GlobalList::build(&galaxy);
        let spec = &list.debris().streams()[0];
        let tube = Arc::new(TubeTable::build(&galaxy, spec).expect("the full potential"));
        let shares = StreamBandShares::of(&galaxy, spec);
        let shell = list.stream_entry(spec.number()).expect("listed").shell();
        let centre = tube.locate(0.5, 0.0, 0.0);
        let mut members = Vec::new();
        let mut found = None;
        for cell in tube.cells_touching(&centre, LightYears::new(100.0)) {
            stream_cell_members(&galaxy, &tube, &shares, cell, MassBand::A, &mut members);
            if let Some(m) = members.iter().find(|m| {
                shell.contains(
                    &GalacticPosition::ORIGIN.displacement_to(m.record().epoch_position()),
                )
            }) {
                found = Some(*m);
                break;
            }
        }
        let member = found.expect("a member inside the shell on the axis");
        let id = member.record().id();
        let own = One(list.clone(), Arc::clone(&tube));
        assert_eq!(resolve_with(&galaxy, id, &own), Ok(*member.record()));
        let mut emptied = list.clone();
        let index = emptied
            .entries
            .iter()
            .position(|e| e.kind() == GlobalEntryKind::Stream(spec.number()))
            .expect("listed");
        emptied.entries[index] = GlobalEntry::new(
            GlobalEntryKind::Stream(spec.number()),
            spec.mass(),
            BoundingShell::new(LightYears::new(1.0), LightYears::ZERO),
        );
        let empty = One(emptied, tube);
        let want = if at_26 {
            Err(crate::galaxy::placement::ResolveSystemError::NoSuchSystem)
        } else {
            Ok(*member.record())
        };
        assert_eq!(resolve_with(&galaxy, id, &empty), want, "{at_26}");
    }
}

/// A lookup that builds each of a drawn galaxy's tables on first use and keeps it.
struct Kept<'g> {
    galaxy: &'g Galaxy,
    list: GlobalList,
    tubes: std::sync::Mutex<std::collections::BTreeMap<StreamNumber, Arc<TubeTable>>>,
}

impl TubeLookup for Kept<'_> {
    fn global_list(&self) -> &GlobalList {
        &self.list
    }

    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
        let spec = self.list.stream(stream).expect("a stream of the list");
        let mut tubes = self.tubes.lock().expect("not poisoned");
        Arc::clone(tubes.entry(stream).or_insert_with(|| {
            Arc::new(TubeTable::build(self.galaxy, spec).expect("the full potential"))
        }))
    }
}

/// P10.T9, from version 26: in a drawn galaxy's list, cut to its centre and its first stream so
/// that one tube is built, the source's members of the centre are the centre's own source's, and
/// its census holds the centre's; and each stream member it returns about a member inside the
/// stream's shell resolves through `resolve_with` to the record it returned.
#[test]
#[ignore = "slow: builds a Milky Way galaxy's kinematic tables, its list, its centre and a tube"]
fn a_drawn_galaxy_s_source_holds_the_centre_s_and_resolves_its_members() {
    use crate::galaxy::features::centre::CentreMemberSource;
    use crate::galaxy::placement::resolve_with;
    use crate::id::SystemIdKind;
    let galaxy = Galaxy::from_params_with_rules(
        Seed::new(0x0a10_8b01),
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_26_hold(),
    )
    .expect("builds")
    .with_full_potential();
    let full = GlobalList::build(&galaxy);
    let spec = full.debris().streams()[0].clone();
    let entry = *full.stream_entry(spec.number()).expect("listed");
    assert_eq!(full.entries()[0].kind(), GlobalEntryKind::Centre);
    let list = GlobalList {
        entries: vec![full.entries()[0], entry],
        debris: full.debris().clone(),
    };
    let lookup = Kept {
        galaxy: &galaxy,
        list,
        tubes: std::sync::Mutex::default(),
    };
    let source = GlobalListSource::new(&galaxy, &lookup, &NoCentreOrbitCache).expect("a centre");
    let centre = CentreMemberSource::new(&galaxy, &NoCentreOrbitCache).expect("a centre");
    let s = sphere(GalacticPosition::ORIGIN, 0.05, 100);
    let mut want = Vec::new();
    centre.systems_in_sphere(&galaxy, &s, stellar(), &mut want);
    want.sort_by_key(|hit| hit.id().raw());
    let got: Vec<SystemHit> = hits(&source, &galaxy, &s)
        .into_iter()
        .filter(|hit| matches!(hit.id().kind(), SystemIdKind::Centre(_)))
        .collect();
    assert!(!want.is_empty(), "no member of the centre within 0.05 ly");
    assert_eq!(got, want);
    let (all, own) = (
        source.expected_in_sphere(&galaxy, &s),
        centre.expected_in_sphere(&galaxy, &s),
    );
    for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
        assert!(
            all.get(layer) >= own.get(layer),
            "{layer:?}: {all:?} {own:?}"
        );
    }
    // A member inside the shell, found as `resolve_with_refuses_…` finds one.
    let tube = lookup.tube(spec.number());
    let shares = StreamBandShares::of(&galaxy, &spec);
    let shell = entry.shell();
    let mut members = Vec::new();
    let mut found = None;
    for cell in tube.cells_touching(&tube.locate(0.5, 0.0, 0.0), LightYears::new(100.0)) {
        stream_cell_members(&galaxy, &tube, &shares, cell, MassBand::A, &mut members);
        found = members.iter().find_map(|m| {
            let at = m.record().epoch_position();
            shell
                .contains(&GalacticPosition::ORIGIN.displacement_to(at))
                .then_some(*at)
        });
        if found.is_some() {
            break;
        }
    }
    let near = found.expect("a member inside the shell on the axis");
    let returned: Vec<SystemHit> = hits(&source, &galaxy, &sphere(near, 20.0, 0))
        .into_iter()
        .filter(|hit| matches!(hit.id().kind(), SystemIdKind::Stream(_)))
        .collect();
    assert!(!returned.is_empty(), "no member about one of stream 0's");
    for hit in returned.iter().take(20) {
        assert_eq!(resolve_with(&galaxy, hit.id(), &lookup), Ok(*hit.record()));
    }
}
