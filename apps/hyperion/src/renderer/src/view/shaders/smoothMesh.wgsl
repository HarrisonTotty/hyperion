// The mesh regime of a lit body (plan R07, T9; Design notes 2 and 3): R05's patches of the body's
// reference spheroid at zero height, one instanced draw of R05's 65 × 65 mesh with skirts per body,
// writing depth, so that the depth test orders the body against other geometry that writes it.
// Composed after frame.wgsl, litBody.wgsl, bodyDisc.wgsl and R05's patchVertex.wgsl.
//
// The geometry is R05's `FaceDifferences` path at h = 0 (`faceDifferencePosition`): each vertex's
// offset from its patch origin, in the body-fixed axes, from the slot's terms; the origin less the
// camera, along the galactic axes, from the instance (narrowed once from f64 on the CPU), so that
// nothing reaches the GPU in world coordinates (brainstorm, "The floating origin"). R05's CDLOD
// morph takes an odd vertex to its parent's mesh at the far end of its level's band; the skirts
// hang below every edge along the spheroid's normal. The figure is selected to a quarter of a
// pixel (`SMOOTH_MESH_TAU_PX`), so it covers every pixel the spheroid covers wholly.
//
// Each fragment draws its pixel as the disc's first draw does (`disc_pixel` with `edge_pass` 0):
// shaded by the body's record from the pixel's own rays against the analytic spheroid, the same
// shading functions, with the meter class in alpha, and only where the body covers the pixel
// wholly. Elsewhere it is discarded, writing no depth; the limb is the disc's second draw at the
// limb's depth, in the painter's sequence. So a promoted disc and its mesh draw the same light.

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body's record in `discs`.
  disc : u32,
  // The body's first instance in `instances`.
  firstInstance : u32,
  // The body-fixed axes into the galactic axes, as `mat4x4f`.
  bodyRotation : mat4x4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;

// smoothMesh.ts's records: the patches' terms at zero height (R05's `SlotRecord`), and one instance
// a drawn patch, both in the frame's order.
@group(2) @binding(3) var<storage, read> slots : array<SlotRecord>;

// R05's `INSTANCE_RECORD_BYTES`.
struct PatchInstance {
  originM : vec3f,
  slot : u32,
  morphStartM : f32,
  morphEndM : f32,
  pad : vec2f,
}

@group(2) @binding(4) var<storage, read> instances : array<PatchInstance>;

struct MeshVarying {
  @builtin(position) position : vec4f,
}

// The vertex's offset from its patch origin on the spheroid, body-fixed metres: R05's formula at
// h = 0, M (d − d₀).
fn spheroidOffset(rec : SlotRecord, x : u32, y : u32) -> vec3f {
  return faceDifferencePosition(rec, x, y, 0.0);
}

// The morph target at zero height: the parent mesh's point, the mean of the two even neighbours on
// its diagonal at an odd vertex (R05's `morphOffset` on the `FaceDifferences` path, with no
// heights to read).
fn spheroidMorphOffset(rec : SlotRecord, x : u32, y : u32) -> vec3f {
  let oddX = (x & 1u) == 1u;
  let oddY = (y & 1u) == 1u;
  if (rec.straddles != 0u || (!oddX && !oddY)) {
    return spheroidOffset(rec, x, y);
  }
  var a : vec3f;
  var b : vec3f;
  if (oddX && !oddY) {
    a = spheroidOffset(rec, x - 1u, y);
    b = spheroidOffset(rec, x + 1u, y);
  } else if (!oddX) {
    a = spheroidOffset(rec, x, y - 1u);
    b = spheroidOffset(rec, x, y + 1u);
  } else {
    a = spheroidOffset(rec, x - 1u, y - 1u);
    b = spheroidOffset(rec, x + 1u, y + 1u);
  }
  return 0.5 * (a + b);
}

@vertex
fn vertexMain(@location(0) grid : vec3f, @builtin(instance_index) instance : u32) -> MeshVarying {
  let inst = instances[draw.firstInstance + instance];
  let rec = slots[inst.slot];
  let x = u32(grid.x);
  let y = u32(grid.y);
  let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
  let origin = draw.offsetFromCameraM + inst.originM;
  let own = spheroidOffset(rec, x, y);
  // CDLOD's morph factor from the unmorphed vertex's distance (R05 Design note 6); no contact
  // holds it on a smooth figure.
  let span = inst.morphEndM - inst.morphStartM;
  let k = select(0.0, clamp((length(origin + rotation * own) - inst.morphStartM) / span, 0.0, 1.0), span > 0.0);
  var q = mix(own, spheroidMorphOffset(rec, x, y), k);
  if (grid.z > 0.5) {
    // A skirt hangs from its edge vertex down the spheroid's normal there.
    let nu = rec.nu0 + normalDifference(rec, directionDifference(rec, x, y));
    q = q - rec.skirtDepthM * nu;
  }
  var out : MeshVarying;
  out.position = frame.clipProjection * frame.viewRotation * vec4f(origin + rotation * q, 1.0);
  return out;
}

@fragment
fn fragmentMain(v : MeshVarying) -> @location(0) vec4f {
  let pixel = disc_pixel(v.position.xy, 0u);
  if (!pixel.drawn) {
    discard;
  }
  return pixel.colour;
}
