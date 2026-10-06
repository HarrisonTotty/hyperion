//! What the eye and the V band see of a spectrum: the CIE integrals, the display chroma, the
//! scotopic-to-photopic ratio and the illuminance of a star of V = 0 (rendering plan R06, Design
//! note 6).
//!
//! Every integral is a sum over the table's 1 nm bins of the spectrum's bin mean times the
//! weighting function at the bin's centre, which is how the CIE recommends its 1 nm tables be used
//! (CIE 015:2018, §7.1).

use std::fmt;

use super::spectrum::{BIN_COUNT, FIRST_BIN_NM, bin_centre_nm};

/// The maximum luminous efficacy of photopic vision, lm W⁻¹ (CIE 018:2019; the SI's `K_cd`
/// definition gives 683.002 at 555 nm, of which 683 is the CIE's rounding).
pub const K_M: f64 = 683.0;

/// The maximum luminous efficacy of scotopic vision, lm W⁻¹ (CIE 018:2019: 1,700, the 507 nm
/// peak of V′ scaled to `K_cd` at 555 nm).
pub const K_M_SCOTOPIC: f64 = 1_700.0;

/// The illuminance of a star of V = 0 outside an atmosphere, lx: Allen's 2.54 µlx (Allen 1973,
/// *Astrophysical Quantities*, 3rd ed., p. 197; Crumey 2014, MNRAS 442, 2600, §1.3, after Cox 1999),
/// the zero point rendering plan R02 uses (`V0_ILLUMINANCE_LX`), which `lux_per_v0` is a ratio
/// to.
pub const V0_ILLUMINANCE_LX: f64 = 2.54e-6;

/// The V band's zero point: the photon-weighted mean flux density of a star of V = 0,
/// W m⁻² nm⁻¹: Bessell, Castelli and Plez (1998, A&A 333, 231, Appendix A.1, eq. A1, and Table A2's
/// V offset 0.000), V = −2.5 log₁₀ ⟨`f_λ`⟩ − 21.100 with ⟨`f_λ`⟩ in erg cm⁻² s⁻¹ Å⁻¹, so
/// 10^(−8.44) erg cm⁻² s⁻¹ Å⁻¹ = 3.631 × 10⁻⁹, which is 3.631 × 10⁻¹¹ W m⁻² nm⁻¹. It is applied to
/// Bessell and Murphy's (2012, PASP 124, 140) photonic V response, whose own zero point (§7.2,
/// Table 3, −0.019, and Table 5, +0.003) would put V = 0 0.016 mag brighter, 1.5% more flux; kept
/// at BCP98's by the ruling of 2026-10-02 (R06's Risks), so every `lux_per_v0` is 0.016 mag lower
/// than under BM12's own.
pub const V0_FLUX_W_M2_NM: f64 = 3.630_780_547_701_014e-11;

/// The chromaticities of the Rec. 709 primaries (ITU-R BT.709-6, Part 1, item 1.3).
const PRIMARIES_XY: [[f64; 2]; 3] = [[0.64, 0.33], [0.30, 0.60], [0.15, 0.06]];

/// The Rec. 709 white, D65 (ITU-R BT.709-6, Part 1, item 1.4).
const WHITE_XY: [f64; 2] = [0.3127, 0.3290];

/// Where the observer's V band comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VBandSource<'a> {
    /// Bessell and Murphy's (2012) Table 1, its text.
    BessellMurphy(&'a str),
    /// The CIE 1924 V(λ), photon-weighted: the stand-in from committed data alone.
    CiePhotopic,
}

/// The weighting functions at each bin's centre.
#[derive(Debug, Clone, PartialEq)]
pub struct Observer {
    /// x̄, ȳ and z̄ of the CIE 1931 2° observer, by bin; zero outside 360–830 nm.
    xyz: Vec<[f64; 3]>,
    /// V(λ), CIE 1924 photopic, by bin.
    photopic: Vec<f64>,
    /// V′(λ), CIE 1951 scotopic, by bin; zero outside 380–780 nm.
    scotopic: Vec<f64>,
    /// Bessell and Murphy's photonic V response times the bin's wavelength, by bin.
    v_photons: Vec<f64>,
    /// D65's relative power, by bin, for the white point's check.
    d65: Vec<f64>,
    /// Linear Rec. 709 from XYZ.
    to_rgb: [[f64; 3]; 3],
}

