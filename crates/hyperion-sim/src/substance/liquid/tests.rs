//! The liquid columns' tests (P14.T49.h): the IAPWS releases reproduced, water's record against
//! its oracles, the R08 files carried, and the record's rules.

use hyperion_surface::field::{
    LIQUID_BIN_CENTRES_NM, MaterialPalette, MechanicsFamily, PaletteEntry, PaletteRole,
};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::super::rows::equations::{
    AMMONIA_IDEAL, CARBON_DIOXIDE_IDEAL, ETHANE_IDEAL, METHANE_IDEAL, NITROGEN_IDEAL,
};
use super::super::rows::liquids::{IAPWS_95, IAPWS_95_IDEAL};
use super::*;
use crate::substance::{by_key, ids};
use crate::tables::liquid_optics as bins;

/// The row `key`, which a test knows to be a row.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap()
}

/// The row's liquid range at its reference composition, K: its first melt, and the temperature
/// the range ends below, the lowest critical point among its components (a pure liquid's own), or
/// 100 K above its first melt for a row that states none.
fn reference_range(id: SubstanceId) -> (f64, f64) {
    let substance = id.substance();
    let lowest = substance
        .condensed()
        .unwrap()
        .transition()
        .temperature()
        .value();
    let components = substance.liquid().unwrap().components();
    let top = if components.is_empty() {
        lowest + 100.0
    } else {
        components
            .iter()
            .filter_map(|c| c.substance().phase_diagram())
            .map(|d| d.critical_temperature().value())
            .fold(f64::INFINITY, f64::min)
    };
    (lowest, top)
}

/// The uniform mean of `f` over [`low`, `high`] by composite Simpson's rule on 64 steps: the
/// oracles below are smooth across a bin, so this is exact to well under their tolerances.
fn simpson_mean(f: impl Fn(f64) -> f64, low: f64, high: f64) -> f64 {
    let steps = 64_u32;
    let h = (high - low) / f64::from(steps);
    let mut sum = f(low) + f(high);
    for i in 1..steps {
        let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
        sum += weight * f(low + h * f64::from(i));
    }
    sum * h / 3.0 / (high - low)
}

/// The liquid bins' edges are R08's, centred on the surface crate's bin centres.
#[test]
fn the_liquid_bins_are_the_surface_crate_s() {
    assert!((LIQUID_BIN_EDGES_NM[0] - 380.0).abs() < 1e-12);
    assert!((LIQUID_BIN_EDGES_NM[LIQUID_BINS] - 760.0).abs() < 1e-12);
    for (k, centre) in LIQUID_BIN_CENTRES_NM.iter().enumerate() {
        let mid = f64::midpoint(LIQUID_BIN_EDGES_NM[k], LIQUID_BIN_EDGES_NM[k + 1]);
        assert!(
            (mid - centre).abs() < 1e-9,
            "bin {k}: {mid} against {centre}"
        );
    }
}

/// IAPWS-95's residual reproduces the release's Table 6 at 500 K and 838.025 kg m⁻³ (`φʳ_δ` =
/// −0.364 366 650, `φʳ_δδ` = 0.856 063 701) and its Table 7 pressures, each printed to nine
/// figures, within 6 × 10⁻⁹ relative: just over half a unit of a ninth figure whose leading
/// digit is 1 (the release warns that the liquid's pressure at low pressures may miss its ninth
/// figure, its footnote a). Its τ-derivatives and its ideal part's reproduce Table 6 too (`φʳ_ττ`
/// = −2.234 407 37, `φʳ_δτ` = −1.121 769 15 and `φ°_ττ` = −1.932 491 85), within 5 × 10⁻⁹, half
/// a unit of their ninth figure (4.5 × 10⁻⁹ for `φʳ_δτ`'s).
#[test]
fn iapws_95_reproduces_its_release_s_tables_6_and_7() {
    let d = IAPWS_95.density_derivatives(KilogramsPerCubicMetre::new(838.025), Kelvin::new(500.0));
    assert!((d.first / -0.364_366_650 - 1.0).abs() < 2e-9, "{}", d.first);
    assert!(
        (d.second / 0.856_063_701 - 1.0).abs() < 2e-9,
        "{}",
        d.second
    );
    let t =
        IAPWS_95.temperature_derivatives(KilogramsPerCubicMetre::new(838.025), Kelvin::new(500.0));
    assert!(
        (t.second / -2.234_407_37 - 1.0).abs() < 5e-9,
        "{}",
        t.second
    );
    assert!((t.mixed / -1.121_769_15 - 1.0).abs() < 5e-9, "{}", t.mixed);
    let ideal = IAPWS_95_IDEAL.second_tau_derivative(647.096 / 500.0);
    assert!((ideal / -1.932_491_85 - 1.0).abs() < 5e-9, "{ideal}");
    for (t, rho, mpa) in [
        (300.0, 0.996_556_0e3, 0.992_418_352e-1),
        (300.0, 0.100_530_8e4, 0.200_022_515e2),
        (300.0, 0.118_820_2e4, 0.700_004_704e3),
        (500.0, 0.435, 0.999_679_423e-1),
        (500.0, 0.453_2e1, 0.999_938_125),
        (500.0, 0.838_025e3, 0.100_003_858e2),
        (500.0, 0.108_456_4e4, 0.700_000_405e3),
        (647.0, 0.358e3, 0.220_384_756e2),
        (900.0, 0.241, 0.100_062_559),
        (900.0, 0.526_15e2, 0.200_000_690e2),
        (900.0, 0.870_769e3, 0.700_000_006e3),
    ] {
        let p = IAPWS_95.pressure(KilogramsPerCubicMetre::new(rho), Kelvin::new(t));
        let ours = p.value() / 1e6;
        assert!(
            (ours / mpa - 1.0).abs() < 6e-9,
            "p({t} K, {rho} kg/m³) = {ours} MPa against {mpa}"
        );
    }
}

/// IAPWS R12-08's viscosity reproduces its release's Table 4 (μ₂ = 1) to its printed 10⁻⁶ μPa s.
#[test]
fn iapws_2008_viscosity_reproduces_its_release_s_table_4() {
    let water = &super::super::rows::liquids::IAPWS_VISCOSITY;
    for (t, rho, micro) in [
        (298.15, 998.0, 889.735_100),
        (298.15, 1_200.0, 1_437.649_467),
        (373.15, 1_000.0, 307.883_622),
        (433.15, 1.0, 14.538_324),
        (433.15, 1_000.0, 217.685_358),
        (873.15, 1.0, 32.619_287),
        (873.15, 100.0, 35.802_262),
        (873.15, 600.0, 77.430_195),
        (1_173.15, 1.0, 44.217_245),
        (1_173.15, 100.0, 47.640_433),
        (1_173.15, 400.0, 64.154_608),
    ] {
        let ours = water.viscosity_pa_s(Kelvin::new(t), KilogramsPerCubicMetre::new(rho)) * 1e6;
        assert!(
            (ours - micro).abs() < 1e-6,
            "μ({t} K, {rho}) = {ours} μPa s against {micro}"
        );
    }
}

/// IAPWS R1-76's surface tension reproduces its release's Table 1, the calculated column, to its
/// printed 0.01 mN m⁻¹.
#[test]
fn iapws_surface_tension_reproduces_its_release_s_table_1() {
    let tension = by_key("H2O")
        .and_then(|id| id.substance().liquid())
        .and_then(LiquidColumns::surface_tension)
        .unwrap();
    for (celsius, milli) in [
        (0.01, 75.65),
        (15.0, 73.49),
        (25.0, 71.97),
        (50.0, 67.94),
        (100.0, 58.91),
        (150.0, 48.74),
        (175.0, 43.30),
    ] {
        let ours = tension.surface_tension_n_per_m(Kelvin::new(273.15 + celsius)) * 1e3;
        assert!(
            (ours - milli).abs() <= 0.005 + 1e-9,
            "{celsius} °C: {ours} mN/m against {milli}"
        );
    }
}

/// IAPWS R9-97's real index of water (its Eq. 1 and Table 1) at `temperature` K, `density`
/// kg m⁻³ and the vacuum wavelength `nm`, the test's oracle.
fn iapws_r9_97(temperature: f64, density: f64, nm: f64) -> f64 {
    let a = [
        0.244_257_733,
        9.746_344_76e-3,
        -3.732_349_96e-3,
        2.686_784_72e-4,
        1.589_205_70e-3,
        2.459_342_59e-3,
        0.900_704_920,
        -1.666_262_19e-2,
    ];
    let (uv, ir) = (0.229_202_0, 5.432_937);
    let t = temperature / 273.15;
    let rho = density / 1_000.0;
    let l = nm / 1_000.0 / 0.589;
    let l2 = l * l;
    let rhs = a[0]
        + a[1] * rho
        + a[2] * t
        + a[3] * l2 * t
        + a[4] / l2
        + a[5] / (l2 - uv * uv)
        + a[6] / (l2 - ir * ir)
        + a[7] * rho * rho;
    from_lorentz_lorenz(rhs * rho)
}

/// R9-97's oracle reproduces the release's own Table 3 at 0 °C and 0.1 megapascal, at IAPWS-95's
/// density there (about 999.842 kg m⁻³, solved here from the residual, as the release says its
/// table takes it), so the comparisons below test the record, not the oracle.
#[test]
fn the_r9_97_oracle_reproduces_its_release_s_table_3() {
    let t = Kelvin::new(273.15);
    let mut rho = 1_000.0_f64;
    for _ in 0..30 {
        let state = KilogramsPerCubicMetre::new(rho);
        let p = IAPWS_95.pressure(state, t).value();
        // dp ÷ dρ = 1 ÷ (ρ κ_T).
        rho -= (p - 0.1e6) * rho * IAPWS_95.isothermal_compressibility_per_pa(state, t);
    }
    assert!((rho - 999.842).abs() < 1e-3, "{rho}");
    for (nm, n) in [
        (226.50, 1.394_527),
        (589.00, 1.334_344),
        (1_013.98, 1.326_135),
    ] {
        let ours = iapws_r9_97(273.15, rho, nm);
        assert!((ours - n).abs() < 2e-6, "{nm} nm: {ours} against {n}");
    }
}

/// Water's index, carried from `water.json`'s 25 °C by Lorentz–Lorenz, lies within 5 × 10⁻⁴ of
/// IAPWS R9-97's at the record's own state over 273.16–303.15 K in every bin
/// (decision-r11-palette-liquids §1.8's oracle; decision-r08-t5g §1 measured the method's miss at
/// the triple point, −4.75 × 10⁻⁴ at 400 nm), and within 2 × 10⁻⁴ at 288.15 K.
#[test]
fn water_s_index_is_iapws_r9_97_s() {
    let water = row("H2O");
    for (t, tolerance) in [
        (273.16, 5e-4),
        (278.15, 5e-4),
        (288.15, 2e-4),
        (298.15, 5e-4),
        (303.15, 5e-4),
    ] {
        let record = liquid_properties(water, Kelvin::new(t), Composition::Reference).unwrap();
        for (k, n) in record.refractive_index.iter().enumerate() {
            let oracle = simpson_mean(
                |nm| iapws_r9_97(t, record.density_kg_m3, nm),
                LIQUID_BIN_EDGES_NM[k],
                LIQUID_BIN_EDGES_NM[k + 1],
            );
            assert!(
                (n - oracle).abs() < tolerance,
                "{t} K bin {k}: {n} against R9-97's {oracle}"
            );
        }
    }
}

/// At its file's own state the index is the file's.
#[test]
fn water_s_index_at_its_file_s_state_is_the_file_s() {
    let record =
        liquid_properties(row("H2O"), Kelvin::new(298.15), Composition::Reference).unwrap();
    for (n, file) in record.refractive_index.iter().zip(&bins::WATER_N) {
        assert!((n - file).abs() < 4e-16, "{n} against {file}");
    }
}

/// Zhang, Hu and He 2009's published code (`betasw_ZHH2009.m`) at zero salinity, the test's
/// oracle: the total scattering coefficient b, m⁻¹, at `nm` and `celsius`, with its own constants
/// (CODATA 1998's `k_B`), its n (Quan and Fry 1995 times Ciddor 1996's air), its `κ_T` (Millero et
/// al. 1980's secant bulk modulus of pure water at one atmosphere), the PMH model and the
/// depolarisation ratio `delta` (the code's own is 0.039).
fn zhang_hu_he(nm: f64, celsius: f64, delta: f64) -> f64 {
    let kb = 1.380_650_3e-23;
    let tk = celsius + 273.15;
    let inverse_um2 = 1.0 / ((nm / 1e3) * (nm / 1e3));
    let n_air =
        1.0 + (5_792_105.0 / (238.018_5 - inverse_um2) + 167_917.0 / (57.362 - inverse_um2)) / 1e8;
    let n_water = (1.314_05 - 2.02e-6 * celsius * celsius + (15.868 - 0.004_23 * celsius) / nm
        - 4_382.0 / (nm * nm)
        + 1.145_5e6 / (nm * nm * nm))
        * n_air;
    let secant_bar = 19_652.21 + 148.420_6 * celsius - 2.327_105 * celsius * celsius
        + 1.360_477e-2 * celsius * celsius * celsius
        - 5.155_288e-5 * celsius * celsius * celsius * celsius;
    let kappa = 1.0 / secant_bar * 1e-5;
    // The code's PMH, written out here so that the oracle does not share the module's.
    let n2 = n_water * n_water;
    let bracket = n_water / 3.0 - 1.0 / 3.0 / n_water;
    let pmh = (n2 - 1.0) * (1.0 + 2.0 / 3.0 * (n2 + 2.0) * bracket * bracket);
    let lambda = nm * 1e-9;
    let beta_90 =
        std::f64::consts::PI * std::f64::consts::PI / 2.0 / (lambda * lambda * lambda * lambda)
            * kb
            * tk
            * kappa
            * pmh
            * pmh
            * (6.0 + 6.0 * delta)
            / (6.0 - 7.0 * delta);
    8.0 * std::f64::consts::PI / 3.0 * beta_90 * (2.0 + delta) / (1.0 + delta)
}

/// Water's scattering lies within 0.5% of Zhang, Hu and He 2009's at zero salinity in every bin
/// over 273.16–303.15 K (their bin means; the difference is their `κ_T`, Millero et al. 1980's, and
/// their index, Quan and Fry's, against IAPWS-95 and R9-97's), their model taking the record's δ,
/// which follows the state from water's anisotropy ratio (decision-p14-t49h-gas-index §4.2), and at
/// 298.15 K, where that δ is their 0.039, their model as published; at 20 °C it is about 0.003 26
/// m⁻¹ in the bin across 443 nm and 0.001 37 in the bin across 545 nm, pure water's: Morel 1974's
/// 0.0045 and 0.0019 m⁻¹ at 450 and 550 nm, which decision-r11-palette-liquids §1.8 quotes from
/// memory, are his seawater's (decision-r10-t10b-r09-t2e §7).
#[test]
fn water_s_scattering_is_zhang_hu_and_he_s_at_zero_salinity() {
    let water = row("H2O");
    let inputs = water.substance().liquid().unwrap().scattering().unwrap();
    for t in [273.16, 283.15, 288.15, 293.15, 298.15, 303.15] {
        let record = liquid_properties(water, Kelvin::new(t), Composition::Reference).unwrap();
        let b = record.scattering_per_m.unwrap();
        let delta = inputs.depolarisation_ratio(
            KilogramsPerCubicMetre::new(record.density_kg_m3),
            Kelvin::new(t),
        );
        if (t - 298.15).abs() < 1e-9 {
            assert!((delta - 0.039).abs() < 1e-9, "{delta}");
        }
        for (k, ours) in b.iter().enumerate() {
            let oracle = simpson_mean(
                |nm| zhang_hu_he(nm, t - 273.15, delta),
                LIQUID_BIN_EDGES_NM[k],
                LIQUID_BIN_EDGES_NM[k + 1],
            );
            assert!(
                (ours / oracle - 1.0).abs() < 5e-3,
                "{t} K bin {k}: {ours} m⁻¹ against {oracle}"
            );
        }
    }
    let warm = liquid_properties(water, Kelvin::new(293.15), Composition::Reference).unwrap();
    let b = warm.scattering_per_m.unwrap();
    // The bins across 443 and 545 nm.
    assert!((b[2] - 0.003_26).abs() < 0.000_07, "{}", b[2]);
    assert!((b[6] - 0.001_37).abs() < 0.000_03, "{}", b[6]);
}

