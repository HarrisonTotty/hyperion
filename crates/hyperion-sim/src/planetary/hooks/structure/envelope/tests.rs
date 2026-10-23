//! P14.T24.d's envelope profile: Guillot's grey profile, its join to the dry adiabat and the
//! adiabat's nodes, on measured giants given by hand, a hot Jupiter and a sub-Neptune.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::hooks::structure::MIN_SEGMENTS;
use crate::substance::by_key;

/// The row whose key is `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

/// Jupiter's measured envelope by number: the Galileo probe's helium, He ÷ H₂ 0.157 (von Zahn et
/// al. 1998; `x_He` 0.1359), and its methane, CH₄ ÷ H₂ 2.1 × 10⁻³ (Niemann et al. 1998).
fn jupiter_gases() -> Vec<(SubstanceId, f64)> {
    vec![
        (row("H2"), 0.8623),
        (row("He"), 0.1359),
        (row("CH4"), 0.0018),
    ]
}

/// A hydrogen–helium envelope at protosolar helium, He ÷ H₂ 0.194 (by number).
fn protosolar_gases() -> Vec<(SubstanceId, f64)> {
    let he = 0.194 / 1.194;
    vec![(row("H2"), 1.0 - he), (row("He"), he)]
}

/// The grey parameters the tests take where a test's own parameters are not the point: an opacity
/// rising as p (n = 2) at 3 × 10⁻³ m² kg⁻¹ at 1 bar, γ 0.4, the planet's mean.
fn grey() -> GreyProfile {
    GreyProfile::new(3e-3, 2.0, 0.4, MEAN_REDISTRIBUTION).unwrap()
}

/// The profile of an envelope of `gases` at internal and equilibrium temperatures `t_int` and
/// `t_eq`, K, under gravity `g`, m s⁻², with Guillot's parameters `grey`.
fn profile(
    gases: &[(SubstanceId, f64)],
    t_int: f64,
    t_eq: f64,
    g: f64,
    grey: GreyProfile,
) -> Result<EnvelopeProfile, DeriveEnvelopeProfileError> {
    EnvelopeProfile::derive(&EnvelopeProfileInputs {
        internal_temperature: Kelvin::new(t_int),
        irradiation_temperature: irradiation_temperature(Kelvin::new(t_eq)),
        gravity: MetresPerSecondSquared::new(g),
        gases,
        grey,
    })
}

/// A Jupiter by hand: its measured gases, `T_int` 99 K and `T_eq` 102 K (Li et al. 2018's Bond
/// albedo 0.503), 24.79 m s⁻².
fn jupiter() -> EnvelopeProfile {
    profile(&jupiter_gases(), 99.0, 102.0, 24.79, grey()).unwrap()
}

/// The cases every structural test runs over: a Jupiter, a Neptune, a hot Jupiter and a
/// sub-Neptune, each by hand.
fn cases() -> Vec<(&'static str, EnvelopeProfile)> {
    let protosolar = protosolar_gases();
    vec![
        ("Jupiter", jupiter()),
        (
            "Neptune",
            profile(&protosolar, 52.0, 46.6, 11.15, grey()).unwrap(),
        ),
        (
            "hot Jupiter",
            profile(&protosolar, 300.0, 1_450.0, 9.4, grey()).unwrap(),
        ),
        (
            "sub-Neptune",
            profile(&protosolar, 60.0, 550.0, 8.9, grey()).unwrap(),
        ),
    ]
}

