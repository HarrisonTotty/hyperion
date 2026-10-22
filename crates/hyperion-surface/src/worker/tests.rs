//! The worker's fields' tests (plan R10, T6.b), on the Moon-like synthetic world through payloads
//! of two small surveys, cut into blocks small enough that each payload is several.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use hyperion_base::units::{Gigayears, PerSquareKilometre};
use hyperion_testkit::float::{bits, bits_f32};
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::bake::{HorizonLayer, SURVEYED, WeightsLayer, decode_maturity};
use crate::cube::{Face, st_to_uv};
use crate::field::{
    CoarseField, CoarseLevel, Cover, CoverRange, PaletteRole, ResolutionCode,
    SYNTHESIS_MARGIN_CELLS, SynthesisCell,
};
use crate::geometry::BAND_LIMIT_M;
use crate::material::{
    AlbedoScale, ClassFractions, MaterialClass, MaturityMix, class_fractions, haloed_albedo_scale,
    maturity_cut, maturity_mix, maturity_node_albedo, maturity_node_share, properties,
};
use crate::patch::{BakeOptions, NormalScale, VertexPath};
use crate::synth::test_support::{SEED, world};
use crate::testing::{FieldBuilder, SyntheticWorld, moon_like_builder, moon_like_crater_params};
use crate::wire::{BLOCK_HEADER_BYTES, decode_payload, encode_within};

/// The fixture's world: the Moon-like, at level 6.
pub(super) const WORLD: SyntheticWorld = SyntheticWorld::MoonLike;

/// The largest block of the fixture's payloads, bytes: small, so that each payload is several
/// blocks, a block 0 with the header and later ones without.
const FIXTURE_BLOCK_BYTES: usize = 4096;

/// The low setting's options with the weights, and without the horizon map, as the client's
/// workers bake until R10.T8.b reads it.
pub(super) const MESH: TerrainBakeOptions = TerrainBakeOptions {
    patch: BakeOptions {
        vertex_path: VertexPath::FaceDifferences,
        normals: NormalScale::Mesh,
        skirt_m: 0.0,
    },
    weights: WeightsLayer::Baked,
    horizon: HorizonLayer::Skipped,
};

/// The high setting's: baked offsets and doubled normals, with no extra skirt margin, as the
/// client's workers bake.
pub(super) const DOUBLE: TerrainBakeOptions = TerrainBakeOptions {
    patch: BakeOptions {
        vertex_path: VertexPath::BakedOffsets,
        normals: NormalScale::Double,
        skirt_m: 0.0,
    },
    weights: WeightsLayer::Baked,
    horizon: HorizonLayer::Skipped,
};

/// The wireframe's: no weights.
pub(super) const DEPTH_ONLY: TerrainBakeOptions = TerrainBakeOptions {
    patch: BakeOptions {
        vertex_path: VertexPath::FaceDifferences,
        normals: NormalScale::Mesh,
        skirt_m: 0.0,
    },
    weights: WeightsLayer::Skipped,
    horizon: HorizonLayer::Skipped,
};

/// The fixture: two surveys of the Moon-like world's face +y and their payloads, the second a
/// delta, each cut into blocks.
pub(super) struct Fixture {
    /// The world's body.
    pub(super) body: BodyRef,
    /// The first survey's 2 × 2 cells, at 50 m.
    pub(super) first: BTreeSet<PatchKey>,
    /// The second's 2 × 2 cells beside the first, at 20 m.
    pub(super) second: BTreeSet<PatchKey>,
    /// The first survey's payload's blocks.
    pub(super) first_blocks: Vec<Vec<u8>>,
    /// The delta from the first survey to both.
    pub(super) second_blocks: Vec<Vec<u8>>,
}

impl Fixture {
    /// Every block, the first payload's first.
    pub(super) fn blocks(&self) -> impl Iterator<Item = &[u8]> {
        self.first_blocks
            .iter()
            .chain(&self.second_blocks)
            .map(Vec::as_slice)
    }

    /// A worker holding every block of the fixture.
    pub(super) fn worker(&self) -> WorkerFields {
        let mut fields = WorkerFields::new();
        let mut blocks = self.blocks();
        fields
            .init(self.body, blocks.next().expect("a payload has a block 0"))
            .unwrap();
        for block in blocks {
            fields.chunk(self.body, block).unwrap();
        }
        fields
    }

    /// The same field as a client builds it from the decoded payloads, block by block.
    pub(super) fn field(&self) -> PartialField {
        let payloads = [&self.first_blocks, &self.second_blocks]
            .map(|blocks| decode_payload(&blocks.concat()).expect("the fixture's payloads decode"));
        let header = payloads[0][0]
            .header()
            .expect("block 0 has the header")
            .clone();
        let mut field = PartialField::new(header);
        for block in payloads.iter().flatten() {
            field.insert(block).unwrap();
        }
        field
    }
}

/// The Moon-like world's cell `(i, j)` of face +y at its level.
pub(super) fn cell(i: u32, j: u32) -> PatchKey {
    PatchKey::new(Face::PosY, world(WORLD).header().level().get(), i, j).unwrap()
}

/// A resolution code of `metres`.
fn code(metres: f64) -> ResolutionCode {
    ResolutionCode::from_resolution(Metres::new(metres)).unwrap()
}

/// The cover of `cells` at the code `code_of` gives each.
fn cover(cells: &BTreeSet<PatchKey>, code_of: impl Fn(PatchKey) -> ResolutionCode) -> Cover {
    let mut indices: Vec<(u32, PatchKey)> = cells.iter().map(|&c| (cell_index(c), c)).collect();
    indices.sort_unstable();
    Cover::from_ranges(
        indices
            .iter()
            .map(|&(i, c)| CoverRange::new(i, i + 1, code_of(c)).unwrap()),
    )
    .unwrap()
}

/// `bytes` cut at each block's length, the `u32` at offset 6 of its frame.
fn split(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut blocks = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let length = u32::from_le_bytes(rest[6..10].try_into().unwrap());
        let (block, after) = rest.split_at(usize::try_from(length).unwrap());
        blocks.push(block.to_vec());
        rest = after;
    }
    blocks
}

/// The fixture.
pub(super) fn fixture() -> Fixture {
    let field: &CoarseField = world(WORLD);
    let first: BTreeSet<PatchKey> = [(22, 22), (23, 22), (22, 23), (23, 23)]
        .map(|(i, j)| cell(i, j))
        .into_iter()
        .collect();
    let second: BTreeSet<PatchKey> = [(24, 22), (25, 22), (24, 23), (25, 23)]
        .map(|(i, j)| cell(i, j))
        .into_iter()
        .collect();
    let code_of = |c: PatchKey| {
        if first.contains(&c) {
            code(50.0)
        } else {
            code(20.0)
        }
    };
    let first_cover = cover(&first, code_of);
    let both: BTreeSet<PatchKey> = first.union(&second).copied().collect();
    let both_cover = cover(&both, code_of);
    let encode = |c: &Cover, since: Option<&Cover>| {
        split(&encode_within(
            field,
            c,
            since,
            SYNTHESIS_MARGIN_CELLS,
            FIXTURE_BLOCK_BYTES,
        ))
    };
    Fixture {
        body: field.header().body(),
        first_blocks: encode(&first_cover, None),
        second_blocks: encode(&both_cover, Some(&first_cover)),
        first,
        second,
    }
}

/// `TERRAIN TEST`'s first survey's centre (plan R10, R10.T16.d), not normalised: on the edge
/// between faces +x and +y, at v = 0.8 of both, some five cells of level 6 from the corner they
/// share with face +z, so that the survey straddles the edge and its margin reaches round the
/// corner (T8.b's third place, "a face edge near a cube corner").
const TERRAIN_FIRST_CENTRE: [f64; 3] = [1.0, 1.0, 0.8];

/// The second survey's centre, along the same edge away from the corner, at v = 0.62: some four
/// cells from the first's, so that the second overlaps the first's rim and extends it.
const TERRAIN_SECOND_CENTRE: [f64; 3] = [1.0, 1.0, 0.62];

/// The cosine of each survey's angular radius about its centre, which a surveyed cell's centre is
/// within: 0.9985, about 3.1°, some two cells of level 6 there.
const TERRAIN_SURVEY_DOT: f64 = 0.9985;

/// One of `TERRAIN TEST`'s payloads: R09.T19.b's answer, at the body's revision `revision`, to a
/// `surface_field` with `have_revision` `have`, cut into the fixture's blocks.
pub(super) struct TerrainPayload {
    /// The body's coverage revision the payload reflects, from 1.
    pub(super) revision: u32,
    /// The revision the asker held, `None` for the whole field.
    pub(super) have: Option<u32>,
    /// The payload's blocks, in order.
    pub(super) blocks: Vec<Vec<u8>>,
}

/// `TERRAIN TEST`'s field (plan R10, R10.T16.d; decision-r10-t16-contracts §4): the Moon-like
/// world's two surveys about the edge of faces +x and +y near their corner with +z, and the
/// payloads R09.T19.b answers over them.
pub(super) struct TerrainFixture {
    /// The world's body.
    pub(super) body: BodyRef,
    /// The coverage at revisions 1 and 2, as the server's survey log folds it (the finer code
    /// winning): the first survey at 50 m, then the second's cells at 20 m with it.
    pub(super) surveys: [Cover; 2],
    /// The first payload (revision 1, whole); the delta from it to revision 2; the whole field at
    /// revision 2; and the answer to an ask at the revision held, block 0 alone.
    pub(super) payloads: [TerrainPayload; 4],
}

impl TerrainFixture {
    /// The blocks a session posts in turn: the first payload's, then the delta's.
    pub(super) fn first_then_delta(&self) -> impl Iterator<Item = &[u8]> {
        self.payloads[0]
            .blocks
            .iter()
            .chain(&self.payloads[1].blocks)
            .map(Vec::as_slice)
    }
}

/// The cells of `field`'s level whose centres lie within the angle whose cosine is `dot` of the
/// direction `centre`.
fn cells_about(field: &CoarseField, centre: [f64; 3], dot: f64) -> BTreeSet<PatchKey> {
    let level = field.header().level();
    let towards = unit_dir(centre);
    (0..level.cell_count())
        .map(|index| cell_at_index(level.get(), index).expect("an index below the count"))
        .filter(|&cell| {
            let p = point(cell, 0.5, 0.5);
            p[0] * towards[0] + p[1] * towards[1] + p[2] * towards[2] >= dot
        })
        .collect()
}

/// `TERRAIN TEST`'s fixture.
pub(super) fn terrain_fixture() -> TerrainFixture {
    terrain_fixture_of(world(WORLD))
}

/// `TERRAIN TEST`'s surveys and payloads over `field`, a Moon-like world of the fixture's level:
/// [`terrain_fixture`]'s of `field` ([`weathered_terrain_world`]'s for `TEST MOON` weathered).
pub(super) fn terrain_fixture_of(field: &CoarseField) -> TerrainFixture {
    let first = cells_about(field, TERRAIN_FIRST_CENTRE, TERRAIN_SURVEY_DOT);
    let second = cells_about(field, TERRAIN_SECOND_CENTRE, TERRAIN_SURVEY_DOT);
    let first_cover = cover(&first, |_| code(50.0));
    let both: BTreeSet<PatchKey> = first.union(&second).copied().collect();
    let both_cover = cover(&both, |c| {
        if second.contains(&c) {
            code(20.0)
        } else {
            code(50.0)
        }
    });
    let encode = |c: &Cover, since: Option<&Cover>| {
        split(&encode_within(
            field,
            c,
            since,
            SYNTHESIS_MARGIN_CELLS,
            FIXTURE_BLOCK_BYTES,
        ))
    };
    let payloads = [
        TerrainPayload {
            revision: 1,
            have: None,
            blocks: encode(&first_cover, None),
        },
        TerrainPayload {
            revision: 2,
            have: Some(1),
            blocks: encode(&both_cover, Some(&first_cover)),
        },
        TerrainPayload {
            revision: 2,
            have: None,
            blocks: encode(&both_cover, None),
        },
        TerrainPayload {
            revision: 2,
            have: Some(2),
            blocks: encode(&both_cover, Some(&both_cover)),
        },
    ];
    TerrainFixture {
        body: field.header().body(),
        surveys: [first_cover, both_cover],
        payloads,
    }
}

/// The Earth-like fixture (plan R10, T12): one survey of the Earth-like world whose field holds a
/// finest-level patch of three classes, for the client's test that a bake's dominant weight names
/// the worker's material at every vertex (`fieldsWasm.test.ts`; `tests/golden/worker/
/// earth_like.golden`, the [`golden`](super::golden) module's documentation gives the spot).
pub(super) struct EarthFixture {
    /// The world's body.
    pub(super) body: BodyRef,
    /// The finest-level patch of three classes.
    pub(super) patch: PatchKey,
    /// The survey's payload's blocks, in order.
    pub(super) blocks: Vec<Vec<u8>>,
}

/// The Earth-like world's cell the fixture surveys at 50 m: (0, 0) of face +x at level 8.
pub(super) fn earth_cell() -> PatchKey {
    PatchKey::new(
        Face::PosX,
        world(SyntheticWorld::EarthLike).header().level().get(),
        0,
        0,
    )
    .unwrap()
}

/// The fixture's patch: (900, 668) of face +x at the Earth-like world's finest level, 19, in
/// [`earth_cell`], whose ground is soil, sand and a channel's sediment of the continental crust's
/// granite.
pub(super) fn earth_patch() -> PatchKey {
    PatchKey::new(Face::PosX, 19, 900, 668).unwrap()
}

/// The Earth-like fixture: [`earth_cell`] surveyed at 50 m with its margin, in the fixture's blocks.
pub(super) fn earth_fixture() -> EarthFixture {
    let field: &CoarseField = world(SyntheticWorld::EarthLike);
    let cells: BTreeSet<PatchKey> = [earth_cell()].into_iter().collect();
    let blocks = split(&encode_within(
        field,
        &cover(&cells, |_| code(50.0)),
        None,
        SYNTHESIS_MARGIN_CELLS,
        FIXTURE_BLOCK_BYTES,
    ));
    EarthFixture {
        body: field.header().body(),
        patch: earth_patch(),
        blocks,
    }
}

/// A worker holding `f`'s blocks.
pub(super) fn earth_worker(f: &EarthFixture) -> WorkerFields {
    let mut fields = WorkerFields::new();
    fields.init(f.body, &f.blocks[0]).unwrap();
    for block in &f.blocks[1..] {
        fields.chunk(f.body, block).unwrap();
    }
    fields
}

/// The point of vertex (`x`, `y`) of `key`, on the patch's own face: an interior patch's vertex
/// direction is `unit_dir(face_uv_to_xyz(...))` of exactly this (`PatchKey::vertex_dir`).
pub(super) fn vertex_point(key: PatchKey, x: u32, y: u32) -> FaceUv {
    let side = f64::from(PATCH_QUADS << key.level());
    FaceUv {
        face: key.face(),
        u: st_to_uv(f64::from(key.i() * PATCH_QUADS + x) / side),
        v: st_to_uv(f64::from(key.j() * PATCH_QUADS + y) / side),
    }
}

/// The palette slot of `weights`' largest byte, the lowest on a tie, as the client's
/// `dominantSlot` takes it; `None` with no weight.
fn dominant_slot(weights: [u8; crate::material::MAX_CLASSES]) -> Option<usize> {
    let mut best = None;
    let mut largest = 0;
    for (k, &w) in weights.iter().enumerate() {
        if w > largest {
            largest = w;
            best = Some(k);
        }
    }
    best
}

/// R10.T12: the Earth-like fixture's patch holds three classes, each the dominant weight at some
/// vertex; its bake is complete; and at every vertex the dominant weight names the slot of the
/// worker's material there (one synthesiser, `surveyed_material`, as `material` answers it), which
/// `material` itself gives at a vertex of each class.
#[test]
fn worker_earth_fixture_patch_holds_three_classes_its_dominant_weights_name() {
    let f = earth_fixture();
    let mut fields = earth_worker(&f);
    let bake = fields.bake(f.body, SEED, f.patch, &MESH).unwrap();
    let layers = bake.terrain().layers();
    assert!(layers.is_complete());
    let palette = layers.palette();
    let held = fields.field(f.body).unwrap().clone();
    let synth = Synthesiser::new(&held, SEED);
    let mut cache = SynthCache::new();
    let mut dominant = BTreeSet::new();
    let mut sample = BTreeMap::new();
    for (n, weights) in layers.weights().iter().enumerate() {
        let n = u32::try_from(n).unwrap();
        let (x, y) = (n % (PATCH_QUADS + 1), n / (PATCH_QUADS + 1));
        let at = vertex_point(f.patch, x, y);
        assert_eq!(
            unit_dir(face_uv_to_xyz(at)),
            f.patch
                .vertex_dir(u8::try_from(x).unwrap(), u8::try_from(y).unwrap())
        );
        let ground = surveyed_material(&synth, &mut cache, at).unwrap().ground();
        let slot = dominant_slot(*weights).expect("a vertex of a complete bake has weight");
        assert_eq!(palette[slot], Some(ground), "vertex ({x}, {y})");
        dominant.insert(slot);
        sample.entry(slot).or_insert(at);
    }
    assert_eq!(dominant.len(), 3, "{palette:?}");
    for at in sample.into_values() {
        let answer = fields.material(f.body, SEED, at).unwrap().unwrap();
        assert_eq!(
            Some(answer),
            Some(surveyed_material(&synth, &mut cache, at).unwrap())
        );
    }
}

