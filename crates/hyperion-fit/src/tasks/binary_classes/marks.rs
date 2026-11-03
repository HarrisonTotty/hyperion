//! One mark of a binary class's sampler fitted to the engine's members (P15.T10.b): a median that
//! follows the mark's condition on 17 nodes, and the residual about it by its quantiles at the
//! ranks k ÷ 16, the shape of plan 11's `tables::binary::SamplerMark`. From the refit at 27 a
//! conditional mark has a residual law at each node as well, so that its spread follows the
//! condition, and every member carries a weight: the hosts its kind stands for in the class's rows
//! (the parent module's).
//!
//! The median at a node is the weighted median of the members' values, each weighted by its own
//! weight, scaled to a mean of one, times a triangle two node spacings wide on either side of the
//! node, so that a node's estimate draws on the members about it and the curve is smooth to a node
//! or two. A node whose weight is under [`MIN_NODE_WEIGHT`] takes the value along the line between
//! its nearest supported neighbours, or the nearest one's beyond the last. The condition's span is
//! its 1st to its 99th percentile, so that the end nodes are supported, and the median is held
//! beyond it, as the sampler holds it. The residuals are the values less the median at their
//! conditions, evaluated as the sampler evaluates it. A node's residual law is the quantiles of the
//! residuals under its triangle, weighted alike, where those members' Kish effective size, (Σw)² ÷
//! Σw², is at least the manifest's `min_members`; a node with fewer takes the law of all the
//! members, the mark's own. The quantiles are the weighted analogue of Hyndman and Fan's (1996,
//! Am. Stat. 50, 361) fifth definition, each value at the middle of its weight's span of ranks and
//! linear between them, the least and the greatest value at ranks 0 and 1, so that a draw spans
//! what the members span. The limits are the members' least and greatest values.
//!
//! A mark whose consumer weighs its low tail by 10⁻ˣ, the recurrence, whose 1 ÷ P is a nova rate,
//! has its rank-0 quantile fitted instead so that its first segment's mean of 10⁻ˣ is its members'
//! over the lowest sixteenth of their weight ([`FittedMark::with_rate_preserving_floor`]): the
//! segment, linear in x between its two quantiles, otherwise spreads its sixteenth evenly down to
//! the members' least value and gives their shortest recurrences far more weight than the members
//! do.

use hyperion_sim::math;
use hyperion_sim::tables::binary::SamplerMark;

/// The ranks of a mark's residual quantiles and the nodes of its median: 17, at k ÷ 16.
pub const NODES: usize = 17;

/// The least weight of members, in the triangle's units, at which a node's median is its own.
pub const MIN_NODE_WEIGHT: f64 = 8.0;

/// The half-width of a node's triangle, in node spacings.
const KERNEL_SPACINGS: f64 = 2.0;

/// The share of the condition left out of its span at each end.
const SPAN_TAIL: f64 = 0.01;

/// One member's point of a mark: its condition (zero for an unconditional mark), its value and
/// its weight, positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkPoint {
    /// The condition the mark's median follows.
    pub condition: f64,
    /// The mark's value.
    pub value: f64,
    /// The hosts the member stands for, in any unit shared by the mark's members.
    pub weight: f64,
}

/// One fitted mark: what `SamplerMark` holds, with the members it was fitted to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FittedMark {
    /// The residual's quantiles at the ranks k ÷ 16 over all the members, non-decreasing.
    pub residual: [f64; NODES],
    /// The residual's law at each node, each non-decreasing, or `None` for an unconditional mark
    /// or one kept from the scratch table.
    pub node_residuals: Option<[[f64; NODES]; NODES]>,
    /// The condition's values at the median's first and last nodes; (0, 1) for an unconditional
    /// mark.
    pub span: (f64, f64),
    /// The median at the 17 nodes; zero for an unconditional mark.
    pub median: [f64; NODES],
    /// The least and the greatest value a draw takes.
    pub limits: (f64, f64),
    /// The members it was fitted to.
    pub members: usize,
    /// The nodes whose law is their own members', not all the members'.
    pub own_nodes: usize,
}

