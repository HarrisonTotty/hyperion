//! P14.T28.e's second-generation planets in generated systems, under their hold's switch.

use super::*;
use crate::planetary::fate::BodyState;
use crate::planetary::fate::FateHost;
use crate::planetary::hosts::second_generation::SecondGenerationDraws;
use crate::planetary::label::BodyLabel;
use crate::planetary::record::Section;
use crate::planetary::system::tests::{SEED, id};
use crate::planetary::system::{fate_host, generate, time_at_age};
use crate::planetary::testing::synthetic_star;
use crate::time::{Span, UniverseTime};
use crate::units::consts::{SOLAR_LUMINOSITY_W, STEFAN_BOLTZMANN};
use crate::units::{Dex, SolarMasses};

/// `system` without its second-generation planets: what [`generate`] gave to version 25, since its
/// step 10 only adds them.
fn without_second_generation(system: &PlanetarySystem) -> PlanetarySystem {
    let mut held = system.clone();
    held.bodies.retain(|body| !body.is_second_generation());
    held
}

/// A star of `mass` M☉, `after` years after its death at the epoch, as the system `index`, and
/// its death's clock time.
fn after_death(index: u32, mass: f64, after: f64) -> (SystemContext, UniverseTime) {
    let probe = synthetic_star(
        id(index),
        SolarMasses::new(mass),
        Dex::ZERO,
        Years::new(1.0e8),
    )
    .expect("a synthetic star");
    let death = probe.stars()[0]
        .death()
        .expect("a massive star dies by 100 Myr")
        .age();
    let ctx = synthetic_star(
        id(index),
        SolarMasses::new(mass),
        Dex::ZERO,
        Years::new(death.value() + after),
    )
    .expect("a synthetic star");
    let at = time_at_age(death, ctx.age_at_epoch()).expect("on the clock");
    (ctx, at)
}

/// The first `n` systems from `first` whose host's mark gives a second-generation system and
/// whose star leaves a neutron star, `after` years after its death, each with its death's time.
fn neutron_stars_with_a_mark(
    first: u32,
    n: usize,
    after: f64,
) -> Vec<(SystemContext, UniverseTime)> {
    let mut found = Vec::new();
    let mut i = first;
    while found.len() < n {
        i += 1;
        if !SecondGenerationDraws::for_host(SEED, id(i), 0).has_system() {
            continue;
        }
        let (ctx, death) = after_death(i, 10.0 + f64::from(i % 9), after);
        let leaves_a_neutron_star = ctx.stars()[0]
            .remnant()
            .is_some_and(|remnant| remnant.kind() == RemnantKind::NeutronStar);
        if leaves_a_neutron_star {
            found.push((ctx, death));
        }
    }
    found
}

/// P14.T28.e's test (e), its time: a neutron star's second-generation planets are not formed until
/// 10 Myr after its death, and present from then on, in slots from `0xC0`, lettered after every
/// primordial planet of their host; a host whose mark says none has none.
#[test]
fn a_neutron_star_s_second_generation_appears_ten_myr_after_its_death() {
    let day = Span::from_seconds(86_400);
    let ten_myr = Span::from_seconds_f64(1.0e7 * crate::units::consts::SECONDS_PER_JULIAN_YEAR)
        .expect("a span");
    let mut planets = 0;
    for (ctx, death) in neutron_stars_with_a_mark(30_000, 6, 3.0e7) {
        let held = without_second_generation(&generate(SEED, &ctx));
        assert!(
            held.bodies()
                .iter()
                .all(|body| !body.is_second_generation())
        );
        let system = generate(SEED, &ctx);
        let second: Vec<&Body> = system
            .bodies()
            .iter()
            .filter(|body| body.is_second_generation())
            .collect();
        let has_disc = system
            .disc(OrbitHost::Star(0))
            .is_some_and(|disc| disc.profile().is_some());
        assert_eq!(!second.is_empty(), has_disc, "{:?}", ctx.id());
        let hosts: Vec<FateHost<'_>> = system
            .zones()
            .iter()
            .map(|zone| fate_host(&ctx, zone))
            .collect();
        let fates = system.fates(&hosts);
        let formation = death.checked_add(ten_myr).expect("on the clock");
        let labels = crate::planetary::label::labels(&system);
        let label_of = |index| {
            labels
                .iter()
                .find(|(at, _)| *at == index)
                .map(|(_, label)| label.clone())
                .expect("a label")
        };
        let letter = |label: &BodyLabel| label.as_str().trim_start_matches("A ").to_owned();
        let last_primordial = system
            .planets()
            .filter(|body| !body.is_second_generation())
            .map(|body| letter(&label_of(body.index())))
            .max_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)));
        for (k, body) in second.iter().enumerate() {
            planets += 1;
            assert_eq!(
                body.index().slot(),
                BodySlot::SecondGeneration(u8::try_from(k).expect("small"))
            );
            assert_eq!(body.host(), OrbitHost::Star(0));
            assert_eq!(system.satellites_of(body.index()).moons(), []);
            let fate = system
                .bodies()
                .iter()
                .zip(&fates)
                .find(|(b, _)| b.index() == body.index())
                .and_then(|(_, fate)| fate.as_ref())
                .expect("a planet has a history");
            let before = formation.checked_sub(day).expect("clock");
            let after = formation.checked_add(day).expect("clock");
            assert_eq!(fate.at(before).state(), BodyState::NotYetFormed);
            assert_eq!(fate.at(after).state(), BodyState::Present);
            assert_eq!(fate.at(UniverseTime::EPOCH).state(), BodyState::Present);
            // Its orbit holds: nothing after the death reaches it.
            assert_eq!(fate.at(after).orbit(), fate.at(UniverseTime::EPOCH).orbit());
            let mine = letter(&label_of(body.index()));
            if let Some(last) = &last_primordial {
                assert!(
                    (mine.len(), &mine) > (last.len(), last),
                    "{mine} after {last}"
                );
            }
        }
    }
    assert!(planets >= 6, "{planets} second-generation planets");
}

