// What the bake's kernels share (plan R06, Design note 21, T13.g): the cube's power-of-two scale
// from the brightest texel's bits, and the rgb9e5ufloat packer, `pack.ts`'s in WGSL, alike to the
// bit for f32 inputs. Every step is a scaling by a power of two (`ldexp`), a floor or a comparison,
// exact in f32.

// The exponent the brightest texel is scaled to at most: 2^15 (`CUBE_PEAK_EXPONENT`).
const PEAK_EXPONENT = 15;
// rgb9e5's largest channel, (2^9 − 1) ÷ 2^9 × 2^16.
const RGB9E5_MAX = 65408.0;

// The scale's exponent k from the brightest luminance: the largest k with peak × 2^k ≤ 2^15, so
// that peak × 2^k lies in (2^14, 2^15]; 0 for a black cube. `mips.ts`'s `peakScaleExponent`.
fn scaleExponent(peak : f32) -> i32 {
  if (!(peak > 0.0)) {
    return 0;
  }
  // peak = fraction × 2^e with fraction in [0.5, 1): ceil(log2 peak) is e, or e − 1 at a power of two.
  let parts = frexp(peak);
  let ceilLog2 = select(parts.exp, parts.exp - 1, parts.fract == 0.5);
  return PEAK_EXPONENT - ceilLog2;
}

// q rounded to nearest, halves up, by its fraction: exact for any f32 q.
fn roundHalfUp(q : f32) -> f32 {
  let whole = floor(q);
  return select(whole, whole + 1.0, q - whole >= 0.5);
}

// A channel clamped to [0, 65,408], negatives taken as 0 (the bake gives no NaN).
fn clampChannel(value : f32) -> f32 {
  return select(0.0, min(value, RGB9E5_MAX), value > 0.0);
}

// One texel packed: red in bits 0–8, green 9–17, blue 18–26, the shared exponent in 27–31
// (EXT_texture_shared_exponent). `pack.ts`'s `packRgb9e5`.
fn packRgb9e5(rgb : vec3f) -> u32 {
  let c = vec3f(clampChannel(rgb.r), clampChannel(rgb.g), clampChannel(rgb.b));
  let largest = max(c.r, max(c.g, c.b));
  var floorExponent = -16;
  if (largest > 0.0) {
    floorExponent = max(-16, frexp(largest).exp - 1);
  }
  let provisional = floorExponent + 1 + 15;
  let largestMantissa = roundHalfUp(ldexp(largest, -(provisional - 24)));
  let exponent = select(provisional, provisional + 1, largestMantissa == 512.0);
  let shift = -(exponent - 24);
  let r = u32(roundHalfUp(ldexp(c.r, shift)));
  let g = u32(roundHalfUp(ldexp(c.g, shift)));
  let b = u32(roundHalfUp(ldexp(c.b, shift)));
  return r | (g << 9u) | (b << 18u) | (u32(exponent) << 27u);
}
