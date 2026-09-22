//! Golden files of the determinism foundation.
//!
//! Every value pinned here is part of the generator version: the pinned `libm`'s function values,
//! the position draw, the ID layouts, the domain-tag hashes, the stream keying, every sampler's
//! output and word consumption, the decision thresholds and the event keys. (The designations
//! printed beside the IDs are not, but a change to them shows up here too.) Each file's first line
//! is `# generator_version = <n>` from [`GENERATOR_VERSION`], so a version bump fails every test
//! here until `just bless` regenerates the files in the same commit, and
//! [`every_golden_file_carries_the_current_version`] holds every golden file of the crate to the
//! same header, whichever test writes it.

use std::f64::consts::{FRAC_PI_4, LN_2, PI};
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

use hyperion_sim::coords::{CellSize, GenCell};
use hyperion_sim::id::{
    BodyId, CatalogueSystemId, CentreMemberId, Designation, DwarfCoreMemberId, EventBin, EventId,
    EventSubject, EventWord, FeatureCell, FeatureMemberId, FeatureRef, Layer, MemberSlot, PinnedId,
    StreamMemberId, SystemId, event_tags,
};
use hyperion_sim::math;
use hyperion_sim::rng::{
    EventKey, ObjectKey, PiecewiseLinear, PiecewisePowerLaw, PowerLaw, Stream, TagScope, Threshold,
    Thresholds, tags,
};
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
const FOUNDATION_GOLDENS: [&str; 8] = [
    "math/functions",
    "coords/positions",
    "id/layouts",
    "rng/tags",
    "rng/streams",
    "rng/samplers",
    "rng/decisions",
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

type Unary = (&'static str, fn(f64) -> f64, &'static [f64]);
type Binary = (&'static str, fn(f64, f64) -> f64, &'static [(f64, f64)]);

/// Just under and just over 0.5 ln 2 and 1.5 ln 2, where `exp` and `exp_m1` change their
/// argument reduction.
const HALF_LN_2_BELOW: f64 = 0.5 * LN_2 - 1e-8;
const HALF_LN_2_ABOVE: f64 = 0.5 * LN_2 + 1e-8;

/// 1 ± 2⁻⁵², the neighbours of 1 where a logarithm's result is smallest.
const ONE_PLUS_ULP: f64 = 1.0 + f64::EPSILON;
const ONE_MINUS_ULP: f64 = 1.0 - f64::EPSILON;

/// A subnormal argument.
const SUBNORMAL: f64 = 1e-310;

/// Arguments of the circular functions: tiny, inside π/4, the reduction's medium range up to
/// 2²⁰ π/2, and the large-argument reduction.
const CIRCULAR: &[f64] = &[
    0.0,
    1e-9,
    1.0,
    FRAC_PI_4,
    3.0 * FRAC_PI_4,
    PI,
    -2.5,
    100.0,
    1e6,
    1e22,
];

/// Arguments of the functions of `[−1, 1]`: tiny, below and above 0.5, the 0.975 branch of
/// `asin`, and the ends.
const UNIT_INTERVAL: &[f64] = &[0.0, 1e-10, 0.3, -0.49, 0.5, 0.7, 0.98, 0.999_999, 1.0, -1.0];

/// Acklam's branch point of the normal quantile, and its mirror: 1 − 0.97575 lies just below
/// 0.02425, so `NORMAL_QUANTILE_HIGH` takes the tail branch and the double below it the central one.
const NORMAL_QUANTILE_LOW: f64 = 0.024_25;
const NORMAL_QUANTILE_HIGH: f64 = 0.975_75;

/// Arguments of the normal quantile: ½; both of Acklam's branch points crossed, 0.02425 ± 2⁻⁵⁵
/// (eight ulps) and 0.97575 ± 2⁻⁵³ (one ulp); the switch from erfc to erf at 0.25; 10⁻³ and its
/// mirror; the tail down to 10⁻¹⁰, the smallest normal and the smallest subnormal, where Φ
/// underflows gradually; and the last double below 1.
const NORMAL_QUANTILE: &[f64] = &[
    0.5,
    NORMAL_QUANTILE_LOW - f64::EPSILON / 8.0,
    NORMAL_QUANTILE_LOW,
    NORMAL_QUANTILE_LOW + f64::EPSILON / 8.0,
    NORMAL_QUANTILE_HIGH - f64::EPSILON / 2.0,
    NORMAL_QUANTILE_HIGH,
    NORMAL_QUANTILE_HIGH + f64::EPSILON / 2.0,
    0.25,
    0.001,
    0.999,
    1e-10,
    f64::MIN_POSITIVE,
    5e-324,
    1.0 - f64::EPSILON / 2.0,
];

const UNARY: &[Unary] = &[
    ("sin", math::sin, CIRCULAR),
    ("cos", math::cos, CIRCULAR),
    ("tan", math::tan, CIRCULAR),
    ("asin", math::asin, UNIT_INTERVAL),
    ("acos", math::acos, UNIT_INTERVAL),
    (
        "atan",
        math::atan,
        &[0.0, 1e-10, 0.3, 0.5, 1.0, 1.5, 2.0, 10.0, 1e20, -3.0],
    ),
    (
        "sinh",
        math::sinh,
        &[0.0, 1e-10, 0.5, 1.0, 3.0, 20.0, 22.0, 700.0, 710.0, -2.0],
    ),
    (
        "cosh",
        math::cosh,
        &[0.0, 1e-10, 0.5, 1.0, 3.0, 20.0, 22.0, 700.0, 710.0, -2.0],
    ),
    (
        "tanh",
        math::tanh,
        &[0.0, 1e-10, 0.5, 1.0, 3.0, 20.0, 22.0, 700.0, 710.0, -2.0],
    ),
    (
        "asinh",
        math::asinh,
        &[0.0, 1e-10, 0.5, 1.5, 3.0, 1e5, 1e10, 1e300, -4.0, -1e-5],
    ),
    (
        "acosh",
        math::acosh,
        &[1.0, 1.000_000_1, 1.5, 2.0, 2.5, 10.0, 1e5, 1e10, 1e300],
    ),
    (
        "atanh",
        math::atanh,
        &[
            0.0, 1e-10, 1e-300, 0.25, -0.4, 0.5, 0.75, 0.9, 0.999_999, -0.99,
        ],
    ),
    (
        "exp",
        math::exp,
        &[
            0.0,
            1e-10,
            1.0,
            -1.0,
            HALF_LN_2_BELOW,
            HALF_LN_2_ABOVE,
            2.5,
            709.0,
            -708.5,
            -745.0,
        ],
    ),
    (
        "exp2",
        math::exp2,
        &[
            0.0, 1e-10, 0.5, 1.0, -1.5, 10.25, 1023.9, -1022.5, -1074.0, 1e-300,
        ],
    ),
    (
        "exp10",
        math::exp10,
        &[0.0, 1e-10, 0.5, 2.0, 15.0, 16.0, -3.0, -7.5, 308.0, -323.0],
    ),
    (
        "exp_m1",
        math::exp_m1,
        &[
            1e-20,
            1e-10,
            1e-5,
            0.3,
            HALF_LN_2_ABOVE,
            1.0,
            10.0,
            57.0,
            700.0,
            -40.0,
        ],
    ),
    (
        "ln",
        math::ln,
        &[
            1.0,
            2.0,
            10.0,
            0.5,
            SUBNORMAL,
            ONE_PLUS_ULP,
            ONE_MINUS_ULP,
            1.414,
            1.415,
            1e300,
        ],
    ),
    (
        "log2",
        math::log2,
        &[
            1.0,
            2.0,
            10.0,
            1000.0,
            0.5,
            SUBNORMAL,
            ONE_PLUS_ULP,
            ONE_MINUS_ULP,
            3.0,
            1e300,
        ],
    ),
    (
        "log10",
        math::log10,
        &[
            1.0,
            2.0,
            10.0,
            1000.0,
            0.5,
            SUBNORMAL,
            ONE_PLUS_ULP,
            ONE_MINUS_ULP,
            3.0,
            1e300,
        ],
    ),
    (
        "ln_1p",
        math::ln_1p,
        &[
            1e-20, 1e-10, 1e-5, 0.41, 0.42, -0.29, -0.3, 1.0, 1e18, -0.999_999,
        ],
    ),
    (
        "cbrt",
        math::cbrt,
        &[
            0.0, 1.0, 27.0, -8.0, 2.0, 0.001, SUBNORMAL, 1e300, -3.0, 1e-5,
        ],
    ),
    (
        "erf",
        math::erf,
        &[0.0, 1e-10, 0.5, 0.843_75, 1.0, -1.2, 2.0, 3.5, 5.0, 7.0],
    ),
    (
        "erfc",
        math::erfc,
        &[0.0, 1e-10, 0.5, 0.843_75, 1.0, -1.2, 2.0, 5.0, 10.0, 27.0],
    ),
    (
        "ln_gamma",
        math::ln_gamma,
        &[1.0, 2.0, 0.5, 10.5, 3e5, 1e-10, -0.5, -2.5, 2.5, 7.9],
    ),
    (
        "gamma",
        math::gamma,
        &[0.5, 1.0, 5.0, 10.5, 1e-10, 0.1, -0.5, -2.5, 171.0, 171.5],
    ),
    ("normal_quantile", math::normal_quantile, NORMAL_QUANTILE),
];

const BINARY: &[Binary] = &[
    (
        "atan2",
        math::atan2,
        &[
            (1.0, 1.0),
            (1.0, -1.0),
            (-1.0, -1.0),
            (-1.0, 1.0),
            (0.0, -1.0),
            (1.0, 0.0),
            (1e-300, 1e10),
            (3.0, 1.0),
            (-2.0, 1e-20),
            (0.5, 26_000.0),
        ],
    ),
    // The mass-function exponents: 1 − α for the Kroupa slopes α = 0.3, 1.3, 2.3 and Salpeter's
    // 2.35, then a negative base, a subnormal result and a large exponent.
    (
        "powf",
        math::powf,
        &[
            (2.0, 0.5),
            (10.0, -0.35),
            (0.08, -1.3),
            (150.0, -2.3),
            (0.01, 0.7),
            (0.5, -0.3),
            (100.0, -1.35),
            (-2.0, 3.0),
            (2.0, -1074.0),
            (1.000_001, 1e6),
        ],
    ),
    (
        "hypot",
        math::hypot,
        &[
            (3.0, 4.0),
            (5.0, 12.0),
            (1e300, 1e300),
            (SUBNORMAL, SUBNORMAL),
            (1.0, 1e-20),
            (-3.0, 0.0),
            (1.5, 2.5),
            (1e150, 1e-150),
            (26_000.0, 100.0),
        ],
    ),
];

/// `mul_add` where fusing matters: a product's rounding error recovered, 1 − ε² that the unfused
/// form loses, the light-year products of `coords`, a subnormal result, and exact cancellation.
const MUL_ADD: &[(f64, f64, f64)] = &[
    (0.1, 1e9, -100_000_000.0),
    (1.0 + f64::EPSILON, 1.0 - f64::EPSILON, -1.0),
    (-65_536.0, 9_460_730_472_580_800.0, 4.7e15),
    (1_234_567.0, 9_460_730_472_580_800.0, -3.3),
    (0.3, 3.0, -0.9),
    (f64::MIN_POSITIVE, 0.25, 0.0),
    (1e300, 1e10, -1e308),
    (2.0, 3.0, 4.0),
    (-1.5, 2.0, 3.0),
];

const POWI: &[(f64, i32)] = &[
    (1.1, 7),
    (2.0, 1023),
    (2.0, -1022),
    (-3.0, 5),
    (0.5, -3),
    (1.000_001, 1_000_000),
    (10.0, 22),
    (10.0, -5),
    (7.0, 0),
    (-1.5, -7),
];

#[test]
fn math_function_values_are_pinned() {
    let mut w = writer();
    for (name, f, arguments) in UNARY {
        w.line("");
        for &x in *arguments {
            w.f64(&format!("{name}({x:?})"), f(x));
        }
    }
    w.line("");
    for &x in CIRCULAR {
        let (sin, cos) = math::sin_cos(x);
        w.f64(&format!("sin_cos({x:?}).sin"), sin);
        w.f64(&format!("sin_cos({x:?}).cos"), cos);
    }
    for (name, f, arguments) in BINARY {
        w.line("");
        for &(x, y) in *arguments {
            w.f64(&format!("{name}({x:?}, {y:?})"), f(x, y));
        }
    }
    w.line("");
    for &(x, a, b) in MUL_ADD {
        w.f64(
            &format!("mul_add({x:?}, {a:?}, {b:?})"),
            math::mul_add(x, a, b),
        );
    }
    w.line("");
    for &(x, n) in POWI {
        w.f64(&format!("powi({x:?}, {n})"), math::powi(x, n));
    }
    golden!("math/functions", w.as_str());
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
    for tag in tags::ALL {
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

/// Sixteen values of each sampler from `selftest.stream`, and the words each consumed.
#[test]
fn samplers_are_pinned() {
    fn section(
        w: &mut GoldenWriter,
        item: u64,
        name: &str,
        mut draw: impl FnMut(&mut Stream) -> Drawn,
    ) {
        let mut stream = Stream::open(
            Seed::new(0x5a3f_1e00_601d_e000),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy_item(item),
        );
        w.line("");
        w.line(name);
        for n in 0..16 {
            match draw(&mut stream) {
                Drawn::Real(x) => w.f64(&format!("[{n}]"), x),
                Drawn::Pair(x, y) => {
                    w.f64(&format!("[{n}].0"), x);
                    w.f64(&format!("[{n}].1"), y);
                }
                Drawn::Count(k) => w.line(&format!("[{n}] = {k}")),
            }
        }
        w.line(&format!("words = {}", stream.position()));
    }

    let six = NonZeroU64::new(6).unwrap();
    let huge = NonZeroU64::new((1 << 63) + 1).unwrap();
    let massive = PowerLaw::new(2.3, 8.0, 150.0).unwrap();
    let flat_log = PowerLaw::new(1.0, 1.0, 1_000.0).unwrap();
    let rising = PowerLaw::new(-0.5, 1.0, 4.0).unwrap();
    let kroupa =
        PiecewisePowerLaw::continuous(&[0.01, 0.08, 0.5, 150.0], &[0.3, 1.3, 2.3]).unwrap();
    let band_c = kroupa.truncated(0.75, 2.5).unwrap();
    let triangle = PiecewiseLinear::new(&[0.0, 1.0, 3.0], &[0.0, 2.0, 0.0]).unwrap();

    let mut w = writer();
    let real = Drawn::Real;
    section(&mut w, 0, "uniform", |s| real(s.uniform()));
    section(&mut w, 1, "uniform_open_low", |s| {
        real(s.uniform_open_low())
    });
    section(&mut w, 2, "uniform_open", |s| real(s.uniform_open()));
    section(&mut w, 3, "uniform_in(-3, 5)", |s| {
        real(s.uniform_in(-3.0, 5.0))
    });
    section(&mut w, 4, "below(6)", |s| Drawn::Count(s.below(six)));
    section(&mut w, 5, "below(2^63 + 1)", |s| {
        Drawn::Count(s.below(huge))
    });
    section(&mut w, 6, "standard_normal", |s| real(s.standard_normal()));
    section(&mut w, 7, "standard_normal_pair", |s| {
        let (x, y) = s.standard_normal_pair();
        Drawn::Pair(x, y)
    });
    section(&mut w, 8, "normal(1.5, 0.25)", |s| {
        real(s.normal(1.5, 0.25))
    });
    section(&mut w, 9, "log_normal(0.3, 0.5)", |s| {
        real(s.log_normal(0.3, 0.5))
    });
    section(&mut w, 10, "log_normal_dex(8, 0.11)", |s| {
        real(s.log_normal_dex(8.0, 0.11))
    });
    for (item, mean) in (11..).zip([0.3, 1.2, 9.99, 10.0, 1_000.0, 3e5]) {
        section(&mut w, item, &format!("poisson({mean})"), |s| {
            Drawn::Count(s.poisson(mean))
        });
    }
    section(&mut w, 17, "power_law(2.3, 8, 150)", |s| {
        real(s.power_law(&massive))
    });
    section(&mut w, 18, "power_law(1, 1, 1000)", |s| {
        real(s.power_law(&flat_log))
    });
    section(&mut w, 19, "power_law(-0.5, 1, 4)", |s| {
        real(s.power_law(&rising))
    });
    section(&mut w, 20, "Kroupa on [0.01, 150]", |s| {
        real(kroupa.sample(s))
    });
    section(&mut w, 21, "Kroupa on [0.75, 2.5]", |s| {
        real(band_c.sample(s))
    });
    section(&mut w, 22, "triangle (0, 0) (1, 2) (3, 0)", |s| {
        real(triangle.sample(s))
    });
    golden!("rng/samplers", w.as_str());
}

/// One draw of a sampler, as the golden prints it.
enum Drawn {
    Real(f64),
    Pair(f64, f64),
    Count(u64),
}

/// The integer-threshold convention: thresholds of fixed probabilities and weights, and sixteen
/// marks from `selftest.stream` with the decisions and picks they give.
#[test]
fn decisions_are_pinned() {
    let mut w = writer();
    let third = Threshold::from_probability(1.0 / 3.0);
    for (label, threshold) in [
        ("0.1", Threshold::from_probability(0.1)),
        ("1/3", third),
        ("4e-5", Threshold::from_probability(4e-5)),
        (
            "1 - 2^-53",
            Threshold::from_probability(1.0 - f64::EPSILON / 2.0),
        ),
        ("1", Threshold::from_probability(1.0)),
        ("0", Threshold::from_probability(0.0)),
    ] {
        w.u64_hex(&format!("from_probability({label})"), threshold.get());
    }
    w.u64_hex("from_ratio(3, 7)", Threshold::from_ratio(3.0, 7.0).get());
    w.u64_hex(
        "from_ratio(0.8, 2.5)",
        Threshold::from_ratio(0.8, 2.5).get(),
    );

    // Four classes with shares 0.1, 0.25, 0.05 and 0.3 of a bound of 2.5; the rest is rejection.
    let weights = [0.25, 0.625, 0.125, 0.75];
    let bound = 2.5;
    let thresholds = Thresholds::from_weights(&weights, bound);
    w.line("");
    w.line(&format!("from_weights({weights:?}, {bound})"));
    for (i, threshold) in thresholds.as_slice().iter().enumerate() {
        w.u64_hex(&format!("[{i}]"), threshold.get());
    }

    let mut stream = Stream::open(
        Seed::new(0x5a3f_1e00_601d_e000),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy_item(23),
    );
    w.line("");
    w.line("marks of selftest.stream, item 23");
    for n in 0..16 {
        let mark = stream.mark();
        let pick = mark.pick(&thresholds);
        assert_eq!(pick, mark.pick_weighted(&weights, bound));
        w.line(&format!(
            "[{n}] mark = 0x{:014x} below(1/3) = {} pick = {pick:?}",
            mark.get(),
            mark.is_below(third)
        ));
    }
    w.line(&format!("words = {}", stream.position()));
    golden!("rng/decisions", w.as_str());
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
