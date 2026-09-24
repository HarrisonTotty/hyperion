//! Tests of the assembled generator, its queries at a time and its labels (P14.T30.a–c).
//!
//! The real systems are `planetary::testing`'s volume-limited sample of a Milky-Way-parameter
//! galaxy at the solar circle (P14.T1.d), whose contexts carry plan 11's companions and plan 06's
//! models; the synthetic hosts are the context builder's.

use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::galaxy::placement::CellKey;
use crate::id::Layer;
use crate::orbit::Eccentricity;
use crate::planetary::architecture::HostMultiplicity;
use crate::planetary::architecture::template::EARTH_MASSES_PER_JUPITER_MASS;
use crate::planetary::derive::{PlanetClass, habitable_zone};
use crate::planetary::fate::DestructionCause;
use crate::planetary::index::{BodySlot, BodySub};
use crate::planetary::params::HILL_STABLE_GAP;
use crate::planetary::placement::mutual_hill_radius;
use crate::planetary::record::{DetailLevel, RecordSection, SectionState};
use crate::planetary::testing::{SampleFilter, sample_contexts, synthetic_binary, synthetic_star};
use crate::stellar::Phase;
use crate::stellar::multiplicity::star_positions_at;
use crate::time::{CLOCK_WINDOW_H, ClockWindow, Span};
use crate::units::{AstronomicalUnits, Dex, Kelvin, Years};

/// The universe every test generates in.
pub(crate) const SEED: Seed = Seed::new(0x5eed_0014_0030);

/// How many real systems the ordinary suite samples.
const SAMPLE: usize = 400;

/// The real systems nearest the solar-circle point, built once for every test.
pub(crate) fn sample() -> &'static [SystemContext] {
    static CONTEXTS: OnceLock<Vec<SystemContext>> = OnceLock::new();
    CONTEXTS.get_or_init(|| sample_contexts(SAMPLE, SEED, SampleFilter::ALL).unwrap())
}

/// The sample's systems, generated once.
pub(crate) fn generated() -> &'static [(SystemContext, PlanetarySystem)] {
    static SYSTEMS: OnceLock<Vec<(SystemContext, PlanetarySystem)>> = OnceLock::new();
    SYSTEMS.get_or_init(|| {
        sample()
            .iter()
            .map(|ctx| (ctx.clone(), generate_planets(SEED, ctx)))
            .collect()
    })
}

/// The ID of candidate `index` of a cell at the solar circle, for synthetic hosts.
pub(crate) fn id(index: u32) -> SystemId {
    CellKey::new(Layer::C, [0, 812, 0])
        .unwrap()
        .candidate_id(index)
        .unwrap()
}

/// A synthetic Sun of age 4.57 Gyr as the system `index`.
pub(crate) fn sun(index: u32) -> SystemContext {
    synthetic_star(
        id(index),
        SolarMasses::new(1.0),
        Dex::ZERO,
        Years::new(4.57e9),
    )
    .unwrap()
}

/// The times the property tests ask about: −H, the epoch and +H.
fn window() -> [UniverseTime; 3] {
    [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END]
}

// P14.T30.a: `generate_planets`.

#[test]
fn generate_gives_the_same_system_twice() {
    for (ctx, system) in generated().iter().take(60) {
        assert_eq!(&generate_planets(SEED, ctx), system);
    }
    let a = generate_planets(SEED, &sun(1));
    assert_eq!(a, generate_planets(SEED, &sun(1)));
    assert_ne!(a, generate_planets(Seed::new(1), &sun(1)));
}

#[test]
fn generate_is_order_independent() {
    let contexts: Vec<SystemContext> = sample().iter().take(40).cloned().collect();
    assert_order_independent(&contexts, |ctx| generate_planets(SEED, ctx));
}

#[test]
fn generate_is_generate_planets_in_the_slice() {
    for (ctx, system) in generated().iter().take(40) {
        assert_eq!(&generate(SEED, ctx), system);
    }
}

