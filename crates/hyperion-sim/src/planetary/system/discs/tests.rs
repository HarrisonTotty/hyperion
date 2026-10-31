//! P14.T28.a's protoplanetary disc body, under its hold's switch.

use hyperion_testkit::lcg::Lcg;

use super::*;
use crate::planetary::SystemContext;
use crate::planetary::fate::FateHost;
use crate::planetary::hosts::white_dwarfs::{
    POLLUTED_SHARE, WhiteDwarfDebrisRules, with_white_dwarf_debris,
};
use crate::planetary::hosts::young::with_protoplanetary_disc;
use crate::planetary::system::tests::{SEED, id};
use crate::planetary::system::{generate, generate_planets};
use crate::planetary::testing::synthetic_star;
use crate::time::{Span, UniverseTime};
use crate::units::{Dex, SolarMasses};

/// A single star of `mass` M☉ and solar \[Fe/H\], `age` years old at the epoch, as the system
/// `index`.
fn star(index: u32, mass: f64, age: f64) -> SystemContext {
    synthetic_star(
        id(index),
        SolarMasses::new(mass),
        Dex::ZERO,
        Years::new(age),
    )
    .expect("a synthetic star")
}

/// The planets of `ctx` with the disc body found under its rules: what [`generate`] makes of
/// both, without its later stages, which the disc does not read.
fn planets_with_disc(ctx: &SystemContext) -> PlanetarySystem {
    let mut system = generate_planets(SEED, ctx);
    let discs = with_protoplanetary_disc(ProtoplanetaryDiscRules::Disc, || {
        SystemDiscs::find(SEED, ctx, &system)
    });
    system.discs = discs;
    system
}

/// The clock time at which the system of `ctx` is `age` years old.
fn at_age(ctx: &SystemContext, age: f64) -> UniverseTime {
    time_at_age(Years::new(age), ctx.age_at_epoch()).expect("an age on the clock")
}

/// The disc's lifetime, years, of `system`'s disc body's host.
fn lifetime_years(system: &PlanetarySystem, disc: ProtoplanetaryDisc) -> f64 {
    let profile = system
        .disc(disc.host())
        .and_then(|disc| disc.profile())
        .expect("a disc body's host has a disc");
    Years::from(profile.lifetime()).value()
}

/// The disc's record at `t`, read off every body's history.
fn record_at(system: &PlanetarySystem, ctx: &SystemContext, t: UniverseTime) -> BodyRecord {
    let hosts: Vec<FateHost<'_>> = system
        .zones
        .iter()
        .map(|zone| fate_host(ctx, zone))
        .collect();
    let fates = system.fates(&hosts);
    let disc = system.protoplanetary_disc().expect("a disc body");
    system
        .disc_record(
            &Epoch::new(system, ctx, t),
            &fates,
            disc.index(),
            BodyLabel::protoplanetary_disc(),
        )
        .expect("the disc body's record")
}

/// The order of a state in the prefix not yet formed → present → destroyed or unbound.
fn rank(state: BodyState) -> u8 {
    match state {
        BodyState::NotYetFormed => 0,
        BodyState::Present => 1,
        BodyState::Destroyed { .. } | BodyState::Unbound { .. } => 2,
    }
}

