// The terrain pass's patches (plan R05, R05.T11.b, Design notes 4 to 6 and 17; shared by plan R10's
// lit material, R10.T10.b): the slot, instance and contact records, their bindings, and the vertex
// and normal arithmetic both terrain materials draw with. A library composed by concatenation with
// frame.wgsl, patchVertex.wgsl (`SlotRecord` and the `FaceDifferences` arithmetic), one of the two
// vertex paths (terrainBakedOffsets.wgsl or terrainFaceDifferences.wgsl, which define `ownOffset`
// and `morphOffset`) and a material's own source (R05's terrain.wgsl, R10's terrainLit.wgsl), which
// declares the `Draw` uniform `draw` with `offsetFromCameraM`, `bodyRotation` and `atlas`, and the
// entry points.
//
// Every position here is relative: a vertex's offset from its patch origin, in the body-fixed
// axes, from the path; the origin less the camera, in the frame the view draws in, from the
// instance record (narrowed once from f64 on the CPU). The absolute position M·d + h·ν is never
// formed in f32, whose step is half a metre at an Earth's radius (Design note 4).

const VERTICES_PER_SIDE : u32 = 65u;
const PATCH_VERTICES : u32 = 4225u;
// R07's meter class of a lit body (view/post/meter.ts, METER_CLASS.litBody), written as alpha.
const METER_LIT_BODY : f32 = 2.0;
// rgba16float's largest finite value (R02's HALF_FLOAT_MAX).
const HALF_FLOAT_MAX : f32 = 65504.0;