/// The cover the blocks of a payload hold, every block's cells with their codes in turn.
pub(super) fn payload_cover(blocks: &[Vec<u8>]) -> Cover {
    let decoded = decode_payload(&blocks.concat()).expect("the fixture's payloads decode");
    Cover::from_ranges(
        decoded
            .iter()
            .flat_map(|block| block.cover().ranges().iter().copied()),
    )
    .expect("a payload's blocks hold its cells in index order")
}

#[test]
fn worker_terrain_fixture_crosses_a_face_edge_by_a_cube_corner() {
    let f = terrain_fixture();
    let faces = |cells: &Cover| -> BTreeSet<u8> {
        cells
            .cells()
            .map(|index| {
                cell_at_index(world(WORLD).header().level().get(), index)
                    .unwrap()
                    .face()
                    .index()
            })
            .collect()
    };
    // Each survey straddles the edge of faces +x and +y; the margin reaches face +z round their
    // corner.
    assert_eq!(faces(&f.surveys[0]), BTreeSet::from([0, 1]));
    assert_eq!(faces(&f.surveys[1]), BTreeSet::from([0, 1]));
    let held = payload_cover(&f.payloads[0].blocks);
    assert_eq!(faces(&held), BTreeSet::from([0, 1, 2]));
    // The second extends the first: new cells, and some of the first's surveyed again, finer.
    let first: BTreeSet<u32> = f.surveys[0].cells().collect();
    let both: BTreeSet<u32> = f.surveys[1].cells().collect();
    assert!(
        both.len() > first.len() + 4,
        "{} {}",
        both.len(),
        first.len()
    );
    assert!(first.is_subset(&both));
    let finer = first
        .iter()
        .filter(|&&cell| f.surveys[1].code(cell) != f.surveys[0].code(cell))
        .count();
    assert!(
        finer > 0,
        "the second survey passes over some of the first's cells"
    );
}

#[test]
fn worker_terrain_fixture_payloads_are_the_server_s_answers_in_several_blocks() {
    let f = terrain_fixture();
    for payload in &f.payloads[..3] {
        assert!(payload.blocks.len() >= 3, "{}", payload.blocks.len());
        for block in &payload.blocks {
            assert!(block.len() <= FIXTURE_BLOCK_BYTES);
        }
    }
    // An ask at the revision held is block 0 alone, the header, whichever the revision.
    assert_eq!(f.payloads[3].blocks.len(), 1);
    assert!(payload_cover(&f.payloads[3].blocks).is_empty());
    let at_first = split(&encode_within(
        world(WORLD),
        &f.surveys[0],
        Some(&f.surveys[0]),
        SYNTHESIS_MARGIN_CELLS,
        FIXTURE_BLOCK_BYTES,
    ));
    assert_eq!(at_first, f.payloads[3].blocks);
    // The first payload and the delta make the field the whole at revision 2 makes.
    let mut stepped = WorkerFields::new();
    let mut blocks = f.first_then_delta();
    stepped.init(f.body, blocks.next().unwrap()).unwrap();
    for block in blocks {
        stepped.chunk(f.body, block).unwrap();
    }
    let mut whole = WorkerFields::new();
    let mut blocks = f.payloads[2].blocks.iter();
    whole.init(f.body, blocks.next().unwrap()).unwrap();
    for block in blocks {
        whole.chunk(f.body, block).unwrap();
    }
    assert_eq!(stepped.field(f.body), whole.field(f.body));
    // Every surveyed cell of revision 2 is reported along the first payload and the delta.
    let mut fields = WorkerFields::new();
    let mut surveyed = Vec::new();
    let mut blocks = f.first_then_delta();
    surveyed.extend(cells_of(
        &fields.init(f.body, blocks.next().unwrap()).unwrap(),
    ));
    for block in blocks {
        surveyed.extend(cells_of(&fields.chunk(f.body, block).unwrap()));
    }
    surveyed.sort_unstable();
    surveyed.dedup();
    assert_eq!(surveyed, f.surveys[1].cells().collect::<Vec<_>>());
}

/// The patch of `level` holding `cell`'s centre.
pub(super) fn patch_over(cell: PatchKey, level: u8) -> PatchKey {
    PatchKey::containing(level, point(cell, 0.5, 0.5)).unwrap()
}

/// The point at (`x`, `y`) of `cell`, each in [0, 1] across it.
pub(super) fn point(cell: PatchKey, x: f64, y: f64) -> [f64; 3] {
    unit_dir(face_uv_to_xyz(face_uv(cell, x, y)))
}

/// The face coordinates of the point at (`x`, `y`) of `cell`.
pub(super) fn face_uv(cell: PatchKey, x: f64, y: f64) -> FaceUv {
    let side = f64::from(1_u32 << cell.level());
    FaceUv {
        face: cell.face(),
        u: st_to_uv((f64::from(cell.i()) + x) / side),
        v: st_to_uv((f64::from(cell.j()) + y) / side),
    }
}

/// The indices of `cells`, in rising order.
fn indices(cells: &BTreeSet<PatchKey>) -> Vec<u32> {
    let mut indices: Vec<u32> = cells.iter().map(|&c| cell_index(c)).collect();
    indices.sort_unstable();
    indices
}

/// The indices a change's runs hold, in order.
fn cells_of(change: &SurveyChange) -> Vec<u32> {
    change.runs().iter().flat_map(|&(a, b)| a..b).collect()
}

#[test]
fn worker_fixture_payloads_are_several_blocks_each() {
    let f = fixture();
    assert!(f.first_blocks.len() >= 2, "{}", f.first_blocks.len());
    assert!(f.second_blocks.len() >= 2, "{}", f.second_blocks.len());
    for block in f.blocks() {
        assert!(block.len() <= FIXTURE_BLOCK_BYTES);
        assert!(block.len() > BLOCK_HEADER_BYTES);
    }
}

#[test]
fn worker_fields_take_every_block_into_the_field_a_client_builds() {
    let f = fixture();
    let mut fields = WorkerFields::new();
    let mut blocks = f.blocks();
    let mut surveyed = Vec::new();
    surveyed.extend(cells_of(
        &fields.init(f.body, blocks.next().unwrap()).unwrap(),
    ));
    for block in blocks {
        surveyed.extend(cells_of(&fields.chunk(f.body, block).unwrap()));
    }
    assert_eq!(fields.field(f.body), Some(&f.field()));
    assert_eq!(fields.bodies().collect::<Vec<_>>(), vec![f.body]);
    // Every surveyed cell is reported once, by the block that carries it.
    surveyed.sort_unstable();
    let all: BTreeSet<PatchKey> = f.first.union(&f.second).copied().collect();
    assert_eq!(surveyed, indices(&all));
}

#[test]
fn worker_fields_report_each_payloads_surveyed_cells_and_nothing_for_a_block_again() {
    let f = fixture();
    let mut fields = WorkerFields::new();
    let mut first: Vec<u32> = cells_of(&fields.init(f.body, &f.first_blocks[0]).unwrap());
    for block in &f.first_blocks[1..] {
        first.extend(cells_of(&fields.chunk(f.body, block).unwrap()));
    }
    assert_eq!(first, indices(&f.first));
    let mut second = Vec::new();
    for block in &f.second_blocks {
        second.extend(cells_of(&fields.chunk(f.body, block).unwrap()));
    }
    assert_eq!(second, indices(&f.second));
    let held = fields.field(f.body).unwrap().clone();
    for block in f.blocks() {
        assert!(fields.chunk(f.body, block).unwrap().surveys_nothing());
    }
    assert_eq!(fields.field(f.body), Some(&held));
}

#[test]
fn worker_survey_change_answers_by_runs() {
    let mut change = SurveyChange::default();
    for index in [3, 4, 5, 9, 11, 12] {
        push_run(&mut change.runs, index);
    }
    for index in [1, 2, 3, 7] {
        push_run(&mut change.held, index);
    }
    assert_eq!(change.runs(), &[(3, 6), (9, 10), (11, 13)]);
    assert_eq!(change.held_runs(), &[(1, 4), (7, 8)]);
    for index in 0..15 {
        let surveyed = [3, 4, 5, 9, 11, 12].contains(&index);
        assert_eq!(change.surveys(index), surveyed, "{index}");
        assert_eq!(
            change.brings(index),
            [1, 2, 3, 7].contains(&index),
            "{index}"
        );
    }
    assert!(change.surveys_any(&[0, 1, 12]));
    assert!(!change.surveys_any(&[0, 6, 10, 13]));
    assert!(change.brings_any(&[0, 7]));
    assert!(!change.brings_any(&[0, 4, 8]));
    assert!(!SurveyChange::default().surveys_any(&[0, 1]));
    assert!(!SurveyChange::default().brings_any(&[0, 1]));
}

#[test]
fn worker_fields_refuse_a_block_before_its_field_and_hold_nothing() {
    let f = fixture();
    let mut fields = WorkerFields::new();
    assert_eq!(
        fields.chunk(f.body, &f.first_blocks[1]),
        Err(TakeBlockError::NotStarted(f.body))
    );
    assert_eq!(fields.bodies().count(), 0);
    assert_eq!(fields.heap_bytes(), 0);
}

#[test]
fn worker_fields_start_only_from_a_block_zero_of_the_body_posted_for() {
    let f = fixture();
    let mut fields = WorkerFields::new();
    assert_eq!(
        fields.init(f.body, &f.first_blocks[1]),
        Err(TakeBlockError::NoHeader { index: 1 })
    );
    let other = BodyRef::new(f.body.raw_system_id(), f.body.body_index() ^ 1);
    assert_eq!(
        fields.init(other, &f.first_blocks[0]),
        Err(TakeBlockError::WrongBody {
            named: other,
            found: f.body
        })
    );
    assert!(matches!(
        fields.init(f.body, &f.first_blocks[0][..20]),
        Err(TakeBlockError::Decode(_))
    ));
    assert_eq!(fields.bodies().count(), 0);
}

#[test]
fn worker_fields_keep_the_field_as_it_was_after_a_refused_block() {
    let f = fixture();
    let mut fields = f.worker();
    let held = fields.field(f.body).unwrap().clone();
    let other = BodyRef::new(f.body.raw_system_id(), f.body.body_index() ^ 1);
    let mut wrong = f.first_blocks[1].clone();
    let end = wrong.len() - 1;
    wrong.truncate(end);
    assert!(matches!(
        fields.chunk(f.body, &wrong),
        Err(TakeBlockError::Decode(DecodeBlockError::Truncated { .. }))
    ));
    // A block of another field of the body: a block 0 whose header is not the held one's, its
    // realised relief a unit in the last place apart.
    let mut stranger = f.first_blocks[0].clone();
    let relief = bits(held.header().realised_relief().value()).to_le_bytes();
    let at = stranger
        .windows(8)
        .position(|w| w == relief)
        .expect("block 0 carries the header's relief");
    stranger[at] ^= 1;
    assert_eq!(
        fields.chunk(f.body, &stranger),
        Err(TakeBlockError::Insert(InsertBlockError::WrongHeader))
    );
    assert_eq!(fields.field(f.body), Some(&held));
    // A refused start keeps the field held too.
    assert_eq!(
        fields.init(other, &f.first_blocks[0]),
        Err(TakeBlockError::WrongBody {
            named: other,
            found: f.body
        })
    );
    assert_eq!(
        fields.init(f.body, &f.second_blocks[1]),
        Err(TakeBlockError::NoHeader { index: 1 })
    );
    assert_eq!(fields.field(f.body), Some(&held));
}

#[test]
fn worker_fields_refuse_a_block_of_the_body_at_another_level() {
    // The Ceres-like world's field (level 5) and a later block of the Moon-like fixture (level
    // 6), its body set to Ceres's, whose cells index past a level-5 field's.
    let ceres = world(SyntheticWorld::CeresLike);
    let body = ceres.header().body();
    let first = split(&encode_within(
        ceres,
        &cover(
            &[PatchKey::new(Face::PosY, 5, 10, 10).unwrap()]
                .into_iter()
                .collect(),
            |_| code(50.0),
        ),
        None,
        SYNTHESIS_MARGIN_CELLS,
        FIXTURE_BLOCK_BYTES,
    ));
    let mut fields = WorkerFields::new();
    fields.init(body, &first[0]).unwrap();
    let held = fields.field(body).unwrap().clone();
    // A later block surveying a cell of face +z, whose index at level 6 is past level 5's cells.
    let moon = split(&encode_within(
        world(WORLD),
        &cover(
            &[PatchKey::new(Face::PosZ, 6, 10, 10).unwrap()]
                .into_iter()
                .collect(),
            |_| code(50.0),
        ),
        None,
        SYNTHESIS_MARGIN_CELLS,
        FIXTURE_BLOCK_BYTES,
    ));
    let mut deeper = moon[1..]
        .iter()
        .find(|b| {
            let block = decode_block(b).unwrap();
            block
                .cover()
                .ranges()
                .iter()
                .any(|r| r.code().is_surveyed())
        })
        .expect("a later block holds the surveyed cell")
        .clone();
    assert!(
        decode_block(&deeper)
            .unwrap()
            .cover()
            .cells()
            .any(|c| c >= CoarseLevel::new(5).unwrap().cell_count())
    );
    deeper[14..22].copy_from_slice(&body.raw_system_id().to_le_bytes());
    deeper[22..24].copy_from_slice(&body.body_index().to_le_bytes());
    assert_eq!(
        fields.chunk(body, &deeper),
        Err(TakeBlockError::Insert(InsertBlockError::WrongLevel {
            found: CoarseLevel::new(6).unwrap(),
            expected: CoarseLevel::new(5).unwrap(),
        }))
    );
    assert_eq!(fields.field(body), Some(&held));
}

#[test]
fn worker_fields_start_afresh_and_release_to_nothing() {
    let f = fixture();
    let mut fields = f.worker();
    let whole = fields.heap_bytes();
    let level = world(WORLD).header().level();
    let slots = usize::try_from(level.cell_count()).unwrap() * size_of::<Option<SynthesisCell>>();
    assert!(whole >= slots, "{whole} < {slots}");
    // A second start replaces the field, holding the first block alone, and reports what that
    // block surveys as a first start does.
    let change = fields.init(f.body, &f.first_blocks[0]).unwrap();
    let mut fresh = WorkerFields::new();
    assert_eq!(fresh.init(f.body, &f.first_blocks[0]).unwrap(), change);
    assert_eq!(fields.field(f.body), fresh.field(f.body));
    assert!(fields.release(f.body));
    assert!(!fields.release(f.body));
    assert_eq!(fields.bodies().count(), 0);
    assert_eq!(fields.heap_bytes(), 0);
    assert_eq!(fields.level_table(f.body), None);
}

#[test]
fn worker_level_table_bounds_every_patch_of_its_level() {
    let f = fixture();
    let mut fields = WorkerFields::new();
    fields.init(f.body, &f.first_blocks[0]).unwrap();
    let early = fields.level_table(f.body).unwrap();
    let fields = f.worker();
    let table = fields.level_table(f.body).unwrap();
    assert_eq!(table.len(), LEVEL_TABLE_ROWS * LEVEL_TABLE_STRIDE);
    let field = f.field();
    let synth = Synthesiser::new(&field, SEED);
    let a = field.header().figure().equatorial_radius_m;
    for level in 0..=MAX_LEVEL {
        let row = &table[usize::from(level) * LEVEL_TABLE_STRIDE..][..LEVEL_TABLE_STRIDE];
        let n = band(level);
        assert_eq!(bits(row[0]), bits(synth.level_bound_m(n)));
        assert!(row[1] <= 0.0 && row[2] >= 0.0, "{level}: {row:?}");
        assert_eq!(bits(row[3]), bits(vertex_spacing(a, level).max_m));
        // Each surveyed patch's range, R09's, lies inside its level's row.
        for cell in f.first.iter().chain(&f.second) {
            let (low, high) = synth.height_range_m(patch_over(*cell, level));
            assert!(
                row[1] <= low && high <= row[2],
                "{level}: {low} {high} {row:?}"
            );
        }
        // The table only widens as blocks bring cells.
        let before = &early[usize::from(level) * LEVEL_TABLE_STRIDE..][..LEVEL_TABLE_STRIDE];
        assert!(row[1] <= before[1] && before[2] <= row[2]);
    }
}

