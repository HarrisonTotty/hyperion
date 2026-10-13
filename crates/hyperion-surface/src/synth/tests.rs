//! The synthesis's tests: the header's types (R09.T2–T5) and the whole height function (R09.T8):
//! its sum and its order, the cells it reads, the patch bake and its range, the closed forms of the
//! unresolved ground against the sampled ground, and a full belt's slopes.

use std::cell::RefCell;
use std::collections::BTreeSet;

use hyperion_base::math;
use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::test_support::{
    Held, Recorder, SEED, king_distances, point_in, random_cells, random_points_in, rim_cells,
    sample_bits, world,
};
use super::*;
use crate::cube::Edge;
use crate::field::{CoarseField, Cover, SYNTHESIS_MARGIN_CELLS, cell_index};
use crate::geometry::BAND_LIMIT_M;
use crate::testing::SyntheticWorld;

#[test]
fn a_band_level_is_a_quadtree_level() {
    assert_eq!(BandLevel::new(19).map(BandLevel::get), Ok(19));
    assert_eq!(
        BandLevel::try_from(MAX_LEVEL + 1),
        Err(NewBandLevelError(25))
    );
}

/// Parts of a silicate crust's law at an Earth's break, with V₁ `v1` m².
fn parts(v1: f64) -> BandSpectrumParts {
    BandSpectrumParts {
        exponent: BandSpectrum::DEFAULT_EXPONENT,
        unit_degree_variance: SquareMetres::new(v1),
        break_degree: 20_015.1,
        small_scale_exponent: 3.0,
    }
}

#[test]
fn a_band_spectrum_needs_a_summable_law_and_a_break() {
    let v = SquareMetres::new(2.253e7);
    let spectrum = BandSpectrum::new(parts(v.value())).unwrap();
    assert_eq!(spectrum.unit_degree_variance(), v);
    assert_same_bits(spectrum.break_degree(), 20_015.1);
    assert_same_bits(spectrum.small_scale_exponent(), 3.0);
    let refused = |p: BandSpectrumParts| BandSpectrum::new(p).unwrap_err();
    assert_eq!(
        refused(BandSpectrumParts {
            exponent: 1.0,
            ..parts(1.0)
        }),
        BuildBandSpectrumError::Exponent(1.0)
    );
    assert_eq!(refused(parts(-1.0)), BuildBandSpectrumError::Variance(-1.0));
    for l in [0.5, 0.0, -3.0, f64::INFINITY] {
        assert_eq!(
            refused(BandSpectrumParts {
                break_degree: l,
                ..parts(1.0)
            }),
            BuildBandSpectrumError::BreakDegree(l)
        );
    }
    assert!(matches!(
        refused(BandSpectrumParts {
            break_degree: f64::NAN,
            ..parts(1.0)
        }),
        BuildBandSpectrumError::BreakDegree(l) if l.is_nan()
    ));
    for b in [1.0, 0.7, f64::INFINITY] {
        assert_eq!(
            refused(BandSpectrumParts {
                small_scale_exponent: b,
                ..parts(1.0)
            }),
            BuildBandSpectrumError::SmallScaleExponent(b)
        );
    }
    // A break at degree 1 is a law of β₂ alone, and is accepted.
    assert!(
        BandSpectrum::new(BandSpectrumParts {
            break_degree: 1.0,
            ..parts(1.0)
        })
        .is_ok()
    );
    // Parts each in range whose closed forms overflow are refused, so that no variance the
    // relief reads is infinite or NaN: a break far beyond every degree under a far steeper
    // β₂ (its factor l_b^(β₂−β₁) overflows), with V₁ above zero and at zero, and a V₁ near
    // the largest f64.
    let overflowing = BandSpectrumParts {
        break_degree: 1e200,
        small_scale_exponent: 4.0,
        ..parts(1.0)
    };
    assert!(matches!(
        refused(overflowing),
        BuildBandSpectrumError::Total(v) if !v.is_finite()
    ));
    assert!(matches!(
        refused(BandSpectrumParts {
            unit_degree_variance: SquareMetres::ZERO,
            ..overflowing
        }),
        BuildBandSpectrumError::Total(v) if v.is_nan()
    ));
    assert!(matches!(
        refused(parts(1.5e308)),
        BuildBandSpectrumError::Total(v) if v.is_infinite() && v > 0.0
    ));
}

#[test]
fn a_spectrum_shape_needs_summable_exponents_and_a_break_wavelength() {
    let shape = SpectrumShape::new(1.8, Metres::new(1_000.0), 2.6).unwrap();
    assert_eq!(shape, SpectrumShape::ICE_RICH);
    assert_same_bits(SpectrumShape::SILICATE.exponent(), 2.0);
    assert_eq!(
        SpectrumShape::SILICATE.break_wavelength(),
        Metres::new(2_000.0)
    );
    assert_same_bits(SpectrumShape::SILICATE.small_scale_exponent(), 3.0);
    let new = |b: f64, w: f64, s: f64| SpectrumShape::new(b, Metres::new(w), s);
    assert_eq!(
        new(1.0, 1e3, 2.6),
        Err(BuildSpectrumShapeError::Exponent(1.0))
    );
    for w in [0.0, -1.0, f64::INFINITY] {
        assert_eq!(
            new(2.0, w, 3.0),
            Err(BuildSpectrumShapeError::BreakWavelength(w))
        );
    }
    assert_eq!(
        new(2.0, 2e3, 0.5),
        Err(BuildSpectrumShapeError::SmallScaleExponent(0.5))
    );
}