#[test]
fn every_index_decodes_is_unique_and_none_is_at_the_stellar_level() {
    let mut bodies = 0_u32;
    for (_, system) in generated() {
        let mut previous: Option<BodyIndex> = None;
        for body in system.bodies() {
            let index = body.index();
            let (slot, sub) = BodyIndex::decode(index.get()).unwrap();
            assert_eq!((slot, sub), (index.slot(), index.sub()));
            assert!(matches!(slot, BodySlot::Planet(n) if n >= 1), "{index:?}");
            assert_eq!(sub, BodySub::Primary);
            assert!(previous.is_none_or(|p| p < index), "sorted and unique");
            assert_eq!(system.body(index), Some(body));
            previous = Some(index);
            bodies += 1;
        }
    }
    assert!(bodies > 300, "{bodies} bodies");
}

#[test]
fn a_brown_dwarf_companion_gains_no_body_in_slot_zero() {
    let mut planets = 0;
    for i in 0..40 {
        let a = Metres::from(AstronomicalUnits::new(30.0));
        let ctx = synthetic_binary(
            id(100 + i),
            SolarMasses::new(1.0),
            SolarMasses::new(0.05),
            a,
            Eccentricity::new(0.2).unwrap(),
            Dex::ZERO,
            Years::new(3e9),
        )
        .unwrap();
        let system = generate_planets(SEED, &ctx);
        assert_eq!(
            system.zones().len(),
            3,
            "a zone about each and one about both"
        );
        for body in system.bodies() {
            assert_ne!(body.index().slot(), BodySlot::Stellar);
            planets += 1;
        }
    }
    assert!(planets > 20, "{planets}");
}

#[test]
fn slots_run_host_by_host_in_hierarchy_order() {
    for (_, system) in generated() {
        let mut last = 0_u16;
        for zone in system.zones() {
            let slots: Vec<u16> = system
                .bodies()
                .iter()
                .filter(|body| body.host() == zone.host())
                .map(|body| body.index().get() >> 8)
                .collect();
            for &slot in &slots {
                assert!(slot > last, "{:?}", system.system());
            }
            last = slots.last().copied().unwrap_or(last);
        }
    }
}

/// The generator is the chain of its stages (P14.T9.c's): each host's disc, class and planets are
/// the stages' own, and the strip radius, far beyond every disc of a field system, changes no bit
/// of them.
#[test]
fn each_host_is_the_chain_of_its_stages() {
    let mut hosts = 0;
    for (ctx, system) in generated().iter().take(150) {
        let (seed, id) = (SEED, ctx.id());
        let stars = ctx.zone_stars();
        let hierarchy = ctx.hierarchy();
        let under = members_under(hierarchy);
        let mut slot = FIRST_PLANET_SLOT;
        for (zone, host) in system.zones().iter().zip(system.hosts()) {
            let inputs = ZoneDiscInputs::for_zone(seed, id, zone, &stars, ctx.fe_h()).unwrap();
            let disc = inputs.derive();
            assert_eq!(host.disc(), &disc);
            let weights = class_weights(zone.host_mass(), ctx.fe_h());
            let drawn = draw_class(
                seed,
                id,
                zone.host_number(),
                &weights,
                &zone.class_constraints(&disc),
            );
            assert_eq!(host.drawn_class(), drawn);
            let plane = plane_of(hierarchy, &under, zone);
            let placement_host = PlacementHost::new(zone.host_number(), *inputs.host(), plane);
            let placed = place(
                seed,
                id,
                &placement_host,
                zone.truncation(),
                &disc,
                drawn,
                slot,
            );
            slot = placed.next_slot();
            assert_eq!(host.class(), placed.class());
            let bodies: Vec<&PlacedPlanet> = system
                .bodies()
                .iter()
                .filter(|body| body.host() == zone.host())
                .map(Body::placed)
                .collect();
            assert_eq!(bodies, placed.planets().iter().collect::<Vec<_>>());
            for body in system.bodies().iter().filter(|b| b.host() == zone.host()) {
                let body_id = body.index().body_id(id);
                assert_eq!(body.radius_rank(), radius_rank(seed, body_id));
                let formation =
                    Formation::draw(seed, body_id, body.mass(), inputs.lifetime()).unwrap();
                assert_eq!(body.formation(), &formation);
            }
            hosts += 1;
        }
    }
    assert!(hosts > 150, "{hosts} hosts");
}

