//! The bake on real terrain's tests (plan R10, T6.a), on the synthetic worlds through a client's
//! field, built from a payload of the cells a test surveys and their margin, as a worker holds it.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::OnceLock;

use hyperion_base::units::{Gigayears, PerSquareKilometre};
use hyperion_testkit::float::{assert_same_bits, bits, bits_f32};
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::cube::{Face, FaceUv, face_uv_to_xyz, st_to_uv, unit_dir};
use crate::field::{CoarseField, Cover, CoverRange, FieldView, PartialField, cell_index};
use crate::material::{
    ClassWeights, GroundMaterial, MAX_CLASSES, MaterialClass, WEIGHT_TOTAL, maturity_at_level,
    weights_at,
};
use crate::patch::normals::decode_octahedral;
use crate::patch::{NormalScale, VertexPath, bake_patch};
use crate::synth::test_support::{SEED, random_cells, rim_cells, world};
use crate::testing::{SyntheticWorld, moon_like_builder, moon_like_crater_params};
use crate::wire::{decode_payload, encode_payload};

/// The options most tests bake with: the low setting's, mesh normals and the weights.
pub(super) const MESH: TerrainBakeOptions = TerrainBakeOptions {
    patch: BakeOptions {
        vertex_path: VertexPath::FaceDifferences,
        normals: NormalScale::Mesh,
        skirt_m: 0.0,
    },
    weights: WeightsLayer::Baked,
    horizon: HorizonLayer::Baked,
};

/// The high setting's: baked offsets and normals at twice the mesh's resolution.
pub(super) const DOUBLE: TerrainBakeOptions = TerrainBakeOptions {
    patch: BakeOptions {
        vertex_path: VertexPath::BakedOffsets,
        normals: NormalScale::Double,
        skirt_m: 0.5,
    },
    weights: WeightsLayer::Baked,
    horizon: HorizonLayer::Baked,
};

/// A resolution code of `metres`.
pub(super) fn code(metres: f64) -> ResolutionCode {
    ResolutionCode::from_resolution(Metres::new(metres)).unwrap()
}

/// The payload's blocks of `cells` of `field`, each surveyed at `code_of`'s code, with their
/// margin.
fn blocks(
    field: &CoarseField,
    cells: &BTreeSet<PatchKey>,
    code_of: impl Fn(PatchKey) -> ResolutionCode,
) -> Vec<crate::wire::DecodedBlock> {
    let mut indices: Vec<(u32, PatchKey)> = cells.iter().map(|&c| (cell_index(c), c)).collect();
    indices.sort_unstable();
    let cover = Cover::from_ranges(
        indices
            .iter()
            .map(|&(i, c)| CoverRange::new(i, i + 1, code_of(c)).unwrap()),
    )
    .unwrap();
    decode_payload(&encode_payload(field, &cover, None)).unwrap()
}

/// A client's field of `w` holding `cells`, each surveyed at `code_of`'s code, with their margin.
pub(super) fn client(
    w: SyntheticWorld,
    cells: &BTreeSet<PatchKey>,
    code_of: impl Fn(PatchKey) -> ResolutionCode,
) -> PartialField {
    client_of(world(w), cells, code_of)
}

/// A client's field of `field` holding `cells`, each surveyed at `code_of`'s code, with their
/// margin.
pub(super) fn client_of(
    field: &CoarseField,
    cells: &BTreeSet<PatchKey>,
    code_of: impl Fn(PatchKey) -> ResolutionCode,
) -> PartialField {
    let blocks = blocks(field, cells, code_of);
    let mut client = PartialField::new(blocks[0].header().unwrap().clone());
    for block in &blocks {
        client.insert(block).unwrap();
    }
    client
}

/// The coarse cells of `field`'s level that hold a vertex or a double-resolution normal's sample
/// of `key`.
pub(super) fn cells_under(field: &CoarseField, key: PatchKey) -> BTreeSet<PatchKey> {
    let level = field.header().level().get();
    (0..=128)
        .flat_map(|y| (0..=128).map(move |x| (x, y)))
        .map(|(x, y)| {
            PatchKey::containing(level, key.sample_dir(x, y, crate::cube::SampleGrid::Double))
                .unwrap()
        })
        .collect()
}

/// The `level` patch at the same place in `cell` (a coarse cell, `level` at least its own), or
/// the one holding it.
pub(super) fn patch_in(cell: PatchKey, level: u8) -> PatchKey {
    if level >= cell.level() {
        let down = level - cell.level();
        let inset = (1_u32 << down) / 3;
        PatchKey::new(
            cell.face(),
            level,
            (cell.i() << down) + inset,
            (cell.j() << down) + inset,
        )
        .unwrap()
    } else {
        let up = cell.level() - level;
        PatchKey::new(cell.face(), level, cell.i() >> up, cell.j() >> up).unwrap()
    }
}

/// Every byte a bake makes, as words: R05's arrays and scalars, then the layers.
fn bake_bits(bake: &TerrainBake) -> Vec<u64> {
    let p = bake.patch();
    let l = bake.layers();
    let f32s = |v: &[f32]| {
        v.iter()
            .map(|&x| u64::from(bits_f32(x)))
            .collect::<Vec<_>>()
    };
    let mut out = vec![p.key.to_u64()];
    out.extend(p.origin.map(bits));
    out.push(bits(p.origin_height_m));
    out.extend(f32s(&p.heights));
    out.extend(p.offsets.as_deref().map(f32s).unwrap_or_default());
    out.extend(f32s(&p.normals));
    out.extend([p.height_range_m.0, p.height_range_m.1].map(|h| u64::from(bits_f32(h))));
    out.extend([bits(p.bounding_radius_m), bits(p.skirt_depth_m)]);
    out.extend(l.palette().map(|m| {
        m.map_or(u64::MAX, |m| {
            (u64::from(u8::from(m.class())) << 16) | m.entry().map_or(0xffff, u64::from)
        })
    }));
    out.extend(l.weights().iter().flatten().map(|&b| u64::from(b)));
    out.push(u64::try_from(l.maturity().len()).unwrap());
    out.extend(l.maturity().iter().flatten().map(|&b| u64::from(b)));
    out.extend(l.survey_mask().iter().map(|&b| u64::from(b)));
    out.extend(l.horizon().near_words().iter().map(|&w| u64::from(w)));
    out.extend(l.horizon().far_words().iter().map(|&w| u64::from(w)));
    out.extend([bits(l.bound().value()), bits(l.sigma().value())]);
    out.push(u64::from(l.is_complete()));
    out.push(u64::from(l.horizon().is_complete()));
    out.push(u64::try_from(l.extra().len()).unwrap());
    out
}

