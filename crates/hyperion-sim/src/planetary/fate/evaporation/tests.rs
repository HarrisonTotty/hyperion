//! The evaporation step's tests (P14.T55.b.a): its rate and temperature read at source, its
//! integration, and the fate transform's small hot rocky bodies under its hold.

use super::*;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::planetary::fate::{
    BodyFate, BodyState, DestructionCause, Evaporation, FateBody, FateHost,
};
use crate::planetary::hosts::young::{Formation, FormationDraws};
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::system::StarModel;
use crate::units::consts::{EARTH_MASS_KG, METRES_PER_AU, SOLAR_RADIUS_M};
use crate::units::{
    AstronomicalUnits, GravitationalParameter, Megayears, Radians, SolarMasses, Years,
};

/// A rocky member's density, kg m⁻³ (the belts' `ROCKY_MEMBER_DENSITY`).
const ROCK: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(2_700.0);

fn sun(age_at_epoch: f64) -> StarModel {
    StarModel::new(
        SolarMasses::new(1.0),
        Composition::SOLAR,
        StarDraws::median(),
        Years::new(age_at_epoch),
    )
    .expect("a valid star")
}

/// A body of `mass` M⊕ formed at a 3 Myr disc's lifetime on a circular orbit of `a_au` about 1 M☉,
/// of rock at [`ROCK`], whose rock evaporates.
fn rock(a_au: f64, mass: f64) -> FateBody {
    let mass = EarthMasses::new(mass);
    let orbit = KeplerElements::from_semi_major_axis(
        Metres::from(AstronomicalUnits::new(a_au)),
        GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).expect("valid"),
        Radians::ZERO,
    )
    .expect("a valid orbit");
    let formation =
        Formation::from_draws(mass, Megayears::new(3.0), &FormationDraws::MEDIAN).expect("valid");
    FateBody::new(formation, orbit, mass, ROCK)
        .expect("valid")
        .evaporating()
}

/// The age of `star` at `t`, Gyr.
fn age_gyr(star: &StarModel, t: UniverseTime) -> f64 {
    star.age_at(t).value() / 1e9
}

/// The age at which `star`'s main sequence ends, Gyr.
fn main_sequence_end_gyr(star: &StarModel) -> f64 {
    star.track()
        .and_then(crate::stellar::sse::Track::main_sequence_end)
        .expect("a solar track ends its main sequence")
        .value()
        / 1e9
}

/// The evaporated bulk is held until the 28 → 29 bump: the rules in force are 28's, and the tests'
/// switch reaches 29's.
#[test]
fn the_evaporated_bulk_is_held_only_until_the_29_bump() {
    assert!(
        GENERATOR_VERSION < EVAPORATED_BULK_VERSION,
        "the 28 → 29 bump reads a present moon's bulk, figure and class at the mass its rock has \
         left, and a belt's largest body at its largest member left: delete \
         `EVAPORATED_BULK_VERSION`, `EvaporatedBulkRules` with its `Formed` arm and its matches in \
         `MoonPart::at_mass_left`, `MoonPart::at_mass` and `PlanetarySystem::belt_record`, the \
         tests' `with_evaporated_bulk`, its line in `HeldRules`, and this test; restate the tests \
         held below 29, and have `a_moon_s_published_conditions_read_its_planet_s_orbit` and \
         `every_moon_s_inventory_is_its_own_ranks_at_its_own_bulk` read a moon's bulk at its mass \
         left (`MoonPart::at_mass_left`), as `moon_laws_are_held_after_at_most_two_rounds` does"
    );
    assert_eq!(EvaporatedBulkRules::in_force(), EvaporatedBulkRules::Formed);
    assert_eq!(
        with_evaporated_bulk(EvaporatedBulkRules::Left, EvaporatedBulkRules::in_force),
        EvaporatedBulkRules::Left
    );
    assert_eq!(
        with_evaporated_bulk(EvaporatedBulkRules::Formed, EvaporatedBulkRules::in_force),
        EvaporatedBulkRules::Formed
    );
}

