//! The payload codec's tests (plan R09, T3): round trips, the partial field's merging and
//! refusals, the sizes, damaged bytes, and the goldens of the codes and the payloads.

use std::sync::OnceLock;

use hyperion_base::units::Kelvin;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::form::{
    ABSENT, PRESENT, SCREENING_ATMOSPHERE, SCREENING_CUTOFF, SCREENING_NONE, unknown,
};
use super::*;
use crate::craters::Screening;
use crate::field::{
    BoundaryKind, BuildPaletteError, ClimateModelKind, Crust, FieldView, FlowDirection,
    InsertBlockError, LogArea, LogPrecipitation, LogSteepness, MechanicsFamily, NO_ENTRY,
    PaletteRole, PartialField, PrecipitationSource, SurfaceClass, Wind, cell_at_index,
    crater_key_order,
};
use crate::substance_key::ParseSubstanceKeyError;
use crate::testing::{SyntheticWorld, synthetic_field};

/// The Earth-like world, built once for the tests that read it (0.6–0.8 s a build).
fn earth() -> &'static CoarseField {
    static EARTH: OnceLock<CoarseField> = OnceLock::new();
    EARTH.get_or_init(|| synthetic_field(SyntheticWorld::EarthLike))
}

/// The Mars-like world, built once.
fn mars() -> &'static CoarseField {
    static MARS: OnceLock<CoarseField> = OnceLock::new();
    MARS.get_or_init(|| synthetic_field(SyntheticWorld::MarsLike))
}

/// The cover of every cell of `field`, surveyed at code 40 (about 0.29 m).
fn whole(field: &CoarseField) -> Cover {
    let cells = field.header().level().cell_count();
    Cover::from_ranges([CoverRange::new(0, cells, ResolutionCode::new(40)).unwrap()]).unwrap()
}

fn range(start: u32, end: u32, code: u8) -> CoverRange {
    CoverRange::new(start, end, ResolutionCode::new(code)).unwrap()
}

/// A survey of a level-7 field: the 32 × 32 cells at face 0's corner (i, j < 32), which touches
/// two face edges and a cube corner, at code 30; an unaligned run of face 2 at code 12; and one
/// cell of face 5 at code 100.
fn region() -> Cover {
    let face = 1 << 14;
    Cover::from_ranges([
        range(0, 1_024, 30),
        range(2 * face + 5_000, 2 * face + 5_101, 12),
        range(5 * face + 777, 5 * face + 778, 100),
    ])
    .unwrap()
}

/// The blocks of `bytes`, which must decode.
fn blocks_of(bytes: &[u8]) -> Vec<DecodedBlock> {
    decode_payload(bytes).unwrap()
}

/// The partial field of `blocks`, inserted in `order`.
fn partial(blocks: &[DecodedBlock], order: impl IntoIterator<Item = usize>) -> PartialField {
    let mut field = PartialField::new(blocks[0].header().unwrap().clone());
    for k in order {
        field.insert(&blocks[k]).unwrap();
    }
    field
}

/// Asserts that `a` and `b` answer alike for every cell of their level and for a cell of another.
fn assert_same_answers(a: &impl FieldView, b: &impl FieldView, what: &str) {
    assert_eq!(a.header(), b.header(), "{what}");
    let level = a.header().level();
    for index in 0..level.cell_count() {
        let cell = cell_at_index(level.get(), index).unwrap();
        assert_eq!(a.cell(cell), b.cell(cell), "{what}, cell {index}");
        assert_eq!(a.climate(cell), b.climate(cell), "{what}, cell {index}");
        assert!(
            a.craters_reaching(cell).eq(b.craters_reaching(cell)),
            "{what}, cell {index}'s craters"
        );
    }
    let coarser = cell_at_index(level.get() - 1, 3).unwrap();
    assert_eq!(a.cell(coarser), None, "{what}");
    assert_eq!(b.cell(coarser), None, "{what}");
}

/// The field the whole payload's blocks hold, rebuilt from them: their cells in order, each climate
/// record once, and each crater once.
fn reassemble(blocks: &[DecodedBlock]) -> CoarseField {
    let header = blocks[0].header().unwrap().clone();
    let mut synthesis = Vec::new();
    let mut climate: Vec<(u32, ClimateCell)> = Vec::new();
    let mut craters: Vec<(u32, CoarseCrater)> = Vec::new();
    for block in blocks {
        synthesis.extend_from_slice(block.cells());
        for (&index, record) in block.climate_indices().iter().zip(block.climate()) {
            if climate.last().is_some_and(|&(last, _)| last == index) {
                assert_eq!(climate.last().unwrap().1, *record, "climate cell {index}");
            } else {
                climate.push((index, *record));
            }
        }
        craters.extend(
            block
                .crater_centres()
                .iter()
                .copied()
                .zip(block.craters().iter().cloned()),
        );
    }
    craters.sort_by(|(a, x), (b, y)| crater_key_order((*a, x.diameter), (*b, y.diameter)));
    craters.dedup_by(|(a, x), (b, y)| {
        let same = crater_key_order((*a, x.diameter), (*b, y.diameter)).is_eq();
        assert!(!same || x == y, "two blocks disagree on a crater");
        same
    });
    CoarseField::new(
        header,
        synthesis,
        climate.into_iter().map(|(_, c)| c).collect(),
        craters.into_iter().map(|(_, c)| c).collect(),
    )
    .unwrap()
}

/// Where two byte strings first differ, a length's end included, or `None` if they are the same.
fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    a.iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .or_else(|| (a.len() != b.len()).then(|| a.len().min(b.len())))
}

/// Every synthetic world's whole payload decodes to its field exactly: rebuilt from the blocks it
/// is the same field, and encodes to the same bytes again; and a partial field of the blocks
/// answers as the field does for every cell, each one surveyed at its code.
#[test]
fn a_whole_wire_payload_round_trips_to_identical_bytes_and_answers() {
    for &world in SyntheticWorld::ALL {
        let built;
        let field = match world {
            SyntheticWorld::EarthLike => earth(),
            SyntheticWorld::MarsLike => mars(),
            _ => {
                built = synthetic_field(world);
                &built
            }
        };
        let cover = whole(field);
        let bytes = encode_payload(field, &cover, None);
        assert_eq!(
            first_difference(&encode_payload(field, &cover, None), &bytes),
            None,
            "{world:?} encodes to other bytes a second time"
        );
        let blocks = blocks_of(&bytes);
        let rebuilt = reassemble(&blocks);
        assert_eq!(&rebuilt, field, "{world:?}");
        assert_eq!(
            first_difference(&encode_payload(&rebuilt, &cover, None), &bytes),
            None,
            "{world:?}'s rebuilt field encodes to other bytes"
        );
        let held = partial(&blocks, 0..blocks.len());
        assert_same_answers(&held, field, &format!("{world:?}"));
        let level = field.header().level().get();
        for index in [0, 1, field.header().level().cell_count() - 1] {
            let cell = cell_at_index(level, index).unwrap();
            assert!(held.is_surveyed(cell), "{world:?}, cell {index}");
            assert_eq!(held.resolution(cell), Some(ResolutionCode::new(40)));
        }
    }
}

