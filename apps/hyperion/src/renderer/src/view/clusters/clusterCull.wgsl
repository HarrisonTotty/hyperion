// The cluster path's cull (plan R14, R14.T1.b; the research `research-terrain-gpu-driven` §5): one
// dispatch a view-frame, a workgroup a consumer instance and a thread a cluster of its page, each
// cluster tested against every pass its instance may enter, given the coarsest template within the
// pass's tolerance, and appended to that template's bucket of the pass (`clusters/buckets.ts`).
// The TypeScript twin is `clusters/cull.ts`' `cullTwin`.
//
// The appends are `atomicAdd`s, so a bucket's order varies from run to run: only depth-only passes
// draw from it, whose result is each texel's nearest depth whatever the order (plan R14's design
// note on compaction). The kernel is `presentation-only`; its counts, integer sums, reach the CPU
// only through `clusters/stats.ts`' `bit-exact` copy.
//
// - _Instances_: the consumer's own records, `counts.y` words each, its first four the origin less
//   the camera (`vec3f`, in the frame the passes' axes are in) and the page; beside them, one
//   `vec2u` an instance: the passes it may enter, a bit a pass (the consumer's own patch-level test,
//   terrain's `castsInto` in `f64`), with bit 31 set where each such pass keeps every cluster of it
//   (terrain's instance whose heights fade from another surface, plan R10, R10.T16.i, which its
//   records do not bound), and its error unit, m, as `f32` bits (terrain's: its level's largest
//   vertex spacing).
// - _A cascade's box_ (kind 1): the cluster's sphere against the light-space box of `castsInto`
//   (`terrainPass.ts`), |v·x_L| ≤ r + ρ, |v·y_L| ≤ r + ρ, v·z_L ≥ −(far + ρ), each widened by
//   ROUNDING_FRACTION of the camera-relative lengths and ROUNDING_M, the `f32` arithmetic's
//   allowance, so that no cluster the `f64` test keeps is refused. A cluster outside x or y draws no
//   texel of the layer, and one past the far plane none either: clipped there, or, under
//   `unclippedDepth`, clamped to the cleared depth, which the comparison leaves as it is. The side
//   towards the light is the instance's own test's: a cluster cut there would change a pancaked
//   caster's texels.
// - _Its receivers' depths_ (T1.c; `clusters/receivers.ts`, where `scalars.w` is 1): a cascade's
//   receivers sample it only at view depths in (fadeStart of the next nearer cascade, its own far
//   end], or below them by falling through a next nearer cascade that does not cover them
//   (`cascadeShadow.wgsl`). A caster's receivers lie within its sphere's depths widened by the
//   pass's reaches, nearer and farther, and across the light within its radius and the pass's
//   reach across. The cluster is refused where all of them lie beyond the far end, or all at or
//   below the near end and inside the next nearer cascade's covered square, each test narrowed by
//   the same allowance, so that it writes no texel any receiver reads.
// - _A camera's frustum_ (kind 2, T1.e): the cluster's sphere against each of the pass's planes,
//   refused where it lies wholly outside one by more than the allowance, and kept where the camera
//   is inside it, as R05's `inFrustum` keeps a patch's sphere; and, where the pass holds one
//   (`scalars.w` above 0, T2.a), against the body's horizon (`cullPass.ts`' `CullHorizon`): refused
//   where it lies wholly beyond the plane through the tangent circle and wholly inside the cone,
//   a sin α − d cos α > ρ, a its distance along the cone's axis and d from it, d's square widened by
//   HORIZON_ROUNDING of |S|² for its `f32` rounding.
// - _An ordered camera pass_ (kind 3, T2.a, the lit pass): the camera's tests, but each cluster's
//   template, or NO_MARK where it is refused or its instance does not enter the pass, is written to
//   `marks`, one word a cluster of every instance, for the stable compaction to write the pass's
//   buckets in the draw set's order (`compact.wgsl`); nothing is appended for it.
// - _The two-pass occlusion_ (T3.d; `clusters/hzb.ts`; the occlusion's variant alone, `cull.ts`'
//   CLUSTER_CULL_OCCLUSION_KERNEL, which composes `clusterOcclusion.wgsl`'s bindings and tests
//   below the bindings here; the plain kernel's stand-ins see every cluster, refuse none and write
//   nothing), an ordered pass of either of two kinds:
//   the early pass (kind 4) marks only the clusters its camera's tests keep that were visible last
//   frame, their bits set in `history` (one a cluster of every slot, a slot's ⌈clusters ÷ 32⌉
//   words, cleared when the slot is filled); the late pass (kind 5), a dispatch of its own after
//   the first raster and the pyramid's build, marks those its camera's tests keep that the early
//   pass did not mark, as `marks` holds them, and that the pyramid does not refuse (`hzb_refuses`),
//   and writes each cluster's bit, whether it was kept and not refused (cleared for an instance that
//   does not enter it). An instance that is a sub-patch of its slot (its `sub.x`, word 8, not 0 in
//   its low five bits) shares its slot's bits with others, so it is never marked early and writes
//   no bit; one kept whole is never refused.
// - _A stand-in's mask_ (plan R14, Design note 19; word 8's bit 31): an instance drawing its slot's
//   patch's own blocks over its area keeps those alone, by their spheres, and its strips along the
//   set's sides whatever theirs, which do not bound them (`mask_cluster`; `clusters/terrain.ts`'
//   `terrainMaskCluster`).
// - _An edge bake's stand-in_ (R14.T3.c's edge stand-ins; its cull input's bits 27 to 30,
//   `clusters/terrain.ts`' TERRAIN_INPUT_STAND_IN_BITS): one whose area lies on blocks all opaque
//   in its slot's records (INPUT_SURVEYED) takes its blocks and strips opaque and real, whatever
//   their records say, and none of its fin clusters, as an interior bake's stand-in; a mask one level
//   down across the survey's edge (INPUT_EDGE_MASK) its blocks by their records, its strips real and
//   may-discard, whose records, its slot's edge strips', do not class them, and its quadrant's six
//   runs by their records and spheres, a side run left to its sibling where its bit says so
//   (INPUT_COLUMN_RUN_SHARED, INPUT_ROW_RUN_SHARED) (`record_class`, `mask_run`;
//   `terrainRecordClass`, `terrainMaskRuns`).
// - _The DAG cut_ (R14.T4.b, a record with the flags' bit 4): kept only where its own error, `f32`
//   in its error bytes' first word, is within the pass's tolerance at its group's sphere, the
//   record's own, and its parent's, in the second, is not at its parent group's, its `lods` entry
//   (`clusters/record.ts`' `ClusterLod`): for a camera pass ε ≤ τ max(|S| − ρ, 0), τ the pass's
//   tolerance in rad, the projected error ε ÷ d of `screenSpaceErrorPx` at the sphere's nearest
//   point, so that inside a sphere only an error of 0 is within it; for a cascade ε ≤ τ, its texel.
//   A cluster's parent test is the same arithmetic on the same bits as its parent group's own test
//   (one origin for every page of a mesh, `tools/meshbuild`), so that the cut is consistent: of a
//   leaf's chain of groups exactly one is drawn (Karis 2021, SIGGRAPH Advances, "A Deep Dive into
//   Nanite Virtualized Geometry").
// - _A record of no cluster_ (R14.T4.b): a negative radius, a page's room past its clusters, which
//   every pass refuses.
// - _The stride_: for a cascade the coarsest s = 2^l, l up to the pass's cap, with s × the unit
//   within the pass's tolerance, in `f32` as the twin forms it; s = 1 where none is, and for a DAG
//   record. For a camera pass (T3.c; `clusters/strides.ts`), up to its cap, the coarsest whose
//   every band below it the cluster's nearest point is past by (1 + β), its bands the largest of
//   the vertices it drops (the shared bands, `cluster_bands`): a block's own need, its neighbours'
//   on its sides within its page, from their records' measured errors under its instance's scale
//   (terrain's `Instance.strides`, words 12 and 13, read where an instance has 16 words or more),
//   and the shared bands of its patch's edges along it (`Instance.edgeBands`, words 20 to 67, half
//   floats of kilometres, where it has 68 or more), a strip's edge's along its block, so that every
//   vertex the stride drops is wholly moved by the stride morph (`terrainStrides.wgsl`); a
//   stand-in's mask's (its sub-patch word's bit 31) under its slot's patch's scale, its set's
//   sides in its edges' place, its blocks sharing its set's needs alone and each strip its block's
//   at the side it hangs from (`mask_bands`), by that block's record, whose sphere holds the
//   side's vertices, not its own on the patch's edge, and β its own, STRIDE_MASK_BAND_FRACTION;
//   s = 1 where the instance's quadrant word has bit 18 (the forced region's) and where it is kept
//   whole (its heights fade from another surface, which its records' spheres do not bound, so that
//   its vertices may stand nearer than its spheres), and at most 4 for a block or a strip on an
//   edge whose coarse–fine hold at k = 0 is set (bits 2 to 5 of that word: a finer patch touches
//   it), whose level-2 vertices stay, or fading in or out (its byte of the instance's shares word,
//   `Instance.holdFades`, word 14, not 0), whose level-2 vertices move. A block or strip of an
//   instance whose quadrant word has bit 19 (a sub-patch on its own grid over its page's
//   triangles, `strides.ts`' INSTANCE_SUB_GRID_STRIDES), kept whole or not, takes the pass's cap
//   up to its sub-patch word's levels wherever it is: each of that stride's triangles lies in one
//   of its page's, so that it draws the surface its stride-1 triangles do.
// - _Strides past τ′_ (T3.e; `clusters/overCount.ts`): where `shapes.w` is not 0, each cluster a
//   camera pass draws at a stride above 1 (not by a may-discard template, which draws it at
//   stride 1 whatever its bands give: R14.T3.c's edge stand-ins' finding) whose own need at it,
//   (E_L + e_s) ÷ τ_rad under its instance's scale (τ the slewed tolerance its stride was chosen
//   at), lies beyond its sphere's nearest point d × τ′ ÷ τ, τ′ the pass's tolerance, rad
//   (`scalars.x`, 0 for none), is counted
//   into `args` at word `shapes.w` + the pass's index among its buckets (its first bucket ÷ the
//   templates a pass has): an `atomicAdd`, so that the count is the same whatever the order.
// - _The bucket_: the record's shape (its flags' bits 0–3) less the consumer's first shape
//   (`shapes.x`) × STRIDE_LEVELS + l, from the pass's first bucket; or, for a record that may
//   discard (its flags' bit 5) of a shape with a may-discard template (`shapes.y` and `shapes.z`,
//   a byte a shape code 0 to 7, NO_DISCARD_TEMPLATE for none; R14.T1.g), that template, at stride
//   1. The entry,
//   (instance << 8) | cluster, at its template's list offset in the pass plus the place the append
//   took. An append past a template's capacity is dropped: none is, the lists holding every
//   cluster of every instance once a pass.

