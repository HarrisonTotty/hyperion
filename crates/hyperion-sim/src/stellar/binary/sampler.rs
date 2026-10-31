//! The catalogue classes' sampler (plan 11's Provides; P11.T8): a binary class's marks drawn by
//! inverse transform from its [`ClassSamplerTable`], one word each.
//!
//! A catalogue entry of a binary class draws what defines it now and then a history consistent
//! with it (design note 10). Each mark's law in the table is a residual given by its quantiles at
//! the 17 ranks k ÷ 16, linear between them, added to a median that follows a condition the caller
//! supplies (an earlier mark, or a quantity derived from the marks), and held to the mark's
//! limits. A draw takes one uniform word for its rank, so a mark's value is a monotone function of
//! its word and a restricted draw ([`ClassSampler::draw_in_ranks`]) is an exact conditional one,
//! as plan 11's design note 1 draws the stripped orbits. The four binary classes share it
//! (P11.T8.a–d), each with its own table; the accreting white dwarfs' is
//! [`AWD_SAMPLER`](crate::tables::binary::AWD_SAMPLER).

use crate::rng::Stream;
use crate::tables::binary::{ClassSamplerTable, SamplerKey};

/// A binary class's sampler over its table (P11.T8): marks by inverse transform, kinds by weight.
///
/// # Examples
///
/// An accreting white dwarf's recurrence period, drawn on the slow side of L alone, and its white
/// dwarf's mass given it: a slow host has the lighter dwarfs.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::math;
/// use hyperion_sim::rng::{ObjectKey, Stream, tags};
/// use hyperion_sim::stellar::binary::sampler::ClassSampler;
/// use hyperion_sim::tables::binary::{AWD_SAMPLER, AwdMark};
/// use hyperion_sim::time::LIGHT_CROSSING_L;
///
/// let sampler = ClassSampler::new(&AWD_SAMPLER);
/// let log_l = math::log10(LIGHT_CROSSING_L.as_julian_years_f64());
/// let split = sampler.rank_of(AwdMark::Recurrence, 0.0, log_l);
/// assert!((split - 0.5).abs() < 1e-12);
/// let mut stream = Stream::open(Seed::new(3), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let log_years = sampler.draw_in_ranks(AwdMark::Recurrence, 0.0, (split, 1.0), &mut stream);
/// assert!(log_years >= log_l - 1e-12);
/// let mass = sampler.draw(AwdMark::WhiteDwarfMass, log_years, &mut stream);
/// assert!((0.55..=1.09).contains(&mass));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassSampler<M: 'static, P: 'static> {
    table: &'static ClassSamplerTable<M, P>,
}

impl<M: SamplerKey, P: SamplerKey> ClassSampler<M, P> {
    /// The sampler over `table`.
    #[must_use]
    pub const fn new(table: &'static ClassSamplerTable<M, P>) -> Self {
        Self { table }
    }

