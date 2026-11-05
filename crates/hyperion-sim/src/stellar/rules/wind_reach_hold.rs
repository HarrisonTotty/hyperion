//! The tests of the wind's reach (P11.T12's fix): the census's pair bound calls a pair `Detached`
//! only where neither star can take 10⁻⁴ M☉ of its partner's wind by the window's end, read at one
//! choke point, `light::detached`. Its hold is deleted (the 29 fold); these are its tests of the
//! new arm.
//!
//! - A wide pair whose secondary takes its primary's AGB wind: read from the tables, and refused by
//!   the brief's gate. The census's bound on such grown stars is checked by `sky::census::cell`'s
//!   `the_star_bound_holds_for_*` tests.
//! - The cheap path: two red dwarfs, whose winds are nil, and a pair too wide to feed, stay
//!   `Detached`.
//!
//! The screen against the generator's wind-fed pairs is tested in
//! `stellar/binary/wind_reach/tests.rs`, and over the generated pairs' tallies in
//! `stellar/binary/light.rs` (`detached_generated_pairs_are_passed_over`).

use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::stellar::Composition;
use crate::stellar::binary::{
    BinaryInput, PairLight, can_interact, pair_keeps_own_models, pair_light_bound,
};
use crate::stellar::draws::StarDraws;
use crate::units::consts::{METRES_PER_AU, SOLAR_RADIUS_M};
use crate::units::{GravitationalParameter, Metres, Radians, SolarMasses, Years};

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

/// The bound on `input` at `age` years.
fn bound(input: &BinaryInput, age: f64) -> Option<PairLight> {
    let [m1, m2] = input.masses();
    let at = Years::new(age);
    pair_light_bound(
        m1,
        m2,
        input.orbit().periapsis(),
        input.composition(),
        at..=at,
    )
}

/// A pair of 1.4 and 1.0 M☉ 58 au apart at periastron: the lobe and decay tests pass it over, but
/// its secondary takes its primary's AGB wind, 10⁻⁴ M☉ by 5 Gyr. The bound reads the tables and
/// the brief's gate refuses it, where version 28's bound called it `Detached`, a broken contract,
/// and its gate certified it; the pair 10⁴ au out at periastron, which takes too little, stays
/// `Detached`. Before the primary's giant branch both stars are their own models.
#[test]
fn a_wind_fed_pair_is_read_from_the_tables() {
    let fed = pair(1.4, 1.0, 58.0, 0.5);
    let age = 5.0e9;
    assert!(can_interact(&fed, Years::new(age)), "the pair is wind-fed");
    let [m1, m2] = fed.masses();
    let at = Years::new(age);
    let verdict = bound(&fed, age);
    assert_ne!(verdict, Some(PairLight::Detached), "{verdict:?}");
    assert!(
        matches!(verdict, Some(PairLight::Bright(_)) | None),
        "{verdict:?}"
    );
    // The brief's gate reads the same `Detached`.
    assert!(!pair_keeps_own_models(
        m1,
        m2,
        fed.orbit().periapsis(),
        &Composition::SOLAR,
        at..=at,
    ));
    // Young, its primary has lost no wind.
    assert!(!can_interact(&fed, Years::new(1.0e9)));
    assert_eq!(bound(&fed, 1.0e9), Some(PairLight::Detached));
    // 10⁴ au out at periastron the secondary takes too little.
    let wide = pair(1.4, 1.0, 1.0e4, 0.5);
    assert!(!can_interact(&wide, Years::new(age)));
    assert_eq!(bound(&wide, age), Some(PairLight::Detached));
}

/// Two red dwarfs lose no wind by 13 Gyr, so the wind's reach leaves even a close pair of them,
/// which the lobe and decay tests pass over, `Detached`.
#[test]
fn two_red_dwarfs_stay_detached_under_29() {
    let dwarfs = pair(0.5, 0.3, 0.2, 0.1);
    let age = 1.3e10;
    assert!(!can_interact(&dwarfs, Years::new(age)));
    assert_eq!(bound(&dwarfs, age), Some(PairLight::Detached));
    let r_p = dwarfs.orbit().periapsis().value() / SOLAR_RADIUS_M;
    assert!(r_p < 50.0, "{r_p} R_sun");
}