struct ClusterRecord {
  centre : vec3f,
  radius : f32,
  errors : vec2u,
  templateRef : u32,
  flags : u32,
}

struct CullPass {
  header : vec4u,
  scalars : vec4f,
  axisX : vec4f,
  axisY : vec4f,
  axisZ : vec4f,
  centre : vec4f,
  receivers : vec4f,
  covered : vec4f,
}

struct CullParams {
  rotation0 : vec4f,
  rotation1 : vec4f,
  rotation2 : vec4f,
  counts : vec4u,
  viewAxis : vec3f,
  templates : u32,
  listOffsets0 : vec4u,
  listOffsets1 : vec4u,
  listOffsets2 : vec4u,
  listOffsets3 : vec4u,
  capacities0 : vec4u,
  capacities1 : vec4u,
  capacities2 : vec4u,
  capacities3 : vec4u,
  shapes : vec4u,
}

// `cullPass.ts`' MAX_CULL_PASSES, a pass's kinds and the strides' levels.
const MAX_CULL_PASSES : u32 = 8u;
const PASS_CASCADE : u32 = 1u;
const PASS_CAMERA : u32 = 2u;
const PASS_CAMERA_ORDERED : u32 = 3u;
// The two-pass occlusion's ordered passes (`cullPass.ts`' `CameraCullPass.occlusion`; T3.d).
const PASS_CAMERA_EARLY : u32 = 4u;
const PASS_CAMERA_LATE : u32 = 5u;
// A cluster an ordered pass refuses (`cull.ts`' NO_MARK).
const NO_MARK : u32 = 0xffffffffu;
// The cull input's bit of an instance each pass it enters keeps whole (`cull.ts`' CULL_INPUT_WHOLE).
const INPUT_WHOLE : u32 = 0x80000000u;
// Its bits of an edge bake's stand-in (`clusters/terrain.ts`' TERRAIN_INPUT_SURVEYED,
// TERRAIN_INPUT_EDGE_MASK, TERRAIN_INPUT_COLUMN_RUN_SHARED and TERRAIN_INPUT_ROW_RUN_SHARED).
const INPUT_SURVEYED : u32 = 0x40000000u;
const INPUT_EDGE_MASK : u32 = 0x20000000u;
const INPUT_COLUMN_RUN_SHARED : u32 = 0x10000000u;
const INPUT_ROW_RUN_SHARED : u32 = 0x08000000u;
// The flags' DAG-cut and may-discard bits (`record.ts`' CLUSTER_FLAG_DAG and
// CLUSTER_FLAG_MAY_DISCARD), and a shape with no may-discard template (`cullPass.ts`').
const CLUSTER_FLAG_DAG : u32 = 16u;
const CLUSTER_FLAG_MAY_DISCARD : u32 = 32u;
// `record.ts`' CLUSTER_FLAG_MEASURED: a strided record whose errors are measured (T3.b).
const CLUSTER_FLAG_MEASURED : u32 = 64u;
const NO_DISCARD_TEMPLATE : u32 = 0xffu;
const STRIDE_LEVELS : u32 = 4u;
// `strides.ts`' STRIDE_BAND_FRACTION, the quadrant word's bit by which a camera pass draws an
// instance at stride 1 (uniforms.ts' `INSTANCE_FINEST`), and its bit by which it draws a sub-patch
// on its own grid at its cap (`INSTANCE_SUB_GRID_STRIDES`).
const STRIDE_BAND_FRACTION : f32 = 0.002;
// `strides.ts`' STRIDE_MASK_BAND_FRACTION: a stand-in's mask's β, its own.
const STRIDE_MASK_BAND_FRACTION : f32 = 0.15;
const INSTANCE_FINEST : u32 = 0x40000u;
const INSTANCE_SUB_GRID_STRIDES : u32 = 0x80000u;
// The threads of a workgroup, each taking every 64th cluster of its instance's page.
const CULL_GROUP : u32 = 64u;
// The `f32` allowance: per metre of the camera-relative lengths a test forms (some sixteen ulps,
// 2⁻²⁴ each), and a metre.
const ROUNDING_FRACTION : f32 = 1e-6;
const ROUNDING_M : f32 = 1.0;
// The receivers' tests' least allowance, m: their lengths' ulps are the fraction's.
const RECEIVER_ROUNDING_M : f32 = 0.01;
// The horizon test's widening of d², per |S|² (`cull.ts`' HORIZON_ROUNDING).
const HORIZON_ROUNDING : f32 = 1e-6;

