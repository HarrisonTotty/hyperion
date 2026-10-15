//! The synthetic palettes' liquid records (decision-r11-palette-liquids §1 and §4): liquid water
//! at 25 °C, liquid methane at Titan's surface temperature, and a basaltic melt, each value with
//! its source and each record with its basis.
//!
//! These are test inputs, entered by hand, not the registry's: the server resolves a generated
//! body's records from plan 14's columns (P14.T49.h, P14.T54.a). The optical members are the
//! uniform means over R08's fifteen bake bins of R08's committed material files
//! (`apps/hyperion/src/renderer/src/view/atmosphere/materials/`), each file's 5 nm grid
//! interpolated linearly in n and in a = 4πk ÷ λ, as decision-r11-palette-liquids §1.8 takes them,
//! six significant figures, the files' own. A member left out is not modelled here, never 0.

use hyperion_base::units::Kelvin;

use crate::field::{LIQUID_BINS, LiquidBasis, LiquidProperties};

/// Liquid water at 25 °C and 100 kPa, every member measured, so `Measured`
/// (decision-r11-palette-liquids §1.8's water row).
///
/// - **n:** R08's `water.json`, Hale and Querry 1973 (Appl. Opt. 12, 555) at 25 °C.
/// - **a:** the same file after R08.T5.g: Pope and Fry 1997 (Appl. Opt. 36, 8710) at 22 °C over
///   380–727.5 nm, Hale and Querry's from 750 nm, and log-linear between. In the bin about 443 nm
///   it is 0.007 29 m⁻¹, against Pope and Fry's 0.006 35 at 440 nm (the bin's mean rises to its
///   red edge), and in the bin about 545 nm 0.051 8, against their 0.056 5 at 550 nm. Hale and
///   Querry's own k, which the file held before, gave 0.031 1 and 0.042 3 there.
/// - **R08.T5.h moves both** (decision-r08-t5g.md §1 and §2): n to IAPWS R9-97, by up to
///   4.5 × 10⁻³ at 380 nm, and k to Mason, Cone and Fry 2016 below 500 nm and Kou, Labrie and
///   Chýlek 1993 above 725 nm. These bins are the file's at R08.T5.g; taking T5.h's re-blesses
///   `wire/payloads.golden` at the version in force, with the reference Earth's copy
///   (`hyperion_sim::planetary::surface::reference`) and the normal albedo of the Earth-like
///   world's sea (`testing/worlds.rs`'s `WATER`), the record's Fresnel reflectance.
/// - **Density:** 997.047 kg m⁻³, IAPWS-95's at 298.15 K and 100 kPa (IAPWS R6-95(2018);
///   Wagner and Pruß 2002, J. Phys. Chem. Ref. Data 31, 387): 997.047 0, and 997.047 6 at 1 atm,
///   the same within the formulation's uncertainty, and the pressure of the viscosity's figure.
/// - **Surface tension:** 0.071 97 N m⁻¹, IAPWS R1-76(2014)'s σ = 235.8 mN m⁻¹ τ^1.256
///   (1 − 0.625 τ), τ = 1 − T ÷ 647.096 K, at 298.15 K, against its own vapour.
/// - **Viscosity:** 8.900 × 10⁻⁴ Pa s, IAPWS R12-08's (Huber et al. 2009, J. Phys. Chem. Ref. Data
///   38, 101), its eqs. (10)–(12) at 298.15 K and IAPWS-95's density at 100 kPa: 0.890 02 mPa s.
/// - **Scattering:** not modelled here (P14.T49.h's Einstein–Smoluchowski–Cabannes fills a
///   generated body's).
///
/// A sea warmer than an Earth's mean, about 18 °C at its surface: the temperature is the file's
/// and its index's, at which every member but the absorption was measured. Pope and Fry measured
/// theirs at 22 °C, and the file and this record take it as it stands at 25 °C (R08.T5.g's
/// deferred "one temperature per liquid").
#[must_use]
pub fn liquid_water() -> LiquidProperties {
    LiquidProperties {
        temperature: Kelvin::new(298.15),
        density_kg_m3: LIQUID_WATER_DENSITY_KG_M3,
        basis: LiquidBasis::Measured,
        refractive_index: [
            1.339_609, 1.338_280, 1.337_267, 1.336_253, 1.335_240, 1.334_227, 1.333_256, 1.332_954,
            1.332_237, 1.331_945, 1.331_219, 1.331_000, 1.330_931, 1.330_194, 1.330_000,
        ],
        absorption_per_m: Some([
            0.008_164_55,
            0.004_722_25,
            0.007_288_68,
            0.010_766_7,
            0.017_950_3,
            0.039_037_0,
            0.051_761_7,
            0.073_031_0,
            0.180_294,
            0.277_590,
            0.334_405,
            0.442_138,
            0.601_402,
            1.353_06,
            2.380_73,
        ]),
        scattering_per_m: None,
        surface_tension_n_per_m: Some(0.071_97),
        viscosity_pa_s: Some(8.900e-4),
    }
}

/// [`liquid_water`]'s density, kg m⁻³, which its [`PaletteRole::Liquid`] entry's grain density
/// is.
///
/// [`PaletteRole::Liquid`]: crate::field::PaletteRole::Liquid
pub const LIQUID_WATER_DENSITY_KG_M3: f64 = 997.047;

