//! The spectral bins (plan 14, P14.T49.i): a row's spherical albedo averaged over each of
//! `substance::optics::SPECTRAL_BIN_EDGES_NM`'s bins, from its optical constants on a fine grid.
//!
//! # The bins
//!
//! A bin's value is the mean over the bin, uniform in wavelength, of the spectral spherical albedo
//! of the row's reference form: at each wavelength, w by the equivalent slab
//! ([`spectral_single_scattering_albedo`]) at the reference grain size, and the spherical albedo of
//! the similarity relation's isotropic medium ([`super::energy_albedos`]), read from a cache
//! ([`SphericalAlbedoCache`]). The mean is the midpoint rule on a grid whose nodes are every
//! knot of the row's data, every join, every bin edge and the data's reach, each interval between
//! two of them cut into equal steps of at most [`SPECTRAL_STEP_NM`]: the integrand is smooth
//! between the data's knots, and a step in it (a join) falls on a node. A bin the data cover in
//! part is the mean over the covered part, and a bin they do not cover takes the nearest covered
//! bin's value ([`spectral_bins`]).
//!
//! # The other inputs
//!
//! [`spectral_bins`] reads a row through [`SpectralGrain`]: its reach, its knots and its w at a
//! wavelength. [`SpectralIndex`] is the grain of optical constants n + ik; a row whose w comes
//! from another input (a laboratory spectrum inverted for w, a mixture of minerals' w, an opaque
//! slab or measured levels) implements the trait in [`super::lab`].

use hyperion_sim::math;
use hyperion_sim::substance::optics::{SPECTRAL_BIN_EDGES_NM, SPECTRAL_BINS};
use hyperion_sim::units::Metres;

use super::{energy_albedos, similar_isotropic_albedo, single_scattering_albedo};

/// The grid's largest step, nm: 0.1. Halving it moves no bin of any row by more than 10⁻⁴
/// (`substance_albedos_the_bins_converge_on_the_grid`, which measures the largest move); at 0.5
/// nm, `CO`'s band moved its last bin by 4.6 × 10⁻⁴.
pub(crate) const SPECTRAL_STEP_NM: f64 = 0.1;

/// The intervals of [`SphericalAlbedoCache`]'s grid in s, uniform over [0, 1].
const CACHE_INTERVALS: u32 = 256;

/// The spherical albedo of a semi-infinite isotropic medium as a function of its single-scattering
/// albedo w*, tabulated for the bins: [`super::energy_albedos`]'s spherical albedo at the nodes
/// s = √(1 − w*) = j ÷ 256, j = 0 to 256, and a cubic through the four nearest nodes between them.
///
/// The spherical albedo is a smooth function of van de Hulst's similarity parameter s (1974, A&A
/// 35, 209, eq. 13; 1 − A ≈ 4s ÷ √3 near s = 0), where it is not of w*, so the grid is in s. The
/// cache's error against [`super::energy_albedos`] is held below 10⁻⁷ at any w by
/// `substance_albedos_the_spherical_albedo_cache_is_exact_to_its_bound`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SphericalAlbedoCache {
    /// The spherical albedo at each node.
    values: Vec<f64>,
}

impl Default for SphericalAlbedoCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SphericalAlbedoCache {
    /// The cache, computed once: 257 of [`super::energy_albedos`]'s isotropic media.
    #[must_use]
    pub(crate) fn new() -> Self {
        let values = (0..=CACHE_INTERVALS)
            .map(|j| {
                let s = f64::from(j) / f64::from(CACHE_INTERVALS);
                energy_albedos(1.0 - s * s, 0.0).1
            })
            .collect();
        Self { values }
    }

