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
/// accepts, with no absent member written as 0: every present by-bin value is above zero, a row
/// whose file's k is not modelled leaves its absorption absent, and a solution with a component
/// whose absorption is not modelled leaves its own absent (Titan's seas: ethane's and nitrogen's;
/// ammonia–water: ammonia's; the sulphate brine: its salt's), so that of the solutions only the
/// chloride brine, its salt's transparency cited, and the acid, its files' bounds, have one. The
/// perchlorate brines, whose record is refused for its index, are the solutions' tests'.
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

/// A temperature outside the liquid range, a composition naming another row, a composition outside
/// the model's range (the pure acid), a composition of a liquid whose mixing is not yet modelled (a
/// perchlorate brine's), and a row with no liquid columns are refused, each by its own error.
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
        if let (Some(optics), false) = (columns.optics, key.as_str() == "H2O") {
            assert_eq!(optics.absorption_checked(), None, "{key}");
        }
    }
}

/// Each row's optics state is its file's: within half a kelvin of the temperature the table
/// carries from the file's label, so a file whose temperature changes fails here (the acid's two
/// files are the solutions' tests').
#[test]
fn each_optics_state_is_its_file_s() {
    let files = [
        ("H2O", bins::WATER_TEMPERATURE_K),
        ("CH4", bins::METHANE_LIQUID_TEMPERATURE_K),
        ("NH3", bins::AMMONIA_LIQUID_TEMPERATURE_K),
        ("N2", bins::NITROGEN_LIQUID_TEMPERATURE_K),
        ("C2H6", bins::ETHANE_LIQUID_TEMPERATURE_K),
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

/// Setzmann and Wagner 1991's methane reproduces their Table 39's `κ_T` at the saturated liquid
/// (90.694, 100 and 120 K) within 2 × 10⁻⁴, the reach of its five printed figures, and the
/// WebBook's evaluation of the same equation within 10⁻⁶.
#[test]
fn setzmann_and_wagner_s_methane_reproduces_its_table_39() {
    let methane = &super::super::rows::equations::METHANE;
    assert_compressibility(
        "CH4 Table 39",
        methane,
        &[
            (90.694, 451.48, 2.1677, 3.3678, 1_538.6),
            (100.0, 438.89, 2.1136, 3.4084, 1_452.0),
            (120.0, 409.90, 2.0196, 3.5493, 1_253.5),
        ],
        2e-4,
    );
    assert_compressibility(
        "CH4 WebBook",
        methane,
        &[
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
        ],
        1e-6,
    );
}

/// Span et al. 2000's nitrogen reproduces their saturation table's `κ_T` (p. 1403: 63.151, 70 and
/// 77 K) within 3 × 10⁻⁴, the reach of its four printed figures, and the WebBook's within 10⁻⁶.
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
    assert_compressibility(
        "N2 WebBook",
        nitrogen,
        &[
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
        ],
        1e-6,
    );
}

/// Span and Wagner 1996's carbon dioxide reproduces their Table 34's `κ_T` at five saturated-liquid
/// states from the triple point to 300 K within 10⁻⁴, and its pressure there at 300 K, 4.1 K below
/// the critical point, where the non-analytic terms bear, within 10⁻⁵; and the WebBook's `κ_T`
/// within 10⁻⁶.
#[test]
fn span_and_wagner_s_carbon_dioxide_reproduces_its_table_34() {
    let co2 = &super::super::rows::equations::CARBON_DIOXIDE;
    assert_compressibility(
        "CO2 Table 34",
        co2,
        &[
            (216.592, 1_178.46, 0.974_66, 1.953_2, 975.85),
            (250.0, 1_045.97, 0.936_43, 2.132_0, 731.78),
            (270.0, 945.83, 0.939_59, 2.453_4, 565.46),
            (290.0, 804.67, 0.993_73, 3.675_6, 371.95),
            (300.0, 679.24, 1.119_9, 8.697_9, 245.67),
        ],
        1e-4,
    );
    let p = co2.pressure(KilogramsPerCubicMetre::new(679.24), Kelvin::new(300.0));
    assert!(
        (p.value() / 6.713_1e6 - 1.0).abs() < 1e-5,
        "{} Pa",
        p.value()
    );
    assert_compressibility(
        "CO2 WebBook",
        co2,
        &[
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
        ],
        1e-6,
    );
}

/// Bücker and Wagner 2006's ethane, its paper not read, reproduces the WebBook's evaluation of the
/// same equation within 3 × 10⁻⁶ from the triple point to 296.7 K.
#[test]
fn bucker_and_wagner_s_ethane_reproduces_the_webbook() {
    assert_compressibility(
        "C2H6 WebBook",
        &super::super::rows::equations::ETHANE,
        &[
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
        ],
        3e-6,
    );
}

/// Gao et al. 2023's ammonia, its associating terms included, reproduces their Table 7's values for
/// computer verification, pressure and `κ_T`, within 2 × 10⁻⁶ (seven printed figures) at eight
/// liquid, vapour and supercritical states.
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
    let co2 = super::super::rows::liquids::CARBON_DIOXIDE
        .viscosity
        .unwrap();
    let co2_basis = |t: f64, rho: f64| {
        co2.viscosity_pa_s(Kelvin::new(t), KilogramsPerCubicMetre::new(rho))
            .1
    };
    assert_eq!(co2_basis(302.0, 450.0), LiquidBasis::Derived);
    assert_eq!(co2_basis(302.0, 650.0), LiquidBasis::Measured);
    assert_eq!(co2_basis(250.0, 1_046.0), LiquidBasis::Measured);
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
/// sum falls below 0 within about 0.18 K of its critical point, and is present 1 K below it.
#[test]
fn a_negative_surface_tension_is_absent() {
    let tension = |t: f64| {
        liquid_properties(row("CH4"), Kelvin::new(t), Composition::Reference)
            .unwrap()
            .surface_tension_n_per_m
    };
    assert_eq!(tension(190.5), None);
    let below = tension(189.564).unwrap();
    assert!(below > 0.0, "{below} N/m");
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
    let co2 = &super::super::rows::liquids::CARBON_DIOXIDE;
    for (key, ruled) in [
        ("CH4", 0.025),
        ("N2", 0.38),
        ("CO2", 0.46),
        ("C2H6", 0.29),
        ("NH3", 0.07),
    ] {
        let columns = if key == "CO2" {
            co2
        } else {
            row(key).substance().liquid().unwrap()
        };
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

/// Mulero et al. 2012's surface tensions, with each reference equation's critical temperature,
/// reproduce the WebBook's saturated-liquid column within 2 × 10⁻⁶ at three states each.
#[test]
fn mulero_s_surface_tensions_reproduce_the_webbook() {
    let co2 = &super::super::rows::liquids::CARBON_DIOXIDE;
    for (key, rows) in [
        (
            "CH4",
            [
                (90.694_057, 0.017_062_798),
                (143.794_26, 0.006_878_156_0),
                (169.726_3, 0.002_559_923_6),
            ],
        ),
        (
            "N2",
            [
                (63.151_008, 0.012_205_148),
                (95.845_1, 0.004_908_255_9),
                (112.654_57, 0.001_795_175_3),
            ],
        ),
        (
            "C2H6",
            [
                (90.367_641, 0.031_663_374),
                (234.486_41, 0.008_534_154_2),
                (273.999_22, 0.003_122_508_5),
            ],
        ),
        (
            "NH3",
            [
                (195.492_05, 0.043_947_790),
                (320.545_21, 0.015_481_965),
                (368.330_58, 0.005_701_248_7),
            ],
        ),
        (
            "CO2",
            [
                (216.591_36, 0.016_494_627),
                (247.749_6, 0.009_500_193_8),
                (277.934_57, 0.003_632_893_5),
            ],
        ),
    ] {
        let columns = if key == "CO2" {
            co2
        } else {
            row(key).substance().liquid().unwrap()
        };
        let tension = columns.surface_tension().unwrap();
        for (t, webbook) in rows {
            let ours = tension.surface_tension_n_per_m(Kelvin::new(t));
            assert!(
                (ours / webbook - 1.0).abs() < 2e-6,
                "{key} at {t} K: {ours} N/m against {webbook}"
            );
        }
    }
}

/// Carbon dioxide's columns are built but wait on its index (decision-p14-t49h-gas-index §1.3): the
/// row answers `NotALiquid`, and its columns give b, σ and μ at its triple point.
#[test]
fn carbon_dioxide_waits_on_its_index() {
    assert!(matches!(
        liquid_properties(row("CO2"), Kelvin::new(250.0), Composition::Reference),
        Err(ResolveLiquidError::NotALiquid { .. })
    ));
    let co2 = &super::super::rows::liquids::CARBON_DIOXIDE;
    assert!(co2.optics().is_none());
    assert_eq!(co2.components(), [row("CO2")]);
    let t = Kelvin::new(216.592);
    let rho = row("CO2")
        .substance()
        .grain_density(CondensedPhase::Liquid, t)
        .unwrap();
    let (mu, basis) = co2.viscosity().unwrap().viscosity_pa_s(t, rho);
    // About 0.253 mPa s at the triple point (the WebBook's 253.37 μPa s at its own density).
    assert!((mu * 1e6 - 253.4).abs() < 2.0, "{mu}");
    assert_eq!(basis, LiquidBasis::Measured);
    let sigma = co2.surface_tension().unwrap().surface_tension_n_per_m(t);
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
