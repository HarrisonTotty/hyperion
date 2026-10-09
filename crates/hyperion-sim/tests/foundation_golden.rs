//! Golden files of the determinism foundation.
//!
//! Every value pinned here is part of the generator version: the position draw, the ID layouts, the
//! domain-tag hashes, the stream keying, every sampler's output and word consumption, the decision
//! thresholds and the event keys. (The designations printed beside the IDs are not, but a change
//! to them shows up here too.) Each file's first line
//! is `# generator_version = <n>` from [`GENERATOR_VERSION`], so a version bump fails every test
//! here until `just bless` regenerates the files in the same commit, and
//! [`every_golden_file_carries_the_current_version`] holds every golden file of the crate to the
//! same header, whichever test writes it. The pinned `libm`'s function values, the samplers and
//! the decisions moved with `math` and `rng` to `hyperion-base`, whose own `foundation_golden.rs`
//! pins them (plan R04, T4.a and T4.d).

use std::path::{Path, PathBuf};

use hyperion_sim::coords::{CellSize, GenCell};
use hyperion_sim::id::{
    BodyId, CatalogueSystemId, CentreMemberId, Designation, DwarfCoreMemberId, EventBin, EventId,
    EventSubject, EventWord, FeatureCell, FeatureMemberId, FeatureRef, Layer, MemberSlot, PinnedId,
    StreamMemberId, SystemId, event_tags,
};
use hyperion_sim::rng::{EventKey, ObjectKey, Stream, TagScope, tags};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// A writer that has already written the header for this build's generator version.
fn writer() -> GoldenWriter {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w
}

/// The golden files this suite writes, by name.
const FOUNDATION_GOLDENS: [&str; 5] = [
    "coords/positions",
    "id/layouts",
    "rng/tags",
    "rng/streams",
    "rng/events",
];

/// The crate's golden directory.
fn golden_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

/// Every `.golden` file under `dir`, by its name relative to `root` without the extension.
fn golden_names(root: &Path, dir: &Path, names: &mut Vec<String>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot list golden directory {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("a directory entry is readable").path();
        if path.is_dir() {
            golden_names(root, &path, names);
        } else if path.extension().is_some_and(|e| e == "golden") {
            let relative = path
                .strip_prefix(root)
                .expect("the file lies under the root");
            let name: Vec<String> = relative
                .with_extension("")
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            names.push(name.join("/"));
        }
    }
}

/// One header check over the whole crate: every golden file on disk, the foundation's and any
/// later plan's, begins with this build's `# generator_version`, so that a stale file left behind
/// by a renamed test is caught as surely as one a test still reads; and every file this suite
/// writes is there.
#[test]
fn every_golden_file_carries_the_current_version() {
    let root = golden_root();
    let mut names = Vec::new();
    golden_names(&root, &root, &mut names);
    names.sort();
    for name in FOUNDATION_GOLDENS {
        assert!(
            names.iter().any(|n| n == name),
            "golden file {name} is missing; if this change is intended, bump GENERATOR_VERSION \
             and run `just bless`"
        );
    }
    let header = format!("# generator_version = {}", GENERATOR_VERSION.get());
    for name in &names {
        let path = root.join(format!("{name}.golden"));
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read golden file {}: {e}", path.display()));
        assert_eq!(
            text.lines().next(),
            Some(header.as_str()),
            "golden file {name} does not carry the current generator version"
        );
    }
}

/// Fixed position words: all zeros, all ones, single bits, alternating patterns and three odd
/// constants (the golden ratio and the `SplitMix64` multipliers).
const POSITION_WORDS: [[u64; 3]; 6] = [
    [0, 0, 0],
    [u64::MAX, u64::MAX, u64::MAX],
    [
        0x8000_0000_0000_0000,
        0x0123_4567_89ab_cdef,
        0xfedc_ba98_7654_3210,
    ],
    [
        0x5555_5555_5555_5555,
        0xaaaa_aaaa_aaaa_aaaa,
        0x0000_0000_0000_0fff,
    ],
    [
        0x9e37_79b9_7f4a_7c15,
        0xbf58_476d_1ce4_e5b9,
        0x94d0_49bb_1331_11eb,
    ],
    [
        0x0000_0000_0000_1000,
        0xffff_ffff_ffff_f000,
        0x7fff_ffff_ffff_ffff,
    ],
];

