//! The empirical check of the colour table: Pickles' (1998, PASP 110, 863) library, integrated by
//! the task's own code and compared with the table at each type's `T_eff` and log g (R06.T3.b).

use hyperion_sim::math;

use super::photometry::{Observer, uv_prime};
use super::spectrum::{ReadSpectrumError, Spectrum};
use super::{Grid, StarColourTable};

/// One type of the comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PicklesType {
    /// Pickles' file.
    pub file: &'static str,
    /// The type's effective temperature, K.
    pub teff: f64,
    /// Its log₁₀ g (cgs).
    pub log_g: f64,
    /// Whether it is read from the giants' side of the grid (the same grid; kept for the label).
    pub giant: bool,
    /// The largest Δ(u′, v′) allowed.
    pub limit: f64,
    /// Why the limit is not 0.005, if it is not.
    pub exception: Option<&'static str>,
}

/// The types compared, and their `T_eff` and log g.
///
/// - Dwarfs: `T_eff` from Mamajek's table of 2022.04.16 (after Pecaut and Mamajek 2013, ApJS 208,
///   9), log g from its masses and radii (A0 V 4.09, F5 V 4.23, G2 V 4.44, K0 V 4.56, M2 V 4.81).
/// - O5 V: Martins, Schaerer and Hillier (2005, A&A 436, 1049), Table 1, the theoretical scale.
/// - Giants: Pickles' own adopted `T_eff` (the library's `synphot.dat`, `LogTe`), and gravities
///   typical of the class (G8 III and K0 III 2.6–2.7, M3 III 1.1, about 1.3 M☉ at 70 R☉), which
///   are assumed, not sourced; the colours barely depend on them.
///
/// The limit is the plan's 0.005 but for four measured exceptions (ruled 2026-10-02, R06's Risks).
pub const PICKLES_TYPES: [PicklesType; 9] = [
    PicklesType {
        file: "uka0v.dat",
        teff: 9_700.0,
        log_g: 4.09,
        giant: false,
        limit: 0.005,
        exception: None,
    },
    PicklesType {
        file: "ukf5v.dat",
        teff: 6_550.0,
        log_g: 4.23,
        giant: false,
        limit: 0.005,
        exception: None,
    },
    PicklesType {
        file: "ukg2v.dat",
        teff: 5_770.0,
        log_g: 4.44,
        giant: false,
        limit: 0.005,
        exception: None,
    },
    PicklesType {
        file: "ukk0v.dat",
        teff: 5_270.0,
        log_g: 4.56,
        giant: false,
        limit: 0.005,
        exception: None,
    },
    PicklesType {
        file: "ukg8iii.dat",
        teff: 5_012.0,
        log_g: 2.7,
        giant: true,
        limit: 0.005,
        exception: None,
    },
    PicklesType {
        file: "ukm2v.dat",
        teff: 3_560.0,
        log_g: 4.81,
        giant: false,
        limit: 0.007,
        exception: Some(
            "it lies 0.006 from the table at its type's 3,560 K and matches a 3,260 K model",
        ),
    },
    PicklesType {
        file: "ukk0iii.dat",
        teff: 4_853.0,
        log_g: 2.6,
        giant: true,
        limit: 0.006,
        exception: Some(
            "it lies 0.005 from the table at its adopted 4,853 K and matches a 5,050 K model",
        ),
    },
    PicklesType {
        file: "uko5v.dat",
        teff: 41_540.0,
        log_g: 3.92,
        giant: false,
        limit: 0.008,
        exception: Some(
            "it lies about 0.007 off the models' locus at every temperature, likely the residual reddening of its source stars",
        ),
    },
    PicklesType {
        file: "ukm3iii.dat",
        teff: 3_631.0,
        log_g: 1.1,
        giant: true,
        limit: 0.012,
        exception: Some("its colour is a 4,240 K model's, not one near its adopted 3,631 K"),
    },
];

/// Reads a Pickles file: fixed columns (the catalogue's `ReadMe`), Å in bytes 1–7 and `f_λ` ÷
/// f(5556 Å) in bytes 8–17; a negative flux, which the library's noisiest ends hold, is read as 0.
///
/// # Errors
///
/// [`ReadSpectrumError`] for a line that is not of that form, or as [`Spectrum::new`].
pub fn parse_pickles(text: &str) -> Result<Spectrum, ReadSpectrumError> {
    let mut wavelength_nm = Vec::new();
    let mut flux = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let w: f64 = line
            .get(..7)
            .and_then(|f| f.trim().parse().ok())
            .ok_or_else(|| ReadSpectrumError::new("a Pickles wavelength is malformed"))?;
        let f: f64 = line
            .get(7..17)
            .and_then(|f| f.trim().parse().ok())
            .ok_or_else(|| ReadSpectrumError::new("a Pickles flux is malformed"))?;
        wavelength_nm.push(w / 10.0);
        flux.push(f.max(0.0));
    }
    Spectrum::new(wavelength_nm, flux)
}

/// The (u′, v′) of `grid`'s colour at `teff` and `log_g`, interpolated as the sim's `sky::colour`
/// interpolates the committed table: bilinearly in log₁₀ `T_eff` and log₁₀ g, clamped.
#[must_use]
pub fn table_uv(
    observer: &Observer,
    grid: &Grid,
    luminance: [f64; 3],
    teff: f64,
    log_g: f64,
) -> [f64; 2] {
    let log_teff: Vec<f64> = grid
        .teff
        .iter()
        .map(|&(t, _)| math::log10(f64::from(t)))
        .collect();
    let (ti, tf) = bracket(&log_teff, math::log10(teff));
    let (gi, gf) = bracket(&grid.log_g, log_g);
    let mix = |field: fn(&super::ColourRow) -> f64| {
        let low =
            field(&grid.row(ti, gi).colour) * (1.0 - gf) + field(&grid.row(ti, gi + 1).colour) * gf;
        let high = field(&grid.row(ti + 1, gi).colour) * (1.0 - gf)
            + field(&grid.row(ti + 1, gi + 1).colour) * gf;
        low * (1.0 - tf) + high * tf
    };
    let red = mix(|row| row.r);
    let green = mix(|row| row.g);
    let blue = (1.0 - luminance[0] * red - luminance[1] * green) / luminance[2];
    uv_prime(observer.rgb_to_xyz([red, green, blue]))
}

/// The interval of rising `nodes` holding `x`, and the clamped fraction along it.
pub(crate) fn bracket(nodes: &[f64], x: f64) -> (usize, f64) {
    let last = nodes.len() - 2;
    let i = nodes
        .partition_point(|&n| n <= x)
        .saturating_sub(1)
        .min(last);
    let t = (x - nodes[i]) / (nodes[i + 1] - nodes[i]);
    (i, t.clamp(0.0, 1.0))
}

/// Δ(u′, v′) of each type: its spectrum's against the table's at its `T_eff` and log g.
#[must_use]
pub fn compare(observer: &Observer, table: &StarColourTable, spectra: &[Spectrum]) -> Vec<f64> {
    PICKLES_TYPES
        .iter()
        .zip(spectra)
        .map(|(ty, spectrum)| {
            let uv = uv_prime(observer.integrals(&spectrum.bin_means()).xyz);
            let at = table_uv(observer, &table.normal, table.luminance, ty.teff, ty.log_g);
            math::hypot(uv[0] - at[0], uv[1] - at[1])
        })
        .collect()
}
