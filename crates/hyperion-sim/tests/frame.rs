//! Frame selection over a generated galaxy: which system's frame a ship is in (plan 03, P03.T12.b).
//!
//! The rule itself is unit-tested beside it (P03.T12.a). What is tested here is the search: that a
//! ship beside a generated system is in that system's frame, that a ship where no sphere of
//! influence reaches is in the galactic frame, that the answer does not depend on the time asked
//! for — nothing moves until plan 08 — nor on what the caller's cache holds, that the query's
//! sources are merged in, both as candidates and as suppressors, that the search finds what a
//! brute-force search over every nearby system finds, and that a time outside the clock window is
//! refused. The frames the search picks for the brute-force test's ships are pinned in
//! `golden/frame/frames.golden` (ruling 24 of 2026-09-22).

#[expect(dead_code, reason = "the frame tests use only the Sun-like point")]
mod common;

use std::collections::BTreeMap;

use common::sunlike_point;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::frame::{FindFrameError, FrameCandidate, frame_at, select_frame};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{
    CellCache, CellKey, NoCache, SystemOrigin, SystemRecord, generate_cell,
};
use hyperion_sim::galaxy::query::{
    LayerCounts, LayerSet, QuerySphere, SystemHit, SystemSource, position_at,
};
use hyperion_sim::galaxy::{Galaxy, PointLy, Population};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::math;
use hyperion_sim::time::{ClockWindow, SourceHorizon, UniverseTime};
use hyperion_sim::units::{LightYears, SolarMasses, Years};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;

/// The seed of the galaxy these ships fly in.
const SEED: u64 = 0x0312_b000_0000_0000;

fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
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
    )
    .expect("a time inside the clock window");
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
    )
    .expect("a time inside the clock window");
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
        )
        .expect("a time inside the clock window"),
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
        )
        .expect("a time inside the clock window"),
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
        let cold = frame_at(&galaxy, &mut NoCache::new(), &[], &ship, t, None)
            .expect("a time inside the clock window");
        assert_eq!(cold, expected, "{years} yr with no cache");
        // The first call fills `keep`, so the second and third are served from memory.
        for pass in 0..3 {
            let warm = frame_at(&galaxy, &mut keep, &[], &ship, t, None)
                .expect("a time inside the clock window");
            assert_eq!(warm, expected, "{years} yr from a warm cache, pass {pass}");
        }
        assert!(!keep.cells.is_empty(), "the warm cache kept its cells");
        let evicting = frame_at(&galaxy, &mut flaky, &[], &ship, t, None)
            .expect("a time inside the clock window");
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
    )
    .expect("a time inside the clock window");
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
    )
    .expect("a time inside the clock window");
    assert_ne!(frame, Some(system.id()));
    // Suppression does not reach the source's own members.
    let frame = frame_at(
        &galaxy,
        &mut NoCache::new(),
        &[&source, &pinned],
        &ship,
        UniverseTime::EPOCH,
        None,
    )
    .expect("a time inside the clock window");
    assert_eq!(frame, Some(member.record().id()));
}

/// The three tidal radii P03.T12.b quotes for the fixture, which set the search radii.
///
/// They are figures of the code, and the fixture's tuning moves them: P02.T11's shortened thin
/// disc took them to 4.235, 3.361 and 22.501 ly for 1 M☉, layer A's heaviest primary (0.5 M☉)
/// and layer E's (150 M☉), from 4.381, 3.477 and 23.279, and P02.T12's lighter, holed fixture
/// brings them to 4.438, 3.523 and 23.582, the task's 4.4, 3.5 and 23 again (plan 02, P02.T12.d).
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
    assert!((one - 4.4).abs() < 0.15, "1 M☉ gives {one} ly, not 4.4");
    assert!(
        (layer_a - 3.5).abs() < 0.15,
        "layer A's heaviest primary gives {layer_a} ly, not 3.5"
    );
    assert!(
        (layer_e - 23.0).abs() < 1.0,
        "layer E's heaviest primary gives {layer_e} ly, not 23"
    );

    // The radius goes as the cube root of the mass, which is why one search radius a layer covers
    // every system in it.
    assert!((layer_e / one - math::cbrt(150.0)).abs() < 1e-9);
    assert!((layer_a / one - math::cbrt(0.5)).abs() < 1e-9);
}

