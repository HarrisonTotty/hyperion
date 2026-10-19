//! The rows from laboratory spectra and the other inputs without an infrared index (plan 14,
//! P14.T49.i; decision-p14-t49i-rock-mixtures): each a [`SpectralGrain`] the bins average, and
//! whose w at 440, 550 and 640 nm the row's values in B, V and R take.
//!
//! # A laboratory spectrum, inverted
//!
//! Each splib07a spectrum (Kokaly et al. 2017, USGS Data Series 1035; the dataset
//! `usgs_splib07a`, read by [`LabLibrary`]) is inverted for w channel by channel, at its own
//! geometry, by the reference form's model ([`Measurement`], [`invert`]):
//!
//! - a Beckman spectrum, an integrating sphere's directional-hemispherical reflectance, as the
//!   similarity relation's plane albedo 1 − √(1 − w*) H(μ₀; w*) at the regolith form's asymmetry
//!   and μ₀ = cos 10° (`BECKMAN_INCIDENCE_DEG`);
//! - an ASD spectrum, a reflectance factor at i 20°, e 30° and g 35.5° (`ASD_GEOMETRY`), as the
//!   IMSA's (w ÷ 4) ÷ (μ₀ + μ) [p(g)(1 + B(g)) + H(μ₀) H(μ) − 1] under the regolith photometry;
//! - SO₂ frost's levels, a reflectance factor at their own geometry, under the frost's.
//!
//! The forward models rise strictly with w, so each channel's w is the one root: bracketed on a
//! table of the forward model at t = √(1 − w) = j ÷ 512 ([`InversionTable`]), and refined by the
//! Illinois method on the exact model ([`Measurement::forward`]) to a residual below 10⁻¹⁴. A
//! reflectance above the model's at w = 1 takes w = 1, and one at or below 0 takes w = 0 (each
//! counted, [`InvertedSpectrum`]).
//!
//! # To the reference form's grains
//!
//! The inverted w is the laboratory grains'. A transparent end-member is taken to the row's grains
//! through the equivalent slab's internal transmission at its stated real index n ([`rescale`]):
//! the slab's w = `S_e` + (1 − `S_e`)(1 − `S_i`) Θ ÷ (1 − `S_i` Θ) at k = 0's surface terms gives
//! Θ, and grains of diameter D take Θ^(D ÷ `D_lab`). A w below `S_e`, which no Θ gives, is held as
//! measured, as is every opaque end-member's and every one whose laboratory grain size is not
//! stated (`GrainScaling::Held`). Between channels w is linear in wavelength.
//!
//! # A mixture
//!
//! An intimate mixture's w is Hapke 2012 §10's cross-section weighting,
//! w̄ = Σ (fⱼ ÷ Dⱼ) wⱼ ÷ Σ (fⱼ ÷ Dⱼ), each part at the row's grains or at `FINE_GRAIN_DIAMETER`
//! ([`MixtureGrain`]), over the wavelengths every part's spectrum covers.
//!
//! # The opaque slab
//!
//! An opaque solid's grains (k above 0.1) transmit nothing at the form's 70 µm, so w is the
//! surface term alone, here the hemispherical average of the unpolarised complex Fresnel
//! reflectance, `S_e` = 2 ∫₀¹ R(μ; m) μ dμ ([`hemispherical_fresnel_reflectance`]), from R08's
//! index file's n and k ([`OpaqueGrain`]).

use std::collections::BTreeMap;
use std::sync::Arc;

use hyperion_sim::math;
use hyperion_sim::substance::optics::{
    ASD_GEOMETRY, BECKMAN_INCIDENCE_DEG, EndMember, FINE_GRAIN_DIAMETER, Geometry, GrainClass,
    GrainScaling, LabSpectrum, MixturePart, Photometry, ReflectanceLevels, SPECTRAL_BIN_EDGES_NM,
    SPECTRAL_BINS, Spectrometer,
};
use hyperion_sim::tables::gauss_legendre::{GL32_NODES, GL32_WEIGHTS};
use hyperion_sim::units::Metres;

use super::spectral::{Curve, Interpolation, Piece, SpectralGrain};
use super::{FitAlbedoError, MU_EDGES, similar_isotropic_albedo};

/// The dataset that holds the laboratory spectra: splib07a's files the rows read.
pub const SPLIB_DATASET: &str = "usgs_splib07a";

/// The Beckman's wavelength file in splib07a, µm by channel.
pub const BECKMAN_WAVELENGTHS: &str = "splib07a_Wavelengths_BECK_Beckman_0.2-3.0_microns.txt";

/// The ASD's wavelength file in splib07a, µm by channel, of every ASD resolution's 2,151
/// channels.
pub const ASD_WAVELENGTHS: &str = "splib07a_Wavelengths_ASD_0.35-2.5_microns_2151_ch.txt";

/// A value splib07a marks deleted, at or below which a channel is dropped: the release writes
/// −1.23 × 10³⁴ (Kokaly et al. 2017, DS 1035).
pub const DELETED_BELOW: f64 = -1.0e30;

