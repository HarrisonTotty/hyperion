// Ported in part from the pinned `libm` 0.2.16's `src/math/log.rs`, `src/math/atan2.rs` and
// `src/math/atan.rs` (https://github.com/rust-lang/compiler-builtins, MIT licence; see NOTICE),
// whose notices for the FreeBSD sources they port are preserved here as those files carry them:
//
/* origin: FreeBSD /usr/src/lib/msun/src/e_log.c */
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunSoft, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */
/* origin: FreeBSD /usr/src/lib/msun/src/e_atan2.c */
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunSoft, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 *
 */
/* origin: FreeBSD /usr/src/lib/msun/src/s_atan.c */
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunPro, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */

//! Four `f64` lanes side by side, for loops that step several independent bodies at once, such as
//! the spray's tracers (plan 10): each lane's result is the scalar operation's, bit for bit.
//!
//! [`F64x4`] applies each operation to its four lanes in turn: `+ − × ÷`, `sqrt`, `abs`,
//! negation, the ordered comparisons and a select ([`Mask4`]). IEEE 754 makes each of them exact,
//! and the compiler never fuses or reorders floating-point operations, so a lane's result is the
//! scalar operation's on that lane whether the compiler emits the four one at a time or packs them
//! into the target's vector unit: SSE2 on x86-64, NEON on aarch64 and simd128 on WebAssembly where
//! it is enabled (no build of the sim enables it today), each at its baseline, two lanes an
//! instruction. The layout is there so that it
//! can: a value's lanes lie side by side, and each operation is the same on every lane. The same
//! source is the scalar fallback on a target without a vector unit, with the same bits.
//!
//! The intrinsics of `core::arch` are not used. On x86-64 and aarch64 Rust lets safe code call
//! one only from a function compiled with `#[target_feature]` naming its feature, even where the
//! target enables that feature everywhere, and calling such a function from one without is
//! `unsafe`, which the workspace forbids (`unsafe_code = "forbid"`).
//!
//! [`ln`] and [`atan2`] are [`super::ln`]'s and [`super::atan2`]'s lane by lane, bit for bit:
//! `libm` 0.2.16's `log`, and the path of its `atan2` and `atan` that a small positive angle
//! takes, transcribed line for line over the lanes, the same constants and the same operations in
//! the same order. Where any lane lies outside that path, every lane is `libm`'s own.

use core::ops::{Add, BitAnd, Div, Mul, Neg, Sub};

/// `log`'s and `atan`'s constants (`libm` 0.2.16's `src/math/log.rs`, lines 63–71, and
/// `src/math/atan.rs`, lines 48–60), each line as those files write it, literal and bit comment.
#[expect(
    clippy::excessive_precision,
    clippy::unreadable_literal,
    reason = "libm's constants copied literally from log.rs and atan.rs, so that each can be \
              checked against them character by character; the tests hold the results to libm's"
)]
mod libm_constants {
    pub(super) const LN2_HI: f64 = 6.93147180369123816490e-01; /* 3fe62e42 fee00000 */
    pub(super) const LN2_LO: f64 = 1.90821492927058770002e-10; /* 3dea39ef 35793c76 */
    pub(super) const LG1: f64 = 6.666666666666735130e-01; /* 3FE55555 55555593 */
    pub(super) const LG2: f64 = 3.999999999940941908e-01; /* 3FD99999 9997FA04 */
    pub(super) const LG3: f64 = 2.857142874366239149e-01; /* 3FD24924 94229359 */
    pub(super) const LG4: f64 = 2.222219843214978396e-01; /* 3FCC71C5 1D8E78AF */
    pub(super) const LG5: f64 = 1.818357216161805012e-01; /* 3FC74664 96CB03DE */
    pub(super) const LG6: f64 = 1.531383769920937332e-01; /* 3FC39A09 D078C69F */
    pub(super) const LG7: f64 = 1.479819860511658591e-01; /* 3FC2F112 DF3E5244 */

