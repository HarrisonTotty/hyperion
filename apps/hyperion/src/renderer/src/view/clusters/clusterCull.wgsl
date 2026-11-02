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
//   terrain's `castsInto` in `f64`), and its error unit, m, as `f32` bits (terrain's: its level's
//   largest vertex spacing).
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
//   within the pass's tolerance, in `f32` as the twin forms it; s = 1 where none is, for a camera
//   pass (until T3.c) and for a DAG record.
// - _The bucket_: the record's shape (its flags' bits 0–3) less the consumer's first shape
//   (`shapes.x`) × STRIDE_LEVELS + l, from the pass's first bucket; the entry,
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
  listOffsetsLow : vec4u,
  listOffsetsHigh : vec4u,
  capacitiesLow : vec4u,
  capacitiesHigh : vec4u,
  shapes : vec4u,
}

// `cullPass.ts`' MAX_CULL_PASSES, a pass's kinds and the strides' levels.
const MAX_CULL_PASSES : u32 = 8u;
const PASS_CASCADE : u32 = 1u;
const PASS_CAMERA : u32 = 2u;
const PASS_CAMERA_ORDERED : u32 = 3u;
// A cluster an ordered pass refuses (`cull.ts`' NO_MARK).
const NO_MARK : u32 = 0xffffffffu;
// The flags' DAG-cut bit (`record.ts`' CLUSTER_FLAG_DAG).
const CLUSTER_FLAG_DAG : u32 = 16u;
const STRIDE_LEVELS : u32 = 4u;
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
// One parent group's LOD sphere a record (`record.ts`' `ClusterLod`), read by a DAG record alone.
@group(0) @binding(8) var<storage, read> lods : array<vec4f>;

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
    case PASS_CAMERA, PASS_CAMERA_ORDERED: {
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

// A record's template in the pass (see the module's head): a cascade's stride for a strided one,
// stride 1 for a camera pass's and a DAG one's.
fn template_of(cull_pass : CullPass, shape : u32, dag : bool, unit : f32) -> u32 {
  let level = select(stride_level(cull_pass, unit), 0u, dag || cull_pass.header.x != PASS_CASCADE);
  return (shape - params.shapes.x) * STRIDE_LEVELS + level;
}

fn template_offset(t : u32) -> u32 {
  if (t < 4u) {
    return params.listOffsetsLow[t];
  }
  return params.listOffsetsHigh[t - 4u];
}

fn template_capacity(t : u32) -> u32 {
  if (t < 4u) {
    return params.capacitiesLow[t];
  }
  return params.capacitiesHigh[t - 4u];
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
  let clusters = params.counts.z;
  let pass_count = min(params.counts.w, MAX_CULL_PASSES);
  for (var c = local.x; c < clusters; c = c + CULL_GROUP) {
    let at = page * clusters + c;
    let record = records[at];
    let centre = origin + page_to_instance(record.centre);
    let shape = record.flags & 0xfu;
    let real = record.radius >= 0.0;
    let dag = (record.flags & CLUSTER_FLAG_DAG) != 0u;
    var parent_centre = centre;
    var parent_radius = record.radius;
    if (dag) {
      let lod = lods[at];
      parent_centre = origin + page_to_instance(lod.xyz);
      parent_radius = lod.w;
    }
    for (var p = 0u; p < pass_count; p = p + 1u) {
      let cull_pass = passes[p];
      let enters = real && (input.x & (1u << p)) != 0u;
      let keeps =
        enters &&
        pass_keeps(cull_pass, centre, record.radius) &&
        (!dag || dag_drawn(cull_pass, record, centre, parent_centre, parent_radius));
      let t = template_of(cull_pass, shape, dag, unit);
      if (cull_pass.header.x == PASS_CAMERA_ORDERED) {
        marks[instance * clusters + c] = select(NO_MARK, t, keeps && t < params.templates);
        continue;
      }
      if (!keeps || t >= params.templates) {
        continue;
      }
      let at = atomicAdd(&args[(cull_pass.header.z + t) * 5u + 1u], 1u);
      if (at < template_capacity(t)) {
        lists[cull_pass.header.w + template_offset(t) + at] = (instance << 8u) | c;
      }
    }
  }
}
