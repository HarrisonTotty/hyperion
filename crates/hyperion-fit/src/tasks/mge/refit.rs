//! The refit with free widths (plan 15, P15.T3.a), run from `manifests/mge.refit.toml`, with the
//! boxy bulge's [`super::boxy`] (P15.T3.b).
//!
//! The first cut ([`super::fit`]) fixes its widths on a logarithmic grid and fits only the
//! weights. The refit frees the widths, as Cappellari's method does (2002, MNRAS 333, 400): the
//! weights by non-negative least squares for given widths, and the widths optimised around that
//! solve. The expansion is linear in the weights and non-linear in the widths, so the problem is
//! separable, and it is solved by variable projection (Golub and Pereyra 1973, SIAM J. Numer.
//! Anal. 10, 413), with a non-negative solve in place of their pseudo-inverse as Cappellari
//! does (2002, §3.2), so the projected residual has kinks where the solve's active set changes.
//! Levenberg–Marquardt ([`levenberg_marquardt`]) moves the 14 logarithms of the
//! widths. At every point it evaluates, the weights are the non-negative least-squares solution
//! ([`nnls`]) for those widths, and the residual is what that solution leaves. Its Jacobian is
//! taken by forward differences in ln width, each shifted residual the plain least-squares one
//! ([`least_squares`]) on the columns that NNLS left free at the point: near the point that is
//! the NNLS solution, so this is the variable-projection derivative where the free set holds, and
//! it costs one QR where a fresh NNLS would cost one per column freed. At the end the widths are
//! sorted, ascending, and the
//! weights solved once more on them, so each table's weights are a non-negative solve on its
//! widths.
//!
//! - `MGE_EXP`: `e^(−s)` at log-spaced `s`, each row divided by `e^(−s)` as in the first cut, so
//!   that the relative error is what is fitted. Levenberg–Marquardt starts from the first cut's
//!   log-spaced widths.
//! - `MGE_HOLED_EXP`: P02.T12.b's construction on the refit's widths and samples, in the same
//!   weighting. It is `MGE_EXP`'s weights less the non-negative fit of the deficit
//!   `e^(−s) (1 − e^(−x ÷ s))`, `x` the sim's [`THIN_DISC_HOLE_LENGTHS`], so its widths are
//!   `MGE_EXP`'s bit for bit and its weights are signed.
//! - `MGE_BAR`: the first cut's profile and mass ([`bar_profile`], [`bar_mass`]) at evenly spaced
//!   `u`, in absolute error, with the first cut's row for the total mass and one row for the mass
//!   inside each of a set of edges, in relative error. Plan 02's R14 rules out a better basis than
//!   non-negative centred Gaussians, so the refit keeps that basis. Least squares trades the three
//!   figures against each other, and no fixed weighting meets all three (3% of the central value,
//!   0.3% of the mass, 5% of the mass inside any edge). So the bar runs in rounds. Each round
//!   polishes the widths in least squares, from the first cut's widths and then from the last
//!   round's. It then solves the weights for the least largest error, each row scaled by its
//!   figure's tolerance, by Lawson's algorithm (1961). Each later round weights its polish by the
//!   row weights of the last minimax solve. The round with the least largest scaled error is kept.
//!
//! Every parameter is read from the manifest. The table's shapes fix four of them, `widths`,
//! `bar_width`, `bar_level` and `bar_end`: the number of terms, and the bar's nominal shape that
//! [`bar_profile`] is written for. The refit reads them and refuses any other value. The figures
//! ([`figures`]) are measured as plan 02's table tests measure them, at fixed points, whatever the
//! manifest says.

use hyperion_sim::galaxy::fields::disc::{THIN_DISC_HOLE_LENGTHS, hole_mass_fraction};
use hyperion_sim::math;

use super::boxy::{self, BoxyFigures, BoxyParams, BoxyRow, ROWS};
use super::{BAR_END, BAR_LEVEL, BAR_WIDTH, WIDTHS, bar_mass, bar_profile, expansion, index};
use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, ManifestParamError};
use crate::optimise::{least_squares, levenberg_marquardt, nnls};
use crate::task::{RunTaskError, TaskOutput};

/// The manifest key whose presence selects the refit; its one accepted value is [`METHOD`]. A
/// manifest without it is the first cut's.
pub(crate) const METHOD_KEY: &str = "method";

/// The refit's method name in its manifest.
pub const METHOD: &str = "free_widths";

/// Every key the one-dimensional refit reads; the boxy fit reads [`BoxyParams::KEYS`].
const KEYS: [&str; 21] = [
    METHOD_KEY,
    "widths",
    "exp_range",
    "exp_samples",
    "exp_start_widths",
    "iterations",
    "difference_step",
    "bar_width",
    "bar_level",
    "bar_end",
    "bar_reach",
    "bar_samples",
    "bar_mass_weight",
    "bar_cumulative_weight",
    "bar_cumulative_step",
    "bar_cumulative_pairs",
    "bar_cumulative_from",
    "bar_start_widths",
    "bar_minimax_scales",
    "minimax_iterations",
    "bar_rounds",
];

/// The most samples, edges or iterations a manifest may ask for: well past any fit here, and an
/// index that fits in `u32` ([`index`]).
const MAX_COUNT: usize = 1_000_000;