/// The bake's materials and weights at vertex `n`, as (material, byte) pairs in palette order.
fn vertex_weights(layers: &TerrainLayers, n: usize) -> Vec<(GroundMaterial, u8)> {
    layers
        .palette()
        .iter()
        .zip(layers.weights()[n])
        .filter_map(|(m, b)| m.filter(|_| b > 0).map(|m| (m, b)))
        .collect()
}

/// `weights` as (material, byte) pairs, in rising material order.
fn sorted(weights: &ClassWeights) -> Vec<(GroundMaterial, u8)> {
    let mut pairs: Vec<_> = weights.iter().collect();
    pairs.sort_unstable();
    pairs
}

/// The face coordinates of vertex (`x`, `y`) of `key`, a patch off the cube's face edges, from
/// which [`PatchKey::vertex_dir`] makes its direction.
fn vertex_uv(key: PatchKey, x: u8, y: u8) -> FaceUv {
    let quads = f64::from(64_u32 << key.level());
    let at = |cell: u32, k: u8| st_to_uv(f64::from(cell * 64 + u32::from(k)) / quads);
    FaceUv {
        face: key.face(),
        u: at(key.i(), x),
        v: at(key.j(), y),
    }
}

/// R05's finest level on `field`'s body, at which the classifier reads the ground.
fn finest_of(field: &CoarseField) -> u8 {
    crate::geometry::finest_level(field.header().figure().equatorial_radius_m)
}

/// The `f32` nearest `x`, as the bake narrows a height.
fn narrowed(x: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the bake's own narrowing of a finite height"
    )]
    let y = x as f32;
    y
}

#[test]
fn bake_heights_are_r09_s_height_at_narrowed_to_f32_and_r05_s_bake_over_the_whole_field() {
    let mut rng = Lcg::new(0x0062_616b_6531);
    for (k, &w) in SyntheticWorld::ALL.iter().enumerate() {
        let field = world(w);
        let coarse = field.header().level().get();
        let finest = finest_of(field);
        // A cube corner's cell on the first world, so that a bake's seams are among them.
        let cell = if k == 0 {
            PatchKey::new(Face::PosX, coarse, 0, 0).unwrap()
        } else {
            random_cells(coarse, 4, &mut rng)[3]
        };
        let whole = Synthesiser::new(field, SEED);
        for level in [coarse - 1, coarse + 2, finest] {
            let key = patch_in(cell, level);
            let view = client(w, &cells_under(field, key), |_| code(100.0));
            let synth = Synthesiser::new(&view, SEED);
            let opts = if level == coarse + 2 && k < 2 {
                DOUBLE
            } else {
                MESH
            };
            let bake = bake_terrain(&synth, &mut SynthCache::new(), key, &opts, &[]).unwrap();
            let layers = bake.layers();
            assert!(
                layers.survey_mask().iter().all(|&m| m == SURVEYED),
                "{w:?} {key:?}"
            );
            assert!(layers.is_complete(), "{w:?} {key:?}");
            // R05's bake over the whole field, through R10.T3's height source.
            let theirs = bake_patch(&whole, key, &opts.patch, &mut SynthCache::new()).unwrap();
            assert_eq!(
                bake_bits(&TerrainBake {
                    patch: theirs,
                    layers: layers.clone()
                }),
                bake_bits(&bake),
                "{w:?} {key:?}"
            );
            // And each vertex's point query, narrowed.
            let mut cache = SynthCache::new();
            let band = BandLevel::new(level).unwrap();
            let parent = BandLevel::new(level - 1).unwrap();
            for y in 0..=64_u8 {
                for x in 0..=64_u8 {
                    let n = usize::from(y) * 65 + usize::from(x);
                    let dir = key.vertex_dir(x, y);
                    let h0 = whole.height_at(&mut cache, dir, band).unwrap().height_m;
                    assert_eq!(
                        bits_f32(bake.patch().heights[2 * n]),
                        bits_f32(narrowed(h0)),
                        "{w:?} {key:?} ({x}, {y})"
                    );
                    if x % 2 == 0 && y % 2 == 0 {
                        let h1 = whole.height_at(&mut cache, dir, parent).unwrap().height_m;
                        assert_eq!(
                            bits_f32(bake.patch().heights[2 * n + 1]),
                            bits_f32(narrowed(h1)),
                            "{w:?} {key:?} ({x}, {y}) at the parent's level"
                        );
                    }
                }
            }
        }
    }
}

/// The Moon-like world's level-2 patch over its face +y's cells 16 to 31, and the 2 × 2 cells
/// surveyed at its middle, 22 and 23 either way: its vertices four to a cell, surveyed, in the
/// survey's margin and held, and beyond the margin, not held.
pub(super) fn straddling() -> (SyntheticWorld, PatchKey, BTreeSet<PatchKey>) {
    let w = SyntheticWorld::MoonLike;
    let coarse = world(w).header().level().get();
    let key = PatchKey::new(Face::PosY, 2, 1, 1).unwrap();
    let surveyed = [(22, 22), (23, 22), (22, 23), (23, 23)]
        .map(|(i, j)| PatchKey::new(Face::PosY, coarse, i, j).unwrap())
        .into_iter()
        .collect();
    (w, key, surveyed)
}

