use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::chem::equilibrium::Network;

/// An upper atmosphere at equilibrium temperature `t_eq` K, its stratosphere 2^(−1/4) of it (the
/// grey skin), metallicity `metallicity`, g = 10 m s⁻² and μ = 2.3 g mol⁻¹.
fn air(t_eq: f64, metallicity: f64) -> UpperAtmosphere {
    UpperAtmosphere {
        equilibrium_temperature: Kelvin::new(t_eq),
        stratosphere_temperature: Kelvin::new(t_eq * math::powf(2.0, -0.25)),
        metallicity,
        gravity: MetresPerSecondSquared::new(10.0),
        molar_mass_g_per_mol: 2.3,
    }
}

/// The gas's sulphur at `metallicity` times protosolar as a mole fraction: S per H over the
/// molecules per H of an H₂–He gas, ½ + He ÷ H.
fn sulphur(metallicity: f64) -> f64 {
    metallicity * Element::S.protosolar_abundance_per_hydrogen()
        / (0.5 + Element::He.protosolar_abundance_per_hydrogen())
}

/// The SO₂ layer's mole fraction, or zero.
fn dioxide(t_eq: f64, metallicity: f64, sulphur: f64, water: f64) -> f64 {
    sulphur_dioxide(&air(t_eq, metallicity), sulphur, water)
        .expect("in range")
        .map_or(0.0, |layer| layer.mole_fraction())
}

/// A haze's column mass, or zero.
fn column(haze: Option<Haze>) -> f64 {
    match haze.map(|h| h.amount()) {
        Some(HazeAmount::ColumnMass(m)) => m.value(),
        Some(HazeAmount::OpticalDepth { .. }) => panic!("a column mass"),
        None => 0.0,
    }
}

/// Photochemical SO₂ is Tsai et al.'s (2023): at 10 × solar each node of their Extended Data
/// Fig. 10b, at 5 and 20 × solar each scaled by their Fig. 4a, none at or below 900 K or at or above
/// 2,500 K, and never more than the gas's sulphur or half its water; its layer is 10 to 0.01 mbar.
#[test]
fn sulphur_dioxide_follows_tsai_et_al() {
    let plenty = 1e-2;
    for (t, want) in SULPHUR_DIOXIDE_BY_TEMPERATURE_K {
        let got = dioxide(t, 10.0, plenty, plenty);
        assert!((got / want - 1.0).abs() < 1e-12, "{t} K: {got}");
    }
    for (m, factor) in SULPHUR_DIOXIDE_BY_METALLICITY {
        let got = dioxide(1_200.0, m, plenty, plenty);
        assert!((got / (5.0e-6 * factor) - 1.0).abs() < 1e-12, "{m}×: {got}");
    }
    assert!(dioxide(1_200.0, 1.0, plenty, plenty) < 1e-10);
    assert!(
        (dioxide(1_200.0, 100.0, plenty, plenty) / dioxide(1_200.0, 20.0, plenty, plenty) - 1.0)
            .abs()
            < 1e-12
    );
    for t in [600.0, 900.0, 2_500.0, 3_000.0] {
        assert_eq!(
            sulphur_dioxide(&air(t, 10.0), plenty, plenty),
            Ok(None),
            "{t} K"
        );
    }
    assert_same_bits(dioxide(1_200.0, 20.0, 1e-5, plenty), 1e-5);
    assert_same_bits(dioxide(1_200.0, 20.0, plenty, 2e-5), 1e-5);
    let layer = sulphur_dioxide(&air(1_200.0, 10.0), sulphur(10.0), plenty)
        .expect("in range")
        .expect("a layer");
    assert_same_bits(layer.base().value(), 1.0e3);
    assert_same_bits(layer.top().value(), 1.0);
}

