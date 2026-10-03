//! The colour table's camera, reddening and bake columns (rendering plan R06, R06.T3.c; Design
//! notes 6 and 18; decision-camera-eta of 2026-10-02).
//!
//! - **The camera band term.** η = ∫S QE λ dλ ÷ ∫S `R_V` λ dλ, the default sensor's electrons per
//!   V-band photon, with `R_V` Bessell and Murphy's (2012) photonic V at peak 1, which gives Design
//!   note 18's Φ₀. The default sensor is unfiltered back-illuminated silicon, QE(λ) = 0.60 (1 −
//!   exp(−α(λ) × 16 µm)) from 400 to 1,100 nm and 0 outside, α = 4πk ÷ λ with k from Green (2008,
//!   Sol. Energ. Mat. Sol. Cells 92, 1305) as the CC0 refractiveindex.info database gives it. The
//!   table holds −2.5 log₁₀(η ÷ η☉), with η☉ (`CAMERA_ETA_SUN`) the same quantity at the table's
//!   5,772 K, log g 4.438 point.
//! - **Reddening.** Each display channel's `A_c ÷ A_V` at `R_V` = 3.1, plan 07's
//!   `ccm::extinction_ratio` at the channel's effective wavelength for the spectrum over that at the
//!   V band's effective wavelength. A channel's weight is the positive part of its Rec. 709
//!   colour-matching function (the matrix applied to x̄, ȳ, z̄), photon-free, as the eye's
//!   tristimulus integrals are; the V band's is photon-weighted, as V is.
//! - **The bake spectrum** (R08's ask). The spectrum's mean over each of the fifteen 25.33 nm bins
//!   from 380 to 760 nm, divided by 683 Σ ȳ̄ᵢ S̄ᵢ Δλ with ȳ̄ᵢ the bin's mean CIE 1924 V(λ), so that
//!   the fifteen bins hold unit photopic illuminance (1 lx) in W m⁻² nm⁻¹ per lux.

use std::fmt;

use hyperion_sim::galaxy::gas::ccm::extinction_ratio;
use hyperion_sim::math;
use hyperion_sim::units::Micrometres;

use super::photometry::{K_M, Observer};
use super::spectrum::{BIN_COUNT, Spectrum, bin_centre_nm};

/// The number of bake wavelengths: R08's fifteen.
pub const BAKE_WAVELENGTH_COUNT: usize = 15;

/// The bake bins' range, nm: 380–760 (R08's Design note 5).
pub const BAKE_RANGE_NM: (f64, f64) = (380.0, 760.0);

/// The edges of bake bin `k`, nm: 25.33 nm wide, from 380 nm.
///
/// # Panics
///
/// If `k` does not fit in `u32`, which no bin index reaches.
#[must_use]
pub fn bake_bin(k: usize) -> (f64, f64) {
    let (lo, hi) = BAKE_RANGE_NM;
    let width = (hi - lo) / 15.0;
    let k = f64::from(u32::try_from(k).expect("a bake bin index is below 15"));
    (lo + width * k, lo + width * (k + 1.0))
}

/// The bake bins' centres, nm: 392.67 + 25.33 k, R08's `BAKE_WAVELENGTHS_NM`.
#[must_use]
pub fn bake_wavelengths_nm() -> [f64; BAKE_WAVELENGTH_COUNT] {
    std::array::from_fn(|k| {
        let (a, b) = bake_bin(k);
        f64::midpoint(a, b)
    })
}

/// The default sensor's peak quantum efficiency (decision-camera-eta, 2026-10-02: lens
/// transmission about 0.9 times a back-illuminated device's about 0.7).
pub const SENSOR_PEAK_QE: f64 = 0.60;

/// The default sensor's silicon depth, µm: that of Gaia's astrometric CCDs, whose measured G − V
/// the curve then follows to 0.03 mag (decision-camera-eta).
pub const SENSOR_DEPTH_UM: f64 = 16.0;

/// The default sensor's band, nm.
pub const SENSOR_BAND_NM: (f64, f64) = (400.0, 1_100.0);

/// The default sensor: its quantum efficiency at each 1 nm bin's centre.
#[derive(Debug, Clone, PartialEq)]
pub struct Sensor {
    qe: Vec<f64>,
}

