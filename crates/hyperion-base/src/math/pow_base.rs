// Ported from the pinned `libm` 0.2.16's `src/math/pow.rs`
// (https://github.com/rust-lang/compiler-builtins, MIT licence; see NOTICE), whose notice for the
// FreeBSD source it ports is preserved here as that file carries it:
//
/* origin: FreeBSD /usr/src/lib/msun/src/e_pow.c */
/*
 * ====================================================
 * Copyright (C) 2004 by Sun Microsystems, Inc. All rights reserved.
 *
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */

//! `PowBase`: one base raised to several real powers, each bit for bit the pinned `libm`'s `pow`,
//! with the base's logarithm taken once.
//!
//! A sum of terms aᵢ x^tᵢ over one x, such as a phase diagram's Wagner equation (IAPWS SR1-86's six
//! powers of one θ), paid for `pow`'s logarithm stage once a term, though that stage reads x alone.
//! R09.T16.a's attribution on the reference Earth at level 8 counted water's ln `p_sat`(T) 140 M
//! times in the climate's τ fit (about 35 s of its 37.5 s), 25 M in the erosion and 9 M in the
//! water cycle, at 196–337 ns a call by temperature band (312–337 ns over 273–310 K; R09.T16.a's
//! probe, test profile, 2026-10-23), against 25–30 ns for the held curve before `032a8c5f`. Here
//! the stage runs once a base, and the rest of `pow` once a power, both transcribed from `libm`
//! 0.2.16's `src/math/pow.rs` line for line; `PowBase`'s documentation gives the argument that
//! every result is `pow`'s to the bit.

/// `pow`'s constants (`libm` 0.2.16's `src/math/pow.rs`, lines 62–87), each line as that file
/// writes it, literal and bit comment, less the three of the |y| > 2³¹ branch that falls back here.
#[expect(
    clippy::excessive_precision,
    clippy::unreadable_literal,
    clippy::approx_constant,
    reason = "libm's constants copied literally from pow.rs, so that each can be checked against \
              it character by character, `LG2` among them, libm's literal for ln 2 (the same f64 \
              as `LN_2`); the tests hold the values to pow's own"
)]
mod libm_constants {
    pub(super) const BP: [f64; 2] = [1.0, 1.5];
    pub(super) const DP_H: [f64; 2] = [0.0, 5.84962487220764160156e-01]; /* 0x3fe2b803_40000000 */
    pub(super) const DP_L: [f64; 2] = [0.0, 1.35003920212974897128e-08]; /* 0x3E4CFDEB, 0x43CFD006 */
    pub(super) const TWO53: f64 = 9007199254740992.0; /* 0x43400000_00000000 */
    pub(super) const HUGE: f64 = 1.0e300;
    pub(super) const TINY: f64 = 1.0e-300;

    // poly coefs for (3/2)*(log(x)-2s-2/3*s**3:
    pub(super) const L1: f64 = 5.99999999999994648725e-01; /* 0x3fe33333_33333303 */
    pub(super) const L2: f64 = 4.28571428578550184252e-01; /* 0x3fdb6db6_db6fabff */
    pub(super) const L3: f64 = 3.33333329818377432918e-01; /* 0x3fd55555_518f264d */
    pub(super) const L4: f64 = 2.72728123808534006489e-01; /* 0x3fd17460_a91d4101 */
    pub(super) const L5: f64 = 2.30660745775561754067e-01; /* 0x3fcd864a_93c9db65 */
    pub(super) const L6: f64 = 2.06975017800338417784e-01; /* 0x3fca7e28_4a454eef */
    pub(super) const P1: f64 = 1.66666666666666019037e-01; /* 0x3fc55555_5555553e */
    pub(super) const P2: f64 = -2.77777777770155933842e-03; /* 0xbf66c16c_16bebd93 */
    pub(super) const P3: f64 = 6.61375632143793436117e-05; /* 0x3f11566a_af25de2c */
    pub(super) const P4: f64 = -1.65339022054652515390e-06; /* 0xbebbbd41_c5d26bf1 */
    pub(super) const P5: f64 = 4.13813679705723846039e-08; /* 0x3e663769_72bea4d0 */
    pub(super) const LG2: f64 = 6.93147180559945286227e-01; /* 0x3fe62e42_fefa39ef */
    pub(super) const LG2_H: f64 = 6.93147182464599609375e-01; /* 0x3fe62e43_00000000 */
    pub(super) const LG2_L: f64 = -1.90465429995776804525e-09; /* 0xbe205c61_0ca86c39 */
    pub(super) const OVT: f64 = 8.0085662595372944372e-017; /* -(1024-log2(ovfl+.5ulp)) */
    pub(super) const CP: f64 = 9.61796693925975554329e-01; /* 0x3feec709_dc3a03fd =2/(3ln2) */
    pub(super) const CP_H: f64 = 9.61796700954437255859e-01; /* 0x3feec709_e0000000 =(float)cp */
    pub(super) const CP_L: f64 = -7.02846165095275826516e-09; /* 0xbe3e2fe0_145b01f5 =tail of cp_h*/
}