/// The S₈ haze forms only at `T_eq` 250–700 K (the plan's test; Gao et al. 2017): with a
/// stratosphere cold enough at every `T_eq` (260 K, Gao et al.'s Gamma Cephei Ab's), none at or
/// outside the range, one at every kelvin inside its tapers.
#[test]
fn the_sulphur_haze_forms_only_at_250_to_700_k() {
    let s = sulphur(1.0);
    for t_eq in 150..=850 {
        let air = UpperAtmosphere {
            stratosphere_temperature: Kelvin::new(260.0),
            ..air(f64::from(t_eq), 1.0)
        };
        let haze = sulphur_haze(&air, s).expect("in range");
        let inside = t_eq > 250 && t_eq < 700;
        assert_eq!(haze.is_some(), inside, "{t_eq} K");
        if let Some(haze) = haze {
            assert_eq!(haze.rule(), HazeRule::Sulphur);
            assert_eq!(haze.material(), by_key("S8").expect("a row"));
            assert_eq!(haze.phase(), CondensedPhase::Solid);
        }
    }
}

/// The S₈ haze needs a cold stratosphere (Gao et al. 2017, §2: "stratospheric temperatures <325K
/// ... assuming that their metallicity is solar ... if the equilibrium S8 mixing ratio is 10 ppm
/// rather than 1 ppm, then sulfur hazes can form up to ∼350 K"): at solar metallicity a haze at
/// 320 K and none at 330 K, and at ten times solar one at 345 K and none at 355 K.
#[test]
fn the_sulphur_haze_needs_a_cold_stratosphere() {
    let haze = |metallicity: f64, stratosphere: f64| {
        let air = UpperAtmosphere {
            stratosphere_temperature: Kelvin::new(stratosphere),
            ..air(450.0, metallicity)
        };
        sulphur_haze(&air, sulphur(metallicity)).expect("in range")
    };
    assert!(haze(1.0, 320.0).is_some());
    assert_eq!(haze(1.0, 330.0), None);
    assert!(haze(10.0, 345.0).is_some());
    assert_eq!(haze(10.0, 355.0), None);
}

/// The S₈ haze's column is Gao et al.'s (2017, §2) estimate: "1 ppmv of S8 at the 100 mbar level
/// with T ∼ 250 K ... a column height equaling one scale height (∼20 km at 100 mbar for this
/// atmosphere), then the column integrated number density of S8 is ∼6 × 10^18 cm−2", within 10%
/// with the gravity their 20 km scale height implies (k T ÷ (μ `m_u` H), μ = 2.3), the vapour all
/// condensed; its particles their log-normal, `r_g` 0.1 µm and σ = 2.
#[test]
fn the_sulphur_haze_s_column_is_gao_et_al_s_estimate() {
    let boltzmann = crate::units::consts::BOLTZMANN_CONSTANT;
    let amu = 1.660_539_066_60e-27;
    let gravity = boltzmann * 250.0 / (2.3 * amu * 20.0e3);
    let air = UpperAtmosphere {
        equilibrium_temperature: Kelvin::new(450.0),
        stratosphere_temperature: Kelvin::new(250.0),
        metallicity: 1.0,
        gravity: MetresPerSecondSquared::new(gravity),
        molar_mass_g_per_mol: 2.3,
    };
    let haze = sulphur_haze(&air, sulphur(1.0))
        .expect("in range")
        .expect("a haze");
    let molecules_per_cm2 = column(Some(haze.clone())) / (256.48e-3 / 6.022_140_76e23) * 1e-4;
    assert!(
        (molecules_per_cm2 / 6e18 - 1.0).abs() < 0.1,
        "{molecules_per_cm2}"
    );
    let size = haze.size();
    assert_eq!(
        size.kind(),
        crate::substance::SizeDistributionKind::LogNormal
    );
    let variance = math::exp(math::ln(2.0) * math::ln(2.0)) - 1.0;
    assert!((size.effective_variance() - variance).abs() < 1e-15);
    let r_g = size.effective_radius().value() * math::powf(1.0 + variance, -2.5);
    assert!((r_g / 1e-7 - 1.0).abs() < 1e-12, "{r_g}");
}