@group(0) @binding(0) var<uniform> params : CullParams;
@group(0) @binding(1) var<uniform> passes : array<CullPass, 8>;
@group(0) @binding(2) var<storage, read> instance_words : array<u32>;
@group(0) @binding(3) var<storage, read> cull_inputs : array<vec2u>;
@group(0) @binding(4) var<storage, read> records : array<ClusterRecord>;
@group(0) @binding(5) var<storage, read_write> lists : array<u32>;
@group(0) @binding(6) var<storage, read_write> args : array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> marks : array<u32>;
// Composed here: the plain kernel's (`cull.ts`' NO_OCCLUSION) one parent group's LOD sphere a
// record (`record.ts`' `ClusterLod`, binding 8, read by a DAG record alone through `lod_of`) and the
// occlusion's stand-ins, or the occlusion's variant's bindings and tests (`clusterOcclusion.wgsl`),
// whose consumer has no DAG record.
//OCCLUSION//

// A record's centre in the instances' frame, turned from its page's axes.
fn page_to_instance(v : vec3f) -> vec3f {
  return params.rotation0.xyz * v.x + params.rotation1.xyz * v.y + params.rotation2.xyz * v.z;
}

// Whether the sphere at `centre`, camera-relative, of radius `radius` may draw into cascade box
// `pass` (see the module's head).
fn cascade_keeps(cull_pass : CullPass, centre : vec3f, radius : f32) -> bool {
  let v = vec3f(dot(centre, cull_pass.axisX.xyz), dot(centre, cull_pass.axisY.xyz), dot(centre, cull_pass.axisZ.xyz)) - cull_pass.centre.xyz;
  let slack = ROUNDING_FRACTION * (length(centre) + length(cull_pass.centre.xyz) + cull_pass.scalars.y) + ROUNDING_M;
  let reach = cull_pass.scalars.y + radius + slack;
  return abs(v.x) <= reach && abs(v.y) <= reach && v.z >= -(cull_pass.scalars.z + radius + slack);
}