use libm_constants::{
    BP, CP, CP_H, CP_L, DP_H, DP_L, HUGE, L1, L2, L3, L4, L5, L6, LG2, LG2_H, LG2_L, OVT, P1, P2,
    P3, P4, P5, TINY, TWO53,
};

/// A base x for several real powers, each x^y bit for bit [`powf`](super::powf)`(x, y)`, with the
/// logarithm `powf` takes of x taken once.
///
/// [`powf`](super::powf) is the pinned `libm`'s `pow` (0.2.16, `src/math/pow.rs`, a port of
/// FreeBSD's `e_pow.c` from Sun's fdlibm). It writes log₂ x as two pieces `t1 + t2`, multiplies
/// them by y in simulated extra precision, and raises 2 to the product. [`PowBase::new`] runs its
/// logarithm stage (pow.rs lines 264–330) once, and [`PowBase::powf`] the rest (lines 332–401),
/// both transcribed line for line: the same constants, the same IEEE operations on the same
/// operands in the same order, and the same reinterpretations of the words. Rust neither fuses nor
/// reorders floating-point operations, so the same operations give the same bits, on every
/// target.
///
/// **Why each result is `pow`'s, to the bit.** For x > 0 finite and x ≠ 1, and y finite with
/// y ∉ {0, ±1, 2, ½} and |y| ≤ 2³¹ (`pow`'s `iy ≤ 0x41e00000`), `pow` takes none of its early
/// returns: y = ±0 (pow.rs lines 104–107), x = 1 (109–112), a NaN (114–121), the special y of
/// 153–187 (±∞, ±1, 2, ½), the special x of 189–211 (±0, ±∞, ±1) and a negative x's (213–224); nor
/// its other logarithm stage, for |y| > 2³¹ (226–263), the only branch by which y chooses the
/// stage. Its logarithm stage then reads x alone, and everything after it reads y, `t1`, `t2` and
/// the sign `s`, which is 1 for a positive x. Every other pair, and every power of any other base
/// (±0, negative, infinite, NaN or 1), is `libm::pow`'s own call, so the result is `pow`'s for
/// every input pair. A test holds the two to the bit over the phase diagrams' exponents and those
/// less 1, random exponents, dense and random bases, subnormals, the logarithm's interval cuts,
/// the overflow and underflow thresholds and every special input.
///
/// # Examples
///
/// ```
/// use hyperion_base::math::{PowBase, powf};
///
/// // IAPWS SR1-86's six Wagner exponents of one θ: one logarithm, and each power `powf`'s bits
/// // (`total_cmp` orders floats by their bits, so it finds two equal only where they are one).
/// let theta = 1.0 - 300.0 / 647.096;
/// let base = PowBase::new(theta);
/// for exponent in [1.0, 1.5, 3.0, 3.5, 4.0, 7.5] {
///     assert!(base.powf(exponent).total_cmp(&powf(theta, exponent)).is_eq());
/// }
/// // Outside the shared stage's domain it is `powf` itself: here θ < 0, above the critical point.
/// assert!(PowBase::new(-0.25).powf(1.5).is_nan());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowBase {
    /// The base.
    x: f64,
    /// `pow`'s log₂ x in two pieces, `(t1, t2)`, where its logarithm stage reads x alone: x > 0,
    /// finite and not 1. `None` for every other x, whose powers are `libm::pow`'s own calls.
    log2: Option<(f64, f64)>,
}

