//! The fate transform's tests (P14.T28.a–c): the state-sequence prefix, continuity except at a
//! supernova, formation on young hosts, engulfment along a 1 M☉ track, and supernovae with no kick.

use std::f64::consts::TAU;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::lcg::Lcg;

use super::*;
use crate::orbit::{Eccentricity, Orientation};
use crate::planetary::hosts::young::FormationDraws;
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::units::{AstronomicalUnits, JupiterMasses, Megayears, Radians};

const JUPITER_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(1_326.0);
const EARTH_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(5_513.0);

fn star(mass: f64, age_at_epoch: f64) -> StarModel {
    StarModel::new(
        SolarMasses::new(mass),
        Composition::SOLAR,
        StarDraws::median(),
        Years::new(age_at_epoch),
    )
    .expect("a valid star")
}

fn orientation(i: f64, node: f64, peri: f64) -> Orientation {
    Orientation::new(Radians::new(i), Radians::new(node), Radians::new(peri)).expect("valid")
}

/// A body on a primordial orbit of `a_au` and `e` about `host_mass` M☉, formed as a small planet
/// of a disc of `disc_myr`, or as a giant at the median rank where its mass makes it one.
fn body(a_au: f64, e: f64, mass: EarthMasses, host_mass: f64, disc_myr: f64) -> FateBody {
    let orbit = KeplerElements::from_semi_major_axis(
        Metres::from(AstronomicalUnits::new(a_au)),
        GravitationalParameter::from_solar_masses(SolarMasses::new(host_mass)),
        Eccentricity::new(e).expect("valid"),
        orientation(0.4, 1.3, 2.2),
        Radians::new(0.9),
    )
    .expect("a valid orbit");
    let formation = Formation::from_draws(mass, Megayears::new(disc_myr), &FormationDraws::MEDIAN)
        .expect("valid");
    let density = if mass.value() > 30.0 {
        JUPITER_DENSITY
    } else {
        EARTH_DENSITY
    };
    FateBody::new(formation, orbit, mass, density).expect("valid")
}

fn earth_mass() -> EarthMasses {
    EarthMasses::new(1.0)
}

fn jupiter_mass() -> EarthMasses {
    EarthMasses::from(JupiterMasses::new(1.0))
}

/// The clock time at which `model` is `age` years old.
fn at_age(model: &StarModel, age: f64) -> UniverseTime {
    clock_time(model, Years::new(age)).expect("on the clock")
}

fn after(t: UniverseTime, seconds: i64) -> UniverseTime {
    t.checked_add(Span::from_seconds(seconds))
        .expect("on the clock")
}

fn axis_au(fate: &FateAt) -> f64 {
    fate.orbit().expect("present").semi_major_axis().value() / crate::units::consts::METRES_PER_AU
}

/// The rank of a state in the sequence not yet formed → present → gone.
fn rank(state: BodyState) -> u8 {
    match state {
        BodyState::NotYetFormed => 0,
        BodyState::Present => 1,
        BodyState::Destroyed { .. } | BodyState::Unbound { .. } => 2,
    }
}

/// Clock times through `model`'s whole life to the end of the clock window: even in log age, and
/// dense in the time left to its death, where its giant phases are.
fn life_times(model: &StarModel) -> Vec<UniverseTime> {
    let age_end = model.age_at(ClockWindow::END).value();
    let death = model
        .state_at(ClockWindow::END)
        .filter(|s| s.phase().is_remnant())
        .and_then(|_| model.lifetime())
        .map(Years::value);
    let mut ages: Vec<f64> = (0..=60)
        .map(|i| 1e5 * crate::math::exp10(f64::from(i) * 0.09))
        .filter(|age| *age < age_end)
        .collect();
    if let Some(death) = death {
        for i in 0..=48 {
            let before = death * crate::math::exp10(-f64::from(i) * 0.25);
            ages.push(death - before);
        }
        ages.extend([death + 1.0, death + 1e6, age_end]);
    }
    ages.push(age_end);
    ages.retain(|age| *age > 0.0 && *age <= age_end);
    ages.sort_by(f64::total_cmp);
    ages.dedup_by(|a, b| a.total_cmp(b).is_eq());
    let mut times: Vec<UniverseTime> = ages
        .into_iter()
        .map(|age| at_age(model, age).min(ClockWindow::END))
        .collect();
    times.dedup();
    times
}

