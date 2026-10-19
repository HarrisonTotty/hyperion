//! The silicate melts' tests (P14.T49.h's step 4; decision-p14-t49h-step4-melts, "For the lane"
//! 3): Giordano, Russell and Dingwell 2008 and Giordano and Russell 2007 against their own papers
//! and against measured melts, §2.3's rule, Lange and Carmichael's density against a measured melt,
//! the rows' oxides, and the record's range and refusals.

use super::super::super::rows::liquids::LIQUIDS;
use super::super::{Composition, LiquidComposition, LiquidConstitution, liquid_properties};
use super::*;
use crate::substance::{SubstanceId, by_key};
use crate::tables::liquid_optics as bins;

/// The row `key`, which a test knows to be a row.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap()
}

/// Every melt row, with its melt, in registry order.
fn melt_rows() -> Vec<(&'static str, &'static SilicateMelt)> {
    LIQUIDS
        .iter()
        .filter_map(|(key, columns)| match columns.constitution {
            LiquidConstitution::Melt(melt) => Some((key.as_str(), melt)),
            LiquidConstitution::Pure { .. }
            | LiquidConstitution::Solution(_)
            | LiquidConstitution::AwaitingSolute(_) => None,
        })
        .collect()
}

/// The melt row `key`'s melt.
fn melt_of(key: &str) -> &'static SilicateMelt {
    melt_rows()
        .into_iter()
        .find(|&(k, _)| k == key)
        .map(|(_, melt)| melt)
        .unwrap()
}

/// The melt row `key`'s record's log₁₀ μ at `t` K, μ in Pa s.
fn record_log10_viscosity(key: &str, t: f64) -> f64 {
    let record = liquid_properties(row(key), Kelvin::new(t), Composition::Reference).unwrap();
    math::log10(record.viscosity_pa_s.unwrap())
}

/// A mineral's oxides by weight per cent from its oxides' moles per formula unit.
fn mineral(moles_per_unit: &[(Oxide, f64)]) -> Vec<(Oxide, f64)> {
    let mut mass = 0.0;
    for &(oxide, n) in moles_per_unit {
        mass += n * oxide.molar_mass_g_per_mol();
    }
    moles_per_unit
        .iter()
        .map(|&(oxide, n)| (oxide, 100.0 * n * oxide.molar_mass_g_per_mol() / mass))
        .collect()
}

/// Anorthite, `CaAl₂Si₂O₈`, by weight.
fn anorthite() -> Vec<(Oxide, f64)> {
    mineral(&[
        (Oxide::Lime, 1.0),
        (Oxide::Alumina, 1.0),
        (Oxide::Silica, 2.0),
    ])
}

/// Diopside, `CaMgSi₂O₆`, by weight.
fn diopside() -> Vec<(Oxide, f64)> {
    mineral(&[
        (Oxide::Lime, 1.0),
        (Oxide::Magnesia, 1.0),
        (Oxide::Silica, 2.0),
    ])
}

/// The diopside–anorthite join at `diopside_wt_pct` weight per cent diopside, Scarfe et al.'s
/// compositions.
fn join(diopside_wt_pct: f64) -> Vec<(Oxide, f64)> {
    let w = diopside_wt_pct / 100.0;
    let (an, di) = (anorthite(), diopside());
    [Oxide::Silica, Oxide::Alumina, Oxide::Magnesia, Oxide::Lime]
        .into_iter()
        .map(|oxide| {
            (
                oxide,
                w * weight_pct(&di, oxide) + (1.0 - w) * weight_pct(&an, oxide),
            )
        })
        .collect()
}

/// `oxide`'s mole per cent among all of `oxides`.
fn mole_pct(oxides: &[(Oxide, f64)], oxide: Oxide) -> f64 {
    let mut total = 0.0;
    for &(o, _) in oxides {
        total += moles(oxides, o);
    }
    100.0 * moles(oxides, oxide) / total
}

