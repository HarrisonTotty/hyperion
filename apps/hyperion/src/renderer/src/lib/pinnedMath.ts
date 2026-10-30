/**
 * The client's transcendental functions, pinned to the sim's: `exp`, `log`, `log2`, `log1p`,
 * `expm1`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `cbrt` and `pow`, each ported line for line
 * from the `libm` crate at 0.2.16, the version the workspace pins exactly for `hyperion_base::math`
 * (plan R08, R08.T20).
 *
 * @remarks
 * Every module whose output is a committed table computes its transcendentals here, never through
 * `Math` or `**`. A committed table is compared with its file bit for bit (`test/bless.ts`), and
 * `Math`'s functions are not one function across platforms: V8 sends `Math.pow` and `**` to the
 * platform's own `pow` (except for the exponents 2 and 0.5, which it computes as `x * x` and
 * `Math.sqrt(x)`), and builds its fdlibm port of the rest with the host compiler, which fuses
 * multiplies and adds on arm64. So Node on macOS arm64 gave other last bits than Node on Linux
 * x86-64 in 17 committed-table tests, by up to 6 × 10⁻¹³ relative in Mars's dust phase fit (plan
 * R08, R08.T20).
 *
 * JavaScript's `+ - * /` and `Math.sqrt` are IEEE 754's correctly rounded operations on every
 * platform, and V8's compilers never fuse a multiply and an add, so these ports, which use nothing
 * else (with `Math.abs`, `Math.floor` and the reinterpretation of a double's bits, all exact), give
 * the same bits everywhere: the bits of `libm` itself, as `hyperion_base::math` returns them on
 * every target of the sim. `src/math/<name>.rs` below is that crate's file. `crates/hyperion-base`'s
 * `tests/client_math_golden.rs` writes `libm`'s values at random inputs, each function's special
 * cases and inputs near a rounding midpoint, and `pinnedMath.test.ts` holds these functions to
 * them bit for bit, with `libm`'s digests of a sweep of 2¹⁷ random inputs a domain.
 *
 * The ports keep `libm`'s constants (their literals as Prettier writes them; each is the double
 * of the hexadecimal comment beside it), its operations in its order, and its integer
 * arithmetic on a double's two 32-bit words (`highWord` and `lowWord`, `libm`'s `get_high_word` and
 * `get_low_word`), with `>>>` where Rust shifts a `u32` and `>>` where it shifts an `i32`. `libm`'s
 * `force_eval!` calls, which raise floating-point exception flags and change no value, are left
 * out, as are the paths no `f64` input reaches (`rem_pio2_large`'s other precisions, `cbrt`'s
 * directed roundings).
 *
 * The FreeBSD msun sources these ports descend from carry Sun Microsystems' notice, which is
 * preserved here as `libm` carries it:
 *
 * ```text
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunPro, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 * Copyright (C) 2004 by Sun Microsystems, Inc. All rights reserved.
 *
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 * ```
 *
 * (SunSoft's files, `log`, `log2`, `asin`, `acos`, `atan2`, `k_sin`, `k_cos` and `k_rem_pio2`, carry the
 * first with "SunSoft" for "SunPro"; `exp`, `pow` and `k_tan`, the second.) `cbrt` is the CORE-MATH
 * project's (`core-math/src/binary64/cbrt/cbrt.c`, Copyright (c) 2021-2022 Alexei Sibidanov, MIT
 * licence; ported to Rust in 2025 by Trevor Gross). `libm` itself is under the MIT licence; see
 * `NOTICE`.
 */

/** The scratch a double's bits are read and written through, big-endian: the high word first. */
const BITS = new DataView(new ArrayBuffer(8));

/** The high 32 bits of `x`: its sign, exponent and the significand's top 20 bits, unsigned. */
function highWord(x: number): number {
  BITS.setFloat64(0, x);
  return BITS.getUint32(0);
}

/** The low 32 bits of `x`'s significand, unsigned. */
function lowWord(x: number): number {
  BITS.setFloat64(0, x);
  return BITS.getUint32(4);
}

/** The double whose high and low words are `hi` and `lo`, each taken modulo 2³². */
function fromWords(hi: number, lo: number): number {
  BITS.setUint32(0, hi);
  BITS.setUint32(4, lo);
  return BITS.getFloat64(0);
}

/** `x` with its high word replaced: `libm`'s `with_set_high_word`. */
function withHighWord(x: number, hi: number): number {
  return fromWords(hi, lowWord(x));
}

/** `x` with its low word replaced: `libm`'s `with_set_low_word`. */
function withLowWord(x: number, lo: number): number {
  return fromWords(highWord(x), lo);
}

/**
 * The double a C99 hexadecimal literal of a normal number names, as `libm`'s `hf64!` reads it:
 * `[-]0x1[.<up to 13 hex digits>]p<exponent>`.
 *
 * @throws Error for any other text: the literals are this file's constants, so one is a bug.
 */
function hf64(text: string): number {
  const match = /^(?<sign>-?)0x1(?:\.(?<fraction>[0-9a-f]{1,13}))?p(?<exponent>[+-]?\d+)$/u.exec(
    text,
  );
  const exponent = Number(match?.groups?.["exponent"]);
  if (match === null || !Number.isInteger(exponent) || exponent < -1022 || exponent > 1023) {
    throw new Error(`${text} is not a normal double's hexadecimal literal`);
  }
  const fraction = (match.groups?.["fraction"] ?? "").padEnd(13, "0");
  const sign = match.groups?.["sign"] === "-" ? 0x80000000 : 0;
  const hi = sign + (exponent + 0x3ff) * 0x100000 + Number.parseInt(fraction.slice(0, 5), 16);
  return fromWords(hi, Number.parseInt(fraction.slice(5), 16));
}

/** 2¹⁰²³. */
const X1P1023 = fromWords(0x7fe00000, 0);

/** 2⁻⁹⁶⁹, `scalbn`'s step towards zero: 2⁻¹⁰²² × 2⁵³. */
const X1P_969 = fromWords(0x03600000, 0);

/**
 * `x` × 2ⁿ, exact unless it overflows or goes subnormal: `src/math/generic/scalbn.rs` for `f64`,
 * whose two pre-scalings let 2ⁿ lie outside the normal range.
 */
function scalbn(x: number, n: number): number {
  let y = x;
  let m = n;
  if (m > 1023) {
    y *= X1P1023;
    m -= 1023;
    if (m > 1023) {
      y *= X1P1023;
      m -= 1023;
      if (m > 1023) {
        m = 1023;
      }
    }
  } else if (m < -1022) {
    y *= X1P_969;
    m += 969;
    if (m < -1022) {
      y *= X1P_969;
      m += 969;
      if (m < -1022) {
        m = -1022;
      }
    }
  }
  return y * fromWords((0x3ff + m) * 0x100000, 0);
}

// `src/math/exp.rs` (FreeBSD's e_exp.c).
const EXP_LN2HI = 6.9314718036912381649e-1; /* 0x3fe62e42, 0xfee00000 */
const EXP_LN2LO = 1.90821492927058770002e-10; /* 0x3dea39ef, 0x35793c76 */
// exp.rs's literal for 1 ÷ ln 2, kept as written (the double Math.LOG2E is).
// oxlint-disable-next-line approx-constant
const EXP_INVLN2 = 1.442695040888963387; /* 0x3ff71547, 0x652b82fe */
const EXP_P1 = 1.66666666666666019037e-1; /* 0x3FC55555, 0x5555553E */
const EXP_P2 = -2.77777777770155933842e-3; /* 0xBF66C16C, 0x16BEBD93 */
const EXP_P3 = 6.61375632143793436117e-5; /* 0x3F11566A, 0xAF25DE2C */
const EXP_P4 = -1.6533902205465251539e-6; /* 0xBEBBBD41, 0xC5D26BF1 */
const EXP_P5 = 4.13813679705723846039e-8; /* 0x3E663769, 0x72BEA4D0 */

/**
 * e raised to `x`, bit for bit the sim's `hyperion_base::math::exp`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/exp.rs` (FreeBSD's `e_exp.c`): `x` is reduced
 * to r = x − k ln 2 with |r| ≤ ½ ln 2, e^r comes from a degree-5 Remez fit of r (e^r + 1) ÷ (e^r − 1),
 * and the result is scaled by 2ᵏ. Under 1 ulp.
 */
export function exp(x: number): number {
  let xr = x;
  let hi: number;
  let lo: number;
  let k: number;
  let hx = highWord(xr);
  const sign = hx >>> 31;
  hx &= 0x7fffffff; /* high word of |x| */

  /* special cases */
  if (hx >= 0x4086232b) {
    /* if |x| >= 708.39... */
    if (Number.isNaN(xr)) {
      return xr;
    }
    if (xr > 709.782712893383973096) {
      /* overflow if x!=inf */
      xr *= X1P1023;
      return xr;
    }
    // exp.rs's threshold as written; it reads as the nearest double, as Rust's does.
    // oxlint-disable-next-line no-loss-of-precision
    if (xr < -708.39641853226410622) {
      /* underflow if x!=-inf */
      if (xr < -745.13321910194110842) {
        return 0;
      }
    }
  }

  /* argument reduction */
  if (hx > 0x3fd62e42) {
    /* if |x| > 0.5 ln2 */
    if (hx >= 0x3ff0a2b2) {
      /* if |x| >= 1.5 ln2 */
      k = (EXP_INVLN2 * xr + (sign === 0 ? 0.5 : -0.5)) | 0;
    } else {
      k = 1 - sign - sign;
    }
    hi = xr - k * EXP_LN2HI; /* k*ln2hi is exact here */
    lo = k * EXP_LN2LO;
    xr = hi - lo;
  } else if (hx > 0x3e300000) {
    /* if |x| > 2**-28 */
    k = 0;
    hi = xr;
    lo = 0;
  } else {
    /* inexact if x!=0 */
    return 1 + xr;
  }

  /* x is now in primary range */
  const xx = xr * xr;
  const c = xr - xx * (EXP_P1 + xx * (EXP_P2 + xx * (EXP_P3 + xx * (EXP_P4 + xx * EXP_P5))));
  const y = 1 + ((xr * c) / (2 - c) - lo + hi);
  return k === 0 ? y : scalbn(y, k);
}

// `src/math/log.rs` (FreeBSD's e_log.c) and `src/math/log1p.rs` (s_log1p.c).
const LOG_LN2_HI = 6.9314718036912381649e-1; /* 3fe62e42 fee00000 */
const LOG_LN2_LO = 1.90821492927058770002e-10; /* 3dea39ef 35793c76 */
const LOG_LG1 = 6.66666666666673513e-1; /* 3FE55555 55555593 */
const LOG_LG2 = 3.999999999940941908e-1; /* 3FD99999 9997FA04 */
const LOG_LG3 = 2.857142874366239149e-1; /* 3FD24924 94229359 */
const LOG_LG4 = 2.222219843214978396e-1; /* 3FCC71C5 1D8E78AF */
const LOG_LG5 = 1.818357216161805012e-1; /* 3FC74664 96CB03DE */
const LOG_LG6 = 1.531383769920937332e-1; /* 3FC39A09 D078C69F */
const LOG_LG7 = 1.479819860511658591e-1; /* 3FC2F112 DF3E5244 */