/// S₈'s vapour pressure is Zahnle et al.'s two branches, continuous where they cross at 412.5 K.
#[test]
fn the_sulphur_vapour_pressure_is_continuous() {
    let p = |t: f64| sulphur_saturation_pressure(Kelvin::new(t)).value() / PASCALS_PER_BAR;
    assert!((p(300.0) / math::exp(20.0 - 11_800.0 / 300.0) - 1.0).abs() < 1e-12);
    assert!((p(500.0) / math::exp(9.6 - 7_510.0 / 500.0) - 1.0).abs() < 1e-12);
    let crossing = 4_290.0 / 10.4;
    assert!((p(crossing - 1e-9) / p(crossing + 1e-9) - 1.0).abs() < 1e-9);
}

/// Soot only below 950 K (the plan's test; Gao et al. 2020): none at or above it, one at every
/// kelvin below, and its share of the haze opacity 1 up to 850 K, falling through about 0.6 at
/// 890 K (Gao et al.'s Fig. 2a, read: about 0.55) to none.
#[test]
fn soot_forms_only_below_950_k() {
    for t_eq in 500..=1_300 {
        let haze = soot(&air(f64::from(t_eq), 1.0), 4e-4).expect("in range");
        assert_eq!(haze.is_some(), t_eq < 950, "{t_eq} K");
        if let Some(haze) = haze {
            assert_eq!(haze.rule(), HazeRule::Soot);
            assert_eq!(haze.material(), by_key("soot").expect("a row"));
        }
    }
    assert_same_bits(hydrocarbon_share(Kelvin::new(850.0)), 1.0);
    assert_same_bits(hydrocarbon_share(Kelvin::new(950.0)), 0.0);
    assert!((hydrocarbon_share(Kelvin::new(890.0)) - 0.6).abs() < 1e-12);
    assert_eq!(soot(&air(800.0, 1.0), 0.0), Ok(None));
}

/// The soot's column is the methane's carbon over a scale height at 1 µbar: `f_CH4` (M(C) ÷ μ) p ÷
/// g, about 2 × 10⁻⁵ kg m⁻² for 4 × 10⁻⁴ of methane at g = 10 m s⁻², in a slab from 1 µbar down a
/// scale height, of 50 nm particles.
#[test]
fn the_soot_s_column_is_the_methane_s_carbon() {
    let haze = soot(&air(700.0, 1.0), 4e-4)
        .expect("in range")
        .expect("a haze");
    let want = 4e-4 * 12.011 / 2.3 * 0.1 / 10.0;
    assert!((column(Some(haze.clone())) / want - 1.0).abs() < 1e-12);
    match haze.profile() {
        HazeProfile::Layer {
            base,
            top,
            exponent,
        } => {
            assert_same_bits(top.value(), 0.1);
            assert!((base.value() / top.value() - core::f64::consts::E).abs() < 1e-12);
            assert_same_bits(exponent, 0.0);
        }
        HazeProfile::Gaussian { .. } => panic!("a layer"),
    }
    assert!((haze.size().effective_radius().value() - 5e-8).abs() < 1e-20);
}

/// The ice giants' haze lies above the methane layer, Irwin et al.'s (2022) Aerosol-3: its base the
/// layer's, its mixing ratio rising with height as (p ÷ `p_base`)^−0.5, to 1 mbar, of optical depth
/// 0.035 at 0.8 µm and 0.05 µm particles.
#[test]
fn the_ice_giant_haze_lies_above_the_methane_layer() {
    for bar in [1.2, 1.6, 2.1] {
        let layer = Pascals::new(bar * PASCALS_PER_BAR);
        let haze = ice_giant_haze(layer).expect("in range").expect("a haze");
        assert_eq!(haze.rule(), HazeRule::IceGiant);
        assert_eq!(haze.material(), by_key("ice_giant_haze").expect("a row"));
        match haze.profile() {
            HazeProfile::Layer {
                base,
                top,
                exponent,
            } => {
                assert_same_bits(base.value(), layer.value());
                assert_same_bits(top.value(), 100.0);
                assert_same_bits(exponent, -0.5);
            }
            HazeProfile::Gaussian { .. } => panic!("a layer"),
        }
        assert_eq!(
            haze.amount(),
            HazeAmount::OpticalDepth {
                optical_depth: 0.035,
                wavelength: Micrometres::new(0.8)
            }
        );
        assert!((haze.size().effective_radius().value() - 5e-8).abs() < 1e-20);
    }
    assert_eq!(ice_giant_haze(Pascals::new(50.0)), Ok(None));
    assert_eq!(ice_giant_haze(Pascals::new(100.0)), Ok(None));
}