    pub(super) const AT: [f64; 11] = [
        3.33333333333329318027e-01,  /* 0x3FD55555, 0x5555550D */
        -1.99999999998764832476e-01, /* 0xBFC99999, 0x9998EBC4 */
        1.42857142725034663711e-01,  /* 0x3FC24924, 0x920083FF */
        -1.11111104054623557880e-01, /* 0xBFBC71C6, 0xFE231671 */
        9.09088713343650656196e-02,  /* 0x3FB745CD, 0xC54C206E */
        -7.69187620504482999495e-02, /* 0xBFB3B0F2, 0xAF749A6D */
        6.66107313738753120669e-02,  /* 0x3FB10D66, 0xA0D03D51 */
        -5.83357013379057348645e-02, /* 0xBFADDE2D, 0x52DEFD9A */
        4.97687799461593236017e-02,  /* 0x3FA97B4B, 0x24760DEB */
        -3.65315727442169155270e-02, /* 0xBFA2B444, 0x2C6A6C2F */
        1.62858201153657823623e-02,  /* 0x3F90AD3A, 0xE322DA11 */
    ];
}

use libm_constants::{AT, LG1, LG2, LG3, LG4, LG5, LG6, LG7, LN2_HI, LN2_LO};

/// Four `f64` lanes, each operated on alone (module documentation).
///
/// # Examples
///
/// Four bodies' distances from the axis, a lane each, the same bits as one at a time:
///
/// ```
/// use hyperion_base::math::lanes::F64x4;
///
/// let (x, y) = ([3.0, 0.1, -2.5, 1e8], [4.0, 0.2, 7.0, 3.0]);
/// let r = (F64x4::new(x) * F64x4::new(x) + F64x4::new(y) * F64x4::new(y)).sqrt();
/// for (k, r) in r.to_array().into_iter().enumerate() {
///     assert_eq!(r.total_cmp(&(x[k] * x[k] + y[k] * y[k]).sqrt()), core::cmp::Ordering::Equal);
/// }
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct F64x4([f64; 4]);

impl F64x4 {
    /// The lanes `lanes`.
    #[inline]
    #[must_use]
    pub const fn new(lanes: [f64; 4]) -> Self {
        Self(lanes)
    }

    /// Every lane `x`.
    #[inline]
    #[must_use]
    pub const fn splat(x: f64) -> Self {
        Self([x; 4])
    }

    /// The lanes.
    #[inline]
    #[must_use]
    pub const fn to_array(self) -> [f64; 4] {
        self.0
    }

    /// Each lane's square root.
    #[inline]
    #[must_use]
    pub fn sqrt(self) -> Self {
        self.map(f64::sqrt)
    }

    /// Each lane's magnitude.
    #[inline]
    #[must_use]
    pub fn abs(self) -> Self {
        self.map(f64::abs)
    }

    /// Where each lane is below `other`'s: false where either is NaN.
    #[inline]
    #[must_use]
    pub fn lt(self, other: Self) -> Mask4 {
        self.compare(other, |a, b| a < b)
    }

    /// Where each lane is at most `other`'s: false where either is NaN.
    #[inline]
    #[must_use]
    pub fn le(self, other: Self) -> Mask4 {
        self.compare(other, |a, b| a <= b)
    }

    /// Where each lane is above `other`'s: false where either is NaN.
    #[inline]
    #[must_use]
    pub fn gt(self, other: Self) -> Mask4 {
        self.compare(other, |a, b| a > b)
    }

    /// Where each lane is at least `other`'s: false where either is NaN.
    #[inline]
    #[must_use]
    pub fn ge(self, other: Self) -> Mask4 {
        self.compare(other, |a, b| a >= b)
    }

    /// Each lane raised to `low`'s where it is below it, `if x < low { low } else { x }`: the
    /// first half of `f64::clamp`, which keeps a NaN.
    #[inline]
    #[must_use]
    pub fn at_least(self, low: Self) -> Self {
        self.lt(low).select(low, self)
    }

    /// Each lane lowered to `high`'s where it is above it, `if x > high { high } else { x }`: the
    /// second half of `f64::clamp`, which keeps a NaN.
    #[inline]
    #[must_use]
    pub fn at_most(self, high: Self) -> Self {
        self.gt(high).select(high, self)
    }

