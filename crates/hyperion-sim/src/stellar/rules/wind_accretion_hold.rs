//! The tests of P11.T12's fix: a pair the pre-test's lobe and decay tests pass over takes its
//! winds, read at one choke point, `wind_accretion::onset`. Its hold is deleted (the 27 fold);
//! these are its tests of the new arm.
//!
//! - The brief's halo pair: 0.85 + 0.80 M☉ at Z = 0.0003 on a 3,000-day orbit. Its 0.80 M☉ star has
//!   gained mass from its partner's wind by 12 Gyr, past its own model's, and the pair can
//!   interact by then though not by 5 Gyr.
//! - Wind Roche-lobe overflow: the same pair's 0.80 M☉ star takes at least three times what it
//!   takes of the Bondi–Hoyle share alone (the test-only arm, as first built).
//! - The pair's past does not depend on the age it is run to, and its masses are continuous where
//!   its segments meet.
//! - The cheap path: a pair of main-sequence stars, and one too wide to take 10⁻⁴ M☉, are one
//!   detached segment.
//!
//! The closed form's numerics are tested in `stellar/binary/wind_accretion/tests.rs`, and its past
//! at any age in `stellar/binary/tests.rs`
//! (`a_timeline_is_the_same_whatever_age_it_is_run_to_under_the_wind_accretion_arm`).

use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::stellar::Composition;
use crate::stellar::binary::wind_accretion::{WIND_ROCHE_LOBE_MAX_SHARE, WindAccretionRules};
use crate::stellar::binary::{
    BinaryInput, BinaryTimeline, Segment, SegmentKind, can_interact, evolve,
};
use crate::stellar::draws::StarDraws;
use crate::stellar::sse::Track;
use crate::units::{
    Dex, GravitationalParameter, HeliumExcess, Radians, Seconds, SolarMasses, Years,
};

/// The age the halo pair is run to, years: 12 Gyr.
const UNTIL: f64 = 1.2e10;

/// The halo pair's [Fe/H]: log₁₀ 0.015, Z = 0.0003.
fn halo_fe_h() -> f64 {
    crate::math::log10(0.015)
}

/// A pair of `m1` and `m2` M☉ at [Fe/H] `fe_h` on a circular orbit of `period_days`, under the
/// version's rules.
fn pair(m1: f64, m2: f64, period_days: f64, fe_h: f64) -> BinaryInput {
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
}

/// The brief's halo pair: 0.85 + 0.80 M☉ at Z = 0.0003 on a 3,000-day orbit.
fn halo_pair() -> BinaryInput {
    pair(0.85, 0.80, 3_000.0, halo_fe_h())
}

/// The halo pair's star `i` on its own track, as the pre-test passes a pair over.
fn own_track(i: usize) -> Track {
    let input = halo_pair();
    Track::full(input.masses()[i], input.composition(), &input.draws()[i])
}

/// The secondary's mass at `age` years, M☉.
fn secondary_mass(timeline: &BinaryTimeline, age: f64) -> f64 {
    timeline.state_at(Years::new(age)).stars()[1].mass().value()
}

/// The brief's halo pair's 0.80 M☉ star has gained mass from its partner's wind by 12 Gyr, past
/// its own model's, and the pair can interact by then though not by 5 Gyr (under the held arm it
/// was one detached segment of its own models, which the pre-test passes over).
#[test]
fn a_wide_halo_pair_s_companion_takes_its_partner_s_wind() {
    let until = Years::new(UNTIL);
    let fed = evolve(&halo_pair(), until);
    let kinds: Vec<SegmentKind> = fed.segments().iter().map(Segment::kind).collect();
    let (own, after) = (
        own_track(1).state_at(until).mass().value(),
        secondary_mass(&fed, UNTIL),
    );
    let now = fed.state_at(until);
    println!(
        "at 12 Gyr: the 0.80 M_sun star {own:.5} M_sun on its own track, {after:.5} fed ({:?}, \
         its partner a {:?} of {:.3} M_sun; class {:?}); segments {:?} from {:.4} Gyr",
        now.stars()[1].phase(),
        now.stars()[0].phase(),
        now.stars()[0].mass().value(),
        fed.class_at(until),
        kinds,
        fed.segments()[1].start().value() / 1e9,
    );
    assert!(can_interact(&halo_pair(), until));
    assert!(!can_interact(&halo_pair(), Years::new(5.0e9)));
    assert!(
        kinds.iter().all(|&k| k == SegmentKind::Detached),
        "{kinds:?}"
    );
    assert!(fed.segments().len() >= 2, "{kinds:?}");
    assert!(after > own + 5e-3, "{own} on its own track, {after} fed");
}

