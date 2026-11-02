//! The tests of P11.T4's dwarf-limit fix, [`DWARF_LIMIT_VERSION`]: from it the binary engine shows
//! no white dwarf at or above the Chandrasekhar mass, and types the dwarf a merger's unplaced
//! product leaves by its core, only under the rules that turn it on, read by `Engine::new_star`'s
//! fallback, `Engine::finish_merger` and the accretor's events through the engine's input's rules.
//! The engine's own cases are `stellar::binary::tests`' (P11.T4's dwarf limit there).
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - Slow: a census of 6 × 10⁴ pairs of layers C, D and E near the Sun, as the generator draws
//!   them, run to 13.8 Gyr: under the version's rules some show a white dwarf above the
//!   Chandrasekhar mass, under every 27 hold none does.
//!
//! The movers under the arm, found by turning it on at the version (and back off), over the sim's
//! tests (with its `testing` feature too) and the binaries' slow censuses (2026-11-02): see the
//! records of P11.T4 in plan 11's Risks.

use hyperion_testkit::threads::in_shares;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::id::Layer;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::dwarf_limit::{DWARF_LIMIT_VERSION, DwarfLimitRules};
use crate::stellar::binary::testing::{generated_pairs, shows_a_dwarf_above_the_limit};
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

/// The version's rules keep the held arm, every version below 27 does and 27 holds the dwarfs
/// below the Chandrasekhar mass; the fitted tables hold only under the version's arm. The
/// tripwire: the bump to 27 deletes this hold.
#[test]
fn the_dwarf_limit_is_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < DWARF_LIMIT_VERSION,
        "the bump to 27 deletes P11.T4's dwarf-limit hold: `DWARF_LIMIT_VERSION`, \
         `DwarfLimitRules` with its `Unchecked` arm, `StellarRules::dwarf_limit`, its reads in \
         `Engine::new_star`, `Engine::finish_merger`, the dwarfs' merger and the accretor's \
         events, the brief's read of `DwarfLimitRules::fitted_tables_hold`, this module and its \
         line in `stellar/rules.rs`, and refits the pair-light and reach tables"
    );
    assert_eq!(version().dwarf_limit, DwarfLimitRules::Unchecked);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).dwarf_limit;
    for v in [GENERATOR_VERSION.get(), 25, 26] {
        assert_eq!(at(v), DwarfLimitRules::Unchecked, "version {v}");
    }
    assert_eq!(at(27), DwarfLimitRules::Chandrasekhar);
    assert_eq!(
        StellarRules::every_27_hold().dwarf_limit,
        DwarfLimitRules::Chandrasekhar
    );
    assert!(version().dwarf_limit.fitted_tables_hold());
    assert!(!DwarfLimitRules::Chandrasekhar.fitted_tables_hold());
}

/// What the census finds under one arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Figures {
    /// The pairs run, those that can interact by [`UNTIL`], in layers C, D and E.
    run: [usize; 3],
    /// Of those, the pairs that show a white dwarf above the Chandrasekhar mass.
    above: [usize; 3],
    /// Of those, the pairs left with no star at their age at the epoch (both members
    /// `NoRemnant`).
    massless: [usize; 3],
}

/// The census of [`PAIRS`] pairs of each of layers C, D and E in `galaxy`, each run under `rules`.
fn census(galaxy: &Galaxy, rules: StellarRules) -> Figures {
    let mut figures = Figures {
        run: [0; 3],
        above: [0; 3],
        massless: [0; 3],
    };
    for (k, layer) in [Layer::C, Layer::D, Layer::E].into_iter().enumerate() {
        let seed = CELLS_SEED + u64::try_from(k).expect("a small index");
        let pairs = generated_pairs(galaxy, layer, PAIRS, seed);
        let shares = in_shares(SHARES, THREADS, |share| {
            let mut counts = [0_usize; 3];
            for (input, age) in pairs.iter().skip(share).step_by(SHARES) {
                let input = input.clone().with_rules(rules);
                if !can_interact(&input, Years::new(UNTIL)) {
                    continue;
                }
                let timeline = evolve(&input, Years::new(UNTIL));
                let at_age = timeline.state_at(Years::new(*age));
                let [a, b] = at_age.stars();
                counts[0] += 1;
                counts[1] += usize::from(shows_a_dwarf_above_the_limit(&timeline, UNTIL));
                counts[2] +=
                    usize::from(a.phase() == Phase::NoRemnant && b.phase() == Phase::NoRemnant);
            }
            counts
        });
        for counts in shares {
            figures.run[k] += counts[0];
            figures.above[k] += counts[1];
            figures.massless[k] += counts[2];
        }
    }
    figures
}

/// The census's figures under the version's rules and under every 27 hold, as measured
/// (2026-11-02, at generator version 25).
const CENSUS_FIGURES: [Figures; 2] = [
    Figures {
        run: [5_983, 15_222, 18_144],
        above: [0, 94, 157],
        massless: [179, 22, 2],
    },
    Figures {
        run: [5_983, 15_222, 18_105],
        above: [0; 3],
        massless: [149, 18, 2],
    },
];

/// A census of 6 × 10⁴ generated pairs ([`census`]): under the version's rules some show a white
/// dwarf above the Chandrasekhar mass, under every 27 hold none does.
#[test]
#[ignore = "slow: 1.2 × 10⁵ generated pairs run through the binary engine to 13.8 Gyr"]
fn no_white_dwarf_passes_the_chandrasekhar_mass_under_every_27_hold() {
    let galaxy = Galaxy::from_params(Seed::new(CENSUS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let before = census(&galaxy, version());
    let after = census(&galaxy, StellarRules::every_27_hold());
    println!("version's rules: {before:?}\nevery 27 hold: {after:?}");
    assert!(before.above.iter().sum::<usize>() > 0, "{before:?}");
    assert_eq!(after.above, [0; 3], "{after:?}");
    assert_eq!([before, after], CENSUS_FIGURES);
}
