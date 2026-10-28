//! The range query at the galactic centre (plan 09, P09.T29): the centre's members as a source of
//! the range query, padded by the radial plunge and scanned in full near the black hole, against a
//! brute-force scan of the inner 2 ly; the full-scan radius at a century; the source merged into
//! the query; the galaxy's held centre; and the pad P09.T28.b.b's Kepler-regime grid records will
//! need.
//!
//! The members are additions that nothing generated reads (P09.T37 registers the source), so
//! these tests pin behaviour, not output.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::PointLy;
use hyperion_sim::galaxy::features::centre::members::CentrePlacement;
use hyperion_sim::galaxy::features::centre::{
    CentreMemberSource, CentreModel, CentreTracer, KeepCentreOrbits, NoCentreOrbitCache,
};
use hyperion_sim::galaxy::imf::{MassBand, MassFunctionKind};
use hyperion_sim::galaxy::motion::{
    EpochState, Regime, StateAt, full_scan_radius, position_velocity_at, regime_of,
};
use hyperion_sim::galaxy::params::{GalaxyParams, GalaxyParamsBuilder};
use hyperion_sim::galaxy::placement::{
    CellCache, CellKey, Existence, SystemOrigin, SystemRecord, generate_cell,
};
use hyperion_sim::galaxy::query::{
    LayerCounts, LayerSet, PAD_SPEED, QuerySphere, RangeQuery, SystemHit, SystemSource,
    cells_in_sphere, kepler_pad_for, pad_for, range_query,
};
use hyperion_sim::id::Layer;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR};
use hyperion_sim::units::{GravitationalParameter, LightYears, Seconds};
use hyperion_testkit::order::assert_order_independent;