/// Scarfe, Cronin, Wenzel and Kauffman 1983's measured viscosities of the diopside–anorthite join
/// (Am. Mineral. 68, 1083, Table 1, log₁₀ η in poise against T in °C, as decision-p14-t49h-step4-
/// melts typed them from the paper's page images at 200 dpi, the lane's copy SHA-256 d712d829…),
/// by weight per cent diopside: all nine compositions, 75 points.
#[expect(
    clippy::approx_constant,
    reason = "Di20's 3.140 at 1,250 °C is a measured log viscosity, not π"
)]
const SCARFE_TABLE_1: [(f64, &[(f64, f64)]); 9] = [
    (
        0.0,
        &[
            (1625.0, 1.333),
            (1600.0, 1.433),
            (1575.0, 1.513),
            (1550.0, 1.625),
            (1525.0, 1.711),
            (1500.0, 1.841),
            (1475.0, 1.968),
            (1450.0, 2.101),
            (1425.0, 2.246),
            (1400.0, 2.402),
            (1375.0, 2.567),
        ],
    ),
    (
        20.0,
        &[
            (1600.0, 1.252),
            (1550.0, 1.434),
            (1525.0, 1.522),
            (1500.0, 1.625),
            (1475.0, 1.738),
            (1450.0, 1.836),
            (1400.0, 2.098),
            (1350.0, 2.384),
            (1300.0, 2.730),
            (1250.0, 3.140),
        ],
    ),
    (
        30.0,
        &[
            (1600.0, 1.179),
            (1550.0, 1.358),
            (1500.0, 1.531),
            (1450.0, 1.730),
            (1425.0, 1.848),
            (1400.0, 1.980),
            (1350.0, 2.258),
            (1300.0, 2.575),
        ],
    ),
    (
        40.0,
        &[
            (1600.0, 1.029),
            (1550.0, 1.194),
            (1500.0, 1.370),
            (1450.0, 1.553),
            (1400.0, 1.756),
            (1375.0, 1.883),
            (1350.0, 2.015),
        ],
    ),
    (
        50.0,
        &[
            (1600.0, 0.956),
            (1550.0, 1.074),
            (1500.0, 1.240),
            (1450.0, 1.432),
            (1400.0, 1.630),
            (1350.0, 1.854),
            (1325.0, 1.989),
            (1300.0, 2.132),
            (1250.0, 2.457),
            (1200.0, 2.846),
        ],
    ),
    (
        58.0,
        &[
            (1600.0, 0.851),
            (1550.0, 0.990),
            (1500.0, 1.146),
            (1450.0, 1.329),
            (1400.0, 1.518),
            (1350.0, 1.721),
            (1275.0, 2.134),
            (1225.0, 2.468),
            (1175.0, 2.859),
        ],
    ),
    (
        80.0,
        &[
            (1600.0, 0.636),
            (1550.0, 0.759),
            (1500.0, 0.898),
            (1450.0, 1.057),
            (1400.0, 1.237),
            (1350.0, 1.442),
            (1300.0, 1.660),
            (1250.0, 1.909),
        ],
    ),
    (
        90.0,
        &[
            (1550.0, 0.648),
            (1500.0, 0.800),
            (1450.0, 0.957),
            (1400.0, 1.128),
            (1350.0, 1.327),
            (1300.0, 1.539),
        ],
    ),
    (
        100.0,
        &[
            (1600.0, 0.465),
            (1550.0, 0.559),
            (1500.0, 0.678),
            (1450.0, 0.821),
            (1400.0, 0.992),
            (1375.0, 1.086),
        ],
    ),
];

/// Scarfe et al. 1983's Table 3 Fulcher fits in kelvin and Pa s (decision-p14-t49h-step4-melts
/// §1: A − 1, B, T₀ + 273.15 K), by weight per cent diopside: An100 −2.343 + 2123 ÷ (T − 1,106.15
/// K), Di20 −2.615 + 2540 ÷ (T − 988.65 K) and Di30 −2.472 + 2319 ÷ (T − 1,000.65 K).
const SCARFE_TABLE_3: [(f64, Vft); 3] = [
    (
        0.0,
        Vft {
            a: -2.343,
            b_kelvin: 2123.0,
            c_kelvin: 1106.15,
        },
    ),
    (
        20.0,
        Vft {
            a: -2.615,
            b_kelvin: 2540.0,
            c_kelvin: 988.65,
        },
    ),
    (
        30.0,
        Vft {
            a: -2.472,
            b_kelvin: 2319.0,
            c_kelvin: 1000.65,
        },
    ),
];

/// Scarfe's Table 3 fit at `diopside_wt_pct`.
fn scarfe_fit(diopside_wt_pct: f64) -> Vft {
    SCARFE_TABLE_3
        .iter()
        .find(|&&(w, _)| (w - diopside_wt_pct).abs() < 1e-9)
        .map(|&(_, fit)| fit)
        .unwrap()
}

