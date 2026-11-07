//! The tests of P11.T4's carbon–oxygen dwarfs at the Chandrasekhar mass, on the record since the
//! 30 folds: every carbon–oxygen white dwarf or core at the Chandrasekhar mass disrupts, leaves
//! nothing and pools one Type Ia candidate (`Engine::end_at_the_chandrasekhar_mass`). The engine's
//! own cases, path by path, are `stellar::binary::tests`' (P11.T4's carbon–oxygen dwarfs at the
//! Chandrasekhar mass there).
//!
//! - The server golden's carbon–oxygen dwarf of 1.12 M☉ (alone) that its pair ends as a neutron star
//!   (plan 15's P15.T9.b, from P11.T11.a's records) takes none of the paths: its dwarf merges with
//!   a helium star, and the product, keeping the dwarf's core, collapses by electron capture at the
//!   merger (P11.T4's `merged_capture`).
//! - Slow: the dwarf-limit census's 6 × 10⁴ pairs of layers C, D and E near the Sun under every 30
//!   hold: each path's endings and first candidates, the pairs' candidates by channel, and the
//!   pairs that show a neutron star at their age.

use crate::planetary::testing::in_shares;

use super::*;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::co_chandrasekhar::{ChandrasekharPath, chandrasekhar_endings};
use crate::stellar::binary::ia::is_detonatable;
use crate::stellar::binary::testing::generated_pairs;
use crate::stellar::binary::{IaPoolChannel, can_interact, evolve};
use crate::units::Years;

/// The census's Milky Way seed: the binary fits' (P15.T10.b's manifests), the dwarf-limit census's.
const CENSUS_SEED: u64 = 0x0b17_15a0_0000_0001;

/// The seed of the census's cells, to which each layer adds its index: the dwarf-limit census's, so
/// that both run the same pairs.
const CELLS_SEED: u64 = 0x5c7a_7c40_0000_0000;

/// The pairs of each layer in the census.
const PAIRS: usize = 20_000;

/// The age each pair of the census is run to, years: 13.8 Gyr.
const UNTIL: f64 = 1.38e10;

/// The shares the census runs in.
const SHARES: usize = 64;

/// The threads the census runs on.
const THREADS: usize = 16;