/// Block 0 of every payload carries the field's header and builds a partial field with it alone,
/// for a survey's payload, a delta, a delta with nothing new and the whole field in three blocks;
/// no other block carries a header.
#[test]
fn wire_block_zero_alone_carries_the_field_header_whole_or_delta() {
    let field = mars();
    let first = Cover::from_ranges([range(0, 1_024, 30)]).unwrap();
    let all = whole(field);
    for (what, cover, since) in [
        ("survey", region(), None),
        ("delta", region(), Some(&first)),
        ("empty delta", region(), Some(&region())),
        ("whole", all.clone(), None),
        ("whole delta", all.clone(), Some(&first)),
    ] {
        let blocks = blocks_of(&encode_payload(field, &cover, since));
        assert!(blocks.len() >= 3 || !what.starts_with("whole"), "{what}");
        let header = blocks[0].header().unwrap_or_else(|| panic!("{what}"));
        assert_eq!(header, field.header(), "{what}");
        assert!(blocks[1..].iter().all(|b| b.header().is_none()), "{what}");
        let mut held = PartialField::new(header.clone());
        held.insert(&blocks[0]).unwrap();
        assert_eq!(held.header(), field.header(), "{what}");
    }
    let empty = blocks_of(&encode_payload(field, &region(), Some(&region())));
    assert_eq!(empty.len(), 1);
    assert!(empty[0].cover().is_empty());
    assert!(empty[0].cells().is_empty() && empty[0].craters().is_empty());
}

/// A partial field of a survey holds the survey's cells at their codes and their margin of
/// `SYNTHESIS_MARGIN_CELLS` king moves as held but not surveyed, answers there as the field does,
/// and answers `None`, or nothing, everywhere else.
#[test]
fn a_wire_partial_field_answers_none_outside_what_it_holds() {
    let field = mars();
    let level = field.header().level();
    let survey = region();
    let held_cover = survey.with_margin(level, SYNTHESIS_MARGIN_CELLS);
    let blocks = blocks_of(&encode_payload(field, &survey, None));
    let held = partial(&blocks, 0..blocks.len());
    let (mut margin, mut outside, mut faces) = (0, 0, [false; 6]);
    for index in 0..level.cell_count() {
        let cell = cell_at_index(level.get(), index).unwrap();
        if let Some(code) = held_cover.code(index) {
            assert_eq!(held.cell(cell), field.cell(cell), "cell {index}");
            assert_eq!(held.climate(cell), field.climate(cell), "cell {index}");
            assert!(held.craters_reaching(cell).eq(field.craters_reaching(cell)));
            assert_eq!(held.resolution(cell), Some(code), "cell {index}");
            assert_eq!(
                held.is_surveyed(cell),
                survey.contains(index),
                "cell {index}"
            );
            margin += usize::from(!survey.contains(index));
            faces[usize::from(cell.face().index())] = true;
        } else {
            assert_eq!(held.cell(cell), None, "cell {index}");
            assert_eq!(held.climate(cell), None, "cell {index}");
            assert_eq!(held.craters_reaching(cell).count(), 0, "cell {index}");
            assert_eq!(held.resolution(cell), None, "cell {index}");
            assert!(!held.is_surveyed(cell), "cell {index}");
            outside += 1;
        }
    }
    // The corner block's margin crosses onto the two other faces at face 0's corner (i = 0 and
    // j = 0 lead to faces 4 and 5 in S2's layout), so with face 2's run the field holds cells of
    // four faces, face 5's single cell among them.
    assert!(
        margin > 1_000 && outside > 90_000,
        "{margin} margin, {outside} outside"
    );
    assert_eq!(faces, [true, false, true, false, true, true], "faces held");
}

/// The fixed permutations the order test inserts blocks in: forwards, backwards, odd places then
/// even ones, the second half then the first, and a shuffle by a fixed seed.
fn orders(n: usize) -> Vec<Vec<usize>> {
    let mut shuffled: Vec<usize> = (0..n).collect();
    let mut g = Lcg::new(0x5eed_0003);
    for k in (1..n).rev() {
        let j = usize::try_from(g.next_below(u64::try_from(k + 1).unwrap())).unwrap();
        shuffled.swap(k, j);
    }
    vec![
        (0..n).collect(),
        (0..n).rev().collect(),
        (1..n).step_by(2).chain((0..n).step_by(2)).collect(),
        (n / 2..n).chain(0..n / 2).collect(),
        shuffled,
    ]
}

/// The blocks of a payload inserted forwards, backwards and in three fixed permutations give the
/// same partial field, every answer equal: the Moon-like world's dozens of craters, most reaching
/// many blocks, in blocks of 48 KiB, and the Mars-like world whole in blocks of 1 MiB.
#[test]
fn wire_blocks_inserted_in_any_order_give_equal_partial_fields() {
    let moon = synthetic_field(SyntheticWorld::MoonLike);
    let small = encode_within(&moon, &whole(&moon), None, SYNTHESIS_MARGIN_CELLS, 48 << 10);
    let whole_mars = encode_payload(mars(), &whole(mars()), None);
    for (what, field, bytes) in [
        ("Moon-like", &moon, small),
        ("Mars-like", mars(), whole_mars),
    ] {
        let blocks = blocks_of(&bytes);
        assert!(blocks.len() >= 3, "{what}: {} blocks", blocks.len());
        let carried: usize = blocks.iter().map(|b| b.craters().len()).sum();
        assert!(
            carried > field.craters().len(),
            "{what}: no crater travels in two blocks"
        );
        let mut fields = orders(blocks.len())
            .into_iter()
            .map(|order| partial(&blocks, order));
        let first = fields.next().unwrap();
        for (k, other) in fields.enumerate() {
            assert_eq!(other, first, "{what}: order {} differs", k + 1);
            assert_same_answers(&other, &first, &format!("{what}, order {}", k + 1));
        }
    }
}

