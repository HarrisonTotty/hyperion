// The ring's shadow on its planet (plan R11, R11.T9.a; Design note 14): a ray from the lit point
// towards the star against the ring plane, reading the profile's transmittance where it crosses
// the annulus ahead of the point. It replaces R07's stub in `litBody.wgsl`, and R07's disc and
// mesh regimes call it for a ringed body (`bodyDisc.wgsl`'s and `discAir.wgsl`'s light loops); a
// body whose record carries no ring, and every body where R11's `ringShadowOnBody` is off, reads 1.
// The TypeScript twin is `view/rings/ringShadows.ts`' `ringShadowOnBody`.
//
// A library composed by concatenation after R08's `surfaceLighting.wgsl`, whose `LIT_PROBE` it
// reads, and `litBody.wgsl`, with no binding of its own. Its includer defines two functions:
// `ring_shadow_row(i : u32) -> vec4f`, the i-th of RING_SHADOW_ROWS rows of the body's ring
// (`ringShadows.ts`' `packRingShadowRows`), every length over the planet's equatorial radius a, and
// `ring_shadow_axes(v : vec3f) -> vec3f`, which turns a direction of the rows' axes into the axes of
// the point and the star (the view's, on the disc). The rows:
//   0  the ring plane's unit normal; w, the inner edge's radius ÷ a;
//   1  x, the bands, 0 for none, at most RING_SHADOW_MAX_BANDS;
//   2 + j, bands 2j and 2j + 1: each its outer edge's radius ÷ a and its normal optical depth, the
//      bands inside out and contiguous from the inner edge.

const RING_SHADOW_ROWS : u32 = 10u;
const RING_SHADOW_MAX_BANDS : u32 = 16u;

// The ring's normal optical depth at radius r ÷ a: 0 off the annulus.
fn ring_shadow_tau(r : f32, inner : f32, bands : u32) -> f32 {
  if (!(r >= inner)) {
    return 0.0;
  }
  for (var j = 0u; j < bands; j = j + 1u) {
    let pair = ring_shadow_row(2u + j / 2u);
    let band = select(pair.xy, pair.zw, j % 2u == 1u);
    if (r < band.x) {
      return band.y;
    }
  }
  return 0.0;
}

// The share of a star's direct light the ring lets through to the point `p`, from the planet's
// centre ÷ a, towards the star along unit `sun`: exp(−τ(r) ÷ |μ₀|) where p + t sun crosses the plane
// at t > 0 and radius r, μ₀ = sun · n; 1 where it crosses nowhere ahead, off the annulus, or where
// the star lies in the plane. Grey, since the ring's particles are large; per channel for the
// callers, which take a vec3f. 1 under the lit probe's `no-ring-shadow` form (`LIT_PROBE` 9), its
// cost's B side, a measurement's picture (the owner's Mac pair `ring-shadow-on-body`).
fn ring_shadow_on_body(p : vec3f, sun : vec3f) -> vec3f {
  let bands = min(u32(ring_shadow_row(1u).x), RING_SHADOW_MAX_BANDS);
  if (bands == 0u || LIT_PROBE == 9u) {
    return vec3f(1.0);
  }
  let head = ring_shadow_row(0u);
  let n = ring_shadow_axes(head.xyz);
  let mu0 = dot(sun, n);
  let height = dot(p, n);
  if (!(abs(mu0) > 0.0) || !(height * mu0 < 0.0)) {
    return vec3f(1.0);
  }
  let tau = ring_shadow_tau(length(p - (height / mu0) * sun), head.w, bands);
  if (!(tau > 0.0)) {
    return vec3f(1.0);
  }
  return vec3f(exp(-tau / abs(mu0)));
}