// The whole height function (R09.T8).

/// The worlds with bands below their coarse cells: relief, channels or craters.
const BANDED: [SyntheticWorld; 4] = [
    SyntheticWorld::EarthLike,
    SyntheticWorld::MarsLike,
    SyntheticWorld::MoonLike,
    SyntheticWorld::CeresLike,
];

/// The shallowest level that resolves the band limit, and so evaluates every contribution with
/// the band limit's band, as every finer level does.
fn complete_level(synth: &Synthesiser<'_, impl FieldView>) -> u8 {
    (0..=MAX_LEVEL)
        .find(|&n| {
            let set = synth.level_set(BandLevel::new(n).unwrap());
            set.shortest_wavelength().value() <= BAND_LIMIT_M
        })
        .unwrap()
}

/// The levels the whole-function tests sample: 0, the coarse level, four below it and the complete
/// level.
fn sampled_levels(synth: &Synthesiser<'_, impl FieldView>) -> [BandLevel; 4] {
    let coarse = synth.field().header().level().get();
    [0, coarse, coarse + 4, complete_level(synth)].map(|n| BandLevel::new(n).unwrap())
}

/// `a` + `b`, heights and gradients, in that order.
fn plus(a: HeightSample, b: HeightSample) -> HeightSample {
    HeightSample {
        height_m: a.height_m + b.height_m,
        gradient: [0, 1, 2].map(|k| a.gradient[k] + b.gradient[k]),
    }
}