/// Guillot's Eq. 29 as written: at τ = 0 its skin is `T_int`⁴ ÷ 2 + f `T_irr`⁴ (½ + √3 γ ÷ 4), deep
/// T⁴ ÷ τ tends to ¾ `T_int`⁴, and at γ = 1 the starlight's term is the same at every depth.
#[test]
fn the_grey_profile_is_guillot_s_eq_29() {
    let (internal, irradiation) = (
        Kelvin::new(99.0),
        irradiation_temperature(Kelvin::new(102.0)),
    );
    for gamma in [0.1, 0.4, 1.0, 2.5] {
        let grey = GreyProfile::new(1e-3, 1.0, gamma, MEAN_REDISTRIBUTION).unwrap();
        let skin = grey.fourth_power(0.0, internal, irradiation);
        let expected = math::powi(99.0, 4) / 2.0
            + MEAN_REDISTRIBUTION
                * math::powi(irradiation.value(), 4)
                * (0.5 + SQRT_3 * gamma / 4.0);
        assert!(
            (skin / expected - 1.0).abs() < 1e-13,
            "γ {gamma}: {skin} against {expected}"
        );
        let deep = grey.fourth_power(1e9, internal, irradiation) / 1e9;
        assert!(
            (deep / (0.75 * math::powi(99.0, 4)) - 1.0).abs() < 1e-8,
            "γ {gamma}"
        );
    }
    let flat = GreyProfile::new(1e-3, 1.0, 1.0, MEAN_REDISTRIBUTION).unwrap();
    let starlight = |tau: f64| flat.fourth_power(tau, Kelvin::new(0.0), irradiation);
    for tau in [0.0, 0.3, 3.0, 300.0] {
        assert!(
            (starlight(tau) / starlight(0.0) - 1.0).abs() < 1e-14,
            "τ {tau}"
        );
    }
}

/// The irradiation temperature is the absorbed starlight's, `T_irr`⁴ = 4 `T_eq`⁴, so that the
/// planet's mean gives `T_eq` back.
#[test]
fn the_mean_irradiation_is_the_equilibrium_temperature() {
    for t_eq in [46.6, 102.0, 1_450.0, 4_050.0] {
        let t_irr = irradiation_temperature(Kelvin::new(t_eq)).value();
        let back = math::powf(MEAN_REDISTRIBUTION * math::powi(t_irr, 4), 0.25);
        assert!((back / t_eq - 1.0).abs() < 1e-14, "{t_eq} K");
    }
}

/// The optical depth goes as pⁿ and its inverse gives the pressure back; τ is 0 at and above p = 0.
#[test]
fn the_optical_depth_inverts() {
    let g = MetresPerSecondSquared::new(24.79);
    for n in [1.0, 2.0] {
        let grey = GreyProfile::new(3e-3, n, 0.4, MEAN_REDISTRIBUTION).unwrap();
        for p in [1.0, 1e3, 1e5, 2.2e6, 1e8] {
            let tau = grey.optical_depth(Pascals::new(p), g);
            let back = grey.pressure_at_depth(tau, g).value();
            assert!((back / p - 1.0).abs() < 1e-12, "n {n}, {p} Pa");
        }
        let at_reference = grey.optical_depth(REFERENCE_PRESSURE, g);
        assert!((at_reference / (3e-3 * 1e5 / (n * 24.79)) - 1.0).abs() < 1e-14);
        assert_same_bits(grey.optical_depth(Pascals::new(0.0), g), 0.0);
        assert_same_bits(grey.optical_depth(Pascals::new(-1.0), g), 0.0);
    }
}

/// With a constant opacity the grey profile's gradient tends to ¼ at depth, below hydrogen's
/// adiabat (R ÷ `c_p` 0.28–0.33), so a Jupiter's never joins it; an opacity rising as p (n = 2)
/// tends to ½ and joins near a bar (the lane's finding, 2026-10-23).
#[test]
fn a_constant_opacity_never_joins_a_hydrogen_adiabat() {
    let gases = jupiter_gases();
    for kappa in [1e-3, 1e-2, 3e-2] {
        for gamma in [0.1, 0.4, 1.0] {
            let constant = GreyProfile::new(kappa, 1.0, gamma, MEAN_REDISTRIBUTION).unwrap();
            assert_eq!(
                profile(&gases, 99.0, 102.0, 24.79, constant),
                Err(DeriveEnvelopeProfileError::NoJoin),
                "κ {kappa}, γ {gamma}"
            );
        }
    }
    let rising = jupiter();
    let join = rising.join_pressure().value();
    assert!((1e4..1e6).contains(&join), "the join at {join} Pa");
}