/// Design note 14: a system whose sphere of influence is small has nothing generated beyond 0.49
/// of it.
#[test]
fn nothing_is_generated_beyond_the_strip_radius() {
    let mut cut = 0;
    for i in 0..60 {
        let tidal = Metres::from(AstronomicalUnits::new(40.0));
        let ctx = SystemContext::builder()
            .system(id(200 + i))
            .star(SolarMasses::new(1.0))
            .age_at_epoch(Years::new(4.57e9))
            .tidal_radius(tidal)
            .build()
            .unwrap();
        let strip = ctx.strip_radius();
        let system = generate_planets(SEED, &ctx);
        let host = &system.hosts()[0];
        assert_eq!(host.limits().outer(), Some(strip));
        if let Some(profile) = host.disc().profile() {
            assert!(profile.outer_edge() <= strip);
            cut += 1;
        }
        for body in system.bodies() {
            assert!(body.orbit().apoapsis() <= strip, "{:?}", body.index());
        }
    }
    assert!(cut > 30, "{cut}");
}

// P14.T30.b: queries at a time.

#[test]
fn an_unused_index_is_no_such_body() {
    let (ctx, system) = generated()
        .iter()
        .find(|(_, s)| !s.bodies().is_empty())
        .unwrap();
    let last = system.bodies().last().unwrap().index();
    let (BodySlot::Planet(n), _) = (last.slot(), last.sub()) else {
        panic!("a planet's slot")
    };
    let unused = [
        BodyIndex::new(BodySlot::Planet(n + 1), BodySub::Primary).unwrap(),
        BodyIndex::new(BodySlot::Planet(1), BodySub::Moon(1)).unwrap(),
        BodyIndex::new(BodySlot::Belt(0), BodySub::Primary).unwrap(),
        BodyIndex::PRIMARY,
    ];
    for index in unused {
        let t = UniverseTime::EPOCH;
        assert_eq!(
            system.body_at(ctx, index, t),
            Err(ResolveBodyError::NoSuchBody)
        );
        assert_eq!(
            system.position_at(ctx, index, t),
            Err(ResolveBodyError::NoSuchBody)
        );
    }
}

#[test]
fn an_unborn_system_s_bodies_are_all_not_yet_formed() {
    let mut bodies = 0;
    for i in 0..30 {
        let ctx = synthetic_star(
            id(300 + i),
            SolarMasses::new(1.0),
            Dex::ZERO,
            Years::new(-500.0),
        )
        .unwrap();
        let system = generate_planets(SEED, &ctx);
        for t in window() {
            let snapshot = system.snapshot_at(&ctx, t);
            for record in snapshot.bodies() {
                assert_eq!(record.identity().state(), BodyState::NotYetFormed);
                assert_eq!(record.position(), None);
                assert_eq!(
                    record.section_state(RecordSection::Orbit),
                    SectionState::NotApplicable
                );
                assert_eq!(
                    record.section_state(RecordSection::Bulk),
                    SectionState::NotApplicable
                );
                assert_eq!(
                    record.mass().ok().copied(),
                    system.body(record.index()).map(Body::mass)
                );
                bodies += 1;
            }
            // The star itself forms 500 years after the epoch.
            let formed = ctx.age_at(t).value() > 0.0;
            let zone = system.habitable_zone_at(&ctx, OrbitHost::Star(0), t);
            assert_eq!(zone.is_some(), formed);
        }
    }
    assert!(bodies > 30, "{bodies}");
}

#[test]
fn a_snapshot_is_body_at_body_by_body() {
    for (ctx, system) in generated().iter().take(80) {
        for t in window() {
            let snapshot = system.snapshot_at(ctx, t);
            assert_eq!(snapshot.system(), system.system());
            assert_eq!(snapshot.time(), t);
            assert_eq!(snapshot.bodies().len(), system.bodies().len());
            for (record, body) in snapshot.bodies().iter().zip(system.bodies()) {
                assert_eq!(record, &system.body_at(ctx, body.index(), t).unwrap());
                assert_eq!(
                    record.position(),
                    system.position_at(ctx, body.index(), t).unwrap()
                );
            }
        }
    }
}

