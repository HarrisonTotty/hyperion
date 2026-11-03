//! The catalogue classes' sampler (plan 11's Provides; P11.T8): a binary class's marks drawn by
//! inverse transform from its [`ClassSamplerTable`], one word each.
//!
//! A catalogue entry of a binary class draws what defines it now and then a history consistent
//! with it (design note 10). Each mark's law in the table is a residual given by its quantiles at
//! the 17 ranks k ÷ 16, linear between them, added to a median that follows a condition the caller
//! supplies (an earlier mark, or a quantity derived from the marks), and held to the mark's
//! limits; a conditional mark may hold a law at each node of its median instead, mixed between
//! the nearest two ([`ClassSampler::residual_at`], P15.T10.b's refit at 27). A draw takes one
//! uniform word for its rank, so a mark's value is a monotone function of its word and a
//! restricted draw ([`ClassSampler::draw_in_ranks`]) is an exact conditional one, as plan 11's
//! design note 1 draws the stripped orbits. The four binary classes share it
//! (P11.T8.a–d), each with its own table: the accreting white dwarfs' are
//! [`AWD_FAST_SAMPLER`](crate::tables::binary::AWD_FAST_SAMPLER) and
//! [`AWD_SLOW_SAMPLER`](crate::tables::binary::AWD_SLOW_SAMPLER), a side each, and the X-ray
//! binaries' [`XRB_SAMPLER`](crate::tables::binary::XRB_SAMPLER).

use crate::rng::Stream;
use crate::tables::binary::{ClassSamplerTable, MedianCurve, SamplerKey};

/// A binary class's sampler over its table (P11.T8): marks by inverse transform, kinds by weight.
///
/// # Examples
///
/// A slow accreting white dwarf's recurrence period, beyond L at every rank of its side's
/// sampler, and its white dwarf's mass given it, inside the mark's limits.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::math;
/// use hyperion_sim::rng::{ObjectKey, Stream, tags};
/// use hyperion_sim::stellar::binary::sampler::ClassSampler;
/// use hyperion_sim::tables::binary::{AWD_SLOW_SAMPLER, AwdMark};
/// use hyperion_sim::time::LIGHT_CROSSING_L;
///
/// let sampler = ClassSampler::new(&AWD_SLOW_SAMPLER);
/// let log_l = math::log10(LIGHT_CROSSING_L.as_julian_years_f64());
/// assert!(sampler.value_at(AwdMark::Recurrence, 0.0, 0.0) > log_l);
/// let mut stream = Stream::open(Seed::new(3), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let log_years = sampler.draw(AwdMark::Recurrence, 0.0, &mut stream);
/// assert!(log_years > log_l);
/// let mass = sampler.draw(AwdMark::WhiteDwarfMass, log_years, &mut stream);
/// let (lightest, heaviest) = AWD_SLOW_SAMPLER.mark(AwdMark::WhiteDwarfMass).limits();
/// assert!((lightest..=heaviest).contains(&mass));
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
        interpolate(curve.nodes(), node_position(curve, condition))
    }

    /// The residual's quantiles at the ranks k ÷ 16 of `mark` at `condition`: the mark's one law,
    /// or for a mark with a law at each node (P15.T10.b's refit at 27) the two nearest nodes'
    /// laws mixed linearly, rank by rank, as the median mixes their medians. Each law is
    /// non-decreasing, so the mixture is.
    ///
    /// # Panics
    ///
    /// If the table holds fewer marks than `M` names.
    #[must_use]
    pub fn residual_at(&self, mark: M, condition: f64) -> [f64; 17] {
        let law = self.table.mark(mark);
        let Some(rows) = law.node_residuals() else {
            return *law.residual();
        };
        let curve = law
            .median()
            .expect("a mark with node laws follows a median (`SamplerMark::with_node_residuals`)");
        let (k, t) = node_step(node_position(curve, condition));
        std::array::from_fn(|j| rows[k][j] + t * (rows[k + 1][j] - rows[k][j]))
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
        let residual = interpolate(
            &self.residual_at(mark, condition),
            rank.clamp(0.0, 1.0) * 16.0,
        );
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
        let q = &self.residual_at(mark, condition);
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

/// Where `condition` lies along `curve`'s 17 nodes, in [0, 16]: held at the span's ends.
#[must_use]
fn node_position(curve: &MedianCurve, condition: f64) -> f64 {
    let (first, last) = curve.span();
    ((condition - first) / (last - first)).clamp(0.0, 1.0) * 16.0
}

/// The segment of 17 nodes at the whole numbers that `s` in [0, 16] lies in, 0–15, and how far
/// along it, in [0, 1].
#[must_use]
fn node_step(s: f64) -> (usize, f64) {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "s is in [0, 16], so its floor is an index of 0–16"
    )]
    let k = (s.floor() as usize).min(15);
    (
        k,
        s - f64::from(u32::try_from(k).expect("an index of 0–15")),
    )
}