#[test]
fn a_disc_lasts_from_the_system_s_birth_to_its_lifetime_and_is_dispersed_from_it() {
    let ctx = star(1, 1.0, 3.0e7);
    let system = planets_with_disc(&ctx);
    let disc = *system
        .protoplanetary_disc()
        .expect("a Sun-like star has a disc");
    assert_eq!(disc.host(), OrbitHost::Star(0));
    assert_eq!(u16::from(disc.index()), 0xE000);
    let profile = *system
        .disc(disc.host())
        .and_then(|disc| disc.profile())
        .expect("a disc");
    let lifetime = lifetime_years(&system, disc);
    let end = at_age(&ctx, lifetime);
    let state = |t: UniverseTime| record_at(&system, &ctx, t).identity().state();
    assert_eq!(state(at_age(&ctx, -1.0e5)), BodyState::NotYetFormed);
    assert_eq!(state(at_age(&ctx, 1.0e3)), BodyState::Present);
    let just_before = end
        .checked_sub(Span::from_seconds(1))
        .expect("on the clock");
    assert_eq!(state(just_before), BodyState::Present);
    let dispersed = BodyState::Destroyed {
        cause: DestructionCause::Dispersed,
        at: end,
    };
    assert_eq!(state(end), dispersed);
    assert_eq!(state(UniverseTime::EPOCH), dispersed);

    let record = record_at(&system, &ctx, at_age(&ctx, 0.5 * lifetime));
    assert_eq!(record.identity().kind(), BodyKind::ProtoplanetaryDisc);
    assert_eq!(record.identity().parent(), Some(OrbitHost::Star(0)));
    let Section::Ok(Population::ProtoplanetaryDisc(held)) = record.population() else {
        panic!(
            "a present disc has its population: {:?}",
            record.population()
        )
    };
    assert_eq!(held.inner_edge(), profile.inner_edge());
    assert_eq!(held.outer_edge(), profile.outer_edge());
    assert_eq!(held.snow_line(), profile.snow_line());
    assert_eq!(
        held.characteristic_radius(),
        profile.characteristic_radius()
    );
    assert_eq!(held.gas_mass(), profile.gas_mass());
    assert_eq!(held.solid_mass(), profile.solid_mass());
    assert_eq!(
        record.mass(),
        &Section::Ok(EarthMasses::from(profile.gas_mass()) + profile.solid_mass())
    );
    assert_eq!(record.position(), None);
    let gone = record_at(&system, &ctx, end);
    assert_eq!(gone.population(), &Section::NotApplicable);
    assert_eq!(gone.mass(), record.mass());
}

/// P14.T28.a's test (a), its second half: no giant forms after its disc has gone, each giant
/// opens its gap from its formation on, and the disc body is `Destroyed { Dispersed }` from its
/// lifetime on, its states a prefix of not yet formed → present → destroyed.
#[test]
fn no_giant_forms_after_its_disc_has_gone_and_the_disc_is_dispersed_from_its_lifetime_on() {
    let mut lcg = Lcg::new(0x28a);
    let mut giants = 0;
    let mut discs = 0;
    for i in 0..240_u32 {
        let mass = 0.3 + 2.7 * lcg.next_f64();
        let ctx = star(4_000 + i, mass, 3.0e7);
        let system = planets_with_disc(&ctx);
        let Some(disc) = system.protoplanetary_disc().copied() else {
            continue;
        };
        discs += 1;
        let lifetime = lifetime_years(&system, disc);
        let end = at_age(&ctx, lifetime);
        let hosts: Vec<FateHost<'_>> = system
            .zones
            .iter()
            .map(|zone| fate_host(&ctx, zone))
            .collect();
        let fates = system.fates(&hosts);
        for (body, fate) in system.bodies.iter().zip(&fates) {
            let Part::Planet(planet) = &body.part else {
                continue;
            };
            let formed = fate
                .as_ref()
                .and_then(BodyFate::formed_at)
                .expect("a planet of a living star forms");
            match planet.fate.formation().kind() {
                FormationKind::Giant => {
                    giants += 1;
                    assert!(
                        formed <= end,
                        "a giant of {:?} forms after its disc",
                        ctx.id()
                    );
                    let gaps = |t: UniverseTime| match record_at(&system, &ctx, t).population() {
                        Section::Ok(Population::ProtoplanetaryDisc(held)) => held
                            .gaps()
                            .iter()
                            .map(|gap| (gap.giant(), gap.radius()))
                            .collect::<Vec<_>>(),
                        _ => Vec::new(),
                    };
                    if formed < end {
                        let orbit = fate
                            .as_ref()
                            .and_then(|fate| fate.at(formed).orbit().copied())
                            .expect("a giant is present from its formation");
                        assert!(
                            gaps(formed).contains(&(body.index, orbit.semi_major_axis())),
                            "a giant of {:?} opens its gap as it forms",
                            ctx.id()
                        );
                        let before = formed.checked_sub(Span::from_seconds(1)).expect("clock");
                        assert!(gaps(before).iter().all(|(giant, _)| *giant != body.index));
                    }
                }
                FormationKind::Small => assert_eq!(formed, end, "small planets form as it goes"),
            }
        }
        let times = [
            at_age(&ctx, -1.0),
            at_age(&ctx, 0.0),
            at_age(&ctx, 0.25 * lifetime),
            end.checked_sub(Span::from_seconds(1)).expect("clock"),
            end,
            at_age(&ctx, lifetime + 1.0e6),
            UniverseTime::EPOCH,
        ];
        let states: Vec<BodyState> = times
            .iter()
            .map(|&t| record_at(&system, &ctx, t).identity().state())
            .collect();
        assert!(
            states.windows(2).all(|w| rank(w[0]) <= rank(w[1])),
            "{states:?}"
        );
        assert_eq!(states[0], BodyState::NotYetFormed);
        assert_eq!(states[3], BodyState::Present);
        for state in &states[4..] {
            assert_eq!(
                *state,
                BodyState::Destroyed {
                    cause: DestructionCause::Dispersed,
                    at: end
                }
            );
        }
    }
    assert!(discs > 200, "{discs} discs");
    assert!(giants > 20, "{giants} giants");
}