/// The join is where the radiative gradient first meets the adiabat's: equal there to 10⁻⁹, and
/// below it at every level above.
#[test]
fn the_join_is_where_the_gradient_first_meets_the_adiabat() {
    for (name, envelope) in cases() {
        let grey = *envelope.grey();
        let g = envelope.gravity();
        let (internal, irradiation) = (
            envelope.internal_temperature(),
            envelope.irradiation_temperature(),
        );
        let gases = match name {
            "Jupiter" => jupiter_gases(),
            _ => protosolar_gases(),
        };
        let gap = |p: f64| {
            let tau = grey.optical_depth(Pascals::new(p), g);
            let t = grey.temperature(tau, internal, irradiation);
            grey.radiative_gradient(tau, internal, irradiation) - 1.0 / heat_capacity(&gases, t)
        };
        let join = envelope.join_pressure().value();
        assert!(
            gap(join).abs() < 1e-9,
            "{name}: the gap at the join {}",
            gap(join)
        );
        let top = grey
            .pressure_at_depth(JOIN_SCAN_TOP_OPTICAL_DEPTH, g)
            .value();
        let mut p = top;
        while p < join * 0.999 {
            assert!(gap(p) < 0.0, "{name}: convective above the join at {p} Pa");
            p *= 1.01;
        }
    }
}

/// Every node lies on the dry adiabat through the join to 10⁻¹² in S̃, the pressures ascend
/// evenly in ln p at most 0.3 apart, from the join to 100 bar.
#[test]
fn every_node_lies_on_the_adiabat_through_the_join() {
    for (name, envelope) in cases() {
        let gases = match name {
            "Jupiter" => jupiter_gases(),
            _ => protosolar_gases(),
        };
        let nodes = envelope.nodes();
        let join = nodes[0];
        assert_same_bits(join.pressure().value(), envelope.join_pressure().value());
        let s0 = entropy(&gases, join.temperature());
        for pair in nodes.windows(2) {
            let step = math::ln(pair[1].pressure().value() / pair[0].pressure().value());
            assert!(
                step > 0.0 && step <= NODE_SPACING_LN_P + 1e-12,
                "{name}: step {step}"
            );
        }
        for node in &nodes[1..] {
            let target = s0 + math::ln(node.pressure().value() / join.pressure().value());
            let residual = entropy(&gases, node.temperature()) - target;
            assert!(
                residual.abs() < 1e-12,
                "{name}: {residual} at {:?}",
                node.pressure()
            );
        }
        let last = nodes[nodes.len() - 1].pressure().value();
        let expected = ADIABAT_BOTTOM
            .value()
            .max(join.pressure().value() * math::exp(NODE_SPACING_LN_P));
        assert_same_bits(last, expected);
        assert!(
            nodes.len() > usize::try_from(MIN_SEGMENTS).unwrap(),
            "{name}"
        );
    }
}

/// `temperature_at` is Guillot's above the join and the nodes' below it: the join's temperature
/// from either side, each node's own at its pressure, the last segment's power law below the last
/// node, the skin at and above p = 0, and NaN for NaN; it never falls with depth.
#[test]
fn the_temperature_is_continuous_and_rises_with_depth() {
    for (name, envelope) in cases() {
        let join = envelope.join_pressure().value();
        let at = |p: f64| envelope.temperature_at(Pascals::new(p)).value();
        let above = at(join * (1.0 - 1e-12));
        assert!(
            (above / at(join) - 1.0).abs() < 1e-11,
            "{name}: {above} against {}",
            at(join)
        );
        for node in envelope.nodes() {
            assert_same_bits(at(node.pressure().value()), node.temperature().value());
        }
        let nodes = envelope.nodes();
        let (a, b) = (nodes[nodes.len() - 2], nodes[nodes.len() - 1]);
        let exponent = math::ln(b.temperature().value() / a.temperature().value())
            / math::ln(b.pressure().value() / a.pressure().value());
        let deep = b.pressure().value() * 100.0;
        let expected = b.temperature().value() * math::powf(100.0, exponent);
        assert!((at(deep) / expected - 1.0).abs() < 1e-12, "{name}");
        assert_same_bits(at(0.0), envelope.skin_temperature().value());
        assert_same_bits(at(-5.0), envelope.skin_temperature().value());
        assert!(at(f64::NAN).is_nan());
        let mut previous = 0.0;
        let mut p = 1e-3;
        while p < 1e9 {
            let t = at(p);
            assert!(t >= previous, "{name}: {t} K at {p} Pa below {previous} K");
            previous = t;
            p *= 1.05;
        }
    }
}