/// Each oxide's molar mass is its elements' weights summed, the registry's arithmetic: `SiO₂`
/// 60.083, `Al₂O₃` 101.961, `FeO` 71.844 and `Na₂O` 61.979 g mol⁻¹.
#[test]
fn each_oxide_s_molar_mass_is_its_elements() {
    for (oxide, expected) in [
        (Oxide::Silica, 60.083),
        (Oxide::Alumina, 101.961),
        (Oxide::Ferrous, 71.844),
        (Oxide::Soda, 61.979),
        (Oxide::Water, 18.015),
    ] {
        let m = oxide.molar_mass_g_per_mol();
        assert!((m - expected).abs() < 1e-9, "{oxide:?}: {m}");
    }
}

/// Giordano, Russell and Dingwell 2008 reproduces its Table 4's Eifel basanite (EIF; their Fig.
/// 9): B 4,457 and C 669 within 1, and log₁₀ μ at 1,473.15 K within 0.01 of 0.993, the value its
/// printed B and C give.
#[test]
fn grd_reproduces_its_table_4_basanite() {
    let basanite = [
        (Oxide::Silica, 41.17),
        (Oxide::Titania, 2.74),
        (Oxide::Alumina, 12.10),
        (Oxide::Ferrous, 10.10),
        (Oxide::Manganese, 0.18),
        (Oxide::Magnesia, 11.24),
        (Oxide::Lime, 15.66),
        (Oxide::Soda, 2.76),
        (Oxide::Potash, 3.04),
        (Oxide::PhosphorusPentoxide, 1.02),
    ];
    let vft = giordano_2008(&basanite);
    assert!((vft.b_kelvin - 4457.0).abs() <= 1.0, "{vft:?}");
    assert!((vft.c_kelvin - 669.0).abs() <= 1.0, "{vft:?}");
    let log = vft.log10_viscosity_pa_s(Kelvin::new(1473.15));
    assert!((log - 0.993).abs() <= 0.01, "{log}");
}

/// Giordano, Russell and Dingwell 2008 reproduces its Table 2's worked example, an iron-poor
/// andesite with 2 wt% water: B 7,720 and C 334 within 1, and log₁₀ μ 3.67 at 1,273 K within 0.01.
#[test]
fn grd_reproduces_its_table_2_andesite() {
    let andesite = [
        (Oxide::Silica, 62.40),
        (Oxide::Titania, 0.55),
        (Oxide::Alumina, 20.01),
        (Oxide::Ferrous, 0.03),
        (Oxide::Manganese, 0.02),
        (Oxide::Magnesia, 3.22),
        (Oxide::Lime, 9.08),
        (Oxide::Soda, 3.52),
        (Oxide::Potash, 0.93),
        (Oxide::PhosphorusPentoxide, 0.12),
        (Oxide::Water, 2.00),
    ];
    let vft = giordano_2008(&andesite);
    assert!((vft.b_kelvin - 7720.0).abs() <= 1.0, "{vft:?}");
    assert!((vft.c_kelvin - 334.0).abs() <= 1.0, "{vft:?}");
    let log = vft.log10_viscosity_pa_s(Kelvin::new(1273.0));
    assert!((log - 3.67).abs() <= 0.01, "{log}");
}

/// The `basalt` row's melt at 1,473.15 K lies within Giordano, Russell and Dingwell 2008's 0.40 log
/// units (its Table 3) of Shaw, Wright, Peck and Okamura 1968's about 500 poise, 50 Pa s, for the
/// Makaopuhi tholeiite's liquid near its liquidus at about 1,200 °C (Am. J. Sci. 266, 225, its
/// abstract): 96 Pa s, 0.28 log units above, for Alae's bulk, a liquid of a like composition.
#[test]
fn basalt_at_1473_k_is_shaw_s_makaopuhi_within_grd_s_error() {
    let miss = record_log10_viscosity("basalt", 1473.15) - math::log10(50.0);
    assert!(miss.abs() <= 0.40, "{miss}");
    assert!((miss - 0.284).abs() < 0.005, "{miss}");
}

