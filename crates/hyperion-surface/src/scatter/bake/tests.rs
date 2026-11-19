//! The rock hook's tests: a finest patch's list is `rocks_near` about it, whatever the cache, and
//! leaves out only what the view lacks.

use std::collections::BTreeSet;

use hyperion_base::units::Metres;
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::bake::{
    HorizonLayer, NormalsLayer, TerrainBake, TerrainBakeOptions, WeightsLayer, bake_terrain,
};
use crate::cube::unit_dir;
use crate::field::{CoarseField, Cover, CoverRange, PartialField, ResolutionCode, cell_index};
use crate::patch::{BakeOptions, NormalScale, VertexPath};
use crate::scatter::{RockCache, rocks_near};
use crate::synth::craters::neighbourhood;
use crate::synth::test_support::{SEED, world};
use crate::testing::SyntheticWorld;
use crate::wire::{decode_payload, encode_payload};

/// The wireframe's bake: no weights, horizon or normals, the cheapest that runs the hooks.
const BARE: TerrainBakeOptions = TerrainBakeOptions {
    patch: BakeOptions {
        vertex_path: VertexPath::FaceDifferences,
        normals: NormalScale::Mesh,
        skirt_m: 0.0,
    },
    weights: WeightsLayer::Skipped,
    horizon: HorizonLayer::Skipped,
    normals: NormalsLayer::Skipped,
};

/// The directions the tests bake a finest patch at: one on each of four faces, and one by a cube
/// edge.
const DIRS: [[f64; 3]; 5] = [
    [0.3, 1.0, 0.2],
    [1.0, 0.1, 0.2],
    [0.2, -0.4, 0.8],
    [-0.8, -0.3, 0.4],
    [1.0, 0.999_99, 0.3],
];

/// A client's field of `field` surveying `cells` at 50 m, with their margin.
fn client_of(field: &CoarseField, cells: &BTreeSet<PatchKey>) -> PartialField {
    let code = ResolutionCode::from_resolution(Metres::new(50.0)).unwrap();
    let mut indices: Vec<u32> = cells.iter().map(|&c| cell_index(c)).collect();
    indices.sort_unstable();
    let cover = Cover::from_ranges(
        indices
            .iter()
            .map(|&i| CoverRange::new(i, i + 1, code).unwrap()),
    )
    .unwrap();
    let blocks = decode_payload(&encode_payload(field, &cover, None)).unwrap();
    let mut client = PartialField::new(blocks[0].header().unwrap().clone());
    for block in &blocks {
        client.insert(block).unwrap();
    }
    client
}

/// The coarse cells about `dir`: its own and its 3 × 3 neighbourhood's.
fn cells_about(field: &CoarseField, dir: [f64; 3]) -> BTreeSet<PatchKey> {
    let level = field.header().level().get();
    let home = PatchKey::containing(level, dir).unwrap();
    let (around, count) = neighbourhood(home);
    around[..count].iter().copied().collect()
}

/// The finest patch holding `dir` on `field`'s body.
fn finest_at(field: &CoarseField, dir: [f64; 3]) -> PatchKey {
    PatchKey::containing(
        finest_level(field.header().figure().equatorial_radius_m),
        unit_dir(dir),
    )
    .unwrap()
}

/// `key`'s bake over `view` with a rock hook on `cache`, the cache handed back.
fn bake_with_rocks(
    view: &PartialField,
    key: PatchKey,
    cache: SynthCache,
) -> (TerrainBake, SynthCache) {
    let synth = Synthesiser::new(view, SEED);
    let hook = RockBakeHook::new(&synth, cache);
    let bake = bake_terrain(&synth, &mut SynthCache::new(), key, &BARE, &[&hook]).unwrap();
    (bake, hook.into_cache())
}

/// The rock layer of `bake`, which must hold exactly one.
fn rocks_of_bake(bake: &TerrainBake) -> &[Rock] {
    match bake.layers().extra() {
        [ExtraLayer::Rocks(rocks)] => rocks,
        other => panic!("a finest patch has one rock layer, not {other:?}"),
    }
}

/// `rocks_near` about `key`: from its centre, out past its farthest corner.
fn rocks_about(view: &PartialField, key: PatchKey) -> Vec<Rock> {
    let synth = Synthesiser::new(view, SEED);
    let figure = view.header().figure();
    let centre = figure.point(key.vertex_dir(32, 32));
    let radius = [(0, 0), (64, 0), (0, 64), (64, 64)]
        .map(|(x, y)| {
            let c = figure.point(key.vertex_dir(x, y));
            (0..3)
                .map(|k| (c[k] - centre[k]) * (c[k] - centre[k]))
                .sum::<f64>()
                .sqrt()
        })
        .into_iter()
        .fold(0.0, f64::max);
    let mut near = Vec::new();
    rocks_near(
        &synth,
        &mut SynthCache::new(),
        &mut RockCache::new(),
        &centre,
        Metres::new(1.01 * radius),
        &mut near,
    )
    .unwrap();
    near
}

