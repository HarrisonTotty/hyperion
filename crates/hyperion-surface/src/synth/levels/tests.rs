//! The level sets' and the level bound's tests (plan R09, T8): the sets nest and reach every
//! contribution at the band limit, and on every synthetic world the height at a level stays within
//! its bound of every finer level's.

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::cube::{Face, PatchKey};
use crate::field::{CoarseField, FieldView, cell_at_index};
use crate::synth::test_support::{
    SEED, point_in, random_cells, random_points_in, rim_cells, world,
};
use crate::synth::{SynthCache, Synthesiser};
use crate::test_sizes::per_target;
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

/// The base elevation's range over every cell of `field`, metres, widened for rounding as the
/// client's worker widens the cells it holds, and what all its cells certify of the channels:
/// the view of a field holding every cell (R09.T8.d (b2)).
fn held_over(field: &CoarseField) -> ((f64, f64), HeldChannels) {
    let level = field.header().level();
    let records: Vec<&SynthesisCell> = (0..level.cell_count())
        .map(|n| field.cell(cell_at_index(level.get(), n).unwrap()).unwrap())
        .collect();
    let (lo, hi) = records.iter().fold((i32::MAX, i32::MIN), |(lo, hi), r| {
        (lo.min(r.elevation_mm), hi.max(r.elevation_mm))
    });
    let (lo, hi) = (f64::from(lo) / 1000.0, f64::from(hi) / 1000.0);
    let slack = |x: f64| 1e-6 + 1e-12 * x.abs();
    let held = HeldChannels::of(records, ChannelBody::of(field.header()));
    ((lo - slack(lo), hi + slack(hi)), held)
}

/// Points on the cube's seams at coarse level `level`: on each face, its four corners, the cube's,
/// and a point on each of its four edges, each exactly on the seam.
fn seam_points(level: u8) -> Vec<[f64; 3]> {
    let last = (1_u32 << level) - 1;
    let mid = last / 2;
    let cell = |face, i, j| PatchKey::new(face, level, i, j).unwrap();
    Face::ALL
        .iter()
        .flat_map(|&face| {
            [
                point_in(cell(face, 0, 0), 0.0, 0.0),
                point_in(cell(face, last, 0), 1.0, 0.0),
                point_in(cell(face, last, last), 1.0, 1.0),
                point_in(cell(face, 0, last), 0.0, 1.0),
                point_in(cell(face, mid, 0), 0.37, 0.0),
                point_in(cell(face, last, mid), 1.0, 0.29),
                point_in(cell(face, mid, last), 0.83, 1.0),
                point_in(cell(face, 0, mid), 0.0, 0.61),
            ]
        })
        .collect()
}

/// What [`sample_levels`] finds: by level n below the band limit's, the largest
/// |h(n) − h(m)| ÷ `level_bound_m(n)` over the finer levels m, and by level to the band limit's,
/// the [`Extremes`] of the contributions.
struct Sampled {
    worst: Vec<f64>,
    extremes: Vec<Extremes>,
}