// Whether no receiver of cascade `pass` can read a texel the sphere at `centre`, camera-relative,
// of radius `radius` writes (see the module's head); never where the pass's receivers' cull is off.
fn receivers_refuse(cull_pass : CullPass, centre : vec3f, radius : f32) -> bool {
  if (cull_pass.scalars.w == 0.0) {
    return false;
  }
  let depths = cull_pass.receivers;
  let below = cull_pass.covered;
  let d = dot(centre, params.viewAxis);
  let slack = ROUNDING_FRACTION * (length(centre) + abs(depths.x) + abs(depths.y) + length(below.xy) + abs(below.z)) + RECEIVER_ROUNDING_M;
  if (d - radius - depths.z - slack > depths.y) {
    return true;
  }
  if (below.z <= 0.0 || d + radius + depths.w + slack > depths.x) {
    return false;
  }
  let across = vec2f(dot(centre, cull_pass.axisX.xyz), dot(centre, cull_pass.axisY.xyz)) - below.xy;
  let reach = radius + below.w + slack;
  return abs(across.x) + reach <= below.z && abs(across.y) + reach <= below.z;
}

// A camera pass's plane `i`, its inward normal and its offset.
fn camera_plane(cull_pass : CullPass, i : u32) -> vec4f {
  switch i {
    case 0u: {
      return cull_pass.axisX;
    }
    case 1u: {
      return cull_pass.axisY;
    }
    case 2u: {
      return cull_pass.axisZ;
    }
    case 3u: {
      return cull_pass.centre;
    }
    case 4u: {
      return cull_pass.receivers;
    }
    default: {
      return cull_pass.covered;
    }
  }
}

// Whether the sphere at `centre`, camera-relative, of radius `radius` lies wholly behind camera pass
// `pass`'s horizon (see the module's head); never where the pass holds none.
fn horizon_refuses(cull_pass : CullPass, centre : vec3f, radius : f32) -> bool {
  let sin_a = cull_pass.scalars.w;
  if (sin_a <= 0.0) {
    return false;
  }
  let span = dot(centre, centre);
  let a = dot(centre, cull_pass.covered.xyz);
  let slack = ROUNDING_FRACTION * (sqrt(span) + cull_pass.scalars.z) + ROUNDING_M;
  if (a - radius - slack <= cull_pass.scalars.z) {
    return false;
  }
  let d = sqrt(max(span - a * a, 0.0) + HORIZON_ROUNDING * span);
  return a * sin_a - d * cull_pass.covered.w > radius + slack;
}

// Whether the sphere at `centre`, camera-relative, of radius `radius` may show in camera pass
// `pass`'s frustum, above its horizon (see the module's head).
fn camera_keeps(cull_pass : CullPass, centre : vec3f, radius : f32) -> bool {
  let slack = ROUNDING_FRACTION * length(centre) + ROUNDING_M;
  if (dot(centre, centre) <= (radius + slack) * (radius + slack)) {
    return true;
  }
  let planes = min(u32(cull_pass.scalars.y), 6u);
  for (var i = 0u; i < planes; i = i + 1u) {
    let plane = camera_plane(cull_pass, i);
    if (dot(plane.xyz, centre) + plane.w < -(radius + slack)) {
      return false;
    }
  }
  return !horizon_refuses(cull_pass, centre, radius);
}

// Whether the sphere may draw into pass `pass` where anything reads it, by the pass's kind.
fn pass_keeps(cull_pass : CullPass, centre : vec3f, radius : f32) -> bool {
  switch cull_pass.header.x {
    case PASS_CASCADE: {
      return cascade_keeps(cull_pass, centre, radius) && !receivers_refuse(cull_pass, centre, radius);
    }
    case PASS_CAMERA, PASS_CAMERA_ORDERED, PASS_CAMERA_EARLY, PASS_CAMERA_LATE: {
      return camera_keeps(cull_pass, centre, radius);
    }
    default: {
      return false;
    }
  }
}

// The distance from the camera to the sphere at `centre`, camera-relative, of radius `radius`: 0
// inside it.
fn group_distance(centre : vec3f, radius : f32) -> f32 {
  return max(length(centre) - radius, 0.0);
}

// Whether a DAG record is drawn at the pass's tolerance (see the module's head): its own error
// within it at its group's sphere (`centre`, `radius`), its parent's not at its parent group's.
fn dag_drawn(cull_pass : CullPass, record : ClusterRecord, centre : vec3f, parent_centre : vec3f, parent_radius : f32) -> bool {
  let own = bitcast<f32>(record.errors.x);
  let parent = bitcast<f32>(record.errors.y);
  let tolerance = cull_pass.scalars.x;
  if (cull_pass.header.x == PASS_CASCADE) {
    return own <= tolerance && parent > tolerance;
  }
  return own <= tolerance * group_distance(centre, record.radius) && parent > tolerance * group_distance(parent_centre, parent_radius);
}

// The coarsest stride's level, up to the pass's cap, whose 2^l × `unit` is within its tolerance.
fn stride_level(cull_pass : CullPass, unit : f32) -> u32 {
  var level = 0u;
  for (var l = 1u; l <= cull_pass.header.y && l < STRIDE_LEVELS; l = l + 1u) {
    if (f32(1u << l) * unit <= cull_pass.scalars.x) {
      level = l;
    }
  }
  return level;
}