/// P14.T28.e: a second-generation planet is derived by P14.T16 about its neutron star lit by its
/// pulsar's spin-down luminosity alone, its record at the epoch whole, and every record of the
/// system not about the pulsar is as without them, bit for bit.
#[test]
fn a_second_generation_planet_is_lit_by_its_pulsar_s_spin_down_alone() {
    let mut derived = 0;
    for (ctx, _) in neutron_stars_with_a_mark(40_000, 4, 3.0e7) {
        let held = without_second_generation(&generate(SEED, &ctx));
        let system = generate(SEED, &ctx);
        let Some(star) = system.pulsar_host() else {
            continue;
        };
        let t = UniverseTime::EPOCH;
        let spin_down = ctx.stars()[star]
            .pulsar_at(t)
            .expect("a neutron star then")
            .spin_down_luminosity()
            .value();
        let light = Epoch::new(&system, &ctx, t)
            .light(star)
            .expect("the pulsar shines");
        assert!((light.luminosity().value() * SOLAR_LUMINOSITY_W / spin_down - 1.0).abs() < 1e-12);
        let thermal = Epoch::new(&held, &ctx, t).light(star).expect("a state");
        assert_ne!(thermal.luminosity(), light.luminosity());
        let (a, b) = (held.snapshot_at(&ctx, t), system.snapshot_at(&ctx, t));
        for body in system
            .bodies()
            .iter()
            .filter(|body| body.is_second_generation())
        {
            derived += 1;
            let record = b.body(body.index()).expect("the snapshot lists it");
            assert_eq!(
                &system.body_at(&ctx, body.index(), t).expect("a body"),
                record
            );
            assert_eq!(record.identity().state(), BodyState::Present);
            let Section::Ok(bulk) = record.bulk() else {
                panic!("a present planet's bulk: {:?}", record.bulk())
            };
            let orbit = record.orbit().ok().expect("a present planet's orbit");
            let a_m = orbit.elements().semi_major_axis().value();
            // Its equilibrium temperature is its pulsar's spin-down's at its Bond albedo, to the
            // stand-in albedo's spread between its photometry and its derivation.
            let flux = spin_down / (4.0 * core::f64::consts::PI * a_m * a_m);
            let Section::Ok(photometry) = record.photometry() else {
                panic!("a present planet's photometry: {:?}", record.photometry())
            };
            let absorbed = 1.0 - photometry.bond_albedo().value();
            let expected = crate::math::powf(flux * absorbed / (4.0 * STEFAN_BOLTZMANN), 0.25);
            let t_eq = bulk.equilibrium_temperature().value();
            assert!(
                (t_eq / expected - 1.0).abs() < 0.02,
                "{t_eq} K against {expected} K"
            );
            assert!(
                system
                    .rotation_of(&ctx, body.index())
                    .expect("a body")
                    .is_some()
            );
        }
        for record in a.bodies() {
            let about_the_pulsar = record.identity().state() == BodyState::Present
                && record.identity().parent() == Some(OrbitHost::Star(0));
            if !about_the_pulsar {
                assert_eq!(b.body(record.index()), Some(record));
            }
        }
    }
    assert!(derived >= 4, "{derived} second-generation planets derived");
}

/// A neutron star's second-generation planets are the same twice and in both queries (the
/// sim-determinism skill's rule), each system generated afresh under their rules.
#[test]
fn the_second_generation_is_the_same_twice_and_in_both_queries() {
    let mut planets = 0;
    for (ctx, _) in neutron_stars_with_a_mark(50_000, 3, 3.0e7) {
        let [a, b] = [(); 2].map(|()| generate(SEED, &ctx));
        assert_eq!(a, b, "{:?}", ctx.id());
        planets += a
            .bodies()
            .iter()
            .filter(|body| body.is_second_generation())
            .count();
        let snapshot = { a.snapshot_at(&ctx, UniverseTime::EPOCH) };
        for (record, index) in snapshot.bodies().iter().zip(a.indices()) {
            let alone =
                { a.body_at(&ctx, index, UniverseTime::EPOCH) }.expect("a body of the system");
            assert_eq!(format!("{record:?}"), format!("{alone:?}"));
        }
    }
    assert!(
        planets > 0,
        "no second-generation planet in three marked systems"
    );
}
