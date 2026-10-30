//! The pinned `libm`'s values for the client's TypeScript ports (rendering plan R08, R08.T20):
//! `golden/math/client_pairs.golden`, which `apps/hyperion/src/renderer/src/lib/pinnedMath.test.ts`
//! reads and holds `pinnedMath.ts` to bit for bit, so that the client's committed tables compute
//! their transcendentals as the sim does.
//!
//! For each of the fourteen functions the client ports (`exp`, `log`, `log2`, `log1p`, `expm1`,
//! `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `cbrt` and `pow`, here through
//! [`hyperion_base::math`], which is exactly `libm`), the file holds its value at:
//!
//! - the special values every function shares (signed zeros, the subnormal and normal extremes,
//!   ±1, ±2, ±½, the largest double, the infinities and a NaN), and for `atan2` and `pow` every pair
//!   of them;
//! - each threshold the function's source tests a high word against, with its neighbours;
//! - its own hard cases: the multiples of π/2 that `rem_pio2` cancels against, Ng's worst case for
//!   the reduction, exact cubes and CORE-MATH's two hard roots, `pow`'s integer, huge and
//!   overflowing powers;
//! - inputs whose exact result lies near a rounding midpoint, where they can be built: for `exp`,
//!   ln(1 + t) at t a half-unit of 1's last place, whose exp is within |x| 2⁻⁵³ of 1 + t; for `log`
//!   and `log1p`, e^y (1 + ½ ulp(y)) for |y| ≥ 16, whose logarithm is within 0.1 ulp of
//!   y + ½ ulp(y); for `pow`, odd integers to the powers 3 to 7 whose results have 54 significant
//!   bits, exact midpoints, scaled and signed;
//! - 64 random inputs from [`Lcg`];
//!
//! and, for each of its sweeps, a digest of its values at 2¹⁷ inputs drawn by a generator both
//! languages run ([`Mulberry32`], [`Domain`]), which the client recomputes.
//!
//! # Line format
//!
//! After the `# generator_version` header and a comment line:
//!
//! ```text
//! <name> <x> <result>
//! <name> <x> <y> <result>
//! sweep <name> <domain> [<domain>] <count> <seed> <digest>
//! ```
//!
//! Every value is its IEEE 754 bits as 16 hexadecimal digits, a NaN as `nan` (its bits are not
//! specified); `atan2`'s arguments are `y` then `x`, as it takes them. A domain is
//! `<sign><low>..<high>`, the sign `+`, `-` or `~` (either, drawn); the digest is 8 hexadecimal
//! digits.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::f64::consts::{FRAC_PI_2, LN_2};

use hyperion_base::{GENERATOR_VERSION, math};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;

