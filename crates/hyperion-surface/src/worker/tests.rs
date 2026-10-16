//! The worker's fields' tests (plan R10, T6.b), on the Moon-like synthetic world through payloads
//! of two small surveys, cut into blocks small enough that each payload is several.

use std::collections::BTreeSet;

use hyperion_testkit::float::bits;
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::bake::{HorizonLayer, SURVEYED, WeightsLayer};
use crate::cube::{Face, st_to_uv};
use crate::field::{
    CoarseField, CoarseLevel, Cover, CoverRange, ResolutionCode, SYNTHESIS_MARGIN_CELLS,
    SynthesisCell,
};
use crate::material::MaterialClass;
use crate::patch::{BakeOptions, NormalScale, VertexPath};
use crate::synth::test_support::{SEED, world};
use crate::testing::SyntheticWorld;
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
    assert!(inside.unsurveyed_cells().is_empty());
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
                Some(SurveyedMaterial { ground, properties })
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
/// baseline, a line of sight, or a material at a point.
#[derive(Debug, Clone, Copy)]
enum Query {
    Bake(usize),
    Reading(usize, f64),
    Sight(usize),
    Material(usize),
}

/// A [`Query`]'s answer.
#[derive(Debug, PartialEq)]
enum Answer {
    Bake(Box<Result<BodyBake, QueryBodyError>>),
    Reading(Result<Option<SurveyedReading>, QueryBodyError>),
    Sight(Result<LineOfSight, QueryBodyError>),
    Material(Result<Option<SurveyedMaterial>, QueryBodyError>),
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

/// Every kind of query, interleaved.
fn queries() -> Vec<Query> {
    let mut queries = Vec::new();
    for n in 0..probes().len() {
        queries.push(Query::Bake(n));
        queries.push(Query::Reading(n % 4, if n % 2 == 0 { 2.0 } else { 100.0 }));
        queries.push(Query::Sight(n % 3));
        queries.push(Query::Material(n % 4));
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