/// Twelve position draws, two at each cell size: one in a cell next to the origin, one in a cell
/// at the root cube's faces.
#[test]
fn position_draws_are_pinned() {
    let mut w = writer();
    for (n, size) in CellSize::ALL.into_iter().enumerate() {
        let per_side = 65_536 / i32::try_from(size.ly()).unwrap();
        let cells = [[-1, 0, 3], [-per_side, 17, per_side - 1]];
        for (i, coordinates) in cells.into_iter().enumerate() {
            let cell = GenCell::new(size, coordinates).unwrap();
            assert!(cell.in_root_cube());
            let words = POSITION_WORDS[(n + 3 * i) % POSITION_WORDS.len()];
            let position = cell.position_from_words(words);
            let [x, y, z] = position.cell().to_array();
            w.line("");
            w.line(&format!(
                "{size:?} cell {coordinates:?} words [{:#018x}, {:#018x}, {:#018x}]",
                words[0], words[1], words[2]
            ));
            w.line(&format!("ly_cell = [{x}, {y}, {z}]"));
            for (axis, offset) in ["x", "y", "z"].into_iter().zip(position.offset_metres()) {
                w.f64(&format!("offset.{axis}"), offset);
            }
        }
    }
    golden!("coords/positions", w.as_str());
}

/// Writes `parts -> raw -> designation` after checking that the ID and its designation both
/// decode back to it.
fn id_line(w: &mut GoldenWriter, parts: &str, id: SystemId) {
    let designation = id.designation();
    assert_eq!(SystemId::from_raw(id.raw()), Ok(id));
    assert_eq!(
        designation.to_string().parse::<Designation>(),
        Ok(designation)
    );
    w.line(&format!("{parts} -> {:#018x} -> {designation}", id.raw()));
}

/// Four grid IDs per layer: the low corner, the origin cell, a cell near the origin and the high
/// corner, with the first, a small and the last index.
fn grid_lines(w: &mut GoldenWriter) {
    for layer in Layer::ALL {
        w.line("");
        let half = 65_536 / i32::try_from(layer.cell_size_ly()).unwrap();
        let last = (1_u32 << layer.index_bits()) - 1;
        for (cell, index) in [
            ([-half, -half, -half], 0),
            ([0, 0, 0], 0),
            ([-1, 3, -7], 42),
            ([half - 1, half - 1, half - 1], last),
        ] {
            let gen_cell = GenCell::new(layer.cell_size(), cell).unwrap();
            let id = SystemId::from_parts(layer, gen_cell, index).unwrap();
            id_line(
                w,
                &format!("grid {layer:?} cell {cell:?} index {index}"),
                id,
            );
        }
    }
}

/// Members of a catalogue feature, a dwarf core and the galactic centre, in cells and at feature
/// level.
fn member_lines(w: &mut GoldenWriter) {
    let slots = [
        MemberSlot::FeatureLevel { index: 0 },
        MemberSlot::FeatureLevel { index: 8_191 },
        MemberSlot::InCell {
            band: Layer::A,
            level: 0,
            cell: [7, 8, 7],
            index: 0,
        },
        MemberSlot::InCell {
            band: Layer::C,
            level: 3,
            cell: [8, 15, 2],
            index: 40,
        },
        MemberSlot::InCell {
            band: Layer::RoguePlanet,
            level: 7,
            cell: [15, 0, 15],
            index: 8_191,
        },
    ];
    w.line("");
    for (cell, index) in [([0, 0, 0], 5), ([-16, 15, -1], 16_383)] {
        let feature = FeatureRef::new(FeatureCell::new(cell).unwrap(), index).unwrap();
        for slot in slots {
            let id = FeatureMemberId::new(feature, slot).unwrap();
            id_line(w, &format!("feature {cell:?} #{index} {slot:?}"), id.into());
        }
    }
    w.line("");
    for number in [0, 3] {
        for slot in [slots[0], slots[3], slots[4]] {
            let id = DwarfCoreMemberId::new(number, slot).unwrap();
            id_line(w, &format!("dwarf core {number} {slot:?}"), id.into());
        }
    }
    w.line("");
    for slot in [
        MemberSlot::FeatureLevel { index: 0 },
        MemberSlot::FeatureLevel { index: 1 },
        MemberSlot::InCell {
            band: Layer::A,
            level: 9,
            cell: [2, 17, 18],
            index: 12,
        },
        MemberSlot::InCell {
            band: Layer::E,
            level: 11,
            cell: [31, 0, 31],
            index: 8_191,
        },
    ] {
        let id = CentreMemberId::new(slot).unwrap();
        id_line(w, &format!("centre {slot:?}"), id.into());
    }
}