/// Water at 15 °C has every member, measured, its density the condensed columns' to the bit, its
/// surface tension R1-76's and its viscosity R12-08's at that density.
#[test]
fn water_at_15_c_is_measured_in_every_member() {
    let water = row("H2O");
    let t = Kelvin::new(288.15);
    let record = liquid_properties(water, t, Composition::Reference).unwrap();
    assert_eq!(record.basis, LiquidBasis::Measured);
    assert_same_bits(record.temperature.value(), 288.15);
    let density = water
        .substance()
        .grain_density(CondensedPhase::Liquid, t)
        .unwrap();
    assert_same_bits(record.density_kg_m3, density.value());
    // About 999.05 kg m⁻³, the saturated liquid's (IAPWS SR1-86's ancillary, T49.b's).
    assert!(
        (record.density_kg_m3 - 999.05).abs() < 0.1,
        "{}",
        record.density_kg_m3
    );
    let sigma = record.surface_tension_n_per_m.unwrap();
    assert!((sigma * 1e3 - 73.49).abs() < 0.01, "{sigma}");
    let mu = record.viscosity_pa_s.unwrap();
    assert!((mu - 1.137_6e-3).abs() < 1e-6, "{mu}");
    assert!(record.absorption_per_m.is_some() && record.scattering_per_m.is_some());
    for (a, file) in record.absorption_per_m.unwrap().iter().zip(&bins::WATER_A) {
        assert_same_bits(*a, *file);
    }
}

/// Water's `κ_T` at the record's state is within 0.1% of Millero et al. 1980's at one atmosphere,
/// the value Zhang, Hu and He take, from 0 to 30 °C.
#[test]
fn water_s_compressibility_is_millero_s() {
    for kelvin in [273.16_f64, 283.15, 293.15, 303.15] {
        let celsius = kelvin - 273.15;
        let t = Kelvin::new(kelvin);
        let rho = row("H2O")
            .substance()
            .grain_density(CondensedPhase::Liquid, t)
            .unwrap();
        let ours = IAPWS_95.isothermal_compressibility_per_pa(rho, t);
        let secant_bar = 19_652.21 + 148.420_6 * celsius - 2.327_105 * celsius * celsius
            + 1.360_477e-2 * celsius * celsius * celsius
            - 5.155_288e-5 * celsius * celsius * celsius * celsius;
        let millero = 1.0 / (secant_bar * 1e5);
        assert!(
            (ours / millero - 1.0).abs() < 1e-3,
            "{celsius} °C: {ours} against {millero}"
        );
    }
}

/// R11's ocean's test values (decision-r11-palette-liquids §4): water about 1.333 in the bin
/// across 589 nm at 20 °C; methane about 1.29 at Titan's 93.65 K; ethane 1.3887 at 670 nm and 94
/// K, Hagermann et al. 2005's measured level, which replaces R11's "about 1.37" and §1.8's "about
/// 1.40" (decision-r08-t5g §7).
#[test]
fn r11_s_test_values_hold() {
    let water = liquid_properties(row("H2O"), Kelvin::new(293.15), Composition::Reference).unwrap();
    assert!(
        (water.refractive_index[8] - 1.333).abs() < 5e-4,
        "{}",
        water.refractive_index[8]
    );
    let methane =
        liquid_properties(row("CH4"), Kelvin::new(93.65), Composition::Reference).unwrap();
    assert!(
        (methane.refractive_index[8] - 1.29).abs() < 2e-3,
        "{}",
        methane.refractive_index[8]
    );
    let ethane = liquid_properties(row("C2H6"), Kelvin::new(94.0), Composition::Reference).unwrap();
    assert!(
        (ethane.refractive_index[11] - 1.388_7).abs() < 5e-4,
        "{}",
        ethane.refractive_index[11]
    );
}

/// Methane at its triple point is Martonchik and Orton's 90 K index carried up 0.694 K, about
/// 6.5 × 10⁻⁴ lower as the liquid expands by about 0.20%, and the record is `Derived`: the file's
/// state lies below the triple point, where the density is the ancillary's triple-point line. Its
/// scattering, surface tension and viscosity are present (P14.T49.h's second step).
#[test]
fn methane_at_its_triple_point_is_martonchik_and_orton_s_carried() {
    let methane = row("CH4");
    let diagram = methane.substance().phase_diagram().unwrap();
    let triple = diagram.triple_temperature();
    let record = liquid_properties(methane, triple, Composition::Reference).unwrap();
    assert_eq!(record.basis, LiquidBasis::Derived);
    let (state, extended) = extended_liquid_density(diagram, Kelvin::new(90.0));
    assert!(extended);
    let ratio = record.density_kg_m3 / state.value();
    for (n, file) in record.refractive_index.iter().zip(&bins::METHANE_LIQUID_N) {
        assert!(
            n < file && file - n < 1e-3,
            "{n} against the 90 K file's {file}"
        );
        let carried = from_lorentz_lorenz(lorentz_lorenz(*file) * ratio);
        assert!((n - carried).abs() < 1e-15, "{n} against {carried}");
    }
    assert!(record.absorption_per_m.is_some());
    assert!(record.scattering_per_m.is_some());
    assert!(record.surface_tension_n_per_m.is_some());
    assert!(record.viscosity_pa_s.is_some());
}

/// Nitrogen at its normal boiling point, 77.35 K, lies within 0.5% of 1.20 in the bin across 545
/// nm (decision-r11-palette-liquids §4), from Augustin's measured liquid.
#[test]
fn nitrogen_at_its_boiling_point_is_within_half_a_percent_of_1_20() {
    let record = liquid_properties(row("N2"), Kelvin::new(77.35), Composition::Reference).unwrap();
    let n = record.refractive_index[6];
    assert!((n / 1.20 - 1.0).abs() < 5e-3, "{n}");
    assert_eq!(record.basis, LiquidBasis::Derived);
    assert!(
        record.absorption_per_m.is_none(),
        "{:?}",
        record.absorption_per_m
    );
}

/// Below a diagram row's triple point its density is the line at the ancillary's slope there,
/// continuous with the ancillary at the triple point, and the slope is the ancillary's own.
#[test]
fn the_extended_density_is_continuous_at_the_triple_point() {
    for (key, _) in &super::super::rows::liquids::LIQUIDS {
        let Some(diagram) = by_key(key.as_str()).and_then(|id| id.substance().phase_diagram())
        else {
            continue;
        };
        let triple = diagram.triple_temperature();
        let equation = diagram.liquid_density_equation();
        let slope = density_slope(equation, triple);
        let h = 1e-4;
        let above = equation.density(Kelvin::new(triple.value() + h)).value();
        let at = equation.density(triple).value();
        let numeric = (above - at) / h;
        assert!(
            (slope / numeric - 1.0).abs() < 1e-3,
            "{key}: slope {slope} against {numeric}"
        );
        let (below, extended) = extended_liquid_density(diagram, Kelvin::new(triple.value() - h));
        assert!(extended);
        assert!((below.value() - (at - slope * h)).abs() < 1e-9, "{key}");
        let (on, extended) = extended_liquid_density(diagram, triple);
        assert!(!extended);
        assert_same_bits(on.value(), at);
    }
}

/// Every row with liquid columns gives a record across its range that a palette's liquid entry
/// accepts, with no absent member written as 0: every present by-bin value is above zero but a bin
/// a pure liquid's file holds transparent, at +0, which is a value (oxygen's from 658.7 nm, where
/// its collision-induced bands end); a row whose file's k is not modelled leaves its absorption
/// absent, and a solution with a component whose absorption is not modelled leaves its own absent
/// (Titan's seas: ethane's and nitrogen's; ammonia–water: ammonia's; the sulphate brine: its
/// salt's), so that of the solutions only the chloride brine, its salt's transparency cited, and
/// the acid, its files' bounds, have one. Every liquid-range row has n and ρ, step 2b's nine among
/// them. The perchlorate brines, whose record is refused for its index, are the solutions' tests'.
#[test]
fn every_record_is_a_palette_s_and_no_absent_member_is_zero() {
    for (key, columns) in &super::super::rows::liquids::LIQUIDS {
        if let LiquidConstitution::AwaitingSolute(_) = columns.constitution {
            continue;
        }
        let id = row(key.as_str());
        let substance = id.substance();
        let (lowest, top) = reference_range(id);
        let highest = top - 1e-6;
        for fraction in [0.0, 0.1, 0.5, 0.9, 1.0] {
            let t = Kelvin::new(lowest + fraction * (highest - lowest));
            let record = liquid_properties(id, t, Composition::Reference).unwrap();
            if let Some(bins) = record.scattering_per_m {
                assert!(
                    bins.iter().all(|v| *v > 0.0),
                    "{key} at {} K: {bins:?}",
                    t.value()
                );
            }
            let stated = columns.optics.and_then(|optics| optics.absorption);
            if let Some(bins) = record.absorption_per_m {
                for (k, v) in bins.iter().enumerate() {
                    let transparent = stated.is_some_and(|(file, _)| file[k] <= 0.0);
                    assert!(
                        *v > 0.0 || (transparent && v.is_sign_positive()),
                        "{key} at {} K: {bins:?}",
                        t.value()
                    );
                }
            }
            let modelled = match columns.optics {
                Some(optics) => optics.absorption.is_some(),
                None => {
                    matches!(key.as_str(), "brine_nacl" | "H2SO4")
                        || matches!(columns.constitution, LiquidConstitution::Melt(_))
                }
            };
            assert_eq!(record.absorption_per_m.is_some(), modelled, "{key}");
            let entry = PaletteEntry {
                substance: *key,
                role: PaletteRole::Liquid,
                normal_albedo_bvr: record.fresnel_normal_albedo_bvr(),
                phase_row: 0,
                grain_density_kg_m3: record.density_kg_m3,
                transition: substance.condensed().unwrap().transition().temperature(),
                mechanics: MechanicsFamily::WaterIce,
                liquid: Some(record),
                feo_mass_fraction: None,
            };
            if let Err(refusal) = MaterialPalette::new(vec![entry]) {
                panic!("{key} at {} K: {refusal:?}", t.value());
            }
        }
    }
}

/// The rows with liquid columns are rows, in registry order, each with a liquid density.
#[test]
fn the_liquid_rows_are_rows_in_registry_order() {
    let indices: Vec<u16> = super::super::rows::liquids::LIQUIDS
        .iter()
        .map(|(key, _)| row(key.as_str()).index())
        .collect();
    assert!(indices.windows(2).all(|w| w[0] < w[1]), "{indices:?}");
    let with_columns = ids().filter(|id| id.substance().liquid().is_some()).count();
    assert_eq!(with_columns, indices.len());
}

/// A temperature outside the liquid range, a composition naming another row, a composition outside
/// the model's range (the pure acid), a composition of a liquid whose mixing is not yet modelled (a
/// perchlorate brine's), and a row with no liquid columns (neon, which has no R08 file) are
/// refused, each by its own error.
#[test]
fn what_is_not_modelled_is_refused() {
    let water = row("H2O");
    for t in [273.0, 647.096, 700.0, f64::NAN] {
        assert!(
            matches!(
                liquid_properties(water, Kelvin::new(t), Composition::Reference),
                Err(ResolveLiquidError::OutOfRange { .. })
            ),
            "{t} K"
        );
    }
    let fractions = [(row("CH4"), 1.0)];
    let composition = LiquidComposition::new(&fractions).unwrap();
    assert!(matches!(
        liquid_properties(water, Kelvin::new(288.15), Composition::Given(composition)),
        Err(ResolveLiquidError::ComponentNotModelled { .. })
    ));
    let acid = row("H2SO4");
    let fractions = [(acid, 1.0)];
    let composition = LiquidComposition::new(&fractions).unwrap();
    assert!(matches!(
        liquid_properties(acid, Kelvin::new(300.0), Composition::Given(composition)),
        Err(ResolveLiquidError::CompositionOutOfRange { .. })
    ));
    let fractions = [(water, 1.0)];
    let composition = LiquidComposition::new(&fractions).unwrap();
    assert!(matches!(
        liquid_properties(
            row("brine_naclo4"),
            Kelvin::new(300.0),
            Composition::Given(composition)
        ),
        Err(ResolveLiquidError::CompositionNotModelled { .. })
    ));
    assert!(matches!(
        liquid_properties(row("Ne"), Kelvin::new(26.0), Composition::Reference),
        Err(ResolveLiquidError::NotALiquid { .. })
    ));
    assert!(matches!(
        LiquidComposition::new(&[(water, 0.5)]),
        Err(BuildLiquidCompositionError::NotWhole { .. })
    ));
    assert_eq!(
        LiquidComposition::new(&[]),
        Err(BuildLiquidCompositionError::Empty)
    );
    assert_eq!(
        LiquidComposition::new(&[(water, 0.5), (water, 0.5)]),
        Err(BuildLiquidCompositionError::Repeated { component: water })
    );
    assert_eq!(
        LiquidComposition::new(&[(water, 1.5)]),
        Err(BuildLiquidCompositionError::FractionOutOfRange { component: water })
    );
}

/// The record is a pure function of its inputs: every row's, at its first melt, mid-range and
/// near its top, the same whatever was resolved before it and in whichever order
/// (sim-determinism, order independence).
#[test]
fn the_record_is_order_independent() {
    let mut cases = Vec::new();
    for (key, _) in &super::super::rows::liquids::LIQUIDS {
        let id = row(key.as_str());
        let (lowest, top) = reference_range(id);
        for fraction in [0.0, 0.5, 0.9] {
            cases.push((id, lowest + fraction * (top - lowest)));
        }
    }
    assert_order_independent(&cases, |&(id, t)| {
        liquid_properties(id, Kelvin::new(t), Composition::Reference)
    });
}

/// Near water's critical point the viscosity is in R12-08's critical region, where its omitted
/// enhancement exceeds 2%, so that member is `Derived` there and `Measured` at 600 K; IAPWS-95's
/// `κ_T` stays positive at the ancillary's states, so the scattering is present, growing as the
/// liquid nears the critical point.
#[test]
fn water_near_its_critical_point_is_derived() {
    let water = row("H2O");
    let hot = liquid_properties(water, Kelvin::new(647.0), Composition::Reference).unwrap();
    let warm = liquid_properties(water, Kelvin::new(600.0), Composition::Reference).unwrap();
    assert!(hot.density_kg_m3 < 405.3, "{}", hot.density_kg_m3);
    let viscosity = &super::super::rows::liquids::IAPWS_VISCOSITY;
    let at = |record: &LiquidProperties| {
        viscosity.basis_at(
            record.temperature,
            KilogramsPerCubicMetre::new(record.density_kg_m3),
        )
    };
    assert_eq!(at(&hot), LiquidBasis::Derived);
    assert_eq!(at(&warm), LiquidBasis::Measured);
    assert_eq!(hot.basis, LiquidBasis::Derived);
    let (hot_b, warm_b) = (
        hot.scattering_per_m.unwrap(),
        warm.scattering_per_m.unwrap(),
    );
    assert!(hot_b[7] > warm_b[7], "{} against {}", hot_b[7], warm_b[7]);
}

/// Water's density is IAPWS-95's saturated liquid, its release's Table 8 (from the phase
/// equilibrium of Eq. 4): within 10⁻⁵ relative at 275 and 450 K, and within 10⁻³ at 625 K, near
/// the critical point, where the ancillary's error grows; and IAPWS-95's pressure at the record's
/// state lies within 0.11 megapascal of zero below 373 K, the saturated liquid's.
#[test]
fn water_s_density_is_iapws_95_s_saturated_liquid() {
    let water = row("H2O");
    for (t, table_8, tolerance) in [
        (275.0, 999.887_406, 1e-5),
        (450.0, 890.341_250, 1e-5),
        (625.0, 567.090_385, 1e-3),
    ] {
        let record = liquid_properties(water, Kelvin::new(t), Composition::Reference).unwrap();
        assert!(
            (record.density_kg_m3 / table_8 - 1.0).abs() < tolerance,
            "{t} K: {} against {table_8}",
            record.density_kg_m3
        );
    }
    for t in [273.16, 298.15, 323.15, 373.15] {
        let at = Kelvin::new(t);
        let rho = water
            .substance()
            .grain_density(CondensedPhase::Liquid, at)
            .unwrap();
        let p = IAPWS_95.pressure(rho, at).value();
        assert!(p.abs() < 0.11e6, "{t} K: {p} Pa");
    }
}