/// The disc body is a stage of its own: [`generate`] under its rules lists it in belt slot `0xE0`
/// as `DISC`, with every other record as without it, bit for bit, and `body_at` agrees with the
/// snapshot, for a system younger than any disc lives (0.3 Myr at least, P06.T15.c) and for a
/// Sun, whose disc has long gone.
#[test]
fn the_disc_body_moves_no_other_record() {
    for (index, age, state) in [(2, 2.5e5, Rank::Present), (3, 4.57e9, Rank::Gone)] {
        assert_the_disc_moves_no_other_record(&star(index, 1.0, age), state);
    }
}

/// Where a disc body is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rank {
    Present,
    Gone,
}

fn assert_the_disc_moves_no_other_record(ctx: &SystemContext, expected: Rank) {
    let held = with_protoplanetary_disc(ProtoplanetaryDiscRules::Held, || generate(SEED, ctx));
    let with = with_protoplanetary_disc(ProtoplanetaryDiscRules::Disc, || generate(SEED, ctx));
    assert_eq!(held.protoplanetary_disc(), None);
    let disc = *with.protoplanetary_disc().expect("a disc");
    let mut indices = held.indices();
    indices.push(disc.index());
    indices.sort_unstable();
    assert_eq!(with.indices(), indices);
    let (a, b) = (
        held.snapshot_at(ctx, UniverseTime::EPOCH),
        with.snapshot_at(ctx, UniverseTime::EPOCH),
    );
    let others: Vec<&BodyRecord> = b
        .bodies()
        .iter()
        .filter(|record| record.index() != disc.index())
        .collect();
    assert_eq!(others, a.bodies().iter().collect::<Vec<_>>());
    let record = b.body(disc.index()).expect("the snapshot lists the disc");
    let rank = match record.identity().state() {
        BodyState::Present => Rank::Present,
        BodyState::Destroyed {
            cause: DestructionCause::Dispersed,
            ..
        } => Rank::Gone,
        other => panic!("a disc is present or dispersed, not {other:?}"),
    };
    assert_eq!(rank, expected);
    assert_eq!(
        record.identity().label(),
        &Section::Ok(BodyLabel::protoplanetary_disc())
    );
    assert_eq!(
        &with
            .body_at(ctx, disc.index(), UniverseTime::EPOCH)
            .expect("a body"),
        record
    );
    assert_eq!(
        with.position_at(ctx, disc.index(), UniverseTime::EPOCH),
        Ok(None)
    );
    assert_eq!(with.surface_seed(disc.index()), Ok(None));
}

/// A white dwarf of `cooling` years' cooling age, born of a star of `mass` M☉, as the system
/// `index`, and the clock time of its birth.
fn white_dwarf(index: u32, mass: f64, cooling: f64) -> (SystemContext, UniverseTime) {
    let probe = star(index, mass, 1.2e10);
    let death = probe.stars()[0]
        .death()
        .expect("a star of over 1.2 M☉ is dead by 12 Gyr")
        .age()
        .value();
    let ctx = star(index, mass, death + cooling);
    let birth = at_age(&ctx, death);
    (ctx, birth)
}

/// Whether the white dwarf host of `system` keeps a belt and a planet at `t`.
fn keeps_a_belt_and_a_planet(
    system: &PlanetarySystem,
    ctx: &SystemContext,
    t: UniverseTime,
) -> bool {
    let host = OrbitHost::Star(0);
    let hosts: Vec<FateHost<'_>> = system
        .zones
        .iter()
        .map(|zone| fate_host(ctx, zone))
        .collect();
    let fates = system.fates(&hosts);
    let planet = system.bodies.iter().zip(&fates).any(|(body, fate)| {
        matches!(body.part, Part::Planet(_))
            && body.host == host
            && fate
                .as_ref()
                .is_some_and(|fate| fate.at(t).state() == BodyState::Present)
    });
    planet
        && system.belts().iter().any(|belt| {
            system
                .body_at(ctx, belt.index(), t)
                .expect("a belt's record")
                .identity()
                .state()
                == BodyState::Present
        })
}