/// The slice's section states (ruling 34): what is not computed says so, and what does not apply
/// to a giant is not applicable.
#[test]
fn a_record_tags_what_the_slice_does_not_compute() {
    let (mut present, mut giants) = (0, 0);
    for (ctx, system) in generated().iter().take(120) {
        let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
        assert_eq!(snapshot.belts(), &Section::NotModelled);
        assert_eq!(snapshot.halo(), &Section::NotModelled);
        for (record, body) in snapshot.bodies().iter().zip(system.bodies()) {
            let identity = record.identity();
            assert_eq!(identity.kind(), BodyKind::Planet);
            assert_eq!(identity.parent(), Some(body.host()));
            assert_eq!(identity.id(), body.index().body_id(system.system()));
            assert_eq!(identity.label().state(), SectionState::Ok);
            assert_eq!(record.level(), DetailLevel::Full);
            for section in [
                RecordSection::Moons,
                RecordSection::Rings,
                RecordSection::Hooks,
            ] {
                assert_eq!(record.section_state(section), SectionState::NotModelled);
            }
            if identity.state() == BodyState::Present {
                present += 1;
                let bulk = record.bulk().ok().unwrap();
                let surface = record.section_state(RecordSection::Surface);
                if bulk.class().has_surface() {
                    assert_eq!(surface, SectionState::NotModelled);
                } else {
                    assert_eq!(surface, SectionState::NotApplicable);
                    giants += 1;
                }
                assert!(record.position().is_some());
                assert_eq!(record.section_state(RecordSection::Orbit), SectionState::Ok);
            }
        }
    }
    assert!(
        present > 100 && giants > 5,
        "{present} present, {giants} giants"
    );
}

/// The host's position is plan 11's walk: a star's is `star_positions_at`'s bit for bit, and a
/// body's is its host's plus its own Kepler offset.
#[test]
fn a_body_is_at_its_host_s_position_plus_its_orbit() {
    let mut positions = Vec::new();
    let mut pairs = 0;
    for (ctx, system) in generated() {
        for t in window() {
            let epoch = Epoch::new(system, ctx, t);
            star_positions_at(ctx.hierarchy(), t, &mut positions);
            for (n, (_, at)) in positions.iter().enumerate() {
                let centre = epoch.centre(1 << n);
                for (x, y) in centre.metres().into_iter().zip(at.metres()) {
                    assert_same_bits(x, y);
                }
            }
            for zone in system.zones().iter().filter(|z| z.members().count() > 1) {
                // A pair's barycentre is its members' mass-weighted mean.
                let centre = epoch.centre(members_of(zone)).metres();
                let (mut moment, mut mass, mut reach) = ([0.0; 3], 0.0, 1.0_f64);
                for m in zone.members() {
                    let star = ctx.hierarchy().stars()[usize::from(m)]
                        .initial_mass()
                        .value();
                    let at = positions[usize::from(m)].1.metres();
                    for k in 0..3 {
                        moment[k] += star * at[k];
                    }
                    mass += star;
                    reach = reach.max(positions[usize::from(m)].1.distance_from_origin().value());
                }
                for k in 0..3 {
                    assert!((moment[k] / mass - centre[k]).abs() <= 1e-12 * reach);
                }
                pairs += 1;
            }
            for body in system.bodies() {
                let record = system.body_at(ctx, body.index(), t).unwrap();
                let Some(orbit) = record.orbit().ok() else {
                    continue;
                };
                let zone = system.zone(body.host()).unwrap();
                let (offset, _) = orbit.elements().relative_state_at(t);
                let expected = epoch.centre(members_of(zone)).translated(offset);
                let got = system.position_at(ctx, body.index(), t).unwrap().unwrap();
                for (x, y) in got.metres().into_iter().zip(expected.metres()) {
                    assert_same_bits(x, y);
                }
            }
        }
    }
    assert!(pairs > 10, "{pairs} pair zones");
}