impl PowBase {
    /// The base `x`, with `pow`'s logarithm stage run for it where `pow` would run that stage.
    #[inline]
    #[must_use]
    pub fn new(x: f64) -> Self {
        let (hx, lx) = (get_high_word(x), get_low_word(x));
        // `pow`'s tests of x alone, on its words (pow.rs line 98): x = 1 (line 110), a NaN
        // (115–116), a negative x's paths (131–151, 213–224) and ±0, ±∞ (189–211). As an unsigned
        // word, a high word under 0x7ff00000 is a positive finite x or +0, and is `pow`'s `ix`
        // (line 101).
        let staged = hx < 0x7ff0_0000 && (hx | lx) != 0 && !(hx == 0x3ff0_0000 && lx == 0);
        Self {
            x,
            log2: staged.then(|| log2_stage(x, hx.cast_signed())),
        }
    }

    /// x^y, bit for bit [`powf`](super::powf)`(x, y)`.
    #[inline]
    #[must_use]
    pub fn powf(&self, y: f64) -> f64 {
        match self.log2 {
            Some((t1, t2)) if reaches_the_tail(y) => tail(y, t1, t2),
            _ => libm::pow(self.x, y),
        }
    }
}

/// Whether `pow`, for an x whose logarithm stage reads x alone, reaches that stage for `y`: that
/// it takes none of its returns on y (pow.rs line 105, y = ±0; lines 115–121, a NaN; 155, ±∞;
/// 170, ±1; 175, 2; 180, ½, for a positive x) and not its other stage, for |y| > 2³¹ (line 227,
/// which every NaN and ∞ also passes).
#[inline]
#[must_use]
fn reaches_the_tail(y: f64) -> bool {
    let (hy, ly) = (get_high_word(y).cast_signed(), get_low_word(y));
    let iy: i32 = hy & 0x7fff_ffff_i32;
    let special = (iy.cast_unsigned() | ly) == 0
        || iy > 0x41e0_0000
        || (ly == 0 && (iy == 0x3ff0_0000 || hy == 0x4000_0000 || hy == 0x3fe0_0000));
    !special
}

/// `pow`'s logarithm stage (pow.rs lines 264–330) for x > 0, finite and not 1, whose high word is
/// `ix`: log₂ x as `(t1, t2)`, `t1` with its low word cleared.
///
/// For a positive x, `pow`'s `ax = fabs(x)` (line 189) is x. `pow`'s `k` is an `i32` that is 0 or
/// 1; here it is a `u8`, so that it indexes and shifts by `From`, the same values.
#[expect(
    clippy::many_single_char_names,
    reason = "libm's names, kept so that each line can be checked against pow.rs"
)]
#[inline]
#[must_use]
fn log2_stage(x: f64, ix: i32) -> (f64, f64) {
    let mut ax: f64 = x;
    let mut ix: i32 = ix;
    let mut n: i32 = 0;

    if ix < 0x0010_0000 {
        // take care subnormal number
        ax *= TWO53;
        n -= 53;
        ix = get_high_word(ax).cast_signed();
    }

    n += (ix >> 20) - 0x3ff;
    let j: i32 = ix & 0x000f_ffff;

    // determine interval
    ix = j | 0x3ff0_0000; // normalize ix
    let k: u8 = if j <= 0x3988E {
        // |x|<sqrt(3/2)
        0
    } else if j < 0xBB67A {
        // |x|<sqrt(3)
        1
    } else {
        n += 1;
        ix -= 0x0010_0000;
        0
    };
    let kk = usize::from(k);
    ax = with_set_high_word(ax, ix.cast_unsigned());

    // compute ss = s_h+s_l = (x-1)/(x+1) or (x-1.5)/(x+1.5)
    let u: f64 = ax - BP[kk]; // bp[0]=1.0, bp[1]=1.5
    let v: f64 = 1.0 / (ax + BP[kk]);
    let ss: f64 = u * v;
    let s_h = with_set_low_word(ss, 0);

    // t_h=ax+bp[k] High
    let t_h: f64 = with_set_high_word(
        0.0,
        ((ix.cast_unsigned() >> 1) | 0x2000_0000) + 0x0008_0000 + (u32::from(k) << 18),
    );
    let t_l: f64 = ax - (t_h - BP[kk]);
    let s_l: f64 = v * ((u - s_h * t_h) - s_h * t_l);

    // compute log(ax)
    let s2: f64 = ss * ss;
    let mut r: f64 = s2 * s2 * (L1 + s2 * (L2 + s2 * (L3 + s2 * (L4 + s2 * (L5 + s2 * L6)))));
    r += s_l * (s_h + ss);
    let s2: f64 = s_h * s_h;
    let t_h: f64 = with_set_low_word(3.0 + s2 + r, 0);
    let t_l: f64 = r - ((t_h - 3.0) - s2);

    // u+v = ss*(1+...)
    let u: f64 = s_h * t_h;
    let v: f64 = s_l * t_h + t_l * ss;

    // 2/(3log2)*(ss+...)
    let p_h: f64 = with_set_low_word(u + v, 0);
    let p_l = v - (p_h - u);
    let z_h: f64 = CP_H * p_h; // cp_h+cp_l = 2/(3*log2)
    let z_l: f64 = CP_L * p_h + p_l * CP + DP_L[kk];

    // log2(ax) = (ss+..)*2/(3*log2) = n + dp_h + z_h + z_l
    let t: f64 = f64::from(n);
    let t1 = with_set_low_word(((z_h + z_l) + DP_H[kk]) + t, 0);
    let t2 = z_l - (((t1 - t) - DP_H[kk]) - z_h);
    (t1, t2)
}