/// Stream members, pinned content and catalogue systems.
fn list_and_catalogue_lines(w: &mut GoldenWriter) {
    w.line("");
    for (number, band, along, across, index) in [
        (0, Layer::A, 0, [0, 0], 0),
        (87, Layer::A, 5_121, [3, 23], 9),
        (4_095, Layer::RoguePlanet, 16_383, [63, 63], 8_191),
    ] {
        let id = StreamMemberId::new(number, band, along, across, index).unwrap();
        id_line(
            w,
            &format!("stream {number} {band:?} along {along} across {across:?} index {index}"),
            id.into(),
        );
    }
    w.line("");
    for number in [0, 42, PinnedId::NUMBER_LIMIT - 1] {
        id_line(
            w,
            &format!("pinned {number}"),
            PinnedId::new(number).unwrap().into(),
        );
    }
    w.line("");
    for (class, cell, index, member) in [
        (0, [-128, -128, -128], 0, 0),
        (3, [8, -67, 26], 118, 1),
        (9, [0, 0, 0], 1, 0),
        (63, [127, 127, 127], (1 << 24) - 1, 15),
    ] {
        let id = CatalogueSystemId::new(class, cell, index, member).unwrap();
        id_line(
            w,
            &format!("catalogue class {class} cell {cell:?} index {index} member {member}"),
            id.into(),
        );
    }
}

/// Body IDs and event IDs in their text forms.
fn body_and_event_lines(w: &mut GoldenWriter) {
    w.line("");
    let black_hole = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE);
    let star = SystemId::from_parts(
        Layer::A,
        GenCell::new(CellSize::Ly8, [652, -4_584, 2_047]).unwrap(),
        7,
    )
    .unwrap();
    for body in [
        BodyId::new(star, 0),
        BodyId::new(star, 0x0100),
        BodyId::new(black_hole, u16::MAX),
    ] {
        assert_eq!(body.to_string().parse::<BodyId>(), Ok(body));
        w.line(&format!("body {body} -> {}", body.designation()));
    }
    for (subject, k, j) in [
        (black_hole.into(), 0, 0),
        (BodyId::new(star, 0).into(), -1, 255),
        (star.into(), EventBin::MIN.get(), 0),
        (star.into(), EventBin::MAX.get(), 1),
    ] {
        let word = EventWord::new(event_tags::SELF_TEST, EventBin::new(k).unwrap(), j);
        let event = EventId::new(subject, word);
        assert_eq!(event.to_string().parse::<EventId>(), Ok(event));
        w.line(&format!("event bin {k} number {j} -> {event}"));
    }
}

/// Some sixty IDs covering every layout and every layer: parts, raw value, designation.
#[test]
fn id_layouts_are_pinned() {
    let mut w = writer();
    grid_lines(&mut w);
    member_lines(&mut w);
    list_and_catalogue_lines(&mut w);
    body_and_event_lines(&mut w);
    golden!("id/layouts", w.as_str());
}

/// Every registered domain tag with its scope and hash, so that a rename or a removal shows in
/// review as a changed line and not only as an addition.
#[test]
fn domain_tags_are_pinned() {
    let mut w = writer();
    // The foundation's registry, the surface crate's, then the sim's (plan R04, Design note 4):
    // the single registry's order byte for byte, since `selftest.stream`, the foundation's one tag,
    // was its first, with the surface crate's tags (R05's, then R09's) after it.
    let registries = [
        hyperion_base::rng::tags::ALL,
        hyperion_surface::tags::ALL,
        tags::ALL,
    ];
    for tag in registries.into_iter().flatten() {
        w.u64_hex(&format!("{} ({:?})", tag.name(), tag.scope()), tag.hash());
    }
    golden!("rng/tags", w.as_str());
}

/// A grid ID of layer A, for the stream keys.
fn layer_a(cell: [i32; 3], index: u32) -> SystemId {
    SystemId::from_parts(Layer::A, GenCell::new(CellSize::Ly8, cell).unwrap(), index).unwrap()
}