/// The Galileo probe's 427.7 K at 22 bar carried up Jupiter's adiabat on normal hydrogen reaches
/// 166.5 K at 1 bar, within a kelvin of the measured 166.1 K (Seiff et al. 1998;
/// science-r08-gases-2.md §1.4: 166.5 K, 167.0 K on equilibrium hydrogen, 170.7 K at constant
/// classes).
#[test]
fn the_galileo_adiabat_reaches_its_one_bar_temperature() {
    let one_bar = dry_adiabat_temperature(
        &jupiter_gases(),
        Pascals::new(22e5),
        Kelvin::new(427.7),
        Pascals::new(1e5),
    )
    .unwrap()
    .value();
    assert!((one_bar - 166.1).abs() < 1.0, "{one_bar} K");
    assert!(
        (one_bar - 166.5).abs() < 0.3,
        "{one_bar} K against the note's 166.5 K"
    );
}

/// Each envelope a sub-Neptune's, a hot Jupiter's or an ice giant's derives, and its photosphere
/// lies above its join for the cold ones.
#[test]
fn every_kind_of_envelope_has_a_profile() {
    for (name, envelope) in cases() {
        assert!(envelope.skin_temperature().value() > 0.0, "{name}");
        assert!(envelope.photosphere_pressure().value() > 0.0, "{name}");
        assert!(
            envelope.join_pressure().value() > envelope.photosphere_pressure().value(),
            "{name}"
        );
    }
}

/// What is not an envelope is refused: a mixture that does not sum to 1, a row without gas
/// columns, a negative or non-finite temperature, no heat at all, and a gravity that is not
/// positive; and the parameters out of their ranges.
#[test]
fn what_is_not_an_envelope_is_refused() {
    let gases = protosolar_gases();
    let half = [(row("H2"), 0.5)];
    assert_eq!(
        profile(&half, 99.0, 102.0, 24.79, grey()),
        Err(DeriveEnvelopeProfileError::Mixture)
    );
    assert_eq!(
        profile(&[], 99.0, 102.0, 24.79, grey()),
        Err(DeriveEnvelopeProfileError::Mixture)
    );
    let iron = [(row("Mg2SiO4"), 1.0)];
    if row("Mg2SiO4").substance().gas().is_none() {
        assert_eq!(
            profile(&iron, 99.0, 102.0, 24.79, grey()),
            Err(DeriveEnvelopeProfileError::NoHeatCapacity(row("Mg2SiO4")))
        );
    }
    assert!(matches!(
        profile(&gases, -1.0, 102.0, 24.79, grey()),
        Err(DeriveEnvelopeProfileError::InternalTemperature(_))
    ));
    assert!(matches!(
        profile(&gases, 99.0, f64::NAN, 24.79, grey()),
        Err(DeriveEnvelopeProfileError::IrradiationTemperature(_))
    ));
    assert_eq!(
        profile(&gases, 0.0, 0.0, 24.79, grey()),
        Err(DeriveEnvelopeProfileError::Unheated)
    );
    assert!(matches!(
        profile(&gases, 99.0, 102.0, 0.0, grey()),
        Err(DeriveEnvelopeProfileError::Gravity(_))
    ));
    assert!(matches!(
        GreyProfile::new(0.0, 1.0, 0.4, 0.25),
        Err(BuildGreyProfileError::ThermalOpacity(_))
    ));
    assert!(matches!(
        GreyProfile::new(1e-3, -1.0, 0.4, 0.25),
        Err(BuildGreyProfileError::OpticalDepthExponent(_))
    ));
    assert!(matches!(
        GreyProfile::new(1e-3, 1.0, 0.0, 0.25),
        Err(BuildGreyProfileError::OpacityRatio(_))
    ));
    assert!(matches!(
        GreyProfile::new(1e-3, 1.0, 0.4, 1.5),
        Err(BuildGreyProfileError::Redistribution(_))
    ));
}

/// The profile is continuous in its inputs: a part in 10⁶ of `T_eq`, `T_int` or g moves the
/// temperature at every level by under 10⁻⁵.
#[test]
fn the_profile_is_continuous_in_its_inputs() {
    let gases = jupiter_gases();
    let base = jupiter();
    let nudged = [
        profile(&gases, 99.0 * (1.0 + 1e-6), 102.0, 24.79, grey()).unwrap(),
        profile(&gases, 99.0, 102.0 * (1.0 + 1e-6), 24.79, grey()).unwrap(),
        profile(&gases, 99.0, 102.0, 24.79 * (1.0 + 1e-6), grey()).unwrap(),
    ];
    for other in &nudged {
        for p in [1.0, 1e3, 3e4, 1e5, 1e6, 1e7, 1e8] {
            let (a, b) = (
                base.temperature_at(Pascals::new(p)).value(),
                other.temperature_at(Pascals::new(p)).value(),
            );
            assert!((a / b - 1.0).abs() < 1e-5, "{a} against {b} at {p} Pa");
        }
    }
}