/// Liquid methane at Titan's surface temperature, 93.65 K (Huygens's HASI at its landing site,
/// 93.65 ± 0.25 K; Fulchignoni et al. 2005, Nature 438, 785), `Derived`
/// (decision-r11-palette-liquids §1.8's methane row, for a sea of methane alone).
///
/// - **Density:** 447.6 kg m⁻³, Setzmann and Wagner 1991's equation of state (J. Phys. Chem. Ref.
///   Data 20, 1061, as the NIST Chemistry WebBook, SRD 69, evaluates it) at 93.65 K and Huygens's
///   1,467 hPa (Fulchignoni et al. 2005): 447.614, against the saturated liquid's 447.525.
/// - **n:** R08's `methane-liquid.json`, Martonchik and Orton 1994 (Appl. Opt. 33, 8306) at 90 K,
///   each bin's mean carried to 93.65 K by Lorentz–Lorenz at constant molar refraction, (n² − 1)
///   ÷ (n² + 2) times 447.6 ÷ 452.4: 452.4 kg m⁻³ is the saturated-liquid line extrapolated to
///   90 K, 0.69 K below the triple point (90.694 K), an extrapolation, so the record is `Derived`.
///   About 0.0034 lower, 1.2903 in the bin about 545 nm.
/// - **Not modelled here:** the absorption (the file holds k = 0 at 380–400 nm, below its source's
///   sensitivity with no bound stated, and its visible k is indicative between band extrema), the
///   scattering, the surface tension and the viscosity.
#[must_use]
pub fn liquid_methane_titan() -> LiquidProperties {
    LiquidProperties {
        temperature: Kelvin::new(93.65),
        density_kg_m3: LIQUID_METHANE_TITAN_DENSITY_KG_M3,
        basis: LiquidBasis::Derived,
        refractive_index: [
            1.296_516, 1.295_764, 1.294_671, 1.293_578, 1.292_486, 1.291_394, 1.290_300, 1.289_209,
            1.288_562, 1.287_754, 1.287_153, 1.286_718, 1.286_668, 1.286_660, 1.286_355,
        ],
        absorption_per_m: None,
        scattering_per_m: None,
        surface_tension_n_per_m: None,
        viscosity_pa_s: None,
    }
}

/// [`liquid_methane_titan`]'s density, kg m⁻³.
pub const LIQUID_METHANE_TITAN_DENSITY_KG_M3: f64 = 447.6;

/// A basaltic melt at 1,200 °C, a lava's, on a basalt crust entry, `StandIn`
/// (decision-r11-palette-liquids §1.8's molten silicate row).
///
/// - **n and a:** R08's `basaltic-glass.json`, Pollack, Toon and Khare 1973 (Icarus 19, 372), a
///   room-temperature glass standing in for the melt: about 10⁴ m⁻¹, opaque at a millimetre.
/// - **Density:** 2,700 kg m⁻³, a stated constant near a tholeiitic melt's at 1,200 °C and 1 bar,
///   not the solid's grain density: about 2,690–2,750 kg m⁻³ for tholeiites there by the partial
///   molar volumes of Lange 1997 and Lange and Carmichael 1987 (Geochim. Cosmochim. Acta 51, 2931)
///   as `DensityX` (Iacovino and Till 2019, Volcanica 2, 1) combines them, at Fe³⁺/ΣFe 0.1.
/// - **Not modelled here:** the scattering, the surface tension and the viscosity.
#[must_use]
pub fn basaltic_melt() -> LiquidProperties {
    LiquidProperties {
        temperature: Kelvin::new(1_473.15),
        density_kg_m3: 2_700.0,
        basis: LiquidBasis::StandIn,
        refractive_index: [
            1.570_000, 1.570_000, 1.570_000, 1.570_000, 1.569_965, 1.569_227, 1.568_213, 1.567_200,
            1.566_187, 1.565_173, 1.564_160, 1.563_147, 1.562_133, 1.561_120, 1.560_107,
        ],
        absorption_per_m: Some(BASALTIC_GLASS_ABSORPTION_PER_M),
        scattering_per_m: None,
        surface_tension_n_per_m: None,
        viscosity_pa_s: None,
    }
}

/// The basaltic glass's absorption by bin, m⁻¹ ([`basaltic_melt`]): the file extrapolates its
/// k over 380–400 nm from its 400 and 420 nm rows, as its own reduction states.
const BASALTIC_GLASS_ABSORPTION_PER_M: [f64; LIQUID_BINS] = [
    27_451.2, 19_827.5, 15_333.2, 13_571.3, 12_378.3, 11_384.0, 10_562.0, 9_813.11, 9_151.09,
    8_868.34, 8_714.66, 8_576.86, 8_457.78, 8_399.9, 8_365.92,
];

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// Water's surface tension is IAPWS R1-76(2014)'s at 25 °C, to its printed 0.01 mN m⁻¹.
    #[test]
    fn the_synthetic_water_s_surface_tension_is_iapws_r1_76() {
        let tau = 1.0 - 298.15 / 647.096;
        let sigma = 235.8e-3 * hyperion_base::math::powf(tau, 1.256) * (1.0 - 0.625 * tau);
        let held = liquid_water().surface_tension_n_per_m.unwrap();
        assert!((sigma - held).abs() < 5e-6, "{sigma} N/m against {held}");
    }

    /// Each record's index falls with wavelength (normal dispersion) and lies where its liquid's
    /// does: water's about 1.33, methane's about 1.29, a basaltic glass's about 1.57. Water's
    /// bounds hold IAPWS R9-97's 1.345 at 380 nm too, which R08.T5.h brings to `water.json`.
    #[test]
    fn the_synthetic_liquids_indices_fall_with_wavelength() {
        for (record, low, high) in [
            (liquid_water(), 1.325, 1.350),
            (liquid_methane_titan(), 1.285, 1.297),
            (basaltic_melt(), 1.559, 1.571),
        ] {
            let n = record.refractive_index;
            assert!(n.windows(2).all(|w| w[1] <= w[0]), "{n:?}");
            assert!(n.iter().all(|v| (low..high).contains(v)), "{n:?}");
        }
    }
}