    /// The spherical albedo of grains of single-scattering albedo `w` and phase-function asymmetry
    /// `asymmetry` by the similarity relation ([`similar_isotropic_albedo`]), in [0, 1].
    ///
    /// # Panics
    ///
    /// Never: the cubic's first node is at most 253, which a `usize` holds on every target.
    #[must_use]
    pub(crate) fn spherical_albedo(&self, w: f64, asymmetry: f64) -> f64 {
        let similar = similar_isotropic_albedo(w, asymmetry);
        let s = (1.0 - similar).clamp(0.0, 1.0).sqrt();
        let x = s * f64::from(CACHE_INTERVALS);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "x is in [0, 256], so its floor is a small non-negative integer"
        )]
        let cell = x.floor() as u32;
        // The cubic's four nodes: the interval's two and one beyond each, shifted inward at the
        // ends.
        let first = cell.saturating_sub(1).min(CACHE_INTERVALS - 3);
        let t = x - f64::from(first);
        let weights = [
            -(t - 1.0) * (t - 2.0) * (t - 3.0) / 6.0,
            t * (t - 2.0) * (t - 3.0) / 2.0,
            -t * (t - 1.0) * (t - 3.0) / 2.0,
            t * (t - 1.0) * (t - 2.0) / 6.0,
        ];
        let start = usize::try_from(first).expect("a node's index fits a usize");
        let sum = weights
            .iter()
            .zip(&self.values[start..start + 4])
            .fold(0.0, |sum, (weight, value)| sum + weight * value);
        sum.clamp(0.0, 1.0)
    }
}

/// A grain's single-scattering albedo at a wavelength where its index may be anything a band
/// gives: the equivalent slab ([`single_scattering_albedo`]) where n is at least 1. Where n falls
/// below 1, inside a strong band's anomalous dispersion (the ices' fundamentals beyond 2.8 µm), the
/// slab's
/// internal terms, `S_i` and the mean path ⟨D⟩ = ⅔ [n² − (n² − 1)^{3/2} ÷ n] D, are not real, so
/// they take their limit at n = 1 (`S_i` = 1 − (0.9413 − 0.8543 R(0) − 0.0870 R(0)²), ⟨D⟩ = ⅔ D),
/// and R(0), the surface's reflection, takes the index as it is: continuous with the slab at n = 1,
/// and the reflection of a band's core, where n ≪ 1 and k is large, kept. The lane's extension,
/// which no source gives; `substance_albedos_the_spectral_slab_takes_any_index` holds its
/// continuity. A negative k, noise about zero in a measurement, is taken as 0.
#[must_use]
pub(crate) fn spectral_single_scattering_albedo(
    n: f64,
    k: f64,
    wavelength_nm: f64,
    diameter: Metres,
) -> f64 {
    let k = k.max(0.0);
    if n >= 1.0 {
        return single_scattering_albedo(n, k, wavelength_nm, diameter);
    }
    let r0 = ((n - 1.0) * (n - 1.0) + k * k) / ((n + 1.0) * (n + 1.0) + k * k);
    let s_e = 0.0587 + 0.8543 * r0 + 0.0870 * r0 * r0;
    let s_i = 1.0 - (0.9413 - 0.8543 * r0 - 0.0870 * r0 * r0);
    let alpha = 4.0 * core::f64::consts::PI * k / (wavelength_nm * 1.0e-9);
    let theta = math::exp(-alpha * 2.0 / 3.0 * diameter.value());
    let w = s_e + (1.0 - s_e) * (1.0 - s_i) * theta / (1.0 - s_i * theta);
    if w > 1.0 { 1.0 } else { w }
}

/// How a tabulated quantity is interpolated between two knots λ₀ < λ < λ₁.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Interpolation {
    /// Linear in λ.
    Linear,
    /// ln y linear in λ, or y linear in λ where either knot's y is not positive: R08's reduction
    /// of k, as [`super::index_at`] takes it.
    LogLinear,
    /// Linear in ln λ: Warren and Brandt 2008's recommendation for n.
    LinearInLogWavelength,
    /// ln y linear in ln λ, or y linear in ln λ where either knot's y is not positive: Warren and
    /// Brandt 2008's recommendation for k.
    LogLog,
    /// Linear in wavenumber, 1 ÷ λ: a table on a wavenumber grid.
    LinearInWavenumber,
}

/// A tabulated quantity of wavelength: knots of rising wavelength, nm, their values, and the
/// interpolation between them; held at its end knots' values outside them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Curve {
    wavelengths_nm: Vec<f64>,
    values: Vec<f64>,
    interpolation: Interpolation,
}

