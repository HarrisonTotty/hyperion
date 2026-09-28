//! The histograms' cells as integration domains, and the misplaced share (plan 15, P15.T6.c–f).
//!
//! A form is fitted by comparing, cell by cell, the share of a class's bound members inside the
//! cube that the orbits put in each cell of its histogram ([`histogram`](super::histogram)) with
//! the share the form puts there. The *misplaced share* is the total-variation distance between
//! the two, `½ Σ |pᵢ − fᵢ|`: the share of objects the form puts in the wrong cell (brainstorm,
//! "Displaced objects: kicks and runaways").
//!
//! A form's share of a cell is its integral over the part of the cell inside the root cube,
//! divided by its integral over the whole cube, which the cells tile. The (R, |z|) cells are
//! integrated with the circle's arc length inside the square (plan 08, Design note 26, as
//! [`hyperion_sim::galaxy::displaced::forms::cube_integral`] reduces it): `2πR` out to the cube's
//! half-width L and `4R (π ÷ 2 − 2 arccos(L ÷ R))` from L to `√2 L`, both signs of z counted.
//! The bar-frame cells in |x|, |y|, |z| are boxes, cut at L. Each cell takes Gauss–Legendre nodes
//! on each axis, in ln of the coordinate for the logarithmic cells and linearly for the first,
//! which reaches down to 0; an R cell holding L is split there, where the arc has a kink.
//!
//! Lengths here are in the thin disc's scale length `R_d`, as the table's are.

use std::num::NonZeroUsize;

use hyperion_sim::coords::ROOT_HALF_WIDTH_LY;
use hyperion_sim::math;
use hyperion_sim::tables::gauss_legendre::{GL4_NODES, GL4_WEIGHTS, GL8_NODES, GL8_WEIGHTS};

use super::histogram::{
    BAR_CELLS, BAR_XY_AXIS, ClassHistogram, LogAxis, R_AXIS, R_CELLS, Z_AXIS, Z_CELLS,
};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};

/// `f(0)`, …, `f(n − 1)` on `threads` threads, in order: one item a chunk, so the results do not
/// depend on the thread count.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// Never: a count that fits in memory fits in 64 bits, and back.
pub fn map_in_order<T: Send>(
    n: usize,
    threads: NonZeroUsize,
    f: impl Fn(usize) -> T + Sync,
) -> Result<Vec<T>, BuildThreadPoolError> {
    let mut out = Vec::with_capacity(n);
    map_reduce_chunks(
        u64::try_from(n).expect("a count that fits in memory fits in 64 bits"),
        1,
        threads,
        |range| f(usize::try_from(range.start).expect("an index below a usize count")),
        |item| out.push(item),
    )?;
    Ok(out)
}

/// The Gauss–Legendre nodes and weights of order `order` on [−1, 1]: 2, 3, 4, and 8 for any
/// other ([`FitParams`](super::table::FitParams) admits only those four).
fn legendre(order: usize) -> Vec<(f64, f64)> {
    match order {
        2 => {
            let x = 1.0 / 3.0_f64.sqrt();
            vec![(-x, 1.0), (x, 1.0)]
        }
        3 => {
            let x = (0.6_f64).sqrt();
            vec![(-x, 5.0 / 9.0), (0.0, 8.0 / 9.0), (x, 5.0 / 9.0)]
        }
        4 => GL4_NODES.iter().copied().zip(GL4_WEIGHTS).collect(),
        _ => GL8_NODES.iter().copied().zip(GL8_WEIGHTS).collect(),
    }
}

/// The edges of a logarithmic axis of `cells` cells whose first log edge is `first` and which
/// spans `octaves` doublings over `cells − 1` cells after a first cell from 0 (the histograms'
/// [`log_cell`](super::histogram) axes), each cut at `end`, in the axis's units.
fn log_edges(first: f64, octaves: f64, cells: usize, end: f64) -> Vec<f64> {
    let last = cells - 1;
    #[expect(clippy::cast_precision_loss, reason = "a cell count below 64")]
    let per = octaves / last as f64;
    let mut edges = Vec::with_capacity(cells + 1);
    edges.push(0.0);
    for i in 0..last {
        #[expect(clippy::cast_precision_loss, reason = "a cell index below 64")]
        let e = first * math::exp2(per * i as f64);
        edges.push(e.min(end));
    }
    edges.push(end);
    edges
}

