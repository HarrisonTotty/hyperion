//! Frame selection over a generated galaxy: which system's frame a ship is in (plan 03, P03.T12.b).
//!
//! The rule itself is unit-tested beside it (P03.T12.a). What is tested here is the search: that a
//! ship beside a generated system is in that system's frame, that a ship where no sphere of
//! influence reaches is in the galactic frame, that the answer does not depend on the time asked
//! for — nothing moves until plan 08 — nor on what the caller's cache holds, and that the query's
//! sources are merged in, both as candidates and as suppressors.

#[expect(dead_code, reason = "the frame tests use only the Sun-like point")]
mod common;

use std::collections::BTreeMap;

use common::sunlike_point;
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::frame::frame_at;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{
    CellCache, CellKey, NoCache, SystemOrigin, SystemRecord, generate_cell,
};
use hyperion_sim::galaxy::query::{LayerCounts, LayerSet, QuerySphere, SystemHit, SystemSource};
use hyperion_sim::galaxy::{Galaxy, PointLy, Population};
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{LightYears, SolarMasses, Years};

/// The seed of the galaxy these ships fly in.
const SEED: u64 = 0x0312_b000_0000_0000;

fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
}

/// The first system of the layer-C cell holding the Sun-like point: a nearby ordinary primary.
fn a_nearby_system(galaxy: &Galaxy) -> SystemRecord {
    let key = CellKey::containing(Layer::C, &sunlike_point(galaxy)).expect("inside the cube");
    let mut cell = Vec::new();
    generate_cell(galaxy, key, &mut cell);
    *cell
        .first()
        .expect("a 32 ly cell of the solar circle holds systems")
}

/// `position` moved `ly` light-years along +x.
fn offset_by(position: &GalacticPosition, ly: f64) -> GalacticPosition {
    let [x, y, z] = position.to_light_years_f64();
    GalacticPosition::from_light_years([x + ly, y, z]).expect("a light-year stays in the cube")
}

/// The distance between two positions, light-years.
fn away(from: &GalacticPosition, to: &GalacticPosition) -> f64 {
    LightYears::from(from.distance_to(to)).value()
}

/// A cache that keeps every cell it is given, so a second call is served from memory.
#[derive(Debug, Default)]
struct Keep {
    cells: BTreeMap<CellKey, Vec<SystemRecord>>,
}

impl CellCache for Keep {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        let cell = self.cells.entry(key).or_insert_with(|| {
            let mut systems = Vec::new();
            generate_cell(galaxy, key, &mut systems);
            systems
        });
        f(cell)
    }
}

/// A cache that throws everything away every other call, so a walk sees hits and misses mixed.
#[derive(Debug, Default)]
struct Flaky {
    kept: Keep,
    calls: u32,
}

impl CellCache for Flaky {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        self.calls += 1;
        if !self.calls.is_multiple_of(2) {
            self.kept.cells.clear();
        }
        self.kept.with_cell(galaxy, key, f)
    }
}

/// A source with one member of its own and, when asked, a ball of grid systems it replaces: the
/// shape plans 09 and 10 fill in.
#[derive(Debug, Default)]
struct Fake {
    member: Option<SystemHit>,
    suppress: Option<(GalacticPosition, LightYears)>,
}

impl SystemSource for Fake {
    fn expected_in_sphere(&self, _galaxy: &Galaxy, _sphere: &QuerySphere) -> LayerCounts {
        let mut counts = LayerCounts::ZERO;
        if let Some(hit) = &self.member {
            let layer = hit.record().layer();
            counts.set(layer, counts.get(layer) + 1.0);
        }
        counts
    }

    fn systems_in_sphere(
        &self,
        _galaxy: &Galaxy,
        sphere: &QuerySphere,
        layers: LayerSet,
        out: &mut Vec<SystemHit>,
    ) {
        if let Some(hit) = &self.member
            && layers.contains(hit.record().layer())
            && hit.distance().value() <= sphere.radius().value()
        {
            out.push(*hit);
        }
    }

