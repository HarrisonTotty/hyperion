//! The disc-born forms, with the young bins regularised (plan 15, P15.T6.c).
//!
//! Each of the 56 thin-disc classes' (R, |z|) histograms of bound members inside the cube is
//! fitted with a flared layer plus a flattened cored power law ([`FlaredLayerParams`],
//! [`CoredPowerLawParams`]) by minimising the misplaced share ([`cells`](super::cells)),
//! Nelder–Mead from a fixed start ([`nelder_mead`](super::nelder_mead)). The scratch fits behind
//! the brainstorm ran to unconstrained directions where the spheroid carried little weight, so:
//!
//! - the parameters are boxed ([`BOXES`]): γ to [2, 8], a to [0.2, 12] `R_d`, q to [0.02, 1], β
//!   to [0.8, 3], `r_flare` to [1, 50] `R_d`, and, where the plan gives no box, `h_r` to [0.05,
//!   50] `R_d` and `h_0` to [10⁻⁴, 20] `R_d`;
//! - a spheroid fitted at under [`DisciplineSettings::drop_weight`] (0.05) of the weight is set to
//!   zero and the layer refitted alone;
//! - along each speed row the parameters of neighbouring age bins are tied by a quadratic penalty
//!   `λ Σ (ln θ − ln θ′)²` on their differences in log, by Gauss–Seidel sweeps along the row, with
//!   the largest λ of the manifest's ladder under which the weighted mean misplaced share rises by
//!   no more than [`DisciplineSettings::rise_limit`] (0.3 points).
//!
//! The classes' weights, for the weighted mean, are the manifest's speed-bin shares times the
//! thin disc's share of deaths in each age bin times the class's bound share inside the cube.

use std::num::NonZeroUsize;

use hyperion_sim::galaxy::displaced::forms::{CoredPowerLawParams, FlaredLayerParams};
use hyperion_sim::galaxy::displaced::{AGE_BINS, SPEED_BINS};
use hyperion_sim::math;

use super::cells::{
    RzCells, fastest_counts, map_in_order, noise_floor, normalise, shares, total_variation,
};
use super::histogram::{ClassHistogram, R_CELLS, Z_CELLS};
use super::nelder_mead::{Bounded, Stop, minimise};
use crate::parallel::BuildThreadPoolError;

/// The boxes of `[h_r, h_0, r_flare, β, a, q, γ, spheroid weight]`, lengths in `R_d` (plan 15,
/// P15.T6.c).
pub const BOXES: [Bounded; 8] = [
    Bounded::log(0.05, 50.0),
    Bounded::log(1e-4, 20.0),
    Bounded::log(1.0, 50.0),
    Bounded::linear(0.8, 3.0),
    Bounded::log(0.2, 12.0),
    Bounded::log(0.02, 1.0),
    Bounded::linear(2.0, 8.0),
    Bounded::linear(0.0, 1.0),
];

/// The fixed start, in the order of [`BOXES`]: a layer of a scale length and a twentieth of one
/// high, flaring over five, and a round-ish spheroid of a fifth of the weight.
pub const START: [f64; 8] = [1.0, 0.05, 5.0, 1.5, 1.0, 0.5, 4.0, 0.2];

/// The names of the parameters, for the report.
pub const NAMES: [&str; 8] = ["h_r", "h_0", "r_flare", "beta", "a", "q", "gamma", "weight"];

/// How close to a box's end, as a share of its width on its scale, counts as on the edge.
pub const EDGE_FRACTION: f64 = 1e-3;

/// How the fits are run, from the manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct DisciplineSettings {
    /// Gauss–Legendre nodes per cell and axis.
    pub order: usize,
    /// The most evaluations of a class's first fit.
    pub evaluations: usize,
    /// The most evaluations of a regularised refit.
    pub refit_evaluations: usize,
    /// The simplex's convergence tolerance, in misplaced share.
    pub tolerance: f64,
    /// A spheroid under this share of the weight is dropped: 0.05.
    pub drop_weight: f64,
    /// The penalty strengths tried, ascending.
    pub lambdas: Vec<f64>,
    /// The largest rise of the weighted mean misplaced share the penalty may cost: 0.003.
    pub rise_limit: f64,
    /// Gauss–Seidel sweeps along a row at each strength.
    pub sweeps: usize,
}