/// Where the bins begin and end, nm: a spectrum's channels outside are not read.
const BINS_FROM_NM: f64 = SPECTRAL_BIN_EDGES_NM[0];

/// Where the bins end, nm.
const BINS_TO_NM: f64 = SPECTRAL_BIN_EDGES_NM[SPECTRAL_BINS];

/// The step, nm, at which a row of levels is sampled and inverted where it varies: 1.
pub const LEVELS_SAMPLE_NM: f64 = 1.0;

/// The values of one splib07a file: its header line's record name and its values in order, a
/// deleted one kept as written.
///
/// # Errors
///
/// [`FitAlbedoError::Spectrum`] if its first line is not a splib07a record's header or a value is
/// not a number.
pub fn splib_values(file: &'static str, text: &str) -> Result<(String, Vec<f64>), FitAlbedoError> {
    let mut lines = text.lines();
    let header = lines.next().unwrap_or_default();
    let record = header
        .trim()
        .strip_prefix("splib07a Record=")
        .and_then(|rest| rest.split_once(':'))
        .map(|(_, name)| name.split_whitespace().collect::<Vec<_>>().join("_"))
        .ok_or_else(|| spectrum_error(file, "its first line is not a splib07a header"))?;
    let values = lines
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| spectrum_error(file, "a value is not a number"))?;
    Ok((record, values))
}

/// The error of a spectrum that is not as its reader expects.
fn spectrum_error(file: &'static str, why: &str) -> FitAlbedoError {
    FitAlbedoError::Spectrum {
        file,
        why: why.to_owned(),
    }
}

/// The laboratory spectra of the `usgs_splib07a` dataset, each file's text by name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LabLibrary {
    texts: BTreeMap<String, String>,
}

impl LabLibrary {
    /// The library of `texts`, each splib07a file's text by its name.
    #[must_use]
    pub fn from_texts(texts: BTreeMap<String, String>) -> Self {
        Self { texts }
    }

    /// The text of `file`.
    fn text(&self, file: &'static str) -> Result<&str, FitAlbedoError> {
        self.texts
            .get(file)
            .map(String::as_str)
            .ok_or_else(|| spectrum_error(file, "it is not one of the dataset's files"))
    }

    /// The channels of `spectrum` inside the bins' 300–5,000 nm: (wavelength nm, reflectance),
    /// wavelength rising, deleted channels dropped, from its file and its spectrometer's
    /// wavelength file.
    ///
    /// # Errors
    ///
    /// [`FitAlbedoError::Spectrum`] if a file is missing or malformed, its header does not name its
    /// record (a header's "<" spelt "lt" in the file's name), its channels are not its wavelength
    /// file's, its wavelengths do not rise strictly, or fewer than two channels remain.
    pub fn channels(&self, spectrum: LabSpectrum) -> Result<Vec<(f64, f64)>, FitAlbedoError> {
        let file = spectrum.file();
        let wavelengths_file = match spectrum.spectrometer() {
            Spectrometer::Beckman => BECKMAN_WAVELENGTHS,
            Spectrometer::Asd => ASD_WAVELENGTHS,
        };
        let (record, values) = splib_values(file, self.text(file)?)?;
        let stem = file
            .strip_prefix("splib07a_")
            .and_then(|f| f.strip_suffix(".txt"))
            .ok_or_else(|| spectrum_error(file, "its name is not splib07a_<record>.txt"))?;
        // The release's file names spell a header's "<" as "lt" ("<60um" as "lt60um").
        if record.replace('<', "lt") != stem {
            return Err(spectrum_error(
                file,
                &format!("its header names the record `{record}`"),
            ));
        }
        let (_, wavelengths_um) = splib_values(wavelengths_file, self.text(wavelengths_file)?)?;
        if wavelengths_um.len() != values.len() {
            return Err(spectrum_error(
                file,
                &format!(
                    "it has {} channels and its wavelength file {}",
                    values.len(),
                    wavelengths_um.len()
                ),
            ));
        }
        let channels: Vec<(f64, f64)> = wavelengths_um
            .iter()
            .zip(&values)
            .map(|(&um, &value)| (um * 1_000.0, value))
            .filter(|&(nm, value)| {
                value > DELETED_BELOW && (BINS_FROM_NM..=BINS_TO_NM).contains(&nm)
            })
            .collect();
        if channels.len() < 2 || !channels.windows(2).all(|pair| pair[0].0 < pair[1].0) {
            return Err(spectrum_error(
                file,
                "its channels are fewer than two or do not rise",
            ));
        }
        Ok(channels)
    }
}

/// The cosines (μ₀, μ, cos g) of `geometry`: cos g = μ₀ μ + sin i sin e cos φ.
#[must_use]
pub fn cosines(geometry: Geometry) -> (f64, f64, f64) {
    let radians = core::f64::consts::PI / 180.0;
    let (sin_i, mu0) = math::sin_cos(geometry.incidence_deg * radians);
    let (sin_e, mu) = math::sin_cos(geometry.emission_deg * radians);
    let cos_phi = math::cos(geometry.azimuth_deg * radians);
    (mu0, mu, mu0 * mu + sin_i * sin_e * cos_phi)
}

