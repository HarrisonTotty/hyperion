//! Hand-written optimisers (plan 15, Design note 3): a library's iteration order could change
//! between releases and move a table in a way the lock file cannot see.
//!
//! Each is deterministic: its iterations are capped, any early stop depends only on the values
//! it has computed, it draws nothing at random, and ties go to the lowest index. They use only
//! IEEE arithmetic and square roots, which are correctly rounded on every platform, and clamp to
//! a bound by an explicit comparison rather than `f64::max`, whose sign at a signed zero is
//! unspecified, so a fitted table comes out the same everywhere.
//!
//! - [`golden_section`] and [`nelder_mead`] (P15.T8.a): derivative-free minima.
//! - [`nnls`] (P15.T3.a): non-negative linear least squares, after Lawson and Hanson (1974).
//! - [`levenberg_marquardt`] (P15.T3.a): non-linear least squares with lower bounds, after
//!   Levenberg (1944) and Marquardt (1963).
//!
//! The last two solve their linear problems by Householder QR (Lawson and Hanson 1974, ch. 10),
//! never through the normal equations, whose condition number is the square of the problem's: a
//! basis of Gaussians whose widths differ by a few per cent is all but dependent.

/// A column below this fraction of its own norm, once the columns before it are taken out, is
/// treated as dependent on them: 2⁻⁴⁰, 2¹² times `f64`'s precision, so a solve that would
/// amplify rounding by more than about 10¹² is refused.
const DEPENDENT: f64 = 1.0 / 1_099_511_627_776.0;

/// The Euclidean norm of `v`.
#[must_use]
fn norm(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |sum, &x| sum + x * x).sqrt()
}

/// `a · b`, summed in index order.
#[must_use]
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |sum, (&x, &y)| sum + x * y)
}

/// Householder QR of the matrix whose columns are `columns`, each of `rhs.len()` rows, in place:
/// `rhs` becomes `Qᵀ rhs`, `columns[l][j]` for `j < l` holds `R`'s entry in row `j` and column
/// `l`, and the returned diagonal holds `R`'s `(j, j)` entries (Lawson and Hanson 1974, ch. 10,
/// Algorithm H1–H2). A zero column leaves a zero diagonal entry and no reflection.
fn householder(columns: &mut [Vec<f64>], rhs: &mut [f64]) -> Vec<f64> {
    let rows = rhs.len();
    let mut diagonal = vec![0.0; columns.len()];
    for j in 0..columns.len().min(rows) {
        let (head, tail) = columns.split_at_mut(j + 1);
        let v = &mut head[j][j..];
        let length = norm(v);
        if length <= 0.0 {
            continue;
        }
        let alpha = if v[0] > 0.0 { -length } else { length };
        v[0] -= alpha;
        let vv = dot(v, v);
        for column in tail {
            let s = 2.0 * dot(v, &column[j..]) / vv;
            for (c, &x) in column[j..].iter_mut().zip(v.iter()) {
                *c -= s * x;
            }
        }
        let s = 2.0 * dot(v, &rhs[j..]) / vv;
        for (c, &x) in rhs[j..].iter_mut().zip(v.iter()) {
            *c -= s * x;
        }
        diagonal[j] = alpha;
    }
    diagonal
}

/// The solution of `R z = c` for the upper-triangular `R` that [`householder`] left in `columns`
/// and `diagonal`, `c` the first entries of `qtb`; `None` if a diagonal entry is below
/// [`DEPENDENT`] times `scale[j]`.
#[must_use]
fn back_substitute(
    columns: &[Vec<f64>],
    diagonal: &[f64],
    qtb: &[f64],
    scale: &[f64],
) -> Option<Vec<f64>> {
    let k = diagonal.len();
    if (0..k).any(|j| diagonal[j].abs() <= DEPENDENT * scale[j]) {
        return None;
    }
    let mut z = vec![0.0; k];
    for j in (0..k).rev() {
        let mut sum = qtb[j];
        for l in j + 1..k {
            sum -= columns[l][j] * z[l];
        }
        z[j] = sum / diagonal[j];
    }
    Some(z)
}