    /// The table it draws from.
    #[must_use]
    pub const fn table(&self) -> &'static ClassSamplerTable<M, P> {
        self.table
    }

    /// The median of `mark` at `condition`: linear between the median curve's nodes and held
    /// beyond its span, or zero for an unconditional mark.
    ///
    /// # Panics
    ///
    /// If the table holds fewer marks than `M` names.
    #[must_use]
    pub fn median(&self, mark: M, condition: f64) -> f64 {
        let Some(curve) = self.table.mark(mark).median() else {
            return 0.0;
        };
        let (first, last) = curve.span();
        let s = ((condition - first) / (last - first)).clamp(0.0, 1.0) * 16.0;
        interpolate(curve.nodes(), s)
    }

    /// The value of `mark` at rank `rank` given `condition`: the median plus the residual's
    /// quantile at the rank, held to the mark's limits. The rank is held to [0, 1].
    ///
    /// # Panics
    ///
    /// If the table holds fewer marks than `M` names.
    #[must_use]
    pub fn value_at(&self, mark: M, condition: f64, rank: f64) -> f64 {
        let law = self.table.mark(mark);
        let residual = interpolate(law.residual(), rank.clamp(0.0, 1.0) * 16.0);
        let (lo, hi) = law.limits();
        (self.median(mark, condition) + residual).clamp(lo, hi)
    }

    /// The rank in [0, 1] at which `mark`, given `condition`, takes `value`: the inverse of
    /// [`value_at`](Self::value_at) inside the limits, 0 at or below the residual's least quantile
    /// and 1 at or above its greatest. Where quantiles tie at the value, the lowest rank of the
    /// tie.
    ///
    /// # Panics
    ///
    /// If the table holds fewer marks than `M` names.
    #[must_use]
    pub fn rank_of(&self, mark: M, condition: f64, value: f64) -> f64 {
        let q = self.table.mark(mark).residual();
        let r = value - self.median(mark, condition);
        if r <= q[0] {
            return 0.0;
        }
        if r >= q[16] {
            return 1.0;
        }
        // The first quantile at or above r, 1–16: the value sits on it, or inside the segment
        // that ends there.
        let j = q.partition_point(|&x| x < r).clamp(1, 16);
        let at = f64::from(u32::try_from(j).expect("a rank index of 1–16"));
        if q[j] <= r {
            return at / 16.0;
        }
        let t = (r - q[j - 1]) / (q[j] - q[j - 1]);
        (at - 1.0 + t) / 16.0
    }

    /// Draws `mark` given `condition`: [`value_at`](Self::value_at) a uniform rank. One word.
    ///
    /// # Panics
    ///
    /// If the table holds fewer marks than `M` names.
    #[must_use]
    pub fn draw(&self, mark: M, condition: f64, stream: &mut Stream) -> f64 {
        self.value_at(mark, condition, stream.uniform())
    }

    /// Draws `mark` given `condition` with its rank restricted to `[lo, hi)`: the exact draw of
    /// the mark conditioned on lying between the values at those ranks. One word.
    ///
    /// # Panics
    ///
    /// If the table holds fewer marks than `M` names.
    #[must_use]
    pub fn draw_in_ranks(
        &self,
        mark: M,
        condition: f64,
        (lo, hi): (f64, f64),
        stream: &mut Stream,
    ) -> f64 {
        let u = stream.uniform();
        self.value_at(mark, condition, lo + (hi - lo) * u)
    }

    /// Picks a kind of choice `pick` by its weights, through plan 01's integer thresholds
    /// ([`Mark::pick_weighted`](crate::rng::Mark::pick_weighted) against their sum): the kind's
    /// index in the choice's order. One word.
    ///
    /// # Panics
    ///
    /// If the table holds fewer choices than `P` names, or the choice has no kinds.
    #[must_use]
    pub fn pick(&self, pick: P, stream: &mut Stream) -> usize {
        let weights = self.table.pick(pick).weights();
        let total = weights.iter().fold(0.0, |sum, w| sum + w);
        // The running total reaches the bound at the last kind, whose threshold saturates, so
        // the pick always lands; the last kind is its own fallback.
        stream
            .mark()
            .pick_weighted(weights, total)
            .unwrap_or(weights.len() - 1)
    }
}