/// One class's fitted form.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscFit {
    /// The layer.
    pub layer: FlaredLayerParams,
    /// The spheroid; weight 0 if dropped.
    pub spheroid: CoredPowerLawParams,
    /// The misplaced share.
    pub misplaced: f64,
    /// The misplaced share its bound members would show by chance alone.
    pub floor: f64,
    /// How many of its parameters sit on a box edge.
    pub at_edge: usize,
    /// Whether the class had no bound member inside the cube, and so no form to fit (the start is
    /// kept).
    pub empty: bool,
}

impl DiscFit {
    /// The parameter vector, in the order of [`BOXES`].
    fn params(&self) -> [f64; 8] {
        let (l, s) = (&self.layer, &self.spheroid);
        [l.h_r, l.h_0, l.r_flare, l.beta, s.a, s.q, s.gamma, s.weight]
    }
}

/// A class's form and data, as the fits use them.
struct Class {
    observed: Vec<f64>,
    bound: u64,
}

/// Scratch space for one evaluation of the model.
struct Scratch {
    layer: Vec<f64>,
    spheroid: Vec<f64>,
    model: Vec<f64>,
}

impl Scratch {
    fn new() -> Self {
        let n = R_CELLS * Z_CELLS;
        Self {
            layer: vec![0.0; n],
            spheroid: vec![0.0; n],
            model: vec![0.0; n],
        }
    }
}

/// The layer's shares of the cells: `exp(−R ÷ h_r) exp(−(|z| ÷ h)^β) ÷ h`, `h = h₀ e^(R ÷
/// r_flare)`.
fn layer_shares(cells: &RzCells, p: &[f64], out: &mut [f64]) {
    let (h_r, h_0, r_flare, beta) = (p[0], p[1], p[2], p[3]);
    let ln_h0 = math::ln(h_0);
    cells.integrate_by_radius(
        |r| {
            let ln_h = ln_h0 + r / r_flare;
            (math::exp(-r / h_r - ln_h), ln_h)
        },
        |ln_h, _, ln_z| math::exp(-math::exp(beta * (ln_z - ln_h))),
        out,
    );
    normalise(out);
}

/// The spheroid's shares of the cells: `(1 + (R² + z² ÷ q²) ÷ a²)^(−γ ÷ 2)`.
pub fn spheroid_shares(cells: &RzCells, a: f64, q: f64, gamma: f64, out: &mut [f64]) {
    let (inv_core_sq, inv_axis_sq) = (1.0 / (a * a), 1.0 / (q * q));
    cells.integrate(
        |r, z| math::exp(-0.5 * gamma * math::ln_1p((r * r + z * z * inv_axis_sq) * inv_core_sq)),
        out,
    );
    normalise(out);
}

/// Which parts of the form a fit holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    /// The layer and the spheroid.
    Full,
    /// The layer alone, the spheroid dropped.
    LayerOnly,
}

impl Form {
    /// The form of a fit whose spheroid weighs `weight`.
    fn of(weight: f64) -> Self {
        if weight > 0.0 {
            Self::Full
        } else {
            Self::LayerOnly
        }
    }

    /// The parameters it fits, from the front of [`BOXES`].
    const fn parameters(self) -> usize {
        match self {
            Self::Full => 8,
            Self::LayerOnly => 4,
        }
    }
}

/// The model's shares for the parameters `p` (in the order of [`BOXES`]; for the layer alone the
/// spheroid's weight is 0 whatever `p[7]`).
fn model(cells: &RzCells, p: &[f64], form: Form, s: &mut Scratch) {
    layer_shares(cells, p, &mut s.layer);
    let w = match form {
        Form::Full => p[7],
        Form::LayerOnly => 0.0,
    };
    if w > 0.0 {
        spheroid_shares(cells, p[4], p[5], p[6], &mut s.spheroid);
        for ((m, l), sp) in s.model.iter_mut().zip(&s.layer).zip(&s.spheroid) {
            *m = (1.0 - w) * l + w * sp;
        }
    } else {
        s.model.copy_from_slice(&s.layer);
    }
}