#[test]
fn a_single_star_s_habitable_zone_is_kopparapu_s_of_its_state() {
    let ctx = sun(7);
    let system = generate_planets(SEED, &ctx);
    let t = UniverseTime::EPOCH;
    let state = ctx.stars()[0].state_at(t).unwrap();
    let zone = system
        .habitable_zone_at(&ctx, OrbitHost::Star(0), t)
        .unwrap();
    let expected = habitable_zone(state.luminosity(), state.effective_temperature());
    assert_eq!(zone, expected);
    let (inner, outer) = zone.conservative();
    let au = |m: Metres| AstronomicalUnits::from(m).value();
    assert!((0.9..1.1).contains(&au(inner)) && (1.5..1.9).contains(&au(outer)));
    assert!(
        system
            .habitable_zone_at(&ctx, OrbitHost::Star(1), t)
            .is_none()
    );
    assert!(
        system
            .habitable_zone_at(&ctx, OrbitHost::Barycentre, t)
            .is_none()
    );
}

#[test]
fn a_companion_s_light_pushes_a_star_s_habitable_zone_out() {
    let a = Metres::from(AstronomicalUnits::new(20.0));
    let ctx = synthetic_binary(
        id(400),
        SolarMasses::new(1.0),
        SolarMasses::new(0.9),
        a,
        Eccentricity::new(0.3).unwrap(),
        Dex::ZERO,
        Years::new(4e9),
    )
    .unwrap();
    let system = generate_planets(SEED, &ctx);
    let t = UniverseTime::EPOCH;
    let state = ctx.stars()[0].state_at(t).unwrap();
    let alone = habitable_zone(state.luminosity(), state.effective_temperature());
    let lit = system
        .habitable_zone_at(&ctx, OrbitHost::Star(0), t)
        .unwrap();
    assert!(lit.maximum_greenhouse() > alone.maximum_greenhouse());
    let both = system
        .habitable_zone_at(&ctx, OrbitHost::Barycentre, t)
        .unwrap();
    assert!(both.moist_greenhouse() > lit.moist_greenhouse());
}

/// P14.T10.a rerun on whole generated systems (P14.T30.b): at −H, the epoch and +H, every pair of
/// present bodies sharing a host has the inner apocentre at least 2√3 mutual Hill radii below the
/// outer pericentre, on the fate transform's orbits at the time; and every body lies inside its
/// zone and the strip radius.
///
/// The Hill radii are those of the host's mass at birth. Design note 9's expansion widens every
/// orbit of a host by M₀ ÷ M, the gaps with them, so in that frame the spacing checked at
/// placement holds at every time. About the host's mass at the time a Hill radius grows by a
/// further (M₀ ÷ M)^⅓, and a pair spaced near the floor about a star that has since become a white
/// dwarf is no longer Hill-stable (Debes and Sigurdsson 2002): 84 of this sample's 3,612 checks of
/// a pair at a time once the calibration (ruling 66) filled rocky groups to the snow line, all
/// about hosts that have lost mass (15 of 3,945 before it). Packed systems are the ones that
/// post-main-sequence mass loss destabilises, so the share rose with the packing. The slice does
/// not model that instability; the bound below holds it to under 5%.
///
/// The zone is the one at birth, its outer limit widened as its host's orbits are, by the
/// host's initial mass over its mass then (design note 11's a₀ M₀ ÷ M): until plan 11's binary
/// evolution (P11.T4) a pair's orbit does not widen as its stars lose mass (ruling 33), while
/// their planets' orbits do, so an evolved star's planet can outgrow the zone its companion bounds
/// at birth: 7 of this sample's 4,830 bodies at the epoch, all about hosts that have lost mass.
#[test]
fn no_overlapping_orbits_in_generated_systems() {
    let (mut pairs, mut bodies, mut outgrown, mut unstable) = (0_u32, 0_u32, 0_u32, 0_u32);
    for (ctx, system) in generated() {
        let strip = ctx.strip_radius();
        for t in window() {
            let snapshot = system.snapshot_at(ctx, t);
            for zone in system.zones() {
                let mut orbits: Vec<(EarthMasses, KeplerElements)> = snapshot
                    .bodies()
                    .iter()
                    .filter(|record| record.identity().parent() == Some(zone.host()))
                    .filter_map(|record| {
                        let mass = *record.mass().ok()?;
                        Some((mass, *record.orbit().ok()?.elements()))
                    })
                    .collect();
                orbits.sort_by(|a, b| a.1.semi_major_axis().total_cmp(&b.1.semi_major_axis()));
                let host = zone
                    .members()
                    .map(|m| ctx.stars()[usize::from(m)].state_at(t).unwrap().mass())
                    .fold(SolarMasses::ZERO, |sum, m| sum + m);
                let widening = zone.host_mass() / host;
                for (_, orbit) in &orbits {
                    if let Some(inner) = zone.inner() {
                        assert!(
                            orbit.periapsis() >= inner * (1.0 - 1e-12),
                            "{:?}",
                            system.system()
                        );
                    }
                    if let Some(outer) = zone.outer() {
                        let widened = outer * (widening * (1.0 + 1e-12));
                        assert!(orbit.apoapsis() <= widened, "{:?}", system.system());
                        if t == UniverseTime::EPOCH && orbit.apoapsis() > outer {
                            assert!(widening > 1.01, "only mass loss widens an orbit");
                            outgrown += 1;
                        }
                    }
                    assert!(orbit.apoapsis() <= strip);
                    bodies += 1;
                }
                for pair in orbits.windows(2) {
                    let ((m1, o1), (m2, o2)) = (pair[0], pair[1]);
                    let (a1, a2) = (o1.semi_major_axis(), o2.semi_major_axis());
                    let hill = mutual_hill_radius(m1, m2, zone.host_mass(), a1, a2).value();
                    let gap = o2.periapsis().value() - o1.apoapsis().value();
                    let now = mutual_hill_radius(m1, m2, host, a1, a2).value();
                    if gap < HILL_STABLE_GAP * now {
                        assert!(widening > 1.01, "only mass loss unsettles a placed pair");
                        unstable += 1;
                    }
                    assert!(
                        gap >= HILL_STABLE_GAP * hill * (1.0 - 1e-12),
                        "{:?} at {t:?}: {gap} m against {} Hill radii of {hill} m",
                        system.system(),
                        HILL_STABLE_GAP
                    );
                    pairs += 1;
                }
            }
        }
    }
    assert!(
        pairs > 300 && bodies > 900,
        "{pairs} pairs, {bodies} bodies"
    );
    assert!(
        outgrown < bodies / 100 && unstable < pairs / 20,
        "{outgrown} of {bodies} bodies outgrew their zone, {unstable} of {pairs} pairs unsettled"
    );
}