/// The value at `s` in [0, 16] of 17 nodes at the whole numbers, linear between them.
#[must_use]
fn interpolate(nodes: &[f64; 17], s: f64) -> f64 {
    let (k, t) = node_step(s);
    nodes[k] + t * (nodes[k + 1] - nodes[k])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::rng::{ObjectKey, tags};
    use crate::tables::binary::{
        AWD_FAST_SAMPLER, AWD_SLOW_SAMPLER, AwdDonorKind, AwdMark, AwdPick, MedianCurve,
        SamplerMark, SamplerPick, XRB_SAMPLER, XrbHostKind, XrbMark, XrbPick,
    };

    /// The accreting white dwarfs' samplers, the fast side's and the slow side's.
    const AWD_SAMPLERS: [&ClassSamplerTable<AwdMark, AwdPick>; 2] =
        [&AWD_FAST_SAMPLER, &AWD_SLOW_SAMPLER];

    fn stream(seed: u64) -> Stream {
        Stream::open(Seed::new(seed), tags::SELFTEST_STREAM, ObjectKey::galaxy())
    }

    /// Each side's marks lie in the order `AwdMark` names them, and every residual is
    /// non-decreasing.
    #[test]
    fn the_awd_tables_follow_their_mark_list() {
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
        for table in AWD_SAMPLERS {
            assert_eq!(table.marks().len(), AwdMark::ALL.len());
            for (mark, name) in AwdMark::ALL.into_iter().zip(names) {
                let law = table.mark(mark);
                assert_eq!(law.name(), name);
                assert!(law.residual().windows(2).all(|w| w[0] <= w[1]), "{name}");
                let (lo, hi) = law.limits();
                assert!(lo < hi, "{name}");
            }
            let kinds = table.pick(AwdPick::DonorKind).weights();
            assert_eq!(kinds.len(), AwdDonorKind::ALL.len());
            assert!((kinds.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        }
    }

    /// The X-ray binaries' marks lie in the order `XrbMark` names them, every residual is
    /// non-decreasing and inside its limits' reach, each choice has one weight per kind of its
    /// side, and the weights over both sides sum to 1.
    #[test]
    fn the_xrb_table_follows_its_mark_list() {
        let names = [
            "neutron_star_mass",
            "black_hole_mass",
            "log_low_mass_period_hours",
            "log_low_mass_donor_mass",
            "low_mass_rate_excess_dex",
            "log_neutron_star_recurrence_years",
            "log_black_hole_recurrence_years",
            "log_bex_period_days",
            "bex_eccentricity",
            "bex_donor_mass",
            "log_supergiant_period_days",
            "supergiant_donor_mass",
            "log_supergiant_rate",
            "bex_recoil_km_s",
            "supergiant_recoil_km_s",
            "log_bex_years_since_supernova",
            "log_supergiant_years_since_supernova",
            "log_neutron_star_progenitor_mass",
            "log_black_hole_progenitor_mass",
            "log_donor_initial_excess",
            "log_ultracompact_period_minutes",
            "log_ultracompact_recurrence_years",
            "log_ultracompact_donor_initial_mass",
            "log_symbiotic_period_days",
            "log_symbiotic_donor_mass",
            "symbiotic_filling",
            "black_hole_high_mass_accretor_mass",
            "log_black_hole_high_mass_period_days",
            "black_hole_high_mass_donor_mass",
            "log_black_hole_high_mass_rate",
        ];
        assert_eq!(XRB_SAMPLER.marks().len(), XrbMark::ALL.len());
        for (mark, name) in XrbMark::ALL.into_iter().zip(names) {
            let law = XRB_SAMPLER.mark(mark);
            assert_eq!(law.name(), name);
            assert!(law.residual().windows(2).all(|w| w[0] <= w[1]), "{name}");
            let (lo, hi) = law.limits();
            assert!(lo < hi, "{name}");
        }
        let low = XRB_SAMPLER.pick(XrbPick::LowMassKind).weights();
        let high = XRB_SAMPLER.pick(XrbPick::HighMassKind).weights();
        assert_eq!(low.len(), XrbHostKind::LOW_MASS.len());
        assert_eq!(high.len(), XrbHostKind::HIGH_MASS.len());
        let (low_sum, high_sum) = (low.iter().sum::<f64>(), high.iter().sum::<f64>());
        assert!((low_sum + high_sum - 1.0).abs() < 1e-12);
        assert!(XrbHostKind::LOW_MASS.iter().all(|k| !k.is_high_mass()));
        assert!(XrbHostKind::HIGH_MASS.iter().all(|k| k.is_high_mass()));
    }

    /// `rank_of` inverts `value_at` inside the limits, at every rank and condition tried, and
    /// the medians are their nodes at the nodes.
    #[test]
    fn the_rank_inverts_the_value() {
        for (table, mark) in AWD_SAMPLERS
            .into_iter()
            .flat_map(|table| AwdMark::ALL.map(|mark| (table, mark)))
        {
            let sampler = ClassSampler::new(table);
            let law = table.mark(mark);
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

    /// Node k's law: a uniform spread of k ÷ 16 about the median, so that the spread widens with
    /// the condition.
    const fn widening() -> [[f64; 17]; 17] {
        let mut rows = [[0.0; 17]; 17];
        let mut k = 0;
        while k < 17 {
            let mut j = 0;
            while j < 17 {
                #[expect(clippy::cast_precision_loss, reason = "indices of 0–16")]
                let (node, rank) = (k as f64, j as f64);
                rows[k][j] = (node / 16.0) * (rank / 8.0 - 1.0);
                j += 1;
            }
            k += 1;
        }
        rows
    }

    /// The node laws of [`NODES`].
    static WIDENING: [[f64; 17]; 17] = widening();

    /// A mark about a median of twice its condition on 0–16, with a law at each node, its own law
    /// a unit spread that a draw does not read.
    static NODES: ClassSamplerTable<Only, Only> = ClassSamplerTable::new(
        &[SamplerMark::new(
            "widening",
            [
                -1.0, -0.875, -0.75, -0.625, -0.5, -0.375, -0.25, -0.125, 0.0, 0.125, 0.25, 0.375,
                0.5, 0.625, 0.75, 0.875, 1.0,
            ],
            Some(MedianCurve::new(
                0.0,
                16.0,
                [
                    0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0,
                    28.0, 30.0, 32.0,
                ],
            )),
            (-100.0, 100.0),
        )
        .with_node_residuals(&WIDENING)],
        &[SamplerPick::new("only", &[1.0])],
    );

    /// A mark with a law at each node takes the node's law at a node and the two nearest nodes'
    /// laws mixed between them, held beyond the span; the rank inverts the value at every
    /// condition; a mark of one law is drawn as before, its node laws `None`.
    #[test]
    fn a_mark_with_node_laws_mixes_its_nearest_two() {
        let sampler = ClassSampler::new(&NODES);
        // At node 4 the spread is ±0.25 about 8.
        assert!((sampler.value_at(Only, 4.0, 0.0) - 7.75).abs() < 1e-12);
        assert!((sampler.value_at(Only, 4.0, 1.0) - 8.25).abs() < 1e-12);
        // Halfway from node 4 to node 5, ±0.281 25 about 9.
        assert!((sampler.value_at(Only, 4.5, 1.0) - 9.281_25).abs() < 1e-12);
        // Beyond the span the last node's ±1 about 32.
        assert!((sampler.value_at(Only, 40.0, 0.0) - 31.0).abs() < 1e-12);
        // At node 0 every quantile ties at the median, which takes the lowest rank.
        for (condition, tied) in [(0.0, true), (3.3, false), (8.0, false), (15.9, false)] {
            for i in 0..=32 {
                let rank = f64::from(i) / 32.0;
                let value = sampler.value_at(Only, condition, rank);
                let back = sampler.rank_of(Only, condition, value);
                let expected = if tied { 0.0 } else { rank };
                assert!(
                    (back - expected).abs() < 1e-9,
                    "{condition}, {rank}: {back}"
                );
            }
        }
        assert!(
            AWD_FAST_SAMPLER
                .mark(AwdMark::Recurrence)
                .node_residuals()
                .is_none()
        );
    }

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

    /// Each side's recurrence lies on its side of L at every rank: a fast host's draw within L, a
    /// slow host's beyond it.
    #[test]
    fn each_side_s_recurrence_lies_on_its_side_of_l() {
        let mark = AwdMark::Recurrence;
        let log_l = crate::math::log10(crate::time::LIGHT_CROSSING_L.as_julian_years_f64());
        let [fast, slow] = AWD_SAMPLERS.map(ClassSampler::new);
        let mut s = stream(1);
        for _ in 0..10_000 {
            assert!(fast.draw(mark, 0.0, &mut s) <= log_l);
            assert!(slow.draw(mark, 0.0, &mut s) > log_l);
        }
    }

    /// Each draw takes one word, and a pick takes its kinds at their weights, a kind of none never.
    #[test]
    fn draws_take_one_word_and_picks_their_weights() {
        let sampler = ClassSampler::new(&AWD_FAST_SAMPLER);
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
        let weights = AWD_FAST_SAMPLER.pick(AwdPick::DonorKind).weights();
        for (count, weight) in counts.iter().zip(weights) {
            let share = f64::from(*count) / f64::from(n);
            let sigma = (weight * (1.0 - weight) / f64::from(n)).sqrt();
            assert!(
                (share - weight).abs() <= 5.0 * sigma,
                "{share} against {weight}"
            );
        }
    }
}