// The edges of a terrain block or strip, bits 0 to 3 for x = 0, x = 64, y = 0 and y = 64, from its
// template reference (`record.ts`), 0 for any other shape.
fn record_edges(shape : u32, template_ref : u32) -> u32 {
  if (shape == 0u) {
    let x0 = template_ref & 0xffu;
    let y0 = (template_ref >> 8u) & 0xffu;
    return select(0u, 1u, x0 == 0u) | select(0u, 2u, x0 == 56u) | select(0u, 4u, y0 == 0u) | select(0u, 8u, y0 == 56u);
  }
  if (shape == 1u) {
    // Edge 0 y = 0, 1 x = 64, 2 y = 64, 3 x = 0 (`gpu/resources.ts`' numbering).
    switch (template_ref >> 16u) & 3u {
      case 0u: {
        return 4u;
      }
      case 1u: {
        return 2u;
      }
      case 2u: {
        return 8u;
      }
      default: {
        return 1u;
      }
    }
  }
  return 0u;
}

// A record's measured error towards stride 2^(level + 1), m (`record.ts`' `strideErrorBits`: e₁
// and e₂, e₄ and e₈ in half floats); +∞ where it is not measured.
fn record_error(record : ClusterRecord, level : u32) -> f32 {
  if ((record.flags & CLUSTER_FLAG_MEASURED) == 0u) {
    return 3.4e38;
  }
  switch level {
    case 0u: {
      return unpack2x16float(record.errors.x).y;
    }
    case 1u: {
      return unpack2x16float(record.errors.y).x;
    }
    default: {
      return unpack2x16float(record.errors.y).y;
    }
  }
}

// A cluster's need towards a stride under `scale` (E_L ÷ τ′_rad, 1 ÷ τ′_rad) of its error `e`
// (`strides.ts`' `strideNeedM`); −1 for none.
fn stride_need(scale : vec2f, e : f32) -> f32 {
  if (scale.y <= 0.0 || !(e >= 0.0) || e > 65504.0) {
    return -1.0;
  }
  return scale.x + e * scale.y;
}

// Bands `a` and `b` shared, stride by stride: the larger, none where either is none (`strides.ts`'
// `sharedStrideBandsM`).
fn shared_bands(a : vec3f, b : vec3f) -> vec3f {
  return select(vec3f(-1.0), max(a, b), (a > vec3f(0.0)) & (b > vec3f(0.0)));
}

// Block `block`'s needs of page `page` under `scale`.
fn block_needs(scale : vec2f, page : u32, block : u32) -> vec3f {
  let record = records[page * params.counts.z + block];
  return vec3f(
    stride_need(scale, record_error(record, 0u)),
    stride_need(scale, record_error(record, 1u)),
    stride_need(scale, record_error(record, 2u)),
  );
}

// The shared bands of edge `edge` (`EDGES`' order) at block `t` along it, of the instance whose
// words begin at `base` (`strides.ts`' `edgeBandIndex`), half floats of kilometres; none where the
// record holds none, its edges' vertices held.
fn edge_bands(base : u32, edge : u32, t : u32) -> vec3f {
  if (params.counts.y < 68u) {
    return vec3f(-1.0);
  }
  var out = vec3f(-1.0);
  for (var l = 0u; l < 3u; l = l + 1u) {
    let h = (8u * edge + t) * 3u + l;
    let km = unpack2x16float(instance_words[base + 20u + h / 2u])[h % 2u];
    out[l] = select(-1.0, km * 1000.0, km > 0.0);
  }
  return out;
}

// The bands of cluster `c` of the instance whose words begin at `base`, of page `page`, under
// `scale` (`strides.ts`' `clusterStrideBandsM`): the largest band of the vertices it drops; none
// for no scale; a mask's (sub-patch word `sub`), its set's sides in its edges' place
// (`mask_bands`).
fn cluster_bands(scale : vec2f, page : u32, c : u32, base : u32, sub : u32) -> vec3f {
  if (scale.y <= 0.0 || c >= 96u || c >= params.counts.z) {
    return vec3f(-1.0);
  }
  if ((sub & SUB_PATCH_MASK) != 0u) {
    return mask_bands(scale, page, c, sub);
  }
  if (c >= 64u) {
    // Strip m of edge e (`gpu/resources.ts`' numbering, anticlockwise) lies along one block.
    let e = (c - 64u) / 8u;
    let m = (c - 64u) % 8u;
    switch e {
      case 0u: {
        return edge_bands(base, 2u, m);
      }
      case 1u: {
        return edge_bands(base, 1u, m);
      }
      case 2u: {
        return edge_bands(base, 3u, 7u - m);
      }
      default: {
        return edge_bands(base, 0u, 7u - m);
      }
    }
  }
  let bx = c % 8u;
  let by = c / 8u;
  var bands = block_needs(scale, page, c);
  if (bx > 0u) {
    bands = shared_bands(bands, block_needs(scale, page, c - 1u));
  } else {
    bands = shared_bands(bands, edge_bands(base, 0u, by));
  }
  if (bx < 7u) {
    bands = shared_bands(bands, block_needs(scale, page, c + 1u));
  } else {
    bands = shared_bands(bands, edge_bands(base, 1u, by));
  }
  if (by > 0u) {
    bands = shared_bands(bands, block_needs(scale, page, c - 8u));
  } else {
    bands = shared_bands(bands, edge_bands(base, 2u, bx));
  }
  if (by < 7u) {
    bands = shared_bands(bands, block_needs(scale, page, c + 8u));
  } else {
    bands = shared_bands(bands, edge_bands(base, 3u, bx));
  }
  return bands;
}

