//! One mark of a binary class's sampler fitted to the engine's members (P15.T10.b): a median that
//! follows the mark's condition on 17 nodes, and the residual about it by its quantiles at the
//! ranks k ÷ 16, the shape of plan 11's `tables::binary::SamplerMark`.
//!
//! The median at a node is the weighted median of the members' values, each weighted by a triangle
//! two node spacings wide on either side of the node, so that a node's estimate draws on the
//! members about it and the curve is smooth to a node or two. A node whose weight is under
//! [`MIN_NODE_WEIGHT`] takes the value along the line between its nearest supported neighbours,
//! or the nearest one's beyond the last. The condition's span is its 1st to its 99th percentile,
//! so that the end nodes are supported, and the median is held beyond it, as the sampler holds
//! it. The residuals are the values less the median at their conditions, evaluated as the
//! sampler evaluates it, and their quantiles are the order statistics' interpolation (Hyndman and
//! Fan 1996's seventh definition), the least and the greatest residual at ranks 0 and 1, so that
//! a draw spans what the members span. The limits are the members' least and greatest values.

use hyperion_sim::tables::binary::SamplerMark;

/// The ranks of a mark's residual quantiles and the nodes of its median: 17, at k ÷ 16.
pub const NODES: usize = 17;

/// The least weight of members, in the triangle's units, at which a node's median is its own.
pub const MIN_NODE_WEIGHT: f64 = 8.0;

/// The half-width of a node's triangle, in node spacings.
const KERNEL_SPACINGS: f64 = 2.0;

/// The share of the condition left out of its span at each end.
const SPAN_TAIL: f64 = 0.01;

/// One fitted mark: what `SamplerMark` holds, with the members it was fitted to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FittedMark {
    /// The residual's quantiles at the ranks k ÷ 16, non-decreasing.
    pub residual: [f64; NODES],
    /// The condition's values at the median's first and last nodes; (0, 1) for an unconditional
    /// mark.
    pub span: (f64, f64),
    /// The median at the 17 nodes; zero for an unconditional mark.
    pub median: [f64; NODES],
    /// The least and the greatest value a draw takes.
    pub limits: (f64, f64),
    /// The members it was fitted to.
    pub members: usize,
}

impl FittedMark {
    /// The scratch table's `mark`, for a mark no member defines: its residual, median and limits,
    /// with no members.
    #[must_use]
    pub fn of_scratch(mark: &SamplerMark) -> Self {
        let (span, median) = mark
            .median()
            .map_or(((0.0, 1.0), [0.0; NODES]), |m| (m.span(), *m.nodes()));
        Self {
            residual: *mark.residual(),
            span,
            median,
            limits: mark.limits(),
            members: 0,
        }
    }

    /// The median at `condition`: linear between the nodes and held beyond the span, the
    /// arithmetic of `ClassSampler::median`, bit for bit.
    #[must_use]
    pub fn median_at(&self, condition: f64) -> f64 {
        median_at(self.span, &self.median, condition)
    }
}

/// The median of nodes `median` over `span` at `condition`, as `ClassSampler::median` takes it.
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

/// The quantile at rank `p` in [0, 1] of `sorted`, ascending and not empty: linear between the
/// order statistics at (n − 1) p.
#[must_use]
pub fn quantile(sorted: &[f64], p: f64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a sample's length, far under 2⁵³"
    )]
    let h = (sorted.len() - 1) as f64 * p.clamp(0.0, 1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "h is in [0, n − 1]"
    )]
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    #[expect(clippy::cast_precision_loss, reason = "an index under 2⁵³")]
    let t = h - lo as f64;
    sorted[lo] + t * (sorted[hi] - sorted[lo])
}

/// The 17 quantiles at k ÷ 16 of `values`, which it sorts.
fn ranks(values: &mut [f64]) -> [f64; NODES] {
    values.sort_by(f64::total_cmp);
    std::array::from_fn(|k| {
        let p = f64::from(u32::try_from(k).expect("a rank of 0–16")) / 16.0;
        quantile(values, p)
    })
}

/// The least and the greatest of `values`.
fn extent(values: impl IntoIterator<Item = f64>) -> (f64, f64) {
    values
        .into_iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        })
}

/// An unconditional mark of `values`, each finite: its quantiles, about a zero median.
///
/// # Panics
///
/// If `values` is empty or holds a value that is not finite.
#[must_use]
pub fn unconditional(values: &[f64]) -> FittedMark {
    assert!(
        !values.is_empty() && values.iter().all(|v| v.is_finite()),
        "a mark is fitted to finite values"
    );
    let mut sorted = values.to_vec();
    let residual = ranks(&mut sorted);
    FittedMark {
        residual,
        span: (0.0, 1.0),
        median: [0.0; NODES],
        limits: extent(values.iter().copied()),
        members: values.len(),
    }
}

