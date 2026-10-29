//! `run rough_plains_cold_traps`: the permanent shadow of rough plains, and the shares of it that
//! sunlit terrain warms least, by ray tracing Gaussian surfaces as Hayne et al. 2021's Methods D
//! builds them: the table that `hyperion_surface::cold_trap::rough_plains` reads (rendering plan
//! R09, R09.T7.g; `science-r09-drainage.md` §8.13, signed off in `signoff-12.md` item 13).
//!
//! # The surfaces
//!
//! Each surface is periodic, on a square grid of unit spacing whose axes point east and north: the
//! real part of a sum of Fourier modes at the integer wavenumbers k (cycles across the domain)
//! with `wavenumber_low` ≤ |k| ≤ `wavenumber_high`, each of amplitude |k|^−(1+H), H the Hurst
//! exponent (Hayne et al.'s 0.9), and of a random phase. A mode's phase is one uniform draw from
//! the task's own counter-based stream ([`Draws`], as the displaced forms' orbits and the
//! atmosphere tracer draw), keyed by the manifest's seed salted with [`ROUGH_PLAINS_STREAM`], the
//! surface's number and the mode's signed wavenumbers, so that no phase depends on which other
//! modes are drawn, and a surface is the same on every grid that holds its modes. The sum is one
//! inverse fast Fourier transform. Its heights are scaled to a directional RMS slope of 1 over the
//! grid's facets, the forward differences along both axes, which are the slopes the bilinear
//! surface's rays meet; a plain of directional RMS slope w = tan `σ_s` is that surface with its
//! heights times w. Hayne et al. strip a margin from a surface that is not periodic; these are
//! periodic instead, so every pixel is traced alike.
//!
//! # The horizons and the permanent shadow
//!
//! From each pixel a ray at each of `azimuths` fixed azimuths marches the bilinear surface in
//! steps of `step` pixels out to `reach`, and keeps the steepest tangent, tan h, of the terrain it
//! meets: the horizon's elevation at that azimuth. It stops once even the surface's highest point
//! could no longer rise above the horizon found so far. Under the vertical scaling a plain of slope
//! w has every tangent times w, so one trace of each surface serves every slope.
//!
//! A point star whose greatest declination is D = δ + α (for a disk, its upper limb's) sweeps every
//! declination within D of the equator over a year. At colatitude e₀ the declination of the sky at
//! azimuth φ (from north) and elevation a is given by sin dec = cos e₀ sin a + sin e₀ cos a cos φ,
//! a sinusoid in a. Up each azimuth's vertical, from the horizon to the zenith, it either rises
//! to one maximum and falls back to 90° − e₀ at the zenith, or rises all the way, or (toward the
//! equator, from a horizon more than 90° − e₀ below the level) first falls below −(90° − e₀),
//! under −D. So wherever e₀ + D < 90°, the sky a pixel sees up an azimuth reaches a declination
//! within D exactly when its horizon's declination is at most D, and a pixel is in permanent
//! shadow exactly when every azimuth's horizon has a declination above D: the chord test that
//! `bowl_permanent_shadow` makes at a bowl's rim. Each pixel's least horizon declination is found
//! once a slope and colatitude, and serves every D. Where e₀ + D ≥ 90° the star passes the zenith,
//! which a height field cannot hide, and nothing is in permanent shadow.
//!
//! # The irradiance
//!
//! A permanently shadowed pixel still sees sunlit terrain, and its peak irradiance from that
//! terrain sets its peak temperature. It is taken to first order, as `science-r09-drainage.md`
//! §8.13's tracer takes it: each facet the star lights is a Lambertian source of its own direct
//! flux S μ, μ the cosine of its incidence, and the shadowed pixel's irradiance is the sum of
//! those fluxes weighted by the view factor from the pixel to each, E = Σ F S μ. It has no albedo
//! or emissivity in it: a body's own (1 − A)(A + ε) turns it into the absorbed flux, since a lit
//! facet scatters A of its flux and re-emits the absorbed 1 − A in the thermal infrared, of which
//! the shadow absorbs 1 − A and ε (`signoff-12.md` item 13, F1). It leaves out light scattered
//! more than once and the shadowed terrain's own emission, which would add to it, and takes the
//! star as a point at its upper limb, which lights the facets a little more than its disk does.
//!
//! The terrain a pixel sees along each view azimuth (every `view_stride`-th azimuth) is the part of
//! the ray's profile that rises above all of it nearer: a piece wherever the ray's steepest tangent
//! grows, spanning the elevations from the previous steepest to its own. A piece is the facet of
//! the pixel nearest its sample, with that pixel's normal, and is lit when the star stands above
//! that pixel's own horizon. Its view factor is (Δφ ÷ π) ∫ max(0, n · d) cos a da over its
//! elevations, n the shadowed pixel's normal and d the direction, in closed form. The star's daily
//! path at declination D, which lies above every other day's at every azimuth, is sampled at every
//! `sun_stride`-th azimuth, and the peak is the largest E over the day. On a spherical bowl every
//! shadowed point's E is f (1 − f) S sin e, f the bowl's view factor 4γ² ÷ (1 + 4γ²), the
//! first-order part of the cavity balance of Ingersoll, Svitek and Murray (1992, Icarus 100, 40);
//! the tests hold the tracer to it.
//!
//! # The axes
//!
//! The slope w = tan `σ_s` at the manifest's nodes, from 0.05 to 1 (`Terrain`'s 45°); the
//! colatitude in radians over w on [0, `colatitude_range`]; and D in radians over w on
//! [0, `limb_range`]. Heights and angles scale together, so at small slopes a plain's shadow
//! depends on these ratios alone, and a slope below 0.05 reads the 0.05 slice at its own ratios.
//! For each node the table holds the permanent shadow's share of the ground, and, on every
//! `cold_colatitude_stride`-th colatitude and `cold_limb_stride`-th limb node, the shares of the
//! shadow whose peak irradiance is at most each of fixed fractions of S sin e, e = min(90°, e₀ +
//! D), the point star's highest elevation: 0, then `irradiance_nodes` fractions even in their
//! logarithm from `irradiance_min` to `irradiance_max`. The shadow falls steeply with D over w
//! (by about three times every 0.2 near the pole), so it needs fine nodes, while its cold shares
//! change slowly and are the costly part. At fixed ratios the irradiance over S sin e scales as w²
//! (a spherical bowl's f (1 − f) ≈ 4γ²), so a read takes each slope node's shares at the fraction
//! scaled by that node's (`w_n` ÷ w)². The table also holds the instantaneous shadow, the share of
//! the ground and azimuths whose horizon is at least the sun's elevation e, over tan e ÷ w on
//! [0, `instant_range`], which depends on that ratio alone.
//!
//! # The estimate
//!
//! Every share is a ratio of counts: the permanent and instantaneous shadows over every pixel of
//! every surface, and the cold shares over at most `samples` shadowed pixels a surface and node,
//! every k-th of them in raster order. Counts are exact, so the shares are the same however they
//! are summed. The trace is cut into chunks of `chunk` rows, and the slopes and colatitudes into
//! chunks of `chunk` nodes, each a pure function of its index, mapped by
//! [`map_reduce_chunks`] and reduced in index order; every
//! transcendental goes through [`hyperion_sim::math`]. So the table is the same for any thread
//! count, any chunk size and on every platform. Shares are rounded to `digits` significant
//! figures.

use std::f64::consts::{FRAC_PI_2, PI, SQRT_2};
use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::math;
use hyperion_sim::units::{Kelvin, Radians, WattsPerSquareMetre};
use hyperion_surface::cold_trap::rough_plains::irradiance_limit;
use hyperion_surface::cold_trap::{Body, BodyParts};

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TableCrate, TaskClass, TaskOutput};
use crate::tasks::displaced_forms::births::Draws;

/// The task's revision.
pub const VERSION: u32 = 0;

/// Mixed into the manifest's seed by exclusive or, so that the surfaces' phases are their own and
/// not those of another task that uses the same generator: `roughpln` in ASCII.
pub const ROUGH_PLAINS_STREAM: u64 = 0x726f_7567_6870_6c6e;

/// The least slope node, w = tan `σ_s` = 0.05 (`σ_s` 2.9°): the slice a smoother plain reads at its
/// own ratios.
pub const SLOPE_MIN: f64 = 0.05;

/// The greatest slope node, w = 1 (`σ_s` 45°), `Terrain`'s greatest plains slope.
pub const SLOPE_MAX: f64 = 1.0;

/// The slopes, w, at which a run measures its figures against `science-r09-drainage.md` §8.13's
/// ray traces: `σ_s` 5.7°, 11.3°, 16.7° and 26.6°.
pub const CHECK_SLOPES: [f64; 4] = [0.1, 0.2, 0.3, 0.5];

/// The grid's parameters, from the manifest's `[params]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    /// The seed of the surfaces' phases.
    pub seed: u64,
    /// The independent surfaces traced.
    pub surfaces: u32,
    /// Pixels along each side of a surface, a power of two.
    pub grid: u32,
    /// The least wavenumber kept, cycles across the domain.
    pub wavenumber_low: f64,
    /// The greatest wavenumber kept, cycles across the domain, below half the grid.
    pub wavenumber_high: f64,
    /// The Hurst exponent H: the modes' amplitudes fall as |k|^−(1+H).
    pub hurst: f64,
    /// The horizons' azimuths, evenly spaced from north.
    pub azimuths: u32,
    /// Every this-many-th azimuth is a view azimuth of the irradiance.
    pub view_stride: u32,
    /// Every this-many-th azimuth is a position of the star on its daily path.
    pub sun_stride: u32,
    /// The rays' step, pixels.
    pub step: f64,
    /// The rays' greatest length, pixels.
    pub reach: f64,
    /// The slope nodes, w = tan `σ_s`, ascending from [`SLOPE_MIN`] to [`SLOPE_MAX`].
    pub slopes: Vec<f64>,
    /// The greatest colatitude over w.
    pub colatitude_range: f64,
    /// Nodes in the colatitude over w, even on [0, `colatitude_range`].
    pub colatitude_nodes: usize,
    /// The greatest D = δ + α over w.
    pub limb_range: f64,
    /// Nodes in D over w, even on [0, `limb_range`].
    pub limb_nodes: usize,
    /// Every this-many-th colatitude node is a node of the cold shares.
    pub cold_colatitude_stride: usize,
    /// Every this-many-th limb node is a node of the cold shares.
    pub cold_limb_stride: usize,
    /// The least positive irradiance fraction of S sin e.
    pub irradiance_min: f64,
    /// The greatest irradiance fraction of S sin e.
    pub irradiance_max: f64,
    /// Positive irradiance fractions, even in their logarithm; the table also holds 0.
    pub irradiance_nodes: usize,
    /// The greatest tan e ÷ w of the instantaneous shadow.
    pub instant_range: f64,
    /// Nodes in tan e ÷ w, even on [0, `instant_range`].
    pub instant_nodes: usize,
    /// The most shadowed pixels a surface and node whose irradiance is traced.
    pub samples: u64,
    /// Significant figures the shares are rounded to.
    pub digits: i32,
    /// Rows of the trace, and slope-and-colatitude nodes of the shadows, a chunk of the map.
    pub chunk: u64,
}

/// The largest count a manifest may give.
const MOST: u64 = 100_000;

