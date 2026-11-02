//! The tests of P11.T12's fix, [`WIND_ACCRETION_VERSION`]: a pair the pre-test's lobe and decay
//! tests pass over takes its winds only under the rules that turn it on, read at one choke point,
//! `wind_accretion::onset`, through the pair's input's rules.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - Failing first, the brief's halo pair: 0.85 + 0.80 M☉ at Z = 0.0003 on a 3,000-day orbit.
//!   Under the arm the 0.80 M☉ star has gained mass by 12 Gyr and the pair can interact; under the
//!   held arm it has not, and the pair is one detached segment of its own models.
//! - Under the arm the pair's past does not depend on the age it is run to, and its masses are
//!   continuous where its segments meet.
//! - The cheap path: a pair of main-sequence stars, and one too wide to take 10⁻⁴ M☉, are their
//!   own models under the arm, segment for segment.
//!
//! [`WIND_ACCRETION_VERSION`]: crate::stellar::binary::wind_accretion::WIND_ACCRETION_VERSION

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::rules::GalaxyRules;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::stellar::Composition;
use crate::stellar::binary::{
    BinaryInput, BinaryTimeline, Segment, SegmentKind, can_interact, evolve,
};
use crate::stellar::draws::StarDraws;
use crate::units::{
    Dex, GravitationalParameter, HeliumExcess, Radians, Seconds, SolarMasses, Years,
};

/// The age the halo pair is run to, years: 12 Gyr.
const UNTIL: f64 = 1.2e10;

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The version's rules with this hold's arm on alone.
fn bondi_hoyle() -> StellarRules {
    StellarRules {
        wind_accretion: WindAccretionRules::BondiHoyle,
        ..version()
    }
}

/// A pair of `m1` and `m2` M☉ at [Fe/H] `fe_h` on a circular orbit of `period_days`, under
/// `rules`.
fn pair(m1: f64, m2: f64, period_days: f64, fe_h: f64, rules: StellarRules) -> BinaryInput {
    let orbit = KeplerElements::from_period(
        Seconds::new(period_days * 86_400.0),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::new(0.3), Radians::new(0.0), Radians::new(0.0))
            .expect("an orientation"),
        Radians::new(0.0),
    )
    .expect("an orbit");
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO),
        orbit,
        [StarDraws::median(), StarDraws::median()],
        Years::new(UNTIL),
    )
    .expect("a pair")
    .with_rules(rules)
}

/// The brief's halo pair: 0.85 + 0.80 M☉ at Z = 0.0003 ([Fe/H] = log₁₀ 0.015) on a 3,000-day
/// orbit.
fn halo_pair(rules: StellarRules) -> BinaryInput {
    pair(0.85, 0.80, 3_000.0, crate::math::log10(0.015), rules)
}

/// The secondary's mass at `age` years, M☉.
fn secondary_mass(timeline: &BinaryTimeline, age: f64) -> f64 {
    timeline.state_at(Years::new(age)).stars()[1].mass().value()
}

/// The version's rules keep the held arm, every version below 27 does and 27 turns the wind
/// accretion on. The tripwire: the bump to 27 deletes this hold.
#[test]
fn the_wind_accretion_is_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < WIND_ACCRETION_VERSION,
        "the bump to 27 deletes P11.T12's wind-accretion hold: `WIND_ACCRETION_VERSION`, \
         `WindAccretionRules` with its `OwnTracks` arm, `StellarRules::wind_accretion`, its read \
         in `wind_accretion::onset` and the brief's gate, this module and its line in \
         `stellar/rules.rs`; and it refits the pair-light and reach tables and gives \
         `pair_light_bound`'s `Detached` the wind's reach"
    );
    assert_eq!(version().wind_accretion, WindAccretionRules::OwnTracks);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).wind_accretion;
    for v in [GENERATOR_VERSION.get(), 25, 26] {
        assert_eq!(at(v), WindAccretionRules::OwnTracks, "version {v}");
    }
    assert_eq!(at(27), WindAccretionRules::BondiHoyle);
    assert_eq!(
        StellarRules::every_26_hold_with_its_retargets().wind_accretion,
        WindAccretionRules::OwnTracks
    );
    assert_eq!(
        StellarRules::every_27_hold().wind_accretion,
        WindAccretionRules::BondiHoyle
    );
    assert_eq!(
        GalaxyRules::every_27_hold().stellar.wind_accretion,
        WindAccretionRules::BondiHoyle
    );
    assert!(version().wind_accretion.fitted_tables_hold());
    assert!(!bondi_hoyle().wind_accretion.fitted_tables_hold());
}