/// Chandrasekhar's H-function at one μ, its quadrature's nodes precomputed: what
/// [`super::h_function`] computes, for any w, without its trigonometry at each call.
#[derive(Debug, Clone, PartialEq)]
pub struct HAtMu {
    mu: f64,
    /// Each node's weight (its kernel included) and its bracket's form: `(c, a, series)`, the
    /// bracket being (1 − w) + w a where `series`, else 1 − w a.
    nodes: Vec<(f64, f64, bool)>,
}

impl HAtMu {
    /// H at `mu` in [0, 1], by [`super::h_function`]'s quadrature.
    #[must_use]
    pub fn new(mu: f64) -> Self {
        let half_pi = core::f64::consts::FRAC_PI_2;
        let panels = super::H_PANELS;
        let mut nodes = Vec::new();
        for panel in 0..panels {
            let lo = f64::from(panel) / f64::from(panels);
            let hi = f64::from(panel + 1) / f64::from(panels);
            let (mid, half) = (f64::midpoint(lo, hi), 0.5 * (hi - lo));
            for (x, weight) in GL32_NODES.iter().zip(GL32_WEIGHTS) {
                let s = mid + half * x;
                let theta = half_pi * s * s;
                let (sin, cos) = math::sin_cos(theta);
                let denominator = cos * cos + mu * mu * sin * sin;
                let c = half * weight / denominator * (core::f64::consts::PI * s);
                if theta < super::H_SERIES_BELOW_RAD {
                    let t2 = theta * theta;
                    let a = t2 / 3.0
                        + t2 * t2 / 45.0
                        + 2.0 * t2 * t2 * t2 / 945.0
                        + t2 * t2 * t2 * t2 / 4_725.0;
                    nodes.push((c, a, true));
                } else {
                    nodes.push((c, theta / math::tan(theta), false));
                }
            }
        }
        Self { mu, nodes }
    }

    /// H(μ; w) for `w` in [0, 1].
    #[must_use]
    pub fn at(&self, w: f64) -> f64 {
        if self.mu <= 0.0 || w <= 0.0 {
            return 1.0;
        }
        let sum = self.nodes.iter().fold(0.0, |sum, &(c, a, series)| {
            let bracket = if series {
                (1.0 - w).max(0.0) + w * a
            } else {
                1.0 - w * a
            };
            sum + c * math::ln(bracket)
        });
        math::exp(-self.mu / core::f64::consts::PI * sum)
    }
}

/// What a spectrum measured, and the model that gives it from w.
#[derive(Debug, Clone, PartialEq)]
pub enum Measurement {
    /// A directional-hemispherical reflectance at incidence cosine μ₀, by the similarity relation's
    /// isotropic medium at the grains' asymmetry ξ: 1 − √(1 − w*) H(μ₀; w*), w* = w (1 − ξ) ÷ (1 −
    /// w ξ).
    DirectionalHemispherical {
        /// ξ, the phase function's asymmetry.
        asymmetry: f64,
        /// H at μ₀.
        h0: HAtMu,
    },
    /// A reflectance factor at (μ₀, μ, cos g) under a photometry, by the IMSA: (w ÷ 4) ÷ (μ₀ + μ)
    /// [p(g)(1 + B(g)) + H(μ₀) H(μ) − 1].
    ReflectanceFactor {
        /// The photometry.
        photometry: Photometry,
        /// The phase cosine.
        cos_g: f64,
        /// H at μ₀.
        h0: HAtMu,
        /// H at μ.
        h: HAtMu,
    },
}

impl Measurement {
    /// A Beckman spectrum's, for grains of phase-function asymmetry `asymmetry`, at μ₀ = cos 10°.
    #[must_use]
    pub fn beckman(asymmetry: f64) -> Self {
        let mu0 = math::cos(BECKMAN_INCIDENCE_DEG * core::f64::consts::PI / 180.0);
        Self::DirectionalHemispherical {
            asymmetry,
            h0: HAtMu::new(mu0),
        }
    }

    /// A reflectance factor at `geometry` under `photometry`: an ASD spectrum's at
    /// `ASD_GEOMETRY`, or a row of levels' at its own.
    #[must_use]
    pub fn reflectance_factor(geometry: Geometry, photometry: &Photometry) -> Self {
        let (mu0, mu, cos_g) = cosines(geometry);
        Self::ReflectanceFactor {
            photometry: *photometry,
            cos_g,
            h0: HAtMu::new(mu0),
            h: HAtMu::new(mu),
        }
    }

    /// An ASD spectrum's, under `photometry`.
    #[must_use]
    pub fn asd(photometry: &Photometry) -> Self {
        Self::reflectance_factor(ASD_GEOMETRY, photometry)
    }