/** 2⁵⁴, `log`'s scaling of a subnormal argument. */
const X1P54 = fromWords(0x43500000, 0);

/**
 * The natural logarithm of `x`, bit for bit the sim's `hyperion_base::math::ln`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/log.rs` (musl's form of FreeBSD's `e_log.c`):
 * `x` = 2ᵏ (1 + f) with 1 + f in [√2 ÷ 2, √2], and log(1 + f) from s = f ÷ (2 + f) by a degree-14
 * Remez polynomial in s. Under 1 ulp.
 */
export function log(x: number): number {
  let xr = x;
  let hx = highWord(xr);
  let lx = lowWord(xr);
  let k = 0;

  if (hx < 0x00100000 || hx >>> 31 !== 0) {
    /* x < 2**-126  */
    if ((hx & 0x7fffffff) === 0 && lx === 0) {
      return -1 / (xr * xr); /* log(+-0)=-inf */
    }
    if (hx >>> 31 !== 0) {
      return (xr - xr) / 0; /* log(-#) = NaN */
    }
    /* subnormal number, scale x up */
    k -= 54;
    xr *= X1P54;
    hx = highWord(xr);
    lx = lowWord(xr);
  } else if (hx >= 0x7ff00000) {
    return xr;
  } else if (hx === 0x3ff00000 && lx === 0) {
    return 0;
  }

  /* reduce x into [sqrt(2)/2, sqrt(2)] */
  hx += 0x3ff00000 - 0x3fe6a09e;
  k += (hx >>> 20) - 0x3ff;
  hx = (hx & 0x000fffff) + 0x3fe6a09e;
  xr = fromWords(hx, lx);

  const f = xr - 1;
  const hfsq = 0.5 * f * f;
  const s = f / (2 + f);
  const z = s * s;
  const w = z * z;
  const t1 = w * (LOG_LG2 + w * (LOG_LG4 + w * LOG_LG6));
  const t2 = z * (LOG_LG1 + w * (LOG_LG3 + w * (LOG_LG5 + w * LOG_LG7)));
  const r = t2 + t1;
  const dk = k;
  return s * (hfsq + r) + dk * LOG_LN2_LO - hfsq + f + dk * LOG_LN2_HI;
}

/**
 * The natural logarithm of 1 + `x`, accurate where `x` is small, bit for bit the sim's
 * `hyperion_base::math::ln_1p`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/log1p.rs` (FreeBSD's `s_log1p.c`): `log`'s
 * reduction and polynomial on u = 1 + `x`, with a correction c ≈ log(1 + x) − log(u) for the
 * rounding of u. Under 1 ulp.
 */
export function log1p(x: number): number {
  let f = 0;
  let c = 0;
  let k = 1;
  const hx = highWord(x);

  if (hx < 0x3fda827a || hx >>> 31 > 0) {
    /* 1+x < sqrt(2)+ */
    if (hx >= 0xbff00000) {
      /* x <= -1.0 */
      if (x === -1) {
        return x / 0; /* log1p(-1) = -inf */
      }
      return (x - x) / 0; /* log1p(x<-1) = NaN */
    }
    if ((hx << 1) >>> 0 < (0x3ca00000 << 1) >>> 0) {
      /* |x| < 2**-53 */
      return x;
    }
    if (hx <= 0xbfd2bec4) {
      /* sqrt(2)/2- <= 1+x < sqrt(2)+ */
      k = 0;
      c = 0;
      f = x;
    }
  } else if (hx >= 0x7ff00000) {
    return x;
  }
  if (k > 0) {
    const u = 1 + x;
    let hu = highWord(u);
    hu += 0x3ff00000 - 0x3fe6a09e;
    k = (hu >>> 20) - 0x3ff;
    /* correction term ~ log(1+x)-log(u), avoid underflow in c/u */
    if (k < 54) {
      c = k >= 2 ? 1 - (u - x) : x - (u - 1);
      c /= u;
    } else {
      c = 0;
    }
    /* reduce u into [sqrt(2)/2, sqrt(2)] */
    hu = (hu & 0x000fffff) + 0x3fe6a09e;
    f = fromWords(hu, lowWord(u)) - 1;
  }
  const hfsq = 0.5 * f * f;
  const s = f / (2 + f);
  const z = s * s;
  const w = z * z;
  const t1 = w * (LOG_LG2 + w * (LOG_LG4 + w * LOG_LG6));
  const t2 = z * (LOG_LG1 + w * (LOG_LG3 + w * (LOG_LG5 + w * LOG_LG7)));
  const r = t2 + t1;
  const dk = k;
  return s * (hfsq + r) + (dk * LOG_LN2_LO + c) - hfsq + f + dk * LOG_LN2_HI;
}

// `src/math/log2.rs` (FreeBSD's e_log2.c).
const LOG2_IVLN2HI = 1.44269504072144627571; /* 0x3ff71547, 0x65200000 */
const LOG2_IVLN2LO = 1.67517131648865118353e-10; /* 0x3de705fc, 0x2eefa200 */

/**
 * The base-2 logarithm of `x`, bit for bit the sim's `hyperion_base::math::log2`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/log2.rs` (FreeBSD's `e_log2.c`): {@link log}'s
 * reduction and polynomial, with log(1 + f) split into a 32-bit head and a tail before each is
 * scaled by 1 ÷ ln 2 in two pieces and the exponent added in extra precision. Under 1 ulp.
 */
export function log2(x: number): number {
  let xr = x;
  let hx = highWord(xr);
  let lx = lowWord(xr);
  let k = 0;
  if (hx < 0x00100000 || hx >>> 31 > 0) {
    if ((hx & 0x7fffffff) === 0 && lx === 0) {
      return -1 / (xr * xr); /* log(+-0)=-inf */
    }
    if (hx >>> 31 > 0) {
      return (xr - xr) / 0; /* log(-#) = NaN */
    }
    /* subnormal number, scale x up */
    k -= 54;
    xr *= X1P54;
    hx = highWord(xr);
    lx = lowWord(xr);
  } else if (hx >= 0x7ff00000) {
    return xr;
  } else if (hx === 0x3ff00000 && lx === 0) {
    return 0;
  }

  /* reduce x into [sqrt(2)/2, sqrt(2)] */
  hx += 0x3ff00000 - 0x3fe6a09e;
  k += (hx >>> 20) - 0x3ff;
  hx = (hx & 0x000fffff) + 0x3fe6a09e;
  xr = fromWords(hx, lx);

  const f = xr - 1;
  const hfsq = 0.5 * f * f;
  const s = f / (2 + f);
  const z = s * s;
  let w = z * z;
  const t1 = w * (LOG_LG2 + w * (LOG_LG4 + w * LOG_LG6));
  const t2 = z * (LOG_LG1 + w * (LOG_LG3 + w * (LOG_LG5 + w * LOG_LG7)));
  const r = t2 + t1;

  /* hi+lo = f - hfsq + s*(hfsq+R) ~ log(1+f) */
  const hi = zeroLowWord(f - hfsq);
  const lo = f - hi - hfsq + s * (hfsq + r);

  let valHi = hi * LOG2_IVLN2HI;
  let valLo = (lo + hi) * LOG2_IVLN2LO + lo * LOG2_IVLN2HI;

  /* spadd(val_hi, val_lo, y), except for not using double_t: */
  const y = k;
  w = y + valHi;
  valLo += y - w + valHi;
  valHi = w;

  return valLo + valHi;
}

// `src/math/expm1.rs` (FreeBSD's s_expm1.c).
const EXPM1_O_THRESHOLD = 7.09782712893383973096e2; /* 0x40862E42, 0xFEFA39EF */
const EXPM1_LN2_HI = 6.9314718036912381649e-1; /* 0x3fe62e42, 0xfee00000 */
const EXPM1_LN2_LO = 1.90821492927058770002e-10; /* 0x3dea39ef, 0x35793c76 */
// expm1.rs's literal for 1 ÷ ln 2, kept as written (the double Math.LOG2E is).
// oxlint-disable-next-line approx-constant
const EXPM1_INVLN2 = 1.442695040888963387; /* 0x3ff71547, 0x652b82fe */
/* Scaled Q's: Qn_here = 2**n * Qn_above, for R(2*z) where z = hxs = x*x/2: */
const EXPM1_Q1 = -3.33333333333331316428e-2; /* BFA11111 111110F4 */
const EXPM1_Q2 = 1.58730158725481460165e-3; /* 3F5A01A0 19FE5585 */
const EXPM1_Q3 = -7.93650757867487942473e-5; /* BF14CE19 9EAADBB7 */
const EXPM1_Q4 = 4.00821782732936239552e-6; /* 3ED0CFCA 86E65239 */
const EXPM1_Q5 = -2.01099218183624371326e-7; /* BE8AFDB7 6E09C32D */

/**
 * e raised to `x`, less 1, accurate where `x` is small, bit for bit the sim's
 * `hyperion_base::math::exp_m1`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/expm1.rs` (FreeBSD's `s_expm1.c`): `exp`'s
 * reduction with its rounding error c carried, a rational fit of e^r − 1, and the scaling by 2ᵏ
 * arranged so that nothing cancels. Under 1 ulp.
 */
