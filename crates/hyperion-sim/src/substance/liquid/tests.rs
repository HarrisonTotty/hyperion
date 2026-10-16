//! The liquid columns' tests (P14.T49.h): the IAPWS releases reproduced, water's record against
//! its oracles, the R08 files carried, and the record's rules.

use hyperion_surface::field::{
    LIQUID_BIN_CENTRES_NM, MaterialPalette, MechanicsFamily, PaletteEntry, PaletteRole,
};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::super::rows::liquids::IAPWS_95;
use super::*;
use crate::substance::{by_key, ids};
use crate::tables::liquid_optics as bins;

/// The row `key`, which a test knows to be a row.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap()
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
/// figure, its footnote a).
#[test]
fn iapws_95_reproduces_its_release_s_tables_6_and_7() {
    let d = IAPWS_95.density_derivatives(KilogramsPerCubicMetre::new(838.025), Kelvin::new(500.0));
    assert!((d.first / -0.364_366_650 - 1.0).abs() < 2e-9, "{}", d.first);
    assert!(
        (d.second / 0.856_063_701 - 1.0).abs() < 2e-9,
        "{}",
        d.second
    );
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
/// al. 1980's secant bulk modulus of pure water at one atmosphere), the PMH model and δ = 0.039.
fn zhang_hu_he(nm: f64, celsius: f64) -> f64 {
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
    let delta = 0.039;
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
/// their index, Quan and Fry's, against IAPWS-95 and R9-97's), and at 20 °C it is about 0.003 26
/// m⁻¹ in the bin across 443 nm and 0.001 37 in the bin across 545 nm, pure water's: Morel
/// 1974's 0.0045 and 0.0019 m⁻¹ at 450 and 550 nm, which decision-r11-palette-liquids §1.8
/// quotes from memory, are his seawater's (decision-r10-t10b-r09-t2e §7).
#[test]
fn water_s_scattering_is_zhang_hu_and_he_s_at_zero_salinity() {
    let water = row("H2O");
    for t in [273.16, 283.15, 288.15, 293.15, 298.15, 303.15] {
        let record = liquid_properties(water, Kelvin::new(t), Composition::Reference).unwrap();
        let b = record.scattering_per_m.unwrap();
        for (k, ours) in b.iter().enumerate() {
            let oracle = simpson_mean(
                |nm| zhang_hu_he(nm, t - 273.15),
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
/// state lies below the triple point, where the density is the ancillary's triple-point line.
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
    assert_eq!(record.scattering_per_m, None);
    assert_eq!(record.surface_tension_n_per_m, None);
    assert_eq!(record.viscosity_pa_s, None);
}

/// Nitrogen at its normal boiling point, 77.35 K, lies within 0.5% of 1.20 in the bin across 545
/// nm (decision-r11-palette-liquids §4), from Augustin's measured liquid.
#[test]
fn nitrogen_at_its_boiling_point_is_within_half_a_percent_of_1_20() {
    let record = liquid_properties(row("N2"), Kelvin::new(77.35), Composition::Reference).unwrap();
    let n = record.refractive_index[6];
    assert!((n / 1.20 - 1.0).abs() < 5e-3, "{n}");
    assert_eq!(record.basis, LiquidBasis::Derived);
    assert_eq!(record.absorption_per_m, None);
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
/// accepts, with no absent member written as 0: every present by-bin value is above zero, and a
/// row whose file's k is not modelled leaves its absorption absent.
#[test]
fn every_record_is_a_palette_s_and_no_absent_member_is_zero() {
    for (key, columns) in &super::super::rows::liquids::LIQUIDS {
        let id = row(key.as_str());
        let substance = id.substance();
        let lowest = substance
            .condensed()
            .unwrap()
            .transition()
            .temperature()
            .value();
        let highest = substance
            .phase_diagram()
            .map_or(lowest + 100.0, |d| d.critical_temperature().value() - 1e-6);
        for fraction in [0.0, 0.1, 0.5, 0.9, 1.0] {
            let t = Kelvin::new(lowest + fraction * (highest - lowest));
            let record = liquid_properties(id, t, Composition::Reference).unwrap();
            for bins in [record.absorption_per_m, record.scattering_per_m]
                .into_iter()
                .flatten()
            {
                assert!(
                    bins.iter().all(|v| *v > 0.0),
                    "{key} at {} K: {bins:?}",
                    t.value()
                );
            }
            assert_eq!(
                record.absorption_per_m.is_some(),
                columns.optics.unwrap().absorption.is_some(),
                "{key}"
            );
            let entry = PaletteEntry {
                substance: *key,
                role: PaletteRole::Liquid,
                normal_albedo_bvr: record.fresnel_normal_albedo_bvr(),
                phase_row: 0,
                grain_density_kg_m3: record.density_kg_m3,
                transition: substance.condensed().unwrap().transition().temperature(),
                mechanics: MechanicsFamily::WaterIce,
                liquid: Some(record),
            };
            assert!(
                MaterialPalette::new(vec![entry]).is_ok(),
                "{key} at {} K",
                t.value()
            );
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

/// A temperature outside the liquid range, a composition, and a row with no liquid columns are
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
    let fractions = [(water, 1.0)];
    let composition = LiquidComposition::new(&fractions).unwrap();
    assert!(matches!(
        liquid_properties(water, Kelvin::new(288.15), Composition::Given(composition)),
        Err(ResolveLiquidError::CompositionNotModelled { .. })
    ));
    assert!(matches!(
        liquid_properties(row("CO2"), Kelvin::new(250.0), Composition::Reference),
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
        let lowest = id
            .substance()
            .condensed()
            .unwrap()
            .transition()
            .temperature()
            .value();
        let top = id
            .substance()
            .phase_diagram()
            .map_or(lowest + 100.0, |d| d.critical_temperature().value());
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
    let window = |w: Option<(Kelvin, Kelvin)>| w.map(|(a, b)| (a.value(), b.value()));
    assert_eq!(window(optics.index_checked()), Some((273.16, 373.15)));
    assert_eq!(window(optics.absorption_checked()), Some((273.16, 303.15)));
    let at = |t: f64| {
        liquid_properties(water, Kelvin::new(t), Composition::Reference)
            .unwrap()
            .basis
    };
    assert_eq!(at(303.15), LiquidBasis::Measured);
    assert_eq!(at(303.16), LiquidBasis::Derived);
    assert_eq!(at(400.0), LiquidBasis::Derived);
    for (key, columns) in &super::super::rows::liquids::LIQUIDS {
        if key.as_str() != "H2O" {
            assert_eq!(columns.optics.unwrap().absorption_checked(), None, "{key}");
        }
    }
}

/// Each row's optics state is its file's: within half a kelvin of the temperature the table
/// carries from the file's label, so a file whose temperature changes fails here.
#[test]
fn each_optics_state_is_its_file_s() {
    let files = [
        ("H2O", bins::WATER_TEMPERATURE_K),
        ("CH4", bins::METHANE_LIQUID_TEMPERATURE_K),
        ("NH3", bins::AMMONIA_LIQUID_TEMPERATURE_K),
        ("N2", bins::NITROGEN_LIQUID_TEMPERATURE_K),
        ("C2H6", bins::ETHANE_LIQUID_TEMPERATURE_K),
        ("H2SO4", bins::SULPHURIC_ACID_75_TEMPERATURE_K),
    ];
    for (key, label) in files {
        let optics = row(key).substance().liquid().unwrap().optics().unwrap();
        let state = optics.state().value();
        assert!(
            (state - label).abs() <= 0.5,
            "{key}: state {state} K against {label} K"
        );
    }
}

/// The sulphuric acid holds its 300 K density and index away from 300 K, so it is `Derived`, and
/// its absorption is the file's bound.
#[test]
fn the_acid_holds_its_stated_state() {
    let acid = row("H2SO4");
    let warm = liquid_properties(acid, Kelvin::new(300.0), Composition::Reference).unwrap();
    let cold = liquid_properties(acid, Kelvin::new(250.0), Composition::Reference).unwrap();
    assert_eq!(warm.basis, LiquidBasis::Derived);
    assert_eq!(cold.basis, LiquidBasis::Derived);
    assert_same_bits(cold.density_kg_m3, warm.density_kg_m3);
    for (a, b) in cold.refractive_index.iter().zip(&bins::SULPHURIC_ACID_75_N) {
        assert!((a - b).abs() < 4e-16);
    }
    for (a, file) in cold
        .absorption_per_m
        .unwrap()
        .iter()
        .zip(&bins::SULPHURIC_ACID_75_A)
    {
        assert_same_bits(*a, *file);
    }
}