/// The cold giants' chromophore no deeper than about 0.2 bar (Braude et al. 2020): its peak at
/// 0.2 bar over every ammonia layer deeper, and at the layer itself over a higher one, Braude et
/// al.'s width of 0.25 in ln p, its column 6.3 × 10⁻⁴ kg m⁻².
#[test]
fn the_chromophore_lies_no_deeper_than_0_2_bar() {
    for bar in [0.05, 0.15, 0.2, 0.5, 0.7, 2.0] {
        let layer = Pascals::new(bar * PASCALS_PER_BAR);
        let haze = chromophore(layer).expect("a haze");
        assert_eq!(haze.material(), by_key("chromophore").expect("a row"));
        match haze.profile() {
            HazeProfile::Gaussian { peak, width } => {
                let want = layer.value().min(2.0e4);
                assert_same_bits(peak.value(), want);
                assert_same_bits(width, 0.25);
            }
            HazeProfile::Layer { .. } => panic!("a Gaussian"),
        }
        assert_eq!(haze.amount(), HazeAmount::ColumnMass(CHROMOPHORE_COLUMN));
    }
}

/// A value as printed, and half a unit of its last printed digit: "2.8e-3" is 2.8 × 10⁻³ ± 5 ×
/// 10⁻⁵.
fn printed(text: &str) -> (f64, f64) {
    let value: f64 = text.parse().expect("a number");
    let (mantissa, exponent) = text.split_once('e').unwrap_or((text, "0"));
    let decimals = mantissa.split_once('.').map_or(0, |(_, d)| d.len());
    let exponent: i32 = exponent.parse().expect("an exponent");
    let decimals = i32::try_from(decimals).expect("a few digits");
    (value, math::powi(10.0, exponent - decimals) / 2.0)
}

/// Parmentier et al.'s (2018) fit is transcribed as an independent evaluation of their Table 1
/// and eq. 1 (the lane's, in Python, from arXiv:1805.00096v2) gives it, each value within half a
/// unit of its last printed digit: the share left at 2,000, 2,500, 3,000, 3,500 and 4,000 K, of H₂O at 1 mbar,
/// 0.1 bar and 1 bar, and of TiO and VO at 1 mbar.
#[test]
fn parmentier_s_fit_is_transcribed() {
    let cases: [(Dissociating, f64, [&str; 5]); 5] = [
        (
            Dissociating::Water,
            1e-3,
            ["0.997", "0.485", "2.8e-3", "1.6e-5", "3e-7"],
        ),
        (
            Dissociating::Water,
            0.1,
            ["1.000", "0.991", "0.721", "0.081", "2.7e-3"],
        ),
        (
            Dissociating::Water,
            1.0,
            ["1.000", "0.999", "0.965", "0.639", "0.125"],
        ),
        (
            Dissociating::TitaniumOxide,
            1e-3,
            ["1.000", "0.943", "0.069", "1.8e-4", "1.4e-6"],
        ),
        (
            Dissociating::VanadiumOxide,
            1e-3,
            ["1.000", "0.896", "0.048", "2.1e-4", "2.5e-6"],
        ),
    ];
    for (molecule, bar, want) in cases {
        for (t, text) in [2_000.0, 2_500.0, 3_000.0, 3_500.0, 4_000.0]
            .iter()
            .zip(want)
        {
            let (want, half) = printed(text);
            let got = molecule
                .remaining(Kelvin::new(*t), Pascals::new(bar * PASCALS_PER_BAR))
                .expect("in range");
            assert!(
                (got - want).abs() <= half * (1.0 + 1e-9),
                "{} at {t} K, {bar} bar: {got} against {text}",
                molecule.key()
            );
        }
    }
}