/// Giordano and Russell 2007 reproduces its Table A.2's B and C for stoichiometric anorthite ("AN",
/// 5,467 and 797) and diopside ("ANDI", 4,487 and 723), each within 1 at the table's printed
/// precision, the integer: anorthite gives 5,467.4 and 797.5 and diopside 4,488.3 and 723.1 (the
/// table's diopside rows are the experiments' analysed melts, near the stoichiometry), which
/// checks every coefficient but `MgO`'s in C.
#[test]
fn gr07_reproduces_its_table_a2_for_anorthite_and_diopside() {
    for (name, oxides, b, c) in [
        ("anorthite", anorthite(), 5467.0, 797.0),
        ("diopside", diopside(), 4487.0, 723.0),
    ] {
        let vft = giordano_russell_2007(&oxides);
        assert!((vft.b_kelvin.round() - b).abs() <= 1.0, "{name}: {vft:?}");
        assert!((vft.c_kelvin.round() - c).abs() <= 1.0, "{name}: {vft:?}");
        assert!((vft.a - GR07_A).abs() < 1e-12, "{name}");
    }
    let an = giordano_russell_2007(&anorthite());
    assert!((an.b_kelvin - 5467.4).abs() < 1e-9 && (an.c_kelvin - 797.5).abs() < 1e-9);
}

/// Giordano and Russell 2007 meets Scarfe et al. 1983's measured join (Table 1, log₁₀ poise less 1,
/// against T in °C plus 273.15): over all 75 points at 0.18 log units rms, inside its stated
/// standard error of 0.224 (its Table 1), every point within 0.5, and anorthite at 1,873.15 K
/// within 0.1 of the measured 0.433.
#[test]
fn gr07_meets_scarfe_s_table_1() {
    let mut squares = 0.0;
    let mut count = 0.0;
    for (diopside_wt_pct, points) in SCARFE_TABLE_1 {
        let vft = giordano_russell_2007(&join(diopside_wt_pct));
        for &(celsius, log_poise) in points {
            let miss = vft.log10_viscosity_pa_s(Kelvin::new(celsius + 273.15)) - (log_poise - 1.0);
            assert!(
                miss.abs() <= 0.5,
                "Di{diopside_wt_pct} at {celsius} °C: {miss}"
            );
            squares += miss * miss;
            count += 1.0;
        }
    }
    let rms = (squares / count).sqrt();
    assert!((count - 75.0).abs() < 0.5, "{count}");
    assert!(rms <= 0.224, "{rms}");
    let anorthite = giordano_russell_2007(&join(0.0)).log10_viscosity_pa_s(Kelvin::new(1873.15));
    assert!((anorthite - 0.433).abs() <= 0.1, "{anorthite}");
}

/// The two Al-rich rows' records each lie within 0.5 log units of Scarfe et al. 1983's measured
/// stand-ins over their measured spans: `anorthosite` of the An100 fit over 1,648.15–1,898.15 K,
/// and `feldspathic` of the Di20 and Di30 fits interpolated linearly in mol% `Al₂O₃` to the soil's
/// 17.47 over 1,573.15–1,873.15 K (decision-p14-t49h-step4-melts §2.2: within about 0.13 and 0.35).
#[test]
fn the_al_rich_rows_meet_scarfe_s_stand_ins_over_their_spans() {
    let an100 = scarfe_fit(0.0);
    for k in 0..=250 {
        let t = 1648.15 + f64::from(k);
        let miss =
            record_log10_viscosity("anorthosite", t) - an100.log10_viscosity_pa_s(Kelvin::new(t));
        assert!(miss.abs() <= 0.5, "anorthosite at {t} K: {miss}");
    }
    let soil = mole_pct(melt_of("feldspathic").oxides, Oxide::Alumina);
    assert!((soil - 17.47).abs() < 0.01, "{soil}");
    let (di20, di30) = (
        mole_pct(&join(20.0), Oxide::Alumina),
        mole_pct(&join(30.0), Oxide::Alumina),
    );
    let toward_di20 = (soil - di30) / (di20 - di30);
    let (fit20, fit30) = (scarfe_fit(20.0), scarfe_fit(30.0));
    for k in 0..=300 {
        let t = Kelvin::new(1573.15 + f64::from(k));
        let stand_in = (1.0 - toward_di20) * fit30.log10_viscosity_pa_s(t)
            + toward_di20 * fit20.log10_viscosity_pa_s(t);
        let miss = record_log10_viscosity("feldspathic", t.value()) - stand_in;
        assert!(miss.abs() <= 0.5, "feldspathic at {} K: {miss}", t.value());
    }
}