/// The vapour's flux ρ `c_s` at KIC 12557548b's 2,145 K is PBC13's eqs. 7, 9 and 14 worked in their
/// own CGS units: olivine's vapour pressure of 6.72 × 10¹⁴ exp(−169 `m_H` 3.21 × 10¹⁰ ÷ (k T)) dyn
/// cm⁻², about 33 dyn cm⁻², at μ = 30 `m_H` and γ = 1.3; the radius falls at `f_duty` = 0.5 of it
/// over 4π `ρ_b`.
#[test]
fn the_flux_is_perez_becker_and_chiang_s_at_kic_1255b_s_temperature() {
    let (m_h_g, k_erg): (f64, f64) = (1.673_532_84e-24, 1.380_649e-16);
    let t: f64 = 2_145.0;
    let pressure = 6.72e14 * math::exp(-169.0 * m_h_g * 3.21e10 / (k_erg * t));
    assert!((pressure / 32.5 - 1.0).abs() < 0.01, "{pressure} dyn/cm²");
    let mu = 30.0 * m_h_g;
    let density = mu * pressure / (k_erg * t);
    let sound = (1.3 * k_erg * t / mu).sqrt();
    // g cm⁻² s⁻¹ to kg m⁻² s⁻¹.
    let flux = density * sound * 10.0;
    let rate = radius_rate(t, ROCK);
    let expected = 0.5 * flux / (4.0 * core::f64::consts::PI * ROCK.value());
    assert!(
        (rate / expected - 1.0).abs() < 1e-12,
        "{rate} m/s against {expected}"
    );
    // A 64 km rock (6 × 10⁻⁷ M⊕ at 3,300 kg m⁻³) lasts some 35,000 years there.
    let years = 64e3 / radius_rate(t, KilogramsPerCubicMetre::new(3_300.0)) / 3.155_76e7;
    assert!((2e4..5e4).contains(&years), "{years} yr");
}

/// The free-streaming rate ρ `c_s` R², toward which all PBC13's winds converge (their Figure 2),
/// is above 0.1 M⊕ Gyr⁻¹ at 0.01 M⊕ (Mercury's density, 5.4 g cm⁻³), which their largest rate there
/// exceeds (§3.2), and grows as the radius squared with the mass.
#[test]
fn the_free_stream_bounds_their_winds() {
    let density = KilogramsPerCubicMetre::new(5_400.0);
    let per_gyr = |mass: f64| {
        let radius = radius_of(EarthMasses::new(mass), density);
        // dM/dt = 4π ρ_b R² dR/dt, without the duty cycle.
        let rate = radius_rate(2_145.0, density) / DUTY_CYCLE;
        4.0 * core::f64::consts::PI * density.value() * radius * radius * rate * 3.155_76e16
            / EARTH_MASS_KG
    };
    assert!(per_gyr(0.01) > 0.1, "{} M⊕/Gyr", per_gyr(0.01));
    let ratio = per_gyr(0.008) / per_gyr(0.001);
    assert!((ratio / 4.0 - 1.0).abs() < 1e-9, "{ratio}");
}

/// PBC13's eq. 6 at KIC 12557548's star, T∗ = 4,450 K and R∗ = 0.65 R☉, at a = 0.013 au gives
/// their 2,150 e^(−τ ÷ 4) K, 2,145 K at τ = 0.01, to the rounding of their parameters.
#[test]
fn kic_1255b_s_substellar_temperature_is_eq_6_s() {
    let radius = 0.65 * SOLAR_RADIUS_M;
    let luminosity =
        4.0 * core::f64::consts::PI * radius * radius * STEFAN_BOLTZMANN * math::powi(4_450.0, 4)
            / SOLAR_LUMINOSITY_W;
    let t = substellar_temperature(
        SolarLuminosities::new(luminosity),
        Metres::new(0.013 * METRES_PER_AU),
        0.0,
    );
    assert!((t / 2_145.0 - 1.0).abs() < 0.005, "{t} K");
    // An eccentric orbit's mean light is brighter by 1 ÷ √(1 − e²).
    let eccentric = substellar_temperature(
        SolarLuminosities::new(luminosity),
        Metres::new(0.013 * METRES_PER_AU),
        0.6,
    );
    assert!((eccentric / t - 1.0 / 0.8_f64.sqrt().sqrt()).abs() < 1e-12);
    assert_eq!(
        substellar_temperature(SolarLuminosities::ZERO, Metres::new(1e9), 0.0),
        0.0
    );
}

