// A patch's vertices in the `FaceDifferences` form (plan R05, R05.T11.b, Design note 4): a slot's
// static terms and the f32 arithmetic that rebuilds a vertex's offset from its patch origin from
// them, never from an absolute position. A library composed by concatenation, with no binding or
// entry point of its own: after frame.wgsl and before terrain.wgsl (R05's terrain pass), or before
// R07's smooth figure (`view/shaders/smoothMesh.wgsl`, R07.T9), which draws it at zero height.
//
// The direction and normal differences follow `hyperion_surface::patch::vertex`'s f32 reference
// operation for operation, so that `vertexEmulation.ts` checks them against T4.b's golden. A GPU
// may still differ in the last bits (fused multiply-adds, WGSL's 2.5 ULP division and its sqrt),
// far inside T4.b's 1 mm bound.

// uniforms.ts's SLOT_RECORD_BYTES: PatchTermsF32 and the skirt depth.
struct SlotRecord {
  axisA : vec3f, s0 : f32,
  axisE1 : vec3f, t0 : f32,
  axisE2 : vec3f, u0 : f32,
  scale : vec3f, v0 : f32,
  m0 : vec3f, step : f32,
  nu0 : vec3f, h0M : f32,
  skirtDepthM : f32,
  straddles : u32,
  pad : vec2u,
}

// The cube sphere's warp, `st_to_uv_f32`.
fn stToUv(s : f32) -> f32 {
  if (s >= 0.5) {
    return (4.0 * s * s - 1.0) / 3.0;
  }
  let r = 1.0 - s;
  return (1.0 - 4.0 * r * r) / 3.0;
}

// u(s₀ + δs) − u(s₀), `warp_difference_f32`.
fn warpDifference(s0 : f32, ds : f32, straddles : bool) -> f32 {
  let s = s0 + ds;
  if (straddles) {
    return stToUv(s) - stToUv(s0);
  }
  if (s0 >= 0.5) {
    return 4.0 * ds * (s + s0) / 3.0;
  }
  return 4.0 * ds * (2.0 - s - s0) / 3.0;
}

// p ÷ |p| − p₀ ÷ |p₀| for p = p₀ + Δ, `unit_difference_f32`, with no difference of nearly equal
// numbers formed.
fn unitDifference(p0 : vec3f, delta : vec3f) -> vec3f {
  let dotP = p0.x * delta.x + p0.y * delta.y + p0.z * delta.z;
  let dd = delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;
  let len0Sq = p0.x * p0.x + p0.y * p0.y + p0.z * p0.z;
  let len0 = sqrt(len0Sq);
  let len = sqrt(len0Sq + 2.0 * dotP + dd);
  let diff = -(2.0 * dotP + dd);
  let factor = diff / (len * len0 * (len + len0));
  return delta / len + p0 * factor;
}

// d − d₀, the vertex direction's difference from the patch centre's.
fn directionDifference(rec : SlotRecord, x : u32, y : u32) -> vec3f {
  let straddles = rec.straddles != 0u;
  let du = warpDifference(rec.s0, (f32(x) - 32.0) * rec.step, straddles);
  let dv = warpDifference(rec.t0, (f32(y) - 32.0) * rec.step, straddles);
  let n0 = rec.axisA + rec.u0 * rec.axisE1 + rec.v0 * rec.axisE2;
  let delta = du * rec.axisE1 + dv * rec.axisE2;
  return unitDifference(n0, delta);
}

// ν − ν₀, the spheroid normal's difference, from d − d₀.
fn normalDifference(rec : SlotRecord, dd : vec3f) -> vec3f {
  return unitDifference(rec.m0, dd / rec.scale);
}

// P − P₀ at height h: M (d − d₀) + h (ν − ν₀) + (h − h₀) ν₀, `face_difference_position_f32`.
fn faceDifferencePosition(rec : SlotRecord, x : u32, y : u32, h : f32) -> vec3f {
  let dd = directionDifference(rec, x, y);
  let dnu = normalDifference(rec, dd);
  let dh = h - rec.h0M;
  return rec.scale * dd + h * dnu + dh * rec.nu0;
}