/// Water's windows are its getters': the carried index checked over 273.16–373.15 K and the
/// absorption over 273.16–303.15 K (decision-p14-t49h-liquids §1); the record is `Measured` at
/// 303.15 K and `Derived` just above it, where the absorption is held from 22–23 °C, and stays
/// `Derived` above 373.15 K, where the index's carry passes 5 × 10⁻⁴ too.
#[test]
fn water_outside_its_checked_windows_is_derived() {
    let water = row("H2O");
    let optics = water.substance().liquid().unwrap().optics().unwrap();
    let assert_window = |window: Option<(Kelvin, Kelvin)>, (low, high): (f64, f64)| {
        let (from, to) = window.unwrap_or_else(|| panic!("water's window from {low} K is absent"));
        assert_same_bits(from.value(), low);
        assert_same_bits(to.value(), high);
    };
    assert_window(optics.index_checked(), (273.16, 373.15));
    assert_window(optics.absorption_checked(), (273.16, 303.15));
    let at = |t: f64| {
        liquid_properties(water, Kelvin::new(t), Composition::Reference)
            .unwrap()
            .basis
    };
    assert_eq!(at(303.15), LiquidBasis::Measured);
    assert_eq!(at(303.16), LiquidBasis::Derived);
    assert_eq!(at(400.0), LiquidBasis::Derived);
    for (key, columns) in &super::super::rows::liquids::LIQUIDS {
        if let (Some(optics), false) = (columns.optics, key.as_str() == "H2O") {
            let checked = optics.absorption_checked();
            assert!(checked.is_none(), "{key}: {checked:?}");
        }
    }
}

/// Each row's optics state is its file's: within half a kelvin of the temperature the table
/// carries from the file's label, for the first five, whose label rounds their reduction's state to
/// the kelvin, and to the bit for step 2b's nine, whose files state it to 0.01 K
/// (decision-p14-t49h-gas-index §2.3), never below the row's triple point; so a file whose
/// temperature changes fails here (the acid's two files are the solutions' tests').
#[test]
fn each_optics_state_is_its_file_s() {
    let labelled = [
        ("H2O", bins::WATER_TEMPERATURE_K),
        ("CH4", bins::METHANE_LIQUID_TEMPERATURE_K),
        ("NH3", bins::AMMONIA_LIQUID_TEMPERATURE_K),
        ("N2", bins::NITROGEN_LIQUID_TEMPERATURE_K),
        ("C2H6", bins::ETHANE_LIQUID_TEMPERATURE_K),
    ];
    for (key, label) in labelled {
        let optics = row(key).substance().liquid().unwrap().optics().unwrap();
        let state = optics.state().value();
        assert!(
            (state - label).abs() <= 0.5,
            "{key}: state {state} K against {label} K"
        );
    }
    for (key, label) in GAS_DERIVED_FILES {
        let substance = row(key).substance();
        let state = substance
            .liquid()
            .unwrap()
            .optics()
            .unwrap()
            .state()
            .value();
        assert_same_bits(state, label);
        let triple = substance.phase_diagram().unwrap().triple_temperature();
        assert!(state >= triple.value(), "{key}: {state} K");
    }
}

/// Step 2b's nine rows, each with its file's stated temperature (`tables::liquid_optics`).
const GAS_DERIVED_FILES: [(&str, f64); 9] = [
    ("O2", bins::OXYGEN_LIQUID_TEMPERATURE_K),
    ("CO2", bins::CARBON_DIOXIDE_LIQUID_TEMPERATURE_K),
    ("Ar", bins::ARGON_LIQUID_TEMPERATURE_K),
    ("Kr", bins::KRYPTON_LIQUID_TEMPERATURE_K),
    ("Xe", bins::XENON_LIQUID_TEMPERATURE_K),
    ("N2O", bins::NITROUS_OXIDE_LIQUID_TEMPERATURE_K),
    ("CO", bins::CARBON_MONOXIDE_LIQUID_TEMPERATURE_K),
    ("SO2", bins::SULPHUR_DIOXIDE_LIQUID_TEMPERATURE_K),
    ("H2S", bins::HYDROGEN_SULPHIDE_LIQUID_TEMPERATURE_K),
];

/// A table's isothermal compressibility, Pa⁻¹, from its `c_v`, `c_p` (any one unit) and sound speed
/// w (m s⁻¹) at `density` (kg m⁻³): `κ_T` = `c_p` ÷ (`c_v` ρ w²).
fn table_compressibility(density: f64, cv: f64, cp: f64, w: f64) -> f64 {
    cp / (cv * density * w * w)
}

/// Asserts `equation`'s `κ_T` at each (T, ρ, `c_v`, `c_p`, w) of `rows` within `tolerance` of the
/// table's, ρ in kg m⁻³.
fn assert_compressibility(
    name: &str,
    equation: &HelmholtzResidual,
    rows: &[(f64, f64, f64, f64, f64)],
    tolerance: f64,
) {
    for &(t, rho, cv, cp, w) in rows {
        let ours = equation
            .isothermal_compressibility_per_pa(KilogramsPerCubicMetre::new(rho), Kelvin::new(t));
        let table = table_compressibility(rho, cv, cp, w);
        assert!(
            (ours / table - 1.0).abs() < tolerance,
            "{name} at {t} K and {rho} kg/m³: κ_T {ours} against the table's {table}"
        );
    }
}

/// Asserts `equation`'s `c_v` and `c_p`, with its ideal-gas part `ideal`, at each (T, ρ, `c_v`,
/// `c_p`, w) of `rows` within `tolerance` of the table's, ρ in kg m⁻³ and the table's heat
/// capacities times `unit` in J kg⁻¹ K⁻¹ (1,000 for kJ kg⁻¹ K⁻¹ or J g⁻¹ K⁻¹, 1 ÷ M for J mol⁻¹
/// K⁻¹ at M kg mol⁻¹).
fn assert_heat_capacities(
    name: &str,
    equation: &HelmholtzResidual,
    ideal: &HelmholtzIdeal,
    rows: &[(f64, f64, f64, f64, f64)],
    unit: f64,
    tolerance: f64,
) {
    for &(t, rho, cv, cp, _) in rows {
        let (density, temperature) = (KilogramsPerCubicMetre::new(rho), Kelvin::new(t));
        let isochoric = equation.isochoric_heat_capacity_j_per_kg_k(ideal, density, temperature);
        let isobaric = equation.isobaric_heat_capacity_j_per_kg_k(ideal, density, temperature);
        let (table_isochoric, table_isobaric) = (cv * unit, cp * unit);
        assert!(
            (isochoric / table_isochoric - 1.0).abs() < tolerance,
            "{name} at {t} K and {rho} kg/m³: c_v {isochoric} against the table's {table_isochoric}"
        );
        assert!(
            (isobaric / table_isobaric - 1.0).abs() < tolerance,
            "{name} at {t} K and {rho} kg/m³: c_p {isobaric} against the table's {table_isobaric}"
        );
    }
}

/// Setzmann and Wagner 1991's methane reproduces their Table 39's `κ_T` at the saturated liquid
/// (90.694, 100 and 120 K) within 2 × 10⁻⁴, the reach of its five printed figures, and the
/// WebBook's evaluation of the same equation within 10⁻⁶; with its ideal part, its `c_v` and `c_p`
/// to the same tolerances.
#[test]
fn setzmann_and_wagner_s_methane_reproduces_its_table_39() {
    let methane = &super::super::rows::equations::METHANE;
    let table_39 = [
        (90.694, 451.48, 2.1677, 3.3678, 1_538.6),
        (100.0, 438.89, 2.1136, 3.4084, 1_452.0),
        (120.0, 409.90, 2.0196, 3.5493, 1_253.5),
    ];
    assert_compressibility("CH4 Table 39", methane, &table_39, 2e-4);
    assert_heat_capacities(
        "CH4 Table 39",
        methane,
        &METHANE_IDEAL,
        &table_39,
        1e3,
        2e-4,
    );
    let webbook = [
        (
            90.694_057,
            451.475_17,
            2.167_703_4,
            3.367_793_4,
            1_538.602_3,
        ),
        (143.794_26, 369.914_39, 1.934_015_9, 3.889_826_7, 994.196_08),
        (169.726_30, 311.290_15, 1.908_936_5, 5.157_407_2, 661.520_68),
        (184.778_79, 252.685_34, 2.055_606_5, 10.800_571, 403.422_41),
    ];
    assert_compressibility("CH4 WebBook", methane, &webbook, 1e-6);
    assert_heat_capacities("CH4 WebBook", methane, &METHANE_IDEAL, &webbook, 1e3, 1e-6);
}

/// Span et al. 2000's nitrogen reproduces their saturation table's `κ_T` (p. 1403: 63.151, 70 and
/// 77 K) within 3 × 10⁻⁴, the reach of its four printed figures, and the WebBook's within 10⁻⁶;
/// with its ideal part, its `c_v` and `c_p` to the same tolerances.
#[test]
fn span_s_nitrogen_reproduces_its_saturation_table() {
    let nitrogen = &super::super::rows::equations::NITROGEN;
    // The table's densities are mol dm⁻³ and its heat capacities J mol⁻¹ K⁻¹, at 28.013 48 g mol⁻¹.
    let rows: Vec<_> = [
        (63.151, 30.957, 32.95, 56.03, 995.3),
        (70.0, 29.933, 31.65, 56.43, 925.7),
        (77.0, 28.832, 30.43, 57.14, 855.0),
    ]
    .iter()
    .map(|&(t, molar, cv, cp, w)| (t, molar * 28.013_48, cv, cp, w))
    .collect();
    assert_compressibility("N2 table", nitrogen, &rows, 3e-4);
    assert_heat_capacities(
        "N2 table",
        nitrogen,
        &NITROGEN_IDEAL,
        &rows,
        1.0 / 0.028_013_48,
        3e-4,
    );
    let webbook = [
        (63.151_008, 867.221_85, 1.176_258_9, 2.000_226_8, 995.279_57),
        (
            95.845_100,
            713.544_24,
            0.996_506_68,
            2.227_649_1,
            653.867_59,
        ),
        (
            112.654_57,
            599.831_68,
            0.968_592_30,
            2.962_472_8,
            438.314_62,
        ),
        (122.431_11, 486.626_97, 1.061_541_7, 6.321_228_7, 266.829_54),
    ];
    assert_compressibility("N2 WebBook", nitrogen, &webbook, 1e-6);
    assert_heat_capacities("N2 WebBook", nitrogen, &NITROGEN_IDEAL, &webbook, 1e3, 1e-6);
}

/// Span and Wagner 1996's carbon dioxide reproduces their Table 34's `κ_T` at five saturated-liquid
/// states from the triple point to 300 K within 10⁻⁴, and its pressure there at 300 K, 4.1 K below
/// the critical point, where the non-analytic terms bear, within 10⁻⁵; and the WebBook's `κ_T`
/// within 10⁻⁶; with its ideal part, its `c_v` and `c_p` to the same tolerances.
#[test]
fn span_and_wagner_s_carbon_dioxide_reproduces_its_table_34() {
    let co2 = &super::super::rows::equations::CARBON_DIOXIDE;
    let table_34 = [
        (216.592, 1_178.46, 0.974_66, 1.953_2, 975.85),
        (250.0, 1_045.97, 0.936_43, 2.132_0, 731.78),
        (270.0, 945.83, 0.939_59, 2.453_4, 565.46),
        (290.0, 804.67, 0.993_73, 3.675_6, 371.95),
        (300.0, 679.24, 1.119_9, 8.697_9, 245.67),
    ];
    assert_compressibility("CO2 Table 34", co2, &table_34, 1e-4);
    assert_heat_capacities(
        "CO2 Table 34",
        co2,
        &CARBON_DIOXIDE_IDEAL,
        &table_34,
        1e3,
        1e-4,
    );
    let p = co2.pressure(KilogramsPerCubicMetre::new(679.24), Kelvin::new(300.0));
    assert!(
        (p.value() / 6.713_1e6 - 1.0).abs() < 1e-5,
        "{} Pa",
        p.value()
    );
    let webbook = [
        (
            216.591_36,
            1_178.464_9,
            0.974_662_32,
            1.953_205_8,
            975.849_78,
        ),
        (
            247.749_60,
            1_055.943_4,
            0.938_175_99,
            2.110_817_5,
            748.839_86,
        ),
        (
            277.934_57,
            897.446_43,
            0.955_293_81,
            2.717_470_4,
            491.339_37,
        ),
        (296.778_81, 730.095_49, 1.047_227_6, 5.557_621_3, 293.277_23),
    ];
    assert_compressibility("CO2 WebBook", co2, &webbook, 1e-6);
    assert_heat_capacities(
        "CO2 WebBook",
        co2,
        &CARBON_DIOXIDE_IDEAL,
        &webbook,
        1e3,
        1e-6,
    );
}

/// Bücker and Wagner 2006's ethane, its paper not read, reproduces the WebBook's evaluation of the
/// same equation within 3 × 10⁻⁶ from the triple point to 296.7 K, and with its ideal part its
/// `c_v` and `c_p` there to the same tolerance.
#[test]
fn bucker_and_wagner_s_ethane_reproduces_the_webbook() {
    let ethane = &super::super::rows::equations::ETHANE;
    let webbook = [
        (
            90.367_641,
            651.529_88,
            1.605_095_0,
            2.325_817_4,
            2_008.688_1,
        ),
        (234.486_41, 474.258_72, 1.494_489_0, 2.781_626_3, 919.146_46),
        (273.999_22, 398.261_56, 1.623_585_8, 3.654_912_1, 571.043_40),
        (296.663_84, 322.928_00, 1.822_061_4, 7.088_005_0, 322.953_97),
    ];
    assert_compressibility("C2H6 WebBook", ethane, &webbook, 3e-6);
    assert_heat_capacities("C2H6 WebBook", ethane, &ETHANE_IDEAL, &webbook, 1e3, 3e-6);
}

/// Gao et al. 2023's ammonia, its associating terms included, reproduces their Table 7's values for
/// computer verification, pressure and `κ_T`, within 2 × 10⁻⁶ (seven printed figures) at eight
/// liquid, vapour and supercritical states; with its ideal part, its `c_v` and `c_p` there too, in
/// J mol⁻¹ K⁻¹ at their 17.030 52 g mol⁻¹.
#[test]
fn gao_s_ammonia_reproduces_its_table_7() {
    let ammonia = &super::super::rows::equations::AMMONIA;
    let molar_mass = 0.017_030_52;
    for (t, molar, mpa, cv, cp, w) in [
        (200.0, 50.0, 555.563_1, 56.159_76, 74.222_62, 2_940.432),
        (250.0, 42.0, 108.726_3, 50.259_07, 72.047_65, 2_049.168),
        (320.0, 35.0, 27.632_75, 46.958_82, 78.920_22, 1_370.446),
        (380.0, 26.0, 8.174_128, 47.829_65, 125.197_1, 668.545_6),
        (405.0, 16.5, 11.252_87, 77.134_42, 4_137.738, 247.269_6),
        (275.0, 0.2, 0.427_065_2, 32.604_71, 45.396_76, 402.625_3),
        (520.0, 20.0, 56.307_50, 43.815_73, 87.329_96, 741.184_2),
        (620.0, 45.0, 1_355.431, 50.781_24, 64.367_35, 3_201.084),
    ] {
        let rho = KilogramsPerCubicMetre::new(molar * 1e3 * molar_mass);
        let p = ammonia.pressure(rho, Kelvin::new(t)).value() / 1e6;
        assert!(
            (p / mpa - 1.0).abs() < 2e-6,
            "{t} K, {molar} mol/dm³: {p} MPa against {mpa}"
        );
        let kappa = ammonia.isothermal_compressibility_per_pa(rho, Kelvin::new(t));
        let table = table_compressibility(rho.value(), cv, cp, w);
        assert!(
            (kappa / table - 1.0).abs() < 2e-6,
            "{t} K, {molar} mol/dm³: κ_T {kappa} against {table}"
        );
        assert_heat_capacities(
            "NH3 Table 7",
            ammonia,
            &AMMONIA_IDEAL,
            &[(t, rho.value(), cv, cp, w)],
            1.0 / molar_mass,
            2e-6,
        );
    }
}

/// Every non-analytic term meets the condition under which its distance function's derivatives
/// vanish at δ = 1 (β < ½ and a > 1), the limit [`HelmholtzResidual`] takes there.
#[test]
fn every_non_analytic_term_meets_the_limit_s_condition() {
    for (name, equation) in [
        ("IAPWS-95", &IAPWS_95),
        ("CO2", &super::super::rows::equations::CARBON_DIOXIDE),
    ] {
        assert!(!equation.non_analytic.is_empty(), "{name}");
        for term in equation.non_analytic {
            assert!(term.beta < 0.5 && term.a > 1.0, "{name}: {term:?}");
        }
    }
}

/// Asserts `correlation` at each (T, ρ, μ) of `rows` within `tolerance`, μPa s, of the paper's.
fn assert_viscosity(
    name: &str,
    correlation: impl Fn(Kelvin, KilogramsPerCubicMetre) -> f64,
    rows: &[(f64, f64, f64)],
    tolerance: impl Fn(f64) -> f64,
) {
    for &(t, rho, micro) in rows {
        let ours = correlation(Kelvin::new(t), KilogramsPerCubicMetre::new(rho)) * 1e6;
        assert!(
            (ours - micro).abs() <= tolerance(micro),
            "{name} at {t} K and {rho} kg/m³: {ours} μPa s against {micro}"
        );
    }
}

