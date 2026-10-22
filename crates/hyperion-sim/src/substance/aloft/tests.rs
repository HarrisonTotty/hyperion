use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::substance::by_key;

/// The row whose key is `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

/// The law aloft of `key`.
fn law(key: &str) -> LawAloft {
    law_aloft(row(key)).unwrap_or_else(|| panic!("{key} has a law aloft"))
}

/// Pascals at `bar` bar.
fn bar(bar: f64) -> Pascals {
    Pascals::new(bar * PASCALS_PER_BAR)
}

/// The registry's condensates aloft, the block P14.T49.b appended after the chemistry's gases
/// (`NH4SH` … `graphite`): each has a law or is listed as not modelled.
const CONDENSATES_ALOFT: [&str; 17] = [
    "NH4SH", "H2SO4", "S8", "KCl", "NaCl", "ZnS", "Na2S", "MnS", "Cr", "FeS", "MgSiO3", "Mg2SiO4",
    "SiO2", "Al2O3", "CaTiO3", "TiO2", "graphite",
];

/// Each law with a paper's curve gives that curve's condensation temperature at 1 bar and solar
/// metallicity within 10 K with its stated `x₀` (plan 14's P14.T49.b test). As built the largest
/// miss is iron's, +6.2 K (Batalha et al. 2025's x₀ against Visscher et al. 2010's curve), and
/// the next forsterite's, +2.6 K.
#[test]
fn each_law_gives_its_paper_s_condensation_temperature_within_10_k() {
    let mut checked = 0;
    for (id, law) in laws_aloft() {
        let Some(reference) = law.reference() else {
            continue;
        };
        let at = law
            .condensation_temperature(bar(1.0), reference.solar_mole_fraction, 0.0)
            .unwrap_or_else(|| panic!("{} condenses at 1 bar", id.substance().key().as_str()));
        let miss = at.value() - reference.condensation_temperature.value();
        assert!(
            miss.abs() < 10.0,
            "{}: {} K against the paper's {} K",
            id.substance().key().as_str(),
            at.value(),
            reference.condensation_temperature.value()
        );
        checked += 1;
    }
    assert_eq!(checked, 12, "the twelve laws with a paper's curve");
}

/// Ammonium hydrosulphide's product law holds Lodders and Fegley 2024's Table 2 (203 K at 1 bar,
/// 151–246 K over 10⁻⁴–10² bar at solar, Lodders 2020) within 0.5 K at each of its seven
/// pressures, as the lane's research recomputed it (0.46 K the largest, at 10⁻² bar).
#[test]
fn ammonium_hydrosulphide_holds_lodders_and_fegley_s_table() {
    let law = law("NH4SH");
    let product = law.reference().expect("a reference").solar_mole_fraction;
    let table = [
        (1e-4, 151.0),
        (1e-3, 162.0),
        (1e-2, 174.0),
        (1e-1, 187.0),
        (1.0, 203.0),
        (10.0, 222.0),
        (100.0, 246.0),
    ];
    for (p, t) in table {
        let at = law
            .condensation_temperature(bar(p), product, 0.0)
            .expect("it condenses");
        assert!(
            (at.value() - t).abs() < 0.5,
            "{p} bar: {} K against {t} K",
            at.value()
        );
    }
}

/// Kulmala and Laaksonen 1990's curve reproduces the lane's check values (in atmospheres, from
/// the form through Dai et al. 2022 and Vehkamäki et al. 2002) to their four significant figures,
/// and meets Ayers et al.'s fit at T₀ = 360 K, where the curvature vanishes.
#[test]
fn the_acid_s_curve_holds_its_check_values() {
    let law = law("H2SO4");
    // Each value with half a unit of its last figure.
    let checks = [
        (298.15, 1.62e-8, 0.005e-8),
        (338.0, 1.013e-6, 0.0005e-6),
        (360.0, 6.446e-6, 0.0005e-6),
        (400.0, 1.042e-4, 0.0005e-4),
        (445.0, 1.216e-3, 0.0005e-3),
        (500.0, 1.232e-2, 0.0005e-2),
    ];
    for (t, atm, half_figure) in checks {
        let ln_pa = law
            .ln_saturation_pressure(Kelvin::new(t), bar(1.0), 0.0)
            .expect("the acid condenses below 905 K");
        let p = math::exp(ln_pa) / PASCALS_PER_ATMOSPHERE;
        assert!(
            (p - atm).abs() <= half_figure,
            "{t} K: {p} atm against {atm}"
        );
    }
    let ayers = 16.259 - 10_156.0 / 360.0;
    assert!((KULMALA_LAAKSONEN.ln_atm(360.0) - ayers).abs() < 1e-12);
    assert_same_bits(law.vapour_share(), SULPHURIC_ACID_VAPOUR_SHARE);
    // Its condensation temperature inverts its curve.
    let p = bar(0.5);
    let base = law
        .condensation_temperature(p, 7.8e-6, 0.0)
        .expect("8 ppm of acid saturates under 0.5 bar");
    let ln_back = law
        .ln_saturation_pressure(base, p, 0.0)
        .expect("below 905 K");
    assert!((ln_back - math::ln(7.8e-6 * p.value())).abs() < 1e-9);
}