/// The Milky Way fixture without its kinematic tables: the centre's members carry their own
/// velocities, and its grid systems stay put.
fn milky_way() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(
            Seed::new(0x0929_0000_0000_0001),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

fn years(y: i64) -> UniverseTime {
    UniverseTime::from_julian_years(y).expect("inside the clock window")
}

fn at(ly: [f64; 3]) -> GalacticPosition {
    GalacticPosition::from_light_years(ly).expect("near the centre")
}

/// The distance of `p` from the black hole, ly.
fn radius_of(p: &GalacticPosition) -> f64 {
    p.distance_to(&GalacticPosition::ORIGIN).value() / METRES_PER_LIGHT_YEAR
}

/// The stellar layers.
fn stellar() -> LayerSet {
    [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .fold(LayerSet::EMPTY, LayerSet::with)
}

/// Every centre member with an epoch position within `radius` ly of the black hole, and the black
/// hole: the brute force, independent of the source's choice of cells.
fn members_within(galaxy: &Galaxy, centre: &CentrePlacement<'_>, radius: f64) -> Vec<EpochState> {
    let mut all = vec![EpochState::of_centre_member(&centre.black_hole())];
    for cell in centre
        .grid()
        .cells_touching(&PointLy::default(), LightYears::new(radius))
    {
        for band in MassBand::ALL {
            all.extend(
                centre
                    .members_in_cell(galaxy, band, cell)
                    .iter()
                    .filter(|m| radius_of(m.record().epoch_position()) <= radius)
                    .map(EpochState::of_centre_member),
            );
        }
    }
    all
}

/// The brute force's hits in `sphere`: every member of `members` that exists at the sphere's
/// time, propagated there by its own law and inside the radius, nearest first.
fn scan(galaxy: &Galaxy, members: &[EpochState], sphere: &QuerySphere) -> Vec<SystemHit> {
    let t = sphere.time();
    let mut hits: Vec<SystemHit> = members
        .iter()
        .filter(|m| matches!(m.record().existence_at(t), Existence::Exists))
        .filter_map(|m| match position_velocity_at(galaxy, m, t) {
            StateAt::Exists(state) => {
                let distance = LightYears::from(sphere.centre().distance_to(state.position()));
                (distance.value() <= sphere.radius().value())
                    .then(|| SystemHit::new(*m.record(), *state.position(), distance))
            }
            StateAt::NoSuchSystem => None,
        })
        .collect();
    sort(&mut hits);
    hits
}

/// Nearest first, then by ID: the range query's order.
fn sort(hits: &mut [SystemHit]) {
    hits.sort_by(|a, b| {
        a.distance()
            .total_cmp(&b.distance())
            .then_with(|| a.id().raw().cmp(&b.id().raw()))
    });
}

/// The source's hits in `sphere` over the stellar layers, nearest first.
fn source_hits(source: &CentreMemberSource<'_>, sphere: &QuerySphere) -> Vec<SystemHit> {
    let mut hits = Vec::new();
    source.systems_in_sphere(milky_way(), sphere, stellar(), &mut hits);
    sort(&mut hits);
    hits
}

/// One brute-force comparison: every sphere of `spheres` (centre and radius, ly) at each of
/// `times` (Julian years), queried through the source and found by scanning every member within
/// `scan_radius` ly of the black hole. The caller chooses the spheres so close to the black hole
/// that no member from beyond `scan_radius` can arrive by those times. Returns how many hits had
/// started outside their sphere, which only the pads find.
fn compare_with_a_scan(
    source: &CentreMemberSource<'_>,
    spheres: &[([f64; 3], f64)],
    times: &[i64],
    scan_radius: f64,
) -> usize {
    let galaxy = milky_way();
    let members = members_within(galaxy, source.placement(), scan_radius);
    println!(
        "{} members within {scan_radius} ly (the black hole included)",
        members.len()
    );
    let mut moved_in = 0_usize;
    for &years_from_epoch in times {
        for &(centre, radius) in spheres {
            let sphere = QuerySphere::new(
                at(centre),
                LightYears::new(radius),
                years(years_from_epoch),
                LightYears::ZERO,
            )
            .expect("a positive radius");
            let found = source_hits(source, &sphere);
            let want = scan(galaxy, &members, &sphere);
            for hit in &found {
                assert!(
                    radius_of(hit.record().epoch_position()) <= scan_radius,
                    "{:?} starts beyond the brute force's {scan_radius} ly",
                    hit.id()
                );
            }
            assert_eq!(
                found.len(),
                want.len(),
                "{radius} ly about {centre:?} at {years_from_epoch} yr"
            );
            assert_eq!(
                found, want,
                "{radius} ly about {centre:?} at {years_from_epoch} yr"
            );
            moved_in += found
                .iter()
                .filter(|h| {
                    LightYears::from(sphere.centre().distance_to(h.record().epoch_position()))
                        .value()
                        > radius
                })
                .count();
            println!(
                "{years_from_epoch:+} yr, {radius} ly about {centre:?}: {} hits",
                found.len()
            );
        }
    }
    moved_in
}

/// P09.T29's test in the fast suite's size: queries a decade either way, at the black hole and
/// beside it, equal a brute-force scan of the inner 0.2 ly, whatever the cache and on a second
/// build of the galaxy; and the pads matter, since members move in from outside the spheres. The
/// plan's own size, a century and the inner 2 ly, is the slow test below.
#[test]
fn a_query_a_decade_either_way_finds_what_a_scan_of_the_inner_fifth_of_a_light_year_finds() {
    let galaxy = milky_way();
    let kept = KeepCentreOrbits::new(galaxy);
    let source = CentreMemberSource::new(galaxy, &kept).expect("the fixture's centre builds");
    // Within 0.1 ly of the black hole, which a member from beyond 0.2 ly would need 3,000 km/s
    // to reach within a decade, twice what the centre allows there.
    let spheres = [
        ([0.0, 0.0, 0.0], 0.02),
        ([0.05, -0.03, 0.02], 0.03),
        ([-0.02, 0.01, 0.06], 0.04),
    ];
    let moved_in = compare_with_a_scan(&source, &spheres, &[-10, 10], 0.2);
    assert!(moved_in > 0);
    println!(
        "{moved_in} hits moved in from outside their sphere; {} members cached",
        kept.members()
    );

    // Order independence behind the cache (the sim-determinism skill): each sphere's hits and
    // census are the same whatever was asked before, warm, and from a cold cache on every call.
    let query = |source: &CentreMemberSource<'_>, &(centre, radius, y): &([f64; 3], f64, i64)| {
        let sphere = QuerySphere::new(
            at(centre),
            LightYears::new(radius),
            years(y),
            LightYears::ZERO,
        )
        .expect("a positive radius");
        (
            source_hits(source, &sphere),
            source.expected_in_sphere(galaxy, &sphere),
        )
    };
    let warm: Vec<([f64; 3], f64, i64)> = [-10, 10]
        .into_iter()
        .flat_map(|y| spheres.map(|(c, r)| (c, r, y)))
        .collect();
    assert_order_independent(&warm, |key| query(&source, key));
    let small = [
        ([0.0, 0.0, 0.0], 0.01, -1),
        ([0.0, 0.0, 0.0], 0.01, 1),
        ([0.05, -0.03, 0.02], 0.02, 1),
        ([0.05, -0.03, 0.02], 0.02, 0),
    ];
    assert_order_independent(&small, |key| {
        let cold = CentreMemberSource::new(galaxy, &NoCentreOrbitCache).expect("the centre builds");
        query(&cold, key)
    });
    assert_order_independent(&small, |key| query(&source, key));

    // The cache changes nothing, and neither does a second build of the galaxy.
    let twin = Galaxy::from_params(
        Seed::new(0x0929_0000_0000_0001),
        GalaxyParams::milky_way_like(),
    )
    .expect("the fixture builds");
    let fresh = CentreMemberSource::new(&twin, &NoCentreOrbitCache).expect("the centre builds");
    let sphere = QuerySphere::new(
        at([0.02, 0.0, -0.01]),
        LightYears::new(0.03),
        years(-10),
        LightYears::ZERO,
    )
    .expect("a positive radius");
    let mut uncached = Vec::new();
    fresh.systems_in_sphere(&twin, &sphere, stellar(), &mut uncached);
    sort(&mut uncached);
    assert_eq!(uncached, source_hits(&source, &sphere));
    assert_ne!(uncached, []);
}

/// P09.T29's test as the plan sets it: queries a century either way equal a brute-force scan of
/// the inner 2 ly, at the black hole, small and large, and off it, all within 1.4 ly of it, where
/// no member from beyond 2 ly can arrive within a century (its speed would have to pass 1,800
/// km/s, three times what the centre allows there). It places some 1.8 million members, most of
/// the five minutes it takes.
#[test]
#[ignore = "slow: places some 1.8 million members, about five minutes"]
fn a_query_a_century_either_way_finds_what_a_scan_of_the_inner_2_ly_finds() {
    let galaxy = milky_way();
    let kept = KeepCentreOrbits::new(galaxy);
    let source = CentreMemberSource::new(galaxy, &kept).expect("the fixture's centre builds");
    let spheres = [
        ([0.0, 0.0, 0.0], 0.05),
        ([0.0, 0.0, 0.0], 0.3),
        ([0.0, 0.0, 0.0], 1.0),
        ([0.4, -0.3, 0.2], 0.25),
        ([1.0, 0.5, -0.2], 0.3),
    ];
    let moved_in = compare_with_a_scan(&source, &spheres, &[-100, 100], 2.0);
    assert!(moved_in > 0);
    println!(
        "{moved_in} hits moved in from outside their sphere; {} members cached",
        kept.members()
    );
}

/// The census: each band's expected candidates over the cells touching the unpadded sphere, the
/// same at any time and whatever is cached, above what the sphere holds at the epoch, with the
/// black hole as one system of layer E when the sphere holds it.
#[test]
fn the_census_counts_the_cells_touching_the_sphere_and_the_black_hole() {
    let galaxy = milky_way();
    let kept = KeepCentreOrbits::new(galaxy);
    let source = CentreMemberSource::new(galaxy, &kept).expect("the fixture's centre builds");
    let sphere = |centre, radius, t| {
        QuerySphere::new(at(centre), LightYears::new(radius), t, LightYears::ZERO)
            .expect("a positive radius")
    };
    let now = sphere([0.0; 3], 0.05, UniverseTime::EPOCH);
    let counts = source.expected_in_sphere(galaxy, &now);
    let at_700_yr = source.expected_in_sphere(galaxy, &sphere([0.0; 3], 0.05, years(700)));
    assert_eq!(counts, at_700_yr);
    assert_eq!(
        counts,
        CentreMemberSource::new(galaxy, &NoCentreOrbitCache)
            .unwrap()
            .expected_in_sphere(galaxy, &now)
    );
    let mut hits = Vec::new();
    source.systems_in_sphere(galaxy, &now, stellar(), &mut hits);
    let mut held = LayerCounts::ZERO;
    for hit in &hits {
        let layer = hit.record().layer();
        held.set(layer, held.get(layer) + 1.0);
    }
    for layer in Layer::ALL {
        println!(
            "layer {layer:?}: {} expected, {} held",
            counts.get(layer),
            held.get(layer)
        );
        assert!(counts.get(layer) >= held.get(layer), "{layer:?}");
    }
    // The black hole: one system of layer E, at rest at the origin.
    let black_hole = source.placement().black_hole();
    assert_eq!(black_hole.record().layer(), Layer::E);
    assert!(hits.iter().any(|h| h.id() == black_hole.record().id()));
    // Layer E's count is its cells' candidates, and one more where the sphere holds the black
    // hole.
    let cells_e = |s: &QuerySphere| {
        let centre = PointLy::from(s.centre());
        source
            .placement()
            .grid()
            .cells_touching(&centre, s.radius())
            .fold(0.0, |sum, cell| {
                sum + source.placement().expected_candidates(MassBand::E, cell)
            })
    };
    let away = sphere([3.0, 0.0, 0.0], 0.01, UniverseTime::EPOCH);
    assert_eq!(
        source.expected_in_sphere(galaxy, &away).get(Layer::E),
        cells_e(&away)
    );
    let close = sphere([1e-3, 0.0, 0.0], 0.01, UniverseTime::EPOCH);
    assert_eq!(
        source.expected_in_sphere(galaxy, &close).get(Layer::E),
        cells_e(&close) + 1.0
    );
    let mut none = Vec::new();
    source.systems_in_sphere(galaxy, &now, LayerSet::EMPTY, &mut none);
    assert_eq!(none, []);
}

/// The plan's figures at a century: `r_full` of 0.31 ly about the black hole (Sgr A*'s mass), and
/// the systems inside it, "about 37,000" (brainstorm, "The range query") and "42,000 under
/// Kroupa's" (plan 09, P09.T29), from the centre's own classes (P09.T29 as built).
#[test]
fn the_full_scan_radius_and_the_systems_inside_it_at_a_century() {
    let century = Seconds::new(100.0 * SECONDS_PER_JULIAN_YEAR);
    for kind in [MassFunctionKind::default(), MassFunctionKind::Kroupa] {
        let params = GalaxyParamsBuilder::new()
            .mass_function(kind)
            .build()
            .expect("Milky Way-like parameters");
        let model = CentreModel::from_params(&params).expect("the fixture's centre builds");
        let black_hole = model.profile().black_hole();
        let r_full = full_scan_radius(
            GravitationalParameter::from_solar_masses(black_hole),
            century,
        );
        assert!(
            (r_full.value() - 0.31).abs() < 0.005,
            "{kind:?}: r_full {} ly",
            r_full.value()
        );
        let mut by_tracer = [0.0; 4];
        for c in model.classes().classes() {
            by_tracer[c.tracer.index()] +=
                c.expected * model.distribution(c.tracer).fraction_within(r_full.value());
        }
        let inside: f64 = by_tracer.iter().sum();
        println!(
            "{kind:?}: r_full {:.4} ly, {inside:.0} systems inside: {:?} by tracer {:?}",
            r_full.value(),
            by_tracer.map(f64::round),
            CentreTracer::ALL
        );
        // Measured 62,443 and 75,285 against the plan's 37,000 and 42,000, a recorded miss
        // (P09.T29 as built, with the split by tracer), pinned here.
        let want = match kind {
            MassFunctionKind::Chabrier => 62_443.0,
            MassFunctionKind::Kroupa => 75_285.0,
        };
        assert!(
            (inside / want - 1.0).abs() < 0.01,
            "{kind:?}: {inside} systems"
        );
    }
}

/// The galaxy holds its centre: [`Galaxy::centre`] is [`CentreModel::new`] bit for bit, built on
/// first read, and reading it changes neither the galaxy's equality nor its `Debug` text, only
/// what it holds on the heap.
#[test]
fn the_galaxy_holds_its_centre() {
    let galaxy = Galaxy::from_params(
        Seed::new(0x0929_0000_0000_0002),
        GalaxyParams::milky_way_like(),
    )
    .expect("the fixture builds");
    let cold = galaxy.clone();
    let (debug, bytes) = (format!("{galaxy:?}"), galaxy.heap_bytes());
    let held = galaxy.centre().expect("the centre builds");
    assert_eq!(held, &CentreModel::new(&cold).expect("the centre builds"));
    assert!(std::ptr::eq(held, galaxy.centre().unwrap()));
    assert_eq!(galaxy, cold);
    assert_eq!(format!("{galaxy:?}"), debug);
    let more = galaxy.heap_bytes() - bytes;
    println!("the centre's model and motion add {more} bytes");
    assert!((200_000..2_000_000).contains(&more), "{more} bytes");
}

/// A cell cache that keeps every cell of the one galaxy it serves, for a test that queries it twice.
#[derive(Default)]
struct KeepCells {
    kept: BTreeMap<CellKey, Vec<SystemRecord>>,
}

impl CellCache for KeepCells {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        f(self.kept.entry(key).or_insert_with(|| {
            let mut systems = Vec::new();
            generate_cell(galaxy, key, &mut systems);
            systems
        }))
    }
}