    /// The measured quantity of grains of single-scattering albedo `w`, in [0, 1].
    #[must_use]
    pub fn forward(&self, w: f64) -> f64 {
        match self {
            Self::DirectionalHemispherical { asymmetry, h0 } => {
                let similar = similar_isotropic_albedo(w, *asymmetry);
                1.0 - (1.0 - similar).max(0.0).sqrt() * h0.at(similar)
            }
            Self::ReflectanceFactor {
                photometry,
                cos_g,
                h0,
                h,
            } => {
                let (mu0, mu) = (h0.mu, h.mu);
                super::reflectance_factor_with(w, photometry, (mu0, mu, *cos_g), h0.at(w), h.at(w))
            }
        }
    }
}

/// The intervals of an [`InversionTable`]'s grid in t = √(1 − w): 512.
const INVERSION_INTERVALS: u32 = 512;

/// A measurement's forward model at t = √(1 − w) = j ÷ 512, j = 0 to 512, falling with t: the
/// brackets of [`invert`].
#[derive(Debug, Clone, PartialEq)]
pub struct InversionTable {
    measurement: Measurement,
    values: Vec<f64>,
}

impl InversionTable {
    /// The table of `measurement`: 513 evaluations of its model.
    #[must_use]
    pub fn new(measurement: Measurement) -> Self {
        let values = (0..=INVERSION_INTERVALS)
            .map(|j| {
                let t = f64::from(j) / f64::from(INVERSION_INTERVALS);
                measurement.forward(1.0 - t * t)
            })
            .collect();
        Self {
            measurement,
            values,
        }
    }

    /// The measurement.
    #[must_use]
    pub const fn measurement(&self) -> &Measurement {
        &self.measurement
    }
}

/// Where an inversion's measured value lay outside the model's range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Clip {
    /// At or above the model's value at w = 1: w = 1.
    Bright,
    /// At or below 0: w = 0.
    Dark,
}

/// The largest number of the Illinois method's steps.
const ILLINOIS_STEPS: u32 = 100;

/// The residual below which the Illinois method stops: 10⁻¹⁵.
const INVERSION_RESIDUAL: f64 = 1.0e-15;

/// The single-scattering albedo whose `table`'s measurement is `value`, and the clip if it lay
/// outside the model's range ([`Clip`]; the [module documentation](self)).
///
/// # Panics
///
/// Never: a node's index is at most 512, which a `u32` holds.
#[must_use]
pub fn invert(table: &InversionTable, value: f64) -> (f64, Option<Clip>) {
    let values = &table.values;
    if value >= values[0] {
        return (1.0, Some(Clip::Bright));
    }
    if value <= 0.0 {
        return (0.0, Some(Clip::Dark));
    }
    // The first node at or below the value: values fall with j, and values[0] > value.
    let upper = values.partition_point(|&v| v > value);
    let n = f64::from(INVERSION_INTERVALS);
    let to_t = |j: usize| f64::from(u32::try_from(j).expect("a node of 513")) / n;
    if upper >= values.len() {
        return (0.0, Some(Clip::Dark));
    }
    let (mut a, mut b) = (to_t(upper - 1), to_t(upper));
    let (mut fa, mut fb) = (values[upper - 1] - value, values[upper] - value);
    if fb >= 0.0 {
        return (1.0 - b * b, None);
    }
    let forward = |t: f64| table.measurement.forward(1.0 - t * t) - value;
    let mut side = 0_i8;
    let mut t = b;
    for _ in 0..ILLINOIS_STEPS {
        t = (a * fb - b * fa) / (fb - fa);
        let ft = forward(t);
        if ft.abs() <= INVERSION_RESIDUAL || (b - a).abs() <= 4.0 * f64::EPSILON {
            break;
        }
        // fa > 0 > fb: the root lies on the side whose sign differs from f(t)'s.
        if ft > 0.0 {
            a = t;
            fa = ft;
            if side == 1 {
                fb /= 2.0;
            }
            side = 1;
        } else {
            b = t;
            fb = ft;
            if side == -1 {
                fa /= 2.0;
            }
            side = -1;
        }
    }
    (1.0 - t * t, None)
}

/// A spectrum inverted for w channel by channel at its own geometry: (wavelength nm, w), the
/// counts of its clipped channels, and its largest residual against the model over the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct InvertedSpectrum {
    /// The channels, (wavelength nm, w at the laboratory grains).
    pub channels: Vec<(f64, f64)>,
    /// The channels above the model's value at w = 1.
    pub bright: usize,
    /// The channels at or below 0.
    pub dark: usize,
    /// The largest |model − measured| over the channels not clipped.
    pub residual: f64,
}

/// `channels`, (wavelength nm, measured value), inverted by `table`.
#[must_use]
pub fn invert_channels(table: &InversionTable, channels: &[(f64, f64)]) -> InvertedSpectrum {
    let mut inverted = InvertedSpectrum {
        channels: Vec::with_capacity(channels.len()),
        bright: 0,
        dark: 0,
        residual: 0.0,
    };
    for &(nm, value) in channels {
        let (w, clip) = invert(table, value);
        match clip {
            Some(Clip::Bright) => inverted.bright += 1,
            Some(Clip::Dark) => inverted.dark += 1,
            None => {
                let residual = (table.measurement.forward(w) - value).abs();
                inverted.residual = inverted.residual.max(residual);
            }
        }
        inverted.channels.push((nm, w));
    }
    inverted
}