impl Curve {
    /// The curve of `knots`, (wavelength nm, value) pairs of strictly rising wavelength, by
    /// `interpolation`.
    ///
    /// # Panics
    ///
    /// If `knots` is empty, or its wavelengths do not rise strictly: a broken invariant, since
    /// every reader checks its knots first and refuses a table without them (`infrared`'s with
    /// `FitAlbedoError::Infrared`, `lab`'s with `FitAlbedoError::Spectrum`).
    #[must_use]
    pub(crate) fn new(knots: &[(f64, f64)], interpolation: Interpolation) -> Self {
        assert!(!knots.is_empty(), "a curve has a knot");
        assert!(
            knots.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "a curve's wavelengths rise strictly"
        );
        Self {
            wavelengths_nm: knots.iter().map(|knot| knot.0).collect(),
            values: knots.iter().map(|knot| knot.1).collect(),
            interpolation,
        }
    }

    /// A constant.
    #[must_use]
    pub(crate) fn constant(value: f64) -> Self {
        Self::new(&[(1.0, value)], Interpolation::Linear)
    }

    /// Its knots' wavelengths, nm.
    #[must_use]
    pub(crate) fn wavelengths_nm(&self) -> &[f64] {
        &self.wavelengths_nm
    }

    /// Its value at `wavelength_nm`: a knot's own value at a knot, interpolated between knots, and
    /// the end knot's value beyond either end.
    #[must_use]
    pub(crate) fn at(&self, wavelength_nm: f64) -> f64 {
        let grid = &self.wavelengths_nm;
        let upper = grid.partition_point(|&w| w < wavelength_nm);
        if upper == grid.len() {
            return self.values[grid.len() - 1];
        }
        if upper == 0 || grid[upper].total_cmp(&wavelength_nm).is_eq() {
            return self.values[upper];
        }
        let lower = upper - 1;
        let (w0, w1) = (grid[lower], grid[upper]);
        let (y0, y1) = (self.values[lower], self.values[upper]);
        let positive = y0 > 0.0 && y1 > 0.0;
        let in_log = || (math::ln(wavelength_nm) - math::ln(w0)) / (math::ln(w1) - math::ln(w0));
        match self.interpolation {
            Interpolation::Linear => y0 + (wavelength_nm - w0) / (w1 - w0) * (y1 - y0),
            Interpolation::LogLinear if positive => {
                let t = (wavelength_nm - w0) / (w1 - w0);
                math::exp(math::ln(y0) + t * (math::ln(y1) - math::ln(y0)))
            }
            Interpolation::LogLinear => {
                let t = (wavelength_nm - w0) / (w1 - w0);
                y0 + t * (y1 - y0)
            }
            Interpolation::LogLog if positive => {
                math::exp(math::ln(y0) + in_log() * (math::ln(y1) - math::ln(y0)))
            }
            Interpolation::LinearInLogWavelength | Interpolation::LogLog => {
                y0 + in_log() * (y1 - y0)
            }
            Interpolation::LinearInWavenumber => {
                let t = (1.0 / wavelength_nm - 1.0 / w0) / (1.0 / w1 - 1.0 / w0);
                y0 + t * (y1 - y0)
            }
        }
    }
}

/// One stretch of a row's optical constants: n and k over [`lo_nm`](Self::lo_nm) to
/// [`hi_nm`](Self::hi_nm), each a [`Curve`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Piece {
    lo_nm: f64,
    hi_nm: f64,
    n: Curve,
    k: Curve,
}

impl Piece {
    /// The stretch from `lo_nm` to `hi_nm` (lo below hi) of real index `n` and imaginary index `k`.
    ///
    /// # Panics
    ///
    /// If `lo_nm` is not below `hi_nm`: a broken invariant, since `infrared`'s readers check their
    /// joins first and refuse, with `FitAlbedoError::Infrared`, a set whose rows do not lie where
    /// its joins need them.
    #[must_use]
    pub(crate) fn new(lo_nm: f64, hi_nm: f64, n: Curve, k: Curve) -> Self {
        assert!(lo_nm < hi_nm, "a piece spans {lo_nm} to {hi_nm} nm");
        Self { lo_nm, hi_nm, n, k }
    }

    /// Its first wavelength, nm.
    #[must_use]
    pub(crate) const fn lo_nm(&self) -> f64 {
        self.lo_nm
    }

    /// Its last wavelength, nm.
    #[must_use]
    pub(crate) const fn hi_nm(&self) -> f64 {
        self.hi_nm
    }

