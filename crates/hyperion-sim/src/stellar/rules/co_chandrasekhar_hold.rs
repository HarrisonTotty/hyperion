//! The tests of P11.T4's carbon–oxygen dwarfs at the Chandrasekhar mass,
//! [`CO_CHANDRASEKHAR_VERSION`]: from it every carbon–oxygen white dwarf or core at the
//! Chandrasekhar mass disrupts, leaves nothing and pools one Type Ia candidate, only under the
//! rules that turn it on, read by the binary engine through its input's rules
//! (`Engine::end_at_the_chandrasekhar_mass`). The engine's own cases, path by path, are
//! `stellar::binary::tests`' (P11.T4's carbon–oxygen dwarfs at the Chandrasekhar mass there).
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - The server golden's carbon–oxygen dwarf of 1.12 M☉ (alone) that its pair ends as a neutron star
//!   (plan 15's P15.T9.b, from P11.T11.a's records) takes none of the paths: its dwarf merges with
//!   a helium star, and the product's track, keeping the dwarf's core, collapses an iron core
//!   (research-leans-38's kept core), under every 29 hold and every 30 hold alike.
//! - Slow: the dwarf-limit census's 6 × 10⁴ pairs of layers C, D and E near the Sun under every 29
//!   hold and every 30 hold: each path's endings and first candidates, the pairs' candidates by
//!   channel, and the pairs that show a neutron star at their age.
//!
//! The movers, at the bump: none of the version's (nothing moves at 24–29, the held arm being the
//! version's code); the pair-light and reach tables, which the engine's runs fit, are refitted under
//! the version's rules, and `type_ia_yield`'s pool is this arm's (plan 15's P15.T9.b).

use crate::planetary::testing::in_shares;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::co_chandrasekhar::{
    CO_CHANDRASEKHAR_VERSION, ChandrasekharPath, CoChandrasekharRules, chandrasekhar_endings,
};
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

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The version's rules keep the held arm, every version from 24 to 29 does and 30 disrupts every
/// carbon–oxygen dwarf or core at the Chandrasekhar mass; the fitted tables hold only under the
/// version's arm. The tripwire: the bump to 30 deletes this hold.
#[test]
fn the_co_chandrasekhar_ending_is_held_only_until_the_30_bump() {
    assert!(
        GENERATOR_VERSION < CO_CHANDRASEKHAR_VERSION,
        "the bump to 30 deletes P11.T4's hold on the carbon–oxygen dwarfs at the Chandrasekhar \
         mass, not its rule: `CO_CHANDRASEKHAR_VERSION`, `CoChandrasekharRules` with its `Held` \
         arm and `ChandrasekharPath::held_ending`, `StellarRules::co_chandrasekhar` and its reads \
         in `Engine::end_at_the_chandrasekhar_mass`, `Engine::chandrasekhar_candidate` and \
         `Engine::finish_merger`, the brief's read of \
         `CoChandrasekharRules::fitted_tables_hold`, this module and its line in \
         `stellar/rules.rs`; it keeps the disruption with its candidate applied always, and \
         refits the pair-light and reach tables"
    );
    assert_eq!(version().co_chandrasekhar, CoChandrasekharRules::Held);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).co_chandrasekhar;
    for v in [24, 25, 26, 27, 28, 29] {
        assert_eq!(at(v), CoChandrasekharRules::Held, "version {v}");
    }
    assert_eq!(at(30), CoChandrasekharRules::Disrupted);
    assert_eq!(
        StellarRules::every_30_hold().co_chandrasekhar,
        CoChandrasekharRules::Disrupted
    );
    assert_eq!(
        GalaxyRules::every_30_hold().stellar,
        StellarRules::every_30_hold()
    );
    // Every 30 hold is the version's rules but for this one.
    assert_eq!(
        StellarRules {
            co_chandrasekhar: CoChandrasekharRules::Held,
            ..StellarRules::every_30_hold()
        },
        version()
    );
    assert!(version().co_chandrasekhar.fitted_tables_hold());
    assert!(!CoChandrasekharRules::Disrupted.fitted_tables_hold());
}