/// What a spectrum gives the table (Design note 6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColourRow {
    /// Linear Rec. 709 red of the spectrum at unit luminance, desaturated into gamut.
    pub r: f64,
    /// Linear Rec. 709 green, likewise; blue is (1 − 0.2126 r − 0.7152 g) ÷ 0.0722.
    pub g: f64,
    /// The photopic illuminance of a star of this spectrum at V = 0, over [`V0_ILLUMINANCE_LX`].
    pub lux_per_v0: f64,
    /// The scotopic-to-photopic ratio ρ = 1,700 ∫S V′ dλ ÷ (683 ∫S V dλ).
    pub sp_ratio: f64,
}

/// The integrals of one spectrum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Integrals {
    /// ∫S x̄, ∫S ȳ, ∫S z̄, over 1 nm bins.
    pub xyz: [f64; 3],
    /// ∫S V.
    pub photopic: f64,
    /// ∫S V′.
    pub scotopic: f64,
    /// ⟨`f_λ`⟩ in the V band, photon-weighted: ∫S R λ ÷ ∫R λ.
    pub v_mean_flux: f64,
}

impl Observer {
    /// Reads the CIE tables and Bessell and Murphy's Table 1.
    ///
    /// Without Table 1 the V band is stood in for by the photopic V(λ), photon-weighted: the smoke
    /// run's, which needs only committed data, since Table 1 is fetched.
    ///
    /// # Errors
    ///
    /// [`ReadObserverError`] if a table is malformed.
    pub fn read(
        xyz_csv: &str,
        photopic_csv: &str,
        scotopic_csv: &str,
        d65_csv: &str,
        v_band: VBandSource<'_>,
    ) -> Result<Self, ReadObserverError> {
        let xyz_rows = csv_rows(xyz_csv, 3)?;
        let mut xyz = vec![[0.0; 3]; BIN_COUNT];
        for (nm, values) in &xyz_rows {
            if let Some(slot) = bin_of(*nm) {
                xyz[slot] = [values[0], values[1], values[2]];
            }
        }
        let column = |text: &str| -> Result<Vec<f64>, ReadObserverError> {
            let mut out = vec![0.0; BIN_COUNT];
            for (nm, values) in csv_rows(text, 1)? {
                if let Some(slot) = bin_of(nm) {
                    out[slot] = values[0];
                }
            }
            Ok(out)
        };
        let photopic = column(photopic_csv)?;
        let scotopic = column(scotopic_csv)?;
        let d65 = column(d65_csv)?;
        let v_photons = match v_band {
            VBandSource::BessellMurphy(table) => {
                let response = bessell_v(table)?;
                (0..BIN_COUNT)
                    .map(|i| {
                        let nm = bin_centre_nm(i);
                        interpolate(&response, nm) * nm
                    })
                    .collect()
            }
            VBandSource::CiePhotopic => (0..BIN_COUNT)
                .map(|i| photopic[i] * bin_centre_nm(i))
                .collect(),
        };
        Ok(Self {
            xyz,
            photopic,
            scotopic,
            v_photons,
            d65,
            to_rgb: xyz_to_rec709(),
        })
    }

    /// The integrals of a spectrum's bin means.
    #[must_use]
    pub fn integrals(&self, bins: &[f64]) -> Integrals {
        let mut xyz = [0.0; 3];
        let mut photopic = 0.0;
        let mut scotopic = 0.0;
        let mut photons = 0.0;
        let mut photon_weight = 0.0;
        for (i, &s) in bins.iter().enumerate() {
            for (sum, w) in xyz.iter_mut().zip(self.xyz[i]) {
                *sum += s * w;
            }
            photopic += s * self.photopic[i];
            scotopic += s * self.scotopic[i];
            photons += s * self.v_photons[i];
            photon_weight += self.v_photons[i];
        }
        Integrals {
            xyz,
            photopic,
            scotopic,
            v_mean_flux: photons / photon_weight,
        }
    }

    /// The table's row for a spectrum's bin means.
    #[must_use]
    pub fn row(&self, bins: &[f64]) -> ColourRow {
        let integrals = self.integrals(bins);
        let [r, g, _] = self.unit_rgb(integrals.xyz);
        ColourRow {
            r,
            g,
            // E = 683 ∫S V dλ for S scaled to ⟨f_λ⟩ = the V = 0 flux; the bins are 1 nm wide.
            lux_per_v0: K_M * V0_FLUX_W_M2_NM * integrals.photopic
                / integrals.v_mean_flux
                / V0_ILLUMINANCE_LX,
            sp_ratio: K_M_SCOTOPIC * integrals.scotopic / (K_M * integrals.photopic),
        }
    }

