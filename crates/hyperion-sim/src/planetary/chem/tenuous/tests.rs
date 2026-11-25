//! The tenuous air's tests: its loss law against Pluto's measured escape, the ice's temperature
//! against Triton's, and the floor.

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

/// The temperature at which `gas`'s saturation pressure on the registry's curve is `pascals`, K.
fn frost_point(gas: Gas, pascals: f64) -> f64 {
    gas.substance()
        .substance()
        .phase_diagram()
        .and_then(|d| d.saturation_temperature(Pascals::new(pascals)))
        .expect("the gas's curve holds the pressure")
        .value()
}

/// P14.T55.e.a's figure ([`METHANE_TERRAIN_WARMING_K`]) against its source, on the registry's
/// curves: Pluto's measured surface CH₄ ÷ N₂ of 0.006–0.0084 over 11 µbar (Gladstone et al. 2016)
/// is pure methane ice 6.15–6.95 K warmer than β-N₂ at its 10–12 µbar, and the figure lies within
/// that span; Triton's 9.8 nbar of CH₄ (Lellouch et al. 2010) over its 38 ± 1 K ice is stated, not
/// asserted, the smaller warming the rule does not hold (recorded). A temperature that is not one
/// is given back.
#[test]
fn the_methane_terrain_lies_as_much_warmer_than_pluto_s_ice_as_its_measured_methane() {
    let (warmest_ice, coldest_ice) = (
        frost_point(Gas::Nitrogen, 1.2),
        frost_point(Gas::Nitrogen, 1.0),
    );
    let (least, most) = (
        frost_point(Gas::Methane, 0.006 * 1.1),
        frost_point(Gas::Methane, 0.0084 * 1.1),
    );
    let span = (least - warmest_ice, most - coldest_ice);
    let triton = frost_point(Gas::Methane, 9.8e-4);
    eprintln!(
        "Pluto: N2 ice {coldest_ice:.3}–{warmest_ice:.3} K, its methane's terrain {least:.3}–\
         {most:.3} K, warmer by {:.3}–{:.3} K; Triton's 9.8 nbar of CH4 at {triton:.3} K, {:.3} \
         K over its 38 K ice; the rule's {METHANE_TERRAIN_WARMING_K} K",
        span.0,
        span.1,
        triton - 38.0,
    );
    assert!(
        (span.0..=span.1).contains(&METHANE_TERRAIN_WARMING_K),
        "{span:?}"
    );
    let terrain = methane_terrain_temperature(Kelvin::new(37.0));
    assert_same_bits(terrain.value(), 37.0 + METHANE_TERRAIN_WARMING_K);
    assert!(
        methane_terrain_temperature(Kelvin::new(f64::NAN))
            .value()
            .is_nan()
    );
    for t in [-1.0, f64::INFINITY] {
        assert_same_bits(methane_terrain_temperature(Kelvin::new(t)).value(), t);
    }
}

/// P14.T55.e.a's and P14.T55.n's census over P14.T53.c's sets (the 400 whole systems, the golden
/// systems and LOOK72's triaged systems; a measurement, run by name), each system generated afresh
/// under the version's rules:
///
/// - stated, the bodies by state, and the tenuous airs' methane and haze, the least, median and
///   largest of each, the largest hazes by name;
/// - asserted, no tenuous air holds a value that is not finite, its methane stays minor (x(CH₄)
///   under 0.1, `DOMINANCE_ONSET`), its record's aerosols are modelled, and its haze's τ(550) is
///   finite and at most 0.1.
#[test]
#[ignore = "a measurement for P14.T55.e.a's and P14.T55.n's record, run by name"]
fn the_tenuous_airs_are_stated() {
    use std::collections::BTreeMap;

    use crate::planetary::chem::partition::DOMINANCE_ONSET;
    use crate::planetary::derive::atmosphere::SurfaceState;
    use crate::planetary::system::tests::{thin_air_census, thin_air_sets};
    let sets = thin_air_sets();
    let bodies = thin_air_census(&sets);
    let mut states: BTreeMap<String, u32> = BTreeMap::new();
    for body in &bodies {
        *states.entry(format!("{:?}", body.state)).or_default() += 1;
    }
    eprintln!(
        "{} bodies with a surface; by state {states:?}",
        bodies.len()
    );
    let mut methane: Vec<f64> = Vec::new();
    let mut hazes: Vec<(f64, String)> = Vec::new();
    for body in bodies.iter().filter(|b| b.state == SurfaceState::Tenuous) {
        let at = || format!("{}: {}", body.label, body.line());
        for value in [body.p, body.column, body.t_s, body.g] {
            assert!(value.is_finite(), "{}", at());
        }
        assert!(body.mu.is_some_and(f64::is_finite), "{}", at());
        for &(_, x) in &body.gases {
            assert!(x.is_finite() && x >= 0.0, "{}", at());
        }
        let x = body.share(Gas::Methane);
        assert!(x < DOMINANCE_ONSET, "{}", at());
        if x > 0.0 {
            methane.push(x);
        }
        let tau = body
            .aerosol_tau
            .unwrap_or_else(|| panic!("a tenuous record's aerosols not modelled: {}", at()));
        assert!(
            tau.is_finite() && (0.0..=0.1).contains(&tau),
            "τ {tau}: {}",
            at()
        );
        if tau > 0.0 {
            hazes.push((
                tau,
                format!(
                    "{}: {}, flux {:.4e} S⊕",
                    body.label,
                    body.line(),
                    body.flux_earths
                ),
            ));
        }
    }
    hazes.sort_by(|a, b| a.0.total_cmp(&b.0));
    let haze_at = |k: usize| hazes.get(k).map_or(f64::NAN, |h| h.0);
    eprintln!(
        "the tenuous airs' hazes: {} with a haze, τ(550) least {:.4e}, median {:.4e}, largest \
         {:.4e}",
        hazes.len(),
        haze_at(0),
        haze_at(hazes.len() / 2),
        haze_at(hazes.len().saturating_sub(1))
    );
    for (tau, line) in hazes.iter().rev().take(5) {
        eprintln!("  τ {tau:.4e}: {line}");
    }
    methane.sort_by(f64::total_cmp);
    let at = |k: usize| methane.get(k).copied().unwrap_or(f64::NAN);
    eprintln!(
        "the tenuous airs' methane: {} with CH4, x(CH4) least {:.4e}, median {:.4e}, largest \
         {:.4e}",
        methane.len(),
        at(0),
        at(methane.len() / 2),
        at(methane.len().saturating_sub(1))
    );
}