/// A delta carries exactly the cells of the cover and its margin that the earlier cover and its
/// margin did not hold at the same code, and the partial field of the earlier payload with the
/// delta is the partial field of the whole cover's payload.
#[test]
fn a_wire_delta_carries_only_what_changed() {
    let field = mars();
    let level = field.header().level();
    let face = 1_u32 << 14;
    let since =
        Cover::from_ranges([range(0, 1_024, 30), range(3 * face, 3 * face + 64, 50)]).unwrap();
    // The first block again, finer; a new run; and the face 3 run unchanged.
    let cover = Cover::from_ranges([
        range(0, 1_024, 20),
        range(2 * face, 2 * face + 512, 60),
        range(3 * face, 3 * face + 64, 50),
    ])
    .unwrap();
    let delta = blocks_of(&encode_payload(field, &cover, Some(&since)));
    let sent: Vec<(u32, ResolutionCode)> = delta
        .iter()
        .flat_map(|b| {
            b.cover()
                .ranges()
                .iter()
                .flat_map(|r| (r.start()..r.end()).map(move |c| (c, r.code())))
        })
        .collect();
    let now = cover.with_margin(level, SYNTHESIS_MARGIN_CELLS);
    let before = since.with_margin(level, SYNTHESIS_MARGIN_CELLS);
    let expected: Vec<(u32, ResolutionCode)> = now
        .ranges()
        .iter()
        .flat_map(|r| (r.start()..r.end()).map(move |c| (c, r.code())))
        .filter(|&(c, code)| before.code(c) != Some(code))
        .collect();
    assert_eq!(sent, expected);
    assert!(
        sent.iter()
            .all(|&(c, _)| !(3 * face..3 * face + 64).contains(&c))
    );
    assert!(
        sent.iter()
            .any(|&(c, code)| c < 1_024 && code == ResolutionCode::new(20))
    );
    let earlier = blocks_of(&encode_payload(field, &since, None));
    let mut grown = partial(&earlier, 0..earlier.len());
    for block in &delta {
        grown.insert(block).unwrap();
    }
    let all = blocks_of(&encode_payload(field, &cover, None));
    assert_eq!(
        grown,
        partial(&all, 0..all.len()),
        "the delta's field differs"
    );
}

/// The blocks of an earlier payload and of its delta, inserted together in five orders, the
/// delta's before the earlier's among them, give the partial field of the whole cover's payload:
/// a cell surveyed twice keeps its finer code whichever block came first.
#[test]
fn wire_payloads_merged_in_any_order_give_the_whole_cover_s_field() {
    let field = synthetic_field(SyntheticWorld::CeresLike);
    let face = 1_u32 << 10;
    let since =
        Cover::from_ranges([range(0, 700, 30), range(3 * face, 3 * face + 64, 50)]).unwrap();
    let cover = Cover::from_ranges([
        range(0, 700, 20),
        range(2 * face, 2 * face + 300, 60),
        range(3 * face, 3 * face + 64, 50),
    ])
    .unwrap();
    let limit = 8 << 10;
    let earlier = encode_within(&field, &since, None, SYNTHESIS_MARGIN_CELLS, limit);
    let delta = encode_within(&field, &cover, Some(&since), SYNTHESIS_MARGIN_CELLS, limit);
    let mut blocks = blocks_of(&earlier);
    blocks.extend(blocks_of(&delta));
    assert!(blocks.len() >= 4, "{} blocks", blocks.len());
    let all = blocks_of(&encode_payload(&field, &cover, None));
    let expected = partial(&all, 0..all.len());
    let header = field.header().clone();
    for (k, order) in orders(blocks.len()).into_iter().enumerate() {
        let mut merged = PartialField::new(header.clone());
        for place in order {
            merged.insert(&blocks[place]).unwrap();
        }
        assert_eq!(merged, expected, "order {k}");
        assert_same_answers(&merged, &expected, &format!("order {k}"));
    }
}

/// The Earth-like world's whole payload is 10–15 MB (about 14.0 MB: Design note 17's 21 bytes a
/// cell with the substance and sea floor bytes, 23, and 50 per four cells, with each block's
/// header, the field header with its palette, and the craters), and no block of it, or of any
/// payload here, exceeds 1 MiB.
#[test]
fn no_wire_block_exceeds_a_mebibyte_and_an_earth_encodes_to_10_to_15_mb() {
    let field = earth();
    let bytes = encode_payload(field, &whole(field), None);
    let records = 393_216 * SynthesisCell::BYTES + 98_304 * ClimateCell::BYTES;
    assert!(
        (10_000_000..=15_000_000).contains(&bytes.len()),
        "{} bytes",
        bytes.len()
    );
    assert!(
        bytes.len() > records && bytes.len() < records + 4_096,
        "{} bytes",
        bytes.len()
    );
    let mut rest = &bytes[..];
    let mut blocks = 0;
    while !rest.is_empty() {
        let length = block_length(rest).unwrap();
        assert!(length <= MAX_BLOCK_BYTES, "block {blocks}: {length} bytes");
        rest = &rest[length..];
        blocks += 1;
    }
    assert_eq!(blocks, 14, "{} bytes", bytes.len());
    for bytes in [
        encode_payload(mars(), &region(), None),
        encode_payload(mars(), &whole(mars()), None),
    ] {
        assert!(!blocks_of(&bytes).is_empty());
        assert!(bytes.len() <= MAX_BLOCK_BYTES * blocks_of(&bytes).len());
    }
}

/// The offsets of each block of `bytes`.
fn starts(bytes: &[u8]) -> Vec<usize> {
    let mut starts = vec![0];
    let mut at = 0;
    while at < bytes.len() {
        at += block_length(&bytes[at..]).unwrap();
        starts.push(at);
    }
    starts.pop();
    starts
}

/// Writes `value` at `at`, little-endian.
fn patch(bytes: &mut [u8], at: usize, value: &[u8]) {
    bytes[at..at + value.len()].copy_from_slice(value);
}

/// An empty payload, a truncated block, a payload cut at a block boundary, and a block of another
/// magic or format, oversize or undersize, are each refused with their own error.
#[test]
fn damaged_wire_payloads_are_refused_with_their_errors() {
    let (_, bytes, at) = ceres_blocks();
    let n = at.len();
    let refused = |bytes: &[u8]| decode_payload(bytes).unwrap_err();
    let block = |block: usize, error: DecodeBlockError| DecodePayloadError::Block { block, error };
    assert_eq!(refused(&[]), DecodePayloadError::Empty);
    let cut = refused(&bytes[..bytes.len() - 1]);
    let truncated = matches!(
        cut,
        DecodePayloadError::Block {
            block,
            error: DecodeBlockError::Truncated { .. },
        } if block == n - 1
    );
    assert!(truncated, "{cut:?}");
    assert_eq!(
        refused(&bytes[..at[n - 1]]),
        DecodePayloadError::MissingBlocks {
            count: u32::try_from(n).unwrap(),
            found: n - 1
        }
    );
    assert_eq!(
        refused(&bytes[..5]),
        block(
            0,
            DecodeBlockError::Truncated {
                needed: 6,
                available: 5
            }
        )
    );
    let damaged = |at: usize, value: &[u8]| {
        let mut b = bytes.clone();
        patch(&mut b, at, value);
        refused(&b)
    };
    assert_eq!(
        damaged(0, b"HYSG"),
        block(0, DecodeBlockError::Magic(*b"HYSG"))
    );
    assert_eq!(
        damaged(4, &2_u16.to_le_bytes()),
        block(0, DecodeBlockError::Format(2))
    );
    let oversize = u32::try_from(MAX_BLOCK_BYTES).unwrap() + 1;
    assert_eq!(
        damaged(6, &oversize.to_le_bytes()),
        block(0, DecodeBlockError::Oversize { length: oversize })
    );
    assert_eq!(
        damaged(6, &32_u32.to_le_bytes()),
        block(0, DecodeBlockError::Undersize { length: 32 })
    );
}

