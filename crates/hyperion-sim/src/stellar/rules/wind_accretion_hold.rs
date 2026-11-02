//! The tests of P11.T12's fix, [`WIND_ACCRETION_VERSION`]: a pair the pre-test's lobe and decay
//! tests pass over takes its winds only under the rules that turn it on, read at one choke point,
//! `wind_accretion::onset`, through the pair's input's rules.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - Failing first, the brief's halo pair: 0.85 + 0.80 M☉ at Z = 0.0003 on a 3,000-day orbit.
//!   Under the arm the 0.80 M☉ star has gained mass by 12 Gyr and the pair can interact; under the
//!   held arm it has not, and the pair is one detached segment of its own models.
//! - Failing first, wind Roche-lobe overflow: the same pair's 0.80 M☉ star takes at least three
//!   times under 27's arm what it takes of the Bondi–Hoyle share alone.
//! - Under the arm the pair's past does not depend on the age it is run to, and its masses are
//!   continuous where its segments meet.
//! - The cheap path: a pair of main-sequence stars, and one too wide to take 10⁻⁴ M☉, are their
//!   own models under the arm, segment for segment.
//!
//! The closed form's numerics are tested in `stellar/binary/wind_accretion/tests.rs`, and its past
//! at any age under the arm in `stellar/binary/tests.rs`
//! (`a_timeline_is_the_same_whatever_age_it_is_run_to_under_the_wind_accretion_arm`).
//!
//! The movers under the arm, found by turning it on at the version over the workspace's fast
//! tests (2026-11-02), are listed in plan 11's records.
//!
//! [`WIND_ACCRETION_VERSION`]: crate::stellar::binary::wind_accretion::WIND_ACCRETION_VERSION

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::rules::GalaxyRules;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::stellar::Composition;
use crate::stellar::binary::wind_accretion::WIND_ROCHE_LOBE_MAX_SHARE;
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

/// The version's rules with this hold's arm, 27's, on alone.
fn arm() -> StellarRules {
    StellarRules {
        wind_accretion: WindAccretionRules::WindRocheLobe,
        ..version()
    }
}

/// The version's rules with the test-only Bondi–Hoyle arm on alone: the arm as first built, with
/// no wind Roche-lobe overflow.
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
    assert_eq!(at(27), WindAccretionRules::WindRocheLobe);
    assert_eq!(
        StellarRules::every_26_hold_with_its_retargets().wind_accretion,
        WindAccretionRules::OwnTracks
    );
    assert_eq!(
        StellarRules::every_27_hold().wind_accretion,
        WindAccretionRules::WindRocheLobe
    );
    assert_eq!(
        GalaxyRules::every_27_hold().stellar.wind_accretion,
        WindAccretionRules::WindRocheLobe
    );
    assert!(version().wind_accretion.fitted_tables_hold());
    assert!(!arm().wind_accretion.fitted_tables_hold());
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
    let fed = evolve(&halo_pair(arm()), until);
    let kinds: Vec<SegmentKind> = fed.segments().iter().map(Segment::kind).collect();
    let (before, after) = (secondary_mass(&held, UNTIL), secondary_mass(&fed, UNTIL));
    let now = fed.state_at(until);
    println!(
        "at 12 Gyr: the 0.80 M_sun star {before:.5} M_sun held, {after:.5} under the arm ({:?}, \
         its partner a {:?} of {:.3} M_sun; class {:?} held, {:?} under the arm); segments {:?} \
         from {:.4} Gyr",
        now.stars()[1].phase(),
        now.stars()[0].phase(),
        now.stars()[0].mass().value(),
        held.class_at(until),
        fed.class_at(until),
        kinds,
        fed.segments()[1].start().value() / 1e9,
    );
    assert_eq!(held.segments().len(), 1);
    assert!(!can_interact(&halo_pair(version()), until));
    assert!(can_interact(&halo_pair(arm()), until));
    assert!(!can_interact(&halo_pair(arm()), Years::new(5.0e9)));
    assert!(
        kinds.iter().all(|&k| k == SegmentKind::Detached),
        "{kinds:?}"
    );
    assert!(fed.segments().len() >= 2, "{kinds:?}");
    assert!(
        after > before + 5e-3,
        "{before} held, {after} under the arm"
    );
    // Before its onset the pair is its own models under the arm too.
    let early = Years::new(5.0e9);
    assert_eq!(held.state_at(early), fed.state_at(early));
}