/// The effective temperature a body is to be kept below: the hottest of the stars that light it,
/// or `None` if one it orbits is a black hole (T16.b leaves those hosts out).
fn hottest_light(ctx: &SystemContext, t: UniverseTime) -> Option<Kelvin> {
    let states: Vec<StarState> = ctx
        .stars()
        .iter()
        .map(|star| star.state_at(t).unwrap())
        .collect();
    if states.iter().any(|s| s.phase() == Phase::BlackHole) {
        return None;
    }
    states
        .iter()
        .map(StarState::effective_temperature)
        .reduce(|a, b| if b > a { b } else { a })
}

/// P14.T16.b rerun on whole generated systems (P14.T30.b): no present body, its internal heat
/// included, is hotter than the hottest star that lights it, at −H, the epoch and +H; systems with
/// a black hole are left out.
#[test]
fn no_planet_hotter_than_its_star_in_generated_systems() {
    let (mut bodies, mut giants) = (0_u32, 0_u32);
    for (ctx, system) in generated() {
        for t in window() {
            let Some(hottest) = hottest_light(ctx, t) else {
                continue;
            };
            for record in system.snapshot_at(ctx, t).bodies() {
                let Some(bulk) = record.bulk().ok() else {
                    continue;
                };
                assert!(
                    bulk.equilibrium_temperature() < hottest,
                    "{:?} {:?}: {:?} against {hottest:?}",
                    system.system(),
                    record.index(),
                    bulk.equilibrium_temperature()
                );
                bodies += 1;
                if bulk.class() == PlanetClass::GasGiant {
                    giants += 1;
                }
            }
        }
    }
    assert!(
        bodies > 900 && giants > 20,
        "{bodies} bodies, {giants} giants"
    );
}

/// P14.T16.b's continuity rerun on whole generated systems: at steps of a year across ±H, no
/// present body's radius, equilibrium temperature or envelope fraction jumps by a relative 10⁻³,
/// except in a step where a star of its system changes phase or the body's own state changes.
#[test]
fn radius_temperature_and_envelope_are_continuous_across_the_window() {
    continuity(generated().iter().take(12), 1);
}