/// The Ceres-like world's whole payload in blocks of 16 KiB, with the offset of each block.
fn ceres_blocks() -> (CoarseField, Vec<u8>, Vec<usize>) {
    let ceres = synthetic_field(SyntheticWorld::CeresLike);
    let bytes = encode_within(
        &ceres,
        &whole(&ceres),
        None,
        SYNTHESIS_MARGIN_CELLS,
        16 << 10,
    );
    let at = starts(&bytes);
    assert!(at.len() >= 4, "{} blocks", at.len());
    (ceres, bytes, at)
}

/// A block of another generator version, level or body, out of place or out of its count, that
/// does not end where its length says, or with an unknown code is refused with its own error;
/// and a block decoded alone is exactly its bytes.
#[test]
fn misplaced_wire_blocks_are_refused_with_their_errors() {
    let (ceres, bytes, at) = ceres_blocks();
    let n = at.len();
    let block = |block: usize, error: DecodeBlockError| DecodePayloadError::Block { block, error };
    let damaged = |at: usize, value: &[u8]| {
        let mut b = bytes.clone();
        patch(&mut b, at, value);
        decode_payload(&b).unwrap_err()
    };
    let version = crate::generator_version();
    assert_eq!(
        damaged(at[1] + 10, &(version + 1).to_le_bytes()),
        block(
            1,
            DecodeBlockError::GeneratorVersion {
                found: version + 1,
                expected: version
            }
        )
    );
    assert_eq!(
        damaged(at[2] + 24, &[9]),
        block(2, DecodeBlockError::Level(9))
    );
    let count = u32::try_from(n).unwrap();
    assert_eq!(
        damaged(at[2] + 25, &count.to_le_bytes()),
        block(
            2,
            DecodeBlockError::Index {
                index: count,
                count
            }
        )
    );
    assert_eq!(
        damaged(at[2] + 25, &3_u32.to_le_bytes()),
        DecodePayloadError::Sequence {
            block: 2,
            index: 3,
            count
        }
    );
    let body = ceres.header().body();
    let other = BodyRef::new(body.raw_system_id(), body.body_index() + 1);
    assert_eq!(
        damaged(at[3] + 22, &other.body_index().to_le_bytes()),
        DecodePayloadError::WrongBody {
            block: 3,
            found: other,
            expected: body
        }
    );
    // A block 1 that declares a byte more than it holds swallows block 2's first byte.
    let length = u32::try_from(at[2] - at[1]).unwrap();
    assert!(matches!(
        damaged(at[1] + 6, &(length + 1).to_le_bytes()),
        DecodePayloadError::Block {
            block: 1,
            error: DecodeBlockError::TrailingBytes { .. }
        }
    ));
    // Block 1's first record's crust, after its fixed header, its cover's count and its ranges.
    let ranges = usize::try_from(u32::from_le_bytes(
        bytes[at[1] + 33..at[1] + 37].try_into().unwrap(),
    ))
    .unwrap();
    let crust = at[1] + 37 + 9 * ranges + 7;
    assert_eq!(damaged(crust, &[9]), block(1, unknown("Crust", 9)));
    // One block alone: exactly its bytes, not one more.
    let one = &bytes[at[1]..at[2]];
    assert_eq!(decode_block(one).unwrap().index(), 1);
    let mut longer = one.to_vec();
    longer.push(0);
    assert_eq!(
        decode_block(&longer),
        Err(DecodeBlockError::TrailingBytes {
            length: one.len(),
            used: one.len() + 1
        })
    );
}

/// The little-endian bytes of `value`'s wire form.
fn f64_bytes(value: f64) -> Vec<u8> {
    let mut out = Vec::new();
    value.put(&mut out);
    out
}

/// A `u32` of `bytes` at `at`.
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// `block`, a block after block 0 whose cover is one range, with that range split in two adjacent
/// ranges of its code.
fn split_first_range(block: &[u8]) -> Vec<u8> {
    assert_eq!(u32_at(block, 33), 1);
    let (start, end, code) = (u32_at(block, 37), u32_at(block, 41), block[45]);
    assert!(end > start + 1);
    let mut out = block[..33].to_vec();
    out.extend_from_slice(&2_u32.to_le_bytes());
    for (s, e) in [(start, start + 1), (start + 1, end)] {
        out.extend_from_slice(&s.to_le_bytes());
        out.extend_from_slice(&e.to_le_bytes());
        out.push(code);
    }
    out.extend_from_slice(&block[46..]);
    let length = u32::try_from(out.len()).unwrap();
    patch(&mut out, 6, &length.to_le_bytes());
    out
}

/// The byte spans of `block`'s craters, a block after block 0, from its decoded counts.
fn crater_spans(block: &[u8]) -> Vec<(usize, usize)> {
    let decoded = decode_block(block).unwrap();
    let cells = usize::try_from(decoded.cover().cell_count()).unwrap();
    let mut at = 33
        + 4
        + 9 * decoded.cover().ranges().len()
        + SynthesisCell::BYTES * cells
        + ClimateCell::BYTES * decoded.climate().len()
        + 4;
    let mut spans = Vec::new();
    for crater in decoded.craters() {
        let length = CRATER_FIXED_BYTES + REACH_RANGE_BYTES * crater.reach.ranges().len();
        spans.push((at, at + length));
        at += length;
    }
    spans
}

