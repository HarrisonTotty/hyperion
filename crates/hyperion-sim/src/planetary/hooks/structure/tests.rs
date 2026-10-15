//! P14.T24.e's vertical structure: on the Solar System table, on Venus's measured state, and over
//! every gas row of the registry, pure and mixed.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::DerivedBody;
use crate::planetary::derive::solar::{found, solar_system};
use crate::planetary::hooks::figures::{VolatileHistory, table_figures};
use crate::planetary::hooks::surface::regime::{ClimateInputs, surface_reservoirs};
use crate::planetary::hooks::surface::tests::table;
use crate::substance::{by_key, ids, mixture_heat_capacity_over_r_at};

/// The air's S̃ ÷ R at `t_k` kelvin.
fn entropy_at(gases: &[(SubstanceId, f64)], t_k: f64) -> f64 {
    entropy(gases, Kelvin::new(t_k))
}

/// The registry's gas rows, in registry order.
fn gas_rows() -> Vec<SubstanceId> {
    ids().filter(|id| id.substance().gas().is_some()).collect()
}

/// The row whose key is `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

/// The exact adiabat's temperature at `p` for the air `gases` of `profile`, K: S̃(T) = S̃(`T_s`) +
/// α ln(p ÷ `p_s`), by 64 bisections in ln T within 5% of `guess`, to rounding (a bracket of 0.1
/// in ln T halved 64 times). What the nodes and the power law between them are held to.
fn exact(profile: &RadiativeConvective, gases: &[(SubstanceId, f64)], p: f64, guess: f64) -> f64 {
    let target = entropy(gases, profile.surface_temperature())
        + profile.lapse_factor() * math::ln(p / profile.surface_pressure().value());
    let (mut cold, mut warm) = (math::ln(guess / 1.05), math::ln(guess * 1.05));
    assert!(
        entropy_at(gases, math::exp(cold)) < target && entropy_at(gases, math::exp(warm)) > target,
        "the adiabat at {p} Pa lies within 5% of {guess} K"
    );
    for _ in 0..64 {
        let mid = f64::midpoint(cold, warm);
        if entropy_at(gases, math::exp(mid)) < target {
            cold = mid;
        } else {
            warm = mid;
        }
    }
    math::exp(f64::midpoint(cold, warm))
}

/// The structure of the air `gases` over a ground at `t_s` K and `p_s` Pa under a skin of `skin`
/// K, its condensable `condensable`.
fn profile_of(
    gases: &[(SubstanceId, f64)],
    t_s: f64,
    p_s: f64,
    skin: f64,
    condensable: Option<SubstanceId>,
) -> RadiativeConvective {
    RadiativeConvective::derive(&StructureInputs {
        surface_temperature: Kelvin::new(t_s),
        surface_pressure: Pascals::new(p_s),
        gases,
        condensable,
        skin_temperature: Kelvin::new(skin),
    })
    .unwrap_or_else(|e| panic!("{gases:?} at {t_s} K, {p_s} Pa: {e}"))
}

/// A sweep's air: its name, its mole fractions and its condensable.
type SweepAir = (String, Vec<(SubstanceId, f64)>, Option<SubstanceId>);

/// The airs every sweep holds to the structure's contract: each gas row of the registry pure, and
/// mixtures spanning the generator's surfaces (an Earth's, a Venus's, a Mars's, a Titan's, a
/// steam world's, a hydrogen and helium air's, and an ammonia, sulphur and hydrocarbon air), each
/// with its condensable.
fn sweep_airs() -> Vec<SweepAir> {
    let mut airs: Vec<_> = gas_rows()
        .into_iter()
        .map(|id| {
            (
                id.substance().key().as_str().to_owned(),
                vec![(id, 1.0)],
                None,
            )
        })
        .collect();
    let water = row("H2O");
    let methane = row("CH4");
    for (name, gases, condensable) in [
        (
            "earth",
            vec![
                (row("N2"), 0.77),
                (row("O2"), 0.21),
                (water, 0.01),
                (row("Ar"), 0.01),
            ],
            Some(water),
        ),
        (
            "venus",
            vec![(row("CO2"), 0.965), (row("N2"), 0.035)],
            Some(row("CO2")),
        ),
        (
            "titan",
            vec![(row("N2"), 0.94), (methane, 0.0565), (row("H2"), 0.0035)],
            Some(methane),
        ),
        ("steam", vec![(water, 0.9), (row("CO2"), 0.1)], Some(water)),
        ("hydrogen", vec![(row("H2"), 0.86), (row("He"), 0.14)], None),
        (
            "reducing",
            vec![
                (row("N2"), 0.6),
                (row("NH3"), 0.2),
                (row("CH4"), 0.1),
                (row("H2S"), 0.05),
                (row("SO2"), 0.03),
                (row("C2H6"), 0.02),
            ],
            Some(methane),
        ),
    ] {
        airs.push((name.to_owned(), gases, condensable));
    }
    airs
}

/// The grounds every sweep takes: (`T_s` K, `p_s` Pa, the skin as a share of `T_s`).
const SWEEP_GROUNDS: [(f64, f64, f64); 7] = [
    (40.0, 1.0e4, 0.6),
    (95.0, 1.5e5, 0.68),
    (210.0, 6.0e2, 0.75),
    (288.0, 1.0e5, 0.25),
    (735.0, 9.25e6, 0.3),
    (1_500.0, 1.0e7, 0.5),
    (2_000.0, 1.0e5, 0.9),
];