#[test]
fn worker_level_table_is_the_datum_before_any_cell() {
    let f = fixture();
    let header = decode_payload(&f.first_blocks.concat()).unwrap()[0]
        .header()
        .unwrap()
        .clone();
    let mut fields = WorkerFields::new();
    // A payload with no cell to send is block 0 alone: the empty cover's.
    let empty = split(&encode_within(
        world(WORLD),
        &Cover::from_ranges([]).unwrap(),
        None,
        SYNTHESIS_MARGIN_CELLS,
        FIXTURE_BLOCK_BYTES,
    ));
    assert_eq!(empty.len(), 1);
    assert!(fields.init(f.body, &empty[0]).unwrap().surveys_nothing());
    assert_eq!(fields.field(f.body).unwrap().header(), &header);
    let table = fields.level_table(f.body).unwrap();
    for row in table.chunks(LEVEL_TABLE_STRIDE) {
        assert_eq!((bits(row[1]), bits(row[2])), (bits(0.0), bits(0.0)));
    }
}

/// Patches about the two surveys: inside the first, astride the two, beside the second's far
/// edge, a coarse one over both, and one beyond every held cell.
pub(super) fn probes() -> Vec<(&'static str, PatchKey, TerrainBakeOptions)> {
    let coarse = world(WORLD).header().level().get();
    vec![
        ("inside", patch_over(cell(22, 22), coarse + 2), DOUBLE),
        ("astride", patch_over(cell(23, 22), coarse + 1), MESH),
        (
            "edge",
            PatchKey::new(Face::PosY, coarse - 1, 11, 11).unwrap(),
            MESH,
        ),
        ("coarse", PatchKey::new(Face::PosY, 2, 1, 1).unwrap(), MESH),
        ("depth", patch_over(cell(24, 23), coarse + 1), DEPTH_ONLY),
        ("beyond", patch_over(cell(40, 40), coarse + 1), MESH),
    ]
}

#[test]
fn worker_bake_is_bake_terrain_over_the_field_with_its_unsurveyed_cells() {
    let f = fixture();
    let mut fields = f.worker();
    let field = f.field();
    let synth = Synthesiser::new(&field, SEED);
    let coarse = field.header().level().get();
    for (name, key, opts) in probes() {
        let bake = fields.bake(f.body, SEED, key, &opts).unwrap();
        let alone = bake_terrain(&synth, &mut SynthCache::new(), key, &opts, &[]).unwrap();
        assert_eq!(bake.terrain(), &alone, "{name}");
        let mask = alone.layers().survey_mask();
        let mut unsurveyed = BTreeSet::new();
        for y in 0..=64_u8 {
            for x in 0..=64_u8 {
                let n = usize::from(y) * 65 + usize::from(x);
                let cell = PatchKey::containing(coarse, key.vertex_dir(x, y)).unwrap();
                assert_eq!(mask[n] == SURVEYED, field.is_surveyed(cell));
                if mask[n] == UNSURVEYED {
                    unsurveyed.insert(cell_index(cell));
                }
            }
        }
        let unsurveyed: Vec<u32> = unsurveyed.into_iter().collect();
        assert_eq!(bake.unsurveyed_cells(), unsurveyed.as_slice(), "{name}");
    }
    let inside = fields.bake(f.body, SEED, probes()[0].1, &DOUBLE).unwrap();
    assert_eq!(inside.unsurveyed_cells(), [] as [u32; 0]);
    assert!(inside.terrain().layers().is_complete());
}

#[test]
fn worker_bake_and_queries_need_the_body_s_field() {
    let f = fixture();
    let mut fields = f.worker();
    let other = BodyRef::new(f.body.raw_system_id(), f.body.body_index() ^ 1);
    let at = face_uv(cell(22, 22), 0.5, 0.5);
    let none = Err(QueryBodyError::NotStarted(other));
    assert_eq!(
        fields.bake(other, SEED, probes()[0].1, &MESH).map(|_| ()),
        none
    );
    assert_eq!(
        fields
            .reading(other, SEED, at, Metres::new(2.0))
            .map(|_| ()),
        none
    );
    assert_eq!(fields.material(other, SEED, at).map(|_| ()), none);
    assert_eq!(
        fields
            .slant_range(
                other,
                SEED,
                &[0.0, 2e6, 0.0],
                &[0.0, -1.0, 0.0],
                Metres::new(1e7)
            )
            .map(|_| ()),
        none
    );
}

#[test]
fn worker_bake_of_a_survey_without_its_margin_waits_for_the_margin() {
    let f = fixture();
    // Each later block of the first payload with block 0 alone, so that a survey's cells can be
    // held while the margin about them, in other blocks, is not.
    let mut waited = false;
    for block in &f.first_blocks[1..] {
        let mut fields = WorkerFields::new();
        fields.init(f.body, &f.first_blocks[0]).unwrap();
        fields.chunk(f.body, block).unwrap();
        for (name, key, opts) in probes() {
            match fields.bake(f.body, SEED, key, &opts) {
                Ok(_) => {}
                Err(QueryBodyError::NotSurveyed(cell)) => {
                    assert!(fields.field(f.body).unwrap().cell(cell).is_none(), "{name}");
                    waited = true;
                }
                Err(e) => panic!("{name}: {e}"),
            }
        }
    }
    assert!(waited, "some survey's bake reads a margin still to come");
    let mut fields = WorkerFields::new();
    fields.init(f.body, &f.first_blocks[0]).unwrap();
    for block in &f.first_blocks[1..] {
        fields.chunk(f.body, block).unwrap();
    }
    for (name, key, opts) in probes() {
        assert!(fields.bake(f.body, SEED, key, &opts).is_ok(), "{name}");
    }
}

#[test]
fn worker_rebake_rule_misses_no_bake_a_block_changes() {
    let f = fixture();
    let mut fields = WorkerFields::new();
    fields.init(f.body, &f.first_blocks[0]).unwrap();
    for block in &f.first_blocks[1..] {
        fields.chunk(f.body, block).unwrap();
    }
    let before: Vec<BodyBake> = probes()
        .into_iter()
        .map(|(_, key, opts)| fields.bake(f.body, SEED, key, &opts).unwrap())
        .collect();
    let mut changed = [false; 6];
    for block in &f.second_blocks {
        let change = fields.chunk(f.body, block).unwrap();
        for (n, old) in before.iter().enumerate() {
            changed[n] |= old.needs_rebake(&change);
        }
    }
    let mut rebaked = 0;
    for (n, ((name, key, opts), old)) in probes().into_iter().zip(&before).enumerate() {
        let new = fields.bake(f.body, SEED, key, &opts).unwrap();
        if !changed[n] {
            assert_eq!(new.terrain(), old.terrain(), "{name} changed, unflagged");
        }
        rebaked += usize::from(changed[n]);
        if name == "inside" {
            assert!(!changed[n], "a patch surveyed whole is never rebaked");
        }
        if name == "astride" || name == "depth" {
            assert!(changed[n], "{name}: the second survey changes its mask");
            assert_ne!(
                new.terrain().layers().survey_mask(),
                old.terrain().layers().survey_mask(),
                "{name}"
            );
        }
        if name == "beyond" {
            assert!(!old.terrain().layers().is_complete());
            assert!(changed[n], "an incomplete bake is rebaked on any block");
        }
    }
    assert!(rebaked < probes().len());
}

/// `opts` with the horizon map, as the lit style's workers bake (R10.T8.b).
pub(super) fn lit(opts: TerrainBakeOptions) -> TerrainBakeOptions {
    TerrainBakeOptions {
        horizon: HorizonLayer::Baked,
        ..opts
    }
}

/// A worker holding the fixture's first payload alone.
fn first_survey(f: &Fixture) -> WorkerFields {
    let mut fields = WorkerFields::new();
    fields.init(f.body, &f.first_blocks[0]).unwrap();
    for block in &f.first_blocks[1..] {
        fields.chunk(f.body, block).unwrap();
    }
    fields
}

/// The blocks of a delta payload from the fixture's first survey to it and `cells` more, surveyed
/// at 20 m.
fn delta_blocks(cells: &BTreeSet<PatchKey>) -> Vec<Vec<u8>> {
    let f = fixture();
    let first_cover = cover(&f.first, |_| code(50.0));
    let both: BTreeSet<PatchKey> = f.first.union(cells).copied().collect();
    let both_cover = cover(&both, |c| {
        if f.first.contains(&c) {
            code(50.0)
        } else {
            code(20.0)
        }
    });
    split(&encode_within(
        world(WORLD),
        &both_cover,
        Some(&first_cover),
        SYNTHESIS_MARGIN_CELLS,
        FIXTURE_BLOCK_BYTES,
    ))
}

/// The probes baked with the map over a worker holding the first survey, each block of `blocks`
/// posted after them, and the bakes again: each probe's name, its bake before and after, and
/// whether the rule took it on some block; every bake the rule passes over is the same bytes after
/// the blocks, its horizon map's words and refused cells included.
fn rule_over(blocks: &[Vec<u8>]) -> Vec<(&'static str, BodyBake, BodyBake, bool)> {
    let f = fixture();
    let mut fields = first_survey(&f);
    let before: Vec<BodyBake> = probes()
        .into_iter()
        .map(|(_, key, opts)| fields.bake(f.body, SEED, key, &lit(opts)).unwrap())
        .collect();
    let mut taken = vec![false; before.len()];
    for block in blocks {
        let change = fields.chunk(f.body, block).unwrap();
        for (n, old) in before.iter().enumerate() {
            taken[n] |= old.needs_rebake(&change);
        }
    }
    probes()
        .into_iter()
        .zip(before)
        .zip(taken)
        .map(|(((name, key, opts), old), taken)| {
            let new = fields.bake(f.body, SEED, key, &lit(opts)).unwrap();
            if !taken {
                assert_eq!(new, old, "{name} changed, unflagged");
            }
            (name, old, new, taken)
        })
        .collect()
}

#[test]
fn worker_fields_report_the_cells_each_block_brings() {
    let f = fixture();
    let mut fields = WorkerFields::new();
    let mut held = BTreeSet::new();
    let mut blocks = f.blocks();
    let first = fields
        .init(f.body, blocks.next().expect("a payload has a block 0"))
        .unwrap();
    let mut changes = vec![first];
    for block in blocks {
        changes.push(fields.chunk(f.body, block).unwrap());
    }
    for (block, change) in f.blocks().zip(&changes) {
        let carried: BTreeSet<u32> = decode_block(block).unwrap().cover().cells().collect();
        let brought: Vec<u32> = change.held_runs().iter().flat_map(|&(a, b)| a..b).collect();
        // Exactly the block's cells not held before it, each once and rising.
        let expected: Vec<u32> = carried.difference(&held).copied().collect();
        assert_eq!(brought, expected);
        assert!(brought.iter().all(|&i| change.brings(i)));
        assert!(change.brings_any(&brought) || brought.is_empty());
        // A cell surveyed anew that was not held is brought; one held as margin is not.
        for i in cells_of(change) {
            assert_eq!(change.brings(i), !held.contains(&i), "{i}");
        }
        held.extend(carried);
    }
    // A block again brings nothing.
    let again = fields.chunk(f.body, &f.second_blocks[0]).unwrap();
    assert_eq!(again, SurveyChange::default());
}

#[test]
fn worker_rebake_rule_passes_over_a_block_elsewhere() {
    // A survey on the far face, beyond every probe's reach.
    let elsewhere: BTreeSet<PatchKey> =
        [PatchKey::new(Face::NegY, world(WORLD).header().level().get(), 30, 30).unwrap()]
            .into_iter()
            .collect();
    let rule = rule_over(&delta_blocks(&elsewhere));
    for (name, old, _, taken) in &rule {
        let layers = old.terrain().layers();
        // An incomplete bake is taken on any block; a complete one only by a cell it names.
        assert_eq!(*taken, !layers.is_complete(), "{name}");
        if *name == "inside" {
            assert!(
                layers.is_complete() && !layers.horizon().is_complete(),
                "{name}"
            );
            assert!(!layers.horizon().refused_cells().is_empty(), "{name}");
        }
    }
    assert!(
        rule.iter().any(|(_, _, _, taken)| !taken),
        "a bake is passed over"
    );
}

#[test]
fn worker_rebake_rule_takes_a_block_that_brings_a_refused_cell_as_margin() {
    let f = fixture();
    let mut fields = first_survey(&f);
    let (_, key, opts) = probes()[0];
    let inside = fields.bake(f.body, SEED, key, &lit(opts)).unwrap();
    let refused = inside.terrain().layers().horizon().refused_cells().to_vec();
    let field = fields.field(f.body).unwrap();
    let level = field.header().level().get();
    let named = |c: PatchKey| refused.binary_search(&cell_index(c)).is_ok();
    // A cell the field does not hold and the map does not name, within a margin of one it does.
    let margin = i64::from(SYNTHESIS_MARGIN_CELLS);
    let side = i64::from(1_u32 << level);
    let (r, c) = refused
        .iter()
        .map(|&i| cell_at_index(level, i).unwrap())
        .find_map(|r| {
            let (ri, rj) = (i64::from(r.i()), i64::from(r.j()));
            (-margin..=margin)
                .flat_map(|di| (-margin..=margin).map(move |dj| (ri + di, rj + dj)))
                .filter(|&(i, j)| (0..side).contains(&i) && (0..side).contains(&j))
                .map(|(i, j)| {
                    PatchKey::new(
                        r.face(),
                        level,
                        u32::try_from(i).unwrap(),
                        u32::try_from(j).unwrap(),
                    )
                    .unwrap()
                })
                .find(|&c| field.resolution(c).is_none() && !named(c))
                .map(|c| (r, c))
        })
        .expect("a refused cell lies within a margin of ground the map does not name");
    let blocks = delta_blocks(&[c].into_iter().collect());
    let mut brought_as_margin = false;
    for block in &blocks {
        let change = fields.chunk(f.body, block).unwrap();
        brought_as_margin |= change.brings(cell_index(r)) && !change.surveys(cell_index(r));
        assert!(
            !change.surveys_any(&refused),
            "no refused cell is surveyed by the block"
        );
    }
    assert!(brought_as_margin, "{r:?} comes as {c:?}'s margin");
    let rule = rule_over(&blocks);
    let (_, old, new, taken) = &rule[0];
    assert!(*taken, "a refused cell brought as margin takes the bake");
    assert!(
        new.terrain()
            .layers()
            .horizon()
            .refused_cells()
            .binary_search(&cell_index(r))
            .is_err(),
        "the cell brought is not named again"
    );
    assert_eq!(old, &inside);
}

#[test]
fn worker_rebake_rule_takes_a_block_that_surveys_a_refused_cell() {
    let f = fixture();
    let mut fields = first_survey(&f);
    let (_, key, opts) = probes()[0];
    let inside = fields.bake(f.body, SEED, key, &lit(opts)).unwrap();
    let refused = inside.terrain().layers().horizon().refused_cells();
    let level = fields.field(f.body).unwrap().header().level().get();
    let r = cell_at_index(level, refused[refused.len() / 2]).unwrap();
    let blocks = delta_blocks(&[r].into_iter().collect());
    let rule = rule_over(&blocks);
    let (name, old, new, taken) = &rule[0];
    assert!(*taken, "{name}: a refused cell surveyed takes the bake");
    assert_eq!(
        old.unsurveyed_cells(),
        new.unsurveyed_cells(),
        "{name}'s mask is whole"
    );
    assert!(
        new.terrain()
            .layers()
            .horizon()
            .refused_cells()
            .binary_search(&cell_index(r))
            .is_err(),
        "{name}: the cell surveyed is not named again"
    );
}

#[test]
fn worker_rebake_rule_misses_no_bake_the_second_survey_changes_with_the_map() {
    let f = fixture();
    let rule = rule_over(&f.second_blocks);
    for (name, old, new, taken) in &rule {
        let layers = old.terrain().layers();
        if !layers.is_complete() {
            assert!(*taken, "{name}: an incomplete bake is rebaked on any block");
        }
        if !taken {
            assert_eq!(new, old, "{name}");
        }
        if *name == "astride" {
            assert!(*taken, "{name}: the second survey changes its mask");
        }
    }
}