    /// `f` of each lane.
    #[inline]
    #[must_use]
    fn map(self, f: impl Fn(f64) -> f64) -> Self {
        let lanes = self.0;
        Self([f(lanes[0]), f(lanes[1]), f(lanes[2]), f(lanes[3])])
    }

    /// `f` of each lane and `other`'s.
    #[inline]
    #[must_use]
    fn zip(self, other: Self, f: impl Fn(f64, f64) -> f64) -> Self {
        let (p, q) = (self.0, other.0);
        Self([f(p[0], q[0]), f(p[1], q[1]), f(p[2], q[2]), f(p[3], q[3])])
    }

    /// `f` of each lane and `other`'s, a truth each.
    #[inline]
    #[must_use]
    fn compare(self, other: Self, f: impl Fn(f64, f64) -> bool) -> Mask4 {
        let (p, q) = (self.0, other.0);
        Mask4([
            Mask4::lane(f(p[0], q[0])),
            Mask4::lane(f(p[1], q[1])),
            Mask4::lane(f(p[2], q[2])),
            Mask4::lane(f(p[3], q[3])),
        ])
    }
}

impl From<[f64; 4]> for F64x4 {
    #[inline]
    fn from(lanes: [f64; 4]) -> Self {
        Self(lanes)
    }
}

impl From<F64x4> for [f64; 4] {
    #[inline]
    fn from(lanes: F64x4) -> Self {
        lanes.0
    }
}

impl Add for F64x4 {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        self.zip(other, |a, b| a + b)
    }
}

impl Sub for F64x4 {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        self.zip(other, |a, b| a - b)
    }
}

impl Mul for F64x4 {
    type Output = Self;

    #[inline]
    fn mul(self, other: Self) -> Self {
        self.zip(other, |a, b| a * b)
    }
}

impl Div for F64x4 {
    type Output = Self;

    #[inline]
    fn div(self, other: Self) -> Self {
        self.zip(other, |a, b| a / b)
    }
}

/// Each lane's sign flipped, as `-x` flips a scalar's: a NaN's and a zero's included.
impl Neg for F64x4 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        self.map(|a| -a)
    }
}

/// Four truths, a lane each: a comparison of two [`F64x4`]s, or what selects between two.
///
/// Each lane is held as a vector unit's comparison leaves it, all ones for true and all zeros for
/// false, so that a select is a choice of bits and a compiler can keep both in the vector unit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Mask4([u64; 4]);

impl Mask4 {
    /// The lane `true` or `false`, as held.
    #[inline]
    #[must_use]
    const fn lane(truth: bool) -> u64 {
        if truth { u64::MAX } else { 0 }
    }

    /// The lanes' truths.
    #[inline]
    #[must_use]
    pub const fn to_array(self) -> [bool; 4] {
        let lanes = self.0;
        [lanes[0] != 0, lanes[1] != 0, lanes[2] != 0, lanes[3] != 0]
    }

    /// Whether every lane is true.
    #[inline]
    #[must_use]
    pub const fn all(self) -> bool {
        let lanes = self.0;
        lanes[0] & lanes[1] & lanes[2] & lanes[3] == u64::MAX
    }

    /// Whether any lane is true.
    #[inline]
    #[must_use]
    pub const fn any(self) -> bool {
        let lanes = self.0;
        lanes[0] | lanes[1] | lanes[2] | lanes[3] != 0
    }

    /// `if_true`'s lane where this lane is true, else `if_false`'s: its bits, NaN's included.
    #[expect(
        clippy::disallowed_methods,
        reason = "a select chooses bits: they are copied and never hashed"
    )]
    #[inline]
    #[must_use]
    pub fn select(self, if_true: F64x4, if_false: F64x4) -> F64x4 {
        let (lanes, then, otherwise) = (self.0, if_true.0, if_false.0);
        let lane = |k: usize| {
            f64::from_bits((lanes[k] & then[k].to_bits()) | (!lanes[k] & otherwise[k].to_bits()))
        };
        F64x4([lane(0), lane(1), lane(2), lane(3)])
    }
}