impl Params {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] for a parameter that is missing or out of range:
    /// - the grid a power of two from 16 to 4,096, and 1 to 1,000 surfaces;
    /// - 0 < `wavenumber_low` < `wavenumber_high` < half the grid with a whole number between
    ///   them, and H in (0, 1];
    /// - 8 to 3,600 azimuths, which `view_stride` and `sun_stride` divide;
    /// - 0 < `step` ≤ `reach` ≤ the grid, and `reach` at most 10⁶ steps;
    /// - at least two ascending slopes from 0.05 to 1;
    /// - positive ranges, 2 to 100,000 nodes on each axis, cold strides that divide their axis's
    ///   nodes less one, and 0 < `irradiance_min` < `irradiance_max`;
    /// - 1 to 10¹² samples, 1 to 12 digits and 1 to 100,000 a chunk.
    pub fn of(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let grid = small(manifest, "grid", 16, 4096)?;
        if !grid.is_power_of_two() {
            return Err(ManifestParamError::new("grid", "a power of two"));
        }
        let half = f64::from(grid / 2);
        let (wavenumber_low, wavenumber_high) = (
            positive(manifest, "wavenumber_low")?,
            positive(manifest, "wavenumber_high")?,
        );
        // ⌈low⌉ ≤ high keeps at least the mode (⌈low⌉, 0), so that a surface is never flat.
        if !(wavenumber_low < wavenumber_high
            && wavenumber_high < half
            && wavenumber_low.ceil() <= wavenumber_high)
        {
            return Err(ManifestParamError::new(
                "wavenumber_low and wavenumber_high",
                "0 < low < high < half the grid, with a whole number between them",
            ));
        }
        let hurst = positive(manifest, "hurst")?;
        if hurst > 1.0 {
            return Err(ManifestParamError::new("hurst", "in (0, 1]"));
        }
        let azimuths = small(manifest, "azimuths", 8, 3600)?;
        let view_stride = small(manifest, "view_stride", 1, 3600)?;
        let sun_stride = small(manifest, "sun_stride", 1, 3600)?;
        if !(azimuths.is_multiple_of(view_stride) && azimuths.is_multiple_of(sun_stride)) {
            return Err(ManifestParamError::new(
                "view_stride and sun_stride",
                "divisors of azimuths",
            ));
        }
        let (step, reach) = (positive(manifest, "step")?, positive(manifest, "reach")?);
        if !(step <= reach && reach <= f64::from(grid) && reach <= 1e6 * step) {
            return Err(ManifestParamError::new(
                "step and reach",
                "0 < step ≤ reach ≤ the grid, and at most 10⁶ steps to the reach",
            ));
        }
        let (irradiance_min, irradiance_max) = (
            positive(manifest, "irradiance_min")?,
            positive(manifest, "irradiance_max")?,
        );
        if irradiance_min >= irradiance_max {
            return Err(ManifestParamError::new(
                "irradiance_min and irradiance_max",
                "0 < min < max",
            ));
        }
        let colatitude_nodes = nodes(manifest, "colatitude_nodes")?;
        let limb_nodes = nodes(manifest, "limb_nodes")?;
        Ok(Self {
            seed: manifest.u64("seed")?,
            surfaces: small(manifest, "surfaces", 1, 1000)?,
            grid,
            wavenumber_low,
            wavenumber_high,
            hurst,
            azimuths,
            view_stride,
            sun_stride,
            step,
            reach,
            slopes: slopes(manifest)?,
            colatitude_range: positive(manifest, "colatitude_range")?,
            colatitude_nodes,
            limb_range: positive(manifest, "limb_range")?,
            limb_nodes,
            cold_colatitude_stride: stride(manifest, "cold_colatitude_stride", colatitude_nodes)?,
            cold_limb_stride: stride(manifest, "cold_limb_stride", limb_nodes)?,
            irradiance_min,
            irradiance_max,
            irradiance_nodes: nodes(manifest, "irradiance_nodes")?,
            instant_range: positive(manifest, "instant_range")?,
            instant_nodes: nodes(manifest, "instant_nodes")?,
            samples: count(manifest, "samples", 1, 1_000_000_000_000)?,
            digits: i32::try_from(small(manifest, "digits", 1, 12)?)
                .map_err(|_| ManifestParamError::new("digits", "from 1 to 12"))?,
            chunk: count(manifest, "chunk", 1, MOST)?,
        })
    }

    /// Pixels along a side, as an index.
    #[must_use]
    fn side(&self) -> usize {
        index(self.grid)
    }
}

/// The integer parameter `key` of `manifest`, from `least` to `most`.
fn count(manifest: &Manifest, key: &str, least: u64, most: u64) -> Result<u64, ManifestParamError> {
    let n = manifest.u64(key)?;
    if n < least || n > most {
        return Err(ManifestParamError::new(
            key,
            &format!("an integer from {least} to {most}"),
        ));
    }
    Ok(n)
}

/// The integer parameter `key` of `manifest`, from `least` to `most`, as a `u32`.
fn small(manifest: &Manifest, key: &str, least: u64, most: u64) -> Result<u32, ManifestParamError> {
    u32::try_from(count(manifest, key, least, most)?)
        .map_err(|_| ManifestParamError::new(key, "a u32"))
}

/// The node count `key` of `manifest`, from 2 to 100,000.
fn nodes(manifest: &Manifest, key: &str) -> Result<usize, ManifestParamError> {
    usize::try_from(count(manifest, key, 2, MOST)?)
        .map_err(|_| ManifestParamError::new(key, "a usize"))
}

/// The finite, positive parameter `key` of `manifest`.
fn positive(manifest: &Manifest, key: &str) -> Result<f64, ManifestParamError> {
    let v = manifest.f64(key)?;
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(ManifestParamError::new(key, "finite and positive"))
    }
}

/// The cold stride `key` of `manifest`, which divides `nodes` − 1.
fn stride(manifest: &Manifest, key: &str, nodes: usize) -> Result<usize, ManifestParamError> {
    let stride = usize::try_from(count(manifest, key, 1, MOST)?)
        .map_err(|_| ManifestParamError::new(key, "a usize"))?;
    if (nodes - 1).is_multiple_of(stride) {
        Ok(stride)
    } else {
        Err(ManifestParamError::new(
            key,
            "a divisor of its axis's nodes less one",
        ))
    }
}

/// The slopes of `manifest`: at least two, ascending, from [`SLOPE_MIN`] to [`SLOPE_MAX`].
fn slopes(manifest: &Manifest) -> Result<Vec<f64>, ManifestParamError> {
    let slopes = floats(manifest, "slopes")?;
    let ascending = slopes.windows(2).all(|w| w[0] < w[1]);
    let ends = slopes
        .first()
        .is_some_and(|&s| (s - SLOPE_MIN).abs() < 1e-12)
        && slopes
            .last()
            .is_some_and(|&s| (s - SLOPE_MAX).abs() < 1e-12);
    if slopes.len() >= 2 && ascending && ends {
        Ok(slopes)
    } else {
        Err(ManifestParamError::new(
            "slopes",
            "at least two ascending slopes from 0.05 to 1",
        ))
    }
}

/// A float list parameter of the manifest.
fn floats(manifest: &Manifest, key: &str) -> Result<Vec<f64>, ManifestParamError> {
    manifest
        .params()
        .get(key)
        .and_then(toml::Value::as_array)
        .and_then(|values| {
            values
                .iter()
                .map(|v| {
                    v.as_float().or_else(|| {
                        v.as_integer()
                            .and_then(|i| i32::try_from(i).ok())
                            .map(f64::from)
                    })
                })
                .collect::<Option<Vec<f64>>>()
        })
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ManifestParamError::new(key, "a non-empty list of numbers"))
}

/// `n` as an index.
#[must_use]
fn index(n: u32) -> usize {
    usize::try_from(n).expect("a u32 fits a usize on every target the fit runs on")
}

/// A size or index as a `u64`.
#[must_use]
fn wide(n: usize) -> u64 {
    u64::try_from(n).expect("a usize fits a u64 on every target the fit runs on")
}

/// A count as an `f64`.
#[must_use]
fn real(n: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "the counts are of pixels and azimuths, below 2⁵³ and exact in an f64"
    )]
    let x = n as f64;
    x
}

/// The cell of `x` along a periodic side of `n` pixels, for an `x` within a few periods of 0.
#[must_use]
fn wrap(x: f64, n: usize) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "x is a whole number below 2³¹ in magnitude (a ray is shorter than the grid)"
    )]
    let i = x as i64;
    let n = i64::try_from(n).expect("a grid of at most 4,096 pixels fits an i64");
    usize::try_from(i.rem_euclid(n)).expect("a remainder of a positive modulus is not negative")
}