/// The surface terms (`S_e`, `S_i`) of Hapke 2012's fits at real index `n` and k = 0, as
/// [`super::single_scattering_albedo`] takes them.
#[must_use]
pub fn surface_terms(n: f64) -> (f64, f64) {
    let r0 = (n - 1.0) * (n - 1.0) / ((n + 1.0) * (n + 1.0));
    let s_e = 0.0587 + 0.8543 * r0 + 0.0870 * r0 * r0;
    let s_i = 1.0 - (0.9413 - 0.8543 * r0 - 0.0870 * r0 * r0) / (n * n);
    (s_e, s_i)
}

/// The slab's w at internal transmission `theta` and surface terms `s_e`, `s_i`, held at most 1.
#[must_use]
pub fn slab_w(theta: f64, s_e: f64, s_i: f64) -> f64 {
    let w = s_e + (1.0 - s_e) * (1.0 - s_i) * theta / (1.0 - s_i * theta);
    if w > 1.0 { 1.0 } else { w }
}

/// The slab's internal transmission Θ whose w is `w` at surface terms `s_e`, `s_i`: the inverse of
/// [`slab_w`], 0 at or below `S_e` and 1 at or above 1.
#[must_use]
pub fn slab_theta(w: f64, s_e: f64, s_i: f64) -> f64 {
    if w <= s_e {
        return 0.0;
    }
    if w >= 1.0 {
        return 1.0;
    }
    (w - s_e) / ((1.0 - s_e) * (1.0 - s_i) + s_i * (w - s_e))
}

/// `w` of laboratory grains taken to grains of `diameter` by `scaling` (the [module
/// documentation](self)): through the slab, Θ^(D ÷ `D_lab`), where w lies above `S_e`; else, or
/// where held, `w` itself.
#[must_use]
pub fn rescale(w: f64, scaling: GrainScaling, diameter: Metres) -> f64 {
    match scaling {
        GrainScaling::Held => w,
        GrainScaling::Slab {
            real_index,
            lab_grain_diameter,
        } => {
            let (s_e, s_i) = surface_terms(real_index);
            let theta = slab_theta(w, s_e, s_i);
            if theta <= 0.0 {
                return w;
            }
            let power = diameter.value() / lab_grain_diameter.value();
            slab_w(math::powf(theta, power), s_e, s_i)
        }
    }
}

/// A complex number, for the Fresnel coefficients.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    fn add(self, other: Self) -> Self {
        Self::new(self.re + other.re, self.im + other.im)
    }

    fn sub(self, other: Self) -> Self {
        Self::new(self.re - other.re, self.im - other.im)
    }

    fn mul(self, other: Self) -> Self {
        Self::new(
            self.re * other.re - self.im * other.im,
            self.re * other.im + self.im * other.re,
        )
    }

    fn div(self, other: Self) -> Self {
        let d = other.re * other.re + other.im * other.im;
        Self::new(
            (self.re * other.re + self.im * other.im) / d,
            (self.im * other.re - self.re * other.im) / d,
        )
    }

    fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// The principal square root, its real part at least 0.
    fn sqrt(self) -> Self {
        let r = math::hypot(self.re, self.im);
        let re = f64::midpoint(r, self.re).max(0.0).sqrt();
        let im = (0.5 * (r - self.re)).max(0.0).sqrt();
        Self::new(re, if self.im < 0.0 { -im } else { im })
    }
}

/// The unpolarised Fresnel reflectance of a smooth surface of index m = `n` + i`k` in vacuum at
/// incidence cosine `mu`: the mean of the squared moduli of the s and p amplitude coefficients,
/// `r_s = (μ − m cos θₜ) ÷ (μ + m cos θₜ)` and `r_p = (m μ − cos θₜ) ÷ (m μ + cos θₜ)`, with Snell's
/// `m sin θₜ = sin θ` and `cos θₜ = √(1 − sin² θ ÷ m²)` on its principal branch, the transmitted
/// wave decaying into the medium (Byrnes 2016, "Multilayer optical calculations", arXiv
/// 1603.02720, eqs. 3 and 6 and App. D.2, for fields as e^(−iωt), Im n > 0 absorbing; p's sign, which
/// half of the textbooks reverse, moves no modulus).
#[must_use]
pub fn fresnel_reflectance(n: f64, k: f64, mu: f64) -> f64 {
    let m = Complex::new(n, k);
    let mu_c = Complex::new(mu, 0.0);
    let sin2 = Complex::new((1.0 - mu * mu).max(0.0), 0.0);
    let cos_t = Complex::new(1.0, 0.0).sub(sin2.div(m.mul(m))).sqrt();
    let m_cos_t = m.mul(cos_t);
    let r_s = mu_c.sub(m_cos_t).div(mu_c.add(m_cos_t));
    let m_mu = m.mul(mu_c);
    let r_p = m_mu.sub(cos_t).div(m_mu.add(cos_t));
    f64::midpoint(r_s.norm_sqr(), r_p.norm_sqr())
}