/// A function of one argument, by the name the client exports it under.
type Unary = (&'static str, fn(f64) -> f64);

/// A function of two, likewise.
type Binary = (&'static str, fn(f64, f64) -> f64);

/// The client's one-argument ports, each the `hyperion_base::math` wrapper of the same `libm`
/// function.
const UNARY: [Unary; 12] = [
    ("exp", math::exp),
    ("log", math::ln),
    ("log2", math::log2),
    ("log1p", math::ln_1p),
    ("expm1", math::exp_m1),
    ("sin", math::sin),
    ("cos", math::cos),
    ("tan", math::tan),
    ("asin", math::asin),
    ("acos", math::acos),
    ("atan", math::atan),
    ("cbrt", math::cbrt),
];

/// The two-argument ports: `atan2(y, x)` and `pow(x, y)`.
const BINARY: [Binary; 2] = [("atan2", math::atan2), ("pow", math::powf)];

/// The values every function is pinned at, by their bits: ±0, the least and greatest subnormals,
/// the least normal, ±1 and its two neighbours, ±2, ±½, the greatest double, ±∞ and a NaN.
const SPECIAL: [u64; 24] = [
    0x0000_0000_0000_0000,
    0x8000_0000_0000_0000,
    0x0000_0000_0000_0001,
    0x8000_0000_0000_0001,
    0x000f_ffff_ffff_ffff,
    0x800f_ffff_ffff_ffff,
    0x0010_0000_0000_0000,
    0x8010_0000_0000_0000,
    0x3ff0_0000_0000_0000,
    0xbff0_0000_0000_0000,
    0x3ff0_0000_0000_0001,
    0x3fef_ffff_ffff_ffff,
    0xbff0_0000_0000_0001,
    0xbfef_ffff_ffff_ffff,
    0x4000_0000_0000_0000,
    0xc000_0000_0000_0000,
    0x3fe0_0000_0000_0000,
    0xbfe0_0000_0000_0000,
    0x7fef_ffff_ffff_ffff,
    0xffef_ffff_ffff_ffff,
    0x7ff0_0000_0000_0000,
    0xfff0_0000_0000_0000,
    0x7ff8_0000_0000_0000,
    0x4008_0000_0000_0000,
];

/// The high words each function's source compares |x|'s against (`libm` 0.2.16's
/// `src/math/<name>.rs`), by function.
fn thresholds(name: &str) -> &'static [u32] {
    match name {
        "exp" => &[
            0x4086_232b,
            0x3fd6_2e42,
            0x3ff0_a2b2,
            0x3e30_0000,
            0x4086_2e42,
        ],
        "log" | "log2" => &[
            0x0010_0000,
            0x3ff0_0000,
            0x3fe6_a09e,
            0x3ff6_a09e,
            0x7fef_ffff,
        ],
        "log1p" => &[
            0x3fda_827a,
            0x3ca0_0000,
            0x3fd2_bec4,
            0x3ff0_0000,
            0x4000_0000,
            0x4350_0000,
        ],
        "expm1" => &[
            0x4043_687a,
            0x3fd6_2e42,
            0x3ff0_a2b2,
            0x3c90_0000,
            0x0010_0000,
            0x4086_2e42,
        ],
        "sin" | "cos" | "tan" => &[
            0x3fe9_21fb,
            0x3e50_0000,
            0x3e46_a09e,
            0x3e40_0000,
            0x400f_6a7a,
            0x4002_d97c,
            0x401c_463b,
            0x4015_fdbc,
            0x4012_d97c,
            0x4019_21fb,
            0x4139_21fb,
            0x3fe5_9428,
        ],
        "asin" | "acos" => &[
            0x3ff0_0000,
            0x3fe0_0000,
            0x0010_0000,
            0x3e50_0000,
            0x3fef_3333,
            0x3c60_0000,
        ],
        "atan" => &[
            0x4410_0000,
            0x3fdc_0000,
            0x3e40_0000,
            0x3ff3_0000,
            0x3fe6_0000,
            0x4003_8000,
            0x0010_0000,
        ],
        "cbrt" => &[0x0010_0000, 0x0000_0001, 0x3ff0_0000, 0x4000_0000],
        _ => &[],
    }
}

/// Each threshold `h` as eight inputs: the doubles at the bottom and top of its high word and of
/// the words beside it, of both signs.
fn threshold_inputs(name: &str) -> Vec<u64> {
    let mut inputs = Vec::new();
    for &h in thresholds(name) {
        let h = u64::from(h);
        for sign in [0, 1_u64 << 63] {
            inputs.push(sign | h << 32);
            inputs.push(sign | h << 32 | 0xffff_ffff);
            inputs.push(sign | (h - 1) << 32 | 0xffff_ffff);
            inputs.push(sign | (h + 1) << 32);
        }
    }
    inputs
}

/// `x` and the doubles one unit in the last place either side of it, by their bits, for a finite
/// nonzero `x`.
fn with_neighbours(x: f64) -> [u64; 3] {
    let b = bits(x);
    [b - 1, b, b + 1]
}

/// 2ⁿ, for n a normal double's exponent.
fn two_to(n: i32) -> f64 {
    let biased = u64::try_from(n + 1023).expect("a normal exponent");
    f64::from_bits(biased << 52)
}

/// Each function's own hard cases, by their bits.
fn hard_cases(name: &str) -> Vec<u64> {
    let mut inputs = Vec::new();
    match name {
        // The overflow and underflow thresholds `exp.rs` and `expm1.rs` write in decimal, and the
        // edges of `expm1`'s scalings at k = 20 and 56 (|x| ≈ (k ± ½) ln 2).
        "exp" | "expm1" => {
            for x in [
                709.782_712_893_384,
                -708.396_418_532_264_1,
                -745.133_219_101_941_1,
                19.5 * LN_2,
                20.5 * LN_2,
                55.5 * LN_2,
                56.5 * LN_2,
                -56.0 * LN_2,
            ] {
                inputs.extend(with_neighbours(x));
            }
        }
        // The multiples of π/2 the reduction cancels against, of both signs, and Ng's worst case
        // for it, 6381956970095103 × 2⁷⁹⁷ (K. C. Ng 1992, "Argument reduction for huge
        // arguments: good to the last bit", table 1), with 10²², 10³⁰⁰ and the greatest double.
        "sin" | "cos" | "tan" => {
            for k in [
                1.0,
                2.0,
                3.0,
                4.0,
                5.0,
                6.0,
                7.0,
                8.0,
                100.0,
                1000.0,
                524_288.0,
                1_048_576.0,
                2_097_152.0,
            ] {
                inputs.extend(with_neighbours(k * FRAC_PI_2));
                inputs.extend(with_neighbours(-k * FRAC_PI_2));
            }
            inputs.push((0x750_u64 << 52) | (6_381_956_970_095_103 - (1_u64 << 52)));
            inputs.extend(with_neighbours(1e22));
            inputs.extend(with_neighbours(1e300));
        }
        // ±½, √3 ÷ 2 and the ends' neighbours.
        "asin" | "acos" => {
            for x in [0.5, -0.5, 0.866_025_403_784_438_6, 0.975, -0.975] {
                inputs.extend(with_neighbours(x));
            }
        }
        // The reduction's centres 0.5, 1, 1.5 and the 2⁶⁶ edge of its last interval.
        "atan" => {
            for x in [0.5, 1.5, -1.5, 2.4375, 7.378_697_629_483_821e19] {
                inputs.extend(with_neighbours(x));
            }
        }
        // Exact cubes, of integers and of a 20-bit fraction, CORE-MATH's two hard roots
        // (`cbrt.rs`'s 0x1.9b78223aa307cp+1 and 0x1.a202bfc89ddffp+2) and its directed roundings'
        // hard cases, each also scaled by 2³ and 2⁻³⁰⁰.
        "cbrt" => {
            for k in 1..=40_u32 {
                let k = f64::from(k);
                inputs.push(bits(k * k * k));
                let fraction = k * two_to(-20);
                inputs.push(bits(-(fraction * fraction * fraction)));
            }
            for hard in [
                0x4009_b782_23aa_307c,
                0x401a_202b_fc89_ddff,
                0x3ff3_a9cc_d7f0_22db,
                0x3ff7_845d_2faa_c6fe,
                0x3ffd_1ef8_1cbb_be71,
                0x4000_a201_4f62_987c,
                0x400f_e18a_044a_5501,
                0x401a_6bb8_c803_147b,
                0x401a_c853_8a03_1cbd,
            ] {
                let x = f64::from_bits(hard);
                inputs.push(hard);
                inputs.push(bits(x * 8.0));
                inputs.push(bits(-x * two_to(-300)));
            }
        }
        _ => {}
    }
    inputs
}

/// Inputs whose exact result lies near a rounding midpoint, for the functions where one can be
/// built (the module's documentation gives the constructions), by their bits.
fn midpoint_inputs(name: &str, lcg: &mut Lcg) -> Vec<u64> {
    let mut inputs = Vec::new();
    match name {
        "exp" => {
            for _ in 0..64 {
                let k = f64::from(u32::try_from(lcg.next_below(1 << 31)).expect("below 2³¹"));
                // 1 + t halfway between 1 + k 2⁻⁵² and its successor; 1 − t between 1 − k 2⁻⁵³
                // and its predecessor.
                let above = (2.0 * k + 1.0) * two_to(-53);
                let below = -(2.0 * k + 1.0) * two_to(-54);
                inputs.push(bits(math::ln_1p(above)));
                inputs.push(bits(math::ln_1p(below)));
            }
        }
        "log" | "log1p" => {
            for _ in 0..64 {
                let e = 4 + i32::try_from(lcg.next_below(6)).expect("below 6");
                let mantissa = lcg.next_u64() & 0x000f_ffff_ffff_ffff;
                let biased = u64::try_from(e + 1023).expect("a normal exponent");
                let y = f64::from_bits(biased << 52 | mantissa);
                let half_ulp = two_to(e - 53);
                // e^y and e^−y within range, and for `log1p` past 2⁵³, where 1 + x is x.
                for y in [y, -y] {
                    if y > 708.0 || y < -744.0 || (name == "log1p" && y < 64.0) {
                        continue;
                    }
                    inputs.push(bits(math::exp(y) * (1.0 + half_ulp)));
                }
            }
        }
        "pow" => {
            // Odd bases whose n-th powers have 54 significant bits, by n: [⌈2^(53/n)⌉, 2^(54/n)).
            for (n, low, high) in [
                (3_u32, 208_065_u64, 262_143_u64),
                (4, 9743, 11585),
                (5, 1553, 1782),
                (6, 457, 511),
                (7, 191, 209),
            ] {
                for _ in 0..16 {
                    let base = (low + lcg.next_below((high - low) / 2 + 1) * 2) | 1;
                    let base = f64::from(u32::try_from(base).expect("an 18-bit base"));
                    let n = f64::from(n);
                    inputs.push(bits(base));
                    inputs.push(bits(n));
                    inputs.push(bits(-base * two_to(-30)));
                    inputs.push(bits(n));
                }
            }
        }
        _ => {}
    }
    inputs
}

/// `pow`'s and `atan2`'s own pairs, by their bits: `pow`'s huge and integer powers of numbers
/// near ±1 and of negative numbers, its overflow and underflow edges and a subnormal result;
/// `atan2`'s ratios at 2⁶⁴.
fn binary_hard_cases(name: &str) -> Vec<(u64, u64)> {
    let mut pairs = Vec::new();
    let mut push = |a: f64, b: f64| pairs.push((bits(a), bits(b)));
    match name {
        "pow" => {
            for x in [
                1.0 + f64::EPSILON,
                1.0 - f64::EPSILON / 2.0,
                1.0 + two_to(-21),
                -1.0,
            ] {
                for y in [
                    two_to(31) + 0.5,
                    two_to(40),
                    -two_to(40),
                    two_to(63),
                    two_to(65),
                    -two_to(65),
                ] {
                    push(x, y);
                }
            }
            for x in [-2.0, -0.5, -3.0, -1e-300] {
                for y in [3.0, 4.0, -3.0, two_to(52) + 1.0, two_to(53), 0.5] {
                    push(x, y);
                }
            }
            for y in [
                1_023.999_999_999,
                1024.0,
                -1022.5,
                -1074.0,
                -1075.0,
                -1075.5,
            ] {
                push(2.0, y);
            }
            push(10.0, 22.0);
            push(10.0, -5.0);
            push(3.0, 33.0);
        }
        "atan2" => {
            for x in [1.0, -1.0, 3.0, -3.0] {
                for ratio in [two_to(64), two_to(64) * 2.0, two_to(-64), two_to(-65)] {
                    push(x * ratio, x);
                    push(-x * ratio, x);
                }
            }
        }
        _ => {}
    }
    pairs
}

/// 64 random inputs: a third over the unit interval and its neighbourhood, a third over all finite
/// doubles by their bits, a third over moderate magnitudes.
fn random_inputs(lcg: &mut Lcg) -> Vec<u64> {
    let mut inputs = Vec::new();
    for i in 0..64 {
        let x = match i % 3 {
            0 => 4.0 * lcg.next_f64() - 2.0,
            1 => loop {
                let x = f64::from_bits(lcg.next_u64());
                if x.is_finite() {
                    break x;
                }
            },
            _ => (lcg.next_f64() - 0.5) * 200.0,
        };
        inputs.push(bits(x));
    }
    inputs
}

/// A value as the file writes it.
fn word(x: f64) -> String {
    if x.is_nan() {
        "nan".to_owned()
    } else {
        format!("{:016x}", bits(x))
    }
}

/// Mulberry32 (Tommy Ettinger's 32-bit generator), the sweeps' source of inputs, which the client
/// test runs with `Math.imul`: a Weyl sequence of step 0x6d2b79f5, mixed. It need only be the same
/// in both languages.
struct Mulberry32(u32);

impl Mulberry32 {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 0x3d));
        t ^ (t >> 14)
    }
}