/// A constant rate takes its radius in radius ÷ rate, bisected to the second; a rate that starts
/// at a break point takes it from there; a stretch whose bound cannot take a share that counts
/// takes nothing; and one that never takes it all says how much it took.
#[test]
fn first_loss_finds_the_time_the_radius_is_lost() {
    let from = UniverseTime::EPOCH;
    let at = |seconds: i64| {
        from.checked_add(Span::from_seconds(seconds))
            .expect("on the clock")
    };
    let to = at(10_000_000);
    let Loss::Gone(lost) = first_loss(
        from,
        to,
        Vec::new,
        1_000.0,
        (|_| 1e-3, |_| 1e-3),
        &mut Vec::new(),
    ) else {
        panic!("a constant rate takes the radius");
    };
    let seconds = lost.checked_since(from).expect("a span").as_seconds_f64();
    assert!((seconds - 1e6).abs() <= 1.0, "{seconds} s");

    let start = at(2_000_000);
    let late = |t: UniverseTime| if t >= start { 1e-3 } else { 0.0 };
    let Loss::Gone(lost) = first_loss(
        from,
        to,
        || vec![start],
        1_000.0,
        (late, late),
        &mut Vec::new(),
    ) else {
        panic!("a rate from a break point takes the radius");
    };
    let seconds = lost.checked_since(from).expect("a span").as_seconds_f64();
    assert!((seconds - 3e6).abs() <= 1.0, "{seconds} s");

    assert_eq!(
        first_loss(
            from,
            to,
            Vec::new,
            1_000.0,
            (|_| 1e-15, |_| 1.0),
            &mut Vec::new()
        ),
        Loss::Part(0.0)
    );
    let Loss::Part(total) = first_loss(
        from,
        to,
        Vec::new,
        1e9,
        (|_| 1e-3, |_| 1e-3),
        &mut Vec::new(),
    ) else {
        panic!("a radius too large is never lost");
    };
    assert!((total / 1e4 - 1.0).abs() < 1e-9, "{total} m");
    assert_eq!(
        first_loss(from, to, Vec::new, 0.0, (|_| 0.0, |_| 0.0), &mut Vec::new()),
        Loss::Gone(from)
    );
}

/// A 10⁻⁶ M⊕ rock at 0.05 au about the Sun, at a substellar 1,600–1,700 K on the main sequence,
/// is `Destroyed { Evaporated }` on it, within 1 Gyr of its formation; a body from 10⁻³ M⊕ is
/// kept, and so is one built by hand.
#[test]
fn a_small_rock_close_to_a_sun_like_star_evaporates_on_its_main_sequence() {
    let star = sun(4.6e9);
    let host = FateHost::star(&star);
    let close = rock(0.05, 1e-6);
    let fate = BodyFate::resolve(&close, &host);
    let Some(BodyState::Destroyed { cause, at }) = fate.ending() else {
        panic!("a hot small rock evaporates: {:?}", fate.ending());
    };
    assert_eq!(cause, DestructionCause::Evaporated);
    let formed = fate.formed_at().expect("formed");
    let myr = at.checked_since(formed).expect("a span").as_seconds_f64() / 3.155_76e13;
    assert!(
        (0.0..1_000.0).contains(&myr),
        "{myr} Myr after its formation"
    );
    let phase = star.state_at(at).expect("formed").phase();
    assert_eq!(phase, crate::stellar::Phase::MainSequence);
    assert_eq!(
        fate.at(UniverseTime::EPOCH).state(),
        BodyState::Destroyed { cause, at }
    );

    let heavier = rock(0.05, 2e-3);
    let heavy = BodyFate::resolve(&heavier, &host);
    assert_eq!(heavy.at(UniverseTime::EPOCH).state(), BodyState::Present);
    let by_hand =
        FateBody::new(*close.formation(), *close.orbit(), close.mass(), ROCK).expect("valid");
    assert_eq!(by_hand.evaporation(), Evaporation::Ignored);
    let unfollowed = BodyFate::resolve(&by_hand, &host);
    assert_eq!(
        unfollowed.at(UniverseTime::EPOCH).state(),
        BodyState::Present
    );
}

/// Near the end of its life a rock's mass is what its radius has left, R(t) = R₀ − ∫ rate dt
/// (PBC13's eq. 28 at a radius that falls): at 90% of the way from its formation to its
/// evaporation, the 10⁻⁶ M⊕ rock at 0.05 au has the mass (1 − ∫ rate dt ÷ R₀)³ of its own, the
/// integral taken here by a trapezoid of 200,000 steps over its orbit and its star's light, to
/// 10⁻³.
#[test]
fn a_rock_near_the_end_of_its_life_has_the_mass_its_radius_has_left() {
    let star = sun(4.6e9);
    let host = FateHost::star(&star);
    let close = rock(0.05, 1e-6);
    let fate = BodyFate::resolve(&close, &host);
    let formed = fate.formed_at().expect("formed");
    let Some(BodyState::Destroyed { at: gone, .. }) = fate.ending() else {
        panic!("the rock evaporates");
    };
    let life = gone.checked_since(formed).expect("a span").as_seconds_f64();
    let at = |share: f64| {
        formed
            .checked_add(Span::from_seconds_f64(life * share).expect("a span"))
            .expect("on the clock")
    };
    let t = at(0.9);
    let rate = |when: UniverseTime| {
        let orbit = fate.orbit_at(when).expect("present");
        let light = star.state_at(when).expect("formed").luminosity();
        radius_rate(
            substellar_temperature(light, orbit.semi_major_axis(), orbit.eccentricity().value()),
            ROCK,
        )
    };
    let steps = 200_000;
    let step = 0.9 * life / f64::from(steps);
    let lost: f64 = (0..steps)
        .map(|k| {
            let (a, b) = (
                at(0.9 * f64::from(k) / f64::from(steps)),
                at(0.9 * f64::from(k + 1) / f64::from(steps)),
            );
            f64::midpoint(rate(a), rate(b)) * step
        })
        .sum();
    let left = 1.0 - lost / radius_of(close.mass(), ROCK);
    let expected = 1e-6 * left * left * left;
    let mass = fate.at(t).mass().value();
    assert!(
        (mass / expected - 1.0).abs() < 1e-3,
        "{mass} M⊕ against {expected}"
    );
    assert!(
        (1e-10..1e-8).contains(&mass),
        "{mass} M⊕ at 90% of its life"
    );
    let later = fate.at(at(0.99)).mass().value();
    assert!(later < mass, "{later} M⊕ after {mass}");
}