/// The continuity test on the whole ordinary sample.
#[test]
#[ignore = "slow: steps every body of 400 real systems through ±H a year at a time"]
fn radius_temperature_and_envelope_are_continuous_across_the_window_slow() {
    continuity(generated().iter(), 1);
}

/// Steps the systems `systems` through ±H at `step_years`, and checks every step (see
/// [`radius_temperature_and_envelope_are_continuous_across_the_window`]).
fn continuity<'a>(
    systems: impl Iterator<Item = &'a (SystemContext, PlanetarySystem)>,
    step_years: i64,
) {
    let window = CLOCK_WINDOW_H.as_julian_years_f64();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "H is a whole number of years, 1,000"
    )]
    let steps = (2.0 * window) as i64 / step_years;
    let mut checked = 0_u64;
    for (ctx, system) in systems {
        if system.bodies().is_empty() {
            continue;
        }
        let mut previous: Option<(Vec<Option<Phase>>, SystemSnapshot)> = None;
        for k in 0..=steps {
            let t = ClockWindow::START
                .checked_add(Span::from_julian_years(k * step_years).unwrap())
                .unwrap();
            let phases: Vec<Option<Phase>> = ctx
                .stars()
                .iter()
                .map(|star| star.state_at(t).map(|s| s.phase()))
                .collect();
            let snapshot = system.snapshot_at(ctx, t);
            if let Some((before_phases, before)) = &previous {
                let changed = before_phases != &phases;
                for (b, n) in before.bodies().iter().zip(snapshot.bodies()) {
                    let (Some(b), Some(n)) = (b.bulk().ok(), n.bulk().ok()) else {
                        continue;
                    };
                    checked += 1;
                    if changed {
                        continue;
                    }
                    let jump = |x: f64, y: f64| ((y - x) / x).abs();
                    let label = format!("{:?}", system.system());
                    assert!(
                        jump(
                            b.equilibrium_temperature().value(),
                            n.equilibrium_temperature().value()
                        ) < 1e-3,
                        "{label}"
                    );
                    assert!(
                        jump(b.radius().value(), n.radius().value()) < 1e-3,
                        "{label}"
                    );
                    let (e0, e1) = (b.fractions().envelope(), n.fractions().envelope());
                    assert!((e1 - e0).abs() < 1e-3 * e0.max(1e-3), "{label}");
                }
            }
            previous = Some((phases, snapshot));
        }
    }
    assert!(checked > 5_000, "{checked} steps");
}

/// T28's states in whole systems: a body that has ended stays ended, and one present at +H was
/// present or not yet formed at −H.
#[test]
fn every_body_s_states_across_the_window_are_a_prefix_of_its_life() {
    let rank = |state: BodyState| match state {
        BodyState::NotYetFormed => 0,
        BodyState::Present => 1,
        BodyState::Destroyed { .. } | BodyState::Unbound { .. } => 2,
    };
    let mut engulfed = 0;
    for (ctx, system) in generated() {
        let snapshots: Vec<SystemSnapshot> = window().map(|t| system.snapshot_at(ctx, t)).to_vec();
        for i in 0..system.bodies().len() {
            let states: Vec<BodyState> = snapshots
                .iter()
                .map(|s| s.bodies()[i].identity().state())
                .collect();
            assert!(
                states.windows(2).all(|w| rank(w[0]) <= rank(w[1])),
                "{states:?}"
            );
            if matches!(
                states[1],
                BodyState::Destroyed {
                    cause: DestructionCause::Engulfed,
                    ..
                }
            ) {
                engulfed += 1;
            }
        }
    }
    // Old, evolved hosts in the sample have swallowed some of their planets.
    assert!(engulfed > 0, "{engulfed}");
}