/// Which signs a sweep's domain draws.
#[derive(Clone, Copy)]
enum Sign {
    Plus,
    Minus,
    Either,
}

/// A sweep's domain: doubles of binary exponent uniform over `low..=high` (−1023 standing for the
/// subnormals) and a uniform 52-bit significand, of the given sign.
#[derive(Clone, Copy)]
struct Domain {
    sign: Sign,
    low: i32,
    high: i32,
}

impl Domain {
    const fn new(sign: Sign, low: i32, high: i32) -> Self {
        Self { sign, low, high }
    }

    /// The next input, from three words: the exponent, the sign's bit and the significand's high
    /// 20 bits, its low 32.
    fn draw(self, g: &mut Mulberry32) -> f64 {
        let (w0, w1, w2) = (g.next(), g.next(), g.next());
        let span = u32::try_from(self.high - self.low + 1).expect("a nonempty range");
        let e = self.low + i32::try_from(w0 % span).expect("under the span");
        let biased = u64::try_from((e + 1023).max(0)).expect("not negative");
        let sign = match self.sign {
            Sign::Plus => 0,
            Sign::Minus => 1,
            Sign::Either => u64::from(w1 >> 31),
        };
        f64::from_bits(
            sign << 63 | biased << 52 | u64::from(w1 & 0x000f_ffff) << 32 | u64::from(w2),
        )
    }