/// Every node of every sweep air's adiabat solves S̃(`T_ad`) = S̃(`T_s`) + α ln(p ÷ `p_s`) to
/// 10⁻¹² in T: the plan's accuracy, which [`NEWTON_STEPS`] reach from the predictor.
#[test]
fn every_node_solves_its_adiabat_to_rounding() {
    let mut worst = (0.0_f64, String::new());
    for (name, gases, condensable) in sweep_airs() {
        for (t_s, p_s, share) in SWEEP_GROUNDS {
            let profile = profile_of(&gases, t_s, p_s, t_s * share, condensable);
            let nodes = profile.nodes();
            assert_same_bits(nodes[0].temperature().value(), t_s);
            assert_same_bits(nodes[0].pressure().value(), p_s);
            let last = nodes[nodes.len() - 1];
            assert_same_bits(last.temperature().value(), t_s * share);
            for node in &nodes[1..nodes.len() - 1] {
                let p = node.pressure().value();
                let t = node.temperature().value();
                let error = (t / exact(&profile, &gases, p, t) - 1.0).abs();
                if error > worst.0 {
                    worst = (error, format!("{name} at {t_s} K, {p} Pa"));
                }
            }
            // The tropopause node is the adiabat's own at its pressure.
            let at_top = exact(
                &profile,
                &gases,
                last.pressure().value(),
                last.temperature().value(),
            );
            assert!(
                (at_top / last.temperature().value() - 1.0).abs() < 1e-12,
                "{name} at {t_s} K: the tropopause's adiabat is {at_top} K"
            );
        }
    }
    assert!(worst.0 < 1e-12, "worst node: {worst:?}");
}

/// The power law between nodes lies within 5 × 10⁻⁴ of the exact adiabat in T, at each segment's
/// quarter points, over every sweep air's adiabat (science-r08-gases-2.md §1.7.2's tolerance at
/// [`NODE_SPACING_LN_P`]).
#[test]
fn the_power_law_between_nodes_follows_the_adiabat() {
    let mut worst = (0.0_f64, String::new());
    for (name, gases, condensable) in sweep_airs() {
        for (t_s, p_s, share) in SWEEP_GROUNDS {
            let profile = profile_of(&gases, t_s, p_s, t_s * share, condensable);
            for pair in profile.nodes().windows(2) {
                let (upper, lower) = (
                    math::ln(pair[1].pressure().value()),
                    math::ln(pair[0].pressure().value()),
                );
                for f in [0.25, 0.5, 0.75] {
                    let p = math::exp(lower + f * (upper - lower));
                    let got = profile.temperature_at(Pascals::new(p)).value();
                    let error = (got / exact(&profile, &gases, p, got) - 1.0).abs();
                    if error > worst.0 {
                        worst = (error, format!("{name} at {t_s} K, {p} Pa"));
                    }
                }
            }
        }
    }
    assert!(worst.0 < 5e-4, "worst interpolation: {worst:?}");
}

/// The nodes are evenly spaced in ln p, max(16, ⌈ln(`p_s` ÷ `p_trop`) ÷ 0.3⌉) segments of them,
/// with the tropopause where S̃(`T_skin`) = S̃(`T_s`) + α ln(`p_trop` ÷ `p_s`).
#[test]
fn the_nodes_follow_the_spacing_rule() {
    for (name, gases, condensable) in sweep_airs() {
        for (t_s, p_s, share) in SWEEP_GROUNDS {
            let profile = profile_of(&gases, t_s, p_s, t_s * share, condensable);
            let alpha = profile.lapse_factor();
            let span = (entropy_at(&gases, t_s) - entropy_at(&gases, t_s * share)) / alpha;
            let nodes = profile.nodes();
            let segments = u32::try_from(nodes.len() - 1).unwrap();
            let needed = span / NODE_SPACING_LN_P;
            assert!(f64::from(segments) >= needed, "{name} at {t_s} K");
            assert!(
                segments == MIN_SEGMENTS || f64::from(segments - 1) < needed,
                "{name} at {t_s} K: {segments} segments for {needed}"
            );
            let top = profile.tropopause_pressure().value();
            assert!(
                (math::ln(p_s / top) / span - 1.0).abs() < 1e-14,
                "{name} at {t_s} K: the tropopause"
            );
            let step = span / f64::from(segments);
            for pair in nodes.windows(2) {
                let spacing = math::ln(pair[0].pressure().value() / pair[1].pressure().value());
                assert!((spacing / step - 1.0).abs() < 1e-9, "{name} at {t_s} K");
                assert!(spacing <= NODE_SPACING_LN_P * (1.0 + 1e-12));
            }
        }
    }
}