/// A finest patch's list is `rocks_near` about it with the rocks whose axis lies elsewhere taken
/// out, rock for rock and in its order, bit for bit, on an airless world and one with air.
#[test]
fn a_finest_patchs_rocks_are_the_rocks_near_it_whose_axis_it_holds() {
    let mut rocks = 0;
    for w in [SyntheticWorld::MoonLike, SyntheticWorld::EarthLike] {
        let field = world(w);
        for dir in DIRS {
            let view = client_of(field, &cells_about(field, unit_dir(dir)));
            let key = finest_at(field, dir);
            let (bake, _) = bake_with_rocks(&view, key, SynthCache::new());
            assert!(bake.layers().is_complete(), "{w:?} {key:?}");
            let got = rocks_of_bake(&bake);
            let want: Vec<Rock> = rocks_about(&view, key)
                .into_iter()
                .filter(|rock| holds(key, rock))
                .collect();
            assert_eq!(got, want, "{w:?} {key:?}");
            assert_eq!(format!("{got:?}"), format!("{want:?}"));
            rocks += got.len();
        }
    }
    assert!(rocks > 50, "only {rocks} rocks were compared");
}

/// The hook's cache, empty, warmed by other patches or by this one, changes no rock.
#[test]
fn a_patchs_rocks_do_not_depend_on_the_hooks_cache() {
    let w = SyntheticWorld::MoonLike;
    let field = world(w);
    let view = client_of(field, &cells_about(field, unit_dir(DIRS[0])));
    let key = finest_at(field, DIRS[0]);
    let neighbours = [(1, 0), (0, 1), (1, 1)].map(|(di, dj)| {
        PatchKey::new(key.face(), key.level(), key.i() + di, key.j() + dj).unwrap()
    });
    let mut keys = vec![key];
    keys.extend(neighbours);
    let warm = std::cell::RefCell::new(SynthCache::new());
    assert_order_independent(&keys, |&k| {
        let (bake, kept) = bake_with_rocks(&view, k, warm.take());
        warm.replace(kept);
        format!("{:?}", rocks_of_bake(&bake))
    });
    for &k in &keys {
        let (cold, _) = bake_with_rocks(&view, k, SynthCache::new());
        let (bake, kept) = bake_with_rocks(&view, k, warm.take());
        warm.replace(kept);
        assert_eq!(rocks_of_bake(&cold), rocks_of_bake(&bake));
    }
}

/// A patch at any level but the finest gains no layer, and its bake is the hookless one.
#[test]
fn only_a_finest_patch_carries_rocks() {
    let w = SyntheticWorld::MoonLike;
    let field = world(w);
    let view = client_of(field, &cells_about(field, unit_dir(DIRS[0])));
    let finest = finest_at(field, DIRS[0]);
    let synth = Synthesiser::new(&view, SEED);
    for up in [1, 2, 6] {
        let key = PatchKey::new(
            finest.face(),
            finest.level() - up,
            finest.i() >> up,
            finest.j() >> up,
        )
        .unwrap();
        let (hooked, _) = bake_with_rocks(&view, key, SynthCache::new());
        assert_eq!(hooked.layers().extra(), []);
        let hookless = bake_terrain(&synth, &mut SynthCache::new(), key, &BARE, &[]).unwrap();
        assert_eq!(hooked, hookless);
    }
}

/// About the survey's edge, a patch's list holds only rocks the whole field's list holds, bit for
/// bit, and the bake is not complete where it leaves some out.
#[test]
fn at_the_survey_edge_the_list_leaves_out_only_what_the_view_lacks() {
    let w = SyntheticWorld::MoonLike;
    let field = world(w);
    let level = field.header().level().get();
    let dir = unit_dir(DIRS[0]);
    let home = PatchKey::containing(level, dir).unwrap();
    // One coarse cell surveyed: its margin is held, the ground beyond it not.
    let view = client_of(field, &BTreeSet::from([home]));
    let whole = client_of(field, &cells_about(field, dir));
    let finest = finest_level(field.header().figure().equatorial_radius_m);
    let down = finest - level;
    let span = 1_u32 << down;
    let (mut equal, mut short) = (0, 0);
    // Finest patches from the cell's middle out across its margin and past it, a sixteenth of a
    // cell apart.
    for step in 0..48 {
        let key = PatchKey::new(
            home.face(),
            finest,
            (home.i() << down) + span / 2 + step * (span / 16),
            (home.j() << down) + span / 2,
        )
        .unwrap();
        let (bake, _) = bake_with_rocks(&view, key, SynthCache::new());
        let (all, _) = bake_with_rocks(&whole, key, SynthCache::new());
        let (got, want) = (rocks_of_bake(&bake), rocks_of_bake(&all));
        for rock in got {
            assert!(want.contains(rock), "{key:?}: {rock:?} is not the field's");
        }
        if got.len() < want.len() {
            short += 1;
            assert!(!bake.layers().is_complete(), "{key:?} leaves rocks out");
        } else if !want.is_empty() {
            equal += 1;
        }
    }
    assert!(equal > 0 && short > 0, "{equal} whole, {short} short");
}

