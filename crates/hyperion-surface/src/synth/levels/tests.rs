//! The level sets' and the level bound's tests (plan R09, T8): the sets nest and reach every
//! contribution at the band limit, and on every synthetic world the height at a level stays within
//! its bound of every finer level's.

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::field::{CoarseField, FieldView};
use crate::synth::test_support::{SEED, random_cells, random_points_in, rim_cells, world};
use crate::synth::{SynthCache, Synthesiser};
use crate::testing::SyntheticWorld;

/// Every band level, 0 to the deepest.
fn every_level() -> impl Iterator<Item = BandLevel> {
    (0..=MAX_LEVEL).map(|n| BandLevel::new(n).unwrap())
}

/// The first level whose bound is zero: from it on, no finer level's height differs.
fn complete_level(bounds: &LevelBounds) -> u8 {
    every_level()
        .find(|&n| bits(bounds.bound_m(n)) == bits(0.0))
        .expect("the deepest level's bound is zero")
        .get()
}

/// The shallowest level that resolves the band limit, from which every level evaluates the same
/// set with the same band.
fn band_limit_level(bounds: &LevelBounds) -> u8 {
    every_level()
        .find(|&n| bounds.set(n).shortest_wavelength().value() <= BAND_LIMIT_M)
        .expect("the deepest level resolves the band limit")
        .get()
}

#[test]
fn every_level_s_set_nests_and_the_band_limit_s_level_evaluates_everything() {
    for &kind in SyntheticWorld::ALL {
        let field = world(kind);
        let header = field.header();
        let bounds = LevelBounds::new(header);
        let octaves = CraterOctaves::new(header);
        let radius = header.radius();
        let finest_relief = finest_octave(radius);
        let last_crater = octaves
            .octaves()
            .last()
            .map(super::super::craters::CraterOctave::index);
        let mut previous: Option<LevelSet> = None;
        for level in every_level() {
            let set = *bounds.set(level);
            assert_eq!(set.level(), level);
            if let Some(p) = previous {
                assert!(set.shortest_wavelength() <= p.shortest_wavelength());
                assert!(
                    set.relief_through() >= p.relief_through(),
                    "{kind:?} {level:?}"
                );
                assert!(
                    set.channels_through() >= p.channels_through(),
                    "{kind:?} {level:?}"
                );
                assert!(
                    set.craters_through() >= p.craters_through(),
                    "{kind:?} {level:?}"
                );
            }
            // R05's rule: octave m at level n where its lattice spacing is at least four of the
            // level's largest vertex spacings, n + 4 on these figures; channels to n + 3; and
            // from the band limit's level on, each to its own 2 m.
            let coarse = header.level().get();
            let limited = set.shortest_wavelength().value() <= BAND_LIMIT_M;
            let deepest_cells_over = |metres: f64| {
                (coarse..=MAX_LEVEL)
                    .filter(|&k| mean_cell_width(radius.value(), k) >= metres)
                    .max()
            };
            if let Some(through) = set.relief_through() {
                let expected = if limited {
                    finest_relief
                } else {
                    level.get() + 4
                };
                assert_eq!(through, expected.min(finest_relief), "{kind:?} {level:?}");
            }
            if let Some(through) = set.channels_through() {
                assert!(through >= coarse);
                if limited {
                    assert_eq!(Some(through), deepest_cells_over(BAND_LIMIT_M), "{kind:?}");
                } else {
                    assert_eq!(through, level.get() + 3, "{kind:?} {level:?}");
                }
            }
            if let Some(last) = set.craters_through() {
                let octave = &octaves.octaves()[usize::from(last)];
                assert!(octave.smallest() >= set.shortest_wavelength());
                if let Some(next) = octaves.octaves().get(usize::from(last) + 1) {
                    assert!(next.smallest() < set.shortest_wavelength());
                }
            }
            previous = Some(set);
        }
        // From the level that resolves the band limit on, everything.
        let full = *bounds.set(BandLevel::new(band_limit_level(&bounds)).unwrap());
        assert_same_bits(full.shortest_wavelength().value(), BAND_LIMIT_M);
        assert_eq!(
            full.relief_through(),
            (first_octave(header.level()) <= finest_relief).then_some(finest_relief)
        );
        assert_eq!(full.craters_through(), last_crater, "{kind:?}");
        let deepest_channels = deepest_possible_level(radius.value(), header.level().get());
        assert!(
            full.channels_through()
                .is_some_and(|c| c >= deepest_channels)
        );
        // R05's finest level resolves the band limit.
        let finest = crate::geometry::finest_level(header.figure().equatorial_radius_m);
        assert!(band_limit_level(&bounds) <= finest, "{kind:?}");
    }
}

