//! The tenuous air's tests: its hold's tripwire, its loss law against Pluto's measured escape, the
//! ice's temperature against Triton's, and the floor.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature};
use crate::units::EarthFluxes;

/// Pluto's mass, kg, and mean radius, m: JPL's GM, 869.3 km³ s⁻² (PLU060, Brozović and Jacobson
/// 2024, AJ 167, 256), and 1,188.3 km (Nimmo et al. 2017).
const PLUTO: (f64, f64) = (869.3e9 / GRAVITATIONAL_CONSTANT, 1_188.3e3);

/// Triton's: JPL's GM, 1,428.495 km³ s⁻², and 1,352.6 km.
const TRITON: (f64, f64) = (1_428.495e9 / GRAVITATIONAL_CONSTANT, 1_352.6e3);

/// Charon's: JPL's GM, 106.1 km³ s⁻² (PLU060), and 606.0 km.
const CHARON: (f64, f64) = (106.1e9 / GRAVITATIONAL_CONSTANT, 606.0e3);

/// The age over which the bodies' losses are stated, yr: the Solar System's 4.5 Gyr.
const AGE: Years = Years::new(4.5e9);

fn mass((kg, _): (f64, f64)) -> Kilograms {
    Kilograms::new(kg)
}

fn radius((_, m): (f64, f64)) -> Metres {
    Metres::new(m)
}

/// The temperature at which β-N₂'s saturation pressure on the registry's curve is `pascals`, K.
fn nitrogen_frost_point(pascals: f64) -> Kelvin {
    crate::substance::by_key("N2")
        .expect("N2 is a registry row")
        .substance()
        .phase_diagram()
        .and_then(|d| d.saturation_temperature(Pascals::new(pascals)))
        .expect("N2's curve holds 1 Pa")
}

/// P14.T53.c is held until the 23 → 24 bump: the rules in force are version 23's, P14.T54.a's
/// ice-held stand-in, with no `Tenuous` state and the ocean under a `Snowball`'s ice listed.
#[test]
fn the_tenuous_air_is_held_only_until_the_24_bump() {
    assert!(
        GENERATOR_VERSION < TENUOUS_VERSION,
        "the 23 → 24 bump (P14.T54.b) puts P14.T53.c on the record, if it rides it: the ice-held \
         retention at the ice's own temperature (`IceHeld::of`'s `TenuousRules::Tenuous` arm), \
         the `Tenuous` state above its floor (`escaping::finish`) and the end of the ocean under \
         a `Snowball`'s ice in \
         `surface_liquids` (`with_surface_under`). Delete `TENUOUS_VERSION`, `TenuousRules` with \
         its `Held` arm (every `match` and test on it keeps its `Tenuous` arm), the tests' \
         `with_tenuous`, its switch in `with_every_24_hold`, and this test; and retire the \
         client's `oceanUnderIce` and `liquidsOnIce` (P14.T43.h's rule), which no 24 record \
         fires"
    );
    assert_eq!(TenuousRules::in_force(), TenuousRules::Held);
}

/// A test's rules hold on its thread inside [`with_tenuous`], nested or not, and the rules before
/// come back after.
#[test]
fn a_test_s_tenuous_rules_hold_inside_with_tenuous_and_are_set_back_after() {
    let outside = TenuousRules::in_force();
    let (tenuous, held) = with_tenuous(TenuousRules::Tenuous, || {
        let held = with_tenuous(TenuousRules::Held, TenuousRules::in_force);
        (TenuousRules::in_force(), held)
    });
    assert_eq!(tenuous, TenuousRules::Tenuous);
    assert_eq!(held, TenuousRules::Held);
    assert_eq!(TenuousRules::in_force(), outside);
}

/// P14.T53.c's check of its loss law (decision-triton-and-e101 §1.8 item 1): Pluto's vapour air
/// over β-N₂ at its measured 10–12 µbar (the registry's curve's 36.65–36.95 K) escapes at
/// [`vapour_escape_molecules_per_second`] within a factor of 10 of its measured 1 × 10²³ N₂ s⁻¹
/// (Gladstone et al. 2016, Science 351, aad8866; Young et al. 2018's (3–7) × 10²²), and loses under
/// 1% of the model's median nitrogen (Earth's 6.5 × 10⁻⁷ per unit mass, × 100 beyond the snow line)
/// over 4.5 Gyr; Pluto's energy-limited 0.7–4 × 10²⁷ s⁻¹, expected before the flyby, would take
/// more than the whole of it.
#[test]
fn pluto_s_vapour_air_escapes_within_a_factor_of_ten_of_its_measured_rate() {
    let median = 6.5e-7 * 100.0 * PLUTO.0;
    for pascals in [1.0, 1.1, 1.2] {
        let ice = nitrogen_frost_point(pascals);
        let rate = vapour_escape_molecules_per_second(
            mass(PLUTO),
            radius(PLUTO),
            Gas::Nitrogen,
            ice,
            Pascals::new(pascals),
        )
        .expect("Pluto's vapour air has an exobase");
        let loss = ice_held_loss(mass(PLUTO), radius(PLUTO), Gas::Nitrogen, ice, AGE)
            .expect("Pluto's ice holds its nitrogen");
        eprintln!(
            "Pluto at {pascals} Pa ({:.3} K): {rate:.3e} N2 s⁻¹, {:.3e} kg over 4.5 Gyr, {:.3e} of \
             its median {median:.3e} kg",
            ice.value(),
            loss.value(),
            loss.value() / median,
        );
        assert!((1e22..=1e24).contains(&rate), "{rate:e}");
        assert!(loss.value() < 0.01 * median, "{loss:?}");
    }
    let energy_limited = 0.7e27 * Gas::Nitrogen.molecular_mass().value() * 4.5e9 * 3.155_76e7;
    assert!(energy_limited > median, "{energy_limited:e}");
}