/// The server golden's grid record `0x6200_5957_fe00_0024` of the galaxy of seed 1234
/// (`systems_in_range_briefs`, GPF 000N1Z D-36), whose 5.71 M☉ primary alone leaves a carbon–oxygen
/// white dwarf of 1.12 M☉, and its pair a neutron star (P11.T11.a's records; plan 15's P15.T9.b):
/// the primary, stripped to a helium star, leaves a 0.94 M☉ carbon–oxygen dwarf, which merges with
/// the 1.64 M☉ helium star its companion's common envelope leaves, with no Type Ia candidate pooled.
/// The product's core is oxygen–neon, the helium star's own track typing its core so (HPT section
/// 6.1; `Engine::coalesced_source`), and no carbon–oxygen dwarf or core reaches the Chandrasekhar
/// mass: its core already holds the capture mass at the merger, where it collapses by electron
/// capture into Mandel and Müller's 1.26 M☉ neutron star (P11.T4's `merged_capture`), where under
/// every 29 hold the product, a helium giant of 2.34 M☉ placed around the dwarf, collapsed an iron
/// core at its track's death into a neutron star of over 1.4 M☉.
#[test]
fn the_golden_s_carbon_oxygen_dwarf_ends_a_neutron_star() {
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::placement::{CellKey, generate_cell};
    use crate::stellar::binary::SegmentKind;
    use crate::stellar::remnant::RemnantKind;
    use crate::stellar::remnant::collapse::electron_capture_remnant;
    use crate::stellar::system::SystemStars;

    let seed = Seed::new(0x4d2);
    let id = crate::id::SystemId::from_raw(0x6200_5957_fe00_0024).expect("a grid ID");
    let rules = GalaxyRules::every_30_hold();
    let case = "every 30 hold";
    let galaxy = Galaxy::from_params_with_rules(
        seed,
        GalaxyParams::from_seed(seed, MassFunctionKind::default()),
        rules,
    )
    .expect("a drawn galaxy keeps half its gas neutral");
    let mut cell = Vec::new();
    generate_cell(&galaxy, CellKey::of(id).expect("a grid cell"), &mut cell);
    let record = *cell
        .iter()
        .find(|r| r.id() == id)
        .expect("the cell holds the record");
    let stars = SystemStars::generate(&galaxy, &record);
    let [pair] = stars.pairs() else {
        panic!("{case}: one pair, not {}", stars.pairs().len());
    };
    let timeline = pair.timeline();
    let [collapse] = timeline.supernovae() else {
        panic!("{case}: one death, not {:?}", timeline.supernovae());
    };
    assert_eq!(
        collapse.remnant().kind(),
        RemnantKind::NeutronStar,
        "{case}"
    );
    assert!(timeline.pooled_ia().is_none(), "{case}");
    // The merger of the dwarf and the helium star comes at or before the collapse.
    let merged = timeline
        .segments()
        .iter()
        .find(|s| s.kind() == SegmentKind::Merged)
        .expect("the pair merges");
    let before = timeline.state_at(Years::new(merged.start().value() * (1.0 - 1e-6)));
    let phases = before.stars().map(|s| s.phase());
    assert!(
        phases.contains(&Phase::CarbonOxygenWhiteDwarf),
        "{case}: {phases:?}"
    );
    assert_eq!(
        collapse.age().value().total_cmp(&merged.start().value()),
        core::cmp::Ordering::Equal,
        "{case}: {collapse:?} at the merger"
    );
    assert_eq!(collapse.remnant(), electron_capture_remnant(), "{case}");
}

/// What the census finds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Figures {
    /// The pairs run, those that can interact by [`UNTIL`], in layers C, D and E.
    run: [usize; 3],
    /// The endings at the Chandrasekhar mass by path ([`ChandrasekharPath::ALL`]'s order;
    /// `co_chandrasekhar::chandrasekhar_endings`): each path's endings, every one a disruption, the
    /// candidates it pooled first for their pairs, with those that can detonate, and the candidates
    /// that can detonate but sit behind an earlier one of their pair's that cannot.
    paths: [[[usize; 4]; 7]; 3],
    /// The pairs' Type Ia candidates (P11.T6's pool) by channel, the merger first: those pooled,
    /// and those that can detonate (`ia::is_detonatable`).
    candidates: [[[usize; 2]; 2]; 3],
    /// Of the pairs run, those that show a neutron star at their age.
    neutron_stars: [usize; 3],
}