/// Hosts across the stellar range, each old enough to have lived its life or aged 13.5 Gyr.
fn hosts() -> Vec<StarModel> {
    [
        (0.08, 5e9),
        (0.5, 13.5e9),
        (1.0, 13.5e9),
        (2.0, 3e9),
        (3.0, 1e9),
        (8.0, 1e8),
        (12.0, 5e7),
        (15.0, 5e7),
        (20.0, 3e7),
        (25.0, 3e7),
    ]
    .into_iter()
    .map(|(mass, age)| star(mass, age))
    .collect()
}

/// Bodies across masses, orbits and eccentricities about a host of `host_mass`.
fn bodies(host_mass: f64, lcg: &mut Lcg) -> Vec<FateBody> {
    let mut bodies = Vec::new();
    for a in [0.03, 0.1, 0.4, 0.9, 1.3, 2.0, 4.0, 10.0, 30.0, 100.0, 400.0] {
        for mass in [0.3, 3.0, 40.0, 1_000.0] {
            let e = 0.5 * lcg.next_f64();
            let disc = 0.3 + 10.0 * lcg.next_f64();
            let b = body(a, e, EarthMasses::new(mass), host_mass, disc);
            let tidal =
                Circularisation::new(Years::new(crate::math::exp10(6.0 + 6.0 * lcg.next_f64())))
                    .expect("positive");
            bodies.push(b.with_circularisation(tidal));
        }
    }
    bodies
}

#[test]
fn state_sequences_are_prefixes_of_formed_present_gone() {
    let mut lcg = Lcg::new(0x28);
    let mut endings = [0_u32; 3];
    for host in hosts() {
        let fate_host = FateHost::star(&host);
        for body in bodies(host.initial_mass().value(), &mut lcg) {
            let fate = BodyFate::resolve(&body, &fate_host);
            let mut last = BodyState::NotYetFormed;
            for t in life_times(&host) {
                let now = fate.at(t);
                let state = now.state();
                assert!(rank(state) >= rank(last), "{last:?} then {state:?} at {t}");
                if rank(last) == 2 {
                    assert_eq!(state, last, "a body that is gone stays gone");
                }
                if let Some(at) = state.ended_at() {
                    assert!(at <= t, "ended at {at} but asked at {t}");
                }
                assert_eq!(now.orbit().is_some(), state == BodyState::Present);
                last = state;
            }
            match fate.ending() {
                Some(BodyState::Destroyed {
                    cause: DestructionCause::Engulfed,
                    ..
                }) => endings[0] += 1,
                Some(BodyState::Unbound { .. }) => endings[1] += 1,
                Some(_) => endings[2] += 1,
                None => {}
            }
        }
    }
    // The sample reaches every ending but the rare disruption.
    assert!(endings[0] > 20 && endings[1] > 20, "endings {endings:?}");
}

#[test]
fn elements_are_continuous_in_time_except_at_a_supernova() {
    let mut lcg = Lcg::new(0x2801);
    for host in hosts() {
        let fate_host = FateHost::star(&host);
        let death = death_of(&host);
        for body in bodies(host.initial_mass().value(), &mut lcg) {
            let fate = BodyFate::resolve(&body, &fate_host);
            for t in life_times(&host) {
                let later = after(t, 31_557_600);
                if later > ClockWindow::END {
                    continue;
                }
                let (now, next) = (fate.at(t), fate.at(later));
                let (Some(a), Some(b)) = (now.orbit(), next.orbit()) else {
                    continue;
                };
                // A sudden death, or a death at which the track sheds an envelope at once (ruling
                // 57.1's super-AGB stars), steps the orbit; nothing else does.
                if death.is_some_and(|d| {
                    t < d.at && d.at <= later && (d.sudden || (d.before - d.after).value() > 1e-6)
                }) {
                    continue;
                }
                let da = b.semi_major_axis() / a.semi_major_axis() - 1.0;
                let de = b.eccentricity().value() - a.eccentricity().value();
                assert!(da.abs() < 1e-3, "a jumps by {da:e} in a year at {t}");
                assert!(de.abs() < 1e-3, "e jumps by {de:e} in a year at {t}");
                assert_same_bits(a.inclination().value(), b.inclination().value());
                assert_same_bits(a.ascending_node().value(), b.ascending_node().value());
            }
        }
    }
}