#[test]
fn bake_survey_mask_is_zero_exactly_on_vertices_of_unsurveyed_or_margin_cells() {
    let (w, key, surveyed) = straddling();
    let field = world(w);
    let coarse = field.header().level().get();
    let view = client(w, &surveyed, |_| code(50.0));
    let synth = Synthesiser::new(&view, SEED);
    let whole = Synthesiser::new(field, SEED);
    let mut cache = SynthCache::new();
    let bake = bake_terrain(&synth, &mut cache, key, &MESH, &[]).unwrap();
    let layers = bake.layers();
    let band = BandLevel::new(key.level()).unwrap();
    let (mut counts, mut answered_margin) = ([0_u32; 3], 0_u32);
    for y in 0..=64_u8 {
        for x in 0..=64_u8 {
            let n = usize::from(y) * 65 + usize::from(x);
            let dir = key.vertex_dir(x, y);
            let cell = PatchKey::containing(coarse, dir).unwrap();
            let mask = layers.survey_mask()[n];
            assert_eq!(
                mask == SURVEYED,
                view.is_surveyed(cell),
                "({x}, {y}) in {cell:?}"
            );
            assert!(mask == SURVEYED || mask == UNSURVEYED);
            // Off the cells' edges, the cell by the lattice's integers alone: four quads a cell.
            if x % 4 != 0 && y % 4 != 0 {
                let (i, j) = (16 + u32::from(x) / 4, 16 + u32::from(y) / 4);
                let by_lattice =
                    surveyed.contains(&PatchKey::new(Face::PosY, coarse, i, j).unwrap());
                assert_eq!(mask == SURVEYED, by_lattice, "({x}, {y})");
            }
            let baked = bake.patch().heights[2 * n];
            let h0 = f64::from(baked);
            match (view.resolution(cell), mask) {
                (Some(_), SURVEYED) => counts[0] += 1,
                (Some(_), _) => {
                    counts[1] += 1;
                    // A margin vertex carries the ground where the view can make it.
                    if let Ok(s) = synth.height_at(&mut SynthCache::new(), dir, band) {
                        let truth = whole.height_at(&mut SynthCache::new(), dir, band).unwrap();
                        assert_eq!(bits(s.height_m), bits(truth.height_m));
                        assert_eq!(bits_f32(narrowed(truth.height_m)), bits_f32(baked));
                        answered_margin += 1;
                    } else {
                        assert_eq!(bits(h0), bits(0.0), "({x}, {y}) takes the reference");
                    }
                }
                (None, _) => {
                    counts[2] += 1;
                    assert_eq!(bits(h0), bits(0.0), "({x}, {y}) is not held");
                    assert!(layers.weights()[n].iter().all(|&b| b == 0));
                }
            }
        }
    }
    assert!(counts.iter().all(|&c| c > 0), "{counts:?}");
    assert!(answered_margin > 0);
    assert!(!layers.is_complete());
    println!("surveyed, margin and unheld vertices: {counts:?}, margin answered {answered_margin}");
}

#[test]
fn bake_a_cell_s_resolution_code_alone_changes_no_byte() {
    let (w, edge, surveyed) = straddling();
    let coarse = world(w).header().level().get();
    let inside = patch_in(*surveyed.first().unwrap(), coarse + 2);
    let codes: [&dyn Fn(PatchKey) -> ResolutionCode; 4] = [
        &|_| code(2.0),
        &|_| code(50.0),
        &|_| code(1_000.0),
        &|c: PatchKey| {
            code(if (c.i() + c.j()).is_multiple_of(2) {
                3.0
            } else {
                700.0
            })
        },
    ];
    for (key, opts) in [(edge, MESH), (inside, DOUBLE)] {
        let bakes: Vec<Vec<u64>> = codes
            .iter()
            .map(|code_of| {
                let view = client(w, &surveyed, code_of);
                let synth = Synthesiser::new(&view, SEED);
                bake_bits(&bake_terrain(&synth, &mut SynthCache::new(), key, &opts, &[]).unwrap())
            })
            .collect();
        for (k, b) in bakes.iter().enumerate().skip(1) {
            assert_eq!(*b, bakes[0], "{key:?} with codes {k}");
        }
    }
}