/// The nodes of one cell `[a, b]` of an axis: in ln of the coordinate for `a > 0`, linear from 0.
fn cell_nodes(a: f64, b: f64, rule: &[(f64, f64)], out: &mut Vec<(f64, f64)>) {
    if b <= a {
        return;
    }
    if a > 0.0 {
        let (la, lb) = (math::ln(a), math::ln(b));
        let (half, mid) = (0.5 * (lb - la), f64::midpoint(la, lb));
        for &(x, w) in rule {
            let v = math::exp(mid + half * x);
            out.push((v, w * half * v));
        }
    } else {
        let (half, mid) = (0.5 * (b - a), f64::midpoint(a, b));
        for &(x, w) in rule {
            out.push((mid + half * x, w * half));
        }
    }
}

/// The nodes of `[a, b]` beyond the square's half-width `l`, in the arc's angle `θ = arccos(l ÷
/// R)`, in which the arc length, with its square root at `R = l`, is smooth (as
/// `forms::cube_integral` takes the corner annulus).
fn corner_nodes(a: f64, b: f64, l: f64, rule: &[(f64, f64)], out: &mut Vec<(f64, f64)>) {
    if b <= a {
        return;
    }
    let (ta, tb) = (math::acos((l / a).min(1.0)), math::acos((l / b).min(1.0)));
    let (half, mid) = (0.5 * (tb - ta), f64::midpoint(ta, tb));
    for &(x, w) in rule {
        let theta = mid + half * x;
        let (sin, cos) = math::sin_cos(theta);
        out.push((l / cos, w * half * l * sin / (cos * cos)));
    }
}

/// The length of the circle of radius `r` inside the square of half-width `l`, over 2π r for
/// `r ≤ l`.
fn arc(r: f64, l: f64) -> f64 {
    if r <= l {
        2.0 * std::f64::consts::PI * r
    } else if r < std::f64::consts::SQRT_2 * l {
        4.0 * r * (std::f64::consts::FRAC_PI_2 - 2.0 * math::acos(l / r))
    } else {
        0.0
    }
}

/// The (R, |z|) cells of a histogram as integration nodes, in `R_d`: the nodes of each axis with
/// their weights (R's times the arc length, z's times 2 for both signs).
#[derive(Debug, Clone, PartialEq)]
pub struct RzCells {
    /// `(cell, R, weight)` of every R node.
    pub r: Vec<(usize, f64, f64)>,
    /// `(cell, |z|, weight)` of every z node.
    pub z: Vec<(usize, f64, f64)>,
    /// `ln |z|` of every z node.
    pub ln_z: Vec<f64>,
}

impl RzCells {
    /// The cells for a thin-disc scale length of `r_d` ly, `order` nodes per axis and cell.
    #[must_use]
    pub fn new(r_d: f64, order: usize) -> Self {
        let rule = legendre(order);
        // The corner's few cells take eight nodes whatever the order: its integrand curves
        // strongly in θ, and three nodes left 7 × 10⁻⁵ of its area.
        let corner_rule = legendre(8);
        let l = f64::from(ROOT_HALF_WIDTH_LY) / r_d;
        let r_edges = log_edges(
            R_AXIS.first_ly / r_d,
            R_AXIS.octaves,
            R_CELLS,
            std::f64::consts::SQRT_2 * l,
        );
        let z_edges = log_edges(Z_AXIS.first_ly / r_d, Z_AXIS.octaves, Z_CELLS, l);
        let mut r = Vec::new();
        let mut nodes = Vec::new();
        for (cell, w) in r_edges.windows(2).enumerate() {
            nodes.clear();
            if w[0] < l {
                cell_nodes(w[0], w[1].min(l), &rule, &mut nodes);
            }
            if w[1] > l {
                corner_nodes(w[0].max(l), w[1], l, &corner_rule, &mut nodes);
            }
            r.extend(nodes.iter().map(|&(x, wt)| (cell, x, wt * arc(x, l))));
        }
        let mut z = Vec::new();
        for (cell, w) in z_edges.windows(2).enumerate() {
            nodes.clear();
            cell_nodes(w[0], w[1], &rule, &mut nodes);
            z.extend(nodes.iter().map(|&(x, wt)| (cell, x, 2.0 * wt)));
        }
        let ln_z = z.iter().map(|&(_, x, _)| math::ln(x)).collect();
        Self { r, z, ln_z }
    }