/// Huber, Perkins and Lemmon 2024's nitrogen reproduces their Table 7, the values for computer
/// verification, to its printed 10⁻⁸ μPa s.
#[test]
fn huber_s_nitrogen_viscosity_reproduces_its_table_7() {
    let c = &super::super::rows::liquids::HUBER_2024;
    assert_viscosity(
        "N2",
        |t, rho| c.viscosity_pa_s(t, rho),
        &[
            (90.0, 0.0, 6.071_155_83),
            (90.0, 756.0, 108.425_507_81),
            (300.0, 0.0, 17.834_460_70),
            (300.0, 28.0, 18.234_788_03),
            (300.0, 560.0, 50.596_059_75),
        ],
        |_| 1e-8,
    );
}

/// Sotiriadou et al. 2026's methane reproduces their Sec. 3.2's three points for computer
/// verification to their printed 10⁻⁴ μPa s, and their Table 8's saturated liquid at 100 and 110
/// K to its printed 0.01 μPa s.
#[test]
fn sotiriadou_s_methane_viscosity_reproduces_its_check_values() {
    let c = &super::super::rows::liquids::SOTIRIADOU_2026;
    assert_viscosity(
        "CH4",
        |t, rho| c.viscosity_pa_s(t, rho),
        &[
            (300.0, 0.0, 11.123_0),
            (300.0, 3.2, 11.189_1),
            (300.0, 75.0, 13.713_0),
        ],
        |_| 0.5e-4,
    );
    assert_viscosity(
        "CH4 Table 8",
        |t, rho| c.viscosity_pa_s(t, rho),
        &[(100.0, 438.89, 154.50), (110.0, 424.78, 121.15)],
        |_| 0.005,
    );
}

/// Laesecke and Muzny 2017's carbon dioxide reproduces their Table 6 to half a unit of its fifth
/// significant figure.
#[test]
fn laesecke_and_muzny_s_carbon_dioxide_viscosity_reproduces_its_table_6() {
    let c = &super::super::rows::liquids::LAESECKE_MUZNY_2017;
    // Table 6 in mPa s, here in μPa s.
    assert_viscosity(
        "CO2",
        |t, rho| c.viscosity_pa_s(t, rho),
        &[
            (100.0, 0.0, 5.375_7),
            (2_000.0, 0.0, 66.079),
            (10_000.0, 0.0, 176.20),
            (220.0, 3.0, 11.104),
            (225.0, 1_150.0, 222.18),
            (300.0, 65.0, 15.563),
            (300.0, 1_400.0, 505.94),
            (700.0, 100.0, 33.112),
            (700.0, 1_200.0, 229.80),
        ],
        |micro: f64| 0.5 * math::powf(10.0, math::log10(micro).floor() - 4.0),
    );
}

/// Monogenidou, Assael and Huber 2018's ammonia reproduces their three points for computer
/// verification to their printed 10⁻⁴ μPa s, their Table 6's saturated liquid within 10⁻⁴, the
/// reach of its five-figure densities, and the WebBook's evaluation of it within 2 × 10⁻⁷.
#[test]
fn monogenidou_s_ammonia_viscosity_reproduces_its_check_values() {
    let c = &super::super::rows::liquids::MONOGENIDOU_2018;
    let viscosity = |t, rho| c.viscosity_pa_s(t, rho);
    assert_viscosity(
        "NH3",
        viscosity,
        &[
            (300.0, 0.0, 10.181_2),
            (300.0, 8.0, 9.921_9),
            (300.0, 609.0, 133.393_7),
        ],
        |_| 0.5e-4,
    );
    assert_viscosity(
        "NH3 Table 6",
        viscosity,
        &[
            (200.0, 728.67, 516.02),
            (300.0, 600.17, 125.00),
            (380.0, 436.25, 53.95),
        ],
        |micro| 1e-4 * micro,
    );
    assert_viscosity(
        "NH3 WebBook",
        viscosity,
        &[
            (195.492_05, 733.852_06, 570.578_73),
            (320.545_21, 567.410_41, 102.667_61),
            (368.330_58, 469.841_69, 62.680_844),
            (395.311_38, 374.023_50, 41.807_403),
        ],
        |micro| 2e-7 * micro,
    );
}

/// Friend, Ingham and Ely 1991's ethane reproduces their Table A2's saturated liquid within 3 ×
/// 10⁻³, the reach of its four-figure densities (in mol dm⁻³ at 30.070 g mol⁻¹), whose effect on
/// the dense liquid's viscosity is about tenfold.
#[test]
fn friend_s_ethane_viscosity_reproduces_its_table_a2() {
    let c = &super::super::rows::liquids::FRIEND_1991;
    let rows: Vec<_> = [
        (94.0, 21.54, 1_099.01),
        (100.0, 21.32, 876.96),
        (150.0, 19.47, 270.35),
        (200.0, 17.42, 138.17),
    ]
    .iter()
    .map(|&(t, molar, micro)| (t, molar * 30.070, micro))
    .collect();
    assert_viscosity(
        "C2H6 Table A2",
        |t, rho| c.viscosity_pa_s(t, rho),
        &rows,
        |micro| 3e-3 * micro,
    );
}

/// Each correlation is `Measured` inside its stated range and `Derived` outside it and inside its
/// critical box: ethane's inside Friend's 90–500 K and outside it above 500 K, nitrogen's within 3%
/// of its critical temperature and 25% of its density, carbon dioxide's in Vesovic et al.'s 300–310
/// K and 300–600 kg m⁻³, and methane's and ammonia's in the reduced box.
#[test]
fn the_viscosities_are_derived_outside_their_ranges() {
    let rows = &super::super::rows::liquids::LIQUIDS;
    let correlation = |key: &str| {
        rows.iter()
            .find(|(k, _)| k.as_str() == key)
            .and_then(|(_, columns)| columns.viscosity)
            .unwrap()
    };
    let basis = |key: &str, t: f64, rho: f64| {
        correlation(key)
            .viscosity_pa_s(Kelvin::new(t), KilogramsPerCubicMetre::new(rho))
            .1
    };
    assert_eq!(basis("C2H6", 94.0, 647.7), LiquidBasis::Measured);
    assert_eq!(basis("C2H6", 520.0, 100.0), LiquidBasis::Derived);
    assert_eq!(basis("N2", 77.0, 807.7), LiquidBasis::Measured);
    assert_eq!(basis("N2", 125.0, 313.3), LiquidBasis::Derived);
    assert_eq!(basis("CH4", 190.0, 162.66), LiquidBasis::Derived);
    assert_eq!(basis("NH3", 300.0, 600.2), LiquidBasis::Measured);
    assert_eq!(basis("NH3", 405.0, 233.3), LiquidBasis::Derived);
    assert_eq!(basis("CO2", 302.0, 450.0), LiquidBasis::Derived);
    assert_eq!(basis("CO2", 302.0, 650.0), LiquidBasis::Measured);
    assert_eq!(basis("CO2", 250.0, 1_046.0), LiquidBasis::Measured);
}

/// The correlation length reproduces Huber, Perkins and Lemmon 2024's Table 8 for nitrogen near
/// its critical point within 10⁻⁸, from Span et al.'s `κ_T` with their amplitudes (`ξ_0` = 0.16 nm,
/// `Γ_0` = 0.075, `T_R` = 1.5 `T_c`).
#[test]
fn the_correlation_length_reproduces_huber_s_table_8() {
    let scattering = by_key("N2")
        .and_then(|id| id.substance().liquid())
        .and_then(LiquidColumns::scattering)
        .unwrap();
    for (t, rho, nm) in [
        (126.192, 265.0, 4.243_797_93),
        (126.212, 333.0, 13.960_741_61),
        (126.952, 300.0, 3.388_641_20),
    ] {
        let xi = scattering.fluctuations().correlation_length_m(
            scattering.equation(),
            KilogramsPerCubicMetre::new(rho),
            Kelvin::new(t),
        ) * 1e9;
        assert!(
            (xi / nm - 1.0).abs() < 1e-8,
            "{t} K, {rho} kg/m³: ξ {xi} nm against {nm}"
        );
    }
}

/// b is `Derived` where `q_max` ξ passes 0.1, near a critical point, and keeps its basis away from
/// one ([`LiquidScattering::basis_at`]): water's scattering is `Measured` at 25 °C and 600 K and
/// `Derived` within 0.1 K of 647.096 K; nitrogen's keeps its own basis at 77 K and is `Derived` at
/// 126.19 K; and a record takes the rule.
#[test]
fn the_scattering_is_derived_near_a_critical_point() {
    let basis = |key: &str, t: f64| {
        let id = row(key);
        let record = liquid_properties(id, Kelvin::new(t), Composition::Reference).unwrap();
        let scattering = id.substance().liquid().unwrap().scattering().unwrap();
        scattering.basis_at(
            record.refractive_index[0],
            KilogramsPerCubicMetre::new(record.density_kg_m3),
            Kelvin::new(t),
        )
    };
    assert_eq!(basis("H2O", 298.15), LiquidBasis::Measured);
    assert_eq!(basis("H2O", 600.0), LiquidBasis::Measured);
    assert_eq!(basis("H2O", 647.0), LiquidBasis::Derived);
    let n2 = row("N2")
        .substance()
        .liquid()
        .unwrap()
        .scattering()
        .unwrap()
        .basis();
    assert_eq!(basis("N2", 77.0), n2);
    assert_eq!(basis("N2", 126.19), LiquidBasis::Derived);
    // Every record at a state where the rule fires is `Derived` by its other members too (water's
    // index window ends at 373.15 K; the other rows' b is `Derived`), so the record shows the rule
    // only through this method.
    assert_eq!(
        liquid_properties(row("H2O"), Kelvin::new(647.0), Composition::Reference)
            .unwrap()
            .basis,
        LiquidBasis::Derived
    );
}

/// A surface tension the correlation gives as negative is absent, not written: methane's Mulero
/// sum falls below 0 within about 0.18 K of its critical point, sulphur dioxide's three terms' at
/// 417.55 K, 13.1 K below its 430.64 K, and xenon's two at 289.611 K, 0.12 K below its 289.733 K;
/// each is present a little cooler.
#[test]
fn a_negative_surface_tension_is_absent() {
    for (key, present, absent) in [
        ("CH4", 189.564, 190.5),
        ("SO2", 417.5, 417.6),
        ("Xe", 289.6, 289.62),
    ] {
        let tension = |t: f64| {
            liquid_properties(row(key), Kelvin::new(t), Composition::Reference)
                .unwrap()
                .surface_tension_n_per_m
        };
        let none = tension(absent);
        assert!(none.is_none(), "{key} at {absent} K: {none:?}");
        let below = tension(present).unwrap();
        assert!(below > 0.0, "{key}: {below} N/m at {present} K");
    }
}

/// Water's anisotropy ratio is the closed form its doc states, x = 45 `ρ_v` `S_0` ÷ (3 − 4`ρ_v`)
/// with `ρ_v` = 0.039 ÷ (2 − 0.039) and `S_0` from IAPWS-95 at the record's saturated density at
/// 298.15 K, to the bit.
#[test]
fn water_s_anisotropy_ratio_is_its_closed_form() {
    let t = Kelvin::new(298.15);
    let water = row("H2O");
    let scattering = water.substance().liquid().unwrap().scattering().unwrap();
    let density = water
        .substance()
        .grain_density(CondensedPhase::Liquid, t)
        .unwrap();
    let s0 = scattering.structure_factor(density, t);
    let vertical = 0.039 / (2.0 - 0.039);
    let closed = 45.0 * vertical * s0 / (3.0 - 4.0 * vertical);
    assert_same_bits(scattering.anisotropy_ratio(), closed);
}

/// The depolarisation follows the state from each row's anisotropy ratio
/// (decision-p14-t49h-gas-index §4.2–§4.3): water's gives 0.039 at 298.15 K, falling towards the
/// critical point; methane's about 0.025, nitrogen's 0.38, carbon dioxide's 0.46, ethane's 0.29 and
/// ammonia's 0.07 at their triple points, within 5% of the ruling's figures (which take the
/// WebBook's `S_0`), and nitrogen's about 0.25 at 77 K.
#[test]
fn the_depolarisation_follows_the_state() {
    let delta = |key: &str, columns: &LiquidColumns, t: f64| {
        let id = row(key);
        let density = id
            .substance()
            .grain_density(CondensedPhase::Liquid, Kelvin::new(t))
            .unwrap();
        columns
            .scattering()
            .unwrap()
            .depolarisation_ratio(density, Kelvin::new(t))
    };
    let water = row("H2O").substance().liquid().unwrap();
    let at_25 = delta("H2O", water, 298.15);
    assert!((at_25 - 0.039).abs() < 1e-9, "δ {at_25} at 298.15 K");
    let at_triple = delta("H2O", water, 273.16);
    assert!(at_triple < 0.039, "δ {at_triple} at the triple point");
    let at_100 = delta("H2O", water, 373.15);
    assert!(at_100 < 0.031, "δ {at_100} at 373.15 K");
    for (key, ruled) in [
        ("CH4", 0.025),
        ("N2", 0.38),
        ("CO2", 0.46),
        ("C2H6", 0.29),
        ("NH3", 0.07),
    ] {
        let columns = row(key).substance().liquid().unwrap();
        let triple = row(key)
            .substance()
            .phase_diagram()
            .unwrap()
            .triple_temperature();
        let ours = delta(key, columns, triple.value());
        assert!(
            (ours / ruled - 1.0).abs() < 0.05,
            "{key}: δ {ours} against {ruled}"
        );
    }
    let n2 = row("N2").substance().liquid().unwrap();
    let at_77 = delta("N2", n2, 77.0);
    assert!((at_77 - 0.25).abs() < 0.015, "δ {at_77} at 77 K");
}

/// Water's b at 20 °C lies within 6% of Morel 1974's pure-water column, read through Twardowski et
/// al. 2007's Table 2 (0.00350 m⁻¹ at 450 nm and 0.00149 at 550 nm, at δ = 0.09, falling as
/// λ^−4.32), rescaled to δ = 0.039 by f(0.039) ÷ f(0.09) = 0.9148, with f(δ) = (6 + 6δ) ÷ (6 − 7δ)
/// × (2 + δ) ÷ (1 + δ), in the bins across 443 and 545 nm, each oracle Morel's law's mean over its
/// bin from the nearer of his two values (decision-p14-t49h-liquids §7(d) and §9: about 4.9% and
/// 3.4% below it).
#[test]
fn water_s_scattering_is_within_six_percent_of_morel_s_pure_water() {
    let f = |d: f64| (6.0 + 6.0 * d) / (6.0 - 7.0 * d) * (2.0 + d) / (1.0 + d);
    let rescale = f(0.039) / f(0.09);
    assert!(
        (rescale - 0.914_8).abs() < 5e-5,
        "f(0.039) ÷ f(0.09) = {rescale}"
    );
    let b = liquid_properties(row("H2O"), Kelvin::new(293.15), Composition::Reference)
        .unwrap()
        .scattering_per_m
        .unwrap();
    for (k, morel, at_nm) in [(2, 0.003_50, 450.0), (6, 0.001_49, 550.0)] {
        let law = simpson_mean(
            |nm| morel * math::powf(nm / at_nm, -4.32),
            LIQUID_BIN_EDGES_NM[k],
            LIQUID_BIN_EDGES_NM[k + 1],
        );
        let oracle = law * rescale;
        let miss = b[k] / oracle - 1.0;
        assert!(
            miss.abs() < 0.06,
            "bin {k}: {} m⁻¹ against {oracle} ({miss:+.3})",
            b[k]
        );
    }
}

/// Three saturated-liquid states, each its temperature, K, and the WebBook's surface tension there,
/// N m⁻¹.
type TensionRows = [(f64, f64); 3];