/// `temperature_at` gives `T_s` at the ground and `T_skin` at and above the tropopause, is
/// continuous across every node, never rises with height, and below the ground continues the
/// first segment's power law.
#[test]
fn temperature_at_is_continuous_and_never_rises_with_height() {
    for (name, gases, condensable) in sweep_airs() {
        for (t_s, p_s, share) in SWEEP_GROUNDS {
            let profile = profile_of(&gases, t_s, p_s, t_s * share, condensable);
            let air = VerticalStructure::RadiativeConvective(profile.clone());
            let at = |p: f64| temperature_at(&air, Pascals::new(p)).value();
            assert_same_bits(at(p_s), t_s);
            let skin = t_s * share;
            let top = profile.tropopause_pressure().value();
            for p in [top, top * 0.5, top * 1e-6, 0.0, -1.0] {
                assert_same_bits(at(p), skin);
            }
            assert!(at(f64::NAN).is_nan());
            // Every node, at its own pressure bit for bit, and approached from each side.
            for node in profile.nodes() {
                let p = node.pressure().value();
                let t = node.temperature().value();
                assert_same_bits(at(p), t);
                for side in [p * (1.0 - 1e-12), p * (1.0 + 1e-12)] {
                    assert!(
                        (at(side) / t - 1.0).abs() < 1e-11,
                        "{name} at {t_s} K, {side} Pa"
                    );
                }
            }
            // Never rising from ten times the ground's pressure to a millionth of the tropopause's.
            let (deep, high) = (math::ln(10.0 * p_s), math::ln(1e-6 * top));
            let mut warmer = at(10.0 * p_s);
            for i in 1..=2_000 {
                let p = math::exp(deep + f64::from(i) / 2_000.0 * (high - deep));
                let t = at(p);
                assert!(
                    t <= warmer,
                    "{name} at {t_s} K rises at {p} Pa: {t} > {warmer}"
                );
                warmer = t;
            }
            // Below the ground, the first segment's power law.
            let nodes = profile.nodes();
            let exponent = math::ln(nodes[1].temperature().value() / t_s)
                / math::ln(nodes[1].pressure().value() / p_s);
            let below = at(p_s * math::exp(1.0));
            assert!((below / (t_s * math::exp(exponent)) - 1.0).abs() < 1e-14);
        }
    }
}

/// Asserts that `a` and `b` are the same structure, bit for bit.
fn assert_same_profile(a: &RadiativeConvective, b: &RadiativeConvective) {
    assert_same_bits(
        a.surface_temperature().value(),
        b.surface_temperature().value(),
    );
    assert_same_bits(a.surface_pressure().value(), b.surface_pressure().value());
    assert_same_bits(a.lapse_factor(), b.lapse_factor());
    assert_same_bits(a.skin_temperature().value(), b.skin_temperature().value());
    assert_eq!(a.nodes().len(), b.nodes().len());
    for (x, y) in a.nodes().iter().zip(b.nodes()) {
        assert_same_bits(x.pressure().value(), y.pressure().value());
        assert_same_bits(x.temperature().value(), y.temperature().value());
    }
}

/// The same inputs give the same bits, twice and in another order: the structure is a pure
/// function of its inputs.
#[test]
fn the_same_inputs_give_the_same_bits() {
    let airs = sweep_airs();
    let first: Vec<_> = airs
        .iter()
        .map(|(_, gases, c)| profile_of(gases, 288.0, 1.0e5, 214.0, *c))
        .collect();
    for (i, (_, gases, c)) in airs.iter().enumerate().rev() {
        assert_same_profile(&profile_of(gases, 288.0, 1.0e5, 214.0, *c), &first[i]);
    }
}

/// A constant `c_p` gives Robinson and Catling's power law, T = `T_s` (p ÷ `p_s`)^(α R ÷ `c_p`):
/// argon's 5/2 at every temperature.
#[test]
fn a_constant_heat_capacity_gives_the_power_law() {
    let argon = [(row("Ar"), 1.0)];
    let profile = profile_of(&argon, 300.0, 1.0e5, 150.0, None);
    let beta = DRY_LAPSE_FACTOR / 2.5;
    for node in profile.nodes() {
        let power_law = 300.0 * math::powf(node.pressure().value() / 1.0e5, beta);
        assert!((node.temperature().value() / power_law - 1.0).abs() < 1e-12);
    }
}

/// A skin at least as warm as the ground is held at the ground's temperature: the adiabat is its
/// ground node alone, and the air is isothermal at `T_s` at every pressure, never warmer than its
/// ground.
#[test]
fn a_skin_as_warm_as_the_ground_leaves_an_isothermal_air() {
    let air = [(row("N2"), 1.0)];
    for skin in [80.0, 100.0] {
        let profile = profile_of(&air, 80.0, 1.0e5, skin, None);
        assert_eq!(profile.nodes().len(), 1);
        assert_same_bits(profile.skin_temperature().value(), 80.0);
        assert_same_bits(profile.tropopause_pressure().value(), 1.0e5);
        for p in [1.0e7, 1.0e5, 1.0e2, 0.0] {
            assert_same_bits(profile.temperature_at(Pascals::new(p)).value(), 80.0);
        }
    }
    // A skin within rounding of the ground likewise.
    let profile = profile_of(&air, 80.0, 1.0e5, 80.0 * (1.0 - 1e-15), None);
    assert_eq!(profile.nodes().len(), 1);
    assert_same_bits(profile.skin_temperature().value(), 80.0);
}

