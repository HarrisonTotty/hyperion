// The terrain pass (plan R05, R05.T11.b, Design notes 4 to 6 and 17): one instanced draw of the
// shared 65 × 65 mesh with skirts, an instance per drawn patch, read from the cache's slots.
// Composed after frame.wgsl and before one of the two vertex paths, terrainBakedOffsets.wgsl or
// terrainFaceDifferences.wgsl, which define `ownOffset` and `morphOffset`.
//
// Every position here is relative: a vertex's offset from its patch origin, in the body-fixed
// axes, from the path; the origin less the camera, in the frame the view draws in, from the
// instance record (narrowed once from f64 on the CPU). The absolute position M·d + h·ν is never
// formed in f32, whose step is half a metre at an Earth's radius (Design note 4).
//
// The direction and normal differences follow `hyperion_surface::patch::vertex`'s f32 reference
// operation for operation, so that `vertexEmulation.ts` checks them against T4.b's golden. A GPU
// may still differ in the last bits (fused multiply-adds, WGSL's 2.5 ULP division and its sqrt),
// far inside T4.b's 1 mm bound.

const VERTICES_PER_SIDE : u32 = 65u;
const PATCH_VERTICES : u32 = 4225u;
// R07's meter class of a lit body (view/post/meter.ts, METER_CLASS.litBody), written as alpha.
const METER_LIT_BODY : f32 = 2.0;
// rgba16float's largest finite value (R02's HALF_FLOAT_MAX).
const HALF_FLOAT_MAX : f32 = 65504.0;

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The unit direction towards the sun, body-fixed; w unused.
  sunDirection : vec4f,
  // albedo ÷ π × the sun's illuminance per channel × the pre-exposure; w unused (Design note 17).
  sunRadiance : vec4f,
  // The normals atlas: tiles across a layer, tiles a layer, a tile's stored texels, samples a side.
  atlas : vec4f,
}

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

// uniforms.ts's INSTANCE_RECORD_BYTES.
struct Instance {
  originM : vec3f,
  slot : u32,
  morphStartM : f32,
  morphEndM : f32,
  pad : vec2f,
}

// uniforms.ts's CONTACT_RECORD_BYTES. The centre is camera-relative in the instance origins' frame
// (the body's rotated axes, as `origin + rotation * offset`), not body-fixed. The hold is
// clamp((|v − c| − r_g) ÷ ramp, 0, 1), a step at r_g when the ramp is 0; T7's `morphHold` and its
// forced region must use the same rule.
struct Contact {
  centreM : vec3f,
  heldRadiusM : f32,
  rampM : f32,
  pad0 : f32,
  pad1 : f32,
  pad2 : f32,
}

struct Contacts {
  count : u32,
  pad0 : u32,
  pad1 : u32,
  pad2 : u32,
  items : array<Contact>,
}

@group(1) @binding(0) var<uniform> draw : Draw;

// Own height then morph height per vertex, slot by slot (the bake's `heights` layout).
@group(2) @binding(0) var<storage, read> heights : array<f32>;
@group(2) @binding(1) var<storage, read> slots : array<SlotRecord>;
@group(2) @binding(2) var<storage, read> instances : array<Instance>;
@group(2) @binding(3) var<storage, read> contacts : Contacts;
// Octahedral normal pairs, body-fixed, one tile a slot with a one-texel gutter.
@group(2) @binding(4) var normals : texture_2d_array<f32>;

struct VertexOut {
  @builtin(position) clip : vec4f,
  // The vertex's place in its patch, (x, y) ÷ 64.
  @location(0) patchUv : vec2f,
  @location(1) @interpolate(flat) slot : u32,
}