/// The WebBook's saturated-liquid surface tensions by row, with the tolerance
/// [`mulero_s_surface_tensions_reproduce_the_webbook`] holds each to.
const MULERO_WEBBOOK: [(&str, f64, TensionRows); 13] = [
    (
        "CH4",
        2e-6,
        [
            (90.694_057, 0.017_062_798),
            (143.794_26, 0.006_878_156_0),
            (169.726_3, 0.002_559_923_6),
        ],
    ),
    (
        "N2",
        2e-6,
        [
            (63.151_008, 0.012_205_148),
            (95.845_1, 0.004_908_255_9),
            (112.654_57, 0.001_795_175_3),
        ],
    ),
    (
        "C2H6",
        2e-6,
        [
            (90.367_641, 0.031_663_374),
            (234.486_41, 0.008_534_154_2),
            (273.999_22, 0.003_122_508_5),
        ],
    ),
    (
        "NH3",
        2e-6,
        [
            (195.492_05, 0.043_947_790),
            (320.545_21, 0.015_481_965),
            (368.330_58, 0.005_701_248_7),
        ],
    ),
    (
        "CO2",
        2e-6,
        [
            (216.591_36, 0.016_494_627),
            (247.749_6, 0.009_500_193_8),
            (277.934_57, 0.003_632_893_5),
        ],
    ),
    (
        "O2",
        1e-10,
        [
            (54.37, 0.022_598_286_314_3),
            (102.93, 0.010_034_034_642_0),
            (127.21, 0.004_609_302_498_89),
        ],
    ),
    (
        "Ar",
        1e-10,
        [
            (83.81, 0.013_403_029_379_7),
            (115.74, 0.005_954_829_973_37),
            (131.705, 0.002_776_735_112_36),
        ],
    ),
    (
        "Kr",
        1e-10,
        [
            (115.78, 0.016_417_265_440_4),
            (160.535, 0.007_314_285_425_45),
            (182.912_5, 0.003_418_232_802_84),
        ],
    ),
    (
        "Xe",
        1e-10,
        [
            (161.41, 0.018_858_632_201_1),
            (222.675, 0.008_506_493_471_75),
            (253.307_5, 0.003_985_122_152_57),
        ],
    ),
    (
        "N2O",
        1e-10,
        [
            (182.34, 0.024_288_096_397_5),
            (242.835, 0.011_163_570_440_8),
            (273.082_5, 0.005_392_345_350_81),
        ],
    ),
    (
        "CO",
        1e-10,
        [
            (68.16, 0.012_446_245_900_0),
            (99.18, 0.005_882_249_127_06),
            (114.69, 0.002_896_407_873_72),
        ],
    ),
    (
        "SO2",
        1e-10,
        [
            (197.7, 0.041_582_250_355_2),
            (309.865, 0.019_391_208_080_3),
            (365.947_5, 0.008_817_895_456_54),
        ],
    ),
    (
        "H2S",
        1e-10,
        [
            (187.7, 0.033_765_934_367_6),
            (276.67, 0.015_335_602_898_4),
            (321.155, 0.007_266_254_562_94),
        ],
    ),
];

/// Mulero et al. 2012's surface tensions, with each reference equation's critical temperature,
/// reproduce the WebBook's saturated-liquid column at three states each: step 2's within 2 × 10⁻⁶,
/// and step 2b's eight, from their triple points to about 0.85 `T_c`, within 10⁻¹⁰ (the research
/// found 4.5 × 10⁻¹², the WebBook's twelve printed figures), sulphur dioxide's at its row's
/// critical point, which is Gao et al.'s.
#[test]
fn mulero_s_surface_tensions_reproduce_the_webbook() {
    for (key, tolerance, rows) in MULERO_WEBBOOK {
        let tension = row(key)
            .substance()
            .liquid()
            .and_then(LiquidColumns::surface_tension)
            .unwrap();
        for (t, webbook) in rows {
            let ours = tension.surface_tension_n_per_m(Kelvin::new(t));
            assert!(
                (ours / webbook - 1.0).abs() < tolerance,
                "{key} at {t} K: {ours} N/m against {webbook}"
            );
        }
    }
}

/// Carbon dioxide's record resolves now that its index has landed (decision-p14-t49h-gas-index
/// §1.2): at its triple point its index is its file's carried 0.008 K down, the record is
/// `Derived` (the index's basis, its file's provenance), and b, σ and μ are present, μ about 0.253
/// mPa s (the WebBook's 253.37 μPa s at its own density) and `Measured`.
#[test]
fn carbon_dioxide_resolves_with_its_index() {
    let co2 = row("CO2");
    let columns = co2.substance().liquid().unwrap();
    assert_eq!(columns.components(), [co2]);
    let optics = columns.optics().unwrap();
    assert_eq!(optics.index_basis(), LiquidBasis::Derived);
    assert!(
        optics.absorption_per_m().is_none(),
        "{:?}",
        optics.absorption_per_m()
    );
    let t = Kelvin::new(216.592);
    let record = liquid_properties(co2, t, Composition::Reference).unwrap();
    assert_eq!(record.basis, LiquidBasis::Derived);
    assert!(
        record.absorption_per_m.is_none(),
        "{:?}",
        record.absorption_per_m
    );
    for (n, file) in record
        .refractive_index
        .iter()
        .zip(&bins::CARBON_DIOXIDE_LIQUID_N)
    {
        assert!(n > file && n - file < 1e-5, "{n} against the file's {file}");
    }
    assert!(record.scattering_per_m.is_some());
    let rho = KilogramsPerCubicMetre::new(record.density_kg_m3);
    let (mu, basis) = columns.viscosity().unwrap().viscosity_pa_s(t, rho);
    assert_same_bits(record.viscosity_pa_s.unwrap(), mu);
    assert!((mu * 1e6 - 253.4).abs() < 2.0, "{mu}");
    assert_eq!(basis, LiquidBasis::Measured);
    let sigma = record.surface_tension_n_per_m.unwrap();
    assert!(sigma > 0.016, "{sigma} N/m");
}

/// Methane's and ethane's viscosities continue below their triple points by their fitted
/// Vogel–Fulcher–Tammann forms (decision-p14-t49b-condensed §2.5), continuous there: at the triple
/// point the form is the correlation's value, to the bit; a kelvin below, within 1% of the
/// correlation's own slope carried down; and it rises steadily to Titan's first melt, 71.15 K,
/// where the correlation itself, at the extended density, would be past its pole (Sotiriadou et
/// al. 2026's Eq. 6 has one where `ρ_r` = f₃ + `T_r`, inside the solid).
#[test]
fn the_supercooled_viscosities_are_continuous_at_the_triple_point() {
    for key in ["CH4", "C2H6"] {
        let id = row(key);
        let columns = id.substance().liquid().unwrap();
        let vft = columns.supercooled_viscosity().unwrap();
        let triple = id.substance().phase_diagram().unwrap().triple_temperature();
        assert_same_bits(vft.triple_temperature().value(), triple.value());
        let at_triple = liquid_properties(id, triple, Composition::Reference)
            .unwrap()
            .viscosity_pa_s
            .unwrap();
        assert_same_bits(vft.viscosity_pa_s(at_triple, triple), at_triple);
        let h = 1e-4;
        let above = liquid_properties(id, Kelvin::new(triple.value() + h), Composition::Reference)
            .unwrap()
            .viscosity_pa_s
            .unwrap();
        let slope = (math::ln(above) - math::ln(at_triple)) / h;
        let below = vft.viscosity_pa_s(at_triple, Kelvin::new(triple.value() - 1.0));
        let carried = at_triple * math::exp(-slope);
        assert!(
            (below / carried - 1.0).abs() < 0.01,
            "{key}: {below} Pa s a kelvin below against the slope's {carried}"
        );
        let mut previous = at_triple;
        for k in 1..=20 {
            let t = triple.value() - (triple.value() - 71.15) * f64::from(k) / 20.0;
            let mu = vft.viscosity_pa_s(at_triple, Kelvin::new(t));
            assert!(mu.is_finite() && mu > previous, "{key} at {t} K: {mu}");
            previous = mu;
        }
        let t0 = vft.divergence_temperature().value();
        assert!((0.0..71.15).contains(&t0), "{key}: T_0 {t0} K");
    }
}

/// The testing worlds' water and methane records carry the registry's scattering, to the bit
/// (decision-r10-t10b-r09-t2e §7, condition 1): `hyperion_surface::testing`'s copies, as shortest
/// round-trip decimals, equal `liquid_properties`' b at each record's temperature.
#[cfg(feature = "testing")]
#[test]
fn the_testing_worlds_carry_the_registry_s_scattering() {
    use hyperion_surface::testing::{liquid_methane_titan, liquid_water};
    for (record, key) in [(liquid_water(), "H2O"), (liquid_methane_titan(), "CH4")] {
        let resolved =
            liquid_properties(row(key), record.temperature, Composition::Reference).unwrap();
        let held = record.scattering_per_m.unwrap();
        for (h, r) in held.iter().zip(&resolved.scattering_per_m.unwrap()) {
            assert_same_bits(*h, *r);
        }
    }
}

/// A saturation table's states: T, K; ρ′, kg m⁻³; `c_v` and `c_p`, J g⁻¹ K⁻¹; and w, m s⁻¹.
type SaturationRows = [(f64, f64, f64, f64, f64); 4];

/// Step 2b's equations with the WebBook's saturated-liquid states for each (NIST SRD 69, fetched
/// 2026-10-19), and whether the equation is Lemmon and Span 2006's, which the WebBook evaluates
/// with CODATA 2014's R.
const GAS_DERIVED_WEBBOOK: [(&str, &HelmholtzResidual, bool, SaturationRows); 7] = [
    (
        "O2",
        &super::super::rows::equations::OXYGEN,
        false,
        [
            (
                54.37,
                1_306.040_101_27,
                1.195_126_187_67,
                1.673_186_515,
                1_123.431_731_36,
            ),
            (
                78.65,
                1_196.829_212_5,
                0.975_586_615_194,
                1.680_515_211_79,
                998.306_150_931,
            ),
            (
                102.93,
                1_075.158_208_79,
                0.885_768_414_758,
                1.754_062_559_92,
                797.045_159_464,
            ),
            (
                127.21,
                923.696_351_177,
                0.831_954_464_448,
                2.073_272_517_9,
                569.085_279_123,
            ),
        ],
    ),
    (
        "Ar",
        &super::super::rows::equations::ARGON,
        false,
        [
            (
                83.81,
                1_416.741_313_67,
                0.549_587_539_271,
                1.115_690_370_39,
                862.398_158_74,
            ),
            (
                99.775,
                1_315.216_188_93,
                0.498_183_953_14,
                1.152_703_109_71,
                748.588_640_142,
            ),
            (
                115.74,
                1_198.251_073_03,
                0.464_086_275_26,
                1.274_772_760_46,
                621.489_771_381,
            ),
            (
                131.705,
                1_049.734_646_33,
                0.449_157_697_443,
                1.627_747_118_68,
                469.818_910_639,
            ),
        ],
    ),
    (
        "Kr",
        &super::super::rows::equations::KRYPTON,
        true,
        [
            (
                115.78,
                2_446.601_975_35,
                0.253_283_459_567,
                0.517_129_910_363,
                698.925_135_62,
            ),
            (
                138.157_5,
                2_269.432_253_21,
                0.234_670_868_639,
                0.542_510_399_411,
                604.150_646_134,
            ),
            (
                160.535,
                2_065.332_701_81,
                0.221_645_888_679,
                0.604_607_834_497,
                501.116_715_438,
            ),
            (
                182.912_5,
                1_806.830_780_92,
                0.216_700_104_589,
                0.775_433_945_568,
                379.253_677_591,
            ),
        ],
    ),
    (
        "Xe",
        &super::super::rows::equations::XENON,
        true,
        [
            (
                161.41,
                2_966.150_426_69,
                0.168_042_680_643,
                0.338_526_172_041,
                653.349_831_976,
            ),
            (
                192.042_5,
                2_753.370_638_63,
                0.154_214_135_043,
                0.352_444_740_603,
                565.013_173_635,
            ),
            (
                222.675,
                2_507.905_773_81,
                0.144_748_626_797,
                0.390_789_226_688,
                468.558_333_288,
            ),
            (
                253.307_5,
                2_196.402_146_9,
                0.140_857_034_091,
                0.498_770_900_504,
                354.626_968_096,
            ),
        ],
    ),
    (
        "CO",
        &super::super::rows::equations::CARBON_MONOXIDE,
        true,
        [
            (
                68.16,
                849.538_097_042,
                1.262_082_095_84,
                2.157_441_259_37,
                998.196_602_86,
            ),
            (
                83.67,
                784.267_282_701,
                1.127_649_069_33,
                2.152_353_371_86,
                847.223_503_193,
            ),
            (
                99.18,
                709.749_112_933,
                1.040_486_122_15,
                2.292_605_345_92,
                687.547_911_725,
            ),
            (
                114.69,
                615.854_956_374,
                0.993_488_141_969,
                2.774_902_129_93,
                506.095_727_718,
            ),
        ],
    ),
    (
        "N2O",
        &super::super::rows::equations::NITROUS_OXIDE,
        true,
        [
            (
                182.34,
                1_237.321_619_04,
                0.960_965_601_087,
                1.717_753_299_63,
                1_149.532_582_98,
            ),
            (
                212.587_5,
                1_144.655_023_33,
                0.928_737_450_79,
                1.765_159_560_72,
                956.262_382_338,
            ),
            (
                242.835,
                1_038.952_148_28,
                0.912_481_116_73,
                1.901_485_957_31,
                762.992_517_528,
            ),
            (
                273.082_5,
                907.409_615_835,
                0.911_144_708_916,
                2.271_262_471_68,
                559.004_946_222,
            ),
        ],
    ),
    (
        "H2S",
        &super::super::rows::equations::HYDROGEN_SULPHIDE,
        true,
        [
            (
                187.7,
                992.310_041_096,
                1.302_479_201_15,
                2.019_750_265_76,
                1_437.845_137_88,
            ),
            (
                232.185,
                914.426_617_583,
                1.180_439_742,
                2.000_316_227_14,
                1_203.987_765_68,
            ),
            (
                276.67,
                825.410_242_74,
                1.103_745_226_04,
                2.110_944_332_23,
                961.008_972_268,
            ),
            (
                321.155,
                712.230_368_033,
                1.063_863_128_52,
                2.528_856_735_9,
                692.699_216_883,
            ),
        ],
    ),
];

/// Step 2b's equations of state reproduce the WebBook's evaluation of the same equations (NIST SRD
/// 69's saturation tables at twelve figures, fetched 2026-10-19, their SHA-256 in plan 14's
/// P14.T49.h step 2b record): `κ_T` against `c_p` ÷ (`c_v` ρ′ w²) at four saturated-liquid states
/// each, from the triple point to about 0.85 `T_c`. Oxygen's and argon's within 10⁻⁶, the WebBook
/// taking their own gas constants. Lemmon and Span 2006's five within 2 × 10⁻⁶: the WebBook
/// evaluates them with CODATA 2014's R = 8.314 459 8 J mol⁻¹ K⁻¹, not the 8.314 472 J mol⁻¹ K⁻¹
/// their coefficients were fitted with, which this crate keeps, so its `κ_T` lies a constant
/// 1.47 × 10⁻⁶ above theirs; scaled by the ratio of the two R, they meet it within 10⁻⁹.
#[test]
fn the_gas_derived_rows_equations_reproduce_the_webbook() {
    let codata_2014_ratio = 8.314_472 / 8.314_459_8;
    for (name, equation, lemmon_and_span, rows) in GAS_DERIVED_WEBBOOK {
        if lemmon_and_span {
            assert_compressibility(name, equation, &rows, 2e-6);
            for (t, rho, cv, cp, w) in rows {
                let ours = equation.isothermal_compressibility_per_pa(
                    KilogramsPerCubicMetre::new(rho),
                    Kelvin::new(t),
                ) * codata_2014_ratio;
                let table = table_compressibility(rho, cv, cp, w);
                assert!(
                    (ours / table - 1.0).abs() < 1e-9,
                    "{name} at {t} K with the WebBook's R: κ_T {ours} against {table}"
                );
            }
        } else {
            assert_compressibility(name, equation, &rows, 1e-6);
        }
    }
}

/// At its file's own state each of step 2b's nine rows is its file's bins, within 4 × 10⁻¹⁶, as
/// water's is: the carry's ratio of densities is exactly 1 there.
#[test]
fn the_gas_derived_rows_files_are_reproduced_at_their_states() {
    for (key, state) in GAS_DERIVED_FILES {
        let id = row(key);
        let optics = id.substance().liquid().unwrap().optics().unwrap();
        let record = liquid_properties(id, Kelvin::new(state), Composition::Reference).unwrap();
        for (n, file) in record.refractive_index.iter().zip(optics.index()) {
            assert!((n - file).abs() < 4e-16, "{key}: {n} against {file}");
        }
    }
}