    /// Linear Rec. 709 of `xyz` at unit luminance, desaturated towards the white of the same
    /// luminance until no channel is negative.
    ///
    /// Out of gamut the colour is mixed with white (r = g = b = 1 at Y = 1) by the least amount
    /// that lifts its lowest channel to zero: Walker's (1996) rule of adding white, "Colour
    /// Rendering of Spectra", at constant luminance, so the hue is kept and no channel is clipped
    /// on its own.
    #[must_use]
    pub fn unit_rgb(&self, xyz: [f64; 3]) -> [f64; 3] {
        let y = xyz[1];
        let unit = xyz.map(|v| v / y);
        let rgb = self
            .to_rgb
            .map(|row| row[0] * unit[0] + row[1] * unit[1] + row[2] * unit[2]);
        let lowest = rgb[0].min(rgb[1]).min(rgb[2]);
        if lowest >= 0.0 {
            rgb
        } else {
            let t = 1.0 / (1.0 - lowest);
            rgb.map(|c| 1.0 + t * (c - 1.0))
        }
    }

    /// Each bin's Rec. 709 colour-matching functions: the matrix applied to x̄, ȳ and z̄.
    #[must_use]
    pub fn rgb_cmf(&self) -> Vec<[f64; 3]> {
        self.xyz
            .iter()
            .map(|c| {
                self.to_rgb
                    .map(|row| row[0] * c[0] + row[1] * c[1] + row[2] * c[2])
            })
            .collect()
    }

    /// Bessell and Murphy's photonic V response times the wavelength, by bin (the photopic V(λ)
    /// stand-in for the smoke run).
    #[must_use]
    pub fn v_photons(&self) -> &[f64] {
        &self.v_photons
    }

    /// The CIE 1924 V(λ) by 1 nm bin.
    #[must_use]
    pub fn photopic(&self) -> &[f64] {
        &self.photopic
    }

    /// The CIE 1951 V′(λ) by 1 nm bin, zero outside 380–780 nm.
    #[must_use]
    pub fn scotopic(&self) -> &[f64] {
        &self.scotopic
    }

    /// The mean of V(λ) over each bake bin, V(λ) taken as linear between its 1 nm samples.
    ///
    /// # Panics
    ///
    /// Never: V(λ) on the bins, padded with zeros, is a valid curve.
    #[must_use]
    pub fn photopic_bin_means(&self) -> [f64; super::columns::BAKE_WAVELENGTH_COUNT] {
        let nm: Vec<f64> = (0..BIN_COUNT).map(bin_centre_nm).collect();
        let curve = super::spectrum::Spectrum::new(
            std::iter::once(f64::from(FIRST_BIN_NM) - 1.0)
                .chain(nm.iter().copied())
                .chain(std::iter::once(bin_centre_nm(BIN_COUNT - 1) + 1.0))
                .collect(),
            std::iter::once(0.0)
                .chain(self.photopic.iter().copied())
                .chain(std::iter::once(0.0))
                .collect(),
        )
        .expect("V(λ) on the bins is a valid curve");
        std::array::from_fn(|k| {
            let (a, b) = super::columns::bake_bin(k);
            curve.integral(a, b) / (b - a)
        })
    }

    /// The luminance Y of unit linear Rec. 709 red, green and blue: the middle row of the matrix
    /// from Rec. 709 to XYZ, about 0.2126, 0.7152 and 0.0722.
    #[must_use]
    pub fn luminance(&self) -> [f64; 3] {
        invert(self.to_rgb)[1]
    }

    /// The XYZ of linear Rec. 709 `rgb`.
    #[must_use]
    pub fn rgb_to_xyz(&self, rgb: [f64; 3]) -> [f64; 3] {
        invert(self.to_rgb).map(|row| row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2])
    }

    /// D65's bin values, for the white point's check.
    #[must_use]
    pub fn d65(&self) -> &[f64] {
        &self.d65
    }
}

/// The CIE 1976 (u′, v′) of `xyz`.
#[must_use]
pub fn uv_prime(xyz: [f64; 3]) -> [f64; 2] {
    let d = xyz[0] + 15.0 * xyz[1] + 3.0 * xyz[2];
    [4.0 * xyz[0] / d, 9.0 * xyz[1] / d]
}

/// The bin of a CIE wavelength, if it is one of the table's.
#[must_use]
fn bin_of(nm: u32) -> Option<usize> {
    let i = usize::try_from(nm.checked_sub(FIRST_BIN_NM)?).ok()?;
    (i < BIN_COUNT).then_some(i)
}

