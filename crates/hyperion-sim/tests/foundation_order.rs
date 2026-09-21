//! Order independence and run-to-run determinism of the foundation's streams.
//!
//! The brainstorm's "generating A then B equals generating B then A equals generating B alone",
//! and the determinism test `rust-dev.md` asks of the sim: the same seed and inputs give identical
//! output across two runs. Plan 01 has no generator yet, so the unit under test is a fixed recipe
//! of draws per ID, run over 64 IDs of every layout under six seeds. Only `selftest.stream` can be
//! opened so far, so the six seeds stand in for six tags' worth of streams. Later plans run their
//! generators through the same helper.

use hyperion_sim::coords::GenCell;
use hyperion_sim::id::{
    CatalogueSystemId, CentreMemberId, DwarfCoreMemberId, EventBin, FeatureCell, FeatureMemberId,
    FeatureRef, Layer, MemberSlot, PinnedId, StreamMemberId, SystemId, SystemIdKind, event_tags,
};
use hyperion_sim::rng::{EventKey, PowerLaw, Stream, Thresholds, tags};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::bits;
use hyperion_testkit::order::assert_order_independent;

/// Six seeds: six tags' worth of streams.
const SEEDS: [u64; 6] = [0, 1, 42, 0x5eed_5eed_5eed_5eed, 1 << 63, u64::MAX];

/// A slot in a nested grid: a cell of `level`, or the feature level when `level` is `None`.
fn slot(level: Option<u8>, band: Layer, cell: [u8; 3], index: u16) -> MemberSlot {
    match level {
        None => MemberSlot::FeatureLevel { index },
        Some(level) => MemberSlot::InCell {
            band,
            level,
            cell,
            index,
        },
    }
}

/// Sixty-four IDs spanning every layout: every grid layer at a corner, the origin and a cell near
/// it, and members of catalogue features, the centre, dwarf cores and streams, pinned content and
/// catalogue systems.
fn ids() -> Vec<SystemId> {
    let mut ids = Vec::with_capacity(64);
    for layer in Layer::ALL {
        let half = 65_536 / i32::try_from(layer.cell_size_ly()).unwrap();
        let last = (1_u32 << layer.index_bits()) - 1;
        for (cell, index) in [
            ([-half, -half, -half], 0),
            ([0, 0, 0], 1),
            ([-1, 3, -7], 42),
            ([half - 1, half - 1, half - 1], last),
        ] {
            let cell = GenCell::new(layer.cell_size(), cell).unwrap();
            ids.push(SystemId::from_parts(layer, cell, index).unwrap());
        }
    }
    let feature = FeatureRef::new(FeatureCell::new([-3, 4, 15]).unwrap(), 212).unwrap();
    for member in [
        slot(None, Layer::A, [0; 3], 0),
        slot(None, Layer::A, [0; 3], 3),
        slot(Some(0), Layer::A, [7, 8, 7], 0),
        slot(Some(2), Layer::C, [15, 0, 3], 17),
        slot(Some(7), Layer::RoguePlanet, [0, 15, 15], 8_191),
        slot(Some(1), Layer::BrownDwarf, [3, 12, 4], 5),
    ] {
        ids.push(FeatureMemberId::new(feature, member).unwrap().into());
    }
    for member in [
        slot(None, Layer::A, [0; 3], 0),
        slot(None, Layer::A, [0; 3], 1),
        slot(Some(0), Layer::B, [16, 15, 16], 3),
        slot(Some(11), Layer::E, [31, 0, 31], 8_191),
        slot(Some(5), Layer::A, [2, 17, 30], 12),
    ] {
        ids.push(CentreMemberId::new(member).unwrap().into());
    }
    for (number, member) in [
        (0, slot(None, Layer::A, [0; 3], 0)),
        (1, slot(Some(0), Layer::D, [8, 8, 8], 9)),
        (2, slot(Some(4), Layer::A, [0, 3, 12], 100)),
        (3, slot(None, Layer::A, [0; 3], 8_191)),
    ] {
        ids.push(DwarfCoreMemberId::new(number, member).unwrap().into());
    }
    for (number, band, along, across, index) in [
        (0, Layer::A, 0, [0, 0], 0),
        (87, Layer::A, 5_121, [3, 23], 9),
        (1_000, Layer::C, 77, [63, 0], 1),
        (2, Layer::E, 1, [0, 63], 4_000),
        (4_095, Layer::RoguePlanet, 16_383, [63, 63], 8_191),
    ] {
        ids.push(
            StreamMemberId::new(number, band, along, across, index)
                .unwrap()
                .into(),
        );
    }
    for number in [0, 42, 1 << 40, PinnedId::NUMBER_LIMIT - 1] {
        ids.push(PinnedId::new(number).unwrap().into());
    }
    for (class, cell, index, member) in [
        (0, [-128, -128, -128], 0, 0),
        (3, [8, -67, 26], 118, 1),
        (9, [0, 0, 0], 1, 0),
        (17, [-1, 0, 1], 40_000, 2),
        (40, [100, -100, 5], 7, 0),
        (63, [127, 127, 127], (1 << 24) - 1, 15),
        (63, [127, 127, 127], (1 << 24) - 1, 0),
        (1, [5, 5, 5], 0, 3),
        (2, [-50, 60, -70], 123_456, 0),
        (5, [0, -1, 0], 2, 1),
        (6, [12, 34, 56], 99, 4),
        (7, [-9, -8, -7], 65_535, 0),
    ] {
        ids.push(
            CatalogueSystemId::new(class, cell, index, member)
                .unwrap()
                .into(),
        );
    }
    ids
}