/// A rock's words are its base centre, its frame, its semi-axes, its exponent, its ground and its
/// axis's direction, in that order, and its frame is a right-handed orthonormal set whose c axis is the datum's normal.
#[test]
fn a_rocks_words_are_its_place_frame_and_shape() {
    let w = SyntheticWorld::MoonLike;
    let field = world(w);
    let figure = field.header().figure();
    let view = client_of(field, &cells_about(field, unit_dir(DIRS[0])));
    let (bake, _) = bake_with_rocks(&view, finest_at(field, DIRS[0]), SynthCache::new());
    let rocks = rocks_of_bake(&bake);
    assert_ne!(rocks, []);
    for rock in rocks {
        let words = rock_words(rock);
        assert_eq!(words[..3], rock.base_centre());
        let [a, b, c] = rock.frame();
        assert_eq!(words[3..12], [a, b, c].concat()[..]);
        assert_eq!(words[12..15], rock.semi_axes().map(Metres::value));
        assert_eq!(words[15], rock.shape().exponent());
        assert_eq!(words[16], rock.ground_height().value());
        assert_eq!(words[17..], rock.dir());
        let dot = |u: [f64; 3], v: [f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
        for (u, v) in [(a, b), (b, c), (c, a)] {
            assert!(dot(u, v).abs() < 1e-12);
        }
        for u in [a, b, c] {
            assert!((dot(u, u) - 1.0).abs() < 1e-12);
        }
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        assert!(dot(cross, c) > 1.0 - 1e-12, "the frame is right-handed");
        assert_eq!(c, figure.normal(rock.dir()));
    }
}

/// A hook that draws the patch's rocks at a stated k ([`patch_rocks_at`]) and keeps the answer.
struct StatedDraw<'s, 'a, F> {
    synth: &'s Synthesiser<'a, F>,
    answer: std::cell::RefCell<Option<Result<Vec<Rock>, crate::scatter::FindRocksError>>>,
}

impl<F: FieldView> BakeHook for StatedDraw<'_, '_, F> {
    fn bake(&self, patch: &BakedHeights<'_>, _out: &mut Vec<ExtraLayer>) -> bool {
        let k = RockAbundance::new(0.18);
        self.answer.replace(Some(patch_rocks_at(
            self.synth,
            &mut SynthCache::new(),
            patch,
            k,
        )));
        true
    }
}

/// Past the survey's edge the stated draw names the first cell the view lacks, never the patch.
#[test]
fn the_stated_draw_names_the_first_cell_the_view_lacks() {
    let w = SyntheticWorld::MoonLike;
    let field = world(w);
    let level = field.header().level().get();
    let home = PatchKey::containing(level, unit_dir(DIRS[0])).unwrap();
    let view = client_of(field, &BTreeSet::from([home]));
    let synth = Synthesiser::new(&view, SEED);
    let finest = finest_level(field.header().figure().equatorial_radius_m);
    let down = finest - level;
    let span = 1_u32 << down;
    let (mut refused, mut drawn) = (0, 0);
    for step in [0, 24, 47] {
        let key = PatchKey::new(
            home.face(),
            finest,
            (home.i() << down) + span / 2 + step * (span / 16),
            (home.j() << down) + span / 2,
        )
        .unwrap();
        let hook = StatedDraw {
            synth: &synth,
            answer: std::cell::RefCell::new(None),
        };
        bake_terrain(&synth, &mut SynthCache::new(), key, &BARE, &[&hook]).unwrap();
        match hook
            .answer
            .take()
            .expect("the finest patch's bake ran the hook")
        {
            Ok(rocks) => drawn += usize::from(!rocks.is_empty()),
            Err(crate::scatter::FindRocksError::Query(QueryHeightError::NotSurveyed(cell))) => {
                assert_eq!(
                    cell.level(),
                    level,
                    "{key:?} names a field cell, not itself"
                );
                assert!(
                    view.cell(cell).is_none(),
                    "{key:?} names {cell:?}, which the view holds"
                );
                refused += 1;
            }
        }
    }
    assert!(refused > 0 && drawn > 0, "{refused} refused, {drawn} drawn");
}