/// The penalty tying a fit to its neighbours along the row: their parameters and forms, and the
/// strength λ.
#[derive(Debug, Clone, Copy)]
struct Tie<'a> {
    neighbours: &'a [([f64; 8], Form)],
    lambda: f64,
}

impl Tie<'_> {
    /// No tie.
    const NONE: Tie<'static> = Tie {
        neighbours: &[],
        lambda: 0.0,
    };

    /// `λ Σ (ln θ − ln θ′)²` over the parameters both fits carry (a dropped spheroid's are left
    /// out), against each neighbour.
    fn penalty(&self, p: &[f64], form: Form) -> f64 {
        if self.lambda <= 0.0 {
            return 0.0;
        }
        let mut sum = 0.0;
        for (q, q_form) in self.neighbours {
            let k = form.parameters().min(q_form.parameters());
            for i in 0..k {
                let d = math::ln(p[i].max(1e-300)) - math::ln(q[i].max(1e-300));
                sum += d * d;
            }
        }
        self.lambda * sum
    }
}

/// Fits one class of `form` from `start`, tied by `tie`, within `limits`.
fn fit_one(
    cells: &RzCells,
    class: &Class,
    start: &[f64; 8],
    form: Form,
    tie: Tie<'_>,
    limits: Stop,
) -> DiscFit {
    let n = form.parameters();
    let mut s = Scratch::new();
    let found = minimise(
        |p| {
            let mut full = *start;
            full[..n].copy_from_slice(p);
            model(cells, &full, form, &mut s);
            total_variation(&s.model, &class.observed) + tie.penalty(&full, form)
        },
        &BOXES[..n],
        &start[..n],
        0.3,
        limits,
    );
    let mut p = *start;
    p[..n].copy_from_slice(&found.values);
    if form == Form::LayerOnly {
        p[7] = 0.0;
    }
    finish(cells, class, &p, form, &mut s)
}

/// The fit at the parameters `p`: its misplaced share, noise floor and edges.
fn finish(cells: &RzCells, class: &Class, p: &[f64; 8], form: Form, s: &mut Scratch) -> DiscFit {
    model(cells, p, form, s);
    let misplaced = total_variation(&s.model, &class.observed);
    let n = form.parameters();
    let at_edge = (0..n)
        .filter(|&i| BOXES[i].at_edge(p[i], EDGE_FRACTION))
        .count();
    let weight = match form {
        Form::Full => p[7],
        Form::LayerOnly => 0.0,
    };
    DiscFit {
        layer: FlaredLayerParams {
            weight: 1.0 - weight,
            h_r: p[0],
            h_0: p[1],
            r_flare: p[2],
            beta: p[3],
        },
        spheroid: CoredPowerLawParams {
            weight,
            a: p[4],
            q: p[5],
            gamma: p[6],
        },
        misplaced,
        floor: noise_floor(&s.model, class.bound),
        at_edge,
        empty: false,
    }
}

/// The start's form for a class with nothing to fit.
fn empty_fit() -> DiscFit {
    let p = START;
    DiscFit {
        layer: FlaredLayerParams {
            weight: 1.0 - p[7],
            h_r: p[0],
            h_0: p[1],
            r_flare: p[2],
            beta: p[3],
        },
        spheroid: CoredPowerLawParams {
            weight: p[7],
            a: p[4],
            q: p[5],
            gamma: p[6],
        },
        misplaced: 0.0,
        floor: 0.0,
        at_edge: 0,
        empty: true,
    }
}

