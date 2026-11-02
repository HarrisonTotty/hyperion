//! The tests of P09.T37.a's source and its hold, `CLASS_SOURCE_VERSION`: its tripwire and the
//! version's rules; the classes' member layers, held by every member of each class's first cells
//! with entries; the census's expected hosts, exact over the cells' bounds, in each class's layers;
//! under every 27 hold, a host of each class returned by the range query at its place at two
//! times, with its origin, as plan 03 resolves it, and none outside its sphere; the census lines
//! and the hits whatever the cell cache holds and the order queries come in; and no grid twin left
//! for the source to suppress.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::catalogue_classes::core_collapse::recent_death_claims;
use crate::galaxy::catalogue_classes::ids::entry_of;
use crate::galaxy::catalogue_classes::lbv::lbv_claims;
use crate::galaxy::catalogue_classes::{
    CatalogueClassCell, CatalogueSystem, ClassProcess, occupied_cells, system_of,
};
use crate::galaxy::displaced::site::explosion_site;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellCache, CellKey, NoCache, SystemOrigin, generate_cell};
use crate::galaxy::query::{MassFloor, RangePlan, RangeQuery, RangeResult, hit_at, range_query};
use crate::galaxy::rules::GalaxyRules;
use crate::id::{CatalogueSystemId, SystemId, SystemIdKind};
use crate::planetary::testing::with_every_27_hold;

/// The seed of the hold's Milky Way fixture.
const SEED: u64 = 0x0937_0000_0000_0001;

/// The fixture under `rules`, with its kinematic tables, so that the classes' hosts move.
fn milky_way(rules: GalaxyRules) -> Galaxy {
    Galaxy::from_params_with_rules(Seed::new(SEED), GalaxyParams::milky_way_like(), rules)
        .expect("the Milky Way fixture builds")
        .with_full_potential()
}

/// The fixture under every 27 hold: the range query registers the classes.
fn at_27() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| milky_way(GalaxyRules::every_27_hold()))
}

/// The fixture under every 26 hold: the grid gives the classes their hosts, and the range query
/// does not register them.
fn at_26() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules::of_version(crate::GENERATOR_VERSION),
        )
        .expect("the Milky Way fixture builds")
    })
}

/// One class of the production lookup.
#[derive(Debug, Clone, Copy)]
struct Only(ClassId);

impl ClassLookup for Only {
    fn visit_class<V: ClassVisitor>(
        &self,
        galaxy: &Galaxy,
        class: ClassId,
        visitor: V,
    ) -> Option<V::Output> {
        if class == self.0 {
            ProductionClasses.visit_class(galaxy, class, visitor)
        } else {
            None
        }
    }
}

/// The classes the production lookup has a process for, in [`ALL_CLASSES`] order.
fn registered_classes(galaxy: &Galaxy) -> Vec<ClassId> {
    /// A visit that only says the process is there.
    struct Has;
    impl ClassVisitor for Has {
        type Output = ();
        fn visit<P: ClassProcess>(self, _: &P) {}
    }
    ALL_CLASSES
        .into_iter()
        .filter(|&class| ProductionClasses.visit_class(galaxy, class, Has).is_some())
        .collect()
}

/// A member of a class's entry: its ID, which member of its entry it is, and its system.
#[derive(Debug, Clone, Copy)]
struct Host {
    id: CatalogueSystemId,
    member: u8,
    system: CatalogueSystem,
}

/// The members of the entries of the first `cells` cells of `class` that hold an entry, in the
/// galaxy's plane (the class cells on either side of z = 0), from the farthest from the galactic
/// centre inwards, where the cells are fewest and the walks about them cheapest.
fn first_hosts(galaxy: &Galaxy, class: ClassId, cells: usize) -> Vec<Host> {
    let mut held = 0;
    hosts_until(galaxy, class, |cell| {
        held += usize::from(!cell.is_empty());
        held == cells
    })
}

/// The members of the entries of `class`'s cells in the galaxy's plane, as [`first_hosts`] walks
/// them, until `enough` is true of the members of the last cell walked.
fn hosts_until(galaxy: &Galaxy, class: ClassId, enough: impl FnMut(&[Host]) -> bool) -> Vec<Host> {
    /// The members of each cell's entries, cell by cell, until enough.
    struct Hosts<'a, F> {
        galaxy: &'a Galaxy,
        enough: F,
    }
    impl<F: FnMut(&[Host]) -> bool> ClassVisitor for Hosts<'_, F> {
        type Output = Vec<Host>;
        fn visit<P: ClassProcess>(mut self, process: &P) -> Vec<Host> {
            let class = process.class();
            let half = ROOT_HALF_WIDTH_LY >> class.cell_log2_ly().expect("a placed class");
            let mut keys: Vec<CatalogueCellKey> = (-half..half)
                .flat_map(|x| (-half..half).flat_map(move |y| [-1, 0].map(|z| [x, y, z])))
                .map(|cell| CatalogueCellKey::new(class, cell).expect("a cell of the cube"))
                .collect();
            // The cell centre's distance from the axis, squared, in cells, farthest first.
            let away = |key: &CatalogueCellKey| {
                let [x, y, _] = key.cell().map(|c| 2 * i64::from(c) + 1);
                x * x + y * y
            };
            keys.sort_by_key(|key| (std::cmp::Reverse(away(key)), *key));
            let mut members = Vec::new();
            for key in keys {
                let first = members.len();
                let cell = CatalogueClassCell::generate(self.galaxy, process, key);
                for entry in cell.entries() {
                    for member in 0..entry.members() {
                        let id = entry.member_id(member).expect("a member of the entry");
                        let system = system_of(self.galaxy, id).expect("an entry's member");
                        members.push(Host { id, member, system });
                    }
                }
                if (self.enough)(&members[first..]) {
                    break;
                }
            }
            members
        }
    }
    ProductionClasses
        .visit_class(galaxy, class, Hosts { galaxy, enough })
        .expect("a registered class")
}