// `clusters/terrain.ts`' TERRAIN_SUB_PATCH_MASK: a sub-patch word's bit of a stand-in's mask.
const SUB_PATCH_MASK : u32 = 0x80000000u;

// How an instance of sub-patch word `sub` and cull input `bits` draws its page's cluster `c`
// (`clusters/terrain.ts`' `terrainMaskCluster`): 1 by its sphere, for every cluster of one that is
// not a mask, a mask's own blocks and an edge mask's runs (`mask_run`); 2 whatever its sphere, for
// a mask's strips along its set's sides; 0 for any other.
fn mask_cluster(sub : u32, c : u32, bits : u32) -> u32 {
  if ((sub & SUB_PATCH_MASK) == 0u) {
    return 1u;
  }
  let side = 8u >> (sub & 31u);
  let b0 = vec2u((sub >> 5u) & 0x1fffu, (sub >> 18u) & 0x1fffu) * side;
  if (c < 64u) {
    let b = vec2u(c % 8u, c / 8u);
    return select(0u, 1u, all(b >= b0) && all(b < b0 + vec2u(side)));
  }
  if (c < 96u) {
    return select(0u, 2u, (c - 64u) % 8u < side);
  }
  return select(0u, 1u, mask_run(sub, c, bits));
}

// Whether fin cluster `c` (96 up) is one of the six runs of its quadrant (i, j) that an edge mask one
// level down of sub-patch word `sub` and cull input `bits` draws (`clusters/terrain.ts`'
// `terrainMaskRuns`): its two interior fins, its two side fins on column and row 32 but where its
// bit leaves one to its sibling, and its two heads on its slot's patch's edges.
fn mask_run(sub : u32, c : u32, bits : u32) -> bool {
  if ((bits & INPUT_EDGE_MASK) == 0u || (sub & 31u) != 1u) {
    return false;
  }
  let q = vec2u((sub >> 5u) & 1u, (sub >> 18u) & 1u);
  if (c == 106u + q.y) {
    return (bits & INPUT_COLUMN_RUN_SHARED) == 0u;
  }
  if (c == 112u + q.x) {
    return (bits & INPUT_ROW_RUN_SHARED) == 0u;
  }
  let head0 = select(100u + (1u - q.x), 96u + q.x, q.y == 0u);
  let head1 = select(102u + (1u - q.y), 98u + q.y, q.x == 1u);
  return c == 104u + 4u * q.x + q.y || c == 110u + 4u * q.y + q.x || c == head0 || c == head1;
}

// Whether record `record` of cluster `c` is a cluster at all, and its flags as the cull takes them,
// of an instance of cull input `bits` (`clusters/terrain.ts`' `terrainRecordClass`): an edge bake's
// stand-in on opaque blocks its blocks and strips real and opaque and no fin cluster; an edge
// mask's strip real and may-discard; else the record's own.
struct RecordClass {
  real : bool,
  flags : u32,
}

fn record_class(record : ClusterRecord, c : u32, bits : u32) -> RecordClass {
  if ((bits & INPUT_SURVEYED) != 0u) {
    return RecordClass(c < 96u, record.flags & ~CLUSTER_FLAG_MAY_DISCARD);
  }
  if ((bits & INPUT_EDGE_MASK) != 0u && c >= 64u && c < 96u) {
    return RecordClass(true, record.flags | CLUSTER_FLAG_MAY_DISCARD);
  }
  return RecordClass(record.radius >= 0.0, record.flags);
}

// The block of its set a stand-in's mask's strip `c` (64 to 95) of sub-patch word `sub` hangs
// from (`strides.ts`' `maskStripBlock`): strip m of its set's side e, anticlockwise as
// `terrain_mask_edge_vertex` lays it, at that side's m-th block; NO_SIDE_BLOCK for any other.
const NO_SIDE_BLOCK : u32 = 0xffffffffu;

fn mask_side_block(sub : u32, c : u32) -> u32 {
  let side = 8u >> (sub & 31u);
  let m = (c - 64u) % 8u;
  if ((sub & SUB_PATCH_MASK) == 0u || c < 64u || c >= 96u || m >= side) {
    return NO_SIDE_BLOCK;
  }
  let b0 = vec2u((sub >> 5u) & 0x1fffu, (sub >> 18u) & 0x1fffu) * side;
  let last = side - 1u;
  var b = b0;
  switch (c - 64u) / 8u {
    case 0u: {
      b = b0 + vec2u(m, 0u);
    }
    case 1u: {
      b = b0 + vec2u(last, m);
    }
    case 2u: {
      b = b0 + vec2u(last - m, last);
    }
    default: {
      b = b0 + vec2u(0u, last - m);
    }
  }
  return b.y * 8u + b.x;
}

// The bands of cluster `c` (below 96) of a stand-in's mask of sub-patch word `sub`, of page
// `page`, under `scale` (`strides.ts`' `maskClusterBandsM`): a block of its set its own need's
// shared with those of its set's blocks on its sides; strip m of its set's side e, anticlockwise
// as `terrain_mask_edge_vertex` lays it, its block's there; none for any other.
fn mask_bands(scale : vec2f, page : u32, c : u32, sub : u32) -> vec3f {
  let side = 8u >> (sub & 31u);
  let b0 = vec2i(vec2u((sub >> 5u) & 0x1fffu, (sub >> 18u) & 0x1fffu) * side);
  let hi = b0 + vec2i(i32(side) - 1);
  let block = select(c, mask_side_block(sub, c), c >= 64u);
  let b = vec2i(i32(block % 8u), i32(block / 8u));
  if (block >= 64u || any(b < b0) || any(b > hi)) {
    return vec3f(-1.0);
  }
  var bands = block_needs(scale, page, block);
  if (b.x > b0.x) {
    bands = shared_bands(bands, block_needs(scale, page, block - 1u));
  }
  if (b.x < hi.x) {
    bands = shared_bands(bands, block_needs(scale, page, block + 1u));
  }
  if (b.y > b0.y) {
    bands = shared_bands(bands, block_needs(scale, page, block - 8u));
  }
  if (b.y < hi.y) {
    bands = shared_bands(bands, block_needs(scale, page, block + 8u));
  }
  return bands;
}