/// The hemispherically averaged Fresnel reflectance of a smooth surface of index `n` + i`k`, the
/// slab's surface term for an opaque grain: `S_e` = 2 ∫₀¹ R(μ) μ dμ, by 32-point Gauss–Legendre on
/// each panel of the hemisphere's [`MU_EDGES`], summed in order.
#[must_use]
pub fn hemispherical_fresnel_reflectance(n: f64, k: f64) -> f64 {
    let mut sum = 0.0;
    for pair in MU_EDGES.windows(2) {
        let (mid, half) = (f64::midpoint(pair[0], pair[1]), 0.5 * (pair[1] - pair[0]));
        for (x, weight) in GL32_NODES.iter().zip(GL32_WEIGHTS) {
            let mu = mid + half * x;
            sum += half * weight * mu * fresnel_reflectance(n, k, mu);
        }
    }
    2.0 * sum
}

/// One end-member's grain: its spectra inverted at their own geometries and how its w is taken to
/// a row's grains; its w the mean of its spectra's, each linear in wavelength between channels.
#[derive(Debug, Clone, PartialEq)]
pub struct EndMemberGrain {
    name: &'static str,
    scaling: GrainScaling,
    spectra: Vec<InvertedSpectrum>,
    reach: [f64; 2],
}

impl EndMemberGrain {
    /// The grain of `end_member`, from `library`, each spectrum inverted by `tables`'s table of its
    /// spectrometer.
    ///
    /// # Errors
    ///
    /// As [`LabLibrary::channels`]; [`FitAlbedoError::Spectrum`] if its spectra share no
    /// wavelength.
    pub fn new(
        end_member: &EndMember,
        library: &LabLibrary,
        tables: &SpectrometerTables,
    ) -> Result<Self, FitAlbedoError> {
        let mut spectra = Vec::with_capacity(end_member.spectra().len());
        for spectrum in end_member.spectra() {
            let channels = library.channels(*spectrum)?;
            spectra.push(invert_channels(
                tables.of(spectrum.spectrometer()),
                &channels,
            ));
        }
        let reach = spectra.iter().fold([BINS_FROM_NM, BINS_TO_NM], |r, s| {
            let (first, last) = (s.channels[0].0, s.channels[s.channels.len() - 1].0);
            [r[0].max(first), r[1].min(last)]
        });
        if reach[0] >= reach[1] {
            let file = end_member.spectra()[0].file();
            return Err(spectrum_error(
                file,
                "its end-member's spectra share no wavelength",
            ));
        }
        Ok(Self {
            name: end_member.name(),
            scaling: end_member.scaling(),
            spectra,
            reach,
        })
    }

    /// Its name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Its spectra, inverted.
    #[must_use]
    pub fn spectra(&self) -> &[InvertedSpectrum] {
        &self.spectra
    }
}

impl SpectralGrain for EndMemberGrain {
    fn reach_nm(&self) -> [f64; 2] {
        self.reach
    }

    fn knots_nm(&self) -> Vec<f64> {
        let [lo, hi] = self.reach;
        let mut knots: Vec<f64> = self
            .spectra
            .iter()
            .flat_map(|spectrum| spectrum.channels.iter().map(|&(nm, _)| nm))
            .filter(|&nm| nm >= lo && nm <= hi)
            .collect();
        knots.sort_by(f64::total_cmp);
        knots.dedup_by(|a, b| a.total_cmp(b).is_eq());
        knots
    }

    /// The mean of its spectra's w at the wavelength, each spectrum's channels taken to grains of
    /// `diameter` by its scaling and w linear between them.
    fn single_scattering_albedo(&self, wavelength_nm: f64, diameter: Metres) -> f64 {
        let mut sum = 0.0;
        for spectrum in &self.spectra {
            let channels = &spectrum.channels;
            let upper = channels
                .partition_point(|&(nm, _)| nm < wavelength_nm)
                .min(channels.len() - 1);
            let lower = upper.saturating_sub(1);
            let scaled = |j: usize| rescale(channels[j].1, self.scaling, diameter);
            sum += if lower == upper || channels[upper].0.total_cmp(&wavelength_nm).is_eq() {
                scaled(upper)
            } else {
                let t =
                    (wavelength_nm - channels[lower].0) / (channels[upper].0 - channels[lower].0);
                let (a, b) = (scaled(lower), scaled(upper));
                a + t * (b - a)
            };
        }
        sum / f64::from(u32::try_from(self.spectra.len()).expect("a few spectra"))
    }
}

/// The inversion tables of the regolith form's two spectrometers.
#[derive(Debug, Clone, PartialEq)]
pub struct SpectrometerTables {
    beckman: InversionTable,
    asd: InversionTable,
}