impl FittedMark {
    /// The scratch table's `mark`, for a mark no member defines: its residual, median and limits,
    /// with no members and no node laws.
    #[must_use]
    pub fn of_scratch(mark: &SamplerMark) -> Self {
        let (span, median) = mark
            .median()
            .map_or(((0.0, 1.0), [0.0; NODES]), |m| (m.span(), *m.nodes()));
        Self {
            residual: *mark.residual(),
            node_residuals: None,
            span,
            median,
            limits: mark.limits(),
            members: 0,
            own_nodes: 0,
        }
    }

    /// The median at `condition`: linear between the nodes and held beyond the span, the
    /// arithmetic of `ClassSampler::median`, bit for bit.
    #[must_use]
    pub fn median_at(&self, condition: f64) -> f64 {
        median_at(self.span, &self.median, condition)
    }

    /// The node laws, or the mark's own law at every node: what the table's rows hold for it.
    #[must_use]
    pub fn node_rows(&self) -> [[f64; NODES]; NODES] {
        self.node_residuals.unwrap_or([self.residual; NODES])
    }

    /// This unconditional mark of log₁₀ x with its rank-0 quantile fitted so that its first
    /// segment's mean of 10⁻ˣ is that of `points`' lowest sixteenth by weight (module
    /// documentation), its lower limit taken down to it if it falls below. Under the segment's
    /// uniform law between its quantiles q₀ and q₁ the mean is (10^−q₀ − 10^−q₁) ÷ ((q₁ − q₀)
    /// ln 10), which falls as q₀ rises to q₁, where it is 10^−q₁, so the root is found by halving.
    /// A sixteenth whose mean is no more than 10^−q₁ keeps q₀ at q₁.
    ///
    /// # Panics
    ///
    /// If `points` is empty or holds a condition or value that is not finite or a weight that is
    /// not positive and finite.
    #[must_use]
    pub fn with_rate_preserving_floor(self, points: &[MarkPoint]) -> Self {
        check(points);
        let mut sorted: Vec<(f64, f64)> = points.iter().map(|p| (p.value, p.weight)).collect();
        sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
        let total = sorted.iter().fold(0.0, |sum, p| sum + p.1);
        let sixteenth = total / 16.0;
        let (mut taken, mut sum) = (0.0, 0.0);
        for &(x, w) in &sorted {
            let w = w.min(sixteenth - taken);
            if w <= 0.0 {
                break;
            }
            taken += w;
            sum += w * math::exp10(-x);
        }
        let wanted = sum / taken;
        let q1 = self.residual[1];
        let mean_above =
            |q0: f64| (math::exp10(-q0) - math::exp10(-q1)) / ((q1 - q0) * std::f64::consts::LN_10);
        let q0 = if wanted <= math::exp10(-q1) {
            q1
        } else {
            // The mean grows without bound as q₀ falls, so 30 dex below q₁ brackets the root; 200
            // halvings take the bracket below a double's spacing.
            let (mut lo, mut hi) = (q1 - 30.0, q1);
            for _ in 0..200 {
                let mid = f64::midpoint(lo, hi);
                if mean_above(mid) > wanted {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            f64::midpoint(lo, hi)
        };
        let mut residual = self.residual;
        residual[0] = q0;
        Self {
            residual,
            limits: (self.limits.0.min(q0), self.limits.1),
            ..self
        }
    }
}

/// The median of nodes `median` over `span` at `condition`, as `ClassSampler::median` takes it.
#[must_use]
fn median_at((first, last): (f64, f64), median: &[f64; NODES], condition: f64) -> f64 {
    let s = ((condition - first) / (last - first)).clamp(0.0, 1.0) * 16.0;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "s is in [0, 16], so its floor is an index of 0–16"
    )]
    let k = (s.floor() as usize).min(15);
    let t = s - f64::from(u32::try_from(k).expect("an index of 0–15"));
    median[k] + t * (median[k + 1] - median[k])
}