// The edges whose hold at k = 0 fades in or out by an instance's shares word (`strides.ts`'
// `strideFadingEdges`): those whose byte is not 0.
fn fading_edges(hold_fades : u32) -> u32 {
  var edges = 0u;
  for (var e = 0u; e < 4u; e = e + 1u) {
    edges = edges | select(0u, 1u << e, ((hold_fades >> (8u * e)) & 0xffu) != 0u);
  }
  return edges;
}

// A camera pass's stride's level for a strided record (see the module's head; `strides.ts`'
// `cameraStrideLevel`): `bands` the record's (`cluster_bands`), `quadrant` and `hold_fades` its
// instance's quadrant and shares words, `distance` the record's sphere's nearest point, `fraction`
// its instance's β (a mask's STRIDE_MASK_BAND_FRACTION).
fn camera_stride_level(cull_pass : CullPass, record : ClusterRecord, bands : vec3f, quadrant : u32, hold_fades : u32, distance : f32, fraction : f32) -> u32 {
  if ((quadrant & INSTANCE_FINEST) != 0u) {
    return 0u;
  }
  let edges = record_edges(record.flags & 0xfu, record.templateRef);
  var cap = min(cull_pass.header.y, STRIDE_LEVELS - 1u);
  if ((edges & (((quadrant >> 2u) & 0xfu) | fading_edges(hold_fades))) != 0u) {
    cap = min(cap, 2u);
  }
  var level = 0u;
  for (var l = 1u; l <= cap; l = l + 1u) {
    let band = bands[l - 1u];
    if (band <= 0.0 || distance < (1.0 + fraction) * band) {
      break;
    }
    level = l;
  }
  return level;
}

// A camera pass's stride's level for a block or strip of a sub-patch on its own grid (see the
// module's head; `cull.ts`' `cameraPassStrideLevel`), `shape` its record's, `quadrant` and `sub`
// its instance's quadrant and sub-patch words: the pass's cap up to the sub-patch's levels; 0 in
// the forced region and for any other shape.
fn sub_grid_stride_level(cull_pass : CullPass, shape : u32, quadrant : u32, sub : u32) -> u32 {
  if ((quadrant & INSTANCE_FINEST) != 0u || shape > 1u) {
    return 0u;
  }
  return min(min(cull_pass.header.y, STRIDE_LEVELS - 1u), sub & 31u);
}

// Whether a camera pass draws a strided record at stride level `level` past its τ′ (see the
// module's head), `distance` the record's sphere's nearest point, `scale` its instance's.
fn drawn_over(cull_pass : CullPass, record : ClusterRecord, scale : vec2f, level : u32, distance : f32) -> bool {
  let tolerance = cull_pass.scalars.x;
  if (level == 0u || tolerance <= 0.0 || scale.y <= 0.0 || cull_pass.header.x == PASS_CASCADE) {
    return false;
  }
  return stride_need(scale, record_error(record, level - 1u)) > distance * tolerance * scale.y;
}

// Counts a cluster drawn past its pass's τ′ (see the module's head), where the dispatch counts.
fn note_over(cull_pass : CullPass, drawn : bool, over : bool) {
  if (drawn && over && params.shapes.w > 0u) {
    atomicAdd(&args[params.shapes.w + cull_pass.header.z / max(params.templates, 1u)], 1u);
  }
}

// A may-discard record's shape's may-discard template, NO_DISCARD_TEMPLATE for a record that may not
// discard or a shape with none (see the module's head).
fn discard_template(shape : u32, flags : u32) -> u32 {
  if ((flags & CLUSTER_FLAG_MAY_DISCARD) == 0u || shape >= 8u) {
    return NO_DISCARD_TEMPLATE;
  }
  let word = select(params.shapes.y, params.shapes.z, shape >= 4u);
  return (word >> (8u * (shape % 4u))) & 0xffu;
}

// A record's template in the pass (see the module's head) at stride level `level`, the pass's for
// a strided one, 0 for a DAG one's; a may-discard record's shape's may-discard template where it
// has one (`discard_template`).
fn template_of(cull_pass : CullPass, shape : u32, flags : u32, level : u32) -> u32 {
  let discarding = discard_template(shape, flags);
  if (discarding != NO_DISCARD_TEMPLATE) {
    return discarding;
  }
  let dag = (flags & CLUSTER_FLAG_DAG) != 0u;
  return (shape - params.shapes.x) * STRIDE_LEVELS + select(level, 0u, dag);
}

// Word `t % 4` of the `t / 4`-th of four `vec4u`, a template's of sixteen.
fn word_of(a : vec4u, b : vec4u, c : vec4u, d : vec4u, t : u32) -> u32 {
  switch t / 4u {
    case 0u: {
      return a[t % 4u];
    }
    case 1u: {
      return b[t % 4u];
    }
    case 2u: {
      return c[t % 4u];
    }
    default: {
      return d[t % 4u];
    }
  }
}

fn template_offset(t : u32) -> u32 {
  return word_of(params.listOffsets0, params.listOffsets1, params.listOffsets2, params.listOffsets3, t);
}

fn template_capacity(t : u32) -> u32 {
  return word_of(params.capacities0, params.capacities1, params.capacities2, params.capacities3, t);
}

