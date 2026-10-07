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
//! - **Reddening through dust** (R06.T9.e; decided 2026-10-06, `decision-r06-t9b-band.md`, item 3
//!   and its addendum), the `star_colour_reddening` tables' columns ([`reddening`]). Each Rec. 709
//!   colour-matching function is split into its positive and negative parts, six positive bands,
//!   beside the V band, the scotopic V′(λ) and the camera's QE λ. For each band, over plan 07's
//!   sightline A<sub>V</sub> (the law's normalisation at 0.549 µm): its first moment of the law, the
//!   limit of its secant −2.5 log₁₀ T(A) ÷ A as A → 0, and its secant at each of
//!   [`REDDENING_A_V_NODES`], each band its own integral over the same dust; and each part's value
//!   per unit luminance.
//!
//! The column records ([`Extras`], [`Reddening`]) are plain data with public fields, as
//! `photometry`'s `ColourRow` is: the task's renderers and tests read them field by field.

use std::fmt;

use hyperion_sim::galaxy::gas::ccm::extinction_ratio;
use hyperion_sim::math;
pub use hyperion_sim::sky::colour::{REDDENING_A_V_NODES, REDDENING_NODE_COUNT};
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

/// The reddening tables' bands, in their column order (R06.T9.e; decided 2026-10-06,
/// `decision-r06-t9b-band.md`, addendum item 1): the six parts of the Rec. 709 colour-matching
/// functions, r̄⁺, r̄⁻, ḡ⁺, ḡ⁻, b̄⁺ and b̄⁻ (part 2c is channel c's positive part, 2c + 1 its
/// negative), then the V band, the scotopic V′(λ) and the default camera, as the sim's
/// `sky::colour` reads them.
pub const BAND_COUNT: usize = 9;

/// The number of parts of the colour-matching functions among the bands.
pub const PART_COUNT: usize = 6;

/// The V band's index among the bands: Bessell and Murphy's photonic V, `R_V` λ.
pub const V_BAND: usize = 6;

/// The scotopic band's index: the CIE 1951 V′(λ).
pub const SCOTOPIC_BAND: usize = 7;

/// The camera's index: the default sensor's QE λ.
pub const CAMERA_BAND: usize = 8;

/// The reddening columns of one spectrum (R06.T9.e as the addendum of 2026-10-06 amends it), each
/// over plan 07's sightline A<sub>V</sub>, the law's normalisation at 0.549 µm times the dust's
/// column: for a band of weights w, the transmission T(A) = ∫S w 10<sup>−0.4 ℓ A</sup> ÷ ∫S w with ℓ
/// plan 07's law, and the secant k(A) = −2.5 log₁₀ T(A) ÷ A.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reddening {
    /// Each part's value per unit of the unreddened luminance, ∫S c̄± ÷ ∫S ȳ: the raw colour of
    /// unit luminance, before T3's lift into gamut, is the positive part less the negative.
    pub parts: [f64; PART_COUNT],
    /// Each band's first moment of the law, ∫S w ℓ ÷ ∫S w: its secant's limit as A → 0.
    pub moments: [f64; BAND_COUNT],
    /// Each band's secant at each node of [`REDDENING_A_V_NODES`], node-major.
    pub secants: [[f64; BAND_COUNT]; REDDENING_NODE_COUNT],
    /// The spectrum's CIE 1931 integrals, as `Observer::row` takes them: the lift's check, not
    /// written.
    pub xyz: [f64; 3],
    /// The CIE 1931 integrals of the spectrum behind the last node's dust, where every row's light
    /// is out of gamut: the lift's check on its other branch, not written.
    pub xyz_at_last_node: [f64; 3],
    /// The largest relative difference over the nodes between the photopic transmission of the
    /// exact parts, Σ Y<sub>c</sub> (c⁺(A) − c⁻(A)), and the direct V(λ) integral's: the photopic
    /// identity's check (the addendum's test 5), not written.
    pub photopic_identity: f64,
}