/// One class's first fit: the full form from [`START`], then the layer alone if the spheroid came
/// out under the drop weight.
fn first_fit(cells: &RzCells, class: &Class, settings: &DisciplineSettings) -> DiscFit {
    if class.bound == 0 {
        return empty_fit();
    }
    let limits = Stop {
        evaluations: settings.evaluations,
        tolerance: settings.tolerance,
    };
    let full = fit_one(cells, class, &START, Form::Full, Tie::NONE, limits);
    if full.spheroid.weight >= settings.drop_weight {
        return full;
    }
    fit_one(
        cells,
        class,
        &full.params(),
        Form::LayerOnly,
        Tie::NONE,
        limits,
    )
}

/// The disc-born fits of every class, `[speed][age]`, and the regularisation's outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscFits {
    /// The fits.
    pub fits: [[DiscFit; AGE_BINS]; SPEED_BINS],
    /// The class weights of the weighted mean, summing to 1 over non-empty classes.
    pub weights: [[f64; AGE_BINS]; SPEED_BINS],
    /// The weighted mean misplaced share without the penalty.
    pub unregularised: f64,
    /// The weighted mean misplaced share as fitted.
    pub mean_misplaced: f64,
    /// The penalty strength chosen.
    pub lambda: f64,
}

/// A row's fits after sweeps at `lambda` from `fits`.
fn sweep_row(
    cells: &RzCells,
    row: &[Class],
    fits: &[DiscFit],
    lambda: f64,
    settings: &DisciplineSettings,
) -> Vec<DiscFit> {
    let mut out = fits.to_vec();
    for _ in 0..settings.sweeps {
        for a in 0..out.len() {
            if out[a].empty {
                continue;
            }
            let neighbours: Vec<([f64; 8], Form)> = [a.checked_sub(1), Some(a + 1)]
                .into_iter()
                .flatten()
                .filter(|&b| b < out.len() && !out[b].empty)
                .map(|b| (out[b].params(), Form::of(out[b].spheroid.weight)))
                .collect();
            out[a] = fit_one(
                cells,
                &row[a],
                &out[a].params(),
                Form::of(out[a].spheroid.weight),
                Tie {
                    neighbours: &neighbours,
                    lambda,
                },
                Stop {
                    evaluations: settings.refit_evaluations,
                    tolerance: settings.tolerance,
                },
            );
        }
    }
    out
}

/// The weighted mean misplaced share of `fits` under `weights`.
fn weighted_mean(
    fits: &[[DiscFit; AGE_BINS]; SPEED_BINS],
    weights: &[[f64; AGE_BINS]; SPEED_BINS],
) -> f64 {
    fits.iter()
        .flatten()
        .zip(weights.iter().flatten())
        .map(|(f, w)| f.misplaced * w)
        .sum()
}