/// The rows of a CIE CSV file: an integer wavelength in nm and `columns` values.
fn csv_rows(text: &str, columns: usize) -> Result<Vec<(u32, Vec<f64>)>, ReadObserverError> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let mut fields = line.trim().split(',');
            let nm = fields
                .next()
                .and_then(|f| f.trim().parse::<u32>().ok())
                .ok_or(ReadObserverError("a CIE wavelength is not a whole number"))?;
            let values = fields
                .map(|f| f.trim().parse::<f64>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| ReadObserverError("a CIE value is not a number"))?;
            if values.len() == columns {
                Ok((nm, values))
            } else {
                Err(ReadObserverError(
                    "a CIE row has the wrong number of columns",
                ))
            }
        })
        .collect()
}

/// The V column of Bessell and Murphy's Table 1 as (nm, response), from its fixed byte columns
/// 23–26 (wavelength in 0.1 nm) and 28–32 (normalised photonic response).
fn bessell_v(text: &str) -> Result<Vec<(f64, f64)>, ReadObserverError> {
    let mut out = Vec::new();
    for line in text.lines() {
        let (Some(w), Some(r)) = (line.get(22..26), line.get(27..32)) else {
            continue;
        };
        if w.trim().is_empty() {
            continue;
        }
        let w: f64 = w
            .trim()
            .parse()
            .map_err(|_| ReadObserverError("a V wavelength is not a number"))?;
        let r: f64 = r
            .trim()
            .parse()
            .map_err(|_| ReadObserverError("a V response is not a number"))?;
        out.push((w / 10.0, r));
    }
    if out.len() < 10 || !out.windows(2).all(|p| p[0].0 < p[1].0) {
        return Err(ReadObserverError(
            "the V response is not a rising list of rows",
        ));
    }
    Ok(out)
}

/// Linear interpolation in a rising table, zero outside it.
#[must_use]
fn interpolate(table: &[(f64, f64)], x: f64) -> f64 {
    let i = table.partition_point(|&(t, _)| t <= x);
    if i == 0 || i == table.len() {
        return 0.0;
    }
    let (x0, y0) = table[i - 1];
    let (x1, y1) = table[i];
    y0 + (y1 - y0) * (x - x0) / (x1 - x0)
}

/// The matrix from XYZ to linear Rec. 709, from the primaries' and D65's chromaticities
/// (SMPTE RP 177-1993's construction).
#[must_use]
fn xyz_to_rec709() -> [[f64; 3]; 3] {
    let column = |[x, y]: [f64; 2]| [x / y, 1.0, (1.0 - x - y) / y];
    let p = PRIMARIES_XY.map(column);
    // Columns are the primaries' XYZ at unit Y.
    let m = [
        [p[0][0], p[1][0], p[2][0]],
        [p[0][1], p[1][1], p[2][1]],
        [p[0][2], p[1][2], p[2][2]],
    ];
    let inv = invert(m);
    let w = column(WHITE_XY);
    let s = [
        inv[0][0] * w[0] + inv[0][1] * w[1] + inv[0][2] * w[2],
        inv[1][0] * w[0] + inv[1][1] * w[1] + inv[1][2] * w[2],
        inv[2][0] * w[0] + inv[2][1] * w[1] + inv[2][2] * w[2],
    ];
    let rgb_to_xyz = m.map(|row| [row[0] * s[0], row[1] * s[1], row[2] * s[2]]);
    invert(rgb_to_xyz)
}

/// The inverse of a 3 × 3 matrix by cofactors.
#[must_use]
fn invert(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let c =
        |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    let cof = [
        [c(1, 2, 1, 2), -c(1, 2, 0, 2), c(1, 2, 0, 1)],
        [-c(0, 2, 1, 2), c(0, 2, 0, 2), -c(0, 2, 0, 1)],
        [c(0, 1, 1, 2), -c(0, 1, 0, 2), c(0, 1, 0, 1)],
    ];
    let det = m[0][0] * cof[0][0] + m[0][1] * cof[0][1] + m[0][2] * cof[0][2];
    // The inverse is the transposed cofactor matrix over the determinant.
    [
        [cof[0][0] / det, cof[1][0] / det, cof[2][0] / det],
        [cof[0][1] / det, cof[1][1] / det, cof[2][1] / det],
        [cof[0][2] / det, cof[1][2] / det, cof[2][2] / det],
    ]
}

/// A weighting table could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadObserverError(&'static str);

impl fmt::Display for ReadObserverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for ReadObserverError {}