/// Plan 07's law at each 1 nm bin's centre, `A_λ ÷ A_V`, normalised at 0.549 µm.
///
/// # Panics
///
/// Never: every bin lies within 360–1,100 nm, in the law's range.
#[must_use]
pub fn law_by_bin() -> Vec<f64> {
    (0..BIN_COUNT)
        .map(|i| {
            extinction_ratio(Micrometres::new(bin_centre_nm(i) / 1_000.0))
                .expect("a bin's wavelength within 360–1,100 nm is in the law's range")
        })
        .collect()
}

/// A spectrum's 1 nm bin means `bins` behind dust of plan 07's sightline A<sub>V</sub> `a_v`:
/// each bin times 10<sup>−0.4 ℓ A<sub>V</sub></sup>, `law` the law by bin ([`law_by_bin`]).
#[must_use]
pub fn dimmed_bins(bins: &[f64], law: &[f64], a_v: f64) -> Vec<f64> {
    bins.iter()
        .zip(law)
        .map(|(&s, &l)| s * math::exp10(-0.4 * l * a_v))
        .collect()
}

/// The nine bands' sums Σ S w over the bins ([`BAND_COUNT`]'s order) of a spectrum whose 1 nm bin
/// means are `bins`, and its photopic sum Σ S V(λ). `rgb_cmf` is the observer's Rec. 709
/// colour-matching functions by bin.
#[must_use]
pub fn band_sums(
    observer: &Observer,
    sensor: &Sensor,
    rgb_cmf: &[[f64; 3]],
    bins: &[f64],
) -> ([f64; BAND_COUNT], f64) {
    let mut sums = [0.0; BAND_COUNT];
    let mut photopic = 0.0;
    for (i, &s) in bins.iter().enumerate() {
        let nm = bin_centre_nm(i);
        for (c, &w) in rgb_cmf[i].iter().enumerate() {
            sums[2 * c] += s * w.max(0.0);
            sums[2 * c + 1] += s * (-w).max(0.0);
        }
        sums[V_BAND] += s * observer.v_photons()[i];
        sums[SCOTOPIC_BAND] += s * observer.scotopic()[i];
        sums[CAMERA_BAND] += s * sensor.qe[i] * nm;
        photopic += s * observer.photopic()[i];
    }
    (sums, photopic)
}