#[test]
fn bake_a_block_of_new_cells_changes_nothing_at_a_vertex_surveyed_before_it() {
    let (w, edge, surveyed) = straddling();
    let coarse = world(w).header().level().get();
    let inside = patch_in(*surveyed.first().unwrap(), coarse + 2);
    let mut view = client(w, &surveyed, |_| code(50.0));
    let keys = [edge, inside];
    // A worker's cache, kept across the block.
    let mut warm = SynthCache::new();
    let (before, answered) = {
        let synth = Synthesiser::new(&view, SEED);
        let before: Vec<TerrainBake> = keys
            .iter()
            .map(|&key| bake_terrain(&synth, &mut warm, key, &MESH, &[]).unwrap())
            .collect();
        // The edge patch's unsurveyed vertices the view could make before the block.
        let band = BandLevel::new(edge.level()).unwrap();
        let answered: Vec<usize> = (0..65 * 65)
            .filter(|&n| before[0].layers().survey_mask()[n] == UNSURVEYED)
            .filter(|&n| {
                synth
                    .height_at(&mut SynthCache::new(), vertex_dir(edge, n), band)
                    .is_ok()
            })
            .collect();
        (before, answered)
    };
    assert_ne!(answered, [] as [usize; 0]);
    // The 2 × 2 cells beside the survey, with their margin, as a later block brings them.
    let beside: BTreeSet<PatchKey> = [(24, 22), (25, 22), (24, 23), (25, 23)]
        .map(|(i, j)| PatchKey::new(Face::PosY, coarse, i, j).unwrap())
        .into_iter()
        .collect();
    for block in &blocks(world(w), &beside, |_| code(10.0)) {
        view.insert(block).unwrap();
    }
    let synth = Synthesiser::new(&view, SEED);
    for (key, old) in keys.iter().zip(&before) {
        let new = bake_terrain(&synth, &mut SynthCache::new(), *key, &MESH, &[]).unwrap();
        // The worker's cache, filled before the block, bakes as a fresh one after it, at both
        // settings.
        for opts in [MESH, DOUBLE] {
            let fresh = bake_terrain(&synth, &mut SynthCache::new(), *key, &opts, &[]).unwrap();
            let kept = bake_terrain(&synth, &mut warm, *key, &opts, &[]).unwrap();
            assert_eq!(bake_bits(&kept), bake_bits(&fresh), "{key:?}");
        }
        // A vertex outside the survey that was real ground is still the same ground: the cells it
        // read arrived whole.
        for &n in answered.iter().filter(|_| *key == edge) {
            assert_eq!(
                bits_f32(new.patch().heights[2 * n]),
                bits_f32(old.patch().heights[2 * n]),
                "vertex {n} outside the survey"
            );
        }
        let mut seen = 0;
        for n in 0..65 * 65 {
            if old.layers().survey_mask()[n] != SURVEYED {
                continue;
            }
            seen += 1;
            assert_eq!(new.layers().survey_mask()[n], SURVEYED);
            for k in [2 * n, 2 * n + 1] {
                assert_eq!(
                    bits_f32(new.patch().heights[k]),
                    bits_f32(old.patch().heights[k]),
                    "{key:?} vertex {n}"
                );
            }
            for k in [2 * n, 2 * n + 1] {
                assert_eq!(
                    bits_f32(new.patch().normals[k]),
                    bits_f32(old.patch().normals[k])
                );
            }
            assert_eq!(
                vertex_weights(new.layers(), n),
                vertex_weights(old.layers(), n),
                "{key:?} vertex {n}"
            );
        }
        assert!(seen > 0, "{key:?}");
        let mask_unchanged = new.layers().survey_mask() == old.layers().survey_mask();
        if mask_unchanged && old.layers().is_complete() && old.layers().horizon().is_complete() {
            // Its mask as it was, and every query answered before: the same bytes.
            assert_eq!(bake_bits(&new), bake_bits(old), "{key:?}");
        }
    }
    // The patch inside the survey is the case of the last check.
    assert!(before[1].layers().is_complete());
}

#[test]
fn bake_is_the_same_bytes_in_any_order_and_from_any_cache() {
    let w = SyntheticWorld::MoonLike;
    let field = world(w);
    let coarse = field.header().level().get();
    let rim = rim_cells(field, 1)[0];
    let mut surveyed = BTreeSet::from([rim]);
    surveyed.extend(crate::synth::test_support::king_distances(rim, 1).into_keys());
    let view = client(w, &surveyed, |_| code(20.0));
    let synth = Synthesiser::new(&view, SEED);
    let key = patch_in(rim, coarse + 2);
    let [child, _, _, other] = key.children();
    let keys = [
        key,
        key.edge_neighbour(crate::cube::Edge::UMax),
        key.edge_neighbour(crate::cube::Edge::VMin),
        key.parent().unwrap(),
        child,
        other,
    ];
    let warm = RefCell::new(SynthCache::new());
    assert_order_independent(&keys, |&k| {
        bake_bits(&bake_terrain(&synth, &mut warm.borrow_mut(), k, &MESH, &[]).unwrap())
    });
    for &k in &keys {
        let cold = bake_terrain(&synth, &mut SynthCache::new(), k, &MESH, &[]).unwrap();
        let warmed = bake_terrain(&synth, &mut warm.borrow_mut(), k, &MESH, &[]).unwrap();
        assert_eq!(bake_bits(&warmed), bake_bits(&cold), "{k:?}");
        assert!(cold.layers().is_complete(), "{k:?}");
    }
}

#[test]
fn bake_over_the_survey_s_edge_is_the_same_bytes_in_any_order_and_from_any_cache() {
    // Incomplete bakes among them: whether the view answers a query outside the survey must not
    // depend on what the cache holds.
    let (w, edge, surveyed) = straddling();
    let coarse = world(w).header().level().get();
    let view = client(w, &surveyed, |_| code(50.0));
    let synth = Synthesiser::new(&view, SEED);
    let keys = [
        edge,
        edge.children()[0],
        edge.edge_neighbour(crate::cube::Edge::UMax),
        edge.parent().unwrap(),
        patch_in(*surveyed.last().unwrap(), coarse + 1),
    ];
    let warm = RefCell::new(SynthCache::new());
    assert_order_independent(&keys, |&k| {
        bake_bits(&bake_terrain(&synth, &mut warm.borrow_mut(), k, &MESH, &[]).unwrap())
    });
    let mut incomplete = 0;
    for &k in &keys {
        let cold = bake_terrain(&synth, &mut SynthCache::new(), k, &MESH, &[]).unwrap();
        let warmed = bake_terrain(&synth, &mut warm.borrow_mut(), k, &MESH, &[]).unwrap();
        assert_eq!(bake_bits(&warmed), bake_bits(&cold), "{k:?}");
        incomplete += usize::from(!cold.layers().is_complete());
    }
    assert!(incomplete >= 3, "{incomplete} incomplete bakes");
}