/// The draws of one ID under one seed: four uniforms, a normal, Poisson variates at 1.2 and at 40,
/// a power law and a threshold pick on the ID's stream, then the count of an event bin under its
/// event key. Floats are compared as bits.
#[derive(Debug, PartialEq, Eq)]
struct Draws {
    uniforms: [u64; 4],
    normal: u64,
    poissons: [u64; 2],
    power_law: u64,
    pick: Option<usize>,
    words: u64,
    events: u64,
}

/// The fixed recipe.
fn recipe(seed: Seed, id: SystemId, law: &PowerLaw, classes: &Thresholds) -> Draws {
    let mut stream = Stream::open(seed, tags::SELFTEST_STREAM, id.into());
    let uniforms = [0, 1, 2, 3].map(|_| bits(stream.uniform()));
    let normal = bits(stream.normal(0.0, 1.0));
    let poissons = [stream.poisson(1.2), stream.poisson(40.0)];
    let power_law = bits(stream.power_law(law));
    let pick = stream.pick(classes);
    let bin = EventBin::new(-2).unwrap();
    let events = EventKey::derive(seed, event_tags::SELF_TEST, id.into())
        .bin_stream(bin)
        .poisson(3.5);
    Draws {
        uniforms,
        normal,
        poissons,
        power_law,
        pick,
        words: stream.position(),
        events,
    }
}

/// Every (seed, ID) pair.
fn keys() -> Vec<(Seed, SystemId)> {
    let ids = ids();
    SEEDS
        .into_iter()
        .flat_map(|seed| ids.iter().map(move |&id| (Seed::new(seed), id)))
        .collect()
}

#[test]
fn the_ids_span_every_layout_once() {
    let ids = ids();
    assert_eq!(ids.len(), 64);
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 64, "the IDs are distinct");
    let mut layers = [false; 7];
    let mut kinds = [false; 7];
    for id in ids {
        let kind = match id.kind() {
            SystemIdKind::Grid(grid) => {
                let layer = usize::from(grid.layer().value());
                layers[layer] = true;
                0
            }
            SystemIdKind::FeatureMember(_) => 1,
            SystemIdKind::Centre(_) => 2,
            SystemIdKind::Stream(_) => 3,
            SystemIdKind::DwarfCore(_) => 4,
            SystemIdKind::Pinned(_) => 5,
            SystemIdKind::Catalogue(_) => 6,
        };
        kinds[kind] = true;
    }
    assert_eq!(layers, [true; 7], "every grid layer");
    assert_eq!(kinds, [true; 7], "every kind of ID");
}

/// A then B equals B then A equals B alone, for 384 streams.
#[test]
fn draws_do_not_depend_on_the_order_of_asking() {
    let law = PowerLaw::new(2.3, 0.5, 150.0).unwrap();
    let classes = Thresholds::from_weights(&[0.5, 1.25, 0.25], 2.5);
    assert_order_independent(&keys(), |&(seed, id)| recipe(seed, id, &law, &classes));
}

/// The sim's determinism test: two complete runs are equal, and the runs are not trivially so.
#[test]
fn two_runs_give_identical_output() {
    let run = || {
        let law = PowerLaw::new(2.3, 0.5, 150.0).unwrap();
        let classes = Thresholds::from_weights(&[0.5, 1.25, 0.25], 2.5);
        keys()
            .into_iter()
            .map(|(seed, id)| recipe(seed, id, &law, &classes))
            .collect::<Vec<_>>()
    };
    let first = run();
    assert_eq!(first, run());
    let mut firsts: Vec<u64> = first.iter().map(|d| d.uniforms[0]).collect();
    firsts.sort_unstable();
    firsts.dedup();
    assert_eq!(firsts.len(), first.len(), "every stream starts differently");
    assert!(GENERATOR_VERSION.is_supported());
}