    fn text(self) -> String {
        let sign = match self.sign {
            Sign::Plus => '+',
            Sign::Minus => '-',
            Sign::Either => '~',
        };
        format!("{sign}{}..{}", self.low, self.high)
    }
}

/// `MurmurHash3`'s 32-bit body (Austin Appleby) over each value's two words, high first, a NaN as
/// `0x7ff8_0000` and 0, and its finaliser over the count of words.
struct Digest {
    hash: u32,
    words: u32,
}

impl Digest {
    const fn new() -> Self {
        Self { hash: 0, words: 0 }
    }

    fn word(&mut self, word: u32) {
        let k = word
            .wrapping_mul(0xcc9e_2d51)
            .rotate_left(15)
            .wrapping_mul(0x1b87_3593);
        self.hash = (self.hash ^ k)
            .rotate_left(13)
            .wrapping_mul(5)
            .wrapping_add(0xe654_6b64);
        self.words += 1;
    }

    fn value(&mut self, x: f64) {
        let b = if x.is_nan() {
            0x7ff8_0000_0000_0000
        } else {
            bits(x)
        };
        self.word(u32::try_from(b >> 32).expect("the high word"));
        self.word(u32::try_from(b & 0xffff_ffff).expect("the low word"));
    }

    fn finish(self) -> u32 {
        let mut h = self.hash ^ self.words;
        h ^= h >> 16;
        h = h.wrapping_mul(0x85eb_ca6b);
        h ^= h >> 13;
        h = h.wrapping_mul(0xc2b2_ae35);
        h ^ (h >> 16)
    }
}