#[test]
fn the_height_is_its_contributions_summed_in_order() {
    for (k, &kind) in SyntheticWorld::ALL.iter().enumerate() {
        let field = world(kind);
        let synth = Synthesiser::new(field, SEED);
        let mut rng = Lcg::new(0x7375_6d00 + u64::try_from(k).unwrap());
        let coarse = field.header().level().get();
        let mut cells = random_cells(coarse, 3, &mut rng);
        cells.extend(rim_cells(field, 2));
        for cell in cells {
            let mut cache = SynthCache::new();
            for dir in random_points_in(cell, 3, &mut rng) {
                for level in sampled_levels(&synth) {
                    let set = *synth.level_set(level);
                    let mut fresh = SynthCache::new();
                    let mut expected = interp::base_elevation(field, dir).unwrap();
                    if let Some(through) = set.relief_through() {
                        let relief = synth.relief().at(field, dir, through, &mut fresh);
                        expected = plus(expected, relief.unwrap());
                    }
                    if let Some(through) = set.channels_through() {
                        let incision = synth
                            .channels()
                            .incision_at(field, dir, through, &mut fresh);
                        expected = plus(expected, incision.unwrap());
                    }
                    if let Some(through) = set.craters_through() {
                        let mut craters = CraterCache::new();
                        expected = plus(
                            expected,
                            synth.craters().at(dir, level, through, &mut craters),
                        );
                    }
                    expected = plus(expected, synth.rims().at(field, dir, level).unwrap());
                    let got = synth.height_at(&mut cache, dir, level).unwrap();
                    assert_eq!(
                        sample_bits(got),
                        sample_bits(expected),
                        "{kind:?} {level:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn no_height_depends_on_the_order_of_queries_or_the_cache() {
    for kind in [SyntheticWorld::EarthLike, SyntheticWorld::MoonLike] {
        let field = world(kind);
        let synth = Synthesiser::new(field, SEED);
        let coarse = field.header().level().get();
        let mut rng = Lcg::new(0x6f72_6465);
        // Points spread over a few cells, so that a warm cache serves some from others' work.
        let points: Vec<[f64; 3]> = random_cells(coarse, 4, &mut rng)
            .into_iter()
            .flat_map(|cell| random_points_in(cell, 4, &mut rng))
            .collect();
        for level in sampled_levels(&synth) {
            let warm = RefCell::new(SynthCache::new());
            let query = |cache: &mut SynthCache, dir: [f64; 3]| {
                sample_bits(synth.height_at(cache, dir, level).unwrap())
            };
            assert_order_independent(&points, |&d| query(&mut warm.borrow_mut(), d));
            assert_order_independent(&points, |&d| query(&mut SynthCache::new(), d));
        }
    }
}

#[test]
fn the_whole_height_function_reads_within_the_synthesis_margin() {
    let margin = SYNTHESIS_MARGIN_CELLS;
    for (k, &kind) in SyntheticWorld::ALL.iter().enumerate() {
        let field = world(kind);
        let coarse = field.header().level();
        let view = Recorder::new(field);
        let synth = Synthesiser::new(&view, SEED);
        let mut rng = Lcg::new(0x7265_6164 + u64::try_from(k).unwrap());
        let mut farthest = 0;
        let mut cells = random_cells(coarse.get(), 6, &mut rng);
        cells.extend(rim_cells(field, 2));
        for cell in cells {
            let mut cache = SynthCache::new();
            // The cell's corners, edges' middles and centre, and two points at random.
            let mut points: Vec<[f64; 3]> = [0.0, 0.5, 1.0]
                .into_iter()
                .flat_map(|x| [0.0, 0.5, 1.0].map(|y| point_in(cell, x, y)))
                .collect();
            points.extend(random_points_in(cell, 2, &mut rng));
            for dir in points {
                let home = PatchKey::containing(coarse.get(), dir).unwrap();
                let distances = king_distances(home, margin);
                let sent = Cover::from_cells([cell_index(home)]).with_margin(coarse, margin);
                for level in sampled_levels(&synth) {
                    view.take();
                    synth.height_at(&mut cache, dir, level).unwrap();
                    for read in view.take() {
                        let d = distances.get(&read).copied().unwrap_or_else(|| {
                            panic!("{kind:?}: {dir:?} in {home:?} at {level:?} read {read:?}")
                        });
                        assert!(sent.contains(cell_index(read)), "{read:?} is not sent");
                        farthest = farthest.max(d);
                    }
                }
            }
        }
        eprintln!("{kind:?}: the farthest read is {farthest} king moves");
        assert!(farthest <= margin);
    }
}

#[test]
fn a_cell_with_its_margin_held_answers_as_the_whole_field() {
    // With the margin about a cell held, and nothing else, the cell's every point is answered as
    // from the whole field at every level; with less, a corner is refused.
    for kind in [SyntheticWorld::MarsLike, SyntheticWorld::CeresLike] {
        let field = world(kind);
        let level = field.header().level().get();
        let whole = Synthesiser::new(field, SEED);
        let last = (1_u32 << level) - 1;
        for cell in [
            PatchKey::new(crate::cube::Face::PosY, level, 0, last).unwrap(),
            PatchKey::new(crate::cube::Face::NegZ, level, last / 2, 3).unwrap(),
        ] {
            let held = Held {
                inner: field,
                held: king_distances(cell, SYNTHESIS_MARGIN_CELLS)
                    .into_keys()
                    .collect(),
            };
            let part = Synthesiser::new(&held, SEED);
            let mut cache = SynthCache::new();
            let mut whole_cache = SynthCache::new();
            for (x, y) in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (0.9, 0.1)] {
                let dir = point_in(cell, x, y);
                if PatchKey::containing(level, dir).unwrap() != cell {
                    continue;
                }
                for band in sampled_levels(&whole) {
                    let from_part = part.height_at(&mut cache, dir, band).unwrap();
                    let from_whole = whole.height_at(&mut whole_cache, dir, band).unwrap();
                    assert_eq!(
                        sample_bits(from_part),
                        sample_bits(from_whole),
                        "{kind:?} {cell:?} at {band:?}"
                    );
                }
            }
            let thin = Held {
                inner: field,
                held: king_distances(cell, 1).into_keys().collect(),
            };
            let thin_synth = Synthesiser::new(&thin, SEED);
            let dir = point_in(cell, 0.02, 0.02);
            let result =
                thin_synth.height_at(&mut SynthCache::new(), dir, BandLevel::new(level).unwrap());
            assert!(
                matches!(result, Err(QueryHeightError::NotSurveyed(c)) if !thin.held.contains(&c)),
                "{kind:?}: {result:?}"
            );
        }
    }
}

/// A patch of `level` inside `cell`, or `cell`'s ancestor at `level`.
fn patch_in(cell: PatchKey, level: u8) -> PatchKey {
    if level >= cell.level() {
        let down = level - cell.level();
        let offset = (1_u32 << down) / 3;
        PatchKey::new(
            cell.face(),
            level,
            (cell.i() << down) + offset,
            (cell.j() << down) + offset,
        )
        .unwrap()
    } else {
        let up = cell.level() - level;
        PatchKey::new(cell.face(), level, cell.i() >> up, cell.j() >> up).unwrap()
    }
}

/// The bits of a bake's samples and parent heights.
fn bake_bits(bake: &PatchHeights) -> Vec<u64> {
    bake.samples()
        .iter()
        .flat_map(|s| sample_bits(*s))
        .chain(bake.parent_heights_m().iter().map(|&h| bits(h)))
        .collect()
}

#[test]
fn a_baked_patch_is_its_vertices_point_queries_bit_for_bit() {
    for (k, &kind) in BANDED.iter().enumerate() {
        let field = world(kind);
        let synth = Synthesiser::new(field, SEED);
        let coarse = field.header().level().get();
        let mut rng = Lcg::new(0x6261_6b65 + u64::try_from(k).unwrap());
        let cell = random_cells(coarse, 4, &mut rng)[3];
        for level in [0, coarse + 1, complete_level(&synth) - 1] {
            let key = patch_in(cell, level);
            let bake = synth.bake_patch(&mut SynthCache::new(), key).unwrap();
            assert_eq!(bake.key(), key);
            assert_eq!(bake.samples().len(), 65 * 65);
            assert_eq!(
                bake.parent_heights_m().len(),
                if level == 0 { 0 } else { 33 * 33 }
            );
            // The point queries, in the reverse order, through another cache.
            let mut other = SynthCache::new();
            let band = BandLevel::new(level).unwrap();
            for y in (0..=64_u8).rev() {
                for x in (0..=64_u8).rev() {
                    let point = synth
                        .height_at(&mut other, key.vertex_dir(x, y), band)
                        .unwrap();
                    assert_eq!(
                        sample_bits(bake.sample(x, y)),
                        sample_bits(point),
                        "{kind:?}"
                    );
                    if level > 0 && x % 2 == 0 && y % 2 == 0 {
                        let parent = BandLevel::new(level - 1).unwrap();
                        let h = synth
                            .height_at(&mut other, key.vertex_dir(x, y), parent)
                            .unwrap();
                        assert_eq!(bits(bake.parent_height_m(x, y).unwrap()), bits(h.height_m));
                    }
                }
            }
            if level == 0 {
                assert_eq!(bake.parent_height_m(0, 0), None);
            }
        }
    }
}

#[test]
fn baking_does_not_depend_on_the_order_of_the_patches() {
    for kind in [SyntheticWorld::EarthLike, SyntheticWorld::MoonLike] {
        let field = world(kind);
        let synth = Synthesiser::new(field, SEED);
        let coarse = field.header().level().get();
        let mut rng = Lcg::new(0x6f72_6465_7273);
        let cell = random_cells(coarse, 5, &mut rng)[4];
        let patch = patch_in(cell, coarse + 3);
        let mut keys = vec![patch, patch.parent().unwrap()];
        keys.extend(patch.children().into_iter().take(2));
        keys.extend([Edge::UMax, Edge::VMin].map(|e| patch.edge_neighbour(e)));
        let warm = RefCell::new(SynthCache::new());
        assert_order_independent(&keys, |&key| {
            bake_bits(&synth.bake_patch(&mut warm.borrow_mut(), key).unwrap())
        });
    }
}

#[test]
fn a_patch_s_heights_lie_in_its_range() {
    for (k, &kind) in SyntheticWorld::ALL.iter().enumerate() {
        let field = world(kind);
        let synth = Synthesiser::new(field, SEED);
        let coarse = field.header().level().get();
        let mut rng = Lcg::new(0x7261_6e67 + u64::try_from(k).unwrap());
        let cell = random_cells(coarse, 3, &mut rng)[2];
        let mut cache = SynthCache::new();
        for level in [1, coarse - 2, coarse + 2, complete_level(&synth)] {
            let key = patch_in(cell, level);
            let (low, high) = synth.height_range_m(key);
            assert!(low < high, "{kind:?} {key:?}");
            let bake = synth.bake_patch(&mut cache, key).unwrap();
            let heights = bake
                .samples()
                .iter()
                .map(|s| s.height_m)
                .chain(bake.parent_heights_m().iter().copied());
            for h in heights {
                assert!(
                    low <= h && h <= high,
                    "{kind:?} {key:?}: {h} m outside {low}..{high} m"
                );
            }
        }
    }
    // A patch whose cells the view does not hold reaches any elevation a cell encodes.
    let field = world(SyntheticWorld::CeresLike);
    let empty = Held {
        inner: field,
        held: BTreeSet::new(),
    };
    let (low, high) = Synthesiser::new(&empty, SEED)
        .height_range_m(PatchKey::new(crate::cube::Face::PosX, 9, 3, 4).unwrap());
    assert!(low < f64::from(i32::MIN) / 1e3 && high > f64::from(i32::MAX) / 1e3);
}

#[test]
fn unresolved_rms_falls_with_the_resolution_to_zero_at_the_band_limit() {
    for (k, &kind) in SyntheticWorld::ALL.iter().enumerate() {
        let field = world(kind);
        let synth = Synthesiser::new(field, SEED);
        let coarse = field.header().level().get();
        let mut rng = Lcg::new(0x726d_7300 + u64::try_from(k).unwrap());
        let mut cells = random_cells(coarse, 12, &mut rng);
        cells.extend(rim_cells(field, 4));
        for cell in cells {
            let mut previous = f64::INFINITY;
            let mut resolution = 4.0 * field.header().boundary_diameter().value();
            while resolution > 0.25 {
                let rms = synth
                    .unresolved_rms(cell, Metres::new(resolution))
                    .unwrap()
                    .value();
                assert!(rms.is_finite() && rms >= 0.0);
                assert!(
                    rms <= previous,
                    "{kind:?} {cell:?}: {rms} m at {resolution} m"
                );
                if resolution <= BAND_LIMIT_M {
                    assert_same_bits(rms, 0.0);
                }
                previous = rms;
                resolution *= 0.8;
            }
            assert_eq!(
                unresolved_rms(field, cell, Metres::new(BAND_LIMIT_M)),
                Some(Metres::ZERO)
            );
        }
    }
}

#[test]
fn the_rims_unresolved_share_is_their_sharpening_s_mean_square_over_the_reach() {
    // On the one-crater world the rims are the whole synthesis: what a level leaves out of the
    // band limit's, integrated over every cell the crater reaches on a grid of each, is the
    // closed form's crater-wide mean square times the reach's disc; and its structure function,
    // the mean square difference at a lag in a random direction, so integrated, is the closed
    // form's at that lag times the disc, as a form between its exact small- and long-lag limits.
    let field = world(SyntheticWorld::OneCrater);
    let synth = Synthesiser::new(field, SEED);
    let header = field.header();
    let coarse = header.level().get();
    let crater = &field.craters()[0];
    let reach = crater_shape_of(field, crater).reach().value();
    let reached: Vec<PatchKey> = (0..header.level().cell_count())
        .map(|n| crate::field::cell_at_index(coarse, n).unwrap())
        .filter(|&c| field.craters_reaching(c).next().is_some())
        .collect();
    let complete = BandLevel::new(complete_level(&synth)).unwrap();
    let lags = [1e3, 1e4, 5e4];
    let mut rng = Lcg::new(0x7269_6d73);
    for n in [coarse - 4, coarse - 2] {
        let level = BandLevel::new(n).unwrap();
        let lambda = synth.level_set(level).shortest_wavelength();
        let closed = synth
            .unresolved_variance(reached[0], lambda)
            .unwrap()
            .rims()
            .value();
        let left_out = |dir: [f64; 3]| {
            synth.rims().at(field, dir, complete).unwrap().height_m
                - synth.rims().at(field, dir, level).unwrap().height_m
        };
        let grid = 24_u32;
        let mut integral = 0.0;
        let mut structure = [0.0; 3];
        for &cell in &reached {
            let area = craters::cell_area(cell, &header.figure()).value();
            let mut sum = 0.0;
            let mut differences = [0.0; 3];
            for a in 0..grid {
                for b in 0..grid {
                    let x = (f64::from(a) + 0.5) / f64::from(grid);
                    let y = (f64::from(b) + 0.5) / f64::from(grid);
                    let dir = point_in(cell, x, y);
                    let here = left_out(dir);
                    sum += here * here;
                    let angle = 2.0 * core::f64::consts::PI * rng.next_f64();
                    let (e, north) = (east(dir), cross(dir, east(dir)));
                    let along =
                        [0, 1, 2].map(|k| math::cos(angle) * e[k] + math::sin(angle) * north[k]);
                    for (slot, &lag) in differences.iter_mut().zip(&lags) {
                        let (moved, _) = step(field, dir, along, lag);
                        let d = left_out(moved) - here;
                        *slot += d * d;
                    }
                }
            }
            let samples = f64::from(grid * grid);
            integral += area * sum / samples;
            for (total, d) in structure.iter_mut().zip(differences) {
                *total += area * d / samples;
            }
        }
        let disc = core::f64::consts::PI * reach * reach;
        let ratio = integral / (closed * disc);
        eprintln!("level {n}: the rims' share sampled ÷ closed {ratio:.4}");
        assert!((ratio - 1.0).abs() < 0.1, "level {n}: {ratio}");
        for (&lag, sampled) in lags.iter().zip(structure) {
            let lag = Metres::new(lag);
            let form = synth
                .structure_function(reached[0], lambda, lag)
                .unwrap()
                .value();
            let ratio = sampled / (form * disc);
            eprintln!(
                "level {n} at {} m: the rims' structure function sampled ÷ closed {ratio:.3}",
                lag.value()
            );
            assert!((ratio - 1.0).abs() < 0.25, "level {n} at {lag:?}: {ratio}");
        }
        // Every cell the crater reaches has the same share, and one it does not, none.
        assert_eq!(
            synth
                .unresolved_variance(reached[reached.len() / 2], lambda)
                .unwrap()
                .rims(),
            SquareMetres::new(closed)
        );
    }
    let far = PatchKey::new(crate::cube::Face::NegX, coarse, 5, 5).unwrap();
    assert!(field.craters_reaching(far).next().is_none());
    let none = synth
        .unresolved_variance(far, Metres::new(1e5))
        .unwrap()
        .rims();
    assert_same_bits(none.value(), 0.0);
}

/// `crater`'s fresh shape on `field`'s body.
fn crater_shape_of(
    field: &CoarseField,
    crater: &crate::field::CoarseCrater,
) -> crate::craters::CraterShape {
    crate::craters::crater_shape(field.header().craters(), crater.diameter)
}

#[test]
fn a_synthesiser_s_closed_forms_and_bounds_are_the_free_functions() {
    let field = world(SyntheticWorld::MarsLike);
    let header = field.header();
    let synth = Synthesiser::new(field, SEED);
    // The bounds from the crater octaves it holds are the free function's, to the bit.
    assert_eq!(*synth.bounds(), LevelBounds::new(header));
    for n in 0..=MAX_LEVEL {
        let level = BandLevel::new(n).unwrap();
        assert_same_bits(synth.level_bound_m(level), level_bound_m(header, level));
    }
    let mut rng = Lcg::new(0x6672_6565);
    let mut cells = random_cells(header.level().get(), 4, &mut rng);
    cells.extend(rim_cells(field, 2));
    for cell in cells {
        for w in [30.0, 3_000.0, 300_000.0] {
            let w = Metres::new(w);
            let (held, free) = (
                synth.unresolved_variance(cell, w).unwrap(),
                unresolved_variance(field, cell, w).unwrap(),
            );
            for (a, b) in [
                (held.relief(), free.relief()),
                (held.channels(), free.channels()),
                (held.craters(), free.craters()),
                (held.rims(), free.rims()),
            ] {
                assert_same_bits(a.value(), b.value());
            }
            assert_same_bits(
                synth.unresolved_rms(cell, w).unwrap().value(),
                unresolved_rms(field, cell, w).unwrap().value(),
            );
            for lag in [0.0, 10.0, 1_000.0] {
                let lag = Metres::new(lag);
                assert_same_bits(
                    synth.structure_function(cell, w, lag).unwrap().value(),
                    structure_function(field, cell, w, lag).unwrap().value(),
                );
            }
        }
    }
    let empty = Held {
        inner: field,
        held: BTreeSet::new(),
    };
    let cell = PatchKey::new(crate::cube::Face::PosX, field.header().level().get(), 0, 0).unwrap();
    assert_eq!(
        structure_function(&empty, cell, Metres::new(1e3), Metres::new(10.0)),
        None
    );
    assert_eq!(unresolved_rms(&empty, cell, Metres::new(1e3)), None);
}

/// A tangent at `dir`: its local east, or the x axis at a pole.
fn east(dir: [f64; 3]) -> [f64; 3] {
    let e = [-dir[1], dir[0], 0.0];
    let n = (e[0] * e[0] + e[1] * e[1]).sqrt();
    if n < 1e-9 {
        [1.0, 0.0, 0.0]
    } else {
        e.map(|c| c / n)
    }
}

/// `a` × `b`.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The direction `lag` metres from `dir` along the unit tangent `along` on a body of radius
/// `radius_m`, and the chord between their spheroid points.
fn step(field: &CoarseField, dir: [f64; 3], along: [f64; 3], lag: f64) -> ([f64; 3], f64) {
    let radius_m = field.header().radius().value();
    let moved = [0, 1, 2].map(|k| dir[k] + lag / radius_m * along[k]);
    let n = (moved[0] * moved[0] + moved[1] * moved[1] + moved[2] * moved[2]).sqrt();
    let moved = moved.map(|c| c / n);
    let figure = field.header().figure();
    let (p, q) = (figure.point(dir), figure.point(moved));
    let chord = [0, 1, 2].map(|k| q[k] - p[k]);
    (
        moved,
        (chord[0] * chord[0] + chord[1] * chord[1] + chord[2] * chord[2]).sqrt(),
    )
}

/// The coarse cells of `field` whose own style and every cell's within two king moves have the
/// amplitude and belt weight `style` picks, at least four cells from a face edge (where the ghosts
/// read a cell further), up to `count`, spread through cell-index order.
fn uniform_cells(
    field: &CoarseField,
    count: usize,
    pick: impl Fn(relief::ReliefStyle) -> bool,
) -> Vec<PatchKey> {
    let level = field.header().level();
    let last = (1_u32 << level.get()) - 1;
    let inside = |c: PatchKey| c.i() >= 4 && c.j() >= 4 && c.i() + 4 <= last && c.j() + 4 <= last;
    let style = |c: PatchKey| relief::ReliefStyle::of(field.cell(c).unwrap());
    let all: Vec<PatchKey> = (0..level.cell_count())
        .map(|n| crate::field::cell_at_index(level.get(), n).unwrap())
        .filter(|&c| inside(c) && pick(style(c)))
        .filter(|&c| {
            let own = style(c);
            king_distances(c, 2).into_keys().all(|n| style(n) == own)
        })
        .collect();
    let stride = (all.len() / count).max(1);
    all.into_iter().step_by(stride).take(count).collect()
}

/// The sampled structure function of the structural octaves and of the whole synthesis (the height
/// less its base elevation) at the complete level, at each of `lags` metres, pooled over `points`
/// point pairs in each of `cells`, beside each one's closed form at the same cells, all in
/// wavelengths shorter than any coarse cell: (sampled relief, closed relief, sampled whole, closed
/// whole) per lag.
fn structure_functions(
    synth: &Synthesiser<'_, CoarseField>,
    cells: &[PatchKey],
    lags: &[f64],
    points: usize,
    rng: &mut Lcg,
) -> Vec<[f64; 4]> {
    let field = synth.field();
    let everything = Metres::new(1e12);
    let level = BandLevel::new(complete_level(synth)).unwrap();
    let finest = synth.relief().finest_octave();
    let mut out = vec![[0.0; 4]; lags.len()];
    for &cell in cells {
        let mut cache = SynthCache::new();
        for dir in random_points_in(cell, points, rng) {
            let angle = 2.0 * core::f64::consts::PI * rng.next_f64();
            let (e, n) = (east(dir), cross(dir, east(dir)));
            let along = [0, 1, 2].map(|k| math::cos(angle) * e[k] + math::sin(angle) * n[k]);
            let relief_at = |cache: &mut SynthCache, d: [f64; 3]| {
                synth.relief().at(field, d, finest, cache).unwrap().height_m
            };
            let whole_at = |cache: &mut SynthCache, d: [f64; 3]| {
                synth.height_at(cache, d, level).unwrap().height_m
                    - interp::base_elevation(field, d).unwrap().height_m
            };
            let (r0, w0) = (relief_at(&mut cache, dir), whole_at(&mut cache, dir));
            for (slot, &lag) in out.iter_mut().zip(lags) {
                let (moved, _) = step(field, dir, along, lag);
                let r = relief_at(&mut cache, moved) - r0;
                let w = whole_at(&mut cache, moved) - w0;
                slot[0] += r * r;
                slot[2] += w * w;
            }
        }
        for (slot, &lag) in out.iter_mut().zip(lags) {
            let lag = Metres::new(lag);
            #[expect(clippy::cast_precision_loss, reason = "a few hundred points")]
            let n = points as f64;
            slot[1] += n * relief::relief_structure_function(field, cell, everything, lag)
                .unwrap()
                .value();
            slot[3] += n * synth
                .structure_function(cell, everything, lag)
                .unwrap()
                .value();
        }
    }
    out
}

#[test]
fn the_closed_form_structure_functions_match_the_relief_on_both_styles_and_the_whole_on_belts() {
    // The plan's 10%: on the structural octaves' share, belt and plain cells alike, as
    // `decision-r09-t5.md` item 7.3 builds it, and on the whole function on belt cells. On
    // channelled plain land the whole is 15–37% above its closed form, from the channels' ×2 form
    // (T6.b; deferred, main's acceptance of 2026-10-13), so it is printed, not asserted there.
    let field = world(SyntheticWorld::EarthLike);
    let synth = Synthesiser::new(field, SEED);
    let lags = [10.0, 100.0, 1_000.0];
    let belts = uniform_cells(field, 10, |s| s.belt() >= 1.0);
    let plains = uniform_cells(field, 10, |s| s.belt() <= 0.0 && s.amplitude() >= 0.1);
    assert!(
        belts.len() >= 4 && plains.len() >= 4,
        "{} belt and {} plain cells",
        belts.len(),
        plains.len()
    );
    let mut rng = Lcg::new(0x7366_756e);
    for (name, cells) in [("belt", &belts), ("plain", &plains)] {
        let sums = structure_functions(&synth, cells, &lags, 300, &mut rng);
        for (lag, [relief, relief_closed, whole, whole_closed]) in lags.iter().zip(sums) {
            let (relief_ratio, whole_ratio) = (relief / relief_closed, whole / whole_closed);
            eprintln!(
                "{name} cells at {lag} m: relief sampled ÷ closed {relief_ratio:.3}, whole {whole_ratio:.3}"
            );
            assert!(
                (relief_ratio - 1.0).abs() <= 0.1,
                "{name} relief at {lag} m: {relief_ratio}"
            );
            if name == "belt" {
                assert!(
                    (whole_ratio - 1.0).abs() <= 0.1,
                    "belt whole at {lag} m: {whole_ratio}"
                );
            }
        }
    }
}

#[test]
#[ignore = "a diagnosis of the structure function by contribution, run by hand"]
fn measure_the_structure_function_by_contribution() {
    let field = world(SyntheticWorld::EarthLike);
    let synth = Synthesiser::new(field, SEED);
    let lags = [10.0, 100.0, 1_000.0];
    let belts = uniform_cells(field, 10, |s| s.belt() >= 1.0);
    let plains = uniform_cells(field, 10, |s| s.belt() <= 0.0 && s.amplitude() >= 0.1);
    let everything = Metres::new(1e12);
    let level = BandLevel::new(complete_level(&synth)).unwrap();
    let set = *synth.level_set(level);
    let mut rng = Lcg::new(0x7366_756e);
    for (name, cells) in [("belt", &belts), ("plain", &plains)] {
        for &cell in cells {
            let record = field.cell(cell).unwrap();
            let style = relief::ReliefStyle::of(record);
            let mut cache = SynthCache::new();
            let mut craters = CraterCache::new();
            let mut sums = vec![[0.0_f64; 2]; lags.len()];
            let points = 200_u32;
            for dir in random_points_in(cell, usize::try_from(points).unwrap(), &mut rng) {
                let angle = 2.0 * core::f64::consts::PI * rng.next_f64();
                let (e, n) = (east(dir), cross(dir, east(dir)));
                let along = [0, 1, 2].map(|k| math::cos(angle) * e[k] + math::sin(angle) * n[k]);
                let mut parts = |d: [f64; 3]| {
                    let c = set.channels_through().map_or(0.0, |t| {
                        synth
                            .channels()
                            .incision_at(field, d, t, &mut cache)
                            .unwrap()
                            .height_m
                    });
                    let k = set.craters_through().map_or(0.0, |t| {
                        synth.craters().at(d, level, t, &mut craters).height_m
                    });
                    [c, k]
                };
                let at = parts(dir);
                for (slot, &lag) in sums.iter_mut().zip(&lags) {
                    let (moved, _) = step(field, dir, along, lag);
                    let there = parts(moved);
                    for p in 0..2 {
                        let d = there[p] - at[p];
                        slot[p] += d * d;
                    }
                }
            }
            for (slot, &lag) in sums.iter().zip(&lags) {
                let lag_m = Metres::new(lag);
                let channels =
                    channels::channel_structure_function(field, cell, everything, lag_m).unwrap();
                let craters = synth
                    .craters()
                    .octaves()
                    .structure_function(everything, lag_m);
                let relief =
                    relief::relief_structure_function(field, cell, everything, lag_m).unwrap();
                let n = f64::from(points);
                eprintln!(
                    "{name} {cell:?} a {:.2} land {} at {lag} m: relief closed {:.3e}; channels \
                     sampled {:.3e} closed {:.3e}; craters sampled {:.3e} closed {:.3e}",
                    style.amplitude(),
                    record.elevation_mm > 0,
                    relief.value(),
                    slot[0] / n,
                    channels.value(),
                    slot[1] / n,
                    craters.value()
                );
            }
        }
    }
}

/// The adirectional slopes, degrees, from forward differences over `baseline` metres along the
/// local east and north at `count` points at random in `cells`, at the complete level.
fn slopes(
    synth: &Synthesiser<'_, CoarseField>,
    cells: &[PatchKey],
    baseline: f64,
    count: usize,
    rng: &mut Lcg,
) -> Vec<f64> {
    let field = synth.field();
    let level = BandLevel::new(complete_level(synth)).unwrap();
    let per_cell = count.div_ceil(cells.len());
    let mut out = Vec::with_capacity(count);
    for &cell in cells {
        let mut cache = SynthCache::new();
        for dir in random_points_in(cell, per_cell, rng) {
            let h = |cache: &mut SynthCache, d| synth.height_at(cache, d, level).unwrap().height_m;
            let h0 = h(&mut cache, dir);
            let e = east(dir);
            let n = cross(dir, e);
            let (to_e, run_e) = step(field, dir, e, baseline);
            let (to_n, run_n) = step(field, dir, n, baseline);
            let ge = (h(&mut cache, to_e) - h0) / run_e;
            let gn = (h(&mut cache, to_n) - h0) / run_n;
            out.push(math::atan((ge * ge + gn * gn).sqrt()).to_degrees());
        }
    }
    out
}

#[test]
#[ignore = "a measurement of the Moon-like world's slopes for the plan's record, run by hand"]
fn measure_the_moon_like_world_s_slopes() {
    // T7.b handed T8 the craters' calibration against the Moon's highland and mare slopes
    // (Rosenburg et al. 2011's medians at 17 m, 7.5° and 2.0°) and LOLA's RMS height differences
    // at 1.9 and 7.6 km (171 and 546 m): measured here by crust, and recorded (deferred).
    let field = world(SyntheticWorld::MoonLike);
    let synth = Synthesiser::new(field, SEED);
    let level = BandLevel::new(complete_level(&synth)).unwrap();
    let mut rng = Lcg::new(0x6d6f_6f6e);
    let mut by_crust: std::collections::BTreeMap<String, Vec<[f64; 4]>> =
        std::collections::BTreeMap::new();
    for cell in random_cells(field.header().level().get(), 120, &mut rng) {
        let crust = format!("{:?}", field.cell(cell).unwrap().crust);
        let mut cache = SynthCache::new();
        for dir in random_points_in(cell, 10, &mut rng) {
            let h = |cache: &mut SynthCache, d| synth.height_at(cache, d, level).unwrap().height_m;
            let h0 = h(&mut cache, dir);
            let (e, n) = (east(dir), cross(dir, east(dir)));
            let (to_e, run_e) = step(field, dir, e, 17.0);
            let (to_n, run_n) = step(field, dir, n, 17.0);
            let (ge, gn) = (
                (h(&mut cache, to_e) - h0) / run_e,
                (h(&mut cache, to_n) - h0) / run_n,
            );
            let slope = math::atan((ge * ge + gn * gn).sqrt()).to_degrees();
            let angle = 2.0 * core::f64::consts::PI * rng.next_f64();
            let along = [0, 1, 2].map(|k| math::cos(angle) * e[k] + math::sin(angle) * n[k]);
            let mut gaps = [0.0; 3];
            for (gap, lag) in gaps.iter_mut().zip([100.0, 1_900.0, 7_600.0]) {
                *gap = h(&mut cache, step(field, dir, along, lag).0) - h0;
            }
            by_crust
                .entry(crust.clone())
                .or_default()
                .push([slope, gaps[0], gaps[1], gaps[2]]);
        }
    }
    for (crust, samples) in &by_crust {
        let mut slopes: Vec<f64> = samples.iter().map(|s| s[0]).collect();
        slopes.sort_by(f64::total_cmp);
        #[expect(clippy::cast_precision_loss, reason = "a few hundred samples")]
        let n = samples.len() as f64;
        let rms = |k: usize| (samples.iter().map(|s| s[k] * s[k]).sum::<f64>() / n).sqrt();
        eprintln!(
            "{crust} ({} points): median slope {:.2}° at 17 m; RMS height difference {:.1} m at \
             100 m, {:.1} m at 1.9 km, {:.1} m at 7.6 km",
            samples.len(),
            slopes[slopes.len() / 2],
            rms(1),
            rms(2),
            rms(3)
        );
    }
}

#[test]
fn full_belt_cells_slope_as_the_belt_cores() {
    // `decision-r09-t5.md` item 7: SRTM's belt cores, and the ruling's replica of the relief.
    let field = world(SyntheticWorld::EarthLike);
    let synth = Synthesiser::new(field, SEED);
    let belts = uniform_cells(field, 12, |s| s.belt() >= 1.0);
    assert!(belts.len() >= 4, "{} full-belt cells", belts.len());
    let mut rng = Lcg::new(0x736c_6f70);
    let rms = |s: &[f64]| {
        #[expect(clippy::cast_precision_loss, reason = "a few thousand slopes")]
        let n = s.len() as f64;
        math::atan(
            (s.iter()
                .map(|d| {
                    let t = math::tan(d.to_radians());
                    t * t
                })
                .sum::<f64>()
                / n)
                .sqrt(),
        )
        .to_degrees()
    };
    let mean = |s: &[f64]| {
        #[expect(clippy::cast_precision_loss, reason = "a few thousand slopes")]
        let n = s.len() as f64;
        s.iter().sum::<f64>() / n
    };
    let ten_m = slopes(&synth, &belts, 10.0, 2_000, &mut rng);
    let ten_m0 = slopes(&synth, &belts, 100.0, 2_000, &mut rng);
    let kilometre = slopes(&synth, &belts, 1_000.0, 2_000, &mut rng);
    #[expect(clippy::cast_precision_loss, reason = "a few thousand slopes")]
    let steep = ten_m.iter().filter(|&&d| d > 40.0).count() as f64 / ten_m.len() as f64;
    eprintln!(
        "full belts ({} cells): RMS {:.1}° at 10 m, {:.1}° at 100 m, {:.1}° at 1 km; mean {:.1}° at \
         100 m; {:.3} above 40° at 10 m",
        belts.len(),
        rms(&ten_m),
        rms(&ten_m0),
        rms(&kilometre),
        mean(&ten_m0),
        steep
    );
    let in_range = |x: f64, lo: f64, hi: f64| (lo..=hi).contains(&x);
    assert!(
        in_range(rms(&ten_m0), 32.0, 40.0),
        "RMS at 100 m {}",
        rms(&ten_m0)
    );
    assert!(
        in_range(rms(&kilometre), 22.0, 30.0),
        "RMS at 1 km {}",
        rms(&kilometre)
    );
    assert!(
        in_range(mean(&ten_m0), 26.0, 34.0),
        "mean at 100 m {}",
        mean(&ten_m0)
    );
    assert!(
        in_range(steep, 0.15, 0.35),
        "share above 40° at 10 m {steep}"
    );
}