/// The unnormalised inverse discrete Fourier transform of `re` + i `im`, a power of two long, in
/// place: `x_j = Σ_k X_k e^(2πi jk ÷ n)`, by the iterative radix-2 algorithm in its fixed order.
/// `twiddles` holds e^(2πi k ÷ n) for k < n ÷ 2.
fn inverse_fft(re: &mut [f64], im: &mut [f64], twiddles: &[(f64, f64)]) {
    let size = re.len();
    assert!(
        size.is_power_of_two() && im.len() == size && twiddles.len() == size / 2,
        "the transform needs a power of two and its twiddles"
    );
    let bits = size.trailing_zeros();
    for i in 0..size {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if j > i {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= size {
        let half = len / 2;
        let stride = size / len;
        for start in (0..size).step_by(len) {
            for k in 0..half {
                let (cos, sin) = twiddles[k * stride];
                let (low, high) = (start + k, start + k + half);
                let vr = re[high] * cos - im[high] * sin;
                let vi = re[high] * sin + im[high] * cos;
                re[high] = re[low] - vr;
                im[high] = im[low] - vi;
                re[low] += vr;
                im[low] += vi;
            }
        }
        len *= 2;
    }
}

/// A periodic height field on a square grid of unit spacing, x east and y north, heights in
/// pixels, row-major (index y × side + x).
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    side: usize,
    heights: Vec<f64>,
    highest: f64,
}

impl Surface {
    /// The surface of `heights`, `side` × `side`, row-major.
    ///
    /// # Panics
    ///
    /// If `heights` is not `side` × `side` long, `side` is below 3, or a height is not finite.
    #[must_use]
    pub fn new(side: usize, heights: Vec<f64>) -> Self {
        assert!(
            side >= 3 && heights.len() == side * side,
            "a surface needs side² heights"
        );
        assert!(
            heights.iter().all(|z| z.is_finite()),
            "a surface's heights are finite"
        );
        let highest = heights.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        Self {
            side,
            heights,
            highest,
        }
    }

    /// Surface `number` of `params`: its Fourier modes, then its heights scaled to a directional
    /// RMS slope of 1 over its facets (the module documentation gives the construction).
    ///
    /// # Panics
    ///
    /// Never for parameters that [`Params::of`] accepts: a grid of 16 to 4,096 pixels, a power of
    /// two, with wavenumbers below half of it.
    #[must_use]
    pub fn gaussian(params: &Params, number: u32) -> Self {
        let n = params.side();
        let half = i64::from(params.grid / 2);
        let modulus = i64::from(params.grid);
        let (mut re, mut im) = (vec![0.0; n * n], vec![0.0; n * n]);
        let key = params.seed ^ ROUGH_PLAINS_STREAM;
        for ky in 0..half {
            for kx in (1 - half)..half {
                if ky == 0 && kx <= 0 {
                    // The other half plane is each mode's conjugate, which the real part takes.
                    continue;
                }
                let squared = kx * kx + ky * ky;
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a squared wavenumber below 2²⁴ is exact in an f64"
                )]
                let k = (squared as f64).sqrt();
                if k < params.wavenumber_low || k > params.wavenumber_high {
                    continue;
                }
                let column =
                    u64::try_from(kx.rem_euclid(modulus)).expect("a remainder is not negative");
                let row = u64::try_from(ky).expect("ky is not negative");
                // Keyed by the signed wavenumber, so that a mode's phase is the same on every grid
                // that holds it: kx + 2³¹ in the low word.
                let signed = u64::try_from(kx + (1 << 31)).expect("|kx| is below 2³¹");
                let mut draws = Draws::new(key, u64::from(number), (row << 32) | signed);
                let (s, c) = math::sin_cos(2.0 * PI * draws.uniform());
                let amplitude = math::powf(k, -(1.0 + params.hurst));
                let at = usize::try_from(row).expect("a row fits a usize") * n
                    + usize::try_from(column).expect("a column fits a usize");
                re[at] = amplitude * c;
                im[at] = amplitude * s;
            }
        }
        let twiddles: Vec<(f64, f64)> = (0..n / 2)
            .map(|k| {
                let (s, c) = math::sin_cos(2.0 * PI * real(wide(k)) / real(wide(n)));
                (c, s)
            })
            .collect();
        for row in 0..n {
            inverse_fft(
                &mut re[row * n..(row + 1) * n],
                &mut im[row * n..(row + 1) * n],
                &twiddles,
            );
        }
        let (mut column_re, mut column_im) = (vec![0.0; n], vec![0.0; n]);
        for x in 0..n {
            for y in 0..n {
                column_re[y] = re[y * n + x];
                column_im[y] = im[y * n + x];
            }
            inverse_fft(&mut column_re, &mut column_im, &twiddles);
            for y in 0..n {
                re[y * n + x] = column_re[y];
            }
        }
        let rough = Self::new(n, re);
        let scale = 1.0 / rough.slope_rms();
        Self::new(n, rough.heights.iter().map(|z| z * scale).collect())
    }

    /// Pixels along a side.
    #[must_use]
    pub fn side(&self) -> usize {
        self.side
    }

    /// The height at pixel (`x`, `y`).
    #[must_use]
    pub fn at(&self, x: usize, y: usize) -> f64 {
        self.heights[y * self.side + x]
    }

    /// The directional RMS slope over the facets: the root mean square of the forward differences
    /// along both axes, summed in row-major order.
    #[must_use]
    pub fn slope_rms(&self) -> f64 {
        let n = self.side;
        let mut sum = 0.0;
        for y in 0..n {
            for x in 0..n {
                let z = self.at(x, y);
                let east = self.at((x + 1) % n, y) - z;
                let north = self.at(x, (y + 1) % n) - z;
                sum += east * east + north * north;
            }
        }
        (sum / (2.0 * real(wide(n * n)))).sqrt()
    }

    /// The height of the bilinear surface at (`x`, `y`), periodic.
    #[must_use]
    fn bilinear(&self, x: f64, y: f64) -> f64 {
        let (fx, fy) = (x.floor(), y.floor());
        let (ax, ay) = (x - fx, y - fy);
        let (i, j) = (wrap(fx, self.side), wrap(fy, self.side));
        let (i1, j1) = ((i + 1) % self.side, (j + 1) % self.side);
        let south = self.at(i, j) + ax * (self.at(i1, j) - self.at(i, j));
        let north = self.at(i, j1) + ax * (self.at(i1, j1) - self.at(i, j1));
        south + ay * (north - south)
    }

    /// The pixel nearest (`x`, `y`), as a row-major index.
    #[must_use]
    fn nearest(&self, x: f64, y: f64) -> u32 {
        let at = wrap(y.round(), self.side) * self.side + wrap(x.round(), self.side);
        u32::try_from(at).expect("a grid of at most 4,096² pixels indexes in a u32")
    }

    /// The gradient (∂z/∂x, ∂z/∂y) at pixel `at`, by central differences.
    #[must_use]
    fn gradient(&self, at: usize) -> (f64, f64) {
        let n = self.side;
        let (x, y) = (at % n, at / n);
        let east = self.at((x + 1) % n, y) - self.at((x + n - 1) % n, y);
        let north = self.at(x, (y + 1) % n) - self.at(x, (y + n - 1) % n);
        (0.5 * east, 0.5 * north)
    }
}

/// The azimuths' directions, (sin φ, cos φ) at φ = 2πk ÷ `azimuths` from north toward east.
#[must_use]
fn directions(azimuths: u32) -> Vec<(f64, f64)> {
    (0..azimuths)
        .map(|k| math::sin_cos(2.0 * PI * f64::from(k) / f64::from(azimuths)))
        .collect()
}

/// One surface's horizons at unit slope: tan h at every pixel and azimuth (the module
/// documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct Horizons {
    azimuths: usize,
    /// Pixel-major: pixel × `azimuths` + azimuth.
    tangents: Vec<f64>,
}

impl Horizons {
    /// The horizons of `surface` traced as `params` gives, on `threads` threads.
    ///
    /// # Errors
    ///
    /// [`BuildThreadPoolError`] if the thread pool cannot be built.
    ///
    /// # Panics
    ///
    /// Never on a target whose `usize` fits a `u64`: the rows' indices are converted.
    pub fn trace(
        surface: &Surface,
        params: &Params,
        threads: NonZeroUsize,
    ) -> Result<Self, BuildThreadPoolError> {
        let n = surface.side;
        let dirs = directions(params.azimuths);
        let azimuths = dirs.len();
        let mut tangents = Vec::with_capacity(n * n * azimuths);
        map_reduce_chunks(
            wide(n),
            params.chunk,
            threads,
            |rows| {
                let rows_len =
                    usize::try_from(rows.end - rows.start).expect("a chunk fits a usize");
                let mut part = Vec::with_capacity(rows_len * n * dirs.len());
                for y in rows {
                    let y = usize::try_from(y).expect("a row fits a usize");
                    for x in 0..n {
                        for &dir in &dirs {
                            part.push(march(surface, (x, y), dir, params, |_, _| {}));
                        }
                    }
                }
                part
            },
            |part| tangents.extend(part),
        )?;
        Ok(Self { azimuths, tangents })
    }

    /// tan h at unit slope at pixel `at` and azimuth `k`.
    #[must_use]
    fn tangent(&self, at: usize, k: usize) -> f64 {
        self.tangents[at * self.azimuths + k]
    }
}

/// The steepest tangent of the terrain from pixel `from` toward `dir`, at unit slope, by a march in
/// steps of `params.step` to `params.reach`. Each step that raises it is a piece of the terrain the
/// pixel sees, handed to `piece` with its nearest pixel and its tangent, nearest first.
fn march(
    surface: &Surface,
    from: (usize, usize),
    dir: (f64, f64),
    params: &Params,
    mut piece: impl FnMut(u32, f64),
) -> f64 {
    let z0 = surface.at(from.0, from.1);
    let rise = surface.highest - z0;
    let (x0, y0) = (real(wide(from.0)), real(wide(from.1)));
    let mut steepest = f64::NEG_INFINITY;
    let mut steps = 1_u32;
    loop {
        let distance = params.step * f64::from(steps);
        // Past `reach`, or where even the highest point could not rise above the steepest so far.
        if distance > params.reach || rise <= steepest * distance {
            break;
        }
        let (x, y) = (x0 + distance * dir.0, y0 + distance * dir.1);
        let tangent = (surface.bilinear(x, y) - z0) / distance;
        if tangent > steepest {
            steepest = tangent;
            piece(surface.nearest(x, y), tangent);
        }
        steps += 1;
    }
    steepest
}

/// The view factor's integrand's antiderivative, ∫ (A cos a + B sin a) cos a da =
/// A (a + sin a cos a) ÷ 2 + B sin² a ÷ 2, at `a`.
#[must_use]
fn view_integral(a: f64, along: f64, up: f64) -> f64 {
    let (s, c) = math::sin_cos(a);
    0.5 * along * (a + s * c) + 0.5 * up * s * s
}

/// The unit normal of a facet of unit-slope gradient `gradient` on a plain of slope `w`.
#[must_use]
fn normal(gradient: (f64, f64), w: f64) -> [f64; 3] {
    let (gx, gy) = (w * gradient.0, w * gradient.1);
    let norm = (1.0 + gx * gx + gy * gy).sqrt();
    [-gx / norm, -gy / norm, 1.0 / norm]
}

/// The pieces a shadowed pixel sees, with their view factors, at one slope.
#[derive(Debug, Clone, PartialEq, Default)]
struct Seen {
    pixels: Vec<u32>,
    factors: Vec<f64>,
    /// The pieces the marches found, kept or not, for the acceptance line.
    pieces: u64,
}

/// What pixel `at` of `surface` sees at slope `w` along every `params.view_stride`-th azimuth:
/// each piece with its view factor (Δφ ÷ π) ∫ max(0, n · d) cos a da over its elevations, n the
/// pixel's normal (the module documentation). A piece whose factor is not positive is dropped.
#[must_use]
fn seen(surface: &Surface, dirs: &[(f64, f64)], at: usize, w: f64, params: &Params) -> Seen {
    let n = normal(surface.gradient(at), w);
    let stride = index(params.view_stride);
    // Δφ ÷ π, Δφ = 2π ÷ the view azimuths.
    let width = 2.0 * real(u64::from(params.view_stride)) / real(wide(dirs.len()));
    let from = (at % surface.side, at / surface.side);
    let mut out = Seen::default();
    for &(sin_phi, cos_phi) in dirs.iter().step_by(stride) {
        let along = n[0] * sin_phi + n[1] * cos_phi;
        let up = n[2];
        // n · d = along cos a + up sin a is positive above this elevation.
        let floor = math::atan(-along / up);
        let mut below = -FRAC_PI_2;
        march(surface, from, (sin_phi, cos_phi), params, |pixel, t| {
            out.pieces += 1;
            let top = math::atan(w * t);
            let bottom = below.max(floor);
            if top > bottom {
                let factor =
                    width * (view_integral(top, along, up) - view_integral(bottom, along, up));
                if factor > 0.0 {
                    out.pixels.push(pixel);
                    out.factors.push(factor);
                }
            }
            below = top;
        });
    }
    out
}

/// One surface's counts at one slope and colatitude: the pixels in permanent shadow at each limb
/// node, and at each cold limb node of a cold colatitude node the shadowed pixels whose irradiance
/// was traced, and of those the pixels whose peak irradiance is at most each irradiance node (limb
/// major).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct NodeCounts {
    shadow: Vec<u64>,
    sampled: Vec<u64>,
    cold: Vec<u64>,
    /// The pieces the sampled pixels' marches found, for the acceptance line.
    pieces: u64,
    /// The sampled pixels' view azimuths, for the acceptance line.
    views: u64,
}

/// The axes of a fit.
#[derive(Debug, Clone, PartialEq)]
pub struct Axes {
    /// The slope nodes, w.
    pub slopes: Vec<f64>,
    /// The colatitude over w at the nodes.
    pub colatitudes: Vec<f64>,
    /// D = δ + α over w at the nodes.
    pub limbs: Vec<f64>,
    /// The colatitude over w at the cold shares' nodes: every `cold_colatitude_stride`-th.
    pub cold_colatitudes: Vec<f64>,
    /// D over w at the cold shares' nodes: every `cold_limb_stride`-th.
    pub cold_limbs: Vec<f64>,
    /// The irradiance nodes, fractions of S sin e: 0, then even in their logarithm.
    pub irradiances: Vec<f64>,
    /// tan e ÷ w at the instantaneous shadow's nodes.
    pub instants: Vec<f64>,
}