/// The refit's parameters, every one read from its manifest by [`RefitParams::from_manifest`],
/// which checks them.
#[derive(Debug, Clone, PartialEq)]
pub struct RefitParams {
    /// The range of `s` that `e^(−s)` is fitted on, in scale lengths.
    exp_range: (f64, f64),
    /// The number of log-spaced samples on `exp_range`, at least [`WIDTHS`].
    exp_samples: usize,
    /// The range of the log-spaced widths that the polish of `MGE_EXP` starts from, in scale
    /// lengths.
    exp_start_widths: (f64, f64),
    /// Levenberg–Marquardt's iterations, for `MGE_EXP` and for each of `MGE_BAR`'s rounds.
    iterations: u32,
    /// The forward-difference step of the Jacobian, in ln width, in (0, 10⁻²).
    difference_step: f64,
    /// The bar's samples are evenly spaced on `[0, bar_reach]`, in half-lengths.
    bar_reach: f64,
    /// The number of the bar's samples, at least [`WIDTHS`].
    bar_samples: usize,
    /// The weight of the bar's mass row in the first round's polish.
    bar_mass_weight: f64,
    /// The weight of each cumulative-mass row in the first round's polish.
    bar_cumulative_weight: f64,
    /// The profile is sampled every `bar_cumulative_step` half-lengths for the cumulative rows,
    /// whose edges are every second sample.
    bar_cumulative_step: f64,
    /// The number of Simpson pairs, so the last edge is at `2 × pairs × step`.
    bar_cumulative_pairs: usize,
    /// The edges below this are not held, in half-lengths.
    bar_cumulative_from: f64,
    /// The range of the log-spaced widths that the first polish of `MGE_BAR` starts from, in
    /// half-lengths.
    bar_start_widths: (f64, f64),
    /// The scales of the bar's minimax weights: of the profile's absolute error (in units of its
    /// central value), of the mass's relative error and of the cumulative mass's relative error.
    bar_minimax_scales: (f64, f64, f64),
    /// The least-squares solves of Lawson's algorithm for the bar's minimax weights, at least 1.
    minimax_iterations: u32,
    /// The bar's rounds of a polish of its widths and a minimax solve of its weights, at least 1.
    bar_rounds: u32,
    /// The boxy bulge's fit (P15.T3.b).
    boxy: BoxyParams,
}

/// A count of the manifest's, from `min` to [`MAX_COUNT`].
pub(super) fn count(
    manifest: &Manifest,
    key: &str,
    min: usize,
) -> Result<usize, ManifestParamError> {
    usize::try_from(manifest.u64(key)?)
        .ok()
        .filter(|n| (min..=MAX_COUNT).contains(n))
        .ok_or_else(|| ManifestParamError::new(key, &format!("a count from {min} to {MAX_COUNT}")))
}

/// An iteration count of the manifest's, from `min` to [`MAX_COUNT`].
pub(super) fn iterations(
    manifest: &Manifest,
    key: &str,
    min: usize,
) -> Result<u32, ManifestParamError> {
    u32::try_from(count(manifest, key, min)?)
        .map_err(|_| ManifestParamError::new(key, &format!("a count from {min} to {MAX_COUNT}")))
}

/// A finite number above 0.
pub(super) fn positive(manifest: &Manifest, key: &str) -> Result<f64, ManifestParamError> {
    let x = manifest.f64(key)?;
    if x.is_finite() && x > 0.0 {
        Ok(x)
    } else {
        Err(ManifestParamError::new(key, "a finite number above 0"))
    }
}

/// A finite number of at least 0.
pub(super) fn non_negative(manifest: &Manifest, key: &str) -> Result<f64, ManifestParamError> {
    let x = manifest.f64(key)?;
    if x.is_finite() && x >= 0.0 {
        Ok(x)
    } else {
        Err(ManifestParamError::new(
            key,
            "a finite number of at least 0",
        ))
    }
}

/// The array of numbers at `key`, each finite and above 0, if it has `n` of them.
pub(super) fn positives(manifest: &Manifest, key: &str, n: usize) -> Option<Vec<f64>> {
    let array = manifest.params().get(key)?.as_array()?;
    let values: Vec<f64> = array
        .iter()
        .filter_map(|v| v.as_float().filter(|x| x.is_finite() && *x > 0.0))
        .collect();
    (array.len() == n && values.len() == n).then_some(values)
}

/// A positive range `[lo, hi]`, `lo < hi`, written as a two-number array.
pub(super) fn range(manifest: &Manifest, key: &str) -> Result<(f64, f64), ManifestParamError> {
    match positives(manifest, key, 2).as_deref() {
        Some(&[lo, hi]) if lo < hi => Ok((lo, hi)),
        _ => Err(ManifestParamError::new(
            key,
            "an array of two positive numbers, ascending",
        )),
    }
}

/// Three positive numbers, written as an array.
fn three(manifest: &Manifest, key: &str) -> Result<(f64, f64, f64), ManifestParamError> {
    match positives(manifest, key, 3).as_deref() {
        Some(&[a, b, c]) => Ok((a, b, c)),
        _ => Err(ManifestParamError::new(
            key,
            "an array of three positive numbers",
        )),
    }
}

/// `key` must hold exactly `value`, which the table's shape fixes.
fn fixed(manifest: &Manifest, key: &str, value: f64, why: &str) -> Result<(), ManifestParamError> {
    // An exact match is meant: the profile is written for these values and no others.
    if manifest.f64(key)?.total_cmp(&value).is_eq() {
        Ok(())
    } else {
        Err(ManifestParamError::new(key, &format!("{value:?}, {why}")))
    }
}