/// The time `years` Julian years from the epoch.
fn at(years: i64) -> UniverseTime {
    UniverseTime::from_julian_years(years).expect("inside the clock window")
}

/// The census's figures, bit for bit, and the hits of `result`.
fn census_lines(result: &RangeResult) -> (Option<Layer>, String, Vec<u64>, Vec<SystemHit>) {
    let census = result.census();
    (
        census.complete_down_to(),
        format!("{:?}", census.stopped_by()),
        census.expected().to_array().map(bits).to_vec(),
        result.systems().to_vec(),
    )
}

/// P09.T37.a is held until the 26 → 27 bump: the rules in force are version 26's and before, under
/// which a galaxy registers no class source.
#[test]
fn the_class_source_is_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < CLASS_SOURCE_VERSION,
        "the 26 → 27 bump puts P09.T37.a on the record: the range query registers the catalogue \
         classes beside the grid. Delete `CLASS_SOURCE_VERSION`, `ClassSourceRules` with its \
         `Unregistered` arm, `GalaxyRules::class_source`, the `match` in \
         `CatalogueClassSource::registered_in` (which then always gives the source) and this \
         test, in the bump's own checkpoint commit, and re-bless what the classes' hosts move"
    );
    assert_eq!(
        GalaxyRules::of_version(GENERATOR_VERSION).class_source,
        ClassSourceRules::Unregistered
    );
    let held = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture builds");
    assert_eq!(CatalogueClassSource::registered_in(&held), None);
}

/// A galaxy under every 26 hold registers no source, so its range query takes the grid and the
/// features' members alone, as before; one under every 27 hold registers the production lookup's.
#[test]
fn a_galaxy_registers_the_source_from_27() {
    assert_eq!(CatalogueClassSource::registered_in(at_26()), None);
    assert_eq!(
        CatalogueClassSource::registered_in(at_27()),
        Some(CatalogueClassSource::new())
    );
}

/// Every class the production lookup has a process for has its member layers, and every member of
/// the entries of each one's first cells with entries is counted in its slot's layers, with no more
/// members than slots. The tidal-disruption victims have no process, so the source skips them.
#[test]
fn every_class_s_members_keep_to_their_layers() {
    let galaxy = at_27();
    let classes = registered_classes(galaxy);
    assert_eq!(classes.len(), 8, "{classes:?}");
    assert!(!classes.contains(&ClassId::TIDAL_DISRUPTION_VICTIM));
    let mut members = 0;
    for class in classes {
        let slots = member_layers(class, galaxy.rules().binary_tables)
            .unwrap_or_else(|| panic!("{class} has no layers"));
        let hosts = first_hosts(galaxy, class, 3);
        assert!(!hosts.is_empty(), "{class}");
        for host in &hosts {
            let layer = host.system.record().layer();
            let slot = slots
                .get(usize::from(host.member))
                .unwrap_or_else(|| panic!("{class}'s member {} has no slot", host.member));
            assert!(slot.contains(layer), "{class} {:?} in {layer:?}", host.id);
            assert_eq!(
                host.system.record().origin(),
                SystemOrigin::CatalogueSystem(class)
            );
        }
        members += hosts.len();
    }
    assert!(members > 8);
}

/// The census's expected hosts are the class cells' bounds × the volume each shares with the cube
/// about the unpadded sphere: exactly one cell's bound × its volume for the cube that is the cell,
/// and the sum over the cells for a sphere across their faces; each class counts them in its
/// layers only, once for each member slot that layer has, and the source's counts are the classes'
/// summed in their order.
#[test]
fn the_census_counts_each_class_s_expected_hosts_in_its_layers() {
    /// A class's process's bound over a cell.
    struct Bound<'a> {
        galaxy: &'a Galaxy,
        key: CatalogueCellKey,
    }
    impl ClassVisitor for Bound<'_> {
        type Output = f64;
        fn visit<P: ClassProcess>(self, process: &P) -> f64 {
            process.bound(self.galaxy, &self.key).value()
        }
    }
    let galaxy = at_27();
    let sphere = |centre: [f64; 3], radius: f64| {
        let centre = GalacticPosition::from_light_years(centre).expect("in the cube");
        QuerySphere::new(
            centre,
            LightYears::new(radius),
            at(300),
            LightYears::new(2.0),
        )
        .expect("a sphere")
    };
    let bound = |key: CatalogueCellKey| {
        ProductionClasses
            .visit_class(galaxy, key.class(), Bound { galaxy, key })
            .expect("a registered class")
    };
    let solar = sphere([0.0, 26_000.0, 20.0], 50.0);
    let mut summed = LayerCounts::ZERO;
    for class in registered_classes(galaxy) {
        let edge = f64::from(1_u32 << class.cell_log2_ly().expect("placed"));
        // The cube about this sphere is the class's cell [0, 6, 0] (or [0, 50, 0] at 512 ly).
        let y = if edge > 512.0 { 6 } else { 50 };
        let key = CatalogueCellKey::new(class, [0, y, 0]).expect("a cell");
        let centre = [0.5 * edge, (f64::from(y) + 0.5) * edge, 0.5 * edge];
        let whole = sphere(centre, 0.5 * edge);
        let counts =
            CatalogueClassSource::with_lookup(Only(class)).expected_in_sphere(galaxy, &whole);
        let slots = member_layers(class, galaxy.rules().binary_tables).expect("layers");
        for layer in Layer::ALL {
            let times = slots.iter().filter(|set| set.contains(layer)).count();
            let times = f64::from(u32::try_from(times).expect("few"));
            assert_same_bits(counts.get(layer), times * (bound(key) * edge * edge * edge));
        }
        // Across the faces: a sphere of 300 ly about a corner of four cells in the plane.
        let corner = sphere([0.0, f64::from(y) * edge, 10.0], 300.0);
        let mut hosts = 0.0;
        for x in [-1, 0] {
            for cy in [y - 1, y] {
                for z in [-1, 0] {
                    let key = CatalogueCellKey::new(class, [x, cy, z]).expect("a cell");
                    let along = |c: f64, origin: f64| {
                        let (low, high) = (c - 300.0, c + 300.0);
                        high.min(origin + edge) - low.max(origin)
                    };
                    let [ox, oy, oz] = key.origin_ly().map(f64::from);
                    let length = along(0.0, ox) * along(f64::from(y) * edge, oy) * along(10.0, oz);
                    hosts += bound(key) * length;
                }
            }
        }
        let counts =
            CatalogueClassSource::with_lookup(Only(class)).expected_in_sphere(galaxy, &corner);
        let layer = slots[0].iter().next().expect("a layer");
        let times = f64::from(
            u32::try_from(slots.iter().filter(|set| set.contains(layer)).count()).expect("few"),
        );
        assert_same_bits(counts.get(layer), times * hosts);
        // The substellar layers take nothing.
        assert_eq!(counts.get(Layer::BrownDwarf), 0.0);
        assert_eq!(counts.get(Layer::RoguePlanet), 0.0);
        let one = CatalogueClassSource::with_lookup(Only(class)).expected_in_sphere(galaxy, &solar);
        for layer in Layer::ALL {
            summed.set(layer, summed.get(layer) + one.get(layer));
        }
    }
    let all = CatalogueClassSource::new().expected_in_sphere(galaxy, &solar);
    for layer in Layer::ALL {
        assert_same_bits(all.get(layer), summed.get(layer));
    }
    // The core collapses, the luminous blue variables and the neutron-star mergers are of layer E
    // alone; the accreting white dwarfs of C and D, and the X-ray binaries of D and E.
    let only =
        |class| CatalogueClassSource::with_lookup(Only(class)).expected_in_sphere(galaxy, &solar);
    let core = only(ClassId::CORE_COLLAPSE);
    assert!(core.get(Layer::E) > 0.0);
    assert_eq!(core.to_array()[..4], [0.0; 4]);
    let wd = only(ClassId::ACCRETING_WHITE_DWARF);
    assert_eq!(wd.get(Layer::C), wd.get(Layer::D));
    assert!(wd.get(Layer::C) > 0.0);
    assert_eq!(
        [wd.get(Layer::A), wd.get(Layer::B), wd.get(Layer::E)],
        [0.0; 3]
    );
}