/// The quantile at rank `p` in [0, 1] of `sorted`, `(value, weight)` pairs ascending by value,
/// not empty, their weights positive: each value at the middle of its weight's span of ranks,
/// linear between them, the least value at and below the first's and the greatest at and above
/// the last's (module documentation). With unit weights it is Hyndman and Fan's (1996, Am. Stat.
/// 50, 361) fifth definition.
///
/// # Panics
///
/// If `sorted` is empty.
#[must_use]
pub fn weighted_quantile(sorted: &[(f64, f64)], p: f64) -> f64 {
    assert!(!sorted.is_empty(), "a quantile of no values");
    let total = sorted.iter().fold(0.0, |sum, s| sum + s.1);
    let target = p.clamp(0.0, 1.0) * total;
    let mut before = 0.0;
    let mut previous: Option<(f64, f64)> = None;
    for &(x, w) in sorted {
        let at = before + 0.5 * w;
        if target <= at {
            return match previous {
                None => x,
                Some((px, pat)) => px + (target - pat) / (at - pat) * (x - px),
            };
        }
        previous = Some((x, at));
        before += w;
    }
    sorted[sorted.len() - 1].0
}

/// The 17 quantiles at k ÷ 16 of `values`, `(value, weight)` pairs, which it sorts.
#[must_use]
fn ranks(values: &mut [(f64, f64)]) -> [f64; NODES] {
    values.sort_by(|a, b| a.0.total_cmp(&b.0));
    std::array::from_fn(|k| {
        let p = f64::from(u32::try_from(k).expect("a rank of 0–16")) / 16.0;
        weighted_quantile(values, p)
    })
}

/// Kish's effective size of weights `weights`: (Σw)² ÷ Σw².
#[must_use]
fn kish(weights: impl IntoIterator<Item = f64>) -> f64 {
    let (sum, squares) = weights
        .into_iter()
        .fold((0.0, 0.0), |(s, q), w| (s + w, q + w * w));
    if squares > 0.0 {
        sum * sum / squares
    } else {
        0.0
    }
}

/// The least and the greatest of `values`, by `total_cmp`, so that a −0 and a +0 among them give
/// the same bits on every target.
#[must_use]
fn extent(values: impl IntoIterator<Item = f64>) -> (f64, f64) {
    values
        .into_iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (
                if v.total_cmp(&lo).is_lt() { v } else { lo },
                if v.total_cmp(&hi).is_gt() { v } else { hi },
            )
        })
}

/// Panics unless `points` is not empty, every condition and value finite and every weight
/// positive and finite.
fn check(points: &[MarkPoint]) {
    assert!(
        !points.is_empty()
            && points.iter().all(|p| {
                p.condition.is_finite()
                    && p.value.is_finite()
                    && p.weight.is_finite()
                    && p.weight > 0.0
            }),
        "a mark is fitted to finite points of positive weight"
    );
}

/// An unconditional mark of `points`, their conditions unused: its weighted quantiles, about a
/// zero median.
///
/// # Panics
///
/// If `points` is empty or holds a condition or value that is not finite or a weight that is not
/// positive and finite.
#[must_use]
pub fn unconditional(points: &[MarkPoint]) -> FittedMark {
    check(points);
    let mut values: Vec<(f64, f64)> = points.iter().map(|p| (p.value, p.weight)).collect();
    FittedMark {
        residual: ranks(&mut values),
        node_residuals: None,
        span: (0.0, 1.0),
        median: [0.0; NODES],
        limits: extent(points.iter().map(|p| p.value)),
        members: points.len(),
        own_nodes: 0,
    }
}