impl BitAnd for Mask4 {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        let (p, q) = (self.0, other.0);
        Self([p[0] & q[0], p[1] & q[1], p[2] & q[2], p[3] & q[3]])
    }
}

/// The natural logarithm of each lane, [`super::ln`]'s bit for bit.
///
/// Where every lane is positive, normal and finite, `log`'s main path runs over the lanes
/// (`log_normal`); otherwise, a lane being zero, negative, subnormal, infinite or NaN, each lane
/// is `libm`'s own.
#[expect(
    clippy::inline_always,
    reason = "measured: the spray's lanes take two a step, and out of line (`#[inline]` left it \
              there) the pal5_like tube's build took 3% longer (perf-spray-step, pod, 16 vCPU)"
)]
#[inline(always)]
#[must_use]
pub fn ln(x: F64x4) -> F64x4 {
    let normal = x.ge(F64x4::splat(f64::MIN_POSITIVE)) & x.lt(F64x4::splat(f64::INFINITY));
    if normal.all() {
        log_normal(x)
    } else {
        lane_by_lane(super::ln, x)
    }
}

/// `f` of each lane alone: out of line, so that the main paths it stands beside stay small
/// enough to inline into a caller's loop.
#[cold]
#[inline(never)]
#[must_use]
fn lane_by_lane(f: fn(f64) -> f64, x: F64x4) -> F64x4 {
    x.map(f)
}

/// `f` of each lane of `y` and of `x` alone, as [`lane_by_lane`].
#[cold]
#[inline(never)]
#[must_use]
fn lane_by_lane_2(f: fn(f64, f64) -> f64, y: F64x4, x: F64x4) -> F64x4 {
    y.zip(x, f)
}

/// `log`'s high-word offset, `0x3ff00000 − 0x3fe6a09e`, in place in a double's bits.
const LOG_OFFSET: u64 = (0x3ff0_0000 - 0x3fe6_a09e) << 32;

/// `log`'s high word of the reduced argument's base, `0x3fe6a09e`, in place in a double's bits.
const LOG_BASE: u64 = 0x3fe6_a09e << 32;

/// The bits of 2⁵², whose low mantissa bits take a small integer exactly.
const TWO_TO_52: u64 = 0x4330_0000_0000_0000;

/// 2⁵² + `0x3ff`, the double whose difference from 2⁵² + n is n − `0x3ff` exactly, for the
/// exponents n of `log`'s reduction.
const TWO_TO_52_AND_BIAS: f64 = 4_503_599_627_371_519.0;