/// Samples `field` at `points` points in each of `cells` random cells of its coarse level, and in a
/// tenth as many cells on coarse rims, and at the cube's corners and on its face edges
/// ([`seam_points`]), at every level to the band limit's. It asserts that no difference between
/// two levels exceeds the coarser one's bound, that the band limit's level and every finer one give
/// the same height to the bit, that every contribution stays within its own term of the level's
/// reach and their sum within [`LevelBounds::contribution_range_m`] (R09.T8.d): the octaves and the
/// channels hard, the small craters, the rims and both together within their quantiles; and that
/// the height as delivered stays within the range the whole field's cells certify
/// ([`LevelBounds::held_height_range_m`], R09.T8.d (b2)), itself within the header's.
fn sample_levels(field: &CoarseField, cells: usize, points: usize, seed: u64) -> Sampled {
    let synth = Synthesiser::new(field, SEED);
    let bounds = synth.bounds();
    let shares = term_shares(field.header());
    let (base, held) = held_over(field);
    let complete = band_limit_level(bounds);
    let mut rng = Lcg::new(seed);
    let mut worst = vec![0.0_f64; usize::from(complete)];
    let mut extremes = vec![[(f64::INFINITY, f64::NEG_INFINITY); 5]; usize::from(complete) + 1];
    let mut sampled = random_cells(field.header().level().get(), cells, &mut rng);
    sampled.extend(rim_cells(field, cells / 10));
    let mut groups: Vec<Vec<[f64; 3]>> = sampled
        .into_iter()
        .map(|cell| random_points_in(cell, points, &mut rng))
        .collect();
    groups.push(seam_points(field.header().level().get()));
    for group in groups {
        let mut cache = SynthCache::new();
        for dir in group {
            let mut heights = Vec::with_capacity(usize::from(complete) + 1);
            for n in 0..=complete {
                let level = BandLevel::new(n).unwrap();
                let parts = synth.contributions(&mut cache, dir, level).unwrap();
                let (relief, channels, craters, rims) = (
                    parts.relief.height_m,
                    parts.channels.height_m,
                    parts.craters.height_m,
                    parts.rims.height_m,
                );
                let added = relief + channels + craters + rims;
                let s = &shares[usize::from(n)];
                let (low, high) = bounds.contribution_range_m(level);
                assert!(
                    relief.abs() <= s.relief_reach,
                    "{dir:?} {n}: relief {relief} m"
                );
                assert!(
                    -s.channels_low <= channels && channels <= s.channels_high,
                    "{dir:?} {n}: channels {channels} m"
                );
                assert!(
                    craters.abs() <= s.small_reach,
                    "{dir:?} {n}: craters {craters} m"
                );
                assert!(rims.abs() <= s.rims_reach, "{dir:?} {n}: rims {rims} m");
                assert!(
                    (craters + rims).abs() <= s.craters_reach,
                    "{dir:?} {n}: craters and rims {} m",
                    craters + rims
                );
                assert!(
                    low <= added && added <= high,
                    "{dir:?} {n}: {added} m outside [{low}, {high}] m"
                );
                let values = [relief, channels, craters, rims, added];
                for (slot, v) in extremes[usize::from(n)].iter_mut().zip(values) {
                    slot.0 = num::min(slot.0, v);
                    slot.1 = num::max(slot.1, v);
                }
                // The height as delivered, the channels' floor at the sea included (R09.T6.f),
                // which keeps it between the sum with the channels and the sum without them.
                let height = synth.height_at(&mut cache, dir, level).unwrap().height_m;
                let floored = height - parts.base.height_m;
                assert!(
                    low <= floored && floored <= high,
                    "{dir:?} {n}: {floored} m outside [{low}, {high}] m"
                );
                let (held_low, held_high) = bounds.held_height_range_m(level, base, &held);
                assert!(
                    held_low <= height && height <= held_high,
                    "{dir:?} {n}: {height} m outside the held [{held_low}, {held_high}] m"
                );
                assert!(base.0 + low <= held_low && held_high <= base.1 + high);
                heights.push(height);
            }
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
    Sampled { worst, extremes }
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
fn the_level_bound_and_the_reach_hold_at_every_level() {
    // 10³ points a world, 25 in each of 40 cells, and 250 in 10 on the browser target, one of
    // them on a rim (`crate::test_sizes`); the slow test below takes 10⁴.
    for (k, &kind) in SyntheticWorld::ALL.iter().enumerate() {
        let sampled = sample_levels(
            world(kind),
            per_target(40, 10),
            25,
            0x626f_756e_6400 + u64::try_from(k).unwrap(),
        );
        eprintln!(
            "{kind:?}: worst |h(n) − h(m)| ÷ bound by level {:.3?}",
            sampled.worst
        );
    }
}

#[test]
#[ignore = "slow: 10⁴ points a world at every level, about three minutes in release"]
fn the_level_bound_and_the_reach_hold_at_every_level_at_ten_thousand_points() {
    for (k, &kind) in SyntheticWorld::ALL.iter().enumerate() {
        let sampled = sample_levels(
            world(kind),
            400,
            25,
            0x736c_6f77_0000 + u64::try_from(k).unwrap(),
        );
        eprintln!(
            "{kind:?}: worst |h(n) − h(m)| ÷ bound by level {:.3?}",
            sampled.worst
        );
    }
}

/// The most the reach may be over a world's realised relief, its highest coarse elevation less its
/// lowest: 7, set from the synthetic worlds (R09.T8.d), whose largest |reach| over every level is
/// 3.8 (Ceres-like), 3.9 (Moon-like), 4.4 (Mars-like) and 6.7 (Earth-like) times their relief. What
/// remains is the craters' terms, each a 1 − 10⁻⁹ per-point quantile at saturation of fresh
/// profiles, and on a world with air the channels' geometric caps.
const REACH_OVER_RELIEF: f64 = 7.0;

#[test]
fn the_reach_is_within_its_stated_factor_of_the_realised_relief() {
    // The flat and one-crater worlds stand apart: their crater density is zero, but the rims'
    // count reads Trask's saturation whatever the production, as the coarse pass's draw may.
    let worlds = [
        SyntheticWorld::EarthLike,
        SyntheticWorld::MarsLike,
        SyntheticWorld::MoonLike,
        SyntheticWorld::CeresLike,
    ];
    for kind in worlds {
        let header = world(kind).header();
        let bounds = LevelBounds::new(header);
        let reach = every_level().fold(0.0_f64, |r, level| {
            let (low, high) = bounds.contribution_range_m(level);
            num::max(r, num::max(-low, high))
        });
        let relief = header.realised_relief().value();
        assert!(
            reach <= REACH_OVER_RELIEF * relief,
            "{kind:?}: reach {reach} m over {REACH_OVER_RELIEF} × the relief {relief} m"
        );
    }
}

#[test]
fn an_airless_body_s_channels_reach_nothing_and_omit_nothing() {
    // The four airless worlds, the Moon-like, the Ceres-like, the flat and the one-crater, so that
    // none drops out of the check unseen.
    let airless_worlds: Vec<SyntheticWorld> = SyntheticWorld::ALL
        .iter()
        .copied()
        .filter(|&kind| world(kind).header().is_airless())
        .collect();
    assert_eq!(
        airless_worlds,
        [
            SyntheticWorld::MoonLike,
            SyntheticWorld::CeresLike,
            SyntheticWorld::Flat,
            SyntheticWorld::OneCrater
        ]
    );
    for &kind in SyntheticWorld::ALL {
        let header = world(kind).header();
        let airless = header.surface_pressure().value() <= 0.0;
        let channels = ChannelCaps::of(header);
        for n in 0..BAND_LEVELS {
            let through = Some(u8::try_from(n).unwrap());
            let (low, high) = channels.reach_m(through);
            let omitted = channels.omitted_m(through);
            // Zero, of either sign: an empty sum of floats is −0.
            if airless {
                for zero in [low, high, omitted] {
                    assert_same_bits(zero.abs(), 0.0);
                }
            }
        }
        // A body with air keeps its geometric caps: some level leaves channels out.
        if !airless {
            assert!(channels.omitted_m(None) > 0.0, "{kind:?}");
        }
    }
    // A tenuous body, 1 Pa, keeps an atmosphere and may have had a wet epoch: its channel terms
    // are the geometric caps', as they were on every body before R09.T8.d.
    let mut parts = world(SyntheticWorld::MoonLike).header().parts().clone();
    parts.surface_pressure = hyperion_base::units::Pascals::new(1.0);
    let tenuous = FieldHeader::new(parts).unwrap();
    let channels = ChannelCaps::of(&tenuous);
    let radius_m = tenuous.radius().value();
    let first = tenuous.level().get();
    let caps: f64 = (first..=deepest_possible_level(radius_m, first))
        .map(|k| groove_cap_m(radius_m, k))
        .sum();
    assert_same_bits(
        channels.omitted_m(None),
        ChannelCaps::correction_factor() * caps,
    );
    assert!(channels.omitted_m(None) > 0.0);
}

#[test]
fn the_held_range_is_the_header_s_with_its_caps_whole_and_never_wider() {
    // A certificate that holds nothing back: an epoch's valleys, tributaries and trunks as deep as
    // any cap.
    let whole = HeldChannels {
        epoch: true,
        tributaries: true,
        trunk_m: f64::INFINITY,
    };
    for &kind in SyntheticWorld::ALL {
        let field = world(kind);
        let header = field.header();
        let bounds = LevelBounds::new(header);
        let (base, held) = held_over(field);
        let coarse_cap = groove_cap_m(header.radius().value(), header.level().get());
        // What each world's cells certify (R09.T8.d (b2)).
        match kind {
            SyntheticWorld::EarthLike => {
                assert!(!held.epoch && held.tributaries, "{held:?}");
                assert!(0.0 < held.trunk_m && held.trunk_m < coarse_cap, "{held:?}");
            }
            SyntheticWorld::MarsLike => {
                assert!(held.epoch && !held.tributaries, "{held:?}");
                assert!(0.0 < held.trunk_m && held.trunk_m < coarse_cap, "{held:?}");
            }
            SyntheticWorld::MoonLike
            | SyntheticWorld::CeresLike
            | SyntheticWorld::Flat
            | SyntheticWorld::OneCrater => {
                assert_eq!(held, HeldChannels::default(), "{kind:?}");
            }
        }
        let mut tightened = false;
        for level in every_level() {
            let n = usize::from(level.get());
            let (low, high) = bounds.contribution_range_m(level);
            let (whole_low, whole_high) = bounds.held_height_range_m(level, base, &whole);
            assert_same_bits(whole_low, base.0 + low);
            assert_same_bits(whole_high, base.1 + high);
            let (held_low, held_high) = bounds.held_height_range_m(level, base, &held);
            assert!(
                whole_low <= held_low && held_high <= whole_high,
                "{kind:?} {n}: [{held_low}, {held_high}] outside [{whole_low}, {whole_high}]"
            );
            tightened |= whole_low < held_low || held_high < whole_high;
            if header.is_airless() {
                assert_same_bits(held_low, whole_low);
                assert_same_bits(held_high, whole_high);
            }
            // On a world wet now the channels add nothing to the lowest height: the sea floor
            // holds, and the sea lies above the base's lowest.
            if kind == SyntheticWorld::EarthLike {
                let others = bounds.relief_reach_m[n] + bounds.craters_reach_m[n];
                assert_same_bits(held_low, base.0 + -others);
            }
        }
        assert_eq!(tightened, !header.is_airless(), "{kind:?}");
    }
}

#[test]
fn a_level_at_or_beyond_the_coarse_band_reaches_no_rim() {
    for &kind in SyntheticWorld::ALL {
        let header = world(kind).header();
        let figure = header.figure();
        let coarse = coarse_band_half_width(&figure, header.level()).value();
        let shares = term_shares(header);
        for (n, s) in shares.iter().enumerate() {
            if band_half_width(&figure, band(n)).value() >= coarse {
                assert_same_bits(s.rims_reach, 0.0);
            } else {
                assert!(s.rims_reach > 0.0, "{kind:?} {n}");
            }
        }
    }
}

/// Each level's reach and bound split by term, for the measurement below: the structural octaves',
/// the channels' (low and high for the reach), the small craters' alone, the coarse rims' alone
/// and both craters' together, each at [`CRATER_BOUND_TAIL`].
#[derive(Debug, Clone, Copy, Default)]
struct TermShares {
    relief_reach: f64,
    channels_low: f64,
    channels_high: f64,
    small_reach: f64,
    rims_reach: f64,
    craters_reach: f64,
    relief_omitted: f64,
    channels_omitted: f64,
    small_change: f64,
    rims_change: f64,
    craters_change: f64,
}

/// [`TermShares`] at every band level of the body of `header`.
fn term_shares(header: &FieldHeader) -> Vec<TermShares> {
    let octaves = CraterOctaves::new(header);
    let terms = BodyTerms::of(header, &octaves);
    let ln_tail = -math::ln(CRATER_BOUND_TAIL);
    let mut craters = CraterTerms::with_capacity(terms.marks.len() + terms.rims.len());
    (0..BAND_LEVELS)
        .map(|n| {
            let set = &terms.sets[n];
            terms.crater_terms(n, &mut craters);
            let (channels_low, channels_high) = terms.channels.reach_m(set.channels);
            // The rims' terms come last, one change and one height a coarse octave.
            let (small, rims) = craters.changes.split_at(terms.marks.len());
            let small_heights = craters.heights.len() - terms.rims.len();
            let (small_h, rims_h) = craters.heights.split_at(small_heights);
            TermShares {
                relief_reach: terms.relief.reach_m(set.relief),
                channels_low,
                channels_high,
                small_reach: poisson_tail_bound(small_h, ln_tail),
                rims_reach: poisson_tail_bound(rims_h, ln_tail),
                craters_reach: poisson_tail_bound(&craters.heights, ln_tail),
                relief_omitted: terms.relief.omitted_m(set.relief),
                channels_omitted: terms.channels.omitted_m(set.channels),
                small_change: poisson_tail_bound(small, ln_tail),
                rims_change: poisson_tail_bound(rims, ln_tail),
                craters_change: poisson_tail_bound(&craters.changes, ln_tail),
            }
        })
        .collect()
}

/// The lowest and highest of each contribution and of h − base over the sampled points, by level:
/// (relief, channels, small craters, rims, h − base), each (lowest, highest), metres.
type Extremes = [(f64, f64); 5];

#[test]
#[ignore = "a measurement of the reach's and the bound's terms for the plan's record, run by hand"]
fn measure_the_reach_s_terms() {
    for (k, &kind) in SyntheticWorld::ALL.iter().enumerate() {
        let field = world(kind);
        let header = field.header();
        let shares = term_shares(header);
        let bounds = LevelBounds::new(header);
        let sampled =
            sample_levels(field, 40, 25, 0x7265_6163_6800 + u64::try_from(k).unwrap()).extremes;
        let (lowest, highest) = (0..BAND_LEVELS).fold((0.0_f64, 0.0_f64), |(lo, hi), n| {
            let (l, h) = bounds.contribution_range_m(band(n));
            (num::min(lo, l), num::max(hi, h))
        });
        // The range the whole field's cells certify, over the base's (R09.T8.d (b2)).
        let (base, held) = held_over(field);
        let (held_lowest, held_highest) =
            (0..BAND_LEVELS).fold((0.0_f64, 0.0_f64), |(lo, hi), n| {
                let (l, h) = bounds.held_height_range_m(band(n), base, &held);
                (num::min(lo, l - base.0), num::max(hi, h - base.1))
            });
        eprintln!(
            "{kind:?}: held {held:?}, base [{:.1}, {:.1}] m, sea {:.1} m; held reach lowest \
             {held_lowest:.1} m, highest {held_highest:.1} m",
            base.0,
            base.1,
            header.sea_level().value(),
        );
        eprintln!(
            "{kind:?}: R {:.0} m, L {}, D_b {:.0} m, coarse band {:.0} m, realised relief {:.1} m, \
             σ_h {:.1} m; reach lowest {lowest:.1} m, highest {highest:.1} m",
            header.radius().value(),
            header.level().get(),
            header.boundary_diameter().value(),
            coarse_band_half_width(&header.figure(), header.level()).value(),
            header.realised_relief().value(),
            header.realised_sigma_h().value(),
        );
        for (n, s) in shares.iter().enumerate() {
            let (low, high) = bounds.contribution_range_m(band(n));
            let sampled = sampled.get(n).map_or_else(String::new, |e| {
                format!(
                    " | sampled h−base [{:.1}, {:.1}], relief [{:.1}, {:.1}], channels [{:.1}, \
                     {:.1}], craters [{:.1}, {:.1}], rims [{:.1}, {:.1}]",
                    e[4].0, e[4].1, e[0].0, e[0].1, e[1].0, e[1].1, e[2].0, e[2].1, e[3].0, e[3].1
                )
            });
            eprintln!(
                "  {n:>2}: reach [{low:.1}, {high:.1}] = relief {:.1}, channels -{:.1}/+{:.1}, \
                 craters {:.1} (small {:.1}, rims {:.1}); ε {:.2} = relief {:.2} + channels {:.2} \
                 + craters {:.2} (small {:.2}, rims {:.2}){sampled}",
                s.relief_reach,
                s.channels_low,
                s.channels_high,
                s.craters_reach,
                s.small_reach,
                s.rims_reach,
                bounds.bound_m(band(n)),
                s.relief_omitted,
                s.channels_omitted,
                s.craters_change,
                s.small_change,
                s.rims_change,
            );
        }
    }
}
