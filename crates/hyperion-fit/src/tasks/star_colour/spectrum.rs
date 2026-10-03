//! Spectra as the colour table reads them: a flux density sampled on increasing wavelengths,
//! integrated over the table's 1 nm bins, and the files they come in.
//!
//! Every spectrum is taken as piecewise linear between its samples. A bin's value is the mean of
//! that line over the bin, computed exactly from the trapezoids it covers, so that a spectrum
//! sampled every 0.01 Å (PHOENIX) and one sampled every 2 nm (ATLAS9) are both reduced by the same
//! flux-conserving rule, with no aliasing of narrow lines.

use std::fmt;

use hyperion_sim::math;

/// The first bin's centre, nm: the CIE tables' first wavelength.
pub const FIRST_BIN_NM: u32 = 360;

/// The last bin's centre, nm: past silicon's red cut-off, for the camera's response.
pub const LAST_BIN_NM: u32 = 1_100;

/// The number of 1 nm bins, from [`FIRST_BIN_NM`] to [`LAST_BIN_NM`].
pub const BIN_COUNT: usize = (LAST_BIN_NM - FIRST_BIN_NM + 1) as usize;

/// The centre of bin `i`, nm.
///
/// # Panics
///
/// If `i` does not fit in `u32`, which no bin index reaches.
#[must_use]
pub fn bin_centre_nm(i: usize) -> f64 {
    f64::from(FIRST_BIN_NM) + f64::from(u32::try_from(i).expect("a bin index fits in u32"))
}

/// A spectrum: flux density in any one unit per unit wavelength, at wavelengths in nm.
#[derive(Debug, Clone, PartialEq)]
pub struct Spectrum {
    wavelength_nm: Vec<f64>,
    flux: Vec<f64>,
}

impl Spectrum {
    /// A spectrum from its samples.
    ///
    /// # Errors
    ///
    /// [`ReadSpectrumError`] if the two lists differ in length, the wavelengths do not increase
    /// strictly, a value is not finite, a flux is negative, or the samples do not cover every bin.
    pub fn new(wavelength_nm: Vec<f64>, flux: Vec<f64>) -> Result<Self, ReadSpectrumError> {
        if wavelength_nm.len() != flux.len() {
            return Err(ReadSpectrumError::LengthMismatch);
        }
        if !wavelength_nm.iter().chain(&flux).all(|v| v.is_finite()) {
            return Err(ReadSpectrumError::NotFinite);
        }
        if flux.iter().any(|&f| f < 0.0) {
            return Err(ReadSpectrumError::NegativeFlux);
        }
        if !wavelength_nm.windows(2).all(|w| w[0] < w[1]) {
            return Err(ReadSpectrumError::NotIncreasing);
        }
        let first = f64::from(FIRST_BIN_NM) - 0.5;
        let last = f64::from(LAST_BIN_NM) + 0.5;
        match (wavelength_nm.first(), wavelength_nm.last()) {
            (Some(&lo), Some(&hi)) if lo <= first && hi >= last => {}
            _ => {
                return Err(ReadSpectrumError::DoesNotCover);
            }
        }
        Ok(Self {
            wavelength_nm,
            flux,
        })
    }

    /// The Planck spectrum of `teff` (K), per nm, in arbitrary units, sampled every 0.5 nm over
    /// the bins: `B_λ` ∝ λ⁻⁵ ÷ (exp(hc ÷ λkT) − 1), with CODATA 2018's exact h, c and k.
    #[must_use]
    pub fn blackbody(teff: f64) -> Self {
        let n = 2 * (LAST_BIN_NM - FIRST_BIN_NM + 2);
        let mut wavelength_nm = Vec::new();
        let mut flux = Vec::new();
        for i in 0..=n {
            let lambda = f64::from(FIRST_BIN_NM) - 1.0 + 0.5 * f64::from(i);
            wavelength_nm.push(lambda);
            flux.push(planck(lambda, teff));
        }
        Self {
            wavelength_nm,
            flux,
        }
    }

    /// The mean flux density over each 1 nm bin, from [`FIRST_BIN_NM`] to [`LAST_BIN_NM`].
    #[must_use]
    pub fn bin_means(&self) -> Vec<f64> {
        (0..BIN_COUNT)
            .map(|i| {
                let centre = bin_centre_nm(i);
                self.integral(centre - 0.5, centre + 0.5)
            })
            .collect()
    }

    /// The integral of the piecewise-linear spectrum from `from` to `to` nm, within its samples.
    #[must_use]
    pub fn integral(&self, from: f64, to: f64) -> f64 {
        let nm = &self.wavelength_nm;
        let flux = &self.flux;
        // The first sample above `from`, so segment `start - 1` holds it.
        let start = nm.partition_point(|&x| x <= from).max(1);
        let mut sum = 0.0;
        let mut k = start - 1;
        while k + 1 < nm.len() && nm[k] < to {
            let lo = from.max(nm[k]);
            let hi = to.min(nm[k + 1]);
            if hi > lo {
                let slope = (flux[k + 1] - flux[k]) / (nm[k + 1] - nm[k]);
                let at_lo = flux[k] + slope * (lo - nm[k]);
                let at_hi = flux[k] + slope * (hi - nm[k]);
                sum += f64::midpoint(at_lo, at_hi) * (hi - lo);
            }
            k += 1;
        }
        sum
    }
}