/// Under every 27 hold, a host of each class is returned by the range query at its place: within a
/// sphere of 1 ly about where its class's motion puts it, at the epoch and four centuries on, with
/// its record, of origin [`SystemOrigin::CatalogueSystem`] of its class, and its position, bit for
/// bit as plan 03 resolves and moves it ([`hit_at`]); and not by a sphere of 1 ly whose centre is
/// 1.5 ly from it. The source's hits are all its class's, each the one plan 03 makes of its
/// record. Each query walks its class's cells alone, about a host as far from the galactic centre
/// as one could be found, where its walk is cheapest.
#[test]
fn under_every_27_hold_a_host_of_each_class_is_returned_at_its_place() {
    let galaxy = at_27();
    let source = |class| CatalogueClassSource::with_lookup(Only(class));
    with_every_27_hold(|| {
        for class in registered_classes(galaxy) {
            let hosts = first_hosts(galaxy, class, 1);
            let host = hosts
                .iter()
                .find(|host| host.member == 0)
                .expect("an entry has member 0");
            let source = source(class);
            let sources: [&dyn SystemSource; 1] = [&source];
            for t in [UniverseTime::EPOCH, at(400)] {
                let place = host.system.position_at(t);
                let query = RangeQuery::builder(place, LightYears::new(1.0))
                    .time(t)
                    .build()
                    .expect("a query");
                let result = range_query(galaxy, &mut NoCache::new(), &sources, &query);
                let hit = result
                    .systems()
                    .iter()
                    .find(|hit| hit.id() == SystemId::from(host.id))
                    .unwrap_or_else(|| panic!("{class}'s host {:?} at {t}", host.id));
                let resolved = system_of(galaxy, host.id).expect("the host resolves");
                assert_eq!(hit.record(), resolved.record());
                assert_eq!(hit.record().origin(), SystemOrigin::CatalogueSystem(class));
                assert_eq!(hit.position(), &place);
                let sphere = QuerySphere::new(place, LightYears::new(1.0), t, LightYears::ZERO)
                    .expect("a sphere");
                for hit in result.systems() {
                    if let SystemIdKind::Catalogue(id) = hit.id().kind() {
                        assert_eq!(ClassId::from_value(id.class()), Some(class));
                        assert_eq!(hit_at(galaxy, hit.record(), &sphere).as_ref(), Some(hit));
                    }
                }
                // A sphere that misses the host by half a light-year.
                let [x, y, z] = place.to_light_years_f64();
                let beside = GalacticPosition::from_light_years([x + 1.5, y, z]).expect("in");
                let query = RangeQuery::builder(beside, LightYears::new(1.0))
                    .time(t)
                    .build()
                    .expect("a query");
                let mut out = Vec::new();
                source.systems_in_sphere(
                    galaxy,
                    &QuerySphere::new(beside, LightYears::new(1.0), t, LightYears::ZERO)
                        .expect("a sphere"),
                    RangePlan::new(galaxy, &sources, &query).census().layers(),
                    &mut out,
                );
                assert!(out.iter().all(|hit| hit.id() != SystemId::from(host.id)));
            }
        }
    });
}