/// `libm` 0.2.16's `log` (its `src/math/log.rs`, lines 75–118) for lanes that are positive,
/// normal and finite, which take none of its special cases.
///
/// For such an `x` the high word `hx` is in `[0x00100000, 0x7ff00000)` and the sign is clear, so
/// `log`'s first three tests are false, and its fourth, `x == 1`, returns the `+0.0` that the main
/// path gives too. The reduction's 32-bit word operations are done in place in the double's 64
/// bits:
///
/// - `hx += 0x3ff00000 − 0x3fe6a09e` adds to the high word alone: the low word gains nothing, and
///   the sum stays below 2³¹, so no bit carries out of either;
/// - `k = (hx >> 20) − 0x3ff`, as `k as f64`, is `(2⁵² + (hx >> 20)) − (2⁵² + 0x3ff)`: both are
///   integers below 2⁵³, exact as doubles, and so is their difference, `+0.0` where it is zero;
/// - `hx = (hx & 0x000fffff) + 0x3fe6a09e` and `ui = (hx << 32) | (ui & 0xffffffff)` keep the
///   low word and set the high one, which stays below 2³².
///
/// The rest is `log`'s arithmetic as it writes it, operation for operation.
#[expect(
    clippy::disallowed_methods,
    clippy::many_single_char_names,
    reason = "libm's own reading of a double's words, which log's reduction is made of: the bits \
              are computed with and never hashed; and libm's names, kept for the line-for-line \
              transcription"
)]
#[inline]
#[must_use]
fn log_normal(x: F64x4) -> F64x4 {
    let ui = x.0;
    let ui = [
        ui[0].to_bits(),
        ui[1].to_bits(),
        ui[2].to_bits(),
        ui[3].to_bits(),
    ];
    let hx = [
        ui[0] + LOG_OFFSET,
        ui[1] + LOG_OFFSET,
        ui[2] + LOG_OFFSET,
        ui[3] + LOG_OFFSET,
    ];
    let exponent = |u: u64| f64::from_bits(TWO_TO_52 | (u >> 52));
    let k = F64x4([
        exponent(hx[0]),
        exponent(hx[1]),
        exponent(hx[2]),
        exponent(hx[3]),
    ]) - F64x4::splat(TWO_TO_52_AND_BIAS);
    let reduced = |u: u64| f64::from_bits((u & 0x000f_ffff_ffff_ffff) + LOG_BASE);
    let x = F64x4([
        reduced(hx[0]),
        reduced(hx[1]),
        reduced(hx[2]),
        reduced(hx[3]),
    ]);

    let f = x - F64x4::splat(1.0);
    let hfsq = F64x4::splat(0.5) * f * f;
    let s = f / (F64x4::splat(2.0) + f);
    let z = s * s;
    let w = z * z;
    let (lg1, lg2, lg3, lg4) = (
        F64x4::splat(LG1),
        F64x4::splat(LG2),
        F64x4::splat(LG3),
        F64x4::splat(LG4),
    );
    let (lg5, lg6, lg7) = (F64x4::splat(LG5), F64x4::splat(LG6), F64x4::splat(LG7));
    let t1 = w * (lg2 + w * (lg4 + w * lg6));
    let t2 = z * (lg1 + w * (lg3 + w * (lg5 + w * lg7)));
    let r = t2 + t1;
    let dk = k;
    s * (hfsq + r) + dk * F64x4::splat(LN2_LO) - hfsq + f + dk * F64x4::splat(LN2_HI)
}

/// The smallest argument of `atan`'s first interval that it does not return as it is, 2⁻²⁷: its
/// test `ix < 0x3e400000` on the high word, as a bound on the value.
const ATAN_TINY: f64 = f64::from_bits(0x3e40_0000_0000_0000);

/// The end of `atan`'s first interval, 7/16: its test `ix < 0x3fdc0000` on the high word, as a
/// bound on the value.
const ATAN_FIRST: f64 = f64::from_bits(0x3fdc_0000_0000_0000);

/// The angle of the point `(x, y)` from the +x axis for each lane's `y` and `x`, radians,
/// [`super::atan2`]'s bit for bit.
///
/// Where every lane's `x` is positive and `y ÷ x` lies in `[2⁻²⁷, 7/16)`, so that `x` and `y` are
/// positive and finite too, the path `atan2` and `atan` take there runs over the lanes
/// (`atan_first`); otherwise each lane is `libm`'s own.
#[inline]
#[must_use]
pub fn atan2(y: F64x4, x: F64x4) -> F64x4 {
    let t = y / x;
    // With `x` positive, `t` in the interval makes `x` finite (`y ÷ ∞` is zero or NaN) and `y`
    // positive and finite (else `t` is NaN, at most zero, or infinite).
    let first =
        x.gt(F64x4::splat(0.0)) & t.ge(F64x4::splat(ATAN_TINY)) & t.lt(F64x4::splat(ATAN_FIRST));
    if first.all() {
        atan_first(t)
    } else {
        lane_by_lane_2(super::atan2, y, x)
    }
}