/// The reddening columns of a spectrum whose 1 nm bin means are `bins` ([`Reddening`]).
///
/// Every band is an integral of its own over the same dust; there is no effective wavelength. The
/// photopic light has no column: ȳ is exactly the Rec. 709 luminance of r̄, ḡ and b̄, and the
/// vendored CIE 1924 V(λ) is the 1931 ȳ in every 1 nm bin, so the parts' luminance is the
/// photopic transmission, which [`Reddening::photopic_identity`] checks.
///
/// # Panics
///
/// If the spectrum has no flux in one of the bands, whose secant is then not a number: no stellar
/// spectrum is dark over a whole optical band.
#[must_use]
pub fn reddening(observer: &Observer, sensor: &Sensor, bins: &[f64]) -> Reddening {
    let law = law_by_bin();
    let rgb_cmf = observer.rgb_cmf();
    let xyz = observer.integrals(bins).xyz;
    let (all, photopic) = band_sums(observer, sensor, &rgb_cmf, bins);
    assert!(
        all.iter().all(|&w| w > 0.0) && photopic > 0.0 && xyz[1] > 0.0,
        "a spectrum with no flux in a band: {all:?}"
    );
    let weighted: Vec<f64> = bins.iter().zip(&law).map(|(&s, &l)| s * l).collect();
    let (moments, _) = band_sums(observer, sensor, &rgb_cmf, &weighted);
    let luminance = observer.luminance();
    let mut photopic_identity: f64 = 0.0;
    let mut xyz_at_last_node = [0.0; 3];
    let secants = std::array::from_fn(|n| {
        let a = REDDENING_A_V_NODES[n];
        let dimmed = dimmed_bins(bins, &law, a);
        if n + 1 == REDDENING_NODE_COUNT {
            xyz_at_last_node = observer.integrals(&dimmed).xyz;
        }
        let (left, photopic_left) = band_sums(observer, sensor, &rgb_cmf, &dimmed);
        let parts_photopic = (0..3).fold(0.0, |sum, c| {
            sum + luminance[c] * (left[2 * c] - left[2 * c + 1])
        }) / xyz[1];
        photopic_identity =
            photopic_identity.max((parts_photopic / (photopic_left / photopic) - 1.0).abs());
        std::array::from_fn(|b| -2.5 * math::log10(left[b] / all[b]) / a)
    });
    Reddening {
        parts: std::array::from_fn(|p| all[p] / xyz[1]),
        moments: std::array::from_fn(|b| moments[b] / all[b]),
        secants,
        xyz,
        xyz_at_last_node,
        photopic_identity,
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
pub(super) mod tests {
    use super::*;
    use hyperion_sim::sky::colour::lift_into_gamut;

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

    /// The Sun's spectrum stood in for by its blackbody: the solar row's ranges of the ruling, as
    /// the addendum corrects them (`decision-r06-t9b-band.md`, items 3 and 4: `A_P ÷ A_V`
    /// 0.97–1.01, `A_S ÷ A_V − A_P ÷ A_V` 0.11–0.15, `A_cam ÷ A_V` 0.84–0.92 and V's own 0.99–1.02,
    /// all as `A_V` → 0).
    ///
    /// V's range is the real V band's, Bessell and Murphy's, which is fetched: without it the
    /// observer's V is the photopic stand-in, and the test holds V's column to that band's own
    /// integral instead. The committed table's solar row, fitted on the real V band, is held to
    /// all four ranges by the sim's `sky::colour` test `the_solar_rows_ratios_are_the_rulings`,
    /// fetched or not.
    #[test]
    fn the_reddening_columns_of_sunlight_are_the_rulings() {
        let observer = super::super::photometry::tests::observer();
        let sensor = sensor();
        let bins = Spectrum::blackbody(5_772.0).bin_means();
        let sun = reddening(&observer, &sensor, &bins);
        let ratios = super::super::ratios_at_zero(&sun.parts, &sun.moments, observer.luminance());
        let [photopic, difference, camera, v] = super::super::SOLAR_REDDENING_RANGES;
        let within = |(lo, hi): (f64, f64), value: f64| (lo..=hi).contains(&value);
        assert!(within(photopic, ratios[0]), "{ratios:?}");
        assert!(within(difference, ratios[1] - ratios[0]), "{ratios:?}");
        assert!(within(camera, ratios[2]), "{ratios:?}");
        if super::super::photometry::tests::bessell().is_some() {
            assert!(within(v, ratios[3]), "{ratios:?}");
        } else {
            // The stand-in is V(λ) photon-weighted, V(λ) λ: V's moment is that band's mean of the
            // law. The weight λ moves it to the red, where the law is lower, so it lies below the
            // photopic's, the same V(λ) unweighted.
            let law = law_by_bin();
            let (mut light, mut moment) = (0.0, 0.0);
            for (i, (&s, &l)) in bins.iter().zip(&law).enumerate() {
                let w = observer.photopic()[i] * bin_centre_nm(i);
                light += s * w;
                moment += s * l * w;
            }
            let stand_in = moment / light;
            assert!(
                (ratios[3] / stand_in - 1.0).abs() < 1e-12,
                "{ratios:?} against the stand-in's {stand_in}"
            );
            assert!(ratios[3] < ratios[0], "{ratios:?}");
        }
        // The dust takes the blue first, so what is left is redder, and silicon and V see less of
        // it per magnitude.
        for b in [V_BAND, CAMERA_BAND] {
            assert!(sun.secants[0][b] < sun.moments[b], "{sun:?}");
        }
        // A cooler star's light sits further to the red, where silicon sees less of the dust.
        let cool = reddening(
            &observer,
            &sensor,
            &Spectrum::blackbody(3_500.0).bin_means(),
        );
        assert!(
            cool.moments[CAMERA_BAND] < sun.moments[CAMERA_BAND],
            "{cool:?} against {sun:?}"
        );
    }

    /// The addendum's test 5 on blackbodies of 2,300 K to 500,000 K: the exact parts' luminance
    /// is the direct V(λ) integral's photopic transmission within 10⁻⁴ at every node; and the
    /// parts at unit luminance are the raw colour `unit_rgb` lifts, ∫S c̄ ÷ ∫S ȳ.
    #[test]
    fn the_parts_luminance_is_the_photopic_light() {
        let observer = super::super::photometry::tests::observer();
        let sensor = sensor();
        for teff in [2_300.0, 3_000.0, 5_772.0, 10_000.0, 30_000.0, 5.0e5] {
            let r = reddening(&observer, &sensor, &Spectrum::blackbody(teff).bin_means());
            assert!(
                r.photopic_identity < 1e-4,
                "{teff} K: {}",
                r.photopic_identity
            );
            let raw = observer.raw_rgb(r.xyz);
            for c in 0..3 {
                assert!(
                    (r.parts[2 * c] - r.parts[2 * c + 1] - raw[c]).abs() < 1e-12,
                    "{teff} K: {:?} against {raw:?}",
                    r.parts
                );
            }
        }
    }

    /// The addendum's test 4, the fit's half, on blackbodies and a narrow band far out of gamut:
    /// the sim's `lift_into_gamut` of the raw colour is `unit_rgb`'s colour, bit for bit. (The
    /// task checks every row of the table, and the slow reproduction test with it.)
    #[test]
    fn the_sims_lift_is_unit_rgbs() {
        let observer = super::super::photometry::tests::observer();
        let mut spectra: Vec<Vec<f64>> = [1_500.0, 2_300.0, 3_000.0, 5_772.0, 30_000.0, 5.0e5]
            .iter()
            .map(|&t| Spectrum::blackbody(t).bin_means())
            .collect();
        let mut narrow = vec![0.0; BIN_COUNT];
        narrow[520 - 360] = 1.0;
        spectra.push(narrow);
        let mut lifted = 0;
        for bins in &spectra {
            let xyz = observer.integrals(bins).xyz;
            let raw = observer.raw_rgb(xyz);
            lifted += usize::from(raw.iter().any(|&c| c < 0.0));
            // `total_cmp` is equal exactly when the bits are.
            let (sim, fit) = (lift_into_gamut(raw), observer.unit_rgb(xyz));
            assert!(
                sim.iter().zip(&fit).all(|(a, b)| a.total_cmp(b).is_eq()),
                "{raw:?}: {sim:?} against {fit:?}"
            );
        }
        assert!(lifted >= 1, "an out-of-gamut colour is among them");
    }

    /// Each band's optical depth k(A) A rises through the nodes, and its moment is its secant's
    /// limit: the direct secant at `A_V` 10⁻⁴ is the moment to 10⁻⁴.
    #[test]
    fn the_secants_rise_in_depth_and_start_at_the_moments() {
        let observer = super::super::photometry::tests::observer();
        let sensor = sensor();
        let rgb_cmf = observer.rgb_cmf();
        let law = law_by_bin();
        for teff in [3_000.0, 5_772.0, 30_000.0] {
            let bins = Spectrum::blackbody(teff).bin_means();
            let r = reddening(&observer, &sensor, &bins);
            let (all, _) = band_sums(&observer, &sensor, &rgb_cmf, &bins);
            let (left, _) = band_sums(
                &observer,
                &sensor,
                &rgb_cmf,
                &dimmed_bins(&bins, &law, 1e-4),
            );
            for b in 0..BAND_COUNT {
                let small = -2.5 * math::log10(left[b] / all[b]) / 1e-4;
                assert!((small - r.moments[b]).abs() < 1e-4, "{teff} K, band {b}");
                let mut depth = 0.0;
                for (n, &a) in REDDENING_A_V_NODES.iter().enumerate() {
                    assert!(r.secants[n][b] * a > depth, "{teff} K, band {b} at {a}");
                    depth = r.secants[n][b] * a;
                }
            }
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