/// The source merged into the range query: the census adds its counts to the grid's, layer by
/// layer, and the systems are the grid's and the source's in the admitted layers.
#[test]
fn a_centre_source_merges_into_the_range_query() {
    let galaxy = milky_way();
    let kept = KeepCentreOrbits::new(galaxy);
    let source = CentreMemberSource::new(galaxy, &kept).expect("the fixture's centre builds");
    // Off the origin, which is a corner of eight cells of every layer, so that the query visits
    // one cell a layer: layer E's eight about the centre hold some 1.7 million records.
    let query = RangeQuery::builder(at([0.5, 0.3, 0.2]), LightYears::new(0.05))
        .time(years(10))
        .build()
        .expect("a valid query");
    let mut cells = KeepCells::default();
    let grid = range_query(galaxy, &mut cells, &[], &query);
    let merged = range_query(galaxy, &mut cells, &[&source], &query);
    let sphere = QuerySphere::new(
        *query.centre(),
        query.radius(),
        query.time(),
        LightYears::ZERO,
    )
    .expect("a positive radius");
    let counts = source.expected_in_sphere(galaxy, &sphere);
    for layer in Layer::ALL {
        let sum = grid.census().expected().get(layer) + counts.get(layer);
        assert!(
            (merged.census().expected().get(layer) - sum).abs() <= 1e-9 * sum.max(1.0),
            "{layer:?}"
        );
    }
    let layers = merged.census().layers();
    let mut want: Vec<SystemHit> = grid
        .systems()
        .iter()
        .filter(|h| layers.contains(h.record().layer()))
        .copied()
        .collect();
    source.systems_in_sphere(galaxy, &sphere, layers, &mut want);
    sort(&mut want);
    assert_eq!(merged.systems(), want.as_slice());
    let from_centre = merged
        .systems()
        .iter()
        .filter(|h| matches!(h.record().origin(), SystemOrigin::CentreMember { .. }))
        .count();
    println!(
        "{} systems, {from_centre} of them the centre's, complete down to {:?}",
        merged.systems().len(),
        merged.census().complete_down_to()
    );
    assert!(merged.systems().len() > grid.systems().len());
}