#[test]
fn worker_queries_are_the_free_functions_over_the_field() {
    let f = fixture();
    let mut fields = f.worker();
    let field = f.field();
    let synth = Synthesiser::new(&field, SEED);
    let points = [
        face_uv(cell(22, 22), 0.3, 0.6),
        face_uv(cell(25, 23), 0.7, 0.2),
        // In the margin, held but not surveyed.
        face_uv(cell(20, 22), 0.5, 0.5),
        // Not held.
        face_uv(cell(40, 40), 0.5, 0.5),
    ];
    // Twice, the second time from the cache the first filled, in the other order.
    for pass in 0..2 {
        let order: Vec<FaceUv> = if pass == 0 {
            points.to_vec()
        } else {
            points.iter().rev().copied().collect()
        };
        for at in order {
            for baseline in [2.0, 100.0] {
                let b = Metres::new(baseline);
                assert_eq!(
                    fields.reading(f.body, SEED, at, b),
                    Ok(surveyed_reading(&synth, &mut SynthCache::new(), at, b).unwrap())
                );
            }
            let expected = if field.is_surveyed(
                PatchKey::containing(field.header().level().get(), unit_dir(face_uv_to_xyz(at)))
                    .unwrap(),
            ) {
                let ground = material_at(&synth, &mut SynthCache::new(), at).unwrap();
                let properties = ground
                    .material()
                    .map(|m| properties(m, field.header().palette()).unwrap());
                // The fixture's header carries no maturity, so the point has none, and its
                // properties are the palette's as resolved (R10.T1.c's identity).
                Some(SurveyedMaterial {
                    ground,
                    maturity: None,
                    properties,
                })
            } else {
                None
            };
            assert_eq!(fields.material(f.body, SEED, at), Ok(expected));
        }
    }
    assert!(
        fields
            .reading(f.body, SEED, points[0], Metres::new(2.0))
            .unwrap()
            .is_some()
    );
    assert!(
        fields
            .reading(f.body, SEED, points[2], Metres::new(2.0))
            .unwrap()
            .is_none()
    );
    assert!(fields.material(f.body, SEED, points[1]).unwrap().is_some());
    assert!(fields.material(f.body, SEED, points[2]).unwrap().is_none());
    for (origin, direction) in sight_lines(&field) {
        assert_eq!(
            fields.slant_range(f.body, SEED, &origin, &direction, Metres::new(1e7)),
            Ok(slant_range(
                &synth,
                &mut SynthCache::new(),
                &origin,
                &direction,
                Metres::new(1e7)
            )
            .unwrap())
        );
    }
}

/// Lines of sight from 20 km above the first survey: straight down onto it, low over the second
/// onto the margin beyond it, and up and away.
pub(super) fn sight_lines(field: &PartialField) -> [([f64; 3], [f64; 3]); 3] {
    let figure = field.header().figure();
    let over = point(cell(22, 22), 0.5, 0.5);
    let origin = figure.surface_point(over, 20_000.0);
    let far = figure.surface_point(point(cell(28, 22), 0.5, 0.5), -2_000.0);
    let down = origin.map(|x| -x);
    let across = [far[0] - origin[0], far[1] - origin[1], far[2] - origin[2]];
    [(origin, down), (origin, across), (origin, origin)]
}

#[test]
fn worker_lines_of_sight_meet_the_survey_leave_it_and_miss() {
    let f = fixture();
    let mut fields = f.worker();
    let field = f.field();
    let [down, across, up] = sight_lines(&field);
    let max = Metres::new(1e7);
    assert!(matches!(
        fields.slant_range(f.body, SEED, &down.0, &down.1, max),
        Ok(LineOfSight::Ground(_))
    ));
    assert_eq!(
        fields.slant_range(f.body, SEED, &across.0, &across.1, max),
        Ok(LineOfSight::NotSurveyed)
    );
    assert_eq!(
        fields.slant_range(f.body, SEED, &up.0, &up.1, max),
        Ok(LineOfSight::NoGround)
    );
}

/// One of the worker's queries, for the order checks: a probe's bake, a reading at a point and
/// baseline, a line of sight, a material at a point, the class map, the normal map, or a ground
/// readout along a line of sight over a baseline.
#[derive(Debug, Clone, Copy)]
enum Query {
    Bake(usize),
    Reading(usize, f64),
    Sight(usize),
    Material(usize),
    ClassMap,
    NormalMap,
    Readout(usize, SlopeBaseline),
}

/// A [`Query`]'s answer; the normal map's by its bits ([`NormalMapBits`]).
#[derive(Debug, PartialEq)]
enum Answer {
    Bake(Box<Result<BodyBake, QueryBodyError>>),
    Reading(Result<Option<SurveyedReading>, QueryBodyError>),
    Sight(Result<LineOfSight, QueryBodyError>),
    Material(Result<Option<SurveyedMaterial>, QueryBodyError>),
    ClassMap(Result<ClassMap, QueryBodyError>),
    NormalMap(Result<NormalMapBits, QueryBodyError>),
    Readout(Result<GroundReadout, QueryBodyError>),
}

/// The points the order checks read: in either survey, in a margin and not held.
fn query_points() -> [FaceUv; 4] {
    [
        face_uv(cell(22, 22), 0.3, 0.6),
        face_uv(cell(25, 23), 0.7, 0.2),
        face_uv(cell(20, 22), 0.5, 0.5),
        face_uv(cell(40, 40), 0.5, 0.5),
    ]
}

/// Every kind of query, interleaved, the class map after every second round of the others and the
/// normal map after each round between.
fn queries() -> Vec<Query> {
    let mut queries = Vec::new();
    for n in 0..probes().len() {
        queries.push(Query::Bake(n));
        queries.push(Query::Reading(n % 4, if n % 2 == 0 { 2.0 } else { 100.0 }));
        queries.push(Query::Sight(n % 3));
        queries.push(Query::Material(n % 4));
        // Sight 0, the one that meets ground, takes the even rounds and with them every baseline,
        // so the readouts that read its 2 m slope a second time (at 5 m and 100 m) run too; sights
        // 1 and 2 share the odd rounds.
        let sight = if n % 2 == 0 { 0 } else { 1 + (n / 2) % 2 };
        queries.push(Query::Readout(sight, SlopeBaseline::ALL[(n / 2) % 3]));
        queries.push(if n % 2 == 0 {
            Query::ClassMap
        } else {
            Query::NormalMap
        });
    }
    queries
}

/// `query`'s answer from `fields`, with the lines of sight `sights`.
fn ask(
    fields: &mut WorkerFields,
    body: BodyRef,
    sights: &[([f64; 3], [f64; 3])],
    q: Query,
) -> Answer {
    match q {
        Query::Bake(n) => {
            let (_, key, opts) = probes()[n];
            Answer::Bake(Box::new(fields.bake(body, SEED, key, &opts)))
        }
        Query::Reading(n, b) => {
            Answer::Reading(fields.reading(body, SEED, query_points()[n], Metres::new(b)))
        }
        Query::Sight(n) => {
            let (origin, direction) = sights[n];
            Answer::Sight(fields.slant_range(body, SEED, &origin, &direction, Metres::new(1e7)))
        }
        Query::Material(n) => Answer::Material(fields.material(body, SEED, query_points()[n])),
        Query::ClassMap => Answer::ClassMap(fields.class_map(body, SEED)),
        Query::NormalMap => Answer::NormalMap(
            fields
                .normal_map(body, SEED)
                .map(|map| normal_map_bits(&map)),
        ),
        Query::Readout(n, slope_baseline) => {
            let (origin_m, direction) = sights[n];
            let query = GroundReadoutQuery {
                origin_m,
                direction,
                max: Metres::new(1e7),
                slope_baseline,
                pad_width: Metres::new(1.0),
                season: Some(SeasonAnomaly::try_from(Radians::new(1.0)).unwrap()),
            };
            Answer::Readout(fields.ground_readout(body, SEED, &query))
        }
    }
}

#[test]
fn worker_answers_are_the_same_in_any_order_from_the_body_s_cache_and_across_blocks() {
    let f = fixture();
    let sights = sight_lines(&f.field());
    let mut first = WorkerFields::new();
    first.init(f.body, &f.first_blocks[0]).unwrap();
    for block in &f.first_blocks[1..] {
        first.chunk(f.body, block).unwrap();
    }
    // Every kind of query shares the body's cache, in any order.
    let fields = std::cell::RefCell::new(first);
    assert_order_independent(&queries(), |&q| {
        ask(&mut fields.borrow_mut(), f.body, &sights, q)
    });
    // The cache warmed under the first survey serves the field the second's blocks grew: each
    // answer is a fresh worker's, whose cache is empty.
    let mut fields = fields.into_inner();
    for block in &f.second_blocks {
        fields.chunk(f.body, block).unwrap();
    }
    for q in queries() {
        let warm = ask(&mut fields, f.body, &sights, q);
        let cold = ask(&mut f.worker(), f.body, &sights, q);
        assert_eq!(warm, cold, "{q:?}");
    }
}

/// One of the queries of the weathered fixture's order check (R10.T1.c's client half): a patch's
/// bake with its weights and their maturity layer, a material at a point, the class map with its
/// texels' maturities and its classes' nodes, or a material's nodes at a band level.
#[derive(Debug, Clone, Copy)]
enum WeatheredQuery {
    Bake(PatchKey, TerrainBakeOptions),
    Material(FaceUv),
    ClassMap,
    Nodes(Material, u8),
}

/// A [`WeatheredQuery`]'s answer.
#[derive(Debug, PartialEq)]
enum WeatheredAnswer {
    Bake(Box<Result<BodyBake, QueryBodyError>>),
    Material(Result<Option<SurveyedMaterial>, QueryBodyError>),
    ClassMap(Result<ClassMap, QueryBodyError>),
    Nodes(Box<Result<DrawnNodes, ResolvePhotometryError>>),
}

/// `q`'s answer on `body` from `fields`.
fn ask_weathered(fields: &mut WorkerFields, body: BodyRef, q: WeatheredQuery) -> WeatheredAnswer {
    match q {
        WeatheredQuery::Bake(key, opts) => {
            WeatheredAnswer::Bake(Box::new(fields.bake(body, SEED, key, &opts)))
        }
        WeatheredQuery::Material(at) => WeatheredAnswer::Material(fields.material(body, SEED, at)),
        WeatheredQuery::ClassMap => WeatheredAnswer::ClassMap(fields.class_map(body, SEED)),
        WeatheredQuery::Nodes(material, level) => {
            WeatheredAnswer::Nodes(Box::new(fields.maturity_nodes(body, material, band(level))))
        }
    }
}

/// The order check on `TEST MOON` weathered, whose header carries a maturity and a rate (R10.T1.c's
/// client half): its bakes with their maturity layers, its materials, its class map and its nodes
/// at several levels share the body's caches (the synthesis's, the unresolved terms and each
/// material's drawn albedo) in any order, and each answer is a fresh worker's, whose caches are
/// empty.
#[test]
fn worker_weathered_answers_are_the_same_in_any_order_and_from_a_fresh_worker() {
    let field = weathered_terrain_world();
    let body = field.header().body();
    let dir = unit_dir([0.3, 1.0, 0.2]);
    let coarse = field.header().level().get();
    let limit = band_limit_level(field);
    let (warm, cells) = surveyed_about(field, dir);
    let inside = *cells.first().expect("the survey holds four cells");
    let regolith = crust_regolith();
    let melt = Material::new(MaterialClass::Melt, 0).unwrap();
    let queries = [
        WeatheredQuery::Nodes(regolith, limit),
        WeatheredQuery::Bake(PatchKey::containing(limit, dir).unwrap(), MESH),
        WeatheredQuery::Material(face_uv(inside, 0.3, 0.6)),
        WeatheredQuery::ClassMap,
        WeatheredQuery::Nodes(melt, 9),
        WeatheredQuery::Bake(PatchKey::containing(coarse + 2, dir).unwrap(), DOUBLE),
        WeatheredQuery::Nodes(regolith, 0),
        WeatheredQuery::Material(face_uv(inside, 0.8, 0.1)),
        WeatheredQuery::Nodes(melt, limit + 1),
    ];
    let warm = std::cell::RefCell::new(warm);
    assert_order_independent(&queries, |&q| {
        ask_weathered(&mut warm.borrow_mut(), body, q)
    });
    let mut warm = warm.into_inner();
    for q in queries {
        let (mut cold, _) = surveyed_about(field, dir);
        assert_eq!(
            ask_weathered(&mut warm, body, q),
            ask_weathered(&mut cold, body, q),
            "{q:?}"
        );
    }
    // The answers compared carry the maturity: the band limit's bake its layer, the texels theirs.
    let key = PatchKey::containing(limit, dir).unwrap();
    let bake = warm.bake(body, SEED, key, &MESH).unwrap();
    assert_eq!(bake.terrain().layers().maturity().len(), 65 * 65);
    let map = warm.class_map(body, SEED).unwrap();
    assert!(map.texels().iter().all(|t| t.maturity().is_some()));
}

/// R10.T10.b: a material's drawn photometry is its properties over the body's palette under the
/// header's albedo scale, 1 where the header holds none, with its law's L(0) at the scaled V; a
/// consolidated form draws Lambert's law whatever its entry; and a body not held, or an entry past
/// the palette, is refused.
#[test]
fn worker_photometry_is_the_palette_s_properties_under_the_field_s_scale() {
    let f = fixture();
    let unscaled = f.worker();
    // The same world with a scale in its header, started from its first payload's block 0 alone:
    // the photometry reads the header, nothing surveyed.
    let field = world(WORLD).clone().with_albedo_scale(Some(0.73)).unwrap();
    let header_block = split(&encode_within(
        &field,
        &cover(&f.first, |_| code(50.0)),
        None,
        SYNTHESIS_MARGIN_CELLS,
        FIXTURE_BLOCK_BYTES,
    ))
    .swap_remove(0);
    assert_eq!(
        unscaled.field(f.body).unwrap().header().albedo_scale(),
        None,
        "the fixture's header holds no scale, so its materials take c = 1"
    );
    let mut scaled = WorkerFields::new();
    scaled.init(f.body, &header_block).unwrap();
    let palette = field.header().palette();
    let entries = u8::try_from(palette.len()).unwrap();
    assert!(entries > 0, "the Moon-like palette names substances");
    let mut moved = false;
    for (fields, c, is_scaled) in [(&unscaled, 1.0, false), (&scaled, 0.73, true)] {
        for entry in 0..entries {
            for class in [
                MaterialClass::Regolith,
                MaterialClass::Frost,
                MaterialClass::Glacier,
            ] {
                let material = Material::new(class, entry).unwrap();
                let resolved = properties(material, palette).unwrap();
                let drawn = fields.photometry(f.body, material).unwrap();
                let bvr = scaled_normal_albedo(&resolved, c);
                assert_eq!(drawn.normal_albedo_bvr().map(bits), bvr.map(bits));
                assert_eq!(drawn.phase_law(), resolved.phase_law());
                assert_eq!(
                    bits(drawn.limb_parameter()),
                    bits(resolved.phase_law().limb_parameter(bvr[1]))
                );
                if class == MaterialClass::Glacier {
                    assert_eq!(drawn.phase_law(), PhaseLaw::Lambert);
                    assert_eq!(bits(drawn.limb_parameter()), bits(0.0));
                }
                moved |= is_scaled && bvr.map(bits) != resolved.normal_albedo_bvr().map(bits);
            }
        }
    }
    assert!(moved, "the scale moves some material's albedo");
    let other = BodyRef::new(f.body.raw_system_id(), f.body.body_index() ^ 1);
    let material = Material::new(MaterialClass::Regolith, 0).unwrap();
    assert_eq!(
        unscaled.photometry(other, material),
        Err(ResolvePhotometryError::NotStarted(other))
    );
    let past = Material::new(MaterialClass::Regolith, entries).unwrap();
    assert_eq!(
        unscaled.photometry(f.body, past),
        Err(ResolvePhotometryError::Properties(
            ResolvePropertiesError::EntryNotInPalette {
                entry: entries,
                len: palette.len()
            }
        ))
    );
}

/// The class map's texel (`i`, `j`) of face +y.
fn texel(i: u32, j: u32) -> PatchKey {
    PatchKey::new(Face::PosY, CLASS_MAP_LEVEL, i, j).unwrap()
}