    /// The integral over each cell, `[r_cell × Z_CELLS + z_cell]`, of the density `f(R, |z|)`,
    /// written into `out`.
    pub fn integrate(&self, mut f: impl FnMut(f64, f64) -> f64, out: &mut [f64]) {
        out.fill(0.0);
        for &(rc, r, wr) in &self.r {
            for &(zc, z, wz) in &self.z {
                out[rc * Z_CELLS + zc] += wr * wz * f(r, z);
            }
        }
    }

    /// The integral over each cell of a density separated as `radial(R) × vertical(c, |z|,
    /// ln |z|)`, where `radial` returns its factor and a value `c` handed to `vertical` for every z
    /// node at that R: the layer's form, whose vertical profile depends on R through its height.
    pub fn integrate_by_radius(
        &self,
        mut radial: impl FnMut(f64) -> (f64, f64),
        mut vertical: impl FnMut(f64, f64, f64) -> f64,
        out: &mut [f64],
    ) {
        out.fill(0.0);
        for &(rc, r, wr) in &self.r {
            let (factor, context) = radial(r);
            let fr = factor * wr;
            if fr <= 0.0 {
                continue;
            }
            let row = &mut out[rc * Z_CELLS..(rc + 1) * Z_CELLS];
            for (&(zc, z, wz), &ln_z) in self.z.iter().zip(&self.ln_z) {
                row[zc] += fr * wz * vertical(context, z, ln_z);
            }
        }
    }
}

/// The bar-frame cells in |x|, |y|, |z| as integration nodes, in `R_d`, each axis's weights
/// times 2 for both signs.
#[derive(Debug, Clone, PartialEq)]
pub struct BarCells {
    /// `(cell, coordinate, weight)` of the |x| nodes.
    pub x: Vec<(usize, f64, f64)>,
    /// Of the |y| nodes.
    pub y: Vec<(usize, f64, f64)>,
    /// Of the |z| nodes.
    pub z: Vec<(usize, f64, f64)>,
}

impl BarCells {
    /// The cells for a thin-disc scale length of `r_d` ly, `order` nodes per axis and cell.
    #[must_use]
    pub fn new(r_d: f64, order: usize) -> Self {
        let rule = legendre(order);
        let l = f64::from(ROOT_HALF_WIDTH_LY) / r_d;
        let axis = |axis: LogAxis, cells: usize| {
            let mut out = Vec::new();
            let mut nodes = Vec::new();
            for (cell, w) in log_edges(axis.first_ly / r_d, axis.octaves, cells, l)
                .windows(2)
                .enumerate()
            {
                nodes.clear();
                cell_nodes(w[0], w[1], &rule, &mut nodes);
                out.extend(nodes.iter().map(|&(x, wt)| (cell, x, 2.0 * wt)));
            }
            out
        };
        Self {
            x: axis(BAR_XY_AXIS, BAR_CELLS[0]),
            y: axis(BAR_XY_AXIS, BAR_CELLS[1]),
            z: axis(Z_AXIS, BAR_CELLS[2]),
        }
    }

    /// The integral over each cell, `[(i × cells_y + j) × cells_z + k]`, of `f(|x|, |y|, |z|)`.
    pub fn integrate(&self, mut f: impl FnMut(f64, f64, f64) -> f64, out: &mut [f64]) {
        out.fill(0.0);
        for &(i, x, wx) in &self.x {
            for &(j, y, wy) in &self.y {
                let wxy = wx * wy;
                let base = (i * BAR_CELLS[1] + j) * BAR_CELLS[2];
                for &(k, z, wz) in &self.z {
                    out[base + k] += wxy * wz * f(x, y, z);
                }
            }
        }
    }
}

/// `values` divided by their sum, in place; left as zeros if the sum is not positive.
pub fn normalise(values: &mut [f64]) {
    let total: f64 = values.iter().sum();
    if total > 0.0 {
        let inv = 1.0 / total;
        for v in values {
            *v *= inv;
        }
    }
}

