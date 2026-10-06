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
//! - **The eye's and the camera's reddening** (R06.T9.e; decided 2026-10-06,
//!   `decision-r06-t9b-band.md`, item 3), the `star_colour_reddening` table's four columns.
//!   `A_P ÷ A_V` and `A_S ÷ A_V` by the display channels' method: plan 07's law at the effective
//!   wavelength of S·V(λ) and of S·V′(λ), photon-free as V(λ) and V′(λ) weigh light, over the law
//!   at the V band's. `A_cam ÷ A_V` at `A_V` → 0 and at `A_V` = 2 from the integrals themselves, since
//!   the camera's band is too broad for one effective wavelength: the camera's extinction
//!   −2.5 log₁₀(∫S QE λ 10<sup>−0.4 ℓ(λ) d</sup> ÷ ∫S QE λ) over the V band's, the same integral
//!   over S `R_V` λ, for dust of d at 0.55 µm ([`reddening`]).
//!
//! The column records ([`Extras`], [`Reddening`]) are plain data with public fields, as
//! `photometry`'s `ColourRow` is: the task's renderers and tests read them field by field.

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
        // The rows are the indented lines after `data: |`; the YAML's header precedes them.
        let (before, block) = yml
            .split_once("data: |")
            .ok_or(ReadSensorError("Green 2008's file has no `data: |` block"))?;
        // The block holds the lines indented deeper than its key, as YAML's literal block does.
        let key_indent = before.len() - before.rfind('\n').map_or(0, |i| i + 1);
        let indent = |l: &str| l.len() - l.trim_start().len();
        let rows_text = block
            .lines()
            .skip(1)
            .take_while(|l| l.trim().is_empty() || indent(l) > key_indent);
        for line in rows_text.filter(|l| !l.trim().is_empty()) {
            let fields: Vec<f64> = line
                .split_whitespace()
                .map(str::parse)
                .collect::<Result<_, _>>()
                .map_err(|_| ReadSensorError("a row of Green 2008's data is not numbers"))?;
            let [um, _n, k] = fields[..] else {
                return Err(ReadSensorError(
                    "a row of Green 2008's data is not λ, n and k",
                ));
            };
            rows.push((um * 1_000.0, k));
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
/// If the spectrum has no flux in the V band or in one of the channels' positive lobes, whose
/// effective wavelength is then not a number, which plan 07's `extinction_ratio` refuses: no
/// stellar spectrum is dark over a whole optical band. An effective wavelength otherwise lies
/// within 360–1,100 nm and so within the law's range.
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

/// The extinction in V at which the camera's second reddening column is taken, mag: the ruling's
/// A<sub>V</sub> 2 (`decision-r06-t9b-band.md`, item 3), between which and A<sub>V</sub> → 0 the sim
/// takes `A_cam ÷ A_V` as linear in A<sub>V</sub>.
pub const CAMERA_REDDENING_A_V: f64 = 2.0;

/// The four columns R06.T9.e adds, for one spectrum: each band's extinction over the V band's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reddening {
    /// `A_P ÷ A_V`, the photopic light's: plan 07's law at the effective wavelength of S·V(λ) over
    /// the law at the V band's.
    pub photopic: f64,
    /// `A_S ÷ A_V`, the scotopic light's, at the effective wavelength of S·V′(λ).
    pub scotopic: f64,
    /// `A_cam ÷ A_V` for the default sensor, at A<sub>V</sub> → 0 and at
    /// [`CAMERA_REDDENING_A_V`], each band's extinction its own integral over the same dust.
    pub camera: [f64; 2],
}