/// Under every 27 hold, a Type Ia's survivor, member 1 of its entry, is returned by the range query
/// at its own place once its progenitor has exploded, flung from the site on its own line, with
/// its own record, counted in its own star's layer.
#[test]
fn under_every_27_hold_a_type_ia_s_survivor_is_returned_at_its_place() {
    let galaxy = at_27();
    let source = CatalogueClassSource::with_lookup(Only(ClassId::TYPE_IA));
    let sources: [&dyn SystemSource; 1] = [&source];
    let hosts = hosts_until(galaxy, ClassId::TYPE_IA, |cell| {
        cell.iter().any(|host| host.member == 1)
    });
    let survivor = hosts
        .iter()
        .find(|host| host.member == 1)
        .expect("a survivor in the plane");
    let progenitor = system_of(galaxy, entry_of(survivor.id)).expect("member 0");
    assert_eq!(survivor.system.motion().at(), progenitor.motion().at());
    assert!(survivor.system.motion().at() < UniverseTime::EPOCH);
    with_every_27_hold(|| {
        for t in [UniverseTime::EPOCH, at(-700)] {
            let place = survivor.system.position_at(t);
            assert_ne!(place, progenitor.position_at(t));
            let query = RangeQuery::builder(place, LightYears::new(1.0))
                .time(t)
                .build()
                .expect("a query");
            let result = range_query(galaxy, &mut NoCache::new(), &sources, &query);
            let hit = result
                .systems()
                .iter()
                .find(|hit| hit.id() == SystemId::from(survivor.id))
                .expect("the survivor is found");
            assert_eq!(hit.record(), survivor.system.record());
            assert_eq!(hit.position(), &place);
            assert!(SURVIVOR_LAYERS.contains(hit.record().layer()));
        }
    });
}

/// The layers a Type Ia's survivor is counted in.
const SURVIVOR_LAYERS: LayerSet = LayerSet::EMPTY
    .with(Layer::A)
    .with(Layer::B)
    .with(Layer::C)
    .with(Layer::D);

/// A cell cache that keeps every cell it generates, so that a query after another reads its cells
/// warm.
#[derive(Debug, Default)]
struct Keep(BTreeMap<CellKey, Vec<crate::galaxy::placement::SystemRecord>>);

impl CellCache for Keep {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[crate::galaxy::placement::SystemRecord]) -> R,
    ) -> R {
        f(self.0.entry(key).or_insert_with(|| {
            let mut systems = Vec::new();
            generate_cell(galaxy, key, &mut systems);
            systems
        }))
    }
}

/// The census lines (how far down it is complete, why it stopped, and each layer's expected count,
/// bit for bit) and the hits of queries with every class registered are the same through one
/// shared cache, whatever the order the queries come in, as through none: the source keeps
/// nothing, and its counts are a function of the galaxy and the sphere. The queries lie in the
/// outer disc, where the classes' walks are cheapest.
#[test]
fn the_census_lines_do_not_depend_on_what_is_cached() {
    let galaxy = at_27();
    let source = CatalogueClassSource::new();
    let sources: [&dyn SystemSource; 1] = [&source];
    let query = |centre: [f64; 3], radius: f64, years: i64| {
        RangeQuery::builder(
            GalacticPosition::from_light_years(centre).expect("in the cube"),
            LightYears::new(radius),
        )
        .time(at(years))
        .build()
        .expect("a query")
    };
    let queries = [
        query([0.0, 48_000.0, 0.0], 30.0, 0),
        query([0.0, 48_000.0, 0.0], 12.0, 400),
        query([2_000.0, 47_500.0, 60.0], 20.0, -300),
    ];
    let uncached: Vec<_> = queries
        .iter()
        .map(|query| census_lines(&range_query(galaxy, &mut NoCache::new(), &sources, query)))
        .collect();
    // The classes' hosts are in the census: the accreting white dwarfs' in layer C.
    let c = usize::from(Layer::C.value());
    assert!(
        queries
            .iter()
            .zip(&uncached)
            .all(|(query, (_, _, expected, _))| {
                let grid = RangePlan::new(galaxy, &[], query)
                    .census()
                    .expected()
                    .get(Layer::C);
                f64::from_bits(expected[c]) > grid
            })
    );
    let cache = RefCell::new(Keep::default());
    assert_order_independent(&queries, |query| {
        let lines = census_lines(&range_query(
            galaxy,
            &mut *cache.borrow_mut(),
            &sources,
            query,
        ));
        let position = queries
            .iter()
            .position(|q| q == query)
            .expect("one of the queries");
        assert_eq!(lines, uncached[position]);
        lines
    });
}

/// The grid keeps no twin of a class host under every 27 hold, so the source suppresses nothing:
/// no layer-E system a 100 ly query at the Sun-like point finds in the grid is one the core
/// collapses or the luminous blue variables would claim (P09.T35's carve-out), and the source
/// suppresses none of the grid's systems.
#[test]
fn the_grid_keeps_no_host_s_twin_and_the_source_suppresses_nothing() {
    let galaxy = at_27();
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).expect("in the cube");
    let query = RangeQuery::builder(sun, LightYears::new(100.0))
        .mass_floor(MassFloor::LayerE)
        .build()
        .expect("a query");
    let grid = range_query(galaxy, &mut NoCache::new(), &[], &query);
    let source = CatalogueClassSource::new();
    let mut layer_e = 0;
    for hit in grid.systems() {
        let record = hit.record();
        assert!(!source.suppresses(galaxy, record, query.time()));
        if record.layer() == Layer::E {
            layer_e += 1;
            let claimed = explosion_site(galaxy, record)
                .is_some_and(|site| recent_death_claims(galaxy, &site, record))
                || lbv_claims(galaxy, record);
            assert!(!claimed, "{:?}", record.id());
        }
    }
    assert!(layer_e > 0);
}

/// A process that hides its class's place read first, and gives every other answer as `P` does:
/// what the source walks without rule 11's lever.
struct NoPlace<'a, P>(&'a P);

