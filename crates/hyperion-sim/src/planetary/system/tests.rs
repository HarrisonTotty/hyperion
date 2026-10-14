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
use crate::planetary::derive::atmosphere::{Gas, SurfaceState, optical_depth_terms};
use crate::planetary::derive::{PlanetClass, habitable_zone};
use crate::planetary::fate::DestructionCause;
use crate::planetary::hooks::figures::{Crust, LEFTOVER_DENSITY, TectonicRegime};
use crate::planetary::index::{BodySlot, BodySub};
use crate::planetary::params::{FLATTENING_CAP, HILL_STABLE_GAP};
use crate::planetary::placement::mutual_hill_radius;
use crate::planetary::record::{DetailLevel, MoonOrigin, Population, RecordSection, SectionState};
use crate::planetary::satellites::generate_satellites;
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

/// The sample's systems, generated whole (P14.T30.a's `generate`), once.
pub(crate) fn whole() -> &'static [(SystemContext, PlanetarySystem)] {
    static SYSTEMS: OnceLock<Vec<(SystemContext, PlanetarySystem)>> = OnceLock::new();
    SYSTEMS.get_or_init(|| {
        sample()
            .iter()
            .map(|ctx| (ctx.clone(), generate(SEED, ctx)))
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
        // `generate` attaches the held rotation laws after phase D, and `generate_planets`
        // holds none (P14.T46.b); every other part of a planet is the same.
        let kept: Vec<Body> = whole
            .planets()
            .map(|body| Body {
                rotation: None,
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
            assert!(alone.rings().is_empty());
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
                    assert_eq!(
                        record.section_state(RecordSection::Hooks),
                        SectionState::NotModelled
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
/// conditions' own, and `Surface::new` refuses the gas-envelope state and conditions before
/// their second stage, which have no regime.
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
            assert_ne!(state, SurfaceState::GasEnvelope);
            assert_eq!(
                surface.conditions().climate_regime().as_ref(),
                Some(surface.climate_regime())
            );
            let rebuilt = Surface::new(state, material, conditions.clone(), figures.clone());
            assert_eq!(rebuilt.as_ref(), Some(surface));
            assert_eq!(
                Surface::new(
                    SurfaceState::GasEnvelope,
                    material,
                    conditions.clone(),
                    figures.clone()
                ),
                None
            );
            assert_eq!(
                Surface::new(state, material, conditions.without_climate(), figures),
                None
            );
            checked += 1;
        }
    }
    assert!(checked > 20, "{checked}");
}

/// P14.T48.e's gas-envelope split on a present body's record with a bulk section: a body with a
/// solid surface has its surface section `Ok`, from `DetailLevel::Surface` and not below, and its
/// envelope not applicable; every other its surface not applicable and its envelope not modelled
/// until P14.T24.d. Whether the body has a solid surface.
fn assert_surface_or_envelope(record: &BodyRecord) -> bool {
    let index = record.index();
    let bulk = record.bulk().ok().unwrap();
    let surface = record.section_state(RecordSection::Surface);
    let envelope = record.section_state(RecordSection::Envelope);
    let solid = bulk.class().has_solid_surface();
    if solid {
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
/// inertia: they differ only by their primordial periods, the moon derivation's fixed 15 h and the
/// law's drawn one (τ ∝ ω), to 10⁻¹².
#[test]
fn a_regular_moon_has_one_locking_time() {
    let mut moons = 0;
    for (ctx, system) in whole() {
        let snapshot = system.snapshot_at(ctx, parent_time(ctx));
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
            let Part::Moon(moon) = &body.part else {
                unreachable!("a regular moon is a moon")
            };
            let elements = moon
                .satellite
                .orbit_at(&moon.parent, ctx.age_at(parent_time(ctx)));
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
            let (record, now) = system.primary_record(&epoch, body, label, &fate);
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
/// where the bulk is; every sampled body's p q is at most 1 per band, and a generated airless-ice
/// body or snowball states a ratio within 5% of 1.
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
                    if matches!(
                        photometry.template(),
                        PhaseTemplate::AirlessIce | PhaseTemplate::Snowball
                    ) {
                        let ratio = photometry.bond_ratio();
                        assert!((ratio - 1.0).abs() < 0.05, "{:?}: {ratio}", record.index());
                        borrowed += 1;
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
                    check_published_figures(index, conditions, figures);
                    check_published_cover(index, conditions, figures, bulk.surface_gravity());
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

/// The published relief and cover of one body follow their rules (P14.T48.a, P14.T24.b;
/// science-p14-t24b-laws §4.1–4.6 and §4.x): continents only on a mobile lid; V in [0, `V_s`] and
/// the constructional share under the strength limit, `h♂` (g♂ ÷ g) ÷ 4, at the body's gravity
/// `gravity`; every fraction in [0, 1] and each list `H2O` alone summing to its fraction; an ocean
/// only where water is liquid or under a `Snowball`'s ice; no ice where the coldest zone is above
/// the melting point; θ the potential ocean's; and the cloud in [0, 1].
fn check_published_cover(
    index: BodyIndex,
    conditions: &SurfaceConditions,
    figures: &GlobalFigures,
    gravity: crate::units::MetresPerSecondSquared,
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
    let water = Gas::Water.substance();
    for (fraction, list) in [
        (figures.ocean_fraction(), figures.surface_liquids()),
        (figures.ice_fraction(), figures.surface_ices()),
    ] {
        assert_eq!(list.len(), 1, "{index:?}");
        assert_eq!(list[0].0, water, "{index:?}");
        assert!((0.0..=1.0).contains(&fraction), "{index:?}: {fraction}");
        assert_same_bits(list.iter().fold(0.0, |sum, &(_, f)| sum + f), fraction);
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
    if figures.ocean_fraction() > 0.0 {
        assert_same_bits(figures.ocean_fraction(), figures.potential_ocean());
    }
    if !conditions.liquid_water() && figures.ocean_fraction() > 0.0 {
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
    figures: &GlobalFigures,
) {
    let floor = hooks::surface::night_floor(figures.heat_flow());
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
            let (_, now) = system.primary_record(&epoch, planet, label, &fate);
            let seasonal = *fate.at(t).orbit().unwrap();
            let orbit = part.satellite.orbit_at(&part.parent, ctx.age_at(t));
            let derived = PlanetarySystem::moon_derived(&epoch, part, &orbit, &now).unwrap();
            let bulk = PlanetarySystem::moon_bulk_of(part, &derived, &now);
            // The greenhouse gain reads T13.c's terms at the published equilibrium temperature,
            // which is the one the moon's τ was computed at.
            let air = derived.atmosphere();
            if air.state() != SurfaceState::Airless {
                let terms =
                    optical_depth_terms(air.partial_pressures(), derived.equilibrium_temperature());
                let sum = terms.iter().fold(0.0, |sum, &(_, term)| sum + term);
                assert_same_bits(sum, air.optical_depth());
            }
            let inputs = |spin: SeasonalSpin| SurfaceInputs {
                atmosphere: derived.atmosphere(),
                equilibrium_temperature: derived.equilibrium_temperature(),
                surface_gravity: bulk.surface_gravity(),
                radius: Metres::from(bulk.radius()),
                material: SurfaceMaterial::of(&bulk.fractions()),
                stellar_flux: WattsPerSquareMetre::from(derived.flux()),
                xuv_flux: derived.xuv_flux(),
                spin,
                heat_flow: match hooks.figures() {
                    Section::Ok(figures) => figures.heat_flow(),
                    other => panic!("{:?} has conditions but figures {other:?}", moon.index()),
                },
                ocean_weight: match hooks.figures() {
                    Section::Ok(figures) => figures.ocean_weight(),
                    other => panic!("{:?} has conditions but figures {other:?}", moon.index()),
                },
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
    golden!("planetary/surface_conditions_published", w.as_str());
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
                assert!(id.substance().phase().is_some(), "{index:?}");
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
                    let (_, now) = system.primary_record(&epoch, planet, label, &fate);
                    let orbit = part.satellite.orbit_at(&part.parent, ctx.age_at(t));
                    let derived =
                        PlanetarySystem::moon_derived(&epoch, part, &orbit, &now).unwrap();
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
                    let (_, now) = system.primary_record(&epoch, body, label, &fate);
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
