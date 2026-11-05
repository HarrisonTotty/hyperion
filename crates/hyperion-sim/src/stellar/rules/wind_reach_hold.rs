//! The tests of the wind's reach hold (P11.T12's fix, held at 29): from 29 the census's pair
//! bound calls a pair `Detached` only where neither star can take 10⁻⁴ M☉ of its partner's wind
//! by the window's end, read at one choke point, `light::detached`.
//!
//! - The tripwire: the hold is in force until the 28 → 29 bump.
//! - A wide pair whose secondary takes its primary's AGB wind: `Detached` under 28's rules, read
//!   from the tables under 29's, and refused by the brief's gate under 29's. The census's bound on
//!   such grown stars is checked by `sky::census::cell`'s `the_star_bound_holds_for_*` tests.
//! - The cheap path: two red dwarfs, whose winds are nil, and a pair too wide to feed, stay
//!   `Detached` under 29's rules.
//!
//! The screen against the generator's wind-fed pairs is tested in
//! `stellar/binary/wind_reach/tests.rs`, and over the generated pairs' tallies in
//! `stellar/binary/light.rs` (`detached_generated_pairs_are_passed_over`).

use crate::galaxy::rules::GalaxyRules;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::stellar::Composition;
use crate::stellar::binary::wind_reach::{WIND_REACH_VERSION, WindReachRules};
use crate::stellar::binary::{
    BinaryInput, PairLight, can_interact, pair_keeps_own_models, pair_light_bound,
    pair_light_bound_under,
};
use crate::stellar::draws::StarDraws;
use crate::stellar::rules::StellarRules;
use crate::units::consts::{METRES_PER_AU, SOLAR_RADIUS_M};
use crate::units::{GravitationalParameter, Metres, Radians, SolarMasses, Years};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The wind's reach is held until the 28 → 29 bump: the rules in force are version 28's and
/// before, and 29's turn it on.
#[test]
fn the_wind_reach_is_held_only_until_the_29_bump() {
    assert!(
        GENERATOR_VERSION < WIND_REACH_VERSION,
        "the 28 → 29 bump puts the wind's reach on the record (p11-t12-wind-reach): delete \
         `WIND_REACH_VERSION`, `WindReachRules` with its `LobeAndDecay` arm, \
         `StellarRules::wind_reach`, the `match` in `light::detached`, the held column of \
         `light::tests::Tally` and this test, and re-pin `detached_generated_pairs_are_passed_over`'s \
         figures at 29"
    );
    assert_eq!(WIND_REACH_VERSION, GeneratorVersion::new(29));
    for v in [23, 27, 28] {
        assert_eq!(
            StellarRules::of_version(GeneratorVersion::new(v)).wind_reach,
            WindReachRules::LobeAndDecay,
            "{v}"
        );
    }
    assert_eq!(
        StellarRules::of_version(GENERATOR_VERSION).wind_reach,
        WindReachRules::LobeAndDecay
    );
    assert_eq!(
        StellarRules::of_version(WIND_REACH_VERSION).wind_reach,
        WindReachRules::Wind
    );
    assert_eq!(
        GalaxyRules::every_29_hold().stellar.wind_reach,
        WindReachRules::Wind
    );
}

/// A pair of `m1` and `m2` M☉ at solar metallicity on an orbit of periastron `periastron_au` and
/// eccentricity `e`, under the median draws.
fn pair(m1: f64, m2: f64, periastron_au: f64, e: f64) -> BinaryInput {
    let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2));
    let a = Metres::new(periastron_au * METRES_PER_AU / (1.0 - e));
    let orbit = KeplerElements::from_semi_major_axis(
        a,
        mu,
        Eccentricity::new(e).expect("an ellipse"),
        Orientation::new(Radians::new(0.0), Radians::new(0.0), Radians::new(0.0))
            .expect("an orientation"),
        Radians::new(0.0),
    )
    .expect("an orbit");
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        Composition::SOLAR,
        orbit,
        [StarDraws::median(), StarDraws::median()],
        Years::new(5.0e9),
    )
    .expect("a pair of 0.08-150 M_sun")
}