    /// Its (n, k) at `wavelength_nm`.
    #[must_use]
    pub(crate) fn at(&self, wavelength_nm: f64) -> (f64, f64) {
        (self.n.at(wavelength_nm), self.k.at(wavelength_nm))
    }

    /// Its ends and its curves' knots between them, nm: where its n and k are smooth between.
    #[must_use]
    pub(crate) fn knots_nm(&self) -> Vec<f64> {
        let mut knots = vec![self.lo_nm, self.hi_nm];
        for curve in [&self.n, &self.k] {
            knots.extend(
                curve
                    .wavelengths_nm()
                    .iter()
                    .copied()
                    .filter(|&w| w > self.lo_nm && w < self.hi_nm),
            );
        }
        knots
    }
}

/// A grain whose spectral single-scattering albedo [`spectral_bins`] averages: what a row's bins
/// are computed from.
pub(crate) trait SpectralGrain {
    /// Its data's reach, nm: the first and last wavelength over which its w is given.
    fn reach_nm(&self) -> [f64; 2];

    /// The wavelengths, nm, within its reach, between which its w is smooth: its data's knots and
    /// joins, which the grid takes as nodes.
    fn knots_nm(&self) -> Vec<f64>;

    /// Its single-scattering albedo at `wavelength_nm`, within its reach, for grains of
    /// `diameter`.
    fn single_scattering_albedo(&self, wavelength_nm: f64, diameter: Metres) -> f64;
}

/// A row's optical constants over its reach: contiguous [`Piece`]s, each starting where the last
/// ends, R08's file's first and the infrared set's after it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpectralIndex {
    pieces: Vec<Piece>,
}

impl SpectralIndex {
    /// The index of `pieces`, contiguous and in order.
    ///
    /// # Panics
    ///
    /// If `pieces` is empty or not contiguous: each row's builder joins them end to end.
    #[must_use]
    pub(crate) fn new(pieces: Vec<Piece>) -> Self {
        assert!(!pieces.is_empty(), "an index has a piece");
        assert!(
            pieces
                .windows(2)
                .all(|pair| pair[0].hi_nm.total_cmp(&pair[1].lo_nm).is_eq()),
            "an index's pieces are contiguous"
        );
        Self { pieces }
    }

    /// Its pieces, in order.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Its (n, k) at `wavelength_nm`: the piece's that holds it, the later one's at a join.
    #[must_use]
    pub(crate) fn at(&self, wavelength_nm: f64) -> (f64, f64) {
        let piece = self
            .pieces
            .iter()
            .rev()
            .find(|piece| piece.lo_nm <= wavelength_nm)
            .unwrap_or(&self.pieces[0]);
        piece.at(wavelength_nm)
    }
}

impl SpectralGrain for SpectralIndex {
    fn reach_nm(&self) -> [f64; 2] {
        [
            self.pieces[0].lo_nm,
            self.pieces[self.pieces.len() - 1].hi_nm,
        ]
    }

    fn knots_nm(&self) -> Vec<f64> {
        self.pieces.iter().flat_map(Piece::knots_nm).collect()
    }

    fn single_scattering_albedo(&self, wavelength_nm: f64, diameter: Metres) -> f64 {
        let (n, k) = self.at(wavelength_nm);
        spectral_single_scattering_albedo(n, k, wavelength_nm, diameter)
    }
}