/// A mark of the members' (condition, value) `pairs`, each finite: its median on 17 nodes over
/// the condition's span, and its residual's quantiles (module documentation).
///
/// # Panics
///
/// If `pairs` is empty or holds a value that is not finite.
#[must_use]
pub fn conditional(pairs: &[(f64, f64)]) -> FittedMark {
    assert!(
        !pairs.is_empty() && pairs.iter().all(|(x, y)| x.is_finite() && y.is_finite()),
        "a mark is fitted to finite pairs"
    );
    let mut conditions: Vec<f64> = pairs.iter().map(|p| p.0).collect();
    conditions.sort_by(f64::total_cmp);
    let (mut first, mut last) = (
        quantile(&conditions, SPAN_TAIL),
        quantile(&conditions, 1.0 - SPAN_TAIL),
    );
    if last - first <= 1e-9 * first.abs().max(1.0) {
        // A condition the members all but share: a span of one unit about it, so that the
        // median is flat there.
        (first, last) = (first - 0.5, last + 0.5);
    }
    let spacing = (last - first) / 16.0;
    let half_width = KERNEL_SPACINGS * spacing;
    let mut supported = [None; NODES];
    let mut near: Vec<(f64, f64)> = Vec::with_capacity(pairs.len());
    for (k, slot) in supported.iter_mut().enumerate() {
        let node = first + spacing * f64::from(u32::try_from(k).expect("a node of 0–16"));
        near.clear();
        near.extend(pairs.iter().filter_map(|&(x, y)| {
            let w = 1.0 - (x - node).abs() / half_width;
            (w > 0.0).then_some((y, w))
        }));
        let total = near.iter().fold(0.0, |sum, (_, w)| sum + w);
        if total >= MIN_NODE_WEIGHT {
            *slot = Some(weighted_median(&mut near, total));
        }
    }
    let median = fill(&supported, pairs);
    let mut residuals: Vec<f64> = pairs
        .iter()
        .map(|&(x, y)| y - median_at((first, last), &median, x))
        .collect();
    FittedMark {
        residual: ranks(&mut residuals),
        span: (first, last),
        median,
        limits: extent(pairs.iter().map(|p| p.1)),
        members: pairs.len(),
    }
}

/// The weighted median of `(value, weight)` pairs of total weight `total`, which it sorts: the
/// least value at which the running weight reaches half the total.
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
/// neighbours, held from the nearest beyond the last, or every node the members' median if none
/// is supported.
fn fill(supported: &[Option<f64>; NODES], pairs: &[(f64, f64)]) -> [f64; NODES] {
    let known: Vec<(usize, f64)> = supported
        .iter()
        .enumerate()
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect();
    if known.is_empty() {
        let mut values: Vec<f64> = pairs.iter().map(|p| p.1).collect();
        values.sort_by(f64::total_cmp);
        return [quantile(&values, 0.5); NODES];
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

    /// The quantiles interpolate the order statistics: the least and greatest at ranks 0 and 1.
    #[test]
    fn quantiles_interpolate_the_order_statistics() {
        let sorted = [1.0, 2.0, 4.0, 8.0, 16.0];
        assert!((quantile(&sorted, 0.0) - 1.0).abs() < 1e-15);
        assert!((quantile(&sorted, 1.0) - 16.0).abs() < 1e-15);
        assert!((quantile(&sorted, 0.5) - 4.0).abs() < 1e-15);
        assert!((quantile(&sorted, 0.625) - 6.0).abs() < 1e-15);
    }

    /// A mark whose values are a line of its condition plus a uniform spread: the median follows
    /// the line and the residual's quantiles are the spread's, within the sample's noise. The two
    /// nodes at each end of the span, whose triangles the span cuts, lean towards the interior's
    /// values, by up to a fifth of the spread there.
    #[test]
    fn a_line_with_a_spread_is_recovered() {
        let n = 40_000;
        let pairs: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let x = f64::from(i % 200) / 199.0 * 4.0;
                // A spread of −1 to 1, by a fixed sequence prime to the condition's.
                let u = f64::from((i * 7_919) % 10_007) / 10_006.0 * 2.0 - 1.0;
                (x, 3.0 - 0.5 * x + u)
            })
            .collect();
        let mark = conditional(&pairs);
        for k in 0..NODES {
            let x = mark.span.0
                + (mark.span.1 - mark.span.0) * f64::from(u32::try_from(k).unwrap()) / 16.0;
            let tolerance = if (2..=14).contains(&k) { 0.05 } else { 0.2 };
            let error = (mark.median[k] - (3.0 - 0.5 * x)).abs();
            assert!(error < tolerance, "{k}: {error}");
        }
        for k in 0..NODES {
            let expected = -1.0 + 2.0 * f64::from(u32::try_from(k).unwrap()) / 16.0;
            assert!(
                (mark.residual[k] - expected).abs() < 0.1,
                "{k}: {}",
                mark.residual[k]
            );
        }
        assert!(mark.residual.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(mark.members, 40_000);
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
        let filled = fill(&supported, &[(0.0, 0.0)]);
        assert!((filled[3] - 3.0).abs() < 1e-12);
        assert!((filled[4] - 4.0).abs() < 1e-12);
        assert!((filled[0] - 1.0).abs() < 1e-12);
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