/// A mark of `points`: its median on 17 nodes over the condition's span, its residual's quantiles
/// over all the members, and its law at each node, its own where the node's members' Kish size is
/// at least `min_members` (module documentation).
///
/// # Panics
///
/// If `points` is empty or holds a condition or value that is not finite or a weight that is not
/// positive and finite.
#[must_use]
pub fn conditional(points: &[MarkPoint], min_members: usize) -> FittedMark {
    check(points);
    // The weights scaled to a mean of one, so that a node's support is in members.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a sample's length, far under 2⁵³"
    )]
    let mean = points.iter().fold(0.0, |sum, p| sum + p.weight) / points.len() as f64;
    let mut conditions: Vec<(f64, f64)> = points.iter().map(|p| (p.condition, p.weight)).collect();
    conditions.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut first, mut last) = (
        weighted_quantile(&conditions, SPAN_TAIL),
        weighted_quantile(&conditions, 1.0 - SPAN_TAIL),
    );
    if last - first <= 1e-9 * first.abs().max(1.0) {
        // A condition the members all but share: a span of one unit about it, so that the
        // median is flat there.
        (first, last) = (first - 0.5, last + 0.5);
    }
    let spacing = (last - first) / 16.0;
    let half_width = KERNEL_SPACINGS * spacing;
    let node = |k: usize| first + spacing * f64::from(u32::try_from(k).expect("a node of 0–16"));
    // Each member's weight under node k's triangle, or `None` outside it.
    let under = |k: usize, p: &MarkPoint| {
        let w = 1.0 - (p.condition - node(k)).abs() / half_width;
        (w > 0.0).then(|| w * p.weight / mean)
    };
    let mut supported = [None; NODES];
    let mut near: Vec<(f64, f64)> = Vec::with_capacity(points.len());
    for (k, slot) in supported.iter_mut().enumerate() {
        near.clear();
        near.extend(
            points
                .iter()
                .filter_map(|p| under(k, p).map(|w| (p.value, w))),
        );
        let total = near.iter().fold(0.0, |sum, (_, w)| sum + w);
        if total >= MIN_NODE_WEIGHT {
            *slot = Some(weighted_median(&mut near, total));
        }
    }
    let median = fill(&supported, points);
    let residual_of = |p: &MarkPoint| p.value - median_at((first, last), &median, p.condition);
    let mut residuals: Vec<(f64, f64)> =
        points.iter().map(|p| (residual_of(p), p.weight)).collect();
    let residual = ranks(&mut residuals);
    let mut own_nodes = 0;
    let node_residuals = std::array::from_fn(|k| {
        near.clear();
        near.extend(
            points
                .iter()
                .filter_map(|p| under(k, p).map(|w| (residual_of(p), w))),
        );
        if kish(near.iter().map(|n| n.1))
            < f64::from(u32::try_from(min_members).unwrap_or(u32::MAX))
        {
            return residual;
        }
        own_nodes += 1;
        ranks(&mut near)
    });
    FittedMark {
        residual,
        node_residuals: Some(node_residuals),
        span: (first, last),
        median,
        limits: extent(points.iter().map(|p| p.value)),
        members: points.len(),
        own_nodes,
    }
}

/// The weighted median of `(value, weight)` pairs of total weight `total`, which it sorts: the
/// least value at which the running weight reaches half the total.
#[must_use]
fn weighted_median(near: &mut [(f64, f64)], total: f64) -> f64 {
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut running = 0.0;
    for &(y, w) in near.iter() {
        running += w;
        if running >= 0.5 * total {
            return y;
        }
    }
    near.last().map_or(0.0, |p| p.0)
}

/// The nodes with each unsupported one filled: along the line between its nearest supported
/// neighbours, held from the nearest beyond the last, or every node the members' weighted median
/// if none is supported.
#[must_use]
fn fill(supported: &[Option<f64>; NODES], points: &[MarkPoint]) -> [f64; NODES] {
    let known: Vec<(usize, f64)> = supported
        .iter()
        .enumerate()
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect();
    if known.is_empty() {
        let mut values: Vec<(f64, f64)> = points.iter().map(|p| (p.value, p.weight)).collect();
        values.sort_by(|a, b| a.0.total_cmp(&b.0));
        return [weighted_quantile(&values, 0.5); NODES];
    }
    std::array::from_fn(|k| {
        let below = known.iter().rev().find(|(j, _)| *j <= k);
        let above = known.iter().find(|(j, _)| *j >= k);
        match (below, above) {
            (Some(&(a, va)), Some(&(b, vb))) if a != b => {
                let t = f64::from(u32::try_from(k - a).expect("a node gap of 0–16"))
                    / f64::from(u32::try_from(b - a).expect("a node gap of 1–16"));
                va + t * (vb - va)
            }
            (Some(&(_, v)), _) | (None, Some(&(_, v))) => v,
            (None, None) => unreachable!("some node is supported"),
        }
    })
}

#[cfg(test)]
mod tests {
    use hyperion_sim::stellar::binary::sampler::ClassSampler;
    use hyperion_sim::tables::binary::{AWD_SAMPLER, AwdMark};
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// Unit-weight points of `pairs`.
    fn unit(pairs: impl IntoIterator<Item = (f64, f64)>) -> Vec<MarkPoint> {
        pairs
            .into_iter()
            .map(|(condition, value)| MarkPoint {
                condition,
                value,
                weight: 1.0,
            })
            .collect()
    }