impl RefitParams {
    /// The parameters of the refit's manifest.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] naming the first key that is missing, extra, of the wrong kind or
    /// out of its range, a `method` other than [`METHOD`], or a key the table's shape fixes at
    /// another value.
    pub fn from_manifest(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        if manifest.str(METHOD_KEY)? != METHOD {
            return Err(ManifestParamError::new(
                METHOD_KEY,
                &format!("\"{METHOD}\" (plan 15's refit), or absent for the first cut"),
            ));
        }
        if let Some(extra) = manifest
            .params()
            .keys()
            .find(|k| !KEYS.contains(&k.as_str()) && !BoxyParams::KEYS.contains(&k.as_str()))
        {
            return Err(ManifestParamError::new(
                extra,
                "absent: the refit reads no such key",
            ));
        }
        if count(manifest, "widths", 0)? != WIDTHS {
            return Err(ManifestParamError::new(
                "widths",
                &format!("{WIDTHS}, the tables' number of terms"),
            ));
        }
        let shape = "the bar's nominal shape, which `bar_profile` is written for";
        fixed(manifest, "bar_width", BAR_WIDTH, shape)?;
        fixed(manifest, "bar_level", BAR_LEVEL, shape)?;
        fixed(manifest, "bar_end", BAR_END, shape)?;
        let difference_step = manifest.f64("difference_step")?;
        if !(difference_step > 0.0 && difference_step < 1e-2) {
            return Err(ManifestParamError::new(
                "difference_step",
                "a step in ln width above 0 and below 0.01",
            ));
        }
        Ok(Self {
            exp_range: range(manifest, "exp_range")?,
            exp_samples: count(manifest, "exp_samples", WIDTHS)?,
            exp_start_widths: range(manifest, "exp_start_widths")?,
            iterations: iterations(manifest, "iterations", 0)?,
            difference_step,
            bar_reach: positive(manifest, "bar_reach")?,
            bar_samples: count(manifest, "bar_samples", WIDTHS)?,
            bar_mass_weight: non_negative(manifest, "bar_mass_weight")?,
            bar_cumulative_weight: non_negative(manifest, "bar_cumulative_weight")?,
            bar_cumulative_step: positive(manifest, "bar_cumulative_step")?,
            bar_cumulative_pairs: count(manifest, "bar_cumulative_pairs", 1)?,
            bar_cumulative_from: non_negative(manifest, "bar_cumulative_from")?,
            bar_start_widths: range(manifest, "bar_start_widths")?,
            bar_minimax_scales: three(manifest, "bar_minimax_scales")?,
            minimax_iterations: iterations(manifest, "minimax_iterations", 1)?,
            bar_rounds: iterations(manifest, "bar_rounds", 1)?,
            boxy: BoxyParams::from_manifest(manifest)?,
        })
    }

    /// The boxy bulge's parameters.
    #[must_use]
    pub fn boxy(&self) -> &BoxyParams {
        &self.boxy
    }

    /// Levenberg–Marquardt's iterations for each polish.
    #[must_use]
    pub fn iterations(&self) -> u32 {
        self.iterations
    }
}

/// `n` points log-spaced from `lo` to `hi`, both exactly included, as [`super::log_spaced`] places
/// them; `n` is at least 2.
#[must_use]
fn log_points(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let step = (math::ln(hi) - math::ln(lo)) / index(n - 1);
    let mut points: Vec<f64> = (0..n)
        .map(|i| math::exp(math::ln(lo) + step * index(i)))
        .collect();
    points[0] = lo;
    points[n - 1] = hi;
    points
}

/// The refitted expansions, each `[(weight, width); WIDTHS]` with widths strictly ascending.
#[derive(Debug, Clone, PartialEq)]
pub struct Refit {
    /// `e^(−s)`, its widths in scale lengths.
    pub exp: [(f64, f64); WIDTHS],
    /// `e^(−x ÷ s − s)`, the holed disc's radial profile, on `exp`'s widths; signed weights.
    pub holed_exp: [(f64, f64); WIDTHS],
    /// The long bar's azimuthally averaged surface density, 1 at the centre, its widths in
    /// half-lengths.
    pub bar: [(f64, f64); WIDTHS],
    /// The Levenberg–Marquardt steps the polishes took: `MGE_EXP`'s, then `MGE_BAR`'s over all its
    /// rounds.
    pub steps: [u32; 2],
}

/// A one-dimensional problem: the design matrix for given widths and its right-hand side.
struct Problem<F: Fn(&[f64; WIDTHS]) -> Vec<[f64; WIDTHS]>> {
    rows: F,
    rhs: Vec<f64>,
}

impl<F: Fn(&[f64; WIDTHS]) -> Vec<[f64; WIDTHS]>> Problem<F> {
    /// The widths `e^θ`.
    #[must_use]
    fn widths(theta: &[f64]) -> [f64; WIDTHS] {
        let mut widths = [0.0; WIDTHS];
        for (w, &t) in widths.iter_mut().zip(theta) {
            *w = math::exp(t);
        }
        widths
    }

    /// The residuals that the non-negative weights for widths `e^θ` leave, into `out`, and those
    /// weights.
    fn residuals(&self, theta: &[f64], out: &mut [f64]) -> Vec<f64> {
        let rows = (self.rows)(&Self::widths(theta));
        let weights = nnls(&rows, &self.rhs);
        Self::leave(&rows, &self.rhs, &weights, out);
        weights
    }

    /// `rows · weights − rhs` into `out`.
    fn leave(rows: &[[f64; WIDTHS]], rhs: &[f64], weights: &[f64], out: &mut [f64]) {
        for ((o, row), &b) in out.iter_mut().zip(rows).zip(rhs) {
            *o = row
                .iter()
                .zip(weights)
                .fold(0.0, |sum, (&a, &w)| sum + a * w)
                - b;
        }
    }

    /// The residuals for widths `e^θ` of the least-squares weights on the columns `free` alone
    /// (the others 0), into `out`; the full non-negative solve if those columns are dependent.
    fn residuals_on(&self, theta: &[f64], free: &[usize], out: &mut [f64]) {
        let rows = (self.rows)(&Self::widths(theta));
        let restricted: Vec<Vec<f64>> = rows
            .iter()
            .map(|row| free.iter().map(|&k| row[k]).collect())
            .collect();
        let solved = if free.is_empty() {
            None
        } else {
            least_squares(&restricted, &self.rhs)
        };
        let weights = match solved {
            Some(coefficients) => {
                let mut weights = vec![0.0; WIDTHS];
                for (&k, c) in free.iter().zip(coefficients) {
                    weights[k] = c;
                }
                weights
            }
            None => nnls(&rows, &self.rhs),
        };
        Self::leave(&rows, &self.rhs, &weights, out);
    }