// uniforms.ts's INSTANCE_RECORD_BYTES. The parent's slot and the patch's quadrant in it (bit 0
// i & 1, bit 1 j & 1) are for the horizon map's and the normals' blends (R10.T8.b, T8.c;
// decision-r10-t8a §1), the slot 0xffffffff where no parent's map is resident. Above the quadrant,
// bits 2 to 17 of `parentQuadrant` are the patch's morph holds at its coarse–fine edges and
// corners (`coarseFineMorph`; coarseFineHolds.ts), so every reader of the quadrant masks it.
struct Instance {
  originM : vec3f,
  slot : u32,
  morphStartM : f32,
  morphEndM : f32,
  parentSlot : u32,
  parentQuadrant : u32,
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

// Own height then morph height per vertex, slot by slot (the bake's `heights` layout).
@group(2) @binding(0) var<storage, read> heights : array<f32>;
@group(2) @binding(1) var<storage, read> slots : array<SlotRecord>;
@group(2) @binding(2) var<storage, read> instances : array<Instance>;
@group(2) @binding(3) var<storage, read> contacts : Contacts;
// Octahedral normal pairs, body-fixed, one tile a slot with a one-texel gutter.
@group(2) @binding(4) var normals : texture_2d_array<f32>;

// The vertex's own height (0) or morph height (1).
fn heightAt(slot : u32, x : u32, y : u32, which : u32) -> f32 {
  return heights[(slot * PATCH_VERTICES + y * VERTICES_PER_SIDE + x) * 2u + which];
}

// The hold bits at k = 0 of vertex (x, y) of a patch's grid: bits 0 to 3 for each of the edges
// x = 0, x = 64, y = 0 and y = 64 it lies on, bits 4 to 7 for the corner (0, 0), (64, 0), (64, 64)
// or (0, 64) it is (coarseFineHolds.ts' `vertexHoldBits`).
fn holdBitsAt(x : u32, y : u32) -> u32 {
  let last = VERTICES_PER_SIDE - 1u;
  let edges = select(0u, 1u, x == 0u) | select(0u, 2u, x == last) | select(0u, 4u, y == 0u) | select(0u, 8u, y == last);
  let corner = select(0u, 16u, edges == 5u) | select(0u, 32u, edges == 6u) | select(0u, 64u, edges == 10u) | select(0u, 128u, edges == 9u);
  return edges | corner;
}

// The coarse–fine hold (R10.T8.c; decision-r10-t16-leans §9) on CDLOD's `k` at vertex (x, y) of
// an instance whose quadrant word is `word`: 1 where its holds at k = 1 (bits 10 to 17) cover the
// vertex, a coarser drawn patch touching it there, else 0 where its holds at k = 0 (bits 2 to 9)
// do, a finer one touching it, else `k`. So both sides of every coarse–fine edge and corner lie on
// the coarser level's surface in every view, whatever its own bands give there; the TypeScript
// twin is coarseFineHolds.ts' `coarseFineMorph`.
fn coarseFineMorph(word : u32, x : u32, y : u32, k : f32) -> f32 {
  let at = holdBitsAt(x, y);
  let toOwn = ((word >> 2u) & 0xffu & at) != 0u;
  let toParent = ((word >> 10u) & 0xffu & at) != 0u;
  return select(select(k, 0.0, toOwn), 1.0, toParent);
}

// CDLOD's morph factor at vertex (x, y) from the unmorphed vertex's distance (Design note 6), held
// at the instance's coarse–fine edges and corners (`coarseFineMorph`), then held at zero near every
// grounded contact and rising to 1 across its ramp: a contact's hold wins, so the ground under a
// craft is its own level's (the forced region's edge, past every ramp, is where its patches meet
// coarser ones).
fn morphFactor(inst : Instance, fromCameraM : vec3f, x : u32, y : u32) -> f32 {
  let span = inst.morphEndM - inst.morphStartM;
  let cdlod = select(0.0, clamp((length(fromCameraM) - inst.morphStartM) / span, 0.0, 1.0), span > 0.0);
  var k = coarseFineMorph(inst.parentQuadrant, x, y, cdlod);
  for (var i = 0u; i < contacts.count; i += 1u) {
    let c = contacts.items[i];
    let beyond = length(fromCameraM - c.centreM) - c.heldRadiusM;
    let hold = select(select(0.0, 1.0, beyond > 0.0), clamp(beyond / c.rampM, 0.0, 1.0), c.rampM > 0.0);
    k = min(k, hold);
  }
  return k;
}

// One vertex of a drawn patch.
struct PatchVertex {
  // The vertex less the camera, in the frame the instance origins are in, metres.
  fromCameraM : vec3f,
  // Its offset from its patch origin, body-fixed, metres; a skirt's hangs below its edge vertex.
  offsetM : vec3f,
  // Its geodetic height above the datum, metres: the own and morph heights mixed by the morph
  // factor, a skirt vertex taking its edge vertex's (the baked heights, never |P| − R).
  heightM : f32,
  // The datum's unit normal ν there, body-fixed: the spheroid's at the vertex's direction.
  datumNormal : vec3f,
  // Its place in its patch, (x, y) ÷ 64.
  patchUv : vec2f,
  slot : u32,
  // Its morph factor k, CDLOD's with the coarse–fine and grounded holds (`morphFactor`), which
  // mixes its heights.
  morph : f32,
}

// The vertex at `grid` (x, y and 1 for a skirt's) of instance `instance`. A skirt's vertex, and a
// lit mesh's fin's, takes its grid vertex's (x, y), so its morph and holds are its edge vertex's.
fn patchVertexOf(grid : vec3f, instance : u32) -> PatchVertex {
  let inst = instances[instance];
  let rec = slots[inst.slot];
  let x = u32(grid.x);
  let y = u32(grid.y);
  let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
  let origin = draw.offsetFromCameraM + inst.originM;
  let own = ownOffset(inst.slot, rec, x, y);
  let k = morphFactor(inst, origin + rotation * own, x, y);
  var q = mix(own, morphOffset(inst.slot, rec, x, y), k);
  let nu = rec.nu0 + normalDifference(rec, directionDifference(rec, x, y));
  if (grid.z > 0.5) {
    // A skirt hangs from its edge vertex down the spheroid's normal there.
    q = q - rec.skirtDepthM * nu;
  }
  var v : PatchVertex;
  v.fromCameraM = origin + rotation * q;
  v.offsetM = q;
  v.heightM = mix(heightAt(inst.slot, x, y, 0u), heightAt(inst.slot, x, y, 1u), k);
  v.datumNormal = nu;
  v.patchUv = vec2f(f32(x), f32(y)) / 64.0;
  v.slot = inst.slot;
  v.morph = k;
  return v;
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

// The terrain's unit normal at `patchUv` of `slot`, body-fixed, from the normals atlas at the
// setting's resolution (`draw.atlas`: tiles across a layer, tiles a layer, a tile's stored texels,
// samples a side).
fn patchNormalAt(slot : u32, patchUv : vec2f) -> vec3f {
  let columns = u32(draw.atlas.x);
  let perLayer = u32(draw.atlas.y);
  let tile = u32(draw.atlas.z);
  let samples = u32(draw.atlas.w);
  let layer = slot / perLayer;
  let within = slot - layer * perLayer;
  let corner = vec2u(within % columns, within / columns) * tile + vec2u(1u);
  // Filtered by hand on the decoded vectors: the pairs are discontinuous across the lower
  // hemisphere's fold, along x = 0 and y = 0 for z < 0 (four southern half-meridians of the
  // body-fixed frame), where interpolated pairs would decode to a wrong normal.
  let s = patchUv * f32(samples - 1u);
  let base = min(vec2u(floor(s)), vec2u(samples - 2u));
  let f = s - vec2f(base);
  let at = corner + base;
  let n00 = decodeOctahedral(textureLoad(normals, at, layer, 0).xy);
  let n10 = decodeOctahedral(textureLoad(normals, at + vec2u(1u, 0u), layer, 0).xy);
  let n01 = decodeOctahedral(textureLoad(normals, at + vec2u(0u, 1u), layer, 0).xy);
  let n11 = decodeOctahedral(textureLoad(normals, at + vec2u(1u, 1u), layer, 0).xy);
  return normalize(mix(mix(n00, n10, f.x), mix(n01, n11, f.x), f.y));
}

// `uniforms.ts`' NO_PARENT_SLOT: an instance with no parent to blend to (a root, a parent not
// resident, or one whose slot holds no horizon map; on the lit layout every bake holds both its
// normals and its map, so a parent with either holds the other).
const NO_PARENT_SLOT : u32 = 0xffffffffu;

// A patch's place `patchUv`, (x, y) ÷ 64, as its parent's place, ((q_x, q_y) + patchUv) ÷ 2,
// for the patch's quadrant `quadrant` in its parent (bit 0 q_x = i & 1, bit 1 q_y = j & 1).
fn parentPatchUv(quadrant : u32, patchUv : vec2f) -> vec2f {
  let q = vec2f(f32(quadrant & 1u), f32((quadrant >> 1u) & 1u));
  return (q + patchUv) * 0.5;
}

// The terrain's shading normal at `patchUv` of `slot`, body-fixed: its own tile's normal blended
// to its parent's by the morph factor `morph`, as the height is mixed to the parent's (CDLOD;
// R10.T8.c, decision-r10-t16-leans §16), normalize(mix(own, parent's, k)), the parent's tile read
// at the patch's place in it (`parentPatchUv`) and only where k > 0; the own tile where `parent.x`
// is NO_PARENT_SLOT. So a fine patch at k = 1 shades as its parent along a coarse–fine edge, where
// its coarse neighbour, at k = 0, shades with its own tile of the parent's level, and both read the
// same samples of that level's normal there. `parent` is the instance's parent slot and its
// quadrant (`terrainLit.wgsl`'s `LitVertexOut.parent`); the TypeScript twin is `shadingNormal.ts`.
fn shadingNormalAt(slot : u32, parent : vec2u, patchUv : vec2f, morph : f32) -> vec3f {
  let own = patchNormalAt(slot, patchUv);
  if (parent.x == NO_PARENT_SLOT || !(morph > 0.0)) {
    return own;
  }
  let coarse = patchNormalAt(parent.x, parentPatchUv(parent.y, patchUv));
  return normalize(mix(own, coarse, min(morph, 1.0)));
}