/// The reddening columns of a spectrum whose 1 nm bin means are `bins`.
///
/// The photopic and scotopic ratios are the display channels' method ([`extras`]): the law at the
/// effective wavelength of S·V(λ) or S·V′(λ), photon-free, over the law at the V band's
/// photon-weighted one. The camera's band is too broad for one effective wavelength (the ruling's
/// item 3), so its ratio is that of two integrals over the same dust, of which the V band's sets
/// `A_V`: with ℓ(λ) plan 07's law and d the dust's extinction at 0.55 µm, a band of weights w
/// (S QE λ for the camera, S `R_V` λ for V) is dimmed by A(d) = −2.5 log₁₀(Σ w 10<sup>−0.4 ℓ d</sup> ÷
/// Σ w). At `A_V` → 0 the ratio is Σ w ℓ ÷ Σ w over the camera's weights by the same over V's; at
/// [`CAMERA_REDDENING_A_V`] it is A<sub>cam</sub>(d) ÷ `A_V`(d) at the d whose `A_V`(d) is that.
/// V's band-mean law exceeds the law at its effective wavelength by 0.76%, and V(λ)'s and V′(λ)'s
/// nearly as much, so the photopic and scotopic ratios, both taken at effective wavelengths, agree
/// with the broadband pairs to 0.0021 (science check of R06.T9.e).
///
/// # Panics
///
/// If the spectrum has no flux in the V band, under V(λ) or under V′(λ), whose effective
/// wavelength is then not a number, which plan 07's `extinction_ratio` refuses; or none in the
/// sensor's band. No stellar spectrum is dark over a whole optical band. Every other wavelength
/// the law is asked at is a bin's, within 360–1,100 nm and so within its range.
#[must_use]
pub fn reddening(observer: &Observer, sensor: &Sensor, bins: &[f64]) -> Reddening {
    let law = |nm: f64| {
        extinction_ratio(Micrometres::new(nm / 1_000.0)).expect(
            "a stellar spectrum has flux in every optical band, so each wavelength asked is a bin's \
             or an effective wavelength within 360–1,100 nm",
        )
    };
    let (mut v_photons, mut v_lambda) = (0.0, 0.0);
    let (mut photopic, mut photopic_lambda) = (0.0, 0.0);
    let (mut scotopic, mut scotopic_lambda) = (0.0, 0.0);
    // Each bin's law, V band weight and camera weight, where either weight is not zero.
    let mut broadband: Vec<(f64, f64, f64)> = Vec::with_capacity(bins.len());
    for (i, &s) in bins.iter().enumerate() {
        let nm = bin_centre_nm(i);
        let v = s * observer.v_photons()[i];
        v_photons += v;
        v_lambda += v * nm;
        photopic += s * observer.photopic()[i];
        photopic_lambda += s * observer.photopic()[i] * nm;
        scotopic += s * observer.scotopic()[i];
        scotopic_lambda += s * observer.scotopic()[i] * nm;
        let camera = s * sensor.qe[i] * nm;
        if v > 0.0 || camera > 0.0 {
            broadband.push((law(nm), v, camera));
        }
    }
    let v_law = law(v_lambda / v_photons);
    let dimmed = |d: f64| {
        let (mut v, mut v_left, mut camera, mut camera_left) = (0.0, 0.0, 0.0, 0.0);
        for &(l, w_v, w_camera) in &broadband {
            let t = math::exp10(-0.4 * l * d);
            v += w_v;
            v_left += w_v * t;
            camera += w_camera;
            camera_left += w_camera * t;
        }
        (
            -2.5 * math::log10(v_left / v),
            -2.5 * math::log10(camera_left / camera),
        )
    };
    let mean_law = |weight: fn(&(f64, f64, f64)) -> f64| {
        let (sum, weights) = broadband.iter().fold((0.0, 0.0), |(sum, weights), b| {
            (sum + b.0 * weight(b), weights + weight(b))
        });
        sum / weights
    };
    assert!(
        broadband.iter().any(|b| b.2 > 0.0),
        "a spectrum with no flux in the sensor's band"
    );
    let (v_mean, camera_mean) = (mean_law(|b| b.1), mean_law(|b| b.2));
    // The dust whose V band extinction is CAMERA_REDDENING_A_V: A_V(d) is d times a mean law that
    // drifts by a few parts in 10³ over it, so each step gains some two and a half digits.
    let mut d = CAMERA_REDDENING_A_V / v_mean;
    for _ in 0..8 {
        d *= CAMERA_REDDENING_A_V / dimmed(d).0;
    }
    let (a_v, a_camera) = dimmed(d);
    Reddening {
        photopic: law(photopic_lambda / photopic) / v_law,
        scotopic: law(scotopic_lambda / scotopic) / v_law,
        camera: [camera_mean / v_mean, a_camera / a_v],
    }
}