/// The atoms' indices, Sinnock and Smith's measured tables, keep their basis only over their
/// checked windows, each from its file's state (`hyperion-fit`'s
/// `liquid_optics_the_atoms_index_windows_are_sinnock_and_smith_s` derives them from the files
/// and the tables): argon's 83.81–90 K, krypton's 115.77–115.95 K and xenon's 161.4 K alone. The
/// other six of step 2b's rows have no window, their index `Derived` throughout (carbon monoxide's
/// and hydrogen sulphide's files `standIn`, the record's `Derived`, decision-p14-t49h-gas-index
/// A.4).
#[test]
fn the_atoms_alone_have_index_windows() {
    let optics = |key: &str| row(key).substance().liquid().unwrap().optics().unwrap();
    let window = |key: &str| {
        optics(key)
            .index_checked()
            .map(|(low, high)| (low.value(), high.value()))
    };
    for (key, checked) in [
        ("Ar", (83.81, 90.0)),
        ("Kr", (115.77, 115.95)),
        ("Xe", (161.4, 161.4)),
    ] {
        let (low, high) = window(key).unwrap_or_else(|| panic!("{key} has an index window"));
        assert_same_bits(low, checked.0);
        assert_same_bits(high, checked.1);
        assert_eq!(optics(key).index_basis(), LiquidBasis::Measured, "{key}");
        assert_same_bits(checked.0, optics(key).state().value());
    }
    for key in ["O2", "CO2", "N2O", "CO", "SO2", "H2S"] {
        let none = window(key);
        assert!(none.is_none(), "{key}: {none:?}");
        assert_eq!(optics(key).index_basis(), LiquidBasis::Derived, "{key}");
    }
}

/// Step 2b's anisotropy ratios are the lane's research's (decision-p14-t49h-gas-index §4.3;
/// `.git/rm45-scratch/p14-liquids/step2b/step2b.json`) within 10⁻⁸ relative, and δ at each triple
/// point at the record's density within 10⁻⁷ (carbon dioxide's 1.1 × 10⁻⁸, the research's
/// equation evaluated apart): oxygen's x 0.466 and δ 0.706, nitrous oxide's 1.013 and
/// 0.665, carbon monoxide's 0.0825 and 0.230 and hydrogen sulphide's 0.006 70 and 0.0252, each its
/// gas's King factor's molecular part plus 0.006; argon's 0.003 93 and 0.0109, krypton's 0.006 39
/// and 0.0171 and xenon's 0.010 16 and 0.0270, their interaction part alone, a stand-in; and
/// carbon dioxide's step 2 value, 0.657 and 0.465.
#[test]
fn the_gas_derived_rows_anisotropy_is_the_research_s() {
    for (key, x, delta) in [
        ("O2", 0.465_724_130_865, 0.706_178_953_819),
        ("CO2", 0.656_564_174_894, 0.465_146_714_766),
        ("Ar", 0.003_931_579_195_82, 0.010_866_598_497_8),
        ("Kr", 0.006_393_949_283_6, 0.017_078_665_293),
        ("Xe", 0.010_155_781_107_9, 0.026_960_559_563_8),
        ("N2O", 1.012_696_876_09, 0.665_497_134_243),
        ("CO", 0.082_5, 0.229_624_764_876),
        ("H2S", 0.006_697_581_314_88, 0.025_174_699_444_6),
    ] {
        let substance = row(key).substance();
        let scattering = substance.liquid().unwrap().scattering().unwrap();
        let ours = scattering.anisotropy_ratio();
        assert!((ours / x - 1.0).abs() < 1e-8, "{key}: x {ours} against {x}");
        let triple = substance.phase_diagram().unwrap().triple_temperature();
        let density = substance
            .grain_density(CondensedPhase::Liquid, triple)
            .unwrap();
        let ours = scattering.depolarisation_ratio(density, triple);
        assert!(
            (ours / delta - 1.0).abs() < 1e-7,
            "{key}: δ {ours} against {delta} at its triple point"
        );
    }
}

/// Step 2b's eight rows beside carbon dioxide have no viscosity, none being ruled for them
/// (decision-p14-t49h-gas-index §1.2), and sulphur dioxide no scattering, its depolarisation unread
/// (§4.3); each record resolves mid-range all the same, `Derived`.
#[test]
fn the_gas_derived_rows_unsourced_members_are_absent() {
    for key in ["O2", "Ar", "Kr", "Xe", "N2O", "CO", "SO2", "H2S"] {
        let id = row(key);
        let columns = id.substance().liquid().unwrap();
        assert!(columns.viscosity().is_none(), "{key}");
        assert_eq!(columns.scattering().is_some(), key != "SO2", "{key}");
        let (lowest, top) = reference_range(id);
        let t = Kelvin::new(f64::midpoint(lowest, top));
        let record = liquid_properties(id, t, Composition::Reference).unwrap();
        assert!(
            record.viscosity_pa_s.is_none(),
            "{key}: {:?}",
            record.viscosity_pa_s
        );
        assert_eq!(record.scattering_per_m.is_some(), key != "SO2", "{key}");
        assert!(record.surface_tension_n_per_m.is_some(), "{key}");
        assert_eq!(record.basis, LiquidBasis::Derived, "{key}");
    }
}

/// The critical-point rule acts on step 2b's rows where the lane's research found it: `q_max` ξ,
/// `q_max` = 4π n ÷ 380 nm with the record's index in the shortest bin and ξ from each row's
/// amplitudes (oxygen's and argon's Lemmon and Jacobsen 2004's, xenon's Velliadou et al. 2021's,
/// the others' R12-08's water amplitudes), passes [`CRITICAL_SCATTERING_LIMIT`] on the saturated
/// liquid 1.17 K below `T_c` for oxygen, 0.41 K for argon, 0.73 K for krypton, 1.66 K for xenon,
/// 0.71 K for nitrous oxide, 0.33 K for carbon monoxide and 1.06 K for hydrogen sulphide: below it
/// 0.01 K cooler, above it 0.01 K warmer.
#[test]
fn the_gas_derived_rows_critical_rule_acts_where_the_research_found() {
    for (key, crossing) in [
        ("O2", 153.407_483_177),
        ("Ar", 150.272_555_562),
        ("Kr", 208.750_049_783),
        ("Xe", 288.077_143_334),
        ("N2O", 308.813_103_829),
        ("CO", 132.533_774_696),
        ("H2S", 372.038_140_693),
    ] {
        let id = row(key);
        let scattering = id.substance().liquid().unwrap().scattering().unwrap();
        let q_xi = |t: f64| {
            let record = liquid_properties(id, Kelvin::new(t), Composition::Reference).unwrap();
            let xi = scattering.fluctuations().correlation_length_m(
                scattering.equation(),
                KilogramsPerCubicMetre::new(record.density_kg_m3),
                Kelvin::new(t),
            );
            4.0 * std::f64::consts::PI * record.refractive_index[0]
                / (LIQUID_BIN_EDGES_NM[0] * 1e-9)
                * xi
        };
        let (cooler, warmer) = (q_xi(crossing - 0.01), q_xi(crossing + 0.01));
        assert!(
            cooler < CRITICAL_SCATTERING_LIMIT && warmer > CRITICAL_SCATTERING_LIMIT,
            "{key}: q ξ {cooler} and {warmer} about {crossing} K"
        );
    }
}

/// A liquid's row in decision-r09-t13i §2.1's table: its key, its range, K, and the least and
/// greatest of its specific heats there, J kg⁻¹ K⁻¹, as the ruling rounds them to the unit.
type RuledSpecificHeat = (&'static str, (f64, f64), (f64, f64));

/// decision-r09-t13i §2.1's table, a [`RuledSpecificHeat`] for each of its six liquids.
const RULED_SPECIFIC_HEATS: [RuledSpecificHeat; 6] = [
    ("H2O", (275.0, 305.0), (4_180.0, 4_214.0)),
    ("CH4", (91.0, 116.0), (3_369.0, 3_515.0)),
    ("C2H6", (95.0, 150.0), (2_273.0, 2_333.0)),
    ("N2", (64.0, 78.0), (2_002.0, 2_045.0)),
    ("NH3", (196.0, 241.0), (4_307.0, 4_470.0)),
    ("CO2", (217.0, 272.0), (1_954.0, 2_508.0)),
];

/// The row `key`'s own specific heat column, which a test knows it has.
fn specific_heat(key: &str) -> &'static LiquidSpecificHeat {
    row(key)
        .substance()
        .liquid()
        .unwrap()
        .specific_heat()
        .unwrap()
}

/// The row `key`'s `c_l` at `t` K, J kg⁻¹ K⁻¹, which a test knows it has.
fn c_l(key: &str, t: f64) -> f64 {
    row(key)
        .substance()
        .liquid_specific_heat_j_per_kg_k(Kelvin::new(t))
        .unwrap()
}

/// A nine-figure saturated-liquid state of the WebBook: T, K; `p_σ`, MPa; ρ′, kg m⁻³; `c_v` and
/// `c_p`, J g⁻¹ K⁻¹; and w, m s⁻¹, as its table prints them.
type NineFigureState = (f64, f64, f64, f64, f64, f64);

/// Water's saturated liquid at 275, 285, 295 and 305 K, four of the seven rows of NIST Chemistry
/// WebBook, NIST Standard Reference Database 69: E. W. Lemmon, I. H. Bell, M. L. Huber and M. O.
/// `McLinden`, "Thermophysical Properties of Fluid Systems", DOI 10.18434/T4D303, saturation table by
/// temperature, nine figures, fetched 2026-10-20
/// (`https://webbook.nist.gov/cgi/fluid.cgi?Action=Data&Wide=on&ID=C7732185&Type=SatP&Digits=9&PLow=&PHigh=&PInc=&THigh=305&TLow=275&TInc=5&RefState=DEF&TUnit=K&PUnit=MPa&DUnit=kg%2Fm3&HUnit=kJ%2Fkg&WUnit=m%2Fs&VisUnit=uPa*s&STUnit=N%2Fm`,
/// SHA-256 86145c448199b0f24704040bfa6a152807d6d9fd326c200d36dcdc91a6eed198); the cross-check of
/// asserted values only (decision-composition §5).
const WATER_WEBBOOK: [NineFigureState; 4] = [
    (
        275.0,
        0.000_698_451_164,
        999.887_406,
        4.213_256_85,
        4.213_946_42,
        1_411.352_19,
    ),
    (
        285.0,
        0.001_389_030_71,
        999.469_969,
        4.185_193_69,
        4.192_729_30,
        1_454.286_41,
    ),
    (
        295.0,
        0.002_621_235_83,
        997.762_492,
        4.150_105_40,
        4.183_169_58,
        1_487.724_88,
    ),
    (
        305.0,
        0.004_719_348_24,
        995.032_997,
        4.109_717_96,
        4.179_773_48,
        1_513.142_88,
    ),
];

/// Ammonia's saturated liquid at 196, 211, 226 and 241 K, NIST Chemistry WebBook, NIST Standard
/// Reference Database 69: E. W. Lemmon, I. H. Bell, M. L. Huber and M. O. `McLinden`,
/// "Thermophysical Properties of Fluid Systems", DOI 10.18434/T4D303, saturation table by
/// temperature, nine figures, fetched 2026-10-20
/// (`https://webbook.nist.gov/cgi/fluid.cgi?Action=Data&Wide=on&ID=C7664417&Type=SatP&Digits=9&PLow=&PHigh=&PInc=&THigh=241&TLow=196&TInc=15&RefState=DEF&TUnit=K&PUnit=MPa&DUnit=kg%2Fm3&HUnit=kJ%2Fkg&WUnit=m%2Fs&VisUnit=uPa*s&STUnit=N%2Fm`,
/// SHA-256 358b45ea4b5cf0a7ef6785845f6f96d9b1f46c6b4853cdf8e5e68abdbdaf8875); the cross-check of
/// asserted values only (decision-composition §5).
const AMMONIA_WEBBOOK: [NineFigureState; 4] = [
    (
        196.0,
        0.006_305_237_90,
        733.268_185,
        2.967_754_62,
        4.306_594_82,
        2_008.133_92,
    ),
    (
        211.0,
        0.018_925_765_2,
        715.969_947,
        2.968_913_67,
        4.355_569_95,
        1_914.707_19,
    ),
    (
        226.0,
        0.048_158_998_1,
        698.359_154,
        2.951_997_58,
        4.412_817_89,
        1_822.092_59,
    ),
    (
        241.0,
        0.107_399_703,
        680.199_467,
        2.918_417_46,
        4.469_765_42,
        1_728.239_62,
    ),
];

/// Methane's saturated liquid at its column's ends, 91 and 116 K, NIST Chemistry WebBook, NIST
/// Standard Reference Database 69: E. W. Lemmon, I. H. Bell, M. L. Huber and M. O. `McLinden`,
/// "Thermophysical Properties of Fluid Systems", DOI 10.18434/T4D303, saturation table by
/// temperature, nine figures, fetched 2026-10-20
/// (`https://webbook.nist.gov/cgi/fluid.cgi?Action=Data&Wide=on&ID=C74828&Type=SatP&Digits=9&PLow=&PHigh=&PInc=&THigh=116&TLow=91&TInc=25&RefState=DEF&TUnit=K&PUnit=MPa&DUnit=kg%2Fm3&HUnit=kJ%2Fkg&WUnit=m%2Fs&VisUnit=uPa*s&STUnit=N%2Fm`,
/// SHA-256 a84f4ea4f602e2c7ab563accd34b598ead4b378226eeff96106e755def731b49); the cross-check of
/// asserted values only (decision-composition §5).
const METHANE_ENDS: [NineFigureState; 2] = [
    (
        91.0,
        0.012_164_065_1,
        451.068_214,
        2.165_700_98,
        3.368_812_96,
        1_535.831_86,
    ),
    (
        116.0,
        0.142_753_834,
        415.957_049,
        2.036_885_46,
        3.514_517_36,
        1_294.440_59,
    ),
];

/// Ethane's saturated liquid at its column's ends, 95 and 150 K, NIST Chemistry WebBook, NIST
/// Standard Reference Database 69: E. W. Lemmon, I. H. Bell, M. L. Huber and M. O. `McLinden`,
/// "Thermophysical Properties of Fluid Systems", DOI 10.18434/T4D303, saturation table by
/// temperature, nine figures, fetched 2026-10-20
/// (`https://webbook.nist.gov/cgi/fluid.cgi?Action=Data&Wide=on&ID=C74840&Type=SatP&Digits=9&PLow=&PHigh=&PInc=&THigh=150&TLow=95&TInc=55&RefState=DEF&TUnit=K&PUnit=MPa&DUnit=kg%2Fm3&HUnit=kJ%2Fkg&WUnit=m%2Fs&VisUnit=uPa*s&STUnit=N%2Fm`,
/// SHA-256 ef0a1dff2b2c3a1f466e3df684b5fc334df5b402658638129a4c4c63394fc443); the cross-check of
/// asserted values only (decision-composition §5).
const ETHANE_ENDS: [NineFigureState; 2] = [
    (
        95.0,
        3.626_791_31e-6,
        646.433_821,
        1.569_313_26,
        2.299_065_26,
        1_974.776_14,
    ),
    (
        150.0,
        0.009_637_999_31,
        585.168_838,
        1.441_718_11,
        2.332_596_88,
        1_575.529_88,
    ),
];

/// Nitrogen's saturated liquid at its column's ends, 64 and 78 K, NIST Chemistry WebBook, NIST
/// Standard Reference Database 69: E. W. Lemmon, I. H. Bell, M. L. Huber and M. O. `McLinden`,
/// "Thermophysical Properties of Fluid Systems", DOI 10.18434/T4D303, saturation table by
/// temperature, nine figures, fetched 2026-10-20
/// (`https://webbook.nist.gov/cgi/fluid.cgi?Action=Data&Wide=on&ID=C7727379&Type=SatP&Digits=9&PLow=&PHigh=&PInc=&THigh=78&TLow=64&TInc=14&RefState=DEF&TUnit=K&PUnit=MPa&DUnit=kg%2Fm3&HUnit=kJ%2Fkg&WUnit=m%2Fs&VisUnit=uPa*s&STUnit=N%2Fm`,
/// SHA-256 cf3b6b3b3c609f1e830b215047d1a25cf0f4a301eb8b43bcf2983102c062a238); the cross-check of
/// asserted values only (decision-composition §5).
const NITROGEN_ENDS: [NineFigureState; 2] = [
    (
        64.0,
        0.014_602_334_6,
        863.731_629,
        1.170_348_15,
        2.001_622_62,
        986.570_915,
    ),
    (
        78.0,
        0.109_255_200,
        803.148_076,
        1.080_340_35,
        2.044_668_41,
        844.820_709,
    ),
];