impl Axes {
    /// The axes of `params`.
    #[must_use]
    pub fn of(params: &Params) -> Self {
        let mut irradiances = vec![0.0];
        let (lo, hi) = (
            math::ln(params.irradiance_min),
            math::ln(params.irradiance_max),
        );
        irradiances.extend(
            even_nodes(lo, hi, params.irradiance_nodes)
                .into_iter()
                .enumerate()
                .map(|(i, x)| {
                    if i == 0 {
                        params.irradiance_min
                    } else if i + 1 == params.irradiance_nodes {
                        params.irradiance_max
                    } else {
                        math::exp(x)
                    }
                }),
        );
        let colatitudes = even_nodes(0.0, params.colatitude_range, params.colatitude_nodes);
        let limbs = even_nodes(0.0, params.limb_range, params.limb_nodes);
        Self {
            slopes: params.slopes.clone(),
            cold_colatitudes: colatitudes
                .iter()
                .copied()
                .step_by(params.cold_colatitude_stride)
                .collect(),
            cold_limbs: limbs
                .iter()
                .copied()
                .step_by(params.cold_limb_stride)
                .collect(),
            colatitudes,
            limbs,
            irradiances,
            instants: even_nodes(0.0, params.instant_range, params.instant_nodes),
        }
    }
}

/// `n` ≥ 2 even nodes from `lo` to `hi`, both included exactly.
#[must_use]
fn even_nodes(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let last = u32::try_from(n - 1).expect("a manifest's node count is at most 100,000");
    (0..=last)
        .map(|i| {
            if i == last {
                hi
            } else {
                lo + (hi - lo) * f64::from(i) / f64::from(last)
            }
        })
        .collect()
}

/// The sun's direction at azimuth (sin φ, cos φ) on its daily path at declination `limb` over a
/// latitude of sine `sin_lat` and cosine `cos_lat`: the elevation a with
/// sin `limb` = sin lat sin a + cos lat cos a cos φ, on the zenith's side, and (east, north, up).
#[must_use]
fn sun_at(dir: (f64, f64), sin_lat: f64, cos_lat: f64, sin_limb: f64) -> [f64; 3] {
    let (sin_phi, cos_phi) = dir;
    let across = cos_lat * cos_phi;
    let radius = (sin_lat * sin_lat + across * across).sqrt();
    let a = math::asin(sin_limb / radius) - math::atan2(across, sin_lat);
    let (s, c) = math::sin_cos(a);
    [c * sin_phi, c * cos_phi, s]
}

/// Whether a slope-and-colatitude node counts its cold shares too, or its shadow alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nodes {
    /// A colatitude node between the cold nodes: the shadow at every limb node.
    ShadowOnly,
    /// A cold colatitude node: the shadow at every limb node, and the cold shares at every cold
    /// limb node.
    WithCold,
}

/// What every node of one surface at one slope shares: the surface, its horizons, the azimuths'
/// directions and the facets' normals at the slope `w`.
#[derive(Debug, Clone, Copy)]
struct SlopeView<'a> {
    surface: &'a Surface,
    horizons: &'a Horizons,
    dirs: &'a [(f64, f64)],
    normals: &'a [[f64; 3]],
    w: f64,
}

/// The counts of `view` at `colatitude`, at every limb node, and for [`Nodes::WithCold`] at every
/// cold limb node.
#[must_use]
fn node_counts(
    view: &SlopeView<'_>,
    params: &Params,
    axes: &Axes,
    colatitude: f64,
    kind: Nodes,
) -> NodeCounts {
    let (w, limbs, cold_limbs) = (view.w, axes.limbs.len(), axes.cold_limbs.len());
    let nodes = axes.irradiances.len();
    let mut counts = NodeCounts {
        shadow: vec![0; limbs],
        sampled: vec![0; cold_limbs],
        cold: vec![0; cold_limbs * nodes],
        pieces: 0,
        views: 0,
    };
    if colatitude >= FRAC_PI_2 {
        return counts;
    }
    let (sin_lat, cos_lat) = math::sin_cos(FRAC_PI_2 - colatitude);
    let least = least_declinations(view.horizons, view.dirs, w, (sin_lat, cos_lat));
    let mut shadowed: Vec<usize> = Vec::with_capacity(least.len());
    for (l, &ratio) in axes.limbs.iter().enumerate() {
        let limb = ratio * w;
        if colatitude + limb >= FRAC_PI_2 {
            continue;
        }
        let sin_limb = math::sin(limb);
        let cold = kind == Nodes::WithCold && l.is_multiple_of(params.cold_limb_stride);
        if !cold {
            counts.shadow[l] = wide(least.iter().filter(|&&low| low > sin_limb).count());
            continue;
        }
        shadowed.clear();
        shadowed.extend((0..least.len()).filter(|&at| least[at] > sin_limb));
        counts.shadow[l] = wide(shadowed.len());
        if shadowed.is_empty() {
            continue;
        }
        let l = l / params.cold_limb_stride;
        // Every k-th shadowed pixel in raster order, at most `samples` of them.
        let stride = usize::try_from(wide(shadowed.len()).div_ceil(params.samples))
            .expect("a stride below the pixels fits a usize");
        let seens: Vec<Seen> = shadowed
            .iter()
            .step_by(stride)
            .map(|&at| seen(view.surface, view.dirs, at, w, params))
            .collect();
        counts.sampled[l] = wide(seens.len());
        counts.pieces += seens.iter().map(|s| s.pieces).sum::<u64>();
        counts.views += wide(seens.len()) * u64::from(params.azimuths / params.view_stride);
        let path = Path {
            sin_lat,
            cos_lat,
            sin_limb,
        };
        let peaks = peak_irradiances(view, params, &seens, path);
        let sin_e = math::sin(colatitude + limb);
        let row = &mut counts.cold[l * nodes..(l + 1) * nodes];
        for peak in peaks {
            let x = if sin_e > 0.0 {
                peak / sin_e
            } else if peak > 0.0 {
                f64::INFINITY
            } else {
                0.0
            };
            for (cold, &node) in row.iter_mut().zip(&axes.irradiances) {
                if x <= node {
                    *cold += 1;
                }
            }
        }
    }
    counts
}

/// Each pixel's least sine of its horizons' declinations at slope `w`, over a latitude of sine and
/// cosine `(sin_lat, cos_lat)`: sin dec = sin lat sin h + cos lat cos h cos φ, from tan h.
#[must_use]
fn least_declinations(
    horizons: &Horizons,
    dirs: &[(f64, f64)],
    w: f64,
    (sin_lat, cos_lat): (f64, f64),
) -> Vec<f64> {
    horizons
        .tangents
        .chunks_exact(horizons.azimuths)
        .map(|tangents| {
            let mut low = f64::INFINITY;
            for (&t1, &(_, cos_phi)) in tangents.iter().zip(dirs) {
                let t = w * t1;
                low = low.min((sin_lat * t + cos_lat * cos_phi) / (1.0 + t * t).sqrt());
            }
            low
        })
        .collect()
}

/// A point star's daily path at one latitude: the latitude's sine and cosine and the star's
/// declination's sine.
#[derive(Debug, Clone, Copy)]
struct Path {
    sin_lat: f64,
    cos_lat: f64,
    sin_limb: f64,
}

/// The peak first-order irradiance over the day, a share of S, of each pixel whose view is in
/// `seens`, under a point star on `path`, sampled at every `sun_stride`-th azimuth: at each, the
/// facets the star stands above are lit with their direct flux μ, and each view sums its pieces'
/// fluxes times their view factors in their fixed order.
#[must_use]
fn peak_irradiances(view: &SlopeView<'_>, params: &Params, seens: &[Seen], path: Path) -> Vec<f64> {
    let (w, normals, horizons) = (view.w, view.normals, view.horizons);
    let mut peaks = vec![0.0_f64; seens.len()];
    let mut flux = vec![0.0; normals.len()];
    for k in (0..view.dirs.len()).step_by(index(params.sun_stride)) {
        let sun = sun_at(view.dirs[k], path.sin_lat, path.cos_lat, path.sin_limb);
        let tan_sun = sun[2] / (sun[0] * sun[0] + sun[1] * sun[1]).sqrt();
        for (at, f) in flux.iter_mut().enumerate() {
            *f = if tan_sun > w * horizons.tangent(at, k) {
                let n = normals[at];
                (n[0] * sun[0] + n[1] * sun[1] + n[2] * sun[2]).max(0.0)
            } else {
                0.0
            };
        }
        for (peak, seen) in peaks.iter_mut().zip(seens) {
            let mut e = 0.0;
            for (&pixel, &factor) in seen.pixels.iter().zip(&seen.factors) {
                e += factor * flux[index(pixel)];
            }
            *peak = peak.max(e);
        }
    }
    peaks
}

/// `value` rounded to `digits` significant figures; zero stays zero.
#[must_use]
fn round_significant(value: f64, digits: i32) -> f64 {
    if value == 0.0 {
        return 0.0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the decimal exponent of a finite share is within ±400"
    )]
    let exponent = math::log10(value.abs()).floor() as i32;
    let scale = math::powi(10.0, digits - 1 - exponent);
    (value * scale).round() / scale
}

/// The cell of the ascending `nodes` that holds `x`, and `x`'s place in it in [0, 1], clamped to
/// the end cells.
#[must_use]
fn cell(nodes: &[f64], x: f64) -> (usize, f64) {
    let last = nodes.len() - 1;
    if x <= nodes[0] {
        return (0, 0.0);
    }
    if x >= nodes[last] {
        return (last - 1, 1.0);
    }
    let i = nodes.partition_point(|&node| node <= x) - 1;
    (i, (x - nodes[i]) / (nodes[i + 1] - nodes[i]))
}

/// One cold node's share of its shadow at the irradiance fraction `x` > 0, from its `shares` at
/// the irradiance `nodes` (0, then even in their logarithm): linear in x from 0 to the first
/// positive node; between positive nodes linear in ln x and ln share where both shares are
/// positive (a tail that falls as a power of x), else linear in ln x; and the last node's share
/// from the last node on.
#[must_use]
fn share_at(nodes: &[f64], shares: &[f64], x: f64) -> f64 {
    let last = nodes.len() - 1;
    if x >= nodes[last] {
        return shares[last];
    }
    if x < nodes[1] {
        return shares[0] + (shares[1] - shares[0]) * (x / nodes[1]);
    }
    let i = nodes.partition_point(|&node| node <= x) - 1;
    let at = math::ln(x / nodes[i]) / math::ln(nodes[i + 1] / nodes[i]);
    let (low, high) = (shares[i], shares[i + 1]);
    if low > 0.0 && high > low {
        low * math::exp(at * math::ln(high / low))
    } else {
        low + at * (high - low)
    }
}

/// What a read gives: the permanent shadow, or the cold share at an irradiance fraction.
#[derive(Debug, Clone, Copy)]
enum Share<'a> {
    /// The permanent shadow's share of the ground.
    Shadow,
    /// The share of the ground in permanent shadow whose peak irradiance is at most `fraction`
    /// × S sin e, from the cold nodes' `shares`.
    Cold {
        /// The cold nodes' shares of their shadow.
        shares: &'a [Vec<f64>],
        /// The irradiance fraction.
        fraction: f64,
    },
}

