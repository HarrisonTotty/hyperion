//! Tests of the assembled generator, its queries at a time and its labels (P14.T30.a–c).
//!
//! The real systems are `planetary::testing`'s volume-limited sample of a Milky-Way-parameter
//! galaxy at the solar circle (P14.T1.d), whose contexts carry plan 11's companions and plan 06's
//! models; the synthetic hosts are the context builder's.

use core::f64::consts::PI;
use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::galaxy::placement::CellKey;
use crate::id::Layer;
use crate::orbit::Eccentricity;
use crate::planetary::architecture::HostMultiplicity;
use crate::planetary::architecture::template::EARTH_MASSES_PER_JUPITER_MASS;
use crate::planetary::derive::atmosphere::{Gas, SurfaceState};
use crate::planetary::derive::{PlanetClass, habitable_zone};
use crate::planetary::fate::DestructionCause;
use crate::planetary::hooks::figures::{Crust, LEFTOVER_DENSITY, TectonicRegime};
use crate::planetary::index::{BodySlot, BodySub};
use crate::planetary::params::{FLATTENING_CAP, HILL_STABLE_GAP};
use crate::planetary::placement::mutual_hill_radius;
use crate::planetary::record::{
    DetailLevel, Modelled, MoonOrigin, Population, RecordSection, SectionState,
};
use crate::planetary::satellites::{MoonPlane, generate_satellites};
use crate::planetary::testing::{SampleFilter, sample_contexts, synthetic_binary, synthetic_star};
use crate::stellar::Phase;
use crate::stellar::multiplicity::star_positions_at;
use crate::time::{CLOCK_WINDOW_H, ClockWindow, Span};
use crate::units::{AstronomicalUnits, Dex, Kelvin, Pascals, Years};

/// The universe every test generates in.
pub(crate) const SEED: Seed = Seed::new(0x5eed_0014_0030);

/// How many real systems the ordinary suite samples.
const SAMPLE: usize = 400;

/// The real systems nearest the solar-circle point, built once for every test.
pub(crate) fn sample() -> &'static [SystemContext] {
    static CONTEXTS: OnceLock<Vec<SystemContext>> = OnceLock::new();
    CONTEXTS.get_or_init(|| sample_contexts(SAMPLE, SEED, SampleFilter::ALL).unwrap())
}

/// The sample's systems, generated once, under the escape's hold whatever rules the test that
/// first asks has set ([`with_rules`](crate::planetary::chem::escape::with_rules)), so that the
/// cache is version 23's.
pub(crate) fn generated() -> &'static [(SystemContext, PlanetarySystem)] {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    static SYSTEMS: OnceLock<Vec<(SystemContext, PlanetarySystem)>> = OnceLock::new();
    SYSTEMS.get_or_init(|| {
        with_rules(EscapeRules::Held, || {
            sample()
                .iter()
                .map(|ctx| (ctx.clone(), generate_planets(SEED, ctx)))
                .collect()
        })
    })
}

/// The sample's systems, generated whole (P14.T30.a's `generate`), once, under the escape's hold
/// whatever rules the test that first asks has set
/// ([`with_rules`](crate::planetary::chem::escape::with_rules)), so that the cache is version
/// 23's.
pub(crate) fn whole() -> &'static [(SystemContext, PlanetarySystem)] {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    static SYSTEMS: OnceLock<Vec<(SystemContext, PlanetarySystem)>> = OnceLock::new();
    SYSTEMS.get_or_init(|| {
        with_rules(EscapeRules::Held, || {
            sample()
                .iter()
                .map(|ctx| (ctx.clone(), generate(SEED, ctx)))
                .collect()
        })
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

/// P14.T53.a: `generate_is_order_independent` under the escape's batch, with both 24 holds lifted
/// (the Rayleigh floor's too): its thermostat's and its ceiling's solves read nothing a system
/// leaves behind.
#[test]
fn under_the_escape_batch_generate_is_order_independent() {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    let contexts: Vec<SystemContext> = sample().iter().take(40).cloned().collect();
    assert_order_independent(&contexts, |ctx| {
        with_rules(EscapeRules::Batch, || generate_planets(SEED, ctx))
    });
}

#[test]
fn generate_is_the_same_twice_and_order_independent() {
    for (ctx, system) in whole().iter().take(40) {
        assert_eq!(&generate(SEED, ctx), system);
    }
    let contexts: Vec<SystemContext> = sample().iter().take(30).cloned().collect();
    assert_order_independent(&contexts, |ctx| generate(SEED, ctx));
}

/// P14.T30.a: `generate_planets` is `generate` with the satellites, belts and halo removed, so
/// that phase D moves no planet.
#[test]
fn generate_planets_is_generate_with_its_satellites_removed() {
    let (mut moons, mut belts, mut halos) = (0, 0, 0);
    for ((ctx, planets), (_, whole)) in generated().iter().zip(whole()) {
        assert_eq!(planets.zones(), whole.zones(), "{:?}", ctx.id());
        assert_eq!(planets.hosts(), whole.hosts());
        // `generate` attaches the held rotation laws (P14.T46.b) and the satellites' Laplace frames
        // (P14.T56.a) after phase D, and `generate_planets` holds neither; every other part of a
        // planet is the same.
        let kept: Vec<Body> = whole
            .planets()
            .map(|body| Body {
                rotation: None,
                laplace: None,
                ..body.clone()
            })
            .collect();
        assert_eq!(kept, planets.bodies());
        assert!(planets.belts().is_empty() && planets.halo().is_none());
        moons += whole
            .bodies()
            .iter()
            .filter(|b| matches!(b.kind(), BodyKind::Moon(_)))
            .count();
        belts += whole.belts().len();
        halos += usize::from(whole.halo().is_some());
    }
    assert!(
        moons > 100 && belts > 50 && halos > 20,
        "{moons} moons, {belts} belts, {halos} halos"
    );
}

/// P14.T30.a: `generate_satellites` for a planet equals its children in `generate`, over 1,000
/// systems.
#[test]
fn generate_satellites_is_each_planet_s_children_in_generate() {
    let contexts = sample_contexts(1_000, Seed::new(0x5a7e_1117), SampleFilter::ALL).unwrap();
    let mut compared = 0;
    for ctx in &contexts {
        let system = generate(SEED, ctx);
        for planet in system.planets() {
            let alone = generate_satellites(SEED, ctx, planet, system.nearest_belt(planet.index()));
            assert_eq!(
                alone,
                system.satellites_of(planet.index()),
                "{:?}",
                planet.index()
            );
            let children: Vec<BodyIndex> =
                system.children(planet.index()).map(Body::index).collect();
            let expected: Vec<BodyIndex> = alone
                .moons()
                .iter()
                .map(Satellite::index)
                .chain(alone.rings().iter().map(Ring::index))
                .collect();
            let mut sorted = expected.clone();
            sorted.sort_unstable();
            assert_eq!(children, sorted);
            compared += 1;
        }
    }
    assert!(compared > 1_000, "{compared} planets");
}

/// P14.T30.a with ruling 95.1: a belt member's satellites alone equal its children in `generate`,
/// its one possible moon at `Member(0x80 + k)`, whose index names the member as its parent.
#[test]
fn generate_satellites_is_each_member_s_children_in_generate() {
    let contexts = sample_contexts(1_000, Seed::new(0x5a7e_1117), SampleFilter::ALL).unwrap();
    let (mut members, mut moons) = (0u32, 0u32);
    for ctx in &contexts {
        let system = generate(SEED, ctx);
        for member in system.bodies().iter().filter(|b| b.member_of().is_some()) {
            let alone = generate_satellites(SEED, ctx, member, None);
            assert_eq!(alone, system.satellites_of(member.index()));
            assert_eq!(alone.rings(), []);
            assert!(alone.moons().len() <= 1);
            let children: Vec<BodyIndex> =
                system.children(member.index()).map(Body::index).collect();
            let expected: Vec<BodyIndex> = alone.moons().iter().map(Satellite::index).collect();
            assert_eq!(children, expected);
            for moon in alone.moons() {
                let BodySub::Member(k) = member.index().sub() else {
                    panic!("a member's index is a member's");
                };
                assert_eq!(moon.index().slot(), member.index().slot());
                assert_eq!(moon.index().sub(), BodySub::Member(k + 0x80));
                assert_eq!(moon.index().parent(), Some(member.index()));
                assert_eq!(moon.origin(), MoonOrigin::GiantImpact);
                moons += 1;
            }
            members += 1;
        }
    }
    assert!(members > 100, "{members} members");
    // 567 of 7,985 members in this sample have a giant-impact moon.
    assert!(moons > 100, "{moons} member moons among {members} members");
}

#[test]
fn every_index_decodes_is_unique_and_none_is_at_the_stellar_level() {
    let mut bodies = 0_u32;
    for (_, system) in whole() {
        let mut previous: Option<BodyIndex> = None;
        for body in system.bodies() {
            let index = body.index();
            let (slot, sub) = BodyIndex::decode(index.get()).unwrap();
            assert_eq!((slot, sub), (index.slot(), index.sub()));
            match body.kind() {
                BodyKind::Planet => {
                    assert!(matches!(slot, BodySlot::Planet(n) if n >= 1), "{index:?}");
                    assert_eq!(sub, BodySub::Primary);
                }
                BodyKind::Moon(_) => assert!(
                    matches!((slot, sub), (BodySlot::Planet(_), BodySub::Moon(_)))
                        || matches!((slot, sub), (BodySlot::Belt(_), BodySub::Member(n)) if n > 0x80),
                    "{index:?}"
                ),
                BodyKind::Ring => assert!(matches!(sub, BodySub::Ring(_)), "{index:?}"),
                BodyKind::DwarfPlanet => {
                    assert!(matches!(
                        (slot, sub),
                        (BodySlot::Belt(_), BodySub::Member(_))
                    ));
                }
                other => panic!("{other:?} among the bodies"),
            }
            assert!(previous.is_none_or(|p| p < index), "sorted and unique");
            assert_eq!(system.body(index), Some(body));
            previous = Some(index);
            bodies += 1;
        }
        let indices = system.indices();
        let mut unique = indices.clone();
        unique.dedup();
        assert_eq!(indices, unique);
        for belt in system.belts() {
            assert!(matches!(belt.index().slot(), BodySlot::Belt(1..=13)));
            assert_eq!(belt.index().sub(), BodySub::Primary);
        }
        if let Some(halo) = system.halo() {
            assert_eq!(halo.index().slot(), BodySlot::Belt(15));
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
        let system = generate(SEED, &ctx);
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
                .filter_map(Body::placed)
                .collect();
            assert_eq!(bodies, placed.planets().iter().collect::<Vec<_>>());
            for body in system.bodies().iter().filter(|b| b.host() == zone.host()) {
                let body_id = body.index().body_id(id);
                assert_eq!(body.radius_rank(), Some(radius_rank(seed, body_id)));
                let formation =
                    Formation::draw(seed, body_id, body.mass(), inputs.lifetime()).unwrap();
                assert_eq!(body.formation(), Some(&formation));
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
            assert!(
                body.orbit().unwrap().apoapsis() <= strip,
                "{:?}",
                body.index()
            );
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
    for (ctx, system) in whole().iter().take(80) {
        for t in window() {
            let snapshot = system.snapshot_at(ctx, t);
            assert_eq!(snapshot.system(), system.system());
            assert_eq!(snapshot.time(), t);
            assert_eq!(snapshot.bodies().len(), system.indices().len());
            for (record, index) in snapshot.bodies().iter().zip(system.indices()) {
                let alone = system.body_at(ctx, index, t).unwrap();
                assert_eq!(record, &alone);
                // Bit for bit, signed zeros included: the surface section's floats with the rest.
                assert_eq!(format!("{record:?}"), format!("{alone:?}"));
                assert_eq!(
                    record.position(),
                    system.position_at(ctx, index, t).unwrap()
                );
            }
        }
    }
}

/// The section states (ruling 34): what is not computed says so, what does not apply to a kind
/// is not applicable, and phase D's sections are `Ok` where modelled (P14.T34).
#[test]
fn a_record_tags_what_the_generator_does_not_compute() {
    let (mut present, mut giants, mut moons, mut rings, mut dwarfs) = (0, 0, 0, 0, 0);
    for (ctx, system) in whole().iter().take(120) {
        let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
        let belts: Vec<BodyIndex> = system.belts().iter().map(Belt::index).collect();
        assert_eq!(snapshot.belts(), &Section::Ok(belts));
        assert_eq!(
            snapshot.halo(),
            &Section::Ok(system.halo().map(CometaryHalo::index))
        );
        assert_eq!(snapshot.bodies().len(), system.indices().len());
        for record in snapshot.bodies() {
            let identity = record.identity();
            assert_eq!(identity.id(), record.index().body_id(system.system()));
            assert_eq!(identity.label().state(), SectionState::Ok);
            assert_eq!(record.level(), DetailLevel::Full);
            match identity.kind() {
                BodyKind::Planet => {
                    let body = system.body(record.index()).unwrap();
                    assert_eq!(identity.parent(), Some(body.host()));
                    assert_eq!(record.section_state(RecordSection::Moons), SectionState::Ok);
                    assert_eq!(record.section_state(RecordSection::Rings), SectionState::Ok);
                    assert_eq!(
                        record.section_state(RecordSection::Population),
                        SectionState::NotApplicable
                    );
                    if identity.state() == BodyState::Present {
                        present += 1;
                        if !assert_surface_or_envelope(record) {
                            giants += 1;
                        }
                        assert!(record.position().is_some());
                        assert_eq!(record.section_state(RecordSection::Orbit), SectionState::Ok);
                    }
                }
                BodyKind::Moon(_) => {
                    moons += 1;
                    assert!(matches!(identity.parent(), Some(OrbitHost::Body(_))));
                    if identity.state() == BodyState::Present {
                        assert_eq!(record.section_state(RecordSection::Bulk), SectionState::Ok);
                        assert_eq!(record.section_state(RecordSection::Orbit), SectionState::Ok);
                        assert!(assert_surface_or_envelope(record), "{:?}", record.index());
                    } else {
                        assert_eq!(
                            record.section_state(RecordSection::Envelope),
                            SectionState::NotApplicable
                        );
                    }
                }
                BodyKind::Ring => {
                    rings += 1;
                    if identity.state() == BodyState::Present {
                        assert!(matches!(
                            record.population(),
                            Section::Ok(Population::Ring(_))
                        ));
                    }
                }
                BodyKind::Belt(_) => {
                    if identity.state() == BodyState::Present {
                        assert!(matches!(
                            record.population(),
                            Section::Ok(Population::Belt(_))
                        ));
                    }
                    assert_eq!(record.position(), None);
                }
                BodyKind::CometaryHalo => {
                    assert_eq!(
                        record.section_state(RecordSection::Mass),
                        SectionState::NotModelled
                    );
                }
                BodyKind::DwarfPlanet => {
                    assert_eq!(
                        record.section_state(RecordSection::Rings),
                        SectionState::NotModelled
                    );
                    if identity.state() == BodyState::Present {
                        dwarfs += 1;
                        assert!(assert_surface_or_envelope(record), "{:?}", record.index());
                    }
                }
                other => panic!("{other:?} in a snapshot"),
            }
        }
    }
    assert!(
        present > 100 && giants > 5 && moons > 20 && rings > 5 && dwarfs > 5,
        "{present} present, {giants} giants, {moons} moons, {rings} rings, {dwarfs} dwarf planets"
    );
}

/// P14.T48.e: a record's surface is rebuilt from its parts bit for bit, its regime the
/// conditions' own and its condensates (P14.T54.a) the published ones it was built with, and
/// `Surface::new` refuses the gas-envelope state and conditions before their second stage, which
/// have no regime.
///
/// From generator version 24 (P14.T53.b part 2), the invariant `CoarseInputs::for_body` relies
/// on, that no `Ok` surface lies where there is none (decision-p14-t35e-wire §2.4), also holds
/// under a supercritical envelope: with every landed 24 hold in force, over the same systems and
/// the census body's ([`SUPERCRITICAL_METHANE_BODY`]) generated afresh, no `Ok` surface lies under
/// one, and every solid class whose surface is not applicable lies under one, its envelope not
/// modelled ([`assert_envelope_tags`]).
#[test]
fn a_surface_holds_a_solid_state_and_its_conditions_regime() {
    let mut checked = 0;
    for (ctx, system) in whole().iter().take(10) {
        for record in system.snapshot_at(ctx, UniverseTime::EPOCH).bodies() {
            let Section::Ok(surface) = record.surface() else {
                continue;
            };
            let (state, material) = (surface.state(), surface.material());
            let (conditions, figures) = (surface.conditions().clone(), surface.figures().clone());
            let condensates = surface.condensates();
            assert_ne!(state, SurfaceState::GasEnvelope);
            assert_eq!(
                surface.conditions().climate_regime().as_ref(),
                Some(surface.climate_regime())
            );
            // With the basis its record built it on (rule (f)), and its crust (P14.T54.a's C3).
            let rebuilt = Surface::new(
                state,
                material,
                conditions.clone(),
                figures.clone(),
                condensates.clone(),
            )
            .map(|rebuilt| rebuilt.with_basis(*surface.basis().unwrap()))
            .map(|rebuilt| rebuilt.with_crust(surface.crust().ok().map(|crust| **crust)))
            // And its air's vertical structure (P14.T54.b), not modelled while the air is held.
            .map(|rebuilt| {
                rebuilt.with_vertical_structure(match surface.vertical_structure() {
                    Modelled::Ok(structure) => Modelled::Ok(structure.cloned()),
                    Modelled::NotModelled => Modelled::NotModelled,
                })
            });
            assert_eq!(rebuilt.as_ref(), Some(surface));
            assert_eq!(
                Surface::new(
                    SurfaceState::GasEnvelope,
                    material,
                    conditions.clone(),
                    figures.clone(),
                    condensates.clone(),
                ),
                None
            );
            assert_eq!(
                Surface::new(
                    state,
                    material,
                    conditions.without_climate(),
                    figures,
                    condensates.clone()
                ),
                None
            );
            checked += 1;
        }
    }
    assert!(checked > 20, "{checked}");

    let census_body = supercritical_methane_context();
    let sets: Vec<(&str, Seed, &SystemContext)> = whole()
        .iter()
        .take(10)
        .map(|(ctx, _)| ("sample", SEED, ctx))
        .chain(core::iter::once(("sample", SEED, census_body)))
        .collect();
    let bodies = crate::planetary::testing::with_every_24_hold(|| envelope_census(&sets));
    let (mut surfaces, mut hidden) = (0_u32, 0_u32);
    for body in &bodies {
        assert_envelope_tags(body);
        surfaces += u32::from(body.surface == SectionState::Ok);
        hidden += u32::from(body.hidden.is_some());
    }
    assert!(
        surfaces > 20 && hidden >= 1,
        "{surfaces} surfaces, {hidden} hidden"
    );
}

/// P14.T54.a: a record's surface section holds P14.T51.a's published condensates, the ones its
/// liquids and ices read ([`check_record_condensates`]), by the cover's rule W1–W4, exactly
/// ([`check_cover_rule`]; decision-p14-t54a-water-consistency §4), and an Earth's condensates hold
/// `H2O`: every temperate world with liquid water whose figures list a water ocean holds a liquid
/// `H2O` entry with a reservoir, a layer and an area, which the test prints.
///
/// Water's sea is its own layer's fill (§1): `01ffeb2ce0000000.0300`, whose ice holds its water,
/// publishes no sea, and R09's inputs read none. Every water sea is printed against the whole
/// water's potential ocean, which it took before, for the bless note.
#[test]
fn the_record_s_surface_holds_its_published_condensates() {
    use crate::planetary::chem::partition::CondensatePhase;
    let water = Gas::Water.substance();
    let (mut surfaces, mut entries, mut earths) = (0_u32, 0_usize, 0_u32);
    let mut tally = CondensateTally::default();
    let mut cover = CoverRuleTally::default();
    let mut ice_bulks = IceBulkTally::default();
    let mut dry = false;
    for (ctx, system) in whole().iter().take(60) {
        let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
        for record in snapshot.bodies() {
            let Section::Ok(surface) = record.surface() else {
                continue;
            };
            let id = record.identity().id();
            let condensates = check_record_condensates(id, surface, &mut tally);
            check_cover_rule(id, surface, None, &mut cover);
            check_ice_bulk(id, surface, &mut ice_bulks);
            if let Some(sea) = condensates.entry(water, CondensatePhase::Liquid) {
                eprintln!(
                    "{id}: water's sea {:.6} (the whole water's potential ocean {:.6}), {:?} m, a \
                     {:?} at {:.2} K",
                    sea.area_fraction(),
                    surface.figures().potential_ocean(),
                    sea.reservoir_gel().map(crate::units::Metres::value),
                    surface.state(),
                    surface.conditions().mean_surface_temperature().value()
                );
            }
            if id.to_string() == "01ffeb2ce0000000.0300" {
                assert!(
                    condensates.entry(water, CondensatePhase::Liquid).is_none(),
                    "{id}: {:?}",
                    condensates.entries()
                );
                let inputs = r09_inputs(record);
                assert!(inputs.liquids().is_empty(), "{id}: {:?}", inputs.liquids());
                assert_same_bits(inputs.ocean_fraction(), 0.0);
                dry = true;
            }
            let ocean = surface
                .figures()
                .surface_liquids()
                .iter()
                .any(|&(s, f)| s == water && f > 0.0);
            if surface.state() == SurfaceState::Temperate
                && surface.conditions().liquid_water()
                && ocean
            {
                let sea = condensates
                    .entry(water, CondensatePhase::Liquid)
                    .unwrap_or_else(|| panic!("{id}: an Earth's condensates hold its water"));
                assert!(
                    sea.reservoir().value() > 0.0
                        && sea.reservoir_gel().is_some_and(|gel| gel.value() > 0.0)
                        && sea.area_fraction() > 0.0,
                    "{id}: {sea:?}"
                );
                eprintln!(
                    "an Earth, {id}, at {:.2} K and {:.0} Pa: {:?}",
                    surface.conditions().mean_surface_temperature().value(),
                    surface.conditions().surface_pressure().value(),
                    condensates
                        .entries()
                        .iter()
                        .map(described)
                        .collect::<Vec<_>>()
                );
                earths += 1;
            }
            surfaces += 1;
            entries += condensates.entries().len();
        }
    }
    eprintln!(
        "{surfaces} surfaces, {entries} entries ({} supercritical; with no layer {:?}), {earths} \
         Earths; the cover's appended water ice (W3 a) {}, ocean under ice (W3 b) {}, scaled \
         lists {}",
        tally.supercritical, tally.unlayered, cover.crust_ice, cover.under_ice, cover.scaled
    );
    eprintln!(
        "ice bulks: {} surfaces, {} listing their crust's frozen water as H2O solid, {} T24.b's \
         surface water, none listed {:?}",
        ice_bulks.surfaces, ice_bulks.crust, ice_bulks.surface_water, ice_bulks.dry
    );
    assert!(
        surfaces > 100 && entries > 100 && earths > 0,
        "{surfaces} {entries} {earths}"
    );
    assert!(ice_bulks.crust > 100, "{ice_bulks:?}");
    assert!(dry, "01ffeb2ce0000000.0300 is in the sample");
}

/// What [`check_ice_bulk`] counts for the bless note: the surfaces whose bulk is ice, those whose
/// `H2O` solid entry is their crust's frozen water, those whose entry is T24.b's surface water,
/// and those that list no water ice, by state.
#[derive(Debug, Default)]
struct IceBulkTally {
    surfaces: u32,
    crust: u32,
    surface_water: u32,
    dry: std::collections::BTreeMap<String, u32>,
}

/// Decision-p14-t54a-ganymede, C1 follow-up §C1.4: on `surface`, the record of `id`, whose bulk is
/// ice and whose surface water holds none (T24.b's, on water's Jeans verdict), an `H2O` solid
/// entry is the crust's frozen water: its area T24.b's ice area and its layer its mass over ice
/// Ih's density at the mean surface temperature, to the bit while the cold traps are held (until
/// P14.T54.b), and no water sea is listed beside it. An ice bulk that lists no water ice is
/// printed, with its state and mean temperature.
fn check_ice_bulk(id: crate::id::BodyId, surface: &Surface, tally: &mut IceBulkTally) {
    use crate::planetary::chem::partition::CondensatePhase;
    use crate::planetary::derive::atmosphere::SurfaceMaterial;
    use crate::planetary::hooks::figures::cover;
    if surface.material() != SurfaceMaterial::Ice {
        return;
    }
    tally.surfaces += 1;
    let water = Gas::Water.substance();
    let (figures, conditions) = (surface.figures(), surface.conditions());
    let mean = conditions.mean_surface_temperature();
    let Some(entry) = surface.condensates().entry(water, CondensatePhase::Solid) else {
        eprintln!(
            "{id}: an ice bulk, a {:?} at {:.2} K, lists no water ice",
            surface.state(),
            mean.value()
        );
        *tally
            .dry
            .entry(format!("{:?}", surface.state()))
            .or_insert(0) += 1;
        return;
    };
    if figures.surface_water().value() > 0.0 {
        tally.surface_water += 1;
        return;
    }
    tally.crust += 1;
    assert!(
        surface
            .condensates()
            .entry(water, CondensatePhase::Liquid)
            .is_none(),
        "{id}: {:?}",
        surface.condensates()
    );
    if entry.trapped_area_fraction() > 0.0 {
        return;
    }
    // Over the ground its condensates were published on (rule (f)).
    let ground = surface.basis().map_or_else(
        || figures.partition_ground(MetresPerSecondSquared::new(0.0), surface.material()),
        |basis| basis.ground,
    );
    let ice = cover::ice_fraction(&cover::IceInputs {
        conditions,
        state: surface.state(),
        water_state: figures.water_state(),
        crust: ground.crust,
        surface_water: ground.surface_water,
    });
    assert_same_bits(entry.area_fraction(), ice);
    let rho = crate::substance::ice_ih_density(mean);
    let layer = entry.reservoir_gel().expect("ice Ih's density is read");
    assert_same_bits(layer.value(), entry.reservoir().value() / rho.value());
}

/// What [`check_cover_rule`] counts for the bless note: the surfaces whose ices take T24.b's water
/// ice with an area and no `H2O` solid entry (W3 (a)), whose liquids take a `Snowball`'s ocean
/// under its ice (W3 (b)), and the lists scaled to the surface (W2); and, where it reads the
/// partition, the water follow-up check's ([`liquid_stability`]).
#[derive(Debug, Default)]
struct CoverRuleTally {
    crust_ice: u32,
    under_ice: u32,
    scaled: u32,
    liquids: LiquidTally,
}

/// Decision-p14-t54a-water-consistency §4.2: `surface`'s liquids and ices, the record of `id`, are
/// the cover its condensates give, exactly.
///
/// - **W1–W3.** Each list is its phase's entries in their order, with at most one `H2O` item
///   appended: in the ices T24.b's water ice where no `H2O` solid is listed, in the liquids a
///   `Snowball`'s ocean under its ice at its potential ocean where no `H2O` liquid is; each divided
///   by the list's total, the appended item included, where that total exceeds 1, and otherwise to
///   the bit. Nothing else is listed.
/// - **W4.** Water's sea is the share of the basins its own layer fills, to the bit, and the other
///   liquids share the basins by volume; no entry has no mass but a supercritical one, which
///   covers nothing; a solid's area is at most its layer spread at the sheet's thickness, but
///   water on an ice-rich crust or under a `Snowball`'s cover, which T24.b does not cap, and an
///   entry with no layer, whose density is not read.
/// - **W5.** Each list sums to at most 1.
/// - **The water follow-up check** (decision-p14-t54a-water-consistency, "Batch B follow-up"),
///   given the partition `partition` the condensates were published from, the published pass's
///   (rule (f)): every liquid entry is liquid where it lies, and every phase `condensate_of`'s
///   fallback set is listed ([`liquid_stability`]).
fn check_cover_rule(
    id: crate::id::BodyId,
    surface: &Surface,
    partition: Option<&crate::planetary::chem::partition::Partition>,
    tally: &mut CoverRuleTally,
) {
    use crate::planetary::chem::partition::{Basins, CondensatePhase};
    use crate::planetary::hooks::figures::cover;
    let water = Gas::Water.substance();
    let (figures, conditions) = (surface.figures(), surface.conditions());
    let condensates = surface.condensates();
    // The ground its condensates were published on: its published pass's context's (rule (f)).
    let ground = surface.basis().map_or_else(
        || figures.partition_ground(MetresPerSecondSquared::new(0.0), surface.material()),
        |basis| basis.ground,
    );
    let mut liquids = condensates.surface_liquids();
    let under_ice = cover::under_ice_ocean(
        surface.state(),
        figures.surface_water(),
        conditions.mean_surface_temperature(),
        figures.heat_flow(),
    );
    if under_ice && condensates.entry(water, CondensatePhase::Liquid).is_none() {
        liquids.push((water, figures.potential_ocean()));
        tally.under_ice += 1;
    }
    let mut ices = condensates.surface_ices();
    let crust = ground.crust;
    if condensates.entry(water, CondensatePhase::Solid).is_none() {
        let ice = cover::ice_fraction(&cover::IceInputs {
            conditions,
            state: surface.state(),
            water_state: figures.water_state(),
            crust,
            surface_water: ground.surface_water,
        });
        ices.push((water, ice));
        tally.crust_ice += u32::from(ice > 0.0);
    }
    for (raw, listed) in [
        (liquids, figures.surface_liquids()),
        (ices, figures.surface_ices()),
    ] {
        let total = raw.iter().fold(0.0, |sum, &(_, f)| sum + f);
        assert_eq!(
            raw.iter().map(|&(s, _)| s).collect::<Vec<_>>(),
            listed.iter().map(|&(s, _)| s).collect::<Vec<_>>(),
            "{id}: {raw:?} against {listed:?}"
        );
        for (&(_, f), &(_, area)) in raw.iter().zip(listed) {
            assert_same_bits(area, if total > 1.0 { f / total } else { f });
        }
        tally.scaled += u32::from(total > 1.0);
        let sum = listed.iter().fold(0.0, |sum, &(_, f)| sum + f);
        assert!(sum <= 1.0 + 1e-12, "{id}: {listed:?}");
    }
    // The fractions are shares of the surface, held at 1 though a scaled list's sum may round an
    // ulp over it (as `filler_b`'s did at C0, 1.000 000 000 000 000 2).
    for fraction in [figures.ocean_fraction(), figures.ice_fraction()] {
        assert!((0.0..=1.0).contains(&fraction), "{id}: {fraction}");
    }
    let basins: Basins = *ground.ground.basins();
    let (mut others, mut other_area) = (0.0, 0.0);
    for e in condensates.entries() {
        match e.phase() {
            CondensatePhase::Supercritical => {
                assert_same_bits(e.reservoir().value(), 0.0);
                assert_same_bits(e.area_fraction(), 0.0);
                continue;
            }
            CondensatePhase::Liquid | CondensatePhase::Solid => {
                assert!(e.reservoir().value() > 0.0, "{id}: {e:?}");
            }
        }
        let Some(layer) = e.reservoir_gel() else {
            continue;
        };
        match e.phase() {
            CondensatePhase::Liquid if e.substance() == water => {
                assert_same_bits(e.area_fraction(), basins.area_filled(layer));
            }
            CondensatePhase::Liquid => {
                others += layer.value();
                other_area += e.area_fraction();
            }
            CondensatePhase::Solid => {
                let uncapped = e.substance() == water
                    && (crust == Crust::IceRich || surface.state() == SurfaceState::Snowball);
                let spread = layer.value() / cover::ICE_SHEET_THICKNESS.value();
                assert!(
                    uncapped || e.area_fraction() <= spread * (1.0 + 1e-12),
                    "{id}: {e:?} over {spread}"
                );
            }
            CondensatePhase::Supercritical => {}
        }
    }
    let filled = basins.area_filled(crate::units::Metres::new(others));
    assert!(
        (other_area - filled).abs() <= 1e-12,
        "{id}: the other liquids cover {other_area}, their layers fill {filled}"
    );
    if let Some(partition) = partition {
        liquid_stability(&id.to_string(), surface, partition, &mut tally.liquids);
    }
}

/// What [`liquid_stability`] counts and lists: the liquid entries checked, the phases
/// `condensate_of`'s fallback set (each with its body, `p_s` and `p_sat`), and the liquid entries
/// with an area that would boil or lie at or above their critical temperature
/// (decision-p14-t54a-water-consistency, "Batch B follow-up", §F.1).
#[derive(Debug, Default)]
struct LiquidTally {
    liquids: u32,
    fallbacks: Vec<String>,
    boiling: Vec<String>,
}

/// The phase a reservoir of the substance of `diagram`, whose frozen assemblage first melts at
/// `first_melt` and whose partial pressure is `partial`, takes at `temperature` under the surface
/// pressure `pressure`, or `None` where it cannot lie condensed there: `chem::partition`'s
/// `condensed_phase_at`, written again from the registry's diagram as the check's oracle.
fn phase_there(
    diagram: &crate::substance::diagram::PhaseDiagram,
    first_melt: Kelvin,
    temperature: Kelvin,
    (pressure, partial): (crate::units::Pascals, crate::units::Pascals),
) -> Option<crate::planetary::chem::partition::CondensatePhase> {
    use crate::planetary::chem::partition::CondensatePhase;
    if temperature >= diagram.critical_temperature() {
        return (partial >= diagram.critical_pressure()).then_some(CondensatePhase::Supercritical);
    }
    let triple = diagram.triple_temperature();
    match diagram
        .phase_at(temperature, pressure)
        .and_then(CondensatePhase::of)
    {
        Some(CondensatePhase::Solid)
            if first_melt < triple && temperature >= first_melt && temperature < triple =>
        {
            Some(CondensatePhase::Liquid)
        }
        phase => phase,
    }
}

/// Decision-p14-t54a-water-consistency, "Batch B follow-up" §F.1 (the bless note's
/// water-consistency follow-up): on `surface`, the record of `what`, whose condensates its
/// partition `partition` published (rule (f): its published pass's), every liquid entry with an
/// area lies where its substance is liquid at the temperature it lies at and the surface
/// pressure, `p_s` ≥ `p_sat`(T) with T below its critical temperature; the liquid part of a
/// reservoir that lies solid lies at the warmest zone, where it melts. Every condensate whose phase
/// `condensate_of`'s fallback set (none at the temperature it lies at: the coldest zone's, else its
/// first melt's rule) is listed with its entries' areas, `p_s` and `p_sat`.
fn liquid_stability(
    what: &str,
    surface: &Surface,
    partition: &crate::planetary::chem::partition::Partition,
    tally: &mut LiquidTally,
) {
    use crate::planetary::chem::partition::CondensatePhase;
    let p_s = partition.surface_pressure();
    for c in partition.condensates() {
        let Some(diagram) = c.substance().substance().phase_diagram() else {
            continue;
        };
        if c.phase() == CondensatePhase::Supercritical || c.reservoir().value() <= 0.0 {
            continue;
        }
        let partial = partition.of(c.substance()).map_or(
            crate::units::Pascals::ZERO,
            crate::planetary::chem::partition::Partitioned::partial_pressure,
        );
        let its = |e: &&crate::planetary::chem::partition::CondensateEntry| {
            e.substance() == c.substance() || e.substance() == c.liquid()
        };
        let t = c.lies_at();
        if phase_there(diagram, c.first_melt(), t, (p_s, partial)).is_none() {
            let entries: Vec<String> = surface
                .condensates()
                .entries()
                .iter()
                .filter(its)
                .map(|e| format!("{:?} over {:.6}", e.phase(), e.area_fraction()))
                .collect();
            tally.fallbacks.push(format!(
                "{what}: {} {:?} (its liquid {}), lies at {:.2} K, coldest {:.2} K, p_s {:.6e} \
                 Pa, p_sat {:.6e} Pa there; entries {entries:?}",
                c.substance().substance().key(),
                c.phase(),
                c.liquid().substance().key(),
                t.value(),
                partition.climate().coldest().value(),
                p_s.value(),
                diagram.saturation_pressure(t).value(),
            ));
        }
        let liquids = surface.condensates().entries().iter().filter(its);
        for e in liquids.filter(|e| e.phase() == CondensatePhase::Liquid) {
            tally.liquids += 1;
            let at = if c.phase() == CondensatePhase::Liquid {
                t
            } else {
                partition.climate().warmest()
            };
            let p_sat = diagram.saturation_pressure(at);
            let liquid = p_s >= p_sat && at < diagram.critical_temperature();
            if !liquid && e.area_fraction() > 0.0 {
                tally.boiling.push(format!(
                    "{what}: {} over {:.6} at {:.2} K, p_s {:.6e} Pa against p_sat {:.6e} Pa, T_c \
                     {:.2} K",
                    e.substance().substance().key(),
                    e.area_fraction(),
                    at.value(),
                    p_s.value(),
                    p_sat.value(),
                    diagram.critical_temperature().value(),
                ));
            }
            if what == "01ffeb2ce0000000.0400" && e.substance() == Gas::Water.substance() {
                eprintln!(
                    "{what}: water's sea over {:.6} lies at {:.3} K under p_s {:.4} bar against \
                     p_sat {:.4} bar there (water's p_sat at 573.8 K by the registry: {:.4} bar; \
                     86.67 bar by IAPWS-95)",
                    e.area_fraction(),
                    at.value(),
                    p_s.value() / 1e5,
                    p_sat.value() / 1e5,
                    diagram.saturation_pressure(Kelvin::new(573.8)).value() / 1e5,
                );
            }
        }
    }
}

/// The condensables the air `air` holds at their cap: each gas with a phase diagram whose airborne
/// weight lies below its inventory's, some of it condensed, by P14.T24.a's greenhouse gain's own
/// test (`greenhouse_gain`'s `Ordering::Less`). A change of the set between passes is an onset
/// (decision-pre-23-review item 22).
fn capped_gases(air: &crate::planetary::derive::Atmosphere) -> Vec<Gas> {
    Gas::ALL
        .into_iter()
        .filter(|&gas| {
            gas.substance().substance().phase_diagram().is_some()
                && matches!(
                    air.partial_pressures()
                        .of(gas)
                        .partial_cmp(&air.inventory_weights().of(gas)),
                    Some(core::cmp::Ordering::Less)
                )
        })
        .collect()
}

/// The members of the discrete key that differ between `a` and `b` (decision-p14-t54a-passes
/// §1.5): the figures present, the basins' shape, the water share's outgassing branch, the regime,
/// the craters' crust class, a moon's tide branch, the inertia lid and `lid_of`'s unresurfaced
/// test.
fn key_members(
    a: &crate::planetary::derive::pass_record::PassContextKey,
    b: &crate::planetary::derive::pass_record::PassContextKey,
) -> Vec<&'static str> {
    [
        ("figures", a.figures != b.figures),
        ("basins", a.basins != b.basins),
        ("water share", a.volcanic != b.volcanic),
        ("regime", a.regime != b.regime),
        ("crust class", a.crust != b.crust),
        ("tide branch", a.tide != b.tide),
        ("lid", a.lid != b.lid),
        ("resurfacing", a.unresurfaced != b.unresurfaced),
    ]
    .into_iter()
    .filter_map(|(member, differs)| differs.then_some(member))
    .collect()
}

/// R09's coarse inputs of `record` (`CoarseInputs::from_record`), with a basalt lid and an entry
/// for each ice on the ground, as its documentation's example builds them: for a planet, whose
/// seasons are its own orbit's.
fn r09_inputs(record: &BodyRecord) -> crate::planetary::surface::CoarseInputs {
    use crate::planetary::surface::{CoarseInputs, CrustInputs, LidCrust, SeasonsFromRecord};
    use hyperion_surface::field::{MaterialPalette, MechanicsFamily, PaletteEntry, PaletteRole};
    use hyperion_surface::substance_key::SubstanceKey;
    let Section::Ok(surface) = record.surface() else {
        panic!("{:?} has a surface", record.index());
    };
    let basalt = SubstanceKey::new("basalt").unwrap();
    let entry = |substance, role| PaletteEntry {
        substance,
        role,
        normal_albedo_bvr: [0.1; 3],
        phase_row: 0,
        grain_density_kg_m3: 3_010.0,
        transition: Kelvin::new(1_253.15),
        mechanics: MechanicsFamily::Silicate,
        liquid: None,
        feo_mass_fraction: None,
    };
    let mut entries = vec![entry(basalt, PaletteRole::SecondaryCrust)];
    let ices = surface.figures().surface_ices().iter();
    let on_the_ground = ices.filter(|&&(_, area_fraction)| area_fraction > 0.0);
    entries.extend(on_the_ground.map(|&(id, _)| entry(id.substance().key(), PaletteRole::Ice)));
    let crust = CrustInputs {
        primary: None,
        secondary: Some(basalt),
        tertiary: None,
        bulk: None,
        lid: LidCrust::Secondary,
        provinces: basalt,
        melt_area_fraction: 0.0,
    };
    CoarseInputs::from_record(
        record,
        SeasonsFromRecord::Own,
        crust,
        [],
        MaterialPalette::new(entries).unwrap(),
    )
    .unwrap()
}

/// What [`check_record_condensates`] counts: the supercritical entries, and the entries with no
/// layer by substance and phase.
#[derive(Debug, Default)]
struct CondensateTally {
    supercritical: u32,
    unlayered: std::collections::BTreeMap<
        (String, crate::planetary::chem::partition::CondensatePhase),
        u32,
    >,
}

/// A published condensate, printed: its substance, phase, reservoir, layer, area and flag.
fn described(e: &crate::planetary::chem::partition::CondensateEntry) -> String {
    format!(
        "{} {:?} {:.4e} kg m⁻² ({:?} m), area {:.4}, seasonal {}",
        e.substance().substance().key(),
        e.phase(),
        e.reservoir().value(),
        e.reservoir_gel().map(crate::units::Metres::value),
        e.area_fraction(),
        e.is_seasonal()
    )
}

/// P14.T54.a: the condensates of `surface`, the record of `id`, which it holds `Ok`, checked. There
/// are at most `MAX_CONDENSATES` entries, each substance and phase once, in `Partition::publish`'s
/// order (by reservoir mass, largest first, ties by substance, then phase). Each area is a
/// fraction, each reservoir and each layer, where the registry has read its density, not
/// negative, and a supercritical fluid covers nothing. Every liquid and solid that covers an area
/// is among the figures' liquids or ices, and every substance listed there but water, which keeps
/// T24.b's ice and its ocean under ice where the partition holds none, has its entry.
fn check_record_condensates<'a>(
    id: crate::id::BodyId,
    surface: &'a Surface,
    tally: &mut CondensateTally,
) -> &'a crate::planetary::chem::partition::Condensates {
    use crate::planetary::chem::partition::{CondensateEntry, CondensatePhase};
    use crate::planetary::surface::MAX_CONDENSATES;
    use std::cmp::Ordering;
    let order = |a: &CondensateEntry, b: &CondensateEntry| {
        b.reservoir()
            .value()
            .total_cmp(&a.reservoir().value())
            .then(a.substance().cmp(&b.substance()))
            .then(a.phase().cmp(&b.phase()))
    };
    let condensates = surface.condensates();
    let (figures, list) = (surface.figures(), condensates.entries());
    assert!(list.len() <= MAX_CONDENSATES, "{id}: {list:?}");
    for pair in list.windows(2) {
        assert_eq!(order(&pair[0], &pair[1]), Ordering::Less, "{id}: {list:?}");
    }
    let unique: std::collections::BTreeSet<_> =
        list.iter().map(|e| (e.substance(), e.phase())).collect();
    assert_eq!(
        unique.len(),
        list.len(),
        "{id}: each substance and phase once: {list:?}"
    );
    let listed = |areas: &[(crate::substance::SubstanceId, f64)], e: &CondensateEntry| {
        areas.iter().any(|&(other, _)| other == e.substance())
    };
    for e in list {
        let area = e.area_fraction();
        assert!((0.0..=1.0).contains(&area), "{id}: {e:?}");
        assert!(e.reservoir().value() >= 0.0, "{id}: {e:?}");
        if let Some(gel) = e.reservoir_gel() {
            assert!(gel.value() >= 0.0, "{id}: {e:?}");
        } else {
            let key = (e.substance().substance().key().to_string(), e.phase());
            *tally.unlayered.entry(key).or_insert(0) += 1;
        }
        match e.phase() {
            CondensatePhase::Supercritical => {
                assert_same_bits(area, 0.0);
                tally.supercritical += 1;
            }
            CondensatePhase::Liquid if area > 0.0 => {
                assert!(listed(figures.surface_liquids(), e), "{id}: {e:?}");
            }
            CondensatePhase::Solid if area > 0.0 => {
                assert!(listed(figures.surface_ices(), e), "{id}: {e:?}");
            }
            CondensatePhase::Liquid | CondensatePhase::Solid => {}
        }
    }
    let water = Gas::Water.substance();
    for (areas, phase) in [
        (figures.surface_liquids(), CondensatePhase::Liquid),
        (figures.surface_ices(), CondensatePhase::Solid),
    ] {
        for &(substance, _) in areas.iter().filter(|&&(s, _)| s != water) {
            assert!(
                condensates.entry(substance, phase).is_some(),
                "{id}: {substance:?} {phase:?} listed with no entry"
            );
        }
    }
    condensates
}

/// P14.T48.e's gas-envelope split on a present body's record with a bulk section: a body with a
/// solid surface has its surface section `Ok`, from `DetailLevel::Surface` and not below, and its
/// envelope not applicable, or, under the escape's batch rules alone (from generator version 24),
/// its surface not applicable and its envelope not modelled under a supercritical envelope
/// (P14.T53.b part 2, whose test the record does not carry: the 24 arm of
/// `a_surface_holds_a_solid_state_and_its_conditions_regime` checks it on the derivation); every
/// other its surface not applicable and its envelope not modelled until P14.T24.d, so that exactly
/// one of the two applies. Whether the body's class has a solid surface.
fn assert_surface_or_envelope(record: &BodyRecord) -> bool {
    use crate::planetary::derive::atmosphere::supercritical_section_in_force;
    let index = record.index();
    let bulk = record.bulk().ok().unwrap();
    let surface = record.section_state(RecordSection::Surface);
    let envelope = record.section_state(RecordSection::Envelope);
    let solid = bulk.class().has_solid_surface();
    if solid && surface == SectionState::NotApplicable {
        assert!(
            supercritical_section_in_force(),
            "{index:?}: a solid surface hidden under the hold"
        );
        assert_eq!(envelope, SectionState::NotModelled, "{index:?}");
        let at = |level| record.degrade(level).section_state(RecordSection::Surface);
        assert_eq!(
            at(DetailLevel::Surface),
            SectionState::NotApplicable,
            "{index:?}"
        );
    } else if solid {
        assert_eq!(surface, SectionState::Ok, "{index:?}");
        assert_eq!(envelope, SectionState::NotApplicable, "{index:?}");
        let at = |level| record.degrade(level).section_state(RecordSection::Surface);
        assert_eq!(at(DetailLevel::Surface), SectionState::Ok, "{index:?}");
        assert_eq!(
            at(DetailLevel::Bulk),
            SectionState::NotResolved,
            "{index:?}"
        );
    } else {
        assert_eq!(surface, SectionState::NotApplicable, "{index:?}");
        assert_eq!(envelope, SectionState::NotModelled, "{index:?}");
    }
    solid
}

/// R09.T1.b: a planet, moon or dwarf planet present at the record's time carries its own detail
/// seed in its hooks section, never its surface seed; a ring, a belt, the halo and a body not
/// present carry no hooks.
#[test]
fn a_present_body_s_hooks_are_its_detail_seed() {
    let (mut seeded, mut absent, mut rings, mut belts) = (0, 0, 0, 0);
    for (ctx, system) in whole().iter().take(120) {
        for record in system.snapshot_at(ctx, UniverseTime::EPOCH).bodies() {
            let identity = record.identity();
            let surfaced = matches!(
                identity.kind(),
                BodyKind::Planet | BodyKind::Moon(_) | BodyKind::DwarfPlanet
            );
            if surfaced && identity.state() == BodyState::Present {
                let seed = hooks::detail_seed(system.seed, identity.id());
                assert_eq!(
                    record.hooks(),
                    &Section::Ok(Hooks::new(seed)),
                    "{}",
                    identity.id()
                );
                let surface = system.surface_seed(record.index()).unwrap().unwrap();
                assert_ne!(seed.get(), surface.get(), "{}", identity.id());
                seeded += 1;
            } else {
                assert_eq!(record.hooks(), &Section::NotApplicable, "{}", identity.id());
                if surfaced {
                    absent += 1;
                } else if identity.kind() == BodyKind::Ring {
                    rings += 1;
                } else if matches!(identity.kind(), BodyKind::Belt(_)) {
                    belts += 1;
                }
            }
        }
    }
    eprintln!("{seeded} seeded, {absent} not present, {rings} rings, {belts} belts");
    assert!(
        seeded > 100 && absent > 0 && rings > 5 && belts > 0,
        "{seeded} seeded, {absent} not present, {rings} rings, {belts} belts"
    );
}

/// P14.T30.b: a moon's position is its planet's plus its own offset, and a ring's its planet's.
#[test]
fn a_moon_is_at_its_planet_s_position_plus_its_own_offset() {
    let mut moons = 0;
    for (ctx, system) in whole().iter().take(200) {
        for t in window() {
            for body in system.bodies() {
                let OrbitHost::Body(parent) = body.host() else {
                    continue;
                };
                let Some(at) = system.position_at(ctx, body.index(), t).unwrap() else {
                    continue;
                };
                let centre = system.position_at(ctx, parent, t).unwrap().unwrap();
                let record = system.body_at(ctx, body.index(), t).unwrap();
                let expected = match record.orbit().ok() {
                    Some(orbit) => centre.translated(orbit.trajectory().relative_state_at(t).0),
                    None => centre,
                };
                for (x, y) in at.metres().into_iter().zip(expected.metres()) {
                    assert_same_bits(x, y);
                }
                if record.orbit().ok().is_some() {
                    moons += 1;
                }
            }
        }
    }
    assert!(moons > 50, "{moons} moons");
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
                let (offset, _) = orbit.trajectory().relative_state_at(t);
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

/// R03.T3: `state_at`'s position is `position_at`'s bit for bit, and it is `None` exactly where
/// `position_at` is.
#[test]
fn a_state_s_position_is_position_at_s_bit_for_bit() {
    let mut states = 0;
    for (ctx, system) in whole().iter().take(200) {
        for t in window() {
            for index in system
                .bodies()
                .iter()
                .map(Body::index)
                .chain(system.belts().iter().map(Belt::index))
            {
                let position = system.position_at(ctx, index, t).unwrap();
                let state = system.state_at(ctx, index, t).unwrap();
                assert_eq!(position.is_some(), state.is_some());
                if let (Some(position), Some((same, _))) = (position, state) {
                    for (x, y) in position.metres().into_iter().zip(same.metres()) {
                        assert_same_bits(x, y);
                    }
                    states += 1;
                }
            }
        }
    }
    assert!(states > 1_000, "{states} states");
}

/// R03.T3: the host's velocity on `state_at`'s walk is `star_states_at`'s bit for bit for a
/// single star's zone, as `centre` is `star_positions_at`'s: the two copies of the walk agree.
#[test]
fn a_star_s_zone_moves_with_star_states_at_bit_for_bit() {
    let mut states = Vec::new();
    let mut stars = 0;
    for (ctx, system) in generated() {
        for t in window() {
            let epoch = Epoch::new(system, ctx, t);
            crate::stellar::multiplicity::star_states_at(ctx.hierarchy(), t, &mut states);
            for (n, (_, _, velocity)) in states.iter().enumerate() {
                let centre = epoch.centre_velocity(1 << n);
                for (x, y) in centre
                    .metres_per_second()
                    .into_iter()
                    .zip(velocity.metres_per_second())
                {
                    assert_same_bits(x, y);
                }
                stars += 1;
            }
        }
    }
    assert!(stars > 1_000, "{stars} stars");
}

/// R03.T3: a body's velocity is the time derivative of its position, by central differences over
/// a second, to 10⁻⁶ relative, with a floor for the positions' own rounding, a few ε of the widest
/// orbit on its path (the host's pairs' and its own apocentres, and its distance), which a
/// difference over 2 s carries into the velocity.
#[test]
fn a_body_s_velocity_is_the_derivative_of_its_position() {
    let second = Span::from_seconds(1);
    let mut checked = 0;
    for (ctx, system) in whole().iter().take(200) {
        let pairs = ctx
            .hierarchy()
            .pairs()
            .map(|(_, orbit)| orbit.apoapsis().value())
            .fold(0.0, f64::max);
        for t in [
            ClockWindow::START.checked_add(second).unwrap(),
            UniverseTime::EPOCH,
            ClockWindow::END.checked_sub(second).unwrap(),
        ] {
            for body in system.bodies() {
                let Some((at, velocity)) = system.state_at(ctx, body.index(), t).unwrap() else {
                    continue;
                };
                let (Some(early), Some(late)) = (
                    system
                        .position_at(ctx, body.index(), t.checked_sub(second).unwrap())
                        .unwrap(),
                    system
                        .position_at(ctx, body.index(), t.checked_add(second).unwrap())
                        .unwrap(),
                ) else {
                    continue;
                };
                let record = system.body_at(ctx, body.index(), t).unwrap();
                let own = record
                    .orbit()
                    .ok()
                    .map_or(0.0, |orbit| orbit.elements().apoapsis().value());
                let reach = pairs.max(own).max(at.distance_from_origin().value());
                let floor = 4.0 * f64::EPSILON * reach;
                let v = velocity.metres_per_second();
                let speed = velocity.speed().value();
                for (axis, v_axis) in v.into_iter().enumerate() {
                    let derivative = (late.metres()[axis] - early.metres()[axis]) / 2.0;
                    assert!(
                        (derivative - v_axis).abs() <= 1e-6 * speed + floor,
                        "{:?} axis {axis} at {t}: {derivative} m/s by differences against {v_axis} m/s",
                        body.index()
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 1_000, "{checked} bodies checked");
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
    // On whole systems (P14.T30.b): the planets about each host, the records whose parent is the
    // host; moons keep T22.b's rule about their planet, and a belt's members, listed under their
    // belt, keep T21.c's clearance of the chaotic zones instead.
    for (ctx, system) in whole() {
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
                    // With the host's mass unchanged `now` is `hill`, and a pair placed at the
                    // limit sits on it to rounding: the same tolerance as the assertion below.
                    if gap < HILL_STABLE_GAP * now * (1.0 - 1e-12) {
                        // Any loss of the host's mass does it: a pair placed at the limit falls
                        // under it by (M₀ ÷ M)^⅓ − 1, 2 × 10⁻⁸ for a main-sequence star that has
                        // lost 7 × 10⁻⁸ of its mass (a pair of 0x41ffecb1ffc00004 after ruling 138).
                        assert!(
                            widening > 1.0,
                            "only mass loss unsettles a placed pair: {widening} at {t:?}, gap {gap} \
                             against {} of {now}, {:?}",
                            HILL_STABLE_GAP * now,
                            system.system()
                        );
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

/// How many real systems of primaries over 8 M☉ the supernova overlap test samples: 600. Since
/// plan 11's ruling 81 nearly every massive primary has close direct companions (Moe and Di
/// Stefano's Table 13; 18 of 120 sampled are single), which leave its planets little room: 120
/// systems held 33 surviving pairs about exploded hosts, where the test asks for more than 60;
/// since ruling 106.2's rocky groups of 2–6, 360 held 57, and since ruling 116.1's farther first
/// rocky planet 480 held 60.
const MASSIVE_SAMPLE: usize = 600;

/// Ruling 71.3: the overlap property on post-supernova survivors. A sudden death gives each
/// survivor its own eccentricity, and the scattering step (P14.T28.c) must leave no two orbits of
/// a host crossing at −H, the epoch or +H. About a single star that died suddenly, whose mass has
/// not changed since, the survivors also keep 2√3 mutual Hill radii at the remnant's mass. The
/// ordinary sample holds no surviving pair of a supernova host, so this one samples primaries of
/// 8 M☉ and more.
#[test]
fn no_orbits_cross_about_hosts_that_lost_mass_at_once() {
    let massive = SampleFilter::primary_masses(SolarMasses::new(8.0), SolarMasses::new(1e9));
    let contexts = sample_contexts(MASSIVE_SAMPLE, SEED, massive).unwrap();
    let (mut pairs, mut after_supernovae) = (0_u32, 0_u32);
    for ctx in &contexts {
        let system = generate_planets(SEED, ctx);
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
                let exploded = zone.members().any(|m| {
                    let star = &ctx.stars()[usize::from(m)];
                    star.death().is_some_and(|d| d.kind().is_sudden())
                        && star.state_at(t).is_some_and(|s| s.phase().is_remnant())
                });
                let alone = zone.members().count() == 1;
                let host = zone
                    .members()
                    .map(|m| ctx.stars()[usize::from(m)].state_at(t).unwrap().mass())
                    .fold(SolarMasses::ZERO, |sum, m| sum + m);
                for pair in orbits.windows(2) {
                    let ((m1, o1), (m2, o2)) = (pair[0], pair[1]);
                    let gap = o2.periapsis().value() - o1.apoapsis().value();
                    assert!(
                        gap > 0.0,
                        "{:?} at {t:?}: orbits cross by {} m",
                        system.system(),
                        -gap
                    );
                    if exploded && alone {
                        let (a1, a2) = (o1.semi_major_axis(), o2.semi_major_axis());
                        let hill = mutual_hill_radius(m1, m2, host, a1, a2).value();
                        assert!(
                            gap >= HILL_STABLE_GAP * hill * (1.0 - 1e-9),
                            "{:?} at {t:?}: {gap} m against {HILL_STABLE_GAP} Hill radii of {hill} m",
                            system.system()
                        );
                    }
                    pairs += 1;
                    if exploded {
                        after_supernovae += 1;
                    }
                }
            }
        }
    }
    assert!(
        after_supernovae > 60 && pairs > after_supernovae,
        "{after_supernovae} of {pairs} pairs about hosts after a supernova"
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
    // On whole systems (P14.T30.b): planets, moons and belt members.
    for (ctx, system) in whole() {
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

/// Across the window, in whole systems: a moon or a ring is never present while its parent is
/// not, and takes its parent's state then (a giant engulfed by its evolved host takes its moons
/// and rings with it); a regular or captured moon and a ring are present whenever their parent
/// is; and a belt member's moon is no exception (P14.T30.b).
#[test]
fn satellites_share_their_parent_s_state_across_the_window() {
    let (mut followed, mut engulfed) = (0u32, 0u32);
    for (ctx, system) in whole() {
        for t in window() {
            let snapshot = system.snapshot_at(ctx, t);
            let state_of = |index: BodyIndex| {
                snapshot
                    .bodies()
                    .iter()
                    .find(|r| r.index() == index)
                    .map(|r| r.identity().state())
                    .unwrap()
            };
            for body in system.bodies() {
                let OrbitHost::Body(parent) = body.host() else {
                    continue;
                };
                let (own, of_parent) = (state_of(body.index()), state_of(parent));
                match (of_parent, body.kind()) {
                    (BodyState::Present, BodyKind::Moon(MoonOrigin::GiantImpact)) => {}
                    (BodyState::Present, _) => assert_eq!(own, BodyState::Present),
                    (other, _) => {
                        assert_eq!(own, other, "{:?} of {parent:?}", body.index());
                        followed += 1;
                        engulfed += u32::from(matches!(
                            other,
                            BodyState::Destroyed {
                                cause: DestructionCause::Engulfed,
                                ..
                            }
                        ));
                    }
                }
            }
        }
    }
    assert!(followed > 0, "no satellite of an absent parent");
    eprintln!("{followed} satellites followed an absent parent, {engulfed} engulfed");
}

/// Every body's primordial circularisation is T8.e's damping time with Chen and Kipping's median
/// radius (ruling 62.5), and a hot Jupiter of up to 1.5 Jupiter masses within five days of an old
/// Sun is circular to 0.01 (P14.T8.e's finding).
#[test]
fn circularisation_is_t8e_s_and_hot_jupiters_circularise() {
    let (mut held, mut floored) = (0, 0);
    for (ctx, system) in generated() {
        for body in system.bodies() {
            let Some(placed) = body.placed() else {
                continue;
            };
            let zone = system.zone(body.host()).unwrap();
            let siblings: Vec<PlacedPlanet> = system
                .bodies()
                .iter()
                .filter(|other| other.host() == body.host())
                .filter_map(|other| other.placed().copied())
                .collect();
            let position = siblings
                .iter()
                .position(|other| other.index() == placed.index())
                .unwrap();
            let expected =
                host_circularisations(&siblings, zone.host_mass(), ctx.age_at_epoch())[position];
            assert_eq!(body.circularisation(), Some(expected));
            assert_eq!(body.fate().unwrap().circularisation(), expected);
            // Ruling 133.1: the damping stops at the floor, so no planet is damped below its floor
            // at its age (or its own primordial eccentricity, where that is lower).
            let age = ctx.age_at_epoch().value();
            let e0 = placed.orbit().eccentricity().value();
            let floor = expected.floor().at(age).min(e0);
            if floor > 0.0 && expected.drain() <= 0.0 {
                held += 1;
                let record = system
                    .body_at(ctx, body.index(), UniverseTime::EPOCH)
                    .unwrap();
                if let Some(orbit) = record.orbit().ok() {
                    let e = orbit.elements().eccentricity().value();
                    assert!(
                        e >= floor * (1.0 - 1e-9),
                        "{:?}: e {e} < {floor}",
                        body.index()
                    );
                    floored += usize::from(e < floor * 1.01);
                }
            }
        }
    }
    assert!(
        held > 100 && floored > 0,
        "{held} held, {floored} at their floor"
    );
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
            let days = body.orbit().unwrap().period().value() / 86_400.0;
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
/// inside it is flagged and its circumbinary zone takes the pair's plane, and one of a pair outside
/// it neither.
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

/// P14.T30.b: on whole systems, every body lies inside its zone and the strip radius at ±H, and
/// every population inside the strip radius; an unborn system's bodies are all not yet formed.
#[test]
fn every_body_of_a_whole_system_lies_inside_its_zone_and_the_strip_radius() {
    let mut checked = 0;
    for (ctx, system) in whole() {
        let strip = ctx.strip_radius();
        for t in window() {
            for record in system.snapshot_at(ctx, t).bodies() {
                match (
                    record.identity().kind(),
                    record.orbit().ok(),
                    record.population().ok(),
                ) {
                    (BodyKind::Planet | BodyKind::DwarfPlanet, Some(orbit), _) => {
                        let body = system.body(record.index()).unwrap();
                        let zone = system.zone(body.host()).unwrap();
                        let elements = orbit.elements();
                        assert!(elements.apoapsis() <= strip, "{:?}", record.index());
                        if let Some(outer) = zone.outer() {
                            // The zone is the one at birth; orbits widen with mass loss.
                            let widened = elements.semi_major_axis().value()
                                / body.orbit().unwrap().semi_major_axis().value();
                            assert!(
                                elements.apoapsis().value() <= outer.value() * widened.max(1.0),
                                "{:?}",
                                record.index()
                            );
                        }
                        checked += 1;
                    }
                    (BodyKind::Belt(_), _, Some(Population::Belt(belt))) => {
                        assert!(belt.outer_edge() <= strip * 1.000_000_1 * widening(ctx, t));
                        checked += 1;
                    }
                    (BodyKind::CometaryHalo, _, Some(Population::CometaryHalo(halo))) => {
                        assert!(halo.outer_edge() <= strip);
                        checked += 1;
                    }
                    (_, _, _) => {}
                }
            }
        }
    }
    assert!(checked > 1_000, "{checked}");
    for i in 0..20 {
        let ctx = synthetic_star(
            id(900 + i),
            SolarMasses::new(1.0),
            Dex::ZERO,
            Years::new(-500.0),
        )
        .unwrap();
        let system = generate(SEED, &ctx);
        let snapshot = system.snapshot_at(&ctx, UniverseTime::EPOCH);
        for record in snapshot.bodies() {
            assert_eq!(
                record.identity().state(),
                BodyState::NotYetFormed,
                "{:?}",
                record.index()
            );
            assert_eq!(record.position(), None);
        }
    }
}

/// How far the orbits of the primary's host have widened by `t` from its mass lost: its initial
/// mass over its mass then.
fn widening(ctx: &SystemContext, t: UniverseTime) -> f64 {
    let star = &ctx.stars()[0];
    star.state_at(t)
        .map_or(1.0, |state| {
            star.initial_mass().value() / state.mass().value()
        })
        .max(1.0)
}

/// Ruling 112.8: a host's placed class reports what was placed, so a host that is not `Barren`
/// holds at least one planet (`close_binary`'s star B was `CompactMulti` with none: its disc,
/// cut by its companion to 0.085–0.27 au, held 0.043 M⊕ of solids, too little for one chain
/// member).
#[test]
fn a_host_that_placed_nothing_is_barren() {
    let (mut empty, mut hosts) = (0, 0);
    for (_, system) in generated() {
        for host in system.hosts() {
            hosts += 1;
            let planets = system
                .bodies()
                .iter()
                .filter(|body| body.host() == host.host() && body.placed().is_some())
                .count();
            if planets == 0 {
                empty += 1;
                assert_eq!(host.class(), ArchitectureClass::Barren, "{:?}", host.host());
            } else {
                assert_ne!(host.class(), ArchitectureClass::Barren, "{:?}", host.host());
            }
        }
    }
    assert!(empty > 0 && hosts > empty, "{empty} empty of {hosts} hosts");
}

/// Ruling 133.4: a companion's pericentre widens by M₀/M when the loss is adiabatic, keeps q₀ when
/// it is impulsive, and is weighted between.
#[test]
fn a_companion_widens_by_its_regime() {
    let q0 = Metres::new(1e15);
    let kept = 0.5;
    let slow = present_pericentre(q0, kept, 1e-3);
    assert!((slow / q0 - 2.0).abs() < 1e-12);
    let fast = present_pericentre(q0, kept, 10.0);
    assert!((fast / q0 - 1.0).abs() < 1e-12);
    // Ψ = 0.1 × 30^½: halfway in the log, w = ½, so √2.
    let middle = present_pericentre(q0, kept, 0.1 * 30.0_f64.sqrt());
    assert!((middle / q0 - 2.0_f64.sqrt()).abs() < 1e-9);
    assert!((present_pericentre(q0, kept, 0.0) / q0 - 2.0).abs() < 1e-12);
}

// P14.T45.a: evolving orbits on drift cells.

/// Each record with a drift in the sample at `t`, with its system.
fn drifting_records(
    t: UniverseTime,
) -> Vec<(&'static SystemContext, &'static PlanetarySystem, BodyRecord)> {
    let mut found = Vec::new();
    for (ctx, system) in whole() {
        for record in system.snapshot_at(ctx, t).bodies() {
            if record
                .orbit()
                .ok()
                .is_some_and(|orbit| orbit.drift().is_some())
            {
                found.push((ctx, system, record.clone()));
            }
        }
    }
    found
}

/// A drift cell's tolerance about `orbit`, m.
fn drift_tolerance(orbit: &KeplerElements) -> f64 {
    crate::planetary::drift::DRIFT_TOLERANCE.value() + f64::EPSILON * orbit.apoapsis().value()
}

#[test]
fn records_at_any_two_times_in_one_drift_cell_are_the_same() {
    let records = drifting_records(UniverseTime::EPOCH);
    assert!(records.len() > 100, "{} drifting records", records.len());
    for (ctx, system, record) in records.iter().take(400) {
        let orbit = record.orbit().ok().expect("drifting");
        let start = orbit.drift().expect("drifting").reference();
        let until = orbit.valid_until().expect("a cell ends inside the window");
        let last = until.checked_sub(Span::new(0, 1).unwrap()).unwrap();
        for t in [start, last] {
            let again = system.body_at(ctx, record.index(), t).unwrap();
            assert_eq!(
                again.orbit().ok(),
                Some(orbit),
                "{:?} at {t}",
                record.index()
            );
        }
    }
}

/// The last instant a drifting record holds: a nanosecond before its `valid_until`, or the window's
/// end for a cell cut there.
fn last_instant_of(orbit: &BodyOrbit) -> UniverseTime {
    orbit.valid_until().map_or(ClockWindow::END, |until| {
        until.checked_sub(Span::new(0, 1).unwrap()).unwrap()
    })
}

/// Records are one per cell where cells are halved or cut too: before the window's start, at the
/// epoch and in the window's last cells, where the sample's 2¹⁶ s cells sit, at the reference, the
/// middle and the last instant of each cell, bit for bit and whatever was asked before (P14.T45.a).
#[test]
fn records_of_halved_and_cut_drift_cells_are_the_same_at_every_time_in_them() {
    use crate::planetary::drift::DRIFT_CELL_MAX_LOG2;
    let before_start = ClockWindow::START
        .checked_sub(Span::new(1 << 20, 0).unwrap())
        .unwrap();
    let near_end = ClockWindow::END
        .checked_sub(Span::new(1 << 15, 0).unwrap())
        .unwrap();
    let (mut checked, mut short) = (0, 0);
    for at in [before_start, UniverseTime::EPOCH, near_end] {
        for (ctx, system, record) in drifting_records(at).iter().take(300) {
            let orbit = record.orbit().ok().expect("drifting");
            let reference = orbit.drift().expect("drifting").reference();
            let last = last_instant_of(orbit);
            let middle = reference
                .checked_add(Span::new((last.seconds() - reference.seconds()) / 2, 0).unwrap())
                .unwrap();
            let times = [reference, middle, last, at];
            let once = |t: &UniverseTime| {
                system
                    .body_at(ctx, record.index(), *t)
                    .unwrap()
                    .orbit()
                    .ok()
                    .copied()
            };
            assert_order_independent(&times, once);
            for t in times {
                assert_eq!(
                    once(&t).as_ref(),
                    Some(orbit),
                    "{:?} at {t}",
                    record.index()
                );
            }
            if last.seconds() - reference.seconds() < (1 << DRIFT_CELL_MAX_LOG2) - 1 {
                short += 1;
            }
            checked += 1;
        }
    }
    assert!(checked > 300, "{checked} records");
    assert!(short > 20, "{short} halved or cut cells");
}

#[test]
fn adjacent_drift_cells_join_within_the_tolerance() {
    let mut joins = 0;
    for (ctx, system, record) in drifting_records(UniverseTime::EPOCH).iter().take(400) {
        let orbit = record.orbit().ok().expect("drifting");
        let until = orbit.valid_until().expect("a cell ends inside the window");
        let next = system.body_at(ctx, record.index(), until).unwrap();
        let (Some(after), true) = (
            next.orbit().ok(),
            next.identity().state() == record.identity().state(),
        ) else {
            continue;
        };
        if after.drift().is_none() {
            continue;
        }
        let last = until.checked_sub(Span::new(0, 1).unwrap()).unwrap();
        let before = orbit.trajectory().relative_state_at(last).0;
        let joined = after.trajectory().relative_state_at(last).0;
        let apart = before
            .metres()
            .iter()
            .zip(joined.metres())
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f64>()
            .sqrt();
        assert!(
            apart < drift_tolerance(orbit.elements()),
            "{:?}: {apart} m at {until}",
            record.index()
        );
        joins += 1;
    }
    assert!(joins > 100, "{joins} joins");
}

/// Every drifting record of the sample at the epoch, receding moons and circularising planets
/// among them, moves at its position's derivative, to 10⁻⁹ of its speed beyond a central
/// difference's own error (P14.T45.e).
#[test]
fn a_drifting_record_s_velocity_is_the_derivative_of_its_position() {
    use crate::planetary::drift::tests::velocity_derivative_excess;
    let (mut moons, mut eccentric) = (0, 0);
    for (_, _, record) in drifting_records(UniverseTime::EPOCH) {
        let orbit = record.orbit().ok().expect("drifting");
        let drift = orbit.drift().expect("drifting");
        let inside = drift
            .reference()
            .checked_add(Span::new(600, 0).unwrap())
            .unwrap();
        if orbit.valid_until().is_some_and(|until| until <= inside) {
            continue;
        }
        let excess = velocity_derivative_excess(&orbit.trajectory(), inside);
        assert!(excess < 1e-9, "{:?}: {excess:e}", record.index());
        if matches!(record.identity().parent(), Some(OrbitHost::Body(_))) {
            moons += 1;
        }
        if drift.eccentricity_rate_per_s() < 0.0 {
            eccentric += 1;
        }
    }
    assert!(
        moons > 0 && eccentric > 0,
        "{moons} moons, {eccentric} circularising"
    );
}

/// The share of evolving orbits whose cells fall to the smallest size, over the sample at three
/// times, counting the cells that neither a segment nor a state cut: under 10⁻³ (P14.T45.a).
#[test]
fn few_evolving_orbits_fall_to_the_smallest_drift_cell() {
    use crate::planetary::drift::{DRIFT_CELL_MAX_LOG2, DRIFT_CELL_MIN_LOG2};
    let (mut cells, mut floor) = (0_u32, 0_u32);
    let mut sizes = [0_u32; DRIFT_CELL_MAX_LOG2 as usize + 1];
    let later = UniverseTime::from_julian_years(500).expect("in the window");
    for t in [ClockWindow::START, UniverseTime::EPOCH, later] {
        for (_, _, record) in drifting_records(t) {
            let orbit = record.orbit().ok().expect("drifting");
            let start = orbit.drift().expect("drifting").reference();
            let Some(until) = orbit.valid_until() else {
                continue;
            };
            let length = until.seconds() - start.seconds();
            let aligned = start.subsec_nanos() == 0
                && until.subsec_nanos() == 0
                && length > 0
                && length.count_ones() == 1
                && start.seconds() % length == 0;
            if !aligned {
                continue;
            }
            let log2 = length.trailing_zeros();
            assert!((DRIFT_CELL_MIN_LOG2..=DRIFT_CELL_MAX_LOG2).contains(&log2));
            sizes[usize::try_from(log2).unwrap()] += 1;
            cells += 1;
            if log2 == DRIFT_CELL_MIN_LOG2 {
                floor += 1;
            }
        }
    }
    assert!(cells > 300, "{cells} cells");
    assert!(
        f64::from(floor) < 1e-3 * f64::from(cells),
        "{floor} of {cells} cells at the floor; sizes by log2 {sizes:?}"
    );
}

/// On the sample's drifting planets, the 16-point rule on each interval between the law's break
/// points agrees with the 32-point rule on the same intervals to the drift module's tolerance:
/// 10 µm along the orbit, plus 4 · 2⁻⁵² of the phase gained since the anchor (P14.T45.d).
#[test]
fn the_phase_integral_agrees_with_a_finer_rule_on_the_sample() {
    let mut checked = 0;
    let later = UniverseTime::from_julian_years(700).expect("in the window");
    let earlier = UniverseTime::from_julian_years(-990).expect("in the window");
    for (ctx, system, record) in drifting_records(UniverseTime::EPOCH) {
        let Some(body) = system.body(record.index()) else {
            continue;
        };
        if !matches!(body.part, Part::Planet(_) | Part::Member(_)) {
            continue;
        }
        let host = fate_host(ctx, system.zone_of(body));
        let fate = system.fate_of(body, &host);
        for t in [earlier, UniverseTime::EPOCH, later] {
            let Some([by16, by32, gained, along]) = fate.quadrature_at(t) else {
                continue;
            };
            let error = (by16 - by32).abs() * along;
            let tolerance = 1e-5 + 4.0 * f64::EPSILON * gained.abs() * along;
            assert!(
                error <= tolerance,
                "{:?} at {t}: {error:e} m against {tolerance:e} m",
                record.index()
            );
            checked += 1;
        }
    }
    assert!(checked > 100, "{checked} checked");
}

/// A moon's record holds until its planet's next change of state or segment, not its planet's
/// drift cell's end (P14.T45.a): a moon that does not recede has no drift and the same
/// `valid_until` as its planet's `changes_at`.
#[test]
fn a_fixed_moon_does_not_follow_its_planet_s_drift_cells() {
    let mut checked = 0;
    for (ctx, system) in whole() {
        let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
        for record in snapshot.bodies() {
            let (Some(orbit), BodyKind::Moon(_)) = (record.orbit().ok(), record.identity().kind())
            else {
                continue;
            };
            if orbit.drift().is_some() {
                continue;
            }
            let Some(OrbitHost::Body(parent)) = record.identity().parent() else {
                continue;
            };
            let Some(parent_orbit) = snapshot.body(parent).and_then(|p| p.orbit().ok()) else {
                continue;
            };
            let Some(cell_end) = parent_orbit.drift().and(parent_orbit.valid_until()) else {
                continue;
            };
            assert_ne!(orbit.valid_until(), Some(cell_end), "{:?}", record.index());
            checked += 1;
        }
    }
    assert!(checked > 10, "{checked} fixed moons of drifting planets");
}

/// Before the window's start a receding moon's record is cut at its planet's next change there,
/// which `FateAt::changes_at` now states (RM1 validation, 2026-10-02): its cell ends no later, and
/// its `valid_until` is that change. The sample holds no moon whose planet changes between −(H + L)
/// and `START`, so the change is supplied here; `fate::tests` checks that the planet states it.
#[test]
fn a_receding_moon_s_record_before_the_window_is_cut_at_its_planet_s_change() {
    let t = ClockWindow::START
        .checked_sub(Span::new(1 << 22, 0).unwrap())
        .unwrap();
    let mut checked = 0;
    for (ctx, system) in whole() {
        for body in system.bodies() {
            let Part::Moon(moon) = &body.part else {
                continue;
            };
            let (alone, until) = moon_trajectory(ctx, moon, t, None);
            let (Some(drift), Some(until)) = (alone.drift(), until) else {
                continue;
            };
            assert!(drift.reference() <= t && t < until);
            let change = t.checked_add(Span::new(1_000, 0).unwrap()).unwrap();
            if change >= until {
                continue;
            }
            let (cut, cut_until) = moon_trajectory(ctx, moon, t, Some(change));
            assert_eq!(cut_until, Some(change), "{:?}", body.index);
            assert!(cut.drift().is_some(), "{:?}", body.index);
            checked += 1;
        }
    }
    assert!(checked > 0, "{checked} receding moons");
}

/// The drift cell that straddles the clock window's start gives one record on each side of it,
/// each holding no further than `START` from before it (P14.T45.a).
#[test]
fn the_window_s_start_cuts_a_drift_cell_on_both_sides() {
    let before = ClockWindow::START
        .checked_sub(Span::new(0, 1).unwrap())
        .unwrap();
    let mut checked = 0;
    for (ctx, system, record) in drifting_records(ClockWindow::START).iter().take(200) {
        let early = system.body_at(ctx, record.index(), before).unwrap();
        let Some(orbit) = early.orbit().ok() else {
            continue;
        };
        if orbit.drift().is_some() {
            assert!(
                orbit
                    .valid_until()
                    .is_some_and(|until| until <= ClockWindow::START),
                "{:?}",
                record.index()
            );
        }
        let late = record.orbit().ok().unwrap();
        assert!(late.drift().unwrap().reference() >= ClockWindow::START);
        checked += 1;
    }
    assert!(checked > 50, "{checked}");
}

/// P14.T46.b (b): every body's held law is the one derived from its record at `parent_time`, bit
/// for bit, whatever order the bodies are asked in; a whole system's records carry it as their
/// rotation section, the same at the epoch and at +H, and a ring's, a belt's and the halo's is not
/// applicable.
#[test]
fn the_rotation_law_is_held_and_on_every_record() {
    let mut laws = 0;
    for (ctx, system) in whole() {
        let indices: Vec<BodyIndex> = system.bodies().iter().map(Body::index).collect();
        let held = |index: BodyIndex| system.rotation_of(ctx, index).unwrap();
        for &index in &indices {
            let body = system.body(index).unwrap();
            let record = system.body_at(ctx, index, parent_time(ctx)).unwrap();
            assert_eq!(
                held(index),
                system.derive_rotation(ctx, body, &record),
                "{index:?}"
            );
        }
        assert_order_independent(&indices, |&index| held(index));
        for t in [UniverseTime::EPOCH, ClockWindow::END] {
            for record in system.snapshot_at(ctx, t).bodies() {
                let rotation = record.rotation();
                match record.identity().kind() {
                    BodyKind::Ring | BodyKind::Belt(_) | BodyKind::CometaryHalo => {
                        assert_eq!(rotation, &Section::NotApplicable);
                    }
                    _ if record.identity().state() != BodyState::Present => {
                        assert_eq!(rotation, &Section::NotApplicable);
                    }
                    _ => match held(record.index()) {
                        Some(law) => {
                            assert_eq!(rotation, &Section::Ok(*law.frame()));
                            laws += 1;
                        }
                        None => assert_eq!(rotation, &Section::NotModelled),
                    },
                }
            }
        }
    }
    assert!(laws > 1_000, "{laws}");
}

/// P14.T46.b (b): a system [`generate_planets`] made holds no law, and its present planets'
/// rotation is not modelled, so no body has two rotations by entry point.
#[test]
fn a_planets_only_system_holds_no_rotation() {
    let mut planets = 0;
    for (ctx, system) in generated() {
        for record in system.snapshot_at(ctx, UniverseTime::EPOCH).bodies() {
            assert_eq!(system.rotation_of(ctx, record.index()).unwrap(), None);
            if record.identity().state() == BodyState::Present {
                assert_eq!(record.rotation(), &Section::NotModelled);
                planets += 1;
            }
        }
    }
    assert!(planets > 100, "{planets}");
}

/// P14.T46.a (a): a regular moon's locking time and its held law's are one law, at one moment of
/// inertia and its own tides (P14.T56.d): they differ only by their primordial periods, the moon
/// derivation's fixed 15 h and the law's drawn one (τ ∝ ω), to 10⁻¹², for every moon its first
/// round held (P14.T54.a's Q2, whose own test holds their tides to the bit:
/// `a_moon_s_law_is_held_after_at_most_two_rounds`). A moon its second round held reads its record's
/// tides under that round's law, not under the first's, and is listed there.
#[test]
fn a_regular_moon_has_one_locking_time() {
    let (mut moons, mut second) = (0, 0);
    for (ctx, system) in whole() {
        let snapshot = system.snapshot_at(ctx, parent_time(ctx));
        let epoch = Epoch::new(system, ctx, parent_time(ctx));
        for record in snapshot.bodies() {
            if record.identity().kind() != BodyKind::Moon(MoonOrigin::Regular) {
                continue;
            }
            let (Section::Ok(bulk), Section::Ok(mass), Some(rotation)) = (
                record.bulk(),
                record.mass(),
                system.rotation_of(ctx, record.index()).unwrap(),
            ) else {
                continue;
            };
            let body = system.body(record.index()).unwrap();
            let (Part::Moon(moon), OrbitHost::Body(parent)) = (&body.part, body.host) else {
                unreachable!("a regular moon is a moon of a body")
            };
            let elements = moon
                .satellite
                .orbit_at(&moon.parent, ctx.age_at(parent_time(ctx)));
            let primary = system.body(parent).unwrap();
            let host = fate_host(ctx, system.zone_of(primary));
            let fate = system.fate_of(primary, &host);
            let cosines = HourCosines::new();
            let now = system.parent_now(&epoch, &cosines, parent, &fate);
            let held = system
                .moon_law(&epoch, &cosines, body, moon, (&now, &fate))
                .unwrap();
            assert_eq!(held.law, rotation, "{:?}", record.index());
            if matches!(held.round, MoonRound::Second { .. }) {
                second += 1;
                continue;
            }
            let derived = system.record_moon(&epoch, body, (&now, &fate)).unwrap();
            let mass = Kilograms::from(*mass);
            let primary = Kilograms::new(
                elements.gravitational_parameter().value() / GRAVITATIONAL_CONSTANT - mass.value(),
            );
            let tau = crate::planetary::moons::regular::locking_time(
                primary,
                mass,
                Metres::from(bulk.radius()),
                elements.semi_major_axis(),
                bulk.class(),
                &bulk.fractions(),
                derived.spin_tides(),
            );
            let fixed = crate::planetary::moons::regular::MOON_PRIMORDIAL_PERIOD_HOURS * 3_600.0;
            let expected =
                rotation.locking_time().value() * (rotation.primordial_period().value() / fixed);
            assert!(
                (tau.value() / expected - 1.0).abs() < 1e-12,
                "{:?}: {} against {expected}",
                record.index(),
                tau.value()
            );
            moons += 1;
        }
    }
    eprintln!("{moons} regular moons held after round 1, {second} after round 2");
    assert!(moons > 100, "{moons}");
}

/// P14.T14.d: every present planet's locking time and the heaviest moon it keeps are those of the
/// tides its bulk section gives, with its one moment of inertia, bit for bit: `rotation::tides`.
#[test]
fn a_planet_s_locking_time_and_moon_limit_take_its_tides() {
    use crate::planetary::derive::OrbitSense;
    use crate::planetary::derive::limits::{TidalPlanet, moon_mass_limit};
    use crate::planetary::derive::rotation::{
        SpinningBody, moment_of_inertia_factor, tidal_locking_time, tides,
    };
    use crate::units::{EarthRadii, Seconds};
    let t = UniverseTime::EPOCH;
    let mut planets = 0;
    let mut enveloped = 0;
    for (ctx, system) in whole() {
        // The rotation's time is the epoch for a system born by then (`parent_time`).
        if ctx.age_at_epoch().value() <= 0.0 {
            continue;
        }
        let epoch = Epoch::new(system, ctx, t);
        for body in system.bodies() {
            if !matches!(body.part, Part::Planet(_)) {
                continue;
            }
            let host = fate_host(ctx, system.zone_of(body));
            let fate = system.fate_of(body, &host);
            let label = label::label(system, body.index).unwrap();
            let (record, now) =
                system.primary_record(&epoch, &HourCosines::new(), body, label, &fate);
            let (Section::Ok(bulk), Section::Ok(mass), Section::Ok(orbit), Some(derived)) =
                (record.bulk(), record.mass(), record.orbit(), now.derived)
            else {
                continue;
            };
            let (class, fractions) = (bulk.class(), bulk.fractions());
            let (kg, r) = (Kilograms::from(*mass), Metres::from(bulk.radius()));
            // Both readers take the tides of these kilograms and metres.
            let (k2, q) = tides(
                class,
                EarthMasses::from(kg),
                EarthRadii::from(r),
                &fractions,
            );

            // P14.T15's limit, at the prograde limit and the age it was derived at.
            let tidal = TidalPlanet::new(kg, r, k2, q).unwrap();
            let age =
                Years::new(ctx.age_at_epoch().value() + t.since_epoch().as_julian_years_f64());
            let limit = moon_mass_limit(
                derived.satellite_limit(OrbitSense::Prograde),
                &tidal,
                Seconds::from(age),
            );
            assert_same_bits(
                derived.maximum_moon_mass().value(),
                EarthMasses::from(limit).value(),
            );

            // P14.T14.b's locking time, on the orbit about the primary the rotation reads.
            let rotation = system
                .rotation_of(ctx, body.index)
                .unwrap()
                .expect("a present planet of a whole system holds its rotation");
            let elements = fate.at(t).orbit().copied().unwrap_or(*orbit.elements());
            let primary = Kilograms::new(
                elements.gravitational_parameter().value() / GRAVITATIONAL_CONSTANT - kg.value(),
            );
            let spinning =
                SpinningBody::new(kg, r, moment_of_inertia_factor(class, &fractions), k2, q)
                    .unwrap();
            let tau = tidal_locking_time(
                &spinning,
                rotation.primordial_period(),
                elements.semi_major_axis(),
                primary,
            );
            assert_same_bits(rotation.locking_time().value(), tau.value());
            planets += 1;
            if matches!(class, PlanetClass::SubNeptune | PlanetClass::IceGiant) {
                enveloped += 1;
            }
        }
    }
    assert!(
        planets > 300 && enveloped > 30,
        "{planets} planets, {enveloped} enveloped"
    );
}

/// P14.T46.d (d): the figure section's states by kind, and its level: `degrade(MassAndOrbit)`
/// withholds the rotation and the figure, and `degrade(Bulk)` keeps them. A figure keeps the
/// record's volume and its rotation's pole, and never exceeds the cap.
#[test]
fn the_figure_section_by_kind_and_level() {
    let mut figures = 0;
    for (ctx, system) in whole() {
        for record in system.snapshot_at(ctx, UniverseTime::EPOCH).bodies() {
            let figure = record.figure();
            match (record.rotation(), record.bulk()) {
                (Section::NotApplicable, _) => assert_eq!(figure, &Section::NotApplicable),
                (Section::Ok(frame), Section::Ok(bulk)) => {
                    let Section::Ok(figure) = figure else {
                        panic!("{:?}: a figure with a rotation and a bulk", record.index())
                    };
                    let radius = Metres::from(bulk.radius()).value();
                    let spheroid = figure.spheroid();
                    assert!((spheroid.volumetric_radius_m() / radius - 1.0).abs() < 1e-12);
                    // A capped figure's (a − c) ÷ a, recomputed from its radii, may round
                    // just above the cap.
                    assert!((0.0..=FLATTENING_CAP + 1e-12).contains(&spheroid.flattening()));
                    for (a, b) in figure.pole().into_iter().zip(frame.pole()) {
                        assert_same_bits(a, b);
                    }
                    figures += 1;
                }
                _ => assert_eq!(figure, &Section::NotModelled, "{:?}", record.index()),
            }
            for section in [RecordSection::Rotation, RecordSection::Figure] {
                assert_eq!(
                    record
                        .degrade(DetailLevel::MassAndOrbit)
                        .section_state(section),
                    SectionState::NotResolved
                );
                assert_eq!(
                    record.degrade(DetailLevel::Bulk).section_state(section),
                    record.section_state(section)
                );
            }
        }
    }
    assert!(figures > 1_000, "{figures}");
}

/// P14.T47.c (c) and T47.b (b): the photometry section's states by kind and level, `Ok` exactly
/// where the bulk is; every sampled body's p q is at most 1 per band, and a generated snowball
/// states a ratio within 5% of 1. A generated airless-ice body states 0.43 × 0.80 ÷ `A_ref` where
/// its p is not capped, its reference the fresh `H2O` frost's (P14.T51.b), where Ganymede's
/// measured Bond albedo, whose p the row draws, is 0.35 (`photometry::tests`, recorded in the bless
/// note): the closed form stands until the weathered icy regolith, its owner P14.T49.m
/// (decision-p14-t54a-ganymede §1b), darkens the reference.
#[test]
fn the_photometry_section_by_kind_and_level() {
    use crate::planetary::derive::photometry::PhaseTemplate;
    let (mut drawn, mut borrowed) = (0, 0);
    for (ctx, system) in whole() {
        for record in system.snapshot_at(ctx, UniverseTime::EPOCH).bodies() {
            let photometry = record.photometry();
            match record.bulk() {
                Section::Ok(_) => {
                    let Section::Ok(photometry) = photometry else {
                        panic!("{:?}: photometry with a bulk", record.index())
                    };
                    let (p, q) = (photometry.geometric_albedo(), photometry.phase_integral());
                    for (p, q) in [(p.b, q.b), (p.v, q.v), (p.r, q.r)] {
                        assert!(p > 0.0 && p * q <= 1.0 + 1e-12, "{:?}", record.index());
                    }
                    let ratio = photometry.bond_ratio();
                    match photometry.template() {
                        PhaseTemplate::Snowball => {
                            assert!((ratio - 1.0).abs() < 0.05, "{:?}: {ratio}", record.index());
                            borrowed += 1;
                        }
                        PhaseTemplate::AirlessIce => {
                            let reference =
                                PhaseTemplate::AirlessIce.row().reference_bond().value();
                            assert!(
                                ratio <= 0.43 * 0.80 / reference + 1e-9,
                                "{:?}: {ratio}",
                                record.index()
                            );
                            borrowed += 1;
                        }
                        PhaseTemplate::Moon
                        | PhaseTemplate::Mercury
                        | PhaseTemplate::Mars
                        | PhaseTemplate::Venus
                        | PhaseTemplate::Earth
                        | PhaseTemplate::Jupiter
                        | PhaseTemplate::Saturn
                        | PhaseTemplate::Uranus
                        | PhaseTemplate::Neptune
                        | PhaseTemplate::Magma => {}
                    }
                    drawn += 1;
                }
                Section::NotApplicable => assert_eq!(photometry, &Section::NotApplicable),
                _ => assert_eq!(photometry, &Section::NotModelled, "{:?}", record.index()),
            }
            assert_eq!(
                record
                    .degrade(DetailLevel::MassAndOrbit)
                    .section_state(RecordSection::Photometry),
                SectionState::NotResolved
            );
            assert_eq!(
                record
                    .degrade(DetailLevel::Bulk)
                    .section_state(RecordSection::Photometry),
                record.section_state(RecordSection::Photometry)
            );
        }
    }
    assert!(drawn > 1_000 && borrowed > 10, "{drawn} {borrowed}");
}

/// P14.T46.c (c): a body's flattening is continuous in time across the clock window, except where
/// its law locks between two times, a recorded state change.
#[test]
fn the_flattening_is_continuous_in_time() {
    let step = CLOCK_WINDOW_H.as_seconds_f64() / 8.0;
    let times: Vec<UniverseTime> = (-8_i32..=8)
        .map(|k| {
            UniverseTime::EPOCH
                .checked_add(Span::from_seconds_f64(f64::from(k) * step).unwrap())
                .unwrap()
        })
        .collect();
    let mut checked = 0;
    for (ctx, system) in whole().iter().take(60) {
        for body in system.bodies() {
            let Some(rotation) = system.rotation_of(ctx, body.index()).unwrap() else {
                continue;
            };
            let flattening = |t: UniverseTime| {
                let record = system.body_at(ctx, body.index(), t).unwrap();
                record.figure().ok().map(|f| f.spheroid().flattening())
            };
            for pair in times.windows(2) {
                let (Some(before), Some(after)) = (flattening(pair[0]), flattening(pair[1])) else {
                    continue;
                };
                let locks = rotation
                    .frame()
                    .rate()
                    .locks_at()
                    .is_some_and(|at| pair[0] < at && at <= pair[1]);
                if !locks {
                    assert!(
                        (after - before).abs() <= 0.01 * before.max(1e-6),
                        "{:?}: {before} to {after}",
                        body.index()
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 1_000, "{checked}");
}

/// P14.T24.a's surface conditions and P14.T24.b's global figures, published from the 21 → 22
/// batch's bump: `hooks_at` repeats the record's surface section (P14.T48.e) in the same state,
/// every present body with a solid surface has both, a moon's read under its planet's orbit, and
/// every gas-envelope body neither.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the sample's hooks, each body's surface checked and tallied"
)]
fn the_hooks_publish_the_record_s_surface_conditions_and_figures() {
    let t = UniverseTime::EPOCH;
    let (mut planets, mut moons, mut envelopes) = (0_u32, 0_u32, 0_u32);
    let mut regimes = std::collections::BTreeMap::new();
    let mut fields = std::collections::BTreeMap::new();
    let mut craters = CraterTally::default();
    let mut histories = HistoryTally::default();
    let mut cover = CoverTally::default();
    for (ctx, system) in whole().iter().take(60) {
        for index in system.indices() {
            let Some(open) = system.hooks_at(ctx, index, t).unwrap() else {
                continue;
            };
            assert_eq!(open.figures().state(), open.surface().state(), "{index:?}");
            let record = system.body_at(ctx, index, t).unwrap();
            assert_eq!(
                open.surface().state(),
                record.surface().state(),
                "{index:?}"
            );
            if let Section::Ok(surface) = record.surface() {
                assert_eq!(open.surface(), &Section::Ok(surface.conditions().clone()));
                assert_eq!(open.figures(), &Section::Ok(surface.figures().clone()));
            }
            let present = record.identity().state() == BodyState::Present;
            match open.surface() {
                Section::Ok(conditions) => {
                    assert!(present, "{index:?}");
                    let Section::Ok(bulk) = record.bulk() else {
                        panic!("{index:?} has conditions but no bulk section");
                    };
                    assert!(
                        matches!(bulk.class(), PlanetClass::Rocky | PlanetClass::Icy),
                        "{index:?} is a {:?}",
                        bulk.class()
                    );
                    let sum = conditions.gases().iter().fold(0.0, |sum, &(_, x)| sum + x);
                    assert!(
                        conditions.gases().is_empty() || (sum - 1.0).abs() < 1e-9,
                        "{index:?}: {sum}"
                    );
                    assert!(conditions.day_night_contrast_k() >= 0.0);
                    let Section::Ok(figures) = open.figures() else {
                        panic!("{index:?} has conditions but no figures");
                    };
                    // The heat flow its conditions read: its published pass's context's (rule (f)).
                    let heat_flow = match record.surface() {
                        Section::Ok(surface) => surface
                            .basis()
                            .map_or(figures.heat_flow(), |basis| basis.heat_flow),
                        _ => figures.heat_flow(),
                    };
                    check_published_figures(index, conditions, (figures, heat_flow));
                    let cover_of = (bulk.surface_gravity(), water_sea(&record));
                    check_published_cover(index, conditions, figures, cover_of);
                    assert_same_bits(
                        figures.craters().system_age().value(),
                        ctx.age_at(t).value() / 1e9,
                    );
                    *regimes.entry(figures.tectonic_regime()).or_insert(0_u32) += 1;
                    *fields.entry(figures.magnetic_field()).or_insert(0_u32) += 1;
                    craters.count(figures);
                    histories.count(conditions, figures);
                    cover.count(conditions, figures);
                    match record.identity().kind() {
                        BodyKind::Moon(_) => moons += 1,
                        BodyKind::Planet | BodyKind::DwarfPlanet => planets += 1,
                        kind @ (BodyKind::Ring
                        | BodyKind::Belt(_)
                        | BodyKind::CometaryHalo
                        | BodyKind::ProtoplanetaryDisc
                        | BodyKind::DebrisDisc
                        | BodyKind::Unresolved) => {
                            panic!("{index:?}, a {kind:?}, has surface conditions")
                        }
                    }
                }
                Section::NotApplicable => {
                    if present {
                        let Section::Ok(bulk) = record.bulk() else {
                            panic!("{index:?} is present with no bulk section");
                        };
                        assert!(
                            matches!(
                                bulk.class(),
                                PlanetClass::SubNeptune
                                    | PlanetClass::IceGiant
                                    | PlanetClass::GasGiant
                            ),
                            "{index:?} is a {:?}",
                            bulk.class()
                        );
                        envelopes += 1;
                    }
                }
                Section::NotModelled => {
                    assert_eq!(open.bulk(), &Section::NotModelled, "{index:?}");
                }
                Section::NotResolved => panic!("{index:?}: hooks are never degraded"),
            }
        }
    }
    eprintln!("{planets} planets and {moons} moons with conditions, {envelopes} envelopes");
    eprintln!("regimes {regimes:?}; fields {fields:?}");
    craters.report();
    histories.report();
    cover.report();
    assert!(planets > 20 && moons > 5 && envelopes > 20);
}

/// The area of `record`'s water sea, its surface's liquid `H2O` condensate's, where it has one.
fn water_sea(record: &BodyRecord) -> Option<f64> {
    use crate::planetary::chem::partition::{CondensateEntry, CondensatePhase};
    let Section::Ok(surface) = record.surface() else {
        return None;
    };
    surface
        .condensates()
        .entry(Gas::Water.substance(), CondensatePhase::Liquid)
        .map(CondensateEntry::area_fraction)
}

/// The published relief and cover of one body follow their rules (P14.T48.a, P14.T24.b;
/// science-p14-t24b-laws §4.1–4.6 and §4.x): continents only on a mobile lid; V in [0, `V_s`] and
/// the constructional share under the strength limit, `h♂` (g♂ ÷ g) ÷ 4, at the body's gravity
/// `gravity`; every fraction in [0, 1] and each list summing to its fraction; water's ocean its
/// `sea`, its condensate's own layer's fill (decision-p14-t54a-water-consistency §1), or where it
/// has none only under a `Snowball`'s ice; no ice where the coldest zone is above the melting
/// point; θ the potential ocean's; and the cloud in [0, 1].
fn check_published_cover(
    index: BodyIndex,
    conditions: &SurfaceConditions,
    figures: &GlobalFigures,
    (gravity, sea): (crate::units::MetresPerSecondSquared, Option<f64>),
) {
    use hooks::figures::relief::{
        CONSTRUCTIONAL_REFERENCE_GRAVITY, TALLEST_EDIFICE_RELIEF, VOLCANISM_SATURATION,
    };
    let v = figures.volcanism();
    assert!(
        (0.0..=VOLCANISM_SATURATION).contains(&v),
        "{index:?}: V {v}"
    );
    let limit = TALLEST_EDIFICE_RELIEF.value() * CONSTRUCTIONAL_REFERENCE_GRAVITY.value()
        / gravity.value()
        / 4.0;
    let built = figures.relief().constructional().value();
    assert!(
        built <= limit * (1.0 + 1e-12),
        "{index:?}: {built} m over {limit}"
    );
    if figures.ice_fraction() > 0.0 {
        // The coldest zone: the zonal profile's least point, or a locked world's night, which the
        // climate's range bounds.
        let coldest = conditions
            .zonal_points()
            .iter()
            .fold(conditions.climate_range().0.value(), |low, &(_, t)| {
                low.min(t)
            });
        assert!(
            coldest < hooks::figures::cover::melting_point().value(),
            "{index:?}: ice where no zone is below melting ({coldest} K)"
        );
    }
    let f_c = figures.continental_fraction();
    assert!((0.0..=0.7).contains(&f_c), "{index:?}: {f_c}");
    if f_c > 0.0 {
        assert_eq!(
            figures.tectonic_regime(),
            TectonicRegime::MobileLid,
            "{index:?}"
        );
    }
    assert!(
        figures.volcanism() >= 0.0 && figures.volcanism().is_finite(),
        "{index:?}"
    );
    assert!(figures.sigma_h().value() > 0.0, "{index:?}");
    // The record's liquids and ices are P14.T51.a's published condensates: each substance listed
    // once, the lists within the whole surface to rounding, and each fraction its list's sum held
    // at 1, which a list divided by its total sums back to only to rounding.
    for (fraction, list) in [
        (figures.ocean_fraction(), figures.surface_liquids()),
        (figures.ice_fraction(), figures.surface_ices()),
    ] {
        for (k, &(id, _)) in list.iter().enumerate() {
            assert!(list[..k].iter().all(|&(other, _)| other != id), "{index:?}");
        }
        assert!((0.0..=1.0).contains(&fraction), "{index:?}: {fraction}");
        let sum = list.iter().fold(0.0, |sum, &(_, f)| sum + f);
        assert!(sum <= 1.0 + 1e-12, "{index:?}: {list:?}");
        assert_same_bits(if sum > 1.0 { 1.0 } else { sum }, fraction);
    }
    assert!(
        (0.0..=1.0).contains(&figures.potential_ocean()),
        "{index:?}"
    );
    assert_eq!(
        figures.ocean_weight(),
        hooks::surface::OceanWeight::of_potential_ocean(figures.potential_ocean()),
        "{index:?}"
    );
    // Water's published liquid is its condensate's `sea`, the share of the basins its own layer
    // fills (decision-p14-t54a-water-consistency §1), or where it has none a `Snowball`'s ocean
    // under its ice at its potential ocean (W3 (b)); each is scaled down with the other liquids
    // where together they would cover more than the surface (`within_the_surface`). Any other
    // liquid is the partition's own sea (P14.T51.a), which the potential ocean, water's, does not
    // read.
    let water = Gas::Water.substance();
    let ocean = figures
        .surface_liquids()
        .iter()
        .find(|&&(id, _)| id == water)
        .map_or(0.0, |&(_, f)| f);
    let liquids = figures.ocean_fraction();
    let whole = sea.unwrap_or(figures.potential_ocean());
    if ocean > 0.0 && liquids < 1.0 - 1e-9 {
        assert_same_bits(ocean, whole);
    }
    assert!(
        ocean <= whole * (1.0 + 1e-12),
        "{index:?}: {ocean} against {whole}"
    );
    if !conditions.liquid_water() && ocean > 0.0 {
        assert!(
            hooks::figures::cover::under_ice_ocean(
                SurfaceState::Snowball,
                figures.surface_water(),
                conditions.mean_surface_temperature(),
                figures.heat_flow(),
            ),
            "{index:?}: an ocean with no liquid at the surface lies under a Snowball's ice"
        );
    }
    assert!((0.0..=1.0).contains(&figures.cloud_fraction()), "{index:?}");
}

/// The published relief and cover's spread over a sample, for the record (P14.T48.a, P14.T24.b):
/// the continents, the volcanism levels, `σ_h`'s range, the oceans published and held back, the
/// ice and the ocean worlds.
#[derive(Debug, Default)]
struct CoverTally {
    bodies: u32,
    continents: u32,
    greatest_continents: f64,
    volcanic: u32,
    greatest_volcanism: f64,
    sigma_h: Option<(f64, f64)>,
    oceans: u32,
    ocean_worlds: u32,
    potential_only: u32,
    iced: u32,
    wholly_iced: u32,
    under_ice: u32,
    continents_without_epoch: u32,
    beyond_calibration: u32,
    over_20_km: u32,
}

impl CoverTally {
    /// Counts `figures`' relief and cover, published with `conditions`.
    fn count(&mut self, conditions: &SurfaceConditions, figures: &GlobalFigures) {
        self.bodies += 1;
        let f_c = figures.continental_fraction();
        if f_c > 0.0 {
            self.continents += 1;
            self.greatest_continents = self.greatest_continents.max(f_c);
            self.continents_without_epoch += u32::from(figures.wet_epoch().is_none());
        }
        self.beyond_calibration += u32::from(figures.volcanism() > 2.76);
        self.over_20_km += u32::from(figures.sigma_h().value() > 20e3);
        if figures.volcanism() > 0.01 {
            self.volcanic += 1;
        }
        self.greatest_volcanism = self.greatest_volcanism.max(figures.volcanism());
        let sigma = figures.sigma_h().value();
        self.sigma_h = Some(self.sigma_h.map_or((sigma, sigma), |(low, high)| {
            (low.min(sigma), high.max(sigma))
        }));
        if figures.ocean_fraction() > 0.0 {
            self.oceans += 1;
            self.under_ice += u32::from(!conditions.liquid_water());
            self.ocean_worlds += u32::from(figures.ocean_fraction() >= 1.0);
        } else if figures.potential_ocean() > 0.0 {
            self.potential_only += 1;
        }
        if figures.ice_fraction() > 0.0 {
            self.iced += 1;
            self.wholly_iced += u32::from(figures.ice_fraction() >= 1.0);
        }
    }

    /// Prints the tally, and checks that the strength limit binds somewhere on the sample
    /// (science-p14-t24b-laws §4.x.1).
    fn report(&self) {
        assert_same_bits(
            self.greatest_volcanism,
            hooks::figures::relief::VOLCANISM_SATURATION,
        );
        eprintln!(
            "cover over {} bodies: {} with continents (greatest {:.3}, {} with no epoch); {} with \
             V over 0.01 (greatest {:.3}, {} over 2.76); σ_h {:?} m ({} over 20 km); {} oceans \
             published ({} under ice, {} ocean worlds), {} potential oceans held back; {} iced \
             ({} wholly)",
            self.bodies,
            self.continents,
            self.greatest_continents,
            self.continents_without_epoch,
            self.volcanic,
            self.greatest_volcanism,
            self.beyond_calibration,
            self.sigma_h,
            self.over_20_km,
            self.oceans,
            self.under_ice,
            self.ocean_worlds,
            self.potential_only,
            self.iced,
            self.wholly_iced
        );
    }
}

/// The published volatile histories' spread over a sample, for the record (P14.T48.b): the bodies
/// wet now, those with a past epoch and their longest span, those wet now by their flag with no
/// epoch, and those from a colder state.
#[derive(Debug, Default)]
struct HistoryTally {
    wet_now: u32,
    past: u32,
    longest_past_gyr: f64,
    flagged_without: u32,
    from_colder: u32,
}

impl HistoryTally {
    /// Counts `figures`' history, published with `conditions`.
    fn count(&mut self, conditions: &SurfaceConditions, figures: &GlobalFigures) {
        match figures.wet_epoch() {
            Some(epoch) if epoch.end().value() <= 0.0 => self.wet_now += 1,
            Some(epoch) => {
                self.past += 1;
                let span = epoch.start().value() - epoch.end().value();
                self.longest_past_gyr = self.longest_past_gyr.max(span);
            }
            None => self.flagged_without += u32::from(conditions.liquid_water()),
        }
        self.from_colder += u32::from(figures.from_colder());
    }

    /// Prints the tally.
    fn report(&self) {
        eprintln!(
            "wet epochs: {} to the present, {} past (longest {:.3} Gyr); {} liquid by the flag \
             with none; {} from a colder state",
            self.wet_now, self.past, self.longest_past_gyr, self.flagged_without, self.from_colder
        );
    }
}

/// The published crater contracts' spread over a sample, for the record (P14.T24.b, T48.c): the
/// crusts, the least and greatest rate, the surfaces under 1 Gyr old, and each source rule's
/// bodies and greatest abundance.
#[derive(Debug, Default)]
struct CraterTally {
    crusts: std::collections::BTreeMap<Crust, u32>,
    abundances: std::collections::BTreeMap<&'static str, (u32, f64)>,
    rates: Option<(f64, f64)>,
    young: u32,
}

impl CraterTally {
    /// Counts `figures`' contract.
    fn count(&mut self, figures: &GlobalFigures) {
        let impactors = figures.impactors();
        *self.crusts.entry(impactors.crust()).or_insert(0) += 1;
        let rule = match impactors.source() {
            hooks::figures::ImpactorSource::InBelt { .. } => "in belt",
            hooks::figures::ImpactorSource::Comets => "comets",
            hooks::figures::ImpactorSource::NearestBelt { .. } => "nearest belt",
            hooks::figures::ImpactorSource::InnerComets { .. } => "inner comets",
        };
        let (bodies, most) = self.abundances.entry(rule).or_insert((0, 0.0));
        *bodies += 1;
        *most = most.max(impactors.abundance());
        let rate = impactors.rate();
        self.rates = Some(self.rates.map_or((rate, rate), |(least, most)| {
            (least.min(rate), most.max(rate))
        }));
        self.young += u32::from(figures.surface_age().value() < 1e9);
    }

    /// Prints the tally.
    fn report(&self) {
        eprintln!(
            "crusts {:?}; R_b from {:?}; {} surfaces under 1 Gyr; bodies and greatest Ψ by \
             source {:?}",
            self.crusts, self.rates, self.young, self.abundances
        );
    }
}

/// What every published body's figures hold with its conditions (P14.T24.b, T48.c): its climate
/// never reaches below the night's floor that its heat flow sets; its heat flow is positive and
/// its outgassing not negative; an ice shell lies over a computed ocean; a conducting body's lid is
/// stagnant unless it is one; its surface age lies within its system's; its crater density is the
/// lunar chronology times its rate, which is at least the floor's share; and an ice-rich crust lies
/// under 273 K by its conditions' mean.
fn check_published_figures(
    index: BodyIndex,
    conditions: &SurfaceConditions,
    (figures, heat_flow): (&GlobalFigures, crate::units::WattsPerSquareMetre),
) {
    let floor = hooks::surface::night_floor(heat_flow);
    let (coldest, warmest) = conditions.climate_range();
    assert!(
        coldest >= floor && warmest >= coldest,
        "{index:?}: {coldest:?} to {warmest:?} over {floor:?}"
    );
    assert!(figures.heat_flow().value() > 0.0, "{index:?}");
    assert!(figures.outgassing() >= 0.0, "{index:?}");
    let ocean = matches!(figures.subsurface_ocean(), Subsurface::Ocean(_));
    match figures.tectonic_regime() {
        TectonicRegime::IceShell => assert!(ocean, "{index:?}"),
        TectonicRegime::MobileLid
        | TectonicRegime::Episodic
        | TectonicRegime::HeatPipe
        | TectonicRegime::StagnantLid => {
            if figures.interior() == hooks::figures::Interior::Conducting {
                assert_eq!(
                    figures.tectonic_regime(),
                    TectonicRegime::StagnantLid,
                    "{index:?}"
                );
            }
        }
    }
    let craters = figures.craters();
    let (age, system_age) = (figures.surface_age().value() / 1e9, craters.system_age());
    assert!(
        (0.0..=system_age.value()).contains(&age),
        "{index:?}: {age} Gyr"
    );
    let chronology =
        hyperion_surface::craters::lunar_chronology(crate::units::Gigayears::new(age), system_age);
    assert_same_bits(
        figures.crater_density().value(),
        (chronology * figures.impactors().rate()).value(),
    );
    let rate = figures.impactors().rate();
    assert!(rate.is_finite() && rate > 0.0, "{index:?}: R_b {rate}");
    assert!(
        figures.impactors().abundance() >= LEFTOVER_DENSITY,
        "{index:?}"
    );
    if figures.impactors().crust() == Crust::IceRich {
        assert!(
            conditions.mean_surface_temperature().value() < 273.0,
            "{index:?}"
        );
    }
    // P14.T48.b: the epoch lies inside the system's age, and only a world wet now has one to the
    // present at Earth's runoff.
    if let Some(epoch) = figures.wet_epoch() {
        let (start, end) = (epoch.start().value(), epoch.end().value());
        assert!(
            0.0 <= end && end <= start && start <= system_age.value(),
            "{index:?}: {epoch:?}"
        );
        assert!(epoch.effective_flow().value() >= 0.0, "{index:?}");
        assert!(epoch.paleo_inventory().value() > 0.0, "{index:?}");
        assert_eq!(
            epoch.liquid(),
            crate::planetary::derive::atmosphere::Gas::Water.substance()
        );
        let wet_now = (epoch.runoff_m_per_year()
            - hooks::figures::wet_epoch::WET_WORLD_RUNOFF_M_PER_YEAR)
            .abs()
            < 1e-15;
        if wet_now {
            assert!(conditions.liquid_water(), "{index:?}");
            assert_same_bits(end, 0.0);
            assert_same_bits(epoch.effective_flow().value() / 1e9, start);
        }
    }
}

/// A moon's surface conditions are read under its planet's orbit at the time, not its own
/// (P14.T24.a): rebuilt here from the derivation, they are the planet's orbit's, which differ from
/// the own orbit's where the moon's pole stands off its planet's orbital normal.
#[test]
fn a_moon_s_published_conditions_read_its_planet_s_orbit() {
    let t = UniverseTime::EPOCH;
    let mut checked = 0_u32;
    for (ctx, system) in whole().iter().take(60) {
        for moon in system.bodies() {
            let (OrbitHost::Body(parent), Part::Moon(part), Some(rotation)) =
                (moon.host, &moon.part, moon.rotation.as_ref())
            else {
                continue;
            };
            let Some(hooks) = system.hooks_at(ctx, moon.index(), t).unwrap() else {
                continue;
            };
            let Section::Ok(published) = hooks.surface() else {
                continue;
            };
            let planet = system.body(parent).unwrap();
            let epoch = Epoch::new(system, ctx, t);
            let host = fate_host(ctx, system.zone_of(planet));
            let fate = system.fate_of(planet, &host);
            let label = label::label(system, planet.index).unwrap();
            let (_, now) = system.primary_record(&epoch, &HourCosines::new(), planet, label, &fate);
            let seasonal = *fate.at(t).orbit().unwrap();
            let orbit = part.satellite.orbit_at(&part.parent, ctx.age_at(t));
            let derived = system.record_moon(&epoch, moon, (&now, &fate)).unwrap();
            let bulk = PlanetarySystem::moon_bulk_of(part, &derived, &now);
            // The greenhouse gain reads T13.c's terms at the published equilibrium temperature,
            // which is the one the moon's τ was computed at.
            let air = derived.atmosphere();
            if air.state() != SurfaceState::Airless {
                let terms = crate::planetary::derive::atmosphere::GreenhouseConstants::in_force()
                    .terms(
                        air.partial_pressures(),
                        derived.equilibrium_temperature(),
                        bulk.surface_gravity(),
                    );
                let sum = terms.iter().fold(0.0, |sum, term| sum + term.depth());
                assert_same_bits(sum, air.optical_depth());
            }
            // Its conditions read its published pass's context's heat flow and θ (rule (f)).
            let record = system.body_at(ctx, moon.index(), t).unwrap();
            let Section::Ok(surface) = record.surface() else {
                panic!("{:?} has conditions but no surface", moon.index());
            };
            let basis = *surface.basis().expect("a record's surface holds its basis");
            let inputs = |spin: SeasonalSpin| SurfaceInputs {
                atmosphere: derived.atmosphere(),
                equilibrium_temperature: derived.equilibrium_temperature(),
                surface_gravity: bulk.surface_gravity(),
                radius: Metres::from(bulk.radius()),
                material: surface.material(),
                lid: basis.lid,
                stellar_flux: WattsPerSquareMetre::from(derived.flux()),
                xuv_flux: derived.xuv_flux(),
                spin,
                heat_flow: basis.heat_flow,
                ocean_weight: basis.ocean_weight,
            };
            let frame = rotation.frame();
            let under_planet = SeasonalSpin::of(frame, t, &seasonal, SeasonsFrom::Planet);
            let under_own = SeasonalSpin::of(frame, t, &orbit, SeasonsFrom::Own);
            // The published conditions carry their climate regime (P14.T48.d), which the rebuilt
            // first stage does not.
            assert!(published.climate_regime().is_some(), "{:?}", moon.index());
            assert_eq!(
                SurfaceConditions::derive(&inputs(under_planet)),
                Some(published.clone().without_climate()),
                "{:?}",
                moon.index()
            );
            if (under_planet.obliquity.value() - under_own.obliquity.value()).abs() > 0.02 {
                let own = SurfaceConditions::derive(&inputs(under_own)).unwrap();
                let apart =
                    (own.equator_pole_contrast_k() - published.equator_pole_contrast_k()).abs();
                assert!(apart > 1e-9, "{:?}", moon.index());
                checked += 1;
            }
        }
    }
    eprintln!("{checked} moons read off their own orbit's obliquity");
    assert!(checked > 0);
}

/// The surface conditions and global figures `hooks_at` publishes, the records' surface sections,
/// pinned over the sample's first systems (P14.T24.a, T24.b, T48.e): what `SeasonalSpin::of` and
/// the bulk sections feed the derivation reaches every contrast's bits, and the heat flow the
/// night's floor.
#[test]
fn the_published_surface_conditions_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    let t = UniverseTime::EPOCH;
    let mut pinned = 0_u32;
    for (ctx, system) in whole().iter().take(8) {
        for body in system.bodies() {
            let Some(hooks) = system.hooks_at(ctx, body.index(), t).unwrap() else {
                continue;
            };
            let Section::Ok(c) = hooks.surface() else {
                continue;
            };
            let name = body.index().body_id(ctx.id()).to_string();
            w.f64(
                &format!("{name}_mean_surface_temperature"),
                c.mean_surface_temperature().value(),
            );
            w.f64(
                &format!("{name}_equator_pole_contrast_k"),
                c.equator_pole_contrast_k(),
            );
            w.f64(
                &format!("{name}_day_night_contrast_k"),
                c.day_night_contrast_k(),
            );
            let (coldest, warmest) = c.climate_range();
            w.f64(&format!("{name}_coldest"), coldest.value());
            w.f64(&format!("{name}_warmest"), warmest.value());
            w.f64(
                &format!("{name}_surface_pressure"),
                c.surface_pressure().value(),
            );
            for &(id, x) in c.gases() {
                w.f64(
                    &format!("{name}_mole_fraction_{}", id.substance().key().as_str()),
                    x,
                );
            }
            w.f64(&format!("{name}_stellar_flux"), c.stellar_flux().value());
            w.line(&format!("{name}_radiation = {:?}", c.radiation()));
            w.line(&format!("{name}_liquid_water = {}", c.liquid_water()));
            hooks::surface::regime::tests::pin_regime(&mut w, &name, c.climate_regime());
            let Section::Ok(f) = hooks.figures() else {
                panic!("{name} has conditions but no figures");
            };
            w.f64(&format!("{name}_heat_flow"), f.heat_flow().value());
            w.f64(
                &format!("{name}_non_tidal_heat_flow"),
                f.non_tidal_heat_flow().value(),
            );
            w.line(&format!("{name}_interior = {:?}", f.interior()));
            w.line(&format!(
                "{name}_tectonic_regime = {:?}",
                f.tectonic_regime()
            ));
            w.line(&format!("{name}_magnetic_field = {:?}", f.magnetic_field()));
            w.f64(&format!("{name}_outgassing"), f.outgassing());
            match f.subsurface_ocean() {
                Subsurface::Ocean(ocean) => {
                    w.f64(
                        &format!("{name}_shell_thickness"),
                        ocean.shell_thickness().value(),
                    );
                    w.f64(
                        &format!("{name}_ocean_thickness"),
                        ocean.thickness().value(),
                    );
                }
                other @ (Subsurface::NoOcean | Subsurface::NotComputed) => {
                    w.line(&format!("{name}_subsurface_ocean = {other:?}"));
                }
            }
            pin_crater_contract(&mut w, &name, f);
            hooks::figures::wet_epoch::tests::pin_history(
                &mut w,
                &name,
                f.wet_epoch(),
                f.from_colder(),
            );
            hooks::figures::tests::pin_relief(&mut w, &name, f);
            hooks::figures::tests::pin_cover(&mut w, &name, f);
            w.f64(
                &format!("{name}_continental_asymptote"),
                hooks::figures::relief::continental_asymptote(
                    system.seed,
                    body.index().body_id(ctx.id()),
                ),
            );
            pinned += 1;
        }
    }
    assert!(pinned > 20, "{pinned}");
    pin_remnant_crust(&mut w, "41ffecb200000005", "41ffecb200000005.e605");
    golden!("planetary/surface_conditions_published", w.as_str());
}

/// Pins the `H2O` solid entry of the record `body` of the sample's system `system`, a remnant's ice
/// bulk on an ice-rich crust, by bits: its area, reservoir and layer, the crust's frozen water a
/// past runaway leaves it (P14.T54.a's C0c; decision-p14-t54a-ganymede, "Remnant ice-bulk bodies'
/// water"). `41ffecb200000005.e605`, at 3.2 K about its host's remnant, lies beyond the systems
/// [`the_published_surface_conditions_are_pinned`] reads.
fn pin_remnant_crust(w: &mut hyperion_testkit::golden::GoldenWriter, system: &str, body: &str) {
    use crate::planetary::chem::partition::CondensatePhase;
    use crate::planetary::derive::atmosphere::SurfaceMaterial;
    let (ctx, generated) = whole()
        .iter()
        .find(|(ctx, _)| ctx.id().to_string() == system)
        .unwrap_or_else(|| panic!("{system} is in the sample"));
    let snapshot = generated.snapshot_at(ctx, UniverseTime::EPOCH);
    let record = snapshot
        .bodies()
        .iter()
        .find(|record| record.identity().id().to_string() == body)
        .unwrap_or_else(|| panic!("{body} is in its system"));
    let Section::Ok(surface) = record.surface() else {
        panic!("{body} has a surface");
    };
    assert_eq!(surface.material(), SurfaceMaterial::Ice, "{body}");
    assert_eq!(
        surface.figures().impactors().crust(),
        Crust::IceRich,
        "{body}"
    );
    let entry = surface
        .condensates()
        .entry(Gas::Water.substance(), CondensatePhase::Solid)
        .unwrap_or_else(|| panic!("{body} lists its crust's water"));
    assert!(entry.area_fraction() > 0.0, "{body}: {entry:?}");
    w.line(&format!("{body}_state = {:?}", surface.state()));
    w.line(&format!("{body}_crust = {:?}", Crust::IceRich));
    w.f64(
        &format!("{body}_h2o_solid_area_fraction"),
        entry.area_fraction(),
    );
    w.f64(
        &format!("{body}_h2o_solid_reservoir_kg_m2"),
        entry.reservoir().value(),
    );
    let layer = entry.reservoir_gel().expect("ice Ih's density is read");
    w.f64(&format!("{body}_h2o_solid_reservoir_gel_m"), layer.value());
}

/// P14.T48.d's published climate regime over the sample: every published body has one, read from
/// its own conditions, history and ice cover. An airless body is airless-like in radiative
/// equilibrium with no condensable; a world locked to its seasonal host takes the locked forcing
/// and no seasonal model; an isothermal surface is an efficient air's; an ice belt came from a
/// colder state with ice that covers some of its sphere, its poles free; and a condensable is a
/// condensable row the air holds.
#[test]
fn the_published_climate_regimes_hold_over_the_sample() {
    use crate::planetary::hooks::surface::regime::{by_redistribution, effective_redistribution};
    use crate::planetary::hooks::surface::{Forcing, ThermalRegime};
    use hyperion_surface::field::ClimateModelKind;
    use std::collections::BTreeMap;

    let t = UniverseTime::EPOCH;
    let mut tally: BTreeMap<String, u32> = BTreeMap::new();
    let mut count = |key: String| *tally.entry(key).or_insert(0) += 1;
    let (mut published, mut slow, mut moved) = (0_u32, 0_u32, 0_u32);
    for (ctx, system) in whole().iter().take(60) {
        for body in system.bodies() {
            let Some(hooks) = system.hooks_at(ctx, body.index(), t).unwrap() else {
                continue;
            };
            let (Section::Ok(c), Section::Ok(f)) = (hooks.surface(), hooks.figures()) else {
                continue;
            };
            let index = body.index();
            let regime = c
                .climate_regime()
                .unwrap_or_else(|| panic!("{index:?} is published with no regime"));
            published += 1;
            count(format!("thermal {:?}", regime.thermal()));
            count(format!("forcing {:?}", regime.forcing()));
            count(match regime.condensable() {
                Some(id) => format!("condensable {}", id.substance().key().as_str()),
                None => "condensable none".to_owned(),
            });
            count(format!("model {:?}", regime.model()));
            if c.gases().is_empty() {
                assert_eq!(regime.thermal(), ThermalRegime::AirlessLike, "{index:?}");
                assert_eq!(
                    regime.model(),
                    ClimateModelKind::RadiativeEquilibrium,
                    "{index:?}"
                );
                assert_eq!(regime.condensable(), None, "{index:?}");
            }
            if regime.forcing() == Forcing::Locked {
                assert_ne!(regime.model(), ClimateModelKind::EnergyBalance, "{index:?}");
            }
            if regime.model() == ClimateModelKind::Isothermal {
                assert_eq!(regime.thermal(), ThermalRegime::Efficient, "{index:?}");
            }
            if regime.forcing() == Forcing::IceBelt {
                assert!(f.from_colder(), "{index:?}");
                assert!(
                    f.ice_fraction() > 0.0 && f.ice_fraction() < 1.0,
                    "{index:?}"
                );
            }
            if let Some(id) = regime.condensable() {
                assert!(id.substance().phase_diagram().is_some(), "{index:?}");
                assert!(
                    c.gases().iter().any(|&(gas, x)| gas == id && x > 0.0),
                    "{index:?}"
                );
            }
            // "main"'s ruling of 2026-10-13: a slow rotator's label reads its damped
            // redistribution, where Koll's literal ε would have read the air's alone.
            let air = c.air_transport();
            if !c.gases().is_empty() && air.diurnal_storage.is_some_and(|storage| storage <= 1.0) {
                slow += 1;
                if by_redistribution(air.redistribution)
                    != by_redistribution(effective_redistribution(&air))
                {
                    moved += 1;
                }
            }
        }
    }
    eprintln!("{published} published regimes: {tally:?}");
    eprintln!("{slow} rotators with air in Koll's own regime, {moved} labels off his literal ε");
    assert!(published > 100, "{published}");
}

/// Pins the crater contract of the published figures `f` under `name`: its source rule, crust,
/// abundance and rate, the parts the shared density reads beside them, its surface age and its
/// N☾(>1 km) (P14.T48.c, T48.f).
fn pin_crater_contract(
    w: &mut hyperion_testkit::golden::GoldenWriter,
    name: &str,
    f: &GlobalFigures,
) {
    let (impactors, craters) = (f.impactors(), f.craters());
    let rule = match impactors.source() {
        hooks::figures::ImpactorSource::InBelt { .. } => "in_belt",
        hooks::figures::ImpactorSource::Comets => "comets",
        hooks::figures::ImpactorSource::NearestBelt { .. } => "nearest_belt",
        hooks::figures::ImpactorSource::InnerComets { .. } => "inner_comets",
    };
    w.line(&format!("{name}_impactor_rule = {rule}"));
    w.line(&format!("{name}_crust = {:?}", impactors.crust()));
    w.f64(&format!("{name}_abundance"), impactors.abundance());
    w.f64(&format!("{name}_rate"), impactors.rate());
    w.f64(
        &format!("{name}_impact_velocity"),
        impactors.velocity().value(),
    );
    w.f64(
        &format!("{name}_projectile_density"),
        impactors.projectile_density().value(),
    );
    w.line(&format!(
        "{name}_target_class = {:?}",
        impactors.target().class
    ));
    w.f64(
        &format!("{name}_target_density"),
        impactors.target().density.value(),
    );
    w.f64(&format!("{name}_k_target"), impactors.k_target());
    match craters.screening() {
        hyperion_surface::craters::Screening::Atmosphere { column_mass } => {
            w.f64(&format!("{name}_column_mass"), column_mass.value());
        }
        other @ (hyperion_surface::craters::Screening::None
        | hyperion_surface::craters::Screening::Cutoff { .. }) => {
            w.line(&format!("{name}_screening = {other:?}"));
        }
    }
    w.f64(&format!("{name}_surface_age"), f.surface_age().value());
    w.f64(
        &format!("{name}_crater_density"),
        f.crater_density().value(),
    );
}

/// The published hooks are the same whatever order the bodies are asked in, a moon before its
/// planet included (P14.T24.a).
#[test]
fn the_published_hooks_are_order_independent() {
    let (ctx, system) = whole()
        .iter()
        .find(|(_, system)| {
            system
                .bodies()
                .iter()
                .any(|body| matches!(body.host, OrbitHost::Body(_)))
        })
        .expect("the sample holds a system with moons");
    let indices: Vec<BodyIndex> = system.bodies().iter().map(Body::index).collect();
    let later = UniverseTime::from_julian_years(500).expect("in the window");
    for t in [UniverseTime::EPOCH, later] {
        assert_order_independent(&indices, |&index| system.hooks_at(ctx, index, t).unwrap());
    }
}

/// P14.T24.b's published figures read each body's interior: a planet's or a member's ocean is
/// not computed and it feels no tide; a moon's tide is its derivation's, P14.T17.b's, added to its
/// own heat flow; and a regular moon's ocean is its flag's, another moon's not computed.
#[test]
fn the_published_figures_read_each_body_s_interior() {
    let t = UniverseTime::EPOCH;
    let (mut regular, mut oceans, mut other_moons) = (0_u32, 0_u32, 0_u32);
    for (ctx, system) in whole().iter().take(60) {
        for body in system.bodies() {
            let Some(hooks) = system.hooks_at(ctx, body.index(), t).unwrap() else {
                continue;
            };
            let Section::Ok(figures) = hooks.figures() else {
                continue;
            };
            let index = body.index();
            match &body.part {
                Part::Planet(_) | Part::Member(_) => {
                    assert_eq!(
                        figures.subsurface_ocean(),
                        &Subsurface::NotComputed,
                        "{index:?}"
                    );
                    assert_same_bits(
                        figures.heat_flow().value(),
                        figures.non_tidal_heat_flow().value(),
                    );
                }
                Part::Moon(part) => {
                    let OrbitHost::Body(parent) = body.host else {
                        panic!("{index:?}: a moon goes round a body");
                    };
                    let planet = system.body(parent).unwrap();
                    let epoch = Epoch::new(system, ctx, t);
                    let host = fate_host(ctx, system.zone_of(planet));
                    let fate = system.fate_of(planet, &host);
                    let label = label::label(system, planet.index).unwrap();
                    let (_, now) =
                        system.primary_record(&epoch, &HourCosines::new(), planet, label, &fate);
                    let derived = system.record_moon(&epoch, body, (&now, &fate)).unwrap();
                    // Every moon's tide is its derivation's, P14.T17.b's.
                    assert_same_bits(
                        figures.heat_flow().value(),
                        figures.non_tidal_heat_flow().value() + derived.heat_flux().value(),
                    );
                    match part.satellite.moon() {
                        SatelliteMoon::Regular(_) => {
                            regular += 1;
                            let ocean = matches!(figures.subsurface_ocean(), Subsurface::Ocean(_));
                            assert_eq!(ocean, derived.subsurface_ocean(), "{index:?}");
                            oceans += u32::from(ocean);
                        }
                        SatelliteMoon::GiantImpact(_) | SatelliteMoon::Captured(_) => {
                            other_moons += 1;
                            assert_eq!(
                                figures.subsurface_ocean(),
                                &Subsurface::NotComputed,
                                "{index:?}"
                            );
                        }
                    }
                }
                Part::Ring(_) => panic!("{index:?}: a ring has no hooks"),
            }
        }
    }
    eprintln!("{regular} regular moons, {oceans} with oceans; {other_moons} other moons");
    assert!(regular > 5 && other_moons > 0, "{regular}, {other_moons}");
}

/// How a moon came to orbit its parent, as the published crater contract's test reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoonBirthKind {
    Regular,
    GiantImpact,
    GiantsCapture,
    RockyCapture,
}

/// The birth of the moon `body` (part `part`) of `system` and the birthplace its crust reads by
/// science-p14-t24b-laws §2.4, from its published bulk composition `composition`: a regular moon
/// is icy when it holds water; a giant-impact moon takes its parent's, icy for an icy belt's
/// member; a giant's capture is icy; a rocky planet's capture takes its neighbouring belt's.
fn moon_birth_of(
    system: &PlanetarySystem,
    body: &Body,
    part: &MoonPart,
    composition: &BulkComposition,
) -> (MoonBirthKind, hooks::figures::Birthplace) {
    use crate::planetary::belts::BeltComposition;
    use crate::planetary::moons::CaptureKind;
    let OrbitHost::Body(parent) = body.host else {
        panic!("{:?}: a moon goes round a body", body.index());
    };
    let (kind, icy) = match part.satellite.moon() {
        SatelliteMoon::Regular(_) => (
            MoonBirthKind::Regular,
            composition.fractions().water() > 0.0,
        ),
        SatelliteMoon::GiantImpact(_) => (
            MoonBirthKind::GiantImpact,
            system
                .body(parent)
                .and_then(Body::member_of)
                .is_some_and(|member| member.composition() == BeltComposition::Icy),
        ),
        SatelliteMoon::Captured(captured) => match captured.kind() {
            CaptureKind::Irregular | CaptureKind::Large => (MoonBirthKind::GiantsCapture, true),
            CaptureKind::Small => (
                MoonBirthKind::RockyCapture,
                system.nearest_belt(parent).is_some_and(|belt| {
                    belt.adjacency() == BeltAdjacency::Neighbouring
                        && belt.composition() == BeltComposition::Icy
                }),
            ),
        },
    };
    let birthplace = if icy {
        hooks::figures::Birthplace::Icy
    } else {
        hooks::figures::Birthplace::Other
    };
    (kind, birthplace)
}

/// P14.T48.c's published crater contract over the sample (science-p14-t24b-laws §2.1–2.4): the
/// projectile abundance of a body that goes round its host holds from one time to another, its
/// belts' masses taken at 4.567 Gyr, and its rate moves only with its escape speed; a planet's
/// surface is no older than the time since its magma ocean's end (P14.T28.a); and every moon's
/// crust is `Crust::of` at the birthplace its birth gives ([`moon_birth_of`]), its water and its
/// radius, so that a cold regular moon holding ice and a cold giant's capture are ice-rich, and a
/// giant-impact moon of a planet or a capture beside a rocky belt is ice-rich by its water alone.
#[test]
fn the_published_crater_contract_reads_each_body_s_history() {
    let later = UniverseTime::from_julian_years(500).expect("in the window");
    let t = UniverseTime::EPOCH;
    let (mut held, mut cold_captures, mut impacts) = (0_u32, 0_u32, 0_u32);
    for (ctx, system) in whole().iter().take(60) {
        for body in system.bodies() {
            let published_at = |time: UniverseTime| {
                let hooks = system.hooks_at(ctx, body.index(), time).unwrap()?;
                let (Section::Ok(conditions), Section::Ok(figures), Section::Ok(bulk)) =
                    (hooks.surface(), hooks.figures(), hooks.bulk())
                else {
                    return None;
                };
                Some((
                    conditions.mean_surface_temperature(),
                    figures.clone(),
                    *bulk,
                ))
            };
            let Some((mean, figures, composition)) = published_at(t) else {
                continue;
            };
            let index = body.index();
            match &body.part {
                Part::Planet(planet) => {
                    let melt = Years::from(planet.fate.formation().molten_until()).value();
                    let since = ctx.age_at(t).value() - melt;
                    assert!(figures.surface_age().value() <= since.max(0.0), "{index:?}");
                }
                Part::Member(_) => {}
                Part::Moon(part) => {
                    let record = system.body_at(ctx, index, t).unwrap();
                    let Section::Ok(bulk) = record.bulk() else {
                        panic!("{index:?} has figures but no bulk section");
                    };
                    let birth = moon_birth_of(system, body, part, &composition);
                    impacts += u32::from(matches!(birth.0, MoonBirthKind::GiantImpact));
                    let cold = mean.value() < 273.0;
                    cold_captures +=
                        u32::from(matches!(birth.0, MoonBirthKind::GiantsCapture) && cold);
                    let kg = Kilograms::from(body.mass()).value();
                    let water = composition
                        .inventory()
                        .water()
                        .value()
                        .max(composition.fractions().water() * kg);
                    let expected = hooks::figures::Crust::of(
                        birth.1,
                        mean,
                        Kilograms::new(water),
                        Metres::from(bulk.radius()),
                    );
                    assert_eq!(
                        figures.impactors().crust(),
                        expected,
                        "{index:?}: {birth:?}"
                    );
                }
                Part::Ring(_) => panic!("{index:?}: a ring has no hooks"),
            }
            if matches!(body.part, Part::Planet(_) | Part::Member(_))
                && let Some((_, after, _)) = published_at(later)
            {
                let (now, then) = (figures.impactors(), after.impactors());
                assert_same_bits(now.abundance(), then.abundance());
                assert!((now.rate() / then.rate() - 1.0).abs() < 1e-6, "{index:?}");
                held += 1;
                if now.rate() > 300.0 || now.rate() < 1e-3 {
                    eprintln!(
                        "{index:?}: {:?}, Ψ {:.3e}, V {:.3e} m/s, R_b {:.3e}",
                        now.source(),
                        now.abundance(),
                        now.encounter_speed().value(),
                        now.rate()
                    );
                }
            }
        }
    }
    eprintln!(
        "{held} rates held in time; {cold_captures} cold giants' captures; {impacts} impact moons"
    );
    assert!(
        held > 20 && cold_captures > 0 && impacts > 0,
        "{held}, {cold_captures}, {impacts}"
    );
}

/// P14.T48.b's past sky reads the hosts and orbits the derivation reads now: at the present its
/// window is each light's flux over Kopparapu's runaway limit at the stars' states now, the zone's
/// along the orbit now and each companion's along its pair's, as `Epoch::lights` and
/// `Epoch::companions` give them, and a single star's planet's is its derivation's flux over the
/// limit. The orbit it reads at any time is the fate's, bit for bit. Checked at the epoch and 500
/// years after it, where the round trip through the age is not exact.
#[test]
fn the_past_sky_is_the_derivation_s_at_the_present() {
    use crate::planetary::derive::habitable_zone::HabitableLimit;

    let later = UniverseTime::from_julian_years(500).expect("in the window");
    let (mut planets, mut multiple, mut orbits) = (0_u32, 0_u32, 0_u32);
    for t in [UniverseTime::EPOCH, later] {
        for (ctx, system) in whole().iter().take(60) {
            let epoch = Epoch::new(system, ctx, t);
            for body in system.bodies() {
                let (Part::Planet(_) | Part::Member(_)) = body.part else {
                    continue;
                };
                let zone = system.zone_of(body);
                let host = fate_host(ctx, zone);
                let fate = system.fate_of(body, &host);
                let past = PastSky::new(ctx, zone, &fate, t);
                // The orbit at past ages and at the present is the fate's.
                for share in [0.01, 0.3, 0.9] {
                    let age = Years::new(share * ctx.age_at(t).value());
                    let Some(then) = time_at_age(age, ctx.age_at_epoch()) else {
                        continue;
                    };
                    let (fast, full) = (fate.orbit_at(then), fate.at(then).orbit().copied());
                    assert_eq!(fast, full, "{:?} at {age:?}", body.index());
                    orbits += u32::from(fast.is_some());
                }
                let Some(orbit) = fate.at(t).orbit().copied() else {
                    continue;
                };
                assert_eq!(fate.orbit_at(t), Some(orbit));
                let window = past.window_at(ctx.age_at(t));
                let runaway = |flux: f64, t_eff: Kelvin| {
                    flux / HabitableLimit::RunawayGreenhouse.flux(t_eff).value()
                };
                let (a, e) = (orbit.semi_major_axis(), orbit.eccentricity().value());
                let lights = epoch.lights(zone).unwrap();
                let companions = epoch.companions(zone).unwrap();
                let expected = lights
                    .iter()
                    .filter(|light| light.luminosity().value() > 0.0)
                    .map(|light| {
                        runaway(
                            luminosity_flux(light.luminosity(), a, e).value(),
                            light.effective_temperature(),
                        )
                    })
                    .chain(
                        companions
                            .iter()
                            .filter(|c| c.host().luminosity().value() > 0.0)
                            .map(|c| runaway(c.flux().value(), c.host().effective_temperature())),
                    )
                    .fold(0.0, |sum, x| sum + x);
                assert!(
                    (window.runaway() / expected - 1.0).abs() < 1e-12,
                    "{:?}: {} against {expected}",
                    body.index(),
                    window.runaway()
                );
                multiple += u32::from(ctx.stars().len() > 1);
                if ctx.stars().len() == 1 {
                    let label = label::label(system, body.index).unwrap();
                    let (_, now) =
                        system.primary_record(&epoch, &HourCosines::new(), body, label, &fate);
                    if let Some(derived) = now.derived {
                        let star = ctx.stars()[0].state_at(t).unwrap();
                        let alone = runaway(derived.flux().value(), star.effective_temperature());
                        assert!(
                            (window.runaway() / alone - 1.0).abs() < 1e-9,
                            "{:?}",
                            body.index()
                        );
                        planets += 1;
                    }
                }
            }
        }
    }
    eprintln!(
        "{planets} single-star bodies' windows, {multiple} in multiple systems; {orbits} past orbits"
    );
    assert!(
        planets > 50 && multiple > 10 && orbits > 100,
        "{planets}, {multiple}, {orbits}"
    );
}

/// The published volatile history reads the body's own past: the window of its primary's past
/// sky from its magma ocean's end, a moon's its planet's, its outgassing then, and its conditions'
/// liquid-water flag (P14.T48.b), as rebuilt here from the parts.
#[test]
fn the_published_volatile_history_reads_each_body_s_past() {
    let t = UniverseTime::EPOCH;
    let (mut checked, mut moons, mut epochs) = (0_u32, 0_u32, 0_u32);
    for (ctx, system) in whole().iter().take(60) {
        for body in system.bodies() {
            let Some(hooks) = system.hooks_at(ctx, body.index(), t).unwrap() else {
                continue;
            };
            let (Section::Ok(conditions), Section::Ok(figures)) =
                (hooks.surface(), hooks.figures())
            else {
                continue;
            };
            let primary = match body.host {
                OrbitHost::Body(parent) => system.body(parent).unwrap(),
                OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => body,
            };
            let zone = system.zone_of(primary);
            let host = fate_host(ctx, zone);
            let fate = system.fate_of(primary, &host);
            let past = PastSky::new(ctx, zone, &fate, t);
            let molten_until = match &body.part {
                Part::Planet(planet) => Some(Years::from(planet.fate.formation().molten_until())),
                Part::Member(_) | Part::Moon(_) | Part::Ring(_) => None,
            };
            let age = ctx.age_at(t);
            let start = molten_until
                .unwrap_or(hooks::figures::wet_epoch::UNDRAWN_HISTORY_START)
                .value();
            let window = |age: Years| past.window_at(age);
            if start < age.value() {
                let expected = !window(Years::new(start)).inside_maximum_greenhouse()
                    && window(age).inside_maximum_greenhouse();
                assert_eq!(figures.from_colder(), expected, "{:?}", body.index());
            }
            if let Some(epoch) = figures.wet_epoch() {
                epochs += 1;
                // It opened inside the window, after the magma ocean.
                let opened = age.value() - epoch.start().value() * 1e9;
                assert!(opened >= start - 1.0, "{:?}", body.index());
                assert!(
                    window(Years::new(opened + 1.0)).in_window(),
                    "{:?}",
                    body.index()
                );
                if epoch.end().value() > 0.0 {
                    assert!(
                        (epoch.runoff_m_per_year()
                            - hooks::figures::wet_epoch::PAST_EPOCH_RUNOFF_M_PER_YEAR)
                            .abs()
                            < 1e-15
                    );
                } else {
                    assert!(conditions.liquid_water(), "{:?}", body.index());
                }
            }
            moons += u32::from(matches!(body.part, Part::Moon(_)));
            checked += 1;
        }
    }
    eprintln!("{checked} published histories, {moons} moons'; {epochs} wet epochs");
    assert!(checked > 100 && moons > 5, "{checked}, {moons}");
}

// P14.T56.a: every moon and ring on its parent's Laplace surface.

/// Asserts two vectors equal bit for bit.
fn same3(a: [f64; 3], b: [f64; 3]) {
    for (x, y) in a.into_iter().zip(b) {
        assert_same_bits(x, y);
    }
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The angle between two unit vectors, rad.
fn between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let c = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    crate::math::atan2(dot3(c, c).sqrt(), dot3(a, b))
}

/// P14.T56.a: the Laplace surface turns planes only. Each parent's satellites as its orbital plane
/// gives them, those `generate_satellites` finds for the parent with no frame (step 7's first
/// pass), keep their index, draws, local elements and mass on its frame, so that T22.b's
/// properties, which read nothing else, stand; and no regular moon or ring of the sample lies in an
/// eccentricity band (decision-p14-moons §1.2 expects none of 5 × 10⁻¹¹ of giants).
#[test]
fn the_laplace_surface_moves_no_satellite_s_elements_mass_or_count() {
    let (mut frames, mut moons, mut rings, mut tilted) = (0, 0, 0, 0);
    for (ctx, system) in whole() {
        for b in system.satellites() {
            let body = system.body(b.parent_index()).unwrap();
            frames += usize::from(body.laplace().is_some());
            let unframed = Body {
                laplace: None,
                ..body.clone()
            };
            let belt = match body.part {
                Part::Planet(_) => system.nearest_belt(body.index()),
                Part::Moon(_) | Part::Ring(_) | Part::Member(_) => None,
            };
            let a = &generate_satellites(SEED, ctx, &unframed, belt);
            assert_eq!(a.parent_index(), b.parent_index());
            assert_eq!(a.parent(), b.parent());
            assert_eq!(a.moonlets(), b.moonlets());
            assert_eq!(a.irregulars(), b.irregulars());
            assert_eq!(a.moons().len(), b.moons().len(), "{:?}", a.parent_index());
            let Some(parent) = a.parent() else {
                continue;
            };
            let age = ctx.age_at_epoch();
            for (x, y) in a.moons().iter().zip(b.moons()) {
                assert_eq!(
                    (x.index(), x.moon(), x.radius_rank(), x.volatiles()),
                    (y.index(), y.moon(), y.radius_rank(), y.volatiles())
                );
                assert_eq!(x.local_orbit_at(parent, age), y.local_orbit_at(parent, age));
                assert_eq!(x.plane(), &MoonPlane::OrbitalPlane);
                if matches!(y.plane(), MoonPlane::Laplace(_)) {
                    tilted += 1;
                }
                moons += 1;
            }
            assert_eq!(a.rings().len(), b.rings().len(), "{:?}", a.parent_index());
            for (x, y) in a.rings().iter().zip(b.rings()) {
                assert_eq!(
                    (
                        x.index(),
                        x.kind(),
                        x.inner_edge(),
                        x.outer_edge(),
                        x.mass(),
                        x.gaps()
                    ),
                    (
                        y.index(),
                        y.kind(),
                        y.inner_edge(),
                        y.outer_edge(),
                        y.mass(),
                        y.gaps()
                    )
                );
                same3(x.normal(), parent.orbit().orientation().normal());
                rings += 1;
            }
        }
    }
    assert!(
        frames > 300 && moons > 100 && rings > 50 && tilted * 10 > moons * 9,
        "{frames} frames, {moons} moons ({tilted} on a Laplace plane), {rings} rings"
    );
}

/// P14.T56.a: each moon's orbit lies at its drawn inclination to its own local Laplace plane,
/// whose pole lies between its parent's spin pole and orbit normal.
#[test]
fn every_moon_lies_at_its_drawn_tilt_to_its_local_laplace_plane() {
    let mut moons = 0;
    for (ctx, system) in whole() {
        let age = ctx.age_at(parent_time(ctx));
        for found in system.satellites() {
            let (Some(parent), Some(frame)) = (
                found.parent(),
                system.body(found.parent_index()).and_then(Body::laplace),
            ) else {
                continue;
            };
            for moon in found.moons() {
                let MoonPlane::Laplace(plane) = moon.plane() else {
                    panic!("{:?} of a parent with a frame lies off it", moon.index());
                };
                let local = moon.local_orbit_at(parent, age);
                let pole = plane.pole_at(local.semi_major_axis());
                let normal = moon.orbit_at(parent, age).orientation().normal();
                let tilt = between(normal, pole);
                assert!(
                    (tilt - local.inclination().value()).abs() < 1e-7,
                    "{:?}",
                    moon.index()
                );
                let (spin, orbit) = (frame.spin_pole(), frame.orbit_normal());
                match moon.moon() {
                    SatelliteMoon::Captured(_) => {
                        assert_eq!(plane.sense(), crate::planetary::laplace::PoleSense::Orbit);
                        assert!(dot3(pole, orbit) > 0.0, "{:?}", moon.index());
                    }
                    SatelliteMoon::Regular(_) | SatelliteMoon::GiantImpact(_) => {
                        assert_eq!(plane.sense(), crate::planetary::laplace::PoleSense::Spin);
                        assert!(dot3(pole, spin) > 0.0, "{:?}", moon.index());
                    }
                }
                let sided = if dot3(pole, orbit) < 0.0 {
                    pole.map(|v| -v)
                } else {
                    pole
                };
                let across = between(spin, sided).min(between(spin.map(|v| -v), sided));
                let apart = between(spin, orbit).min(between(spin.map(|v| -v), orbit));
                assert!(across <= apart + 1e-12, "{:?}", moon.index());
                assert!(plane.laplace_radius().value() > 0.0);
                moons += 1;
            }
        }
    }
    assert!(moons > 100, "{moons} moons");
}

/// P14.T56.a: a ring lies on its parent's local Laplace plane at the geometric mean of its edges,
/// with the regular moons inside that radius counted, on the spin's side. Close in that is the
/// equator: under 0.1 `r_L` the pole is within ¼ sin 2ε′ (r ÷ `r_L`)⁵, the small-radius limit of
/// the plane's tilt, and within 10⁻⁹ rad wherever that limit is (a technical correction of the plan's
/// "10⁻⁹ rad under 0.1 `r_L`", which the surface's own law exceeds there: Saturn's rings, at 0.063
/// `r_L`, tilt 1.9 × 10⁻⁷ rad).
#[test]
fn every_ring_lies_on_its_local_laplace_plane_and_close_in_on_the_equator() {
    let (mut rings, mut close, mut nanoradian) = (0, 0, 0);
    for (_, system) in whole() {
        for found in system.satellites() {
            let Some(frame) = system.body(found.parent_index()).and_then(Body::laplace) else {
                continue;
            };
            for ring in found.rings() {
                let middle =
                    Metres::new((ring.inner_edge().value() * ring.outer_edge().value()).sqrt());
                let inside = found.moons().iter().filter_map(|moon| match moon.moon() {
                    SatelliteMoon::Regular(regular) => {
                        let a = regular.orbit().semi_major_axis();
                        (a < middle).then(|| crate::planetary::laplace::InnerMoon {
                            mass_ratio: Kilograms::from(regular.mass()).value()
                                / frame.parent_mass().value(),
                            semi_major_axis: a,
                        })
                    }
                    SatelliteMoon::GiantImpact(_) | SatelliteMoon::Captured(_) => None,
                });
                let rl = frame.laplace_radius(inside);
                let expected = frame
                    .local(rl, crate::planetary::laplace::PoleSense::Spin)
                    .pole_at(middle);
                same3(ring.normal(), expected);
                let x = ring.outer_edge().value() / rl.value();
                if x < 0.1 {
                    let e = frame
                        .obliquity()
                        .value()
                        .min(PI - frame.obliquity().value());
                    let limit = 0.25 * crate::math::sin(2.0 * e) * crate::math::powi(x, 5);
                    let tilt = between(ring.normal(), frame.spin_pole());
                    assert!(
                        tilt <= limit * (1.0 + 1e-6) + 1e-15,
                        "{:?}: {tilt} > {limit}",
                        ring.index()
                    );
                    if limit < 1e-9 {
                        assert!(tilt < 1e-9);
                        nanoradian += 1;
                    }
                    close += 1;
                }
                rings += 1;
            }
        }
    }
    assert!(
        rings > 50 && close > 40 && nanoradian > 0,
        "{rings} rings, {close} close in, {nanoradian} within 1 nrad"
    );
}

/// P14.T56.a with P14.T30.a: on the Laplace surface `generate_satellites` for a planet or member,
/// on the frame its body holds, equals its children in `generate`, over 1,000 systems. A
/// giant-impact moon's plane is held where its drift law anchors it (decision-p14-moons §1.2,
/// "Read once"); the largest turn of its local Laplace plane across the clock window, which bounds
/// every step between its drift cells, is under the ruling's 0.1° (5.0 × 10⁻⁴° over this sample's
/// 1,024 giant-impact moons, 2026-10-15).
#[test]
fn on_the_laplace_surface_generate_satellites_is_each_parent_s_children_in_generate() {
    let contexts = sample_contexts(1_000, Seed::new(0x5a7e_1117), SampleFilter::ALL).unwrap();
    let (mut compared, mut impacts, mut widest) = (0, 0, 0.0_f64);
    for ctx in &contexts {
        let system = generate(SEED, ctx);
        let (start, end) = (ctx.age_at(ClockWindow::START), ctx.age_at(ClockWindow::END));
        let parents = system
            .bodies()
            .iter()
            .filter(|body| matches!(body.kind(), BodyKind::Planet | BodyKind::DwarfPlanet));
        for parent in parents {
            let belt = if parent.kind() == BodyKind::Planet {
                system.nearest_belt(parent.index())
            } else {
                None
            };
            let alone = generate_satellites(SEED, ctx, parent, belt);
            assert_eq!(
                alone,
                system.satellites_of(parent.index()),
                "{:?}",
                parent.index()
            );
            let children: Vec<BodyIndex> =
                system.children(parent.index()).map(Body::index).collect();
            let mut expected: Vec<BodyIndex> = alone
                .moons()
                .iter()
                .map(Satellite::index)
                .chain(alone.rings().iter().map(Ring::index))
                .collect();
            expected.sort_unstable();
            assert_eq!(children, expected);
            compared += 1;
            let Some(moon_parent) = alone.parent() else {
                continue;
            };
            for moon in alone.moons() {
                let (SatelliteMoon::GiantImpact(impact), MoonPlane::Laplace(plane)) =
                    (moon.moon(), moon.plane())
                else {
                    continue;
                };
                let a = |age| impact.orbit_at(moon_parent, age).semi_major_axis();
                let turn = between(plane.pole_at(a(start)), plane.pole_at(a(end)));
                widest = widest.max(turn);
                impacts += 1;
            }
        }
    }
    assert!(
        compared > 1_000 && impacts > 100,
        "{compared} parents, {impacts} impact moons"
    );
    assert!(
        widest.to_degrees() < 0.1,
        "a giant-impact moon's plane turns {}° across the window",
        widest.to_degrees()
    );
}

/// P14.T56.a: each planet's and member's frame is read off its own held law and published figure
/// at `parent_time`: its spin pole, the normal of the orbit its law was derived on, J₂ of its
/// figure's equatorial radius and moment of inertia at its spin rate then, and its age then.
#[test]
fn each_parent_s_frame_is_its_own_law_and_figure_at_its_time() {
    let mut frames = 0;
    for (ctx, system) in whole().iter().take(150) {
        let t = parent_time(ctx);
        for body in system.bodies() {
            let primary = matches!(body.part, Part::Planet(_) | Part::Member(_));
            let Some(frame) = body.laplace() else {
                assert!(
                    !primary || body.rotation.is_none(),
                    "{:?} has a law and no frame",
                    body.index()
                );
                continue;
            };
            let law = body.rotation.expect("a body with a frame has a law");
            // The frame takes its axes as unit vectors again, to the last bit or so.
            assert!(between(frame.spin_pole(), law.frame().pole()) < 1e-15);
            let record = system.body_at(ctx, body.index(), t).unwrap();
            let (Section::Ok(orbit), Section::Ok(bulk), Section::Ok(mass)) =
                (record.orbit(), record.bulk(), record.mass())
            else {
                panic!("{:?} has a frame and no record then", body.index());
            };
            let elements = system.elements_at_parent_time(ctx, body, orbit.elements());
            assert!(between(frame.orbit_normal(), elements.orientation().normal()) < 1e-15);
            let Section::Ok(figure) = body.figure_section(Some(bulk), *mass, t) else {
                panic!("{:?} has no figure", body.index());
            };
            let radius = Metres::new(figure.spheroid().equatorial_radius_m);
            let spin = law.frame();
            let j2 = hydrostatic_j2(
                figure.moment_factor(),
                spin.rate().rate_at(t),
                radius,
                Kilograms::from(*mass),
                spin.state_at(t),
            );
            assert_same_bits(frame.j2(), j2);
            assert_same_bits(frame.equatorial_radius().value(), radius.value());
            assert_same_bits(frame.read_at().value(), ctx.age_at(t).value());
            frames += 1;
        }
    }
    assert!(frames > 100, "{frames} frames");
}

/// P14.T56.a with P14.T46.b (b): on the Laplace surface every body's held law, a moon's derived
/// on its plane after its parent's, is still the one derived from its record at `parent_time`,
/// bit for bit, whatever order the bodies are asked in.
#[test]
fn on_the_laplace_surface_every_law_is_its_record_s() {
    let mut laws = 0;
    for (ctx, system) in whole().iter().take(150) {
        let indices: Vec<BodyIndex> = system.bodies().iter().map(Body::index).collect();
        let held = |index: BodyIndex| system.rotation_of(ctx, index).unwrap();
        for &index in &indices {
            let body = system.body(index).unwrap();
            let record = system.body_at(ctx, index, parent_time(ctx)).unwrap();
            let derived = system.derive_rotation(ctx, body, &record);
            assert_eq!(held(index), derived, "{index:?}");
            laws += usize::from(derived.is_some());
        }
        assert_order_independent(&indices, |&index| held(index));
    }
    assert!(laws > 300, "{laws} laws");
}

/// P14.T56.c and its follow-up: every moon's inventory is the one its own ranks and site give
/// ([`Satellite::own_volatiles`](crate::planetary::satellites::Satellite::own_volatiles)) at its
/// own bulk ([`PlanetarySystem::moon_own_bulk`]), and its atmosphere's escape is judged at its
/// record's own radius and mass, which for a regular moon is its derivation's
/// ([`DerivedMoon::radius`]). From P14.T54.a its bulk composition holds its derivation's element
/// inventory, whose carriers are that inventory.
#[test]
fn every_moon_s_inventory_is_its_own_ranks_at_its_own_bulk() {
    use crate::planetary::derive::atmosphere::Retention;
    use crate::planetary::moons::regular::{MoonBulk, MoonVolatiles, derive_moon};
    let (mut moons, mut stated, mut own_ranks) = (0, 0, 0);
    for (ctx, system) in whole().iter().take(150) {
        let t = UniverseTime::EPOCH;
        for body in system.bodies() {
            let (Part::Moon(part), OrbitHost::Body(parent)) = (&body.part, body.host) else {
                continue;
            };
            let index = body.index();
            let Some((mine, elements)) =
                system.hooks_at(ctx, index, t).unwrap().and_then(|hooks| {
                    hooks
                        .bulk()
                        .ok()
                        .map(|bulk| (*bulk.inventory(), *bulk.element_inventory()))
                })
            else {
                continue;
            };
            assert_eq!(&mine, elements.volatiles(), "{index:?}");
            let planet = system.body(parent).unwrap();
            let epoch = Epoch::new(system, ctx, t);
            let host = fate_host(ctx, system.zone_of(planet));
            let fate = system.fate_of(planet, &host);
            let label = label::label(system, planet.index).unwrap();
            let (_, now) = system.primary_record(&epoch, &HourCosines::new(), planet, label, &fate);
            let satellite = &part.satellite;
            let orbit = satellite.orbit_at(&part.parent, ctx.age_at(t));
            let bulk = PlanetarySystem::moon_own_bulk(part, &orbit, &now);
            let mut inputs = MoonInputs {
                mass: satellite.mass(),
                orbit,
                radius_rank: satellite.radius_rank(),
                volatiles: satellite.own_volatiles(),
                bulk,
            };
            let derive = |inputs: &MoonInputs| {
                derive_moon(
                    inputs,
                    &part.parent,
                    now.nursery.as_ref().unwrap(),
                    now.sky.as_ref().unwrap(),
                    ctx.age_at_epoch(),
                    t,
                )
                .unwrap()
            };
            let derived = derive(&inputs);
            assert_eq!(*derived.inventory(), mine, "{index:?}");
            assert_eq!(*derived.element_inventory(), elements, "{index:?}");
            // Its own ranks, not the median's, make its inventory.
            inputs.volatiles = MoonVolatiles {
                site: satellite.volatile_site(),
                ..MoonVolatiles::SHARED
            };
            own_ranks += usize::from(*derive(&inputs).inventory() != mine);
            // The air's escape reads the record's own radius and mass: the stated bulk's, or a
            // regular moon's derivation's.
            let radius = match bulk {
                MoonBulk::Stated { radius, .. } => {
                    stated += 1;
                    radius
                }
                MoonBulk::Derived | MoonBulk::Kept { .. } => derived.radius(),
            };
            let record = PlanetarySystem::moon_bulk_of(part, &derived, &now);
            assert_same_bits(EarthRadii::from(radius).value(), record.radius().value());
            let air = derived.atmosphere();
            // With what an ice crust holds as ice beside it (decision-p14-t54a-ganymede, C0
            // follow-up, rule (b)).
            let escape = Retention::of(
                Kilograms::from(satellite.mass()),
                radius,
                air.exobase_temperature(),
            )
            .with(air.ice_held())
            .with_methane_as_nitrogen();
            assert_eq!(air.retention(), escape, "{index:?}");
            moons += 1;
        }
    }
    assert!(
        moons > 100 && stated > 10 && own_ranks * 10 > moons * 9,
        "{moons} moons, {stated} stated bulks, {own_ranks} moved by their own ranks"
    );
}

// P14.T56.d: moons' Love numbers and dissipation by state.

/// P14.T56.d: under its own tides every moon's stored rotation law is its record's at
/// `parent_time` through `derive_rotation`'s moon arm, with its own pair, and a regular moon's
/// locking time is its derivation's, to its primordial period (decision-p14-moons §3.4 (d): "so
/// that the stored rotation law and `DerivedMoon::locking_time` agree"), for every moon its first
/// round held (P14.T54.a's Q2; `a_moon_s_law_is_held_after_at_most_two_rounds` lists the second's).
#[test]
fn under_their_own_tides_a_moon_s_law_and_its_locking_time_agree() {
    use crate::planetary::moons::regular::MOON_PRIMORDIAL_PERIOD_HOURS;
    let (mut laws, mut regular, mut second) = (0, 0, 0);
    for (ctx, system) in whole().iter().take(150) {
        let t = parent_time(ctx);
        let epoch = Epoch::new(system, ctx, t);
        for body in system.bodies() {
            let (Part::Moon(part), OrbitHost::Body(parent)) = (&body.part, body.host) else {
                continue;
            };
            let index = body.index();
            let record = system.body_at(ctx, index, t).unwrap();
            let law = system.derive_rotation(ctx, body, &record);
            assert_eq!(system.rotation_of(ctx, index).unwrap(), law, "{index:?}");
            let Some(law) = law else {
                continue;
            };
            laws += 1;
            let primary = system.body(parent).unwrap();
            let host = fate_host(ctx, system.zone_of(primary));
            let fate = system.fate_of(primary, &host);
            let cosines = HourCosines::new();
            let now = system.parent_now(&epoch, &cosines, parent, &fate);
            let held = system
                .moon_law(&epoch, &cosines, body, part, (&now, &fate))
                .unwrap();
            if matches!(held.round, MoonRound::Second { .. }) {
                second += 1;
                continue;
            }
            let derived = system.record_moon(&epoch, body, (&now, &fate)).unwrap();
            if matches!(part.satellite.moon(), SatelliteMoon::Regular(_)) {
                let fixed = MOON_PRIMORDIAL_PERIOD_HOURS * 3_600.0;
                let expected =
                    law.locking_time().value() * (law.primordial_period().value() / fixed);
                let tau = derived.locking_time().value();
                assert!(
                    (tau / expected - 1.0).abs() < 1e-12,
                    "{index:?}: {tau} against {expected}"
                );
                regular += 1;
            }
        }
    }
    eprintln!("{laws} laws, {regular} regular moons; {second} held after round 2");
    assert!(
        laws > 100 && regular > 50,
        "{laws} laws, {regular} regular moons"
    );
}

/// P14.T56.d (decision-p14-moons §F-ocean F3): under their own tides a moon's published heat
/// flow is P14.T24.b's `F_nt` with its tide, and its branch's whole heat flow to 10⁻⁹ whatever
/// its branch (its `F_nt` read at no tide and no ocean); a regular moon's published ocean is its
/// branch's, kept at `F_ocean`; another moon's is not published.
#[test]
fn under_their_own_tides_a_regular_moon_s_published_ocean_is_its_branch_s() {
    use crate::planetary::hooks::figures::surface_age::resurfacing_time;
    use crate::planetary::moons::regular::ICE_CONDUCTIVITY_COEFFICIENT;
    use crate::planetary::moons::tides::TidalBranch;
    let t = UniverseTime::EPOCH;
    let (mut regular, mut oceans, mut other) = (0_u32, 0_u32, 0_u32);
    let (mut ice_rich, mut at_its_tau) = (0_u32, 0_u32);
    let mut branches = [0_u32; 3];
    for (ctx, system) in whole().iter().take(150) {
        let epoch = Epoch::new(system, ctx, t);
        for body in system.bodies() {
            let (Part::Moon(part), OrbitHost::Body(parent)) = (&body.part, body.host) else {
                continue;
            };
            let index = body.index();
            let Some(hooks) = system.hooks_at(ctx, index, t).unwrap() else {
                continue;
            };
            let Section::Ok(figures) = hooks.figures() else {
                continue;
            };
            let primary = system.body(parent).unwrap();
            let host = fate_host(ctx, system.zone_of(primary));
            let fate = system.fate_of(primary, &host);
            let now = system.parent_now(&epoch, &HourCosines::new(), parent, &fate);
            let derived = system.record_moon(&epoch, body, (&now, &fate)).unwrap();
            let tides = derived.tides();
            assert_same_bits(
                figures.heat_flow().value(),
                figures.non_tidal_heat_flow().value() + derived.heat_flux().value(),
            );
            // Its branch read the heat flow the figures publish: its F_nt at no tide and no ocean
            // is the figures' at its tide and ocean.
            let published = figures.heat_flow().value();
            let f = tides.heat_flow().value();
            assert!(
                (published / f - 1.0).abs() < 1e-9,
                "{index:?}: {published} {f}"
            );
            // An ice-rich surface's age reads the moons' tides' resurfacing calibration
            // (decision-p14-t56d-membrane §4.6): never older than its τ there.
            if figures.impactors().crust() == Crust::IceRich {
                let tau = resurfacing_time(Crust::IceRich, figures.heat_flow()).value();
                let age = figures.surface_age().value();
                assert!(
                    age <= tau * (1.0 + 1e-12),
                    "{index:?}: {age} yr against τ {tau}"
                );
                ice_rich += 1;
                at_its_tau += u32::from((age / tau - 1.0).abs() < 1e-12);
            }
            branches[match tides.branch() {
                TidalBranch::Cold => 0,
                TidalBranch::Ocean => 1,
                TidalBranch::Heated => 2,
            }] += 1;
            let SatelliteMoon::Regular(_) = part.satellite.moon() else {
                assert_eq!(
                    figures.subsurface_ocean(),
                    &Subsurface::NotComputed,
                    "{index:?}"
                );
                other += 1;
                continue;
            };
            regular += 1;
            match figures.subsurface_ocean() {
                Subsurface::Ocean(ocean) => {
                    assert_eq!(tides.branch(), TidalBranch::Ocean, "{index:?}");
                    let t_s = derived.equilibrium_temperature().value();
                    let shell = ICE_CONDUCTIVITY_COEFFICIENT * crate::math::ln(273.0 / t_s) / f;
                    assert!((ocean.shell_thickness().value() / shell - 1.0).abs() < 1e-12);
                    oceans += 1;
                }
                Subsurface::NoOcean => {
                    assert_ne!(tides.branch(), TidalBranch::Ocean, "{index:?}");
                }
                Subsurface::NotComputed => panic!("{index:?}: a regular moon's ocean is computed"),
            }
        }
    }
    eprintln!(
        "{regular} regular moons, {oceans} with oceans; {other} other moons; branches (cold, \
         ocean, heated) {branches:?}; {ice_rich} ice-rich surfaces, {at_its_tau} at their τ"
    );
    assert!(
        regular > 20 && oceans > 5 && other > 0,
        "{regular}, {oceans}, {other}"
    );
    assert!(
        at_its_tau > 0,
        "{ice_rich} ice-rich surfaces, none at its τ"
    );
}

/// P14.T51.h with P14.T54.a's passes over the record (decision-p14-t54a-passes §1.8, §1.9 tests
/// 9–13, and "Cost escalation (after Batch B)", rule (f) and item 5): over every present planet,
/// member and moon of the first 60 whole systems at the epoch, the photometry states the
/// derivation's albedo, and the passes' re-check is tallied by kind.
///
/// - **Every body:** at most `MOST_PASSES` (94) passes; its equilibrium temperature at its
///   published albedo; derived over its record (its law's), never the stand-in.
/// - **Every converged body:** its published context ≡ K(published) (§1.2), and the record's
///   figures, its own air's K(P*), agree with the context its published pass read: the basins,
///   the water share, the heat flow and the surface age within 10⁻⁹, the key to the bit.
/// - **Rule (f), every body:** one pass at its published albedo under its published context
///   reproduces its derivation to the bit, and its publication is the record's: the conditions'
///   range and contrasts (which read θ) to the bit, and the condensates' entries to the bit, their
///   substances and phases included, but for an ice bulk's crust, which the record lists and the
///   pass does not (listed). A column with nothing that condenses reads no climate there.
/// - **Every bracketed body:** its joint residual |F(A*, K(P*)) − A*|, one test-only pass at its
///   published albedo under the context its own air gives, stated (§1.5); a fixed point's at
///   most 10⁻³. Its class: a diagonal jump (residual above `JUMP_RESIDUAL`), a context jump, a
///   lossless frost or a fixed point, and whether it took the second round. Derived twice, the
///   same.
/// - **The limits** (over all bodies, item 5): at most 3% bracketed and 1% jumping, and planets
///   jumping at most 2%. Reported by kind with each kind's pass histogram.
/// - **The lid** (P14.T54.a's C3; the first relay's sample test 11, item 17's constraint 1): every
///   body's published lid, `crust_lid`, is the lid the published pass took again at its published
///   albedo and context, whose row its regolith read, and its record's crust's lid row, to the
///   bit. Every body whose lid changed between its passes after its first (under C₀, which takes
///   none) is listed, settled or bracketed (§1.8: a lid flip).
///   The trace's last pass is `PassDriver::Again`'s, at the published albedo under the published
///   context after every round the solve ran, a context jump's second round included, so its lid
///   is the published pass's re-run (Batch D's review, D3-3). Where a pass's lid built its
///   record's conditions for its cometary test, reading none of its own, their warmest is its
///   record's, to the bit (D3-2).
/// - **The radiative ground** (the six points' item 5; Batch D's review, D3-1 and PD-1; Batch E's
///   review, I1): every airless body's record's mean surface temperature is its equilibrium
///   temperature to the bit. Every body whose climate regime takes R09's radiative model
///   (`ClimateModelKind::RadiativeEquilibrium`, airless and thin airs alike) and whose inputs
///   `for_body` reads has the Γ R09's model reads of them (`Regolith::of`) its record's
///   conditions' ground's, the same bits (asserted), bracketed ones included: since Batch E's D3
///   the inputs carry the record's Γ. The bodies `for_body` answers not modelled (no crust, an
///   unlit record, an ice the registry has no density of) are counted.
/// - **A column with nothing that condenses** reads no climate in its pass, and its θ is 0 to the
///   bit (R2-8).
/// - **The per-pass trace** (§1.8; P14.T54.a's Batch E, D1): every body's solve, run again with each
///   pass recorded (`PassDriver::Traced`), is its derivation to the bit, one traced pass a pass.
///   Each jump across the diagonal is listed by class, by what differs between its bracket's ends
///   under the frozen context (design-batch-c §4): the state (the state edge), the lid row (a lid
///   flip), the capped set (an onset, decision-pre-23-review item 22), else a cover flip. Each
///   context jump is listed by the key's members that differ. The settled bodies whose context's
///   key changed during their passes, from the third pass's context on (a lid first taken aside),
///   or whose capped set changed from pass to pass, are listed by member, not counted (items 17
///   and 22).
/// - **E2** (decision-pre-23-review item 30): no body's area mix at its clamp,
///   `LARGEST_BOND_ALBEDO`, and the area mix weights more than three bands, which T51.b's anchors'
///   tests read.
/// - **The cover rule and the water follow-up check** ([`check_cover_rule`] over the published
///   pass's partition, the record's by rule (f)): every liquid entry with an area is liquid where
///   it lies; the phases `condensate_of`'s fallback set are listed.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the sample's planets, members and moons, tallied and listed"
)]
fn under_the_batch_the_photometry_states_the_derivation_s_albedo() {
    use std::collections::BTreeMap;

    use hyperion_surface::field::ClimateModelKind;

    use crate::planetary::chem::partition::{Basins, Condensates};
    use crate::planetary::derive::atmosphere::LARGEST_BOND_ALBEDO;
    use crate::planetary::derive::atmosphere::spun_read::{self, SpunRead};
    use crate::planetary::derive::passes::{
        Context, ContextFit, Convergence, JUMP_RESIDUAL, MOST_PASSES, floats_agree,
    };
    use crate::planetary::derive::photometry::area_mix::OPTICS_BANDS;
    use crate::planetary::derive::{
        DerivedBody, TracedPass, Under, equilibrium_temperature, traced,
    };
    use crate::planetary::hooks::crust::{CrustLid, CrustLithology, LidCrust};
    use crate::planetary::hooks::figures::cover::water_surface_share;
    use crate::planetary::hooks::surface::OceanWeight;
    use crate::planetary::surface::steps::climate::regolith_inertia_of;
    use crate::planetary::surface::{CoarseInputs, ReadCoarseInputsError, SurfaceInputsError};

    /// What changed discretely during the passes of a settled body's trace `trace` (§1.8's listed,
    /// not counted; decision-pre-23-review items 17 and 22): its context's key from the third
    /// pass's context on, C₀ and the first K aside, a lid first taken no change; and its capped set
    /// from pass to pass, an onset. The members that changed, and each change with the pass that
    /// read or gave it.
    fn settled_moves(trace: &[TracedPass]) -> (Vec<&'static str>, Vec<String>) {
        let (mut members, mut moves) = (Vec::new(), Vec::new());
        for (n, pair) in trace.windows(2).enumerate().skip(1) {
            let (a, b) = (pair[0].context.key(), pair[1].context.key());
            for member in key_members(&a, &b) {
                if member == "lid" && a.lid.is_none() {
                    continue;
                }
                if !members.contains(&member) {
                    members.push(member);
                }
                moves.push(format!("{member} read at pass {}", n + 2));
            }
        }
        for (n, pair) in trace.windows(2).enumerate() {
            let (a, b) = (capped_gases(&pair[0].air), capped_gases(&pair[1].air));
            if a != b {
                if !members.contains(&"onset") {
                    members.push("onset");
                }
                moves.push(format!("capped {a:?} → {b:?} at pass {}", n + 2));
            }
        }
        (members, moves)
    }

    /// One traced pass, described.
    fn described_pass(p: &TracedPass) -> String {
        format!(
            "read {:.10}, gives {:.10}, T_eq {:.3} K, {:?} at {:.2} K under {:.4e} Pa, lid {:?}, \
             capped {:?}",
            p.read.value(),
            p.air.albedo().value(),
            p.equilibrium.value(),
            p.air.state(),
            p.air.surface_temperature().value(),
            p.air
                .surface_pressure()
                .map_or(0.0, crate::units::Pascals::value),
            p.lid.map(|lid| lid.lithology()),
            capped_gases(&p.air),
        )
    }

    /// The class of a jump across the diagonal (design-batch-c §4, decision-p14-t54a-passes §1.8):
    /// what differs between the bracket's ends under its frozen context `context`, the published
    /// pass at the albedo `published` and the nearest pass of `trace` under that context on the
    /// other side of the diagonal: the state (the 100 Pa edge, the melt's), else the lid row (a lid
    /// flip), else the capped set (an onset), else a cover flip. With both ends described.
    fn jump_class(
        trace: &[TracedPass],
        published: BondAlbedo,
        context: &PassContext,
    ) -> (&'static str, String) {
        let rises = |p: &TracedPass| p.air.albedo().value() - p.read.value() >= 0.0;
        let under = |p: &&TracedPass| p.context.rank(context).is_eq();
        let Some(end) = trace
            .iter()
            .filter(under)
            .find(|p| p.read.value().total_cmp(&published.value()).is_eq())
        else {
            return ("unresolved", format!("no traced pass at {published:?}"));
        };
        let apart = |p: &TracedPass| (p.read.value() - published.value()).abs();
        let Some(other) = trace
            .iter()
            .filter(under)
            .filter(|p| rises(p) != rises(end))
            .min_by(|a, b| apart(a).total_cmp(&apart(b)))
        else {
            return ("unresolved", described_pass(end));
        };
        let class = if end.air.state() != other.air.state() {
            "state edge"
        } else if end.lid.map(|lid| lid.row()) != other.lid.map(|lid| lid.row()) {
            "lid flip"
        } else if capped_gases(&end.air) != capped_gases(&other.air) {
            "onset"
        } else {
            "cover flip"
        };
        (
            class,
            format!(
                "published {}; the other end {}",
                described_pass(end),
                described_pass(other)
            ),
        )
    }

    /// The lithologies a body's passes took, in order, each change kept once: its lid flips.
    fn lid_changes(lids: &[Option<CrustLid>]) -> Vec<Option<CrustLithology>> {
        let mut seen: Vec<Option<CrustLithology>> = Vec::new();
        for lid in lids.iter().skip_while(|lid| lid.is_none()) {
            let lithology = lid.map(|lid| lid.lithology());
            if seen.last() != Some(&lithology) {
                seen.push(lithology);
            }
        }
        seen
    }

    /// The classes of a bracketed body (decision-p14-t51-batch §4, decision-p14-t54a-passes
    /// §1.8).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Class {
        /// A jump across the diagonal: a state or cover change with no self-consistent albedo.
        Jump,
        /// A context jump: no self-consistent context among the two rounds'.
        ContextJump,
        /// A lossless frost at the area mix's clamp.
        Frost,
        /// A fixed point that simple iteration oscillates about.
        FixedPoint,
    }

    /// What one kind tallies.
    #[derive(Debug, Default)]
    struct Tally {
        bodies: u32,
        passes: BTreeMap<u32, u32>,
        bracketed: BTreeMap<Class, Vec<String>>,
        second_round: u32,
        largest_joint: f64,
    }

    /// The record's surface against the published pass's publication, re-run (rule (f)): the
    /// conditions' range and contrasts to the bit, and the condensates' entries, but for an ice
    /// bulk's water, whose crust the record lists. Whether the entries differed (an ice bulk's).
    fn same_publication(what: &str, surface: &Surface, read: &SpunRead) -> bool {
        let record = surface.conditions();
        if let Some(pass) = &read.conditions {
            let bits = |c: &SurfaceConditions| {
                let (cold, warm) = c.climate_range();
                [
                    cold.value(),
                    warm.value(),
                    c.mean_surface_temperature().value(),
                    c.equator_pole_contrast_k(),
                    c.day_night_contrast_k(),
                ]
            };
            for (a, b) in bits(record).iter().zip(bits(pass)) {
                assert_same_bits(*a, b);
            }
        }
        let entries = |c: &Condensates, ice_bulk_water: bool| {
            c.entries()
                .iter()
                .filter(|e| !(ice_bulk_water && e.substance() == Gas::Water.substance()))
                .map(|e| format!("{e:?}"))
                .collect::<Vec<_>>()
        };
        let ice_bulk = surface.material() == SurfaceMaterial::Ice;
        assert_eq!(
            entries(surface.condensates(), ice_bulk),
            entries(&read.condensates, ice_bulk),
            "{what}"
        );
        ice_bulk && entries(surface.condensates(), false) != entries(&read.condensates, false)
    }

    /// Whether the record's figures `figures` agree with the context `context` the published pass
    /// read: the heat flow, the surface age, the water share and the basins' layers within 10⁻⁹,
    /// and the craters' crust and the regime to the bit.
    fn figures_agree(figures: &GlobalFigures, context: &PassContext) -> bool {
        let Some(first) = context.figures() else {
            return false;
        };
        let basins = Basins::of(figures.relief().hypsometry());
        let layer = |b: &Basins| b.known().map(|(_, full)| full.value());
        floats_agree(figures.heat_flow().value(), first.heat_flow().value())
            && floats_agree(figures.surface_age().value(), first.surface_age().value())
            && floats_agree(
                water_surface_share(figures.outgassing()),
                context.water_share(),
            )
            && layer(&basins)
                .zip(layer(&context.basins()))
                .is_some_and(|(a, b)| floats_agree(a, b))
            && figures.tectonic_regime() == first.tectonic_regime()
            && figures.impactors().crust() == first.crust()
    }

    let t = UniverseTime::EPOCH;
    let mut tallies: BTreeMap<&str, Tally> = BTreeMap::new();
    let (mut icy, mut crusts, mut climates_unread) = (0_u32, 0_u32, 0_u32);
    let mut differing_figures: Vec<String> = Vec::new();
    // The lid (C3): bodies with a lid, their lid flips settled and bracketed, the airless bodies
    // and the radiative ones checked, those `for_body` reads (airless and under a thin air, each
    // R09's Γ its record's, asserted) and those it answers not modelled (counted), and the deferred
    // warmest checked.
    let (mut lids, mut airless, mut radiative, mut deferred) = (0_u32, 0_u32, 0_u32, 0_u32);
    let (mut settled_flips, mut bracketed_flips) = (Vec::<String>::new(), Vec::<String>::new());
    let (mut airless_read, mut thin_read, mut radiative_unread) = (0_u32, 0_u32, 0_u32);
    // The per-pass trace's lists (§1.8): the jumps across the diagonal by class, the context
    // jumps by member, the settled bodies whose context changed discretely during their passes by
    // member (listed, not counted); and the cover rule with the water follow-up check.
    let mut jumps_by: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut context_jumps: Vec<String> = Vec::new();
    let mut settled_members: BTreeMap<&'static str, u32> = BTreeMap::new();
    let mut settled_changes: Vec<String> = Vec::new();
    let mut cover = CoverRuleTally::default();
    // Tallies a derivation of the kind `kind`, re-run once at its published albedo and context as
    // `again` re-runs it, its record's surface `surface`, the record of `id`, whose inputs
    // `for_body` reads as `read_inputs` reads them.
    let mut tally =
        |kind: &'static str,
         derived: &DerivedBody,
         what: String,
         (id, surface): (crate::id::BodyId, Option<&Surface>),
         again: &dyn Fn(PassDriver) -> (DerivedBody, Option<SpunRead>),
         read_inputs: &dyn Fn() -> Result<CoarseInputs, ReadCoarseInputsError>| {
            let tally = tallies.entry(kind).or_default();
            tally.bodies += 1;
            let passes = derived.atmosphere_passes();
            *tally.passes.entry(passes).or_default() += 1;
            assert!(passes <= MOST_PASSES, "{what}: {passes}");
            assert_same_bits(
                derived.equilibrium_temperature().value(),
                equilibrium_temperature(derived.flux(), derived.albedo()).value(),
            );
            let spun = derived
                .spun()
                .unwrap_or_else(|| panic!("{what}: derived over the stand-in"));
            // Rule (f): one pass at the published albedo under the published context is the
            // derivation, and its publication the record's.
            let (once, read) = again(PassDriver::Again(Under::Read));
            assert_eq!(
                format!("{:?}", once.atmosphere()),
                format!("{:?}", derived.atmosphere()),
                "{what}"
            );
            assert_eq!(once.spun().map(|s| *s.next()), Some(*spun.next()), "{what}");
            // The per-pass trace (§1.8): the solve once more with each pass recorded, its outcome
            // the solve's to the bit, one traced pass for each pass counted.
            let ((traced_body, _), trace) = traced(|| again(PassDriver::Traced));
            assert_eq!(format!("{traced_body:?}"), format!("{derived:?}"), "{what}");
            assert_eq!(trace.len(), usize::try_from(passes).unwrap(), "{what}");
            // E2 (decision-pre-23-review item 30): no body's area mix at its clamp.
            assert!(
                derived.albedo().value() < LARGEST_BOND_ALBEDO
                    && derived.atmosphere().albedo().value() < LARGEST_BOND_ALBEDO,
                "{what}: an area mix at its clamp"
            );
            if let (Some(surface), Some(read)) = (surface, &read) {
                // The cover rule and the water follow-up check over the published pass's
                // partition, the record's by rule (f).
                check_cover_rule(id, surface, Some(&read.partition), &mut cover);
                crusts += u32::from(same_publication(&what, surface, read));
                if read.conditions.is_none() {
                    climates_unread += 1;
                    // R2-8: no condensed water fills a sea, so θ is 0's to the bit.
                    assert_same_bits(
                        spun.ocean_weight().get(),
                        OceanWeight::of_potential_ocean(0.0).get(),
                    );
                }
                // D3-2: the warmest the published pass's lid built of its record's conditions, where
                // it read none of its own, is its record's.
                if let Some(warmest) = read.deferred.last().copied().flatten() {
                    assert_same_bits(
                        warmest.value(),
                        surface.conditions().climate_range().1.value(),
                    );
                    deferred += 1;
                }
                if !figures_agree(surface.figures(), spun.context()) {
                    differing_figures.push(format!("{what}: {:?}", derived.convergence()));
                }
            }
            // Test 11: the published lid is the one its pass took again, whose row its regolith
            // read, and its record's crust's lid row, to the bit.
            let lid = derived.crust_lid().copied();
            assert_eq!(once.crust_lid().copied(), lid, "{what}");
            if let Some(read) = &read {
                // The trace's last pass is the re-run's, after every round of the solve (D3-3).
                assert_eq!(read.lids.last().copied().flatten(), lid, "{what}");
                let changes = lid_changes(&read.lids);
                if changes.len() > 1 {
                    let listed = format!("{what}: {changes:?}, {:?}", derived.convergence());
                    match derived.convergence() {
                        Convergence::Converged => settled_flips.push(listed),
                        Convergence::Bracketed { .. } => bracketed_flips.push(listed),
                    }
                }
            }
            if let Some(surface) = surface {
                let record_lid = surface.crust().ok().map(|crust| {
                    match crust.lid() {
                        LidCrust::Primary => crust.primary(),
                        LidCrust::Secondary => crust.secondary(),
                        LidCrust::Bulk => crust.bulk(),
                    }
                    .expect("a crust's lid names a crust it has")
                });
                assert_eq!(record_lid, lid.map(|lid| lid.row()), "{what}");
                lids += u32::from(lid.is_some());
                // The six points' item 5: an airless body's record's mean T_s is its T_eq.
                let conditions = surface.conditions();
                let mean = conditions.mean_surface_temperature();
                let is_airless = surface.state() == SurfaceState::Airless;
                if is_airless {
                    assert_same_bits(mean.value(), derived.equilibrium_temperature().value());
                    airless += 1;
                }
                // D3-1 and Batch E's review, I1: R09's radiative model, airless or under a thin
                // air, reads the Γ of its inputs from `for_body`, the record's (D3's one Γ), on
                // every body `for_body` reads; one it answers not modelled reads no inputs.
                let model = conditions.climate_regime().map(|regime| regime.model());
                if model == Some(ClimateModelKind::RadiativeEquilibrium) {
                    radiative += 1;
                    match read_inputs() {
                        Ok(inputs) => {
                            let r09 = regolith_inertia_of(&inputs);
                            let record = conditions.ground().inertia_j_per_m2_k_sqrt_s();
                            assert!(
                                r09.total_cmp(&record).is_eq(),
                                "{what}: {:?}, R09's Γ {r09} against the record's {record}, {:?}",
                                surface.state(),
                                derived.convergence(),
                            );
                            if is_airless {
                                airless_read += 1;
                            } else {
                                thin_read += 1;
                            }
                        }
                        Err(error) => {
                            assert_eq!(
                                error,
                                ReadCoarseInputsError::Section(SurfaceInputsError::NotModelled(
                                    RecordSection::Surface
                                )),
                                "{what}"
                            );
                            radiative_unread += 1;
                        }
                    }
                }
            }
            match derived.convergence() {
                Convergence::Converged => {
                    assert!(spun.next().agrees(spun.context()), "{what}: {spun:?}");
                    if let Some(surface) = surface {
                        assert!(
                            figures_agree(surface.figures(), spun.context()),
                            "{what}: the record's figures against {spun:?}"
                        );
                    }
                    let (members, moves) = settled_moves(&trace);
                    if !moves.is_empty() {
                        for member in members {
                            *settled_members.entry(member).or_default() += 1;
                        }
                        settled_changes.push(format!("{what}: {}", moves.join("; ")));
                    }
                }
                Convergence::Bracketed {
                    residual,
                    fit: context,
                } => {
                    assert!(
                        residual.is_finite() && residual >= 0.0,
                        "{what}: {residual}"
                    );
                    // The joint residual: one pass at A* under K(P*).
                    let (joint, _) = again(PassDriver::Again(Under::Next));
                    let joint =
                        (joint.atmosphere().albedo().value() - derived.albedo().value()).abs();
                    tally.second_round += u32::from(context != ContextFit::FirstRound);
                    let class = if residual > JUMP_RESIDUAL {
                        let (by, ends) = jump_class(&trace, derived.albedo(), spun.context());
                        jumps_by
                            .entry(by)
                            .or_default()
                            .push(format!("{what}: {ends}"));
                        Class::Jump
                    } else if context == ContextFit::Jump {
                        let members = key_members(&spun.context().key(), &spun.next().key());
                        context_jumps.push(format!("{what}: {members:?}"));
                        Class::ContextJump
                    } else if derived.albedo().value() > 1.0 - 1e-6 {
                        Class::Frost
                    } else {
                        assert!(
                            residual < 1e-9,
                            "{what}: a fixed point's residual {residual:e}"
                        );
                        assert!(
                            joint <= 1e-3,
                            "{what}: a fixed point's joint residual {joint:e}"
                        );
                        tally.largest_joint = tally.largest_joint.max(joint);
                        Class::FixedPoint
                    };
                    // Derived twice, the same (the sim-determinism skill).
                    let (twice, _) = again(PassDriver::Solve);
                    assert_eq!(format!("{twice:?}"), format!("{derived:?}"), "{what}");
                    tally.bracketed.entry(class).or_default().push(format!(
                        "{what}, {passes} passes, residual {residual:.3e}, joint {joint:.3e}, \
                     {context:?}, key {:?} → {:?}",
                        spun.context().key(),
                        spun.next().key()
                    ));
                }
            }
        };
    for (ctx, system) in whole().iter().take(60) {
        if ctx.age_at_epoch().value() <= 0.0 {
            continue;
        }
        let epoch = Epoch::new(system, ctx, t).with_shades();
        let cosines = HourCosines::new();
        for body in system.bodies() {
            let kind = match body.part {
                Part::Planet(_) => "planets",
                Part::Member(_) => "members",
                Part::Moon(_) | Part::Ring(_) => continue,
            };
            let host = fate_host(ctx, system.zone_of(body));
            let fate = system.fate_of(body, &host);
            let label = label::label(system, body.index).unwrap();
            let (record, now) = system.primary_record(&epoch, &cosines, body, label, &fate);
            let (Section::Ok(photometry), Some(derived)) = (record.photometry(), now.derived)
            else {
                continue;
            };
            assert_same_bits(photometry.bond_albedo().value(), derived.albedo().value());
            if let (Some(ice), Section::Ok(bulk)) = (icy_member_ice(body), record.bulk()) {
                assert!((bulk.fractions().water() - ice).abs() < 1e-15);
                icy += 1;
            }
            let at = fate.at(t);
            let again = |driver| {
                let ((derived, _, _), read) = spun_read::capture(|| {
                    system.primary_derived(&epoch, &cosines, (body, &fate), &at, driver)
                });
                (derived, read)
            };
            let surface = match record.surface() {
                Section::Ok(surface) => Some(surface),
                _ => None,
            };
            tally(
                kind,
                &derived,
                format!(
                    "{:?} {:?}: {:?} {:?}, read {:.4}, gives {:.4}, T_eq {:.2} K, T_s {:.2} K, water {:.3}",
                    ctx.id(),
                    body.index,
                    derived.class(),
                    derived.atmosphere().state(),
                    derived.albedo().value(),
                    derived.atmosphere().albedo().value(),
                    derived.equilibrium_temperature().value(),
                    derived.atmosphere().surface_temperature().value(),
                    derived.fractions().water(),
                ),
                (record.identity().id(), surface),
                &again,
                &|| CoarseInputs::for_body(&record, None, ctx),
            );
            let parent = &record;
            for child in system.children(body.index) {
                if !matches!(child.part, Part::Moon(_)) {
                    continue;
                }
                let label = label::label(system, child.index).unwrap();
                let record = system.satellite_record(&epoch, &cosines, child, label, &now, &fate);
                let Section::Ok(photometry) = record.photometry() else {
                    continue;
                };
                let derived = system.record_moon(&epoch, child, (&now, &fate)).unwrap();
                assert_same_bits(photometry.bond_albedo().value(), derived.albedo().value());
                // A moon's context holds its tide at its pass (decision-p14-t54a-passes §1.1).
                assert!(
                    derived
                        .spun()
                        .is_some_and(|spun| spun.next().tide().is_some()),
                    "{:?}: {:?}",
                    child.index,
                    derived.spun()
                );
                let again = |driver| {
                    let (derived, read) = spun_read::capture(|| {
                        system.record_moon_driven(&epoch, child, (&now, &fate), driver)
                    });
                    (*derived.unwrap().body(), read)
                };
                let surface = match record.surface() {
                    Section::Ok(surface) => Some(surface),
                    _ => None,
                };
                tally(
                    "moons",
                    derived.body(),
                    format!(
                        "{:?} {:?}: moon {:?}, read {:.4}, gives {:.4}, T_eq {:.2} K, T_s {:.2} K",
                        ctx.id(),
                        child.index,
                        derived.atmosphere().state(),
                        derived.albedo().value(),
                        derived.atmosphere().albedo().value(),
                        derived.equilibrium_temperature().value(),
                        derived.atmosphere().surface_temperature().value(),
                    ),
                    (record.identity().id(), surface),
                    &again,
                    &|| CoarseInputs::for_body(&record, Some(parent), ctx),
                );
            }
        }
    }
    let count = |tally: &Tally, class: Class| tally.bracketed.get(&class).map_or(0, Vec::len);
    let (mut total, mut held, mut jumps) = (0_usize, 0_usize, 0_usize);
    for (kind, tally) in &tallies {
        let bracketed = tally.bracketed.values().map(Vec::len).sum::<usize>();
        let jumping = count(tally, Class::Jump) + count(tally, Class::ContextJump);
        let listed = tally
            .bracketed
            .iter()
            .map(|(class, list)| {
                format!("  {} {class:?}:\n    {}", list.len(), list.join("\n    "))
            })
            .collect::<Vec<_>>()
            .join("\n");
        eprintln!(
            "{kind}: {} bodies; passes {:?}; bracketed {bracketed} (diagonal jumps {}, context \
             jumps {}, frosts {}, fixed points {}, the largest joint residual {:.3e}); {} took \
             the second round\n{listed}",
            tally.bodies,
            tally.passes,
            count(tally, Class::Jump),
            count(tally, Class::ContextJump),
            count(tally, Class::Frost),
            count(tally, Class::FixedPoint),
            tally.largest_joint,
            tally.second_round,
        );
        total += usize::try_from(tally.bodies).unwrap();
        held += bracketed;
        jumps += jumping;
    }
    eprintln!(
        "{total} bodies ({icy} icy members): bracketed {held}, jumping {jumps}; {crusts} ice bulks' \
         water entries differ from their pass's by their crust; {climates_unread} columns with \
         nothing that condenses read no climate; {} bodies' figures differ from the context their \
         published pass read:\n  {}",
        differing_figures.len(),
        differing_figures.join("\n  ")
    );
    eprintln!(
        "lids: {lids} bodies with a lid on their record; lid flips during the passes, settled {} \
         (listed, not counted):\n  {}\nbracketed {}:\n  {}\n{airless} airless bodies, their mean \
         T_s their T_eq to the bit; {radiative} under R09's radiative model: {airless_read} \
         airless and {thin_read} under a thin air read by `for_body`, R09's Γ their record's, \
         and {radiative_unread} it answers not modelled; {deferred} published passes' lids read \
         their record's warmest, deferred",
        settled_flips.len(),
        settled_flips.join("\n  "),
        bracketed_flips.len(),
        bracketed_flips.join("\n  "),
    );
    eprintln!(
        "the jumps across the diagonal by class (state edge, lid flip, onset, cover flip):\n  {}\n\
         context jumps by member {}:\n  {}\nsettled bodies whose context changed discretely \
         during their passes (listed, not counted; items 17 and 22) {}, by member \
         {settled_members:?}:\n  {}",
        jumps_by
            .iter()
            .map(|(class, list)| format!("{} {class}:\n    {}", list.len(), list.join("\n    ")))
            .collect::<Vec<_>>()
            .join("\n  "),
        context_jumps.len(),
        context_jumps.join("\n  "),
        settled_changes.len(),
        settled_changes.join("\n  "),
    );
    eprintln!(
        "the cover rule over the published passes' partitions: W3 (a) {}, W3 (b) {}, scaled {}; \
         {} liquid entries, each liquid where it lies; phases condensate_of's fallback set {}:\n  \
         {}",
        cover.crust_ice,
        cover.under_ice,
        cover.scaled,
        cover.liquids.liquids,
        cover.liquids.fallbacks.len(),
        cover.liquids.fallbacks.join("\n  "),
    );
    // The water follow-up check (decision-p14-t54a-water-consistency, "Batch B follow-up" §F.1):
    // no liquid entry with an area would boil at its surface pressure.
    assert!(
        cover.liquids.boiling.is_empty(),
        "{:?}",
        cover.liquids.boiling
    );
    // E2: T51.b's anchors are active, the area mix weighting more than three bands
    // (`an_enceladus_takes_its_fresh_frost_near_0_8`,
    // `a_pluto_and_a_triton_given_their_ices_take_their_bond_albedos`).
    const { assert!(OPTICS_BANDS > 3, "the anchors read more than three bands") };
    // PD-1 and I1: R09's radiative model read its record's Γ on every body `for_body` reads,
    // bracketed ones too, airless and under a thin air (asserted body by body above).
    assert!(
        airless_read > 0
            && thin_read > 0
            && airless_read + thin_read + radiative_unread == radiative,
        "{airless_read} airless, {thin_read} thin, {radiative_unread} not modelled of {radiative}"
    );
    // The ruling's measures, over all bodies (decision-p14-t54a-passes §1.8 and item 5): at most
    // 1% jumping, a diagonal or a context jump, at most 3% bracketed. Above either goes to main.
    assert!(100 * jumps <= total, "{jumps} of {total} jump");
    assert!(100 * held <= 3 * total, "{held} of {total} bracketed");
    let planets = &tallies["planets"];
    let planet_jumps = count(planets, Class::Jump) + count(planets, Class::ContextJump);
    // The planets' 2% guard is over the 400 systems' planets (main, on C2's escalation:
    // `the_passes_endings_over_the_whole_sample_are_recorded`), not this sample's 226: pinned here
    // at its measured value, C2's 5 and C3's 5, all across the diagonal.
    assert!(
        planet_jumps <= 5,
        "{planet_jumps} of {} planets jump",
        planets.bodies
    );
    assert!(
        lids > 100 && airless > 100 && radiative >= airless,
        "{lids} {airless} {radiative}"
    );
    assert!(
        tallies["planets"].bodies + tallies["members"].bodies > 100
            && tallies["moons"].bodies > 10
            && icy > 0,
        "{tallies:?} {icy}"
    );
}

/// P14.T54.a's Q2 (decision-p14-t54a-passes §2.3–2.5, tests 1–3): every moon of the first 150
/// whole systems holds its law after at most two rounds at [`parent_time`].
///
/// - **Round 1** derives it under the locked spin: that spin is, to the bit, the spin
///   [`SeasonalSpin::of`] gives under every law it held there, all locked; and its record,
///   derived under that law, is round 1's derivation to the bit (its tides, its locking time,
///   everything), whose tides give that law to the bit (`BodyRotation`'s `PartialEq`).
/// - **Round 2** takes every moon round 1 left unlocked: each is listed with its state and τ
///   under both rounds, its record's tides' τ and law, and whether it oscillates (unlocked after
///   round 1 and locked after round 2, or a round-2 law whose record's tides give the other
///   state). Its record is round 2's derivation to the bit where round 2 gave round 1's law to the
///   bit, as every one does (test 1 for round 2; decision-p14-t54a-passes, main's G2).
/// - **The guards**, as main restated them on C2's escalation (G2): the moons whose state differs
///   between the rounds, with the oscillations, at most 1% of the moons; the regular moons in round
///   2 at most 1% of the regular moons; and the round-2 count pinned at its measured 151.
#[test]
fn a_moon_s_law_is_held_after_at_most_two_rounds() {
    moon_laws_are_held_after_at_most_two_rounds(&whole()[..150]);
}

/// P14.T24.h.a's guard G2 under the Rayleigh floor (decision-p14-t54a-passes §G3.5, test 4's
/// "flips ≤ 1%"), a measurement run by name: [`a_moon_s_law_is_held_after_at_most_two_rounds`]'s
/// walk and guards over the same 150 whole systems, generated afresh under the floor.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn a_moon_s_law_is_held_after_at_most_two_rounds_under_the_rayleigh_floor() {
    use crate::planetary::derive::photometry::area_mix::{RayleighFloorRules, with_rayleigh_floor};
    with_rayleigh_floor(RayleighFloorRules::Floored, || {
        let systems: Vec<(SystemContext, PlanetarySystem)> = sample()[..150]
            .iter()
            .map(|ctx| (ctx.clone(), generate(SEED, ctx)))
            .collect();
        moon_laws_are_held_after_at_most_two_rounds(&systems);
    });
}

/// [`a_moon_s_law_is_held_after_at_most_two_rounds`]'s walk and guards over `systems`.
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the sample's moons, each round checked and listed"
)]
fn moon_laws_are_held_after_at_most_two_rounds(systems: &[(SystemContext, PlanetarySystem)]) {
    use std::collections::BTreeMap;

    let spin_bits = |spin: &SeasonalSpin| {
        format!(
            "{:?} {:?} {:?}",
            spin.obliquity.value(),
            spin.rotation_period.value(),
            spin.solar_day.map(Seconds::value)
        )
    };
    let (mut moons, mut first, mut regular, mut repeated) = (0_usize, 0_usize, 0_usize, 0_usize);
    let mut states: BTreeMap<String, usize> = BTreeMap::new();
    let mut second: Vec<String> = Vec::new();
    let (mut regular_second, mut flips_or_oscillations) = (0_usize, 0_usize);
    for (ctx, system) in systems {
        let t = parent_time(ctx);
        let epoch = Epoch::new(system, ctx, t).with_shades();
        let cosines = HourCosines::new();
        for body in system.bodies() {
            let (Part::Moon(moon), OrbitHost::Body(parent)) = (&body.part, body.host) else {
                continue;
            };
            let index = body.index();
            let primary = system.body(parent).unwrap();
            let host = fate_host(ctx, system.zone_of(primary));
            let fate = system.fate_of(primary, &host);
            let now = system.parent_now(&epoch, &cosines, parent, &fate);
            let held = system.moon_law(&epoch, &cosines, body, moon, (&now, &fate));
            // The law held is the generator's, `moon_rotations`'.
            assert_eq!(
                system.rotation_of(ctx, index).unwrap(),
                held.as_ref().map(|held| held.law),
                "{index:?}"
            );
            let Some(held) = held else {
                continue;
            };
            moons += 1;
            let is_regular = body.kind() == BodyKind::Moon(MoonOrigin::Regular);
            regular += usize::from(is_regular);
            let seasonal = *fate.at(t).orbit().unwrap();
            let state = |law: &BodyRotation| law.frame().state_at(t);
            let record = system.record_moon(&epoch, body, (&now, &fate)).unwrap();
            let orbit = moon.satellite.orbit_at(&moon.parent, ctx.age_at(t));
            let bulk = SpinBulk::from(&PlanetarySystem::moon_bulk_of(moon, &record, &now));
            let record_law = system
                .rotation_from(ctx, body, &orbit, bulk, body.mass(), record.spin_tides())
                .unwrap();
            *states
                .entry(format!(
                    "{:?} round {} {:?}",
                    body.kind(),
                    if held.round == MoonRound::First { 1 } else { 2 },
                    state(&held.law)
                ))
                .or_default() += 1;
            match held.round {
                MoonRound::First => {
                    first += 1;
                    // Test 2: the locked spin is the held law's own, to the bit.
                    let spin =
                        SeasonalSpin::of(held.law.frame(), t, &seasonal, SeasonsFrom::Planet);
                    assert_eq!(spin_bits(&spin), spin_bits(&held.locked), "{index:?}");
                    // Test 1: its record is round 1's derivation, and its law the held one.
                    let case = MoonNow {
                        body,
                        moon,
                        orbit,
                        parent: &now,
                        fate: &fate,
                    };
                    let round_one = system
                        .moon_derived_with(
                            &epoch,
                            &cosines,
                            &case,
                            Some(held.locked),
                            PassDriver::Solve,
                        )
                        .unwrap();
                    assert_eq!(record, round_one, "{index:?}");
                    assert_eq!(record_law, held.law, "{index:?}");
                }
                MoonRound::Second { first: law } => {
                    regular_second += usize::from(is_regular);
                    repeated += usize::from(*law == held.law);
                    // Test 1 for round 2 (G2): where round 2 gave round 1's law to the bit, its
                    // record, derived under the held law, is round 2's derivation, under round 1's.
                    if *law == held.law {
                        let case = MoonNow {
                            body,
                            moon,
                            orbit,
                            parent: &now,
                            fate: &fate,
                        };
                        let round_two = system
                            .moon_derived_with(
                                &epoch,
                                &cosines,
                                &case,
                                Some(SeasonalSpin::of(
                                    law.frame(),
                                    t,
                                    &seasonal,
                                    SeasonsFrom::Planet,
                                )),
                                PassDriver::Solve,
                            )
                            .unwrap();
                        assert_eq!(record, round_two, "{index:?}");
                    }
                    let oscillates = matches!(state(&held.law), SpinState::Locked(_))
                        || state(&record_law) != state(&held.law);
                    // A flip: round 2's law in another state than round 1's.
                    let flips = state(&law) != state(&held.law);
                    flips_or_oscillations += usize::from(flips || oscillates);
                    second.push(format!(
                        "{:?} {index:?}: round 1 {:?} τ {:.4e} s, round 2 {:?} τ {:.4e} s; the \
                         record's tides' law {:?} τ {:.4e} s ({:+.3e} of the held τ), its \
                         derivation's τ {:.4e} s at 15 h{}",
                        body.kind(),
                        state(&law),
                        law.locking_time().value(),
                        state(&held.law),
                        held.law.locking_time().value(),
                        state(&record_law),
                        record_law.locking_time().value(),
                        record_law.locking_time().value() / held.law.locking_time().value() - 1.0,
                        record.locking_time().value(),
                        if oscillates { ", an oscillation" } else { "" },
                    ));
                }
            }
        }
    }
    eprintln!(
        "{moons} moons ({regular} regular): {first} held after round 1, {} after round 2 \
         ({regular_second} regular; {repeated} whose round 2 gave round 1's law to the bit; \
         {flips_or_oscillations} flipping or oscillating); by origin, round and state {states:?}\n  \
         {}",
        second.len(),
        second.join("\n  ")
    );
    assert!(moons > 300 && regular > 50, "{moons} {regular}");
    // The ruling's guard (§2.5, test 3), at most 1% of moons in round 2, tripped at 23's rules:
    // 151 of 426 moons (35%), every one a captured or giant-impact moon unlocked in both rounds,
    // with no oscillation, whose round 2 gives round 1's law to the bit. Main accepted round 2 as
    // built and re-based the guard (on C2's escalation): the flips and oscillations at most 1% of
    // the moons, and the regular moons taking round 2 at most 1% of the regular moons. The 151 is
    // pinned at its measured value.
    assert!(
        100 * regular_second <= regular,
        "{regular_second} of {regular} regular moons"
    );
    assert!(
        100 * flips_or_oscillations <= moons,
        "{flips_or_oscillations} of {moons} moons flip or oscillate"
    );
    assert!(
        second.len() <= 151 && repeated == second.len(),
        "{} of {moons} took round 2, {repeated} repeating round 1's law",
        second.len()
    );
}

/// Decision-p14-t54a-ganymede §C3.1 (Batch D's fix commit, RG1): over the first 60 whole systems
/// at the epoch, the record lists an ice bulk's crust water as its `H2O` solid only where its lid is
/// `H2O` (or not known). Against C3's rule, its crust water listed by the material alone
/// (`with_crust_water_by_material`), every ice bulk under another lid that listed the entry lists
/// none (counted, by lid: the ruling's 31 that listed it over some of their surface, and 3 more at
/// no area), and nothing else of any record moves: its photometry,
/// bulk, state, conditions (its climate regime among them) and crust to the bit, its other
/// condensates in their order, and its figures to the bit but for the ice cover, which takes
/// T24.b's water ice at the cover's ice area where the entry was, appended after the condensates'
/// ices (the figures' rule where the condensates hold no `H2O` solid): the same items, a list the
/// surface scales moving in its last bits by its total's new order (listed, with its sum).
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the sample's records against C3's rule, each section compared"
)]
fn an_ice_bulk_under_a_dark_lid_lists_no_crust_water() {
    use std::collections::BTreeMap;

    use crate::planetary::chem::partition::CondensatePhase;
    use hyperion_testkit::float;

    use crate::planetary::hooks::crust::{CrustLithology, LidCrust};

    let water = Gas::Water.substance();
    let rendered = |entries: &[crate::planetary::chem::partition::CondensateEntry]| {
        entries.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>()
    };
    let (mut surfaces, mut dark, mut delisted, mut truncated) = (0_u32, 0_u32, 0_u32, 0_u32);
    let mut at_no_area: Vec<String> = Vec::new();
    let (mut ulp_covers, mut ulp_sums) = (0_u32, 0_u32);
    let mut by_lid: BTreeMap<String, u32> = BTreeMap::new();
    let mut covers: Vec<String> = Vec::new();
    for (ctx, system) in whole().iter().take(60) {
        let now = system.snapshot_at(ctx, UniverseTime::EPOCH);
        let c3 = with_crust_water_by_material(|| system.snapshot_at(ctx, UniverseTime::EPOCH));
        assert_eq!(now.bodies().len(), c3.bodies().len());
        for (record, before) in now.bodies().iter().zip(c3.bodies()) {
            let what = format!("{:?} {:?}", ctx.id(), record.index());
            assert_eq!(
                format!("{:?}", record.photometry()),
                format!("{:?}", before.photometry()),
                "{what}"
            );
            assert_eq!(
                format!("{:?}", record.bulk()),
                format!("{:?}", before.bulk()),
                "{what}"
            );
            let (Section::Ok(surface), Section::Ok(old)) = (record.surface(), before.surface())
            else {
                assert_eq!(
                    format!("{:?}", record.surface()),
                    format!("{:?}", before.surface()),
                    "{what}"
                );
                continue;
            };
            surfaces += 1;
            assert_eq!(surface.state(), old.state(), "{what}");
            assert_eq!(surface.material(), old.material(), "{what}");
            assert_eq!(
                format!("{:?}", surface.conditions()),
                format!("{:?}", old.conditions()),
                "{what}: the conditions, the climate regime among them"
            );
            assert_eq!(
                format!("{:?}", surface.crust()),
                format!("{:?}", old.crust()),
                "{what}"
            );
            let lid = surface.crust().ok().map(|crust| {
                match crust.lid() {
                    LidCrust::Primary => crust.primary(),
                    LidCrust::Secondary => crust.secondary(),
                    LidCrust::Bulk => crust.bulk(),
                }
                .expect("a crust's lid names a crust it has")
            });
            let is_dark = surface.material() == SurfaceMaterial::Ice
                && lid.is_some_and(|row| row != CrustLithology::Water.substance());
            dark += u32::from(is_dark);
            let listed = old
                .condensates()
                .entry(water, CondensatePhase::Solid)
                .is_some();
            let (entries, old_entries) = (
                rendered(surface.condensates().entries()),
                rendered(old.condensates().entries()),
            );
            if !(is_dark && listed) {
                assert_eq!(entries, old_entries, "{what}");
                assert_eq!(
                    format!("{:?}", surface.figures()),
                    format!("{:?}", old.figures()),
                    "{what}"
                );
                continue;
            }
            // De-listed: the entry is gone, the others as they were, and an entry the list had
            // truncated may follow them.
            assert!(
                surface
                    .condensates()
                    .entry(water, CondensatePhase::Solid)
                    .is_none(),
                "{what}"
            );
            let kept: Vec<String> = old
                .condensates()
                .entries()
                .iter()
                .filter(|e| !(e.substance() == water && e.phase() == CondensatePhase::Solid))
                .map(|e| format!("{e:?}"))
                .collect();
            assert_eq!(entries[..kept.len()], kept[..], "{what}");
            assert!(entries.len() <= kept.len() + 1, "{what}");
            truncated += u32::from(entries.len() > kept.len());
            delisted += 1;
            if old
                .condensates()
                .entry(water, CondensatePhase::Solid)
                .is_some_and(|e| e.area_fraction() <= 0.0)
            {
                at_no_area.push(what.clone());
            }
            let key = lid.map_or_else(
                || "none".to_owned(),
                |row| row.substance().key().as_str().to_owned(),
            );
            *by_lid.entry(key.clone()).or_default() += 1;
            // The cover: the liquids as they were; the ices the same items, T24.b's water ice at
            // the entry's area appended after the condensates' own.
            let (figures, old_figures) = (surface.figures(), old.figures());
            assert_eq!(
                format!("{:?}", figures.surface_liquids()),
                format!("{:?}", old_figures.surface_liquids()),
                "{what}"
            );
            // The same substances, each once, each area within a few ulps of its own: a list the
            // surface scales sums its total in the new order (`within_the_surface`).
            let (ices, old_ices) = (figures.surface_ices(), old_figures.surface_ices());
            assert_eq!(ices.len(), old_ices.len(), "{what}");
            let mut moved_bits = false;
            for &(id, area) in ices {
                let &(_, was) = old_ices
                    .iter()
                    .find(|(other, _)| *other == id)
                    .unwrap_or_else(|| panic!("{what}: {id:?} not in {old_ices:?}"));
                assert!(
                    (area - was).abs() <= 4.0 * f64::EPSILON * area.abs().max(was.abs()),
                    "{what}: {id:?} {area:e} against {was:e}"
                );
                moved_bits |= float::bits(area) != float::bits(was);
            }
            assert_eq!(ices.last().map(|(id, _)| *id), Some(water), "{what}");
            let sum_moved =
                float::bits(figures.ice_fraction()) != float::bits(old_figures.ice_fraction());
            ulp_covers += u32::from(moved_bits);
            ulp_sums += u32::from(sum_moved);
            covers.push(format!(
                "{what}: lid {key}, ices {ices:?} (was {old_ices:?}), sum {:e} (was {:e}){}",
                figures.ice_fraction(),
                old_figures.ice_fraction(),
                if moved_bits {
                    ", areas moved in their last bits"
                } else {
                    ""
                },
            ));
        }
    }
    eprintln!(
        "{surfaces} surfaces, {dark} ice bulks under a non-H2O lid; {delisted} de-listed their H2O \
         solid ({truncated} with a truncated entry back, {} at no area: {at_no_area:?}), by lid \
         {by_lid:?}; {ulp_covers} ice covers moved in their last bits, {ulp_sums} sums:\n  {}",
        at_no_area.len(),
        covers.join("\n  ")
    );
    // The ruling's count (§C3.1) is the 31 that listed the entry over some of their surface. The
    // same rule de-lists 3 more, whose entry lay at no area (61ffd9680000000c's 0201 and 0202 and
    // 42002cb1ffc00004's 0401, under `chondrite_h`, `chondrite_ll` and `basalt_fe` lids), which
    // C3's count of entries over the surface left out.
    assert_eq!(
        (
            delisted - u32::try_from(at_no_area.len()).expect("a few bodies"),
            at_no_area.len()
        ),
        (31, 3),
        "{by_lid:?}"
    );
}

/// P14.T54.a's six points, item 2 (decision-p14-t54a-passes §3.2): no record section reads a
/// stand-in derivation. Over the first 60 whole systems at the epoch and at [`parent_time`], every
/// present planet's, member's and moon's record derives it over its record, under its law, where
/// it holds one: its derivation carries the context its published pass read. One with no law, a
/// body not present at its law's time, keeps the stand-in, and its surface is not modelled
/// (listed); every body of a `generate_planets` system does.
#[test]
fn no_record_section_reads_a_stand_in_derivation() {
    let (mut real, mut stand_in) = (0_u32, Vec::<String>::new());
    for (ctx, system) in whole().iter().take(60) {
        for t in [UniverseTime::EPOCH, parent_time(ctx)] {
            let epoch = Epoch::new(system, ctx, t).with_shades();
            let cosines = HourCosines::new();
            for body in system.bodies() {
                if !matches!(body.part, Part::Planet(_) | Part::Member(_)) {
                    continue;
                }
                let host = fate_host(ctx, system.zone_of(body));
                let fate = system.fate_of(body, &host);
                let label = label::label(system, body.index).unwrap();
                let (record, now) = system.primary_record(&epoch, &cosines, body, label, &fate);
                let mut check = |index: BodyIndex,
                                 derived: &DerivedBody,
                                 surface: &Section<Surface>,
                                 law: bool| {
                    if law {
                        assert!(derived.spun().is_some(), "{index:?} at {t:?}");
                        real += 1;
                    } else {
                        assert!(derived.spun().is_none(), "{index:?} at {t:?}");
                        assert!(
                            matches!(surface, Section::NotModelled | Section::NotApplicable),
                            "{index:?} at {t:?}: {surface:?}"
                        );
                        stand_in.push(format!("{index:?} at {t:?}"));
                    }
                };
                let Some(derived) = now.derived else {
                    continue;
                };
                check(
                    body.index,
                    &derived,
                    record.surface(),
                    body.rotation.is_some(),
                );
                for child in system.children(body.index) {
                    if !matches!(child.part, Part::Moon(_)) {
                        continue;
                    }
                    let label = label::label(system, child.index).unwrap();
                    let record =
                        system.satellite_record(&epoch, &cosines, child, label, &now, &fate);
                    // A moon not present, lost before the time, has no section a derivation fills.
                    if record.identity().state() != BodyState::Present {
                        continue;
                    }
                    let Some(derived) = system.record_moon(&epoch, child, (&now, &fate)) else {
                        continue;
                    };
                    check(
                        child.index,
                        derived.body(),
                        record.surface(),
                        child.rotation.is_some(),
                    );
                }
            }
        }
    }
    for (ctx, system) in generated().iter().take(20) {
        for record in system.snapshot_at(ctx, UniverseTime::EPOCH).bodies() {
            if record.identity().state() == BodyState::Present {
                assert!(
                    matches!(
                        record.surface(),
                        Section::NotModelled | Section::NotApplicable
                    ),
                    "{:?}",
                    record.index()
                );
            }
        }
    }
    eprintln!(
        "{real} derivations over their records; {} over the stand-in, with no law:\n  {}",
        stand_in.len(),
        stand_in.join("\n  ")
    );
    assert!(real > 1_000, "{real}");
}

/// P14.T54.a's six points, item 2, and Q1's body test 8 (decision-p14-t54a-passes §1.2, §3.2): an
/// envelope body's passes over its record are its stand-in's, T51.h's, to the bit, their count
/// included, since K gives a body with no figures C₀ at every pass. So the rings of a giant, whose
/// parent `RingParent` reads its effective or equilibrium temperature off the stand-in derivation
/// at generation (`parent_at_epoch`), read what its record's passes give. Every ringed planet
/// without an envelope is listed with the difference between the two.
#[test]
fn an_envelope_body_s_passes_over_its_record_are_its_stand_in_s() {
    let (mut envelopes, mut ringed) = (0_u32, Vec::<String>::new());
    for (ctx, system) in whole().iter().take(60) {
        let t = parent_time(ctx);
        let epoch = Epoch::new(system, ctx, t).with_shades();
        let cosines = HourCosines::new();
        for body in system.planets() {
            if body.rotation.is_none() {
                continue;
            }
            let host = fate_host(ctx, system.zone_of(body));
            let fate = system.fate_of(body, &host);
            let at = fate.at(t);
            if at.state() != BodyState::Present {
                continue;
            }
            let (real, _, _) =
                system.primary_derived(&epoch, &cosines, (body, &fate), &at, PassDriver::Solve);
            let placed = PlanetarySystem::placed_now(body, at.orbit().unwrap(), at.mass());
            let (stand_in, _) = system.derive(
                &epoch,
                body,
                system.zone_of(body),
                &placed,
                fate.host_mass(t),
                (PassRecord::StandIn, PassDriver::Solve),
            );
            let summary = |d: &DerivedBody| {
                format!(
                    "{:?} {:?} {:?} {:?} {} {:?}",
                    d.albedo().value(),
                    d.equilibrium_temperature().value(),
                    d.effective_temperature().map(Kelvin::value),
                    d.atmosphere(),
                    d.atmosphere_passes(),
                    d.convergence()
                )
            };
            if real.atmosphere().state() == SurfaceState::GasEnvelope {
                assert_eq!(summary(&real), summary(&stand_in), "{:?}", body.index);
                envelopes += 1;
            } else if !system.satellites_of(body.index).rings().is_empty() {
                let temperature = |d: &DerivedBody| {
                    d.effective_temperature()
                        .unwrap_or_else(|| d.equilibrium_temperature())
                        .value()
                };
                ringed.push(format!(
                    "{:?} {:?}: {:.3} K over its record, {:.3} K over the stand-in",
                    body.index,
                    real.atmosphere().state(),
                    temperature(&real),
                    temperature(&stand_in)
                ));
            }
        }
    }
    eprintln!(
        "{envelopes} envelope bodies the same; ringed planets with a surface:\n  {}",
        ringed.join("\n  ")
    );
    assert!(envelopes > 50, "{envelopes}");
}

/// The C1 review's DD2: the bracket's reuse of an end that read its frozen context to the bit,
/// which reads `rank`'s `Equal`, takes the same bits as evaluating it again. Over the first 20
/// whole systems at the epoch, every planet's, member's and moon's derivation with every end
/// re-evaluated (`with_ends_evaluated`) is the one with the reuse, but for its count of passes,
/// at most two more a round.
#[test]
fn reusing_a_bracket_s_ends_moves_no_bit_of_the_sample() {
    use crate::planetary::derive::passes::{Convergence, with_ends_evaluated};

    let t = UniverseTime::EPOCH;
    let (mut bodies, mut bracketed) = (0_u32, 0_u32);
    for (ctx, system) in whole().iter().take(20) {
        if ctx.age_at_epoch().value() <= 0.0 {
            continue;
        }
        let epoch = Epoch::new(system, ctx, t).with_shades();
        let cosines = HourCosines::new();
        let mut compare = |derive: &dyn Fn() -> Option<DerivedBody>| {
            let (Some(reused), Some(evaluated)) = (derive(), with_ends_evaluated(derive)) else {
                return;
            };
            let read = |d: &DerivedBody| {
                format!(
                    "{:?} {:?} {:?} {:?}",
                    d.albedo().value(),
                    d.atmosphere(),
                    d.convergence(),
                    d.spun()
                )
            };
            assert_eq!(read(&reused), read(&evaluated));
            let extra = evaluated.atmosphere_passes() - reused.atmosphere_passes();
            assert!(extra <= 4, "{extra}");
            bodies += 1;
            bracketed += u32::from(matches!(
                reused.convergence(),
                Convergence::Bracketed { .. }
            ));
        };
        for body in system.bodies() {
            match &body.part {
                Part::Planet(_) | Part::Member(_) => {
                    let host = fate_host(ctx, system.zone_of(body));
                    let fate = system.fate_of(body, &host);
                    let at = fate.at(t);
                    if at.state() != BodyState::Present {
                        continue;
                    }
                    compare(&|| {
                        Some(
                            system
                                .primary_derived(
                                    &epoch,
                                    &cosines,
                                    (body, &fate),
                                    &at,
                                    PassDriver::Solve,
                                )
                                .0,
                        )
                    });
                }
                Part::Moon(_) => {
                    let OrbitHost::Body(parent) = body.host else {
                        unreachable!("a moon's host is a body")
                    };
                    let primary = system.body(parent).unwrap();
                    let host = fate_host(ctx, system.zone_of(primary));
                    let fate = system.fate_of(primary, &host);
                    let now = system.parent_now(&epoch, &cosines, parent, &fate);
                    compare(&|| {
                        system
                            .record_moon(&epoch, body, (&now, &fate))
                            .map(|d| *d.body())
                    });
                }
                Part::Ring(_) => {}
            }
        }
    }
    eprintln!("{bodies} derivations, {bracketed} bracketed, the same with every end re-evaluated");
    assert!(bodies > 200 && bracketed > 0, "{bodies} {bracketed}");
}

/// A measurement for the bless note, run by name (decision-p14-t54a-passes, "Cost escalation
/// (after Batch B)", item 5): how the passes end for every present planet, member and moon of all
/// 400 whole systems at the epoch, by kind, the population the ruling's planets' 1.53% (24 of
/// 1,568, at Batch B's stand-ins) was measured on: bodies, bracketed, jumping (across the diagonal
/// or a context jump) and the pass histogram. It asserts the planets' guard alone, at most 2% of
/// them jumping, which main ruled is over this population (on C2's escalation).
#[test]
#[ignore = "a measurement, run by name"]
fn the_passes_endings_over_the_whole_sample_are_recorded() {
    use std::collections::BTreeMap;

    use crate::planetary::derive::passes::Convergence;

    /// A kind's bodies, bracketed and jumping, and its pass histogram.
    type Endings = (u32, u32, u32, BTreeMap<u32, u32>);

    let t = UniverseTime::EPOCH;
    let mut kinds: BTreeMap<&str, Endings> = BTreeMap::new();
    let mut tally = |kind: &'static str, derived: &DerivedBody| {
        let entry = kinds.entry(kind).or_default();
        entry.0 += 1;
        entry.1 += u32::from(matches!(
            derived.convergence(),
            Convergence::Bracketed { .. }
        ));
        entry.2 += u32::from(derived.convergence().is_jump());
        *entry.3.entry(derived.atmosphere_passes()).or_default() += 1;
    };
    for (ctx, system) in whole() {
        if ctx.age_at_epoch().value() <= 0.0 {
            continue;
        }
        let epoch = Epoch::new(system, ctx, t).with_shades();
        let cosines = HourCosines::new();
        for body in system.bodies() {
            let kind = match body.part {
                Part::Planet(_) => "planets",
                Part::Member(_) => "members",
                Part::Moon(_) | Part::Ring(_) => continue,
            };
            let host = fate_host(ctx, system.zone_of(body));
            let fate = system.fate_of(body, &host);
            let at = fate.at(t);
            let now = system.primary_now(&epoch, &cosines, body, &fate, &at);
            let Some(derived) = now.derived else {
                continue;
            };
            tally(kind, &derived);
            for child in system.children(body.index) {
                if let Some(moon) = system.record_moon(&epoch, child, (&now, &fate)) {
                    tally("moons", moon.body());
                }
            }
        }
    }
    for (kind, (bodies, bracketed, jumping, passes)) in &kinds {
        eprintln!(
            "{kind}: {bodies} bodies, {bracketed} bracketed ({:.2}%), {jumping} jumping ({:.2}%); \
             passes {passes:?}",
            100.0 * f64::from(*bracketed) / f64::from(*bodies),
            100.0 * f64::from(*jumping) / f64::from(*bodies),
        );
    }
    let (planets, _, planets_jumping, _) = &kinds["planets"];
    assert!(
        50 * planets_jumping <= *planets,
        "{planets_jumping} of {planets} planets jump"
    );
}

/// A measurement for the bless note, run by name (decision-pre-23-review items 7, 16 and 17;
/// P14.T54.a's C3): over every present planet, member and moon of the first 60 whole systems at
/// the epoch, each record derived with its lid and without it (`without_crust`, the derivation
/// before C3: its material's regolith and inertia), by lid lithology:
///
/// - the bodies whose regolith row changed from the stand-in (an airless body's by its bare
///   regolith's Bond albedo, which the space-weathering cap may leave the Moon's 0.11; any other's
///   by its row against the material's, `basalt` or `H2O`), with their equilibrium temperatures'
///   moves;
/// - the airless bodies, how many take the cap (a `Silicate` lid), and the nights the ground's
///   inertia at the lid moves (the coldest of the climate's range);
/// - the ice bulks whose lid is not `H2O`, whose record lists the crust's water as its `H2O` solid
///   (decision-p14-t54a-ganymede, C1 follow-up §C1.4) beside a regolith of another row.
///
/// It asserts only that the walk found the sample's surfaces.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the sample's bodies with and without the lid, tallied by lithology"
)]
fn the_lid_s_moves_from_the_stand_in_are_recorded() {
    use std::collections::BTreeMap;

    use crate::planetary::chem::partition::{CondensateEntry, CondensatePhase};
    use crate::planetary::derive::photometry::area_mix::{Regolith, capped_lid};
    use crate::planetary::derive::without_crust;
    use crate::planetary::hooks::crust::CrustLithology;

    /// One lithology's tally: bodies, rows changed, the moves of `T_eq` of the changed (least,
    /// greatest, sum), airless, capped, airless nights moved (least, greatest), and of them those
    /// whose `T_eq` held to the bit, which the ground's inertia alone moved.
    #[derive(Debug, Default)]
    struct Moves {
        bodies: u32,
        changed: u32,
        t_eq: Option<(f64, f64)>,
        t_eq_sum: f64,
        airless: u32,
        capped: u32,
        nights_moved: u32,
        nights: Option<(f64, f64)>,
        inertia_nights: u32,
        inertia: Option<(f64, f64)>,
    }

    let widen = |range: &mut Option<(f64, f64)>, x: f64| {
        *range = Some(range.map_or((x, x), |(lo, hi)| (lo.min(x), hi.max(x))));
    };
    let t = UniverseTime::EPOCH;
    let mut moves: BTreeMap<String, Moves> = BTreeMap::new();
    let (mut bodies, mut unchanged_lidless, mut ice_bulks_apart) = (0_u32, 0_u32, Vec::new());
    let mut record = |what: String,
                      with: (&BodyRecord, &DerivedBody),
                      without: (&BodyRecord, &DerivedBody)| {
        let (Section::Ok(surface), Section::Ok(before)) = (with.0.surface(), without.0.surface())
        else {
            return;
        };
        bodies += 1;
        let Some(lid) = with.1.crust_lid() else {
            unchanged_lidless += 1;
            return;
        };
        let lithology = lid.lithology();
        let tally = moves.entry(lithology.key().to_owned()).or_default();
        tally.bodies += 1;
        let state = surface.state();
        let material = surface.material();
        let changed = if state == SurfaceState::Airless {
            with.1
                .regolith_albedo()
                .total_cmp(&without.1.regolith_albedo())
                .is_ne()
        } else {
            lid.row() != Regolith::stand_in_row(material)
        };
        if changed {
            tally.changed += 1;
            let dt = with.1.equilibrium_temperature().value()
                - without.1.equilibrium_temperature().value();
            widen(&mut tally.t_eq, dt);
            tally.t_eq_sum += dt;
        }
        if state == SurfaceState::Airless {
            tally.airless += 1;
            tally.capped += u32::from(capped_lid(lid.row(), state));
            if before.state() == SurfaceState::Airless {
                let night = surface.conditions().climate_range().0.value();
                let night_before = before.conditions().climate_range().0.value();
                if night.total_cmp(&night_before).is_ne() {
                    tally.nights_moved += 1;
                    widen(&mut tally.nights, night - night_before);
                    let held = with
                        .1
                        .equilibrium_temperature()
                        .value()
                        .total_cmp(&without.1.equilibrium_temperature().value());
                    if held.is_eq() {
                        tally.inertia_nights += 1;
                        widen(&mut tally.inertia, night - night_before);
                    }
                }
            }
        }
        if material == SurfaceMaterial::Ice && lithology != CrustLithology::Water {
            let ice = surface
                .condensates()
                .entry(Gas::Water.substance(), CondensatePhase::Solid)
                .map_or(0.0, CondensateEntry::area_fraction);
            ice_bulks_apart.push(format!(
                "{what}: {}, {state:?}, its H2O solid entry over {ice:.3}",
                lithology.key()
            ));
        }
    };
    for (ctx, system) in whole().iter().take(60) {
        if ctx.age_at_epoch().value() <= 0.0 {
            continue;
        }
        let epoch = Epoch::new(system, ctx, t).with_shades();
        let cosines = HourCosines::new();
        for body in system.bodies() {
            if !matches!(body.part, Part::Planet(_) | Part::Member(_)) {
                continue;
            }
            let host = fate_host(ctx, system.zone_of(body));
            let fate = system.fate_of(body, &host);
            let label = label::label(system, body.index).unwrap();
            let (with, now) = system.primary_record(&epoch, &cosines, body, label.clone(), &fate);
            let (without, before) = without_crust(|| {
                system.primary_record(&epoch, &cosines, body, label.clone(), &fate)
            });
            let (Some(derived), Some(prior)) = (now.derived, before.derived) else {
                continue;
            };
            record(
                format!("{:?} {:?}", ctx.id(), body.index),
                (&with, &derived),
                (&without, &prior),
            );
            for child in system.children(body.index) {
                if !matches!(child.part, Part::Moon(_)) {
                    continue;
                }
                let label = label::label(system, child.index).unwrap();
                let with =
                    system.satellite_record(&epoch, &cosines, child, label.clone(), &now, &fate);
                let without = without_crust(|| {
                    system.satellite_record(&epoch, &cosines, child, label, &before, &fate)
                });
                let (Some(derived), Some(prior)) = (
                    system.record_moon(&epoch, child, (&now, &fate)),
                    without_crust(|| system.record_moon(&epoch, child, (&before, &fate))),
                ) else {
                    continue;
                };
                record(
                    format!("{:?} {:?} (moon)", ctx.id(), child.index),
                    (&with, derived.body()),
                    (&without, prior.body()),
                );
            }
        }
    }
    let listed = moves
        .iter()
        .map(|(key, m)| {
            let range = |range: Option<(f64, f64)>| {
                range.map_or_else(|| "none".to_owned(), |(lo, hi)| format!("{lo:+.3} to {hi:+.3} K"))
            };
            format!(
                "{key}: {} bodies, {} rows changed, ΔT_eq {} (mean {:+.3} K); {} airless, {} capped, \
                 {} nights moved {}, {} of them by the inertia alone {}",
                m.bodies,
                m.changed,
                range(m.t_eq),
                if m.changed > 0 { m.t_eq_sum / f64::from(m.changed) } else { 0.0 },
                m.airless,
                m.capped,
                m.nights_moved,
                range(m.nights),
                m.inertia_nights,
                range(m.inertia),
            )
        })
        .collect::<Vec<_>>();
    eprintln!(
        "{bodies} surfaces, {unchanged_lidless} with no lid; by lid lithology:\n  {}\n{} ice bulks \
         whose lid is not H2O:\n  {}",
        listed.join("\n  "),
        ice_bulks_apart.len(),
        ice_bulks_apart.join("\n  ")
    );
    assert!(bodies > 500, "{bodies}");
}

/// Rule (f)'s reuse (decision-p14-t54a-passes, "Cost escalation (after Batch B)" §C.3): a record's
/// figures reuse the first stage its passes' solve computed last, K(P*), which is the stage of its
/// own air's figures' inputs to the bit. Over the first 20 whole systems, the snapshots at the
/// epoch with every record's first stage computed again (`with_first_stage_recomputed`) are the
/// same, bit for bit.
#[test]
fn the_record_s_figures_reuse_its_last_pass_s_k_to_the_bit() {
    let mut surfaces = 0_usize;
    for (ctx, system) in whole().iter().take(20) {
        let reused = system.snapshot_at(ctx, UniverseTime::EPOCH);
        let recomputed =
            with_first_stage_recomputed(|| system.snapshot_at(ctx, UniverseTime::EPOCH));
        assert_eq!(
            format!("{reused:?}"),
            format!("{recomputed:?}"),
            "{:?}",
            ctx.id()
        );
        surfaces += reused
            .bodies()
            .iter()
            .filter(|record| matches!(record.surface(), Section::Ok(_)))
            .count();
    }
    assert!(surfaces > 50, "{surfaces}");
}

/// The system `system` with its records' cold traps under `cold_traps` (P14.T51.i), its other
/// rules those in force.
fn under_cold_traps(system: &PlanetarySystem, cold_traps: ColdTrapRules) -> PlanetarySystem {
    system.clone().with_record_rules(RecordRules {
        cold_traps,
        ..RecordRules::in_force()
    })
}

/// P14.T51.i, end to end: under 24's record rules (the cold traps' batch) the sample's airless,
/// uncollisional planets and members carry the regolith's albedo from their derivation, and their
/// records' ices move from the held rules' where the cold traps' branch and traps apply; under the
/// cold traps' held rules their derivations give the albedo too but the record reads none of it.
#[test]
fn under_24_s_record_rules_the_records_read_the_derivations_regolith() {
    use crate::planetary::hooks::cold_traps::{ColdTrapRules, is_collisional};

    let t = UniverseTime::EPOCH;
    let (mut airless, mut moved) = (0_u32, 0_u32);
    for (ctx, system) in whole().iter().take(12) {
        if ctx.age_at_epoch().value() <= 0.0 {
            continue;
        }
        let under = |cold_traps| under_cold_traps(system, cold_traps);
        let (held, batch) = (under(ColdTrapRules::Held), under(ColdTrapRules::Batch));
        let (held_epoch, batch_epoch) = (Epoch::new(&held, ctx, t), Epoch::new(&batch, ctx, t));
        for body in batch.bodies() {
            if !matches!(body.part, Part::Planet(_) | Part::Member(_)) {
                continue;
            }
            let host = fate_host(ctx, batch.zone_of(body));
            let fate = batch.fate_of(body, &host);
            let label = label::label(&batch, body.index).unwrap();
            let (record, now) =
                batch.primary_record(&batch_epoch, &HourCosines::new(), body, label, &fate);
            let (Some(derived), Section::Ok(bulk), Section::Ok(surface)) =
                (now.derived, record.bulk(), record.surface())
            else {
                continue;
            };
            let air = derived.atmosphere();
            if air.state() != SurfaceState::Airless || is_collisional(air, bulk.surface_gravity()) {
                continue;
            }
            let regolith = derived.regolith_albedo();
            assert!((0.0..1.0).contains(&regolith), "{regolith}");
            airless += 1;
            let label = label::label(&held, body.index).unwrap();
            let (other, _) =
                held.primary_record(&held_epoch, &HourCosines::new(), body, label, &fate);
            let Section::Ok(other) = other.surface() else {
                continue;
            };
            let ices = |s: &Surface| format!("{:?}", s.figures().surface_ices());
            moved += u32::from(ices(surface) != ices(other));
        }
    }
    eprintln!(
        "{airless} airless, uncollisional bodies; {moved} with their ices moved by the traps"
    );
    assert!(airless > 20 && moved > 5, "{airless} {moved}");
}

/// P14.T51.i's budget over the sample (decision-p14-t51-batch §1.5): the cold traps' cost per system
/// on 24's record path, measured, not counted. Each of the first 60 systems generated whole is
/// derived and its every planet's, member's and moon's record built with the cold traps' batch and
/// with their hold, the best of three
/// interleaved rounds of each; the difference is the cold traps'. The budget is a mean of at most
/// 7 ms a system and at most 50 ms in any. A timing alone, run by name in release and recorded in
/// plan 14's Risks; it asserts nothing.
#[test]
#[ignore = "a timing, run by name in release"]
fn time_the_cold_traps_over_the_sample() {
    use std::time::Instant;

    use crate::planetary::hooks::cold_traps::{ColdTrapRules, is_collisional};

    let t = UniverseTime::EPOCH;
    let mut costs: Vec<(f64, u32, String)> = Vec::new();
    let mut listed = [0_usize; 2];
    for (ctx, system) in whole().iter().take(60) {
        if ctx.age_at_epoch().value() <= 0.0 {
            continue;
        }
        let under = |cold_traps| under_cold_traps(system, cold_traps);
        let (held, batch) = (under(ColdTrapRules::Held), under(ColdTrapRules::Batch));
        // Every record of the system, how many of its bodies are airless and not collisional, and
        // how many ices its records' figures list, the cold traps' among them.
        let records = |system: &PlanetarySystem| {
            let (epoch, cosines) = (Epoch::new(system, ctx, t), HourCosines::new());
            let mut airless = 0_u32;
            let mut ices = 0_usize;
            let mut count = |record: &BodyRecord| {
                if let Section::Ok(surface) = record.surface() {
                    ices += surface
                        .figures()
                        .surface_ices()
                        .iter()
                        .filter(|&&(_, area)| area > 0.0)
                        .count();
                }
            };
            for body in system.bodies() {
                if !matches!(body.part, Part::Planet(_) | Part::Member(_)) {
                    continue;
                }
                let host = fate_host(ctx, system.zone_of(body));
                let fate = system.fate_of(body, &host);
                let label = label::label(system, body.index).unwrap();
                let (record, now) = system.primary_record(&epoch, &cosines, body, label, &fate);
                if let (Some(derived), Section::Ok(bulk)) = (now.derived, record.bulk()) {
                    let air = derived.atmosphere();
                    airless += u32::from(
                        air.state() == SurfaceState::Airless
                            && !is_collisional(air, bulk.surface_gravity()),
                    );
                }
                for child in system.children(body.index) {
                    if !matches!(child.part, Part::Moon(_)) {
                        continue;
                    }
                    let label = label::label(system, child.index).unwrap();
                    let moon = system.satellite_record(&epoch, &cosines, child, label, &now, &fate);
                    count(&moon);
                }
                count(&record);
            }
            (airless, ices)
        };
        let (mut best_held, mut best_batch) = (f64::INFINITY, f64::INFINITY);
        let (mut airless, mut ices) = (0, [0; 2]);
        for _ in 0..3 {
            for (k, system) in [&held, &batch].into_iter().enumerate() {
                let start = Instant::now();
                (airless, ices[k]) = records(system);
                let ms = start.elapsed().as_secs_f64() * 1e3;
                if k == 0 && ms < best_held {
                    best_held = ms;
                }
                if k == 1 && ms < best_batch {
                    best_batch = ms;
                }
            }
        }
        costs.push((
            best_batch - best_held,
            airless,
            format!(
                "{:?}: held {best_held:.2} ms, batch {best_batch:.2} ms, ices listed {} held and {} \
                 with the cold traps",
                ctx.id(),
                ices[0],
                ices[1]
            ),
        ));
        listed[0] += ices[0];
        listed[1] += ices[1];
    }
    let n = costs.len();
    let total: f64 = costs.iter().map(|c| c.0).sum();
    costs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let median = costs[n / 2].0;
    let planets: u32 = costs.iter().map(|c| c.1).sum();
    eprintln!(
        "the cold traps over {n} systems ({planets} airless, uncollisional planets and members): \
         mean {:.3} ms a system, median {median:.3} ms, least {:.3} ms, most {:.3} ms",
        total / f64::from(u32::try_from(n).unwrap()),
        costs[0].0,
        costs[n - 1].0,
    );
    eprintln!(
        "ices listed on the records: {} held, {} with the cold traps",
        listed[0], listed[1]
    );
    for (cost, airless, what) in costs.iter().rev().take(5) {
        eprintln!("  {cost:.3} ms, {airless} airless: {what}");
    }
}

/// The batch's surface temperatures over the sample, measured for P14.T54.a's bless note
/// (P14.T57.a's moves at 23): over the sample's first 60 systems generated whole (`whole`), every
/// present planet's, member's and moon's mean surface temperature (P14.T51.a's partition, T51.b's
/// area mix, T51.h's passes and T57.a's greenhouse constants), each body on its own line with its
/// equilibrium temperature, pressure and partial pressures of carbon dioxide and water, and the
/// bodies by state. Version 22's figures are plan 14's (P14.T57.a's record); a measurement, it
/// asserts nothing.
#[test]
#[ignore = "a measurement, run by name"]
fn the_batch_s_surface_temperatures_over_the_sample_are_recorded() {
    use std::collections::BTreeMap;

    use crate::planetary::derive::DerivedBody;

    /// What a body's line holds: its state, `T_s` and `T_eq`, K, and its surface pressure and
    /// partial pressures of carbon dioxide and water, Pa.
    type Row = (SurfaceState, f64, f64, f64, f64, f64);
    let row = |derived: &DerivedBody| -> Row {
        let air = derived.atmosphere();
        (
            air.state(),
            air.surface_temperature().value(),
            derived.equilibrium_temperature().value(),
            air.surface_pressure()
                .map_or(0.0, crate::units::Pascals::value),
            air.partial_pressures().of(Gas::CarbonDioxide).value(),
            air.partial_pressures().of(Gas::Water).value(),
        )
    };
    let t = UniverseTime::EPOCH;
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    for (ctx, system) in whole().iter().take(60) {
        if ctx.age_at_epoch().value() <= 0.0 {
            continue;
        }
        let epoch = Epoch::new(system, ctx, t);
        for body in system.bodies() {
            if !matches!(body.part, Part::Planet(_) | Part::Member(_)) {
                continue;
            }
            let host = fate_host(ctx, system.zone_of(body));
            let fate = system.fate_of(body, &host);
            let label = label::label(system, body.index).unwrap();
            let (_, now) = system.primary_record(&epoch, &HourCosines::new(), body, label, &fate);
            let Some(derived) = now.derived else {
                continue;
            };
            rows.insert(format!("{:?} {:?}", ctx.id(), body.index), row(&derived));
            for child in system.children(body.index) {
                if !matches!(child.part, Part::Moon(_)) {
                    continue;
                }
                let Some(derived) = system.record_moon(&epoch, child, (&now, &fate)) else {
                    continue;
                };
                rows.insert(
                    format!("{:?} {:?} moon", ctx.id(), child.index),
                    row(derived.body()),
                );
            }
        }
    }
    let mut states: BTreeMap<String, u32> = BTreeMap::new();
    for (key, b) in &rows {
        eprintln!(
            "T57A | {key} | {:?} | {:.3} | {:.3} | {:.6e} | {:.6e} | {:.6e}",
            b.0, b.1, b.2, b.3, b.4, b.5
        );
        *states.entry(format!("{:?}", b.0)).or_default() += 1;
    }
    eprintln!("{} bodies; by state: {states:?}", rows.len());
}

// P14.T54.a, lever (h): a body's zonal insolation table (decision-p14-t54a-passes §C.3).

/// Asserts two renderings `with` and `without` the same, showing where they first differ.
fn same_rendering(with: &str, without: &str, what: &str) {
    if with == without {
        return;
    }
    let at = with
        .bytes()
        .zip(without.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| with.len().min(without.len()));
    let near = |s: &str| {
        let start = (at.saturating_sub(300)..=at)
            .find(|&k| s.is_char_boundary(k))
            .unwrap_or(0);
        let end = (at..=(at + 300).min(s.len()))
            .rev()
            .find(|&k| s.is_char_boundary(k))
            .unwrap_or(s.len());
        s.get(start..end).unwrap_or_default().to_owned()
    };
    panic!(
        "{what} differs at byte {at}\n  with the table: …{}…\n  without it:     …{}…",
        near(with),
        near(without)
    );
}

/// Lever (h): over the 60-system sample, every zonal profile `generate` and `snapshot_at` build
/// reads its body's insolation table to the bit of the profile's own path, each profile checked
/// as it is built (`TableReads::Checked`), and the generated systems and their snapshots at the
/// epoch are the same, bit for bit, with the table and without it (`TableReads::Unlevered`).
#[test]
fn the_zonal_insolation_table_moves_no_bit_of_the_sample() {
    use crate::planetary::hooks::surface::{TableReads, with_table_reads};

    let run = || -> Vec<String> {
        sample()
            .iter()
            .take(60)
            .map(|ctx| {
                let system = generate(SEED, ctx);
                let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
                format!("{system:?}\n{snapshot:?}")
            })
            .collect()
    };
    let (read, checked) = with_table_reads(TableReads::Checked, run);
    let (unlevered, uncounted) = with_table_reads(TableReads::Unlevered, run);
    assert_eq!(uncounted, 0);
    assert_eq!(read.len(), 60);
    for (k, (with, without)) in read.iter().zip(&unlevered).enumerate() {
        same_rendering(with, without, &format!("system {k} of the sample"));
    }
    eprintln!("{checked} profiles with modes read off their tables and checked");
    // 1,461 at L1; 744 since lever (a), whose record builds one profile a surface, not two.
    assert!(checked > 500, "{checked}");
}

// P14.T54.a, levers (a) and (p): one profile a surface, and the bit-exact exits
// (decision-p14-t54a-passes, "Cost escalation (after Batch B)").

/// Every planet's, member's and moon's derivation in `system`, of context `ctx`, at the epoch, as
/// the records read them: its passes and their convergence included.
fn derivations_at_epoch(ctx: &SystemContext, system: &PlanetarySystem) -> String {
    use core::fmt::Write as _;

    let mut out = String::new();
    if ctx.age_at_epoch().value() <= 0.0 {
        return out;
    }
    let t = UniverseTime::EPOCH;
    let epoch = Epoch::new(system, ctx, t);
    for body in system.bodies() {
        if !matches!(body.part, Part::Planet(_) | Part::Member(_)) {
            continue;
        }
        let host = fate_host(ctx, system.zone_of(body));
        let fate = system.fate_of(body, &host);
        let label = label::label(system, body.index).unwrap();
        let (_, now) = system.primary_record(&epoch, &HourCosines::new(), body, label, &fate);
        writeln!(out, "{:?}: {:?}", body.index, now.derived).unwrap();
        for child in system.children(body.index) {
            if !matches!(child.part, Part::Moon(_)) {
                continue;
            }
            let derived = system.record_moon(&epoch, child, (&now, &fate));
            writeln!(out, "{:?}: {derived:?}", child.index).unwrap();
        }
    }
    out
}

/// Levers (a) and (p) over the 60-system sample: T24.b's water ice read from the record's
/// partition's cover, and the partition's solves and the partitioned greenhouses stopping at their
/// bit-exact fixed points.
///
/// - Every ice the record's second stage reads (`IceReads::Checked`), and every greenhouse and every
///   solve outside one (`Exits::Checked`), is checked against the unlevered path to the bit as it
///   is made.
/// - The generated systems, their snapshots at the epoch and every planet's, member's and moon's
///   derivation then, its pass count included, are the same bit for bit with the levers and
///   without (`IceReads::Unlevered`, `Exits::Unlevered`).
#[test]
fn levers_a_and_p_move_no_bit_of_the_sample() {
    use crate::planetary::chem::partition::{ExitCounts, Exits, with_exits};
    use crate::planetary::hooks::figures::{IceReads, with_ice_reads};

    let run = || -> Vec<String> {
        sample()
            .iter()
            .take(60)
            .map(|ctx| {
                let system = generate(SEED, ctx);
                let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
                let derivations = derivations_at_epoch(ctx, &system);
                format!("{system:?}\n{snapshot:?}\n{derivations}")
            })
            .collect()
    };
    let ((levered, counts), ices) =
        with_ice_reads(IceReads::Checked, || with_exits(Exits::Checked, run));
    let ((unlevered, uncounted), unread) =
        with_ice_reads(IceReads::Unlevered, || with_exits(Exits::Unlevered, run));
    assert_eq!(uncounted, ExitCounts::default());
    assert_eq!(unread, 0);
    assert_eq!(levered.len(), 60);
    for (k, (with, without)) in levered.iter().zip(&unlevered).enumerate() {
        same_rendering(with, without, &format!("system {k} of the sample"));
    }
    let most =
        counts.greenhouses * u64::from(crate::planetary::derive::atmosphere::GREENHOUSE_STEPS + 1);
    eprintln!(
        "{ices} record ices checked; {} greenhouses checked, {} evaluations of {most} at most, {} \
         step climates read; {} solves outside them checked; {} solves, {} sweeps of {} at most",
        counts.checked_greenhouses,
        counts.evaluations,
        counts.climates,
        counts.checked_solves,
        counts.solves,
        counts.sweeps,
        counts.solves * u64::from(crate::planetary::chem::partition::PARTITION_SWEEPS),
    );
    assert!(ices > 50, "{ices}");
    assert_eq!(counts.checked_greenhouses, counts.greenhouses);
    assert!(
        counts.greenhouses > 1_000 && counts.checked_solves > 1_000,
        "{counts:?}"
    );
}

// P14.T54.a, levers (d) and (e): the host shade's constants once a call chain, and generation's
// dead derivations (decision-p14-t54a-passes, "Cost escalation (after Batch B)" §C.3).

/// The first 60 systems of the sample, generated, and their snapshots at the epoch, rendered.
fn sixty_rendered() -> Vec<String> {
    sample()
        .iter()
        .take(60)
        .map(|ctx| {
            let system = generate(SEED, ctx);
            let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
            format!("{system:?}\n{snapshot:?}")
        })
        .collect()
}

/// Lever (d) over the 60-system sample: every body's shade that `generate` and `snapshot_at`
/// build, planets', members' and moons' alike, read off its call chain's book, is the shade each
/// body computes for itself, to the bit, each checked as it is built (`ShadeReads::Checked`); and
/// the generated systems and their snapshots are the same, bit for bit, with the book and without
/// it (`ShadeReads::Unlevered`).
#[test]
fn every_host_shade_off_its_book_is_its_own_to_the_bit() {
    use crate::planetary::derive::photometry::area_mix::{ShadeReads, with_shade_reads};

    let (read, checked) = with_shade_reads(ShadeReads::Checked, sixty_rendered);
    let (unlevered, uncounted) = with_shade_reads(ShadeReads::Unlevered, sixty_rendered);
    assert_eq!(uncounted, 0);
    assert_eq!(read.len(), 60);
    for (k, (with, without)) in read.iter().zip(&unlevered).enumerate() {
        same_rendering(with, without, &format!("system {k} of the sample"));
    }
    eprintln!("{checked} shades read off their books and checked");
    assert!(checked > 2_000, "{checked}");
}

/// Lever (e) over the 60-system sample: the rotation pass's and the Laplace frames' records at
/// `parent_time` read before the passes (e1), each member as its moons' parent before its passes
/// (e2) and each parent as its moons read it without its record (e3) are the unlevered path's,
/// each checked as it is read (`PassFree::Checked`); and the generated systems (every law,
/// Laplace frame, satellite, ring and moon) and their snapshots at the epoch are the same, bit for
/// bit, with the lever and without it (`PassFree::Unlevered`).
///
/// (e1) and (e2) are checked against paths of their own. (e3) is a structural move with no
/// independent check (Batch D's review, RD1): `primary_record` builds its record on
/// `primary_now_under`, which the record-free read's `primary_now` wraps, so its oracle is that
/// function again, and its count is the reads' count alone.
#[test]
fn generation_s_dead_derivations_move_no_bit_of_the_sample() {
    let (read, checked) = with_pass_free(PassFree::Checked, sixty_rendered);
    let (unlevered, uncounted) = with_pass_free(PassFree::Unlevered, sixty_rendered);
    assert_eq!(uncounted, PassFreeCounts::default());
    assert_eq!(read.len(), 60);
    for (k, (with, without)) in read.iter().zip(&unlevered).enumerate() {
        same_rendering(with, without, &format!("system {k} of the sample"));
    }
    eprintln!("checked: {checked:?}");
    assert_eq!(checked.records, 60, "{checked:?}");
    assert!(checked.members > 400 && checked.parents > 50, "{checked:?}");
}

// P14.T54.a, Batch E's D1: the states census, the dark-thick-air census and item 26's re-check over
// the sample and the golden systems.

/// The universe the golden systems are pinned in: `tests/planetary_golden.rs`'s `SYSTEMS_SEED`.
const GOLDEN_SYSTEMS_SEED: u64 = 0x5eed_0000_0014_0032;

/// The golden systems of `tests/planetary_golden.rs` (its `GOLDEN_SYSTEMS`, P14.T32) by name and
/// pinned ID, whose `planetary/systems` goldens pin them: the census's fixtures. A copy, since an
/// integration test's items are not the library's; `the_golden_systems_are_the_golden_test_s`
/// holds it to the original, in order.
const GOLDEN_SYSTEMS: [(&str, u64); 15] = [
    ("m_dwarf_resonant_chain", 0x0200_2b2c_a000_0000),
    ("hot_jupiter", 0x41fc_ecba_0000_0006),
    ("solar_like", 0x4200_6cba_0000_0009),
    ("eccentric_giant", 0x4200_acaa_0000_000a),
    ("halo_star", 0x0202_5b2b_e000_0001),
    ("close_binary", 0x4200_2cb2_0000_0009),
    ("wide_binary", 0x41ff_ecae_0000_0004),
    ("hierarchical_triple", 0x4200_2cb2_0000_0003),
    ("subgiant", 0x4200_6c9a_0000_0004),
    ("red_giant", 0x4200_ecd2_0000_0003),
    ("fallback_black_hole", 0x8200_b2e0_0000_000a),
    ("filler_a", 0x01ff_fb2c_6000_0000),
    ("filler_b", 0x2200_1659_8000_0000),
    ("filler_c", 0x4200_2cb2_0000_0000),
    ("t_tauri", 0x41fe_2c96_0000_000c),
];

/// The golden systems ([`GOLDEN_SYSTEMS`]) in the Milky Way fixture at their seed, generated whole
/// as `golden_systems_are_pinned` generates them, once, under the escape's hold whatever rules the
/// test that first asks has set ([`with_rules`](crate::planetary::chem::escape::with_rules)), so
/// that the cache is version 23's.
pub(super) fn golden_systems() -> &'static [(&'static str, SystemContext, PlanetarySystem)] {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    static SYSTEMS: OnceLock<Vec<(&'static str, SystemContext, PlanetarySystem)>> = OnceLock::new();
    SYSTEMS.get_or_init(|| with_rules(EscapeRules::Held, golden_systems_afresh))
}

/// The golden systems ([`GOLDEN_SYSTEMS`]) generated afresh, under the rules in force on the
/// calling thread (a test's switch among them, which a system generated once would not read).
pub(super) fn golden_systems_afresh() -> Vec<(&'static str, SystemContext, PlanetarySystem)> {
    let galaxy = crate::galaxy::Galaxy::from_params(
        Seed::new(GOLDEN_SYSTEMS_SEED),
        crate::galaxy::params::GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    GOLDEN_SYSTEMS
        .iter()
        .map(|&(name, id)| {
            let id = SystemId::from_raw(id).expect("a pinned ID is well formed");
            let ctx = SystemContext::for_system(&galaxy, id).expect("a pinned ID names a system");
            let system = generate(galaxy.seed(), &ctx);
            (name, ctx, system)
        })
        .collect()
}

/// Batch E's review, R9: [`GOLDEN_SYSTEMS`] and [`GOLDEN_SYSTEMS_SEED`] are the golden test's own,
/// `tests/planetary_golden.rs`'s `GOLDEN_SYSTEMS` (each name and pinned ID, in order) and
/// `SYSTEMS_SEED`, as its source states them.
#[test]
fn the_golden_systems_are_the_golden_test_s() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/planetary_golden.rs"
    ));
    // A pinned ID as the source writes it, in groups of four hexadecimal digits.
    let literal = |value: u64| {
        let hex = format!("{value:016x}");
        format!(
            "0x{}_{}_{}_{}",
            &hex[0..4],
            &hex[4..8],
            &hex[8..12],
            &hex[12..16]
        )
    };
    let seed = format!(
        "const SYSTEMS_SEED: u64 = {};",
        literal(GOLDEN_SYSTEMS_SEED)
    );
    assert!(source.contains(&seed), "{seed}");
    let (_, list) = source
        .split_once("const GOLDEN_SYSTEMS: [GoldenSystem; 15] = [")
        .expect("the golden test's list");
    let (list, _) = list.split_once("\n];").expect("the list's end");
    let found: Vec<(String, String)> = list
        .split("GoldenSystem {")
        .skip(1)
        .map(|entry| {
            let field = |name: &str| {
                entry
                    .lines()
                    .find_map(|line| line.trim().strip_prefix(name))
                    .map_or_else(
                        || panic!("an entry without its {name}: {entry}"),
                        |value| value.trim_end_matches(',').to_owned(),
                    )
            };
            (field("name: "), field("id: "))
        })
        .collect();
    let expected: Vec<(String, String)> = GOLDEN_SYSTEMS
        .iter()
        .map(|&(name, id)| (format!("\"{name}\""), literal(id)))
        .collect();
    assert_eq!(found, expected);
}

/// A body derived again under a driver, with its published pass's publication.
type DerivedAgain<'a> = &'a dyn Fn(
    PassDriver,
) -> (
    DerivedBody,
    Option<crate::planetary::derive::atmosphere::spun_read::SpunRead>,
);

/// A present planet, member or moon with a surface, as the census walks it ([`census_walk`]).
struct CensusCase<'a> {
    /// "sample", or the golden system's name.
    set: &'a str,
    /// P14.T57.a's key, `SystemId(…) BodyIndex(…)`, with " moon" for a moon.
    key: String,
    /// "planets", "members" or "moons".
    kind: &'static str,
    /// Its record at the epoch.
    record: &'a BodyRecord,
    /// Its record's surface.
    surface: &'a Surface,
    /// Its derivation over its record.
    derived: &'a DerivedBody,
    /// Its zone's stars at the time: each one's phase and effective temperature.
    hosts: String,
    /// Its zone's stars' phases.
    phases: String,
    /// Σ Fᵢ ÷ `S_eff,i` at the runaway-greenhouse limit at the hosts' present luminosities
    /// ([`runaway_now`]): 1 or more inside the present runaway limit.
    runaway_now: f64,
    /// The same at the hosts' largest past luminosities ([`runaway_at_peak`]), `Sky::of`'s sum:
    /// 1 or more inside the runaway limit at the peak.
    runaway_peak: f64,
    /// Σ Fᵢ ÷ `S_eff,i` at the maximum-greenhouse limit at the hosts' present luminosities
    /// ([`limit_now`]): below 1 beyond it.
    maximum_now: f64,
    /// [`runaway_now`], [`runaway_peak`](Self::runaway_peak) and [`maximum_now`](Self::maximum_now)
    /// with each `S_eff` unheld below the fit's 2,600 K ([`unheld_flux`]), in that order: P14.T53.b
    /// part 1's dependence on the hold beyond Kopparapu et al.'s fit.
    unheld: [f64; 3],
    /// It derived again under a driver.
    again: DerivedAgain<'a>,
}

/// The phases and effective temperatures of the stars of `zone` at `t`.
fn zone_hosts(ctx: &SystemContext, zone: &OrbitZone, t: UniverseTime) -> (String, String) {
    let states: Vec<_> = zone
        .members()
        .map(|member| ctx.stars()[usize::from(member)].state_at(t))
        .collect();
    let hosts = states
        .iter()
        .map(|state| {
            state.map_or_else(
                || "unformed".to_owned(),
                |s| format!("{:?} {:.0} K", s.phase(), s.effective_temperature().value()),
            )
        })
        .collect::<Vec<_>>()
        .join(" + ");
    let phases = states
        .iter()
        .map(|state| state.map_or_else(|| "unformed".to_owned(), |s| format!("{:?}", s.phase())))
        .collect::<Vec<_>>()
        .join(" + ");
    (hosts, phases)
}

/// Σ Fᵢ ÷ `S_eff,i` at the runaway-greenhouse limit (Kopparapu et al. 2013, P14.T12.b) at the
/// present luminosities of the stars `zone`'s bodies orbit, along `orbit`, and of the companions,
/// along their pairs' orbits: `Sky::of`'s sum, at the luminosities now rather than at their
/// largest past (decision-pre-23-review item 33). 1 or more inside the present runaway limit.
fn runaway_now(epoch: &Epoch<'_>, zone: &OrbitZone, orbit: &KeplerElements) -> f64 {
    limit_now(
        crate::planetary::derive::habitable_zone::HabitableLimit::RunawayGreenhouse,
        epoch,
        zone,
        orbit,
    )
}

/// The flux `S_eff` of the limit `limit` about a host at `t_eff`, as the generator reads it:
/// [`HabitableLimit::flux`], held to the fit's 2,600–7,200 K.
///
/// [`HabitableLimit::flux`]: crate::planetary::derive::habitable_zone::HabitableLimit::flux
fn held_flux(
    limit: crate::planetary::derive::habitable_zone::HabitableLimit,
    t_eff: Kelvin,
) -> f64 {
    limit.flux(t_eff).value()
}

/// [`held_flux`] unheld below the fit's 2,600 K: Kopparapu et al.'s quartic (2013, ApJ 765, 131,
/// eq. 2, the erratum's coefficients, ApJ 770, 82, Table 3) extrapolated to a cooler host, and held
/// above 7,200 K as the generator holds it, so that it is [`held_flux`]'s to the bit from 2,600 K
/// up. For P14.T53.b part 1's census of the present class's dependence on the hold (the batch
/// review's item 8): the fit is not valid below 2,600 K, so neither value is the true limit there.
fn unheld_flux(
    limit: crate::planetary::derive::habitable_zone::HabitableLimit,
    t_eff: Kelvin,
) -> f64 {
    use crate::planetary::derive::habitable_zone::{FIT_CENTRE, FIT_HOTTEST};
    let [at_centre, linear, quadratic, cubic, quartic] = limit.coefficients();
    let offset = t_eff.value().min(FIT_HOTTEST.value()) - FIT_CENTRE.value();
    at_centre
        + linear * offset
        + quadratic * offset * offset
        + cubic * offset * offset * offset
        + quartic * crate::math::powi(offset, 4)
}

/// Σ Fᵢ ÷ `S_eff,i` at the runaway-greenhouse limit at the largest past luminosities of the
/// stars `zone`'s bodies orbit, along `orbit`, and of the companions, along their pairs' orbits:
/// `Sky::of`'s sum for the insolation the inventory reads (P14.T53.b part 1 (a)). 1 or more inside
/// the runaway limit at the hosts' peak.
fn runaway_at_peak(epoch: &Epoch<'_>, zone: &OrbitZone, orbit: &KeplerElements) -> f64 {
    limit_at(
        crate::planetary::derive::habitable_zone::HabitableLimit::RunawayGreenhouse,
        (HostLight::peak_luminosity, held_flux),
        (epoch, zone, orbit),
    )
}

/// Σ Fᵢ ÷ `S_eff,i` at the habitable zone's limit `limit` at the present luminosities of the stars
/// `zone`'s bodies orbit, along `orbit`, and of the companions, along their pairs' orbits
/// ([`runaway_now`]; at the maximum-greenhouse limit `Sky::of`'s sum now, the ceiling's: below 1
/// beyond it).
fn limit_now(
    limit: crate::planetary::derive::habitable_zone::HabitableLimit,
    epoch: &Epoch<'_>,
    zone: &OrbitZone,
    orbit: &KeplerElements,
) -> f64 {
    limit_at(
        limit,
        (HostLight::luminosity, held_flux),
        (epoch, zone, orbit),
    )
}

/// The runaway ratio now, at the peak and the maximum-greenhouse ratio now of a body of `zone`
/// along `orbit` with each `S_eff` unheld below 2,600 K ([`unheld_flux`]; [`CensusCase::unheld`]).
fn unheld_ratios(epoch: &Epoch<'_>, zone: &OrbitZone, orbit: &KeplerElements) -> [f64; 3] {
    use crate::planetary::derive::habitable_zone::HabitableLimit;
    let at = |limit, luminosity| limit_at(limit, (luminosity, unheld_flux), (epoch, zone, orbit));
    [
        at(HabitableLimit::RunawayGreenhouse, HostLight::luminosity),
        at(
            HabitableLimit::RunawayGreenhouse,
            HostLight::peak_luminosity,
        ),
        at(HabitableLimit::MaximumGreenhouse, HostLight::luminosity),
    ]
}

/// The limits' fluxes `S_eff`(`T_eff`) as a census reads them: [`held_flux`] or [`unheld_flux`].
type LimitFlux = fn(crate::planetary::derive::habitable_zone::HabitableLimit, Kelvin) -> f64;

/// [`limit_now`] at each host's luminosity as `luminosity` reads it, its present one or its
/// largest past ([`runaway_at_peak`]), over the limit's flux as `s_eff` reads it, held or not. A
/// host counts where it shines now, as `Sky::of` counts it.
fn limit_at(
    limit: crate::planetary::derive::habitable_zone::HabitableLimit,
    (luminosity, s_eff): (fn(&HostLight) -> crate::units::SolarLuminosities, LimitFlux),
    (epoch, zone, orbit): (&Epoch<'_>, &OrbitZone, &KeplerElements),
) -> f64 {
    let share = |light: &HostLight, (a, e): (crate::units::Metres, f64)| {
        if light.luminosity().value() > 0.0 {
            luminosity_flux(luminosity(light), a, e).value()
                / s_eff(limit, light.effective_temperature())
        } else {
            0.0
        }
    };
    let along = (orbit.semi_major_axis(), orbit.eccentricity().value());
    let orbited: f64 = epoch
        .lights(zone)
        .unwrap_or_default()
        .iter()
        .map(|light| share(light, along))
        .sum();
    let companions: f64 = epoch
        .companions(zone)
        .unwrap_or_default()
        .iter()
        .map(|c| share(c.host(), (c.semi_major_axis(), c.eccentricity())))
        .sum();
    orbited + companions
}

/// Every present planet, member and moon of `system` with a surface and a derivation at the epoch,
/// in body order, each given to `visit` ([`CensusCase`]).
fn census_walk(
    set: &str,
    ctx: &SystemContext,
    system: &PlanetarySystem,
    visit: &mut dyn FnMut(&CensusCase<'_>),
) {
    use crate::planetary::derive::atmosphere::spun_read;
    let t = UniverseTime::EPOCH;
    if ctx.age_at_epoch().value() <= 0.0 {
        return;
    }
    let epoch = Epoch::new(system, ctx, t).with_shades();
    let cosines = HourCosines::new();
    for body in system.bodies() {
        let kind = match body.part {
            Part::Planet(_) => "planets",
            Part::Member(_) => "members",
            Part::Moon(_) | Part::Ring(_) => continue,
        };
        let zone = system.zone_of(body);
        let host = fate_host(ctx, zone);
        let fate = system.fate_of(body, &host);
        let at = fate.at(t);
        let (hosts, phases) = zone_hosts(ctx, zone, t);
        let runaway = at
            .orbit()
            .map_or(f64::NAN, |orbit| runaway_now(&epoch, zone, orbit));
        let runaway_peak = at
            .orbit()
            .map_or(f64::NAN, |orbit| runaway_at_peak(&epoch, zone, orbit));
        let maximum = at.orbit().map_or(f64::NAN, |orbit| {
            limit_now(
                habitable_zone::HabitableLimit::MaximumGreenhouse,
                &epoch,
                zone,
                orbit,
            )
        });
        let unheld = at
            .orbit()
            .map_or([f64::NAN; 3], |orbit| unheld_ratios(&epoch, zone, orbit));
        let label = label::label(system, body.index).unwrap();
        let (record, now) = system.primary_record(&epoch, &cosines, body, label, &fate);
        if let (Section::Ok(surface), Some(derived)) = (record.surface(), now.derived.as_ref()) {
            let again = |driver| {
                let ((derived, _, _), read) = spun_read::capture(|| {
                    system.primary_derived(&epoch, &cosines, (body, &fate), &at, driver)
                });
                (derived, read)
            };
            visit(&CensusCase {
                set,
                key: format!("{:?} {:?}", ctx.id(), body.index),
                kind,
                record: &record,
                surface,
                derived,
                hosts: hosts.clone(),
                phases: phases.clone(),
                runaway_now: runaway,
                runaway_peak,
                maximum_now: maximum,
                unheld,
                again: &again,
            });
        }
        for child in system.children(body.index) {
            if !matches!(child.part, Part::Moon(_)) {
                continue;
            }
            let label = label::label(system, child.index).unwrap();
            let record = system.satellite_record(&epoch, &cosines, child, label, &now, &fate);
            let Some(moon) = system.record_moon(&epoch, child, (&now, &fate)) else {
                continue;
            };
            let Section::Ok(surface) = record.surface() else {
                continue;
            };
            let again = |driver| {
                let (derived, read) = spun_read::capture(|| {
                    system.record_moon_driven(&epoch, child, (&now, &fate), driver)
                });
                (*derived.unwrap().body(), read)
            };
            visit(&CensusCase {
                set,
                key: format!("{:?} {:?} moon", ctx.id(), child.index),
                kind: "moons",
                record: &record,
                surface,
                derived: moon.body(),
                hosts: hosts.clone(),
                phases: phases.clone(),
                runaway_now: runaway,
                runaway_peak,
                maximum_now: maximum,
                unheld,
                again: &again,
            });
        }
    }
}

/// Every present planet, member and moon of `system` with a derivation and a bulk section at the
/// epoch, whatever its surface's tag, in body order, each given to `visit` with its kind
/// ("planets", "members" or "moons"), its record and its derivation: P14.T53.b part 2's walk,
/// since [`census_walk`] visits only the `Ok` surfaces.
fn envelope_walk(
    ctx: &SystemContext,
    system: &PlanetarySystem,
    visit: &mut dyn FnMut(&str, &BodyRecord, &DerivedBody),
) {
    let t = UniverseTime::EPOCH;
    if ctx.age_at_epoch().value() <= 0.0 {
        return;
    }
    let epoch = Epoch::new(system, ctx, t).with_shades();
    let cosines = HourCosines::new();
    for body in system.bodies() {
        let kind = match body.part {
            Part::Planet(_) => "planets",
            Part::Member(_) => "members",
            Part::Moon(_) | Part::Ring(_) => continue,
        };
        let host = fate_host(ctx, system.zone_of(body));
        let fate = system.fate_of(body, &host);
        let label = label::label(system, body.index).unwrap();
        let (record, now) = system.primary_record(&epoch, &cosines, body, label, &fate);
        if let (Section::Ok(_), Some(derived)) = (record.bulk(), now.derived.as_ref()) {
            visit(kind, &record, derived);
        }
        for child in system.children(body.index) {
            if !matches!(child.part, Part::Moon(_)) {
                continue;
            }
            let label = label::label(system, child.index).unwrap();
            let record = system.satellite_record(&epoch, &cosines, child, label, &now, &fate);
            if record.bulk().ok().is_none() {
                continue;
            }
            if let Some(moon) = system.record_moon(&epoch, child, (&now, &fate)) {
                visit("moons", &record, moon.body());
            }
        }
    }
}

/// What the census tallies and lists ([`the_states_and_the_dark_thick_airs_are_recorded`]).
#[derive(Default)]
struct Census {
    bodies: std::collections::BTreeMap<String, u32>,
    dark: Vec<String>,
    dark_by_kind: std::collections::BTreeMap<String, u32>,
    snowballs: std::collections::BTreeMap<String, u32>,
    hot: Vec<String>,
    iced: Vec<String>,
    dry_melts: Vec<String>,
    outside_runaway: Vec<String>,
    airless_with_pressure: std::collections::BTreeMap<String, (u32, f64)>,
    under_ice: Vec<String>,
    second_starts: Vec<String>,
    escalations: Vec<String>,
    cover: CoverRuleTally,
}

impl Census {
    /// Tallies and lists the body `case`.
    #[expect(
        clippy::too_many_lines,
        reason = "one body's census across the rulings' classes, each listed"
    )]
    fn visit(&mut self, case: &CensusCase<'_>) {
        use crate::planetary::chem::partition::CondensatePhase;
        use crate::planetary::derive::Under;
        use crate::planetary::derive::atmosphere::GreenhouseConstants;
        use crate::planetary::hooks::figures::cover::UNDER_ICE_MELTING_POINT;
        let (surface, derived) = (case.surface, case.derived);
        let group = if case.set == "sample" {
            "sample"
        } else {
            "goldens"
        };
        let index = case.record.index().get();
        let name = if case.set == "sample" {
            format!("{} ({})", case.record.identity().id(), case.key)
        } else {
            format!("{} {index:04x}", case.set)
        };
        let state = surface.state();
        let (conditions, figures) = (surface.conditions(), surface.figures());
        let t_s = conditions.mean_surface_temperature().value();
        let p = conditions.surface_pressure().value();
        let (bond, t_eq) = (
            derived.albedo().value(),
            derived.equilibrium_temperature().value(),
        );
        let g = derived.surface_gravity().value();
        let (ice, ocean) = (figures.ice_fraction(), figures.ocean_fraction());
        let terms = GreenhouseConstants::in_force().terms(
            derived.atmosphere().partial_pressures(),
            derived.equilibrium_temperature(),
            derived.surface_gravity(),
        );
        let largest = terms
            .iter()
            .max_by(|a, b| a.depth().total_cmp(&b.depth()))
            .copied()
            .expect("the greenhouse has its five terms");
        *self
            .bodies
            .entry(format!("{group} {}", case.kind))
            .or_default() += 1;
        let line = format!(
            "{name}: {} {state:?}, T_s {t_s:.2} K, p_s {:.4} bar, T_eq {t_eq:.2} K, A {bond:.4}, \
             ice {ice:.4}, ocean {ocean:.4}, g {g:.3} m s⁻², τ {terms:?}, host {}",
            case.kind,
            p / 1e5,
            case.hosts,
        );
        eprintln!(
            "CENSUS | {} | {} | {} | {index:04x} | {} | {state:?} | {t_s:.6} | {p:.6e} | \
             {t_eq:.6} | {bond:.6} | {ice:.6} | {ocean:.6} | {g:.6} | {:?} | {}",
            case.set,
            case.key,
            case.record.identity().id(),
            case.kind,
            largest.gas(),
            case.phases,
        );
        // G3: the dark-thick-air census (decision-p14-t54a-passes §G3.6).
        if state != SurfaceState::MagmaOcean && p >= 1e5 && bond < 0.1 {
            *self
                .dark_by_kind
                .entry(format!("{group} {}", case.kind))
                .or_default() += 1;
            self.dark.push(format!(
                "{line}; p_s ÷ g {:.2} of Earth's",
                (p / g) / (101_325.0 / 9.806_65)
            ));
        }
        // Decision-p14-t54a-states §6.4: the states.
        match state {
            SurfaceState::Snowball => {
                *self
                    .snowballs
                    .entry(format!("{group} {}", case.phases))
                    .or_default() += 1;
                if t_s > 273.15 {
                    self.hot
                        .push(format!("{line}; its largest τ term {largest:?}"));
                    if largest.gas() != Gas::CarbonDioxide {
                        self.escalations.push(format!(
                            "a hot Snowball whose largest τ term is not CO₂'s: {line}"
                        ));
                    }
                }
                let liquid = surface
                    .condensates()
                    .entry(Gas::Water.substance(), CondensatePhase::Liquid)
                    .is_some();
                let layer = figures.surface_water().value();
                let melting = UNDER_ICE_MELTING_POINT.value();
                if t_s >= melting
                    && figures.heat_flow().value() > 0.0
                    && (t_s > melting || layer > 0.0)
                {
                    self.under_ice.push(format!(
                        "{line}; its surface water {layer:.4e} kg m⁻², an H2O liquid entry \
                         {liquid}, so the guard {}",
                        if liquid {
                            "moved nothing"
                        } else {
                            "removed an ocean under ice"
                        }
                    ));
                }
            }
            SurfaceState::Temperate if ice >= 0.5 => self.iced.push(line.clone()),
            SurfaceState::MagmaOcean if p <= 0.0 => self.dry_melts.push(line.clone()),
            SurfaceState::RunawayGreenhouse => {
                if case.runaway_now < 1.0 {
                    self.outside_runaway.push(format!(
                        "{line}; Σ F ÷ S_eff,runaway now {:.4}",
                        case.runaway_now
                    ));
                } else if t_s < 373.15 {
                    self.escalations.push(format!(
                        "a RunawayGreenhouse below 373.15 K inside the present runaway limit \
                         ({:.4}): {line}",
                        case.runaway_now
                    ));
                }
            }
            SurfaceState::Airless if p > 0.0 => {
                let entry = self
                    .airless_with_pressure
                    .entry(group.to_owned())
                    .or_default();
                entry.0 += 1;
                entry.1 = entry.1.max(p);
                if p >= 100.0 {
                    self.escalations
                        .push(format!("an Airless body at or above 100 Pa: {line}"));
                }
            }
            SurfaceState::Temperate
            | SurfaceState::MagmaOcean
            | SurfaceState::Airless
            | SurfaceState::GasEnvelope => {}
        }
        // Item 26's re-check (decision-p14-t54a-states §6.3): the second start, T13.c's table
        // albedo of the other branch.
        let start = match state {
            SurfaceState::Snowball if t_s > 273.15 => Some(0.50),
            SurfaceState::Temperate if ice >= 0.5 => Some(0.294),
            SurfaceState::RunawayGreenhouse => {
                Some(SurfaceState::MagmaOcean.albedo(surface.material()).value())
            }
            SurfaceState::Snowball
            | SurfaceState::Temperate
            | SurfaceState::MagmaOcean
            | SurfaceState::Airless
            | SurfaceState::GasEnvelope => None,
        };
        if let Some(start) = start {
            let (second, _) = (case.again)(PassDriver::From(BondAlbedo::from_fraction(start)));
            let described = |d: &DerivedBody| {
                format!(
                    "{:?} at {:.2} K, A {:.6} ({} passes, {:?})",
                    d.atmosphere().state(),
                    d.atmosphere().surface_temperature().value(),
                    d.albedo().value(),
                    d.atmosphere_passes(),
                    d.convergence(),
                )
            };
            let same = second.atmosphere().state() == state
                && (second.albedo().value() - bond).abs() < 1e-6;
            self.second_starts.push(format!(
                "{name}: from 0.3, the generator's start, which chose the published, {}; from \
                 {start} {}: {}",
                described(derived),
                described(&second),
                if same { "single-branch" } else { "bistable" },
            ));
        }
        // The cover rule and the water follow-up check over the published pass's partition (rule
        // (f): the record's), where the pass published one (not a melt's).
        let (_, read) = (case.again)(PassDriver::Again(Under::Read));
        check_cover_rule(
            case.record.identity().id(),
            surface,
            read.as_ref().map(|read| &read.partition),
            &mut self.cover,
        );
    }
}

/// A measurement for the bless note, run by name (decision-p14-t54a-passes §G3.6;
/// decision-p14-t54a-states §6.3–6.4 and §7; decision-pre-23-review item 26;
/// decision-p14-t54a-water-consistency, "Batch B follow-up"), over every present planet, member
/// and moon with a surface of the first 60 whole systems and of the 15 golden systems at the
/// epoch:
///
/// - **the dark-thick-air census:** every body not molten under at least 1 bar with a Bond albedo
///   below 0.1, by kind, with `p_s`, g, `p_s` ÷ g against Earth's, the Bond albedo, `T_s` and its
///   hosts;
/// - **the states census:** the snowballs by set and by their hosts' phase, each above 273.15 K
///   with `T_s`, p, `T_eq`, A, ice, ocean and its τ terms; the Temperate worlds whose ices cover at
///   least half the surface; the `MagmaOcean` worlds with no volatile air; the runaway greenhouses
///   outside the present runaway limit; the `Airless` bodies with pressure; and the under-ice
///   guard's cases (the Snowballs at or above 273.15 K with a heat flow, where the unguarded shell
///   term would have listed an ocean under their ice);
/// - **item 26's re-check:** every such hot `Snowball` from 0.50, every such iced `Temperate` from
///   0.294 and every runaway greenhouse from the melt's 0.10 (T13.c's table albedos,
///   `PassDriver::From`): what each settles to, and whether it is bistable or single-branch, the
///   generator's start, 0.3, having chosen the published;
/// - **the cover rule and the water follow-up check** ([`check_cover_rule`]) over the published
///   pass's partition: every liquid entry with an area liquid where it lies, asserted, and every
///   phase `condensate_of`'s fallback set listed.
///
/// The rulings' escalations are listed, not failed: a hot snowball whose largest τ term is not
/// CO₂'s, an `Airless` body at or above 100 Pa, a runaway greenhouse below 373.15 K inside the
/// present runaway limit. Each body also prints a `CENSUS` line, which the bless note joins with
/// version 22's figures (P14.T57.a's run over the sample; the goldens at 22): more than 3 sample
/// snowballs below 273.15 K at 22 and above at 23 go to main.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn the_states_and_the_dark_thick_airs_are_recorded() {
    let mut census = Census::default();
    for (ctx, system) in whole().iter().take(60) {
        census_walk("sample", ctx, system, &mut |case| census.visit(case));
    }
    for (name, ctx, system) in golden_systems() {
        census_walk(name, ctx, system, &mut |case| census.visit(case));
    }
    let list = |items: &[String]| items.join("\n  ");
    let cover = &census.cover;
    eprintln!("bodies with a surface: {:?}", census.bodies);
    eprintln!(
        "the dark-thick-air census (not molten, p_s ≥ 1 bar, Bond < 0.1) by kind {:?}:\n  {}",
        census.dark_by_kind,
        list(&census.dark)
    );
    eprintln!(
        "snowballs by set and host phase {:?}; above 273.15 K {}:\n  {}",
        census.snowballs,
        census.hot.len(),
        list(&census.hot)
    );
    eprintln!(
        "Temperate with ices ≥ 0.5: {}:\n  {}",
        census.iced.len(),
        list(&census.iced)
    );
    eprintln!(
        "MagmaOcean with no volatile air: {}:\n  {}",
        census.dry_melts.len(),
        list(&census.dry_melts)
    );
    eprintln!(
        "runaway greenhouses outside the present runaway limit: {}:\n  {}",
        census.outside_runaway.len(),
        list(&census.outside_runaway)
    );
    eprintln!(
        "Airless with pressure (bodies, the most, Pa): {:?}",
        census.airless_with_pressure
    );
    eprintln!(
        "the under-ice guard's cases: {}:\n  {}",
        census.under_ice.len(),
        list(&census.under_ice)
    );
    eprintln!(
        "item 26's second starts: {}:\n  {}",
        census.second_starts.len(),
        list(&census.second_starts)
    );
    eprintln!(
        "the cover rule: W3 (a) {}, W3 (b) {}, scaled {}; liquid entries checked {}; phases \
         condensate_of's fallback set {}:\n  {}\nliquid entries that would boil {}:\n  {}",
        cover.crust_ice,
        cover.under_ice,
        cover.scaled,
        cover.liquids.liquids,
        cover.liquids.fallbacks.len(),
        list(&cover.liquids.fallbacks),
        cover.liquids.boiling.len(),
        list(&cover.liquids.boiling)
    );
    eprintln!(
        "ESCALATIONS (to the lane): {}:\n  {}",
        census.escalations.len(),
        list(&census.escalations)
    );
    assert!(
        cover.liquids.boiling.is_empty(),
        "{:?}",
        cover.liquids.boiling
    );
}

/// How far below its published pass's Rayleigh floor a body's published Bond albedo may lie: the
/// passes' own settling, 10⁻⁶ of the floor, since the published albedo is the solve's fixed point
/// and the floor is read again at it (P14.T24.h.a's tests).
const FLOOR_SETTLING: f64 = 1e-6;

/// What one body of a census walk under the Rayleigh floor publishes against its published pass's
/// floor ([`floored_census`]).
struct FlooredCase {
    /// The golden system's name and the body's index, as the dark-thick-air census names it, or
    /// the sample's record ID and key.
    name: String,
    /// "planets", "members" or "moons".
    kind: &'static str,
    /// Its published Bond albedo.
    bond: f64,
    /// Its published pass's floor, read by running that pass again.
    floor: f64,
    /// Its state.
    state: SurfaceState,
    /// Its surface pressure.
    pressure: Pascals,
    /// Its surface gravity.
    gravity: MetresPerSecondSquared,
    /// Its mean surface temperature.
    surface_temperature: Kelvin,
    /// How its passes ended.
    convergence: crate::planetary::derive::passes::Convergence,
    /// Its zone's stars.
    hosts: String,
}

/// Every body with a surface of `system`, walked as [`census_walk`] walks it, each with its
/// published pass's floor, read by running that pass again (`PassDriver::Again`), under the
/// floor's rules where the caller sets them.
fn floored_census(set: &str, ctx: &SystemContext, system: &PlanetarySystem) -> Vec<FlooredCase> {
    use crate::planetary::derive::atmosphere::floor_read;
    use crate::planetary::derive::{PassDriver, Under};
    let mut cases = Vec::new();
    census_walk(set, ctx, system, &mut |case| {
        // The pass is run again only for the floor it reads; its derivation is the case's own.
        floor_read::clear();
        let (_again, _read) = (case.again)(PassDriver::Again(Under::Read));
        let floor = floor_read::take().expect("a body with a surface reads its floor");
        let conditions = case.surface.conditions();
        let name = if case.set == "sample" {
            format!("{} ({})", case.record.identity().id(), case.key)
        } else {
            format!("{} {:04x}", case.set, case.record.index().get())
        };
        cases.push(FlooredCase {
            name,
            kind: case.kind,
            bond: case.derived.albedo().value(),
            floor,
            state: case.surface.state(),
            pressure: conditions.surface_pressure(),
            gravity: case.derived.surface_gravity(),
            surface_temperature: conditions.mean_surface_temperature(),
            convergence: case.derived.convergence(),
            hosts: case.hosts.clone(),
        });
    });
    cases
}

/// Whether a body is one of the dark-thick-air census's (decision-p14-t54a-passes §G3.6): not
/// molten, under at least 1 bar, with a Bond albedo below 0.1.
fn dark_thick(state: SurfaceState, pressure: Pascals, bond: f64) -> bool {
    state != SurfaceState::MagmaOcean && pressure.value() >= 1e5 && bond < 0.1
}

/// decision-p14-t54a-passes §G3.3's 13 dark thick airs of the golden systems, by name and index:
/// the 4 still below a Bond albedo of 0.1 at 23 (`close_binary` 0900 and 0a00, `filler_c` 0400 and
/// e201) and the 9 that P14.T51.c's lids lifted above it, which stay dark against their own
/// Rayleigh layer.
const DARK_THICK_AIRS: [&str; 13] = [
    "close_binary 0700",
    "close_binary 0900",
    "close_binary 0a00",
    "close_binary e303",
    "close_binary e304",
    "close_binary e401",
    "close_binary e402",
    "close_binary e501",
    "filler_c 0400",
    "filler_c e201",
    "solar_like 0400",
    "subgiant 0402",
    "wide_binary 0600",
];

/// P14.T24.h.a's second test (decision-p14-t54a-passes §G3.5): §G3.3's 13 dark thick airs of the
/// golden systems ([`DARK_THICK_AIRS`]) each take at least their air's own Rayleigh layer's Bond
/// albedo under the floor, stated with `p_s` ÷ g against Earth's and the hosts, and `filler_c`
/// e201's is at least 0.25 under its F8 host (§G3.2's bound, with CH₄ black beyond 0.6 µm). Every
/// body with a surface of those systems publishes at least its published pass's floor, to the
/// passes' settling ([`FLOOR_SETTLING`]), and those the area mix alone leaves dark under 1 bar or
/// more are among the 13.
#[test]
fn the_rayleigh_floor_lifts_the_golden_systems_dark_thick_airs() {
    use std::collections::BTreeMap;

    use crate::planetary::derive::photometry::area_mix::{RayleighFloorRules, with_rayleigh_floor};

    let mut held = BTreeMap::new();
    for (name, ctx, system) in golden_systems() {
        census_walk(name, ctx, system, &mut |case| {
            let bond = case.derived.albedo().value();
            let pressure = case.surface.conditions().surface_pressure();
            let key = format!("{name} {:04x}", case.record.index().get());
            if dark_thick(case.surface.state(), pressure, bond) {
                assert!(
                    DARK_THICK_AIRS.contains(&key.as_str()),
                    "{key} is dark and thick"
                );
            }
            held.insert(key, (bond, pressure));
        });
    }
    let earth_column_kg_m2 = 101_325.0 / 9.806_65;
    let cases: Vec<FlooredCase> = with_rayleigh_floor(RayleighFloorRules::Floored, || {
        golden_systems_afresh()
            .iter()
            .flat_map(|(name, ctx, system)| floored_census(name, ctx, system))
            .collect()
    });
    let mut lifted = Vec::new();
    for case in &cases {
        assert!(
            case.bond >= case.floor * (1.0 - FLOOR_SETTLING),
            "{}: Bond {} under its floor {}",
            case.name,
            case.bond,
            case.floor
        );
        if !DARK_THICK_AIRS.contains(&case.name.as_str()) {
            continue;
        }
        let (bond, pressure) = held[&case.name];
        let (p_pa, g_m_s2) = (case.pressure.value(), case.gravity.value());
        lifted.push(format!(
            "{}: {} {:?}, p_s {:.3} → {:.3} bar, g {g_m_s2:.3} m s⁻², p_s ÷ g {:.2} of Earth's, \
             Bond {bond:.4} → {:.4} (floor {:.4}{}), T_s {:.2} K, {:?}, host {}",
            case.name,
            case.kind,
            case.state,
            pressure.value() / 1e5,
            p_pa / 1e5,
            p_pa / g_m_s2 / earth_column_kg_m2,
            case.bond,
            case.floor,
            if case.floor >= case.bond * (1.0 - FLOOR_SETTLING) {
                ", binding"
            } else {
                ""
            },
            case.surface_temperature.value(),
            case.convergence,
            case.hosts,
        ));
        if case.name == "filler_c e201" {
            assert!(case.bond >= 0.25, "filler_c e201's Bond {}", case.bond);
        }
    }
    eprintln!(
        "§G3.3's dark thick airs under the floor ({} bodies with a surface):\n  {}",
        cases.len(),
        lifted.join("\n  ")
    );
    assert_eq!(lifted.len(), DARK_THICK_AIRS.len(), "{lifted:#?}");
}

/// P14.T24.h.a's fourth test, a measurement run by name (decision-p14-t54a-passes §G3.5): over
/// every present planet, member and moon with a surface of the first 60 whole systems at the
/// epoch, generated afresh under the floor, no body publishes a Bond albedo below its published
/// pass's floor ([`FLOOR_SETTLING`]) but a jump, which publishes the lower albedo of its bistable
/// pair (P14.T51.h) and is listed with its floor, and the passes' guards hold: at most 3% of the bodies
/// bracketed, 1% jumping, and 1% changing their state against the area mix alone; over all 400
/// whole systems, at most 2% of the planets jumping (G1's population). It states how many bodies
/// the floor moves, by kind and state, and the dark thick airs it leaves.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the sample under the floor, against the area mix alone, with the guards"
)]
fn the_rayleigh_floor_holds_over_the_sample_and_the_passes_guards_with_it() {
    use std::collections::BTreeMap;

    use crate::planetary::derive::photometry::area_mix::{RayleighFloorRules, with_rayleigh_floor};

    let mut held = BTreeMap::new();
    for (ctx, system) in whole().iter().take(60) {
        census_walk("sample", ctx, system, &mut |case| {
            held.insert(
                format!("{} ({})", case.record.identity().id(), case.key),
                (
                    case.surface.state(),
                    case.derived.albedo().value(),
                    case.derived.convergence(),
                ),
            );
        });
    }
    let (cases, planets, below_jumps): (Vec<FlooredCase>, (u32, u32), Vec<String>) =
        with_rayleigh_floor(RayleighFloorRules::Floored, || {
            use crate::planetary::derive::atmosphere::floor_read;
            use crate::planetary::derive::{PassDriver, Under};
            let mut cases = Vec::new();
            let (mut planets, mut jumping) = (0_u32, 0_u32);
            let mut below_jumps = Vec::new();
            for (i, ctx) in sample().iter().enumerate() {
                let system = generate(SEED, ctx);
                if i < 60 {
                    cases.extend(floored_census("sample", ctx, &system));
                }
                if ctx.age_at_epoch().value() <= 0.0 {
                    continue;
                }
                let t = UniverseTime::EPOCH;
                let epoch = Epoch::new(&system, ctx, t).with_shades();
                let cosines = HourCosines::new();
                for body in system.bodies() {
                    if !matches!(body.part, Part::Planet(_)) {
                        continue;
                    }
                    let host = fate_host(ctx, system.zone_of(body));
                    let fate = system.fate_of(body, &host);
                    let at = fate.at(t);
                    let now = system.primary_now(&epoch, &cosines, body, &fate, &at);
                    let Some(derived) = now.derived else {
                        continue;
                    };
                    planets += 1;
                    if !derived.convergence().is_jump() {
                        continue;
                    }
                    jumping += 1;
                    // A jump's published pass, run again, reads its floor.
                    floor_read::clear();
                    let _again = system.primary_derived(
                        &epoch,
                        &cosines,
                        (body, &fate),
                        &at,
                        PassDriver::Again(Under::Read),
                    );
                    if let Some(floor) = floor_read::take()
                        && derived.albedo().value() < floor * (1.0 - FLOOR_SETTLING)
                    {
                        below_jumps.push(format!(
                            "{}: Bond {:.4} under its floor {floor:.4}",
                            body.index.body_id(ctx.id()),
                            derived.albedo().value()
                        ));
                    }
                }
            }
            (cases, (planets, jumping), below_jumps)
        });
    let (mut below, mut bracketed, mut jumps, mut flips) = (Vec::new(), 0_u32, 0_u32, Vec::new());
    let (mut held_bracketed, mut held_jumps, mut jumpers) = (0_u32, 0_u32, Vec::new());
    let mut unjumped = Vec::new();
    let mut moved: BTreeMap<String, u32> = BTreeMap::new();
    let mut dark = Vec::new();
    for case in &cases {
        let (state, bond, convergence) = held
            .get(&case.name)
            .copied()
            .unwrap_or_else(|| panic!("{} has a surface under the area mix alone", case.name));
        held_bracketed += u32::from(matches!(
            convergence,
            crate::planetary::derive::passes::Convergence::Bracketed { .. }
        ));
        held_jumps += u32::from(convergence.is_jump());
        if convergence.is_jump() && !case.convergence.is_jump() {
            unjumped.push(case.name.clone());
        }
        if case.convergence.is_jump() {
            // A jump publishes the lower albedo of its bistable pair (P14.T51.h), not a pass's
            // own: its published pass reads the floor at that albedo, and its output is the
            // other end. Listed, with its floor, not asserted.
            jumpers.push(format!(
                "{}: {} {:?}, p_s {:.4} bar, T_s {:.2} K, Bond {:.4} (floor {:.4}; held {:.4}, \
                 {}a jump there)",
                case.name,
                case.kind,
                case.state,
                case.pressure.value() / 1e5,
                case.surface_temperature.value(),
                case.bond,
                case.floor,
                bond,
                if convergence.is_jump() { "" } else { "not " },
            ));
        } else if case.bond < case.floor * (1.0 - FLOOR_SETTLING) {
            below.push(format!(
                "{}: {} {:?}, p_s {:.4} bar, T_s {:.2} K, {:?}: Bond {} under its floor {}",
                case.name,
                case.kind,
                case.state,
                case.pressure.value() / 1e5,
                case.surface_temperature.value(),
                case.convergence,
                case.bond,
                case.floor
            ));
        }
        bracketed += u32::from(matches!(
            case.convergence,
            crate::planetary::derive::passes::Convergence::Bracketed { .. }
        ));
        jumps += u32::from(case.convergence.is_jump());
        if state != case.state {
            flips.push(format!("{}: {state:?} → {:?}", case.name, case.state));
        }
        if bond.total_cmp(&case.bond).is_ne() {
            *moved
                .entry(format!("{} {:?}", case.kind, case.state))
                .or_default() += 1;
        }
        if dark_thick(case.state, case.pressure, case.bond) {
            dark.push(format!(
                "{}: {:?}, p_s {:.3} bar, Bond {:.4} (floor {:.4})",
                case.name,
                case.state,
                case.pressure.value() / 1e5,
                case.bond,
                case.floor
            ));
        }
    }
    let bodies = u32::try_from(cases.len()).expect("the sample has fewer than 2³² bodies");
    let share = |n: u32| 100.0 * f64::from(n) / f64::from(bodies);
    let flipped = u32::try_from(flips.len()).expect("fewer than 2³² flips");
    eprintln!(
        "under the floor, {bodies} bodies with a surface: {bracketed} bracketed ({:.2}%), {jumps} \
         jumping ({:.2}%), {flipped} changing state ({:.2}%):\n  {}\nunder the area mix alone, \
         {held_bracketed} bracketed and {held_jumps} jumping, of which no longer jumping {unjumped:?}\n\
         the jumps under the floor:\n  {}\n\
         moved Bond by kind and state {moved:?}\ndark thick airs left {}:\n  {}\nthe 400's planets: \
         {} jumping of {} ({:.2}%), {} of them under their floor:\n  {}",
        share(bracketed),
        share(jumps),
        share(flipped),
        flips.join("\n  "),
        jumpers.join("\n  "),
        dark.len(),
        dark.join("\n  "),
        planets.1,
        planets.0,
        100.0 * f64::from(planets.1) / f64::from(planets.0),
        below_jumps.len(),
        below_jumps.join("\n  "),
    );
    assert!(below.is_empty(), "{below:?}");
    assert!(bodies > 100, "{bodies}");
    assert!(
        100 * bracketed <= 3 * bodies,
        "{bracketed} of {bodies} bracketed"
    );
    assert!(100 * jumps <= bodies, "{jumps} of {bodies} jumping");
    assert!(
        100 * flipped <= bodies,
        "{flipped} of {bodies} change state"
    );
    assert!(
        50 * planets.1 <= planets.0,
        "{} of {} planets jump",
        planets.1,
        planets.0
    );
}

/// What P14.T24.i moves over a set of systems ([`unlit_moves`]).
#[derive(Debug, Default)]
struct UnlitMoves {
    /// Each unlit record's line: its state, its mean and coldest temperatures, its ices and what
    /// `for_body` answers on it.
    unlit: Vec<String>,
    /// How many unlit records `for_body` reads, and how many it refuses for an argon ice with no
    /// solid density (P14.T49.n's row).
    unlit_read: usize,
    unlit_argon: usize,
    /// How many lit airless records rise to their night floor, and of them how many change their
    /// surface ices or condensates, how many `for_body` answers differently on, and how many hold
    /// an argon ice (the overlap with P14.T49.n's 15–19 K bodies), with their means' range.
    raised: usize,
    raised_cover: usize,
    raised_read: usize,
    raised_argon: usize,
    raised_argon_means: Option<(f64, f64)>,
    /// The records above their floor whose passes reached it on their way, settled again within
    /// [`RESETTLED`]: their state unchanged, and their `T_eq` and mean within it.
    resettled: Vec<String>,
    /// The records with an air whose `T_eq` lies under their night floor, which the base
    /// temperature raises (P14.T24.i's follow-up, decision-t24i-airless-state D1.3: the greenhouse
    /// starts from the floored mean), with their state and mean held and floored.
    raised_airs: Vec<String>,
    /// Any other record that moved, with its first difference.
    moved: Vec<String>,
}

/// The labelled refusal `for_body` gives a body whose ice the registry holds no solid density of,
/// as `ReadCoarseInputsError`'s `Debug` writes it, and whether a refusal is it.
fn is_unmodelled_surface(read: &str) -> bool {
    read == "Section(NotModelled(Surface))"
}

/// Whether the record `record` lists an argon ice on its surface.
fn holds_argon_ice(record: &BodyRecord) -> bool {
    record.surface().ok().is_some_and(|surface| {
        surface
            .figures()
            .surface_ices()
            .iter()
            .any(|&(ice, _)| ice.substance().key().as_str() == "Ar")
    })
}

/// What P14.T24.i moves over the systems `held` and the same systems generated under the unlit
/// mean's rules, `floored`, in order: their records at
/// [`RECORD_TIME`](crate::planetary::surface::RECORD_TIME), each held record derived under the
/// unlit mean's hold and each floored one under its rules, every other rule the caller's (every 24
/// hold, `planetary::testing::with_every_24_hold`, for the tests of a 24 figure).
///
/// - Each record with a surface whose mean surface temperature is not positive held (an unlit
///   one) reads, floored, at least its conditions' coldest temperature, its night floor, above
///   0 K; `for_body` reads it, or refuses it for an argon ice with no solid density, the labelled
///   refusal (asserted), which under P14.T49.n's densities (held until 24, a caller's switch) it
///   gives none.
/// - A lit airless record whose mean, its equilibrium temperature, lies under its night floor
///   rises to it, as the rule says: counted, with what else moves on it.
/// - A record above its floor whose passes reached it on their way may move its `T_eq` and mean by
///   [`RESETTLED`] of them, its state unchanged: listed.
/// - A record with an air, held or floored, whose `T_eq` lies under its night floor (`night_floor`
///   at the heat flow its conditions read) rises with its greenhouse's base (P14.T24.i's follow-up,
///   decision-t24i-airless-state D1.3): listed.
/// - Every other record is unchanged to the bit, its whole `Debug` text, or it is listed.
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the records, held and floored, each classed and listed"
)]
fn unlit_moves(
    held: &[(&str, SystemContext, PlanetarySystem)],
    floored: &[(&str, SystemContext, PlanetarySystem)],
) -> UnlitMoves {
    use crate::planetary::derive::atmosphere::{UnlitMeanRules, with_unlit_mean};
    use crate::planetary::surface::{CoarseInputs, RECORD_TIME};
    let mut moves = UnlitMoves::default();
    let mean_of = |record: &BodyRecord| {
        record.surface().ok().map_or(f64::NAN, |surface| {
            surface.conditions().mean_surface_temperature().value()
        })
    };
    for ((name, ctx, before), (_, _, after)) in held.iter().zip(floored) {
        // The records derive at the snapshot, so the held ones under the floor's hold and the
        // floored ones under its rules.
        let before = with_unlit_mean(UnlitMeanRules::Held, || {
            before.snapshot_at(ctx, RECORD_TIME)
        });
        let after = with_unlit_mean(UnlitMeanRules::Floored, || {
            after.snapshot_at(ctx, RECORD_TIME)
        });
        let (old_records, records) = (before.bodies(), after.bodies());
        let read = |record: &BodyRecord, records: &[BodyRecord], rules: UnlitMeanRules| {
            let planet = CoarseInputs::planet_of(record)
                .and_then(|index| records.iter().find(|r| r.index() == index));
            match with_unlit_mean(rules, || CoarseInputs::for_body(record, planet, ctx)) {
                Ok(_) => "Ok".to_owned(),
                Err(error) => format!("{error:?}"),
            }
        };
        for (old, new) in old_records.iter().zip(records) {
            let id = format!("{name} {}", new.identity().id());
            let mean_held = mean_of(old);
            // A mean that is not a number held is unlit too.
            if (mean_held.is_nan() || mean_held <= 0.0)
                && let Some(surface) = new.surface().ok()
            {
                let conditions = surface.conditions();
                let mean = conditions.mean_surface_temperature().value();
                let coldest = conditions.climate_range().0.value();
                assert!(
                    mean > 0.0 && mean >= coldest * (1.0 - 1e-12),
                    "{id}: mean {mean} K under its coldest {coldest} K"
                );
                let answer = read(new, records, UnlitMeanRules::Floored);
                let argon = holds_argon_ice(new);
                if answer == "Ok" {
                    moves.unlit_read += 1;
                } else {
                    assert!(
                        is_unmodelled_surface(&answer) && argon,
                        "{id}: for_body {answer} with no argon ice"
                    );
                    moves.unlit_argon += 1;
                }
                moves.unlit.push(format!(
                    "{id}: {:?}, mean {mean:.3} K (0 K held), coldest {coldest:.3} K, argon ice \
                     {argon}; for_body {answer}",
                    surface.state()
                ));
                continue;
            }
            if format!("{old:?}") == format!("{new:?}") {
                continue;
            }
            // A lit body whose equilibrium temperature lies under its night floor rises to it.
            if let (Some(before_surface), Some(after_surface)) =
                (old.surface().ok(), new.surface().ok())
            {
                let coldest = after_surface.conditions().climate_range().0.value();
                let mean = mean_of(new);
                if before_surface.state() == SurfaceState::Airless
                    && mean_held < coldest
                    && mean >= coldest * (1.0 - 1e-12)
                {
                    moves.raised += 1;
                    let cover = |s: &Surface| {
                        format!(
                            "{:?} {:?}",
                            s.figures().surface_ices(),
                            s.condensates().entries()
                        )
                    };
                    moves.raised_cover +=
                        usize::from(cover(before_surface) != cover(after_surface));
                    moves.raised_read += usize::from(
                        read(old, old_records, UnlitMeanRules::Held)
                            != read(new, records, UnlitMeanRules::Floored),
                    );
                    if holds_argon_ice(old) {
                        moves.raised_argon += 1;
                        let (lo, hi) = moves.raised_argon_means.unwrap_or((mean, mean));
                        moves.raised_argon_means = Some((lo.min(mean), hi.max(mean)));
                    }
                    continue;
                }
            }
            // A body above its floor whose passes reached it on their way (from the first pass's
            // albedo) settles again within the passes' convergence, its state unchanged.
            let t_eq = |record: &BodyRecord| {
                record
                    .bulk()
                    .ok()
                    .map_or(f64::NAN, |bulk| bulk.equilibrium_temperature().value())
            };
            let state = |record: &BodyRecord| record.surface().ok().map(Surface::state);
            let (t0, t1) = (t_eq(old), t_eq(new));
            let mean = mean_of(new);
            if state(old) == state(new)
                && (t1 - t0).abs() <= RESETTLED * t0
                && (mean - mean_held).abs() <= RESETTLED * mean_held
            {
                moves.resettled.push(format!(
                    "{id}: T_eq {t0} → {t1} K, mean {mean_held} → {mean} K"
                ));
                continue;
            }
            // An air whose `T_eq` lies under its floor: its greenhouse starts from the floor.
            let under_floor = |record: &BodyRecord| {
                record.surface().ok().is_some_and(|surface| {
                    let heat_flow = surface
                        .basis()
                        .map_or_else(|| surface.figures().heat_flow(), |basis| basis.heat_flow);
                    surface.state() != SurfaceState::Airless
                        && t_eq(record)
                            < crate::planetary::hooks::surface::night_floor(heat_flow).value()
                })
            };
            if under_floor(old) || under_floor(new) {
                moves.raised_airs.push(format!(
                    "{id}: {:?} → {:?}, mean {mean_held} → {mean} K",
                    state(old),
                    state(new)
                ));
                continue;
            }
            let (a, b) = (format!("{old:?}"), format!("{new:?}"));
            let at = a
                .char_indices()
                .zip(b.chars())
                .find(|((_, x), y)| x != y)
                .map_or(0, |((i, _), _)| i);
            let start = a[..at].char_indices().rev().nth(120).map_or(0, |(i, _)| i);
            moves.moved.push(format!(
                "{id}: mean {mean_held} → {mean} K; first difference after …{}",
                &a[start..at]
            ));
        }
    }
    moves
}

/// How far a body above its night floor may move its equilibrium temperature and its mean under
/// the unlit mean's rules, as a share of each: 10⁻⁸, ten times the passes' convergence
/// ([`ATMOSPHERE_CONVERGENCE`](crate::planetary::derive::ATMOSPHERE_CONVERGENCE)'s 10⁻⁹), where an
/// early pass reached the floor and the passes settled again at the same fixed point.
const RESETTLED: f64 = 1e-8;

/// The held and the floored systems of `systems`, each generated afresh under the rules in force
/// but for the unlit mean's, held and floored, for [`unlit_moves`]: inside
/// `planetary::testing::with_every_24_hold`, every 24 hold but the unlit mean's against all of
/// them (P14.T24.i's follow-up, decision-t24i-airless-state D1.4).
fn unlit_moves_of(
    systems: &dyn Fn() -> Vec<(&'static str, SystemContext, PlanetarySystem)>,
) -> UnlitMoves {
    use crate::planetary::derive::atmosphere::{UnlitMeanRules, with_unlit_mean};
    let held = with_unlit_mean(UnlitMeanRules::Held, systems);
    let floored = with_unlit_mean(UnlitMeanRules::Floored, systems);
    unlit_moves(&held, &floored)
}

/// The 400 whole systems of the sample, each generated afresh under the rules in force.
fn sample_afresh() -> Vec<(&'static str, SystemContext, PlanetarySystem)> {
    sample()
        .iter()
        .map(|ctx| ("sample", ctx.clone(), generate(SEED, ctx)))
        .collect()
}

/// The names of the records `lines` lists, each line's text to its first colon.
fn named(lines: &[String]) -> Vec<&str> {
    lines
        .iter()
        .map(|line| line.split(':').next().unwrap_or_default())
        .collect()
}

/// P14.T24.i's tests over the golden systems (decision-bump-plan-after-22, "Version-24 lanes after
/// 23", V3.3, with main's ruling on its reach, 2026-10-22), with every landed 24 hold in force
/// (P14.T24.i's follow-up, decision-t24i-airless-state D1.4: the 24 combination, every 24 hold
/// but the unlit mean's against all of them): `fallback_black_hole`'s 19 unlit bodies, whose
/// records publish a mean surface temperature of 0 K at 23, read at least their night floor, and
/// `for_body` reads all 19, none refused for an argon ice (under the escape's batch none of them
/// holds one); 11 lit airless records under their floor rise to it, 8 of them changing their ices
/// or condensates, none their `for_body` answer, one holding an argon ice, now at 30.17 K; the air
/// whose `T_eq` lies under its floor, subgiant's moon 0504, rises with its greenhouse's base; no
/// record settles again; and every other record of the fifteen systems is unchanged to the bit.
///
/// Restated from the held escape's (T24.i's follow-up's held path, 23's densities): the unlit
/// records' (19, 15, 4) became (19, 19, 0); the raised airless records 10 → 11, subgiant's moon
/// 0503, an air on the held path (64.2 → 128.1 K), being airless at its 117.7 K floor under the
/// escape's batch; the raised airs [0503, 0504] → [0504].
#[test]
fn under_every_24_hold_the_golden_systems_unlit_records_read_their_night_floor() {
    let moves =
        crate::planetary::testing::with_every_24_hold(|| unlit_moves_of(&golden_systems_afresh));
    eprintln!("the golden systems under the unlit mean, every 24 hold in force: {moves:#?}");
    assert!(
        moves
            .unlit
            .iter()
            .all(|line| line.starts_with("fallback_black_hole")),
        "{:#?}",
        moves.unlit
    );
    assert_eq!(
        (moves.unlit.len(), moves.unlit_read, moves.unlit_argon),
        (19, 19, 0)
    );
    assert_eq!(
        (
            moves.raised,
            moves.raised_cover,
            moves.raised_read,
            moves.raised_argon
        ),
        (11, 8, 0, 1)
    );
    assert_eq!(moves.resettled, Vec::<String>::new());
    assert_eq!(
        named(&moves.raised_airs),
        ["subgiant 42006c9a00000004.0504"]
    );
    assert_eq!(moves.moved, Vec::<String>::new());
}

/// P14.T24.i's tests over the 400 whole systems with every landed 24 hold in force, a measurement
/// run by name (P14.T24.i's follow-up, decision-t24i-airless-state D1.4, the conformance review's
/// must-fix): their 18 unlit records (bless-note-23's) each read at least their night floor (the
/// walk asserts it on each) and `for_body` answers `Ok` on all 18, none refused for its argon ice;
/// the lit airless records raised to their floor are counted, with what else moves on them and
/// their overlap with the argon-ice bodies; the airs whose `T_eq` lies under their floor, which
/// rise with their greenhouse's base (the follow-up's D1.3), are listed; the records that settle
/// again within [`RESETTLED`] are listed; and no other record moves.
///
/// Restated from T24.i's (the escape held, 23's densities): the unlit records' (18, 10, 8) became
/// (18, 18, 0), under the escape's batch none of them holding an argon ice; the raised, their cover
/// changes, their `for_body` changes, their argon ices and the re-settled went
/// 1,405 / 1,144 / 3 / 622 / 3 to 1,403 / 1,127 / 0 / 383 / 4 (the argon ices' means 7.79–69.69
/// K, were 7.8–451 K; 02003b2be0000000.0700 the fourth to re-settle, as on the follow-up's held
/// path); and the airs raised with their greenhouse's base are 7.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn under_every_24_hold_the_sample_s_unlit_records_read_their_night_floor() {
    let moves = crate::planetary::testing::with_every_24_hold(|| unlit_moves_of(&sample_afresh));
    eprintln!("the sample under the unlit mean, every 24 hold in force: {moves:#?}");
    assert_eq!(
        (moves.unlit.len(), moves.unlit_read, moves.unlit_argon),
        (18, 18, 0)
    );
    assert!(
        moves.unlit.iter().all(|line| line.ends_with("for_body Ok")),
        "{:#?}",
        moves.unlit
    );
    assert_eq!(
        (
            moves.raised,
            moves.raised_cover,
            moves.raised_read,
            moves.raised_argon
        ),
        (1_403, 1_127, 0, 383)
    );
    assert_eq!(
        named(&moves.resettled),
        [
            "sample 01ffdb2c60020000.0700",
            "sample 01fffb2d60010000.0601",
            "sample 02003b2be0000000.0700",
            "sample 41ffecb1ff800000.e486"
        ]
    );
    assert_eq!(moves.raised_airs.len(), 7, "{:#?}", moves.raised_airs);
    eprintln!(
        "raised {}, their cover moved {}, their for_body {}, argon ices {} (means {:?}); \
         re-settled {}; airs raised {}",
        moves.raised,
        moves.raised_cover,
        moves.raised_read,
        moves.raised_argon,
        moves.raised_argon_means,
        moves.resettled.len(),
        moves.raised_airs.len()
    );
    assert_eq!(moves.moved, Vec::<String>::new());
}

/// P14.T49.n's case of T24.i's golden-systems test (main moved T24.i's "`for_body` answers `Ok` on
/// them" here, 2026-10-22), in the 24 combination (P14.T24.i's follow-up, decision-t24i-airless-
/// state D1.4): with every landed 24 hold in force, 24's densities among them
/// (`READ_DENSITIES_VERSION`, solid argon's fcc read), `fallback_black_hole`'s 19 unlit records
/// read at least their night floor and `for_body` reads all 19, and so it does with every 24 hold
/// but 24's densities: under the escape's batch none of the 19 holds an argon ice (P14.T49.n's
/// science check: the unlit bodies lose theirs under 24's rules), so none needs solid argon's
/// density.
///
/// Restated from the held escape's (19, 19, 0) under 24's densities and (19, 15, 4) under 23's: (19,
/// 19, 0) under both.
#[test]
fn under_24_s_densities_the_golden_systems_unlit_records_all_read() {
    use crate::substance::{ReadDensitiesRules, with_read_densities};
    let (read, held) = crate::planetary::testing::with_every_24_hold(|| {
        (
            unlit_moves_of(&golden_systems_afresh),
            with_read_densities(ReadDensitiesRules::Held, || {
                unlit_moves_of(&golden_systems_afresh)
            }),
        )
    });
    eprintln!(
        "the golden systems' unlit records, every 24 hold in force: {:#?}; with 23's densities: \
         {:#?}",
        read.unlit, held.unlit
    );
    for moves in [&read, &held] {
        assert_eq!(
            (moves.unlit.len(), moves.unlit_read, moves.unlit_argon),
            (19, 19, 0)
        );
        assert!(
            moves
                .unlit
                .iter()
                .all(|line| line.contains("argon ice false")),
            "{:#?}",
            moves.unlit
        );
    }
}

/// P14.T49.n's case of T24.i's test over the 400 whole systems, a measurement run by name, in the
/// 24 combination (as [`under_24_s_densities_the_golden_systems_unlit_records_all_read`]'s): with
/// every landed 24 hold in force the 18 unlit records read their night floor and `for_body` reads
/// all 18, and so it does with every 24 hold but 24's densities: under the escape's batch the 8
/// unlit 22–24 K members that held an argon ice under the hold (81ffb2dff0000019.e201–e208) hold
/// none.
///
/// Restated from the held escape's (18, 18, 0) under 24's densities and (18, 10, 8) under 23's: (18,
/// 18, 0) under both.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn under_24_s_densities_the_sample_s_unlit_records_all_read() {
    use crate::substance::{ReadDensitiesRules, with_read_densities};
    let (read, held) = crate::planetary::testing::with_every_24_hold(|| {
        (
            unlit_moves_of(&sample_afresh),
            with_read_densities(ReadDensitiesRules::Held, || unlit_moves_of(&sample_afresh)),
        )
    });
    eprintln!(
        "the sample's unlit records, every 24 hold in force: {:#?}; with 23's densities: {:#?}",
        read.unlit, held.unlit
    );
    for moves in [&read, &held] {
        assert_eq!(
            (moves.unlit.len(), moves.unlit_read, moves.unlit_argon),
            (18, 18, 0)
        );
        assert!(
            moves
                .unlit
                .iter()
                .all(|line| line.contains("argon ice false")),
            "{:#?}",
            moves.unlit
        );
    }
}

/// What `for_body` answers on the record `record` of a system of records `records` and context
/// `ctx`, under the rules in force on the calling thread: `None` where it reads it; otherwise its
/// refusal's cause as the surface-inputs census names it (`unread_cause`: "no crust", "unlit", or
/// the palette's first unresolved entry as `"<key> <role>: <cause>"`), and any other error's
/// `Debug` text.
fn refusal_of(record: &BodyRecord, records: &[BodyRecord], ctx: &SystemContext) -> Option<String> {
    use crate::planetary::surface::inputs::tests::unread_cause;
    use crate::planetary::surface::{CoarseInputs, ReadCoarseInputsError, SurfaceInputsError};
    let planet = CoarseInputs::planet_of(record)
        .and_then(|index| records.iter().find(|r| r.index() == index));
    let error = CoarseInputs::for_body(record, planet, ctx).err()?;
    let not_modelled =
        ReadCoarseInputsError::Section(SurfaceInputsError::NotModelled(RecordSection::Surface));
    match (record.surface(), error == not_modelled) {
        (Section::Ok(surface), true) => Some(
            unread_cause(surface).unwrap_or_else(|| format!("{error:?}, its palette resolved")),
        ),
        _ => Some(format!("{error:?}")),
    }
}

/// The refusals by cause of `for_body` over the records of `snapshot`'s bodies with a surface, of
/// the system of context `ctx`, added to `refused`, with those bodies counted into `bodies`.
fn tally_refusals(
    snapshot: &SystemSnapshot,
    ctx: &SystemContext,
    bodies: &mut usize,
    refused: &mut std::collections::BTreeMap<String, usize>,
) {
    let records = snapshot.bodies();
    for record in records {
        if !matches!(record.surface(), Section::Ok(_)) {
            continue;
        }
        *bodies += 1;
        if let Some(cause) = refusal_of(record, records, ctx) {
            *refused.entry(cause).or_default() += 1;
        }
    }
}

/// One argon-ice record of P14.T49.n's census ([`the_sample_s_argon_ices_under_24_s_rules`]):
/// its line and what the summary counts.
#[expect(
    clippy::struct_excessive_bools,
    reason = "a census row's flags, each counted apart in the summary"
)]
struct ArgonIce {
    line: String,
    mean: f64,
    lies_at: f64,
    lit: bool,
    airless: bool,
    exosphere: bool,
    /// Whether its profile is zonal (not a locked world's), and whether its derivation's air is
    /// collisional (`cold_traps::is_collisional`): the cold traps' batch takes an airless body with
    /// both a zonal profile and an air that is not.
    zonal: bool,
    collisional_air: bool,
    /// The Hertz–Knudsen sublimation over the age over the ice's reservoir per unit area of the
    /// ice, at the warmest temperature, at the mean and where the ice lies.
    sublimed_warmest: f64,
    sublimed_mean: f64,
    sublimed_where_it_lies: f64,
    /// The Jeans escape over the age of an exosphere at the ice's vapour pressure at the warmest
    /// temperature, over the same reservoir.
    escaped: f64,
    /// Argon's Jeans parameter at the surface's warmest temperature.
    lambda_surface: f64,
    /// Whether argon's vapour at the mean, and at the temperature the ice lies at, is a
    /// collisional air.
    collisional_at_mean: bool,
    collisional_where_it_lies: bool,
}

/// Argon's molecular mass, kg ([`Gas::molecular_mass`]).
fn argon_molecule_kg() -> f64 {
    Gas::Argon.molecular_mass().value()
}

/// Argon's vapour pressure over its solid at `t` K on the registry's sublimation curve (P14.T49.b's
/// diagram), Pa; above its triple point its liquid's, and `NaN` where the curve gives none.
fn argon_vapour_pa(t: f64) -> f64 {
    crate::substance::saturation_pressure(Gas::Argon.substance(), Kelvin::new(t))
        .map_or(f64::NAN, Pascals::value)
}

/// The Hertz–Knudsen flux of argon's ice into a vacuum at `t` K, kg m⁻² s⁻¹: `p_sat` √(m ÷ 2π k T)
/// (no return flux), m argon's molecular mass.
fn argon_hertz_knudsen(t: f64) -> f64 {
    use crate::units::consts::BOLTZMANN_CONSTANT;
    argon_vapour_pa(t) * (argon_molecule_kg() / (2.0 * PI * BOLTZMANN_CONSTANT * t)).sqrt()
}

/// The temperature, K, at which [`argon_hertz_knudsen`] is the airless ice line's loss of 144 kg
/// m⁻² Gyr⁻¹ (`AIRLESS_ICE_LOSS_KG_PER_M2_GYR`), by bisection on 5–83 K: the census's check of
/// its formula against the partition's vacuum edge (`exosphere_threshold`).
fn argon_vacuum_edge_k() -> f64 {
    use crate::planetary::chem::partition::AIRLESS_ICE_LOSS_KG_PER_M2_GYR;
    use crate::units::consts::SECONDS_PER_GIGAYEAR;
    let loss = AIRLESS_ICE_LOSS_KG_PER_M2_GYR / SECONDS_PER_GIGAYEAR;
    let (mut lo, mut hi) = (5.0_f64, 83.0_f64);
    for _ in 0..80 {
        let mid = f64::midpoint(lo, hi);
        if argon_hertz_knudsen(mid) < loss {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    f64::midpoint(lo, hi)
}

/// The [`ArgonIce`] of the census case `case`, whose system's context is `ctx`, at the rules in
/// force; `None` where its record holds no argon ice.
///
/// The estimates, each per square metre of the ice (its reservoir over its area), with m argon's
/// molecular mass, k Boltzmann's constant, M and R the body's mass and radius and t the system's
/// age:
///
/// - the vapour pressure `p_sat`(T) on the registry's sublimation curve ([`argon_vapour_pa`]);
/// - the Hertz–Knudsen flux J = `p_sat` √(m ÷ 2π k T), kg m⁻² s⁻¹, the gross sublimation into a
///   vacuum with no return flux ([`argon_hertz_knudsen`]), times t;
/// - the Jeans escape of a surface-bounded exosphere in equilibrium with the ice, n = `p_sat` ÷ k T
///   at the surface as its exobase: Φ = m n √(2kT ÷ m) ÷ (2√π) (1 + λ) e^−λ = J (1 + λ) e^−λ,
///   λ = G M m ÷ (k T R), times t;
/// - J at the warmest temperature of the record's climate range, at its mean and where the ice
///   lies (its condensate's `lies_at`), and Φ at the warmest;
/// - whether an argon air at `p_sat` is collisional on the body (`cold_traps::collisional`, p ≥ m g
///   ÷ `σ_c`), at the mean and where the ice lies.
///
/// Its solid edge's rule is the cold traps' batch's, which the census reads its records under: the
/// zonal annual maxima's vacuum edge (`SolidEdges::Exosphere`) on a body the batch takes (a zonal
/// profile, airless, its air not collisional: `cold_traps::qualifies`), T51.a's partition's edge
/// elsewhere. Its dominance and `lies_at` are its published pass's partition's, the record's; its
/// air's pressure is its derivation's atmosphere's, which an airless record's conditions state as
/// none.
#[expect(
    clippy::too_many_lines,
    reason = "one record's census line, each figure read and formatted"
)]
fn argon_ice_of(case: &CensusCase<'_>, ctx: &SystemContext) -> Option<ArgonIce> {
    use crate::planetary::chem::partition::{CondensatePhase, Partitioned};
    use crate::planetary::derive::atmosphere::jeans_parameter;
    use crate::planetary::derive::{PassDriver, Under};
    use crate::planetary::hooks::cold_traps::{collisional, is_collisional};
    use crate::planetary::hooks::surface::ColdProfile;
    use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
    use crate::units::{EarthMasses, Kilograms, Metres};
    let ar = Gas::Argon.substance();
    let surface = case.surface;
    let entry = *surface.condensates().entry(ar, CondensatePhase::Solid)?;
    let record = case.record;
    let bulk = record.bulk().ok()?;
    let conditions = surface.conditions();
    let (coldest, warmest) = conditions.climate_range();
    let (warmest, mean) = (
        warmest.value(),
        conditions.mean_surface_temperature().value(),
    );
    let profile = conditions.cold_profile();
    let (zonal_lo, zonal_hi) = match &profile {
        ColdProfile::Eyeball { day_k, night_k } => (*night_k, *day_k),
        ColdProfile::Zonal(points) => points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &(_, t)| {
                (lo.min(t), hi.max(t))
            }),
    };
    let air = case.derived.atmosphere();
    let g = bulk.surface_gravity();
    let airless = surface.state() == SurfaceState::Airless;
    let zonal = matches!(profile, ColdProfile::Zonal(_));
    let collisional_air = is_collisional(air, g);
    let exosphere = zonal && airless && !collisional_air;
    let (_, read) = (case.again)(PassDriver::Again(Under::Read));
    let (d, lies_at, condensed, airborne) =
        read.as_ref()
            .map_or((f64::NAN, f64::NAN, f64::NAN, f64::NAN), |read| {
                let partition = &read.partition;
                let condensate = partition.condensate(ar);
                (
                    partition.of(ar).map_or(f64::NAN, Partitioned::dominance),
                    condensate.map_or(f64::NAN, |c| c.lies_at().value()),
                    condensate.map_or(f64::NAN, |c| c.reservoir().value()),
                    partition
                        .of(ar)
                        .map_or(f64::NAN, |s| s.partial_pressure().value()),
                )
            });
    let p_s = air.surface_pressure().map_or(f64::NAN, Pascals::value);
    let p_ar = air.partial_pressures().of(Gas::Argon).value();
    let t_exo = air.exobase_temperature().value();
    let mass = Kilograms::from(
        record
            .mass()
            .ok()
            .copied()
            .unwrap_or(EarthMasses::new(f64::NAN)),
    );
    let radius = Metres::from(bulk.radius());
    let lambda_at = |t: f64| jeans_parameter(mass, radius, Gas::Argon, Kelvin::new(t));
    let lambda_surface = lambda_at(warmest);
    let lambda_exo = lambda_at(t_exo.max(warmest));
    let age_s = ctx.age_at_epoch().value() * SECONDS_PER_JULIAN_YEAR;
    let area = entry.area_fraction();
    let per_ice = entry.reservoir().value() / area;
    let j_warm = argon_hertz_knudsen(warmest);
    let jeans = j_warm * (1.0 + lambda_surface) * crate::math::exp(-lambda_surface);
    let over_reservoir = |flux: f64| flux * age_s / per_ice;
    let collides = |t: f64| {
        collisional(
            Pascals::new(argon_vapour_pa(t)),
            Gas::Argon.molar_mass_g_per_mol(),
            g,
        )
    };
    let ice = ArgonIce {
        line: String::new(),
        mean,
        lies_at,
        lit: bulk.equilibrium_temperature().value() > 0.0,
        airless,
        exosphere,
        zonal,
        collisional_air,
        sublimed_warmest: over_reservoir(j_warm),
        sublimed_mean: over_reservoir(argon_hertz_knudsen(mean)),
        sublimed_where_it_lies: over_reservoir(argon_hertz_knudsen(lies_at)),
        escaped: over_reservoir(jeans),
        lambda_surface,
        collisional_at_mean: collides(mean),
        collisional_where_it_lies: collides(lies_at),
    };
    let line = format!(
        "{} {} {:?} {:?}: M {:.3e} kg, R {:.1} km, g {:.3} m s⁻², age {:.3} Gyr; T_s {mean:.2} K \
         (range {:.2}–{warmest:.2} K, zonal annual means {zonal_lo:.2}–{zonal_hi:.2} K), T_eq \
         {:.2} K, {}; Ar ice: lies_at {lies_at:.2} K, area {area:.4e} (trapped {:.3e}), reservoir \
         {:.4e} kg m⁻² ({per_ice:.4e} per m² of ice; the partition's {condensed:.4e}), layer {}; \
         air p_s {p_s:.4e} Pa, p_Ar {p_ar:.4e} Pa (the partition's airborne {airborne:.4e}), {} \
         profile, air {}collisional; edge {}, d {d:.4}; T_exo {t_exo:.1} K, λ_Ar {lambda_exo:.2} \
         at max(T_exo, warmest), {lambda_surface:.2} at the warmest; p_sat(Ar) {:.3e} Pa at the \
         warmest, {:.3e} at the mean, {:.3e} at lies_at; HK × age ÷ reservoir {:.3e} at the \
         warmest, {:.3e} at the mean, {:.3e} at lies_at; Jeans × age ÷ reservoir {:.3e}; an Ar air \
         at p_sat collisional at the mean {}, at lies_at {}",
        record.identity().id(),
        case.kind,
        surface.state(),
        bulk.class(),
        mass.value(),
        radius.value() / 1e3,
        g.value(),
        ctx.age_at_epoch().value() / 1e9,
        coldest.value(),
        bulk.equilibrium_temperature().value(),
        if ice.lit { "lit" } else { "unlit" },
        entry.trapped_area_fraction(),
        entry.reservoir().value(),
        entry
            .reservoir_gel()
            .map_or("none".to_owned(), |gel| format!("{:.4e} m", gel.value())),
        if zonal { "zonal" } else { "locked" },
        if collisional_air { "" } else { "not " },
        if exosphere { "Exosphere" } else { "Partition" },
        argon_vapour_pa(warmest),
        argon_vapour_pa(mean),
        argon_vapour_pa(lies_at),
        ice.sublimed_warmest,
        ice.sublimed_mean,
        ice.sublimed_where_it_lies,
        ice.escaped,
        ice.collisional_at_mean,
        ice.collisional_where_it_lies,
    );
    Some(ArgonIce { line, ..ice })
}

/// P14.T49.n's census of the argon ice, a measurement run by name for its science check ("that
/// argon ice is retained on the airless bodies that hold it"; plan 14): over the 400 whole systems,
/// `for_body`'s refusals by cause at 23's rules and at all of 24's at once (the read densities, the
/// unlit mean floored, the compressed liquids and the cold traps' batch), and every record holding
/// an argon ice at 24's rules, its state, its climate, its ice, its air, the solid edge's rule
/// that applied, its dominance, its exobase and Jeans parameters, its ice's vapour pressure and the
/// sublimation and escape over its age against its reservoir ([`argon_ice_of`]'s estimates), with
/// the summary counts and what became at 24 of the records unlit at 23. It eprints and writes
/// nothing to the tree; it asserts only that 24's rules refuse no ice for its density.
#[test]
#[ignore = "a measurement for P14.T49.n's science check, run by name"]
#[expect(
    clippy::too_many_lines,
    reason = "one census at two rule sets, each argon-ice record listed and tallied"
)]
fn the_sample_s_argon_ices_under_24_s_rules() {
    use crate::planetary::chem::partition::CondensatePhase;
    use crate::planetary::derive::atmosphere::{UnlitMeanRules, with_unlit_mean};
    use crate::planetary::hooks::cold_traps::ColdTrapRules;
    use crate::planetary::surface::RECORD_TIME;
    use crate::substance::{
        CompressedLiquidsRules, ReadDensitiesRules, with_compressed_liquids, with_read_densities,
    };
    let ar = Gas::Argon.substance();
    let holds_ar = |record: &BodyRecord| {
        record.surface().ok().is_some_and(|surface| {
            surface
                .condensates()
                .entry(ar, CondensatePhase::Solid)
                .is_some()
        })
    };
    let ices_of = |record: &BodyRecord| {
        record.surface().ok().map_or(String::new(), |surface| {
            let keys: Vec<String> = surface
                .condensates()
                .entries()
                .iter()
                .filter(|e| e.phase() == CondensatePhase::Solid)
                .map(|e| e.substance().substance().key().as_str().to_owned())
                .collect();
            keys.join(" ")
        })
    };
    // 23's rules: every hold in force.
    let (mut bodies_23, mut refused_23) = (0_usize, std::collections::BTreeMap::new());
    let mut argon_23 = 0_usize;
    let mut unlit_23: Vec<String> = Vec::new();
    for (ctx, system) in whole() {
        let snapshot = system.snapshot_at(ctx, RECORD_TIME);
        for record in snapshot.bodies() {
            argon_23 += usize::from(holds_ar(record));
            let unlit = record.surface().ok().is_some_and(|surface| {
                surface.conditions().mean_surface_temperature().value() <= 0.0
            });
            if unlit {
                unlit_23.push(format!(
                    "{} (ices {})",
                    record.identity().id(),
                    ices_of(record)
                ));
            }
        }
        tally_refusals(&snapshot, ctx, &mut bodies_23, &mut refused_23);
    }
    // 24's rules, all at once: generation, the snapshot, the census's derivations and `for_body`
    // inside every switch, as the records derive lazily.
    let (bodies_24, refused_24, argon_24, ices, unlit_at_24) =
        with_read_densities(ReadDensitiesRules::Read, || {
            with_unlit_mean(UnlitMeanRules::Floored, || {
                with_compressed_liquids(CompressedLiquidsRules::Compressed, || {
                    let (mut bodies, mut refused) = (0_usize, std::collections::BTreeMap::new());
                    let mut argon = 0_usize;
                    let mut ices: Vec<ArgonIce> = Vec::new();
                    let mut unlit_at_24: Vec<String> = Vec::new();
                    for ctx in sample() {
                        let system = generate(SEED, ctx).with_record_rules(RecordRules {
                            cold_traps: ColdTrapRules::Batch,
                            ..RecordRules::in_force()
                        });
                        let snapshot = system.snapshot_at(ctx, RECORD_TIME);
                        tally_refusals(&snapshot, ctx, &mut bodies, &mut refused);
                        let mut any = false;
                        for record in snapshot.bodies() {
                            any |= holds_ar(record);
                            argon += usize::from(holds_ar(record));
                            let unlit = record
                                .bulk()
                                .ok()
                                .is_some_and(|bulk| bulk.equilibrium_temperature().value() <= 0.0);
                            if let (true, Some(surface)) = (unlit, record.surface().ok()) {
                                unlit_at_24.push(format!(
                                    "{}: {:?}, mean {:.3} K, ices {}",
                                    record.identity().id(),
                                    surface.state(),
                                    surface.conditions().mean_surface_temperature().value(),
                                    ices_of(record)
                                ));
                            }
                        }
                        if !any {
                            continue;
                        }
                        census_walk("sample", ctx, &system, &mut |case| {
                            if let Some(ice) = argon_ice_of(case, ctx) {
                                ices.push(ice);
                            }
                        });
                    }
                    (bodies, refused, argon, ices, unlit_at_24)
                })
            })
        });
    let ice_refusals = |refused: &std::collections::BTreeMap<String, usize>, key: &str| {
        refused
            .get(&format!("{key} Ice: DensityNotModelled"))
            .copied()
            .unwrap_or(0)
    };
    eprintln!(
        "(a) for_body over the sample's {bodies_23} solid bodies at 23's rules ({argon_23} with an \
         argon ice entry): refused {refused_23:?}; Ar ice {}, O2 ice {}",
        ice_refusals(&refused_23, "Ar"),
        ice_refusals(&refused_23, "O2"),
    );
    eprintln!(
        "(a) for_body over the sample's {bodies_24} solid bodies at 24's rules ({argon_24} with an \
         argon ice entry): refused {refused_24:?}; Ar ice {}, O2 ice {}",
        ice_refusals(&refused_24, "Ar"),
        ice_refusals(&refused_24, "O2"),
    );
    eprintln!(
        "(a) the {} records unlit at 23: {unlit_23:#?}",
        unlit_23.len()
    );
    eprintln!(
        "(a) the {} records with T_eq 0 at 24's rules: {unlit_at_24:#?}",
        unlit_at_24.len()
    );
    let edge = crate::planetary::chem::partition::exosphere_threshold(
        ar,
        crate::planetary::chem::partition::AIRLESS_ICE_LOSS_KG_PER_M2_GYR,
    )
    .expect("argon's row has a phase diagram and a molar mass")
    .value();
    eprintln!(
        "(b, c) the {} records holding an argon ice at 24's rules that the census derives (Ar's \
         vacuum edge at 144 kg m⁻² Gyr⁻¹: the partition's {edge:.3} K, this census's Hertz–Knudsen \
         {:.3} K):",
        ices.len(),
        argon_vacuum_edge_k()
    );
    for ice in &ices {
        eprintln!("  {}", ice.line);
    }
    let bands = [15.0, 19.0, 25.0, 35.0, 50.0];
    let mut by_band = [0_usize; 6];
    for ice in &ices {
        let band = bands.iter().take_while(|&&edge| ice.mean >= edge).count();
        by_band[band] += 1;
    }
    let count = |test: &dyn Fn(&ArgonIce) -> bool| ices.iter().filter(|ice| test(ice)).count();
    let warmest = ices
        .iter()
        .map(|ice| ice.mean)
        .fold(f64::NEG_INFINITY, f64::max);
    eprintln!(
        "(d) argon-ice records by mean T_s: <15 K {}, 15–19 K {}, 19–25 K {}, 25–35 K {}, 35–50 K \
         {}, >50 K {}; airless {}, under air {}; lit {}, unlit {}; edge Exosphere {}, Partition {} \
         ({} of them airless: {} locked, {} zonal under a collisional air); lying above Ar's \
         vacuum edge {} (Exosphere {}, Partition {}); HK × age > reservoir at the warmest {}, at \
         the mean {}, at lies_at {}; Jeans × age > reservoir {}; λ_Ar < 25 at the warmest surface \
         temperature {}; an Ar air at p_sat collisional at the mean {}, at lies_at {} (airless \
         {}); the warmest mean T_s {warmest:.2} K",
        by_band[0],
        by_band[1],
        by_band[2],
        by_band[3],
        by_band[4],
        by_band[5],
        count(&|ice| ice.airless),
        count(&|ice| !ice.airless),
        count(&|ice| ice.lit),
        count(&|ice| !ice.lit),
        count(&|ice| ice.exosphere),
        count(&|ice| !ice.exosphere),
        count(&|ice| !ice.exosphere && ice.airless),
        count(&|ice| !ice.exosphere && ice.airless && !ice.zonal),
        count(&|ice| !ice.exosphere && ice.airless && ice.zonal && ice.collisional_air),
        count(&|ice| ice.lies_at > edge),
        count(&|ice| ice.lies_at > edge && ice.exosphere),
        count(&|ice| ice.lies_at > edge && !ice.exosphere),
        count(&|ice| ice.sublimed_warmest > 1.0),
        count(&|ice| ice.sublimed_mean > 1.0),
        count(&|ice| ice.sublimed_where_it_lies > 1.0),
        count(&|ice| ice.escaped > 1.0),
        count(&|ice| ice.lambda_surface < 25.0),
        count(&|ice| ice.collisional_at_mean),
        count(&|ice| ice.collisional_where_it_lies),
        count(&|ice| ice.collisional_where_it_lies && ice.airless),
    );
    let mut warm: Vec<&ArgonIce> = ices.iter().collect();
    warm.sort_by(|a, b| b.mean.total_cmp(&a.mean));
    eprintln!("(d) the ten warmest argon-ice records:");
    for ice in warm.iter().take(10) {
        eprintln!("  {}", ice.line);
    }
    assert_eq!(ice_refusals(&refused_24, "Ar"), 0, "{refused_24:?}");
    assert_eq!(ice_refusals(&refused_24, "O2"), 0, "{refused_24:?}");
}

/// What P14.T56.f moves over a set of systems ([`MoonBulkMoves::visit`]).
#[derive(Debug, Default)]
struct MoonBulkMoves {
    /// Each moved moon's line: its R, T0, `t_hot`, d, water before and after, class and density.
    lines: Vec<String>,
    /// Moons made `Rocky`, and thinned but still `Icy`.
    rocky: u32,
    thinned_icy: u32,
    /// Moves by arm: captures formed inside the snow line, beside a rocky belt and with no
    /// neighbouring belt, and moons hot above the melting point.
    inside_rocky_belt: u32,
    inside_no_belt: u32,
    hot: u32,
    /// Hot ice bulks (T0 above the limit) left `Icy`, each with an `Ice` surface: a wrong state on
    /// the published surface, P14.T56.g's serve-gate must-fix (main, 2026-10-22).
    hot_icy: Vec<String>,
    /// Stripped bodies above 200 km: porosity against their kept bulk's grain and the uniform
    /// sphere's central pressure, (2π ÷ 3) G ρ² R².
    stripped_large: Vec<String>,
    /// Ice bulks at T0 of 200–273.15 K under 10 km, which the lag may empty over Gyr (G2), by their
    /// kept water.
    small_warm_ice: u32,
    /// Bodies not moons whose bulk is icy and whose `T_eq` is above the melting point (R1.6's
    /// "a hot ice bulk that is not a moon", for P14.T56.g).
    hot_icy_not_moons: Vec<String>,
    /// Records with an `Ice` surface whose T0 is above the melting point, after the arms.
    ice_surfaces_above_melting: u32,
    /// Every moon's bulk read; and records that moved beyond the moved moons (asserted none).
    moons: u32,
    other_moved: Vec<String>,
}

impl MoonBulkMoves {
    /// Adds the moves of the system `name` of context `ctx`, held `held` and under P14.T56.f's
    /// arms alone `batch` ([`MoonBulkRules::BulkOnly`](moon_ice::MoonBulkRules::BulkOnly), with no
    /// P14.T56.g lag).
    #[expect(
        clippy::too_many_lines,
        reason = "one walk of a system's records, held and under the batch, each classed and listed"
    )]
    fn visit(
        &mut self,
        name: &str,
        ctx: &SystemContext,
        (held, batch): (&PlanetarySystem, &PlanetarySystem),
    ) {
        use crate::planetary::hooks::figures::interior::ROCK_THERMAL_DIFFUSIVITY_M2_PER_S;
        use crate::planetary::surface::RECORD_TIME;
        use moon_ice::{HOT_ICE_LIMIT, MoonBulkRules, with_moon_bulk};

        let t = RECORD_TIME;
        if ctx.age_at(t).value() <= 0.0 {
            return;
        }
        let before = held.snapshot_at(ctx, t);
        with_moon_bulk(MoonBulkRules::BulkOnly, || {
            let after = batch.snapshot_at(ctx, t);
            let epoch = Epoch::new(batch, ctx, t).with_shades();
            let cosines = HourCosines::new();
            let mut moved_moons = Vec::new();
            for body in batch.bodies() {
                let id = format!("{name} {}", body.index.body_id(ctx.id()));
                let find = |records: &[BodyRecord]| {
                    records
                        .iter()
                        .find(|r| r.index() == body.index)
                        .and_then(|r| r.bulk().ok().copied())
                };
                let (Some(old), Some(new)) = (find(before.bodies()), find(after.bodies())) else {
                    continue;
                };
                let (OrbitHost::Body(parent), Part::Moon(part)) = (body.host, &body.part) else {
                    if new.class() == PlanetClass::Icy
                        && new.equilibrium_temperature() > HOT_ICE_LIMIT
                    {
                        self.hot_icy_not_moons.push(format!(
                            "{id}: T_eq {:.0} K, water {:.3}",
                            new.equilibrium_temperature().value(),
                            new.fractions().water()
                        ));
                    }
                    continue;
                };
                self.moons += 1;
                let primary = batch.body(parent).expect("a moon's parent is a body");
                let fate_host = fate_host(ctx, batch.zone_of(primary));
                let fate = batch.fate_of(primary, &fate_host);
                let at = fate.at(t);
                let now = batch.primary_now(&epoch, &cosines, primary, &fate, &at);
                let ice = now.moon_ice.and_then(|m| m.ice);
                let t0 = ice.map_or(f64::NAN, |ice| ice.t0(&part.orbit).value());
                let hot_gyr = ice.map_or(0.0, |ice| {
                    ice.history.hot_for(Kelvin::new(t0)).value() / 1e9
                });
                let r_km = Metres::from(new.radius()).value() / 1e3;
                let (w0, w1) = (old.fractions().water(), new.fractions().water());
                if w1 > 0.0 && r_km < 10.0 && (200.0..=HOT_ICE_LIMIT.value()).contains(&t0) {
                    self.small_warm_ice += 1;
                }
                let hot = t0 > HOT_ICE_LIMIT.value();
                if hot && new.class() == PlanetClass::Icy {
                    self.hot_icy.push(format!(
                        "{id}: {r_km:.0} km at T0 {t0:.0} K keeps {w1:.3}, {:?}",
                        new.class()
                    ));
                }
                let inside = match part.satellite.moon() {
                    SatelliteMoon::Captured(captured) => {
                        let inside = PlanetarySystem::capture_formed_inside(captured.kind(), &now);
                        // The bulk and the crust read one birthplace (§C3.2).
                        let craters = batch
                            .moon_craters(primary, part, 0.0, &part.orbit)
                            .expect("a capture's parent orbits its host");
                        let ImpactorOrbit::Moon { formed, .. } = craters.orbit else {
                            panic!("{id}: a moon's impactors are a moon's");
                        };
                        assert_eq!(formed == SnowLineSide::Inside, inside, "{id}");
                        inside
                    }
                    SatelliteMoon::Regular(_) | SatelliteMoon::GiantImpact(_) => false,
                };
                if format!("{old:?}") == format!("{new:?}") {
                    continue;
                }
                moved_moons.push(body.index);
                // A bulk moves only by one of the two arms: a capture formed inside the snow
                // line, or a moon hot above the melting point.
                assert!(inside || hot, "{id}: moved neither inside nor hot");
                if inside {
                    let rocky_belt = batch
                        .nearest_belt(primary.index)
                        .is_some_and(|belt| belt.adjacency() == BeltAdjacency::Neighbouring);
                    if rocky_belt {
                        self.inside_rocky_belt += 1;
                    } else {
                        self.inside_no_belt += 1;
                    }
                    assert_same_bits(w1, 0.0);
                } else {
                    self.hot += 1;
                }
                // A thinned regular moon's derivation reads the water its record states (G2
                // amendment 3): its own bulk keeps it, to the bit.
                if let SatelliteMoon::Regular(_) = part.satellite.moon() {
                    let orbit = part.satellite.orbit_at(&part.parent, ctx.age_at(t));
                    let own = PlanetarySystem::moon_own_bulk(part, &orbit, &now);
                    let MoonBulk::Kept { water, .. } = own else {
                        panic!("{id}: {own:?}");
                    };
                    assert_same_bits(water, w1);
                }
                match new.class() {
                    PlanetClass::Rocky => self.rocky += 1,
                    PlanetClass::Icy => self.thinned_icy += 1,
                    PlanetClass::GasGiant | PlanetClass::IceGiant | PlanetClass::SubNeptune => {}
                }
                let depth_km = (ROCK_THERMAL_DIFFUSIVITY_M2_PER_S
                    * hot_gyr
                    * 1e9
                    * crate::units::consts::SECONDS_PER_JULIAN_YEAR)
                    .sqrt()
                    / 1e3;
                let density = new.density().value();
                if w1 < w0 && r_km > 200.0 {
                    let pressure_mpa = 2.0 * PI / 3.0
                        * crate::units::consts::GRAVITATIONAL_CONSTANT
                        * density
                        * density
                        * (r_km * 1e3)
                        * (r_km * 1e3)
                        / 1e6;
                    // Its grain density, the kept ice's and the rock's without pore space.
                    let grain = 1.0
                        / (w1 / MOON_ICE_DENSITY.value() + (1.0 - w1) / MOON_ROCK_DENSITY.value());
                    self.stripped_large.push(format!(
                        "{id}: {r_km:.0} km at {density:.0} kg m⁻³, porosity {:.0}% against its \
                         grain's {grain:.0} kg m⁻³, central pressure {pressure_mpa:.0} MPa",
                        100.0 * (1.0 - density / grain),
                    ));
                }
                let kind = match part.satellite.moon() {
                    SatelliteMoon::Regular(_) => "regular".to_owned(),
                    SatelliteMoon::GiantImpact(_) => "giant impact".to_owned(),
                    SatelliteMoon::Captured(captured) => format!("{:?} capture", captured.kind()),
                };
                self.lines.push(format!(
                    "{id} ({kind}{}): R {r_km:.1} km, T0 {t0:.1} K, t_hot {hot_gyr:.3} Gyr, d \
                     {depth_km:.0} km, water {w0:.4} → {w1:.4}, {:?} → {:?}, {density:.0} kg m⁻³",
                    if inside { ", formed inside" } else { "" },
                    old.class(),
                    new.class(),
                ));
            }
            // Every other record is unchanged to the bit, and an Ice surface above the melting
            // point is counted.
            for (old, new) in before.bodies().iter().zip(after.bodies()) {
                if let (Some(surface), Some(bulk)) = (new.surface().ok(), new.bulk().ok())
                    && surface.material() == SurfaceMaterial::Ice
                    && bulk.equilibrium_temperature() > HOT_ICE_LIMIT
                {
                    self.ice_surfaces_above_melting += 1;
                }
                if !moved_moons.contains(&new.index()) && format!("{old:?}") != format!("{new:?}") {
                    self.other_moved
                        .push(format!("{name} {}", new.identity().id()));
                }
            }
        });
    }
}

/// P14.T56.f's tests over the golden systems (decision-p14-t54a-ganymede §C3.2;
/// decision-bump-plan-after-22 R1.6 and G2, with main's corrections, 2026-10-22):
///
/// - the two small captures beside rocky belts (`close_binary` 0602 and `filler_c` 0201) are
///   `Rocky` with no water, their bulk and crust one birthplace;
/// - the hot ice bulks of `wide_binary` 0101–0104, `subgiant` 0201–0304 and `red_giant` 0505–0508
///   lose their ice; `subgiant` 0101 and `red_giant` 0503 and 0504 are thinned and stay `Icy`;
/// - every moved moon moved by one of the two arms, and is listed with its R, T0, `t_hot`, d and
///   water; every other record is unchanged to the bit: the cold irregulars (R1.6 moves a giant's
///   hot ones, so the plan's "a giant's irregulars, unchanged" reads "below the melting point"),
///   `eccentric_giant` 0101–0103 among them.
///
/// Under P14.T56.f's arms alone (`BulkOnly`), so that P14.T56.g's lag, which moves `red_giant`
/// e202, e204 and e206's surfaces too, is attributed to its own subtask (decision-p14-t56g §2).
#[test]
fn a_moon_s_bulk_reads_its_birthplace_and_its_ice_s_history() {
    use moon_ice::{MoonBulkRules, with_moon_bulk};
    let batch = with_moon_bulk(MoonBulkRules::BulkOnly, golden_systems_afresh);
    let mut moves = MoonBulkMoves::default();
    for ((name, ctx, held), (_, _, batch)) in golden_systems().iter().zip(&batch) {
        moves.visit(name, ctx, (held, batch));
    }
    eprintln!(
        "the golden systems' {} moons: {} moved:\n  {}\n{moves:#?}",
        moves.moons,
        moves.lines.len(),
        moves.lines.join("\n  ")
    );
    for capture in [
        "close_binary 42002cb200000009.0602",
        "filler_c 42002cb200000000.0201",
    ] {
        assert!(
            moves
                .lines
                .iter()
                .any(|line| line.starts_with(capture) && line.contains("formed inside")),
            "{capture} is rock by its birthplace"
        );
    }
    for unchanged in ["0101", "0102", "0103"] {
        assert!(
            !moves
                .lines
                .iter()
                .any(|line| line.starts_with("eccentric_giant")
                    && line.contains(&format!(".{unchanged} "))),
            "eccentric_giant {unchanged} keeps its bulk"
        );
    }
    // G2's expected, as main corrected it: 18 made Rocky (2 by birthplace, 16 hot), and 3 hot ice
    // bulks left Icy (subgiant 0101 at 0.113, red_giant 0503 and 0504 at 0.124 and 0.123).
    assert_eq!(
        (moves.rocky, moves.thinned_icy),
        (18, 3),
        "{:#?}",
        moves.lines
    );
    assert_eq!(
        (moves.inside_rocky_belt + moves.inside_no_belt, moves.hot),
        (2, 19)
    );
    assert_eq!(moves.hot_icy.len(), 3, "{:?}", moves.hot_icy);
    assert_eq!(moves.other_moved, Vec::<String>::new());
}

/// P14.T56.f's census over the 400 whole systems, a measurement run by name: the moons it moves,
/// by class and by arm (the birthplace arm beside a rocky belt and with no neighbouring belt), the
/// hot ice bulks it leaves `Icy` (each to main at the gate, G2 item 10, and an `Ice` surface on
/// them P14.T56.g's must-fix), the stripped bodies above 200 km, the small warm ice bulks, the hot
/// ice bulks that are not moons, and the `Ice` surfaces above the melting point; no record moves
/// beyond the moved moons. Under P14.T56.f's arms alone (`BulkOnly`), as its golden twin.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn a_moon_s_bulk_reads_its_birthplace_over_the_sample() {
    use moon_ice::{MoonBulkRules, with_moon_bulk};
    let mut moves = MoonBulkMoves::default();
    for ctx in sample() {
        let held = generate(SEED, ctx);
        let batch = with_moon_bulk(MoonBulkRules::BulkOnly, || generate(SEED, ctx));
        moves.visit("sample", ctx, (&held, &batch));
    }
    eprintln!(
        "the sample's {} moons: {} moved:\n  {}\n{moves:#?}",
        moves.moons,
        moves.lines.len(),
        moves.lines.join("\n  ")
    );
    assert!(moves.moons > 1_000, "{}", moves.moons);
    assert_eq!(moves.other_moved, Vec::<String>::new());
}

/// The class of the first star of `hosts`, [`zone_hosts`]'s "phase, effective temperature" list:
/// a main-sequence star's spectral class by its effective temperature, at Pecaut and Mamajek's
/// (2013, `ApJS` 208, 9) dwarf zero points (B0 31,400 K, A0 9,700 K, F0 7,220 K, G0 5,930 K, K0
/// 5,270 K), an M dwarf below the escape's `M_DWARF_EDGE`, 3,900 K; or "pre-main-sequence",
/// "evolved", "substellar" (plan 06's `Substellar`, the latest M dwarfs among them) or "remnant".
fn host_class(hosts: &str) -> String {
    let first = hosts.split(" + ").next().unwrap_or_default();
    let mut words = first.split_whitespace();
    let phase = words.next().unwrap_or_default();
    let t: f64 = words
        .next()
        .and_then(|t| t.parse().ok())
        .unwrap_or(f64::NAN);
    let class = match phase {
        "MainSequence" if t >= 31_400.0 => "main-sequence O",
        "MainSequence" if t >= 9_700.0 => "main-sequence B",
        "MainSequence" if t >= 7_220.0 => "main-sequence A",
        "MainSequence" if t >= 5_930.0 => "main-sequence F",
        "MainSequence" if t >= 5_270.0 => "main-sequence G",
        "MainSequence" if t >= 3_900.0 => "main-sequence K",
        "MainSequence" => "M dwarf",
        "Protostar" | "PreMainSequence" => "pre-main-sequence",
        "Substellar" => "substellar",
        "HeliumWhiteDwarf"
        | "CarbonOxygenWhiteDwarf"
        | "OxygenNeonWhiteDwarf"
        | "NeutronStar"
        | "BlackHole"
        | "NoRemnant" => "remnant",
        "unformed" => "unformed",
        _ => "evolved",
    };
    class.to_owned()
}

/// One body's figures as [`under_the_escape_batch_the_census_is_compared_body_by_body`] compares
/// them: its record's and its derivation's at the epoch, under one set of rules.
#[derive(Debug, Clone, PartialEq)]
struct EscapeCensusBody {
    /// "sample", or the golden system's name.
    set: String,
    /// The census's key ([`CensusCase::key`]).
    key: String,
    /// The body's ID, as its record gives it.
    id: String,
    /// Its index in its system.
    index: u16,
    /// "planets", "members" or "moons".
    kind: &'static str,
    /// Its zone's stars at the epoch.
    hosts: String,
    /// Its zone's first star's class ([`host_class`]).
    host_class: String,
    /// Its record's surface state.
    state: SurfaceState,
    /// Its record's mean surface temperature, K.
    t_s: f64,
    /// Its record's surface pressure, Pa.
    p: f64,
    /// Its derivation's equilibrium temperature, K.
    t_eq: f64,
    /// Its record's ice fraction, 0 to 1.
    ice: f64,
    /// Its record's coldest zone, the colder end of its climate range, K.
    coldest: f64,
    /// Carbon dioxide's mole fraction in its record's air, and whether it is the air's largest.
    co2: (f64, bool),
    /// Carbon dioxide's frost point at its partial pressure, its mole fraction of the surface
    /// pressure, K, where it has one.
    co2_frost: Option<f64>,
    /// Its record's carbon dioxide ice: its area fraction and its reservoir, kg m⁻².
    co2_ice: Option<(f64, f64)>,
    /// Its derivation's partial pressures (column weights), bar, of each gas with any.
    air: String,
    /// The greenhouse's terms under the rules in force, each absorber's depth.
    terms: String,
    /// Its escape, the batch's alone: the shoreline, the weathering thermostat's airborne share of
    /// the carbon, and the oxygen of its water the air keeps, kg.
    escape: Option<(crate::planetary::chem::escape::Shoreline, f64, f64)>,
    /// Its derivation's atmosphere passes.
    passes: u32,
    /// Its record's warmest zone, the warmer end of its climate range, K.
    warmest: f64,
    /// The maximum-greenhouse ceiling its grey surface is held under (P14.T53.a).
    ceiling: crate::planetary::derive::atmosphere::GreenhouseCeiling,
    /// The ceiling's temperature, max(273.15 K, `T_rest`), `T_rest` its nitrogen's and hydrogen's
    /// grey surface, K.
    cap: f64,
    /// How its derivation's passes ended.
    convergence: crate::planetary::derive::passes::Convergence,
}

impl EscapeCensusBody {
    /// The figures of the body `case`, under the rules in force.
    fn of(case: &CensusCase<'_>) -> Self {
        use crate::planetary::chem::partition::CondensatePhase;
        use crate::planetary::derive::atmosphere::GreenhouseConstants;
        let (surface, derived) = (case.surface, case.derived);
        let conditions = surface.conditions();
        let p = conditions.surface_pressure().value();
        let carbon_dioxide = Gas::CarbonDioxide.substance();
        let fraction = conditions
            .gases()
            .iter()
            .find(|&&(id, _)| id == carbon_dioxide)
            .map_or(0.0, |&(_, x)| x);
        let dominant = conditions
            .gases()
            .first()
            .is_some_and(|&(id, _)| id == carbon_dioxide);
        let co2_frost = carbon_dioxide
            .substance()
            .phase_diagram()
            .and_then(|diagram| {
                diagram.saturation_temperature(crate::units::Pascals::new(fraction * p))
            })
            .map(Kelvin::value);
        let co2_ice = surface
            .condensates()
            .entry(carbon_dioxide, CondensatePhase::Solid)
            .map(|entry| (entry.area_fraction(), entry.reservoir().value()));
        let partials = derived.atmosphere().partial_pressures();
        let air = Gas::ALL
            .into_iter()
            .filter(|&gas| partials.of(gas).value() > 0.0)
            .map(|gas| format!("{gas:?} {:.4e}", partials.of(gas).value() / 1e5))
            .collect::<Vec<_>>()
            .join(", ");
        let grey_terms = GreenhouseConstants::in_force().terms(
            partials,
            derived.equilibrium_temperature(),
            derived.surface_gravity(),
        );
        let terms = grey_terms
            .iter()
            .map(|term| format!("{:?} {:.4}", term.absorber(), term.depth()))
            .collect::<Vec<_>>()
            .join(", ");
        let rest = grey_terms
            .iter()
            .filter(|term| {
                matches!(
                    term.absorber(),
                    crate::planetary::derive::atmosphere::Absorber::Nitrogen
                        | crate::planetary::derive::atmosphere::Absorber::Hydrogen
                )
            })
            .fold(0.0, |sum, term| sum + term.depth());
        let cap = crate::planetary::derive::atmosphere::grey_surface_temperature(
            derived.equilibrium_temperature(),
            rest,
        )
        .value()
        .max(crate::planetary::derive::atmosphere::WATER_MELTING_POINT.value());
        let escape = derived.atmosphere().escape().map(|escape| {
            (
                *escape.shoreline(),
                escape.airborne_carbon_share(),
                escape.oxygen().kept().value(),
            )
        });
        Self {
            set: case.set.to_owned(),
            key: case.key.clone(),
            id: case.record.identity().id().to_string(),
            index: case.record.index().get(),
            kind: case.kind,
            hosts: case.hosts.clone(),
            host_class: host_class(&case.hosts),
            state: surface.state(),
            t_s: conditions.mean_surface_temperature().value(),
            p,
            t_eq: derived.equilibrium_temperature().value(),
            ice: surface.figures().ice_fraction(),
            coldest: conditions.climate_range().0.value(),
            co2: (fraction, dominant),
            co2_frost,
            co2_ice,
            air,
            terms,
            escape,
            passes: derived.atmosphere_passes(),
            warmest: conditions.climate_range().1.value(),
            ceiling: derived.atmosphere().greenhouse_ceiling(),
            cap,
            convergence: derived.convergence(),
        }
    }

    /// What water its record's warmest zone holds under its surface pressure.
    fn water(&self) -> crate::planetary::derive::atmosphere::WaterPhase {
        crate::planetary::derive::atmosphere::water_phase(
            Kelvin::new(self.warmest),
            crate::units::Pascals::new(self.p),
        )
    }

    /// Its name: its ID, after its golden system's name and its index for a golden.
    fn label(&self) -> String {
        if self.set == "sample" {
            format!("{} {}", self.id, self.kind)
        } else {
            format!(
                "{} {:04x} ({}) {}",
                self.set, self.index, self.id, self.kind
            )
        }
    }

    /// Whether it holds no air: `Airless`, or under [`AIRLESS_PRESSURE`](crate::planetary::derive::atmosphere::AIRLESS_PRESSURE).
    fn airless(&self) -> bool {
        self.state == SurfaceState::Airless
            || self.p < crate::planetary::derive::atmosphere::AIRLESS_PRESSURE.value()
    }

    /// Whether it is a Mars-like world on the warm branch: a `Temperate` world of 0.05–1 bar whose
    /// largest gas is carbon dioxide, whose frost point at its partial pressure lies below the
    /// coldest zone, so that no cold trap forms.
    fn mars_like_warm(&self) -> bool {
        self.state == SurfaceState::Temperate
            && self.co2.1
            && (5e3..=1e5).contains(&self.p)
            && self.co2_frost.is_some_and(|frost| frost < self.coldest)
    }

    /// The shoreline's cause of its loss: "impact" where the impact term's share (its impact share
    /// times its chronology) is the larger and at least a half, "XUV" where the X-ray and
    /// ultraviolet term's is, "other" where neither takes half.
    fn loss_cause(&self) -> &'static str {
        match self.escape {
            Some((shoreline, ..)) => {
                let impact = shoreline.impact() * shoreline.chronology();
                let xuv = shoreline.xuv();
                if impact.max(xuv) < 0.5 {
                    "other"
                } else if impact > xuv {
                    "impact"
                } else {
                    "XUV"
                }
            }
            None => "no escape outcome",
        }
    }

    /// One line of its figures.
    fn describe(&self) -> String {
        let escape = self.escape.map_or_else(
            || "none".to_owned(),
            |(shoreline, share, oxygen)| {
                format!(
                    "XUV {:.4}, impact {:.4} × chronology {:.4}, kept {:.4e}, airborne C share \
                     {share:.4e}, O2 kept {oxygen:.3e} kg",
                    shoreline.xuv(),
                    shoreline.impact(),
                    shoreline.chronology(),
                    shoreline.kept(),
                )
            },
        );
        let frost = self
            .co2_frost
            .map_or_else(|| "none".to_owned(), |t| format!("{t:.2} K"));
        let ice = self.co2_ice.map_or_else(
            || "none".to_owned(),
            |(area, reservoir)| format!("area {area:.4}, {reservoir:.4e} kg m⁻²"),
        );
        format!(
            "{:?} {:.2} K, {:.4e} bar, T_eq {:.2} K, ice {:.3}, coldest {:.2} K, CO2 x {:.4}{} \
             (frost {frost}), CO2 ice {ice}; air (bar) [{}]; τ [{}]; escape {escape}; {} passes",
            self.state,
            self.t_s,
            self.p / 1e5,
            self.t_eq,
            self.ice,
            self.coldest,
            self.co2.0,
            if self.co2.1 { ", the largest" } else { "" },
            self.air,
            self.terms,
            self.passes,
        )
    }
}

/// One census run under one set of rules ([`under_the_escape_batch_the_census_is_compared_body_by_body`]).
#[derive(Debug)]
struct EscapeCensusRun {
    /// The systems, in the census's order.
    systems: Vec<PlanetarySystem>,
    /// The bodies, in the census's order.
    bodies: Vec<EscapeCensusBody>,
    /// The systems' generation's wall time, ms.
    generate_ms: f64,
    /// The census walk's wall time, its records' derivations, ms.
    walk_ms: f64,
    /// What the batch's escape counted over the generation.
    generated: crate::planetary::derive::atmosphere::ThermostatCounts,
    /// What it counted over the walk.
    walked: crate::planetary::derive::atmosphere::ThermostatCounts,
    /// The partition's solves and the greenhouses' steps over the generation.
    generated_exits: crate::planetary::chem::partition::ExitCounts,
    /// And over the walk.
    walked_exits: crate::planetary::chem::partition::ExitCounts,
}

impl EscapeCensusRun {
    /// One line of its counts over the generation and one over the walk: the passes and the wall
    /// time; the greenhouses and their steps (each step's evaluation and the last), the solves
    /// the steps' bit-exact exit did not stop, by kind (each atmosphere's first, the thermostat's
    /// all-airborne states, its trials and its states taken at the edge, and the cold traps' and
    /// methane reservoirs' solves again); the partition's solves and their sweeps; the step climates
    /// read; and the thermostat's outcomes.
    fn counts_line(&self, label: &str) -> String {
        let line =
            |what: &str,
             exits: &crate::planetary::chem::partition::ExitCounts,
             counts: &crate::planetary::derive::atmosphere::ThermostatCounts| {
                let per = |a: u64, b: u64| {
                    u32::try_from(a).map_or(f64::NAN, f64::from)
                        / u32::try_from(b).map_or(f64::NAN, f64::from)
                };
                let first = exits.greenhouses - counts.extra_solves();
                let first_steps = exits.evaluations
                    - counts.whole_steps
                    - counts.trial_steps
                    - counts.edge_steps
                    - counts.resolve_steps;
                format!(
                    "{what}: greenhouses {} ({} steps, {:.3} each, {} not settled); first solves \
                 {first} ({first_steps} steps, {:.3} each); the thermostat's all-airborne \
                 {} ({} steps), trials {} ({} steps), edges {} ({} steps, {} of them a trial \
                 solved again, {} of those hot in full); solves again {} ({} steps); partition \
                 solves {} ({:.3} sweeps each), climates {}; searched {}, frozen {}, too cold {}, \
                 hot {}, cold ends {}; \
                 edges frozen {}, below the set point by {:.3e} K at most, above by {:.3e} K",
                    exits.greenhouses,
                    exits.evaluations,
                    per(exits.evaluations, exits.greenhouses),
                    exits.capped,
                    per(first_steps, first),
                    counts.whole,
                    counts.whole_steps,
                    counts.trials,
                    counts.trial_steps,
                    counts.edges,
                    counts.edge_steps,
                    counts.retaken,
                    counts.retaken_hot,
                    counts.cold_trap + counts.methane,
                    counts.resolve_steps,
                    exits.solves,
                    per(exits.sweeps, exits.solves),
                    exits.climates,
                    counts.searched,
                    counts.frozen,
                    counts.too_cold,
                    counts.hot,
                    counts.cold_end,
                    counts.taken_frozen,
                    counts.taken_below,
                    counts.taken_above,
                )
            };
        format!(
            "COUNTS {label}: passes {}, generate {:.1} ms, walk {:.1} ms\n  {}\n  {}",
            self.bodies.iter().map(|b| b.passes).sum::<u32>(),
            self.generate_ms,
            self.walk_ms,
            line("generate", &self.generated_exits, &self.generated),
            line("walk", &self.walked_exits, &self.walked),
        )
    }
}

/// P14.T53.a's sample census under the escape's batch (spec C4), a measurement for its record:
/// every present planet, member and moon with a surface of the first 60 sample systems and of the
/// 15 golden systems at the epoch ([`census_walk`]), each system generated and derived afresh
/// under the hold and under the batch ([`with_rules`](crate::planetary::chem::escape::with_rules)),
/// compared body by body. [`whole`]'s and [`golden_systems`]'s systems are cached under the hold,
/// so they are only checked against the hold's fresh ones (asserted). Stated:
///
/// - the bodies the shoreline's impact term and its X-ray and ultraviolet term make airless, by
///   their hosts' class ([`host_class`]);
/// - every body whose state or air moves: a state change, or a surface pressure moved by more than
///   10% or across 100 Pa;
/// - the moved `Temperate` worlds, bless note 23's iced ones (ices ≥ 0.5) first, with the weathering
///   thermostat's airborne share;
/// - the hot `Snowball`s (`T_s` ≥ 273.15 K) under either rules, by their cure under the batch: held
///   cold, still hot at 10 bar or more (P14.T24.h.a's Rayleigh floor), or neither;
/// - the Mars-like worlds on the warm branch ([`EscapeCensusBody::mars_like_warm`]), and the carbon
///   dioxide ice of the moved worlds;
/// - the bodies that flip from a cold branch (below 200 K) to a hot one (above 400 K);
/// - the thermostat's extra greenhouse solves, and the generation's and the walk's wall time under
///   each rules, the medians of three alternated rounds (provisional, the machine's load stated
///   beside the run);
/// - the work by kind under each rules ([`EscapeCensusRun::counts_line`]), and under the batch with
///   the thermostat off (`with_thermostat`), run after the timed rounds: the Temperates the
///   thermostat raises, their passes with it and without, and their warmest zones' distance from
///   its set point;
/// - decision-p14-t54a-ganymede C1.3's six O₂ airs;
/// - every body of at least 0.5 bar under the hold left under 100 Pa by the batch, with its cause;
/// - the maximum-greenhouse ceiling's bodies;
/// - the Temperates the thermostat raises, each with how its passes end under the batch, converged
///   or bracketed with its residual, any jump named, and its passes against the hold's
///   (decision-24-cost).
#[test]
#[ignore = "a measurement for P14.T53.a's record, run by name"]
#[expect(
    clippy::too_many_lines,
    reason = "one census's comparison across the spec's nine statements, each listed"
)]
fn under_the_escape_batch_the_census_is_compared_body_by_body() {
    use std::collections::BTreeMap;
    use std::time::Instant;

    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    use crate::planetary::chem::partition::{Exits, with_exits};
    use crate::planetary::derive::atmosphere::{
        TEMPERATE_AIRBORNE_CARBON, Thermostat, with_thermostat,
    };

    let goldens = golden_systems();
    let sets: Vec<(&str, Seed, &SystemContext)> = sample()
        .iter()
        .take(60)
        .map(|ctx| ("sample", SEED, ctx))
        .chain(
            goldens
                .iter()
                .map(|(name, ctx, _)| (*name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx)),
        )
        .collect();
    let run_with = |rules: EscapeRules, thermostat: Thermostat| {
        with_rules(rules, || {
            let start = Instant::now();
            let ((systems, generated), generated_exits) = with_exits(Exits::Taken, || {
                with_thermostat(thermostat, || {
                    sets.iter()
                        .map(|&(_, seed, ctx)| generate(seed, ctx))
                        .collect::<Vec<_>>()
                })
            });
            let generate_ms = start.elapsed().as_secs_f64() * 1e3;
            let start = Instant::now();
            let ((bodies, walked), walked_exits) = with_exits(Exits::Taken, || {
                with_thermostat(thermostat, || {
                    let mut bodies = Vec::new();
                    for (&(set, _, ctx), system) in sets.iter().zip(&systems) {
                        census_walk(set, ctx, system, &mut |case| {
                            bodies.push(EscapeCensusBody::of(case));
                        });
                    }
                    bodies
                })
            });
            let walk_ms = start.elapsed().as_secs_f64() * 1e3;
            EscapeCensusRun {
                systems,
                bodies,
                generate_ms,
                walk_ms,
                generated,
                walked,
                generated_exits,
                walked_exits,
            }
        })
    };
    let run = |rules: EscapeRules| run_with(rules, Thermostat::On);

    // Three rounds, alternated; the first of each rules is the one compared, and the others must
    // give it again.
    let mut times: BTreeMap<EscapeRules, Vec<(f64, f64)>> = BTreeMap::new();
    let mut first: BTreeMap<EscapeRules, EscapeCensusRun> = BTreeMap::new();
    for order in [
        [EscapeRules::Held, EscapeRules::Batch],
        [EscapeRules::Batch, EscapeRules::Held],
        [EscapeRules::Held, EscapeRules::Batch],
    ] {
        for rules in order {
            let again = run(rules);
            eprintln!(
                "ROUND {rules:?}: generate {:.1} ms, walk {:.1} ms, {} bodies",
                again.generate_ms,
                again.walk_ms,
                again.bodies.len()
            );
            times
                .entry(rules)
                .or_default()
                .push((again.generate_ms, again.walk_ms));
            if let Some(known) = first.get(&rules) {
                let differing: Vec<String> = known
                    .bodies
                    .iter()
                    .zip(&again.bodies)
                    .filter(|(a, b)| a != b)
                    .map(|(a, _)| a.label())
                    .collect();
                assert_eq!(differing, Vec::<String>::new(), "{rules:?} again");
                assert_eq!(known.bodies.len(), again.bodies.len(), "{rules:?} again");
            } else {
                first.insert(rules, again);
            }
        }
    }
    let (held, batch) = (&first[&EscapeRules::Held], &first[&EscapeRules::Batch]);
    // The work by kind under each, and under the batch with the thermostat off, for its share:
    // the Temperates it moves, their passes with it and without, and their warmest zones' distance
    // from its set point.
    let off = run_with(EscapeRules::Batch, Thermostat::Off);
    let set = crate::planetary::derive::atmosphere::THERMOSTAT_SET_POINT.value();
    let thermostat_moved: Vec<(&EscapeCensusBody, &EscapeCensusBody)> = batch
        .bodies
        .iter()
        .zip(&off.bodies)
        .filter(|(b, _)| {
            b.state == SurfaceState::Temperate
                && b.escape.is_some_and(|(_, share, _)| {
                    share > TEMPERATE_AIRBORNE_CARBON * (1.0 + 1e-9) && share < 1.0
                })
        })
        .collect();
    let (below, above) = thermostat_moved
        .iter()
        .fold((0.0_f64, 0.0_f64), |(below, above), (b, _)| {
            (below.max(set - b.warmest), above.max(b.warmest - set))
        });
    let passes_of = |pick: fn(&(&EscapeCensusBody, &EscapeCensusBody)) -> u32| {
        thermostat_moved.iter().map(pick).sum::<u32>()
    };
    eprintln!(
        "{}\n{}\n{}\n  the Temperates the thermostat raises: {}, {} passes with it and {} without \
         ({}), their warmest zones below the set point by {below:.3e} K at most, above by \
         {above:.3e} K",
        held.counts_line("Held"),
        batch.counts_line("Batch"),
        off.counts_line("Batch, the thermostat off"),
        thermostat_moved.len(),
        passes_of(|(b, _)| b.passes),
        passes_of(|(_, o)| o.passes),
        thermostat_moved
            .iter()
            .map(|(b, o)| format!("{} {} → {}", b.label(), b.passes, o.passes))
            .collect::<Vec<_>>()
            .join("; "),
    );

    // The cached systems are the hold's.
    let cached: Vec<&PlanetarySystem> = whole()
        .iter()
        .take(60)
        .map(|(_, system)| system)
        .chain(goldens.iter().map(|(_, _, system)| system))
        .collect();
    let stale: Vec<String> = cached
        .iter()
        .zip(&held.systems)
        .zip(&sets)
        .filter(|((cached, fresh), _)| **cached != *fresh)
        .map(|(_, (set, _, ctx))| format!("{set} {:?}", ctx.id()))
        .collect();
    assert_eq!(stale, Vec::<String>::new(), "cached systems not the hold's");
    let generation_moved: Vec<String> = held
        .systems
        .iter()
        .zip(&batch.systems)
        .zip(&sets)
        .filter(|((a, b), _)| a != b)
        .map(|(_, (set, _, ctx))| format!("{set} {:?}", ctx.id()))
        .collect();

    // The bodies, joined by set and key.
    let by_key = |run: &EscapeCensusRun| -> BTreeMap<(String, String), usize> {
        run.bodies
            .iter()
            .enumerate()
            .map(|(i, body)| ((body.set.clone(), body.key.clone()), i))
            .collect()
    };
    let (held_keys, batch_keys) = (by_key(held), by_key(batch));
    let pairs: Vec<(&EscapeCensusBody, &EscapeCensusBody)> = held
        .bodies
        .iter()
        .filter_map(|h| {
            batch_keys
                .get(&(h.set.clone(), h.key.clone()))
                .map(|&i| (h, &batch.bodies[i]))
        })
        .collect();
    let unmatched: Vec<String> = held
        .bodies
        .iter()
        .filter(|h| !batch_keys.contains_key(&(h.set.clone(), h.key.clone())))
        .map(|h| format!("under the hold only: {}", h.label()))
        .chain(
            batch
                .bodies
                .iter()
                .filter(|b| !held_keys.contains_key(&(b.set.clone(), b.key.clone())))
                .map(|b| format!("under the batch only: {}", b.label())),
        )
        .collect();
    let moved = |h: &EscapeCensusBody, b: &EscapeCensusBody| {
        let airless_line = crate::planetary::derive::atmosphere::AIRLESS_PRESSURE.value();
        h.state != b.state
            || (h.p < airless_line) != (b.p < airless_line)
            || (b.p - h.p).abs() > 0.1 * h.p
    };
    let pair_line = |h: &EscapeCensusBody, b: &EscapeCensusBody| {
        format!(
            "{} [{}; {}]\n      hold:  {}\n      batch: {}",
            h.label(),
            h.host_class,
            h.hosts,
            h.describe(),
            b.describe()
        )
    };
    let list = |items: &[String]| items.join("\n  ");
    let count = |n: usize| u32::try_from(n).map_or(f64::NAN, f64::from);

    eprintln!(
        "bodies: {} under the hold, {} under the batch, {} joined; unmatched {}:\n  {}",
        held.bodies.len(),
        batch.bodies.len(),
        pairs.len(),
        unmatched.len(),
        list(&unmatched)
    );
    eprintln!(
        "systems whose generation the batch moves: {} of {}:\n  {}",
        generation_moved.len(),
        sets.len(),
        list(&generation_moved)
    );
    for &(h, b) in &pairs {
        eprintln!(
            "BODY | {} | {} | {} | {:?} → {:?} | {:.3} → {:.3} K | {:.6e} → {:.6e} Pa | {} | {}",
            h.set,
            h.label(),
            h.host_class,
            h.state,
            b.state,
            h.t_s,
            b.t_s,
            h.p,
            b.p,
            if moved(h, b) { "moved" } else { "same" },
            b.loss_cause(),
        );
    }

    // 1. The bodies the shoreline makes airless, by host class.
    let mut airless_by_class: BTreeMap<String, [u32; 4]> = BTreeMap::new();
    let mut made_airless = Vec::new();
    for &(h, b) in &pairs {
        if h.airless() || !b.airless() {
            continue;
        }
        let cause = b.loss_cause();
        let entry = airless_by_class.entry(h.host_class.clone()).or_default();
        let both = b
            .escape
            .is_some_and(|(s, ..)| s.xuv() >= 0.99 && s.impact() * s.chronology() >= 0.99);
        match cause {
            "impact" => entry[0] += 1,
            "XUV" => entry[1] += 1,
            _ => entry[2] += 1,
        }
        if both {
            entry[3] += 1;
        }
        made_airless.push(format!("({cause}) {}", pair_line(h, b)));
    }
    eprintln!(
        "1. made airless (air under the hold, none under the batch), by host class [impact, XUV, \
         other, both terms ≥ 0.99]: {airless_by_class:?}; {}:\n  {}",
        made_airless.len(),
        list(&made_airless)
    );

    // 2. Every body whose state or air moves.
    let mut transitions: BTreeMap<String, u32> = BTreeMap::new();
    let mut moved_lines = Vec::new();
    for &(h, b) in &pairs {
        if !moved(h, b) {
            continue;
        }
        *transitions
            .entry(format!("{:?} → {:?}", h.state, b.state))
            .or_default() += 1;
        moved_lines.push(pair_line(h, b));
    }
    eprintln!(
        "2. moved (a state change, or p_s by more than 10% or across 100 Pa): {} of {}; by \
         transition {transitions:?}:\n  {}",
        moved_lines.len(),
        pairs.len(),
        list(&moved_lines)
    );

    // 3. The moved Temperates, the iced ones first.
    let share = |b: &EscapeCensusBody| b.escape.map_or(f64::NAN, |(_, share, _)| share);
    let temperate_line = |h: &EscapeCensusBody, b: &EscapeCensusBody| {
        format!(
            "{} [{}]: hold {:?} {:.2} K, ice {:.3}, {:.4e} bar → batch {:?} {:.2} K, ice {:.3}, \
             {:.4e} bar, airborne C share {:.4e}{}",
            h.label(),
            h.host_class,
            h.state,
            h.t_s,
            h.ice,
            h.p / 1e5,
            b.state,
            b.t_s,
            b.ice,
            b.p / 1e5,
            share(b),
            if moved(h, b) { "" } else { " (not moved)" }
        )
    };
    let iced: Vec<String> = pairs
        .iter()
        .filter(|(h, _)| h.state == SurfaceState::Temperate && h.ice >= 0.5)
        .map(|&(h, b)| temperate_line(h, b))
        .collect();
    let other_temperates: Vec<String> = pairs
        .iter()
        .filter(|(h, b)| {
            (h.state == SurfaceState::Temperate || b.state == SurfaceState::Temperate)
                && !(h.state == SurfaceState::Temperate && h.ice >= 0.5)
                && moved(h, b)
        })
        .map(|&(h, b)| temperate_line(h, b))
        .collect();
    eprintln!(
        "3. the iced Temperates under the hold (ices ≥ 0.5): {}:\n  {}\n   the other moved \
         Temperates (under either rules): {}:\n  {}",
        iced.len(),
        list(&iced),
        other_temperates.len(),
        list(&other_temperates)
    );

    // 4. The hot Snowballs by cure.
    let hot_snowball = |x: &EscapeCensusBody| x.state == SurfaceState::Snowball && x.t_s >= 273.15;
    let mut cures: BTreeMap<&str, u32> = BTreeMap::new();
    let mut hot = Vec::new();
    for &(h, b) in &pairs {
        if !(hot_snowball(h) || hot_snowball(b)) {
            continue;
        }
        let cure = if b.t_s < 273.15 {
            "held cold under the batch"
        } else if b.p >= 1e6 {
            "still hot at ≥ 10 bar (T24.h.a's Rayleigh)"
        } else {
            "neither"
        };
        *cures.entry(cure).or_default() += 1;
        hot.push(format!(
            "({cure}; hot Snowball under the {}) {}",
            match (hot_snowball(h), hot_snowball(b)) {
                (true, true) => "hold and the batch",
                (true, false) => "hold",
                (false, _) => "batch",
            },
            pair_line(h, b)
        ));
    }
    eprintln!(
        "4. the hot Snowballs (T_s ≥ 273.15 K) under either rules, by cure under the batch \
         {cures:?}: {}:\n  {}",
        hot.len(),
        list(&hot)
    );

    // 5. The Mars-like worlds on the warm branch, and the moved worlds' carbon dioxide ice.
    let warm_held = held.bodies.iter().filter(|x| x.mars_like_warm()).count();
    let warm: Vec<String> = pairs
        .iter()
        .filter(|(_, b)| b.mars_like_warm())
        .map(|&(h, b)| pair_line(h, b))
        .collect();
    let co2_temperates = batch
        .bodies
        .iter()
        .filter(|x| x.state == SurfaceState::Temperate && x.co2.1 && (5e3..=1e5).contains(&x.p))
        .count();
    let co2_ice: Vec<String> = pairs
        .iter()
        .filter(|(h, b)| moved(h, b) && (h.co2_ice.is_some() || b.co2_ice.is_some()))
        .map(|&(h, b)| {
            format!(
                "{}: CO2 ice {:?} → {:?} (area, kg m⁻²)",
                h.label(),
                h.co2_ice,
                b.co2_ice
            )
        })
        .collect();
    let moved_without_co2_ice = pairs
        .iter()
        .filter(|(h, b)| moved(h, b) && h.co2_ice.is_none() && b.co2_ice.is_none())
        .count();
    eprintln!(
        "5. Mars-like worlds on the warm branch (Temperate, CO2 the largest gas, 0.05–1 bar, CO2's \
         frost point below the coldest zone): {warm_held} under the hold, {} under the batch (of \
         {co2_temperates} CO2-led Temperates of 0.05–1 bar under the batch):\n  {}\n   the moved \
         worlds' CO2 ice: {} with some, {moved_without_co2_ice} with none:\n  {}",
        warm.len(),
        list(&warm),
        co2_ice.len(),
        list(&co2_ice)
    );

    // 6. Cold-to-hot flips.
    let flips: Vec<String> = pairs
        .iter()
        .filter(|(h, b)| h.t_s < 200.0 && b.t_s > 400.0)
        .map(|&(h, b)| pair_line(h, b))
        .collect();
    let reverse = pairs
        .iter()
        .filter(|(h, b)| h.t_s > 400.0 && b.t_s < 200.0)
        .count();
    eprintln!(
        "6. cold (< 200 K) → hot (> 400 K) flips: {} (and hot → cold {reverse}):\n  {}",
        flips.len(),
        list(&flips)
    );

    // 7. The thermostat's cost and the census's wall time.
    let raised = batch
        .bodies
        .iter()
        .filter(|b| share(b) > TEMPERATE_AIRBORNE_CARBON * (1.0 + 1e-9) && share(b) < 1.0)
        .count();
    let whole_share = batch
        .bodies
        .iter()
        .filter(|b| b.state == SurfaceState::Temperate && share(b) >= 1.0)
        .count();
    let stored = batch
        .bodies
        .iter()
        .filter(|b| share(b) <= TEMPERATE_AIRBORNE_CARBON * (1.0 + 1e-9))
        .count();
    // The thermostat's moved Temperates by the water at their warmest zone: all of it airborne
    // only too cold (decision-bump-plan-after-22, "T53.a's thick-air ceiling", K5), and none above
    // water's liquid range.
    let by_water = |all: bool| {
        let mut split: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for b in batch.bodies.iter().filter(|b| {
            b.state == SurfaceState::Temperate
                && share(b) > TEMPERATE_AIRBORNE_CARBON * (1.0 + 1e-9)
                && (share(b) >= 1.0) == all
        }) {
            split
                .entry(format!("{:?}", b.water()))
                .or_default()
                .push(format!(
                    "{} ({:.2} K, warmest {:.5} K)",
                    b.label(),
                    b.t_s,
                    b.warmest
                ));
        }
        split
    };
    let (all_airborne, raised_by_water) = (by_water(true), by_water(false));
    eprintln!(
        "7a. the moved Temperates by the water at their warmest zone: all of it airborne {:?}; \
         raised {:?}",
        all_airborne
            .iter()
            .map(|(phase, bodies)| (phase.as_str(), bodies.len()))
            .collect::<Vec<_>>(),
        raised_by_water
            .iter()
            .map(|(phase, bodies)| (phase.as_str(), bodies.len()))
            .collect::<Vec<_>>(),
    );
    for (phase, bodies) in all_airborne.iter().chain(&raised_by_water) {
        eprintln!("  {phase}: {}", bodies.join("; "));
    }
    let boiled: Vec<&String> = all_airborne
        .get("Hot")
        .into_iter()
        .chain(raised_by_water.get("Hot"))
        .flatten()
        .collect();
    assert_eq!(
        boiled,
        Vec::<&String>::new(),
        "a moved Temperate above water's liquid range"
    );
    let passes = |run: &EscapeCensusRun| run.bodies.iter().map(|b| b.passes).sum::<u32>();
    let median = |values: &mut [f64]| {
        values.sort_by(f64::total_cmp);
        values[values.len() / 2]
    };
    let mut medians: BTreeMap<EscapeRules, (f64, f64)> = BTreeMap::new();
    for (rules, rounds) in &times {
        let mut generated: Vec<f64> = rounds.iter().map(|&(g, _)| g).collect();
        let mut walked: Vec<f64> = rounds.iter().map(|&(_, w)| w).collect();
        medians.insert(*rules, (median(&mut generated), median(&mut walked)));
    }
    let (held_time, batch_time) = (medians[&EscapeRules::Held], medians[&EscapeRules::Batch]);
    eprintln!(
        "7. the thermostat under the batch: generation {:?}, walk {:?}; under the hold: \
         generation {:?}, walk {:?}. Batch bodies whose airborne C share the thermostat raised \
         (between the stored {TEMPERATE_AIRBORNE_CARBON:e} and 1) {raised}, Temperate with all of \
         it airborne {whole_share}, at the stored share {stored}, of {} bodies ({:.2}% raised, \
         {:.2}% all airborne); the walk's atmosphere passes {} under the hold, {} under the batch \
         (the walk's extra solves {:.2}% of the batch's passes)",
        batch.generated,
        batch.walked,
        held.generated,
        held.walked,
        batch.bodies.len(),
        100.0 * count(raised) / count(batch.bodies.len()),
        100.0 * count(whole_share) / count(batch.bodies.len()),
        passes(held),
        passes(batch),
        100.0 * u32::try_from(batch.walked.extra_solves()).map_or(f64::NAN, f64::from)
            / f64::from(passes(batch)),
    );
    eprintln!(
        "   wall time, medians of 3 alternated rounds (ms): hold generate {:.1}, walk {:.1}, total \
         {:.1}; batch generate {:.1}, walk {:.1}, total {:.1}; batch ÷ hold: generate {:.3}, walk \
         {:.3}, total {:.3}; every round {times:?}",
        held_time.0,
        held_time.1,
        held_time.0 + held_time.1,
        batch_time.0,
        batch_time.1,
        batch_time.0 + batch_time.1,
        batch_time.0 / held_time.0,
        batch_time.1 / held_time.1,
        (batch_time.0 + batch_time.1) / (held_time.0 + held_time.1),
    );

    // 8. The six O₂ airs.
    for (system, index) in OXYGEN_AIRS {
        let id = format!("{system:016x}.{index:04x}");
        let found = pairs.iter().find(|(h, _)| h.id == id);
        eprintln!(
            "8. {id}: {}",
            found.map_or_else(|| "not in the census".to_owned(), |&(h, b)| pair_line(h, b))
        );
    }

    // 9. The thick airs the batch removes.
    let thick_airs_lost: Vec<String> = pairs
        .iter()
        .filter(|(h, b)| h.p >= 0.5e5 && b.p < 100.0)
        .map(|&(h, b)| format!("({}) {}", b.loss_cause(), pair_line(h, b)))
        .collect();
    eprintln!(
        "9. at least 0.5 bar under the hold, under 100 Pa under the batch: {}:\n  {}",
        thick_airs_lost.len(),
        list(&thick_airs_lost)
    );

    // 10. The maximum-greenhouse ceiling ("T53.a's thick-air ceiling", K4, test 3): no body beyond
    // the limit above max(273.15 K, T_rest); those at it by host class.
    let beyond: Vec<&EscapeCensusBody> = batch
        .bodies
        .iter()
        .filter(|b| {
            b.ceiling == crate::planetary::derive::atmosphere::GreenhouseCeiling::MaximumGreenhouse
        })
        .collect();
    let mut at_cap: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for b in beyond
        .iter()
        .filter(|b| !b.airless() && b.t_s >= b.cap - 1e-6)
    {
        at_cap
            .entry(b.host_class.as_str())
            .or_default()
            .push(format!(
                "{} {:?} {:.2} K, {:.4e} bar",
                b.label(),
                b.state,
                b.t_s,
                b.p / 1e5
            ));
    }
    let above: Vec<String> = beyond
        .iter()
        .filter(|b| b.t_s > b.cap + 1e-6)
        .map(|b| format!("{}: {:.3} K over its cap {:.3} K", b.label(), b.t_s, b.cap))
        .collect();
    eprintln!(
        "10. beyond the maximum-greenhouse limit under the batch: {} bodies; at the ceiling, by host \
         class {:?}; above it {}",
        beyond.len(),
        at_cap
            .iter()
            .map(|(class, bodies)| (*class, bodies.len()))
            .collect::<Vec<_>>(),
        above.len()
    );
    for (class, bodies) in &at_cap {
        eprintln!("  {class}: {}", bodies.join("; "));
    }

    // 11. The Temperates the thermostat raises (decision-24-cost): how each one's passes end under
    // the batch, converged or bracketed with its residual, and whether at a jump, with its passes
    // under the batch against the hold's.
    let raised_endings: Vec<String> = thermostat_moved
        .iter()
        .map(|&(b, _)| {
            use crate::planetary::derive::passes::Convergence;
            let ending = match b.convergence {
                Convergence::Converged => "converged".to_owned(),
                Convergence::Bracketed { residual, fit } => format!(
                    "bracketed, residual {residual:.3e}, {fit:?}{}",
                    if b.convergence.is_jump() {
                        ", a jump"
                    } else {
                        ", no jump"
                    }
                ),
            };
            let held_passes = held_keys
                .get(&(b.set.clone(), b.key.clone()))
                .map_or_else(|| "none".to_owned(), |&i| held.bodies[i].passes.to_string());
            format!(
                "{}: {ending}; {} passes under the batch, {held_passes} under the hold",
                b.label(),
                b.passes
            )
        })
        .collect();
    eprintln!(
        "11. the Temperates the thermostat raises, by their passes' ending under the batch: {}",
        raised_endings.len()
    );
    for (n, line) in raised_endings.iter().enumerate() {
        eprintln!("  {}. {line}", n + 1);
    }

    assert_eq!(
        above,
        Vec::<String>::new(),
        "beyond the limit, above the ceiling"
    );
}

/// The seed of look-72's universe, LOOK72 (`tests/observe_brief.rs`'s).
const LOOK72: u64 = 0xa49f_649b_ef43_4b20;

/// The 62 bodies of LOOK72 whose surface state the triage of 2026-10-22 found at odds with their
/// temperature or phases (triage-absurd-states; decision-absurd-states-24 R4.3), in its order.
const LOOK72_TRIAGED: [&str; 62] = [
    "820e6ee00000008d.0900",
    "020e56ef60080004.0200",
    "020e56ef60080007.0200",
    "020e56ef60080007.0400",
    "020e56ef60080003.0400",
    "020e56ef60080003.0500",
    "020e56ef60080003.0600",
    "020e56ef60080003.0700",
    "220e4dde80200008.0b00",
    "220e4dde80200008.0c00",
    "220e4dde80200008.0d00",
    "020e56ef60080000.0200",
    "620e377802000074.0703",
    "020e56ef60070001.0400",
    "020e56efa0080003.0100",
    "020e56ef60090005.0200",
    "020e56ef60090005.0300",
    "020e56ef60090005.0400",
    "020e56ef60090005.0500",
    "020e66ef60070004.0200",
    "020e66ef60070004.0300",
    "020e66ef60070004.0400",
    "020e56ef20080001.0300",
    "020e56ef60090006.0300",
    "020e66efa0080003.0200",
    "020e66efa0080003.0300",
    "020e66efa0080003.0400",
    "220e6dde8020000d.0201",
    "020e56ef60070005.0100",
    "020e56ef20080005.0300",
    "020e56ef20080005.0400",
    "020e66efa0070007.0500",
    "020e66efa0070007.0600",
    "020e56ef20080000.0500",
    "020e56ef20080000.0600",
    "020e56ef20080000.0700",
    "020e56ef20080000.0900",
    "220e4dde8020000b.0700",
    "220e4dde8020000b.0800",
    "220e4dde8020000b.0900",
    "220e6dde80180002.0200",
    "020e66efa007000b.0300",
    "020e66efa007000b.0400",
    "020e66efa007000b.0500",
    "020e66efa007000b.0700",
    "020e66efa007000b.0800",
    "020e66efa007000b.0900",
    "020e66efa007000b.0a00",
    "020e56efa0080000.0100",
    "020e56efa0080000.0200",
    "020e56ef20090004.0400",
    "020e56ef20090004.0600",
    "020e56ef20080004.0400",
    "020e56ef20080004.0500",
    "020e56ef20080004.0600",
    "020e66efa007000c.0500",
    "020e66efa007000c.0600",
    "020e66efa007000c.0800",
    "020e56ef20080006.0100",
    "020e56ef20080006.0200",
    "020e56ef20080006.0300",
    "020e56ef20080006.0400",
];

/// LOOK72's galaxy, as the server builds it, once per process.
fn look72_galaxy() -> &'static crate::galaxy::Galaxy {
    static GALAXY: OnceLock<crate::galaxy::Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| crate::galaxy::Galaxy::new(Seed::new(LOOK72)).with_full_potential())
}

/// The contexts of the systems of [`LOOK72_TRIAGED`]'s bodies, each once, in ID order.
fn look72_triaged_contexts() -> Vec<SystemContext> {
    let galaxy = look72_galaxy();
    let ids: std::collections::BTreeSet<&str> = LOOK72_TRIAGED
        .iter()
        .map(|id| id.split_once('.').expect("a body ID").0)
        .collect();
    ids.into_iter()
        .map(|id| {
            SystemContext::for_system(galaxy, id.parse().expect("a system ID"))
                .expect("a system of LOOK72")
        })
        .collect()
}

/// A condensable's critical isochore at `t_s` against its column (decision-absurd-states-24 R2.3),
/// [`SupercriticalReservoir::of`](crate::planetary::derive::atmosphere::SupercriticalReservoir::of)'s
/// figures at the record's `T_s` and the derivation's gravity: its own partial pressure `p_i`,
/// p* = `p_c` + `s_c` (`T_s` − `T_c`) with `s_c` the slope of the registry's vapour-pressure curve
/// at its critical point, its melting pressure at `T_s`, and the reservoir's share of the body's
/// mass, 4π G Δp ÷ g², Δp the column between p* and the lesser of `p_i` and the melting pressure.
struct IsochoreCase {
    gas: Gas,
    t_ratio: f64,
    p_i: f64,
    p_star: f64,
    p_melt: Option<f64>,
    fraction: f64,
}

/// [`IsochoreCase`] for each gas of `derived`'s air at or above its critical temperature at `t_s`.
fn isochore_cases(derived: &DerivedBody, t_s: f64) -> Vec<IsochoreCase> {
    use crate::planetary::derive::atmosphere::SupercriticalReservoir;
    let partials = derived.atmosphere().partial_pressures();
    let g = derived.surface_gravity();
    Gas::ALL
        .into_iter()
        .filter_map(|gas| {
            let column = SupercriticalReservoir::of(gas, Kelvin::new(t_s), partials.of(gas), g)?;
            Some(IsochoreCase {
                gas,
                t_ratio: column.temperature_ratio(),
                p_i: column.partial_pressure().value(),
                p_star: column.isochore_pressure().value(),
                p_melt: column.melting_pressure().map(Pascals::value),
                fraction: column.share(),
            })
        })
        .collect()
}

/// decision-absurd-states-24 R4.3's evidence run: LOOK72's 62 triaged bodies ([`LOOK72_TRIAGED`])
/// generated and derived afresh under version 23's rules and with every landed 24 hold in force
/// ([`with_every_24_hold`](crate::planetary::testing::with_every_24_hold);
/// decision-t24i-airless-state D2–D3.3), stated by the decision's four groups:
///
/// - the nine hot `Snowball`s against the maximum-greenhouse ceiling's cap, max(273.15 K,
///   `T_rest`);
/// - the two dry `Temperate`s against water's saturation temperature at their surface pressure;
/// - the three methane `Snowball`s against the critical isochore ([`isochore_cases`]);
/// - the 106 K runaway's present ratios at the runaway and maximum-greenhouse limits.
///
/// Every triaged body's state, `T_s` and `p_s` under each rules follow, and any not found; then
/// every body of the 27 systems whose state, `T_s` or `p_s` differs between the escape's batch
/// rules alone (the census's first run) and every 24 hold.
#[test]
#[ignore = "a measurement for decision-absurd-states-24 R4.3, run by name"]
#[expect(
    clippy::too_many_lines,
    reason = "one census's four groups and its body list, each stated"
)]
fn under_every_24_hold_look72_s_triaged_bodies_are_stated() {
    use std::collections::BTreeMap;

    use crate::planetary::chem::escape::{EscapeRules, with_rules};

    struct Row {
        body: EscapeCensusBody,
        runaway_now: f64,
        maximum_now: f64,
        isochore: Vec<IsochoreCase>,
    }
    let contexts = look72_triaged_contexts();
    let seed = look72_galaxy().seed();
    let collect = || {
        let mut rows: BTreeMap<String, Row> = BTreeMap::new();
        for ctx in &contexts {
            let system = generate(seed, ctx);
            census_walk("look72", ctx, &system, &mut |case| {
                let body = EscapeCensusBody::of(case);
                let isochore = isochore_cases(case.derived, body.t_s);
                rows.insert(
                    body.id.clone(),
                    Row {
                        body,
                        runaway_now: case.runaway_now,
                        maximum_now: case.maximum_now,
                        isochore,
                    },
                );
            });
        }
        rows
    };
    let held = with_rules(EscapeRules::Held, collect);
    let escape_alone = with_rules(EscapeRules::Batch, collect);
    let batch = crate::planetary::testing::with_every_24_hold(collect);
    let water = Gas::Water
        .substance()
        .substance()
        .phase_diagram()
        .expect("water's diagram");
    let line = |row: &Row| {
        let b = &row.body;
        format!(
            "{:?} T_s {:.2} K, p_s {:.4e} bar, T_eq {:.2} K, cap {:.2} K, {:?}, air [{}]",
            b.state,
            b.t_s,
            b.p / 1e5,
            b.t_eq,
            b.cap,
            b.ceiling,
            b.air
        )
    };
    let missing: Vec<&str> = LOOK72_TRIAGED
        .iter()
        .copied()
        .filter(|id| !held.contains_key(*id) || !batch.contains_key(*id))
        .collect();
    let groups: [(&str, &[&str]); 4] = [
        ("A, the 106 K runaway", &["820e6ee00000008d.0900"]),
        (
            "B, the nine hot Snowballs",
            &[
                "220e6dde8020000d.0201",
                "220e4dde80200008.0d00",
                "020e66efa0080003.0300",
                "020e56ef20080000.0700",
                "020e56ef20080006.0100",
                "020e66ef60070004.0300",
                "220e4dde8020000b.0900",
                "020e66efa007000b.0500",
                "020e56ef20080005.0400",
            ],
        ),
        (
            "D, the two dry Temperates",
            &["020e66efa0070007.0500", "020e56ef20080004.0400"],
        ),
        (
            "C, the three methane Snowballs",
            &[
                "020e56ef20080000.0900",
                "020e56ef60090005.0500",
                "020e56ef60080003.0600",
            ],
        ),
    ];
    for (name, ids) in groups {
        eprintln!("GROUP {name}");
        for id in ids {
            let (Some(h), Some(b)) = (held.get(*id), batch.get(*id)) else {
                eprintln!(
                    "  {id}: not found (held {}, batch {})",
                    held.contains_key(*id),
                    batch.contains_key(*id)
                );
                continue;
            };
            eprintln!("  {id}\n    held  {}\n    batch {}", line(h), line(b));
            let bb = &b.body;
            let saturation = water
                .saturation_temperature(Pascals::new(bb.p))
                .map_or(f64::NAN, Kelvin::value);
            eprintln!(
                "    batch: runaway now {:.4}, maximum now {:.4}; above the cap {}; T_sat(H2O, p_s) {:.2} K, above it {}; airless {}",
                b.runaway_now,
                b.maximum_now,
                bb.t_s > bb.cap,
                saturation,
                bb.t_s > saturation,
                bb.airless(),
            );
            for (rules, row) in [("held", h), ("batch", b)] {
                for case in &row.isochore {
                    eprintln!(
                        "    {rules} isochore {:?}: T/T_c {:.4}, p_i {:.4e} bar, p* {:.4e} bar, p_melt {}, above {}, fraction {:.3e} (mover {})",
                        case.gas,
                        case.t_ratio,
                        case.p_i / 1e5,
                        case.p_star / 1e5,
                        case.p_melt
                            .map_or_else(|| "none".to_owned(), |m| format!("{:.4e} bar", m / 1e5)),
                        case.p_i > case.p_star,
                        case.fraction,
                        case.fraction >= crate::planetary::params::THIN_ENVELOPE_FRACTION,
                    );
                }
            }
        }
    }
    eprintln!(
        "ALL {} triaged bodies (held -> batch):",
        LOOK72_TRIAGED.len()
    );
    for id in LOOK72_TRIAGED {
        if let (Some(h), Some(b)) = (held.get(id), batch.get(id)) {
            eprintln!(
                "  {id}: {:?} {:.1} K {:.4e} bar -> {:?} {:.1} K {:.4e} bar (runaway now {:.3}, maximum now {:.3})",
                h.body.state,
                h.body.t_s,
                h.body.p / 1e5,
                b.body.state,
                b.body.t_s,
                b.body.p / 1e5,
                b.runaway_now,
                b.maximum_now,
            );
        }
    }
    eprintln!("MISSING {missing:?}");
    eprintln!("CHANGED between the escape's batch alone and every 24 hold:");
    for (id, row) in &batch {
        let Some(alone) = escape_alone.get(id) else {
            eprintln!("  {id}: only under every 24 hold");
            continue;
        };
        let (a, b) = (&alone.body, &row.body);
        if a.state != b.state || a.t_s.total_cmp(&b.t_s).is_ne() || a.p.total_cmp(&b.p).is_ne() {
            eprintln!(
                "  {id}: {:?} {:.2} K {:.4e} bar -> {:?} {:.2} K {:.4e} bar",
                a.state,
                a.t_s,
                a.p / 1e5,
                b.state,
                b.t_s,
                b.p / 1e5
            );
        }
    }
    eprintln!(
        "MOVERS (batch, any gas above its isochore with fraction >= THIN_ENVELOPE_FRACTION) over all {} bodies of the 27 systems:",
        batch.len()
    );
    for (id, row) in &batch {
        for case in &row.isochore {
            if case.p_i > case.p_star
                && case.fraction >= crate::planetary::params::THIN_ENVELOPE_FRACTION
            {
                eprintln!(
                    "  {id} {:?} {:?}: T/T_c {:.4}, fraction {:.3e}",
                    row.body.state, case.gas, case.t_ratio, case.fraction
                );
            }
        }
    }
}

// P14.T24.i's follow-up: the floored mean on the escape's batch path, and the state read at it
// (decision-t24i-airless-state D1.4), with every landed 24 hold in force together (D3.3,
// `planetary::testing::with_every_24_hold`).

/// One body of [`FloorCensus`]: its figures at the epoch under one set of rules.
#[derive(Debug, Clone)]
struct FloorBody {
    /// Its name: its set ("sample", the golden system's name, or "look72"), its ID and its kind.
    label: String,
    /// Its ID, as its record gives it.
    id: String,
    /// Its record's surface state.
    state: SurfaceState,
    /// Its record's mean surface temperature, K.
    t_s: f64,
    /// Its derivation's surface pressure, Pa.
    p_s: f64,
    /// Its derivation's equilibrium temperature, K.
    t_eq: f64,
    /// Its night floor, K: `night_floor` at the heat flow its record's conditions read, its
    /// published pass's context's, the one that pass's base temperature read.
    floor: f64,
    /// Its derivation's partial pressures, Pa, of each gas with any.
    partials: String,
    /// Its derivation's argon partial pressure, Pa.
    argon: f64,
    /// The airborne total of its published pass's partition, Pa, and its argon's, for an
    /// `Airless` record: the published pass repeated under the context it read
    /// (`PassDriver::Again(Under::Read)`), to the bit.
    airborne: Option<(f64, f64)>,
    /// The species its ice holds, not Jeans's test (`Atmosphere::ice_held`).
    ice_held: crate::planetary::derive::atmosphere::Retention,
    /// Its derivation's atmosphere passes.
    passes: u32,
    /// A hash of its record's and its derivation's whole `Debug` text: the same, or moved.
    bits: u64,
}

impl FloorBody {
    /// The figures of the body `case`, under the rules in force.
    fn of(case: &CensusCase<'_>) -> Self {
        use std::hash::{DefaultHasher, Hash, Hasher};

        use crate::planetary::derive::{PassDriver, Under};
        use crate::planetary::hooks::surface::night_floor;
        let (surface, derived) = (case.surface, case.derived);
        let air = derived.atmosphere();
        let heat_flow = surface
            .basis()
            .map_or_else(|| surface.figures().heat_flow(), |basis| basis.heat_flow);
        let airborne = (surface.state() == SurfaceState::Airless).then(|| {
            let (_, read) = (case.again)(PassDriver::Again(Under::Read));
            read.map_or((f64::NAN, f64::NAN), |read| {
                let species = read.partition.species();
                let total = species
                    .iter()
                    .fold(0.0, |sum, s| sum + s.partial_pressure().value());
                let argon = read
                    .partition
                    .of(Gas::Argon.substance())
                    .map_or(0.0, |s| s.partial_pressure().value());
                (total, argon)
            })
        });
        let mut hasher = DefaultHasher::new();
        format!("{:?} {:?}", case.record, derived).hash(&mut hasher);
        let partials = air.partial_pressures();
        let id = case.record.identity().id().to_string();
        Self {
            label: format!("{} {id} {}", case.set, case.kind),
            id,
            state: surface.state(),
            t_s: surface.conditions().mean_surface_temperature().value(),
            p_s: air.surface_pressure().map_or(f64::NAN, Pascals::value),
            t_eq: derived.equilibrium_temperature().value(),
            floor: night_floor(heat_flow).value(),
            partials: Gas::ALL
                .into_iter()
                .filter(|&gas| partials.of(gas).value() > 0.0)
                .map(|gas| format!("{gas:?} {:.4e}", partials.of(gas).value()))
                .collect::<Vec<_>>()
                .join(", "),
            argon: partials.of(Gas::Argon).value(),
            airborne,
            ice_held: air.ice_held(),
            passes: derived.atmosphere_passes(),
            bits: hasher.finish(),
        }
    }

    /// Whether its equilibrium temperature lies under its night floor.
    fn under_its_floor(&self) -> bool {
        self.t_eq < self.floor
    }

    /// Its state, `T_s`, `T_eq`, floor, `p_s` and passes, for a census line.
    fn line(&self) -> String {
        format!(
            "{:?} T_s {:.3} K (T_eq {:.3} K, floor {:.3} K), p_s {:.4e} Pa [{}], {} passes",
            self.state, self.t_s, self.t_eq, self.floor, self.p_s, self.partials, self.passes
        )
    }
}

/// P14.T24.i's follow-up's census (decision-t24i-airless-state D1.4): every present planet, member
/// and moon with a surface at the epoch ([`census_walk`]) of the first 60 sample systems, the 15
/// golden systems (the escape batch's census's set,
/// [`under_the_escape_batch_the_census_is_compared_body_by_body`]) and the 27 systems of LOOK72's
/// 62 triaged bodies ([`look72_triaged_contexts`]), each system generated and derived afresh,
/// since the tests' caches are version 23's: with every 24 hold in force (`floored`), and with
/// every 24 hold but the unlit mean's (`held`), body by body in the same order. Built once a
/// process, for the tests that read it.
struct FloorCensus {
    floored: Vec<FloorBody>,
    held: Vec<FloorBody>,
}

/// The [`FloorCensus`], built once.
fn floor_census() -> &'static FloorCensus {
    use crate::planetary::derive::atmosphere::{UnlitMeanRules, with_unlit_mean};
    use crate::planetary::testing::with_every_24_hold;
    static CENSUS: OnceLock<FloorCensus> = OnceLock::new();
    CENSUS.get_or_init(|| {
        let look72 = look72_triaged_contexts();
        let look72_seed = look72_galaxy().seed();
        let sets: Vec<(&str, Seed, &SystemContext)> = sample()
            .iter()
            .take(60)
            .map(|ctx| ("sample", SEED, ctx))
            .chain(
                golden_systems()
                    .iter()
                    .map(|(name, ctx, _)| (*name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx)),
            )
            .chain(look72.iter().map(|ctx| ("look72", look72_seed, ctx)))
            .collect();
        let walk = || {
            let mut bodies = Vec::new();
            for &(set, seed, ctx) in &sets {
                let system = generate(seed, ctx);
                census_walk(set, ctx, &system, &mut |case| {
                    bodies.push(FloorBody::of(case));
                });
            }
            bodies
        };
        let floored = with_every_24_hold(walk);
        let held = with_every_24_hold(|| with_unlit_mean(UnlitMeanRules::Held, walk));
        let labels =
            |bodies: &[FloorBody]| bodies.iter().map(|b| b.label.clone()).collect::<Vec<_>>();
        assert_eq!(labels(&floored), labels(&held), "the same bodies, in order");
        FloorCensus { floored, held }
    })
}

/// P14.T24.i's follow-up over the census's set and LOOK72's systems, every 24 hold in force
/// (decision-t24i-airless-state D1.4): no surface body's mean surface temperature lies below its
/// night floor, and no `Airless` record's published partition holds
/// [`AIRLESS_PRESSURE`](crate::planetary::derive::atmosphere::AIRLESS_PRESSURE) or more airborne;
/// LOOK72's 62 triaged bodies are among them. The 18 unlit records' `for_body` answer waits for
/// P14.T49.n's solid argon (D2).
#[test]
fn under_every_24_hold_no_surface_lies_below_its_night_floor() {
    use crate::planetary::derive::atmosphere::AIRLESS_PRESSURE;
    let census = floor_census();
    let bodies = &census.floored;
    let below: Vec<String> = bodies
        .iter()
        .filter(|b| b.t_s.is_nan() || b.t_s < b.floor * (1.0 - 1e-12))
        .map(|b| format!("{}: {}", b.label, b.line()))
        .collect();
    assert_eq!(below, Vec::<String>::new(), "a mean below its night floor");
    let airborne: Vec<String> = bodies
        .iter()
        .filter(|b| {
            b.airborne
                .is_some_and(|(total, _)| total.is_nan() || total >= AIRLESS_PRESSURE.value())
        })
        .map(|b| format!("{}: {}, airborne {:?}", b.label, b.line(), b.airborne))
        .collect();
    assert_eq!(
        airborne,
        Vec::<String>::new(),
        "an Airless record whose partition holds 100 Pa or more airborne"
    );
    let missing: Vec<&str> = LOOK72_TRIAGED
        .iter()
        .copied()
        .filter(|id| {
            !bodies
                .iter()
                .any(|b| b.label.starts_with("look72") && b.id == *id)
        })
        .collect();
    assert_eq!(missing, Vec::<&str>::new(), "LOOK72's triaged bodies");
    let airless = bodies.iter().filter(|b| b.airborne.is_some()).count();
    let under = bodies.iter().filter(|b| b.under_its_floor()).count();
    eprintln!(
        "the floor's census under every 24 hold: {} bodies, {airless} Airless, {under} with T_eq \
         under their floor",
        bodies.len()
    );
}

/// P14.T24.i's follow-up, to the bit (decision-t24i-airless-state D1.3–D1.4): over the census's set
/// and LOOK72's systems, every body whose equilibrium temperature is at or above its night floor
/// under both rules is the same, its whole record and derivation, with every 24 hold in force as
/// with every 24 hold but the unlit mean's, since the base is then its `T_eq`.
#[test]
fn under_every_24_hold_a_body_above_its_floor_is_unchanged_by_it() {
    let census = floor_census();
    let moved: Vec<String> = census
        .floored
        .iter()
        .zip(&census.held)
        .filter(|(floored, held)| {
            !floored.under_its_floor() && !held.under_its_floor() && floored.bits != held.bits
        })
        .map(|(floored, held)| format!("{}: {} → {}", floored.label, held.line(), floored.line()))
        .collect();
    assert_eq!(moved, Vec::<String>::new());
    let above = census
        .floored
        .iter()
        .filter(|b| !b.under_its_floor())
        .count();
    eprintln!(
        "{above} of {} bodies at or above their floor, unchanged",
        census.floored.len()
    );
}

/// The four bodies of decision-t24i-airless-state D1.1, with every 24 hold in force: their state
/// and greenhouse read the floored mean, as their published mean and partition do (D1.3). With
/// every 24 hold but the unlit mean's, the escape batch's greenhouse from `T_eq` (P14.T53.a), each
/// is `Airless` at its `T_eq` with mPa to a few Pa of nitrogen and argon, while its night floor
/// lies 12–28 K above it.
///
/// From the floor its nitrogen ice, the bulk of its airborne inventory, sublimes as its argon does
/// (nitrogen's vapour is some kPa at 54–57 K), and its √p greenhouse warms the surface above the
/// floor, so three of the four flip from `Airless` to their division's state (D1.4's "a state
/// flipped"); the ruling's expectation of argon alone (331 Pa, and 2.5–9.1 Pa) counted no nitrogen:
///
/// - 01ffeb2b9ffc0000.0700, published `Airless` at 3.6 Pa from its `T_eq`, is a `Snowball` at
///   69.69 K over its 57.15 K floor with 40.9 kPa of air: nitrogen 35.4 kPa, argon 5.35 kPa (above
///   the 331 Pa of D1.1's partition at the floor), methane 64.6 Pa;
/// - 01ffdb2c1fff0001.0d00 is a `Snowball` at 46.82 K over its 45.77 K floor at 100.67 Pa, a state
///   edge (nitrogen 89.8 Pa, argon 10.9 Pa);
/// - 01fffb2c20040000.0300 is a `Snowball` at 62.35 K over its 54.50 K floor with 11.5 kPa
///   (nitrogen 10.2 kPa, argon 1.29 kPa);
/// - 02000b2b60020000.0a00 stays `Airless`, at its 44.36 K floor to the bit, publishing its floored
///   mean's partial pressures, 55.8 Pa (nitrogen 50.7 Pa, argon 5.14 Pa), its published partition
///   holding 54.6 Pa airborne (argon 2.49 Pa).
#[test]
fn under_every_24_hold_the_four_split_bodies_read_their_floor() {
    use crate::planetary::derive::atmosphere::{AIRLESS_PRESSURE, UnlitMeanRules, with_unlit_mean};
    use crate::planetary::testing::with_every_24_hold;
    const FLIPPED: [&str; 3] = [
        "01ffeb2b9ffc0000.0700",
        "01ffdb2c1fff0001.0d00",
        "01fffb2c20040000.0300",
    ];
    const AIRLESS: &str = "02000b2b60020000.0a00";
    let ids = || FLIPPED.into_iter().chain([AIRLESS]);
    let systems: std::collections::BTreeSet<SystemId> = ids()
        .map(|id| {
            id.split_once('.')
                .expect("a body ID")
                .0
                .parse()
                .expect("a system ID")
        })
        .collect();
    let walk = || {
        let mut bodies = std::collections::BTreeMap::new();
        for id in &systems {
            let ctx = sample()
                .iter()
                .find(|ctx| ctx.id() == *id)
                .expect("a system of the sample");
            let system = generate(SEED, ctx);
            census_walk("sample", ctx, &system, &mut |case| {
                let body = FloorBody::of(case);
                bodies.insert(body.id.clone(), body);
            });
        }
        bodies
    };
    let held = with_every_24_hold(|| with_unlit_mean(UnlitMeanRules::Held, walk));
    let floored = with_every_24_hold(walk);
    for id in ids() {
        eprintln!(
            "{id}:\n  held    {}, airborne {:?}\n  floored {}, airborne {:?}",
            held[id].line(),
            held[id].airborne,
            floored[id].line(),
            floored[id].airborne
        );
        let (before, after) = (&held[id], &floored[id]);
        // The split: published at `T_eq` under a floor that lies above it.
        assert_eq!(
            before.state,
            SurfaceState::Airless,
            "{id}: {}",
            before.line()
        );
        assert!(
            (before.t_s - before.t_eq).abs() <= RESETTLED * before.t_eq,
            "{id}: {}",
            before.line()
        );
        assert!(
            before.under_its_floor() && after.under_its_floor(),
            "{id}: {} → {}",
            before.line(),
            after.line()
        );
        assert!(after.t_s >= after.floor, "{id}: {}", after.line());
    }
    for id in FLIPPED {
        let body = &floored[id];
        assert_eq!(body.state, SurfaceState::Snowball, "{id}: {}", body.line());
        assert!(
            body.p_s >= AIRLESS_PRESSURE.value(),
            "{id}: {}",
            body.line()
        );
        assert!(body.t_s > body.floor, "{id}: {}", body.line());
    }
    assert!(
        floored[FLIPPED[0]].argon >= 331.0,
        "{}: {}",
        FLIPPED[0],
        floored[FLIPPED[0]].line()
    );
    let body = &floored[AIRLESS];
    assert_eq!(
        body.state,
        SurfaceState::Airless,
        "{AIRLESS}: {}",
        body.line()
    );
    assert_same_bits(body.t_s, body.floor);
    assert!(body.p_s > 1.0, "{AIRLESS}: {}", body.line());
    let (total, argon) = body.airborne.expect("an Airless record's partition");
    assert!(
        total < AIRLESS_PRESSURE.value(),
        "{AIRLESS}: {total} Pa airborne"
    );
    assert!(
        (2.0..10.0).contains(&body.argon) && (2.0..10.0).contains(&argon),
        "{AIRLESS}: argon {} Pa published, {argon} Pa in its partition",
        body.argon
    );
}

/// P14.T24.i's follow-up's stated census, a measurement for its record, run by name
/// (decision-t24i-airless-state D1.4): over the census's set and LOOK72's systems ([`FloorCensus`]),
/// the bodies the floored mean moves between every 24 hold and every 24 hold but the unlit mean's,
/// by class, each with its passes under both:
///
/// - the unlit records (`T_eq` of 0 K), at their floor;
/// - the airless partials: `Airless` under both, their published partial pressures moved, and the
///   airless means raised alone, their partial pressures the same;
/// - the airs whose `T_eq` lies under their floor: an air under either, moved;
/// - the ice-held verdicts changed;
/// - the states flipped, each by name;
/// - any other body moved (none at or above its floor, asserted by
///   [`under_every_24_hold_a_body_above_its_floor_is_unchanged_by_it`]).
#[test]
#[ignore = "a measurement for P14.T24.i's follow-up's record, run by name"]
fn under_every_24_hold_the_floor_s_moved_bodies_are_stated() {
    let census = floor_census();
    let pairs: Vec<(&FloorBody, &FloorBody)> = census.floored.iter().zip(&census.held).collect();
    let line = |(floored, held): &(&FloorBody, &FloorBody)| {
        format!(
            "  {}:\n    held    {}\n    floored {}",
            floored.label,
            held.line(),
            floored.line()
        )
    };
    let state = |pick: &dyn Fn(&(&FloorBody, &FloorBody)) -> bool| {
        pairs
            .iter()
            .filter(|pair| pick(pair))
            .map(line)
            .collect::<Vec<_>>()
    };
    let airless = |(f, h): &(&FloorBody, &FloorBody)| {
        f.state == SurfaceState::Airless && h.state == SurfaceState::Airless && f.bits != h.bits
    };
    let classes: [(&str, Vec<String>); 7] = [
        (
            "the unlit records",
            state(&|(f, _)| f.t_eq.is_nan() || f.t_eq <= 0.0),
        ),
        (
            "the airless partials moved",
            state(&|pair| airless(pair) && pair.0.p_s.total_cmp(&pair.1.p_s).is_ne()),
        ),
        (
            "the airless means raised alone, their partials the same",
            state(&|pair| airless(pair) && pair.0.p_s.total_cmp(&pair.1.p_s).is_eq()),
        ),
        (
            "the airs whose T_eq lies under their floor, moved",
            state(&|(f, h)| {
                (f.state != SurfaceState::Airless || h.state != SurfaceState::Airless)
                    && (f.under_its_floor() || h.under_its_floor())
                    && f.bits != h.bits
            }),
        ),
        (
            "the ice-held verdicts changed",
            state(&|(f, h)| f.ice_held != h.ice_held),
        ),
        ("the states flipped", state(&|(f, h)| f.state != h.state)),
        (
            "any other body moved",
            state(&|(f, h)| f.bits != h.bits && !(f.under_its_floor() || h.under_its_floor())),
        ),
    ];
    let moved = pairs.iter().filter(|(f, h)| f.bits != h.bits).count();
    let passes = |pick: fn(&(&FloorBody, &FloorBody)) -> u32| {
        pairs
            .iter()
            .filter(|(f, h)| f.bits != h.bits)
            .map(pick)
            .sum::<u32>()
    };
    eprintln!(
        "the floor's census: {} bodies, {moved} moved, their passes {} held → {} floored",
        pairs.len(),
        passes(|(_, h)| h.passes),
        passes(|(f, _)| f.passes),
    );
    for (name, lines) in &classes {
        eprintln!("CLASS {name}: {}\n{}", lines.len(), lines.join("\n"));
    }
}

/// The bistability re-check's driver from the generator's own first albedo, 0.3, is the solve to
/// the bit (`PassDriver::From`; decision-pre-23-review item 26): over the first 5 whole systems'
/// present planets, members and moons with a surface, over their records at the epoch.
#[test]
fn a_second_start_at_the_first_s_albedo_is_the_solve() {
    let mut bodies = 0;
    for (ctx, system) in whole().iter().take(5) {
        census_walk("sample", ctx, system, &mut |case| {
            let (from, _) = (case.again)(PassDriver::From(BondAlbedo::BEFORE_ATMOSPHERES));
            assert_eq!(
                format!("{from:?}"),
                format!("{:?}", case.derived),
                "{}",
                case.key
            );
            bodies += 1;
        });
    }
    assert!(bodies > 20, "{bodies}");
}

/// P14.T54.a's test list: every present planet's keyed members hold registry keys only, over the
/// first 60 whole systems at the epoch. Each substance its surface section names (its condensates'
/// entries, its liquids and ices, its air's gases and its crust's rows) is a row of the substance
/// registry whose key resolves to it.
#[test]
fn every_present_planet_s_keyed_members_hold_registry_keys_only() {
    use crate::substance::SubstanceId;
    let resolves = |id: SubstanceId, what: &str| {
        let key = id.substance().key();
        assert_eq!(
            crate::substance::by_key(key.as_str()),
            Some(id),
            "{what}: {key:?}"
        );
    };
    let (mut planets, mut keys) = (0_u32, 0_u32);
    for (ctx, system) in whole().iter().take(60) {
        for record in system.snapshot_at(ctx, UniverseTime::EPOCH).bodies() {
            let identity = record.identity();
            if identity.kind() != BodyKind::Planet || identity.state() != BodyState::Present {
                continue;
            }
            let Section::Ok(surface) = record.surface() else {
                continue;
            };
            let what = identity.id().to_string();
            let figures = surface.figures();
            let mut named: Vec<SubstanceId> = surface
                .condensates()
                .entries()
                .iter()
                .map(crate::planetary::chem::partition::CondensateEntry::substance)
                .collect();
            named.extend(figures.surface_liquids().iter().map(|&(id, _)| id));
            named.extend(figures.surface_ices().iter().map(|&(id, _)| id));
            named.extend(surface.conditions().gases().iter().map(|&(id, _)| id));
            if let Some(crust) = surface.crust().ok() {
                named.extend(
                    [
                        crust.primary(),
                        crust.secondary(),
                        crust.tertiary(),
                        crust.bulk(),
                    ]
                    .into_iter()
                    .flatten(),
                );
                named.push(crust.provinces());
            }
            for id in named {
                resolves(id, &what);
                keys += 1;
            }
            planets += 1;
        }
    }
    eprintln!("{planets} present planets with a surface, {keys} keys, each the registry's");
    assert!(planets > 50 && keys > 500, "{planets} {keys}");
}

// P14.T53.a part C: the escape's stated cases in the sample and the goldens, each system generated
// afresh under the escape's batch rules (the cached systems were built under the rules in force).

/// The present planet or member `index` of `system` (of `ctx`) derived at the epoch under the rules
/// in force, as the record derives it: its derivation, its orbit then, and its hosts' mass then.
fn derived_at_epoch(
    ctx: &SystemContext,
    system: &PlanetarySystem,
    index: u16,
) -> Option<(DerivedBody, KeplerElements, SolarMasses)> {
    let t = UniverseTime::EPOCH;
    let epoch = Epoch::new(system, ctx, t).with_shades();
    let cosines = HourCosines::new();
    let body = system
        .bodies()
        .iter()
        .find(|body| body.index.get() == index)?;
    if !matches!(body.part, Part::Planet(_) | Part::Member(_)) {
        return None;
    }
    let zone = system.zone_of(body);
    let host = fate_host(ctx, zone);
    let fate = system.fate_of(body, &host);
    let at = fate.at(t);
    let orbit = *at.orbit()?;
    let (derived, _, _) =
        system.primary_derived(&epoch, &cosines, (body, &fate), &at, PassDriver::Solve);
    Some((derived, orbit, fate.host_mass(t)))
}

/// The cosmic shoreline's figures of a planet or member `derived` on `orbit` about hosts of mass
/// `host`: its escape speed, its X-ray and ultraviolet fluence over Earth's and the insolation, each
/// in dex from the line 0.66 (v ÷ 5 km s⁻¹)⁴ (Zahnle and Catling 2017, Fig. 1, as
/// `SHORELINE_FLUENCE_AT_REFERENCE` reads it; positive on the airless side), and its impact speed
/// over its escape speed at its circular orbital speed.
fn shoreline_figures(derived: &DerivedBody, orbit: &KeplerElements, host: SolarMasses) -> String {
    use crate::planetary::chem::escape::{
        SHORELINE_FLUENCE_AT_REFERENCE, SHORELINE_REFERENCE_ESCAPE_SPEED, planet_impact_speed,
        shoreline_distance_dex,
    };
    use crate::planetary::derive::atmosphere::earth_xuv_fluence;
    use crate::units::consts::GRAVITATIONAL_CONSTANT;
    use crate::units::{Kilograms, Metres, MetresPerSecond};
    let (m, r) = (
        Kilograms::from(derived.mass()).value(),
        Metres::from(derived.radius()).value(),
    );
    let escape = MetresPerSecond::new((2.0 * GRAVITATIONAL_CONSTANT * m / r).sqrt());
    let fluence = derived.xuv_fluence().value() / earth_xuv_fluence().value();
    let line = SHORELINE_FLUENCE_AT_REFERENCE
        * crate::math::powi(escape.value() / SHORELINE_REFERENCE_ESCAPE_SPEED.value(), 4);
    let insolation = derived.flux().value();
    let orbital = (GRAVITATIONAL_CONSTANT * Kilograms::from(host).value()
        / orbit.semi_major_axis().value())
    .sqrt();
    let impact = planet_impact_speed(MetresPerSecond::new(orbital), escape);
    format!(
        "v_esc {:.3} km/s; XUV {fluence:.4} F⊕ at {:+.3} dex; insolation {insolation:.4} S⊕ at \
         {:+.3} dex; impact {:.3} km/s, ratio {:.3}",
        escape.value() / 1e3,
        shoreline_distance_dex(fluence, escape),
        crate::math::log10(insolation / line),
        impact.value() / 1e3,
        impact.value() / escape.value(),
    )
}

/// A one-line statement of an air under the escape's batch: its state, temperature, pressure,
/// oxygen and carbon dioxide, and its escape's shoreline and oxygen.
fn escape_line(derived: &DerivedBody) -> String {
    let air = derived.atmosphere();
    let bar = |gas: Gas| air.partial_pressures().of(gas).value() / 1e5;
    format!(
        "{:?} {:.2} K at {:.4} bar (O2 {:.4} bar, CO2 {:.4} bar, N2 {:.4} bar, water inventory \
         {:.4e} kg); escape {:?}",
        air.state(),
        air.surface_temperature().value(),
        air.surface_pressure().map_or(0.0, |p| p.value() / 1e5),
        bar(Gas::Oxygen),
        bar(Gas::CarbonDioxide),
        bar(Gas::Nitrogen),
        derived.inventory().water().value(),
        air.escape()
            .map(|escape| (escape.shoreline(), escape.oxygen())),
    )
}

/// decision-p14-t54a-ganymede C1.3's six O₂ airs, as the hold records them (98.7, 9.19, 2,678,
/// 351, 100 and 14.9 bar of O₂), by system and body: 21fff6587ff80000.0200,
/// 01ffeb2c20000000.0400, 01ffeb2c1ffe0000.0200 (the 2,678 bar case), 02001b2c9ffe0000.0100,
/// 01ffeb2c20000000.0200 and 01ffeb2c20000000.0300.
const OXYGEN_AIRS: [(u64, u16); 6] = [
    (0x21ff_f658_7ff8_0000, 0x0200),
    (0x01ff_eb2c_2000_0000, 0x0400),
    (0x01ff_eb2c_1ffe_0000, 0x0200),
    (0x0200_1b2c_9ffe_0000, 0x0100),
    (0x01ff_eb2c_2000_0000, 0x0200),
    (0x01ff_eb2c_2000_0000, 0x0300),
];

/// P14.T53.a: decision-p14-t54a-ganymede C1.3's six O₂ airs and the 2,678 bar case, recomputed
/// under the batch's timed route (Luger and Barnes 2015's loss over the exposure, Schaefer et al.
/// 2016's melt, Earth's sink per unit area after it, and the cosmic shoreline before), each stated
/// beside the hold's, with its shoreline's figures (for P14.T54.b's bless note).
#[test]
#[ignore = "a measurement for P14.T53.a's record, run by name"]
fn under_the_escape_batch_the_six_oxygen_airs_are_stated() {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    let mut systems: Vec<(u64, PlanetarySystem, PlanetarySystem)> = Vec::new();
    for (id, index) in OXYGEN_AIRS {
        let ctx = sample()
            .iter()
            .find(|ctx| ctx.id().raw() == id)
            .expect("the six O₂ airs' systems are in the sample");
        if !systems.iter().any(|(known, ..)| *known == id) {
            let held = generate(SEED, ctx);
            let batch = with_rules(EscapeRules::Batch, || generate(SEED, ctx));
            systems.push((id, held, batch));
        }
        let (_, held, batch) = systems.iter().find(|(known, ..)| *known == id).unwrap();
        let (was, ..) = derived_at_epoch(ctx, held, index).expect("a present body");
        let (now, orbit, host) =
            with_rules(EscapeRules::Batch, || derived_at_epoch(ctx, batch, index))
                .expect("a present body");
        eprintln!(
            "{id:016x}.{index:04x}: {:.3} M⊕, under the batch {}\n  {}\n  under the hold {}",
            now.mass().value(),
            escape_line(&now),
            shoreline_figures(&now, &orbit, host),
            escape_line(&was),
        );
    }
}

/// The present maximum-greenhouse ratio, Σ Fᵢ ÷ `S_eff,max`, of the planet or member `index` of
/// `system` at the epoch ([`limit_now`]): below 1 beyond the limit.
fn maximum_greenhouse_ratio_at_epoch(
    ctx: &SystemContext,
    system: &PlanetarySystem,
    index: u16,
) -> f64 {
    let t = UniverseTime::EPOCH;
    let epoch = Epoch::new(system, ctx, t);
    let body = system
        .bodies()
        .iter()
        .find(|body| body.index.get() == index)
        .expect("a body");
    let zone = system.zone_of(body);
    let host = fate_host(ctx, zone);
    let fate = system.fate_of(body, &host);
    let orbit = *fate.at(t).orbit().expect("an orbit at the epoch");
    limit_now(
        crate::planetary::derive::habitable_zone::HabitableLimit::MaximumGreenhouse,
        &epoch,
        zone,
        &orbit,
    )
}

/// One line of the state of `derived` for the ceiling's and the thermostat's named bodies: its
/// state, `T_s`, `p_s`, its carbon dioxide airborne and condensed (column weights, bar), the O₂
/// it keeps and its thermostat's airborne carbon share.
fn named_state(derived: &DerivedBody) -> String {
    let air = derived.atmosphere();
    let co2 = Gas::CarbonDioxide;
    let (airborne, column) = (
        air.partial_pressures().of(co2).value(),
        air.inventory_weights().of(co2).value(),
    );
    let escape = air.escape();
    format!(
        "{:?} {:.3} K, {:.4e} bar; CO2 {:.4e} bar airborne, {:.4e} bar condensed; O2 kept {:.4e} \
         kg, airborne C share {:.4e}",
        air.state(),
        air.surface_temperature().value(),
        air.surface_pressure().map_or(0.0, |p| p.value() / 1e5),
        airborne / 1e5,
        (column - airborne).max(0.0) / 1e5,
        escape.map_or(0.0, |e| e.oxygen().kept().value()),
        escape.map_or(
            f64::NAN,
            crate::planetary::chem::escape::EscapeOutcome::airborne_carbon_share
        ),
    )
}

/// The census's three thick airs beyond the maximum-greenhouse limit that the batch, before its
/// ceiling, made 2,469 K and 2,599 K magma oceans and a 671 K runaway (decision-bump-plan-after-22,
/// "T53.a's thick-air ceiling", K1).
const CEILING_BODIES: [(u64, u16); 3] = [
    (0x0200_1b2c_dfff_0000, 0x0100),
    (0x01ff_eb2c_2000_0000, 0x0500),
    (0x01ff_eb2c_2000_0000, 0x0300),
];

/// The ceiling's three (K4, test 2), under both 24 holds (the escape's batch and the Rayleigh
/// floor): each beyond the maximum-greenhouse limit now (its present ratio stated, and below 1),
/// none a `MagmaOcean`, each at or under max(273.15 K, `T_rest`), its new state stated.
#[test]
fn under_the_escape_batch_the_ceiling_holds_the_three_thick_airs_at_or_under_freezing() {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    use crate::planetary::derive::atmosphere::{
        Absorber, GreenhouseConstants, WATER_MELTING_POINT, grey_surface_temperature,
    };
    with_rules(EscapeRules::Batch, || {
        for (id, index) in CEILING_BODIES {
            let ctx = sample()
                .iter()
                .find(|ctx| ctx.id().raw() == id)
                .expect("the ceiling's three are in the sample");
            let system = generate(SEED, ctx);
            let ratio = maximum_greenhouse_ratio_at_epoch(ctx, &system, index);
            let (derived, ..) = derived_at_epoch(ctx, &system, index).expect("a present body");
            let air = derived.atmosphere();
            let rest = GreenhouseConstants::in_force()
                .terms(
                    air.partial_pressures(),
                    derived.equilibrium_temperature(),
                    derived.surface_gravity(),
                )
                .iter()
                .filter(|term| matches!(term.absorber(), Absorber::Nitrogen | Absorber::Hydrogen))
                .fold(0.0, |sum, term| sum + term.depth());
            let cap = grey_surface_temperature(derived.equilibrium_temperature(), rest)
                .value()
                .max(WATER_MELTING_POINT.value());
            eprintln!(
                "{id:016x}.{index:04x}: present ratio {ratio:.4}, T_eq {:.3} K, cap {cap:.3} K: {}",
                derived.equilibrium_temperature().value(),
                named_state(&derived)
            );
            assert!(
                ratio < 1.0,
                "{id:016x}.{index:04x}: inside the limit, at {ratio}"
            );
            assert_ne!(
                air.state(),
                SurfaceState::MagmaOcean,
                "{id:016x}.{index:04x}"
            );
            assert!(
                air.surface_temperature().value() <= cap + 1e-9,
                "{id:016x}.{index:04x}: {} K over {cap} K",
                air.surface_temperature().value()
            );
        }
    });
}

/// The census's three Temperates the weathering thermostat took all-airborne past the liquid
/// window before its correction (decision-bump-plan-after-22, "T53.a's thick-air ceiling", K5):
/// 02000b2ce0010000.0200 (199 → 850 K, 577 bar), 01ffeb2c60020000.0400 (189 → 861 K, 490 bar),
/// and `close_binary` .0600 (195 → 723 K, 183 bar).
const OVERSHOT_TEMPERATES: [(&str, u64, u16); 3] = [
    ("sample", 0x0200_0b2c_e001_0000, 0x0200),
    ("sample", 0x01ff_eb2c_6002_0000, 0x0400),
    ("close_binary", 0, 0x0600),
];

/// The thermostat's correction (R3 Q1 (3), as K5 applies it), on its three overshot Temperates,
/// under both 24 holds: each stays `Temperate` with part of its carbon stored, its surface at the
/// set point, liquid at its edge or frozen just below a jump, and none above water's liquid range
/// (each state stated). Before it, each took all its carbon airborne at 723–861 K.
#[test]
fn under_the_escape_batch_the_three_overshot_temperates_stop_at_the_set_point() {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    use crate::planetary::derive::atmosphere::{WaterPhase, water_phase};
    with_rules(EscapeRules::Batch, || {
        for (set, id, index) in OVERSHOT_TEMPERATES {
            let (ctx, seed) = if set == "sample" {
                let ctx = sample()
                    .iter()
                    .find(|ctx| ctx.id().raw() == id)
                    .expect("the overshot Temperates' systems are in the sample");
                (ctx, SEED)
            } else {
                let (_, ctx, _) = golden_systems()
                    .iter()
                    .find(|(golden, ..)| *golden == set)
                    .expect("a golden system");
                (ctx, Seed::new(GOLDEN_SYSTEMS_SEED))
            };
            let system = generate(seed, ctx);
            let (derived, ..) = derived_at_epoch(ctx, &system, index).expect("a present body");
            let air = derived.atmosphere();
            let (t, p) = (
                air.surface_temperature(),
                air.surface_pressure()
                    .unwrap_or(crate::units::Pascals::ZERO),
            );
            let share = air.escape().map_or(
                f64::NAN,
                crate::planetary::chem::escape::EscapeOutcome::airborne_carbon_share,
            );
            eprintln!("{set} {id:016x}.{index:04x}: {}", named_state(&derived));
            assert_eq!(air.state(), SurfaceState::Temperate, "{set} {index:04x}");
            assert!(share < 1.0, "{set} {index:04x}: all of its carbon airborne");
            assert_ne!(water_phase(t, p), WaterPhase::Hot, "{set} {index:04x}");
            assert!(t.value() < 330.0, "{set} {index:04x}: {} K", t.value());
        }
    });
}

/// P14.T53.a's cost lever: a methane reservoir whose air stays at its cap through its loss keeps
/// the state its greenhouse solved, the partition holding the same air. Over the golden systems'
/// generation and the sample's first 20, every such loss is solved again all the same and compared:
/// the largest share by which a temperature or a partial pressure moves is under 10⁻⁹, the passes'
/// own convergence (`ATMOSPHERE_CONVERGENCE`), with the counts stated.
#[test]
fn under_the_escape_batch_a_saturated_methane_reservoir_s_loss_keeps_its_state() {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    use crate::planetary::derive::atmosphere::{Thermostat, with_resolve_check, with_thermostat};
    let ((((), difference), counts), systems) = with_rules(EscapeRules::Batch, || {
        let mut systems = 0;
        let checked = with_thermostat(Thermostat::On, || {
            with_resolve_check(|| {
                for (_, ctx, _) in golden_systems() {
                    let _ = generate(Seed::new(GOLDEN_SYSTEMS_SEED), ctx);
                    systems += 1;
                }
                for ctx in sample().iter().take(20) {
                    let _ = generate(SEED, ctx);
                    systems += 1;
                }
            })
        });
        (checked, systems)
    });
    eprintln!(
        "{systems} systems under the batch: {counts:?}; the largest share a saturated reservoir's \
         solve again moves: {difference:e}"
    );
    assert!(counts.saturated > 0, "{counts:?}");
    assert!(difference < 1e-9, "{difference:e}: {counts:?}");
}

/// The golden systems' knife edges of the cosmic shoreline (decision-p14-t54a-ganymede §1.7, C0
/// item 1; decision-bump-plan-after-22, R3 Q2): `close_binary` e304, `hierarchical_triple` 0500
/// and `solar_like` .0300.
const KNIFE_EDGES: [(&str, u16); 3] = [
    ("close_binary", 0xe304),
    ("hierarchical_triple", 0x0500),
    ("solar_like", 0x0300),
];

/// P14.T53.a: the shoreline's knife edges under the batch, stated, not asserted (R3 Q2): each one's
/// verdict, its distance in dex on the XUV axis and on the insolation axis, and its impact ratio,
/// beside the hold's state; and the weathering thermostat's solves over them.
#[test]
#[ignore = "a measurement for P14.T53.a's record, run by name"]
fn under_the_escape_batch_the_shoreline_s_knife_edges_are_stated() {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    use crate::planetary::derive::atmosphere::{Thermostat, with_thermostat};
    for (name, index) in KNIFE_EDGES {
        let (_, ctx, held) = golden_systems()
            .iter()
            .find(|(golden, ..)| *golden == name)
            .expect("a golden system");
        let (was, ..) = derived_at_epoch(ctx, held, index).expect("a present body");
        let ((now, orbit, host), counts) = with_thermostat(Thermostat::On, || {
            with_rules(EscapeRules::Batch, || {
                let batch = generate(Seed::new(GOLDEN_SYSTEMS_SEED), ctx);
                derived_at_epoch(ctx, &batch, index).expect("a present body")
            })
        });
        eprintln!(
            "{name} {index:04x}: {:.3} M⊕, under the batch {}\n  {}\n  under the hold {}\n  {counts:?}",
            now.mass().value(),
            escape_line(&now),
            shoreline_figures(&now, &orbit, host),
            escape_line(&was),
        );
    }
}

/// P14.T53.a: each golden envelope's He/H and helium mass fraction Y after its escape's
/// fractionation, stated for P14.T54.b to pass to P14.T50.b as its primordial He/H
/// (decision-p14-t50b-envelope §5): every planet of the golden systems that formed with an
/// envelope, its envelope's share and loss, generated and derived under the batch. Stated with
/// them, for the batch's cost against decision-p14-t54a-passes §C.6, the weathering thermostat's
/// and the batch's extra greenhouse solves over the fifteen systems' generation and their planets'
/// and members' derivations.
#[test]
#[ignore = "a measurement for P14.T53.a's record, run by name"]
fn under_the_escape_batch_the_golden_envelopes_helium_is_stated() {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    use crate::planetary::derive::atmosphere::{Thermostat, with_thermostat};
    let ((), counts) = with_thermostat(Thermostat::On, || {
        with_rules(EscapeRules::Batch, || {
            for (name, ctx, _) in golden_systems() {
                let system = generate(Seed::new(GOLDEN_SYSTEMS_SEED), ctx);
                for body in system.bodies() {
                    let Some((derived, ..)) = derived_at_epoch(ctx, &system, body.index.get())
                    else {
                        continue;
                    };
                    let Some(envelope) = derived
                        .atmosphere()
                        .escape()
                        .and_then(|escape| escape.envelope().copied())
                    else {
                        continue;
                    };
                    eprintln!(
                        "{name} {:04x}: {:.3} M⊕, {:?}, envelope {:.5} of its mass (formed with \
                         {:.5}, {:.4e} M⊕ lost); He/H {:.6}, Y {:.6}, H kept {:.6}, He kept {:.6}",
                        body.index.get(),
                        derived.mass().value(),
                        derived.atmosphere().state(),
                        derived.fractions().envelope(),
                        derived.initial_envelope_fraction(),
                        derived.envelope_lost().value(),
                        envelope.helium_to_hydrogen(),
                        envelope.helium_mass_fraction(),
                        envelope.hydrogen_kept_share(),
                        envelope.helium_kept_share(),
                    );
                }
            }
        });
    });
    eprintln!("the batch's extra solves over the goldens: {counts:?}");
}

// P14.T53.b part 1: the state from the present insolation, and the runaway's deck
// (decision-absurd-states-24 R3–R4; decision-pre-23-review item 33).

/// How [`PresentBody::class`] reads the limits' `S_eff` below Kopparapu et al.'s fit's 2,600 K.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SEffFit {
    /// Held at its 2,600 K value, as the generator holds it ([`held_flux`]).
    Held,
    /// Extrapolated below 2,600 K ([`unheld_flux`]).
    Unheld,
}

/// One body of P14.T53.b part 1's census ([`present_census`]), under the rules in force.
#[derive(Debug, Clone, PartialEq)]
struct PresentBody {
    /// Its set ("sample", the golden system's name, or "look72"), its ID and its kind.
    label: String,
    /// Its ID, as its record gives it.
    id: String,
    /// Its zone's stars at the epoch: each one's phase and effective temperature.
    hosts: String,
    /// Its record's surface state.
    state: SurfaceState,
    /// The state its water follows (`Atmosphere::water_state`).
    water_state: SurfaceState,
    /// Whether a runaway keeps Venus's deck (`Atmosphere::runaway_deck`).
    deck: crate::planetary::derive::atmosphere::RunawayDeck,
    /// Its record's mean surface temperature, K.
    t_s: f64,
    /// Its derivation's mean surface temperature, the passes', K: the one P14.T53.b part 1 (c)'s
    /// test reads.
    air_t_s: f64,
    /// Its derivation's surface pressure, Pa.
    p_s: f64,
    /// Its derivation's Bond albedo.
    bond: f64,
    /// Its derivation's cloud fraction.
    cloud: f64,
    /// For a runaway outside the present runaway limit in part 1's census run
    /// ([`PresentState::Present`]), its derivation again with (c) off
    /// ([`PresentState::WithoutHothouse`]): a hothouse's first solve, which (c) judged; `None` for
    /// every other body.
    ///
    /// [`PresentState::Present`]: crate::planetary::derive::atmosphere::PresentState::Present
    /// [`PresentState::WithoutHothouse`]: crate::planetary::derive::atmosphere::PresentState::WithoutHothouse
    first_solve: Option<DerivedBody>,
    /// Σ Fᵢ ÷ `S_eff,runaway` at its hosts' present luminosities ([`runaway_now`]).
    runaway_now: f64,
    /// The same at their largest past luminosities ([`runaway_at_peak`]).
    runaway_peak: f64,
    /// Σ Fᵢ ÷ `S_eff,max` now ([`limit_now`]).
    maximum_now: f64,
    /// The runaway ratio now, at the peak and the maximum-greenhouse ratio now with `S_eff` unheld
    /// below 2,600 K ([`CensusCase::unheld`]).
    unheld: [f64; 3],
    /// Its record's surface water, kg m⁻².
    surface_water: f64,
    /// Its record's potential ocean, 0 to 1.
    potential_ocean: f64,
    /// Its record's ocean fraction, 0 to 1.
    ocean: f64,
    /// The `H2O` areas of its record's ices and liquids, the ocean under a `Snowball`'s ice among
    /// them.
    h2o: (f64, f64),
    /// Whether its record's condensates hold an `H2O` solid and an `H2O` liquid entry.
    h2o_entries: (bool, bool),
    /// Its derivation's atmosphere passes.
    passes: u32,
    /// How its derivation's passes ended.
    convergence: crate::planetary::derive::passes::Convergence,
    /// A hash of its record's and its derivation's whole `Debug` text: the same, or moved.
    bits: u64,
}

impl PresentBody {
    /// The figures of the body `case`, under the rules in force, which are `run`'s: under
    /// [`PresentState::Present`](crate::planetary::derive::atmosphere::PresentState::Present) a
    /// runaway outside the present runaway limit is derived again with (c) off for its first
    /// solve ([`first_solve`](Self::first_solve)).
    fn of(case: &CensusCase<'_>, run: crate::planetary::derive::atmosphere::PresentState) -> Self {
        use std::hash::{DefaultHasher, Hash, Hasher};

        use crate::planetary::chem::partition::CondensatePhase;
        use crate::planetary::derive::atmosphere::{PresentState, with_present_state};
        let (surface, derived) = (case.surface, case.derived);
        let first_solve = (run == PresentState::Present
            && surface.state() == SurfaceState::RunawayGreenhouse
            && (case.runaway_now.is_nan() || case.runaway_now < 1.0))
            .then(|| {
                with_present_state(PresentState::WithoutHothouse, || {
                    (case.again)(PassDriver::Solve).0
                })
            });
        let (air, figures) = (derived.atmosphere(), surface.figures());
        let water = Gas::Water.substance();
        let area = |list: &[(crate::substance::SubstanceId, f64)]| {
            list.iter()
                .filter(|(id, _)| *id == water)
                .fold(0.0, |sum, &(_, area)| sum + area)
        };
        let mut hasher = DefaultHasher::new();
        format!("{:?} {:?}", case.record, derived).hash(&mut hasher);
        let id = case.record.identity().id().to_string();
        let condensates = surface.condensates();
        Self {
            label: format!("{} {id} {}", case.set, case.kind),
            id,
            hosts: case.hosts.clone(),
            state: surface.state(),
            water_state: air.water_state(),
            deck: air.runaway_deck(),
            t_s: surface.conditions().mean_surface_temperature().value(),
            air_t_s: air.surface_temperature().value(),
            p_s: air.surface_pressure().map_or(f64::NAN, Pascals::value),
            bond: derived.albedo().value(),
            cloud: air.cloud_fraction(),
            first_solve,
            runaway_now: case.runaway_now,
            runaway_peak: case.runaway_peak,
            maximum_now: case.maximum_now,
            unheld: case.unheld,
            surface_water: figures.surface_water().value(),
            potential_ocean: figures.potential_ocean(),
            ocean: figures.ocean_fraction(),
            h2o: (
                area(figures.surface_ices()),
                area(figures.surface_liquids()),
            ),
            h2o_entries: (
                condensates.entry(water, CondensatePhase::Solid).is_some(),
                condensates.entry(water, CondensatePhase::Liquid).is_some(),
            ),
            passes: derived.atmosphere_passes(),
            convergence: derived.convergence(),
            bits: hasher.finish(),
        }
    }

    /// Its state, deck, `T_s`, A, `p_s` and ratios, for a census line.
    fn line(&self) -> String {
        format!(
            "{:?} ({:?}, water {:?}) T_s {:.2} K, A {:.4}, cloud {:.4}, p_s {:.4e} bar; runaway \
             now {:.4}, at the peak {:.4}, maximum now {:.4}; {} passes, {:?}",
            self.state,
            self.deck,
            self.water_state,
            self.t_s,
            self.bond,
            self.cloud,
            self.p_s / 1e5,
            self.runaway_now,
            self.runaway_peak,
            self.maximum_now,
            self.passes,
            self.convergence
        )
    }

    /// A hothouse's figures, for a census line (P14.T53.b part 1 (c); decision-t53b-arm-c): its
    /// present ratios, its first solve's ([`first_solve`](Self::first_solve)) and its published
    /// solve's state, deck, `T_s`, Bond albedo, `p_s` and passes, and water's saturation
    /// temperature at each `p_s` by (c)'s convention.
    fn hothouse_line(&self) -> String {
        use crate::planetary::derive::atmosphere::water_saturation_temperature;
        let saturation =
            |p: f64| water_saturation_temperature(Pascals::new(p)).map_or(f64::NAN, Kelvin::value);
        let first = self.first_solve.as_ref().map_or_else(
            || "none".to_owned(),
            |first| {
                let air = first.atmosphere();
                let p = air.surface_pressure().map_or(f64::NAN, Pascals::value);
                format!(
                    "{:?} {:?} T_s {:.2} K, A {:.4}, p_s {:.4e} bar (water's T_sat {:.3} K), {} \
                     passes",
                    air.state(),
                    air.runaway_deck(),
                    air.surface_temperature().value(),
                    first.albedo().value(),
                    p / 1e5,
                    saturation(p),
                    first.atmosphere_passes()
                )
            },
        );
        format!(
            "runaway now {:.4}, at the peak {:.4}, maximum now {:.4} | first solve {first} | \
             published {:?} {:?} T_s {:.2} K (the record's {:.2} K), A {:.4}, p_s {:.4e} bar \
             (water's T_sat {:.3} K), {} passes",
            self.runaway_now,
            self.runaway_peak,
            self.maximum_now,
            self.state,
            self.deck,
            self.air_t_s,
            self.t_s,
            self.bond,
            self.p_s / 1e5,
            saturation(self.p_s),
            self.passes
        )
    }

    /// Its water's figures, for a census line.
    fn water_line(&self) -> String {
        format!(
            "surface water {:.4e} kg m⁻², potential ocean {:.4}, ocean {:.4}, H2O ice {:.4} and \
             liquid {:.4}, entries {:?}",
            self.surface_water,
            self.potential_ocean,
            self.ocean,
            self.h2o.0,
            self.h2o.1,
            self.h2o_entries
        )
    }

    /// The class its ratios give, `(runaway at the peak, runaway now, maximum now)` read with
    /// `S_eff` as `fit` reads it below 2,600 K: inside the runaway limit at the peak and now, a
    /// runaway; at the peak alone, its present class (P14.T53.b part 1 (a)); otherwise its peak
    /// class. For the census of the dependence on the hold, as `Sky::of`'s sums read them.
    fn class(&self, fit: SEffFit) -> &'static str {
        let (peak, now, maximum) = match fit {
            SEffFit::Held => (self.runaway_peak, self.runaway_now, self.maximum_now),
            SEffFit::Unheld => (self.unheld[1], self.unheld[0], self.unheld[2]),
        };
        if peak >= 1.0 && now >= 1.0 {
            "RunawayGreenhouse"
        } else if maximum < 1.0 {
            "Snowball"
        } else {
            "Temperate"
        }
    }

    /// Whether its water holds no more than `other`'s: its surface water, potential ocean, `H2O`
    /// liquid area (the ocean under a `Snowball`'s ice among it) and the condensates' `H2O`
    /// entries `other`'s to the bit, and no `H2O` ice where `other` holds none. An ice-rich
    /// crust's own ice, which the cover lists at the state's threshold over the climate, may move
    /// with them.
    fn holds_no_more_water(&self, other: &Self) -> bool {
        let same = |a: f64, b: f64| a.total_cmp(&b).is_eq();
        same(self.surface_water, other.surface_water)
            && same(self.potential_ocean, other.potential_ocean)
            && same(self.h2o.1, other.h2o.1)
            && self.h2o_entries == other.h2o_entries
            && (other.h2o.0 > 0.0 || same(self.h2o.0, other.h2o.0))
    }
}

/// P14.T53.b part 1's census (decision-absurd-states-24 R3–R4): every present planet, member and
/// moon with a surface at the epoch ([`census_walk`]) of the first 60 sample systems, the 15
/// golden systems and the 27 systems of LOOK72's 62 triaged bodies ([`look72_triaged_contexts`]),
/// the floor census's set ([`floor_census`]), each system generated and derived afresh with every
/// 24 hold in force, since the tests' caches are version 23's: with part 1 (`present`), each
/// runaway outside the present runaway limit with its first solve, (c) off, beside it
/// ([`PresentBody::first_solve`]), and as P14.T53.a's batch built it (`peak`,
/// `with_present_state(PresentState::Peak, ..)`), body by body in the same order; with the solves
/// without the deck and as a hothouse that generating and walking the `present` systems took
/// ([`with_deck_counts`](crate::planetary::derive::atmosphere::with_deck_counts)). Built once a
/// process, for the tests that read it.
struct PresentCensus {
    present: Vec<PresentBody>,
    peak: Vec<PresentBody>,
    generated: crate::planetary::derive::atmosphere::DeckCounts,
    walked: crate::planetary::derive::atmosphere::DeckCounts,
}

/// The [`PresentCensus`], built once.
fn present_census() -> &'static PresentCensus {
    use crate::planetary::derive::atmosphere::{
        PresentState, with_deck_counts, with_present_state,
    };
    use crate::planetary::testing::with_every_24_hold;
    static CENSUS: OnceLock<PresentCensus> = OnceLock::new();
    CENSUS.get_or_init(|| {
        let look72 = look72_triaged_contexts();
        let look72_seed = look72_galaxy().seed();
        let sets: Vec<(&str, Seed, &SystemContext)> = sample()
            .iter()
            .take(60)
            .map(|ctx| ("sample", SEED, ctx))
            .chain(
                golden_systems()
                    .iter()
                    .map(|(name, ctx, _)| (*name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx)),
            )
            .chain(look72.iter().map(|ctx| ("look72", look72_seed, ctx)))
            .collect();
        let run = |run: PresentState| {
            let (systems, generated) = with_deck_counts(|| {
                sets.iter()
                    .map(|&(_, seed, ctx)| generate(seed, ctx))
                    .collect::<Vec<_>>()
            });
            let (bodies, walked) = with_deck_counts(|| {
                let mut bodies = Vec::new();
                for (&(set, _, ctx), system) in sets.iter().zip(&systems) {
                    census_walk(set, ctx, system, &mut |case| {
                        bodies.push(PresentBody::of(case, run));
                    });
                }
                bodies
            });
            (bodies, generated, walked)
        };
        let (present, generated, walked) = with_every_24_hold(|| run(PresentState::Present));
        let (peak, ..) = with_every_24_hold(|| {
            with_present_state(PresentState::Peak, || run(PresentState::Peak))
        });
        let labels =
            |bodies: &[PresentBody]| bodies.iter().map(|b| b.label.clone()).collect::<Vec<_>>();
        assert_eq!(labels(&present), labels(&peak), "the same bodies, in order");
        PresentCensus {
            present,
            peak,
            generated,
            walked,
        }
    })
}

/// The census's five decked runaways about M dwarfs (decision-absurd-states-24 R3.1), each
/// published under Venus's deck at 273–360 K by P14.T53.a's batch.
const DECKED_FIVE: [&str; 5] = [
    "220016587ff80000.0900",
    "02001b2c9ffe0000.0200",
    "01fffb2c20000001.0200",
    "01ffeb2ce0000000.0100",
    "01ffeb2ca0000000.0100",
];

/// P14.T53.b part 1's moved count, a measurement for the record (decision-absurd-states-24
/// R3–R4), run by name over [`present_census`]'s set: every body whose state or deck part 1 moves,
/// by name, with its peak and present runaway ratios and its maximum-greenhouse ratio, its state,
/// deck, `T_s`, Bond albedo and `p_s` before (P14.T53.a's batch as built) and after, by its hosts'
/// class; the past-only runaways' water before and after; the bodies part 1 moves in any other
/// figure (none expected); the runaways solved without the deck whose deckless `T_s` is at or above
/// the cycling temperature (a partial deck, not modelled, for P14.T53.e); the bodies whose present
/// runaway ratio exceeds their peak's; the re-solve's count, its share of the bodies and its
/// passes; the `Temperate`s at or above water's saturation temperature at their `p_s` (R4.2's
/// test, none expected); (c)'s hothouses between the limits, each with its present ratios, both
/// solves' `T_s`, Bond albedo and `p_s`, water's saturation temperature there and P14.T53.a's
/// batch's figures, and (c)'s count and cost beside (b)'s (decision-t53b-arm-c); and (the batch
/// review's item 8) each body whose state part 1 moves with its present
/// runaway and maximum-greenhouse ratios, `S_eff` held at its 2,600 K value below Kopparapu et
/// al.'s fit and unheld ([`unheld_flux`]), their distances from 1 and its class both ways, with
/// every census body whose class or ceiling the extrapolation would flip.
#[test]
#[ignore = "a measurement for P14.T53.b part 1's record, run by name"]
#[expect(
    clippy::too_many_lines,
    reason = "one census's moved bodies and its cost, each listed"
)]
fn under_every_24_hold_part_one_s_moved_bodies_are_stated() {
    use crate::planetary::derive::atmosphere::{ACID_CYCLING_TEMPERATURE, RunawayDeck};
    let census = present_census();
    let pairs: Vec<(&PresentBody, &PresentBody)> =
        census.peak.iter().zip(&census.present).collect();
    let moved: Vec<&(&PresentBody, &PresentBody)> = pairs
        .iter()
        .filter(|(peak, now)| peak.state != now.state || peak.deck != now.deck)
        .collect();
    eprintln!(
        "bodies with a surface: {}; moved by part 1: {}",
        pairs.len(),
        moved.len()
    );
    let mut by_class: std::collections::BTreeMap<(String, String), u32> =
        std::collections::BTreeMap::new();
    for (peak, now) in &moved {
        *by_class
            .entry((
                host_class(&now.hosts),
                format!(
                    "{:?} {:?} → {:?} {:?}",
                    peak.state, peak.deck, now.state, now.deck
                ),
            ))
            .or_default() += 1;
        eprintln!(
            "MOVED | {} | {} | {} | {:?} {:?} → {:?} {:?} | runaway peak {:.4}, now {:.4}, \
             maximum now {:.4} | T_s {:.2} → {:.2} K | A {:.4} → {:.4} | p_s {:.4e} → {:.4e} bar \
             | passes {} → {}",
            now.label,
            host_class(&now.hosts),
            now.hosts,
            peak.state,
            peak.deck,
            now.state,
            now.deck,
            now.runaway_peak,
            now.runaway_now,
            now.maximum_now,
            peak.t_s,
            now.t_s,
            peak.bond,
            now.bond,
            peak.p_s / 1e5,
            now.p_s / 1e5,
            peak.passes,
            now.passes,
        );
        if peak.state != now.state {
            eprintln!(
                "  its water: {}\n         was {}; the same {}",
                now.water_line(),
                peak.water_line(),
                now.holds_no_more_water(peak)
            );
        }
    }
    eprintln!("moved by host class and move: {by_class:?}");
    // The batch review's item 8: the present class's dependence on `S_eff` beyond Kopparapu et
    // al.'s fit, which `HabitableLimit::flux` holds at its 2,600 K value below it. Each body whose
    // state part 1 moves, with its present ratios held and unheld and their distances from 1: a
    // hothouse (c) publishes as before among them, whose first solve (a) moved.
    eprintln!(
        "S_EFF BEYOND THE FIT | body | hosts | runaway now held → unheld (1 − r) | maximum now \
         held → unheld (r − 1) | class held → unheld"
    );
    let first_moved = |(peak, now): &(&PresentBody, &PresentBody)| {
        now.first_solve
            .as_ref()
            .is_some_and(|first| first.atmosphere().state() != peak.state)
    };
    for (peak, now) in pairs
        .iter()
        .filter(|pair| pair.0.state != pair.1.state || first_moved(pair))
    {
        eprintln!(
            "S_EFF | {} | {} | {:.4} → {:.4} ({:.4} → {:.4}) | {:.4} → {:.4} ({:+.4} → {:+.4}) | \
             {} → {} (published {:?}, was {:?})",
            now.label,
            now.hosts,
            now.runaway_now,
            now.unheld[0],
            1.0 - now.runaway_now,
            1.0 - now.unheld[0],
            now.maximum_now,
            now.unheld[2],
            now.maximum_now - 1.0,
            now.unheld[2] - 1.0,
            now.class(SEffFit::Held),
            now.class(SEffFit::Unheld),
            now.state,
            peak.state,
        );
    }
    let flips: Vec<String> = census
        .present
        .iter()
        .filter(|b| b.class(SEffFit::Held) != b.class(SEffFit::Unheld))
        .map(|b| {
            format!(
                "{}: {} → {} (peak {:.4} → {:.4}, now {:.4} → {:.4}, maximum {:.4} → {:.4}; {})",
                b.label,
                b.class(SEffFit::Held),
                b.class(SEffFit::Unheld),
                b.runaway_peak,
                b.unheld[1],
                b.runaway_now,
                b.unheld[0],
                b.maximum_now,
                b.unheld[2],
                b.hosts
            )
        })
        .collect();
    eprintln!(
        "verdicts the extrapolation below 2,600 K flips, of {} bodies: {}:\n  {}",
        census.present.len(),
        flips.len(),
        flips.join("\n  ")
    );
    let ceiling_flips = census
        .present
        .iter()
        .filter(|b| (b.maximum_now < 1.0) != (b.unheld[2] < 1.0))
        .count();
    eprintln!(
        "maximum-greenhouse ceilings (ratio below 1) the extrapolation flips: {ceiling_flips}"
    );
    let past_only: Vec<String> = pairs
        .iter()
        .filter(|(_, now)| now.runaway_peak >= 1.0 && now.runaway_now < 1.0)
        .map(|(peak, now)| {
            format!(
                "{} [{}; {}]: {}
      was {}",
                now.label,
                host_class(&now.hosts),
                now.hosts,
                now.line(),
                peak.line()
            )
        })
        .collect();
    eprintln!(
        "inside the runaway limit at the hosts' peak alone: {}:\n  {}",
        past_only.len(),
        past_only.join("\n  ")
    );
    let elsewhere: Vec<String> = pairs
        .iter()
        .filter(|(peak, now)| {
            peak.state == now.state && peak.deck == now.deck && peak.bits != now.bits
        })
        .map(|(peak, now)| format!("{}: {} → {}", now.label, peak.line(), now.line()))
        .collect();
    eprintln!(
        "moved in another figure alone: {}:\n  {}",
        elsewhere.len(),
        elsewhere.join("\n  ")
    );
    let deckless: Vec<&&(&PresentBody, &PresentBody)> = moved
        .iter()
        .filter(|(_, now)| now.deck == RunawayDeck::Deckless)
        .collect();
    let partial: Vec<String> = deckless
        .iter()
        .filter(|(_, now)| now.t_s >= ACID_CYCLING_TEMPERATURE.value())
        .map(|(peak, now)| {
            format!(
                "{}: {} (under the deck {:.2} K)",
                now.label,
                now.line(),
                peak.t_s
            )
        })
        .collect();
    eprintln!(
        "solved without the deck: {} of {} bodies ({:.3}%), their passes {} under the deck and {} \
         without (the walk's counts {:?}; the generation's, every derivation of the systems, \
         {:?}); deckless at or above {} K: {}:\n  {}",
        deckless.len(),
        pairs.len(),
        100.0 * f64::from(u32::try_from(deckless.len()).unwrap())
            / f64::from(u32::try_from(pairs.len()).unwrap()),
        deckless.iter().map(|(peak, _)| peak.passes).sum::<u32>(),
        deckless.iter().map(|(_, now)| now.passes).sum::<u32>(),
        census.walked,
        census.generated,
        ACID_CYCLING_TEMPERATURE.value(),
        partial.len(),
        partial.join("\n  ")
    );
    let passes = |bodies: &[PresentBody]| bodies.iter().map(|b| u64::from(b.passes)).sum::<u64>();
    eprintln!(
        "the walk's passes: {} with part 1, {} under P14.T53.a's batch as built; the re-solves' \
         {} passes are {:.3}% of part 1's",
        passes(&census.present),
        passes(&census.peak),
        census.walked.deckless_passes,
        100.0 * f64::from(u32::try_from(census.walked.deckless_passes).unwrap())
            / f64::from(u32::try_from(passes(&census.present)).unwrap()),
    );
    let hothouse_passes = census.walked.hothouse_passes + census.walked.hothouse_deckless_passes;
    eprintln!(
        "(c)'s re-solves: {} passes after {} in the first solves they replace, {:.3}% and {:.3}% \
         of part 1's walk",
        hothouse_passes,
        census.walked.temperate_passes,
        100.0 * f64::from(u32::try_from(hothouse_passes).unwrap())
            / f64::from(u32::try_from(passes(&census.present)).unwrap()),
        100.0 * f64::from(u32::try_from(census.walked.temperate_passes).unwrap())
            / f64::from(u32::try_from(passes(&census.present)).unwrap()),
    );
    let above_peak: Vec<String> = census
        .present
        .iter()
        .filter(|b| b.runaway_now > b.runaway_peak)
        .map(|b| format!("{}: {}", b.label, b.line()))
        .collect();
    eprintln!(
        "present runaway ratio above the peak's: {}:\n  {}",
        above_peak.len(),
        above_peak.join("\n  ")
    );
    // decision-absurd-states-24 R4.2's test of arm (c): a `Temperate` at or above water's
    // saturation temperature at its p_s, by (c)'s convention (the liquid–vapour curve, 647.096 K at
    // or above 22.064 MPa, not judged below 611.657 Pa; decision-t53b-arm-c §2).
    let hot_temperate = |b: &PresentBody| {
        b.state == SurfaceState::Temperate
            && crate::planetary::derive::atmosphere::water_saturation_temperature(Pascals::new(
                b.p_s,
            ))
            .is_some_and(|t| b.air_t_s >= t.value())
    };
    let hot: Vec<String> = pairs
        .iter()
        .filter(|(_, now)| hot_temperate(now))
        .map(|(peak, now)| format!("{}: {}\n      was {}", now.label, now.line(), peak.line()))
        .collect();
    eprintln!(
        "Temperates at or above water's saturation temperature at their p_s (arm (c)'s test): \
         {}, under P14.T53.a's batch {}:\n  {}",
        hot.len(),
        census.peak.iter().filter(|b| hot_temperate(b)).count(),
        hot.join("\n  ")
    );
    // Arm (c): each hothouse between the limits, with both solves and P14.T53.a's batch's, and its
    // cost beside (b)'s.
    let hothouses: Vec<&(&PresentBody, &PresentBody)> = pairs
        .iter()
        .filter(|(_, now)| {
            now.first_solve.is_some() && now.state == SurfaceState::RunawayGreenhouse
        })
        .collect();
    for (peak, now) in &hothouses {
        eprintln!(
            "HOTHOUSE | {} | {} | {} | {}\n  under P14.T53.a's batch {}; the same to the bit: {}",
            now.label,
            host_class(&now.hosts),
            now.hosts,
            now.hothouse_line(),
            peak.line(),
            peak.bits == now.bits
        );
    }
    eprintln!(
        "(c)'s hothouses: {} of {} bodies, {} of them deckless; the walk's {} re-solves took {} \
         passes under the deck after {} in their first solves ({} third solves without it, {} \
         passes); the generation's {:?}",
        hothouses.len(),
        pairs.len(),
        hothouses
            .iter()
            .filter(|(_, now)| now.deck == RunawayDeck::Deckless)
            .count(),
        census.walked.hothouses,
        census.walked.hothouse_passes,
        census.walked.temperate_passes,
        census.walked.hothouse_deckless,
        census.walked.hothouse_deckless_passes,
        census.generated,
    );
    for name in DECKED_FIVE {
        for (peak, now) in pairs.iter().filter(|(_, now)| now.id == name) {
            eprintln!(
                "DECKED FIVE | {} | {} | was {}",
                now.label,
                now.line(),
                peak.line()
            );
        }
    }
}

/// P14.T53.b part 1 (b)'s re-solve in the tests' trace (the batch review's fixes): a runaway solved
/// once more without its deck drops its first solve's traced passes alone, not what was traced
/// before it. With every 24 hold in force, the census's decked runaway
/// [`DECKED_FIVE`]'s 01ffeb2ca0000000.0100 is published deckless, and its traced solve lists one
/// pass for each it counts; two of them traced together list both.
#[test]
fn under_every_24_hold_a_re_solve_keeps_what_was_traced_before_it() {
    use crate::planetary::derive::atmosphere::RunawayDeck;
    use crate::planetary::derive::{PassDriver, traced};
    use crate::planetary::testing::with_every_24_hold;
    let body = DECKED_FIVE[4];
    let (system_id, _) = body.split_once('.').expect("a body ID");
    let ctx = sample()
        .iter()
        .find(|ctx| ctx.id().to_string() == system_id)
        .expect("the decked runaway's system is in the sample");
    let mut found = 0;
    with_every_24_hold(|| {
        let system = generate(SEED, ctx);
        census_walk("sample", ctx, &system, &mut |case| {
            if case.record.identity().id().to_string() != body {
                return;
            }
            found += 1;
            let passes = usize::try_from(case.derived.atmosphere_passes()).unwrap();
            assert_eq!(
                case.derived.atmosphere().runaway_deck(),
                RunawayDeck::Deckless,
                "{body}"
            );
            let (_, alone) = traced(|| (case.again)(PassDriver::Traced));
            assert_eq!(alone.len(), passes, "{body}");
            let (_, both) = traced(|| {
                (case.again)(PassDriver::Traced);
                (case.again)(PassDriver::Traced)
            });
            assert_eq!(both.len(), 2 * passes, "{body}");
        });
    });
    assert_eq!(found, 1, "{body} is present at the epoch");
}

/// decision-pre-23-review item 33's world: the sample's planet at the airless threshold whose
/// other side was a `RunawayGreenhouse` under Venus's deck, at a `T_s` of 45.8 K by P14.T51.h's
/// census at 23's stand-ins (plan 14's deferred findings: "one rocky member about an evolved
/// host") and at 100.01 Pa and 56.6 K at 23 (bless-note-23, "across the diagonal": published
/// `Airless` at 99.86 Pa), inside the runaway limit only at its substellar host's largest past
/// luminosity (a runaway ratio of 1.51 then, 0.0029 now).
const ITEM_33_WORLD: &str = "02000b2ca0000001.1100";

/// LOOK72's 106 K runaway (decision-absurd-states-24 R4.1, item 1): at 23 a `RunawayGreenhouse`
/// at 106.5 K under 5.33 bar, mostly O₂, its present runaway ratio 0.0405 and its
/// maximum-greenhouse ratio 0.1152.
const LOOK72_106_K_RUNAWAY: &str = "820e6ee00000008d.0900";

/// The census's hothouse between the limits (decision-t53b-arm-c §1): about a 2,456 K substellar
/// host, its runaway ratio 61.9 at the peak and 0.547 now, its maximum-greenhouse ratio 2.11 now;
/// under P14.T53.a's batch a `RunawayGreenhouse` under Venus's deck at 934.6 K, A 0.76 and 414.8
/// bar of carbon dioxide, and under P14.T53.b part 1 (a) alone a `Temperate` at 1,221.6 K and A
/// 0.30.
const HOTHOUSE_BODY: &str = "01ffeb2c5ffe0000.0100";

/// P14.T53.b part 1's assertions over [`present_census`]'s set, with every 24 hold in force
/// (decision-absurd-states-24 R3.4, R4.1 and R4.2; decision-t53b-arm-c §5.4–5.5; bless-note-23's
/// states census, line 4, made an assertion):
///
/// - no `RunawayGreenhouse` lies beyond the maximum-greenhouse limit now ([`limit_now`] below 1),
///   and none between the limits now ([`runaway_now`] below 1) but (c)'s hothouses, each listed by
///   name with its present ratios, both solves' state, `T_s`, Bond albedo and `p_s`, and water's
///   saturation temperature there: its first solve, with (c) off, a `Temperate` that (c) judges a
///   hothouse (`solved_as_a_hothouse`), and at 24 the census's [`HOTHOUSE_BODY`] alone;
/// - no `Temperate` lies at or above water's saturation temperature at its `p_s` by (c)'s
///   convention (`water_saturation_temperature`: the liquid–vapour curve, 647.096 K at or above
///   22.064 MPa, not judged below 611.657 Pa; R4.2's serve-gate test);
/// - none keeps Venus's deck below the acid's cycling temperature, 430 K;
/// - every runaway P14.T53.a's batch published outside the present limit but (c)'s now publishes
///   its present class, `Snowball` beyond the maximum-greenhouse limit now and `Temperate` within
///   it, its water still a runaway's (`water_state`) and its water's figures (surface water,
///   potential ocean, ocean, `H2O` ice and liquid and the condensates' `H2O` entries) P14.T53.a's
///   batch's, to the bit: its water stays lost, LOOK72's 106 K runaway ([`LOOK72_106_K_RUNAWAY`]) a
///   `Snowball` among them;
/// - decision-pre-23-review item 33's world ([`ITEM_33_WORLD`]) is not a runaway;
/// - the census's five decked runaways ([`DECKED_FIVE`]) are solved without the deck, inside the
///   present limit, each stated with its present ratio, `T_s`, Bond albedo and `p_s`;
/// - every body whose state and deck part 1 does not move is P14.T53.a's batch's to the bit, its
///   whole record and derivation: a past runaway (c) publishes under the deck is its decked solve
///   again;
/// - no present runaway ratio exceeds its peak's.
///
/// The bodies solved without the deck whose deckless `T_s` is at or above the cycling temperature
/// (a partial deck, not modelled, for P14.T53.e) are counted, not failed; and so (count (xi),
/// brief-notes-24) are (c)'s bodies that end deckless or whose published `T_s` lies under water's
/// saturation temperature, by name (none at 24).
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "part 1's assertions over one census, each stated"
)]
fn under_every_24_hold_only_a_hothouse_lies_outside_the_present_limit_and_none_on_a_cold_deck() {
    use crate::planetary::derive::atmosphere::{
        ACID_CYCLING_TEMPERATURE, RunawayDeck, solved_as_a_hothouse, water_saturation_temperature,
    };
    use crate::planetary::testing::with_every_24_hold;
    let census = present_census();
    let cycling = ACID_CYCLING_TEMPERATURE.value();
    let runaways = |bodies: &[PresentBody], test: &dyn Fn(&PresentBody) -> bool| -> Vec<String> {
        bodies
            .iter()
            .filter(|b| b.state == SurfaceState::RunawayGreenhouse && test(b))
            .map(|b| format!("{}: {}", b.label, b.line()))
            .collect()
    };
    let outside = |b: &PresentBody| b.runaway_now.is_nan() || b.runaway_now < 1.0;
    let beyond = |b: &PresentBody| b.maximum_now.is_nan() || b.maximum_now < 1.0;
    let between = |b: &PresentBody| outside(b) && !beyond(b);
    let too_cold = |b: &PresentBody| b.deck == RunawayDeck::Kept && b.t_s < cycling;
    let was_outside = runaways(&census.peak, &outside);
    let was_too_cold = runaways(&census.peak, &too_cold);
    eprintln!(
        "under P14.T53.a's batch: runaways outside the present limit {}, under the deck below {} \
         K {}",
        was_outside.len(),
        cycling,
        was_too_cold.len()
    );
    assert_eq!(
        runaways(&census.present, &beyond),
        Vec::<String>::new(),
        "a runaway beyond the maximum-greenhouse limit now"
    );
    assert_eq!(
        runaways(&census.present, &too_cold),
        Vec::<String>::new(),
        "a runaway under Venus's deck below the cycling temperature"
    );
    assert!(
        !was_outside.is_empty() && !was_too_cold.is_empty(),
        "the census holds P14.T53.a's cases"
    );

    // Arm (c): every runaway between the limits now is a hothouse, its first solve (c)'s case.
    let hothouses: Vec<&PresentBody> = census
        .present
        .iter()
        .filter(|b| b.state == SurfaceState::RunawayGreenhouse && between(b))
        .collect();
    eprintln!(
        "runaways between the limits now, (c)'s hothouses: {}",
        hothouses.len()
    );
    for hothouse in &hothouses {
        eprintln!(
            "HOTHOUSE | {} | {}",
            hothouse.label,
            hothouse.hothouse_line()
        );
        let first = hothouse
            .first_solve
            .as_ref()
            .unwrap_or_else(|| panic!("{}: no first solve", hothouse.label));
        let air = first.atmosphere();
        assert_eq!(air.state(), SurfaceState::Temperate, "{}", hothouse.label);
        assert!(
            with_every_24_hold(|| solved_as_a_hothouse(air)),
            "{}: its first solve is not (c)'s: {}",
            hothouse.label,
            hothouse.hothouse_line()
        );
    }
    assert_eq!(
        hothouses
            .iter()
            .map(|b| b.label.clone())
            .collect::<Vec<_>>(),
        vec![format!("sample {HOTHOUSE_BODY} planets")],
        "(c)'s hothouses at 24"
    );
    // Count (xi): a hothouse that ends deckless, or whose published `T_s` lies under water's
    // saturation temperature, is named to main.
    let saturation =
        |p: f64| water_saturation_temperature(Pascals::new(p)).map_or(f64::NAN, Kelvin::value);
    let named: Vec<String> = hothouses
        .iter()
        .filter(|b| b.deck == RunawayDeck::Deckless || b.air_t_s < saturation(b.p_s))
        .map(|b| format!("{}: {}", b.label, b.hothouse_line()))
        .collect();
    eprintln!(
        "count (xi): hothouses deckless or under water's saturation temperature: {}:\n  {}",
        named.len(),
        named.join("\n  ")
    );
    // R4.2's test: no `Temperate` at or above water's saturation temperature at its `p_s`.
    let hot_temperate = |b: &PresentBody| {
        b.state == SurfaceState::Temperate
            && water_saturation_temperature(Pascals::new(b.p_s))
                .is_some_and(|t| b.air_t_s >= t.value())
    };
    assert_eq!(
        census
            .present
            .iter()
            .filter(|b| hot_temperate(b))
            .map(|b| format!("{}: {}", b.label, b.line()))
            .collect::<Vec<_>>(),
        Vec::<String>::new(),
        "a Temperate at or above water's saturation temperature at its p_s"
    );
    eprintln!(
        "Temperates at or above water's saturation temperature at their p_s: none, under \
         P14.T53.a's batch {}",
        census.peak.iter().filter(|b| hot_temperate(b)).count()
    );

    let pairs: Vec<(&PresentBody, &PresentBody)> =
        census.peak.iter().zip(&census.present).collect();
    let mut past_only = 0;
    for (peak, now) in &pairs {
        let moved = peak.state != now.state || peak.deck != now.deck;
        if !moved {
            assert_eq!(peak.bits, now.bits, "{}: {}", now.label, now.line());
            continue;
        }
        if now.water_state == SurfaceState::RunawayGreenhouse
            && now.state != SurfaceState::RunawayGreenhouse
        {
            assert!(
                now.runaway_peak >= 1.0 && outside(now),
                "{}: {}",
                now.label,
                now.line()
            );
            let class = if now.maximum_now < 1.0 {
                SurfaceState::Snowball
            } else {
                SurfaceState::Temperate
            };
            let at = format!("{}: {} (was {})", now.label, now.line(), peak.line());
            assert_eq!(now.state, class, "{at}");
            assert_eq!(now.water_state, SurfaceState::RunawayGreenhouse, "{at}");
            assert!(
                now.holds_no_more_water(peak),
                "{at}: its water {} against {}",
                now.water_line(),
                peak.water_line()
            );
            past_only += 1;
        } else if now.state == SurfaceState::RunawayGreenhouse && between(now) {
            // A hothouse (c) moves: a dry `Temperate` under P14.T53.a's batch, or one solved
            // without the deck after its decked solve.
            let at = format!(
                "{}: {} (was {})",
                now.label,
                now.hothouse_line(),
                peak.line()
            );
            assert!(
                peak.state == SurfaceState::Temperate
                    || (peak.state == SurfaceState::RunawayGreenhouse
                        && now.deck == RunawayDeck::Deckless),
                "{at}"
            );
        } else {
            assert_eq!(peak.state, now.state, "{}: {}", now.label, now.line());
            assert_eq!(now.deck, RunawayDeck::Deckless, "{}", now.label);
            assert!(peak.t_s < cycling, "{}: {}", now.label, peak.line());
        }
    }
    let runaway_106_k: Vec<&(&PresentBody, &PresentBody)> = pairs
        .iter()
        .filter(|(_, now)| now.label.starts_with("look72") && now.id == LOOK72_106_K_RUNAWAY)
        .collect();
    assert_eq!(runaway_106_k.len(), 1, "{LOOK72_106_K_RUNAWAY}");
    for (peak, now) in runaway_106_k {
        eprintln!(
            "LOOK72's 106 K runaway {}: {}; its water {}\n  was {}; its water {}",
            now.label,
            now.line(),
            now.water_line(),
            peak.line(),
            peak.water_line()
        );
        assert_eq!(peak.state, SurfaceState::RunawayGreenhouse);
        assert_eq!(now.state, SurfaceState::Snowball);
        assert!(now.holds_no_more_water(peak));
    }
    let item_33: Vec<&PresentBody> = census
        .present
        .iter()
        .filter(|b| b.label.starts_with("sample") && b.id == ITEM_33_WORLD)
        .collect();
    assert_eq!(item_33.len(), 1, "{ITEM_33_WORLD}");
    for world in item_33 {
        eprintln!("item 33's world {}: {}", world.label, world.line());
        assert_ne!(world.state, SurfaceState::RunawayGreenhouse);
    }
    for name in DECKED_FIVE {
        let found: Vec<&(&PresentBody, &PresentBody)> = pairs
            .iter()
            .filter(|(_, now)| now.label.starts_with("sample") && now.id == name)
            .collect();
        assert_eq!(found.len(), 1, "{name}");
        for (peak, now) in found {
            eprintln!("{name}: {}; under the deck {}", now.line(), peak.line());
            assert_eq!(now.state, SurfaceState::RunawayGreenhouse, "{name}");
            assert_eq!(now.deck, RunawayDeck::Deckless, "{name}");
            assert!(now.runaway_now >= 1.0, "{name}");
        }
    }
    let partial: Vec<String> = census
        .present
        .iter()
        .filter(|b| b.deck == RunawayDeck::Deckless && b.t_s >= cycling)
        .map(|b| format!("{}: {}", b.label, b.line()))
        .collect();
    let deckless = census
        .present
        .iter()
        .filter(|b| b.deck == RunawayDeck::Deckless)
        .count();
    eprintln!(
        "{past_only} past-only runaways publish their present class; {} hothouses publish a \
         runaway; {deckless} runaways are solved without the deck ({:?} in the walk), {} of them \
         at or above {cycling} K without it (a partial deck, for P14.T53.e):\n  {}",
        hothouses.len(),
        census.walked,
        partial.len(),
        partial.join("\n  ")
    );
    let above_peak: Vec<String> = census
        .present
        .iter()
        .filter(|b| b.runaway_now > b.runaway_peak)
        .map(|b| format!("{}: {}", b.label, b.line()))
        .collect();
    assert_eq!(
        above_peak,
        Vec::<String>::new(),
        "a present ratio above the peak's"
    );
}

/// P14.T53.b part 1 (c) through the passes, with every 24 hold in force (decision-t53b-arm-c §3,
/// §5.5): the census's hothouse between the limits, [`HOTHOUSE_BODY`], publishes a
/// `RunawayGreenhouse` under Venus's deck, P14.T24.g's cycling case (`SulphurCycle::case_of`'s
/// first arm, which `hooks/sulphur.rs`'s own example pins: the record's tests do not call the hook
/// before P14.T54.b, `nothing_on_the_record_calls_the_inventory_yet`), its record and derivation
/// P14.T53.a's batch's to the bit (its solve
/// under the runaway's branches is that batch's decked solve again), its `T_s`, A and `p_s` stated
/// against T53.a's batch's 934.6 K, 0.76 and 414.8 bar. Its first solve, with (c) off
/// (`PresentState::WithoutHothouse`), is a `Temperate` at about 1,221.6 K and A 0.30, which (c)
/// judges a hothouse; both lie above water's critical pressure, so its saturation temperature is
/// 647.096 K. Its passes are counted as one hothouse re-solve, with no third solve.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one body's two solves and its counts, each stated"
)]
fn under_every_24_hold_the_census_s_hothouse_publishes_a_runaway_under_its_deck() {
    use crate::planetary::derive::atmosphere::{
        PresentState, RunawayDeck, solved_as_a_hothouse, water_saturation_temperature,
        with_deck_counts, with_present_state,
    };
    use crate::planetary::testing::with_every_24_hold;
    /// The hothouse as one run publishes it: its record's and derivation's `Debug` text, its
    /// record's state and mean `T_s`, its derivation, and under part 1 its first solve with (c)
    /// off.
    type Found = (String, SurfaceState, f64, DerivedBody, Option<DerivedBody>);
    let (system_id, _) = HOTHOUSE_BODY.split_once('.').expect("a body ID");
    let ctx = sample()
        .iter()
        .find(|ctx| ctx.id().to_string() == system_id)
        .expect("the hothouse's system is in the sample");
    let walk = |run: PresentState| {
        let system = generate(SEED, ctx);
        let mut found: Vec<Found> = Vec::new();
        let ((), counts) = with_deck_counts(|| {
            census_walk("sample", ctx, &system, &mut |case| {
                if case.record.identity().id().to_string() != HOTHOUSE_BODY {
                    return;
                }
                let first = (run == PresentState::Present).then(|| {
                    with_present_state(PresentState::WithoutHothouse, || {
                        (case.again)(PassDriver::Solve).0
                    })
                });
                found.push((
                    format!("{:?} {:?}", case.record, case.derived),
                    case.surface.state(),
                    case.surface.conditions().mean_surface_temperature().value(),
                    *case.derived,
                    first,
                ));
            });
        });
        (found, counts)
    };
    with_every_24_hold(|| {
        let (now, counts) = walk(PresentState::Present);
        let (peak, _) = with_present_state(PresentState::Peak, || walk(PresentState::Peak));
        assert_eq!(
            (now.len(), peak.len()),
            (1, 1),
            "{HOTHOUSE_BODY} is present at the epoch"
        );
        let (bits, state, t_s, derived, first) = &now[0];
        let air = derived.atmosphere();
        let first = first.as_ref().expect("part 1's run takes the first solve");
        let first_air = first.atmosphere();
        let bar = |air: &crate::planetary::derive::atmosphere::Atmosphere| {
            air.surface_pressure().map_or(f64::NAN, |p| p.value() / 1e5)
        };
        let saturation = |air: &crate::planetary::derive::atmosphere::Atmosphere| {
            air.surface_pressure()
                .and_then(water_saturation_temperature)
                .map_or(f64::NAN, Kelvin::value)
        };
        eprintln!(
            "{HOTHOUSE_BODY}: first solve {:?} {:?} T_s {:.3} K, A {:.4}, p_s {:.4} bar (water's \
             T_sat {:.3} K), {} passes; published {:?} {:?} T_s {:.3} K (the record's {:.3} K), \
             A {:.4}, p_s {:.4} bar (water's T_sat {:.3} K), {} passes (T53.a's batch: 934.6 K, \
             A 0.76, 414.8 bar); counts {counts:?}",
            first_air.state(),
            first_air.runaway_deck(),
            first_air.surface_temperature().value(),
            first.albedo().value(),
            bar(first_air),
            saturation(first_air),
            first.atmosphere_passes(),
            air.state(),
            air.runaway_deck(),
            air.surface_temperature().value(),
            t_s,
            derived.albedo().value(),
            bar(air),
            saturation(air),
            derived.atmosphere_passes(),
        );
        assert_eq!(*state, SurfaceState::RunawayGreenhouse);
        assert_eq!(air.state(), SurfaceState::RunawayGreenhouse);
        // A runaway under Venus's deck: P14.T24.g's cycling case.
        assert_eq!(air.runaway_deck(), RunawayDeck::Kept);
        assert_eq!(
            bits, &peak[0].0,
            "P14.T53.a's batch's decked solve, to the bit"
        );
        assert!((t_s - 934.6).abs() < 0.1, "{t_s} K");
        assert!((derived.albedo().value() - 0.76).abs() < 0.005);
        assert!((bar(air) - 414.8).abs() < 0.1, "{} bar", bar(air));
        assert_same_bits(saturation(air), 647.096);
        assert!(air.surface_temperature().value() >= saturation(air));
        assert_eq!(first_air.state(), SurfaceState::Temperate);
        assert_eq!(first_air.water_state(), SurfaceState::RunawayGreenhouse);
        assert!((first_air.surface_temperature().value() - 1_221.6).abs() < 0.5);
        assert!((first.albedo().value() - 0.30).abs() < 0.005);
        assert_same_bits(saturation(first_air), 647.096);
        assert!(solved_as_a_hothouse(first_air));
        assert_eq!(
            (
                counts.hothouses,
                counts.temperate_passes,
                counts.hothouse_passes,
                counts.hothouse_deckless,
            ),
            (
                1,
                u64::from(first.atmosphere_passes()),
                u64::from(derived.atmosphere_passes()),
                0
            ),
            "{counts:?}"
        );
    });
}

// P14.T53.b part 2: the supercritical envelope's section (decision-absurd-states-24 R2), with every
// landed 24 hold in force (decision-t24i-airless-state D3.3).

/// The census's sample body under kilobars of supercritical methane (decision-absurd-states-24
/// R2.1): a `Snowball` at 12.9 kbar and 209.1 K under the hold, and about 10 kbar, 8.78 of CH₄
/// and 1.27 of N₂, at 192.6 K under the escape's batch, 2 K above methane's critical point.
const SUPERCRITICAL_METHANE_BODY: &str = "02001b2ca0000000.0400";

/// LOOK72's three methane `Snowball`s above methane's critical temperature at 23
/// (decision-absurd-states-24 R2.1–R2.2; R4.3's census).
const LOOK72_METHANE_SNOWBALLS: [&str; 3] = [
    "020e56ef20080000.0900",
    "020e56ef60090005.0500",
    "020e56ef60080003.0600",
];

/// LOOK72's nitrogen `Snowball`s at 81–313 bar (decision-absurd-states-24 R2.3).
const LOOK72_NITROGEN_SNOWBALLS: [&str; 4] = [
    "020e56ef20090004.0600",
    "020e66efa007000b.0900",
    "020e66efa007000b.0a00",
    "020e56ef20080006.0300",
];

/// The sample's context of [`SUPERCRITICAL_METHANE_BODY`]'s system.
fn supercritical_methane_context() -> &'static SystemContext {
    let (system, _) = SUPERCRITICAL_METHANE_BODY
        .split_once('.')
        .expect("a body ID");
    sample()
        .iter()
        .find(|ctx| ctx.id().to_string() == system)
        .expect("the census body's system is in the sample")
}

/// One present planet, member or moon of [`envelope_walk`] at the epoch: its record's section
/// tags, and each condensable of its derivation's air at or above its critical temperature
/// against its critical isochore under its bulk's gravity (P14.T53.b part 2).
struct EnvelopeBody {
    /// Its set ("sample", the golden system's name, or "look72"), its ID and its kind.
    label: String,
    /// Its ID, as its record gives it.
    id: String,
    /// Its bulk's class.
    class: PlanetClass,
    /// Its record's surface section's tag.
    surface: SectionState,
    /// Its record's envelope section's tag.
    envelope: SectionState,
    /// Its derivation's surface state.
    state: SurfaceState,
    /// Its atmosphere's mean surface temperature, K.
    t_s: f64,
    /// Its atmosphere's surface pressure, Pa.
    p_s: f64,
    /// Its bulk's surface gravity, m s⁻².
    g: f64,
    /// Its condensables at or above their critical temperatures, whatever their shares.
    columns: Vec<crate::planetary::derive::atmosphere::SupercriticalReservoir>,
    /// The reservoir that hides its surface, as the record reads it.
    hidden: Option<crate::planetary::derive::atmosphere::SupercriticalReservoir>,
}

impl EnvelopeBody {
    /// The body `record` of the set `set` and kind `kind`, derived as `derived`, under the rules in
    /// force.
    fn of(set: &str, kind: &str, record: &BodyRecord, derived: &DerivedBody) -> Self {
        let bulk = record.bulk().ok().expect("a walked body has its bulk");
        let gravity = bulk.surface_gravity();
        let air = derived.atmosphere();
        let id = record.identity().id().to_string();
        Self {
            label: format!("{set} {id} {kind}"),
            id,
            class: bulk.class(),
            surface: record.section_state(RecordSection::Surface),
            envelope: record.section_state(RecordSection::Envelope),
            state: air.state(),
            t_s: air.surface_temperature().value(),
            p_s: air.surface_pressure().map_or(f64::NAN, Pascals::value),
            g: gravity.value(),
            columns: air.critical_columns(gravity).collect(),
            hidden: air.supercritical_reservoir(gravity),
        }
    }

    /// A column's figures, for a census line: its gas, `T_s` ÷ `T_c`, `p_i`, p*, the melting
    /// pressure, the share and the margins `p_i` ÷ p* and share ÷ [`THIN_ENVELOPE_FRACTION`].
    ///
    /// [`THIN_ENVELOPE_FRACTION`]: crate::planetary::params::THIN_ENVELOPE_FRACTION
    fn column_line(
        column: &crate::planetary::derive::atmosphere::SupercriticalReservoir,
    ) -> String {
        format!(
            "{:?} T/T_c {:.4}, p_i {:.4e} bar, p* {:.4e} bar (p_i/p* {:.4}), p_melt {}, share \
             {:.4e} (of the line {:.4})",
            column.gas(),
            column.temperature_ratio(),
            column.partial_pressure().value() / 1e5,
            column.isochore_pressure().value() / 1e5,
            column.partial_pressure().value() / column.isochore_pressure().value(),
            column.melting_pressure().map_or_else(
                || "none".to_owned(),
                |m| format!("{:.4e} bar", m.value() / 1e5)
            ),
            column.share(),
            column.share() / crate::planetary::params::THIN_ENVELOPE_FRACTION,
        )
    }

    /// Its tags, state, `T_s`, `p_s`, g and columns, for a census line.
    fn line(&self) -> String {
        format!(
            "{:?} {:?}, T_s {:.3} K, p_s {:.4e} bar, g {:.4} m s⁻²; surface {:?}, envelope {:?}; \
             [{}]",
            self.class,
            self.state,
            self.t_s,
            self.p_s / 1e5,
            self.g,
            self.surface,
            self.envelope,
            self.columns
                .iter()
                .map(Self::column_line)
                .collect::<Vec<_>>()
                .join("; ")
        )
    }
}

/// Every body of [`envelope_walk`] over the systems `sets`, each generated afresh at its seed
/// under the rules in force on the calling thread, in order.
fn envelope_census(sets: &[(&str, Seed, &SystemContext)]) -> Vec<EnvelopeBody> {
    let mut bodies = Vec::new();
    for &(set, seed, ctx) in sets {
        let system = generate(seed, ctx);
        envelope_walk(ctx, &system, &mut |kind, record, derived| {
            bodies.push(EnvelopeBody::of(set, kind, record, derived));
        });
    }
    bodies
}

/// The census's [`SUPERCRITICAL_METHANE_BODY`]'s record and derivation at the epoch, its system
/// generated afresh under the rules in force on the calling thread (P14.T53.b part 2), for the
/// record builder's test.
pub(crate) fn supercritical_methane_body() -> (BodyRecord, DerivedBody) {
    let ctx = supercritical_methane_context();
    let system = generate(SEED, ctx);
    let mut found = None;
    envelope_walk(ctx, &system, &mut |_, record, derived| {
        if record.identity().id().to_string() == SUPERCRITICAL_METHANE_BODY {
            found = Some((record.clone(), *derived));
        }
    });
    found.expect("the census body is present at the epoch")
}

/// The census body's moon, which a hand-built nitrogen rank puts under a supercritical envelope
/// ([`under_every_24_hold_a_moon_under_a_supercritical_envelope_takes_the_envelope_s_tags`]): at
/// its own ranks a `Snowball` holding 18.3 bar of N₂ at 135.9 K, 1.08 N₂'s `T_c`, against its
/// isochore's 49.9 bar, the sample's moon nearest the line (P14.T53.b part 2's moons' census).
const SUPERCRITICAL_MOON: &str = "02001b2ca0000000.0401";

/// P14.T53.b part 2 through a moon's record (the batch review's fixes): no moon of the 400 lies
/// under a supercritical envelope (`under_every_24_hold_the_sample_s_moons_supercritical_columns_
/// are_stated`), so one is built by hand from the sample's nearest, [`SUPERCRITICAL_MOON`], its
/// nitrogen rank raised to 1 − 10⁻⁶ in its generated system, every landed 24 hold in force. Its
/// record (`satellite_record`) then takes the envelope's tags, its surface not applicable and its
/// envelope not modelled, under its nitrogen's reservoir above N₂'s critical isochore; at its own
/// ranks its surface stays.
#[test]
fn under_every_24_hold_a_moon_under_a_supercritical_envelope_takes_the_envelope_s_tags() {
    use crate::stellar::draws::UnitUniform;
    let ctx = supercritical_methane_context();
    let (_, sub) = SUPERCRITICAL_MOON.split_once('.').expect("a body ID");
    let index = u16::from_str_radix(sub, 16).expect("a hexadecimal index");
    let moon_of = |system: &PlanetarySystem| {
        let mut found = Vec::new();
        envelope_walk(ctx, system, &mut |kind, record, derived| {
            if record.identity().id().to_string() == SUPERCRITICAL_MOON {
                found.push(EnvelopeBody::of("sample", kind, record, derived));
            }
        });
        assert_eq!(
            found.len(),
            1,
            "{SUPERCRITICAL_MOON} is present at the epoch"
        );
        found.pop().expect("one body")
    };
    let (own, raised) = crate::planetary::testing::with_every_24_hold(|| {
        let mut system = generate(SEED, ctx);
        let own = moon_of(&system);
        let moon = system
            .bodies
            .iter_mut()
            .find(|body| body.index.get() == index)
            .expect("the moon is a body of its system");
        let Part::Moon(part) = &mut moon.part else {
            panic!("{SUPERCRITICAL_MOON} is a moon")
        };
        let draws = crate::planetary::derive::atmosphere::VolatileDraws {
            nitrogen: UnitUniform::new(1.0 - 1e-6).expect("a rank in (0, 1)"),
            ..*part.satellite.volatiles()
        };
        part.satellite = part.satellite.with_volatiles(draws);
        (own, moon_of(&system))
    });
    eprintln!(
        "{SUPERCRITICAL_MOON}\n  at its own ranks {}\n  its nitrogen raised {}",
        own.line(),
        raised.line()
    );
    assert_envelope_tags(&own);
    assert_eq!(own.surface, SectionState::Ok);
    assert!(own.hidden.is_none());
    assert_envelope_tags(&raised);
    assert!(raised.label.ends_with("moons"), "{}", raised.label);
    assert!(raised.class.has_solid_surface(), "{}", raised.line());
    assert_eq!(raised.surface, SectionState::NotApplicable);
    assert_eq!(raised.envelope, SectionState::NotModelled);
    let hidden = raised
        .hidden
        .expect("its nitrogen's reservoir hides the surface");
    eprintln!("  hidden by {}", EnvelopeBody::column_line(&hidden));
    assert_eq!(hidden.gas(), Gas::Nitrogen);
}

/// P14.T53.b part 2's moons over the 400 whole systems, a measurement run by name (the batch
/// review's fixes, item 2): with every landed 24 hold in force, every moon with a condensable at
/// or above its critical temperature, its margins, and every moon a supercritical envelope hides,
/// each with its tags checked ([`assert_envelope_tags`]).
#[test]
#[ignore = "a measurement for P14.T53.b part 2's record, run by name"]
fn under_every_24_hold_the_sample_s_moons_supercritical_columns_are_stated() {
    let sets: Vec<(&str, Seed, &SystemContext)> =
        sample().iter().map(|ctx| ("sample", SEED, ctx)).collect();
    let bodies = crate::planetary::testing::with_every_24_hold(|| envelope_census(&sets));
    let moons: Vec<&EnvelopeBody> = bodies
        .iter()
        .filter(|b| b.label.ends_with("moons"))
        .collect();
    let (mut columns, mut above, mut hidden) = (0, 0, 0);
    for body in &moons {
        assert_envelope_tags(body);
        columns += body.columns.len();
        above += body.columns.iter().filter(|c| c.share() > 0.0).count();
        hidden += usize::from(body.hidden.is_some());
        if !body.columns.is_empty() {
            eprintln!("MOON {}: {}", body.label, body.line());
        }
    }
    eprintln!(
        "{} moons of {} bodies; {columns} columns at or above T_c, {above} above their isochores; \
         {hidden} hidden",
        moons.len(),
        bodies.len()
    );
}

/// P14.T53.b part 2's tags on `body`: a solid class has its surface `Ok` and its envelope not
/// applicable exactly where no supercritical envelope hides its surface, and its surface not
/// applicable and its envelope not modelled where one does; every other class its surface not
/// applicable and its envelope not modelled. So no `Ok` surface lies under a supercritical
/// envelope, and exactly one of the two applies.
fn assert_envelope_tags(body: &EnvelopeBody) {
    let at = || format!("{}: {}", body.label, body.line());
    if body.class.has_solid_surface() && body.hidden.is_none() {
        assert_eq!(body.surface, SectionState::Ok, "{}", at());
        assert_eq!(body.envelope, SectionState::NotApplicable, "{}", at());
    } else {
        assert_eq!(body.surface, SectionState::NotApplicable, "{}", at());
        assert_eq!(body.envelope, SectionState::NotModelled, "{}", at());
    }
    if let Some(hidden) = &body.hidden {
        assert!(hidden.hides_the_surface(), "{}", at());
        let largest = body
            .columns
            .iter()
            .map(crate::planetary::derive::atmosphere::SupercriticalReservoir::share)
            .fold(0.0, f64::max);
        assert_same_bits(hidden.share(), largest);
    }
}

/// P14.T53.b part 2 (decision-absurd-states-24 R2.1): with every landed 24 hold in force, the
/// census's [`SUPERCRITICAL_METHANE_BODY`], a solid body under kilobars of methane 2 K above its
/// critical point, takes the envelope section, its surface not applicable and its envelope not
/// modelled, under methane's reservoir; under the escape's hold, to version 23, it keeps its
/// surface. Its figures are printed: `T_s`, `p_i`, p*, the melting pressure, the share and g.
#[test]
fn under_every_24_hold_the_census_s_methane_snowball_takes_the_envelope_section() {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    let sets = [("sample", SEED, supercritical_methane_context())];
    let find = |bodies: Vec<EnvelopeBody>| {
        bodies
            .into_iter()
            .find(|body| body.id == SUPERCRITICAL_METHANE_BODY)
            .expect("the census body is present at the epoch")
    };
    let held = find(with_rules(EscapeRules::Held, || envelope_census(&sets)));
    let batch = find(crate::planetary::testing::with_every_24_hold(|| {
        envelope_census(&sets)
    }));
    eprintln!(
        "{SUPERCRITICAL_METHANE_BODY}\n  held  {}\n  batch {}",
        held.line(),
        batch.line()
    );
    assert_envelope_tags(&held);
    assert_eq!(held.surface, SectionState::Ok);
    assert!(held.hidden.is_none());
    assert_envelope_tags(&batch);
    assert!(batch.class.has_solid_surface(), "{}", batch.line());
    assert_eq!(batch.surface, SectionState::NotApplicable);
    assert_eq!(batch.envelope, SectionState::NotModelled);
    let hidden = batch.hidden.expect("methane's reservoir hides the surface");
    eprintln!("  hidden by {}", EnvelopeBody::column_line(&hidden));
    assert_eq!(hidden.gas(), Gas::Methane);
    assert!(hidden.temperature_ratio() >= 1.0);
}

/// P14.T53.b part 2 (decision-absurd-states-24 R2.3, R4.3): with every landed 24 hold in force,
/// LOOK72's three methane `Snowball`s and its nitrogen `Snowball`s at 81–313 bar keep their
/// surfaces, none under a supercritical envelope, their margins printed: 0900 keeps about 27 bar
/// of methane at 190.9 K against its isochore's 46.5 bar, 0500 and 0600 keep no methane (T53.a's
/// methane route), and the nitrogen bodies' columns above their isochores hold far less than a
/// thousandth of their mass.
#[test]
fn under_every_24_hold_look72_s_snowballs_keep_their_surfaces() {
    let galaxy = look72_galaxy();
    let systems: std::collections::BTreeSet<&str> = LOOK72_METHANE_SNOWBALLS
        .iter()
        .chain(&LOOK72_NITROGEN_SNOWBALLS)
        .map(|id| id.split_once('.').expect("a body ID").0)
        .collect();
    let contexts: Vec<SystemContext> = systems
        .into_iter()
        .map(|id| {
            SystemContext::for_system(galaxy, id.parse().expect("a system ID"))
                .expect("a system of LOOK72")
        })
        .collect();
    let sets: Vec<(&str, Seed, &SystemContext)> = contexts
        .iter()
        .map(|ctx| ("look72", galaxy.seed(), ctx))
        .collect();
    let bodies = crate::planetary::testing::with_every_24_hold(|| envelope_census(&sets));
    for body in &bodies {
        assert_envelope_tags(body);
    }
    for (group, ids) in [
        ("methane", &LOOK72_METHANE_SNOWBALLS[..]),
        ("nitrogen", &LOOK72_NITROGEN_SNOWBALLS[..]),
    ] {
        for id in ids {
            let body = bodies
                .iter()
                .find(|body| body.id == *id)
                .unwrap_or_else(|| panic!("{id} is present at the epoch"));
            eprintln!("LOOK72's {group} Snowball {}: {}", body.label, body.line());
            assert_eq!(body.surface, SectionState::Ok, "{}", body.line());
            assert!(body.hidden.is_none(), "{}", body.line());
        }
    }
}

/// P14.T53.b part 2's census, a measurement for the record (decision-absurd-states-24 R2.3), run
/// by name over part 1's census set ([`present_census`]'s: the first 60 sample systems, the
/// golden systems and the 27 systems of LOOK72's triaged bodies), each generated afresh with
/// every landed 24 hold in force: every body whose surface a supercritical envelope hides, by
/// name, substance, `T_s` ÷ `T_c` and share of its mass; those beyond 2.5 `T_c`, past the line's
/// checked range, with their margins; the near misses, a column of 10⁻⁴ to 10⁻³ of the mass; and
/// the count of columns above their isochores. Every body's tags are checked
/// ([`assert_envelope_tags`]).
#[test]
#[ignore = "a measurement for P14.T53.b part 2's record, run by name"]
fn under_every_24_hold_part_two_s_movers_are_stated() {
    let look72 = look72_triaged_contexts();
    let look72_seed = look72_galaxy().seed();
    let sets: Vec<(&str, Seed, &SystemContext)> = sample()
        .iter()
        .take(60)
        .map(|ctx| ("sample", SEED, ctx))
        .chain(
            golden_systems()
                .iter()
                .map(|(name, ctx, _)| (*name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx)),
        )
        .chain(look72.iter().map(|ctx| ("look72", look72_seed, ctx)))
        .collect();
    let bodies = crate::planetary::testing::with_every_24_hold(|| envelope_census(&sets));
    for body in &bodies {
        assert_envelope_tags(body);
    }
    let columns: usize = bodies.iter().map(|body| body.columns.len()).sum();
    let above: Vec<(
        &EnvelopeBody,
        &crate::planetary::derive::atmosphere::SupercriticalReservoir,
    )> = bodies
        .iter()
        .flat_map(|body| body.columns.iter().map(move |column| (body, column)))
        .filter(|(_, column)| column.share() > 0.0)
        .collect();
    let movers: Vec<&EnvelopeBody> = bodies.iter().filter(|b| b.hidden.is_some()).collect();
    eprintln!(
        "{} bodies of {} systems; {columns} columns at or above T_c, {} above their isochores; {} \
         movers",
        bodies.len(),
        sets.len(),
        above.len(),
        movers.len()
    );
    eprintln!("MOVERS:");
    for body in &movers {
        let hidden = body.hidden.as_ref().expect("a mover's reservoir");
        eprintln!(
            "  {} by {}\n    {}",
            body.label,
            EnvelopeBody::column_line(hidden),
            body.line()
        );
    }
    eprintln!("MOVERS BEYOND 2.5 T_c:");
    for body in &movers {
        for column in body.columns.iter().filter(|c| c.hides_the_surface()) {
            if column.temperature_ratio() > 2.5 {
                eprintln!("  {}: {}", body.label, EnvelopeBody::column_line(column));
            }
        }
    }
    eprintln!("NEAR MISSES (a column of 1e-4 to 1e-3 of the mass):");
    for (body, column) in &above {
        if (1e-4..crate::planetary::params::THIN_ENVELOPE_FRACTION).contains(&column.share()) {
            eprintln!(
                "  {} (hidden {}): {}",
                body.label,
                body.hidden.is_some(),
                EnvelopeBody::column_line(column)
            );
        }
    }
    eprintln!("EVERY COLUMN ABOVE ITS ISOCHORE:");
    for (body, column) in &above {
        eprintln!("  {}: {}", body.label, EnvelopeBody::column_line(column));
    }
}