/// Giordano and Russell 2007's viscosity falls monotonically as the melt warms, over the two rows'
/// whole range from their 1,533.15 K solidus, here to 10,000 K, towards its 10^−4.67 Pa s.
#[test]
fn gr07_falls_monotonically_over_the_record_s_range() {
    for key in ["anorthosite", "feldspathic"] {
        let mut previous = f64::INFINITY;
        for k in 0..=8_467 {
            let t = 1533.15 + f64::from(k);
            let mu = liquid_properties(row(key), Kelvin::new(t), Composition::Reference)
                .unwrap()
                .viscosity_pa_s
                .unwrap();
            assert!(mu < previous, "{key} at {t} K");
            assert!(mu > math::exp10(GR07_A), "{key} at {t} K");
            previous = mu;
        }
    }
}

/// decision-p14-t49h-step4-melts §2.3's rule sends exactly `anorthosite` and `feldspathic` to
/// Giordano and Russell 2007, and no row to a ruling; each melt row cites the model it takes. Its
/// edges: 15 wt% `Al₂O₃` and 1 wt% alkalis are inside it, a melt that meets both but drops more
/// than 3 wt% is unruled, with its viscosity absent, and one with more alkalis or less alumina
/// takes GRD.
#[test]
fn the_rule_sends_exactly_the_al_rich_rows_to_gr07() {
    let mut chosen = Vec::new();
    for (key, melt) in melt_rows() {
        let model = MeltViscosityModel::of(melt.oxides);
        assert_ne!(model, MeltViscosityModel::Unruled, "{key}");
        let columns = row(key).substance().liquid().unwrap();
        let cites = |author: &str| {
            columns
                .sources()
                .iter()
                .any(|s| s.covers() == "μ" && s.citation().starts_with(author))
        };
        if model == MeltViscosityModel::GiordanoRussell2007 {
            chosen.push(key);
            assert!(cites("Giordano and Russell 2007"), "{key}");
            assert!(!cites("Giordano, Russell and Dingwell 2008"), "{key}");
        } else {
            assert!(cites("Giordano, Russell and Dingwell 2008"), "{key}");
            assert!(!cites("Giordano and Russell 2007"), "{key}");
        }
    }
    assert_eq!(chosen, ["anorthosite", "feldspathic"]);
    let edge = [
        (Oxide::Silica, 50.0),
        (Oxide::Alumina, 15.0),
        (Oxide::Lime, 20.0),
        (Oxide::Magnesia, 11.0),
        (Oxide::Soda, 1.0),
        (Oxide::Titania, 2.0),
        (Oxide::Ferrous, 1.0),
    ];
    assert_eq!(
        MeltViscosityModel::of(&edge),
        MeltViscosityModel::GiordanoRussell2007
    );
    let mut titanian = edge;
    titanian[5] = (Oxide::Titania, 2.5);
    assert_eq!(
        MeltViscosityModel::of(&titanian),
        MeltViscosityModel::Unruled
    );
    assert_eq!(MeltViscosityModel::Unruled.vft(&titanian), None);
    let mut sodic = edge;
    sodic[4] = (Oxide::Soda, 1.5);
    assert_eq!(
        MeltViscosityModel::of(&sodic),
        MeltViscosityModel::Giordano2008
    );
    let mut aluminous = edge;
    aluminous[1] = (Oxide::Alumina, 14.9);
    assert_eq!(
        MeltViscosityModel::of(&aluminous),
        MeltViscosityModel::Giordano2008
    );
}

/// Lange and Carmichael's density of Alae's melt at 1,130 °C meets Peck 1978's 2,740 kg m⁻³ for
/// the lava lake's melt fraction, calculated from the lake's level surveys and its core's density,
/// within the ±30 kg m⁻³ he puts on the bulk (USGS Professional Paper 935-B, p. B37): 2,713.3, 1.0%
/// below, as the lane's research computes it (with CIAAW molar masses, 2,713.3).
#[test]
fn alae_s_melt_density_is_peck_s() {
    let record =
        liquid_properties(row("basalt"), Kelvin::new(1403.15), Composition::Reference).unwrap();
    assert!(
        (record.density_kg_m3 - 2740.0).abs() <= 30.0,
        "{}",
        record.density_kg_m3
    );
    assert!(
        (record.density_kg_m3 - 2713.3).abs() <= 0.5,
        "{}",
        record.density_kg_m3
    );
}

