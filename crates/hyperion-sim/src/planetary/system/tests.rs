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
use crate::planetary::record::{DetailLevel, MoonOrigin, Population, RecordSection, SectionState};
use crate::planetary::satellites::{MoonPlane, generate_satellites};
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
            .map(|rebuilt| rebuilt.with_crust(surface.crust().ok().map(|crust| **crust)));
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
            check_cover_rule(id, surface, &mut cover);
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
        || figures.partition_ground(crate::units::MetresPerSecondSquared::new(0.0)),
        |basis| basis.ground,
    );
    let ice = cover::ice_fraction(&cover::IceInputs {
        conditions,
        state: surface.state(),
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
/// under its ice (W3 (b)), and the lists scaled to the surface (W2).
#[derive(Debug, Default)]
struct CoverRuleTally {
    crust_ice: u32,
    under_ice: u32,
    scaled: u32,
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
fn check_cover_rule(id: crate::id::BodyId, surface: &Surface, tally: &mut CoverRuleTally) {
    use crate::planetary::chem::partition::{Basins, CondensatePhase};
    use crate::planetary::hooks::figures::cover;
    let water = Gas::Water.substance();
    let (figures, conditions) = (surface.figures(), surface.conditions());
    let condensates = surface.condensates();
    // The ground its condensates were published on: its published pass's context's (rule (f)).
    let ground = surface.basis().map_or_else(
        || figures.partition_ground(crate::units::MetresPerSecondSquared::new(0.0)),
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
                let terms = crate::planetary::derive::atmosphere::GreenhouseConstants::FITTED
                    .terms(air.partial_pressures(), derived.equilibrium_temperature());
                let sum = terms.iter().fold(0.0, |sum, &(_, term)| sum + term);
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
                material: SurfaceMaterial::of(&bulk.fractions()),
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
            let bulk = PlanetarySystem::moon_own_bulk(part, &now);
            let mut inputs = MoonInputs {
                mass: satellite.mass(),
                orbit: satellite.orbit_at(&part.parent, ctx.age_at(t)),
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
                MoonBulk::Derived => derived.radius(),
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
/// - **The radiative ground** (the six points' item 5; Batch D's review, D3-1 and PD-1): every
///   airless body's record's mean surface temperature is its equilibrium temperature to the bit.
///   Every body whose climate regime takes R09's radiative model
///   (`ClimateModelKind::RadiativeEquilibrium`, airless and thin airs alike) has R09's regolith
///   inertia at its record's crust's lid and mean surface temperature compared with its record's
///   conditions' ground's: the same bits on every airless body (asserted), bracketed ones
///   included; a thin air's, which R09 reads at its mean surface temperature and the record at
///   `T_eq`, listed, not failed, until P14.T54.a's wire subtask carries the record's Γ into R09's
///   inputs.
/// - **A column with nothing that condenses** reads no climate in its pass, and its θ is 0 to the
///   bit (R2-8).
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the sample's planets, members and moons, tallied and listed"
)]
fn under_the_batch_the_photometry_states_the_derivation_s_albedo() {
    use std::collections::BTreeMap;

    use hyperion_surface::field::ClimateModelKind;

    use crate::planetary::chem::partition::{Basins, Condensates};
    use crate::planetary::derive::atmosphere::spun_read::{self, SpunRead};
    use crate::planetary::derive::passes::{
        Context, ContextFit, Convergence, JUMP_RESIDUAL, MOST_PASSES, floats_agree,
    };
    use crate::planetary::derive::{DerivedBody, Under, equilibrium_temperature};
    use crate::planetary::hooks::crust::{CrustLid, CrustLithology, LidCrust};
    use crate::planetary::hooks::figures::cover::water_surface_share;
    use crate::planetary::hooks::surface::OceanWeight;
    use crate::planetary::surface::steps::climate::regolith_inertia_j_per_m2_k_sqrt_s;

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
    // and the radiative ones checked, those whose R09 inertia is not their record's (airless,
    // asserted none; thin airs, listed), and the deferred warmest checked.
    let (mut lids, mut airless, mut radiative, mut deferred) = (0_u32, 0_u32, 0_u32, 0_u32);
    let (mut settled_flips, mut bracketed_flips) = (Vec::<String>::new(), Vec::<String>::new());
    let (mut airless_apart, mut thin_apart) = (Vec::<String>::new(), Vec::<String>::new());
    // Tallies a derivation of the kind `kind`, re-run once at its published albedo and context as
    // `again` re-runs it, its record's surface `surface`.
    let mut tally =
        |kind: &'static str,
         derived: &DerivedBody,
         what: String,
         surface: Option<&Surface>,
         again: &dyn Fn(PassDriver) -> (DerivedBody, Option<SpunRead>)| {
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
            if let (Some(surface), Some(read)) = (surface, &read) {
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
                // D3-1: R09's radiative model, airless or under a thin air, takes Γ at its crust's
                // lid (the published pass's) and its mean T_s; the record's ground at the context's
                // lid and T_eq.
                let model = conditions.climate_regime().map(|regime| regime.model());
                if model == Some(ClimateModelKind::RadiativeEquilibrium) {
                    radiative += 1;
                    let r09 = regolith_inertia_j_per_m2_k_sqrt_s(
                        lid.map(|lid| lid.lithology()),
                        surface.material(),
                        mean,
                    );
                    let record = conditions.ground().inertia_j_per_m2_k_sqrt_s();
                    if r09.total_cmp(&record).is_ne() {
                        let listed = format!(
                            "{what}: {:?}, R09's Γ {r09:.3} against the record's {record:.3}, \
                             lids {:?} and the context's {:?}, {:?}",
                            surface.state(),
                            lid.map(|lid| lid.lithology()),
                            spun.context().lid().map(|lid| lid.lithology),
                            derived.convergence(),
                        );
                        if is_airless {
                            airless_apart.push(listed);
                        } else {
                            thin_apart.push(listed);
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
                        Class::Jump
                    } else if context == ContextFit::Jump {
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
                surface,
                &again,
            );
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
                    surface,
                    &again,
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
         T_s their T_eq to the bit; {radiative} under R09's radiative model, {} airless and {} \
         under a thin air whose R09 inertia is not their record's:\n  {}\n  {}\n{deferred} \
         published passes' lids read their record's warmest, deferred",
        settled_flips.len(),
        settled_flips.join("\n  "),
        bracketed_flips.len(),
        bracketed_flips.join("\n  "),
        airless_apart.len(),
        thin_apart.len(),
        airless_apart.join("\n  "),
        thin_apart.join("\n  "),
    );
    // PD-1: R09's airless model reads its record's Γ on every airless body, bracketed ones too.
    assert!(airless_apart.is_empty(), "{airless_apart:?}");
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
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the sample's moons, each round checked and listed"
)]
fn a_moon_s_law_is_held_after_at_most_two_rounds() {
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
    for (ctx, system) in whole().iter().take(150) {
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
        let under = |cold_traps| system.clone().with_record_rules(RecordRules { cold_traps });
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
        let under = |cold_traps| system.clone().with_record_rules(RecordRules { cold_traps });
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