/// A table's read, `what` at slope `w`, colatitude and D = δ + α in radians (the module
/// documentation's axes).
///
/// It is zero for a plain with no slope, for a fraction that is not positive, where colatitude +
/// D ≥ 90°, and beyond the colatitude's or D's last node. A slope below the first node reads the
/// first slice at its own ratios, and one above the last the last's. The shadow is trilinear in
/// w, the colatitude over w and D over w. The cold share is the shadow times the mean of the
/// cold nodes' shares about the point, each weighted by its trilinear weight times its node's
/// shadow, so that the cold share never exceeds the shadow. Each slope node `w_n` reads its shares
/// at the fraction times (`w_n` ÷ w)², since at fixed ratios the irradiance over S sin e scales as
/// w² (a spherical bowl's f (1 − f) ≈ 4γ²), and each share is read by [`share_at`].
#[must_use]
fn read(
    axes: &Axes,
    shadow: &[Vec<f64>],
    what: Share<'_>,
    w: f64,
    colatitude: f64,
    limb: f64,
) -> f64 {
    let not_positive = matches!(what, Share::Cold { fraction, .. } if fraction <= 0.0);
    if w <= 0.0 || not_positive || colatitude + limb >= FRAC_PI_2 {
        return 0.0;
    }
    let over = |angle: f64| if angle > 0.0 { angle / w } else { 0.0 };
    let (colatitude_over, limb_over) = (over(colatitude), over(limb));
    let (colatitude_last, limb_last) = (axes.colatitudes.len() - 1, axes.limbs.len() - 1);
    if colatitude_over >= axes.colatitudes[colatitude_last] || limb_over >= axes.limbs[limb_last] {
        return 0.0;
    }
    let (slope_cell, slope_at) = cell(&axes.slopes, w);
    let (colatitude_cell, colatitude_at) = cell(&axes.colatitudes, colatitude_over);
    let (limb_cell, limb_at) = cell(&axes.limbs, limb_over);
    let width = axes.colatitudes.len();
    let mut share = 0.0;
    for (slope_step, slope_weight) in [(0, 1.0 - slope_at), (1, slope_at)] {
        for (colatitude_step, colatitude_weight) in [(0, 1.0 - colatitude_at), (1, colatitude_at)] {
            let row =
                &shadow[(slope_cell + slope_step) * width + colatitude_cell + colatitude_step];
            for (limb_step, limb_weight) in [(0, 1.0 - limb_at), (1, limb_at)] {
                share +=
                    slope_weight * colatitude_weight * limb_weight * row[limb_cell + limb_step];
            }
        }
    }
    let Share::Cold { shares, fraction } = what else {
        return share;
    };
    if share <= 0.0 {
        return 0.0;
    }
    let (cold_width, cold_limbs) = (axes.cold_colatitudes.len(), axes.cold_limbs.len());
    let colatitude_stride = (width - 1) / (cold_width - 1);
    let limb_stride = (axes.limbs.len() - 1) / (cold_limbs - 1);
    let (cold_colatitude_cell, cold_colatitude_at) = cell(&axes.cold_colatitudes, colatitude_over);
    let (cold_limb_cell, cold_limb_at) = cell(&axes.cold_limbs, limb_over);
    let (mut sum, mut weight) = (0.0, 0.0);
    for (slope_step, slope_weight) in [(0, 1.0 - slope_at), (1, slope_at)] {
        let scale = axes.slopes[slope_cell + slope_step] / w;
        let scaled = fraction * (scale * scale);
        for (colatitude_step, colatitude_weight) in
            [(0, 1.0 - cold_colatitude_at), (1, cold_colatitude_at)]
        {
            let fine = &shadow[(slope_cell + slope_step) * width
                + (cold_colatitude_cell + colatitude_step) * colatitude_stride];
            let coarse =
                (slope_cell + slope_step) * cold_width + cold_colatitude_cell + colatitude_step;
            for (limb_step, limb_weight) in [(0, 1.0 - cold_limb_at), (1, cold_limb_at)] {
                let limb_node = cold_limb_cell + limb_step;
                let k =
                    slope_weight * colatitude_weight * limb_weight * fine[limb_node * limb_stride];
                let row = &shares[coarse * cold_limbs + limb_node];
                sum += k * share_at(&axes.irradiances, row, scaled);
                weight += k;
            }
        }
    }
    if weight > 0.0 {
        share * (sum / weight).min(1.0)
    } else {
        0.0
    }
}

/// The fitted table.
#[derive(Debug, Clone, PartialEq)]
pub struct RoughPlainsFit {
    /// The axes.
    pub axes: Axes,
    /// The permanent shadow's share of the ground: a row for each slope and colatitude (slope
    /// major), a share for each limb.
    pub shadow: Vec<Vec<f64>>,
    /// The shares of the permanent shadow whose peak irradiance is at most each irradiance node:
    /// a row for each slope, cold colatitude and cold limb (in that order of majority), a share
    /// for each node.
    pub cold: Vec<Vec<f64>>,
    /// The instantaneous shadow at the tan e ÷ w nodes.
    pub instantaneous: Vec<f64>,
    /// The parameters it was made with.
    pub params: Params,
}

impl RoughPlainsFit {
    /// The permanent shadow's share of the ground at slope `w`, colatitude `colatitude_rad` and
    /// D = δ + α `limb_rad`, as `hyperion_surface::cold_trap::rough_plains` reads the committed
    /// table.
    #[must_use]
    pub fn permanent_shadow(&self, w: f64, colatitude_rad: f64, limb_rad: f64) -> f64 {
        read(
            &self.axes,
            &self.shadow,
            Share::Shadow,
            w,
            colatitude_rad,
            limb_rad,
        )
    }

    /// The share of the ground in permanent shadow whose peak irradiance from sunlit terrain is
    /// at most `irradiance` × S sin e, e = min(90°, colatitude + D), as the reader reads it.
    #[must_use]
    pub fn cold_share(&self, w: f64, colatitude_rad: f64, limb_rad: f64, irradiance: f64) -> f64 {
        let what = Share::Cold {
            shares: &self.cold,
            fraction: irradiance,
        };
        read(&self.axes, &self.shadow, what, w, colatitude_rad, limb_rad)
    }

    /// The instantaneous shadow at slope `w` with the sun at elevation `elevation_rad`, linear in
    /// tan e ÷ w and the last node's value beyond it; zero for a plain with no slope.
    #[must_use]
    pub fn instantaneous_shadow(&self, w: f64, elevation_rad: f64) -> f64 {
        if w <= 0.0 {
            return 0.0;
        }
        let (i, at) = cell(&self.axes.instants, math::tan(elevation_rad) / w);
        (1.0 - at) * self.instantaneous[i] + at * self.instantaneous[i + 1]
    }
}

/// One surface's counts at every node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct SurfaceCounts {
    /// A [`NodeCounts`] for each slope and colatitude, slope major.
    nodes: Vec<NodeCounts>,
    /// The horizons at least each instantaneous node.
    instants: Vec<u64>,
    /// The horizons at least each of [`smith_points`]'s tan e ÷ w.
    smith: Vec<u64>,
    /// The pieces the sampled pixels' marches found.
    pieces: u64,
    /// The sampled pixels' view azimuths.
    views: u64,
}

/// The points at which a run compares its instantaneous shadow with Smith 1967's: w 0.1, 0.3 and
/// 0.5 (`σ_s` 5.7°, 16.7° and 26.6°) at sun elevations of 0.5° to 4°, every 0.5°: (w, degrees).
#[must_use]
pub fn smith_points() -> Vec<(f64, f64)> {
    let mut points = Vec::with_capacity(24);
    for w in [0.1, 0.3, 0.5] {
        for half in 1..=8_u32 {
            points.push((w, 0.5 * f64::from(half)));
        }
    }
    points
}

/// Smith's (1967, "Lunar surface roughness: shadowing and thermal emission", JGR 72, 4059)
/// shadowed share of a Gaussian surface of directional RMS slope `w`
/// under a sun at elevation `elevation_rad`: one less (1 − erfc(v) ÷ 2) ÷ (1 + Λ), with
/// v = tan e ÷ (√2 w) and Λ = (e^(−v²) ÷ (v √π) − erfc(v)) ÷ 2.
#[must_use]
pub fn smith_shadow(w: f64, elevation_rad: f64) -> f64 {
    let v = math::tan(elevation_rad) / (SQRT_2 * w);
    let lambda = 0.5 * (math::exp(-v * v) / (v * PI.sqrt()) - math::erfc(v));
    1.0 - (1.0 - 0.5 * math::erfc(v)) / (1.0 + lambda)
}

/// How many of `tangents` are at least each of the ascending `nodes`.
#[must_use]
fn at_least(tangents: &[f64], nodes: &[f64]) -> Vec<u64> {
    let mut hits = vec![0_u64; nodes.len() + 1];
    for &t in tangents {
        hits[nodes.partition_point(|&node| node <= t)] += 1;
    }
    // A tangent counts for every node at or below it.
    let mut out = vec![0_u64; nodes.len()];
    let mut running = 0;
    for i in (0..nodes.len()).rev() {
        running += hits[i + 1];
        out[i] = running;
    }
    out
}

/// Surface `number`'s counts, on `threads` threads.
fn surface_counts(
    params: &Params,
    axes: &Axes,
    number: u32,
    threads: NonZeroUsize,
) -> Result<SurfaceCounts, BuildThreadPoolError> {
    let surface = Surface::gaussian(params, number);
    let horizons = Horizons::trace(&surface, params, threads)?;
    let smith: Vec<f64> = smith_points()
        .iter()
        .map(|&(w, e)| math::tan(e * PI / 180.0) / w)
        .collect();
    let mut sorted = smith.clone();
    sorted.sort_by(f64::total_cmp);
    let sorted_counts = at_least(&horizons.tangents, &sorted);
    let mut counts = SurfaceCounts {
        nodes: Vec::with_capacity(axes.slopes.len() * axes.colatitudes.len()),
        instants: at_least(&horizons.tangents, &axes.instants),
        smith: smith
            .iter()
            .map(|t| sorted_counts[sorted.partition_point(|s| s < t)])
            .collect(),
        pieces: 0,
        views: 0,
    };
    let width = axes.colatitudes.len();
    let dirs = directions(params.azimuths);
    let pixels = surface.side * surface.side;
    let normals: Vec<Vec<[f64; 3]>> = axes
        .slopes
        .iter()
        .map(|&w| {
            (0..pixels)
                .map(|at| normal(surface.gradient(at), w))
                .collect()
        })
        .collect();
    map_reduce_chunks(
        wide(axes.slopes.len() * width),
        params.chunk,
        threads,
        |range| {
            range
                .map(|node| {
                    let node = usize::try_from(node).expect("a node fits a usize");
                    let slope = node / width;
                    let view = SlopeView {
                        surface: &surface,
                        horizons: &horizons,
                        dirs: &dirs,
                        normals: &normals[slope],
                        w: axes.slopes[slope],
                    };
                    let colatitude = axes.colatitudes[node % width] * view.w;
                    let kind = if (node % width).is_multiple_of(params.cold_colatitude_stride) {
                        Nodes::WithCold
                    } else {
                        Nodes::ShadowOnly
                    };
                    node_counts(&view, params, axes, colatitude, kind)
                })
                .collect::<Vec<NodeCounts>>()
        },
        |part| counts.nodes.extend(part),
    )?;
    counts.pieces = counts.nodes.iter().map(|c| c.pieces).sum();
    counts.views = counts.nodes.iter().map(|c| c.views).sum();
    Ok(counts)
}