/// For P09.T28.b.b: once a Kepler-regime grid record follows its orbit, [`kepler_pad_for`] still
/// holds it. Every layer-A and layer-D record of the fixture with its kinematic tables inside the
/// influence radius, propagated to ±100 yr and ±1,000 yr, lies inside a sphere of 0.01 ly about
/// where it then is, the sphere that asks the most of the pad, and inside spheres about the black
/// hole and beside it; each started within the sphere's radius plus the pad.
#[test]
fn the_kepler_pad_holds_every_kepler_regime_grid_record() {
    let galaxy = Galaxy::from_params(
        Seed::new(0x0928_b0a0_0000_0001),
        GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture builds")
    .with_full_potential();
    let edge = galaxy.centre_motion().influence_radius().value();
    let spheres = [
        ([0.0, 0.0, 0.0], 0.5),
        ([0.0, 0.0, 0.0], 2.0),
        ([1.5, -0.5, 0.3], 1.0),
        ([3.0, 2.0, -1.0], 0.8),
        ([-6.0, 1.0, 0.5], 1.5),
    ];
    let mut kepler = 0_u64;
    let mut farthest = 0.0_f64;
    let mut beyond_straight = 0_u64;
    for layer in [Layer::A, Layer::D] {
        let near = grid_records_within(&galaxy, layer, edge);
        for record in near
            .iter()
            .filter(|r| regime_of(&galaxy, r) == Regime::Kepler)
        {
            kepler += 1;
            let start = EpochState::of_grid_record(&galaxy, record);
            for y in [-1_000, -100, 100, 1_000] {
                let t = years(y);
                let state = match position_velocity_at(&galaxy, &start, t) {
                    StateAt::Exists(state) => state,
                    StateAt::NoSuchSystem => continue,
                };
                let moved =
                    LightYears::from(state.position().distance_to(record.epoch_position())).value();
                farthest = farthest.max(moved);
                if moved > pad_for(t, PAD_SPEED).value() {
                    beyond_straight += 1;
                }
                let own = (*state.position(), 0.01);
                let others = spheres.iter().map(|&(c, r)| (at(c), r));
                for (c, radius) in std::iter::once(own).chain(others) {
                    if LightYears::from(c.distance_to(state.position())).value() > radius {
                        continue;
                    }
                    let from = LightYears::from(c.distance_to(record.epoch_position())).value();
                    let pad = kepler_pad_for(&galaxy, layer, &c, LightYears::new(radius), t);
                    assert!(
                        from <= radius + pad.value(),
                        "{:?} of layer {layer:?} at {y} yr: {from} ly from {c:?}, beyond \
                         {radius} + {} ly",
                        record.id(),
                        pad.value()
                    );
                }
            }
        }
    }
    println!(
        "{kepler} Kepler-regime records; the farthest moved {farthest:.3} ly, and {beyond_straight} \
         record-times moved farther than the straight lines' pad"
    );
    assert!(kepler > 1_000);
    for y in [100, 1_000] {
        let c = GalacticPosition::ORIGIN;
        println!(
            "{y} yr: the pad at the black hole for 1 ly is {:.3} ly (straight {:.3})",
            kepler_pad_for(&galaxy, Layer::A, &c, LightYears::new(1.0), years(y)).value(),
            pad_for(years(y), PAD_SPEED).value()
        );
    }
}

/// Every grid record of `layer` with its epoch position within `radius` ly of the black hole.
fn grid_records_within(galaxy: &Galaxy, layer: Layer, radius: f64) -> Vec<SystemRecord> {
    let sphere = QuerySphere::new(
        GalacticPosition::ORIGIN,
        LightYears::new(radius),
        UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .expect("a sphere at the centre");
    let (mut cell, mut near) = (Vec::new(), Vec::new());
    for key in cells_in_sphere(layer, &sphere) {
        generate_cell(galaxy, key, &mut cell);
        near.extend(
            cell.iter()
                .copied()
                .filter(|r| radius_of(r.epoch_position()) <= radius),
        );
    }
    let unique: BTreeSet<u64> = near.iter().map(|r| r.id().raw()).collect();
    assert_eq!(unique.len(), near.len());
    near
}