impl SpectrometerTables {
    /// The tables under `photometry`, the regolith form's.
    #[must_use]
    pub fn new(photometry: &Photometry) -> Self {
        let asymmetry = photometry.phase_function().asymmetry();
        Self {
            beckman: InversionTable::new(Measurement::beckman(asymmetry)),
            asd: InversionTable::new(Measurement::asd(photometry)),
        }
    }

    /// The table of `spectrometer`.
    #[must_use]
    pub const fn of(&self, spectrometer: Spectrometer) -> &InversionTable {
        match spectrometer {
            Spectrometer::Beckman => &self.beckman,
            Spectrometer::Asd => &self.asd,
        }
    }
}

/// The end-members' grains, each inverted once in a fit, by name.
#[derive(Debug, Clone, Default)]
pub struct EndMemberCache {
    grains: BTreeMap<&'static str, Arc<EndMemberGrain>>,
}

impl EndMemberCache {
    /// The empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The grain of `end_member`, inverted on first use.
    ///
    /// # Errors
    ///
    /// As [`EndMemberGrain::new`].
    pub fn grain(
        &mut self,
        end_member: &EndMember,
        library: &LabLibrary,
        tables: &SpectrometerTables,
    ) -> Result<Arc<EndMemberGrain>, FitAlbedoError> {
        if let Some(grain) = self.grains.get(end_member.name()) {
            return Ok(Arc::clone(grain));
        }
        let grain = Arc::new(EndMemberGrain::new(end_member, library, tables)?);
        self.grains.insert(end_member.name(), Arc::clone(&grain));
        Ok(grain)
    }
}

/// An intimate mixture's grain (the [module documentation](self)): its parts' grains and weights.
#[derive(Debug, Clone, PartialEq)]
pub struct MixtureGrain {
    /// Each part's grain, volume fraction and grain class.
    parts: Vec<(Arc<EndMemberGrain>, f64, GrainClass)>,
    reach: [f64; 2],
}

impl MixtureGrain {
    /// The mixture of `parts`, each end-member's grain from `cache`.
    ///
    /// # Errors
    ///
    /// As [`EndMemberCache::grain`]; [`FitAlbedoError::Spectrum`] if the parts' spectra share no
    /// wavelength.
    pub fn new(
        parts: &[MixturePart],
        library: &LabLibrary,
        tables: &SpectrometerTables,
        cache: &mut EndMemberCache,
    ) -> Result<Self, FitAlbedoError> {
        let first = parts
            .first()
            .ok_or_else(|| spectrum_error("no file", "a mixture has no part"))?;
        let mut grains = Vec::with_capacity(parts.len());
        let mut reach = [BINS_FROM_NM, BINS_TO_NM];
        for part in parts {
            let grain = cache.grain(part.end_member(), library, tables)?;
            let [lo, hi] = grain.reach_nm();
            reach = [reach[0].max(lo), reach[1].min(hi)];
            grains.push((grain, part.volume_fraction(), part.grain()));
        }
        if reach[0] >= reach[1] {
            let file = first.end_member().spectra()[0].file();
            return Err(spectrum_error(
                file,
                "a mixture's spectra share no wavelength",
            ));
        }
        Ok(Self {
            parts: grains,
            reach,
        })
    }

    /// Each part's cross-section weight fⱼ ÷ Dⱼ, m⁻¹, for a form of grains `diameter`.
    #[must_use]
    pub fn weights(&self, diameter: Metres) -> Vec<f64> {
        self.parts
            .iter()
            .map(|(_, fraction, grain)| fraction / part_diameter(*grain, diameter).value())
            .collect()
    }
}

/// The diameter of a part of class `grain` in a form of grains `diameter`.
#[must_use]
pub const fn part_diameter(grain: GrainClass, diameter: Metres) -> Metres {
    match grain {
        GrainClass::Form => diameter,
        GrainClass::Fine => FINE_GRAIN_DIAMETER,
    }
}

impl SpectralGrain for MixtureGrain {
    fn reach_nm(&self) -> [f64; 2] {
        self.reach
    }

    fn knots_nm(&self) -> Vec<f64> {
        let [lo, hi] = self.reach;
        let mut knots: Vec<f64> = self
            .parts
            .iter()
            .flat_map(|(grain, _, _)| grain.knots_nm())
            .filter(|&nm| nm >= lo && nm <= hi)
            .collect();
        knots.sort_by(f64::total_cmp);
        knots.dedup_by(|a, b| a.total_cmp(b).is_eq());
        knots
    }

    /// Σ (fⱼ ÷ Dⱼ) wⱼ ÷ Σ (fⱼ ÷ Dⱼ), each part's w at its own diameter, summed in the parts' order.
    fn single_scattering_albedo(&self, wavelength_nm: f64, diameter: Metres) -> f64 {
        let (mut sum, mut weights) = (0.0, 0.0);
        for (grain, fraction, class) in &self.parts {
            let d = part_diameter(*class, diameter);
            let weight = fraction / d.value();
            sum += weight * grain.single_scattering_albedo(wavelength_nm, d);
            weights += weight;
        }
        sum / weights
    }
}