/// The table on `threads` threads, surface by surface, each surface's trace and nodes cut into
/// chunks of `params.chunk` (the module documentation's estimate).
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
pub fn fit(
    params: Params,
    threads: NonZeroUsize,
) -> Result<(RoughPlainsFit, RunCounts), BuildThreadPoolError> {
    let axes = Axes::of(&params);
    let (limbs, cold_limbs) = (axes.limbs.len(), axes.cold_limbs.len());
    let (width, cold_width) = (axes.colatitudes.len(), axes.cold_colatitudes.len());
    let nodes = axes.irradiances.len();
    let mut shadow = vec![vec![0_u64; limbs]; axes.slopes.len() * width];
    let mut sampled = vec![vec![0_u64; cold_limbs]; axes.slopes.len() * cold_width];
    let mut cold = vec![vec![0_u64; nodes]; axes.slopes.len() * cold_width * cold_limbs];
    let mut instants = vec![0_u64; axes.instants.len()];
    let mut run = RunCounts::default();
    for number in 0..params.surfaces {
        let counts = surface_counts(&params, &axes, number, threads)?;
        for (node, c) in counts.nodes.iter().enumerate() {
            for (total, s) in shadow[node].iter_mut().zip(&c.shadow) {
                *total += s;
            }
            if !(node % width).is_multiple_of(params.cold_colatitude_stride) {
                continue;
            }
            let coarse = node / width * cold_width + node % width / params.cold_colatitude_stride;
            for l in 0..cold_limbs {
                sampled[coarse][l] += c.sampled[l];
                for (total, k) in cold[coarse * cold_limbs + l]
                    .iter_mut()
                    .zip(&c.cold[l * nodes..(l + 1) * nodes])
                {
                    *total += k;
                }
            }
        }
        for (total, c) in instants.iter_mut().zip(&counts.instants) {
            *total += c;
        }
        let side = real(wide(params.side()));
        run.surface_shadow.push(
            counts
                .nodes
                .iter()
                .map(|c| c.shadow.iter().map(|&s| real(s) / (side * side)).collect())
                .collect(),
        );
        run.smith.resize(counts.smith.len(), 0);
        for (total, c) in run.smith.iter_mut().zip(&counts.smith) {
            *total += c;
        }
        run.pieces += counts.pieces;
        run.views += counts.views;
    }
    let side = wide(params.side());
    run.pixels = real(side * side * u64::from(params.surfaces));
    let pixels = run.pixels;
    let digits = params.digits;
    let share = |count: u64, of: f64| round_significant(real(count) / of, digits);
    let fit = RoughPlainsFit {
        shadow: shadow
            .iter()
            .map(|row| row.iter().map(|&s| share(s, pixels)).collect())
            .collect(),
        cold: cold
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let n = sampled[i / cold_limbs][i % cold_limbs];
                row.iter()
                    .map(|&c| if n == 0 { 0.0 } else { share(c, real(n)) })
                    .collect()
            })
            .collect(),
        instantaneous: instants
            .iter()
            .map(|&c| share(c, pixels * f64::from(params.azimuths)))
            .collect(),
        axes,
        params,
    };
    Ok((fit, run))
}

/// What a run counted beyond its table, for its figures.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunCounts {
    /// Each surface's permanent shadow, unrounded, laid out as the table's.
    pub surface_shadow: Vec<Vec<Vec<f64>>>,
    /// The horizons at least each of [`smith_points`]' tan e ÷ w, over every surface.
    pub smith: Vec<u64>,
    /// The pieces the sampled pixels' marches found, over every surface.
    pub pieces: u64,
    /// The sampled pixels' view azimuths, over every surface.
    pub views: u64,
    /// The pixels, over every surface.
    pub pixels: f64,
}

/// The Moon's greatest solar declination, 1.54°, Hayne et al.'s δ, in radians.
const MOON_DECLINATION: f64 = 1.54 * PI / 180.0;

/// The Moon under the real Sun, as `science-r09-drainage.md` §8.13 takes it: 1,361 W m⁻² (IAU
/// 2015 Resolution B3's nominal total solar irradiance) at perihelion, 0.98329 au, where the Sun's
/// 959.6″ semidiameter at 1 au makes it 0.2711° in radius; Hayne et al.'s greatest declination
/// 1.54°, albedo 0.12 and emissivity 0.95; and the heat flow 0.018 W m⁻² of the Apollo 15 and 17
/// heat-flow experiments (21 and 14–16 mW m⁻²: Langseth, Keihm and Peters 1976, Proc. Lunar Sci.
/// Conf. 7, 3143, as `cold_trap::BodyParts` cites them).
#[must_use]
fn real_moon() -> Body {
    Body::new(BodyParts {
        flux: WattsPerSquareMetre::new(1361.0 / (0.98329 * 0.98329)),
        greatest_declination: Radians::new(MOON_DECLINATION),
        star_radius: Radians::new(0.2711 * PI / 180.0),
        bond_albedo: 0.12,
        emissivity: 0.95,
        heat_flow: WattsPerSquareMetre::new(0.018),
    })
    .expect("the real Moon's parts are in range")
}

/// The mean of `f` over the latitudes `lo`..`hi` degrees, weighted by area, by the midpoint rule
/// in 1,000 steps; `f` takes the colatitude in radians.
#[must_use]
fn band_mean(lo: f64, hi: f64, f: impl Fn(f64) -> f64) -> f64 {
    let steps = 1000_u32;
    let width = (hi - lo) / f64::from(steps);
    let (mut sum, mut weight) = (0.0, 0.0);
    for i in 0..steps {
        let lat = (lo + width * (f64::from(i) + 0.5)) * PI / 180.0;
        let w = math::cos(lat);
        sum += w * f(FRAC_PI_2 - lat);
        weight += w;
    }
    sum / weight
}

/// The real Moon's cold share of plains of slope `w` at `colatitude` below `threshold`, at the
/// reader's own irradiance fraction.
#[must_use]
fn moon_cold(fit: &RoughPlainsFit, w: f64, colatitude: f64, threshold: Kelvin) -> f64 {
    let moon = real_moon();
    let limb = MOON_DECLINATION + moon.parts().star_radius.value();
    let x = irradiance_limit(Radians::new(colatitude), &moon, threshold);
    fit.cold_share(w, colatitude, limb, x)
}

/// One check slope's figures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandFigures {
    /// The slope w.
    pub slope: f64,
    /// Permanent shadow over 80–90° under a point star 1.54° from the equator.
    pub polar_shadow: f64,
    /// Permanent shadow over 70–80°, the same.
    pub subpolar_shadow: f64,
    /// Water's cold traps over 80–90° under the real Sun, 110 K.
    pub water: f64,
    /// CO₂'s cold traps over 80–90° under the real Sun, 57 K.
    pub co2: f64,
}

/// What a run measured, for the header's acceptance line.
#[derive(Debug, Clone, PartialEq)]
pub struct Figures {
    /// The largest |traced ÷ Smith − 1| of the instantaneous shadow at [`smith_points`], on the
    /// traced horizons themselves, and its point (w, degrees).
    pub smith_worst: (f64, f64, f64),
    /// The bands at each of [`CHECK_SLOPES`].
    pub bands: Vec<BandFigures>,
    /// The least and greatest surface's permanent shadow over 80–90° at w 0.1 and 0.3, under a
    /// point star 1.54° from the equator.
    pub spreads: [(f64, f64); 2],
    /// The largest permanent shadow at the last colatitude node, and at the last limb node.
    pub edges: (f64, f64),
    /// The pieces a sampled pixel's march finds along a view azimuth, on average.
    pub pieces_per_view: f64,
}

/// The figures of `fit` and `run`.
#[must_use]
pub fn figures(fit: &RoughPlainsFit, run: &RunCounts) -> Figures {
    let mut smith_worst = (0.0, 0.0, 0.0);
    for (&(w, e), &count) in smith_points().iter().zip(&run.smith) {
        let traced = real(count) / (run.pixels * f64::from(fit.params.azimuths));
        let error = (traced / smith_shadow(w, e * PI / 180.0) - 1.0).abs();
        if error > smith_worst.0 {
            smith_worst = (error, w, e);
        }
    }
    let bands = CHECK_SLOPES
        .iter()
        .map(|&w| BandFigures {
            slope: w,
            polar_shadow: band_mean(80.0, 90.0, |c| fit.permanent_shadow(w, c, MOON_DECLINATION)),
            subpolar_shadow: band_mean(70.0, 80.0, |c| {
                fit.permanent_shadow(w, c, MOON_DECLINATION)
            }),
            water: band_mean(80.0, 90.0, |c| moon_cold(fit, w, c, Kelvin::new(110.0))),
            co2: band_mean(80.0, 90.0, |c| moon_cold(fit, w, c, Kelvin::new(57.0))),
        })
        .collect();
    let spread = |w: f64| {
        let mut range = (f64::INFINITY, f64::NEG_INFINITY);
        for surface in &run.surface_shadow {
            let band = band_mean(80.0, 90.0, |c| {
                read(&fit.axes, surface, Share::Shadow, w, c, MOON_DECLINATION)
            });
            range = (range.0.min(band), range.1.max(band));
        }
        range
    };
    let width = fit.axes.colatitudes.len();
    let mut edges = (0.0_f64, 0.0_f64);
    for (row, shares) in fit.shadow.iter().enumerate() {
        if row % width == width - 1 {
            edges.0 = shares.iter().copied().fold(edges.0, f64::max);
        }
        edges.1 = edges.1.max(shares[shares.len() - 1]);
    }
    Figures {
        smith_worst,
        bands,
        spreads: [spread(0.1), spread(0.3)],
        edges,
        pieces_per_view: real(run.pieces) / real(run.views.max(1)),
    }
}

/// A table of rows as a `pub static`, one row a line in the emitter's literals: a `static` rather
/// than the emitter's `const`, since Clippy refuses so large a `const` array
/// (`large_const_arrays`), as `bowl_permanent_shadow`'s `SHADOW` is.
#[must_use]
fn rows_item(name: &str, doc: &[&str], rows: &[Vec<f64>]) -> TableItem {
    let width = rows.first().map_or(0, Vec::len);
    let mut out = String::new();
    for line in doc {
        // Writing to a String cannot fail.
        let _ = writeln!(out, "/// {line}");
    }
    out.push_str("#[rustfmt::skip]\n");
    // Writing to a String cannot fail.
    let _ = writeln!(
        out,
        "pub static {name}: [[f64; {width}]; {}] = [",
        rows.len()
    );
    for row in rows {
        let values: Vec<String> = row.iter().map(|&v| crate::emit::literal(v)).collect();
        // Writing to a String cannot fail.
        let _ = writeln!(out, "    [{}],", values.join(", "));
    }
    out.push_str("];\n");
    TableItem::Source(out)
}