    /// Polishes `start`'s widths in ln width, then solves the weights on the sorted widths: the
    /// table and the steps taken.
    #[must_use]
    fn polish(
        &self,
        start: [f64; WIDTHS],
        iterations: u32,
        step: f64,
    ) -> ([(f64, f64); WIDTHS], u32) {
        let m = self.rhs.len();
        let model = |theta: &[f64], r: &mut [f64], jacobian: Option<&mut [f64]>| {
            let weights = self.residuals(theta, r);
            if let Some(jacobian) = jacobian {
                let free: Vec<usize> = (0..WIDTHS).filter(|&k| weights[k] > 0.0).collect();
                let mut shifted = theta.to_vec();
                for (k, column) in jacobian.chunks_exact_mut(m).enumerate() {
                    shifted[k] = theta[k] + step;
                    let h = shifted[k] - theta[k];
                    self.residuals_on(&shifted, &free, column);
                    for (c, &base) in column.iter_mut().zip(r.iter()) {
                        *c = (*c - base) / h;
                    }
                    shifted[k] = theta[k];
                }
            }
        };
        let theta: Vec<f64> = start.iter().map(|&s| math::ln(s)).collect();
        let fit = levenberg_marquardt(
            model,
            &theta,
            &[f64::NEG_INFINITY; WIDTHS],
            &[f64::INFINITY; WIDTHS],
            m,
            iterations,
        );
        let mut widths = Self::widths(&fit.params);
        widths.sort_by(f64::total_cmp);
        let weights = nnls(&(self.rows)(&widths), &self.rhs);
        let mut table = [(0.0, 0.0); WIDTHS];
        for (entry, (w, s)) in table.iter_mut().zip(weights.into_iter().zip(widths)) {
            *entry = (w, s);
        }
        (table, fit.steps)
    }
}

/// The largest of `errors`, a NaN counting as larger than any number.
#[must_use]
pub(super) fn largest(errors: &[f64]) -> f64 {
    errors
        .iter()
        .fold(0.0, |w: f64, &e| if e.is_nan() || e > w { e } else { w })
}

/// The non-negative weights that make the largest scaled residual `|rowsᵢ · w − rhsᵢ| ÷ scaleᵢ`
/// least, by Lawson's algorithm (1961, thesis, UCLA; Rice and Usow 1968, Math. Comp. 22, 118).
///
/// It solves a weighted least-squares problem, here by [`nnls`], with every row's weight `vᵢ`
/// from 1 ÷ m. After each solve it multiplies each weight by its row's scaled residual and
/// renormalises them to sum to 1, which moves the weight onto the rows where the error is
/// largest. Its convergence to the minimax solution is proved for unconstrained linear
/// approximation on a Haar set; with the non-negative solve it is a heuristic, so the result is
/// the iterate with the least largest scaled residual, the first among equals, over `iterations`
/// solves. A NaN residual never wins.
#[must_use]
fn lawson<R: AsRef<[f64]>>(rows: &[R], rhs: &[f64], scale: &[f64], iterations: u32) -> Minimax {
    let mut v = vec![1.0 / index(rows.len()); rows.len()];
    let columns = rows.first().map_or(0, |r| r.as_ref().len());
    let mut best = Minimax {
        weights: vec![0.0; columns],
        largest: f64::INFINITY,
        row_weights: v.clone(),
    };
    for _ in 0..iterations {
        let factor: Vec<f64> = v.iter().zip(scale).map(|(&vi, &s)| vi.sqrt() / s).collect();
        let weighted: Vec<Vec<f64>> = rows
            .iter()
            .zip(&factor)
            .map(|(row, &f)| row.as_ref().iter().map(|a| a * f).collect())
            .collect();
        let weighted_rhs: Vec<f64> = rhs.iter().zip(&factor).map(|(&b, &f)| b * f).collect();
        let weights = nnls(&weighted, &weighted_rhs);
        let errors: Vec<f64> = rows
            .iter()
            .zip(rhs)
            .zip(scale)
            .map(|((row, &b), &s)| {
                (row.as_ref()
                    .iter()
                    .zip(&weights)
                    .fold(0.0, |sum, (&a, &w)| sum + a * w)
                    - b)
                    .abs()
                    / s
            })
            .collect();
        let worst = largest(&errors);
        if worst < best.largest {
            best = Minimax {
                weights,
                largest: worst,
                row_weights: v.clone(),
            };
        }
        let total = v
            .iter()
            .zip(&errors)
            .fold(0.0, |sum, (&vi, &e)| sum + vi * e);
        if total.is_nan() || total <= 0.0 {
            break;
        }
        for (vi, &e) in v.iter_mut().zip(&errors) {
            *vi *= e / total;
        }
    }
    best
}

/// What [`lawson`] found: the weights, their largest scaled residual, and the rows' weights of
/// the least-squares problem that gave them.
struct Minimax {
    weights: Vec<f64>,
    largest: f64,
    row_weights: Vec<f64>,
}

/// `e^(−s)`'s rows at `samples`, each divided by `e^(−s)`: `exp(s − s² ÷ 2σ²)`.
#[must_use]
fn exp_rows(samples: &[f64], widths: &[f64; WIDTHS]) -> Vec<[f64; WIDTHS]> {
    samples
        .iter()
        .map(|&s| widths.map(|sigma| math::exp(s - s * s / (2.0 * sigma * sigma))))
        .collect()
}