/// Block 0's header parts out of their ranges, an unknown screening tag, a header length its
/// parts do not fill, a header of another body or level than its block, an empty or outlying
/// cover range, a non-canonical cover, a record with water below its ground, and two craters out
/// of order are each refused with their own error.
#[test]
fn malformed_wire_parts_are_refused_with_their_errors() {
    let (ceres, bytes, at) = ceres_blocks();
    let block = |block: usize, error: DecodeBlockError| DecodePayloadError::Block { block, error };
    let damaged = |at: usize, value: &[u8]| {
        let mut b = bytes.clone();
        patch(&mut b, at, value);
        decode_payload(&b).unwrap_err()
    };
    // Block 0's header section: its length at 33, then the body at 35, the radius at 45, the
    // spectrum's exponent at 85, its break degree at 101 and its small-scale exponent at 109,
    // N(>1 km) at 117 and the screening's tag at 125.
    let body = ceres.header().body();
    let other = BodyRef::new(body.raw_system_id(), body.body_index() + 1);
    let header_body = DecodeBlockError::HeaderBody {
        header: other,
        block: body,
    };
    assert_eq!(
        damaged(43, &other.body_index().to_le_bytes()),
        block(0, header_body)
    );
    let radius = DecodeBlockError::Header(BuildFieldHeaderError::Radius(-1.0));
    assert_eq!(damaged(45, &f64_bytes(-1.0)), block(0, radius));
    let spectrum = DecodeBlockError::Spectrum(BuildBandSpectrumError::Exponent(0.5));
    assert_eq!(damaged(85, &f64_bytes(0.5)), block(0, spectrum));
    let break_degree = DecodeBlockError::Spectrum(BuildBandSpectrumError::BreakDegree(0.5));
    assert_eq!(damaged(101, &f64_bytes(0.5)), block(0, break_degree));
    let small = DecodeBlockError::Spectrum(BuildBandSpectrumError::SmallScaleExponent(1.0));
    assert_eq!(damaged(109, &f64_bytes(1.0)), block(0, small));
    let density = DecodeBlockError::CraterParams(BuildCraterParamsError::Density(-1.0));
    assert_eq!(damaged(117, &f64_bytes(-1.0)), block(0, density));
    assert_eq!(ceres.header().craters().screening(), Screening::None);
    let tag = DecodeBlockError::Tag {
        part: "screening",
        code: 7,
    };
    assert_eq!(damaged(125, &[7]), block(0, tag));
    let declared = u16::from_le_bytes([bytes[33], bytes[34]]);
    for wrong in [declared - 1, declared + 1] {
        let error = DecodeBlockError::HeaderLength { declared: wrong };
        assert_eq!(damaged(33, &wrong.to_le_bytes()), block(0, error));
    }
    let level = DecodeBlockError::HeaderLevel {
        header: CoarseLevel::MIN,
        block: CoarseLevel::new(6).unwrap(),
    };
    assert_eq!(damaged(24, &[6]), block(0, level));
    // The header section's last parts: the Ceres-like palette (a count and three entries of 59
    // bytes), each crust's entry (four options, the lid's present: five bytes) and the main
    // liquid's (absent: one byte).
    let end = 35 + usize::from(declared);
    let palette = end - 6 - (1 + 3 * 59);
    assert_eq!(bytes[palette], 3);
    let unsorted = DecodeBlockError::Palette(BuildPaletteError::Unsorted { entry: 1 });
    let second_role = palette + 1 + 59 + 16;
    assert_eq!(
        bytes[second_role],
        u8::from(PaletteRole::Ice),
        "the second entry is the ice"
    );
    assert_eq!(
        damaged(second_role, &[u8::from(PaletteRole::PrimaryCrust)]),
        block(0, unsorted)
    );
    let key = DecodeBlockError::SubstanceKey(ParseSubstanceKeyError::Malformed { at: 2 });
    assert_eq!(damaged(palette + 1, b"P"), block(0, key));
    let crust = DecodeBlockError::Header(BuildFieldHeaderError::CrustPalette {
        crust: Crust::Lid,
        entry: 1,
    });
    assert_eq!(bytes[end - 4..end], [PRESENT, 0, ABSENT, ABSENT]);
    assert_eq!(damaged(end - 3, &[1]), block(0, crust));
    // Block 1's cover, one range at 37, and its first record after it.
    let start = u32_at(&bytes, at[1] + 37);
    let empty = DecodeBlockError::Cover(BuildCoverError::EmptyRange { start, end: start });
    assert_eq!(damaged(at[1] + 41, &start.to_le_bytes()), block(1, empty));
    let outside = DecodeBlockError::CoverOutsideField { end: 6_145 };
    assert_eq!(
        damaged(at[1] + 41, &6_145_u32.to_le_bytes()),
        block(1, outside)
    );
    let dry = DecodeBlockError::Record(BuildFieldError::WaterBelowGround { cell: start });
    assert_eq!(
        damaged(at[1] + 46 + 14, &i32::MIN.to_le_bytes()),
        block(1, dry)
    );
    // The record's last byte, its sea floor: a lid has none.
    let seafloor = DecodeBlockError::Record(BuildFieldError::Seafloor { cell: start });
    assert_eq!(damaged(at[1] + 46 + 22, &[0x10]), block(1, seafloor));
    let one = &bytes[at[1]..at[2]];
    assert_eq!(
        decode_block(&split_first_range(one)),
        Err(DecodeBlockError::CoverNotCanonical)
    );
    // The first block after block 0 with two craters, its first two swapped.
    let ends: Vec<usize> = at.iter().skip(1).copied().chain([bytes.len()]).collect();
    let (from, to, spans) = (1..at.len())
        .map(|k| (at[k], ends[k], crater_spans(&bytes[at[k]..ends[k]])))
        .find(|(_, _, spans)| spans.len() >= 2)
        .expect("a Ceres-like block carries two craters");
    let mut swapped = bytes[from..to].to_vec();
    let ((a0, a1), (b0, b1)) = (spans[0], spans[1]);
    let (first, second) = (swapped[a0..a1].to_vec(), swapped[b0..b1].to_vec());
    swapped.splice(a0..b1, second.into_iter().chain(first));
    let unsorted = BuildFieldError::CratersUnsorted { crater: 1 };
    assert_eq!(
        decode_block(&swapped),
        Err(DecodeBlockError::Record(unsorted))
    );
}