/// `pow` after its logarithm stage (pow.rs lines 332–401), for a positive x, whose sign `s` is 1,
/// and a y that reaches it: y log₂ x from y's split into y1 + y2 and `(t1, t2)`, its overflow and
/// underflow, and 2 raised to it.
#[expect(
    clippy::many_single_char_names,
    reason = "libm's names, kept so that each line can be checked against pow.rs"
)]
#[inline]
#[must_use]
fn tail(y: f64, t1: f64, t2: f64) -> f64 {
    // pow's sign of the result (line 213), 1 for a positive x.
    let s: f64 = 1.0;

    // split up y into y1+y2 and compute (y1+y2)*(t1+t2)
    let y1: f64 = with_set_low_word(y, 0);
    let p_l: f64 = (y - y1) * t1 + y * t2;
    let mut p_h: f64 = y1 * t1;
    let z: f64 = p_l + p_h;
    let mut j: i32 = get_high_word(z).cast_signed();
    let i: i32 = get_low_word(z).cast_signed();

    if j >= 0x4090_0000 {
        // z >= 1024
        if ((j - 0x4090_0000) | i) != 0 {
            // if z > 1024
            return s * HUGE * HUGE; // overflow
        }

        if p_l + OVT > z - p_h {
            return s * HUGE * HUGE; // overflow
        }
    } else if (j & 0x7fff_ffff) >= 0x4090_cc00 {
        // z <= -1075. Here j < 0x40900000 and j's magnitude is at least 0x4090cc00, so j's sign is
        // set and its word at least 0xc090cc00: the subtraction cannot wrap, in libm or here.
        if ((j.cast_unsigned() - 0xc090_cc00) | i.cast_unsigned()) != 0 {
            // z < -1075
            return s * TINY * TINY; // underflow
        }

        if p_l <= z - p_h {
            return s * TINY * TINY; // underflow
        }
    }

    // compute 2**(p_h+p_l)
    let i: i32 = j & 0x7fff_ffff_i32;
    let mut k: i32 = (i >> 20) - 0x3ff;
    let mut n: i32 = 0;

    if i > 0x3fe0_0000 {
        // if |z| > 0.5, set n = [z+0.5]
        n = j + (0x0010_0000 >> (k + 1));
        k = ((n & 0x7fff_ffff) >> 20) - 0x3ff; // new k for n
        let t: f64 = with_set_high_word(0.0, (n & !(0x000f_ffff >> k)).cast_unsigned());
        n = ((n & 0x000f_ffff) | 0x0010_0000) >> (20 - k);
        if j < 0 {
            n = -n;
        }
        p_h -= t;
    }

    let t: f64 = with_set_low_word(p_l + p_h, 0);
    let u: f64 = t * LG2_H;
    let v: f64 = (p_l - (t - p_h)) * LG2 + t * LG2_L;
    let mut z: f64 = u + v;
    let w: f64 = v - (z - u);
    let t: f64 = z * z;
    let t1: f64 = z - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    let r: f64 = (z * t1) / (t1 - 2.0) - (w + z * w);
    z = 1.0 - (r - z);
    j = get_high_word(z).cast_signed();
    j += n << 20;

    if (j >> 20) <= 0 {
        // subnormal output
        z = libm::scalbn(z, n);
    } else {
        z = with_set_high_word(z, j.cast_unsigned());
    }

    s * z
}