/// Plan 02's measure of the bar (`mge_bar_reproduces_the_bar_profile`), which the figures use:
/// the profile every 0.01 half-lengths out to 2.5, the mass inside each even sample by Simpson's
/// rule on them, and the edges from 0.05 on.
const PLAN_02_STEP: f64 = 0.01;
const PLAN_02_PAIRS: usize = 125;
const PLAN_02_FROM: f64 = 0.05;

/// The mass of the bar's profile inside each even sample `u` = 2 step, 4 step, …, 2 × pairs ×
/// step (half-lengths), `2π ∫ Σ̄ u du` by Simpson's rule on its samples every `step`, as plan
/// 02's `mge_bar_reproduces_the_bar_profile` takes it: `(edge, mass)`.
#[must_use]
fn bar_inside(step: f64, pairs: usize) -> Vec<(f64, f64)> {
    let two_pi = 2.0 * core::f64::consts::PI;
    let at = |i: usize| step * index(i) * bar_profile(step * index(i));
    let mut inside = 0.0;
    (0..pairs)
        .map(|pair| {
            let i = 2 * pair;
            inside += two_pi * step / 3.0 * (at(i) + 4.0 * at(i + 1) + at(i + 2));
            (step * index(i + 2), inside)
        })
        .collect()
}

/// The mass inside `u` of a Gaussian of unit amplitude and width `sigma` in the plane:
/// `2πσ² (1 − e^(−u² ÷ 2σ²))`.
#[must_use]
fn gaussian_inside(sigma: f64, u: f64) -> f64 {
    let two_pi = 2.0 * core::f64::consts::PI;
    -two_pi * sigma * sigma * math::exp_m1(-u * u / (2.0 * sigma * sigma))
}

/// The bar's design, unweighted: its profile's samples, its mass row and its cumulative-mass
/// rows.
struct BarRows<'a> {
    samples: &'a [f64],
    /// `(edge, mass inside)` of each held edge.
    cumulative: &'a [(f64, f64)],
}

impl BarRows<'_> {
    /// The rows for `widths`: the profile at each sample in absolute error, the mass in relative
    /// error, then the mass inside each held edge in relative error.
    #[must_use]
    fn rows(&self, widths: &[f64; WIDTHS]) -> Vec<[f64; WIDTHS]> {
        let two_pi = 2.0 * core::f64::consts::PI;
        let mut rows: Vec<[f64; WIDTHS]> = self
            .samples
            .iter()
            .map(|&u| widths.map(|sigma| math::exp(-u * u / (2.0 * sigma * sigma))))
            .collect();
        rows.push(widths.map(|sigma| two_pi * sigma * sigma / bar_mass()));
        for &(edge, inside) in self.cumulative {
            rows.push(widths.map(|sigma| gaussian_inside(sigma, edge) / inside));
        }
        rows
    }

    /// The right-hand side for [`BarRows::rows`].
    #[must_use]
    fn rhs(&self) -> Vec<f64> {
        let mut rhs: Vec<f64> = self.samples.iter().map(|&u| bar_profile(u)).collect();
        rhs.push(1.0);
        rhs.extend(self.cumulative.iter().map(|_| 1.0));
        rhs
    }
}

/// Runs the refit. Single-threaded, with no randomness, so the result is the same on every run
/// and platform.
#[must_use]
pub fn refit(params: &RefitParams) -> Refit {
    let samples = log_points(params.exp_range.0, params.exp_range.1, params.exp_samples);
    let exp_problem = Problem {
        rows: |widths: &[f64; WIDTHS]| exp_rows(&samples, widths),
        rhs: vec![1.0; samples.len()],
    };
    let (exp, exp_steps) = exp_problem.polish(
        super::log_spaced(params.exp_start_widths.0, params.exp_start_widths.1),
        params.iterations,
        params.difference_step,
    );
    // P02.T12.b's deficit, on the polished widths and the same samples and weighting.
    let widths = exp.map(|(_, sigma)| sigma);
    let deficit_rhs: Vec<f64> = samples
        .iter()
        .map(|&s| -math::exp_m1(-THIN_DISC_HOLE_LENGTHS / s))
        .collect();
    let deficit = nnls(&exp_rows(&samples, &widths), &deficit_rhs);
    let mut holed_exp = [(0.0, 0.0); WIDTHS];
    for ((entry, &(w, sigma)), d) in holed_exp.iter_mut().zip(&exp).zip(deficit) {
        *entry = (w - d, sigma);
    }
    let (bar, bar_steps) = refit_bar(params);
    Refit {
        exp,
        holed_exp,
        bar,
        steps: [exp_steps, bar_steps],
    }
}

