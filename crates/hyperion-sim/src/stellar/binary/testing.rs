//! Test helpers shared by the census bounds' tests (P11.T17.a's `light.rs` and P11.T17.b's
//! `pair_light`): a splitmix64 stream and the generator's own star–star pairs.

use crate::Seed;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, generate_cell};
use crate::id::Layer;
use crate::stellar::binary::BinaryInput;
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::{
    HierarchyNode, RedrawAttempt, SlotKind, StarIndex, draw_hierarchy_of_composition,
};
use crate::stellar::system::{draw_metallicity, grid_multiplicity, primary_draws};

/// A splitmix64 stream.
pub(crate) struct Mix(pub(crate) u64);

impl Mix {
    pub(crate) fn word(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub(crate) fn unit(&mut self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a 53-bit word and 2⁵³ are exact in an f64"
        )]
        let u = (self.word() >> 11) as f64 / (1_u64 << 53) as f64;
        u
    }

    /// A whole number in `-k..=k`.
    pub(crate) fn within(&mut self, k: i32) -> i32 {
        let span = u64::try_from(2 * k + 1).expect("a positive span");
        i32::try_from(self.word() % span).expect("a small number") - k
    }
}

/// The Milky Way fixture under `seed`.
#[must_use]
pub(crate) fn milky_way(seed: u64) -> Galaxy {
    Galaxy::from_params(Seed::new(seed), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// `n` star–star pairs of `layer` near the solar circle, as the generator draws them (plan 11's
/// `run_pairs`), from `mix_seed`'s stream: up to three records from each of random cells within
/// 4,000 ly along x and 2,000 ly along y of the point 26,000 ly out along +y, each record's
/// hierarchy at attempt (its count mod 8), its composition, its primary's draws and its
/// companions' at that attempt, with the record's age at the epoch, years.
pub(crate) fn generated_pairs(
    galaxy: &Galaxy,
    layer: Layer,
    n: usize,
    mix_seed: u64,
) -> Vec<(BinaryInput, f64)> {
    let mut mix = Mix(mix_seed);
    let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
    let (along, across) = ((4_000 / size).max(1), (2_000 / size).max(1));
    let mut cell = Vec::new();
    let mut pairs = Vec::with_capacity(n + 8);
    let mut records = 0_u32;
    while pairs.len() < n {
        let key = CellKey::new(
            layer,
            [mix.within(along), 26_000 / size + mix.within(across), 0],
        )
        .expect("a cell of the grid");
        generate_cell(galaxy, key, &mut cell);
        for record in cell.iter().take(3) {
            let attempt = RedrawAttempt::new(u8::try_from(records % 8).expect("below 8"))
                .expect("an attempt below eight");
            records += 1;
            let comp = draw_metallicity(galaxy, record);
            let h = draw_hierarchy_of_composition(
                galaxy,
                record,
                &comp,
                grid_multiplicity(record),
                attempt,
            );
            let draws = |s: StarIndex| {
                if s == StarIndex::PRIMARY {
                    primary_draws(galaxy, record)
                } else {
                    let attempt = u32::from(attempt.get());
                    StarDraws::for_attempt(galaxy.seed(), h.star(s).body(), attempt)
                }
            };
            for (node, orbit) in h.pairs() {
                let HierarchyNode::Pair { inner, outer, .. } = *h.node(node) else {
                    continue;
                };
                let (HierarchyNode::Star(a), HierarchyNode::Star(b)) =
                    (*h.node(inner), *h.node(outer))
                else {
                    continue;
                };
                if [a, b]
                    .iter()
                    .any(|&s| h.star(s).kind() == SlotKind::BrownDwarf)
                {
                    continue;
                }
                let input = BinaryInput::new(
                    h.star(a).initial_mass(),
                    h.star(b).initial_mass(),
                    comp,
                    *orbit,
                    [draws(a), draws(b)],
                    record.age_at_epoch(),
                )
                .expect("a pair's stars are of 0.08-150 M_sun with a finite age");
                pairs.push((input, record.age_at_epoch().value()));
            }
        }
    }
    pairs.truncate(n);
    pairs
}
