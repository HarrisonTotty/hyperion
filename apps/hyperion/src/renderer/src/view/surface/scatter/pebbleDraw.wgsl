// Decorative scatter's draw (plan R11, R11.T6.a; Design notes 2 and 3; `pebbleMaterial.ts`): each
// pebble the placement wrote (`pebbles.wgsl`), drawn by a bucket of the particle path's instance
// cull (`clusters/particlePath.ts`) as a half-buried ellipsoid of `PEBBLE_AXES`, lit as the ground
// it stands on: `terrainLit.wgsl`'s `shade_terrain` at the ground's place under the pebble, its
// palette's laws by their class shares, its horizon map, the shadow cascades, the air and the
// planetshine, with the pebble's own normal and its tint on the albedo. Composed after the lit
// terrain's source, its own vertex and fragment stages renamed (`pebbleMaterial.ts`), whose `Draw`
// takes `clusterBase`; it declares the instances, the lists and the patches' records past the lit
// material's bindings.
//
// Its instance (`clusters/particles.ts`' record): the centre less the camera in the passes' frame,
// the radius D ÷ 2 times the fade, the rotation of the pebble's axes, the colour word (its place in
// its patch, two 12-bit fractions, and its tint's byte above them) and its tag, its patch record's
// index (`pebbles.wgsl`'s `ScatterPatch`).

struct PebbleRecord {
  centre : vec3f,
  radius : f32,
  rotation : vec2u,
  colour : u32,
  tag : u32,
}

// `pebbles.wgsl`'s `ScatterPatch`.
struct PebblePatch {
  originM : vec3f,
  slot : u32,
  morphStartM : f32,
  morphEndM : f32,
  parentSlot : u32,
  parentQuadrant : u32,
  key : vec4u,
  cells : vec4u,
}

@group(2) @binding(25) var<storage, read> pebble_instances : array<PebbleRecord>;
@group(2) @binding(26) var<storage, read> pebble_lists : array<u32>;
@group(2) @binding(27) var<storage, read> pebble_patches : array<PebblePatch>;

// `pebbleLaw.ts`' PEBBLE_AXES and PEBBLE_TINT.
const PEBBLE_AXES : vec3f = vec3f(1.1892071150027210, 0.8408964152537145, 0.75);
const PEBBLE_TINT_LOW : f32 = 0.75;
const PEBBLE_TINT_HIGH : f32 = 1.25;

struct PebbleVertexOut {
  @builtin(position) clip : vec4f,
  // The ground's place in its patch under the pebble, (x, y) ÷ 64, and its slot.
  @location(0) patchUv : vec2f,
  @location(1) @interpolate(flat) slot : u32,
  // The vertex less the camera, body-fixed, metres.
  @location(2) fromCameraBf : vec3f,
  // The datum's normal ν under the pebble, body-fixed (renormalised in the fragment).
  @location(3) datumNormal : vec3f,
  // The ground's geodetic height under the pebble, metres.
  @location(4) heightM : f32,
  @location(5) horizonSt : vec2f,
  @location(6) @interpolate(flat) morph : f32,
  @location(7) @interpolate(flat) horizonFace : u32,
  @location(8) @interpolate(flat) parent : vec2u,
  @location(9) @interpolate(flat) cells : vec4u,
  // The pebble's own outward normal at the vertex, body-fixed.
  @location(10) normalBf : vec3f,
  // Its tint, a factor on its ground's albedo.
  @location(11) @interpolate(flat) tint : f32,
}

// `v` turned by the unit quaternion `q` (x, y, z, w), as `particleInstance.wgsl`'s.
fn pebble_rotate(q : vec4f, v : vec3f) -> vec3f {
  let t = 2.0 * cross(q.xyz, v);
  return v + q.w * t + cross(q.xyz, t);
}