#[test]
fn a_supernova_steps_the_orbit() {
    // 20 M☉ collapses to a black hole of 6.81 M☉ from 8.30 M☉: a circular orbit keeps its radius
    // as its new pericentre and takes e = ΔM ÷ M_after = 0.219.
    let host = star(20.0, 3e7);
    let death = death_of(&host).expect("dead by the epoch");
    assert!(death.sudden);
    let wide = body(300.0, 0.0, jupiter_mass(), 20.0, 0.3);
    let fate_host = FateHost::star(&host);
    let fate = BodyFate::resolve(&wide, &fate_host);
    let before = fate.at(after(death.at, -1));
    let later = fate.at(death.at);
    // The death was 20 Myr ago, outside the clock window, so no orbit says it holds until then.
    assert_eq!(before.valid_until(), None);
    assert_eq!(fate.segments[1].start, death.at);
    let (a, b) = (
        before.orbit().expect("present"),
        later.orbit().expect("bound"),
    );
    let e = (death.before - death.after).value() / death.after.value();
    assert!((e - 0.219).abs() < 0.001, "{e}");
    assert!((b.eccentricity().value() - e).abs() < 1e-6);
    assert_same_bits(a.eccentricity().value(), 0.0);
    assert!((b.periapsis() / a.semi_major_axis() - 1.0).abs() < 1e-6);
}

/// Test (b): along a 1 M☉ track, planets inside about 1 au at birth are destroyed by the tip of
/// the asymptotic giant branch, and survivors end at a₀ × M₀ ÷ `M_wd`.
///
/// "About 1 au" is the Jovian limit, 1.17 au; with ruling 62's reach a rocky planet must be born
/// inside 0.68 au.
#[test]
fn along_a_solar_track_the_inner_planets_are_engulfed_and_the_rest_widen() {
    let sun = star(1.0, 13.5e9);
    let host = FateHost::star(&sun);
    let death = death_of(&sun).expect("the Sun is a white dwarf at 13.5 Gyr");
    let white_dwarf = sun.remnant().expect("a remnant").mass();
    assert!(!death.sudden);
    assert!((white_dwarf.value() - 0.52).abs() < 0.01);
    for (mass, inner) in [(earth_mass(), 0.6), (jupiter_mass(), 1.1)] {
        for a in [0.02, 0.05, 0.1, 0.3, 0.5, 0.6, 0.9, 1.0, 1.1] {
            if a > inner {
                continue;
            }
            let planet = body(a, 0.0, mass, 1.0, 2.0);
            let fate = BodyFate::resolve(&planet, &host);
            let Some(BodyState::Destroyed { cause, at }) = fate.ending() else {
                panic!("a planet of {mass:?} at {a} au survives the Sun");
            };
            assert_eq!(cause, DestructionCause::Engulfed);
            assert!(at <= death.at);
        }
        for a in [1.5, 2.5, 5.0, 10.0, 30.0, 100.0] {
            let planet = body(a, 0.0, mass, 1.0, 2.0);
            let now = state_at(&planet, &host, UniverseTime::EPOCH);
            assert_eq!(now.state(), BodyState::Present, "{mass:?} at {a} au");
            let expected = a * (1.0 / white_dwarf.value());
            assert!(
                (axis_au(&now) / expected - 1.0).abs() < 1e-9,
                "{mass:?} at {a} au ends at {} au, not {expected}",
                axis_au(&now)
            );
        }
    }
}