/// `libm`'s `get_high_word` (its `src/math/mod.rs`): the high 32 bits of `x`.
#[expect(
    clippy::disallowed_methods,
    reason = "libm's own reading of a float's high word, which pow's arithmetic is made of: the \
              bits are computed with and never hashed"
)]
#[inline]
#[must_use]
fn get_high_word(x: f64) -> u32 {
    // Lossless: a u64 shifted right by 32 has at most 32 significant bits, so it fits a u32.
    (x.to_bits() >> 32) as u32
}

/// `libm`'s `get_low_word` (its `src/math/mod.rs`): the low 32 bits of `x`.
#[expect(
    clippy::disallowed_methods,
    clippy::cast_possible_truncation,
    reason = "libm's own reading of a float's low word, which pow's arithmetic is made of: the \
              bits are computed with and never hashed, and the truncation to the low word is the \
              point"
)]
#[inline]
#[must_use]
fn get_low_word(x: f64) -> u32 {
    x.to_bits() as u32
}

/// `libm`'s `with_set_high_word` (its `src/math/mod.rs`): `f` with its high 32 bits `hi`.
#[expect(
    clippy::disallowed_methods,
    reason = "libm's own setting of a float's high word, which pow's arithmetic is made of: the \
              bits are computed with and never hashed"
)]
#[inline]
#[must_use]
fn with_set_high_word(f: f64, hi: u32) -> f64 {
    let mut tmp = f.to_bits();
    tmp &= 0x0000_0000_ffff_ffff;
    tmp |= u64::from(hi) << 32;
    f64::from_bits(tmp)
}