// A tint byte's factor (`pebbleLaw.ts`' `pebbleTint`).
fn pebble_tint(byte : u32) -> f32 {
  return PEBBLE_TINT_LOW + (PEBBLE_TINT_HIGH - PEBBLE_TINT_LOW) * f32(byte) / 255.0;
}

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> PebbleVertexOut {
  let pebble = pebble_instances[pebble_lists[draw.clusterBase + entry]];
  let raw = vec4f(unpack2x16snorm(pebble.rotation.x), unpack2x16snorm(pebble.rotation.y));
  let q = raw * inverseSqrt(max(dot(raw, raw), 1e-12));
  let from_camera = pebble.centre + pebble.radius * pebble_rotate(q, local * PEBBLE_AXES);
  let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
  let tile = pebble_patches[pebble.tag];
  let rec = slots[tile.slot];
  let uv = vec2f(f32(pebble.colour & 0xfffu), f32((pebble.colour >> 12u) & 0xfffu)) / 4095.0;
  let at = uv * 64.0;
  var out : PebbleVertexOut;
  out.clip = frame.clipProjection * frame.viewRotation * vec4f(from_camera, 1.0);
  out.patchUv = uv;
  out.slot = tile.slot;
  out.fromCameraBf = transpose(rotation) * from_camera;
  out.datumNormal = rec.nu0 + normalDifference(rec, directionDifferenceAt(rec, at));
  // The ground's morph at the pebble's centre, by its distance as the placement took it.
  var k = 0.0;
  if (tile.morphEndM > tile.morphStartM) {
    k = clamp((length(pebble.centre) - tile.morphStartM) / (tile.morphEndM - tile.morphStartM), 0.0, 1.0);
  }
  let heights = surfaceHeightsAt(tile.slot, at);
  out.heightM = mix(heights.x, heights.y, k);
  out.horizonSt = horizon_face_st_at(rec, at);
  out.morph = k;
  out.horizonFace = horizon_face_of(rec);
  out.parent = vec2u(tile.parentSlot, tile.parentQuadrant & 3u);
  out.cells = tile.cells;
  out.normalBf = transpose(rotation) * pebble_rotate(q, normalize(local / PEBBLE_AXES));
  out.tint = pebble_tint(pebble.colour >> 24u);
  return out;
}

@fragment
fn fragmentMain(in : PebbleVertexOut) -> @location(0) vec4f {
  // The ground under the pebble as the lit terrain's fragment would take it: a grid texel, no
  // rebake's fade, its own class sets.
  var ground : LitVertexOut;
  ground.clip = in.clip;
  ground.patchUv = in.patchUv;
  ground.slot = in.slot;
  ground.fromCameraBf = in.fromCameraBf;
  ground.datumNormal = in.datumNormal;
  ground.heightM = in.heightM;
  ground.horizonSt = in.horizonSt;
  ground.morph = in.morph;
  ground.horizonFace = in.horizonFace;
  ground.parent = in.parent;
  ground.kind = LIT_GRID;
  ground.gridHeightM = in.heightM;
  ground.lipM = 0.0;
  ground.rebakeSlot = 0u;
  ground.rebakeFade = 0.0;
  ground.cells = in.cells;
  ground.sub = 0u;
  let normal = normalize(in.normalBf);
  var cascade_lit = 1.0;
  if (LIT_PROBE != 6u) {
    let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
    let sun = terrain_row(FIRST_TERRAIN_LIGHT_ROW).xyz;
    cascade_lit = cascade_lit_fraction(rotation * in.fromCameraBf, rotation * normal, rotation * sun);
  }
  let shaded = shade_terrain(ground, normal, false, cascade_lit, own_shade_place(ground));
  var meter = METER_OTHER;
  if (u32(terrain_row(0u).x) > 0u) {
    meter = select(METER_UNLIT_BODY, METER_LIT_BODY, shaded.lit);
  }
  return vec4f(min(shaded.radiance * in.tint, vec3f(HALF_FLOAT_MAX)), meter);
}