/// Fits the thin disc's 56 classes, `records[speed × AGE_BINS + age]`, with class weights
/// `speed_weights[s] × age_weights[a] ×` the class's bound share inside the cube, on `threads`
/// threads; the result does not depend on their number.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If `records` does not hold 56 classes.
pub fn fit_disc(
    cells: &RzCells,
    records: &[ClassHistogram],
    speed_weights: &[f64; SPEED_BINS],
    age_weights: &[f64; AGE_BINS],
    settings: &DisciplineSettings,
    threads: NonZeroUsize,
) -> Result<DiscFits, BuildThreadPoolError> {
    assert_eq!(records.len(), SPEED_BINS * AGE_BINS, "56 thin-disc classes");
    let classes: Vec<Class> = records
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let (s, a) = (i / AGE_BINS, i % AGE_BINS);
            if s + 1 < SPEED_BINS {
                return Class {
                    observed: shares(&r.bound),
                    bound: r.in_cube_bound,
                };
            }
            let column: Vec<&ClassHistogram> = (0..SPEED_BINS)
                .map(|s| &records[s * AGE_BINS + a])
                .collect();
            let counts = fastest_counts(&column);
            Class {
                observed: shares(&counts),
                bound: counts.iter().sum(),
            }
        })
        .collect();
    let mut weights = [[0.0; AGE_BINS]; SPEED_BINS];
    for (s, row) in weights.iter_mut().enumerate() {
        for (a, w) in row.iter_mut().enumerate() {
            let r = &records[s * AGE_BINS + a];
            #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
            let in_cube = if r.orbits > 0 {
                r.in_cube_bound as f64 / r.orbits as f64
            } else {
                0.0
            };
            *w = speed_weights[s] * age_weights[a] * in_cube;
        }
    }
    let total: f64 = weights.iter().flatten().sum();
    if total > 0.0 {
        for w in weights.iter_mut().flatten() {
            *w /= total;
        }
    }
    let first = map_in_order(SPEED_BINS * AGE_BINS, threads, |i| {
        first_fit(cells, &classes[i], settings)
    })?;
    let mut fits = [[empty_fit(); AGE_BINS]; SPEED_BINS];
    for (i, f) in first.into_iter().enumerate() {
        fits[i / AGE_BINS][i % AGE_BINS] = f;
    }
    let unregularised = weighted_mean(&fits, &weights);
    let mut chosen = (fits, unregularised, 0.0);
    for &lambda in &settings.lambdas {
        if lambda <= 0.0 {
            continue;
        }
        let start = chosen.0;
        let rows = map_in_order(SPEED_BINS, threads, |s| {
            sweep_row(
                cells,
                &classes[s * AGE_BINS..(s + 1) * AGE_BINS],
                &start[s],
                lambda,
                settings,
            )
        })?;
        let mut tried = [[empty_fit(); AGE_BINS]; SPEED_BINS];
        for (s, row) in rows.into_iter().enumerate() {
            tried[s].copy_from_slice(&row);
        }
        let mean = weighted_mean(&tried, &weights);
        if mean - unregularised > settings.rise_limit {
            break;
        }
        chosen = (tried, mean, lambda);
    }
    Ok(DiscFits {
        fits: chosen.0,
        weights,
        unregularised,
        mean_misplaced: chosen.1,
        lambda: chosen.2,
    })
}

/// Along each row, how many interior age bins' second differences of each fitted parameter's log
/// change sign against the previous bin's: the plan's smoothness check, counted (P15.T6.c).
#[must_use]
pub fn second_difference_sign_changes(fits: &DiscFits) -> usize {
    let mut changes = 0;
    for row in &fits.fits {
        for k in 0..8 {
            let logs: Vec<f64> = row
                .iter()
                .filter(|f| !f.empty && (k < 4 || f.spheroid.weight > 0.0))
                .map(|f| math::ln(f.params()[k].max(1e-300)))
                .collect();
            let second: Vec<f64> = logs.windows(3).map(|w| w[0] - 2.0 * w[1] + w[2]).collect();
            changes += second.windows(2).filter(|w| w[0] * w[1] < 0.0).count();
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::displaced_forms::histogram::{r_cell, z_cell};

    /// A histogram drawn from a known flared layer is fitted back to it.
    #[test]
    fn a_layer_is_fitted_back() {
        let r_d = 7_000.0;
        let cells = RzCells::new(r_d, 2);
        let truth = [1.2, 0.04, 6.0, 1.3, 1.0, 0.5, 4.0, 0.0];
        let mut s = Scratch::new();
        model(&cells, &truth, Form::LayerOnly, &mut s);
        // Exact expected counts of 10⁶ objects, rounded.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "non-negative counts below 10⁶"
        )]
        let counts: Vec<u64> = s.model.iter().map(|p| (p * 1e6).round() as u64).collect();
        let class = Class {
            observed: shares(&counts),
            bound: counts.iter().sum(),
        };
        let settings = DisciplineSettings {
            order: 2,
            evaluations: 1_500,
            refit_evaluations: 300,
            tolerance: 1e-9,
            drop_weight: 0.05,
            lambdas: vec![],
            rise_limit: 0.003,
            sweeps: 1,
        };
        let fit = first_fit(&cells, &class, &settings);
        assert!(fit.misplaced < 0.01, "{fit:?}");
        assert!((fit.layer.h_0 / 0.04 - 1.0).abs() < 0.1, "{fit:?}");
        assert!(r_cell(r_d) > 0 && z_cell(1.0) > 0);
    }
}