/// Under the arm the halo pair's past does not depend on the age it is run to: run to 12 Gyr, to
/// the middle of its accretion, and to 13.5 Gyr, its states agree bit for bit wherever both
/// hold them. Its masses are continuous where its segments meet, to 10⁻⁶ M☉ at the onset and
/// with no jump at the accretor's main-sequence end.
#[test]
fn the_wind_fed_pair_s_past_does_not_depend_on_its_age() {
    let input = halo_pair(arm());
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
        let [held, arm] = [version(), arm()].map(|rules| pair(m1, m2, days, fe_h, rules));
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

/// Failing first, wind Roche-lobe overflow (research-leans-30 §2): beside its partner's
/// asymptotic giant branch the halo pair's 0.80 M☉ star takes at least three times under 27's arm
/// what it takes of the Bondi–Hoyle share alone, 2 Myr after its partner's death, and no more than
/// half the wind its partner loses on that branch beyond its Bondi–Hoyle share before it. Before
/// the branch the two arms agree, state for state.
#[test]
fn an_agb_partner_s_wind_overflows_to_its_companion_under_the_arm() {
    let until = Years::new(UNTIL);
    let [own, held, overflow] =
        [version(), bondi_hoyle(), arm()].map(|rules| evolve(&halo_pair(rules), until));
    let primary = own.state_at(until);
    let death = (1..=12_000)
        .map(|k| 1.0e6 * f64::from(k))
        .find(|&age| !own.state_at(Years::new(age)).stars()[0].phase().is_living())
        .expect("the primary dies by 12 Gyr");
    let agb = (1..=12_000)
        .map(|k| 1.0e6 * f64::from(k))
        .find(|&age| {
            matches!(
                own.state_at(Years::new(age)).stars()[0].phase(),
                crate::stellar::Phase::EarlyAgb | crate::stellar::Phase::ThermallyPulsingAgb
            )
        })
        .expect("the primary reaches its asymptotic giant branch");
    let after = death + 2.0e6;
    let gained =
        |timeline: &BinaryTimeline| secondary_mass(timeline, after) - secondary_mass(&own, after);
    let (by_bondi_hoyle, by_overflow) = (gained(&held), gained(&overflow));
    let lost_on_the_agb = own.state_at(Years::new(agb - 1.0e6)).stars()[0]
        .mass()
        .value()
        - primary.stars()[0].mass().value();
    println!(
        "by {:.4} Gyr: {by_bondi_hoyle:.5} M_sun of the Bondi-Hoyle share, {by_overflow:.5} with \
         the overflow ({:.2} times); the primary lost {lost_on_the_agb:.4} M_sun from its AGB at \
         {:.4} Gyr; at 12 Gyr the 0.80 M_sun star is a {:?} of {:.5} M_sun",
        after / 1e9,
        by_overflow / by_bondi_hoyle,
        agb / 1e9,
        overflow.state_at(until).stars()[1].phase(),
        secondary_mass(&overflow, UNTIL),
    );
    assert!(by_bondi_hoyle > 5e-3, "{by_bondi_hoyle}");
    assert!(
        by_overflow >= 3.0 * by_bondi_hoyle,
        "{by_overflow} with the overflow against {by_bondi_hoyle}"
    );
    assert!(
        by_overflow <= by_bondi_hoyle + WIND_ROCHE_LOBE_MAX_SHARE * lost_on_the_agb,
        "{by_overflow} against {lost_on_the_agb} lost on the AGB"
    );
    for j in 0..=100 {
        let age = Years::new((agb - 1.0e6) * f64::from(j) / 100.0);
        assert_eq!(held.state_at(age), overflow.state_at(age), "at {age:?}");
    }
}