#[test]
fn the_level_bound_is_zero_from_the_complete_level_and_never_rises_with_the_level() {
    for &kind in SyntheticWorld::ALL {
        let header = world(kind).header();
        let bounds = LevelBounds::new(header);
        let complete = complete_level(&bounds);
        let limit = band_limit_level(&bounds);
        assert!(complete <= limit);
        let banded = !matches!(kind, SyntheticWorld::Flat | SyntheticWorld::OneCrater);
        if banded {
            assert_eq!(complete, limit, "{kind:?}");
        }
        let mut previous = f64::INFINITY;
        for level in every_level() {
            let bound = bounds.bound_m(level);
            assert!(bound.is_finite() && bound >= 0.0, "{kind:?} {level:?}");
            assert!(
                bound <= previous,
                "{kind:?}: {bound} m at {level:?} over {previous} m"
            );
            assert_eq!(
                bits(bound) == bits(0.0),
                level.get() >= complete,
                "{kind:?} {level:?}"
            );
            previous = bound;
            let (low, high) = bounds.contribution_range_m(level);
            assert!(low <= 0.0 && high >= 0.0 && low.is_finite() && high.is_finite());
        }
        assert_same_bits(
            level_bound_m(header, BandLevel::new(9).unwrap()),
            bounds.bound_m(BandLevel::new(9).unwrap()),
        );
    }
}

/// The largest |h(n) − h(m)| ÷ `level_bound_m(n)` over the levels m finer than n, per level n
/// below the band limit's, at `points` points in each of `cells` random cells of `field`'s coarse
/// level; it asserts that no difference exceeds its bound, and that the band limit's level and
/// every finer one give the same height to the bit.
fn worst_ratios(field: &CoarseField, cells: usize, points: usize, seed: u64) -> Vec<f64> {
    let synth = Synthesiser::new(field, SEED);
    let bounds = synth.bounds();
    let complete = band_limit_level(bounds);
    let mut rng = Lcg::new(seed);
    let mut worst = vec![0.0_f64; usize::from(complete)];
    let mut sampled = random_cells(field.header().level().get(), cells, &mut rng);
    sampled.extend(rim_cells(field, cells / 10));
    for cell in sampled {
        let mut cache = SynthCache::new();
        for dir in random_points_in(cell, points, &mut rng) {
            let heights: Vec<f64> = (0..=complete)
                .map(|n| {
                    synth
                        .height_at(&mut cache, dir, BandLevel::new(n).unwrap())
                        .unwrap()
                        .height_m
                })
                .collect();
            for n in 0..complete {
                let bound = bounds.bound_m(BandLevel::new(n).unwrap());
                for m in n + 1..=complete {
                    let gap = (heights[usize::from(n)] - heights[usize::from(m)]).abs();
                    assert!(
                        gap <= bound,
                        "{dir:?}: |h({n}) − h({m})| = {gap} m over the bound {bound} m"
                    );
                    if bound > 0.0 {
                        worst[usize::from(n)] = num::max(worst[usize::from(n)], gap / bound);
                    }
                }
            }
            // Beyond the complete level the heights are its own, to the bit.
            let beyond = synth
                .height_at(&mut cache, dir, BandLevel::new(MAX_LEVEL).unwrap())
                .unwrap()
                .height_m;
            assert_eq!(bits(beyond), bits(heights[usize::from(complete)]));
        }
    }
    worst
}

#[test]
#[ignore = "a measurement of the bound's terms for the plan's record, run by hand"]
fn measure_the_bound_s_terms() {
    for &kind in SyntheticWorld::ALL {
        let header = world(kind).header();
        let octaves = CraterOctaves::new(header);
        let bounds = LevelBounds::of(header, &octaves);
        let relief = Octaves::of(header);
        let channels = ChannelCaps::of(header);
        for level in every_level() {
            let set = bounds.set(level);
            let (r, c) = (
                relief.omitted_m(set.relief),
                channels.omitted_m(set.channels),
            );
            let total = bounds.bound_m(level);
            eprintln!(
                "{kind:?} {:>2}: λ {:>9.1} m, bound {total:>10.2} m = relief {r:>9.2} + channels \
                 {c:>9.2} + craters {:>9.2}; set {:?} {:?} {:?}",
                level.get(),
                set.shortest_wavelength().value(),
                total - r - c,
                set.relief,
                set.channels,
                set.craters,
            );
        }
    }
}

#[test]
fn the_level_bound_holds_between_every_pair_of_levels() {
    // 10³ points a world, 25 in each of 40 cells (the slow test below takes 10⁴).
    for (k, &kind) in SyntheticWorld::ALL.iter().enumerate() {
        let worst = worst_ratios(
            world(kind),
            40,
            25,
            0x626f_756e_6400 + u64::try_from(k).unwrap(),
        );
        eprintln!("{kind:?}: worst |h(n) − h(m)| ÷ bound by level {worst:.3?}");
    }
}

#[test]
#[ignore = "slow: 10⁴ points a world at every level, about a minute in release"]
fn the_level_bound_holds_between_every_pair_of_levels_at_ten_thousand_points() {
    for (k, &kind) in SyntheticWorld::ALL.iter().enumerate() {
        let worst = worst_ratios(
            world(kind),
            400,
            25,
            0x736c_6f77_0000 + u64::try_from(k).unwrap(),
        );
        eprintln!("{kind:?}: worst |h(n) − h(m)| ÷ bound by level {worst:.3?}");
    }
}