/// The frame by brute force: every system of every layer within three times the largest sphere of
/// influence at the ship, each with its tidal radius at its own position, through `select_frame`.
///
/// It shares with `frame_at` only the rule and the tidal radius. Its reach is fixed and generous
/// where `frame_at` walks each layer over 1.25 times that layer's own largest sphere, so a search
/// that is too narrow — a margin of 0.5 instead of 1.25 passed every other test here when the code
/// was perturbed in validation — shows as a ship this finds held and `frame_at` does not.
fn brute_force_frame(
    galaxy: &Galaxy,
    cache: &mut Keep,
    ship: &GalacticPosition,
    t: UniverseTime,
    current: Option<SystemId>,
) -> Option<SystemId> {
    let largest = LightYears::from(
        galaxy
            .potential()
            .tidal_radius(SolarMasses::new(150.0), &PointLy::from(ship)),
    )
    .value();
    let reach = whole_ly(3.0 * largest) + 1;
    let centre = ship.cell().to_array().map(i64::from);
    let mut candidates = Vec::new();
    for layer in [Layer::E, Layer::D, Layer::C, Layer::B, Layer::A] {
        let size = i64::from(layer.cell_size_ly());
        let span = |axis: usize| {
            (centre[axis] - reach).div_euclid(size)..=(centre[axis] + reach).div_euclid(size)
        };
        for x in span(0) {
            for y in span(1) {
                for z in span(2) {
                    let key = [x, y, z].map(|c| i32::try_from(c).expect("a cell near the ship"));
                    let key = CellKey::new(layer, key).expect("a cell inside the cube");
                    cache.with_cell(galaxy, key, |cell| {
                        for record in cell {
                            if record.age_at(t).value() <= 0.0 {
                                continue;
                            }
                            let position = position_at(galaxy, record, t);
                            let tidal_radius = galaxy.potential().tidal_radius(
                                record.primary_initial_mass(),
                                &PointLy::from(&position),
                            );
                            if let Ok(candidate) = FrameCandidate::new(
                                record.id(),
                                ship.distance_to(&position),
                                tidal_radius,
                            ) {
                                candidates.push(candidate);
                            }
                        }
                    });
                }
            }
        }
    }
    select_frame(&candidates, current)
}

/// Whole light-years covering a small non-negative length.
fn whole_ly(ly: f64) -> i64 {
    assert!((0.0..1.0e6).contains(&ly), "{ly} ly is no search reach");
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the assertion holds the value under 10⁶, exact in i64"
    )]
    let whole = ly.ceil() as i64;
    whole
}

/// `frame_at` against [`brute_force_frame`] for each ship at the epoch and at the end of the clock
/// window; returns how many of the answers named a system.
///
/// Both searches read cells through one cache the ships share, since a layer-E cell at the centre
/// costs tens of seconds to generate and the answer does not depend on the cache (tested above).
fn assert_frames_match_brute_force(galaxy: &Galaxy, ships: &[[f64; 3]]) -> usize {
    let mut cache = Keep::default();
    let mut held = 0;
    for ship in ships {
        let at = GalacticPosition::from_light_years(*ship).expect("inside the cube");
        for t in [UniverseTime::EPOCH, ClockWindow::END] {
            let found = frame_at(galaxy, &mut cache, &[], &at, t, None)
                .expect("a time inside the clock window");
            assert_eq!(
                found,
                brute_force_frame(galaxy, &mut cache, &at, t, None),
                "a ship at {ship:?} ly, t = {t}"
            );
            held += usize::from(found.is_some());
        }
    }
    held
}

/// Ships up to six light-years out on each axis from the first `systems` systems of the layer-C
/// cell at `at`: inside some spheres of influence and outside others, since at the solar circle a
/// layer-A primary's is about 3.5 ly and a layer-E one's about 23.6.
fn ships_beside_systems(
    galaxy: &Galaxy,
    at: [f64; 3],
    systems: usize,
    lcg: &mut Lcg,
) -> Vec<[f64; 3]> {
    let at = GalacticPosition::from_light_years(at).expect("inside the cube");
    let key = CellKey::containing(Layer::C, &at).expect("inside the cube");
    let mut cell = Vec::new();
    generate_cell(galaxy, key, &mut cell);
    let mut unit = || {
        let draw = lcg.next_below(2_001);
        f64::from(u32::try_from(draw).expect("under 2,001")) / 1_000.0 - 1.0
    };
    let mut ships = Vec::new();
    for record in cell.iter().take(systems) {
        let [x, y, z] = record.epoch_position().to_light_years_f64();
        for _ in 0..2 {
            ships.push([x + 6.0 * unit(), y + 6.0 * unit(), z + 6.0 * unit()]);
        }
    }
    ships
}

/// The 28 ships of the solar circle: two beside each of the first four systems of the layer-C cells
/// at the Sun-like point and 800 ly above it, and twelve at random points within ±10,000 ly of it
/// along x and ±100 ly in y and z.
fn solar_circle_ships(galaxy: &Galaxy) -> Vec<[f64; 3]> {
    let mut lcg = Lcg::new(0x0312_b0f0);
    let mut ships = ships_beside_systems(galaxy, [0.0, 26_000.0, 0.0], 4, &mut lcg);
    ships.extend(ships_beside_systems(
        galaxy,
        [0.0, 26_000.0, 800.0],
        4,
        &mut lcg,
    ));
    for _ in 0..12 {
        let [x, y, z] = [0, 1, 2].map(|_| {
            let draw = lcg.next_below(2_001);
            f64::from(u32::try_from(draw).expect("under 2,001")) / 1_000.0 - 1.0
        });
        ships.push([10_000.0 * x, 26_000.0 + 100.0 * y, 100.0 * z]);
    }
    ships
}