/// The value at `s` in [0, 16] of 17 nodes at the whole numbers, linear between them.
#[must_use]
fn interpolate(nodes: &[f64; 17], s: f64) -> f64 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "s is in [0, 16], so its floor is an index of 0–16"
    )]
    let k = (s.floor() as usize).min(15);
    let t = s - f64::from(u32::try_from(k).expect("an index of 0–15"));
    nodes[k] + t * (nodes[k + 1] - nodes[k])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::rng::{ObjectKey, tags};
    use crate::tables::binary::{
        AWD_SAMPLER, AwdDonorKind, AwdMark, AwdPick, MedianCurve, SamplerMark, SamplerPick,
    };

    fn stream(seed: u64) -> Stream {
        Stream::open(Seed::new(seed), tags::SELFTEST_STREAM, ObjectKey::galaxy())
    }

    /// The marks lie in the order `AwdMark` names them, and every residual is non-decreasing.
    #[test]
    fn the_awd_table_follows_its_mark_list() {
        let names = [
            "log_recurrence_years",
            "white_dwarf_mass",
            "log_main_sequence_period_hours",
            "log_helium_period_hours",
            "log_giant_period_hours",
            "log_main_sequence_donor_mass",
            "log_helium_donor_mass",
            "log_giant_donor_mass",
            "log_primary_initial_mass",
            "log_donor_initial_mass",
            "log_age_years",
        ];
        assert_eq!(AWD_SAMPLER.marks().len(), AwdMark::ALL.len());
        for (mark, name) in AwdMark::ALL.into_iter().zip(names) {
            let law = AWD_SAMPLER.mark(mark);
            assert_eq!(law.name(), name);
            assert!(law.residual().windows(2).all(|w| w[0] <= w[1]), "{name}");
            let (lo, hi) = law.limits();
            assert!(lo < hi, "{name}");
        }
        let kinds = AWD_SAMPLER.pick(AwdPick::DonorKind).weights();
        assert_eq!(kinds.len(), AwdDonorKind::ALL.len());
        assert!((kinds.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }

    /// `rank_of` inverts `value_at` inside the limits, at every rank and condition tried, and
    /// the medians are their nodes at the nodes.
    #[test]
    fn the_rank_inverts_the_value() {
        let sampler = ClassSampler::new(&AWD_SAMPLER);
        for mark in AwdMark::ALL {
            let law = AWD_SAMPLER.mark(mark);
            let (lo, hi) = law.limits();
            let conditions: Vec<f64> = law.median().map_or_else(
                || vec![0.0],
                |c| {
                    let (a, b) = c.span();
                    (0..=8).map(|k| a + (b - a) * f64::from(k) / 8.0).collect()
                },
            );
            for &c in &conditions {
                for k in 0..=64 {
                    let rank = f64::from(k) / 64.0;
                    let v = sampler.value_at(mark, c, rank);
                    if v <= lo || v >= hi {
                        continue;
                    }
                    let back = sampler.rank_of(mark, c, v);
                    assert!((back - rank).abs() < 1e-9, "{mark:?} {c} {rank}: {back}");
                }
            }
            if let Some(curve) = law.median() {
                let (a, b) = curve.span();
                for (k, &node) in curve.nodes().iter().enumerate() {
                    let x = a + (b - a) * f64::from(u32::try_from(k).unwrap()) / 16.0;
                    assert!((sampler.median(mark, x) - node).abs() < 1e-12);
                }
            }
        }
    }

    /// A one-mark key for the tied table.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Only;

    impl SamplerKey for Only {
        fn index(self) -> usize {
            0
        }
    }

    /// A residual whose quantiles tie at 1 over ranks 5–8 ÷ 16.
    static TIED: ClassSamplerTable<Only, Only> = ClassSamplerTable::new(
        &[SamplerMark::new(
            "tied",
            [
                0.0, 0.2, 0.4, 0.6, 0.8, 1.0, 1.0, 1.0, 1.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.2, 2.4, 2.6,
            ],
            None::<MedianCurve>,
            (0.0, 2.6),
        )],
        &[SamplerPick::new("only", &[1.0])],
    );

    /// At a tie of quantiles the rank is the tie's lowest, so a split at the tied value takes the
    /// whole tie on its upper side; around it the rank is linear.
    #[test]
    fn a_tie_takes_its_lowest_rank() {
        let sampler = ClassSampler::new(&TIED);
        assert!((sampler.rank_of(Only, 0.0, 1.0) - 5.0 / 16.0).abs() < 1e-15);
        assert!((sampler.rank_of(Only, 0.0, 0.9) - 4.5 / 16.0).abs() < 1e-12);
        assert!((sampler.rank_of(Only, 0.0, 1.1) - 8.5 / 16.0).abs() < 1e-12);
        assert!((sampler.rank_of(Only, 0.0, 0.2) - 1.0 / 16.0).abs() < 1e-15);
        assert_eq!(sampler.pick(Only, &mut stream(4)), 0);
    }

    /// The recurrence's median is L, so the two classes split the hosts in halves, as the share
    /// table gives them; a restricted draw stays on its side.
    #[test]
    fn the_recurrence_splits_at_l_in_halves() {
        let sampler = ClassSampler::new(&AWD_SAMPLER);
        let mark = AwdMark::Recurrence;
        let log_l = crate::math::log10(crate::time::LIGHT_CROSSING_L.as_julian_years_f64());
        let split = sampler.rank_of(mark, 0.0, log_l);
        assert!((split - 0.5).abs() < 1e-12, "{split}");
        let mut s = stream(1);
        for _ in 0..10_000 {
            assert!(sampler.draw_in_ranks(mark, 0.0, (0.0, split), &mut s) <= log_l + 1e-12);
            assert!(sampler.draw_in_ranks(mark, 0.0, (split, 1.0), &mut s) >= log_l - 1e-12);
        }
    }

    /// Each draw takes one word, and a pick takes its kinds at their weights.
    #[test]
    fn draws_take_one_word_and_picks_their_weights() {
        let sampler = ClassSampler::new(&AWD_SAMPLER);
        let mut a = stream(2);
        let _ = sampler.draw(AwdMark::Age, 0.5, &mut a);
        let _ = sampler.pick(AwdPick::DonorKind, &mut a);
        let mut b = stream(2);
        let _ = b.next_u64();
        let _ = b.next_u64();
        assert_eq!(a.next_u64(), b.next_u64());
        let mut s = stream(3);
        let mut counts = [0_u32; 3];
        let n = 200_000;
        for _ in 0..n {
            counts[sampler.pick(AwdPick::DonorKind, &mut s)] += 1;
        }
        let weights = AWD_SAMPLER.pick(AwdPick::DonorKind).weights();
        for (count, weight) in counts.iter().zip(weights) {
            let share = f64::from(*count) / f64::from(n);
            let sigma = (weight * (1.0 - weight) / f64::from(n)).sqrt();
            assert!(
                (share - weight).abs() < 5.0 * sigma,
                "{share} against {weight}"
            );
        }
    }
}