/// A crater that reaches none of its block's cells, a block of another level than block 0, and a
/// block whose craters' reaches would exceed the index's budget are refused with their errors.
#[test]
fn wire_craters_and_levels_out_of_place_are_refused() {
    let one = synthetic_field(SyntheticWorld::OneCrater);
    let bytes = encode_within(&one, &whole(&one), None, SYNTHESIS_MARGIN_CELLS, 64 << 10);
    let at = starts(&bytes);
    let ends: Vec<usize> = at.iter().skip(1).copied().chain([bytes.len()]).collect();
    let k = (1..at.len())
        .find(|&k| {
            decode_block(&bytes[at[k]..ends[k]])
                .unwrap()
                .craters()
                .len()
                == 1
        })
        .expect("a block after block 0 carries the crater");
    // Its one range moved to face 5, which the crater on faces 0 and 2 does not reach, the cells'
    // places in their climate cells kept.
    let mut moved = bytes[at[k]..ends[k]].to_vec();
    let (start, end) = (u32_at(&moved, 37), u32_at(&moved, 41));
    let face_five = 5 * 4_096 + start % 4;
    patch(&mut moved, 37, &face_five.to_le_bytes());
    patch(&mut moved, 41, &(face_five + end - start).to_le_bytes());
    assert_eq!(
        decode_block(&moved),
        Err(DecodeBlockError::CraterMissesBlock { crater: 0 })
    );
    let flat = synthetic_field(SyntheticWorld::Flat);
    let mut bytes = encode_within(&flat, &whole(&flat), None, SYNTHESIS_MARGIN_CELLS, 64 << 10);
    let at = starts(&bytes);
    patch(&mut bytes, at[1] + 24, &[7]);
    assert_eq!(
        decode_payload(&bytes),
        Err(DecodePayloadError::WrongLevel {
            block: 1,
            found: CoarseLevel::new(7).unwrap(),
            expected: CoarseLevel::new(6).unwrap()
        })
    );
    // Thirty-three craters each reaching every cell are more than 32 reaches a cell.
    let blocks = blocks_of(&encode_payload(&one, &whole(&one), None));
    let mut crowded = blocks[0].clone();
    let crater = crowded.craters[0].clone();
    let all = Cover::from_cells(0..one.header().level().cell_count());
    crowded.craters = (0..33_u32)
        .map(|k| CoarseCrater {
            diameter: crater.diameter + Metres::new(f64::from(k)),
            reach: all.clone(),
            ..crater.clone()
        })
        .collect();
    crowded.crater_centres = vec![crowded.crater_centres[0]; 33];
    let mut field = PartialField::new(one.header().clone());
    let empty = field.clone();
    assert_eq!(
        field.insert(&crowded),
        Err(InsertBlockError::TooManyReaches)
    );
    assert_eq!(field, empty);
}

/// A partial field refuses a block of another body or level, a block 0 of another header, a
/// record that breaks a rule under its header, and a cell, climate record or crater that differs
/// from the one it holds, each with its own error, and is left unchanged by each.
#[test]
fn a_wire_partial_field_refuses_blocks_that_are_not_its_fields() {
    let flat = synthetic_field(SyntheticWorld::Flat);
    let one = synthetic_field(SyntheticWorld::OneCrater);
    let flat_blocks = blocks_of(&encode_payload(&flat, &whole(&flat), None));
    let blocks = blocks_of(&encode_payload(&one, &whole(&one), None));
    let held = partial(&blocks, 0..blocks.len());
    let refuse = |block: &DecodedBlock| {
        let mut field = held.clone();
        let error = field.insert(block).unwrap_err();
        assert_eq!(field, held, "{error:?} changed the field");
        error
    };
    assert_eq!(
        refuse(&flat_blocks[0]),
        InsertBlockError::WrongBody {
            found: flat.header().body(),
            expected: one.header().body()
        }
    );
    let mut deeper = blocks[0].clone();
    deeper.level = CoarseLevel::new(7).unwrap();
    assert_eq!(
        refuse(&deeper),
        InsertBlockError::WrongLevel {
            found: CoarseLevel::new(7).unwrap(),
            expected: CoarseLevel::new(6).unwrap()
        }
    );
    let mut lit = blocks[0].clone();
    lit.header = Some(one.header().clone().with_albedo_scale(Some(0.9)).unwrap());
    assert_eq!(refuse(&lit), InsertBlockError::WrongHeader);
    let mut seasons = blocks[0].clone();
    seasons.climate[3].month_anomaly[1] = 2;
    let cell = seasons.climate_indices[3];
    assert_eq!(
        refuse(&seasons),
        InsertBlockError::Record(BuildFieldError::MonthOutsideYear { cell })
    );
    // A one-month year's eleven later winds are calm.
    let mut breezy = blocks[0].clone();
    breezy.climate[3].wind[1] = Wind {
        azimuth: 64,
        speed: 100,
    };
    let cell = breezy.climate_indices[3];
    assert_eq!(
        refuse(&breezy),
        InsertBlockError::Record(BuildFieldError::MonthOutsideYear { cell })
    );
    // The one-crater world's palette is empty, so no cell of it has ice.
    let mut icy = blocks[0].clone();
    icy.cells[4].ice = 9;
    let cell = icy.cover.cells().nth(4).unwrap();
    assert_eq!(
        refuse(&icy),
        InsertBlockError::Record(BuildFieldError::IceWithoutEntry { cell })
    );
    let mut moved = blocks[0].clone();
    moved.cells[10].plate = 7;
    let cell = moved.cover.cells().nth(10).unwrap();
    assert_eq!(refuse(&moved), InsertBlockError::CellConflict { cell });
    let mut warmer = blocks[0].clone();
    warmer.climate[2].sea_level_temperature += 1;
    let cell = warmer.climate_indices[2];
    assert_eq!(refuse(&warmer), InsertBlockError::ClimateConflict { cell });
    let mut worn = blocks[0].clone();
    worn.craters[0].degradation += 1;
    let centre_cell = worn.crater_centres[0];
    assert_eq!(
        refuse(&worn),
        InsertBlockError::CraterConflict { centre_cell }
    );
    let mut small = blocks[0].clone();
    small.craters[0].diameter = one.header().boundary_diameter() * 0.5;
    assert_eq!(
        refuse(&small),
        InsertBlockError::Record(BuildFieldError::CraterDiameter { crater: 0 })
    );
    // The same block twice changes nothing.
    let mut again = held.clone();
    again.insert(&blocks[0]).unwrap();
    assert_eq!(again, held);
    let flat_field = partial(&flat_blocks, 0..flat_blocks.len());
    assert!(
        format!("{flat_field:?}").contains("held_cells: 24576"),
        "{flat_field:?}"
    );
}

/// Payloads damaged at a thousand pinned places, or cut short at hundreds of lengths, decode to
/// an error or to blocks, and the blocks insert or are refused, without a panic.
#[test]
fn damaged_wire_bytes_never_panic_the_decoder() {
    let (ceres, bytes, at) = ceres_blocks();
    let header = ceres.header().clone();
    let try_all = |b: &[u8]| {
        if let Ok(blocks) = decode_payload(b) {
            let mut field = PartialField::new(header.clone());
            for block in &blocks {
                let _ = field.insert(block);
            }
        }
        for window in at.windows(2) {
            let _ = decode_block(&b[window[0].min(b.len())..window[1].min(b.len())]);
        }
    };
    let mut g = Lcg::new(0x00de_c0de);
    let len = u64::try_from(bytes.len()).unwrap();
    for k in 0..1_000 {
        let mut damaged = bytes.clone();
        // Half the damage in the blocks' headers and covers, where a byte steers the parse.
        let place = if k % 2 == 0 {
            let start =
                at[usize::try_from(g.next_below(u64::try_from(at.len()).unwrap())).unwrap()];
            start + usize::try_from(g.next_below(220)).unwrap()
        } else {
            usize::try_from(g.next_below(len)).unwrap()
        };
        let place = place.min(bytes.len() - 1);
        damaged[place] = u8::try_from(g.next_below(256)).unwrap();
        try_all(&damaged);
    }
    for k in 0..300 {
        let cut = usize::try_from(g.next_below(len)).unwrap();
        try_all(&bytes[..cut]);
        if k < 60 {
            try_all(&bytes[..k]);
        }
    }
}