impl<P: ClassProcess> ClassProcess for NoPlace<'_, P> {
    type Candidate = P::Candidate;
    type Entry = P::Entry;

    fn class(&self) -> ClassId {
        self.0.class()
    }

    fn bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> crate::units::PerCubicLightYear {
        self.0.bound(galaxy, cell)
    }

    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: crate::units::PerCubicLightYear,
    ) -> Option<P::Candidate> {
        self.0.candidate(galaxy, cell, id, bound)
    }

    fn test(&self, galaxy: &Galaxy, candidate: P::Candidate) -> Option<P::Entry> {
        self.0.test(galaxy, candidate)
    }

    fn members(&self, entry: &P::Entry) -> u8 {
        self.0.members(entry)
    }

    fn system(
        &self,
        galaxy: &Galaxy,
        member: &crate::galaxy::catalogue_classes::CatalogueMember<P::Entry>,
    ) -> CatalogueSystem {
        self.0.system(galaxy, member)
    }

    fn event_time(&self, galaxy: &Galaxy, id: CatalogueSystemId) -> Option<UniverseTime> {
        self.0.event_time(galaxy, id)
    }
}

/// The production lookup with its processes' places hidden ([`NoPlace`]): of one class, or of
/// every class.
#[derive(Debug, Clone, Copy)]
struct Unplaced(Option<ClassId>);

impl ClassLookup for Unplaced {
    fn visit_class<V: ClassVisitor>(
        &self,
        galaxy: &Galaxy,
        class: ClassId,
        visitor: V,
    ) -> Option<V::Output> {
        /// The visit of `V` with the process's place hidden.
        struct Hide<V>(V);
        impl<V: ClassVisitor> ClassVisitor for Hide<V> {
            type Output = V::Output;
            fn visit<P: ClassProcess>(self, process: &P) -> V::Output {
                self.0.visit(&NoPlace(process))
            }
        }
        if self.0.is_some_and(|only| only != class) {
            return None;
        }
        ProductionClasses.visit_class(galaxy, class, Hide(visitor))
    }
}

/// `class`'s place read first for the candidate whose entry `id` is a member of.
fn place_of(galaxy: &Galaxy, class: ClassId, id: CatalogueSystemId) -> Option<GalacticPosition> {
    /// The place of candidate `id`.
    struct Place<'a> {
        galaxy: &'a Galaxy,
        id: CatalogueSystemId,
    }
    impl ClassVisitor for Place<'_> {
        type Output = Option<GalacticPosition>;
        fn visit<P: ClassProcess>(self, process: &P) -> Option<GalacticPosition> {
            let key = CatalogueCellKey::of(self.id).expect("a class cell's ID");
            process.epoch_place(self.galaxy, &key, entry_of(self.id))
        }
    }
    ProductionClasses
        .visit_class(galaxy, class, Place { galaxy, id })
        .expect("a registered class")
}