/// A 10⁻⁶ M⊕ rock at 1 au about the Sun of 13.5 Gyr, at a substellar 400–460 K on the main
/// sequence, evaporates once its host has left it, as the red-giant branch brightens it past
/// 2,000 K, before the white dwarf's birth: never at its system's age. One at 2 au, whose
/// widening orbit the giant phases heat to some 1,700 K for a few million years alone, is kept.
#[test]
fn a_rock_evaporates_only_once_its_host_leaves_its_main_sequence() {
    let star = sun(13.5e9);
    let host = FateHost::star(&star);
    let near = rock(1.0, 1e-6);
    let fate = BodyFate::resolve(&near, &host);
    let Some(BodyState::Destroyed { cause, at }) = fate.ending() else {
        panic!(
            "a small rock at 1 au evaporates on the giant branches: {:?}",
            fate.ending()
        );
    };
    assert_eq!(cause, DestructionCause::Evaporated);
    let age = age_gyr(&star, at);
    let left = main_sequence_end_gyr(&star);
    assert!(
        age > left,
        "evaporated at {age} Gyr, its host's main sequence ending at {left}"
    );
    assert!(
        age < 12.6,
        "evaporated at {age} Gyr, after the white dwarf's birth"
    );
    let far = rock(2.0, 1e-6);
    let cooler = BodyFate::resolve(&far, &host);
    assert_eq!(cooler.at(UniverseTime::EPOCH).state(), BodyState::Present);
}

/// A moon has its own ending along its planet's history (`BodyFate::moon_evaporation`): a
/// 10⁻⁶ M⊕ moon of an Earth at 1 au about the Sun of 13.5 Gyr evaporates once its host leaves its
/// main sequence, at the time a member of its mass on its planet's orbit would, to the second's
/// bisection; a moon from 10⁻³ M⊕ does not.
#[test]
fn a_moon_evaporates_along_its_planet_s_history() {
    let star = sun(13.5e9);
    let host = FateHost::star(&star);
    let planet = rock(1.0, 1.0);
    let fate = BodyFate::resolve(&planet, &host);
    assert_eq!(fate.at(UniverseTime::EPOCH).state(), BodyState::Present);
    let at = fate
        .moon_evaporation(EarthMasses::new(1e-6))
        .expect("a small moon evaporates on the giant branches");
    assert!(age_gyr(&star, at) > main_sequence_end_gyr(&star));
    // A rock of the moon's mass and density on the planet's orbit.
    let twin = FateBody::new(
        *planet.formation(),
        *planet.orbit(),
        EarthMasses::new(1e-6),
        MOON_EVAPORATING_ROCK_DENSITY,
    )
    .expect("valid")
    .evaporating();
    let alone = BodyFate::resolve(&twin, &host);
    assert_eq!(
        alone.ending(),
        Some(BodyState::Destroyed {
            cause: DestructionCause::Evaporated,
            at
        })
    );
    // Near its end, its mass is the twin's, its radius's share left.
    let near = at
        .checked_sub(Span::from_seconds(31_557_600_000))
        .expect("on the clock");
    let left = fate.moon_mass_at(EarthMasses::new(1e-6), near);
    assert_eq!(left, alone.at(near).mass());
    assert!(
        left.value() <= 1e-6,
        "{} M⊕ a millennium before its end",
        left.value()
    );
    assert_eq!(fate.moon_evaporation(EarthMasses::new(1e-3)), None);
}