export function expm1(x: number): number {
  let xr = x;
  let hi: number;
  let lo: number;
  let k: number;
  let c: number;
  let t: number;
  let y: number;

  const hw = highWord(xr);
  const hx = hw & 0x7fffffff;
  const sign = hw >>> 31;

  /* filter out huge and non-finite argument */
  if (hx >= 0x4043687a) {
    /* if |x|>=56*ln2 */
    if (Number.isNaN(xr)) {
      return xr;
    }
    if (sign !== 0) {
      return -1;
    }
    if (xr > EXPM1_O_THRESHOLD) {
      xr *= X1P1023;
      return xr;
    }
  }

  /* argument reduction */
  if (hx > 0x3fd62e42) {
    /* if  |x| > 0.5 ln2 */
    if (hx < 0x3ff0a2b2) {
      /* and |x| < 1.5 ln2 */
      if (sign === 0) {
        hi = xr - EXPM1_LN2_HI;
        lo = EXPM1_LN2_LO;
        k = 1;
      } else {
        hi = xr + EXPM1_LN2_HI;
        lo = -EXPM1_LN2_LO;
        k = -1;
      }
    } else {
      k = (EXPM1_INVLN2 * xr + (sign !== 0 ? -0.5 : 0.5)) | 0;
      t = k;
      hi = xr - t * EXPM1_LN2_HI; /* t*ln2_hi is exact here */
      lo = t * EXPM1_LN2_LO;
    }
    xr = hi - lo;
    c = hi - xr - lo;
  } else if (hx < 0x3c900000) {
    /* |x| < 2**-54, return x */
    return xr;
  } else {
    c = 0;
    k = 0;
  }

  /* x is now in primary range */
  const hfx = 0.5 * xr;
  const hxs = xr * hfx;
  const r1 =
    1 + hxs * (EXPM1_Q1 + hxs * (EXPM1_Q2 + hxs * (EXPM1_Q3 + hxs * (EXPM1_Q4 + hxs * EXPM1_Q5))));
  t = 3 - r1 * hfx;
  let e = hxs * ((r1 - t) / (6 - xr * t));
  if (k === 0) {
    /* c is 0 */
    return xr - (xr * e - hxs);
  }
  e = xr * (e - c) - c;
  e -= hxs;
  /* exp(x) ~ 2^k (x_reduced - e + 1) */
  if (k === -1) {
    return 0.5 * (xr - e) - 0.5;
  }
  if (k === 1) {
    if (xr < -0.25) {
      return -2 * (e - (xr + 0.5));
    }
    return 1 + 2 * (xr - e);
  }
  const twopk = fromWords((0x3ff + k) * 0x100000, 0); /* 2^k */
  if (k < 0 || k > 56) {
    /* suffice to return exp(x)-1 */
    y = xr - e + 1;
    if (k === 1024) {
      y = y * 2 * X1P1023;
    } else {
      y = y * twopk;
    }
    return y - 1;
  }
  const uf = fromWords((0x3ff - k) * 0x100000, 0); /* 2^-k */
  if (k < 20) {
    y = (xr - e + (1 - uf)) * twopk;
  } else {
    y = (xr - (e + uf) + 1) * twopk;
  }
  return y;
}

// `src/math/k_sin.rs` (FreeBSD's k_sin.c).
const SIN_S1 = -1.66666666666666324348e-1; /* 0xBFC55555, 0x55555549 */
const SIN_S2 = 8.33333333332248946124e-3; /* 0x3F811111, 0x1110F8A6 */
const SIN_S3 = -1.98412698298579493134e-4; /* 0xBF2A01A0, 0x19C161D5 */
const SIN_S4 = 2.75573137070700676789e-6; /* 0x3EC71DE3, 0x57B1FE7D */
const SIN_S5 = -2.50507602534068634195e-8; /* 0xBE5AE5E6, 0x8A2B9CEB */
const SIN_S6 = 1.58969099521155010221e-10; /* 0x3DE5D93A, 0x5ACFD57C */

/** sin(x + y) on [−π/4, π/4], y the tail of x, or 0 where `iy` is 0: `libm`'s `k_sin`. */
function kSin(x: number, y: number, iy: number): number {
  const z = x * x;
  const w = z * z;
  const r = SIN_S2 + z * (SIN_S3 + z * SIN_S4) + z * w * (SIN_S5 + z * SIN_S6);
  const v = z * x;
  if (iy === 0) {
    return x + v * (SIN_S1 + z * r);
  }
  return x - (z * (0.5 * y - v * r) - y - v * SIN_S1);
}

// `src/math/k_cos.rs` (FreeBSD's k_cos.c).
const COS_C1 = 4.16666666666666019037e-2; /* 0x3FA55555, 0x5555554C */
const COS_C2 = -1.38888888888741095749e-3; /* 0xBF56C16C, 0x16C15177 */
const COS_C3 = 2.48015872894767294178e-5; /* 0x3EFA01A0, 0x19CB1590 */
const COS_C4 = -2.75573143513906633035e-7; /* 0xBE927E4F, 0x809C52AD */
const COS_C5 = 2.0875723212981748279e-9; /* 0x3E21EE9E, 0xBDB4B1C4 */
const COS_C6 = -1.13596475577881948265e-11; /* 0xBDA8FAE9, 0xBE8838D4 */

/** cos(x + y) on [−π/4, π/4], y the tail of x: `libm`'s `k_cos`. */
function kCos(x: number, y: number): number {
  const z = x * x;
  const w = z * z;
  const r = z * (COS_C1 + z * (COS_C2 + z * COS_C3)) + w * w * (COS_C4 + z * (COS_C5 + z * COS_C6));
  const hz = 0.5 * z;
  const w1 = 1 - hz;
  return w1 + (1 - w1 - hz + (z * r - x * y));
}

// `src/math/k_tan.rs` (FreeBSD's k_tan.c).
const TAN_T = [
  3.33333333333334091986e-1 /* 3FD55555, 55555563 */,
  1.33333333333201242699e-1 /* 3FC11111, 1110FE7A */,
  5.39682539762260521377e-2 /* 3FABA1BA, 1BB341FE */,
  2.18694882948595424599e-2 /* 3F9664F4, 8406D637 */,
  8.86323982359930005737e-3 /* 3F8226E3, E96E8493 */,
  3.59207910759131235356e-3 /* 3F6D6D22, C9560328 */,
  1.45620945432529025516e-3 /* 3F57DBC8, FEE08315 */,
  5.88041240820264096874e-4 /* 3F4344D8, F2F26501 */,
  2.46463134818469906812e-4 /* 3F3026F7, 1A8D1068 */,
  7.817944429395570923e-5 /* 3F147E88, A03792A6 */,
  7.14072491382608190305e-5 /* 3F12B80F, 32F0A7E9 */,
  -1.85586374855275456654e-5 /* BEF375CB, DB605373 */,
  2.59073051863633712884e-5 /* 3EFB2A70, 74BF7AD4 */,
] as const;
const TAN_PIO4 = 7.85398163397448278999e-1; /* 3FE921FB, 54442D18 */
const TAN_PIO4_LO = 3.06161699786838301793e-17; /* 3C81A626, 33145C07 */

/** `x` with its low word cleared: `k_tan.rs`'s `zero_low_word`. */
function zeroLowWord(x: number): number {
  return fromWords(highWord(x), 0);
}

/**
 * tan(x + y), or −1 ÷ tan(x + y) where `odd` is 1, on [−π/4, π/4], y the tail of x: `libm`'s
 * `k_tan`.
 */
function kTan(x0: number, y0: number, odd: number): number {
  let x = x0;
  let y = y0;
  const hx = highWord(x);
  const big = (hx & 0x7fffffff) >= 0x3fe59428; /* |x| >= 0.6744 */
  if (big) {
    const sign = hx >>> 31;
    if (sign !== 0) {
      x = -x;
      y = -y;
    }
    x = TAN_PIO4 - x + (TAN_PIO4_LO - y);
    y = 0;
  }
  const z = x * x;
  const w = z * z;
  /*
   * Break x^5*(T[1]+x^2*T[2]+...) into
   * x^5(T[1]+x^4*T[3]+...+x^20*T[11]) +
   * x^5(x^2*(T[2]+x^4*T[4]+...+x^22*[T12]))
   */
  const r0 =
    TAN_T[1] + w * (TAN_T[3] + w * (TAN_T[5] + w * (TAN_T[7] + w * (TAN_T[9] + w * TAN_T[11]))));
  const v0 =
    z *
    (TAN_T[2] + w * (TAN_T[4] + w * (TAN_T[6] + w * (TAN_T[8] + w * (TAN_T[10] + w * TAN_T[12])))));
  const s0 = z * x;
  const r = y + z * (s0 * (r0 + v0) + y) + s0 * TAN_T[0];
  const w1 = x + r;
  if (big) {
    const sign = hx >>> 31;
    const s = 1 - 2 * odd;
    const v = s - 2 * (x + (r - (w1 * w1) / (w1 + s)));
    return sign !== 0 ? -v : v;
  }
  if (odd === 0) {
    return w1;
  }
  /* -1.0/(x+r) has up to 2ulp error, so compute it accurately */
  const w0 = zeroLowWord(w1);
  const v = r - (w0 - x); /* w0+v = r+x */
  const a = -1 / w1;
  const a0 = zeroLowWord(a);
  return a0 + a * (1 + a0 * w0 + a0 * v);
}

// `src/math/rem_pio2.rs` (FreeBSD's e_rem_pio2.c, optimized by Bruce D. Evans).
const REM_EPS = 2.2204460492503131e-16;
const REM_TO_INT = 1.5 / REM_EPS;
/** 53 bits of 2/pi */
const REM_INV_PIO2 = 6.36619772367581382433e-1; /* 0x3FE45F30, 0x6DC9C883 */
/** first 33 bits of pi/2 */
const REM_PIO2_1 = 1.57079632673412561417; /* 0x3FF921FB, 0x54400000 */
/** pi/2 - PIO2_1 */
const REM_PIO2_1T = 6.07710050650619224932e-11; /* 0x3DD0B461, 0x1A626331 */
/** second 33 bits of pi/2 */
const REM_PIO2_2 = 6.0771005063039659766e-11; /* 0x3DD0B461, 0x1A600000 */
/** pi/2 - (PIO2_1+PIO2_2) */
const REM_PIO2_2T = 2.02226624879595063154e-21; /* 0x3BA3198A, 0x2E037073 */
/** third 33 bits of pi/2 */
const REM_PIO2_3 = 2.0222662487111664558e-21; /* 0x3BA3198A, 0x2E000000 */
/** pi/2 - (PIO2_1+PIO2_2+PIO2_3) */
const REM_PIO2_3T = 8.47842766036889956997e-32; /* 0x397B839A, 0x252049C1 */

/** 2²⁴. */
const X1P24 = fromWords(0x41700000, 0);
/** 2⁻²⁴. */
const X1P_24 = fromWords(0x3e700000, 0);

/**
 * The remainder's head and tail from {@link remPio2}: x − n π/2 = `remHead` + `remTail`. Module
 * state in place of the Rust tuple, so that the circular functions allocate nothing; JavaScript
 * runs one call at a time, and each caller reads them at once.
 */
let remHead = 0;
let remTail = 0;

/**
 * `rem_pio2`'s `medium` case: n = rint(x ÷ (π/2)) for |x| ≲ 2²⁰ π/2, and x − n π/2 in `remHead` and
 * `remTail`, in up to three rounds of π/2's 33-bit pieces.
 */
function remPio2Medium(x: number, ix: number): number {
  /* rint(x/(pi/2)), Assume round-to-nearest. */
  const tmp = x * REM_INV_PIO2 + REM_TO_INT;
  const fN = tmp - REM_TO_INT;
  const n = fN | 0;
  let r = x - fN * REM_PIO2_1;
  let w = fN * REM_PIO2_1T; /* 1st round, good to 85 bits */
  let y0 = r - w;
  const ey = (highWord(y0) >>> 20) & 0x7ff;
  const ex = ix >>> 20;
  if (ex - ey > 16) {
    /* 2nd round, good to 118 bits */
    let t = r;
    w = fN * REM_PIO2_2;
    r = t - w;
    w = fN * REM_PIO2_2T - (t - r - w);
    y0 = r - w;
    const ey2 = (highWord(y0) >>> 20) & 0x7ff;
    if (ex - ey2 > 49) {
      /* 3rd round, good to 151 bits, covers all cases */
      t = r;
      w = fN * REM_PIO2_3;
      r = t - w;
      w = fN * REM_PIO2_3T - (t - r - w);
      y0 = r - w;
    }
  }
  remHead = y0;
  remTail = r - y0 - w;
  return n;
}