/// The least-squares coefficients of `rhs` on the columns `free` of `columns` (norms `norms`), or
/// `None` if one of them depends on the others ([`DEPENDENT`]).
#[must_use]
fn solve_on(columns: &[Vec<f64>], norms: &[f64], free: &[usize], rhs: &[f64]) -> Option<Vec<f64>> {
    let mut work: Vec<Vec<f64>> = free.iter().map(|&j| columns[j].clone()).collect();
    let mut qtb = rhs.to_vec();
    let diagonal = householder(&mut work, &mut qtb);
    let scale: Vec<f64> = free.iter().map(|&j| norms[j]).collect();
    back_substitute(&work, &diagonal, &qtb, &scale)
}

/// The non-negative least-squares solution: the `x ≥ 0` that minimises `‖A x − b‖₂`, with `A`'s
/// rows `rows` and `b` = `rhs`, by Lawson and Hanson's active-set algorithm NNLS (1974, *Solving
/// Least Squares Problems*, Prentice-Hall, ch. 23, §3).
///
/// It starts from `x = 0` with every variable held at zero. Each outer step frees the held
/// variable whose component of the gradient `w = Aᵀ(b − A x)` is largest, the lowest index among
/// equals, provided it exceeds 2⁻⁴⁰ `‖aⱼ‖ ‖b‖`, and solves the unconstrained problem on the free
/// variables by Householder QR. A freed variable whose solution comes out non-positive, as
/// rounding can make it, is held again and the next is tried (the book's step 6). While the
/// solution has a non-positive component, each inner step moves from the last feasible point
/// towards it as far as feasibility allows, holds the variables that reach zero, and solves
/// again. An outer step that frees nothing ends the run: the Kuhn–Tucker conditions (the book's
/// Theorem (23.4), as §3 applies it to NNLS) then hold to rounding. Lawson and Hanson's own
/// subroutine (1974, App. C) gives up after `3n` passes of its inner loop and reports failure;
/// here `3n` outer steps, each of at most `n` inner passes, bound the work, and the run returns
/// the last feasible point either way.
///
/// The result has exactly `0.0` for every held variable.
///
/// # Panics
///
/// If `rows` is empty, its rows are empty or of differing lengths, or `rhs` has another length.
///
/// # Examples
///
/// ```
/// use hyperion_fit::optimise::nnls;
///
/// // Unconstrained, the second coefficient would be −1: held at zero, the first takes 1.5.
/// let x = nnls(&[[1.0, 0.0], [0.0, 1.0], [1.0, 1.0]], &[2.0, -1.0, 1.0]);
/// assert!((x[0] - 1.5).abs() < 1e-15);
/// assert!(x[1].total_cmp(&0.0).is_eq());
/// ```
#[must_use]
#[expect(
    clippy::many_single_char_names,
    reason = "the book's symbols: m × n, x, w, t and z"
)]
pub fn nnls<R: AsRef<[f64]>>(rows: &[R], rhs: &[f64]) -> Vec<f64> {
    let m = rows.len();
    assert!(
        m > 0 && rhs.len() == m,
        "NNLS needs rows and a right-hand side of the same length"
    );
    let n = rows[0].as_ref().len();
    assert!(
        n > 0 && rows.iter().all(|r| r.as_ref().len() == n),
        "NNLS needs rows of one non-zero length"
    );
    let columns: Vec<Vec<f64>> = (0..n)
        .map(|j| rows.iter().map(|r| r.as_ref()[j]).collect())
        .collect();
    let norms: Vec<f64> = columns.iter().map(|c| norm(c)).collect();
    let threshold = norm(rhs) * DEPENDENT;
    let mut x = vec![0.0; n];
    let mut free = vec![false; n];
    let mut residual = rhs.to_vec();
    for _ in 0..3 * n {
        let gradient: Vec<f64> = columns.iter().map(|c| dot(c, &residual)).collect();
        let mut tried = vec![false; n];
        let solution = loop {
            let mut best: Option<usize> = None;
            for j in 0..n {
                if free[j] || tried[j] || gradient[j] <= threshold * norms[j] {
                    continue;
                }
                if best.is_none_or(|b| gradient[j] > gradient[b]) {
                    best = Some(j);
                }
            }
            let Some(t) = best else {
                break None;
            };
            tried[t] = true;
            free[t] = true;
            let indices: Vec<usize> = (0..n).filter(|&j| free[j]).collect();
            match solve_on(&columns, &norms, &indices, rhs) {
                Some(z) if z[indices.iter().position(|&j| j == t).expect("t is free")] > 0.0 => {
                    break Some((indices, z));
                }
                _ => free[t] = false,
            }
        };
        let Some((mut indices, mut z)) = solution else {
            break;
        };
        // The inner loop: each pass holds at least one more variable, so `n` passes suffice.
        for _ in 0..n {
            if z.iter().all(|&v| v > 0.0) {
                break;
            }
            let mut alpha = f64::INFINITY;
            let mut first = 0;
            for (i, &j) in indices.iter().enumerate() {
                if z[i] <= 0.0 {
                    let step = x[j] / (x[j] - z[i]);
                    if step < alpha {
                        alpha = step;
                        first = j;
                    }
                }
            }
            for (i, &j) in indices.iter().enumerate() {
                x[j] += alpha * (z[i] - x[j]);
            }
            x[first] = 0.0;
            for &j in &indices {
                if x[j] <= 0.0 {
                    x[j] = 0.0;
                    free[j] = false;
                }
            }
            indices = (0..n).filter(|&j| free[j]).collect();
            // In exact arithmetic a subset of independent columns is independent, but rounding can
            // put a diagonal that just cleared `DEPENDENT` below it in the subset. Then the point
            // already reached, which is feasible, is kept.
            if let Some(next) = solve_on(&columns, &norms, &indices, rhs) {
                z = next;
            } else {
                z = indices.iter().map(|&j| x[j]).collect();
                break;
            }
        }
        x.fill(0.0);
        for (i, &j) in indices.iter().enumerate() {
            x[j] = if z[i] > 0.0 { z[i] } else { 0.0 };
            free[j] = x[j] > 0.0;
        }
        residual.copy_from_slice(rhs);
        for (column, &xj) in columns.iter().zip(&x) {
            if xj > 0.0 {
                for (r, &a) in residual.iter_mut().zip(column) {
                    *r -= a * xj;
                }
            }
        }
    }
    x
}