#[test]
fn worker_class_map_holds_each_surveyed_cell_with_the_weights_at_its_centre() {
    let f = fixture();
    let mut fields = f.worker();
    let field = f.field();
    let synth = Synthesiser::new(&field, SEED);
    let map = fields.class_map(f.body, SEED).unwrap();
    assert_eq!(
        map.field_level(),
        CLASS_MAP_LEVEL,
        "the Moon-like field is at the map's level, a texel a cell"
    );
    assert!(map.is_complete());
    // No texel for a margin cell or one not held: the surveyed cells, each once, rising.
    let surveyed: BTreeSet<PatchKey> = f.first.union(&f.second).copied().collect();
    assert_eq!(
        map.texels()
            .iter()
            .map(|t| cell_index(t.cell()))
            .collect::<Vec<_>>(),
        indices(&surveyed)
    );
    let classes: Vec<GroundMaterial> = map.classes().iter().map(ClassMapClass::material).collect();
    assert_ne!(classes, []);
    assert!(
        classes.len() <= CLASS_MAP_CLASSES,
        "{} classes",
        classes.len()
    );
    assert!(
        classes.windows(2).all(|w| w[0] < w[1]),
        "the classes once each, in GroundMaterial's order: {classes:?}"
    );
    let band = BandLevel::new(CLASS_MAP_BAND).unwrap();
    for t in map.texels() {
        assert_eq!((t.surveyed_cells(), t.cells()), (1, 1));
        assert_eq!(bits(t.surveyed_fraction()), bits(1.0));
        let at = face_uv(t.cell(), 0.5, 0.5);
        let expected = weights_at(&synth, &mut SynthCache::new(), at, band).unwrap();
        let mut bytes = [0_u8; CLASS_MAP_CLASSES];
        for (material, byte) in expected.iter() {
            let k = classes
                .iter()
                .position(|&c| c == material)
                .expect("every material a texel's weights name is a class");
            bytes[k] = byte;
        }
        assert_eq!(t.weights(), bytes, "texel {:?}", t.cell());
    }
    for class in map.classes() {
        assert_eq!(
            class.photometry(),
            class
                .material()
                .material()
                .map(|m| fields.photometry(f.body, m).unwrap()),
            "{:?}",
            class.material()
        );
    }
    // From the body's cache again, the same map.
    assert_eq!(fields.class_map(f.body, SEED).unwrap(), map);
}

#[test]
fn worker_class_map_needs_the_body_s_field() {
    let f = fixture();
    let mut fields = f.worker();
    let other = BodyRef::new(f.body.raw_system_id(), f.body.body_index() ^ 1);
    assert_eq!(
        fields.class_map(other, SEED),
        Err(QueryBodyError::NotStarted(other))
    );
}

#[test]
fn worker_class_map_of_a_survey_without_its_margin_waits_for_the_margin() {
    let f = fixture();
    let mut fresh = WorkerFields::new();
    fresh.init(f.body, &f.first_blocks[0]).unwrap();
    for block in &f.first_blocks[1..] {
        fresh.chunk(f.body, block).unwrap();
    }
    let full = fresh.class_map(f.body, SEED).unwrap();
    assert!(full.is_complete());
    let full_texels: BTreeSet<PatchKey> = full.texels().iter().map(ClassMapTexel::cell).collect();
    // Each later block of the first payload with block 0 alone, so that a texel's cell can be held
    // surveyed while the margin its weights read, in other blocks, is not.
    let mut waited = false;
    for (k, block) in f.first_blocks.iter().enumerate().skip(1) {
        let mut fields = WorkerFields::new();
        fields.init(f.body, &f.first_blocks[0]).unwrap();
        fields.chunk(f.body, block).unwrap();
        let partial = fields.class_map(f.body, SEED).unwrap();
        if partial.is_complete() {
            continue;
        }
        let held: BTreeSet<PatchKey> = partial.texels().iter().map(ClassMapTexel::cell).collect();
        let field = fields.field(f.body).unwrap();
        let waiting = *full_texels
            .iter()
            .find(|&&t| field.is_surveyed(t) && !held.contains(&t))
            .unwrap_or_else(|| panic!("block {k}: an incomplete map leaves out a surveyed texel"));
        // The rest of the payload: the map is whole, holds the texel and is a fresh worker's.
        for (n, rest) in f.first_blocks.iter().enumerate().skip(1) {
            if n != k {
                fields.chunk(f.body, rest).unwrap();
            }
        }
        let whole = fields.class_map(f.body, SEED).unwrap();
        assert!(whole.is_complete(), "block {k}");
        assert!(
            whole.texels().iter().any(|t| t.cell() == waiting),
            "block {k}: {waiting:?}"
        );
        assert_eq!(whole, full, "block {k}");
        waited = true;
    }
    assert!(waited, "some texel's weights read a margin still to come");
}

#[test]
fn worker_class_map_weighs_a_partly_surveyed_texel_by_its_surveyed_cells() {
    // The Mars-like world, at level 7: four cells a texel.
    let mars = world(SyntheticWorld::MarsLike);
    let level = mars.header().level().get();
    assert_eq!(level, CLASS_MAP_LEVEL + 1);
    let whole = texel(20, 20);
    let part = texel(22, 20);
    let centre_cell = PatchKey::containing(level, point(part, 0.5, 0.5)).unwrap();
    let mut cells: BTreeSet<PatchKey> = whole.children().into_iter().collect();
    // Three of the part's four, not the one holding its centre.
    cells.extend(part.children().into_iter().filter(|&c| c != centre_cell));
    assert_eq!(cells.len(), 7);
    let blocks = split(&encode_within(
        mars,
        &cover(&cells, |_| code(50.0)),
        None,
        SYNTHESIS_MARGIN_CELLS,
        FIXTURE_BLOCK_BYTES,
    ));
    let body = mars.header().body();
    let mut fields = WorkerFields::new();
    fields.init(body, &blocks[0]).unwrap();
    for block in &blocks[1..] {
        fields.chunk(body, block).unwrap();
    }
    let map = fields.class_map(body, SEED).unwrap();
    assert_eq!(map.field_level(), level);
    assert!(map.is_complete());
    let texels: Vec<PatchKey> = map.texels().iter().map(ClassMapTexel::cell).collect();
    assert_eq!(texels, [whole, part]);
    let [w, p] = [map.texels()[0], map.texels()[1]];
    assert_eq!((w.surveyed_cells(), w.cells()), (4, 4));
    assert_eq!((p.surveyed_cells(), p.cells()), (3, 4));
    assert_eq!(bits(p.surveyed_fraction()), bits(0.75));
    let classes: Vec<GroundMaterial> = map.classes().iter().map(ClassMapClass::material).collect();
    let field = fields.field(body).unwrap().clone();
    let synth = Synthesiser::new(&field, SEED);
    let band = BandLevel::new(CLASS_MAP_BAND).unwrap();
    let bytes_at = |at: FaceUv| {
        let weights = weights_at(&synth, &mut SynthCache::new(), at, band).unwrap();
        let mut bytes = [0_u8; CLASS_MAP_CLASSES];
        for (material, byte) in weights.iter() {
            let k = classes.iter().position(|&c| c == material).unwrap();
            bytes[k] = byte;
        }
        bytes
    };
    // The whole texel: the weights at its centre, its centre's cell surveyed.
    assert_eq!(
        w.weights(),
        bytes_at(face_uv(whole, 0.5, 0.5)),
        "the whole texel's weights are its centre's"
    );
    // The part: the mean of its three surveyed cells' centres, as bytes summing to 255.
    let points: Vec<[u8; CLASS_MAP_CLASSES]> = cells
        .iter()
        .filter(|c| part.children().contains(c))
        .map(|&c| bytes_at(face_uv(c, 0.5, 0.5)))
        .collect();
    assert_eq!(points.len(), 3);
    for (k, &byte) in p.weights().iter().enumerate() {
        let mean = points.iter().map(|b| f64::from(b[k])).sum::<f64>() / 3.0;
        assert!(
            (f64::from(byte) - mean).abs() < 1.0,
            "class {k}: byte {byte} against the mean {mean}"
        );
    }
    assert_eq!(
        p.weights().iter().map(|&b| u32::from(b)).sum::<u32>(),
        u32::from(WEIGHT_TOTAL)
    );
}

/// [`bytes_by_largest_remainder`]'s bytes for `values`, those past them checked 0.
fn bytes_of(values: &[f64]) -> Vec<u8> {
    let bytes = bytes_by_largest_remainder(values);
    let (held, rest) = bytes.split_at(values.len());
    assert!(rest.iter().all(|&b| b == 0), "{values:?}: {bytes:?}");
    held.to_vec()
}

#[test]
fn worker_class_map_bytes_keep_their_sum_by_the_largest_remainder() {
    assert_eq!(bytes_of(&[85.0, 85.0, 85.0]), [85, 85, 85]);
    // Ties to the earlier.
    assert_eq!(bytes_of(&[127.5, 127.5]), [128, 127]);
    assert_eq!(bytes_of(&[255.0 / 3.0, 510.0 / 3.0]), [85, 170]);
    assert_eq!(bytes_of(&[100.4, 100.4, 54.2]), [101, 100, 54]);
    // A sum short of 255, a class left out: the rounded sum.
    assert_eq!(bytes_of(&[60.6, 60.6]), [61, 60]);
    // As many values as the map has classes, and none.
    let sixteen = [255.0 / 16.0; CLASS_MAP_CLASSES];
    assert_eq!(
        bytes_by_largest_remainder(&sixteen),
        [
            16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 15
        ]
    );
    assert_eq!(bytes_of(&[]), [] as [u8; 0]);
}

#[test]
fn worker_class_map_keeps_the_heaviest_classes_past_sixteen() {
    // Seventeen materials, each on its own texel, the k-th at a weight of k + 1 in a whole texel,
    // and the heaviest also at a quarter on a quarter-surveyed texel.
    let materials: Vec<GroundMaterial> = (0..17_u8)
        .map(|k| GroundMaterial::of(MaterialClass::Regolith, Some(k)))
        .collect();
    let mut readings: Vec<TexelReading> = materials
        .iter()
        .enumerate()
        .map(|(k, &m)| TexelReading {
            cell: texel(u32::try_from(k).unwrap(), 0),
            surveyed: 1,
            cells: 1,
            weights: BTreeMap::from([(m, f64::from(u8::try_from(k + 1).unwrap()))]),
            maturity: None,
        })
        .collect();
    readings.push(TexelReading {
        cell: texel(0, 1),
        surveyed: 1,
        cells: 4,
        weights: BTreeMap::from([(materials[0], 4.0)]),
        maturity: None,
    });
    // Material 0 now weighs 1 + 1, the same as material 1, and the lighter by the order is left
    // out: of the two at 2, material 1, the later.
    let kept = kept_classes(&readings);
    assert_eq!(kept.len(), CLASS_MAP_CLASSES);
    assert!(kept.contains(&materials[0]));
    assert!(!kept.contains(&materials[1]));
    assert_eq!(kept, [&materials[..1], &materials[2..]].concat());
}

/// The high setting's doubled normals alone: on the face-differences path, which moves the vertices
/// and changes no normal, with no weights and no horizon map, neither of which changes a patch's
/// normals either.
const NORMALS_ONLY: TerrainBakeOptions = TerrainBakeOptions {
    patch: BakeOptions {
        vertex_path: VertexPath::FaceDifferences,
        normals: NormalScale::Double,
        skirt_m: 0.0,
    },
    weights: WeightsLayer::Skipped,
    horizon: HorizonLayer::Skipped,
};

/// A normal map by its bits, as the tests compare two: its level, its cells, each pair's bits and
/// whether it is complete.
type NormalMapBits = (u8, Vec<u32>, Vec<[u32; 2]>, bool);

/// `map`'s [`NormalMapBits`].
fn normal_map_bits(map: &NormalMap) -> NormalMapBits {
    (
        map.level(),
        map.cells().to_vec(),
        map.pairs().iter().map(|pair| pair.map(bits_f32)).collect(),
        map.is_complete(),
    )
}

/// The coarsest patch whose doubled normals hold `cell`'s centre as a sample: the one of level
/// max(L − 6, 0) holding it, L the cell's level.
fn coarsest_patch_over(cell: PatchKey) -> PatchKey {
    patch_over(cell, cell.level().saturating_sub(6))
}

/// The pair `bake`'s doubled normals hold at the centre of `cell`, a cell of a level at most 6
/// below the patch's: of the patch's 129 samples a side (128 intervals), sample
/// (2 c + 1) · 2^(6 − d) for the cell's place c in the patch, d levels below it (the odd–odd
/// samples at d = 6).
fn doubled_pair_at(bake: &BodyBake, cell: PatchKey) -> [f32; 2] {
    let key = bake.terrain().patch().key;
    let shift = u32::from(cell.level() - key.level());
    let step = 1_u32 << (6 - shift);
    let sample = |c: u32, k: u32| usize::try_from((2 * (c - (k << shift)) + 1) * step).unwrap();
    let (x, y) = (sample(cell.i(), key.i()), sample(cell.j(), key.j()));
    let at = 2 * (y * NormalScale::Double.side() + x);
    let normals = &bake.terrain().patch().normals;
    [normals[at], normals[at + 1]]
}

/// Checks that each pair of `map`, of `body`'s field in `fields`, is what the coarsest patch over its
/// cell bakes at the cell's centre on the high setting, bit for bit, and the octahedral pair of a
/// finite, unit and outward normal there, made apart from the worker.
fn assert_normal_map_is_the_coarsest_doubled_bake(
    fields: &mut WorkerFields,
    body: BodyRef,
    map: &NormalMap,
) {
    assert_eq!(map.pairs().len(), map.cells().len());
    let field = fields.field(body).unwrap().clone();
    assert_eq!(map.level(), field.header().level().get());
    let figure = field.header().figure();
    let synth = Synthesiser::new(&field, SEED);
    let band_level = band(map.band());
    let mut bakes = BTreeMap::<PatchKey, BodyBake>::new();
    for (&index, &pair) in map.cells().iter().zip(map.pairs()) {
        let cell = cell_at_index(map.level(), index).unwrap();
        assert!(field.is_surveyed(cell), "{cell:?}");
        let key = coarsest_patch_over(cell);
        let bake = bakes
            .entry(key)
            .or_insert_with(|| fields.bake(body, SEED, key, &NORMALS_ONLY).unwrap());
        assert_eq!(
            pair.map(bits_f32),
            doubled_pair_at(bake, cell).map(bits_f32),
            "{cell:?}: the map's pair against {key:?}'s doubled normal"
        );
        let dir = point(cell, 0.5, 0.5);
        let sample = synth
            .height_at(&mut SynthCache::new(), dir, band_level)
            .unwrap();
        let normal = surface_normal(&figure, dir, sample.height_m, sample.gradient);
        assert!(normal.iter().all(|c| c.is_finite()), "{cell:?}: {normal:?}");
        let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        assert!((length - 1.0).abs() < 1e-12, "{cell:?}: |n| = {length}");
        let nu = figure.normal(dir);
        let up = normal[0] * nu[0] + normal[1] * nu[1] + normal[2] * nu[2];
        assert!(up > 0.5, "{cell:?}: n · ν = {up}, a slope past 60°");
        assert_eq!(
            pair.map(bits_f32),
            encode_octahedral_f32(normal).map(bits_f32),
            "{cell:?}"
        );
        assert!(pair.iter().all(|c| c.abs() <= 1.0), "{cell:?}: {pair:?}");
    }
}

#[test]
fn worker_normal_map_holds_each_surveyed_cell_with_the_root_s_doubled_normal_at_its_centre() {
    let f = fixture();
    let mut fields = f.worker();
    let map = fields.normal_map(f.body, SEED).unwrap();
    assert_eq!(
        (map.level(), map.band()),
        (6, 0),
        "the Moon-like field is at level 6, its normals at the root's band"
    );
    assert!(map.is_complete());
    // No cell for a margin cell or one not held: the surveyed cells, each once, rising.
    let surveyed: BTreeSet<PatchKey> = f.first.union(&f.second).copied().collect();
    assert_eq!(map.cells(), indices(&surveyed).as_slice());
    assert!(
        surveyed
            .iter()
            .all(|&cell| coarsest_patch_over(cell) == PatchKey::root(Face::PosY)),
        "each cell's coarsest patch is the root of face +y"
    );
    assert_normal_map_is_the_coarsest_doubled_bake(&mut fields, f.body, &map);
    // From the body's cache again, and from a fresh worker, the same map, bit for bit.
    let bits = normal_map_bits(&map);
    assert_eq!(
        normal_map_bits(&fields.normal_map(f.body, SEED).unwrap()),
        bits
    );
    assert_eq!(
        normal_map_bits(&f.worker().normal_map(f.body, SEED).unwrap()),
        bits
    );
}

#[test]
fn worker_normal_map_is_the_coarsest_doubled_bake_at_every_field_level() {
    let mut levels = BTreeSet::new();
    for &w in SyntheticWorld::ALL {
        let field = world(w);
        let (mut fields, cells) = surveyed_about(field, unit_dir([0.3, 1.0, 0.2]));
        let body = field.header().body();
        let map = fields.normal_map(body, SEED).unwrap();
        let level = field.header().level().get();
        assert_eq!(
            (map.level(), map.band()),
            (level, level.saturating_sub(6)),
            "{w:?}"
        );
        assert!(map.is_complete(), "{w:?}");
        assert_eq!(map.cells(), indices(&cells).as_slice(), "{w:?}");
        assert_normal_map_is_the_coarsest_doubled_bake(&mut fields, body, &map);
        levels.insert(level);
    }
    assert_eq!(levels.into_iter().collect::<Vec<_>>(), [5, 6, 7, 8]);
}