/// P14.T28.d's test (d): 25–50% of white dwarfs of 1–3 Gyr cooling age with a belt and a planet
/// are polluted, and none without them.
#[test]
fn a_quarter_to_a_half_of_white_dwarfs_with_a_belt_and_a_planet_are_polluted() {
    let mut lcg = Lcg::new(0x28d);
    let (mut eligible, mut polluted) = (0_u32, 0_u32);
    for i in 0..1_000_u32 {
        let mass = 1.2 + 2.8 * lcg.next_f64();
        let cooling = 1.0e9 + 2.0e9 * lcg.next_f64();
        let (ctx, _) = white_dwarf(10_000 + i, mass, cooling);
        let system =
            with_white_dwarf_debris(WhiteDwarfDebrisRules::Debris, || generate(SEED, &ctx));
        let wd = system
            .white_dwarf()
            .expect("a single white dwarf is marked");
        assert_eq!(wd.host(), OrbitHost::Star(0));
        let is = system.is_polluted_at(&ctx, UniverseTime::EPOCH);
        assert_eq!(is, wd.marks().is_polluted());
        if keeps_a_belt_and_a_planet(&system, &ctx, UniverseTime::EPOCH) {
            eligible += 1;
            polluted += u32::from(is);
        } else {
            assert!(!is, "{:?} is polluted with nothing to pollute it", ctx.id());
        }
    }
    let share = f64::from(polluted) / f64::from(eligible);
    assert!(
        eligible > 250,
        "{eligible} white dwarfs with a belt and a planet"
    );
    assert!(
        (0.25..=0.5).contains(&share),
        "{polluted} of {eligible} polluted, {share}"
    );
    let sigma = (POLLUTED_SHARE * (1.0 - POLLUTED_SHARE) / f64::from(eligible)).sqrt();
    assert!((share - POLLUTED_SHARE).abs() < 4.0 * sigma, "{share}");
}

/// A debris disc is a body in belt slot `0xEE`, listed as `DEBRIS DISC`: not yet formed before its
/// white dwarf's birth, present from it, between its sublimation radius and its Roche limit of
/// about 1 R☉, and dispersed once its span of cooling age ends; it moves no other record.
#[test]
fn a_debris_disc_lies_inside_its_white_dwarf_s_roche_limit_from_its_birth() {
    let mut lcg = Lcg::new(0x28d0);
    let solar_radius = Metres::from(crate::units::SolarRadii::new(1.0)).value();
    let mut found = 0;
    for i in 0..4_000_u32 {
        let index = 20_000 + i;
        // Only systems whose marks would give a disc are generated.
        let draws = WhiteDwarfDraws::for_system(SEED, id(index));
        if WhiteDwarfMarks::decide(&draws, Survivors::BeltAndPlanet)
            .disc_until()
            .is_none()
        {
            continue;
        }
        let mass = 1.2 + 2.8 * lcg.next_f64();
        let (ctx, birth) = white_dwarf(index, mass, 2.0e8);
        let system =
            with_white_dwarf_debris(WhiteDwarfDebrisRules::Debris, || generate(SEED, &ctx));
        let wd = *system
            .white_dwarf()
            .expect("a single white dwarf is marked");
        let Some(index) = wd.debris_disc() else {
            assert!(!keeps_a_belt_and_a_planet(&system, &ctx, birth));
            continue;
        };
        found += 1;
        assert_eq!(u16::from(index), 0xEE00);
        assert!(system.indices().contains(&index));
        assert!(wd.marks().is_polluted());
        let until = wd.marks().disc_until().expect("a disc's span").value();
        let death = ctx.stars()[0].death().expect("a death").age().value();
        let end = at_age(&ctx, death + until);
        let state = |t: UniverseTime| {
            system
                .body_at(&ctx, index, t)
                .expect("the disc's record")
                .identity()
                .state()
        };
        let before = birth.checked_sub(Span::from_seconds(1)).expect("clock");
        assert_eq!(state(before), BodyState::NotYetFormed);
        assert_eq!(state(birth), BodyState::Present);
        assert_eq!(
            state(end),
            BodyState::Destroyed {
                cause: DestructionCause::Dispersed,
                at: end
            }
        );
        let epoch_state = state(UniverseTime::EPOCH);
        let record = system
            .body_at(&ctx, index, UniverseTime::EPOCH)
            .expect("the disc's record");
        assert_eq!(record.identity().kind(), BodyKind::DebrisDisc);
        assert_eq!(record.mass(), &Section::NotModelled);
        assert_eq!(
            record.identity().label(),
            &Section::Ok(BodyLabel::debris_disc())
        );
        match (epoch_state, record.population()) {
            (BodyState::Present, Section::Ok(Population::DebrisDisc(disc))) => {
                let outer = disc.outer_edge().value() / solar_radius;
                assert!((0.6..1.3).contains(&outer), "a Roche limit of {outer} R☉");
                assert!(disc.inner_edge() <= disc.outer_edge());
                assert!(disc.inner_edge().value() > 0.0);
            }
            (BodyState::Destroyed { .. }, Section::NotApplicable) => {}
            (state, population) => panic!("{state:?} with {population:?}"),
        }
        let held = with_white_dwarf_debris(WhiteDwarfDebrisRules::Held, || generate(SEED, &ctx));
        assert_eq!(held.white_dwarf(), None);
        let (a, b) = (
            held.snapshot_at(&ctx, UniverseTime::EPOCH),
            system.snapshot_at(&ctx, UniverseTime::EPOCH),
        );
        assert_eq!(b.body(index), Some(&record));
        let others: Vec<&BodyRecord> = b
            .bodies()
            .iter()
            .filter(|record| record.index() != index)
            .collect();
        assert_eq!(others, a.bodies().iter().collect::<Vec<_>>());
    }
    assert!(found >= 5, "{found} debris discs");
}