/// The inputs a sweep draws.
const SWEEP_COUNT: u32 = 1 << 17;

/// Each one-argument function's sweeps.
fn unary_sweeps(name: &str) -> Vec<Domain> {
    use Sign::{Either, Minus, Plus};
    match name {
        "exp" | "expm1" => vec![
            Domain::new(Either, -30, 9),
            Domain::new(Either, -1023, 1023),
        ],
        "log" | "log2" => vec![Domain::new(Plus, -1023, 1023), Domain::new(Plus, -1, 0)],
        "log1p" => vec![Domain::new(Minus, -60, -1), Domain::new(Plus, -60, 1023)],
        "sin" | "cos" | "tan" => vec![Domain::new(Either, -30, 20), Domain::new(Either, 20, 1023)],
        "asin" | "acos" => vec![Domain::new(Either, -60, -1)],
        "atan" => vec![Domain::new(Either, -60, 70)],
        "cbrt" => vec![Domain::new(Either, -1023, 1023)],
        _ => Vec::new(),
    }
}

/// Each two-argument function's sweeps, as the pairs of domains of its arguments in its order.
fn binary_sweeps(name: &str) -> Vec<(Domain, Domain)> {
    use Sign::{Either, Plus};
    match name {
        "atan2" => vec![
            (Domain::new(Either, -30, 30), Domain::new(Either, -30, 30)),
            (
                Domain::new(Either, -1023, 1023),
                Domain::new(Either, -1023, 1023),
            ),
        ],
        "pow" => vec![
            (Domain::new(Plus, -10, 10), Domain::new(Either, -10, 10)),
            (Domain::new(Plus, -1023, 1023), Domain::new(Either, -30, 30)),
            (Domain::new(Plus, -1, 0), Domain::new(Either, 20, 40)),
        ],
        _ => Vec::new(),
    }
}