/// The rule's discriminating case (Johnson et al. 2015, ApJ 809, 43, as Young, Braga-Ribas and
/// Johnson 2020 report: Pluto and Triton keep their nitrogen, Charon has lost its): a Charon's
/// nitrogen ice at Pluto's distance, at Pluto's ice temperature, has an upper atmosphere with no
/// exobase (`λ_s` ≈ 8), so its ice keeps none; Triton's, at its 37.7 K, loses some 10⁹ kg over the
/// age, under 10⁻⁶ of its median.
#[test]
fn a_charon_s_nitrogen_escapes_and_triton_s_is_kept() {
    let ice = nitrogen_frost_point(1.1);
    assert_eq!(
        ice_held_loss(mass(CHARON), radius(CHARON), Gas::Nitrogen, ice, AGE),
        None
    );
    let triton_ice = Kelvin::new(37.7);
    let loss = ice_held_loss(mass(TRITON), radius(TRITON), Gas::Nitrogen, triton_ice, AGE)
        .expect("Triton's ice holds its nitrogen");
    let median = 6.5e-7 * 100.0 * TRITON.0;
    eprintln!(
        "Triton at 37.7 K: {:.3e} kg over 4.5 Gyr, {:.3e} of its median",
        loss.value(),
        loss.value() / median
    );
    assert!(loss.value() < 1e-6 * median, "{loss:?}");
}

/// The ice's temperature (decision-triton-and-e101 §1.6 and §1.8 item 2): Triton about the Sun at
/// Neptune's 30.07 au, under its measured 0.82 Bond albedo with no heat flow, holds its nitrogen
/// ice at 37.7 K, within 2 K of Tryka et al.'s 38 ± 1 K (1994, Icarus 112, 513), and its vapour at
/// 1.9 Pa, within a factor of 3 of Voyager's 1.4 Pa; at the bare surface's 0.95 it would sit near
/// 33.5 K, a factor of 15 low.
#[test]
fn triton_s_nitrogen_ice_lies_at_its_measured_temperature() {
    let flux = EarthFluxes::new(1.0 / (30.07 * 30.07));
    let t_eq = equilibrium_temperature(flux, BondAlbedo::from_fraction(0.82));
    let ice = ice_temperature(Gas::Nitrogen, t_eq, WattsPerSquareMetre::ZERO);
    let vapour = crate::substance::saturation_pressure(Gas::Nitrogen.substance(), ice)
        .expect("N2 has a diagram")
        .value();
    let bare = ice_temperature(Gas::Water, t_eq, WattsPerSquareMetre::ZERO);
    eprintln!(
        "Triton: T_eq {:.3} K, its N2 ice {:.3} K at {vapour:.3} Pa; at 0.95, {:.3} K",
        t_eq.value(),
        ice.value(),
        bare.value()
    );
    assert!((ice.value() - 38.0).abs() < 2.0, "{ice:?}");
    assert!((1.4 / 3.0..=1.4 * 3.0).contains(&vapour), "{vapour}");
    assert!(bare.value() < 34.0, "{bare:?}");
    // The heat flow raises it, and no heat and no light leaves none.
    let warmed = ice_temperature(Gas::Nitrogen, t_eq, WattsPerSquareMetre::new(0.01));
    assert!(warmed > ice, "{warmed:?}");
    assert_same_bits(
        ice_temperature(Gas::Nitrogen, Kelvin::ZERO, WattsPerSquareMetre::ZERO).value(),
        0.0,
    );
}

/// The floor ([`TENUOUS_FLOOR_COLUMN`]): Pluto's 1.1 Pa over 0.62 m s⁻² and Triton's 1.4 Pa over
/// 0.78 m s⁻² reach it; Io's 1 nbar of SO₂ over 1.80 m s⁻², and the 10⁻⁷ Pa of a water-ice moon's
/// sublimation, do not; a pressure or a gravity that is not positive reaches none.
#[test]
fn the_floor_holds_pluto_s_and_triton_s_airs_and_not_io_s() {
    let held = |p: f64, g: f64| holds_tenuous_air(Pascals::new(p), MetresPerSecondSquared::new(g));
    assert!(held(1.1, 0.62));
    assert!(held(1.4, 0.78));
    assert!(!held(1e-4, 1.80));
    assert!(!held(1e-7, 1.43));
    assert!(!held(0.0, 1.0));
    assert!(!held(1.0, 0.0));
    assert!(!held(f64::NAN, 1.0));
}