/// `MGE_BAR` in rounds (module documentation): the table and the polishes' steps.
#[must_use]
fn refit_bar(params: &RefitParams) -> ([(f64, f64); WIDTHS], u32) {
    let step = params.bar_reach / index(params.bar_samples - 1);
    let bar_samples: Vec<f64> = (0..params.bar_samples).map(|i| step * index(i)).collect();
    let held: Vec<(f64, f64)> = bar_inside(params.bar_cumulative_step, params.bar_cumulative_pairs)
        .into_iter()
        .filter(|&(edge, _)| edge >= params.bar_cumulative_from)
        .collect();
    let design = BarRows {
        samples: &bar_samples,
        cumulative: &held,
    };
    let bar_rhs = design.rhs();
    let (profile, mass, cumulative) = params.bar_minimax_scales;
    let scale: Vec<f64> = bar_samples
        .iter()
        .map(|_| profile)
        .chain([mass])
        .chain(held.iter().map(|_| cumulative))
        .collect();
    // Each round polishes the widths in least squares, its rows weighted as the manifest says in
    // the first round and by the last minimax solve's row weights over their scales after, then
    // solves the minimax weights on them. The round with the least largest scaled error wins.
    let mut factor: Vec<f64> = bar_samples
        .iter()
        .map(|_| 1.0)
        .chain([params.bar_mass_weight])
        .chain(held.iter().map(|_| params.bar_cumulative_weight))
        .collect();
    let mut widths = super::log_spaced(params.bar_start_widths.0, params.bar_start_widths.1);
    let mut best: Option<(f64, [(f64, f64); WIDTHS])> = None;
    let mut bar_steps = 0;
    for _ in 0..params.bar_rounds {
        let problem = Problem {
            rows: |w: &[f64; WIDTHS]| {
                let mut rows = design.rows(w);
                for (row, &f) in rows.iter_mut().zip(&factor) {
                    *row = row.map(|a| a * f);
                }
                rows
            },
            rhs: bar_rhs.iter().zip(&factor).map(|(&b, &f)| b * f).collect(),
        };
        let (polished, steps) = problem.polish(widths, params.iterations, params.difference_step);
        bar_steps += steps;
        widths = polished.map(|(_, sigma)| sigma);
        let minimax = lawson(
            &design.rows(&widths),
            &bar_rhs,
            &scale,
            params.minimax_iterations,
        );
        if best.is_none_or(|(least, _)| minimax.largest < least) {
            let mut weights = [0.0; WIDTHS];
            weights.copy_from_slice(&minimax.weights);
            best = Some((minimax.largest, super::table(weights, widths)));
        }
        factor = minimax
            .row_weights
            .iter()
            .zip(&scale)
            .map(|(&v, &s)| v.sqrt() / s)
            .collect();
    }
    let (_, bar) = best.expect("the manifest asks for at least one round");
    (bar, bar_steps)
}

/// How closely the refit follows its profiles: plan 15's and plan 02's figures for the
/// one-dimensional tables (P15.T3.a's acceptance; the sim's `tests/galaxy_potential.rs`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RefitFigures {
    /// The largest `|expansion ÷ e^(−s) − 1|` of `MGE_EXP` at 4,001 log-spaced `s` on 0.05–10.
    pub exp_worst_relative: f64,
    /// The same on 0.05–8, plan 02's range.
    pub exp_worst_relative_to_8: f64,
    /// `Σ w (2π)^(3÷2) σ³ ÷ 8π − 1`: the spherical exponential's mass.
    pub exp_spherical_mass: f64,
    /// The largest `|expansion − e^(−x ÷ s − s)| ÷ e^(−s)` of `MGE_HOLED_EXP` on 0.05–8.
    pub holed_worst: f64,
    /// `Σ w σ² ÷ hole_mass_fraction(x) − 1`: the holed disc's surface mass.
    pub holed_mass: f64,
    /// The largest `|expansion − profile|` of `MGE_BAR` at `u` = 0, 0.01, …, 2.5 half-lengths, in
    /// units of the central value.
    pub bar_worst: f64,
    /// The `u` of [`RefitFigures::bar_worst`], in half-lengths.
    pub bar_worst_at: f64,
    /// `Σ w 2π σ² ÷ bar_mass() − 1`.
    pub bar_mass: f64,
    /// The largest relative error of the mass inside `u`, for `u` = 0.06, 0.08, …, 2.5
    /// half-lengths, the mass of the profile by Simpson's rule on its samples at 0.01, as plan
    /// 02's `mge_bar_reproduces_the_bar_profile` measures it.
    pub bar_cumulative_worst: f64,
    /// The `u` of [`RefitFigures::bar_cumulative_worst`], in half-lengths.
    pub bar_cumulative_worst_at: f64,
}

/// `s` from 0.05 to `hi` at 4,001 log-spaced points, as plan 02's table tests sample it.
fn test_points(hi: f64) -> impl Iterator<Item = f64> {
    (0..=4_000).map(move |i| 0.05 * math::exp(f64::from(i) / 4_000.0 * math::ln(hi / 0.05)))
}

/// The largest `|error|` of `(place, error)` pairs and its place, the first among equals; a NaN
/// error counts as larger than any number.
#[must_use]
fn worst(errors: impl Iterator<Item = (f64, f64)>) -> (f64, f64) {
    errors.fold((0.0, 0.0), |(w, at), (place, error)| {
        let e = error.abs();
        if e.is_nan() || e > w {
            (e, place)
        } else {
            (w, at)
        }
    })
}

/// `(x, f(x))` over `points`.
fn errors(
    points: impl Iterator<Item = f64>,
    f: impl Fn(f64) -> f64,
) -> impl Iterator<Item = (f64, f64)> {
    points.map(move |x| (x, f(x)))
}

/// The acceptance figures of `refit`.
#[must_use]
pub fn figures(refit: &Refit) -> RefitFigures {
    let relative = |s: f64| expansion(&refit.exp, s) / math::exp(-s) - 1.0;
    let two_pi = 2.0 * core::f64::consts::PI;
    let spherical = refit.exp.iter().fold(0.0, |sum, &(w, s)| {
        sum + w * two_pi * two_pi.sqrt() * s * s * s
    });
    let x = THIN_DISC_HOLE_LENGTHS;
    let holed = |s: f64| (expansion(&refit.holed_exp, s) - math::exp(-x / s - s)) / math::exp(-s);
    let holed_mass = refit
        .holed_exp
        .iter()
        .fold(0.0, |sum, &(w, sigma)| sum + w * sigma * sigma);
    let (bar_worst, bar_worst_at) = worst(errors(
        (0..=2 * PLAN_02_PAIRS).map(|i| PLAN_02_STEP * index(i)),
        |u| expansion(&refit.bar, u) - bar_profile(u),
    ));
    let bar_total = refit
        .bar
        .iter()
        .fold(0.0, |sum, &(w, s)| sum + w * two_pi * s * s);
    let (bar_cumulative_worst, bar_cumulative_worst_at) = worst(
        bar_inside(PLAN_02_STEP, PLAN_02_PAIRS)
            .into_iter()
            .filter(|&(edge, _)| edge >= PLAN_02_FROM)
            .map(|(edge, inside)| {
                let expanded = refit.bar.iter().fold(0.0, |sum, &(w, s)| {
                    sum + w * two_pi * s * s * (1.0 - math::exp(-edge * edge / (2.0 * s * s)))
                });
                (edge, expanded / inside - 1.0)
            }),
    );
    RefitFigures {
        exp_worst_relative: worst(errors(test_points(10.0), relative)).0,
        exp_worst_relative_to_8: worst(errors(test_points(8.0), relative)).0,
        exp_spherical_mass: spherical / (4.0 * two_pi) - 1.0,
        holed_worst: worst(errors(test_points(8.0), holed)).0,
        holed_mass: holed_mass / hole_mass_fraction(x) - 1.0,
        bar_worst,
        bar_worst_at,
        bar_mass: bar_total / bar_mass() - 1.0,
        bar_cumulative_worst,
        bar_cumulative_worst_at,
    }
}