#[test]
fn bake_a_child_at_morph_one_lies_on_its_parent_across_the_survey_s_edge() {
    // Each query takes the reference surface on its own, so a child's morph target at an even
    // vertex is its parent patch's own height there, real ground or the datum.
    let (w, parent, surveyed) = straddling();
    let view = client(w, &surveyed, |_| code(50.0));
    let synth = Synthesiser::new(&view, SEED);
    let mut cache = SynthCache::new();
    let theirs = bake_terrain(&synth, &mut cache, parent, &MESH, &[]).unwrap();
    for (c, child) in parent.children().into_iter().enumerate() {
        let ours = bake_terrain(&synth, &mut cache, child, &MESH, &[]).unwrap();
        let (ox, oy) = (32 * (c % 2), 32 * (c / 2));
        for y in (0..65).step_by(2) {
            for x in (0..65).step_by(2) {
                let n = y * 65 + x;
                let m = (oy + y / 2) * 65 + ox + x / 2;
                assert_eq!(
                    bits_f32(ours.patch().heights[2 * n + 1]),
                    bits_f32(theirs.patch().heights[2 * m]),
                    "child {c} ({x}, {y})"
                );
            }
        }
    }
    assert!(!theirs.layers().is_complete());
}

#[test]
fn bake_weights_are_each_vertex_s_class_weights_over_the_patch_s_palette() {
    let mut rng = Lcg::new(0x0077_6569_6768);
    for &w in &[SyntheticWorld::EarthLike, SyntheticWorld::MoonLike] {
        let field = world(w);
        let coarse = field.header().level().get();
        let finest = finest_of(field);
        // Cells off the face edges, at levels from a coarse one to the finest.
        let cell = random_cells(coarse, 8, &mut rng)
            .into_iter()
            .skip(3)
            .find(|c| {
                let last = (1_u32 << coarse) - 1;
                c.i() > 0 && c.j() > 0 && c.i() < last && c.j() < last
            })
            .unwrap();
        for level in [coarse - 1, coarse + 3, finest] {
            let key = patch_in(cell, level);
            let view = client(w, &cells_under(field, key), |_| code(100.0));
            let synth = Synthesiser::new(&view, SEED);
            let mut cache = SynthCache::new();
            let bake = bake_terrain(&synth, &mut cache, key, &MESH, &[]).unwrap();
            let layers = bake.layers();
            let palette: Vec<GroundMaterial> = layers.palette().iter().flatten().copied().collect();
            assert!(palette.windows(2).all(|p| p[0] < p[1]), "{palette:?}");
            let band = BandLevel::new(level).unwrap();
            let mut union = BTreeSet::new();
            for y in 0..=64_u8 {
                for x in 0..=64_u8 {
                    let n = usize::from(y) * 65 + usize::from(x);
                    let uv = vertex_uv(key, x, y);
                    assert_eq!(
                        unit_dir(face_uv_to_xyz(uv)).map(bits),
                        key.vertex_dir(x, y).map(bits)
                    );
                    let theirs = weights_at(&synth, &mut cache, uv, band).unwrap();
                    union.extend(theirs.iter().map(|(m, _)| m));
                    let mut ours = vertex_weights(layers, n);
                    ours.sort_unstable();
                    assert_eq!(ours, sorted(&theirs), "{w:?} {key:?} ({x}, {y})");
                    assert_eq!(
                        layers.weights()[n]
                            .iter()
                            .map(|&b| u32::from(b))
                            .sum::<u32>(),
                        u32::from(WEIGHT_TOTAL)
                    );
                }
            }
            assert_eq!(
                palette,
                union.into_iter().collect::<Vec<_>>(),
                "{w:?} {key:?}"
            );
        }
    }
}

#[test]
fn bake_bound_and_sigma_are_r09_s_at_the_level() {
    let (w, edge, surveyed) = straddling();
    let field = world(w);
    let coarse = field.header().level().get();
    let view = client(w, &surveyed, |_| code(50.0));
    let synth = Synthesiser::new(&view, SEED);
    let inside = patch_in(*surveyed.first().unwrap(), coarse + 2);
    // Over unheld ground, beyond the survey's margin.
    let away = PatchKey::new(Face::NegZ, coarse + 1, 3, 5).unwrap();
    for key in [edge, inside, away] {
        let band = BandLevel::new(key.level()).unwrap();
        for opts in [
            MESH,
            TerrainBakeOptions {
                weights: WeightsLayer::Skipped,
                ..MESH
            },
        ] {
            let bake = bake_terrain(&synth, &mut SynthCache::new(), key, &opts, &[]).unwrap();
            let layers = bake.layers();
            assert_eq!(
                bits(layers.bound().value()),
                bits(synth.level_bound_m(band))
            );
            let cells: BTreeSet<PatchKey> = (0..65 * 65)
                .filter(|&n| layers.survey_mask()[n] == SURVEYED)
                .map(|n| PatchKey::containing(coarse, vertex_dir(key, n)).unwrap())
                .collect();
            let expected = cells
                .iter()
                .map(|&c| synth.omitted_rms(c, band).unwrap().value())
                .fold(0.0, f64::max);
            assert_eq!(bits(layers.sigma().value()), bits(expected), "{key:?}");
            assert_eq!(cells.is_empty(), key == away);
        }
    }
}

#[test]
fn bake_without_weights_leaves_the_palette_empty_and_the_rest_as_baked() {
    let (w, edge, surveyed) = straddling();
    let view = client(w, &surveyed, |_| code(50.0));
    let synth = Synthesiser::new(&view, SEED);
    let skipped = TerrainBakeOptions {
        weights: WeightsLayer::Skipped,
        ..MESH
    };
    let with = bake_terrain(&synth, &mut SynthCache::new(), edge, &MESH, &[]).unwrap();
    let without = bake_terrain(&synth, &mut SynthCache::new(), edge, &skipped, &[]).unwrap();
    assert_eq!(without.layers().palette(), [None; MAX_CLASSES]);
    assert_eq!(without.layers().weights(), [] as [[u8; MAX_CLASSES]; 0]);
    assert_eq!(without.patch(), with.patch());
    assert_eq!(without.layers().survey_mask(), with.layers().survey_mask());
    assert_eq!(
        bits(without.layers().sigma().value()),
        bits(with.layers().sigma().value())
    );
    assert_eq!(without.layers().is_complete(), with.layers().is_complete());
    assert_eq!(with.layers().weights().len(), 65 * 65);
}