/// FNV-1a 64 over `bytes`, for the payloads' golden.
fn digest(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Writes a payload's line: its block count and lengths, its bytes and their digest, each block's
/// generator version (bytes 10 to 13) zeroed first, so that a bump alone moves only the file's
/// header: the decoders' tests hold the version in every block.
fn write_payload(w: &mut GoldenWriter, label: &str, bytes: &[u8]) {
    let at = starts(bytes);
    let mut unversioned = bytes.to_vec();
    for &start in &at {
        patch(&mut unversioned, start + 10, &[0; 4]);
    }
    let lengths: Vec<String> = at
        .iter()
        .zip(at.iter().skip(1).chain([&bytes.len()]))
        .map(|(a, b)| (b - a).to_string())
        .collect();
    w.line(&format!(
        "{label} blocks {} bytes {} lengths {}",
        at.len(),
        bytes.len(),
        lengths.join(",")
    ));
    w.u64_hex(&format!("{label} digest"), digest(&unversioned));
}

/// The golden file `tests/golden/wire/payloads.golden`: each synthetic world's whole payload, the
/// Mars-like world's survey, its delta and its margin, and the Moon-like world split at 48 KiB,
/// each as its blocks' lengths and an FNV-1a 64 digest of its bytes but the generator version's.
/// The format, the margin, the
/// delta rule and the split are generated output (Design note 17), so a change to any of them
/// fails here, and is a generator-version change.
#[test]
fn wire_payloads_match_their_golden() {
    let mut w = GoldenWriter::new();
    w.header(crate::generator_version());
    for &world in SyntheticWorld::ALL {
        let built;
        let field = match world {
            SyntheticWorld::EarthLike => earth(),
            SyntheticWorld::MarsLike => mars(),
            _ => {
                built = synthetic_field(world);
                &built
            }
        };
        write_payload(
            &mut w,
            &format!("{world:?} whole"),
            &encode_payload(field, &whole(field), None),
        );
    }
    let first = Cover::from_ranges([range(0, 1_024, 30)]).unwrap();
    write_payload(
        &mut w,
        "MarsLike region",
        &encode_payload(mars(), &region(), None),
    );
    write_payload(
        &mut w,
        "MarsLike region since its first range",
        &encode_payload(mars(), &region(), Some(&first)),
    );
    let margin = region().with_margin(mars().header().level(), SYNTHESIS_MARGIN_CELLS);
    for r in margin.ranges() {
        w.line(&format!(
            "MarsLike region held {}..{} code {}",
            r.start(),
            r.end(),
            r.code().get()
        ));
    }
    let moon = synthetic_field(SyntheticWorld::MoonLike);
    write_payload(
        &mut w,
        "MoonLike whole in 48 KiB",
        &encode_within(&moon, &whole(&moon), None, SYNTHESIS_MARGIN_CELLS, 48 << 10),
    );
    golden!("wire/payloads", w.as_str());
}

/// Writes a variant's line for every code of a field enum, in code order.
fn write_codes<T: Copy + std::fmt::Debug>(w: &mut GoldenWriter, kind: &str, all: &[T])
where
    u8: From<T>,
{
    for &value in all {
        w.line(&format!("{kind} {} = {value:?}", u8::from(value)));
    }
}

/// The golden file `tests/golden/wire/codes.golden`: every code a payload carries, decoded. Each
/// one-byte enum's codes and variants, and the layout's own tags; the value of every code of every
/// one-byte scale (the steepness index, the precipitation rate, the wind's speed and azimuth, the
/// resolution, the boundary obliquity, the ice share and the crater degradation); a sample of the
/// two-byte drainage area's; the header's temperature and anomaly steps for every exponent and its
/// temperatures at pinned codes; the cells' heights and distances at pinned codes; and the sea
/// floor's two nibbles, every code. A code renumbered fails it; a code appended only extends it
/// (`SURFACE_PAYLOAD_FORMAT`'s rule).
#[test]
fn wire_codes_match_their_golden() {
    let mut w = GoldenWriter::new();
    w.header(crate::generator_version());
    write_enum_codes(&mut w);
    write_byte_scales(&mut w);
    write_header_steps(&mut w);
    write_cell_readers(&mut w);
    write_seafloor_scales(&mut w);
    golden!("wire/codes", w.as_str());
}

/// The sea floor byte's two scales, each nibble's sixteen codes (`decision-r09-t5.md` item 5).
fn write_seafloor_scales(w: &mut GoldenWriter) {
    for code in 0..=0x0F_u8 {
        let at = SynthesisCell {
            crust: Crust::Oceanic,
            seafloor: SynthesisCell::pack_seafloor(code, code).unwrap(),
            ..PLAIN_CELL
        };
        match at.hill_relief() {
            Some(h) => w.f64(&format!("seafloor_hill_relief_m[{code}]"), h.value()),
            None => w.line(&format!("seafloor_hill_relief_m[{code}] = none")),
        }
        w.f64(
            &format!("seafloor_ponded_sediment_m[{code}]"),
            at.ponded_sediment().value(),
        );
    }
}

/// A dry, plain synthesis record, whose fields the readers' lines vary one at a time.
const PLAIN_CELL: SynthesisCell = SynthesisCell {
    elevation_mm: 0,
    boundary_distance_km: 0,
    plate: 0,
    crust: Crust::Lid,
    boundary: BoundaryKind::Divergent,
    boundary_obliquity: 0,
    flow: FlowDirection::Terminal,
    drainage: LogArea::ZERO,
    steepness: LogSteepness::ZERO,
    water_surface_mm: 0,
    ice: 0,
    substances: SynthesisCell::NO_SUBSTANCES,
    class: SurfaceClass::UNCLASSIFIED,
    crater_state: 0,
    seafloor: SynthesisCell::NO_SEAFLOOR,
};

/// Each one-byte enum's codes and variants, and the layout's own tags.
fn write_enum_codes(w: &mut GoldenWriter) {
    write_codes(w, "crust", Crust::ALL);
    write_codes(w, "boundary", BoundaryKind::ALL);
    write_codes(w, "flow", FlowDirection::ALL);
    write_codes(w, "morphology", Morphology::ALL);
    write_codes(w, "climate_model", ClimateModelKind::ALL);
    write_codes(w, "precipitation_source", PrecipitationSource::ALL);
    write_codes(w, "palette_role", PaletteRole::ALL);
    write_codes(w, "mechanics_family", MechanicsFamily::ALL);
    w.line(&format!("substance_nibble {NO_ENTRY} = None"));
    write_surface_class_ranges(w);
    w.line(&format!("screening {SCREENING_NONE} = None"));
    w.line(&format!("screening {SCREENING_ATMOSPHERE} = Atmosphere"));
    w.line(&format!("screening {SCREENING_CUTOFF} = Cutoff"));
    w.line(&format!("option {ABSENT} = None"));
    w.line(&format!("option {PRESENT} = Some"));
}

/// The surface class's code ranges, as runs of codes in one range, computed from every code.
fn write_surface_class_ranges(w: &mut GoldenWriter) {
    let mut start = 0_u8;
    for code in 1..=u8::MAX {
        let range = SurfaceClass::new(code).range();
        if range != SurfaceClass::new(start).range() {
            w.line(&format!(
                "surface_class {start}..={} = {:?}",
                code - 1,
                SurfaceClass::new(start).range()
            ));
            start = code;
        }
    }
    w.line(&format!(
        "surface_class {start}..=255 = {:?}",
        SurfaceClass::new(start).range()
    ));
}

/// The value of every code of every one-byte scale, and a sample of the drainage area's.
fn write_byte_scales(w: &mut GoldenWriter) {
    let degradation = |code: u8| CoarseCrater {
        centre: [1.0, 0.0, 0.0],
        diameter: Metres::new(1e5),
        morphology: Morphology::Simple,
        age: Gigayears::ZERO,
        degradation: code,
        reach: Cover::new(),
    };
    for code in 0..=u8::MAX {
        w.f64(
            &format!("steepness_m0_9[{code}]"),
            LogSteepness::new(code).index_m0_9(),
        );
    }
    for code in 0..=u8::MAX {
        w.f64(
            &format!("precipitation_kg_m2_s[{code}]"),
            LogPrecipitation::new(code).rate_kg_per_m2_s(),
        );
    }
    for code in 0..=u8::MAX {
        let wind = Wind {
            azimuth: code,
            speed: code,
        };
        w.f64(&format!("wind_speed_m_s[{code}]"), wind.speed().value());
        w.f64(
            &format!("wind_azimuth_rad[{code}]"),
            wind.azimuth_angle().value(),
        );
    }
    for code in 0..=u8::MAX {
        match ResolutionCode::new(code).resolution() {
            Some(r) => w.f64(&format!("resolution_m[{code}]"), r.value()),
            None => w.line(&format!("resolution_m[{code}] = none")),
        }
    }
    for code in 0..=u8::MAX {
        let at = SynthesisCell {
            boundary_obliquity: code,
            ice: code,
            ..PLAIN_CELL
        };
        w.f64(
            &format!("boundary_obliquity_rad[{code}]"),
            at.boundary_obliquity_angle().value(),
        );
        w.f64(&format!("ice_fraction[{code}]"), at.ice_fraction());
        w.f64(
            &format!("degradation_fraction[{code}]"),
            degradation(code).degradation_fraction(),
        );
    }
    let areas = [0_u16, 1, 2]
        .into_iter()
        .chain((1..=255).map(|k| 1 + 256 * k))
        .chain([u16::MAX]);
    for code in areas {
        w.f64(
            &format!("drainage_m2[{code}]"),
            LogArea::new(code).area().value(),
        );
    }
}

/// The header's temperature and anomaly steps for every exponent, and its temperatures at pinned
/// codes under two of them.
fn write_header_steps(w: &mut GoldenWriter) {
    let mut parts = mars().header().parts().clone();
    parts.reference_temperature = Kelvin::new(250.0);
    for n in 0..=15 {
        parts.temperature_step = n;
        parts.anomaly_step = n;
        let header = FieldHeader::new(parts.clone()).unwrap();
        w.f64(
            &format!("temperature_step_k[{n}]"),
            header.temperature_step_k(),
        );
        w.f64(&format!("anomaly_step_k[{n}]"), header.anomaly_step_k());
        if n == 0 || n == 5 {
            let plain = mars().climate_layer()[0];
            for t in [i16::MIN, -12_345, -1, 0, 1, 12_345, i16::MAX] {
                let climate = ClimateCell {
                    sea_level_temperature: t,
                    month_anomaly: [-128, -1, 0, 1, 127, 0, 0, 0, 0, 0, 0, 0],
                    ..plain
                };
                w.f64(
                    &format!("sea_level_temperature_k[n={n}][{t}]"),
                    header.sea_level_temperature(&climate).value(),
                );
                for month in 0..5 {
                    w.f64(
                        &format!("month_temperature_k[n={n}][{t}][{month}]"),
                        header.month_temperature(&climate, month).unwrap().value(),
                    );
                }
            }
            for anomaly in i8::MIN..=i8::MAX {
                let mut month_anomaly = [0; 12];
                month_anomaly[0] = anomaly;
                let climate = ClimateCell {
                    sea_level_temperature: 0,
                    month_anomaly,
                    ..plain
                };
                w.f64(
                    &format!("month_temperature_k[n={n}][0][anomaly {anomaly}]"),
                    header.month_temperature(&climate, 0).unwrap().value(),
                );
            }
        }
    }
}

/// The cells' heights and boundary distances at pinned codes.
fn write_cell_readers(w: &mut GoldenWriter) {
    for mm in [i32::MIN, -4_279_001, -1, 0, 1, 8_848_860, i32::MAX] {
        let at = SynthesisCell {
            elevation_mm: mm,
            water_surface_mm: mm,
            ..PLAIN_CELL
        };
        w.f64(&format!("elevation_m[{mm}]"), at.elevation().value());
        w.f64(
            &format!("water_surface_m[{mm}]"),
            at.water_surface().value(),
        );
    }
    for km in [-32_767, -1, 0, 1, 32_767, SynthesisCell::NO_BOUNDARY_KM] {
        let at = SynthesisCell {
            boundary_distance_km: km,
            ..PLAIN_CELL
        };
        match at.boundary_distance() {
            Some(d) => w.f64(&format!("boundary_distance_m[{km}]"), d.value()),
            None => w.line(&format!("boundary_distance_m[{km}] = none")),
        }
    }
}

/// Tests that read files, which the browser target cannot.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    /// Every golden file under `tests/golden/wire/` carries `GENERATOR_VERSION`, not R05's
    /// `TEST_PLANET_VERSION`, which heads the files directly under `tests/golden/`.
    #[test]
    fn every_wire_golden_carries_the_generator_version() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("wire");
        let expected = format!("# generator_version = {}", crate::generator_version());
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("the wire golden directory is readable") {
            let path = entry.expect("a directory entry is readable").path();
            if path.extension().is_some_and(|e| e == "golden") {
                let text = std::fs::read_to_string(&path).expect("a golden file is readable");
                assert_eq!(
                    text.lines().next(),
                    Some(expected.as_str()),
                    "{}",
                    path.display()
                );
                seen += 1;
            }
        }
        assert_eq!(seen, 2, "the wire goldens in {}", dir.display());
    }
}
