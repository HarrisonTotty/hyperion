//! The Milky Way fixture (plan 02, P02.T5.c): measured values, no scatter anywhere but the black
//! hole's, which is the Milky Way's measured offset from the M–σ relation.
//!
//! Every value the plan fixes is set here with its source. Values the plan does not fix, and for
//! which no measurement applies to this model, take the middle of their ranges and say so. The
//! fixture fixes its sizes instead of coupling them to the masses, so that they are the measured
//! ones; plan 02's P02.T11 compares the model built on it with the Milky Way.

use super::accretion::Orbit;
use super::inputs::{
    ArmCount, HaloComponentInput, Inputs, LesserProgenitorInput, RecentProgenitorInput, Size,
};
use crate::galaxy::imf::MassFunctionKind;
use crate::units::{LightYears, Radians};

const GYR: f64 = 1e9;

/// An orbit at the middle of the provisional ranges: apocentre at the geometric middle of
/// 20,000–200,000 ly, pericentre at 0.325 of it, inclination π ÷ 2, node and phase π.
fn middle_orbit() -> Orbit {
    let apocentre = 63_245.553_203_367_59;
    Orbit::new(
        LightYears::new(apocentre),
        LightYears::new(0.325 * apocentre),
        Radians::new(0.5 * core::f64::consts::PI),
        Radians::new(core::f64::consts::PI),
        Radians::new(core::f64::consts::PI),
    )
}

/// The Gaia–Sausage–Enceladus orbit: eccentricity 0.9, the middle of the dominant merger's
/// range (Belokurov et al. 2018, MNRAS 478, 611), with the middle orbit's apocentre of 19.4 kpc,
/// between the debris's apocentres of 16–18 and 30 kpc (Naidu et al. 2021, ApJ 923, 92).
fn gse_orbit() -> Orbit {
    let middle = middle_orbit();
    let e = 0.9;
    Orbit::new(
        middle.apocentre(),
        middle.apocentre() * ((1.0 - e) / (1.0 + e)),
        middle.inclination(),
        middle.node(),
        middle.phase(),
    )
}

