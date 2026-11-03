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
// `sub.x` is the part of the slot's patch the instance draws, 0 for the whole (`sourceGridOf`;
// uniforms.ts' `subPatchWord`); everything else the instance carries is the slot's patch's. `sub.y`
// to `sub.w` are its fades (R10.T16.i): its edges' and its corners' bytes, then its own fade's
// byte, its rebake's fade's and the slot of the bake it fades from (`fadeFloor`, `rebakeOf`).
// `strides` are its view's stride bands towards strides 2, 4 and 8, metres, at or below 0 for none
// (plan R14, R14.T3.c; `terrainStrides.wgsl`), and bit 18 of `parentQuadrant` holds its camera
// passes at stride 1 (uniforms.ts' `INSTANCE_FINEST`).
struct Instance {
  originM : vec3f,
  slot : u32,
  morphStartM : f32,
  morphEndM : f32,
  parentSlot : u32,
  parentQuadrant : u32,
  sub : vec4u,
  strides : vec4f,
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

// Byte `n` (0 to 3) of a fades word, as a fade of 0 to 1 (coarseFineHolds.ts' `FADE_STEPS`).
fn fadeOf(word : u32, n : u32) -> f32 {
  return f32((word >> (8u * n)) & 0xffu) / 255.0;
}

// The fade floor (plan R10, R10.T16.i) at the vertex whose hold bits are `at` (`holdBitsAt`): the
// largest of the instance's own fade (`sub.w`'s low byte) and the fades of the edges (`sub.y`, byte
// n edge n) and the corner (`sub.z`) it lies on; the TypeScript twin is coarseFineHolds.ts'
// `fadeFloor`.
fn fadeFloor(inst : Instance, at : u32) -> f32 {
  var floor = fadeOf(inst.sub.w, 0u);
  for (var n = 0u; n < 4u; n += 1u) {
    if ((at & (1u << n)) != 0u) {
      floor = max(floor, fadeOf(inst.sub.y, n));
    }
    if ((at & (16u << n)) != 0u) {
      floor = max(floor, fadeOf(inst.sub.z, n));
    }
  }
  return floor;
}

// CDLOD's morph factor at vertex (x, y) from the unmorphed vertex's distance (Design note 6), held
// at the instance's coarse–fine edges and corners (`coarseFineMorph`), raised to its fades' floor
// (`fadeFloor`; R10.T16.i) but where a hold at k = 0 alone covers the vertex, then held at zero near
// every grounded contact and rising to 1 across its ramp: a contact's hold wins, so the ground under
// a craft is its own level's (the forced region's edge, past every ramp, is where its patches meet
// coarser ones). The TypeScript twin of all but the contacts is coarseFineHolds.ts' `fadedMorph`.
fn morphFactor(inst : Instance, fromCameraM : vec3f, x : u32, y : u32) -> f32 {
  let span = inst.morphEndM - inst.morphStartM;
  let cdlod = select(0.0, clamp((length(fromCameraM) - inst.morphStartM) / span, 0.0, 1.0), span > 0.0);
  var k = coarseFineMorph(inst.parentQuadrant, x, y, cdlod);
  let at = holdBitsAt(x, y);
  let toOwnAlone = ((inst.parentQuadrant >> 2u) & 0xffu & at) != 0u && ((inst.parentQuadrant >> 10u) & 0xffu & at) == 0u;
  if (!toOwnAlone) {
    k = max(k, fadeFloor(inst, at));
  }
  for (var i = 0u; i < contacts.count; i += 1u) {
    let c = contacts.items[i];
    let beyond = length(fromCameraM - c.centreM) - c.heldRadiusM;
    let hold = select(select(0.0, 1.0, beyond > 0.0), clamp(beyond / c.rampM, 0.0, 1.0), c.rampM > 0.0);
    k = min(k, hold);
  }
  return k;
}

// The place of an instance's grid vertex (x, y) in its slot's patch's grid, quads from its corner:
// the vertex itself for an instance drawing the whole patch, and for a sub-patch, `levels` deeper
// at (i, j) of the patch's 2^levels × 2^levels parts (bits 0 to 4, 5 to 17 and 18 to 30 of
// `sub.x`), ((i, j) 64 + (x, y)) ÷ 2^levels, exact in f32 (the seamless lane: a missing selected
// patch drawn on its nearest resident ancestor's surface over its own area alone).
fn sourceGridOf(inst : Instance, x : u32, y : u32) -> vec2f {
  let levels = inst.sub.x & 31u;
  let part = vec2u((inst.sub.x >> 5u) & 0x1fffu, (inst.sub.x >> 18u) & 0x1fffu);
  return vec2f(part * 64u + vec2u(x, y)) / f32(1u << levels);
}

// The slot's surface at the place `at` of its grid: its offsets, own and morph, and its heights,
// own and morph, at the triangle's corners holding `at`, mixed by their barycentric weights. The
// mesh splits each quad on its (0, 0)–(1, 1) diagonal (gpu/resources.ts' `patchMeshData`), so the
// point lies on the slot's own triangles, unmorphed and morphed alike: (0, 0), (1, 0), (1, 1) where
// the place's part of its quad has f.x ≥ f.y, else (0, 0), (0, 1), (1, 1).
struct SurfacePoint {
  own : vec3f,
  morph : vec3f,
  ownHeightM : f32,
  morphHeightM : f32,
}

fn surfacePointAt(slot : u32, rec : SlotRecord, at : vec2f) -> SurfacePoint {
  let q = min(vec2u(floor(at)), vec2u(VERTICES_PER_SIDE - 2u));
  let f = at - vec2f(q);
  let below = f.x >= f.y;
  let mid = select(q + vec2u(0u, 1u), q + vec2u(1u, 0u), below);
  let far = q + vec2u(1u);
  let w0 = 1.0 - max(f.x, f.y);
  let w1 = abs(f.x - f.y);
  let w2 = min(f.x, f.y);
  var p : SurfacePoint;
  p.own = w0 * ownOffset(slot, rec, q.x, q.y) + w1 * ownOffset(slot, rec, mid.x, mid.y) + w2 * ownOffset(slot, rec, far.x, far.y);
  p.morph = w0 * morphOffset(slot, rec, q.x, q.y) + w1 * morphOffset(slot, rec, mid.x, mid.y) + w2 * morphOffset(slot, rec, far.x, far.y);
  p.ownHeightM = w0 * heightAt(slot, q.x, q.y, 0u) + w1 * heightAt(slot, mid.x, mid.y, 0u) + w2 * heightAt(slot, far.x, far.y, 0u);
  p.morphHeightM = w0 * heightAt(slot, q.x, q.y, 1u) + w1 * heightAt(slot, mid.x, mid.y, 1u) + w2 * heightAt(slot, far.x, far.y, 1u);
  return p;
}

// The slot's own and morph heights at the place `at` of its grid, as `surfacePointAt` mixes them.
fn surfaceHeightsAt(slot : u32, at : vec2f) -> vec2f {
  let q = min(vec2u(floor(at)), vec2u(VERTICES_PER_SIDE - 2u));
  let f = at - vec2f(q);
  let mid = select(q + vec2u(0u, 1u), q + vec2u(1u, 0u), f.x >= f.y);
  let far = q + vec2u(1u);
  let w0 = 1.0 - max(f.x, f.y);
  let w1 = abs(f.x - f.y);
  let w2 = min(f.x, f.y);
  return vec2f(
    w0 * heightAt(slot, q.x, q.y, 0u) + w1 * heightAt(slot, mid.x, mid.y, 0u) + w2 * heightAt(slot, far.x, far.y, 0u),
    w0 * heightAt(slot, q.x, q.y, 1u) + w1 * heightAt(slot, mid.x, mid.y, 1u) + w2 * heightAt(slot, far.x, far.y, 1u),
  );
}

// The slot a rise from the reference surface names (uniforms.ts' `DATUM_SLOT`): the datum, height
// 0 everywhere (R10.T16.i).
const DATUM_SLOT : u32 = 0xfffu;

// The slot a refine's or a coarsen's fade names (uniforms.ts' `PARENT_MIX_SLOT`; R10.T16.i): the
// instance's parent, its heights read at the instance's place in it (`parentPlaceOf`) and held at
// the datum where the parent's survey held it unsurveyed, so that a change between two bakes that
// disagree (the survey's delta, its edge) fades from the parent's drawn surface.
const PARENT_MIX_SLOT : u32 = 0xffeu;

// The rebake an instance fades from (plan R10, R10.T16.i; uniforms.ts' `fadeWord`): the slot of
// the older bake (bits 20 to 31 of `sub.w`), `DATUM_SLOT` for ground rising out of the reference
// surface or `PARENT_MIX_SLOT` for a refine's or a coarsen's parent, and the fade of its heights,
// normals and maturity towards it (bits 8 to 19, in 4095 steps), 0 with none. A material's own source gives `olderHeightsAt`, the older bake's own and morph heights.
struct Rebake {
  slot : u32,
  fade : f32,
}

// The place `at` of a patch's grid in its parent's grid, quads, by the patch's quadrant there
// (bits 0 and 1 of `quadrant`): (q × 64 + at) ÷ 2.
fn parentPlaceOf(quadrant : u32, at : vec2f) -> vec2f {
  let q = vec2f(f32(quadrant & 1u), f32((quadrant >> 1u) & 1u));
  return (q * 64.0 + at) * 0.5;
}

fn rebakeOf(inst : Instance) -> Rebake {
  var r : Rebake;
  r.slot = inst.sub.w >> 20u;
  r.fade = f32((inst.sub.w >> 8u) & 0xfffu) / 4095.0;
  return r;
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
  // Its place in its slot's patch, (x, y) ÷ 64: a sub-patch's in the slot's patch's grid
  // (`sourceGridOf`), which its maps are read at.
  patchUv : vec2f,
  slot : u32,
  // Its morph factor k, CDLOD's with the coarse–fine and grounded holds (`morphFactor`), which
  // mixes its heights.
  morph : f32,
  // The rebake it fades from (`rebakeOf`), whose slot's normals and maturity its fragments mix in.
  rebakeSlot : u32,
  rebakeFade : f32,
}

// The vertex at `grid` (x, y and 1 for a skirt's) of instance `instance`. A skirt's vertex, and a
// lit mesh's fin's, takes its grid vertex's (x, y), so its morph and holds are its edge vertex's. A
// sub-patch's vertex lies on its slot's patch's own triangles (`surfacePointAt`), at its place in
// that patch's grid (`sourceGridOf`), with its morph factor from its own place and holds: its slot's
// surface exactly at k = 0, and the slot's morphed surface at 1.
fn patchVertexOf(grid : vec3f, instance : u32) -> PatchVertex {
  let inst = instances[instance];
  let rec = slots[inst.slot];
  let x = u32(grid.x);
  let y = u32(grid.y);
  let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
  let origin = draw.offsetFromCameraM + inst.originM;
  var own : vec3f;
  var morph : vec3f;
  var ownHeightM : f32;
  var morphHeightM : f32;
  var nu : vec3f;
  var at = vec2f(f32(x), f32(y));
  if ((inst.sub.x & 31u) == 0u) {
    own = ownOffset(inst.slot, rec, x, y);
    morph = morphOffset(inst.slot, rec, x, y);
    ownHeightM = heightAt(inst.slot, x, y, 0u);
    morphHeightM = heightAt(inst.slot, x, y, 1u);
    nu = rec.nu0 + normalDifference(rec, directionDifference(rec, x, y));
  } else {
    at = sourceGridOf(inst, x, y);
    let p = surfacePointAt(inst.slot, rec, at);
    own = p.own;
    morph = p.morph;
    ownHeightM = p.ownHeightM;
    morphHeightM = p.morphHeightM;
    nu = rec.nu0 + normalDifference(rec, directionDifferenceAt(rec, at));
  }
  // A rebake's fade mixes its heights from the older bake's; a refine's or a coarsen's moves both
  // by its parent's own surface less its morph target, which the fade's floor on k leads to, so
  // that where the bakes agree nothing moves and shared vertices still agree. A vertex's offset is
  // linear in its height, along ν, on either vertex path.
  let rebake = rebakeOf(inst);
  if (rebake.fade > 0.0) {
    var older = vec2f(ownHeightM, morphHeightM);
    if (rebake.slot != PARENT_MIX_SLOT) {
      older = olderHeightsAt(rebake.slot, at);
    } else if (inst.parentSlot != NO_PARENT_SLOT) {
      let parentM = olderHeightsAt(inst.parentSlot, parentPlaceOf(inst.parentQuadrant, at)).x;
      older = vec2f(ownHeightM, morphHeightM) + (parentM - morphHeightM);
    }
    let dOwn = rebake.fade * (older.x - ownHeightM);
    let dMorph = rebake.fade * (older.y - morphHeightM);
    own = own + dOwn * nu;
    morph = morph + dMorph * nu;
    ownHeightM = ownHeightM + dOwn;
    morphHeightM = morphHeightM + dMorph;
  }
  let k = morphFactor(inst, origin + rotation * own, x, y);
  var q = mix(own, morph, k);
  if (grid.z > 0.5) {
    // A skirt hangs from its edge vertex down the spheroid's normal there.
    q = q - rec.skirtDepthM * nu;
  }
  var v : PatchVertex;
  v.fromCameraM = origin + rotation * q;
  v.offsetM = q;
  v.heightM = mix(ownHeightM, morphHeightM, k);
  v.datumNormal = nu;
  v.patchUv = at / 64.0;
  v.slot = inst.slot;
  v.morph = k;
  v.rebakeSlot = rebake.slot;
  v.rebakeFade = rebake.fade;
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