/// Every vapour and product law's condensation temperature inverts its saturation pressure, at
/// every metallicity and pressure the laws are fitted over.
#[test]
fn the_condensation_temperature_inverts_the_saturation_pressure() {
    for (id, law) in laws_aloft() {
        if !matches!(
            law.law(),
            SaturationLaw::Vapour { .. } | SaturationLaw::Product { .. }
        ) {
            continue;
        }
        for (p, m) in [(1e-3, -0.5), (1.0, 0.0), (100.0, 1.0)] {
            let x = 1e-6;
            let t = law
                .condensation_temperature(bar(p), x, m)
                .unwrap_or_else(|| panic!("{} condenses", id.substance().key().as_str()));
            let ln_p = law.ln_saturation_pressure(t, bar(p), m).expect("finite");
            let wanted = match law.law() {
                SaturationLaw::Product { .. } => math::ln(x * bar(p).value() * bar(p).value()),
                _ => math::ln(x * bar(p).value()),
            };
            assert!(
                (ln_p - wanted).abs() < 1e-9,
                "{} at {p} bar and [M] {m}: {ln_p} against {wanted}",
                id.substance().key().as_str()
            );
        }
    }
}

/// A row with a phase diagram takes its own saturation curve, to the bit, and none at or above
/// its critical temperature; a row with neither a closed form nor a diagram has no law.
#[test]
fn a_diagram_row_takes_its_own_curve() {
    let mut diagram_rows = 0;
    for id in crate::substance::condensables() {
        let law = law_aloft(id).expect("a diagram row has a law");
        assert_eq!(law.law(), SaturationLaw::Diagram);
        assert_eq!(
            law.limiting().vapour(),
            Vapour::Species(id.substance().key())
        );
        let diagram = id.substance().phase_diagram().expect("a diagram");
        let t = Kelvin::new(0.8 * diagram.triple_temperature().value());
        let ln_p = law
            .ln_saturation_pressure(t, bar(1.0), 0.0)
            .expect("below the critical point");
        assert_same_bits(
            ln_p,
            diagram
                .ln_saturation_pressure(t)
                .expect("below the critical point"),
        );
        assert_eq!(
            law.ln_saturation_pressure(diagram.critical_temperature(), bar(1.0), 0.0),
            None
        );
        diagram_rows += 1;
    }
    assert_eq!(diagram_rows, 14);
    assert_eq!(law_aloft(row("He")), None);
    assert_eq!(law_aloft(row("basalt")), None);
}

/// Every condensate aloft of the registry has a law or is listed as not modelled, never both;
/// every law's condensate, species and displacing condensate is a row, each species a gas row
/// with a molar mass, and each element reactant an element the condensate holds.
#[test]
fn every_condensate_aloft_has_a_law_or_is_listed() {
    for key in CONDENSATES_ALOFT {
        let listed = NOT_MODELLED.iter().any(|n| n.key == key);
        assert_ne!(
            law_aloft(row(key)).is_some(),
            listed,
            "{key}: a law, or listed as not modelled"
        );
    }
    for n in NOT_MODELLED {
        assert!(
            CONDENSATES_ALOFT.contains(&n.key),
            "{} is a condensate aloft",
            n.key
        );
    }
    for (id, law) in laws_aloft() {
        let condensate = id.substance();
        assert_eq!(condensate.key(), law.condensate());
        for reactant in std::iter::once(law.limiting()).chain(law.others().iter().copied()) {
            assert!(reactant.count() > 0);
            match reactant.vapour() {
                Vapour::Species(key) => {
                    let gas = row(key.as_str()).substance();
                    assert!(gas.gas().is_some(), "{} is a gas row", key.as_str());
                    assert!(gas.molar_mass_g_per_mol().is_some());
                }
                Vapour::Element(element) => {
                    let held = condensate
                        .elements()
                        .iter()
                        .find(|(e, _)| *e == element)
                        .map(|(_, n)| *n);
                    assert_eq!(
                        held,
                        Some(reactant.count()),
                        "{} holds {} {element:?} a formula unit",
                        law.condensate().as_str(),
                        reactant.count()
                    );
                }
            }
        }
        if let Some(displacing) = law.displaced_by() {
            assert!(law_aloft(row(displacing.as_str())).is_some());
        }
        assert_ne!(law.sources(), []);
        assert!(law.vapour_share() > 0.0 && law.vapour_share() <= 1.0);
    }
}

/// The laws are pure functions of their inputs: the same bits on every call and in every order
/// (they hold no cache), and a vapour law's saturation pressure falls with metallicity where its
/// C is positive.
#[test]
fn the_laws_are_pure() {
    let forsterite = law("Mg2SiO4");
    let first = forsterite.ln_saturation_pressure(Kelvin::new(1_700.0), bar(1.0), 0.3);
    let _ = law("Fe").ln_saturation_pressure(Kelvin::new(1_700.0), bar(1.0), 0.3);
    let again = forsterite.ln_saturation_pressure(Kelvin::new(1_700.0), bar(1.0), 0.3);
    assert_same_bits(first.expect("finite"), again.expect("finite"));
    let richer = forsterite
        .ln_saturation_pressure(Kelvin::new(1_700.0), bar(1.0), 1.0)
        .expect("finite");
    assert!(richer < first.expect("finite"));
    assert_eq!(
        forsterite.ln_saturation_pressure(Kelvin::new(f64::NAN), bar(1.0), 0.0),
        None
    );
    assert_eq!(
        forsterite.condensation_temperature(bar(1.0), 0.0, 0.0),
        None
    );
}