#[test]
fn worker_normal_map_needs_the_body_s_field() {
    let f = fixture();
    let mut fields = f.worker();
    let other = BodyRef::new(f.body.raw_system_id(), f.body.body_index() ^ 1);
    assert_eq!(
        fields.normal_map(other, SEED),
        Err(QueryBodyError::NotStarted(other))
    );
}

#[test]
fn worker_normal_map_of_a_survey_without_its_margin_waits_for_the_margin() {
    let f = fixture();
    let mut fresh = WorkerFields::new();
    fresh.init(f.body, &f.first_blocks[0]).unwrap();
    for block in &f.first_blocks[1..] {
        fresh.chunk(f.body, block).unwrap();
    }
    let full = fresh.normal_map(f.body, SEED).unwrap();
    assert!(full.is_complete());
    assert_eq!(full.cells(), indices(&f.first).as_slice());
    // Each later block of the first payload with block 0 alone, so that a cell can be held surveyed
    // while the margin its height reads, in other blocks, is not.
    let mut waited = false;
    for (k, block) in f.first_blocks.iter().enumerate().skip(1) {
        let mut fields = WorkerFields::new();
        fields.init(f.body, &f.first_blocks[0]).unwrap();
        fields.chunk(f.body, block).unwrap();
        let partial = fields.normal_map(f.body, SEED).unwrap();
        // What it holds is the whole map's, bit for bit.
        for (cell, pair) in partial.cells().iter().zip(partial.pairs()) {
            let at = full.cells().binary_search(cell).unwrap();
            assert_eq!(
                pair.map(bits_f32),
                full.pairs()[at].map(bits_f32),
                "block {k}"
            );
        }
        if partial.is_complete() {
            continue;
        }
        let field = fields.field(f.body).unwrap();
        let waiting = *full
            .cells()
            .iter()
            .find(|&&c| {
                field.is_surveyed(cell_at_index(full.level(), c).unwrap())
                    && !partial.cells().contains(&c)
            })
            .unwrap_or_else(|| panic!("block {k}: an incomplete map leaves out a surveyed cell"));
        // The rest of the payload: the map is whole, holds the cell and is a fresh worker's.
        for (n, rest) in f.first_blocks.iter().enumerate().skip(1) {
            if n != k {
                fields.chunk(f.body, rest).unwrap();
            }
        }
        let whole = fields.normal_map(f.body, SEED).unwrap();
        assert!(whole.is_complete(), "block {k}");
        assert!(whole.cells().contains(&waiting), "block {k}: {waiting}");
        assert_eq!(normal_map_bits(&whole), normal_map_bits(&full), "block {k}");
        waited = true;
    }
    assert!(waited, "some cell's height reads a margin still to come");
}

/// A worker holding a survey of the 2 × 2 cells of `field`'s level about the direction `dir`, at
/// 50 m, with its margin, and the cells.
pub(super) fn surveyed_about(
    field: &CoarseField,
    dir: [f64; 3],
) -> (WorkerFields, BTreeSet<PatchKey>) {
    let level = field.header().level().get();
    let home = PatchKey::containing(level, dir).unwrap();
    let (i, j) = (home.i() & !1, home.j() & !1);
    let cells: BTreeSet<PatchKey> = [(0, 0), (1, 0), (0, 1), (1, 1)]
        .map(|(a, b)| PatchKey::new(home.face(), level, i + a, j + b).unwrap())
        .into_iter()
        .collect();
    let blocks = split(&encode_within(
        field,
        &cover(&cells, |_| code(50.0)),
        None,
        SYNTHESIS_MARGIN_CELLS,
        FIXTURE_BLOCK_BYTES,
    ));
    let body = field.header().body();
    let mut fields = WorkerFields::new();
    fields.init(body, &blocks[0]).unwrap();
    for block in &blocks[1..] {
        fields.chunk(body, block).unwrap();
    }
    (fields, cells)
}

/// A drawn photometry's bits: its albedos, its law and its L(0).
fn drawn_bits(d: DrawnPhotometry) -> ([u64; 3], PhaseLaw, u64) {
    (
        d.normal_albedo_bvr().map(bits),
        d.phase_law(),
        bits(d.limb_parameter()),
    )
}

/// R10.T1.c's identity: on every synthetic world, whose header carries neither a maturity nor a
/// cover, the worker's photometry, class map and material are their as-built answers, the
/// palette's properties as resolved under the header's scale, bit for bit.
#[test]
fn worker_answers_are_as_built_without_a_maturity_or_a_cover_on_every_world() {
    for &w in SyntheticWorld::ALL {
        let field = world(w);
        let header = field.header();
        assert_eq!((header.maturity(), header.organic_cover()), (None, None));
        let (mut fields, cells) = surveyed_about(field, unit_dir([0.3, 1.0, 0.2]));
        let body = header.body();
        let palette = header.palette();
        let c = header.albedo_scale().unwrap_or(1.0);
        let as_built = |m: Material| {
            let p = properties(m, palette).unwrap();
            let bvr = scaled_normal_albedo(&p, c);
            (
                bvr.map(bits),
                p.phase_law(),
                bits(p.phase_law().limb_parameter(bvr[1])),
            )
        };
        for entry in 0..u8::try_from(palette.len()).unwrap() {
            for &class in MaterialClass::ALL {
                let m = Material::new(class, entry).unwrap();
                let drawn = fields.photometry(body, m).unwrap();
                assert_eq!(drawn_bits(drawn), as_built(m), "{w:?} {m:?}");
            }
        }
        let held = fields.field(body).unwrap().clone();
        let synth = Synthesiser::new(&held, SEED);
        for &cell in &cells {
            for (x, y) in [(0.3, 0.6), (0.7, 0.2)] {
                let at = face_uv(cell, x, y);
                let ground = material_at(&synth, &mut SynthCache::new(), at).unwrap();
                let want = SurveyedMaterial {
                    ground,
                    maturity: None,
                    properties: ground.material().map(|m| properties(m, palette).unwrap()),
                };
                assert_eq!(fields.material(body, SEED, at), Ok(Some(want)), "{w:?}");
            }
        }
        let map = fields.class_map(body, SEED).unwrap();
        assert!(!map.classes().is_empty(), "{w:?}");
        for class in map.classes() {
            match class.material().material() {
                Some(m) => assert_eq!(
                    class.photometry().map(drawn_bits),
                    Some(as_built(m)),
                    "{w:?}"
                ),
                None => assert_eq!(class.photometry(), None, "{w:?}"),
            }
        }
    }
}

/// R10.T1.c on a weathered Moon-like world: the photometry, and so the class map, draws each
/// material's albedo exposed at the header's maturity, `Melt` fresh, darker than the fresh
/// world's, and at maturity 0 the fresh world's to the bit; and the point query takes the point's
/// own maturity, `maturity_at`'s, and its material's properties exposed there.
#[test]
fn worker_photometry_takes_the_header_s_maturity_and_material_the_point_s() {
    let weathered = moon_like_builder()
        .maturity(1.0)
        .weathering_rate(1.0)
        .build();
    let young = moon_like_builder()
        .maturity(0.0)
        .weathering_rate(1.0)
        .build();
    let dir = point(cell(22, 22), 0.5, 0.5);
    let (fresh, _) = surveyed_about(world(WORLD), dir);
    let (zero, _) = surveyed_about(&young, dir);
    let (mut mature, cells) = surveyed_about(&weathered, dir);
    let header = weathered.header();
    let body = header.body();
    assert_eq!(body, world(WORLD).header().body());
    let palette = header.palette();
    let exposure = GroundExposure::of_header(header);
    assert_eq!(exposure.maturity().map(bits), Some(bits(1.0)));
    let mut darker = false;
    for entry in 0..u8::try_from(palette.len()).unwrap() {
        for &class in MaterialClass::ALL {
            let m = Material::new(class, entry).unwrap();
            let unweathered = fresh.photometry(body, m).unwrap();
            assert_eq!(
                drawn_bits(zero.photometry(body, m).unwrap()),
                drawn_bits(unweathered),
                "{m:?} at maturity 0"
            );
            let drawn = mature.photometry(body, m).unwrap();
            let exposed = exposed_properties(m, palette, exposure).unwrap();
            let bvr = scaled_normal_albedo(&exposed, header.albedo_scale().unwrap_or(1.0));
            assert_eq!(drawn.normal_albedo_bvr().map(bits), bvr.map(bits), "{m:?}");
            if class == MaterialClass::Melt {
                assert_eq!(drawn_bits(drawn), drawn_bits(unweathered), "{m:?}");
            }
            darker |= drawn.normal_albedo_bvr()[1] < unweathered.normal_albedo_bvr()[1];
        }
    }
    assert!(darker, "the weathered crust draws darker");
    let held = mature.field(body).unwrap().clone();
    let synth = Synthesiser::new(&held, SEED);
    let mut immature = false;
    for &cell in &cells {
        for (x, y) in [(0.3, 0.6), (0.7, 0.2), (0.5, 0.5)] {
            let at = face_uv(cell, x, y);
            let got = mature.material(body, SEED, at).unwrap().unwrap();
            let local = maturity_at(&synth, &mut SynthCache::new(), at)
                .unwrap()
                .unwrap();
            assert_eq!(got.maturity().map(bits), Some(bits(local)));
            immature |= local < 1.0;
            let ground = material_at(&synth, &mut SynthCache::new(), at).unwrap();
            assert_eq!(got.ground(), ground);
            let want = ground
                .material()
                .map(|m| exposed_properties(m, palette, exposure.with_maturity(local)).unwrap());
            assert_eq!(got.properties(), want.as_ref());
        }
    }
    assert!(immature, "some point lies in a young crater's halo");
    let map = mature.class_map(body, SEED).unwrap();
    for class in map.classes() {
        if let Some(m) = class.material().material() {
            assert_eq!(
                class.photometry().map(drawn_bits),
                Some(drawn_bits(mature.photometry(body, m).unwrap()))
            );
        }
    }
}

/// The reference pad, 1.0 m (the guide's `PAD WIDTH`).
const PAD: Metres = Metres::new(1.0);

/// A ground readout's query along `sight`, over `slope_baseline`, under the reference pad, at the
/// season `season` (radians, or none).
fn readout_query(
    (origin_m, direction): ([f64; 3], [f64; 3]),
    slope_baseline: SlopeBaseline,
    season: Option<f64>,
) -> GroundReadoutQuery {
    GroundReadoutQuery {
        origin_m,
        direction,
        max: Metres::new(1e7),
        slope_baseline,
        pad_width: PAD,
        season: season.map(|m| SeasonAnomaly::try_from(Radians::new(m)).unwrap()),
    }
}

/// The readout's parts where it meets the ground, which the test expects it to.
fn on_ground(readout: GroundReadout) -> GroundAtSight {
    match readout {
        GroundReadout::Ground(ground) => *ground,
        GroundReadout::NoGround | GroundReadout::NotSurveyed => {
            panic!("the line of sight meets no surveyed ground: {readout:?}")
        }
    }
}

/// R10.T12: a baseline is a member of the readout's list, to the bit, and nothing else.
#[test]
fn worker_slope_baselines_are_the_readout_s_three_alone() {
    for (baseline, metres) in SlopeBaseline::ALL.into_iter().zip(SLOPE_BASELINES) {
        assert_eq!(bits(baseline.metres().value()), bits(metres.value()));
        assert_eq!(SlopeBaseline::try_from(metres), Ok(baseline));
    }
    assert_eq!(SlopeBaseline::default(), CAPACITY_SLOPE_BASELINE);
    assert_eq!(bits(CAPACITY_SLOPE_BASELINE.metres().value()), bits(2.0));
    for length in [0.0, 2.0_f64.next_up(), 3.0, -2.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            SlopeBaseline::try_from(Metres::new(length)).map_err(|e| e.to_string()),
            Err(format!(
                "a slope baseline of {length} m is none of the readout's 2 m, 5 m and 100 m"
            ))
        );
    }
    for anomaly in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            SeasonAnomaly::try_from(Radians::new(anomaly)).map_err(|e| e.to_string()),
            Err(format!(
                "a season's mean anomaly of {anomaly} rad is not finite"
            ))
        );
    }
    let far = SeasonAnomaly::try_from(Radians::new(-1e9)).unwrap();
    assert_eq!(bits(far.radians().value()), bits(-1e9));
}

/// R10.T12: every part of a ground readout is the separate query's to the bit, the line of sight's
/// `slant_range`, the foot's `reading` over each baseline and its `material`; the capacity is
/// `bearing_capacity_on_slope` at the body's gravity on the reading's RMS slope over 2 m, whatever
/// the baseline; and a line missing the body reads `NoGround`, one low over the survey's margin
/// `NotSurveyed`, as `slant_range` gives them.
#[test]
fn worker_ground_readout_is_the_separate_queries_bit_for_bit() {
    let f = fixture();
    let mut fields = f.worker();
    let field = f.field();
    let header = field.header();
    let max = Metres::new(1e7);
    let mut grounds = 0;
    for sight in sight_lines(&field) {
        for baseline in SlopeBaseline::ALL {
            let readout = fields
                .ground_readout(f.body, SEED, &readout_query(sight, baseline, None))
                .unwrap();
            let line = fields
                .slant_range(f.body, SEED, &sight.0, &sight.1, max)
                .unwrap();
            let ground = match (line, readout) {
                (LineOfSight::Ground(line), GroundReadout::Ground(ground)) => {
                    assert_eq!(*ground.sight(), line);
                    ground
                }
                (LineOfSight::NoGround, readout) => {
                    assert_eq!(readout, GroundReadout::NoGround);
                    continue;
                }
                (LineOfSight::NotSurveyed, readout) => {
                    assert_eq!(readout, GroundReadout::NotSurveyed);
                    continue;
                }
                (LineOfSight::Ground(_), readout) => panic!("{readout:?} on ground"),
            };
            grounds += 1;
            let at = ground.sight().at();
            assert_eq!(
                fields.reading(f.body, SEED, at, baseline.metres()),
                Ok(Some(*ground.reading()))
            );
            assert_eq!(
                fields.material(f.body, SEED, at),
                Ok(Some(*ground.material()))
            );
            let two_metres = fields
                .reading(f.body, SEED, at, CAPACITY_SLOPE_BASELINE.metres())
                .unwrap()
                .unwrap();
            let properties = ground.properties();
            assert_eq!(
                bits(properties.capacity_slope().value()),
                bits(two_metres.slope_rms().value())
            );
            assert_eq!(bits(properties.pad_width().value()), bits(PAD.value()));
            let p = ground
                .material()
                .properties()
                .expect("the regolith is named");
            let capacity = bearing_capacity_on_slope(
                p,
                header.craters().gravity(),
                PAD,
                two_metres.slope_rms(),
            )
            .expect("the regolith has a shear table");
            assert_eq!(
                properties.bearing_capacity().map(|q| bits(q.value())),
                Some(bits(capacity.value()))
            );
        }
    }
    assert_eq!(
        grounds,
        SlopeBaseline::ALL.len(),
        "the line down meets the ground"
    );
    let [_, across, up] = sight_lines(&field);
    let ask = |fields: &mut WorkerFields, sight| {
        fields.ground_readout(
            f.body,
            SEED,
            &readout_query(sight, CAPACITY_SLOPE_BASELINE, None),
        )
    };
    assert_eq!(ask(&mut fields, up), Ok(GroundReadout::NoGround));
    assert_eq!(ask(&mut fields, across), Ok(GroundReadout::NotSurveyed));
    assert_eq!(
        ask(&mut WorkerFields::new(), up),
        Err(QueryBodyError::NotStarted(f.body))
    );
}