/**
 * n, with x − n π/2 in `remHead` and `remTail`, for |x| ≳ π/4 (the caller handles the rest):
 * `libm`'s `rem_pio2`.
 */
function remPio2(x: number): number {
  const sign = highWord(x) >>> 31;
  const ix = highWord(x) & 0x7fffffff;

  if (ix <= 0x400f6a7a) {
    /* |x| ~<= 5pi/4 */
    if ((ix & 0xfffff) === 0x921fb) {
      /* |x| ~= pi/2 or 2pi/2 */
      return remPio2Medium(x, ix); /* cancellation -- use medium case */
    }
    if (ix <= 0x4002d97c) {
      /* |x| ~<= 3pi/4 */
      if (sign === 0) {
        const z = x - REM_PIO2_1; /* one round good to 85 bits */
        remHead = z - REM_PIO2_1T;
        remTail = z - remHead - REM_PIO2_1T;
        return 1;
      }
      const z = x + REM_PIO2_1;
      remHead = z + REM_PIO2_1T;
      remTail = z - remHead + REM_PIO2_1T;
      return -1;
    } else if (sign === 0) {
      const z = x - 2 * REM_PIO2_1;
      remHead = z - 2 * REM_PIO2_1T;
      remTail = z - remHead - 2 * REM_PIO2_1T;
      return 2;
    }
    const z = x + 2 * REM_PIO2_1;
    remHead = z + 2 * REM_PIO2_1T;
    remTail = z - remHead + 2 * REM_PIO2_1T;
    return -2;
  }
  if (ix <= 0x401c463b) {
    /* |x| ~<= 9pi/4 */
    if (ix <= 0x4015fdbc) {
      /* |x| ~<= 7pi/4 */
      if (ix === 0x4012d97c) {
        /* |x| ~= 3pi/2 */
        return remPio2Medium(x, ix);
      }
      if (sign === 0) {
        const z = x - 3 * REM_PIO2_1;
        remHead = z - 3 * REM_PIO2_1T;
        remTail = z - remHead - 3 * REM_PIO2_1T;
        return 3;
      }
      const z = x + 3 * REM_PIO2_1;
      remHead = z + 3 * REM_PIO2_1T;
      remTail = z - remHead + 3 * REM_PIO2_1T;
      return -3;
    }
    if (ix === 0x401921fb) {
      /* |x| ~= 4pi/2 */
      return remPio2Medium(x, ix);
    }
    if (sign === 0) {
      const z = x - 4 * REM_PIO2_1;
      remHead = z - 4 * REM_PIO2_1T;
      remTail = z - remHead - 4 * REM_PIO2_1T;
      return 4;
    }
    const z = x + 4 * REM_PIO2_1;
    remHead = z + 4 * REM_PIO2_1T;
    remTail = z - remHead + 4 * REM_PIO2_1T;
    return -4;
  }
  if (ix < 0x413921fb) {
    /* |x| ~< 2^20*(pi/2), medium size */
    return remPio2Medium(x, ix);
  }
  /*
   * all other (large) arguments
   */
  if (ix >= 0x7ff00000) {
    /* x is inf or NaN */
    remHead = x - x;
    remTail = remHead;
    return 0;
  }
  /* set z = scalbn(|x|,-ilogb(x)+23) */
  let z = fromWords((highWord(x) & 0x000fffff) | ((0x3ff + 23) * 0x100000), lowWord(x));
  const tx = [0, 0, 0];
  for (let i = 0; i < 2; i += 1) {
    const whole = z | 0;
    tx[i] = whole;
    z = (z - whole) * X1P24;
  }
  tx[2] = z;
  /* skip zero terms, first term is non-zero */
  let i = 2;
  while (i !== 0 && tx[i] === 0) {
    i -= 1;
  }
  const n = remPio2Large(tx.slice(0, i + 1), (ix >>> 20) - (0x3ff + 23));
  if (sign !== 0) {
    remHead = -remHead;
    remTail = -remTail;
    return -n;
  }
  return n;
}

// `src/math/rem_pio2_large.rs` (FreeBSD's k_rem_pio2.c).
/**
 * 2 ÷ π in 24-bit pieces: the 66 entries of `libm`'s table for 32-bit targets, which are all an
 * `f64` argument reads (its 64-bit table's 690 serve wider formats; the sim's WebAssembly build
 * reads these 66 and gives the native build's bits).
 */
const IPIO2 = [
  0xa2f983, 0x6e4e44, 0x1529fc, 0x2757d1, 0xf534dd, 0xc0db62, 0x95993c, 0x439041, 0xfe5163,
  0xabdebb, 0xc561b7, 0x246e3a, 0x424dd2, 0xe00649, 0x2eea09, 0xd1921c, 0xfe1deb, 0x1cb129,
  0xa73ee8, 0x8235f5, 0x2ebb44, 0x84e99c, 0x7026b4, 0x5f7e41, 0x3991d6, 0x398353, 0x39f49c,
  0x845f8b, 0xbdf928, 0x3b1ff8, 0x97ffde, 0x05980f, 0xef2f11, 0x8b5a0a, 0x6d1f6d, 0x367ecf,
  0x27cb09, 0xb74f46, 0x3f669e, 0x5fea2d, 0x7527ba, 0xc7ebe5, 0xf17b3d, 0x0739f7, 0x8a5292,
  0xea6bfb, 0x5fb11f, 0x8d5d08, 0x560330, 0x46fc7b, 0x6babf0, 0xcfbc20, 0x9af436, 0x1da9e3,
  0x91615e, 0xe61b08, 0x659985, 0x5f14a0, 0x68408d, 0xffd880, 0x4d7327, 0x310606, 0x1556ca,
  0x73a8c9, 0x60e27b, 0xc08c6b,
] as const;

/** π ÷ 2 in pieces of 24 bits. */
const PIO2 = [
  1.57079625129699707031 /* 0x3FF921FB, 0x40000000 */,
  7.54978941586159635335e-8 /* 0x3E74442D, 0x00000000 */,
  5.39030252995776476554e-15 /* 0x3CF84698, 0x80000000 */,
  3.28200341580791294123e-22 /* 0x3B78CC51, 0x60000000 */,
  1.27065575308067607349e-29 /* 0x39F01B83, 0x80000000 */,
  1.22933308981111328932e-36 /* 0x387A2520, 0x40000000 */,
  2.73370053816464559624e-44 /* 0x36E38222, 0x80000000 */,
  2.16741683877804819444e-51 /* 0x3569F31D, 0x00000000 */,
] as const;

/** The `index`th entry of `values`, which a caller's bounds guarantee. */
function at(values: ArrayLike<number>, index: number): number {
  const value = values[index];
  if (value === undefined) {
    throw new Error(`rem_pio2_large reads entry ${String(index)} of ${String(values.length)}`);
  }
  return value;
}

/**
 * n mod 8, with x − n π/2 in `remHead` and `remTail`, for x = Σ `x[i]` 2^(`e0` − 24 i), at
 * `libm`'s `rem_pio2_large` precision 1 (53 bits), the one `rem_pio2` asks for.
 */
function remPio2Large(x: ReadonlyArray<number>, e0: number): number {
  const f = new Float64Array(20);
  const fq = new Float64Array(20);
  const q = new Float64Array(20);
  const iq = new Int32Array(20);
  let fw: number;
  let n: number;
  let ih: number;
  let z: number;

  /* initialize jk*/
  const jk = 4; /* INIT_JK[prec] for prec 1 */
  const jp = jk;

  /* determine jx,jv,q0, note that 3>q0 */
  const jx = x.length - 1;
  let jv = ((e0 - 3) / 24) | 0;
  if (jv < 0) {
    jv = 0;
  }
  let q0 = e0 - 24 * (jv + 1);

  /* set up f[0] to f[jx+jk] where f[jx+jk] = ipio2[jv+jk] */
  let j = jv - jx;
  const m = jx + jk;
  for (let i = 0; i <= m; i += 1) {
    f[i] = j < 0 ? 0 : at(IPIO2, j);
    j += 1;
  }

  /* compute q[0],q[1],...q[jk] */
  for (let i = 0; i <= jk; i += 1) {
    fw = 0;
    for (let jj = 0; jj <= jx; jj += 1) {
      fw += at(x, jj) * at(f, jx + i - jj);
    }
    q[i] = fw;
  }

  let jz = jk;

  for (;;) {
    /* distill q[] into iq[] reversingly */
    let i = 0;
    z = at(q, jz);
    for (let jj = jz; jj >= 1; jj -= 1) {
      fw = (X1P_24 * z) | 0;
      iq[i] = (z - X1P24 * fw) | 0;
      z = at(q, jj - 1) + fw;
      i += 1;
    }

    /* compute n */
    z = scalbn(z, q0); /* actual value of z */
    z -= 8 * Math.floor(z * 0.125); /* trim off integer >= 8 */
    n = z | 0;
    z -= n;
    ih = 0;
    if (q0 > 0) {
      /* need iq[jz-1] to determine n */
      i = at(iq, jz - 1) >> (24 - q0);
      n += i;
      iq[jz - 1] = at(iq, jz - 1) - (i << (24 - q0));
      ih = at(iq, jz - 1) >> (23 - q0);
    } else if (q0 === 0) {
      ih = at(iq, jz - 1) >> 23;
    } else if (z >= 0.5) {
      ih = 2;
    }

    if (ih > 0) {
      /* q > 0.5 */
      n += 1;
      let carry = 0;
      for (let ii = 0; ii < jz; ii += 1) {
        /* compute 1-q */
        const jj = at(iq, ii);
        if (carry === 0) {
          if (jj !== 0) {
            carry = 1;
            iq[ii] = 0x1000000 - jj;
          }
        } else {
          iq[ii] = 0xffffff - jj;
        }
      }
      if (q0 > 0) {
        /* rare case: chance is 1 in 12 */
        if (q0 === 1) {
          iq[jz - 1] = at(iq, jz - 1) & 0x7fffff;
        } else if (q0 === 2) {
          iq[jz - 1] = at(iq, jz - 1) & 0x3fffff;
        }
      }
      if (ih === 2) {
        z = 1 - z;
        if (carry !== 0) {
          z -= scalbn(1, q0);
        }
      }
    }

    /* check if recomputation is needed */
    if (z === 0) {
      let jj = 0;
      for (let ii = jz - 1; ii >= jk; ii -= 1) {
        jj |= at(iq, ii);
      }
      if (jj === 0) {
        /* need recomputation */
        let k = 1;
        while (at(iq, jk - k) === 0) {
          k += 1; /* k = no. of terms needed */
        }

        for (let ii = jz + 1; ii <= jz + k; ii += 1) {
          /* add q[jz+1] to q[jz+k] */
          f[jx + ii] = at(IPIO2, jv + ii);
          fw = 0;
          for (let jjj = 0; jjj <= jx; jjj += 1) {
            fw += at(x, jjj) * at(f, jx + ii - jjj);
          }
          q[ii] = fw;
        }
        jz += k;
        continue;
      }
    }

    break;
  }

  /* chop off zero terms */
  if (z === 0) {
    jz -= 1;
    q0 -= 24;
    while (at(iq, jz) === 0) {
      jz -= 1;
      q0 -= 24;
    }
  } else {
    /* break z into 24-bit if necessary */
    z = scalbn(z, -q0);
    if (z >= X1P24) {
      fw = (X1P_24 * z) | 0;
      iq[jz] = (z - X1P24 * fw) | 0;
      jz += 1;
      q0 += 24;
      iq[jz] = fw | 0;
    } else {
      iq[jz] = z | 0;
    }
  }

  /* convert integer "bit" chunk to floating-point value */
  fw = scalbn(1, q0);
  for (let i = jz; i >= 0; i -= 1) {
    q[i] = fw * at(iq, i);
    fw *= X1P_24;
  }

  /* compute PIo2[0,...,jp]*q[jz,...,0] */
  for (let i = jz; i >= 0; i -= 1) {
    fw = 0;
    let k = 0;
    while (k <= jp && k <= jz - i) {
      fw += at(PIO2, k) * at(q, i + k);
      k += 1;
    }
    fq[jz - i] = fw;
  }

  /* compress fq[] into y[] */
  fw = 0;
  for (let i = jz; i >= 0; i -= 1) {
    fw += at(fq, i);
  }
  remHead = ih === 0 ? fw : -fw;
  fw = at(fq, 0) - fw;
  for (let i = 1; i <= jz; i += 1) {
    fw += at(fq, i);
  }
  remTail = ih === 0 ? fw : -fw;
  return n & 7;
}