    /// With unit weights the quantiles are Hyndman and Fan's fifth definition, the least and the
    /// greatest at ranks 0 and 1. A value of weight three sits at the middle of its span of ranks,
    /// so its quantiles are never above those of three copies of it, which fill the span.
    #[test]
    fn quantiles_put_each_value_at_the_middle_of_its_weight() {
        let sorted: Vec<(f64, f64)> = [1.0, 2.0, 4.0, 8.0, 16.0].map(|x| (x, 1.0)).to_vec();
        assert_same_bits(weighted_quantile(&sorted, 0.0), 1.0);
        assert_same_bits(weighted_quantile(&sorted, 1.0), 16.0);
        assert!((weighted_quantile(&sorted, 0.5) - 4.0).abs() < 1e-15);
        // Rank 0.6 sits at 3 of 5, halfway from the third value's middle (2.5) to the fourth's.
        assert!((weighted_quantile(&sorted, 0.6) - 6.0).abs() < 1e-12);
        let weighted = [(1.0, 1.0), (2.0, 3.0)];
        let spread = [(1.0, 1.0), (2.0, 1.0), (2.0, 1.0), (2.0, 1.0)];
        for p in [0.1, 0.3, 0.5, 0.9] {
            let (a, b) = (
                weighted_quantile(&weighted, p),
                weighted_quantile(&spread, p),
            );
            assert!(a <= b + 1e-12 && a >= 1.0, "{p}: {a} against {b}");
        }
    }