/// The grid's rule 11 as each class keeps it: the accreting white dwarfs, the two merger classes
/// and the luminous blue variables read their place first, and it is every member's epoch
/// position, bit for bit, each moving below plan 03's `PAD_SPEED`, over each class's first cells
/// with entries; the supernova classes and the X-ray binaries read none.
#[test]
fn the_classes_that_read_their_place_first_keep_their_entries_there() {
    let galaxy = at_27();
    let placed = [
        ClassId::STELLAR_MERGER,
        ClassId::NEUTRON_STAR_MERGER,
        ClassId::LUMINOUS_BLUE_VARIABLE,
        ClassId::ACCRETING_WHITE_DWARF,
        ClassId::ACCRETING_WHITE_DWARF_SLOW,
    ];
    let mut checked = 0;
    for class in registered_classes(galaxy) {
        for host in first_hosts(galaxy, class, 3) {
            let place = place_of(galaxy, class, host.id);
            assert_eq!(place.is_some(), placed.contains(&class), "{class}");
            if let Some(place) = place {
                assert_eq!(
                    *host.system.record().epoch_position(),
                    place,
                    "{:?}",
                    host.id
                );
                assert_eq!(host.system.position_at(UniverseTime::EPOCH), place);
                let speed = host
                    .system
                    .motion()
                    .velocity_at(UniverseTime::EPOCH)
                    .speed()
                    .value();
                assert!(speed < 1e3 * PAD_SPEED.value(), "{class}: {speed} m/s");
                assert_eq!(
                    host.system.motion().velocity_at(at(-1_000)),
                    host.system.motion().velocity_at(at(1_000))
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 10, "{checked}");
}

/// The place read first changes no hit: about each place-reading class's first host, at both ends
/// of the clock window and at the epoch, the class's hits are those it finds with its place hidden,
/// and about a point of the outer disc every class's are, bit for bit and in order.
#[test]
fn the_place_read_first_changes_no_hit() {
    let galaxy = at_27();
    let stellar: LayerSet = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .collect();
    let hits = |source: &dyn SystemSource, sphere: &QuerySphere| {
        let mut out = Vec::new();
        source.systems_in_sphere(galaxy, sphere, stellar, &mut out);
        out
    };
    let mut found = 0;
    for class in [
        ClassId::STELLAR_MERGER,
        ClassId::NEUTRON_STAR_MERGER,
        ClassId::LUMINOUS_BLUE_VARIABLE,
        ClassId::ACCRETING_WHITE_DWARF,
        ClassId::ACCRETING_WHITE_DWARF_SLOW,
    ] {
        let host = first_hosts(galaxy, class, 1)[0];
        let placed = CatalogueClassSource::with_lookup(Only(class));
        let unplaced = CatalogueClassSource::with_lookup(Unplaced(Some(class)));
        for t in [at(-1_000), UniverseTime::EPOCH, at(1_000)] {
            let place = host.system.position_at(t);
            let sphere = QuerySphere::new(place, LightYears::new(2.0), t, LightYears::ZERO)
                .expect("a sphere");
            let expected = hits(&unplaced, &sphere);
            found += expected.len();
            assert_eq!(hits(&placed, &sphere), expected, "{class} at {t}");
        }
    }
    assert!(found >= 15, "{found}");
    let outer = GalacticPosition::from_light_years([0.0, 48_000.0, 0.0]).expect("in the cube");
    let sphere = QuerySphere::new(outer, LightYears::new(40.0), at(900), LightYears::ZERO)
        .expect("a sphere");
    let unplaced = CatalogueClassSource::with_lookup(Unplaced(None));
    assert_eq!(
        hits(&CatalogueClassSource::new(), &sphere),
        hits(&unplaced, &sphere)
    );
}

/// A class-cell cache that keeps every cell's systems it makes, for one galaxy.
#[derive(Debug, Default)]
struct KeepClassCells(RefCell<BTreeMap<CatalogueCellKey, Arc<ClassCellSystems>>>);

impl CatalogueClassCellCache for KeepClassCells {
    fn systems(
        &self,
        _galaxy: &Galaxy,
        key: CatalogueCellKey,
        make: &mut dyn FnMut() -> ClassCellSystems,
    ) -> Arc<ClassCellSystems> {
        let mut kept = self.0.borrow_mut();
        Arc::clone(kept.entry(key).or_insert_with(|| Arc::new(make())))
    }
}

/// The systems of class cell `key` made afresh.
struct Made<'a> {
    galaxy: &'a Galaxy,
    key: CatalogueCellKey,
}

impl ClassVisitor for Made<'_> {
    type Output = ClassCellSystems;
    fn visit<P: ClassProcess>(self, process: &P) -> ClassCellSystems {
        ClassCellSystems::of(self.galaxy, process, self.key)
    }
}

/// The class-cell cache changes no answer (P09.T40): the census and the hits of queries in the
/// outer disc, at three times and of four radii, and about a placed host and a core collapse, are
/// the same through one cache that keeps every cell, whatever the order the queries come in, as
/// through none; and each cell it holds is the one made afresh.
#[test]
fn a_kept_class_cell_changes_no_answer_whatever_was_asked_before() {
    let galaxy = at_27();
    let sphere = |centre: GalacticPosition, radius: f64, t: UniverseTime| {
        QuerySphere::new(centre, LightYears::new(radius), t, LightYears::ZERO).expect("a sphere")
    };
    let ly = |p: [f64; 3]| GalacticPosition::from_light_years(p).expect("in the cube");
    let core = first_hosts(galaxy, ClassId::CORE_COLLAPSE, 1)[0].system;
    let placed = first_hosts(galaxy, ClassId::ACCRETING_WHITE_DWARF, 1)[0].system;
    let spheres = [
        sphere(ly([0.0, 48_000.0, 0.0]), 50.0, UniverseTime::EPOCH),
        sphere(ly([0.0, 48_000.0, 0.0]), 1.0, at(400)),
        sphere(ly([1_500.0, 47_000.0, 80.0]), 20.0, at(-900)),
        sphere(core.position_at(at(300)), 2.0, at(300)),
        sphere(
            placed.position_at(UniverseTime::EPOCH),
            1.0,
            UniverseTime::EPOCH,
        ),
    ];
    let stellar: LayerSet = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .collect();
    let answer = |source: &dyn SystemSource, sphere: &QuerySphere| {
        let mut hits = Vec::new();
        source.systems_in_sphere(galaxy, sphere, stellar, &mut hits);
        let counts = source
            .expected_in_sphere(galaxy, sphere)
            .to_array()
            .map(bits);
        (counts, hits)
    };
    let uncached: Vec<_> = spheres
        .iter()
        .map(|sphere| answer(&CatalogueClassSource::new(), sphere))
        .collect();
    assert!(uncached.iter().filter(|(_, hits)| !hits.is_empty()).count() >= 2);
    let kept = KeepClassCells::default();
    assert_order_independent(&spheres, |sphere| {
        let found = answer(&CatalogueClassSource::new().with_cells(&kept), sphere);
        let position = spheres
            .iter()
            .position(|s| s == sphere)
            .expect("one of the spheres");
        assert_eq!(found, uncached[position]);
        found
    });
    let held = kept.0.borrow();
    assert!(held.len() > 1_000, "{}", held.len());
    for (&key, systems) in held.iter().step_by(97) {
        let made = ProductionClasses
            .visit_class(galaxy, key.class(), Made { galaxy, key })
            .expect("a registered class");
        assert_eq!(**systems, made, "{key:?}");
    }
}

/// The X-ray binaries' layers follow the binary tables' hold (P15.T10.b at 27): layer E alone under
/// the scratch tables, D and E under the fitted ones, whose progenitors reach down to 3.2 M☉; and
/// under every 27 hold the fixture's first cells hold one of layer D, the entry that layer E alone
/// left out of the census: in its first eight cells from generator version 26, whose refitted
/// tables place it in the eighth (the first three until 26).
#[test]
fn the_x_ray_binaries_layers_follow_the_binary_tables_hold() {
    use crate::galaxy::catalogue_classes::binary::BinaryTableRules;
    let e = LayerSet::EMPTY.with(Layer::E);
    let d_and_e = e.with(Layer::D);
    assert_eq!(
        member_layers(ClassId::XRAY_BINARY, BinaryTableRules::Scratch),
        Some(&[e][..])
    );
    assert_eq!(
        member_layers(ClassId::XRAY_BINARY, BinaryTableRules::Fitted),
        Some(&[d_and_e][..])
    );
    assert_eq!(
        GalaxyRules::every_27_hold().binary_tables,
        BinaryTableRules::Fitted
    );
    assert_eq!(
        GalaxyRules::of_version(crate::GENERATOR_VERSION).binary_tables,
        BinaryTableRules::Scratch
    );
    let hosts = first_hosts(at_27(), ClassId::XRAY_BINARY, 8);
    assert!(
        hosts
            .iter()
            .any(|host| host.system.record().layer() == Layer::D),
        "{:?}",
        hosts
            .iter()
            .map(|host| host.system.record().layer())
            .collect::<Vec<_>>()
    );
}

/// A class-cell cache that keeps every cell it makes, as [`KeepClassCells`], and counts what it
/// makes and what it is asked for.
#[derive(Debug, Default)]
struct CountClassCells {
    kept: KeepClassCells,
    made: std::cell::Cell<usize>,
    asked: RefCell<Vec<CatalogueCellKey>>,
}

impl CatalogueClassCellCache for CountClassCells {
    fn systems(
        &self,
        galaxy: &Galaxy,
        key: CatalogueCellKey,
        make: &mut dyn FnMut() -> ClassCellSystems,
    ) -> Arc<ClassCellSystems> {
        self.asked.borrow_mut().push(key);
        self.kept.systems(galaxy, key, &mut || {
            self.made.set(self.made.get() + 1);
            make()
        })
    }
}

/// The cells a query reads, made ahead in shares in any order (P09.T37's bench), are every cell
/// its hits read, so its lookups make none, and the answer is the one through no cache, bit for
/// bit: queries in the outer disc and about a placed host and a core collapse, at three times,
/// each class's cells read in the layers of the plan without the sources, which hold the plan's
/// with them. With no layer asked, none is read.
#[test]
fn the_cells_read_ahead_are_every_cell_the_hits_read() {
    let galaxy = at_27();
    let ly = |p: [f64; 3]| GalacticPosition::from_light_years(p).expect("in the cube");
    let core = first_hosts(galaxy, ClassId::CORE_COLLAPSE, 1)[0].system;
    let placed = first_hosts(galaxy, ClassId::ACCRETING_WHITE_DWARF, 1)[0].system;
    let queries = [
        (ly([0.0, 48_000.0, 0.0]), 50.0, 0),
        (ly([1_500.0, 47_000.0, 80.0]), 20.0, -900),
        (core.position_at(at(300)), 2.0, 300),
        (placed.position_at(UniverseTime::EPOCH), 1.0, 0),
    ]
    .map(|(centre, radius, years)| {
        RangeQuery::builder(centre, LightYears::new(radius))
            .time(at(years))
            .build()
            .expect("a query")
    });
    let classes = CatalogueClassSource::new();
    let mut hits = 0;
    for query in &queries {
        let layers = RangePlan::new(galaxy, &[], query).census().layers();
        let cells = classes.cells_read(galaxy, query, layers);
        assert_eq!(classes.cells_read(galaxy, query, LayerSet::EMPTY), []);
        let counted = CountClassCells::default();
        let shares: Vec<_> = cells.chunks(cells.len().div_ceil(7)).collect();
        for share in shares.iter().rev() {
            classes.with_cells(&counted).read_ahead(galaxy, share);
        }
        let ahead = counted.made.get();
        assert_eq!(ahead, cells.len(), "each cell made once, ahead");
        counted.asked.borrow_mut().clear();
        let cached = classes.with_cells(&counted);
        let found = range_query(galaxy, &mut NoCache::new(), &[&cached], query);
        assert_eq!(counted.made.get(), ahead, "the hits make no cell");
        let read: std::collections::BTreeSet<_> = counted.asked.borrow().iter().copied().collect();
        let held: std::collections::BTreeSet<_> = cells.iter().copied().collect();
        assert!(read.is_subset(&held));
        let uncached = range_query(galaxy, &mut NoCache::new(), &[&classes], query);
        assert_eq!(found, uncached);
        hits += found
            .systems()
            .iter()
            .filter(|hit| matches!(hit.id().kind(), SystemIdKind::Catalogue(_)))
            .count();
    }
    assert!(hits >= 2, "{hits} class hosts among the answers");
}

/// The members of class cell `key`'s entries resolved one ID at a time ([`entry_systems`]), each
/// candidate in index order, as plan 03's resolve finds them.
struct Resolved<'a> {
    galaxy: &'a Galaxy,
    key: CatalogueCellKey,
}