/**
 * The sine of `x` radians, bit for bit the sim's `hyperion_base::math::sin`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/sin.rs` (FreeBSD's `s_sin.c`): `x` is reduced
 * by π/2 to [−π/4, π/4] (`rem_pio2`, Payne and Hanek's method past 2²⁰ π/2), and the quadrant's
 * kernel, `k_sin` or `k_cos`, evaluates it. Under 1 ulp.
 */
export function sin(x: number): number {
  /* High word of x. */
  const ix = highWord(x) & 0x7fffffff;

  /* |x| ~< pi/4 */
  if (ix <= 0x3fe921fb) {
    if (ix < 0x3e500000) {
      /* |x| < 2**-26 */
      /* raise inexact if x != 0 and underflow if subnormal*/
      return x;
    }
    return kSin(x, 0, 0);
  }

  /* sin(Inf or NaN) is NaN */
  if (ix >= 0x7ff00000) {
    return x - x;
  }

  /* argument reduction needed */
  const n = remPio2(x);
  switch (n & 3) {
    case 0:
      return kSin(remHead, remTail, 1);
    case 1:
      return kCos(remHead, remTail);
    case 2:
      return -kSin(remHead, remTail, 1);
    default:
      return -kCos(remHead, remTail);
  }
}

/**
 * The cosine of `x` radians, bit for bit the sim's `hyperion_base::math::cos`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/cos.rs` (FreeBSD's `s_cos.c`), with
 * {@link sin}'s reduction and kernels. Under 1 ulp.
 */
export function cos(x: number): number {
  const ix = highWord(x) & 0x7fffffff;

  /* |x| ~< pi/4 */
  if (ix <= 0x3fe921fb) {
    if (ix < 0x3e46a09e) {
      /* if x < 2**-27 * sqrt(2) */
      /* raise inexact if x != 0 */
      if ((x | 0) === 0) {
        return 1;
      }
    }
    return kCos(x, 0);
  }

  /* cos(Inf or NaN) is NaN */
  if (ix >= 0x7ff00000) {
    return x - x;
  }

  /* argument reduction needed */
  const n = remPio2(x);
  switch (n & 3) {
    case 0:
      return kCos(remHead, remTail);
    case 1:
      return -kSin(remHead, remTail, 1);
    case 2:
      return -kCos(remHead, remTail);
    default:
      return kSin(remHead, remTail, 1);
  }
}

/**
 * The tangent of `x` radians, bit for bit the sim's `hyperion_base::math::tan`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/tan.rs` (FreeBSD's `s_tan.c`), with
 * {@link sin}'s reduction and the kernel `k_tan`. Under 1 ulp.
 */
export function tan(x: number): number {
  const ix = highWord(x) & 0x7fffffff;
  /* |x| ~< pi/4 */
  if (ix <= 0x3fe921fb) {
    if (ix < 0x3e400000) {
      /* |x| < 2**-27 */
      /* raise inexact if x!=0 and underflow if subnormal */
      return x;
    }
    return kTan(x, 0, 0);
  }

  /* tan(Inf or NaN) is NaN */
  if (ix >= 0x7ff00000) {
    return x - x;
  }

  /* argument reduction */
  const n = remPio2(x);
  return kTan(remHead, remTail, n & 1);
}

// `src/math/asin.rs` (FreeBSD's e_asin.c) and `src/math/acos.rs` (e_acos.c).
const ASIN_PIO2_HI = 1.570796326794896558; /* 0x3FF921FB, 0x54442D18 */
const ASIN_PIO2_LO = 6.12323399573676603587e-17; /* 0x3C91A626, 0x33145C07 */
/* coefficients for R(x^2) */
const ASIN_P_S0 = 1.66666666666666657415e-1; /* 0x3FC55555, 0x55555555 */
const ASIN_P_S1 = -3.25565818622400915405e-1; /* 0xBFD4D612, 0x03EB6F7D */
const ASIN_P_S2 = 2.01212532134862925881e-1; /* 0x3FC9C155, 0x0E884455 */
const ASIN_P_S3 = -4.00555345006794114027e-2; /* 0xBFA48228, 0xB5688F3B */
const ASIN_P_S4 = 7.91534994289814532176e-4; /* 0x3F49EFE0, 0x7501B288 */
const ASIN_P_S5 = 3.4793310759602116757e-5; /* 0x3F023DE1, 0x0DFDF709 */
const ASIN_Q_S1 = -2.40339491173441421878; /* 0xC0033A27, 0x1C8A2D4B */
const ASIN_Q_S2 = 2.02094576023350569471; /* 0x40002AE5, 0x9C598AC8 */
const ASIN_Q_S3 = -6.8828397160545329303e-1; /* 0xBFE6066C, 0x1B8D0159 */
const ASIN_Q_S4 = 7.70381505559019352791e-2; /* 0x3FB3B8C5, 0xB12E9282 */

/** 2⁻¹²⁰, the inexact tail of `asin(±1)` and `acos`'s. */
const X1P_120 = fromWords(0x38700000, 0);

/** R(z) = (asin(√z) − √z) ÷ z^1.5 as a rational fit: `asin.rs`'s `comp_r` and `acos.rs`'s `r`. */
function asinR(z: number): number {
  const p =
    z *
    (ASIN_P_S0 +
      z * (ASIN_P_S1 + z * (ASIN_P_S2 + z * (ASIN_P_S3 + z * (ASIN_P_S4 + z * ASIN_P_S5)))));
  const q = 1 + z * (ASIN_Q_S1 + z * (ASIN_Q_S2 + z * (ASIN_Q_S3 + z * ASIN_Q_S4)));
  return p / q;
}

/**
 * The arcsine of `x`, in radians, bit for bit the sim's `hyperion_base::math::asin`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/asin.rs` (FreeBSD's `e_asin.c`): a rational
 * fit below |x| = ½, and above it asin(x) = π/2 − 2 asin(√((1 − |x|) ÷ 2)) with the square root
 * split in two above 0.975. NaN outside [−1, 1]. Under 1 ulp.
 */
export function asin(x: number): number {
  let xr = x;
  const hx = highWord(xr);
  const ix = hx & 0x7fffffff;
  /* |x| >= 1 or nan */
  if (ix >= 0x3ff00000) {
    const lx = lowWord(xr);
    if (((ix - 0x3ff00000) | lx) === 0) {
      /* asin(1) = +-pi/2 with inexact */
      return xr * ASIN_PIO2_HI + X1P_120;
    }
    // asin.rs's NaN outside [−1, 1]: 0 ÷ 0, or 0 ÷ NaN for a NaN argument.
    // oxlint-disable-next-line erasing-op
    return 0 / (xr - xr);
  }
  /* |x| < 0.5 */
  if (ix < 0x3fe00000) {
    /* if 0x1p-1022 <= |x| < 0x1p-26, avoid raising underflow */
    if (ix >= 0x00100000 && ix < 0x3e500000) {
      return xr;
    }
    return xr + xr * asinR(xr * xr);
  }
  /* 1 > |x| >= 0.5 */
  const z = (1 - Math.abs(xr)) * 0.5;
  const s = Math.sqrt(z);
  const r = asinR(z);
  if (ix >= 0x3fef3333) {
    /* if |x| > 0.975 */
    xr = ASIN_PIO2_HI - (2 * (s + s * r) - ASIN_PIO2_LO);
  } else {
    /* f+c = sqrt(z) */
    const f = withLowWord(s, 0);
    const c = (z - f * f) / (s + f);
    xr = 0.5 * ASIN_PIO2_HI - (2 * s * r - (ASIN_PIO2_LO - 2 * c) - (0.5 * ASIN_PIO2_HI - 2 * f));
  }
  return hx >>> 31 !== 0 ? -xr : xr;
}

/**
 * The arccosine of `x`, in radians, bit for bit the sim's `hyperion_base::math::acos`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/acos.rs` (FreeBSD's `e_acos.c`), with
 * {@link asin}'s rational fit: acos(x) = π/2 − (x + x R(x²)) below |x| = ½, and 2 asin(√((1 ∓ x) ÷
 * 2)) or its complement above. NaN outside [−1, 1]. Under 1 ulp.
 */
export function acos(x: number): number {
  const hx = highWord(x);
  const ix = hx & 0x7fffffff;
  /* |x| >= 1 or nan */
  if (ix >= 0x3ff00000) {
    const lx = lowWord(x);
    if (((ix - 0x3ff00000) | lx) === 0) {
      /* acos(1)=0, acos(-1)=pi */
      if (hx >>> 31 !== 0) {
        return 2 * ASIN_PIO2_HI + X1P_120;
      }
      return 0;
    }
    // acos.rs's NaN outside [−1, 1]: 0 ÷ 0, or 0 ÷ NaN for a NaN argument.
    // oxlint-disable-next-line erasing-op
    return 0 / (x - x);
  }
  /* |x| < 0.5 */
  if (ix < 0x3fe00000) {
    if (ix <= 0x3c600000) {
      /* |x| < 2**-57 */
      return ASIN_PIO2_HI + X1P_120;
    }
    return ASIN_PIO2_HI - (x - (ASIN_PIO2_LO - x * asinR(x * x)));
  }
  /* x < -0.5 */
  if (hx >>> 31 !== 0) {
    const z = (1 + x) * 0.5;
    const s = Math.sqrt(z);
    const w = asinR(z) * s - ASIN_PIO2_LO;
    return 2 * (ASIN_PIO2_HI - (s + w));
  }
  /* x > 0.5 */
  const z = (1 - x) * 0.5;
  const s = Math.sqrt(z);
  const df = withLowWord(s, 0);
  const c = (z - df * df) / (s + df);
  const w = asinR(z) * s + c;
  return 2 * (df + w);
}