/// Carbon dioxide's saturated liquid at its column's ends, 217 and 272 K, the first and last of
/// the twelve rows of NIST Chemistry WebBook, NIST Standard Reference Database 69: E. W. Lemmon, I.
/// H. Bell, M. L. Huber and M. O. `McLinden`, "Thermophysical Properties of Fluid Systems", DOI
/// 10.18434/T4D303, saturation table by temperature, nine figures, fetched 2026-10-20
/// (`https://webbook.nist.gov/cgi/fluid.cgi?Action=Data&Wide=on&ID=C124389&Type=SatP&Digits=9&PLow=&PHigh=&PInc=&THigh=272&TLow=217&TInc=5&RefState=DEF&TUnit=K&PUnit=MPa&DUnit=kg%2Fm3&HUnit=kJ%2Fkg&WUnit=m%2Fs&VisUnit=uPa*s&STUnit=N%2Fm`,
/// SHA-256 66d222adee2b7eb9edc57a5c5231b5a7d48d2558048b20088cd3b426310d04a6); the cross-check of
/// asserted values only (decision-composition §5).
const CARBON_DIOXIDE_ENDS: [NineFigureState; 2] = [
    (
        217.0,
        0.527_215_953,
        1_176.998_02,
        0.974_074_048,
        1.954_152_68,
        972.892_274,
    ),
    (
        272.0,
        3.380_232_92,
        934.258_898,
        0.942_826_708,
        2.507_868_71,
        547.108_237,
    ),
];

/// The gas constant the WebBook evaluates Gao et al. 2023's ammonia with, CODATA 2014's R =
/// 8.314 459 8 J mol⁻¹ K⁻¹ (Mohr, Newell and Taylor 2016, Rev. Mod. Phys. 88, 035009), over the
/// 8.314 462 618 J mol⁻¹ K⁻¹ its coefficients were fitted with (decision-r09-t13i-cl-licence §6.2):
/// the WebBook's heat capacities are the equation's times this.
const WEBBOOK_AMMONIA_GAS_CONSTANT_RATIO: f64 = 8.314_459_8 / 8.314_462_618;

/// The six liquids with a column: each one's equation and ideal part, its nine-figure WebBook
/// states (its column's two ends first and last), the factor that takes the table's heat
/// capacities to J kg⁻¹ K⁻¹ at the equation's own gas constant, and the tolerance its `c_v`, `c_p`
/// and `κ_T` meet there.
type NineFigureRow = (
    &'static str,
    &'static HelmholtzResidual,
    &'static HelmholtzIdeal,
    &'static [NineFigureState],
    f64,
    f64,
);

/// [`NineFigureRow`]s: ethane's tolerance is its `κ_T` test's, and ammonia's heat capacities are
/// taken to its own gas constant by [`WEBBOOK_AMMONIA_GAS_CONSTANT_RATIO`].
const NINE_FIGURE_WEBBOOK: [NineFigureRow; 6] = [
    ("H2O", &IAPWS_95, &IAPWS_95_IDEAL, &WATER_WEBBOOK, 1e3, 1e-6),
    (
        "CH4",
        &super::super::rows::equations::METHANE,
        &METHANE_IDEAL,
        &METHANE_ENDS,
        1e3,
        1e-6,
    ),
    (
        "C2H6",
        &super::super::rows::equations::ETHANE,
        &ETHANE_IDEAL,
        &ETHANE_ENDS,
        1e3,
        3e-6,
    ),
    (
        "N2",
        &super::super::rows::equations::NITROGEN,
        &NITROGEN_IDEAL,
        &NITROGEN_ENDS,
        1e3,
        1e-6,
    ),
    (
        "NH3",
        &super::super::rows::equations::AMMONIA,
        &AMMONIA_IDEAL,
        &AMMONIA_WEBBOOK,
        1e3 / WEBBOOK_AMMONIA_GAS_CONSTANT_RATIO,
        1e-6,
    ),
    (
        "CO2",
        &super::super::rows::equations::CARBON_DIOXIDE,
        &CARBON_DIOXIDE_IDEAL,
        &CARBON_DIOXIDE_ENDS,
        1e3,
        1e-6,
    ),
];

/// With its ideal part, each liquid's equation reproduces the WebBook's evaluation of the same
/// equation at its nine-figure states ([`NINE_FIGURE_WEBBOOK`], each table cited at its constant):
/// `c_v`, `c_p` and `κ_T` at the table's densities within 10⁻⁶, ethane's within 3 × 10⁻⁶ as its
/// `κ_T` test holds, and ammonia's heat capacities after the ratio of the WebBook's gas constant to
/// Gao et al.'s, 3.4 × 10⁻⁷ (decision-r09-t13i-cl-licence §6.3): water's and ammonia's at four
/// states each, the others' at their columns' two ends.
#[test]
fn each_equation_s_heat_capacities_reproduce_the_webbook_s_nine_figures() {
    for (key, equation, ideal, states, unit, tolerance) in NINE_FIGURE_WEBBOOK {
        let rows: Vec<_> = states
            .iter()
            .map(|&(t, _, rho, cv, cp, w)| (t, rho, cv, cp, w))
            .collect();
        assert_heat_capacities(key, equation, ideal, &rows, unit, tolerance);
        assert_compressibility(key, equation, &rows, tolerance);
    }
}

/// Each column at its range's two ends, its equation's `c_p` at the equation's saturated liquid on
/// the row's phase diagram, meets the WebBook's nine-figure saturated `c_p` within 10⁻⁴
/// (decision-r09-t13i-cl-licence §6.3, item 4), the WebBook's own value, ammonia's not scaled by
/// its gas constants' ratio; each end prints its miss, the diagram's vapour pressure against the
/// WebBook's and the Newton density against the WebBook's.
#[test]
fn each_liquid_specific_heat_meets_the_webbook_at_its_range_s_ends() {
    for (key, equation, _, states, _, _) in NINE_FIGURE_WEBBOOK {
        let diagram = row(key).substance().phase_diagram().unwrap();
        let (low, high) = specific_heat(key).range();
        let ends = [states[0], states[states.len() - 1]];
        for (end, (t, p_mpa, rho, _, cp, _)) in [low, high].into_iter().zip(ends) {
            assert_same_bits(end.value(), t);
            let temperature = Kelvin::new(t);
            let c = c_l(key, t);
            let webbook = cp * 1e3;
            let miss = c / webbook - 1.0;
            let p_sigma = diagram.saturation_pressure(temperature);
            let p_webbook = p_mpa * 1e6;
            let start = diagram.liquid_density(temperature).unwrap();
            let density = equation.density_at_pressure(start, p_sigma, temperature);
            let left = equation.pressure(density, temperature).value() - p_sigma.value();
            println!(
                "{key} at {t} K: c_l {c:.4} against {webbook:.4} J kg⁻¹ K⁻¹, miss {miss:.2e}; p_σ \
                 {:.6e} against {p_webbook:.6e} Pa, miss {:.2e}; ρ {:.6} against {rho} kg m⁻³, \
                 miss {:.2e} (the ancillary's {:.2e}); p − p_σ {left:.2e} Pa",
                p_sigma.value(),
                p_sigma.value() / p_webbook - 1.0,
                density.value(),
                density.value() / rho - 1.0,
                start.value() / rho - 1.0,
            );
            assert!(
                miss.abs() < 1e-4,
                "{key} at {t} K: c_l {c} against the WebBook's {webbook}, miss {miss:e}"
            );
        }
    }
}

/// Each column's range is the ruling's, through the column and the row, and outside it the end
/// value is held to the bit; a NaN gives a NaN.
#[test]
fn each_liquid_specific_heat_holds_its_ends_outside_its_range() {
    for (key, (low, high), _) in RULED_SPECIFIC_HEATS {
        let substance = row(key).substance();
        let column = specific_heat(key);
        let (from, to) = column.range();
        assert_same_bits(from.value(), low);
        assert_same_bits(to.value(), high);
        assert_same_bits(column.low().value(), low);
        assert_same_bits(column.high().value(), high);
        let (row_from, row_to) = substance.liquid_specific_heat_range().unwrap();
        assert_same_bits(row_from.value(), low);
        assert_same_bits(row_to.value(), high);
        let (first, last) = (c_l(key, low), c_l(key, high));
        for t in [0.0, low - 20.0, low - 1e-6] {
            assert_same_bits(c_l(key, t), first);
        }
        for t in [high + 1e-6, high + 50.0, 1e4] {
            assert_same_bits(c_l(key, t), last);
        }
        assert!(c_l(key, f64::NAN).is_nan(), "{key}");
    }
}

/// The rows with a specific heat column are the ruling's six, each over a range above its triple
/// point and below its critical point, from its scattering's own equation; at every half kelvin of
/// the range its value is finite and positive, inside the ruling's rounded least and greatest,
/// which its own least and greatest round to, and its saturated liquid's pressure is the
/// diagram's vapour pressure within 10⁻⁴ Pa, the rounding of the dense liquid's pressure (3.3 ×
/// 10⁻⁵ Pa at most in water, whose `ρRT` is about 1.3 × 10⁸ Pa; under 10⁻⁶ Pa in the others).
#[test]
fn each_liquid_specific_heat_is_ordered_and_physical() {
    let mut with_column: Vec<&str> = super::super::rows::liquids::LIQUIDS
        .iter()
        .filter(|(_, columns)| columns.specific_heat().is_some())
        .map(|(key, _)| key.as_str())
        .collect();
    let mut ruled: Vec<&str> = RULED_SPECIFIC_HEATS.iter().map(|&(key, ..)| key).collect();
    with_column.sort_unstable();
    ruled.sort_unstable();
    assert_eq!(with_column, ruled);
    for (key, (low, high), (least, greatest)) in RULED_SPECIFIC_HEATS {
        let substance = row(key).substance();
        let columns = substance.liquid().unwrap();
        let column = specific_heat(key);
        let diagram = substance.phase_diagram().unwrap();
        assert!(low < high, "{key}");
        assert!(diagram.triple_temperature().value() < low, "{key}");
        assert!(high < diagram.critical_temperature().value(), "{key}");
        assert!(
            core::ptr::eq(column.equation(), columns.scattering().unwrap().equation()),
            "{key}: the specific heat's equation is the scattering column's own"
        );
        let mut own_least = f64::INFINITY;
        let mut own_greatest = f64::NEG_INFINITY;
        let mut half_kelvins = 0.0;
        loop {
            let t = low + half_kelvins / 2.0;
            if t > high {
                break;
            }
            half_kelvins += 1.0;
            let c = c_l(key, t);
            assert!(c.is_finite() && c > 0.0, "{key} at {t} K: {c}");
            assert!(
                (least - 0.5..=greatest + 0.5).contains(&c),
                "{key} at {t} K: {c} outside the ruling's {least}–{greatest}"
            );
            own_least = own_least.min(c);
            own_greatest = own_greatest.max(c);
            let temperature = Kelvin::new(t);
            let p_sigma = diagram.saturation_pressure(temperature);
            let start = diagram.liquid_density(temperature).unwrap();
            let density = column
                .equation()
                .density_at_pressure(start, p_sigma, temperature);
            let left = column.equation().pressure(density, temperature).value() - p_sigma.value();
            assert!(left.abs() < 1e-4, "{key} at {t} K: p − p_σ = {left} Pa");
        }
        assert!(
            (own_least - least).abs() <= 0.5 && (own_greatest - greatest).abs() <= 0.5,
            "{key}: {own_least}–{own_greatest} against the ruling's {least}–{greatest}"
        );
    }
}

/// Titan's seas' specific heat at 100 K is its components' mean weighted by mass at its reference
/// composition, by hand: methane's own column at 100 K, nitrogen's held from its 78 K end and
/// ethane's at 100 K, each weighted by wᵢ = xᵢ Mᵢ ÷ Σ xⱼ Mⱼ. A solution has no range of its own.
#[test]
fn titan_s_seas_specific_heat_is_its_components_mass_weighted_mean() {
    let titan = row("ch4_c2h6_n2").substance();
    let reference = titan.liquid().unwrap().reference_composition().unwrap();
    let fraction = |key: &str| {
        reference
            .mole_fractions()
            .iter()
            .find(|&&(id, _)| id == row(key))
            .unwrap()
            .1
    };
    let by_hand = [
        ("CH4", c_l("CH4", 100.0)),
        ("N2", c_l("N2", 78.0)),
        ("C2H6", c_l("C2H6", 100.0)),
    ];
    assert_same_bits(c_l("N2", 100.0), by_hand[1].1);
    let masses = by_hand.map(|(key, c)| {
        let molar_mass = row(key).substance().molar_mass_g_per_mol().unwrap();
        (fraction(key) * molar_mass, c)
    });
    let total: f64 = masses.iter().map(|&(m, _)| m).sum();
    let mean: f64 = masses.iter().map(|&(m, c)| m / total * c).sum();
    let c = titan
        .liquid_specific_heat_j_per_kg_k(Kelvin::new(100.0))
        .unwrap();
    assert!(
        (c / mean - 1.0).abs() < 1e-12,
        "{c} J kg⁻¹ K⁻¹ against the hand mean {mean}"
    );
    assert!(titan.liquid_specific_heat_range().is_none());
}

/// A row with no column of its own or no liquid has no specific heat: a liquid the ruling gives
/// none (oxygen), a row with no liquid columns (neon), a silicate melt, the chloride and sulphate
/// brines (their salts have no column), the perchlorate brines (their solutes have no rows) and
/// the aqueous acid (the acid has none). Ammonia–water, whose components both have one, lies
/// between them.
#[test]
fn a_liquid_without_a_specific_heat_has_none() {
    for key in [
        "O2",
        "Ne",
        "basalt",
        "brine_nacl",
        "brine_mgso4",
        "brine_naclo4",
        "brine_mgclo4",
        "H2SO4",
    ] {
        let substance = row(key).substance();
        for t in [80.0, 280.0, 1_500.0] {
            let c = substance.liquid_specific_heat_j_per_kg_k(Kelvin::new(t));
            assert!(c.is_none(), "{key} at {t} K: {c:?}");
        }
        assert!(substance.liquid_specific_heat_range().is_none(), "{key}");
    }
    let mixed = c_l("nh3_h2o", 200.0);
    let (water, ammonia) = (c_l("H2O", 200.0), c_l("NH3", 200.0));
    assert!(
        water.min(ammonia) < mixed && mixed < water.max(ammonia),
        "{mixed} between {water} and {ammonia}"
    );
}

// P14.T49.n: the liquids' compression (decision-p14-t54a-water-consistency, its Batch B follow-up
// of 2026-10-20, F.1 and C), held until the 23 → 24 bump.

/// The rows whose liquid the compression's law reads a reference equation of state for, in
/// registry order.
fn compressible_rows() -> Vec<SubstanceId> {
    let mut rows: Vec<SubstanceId> = super::super::rows::liquids::LIQUIDS
        .iter()
        .map(|(key, _)| row(key.as_str()))
        .filter(|id| equation_of_state(id.substance()).is_some())
        .collect();
    rows.sort_unstable();
    rows
}

/// The liquids' compression is held until P14.T54.b's bump: below it every reader takes the
/// saturated liquid, and the tests' switch reaches 24's compressed liquids ahead of it.
#[test]
fn the_compressed_liquids_are_held_only_until_the_24_bump() {
    assert!(
        crate::GENERATOR_VERSION < COMPRESSED_LIQUIDS_VERSION,
        "the 23 → 24 bump (P14.T54.b) compresses the record's liquids to their body's surface \
         pressure (P14.T49.n). Delete `COMPRESSED_LIQUIDS_VERSION`, `CompressedLiquidsRules` with \
         its `Saturated` arm, `with_compressed_liquids` and its thread's rules (and the switch in \
         every test that sets it), the `Saturated` arms of `Substance::liquid_density_at_pressure` \
         and `liquid_properties_at_pressure`, this test and \
         `a_panic_inside_the_compressed_liquids_switch_sets_the_rules_back`. Re-bless the goldens \
         it moves: \
         tests/golden/planetary/systems/*.golden (every sea's layer and area, a pure liquid's \
         compressed to its body's surface pressure: close_binary's and m_dwarf_resonant_chain's \
         water, subgiant's nitrogen; through the area mix's sea records, which read the \
         compressed record (planetary/derive/photometry/area_mix.rs's `liquid_record`, its \
         reference Earth's ocean left saturated), every body's Bond albedo over a pure liquid \
         sea and so every quantity the passes give it; and, through the own-ocean weight's \
         dominance pull, every quantity of a body with a dominant pure liquid, methane's under \
         Titan's seas included), tests/golden/planetary/partition.golden, \
         tests/golden/planetary/surface_conditions_published.golden and \
         tests/golden/planetary/surface_inputs.golden (the palette's liquid entries: their \
         grain density, index, scattering and viscosity at the compressed density). \
         tests/golden/substance/liquids.golden does not move (`liquid_properties` stays the \
         saturated record). Then settle the record tests that read the saturated liquid: \
         `an_earth_like_body_s_sea_record_is_measured` (surface/inputs.rs, \"a pure fluid's grain \
         density is its saturated liquid's\"); and the tests that set the switch, whose held \
         halves read 23's saturated liquid: \
         `under_compression_earth_s_sea_reads_one_density`, \
         `under_compression_a_carbon_dioxide_sea_and_its_ocean_weight_read_one_density` and \
         `close_binary_s_sea_shrinks_under_its_compressed_water` (partition/tests.rs), \
         `a_liquid_s_entry_reads_its_record_compressed_to_the_surface_pressure` (palette.rs), \
         `under_compression_the_area_mix_s_sea_reads_its_layer_s_density` and \
         `under_compression_the_area_mix_s_sea_reads_the_palette_s_density` \
         (derive/photometry/area_mix/tests.rs) and \
         `the_record_at_a_pressure_reads_the_compressed_density` (liquid/tests.rs)"
    );
    assert_eq!(
        CompressedLiquidsRules::in_force(),
        CompressedLiquidsRules::Saturated
    );
    assert_eq!(
        with_compressed_liquids(
            CompressedLiquidsRules::Compressed,
            CompressedLiquidsRules::in_force
        ),
        CompressedLiquidsRules::Compressed
    );
    assert_eq!(
        CompressedLiquidsRules::in_force(),
        CompressedLiquidsRules::Saturated
    );
}