/// A hook that records what it is handed.
struct Recording {
    calls: RefCell<Vec<(PatchKey, Vec<u32>, usize)>>,
}

impl BakeHook for Recording {
    fn bake(&self, key: PatchKey, heights: &[f32], out: &mut Vec<ExtraLayer>) {
        let heights = heights.iter().map(|&h| bits_f32(h)).collect();
        self.calls.borrow_mut().push((key, heights, out.len()));
    }
}

#[test]
fn bake_hands_each_hook_the_patch_and_its_heights_in_turn() {
    let (w, _, surveyed) = straddling();
    let coarse = world(w).header().level().get();
    let key = patch_in(*surveyed.first().unwrap(), coarse + 2);
    let view = client(w, &surveyed, |_| code(50.0));
    let synth = Synthesiser::new(&view, SEED);
    let (first, second) = (
        Recording {
            calls: RefCell::new(Vec::new()),
        },
        Recording {
            calls: RefCell::new(Vec::new()),
        },
    );
    let bake = bake_terrain(
        &synth,
        &mut SynthCache::new(),
        key,
        &MESH,
        &[&first, &second],
    )
    .unwrap();
    let heights: Vec<u32> = bake.patch().heights.iter().map(|&h| bits_f32(h)).collect();
    for hook in [&first, &second] {
        assert_eq!(*hook.calls.borrow(), vec![(key, heights.clone(), 0)]);
    }
    assert_eq!(bake.layers().extra(), []);
    assert!(
        !bake.layers().horizon().is_empty(),
        "the horizon map is baked beside the hooks"
    );
    let without = bake_terrain(&synth, &mut SynthCache::new(), key, &MESH, &[]).unwrap();
    assert_eq!(bake_bits(&without), bake_bits(&bake));
}

#[test]
fn bake_a_patch_wholly_outside_the_survey_is_the_reference_surface() {
    let (w, _, surveyed) = straddling();
    let coarse = world(w).header().level().get();
    let held = client(w, &surveyed, |_| code(50.0));
    let empty = PartialField::new(world(w).header().clone());
    let away = PatchKey::new(Face::NegZ, coarse + 1, 3, 5).unwrap();
    for view in [&held, &empty] {
        let synth = Synthesiser::new(view, SEED);
        for opts in [MESH, DOUBLE] {
            let bake = bake_terrain(&synth, &mut SynthCache::new(), away, &opts, &[]).unwrap();
            let (patch, layers) = (bake.patch(), bake.layers());
            assert!(patch.heights.iter().all(|&h| bits_f32(h) == bits_f32(0.0)));
            assert!(layers.survey_mask().iter().all(|&m| m == UNSURVEYED));
            assert!(layers.weights().iter().flatten().all(|&b| b == 0));
            assert_eq!(layers.palette(), [None; MAX_CLASSES]);
            assert!(!layers.is_complete());
            assert_eq!(bits(layers.sigma().value()), bits(0.0));
            // Its normals are the datum's.
            let figure = HeightSource::figure(&synth);
            let side = opts.patch.normals.side();
            let grid = opts.patch.normals.grid();
            for (k, pair) in patch.normals.chunks(2).enumerate().step_by(97) {
                let (x, y) = (k % side, k / side);
                let dir =
                    away.sample_dir(u32::try_from(x).unwrap(), u32::try_from(y).unwrap(), grid);
                let got = decode_octahedral([f64::from(pair[0]), f64::from(pair[1])]);
                let nu = figure.normal(dir);
                let cos: f64 = (0..3).map(|a| got[a] * nu[a]).sum();
                assert!(cos > 1.0 - 1e-12, "({x}, {y}): {cos}");
            }
        }
    }
}

/// The material of class `class` and entry `entry`.
fn ground(class: MaterialClass, entry: u8) -> GroundMaterial {
    GroundMaterial::of(class, Some(entry))
}

/// Weights of `pairs`, in the order given.
fn weights_of(pairs: &[(GroundMaterial, u8)]) -> ClassWeights {
    let mut palette = [None; MAX_CLASSES];
    let mut weights = [0; MAX_CLASSES];
    for (k, &(m, b)) in pairs.iter().enumerate() {
        palette[k] = Some(m);
        weights[k] = b;
    }
    ClassWeights::from_slots(palette, weights)
}

#[test]
fn bake_palette_is_the_union_in_discriminant_order() {
    let (soil, sand, rock) = (
        ground(MaterialClass::Soil, 1),
        ground(MaterialClass::Sand, 0),
        ground(MaterialClass::Bedrock, 2),
    );
    let vertices = [
        Some(weights_of(&[(soil, 200), (sand, 55)])),
        None,
        Some(weights_of(&[(rock, 255)])),
        Some(weights_of(&[(sand, 128), (rock, 127)])),
    ];
    let (palette, bytes) = layers::gather(&vertices);
    assert_eq!(
        palette[..4],
        [Some(rock), Some(soil), Some(sand), None],
        "Bedrock 0, Soil 2, Sand 3"
    );
    assert_eq!(bytes[0][..3], [0, 200, 55]);
    assert_eq!(bytes[1], [0; MAX_CLASSES]);
    assert_eq!(bytes[2][..3], [255, 0, 0]);
    assert_eq!(bytes[3][..3], [127, 0, 128]);
}

