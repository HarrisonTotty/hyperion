//! Golden files of the foundation that `hyperion-base` holds.
//!
//! Every value pinned here is part of the generator version: the pinned `libm`'s function values,
//! every sampler's output and word consumption, and the decision thresholds, which moved here from
//! `hyperion-sim` with `math` and `rng` (plan R04, T4.a and T4.d) byte for byte, and `j0`. Each
//! file's first line is `# generator_version = <n>` from [`GENERATOR_VERSION`], so a version bump
//! fails every test here until `just bless` regenerates the files in the same commit, and
//! `every_golden_file_carries_the_current_version` (native only, since it reads the directory)
//! holds every golden file of the crate to the same header, whichever test writes it.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::f64::consts::{FRAC_PI_4, LN_2, PI};
use std::num::NonZeroU64;

use hyperion_base::rng::{
    DetailSeed, DomainTag, ObjectKey, PiecewiseLinear, PiecewisePowerLaw, PowerLaw, Stream,
    SurfaceSeed, TagScope, Threshold, Thresholds, tags,
};
use hyperion_base::{GENERATOR_VERSION, Seed, math};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// A writer that has already written the header for this build's generator version.
fn writer() -> GoldenWriter {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w
}

/// What reads the golden directory, which only a native run can (plan R04, Design note 12).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::{Path, PathBuf};

    use hyperion_base::GENERATOR_VERSION;

    /// The golden files this suite writes, by name.
    const FOUNDATION_GOLDENS: [&str; 4] = [
        "math/functions",
        "math/bessel",
        "rng/samplers",
        "rng/decisions",
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

    /// One header check over the whole crate: every golden file on disk begins with this build's
    /// `# generator_version`, so that a stale file left behind by a renamed test is caught as
    /// surely as one a test still reads; and every file this suite writes is there.
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

/// Arguments of J₀: zero, points around its first zero near 2.404 8, negative ones (it is even)
/// and large ones, where `libm` switches to its asymptotic form.
const J0: &[f64] = &[
    0.0,
    -0.0,
    1e-10,
    0.5,
    1.0,
    2.0,
    2.404_825_557_695_773,
    2.5,
    -1.0,
    -2.404_825_557_695_773,
    -7.5,
    5.520_078_110_286_311,
    8.0,
    10.0,
    25.0,
    100.0,
    1e4,
    1e8,
    -1e8,
    1e300,
];

/// `math::j0`, in a golden of its own so that `math/functions` stays byte for byte what it was
/// (plan R04, T4.c).
#[test]
fn j0_values_are_pinned() {
    let mut w = writer();
    for &x in J0 {
        w.f64(&format!("j0({x:?})"), math::j0(x));
    }
    golden!("math/bessel", w.as_str());
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

/// The surface keys' packing and the streams the two surface seeds open (plan R09.T1.a): each cell
/// key's word, then the first four words of each seed's stream under a tag of its own scope, for
/// cell keys (two of them alike but for `i` and `j` swapped) and item keys. The tags are minted
/// here, since the surface crate's registry holds the real ones, which this crate cannot see; a
/// tag's hash is its name's alone, so the lines pin the packing and the seeds' keying, both of
/// which R09's "Generator version" reserves.
#[test]
fn surface_streams_are_pinned() {
    const COARSE_TAG: DomainTag =
        DomainTag::registered("selftest.surface_coarse", TagScope::SurfaceCoarse);
    const DETAIL_TAG: DomainTag =
        DomainTag::registered("selftest.surface_detail", TagScope::SurfaceDetail);
    let cells: [(u8, u8, u32, u32, u16); 6] = [
        (0, 0, 0, 0, 0),
        (1, 3, 5, 2, 0),
        (1, 3, 2, 5, 0),
        (2, 8, 131, 77, 5),
        (4, 19, 0x0004_1234, 0x0007_ffff, 1),
        (5, 28, 0x0abc_def1, 0x0123_4567, u16::MAX),
    ];
    let mut w = writer();
    let mut keys = Vec::new();
    for (face, level, i, j, instance) in cells {
        let key = ObjectKey::surface_cell(face, level, i, j, instance).unwrap();
        let label = format!("cell {face} {level} ({i}, {j}) instance {instance}");
        w.u64_hex(&format!("{label} word"), key.word());
        keys.push((label, key));
    }
    for n in [0, 1, 48, u64::MAX] {
        keys.push((format!("item {n}"), ObjectKey::surface_item(n)));
    }
    for seed in [0, 0x5eed_0000_0009_0001, u64::MAX] {
        for (label, key) in &keys {
            let mut coarse = SurfaceSeed::new(seed).stream(COARSE_TAG, *key);
            for n in 0..4 {
                let label = format!("surface seed {seed:016x} {label} word {n}");
                w.u64_hex(&label, coarse.next_u64());
            }
            let mut detail = DetailSeed::new(seed).stream(DETAIL_TAG, *key);
            for n in 0..4 {
                let label = format!("detail seed {seed:016x} {label} word {n}");
                w.u64_hex(&label, detail.next_u64());
            }
        }
    }
    golden!("rng/surface_streams", w.as_str());
}