/// The bound on `input` at `age` years under `rules`.
fn bound(input: &BinaryInput, age: f64, rules: StellarRules) -> Option<PairLight> {
    let [m1, m2] = input.masses();
    let at = Years::new(age);
    pair_light_bound_under(
        m1,
        m2,
        input.orbit().periapsis(),
        input.composition(),
        at..=at,
        rules,
    )
}

/// A pair of 1.4 and 1.0 M☉ 58 au apart at periastron: the lobe and decay tests pass it over, but
/// its secondary takes its primary's AGB wind, 10⁻⁴ M☉ by 5 Gyr. Under 28's rules the bound calls
/// it `Detached`, a broken contract, and the brief's gate certifies it; under 29's it reads the
/// tables and the gate refuses it, while the pair 10⁴ au out at periastron, which takes too
/// little, stays `Detached`. Before the primary's giant branch both stars are their own models
/// under either.
#[test]
fn a_wind_fed_pair_is_detached_under_28_and_read_from_the_tables_under_29() {
    let rules_29 = GalaxyRules::every_29_hold().stellar;
    let rules_28 = StellarRules::of_version(GeneratorVersion::new(28));
    let fed = pair(1.4, 1.0, 58.0, 0.5);
    let age = 5.0e9;
    assert!(can_interact(&fed, Years::new(age)), "the pair is wind-fed");
    assert_eq!(bound(&fed, age, rules_28), Some(PairLight::Detached));
    let [m1, m2] = fed.masses();
    let at = Years::new(age);
    assert_eq!(
        bound(&fed, age, rules_28),
        pair_light_bound(
            m1,
            m2,
            fed.orbit().periapsis(),
            &Composition::SOLAR,
            at..=at
        )
    );
    let verdict = bound(&fed, age, rules_29);
    assert_ne!(verdict, Some(PairLight::Detached), "{verdict:?}");
    assert!(
        matches!(verdict, Some(PairLight::Bright(_)) | None),
        "{verdict:?}"
    );
    // The brief's gate reads the same `Detached`.
    let gate = |rules| {
        pair_keeps_own_models(
            m1,
            m2,
            fed.orbit().periapsis(),
            &Composition::SOLAR,
            at..=at,
            rules,
        )
    };
    assert!(gate(rules_28));
    assert!(!gate(rules_29));
    // Young, its primary has lost no wind.
    assert!(!can_interact(&fed, Years::new(1.0e9)));
    assert_eq!(bound(&fed, 1.0e9, rules_29), Some(PairLight::Detached));
    // 10⁴ au out at periastron the secondary takes too little.
    let wide = pair(1.4, 1.0, 1.0e4, 0.5);
    assert!(!can_interact(&wide, Years::new(age)));
    assert_eq!(bound(&wide, age, rules_29), Some(PairLight::Detached));
}

/// Two red dwarfs lose no wind by 13 Gyr, so the wind's reach leaves even a close pair of them,
/// which the lobe and decay tests pass over, `Detached`.
#[test]
fn two_red_dwarfs_stay_detached_under_29() {
    let rules_29 = GalaxyRules::every_29_hold().stellar;
    let dwarfs = pair(0.5, 0.3, 0.2, 0.1);
    let age = 1.3e10;
    assert!(!can_interact(&dwarfs, Years::new(age)));
    assert_eq!(
        bound(&dwarfs, age, rules_29),
        bound(&dwarfs, age, StellarRules::of_version(GENERATOR_VERSION))
    );
    assert_eq!(bound(&dwarfs, age, rules_29), Some(PairLight::Detached));
    let r_p = dwarfs.orbit().periapsis().value() / SOLAR_RADIUS_M;
    assert!(r_p < 50.0, "{r_p} R_sun");
}
