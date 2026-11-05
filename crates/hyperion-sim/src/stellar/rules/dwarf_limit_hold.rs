//! The tests of P11.T4's dwarf-limit fix: the binary engine shows no white dwarf at or above the
//! Chandrasekhar mass, and types the dwarf a merger's unplaced product leaves by its core
//! (`stellar::binary::dwarf_limit`, read by `Engine::new_star`'s fallback, `Engine::finish_merger`
//! and the accretor's events). Its holds are deleted (the 27 fold, and the 29 fold of the
//! white-dwarf union's fixes); these are its tests of the new arm, which the bump re-pins by name.
//! The engine's own cases are `stellar::binary::tests`' (P11.T4's dwarf limit there).
//!
//! - Slow: a census of 6 × 10⁴ pairs of layers C, D and E near the Sun, as the generator draws
//!   them, run to 13.8 Gyr under the version's rules: none shows a white dwarf above the
//!   Chandrasekhar mass (under version 25's rules some did) but a pinned primary's (design note
//!   16), counted apart, which `Engine::held_product` takes as at 27; the merger products no star
//!   holds, by what they leave; the swollen dwarfs and placed products of dwarfs that come back
//!   the other kind, none; and the deaths of giants whose cores are dwarfs', by ending.

use crate::planetary::testing::in_shares;

use super::*;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::id::Layer;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::dwarf_limit::{dwarf_core_deaths, unplaced_counts};
use crate::stellar::binary::testing::{
    dwarfs_above_the_limit, generated_pairs, oxygen_neon_dwarfs_above_the_capture_mass,
    pinned_primary, swollen_dwarf_kinds,
};
use crate::stellar::binary::{can_interact, evolve};
use crate::units::Years;

/// The census's Milky Way seed: the binary fits' (P15.T10.b's manifests).
const CENSUS_SEED: u64 = 0x0b17_15a0_0000_0001;

/// The seed of the census's cells, to which each layer adds its index.
const CELLS_SEED: u64 = 0x5c7a_7c40_0000_0000;

/// The pairs of each layer in the census.
const PAIRS: usize = 20_000;

/// The age each pair of the census is run to, years: 13.8 Gyr.
const UNTIL: f64 = 1.38e10;

/// The shares the census runs in.
const SHARES: usize = 64;

/// The threads the census runs on, the slots its test reserves (`.config/nextest.toml`).
const THREADS: usize = 16;

/// What the census finds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Figures {
    /// The pairs run, those that can interact by [`UNTIL`], in layers C, D and E.
    run: [usize; 3],
    /// Of those, the pairs that show a white dwarf above the Chandrasekhar mass other than a pinned
    /// primary's.
    above: [usize; 3],
    /// Of the others, those whose pinned primary (design note 16) shows one, left to its pin.
    pinned: [usize; 3],
    /// Of the pairs run, those left with no star at their age at the epoch (both members
    /// `NoRemnant`).
    massless: [usize; 3],
    /// The merger products no star could hold, by what they left
    /// (`dwarf_limit::unplaced_counts`): helium, carbon–oxygen and oxygen–neon dwarfs, naked
    /// helium stars, iron cores, pinned primaries' products left to their pin, disruptions and
    /// electron captures.
    unplaced: [[usize; 8]; 3],
    /// Of the pairs run, the carbon–oxygen and oxygen–neon dwarfs whose swollen giant came back the
    /// other kind (`testing::swollen_dwarf_kinds`): carbon–oxygen to oxygen–neon, and back.
    swollen: [[usize; 2]; 3],
    /// The same through a merger product placed on a track of its own, which the swollen dwarf's
    /// rule does not reach.
    merged: [[usize; 2]; 3],
    /// The deaths of giants whose cores are carbon–oxygen or oxygen–neon dwarfs', by ending
    /// (`dwarf_limit::dwarf_core_deaths`): their tracks' own, a white dwarf of the dwarf's kind,
    /// an electron capture and a disruption at their tracks' deaths, and an electron capture at
    /// their cores' limits.
    endings: [[usize; 5]; 3],
    /// Of the pairs run, those that show an oxygen–neon white dwarf above the capture mass,
    /// 1.37 M☉ (`testing::oxygen_neon_dwarfs_above_the_capture_mass`): other than a pinned
    /// primary's, and a pinned primary's.
    oxygen_neon_above: [[usize; 2]; 3],
}

