//! The tests of P11.T4's dwarf-limit fix: the binary engine shows no white dwarf at or above the
//! Chandrasekhar mass, and types the dwarf a merger's unplaced product leaves by its core
//! (`stellar::binary::dwarf_limit`, read by `Engine::new_star`'s fallback, `Engine::finish_merger`
//! and the accretor's events). Its first hold is deleted (the 27 fold); the white-dwarf union's
//! fixes are held at [`DWARF_LIMIT_VERSION`]. The engine's own cases are `stellar::binary::tests`'
//! (P11.T4's dwarf limit there).
//!
//! - The version's rules keep the union's held arm, and the tripwire holds it only until the bump.
//! - Slow: a census of 6 × 10⁴ pairs of layers C, D and E near the Sun, as the generator draws
//!   them, run to 13.8 Gyr under the version's rules and under every 29 hold: none shows a white
//!   dwarf above the Chandrasekhar mass (under the held arm some did) but a pinned primary's
//!   (design note 16), counted apart, which `Engine::held_product` takes as at 27. Every 29 hold
//!   adds P11.T4's holds at 29; the merger products no star holds, by what they leave; and the
//!   swollen dwarfs and placed products of dwarfs that come back the other kind, none under every
//!   29 hold; and the deaths of giants whose cores are dwarfs', by ending.

use crate::planetary::testing::in_shares;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::id::Layer;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::dwarf_limit::{
    DWARF_LIMIT_VERSION, DwarfLimitRules, dwarf_core_deaths, unplaced_counts,
};
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

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The version's rules keep the held arm, every version below 29 does and 29 takes the white-dwarf
/// union's fixes; the fitted tables hold only under the version's arm. The tripwire: the bump to
/// 29 deletes this hold.
#[test]
fn the_dwarf_limit_is_held_only_until_the_29_bump() {
    assert!(
        GENERATOR_VERSION < DWARF_LIMIT_VERSION,
        "the bump to 29 deletes the white-dwarf union's dwarf-limit hold, not its rule: \
         `DWARF_LIMIT_VERSION`, `DwarfLimitRules`' `Held` arm and its branches, \
         `StellarRules::dwarf_limit` and its reads in `Engine::new_star_around`, the dwarfs' \
         merger, `Engine::finish_merger`, the edge-lit arm of `Engine::accretor_events` and \
         `Engine::to_the_dwarf_limit`, the brief's read of \
         `DwarfLimitRules::fitted_tables_hold`, its tripwire and its line in \
         `stellar/rules.rs`; it keeps `DwarfLimitRules::unplaced`'s new arm, \
         `DwarfLimitRules::placed`'s record, `detached::Stop::CoreLimit` and the pin's \
         bypasses applied always, and refits the pair-light and reach tables"
    );
    assert_eq!(version().dwarf_limit, DwarfLimitRules::Held);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).dwarf_limit;
    for v in [24, 25, 26, 27, 28] {
        assert_eq!(at(v), DwarfLimitRules::Held, "version {v}");
    }
    assert_eq!(at(29), DwarfLimitRules::Whole);
    assert_eq!(
        StellarRules::every_29_hold().dwarf_limit,
        DwarfLimitRules::Whole
    );
    assert_eq!(
        StellarRules::of_version(crate::GENERATOR_VERSION).dwarf_limit,
        DwarfLimitRules::Held
    );
    assert!(version().dwarf_limit.fitted_tables_hold());
    assert!(!DwarfLimitRules::Whole.fitted_tables_hold());
}

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
    /// an electron capture and a disruption at their tracks' deaths, and an electron capture and
    /// a disruption at their cores' limits.
    endings: [[usize; 6]; 3],
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
        endings: [[0; 6]; 3],
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
            let endings: [usize; 6] =
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

/// The census's figures under the version's rules and under every 29 hold, as measured (2026-11-05,
/// at generator versions 27 and 28 alike; the 27 pin before, run [5983, 15222, 18105] and massless
/// [149, 18, 2], was stale at version 27's base; under the held arm of version 25, 94 of layer D's
/// pairs and 157 of E's showed a dwarf above the limit; the unplaced products: no iron core, every
/// thermally pulsing product of layer E that no star holds being a pinned primary's; under every 29
/// hold no placed product changes kind, and the carbon–oxygen cores whose tracks would collapse
/// disrupt and leave nothing, most at the Chandrasekhar mass on their early AGB, which leaves both
/// members `NoRemnant`; from P11.T4's kept-core endings a kept core ends as its track does, its
/// carbon–oxygen core disrupting where its track captures electrons, most on the tracks of helium
/// stars of 1.6–2.25 M☉ (research-leans-40), no oxygen–neon dwarf passes the capture mass, and no
/// unplaced product captures electrons).
const CENSUS_FIGURES: [Figures; 2] = [
    Figures {
        run: [8_519, 17_814, 19_213],
        above: [0; 3],
        pinned: [0; 3],
        massless: [148, 20, 6],
        unplaced: [[0; 8], [0, 4, 0, 3, 0, 0, 0, 0], [0, 0, 0, 0, 0, 109, 0, 0]],
        swollen: [[0, 0], [1, 0], [0, 0]],
        merged: [[6, 0], [371, 26], [25, 0]],
        endings: [[0; 6]; 3],
        oxygen_neon_above: [[0, 0], [225, 0], [89, 0]],
    },
    Figures {
        run: [8_519, 17_814, 19_213],
        above: [0; 3],
        pinned: [0; 3],
        massless: [174, 56, 12],
        unplaced: [[0; 8], [0, 1, 0, 6, 0, 0, 0, 0], [0, 0, 0, 0, 0, 109, 0, 0]],
        swollen: [[0; 2]; 3],
        merged: [[0; 2]; 3],
        endings: [
            [25, 0, 0, 0, 0, 0],
            [848, 45, 0, 36, 11, 0],
            [69, 4, 0, 2, 0, 0],
        ],
        oxygen_neon_above: [[0; 2]; 3],
    },
];

/// A census of 6 × 10⁴ generated pairs ([`census`]) under the version's rules and under every 29
/// hold: none shows a white dwarf above the Chandrasekhar mass.
#[test]
#[ignore = "slow: 6 × 10⁴ generated pairs run through the binary engine to 13.8 Gyr, twice"]
fn no_white_dwarf_passes_the_chandrasekhar_mass_under_every_27_hold() {
    let galaxy = Galaxy::from_params(Seed::new(CENSUS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let figures = [
        StellarRules::of_version(crate::GENERATOR_VERSION),
        StellarRules::every_29_hold(),
    ]
    .map(|rules| {
        let figures = census(&galaxy, rules);
        println!("{rules:?}: {figures:?}");
        assert_eq!(figures.above, [0; 3], "{figures:?}");
        figures
    });
    assert_eq!(figures[1].swollen, [[0; 2]; 3], "{:?}", figures[1]);
    // Carbon burning is not undone: no oxygen–neon core comes back carbon–oxygen.
    assert!(
        figures[1].merged.iter().all(|[_, back]| *back == 0),
        "{:?}",
        figures[1]
    );
    assert!(
        figures[1]
            .oxygen_neon_above
            .iter()
            .all(|[other, _]| *other == 0),
        "{:?}",
        figures[1]
    );
    assert_eq!(figures, CENSUS_FIGURES);
}