/// Failing first: under the arm the brief's halo pair's 0.80 M☉ star has gained mass from its
/// partner's wind by 12 Gyr, and the pair can interact by then though not by 5 Gyr; under the
/// held arm it has not, and the pair is one detached segment of its own models, which the
/// pre-test passes over.
#[test]
fn a_wide_halo_pair_s_companion_takes_its_partner_s_wind_under_the_arm() {
    let until = Years::new(UNTIL);
    let held = evolve(&halo_pair(version()), until);
    let arm = evolve(&halo_pair(bondi_hoyle()), until);
    let kinds: Vec<SegmentKind> = arm.segments().iter().map(Segment::kind).collect();
    let (before, after) = (secondary_mass(&held, UNTIL), secondary_mass(&arm, UNTIL));
    let now = arm.state_at(until);
    println!(
        "at 12 Gyr: the 0.80 M_sun star {before:.5} M_sun held, {after:.5} under the arm ({:?}, \
         its partner a {:?} of {:.3} M_sun; class {:?} held, {:?} under the arm); segments {:?} \
         from {:.4} Gyr",
        now.stars()[1].phase(),
        now.stars()[0].phase(),
        now.stars()[0].mass().value(),
        held.class_at(until),
        arm.class_at(until),
        kinds,
        arm.segments()[1].start().value() / 1e9,
    );
    assert_eq!(held.segments().len(), 1);
    assert!(!can_interact(&halo_pair(version()), until));
    assert!(can_interact(&halo_pair(bondi_hoyle()), until));
    assert!(!can_interact(&halo_pair(bondi_hoyle()), Years::new(5.0e9)));
    assert!(
        kinds.iter().all(|&k| k == SegmentKind::Detached),
        "{kinds:?}"
    );
    assert!(arm.segments().len() >= 2, "{kinds:?}");
    assert!(
        after > before + 5e-3,
        "{before} held, {after} under the arm"
    );
    // Before its onset the pair is its own models under the arm too.
    let early = Years::new(5.0e9);
    assert_eq!(held.state_at(early), arm.state_at(early));
}

/// Under the arm the halo pair's past does not depend on the age it is run to: run to 12 Gyr, to
/// the middle of its accretion, and to 13.5 Gyr, its states agree bit for bit wherever both
/// hold them. Its masses are continuous where its segments meet, to 10⁻⁶ M☉ at the onset and
/// with no jump at the accretor's main-sequence end.
#[test]
fn the_wind_fed_pair_s_past_does_not_depend_on_its_age() {
    let input = halo_pair(bondi_hoyle());
    let full = evolve(&input, Years::new(UNTIL));
    let onset = full.segments()[1].start().value();
    let fed = full.segments()[1].end().value().min(UNTIL);
    let ages = [f64::midpoint(onset, fed), UNTIL, 1.35e10];
    let timelines: Vec<BinaryTimeline> = ages
        .iter()
        .map(|&a| evolve(&input, Years::new(a)))
        .collect();
    for (k, timeline) in timelines.iter().enumerate() {
        for other in &timelines[k..] {
            for j in 0..=200 {
                let age = ages[k] * f64::from(j) / 200.0;
                assert_eq!(
                    timeline.state_at(Years::new(age)),
                    other.state_at(Years::new(age)),
                    "at {age} yr, run to {} and {} yr",
                    ages[k],
                    other.until().value()
                );
            }
        }
    }
    let longest = &timelines[2];
    for pair in longest.segments().windows(2) {
        let at = pair[1].start().value();
        let jump =
            (secondary_mass(longest, at) - secondary_mass(longest, at * (1.0 - 1e-12))).abs();
        println!(
            "at {:.5} Gyr: {:?} → {:?}, a jump of {jump:.3e} M_sun",
            at / 1e9,
            pair[0].kind(),
            pair[1].kind()
        );
        assert!(jump < 1e-6, "{jump} M_sun at {at} yr");
    }
}

/// The cheap path: under the arm a pair of main-sequence stars, and a pair whose primary dies but
/// too wide for its companion to take 10⁻⁴ M☉, are one detached segment of their own models, state
/// for state, as under the held arm.
#[test]
fn pairs_that_take_too_little_wind_stay_their_own_models() {
    let until = Years::new(UNTIL);
    for (m1, m2, days, fe_h) in [(0.6, 0.3, 1.0e3, 0.0), (0.85, 0.80, 1.0e8, -1.82)] {
        let [held, arm] = [version(), bondi_hoyle()].map(|rules| pair(m1, m2, days, fe_h, rules));
        assert!(!can_interact(&arm, until), "{m1} + {m2} M_sun at {days} d");
        let [held, arm] = [held, arm].map(|input| evolve(&input, until));
        assert_eq!(arm.segments().len(), 1, "{m1} + {m2} M_sun at {days} d");
        for j in 0..=100 {
            let age = Years::new(UNTIL * f64::from(j) / 100.0);
            assert_eq!(
                held.state_at(age),
                arm.state_at(age),
                "{m1} + {m2} M_sun at {days} d"
            );
        }
    }
}