/// The Planck function's shape at `lambda_nm` and `teff` K, per nm, in arbitrary units.
#[must_use]
pub fn planck(lambda_nm: f64, teff: f64) -> f64 {
    // hc ÷ k in nm K, from CODATA 2018's exact h, c and k_B.
    const HC_OVER_K_NM_K: f64 = 6.626_070_15e-34 * 299_792_458.0 / 1.380_649e-23 * 1.0e9;
    let x = HC_OVER_K_NM_K / (lambda_nm * teff);
    // Scaled by 10¹⁵ so that the values sit near one; the scale cancels in every ratio.
    1.0e15 / (math::powi(lambda_nm, 5) * math::exp_m1(x))
}

/// Reads an SVO theoretical-spectrum file in its ASCII form: `#` comment lines, then lines of
/// wavelength (Å) and flux density (erg cm⁻² s⁻¹ Å⁻¹).
///
/// # Errors
///
/// [`ReadSpectrumError`] for a line that is not two numbers, or as [`Spectrum::new`].
pub fn parse_svo_ascii(text: &str) -> Result<Spectrum, ReadSpectrumError> {
    let mut wavelength_nm = Vec::new();
    let mut flux = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let (Some(w), Some(f), None) = (fields.next(), fields.next(), fields.next()) else {
            return Err(ReadSpectrumError::Malformed(
                "a data line is not two numbers",
            ));
        };
        let w: f64 = w
            .parse()
            .map_err(|_| ReadSpectrumError::Malformed("a wavelength is not a number"))?;
        let f: f64 = f
            .parse()
            .map_err(|_| ReadSpectrumError::Malformed("a flux is not a number"))?;
        wavelength_nm.push(w / 10.0);
        flux.push(f);
    }
    Spectrum::new(wavelength_nm, flux)
}

/// Reads the primary array of a FITS file that holds one axis of IEEE floats (`BITPIX` −32 or
/// −64), as PHOENIX's spectra and wavelength file do: 2,880-byte header blocks of 80-character
/// cards up to `END`, then the big-endian values.
///
/// # Errors
///
/// [`ReadSpectrumError`] if the header is malformed, the array is not one-dimensional floats, or
/// the file is shorter than the array.
///
/// # Panics
///
/// Never: the values are read in chunks of exactly their width.
pub fn read_fits_vector(bytes: &[u8]) -> Result<Vec<f64>, ReadSpectrumError> {
    const BLOCK: usize = 2_880;
    const CARD: usize = 80;
    let mut bitpix = None;
    let mut naxis = None;
    let mut naxis1 = None;
    let mut end = None;
    for (i, card) in bytes.chunks(CARD).enumerate() {
        let card = std::str::from_utf8(card)
            .map_err(|_| ReadSpectrumError::Malformed("a FITS header card is not text"))?;
        let key = card.get(..8).unwrap_or(card).trim_end();
        if key == "END" {
            end = Some((i + 1) * CARD);
            break;
        }
        let value = || -> Result<i64, ReadSpectrumError> {
            card.get(10..)
                .and_then(|v| v.split('/').next())
                .and_then(|v| v.trim().parse().ok())
                .ok_or(ReadSpectrumError::Malformed(
                    "a FITS integer keyword is malformed",
                ))
        };
        match key {
            "BITPIX" => bitpix = Some(value()?),
            "NAXIS" => naxis = Some(value()?),
            "NAXIS1" => naxis1 = Some(value()?),
            _ => {}
        }
    }
    let end = end.ok_or(ReadSpectrumError::Malformed(
        "the FITS header has no END card",
    ))?;
    if naxis != Some(1) {
        return Err(ReadSpectrumError::Malformed(
            "the FITS array is not one-dimensional",
        ));
    }
    let count = naxis1
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(ReadSpectrumError::Malformed("the FITS array has no length"))?;
    let data = end.div_ceil(BLOCK) * BLOCK;
    let width = match bitpix {
        Some(-32) => 4,
        Some(-64) => 8,
        _ => {
            return Err(ReadSpectrumError::Malformed(
                "the FITS array is not of IEEE floats",
            ));
        }
    };
    let body = bytes
        .get(data..data + count * width)
        .ok_or(ReadSpectrumError::Malformed(
            "the FITS file is shorter than its array",
        ))?;
    Ok(body
        .chunks_exact(width)
        .map(|chunk| match chunk.len() {
            4 => f64::from(f32::from_be_bytes(
                chunk
                    .try_into()
                    .expect("a chunk of four bytes is four bytes"),
            )),
            8 => f64::from_be_bytes(
                chunk
                    .try_into()
                    .expect("a chunk of eight bytes is eight bytes"),
            ),
            _ => unreachable!("chunks_exact yields chunks of the width asked for"),
        })
        .collect())
}