/// P14.T52.a's network dissociates H₂O, TiO and VO as Parmentier et al.'s fit does, each within its
/// stated tolerance: over 2,500–4,000 K and 1 mbar to 1 bar, where the fit leaves between 10⁻³ and
/// 0.9 of the molecule, the network's mole fraction over its own at 2,000 K and 10 bar (Parmentier
/// et al. "fix the volume mixing ratio of water to its abundance at 10 bar") against the fit.
///
/// - **H₂O within 0.3 dex**, the fit's own scatter about their CEA map (their Fig. 12, read cell by
///   cell by the lane: 133 of 141 cells within 0.3 dex). As built, at nine points, within 0.15 dex:
///   the farthest +0.14 dex at 3,000 K and 10 mbar, where OH, not carried (P14.T52.e), would hold
///   some of the oxygen.
/// - **TiO within 0.5 dex**: the network dissociates it more, by 0.14–0.45 dex at eight points (the
///   farthest −0.45 dex at 3,500 K and 1 bar), its thermochemistry against CEA's in Parmentier et
///   al.'s grid, with TiO₂ not carried; a recorded figure.
/// - **VO within 1.1 dex**: more again, by 0.50–1.02 dex at nine points (the farthest −1.02 dex at
///   3,500 K and 1 bar), the network's CEA record of VO (Gurvich 1982) being about 19 kJ mol⁻¹ less
///   bound than FastChem's JANAF fit (P14.T52.a's recorded finding); a recorded figure.
#[test]
fn the_network_dissociates_as_parmentier_s_fit_does() {
    let network = Network::registry().expect("the registry's network builds");
    let gas: Vec<(Element, f64)> = Element::ALL
        .iter()
        .map(|&e| (e, e.protosolar_abundance_per_hydrogen()))
        .collect();
    for (molecule, tolerance) in [
        (Dissociating::Water, 0.3),
        (Dissociating::TitaniumOxide, 0.5),
        (Dissociating::VanadiumOxide, 1.1),
    ] {
        let id = by_key(molecule.key()).expect("a row");
        let solve = |t: f64, p: f64| {
            network
                .solve(gas.iter().copied(), Kelvin::new(t), Pascals::new(p))
                .expect("solved")
                .mole_fraction(id)
                .expect("carried")
        };
        let deep = solve(2_000.0, 10.0 * PASCALS_PER_BAR);
        let mut compared = 0;
        for t in [2_500.0, 3_000.0, 3_500.0, 4_000.0] {
            for bar in [1e-3, 1e-2, 0.1, 1.0] {
                let p = bar * PASCALS_PER_BAR;
                let fit = molecule
                    .remaining(Kelvin::new(t), Pascals::new(p))
                    .expect("in range");
                if !(1e-3..=0.9).contains(&fit) {
                    continue;
                }
                let network_share = solve(t, p) / deep;
                let dex = math::log10(network_share / fit);
                assert!(
                    dex.abs() < tolerance,
                    "{} at {t} K, {bar} bar: {network_share} against {fit}",
                    molecule.key()
                );
                compared += 1;
            }
        }
        assert!(compared >= 6, "{}: {compared}", molecule.key());
    }
}

