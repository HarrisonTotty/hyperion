//! Golden files of the foundation that `hyperion-base` holds.
//!
//! Every value pinned here is part of the generator version: so far the pinned `libm`'s function
//! values, which moved here from `hyperion-sim` with `math` (plan R04, T4.a) byte for byte. Each
//! file's first line is `# generator_version = <n>` from [`GENERATOR_VERSION`], so a version bump
//! fails every test here until `just bless` regenerates the files in the same commit, and
//! [`every_golden_file_carries_the_current_version`] holds every golden file of the crate to the
//! same header, whichever test writes it.

use std::f64::consts::{FRAC_PI_4, LN_2, PI};
use std::path::{Path, PathBuf};

use hyperion_base::{GENERATOR_VERSION, math};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// A writer that has already written the header for this build's generator version.
fn writer() -> GoldenWriter {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w
}

/// The golden files this suite writes, by name.
const FOUNDATION_GOLDENS: [&str; 2] = ["math/functions", "math/bessel"];

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
/// `# generator_version`, so that a stale file left behind by a renamed test is caught as surely
/// as one a test still reads; and every file this suite writes is there.
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