/// P14.T54.b's re-check: at the 23 → 24 bump the table's Venus is P14.T57.a's, and T24.e's
/// 300–370 K test at 1 bar is re-run on it (decision-bump-plan-after-22 §1.3; plan 14's Risks,
/// P14.T24.e). This tripwire fails at version 24 until the wiring re-checks it and deletes it.
#[test]
fn the_venus_test_is_rechecked_at_the_23_to_24_bump() {
    assert!(
        crate::GENERATOR_VERSION.get() < 24,
        "P14.T54.b: re-check T24.e's 300–370 K Venus at 1 bar on T57.a's Venus, then delete this \
         tripwire"
    );
}

/// Each refusal names what is not an air. Errors carrying a float are compared by their `Debug`
/// form, Rust's shortest round-trip decimal, which tells every value but a NaN's sign apart.
#[test]
fn the_derivation_refuses_what_is_not_an_air() {
    let n2 = row("N2");
    let gases = [(n2, 1.0)];
    let inputs = StructureInputs {
        surface_temperature: Kelvin::new(288.0),
        surface_pressure: Pascals::new(1.0e5),
        gases: &gases,
        condensable: None,
        skin_temperature: Kelvin::new(214.0),
    };
    let refused = |inputs: StructureInputs<'_>| {
        format!("{:?}", RadiativeConvective::derive(&inputs).unwrap_err())
    };
    let named = |error: DeriveStructureError| format!("{error:?}");
    for p in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            refused(StructureInputs {
                surface_pressure: Pascals::new(p),
                ..inputs
            }),
            named(DeriveStructureError::NoAir(Pascals::new(p)))
        );
    }
    assert_eq!(
        refused(StructureInputs {
            gases: &[],
            ..inputs
        }),
        named(DeriveStructureError::NoAir(Pascals::new(1.0e5)))
    );
    for x in [-0.1, f64::NAN, f64::INFINITY, 0.0, 0.9, 1.0 + 2e-6] {
        assert_eq!(
            refused(StructureInputs {
                gases: &[(n2, x)],
                ..inputs
            }),
            named(DeriveStructureError::Mixture)
        );
    }
    let basalt = row("basalt");
    assert_eq!(
        refused(StructureInputs {
            gases: &[(n2, 0.5), (basalt, 0.5)],
            ..inputs
        }),
        named(DeriveStructureError::NoHeatCapacity(basalt))
    );
    for t in [0.0, -5.0, f64::INFINITY] {
        assert_eq!(
            refused(StructureInputs {
                surface_temperature: Kelvin::new(t),
                ..inputs
            }),
            named(DeriveStructureError::SurfaceTemperature(Kelvin::new(t)))
        );
        assert_eq!(
            refused(StructureInputs {
                skin_temperature: Kelvin::new(t),
                ..inputs
            }),
            named(DeriveStructureError::SkinTemperature(Kelvin::new(t)))
        );
    }
    // A skin of 10⁻³⁰⁰ K lies some 2,800 e-folds of pressure above the ground.
    assert_eq!(
        refused(StructureInputs {
            skin_temperature: Kelvin::new(1e-300),
            ..inputs
        }),
        named(DeriveStructureError::TooDeep)
    );
    // Fractions within the tolerance of 1 are an air.
    let near = [(n2, 1.0 - 5e-7)];
    assert!(
        RadiativeConvective::derive(&StructureInputs {
            gases: &near,
            ..inputs
        })
        .is_ok()
    );
    assert_eq!(
        DeriveStructureError::NoHeatCapacity(basalt).to_string(),
        "basalt has no gas columns, so no heat capacity"
    );
}

/// The lapse factor is water's 0.6 and methane's 0.77 where their latent heat can drive the lapse
/// rate, and the dry 0.88 for carbon dioxide, every other condensable, none, a scarce water and a
/// water above its critical temperature.
#[test]
fn the_lapse_factor_follows_the_condensable_and_its_latent_heat() {
    let (water, methane, co2, n2, ar) = (row("H2O"), row("CH4"), row("CO2"), row("N2"), row("Ar"));
    let earth = [(n2, 0.77), (row("O2"), 0.21), (water, 0.01), (ar, 0.01)];
    let t = Kelvin::new(288.0);
    assert_same_bits(lapse_factor(&earth, t, Some(water)), WATER_LAPSE_FACTOR);
    assert_same_bits(lapse_factor(&earth, t, None), DRY_LAPSE_FACTOR);
    let titan = [(n2, 0.94), (methane, 0.06)];
    assert_same_bits(
        lapse_factor(&titan, Kelvin::new(94.0), Some(methane)),
        METHANE_LAPSE_FACTOR,
    );
    for condensable in [co2, n2, ar] {
        assert_same_bits(
            lapse_factor(
                &[(co2, 0.95), (n2, 0.03), (ar, 0.02)],
                Kelvin::new(150.0),
                Some(condensable),
            ),
            DRY_LAPSE_FACTOR,
        );
    }
    // A world whose only condensable is water at a Mars's partial pressure (210 ppm of 6.1 mbar,
    // the regime classifier's own Mars) takes the dry α.
    let mars = [(co2, 0.951), (n2, 0.028), (ar, 0.0208), (water, 2.1e-4)];
    assert_same_bits(
        lapse_factor(&mars, Kelvin::new(210.0), Some(water)),
        DRY_LAPSE_FACTOR,
    );
    // Water above its critical point releases no latent heat at the ground.
    let steam = [(water, 0.5), (co2, 0.5)];
    assert_same_bits(
        lapse_factor(&steam, Kelvin::new(700.0), Some(water)),
        DRY_LAPSE_FACTOR,
    );
    assert_same_bits(
        lapse_factor(&steam, Kelvin::new(600.0), Some(water)),
        WATER_LAPSE_FACTOR,
    );
    // A condensable the air does not hold drives nothing.
    assert_same_bits(lapse_factor(&[(n2, 1.0)], t, Some(water)), DRY_LAPSE_FACTOR);
}