/// The table's contents.
#[must_use]
pub fn render(fit: &RoughPlainsFit) -> RustTable {
    let p = &fit.params;
    let a = &fit.axes;
    let notes = format!(
        "\
Axes: the slope w = tan `σ_s` at the {nw} nodes of `SLOPE`; the colatitude e₀ in radians
over w at {nc} even nodes on [0, {cr}]; D = δ + α, the star's greatest declination plus its
angular radius, in radians over w at {nd} even nodes on [0, {dr}]; the cold shares' nodes,
every {cs}th colatitude and every {ds}th limb; and the irradiance as a fraction of S sin e,
e = min(90°, e₀ + D), at 0 and {nx} nodes even in their logarithm from {xl} to {xh}.
`SHADOW` has a row for each slope and colatitude, slope major, and a share of the ground for
each limb; read it trilinear in w, e₀ ÷ w and D ÷ w. `COLD` has a row for each slope, cold
colatitude and cold limb, in that order, and the share of that node's permanent shadow whose
peak irradiance from sunlit terrain is at most each fraction. A point's cold share is its
shadow times the mean of the cold shares at the eight cold nodes about it, each weighted by
its trilinear weight and its node's shadow, and each read at the fraction times (`w_n` ÷ w)²
for its slope node `w_n`: linear in the fraction from 0 to the first node, then linear in the
logarithms of the fraction and the share where both shares are positive and in the fraction's
logarithm alone where not, and the last node's from there on. Both are zero for a plain with
no slope, where e₀ + D ≥ 90° and beyond the last colatitude or limb; a slope below the first
node reads the first slice at its own ratios. `INSTANTANEOUS` is the share of the ground and
azimuths whose horizon is at least the sun's elevation e, at the {ni} even nodes of tan e ÷ w
on [0, {ir}].

Traced on {s} periodic surfaces of {g} × {g} pixels from seed {seed:#x}: modes of amplitude
|k|^−(1+{h}) at {kl} ≤ |k| ≤ {kh} cycles across, random phases, heights scaled to a
directional RMS slope of 1 over the facets. Horizons at {k} azimuths by rays of {step}-pixel
steps to {reach} pixels; the irradiance first order, the lit facets' direct flux weighted by
view factor along every {vs}th azimuth, its peak over the star's path sampled at every
{ss}th; at most {samples} shadowed pixels a surface and node traced for it. Shares rounded
to {digits} significant figures.",
        nw = a.slopes.len(),
        nc = a.colatitudes.len(),
        cr = p.colatitude_range,
        nd = a.limbs.len(),
        dr = p.limb_range,
        cs = p.cold_colatitude_stride,
        ds = p.cold_limb_stride,
        nx = p.irradiance_nodes,
        xl = p.irradiance_min,
        xh = p.irradiance_max,
        ni = a.instants.len(),
        ir = p.instant_range,
        s = p.surfaces,
        g = p.grid,
        seed = p.seed,
        h = p.hurst,
        kl = p.wavenumber_low,
        kh = p.wavenumber_high,
        k = p.azimuths,
        step = p.step,
        reach = p.reach,
        vs = p.view_stride,
        ss = p.sun_stride,
        samples = p.samples,
        digits = p.digits,
    );
    RustTable {
        summary: vec![
            "Rough plains' permanent shadow and its cold shares, by ray tracing Gaussian surfaces"
                .to_owned(),
            "(rendering plan R09, R09.T7.g): what `cold_trap::rough_plains` reads.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: items(fit),
    }
}

/// The table's items, in [`RoughPlainsColdTrapsTask`]'s order.
#[must_use]
fn items(fit: &RoughPlainsFit) -> Vec<TableItem> {
    let a = &fit.axes;
    let array = |name: &str, doc: &[&str], values: &[f64]| TableItem::Array {
        name: name.to_owned(),
        doc: doc.iter().map(|&l| l.to_owned()).collect(),
        values: values.to_vec(),
    };
    vec![
        array(
            "SLOPE",
            &["The slope nodes, the directional RMS slope w = tan `σ_s`, ascending."],
            &a.slopes,
        ),
        array(
            "COLATITUDE_PER_SLOPE",
            &["The colatitude in radians over w at the nodes, ascending from 0."],
            &a.colatitudes,
        ),
        array(
            "LIMB_PER_SLOPE",
            &[
                "The star's greatest declination plus its angular radius, in radians over w, at",
                "the nodes, ascending from 0.",
            ],
            &a.limbs,
        ),
        array(
            "COLD_COLATITUDE_PER_SLOPE",
            &[
                "The colatitude in radians over w at the cold shares' nodes, every k-th of the",
                "colatitude's.",
            ],
            &a.cold_colatitudes,
        ),
        array(
            "COLD_LIMB_PER_SLOPE",
            &["D = δ + α in radians over w at the cold shares' nodes, every k-th of the limb's."],
            &a.cold_limbs,
        ),
        array(
            "IRRADIANCE",
            &[
                "The peak irradiance's nodes, fractions of S sin e: 0, then even in their",
                "logarithm.",
            ],
            &a.irradiances,
        ),
        rows_item(
            "SHADOW",
            &[
                "The permanent shadow's share of the ground, a row for each slope and colatitude",
                "(slope major) and a share for each limb.",
            ],
            &fit.shadow,
        ),
        rows_item(
            "COLD",
            &[
                "The share of each cold node's permanent shadow whose peak irradiance is at most",
                "each fraction, a row for each slope, cold colatitude and cold limb (in that",
                "order).",
            ],
            &fit.cold,
        ),
        array(
            "TAN_ELEVATION_PER_SLOPE",
            &["tan e ÷ w at the instantaneous shadow's nodes, ascending from 0."],
            &a.instants,
        ),
        array(
            "INSTANTANEOUS",
            &[
                "The share of the ground and azimuths whose horizon is at least the sun's elevation.",
            ],
            &fit.instantaneous,
        ),
    ]
}

/// The task (R09.T7.g): slow, so its committed body is held to the rendering of its committed
/// values, its manifest to its inputs hash, and its smoke run is the tests'.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RoughPlainsColdTrapsTask;

impl FitTask for RoughPlainsColdTrapsTask {
    fn name(&self) -> &'static str {
        "rough_plains_cold_traps"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_crate(&self) -> TableCrate {
        TableCrate::Surface
    }

    fn table_path(&self) -> &'static str {
        "rough_plains_cold_traps.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "SLOPE",
            "COLATITUDE_PER_SLOPE",
            "LIMB_PER_SLOPE",
            "COLD_COLATITUDE_PER_SLOPE",
            "COLD_LIMB_PER_SLOPE",
            "IRRADIANCE",
            "SHADOW",
            "COLD",
            "TAN_ELEVATION_PER_SLOPE",
            "INSTANTANEOUS",
        ]
    }

    /// None: the fit reads no constant of the sim, only `math` and the Stefan–Boltzmann constant
    /// of its figures.
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(Vec::new())
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let (fit, run) = fit(Params::of(manifest)?, threads)?;
        Ok(TaskOutput {
            table: render(&fit),
            source: "Hayne, Aharonson and Schörghofer (2021, Nat. Astron. 5, 169), Methods D \
                     (arXiv:2005.05369v1); Smith (1967, JGR 72, 4059); the first-order tracer of \
                     science-r09-drainage.md §8.13"
                .to_owned(),
            acceptance: acceptance(&fit.params, &figures(&fit, &run)),
            provisional: None,
        })
    }
}