/// The census of [`PAIRS`] pairs of each of layers C, D and E in `galaxy`, each run under `rules`.
fn census(galaxy: &Galaxy, rules: StellarRules) -> Figures {
    let mut figures = Figures {
        run: [0; 3],
        paths: [[[0; 4]; 7]; 3],
        candidates: [[[0; 2]; 2]; 3],
        neutron_stars: [0; 3],
    };
    for (k, layer) in [Layer::C, Layer::D, Layer::E].into_iter().enumerate() {
        let seed = CELLS_SEED + u64::try_from(k).expect("a small index");
        let pairs = generated_pairs(galaxy, layer, PAIRS, seed);
        let shares = in_shares(SHARES, THREADS, |share| {
            // The share runs on one thread, whose counter it reads before and after.
            let before = chandrasekhar_endings();
            let mut counts = [0_usize; 6];
            for (input, age) in pairs.iter().skip(share).step_by(SHARES) {
                let input = input.clone().with_rules(rules);
                if !can_interact(&input, Years::new(UNTIL)) {
                    continue;
                }
                let timeline = evolve(&input, Years::new(UNTIL));
                counts[0] += 1;
                if let Some(event) = timeline.pooled_ia() {
                    let c = match event.channel() {
                        IaPoolChannel::Merger => 1,
                        IaPoolChannel::Accretion => 3,
                    };
                    counts[c] += 1;
                    counts[c + 1] += usize::from(is_detonatable(&event));
                }
                let state = timeline.state_at(Years::new(*age));
                counts[5] += usize::from(
                    state
                        .stars()
                        .iter()
                        .any(|s| s.phase() == Phase::NeutronStar),
                );
            }
            let after = chandrasekhar_endings();
            let paths: [[usize; 4]; 7] =
                core::array::from_fn(|p| core::array::from_fn(|j| after[p][j] - before[p][j]));
            (counts, paths)
        });
        for (counts, paths) in shares {
            figures.run[k] += counts[0];
            figures.candidates[k][0][0] += counts[1];
            figures.candidates[k][0][1] += counts[2];
            figures.candidates[k][1][0] += counts[3];
            figures.candidates[k][1][1] += counts[4];
            figures.neutron_stars[k] += counts[5];
            for (total, n) in figures.paths[k].iter_mut().zip(paths) {
                for (t, n) in total.iter_mut().zip(n) {
                    *t += n;
                }
            }
        }
    }
    figures
}

/// The census's figures under every 30 hold, as measured (2026-11-05 and 2026-11-06, at generator
/// version 28; pod, 16 vCPU). Layer C reaches the Chandrasekhar mass by no path. Every ending
/// disrupts: layer D's 148 white dwarfs fed by transfer and 10 merged dwarfs, all but 5 of the fed
/// ones having pooled their pair's candidate before, and its 36 kept cores, 35 of which pool their
/// pair's first candidate, every one detonatable, so that layer D pools 319 accretions (234
/// detonatable); layer E's 11 fed dwarfs and 2 kept cores. Of layer D's pairs 2,072 show a neutron
/// star at their age, and of layer E's 11,190, where under every 29 hold 2,127 and 11,193 did.
/// Behind an earlier candidate of their pair's that cannot detonate, 9 of layer D's fed dwarfs and
/// 2 of layer E's kept cores could have detonated, which the pool, holding a pair's first, does not
/// record.
const CENSUS_FIGURES: Figures = Figures {
    run: [8_519, 17_814, 19_213],
    paths: [
        [[0; 4]; 7],
        [
            [148, 5, 5, 9],
            [0; 4],
            [10, 0, 0, 0],
            [0; 4],
            [0; 4],
            [36, 35, 35, 0],
            [0; 4],
        ],
        [
            [11, 0, 0, 0],
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
            [2, 0, 0, 2],
            [0; 4],
        ],
    ],
    candidates: [
        [[59, 0], [15, 0]],
        [[124, 55], [319, 234]],
        [[5, 5], [25, 21]],
    ],
    neutron_stars: [1, 2_072, 11_190],
};

/// A census of 6 × 10⁴ generated pairs ([`census`]) under every 30 hold: every ending is a
/// disruption, and every candidate a path pools first can detonate.
#[test]
#[ignore = "slow: 6 × 10⁴ generated pairs run through the binary engine to 13.8 Gyr"]
fn every_carbon_oxygen_dwarf_at_the_chandrasekhar_mass_disrupts_under_every_30_hold() {
    let galaxy = Galaxy::from_params(Seed::new(CENSUS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let figures = census(&galaxy, StellarRules::every_30_hold());
    println!("{figures:?}");
    for (layer, paths) in figures.paths.iter().enumerate() {
        for (path, [_, first, detonatable, _]) in ChandrasekharPath::ALL.iter().zip(paths) {
            assert_eq!(detonatable, first, "layer {layer}, {path:?}");
        }
    }
    assert_eq!(figures, CENSUS_FIGURES);
}