/// The gate's molar form is signoff-3's ε L² r ÷ (`c_p` R T²) with every quantity per unit mass,
/// and its departures are those the sign-off computed by hand: about 1.6 for Earth's water, 1.8
/// for Titan's methane and 0.05 for a Mars's.
#[test]
fn the_latent_heat_departure_is_the_dilute_pseudoadiabat_s() {
    let (water, methane, n2, co2, ar) = (row("H2O"), row("CH4"), row("N2"), row("CO2"), row("Ar"));
    let earth_water = crate::substance::saturation_pressure(water, Kelvin::new(288.0))
        .unwrap()
        .value()
        / 1.0e5;
    for (gases, t, condensable, near) in [
        // Earth at 288 K under 1 bar, its water saturated, as the sign-off took it.
        (
            vec![
                (n2, 0.77),
                (row("O2"), 0.2),
                (water, earth_water),
                (ar, 0.03 - earth_water),
            ],
            288.0,
            water,
            1.6,
        ),
        // Titan at 94 K, its methane at Huygens's 5.65%.
        (vec![(n2, 0.9435), (methane, 0.0565)], 94.0, methane, 1.8),
        // Mars at 210 K, its water at 210 ppm.
        (
            vec![(co2, 0.951), (n2, 0.028), (ar, 0.0208), (water, 2.1e-4)],
            210.0,
            water,
            0.05,
        ),
    ] {
        let departure = latent_heat_departure(&gases, Kelvin::new(t), condensable).unwrap();
        // The sign-off's figures are by hand, its enthalpies from memory: within a quarter.
        assert!(
            (departure / near - 1.0).abs() < 0.25,
            "{departure} against {near}"
        );
        // Per unit mass: ε L² r ÷ (c_p R_air T²).
        let phase = condensable.substance().phase().unwrap();
        let latent_molar = if t < phase.triple_temperature().value() {
            phase.sublimation_enthalpy_j_per_mol()
        } else {
            phase.vaporisation_enthalpy_j_per_mol()
        };
        let m_c = condensable.substance().molar_mass_g_per_mol().unwrap() * 1e-3;
        let m_air = mean_molar_mass_g_per_mol(&gases).unwrap() * 1e-3;
        let x = gases.iter().find(|(id, _)| *id == condensable).unwrap().1;
        let latent = latent_molar / m_c;
        let r_air = MOLAR_GAS_CONSTANT / m_air;
        let c_p = mixture_heat_capacity_over_r_at(&gases, Kelvin::new(t)).unwrap() * r_air;
        let epsilon = m_c / m_air;
        let mixing_ratio = x * m_c / ((1.0 - x) * (m_air - x * m_c) / (1.0 - x));
        let by_mass = epsilon * latent * latent * mixing_ratio / (c_p * r_air * t * t);
        assert!(
            (departure / by_mass - 1.0).abs() < 1e-12,
            "{departure} against {by_mass}"
        );
    }
    let pure = latent_heat_departure(&[(water, 1.0)], Kelvin::new(300.0), water).unwrap();
    assert!(pure.is_infinite() && pure > 0.0);
    assert!(latent_heat_departure(&[(n2, 1.0)], Kelvin::new(0.0), water).is_none());
    assert!(latent_heat_departure(&[(n2, 1.0)], Kelvin::new(80.0), row("H2")).is_none());
}

/// Venus's measured state, by hand: 90.4 bar of carbon dioxide and 2.09 of nitrogen by weight
/// (3.51% nitrogen by number), 735 K at 92.5 bar (science-r08-gases-2.md §1.4).
fn measured_venus() -> Vec<(SubstanceId, f64)> {
    let (co2, n2) = (row("CO2"), row("N2"));
    let moles = |id: SubstanceId, bar: f64| bar / id.substance().molar_mass_g_per_mol().unwrap();
    let (c, n) = (moles(co2, 90.4), moles(n2, 2.09));
    vec![(co2, c / (c + n)), (n2, n / (c + n))]
}

/// The Venus International Reference Atmosphere's low-latitude levels, (bar, K): its 0, 10, 20,
/// 30, 40 and 50 km (Seiff et al. 1985, Adv. Space Res. 5(11), 3, read through Venus-GRAM's
/// `VIRALow.txt`, "mean values … from the VIRA data for latitudes less than 30°" by its User
/// Guide, Justh et al., NASA/TM-20210022168, §2.1; the EPIC model's `VIRA.dat`, from the
/// Planetary Scientist's Companion, p. 119, agrees). The original was not reachable.
const VIRA: [(f64, f64); 6] = [
    (92.10, 735.3),
    (47.39, 658.2),
    (22.52, 580.7),
    (9.581, 496.9),
    (3.501, 417.6),
    (1.066, 350.5),
];