/// Each path's ending and candidate: under the held arm a white dwarf's collapse with a candidate
/// and a core's disruption with none, and under every 30 hold a disruption with one for every path.
#[test]
fn every_path_disrupts_and_pools_from_30() {
    use crate::stellar::binary::co_chandrasekhar::ChandrasekharEnding;

    let (collapse, disruption) = (
        ChandrasekharEnding::Collapse,
        ChandrasekharEnding::Disruption,
    );
    for path in ChandrasekharPath::ALL {
        let white_dwarf = matches!(
            path,
            ChandrasekharPath::Transfer
                | ChandrasekharPath::Wind
                | ChandrasekharPath::MergedDwarf
                | ChandrasekharPath::PinnedMerger
        );
        let held = CoChandrasekharRules::Held;
        assert_eq!(
            held.ending(path),
            if white_dwarf { collapse } else { disruption },
            "{path:?}"
        );
        assert_eq!(held.pools(path), white_dwarf, "{path:?}");
        let disrupted = CoChandrasekharRules::Disrupted;
        assert_eq!(disrupted.ending(path), disruption, "{path:?}");
        assert!(disrupted.pools(path), "{path:?}");
    }
}

/// The server golden's grid record `0x6200_5957_fe00_0024` of the galaxy of seed 1234
/// (`systems_in_range_briefs`, GPF 000N1Z D-36), whose 5.71 M☉ primary alone leaves a carbon–oxygen
/// white dwarf of 1.12 M☉, and its pair a neutron star (P11.T11.a's records; plan 15's P15.T9.b):
/// the primary, stripped to a helium star, leaves a 0.94 M☉ carbon–oxygen dwarf, which merges with
/// the 1.64 M☉ helium star its companion's common envelope leaves; the product, a helium giant of
/// 2.34 M☉ placed around the dwarf, collapses an iron core at its track's death into a 1.45 M☉
/// neutron star, with no Type Ia candidate pooled. Its track's `Mc,BAGB`, its mass, passes 2.25 M☉
/// (HPT section 6.1), and a kept core ends as its track does there (research-leans-38, P11.T4's
/// kept-core endings): no path to the Chandrasekhar mass, so under the version's rules and every 30
/// hold alike.
#[test]
fn the_golden_s_carbon_oxygen_dwarf_ends_a_neutron_star_by_its_kept_core() {
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::placement::{CellKey, generate_cell};
    use crate::stellar::binary::SegmentKind;
    use crate::stellar::remnant::RemnantKind;
    use crate::stellar::system::SystemStars;

    let seed = Seed::new(0x4d2);
    let id = crate::id::SystemId::from_raw(0x6200_5957_fe00_0024).expect("a grid ID");
    for rules in [
        GalaxyRules::of_version(GENERATOR_VERSION),
        GalaxyRules::every_30_hold(),
    ] {
        let case = format!("{:?}", rules.stellar.co_chandrasekhar);
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
        assert!(
            collapse.remnant().mass().value() > 1.4,
            "{case}: {collapse:?}"
        );
        assert!(timeline.pooled_ia().is_none(), "{case}");
        // The merger of the dwarf and the helium star comes before the collapse.
        let merged = timeline
            .segments()
            .iter()
            .find(|s| s.kind() == SegmentKind::Merged)
            .expect("the pair merges");
        assert!(merged.start() <= collapse.age(), "{case}");
        let before = timeline.state_at(Years::new(merged.start().value() * (1.0 - 1e-6)));
        let phases = before.stars().map(|s| s.phase());
        assert!(
            phases.contains(&Phase::CarbonOxygenWhiteDwarf),
            "{case}: {phases:?}"
        );
    }
}

/// What the census finds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Figures {
    /// The pairs run, those that can interact by [`UNTIL`], in layers C, D and E.
    run: [usize; 3],
    /// The endings at the Chandrasekhar mass by path ([`ChandrasekharPath::ALL`]'s order;
    /// `co_chandrasekhar::chandrasekhar_endings`): each path's endings, its accretion-induced
    /// collapses and disruptions, and the candidates it pooled first for their pairs, with those
    /// that can detonate.
    paths: [[[usize; 5]; 7]; 3],
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
        paths: [[[0; 5]; 7]; 3],
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
            let paths: [[usize; 5]; 7] =
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