/// The fixture's primary values.
pub(super) fn inputs() -> Inputs {
    let middle_halo_age = 11.5 * GYR;
    Inputs {
        mass_function: MassFunctionKind::Kroupa,
        // 6.08 ± 1.14 × 10¹⁰ M☉ (Licquia and Newman 2015, ApJ 806, 96).
        stellar_mass: 6.0e10,
        // The thick disc about a tenth, and the bulge with its bar 31%: "roughly a quarter to 30%"
        // of the stellar mass (Bland-Hawthorn and Gerhard 2016, ARA&A 54, 529), the bar 30% of it
        // (Portail et al. 2017, MNRAS 465, 1621; plan 02, Risks, R1).
        share_thick: 0.10,
        share_bulge_bar: 0.31,
        share_bar_of_bulge: 0.30,
        // About 10⁹ M☉, 1.2–2.4% of the stars (Launhardt et al. 2002, A&A 384, 112; Sormani et
        // al. 2022, MNRAS 512, 1857): 1.05 × 10⁹ of 6 × 10¹⁰.
        share_nuclear_disc: 0.0175,
        // "About 1%" (brainstorm, "Populations").
        share_halo: 0.01,
        // The middle of 5–9 Gyr: a present formation rate about half the past average, against
        // Licquia and Newman's 1.65 ± 0.19 M☉ a year.
        sfh_timescale: 7.0 * GYR,
        // 2.6 ± 0.5 kpc and about 300 pc (Bland-Hawthorn and Gerhard 2016).
        thin_length: Size::Fixed(8_480.0),
        thin_mean_height: 1_000.0,
        // "The young disc's 150 ly height" (brainstorm, "Sizing the layers").
        young_height: 150.0,
        // A thick disc of 2.0 kpc by 0.9 kpc (Bland-Hawthorn and Gerhard 2016).
        thick_length_ratio: 0.77,
        thick_height_ratio: 3.0,
        // 0.70 × 0.44 × 0.25 kpc, a boxy exponential (Wegg and Gerhard 2013, MNRAS 435, 1874).
        bulge_length: Size::Fixed(2_280.0),
        bulge_b_over_a: 1_440.0 / 2_280.0,
        bulge_c_over_a: 820.0 / 2_280.0,
        bulge_boxiness: 3.5,
        // A half-length of 5.0 kpc and a thin-bar scale height of 180 pc (Wegg, Gerhard and
        // Portail 2015, MNRAS 450, 4050); corotation near 6 kpc (Portail et al. 2017).
        bar_length: Size::Fixed(16_000.0),
        bar_width_ratio: 0.10,
        bar_height: 590.0,
        bar_corotation_ratio: 1.2,
        // 88.6 pc by 28.4 pc (Sormani et al. 2022).
        nuclear_length: Size::Fixed(290.0),
        nuclear_height_ratio: 93.0 / 290.0,
        nuclear_cluster_mass_scatter: 0.0,
        // Four arms pitched at about 12° (Bland-Hawthorn and Gerhard 2016).
        arm_count: ArmCount::Four,
        arm_pitch: 12.0,
        // The middle of each range: the arm profile is this model's own.
        arm_young_width: 375.0,
        arm_young_fraction: 0.8,
        arm_old_amplitude: 0.2,
        // "About 15% of the thin disc's" (brainstorm, "Galaxy parameters"); the length ratio is
        // the middle of its range.
        gas_mass_fraction: 0.15,
        gas_length_ratio: 1.75,
        // M₂₀₀ = 1.19 × 10¹² M☉, against 1.3 × 10¹² (McMillan 2017, MNRAS 465, 76).
        dark_f_star: 0.32,
        dark_concentration_scatter: 0.0,
        // Sgr A* is (4.297 ± 0.012) × 10⁶ M☉ (GRAVITY Collaboration 2022, A&A 657, L12). At the
        // fixture's bulge dispersion of 119.3 km/s the M–σ relation of McConnell and Ma (2013,
        // ApJ 764, 184) gives 1.13 × 10⁷ M☉; the Milky Way lies 0.421 dex below it, 1.1 times the
        // relation's intrinsic scatter, and this offset puts the fixture's black hole at 4.30 ×
        // 10⁶ M☉. It holds for this σ only: when plan 08 replaces the σ estimator, it is set
        // again.
        bh_scatter: -0.421,
        // "About −0.05 dex per kpc in the Milky Way disc" (brainstorm, "Fields").
        metallicity_gradient: -0.05,
        // The halo's components and the accretion history at the middle of their ranges.
        halo_in_situ: HaloComponentInput {
            share: 0.225,
            flattening: 0.5,
            core: 2_250.0,
            slope: 3.5,
            age_centre: middle_halo_age,
        },
        halo_dominant: HaloComponentInput {
            share: 0.475,
            flattening: 0.7,
            core: 3_500.0,
            slope: 3.5,
            age_centre: middle_halo_age,
        },
        halo_dominant_break_radius: 65_000.0,
        halo_dominant_break_steepening: 1.5,
        halo_lesser_share_total: 0.175,
        halo_lesser: (0..3)
            .map(|_| LesserProgenitorInput {
                weight: 1.0,
                flattening: 0.8,
                slope: 3.5,
                age_centre: middle_halo_age,
                feh_mean: -1.5,
                accreted: 9.0 * GYR,
                orbit: middle_orbit(),
            })
            .collect(),
        halo_debris: HaloComponentInput {
            share: 0.115,
            flattening: 1.0,
            core: 4_000.0,
            slope: 4.25,
            age_centre: middle_halo_age,
        },
        halo_discrete_share: 0.085,
        // The Gaia–Sausage–Enceladus merger, about 10 Gyr ago (Helmi et al. 2018, Nature 563, 85).
        last_major_merger: 10.0 * GYR,
        dominant_orbit: gse_orbit(),
        // One recent progenitor at the middle of the ranges: 10⁷·²⁵ M☉ accreted 3 Gyr ago.
        recent: vec![RecentProgenitorInput {
            mass: 17_782_794.100_389_23,
            accreted: 3.0 * GYR,
            orbit: middle_orbit(),
        }],
        globular_count_scatter: 0.0,
    }
}