/// The census of [`PAIRS`] pairs of each of layers C, D and E in `galaxy`, each run under `rules`.
fn census(galaxy: &Galaxy, rules: StellarRules) -> Figures {
    let mut figures = Figures {
        run: [0; 3],
        above: [0; 3],
        pinned: [0; 3],
        massless: [0; 3],
        unplaced: [[0; 8]; 3],
        swollen: [[0; 2]; 3],
        merged: [[0; 2]; 3],
        endings: [[0; 5]; 3],
        oxygen_neon_above: [[0; 2]; 3],
    };
    for (k, layer) in [Layer::C, Layer::D, Layer::E].into_iter().enumerate() {
        let seed = CELLS_SEED + u64::try_from(k).expect("a small index");
        let pairs = generated_pairs(galaxy, layer, PAIRS, seed);
        let (co, one) = (Phase::CarbonOxygenWhiteDwarf, Phase::OxygenNeonWhiteDwarf);
        let shares = in_shares(SHARES, THREADS, |share| {
            // The share runs on one thread, whose counters it reads before and after.
            let unplaced_before = unplaced_counts();
            let endings_before = dwarf_core_deaths();
            let mut counts = [0_usize; 10];
            for (input, age) in pairs.iter().skip(share).step_by(SHARES) {
                let input = input.clone().with_rules(rules);
                if !can_interact(&input, Years::new(UNTIL)) {
                    continue;
                }
                let timeline = evolve(&input, Years::new(UNTIL));
                let at_age = timeline.state_at(Years::new(*age));
                let [a, b] = at_age.stars();
                let [primary, secondary] = dwarfs_above_the_limit(&timeline, UNTIL);
                let pinned = pinned_primary(&input);
                let above = secondary || (primary && !pinned);
                counts[0] += 1;
                counts[1] += usize::from(above);
                counts[2] += usize::from(!above && primary && pinned);
                counts[3] +=
                    usize::from(a.phase() == Phase::NoRemnant && b.phase() == Phase::NoRemnant);
                let [one_primary, one_secondary] =
                    oxygen_neon_dwarfs_above_the_capture_mass(&timeline, UNTIL);
                let one_above = one_secondary || (one_primary && !pinned);
                counts[8] += usize::from(one_above);
                counts[9] += usize::from(!one_above && one_primary && pinned);
                for (before, after, merged) in swollen_dwarf_kinds(&timeline) {
                    let j = if merged { 6 } else { 4 };
                    counts[j] += usize::from(before == co && after == one);
                    counts[j + 1] += usize::from(before == one && after == co);
                }
            }
            let unplaced_after = unplaced_counts();
            let unplaced: [usize; 8] =
                core::array::from_fn(|j| unplaced_after[j] - unplaced_before[j]);
            let endings_after = dwarf_core_deaths();
            let endings: [usize; 5] =
                core::array::from_fn(|j| endings_after[j] - endings_before[j]);
            (counts, unplaced, endings)
        });
        for (counts, unplaced, endings) in shares {
            figures.run[k] += counts[0];
            figures.above[k] += counts[1];
            figures.pinned[k] += counts[2];
            figures.massless[k] += counts[3];
            figures.swollen[k][0] += counts[4];
            figures.swollen[k][1] += counts[5];
            figures.merged[k][0] += counts[6];
            figures.merged[k][1] += counts[7];
            figures.oxygen_neon_above[k][0] += counts[8];
            figures.oxygen_neon_above[k][1] += counts[9];
            for (total, n) in figures.unplaced[k].iter_mut().zip(unplaced) {
                *total += n;
            }
            for (total, n) in figures.endings[k].iter_mut().zip(endings) {
                *total += n;
            }
        }
    }
    figures
}

/// The census's figures, as measured (2026-11-05, under every 29 hold at generator versions 27 and
/// 28 alike, which the 29 folds put on the record; under the held arm of version 25, 94 of layer
/// D's pairs and 157 of E's showed a dwarf above the limit; the unplaced products: no iron core,
/// every thermally pulsing product of layer E that no star holds being a pinned primary's; no
/// placed product changes kind; a kept core ends as its track does, its carbon–oxygen core
/// disrupting where its track captures electrons, most on the tracks of helium stars of
/// 1.6–2.25 M☉ (research-leans-40), and the disruptions leave both members `NoRemnant`; no
/// oxygen–neon dwarf passes the capture mass, and no unplaced product captures electrons).
const CENSUS_FIGURES: Figures = Figures {
    run: [8_519, 17_814, 19_213],
    above: [0; 3],
    pinned: [0; 3],
    massless: [174, 56, 12],
    unplaced: [[0; 8], [0, 1, 0, 6, 0, 0, 0, 0], [0, 0, 0, 0, 0, 109, 0, 0]],
    swollen: [[0; 2]; 3],
    merged: [[0; 2]; 3],
    endings: [[25, 0, 0, 0, 0], [848, 45, 0, 36, 11], [69, 4, 0, 2, 0]],
    oxygen_neon_above: [[0; 2]; 3],
};

/// A census of 6 × 10⁴ generated pairs ([`census`]) under the version's rules: none shows a white
/// dwarf above the Chandrasekhar mass.
#[test]
#[ignore = "slow: 6 × 10⁴ generated pairs run through the binary engine to 13.8 Gyr"]
fn no_white_dwarf_passes_the_chandrasekhar_mass_under_every_27_hold() {
    let galaxy = Galaxy::from_params(Seed::new(CENSUS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let rules = StellarRules::of_version(crate::GENERATOR_VERSION);
    let figures = census(&galaxy, rules);
    println!("{rules:?}: {figures:?}");
    assert_eq!(figures.above, [0; 3], "{figures:?}");
    assert_eq!(figures.swollen, [[0; 2]; 3], "{figures:?}");
    // Carbon burning is not undone: no oxygen–neon core comes back carbon–oxygen.
    assert!(
        figures.merged.iter().all(|[_, back]| *back == 0),
        "{figures:?}"
    );
    assert!(
        figures
            .oxygen_neon_above
            .iter()
            .all(|[other, _]| *other == 0),
        "{figures:?}"
    );
    assert_eq!(figures, CENSUS_FIGURES);
}