/// Every body's primordial circularisation is T8.e's damping time with Chen and Kipping's median
/// radius (ruling 62.5), and a hot Jupiter of up to 1.5 Jupiter masses within five days of an old
/// Sun is circular to 0.01 (P14.T8.e's finding).
#[test]
fn circularisation_is_t8e_s_and_hot_jupiters_circularise() {
    for (_, system) in generated() {
        for body in system.bodies() {
            let zone = system.zone(body.host()).unwrap();
            let radius = primordial_radius(body.mass());
            let expected = circularisation(body.placed(), zone.host_mass(), radius);
            assert_eq!(body.circularisation(), expected);
            assert_eq!(body.fate().circularisation(), expected);
        }
    }
    let mut hot = 0;
    for i in 0..3_000 {
        let ctx = synthetic_star(
            id(3_000 + i),
            SolarMasses::new(1.0),
            Dex::new(0.3),
            Years::new(6e9),
        )
        .unwrap();
        let system = generate_planets(SEED, &ctx);
        for body in system.bodies() {
            let jupiters = body.mass().value() / EARTH_MASSES_PER_JUPITER_MASS;
            let days = body.orbit().period().value() / 86_400.0;
            if (0.3..1.5).contains(&jupiters) && days < 5.0 {
                let record = system
                    .body_at(&ctx, body.index(), UniverseTime::EPOCH)
                    .unwrap();
                let orbit = record.orbit().ok().unwrap();
                let e = orbit.elements().eccentricity().value();
                assert!(e < 0.01, "{:?}: e {e}", body.index());
                hot += 1;
            }
        }
        if hot >= 5 {
            break;
        }
    }
    assert!(hot >= 5, "{hot} hot Jupiters");
}

// Round 8's validation of the slice (`val14`).

/// A query at one time is a function of the system, its context and that time alone: asking the
/// same system at other times first, and for single bodies, changes no record of a later snapshot
/// (sim-determinism, "Order independence").
#[test]
fn a_query_does_not_depend_on_the_queries_before_it() {
    for (ctx, system) in generated().iter().take(80) {
        let fresh = generate_planets(SEED, ctx);
        let expected: Vec<_> = window()
            .iter()
            .map(|&t| fresh.snapshot_at(ctx, t))
            .collect();
        // The same queries in the other order, with single-body queries between them.
        for (i, &t) in window().iter().enumerate().rev() {
            // The answers between are not the point, only that asking them moves nothing after.
            for body in system.bodies() {
                let _ = system.position_at(ctx, body.index(), t);
                let _ = system.body_at(ctx, body.index(), t);
            }
            for zone in system.zones() {
                let _ = system.habitable_zone_at(ctx, zone.host(), t);
            }
            assert_eq!(system.snapshot_at(ctx, t), expected[i], "{:?}", ctx.id());
        }
    }
}

/// Design note 10's close-binary flag is Kraus et al.'s (2016) 47 au cut: a component of a pair
/// inside it has its planets suppressed and its circumbinary zone takes the pair's plane, and one
/// of a pair outside it neither.
#[test]
fn the_close_binary_flag_and_the_aligned_plane_are_kraus_s_cut() {
    let e = Eccentricity::new(0.3).unwrap();
    for (index, (au, close)) in [(45.0, true), (50.0, false)].into_iter().enumerate() {
        let ctx = synthetic_binary(
            id(900 + u32::try_from(index).unwrap()),
            SolarMasses::new(1.0),
            SolarMasses::new(0.8),
            Metres::from(AstronomicalUnits::new(au)),
            e,
            Dex::ZERO,
            Years::new(4.57e9),
        )
        .unwrap();
        let system = generate_planets(SEED, &ctx);
        let star = system
            .zone(OrbitHost::Star(0))
            .expect("the primary keeps a zone");
        assert_eq!(
            star.host_multiplicity(),
            if close {
                HostMultiplicity::CloseBinary
            } else {
                HostMultiplicity::SingleOrWide
            },
            "{au} au"
        );
        let pair = ctx.hierarchy().pairs().next().unwrap().1;
        let aligned = SystemPlane::of_orbit(pair.orientation());
        let barycentre = system.host(OrbitHost::Barycentre).unwrap();
        assert_eq!(
            system
                .zone(OrbitHost::Barycentre)
                .unwrap()
                .host_multiplicity(),
            HostMultiplicity::SingleOrWide,
            "a pair's own circumbinary zone is never flagged"
        );
        assert_eq!(barycentre.plane() == aligned, close, "{au} au");
    }
}