/// The spherical albedo of `grain` in each spectral bin, for grains of `diameter` and
/// phase-function asymmetry `asymmetry`, on a grid of largest step `step_nm` (the [module
/// documentation](self)).
///
/// # Panics
///
/// If the grain's reach covers no bin, or `step_nm` is not positive.
#[must_use]
pub(crate) fn spectral_bins(
    grain: &dyn SpectralGrain,
    diameter: Metres,
    asymmetry: f64,
    cache: &SphericalAlbedoCache,
    step_nm: f64,
) -> [f64; SPECTRAL_BINS] {
    assert!(step_nm > 0.0, "the grid's step is positive, not {step_nm}");
    let [lo, hi] = grain.reach_nm();
    let edges = SPECTRAL_BIN_EDGES_NM;
    let mut nodes: Vec<f64> = grain
        .knots_nm()
        .into_iter()
        .chain(edges)
        .chain([lo, hi])
        .filter(|&w| w >= lo && w <= hi)
        .collect();
    nodes.sort_by(f64::total_cmp);
    nodes.dedup_by(|a, b| a.total_cmp(b).is_eq());
    let mut sums = [0.0; SPECTRAL_BINS];
    let mut widths = [0.0; SPECTRAL_BINS];
    for pair in nodes.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let Some(bin) = edges.windows(2).position(|e| a >= e[0] && b <= e[1]) else {
            continue;
        };
        let steps = ((b - a) / step_nm).ceil();
        let width = (b - a) / steps;
        let mut sum = 0.0;
        let mut j = 0.0;
        while j < steps {
            let wavelength = a + (j + 0.5) * width;
            let w = grain.single_scattering_albedo(wavelength, diameter);
            sum += cache.spherical_albedo(w, asymmetry);
            j += 1.0;
        }
        sums[bin] += sum * width;
        widths[bin] += b - a;
    }
    let covered: Vec<usize> = (0..SPECTRAL_BINS).filter(|&b| widths[b] > 0.0).collect();
    assert!(!covered.is_empty(), "the grain's reach covers a bin");
    core::array::from_fn(|bin| {
        let source = if widths[bin] > 0.0 {
            bin
        } else {
            // The nearest covered bin: the reach is one interval, so the covered bins are a run.
            *covered
                .iter()
                .min_by_key(|&&c| c.abs_diff(bin))
                .expect("a bin is covered")
        };
        (sums[source] / widths[source]).clamp(0.0, 1.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The curve's rules: a knot's own value, the end values held, and each rule's midpoint.
    #[test]
    fn substance_albedos_a_curve_interpolates_by_its_rule() {
        let knots = [(100.0, 1.0), (400.0, 4.0)];
        let mid = 250.0;
        assert!((Curve::new(&knots, Interpolation::Linear).at(mid) - 2.5).abs() < 1e-12);
        assert!((Curve::new(&knots, Interpolation::LogLinear).at(mid) - 2.0).abs() < 1e-12);
        let geometric = 200.0;
        let log_lambda = Curve::new(&knots, Interpolation::LinearInLogWavelength);
        assert!((log_lambda.at(geometric) - 2.5).abs() < 1e-12);
        assert!((Curve::new(&knots, Interpolation::LogLog).at(geometric) - 2.0).abs() < 1e-12);
        let harmonic = 160.0;
        let wavenumber = Curve::new(&knots, Interpolation::LinearInWavenumber);
        assert!((wavenumber.at(harmonic) - 2.5).abs() < 1e-12);
        let curve = Curve::new(&knots, Interpolation::LogLinear);
        assert!(curve.at(100.0).total_cmp(&1.0).is_eq());
        assert!(curve.at(50.0).total_cmp(&1.0).is_eq());
        assert!(curve.at(500.0).total_cmp(&4.0).is_eq());
        let zero = Curve::new(&[(100.0, 0.0), (400.0, 4.0)], Interpolation::LogLinear);
        assert!((zero.at(mid) - 2.0).abs() < 1e-12, "linear to a zero knot");
    }

    /// The slab's extension below n = 1 is continuous with the slab there, an opaque grain of n
    /// below 1 scatters its surface's reflection alone, and a negative k is a lossless grain's.
    #[test]
    fn substance_albedos_the_spectral_slab_takes_any_index() {
        let d = Metres::new(100.0e-6);
        for k in [1e-6, 1e-4, 3e-3, 0.1] {
            let below = spectral_single_scattering_albedo(1.0 - 1e-9, k, 4_000.0, d);
            let at = spectral_single_scattering_albedo(1.0, k, 4_000.0, d);
            assert!((below - at).abs() < 1e-8, "k {k}: {below} against {at}");
        }
        let opaque = spectral_single_scattering_albedo(0.5, 1.0, 4_000.0, d);
        let r0: f64 = (0.25 + 1.0) / (2.25 + 1.0);
        assert!((opaque - (0.0587 + 0.8543 * r0 + 0.0870 * r0 * r0)).abs() < 1e-12);
        assert!(
            spectral_single_scattering_albedo(1.3, -1e-3, 4_000.0, d)
                .total_cmp(&1.0)
                .is_eq()
        );
    }
}