/// The header's acceptance line: what the run measured.
#[must_use]
fn acceptance(params: &Params, f: &Figures) -> String {
    let mut out = String::new();
    let (sw, sl, se) = f.smith_worst;
    // Writing to a String cannot fail.
    let _ = write!(
        out,
        "{s} surfaces of {g}² pixels, {pv:.1} pieces a sampled view azimuth; instantaneous \
         shadow within {sw:.3} of Smith 1967's at 0.5–4° for w 0.1, 0.3 and 0.5 (worst at w {sl}, \
         {se}°); under a point star 1.54° from the equator, permanent shadow over 80–90° from \
         {a0:.4} to {a1:.4} across the surfaces at w 0.1 and {b0:.4} to {b1:.4} at w 0.3",
        s = params.surfaces,
        g = params.grid,
        pv = f.pieces_per_view,
        a0 = f.spreads[0].0,
        a1 = f.spreads[0].1,
        b0 = f.spreads[1].0,
        b1 = f.spreads[1].1,
    );
    for b in &f.bands {
        // Writing to a String cannot fail.
        let _ = write!(
            out,
            "; w {w}: shadow {p:.4} over 80–90° and {q:.5} over 70–80°, under the real Sun water \
             (110 K) {h:.4} and CO₂ (57 K) {c:.6} over 80–90°",
            w = b.slope,
            p = b.polar_shadow,
            q = b.subpolar_shadow,
            h = b.water,
            c = b.co2,
        );
    }
    // Writing to a String cannot fail.
    let _ = write!(
        out,
        "; the largest shadow at the last colatitude node {e0:.2e} and at the last limb node \
         {e1:.2e}",
        e0 = f.edges.0,
        e1 = f.edges.1,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::bowl_permanent_shadow::polar_shadow;

    /// The smoke manifest.
    fn smoke() -> Manifest {
        Manifest::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("manifests/rough_plains_cold_traps.smoke.toml"),
        )
        .unwrap()
    }

    fn degrees(x: f64) -> f64 {
        x * PI / 180.0
    }

    #[test]
    fn the_fft_is_the_direct_sum() {
        let n = 16_usize;
        let re0: Vec<f64> = (0..n)
            .map(|i| math::sin(1.3 * real(wide(i)) + 0.2))
            .collect();
        let im0: Vec<f64> = (0..n)
            .map(|i| math::cos(0.7 * real(wide(i)) - 1.1))
            .collect();
        let twiddles: Vec<(f64, f64)> = (0..n / 2)
            .map(|k| {
                let (s, c) = math::sin_cos(2.0 * PI * real(wide(k)) / real(wide(n)));
                (c, s)
            })
            .collect();
        let (mut re, mut im) = (re0.clone(), im0.clone());
        inverse_fft(&mut re, &mut im, &twiddles);
        for j in 0..n {
            let (mut sr, mut si) = (0.0, 0.0);
            for k in 0..n {
                let (s, c) = math::sin_cos(2.0 * PI * real(wide(j * k)) / real(wide(n)));
                sr += re0[k] * c - im0[k] * s;
                si += re0[k] * s + im0[k] * c;
            }
            assert!(
                (re[j] - sr).abs() < 1e-12 && (im[j] - si).abs() < 1e-12,
                "{j}"
            );
        }
    }

    #[test]
    fn a_surface_has_unit_slope_and_its_phases_are_its_own() {
        let params = Params::of(&smoke()).unwrap();
        let first = Surface::gaussian(&params, 0);
        assert!(
            (first.slope_rms() - 1.0).abs() < 1e-12,
            "{}",
            first.slope_rms()
        );
        let mean = first.heights.iter().sum::<f64>() / real(wide(first.heights.len()));
        assert!(mean.abs() < 1e-12, "{mean}");
        let again = Surface::gaussian(&params, 0);
        assert!(
            first
                .heights
                .iter()
                .zip(&again.heights)
                .all(|(a, b)| a.total_cmp(b).is_eq())
        );
        let second = Surface::gaussian(&params, 1);
        assert!(
            first
                .heights
                .iter()
                .zip(&second.heights)
                .any(|(a, b)| (a - b).abs() > 1e-3)
        );
        let wide = Params {
            wavenumber_high: 12.0,
            ..params.clone()
        };
        assert_ne!(Surface::gaussian(&wide, 0), first);
        // A mode's phase is keyed by its signed wavenumbers, so a grid twice as fine holds the
        // same surface: its even pixels are the coarse pixels' heights, up to the slope's scale.
        let fine = Surface::gaussian(
            &Params {
                grid: 64,
                reach: 32.0,
                ..params.clone()
            },
            0,
        );
        let ratio = fine.at(0, 0) / first.at(0, 0);
        for (x, y) in [(1, 0), (5, 3), (17, 29), (31, 31)] {
            let r = fine.at(2 * x, 2 * y) / first.at(x, y);
            assert!(
                (r / ratio - 1.0).abs() < 1e-9,
                "({x}, {y}): {r} against {ratio}"
            );
        }
    }

    #[test]
    fn a_flat_surface_has_level_horizons() {
        let params = Params::of(&smoke()).unwrap();
        let flat = Surface::new(32, vec![0.0; 32 * 32]);
        let horizons = Horizons::trace(&flat, &params, NonZeroUsize::MIN).unwrap();
        assert!(horizons.tangents.iter().all(|&t| t == 0.0));
        // One piece a view, the level ground itself, which a level facet does not see.
        let view = seen(&flat, &directions(params.azimuths), 37, 0.3, &params);
        assert_eq!(view.pieces, 18);
        assert_eq!(view.pixels, Vec::<u32>::new());
    }

    /// A spherical bowl of depth ratio `gamma` and rim radius `rim` pixels at the centre of a flat
    /// `side` × `side` plain.
    fn bowl(side: usize, rim: f64, gamma: f64) -> Surface {
        let depth = 2.0 * gamma * rim;
        let sphere = (rim * rim + depth * depth) / (2.0 * depth);
        let centre = sphere - depth;
        let middle = real(wide(side / 2));
        let heights = (0..side * side)
            .map(|at| {
                let (x, y) = (
                    real(wide(at % side)) - middle,
                    real(wide(at / side)) - middle,
                );
                let r2 = x * x + y * y;
                if r2 < rim * rim {
                    centre - (sphere * sphere - r2).sqrt()
                } else {
                    0.0
                }
            })
            .collect();
        Surface::new(side, heights)
    }

    /// The bowl's trace: fine steps, rays across the plain.
    fn bowl_params() -> Params {
        Params {
            azimuths: 180,
            view_stride: 2,
            sun_stride: 6,
            step: 0.5,
            reach: 128.0,
            chunk: 16,
            ..Params::of(&smoke()).unwrap()
        }
    }

    #[test]
    fn a_bowl_at_the_pole_keeps_eq_6_and_its_shadow_sees_f_one_less_f() {
        // A spherical bowl's polar shadow is Hayne et al.'s eq. 6, and to first order every
        // shadowed point's irradiance is S sin e f (1 − f): the opening's light, π r² S sin e,
        // spread evenly over the sphere's 4π R², since on a sphere the view factor to an element
        // is its share of the sphere's area. As built: 0.7566 against eq. 6's 0.7562 at d/D 0.1
        // and 0.8915 against 0.8876 at 0.2, the irradiance's mean 2.5% and 3.4% below.
        let (side, rim) = (128, 40.0);
        let params = bowl_params();
        let dirs = directions(params.azimuths);
        for gamma in [0.1, 0.2] {
            let surface = bowl(side, rim, gamma);
            let horizons =
                Horizons::trace(&surface, &params, NonZeroUsize::new(4).unwrap()).unwrap();
            let limb = degrees(1.54);
            let middle = real(wide(side / 2));
            let inside: Vec<usize> = (0..side * side)
                .filter(|&at| {
                    let (x, y) = (
                        real(wide(at % side)) - middle,
                        real(wide(at / side)) - middle,
                    );
                    x * x + y * y < rim * rim
                })
                .collect();
            // At the pole a horizon's declination is its elevation.
            let shadowed: Vec<usize> = inside
                .iter()
                .copied()
                .filter(|&at| (0..dirs.len()).all(|k| horizons.tangent(at, k) > math::tan(limb)))
                .collect();
            let share = real(wide(shadowed.len())) / real(wide(inside.len()));
            let eq6 = polar_shadow(gamma, limb);
            assert!(
                (share - eq6).abs() < 0.02,
                "d/D {gamma}: {share} against eq. 6's {eq6}"
            );
            let seens: Vec<Seen> = shadowed
                .iter()
                .map(|&at| seen(&surface, &dirs, at, 1.0, &params))
                .collect();
            let normals: Vec<[f64; 3]> = (0..side * side)
                .map(|at| normal(surface.gradient(at), 1.0))
                .collect();
            let path = Path {
                sin_lat: 1.0,
                cos_lat: 0.0,
                sin_limb: math::sin(limb),
            };
            let view = SlopeView {
                surface: &surface,
                horizons: &horizons,
                dirs: &dirs,
                normals: &normals,
                w: 1.0,
            };
            let peaks = peak_irradiances(&view, &params, &seens, path);
            let f = 4.0 * gamma * gamma / (1.0 + 4.0 * gamma * gamma);
            let expected = f * (1.0 - f);
            let mean = peaks.iter().sum::<f64>() / real(wide(peaks.len())) / math::sin(limb);
            assert!(
                (mean / expected - 1.0).abs() < 0.05,
                "d/D {gamma}: mean {mean} against f (1 − f) {expected}"
            );
            let within = peaks
                .iter()
                .filter(|&&p| (p / math::sin(limb) / expected - 1.0).abs() < 0.15)
                .count();
            assert!(
                within * 10 >= peaks.len() * 9,
                "d/D {gamma}: {within} of {} within 15%",
                peaks.len()
            );
        }
    }

    #[test]
    fn a_pixel_in_permanent_shadow_never_sees_the_star_on_its_path() {
        // The least horizon declination above sin D is the star below the horizon at every
        // azimuth of its path at D, and at or below it the star rises somewhere.
        let params = Params::of(&smoke()).unwrap();
        let surface = Surface::gaussian(&params, 0);
        let horizons = Horizons::trace(&surface, &params, NonZeroUsize::MIN).unwrap();
        let dirs = directions(params.azimuths);
        for (w, colatitude, limb) in [(0.3, 0.0, 0.05), (0.3, 0.2, 0.03), (1.0, 0.6, 0.2)] {
            let (sin_lat, cos_lat) = math::sin_cos(FRAC_PI_2 - colatitude);
            let least = least_declinations(&horizons, &dirs, w, (sin_lat, cos_lat));
            let sin_limb = math::sin(limb);
            for (at, &low) in least.iter().enumerate() {
                let rises = dirs.iter().enumerate().any(|(k, &dir)| {
                    let sun = sun_at(dir, sin_lat, cos_lat, sin_limb);
                    sun[2] / (sun[0] * sun[0] + sun[1] * sun[1]).sqrt()
                        > w * horizons.tangent(at, k)
                });
                assert_eq!(
                    low <= sin_limb,
                    rises,
                    "w {w}, e₀ {colatitude}, D {limb}: {at}"
                );
            }
        }
    }

    #[test]
    fn the_table_is_the_same_on_any_thread_count_and_chunk_size() {
        let params = Params::of(&smoke()).unwrap();
        let (one, _) = fit(
            Params {
                chunk: 1,
                ..params.clone()
            },
            NonZeroUsize::MIN,
        )
        .unwrap();
        let (four, _) = fit(
            Params {
                chunk: 3,
                ..params.clone()
            },
            NonZeroUsize::new(4).unwrap(),
        )
        .unwrap();
        let same = |a: &[Vec<f64>], b: &[Vec<f64>]| {
            a.len() == b.len()
                && a.iter()
                    .flatten()
                    .zip(b.iter().flatten())
                    .all(|(x, y)| x.total_cmp(y).is_eq())
        };
        assert!(same(&one.shadow, &four.shadow) && same(&one.cold, &four.cold));
        assert!(
            one.instantaneous
                .iter()
                .zip(&four.instantaneous)
                .all(|(x, y)| x.total_cmp(y).is_eq())
        );
        // Shares in [0, 1]; the shadow does not rise with D, the cold shares fall with the
        // fraction nowhere, and the cold share read never exceeds the shadow.
        for row in &one.shadow {
            assert!(row.iter().all(|s| (0.0..=1.0).contains(s)), "{row:?}");
            assert!(row.windows(2).all(|w| w[1] <= w[0]), "{row:?}");
        }
        for row in &one.cold {
            assert!(row.windows(2).all(|w| w[1] >= w[0]), "{row:?}");
        }
        for w in [0.02, 0.05, 0.2, 1.0] {
            for c in [0.0, 0.01, 0.05, 0.2] {
                for d in [0.0, 0.005, 0.02, 0.1] {
                    let shadow = one.permanent_shadow(w, c, d);
                    for x in [1e-4, 0.01, 0.3, 5.0, 100.0] {
                        assert!(one.cold_share(w, c, d, x) <= shadow, "{w} {c} {d} {x}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_task_runs_its_smoke_manifest() {
        let task = RoughPlainsColdTrapsTask;
        let output = task.run(&smoke(), NonZeroUsize::new(4).unwrap()).unwrap();
        let body = output.table.body().unwrap();
        let declared: Vec<&str> = body
            .lines()
            .filter_map(|l| {
                l.strip_prefix("pub const ")
                    .or_else(|| l.strip_prefix("pub static "))
            })
            .filter_map(|l| l.split(':').next())
            .collect();
        assert_eq!(declared, task.items());
        assert!(output.acceptance.contains("of Smith 1967's at 0.5–4°"));
        assert!(output.acceptance.contains("w 0.5: shadow"));
        let bad = String::from_utf8(std::fs::read(smoke().path()).unwrap())
            .unwrap()
            .replace("cold_limb_stride = 3", "cold_limb_stride = 4");
        let bad = Manifest::from_bytes(std::path::Path::new("bad.toml"), bad.into_bytes()).unwrap();
        assert!(matches!(
            task.run(&bad, NonZeroUsize::MIN),
            Err(RunTaskError::Param(_))
        ));
        assert!(task.fingerprint().is_empty());
        assert_eq!(task.table_crate(), TableCrate::Surface);
        assert_eq!(task.class(), TaskClass::Slow);
    }

    #[test]
    fn the_manifest_refuses_parameters_out_of_range() {
        let text = String::from_utf8(std::fs::read(smoke().path()).unwrap()).unwrap();
        for changes in [
            &[("grid = 32", "grid = 48")][..],
            &[("azimuths = 36", "azimuths = 35")],
            &[("wavenumber_low = 2.0", "wavenumber_low = 9.0")],
            &[
                ("wavenumber_low = 2.0", "wavenumber_low = 2.1"),
                ("wavenumber_high = 8.0", "wavenumber_high = 2.9"),
            ],
            &[("hurst = 0.9", "hurst = 1.5")],
            &[("slopes = [0.05, 0.3, 1.0]", "slopes = [0.1, 0.3, 1.0]")],
            &[("step = 1.0", "step = 1e-6")],
            &[("cold_limb_stride = 3", "cold_limb_stride = 4")],
        ] {
            let mut bad = text.clone();
            for (from, to) in changes {
                assert!(bad.contains(from), "{from}");
                bad = bad.replace(from, to);
            }
            let manifest =
                Manifest::from_bytes(std::path::Path::new("bad.toml"), bad.into_bytes()).unwrap();
            assert!(Params::of(&manifest).is_err(), "{changes:?}");
        }
        assert!(Params::of(&smoke()).is_ok());
    }

    #[test]
    fn a_share_is_read_as_a_power_of_the_fraction_between_nodes() {
        let nodes = [0.0, 1e-3, 1e-2, 1e-1];
        // A tail ∝ x², read exactly between positive nodes.
        let squares = [0.0, 1e-6, 1e-4, 1e-2];
        for x in [1.5e-3, 3e-3, 7e-3, 2e-2, 5e-2] {
            let read = share_at(&nodes, &squares, x);
            assert!((read / (x * x) - 1.0).abs() < 1e-12, "{x}: {read}");
        }
        // Linear from 0 to the first node, the last node's beyond the last, and linear in ln x
        // where a share is zero.
        assert!((share_at(&nodes, &[0.2, 0.4, 0.6, 0.8], 5e-4) - 0.3).abs() < 1e-15);
        assert!((share_at(&nodes, &squares, 5.0) - 1e-2).abs() < 1e-18);
        let flat = [0.0, 0.0, 0.5, 1.0];
        let midway = share_at(&nodes, &flat, math::powf(10.0, -2.5));
        assert!((midway - 0.25).abs() < 1e-12, "{midway}");
    }
}