/// Each rule is continuous in its inputs: in steps of 0.1 K of `T_eq` and the stratosphere's
/// temperature, of 0.001 in log₁₀ of the metallicity and of 0.001 dex in pressure, no rule's output
/// moves by more than 1% of its largest value over the scan.
#[test]
fn each_rule_is_continuous_in_its_inputs() {
    let scan = |name: &str, f: &dyn Fn(f64) -> f64, from: f64, to: f64, step: f64| {
        let mut values = Vec::new();
        let mut x = from;
        while x <= to {
            values.push(f(x));
            x += step;
        }
        let largest = values.iter().copied().fold(0.0, f64::max);
        assert!(largest > 0.0, "{name}");
        for pair in values.windows(2) {
            assert!(
                (pair[1] - pair[0]).abs() <= 0.01 * largest,
                "{name}: {pair:?}"
            );
        }
    };
    let plenty = 1e-2;
    scan(
        "SO₂ in T_eq",
        &|t| dioxide(t, 10.0, plenty, plenty),
        800.0,
        2_600.0,
        0.1,
    );
    scan(
        "SO₂ in metallicity",
        &|dex| dioxide(1_200.0, math::exp10(dex), plenty, plenty),
        0.0,
        2.0,
        0.001,
    );
    let s8 = |t_eq: f64, stratosphere: f64, metallicity: f64| {
        let air = UpperAtmosphere {
            stratosphere_temperature: Kelvin::new(stratosphere),
            ..air(t_eq, metallicity)
        };
        column(sulphur_haze(&air, sulphur(metallicity)).expect("in range"))
    };
    scan("S₈ in T_eq", &|t| s8(t, 260.0, 1.0), 200.0, 750.0, 0.1);
    scan(
        "S₈ in the stratosphere",
        &|t| s8(450.0, t, 1.0),
        200.0,
        400.0,
        0.1,
    );
    scan(
        "S₈ in metallicity",
        &|dex| s8(450.0, 300.0, math::exp10(dex)),
        -1.0,
        1.0,
        0.001,
    );
    scan(
        "soot in T_eq",
        &|t| column(soot(&air(t, 1.0), 4e-4).expect("in range")),
        700.0,
        1_000.0,
        0.1,
    );
    let peak = |dex: f64| match chromophore(Pascals::new(math::exp10(dex)))
        .expect("a haze")
        .profile()
    {
        HazeProfile::Gaussian { peak, .. } => peak.value(),
        HazeProfile::Layer { .. } => panic!("a Gaussian"),
    };
    scan("the chromophore's peak", &peak, 3.0, 5.5, 0.001);
    let base = |dex: f64| match ice_giant_haze(Pascals::new(math::exp10(dex)))
        .expect("in range")
        .expect("a haze")
        .profile()
    {
        HazeProfile::Layer { base, .. } => base.value(),
        HazeProfile::Gaussian { .. } => panic!("a layer"),
    };
    scan("the ice giants' haze's base", &base, 4.0, 5.5, 0.001);
    for molecule in Dissociating::ALL {
        let at = |t: f64| {
            molecule
                .remaining(Kelvin::new(t), Pascals::new(100.0))
                .expect("in range")
        };
        scan(molecule.key(), &at, 1_500.0, 5_000.0, 0.1);
        let at = |dex: f64| {
            molecule
                .remaining(Kelvin::new(3_000.0), Pascals::new(math::exp10(dex)))
                .expect("in range")
        };
        scan(molecule.key(), &at, -1.0, 7.0, 0.001);
    }
}

