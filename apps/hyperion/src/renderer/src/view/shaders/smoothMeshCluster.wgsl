// The mesh regime's figure on the cluster path (plan R14, R14.T4.a): a vertex stage over a bucket's
// list (`clusters/buckets.ts`) and the figure's cluster mesh's templates (`clusters/templates.ts`'
// `smoothFigureClusterMesh`), which hands `smooth_vertex`, the figure's own vertex stage
// (smoothMesh.wgsl's, as `bodies/draw.ts`' `withFigureClusters` names it), exactly R05's patch
// mesh's vertex and the instance's index in the frame's records, so that each vertex is placed as
// the instanced draw places it, its position `@invariant`. The decode is terrainCluster.wgsl's,
// whose TypeScript twin is `clusters/terrain.ts`' `terrainClusterGrid`. Composed after
// smoothMesh.wgsl, its `Draw` declaring `clusterBase`, a bucket's list base, and its
// `firstInstance` 0, since a list's entry names its instance in the frame's records.

// The buckets' lists, one `u32` an entry, (instance << 8) | cluster, each bucket from its base.
@group(2) @binding(22) var<storage, read> cluster_lists : array<u32>;

// A patch's blocks, 8 × 8, then its skirt strips, 8 an edge (`clusters/terrain.ts`), and the quads
// along a block's side or a strip.
const FIGURE_BLOCKS : u32 = 64u;
const FIGURE_CLUSTER_QUADS : u32 = 8u;

// The grid vertex that is the `k`-th of edge `edge`, anticlockwise (`gpu/resources.ts`' own).
fn figure_edge_vertex(edge : u32, k : u32) -> vec2u {
  switch edge {
    case 0u: {
      return vec2u(k, 0u);
    }
    case 1u: {
      return vec2u(64u, k);
    }
    case 2u: {
      return vec2u(64u - k, 64u);
    }
    default: {
      return vec2u(0u, 64u - k);
    }
  }
}

// R05's patch mesh's (x, y, skirt) of template vertex `local` ((lx, ly, skirt)) of `cluster`.
fn figure_cluster_grid(cluster : u32, local : vec3f) -> vec3f {
  let lx = u32(local.x);
  let ly = u32(local.y);
  if (cluster < FIGURE_BLOCKS) {
    let at = vec2u((cluster % 8u) * FIGURE_CLUSTER_QUADS + lx, (cluster / 8u) * FIGURE_CLUSTER_QUADS + ly);
    return vec3f(f32(at.x), f32(at.y), local.z);
  }
  let strip = cluster - FIGURE_BLOCKS;
  let at = figure_edge_vertex(strip / 8u, (strip % 8u) * FIGURE_CLUSTER_QUADS + lx);
  return vec3f(f32(at.x), f32(at.y), local.z);
}

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> MeshVarying {
  let item = cluster_lists[draw.clusterBase + entry];
  return smooth_vertex(figure_cluster_grid(item & 0xffu, local), item >> 8u);
}