/// The damping's start, floor and ceiling in [`levenberg_marquardt`], relative to the columns'
/// scales: from about a Gauss–Newton step to about a gradient step of 10⁻¹² of the scale.
const LAMBDA_START: f64 = 1e-3;
const LAMBDA_MIN: f64 = 1e-12;
const LAMBDA_MAX: f64 = 1e12;

/// What [`levenberg_marquardt`] found.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LeastSquaresFit {
    /// The parameters at the lowest sum of squares found.
    pub params: Vec<f64>,
    /// The sum of the squared residuals there.
    pub sum_of_squares: f64,
    /// The steps taken, each of which lowered the sum.
    pub steps: u32,
}

/// The parameters `p ≥ lower` that minimise `Σ rᵢ(p)²` from `start`, by Levenberg's (1944, Q.
/// Appl. Math. 2, 164) and Marquardt's (1963, J. SIAM 11, 431) damped Gauss–Newton method, over at
/// most `iterations` iterations.
///
/// `model(p, r, jacobian)` writes the `residuals` values `rᵢ(p)` into `r` and, when `jacobian` is
/// `Some`, the derivatives `∂rᵢ ÷ ∂pₖ` into it column by column: `jacobian[k × residuals + i]`.
///
/// Each iteration takes the Jacobian at the current point and solves the damped problem
/// `min ‖J δ + r‖² + λ ‖D δ‖²` by Householder QR of `[J; √λ D]`, where `D` holds the largest
/// norm each column has had so far, or 1 for a column that has always been zero: Marquardt's
/// scaling, kept as Moré (1978, in *Numerical Analysis*, Lecture Notes in Mathematics 630,
/// Springer, 105–116) keeps it and MINPACK's `lmder` implements it. The trial point is `p + δ`
/// projected onto the bounds. A trial that
/// lowers the sum is taken and `λ` divided by 10; otherwise `λ` is multiplied by 10 and the step
/// solved again, from 10⁻³ between the limits 10⁻¹² and 10¹². A parameter on its bound whose
/// descent direction points out of the box is held for the iteration, so that the others' step
/// does not assume it moves: Bertsekas's set `I⁺(x)` (1982, SIAM J. Control Optim. 20, 221,
/// Prop. 1), without his ε-enlargement of it. A
/// value at or below its bound becomes the bound itself, so a bound of `0.0` is met by `+0.0`
/// whatever the sign of a zero step.
///
/// The run ends when the iterations are spent, or when no step lowers the sum even at the largest
/// damping: every later iteration would then repeat the same failed trial, so stopping changes no
/// bit. A residual that is not finite makes its trial fail.
///
/// # Panics
///
/// If `start` is empty, `lower` has another length, or there are fewer residuals than
/// parameters.
///
/// # Examples
///
/// ```
/// use hyperion_fit::optimise::levenberg_marquardt;
///
/// // A decay y = a e^(−b t) through four exact points, from a poor start.
/// let t = [0.0, 1.0, 2.0, 3.0];
/// let fit = levenberg_marquardt(
///     |p: &[f64], r: &mut [f64], mut jacobian: Option<&mut [f64]>| {
///         for (i, &ti) in t.iter().enumerate() {
///             let e = hyperion_sim::math::exp(-p[1] * ti);
///             r[i] = p[0] * e - 2.0 * hyperion_sim::math::exp(-0.5 * ti);
///             if let Some(j) = jacobian.as_deref_mut() {
///                 j[i] = e;
///                 j[4 + i] = -p[0] * ti * e;
///             }
///         }
///     },
///     &[1.0, 1.0],
///     &[f64::NEG_INFINITY; 2],
///     4,
///     100,
/// );
/// assert!((fit.params[0] - 2.0).abs() < 1e-12 && (fit.params[1] - 0.5).abs() < 1e-12);
/// ```
#[must_use]
#[expect(
    clippy::many_single_char_names,
    reason = "the method's symbols: m residuals r of n parameters p, the damped step's D"
)]
pub fn levenberg_marquardt(
    mut model: impl FnMut(&[f64], &mut [f64], Option<&mut [f64]>),
    start: &[f64],
    lower: &[f64],
    residuals: usize,
    iterations: u32,
) -> LeastSquaresFit {
    let n = start.len();
    let m = residuals;
    assert!(
        n > 0 && lower.len() == n && m >= n,
        "Levenberg–Marquardt needs bounds for each parameter and as many residuals"
    );
    let mut p: Vec<f64> = start
        .iter()
        .zip(lower)
        .map(|(&s, &l)| if s > l { s } else { l })
        .collect();
    let mut r = vec![0.0; m];
    let mut jacobian = vec![0.0; m * n];
    model(&p, &mut r, Some(&mut jacobian));
    let mut sum = dot(&r, &r);
    let mut current = true;
    let mut scale = vec![0.0_f64; n];
    let mut lambda = LAMBDA_START;
    let mut steps = 0;
    let mut trial = vec![0.0; n];
    let mut r_trial = vec![0.0; m];
    for _ in 0..iterations {
        if !current {
            model(&p, &mut r, Some(&mut jacobian));
            current = true;
        }
        let column = |k: usize| &jacobian[k * m..(k + 1) * m];
        let free: Vec<usize> = (0..n)
            .filter(|&k| !(p[k] <= lower[k] && dot(column(k), &r) > 0.0))
            .collect();
        for (k, s) in scale.iter_mut().enumerate() {
            *s = s.max(norm(column(k)));
        }
        let mut columns: Vec<Vec<f64>> = free.iter().map(|&k| column(k).to_vec()).collect();
        let mut qtr = r.clone();
        let diagonal = householder(&mut columns, &mut qtr);
        let nf = free.len();
        let mut moved = false;
        loop {
            // [R; √λ D] δ ≈ −[Qᵀr; 0], by a second QR of the 2n_f × n_f stack.
            let mut stack: Vec<Vec<f64>> = (0..nf)
                .map(|l| {
                    let mut c = vec![0.0; 2 * nf];
                    c[..l].copy_from_slice(&columns[l][..l]);
                    c[l] = diagonal[l];
                    let d = if scale[free[l]] > 0.0 {
                        scale[free[l]]
                    } else {
                        1.0
                    };
                    c[nf + l] = lambda.sqrt() * d;
                    c
                })
                .collect();
            let mut rhs = vec![0.0; 2 * nf];
            for (x, &q) in rhs.iter_mut().zip(&qtr[..nf]) {
                *x = -q;
            }
            let stack_diagonal = householder(&mut stack, &mut rhs);
            // The damping makes the stack full rank, so only an exact zero is refused.
            let no_floor = vec![0.0; nf];
            let step = back_substitute(&stack, &stack_diagonal, &rhs, &no_floor);
            if let Some(delta) = step {
                trial.copy_from_slice(&p);
                for (&k, d) in free.iter().zip(delta) {
                    let moved_to = p[k] + d;
                    trial[k] = if moved_to > lower[k] {
                        moved_to
                    } else {
                        lower[k]
                    };
                }
                model(&trial, &mut r_trial, None);
                let trial_sum = dot(&r_trial, &r_trial);
                if trial_sum.is_finite() && trial_sum < sum {
                    std::mem::swap(&mut p, &mut trial);
                    std::mem::swap(&mut r, &mut r_trial);
                    sum = trial_sum;
                    lambda = (lambda * 0.1).max(LAMBDA_MIN);
                    steps += 1;
                    current = false;
                    moved = true;
                    break;
                }
            }
            if lambda >= LAMBDA_MAX {
                break;
            }
            lambda = (lambda * 10.0).min(LAMBDA_MAX);
        }
        if !moved {
            break;
        }
    }
    LeastSquaresFit {
        params: p,
        sum_of_squares: sum,
        steps,
    }
}