// `src/math/atan.rs` (FreeBSD's s_atan.c).
const ATANHI = [
  4.63647609000806093515e-1 /* atan(0.5)hi 0x3FDDAC67, 0x0561BB4F */,
  7.85398163397448278999e-1 /* atan(1.0)hi 0x3FE921FB, 0x54442D18 */,
  9.82793723247329054082e-1 /* atan(1.5)hi 0x3FEF730B, 0xD281F69B */,
  1.570796326794896558 /* atan(inf)hi 0x3FF921FB, 0x54442D18 */,
] as const;
const ATANLO = [
  2.26987774529616870924e-17 /* atan(0.5)lo 0x3C7A2B7F, 0x222F65E2 */,
  3.06161699786838301793e-17 /* atan(1.0)lo 0x3C81A626, 0x33145C07 */,
  1.39033110312309984516e-17 /* atan(1.5)lo 0x3C700788, 0x7AF0CBBD */,
  6.12323399573676603587e-17 /* atan(inf)lo 0x3C91A626, 0x33145C07 */,
] as const;
const AT = [
  3.33333333333329318027e-1 /* 0x3FD55555, 0x5555550D */,
  -1.99999999998764832476e-1 /* 0xBFC99999, 0x9998EBC4 */,
  1.42857142725034663711e-1 /* 0x3FC24924, 0x920083FF */,
  -1.1111110405462355788e-1 /* 0xBFBC71C6, 0xFE231671 */,
  9.09088713343650656196e-2 /* 0x3FB745CD, 0xC54C206E */,
  -7.69187620504482999495e-2 /* 0xBFB3B0F2, 0xAF749A6D */,
  6.66107313738753120669e-2 /* 0x3FB10D66, 0xA0D03D51 */,
  -5.83357013379057348645e-2 /* 0xBFADDE2D, 0x52DEFD9A */,
  4.97687799461593236017e-2 /* 0x3FA97B4B, 0x24760DEB */,
  -3.6531572744216915527e-2 /* 0xBFA2B444, 0x2C6A6C2F */,
  1.62858201153657823623e-2 /* 0x3F90AD3A, 0xE322DA11 */,
] as const;

/**
 * `atan.rs`'s addend for |x| ≥ 2⁶⁶, written `f64::from_bits(0x0380_0000)` beside a `0x1p-120f`
 * comment: as an `f64`, a subnormal of about 2.9 × 10⁻³¹⁶, which leaves atan(±∞) at ±π/2's head.
 */
const ATAN_HUGE_TAIL = fromWords(0, 0x03800000);

/**
 * The arctangent of `x`, in radians, bit for bit the sim's `hyperion_base::math::atan`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/atan.rs` (FreeBSD's `s_atan.c`): `x` is
 * reduced against atan(0.5), atan(1), atan(1.5) or atan(∞) by the matching identity, and the rest
 * comes from a degree-22 odd polynomial split in two. Under 1 ulp.
 */