/// The system of `ctx` generated twice by `generate_it`: the same system, and at `t` each record
/// of its snapshot the same as `body_at`'s, bit for bit, under the same rules.
fn assert_the_same_twice_and_in_both_queries(
    ctx: &SystemContext,
    t: UniverseTime,
    generate_it: impl Fn() -> PlanetarySystem,
) -> PlanetarySystem {
    let (a, b) = (generate_it(), generate_it());
    assert_eq!(a, b, "{:?}", ctx.id());
    let snapshot = a.snapshot_at(ctx, t);
    for (record, index) in snapshot.bodies().iter().zip(a.indices()) {
        let alone = a.body_at(ctx, index, t).expect("a body of the system");
        assert_eq!(format!("{record:?}"), format!("{alone:?}"));
    }
    a
}

/// The disc bodies are the same twice and in both queries (the sim-determinism skill's rule): a
/// young star's protoplanetary disc, and a young white dwarf's debris disc, each generated afresh
/// under its rules.
#[test]
fn the_disc_bodies_are_the_same_twice_and_in_both_queries() {
    let young = star(30_000, 1.0, 1.0e6);
    let system = with_protoplanetary_disc(ProtoplanetaryDiscRules::Disc, || {
        assert_the_same_twice_and_in_both_queries(&young, UniverseTime::EPOCH, || {
            generate(SEED, &young)
        })
    });
    let disc = system.protoplanetary_disc().expect("a young star's disc");
    let record = system
        .body_at(&young, disc.index(), UniverseTime::EPOCH)
        .expect("the disc body");
    assert_eq!(record.identity().state(), BodyState::Present);

    let mut lcg = Lcg::new(0x28d1);
    let debris = (30_001..34_000_u32).find_map(|index| {
        let draws = WhiteDwarfDraws::for_system(SEED, id(index));
        WhiteDwarfMarks::decide(&draws, Survivors::BeltAndPlanet)
            .disc_until()
            .is_some()
            .then(|| white_dwarf(index, 1.2 + 2.8 * lcg.next_f64(), 1.0e7).0)
            .filter(|ctx| {
                with_white_dwarf_debris(WhiteDwarfDebrisRules::Debris, || {
                    assert_the_same_twice_and_in_both_queries(ctx, UniverseTime::EPOCH, || {
                        generate(SEED, ctx)
                    })
                })
                .white_dwarf()
                .is_some_and(|wd| wd.debris_disc().is_some())
            })
    });
    assert!(debris.is_some(), "a debris disc in 4,000 systems");
}