/// The figures as the header's acceptance line states them.
#[must_use]
pub(crate) fn acceptance(f: &RefitFigures) -> String {
    format!(
        "`MGE_EXP` within {:.3}% of e^(−s) over s = 0.05–10 and its spherical mass within {:.4}%; \
         `MGE_HOLED_EXP` within {:.3}% of e^(−s) over 0.05–8 and its mass fraction within {:.1e}; \
         `MGE_BAR` within {:.2}% of its central value (worst at u = {:.2}), its mass within \
         {:.3}% and its cumulative mass from u = 0.05 to 2.5 within {:.2}% (worst at u = {:.2}); \
         the thin disc's speed against Freeman (1970) is held to 0.5% by `tests/mge.rs`",
        100.0 * f.exp_worst_relative,
        100.0 * f.exp_spherical_mass.abs(),
        100.0 * f.holed_worst,
        f.holed_mass.abs(),
        100.0 * f.bar_worst,
        f.bar_worst_at,
        100.0 * f.bar_mass.abs(),
        100.0 * f.bar_cumulative_worst,
        f.bar_cumulative_worst_at,
    )
}

/// Whether every figure meets P15.T3.a's criterion: `MGE_EXP` under 0.5% on 0.05–10 and its
/// spherical mass within 0.05%; `MGE_BAR` within 3% of its central value, its mass within 0.3%
/// and its cumulative mass within 5%; `MGE_HOLED_EXP` within plan 02's 1% and 10⁻⁵. Freeman's
/// figure is not among them: its Bessel functions live in the task's tests, which hold it.
#[must_use]
pub fn passes(f: &RefitFigures) -> bool {
    f.exp_worst_relative < 0.005
        && f.exp_spherical_mass.abs() < 5e-4
        && f.holed_worst < 0.01
        && f.holed_mass.abs() < 1e-5
        && f.bar_worst < 0.03
        && f.bar_mass.abs() < 3e-3
        && f.bar_cumulative_worst < 0.05
}

/// The notes of the refit's table: its method and inputs.
#[must_use]
pub(crate) fn notes(params: &RefitParams) -> String {
    format!(
        "\
Each one-dimensional table is `[(weight, width); {WIDTHS}]`, widths ascending, and stands for the
profile `Σ weight × exp(−x² ÷ 2 width²)`. The widths are free (Cappellari 2002, MNRAS 333,
400). Levenberg–Marquardt moves their logarithms, at most {iterations} iterations, its Jacobian by
forward differences of {step} in ln width. At each point it evaluates, the weights are the
non-negative least-squares solution (Lawson and Hanson 1974) for those widths. The weights are
solved once more on the sorted widths. Inputs:

- [`MGE_EXP`]: `e^(−s)` at {samples} log-spaced `s` from {} to {}, each row weighted by
  `1 ÷ e^(−s)` so that the relative error is what is fitted. Started from log-spaced widths
  {} to {}.
- [`MGE_HOLED_EXP`]: `e^(−x ÷ s − s)` with `x = {hole}`, the thin disc's central hole in scale
  lengths: [`MGE_EXP`]'s weights less those of the deficit `e^(−s) (1 − e^(−x ÷ s))`, fitted
  on the same widths and samples in the same weighting, so its weights are signed.
- [`MGE_BAR`]: the azimuthal average of the long bar's surface density `L(|x|) exp(−y² ÷ 2w²)`,
  with `w = {}` and `L` level to {} with a Gaussian end of width {}, all in half-lengths: at
  {bar_samples} evenly spaced `u` from 0 to {}, in absolute error, with a row for the total mass
  and one in relative error for the mass inside each edge from {from} to {edge}, every {pair},
  that mass by Simpson's rule on the profile every {cstep}. In each of {rounds} rounds the widths
  are polished, from log-spaced widths {} to {} in the first round and from the last round's
  after it. The mass row is weighted {} and the cumulative rows {cumulative} in the first round,
  and the rows by the last minimax solve's weights in the later ones. Then Lawson's algorithm
  (1961), over {minimax} non-negative solves, makes the largest error least, the profile's scaled
  by {}, the mass's by {} and the cumulative mass's by {}. The round with the least such error is
  kept.",
        params.exp_range.0,
        params.exp_range.1,
        params.exp_start_widths.0,
        params.exp_start_widths.1,
        BAR_WIDTH,
        BAR_LEVEL,
        BAR_END,
        params.bar_reach,
        params.bar_start_widths.0,
        params.bar_start_widths.1,
        params.bar_mass_weight,
        params.bar_minimax_scales.0,
        params.bar_minimax_scales.1,
        params.bar_minimax_scales.2,
        from = params.bar_cumulative_from,
        edge = 2.0 * params.bar_cumulative_step * index(params.bar_cumulative_pairs),
        pair = 2.0 * params.bar_cumulative_step,
        cstep = params.bar_cumulative_step,
        rounds = params.bar_rounds,
        minimax = params.minimax_iterations,
        iterations = params.iterations,
        cumulative = literal(params.bar_cumulative_weight),
        step = literal(params.difference_step),
        samples = params.exp_samples,
        bar_samples = params.bar_samples,
        hole = literal(THIN_DISC_HOLE_LENGTHS),
    )
}