impl ClassVisitor for Resolved<'_> {
    type Output = Vec<CatalogueSystem>;
    fn visit<P: ClassProcess>(self, process: &P) -> Vec<CatalogueSystem> {
        let candidates = occupied_cells(self.galaxy, process, [self.key])
            .first()
            .map_or(0, |&(_, count)| count);
        (0..candidates)
            .flat_map(|index| {
                let id = self
                    .key
                    .candidate_id(index, 0)
                    .expect("an index of the cell");
                entry_systems(self.galaxy, process, id)
            })
            .collect()
    }
}

/// A cell's systems are its members' resolves, bit for bit (P09.T37's bench): each entry of the
/// cells holding the first hosts of each class that reads no place, and the cell of the first Type
/// Ia's survivor, kept under the cell's one bound, as each member's ID resolves it.
#[test]
fn a_cell_s_systems_are_its_members_resolves() {
    let galaxy = at_27();
    let mut members = 0;
    let survivor = hosts_until(galaxy, ClassId::TYPE_IA, |cell| {
        cell.iter().any(|host| host.member == 1)
    })
    .into_iter()
    .find(|host| host.member == 1)
    .expect("a survivor in the plane");
    for class in [
        ClassId::CORE_COLLAPSE,
        ClassId::TYPE_IA,
        ClassId::XRAY_BINARY,
    ] {
        let mut cells: std::collections::BTreeSet<_> = first_hosts(galaxy, class, 3)
            .iter()
            .map(|host| CatalogueCellKey::of(host.id).expect("a host's cell"))
            .collect();
        if class == ClassId::TYPE_IA {
            cells.insert(CatalogueCellKey::of(survivor.id).expect("a host's cell"));
        }
        for key in cells {
            let made = ProductionClasses
                .visit_class(galaxy, class, Made { galaxy, key })
                .expect("a registered class");
            let resolved = ProductionClasses
                .visit_class(galaxy, class, Resolved { galaxy, key })
                .expect("a registered class");
            assert!(!resolved.is_empty(), "{key:?}");
            assert_eq!(made.members(), &resolved[..], "{key:?}");
            members += resolved.len();
        }
    }
    assert!(members > 9, "{members}");
}