// The vertex's own height (0) or morph height (1).
fn heightAt(slot : u32, x : u32, y : u32, which : u32) -> f32 {
  return heights[(slot * PATCH_VERTICES + y * VERTICES_PER_SIDE + x) * 2u + which];
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

// CDLOD's morph factor from the unmorphed vertex's distance (Design note 6), held at zero near
// every grounded contact and rising to 1 across its ramp.
fn morphFactor(inst : Instance, fromCameraM : vec3f) -> f32 {
  let span = inst.morphEndM - inst.morphStartM;
  var k = select(0.0, clamp((length(fromCameraM) - inst.morphStartM) / span, 0.0, 1.0), span > 0.0);
  for (var i = 0u; i < contacts.count; i += 1u) {
    let c = contacts.items[i];
    let beyond = length(fromCameraM - c.centreM) - c.heldRadiusM;
    let hold = select(select(0.0, 1.0, beyond > 0.0), clamp(beyond / c.rampM, 0.0, 1.0), c.rampM > 0.0);
    k = min(k, hold);
  }
  return k;
}

@vertex
fn vertexMain(@location(0) grid : vec3f, @builtin(instance_index) instance : u32) -> VertexOut {
  let inst = instances[instance];
  let rec = slots[inst.slot];
  let x = u32(grid.x);
  let y = u32(grid.y);
  let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
  let origin = draw.offsetFromCameraM + inst.originM;
  let own = ownOffset(inst.slot, rec, x, y);
  let k = morphFactor(inst, origin + rotation * own);
  var q = mix(own, morphOffset(inst.slot, rec, x, y), k);
  if (grid.z > 0.5) {
    // A skirt hangs from its edge vertex down the spheroid's normal there.
    let nu = rec.nu0 + normalDifference(rec, directionDifference(rec, x, y));
    q = q - rec.skirtDepthM * nu;
  }
  var out : VertexOut;
  out.clip = frame.clipProjection * frame.viewRotation * vec4f(origin + rotation * q, 1.0);
  out.patchUv = vec2f(f32(x), f32(y)) / 64.0;
  out.slot = inst.slot;
  return out;
}

// +1 for x ≥ 0, −1 otherwise, as the bake's `sign_not_zero`.
fn signNotZero(v : vec2f) -> vec2f {
  return select(vec2f(1.0), vec2f(-1.0), v < vec2f(0.0));
}

// The unit vector of an octahedral pair, the bake's `decode_octahedral`.
fn decodeOctahedral(e : vec2f) -> vec3f {
  let z = 1.0 - abs(e.x) - abs(e.y);
  var xy = e;
  if (z < 0.0) {
    xy = (1.0 - abs(e.yx)) * signNotZero(e);
  }
  return normalize(vec3f(xy, z));
}

@fragment
fn fragmentMain(in : VertexOut) -> @location(0) vec4f {
  let columns = u32(draw.atlas.x);
  let perLayer = u32(draw.atlas.y);
  let tile = u32(draw.atlas.z);
  let samples = u32(draw.atlas.w);
  let layer = in.slot / perLayer;
  let within = in.slot - layer * perLayer;
  let corner = vec2u(within % columns, within / columns) * tile + vec2u(1u);
  // Filtered by hand on the decoded vectors: the pairs are discontinuous across the lower
  // hemisphere's fold, along x = 0 and y = 0 for z < 0 (four southern half-meridians of the
  // body-fixed frame), where interpolated pairs would decode to a wrong normal.
  let s = in.patchUv * f32(samples - 1u);
  let base = min(vec2u(floor(s)), vec2u(samples - 2u));
  let f = s - vec2f(base);
  let at = corner + base;
  let n00 = decodeOctahedral(textureLoad(normals, at, layer, 0).xy);
  let n10 = decodeOctahedral(textureLoad(normals, at + vec2u(1u, 0u), layer, 0).xy);
  let n01 = decodeOctahedral(textureLoad(normals, at + vec2u(0u, 1u), layer, 0).xy);
  let n11 = decodeOctahedral(textureLoad(normals, at + vec2u(1u, 1u), layer, 0).xy);
  let n = normalize(mix(mix(n00, n10, f.x), mix(n01, n11, f.x), f.y));
  let cosine = max(dot(n, draw.sunDirection.xyz), 0.0);
  return vec4f(min(draw.sunRadiance.rgb * cosine, vec3f(HALF_FLOAT_MAX)), METER_LIT_BODY);
}
