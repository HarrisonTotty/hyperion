// The far limb's census over a march's area (plan R08, R08.T8.d; check-only, the inspection
// profile's): each texel's rays as the footprint's entry point marches them
// (`rayMarchFootprint.wgsl`), counted over the area's tiles. A texel marches its centre ray, unless
// its footprint's blend is 1 (E4), and its footprint's nodes where the blend is above 0; a ray here
// is one that meets the top's shell ahead of the camera, since one that misses it returns at once.
// Each tile, the march's workgroup, waits on its slowest texel, so that its texels together cost
// its count of texels times that texel's rays: the census sums both, the rays and the lanes' rays.
// Dispatched over the march's tiles with the march's own uniforms and depth (`hillaire.ts`'s
// MARCH_CENSUS_KERNEL), the census zeroed before it. Follows `view/shaders/preciseTrig.wgsl`,
// `common.wgsl`, `view.wgsl`, `oblate.wgsl`, `farLimb.wgsl` and `marchArea.wgsl`.

// The most nodes a footprint takes: the ground piece's 2 and the shell piece's 8.
const MARCH_CENSUS_NODES : u32 = 10u;

// The census (`hillaire.ts`'s MarchCensus, word for word): the texels by their footprints' nodes,
// 0 to MARCH_CENSUS_NODES; the texels whose footprint's blend is 1, which march no centre ray; the
// texels whose centre ray meets the shell; the tiles, those with a footprint's node, the rays, and
// the lanes' rays.
struct MarchCensus {
  byNodes : array<atomic<u32>, 11>,
  wholeMeans : atomic<u32>,
  centresInAir : atomic<u32>,
  tiles : atomic<u32>,
  limbTiles : atomic<u32>,
  rays : atomic<u32>,
  laneRays : atomic<u32>,
}

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<uniform> view : AtmosphereView;
@group(0) @binding(2) var<uniform> marchArea : MarchArea;
@group(0) @binding(3) var sceneDepth : texture_depth_2d;
@group(0) @binding(4) var oblateBody : texture_2d<f32>;
@group(0) @binding(5) var<storage, read_write> census : MarchCensus;

// The tile's slowest texel's rays, its texels on the target, and whether any takes a node.
var<workgroup> tileMost : atomic<u32>;
var<workgroup> tileTexels : atomic<u32>;
var<workgroup> tileNodes : atomic<u32>;

@compute @workgroup_size(8, 8, 1)
fn main(
  @builtin(workgroup_id) group : vec3u,
  @builtin(local_invocation_id) local : vec3u,
  @builtin(local_invocation_index) lane : u32,
) {
  if (lane == 0u) {
    atomicStore(&tileMost, 0u);
    atomicStore(&tileTexels, 0u);
    atomicStore(&tileNodes, 0u);
  }
  workgroupBarrier();
  let texel = marchTexelOf(marchArea, group.xy, local.xy);
  // As the footprint's entry point takes the texel, its target and its need.
  let size = vec2u(ceil(view.output.xy * view.output.z));
  if (texel.x < size.x && texel.y < size.y) {
    let uv = (vec2f(texel) + 0.5) / vec2f(size);
    let full = vec2u(view.output.xy);
    let depthTexel = min(vec2u(uv * vec2f(full)), full - 1u);
    let depth = textureLoad(sceneDepth, depthTexel, 0);
    var tSurface = 3.4e38;
    if (depth > 0.0) {
      tSurface = distanceAtDepth(view, uv, depth);
    }
    let needed = view.exact.w > 0.0 || (depth > 0.0 && tSurface > view.tables.y);
    var nodes = 0u;
    var rays = 0u;
    if (needed) {
      let body = oblateBodyOf(oblateBody);
      let rays0 = farLimbRaysOf(view);
      let topM = medium.topHeightM;
      let top = farLimbApproach(
        rays0.camera,
        farLimbRayThrough(rays0, uv),
        rays0.a + topM * body.scales.z,
        rays0.c + topM * body.scales.w,
      );
      let inAir = top.x < 0.0 && (top.y > 0.0 || view.camera.w * view.curvature.z < topM);
      var centre = inAir;
      let above = view.camera.w * view.curvature.z >= topM;
      if (above) {
        let footprint = farLimbFootprintOf(rays0, body, oblateBody, uv, vec2f(size));
        if (footprint.blend > 0.0) {
          nodes = min(footprint.count, MARCH_CENSUS_NODES);
        }
        if (footprint.blend >= 1.0) {
          centre = false;
          atomicAdd(&census.wholeMeans, 1u);
        }
      }
      if (inAir) {
        atomicAdd(&census.centresInAir, 1u);
      }
      rays = select(0u, 1u, centre) + nodes;
    }
    atomicAdd(&census.byNodes[nodes], 1u);
    atomicAdd(&census.rays, rays);
    atomicMax(&tileMost, rays);
    atomicAdd(&tileTexels, 1u);
    if (nodes > 0u) {
      atomicStore(&tileNodes, 1u);
    }
  }
  workgroupBarrier();
  if (lane == 0u) {
    atomicAdd(&census.tiles, 1u);
    atomicAdd(&census.laneRays, atomicLoad(&tileMost) * atomicLoad(&tileTexels));
    if (atomicLoad(&tileNodes) > 0u) {
      atomicAdd(&census.limbTiles, 1u);
    }
  }
}
