//! The tests of P08.T12's hold at 27, `PLACEMENT_FATES_VERSION` (plan 08, P08.T12 at 27, the
//! redesign research-leans-19 §3 sketched): under every 27 hold a dead record's marks read its
//! progenitor's fate from the fitted fate table where it can stand for the track's.
//!
//! - The table read's premises, over sweeps (slow, over finer ones): below `HD_FREE_MASSES` the
//!   Humphreys–Davidson arms build the same fates, bit for bit, and the companion-stripped mark
//!   moves no iron core's or electron capture's fate.
//!
//! The tripwire is here. The bump to 27 deletes the held arm, and these become tests of the
//! version's own galaxy.

use std::sync::OnceLock;

use hyperion_testkit::threads::in_shares;

use super::marks::{HD_FREE_MASSES, PLACEMENT_FATES_VERSION, PlacementFateRules, hd_free_mass};
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::id::{BodyId, SystemId};
use crate::math;
use crate::rng::{Mark, Seed};
use crate::stellar::draws::{StandardNormal, StarDraws};
use crate::stellar::fates::node_draws;
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::{TrackOptions, fate_of};
use crate::stellar::{Composition, Phase};
use crate::units::{Dex, HeliumExcess, SolarMasses};

/// The seed of the hold's Milky Way fixtures.
const SEED: u64 = 0x0812_f000_0000_0001;

/// The Milky Way fixture under every 27 hold, the fate table read, with its kinematic tables, which
/// a globular's members need.
fn fitted() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way fixture builds")
        .with_full_potential()
    })
}

/// The tripwire of the hold: below 27 every dead record's fate is its track's.
#[test]
fn the_placement_fates_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < PLACEMENT_FATES_VERSION,
        "the 26 → 27 bump reads a dead record's fate from the fate table where it can stand for \
         the track's: the ages of most of layer E's dead records and of the hypervelocity \
         survivors, and so their cells' records, move. Delete `PLACEMENT_FATES_VERSION`, \
         `PlacementFateRules` with its `Exact` arm, `GalaxyRules::placement_fates`, `fate`'s \
         test of it and this test in the bump's own commit (`HD_FREE_MASSES` and `hd_free_mass` \
         go with `HdLimitRules`' `Belczynski` arm, once the fate tables are refit under the \
         version's), and re-bless the goldens plan 08's \"P08.T12 at 27, as built\" lists"
    );
    assert_eq!(
        GalaxyRules::of_version(GENERATOR_VERSION).placement_fates,
        PlacementFateRules::Exact
    );
    assert_eq!(fitted().rules().placement_fates, PlacementFateRules::Fitted);
}

/// The threads a slow test of the hold runs (its `threads-required` in `.config/nextest.toml`).
const SLOW_THREADS: usize = 4;

/// `f` of each of `items`, in their order, on at most `threads` threads ([`in_shares`]; one runs
/// them on this thread): share k takes the items k, k + n, …
fn over<T: Sync, R: Send>(items: &[T], threads: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let n = threads.max(1);
    let parts = in_shares(n, threads, |k| {
        items.iter().skip(k).step_by(n).map(&f).collect::<Vec<_>>()
    });
    let mut shares: Vec<_> = parts.into_iter().map(Vec::into_iter).collect();
    (0..items.len())
        .map(|i| shares[i % n].next().expect("each share holds its items"))
        .collect()
}

/// The \[Fe/H\] of `Z_FIT_MAX`, the most the formulae read.
const MOST_FE_H: f64 = 0.176;

/// The draws a premise's sweep takes at each mass and composition: the median star's at five η
/// across the table's single-node panel's (±2.4), and three records' own.
fn sweep_draws() -> Vec<StarDraws> {
    let mut draws: Vec<StarDraws> = [-2.4, -1.2, 0.0, 1.2, 2.4]
        .into_iter()
        .map(|eta| node_draws(StandardNormal::new(eta).expect("a finite η")))
        .collect();
    draws.extend((0..3).map(|k| {
        StarDraws::for_star(
            Seed::new(SEED),
            BodyId::new(
                SystemId::from_raw(0x4200_2cb2_0000_0001 + 2 * k).expect("a system ID"),
                0,
            ),
        )
    }));
    draws
}

/// The fates of a star of `m` and `composition` under every 27 hold's stellar rules and the
/// version's, for each of `draws`, as text: the same text is the same bits.
fn fates_under_both_arms(
    m: f64,
    composition: &Composition,
    draws: &[StarDraws],
) -> Vec<(String, String)> {
    let at = |rules: StellarRules, d: &StarDraws| {
        format!(
            "{:?}",
            fate_of(
                SolarMasses::new(m),
                composition,
                d,
                TrackOptions::default().with_rules(rules)
            )
        )
    };
    let held = GalaxyRules::every_27_hold().stellar;
    let version = StellarRules::of_version(GENERATOR_VERSION);
    draws
        .iter()
        .map(|d| (at(held, d), at(version, d)))
        .collect()
}