/// Each refusal names its cause.
#[test]
fn each_refusal_names_its_cause() {
    let good = air(1_200.0, 10.0);
    assert_eq!(
        sulphur_dioxide(
            &UpperAtmosphere {
                equilibrium_temperature: Kelvin::new(f64::NAN),
                ..good
            },
            1e-4,
            1e-3
        )
        .map_err(|e| e.to_string()),
        Err("temperature NaN K is not finite and positive".to_owned())
    );
    assert_eq!(
        soot(
            &UpperAtmosphere {
                metallicity: 0.0,
                ..good
            },
            1e-4
        ),
        Err(EvaluateRuleError::Metallicity(0.0))
    );
    assert_eq!(
        sulphur_haze(
            &UpperAtmosphere {
                gravity: MetresPerSecondSquared::new(-1.0),
                ..good
            },
            1e-4
        ),
        Err(EvaluateRuleError::Gravity(MetresPerSecondSquared::new(
            -1.0
        )))
    );
    assert_eq!(
        soot(
            &UpperAtmosphere {
                molar_mass_g_per_mol: f64::INFINITY,
                ..good
            },
            1e-4
        ),
        Err(EvaluateRuleError::MolarMass {
            molar_mass_g_per_mol: f64::INFINITY
        })
    );
    assert_eq!(
        sulphur_dioxide(&good, 1.5, 1e-3),
        Err(EvaluateRuleError::MoleFraction(1.5))
    );
    assert_eq!(
        chromophore(Pascals::new(0.0)),
        Err(EvaluateRuleError::Pressure(Pascals::new(0.0)))
    );
    assert_eq!(
        ice_giant_haze(Pascals::new(f64::NAN)).map_err(|e| e.to_string()),
        Err("pressure NaN Pa is not finite and positive".to_owned())
    );
    assert_eq!(
        Dissociating::Water.remaining(Kelvin::new(-5.0), Pascals::new(1.0)),
        Err(EvaluateRuleError::Temperature(Kelvin::new(-5.0)))
    );
}

/// The rules pinned bit for bit at a few inputs each. Nothing on the record calls them before
/// P14.T54.b, so this golden is re-blessed with the rules it pins, at the version in force (22,
/// then 23), until then, with no bump (the sim-determinism skill's chemistry exception).
#[test]
fn the_rules_are_pinned() {
    use crate::GENERATOR_VERSION;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (t, m) in [
        (950.0, 10.0),
        (1_170.0, 10.0),
        (1_500.0, 3.0),
        (2_200.0, 30.0),
    ] {
        w.f64(
            &format!("SO2 at {t} K, {m}x"),
            dioxide(t, m, sulphur(m), 1e-2),
        );
    }
    for (t, stratosphere, m) in [
        (270.0, 230.0, 1.0),
        (450.0, 300.0, 1.0),
        (600.0, 330.0, 10.0),
    ] {
        let air = UpperAtmosphere {
            stratosphere_temperature: Kelvin::new(stratosphere),
            ..air(t, m)
        };
        let haze = sulphur_haze(&air, sulphur(m)).expect("in range");
        w.f64(
            &format!("S8 at {t} K, {stratosphere} K, {m}x"),
            column(haze.clone()),
        );
        if let Some(Haze {
            profile: HazeProfile::Layer { top, .. },
            ..
        }) = haze
        {
            w.f64(
                &format!("S8 top at {t} K, {stratosphere} K, {m}x"),
                top.value(),
            );
        }
    }
    for t in [600.0, 900.0] {
        w.f64(
            &format!("soot at {t} K"),
            column(soot(&air(t, 1.0), 4e-4).expect("in range")),
        );
    }
    for molecule in Dissociating::ALL {
        for (t, bar) in [(2_500.0, 1e-3), (3_200.0, 0.1), (4_600.0, 0.05)] {
            w.f64(
                &format!("{} left at {t} K, {bar} bar", molecule.key()),
                molecule
                    .remaining(Kelvin::new(t), Pascals::new(bar * PASCALS_PER_BAR))
                    .expect("in range"),
            );
        }
    }
    for t in [300.0, 500.0] {
        w.f64(
            &format!("S8 vapour pressure at {t} K"),
            sulphur_saturation_pressure(Kelvin::new(t)).value(),
        );
    }
    let size = log_normal(SULPHUR_HAZE_PARTICLES.0, SULPHUR_HAZE_PARTICLES.1);
    w.f64("S8 r_eff", size.effective_radius().value());
    w.f64("S8 v_eff", size.effective_variance());
    golden!("planetary/photochem", w.as_str());
}