/// A spectrum could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReadSpectrumError {
    /// The wavelengths and fluxes differ in number.
    LengthMismatch,
    /// A wavelength or flux is not finite.
    NotFinite,
    /// A flux is negative.
    NegativeFlux,
    /// The wavelengths do not increase strictly.
    NotIncreasing,
    /// The samples do not cover the bins, 359.5–1,100.5 nm.
    DoesNotCover,
    /// The file is not of its format; the text says how.
    Malformed(&'static str),
}

impl fmt::Display for ReadSpectrumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch => f.write_str("the wavelengths and fluxes differ in number"),
            Self::NotFinite => f.write_str("a value is not finite"),
            Self::NegativeFlux => f.write_str("a flux is negative"),
            Self::NotIncreasing => f.write_str("the wavelengths do not increase"),
            Self::DoesNotCover => f.write_str("the samples do not cover 359.5–1,100.5 nm"),
            Self::Malformed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for ReadSpectrumError {}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    fn flat_with_line() -> Spectrum {
        // A flat spectrum of 1 with a triangular dip to 0 at 500 nm, 0.2 nm wide at its base.
        let w = vec![300.0, 499.9, 500.0, 500.1, 1_200.0];
        let f = vec![1.0, 1.0, 0.0, 1.0, 1.0];
        Spectrum::new(w, f).unwrap()
    }

    #[test]
    fn a_bin_mean_conserves_the_flux_of_a_narrow_line() {
        let means = flat_with_line().bin_means();
        assert_eq!(means.len(), BIN_COUNT);
        // The dip removes 0.1 nm of flux from the 500 nm bin.
        let at_500 = means[500 - FIRST_BIN_NM as usize];
        assert!((at_500 - 0.9).abs() < 1e-12, "{at_500}");
        assert!((means[0] - 1.0).abs() < 1e-12);
        assert!((means[BIN_COUNT - 1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_short_or_disordered_spectrum_is_refused() {
        assert_eq!(
            Spectrum::new(vec![400.0, 1_200.0], vec![1.0, 1.0]),
            Err(ReadSpectrumError::DoesNotCover)
        );
        assert_eq!(
            Spectrum::new(vec![300.0, 1_200.0, 1_100.0], vec![1.0, 1.0, 1.0]),
            Err(ReadSpectrumError::NotIncreasing)
        );
        assert_eq!(
            Spectrum::new(vec![300.0, 1_200.0], vec![1.0, -1.0]),
            Err(ReadSpectrumError::NegativeFlux)
        );
        assert_eq!(
            Spectrum::new(vec![300.0, 1_200.0], vec![1.0]),
            Err(ReadSpectrumError::LengthMismatch)
        );
        assert_eq!(
            Spectrum::new(vec![300.0, 1_200.0], vec![1.0, f64::NAN]),
            Err(ReadSpectrumError::NotFinite)
        );
    }

    #[test]
    fn the_svo_form_is_read_in_nanometres() {
        let text = "# comment\n 3000.0  1.0e5 \n 6000 2.0e5\n12000 3.0e5\n";
        let s = parse_svo_ascii(text).unwrap();
        assert_eq!(s.wavelength_nm, [300.0, 600.0, 1_200.0]);
        assert_eq!(
            parse_svo_ascii("3000 1 2\n"),
            Err(ReadSpectrumError::Malformed(
                "a data line is not two numbers"
            ))
        );
    }

    #[test]
    fn the_blackbody_peaks_where_wien_says() {
        // Wien's displacement constant b = 2.897771955 × 10⁶ nm K (CODATA 2018).
        let s = Spectrum::blackbody(5_000.0);
        let means = s.bin_means();
        let peak = (0..BIN_COUNT)
            .max_by(|&a, &b| means[a].total_cmp(&means[b]))
            .unwrap();
        assert!((bin_centre_nm(peak) - 579.55).abs() < 1.0, "{peak}");
    }

    #[test]
    fn a_fits_vector_is_read() {
        let mut header = String::new();
        for card in [
            "SIMPLE  =                    T",
            "BITPIX  =                  -32",
            "NAXIS   =                    1",
            "NAXIS1  =                    3",
            "END",
        ] {
            write!(header, "{card:<80}").unwrap();
        }
        let mut bytes = header.into_bytes();
        bytes.resize(2_880, b' ');
        // 1.5, −2.0 and 3.25 as big-endian IEEE singles.
        bytes.extend_from_slice(&[0x3f, 0xc0, 0, 0, 0xc0, 0, 0, 0, 0x40, 0x50, 0, 0]);
        assert_eq!(read_fits_vector(&bytes).unwrap(), [1.5, -2.0, 3.25]);
        bytes.truncate(2_884);
        assert_eq!(
            read_fits_vector(&bytes),
            Err(ReadSpectrumError::Malformed(
                "the FITS file is shorter than its array"
            ))
        );
    }
}