/// For each step of `HD_FREE_MASSES`, `per_step` metallicities from its least \[Fe/H\] to the
/// next's, and `masses(cut)` below its mass, every fate under every 27 hold's stellar rules is the
/// version's, and `hd_free_mass` reads the step; the number of fates compared, on `threads`.
fn hd_arms_agree(per_step: u32, masses: impl Fn(f64) -> Vec<f64>, threads: usize) -> usize {
    let draws = sweep_draws();
    let mut points = Vec::new();
    for (k, &(least, cut)) in HD_FREE_MASSES.iter().enumerate() {
        let next = HD_FREE_MASSES
            .get(k + 1)
            .map_or(MOST_FE_H, |&(fe_h, _)| fe_h);
        for i in 0..per_step {
            // Just above the least: a composition's `z_fit` read back in \[Fe/H\] can fall an
            // ulp short of the value it was made from.
            let fe_h = least + 1e-9 + (next - least) * f64::from(i) / f64::from(per_step);
            points.extend(masses(cut).into_iter().map(|m| (fe_h, cut, m)));
        }
    }
    over(&points, threads, |&(fe_h, cut, m)| {
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        assert_eq!(hd_free_mass(&composition), SolarMasses::new(cut), "{fe_h}");
        assert!(m < cut, "{m} {cut}");
        for (held, version) in fates_under_both_arms(m, &composition, &draws) {
            assert_eq!(held, version, "{m} M☉ at [Fe/H] {fe_h}");
        }
        draws.len()
    })
    .into_iter()
    .sum()
}

/// `n` masses, M☉, log-spaced from `lo` up to but not reaching `hi`.
fn log_masses(lo: f64, hi: f64, n: u32) -> Vec<f64> {
    (0..n)
        .map(|i| lo * math::powf(hi / lo, f64::from(i) / f64::from(n)))
        .collect()
}

/// The table read's first premise, near its edge: within a sixth below each step's mass, where a
/// star's luminosity comes nearest the limit, the Humphreys–Davidson arms build the same fate,
/// bit for bit, at three metallicities of each step and eight draws.
#[test]
fn below_the_hd_free_masses_the_arms_build_the_same_fates() {
    let compared = hd_arms_agree(3, |cut| log_masses(0.85 * cut, cut, 12), 1);
    assert_eq!(
        compared,
        HD_FREE_MASSES.len() * 3 * 12 * sweep_draws().len()
    );
}

/// The first premise over every mass the table holds, 0.741 M☉ to each step's, at metallicities
/// 0.05 dex apart or nearer.
#[test]
#[ignore = "slow: some 60,000 tracks, twice"]
fn below_the_hd_free_masses_the_arms_build_the_same_fates_over_every_mass() {
    let compared = hd_arms_agree(10, |cut| log_masses(0.741, cut, 160), SLOW_THREADS);
    println!("{compared} fates compared");
}

/// For each of `metallicities`, `etas` and `n` masses over the stripped window, the single
/// star's fate and the companion-stripped one's: where the single star's collapses an iron core
/// or captures electrons, the two are the same, bit for bit. The fates compared there, and the
/// number of the others the mark moved.
fn stripped_mark_agrees(metallicities: &[f64], etas: &[f64], n: u32) -> (u32, u32) {
    let rules = GalaxyRules::every_27_hold().stellar;
    let options = TrackOptions::default().with_rules(rules);
    let (mut compared, mut moved) = (0, 0);
    for &fe_h in metallicities {
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        for &eta in etas {
            let single = node_draws(StandardNormal::new(eta).expect("a finite η"));
            let stripped = single.clone().with_stripped(Mark::from_word(0));
            for i in 0..=n {
                let m = SolarMasses::new(5.5 + 5.5 * f64::from(i) / f64::from(n));
                let alone = fate_of(m, &composition, &single, options);
                let with = fate_of(m, &composition, &stripped, options);
                let same = format!("{alone:?}") == format!("{with:?}");
                if alone.iron_core.is_some() || alone.phase == Phase::NeutronStar {
                    assert!(same, "{m:?} at [Fe/H] {fe_h}, η {eta}: {alone:?}, {with:?}");
                    compared += 1;
                } else if !same {
                    moved += 1;
                }
            }
        }
    }
    (compared, moved)
}

/// The table read's second premise: over the stripped window, 5.5–11 M☉, at four metallicities
/// and three η, the companion-stripped mark moves no iron core's or electron capture's fate, bit
/// for bit, and does move some white dwarfs', so that the sweep reaches the window.
#[test]
fn the_stripped_mark_moves_no_iron_core_or_electron_capture() {
    let (compared, moved) =
        stripped_mark_agrees(&[-2.3, -0.5, 0.086, MOST_FE_H], &[-3.0, 0.0, 3.0], 40);
    assert!(compared > 200, "{compared}");
    assert!(moved > 0, "{moved}");
}

/// The second premise over a finer sweep: 400 masses, eight metallicities and five η.
#[test]
#[ignore = "slow: 16,000 tracks, twice"]
fn the_stripped_mark_moves_no_iron_core_or_electron_capture_over_a_finer_sweep() {
    let (compared, moved) = stripped_mark_agrees(
        &[-2.3, -1.5, -1.0, -0.5, -0.25, 0.0, 0.1, MOST_FE_H],
        &[-3.0, -1.5, 0.0, 1.5, 3.0],
        399,
    );
    println!("{compared} iron cores and electron captures the same; {moved} others moved");
    assert!(moved > 0);
}