/// Every melt row's oxides are listed once each, positive, and sum to 100 within 1 wt%, the
/// analyses' own spread: from G-1's 99.57 (its water dropped) to the Apollo 16 soil's 100.71
/// (Table 7.15's column as printed), each sum the one its source's doc states.
#[test]
fn every_melt_s_oxides_sum_to_its_analysis() {
    let stated = [
        ("mars_dust", 99.66),
        ("anorthosite", 100.001),
        ("feldspathic", 100.71),
        ("basalt", 100.00),
        ("basalt_fe", 99.66),
        ("komatiite", 100.1),
        ("basalt_reduced", 99.98),
        ("granite", 99.57),
        ("ultramafic", 100.1463),
        ("ferric", 99.66),
        ("lava_glass", 100.00),
    ];
    let rows = melt_rows();
    assert_eq!(rows.len(), stated.len());
    for ((key, melt), (stated_key, total)) in rows.into_iter().zip(stated) {
        assert_eq!(key, stated_key);
        let mut sum = 0.0;
        for (i, &(oxide, w)) in melt.oxides.iter().enumerate() {
            assert!(w > 0.0, "{key}: {oxide:?}");
            assert!(
                melt.oxides[..i].iter().all(|&(other, _)| other != oxide),
                "{key}: {oxide:?} twice"
            );
            sum += w;
        }
        assert!((sum - total).abs() < 1e-6, "{key}: {sum} against {total}");
        assert!((sum - 100.0).abs() <= 1.0, "{key}: {sum}");
    }
}

/// The lithologies, minerals and organics with no melt record answer `NotALiquid`, each for its
/// reason in `rows/melts.rs`'s note, and so does molten sulphur, whose liquid density waits for
/// P14.T49.b's last piece.
#[test]
fn the_rows_without_a_melt_record_are_not_liquids() {
    for key in [
        "ca_al_residue",
        "iron_nickel",
        "phyllosilicate",
        "chondrite_ci",
        "chondrite_cm",
        "chondrite_cv",
        "chondrite_h",
        "chondrite_l",
        "chondrite_ll",
        "chondrite_e",
        "cometary",
        "SiC",
        "carbonaceous",
        "MgSO4",
        "CaCO3",
        "Na2CO3",
        "S8",
    ] {
        let id = row(key);
        assert!(id.substance().liquid().is_none(), "{key}");
        assert_eq!(
            liquid_properties(id, Kelvin::new(2_000.0), Composition::Reference),
            Err(ResolveLiquidError::NotALiquid {
                key: id.substance().key()
            }),
            "{key}"
        );
    }
}

/// A melt's given composition is not modelled, its composition being its oxides
/// (decision-p14-t49h-step3-composition §2.3), and that refusal comes before the range's: `SiO₂`'s
/// own row given to `basalt` below its solidus is `CompositionNotModelled`.
#[test]
fn a_melt_s_given_composition_is_not_modelled() {
    let silica = row("SiO2");
    let fractions = [(silica, 1.0)];
    let given = Composition::Given(LiquidComposition::new(&fractions).unwrap());
    for (key, _) in melt_rows() {
        let id = row(key);
        let columns = id.substance().liquid().unwrap();
        assert_eq!(columns.components(), [] as [SubstanceId; 0], "{key}");
        assert_eq!(columns.reference_composition(), None, "{key}");
        for t in [1_000.0, 2_000.0] {
            assert_eq!(
                liquid_properties(id, Kelvin::new(t), given),
                Err(ResolveLiquidError::CompositionNotModelled {
                    key: id.substance().key()
                }),
                "{key} at {t} K"
            );
        }
    }
}

/// A melt's range runs from its solidus with no upper end: a microkelvin below it, and at a
/// temperature that is not finite, the record is `OutOfRange` with `below` `None`; at it and at
/// 10,000 K it resolves.
#[test]
fn a_melt_below_its_solidus_is_out_of_range() {
    for (key, _) in melt_rows() {
        let id = row(key);
        let solidus = id
            .substance()
            .condensed()
            .unwrap()
            .transition()
            .temperature();
        for t in [solidus.value() - 1e-6, f64::NAN, f64::INFINITY] {
            let refusal = liquid_properties(id, Kelvin::new(t), Composition::Reference);
            assert!(
                matches!(
                    refusal,
                    Err(ResolveLiquidError::OutOfRange { lowest, below: None, .. })
                        if lowest == solidus
                ),
                "{key} at {t} K: {refusal:?}"
            );
        }
        for t in [solidus.value(), 10_000.0] {
            assert!(
                liquid_properties(id, Kelvin::new(t), Composition::Reference).is_ok(),
                "{key} at {t} K"
            );
        }
    }
}