/// The halo pair's past does not depend on the age it is run to: run to 12 Gyr, to the middle of
/// its accretion, and to 13.5 Gyr, its states agree bit for bit wherever both hold them. Its masses
/// are continuous where its segments meet, to 10⁻⁶ M☉ at the onset and with no jump at the
/// accretor's main-sequence end.
#[test]
fn the_wind_fed_pair_s_past_does_not_depend_on_its_age() {
    let input = halo_pair();
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

/// The cheap path: a pair of main-sequence stars, and a pair whose primary dies but too wide for
/// its companion to take 10⁻⁴ M☉, are one detached segment, which the pre-test passes over.
#[test]
fn pairs_that_take_too_little_wind_stay_their_own_models() {
    let until = Years::new(UNTIL);
    for (m1, m2, days, fe_h) in [(0.6, 0.3, 1.0e3, 0.0), (0.85, 0.80, 1.0e8, -1.82)] {
        let input = pair(m1, m2, days, fe_h);
        assert!(
            !can_interact(&input, until),
            "{m1} + {m2} M_sun at {days} d"
        );
        let timeline = evolve(&input, until);
        assert_eq!(
            timeline.segments().len(),
            1,
            "{m1} + {m2} M_sun at {days} d"
        );
    }
}

/// Wind Roche-lobe overflow (research-leans-30 §2): beside its partner's asymptotic giant branch
/// the halo pair's 0.80 M☉ star takes at least three times what it takes of the Bondi–Hoyle share
/// alone (the test-only arm), 2 Myr after its partner's death, each past its own track's mass, and
/// no more than half the wind its partner loses on that branch beyond its Bondi–Hoyle share before
/// it. Before the branch the two arms agree, state for state.
#[test]
fn an_agb_partner_s_wind_overflows_to_its_companion() {
    let until = Years::new(UNTIL);
    let [held, overflow] = [
        WindAccretionRules::BondiHoyle,
        WindAccretionRules::WindRocheLobe,
    ]
    .map(|arm| evolve(&halo_pair().with_wind(arm), until));
    let (primary, secondary) = (own_track(0), own_track(1));
    let death = (1..=12_000)
        .map(|k| 1.0e6 * f64::from(k))
        .find(|&age| !primary.state_at(Years::new(age)).phase().is_living())
        .expect("the primary dies by 12 Gyr");
    let agb = (1..=12_000)
        .map(|k| 1.0e6 * f64::from(k))
        .find(|&age| {
            matches!(
                primary.state_at(Years::new(age)).phase(),
                crate::stellar::Phase::EarlyAgb | crate::stellar::Phase::ThermallyPulsingAgb
            )
        })
        .expect("the primary reaches its asymptotic giant branch");
    let after = death + 2.0e6;
    let own = secondary.state_at(Years::new(after)).mass().value();
    let gained = |timeline: &BinaryTimeline| secondary_mass(timeline, after) - own;
    let (by_bondi_hoyle, by_overflow) = (gained(&held), gained(&overflow));
    let lost_on_the_agb = primary.state_at(Years::new(agb - 1.0e6)).mass().value()
        - primary.state_at(until).mass().value();
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