/// The dry factor's refit: from Venus's measured state to VIRA's 350.5 K at 1.066 bar on
/// P14.T49.b's `c_p`(T), α = (S̃(350.5 K) − S̃(735 K)) ÷ ln(1.066 ÷ 92.5), is 0.884, which
/// [`DRY_LAPSE_FACTOR`] holds to two places; the measured Venus's adiabat at 0.88 then lies at
/// science-r08-gases-2.md §1.4's levels.
#[test]
fn the_dry_factor_is_refitted_on_venus_s_measured_state() {
    let venus = measured_venus();
    assert!((venus[1].1 - 0.0351).abs() < 1e-4, "{venus:?}");
    let fit =
        (entropy_at(&venus, VIRA[5].1) - entropy_at(&venus, 735.0)) / math::ln(VIRA[5].0 / 92.5);
    assert!((fit - 0.8844).abs() < 5e-4, "the fit gives {fit}");
    assert!(
        (fit - DRY_LAPSE_FACTOR).abs() < 0.005,
        "{fit} rounds to 0.88"
    );
    // At the fitted α, 350.5 K at 1.066 bar by construction, and the ruling's 346 K at 1 bar.
    let fitted_at = |bar: f64| {
        let target = entropy_at(&venus, 735.0) + fit * math::ln(bar / 92.5);
        let (mut cold, mut warm) = (math::ln(200.0), math::ln(800.0));
        for _ in 0..64 {
            let mid = f64::midpoint(cold, warm);
            if entropy_at(&venus, math::exp(mid)) < target {
                cold = mid;
            } else {
                warm = mid;
            }
        }
        math::exp(f64::midpoint(cold, warm))
    };
    assert!((fitted_at(1.066) - 350.5).abs() < 1e-9);
    assert!((fitted_at(1.0) - 346.0).abs() < 0.5, "{} K", fitted_at(1.0));
    // The measured Venus at the dry factor, its skin under its own 226.6 K effective temperature
    // (NASA's fact sheet's black-body temperature): the rounded α's warmth at 1.066 and 1 bar
    // under 2 K (1.47 K as built), and 300–370 K at 1 bar, the plan's Venus test on the measured
    // state.
    let profile = profile_of(&venus, 735.0, 92.5e5, 226.6 * math::powf(0.5, 0.25), None);
    assert_same_bits(profile.lapse_factor(), DRY_LAPSE_FACTOR);
    let at = |bar: f64| profile.temperature_at(Pascals::new(bar * 1e5)).value();
    for bar in [1.066, 1.0] {
        let warm = at(bar) - fitted_at(bar);
        assert!((0.0..2.0).contains(&warm), "{warm} K warm at {bar} bar");
    }
    assert!((300.0..370.0).contains(&at(1.0)), "{} K at 1 bar", at(1.0));
    // At VIRA's 10, 20 and 40 km the ruling computed 665, 592 and 435 K, against VIRA's 658.2,
    // 580.7 and 417.6 K: the single α runs warm where VIRA's 30–45 km is sub-adiabatic.
    for (bar, ruling) in [(47.39, 665.0), (22.52, 592.0), (3.501, 435.0)] {
        assert!((at(bar) - ruling).abs() < 1.5, "{} K at {bar} bar", at(bar));
    }
    for &(bar, vira) in &VIRA[1..5] {
        let warm = at(bar) - vira;
        assert!((0.0..25.0).contains(&warm), "{warm} K warm at {bar} bar");
    }
}

/// The table's body `name`: its derivation, its conditions, and the inputs its structure reads,
/// its condensable its climate regime's.
fn table_structure(name: &str) -> (DerivedBody, SurfaceConditions, RadiativeConvective) {
    let (_, body, conditions) = table()
        .into_iter()
        .find(|(n, ..)| *n == name)
        .unwrap_or_else(|| panic!("{name} is in the table"));
    let figures = table_figures(&body).expect("the table's bodies have figures");
    let published = figures
        .clone()
        .with_surface(&conditions, VolatileHistory::NONE);
    let reservoirs = surface_reservoirs(body.atmosphere());
    let conditions = conditions.with_climate(&ClimateInputs {
        mass: body.mass(),
        reservoirs: &reservoirs,
        from_colder: false,
        ice_fraction: published.ice_fraction(),
        ice_threshold: published.ice_threshold(),
    });
    let inputs = StructureInputs::of_surface(
        &conditions,
        body.equilibrium_temperature(),
        figures.heat_flow(),
    )
    .expect("the conditions carry their regime");
    let profile = RadiativeConvective::derive(&inputs).unwrap();
    (body, conditions, profile)
}