export function atan(x: number): number {
  let xr = x;
  let ix = highWord(xr);
  const sign = ix >>> 31;
  ix &= 0x7fffffff;
  if (ix >= 0x44100000) {
    if (Number.isNaN(xr)) {
      return xr;
    }
    const z = ATANHI[3] + ATAN_HUGE_TAIL;
    return sign !== 0 ? -z : z;
  }

  let id: number;
  if (ix < 0x3fdc0000) {
    /* |x| < 0.4375 */
    if (ix < 0x3e400000) {
      /* |x| < 2^-27 */
      return xr;
    }
    id = -1;
  } else {
    xr = Math.abs(xr);
    if (ix < 0x3ff30000) {
      /* |x| < 1.1875 */
      if (ix < 0x3fe60000) {
        /* 7/16 <= |x| < 11/16 */
        xr = (2 * xr - 1) / (2 + xr);
        id = 0;
      } else {
        /* 11/16 <= |x| < 19/16 */
        xr = (xr - 1) / (xr + 1);
        id = 1;
      }
    } else if (ix < 0x40038000) {
      /* |x| < 2.4375 */
      xr = (xr - 1.5) / (1 + 1.5 * xr);
      id = 2;
    } else {
      /* 2.4375 <= |x| < 2^66 */
      xr = -1 / xr;
      id = 3;
    }
  }

  const z = xr * xr;
  const w = z * z;
  /* break sum from i=0 to 10 AT[i]z**(i+1) into odd and even poly */
  const s1 = z * (AT[0] + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
  const s2 = w * (AT[1] + w * (AT[3] + w * (AT[5] + w * (AT[7] + w * AT[9]))));

  if (id < 0) {
    return xr - xr * (s1 + s2);
  }

  const zr = at(ATANHI, id) - (xr * (s1 + s2) - at(ATANLO, id) - xr);
  return sign !== 0 ? -zr : zr;
}

// `src/math/atan2.rs` (FreeBSD's e_atan2.c).
// atan2.rs's literal for π, kept as written (the double Math.PI is).
// oxlint-disable-next-line approx-constant
const ATAN2_PI = 3.141592653589793116; /* 0x400921FB, 0x54442D18 */
// atan2.rs's literal as written; it reads as the nearest double, as Rust's does.
// oxlint-disable-next-line no-loss-of-precision
const ATAN2_PI_LO = 1.2246467991473531772e-16; /* 0x3CA1A626, 0x33145C07 */

/**
 * The angle of the point (`x`, `y`) from the +x axis, in radians in [−π, π], bit for bit the sim's
 * `hyperion_base::math::atan2`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/atan2.rs` (FreeBSD's `e_atan2.c`): the signs
 * and the special values pick the quadrant, and {@link atan} of |y ÷ x| the angle within it.
 * Under 2 ulp.
 */
export function atan2(y: number, x: number): number {
  if (Number.isNaN(x) || Number.isNaN(y)) {
    return x + y;
  }
  let ix = highWord(x);
  const lx = lowWord(x);
  let iy = highWord(y);
  const ly = lowWord(y);
  if (((ix - 0x3ff00000) | lx) === 0) {
    /* x = 1.0 */
    return atan(y);
  }
  const m = ((iy >>> 31) & 1) | ((ix >>> 30) & 2); /* 2*sign(x)+sign(y) */
  ix &= 0x7fffffff;
  iy &= 0x7fffffff;

  /* when y = 0 */
  if ((iy | ly) === 0) {
    switch (m) {
      case 0:
      case 1:
        return y; /* atan(+-0,+anything)=+-0 */
      case 2:
        return ATAN2_PI; /* atan(+0,-anything) = PI */
      default:
        return -ATAN2_PI; /* atan(-0,-anything) =-PI */
    }
  }
  /* when x = 0 */
  if ((ix | lx) === 0) {
    return (m & 1) !== 0 ? -ATAN2_PI / 2 : ATAN2_PI / 2;
  }
  /* when x is INF */
  if (ix === 0x7ff00000) {
    if (iy === 0x7ff00000) {
      switch (m) {
        case 0:
          return ATAN2_PI / 4; /* atan(+INF,+INF) */
        case 1:
          return -ATAN2_PI / 4; /* atan(-INF,+INF) */
        case 2:
          return (3 * ATAN2_PI) / 4; /* atan(+INF,-INF) */
        default:
          return (-3 * ATAN2_PI) / 4; /* atan(-INF,-INF) */
      }
    }
    switch (m) {
      case 0:
        return 0; /* atan(+...,+INF) */
      case 1:
        return -0; /* atan(-...,+INF) */
      case 2:
        return ATAN2_PI; /* atan(+...,-INF) */
      default:
        return -ATAN2_PI; /* atan(-...,-INF) */
    }
  }
  /* |y/x| > 0x1p64 */
  if (ix + (64 << 20) < iy || iy === 0x7ff00000) {
    return (m & 1) !== 0 ? -ATAN2_PI / 2 : ATAN2_PI / 2;
  }

  /* z = atan(|y/x|) without spurious underflow */
  const z =
    (m & 2) !== 0 && iy + (64 << 20) < ix ? 0 : atan(Math.abs(y / x)); /* |y/x| < 0x1p-64, x<0 */
  switch (m) {
    case 0:
      return z; /* atan(+,+) */
    case 1:
      return -z; /* atan(-,+) */
    case 2:
      return ATAN2_PI - (z - ATAN2_PI_LO); /* atan(+,-) */
    default:
      return z - ATAN2_PI_LO - ATAN2_PI; /* atan(-,-) */
  }
}

// `src/math/cbrt.rs` (CORE-MATH's binary64 cbrt.c).
/** 1, 2^(1/3) and 2^(2/3). */
const CBRT_ESCALE = [1, hf64("0x1.428a2f98d728bp+0"), hf64("0x1.965fea53d6e3dp+0")] as const;
/* the polynomial c0+c1*x+c2*x^2+c3*x^3 approximates x^(1/3) on [1,2]
with maximal error < 9.2e-5 (attained at x=2) */
const CBRT_C = [
  hf64("0x1.1b0babccfef9cp-1"),
  hf64("0x1.2c9a3e94d1da5p-1"),
  hf64("-0x1.4dc30b1a1ddbap-3"),
  hf64("0x1.7a8d3e4ec9b07p-6"),
] as const;
const CBRT_U0 = hf64("0x1.5555555555555p-2");
const CBRT_U1 = hf64("0x1.c71c71c71c71cp-3");
/** `cbrt.rs`'s `rsc`: ±1, ±½ and ±¼, by `(it << 1) | sign`. */
const CBRT_RSC = [1, -1, 0.5, -0.5, 0.25, -0.25] as const;
/** `off[Round::Nearest]`. */
const CBRT_OFF = hf64("0x1p-53");
const CBRT_X1P_52 = hf64("0x1p-52");
const CBRT_X1P_60 = hf64("0x1p-60");
const CBRT_X1P_75 = hf64("0x1p-75");
const CBRT_X1P_98 = hf64("0x1p-98");
const CBRT_HARD_1 = hf64("0x1.9b78223aa307cp+1");
const CBRT_HARD_1_ROOT = hf64("0x1.79d15d0e8d59cp+0");
const CBRT_HARD_2 = hf64("0x1.a202bfc89ddffp+2");
const CBRT_HARD_2_ROOT = hf64("0x1.de87aa837820fp+0");

/** Veltkamp's splitter for doubles, 2²⁷ + 1. */
const VELTKAMP_SPLIT = 134_217_729;

/**
 * a × b − `product`, exactly, for `product` = a × b rounded: what `cbrt.rs`'s `a.fma(b, -product)`
 * returns, by Dekker's product (Dekker 1971, Numer. Math. 18, 224), as `hyperion_base::math`'s
 * `two_product` computes it.
 *
 * @remarks
 * Dekker's error is exact, and so the fused multiply-add's to the bit, wherever the factors' split
 * does not overflow and no partial product underflows; `cbrt` asks it only of factors and products
 * of magnitude in [1, 8).
 */
function productError(a: number, b: number, product: number): number {
  let scaled = VELTKAMP_SPLIT * a;
  const aHigh = scaled - (scaled - a);
  const aLow = a - aHigh;
  scaled = VELTKAMP_SPLIT * b;
  const bHigh = scaled - (scaled - b);
  const bLow = b - bHigh;
  return aHigh * bHigh - product + aHigh * bLow + aLow * bHigh + aLow * bLow;
}

/**
 * The cube root of `x`, correctly rounded, bit for bit the sim's `hyperion_base::math::cbrt`.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/cbrt.rs` (CORE-MATH's `cbrt.c`), its
 * rounding to nearest: a cubic fit of z^(1/3) on [1, 2], one cubic and one linear Newton step with
 * the cube carried to about 106 bits, and a second pass where the result lies near a rounding
 * boundary. The fused multiply-adds are {@link productError}'s.
 */
export function cbrt(x: number): number {
  /* rm=0 for rounding to nearest, and other values for directed roundings */
  const hx = highWord(x);
  let mantHi = hx & 0x000fffff;
  let mantLo = lowWord(x);
  const sign = hx >>> 31;

  let e = (hx >>> 20) & 0x7ff;

  if (((e + 1) & 0x7ff) < 2) {
    const ixHi = hx & 0x7fffffff;

    /* 0, inf, nan: we return x + x instead of simply x,
    to that for x a signaling NaN, it correctly triggers
    the invalid exception. */
    if (e === 0x7ff || (ixHi | mantLo) === 0) {
      return x + x;
    }

    const nz = (ixHi !== 0 ? Math.clz32(ixHi) : 32 + Math.clz32(mantLo)) - 11; /* subnormal */
    if (nz >= 32) {
      mantHi = mantLo << (nz - 32);
      mantLo = 0;
    } else {
      mantHi = (mantHi << nz) | (mantLo >>> (32 - nz));
      mantLo = mantLo << nz;
    }
    mantHi &= 0x000fffff;
    e -= nz - 1;
  }

  e += 3072;
  const cvt1Hi = mantHi | 0x3ff00000;

  const et = Math.floor(e / 3);
  const it = e % 3;

  /* 2^(3k+it) <= x < 2^(3k+it+1), with 0 <= it <= 3 */
  const zz = fromWords(cvt1Hi + it * 0x100000 + sign * 0x80000000, mantLo);

  /* cbrt(x) = cbrt(zz)*2^(et-1365) where 1 <= zz < 8 */
  const isc = sign !== 0 ? -at(CBRT_ESCALE, it) : at(CBRT_ESCALE, it);
  const z = fromWords(cvt1Hi, mantLo);

  /* cbrt(zz) = cbrt(z)*isc, where isc encodes 1, 2^(1/3) or 2^(2/3),
  and 1 <= z < 2 */
  const r = 1 / z;
  const rr = r * at(CBRT_RSC, (it << 1) | sign);
  const z2 = z * z;
  const c0 = CBRT_C[0] + z * CBRT_C[1];
  const c2 = CBRT_C[2] + z * CBRT_C[3];
  let y = c0 + z2 * c2;
  let y2 = y * y;

  /* y is an approximation of z^(1/3) */
  let h = y2 * (y * r) - 1;

  /* h determines the error between y and z^(1/3) */
  y -= h * y * (CBRT_U0 - CBRT_U1 * h);

  /* The correction y -= (h*y)*(u0 - u1*h) corresponds to a cubic variant
  of Newton's method, with the function f(y) = 1-z/y^3. */
  y *= isc;

  /* Now y is an approximation of zz^(1/3),
   * and rr an approximation of 1/zz. We now perform another iteration of
   * Newton-Raphson, this time with a linear approximation only. */
  y2 = y * y;
  let y2l = productError(y, y, y2);

  /* y2 + y2l = y^2 exactly */
  let y3 = y2 * y;
  let y3l = productError(y, y2, y3) + y * y2l;

  /* y3 + y3l approximates y^3 with about 106 bits of accuracy */
  h = (y3 - zz + y3l) * rr;
  let dy = h * (y * CBRT_U0);

  /* the approximation of zz^(1/3) is y - dy */
  let y1 = y - dy;
  dy = y - y1 - dy;

  /* the approximation of zz^(1/3) is now y1 + dy, where |dy| < 1/2 ulp(y)
   * (for rounding to nearest) */
  let ady = Math.abs(dy);

  /* for rounding to nearest, ady0 is tiny when dy is near from 1/2 ulp(1),
   * or from 3/2 ulp(1). */
  let ady0 = Math.abs(ady - CBRT_OFF);
  let ady1 = Math.abs(ady - (CBRT_X1P_52 + CBRT_OFF));

  if (ady0 < CBRT_X1P_75 || ady1 < CBRT_X1P_75) {
    y2 = y1 * y1;
    y2l = productError(y1, y1, y2);
    y3 = y2 * y1;
    y3l = productError(y1, y2, y3) + y1 * y2l;
    h = (y3 - zz + y3l) * rr;
    dy = h * (y1 * CBRT_U0);
    y = y1 - dy;
    dy = y1 - y - dy;
    y1 = y;
    ady = Math.abs(dy);
    ady0 = Math.abs(ady - CBRT_OFF);
    ady1 = Math.abs(ady - (CBRT_X1P_52 + CBRT_OFF));

    if (ady0 < CBRT_X1P_98 || ady1 < CBRT_X1P_98) {
      const azz = Math.abs(zz);

      // ~ 0x1.79d15d0e8d59b80000000000000ffc3dp+0
      if (azz === CBRT_HARD_1) {
        y1 = zz < 0 ? -CBRT_HARD_1_ROOT : CBRT_HARD_1_ROOT;
      }

      // ~ 0x1.de87aa837820e80000000000001c0f08p+0
      if (azz === CBRT_HARD_2) {
        y1 = zz < 0 ? -CBRT_HARD_2_ROOT : CBRT_HARD_2_ROOT;
      }
    }
  }

  /* cvt3 = y1's bits with et - 342 - 1023 added to the exponent, the u64's wrapping add on its
  high word (its low word gains nothing) */
  let cvt3Hi = (highWord(y1) + (et - 342 - 1023) * 0x100000) >>> 0;
  let cvt3Lo = lowWord(y1);
  /* m0 = cvt3 << 30 and m1 = m0 >> 63 (logical): (m0 ^ m1) <= 1 << 30 holds just where cvt3's
  low 34 bits are 0 or 1 */
  if ((cvt3Hi & 3) === 0 && cvt3Lo <= 1) {
    const sum4 = lowWord(y1) + (164 << 15);
    const cvt4 = fromWords(highWord(y1) + (sum4 >= 0x100000000 ? 1 : 0), sum4 & 0xffff0000);

    if (Math.abs(cvt4 - y1 - dy) < CBRT_X1P_60 || Math.abs(zz) === 1) {
      const sum3 = cvt3Lo + (1 << 15);
      cvt3Hi += sum3 >= 0x100000000 ? 1 : 0;
      cvt3Lo = sum3 & 0xffff0000;
    }
  }

  return fromWords(cvt3Hi, cvt3Lo);
}

// `src/math/pow.rs` (FreeBSD's e_pow.c).
const POW_BP = [1.0, 1.5] as const;
const POW_DP_H = [0.0, 5.84962487220764160156e-1] as const; /* 0x3fe2b803_40000000 */
const POW_DP_L = [0.0, 1.35003920212974897128e-8] as const; /* 0x3E4CFDEB, 0x43CFD006 */
const POW_TWO53 = 9007199254740992.0; /* 0x43400000_00000000 */
const POW_HUGE = 1.0e300;
const POW_TINY = 1.0e-300;

// poly coefs for (3/2)*(log(x)-2s-2/3*s**3:
const POW_L1 = 5.99999999999994648725e-1; /* 0x3fe33333_33333303 */
const POW_L2 = 4.28571428578550184252e-1; /* 0x3fdb6db6_db6fabff */
const POW_L3 = 3.33333329818377432918e-1; /* 0x3fd55555_518f264d */
const POW_L4 = 2.72728123808534006489e-1; /* 0x3fd17460_a91d4101 */
const POW_L5 = 2.30660745775561754067e-1; /* 0x3fcd864a_93c9db65 */
const POW_L6 = 2.06975017800338417784e-1; /* 0x3fca7e28_4a454eef */
const POW_P1 = 1.66666666666666019037e-1; /* 0x3fc55555_5555553e */
const POW_P2 = -2.77777777770155933842e-3; /* 0xbf66c16c_16bebd93 */
const POW_P3 = 6.61375632143793436117e-5; /* 0x3f11566a_af25de2c */
const POW_P4 = -1.6533902205465251539e-6; /* 0xbebbbd41_c5d26bf1 */
const POW_P5 = 4.13813679705723846039e-8; /* 0x3e663769_72bea4d0 */
// pow.rs's literal for ln 2, kept as written (the double Math.LN2 is).
// oxlint-disable-next-line approx-constant
const POW_LG2 = 6.93147180559945286227e-1; /* 0x3fe62e42_fefa39ef */
const POW_LG2_H = 6.93147182464599609375e-1; /* 0x3fe62e43_00000000 */
const POW_LG2_L = -1.90465429995776804525e-9; /* 0xbe205c61_0ca86c39 */
// pow.rs's literal as written; it reads as the nearest double, as Rust's does.
// oxlint-disable-next-line no-loss-of-precision
const POW_OVT = 8.0085662595372944372e-17; /* -(1024-log2(ovfl+.5ulp)) */
const POW_CP = 9.61796693925975554329e-1; /* 0x3feec709_dc3a03fd =2/(3ln2) */
const POW_CP_H = 9.61796700954437255859e-1; /* 0x3feec709_e0000000 =(float)cp */
const POW_CP_L = -7.02846165095275826516e-9; /* 0xbe3e2fe0_145b01f5 =tail of cp_h*/
// pow.rs's literal for 1 ÷ ln 2, kept as written (the double Math.LOG2E is).
// oxlint-disable-next-line approx-constant
const POW_IVLN2 = 1.442695040888963387; /* 0x3ff71547_652b82fe =1/ln2 */
const POW_IVLN2_H = 1.44269502162933349609; /* 0x3ff71547_60000000 =24b 1/ln2*/
const POW_IVLN2_L = 1.92596299112661746887e-8; /* 0x3e54ae0b_f85ddf44 =1/ln2 tail*/

/**
 * `x` raised to the power `y`, bit for bit the sim's `hyperion_base::math::powf`: what every `**`
 * on a committed table's path is, but `** 2` (`x * x`) and `** 0.5` (`Math.sqrt(x)`), which are
 * exact on every platform.
 *
 * @remarks
 * Ported line for line from `libm` 0.2.16's `src/math/pow.rs` (FreeBSD's `e_pow.c`), as
 * `hyperion_base`'s `math/pow_base.rs` transcribes it: log₂|x| in two pieces of extra precision,
 * y log₂|x| = n + y′ with |y′| ≤ ½ in simulated multi-precision, and 2^y′ by `exp`'s fit, scaled by
 * 2ⁿ; the special values of C99's `pow` before it. Under 1 ulp, not correctly rounded: it is not
 * Linux's, macOS's or Windows's `pow`, which is why the committed tables call it.
 */
export function pow(x: number, y: number): number {
  let t1: number;
  let t2: number;

  const hx = highWord(x) | 0;
  const lx = lowWord(x);
  const hy = highWord(y) | 0;
  const ly = lowWord(y);

  let ix = hx & 0x7fffffff;
  const iy = hy & 0x7fffffff;

  /* x**0 = 1, even if x is NaN */
  if ((iy | ly) === 0) {
    return 1.0;
  }

  /* 1**y = 1, even if y is NaN */
  if (hx === 0x3ff00000 && lx === 0) {
    return 1.0;
  }

  /* NaN if either arg is NaN */
  if (
    ix > 0x7ff00000 ||
    (ix === 0x7ff00000 && lx !== 0) ||
    iy > 0x7ff00000 ||
    (iy === 0x7ff00000 && ly !== 0)
  ) {
    return x + y;
  }

  /* determine if y is an odd int when x < 0
   * yisint = 0       ... y is not an integer
   * yisint = 1       ... y is an odd int
   * yisint = 2       ... y is an even int
   */
  let yisint = 0;
  let k: number;
  let j: number;
  if (hx < 0) {
    if (iy >= 0x43400000) {
      yisint = 2; /* even integer y */
    } else if (iy >= 0x3ff00000) {
      k = (iy >> 20) - 0x3ff; /* exponent */

      if (k > 20) {
        j = (ly >>> (52 - k)) | 0;

        if (j << (52 - k) === (ly | 0)) {
          yisint = 2 - (j & 1);
        }
      } else if (ly === 0) {
        j = iy >> (20 - k);

        if (j << (20 - k) === iy) {
          yisint = 2 - (j & 1);
        }
      }
    }
  }

  if (ly === 0) {
    /* special value of y */
    if (iy === 0x7ff00000) {
      /* y is +-inf */
      if (((ix - 0x3ff00000) | lx) === 0) {
        /* (-1)**+-inf is 1 */
        return 1.0;
      } else if (ix >= 0x3ff00000) {
        /* (|x|>1)**+-inf = inf,0 */
        return hy >= 0 ? y : 0.0;
      }
      /* (|x|<1)**+-inf = 0,inf */
      return hy >= 0 ? 0.0 : -y;
    }

    if (iy === 0x3ff00000) {
      /* y is +-1 */
      return hy >= 0 ? x : 1.0 / x;
    }

    if (hy === 0x40000000) {
      /* y is 2 */
      return x * x;
    }

    if (hy === 0x3fe00000) {
      /* y is 0.5 */
      if (hx >= 0) {
        /* x >= +0 */
        return Math.sqrt(x);
      }
    }
  }

  let ax = Math.abs(x);
  if (lx === 0) {
    /* special value of x */
    if (ix === 0x7ff00000 || ix === 0 || ix === 0x3ff00000) {
      /* x is +-0,+-inf,+-1 */
      let z = ax;

      if (hy < 0) {
        /* z = (1/|x|) */
        z = 1.0 / z;
      }

      if (hx < 0) {
        if (((ix - 0x3ff00000) | yisint) === 0) {
          z = (z - z) / (z - z); /* (-1)**non-int is NaN */
        } else if (yisint === 1) {
          z = -z; /* (x<0)**odd = -(|x|**odd) */
        }
      }

      return z;
    }
  }

  let s = 1.0; /* sign of result */
  if (hx < 0) {
    if (yisint === 0) {
      /* (x<0)**(non-int) is NaN */
      return (x - x) / (x - x);
    }

    if (yisint === 1) {
      /* (x<0)**(odd int) */
      s = -1.0;
    }
  }

  /* |y| is HUGE */
  if (iy > 0x41e00000) {
    /* if |y| > 2**31 */
    if (iy > 0x43f00000) {
      /* if |y| > 2**64, must o/uflow */
      if (ix <= 0x3fefffff) {
        return hy < 0 ? POW_HUGE * POW_HUGE : POW_TINY * POW_TINY;
      }

      if (ix >= 0x3ff00000) {
        return hy > 0 ? POW_HUGE * POW_HUGE : POW_TINY * POW_TINY;
      }
    }

    /* over/underflow if x is not close to one */
    if (ix < 0x3fefffff) {
      return hy < 0 ? s * POW_HUGE * POW_HUGE : s * POW_TINY * POW_TINY;
    }
    if (ix > 0x3ff00000) {
      return hy > 0 ? s * POW_HUGE * POW_HUGE : s * POW_TINY * POW_TINY;
    }

    /* now |1-x| is TINY <= 2**-20, suffice to compute
    log(x) by x-x^2/2+x^3/3-x^4/4 */
    const t = ax - 1.0; /* t has 20 trailing zeros */
    // pow.rs's literal for ⅓ as written; it reads as the nearest double, as Rust's does.
    // oxlint-disable-next-line no-loss-of-precision
    const w = t * t * (0.5 - t * (0.3333333333333333333333 - t * 0.25));
    const u = POW_IVLN2_H * t; /* ivln2_h has 21 sig. bits */
    const v = t * POW_IVLN2_L - w * POW_IVLN2;
    t1 = withLowWord(u + v, 0);
    t2 = v - (t1 - u);
  } else {
    let n = 0;

    if (ix < 0x00100000) {
      /* take care subnormal number */
      ax *= POW_TWO53;
      n -= 53;
      ix = highWord(ax) | 0;
    }

    n += (ix >> 20) - 0x3ff;
    j = ix & 0x000fffff;

    /* determine interval */
    let kk: number;
    ix = j | 0x3ff00000; /* normalize ix */
    if (j <= 0x3988e) {
      /* |x|<sqrt(3/2) */
      kk = 0;
    } else if (j < 0xbb67a) {
      /* |x|<sqrt(3)   */
      kk = 1;
    } else {
      kk = 0;
      n += 1;
      ix -= 0x00100000;
    }
    ax = withHighWord(ax, ix);

    /* compute ss = s_h+s_l = (x-1)/(x+1) or (x-1.5)/(x+1.5) */
    const bp = at(POW_BP, kk);
    const u = ax - bp; /* bp[0]=1.0, bp[1]=1.5 */
    const v = 1.0 / (ax + bp);
    const ss = u * v;
    const sH = withLowWord(ss, 0);

    /* t_h=ax+bp[k] High */
    let tH = fromWords(((ix >>> 1) | 0x20000000) + 0x00080000 + kk * 0x40000, 0);
    let tL = ax - (tH - bp);
    const sL = v * (u - sH * tH - sH * tL);

    /* compute log(ax) */
    let s2 = ss * ss;
    let r =
      s2 *
      s2 *
      (POW_L1 + s2 * (POW_L2 + s2 * (POW_L3 + s2 * (POW_L4 + s2 * (POW_L5 + s2 * POW_L6)))));
    r += sL * (sH + ss);
    s2 = sH * sH;
    tH = withLowWord(3.0 + s2 + r, 0);
    tL = r - (tH - 3.0 - s2);

    /* u+v = ss*(1+...) */
    const uu = sH * tH;
    const vv = sL * tH + tL * ss;

    /* 2/(3log2)*(ss+...) */
    const pH = withLowWord(uu + vv, 0);
    const pL = vv - (pH - uu);
    const zH = POW_CP_H * pH; /* cp_h+cp_l = 2/(3*log2) */
    const zL = POW_CP_L * pH + pL * POW_CP + at(POW_DP_L, kk);

    /* log2(ax) = (ss+..)*2/(3*log2) = n + dp_h + z_h + z_l */
    const t = n;
    const dpH = at(POW_DP_H, kk);
    t1 = withLowWord(zH + zL + dpH + t, 0);
    t2 = zL - (t1 - t - dpH - zH);
  }

  /* split up y into y1+y2 and compute (y1+y2)*(t1+t2) */
  const y1 = withLowWord(y, 0);
  const pL = (y - y1) * t1 + y * t2;
  let pH = y1 * t1;
  let z = pL + pH;
  j = highWord(z) | 0;
  let i = lowWord(z) | 0;

  if (j >= 0x40900000) {
    /* z >= 1024 */
    if (((j - 0x40900000) | i) !== 0) {
      /* if z > 1024 */
      return s * POW_HUGE * POW_HUGE; /* overflow */
    }

    if (pL + POW_OVT > z - pH) {
      return s * POW_HUGE * POW_HUGE; /* overflow */
    }
  } else if ((j & 0x7fffffff) >= 0x4090cc00) {
    /* z <= -1075 */
    if ((((j >>> 0) - 0xc090cc00) | i) !== 0) {
      /* z < -1075 */
      return s * POW_TINY * POW_TINY; /* underflow */
    }

    if (pL <= z - pH) {
      return s * POW_TINY * POW_TINY; /* underflow */
    }
  }

  /* compute 2**(p_h+p_l) */
  i = j & 0x7fffffff;
  k = (i >> 20) - 0x3ff;
  let n = 0;

  if (i > 0x3fe00000) {
    /* if |z| > 0.5, set n = [z+0.5] */
    n = j + (0x00100000 >> (k + 1));
    k = ((n & 0x7fffffff) >> 20) - 0x3ff; /* new k for n */
    const t = fromWords(n & ~(0x000fffff >> k), 0);
    n = ((n & 0x000fffff) | 0x00100000) >> (20 - k);
    if (j < 0) {
      n = -n;
    }
    pH -= t;
  }

  const t = withLowWord(pL + pH, 0);
  const u = t * POW_LG2_H;
  const v = (pL - (t - pH)) * POW_LG2 + t * POW_LG2_L;
  z = u + v;
  const w = v - (z - u);
  const tt = z * z;
  const tt1 = z - tt * (POW_P1 + tt * (POW_P2 + tt * (POW_P3 + tt * (POW_P4 + tt * POW_P5))));
  const r = (z * tt1) / (tt1 - 2.0) - (w + z * w);
  z = 1.0 - (r - z);
  j = highWord(z) | 0;
  j += n << 20;

  if (j >> 20 <= 0) {
    /* subnormal output */
    z = scalbn(z, n);
  } else {
    z = withHighWord(z, j);
  }

  return s * z;
}