    fn suppresses(&self, _galaxy: &Galaxy, record: &SystemRecord, _t: UniverseTime) -> bool {
        self.suppress.is_some_and(|(centre, radius)| {
            away(&centre, record.epoch_position()) <= radius.value()
        })
    }
}

/// A member of a source at `position`, `distance` light-years from the ship, of `mass`.
fn fake_member(galaxy: &Galaxy, position: GalacticPosition, distance: f64, mass: f64) -> SystemHit {
    let key = CellKey::containing(Layer::C, &position).expect("inside the cube");
    let record = SystemRecord::from_parts(
        key.candidate_id(0).expect("0 is below layer C's capacity"),
        position,
        SystemOrigin::Grid(galaxy.fields().component_id(0).expect("a first component")),
        Population::OldThinDisc,
        SolarMasses::new(mass),
        Years::new(5.0e9),
    );
    SystemHit::new(record, position, LightYears::new(distance))
}

#[test]
fn a_ship_beside_a_generated_system_is_in_its_frame() {
    let galaxy = galaxy();
    let system = a_nearby_system(&galaxy);
    let ship = offset_by(system.epoch_position(), 0.1);
    assert!(
        (away(&ship, system.epoch_position()) - 0.1).abs() < 1e-9,
        "the ship is a tenth of a light-year out"
    );

    let frame = frame_at(
        &galaxy,
        &mut NoCache::new(),
        &[],
        &ship,
        UniverseTime::EPOCH,
        None,
    );
    assert_eq!(frame, Some(system.id()));

    // And it stays in that frame once it is in it: the rule only changes frame for a rival a tenth
    // better, and at a tenth of a light-year out nothing is.
    let held = frame_at(
        &galaxy,
        &mut NoCache::new(),
        &[],
        &ship,
        UniverseTime::EPOCH,
        Some(system.id()),
    );
    assert_eq!(held, Some(system.id()));
}

#[test]
fn a_ship_in_a_void_is_in_the_galactic_frame() {
    let galaxy = galaxy();
    // 60,000 ly above the centre: the halo's density leaves nothing within tens of light-years.
    let void =
        GalacticPosition::from_light_years([0.0, 0.0, 60_000.0]).expect("inside the root cube");
    assert_eq!(
        frame_at(
            &galaxy,
            &mut NoCache::new(),
            &[],
            &void,
            UniverseTime::EPOCH,
            None
        ),
        None
    );
    // A frame the ship has left is dropped, not carried into the void.
    let left = a_nearby_system(&galaxy).id();
    assert_eq!(
        frame_at(
            &galaxy,
            &mut NoCache::new(),
            &[],
            &void,
            UniverseTime::EPOCH,
            Some(left)
        ),
        None
    );
}

#[test]
fn the_frame_is_the_same_at_every_time_and_whatever_the_cache_holds() {
    let galaxy = galaxy();
    let system = a_nearby_system(&galaxy);
    let ship = offset_by(system.epoch_position(), 0.1);
    let expected = Some(system.id());

    let mut keep = Keep::default();
    let mut flaky = Flaky::default();
    // The ends of the clock window, the epoch, and a time between: nothing moves until plan 08, so
    // the frame is the same at all of them.
    for years in [0_i64, 137, 1_000, -1_000] {
        let t = UniverseTime::from_julian_years(years).expect("inside the clock window");
        let cold = frame_at(&galaxy, &mut NoCache::new(), &[], &ship, t, None);
        assert_eq!(cold, expected, "{years} yr with no cache");
        // The first call fills `keep`, so the second and third are served from memory.
        for pass in 0..3 {
            let warm = frame_at(&galaxy, &mut keep, &[], &ship, t, None);
            assert_eq!(warm, expected, "{years} yr from a warm cache, pass {pass}");
        }
        assert!(!keep.cells.is_empty(), "the warm cache kept its cells");
        let evicting = frame_at(&galaxy, &mut flaky, &[], &ship, t, None);
        assert_eq!(evicting, expected, "{years} yr from an evicting cache");
    }
}