impl Sensor {
    /// The sensor from Green's (2008) k of silicon, as the refractiveindex.info database's
    /// `Green-2008.yml` tabulates it: lines of λ (µm), n and k under `data: |`. k is interpolated
    /// linearly in log k between its 10 nm samples, since absorption falls exponentially towards
    /// the band gap.
    ///
    /// # Errors
    ///
    /// [`ReadSensorError`] if no row is read or the rows do not span the sensor's band.
    pub fn from_green_2008(yml: &str) -> Result<Self, ReadSensorError> {
        let mut rows: Vec<(f64, f64)> = Vec::new();
        for line in yml.lines() {
            let fields: Vec<f64> = line
                .split_whitespace()
                .map(str::parse)
                .collect::<Result<_, _>>()
                .unwrap_or_default();
            if let [um, _n, k] = fields[..] {
                rows.push((um * 1_000.0, k));
            }
        }
        if !rows.windows(2).all(|w| w[0].0 < w[1].0) {
            return Err(ReadSensorError("Green 2008's wavelengths do not rise"));
        }
        let (lo, hi) = SENSOR_BAND_NM;
        match (rows.first(), rows.last()) {
            (Some(first), Some(last)) if first.0 <= lo && last.0 >= hi => {}
            _ => {
                return Err(ReadSensorError(
                    "Green 2008's rows do not span 400–1,100 nm",
                ));
            }
        }
        let qe = (0..BIN_COUNT)
            .map(|i| {
                let nm = bin_centre_nm(i);
                if nm < lo || nm > hi {
                    return 0.0;
                }
                let j = rows
                    .partition_point(|&(w, _)| w <= nm)
                    .clamp(1, rows.len() - 1);
                let (w0, k0) = rows[j - 1];
                let (w1, k1) = rows[j];
                let f = (nm - w0) / (w1 - w0);
                let k = math::exp(math::ln(k0) * (1.0 - f) + math::ln(k1) * f);
                // α = 4πk ÷ λ, per µm with λ in µm.
                let alpha_per_um = 4.0 * std::f64::consts::PI * k / (nm / 1_000.0);
                SENSOR_PEAK_QE * -math::exp_m1(-alpha_per_um * SENSOR_DEPTH_UM)
            })
            .collect();
        Ok(Self { qe })
    }

    /// The quantum efficiency at each 1 nm bin.
    #[must_use]
    pub fn qe(&self) -> &[f64] {
        &self.qe
    }
}

/// The three columns T3.c adds, for one spectrum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extras {
    /// η, the default sensor's electrons per V-band photon (not yet relative to the Sun's).
    pub camera_eta: f64,
    /// `A_c ÷ A_V` for red, green and blue.
    pub extinction: [f64; 3],
    /// The bake spectrum, W m⁻² nm⁻¹ per lux of photopic illuminance.
    pub bake: [f64; BAKE_WAVELENGTH_COUNT],
}

/// The columns of `spectrum`, whose 1 nm bin means are `bins`.
///
/// # Panics
///
/// If plan 07's `extinction_ratio` refuses an effective wavelength, which lies within 360–1,100 nm
/// and so within its range.
#[must_use]
pub fn extras(observer: &Observer, sensor: &Sensor, spectrum: &Spectrum, bins: &[f64]) -> Extras {
    let mut electrons = 0.0;
    let mut v_photons = 0.0;
    let mut v_lambda = 0.0;
    let mut channel = [0.0; 3];
    let mut channel_lambda = [0.0; 3];
    let rgb_cmf = observer.rgb_cmf();
    for (i, &s) in bins.iter().enumerate() {
        let nm = bin_centre_nm(i);
        electrons += s * sensor.qe[i] * nm;
        v_photons += s * observer.v_photons()[i];
        v_lambda += s * observer.v_photons()[i] * nm;
        for c in 0..3 {
            let w = rgb_cmf[i][c].max(0.0);
            channel[c] += s * w;
            channel_lambda[c] += s * w * nm;
        }
    }
    let ratio = |nm: f64| {
        extinction_ratio(Micrometres::new(nm / 1_000.0))
            .expect("an effective wavelength within 360–1,100 nm is in the law's range")
    };
    let a_v = ratio(v_lambda / v_photons);
    let extinction = std::array::from_fn(|c| ratio(channel_lambda[c] / channel[c]) / a_v);
    let means: [f64; BAKE_WAVELENGTH_COUNT] = std::array::from_fn(|k| {
        let (a, b) = bake_bin(k);
        spectrum.integral(a, b) / (b - a)
    });
    let y_means = observer.photopic_bin_means();
    let width = bake_bin(0).1 - bake_bin(0).0;
    let lux: f64 = means
        .iter()
        .zip(&y_means)
        .fold(0.0, |sum, (s, y)| sum + K_M * y * s * width);
    Extras {
        camera_eta: electrons / v_photons,
        extinction,
        bake: means.map(|s| s / lux),
    }
}

/// The default sensor could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadSensorError(&'static str);

impl fmt::Display for ReadSensorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for ReadSensorError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bake_bins_are_r08s() {
        let centres = bake_wavelengths_nm();
        assert!((centres[0] - 392.667).abs() < 1e-3);
        assert!((centres[14] - 747.333).abs() < 1e-3);
        for k in 1..15 {
            assert!((centres[k] - centres[k - 1] - 25.333).abs() < 1e-3);
        }
    }

    #[test]
    fn the_sensor_follows_silicon() {
        let yml = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/green2008_si/Green-2008.yml"
        ))
        .unwrap();
        let sensor = Sensor::from_green_2008(&yml).unwrap();
        let at = |nm: usize| sensor.qe()[nm - 360];
        // decision-camera-eta: peak 0.60, 0.45 at 800 nm, 0.13 at 950 nm.
        assert!((at(550) - 0.60).abs() < 0.005, "{}", at(550));
        assert!((at(800) - 0.45).abs() < 0.02, "{}", at(800));
        assert!((at(950) - 0.13).abs() < 0.02, "{}", at(950));
        assert!(at(399).abs() < 1e-12 && at(1_100) < 0.01);
    }
}