/// R08.T20: `libm`'s values for the client's pinned ports, at their special values, thresholds,
/// hard cases, near-midpoint and random inputs, with the digests of their sweeps.
#[test]
fn the_pinned_functions_are_written_for_the_client() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line(
        "# libm's values for the client's lib/pinnedMath.ts (R08.T20): <name> <x> [<y>] <result>, \
         each by its bits, or a sweep's digest",
    );
    let mut lcg = Lcg::new(0x5208_7420);
    let mut seed = 0_u32;
    for (name, f) in UNARY {
        let mut inputs: Vec<u64> = SPECIAL.to_vec();
        inputs.extend(threshold_inputs(name));
        inputs.extend(hard_cases(name));
        inputs.extend(midpoint_inputs(name, &mut lcg));
        inputs.extend(random_inputs(&mut lcg));
        for x in inputs {
            let x = f64::from_bits(x);
            w.line(&format!("{name} {} {}", word(x), word(f(x))));
        }
        for domain in unary_sweeps(name) {
            seed += 1;
            let mut g = Mulberry32(seed);
            let mut digest = Digest::new();
            for _ in 0..SWEEP_COUNT {
                digest.value(f(domain.draw(&mut g)));
            }
            w.line(&format!(
                "sweep {name} {} {SWEEP_COUNT} {seed} {:08x}",
                domain.text(),
                digest.finish()
            ));
        }
    }
    for (name, f) in BINARY {
        let mut pairs: Vec<(u64, u64)> = SPECIAL
            .iter()
            .flat_map(|&a| SPECIAL.iter().map(move |&b| (a, b)))
            .collect();
        pairs.extend(binary_hard_cases(name));
        let midpoints = midpoint_inputs(name, &mut lcg);
        pairs.extend(midpoints.as_chunks::<2>().0.iter().map(|&[a, b]| (a, b)));
        let first = random_inputs(&mut lcg);
        let second = random_inputs(&mut lcg);
        pairs.extend(first.into_iter().zip(second));
        for (a, b) in pairs {
            let (a, b) = (f64::from_bits(a), f64::from_bits(b));
            w.line(&format!("{name} {} {} {}", word(a), word(b), word(f(a, b))));
        }
        for (first, second) in binary_sweeps(name) {
            seed += 1;
            let mut g = Mulberry32(seed);
            let mut digest = Digest::new();
            for _ in 0..SWEEP_COUNT {
                let a = first.draw(&mut g);
                let b = second.draw(&mut g);
                digest.value(f(a, b));
            }
            w.line(&format!(
                "sweep {name} {} {} {SWEEP_COUNT} {seed} {:08x}",
                first.text(),
                second.text(),
                digest.finish()
            ));
        }
    }
    golden!("math/client_pairs", w.as_str());
}