/// The first eight words of twenty streams: each kind of key, with the structured neighbours the
/// brainstorm names (adjacent cells, consecutive candidates, consecutive bodies) and extreme
/// seeds. Plan 01 registers one openable tag, `selftest.stream`, which accepts a key of any scope,
/// so the tag's hash is pinned by every line and the tag itself does not vary here.
#[test]
fn streams_are_pinned() {
    let star = layer_a([652, -4_584, 2_047], 7);
    let feature = FeatureRef::new(FeatureCell::new([-3, 4, 15]).unwrap(), 212).unwrap();
    let black_hole = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE);
    let keys: [(u64, ObjectKey); 20] = [
        (0, ObjectKey::galaxy()),
        (1, ObjectKey::galaxy()),
        (u64::MAX, ObjectKey::galaxy()),
        (0xdead_beef, ObjectKey::galaxy_item(1)),
        (0xdead_beef, ObjectKey::galaxy_item(2)),
        (0xdead_beef, ObjectKey::galaxy_item(u64::MAX)),
        (42, ObjectKey::cell(layer_a([0, 0, 0], 0).cell_word())),
        (42, ObjectKey::cell(layer_a([1, 0, 0], 0).cell_word())),
        (42, ObjectKey::cell(layer_a([0, 1, 0], 0).cell_word())),
        (42, ObjectKey::cell(layer_a([0, 0, -1], 0).cell_word())),
        (42, ObjectKey::feature(feature.object_word())),
        (42, star.into()),
        (42, layer_a([652, -4_584, 2_047], 8).into()),
        (42, layer_a([652, -4_584, 2_047], 9).into()),
        (42, black_hole.into()),
        (42, BodyId::new(star, 0).into()),
        (42, BodyId::new(star, 1).into()),
        (42, BodyId::new(star, 2).into()),
        (42, BodyId::new(star, u16::MAX).into()),
        (1 << 63, BodyId::new(black_hole, 0x0100).into()),
    ];
    let mut w = writer();
    for (seed, key) in keys {
        let seed = Seed::new(seed);
        let tag = tags::SELFTEST_STREAM;
        let mut stream = Stream::open(seed, tag, key);
        w.line("");
        w.line(&format!(
            "seed {seed} tag {} key {:?} word {:#018x} sub {}",
            tag.name(),
            key.scope(),
            key.word(),
            key.sub()
        ));
        for n in 0..8 {
            w.u64_hex(&format!("word[{n}]"), stream.next_u64());
        }
    }
    assert_eq!(keys[15].1.scope(), TagScope::Body);
    golden!("rng/streams", w.as_str());
}

/// A subject in its text form.
fn subject_text(subject: EventSubject) -> String {
    match subject {
        EventSubject::System(system) => system.to_string(),
        EventSubject::Body(body) => body.to_string(),
    }
}

/// Event keys in two steps: the keys of systems and bodies, then the first words of bin and event
/// streams across the whole range of k.
#[test]
fn event_keys_are_pinned() {
    let star = layer_a([652, -4_584, 2_047], 7);
    let black_hole = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE);
    let subjects: [EventSubject; 6] = [
        star.into(),
        BodyId::new(star, 0).into(),
        BodyId::new(star, 1).into(),
        BodyId::new(star, u16::MAX).into(),
        black_hole.into(),
        BodyId::new(black_hole, 0x0100).into(),
    ];
    let tag = event_tags::SELF_TEST;
    let mut w = writer();
    w.line(&format!("tag {:#06x} {}", tag.number(), tag.name()));
    for seed in [Seed::new(42), Seed::new(1 << 63)] {
        w.line("");
        for subject in subjects {
            let [k0, k1] = EventKey::derive(seed, tag, subject).words();
            w.line(&format!(
                "seed {seed} subject {} key = 0x{k0:016x} 0x{k1:016x}",
                subject_text(subject)
            ));
        }
    }

    let key = EventKey::derive(Seed::new(42), tag, star.into());
    let mut section = |label: String, mut stream: Stream| {
        w.line("");
        w.line(&label);
        for n in 0..4 {
            w.u64_hex(&format!("word[{n}]"), stream.next_u64());
        }
    };
    for k in [EventBin::MIN.get(), -1, 0, 1, EventBin::MAX.get()] {
        let bin = EventBin::new(k).unwrap();
        section(format!("{star} bin {k} slot 0"), key.bin_stream(bin));
    }
    for k in [-1, 0] {
        let bin = EventBin::new(k).unwrap();
        for j in [0, 1, 255] {
            let event = EventId::new(star.into(), EventWord::new(tag, bin, j));
            section(
                format!("{event} bin {k} event {j}"),
                key.event_stream(bin, j),
            );
        }
    }
    golden!("rng/events", w.as_str());
}