/// `libm` 0.2.16's `atan2(y, x)` (its `src/math/atan2.rs`, lines 51–115) where `x` is positive
/// and `t = y ÷ x` lies in `[2⁻²⁷, 7/16)`, `x` and `y` being positive and finite then: its
/// `atan(t)` (`src/math/atan.rs`, lines 67–131) on `atan`'s first interval.
///
/// There `atan2` takes none of its special cases: neither is NaN, zero or infinite; `x = 1`
/// returns `atan(y)`, which is `atan(t)` since `t = y ÷ 1 = y`; `|y ÷ x| > 2⁶⁴`, read from the
/// exponents, would need `t` of 2⁶³ or more; and `m = 0`, the signs being clear, returns
/// `atan(fabs(t))`, `t` itself. `atan` of such a `t` is below 2⁶⁶ and in `[2⁻²⁷, 7/16)`, its `id`
/// −1, and returns `x − x (s1 + s2)` (`x` being `t`), as written here operation for operation.
#[inline]
#[must_use]
fn atan_first(x: F64x4) -> F64x4 {
    let at = |k: usize| F64x4::splat(AT[k]);
    let at = [
        at(0),
        at(1),
        at(2),
        at(3),
        at(4),
        at(5),
        at(6),
        at(7),
        at(8),
        at(9),
        at(10),
    ];
    let z = x * x;
    let w = z * z;
    let s1 = z * (at[0] + w * (at[2] + w * (at[4] + w * (at[6] + w * (at[8] + w * at[10])))));
    let s2 = w * (at[1] + w * (at[3] + w * (at[5] + w * (at[7] + w * at[9]))));
    x - x * (s1 + s2)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;
    use hyperion_testkit::lcg::Lcg;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    /// A float's bits for a failure message, or "NaN".
    fn shown(value: f64) -> String {
        if value.is_nan() {
            format!("{value:?}")
        } else {
            format!("{value:e} (0x{:016x})", bits(value))
        }
    }

    /// `ours` against `scalar`, lane by lane: the same bits, or both NaN, whose bits Rust leaves
    /// unspecified (the testkit's `bits` refuses them).
    #[track_caller]
    fn assert_lanes(what: &str, input: &str, ours: F64x4, scalar: [f64; 4]) {
        for (k, (ours, scalar)) in ours.to_array().into_iter().zip(scalar).enumerate() {
            if ours.is_nan() || scalar.is_nan() {
                assert!(
                    ours.is_nan() && scalar.is_nan(),
                    "{what} of {input}, lane {k}: {} against {}",
                    shown(ours),
                    shown(scalar)
                );
            } else {
                assert_eq!(
                    bits(ours),
                    bits(scalar),
                    "{what} of {input}, lane {k}: {} against {}",
                    shown(ours),
                    shown(scalar)
                );
            }
        }
    }

    /// [`ln`] of `x` against [`super::ln`], lane by lane.
    #[track_caller]
    fn assert_ln(x: [f64; 4]) {
        assert_lanes(
            "ln",
            &format!("{:?}", x.map(shown)),
            ln(F64x4::new(x)),
            x.map(super::super::ln),
        );
    }

    /// [`atan2`] of `y` and `x` against [`super::atan2`], lane by lane.
    #[track_caller]
    fn assert_atan2(y: [f64; 4], x: [f64; 4]) {
        let scalar: [f64; 4] = core::array::from_fn(|k| super::super::atan2(y[k], x[k]));
        assert_lanes(
            "atan2",
            &format!("{:?} over {:?}", y.map(shown), x.map(shown)),
            atan2(F64x4::new(y), F64x4::new(x)),
            scalar,
        );
    }

    /// A random bit pattern in `[low, high)`.
    fn random_bits(g: &mut Lcg, low: u64, high: u64) -> f64 {
        f64::from_bits(low + g.next_below(high - low))
    }

    /// The bits of the smallest normal and of infinity.
    const SMALLEST_NORMAL: u64 = 0x0010_0000_0000_0000;
    const INFINITY: u64 = 0x7ff0_0000_0000_0000;

    /// Over random positive normal finite lanes, every pattern of bits equally likely, and over
    /// lanes of the spray's table, 2⁻⁴ to 2²¹ ly, the lanes' logarithms are `libm`'s, by the main
    /// path.
    #[test]
    fn ln_is_libm_s_over_positive_normal_lanes() {
        let mut g = Lcg::new(0x1a4e_5000);
        for _ in 0..50_000 {
            assert_ln(core::array::from_fn(|_| {
                random_bits(&mut g, SMALLEST_NORMAL, INFINITY)
            }));
        }
        for _ in 0..50_000 {
            assert_ln(core::array::from_fn(|_| {
                0.0625 * super::super::exp2(25.0 * g.next_f64())
            }));
        }
    }

    /// At the edges of `log`'s main path and its reduction: 1, whose logarithm `log` returns
    /// before the reduction; the smallest normal and the largest finite; and √2 ÷ 2 at the
    /// reduction's split (`0x3fe6a09e`), with its neighbours, each in lanes with others.
    #[test]
    fn ln_is_libm_s_at_the_edges_of_its_main_path() {
        let split = f64::from_bits(0x3fe6_a09e_0000_0000);
        let edges = [
            1.0,
            f64::from_bits(0x3fef_ffff_ffff_ffff),
            f64::from_bits(0x3ff0_0000_0000_0001),
            f64::MIN_POSITIVE,
            f64::from_bits(SMALLEST_NORMAL + 1),
            f64::MAX,
            split,
            f64::from_bits(0x3fe6_a09e_0000_0000 - 1),
            f64::from_bits(0x3fe6_a09d_ffff_ffff),
            f64::from_bits(0x3fe6_a09e_ffff_ffff),
            2.0 * split,
            0.0625,
            2.0,
            0.5,
        ];
        for &a in &edges {
            for &b in &edges {
                assert_ln([a, b, 1.0, a]);
                assert_ln([b, a, a, b]);
            }
        }
    }

    /// Where any lane is zero, negative, subnormal, infinite or NaN, every lane is `libm`'s,
    /// the others included.
    #[test]
    fn ln_is_libm_s_where_a_lane_is_special() {
        let specials = [
            0.0,
            -0.0,
            -1.0,
            -f64::MIN_POSITIVE,
            f64::from_bits(1),
            f64::from_bits(SMALLEST_NORMAL - 1),
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ];
        for &special in &specials {
            for k in 0..4 {
                let mut x = [0.3, 7.0, 1e300, 1.0];
                x[k] = special;
                assert_ln(x);
            }
        }
        assert_ln(specials[..4].try_into().expect("four"));
        assert_ln(specials[4..8].try_into().expect("four"));
    }

    /// Over random positive lanes, the coordinates over 60 decades, the lanes' angles are
    /// `libm`'s: half the groups with every tangent inside `[2⁻²⁷, 7/16)`, which take the first
    /// interval's path, and half with tangents from 2⁻⁴⁰ to 16, nearly all of which have a lane
    /// off it.
    #[test]
    fn atan2_is_libm_s_over_random_positive_lanes() {
        let mut g = Lcg::new(0x1a4e_5a00);
        let mut on_the_path = 0;
        for round in 0..100_000 {
            let x: [f64; 4] =
                core::array::from_fn(|_| super::super::exp2(200.0 * g.next_f64() - 100.0));
            let y: [f64; 4] = core::array::from_fn(|k| {
                let tangent = if round % 2 == 0 {
                    super::super::exp2(-26.5 + 25.0 * g.next_f64()).min(0.43)
                } else {
                    super::super::exp2(-40.0 + 44.0 * g.next_f64())
                };
                x[k] * tangent
            });
            let tangents = F64x4::new(y) / F64x4::new(x);
            if (tangents.ge(F64x4::splat(ATAN_TINY)) & tangents.lt(F64x4::splat(ATAN_FIRST))).all()
            {
                on_the_path += 1;
            }
            assert_atan2(y, x);
        }
        assert!(
            (53_000..60_000).contains(&on_the_path),
            "{on_the_path} groups of 100,000 on the first interval's path"
        );
    }

    /// At the edges of the first interval's path: the tangent at 2⁻²⁷ and 7/16 and their
    /// neighbours, through `x = 1` (where `atan2` returns `atan(y)`) and other `x`; and every
    /// special lane `atan2` treats apart, each among lanes on the path.
    #[test]
    fn atan2_is_libm_s_at_the_edges_of_its_first_interval() {
        let tangents = [
            ATAN_TINY,
            f64::from_bits(0x3e40_0000_0000_0000 - 1),
            f64::from_bits(0x3e40_0000_0000_0001),
            ATAN_FIRST,
            f64::from_bits(0x3fdc_0000_0000_0000 - 1),
            f64::from_bits(0x3fdc_0000_0000_0001),
            0.1,
        ];
        for &t in &tangents {
            for x in [1.0, 3.0, 1e-300, 1e300, 0.7] {
                assert_atan2([t * x, 0.01 * x, t, 0.2], [x, x, 1.0, 1.0]);
            }
        }
        let specials = [
            (0.0, 1.0),
            (-0.0, 1.0),
            (0.1, 0.0),
            (0.1, -0.0),
            (0.1, -1.0),
            (-0.1, 1.0),
            (-0.1, -1.0),
            (f64::INFINITY, 1.0),
            (0.1, f64::INFINITY),
            (f64::INFINITY, f64::INFINITY),
            (f64::NAN, 1.0),
            (0.1, f64::NAN),
            (1e300, 1e-300),
            (1e-300, 1e300),
            (f64::from_bits(1), 1.0),
            (0.1, f64::from_bits(1)),
            (f64::from_bits(1), f64::from_bits(3)),
        ];
        for (y, x) in specials {
            for k in 0..4 {
                let (mut ys, mut xs) = ([0.01, 0.2, 3.0, 1e-5], [1.0, 2.0, 10.0, 1e-3]);
                ys[k] = y;
                xs[k] = x;
                assert_atan2(ys, xs);
            }
        }
    }

    /// The lanes' arithmetic is the scalar operations', lane by lane, signed zeros and NaN
    /// included, and the clamp's halves keep `f64::clamp`'s answers.
    #[test]
    fn the_lanes_are_the_scalar_operations() {
        let values = [
            0.0,
            -0.0,
            1.5,
            -2.25,
            1e-310,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ];
        for &a in &values {
            for &b in &values {
                let (x, y) = (F64x4::new([a, b, a, b]), F64x4::new([b, a, a, b]));
                let scalar = |f: fn(f64, f64) -> f64| [f(a, b), f(b, a), f(a, a), f(b, b)];
                assert_lanes("+", "", x + y, scalar(|p, q| p + q));
                assert_lanes("−", "", x - y, scalar(|p, q| p - q));
                assert_lanes("×", "", x * y, scalar(|p, q| p * q));
                assert_lanes("÷", "", x / y, scalar(|p, q| p / q));
                assert_lanes("−x", "", -x, [-a, -b, -a, -b]);
                assert_lanes("√", "", x.sqrt(), [a.sqrt(), b.sqrt(), a.sqrt(), b.sqrt()]);
                assert_lanes("|x|", "", x.abs(), [a.abs(), b.abs(), a.abs(), b.abs()]);
                let compare = |f: fn(f64, f64) -> bool| [f(a, b), f(b, a), f(a, a), f(b, b)];
                assert_eq!(x.lt(y).to_array(), compare(|p, q| p < q));
                assert_eq!(x.le(y).to_array(), compare(|p, q| p <= q));
                assert_eq!(x.gt(y).to_array(), compare(|p, q| p > q));
                assert_eq!(x.ge(y).to_array(), compare(|p, q| p >= q));
                assert_lanes(
                    "select",
                    "",
                    x.lt(y).select(x, y),
                    scalar(|p, q| if p < q { p } else { q }),
                );
            }
            let (low, high) = (F64x4::splat(-1.0), F64x4::splat(2.0));
            let clamped = F64x4::splat(a).at_least(low).at_most(high);
            assert_lanes("clamp", "", clamped, [a.clamp(-1.0, 2.0); 4]);
        }
        let mask = F64x4::new([1.0, 2.0, 3.0, 4.0]).gt(F64x4::splat(2.5));
        assert!(mask.any() && !mask.all());
        assert!((mask & F64x4::splat(1.0).gt(F64x4::splat(0.0))).any());
        assert!(F64x4::splat(1.0).lt(F64x4::splat(2.0)).all());
        assert!(!F64x4::splat(f64::NAN).lt(F64x4::splat(2.0)).any());
    }
}