@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group : vec3u, @builtin(local_invocation_id) local : vec3u) {
  let instance = group.x;
  if (instance >= params.counts.x) {
    return;
  }
  let base = instance * params.counts.y;
  let origin = vec3f(
    bitcast<f32>(instance_words[base]),
    bitcast<f32>(instance_words[base + 1u]),
    bitcast<f32>(instance_words[base + 2u]),
  );
  let page = instance_words[base + 3u];
  let input = cull_inputs[instance];
  let unit = bitcast<f32>(input.y);
  let whole = (input.x & INPUT_WHOLE) != 0u;
  let clusters = params.counts.z;
  let pass_count = min(params.counts.w, MAX_CULL_PASSES);
  // The instance's quadrant word and stride bands, where its record holds them (T3.c), and whether
  // it is a sub-patch of its slot (T3.d).
  var quadrant = 0u;
  var hold_fades = 0u;
  var scale = vec2f(0.0);
  var sub = false;
  var sub_word = 0u;
  if (params.counts.y >= 16u) {
    quadrant = instance_words[base + 7u];
    sub_word = instance_words[base + 8u];
    hold_fades = instance_words[base + 14u];
    sub = (sub_word & 31u) != 0u;
    scale = vec2f(bitcast<f32>(instance_words[base + 12u]), bitcast<f32>(instance_words[base + 13u]));
  }
  // A slot's visibility words, a bit a cluster.
  let history_words = (clusters + 31u) / 32u;
  for (var c = local.x; c < clusters; c = c + CULL_GROUP) {
    let at = page * clusters + c;
    let word = page * history_words + c / 32u;
    let bit = 1u << (c % 32u);
    let record = records[at];
    let centre = origin + page_to_instance(record.centre);
    let shape = record.flags & 0xfu;
    let taken = record_class(record, c, input.x);
    let real = taken.real;
    let masked = mask_cluster(sub_word, c, input.x);
    let dag = (record.flags & CLUSTER_FLAG_DAG) != 0u;
    var parent_centre = centre;
    var parent_radius = record.radius;
    if (dag) {
      let lod = lod_of(at);
      parent_centre = origin + page_to_instance(lod.xyz);
      parent_radius = lod.w;
    }
    // The camera passes' bands, alike in each, and the record they take the stride by, at its
    // sphere: a mask's strip's its block's at its set's side, whose sphere holds the side's
    // vertices and whose errors bound them (`mask_side_block`).
    var bands = vec3f(-1.0);
    if (!dag && !whole && scale.y > 0.0) {
      bands = cluster_bands(scale, page, c, base, sub_word);
    }
    var stride_record = record;
    var stride_centre = centre;
    let side_block = mask_side_block(sub_word, c);
    if (side_block != NO_SIDE_BLOCK) {
      stride_record = records[page * clusters + side_block];
      stride_centre = origin + page_to_instance(stride_record.centre);
    }
    for (var p = 0u; p < pass_count; p = p + 1u) {
      let cull_pass = passes[p];
      let enters = real && (input.x & (1u << p)) != 0u;
      let keeps =
        enters &&
        masked != 0u &&
        (whole || masked == 2u || pass_keeps(cull_pass, centre, record.radius)) &&
        (!dag || dag_drawn(cull_pass, record, centre, parent_centre, parent_radius));
      var level = 0u;
      var over = false;
      if (cull_pass.header.x == PASS_CASCADE) {
        level = stride_level(cull_pass, unit);
      } else if (cull_pass.header.y > 0u && (quadrant & INSTANCE_SUB_GRID_STRIDES) != 0u) {
        level = sub_grid_stride_level(cull_pass, shape, quadrant, sub_word);
      } else if (cull_pass.header.y > 0u && !whole) {
        let distance = group_distance(stride_centre, stride_record.radius);
        let fraction = select(STRIDE_BAND_FRACTION, STRIDE_MASK_BAND_FRACTION, (sub_word & SUB_PATCH_MASK) != 0u);
        level = camera_stride_level(cull_pass, stride_record, bands, quadrant, hold_fades, distance, fraction);
        // A cluster its may-discard template draws is drawn at stride 1, never past τ′.
        let drawn_level = select(level, 0u, discard_template(shape, taken.flags) != NO_DISCARD_TEMPLATE);
        over = drawn_over(cull_pass, stride_record, scale, drawn_level, distance);
      }
      let t = template_of(cull_pass, shape, taken.flags, level);
      let mark = instance * clusters + c;
      if (cull_pass.header.x == PASS_CAMERA_ORDERED) {
        let drawn = keeps && t < params.templates;
        marks[mark] = select(NO_MARK, t, drawn);
        note_over(cull_pass, drawn, over);
        continue;
      }
      if (cull_pass.header.x == PASS_CAMERA_EARLY) {
        let seen = !sub && occlusion_seen(word, bit);
        let drawn = keeps && seen && t < params.templates;
        marks[mark] = select(NO_MARK, t, drawn);
        note_over(cull_pass, drawn, over);
        continue;
      }
      if (cull_pass.header.x == PASS_CAMERA_LATE) {
        let early = marks[mark] != NO_MARK;
        let visible =
          keeps && t < params.templates && (whole || masked == 2u || !occlusion_refuses(centre, record.radius));
        marks[mark] = select(NO_MARK, t, visible && !early);
        note_over(cull_pass, visible && !early, over);
        if (!sub) {
          occlusion_write(word, bit, visible);
        }
        continue;
      }
      if (!keeps || t >= params.templates) {
        continue;
      }
      note_over(cull_pass, true, over);
      let at = atomicAdd(&args[(cull_pass.header.z + t) * 5u + 1u], 1u);
      if (at < template_capacity(t)) {
        lists[cull_pass.header.w + template_offset(t) + at] = (instance << 8u) | c;
      }
    }
  }
}
