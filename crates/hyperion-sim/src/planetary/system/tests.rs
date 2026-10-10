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
                assert_eq!(record, &system.body_at(ctx, index, t).unwrap());
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
    let (mut present, mut giants, mut moons, mut rings) = (0, 0, 0, 0);
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
                BodyKind::Moon(_) => {
                    moons += 1;
                    assert!(matches!(identity.parent(), Some(OrbitHost::Body(_))));
                    if identity.state() == BodyState::Present {
                        assert_eq!(record.section_state(RecordSection::Bulk), SectionState::Ok);
                        assert_eq!(record.section_state(RecordSection::Orbit), SectionState::Ok);
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
                }
                other => panic!("{other:?} in a snapshot"),
            }
        }
    }
    assert!(
        present > 100 && giants > 5 && moons > 20 && rings > 5,
        "{present} present, {giants} giants, {moons} moons, {rings} rings"
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
/// tides its bulk section gives, with its one moment of inertia, bit for bit:
/// `rotation::tides_in_force`, which is `tides` from the 21 → 22 batch's bump.
#[test]
fn a_planet_s_locking_time_and_moon_limit_take_its_tides() {
    use crate::planetary::derive::OrbitSense;
    use crate::planetary::derive::limits::{TidalPlanet, moon_mass_limit};
    use crate::planetary::derive::rotation::{
        SpinningBody, moment_of_inertia_factor, tidal_locking_time, tides_in_force,
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
            let (k2, q) = tides_in_force(
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
