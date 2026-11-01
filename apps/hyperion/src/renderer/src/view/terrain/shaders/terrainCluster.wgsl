// Terrain's clusters as the cluster path draws them (plan R14, R14.T1.b): a vertex stage over a
// bucket's list (`clusters/buckets.ts`) and the terrain cluster mesh's templates
// (`clusters/templates.ts`), which hands `lit_vertex`, the lit material's own vertex stage
// (terrainLit.wgsl's, as `terrainMaterial.ts`' `withClusterVertex` names it), exactly the lit
// mesh's vertex, so that each is placed by `lit_place` as the lit mesh's is. The TypeScript twin of
// the decode is `clusters/terrain.ts`' `terrainClusterGrid`. Composed after the lit material's
// library and a `Draw` that declares `clusterBase`, a bucket's list base.

// The buckets' lists, one `u32` an entry, (instance << 8) | cluster, each bucket from its base.
@group(2) @binding(22) var<storage, read> cluster_lists : array<u32>;

// A patch's blocks, 8 × 8, then its skirt-foot strips, 8 an edge (`clusters/terrain.ts`), and the
// quads along a block's side or a strip.
const TERRAIN_BLOCKS : u32 = 64u;
const CLUSTER_QUADS : u32 = 8u;

// The grid vertex that is the `k`-th of edge `edge`, anticlockwise (`gpu/resources.ts`' own).
fn terrain_edge_vertex(edge : u32, k : u32) -> vec2u {
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

// The lit mesh's (x, y, code) of template vertex `local` ((lx, ly, code)) of `cluster`.
fn terrain_cluster_grid(cluster : u32, local : vec3f) -> vec3f {
  let lx = u32(local.x);
  let ly = u32(local.y);
  if (cluster < TERRAIN_BLOCKS) {
    let at = vec2u((cluster % 8u) * CLUSTER_QUADS + lx, (cluster / 8u) * CLUSTER_QUADS + ly);
    return vec3f(f32(at.x), f32(at.y), local.z);
  }
  let strip = cluster - TERRAIN_BLOCKS;
  let at = terrain_edge_vertex(strip / 8u, (strip % 8u) * CLUSTER_QUADS + lx);
  return vec3f(f32(at.x), f32(at.y), local.z);
}

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> LitVertexOut {
  let item = cluster_lists[draw.clusterBase + entry];
  return lit_vertex(terrain_cluster_grid(item & 0xffu, local), item >> 8u);
}