    /// A mark whose values are a line of its condition plus a spread that widens with it: the
    /// median follows the line; the residual's quantiles over all the members are the spread's
    /// mixture; each interior node's law is its own spread, within the sample's noise; and a mark
    /// too sparse for its nodes keeps its own law at every one.
    #[test]
    fn a_line_with_a_widening_spread_is_recovered() {
        let n = 40_000;
        let points = unit((0..n).map(|i| {
            let x = f64::from(i % 200) / 199.0 * 4.0;
            // A spread of −1 to 1, by a fixed sequence prime to the condition's, scaled by
            // 0.5 + x ÷ 4.
            let u = f64::from((i * 7_919) % 10_007) / 10_006.0 * 2.0 - 1.0;
            (x, 3.0 - 0.5 * x + (0.5 + 0.25 * x) * u)
        }));
        let mark = conditional(&points, 50);
        for k in 0..NODES {
            let x = mark.span.0
                + (mark.span.1 - mark.span.0) * f64::from(u32::try_from(k).unwrap()) / 16.0;
            let tolerance = if (2..=14).contains(&k) { 0.05 } else { 0.3 };
            let error = (mark.median[k] - (3.0 - 0.5 * x)).abs();
            assert!(error < tolerance, "{k}: {error}");
        }
        let rows = mark
            .node_residuals
            .expect("a conditional mark has node laws");
        assert_eq!(mark.own_nodes, NODES);
        for (k, row) in rows.iter().enumerate().take(14).skip(2) {
            let x = mark.span.0
                + (mark.span.1 - mark.span.0) * f64::from(u32::try_from(k).unwrap()) / 16.0;
            let half = 0.5 + 0.25 * x;
            // A uniform spread of ±h has quartiles ±h ÷ 2; the triangle mixes the spreads about the
            // node, whose mean is the node's.
            let iqr = row[12] - row[4];
            assert!((iqr - half).abs() < 0.1 * half, "{k}: {iqr} against {half}");
            assert!(
                row[16] >= half && row[0] <= -half,
                "{k}: {} to {}",
                row[0],
                row[16]
            );
            assert!(row.windows(2).all(|w| w[0] <= w[1]));
        }
        assert!(mark.residual.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(mark.members, 40_000);
        let sparse = conditional(&points[..400], 50);
        assert!(sparse.own_nodes < NODES);
        let rows = sparse.node_residuals.expect("node laws");
        let own_law = |row: &[f64; NODES]| {
            row.iter()
                .zip(&sparse.residual)
                .all(|(a, b)| (a - b).abs() < f64::MIN_POSITIVE)
        };
        assert!(rows.iter().any(own_law));
    }

    /// A conditional mark counts its members' weights: points of value 0 weighing three for each
    /// point of value 1 put every node's median at 0, and each interior node's law at 0 below
    /// about three quarters of its ranks and at 1 above.
    #[test]
    fn a_conditional_mark_counts_its_weights() {
        let points: Vec<MarkPoint> = (0..4_000)
            .map(|i| {
                let (value, weight) = if i % 2 == 0 { (0.0, 3.0) } else { (1.0, 1.0) };
                MarkPoint {
                    condition: f64::from(i % 100),
                    value,
                    weight,
                }
            })
            .collect();
        let mark = conditional(&points, 50);
        assert!(
            mark.median.iter().all(|m| m.abs() < 1e-12),
            "{:?}",
            mark.median
        );
        let rows = mark.node_residuals.expect("node laws");
        for (k, row) in rows.iter().enumerate().take(15).skip(2) {
            assert!(row[10].abs() < 1e-12, "{k}: {row:?}");
            assert!((row[14] - 1.0).abs() < 1e-12, "{k}: {row:?}");
        }
    }

    /// An unsupported node lies on the line between its supported neighbours.
    #[test]
    fn an_unsupported_node_is_filled_along_the_line() {
        let mut supported = [Some(1.0); NODES];
        supported[3] = None;
        supported[4] = None;
        supported[0] = None;
        supported[2] = Some(2.0);
        supported[5] = Some(5.0);
        let filled = fill(&supported, &unit([(0.0, 0.0)]));
        assert!((filled[3] - 3.0).abs() < 1e-12);
        assert!((filled[4] - 4.0).abs() < 1e-12);
        assert!((filled[0] - 1.0).abs() < 1e-12);
    }

    /// The rate-preserving floor makes the first segment's mean of 10⁻ˣ the members': a sixteenth
    /// that clusters at its top raises rank 0 above the least value, one at the least value
    /// lowers it, with the lower limit; the other ranks stay.
    #[test]
    fn the_floor_preserves_the_lowest_sixteenth_s_rate() {
        // 1,600 values: the lowest sixteenth is 99 at 2.9 and one at 1.0, the rest 3 to 6.
        let values: Vec<f64> = (0..1_600)
            .map(|i| match i {
                0 => 1.0,
                1..100 => 2.9,
                _ => 3.0 + 3.0 * f64::from(i - 100) / 1_500.0,
            })
            .collect();
        let points = unit(values.iter().map(|&v| (0.0, v)));
        let mark = unconditional(&points);
        let floored = mark.with_rate_preserving_floor(&points);
        let mean_of = |q0: f64, q1: f64| {
            (math::exp10(-q0) - math::exp10(-q1)) / ((q1 - q0) * std::f64::consts::LN_10)
        };
        let wanted = (math::exp10(-1.0) + 99.0 * math::exp10(-2.9)) / 100.0;
        let got = mean_of(floored.residual[0], floored.residual[1]);
        assert!((got / wanted - 1.0).abs() < 1e-9, "{got} against {wanted}");
        assert!(floored.residual[0] > 1.0 && floored.residual[0] < floored.residual[1]);
        for (a, b) in floored.residual[1..].iter().zip(&mark.residual[1..]) {
            assert_same_bits(*a, *b);
        }
        assert_same_bits(floored.limits.0, 1.0);
        // All of the sixteenth at the least value: rank 0 falls below it, and the limit with it.
        let piled = unit((0..1_600).map(|i| (0.0, if i < 100 { 1.0 } else { 3.0 })));
        let floored = unconditional(&piled).with_rate_preserving_floor(&piled);
        assert!(floored.residual[0] < 1.0, "{}", floored.residual[0]);
        assert_same_bits(floored.limits.0, floored.residual[0]);
    }

    /// A fitted mark's median is the sampler's at every condition, bit for bit, so that the
    /// residuals are taken about the curve a draw adds them to.
    #[test]
    fn the_median_is_the_sampler_s() {
        let sampler = ClassSampler::new(&AWD_SAMPLER);
        let mark = AwdMark::WhiteDwarfMass;
        let fitted = FittedMark::of_scratch(AWD_SAMPLER.mark(mark));
        for i in 0..=100 {
            let x = 4.0 + f64::from(i) * 0.045;
            assert_same_bits(fitted.median_at(x), sampler.median(mark, x));
        }
    }
}