/// A panic inside the tests' switch sets the rules back. Skipped, not failed, where panics abort
/// (wasm32-wasip1), since `catch_unwind` would abort the whole test binary there; the other CI
/// architectures run it.
#[test]
#[cfg(panic = "unwind")]
fn a_panic_inside_the_compressed_liquids_switch_sets_the_rules_back() {
    let failed = std::panic::catch_unwind(|| {
        with_compressed_liquids(CompressedLiquidsRules::Compressed, || {
            panic!("a test fails inside")
        });
    });
    assert!(failed.is_err());
    assert_eq!(
        CompressedLiquidsRules::in_force(),
        CompressedLiquidsRules::Saturated,
        "a panic inside the switch sets the rules back"
    );
}

/// The law's rows are the pure liquids with a reference equation of state, each a diagram row:
/// the thirteen with a scattering equation, sulphur dioxide (none) and every solution and melt
/// left incompressible.
#[test]
fn the_compressible_liquids_are_the_pure_rows_with_an_equation_of_state() {
    let keys: Vec<String> = compressible_rows()
        .iter()
        .map(|id| id.substance().key().as_str().to_owned())
        .collect();
    let mut expected = [
        "H2O", "CH4", "NH3", "N2", "O2", "CO2", "Ar", "Kr", "Xe", "N2O", "CO", "H2S", "C2H6",
    ]
    .map(row);
    expected.sort_unstable();
    let expected: Vec<String> = expected
        .iter()
        .map(|id| id.substance().key().as_str().to_owned())
        .collect();
    assert_eq!(keys, expected);
    for id in compressible_rows() {
        let substance = id.substance();
        assert!(
            matches!(
                substance.condensed().unwrap().grain_density(),
                GrainDensity::OfDiagram
            ),
            "{}",
            substance.key()
        );
    }
}

/// Water against NIST's isotherms (the NIST Chemistry WebBook, SRD 69, its evaluation of
/// IAPWS-95; decision-p14-t54a-water-consistency's Batch B follow-up, F.1): at 596.41 K, 690.48
/// kg m⁻³ under 22.5 MPa against 658.85 saturated at 11.78 MPa (+4.8%), and at 546.30 K, 765.86
/// under 8.25 MPa against 762.165 saturated (+0.48%), the law within 0.05% of each, the
/// registry's saturated liquid within 0.03% of NIST's and the compression's ratio within 0.03%
/// of NIST's; the diagram's vapour pressure at 596.41 K within 0.2% of NIST's 11.78 MPa.
#[test]
fn water_s_compressed_liquid_meets_nist_s_isotherms() {
    let water = row("H2O").substance();
    let diagram = water.phase_diagram().unwrap();
    for (t, p, nist, nist_saturated, nist_vapour) in [
        (596.41, 22.5e6, 690.48, 658.85, Some(11.78e6)),
        (546.30, 8.25e6, 765.86, 762.165, None),
    ] {
        let temperature = Kelvin::new(t);
        let saturated = water
            .grain_density(CondensedPhase::Liquid, temperature)
            .unwrap()
            .value();
        let compressed = water
            .compressed_liquid_density(temperature, Pascals::new(p))
            .unwrap()
            .value();
        let vapour = diagram.saturation_pressure(temperature).value();
        let ratio = compressed / saturated;
        eprintln!(
            "water at {t} K: saturated {saturated:.3} kg m⁻³ (NIST {nist_saturated}) at {:.4} MPa \
             (NIST {nist_vapour:?} Pa); under {:.2} MPa {compressed:.3} (NIST {nist}, miss \
             {:.2e}), ×{ratio:.6} (NIST ×{:.6})",
            vapour / 1e6,
            p / 1e6,
            compressed / nist - 1.0,
            nist / nist_saturated,
        );
        assert!(
            (compressed / nist - 1.0).abs() < 5e-4,
            "{t} K: {compressed}"
        );
        assert!((saturated / nist_saturated - 1.0).abs() < 3e-4, "{t} K");
        assert!(
            (ratio / (nist / nist_saturated) - 1.0).abs() < 3e-4,
            "{t} K"
        );
        if let Some(nist_vapour) = nist_vapour {
            assert!(
                (vapour / nist_vapour - 1.0).abs() < 2e-3,
                "{t} K: {vapour} Pa"
            );
        }
    }
}

/// Earth's sea at 288.15 K under one atmosphere is denser than the saturated liquid by `κ_T` Δp,
/// about 4.6 × 10⁻⁵ (`κ_T` = 4.67 × 10⁻¹⁰ Pa⁻¹, Δp = 101,325 − 1,705 Pa; Millero et al. 1980's
/// secant bulk modulus, as `water_s_compressibility_is_millero_s` reads it), within 2 × 10⁻⁶.
#[test]
fn earth_s_sea_is_compressed_by_its_air() {
    let water = row("H2O").substance();
    let t = Kelvin::new(288.15);
    let saturated = water
        .grain_density(CondensedPhase::Liquid, t)
        .unwrap()
        .value();
    let sea = water
        .compressed_liquid_density(t, Pascals::new(101_325.0))
        .unwrap()
        .value();
    let relative = sea / saturated - 1.0;
    eprintln!("Earth's sea at 288.15 K and 1 atm: {sea:.6} against {saturated:.6}, {relative:.4e}");
    assert!((relative - 4.6e-5).abs() < 2e-6, "{relative:e}");
}

/// The law never expands a liquid and is the registry's saturated liquid to the bit where it does
/// not compress it: at and below each row's vapour pressure, at no pressure, at a pressure that
/// is not a number, at the critical temperature and for every liquid with no equation of state at
/// any pressure; above the vapour pressure it is denser, rising with the pressure, and held above
/// the ceiling.
#[test]
fn the_law_compresses_and_never_expands() {
    for id in compressible_rows() {
        let substance = id.substance();
        let key = substance.key();
        let diagram = substance.phase_diagram().unwrap();
        let (triple, critical) = (
            diagram.triple_temperature().value(),
            diagram.critical_temperature().value(),
        );
        for k in 0..=10_u32 {
            let t = Kelvin::new(triple + (critical - triple) * f64::from(k) / 10.0);
            let saturated = substance
                .grain_density(CondensedPhase::Liquid, t)
                .unwrap()
                .value();
            let vapour = diagram.saturation_pressure(t).value();
            for p in [0.0, -1.0, f64::NAN, vapour * 0.5, vapour] {
                let law = substance
                    .compressed_liquid_density(t, Pascals::new(p))
                    .unwrap();
                assert_same_bits(law.value(), saturated);
            }
            if k == 10 {
                continue;
            }
            let mut last = saturated;
            for p in [vapour * 1.001 + 1.0, vapour + 1e6, vapour + 2e7, 1e8, 1e9] {
                let law = substance
                    .compressed_liquid_density(t, Pascals::new(p))
                    .unwrap()
                    .value();
                assert!(
                    law.is_finite() && law > last,
                    "{key} at {t:?} under {p} Pa: {law}"
                );
                last = law;
            }
            let held = substance
                .compressed_liquid_density(t, Pascals::new(f64::INFINITY))
                .unwrap()
                .value();
            assert_same_bits(held, last);
        }
    }
    for key in [
        "SO2",
        "brine_nacl",
        "nh3_h2o",
        "ch4_c2h6_n2",
        "H2SO4",
        "basalt",
    ] {
        let substance = row(key).substance();
        let t = Kelvin::new(300.0);
        let stated = substance.grain_density(CondensedPhase::Liquid, t);
        for p in [0.0, 1e5, 1e8] {
            let law = substance.compressed_liquid_density(t, Pascals::new(p));
            match (law, stated) {
                (Some(law), Some(stated)) => assert_same_bits(law.value(), stated.value()),
                (None, None) => {}
                (law, stated) => panic!("{key} under {p} Pa: {law:?} against {stated:?}"),
            }
        }
    }
}

/// The compression's fixed count meets the pressure to rounding: on every compressible row, over
/// its liquid range to a part in a thousand below its critical point and from just above its
/// vapour pressure to the ceiling, [`COMPRESSION_STEPS`] steps leave the pressure within 10⁻⁹ of
/// the target, or within 10⁻³ Pa, the rounding of a dense liquid's pressure (whose `ρRT` is about
/// 10⁸ Pa), and are where eight more steps leave them, within 10⁻¹², denser than the saturated
/// liquid. Each row prints the most steps any case needed to come within 10⁻¹³ of where 64 leave
/// it.
#[test]
fn the_compression_converges_over_every_row_s_liquid_range() {
    use super::super::helmholtz::COMPRESSION_STEPS;
    // The steps the iteration takes to come within 10⁻¹³ of where 64 leave it.
    let steps_needed = |equation: &HelmholtzResidual, start, target, t| {
        let mut sequence = vec![start];
        for _ in 0..64 {
            let last = *sequence.last().unwrap();
            sequence.push(equation.compressed_density_in(last, target, t, 1));
        }
        let settled: KilogramsPerCubicMetre = sequence[64];
        sequence
            .iter()
            .position(|rho| (rho.value() / settled.value() - 1.0).abs() < 1e-13)
            .unwrap()
    };
    for id in compressible_rows() {
        let substance = id.substance();
        let key = substance.key();
        let (equation, diagram) = equation_of_state(substance).unwrap();
        let (triple, critical) = (
            diagram.triple_temperature().value(),
            diagram.critical_temperature().value(),
        );
        let mut worst = (0, 0.0, 0.0, 0.0);
        let mut worst_miss: f64 = 0.0;
        for fraction in [
            0.0, 0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 0.98, 0.99, 0.995, 0.999,
        ] {
            let t = Kelvin::new(triple + (critical - triple) * fraction);
            let saturated = substance.grain_density(CondensedPhase::Liquid, t).unwrap();
            let vapour = diagram.saturation_pressure(t);
            let at_vapour = equation.density_at_pressure(saturated, vapour, t);
            for p in [
                vapour.value() * 1.01 + 1e3,
                vapour.value() + 1e5,
                vapour.value() + 1e6,
                vapour.value() + 1e7,
                22.5e6_f64.max(vapour.value() * 1.5),
                1e8,
                3e8,
                1e9,
            ] {
                let target = Pascals::new(p);
                let rho = equation.compressed_density(at_vapour, target, t);
                let more =
                    equation.compressed_density_in(at_vapour, target, t, COMPRESSION_STEPS + 8);
                let off = equation.pressure(rho, t).value() - p;
                let miss = off / p;
                let settled = rho.value() / more.value() - 1.0;
                assert!(
                    (miss.abs() < 1e-9 || off.abs() < 1e-3) && settled.abs() < 1e-12,
                    "{key} at {t:?} under {p} Pa: ρ {rho:?}, p miss {miss:e}, {settled:e} from \
                     {more:?}"
                );
                assert!(rho > at_vapour, "{key} at {t:?} under {p} Pa");
                let needed = steps_needed(equation, at_vapour, target, t);
                if needed > worst.0 {
                    worst = (needed, t.value(), p, rho.value() / at_vapour.value());
                }
                worst_miss = worst_miss.max(miss.abs());
            }
        }
        eprintln!(
            "{key}: at most {} of {COMPRESSION_STEPS} steps (at {:.3} K under {:.3e} Pa, ×{:.4}), \
             pressure miss at most {worst_miss:.1e}",
            worst.0, worst.1, worst.2, worst.3
        );
    }
}

/// A step that is not finite ends the compression at the density it would step from, rather than
/// carrying a density that is not a number onto the record: at a temperature that is not a
/// number every step is, and water's compression gives back its start to the bit.
#[test]
fn a_step_that_is_not_finite_ends_the_compression_at_its_last_density() {
    let (equation, _) = equation_of_state(row("H2O").substance()).unwrap();
    let start = KilogramsPerCubicMetre::new(997.0);
    let rho = equation.compressed_density(start, Pascals::new(1e7), Kelvin::new(f64::NAN));
    assert_same_bits(rho.value(), start.value());
}

/// The record at a pressure is [`liquid_properties`]' to the bit under 23's rules, for a pure
/// liquid, a solution and a melt at any pressure; under 24's, a pure liquid's density is the
/// law's at the record's temperature to the bit (W4: the palette reads the partition's density),
/// its index carried to it, its scattering's `κ_T` there and its viscosity there, and a
/// solution's and a melt's record are unchanged.
#[test]
fn the_record_at_a_pressure_reads_the_compressed_density() {
    let cases = [
        ("H2O", 288.15, 101_325.0),
        ("H2O", 596.41, 22.5e6),
        ("CH4", 93.65, 1.5e5),
        ("N2", 64.2, 1.52e4),
        ("CO2", 250.0, 5e6),
        ("ch4_c2h6_n2", 93.65, 1.5e5),
        ("brine_nacl", 280.0, 1e7),
        ("basalt", 1_500.0, 1e7),
    ];
    for (key, t, p) in cases {
        let id = row(key);
        let temperature = Kelvin::new(t);
        let pressure = Pascals::new(p);
        let saturated = liquid_properties(id, temperature, Composition::Reference).unwrap();
        let held = liquid_properties_at_pressure(id, temperature, Composition::Reference, pressure)
            .unwrap();
        assert_eq!(held, saturated, "{key}");
        let compressed = with_compressed_liquids(CompressedLiquidsRules::Compressed, || {
            liquid_properties_at_pressure(id, temperature, Composition::Reference, pressure)
        })
        .unwrap();
        if equation_of_state(id.substance()).is_none() {
            assert_eq!(compressed, saturated, "{key}");
            continue;
        }
        let law = id
            .substance()
            .compressed_liquid_density(temperature, pressure)
            .unwrap();
        assert_same_bits(compressed.density_kg_m3, law.value());
        let held_density = with_compressed_liquids(CompressedLiquidsRules::Compressed, || {
            id.substance()
                .liquid_density_at_pressure(temperature, pressure)
        })
        .unwrap();
        assert_same_bits(held_density.value(), law.value());
        assert!(compressed.density_kg_m3 > saturated.density_kg_m3, "{key}");
        let carried = carry_index(
            &saturated.refractive_index,
            KilogramsPerCubicMetre::new(saturated.density_kg_m3),
            law,
        );
        for (ours, theirs) in compressed.refractive_index.iter().zip(carried) {
            assert!(
                (ours / theirs - 1.0).abs() < 1e-12,
                "{key}: {ours} against {theirs}"
            );
        }
        let columns = id.substance().liquid().unwrap();
        let equation = columns.scattering().unwrap().equation();
        let kappa = equation.isothermal_compressibility_per_pa(law, temperature);
        let saturated_kappa = equation.isothermal_compressibility_per_pa(
            KilogramsPerCubicMetre::new(saturated.density_kg_m3),
            temperature,
        );
        assert!(
            kappa < saturated_kappa,
            "{key}: κ_T falls as the liquid is compressed"
        );
        if let Some(viscosity) = columns.viscosity() {
            let (mu, _) = viscosity.viscosity_pa_s(temperature, law);
            assert_same_bits(compressed.viscosity_pa_s.unwrap(), mu);
        }
        eprintln!(
            "{key} at {t} K under {p:.4e} Pa: ρ {:.4} → {:.4} kg m⁻³, n(545 nm) {:.6} → {:.6}, \
             b(380–405 nm) {:?} → {:?}, μ {:?} → {:?}, {:?} → {:?}",
            saturated.density_kg_m3,
            compressed.density_kg_m3,
            saturated.refractive_index[6],
            compressed.refractive_index[6],
            saturated.scattering_per_m.map(|b| b[0]),
            compressed.scattering_per_m.map(|b| b[0]),
            saturated.viscosity_pa_s,
            compressed.viscosity_pa_s,
            saturated.basis,
            compressed.basis,
        );
    }
}