/// The table's Earth, Venus and Titan meet the plan's tests: Earth's tropopause at 0.12–0.25 bar
/// and 205–220 K, and Titan's below 0.3 bar at 60–75 K, with water's, the dry and methane's α.
///
/// The table's Venus is P14.T13.c's 735 K at 57.8 bar with 1.65% nitrogen until P14.T57.a's refit
/// lands, and gives 381.1 K at 1 bar, 1.1 K above the plan's interim 300–380 K (computed by hand
/// as 378 K on other inputs). That is recorded and deferred (plan 14's Risks, P14.T24.e), and the
/// plan's 300–370 K is held on the measured Venus
/// (`the_dry_factor_is_refitted_on_venus_s_measured_state`) and re-checked by P14.T54.b on
/// T57.a's Venus. Here the table's Venus, its column shallower than the measured one, ends warmer
/// at 1 bar, and the golden pins it.
#[test]
fn the_table_s_tropopauses_skins_and_lapse_factors_fall_in_their_ranges() {
    let (_, _, earth) = table_structure("Earth");
    assert_same_bits(earth.lapse_factor(), WATER_LAPSE_FACTOR);
    let top = earth.tropopause_pressure().value();
    assert!(
        (0.12e5..0.25e5).contains(&top),
        "Earth's tropopause at {top} Pa"
    );
    let skin = earth.skin_temperature().value();
    assert!((205.0..220.0).contains(&skin), "Earth's skin {skin} K");

    let (_, venus_conditions, venus) = table_structure("Venus");
    assert_same_bits(venus.lapse_factor(), DRY_LAPSE_FACTOR);
    assert!(
        venus_conditions
            .climate_regime()
            .unwrap()
            .condensable()
            .is_none()
    );
    let one_bar = venus.temperature_at(Pascals::new(1.0e5)).value();
    let measured = profile_of(
        &measured_venus(),
        735.0,
        92.5e5,
        venus.skin_temperature().value(),
        None,
    );
    let measured_one_bar = measured.temperature_at(Pascals::new(1.0e5)).value();
    assert!(
        one_bar > measured_one_bar,
        "Venus at 1 bar: {one_bar} K, the measured {measured_one_bar} K"
    );

    let (_, _, titan) = table_structure("Titan");
    assert_same_bits(titan.lapse_factor(), METHANE_LAPSE_FACTOR);
    let top = titan.tropopause_pressure().value();
    assert!(top < 0.3e5, "Titan's tropopause at {top} Pa");
    let skin = titan.skin_temperature().value();
    assert!((60.0..75.0).contains(&skin), "Titan's skin {skin} K");

    // The table's Mars holds its water at about 200 ppm and takes the dry α.
    let (_, mars_conditions, mars) = table_structure("Mars");
    assert_eq!(
        mars_conditions.climate_regime().unwrap().condensable(),
        Some(row("H2O"))
    );
    assert_same_bits(mars.lapse_factor(), DRY_LAPSE_FACTOR);
}

/// The skin is 2^(−1/4) `T_eff` (decision-composition §1.12): Earth's, warmed by its 36 K of
/// internal heat, within 0.05 K of P14.T13.c's sunlight-only 2^(−1/4) `T_eq`; a giant's, from its
/// record's equilibrium and effective temperatures, 2^(−1/4) of its own `T_eff` (the table's
/// Jupiter's, and its Saturn's, whose effective temperature is its equilibrium one: the generated
/// Saturn, 0.2994 Jupiter masses, lies below the giant cooling model's 0.3 and carries no internal
/// luminosity, a finding recorded for P14.T11.d); and the measured giants' skins of §1.12's table,
/// 79.9 K for Saturn's 95.0 K (Voyager's, Hanel et al. 1983, Icarus 53, 262, as Li et al. 2010,
/// JGR 115, E11002, Table 3, reproduces it; Cassini's is 96.67 ± 0.17 K, Li et al.'s abstract) and
/// 104.6 K for Jupiter's 124.4 K (Hanel et al. 1981, JGR 86, 8705, abstract).
#[test]
fn the_skin_is_the_grey_atmosphere_s_with_internal_heat() {
    let (body, _, earth) = table_structure("Earth");
    let sunlight = body.equilibrium_temperature().value() * math::powf(0.5, 0.25);
    let skin = earth.skin_temperature().value();
    assert!(
        skin > sunlight && skin - sunlight < 0.05,
        "{skin} against {sunlight}"
    );

    let planets = solar_system();
    for name in ["Jupiter", "Saturn"] {
        let giant = found(&planets, name);
        let equilibrium = giant.equilibrium_temperature();
        let effective = giant.effective_temperature().unwrap_or(equilibrium);
        let internal = Kelvin::new(math::powf(
            math::powi(effective.value(), 4) - math::powi(equilibrium.value(), 4),
            0.25,
        ));
        let skin = skin_temperature(effective_temperature(equilibrium, internal));
        let own = skin_temperature(effective);
        assert!(
            (skin.value() - own.value()).abs() < 2.0,
            "{name}'s skin {} K against {} K",
            skin.value(),
            own.value()
        );
        assert!((skin.value() / own.value() - 1.0).abs() < 1e-12, "{name}");
    }
    assert!(found(&planets, "Jupiter").effective_temperature().is_some());
    assert!(found(&planets, "Saturn").effective_temperature().is_none());
    for (effective, skin) in [(95.0, 79.9), (124.4, 104.6)] {
        let got = skin_temperature(Kelvin::new(effective)).value();
        assert!((got - skin).abs() < 0.05, "{got} K for {effective} K");
    }

    for flow in [0.0, -1.0, f64::NAN] {
        assert_same_bits(
            internal_temperature(WattsPerSquareMetre::new(flow)).value(),
            0.0,
        );
    }
}