/// `libm`'s `with_set_low_word` (its `src/math/mod.rs`): `f` with its low 32 bits `lo`.
#[expect(
    clippy::disallowed_methods,
    reason = "libm's own setting of a float's low word, which pow's arithmetic is made of: the \
              bits are computed with and never hashed"
)]
#[inline]
#[must_use]
fn with_set_low_word(f: f64, lo: u32) -> f64 {
    let mut tmp = f.to_bits();
    tmp &= 0xffff_ffff_0000_0000;
    tmp |= u64::from(lo);
    f64::from_bits(tmp)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;
    use hyperion_testkit::lcg::Lcg;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    /// Every exponent of the sim's phase diagrams (`substance::rows::diagrams`, its `terms`
    /// tables: the Wagner equations, the sublimation pieces, the liquid densities and the melting
    /// equations), each once, in the order the rows first give it.
    const DIAGRAM_EXPONENTS: [f64; 133] = [
        1.0,
        1.5,
        3.0,
        3.5,
        4.0,
        7.5,
        1.0 / 3.0,
        2.0 / 3.0,
        5.0 / 3.0,
        16.0 / 3.0,
        43.0 / 3.0,
        110.0 / 3.0,
        0.003_333_333_33,
        1.206_666_67,
        1.703_333_33,
        25.75,
        103.75,
        60.0,
        8.0,
        4.6,
        -1.0,
        5.0,
        22.0,
        2.0,
        4.5,
        0.354,
        0.5,
        2.5,
        1.85,
        2.1,
        1.6,
        3.7,
        0.07,
        0.46,
        0.77,
        1.05,
        1.25,
        4.4,
        1.51,
        0.3294,
        8.0 / 3.0,
        35.0 / 6.0,
        1.789_63,
        7.0,
        9.0,
        0.34,
        10.0 / 6.0,
        11.0 / 6.0,
        1.9,
        2.9,
        0.334,
        7.0 / 3.0,
        2.7,
        1.275,
        0.822,
        0.961,
        1.811,
        2.937,
        13.131,
        15.629,
        0.423,
        1.611,
        2.063,
        6.632,
        12.833,
        19.438,
        1.057,
        1.12,
        3.837,
        4.253,
        9.273,
        1.349,
        0.084,
        0.346,
        2.447,
        5.49,
        6.019,
        15.158,
        1.06,
        1.105,
        1.754,
        4.415,
        9.887,
        12.55,
        0.374,
        6.796,
        13.027,
        13.205,
        14.362,
        18.638,
        0.052,
        0.968,
        1.972,
        2.761,
        4.199,
        5.126,
        0.44,
        0.672,
        1.334,
        3.874,
        4.413,
        9.257,
        2.2,
        4.7,
        6.0,
        0.54,
        0.88,
        1.23,
        1.003,
        1.146,
        1.668,
        4.886,
        1.558,
        9.098,
        0.572,
        0.586,
        2.075,
        3.069,
        4.223,
        4.619,
        0.871,
        0.902,
        1.663,
        2.045,
        3.946,
        14.704,
        0.211,
        0.408,
        1.789,
        10.003,
        12.669,
        16.309,
        2.55,
    ];

    /// The diagram exponents and each less 1 (`slope_k`'s θ^(t − 1)), computed as the sim does.
    fn exponents() -> Vec<f64> {
        DIAGRAM_EXPONENTS
            .iter()
            .flat_map(|&e| [e, e - 1.0])
            .collect()
    }

    /// A float's bits for a failure message, or "NaN".
    fn shown(value: f64) -> String {
        if value.is_nan() {
            format!("{value:?}")
        } else {
            format!("{value:e} (0x{:016x})", bits(value))
        }
    }

    /// `base.powf(y)` against `libm::pow(x, y)`, `base` being `PowBase::new(x)`: the same bits,
    /// or both NaN, whose bits Rust leaves unspecified (the testkit's `bits` refuses them).
    #[track_caller]
    fn assert_is_pow(base: &PowBase, x: f64, y: f64) {
        let (ours, libms) = (base.powf(y), libm::pow(x, y));
        if ours.is_nan() || libms.is_nan() {
            assert!(
                ours.is_nan() && libms.is_nan(),
                "{}^{}: {} against libm's {}",
                shown(x),
                shown(y),
                shown(ours),
                shown(libms)
            );
        } else {
            assert_eq!(
                bits(ours),
                bits(libms),
                "{}^{}: {} against libm's {}",
                shown(x),
                shown(y),
                shown(ours),
                shown(libms)
            );
        }
    }

    /// A random finite exponent in (−64, 64).
    fn random_exponent(g: &mut Lcg) -> f64 {
        128.0 * g.next_f64() - 64.0
    }

    /// A random bit pattern in `[low, high)`.
    fn random_bits(g: &mut Lcg, low: u64, high: u64) -> f64 {
        f64::from_bits(low + g.next_below(high - low))
    }

    /// `x` against `libm::pow` for every exponent of `ys`.
    fn assert_every_power_is_pow(x: f64, ys: &[f64]) {
        let base = PowBase::new(x);
        for &y in ys {
            assert_is_pow(&base, x, y);
        }
    }

    /// The bits of 1.0, 2⁻¹⁰²² (the smallest normal) and 2¹⁰ (the top of the sweep above 1).
    const ONE: u64 = 0x3ff0_0000_0000_0000;
    const SMALLEST_NORMAL: u64 = 0x0010_0000_0000_0000;
    const TWO_TO_TEN: u64 = 0x4090_0000_0000_0000;

    /// The logarithm stage runs for every positive finite base but 1, the subnormals included, and
    /// for no other; and the tail for every finite exponent but `pow`'s special ones.
    #[test]
    fn the_shared_stage_runs_where_pow_reaches_it() {
        for x in [
            5e-324,
            f64::MIN_POSITIVE,
            0.25,
            1.0 - f64::EPSILON / 2.0,
            1.0 + f64::EPSILON,
            7.0,
            f64::MAX,
        ] {
            assert!(PowBase::new(x).log2.is_some(), "{x:e}");
        }
        for x in [
            0.0,
            -0.0,
            1.0,
            -1.0,
            -0.25,
            -5e-324,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
            -f64::NAN,
        ] {
            assert!(PowBase::new(x).log2.is_none(), "{x:e}");
        }
        let two_31 = 2_147_483_648.0;
        for y in [
            1.5,
            -2.0,
            -0.5,
            3.0,
            -1074.0,
            5e-324,
            two_31,
            -two_31,
            two_31 + 1.0,
            f64::from_bits(0x41e0_0000_ffff_ffff),
        ] {
            assert!(reaches_the_tail(y), "{y:e}");
        }
        for y in [
            0.0,
            -0.0,
            1.0,
            -1.0,
            2.0,
            0.5,
            f64::from_bits(0x41e0_0001_0000_0000),
            2.0 * two_31,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            assert!(!reaches_the_tail(y), "{y:e}");
        }
    }

    /// θ = 1 − T ÷ `T_c` across (0, 1): 10⁵ evenly spaced with every diagram exponent and each less
    /// 1, and 10⁶ evenly spaced with two of them in turn and two random exponents each.
    #[test]
    fn powf_is_pow_over_an_even_sweep_of_theta() {
        let ys = exponents();
        for i in 1..100_000_u32 {
            assert_every_power_is_pow(f64::from(i) / 100_000.0, &ys);
        }
        let mut g = Lcg::new(0x0916_a001);
        let count = u32::try_from(ys.len()).unwrap();
        for i in 1..1_000_000_u32 {
            let x = f64::from(i) / 1_000_000.0;
            let base = PowBase::new(x);
            for y in [
                ys[usize::try_from(i % count).unwrap()],
                ys[usize::try_from((i / count) % count).unwrap()],
                random_exponent(&mut g),
                random_exponent(&mut g),
            ] {
                assert_is_pow(&base, x, y);
            }
        }
    }

    /// 10⁶ random bit patterns in [2⁻¹⁰²², 1), 10⁵ in (1, 2¹⁰) and 10⁵ across every positive
    /// finite float, each with two diagram exponents, one less 1 and two random exponents in
    /// (−64, 64).
    #[test]
    fn powf_is_pow_for_random_bases() {
        let mut g = Lcg::new(0x0916_a002);
        let count = u64::try_from(DIAGRAM_EXPONENTS.len()).unwrap();
        let diagram_exponent =
            |g: &mut Lcg| DIAGRAM_EXPONENTS[usize::try_from(g.next_below(count)).unwrap()];
        for (n, low, high) in [
            (1_000_000, SMALLEST_NORMAL, ONE),
            (100_000, ONE + 1, TWO_TO_TEN),
            (100_000, 1, 0x7ff0_0000_0000_0000),
        ] {
            for _ in 0..n {
                let x = random_bits(&mut g, low, high);
                let base = PowBase::new(x);
                for y in [
                    diagram_exponent(&mut g),
                    diagram_exponent(&mut g),
                    diagram_exponent(&mut g) - 1.0,
                    random_exponent(&mut g),
                    random_exponent(&mut g),
                ] {
                    assert_is_pow(&base, x, y);
                }
            }
        }
    }

    /// The logarithm stage's edges, with every diagram exponent, each less 1, and 32 random
    /// exponents: subnormal bases (scaled by 2⁵³ first), 1 ± k ulp for k ≤ 64, and mantissas at and
    /// beside the interval cuts 0x3988E (√(3/2), k = 0 below, 1 above) and 0xBB67A (√3, above which
    /// x is halved and n rises), each with 32 low words, over the exponents' range.
    #[test]
    fn powf_is_pow_at_the_edges_of_the_logarithm_stage() {
        let mut g = Lcg::new(0x0916_a003);
        let mut ys = exponents();
        ys.extend((0..32).map(|_| random_exponent(&mut g)));
        let mut xs = vec![
            f64::from_bits(1),
            f64::from_bits(0x000f_ffff_ffff_ffff),
            f64::from_bits(0x0008_0000_0000_0000),
            f64::MIN_POSITIVE,
        ];
        xs.extend((0..1_000).map(|_| random_bits(&mut g, 1, SMALLEST_NORMAL)));
        for k in 1..=64 {
            xs.push(f64::from_bits(ONE - k));
            xs.push(f64::from_bits(ONE + k));
        }
        for exponent in [0_u64, 1, 0x3f0, 0x3fe, 0x3ff, 0x400, 0x409, 0x7fe] {
            for mantissa_high in [
                0_u64, 1, 0x3988D, 0x3988E, 0x3988F, 0xBB679, 0xBB67A, 0xBB67B, 0xF_FFFF,
            ] {
                let random_lows: Vec<u64> = (0..28).map(|_| g.next_below(1 << 32)).collect();
                for low in [0_u64, 1, 0x8000_0000, 0xffff_ffff]
                    .into_iter()
                    .chain(random_lows)
                {
                    let x = f64::from_bits((exponent << 52) | (mantissa_high << 32) | low);
                    if x > 0.0 {
                        xs.push(x);
                    }
                }
            }
        }
        for x in xs {
            assert_every_power_is_pow(x, &ys);
        }
    }

    /// The tail's branches: powers at and beside the overflow threshold (z = y log₂ x = 1024), the
    /// underflow threshold (z = −1075), the subnormal results (z below −1022) and z = ±½, where its
    /// 2^z takes its integer part, for bases near and far from 1.
    #[test]
    fn powf_is_pow_at_the_overflow_underflow_and_subnormal_results() {
        let bases = [
            2.0,
            0.5,
            4.0,
            0.25,
            10.0,
            0.1,
            core::f64::consts::E,
            1.5,
            0.75,
            1.000_000_1,
            0.999_999_9,
            1.0 + f64::EPSILON,
            1.0 - f64::EPSILON,
            3.0,
            1.0 / 3.0,
            1e-300,
            1e300,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
            f64::MAX,
        ];
        let targets = [
            1023.0, 1023.5, 1023.9, 1024.0, 1024.1, 1025.0, -1021.0, -1022.0, -1022.1, -1023.0,
            -1073.9, -1074.0, -1074.5, -1075.0, -1075.1, -1076.0, -1080.0, 0.5, -0.5, 0.49, 0.51,
        ];
        for x in bases {
            let base = PowBase::new(x);
            let log = super::super::log2(x);
            for target in targets {
                let mut y = target / log;
                for _ in 0..3 {
                    y = y.next_down();
                }
                for _ in 0..7 {
                    assert_is_pow(&base, x, y);
                    assert_is_pow(&base, x, -y);
                    y = y.next_up();
                }
                assert_is_pow(&base, x, target);
                assert_is_pow(&base, x, -target);
            }
        }
    }

    /// Every input `pow` treats specially, as base or exponent, and the edges of the exponent's
    /// domain: each pair is `libm::pow`'s own, through the fall-back or the shared stage.
    #[test]
    fn powf_is_pow_for_every_special_input() {
        let two_31 = 2_147_483_648.0_f64;
        let two_64 = 18_446_744_073_709_551_616.0_f64;
        let xs = [
            0.0,
            -0.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
            -f64::NAN,
            1.0,
            -1.0,
            -0.3,
            -0.25,
            -2.0,
            -7.5,
            -5e-324,
            -f64::MIN_POSITIVE,
            -f64::MAX,
            5e-324,
            f64::MIN_POSITIVE,
            f64::MAX,
            0.3,
            0.5,
            2.0,
            7.0,
            1.0 + f64::EPSILON,
            1.0 - f64::EPSILON / 2.0,
        ];
        let mut ys = vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            2.0,
            -2.0,
            0.5,
            -0.5,
            3.0,
            -3.0,
            4.0,
            -7.0,
            1.0 / 3.0,
            7.5,
            two_31 - 1.0,
            two_31,
            two_31 + 1.0,
            f64::from_bits(0x41e0_0000_0000_0001),
            f64::from_bits(0x41e0_0000_ffff_ffff),
            f64::from_bits(0x41e0_0001_0000_0000),
            two_64,
            two_64.next_up(),
            two_64.next_down(),
            9_007_199_254_740_992.0,
            1e17,
            1e300,
            f64::MAX,
            f64::INFINITY,
            f64::NAN,
            5e-324,
            f64::MIN_POSITIVE,
            1e-20,
        ];
        let negated: Vec<f64> = ys.iter().map(|&y| -y).collect();
        ys.extend(negated);
        ys.extend(exponents());
        for x in xs {
            assert_every_power_is_pow(x, &ys);
        }
    }
}