/// The census's figures under every 29 hold and every 30 hold, as measured (2026-11-05, at
/// generator version 28; 272 s for both, pod, 16 vCPU). Layer C reaches the Chandrasekhar mass by
/// no path. Under every 29 hold layer D's 148 white dwarfs fed by transfer and 10 merged dwarfs
/// collapse into neutron stars, all but 5 of the fed ones having pooled their pair's candidate
/// before, and its 36 kept cores disrupt with none; layer E's 11 fed dwarfs collapse and its 2 kept
/// cores disrupt. Under every 30 hold every one of them
/// disrupts: 35 of layer D's kept cores pool their pair's first candidate, every one detonatable,
/// so that layer D pools 319 accretions, not 284 (234 detonatable, not 199). Of layer D's pairs
/// 1,987 rather than 2,127 show a neutron star at their age with this hold alone, and 2,072 with
/// every 30 hold, whose oxygen–neon dwarfs' coalescences collapse at once (P11.T4's
/// `merged_capture`, 2026-11-06); layer E's 11,193 become 11,187 and 11,190.
const CENSUS_FIGURES: [Figures; 2] = [
    Figures {
        run: [8_519, 17_814, 19_213],
        paths: [
            [[0; 5]; 7],
            [
                [148, 148, 0, 5, 5],
                [0; 5],
                [10, 10, 0, 0, 0],
                [0; 5],
                [0; 5],
                [36, 0, 36, 0, 0],
                [0; 5],
            ],
            [
                [11, 11, 0, 0, 0],
                [0; 5],
                [0; 5],
                [0; 5],
                [0; 5],
                [2, 0, 2, 0, 0],
                [0; 5],
            ],
        ],
        candidates: [
            [[59, 0], [15, 0]],
            [[124, 55], [284, 199]],
            [[5, 5], [25, 21]],
        ],
        neutron_stars: [1, 2_127, 11_193],
    },
    Figures {
        run: [8_519, 17_814, 19_213],
        paths: [
            [[0; 5]; 7],
            [
                [148, 0, 148, 5, 5],
                [0; 5],
                [10, 0, 10, 0, 0],
                [0; 5],
                [0; 5],
                [36, 0, 36, 35, 35],
                [0; 5],
            ],
            [
                [11, 0, 11, 0, 0],
                [0; 5],
                [0; 5],
                [0; 5],
                [0; 5],
                [2, 0, 2, 0, 0],
                [0; 5],
            ],
        ],
        candidates: [
            [[59, 0], [15, 0]],
            [[124, 55], [319, 234]],
            [[5, 5], [25, 21]],
        ],
        neutron_stars: [1, 2_072, 11_190],
    },
];

/// A census of 6 × 10⁴ generated pairs ([`census`]) under every 29 hold and every 30 hold: under
/// every 30 hold no path collapses a dwarf, every ending is a disruption, and every candidate a
/// path pools first can detonate.
#[test]
#[ignore = "slow: 6 × 10⁴ generated pairs run through the binary engine to 13.8 Gyr, twice"]
fn every_carbon_oxygen_dwarf_at_the_chandrasekhar_mass_disrupts_under_every_30_hold() {
    let galaxy = Galaxy::from_params(Seed::new(CENSUS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let figures = [version(), StellarRules::every_30_hold()].map(|rules| {
        let figures = census(&galaxy, rules);
        println!("{:?}: {figures:?}", rules.co_chandrasekhar);
        figures
    });
    for (layer, paths) in figures[1].paths.iter().enumerate() {
        for (path, [events, collapses, disruptions, first, detonatable]) in
            ChandrasekharPath::ALL.iter().zip(paths)
        {
            let case = format!("layer {layer}, {path:?}");
            assert_eq!(*collapses, 0, "{case}");
            assert_eq!(disruptions, events, "{case}");
            assert_eq!(detonatable, first, "{case}");
        }
    }
    assert_eq!(figures, CENSUS_FIGURES);
}