/// R10.T12: on regolith the pad friction is NAVFAC's constant, at any temperature and with none:
/// it does not depend on the temperature, so the readout states no `AT`, and a year of twelve
/// months with no season takes no temperature.
#[test]
fn worker_ground_readout_reads_a_constant_friction_on_regolith() {
    let f = fixture();
    let mut fields = f.worker();
    let [down, ..] = sight_lines(&f.field());
    let without = on_ground(
        fields
            .ground_readout(
                f.body,
                SEED,
                &readout_query(down, SlopeBaseline::TwoMetres, None),
            )
            .unwrap(),
    );
    let with = on_ground(
        fields
            .ground_readout(
                f.body,
                SEED,
                &readout_query(down, SlopeBaseline::TwoMetres, Some(2.0)),
            )
            .unwrap(),
    );
    assert_eq!(without.material().ground().class(), MaterialClass::Regolith);
    let p = without.material().properties().unwrap();
    assert!(matches!(
        p.interface_friction_model(),
        Some(FrictionModel::Constant(_))
    ));
    for ground in [&without, &with] {
        let properties = ground.properties();
        assert!(!properties.friction_on_temperature());
        for t in [100.0, 250.0, 400.0] {
            assert_eq!(
                properties.friction().map(bits),
                interface_friction(p, Kelvin::new(t)).map(bits)
            );
        }
    }
    assert_eq!(without.properties().temperature(), None);
    assert!(with.properties().temperature().is_some());
}

/// R10.T12: a pad whose width is not finite and positive has no capacity, the zero width included,
/// under which `bearing_capacity_on_slope` gives the regolith's cohesion term alone; the readout's
/// other parts are the reference pad's.
#[test]
fn worker_ground_readout_takes_no_capacity_under_a_pad_not_finite_and_positive() {
    let f = fixture();
    let mut fields = f.worker();
    let gravity = f.field().header().craters().gravity();
    let [down, ..] = sight_lines(&f.field());
    let reference = on_ground(
        fields
            .ground_readout(
                f.body,
                SEED,
                &readout_query(down, SlopeBaseline::TwoMetres, None),
            )
            .unwrap(),
    );
    let p = reference.material().properties().unwrap();
    let slope = reference.properties().capacity_slope();
    assert!(
        bearing_capacity_on_slope(p, gravity, Metres::ZERO, slope).is_some(),
        "the formula bears a cohesive soil at zero width, so the guard is the readout's"
    );
    assert!(reference.properties().bearing_capacity().is_some());
    for width in [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY] {
        let query = GroundReadoutQuery {
            pad_width: Metres::new(width),
            ..readout_query(down, SlopeBaseline::TwoMetres, None)
        };
        let ground = on_ground(fields.ground_readout(f.body, SEED, &query).unwrap());
        let properties = ground.properties();
        assert_eq!(properties.bearing_capacity(), None, "{width} m");
        assert!(
            properties.pad_width().value().total_cmp(&width).is_eq(),
            "the width asked is the width held, a NaN's bits included: {width} m"
        );
        assert_eq!(ground.reading(), reference.reading());
        assert_eq!(ground.material(), reference.material());
        assert_eq!(
            properties.friction().map(bits),
            reference.properties().friction().map(bits)
        );
    }
}

/// R10.T12: a foot whose field cell is not surveyed, a survey's margin or a cell not held, reads
/// `NotSurveyed`, as `material` judges the cell, so that the readout never reads ground the survey
/// does not hold. The march meets surveyed ground alone, so the foot's guard is held here over the
/// readout's own reading at a foot.
#[test]
fn worker_ground_readout_reads_a_margin_foot_as_not_surveyed() {
    let f = fixture();
    let field = f.field();
    let synth = Synthesiser::new(&field, SEED);
    let query = readout_query(sight_lines(&field)[0], SlopeBaseline::TwoMetres, None);
    let margin = face_uv(cell(20, 22), 0.5, 0.5);
    let beyond = face_uv(cell(40, 40), 0.5, 0.5);
    let surveyed = face_uv(cell(22, 22), 0.3, 0.6);
    let mut cache = SynthCache::new();
    assert!(!field.is_surveyed(field_cell(&field, margin)));
    assert!(
        field.resolution(field_cell(&field, margin)).is_some(),
        "held as margin"
    );
    for at in [margin, beyond] {
        assert!(ground_at(&synth, &mut cache, at, &query).unwrap().is_none());
    }
    let ground = ground_at(&synth, &mut cache, surveyed, &query)
        .unwrap()
        .expect("surveyed ground is read");
    let mut fields = f.worker();
    assert_eq!(
        fields.reading(f.body, SEED, surveyed, Metres::new(2.0)),
        Ok(Some(ground.reading))
    );
    assert_eq!(
        fields.material(f.body, SEED, surveyed),
        Ok(Some(ground.material))
    );
}

/// A direction, the unit vector at latitude `lat_deg` and longitude `lon_deg`, degrees.
fn lat_lon(lat_deg: f64, lon_deg: f64) -> [f64; 3] {
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    [
        hyperion_base::math::cos(lat) * hyperion_base::math::cos(lon),
        hyperion_base::math::cos(lat) * hyperion_base::math::sin(lon),
        hyperion_base::math::sin(lat),
    ]
}

/// The Earth-like world's first point, on a grid of the north polar ice from 70° of latitude, whose
/// material is glacier ice of water ice, whose friction follows the temperature.
fn earth_glacier() -> [f64; 3] {
    let field = world(SyntheticWorld::EarthLike);
    let synth = Synthesiser::new(field, SEED);
    let mut cache = SynthCache::new();
    (70..=86)
        .step_by(4)
        .flat_map(|lat| (0..360).step_by(30).map(move |lon| (lat, lon)))
        .map(|(lat, lon)| lat_lon(f64::from(lat), f64::from(lon)))
        .find(|&dir| {
            let ground = material_at(&synth, &mut cache, crate::cube::xyz_to_face_uv(dir)).unwrap();
            ground.class() == MaterialClass::Glacier
                && ground.material().is_some_and(|m| {
                    matches!(
                        properties(m, field.header().palette())
                            .unwrap()
                            .interface_friction_model(),
                        Some(FrictionModel::ByTemperature(_))
                    )
                })
        })
        .expect("the Earth-like world's polar caps hold glacier ice of H2O")
}

/// R10.T12 (decision-r10-t12-leans §6): on the Earth-like world's glacier ice of H2O, a year of
/// twelve months, the readout's temperature is the month's mean at sea level carried to the
/// reading's elevation by the classifier's expression, `T_sea − Γ (h − sea level)`, bit for bit; the
/// month is `month_at` the season's mean anomaly, month k − 1 just below edge k and month k at it;
/// the friction is `interface_friction` at that temperature and depends on it; and with no season
/// there is no temperature, so no friction, `PAD FRICTION —` with no `AT`.
#[test]
fn worker_ground_readout_takes_the_month_s_temperature_on_glacier_ice() {
    let field = world(SyntheticWorld::EarthLike);
    let header = field.header();
    assert_eq!(header.months(), 12);
    assert!(header.lapse_rate_k_per_m() > 0.0, "a lapse rate to carry");
    let dir = earth_glacier();
    let (mut fields, _) = surveyed_about(field, dir);
    let body = header.body();
    let origin = header.figure().surface_point(dir, 20_000.0);
    let sight = (origin, origin.map(|x| -x));
    let edges = crate::field::month_edges(header.season_eccentricity()).unwrap();
    let readout_at = |fields: &mut WorkerFields, season: Option<f64>| {
        on_ground(
            fields
                .ground_readout(
                    body,
                    SEED,
                    &readout_query(sight, SlopeBaseline::TwoMetres, season),
                )
                .unwrap(),
        )
    };
    let none = readout_at(&mut fields, None);
    let p = *none.material().properties().unwrap();
    assert_eq!(none.material().ground().class(), MaterialClass::Glacier);
    assert!(none.properties().friction_on_temperature());
    assert_eq!(
        (
            none.properties().temperature(),
            none.properties().friction()
        ),
        (None, None)
    );
    let held = fields.field(body).unwrap();
    let foot = none.sight().at();
    let climate = *held.climate(field_cell(held, foot)).unwrap();
    let h = none.reading().elevation().value();
    // `facts`' expression, written out.
    let expected = |month: u8| {
        header.month_temperature(&climate, month).unwrap().value()
            - header.lapse_rate_k_per_m() * (h - header.sea_level().value())
    };
    let mut stepped = false;
    let mut friction_known = false;
    for k in 0..12_u8 {
        let edge = edges[usize::from(k)].value();
        // Just before periapsis is just below the year's end, edge 12: a step below 0 itself
        // reduces to 0 (`month_at`'s rounding of a lift to 2π).
        let below = if k == 0 {
            edges[12].value().next_down()
        } else {
            edge.next_down()
        };
        let before = if k == 0 { 11 } else { k - 1 };
        for (anomaly, month) in [(edge, k), (below, before)] {
            assert_eq!(month_at(header, Radians::new(anomaly)), month);
            let ground = readout_at(&mut fields, Some(anomaly));
            let t = ground
                .properties()
                .temperature()
                .expect("a month's temperature");
            assert_eq!(bits(t.value()), bits(expected(month)), "month {month}");
            assert_eq!(
                ground.properties().friction().map(bits),
                interface_friction(&p, t).map(bits)
            );
            friction_known |= ground.properties().friction().is_some();
        }
        stepped |= expected(k).total_cmp(&expected(before)).is_ne();
    }
    assert!(stepped, "the temperature steps at some month's edge");
    assert!(
        friction_known,
        "some month lies inside Liefferink et al.'s points"
    );
}

/// R10.T12: a one-month year reads its one month whatever season is given, or none: the flat
/// world's ground, whose palette names no substance, so that neither property is computed.
#[test]
fn worker_ground_readout_reads_a_one_month_year_s_month_with_no_season() {
    let field = world(SyntheticWorld::Flat);
    let header = field.header();
    assert_eq!(header.months(), 1);
    let dir = unit_dir([0.2, 1.0, 0.3]);
    let (mut fields, _) = surveyed_about(field, dir);
    let body = header.body();
    let origin = header.figure().surface_point(dir, 20_000.0);
    let sight = (origin, origin.map(|x| -x));
    let read = |fields: &mut WorkerFields, season| {
        on_ground(
            fields
                .ground_readout(
                    body,
                    SEED,
                    &readout_query(sight, SlopeBaseline::TwoMetres, season),
                )
                .unwrap(),
        )
    };
    let none = read(&mut fields, None);
    let held = fields.field(body).unwrap();
    let climate = *held.climate(field_cell(held, none.sight().at())).unwrap();
    let t = header.month_temperature(&climate, 0).unwrap().value()
        - header.lapse_rate_k_per_m()
            * (none.reading().elevation().value() - header.sea_level().value());
    for season in [None, Some(0.0), Some(3.0), Some(-40.0)] {
        let ground = read(&mut fields, season);
        let properties = ground.properties();
        assert_eq!(
            properties.temperature().map(|k| bits(k.value())),
            Some(bits(t))
        );
        assert_eq!(ground.material().properties(), None);
        assert_eq!(
            (properties.bearing_capacity(), properties.friction()),
            (None, None)
        );
        assert!(!properties.friction_on_temperature());
    }
}

// ---- R10.T1.c's client half: the albedo nodes and the class map's maturity
// (decision-r10-t1c-client-scope §2.1 and §2.5's tests 3, 7 and 9) ----

/// The terrain test's geometric albedo in B, V and R, the p `TEST MOON` weathered is solved for:
/// the Moon-like palette's primary crust's normal albedo, 0.20 in each band, which the client's
/// `terrainMoonPhotometry` takes as p (p = A (2 + L(0)) ÷ 3 at L(0) = 1).
pub(super) fn terrain_test_p() -> [f64; 3] {
    world(WORLD)
        .header()
        .palette()
        .entries()
        .iter()
        .find(|entry| entry.role == PaletteRole::PrimaryCrust)
        .expect("the Moon-like palette has a primary crust")
        .normal_albedo_bvr
}

/// `TEST MOON` weathered's world before its scale: the Moon-like world on a 1 Gyr surface, its
/// crater contract's N(>1 km) 8.4 × 10⁻⁴ per km², at maturity 1 and the Moon's rate, with its
/// class fractions and maturity mix, the haloed solve's inputs: built once per test process.
fn weathered_terrain_inputs() -> &'static (CoarseField, ClassFractions, MaturityMix) {
    static INPUTS: OnceLock<(CoarseField, ClassFractions, MaturityMix)> = OnceLock::new();
    INPUTS.get_or_init(|| {
        let field = moon_like_builder()
            .surface_age(Gigayears::new(1.0))
            .crater_params(moon_like_crater_params(PerSquareKilometre::new(8.4e-4)))
            .maturity(1.0)
            .weathering_rate(1.0)
            .build();
        let synth = Synthesiser::new(&field, SEED);
        let fractions = class_fractions(&synth, &mut SynthCache::new())
            .expect("a coarse field holds every cell");
        let mix = maturity_mix(&synth).expect("the field carries a maturity");
        (field, fractions, mix)
    })
}

/// The haloed solve ([`haloed_albedo_scale`]) of `TEST MOON` weathered's world for the geometric
/// albedo `p`.
fn weathered_terrain_solve(p: [f64; 3]) -> AlbedoScale {
    let (field, fractions, mix) = weathered_terrain_inputs();
    let header = field.header();
    haloed_albedo_scale(
        fractions,
        header.palette(),
        GroundExposure::of_header(header),
        mix,
        p,
    )
    .expect("the world names its ground")
}

/// The primary crust's regolith on the Moon-like palette.
fn crust_regolith() -> Material {
    let entry = world(WORLD)
        .header()
        .palette()
        .entries()
        .iter()
        .position(|entry| entry.role == PaletteRole::PrimaryCrust)
        .expect("the Moon-like palette has a primary crust");
    Material::new(MaterialClass::Regolith, u8::try_from(entry).unwrap()).unwrap()
}

/// The geometric albedo in B, V and R `TEST MOON` weathered is solved for (decision-r10-t1c-client-
/// scope §3): in V 0.12, the Moon's geometric albedo ("Geometric albedo 0.12", D. R. Williams,
/// NASA NSSDCA Moon Fact Sheet, last updated 11 January 2024, read), and in B and R in the ratios
/// of the primary crust's regolith's own albedo at the header's `m_body`, its weathered colour,
/// which the solve, holding V alone, reads only into its residuals.
///
/// Its haloed solve gives c = 1.22997, 0.46% of the named ground held at its range's top in V, a
/// relative residual of −1.2 × 10⁻¹⁶ in V, +0.011 in B and −0.0044 in R (the maria's colour), and
/// a halo excess in V of 5.76%, the ruling's 1 Gyr ground's 5.7%; where the terrain test's own p
/// ([`terrain_test_p`]), the fresh crust's 0.20, gives c = 3.552 with every named ground held at
/// its range's top, a residual of −8.0% in V and a halo excess of 9 × 10⁻⁶, the weathered palette
/// being too dark for it
/// (`worker_the_weathered_fixture_s_p_is_the_moon_s_and_shows_its_haloes`).
pub(super) fn weathered_terrain_test_p() -> [f64; 3] {
    let (field, ..) = weathered_terrain_inputs();
    let header = field.header();
    let [b, v, r] = exposed_properties(
        crust_regolith(),
        header.palette(),
        GroundExposure::of_header(header),
    )
    .expect("the crust is the palette's")
    .normal_albedo_bvr();
    let p_v = 0.12;
    [p_v * b / v, p_v, p_v * r / v]
}

/// `TEST MOON` weathered (decision-r10-t1c-client-scope §3): [`weathered_terrain_inputs`]' world
/// with its header's albedo scale the haloed solve for [`weathered_terrain_test_p`]: built once
/// per test process.
pub(super) fn weathered_terrain_world() -> &'static CoarseField {
    static FIELD: OnceLock<CoarseField> = OnceLock::new();
    FIELD.get_or_init(|| {
        let scale = weathered_terrain_solve(weathered_terrain_test_p()).scale();
        weathered_terrain_inputs()
            .0
            .clone()
            .with_albedo_scale(Some(scale))
            .expect("a solved scale is finite and positive")
    })
}

/// The weathered fixture's p is the Moon's, and its haloes show: at [`weathered_terrain_test_p`]
/// the haloed solve meets p in V, clamps under 1% of the ground and states a halo excess above
/// 5%, where at the terrain test's own p, the fresh crust's, every named ground is held at its
/// range's top and the haloes add under 10⁻⁴; printed.
#[test]
fn worker_the_weathered_fixture_s_p_is_the_moon_s_and_shows_its_haloes() {
    for (name, p) in [
        ("the Moon's p", weathered_terrain_test_p()),
        ("the terrain test's p", terrain_test_p()),
    ] {
        let s = weathered_terrain_solve(p);
        eprintln!(
            "{name} {p:?}: c {:.6}, clamped high {:.5}, low {:.5}, relative residual {:?}, halo \
             excess {:.5}",
            s.scale(),
            s.clamped_high_share(),
            s.clamped_low_share(),
            s.relative_residual(),
            s.halo_excess_v()
        );
    }
    let moon = weathered_terrain_solve(weathered_terrain_test_p());
    assert!(moon.relative_residual()[1].abs() < 1e-9, "{moon:?}");
    assert!(moon.clamped_high_share() < 0.01, "{moon:?}");
    assert!(moon.halo_excess_v() > 0.05, "{moon:?}");
    let fresh = weathered_terrain_solve(terrain_test_p());
    assert!(fresh.clamped_high_share() > 0.99, "{fresh:?}");
    assert!(fresh.halo_excess_v() < 1e-4, "{fresh:?}");
    assert_eq!(
        weathered_terrain_world().header().albedo_scale().map(bits),
        Some(bits(moon.scale()))
    );
}