#[test]
fn bake_palette_past_eight_keeps_the_heaviest_and_makes_each_vertex_whole_again() {
    // Ten materials, entry k at (k + 1) vertices of its own; with the two vertices below, entries 0
    // and 1 weigh least over the patch and are dropped.
    let all: Vec<GroundMaterial> = (0..10)
        .map(|k| ground(MaterialClass::Regolith, k))
        .collect();
    let mut vertices: Vec<Option<ClassWeights>> = (0..10_u8)
        .map(|k| Some(weights_of(&[(all[usize::from(k)], 255)])))
        .collect();
    // Weigh each by repeating it, and give one vertex the two dropped and a kept one.
    for k in 0..10 {
        for _ in 0..k {
            vertices.push(vertices[k]);
        }
    }
    vertices.push(Some(weights_of(&[
        (all[9], 100),
        (all[0], 90),
        (all[1], 65),
    ])));
    vertices.push(Some(weights_of(&[(all[0], 200), (all[1], 55)])));
    let (palette, bytes) = layers::gather(&vertices);
    let kept: Vec<GroundMaterial> = palette.iter().flatten().copied().collect();
    assert_eq!(kept, all[2..].to_vec());
    for b in &bytes {
        let sum: u32 = b.iter().map(|&x| u32::from(x)).sum();
        assert_eq!(sum, 255);
    }
    // The mixed vertex: its one kept material takes the whole.
    assert_eq!(bytes[vertices.len() - 2][7], 255);
    // A vertex of dropped materials alone: the whole on the patch's heaviest, entry 9.
    assert_eq!(bytes[vertices.len() - 1], [0, 0, 0, 0, 0, 0, 0, 255]);
    // A vertex with no weights has none.
    let (_, bytes) = layers::gather(&[None, Some(weights_of(&[(all[3], 255)]))]);
    assert_eq!(bytes[0], [0; MAX_CLASSES]);
    // Equal totals at the eighth slot: the lower material is kept.
    let mut vertices = vec![Some(weights_of(&[
        (all[0], 85),
        (all[1], 85),
        (all[2], 85),
    ]))];
    vertices.extend((3..10).map(|k| Some(weights_of(&[(all[k], 255)]))));
    let (palette, bytes) = layers::gather(&vertices);
    let mut expected = vec![all[0]];
    expected.extend_from_slice(&all[3..]);
    assert_eq!(
        palette.iter().flatten().copied().collect::<Vec<_>>(),
        expected
    );
    assert_eq!(bytes[0], [255, 0, 0, 0, 0, 0, 0, 0]);
    // One dropped of three: the two kept made whole by the largest remainder. Of 101 and 104 over
    // 205, 125.6 and 129.4: 126 and 129.
    let mut vertices = vec![Some(weights_of(&[
        (all[0], 101),
        (all[1], 50),
        (all[2], 104),
    ]))];
    vertices.extend((3..9).map(|k| Some(weights_of(&[(all[k], 255)]))));
    let (palette, bytes) = layers::gather(&vertices);
    assert_eq!(palette[..2], [Some(all[0]), Some(all[2])]);
    assert_eq!(bytes[0][..3], [126, 129, 0]);
}

// ---- R10.T1.c's client half: the maturity layer (decision-r10-t1c-client-scope §2.5) ----

/// The low setting's bake without the horizon map, which the maturity's tests do not read.
const MESH_ALONE: TerrainBakeOptions = TerrainBakeOptions {
    horizon: HorizonLayer::Skipped,
    ..MESH
};

/// The weathered Moon-like world on its 1 Gyr surface (N(>1 km) 8.4 × 10⁻⁴) at maturity 1 and
/// the Moon's rate, the ruling's look fixture's world, and the same world without a maturity:
/// built once per test process.
fn weathered_pair() -> &'static (CoarseField, CoarseField) {
    static WORLDS: OnceLock<(CoarseField, CoarseField)> = OnceLock::new();
    WORLDS.get_or_init(|| {
        let builder = || {
            moon_like_builder()
                .surface_age(Gigayears::new(1.0))
                .crater_params(moon_like_crater_params(PerSquareKilometre::new(8.4e-4)))
        };
        (
            builder().maturity(1.0).weathering_rate(1.0).build(),
            builder().build(),
        )
    })
}

/// The patches the maturity's bake tests read: the level-12 patch at the scope probe's place, its
/// neighbour across its u edge, its parent and two of its children.
fn maturity_keys() -> [PatchKey; 5] {
    let key = PatchKey::containing(12, unit_dir([0.3, 1.0, 0.2])).unwrap();
    let [child, _, _, other] = key.children();
    [
        key,
        key.edge_neighbour(crate::cube::Edge::UMax),
        key.parent().unwrap(),
        child,
        other,
    ]
}

/// A client's field of `field` holding the coarse cells under each of `keys`, surveyed at 20 m,
/// with their margin.
fn maturity_view(field: &CoarseField, keys: &[PatchKey]) -> PartialField {
    let cells = keys.iter().flat_map(|&k| cells_under(field, k)).collect();
    client_of(field, &cells, |_| code(20.0))
}

/// Test 2: the maturity layer is the same bytes in any order of bakes and from any cache, on the
/// weathered world, where its codes are not all the background's.
#[test]
fn bake_maturity_is_the_same_bytes_in_any_order_and_from_any_cache() {
    let (weathered, _) = weathered_pair();
    let keys = maturity_keys();
    let view = maturity_view(weathered, &keys);
    let synth = Synthesiser::new(&view, SEED);
    let warm = RefCell::new(SynthCache::new());
    assert_order_independent(&keys, |&k| {
        bake_bits(&bake_terrain(&synth, &mut warm.borrow_mut(), k, &MESH_ALONE, &[]).unwrap())
    });
    for &k in &keys {
        let cold = bake_terrain(&synth, &mut SynthCache::new(), k, &MESH_ALONE, &[]).unwrap();
        let warmed = bake_terrain(&synth, &mut warm.borrow_mut(), k, &MESH_ALONE, &[]).unwrap();
        assert_eq!(bake_bits(&warmed), bake_bits(&cold), "{k:?}");
        let maturity = cold.layers().maturity();
        assert_eq!(maturity.len(), 65 * 65, "{k:?}");
        assert!(
            maturity.iter().any(|&[here, _]| here > 0),
            "{k:?}: some of the 1 Gyr ground is fresher than its background"
        );
    }
}