/// A row of reflectance levels' grain: the levels sampled where they vary, each sample inverted
/// under the row's photometry at the levels' geometry, w linear between samples.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelsGrain {
    inverted: InvertedSpectrum,
    curve: Curve,
    reach: [f64; 2],
}

impl LevelsGrain {
    /// The grain of `levels` under `photometry`: the levels at each knot and every
    /// [`LEVELS_SAMPLE_NM`] between two knots of different levels, inverted.
    ///
    /// # Panics
    ///
    /// If the levels have fewer than two knots or their wavelengths do not rise: the rows' are
    /// stated so.
    #[must_use]
    pub fn new(levels: &ReflectanceLevels, photometry: &Photometry) -> Self {
        let knots = levels.knots();
        assert!(
            knots.len() >= 2 && knots.windows(2).all(|p| p[0].0 < p[1].0),
            "levels have two knots or more, rising"
        );
        let mut samples = vec![knots[0]];
        for pair in knots.windows(2) {
            let ((a, ra), (b, rb)) = (pair[0], pair[1]);
            if ra.total_cmp(&rb).is_ne() {
                let steps = ((b - a) / LEVELS_SAMPLE_NM).ceil();
                let mut j = 1.0;
                while j < steps {
                    let nm = a + j * (b - a) / steps;
                    samples.push((nm, ra + (nm - a) / (b - a) * (rb - ra)));
                    j += 1.0;
                }
            }
            samples.push((b, rb));
        }
        let table = InversionTable::new(Measurement::reflectance_factor(
            levels.geometry(),
            photometry,
        ));
        let inverted = invert_channels(&table, &samples);
        let curve = Curve::new(&inverted.channels, Interpolation::Linear);
        let reach = [
            knots[0].0.max(BINS_FROM_NM),
            knots[knots.len() - 1].0.min(BINS_TO_NM),
        ];
        Self {
            inverted,
            curve,
            reach,
        }
    }

    /// The inverted samples.
    #[must_use]
    pub const fn inverted(&self) -> &InvertedSpectrum {
        &self.inverted
    }
}

impl SpectralGrain for LevelsGrain {
    fn reach_nm(&self) -> [f64; 2] {
        self.reach
    }

    fn knots_nm(&self) -> Vec<f64> {
        let [lo, hi] = self.reach;
        self.curve
            .wavelengths_nm()
            .iter()
            .copied()
            .filter(|&nm| nm >= lo && nm <= hi)
            .collect()
    }

    /// The inverted w, with no grain size: the levels' grains are not stated.
    fn single_scattering_albedo(&self, wavelength_nm: f64, _diameter: Metres) -> f64 {
        self.curve.at(wavelength_nm)
    }
}

/// An opaque solid's grain: w = `S_e` of its index's complex Fresnel reflectance at each
/// wavelength, from R08's file's piece.
#[derive(Debug, Clone, PartialEq)]
pub struct OpaqueGrain {
    piece: Piece,
}

impl OpaqueGrain {
    /// The grain of R08's file's `piece`.
    #[must_use]
    pub const fn new(piece: Piece) -> Self {
        Self { piece }
    }
}

impl SpectralGrain for OpaqueGrain {
    fn reach_nm(&self) -> [f64; 2] {
        [self.piece.lo_nm(), self.piece.hi_nm()]
    }

    fn knots_nm(&self) -> Vec<f64> {
        self.piece.knots_nm()
    }

    /// `S_e` at the wavelength, at any grain size the form's photometry takes: the grains
    /// transmit nothing (k above 0.1 at the form's 70 µm gives Θ below 10⁻¹⁰⁰).
    fn single_scattering_albedo(&self, wavelength_nm: f64, _diameter: Metres) -> f64 {
        let (n, k) = self.piece.at(wavelength_nm);
        hemispherical_fresnel_reflectance(n, k)
    }
}

/// A grain whose w is held at most a limit: a regolith row from a laboratory spectrum at
/// `REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO`, the lunar photometry's energy bound, a frost's at 1.
#[derive(Debug, Clone, PartialEq)]
pub struct Clamped<G> {
    grain: G,
    limit: f64,
}

impl<G: SpectralGrain> Clamped<G> {
    /// `grain`, its w held at most `limit`.
    #[must_use]
    pub const fn new(grain: G, limit: f64) -> Self {
        Self { grain, limit }
    }

    /// The grain, unclamped.
    #[must_use]
    pub const fn inner(&self) -> &G {
        &self.grain
    }

    /// The limit.
    #[must_use]
    pub const fn limit(&self) -> f64 {
        self.limit
    }
}

impl<G: SpectralGrain> SpectralGrain for Clamped<G> {
    fn reach_nm(&self) -> [f64; 2] {
        self.grain.reach_nm()
    }

    fn knots_nm(&self) -> Vec<f64> {
        self.grain.knots_nm()
    }

    fn single_scattering_albedo(&self, wavelength_nm: f64, diameter: Metres) -> f64 {
        self.grain
            .single_scattering_albedo(wavelength_nm, diameter)
            .min(self.limit)
    }
}

#[cfg(test)]
mod tests;