/// The Moon-like world weathered on its 3.5 Gyr surface (N(>1 km) 4.8 × 10⁻³ per km²), at maturity
/// 1 and the Moon's rate, with no albedo scale: built once per test process.
pub(super) fn middle_weathered_world() -> &'static CoarseField {
    static FIELD: OnceLock<CoarseField> = OnceLock::new();
    FIELD.get_or_init(|| {
        moon_like_builder()
            .surface_age(Gigayears::new(3.5))
            .crater_params(moon_like_crater_params(PerSquareKilometre::new(4.8e-3)))
            .maturity(1.0)
            .weathering_rate(1.0)
            .build()
    })
}

/// The shallowest band level of `field` whose shortest wavelength is the band limit, where every
/// octave is drawn crater by crater.
pub(super) fn band_limit_level(field: &CoarseField) -> u8 {
    let figure = field.header().figure();
    (0..=MAX_LEVEL)
        .find(|&n| maturity_cut(&figure, band(n)).value() <= 0.5 * BAND_LIMIT_M)
        .expect("a body's finest levels reach the band limit")
}

/// The drawn albedo G(m) of `material` on the body of `header`: its exposed properties at the
/// header's exposure at maturity m, under the header's scale.
fn drawn_on(header: &FieldHeader, material: Material) -> impl Fn(f64) -> [f64; 3] + '_ {
    move |m| {
        let exposed = exposed_properties(
            material,
            header.palette(),
            GroundExposure::of_header(header).with_maturity(m),
        )
        .unwrap();
        scaled_normal_albedo(&exposed, header.albedo_scale().unwrap_or(1.0))
    }
}

/// A list of nodes' bits.
fn node_bits(nodes: &[[f64; 3]; MATURITY_NODES]) -> Vec<[u64; 3]> {
    nodes.iter().map(|node| node.map(bits)).collect()
}

/// Test 3, the identity (decision-r10-t1c-client-scope §2.5): on every synthetic world, whose
/// header carries no maturity, every node at every level and its parent's repeats the material's
/// photometry's albedo to the bit, the photometry as built; the class map's texels carry no
/// maturity and each named class's nodes repeat its photometry's albedo, an unnamed one's none;
/// and a bake's layer is empty.
#[test]
fn worker_nodes_repeat_the_photometry_without_a_maturity_on_every_world() {
    for &w in SyntheticWorld::ALL {
        let field = world(w);
        let header = field.header();
        assert_eq!(header.maturity(), None);
        let (mut fields, _) = surveyed_about(field, unit_dir([0.3, 1.0, 0.2]));
        let body = header.body();
        let palette = header.palette();
        for entry in 0..u8::try_from(palette.len()).unwrap() {
            for &class in MaterialClass::ALL {
                let m = Material::new(class, entry).unwrap();
                let albedo = fields.photometry(body, m).unwrap().normal_albedo_bvr();
                for level in [0, 7, 14, MAX_LEVEL] {
                    let nodes = fields.maturity_nodes(body, m, band(level)).unwrap();
                    let repeated = node_bits(&[albedo; MATURITY_NODES]);
                    assert_eq!(node_bits(nodes.level()), repeated, "{w:?} {m:?} {level}");
                    assert_eq!(node_bits(nodes.parent()), repeated, "{w:?} {m:?} {level}");
                }
            }
        }
        let map = fields.class_map(body, SEED).unwrap();
        assert!(map.texels().iter().all(|t| t.maturity().is_none()), "{w:?}");
        for class in map.classes() {
            match class.photometry() {
                Some(p) => assert_eq!(
                    class.nodes().map(node_bits),
                    Some(node_bits(&[p.normal_albedo_bvr(); MATURITY_NODES]))
                ),
                None => assert_eq!(class.nodes(), None),
            }
        }
        let key = PatchKey::containing(10, unit_dir([0.3, 1.0, 0.2])).unwrap();
        let bake = fields.bake(body, SEED, key, &MESH).unwrap();
        assert!(bake.terrain().layers().maturity().is_empty(), "{w:?}");
    }
    let past = Material::new(MaterialClass::Regolith, 9).unwrap();
    let f = fixture();
    let mut fields = f.worker();
    assert_eq!(
        fields.maturity_nodes(f.body, past, band(3)),
        Err(ResolvePhotometryError::Properties(
            ResolvePropertiesError::EntryNotInPalette {
                entry: 9,
                len: world(WORLD).header().palette().len()
            }
        ))
    );
    let other = BodyRef::new(f.body.raw_system_id(), f.body.body_index() ^ 1);
    assert_eq!(
        fields.maturity_nodes(other, past, band(3)),
        Err(ResolvePhotometryError::NotStarted(other))
    );
}

/// The worker's nodes on `TEST MOON` weathered and on the 3.5 Gyr world: at each level they are
/// `maturity_nodes` over the level's unresolved mix of the material's drawn albedo, made afresh,
/// to the bit, asked in any order and again; a level's parent nodes are the level below's own;
/// the band limit's are the drawn albedo at each node's maturity, the last the photometry's; and
/// band level 0's coarser mix brightens a weathered material's nodes at `m_body`.
#[test]
fn worker_nodes_are_each_level_s_unresolved_mean_of_the_drawn_albedo() {
    for field in [weathered_terrain_world(), middle_weathered_world()] {
        let header = field.header();
        let body = header.body();
        let m_body = header.maturity().unwrap();
        let terms = unresolved_terms(header).unwrap();
        let limit = band_limit_level(field);
        let materials: Vec<Material> = [MaterialClass::Regolith, MaterialClass::Melt]
            .into_iter()
            .flat_map(|class| (0..2).map(move |entry| Material::new(class, entry).unwrap()))
            .collect();
        let levels = [limit, 0, 9, limit + 1, 14, 1, 9];
        let ask = |fields: &mut WorkerFields, order: &[u8]| -> Vec<(u8, Material, DrawnNodes)> {
            let mut answers = Vec::new();
            for &level in order {
                for &m in &materials {
                    answers.push((
                        level,
                        m,
                        fields.maturity_nodes(body, m, band(level)).unwrap(),
                    ));
                }
            }
            answers.sort_by_key(|&(level, m, _)| (level, m));
            answers
        };
        let (mut fields, _) = surveyed_about(field, unit_dir([0.3, 1.0, 0.2]));
        let answers = ask(&mut fields, &levels);
        let mut reversed: Vec<u8> = levels.to_vec();
        reversed.reverse();
        let (mut again, _) = surveyed_about(field, unit_dir([0.3, 1.0, 0.2]));
        assert_eq!(ask(&mut again, &reversed), answers);
        for (level, m, nodes) in &answers {
            let g = drawn_on(header, *m);
            let at = |n: u8| maturity_nodes(&unresolved_mix(&terms, band(n)), &g);
            assert_eq!(
                node_bits(nodes.level()),
                node_bits(&at(*level)),
                "{m:?} {level}"
            );
            let parent = level.saturating_sub(1);
            assert_eq!(
                node_bits(nodes.parent()),
                node_bits(&at(parent)),
                "{m:?} {level}"
            );
            if *level >= limit {
                for (j, node) in nodes.level().iter().enumerate() {
                    assert_eq!(node.map(bits), g(m_body * maturity_node_share(j)).map(bits));
                }
                let photometry = fields.photometry(body, *m).unwrap().normal_albedo_bvr();
                assert_eq!(
                    nodes.level()[MATURITY_NODES - 1].map(bits),
                    photometry.map(bits)
                );
            }
        }
        let regolith = Material::new(MaterialClass::Regolith, 0).unwrap();
        let coarse = fields.maturity_nodes(body, regolith, band(0)).unwrap();
        let finest = fields.maturity_nodes(body, regolith, band(limit)).unwrap();
        let last = MATURITY_NODES - 1;
        assert!(
            coarse.level()[last][1] > finest.level()[last][1],
            "the unresolved haloes brighten the background: {:?} against {:?}",
            coarse.level()[last],
            finest.level()[last]
        );
    }
}

/// Test 7 (decision-r10-t1c-client-scope §2.5; decision-r10-t1c-maturity §5, test 4): on `TEST
/// MOON` weathered, each texel's maturity is the code of the point rule at band level 0 at its
/// centre, to the bit, and each class's nodes are the worker's at band level 0; and on a weathered
/// field of level 7, whose texels each read their four surveyed cells' centres, it is the code of
/// those points' mean.
#[test]
fn worker_class_map_texel_maturity_is_the_point_rule_at_band_zero() {
    let field = weathered_terrain_world();
    let header = field.header();
    let body = header.body();
    let m_body = header.maturity().unwrap();
    let (mut fields, cells) = surveyed_about(field, unit_dir([0.3, 1.0, 0.2]));
    let map = fields.class_map(body, SEED).unwrap();
    assert_eq!(map.texels().len(), cells.len());
    let held = fields.field(body).unwrap().clone();
    let synth = Synthesiser::new(&held, SEED);
    let mut immature = 0;
    for texel in map.texels() {
        let at = face_uv(texel.cell(), 0.5, 0.5);
        let m = maturity_at_level(&synth, &mut SynthCache::new(), at, band(CLASS_MAP_BAND))
            .unwrap()
            .unwrap();
        assert_eq!(
            texel.maturity(),
            Some(encode_maturity(m, m_body)),
            "{:?}",
            texel.cell()
        );
        immature += usize::from(m < m_body);
    }
    eprintln!(
        "{immature} of {} texels below m_body at band 0",
        map.texels().len()
    );
    for class in map.classes() {
        let Some(m) = class.material().material() else {
            assert_eq!(class.nodes(), None);
            continue;
        };
        let want = *fields.maturity_nodes(body, m, band(0)).unwrap().level();
        assert_eq!(class.nodes().map(node_bits), Some(node_bits(&want)));
    }
    // A weathered field of level 7, four cells a texel: a whole texel reads its centre, and one
    // with three of its four cells surveyed, not its centre's, reads their centres, in the halo of
    // a young coarse crater a texel beyond it.
    let part = texel(22, 20);
    let deeper = FieldBuilder::new(Metres::new(3.39e6))
        .maturity(0.6)
        .weathering_rate(1.0)
        .crater(
            point(texel(23, 20), 0.5, 0.5),
            Metres::new(150_000.0),
            Gigayears::new(0.05),
            0,
        )
        .build();
    let level = deeper.header().level().get();
    assert_eq!(level, CLASS_MAP_LEVEL + 1);
    let centre_cell = PatchKey::containing(level, point(part, 0.5, 0.5)).unwrap();
    let mut surveyed: BTreeSet<PatchKey> = texel(20, 20).children().into_iter().collect();
    surveyed.extend(part.children().into_iter().filter(|&c| c != centre_cell));
    let blocks = split(&encode_within(
        &deeper,
        &cover(&surveyed, |_| code(50.0)),
        None,
        SYNTHESIS_MARGIN_CELLS,
        FIXTURE_BLOCK_BYTES,
    ));
    let body = deeper.header().body();
    let mut fields = WorkerFields::new();
    fields.init(body, &blocks[0]).unwrap();
    for block in &blocks[1..] {
        fields.chunk(body, block).unwrap();
    }
    let map = fields.class_map(body, SEED).unwrap();
    let held = fields.field(body).unwrap().clone();
    let synth = Synthesiser::new(&held, SEED);
    let at_level = |at: FaceUv| {
        maturity_at_level(&synth, &mut SynthCache::new(), at, band(CLASS_MAP_BAND))
            .unwrap()
            .unwrap()
    };
    let texels: Vec<PatchKey> = map.texels().iter().map(ClassMapTexel::cell).collect();
    assert_eq!(texels, [texel(20, 20), part]);
    let whole = at_level(face_uv(texel(20, 20), 0.5, 0.5));
    assert_eq!(
        map.texels()[0].maturity(),
        Some(encode_maturity(whole, 0.6))
    );
    let read: Vec<f64> = surveyed
        .iter()
        .filter(|c| part.children().contains(c))
        .map(|&c| at_level(face_uv(c, 0.5, 0.5)))
        .collect();
    assert_eq!(read.len(), 3);
    let mean = read.iter().fold(0.0, |sum, m| sum + m) / 3.0;
    let mean = if mean < 0.6 { mean } else { 0.6 };
    eprintln!("the part's three maturities {read:?}, mean {mean}");
    assert!(
        read.iter().any(|&m| m < 0.6),
        "the part lies in the crater's halo"
    );
    assert_eq!(map.texels()[1].maturity(), Some(encode_maturity(mean, 0.6)));
}

/// Test 9 (decision-r10-t1c-client-scope §2.4 and §2.5): at each vertex of a band-limit-level
/// patch whose weights name one slot whole, the albedo the terrain draws there (its level-n code
/// decoded, the slot's level nodes interpolated) is the worker's material query's albedo there
/// under the header's scale, within the interpolation's and the codes' stated errors (tests 5 and
/// 6: 0.5% at a scale of 1 and 1.2% where a range clamps, plus the unorm16 code's 0.25%), on the
/// 3.5 Gyr world at a scale of 1 and on `TEST MOON` weathered at its haloed scale; printed.
///
/// The clamp corner's error depends on the scale c: at K = 16, 0.495% at the fixture's c of
/// 1.22997 and 0.86% at the 3.5 Gyr world's haloed c, but up to 2.19% for c from 1.0 to 1.8 on the
/// Moon-like palette (1.73% at K = 20, 1.44% at 24 and 1.07% at 32; the science checker's
/// replica), so the 1.2% holds for the tested scales alone (plan R10's Risks, R10.T1.c's
/// client-half deferred findings).
#[test]
fn worker_drawn_albedo_at_a_vertex_is_the_material_query_s() {
    for (field, interpolation) in [
        (middle_weathered_world(), 0.005),
        (weathered_terrain_world(), 0.012),
    ] {
        let header = field.header();
        let body = header.body();
        let m_body = header.maturity().unwrap();
        let scale = header.albedo_scale().unwrap_or(1.0);
        let limit = band_limit_level(field);
        let dir = unit_dir([0.3, 1.0, 0.2]);
        let (mut fields, _) = surveyed_about(field, dir);
        let key = PatchKey::containing(limit, dir).unwrap();
        let bake = fields.bake(body, SEED, key, &MESH).unwrap();
        let layers = bake.terrain().layers().clone();
        let (mut worst, mut compared, mut immature) = (0.0_f64, 0, 0);
        let step = if cfg!(target_arch = "wasm32") { 8 } else { 2 };
        for y in (0..=PATCH_QUADS).step_by(step) {
            for x in (0..=PATCH_QUADS).step_by(step) {
                let n = usize::try_from(y * (PATCH_QUADS + 1) + x).unwrap();
                let Some(slot) = layers.weights()[n].iter().position(|&w| w == WEIGHT_TOTAL) else {
                    continue;
                };
                let Some(material) = layers.palette()[slot].and_then(GroundMaterial::material)
                else {
                    continue;
                };
                let answer = fields
                    .material(body, SEED, vertex_point(key, x, y))
                    .unwrap()
                    .unwrap();
                assert_eq!(answer.ground().material(), Some(material), "({x}, {y})");
                let want = scaled_normal_albedo(answer.properties().unwrap(), scale);
                let nodes = *fields
                    .maturity_nodes(body, material, band(limit))
                    .unwrap()
                    .level();
                let code = layers.maturity()[n][0];
                let got = maturity_node_albedo(&nodes, m_body, decode_maturity(code, m_body));
                for (g, w) in got.iter().zip(&want) {
                    let error = (g / w - 1.0).abs();
                    worst = if error > worst { error } else { worst };
                }
                compared += 1;
                immature += usize::from(code > 0);
            }
        }
        eprintln!(
            "{key:?}: {compared} vertices compared, {immature} below m_body; the worst drawn \
             albedo against the material query's {:.4}%",
            100.0 * worst
        );
        assert!(compared > 0);
        assert!(
            worst <= interpolation + 0.0025,
            "{worst} over {interpolation} + 0.0025 at c {scale}; where a range clamps, the 1.2% \
             holds for the tested scales alone, the error depending on c (up to 2.19% at K = 16 \
             for c 1.0-1.8 on the Moon-like palette; plan R10's R10.T1.c client-half deferred \
             findings)"
        );
    }
}
