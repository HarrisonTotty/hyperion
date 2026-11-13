// Trigonometry to f32's precision, without WGSL's builtins (plan R08, Design note 18; R08.T6.b's
// forms, moved here and put in Horner form by R08.T6.g). A library prepended to every atmosphere
// module before `common.wgsl`: the per-planet kernels, the per-view kernels and the composite. No
// atmosphere module calls WGSL's `sin`, `cos`, `tan`, `asin`, `acos`, `atan` or `atan2`, whose bounds
// are loose: `cos` and `sin` to 2^-11 absolute, and only within [-pi, pi]; `atan` and `atan2` to
// 4,096 ULP; `acos` and `asin` to the worse of 6.77e-5 and 6.81e-5 absolute and atan2's inherited
// bound (W3C WGSL, Candidate Recommendation Draft 2026-09-21, section 15.7.4.1, "Accuracy of
// Concrete Floating Point Expressions"). An implementation such as SwiftShader comes near them
// (R08.T6.b), while at the limb the sky view moves by 2e-2 of itself for 1e-4 in a ray's zenith
// cosine. Directions are carried as direction cosines; these forms serve where an angle is itself a
// table's parameter (the sky view's zenith, a tabulated phase's u = sqrt(theta / pi)), so that a
// table's write and read invert.
//
// Each series is a polynomial in Horner form with multiplications by constant coefficients, which
// WGSL rounds correctly, in place of R08.T6.b's divisions by constants, which it allows 2.5 ULP.
// Emulated in f32 with correctly rounded operations (`precision.test.ts`), each is within 5e-7 of
// f64 over its domain; with every division and `sqrt` at the extremes WGSL allows them (2.5 ULP,
// and `sqrt` inherited from 1 / inverseSqrt), preciseAtan2 reaches about 1e-6 (the science
// check, R08.T6.g).

const TRIG_PI : f32 = 3.14159265358979;
const TRIG_HALF_PI : f32 = 1.57079632679490;

// sin and cos of theta in [0, pi], as (sin, cos): s and c are the Taylor series of sin and cos at
// y = theta / 2 in [0, pi / 2], to y^15 and y^14 (truncation under 7e-11 at y = pi / 2), and
// sin theta = 2 s c, cos theta = (c - s)(c + s), neither of which cancels.
fn preciseSinCos(theta : f32) -> vec2f {
  let y = 0.5 * theta;
  let y2 = y * y;
  let sinSeries = 1.0 + y2 * (-1.0 / 6.0 + y2 * (1.0 / 120.0 + y2 * (-1.0 / 5040.0
    + y2 * (1.0 / 362880.0 + y2 * (-1.0 / 39916800.0 + y2 * (1.0 / 6227020800.0
    + y2 * (-1.0 / 1307674368000.0)))))));
  let cosSeries = 1.0 + y2 * (-1.0 / 2.0 + y2 * (1.0 / 24.0 + y2 * (-1.0 / 720.0
    + y2 * (1.0 / 40320.0 + y2 * (-1.0 / 3628800.0 + y2 * (1.0 / 479001600.0
    + y2 * (-1.0 / 87178291200.0)))))));
  let s = y * sinSeries;
  let c = cosSeries;
  return vec2f(2.0 * s * c, (c - s) * (c + s));
}

// cos theta for theta in [0, pi] (`preciseSinCos`).
fn preciseCos(theta : f32) -> f32 {
  return preciseSinCos(theta).y;
}

// atan x for x in [0, 1]: two halvings, atan x = 2 atan(x / (1 + sqrt(1 + x^2))), take [0, 1] into
// [0, tan(pi / 16)] = [0, 0.199], and there the Taylor series to x^15 (truncation under 1e-13 at
// the reduced argument, 3e-13 in the result).
fn preciseAtanUnit(x : f32) -> f32 {
  var t = x / (1.0 + sqrt(1.0 + x * x));
  t = t / (1.0 + sqrt(1.0 + t * t));
  let t2 = t * t;
  let series = 1.0 + t2 * (-1.0 / 3.0 + t2 * (1.0 / 5.0 + t2 * (-1.0 / 7.0 + t2 * (1.0 / 9.0
    + t2 * (-1.0 / 11.0 + t2 * (1.0 / 13.0 + t2 * (-1.0 / 15.0)))))));
  return 4.0 * (t * series);
}

// atan x for x >= 0: above 1, pi / 2 - atan(1 / x).
fn preciseAtan(x : f32) -> f32 {
  if (x > 1.0) {
    return TRIG_HALF_PI - preciseAtanUnit(1.0 / x);
  }
  return preciseAtanUnit(x);
}

// The angle in [0, pi] whose sine is proportional to y >= 0 and whose cosine is proportional to x,
// as atan2(y, x), for y and x not both 0: the smaller of |x| and y over the larger, so that nothing
// is divided by 0 and one series serves.
fn preciseAtan2(y : f32, x : f32) -> f32 {
  let ax = abs(x);
  let a = preciseAtanUnit(min(y, ax) / max(y, ax));
  let b = select(a, TRIG_HALF_PI - a, y > ax);
  return select(b, TRIG_PI - b, x < 0.0);
}