#[cfg(test)]
pub(super) mod tests {
    use super::super::spectrum::Spectrum;
    use super::*;

    /// The observer from the committed datasets.
    pub(in super::super) fn observer() -> Observer {
        let data = concat!(env!("CARGO_MANIFEST_DIR"), "/data/");
        let read = |p: &str| std::fs::read_to_string(format!("{data}{p}")).unwrap();
        Observer::read(
            &read("cie_cmf/CIE_xyz_1931_2deg.csv"),
            &read("cie_cmf/CIE_sle_photopic.csv"),
            &read("cie_cmf/CIE_sle_scotopic.csv"),
            &read("cie_cmf/CIE_std_illum_D65.csv"),
            bessell()
                .as_deref()
                .map_or(VBandSource::CiePhotopic, VBandSource::BessellMurphy),
        )
        .unwrap()
    }

    /// Bessell and Murphy's Table 1, if it has been fetched into the cache.
    pub(in super::super) fn bessell() -> Option<String> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/cache/bessell_murphy_2012/table1.dat"
        );
        let text = std::fs::read_to_string(path).ok();
        if text.is_none() {
            eprintln!("{path} is not fetched: the V band is the photopic stand-in");
        }
        text
    }

    #[test]
    fn illuminant_a_and_the_sun_have_their_published_sp_ratios() {
        let observer = observer();
        // CIE Illuminant A is a 2,856 K Planckian radiator. Crumey (2014, MNRAS 442, 2600, §1.3,
        // eq. 5 with K_sc 1,700 and K_ph 683) gives ρ = 1.408 at 2,850 K, and his eq. 7 fit 1.413
        // at 2,856 K and 2.324 at 5,772 K.
        let a = observer.row(&Spectrum::blackbody(2_856.0).bin_means());
        assert!((a.sp_ratio - 1.41).abs() < 0.005, "{}", a.sp_ratio);
        let sun = observer.row(&Spectrum::blackbody(5_772.0).bin_means());
        assert!((sun.sp_ratio - 2.32).abs() < 0.01, "{}", sun.sp_ratio);
    }

    #[test]
    fn d65_is_rec709_white() {
        let observer = observer();
        let xyz = observer.integrals(observer.d65()).xyz;
        let rgb = observer.unit_rgb(xyz);
        for c in rgb {
            assert!((c - 1.0).abs() < 1e-3, "{rgb:?}");
        }
        // The matrix alone maps the defined white exactly.
        let [x, y] = WHITE_XY;
        let exact = observer.unit_rgb([x / y, 1.0, (1.0 - x - y) / y]);
        for c in exact {
            assert!((c - 1.0).abs() < 1e-12, "{exact:?}");
        }
    }

    #[test]
    fn an_out_of_gamut_colour_is_desaturated_towards_white_at_its_luminance() {
        let observer = observer();
        // A narrow band at 520 nm, far outside Rec. 709's gamut.
        let mut bins = vec![0.0; BIN_COUNT];
        bins[520 - FIRST_BIN_NM as usize] = 1.0;
        let xyz = observer.integrals(&bins).xyz;
        let raw = observer
            .to_rgb
            .map(|row| (row[0] * xyz[0] + row[1] * xyz[1] + row[2] * xyz[2]) / xyz[1]);
        assert!(raw[0] < 0.0 && raw[2] < 0.0, "{raw:?}");
        let rgb = observer.unit_rgb(xyz);
        let lowest = rgb[0].min(rgb[1]).min(rgb[2]);
        assert!(lowest.abs() < 1e-12, "{rgb:?}");
        let luminance = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
        assert!((luminance - 1.0).abs() < 1e-3, "{luminance}");
        // The mix is along the line from white: every channel moved by the same factor.
        let t: Vec<f64> = (0..3).map(|i| (rgb[i] - 1.0) / (raw[i] - 1.0)).collect();
        assert!(
            (t[0] - t[1]).abs() < 1e-12 && (t[1] - t[2]).abs() < 1e-12,
            "{t:?}"
        );
    }

    #[test]
    fn a_v_zero_star_of_flat_spectrum_is_near_allens_illuminance() {
        // A flat f_λ: ⟨f_λ⟩ is the flux itself, and E = 683 × 3.631e-11 × ∫V dλ (106.86 nm).
        if bessell().is_none() {
            return;
        }
        let observer = observer();
        let row = observer.row(&vec![1.0; BIN_COUNT]);
        assert!((row.lux_per_v0 - 1.042).abs() < 0.005, "{}", row.lux_per_v0);
    }
}