/// `frame_at` finds exactly the frame a brute-force search over every nearby system finds, beside
/// generated systems on and above the plane at the Sun-like point and at random points of the solar
/// circle (P03.T12.b, validation).
#[test]
fn the_search_finds_the_frame_a_brute_force_search_finds() {
    let galaxy = galaxy();
    let ships = solar_circle_ships(&galaxy);
    let held = assert_frames_match_brute_force(&galaxy, &ships);
    // A good share of the answers name a system, so the comparison is not of two `None`s. It was
    // over half at version 8 and is 28 of 56 at version 10, where P02.T11's tuning shrank the
    // tidal radii by about 3%.
    let answers = 2 * ships.len();
    assert!(
        3 * held >= answers,
        "only {held} of {answers} answers named a system"
    );
}

/// The frame `frame_at` picks for each of the solar circle's 28 ships at the epoch and at the end of
/// the clock window, pinned (ruling 24 of 2026-09-22).
///
/// The brute-force test shows that the search is right; this shows that its answers stay put across
/// generator changes, since which system a ship is in is game state. Each ship's position is pinned
/// beside its two answers, so a move of the systems the ships are placed beside reads as such, not
/// as a change of the frame rule. The answers are computed apart from the brute-force search, so
/// that a perturbed rule turns this red on its own: a search margin of 0.5 in place of 1.25 does.
#[test]
fn the_frames_of_the_solar_circle_ships_are_pinned() {
    let galaxy = galaxy();
    let mut cache = Keep::default();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line(&format!(
        "# seed 0x{SEED:016x}, milky_way_like, no sources, no current frame"
    ));
    for (i, ship) in solar_circle_ships(&galaxy).into_iter().enumerate() {
        let label = format!("ship[{i:02}]");
        for (axis, value) in ["x", "y", "z"].into_iter().zip(ship) {
            w.f64(&format!("{label}.{axis}_ly"), value);
        }
        let at = GalacticPosition::from_light_years(ship).expect("inside the cube");
        for (when, t) in [("epoch", UniverseTime::EPOCH), ("end", ClockWindow::END)] {
            let frame = frame_at(&galaxy, &mut cache, &[], &at, t, None)
                .expect("a time inside the clock window");
            let answer = frame.map_or_else(
                || "none".to_owned(),
                |id| format!("0x{:016x} ({})", id.raw(), id.designation()),
            );
            w.line(&format!("{label}.frame_at_{when} = {answer}"));
        }
    }
    golden!("frame/frames", w.as_str());
}

/// The same in the dense inner galaxy, where one layer-E cell holds up to a quarter of a million
/// systems: beside a system of the outer bulge, and ever closer to the centre, where the tidal
/// radius falls as a point mass's does, in proportion to the distance (P03.T12.b, validation).
#[test]
#[ignore = "slow: brute-force frame searches through the galactic centre's layer-E cell"]
fn the_search_finds_the_brute_force_frame_towards_the_centre() {
    let galaxy = galaxy();
    let mut lcg = Lcg::new(0x0312_b0f1);
    let mut ships = ships_beside_systems(&galaxy, [2_262.7, 2_262.7, 0.0], 2, &mut lcg);
    for r in [1_000.0, 100.0, 10.0, 1.0] {
        ships.push([r * 0.6, r * 0.8, 0.0]);
    }
    let held = assert_frames_match_brute_force(&galaxy, &ships);
    println!("{held} of {} answers named a system", 2 * ships.len());
}

/// A time outside the clock window is refused, as a range query refuses it, rather than walking a
/// sphere padded for millennia of drift (P03.T12.b, validation: at 10⁵ years a search took over a
/// second and grew as the cube of the time).
#[test]
fn a_time_outside_the_clock_window_is_refused() {
    let galaxy = galaxy();
    let ship = sunlike_point(&galaxy);
    let just_after = UniverseTime::from_julian_years(1_001).expect("a representable time");
    let just_before = UniverseTime::from_julian_years(-1_001).expect("a representable time");
    for t in [just_after, just_before, SourceHorizon::START] {
        assert_eq!(
            frame_at(&galaxy, &mut NoCache::new(), &[], &ship, t, None),
            Err(FindFrameError::TimeOutsideClockWindow(t)),
            "{t}"
        );
    }
    // Both ends of the window are inside it.
    for t in [ClockWindow::START, ClockWindow::END] {
        assert!(frame_at(&galaxy, &mut NoCache::new(), &[], &ship, t, None).is_ok());
    }
}