/// The same inputs give the same bits twice.
#[test]
fn the_profile_is_the_same_bits_twice() {
    let (a, b) = (jupiter(), jupiter());
    assert_same_bits(a.join_pressure().value(), b.join_pressure().value());
    assert_eq!(a.nodes().len(), b.nodes().len());
    for (x, y) in a.nodes().iter().zip(b.nodes()) {
        assert_same_bits(x.pressure().value(), y.pressure().value());
        assert_same_bits(x.temperature().value(), y.temperature().value());
    }
}

/// The radiative gradient stays below n ÷ 4 at every depth, for every γ, `T_int` and `T_irr`
/// (decision-p14-t24d-profile §1.1), and tends to it at depth where `T_int` is positive.
#[test]
fn the_radiative_gradient_stays_below_n_over_four() {
    for n in [1.0, 2.0] {
        for gamma in [0.1, 0.4, 1.0, 3.0] {
            let grey = GreyProfile::new(3e-3, n, gamma, MEAN_REDISTRIBUTION).unwrap();
            for (t_int, t_eq) in [
                (99.0, 102.0),
                (0.0, 1_450.0),
                (300.0, 1_450.0),
                (52.0, 46.6),
            ] {
                let (internal, irradiation) = (
                    Kelvin::new(t_int),
                    irradiation_temperature(Kelvin::new(t_eq)),
                );
                let mut tau = 1e-8;
                while tau < 1e12 {
                    let gradient = grey.radiative_gradient(tau, internal, irradiation);
                    assert!(
                        gradient < n / 4.0,
                        "n {n}, γ {gamma}, T_int {t_int}, T_eq {t_eq}: {gradient} at τ {tau}"
                    );
                    tau *= 1.3;
                }
                if t_int > 0.0 {
                    let deep = grey.radiative_gradient(1e18, internal, irradiation);
                    assert!(
                        (deep / (n / 4.0) - 1.0).abs() < 1e-6,
                        "n {n}, γ {gamma}: {deep}"
                    );
                }
            }
        }
    }
}

/// At n = 2 and γ 0.4 the join forms at every positive internal temperature, inside its bracket
/// `τ_max` = 4 (⅔ + `c₀` (`T_eq` ÷ `T_int`)⁴), to rounding where the gas's `c_p` ÷ R is 5/2 (a 1 K
/// unlit envelope's): over `T_int` 1–1,000 K and `T_eq` 0–3,000 K, for the protosolar gas; with no
/// internal heat there is none.
#[test]
fn the_join_forms_inside_its_bracket_at_every_internal_temperature() {
    let gases = protosolar_gases();
    let g = MetresPerSecondSquared::new(10.0);
    for t_int in [1.0, 10.0, 50.0, 100.0, 400.0, 1_000.0] {
        for t_eq in [0.0, 50.0, 300.0, 1_000.0, 2_000.0, 3_000.0] {
            let envelope = profile(&gases, t_int, t_eq, 10.0, GreyProfile::ADOPTED)
                .unwrap_or_else(|e| panic!("T_int {t_int}, T_eq {t_eq}: {e}"));
            let grey = GreyProfile::ADOPTED;
            let tau = grey.optical_depth(envelope.join_pressure(), g);
            let bracket = grey.join_bracket_optical_depth(
                Kelvin::new(t_int),
                irradiation_temperature(Kelvin::new(t_eq)),
            );
            assert!(
                tau > 0.0 && tau <= bracket * (1.0 + 1e-12),
                "T_int {t_int}, T_eq {t_eq}: τ {tau} against {bracket}"
            );
        }
    }
    assert_eq!(
        profile(&gases, 0.0, 1_000.0, 10.0, GreyProfile::ADOPTED),
        Err(DeriveEnvelopeProfileError::NoJoin)
    );
}