/// The refit's table: its summary, its notes and its items, in the order T3.c commits them, after
/// the `use` line of `MGE_BOXY`'s row type.
#[must_use]
pub(crate) fn render(
    params: &RefitParams,
    refit: &Refit,
    boxy_rows: &[BoxyRow; ROWS],
) -> RustTable {
    let rows = |table: &[(f64, f64); WIDTHS]| table.iter().map(|&(w, s)| vec![w, s]).collect();
    let doc = |lines: &[&str]| lines.iter().map(|&l| l.to_owned()).collect();
    RustTable {
        summary: vec![
            "Gaussian expansions of the profiles the galaxy's potential is built from (plan 02, Design"
                .to_owned(),
            "note 6).".to_owned(),
        ],
        notes: notes(params)
            .lines()
            .chain(boxy::notes(&params.boxy).lines())
            .map(str::to_owned)
            .collect(),
        items: vec![
            TableItem::Source(boxy::USE_LINE.to_owned()),
            TableItem::Tuples {
                name: "MGE_EXP".to_owned(),
                doc: doc(&["`e^(−s) ≈ Σ weight × exp(−s² ÷ 2 width²)` for `s ≥ 0`."]),
                rows: rows(&refit.exp),
            },
            TableItem::Tuples {
                name: "MGE_HOLED_EXP".to_owned(),
                doc: doc(&[
                    "`e^(−x ÷ s − s) ≈ Σ weight × exp(−s² ÷ 2 width²)` for `s ≥ 0`, with `x` the thin disc's",
                    "hole in scale lengths; the weights are signed.",
                ]),
                rows: rows(&refit.holed_exp),
            },
            TableItem::Tuples {
                name: "MGE_BAR".to_owned(),
                doc: doc(&[
                    "The long bar's azimuthally averaged surface density, 1 at the centre: at `u`",
                    "half-lengths it is `≈ Σ weight × exp(−u² ÷ 2 width²)`, the widths in half-lengths.",
                ]),
                rows: rows(&refit.bar),
            },
            TableItem::Source(boxy::item(boxy_rows)),
        ],
    }
}

/// The refit's run on `manifest`: its table, `@generated` if every figure passes ([`passes`] and
/// [`BoxyFigures::passes`]), else provisional by the first subtask whose figures miss, P15.T3.a or
/// P15.T3.b, with the figures in its acceptance line.
///
/// # Errors
///
/// [`RunTaskError::Param`] as [`RefitParams::from_manifest`].
pub(crate) fn run(manifest: &Manifest) -> Result<TaskOutput, RunTaskError> {
    let params = RefitParams::from_manifest(manifest)?;
    let tables = refit(&params);
    let f = figures(&tables);
    let boxy_rows = boxy::fit(&tables.exp, &params.boxy);
    let boxy_figures = boxy_rows.each_ref().map(boxy::figures);
    let provisional = if !passes(&f) {
        Some("P15.T3.a")
    } else if !boxy_figures.iter().all(BoxyFigures::passes) {
        Some("P15.T3.b")
    } else {
        None
    };
    Ok(TaskOutput {
        table: render(&params, &tables, &boxy_rows),
        source: "the profiles of plan 02's mass model (Design note 6), expanded as sums of \
                 Gaussians after Emsellem, Monnet and Bacon (1994, A&A 285, 723), the widths \
                 free after Cappellari (2002, MNRAS 333, 400)"
            .to_owned(),
        acceptance: format!("{}; {}", acceptance(&f), boxy::acceptance(&boxy_figures)),
        provisional,
    })
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// The points the refit samples are where the first cut's `log_spaced` puts them, bit for bit.
    #[test]
    fn log_points_are_the_first_cut_s_log_spacing() {
        let first_cut: [f64; WIDTHS] = super::super::log_spaced(0.04, 3.5);
        for (&got, want) in log_points(0.04, 3.5, WIDTHS).iter().zip(first_cut) {
            assert_same_bits(got, want);
        }
    }

    /// The best constant through 1, 2 and 4 in the largest error is their midrange, 2.5, which
    /// misses by 1.5; least squares would give their mean, 7 ÷ 3.
    #[test]
    fn lawson_finds_the_minimax_constant() {
        let found = lawson(&[[1.0], [1.0], [1.0]], &[1.0, 2.0, 4.0], &[1.0; 3], 200);
        assert!(
            (found.weights[0] - 2.5).abs() < 1e-6,
            "{}",
            found.weights[0]
        );
        assert!((found.largest - 1.5).abs() < 1e-6, "{}", found.largest);
    }

    /// Its weights stay non-negative: against targets below zero the best weight is 0.
    #[test]
    fn lawson_keeps_its_weights_non_negative() {
        let found = lawson(&[[1.0], [1.0]], &[-1.0, -2.0], &[1.0; 2], 20);
        assert!(
            found.weights[0].total_cmp(&0.0).is_eq(),
            "{}",
            found.weights[0]
        );
        assert!((found.largest - 2.0).abs() < 1e-15, "{}", found.largest);
    }

    #[test]
    fn the_largest_error_counts_a_nan_as_the_largest() {
        assert!(largest(&[0.1, f64::NAN, 0.3]).is_nan());
        assert!(
            worst([(1.0, 0.2), (2.0, f64::NAN), (3.0, 0.5)].into_iter())
                .0
                .is_nan()
        );
        assert!((largest(&[0.1, 0.3, 0.2]) - 0.3).abs() < 1e-15);
    }
}