/// Each melt's record is its models' and its held values: the density Lange and Carmichael's, the
/// viscosity its rule's model's, the index and absorption the basaltic glass's bins to the bit,
/// the stated surface tension, no scattering, and `StandIn`; and at 1,673.15 K its log₁₀ μ and ρ
/// meet the lane's independent evaluation (`step4/impl/scripts/oracle.py`, with the registry's
/// molar masses) within 10⁻⁹ log units and 10⁻⁶ kg m⁻³.
#[test]
fn each_melt_s_record_is_its_models() {
    let oracle = [
        ("mars_dust", 0.351_914_399_605_047_56, 2_825.611_103_946_418),
        (
            "anorthosite",
            1.523_445_173_358_949_1,
            2_587.378_978_726_546,
        ),
        (
            "feldspathic",
            0.870_005_736_767_200_9,
            2_647.229_995_217_491_3,
        ),
        ("basalt", 0.745_738_957_199_690_4, 2_664.378_983_098_012_7),
        ("basalt_fe", 0.351_914_399_605_047_56, 2_825.611_103_946_418),
        (
            "komatiite",
            -0.226_887_869_680_489_02,
            2_818.294_518_491_296,
        ),
        (
            "basalt_reduced",
            0.503_005_493_392_603_1,
            2_597.939_852_596_054,
        ),
        ("granite", 3.732_298_477_070_958, 2_321.803_230_857_328),
        (
            "ultramafic",
            -0.096_673_027_706_996_7,
            2_815.693_337_814_139,
        ),
        ("ferric", 0.351_914_399_605_047_56, 2_825.611_103_946_418),
        (
            "lava_glass",
            0.745_738_957_199_690_4,
            2_664.378_983_098_012_7,
        ),
    ];
    let t = Kelvin::new(1673.15);
    for ((key, melt), (oracle_key, log_mu, rho)) in melt_rows().into_iter().zip(oracle) {
        assert_eq!(key, oracle_key);
        let record = liquid_properties(row(key), t, Composition::Reference).unwrap();
        assert_eq!(record.basis, LiquidBasis::StandIn, "{key}");
        assert_eq!(record.refractive_index, bins::BASALTIC_GLASS_N, "{key}");
        assert_eq!(
            record.absorption_per_m,
            Some(bins::BASALTIC_GLASS_A),
            "{key}"
        );
        assert_eq!(record.scattering_per_m, None, "{key}");
        assert_eq!(
            record.surface_tension_n_per_m,
            Some(melt.surface_tension_n_per_m),
            "{key}"
        );
        let density = lange_carmichael_density(melt.oxides, t).value();
        assert!((record.density_kg_m3 - density).abs() < 1e-12, "{key}");
        let vft = MeltViscosityModel::of(melt.oxides)
            .vft(melt.oxides)
            .unwrap();
        let mu = record.viscosity_pa_s.unwrap();
        assert!(
            (math::log10(mu) - vft.log10_viscosity_pa_s(t)).abs() < 1e-12,
            "{key}"
        );
        assert!((math::log10(mu) - log_mu).abs() < 1e-9, "{key}: {mu}");
        assert!((record.density_kg_m3 - rho).abs() < 1e-6, "{key}");
    }
}

/// The surface tensions are the sources' constants: Bagdassarov, Dorfman and Dingwell 2000's 0.29
/// N m⁻¹ for `granite`, Walker and Mullins 1981's 0.36 for every other melt, the same at the
/// solidus and at 3,000 K.
#[test]
fn the_surface_tensions_are_the_stated_constants() {
    for (key, _) in melt_rows() {
        let expected = if key == "granite" { 0.29 } else { 0.36 };
        let id = row(key);
        let solidus = id
            .substance()
            .condensed()
            .unwrap()
            .transition()
            .temperature();
        for t in [solidus, Kelvin::new(3_000.0)] {
            let sigma = liquid_properties(id, t, Composition::Reference)
                .unwrap()
                .surface_tension_n_per_m
                .unwrap();
            assert!((sigma - expected).abs() < 1e-12, "{key}: {sigma}");
        }
    }
}