/// The broadband `A_P ÷ A_V` and `A_S ÷ A_V` of a spectrum whose 1 nm bin means are `bins`, for
/// the dust that dims its V band by `a_v`: each band's own integral, −2.5 log₁₀(Σ S w
/// 10<sup>−0.4 ℓ d</sup> ÷ Σ S w) with w its weighting function and ℓ plan 07's law, over the V
/// band's. The check of the effective-wavelength columns ([`reddening`]), which the ruling holds
/// to 0.003.
///
/// # Panics
///
/// If `a_v` is not positive and finite, or the spectrum has no flux in one of the bands.
#[must_use]
pub fn broadband_eye_reddening(observer: &Observer, bins: &[f64], a_v: f64) -> [f64; 2] {
    assert!(a_v > 0.0 && a_v.is_finite(), "an extinction of {a_v} mag");
    let law: Vec<f64> = (0..bins.len())
        .map(|i| {
            extinction_ratio(Micrometres::new(bin_centre_nm(i) / 1_000.0))
                .expect("a bin's wavelength is in the law's range")
        })
        .collect();
    let dimmed = |weights: &[f64], d: f64| {
        let (mut all, mut left) = (0.0, 0.0);
        for ((&s, &w), &l) in bins.iter().zip(weights).zip(&law) {
            all += s * w;
            left += s * w * math::exp10(-0.4 * l * d);
        }
        assert!(all > 0.0, "a spectrum with no flux in a band");
        -2.5 * math::log10(left / all)
    };
    // The dust of the V band's a_v, by bisection: the V band's extinction rises with the dust.
    let (mut lo, mut hi) = (0.0, 3.0 * a_v);
    for _ in 0..80 {
        let mid = f64::midpoint(lo, hi);
        if dimmed(observer.v_photons(), mid) < a_v {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let d = f64::midpoint(lo, hi);
    let v = dimmed(observer.v_photons(), d);
    [
        dimmed(observer.photopic(), d) / v,
        dimmed(observer.scotopic(), d) / v,
    ]
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
pub(super) mod tests {
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

    /// The default sensor from the committed dataset.
    pub(in super::super) fn sensor() -> Sensor {
        let yml = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/green2008_si/Green-2008.yml"
        ))
        .unwrap();
        Sensor::from_green_2008(&yml).unwrap()
    }

    #[test]
    fn a_malformed_silicon_table_is_refused() {
        assert_eq!(
            Sensor::from_green_2008("no data here"),
            Err(ReadSensorError("Green 2008's file has no `data: |` block"))
        );
        assert_eq!(
            Sensor::from_green_2008("DATA:\n  data: |\n    0.4 5.6 x\n  other: 1\n"),
            Err(ReadSensorError("a row of Green 2008's data is not numbers"))
        );
    }

    /// The Sun's spectrum stood in for by its blackbody: the ruling's ranges for the colour table's
    /// solar row (`decision-r06-t9b-band.md`, item 3: 0.987, 1.122 and 0.81–0.84 from the model).
    #[test]
    fn the_reddening_columns_of_sunlight_are_the_rulings() {
        let observer = super::super::photometry::tests::observer();
        let sensor = sensor();
        let sun = reddening(
            &observer,
            &sensor,
            &Spectrum::blackbody(5_772.0).bin_means(),
        );
        let [photopic, difference, camera] = super::super::SOLAR_REDDENING_RANGES;
        let within = |(lo, hi): (f64, f64), value: f64| (lo..=hi).contains(&value);
        assert!(within(photopic, sun.photopic), "{sun:?}");
        assert!(within(difference, sun.scotopic - sun.photopic), "{sun:?}");
        assert!(within(camera, sun.camera[0]), "{sun:?}");
        // The dust takes the blue first, so what is left is redder and dimmed less per magnitude.
        assert!(sun.camera[1] < sun.camera[0], "{sun:?}");
        // A cooler star's light sits further to the red, where silicon sees less of the dust.
        let cool = reddening(
            &observer,
            &sensor,
            &Spectrum::blackbody(3_500.0).bin_means(),
        );
        assert!(cool.camera[0] < sun.camera[0], "{cool:?} against {sun:?}");
    }

    /// The photopic and scotopic ratios at their effective wavelengths are the broadband
    /// integrals' over the V band's to 0.003 at `A_V` 1 (the ruling's accuracy for the method),
    /// from 3,000 to 30,000 K; and the camera's columns are its own integrals' over the V band's
    /// at `A_V` 2 and as `A_V` → 0.
    #[test]
    fn the_ratios_are_the_broadband_integrals() {
        let observer = super::super::photometry::tests::observer();
        let sensor = sensor();
        let law = |nm: f64| extinction_ratio(Micrometres::new(nm / 1_000.0)).unwrap();
        for teff in [3_000.0, 5_772.0, 10_000.0, 30_000.0] {
            let bins = Spectrum::blackbody(teff).bin_means();
            let columns = reddening(&observer, &sensor, &bins);
            let [photopic, scotopic] = broadband_eye_reddening(&observer, &bins, 1.0);
            assert!(
                (photopic - columns.photopic).abs() < 0.003,
                "{teff} K: {photopic} against {}",
                columns.photopic
            );
            assert!(
                (scotopic - columns.scotopic).abs() < 0.003,
                "{teff} K: {scotopic} against {}",
                columns.scotopic
            );
            // The camera's and the V band's extinction by dust of d at 0.55 µm, by bisection on d
            // for the V band's.
            let dimmed = |w: &dyn Fn(usize) -> f64, d: f64| {
                let (mut all, mut left) = (0.0, 0.0);
                for (i, &s) in bins.iter().enumerate() {
                    all += s * w(i);
                    left += s * w(i) * math::exp10(-0.4 * law(bin_centre_nm(i)) * d);
                }
                -2.5 * math::log10(left / all)
            };
            let v = |i: usize| observer.v_photons()[i];
            let camera = |i: usize| sensor.qe()[i] * bin_centre_nm(i);
            let at = |a_v: f64| {
                let (mut lo, mut hi) = (0.0, 3.0 * a_v);
                for _ in 0..80 {
                    let mid = f64::midpoint(lo, hi);
                    if dimmed(&v, mid) < a_v {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                f64::midpoint(lo, hi)
            };
            let camera_2 = dimmed(&camera, at(CAMERA_REDDENING_A_V)) / CAMERA_REDDENING_A_V;
            assert!(
                (camera_2 - columns.camera[1]).abs() < 1e-9,
                "{teff} K: {camera_2} against {}",
                columns.camera[1]
            );
            let small = at(1e-4);
            let camera_0 = dimmed(&camera, small) / dimmed(&v, small);
            assert!(
                (camera_0 - columns.camera[0]).abs() < 1e-4,
                "{teff} K: {camera_0} against {}",
                columns.camera[0]
            );
        }
    }

    #[test]
    fn the_sensor_follows_silicon() {
        let sensor = sensor();
        let at = |nm: usize| sensor.qe()[nm - 360];
        // decision-camera-eta: peak 0.60, 0.45 at 800 nm, 0.13 at 950 nm.
        assert!((at(550) - 0.60).abs() < 0.005, "{}", at(550));
        assert!((at(800) - 0.45).abs() < 0.02, "{}", at(800));
        assert!((at(950) - 0.13).abs() < 0.02, "{}", at(950));
        assert!(at(399).abs() < 1e-12 && at(1_100) < 0.01);
    }
}