/// The minimum of `f` on `[lo, hi]` by golden-section search over `iterations` steps: the midpoint
/// of the last bracket. `f` should be unimodal on the interval.
#[must_use]
#[expect(
    clippy::many_single_char_names,
    reason = "the method's usual bracket symbols"
)]
pub fn golden_section(mut f: impl FnMut(f64) -> f64, lo: f64, hi: f64, iterations: u32) -> f64 {
    let g = 0.5 * (5.0_f64.sqrt() - 1.0);
    let (mut a, mut b) = (lo, hi);
    let mut c = b - g * (b - a);
    let mut d = a + g * (b - a);
    let (mut fc, mut fd) = (f(c), f(d));
    for _ in 0..iterations {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - g * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + g * (b - a);
            fd = f(d);
        }
    }
    f64::midpoint(a, b)
}

/// The minimum of `f` from `start` by Nelder and Mead's (1965) simplex, the initial simplex
/// `start` plus `step` along each axis in turn, with the standard coefficients (reflection 1,
/// expansion 2, contraction ½, shrink ½), over `iterations` iterations: the best vertex and its
/// value.
///
/// # Panics
///
/// If `start` and `step` differ in length or are empty.
#[must_use]
pub fn nelder_mead(
    mut f: impl FnMut(&[f64]) -> f64,
    start: &[f64],
    step: &[f64],
    iterations: u32,
) -> (Vec<f64>, f64) {
    let n = start.len();
    assert!(
        n > 0 && step.len() == n,
        "a simplex needs matching start and steps"
    );
    let mut simplex: Vec<Vec<f64>> = vec![start.to_vec()];
    for i in 0..n {
        let mut v = start.to_vec();
        v[i] += step[i];
        simplex.push(v);
    }
    let mut values: Vec<f64> = simplex.iter().map(|v| f(v)).collect();
    let along = |from: &[f64], to: &[f64], t: f64| -> Vec<f64> {
        from.iter()
            .zip(to)
            .map(|(&a, &b)| a + t * (b - a))
            .collect()
    };
    for _ in 0..iterations {
        // Order the vertices by value, ties by their index, so that the order is reproducible.
        let mut order: Vec<usize> = (0..=n).collect();
        order.sort_by(|&i, &j| values[i].total_cmp(&values[j]).then(i.cmp(&j)));
        simplex = order.iter().map(|&i| simplex[i].clone()).collect();
        values = order.iter().map(|&i| values[i]).collect();
        let centroid: Vec<f64> = (0..n)
            .map(|k| {
                simplex[..n].iter().fold(0.0, |s, v| s + v[k])
                    / f64::from(u32::try_from(n).expect("a small dimension"))
            })
            .collect();
        let worst = simplex[n].clone();
        let reflected = along(&centroid, &worst, -1.0);
        let fr = f(&reflected);
        if fr < values[0] {
            let expanded = along(&centroid, &worst, -2.0);
            let fe = f(&expanded);
            if fe < fr {
                simplex[n] = expanded;
                values[n] = fe;
            } else {
                simplex[n] = reflected;
                values[n] = fr;
            }
        } else if fr < values[n - 1] {
            simplex[n] = reflected;
            values[n] = fr;
        } else {
            let (towards, f_towards) = if fr < values[n] {
                (reflected, fr)
            } else {
                (worst, values[n])
            };
            let contracted = along(&centroid, &towards, 0.5);
            let fc = f(&contracted);
            if fc < f_towards {
                simplex[n] = contracted;
                values[n] = fc;
            } else {
                let best = simplex[0].clone();
                for i in 1..=n {
                    simplex[i] = along(&best, &simplex[i], 0.5);
                    values[i] = f(&simplex[i]);
                }
            }
        }
    }
    let best = (0..=n)
        .min_by(|&i, &j| values[i].total_cmp(&values[j]).then(i.cmp(&j)))
        .expect("a simplex has vertices");
    (simplex[best].clone(), values[best])
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    #[test]
    fn golden_section_finds_a_parabola_s_minimum() {
        let x = golden_section(|x| (x - 0.3) * (x - 0.3), -2.0, 2.0, 80);
        assert!((x - 0.3).abs() < 1e-9, "{x}");
    }

    #[test]
    fn nelder_mead_finds_rosenbrock_s_minimum() {
        let rosenbrock = |v: &[f64]| {
            let (x, y) = (v[0], v[1]);
            (1.0 - x) * (1.0 - x) + 100.0 * (y - x * x) * (y - x * x)
        };
        let (best, value) = nelder_mead(rosenbrock, &[-1.2, 1.0], &[0.5, 0.5], 2_000);
        assert!(
            (best[0] - 1.0).abs() < 1e-6 && (best[1] - 1.0).abs() < 1e-6,
            "{best:?}"
        );
        assert!(value < 1e-12);
        assert_eq!(
            nelder_mead(rosenbrock, &[-1.2, 1.0], &[0.5, 0.5], 2_000),
            (best, value)
        );
    }

    /// Lawson and Hanson's conditions at a solution (1974, ch. 23, Theorem (23.4), as §3 applies
    /// it to NNLS): `x ≥ 0`, and
    /// `w = Aᵀ(b − Ax)` is zero where `x > 0` and not positive where `x = 0`, to rounding.
    fn assert_kuhn_tucker(rows: &[Vec<f64>], rhs: &[f64], x: &[f64]) {
        let residual: Vec<f64> = rows
            .iter()
            .zip(rhs)
            .map(|(row, &b)| b - dot(row, x))
            .collect();
        let size = norm(rhs) * rows.iter().map(|r| norm(r)).fold(0.0, f64::max);
        for (j, &xj) in x.iter().enumerate() {
            let column: Vec<f64> = rows.iter().map(|r| r[j]).collect();
            let w = dot(&column, &residual);
            assert!(xj >= 0.0, "x[{j}] = {xj}");
            if xj > 0.0 {
                assert!(w.abs() < 1e-12 * size, "w[{j}] = {w} at x[{j}] = {xj}");
            } else {
                assert!(w < 1e-12 * size, "w[{j}] = {w} with x[{j}] held");
            }
        }
    }

    #[test]
    fn nnls_gives_the_least_squares_solution_when_it_is_positive() {
        let rows = [[1.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, -1.0]];
        let x = nnls(&rows, &[1.0, 2.0, 3.0, -1.0]);
        assert!(
            (x[0] - 1.0).abs() < 1e-15 && (x[1] - 2.0).abs() < 1e-15,
            "{x:?}"
        );
    }

    #[test]
    fn nnls_holds_a_variable_whose_free_value_would_be_negative() {
        let rows = [[1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        let x = nnls(&rows, &[2.0, -1.0, 1.0]);
        assert!((x[0] - 1.5).abs() < 1e-15, "{x:?}");
        assert!(x[1].total_cmp(&0.0).is_eq(), "{x:?}");
    }

    #[test]
    fn nnls_is_zero_when_every_column_opposes_the_right_hand_side() {
        let x = nnls(&[[1.0, 2.0], [1.0, 0.5]], &[-1.0, -2.0]);
        assert_eq!(x.len(), 2);
        assert!(x.iter().all(|v| v.total_cmp(&0.0).is_eq()), "{x:?}");
    }

    /// A sum of two Gaussians in a basis of five whose neighbours overlap strongly: the basis is
    /// ill-conditioned, the normal equations' condition number about 10¹², and the weights still
    /// come back exactly.
    #[test]
    fn nnls_recovers_a_sum_of_gaussians_from_an_overlapping_basis() {
        let widths = [0.5, 1.0, 1.5, 2.0, 3.0];
        let points: Vec<f64> = (0..200).map(|i| 0.04 * f64::from(i)).collect();
        let gaussian = |x: f64, s: f64| hyperion_sim::math::exp(-x * x / (2.0 * s * s));
        let rows: Vec<Vec<f64>> = points
            .iter()
            .map(|&x| widths.iter().map(|&s| gaussian(x, s)).collect())
            .collect();
        let rhs: Vec<f64> = points
            .iter()
            .map(|&x| 0.3 * gaussian(x, 1.0) + 0.7 * gaussian(x, 2.0))
            .collect();
        let x = nnls(&rows, &rhs);
        for (got, want) in x.iter().zip([0.0, 0.3, 0.0, 0.7, 0.0]) {
            assert!((got - want).abs() < 1e-9, "{x:?}");
        }
        assert_kuhn_tucker(&rows, &rhs, &x);
    }

    /// A problem in which some variables are held and some are free, with entries from a fixed
    /// formula, meets the Kuhn–Tucker conditions; and two runs give the same bits.
    #[test]
    fn nnls_meets_the_kuhn_tucker_conditions_and_is_deterministic() {
        let rows: Vec<Vec<f64>> = (0..30)
            .map(|i| {
                (0..8)
                    .map(|j| {
                        hyperion_sim::math::sin(1.3 * f64::from(i) + 0.7 * f64::from(j * j) + 0.1)
                    })
                    .collect()
            })
            .collect();
        let rhs: Vec<f64> = (0..30)
            .map(|i| hyperion_sim::math::cos(0.9 * f64::from(i)))
            .collect();
        let x = nnls(&rows, &rhs);
        assert!(x.iter().any(|&v| v > 0.0) && x.iter().any(|v| v.total_cmp(&0.0).is_eq()));
        assert_kuhn_tucker(&rows, &rhs, &x);
        assert_eq!(nnls(&rows, &rhs), x);
    }

    /// Rosenbrock's function as least squares, `r = (10 (y − x²), 1 − x)`, from Moré, Garbow and
    /// Hillstrom's (1981, ACM TOMS 7, 17, problem 1) standard start.
    fn rosenbrock(p: &[f64], r: &mut [f64], jacobian: Option<&mut [f64]>) {
        r[0] = 10.0 * (p[1] - p[0] * p[0]);
        r[1] = 1.0 - p[0];
        if let Some(j) = jacobian {
            j.copy_from_slice(&[-20.0 * p[0], -1.0, 10.0, 0.0]);
        }
    }

    #[test]
    fn levenberg_marquardt_finds_rosenbrock_s_minimum() {
        let fit = levenberg_marquardt(rosenbrock, &[-1.2, 1.0], &[f64::NEG_INFINITY; 2], 2, 200);
        assert!(
            (fit.params[0] - 1.0).abs() < 1e-12 && (fit.params[1] - 1.0).abs() < 1e-12,
            "{fit:?}"
        );
        assert!(fit.sum_of_squares < 1e-24, "{fit:?}");
        assert!(fit.steps > 5, "{fit:?}");
    }

    /// `(x + 1)² + (y − 2)² + (x + y − 1)²` is least at (−1, 2); with `x ≥ 0` it is least at
    /// (0, 1.5), where the gradient in x points out of the box. There the sum is 1.5, not 0, and
    /// within about √ε of its minimum it is flat to rounding, so no step can be seen to lower it:
    /// y is good to about 10⁻⁸ and x, on its bound, exactly.
    #[test]
    fn levenberg_marquardt_stops_on_a_lower_bound() {
        let model = |p: &[f64], r: &mut [f64], jacobian: Option<&mut [f64]>| {
            r.copy_from_slice(&[p[0] + 1.0, p[1] - 2.0, p[0] + p[1] - 1.0]);
            if let Some(j) = jacobian {
                j.copy_from_slice(&[1.0, 0.0, 1.0, 0.0, 1.0, 1.0]);
            }
        };
        let fit = levenberg_marquardt(model, &[3.0, -4.0], &[0.0, f64::NEG_INFINITY], 3, 100);
        assert!(fit.params[0].total_cmp(&0.0).is_eq(), "{fit:?}");
        assert!((fit.params[1] - 1.5).abs() < 1e-8, "{fit:?}");
        assert!((fit.sum_of_squares - 1.5).abs() < 1e-15, "{fit:?}");
        let free = levenberg_marquardt(model, &[3.0, -4.0], &[f64::NEG_INFINITY; 2], 3, 100);
        assert!(
            (free.params[0] + 1.0).abs() < 1e-12 && (free.params[1] - 2.0).abs() < 1e-12,
            "{free:?}"
        );
    }

    #[test]
    fn levenberg_marquardt_is_deterministic() {
        let run = || levenberg_marquardt(rosenbrock, &[-1.2, 1.0], &[f64::NEG_INFINITY; 2], 2, 40);
        assert_eq!(run(), run());
    }

    /// `r = p² − 9` from `p = 1`, with no residual (NaN) beyond 3.5: the first Gauss–Newton step,
    /// to 5, fails, the damping grows until a step stays inside, and the run still ends at 3.
    #[test]
    fn levenberg_marquardt_refuses_a_trial_whose_residual_is_not_finite() {
        let model = |p: &[f64], r: &mut [f64], jacobian: Option<&mut [f64]>| {
            r[0] = if p[0] > 3.5 {
                f64::NAN
            } else {
                p[0] * p[0] - 9.0
            };
            if let Some(j) = jacobian {
                j[0] = 2.0 * p[0];
            }
        };
        let fit = levenberg_marquardt(model, &[1.0], &[f64::NEG_INFINITY], 1, 100);
        assert!((fit.params[0] - 3.0).abs() < 1e-12, "{fit:?}");
        assert!(fit.sum_of_squares.is_finite(), "{fit:?}");
    }

    /// The optimisers' bits, pinned: a later table (P15.T3.c's) reads them, so an algebraically
    /// equal rewrite of the arithmetic, which moves bits, must fail here first.
    #[test]
    fn the_optimisers_keep_their_bits() {
        let widths = [0.5, 1.0, 1.5, 2.0, 3.0];
        let gaussian = |x: f64, s: f64| hyperion_sim::math::exp(-x * x / (2.0 * s * s));
        let rows: Vec<Vec<f64>> = (0..200)
            .map(|i| {
                let x = 0.04 * f64::from(i);
                widths.iter().map(|&s| gaussian(x, s)).collect()
            })
            .collect();
        let rhs: Vec<f64> = (0..200)
            .map(|i| {
                let x = 0.04 * f64::from(i);
                0.3 * gaussian(x, 1.2) + 0.7 * gaussian(x, 2.3)
            })
            .collect();
        // Widths 1.2 and 2.3 are not in the basis, so some weights are held and some are free.
        let x = nnls(&rows, &rhs);
        let pinned = [
            0.0,
            0.167_039_182_819_001_76,
            0.0,
            0.693_973_982_264_288_7,
            0.140_787_707_845_158_57,
        ];
        for (&got, want) in x.iter().zip(pinned) {
            assert_same_bits(got, want);
        }
        // Twelve iterations stop Rosenbrock's fit half-way, where every step's bits still count.
        let fit = levenberg_marquardt(rosenbrock, &[-1.2, 1.0], &[f64::NEG_INFINITY; 2], 2, 12);
        assert_same_bits(fit.params[0], 0.593_736_058_611_009_4);
        assert_same_bits(fit.params[1], 0.348_491_381_232_140_85);
        assert_same_bits(fit.sum_of_squares, 0.166_675_387_806_331_8);
    }
}