/// Test 3: without a maturity the layer is empty and no other byte of the bake moves, the same
/// world with and without the header's maturity baking the same bytes but the layer's; nor is it
/// made without the weights; and no synthetic world carries one.
#[test]
fn bake_maturity_without_a_maturity_is_empty_and_moves_no_other_byte() {
    let (weathered, plain) = weathered_pair();
    let key = maturity_keys()[0];
    let bake = |field: &CoarseField, opts: &TerrainBakeOptions| {
        let view = maturity_view(field, &[key]);
        let synth = Synthesiser::new(&view, SEED);
        bake_terrain(&synth, &mut SynthCache::new(), key, opts, &[]).unwrap()
    };
    let with = bake(weathered, &MESH);
    let without = bake(plain, &MESH);
    assert_eq!(with.layers().maturity().len(), 65 * 65);
    assert_eq!(without.layers().maturity(), [] as [[u8; 2]; 0]);
    let mut stripped = with.clone();
    stripped.layers.maturity.clear();
    assert_eq!(bake_bits(&stripped), bake_bits(&without));
    let skipped = TerrainBakeOptions {
        weights: WeightsLayer::Skipped,
        ..MESH
    };
    let unweighted = bake(weathered, &skipped);
    assert_eq!(unweighted.layers().maturity(), [] as [[u8; 2]; 0]);
    assert_eq!(unweighted.patch(), with.patch());
    assert_eq!(unweighted.layers().horizon(), with.layers().horizon());
    for &w in SyntheticWorld::ALL {
        assert_eq!(world(w).header().maturity(), None, "{w:?}");
    }
}

/// Each vertex's pair of codes is the point rule's at its direction at the patch's level and at
/// its parent's ([`maturity_at_level`]), encoded against `m_body`, inside the survey and over its
/// edge, and a vertex without weights is the background's, code 0, for both.
#[test]
fn bake_maturity_is_each_vertex_s_point_rule_at_its_level_and_its_parent_s() {
    let (weathered, _) = weathered_pair();
    let background = weathered.header().maturity().unwrap();
    let key = maturity_keys()[0];
    let inside = maturity_view(weathered, &[key]);
    let (_, edge, surveyed) = straddling();
    let over_the_edge = client_of(weathered, &surveyed, |_| code(50.0));
    let (mut fresh, mut without) = (0, 0);
    for (view, key) in [(&inside, key), (&over_the_edge, edge)] {
        let synth = Synthesiser::new(view, SEED);
        let mut cache = SynthCache::new();
        let bake = bake_terrain(&synth, &mut cache, key, &MESH_ALONE, &[]).unwrap();
        let layers = bake.layers();
        let level = BandLevel::new(key.level()).unwrap();
        let parent = BandLevel::new(key.level() - 1).unwrap();
        for k in 0..=64_u8 {
            for (x, y) in [(k, k), (64 - k, k)] {
                let n = usize::from(y) * 65 + usize::from(x);
                let pair = layers.maturity()[n];
                if layers.weights()[n] == [0; MAX_CLASSES] {
                    assert_eq!(pair, [0, 0], "{key:?} ({x}, {y})");
                    without += 1;
                    continue;
                }
                let uv = vertex_uv(key, x, y);
                assert_eq!(
                    unit_dir(face_uv_to_xyz(uv)).map(bits),
                    key.vertex_dir(x, y).map(bits)
                );
                let mut at = |band: BandLevel| {
                    let m = maturity_at_level(&synth, &mut cache, uv, band)
                        .unwrap()
                        .unwrap();
                    encode_maturity(m, background)
                };
                assert_eq!(pair, [at(level), at(parent)], "{key:?} ({x}, {y})");
                fresh += usize::from(pair[0] > 0);
            }
        }
    }
    assert!(
        fresh > 0,
        "some vertex's ground is fresher than its background"
    );
    assert!(
        without > 0,
        "the patch over the survey's edge has vertices without weights"
    );
}

/// The layer's codes ([`encode_maturity`], [`decode_maturity`]): 0 at the background and 255 on
/// fresh ground, each held to the bit; a code within half a step of r = √(D ÷ `m_body`), falling
/// as the maturity rises; every code's maturity encoding to the code again; and every code 0, each
/// holding +0, on a background of 0.
#[test]
fn bake_maturity_codes_round_trip_and_hold_their_ends() {
    for background in [1.0, 0.75, 0.3, 1e-6] {
        assert_eq!(encode_maturity(background, background), 0);
        assert_eq!(encode_maturity(0.0, background), 255);
        assert_same_bits(decode_maturity(0, background), background);
        assert_same_bits(decode_maturity(255, background), 0.0);
        let mut previous = u8::MAX;
        for k in 0..=1_000_u32 {
            let m = background * f64::from(k) / 1_000.0;
            let code = encode_maturity(m, background);
            assert!(
                code <= previous,
                "{background}: {code} after {previous} at {m}"
            );
            previous = code;
            let r = ((background - m) / background).sqrt();
            let back = ((background - decode_maturity(code, background)) / background).sqrt();
            assert!(
                (back - r).abs() <= 0.5 / 255.0 + 1e-12,
                "{background}: {m} encodes to {code}, r {back} against {r}"
            );
        }
        for code in 0..=255_u8 {
            let m = decode_maturity(code, background);
            assert!(
                (0.0..=background).contains(&m),
                "{background}: {code} holds {m}"
            );
            assert_eq!(encode_maturity(m, background), code, "{background}: {m}");
        }
    }
    for m in [0.0, -0.0] {
        assert_eq!(encode_maturity(m, 0.0), 0);
    }
    for code in [0, 1, 128, 255] {
        assert_same_bits(decode_maturity(code, 0.0), 0.0);
    }
}