/// The sky's walk never leaves out a host the census keeps (rendering plan R06, R06.T16.a): a host
/// about 1,000 ly from an observer, whose line reverses at its break, at 1,000 km/s towards the
/// observer before it and away after, its break swept over a few years about its light's emission
/// and its place over the hundredth of a light-year inside the shell's outer edge, is one
/// `seen_in_shell` gives wherever the census's one fixed-point step on the light cone
/// ([`retarded`](crate::observe::retarded)) shows it inside the edge, though the step shows some
/// from before the light's window.
#[test]
fn the_sky_s_walk_keeps_every_host_the_census_shows_in_the_shell() {
    use crate::coords::GalacticVelocity;
    use crate::galaxy::Population;
    use crate::galaxy::catalogue_classes::CatalogueMotion;
    use crate::observe::{Observer, retarded};
    use crate::time::Span;
    use crate::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR};
    use crate::units::{SolarMasses, Years};

    /// The shell's outer edge, ly.
    const OUTER_LY: f64 = 1_000.0;
    /// The host's speed each side of its break, m/s.
    const SPEED: f64 = 1e6;
    let here = [0.0, 26_000.0, 0.0];
    let observer = Observer::new(
        GalacticPosition::from_light_years(here).expect("in the cube"),
        UniverseTime::EPOCH,
    )
    .expect("on the clock");
    let years = |y: f64| Span::from_seconds_f64(y * SECONDS_PER_JULIAN_YEAR).expect("a span");
    let earliest = UniverseTime::EPOCH
        .checked_sub(years(OUTER_LY))
        .expect("on the clock");
    let window = (earliest, UniverseTime::EPOCH);
    let id = CatalogueCellKey::new(ClassId::LUMINOUS_BLUE_VARIABLE, [0, 6, 0])
        .expect("a cell")
        .candidate_id(0, 0)
        .expect("an ID");
    let (mut kept, mut early) = (0, 0);
    for j in 0..100 {
        let distance = OUTER_LY - 0.01 + f64::from(j) * 1e-4;
        let point =
            GalacticPosition::from_light_years([distance, here[1], here[2]]).expect("in the cube");
        let record = SystemRecord::from_parts(
            SystemId::from(id),
            point,
            SystemOrigin::CatalogueSystem(ClassId::LUMINOUS_BLUE_VARIABLE),
            Population::YoungThinDisc,
            SolarMasses::new(40.0),
            Years::new(5e6),
        );
        for k in -40..=40 {
            let at = UniverseTime::EPOCH
                .checked_sub(years(distance + f64::from(k) * 0.1))
                .expect("on the clock");
            let motion = CatalogueMotion::broken(
                point,
                at,
                GalacticVelocity::new([-SPEED, 0.0, 0.0]),
                GalacticVelocity::new([SPEED, 0.0, 0.0]),
            );
            let system = CatalogueSystem::new(record, motion);
            let r = retarded(&observer, &motion);
            let shown = observer
                .position()
                .distance_to(r.apparent_position())
                .value()
                / METRES_PER_LIGHT_YEAR;
            if shown < OUTER_LY {
                kept += 1;
                early += usize::from(r.emitted() < earliest);
                assert!(
                    seen_in_shell(
                        &system,
                        Layer::E,
                        observer.position(),
                        window,
                        (LightYears::ZERO, LightYears::new(OUTER_LY)),
                    ),
                    "a host {distance} ly away, broken {k} tenths of a year from its light's \
                     emission, shown at {shown} ly, is left out"
                );
            }
        }
    }
    assert!(
        early > 0,
        "of {kept} hosts kept, none shown from before the window"
    );
}

/// A class host is seen where its record exists, and a further member of its entry only from its
/// entry's break (P09.T34.a's lean; [`CatalogueSystem::shown_at`]): a Type Ia's member 1 at its
/// break and not a second before it, its member 0 on both sides, and a record a century old at the
/// epoch not two centuries before it but half a century before it.
#[test]
fn a_host_is_seen_where_it_exists_and_a_further_member_from_its_break() {
    use crate::coords::GalacticVelocity;
    use crate::galaxy::Population;
    use crate::galaxy::catalogue_classes::CatalogueMotion;
    use crate::time::Span;
    use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
    use crate::units::{SolarMasses, Years};

    let years = |y: f64| Span::from_seconds_f64(y * SECONDS_PER_JULIAN_YEAR).expect("a span");
    let before_epoch = |y: f64| {
        UniverseTime::EPOCH
            .checked_sub(years(y))
            .expect("on the clock")
    };
    let key = CatalogueCellKey::new(ClassId::TYPE_IA, [0, 50, 0]).expect("a cell");
    let place = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube");
    let at = before_epoch(500.0);
    let motion = CatalogueMotion::broken(
        place,
        at,
        GalacticVelocity::new([1e5, 0.0, 0.0]),
        GalacticVelocity::new([-2e5, 0.0, 0.0]),
    );
    let host = |member: u8, age: f64| {
        let id = key.candidate_id(0, member).expect("an ID");
        CatalogueSystem::new(
            SystemRecord::from_parts(
                SystemId::from(id),
                place,
                SystemOrigin::CatalogueSystem(ClassId::TYPE_IA),
                Population::OldThinDisc,
                SolarMasses::new(1.0),
                Years::new(age),
            ),
            motion,
        )
    };
    let just_before = at
        .checked_sub(Span::from_seconds_f64(1.0).expect("a span"))
        .expect("on the clock");
    assert!(!host(1, 1e9).shown_at(just_before));
    assert!(host(1, 1e9).shown_at(at));
    assert!(host(0, 1e9).shown_at(just_before));
    let young = host(0, 100.0);
    assert!(!young.shown_at(before_epoch(200.0)));
    assert!(young.shown_at(before_epoch(50.0)));
}