/// The counts a speed row's fastest class is fitted to: its own bound members inside the cube and
/// every class's unbound still inside, which plan 08 moves there (P15.T6's format; plan 08, Design
/// note 23). `records` are one row's classes by speed bin, the fastest last.
#[must_use]
pub fn fastest_counts(records: &[&ClassHistogram]) -> Vec<u64> {
    let mut counts = records.last().map_or_else(Vec::new, |r| r.bound.clone());
    for r in records {
        for (c, u) in counts.iter_mut().zip(&r.unbound) {
            *c += u;
        }
    }
    counts
}

/// The observed shares of a histogram's counts: each count over their total, all zero if empty.
#[must_use]
pub fn shares(counts: &[u64]) -> Vec<f64> {
    let total: u64 = counts.iter().sum();
    if total == 0 {
        return vec![0.0; counts.len()];
    }
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    let inv = 1.0 / total as f64;
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    counts.iter().map(|&c| c as f64 * inv).collect()
}

/// The total-variation distance `½ Σ |pᵢ − fᵢ|` between two share vectors.
#[must_use]
pub fn total_variation(p: &[f64], f: &[f64]) -> f64 {
    0.5 * p.iter().zip(f).map(|(a, b)| (a - b).abs()).sum::<f64>()
}

/// The misplaced share that `n` objects drawn from the model `p` would show by chance alone: the
/// expected total-variation distance, `½ Σ E|Xᵢ − n pᵢ| ÷ n` with `Xᵢ` Poisson of mean `n pᵢ`,
/// `E|X − λ| = 2 e^(−λ) λ^(⌊λ⌋ + 1) ÷ ⌊λ⌋!` exactly. It stands in for P15.T6.c's noise floor from
/// two half-samples, which the histograms, being summed, cannot give.
#[must_use]
pub fn noise_floor(p: &[f64], n: u64) -> f64 {
    if n == 0 {
        return 0.0;
    }
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    let n = n as f64;
    let mut sum = 0.0;
    for &pi in p {
        let lambda = n * pi;
        if lambda <= 0.0 {
            continue;
        }
        let k = lambda.floor();
        let ln = math::ln(2.0) - lambda + (k + 1.0) * math::ln(lambda) - math::ln_gamma(k + 1.0);
        sum += math::exp(ln);
    }
    0.5 * sum / n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cells_tile_the_cube() {
        let r_d = 7_000.0;
        let l = f64::from(ROOT_HALF_WIDTH_LY) / r_d;
        let cells = RzCells::new(r_d, 3);
        let mut out = vec![0.0; R_CELLS * Z_CELLS];
        cells.integrate(|_, _| 1.0, &mut out);
        let volume: f64 = out.iter().sum();
        let cube = 8.0 * l * l * l;
        assert!(
            (volume / cube - 1.0).abs() < 1e-6,
            "{volume} against {cube}"
        );
        let bar = BarCells::new(r_d, 3);
        let mut out = vec![0.0; BAR_CELLS.iter().product()];
        bar.integrate(|_, _, _| 1.0, &mut out);
        let volume: f64 = out.iter().sum();
        assert!((volume / cube - 1.0).abs() < 1e-7, "{volume}");
    }

    #[test]
    fn the_fastest_class_takes_every_unbound_member() {
        use crate::tasks::displaced_forms::histogram::Integration;
        let mut slow = ClassHistogram::new(false);
        let mut fast = ClassHistogram::new(false);
        slow.record([100.0, 0.0, 0.0], [0.0; 3], false, Integration::default());
        fast.record([100.0, 0.0, 0.0], [0.0; 3], true, Integration::default());
        fast.record([200.0, 0.0, 0.0], [0.0; 3], false, Integration::default());
        let counts = fastest_counts(&[&slow, &fast]);
        assert_eq!(counts.iter().sum::<u64>(), 3);
    }

    #[test]
    fn the_noise_floor_matches_the_uniform_estimate() {
        // K equal cells of many objects each: √(K ÷ 2πn).
        let k = 400;
        #[expect(clippy::cast_precision_loss, reason = "a small count")]
        let p = vec![1.0 / k as f64; k];
        let n = 4_000_000;
        #[expect(clippy::cast_precision_loss, reason = "small counts")]
        let expected = (k as f64 / (2.0 * std::f64::consts::PI * n as f64)).sqrt();
        let got = noise_floor(&p, n);
        assert!(
            (got / expected - 1.0).abs() < 0.01,
            "{got} against {expected}"
        );
        assert!((total_variation(&[0.5, 0.5], &[1.0, 0.0]) - 0.5).abs() < 1e-15);
    }
}