/// `StructureInputs::of_surface` reads the conditions' mean temperature, pressure and gases, its
/// regime's condensable, and the skin of the equilibrium temperature with the heat flow's
/// internal temperature; conditions without their regime give none.
#[test]
fn the_inputs_of_a_surface_are_its_conditions() {
    let (body, conditions, profile) = table_structure("Earth");
    let figures = table_figures(&body).unwrap();
    let inputs = StructureInputs::of_surface(
        &conditions,
        body.equilibrium_temperature(),
        figures.heat_flow(),
    )
    .unwrap();
    assert_same_bits(
        inputs.surface_temperature.value(),
        conditions.mean_surface_temperature().value(),
    );
    assert_same_bits(
        inputs.surface_pressure.value(),
        conditions.surface_pressure().value(),
    );
    assert_eq!(inputs.gases.len(), conditions.gases().len());
    for (&(a, x), &(b, y)) in inputs.gases.iter().zip(conditions.gases()) {
        assert_eq!(a, b);
        assert_same_bits(x, y);
    }
    assert_eq!(inputs.condensable, Some(row("H2O")));
    assert_same_bits(
        inputs.skin_temperature.value(),
        skin_temperature(effective_temperature(
            body.equilibrium_temperature(),
            internal_temperature(figures.heat_flow()),
        ))
        .value(),
    );
    assert_same_profile(&RadiativeConvective::derive(&inputs).unwrap(), &profile);
    let (_, first_stage) = table()
        .into_iter()
        .find(|(n, ..)| *n == "Earth")
        .map(|(_, b, c)| (b, c))
        .unwrap();
    assert!(
        StructureInputs::of_surface(
            &first_stage,
            body.equilibrium_temperature(),
            figures.heat_flow()
        )
        .is_none()
    );
}

/// The pressures, Pa, at which the golden states each body's temperature beside its nodes: twice
/// and once its ground's, then from 1 bar to 1 Pa, and every segment's geometric midpoint.
fn golden_levels(profile: &RadiativeConvective) -> Vec<f64> {
    let p_s = profile.surface_pressure().value();
    let mut levels = vec![2.0 * p_s, p_s];
    levels.extend([
        1.0e5, 5.0e4, 2.0e4, 1.0e4, 5.0e3, 2.0e3, 1.0e3, 1.0e2, 1.0e1, 1.0,
    ]);
    for pair in profile.nodes().windows(2) {
        levels.push((pair[0].pressure().value() * pair[1].pressure().value()).sqrt());
    }
    levels
}

/// The table's Earth, Venus and Titan, pinned: each structure's inputs and parameters, its nodes,
/// and `temperature_at` at stated pressures, which rendering plan R08's `temperatureAt` is held to
/// at 10⁻¹² (P14.T24.e). Floats are their bits with Rust's shortest round-trip decimal beside.
#[test]
fn the_structure_is_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    w.line("# P14.T24.e's vertical structure of the Solar System table's Earth, Venus and Titan");
    w.line(
        "# (hyperion_sim::planetary::hooks::structure), for rendering plan R08's temperatureAt.",
    );
    w.line(
        "# Per body: its T_s (K), p_s (Pa), alpha, T_eq and T_int (K), T_skin (K) and segments;",
    );
    w.line(
        "# node[k] its adiabat's k-th node, pressure (Pa) then temperature (K), from the ground",
    );
    w.line(
        "# to the tropopause, evenly in ln p; level[k] temperature_at at a stated pressure. The",
    );
    w.line("# temperature between two nodes is the power law through them, below the ground the");
    w.line("# first segment's, and above the last node T_skin.");
    for name in ["Earth", "Venus", "Titan"] {
        let (body, conditions, profile) = table_structure(name);
        let figures = table_figures(&body).unwrap();
        let n = name.to_lowercase();
        w.f64(
            &format!("{n}.surface_temperature_k"),
            profile.surface_temperature().value(),
        );
        w.f64(
            &format!("{n}.surface_pressure_pa"),
            profile.surface_pressure().value(),
        );
        w.line(&format!(
            "{n}.condensable = {}",
            conditions
                .climate_regime()
                .and_then(|r| r.condensable())
                .map_or_else(
                    || "None".to_owned(),
                    |id| id.substance().key().as_str().to_owned()
                )
        ));
        w.f64(&format!("{n}.lapse_factor"), profile.lapse_factor());
        w.f64(
            &format!("{n}.equilibrium_temperature_k"),
            body.equilibrium_temperature().value(),
        );
        w.f64(
            &format!("{n}.internal_temperature_k"),
            internal_temperature(figures.heat_flow()).value(),
        );
        w.f64(
            &format!("{n}.skin_temperature_k"),
            profile.skin_temperature().value(),
        );
        w.line(&format!("{n}.segments = {}", profile.nodes().len() - 1));
        for (k, node) in profile.nodes().iter().enumerate() {
            w.f64(
                &format!("{n}.node[{k:02}].pressure_pa"),
                node.pressure().value(),
            );
            w.f64(
                &format!("{n}.node[{k:02}].temperature_k"),
                node.temperature().value(),
            );
        }
        for (k, p) in golden_levels(&profile).into_iter().enumerate() {
            w.f64(&format!("{n}.level[{k:02}].pressure_pa"), p);
            w.f64(
                &format!("{n}.level[{k:02}].temperature_k"),
                profile.temperature_at(Pascals::new(p)).value(),
            );
        }
    }
    golden!("planetary/vertical_structure", w.as_str());
}