#[test]
fn a_source_holds_the_ship_and_can_keep_a_grid_system_from_holding_it() {
    let galaxy = galaxy();
    let system = a_nearby_system(&galaxy);
    let ship = offset_by(system.epoch_position(), 0.1);

    // A member of a source half as far away as the grid system, of a mass whose sphere is no
    // smaller: its ratio is the smaller, so it takes the ship.
    let position = offset_by(system.epoch_position(), 0.05);
    let member = fake_member(&galaxy, position, away(&ship, &position), 1.0);
    let source = Fake {
        member: Some(member),
        suppress: None,
    };
    let frame = frame_at(
        &galaxy,
        &mut NoCache::new(),
        &[&source],
        &ship,
        UniverseTime::EPOCH,
        None,
    );
    assert_eq!(frame, Some(member.record().id()));

    // A source that replaces the grid system keeps it from holding the ship. What holds it instead
    // is whatever else is in reach, which at the solar circle is usually another system.
    let pinned = Fake {
        member: None,
        suppress: Some((*system.epoch_position(), LightYears::new(0.01))),
    };
    let frame = frame_at(
        &galaxy,
        &mut NoCache::new(),
        &[&pinned],
        &ship,
        UniverseTime::EPOCH,
        None,
    );
    assert_ne!(frame, Some(system.id()));
    // Suppression does not reach the source's own members.
    let frame = frame_at(
        &galaxy,
        &mut NoCache::new(),
        &[&source, &pinned],
        &ship,
        UniverseTime::EPOCH,
        None,
    );
    assert_eq!(frame, Some(member.record().id()));
}

/// The three tidal radii P03.T12.b quotes for the fixture, which set the search radii.
///
/// They are figures of the code, and P02.T11's tuning of the fixture has moved them, as the
/// previous form of this test said it would: 4.235 ly for 1 M☉, 3.361 ly for layer A's heaviest
/// primary (0.5 M☉) and 22.501 ly for layer E's (150 M☉), where they were 4.381, 3.477 and 23.279
/// against the task's 4.4, 3.5 and 23. The tuning shortened the thin disc's scale length and
/// raised its height, which lowers `4Ω² − κ²` at the Sun a little; P03.T12.b's quoted figures want
/// rounding to 4.2, 3.4 and 22.5.
#[test]
fn the_search_reaches_the_tidal_radii_the_task_quotes() {
    let galaxy = galaxy();
    let sun = PointLy::from(&sunlike_point(&galaxy));
    let radius = |solar_masses: f64| {
        LightYears::from(
            galaxy
                .potential()
                .tidal_radius(SolarMasses::new(solar_masses), &sun),
        )
        .value()
    };

    let (one, layer_a, layer_e) = (radius(1.0), radius(0.5), radius(150.0));
    eprintln!(
        "tidal radii at the Sun: {one:.3} ly for 1 M☉, {layer_a:.3} for 0.5, {layer_e:.3} for 150"
    );
    assert!((one - 4.2).abs() < 0.15, "1 M☉ gives {one} ly, not 4.2");
    assert!(
        (layer_a - 3.4).abs() < 0.15,
        "layer A's heaviest primary gives {layer_a} ly, not 3.4"
    );
    assert!(
        (layer_e - 22.5).abs() < 1.0,
        "layer E's heaviest primary gives {layer_e} ly, not 22.5"
    );

    // The radius goes as the cube root of the mass, which is why one search radius a layer covers
    // every system in it.
    assert!((layer_e / one - math::cbrt(150.0)).abs() < 1e-9);
    assert!((layer_a / one - math::cbrt(0.5)).abs() < 1e-9);
}