/// The largest a₀ engulfed about a host of `host_mass` M☉ 13.5 Gyr old, or at least dead, for a
/// planet of `mass`, to 10⁻⁴ au.
fn engulfment_limit(host_mass: f64, mass: EarthMasses) -> f64 {
    let model = star(host_mass, 13.5e9);
    let host = FateHost::star(&model);
    let engulfed = |a: f64| {
        BodyFate::resolve(&body(a, 0.0, mass, host_mass, 2.0), &host)
            .ending()
            .is_some()
    };
    let (mut lo, mut hi) = (0.1, 10.0);
    assert!(engulfed(lo) && !engulfed(hi));
    while hi - lo > 1e-4 {
        let mid = f64::midpoint(lo, hi);
        if engulfed(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

#[test]
fn the_engulfment_limits_follow_the_tracks_radii_times_mustill_and_villavers_reach() {
    // 1 M☉: the red-giant tip, 0.862 au at 0.759 M☉, sets every limit: f × 0.862 × 0.759 au.
    let rocky = engulfment_limit(1.0, earth_mass());
    let neptune = engulfment_limit(1.0, EarthMasses::new(17.1));
    let jovian = engulfment_limit(1.0, jupiter_mass());
    for (limit, f) in [(rocky, 1.036), (neptune, 1.264), (jovian, 1.786)] {
        let expected = f * 0.862 * 0.759;
        assert!(
            (limit / expected - 1.0).abs() < 0.01,
            "{limit} au against {expected}"
        );
    }
    // 1.5 and 2 M☉: the AGB sets them. Mustill and Villaver's critical axes at the start of the
    // AGB, over their stars' largest AGB radius, are 0.78, 0.93 and 1.32 at 1.5 M☉ and 0.71, 0.83
    // and 1.18 at 2 M☉; the tracks' share the pattern, Jovian over Terrestrial 1.72 against 1.71
    // and 1.65.
    for host in [1.5, 2.0] {
        let t = engulfment_limit(host, earth_mass());
        let n = engulfment_limit(host, EarthMasses::new(17.1));
        let j = engulfment_limit(host, jupiter_mass());
        assert!((n / t - 1.22).abs() < 0.02, "{host} M_sun: {n} over {t}");
        assert!((j / t - 1.72).abs() < 0.03, "{host} M_sun: {j} over {t}");
    }
}

#[test]
fn a_planet_engulfed_at_the_red_giant_tip_stays_destroyed_though_the_winds_widen_its_orbit() {
    // At 0.65 au an Earth is inside the red-giant tip's reach there, and outside the largest
    // radius's reach by the end, 1.25 au against 1.05: the scan must find the tip.
    let sun = star(1.0, 13.5e9);
    let host = FateHost::star(&sun);
    let planet = body(0.65, 0.0, earth_mass(), 1.0, 2.0);
    let fate = BodyFate::resolve(&planet, &host);
    let Some(BodyState::Destroyed { cause, at }) = fate.ending() else {
        panic!("the planet is engulfed at the red-giant tip");
    };
    assert_eq!(cause, DestructionCause::Engulfed);
    let age = sun.age_at(at).value();
    assert!((12.2e9..12.33e9).contains(&age), "engulfed at {age:e} yr");
    let segment = fate.segments[0];
    let end = fate.at(after(at, -1));
    let reach = engulfment_reach(earth_mass());
    let final_axis = fate
        .orbit_in(&segment, ClockWindow::END, fate.host_mass(ClockWindow::END))
        .semi_major_axis()
        .value();
    assert!(final_axis > reach * fate.largest_radius(&segment, ClockWindow::END));
    assert_eq!(end.state(), BodyState::Present);
}

/// Test (c): a planet of a star that loses over half its mass at once with no kick is unbound
/// from a circular orbit, and black holes from complete fallback keep their bodies.
#[test]
fn a_neutron_star_unbinds_its_planets_and_a_fallback_black_hole_keeps_them() {
    let neutron = star(12.0, 5e7);
    let death = death_of(&neutron).expect("dead by the epoch");
    assert!(death.sudden && death.after.value() < 0.5 * death.before.value());
    let host = FateHost::star(&neutron);
    for a in [60.0, 200.0, 1_000.0] {
        let planet = body(a, 0.0, jupiter_mass(), 12.0, 0.3);
        assert_eq!(
            state_at(&planet, &host, UniverseTime::EPOCH).state(),
            BodyState::Unbound { at: death.at },
            "at {a} au"
        );
    }
    // 25 M☉ collapses with complete fallback, losing 10⁻⁸ of its mass.
    let hole = star(25.0, 3e7);
    let death = death_of(&hole).expect("dead by the epoch");
    assert!(death.sudden);
    assert!((death.before - death.after).value() < 1e-6 * death.before.value());
    let host = FateHost::star(&hole);
    for a in [60.0, 200.0, 1_000.0] {
        let planet = body(a, 0.0, jupiter_mass(), 25.0, 0.3);
        let fate = BodyFate::resolve(&planet, &host);
        let before = fate.at(after(death.at, -1));
        let now = fate.at(UniverseTime::EPOCH);
        assert_eq!(now.state(), BodyState::Present, "at {a} au");
        let (b, n) = (
            before.orbit().expect("present"),
            now.orbit().expect("bound"),
        );
        assert!((n.semi_major_axis() / b.semi_major_axis() - 1.0).abs() < 1e-6);
        assert!(n.eccentricity().value() < 1e-6);
    }
    // Inside the supergiant's reach, 1.79 times its 7.2 au at 8.3 M☉, which a Jupiter born inside
    // 4.2 au reaches as the winds widen its orbit, nothing is left.
    let close = body(3.0, 0.0, jupiter_mass(), 25.0, 0.3);
    assert!(matches!(
        BodyFate::resolve(&close, &host).ending(),
        Some(BodyState::Destroyed {
            cause: DestructionCause::Engulfed,
            ..
        })
    ));
}

#[test]
fn a_body_forms_at_its_formation_age() {
    // A 1 M☉ star 2 Myr old whose disc lives 500 years more: a small planet forms then, and a
    // giant of the median rank formed at the geometric mean of 0.5 Myr and the lifetime.
    let lifetime = 2.0 + 500.0 / 1e6;
    let young = star(1.0, 2e6);
    let host = FateHost::star(&young);
    let small = body(1.0, 0.1, earth_mass(), 1.0, lifetime);
    let fate = BodyFate::resolve(&small, &host);
    let formed = fate.formed_at().expect("forms");
    assert_eq!(
        Some(formed),
        clock_time(&young, Years::from(Megayears::new(lifetime)))
    );
    assert!(formed > UniverseTime::EPOCH && formed <= ClockWindow::END);
    let now = fate.at(UniverseTime::EPOCH);
    assert_eq!(now.state(), BodyState::NotYetFormed);
    assert_eq!(now.valid_until(), Some(formed));
    let just_before = formed
        .checked_sub(Span::new(0, 1).expect("valid"))
        .expect("on the clock");
    assert_eq!(fate.at(just_before).state(), BodyState::NotYetFormed);
    let born = fate.at(formed);
    assert_eq!(born.state(), BodyState::Present);
    assert_eq!(born.valid_until(), None);
    // On the main sequence the Sun has lost nothing, so the orbit is the primordial one.
    assert_eq!(born.orbit(), Some(small.orbit()));
    let giant = body(5.0, 0.05, jupiter_mass(), 1.0, lifetime);
    let giant_formed = BodyFate::resolve(&giant, &host).formed_at().expect("forms");
    let giant_age = young.age_at(giant_formed).value();
    assert!((giant_age / (0.5e6 * lifetime * 1e6).sqrt() - 1.0).abs() < 1e-9);
    assert_eq!(
        state_at(&giant, &host, UniverseTime::EPOCH).state(),
        BodyState::Present
    );
}

#[test]
fn every_body_of_a_system_not_yet_born_is_not_yet_formed() {
    let unborn = star(1.0, -2e6);
    let host = FateHost::star(&unborn);
    let planet = body(1.0, 0.0, earth_mass(), 1.0, 3.0);
    for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
        let now = state_at(&planet, &host, t);
        assert_eq!(now.state(), BodyState::NotYetFormed);
        assert_eq!(now.valid_until(), None);
        assert_eq!(now.orbit(), None);
    }
}

#[test]
fn a_circumbinary_planet_orbits_the_pairs_total_mass() {
    let (a, b) = (star(1.0, 13.5e9), star(0.5, 13.5e9));
    let host = FateHost::stars([&a, &b]).expect("coeval");
    let planet = body(10.0, 0.0, jupiter_mass(), 1.5, 2.0);
    let now = state_at(&planet, &host, UniverseTime::EPOCH);
    let white_dwarf = a.remnant().expect("dead").mass().value();
    let red_dwarf = b
        .state_at(UniverseTime::EPOCH)
        .expect("alive")
        .mass()
        .value();
    let expected = 10.0 * (1.5 / (white_dwarf + red_dwarf));
    assert!(
        (axis_au(&now) / expected - 1.0).abs() < 1e-9,
        "{}",
        axis_au(&now)
    );
    // A pair's planet is engulfed by whichever star grows over it.
    let close = body(0.8, 0.0, jupiter_mass(), 1.5, 2.0);
    assert!(matches!(
        BodyFate::resolve(&close, &host).ending(),
        Some(BodyState::Destroyed {
            cause: DestructionCause::Engulfed,
            ..
        })
    ));
    assert_eq!(
        FateHost::stars([&a, &star(0.5, 1e9)]),
        Err(BuildFateHostError::NotCoeval {
            first: Years::new(13.5e9),
            other: Years::new(1e9)
        })
    );
    assert_eq!(
        FateHost::stars(std::iter::empty::<&StarModel>()),
        Err(BuildFateHostError::NoStars)
    );
}

#[test]
fn a_circumbinary_planet_sees_a_companions_supernova_as_the_pairs_mass_loss() {
    // 20 M☉ and 3 M☉: at the black hole's birth the pair drops from 8.30 + 3 to 6.81 + 3 M☉.
    let (big, small) = (star(20.0, 3e7), star(3.0, 3e7));
    let host = FateHost::stars([&big, &small]).expect("coeval");
    let planet = body(400.0, 0.0, jupiter_mass(), 23.0, 0.3);
    let fate = BodyFate::resolve(&planet, &host);
    let death = death_of(&big).expect("dead");
    let other = small.state_at(death.at).expect("alive").mass().value();
    let before = fate.at(after(death.at, -1));
    let now = fate.at(death.at);
    let e = (death.before - death.after).value() / (death.after.value() + other);
    assert!(
        (now.orbit().expect("bound").eccentricity().value() - e).abs() < 1e-6,
        "e = {e}"
    );
    assert_same_bits(before.orbit().expect("present").eccentricity().value(), 0.0);
}

#[test]
fn a_death_in_the_window_is_when_the_orbit_holds_until() {
    let lifetime = death_of(&star(20.0, 3e7)).expect("dead");
    let age = star(20.0, 3e7).age_at(lifetime.at).value() - 500.0;
    let host_star = star(20.0, age);
    let host = FateHost::star(&host_star);
    let planet = body(400.0, 0.0, jupiter_mass(), 20.0, 0.3);
    let death = death_of(&host_star).expect("dead by the end of the window");
    assert!(death.at > UniverseTime::EPOCH);
    let now = state_at(&planet, &host, UniverseTime::EPOCH);
    assert_eq!(now.valid_until(), Some(death.at));
    assert_eq!(
        now.body_orbit().expect("present").valid_until(),
        Some(death.at)
    );
    assert_eq!(state_at(&planet, &host, death.at).valid_until(), None);
}

#[test]
fn the_transform_is_deterministic_and_independent_of_query_order() {
    let sun = star(1.0, 13.5e9);
    let host = FateHost::star(&sun);
    let planet = body(1.6, 0.3, jupiter_mass(), 1.0, 2.0)
        .with_circularisation(Circularisation::new(Years::new(3e9)).expect("positive"));
    let times = life_times(&sun);
    let forward: Vec<FateAt> = times.iter().map(|t| state_at(&planet, &host, *t)).collect();
    let fate = BodyFate::resolve(&planet, &host);
    let backward: Vec<FateAt> = times.iter().rev().map(|t| fate.at(*t)).collect();
    assert!(forward.iter().eq(backward.iter().rev()));
    assert_eq!(BodyFate::resolve(&planet, &host), fate);
}

#[test]
fn circularisation_comes_first() {
    // A hot Jupiter on e = 0.2 with τ_c = 100 Myr is circular at 5 Gyr, at a (1 − e²).
    let sun = star(1.0, 5e9);
    let host = FateHost::star(&sun);
    let hot_jupiter = body(0.04, 0.2, jupiter_mass(), 1.0, 2.0)
        .with_circularisation(Circularisation::new(Years::new(1e8)).expect("positive"));
    let now = state_at(&hot_jupiter, &host, UniverseTime::EPOCH);
    let orbit = now.orbit().expect("present");
    assert!(orbit.eccentricity().value() < 1e-10);
    assert!((axis_au(&now) / (0.04 * 0.96) - 1.0).abs() < 1e-12);
    // The mean anomaly at the epoch is kept; the phase follows the new period.
    assert_same_bits(
        orbit.mean_anomaly_at_epoch().value(),
        hot_jupiter.orbit().mean_anomaly_at_epoch().value(),
    );
    assert!(orbit.mean_anomaly_at_epoch().value() < TAU);
}

#[test]
fn errors_name_what_was_wrong() {
    assert_eq!(
        BuildFateBodyError::MassNotPositive(EarthMasses::new(0.0)).to_string(),
        "body mass 0 M_earth is not finite and positive"
    );
    assert_eq!(
        BuildFateBodyError::DensityNotPositive(KilogramsPerCubicMetre::new(-1.0)).to_string(),
        "body density -1 kg/m^3 is not finite and positive"
    );
    assert_eq!(
        BuildFateHostError::NoStars.to_string(),
        "a host has no stars"
    );
    assert_eq!(
        BuildFateHostError::NotCoeval {
            first: Years::new(1.0),
            other: Years::new(2.0)
        }
        .to_string(),
        "host stars aged 1 yr and 2 yr at the epoch are not coeval"
    );
    let good = body(1.0, 0.0, earth_mass(), 1.0, 2.0);
    assert_eq!(
        FateBody::new(
            *good.formation(),
            *good.orbit(),
            EarthMasses::new(f64::NAN),
            EARTH_DENSITY
        )
        .map_err(|e| e.to_string()),
        Err("body mass NaN M_earth is not finite and positive".to_owned())
    );
    assert_eq!(
        FateBody::new(
            *good.formation(),
            *good.orbit(),
            earth_mass(),
            KilogramsPerCubicMetre::ZERO
        ),
        Err(BuildFateBodyError::DensityNotPositive(
            KilogramsPerCubicMetre::ZERO
        ))
    );
}

#[test]
fn the_engulfment_searchs_axis_is_the_orbits_to_the_bit() {
    let mut lcg = Lcg::new(0x2802);
    for host in [star(1.0, 13.5e9), star(3.0, 1e9), star(20.0, 3e7)] {
        let fate_host = FateHost::star(&host);
        for planet in bodies(host.initial_mass().value(), &mut lcg) {
            let fate = BodyFate::resolve(&planet, &fate_host);
            // A disc that outlives its 20 M☉ star forms nothing.
            let Some(&segment) = fate.segments.first() else {
                assert_eq!(fate.formed_at(), None);
                continue;
            };
            for t in life_times(&host) {
                if t < segment.start || fate.segments.get(1).is_some_and(|s| t >= s.start) {
                    continue;
                }
                let mass = fate.host_mass(t);
                assert_same_bits(
                    fate.axis_in(&segment, t, mass).value(),
                    fate.orbit_in(&segment, t, mass).semi_major_axis().value(),
                );
            }
        }
    }
}

/// Measured for P06.T19's lift of P14.T28.c's zero kick: the share of planets a supernova
/// unbinds around a Kroupa sample of 8–150 M☉ hosts at Z = 0.02 (the kick law's reference
/// sample), with the remnant's natal kick and with the mass loss alone, for Jupiters on circular
/// orbits of 10–300 au that survive to the explosion. Printed for the plan's record: around
/// neutron stars the mass loss alone unbinds nearly all of them, the kick unbinds more around
/// black holes of partial fallback, and after complete fallback it changes nothing.
#[test]
#[ignore = "slow: 800 full tracks of massive stars and their planets' fates"]
fn natal_kicks_unbind_planets_the_mass_loss_alone_would_keep() {
    use crate::Seed;
    use crate::stellar::remnant::RemnantKind;
    use crate::stellar::remnant::reference::ReferencePopulation;

    let pop = ReferencePopulation::default();
    let seed = Seed::new(0x1428_c000_0000_0019);
    // By remnant (neutron star, partial-fallback black hole, complete fallback): planets present
    // just before, unbound with the kick, unbound by the mass loss alone.
    let mut tally = [[0_u32; 3]; 3];
    for i in 0..400 {
        let s = pop.star(seed, i);
        let m0 = s.initial_mass();
        let probe = StarModel::new(m0, Composition::SOLAR, s.draws().clone(), Years::new(1e6))
            .expect("a valid star");
        let life = probe.lifetime().expect("a star dies").value();
        let model = StarModel::new(
            m0,
            Composition::SOLAR,
            s.draws().clone(),
            Years::new(life + 1e6),
        )
        .expect("a valid star");
        let Some(death) = death_of(&model).filter(|d| d.sudden) else {
            continue;
        };
        let row = match (model.remnant().map(|r| r.kind()), model.natal_kick()) {
            (Some(RemnantKind::NeutronStar), _) => 0,
            (Some(RemnantKind::BlackHole), Some(k)) if k.speed().value() > 0.0 => 1,
            (Some(RemnantKind::BlackHole), _) => 2,
            _ => continue,
        };
        let host = FateHost::star(&model);
        for a in [10.0, 30.0, 100.0, 300.0] {
            let planet = body(a, 0.0, jupiter_mass(), m0.value(), 0.3);
            let fate = BodyFate::resolve(&planet, &host);
            let before = fate.at(after(death.at, -1));
            let Some(orbit) = before.orbit() else {
                continue;
            };
            tally[row][0] += 1;
            let kicked = matches!(fate.at(death.at).state(), BodyState::Unbound { .. });
            tally[row][1] += u32::from(kicked);
            let mu_after = GravitationalParameter::from_solar_masses(death.after);
            let alone = matches!(
                supernova(
                    orbit,
                    death.at,
                    mu_after,
                    SystemVelocity::ZERO,
                    JUPITER_DENSITY
                ),
                Aftermath::Unbound
            );
            tally[row][2] += u32::from(alone);
        }
    }
    for (name, [present, kicked, alone]) in
        ["neutron stars", "kicked black holes", "complete fallback"]
            .into_iter()
            .zip(tally)
    {
        let share = |k: u32| f64::from(k) / f64::from(present.max(1));
        eprintln!(
            "{name}: {present} Jupiters at the explosion; unbound {:.3} with the kick, {:.3} by \
             the mass loss alone",
            share(kicked),
            share(alone)
        );
    }
    // A neutron star keeps under half the mass, so its mass loss alone unbinds nearly every
    // circular orbit; a kick along a planet's motion can keep one of them. Partial fallback loses
    // under half, and its kick unbinds more; complete fallback has no kick to add.
    let [ns, bh, fallback] = tally;
    assert!(ns[0] > 0 && 20 * ns[1] >= 19 * ns[0], "{ns:?}");
    assert!(bh[0] > 0 && bh[1] >= bh[2], "{bh:?}");
    assert_eq!(fallback[1], fallback[2], "{fallback:?}");
}
