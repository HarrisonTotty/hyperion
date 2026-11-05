// The visibility buffer's ID raster on the cluster path (plan R14, R14.T2.c; Design note 9): the
// lit pass's opaque buckets drawn into the scene's depth and an `r32uint` ID a pixel,
// (visible-cluster index << 7) | triangle (`clusters/visibility.ts`' `packVisibilityId`), with no
// discard and no shading. Its vertex stage places each template vertex as the lit cluster form's
// does (`terrainCluster.wgsl`'s decode, then `stride_place` with the lit surface's stride morph), and
// its clip position, `@invariant` as every terrain vertex stage's is, is that form's to the bit, so
// that the depth it writes is the forward pass's. It hands the fragment stage one flat `u32` beside
// the position: the ID on the de-indexed path (`terrainClusterIdDeindexed.wgsl`), the
// visible-cluster index on the `primitive_index` one (`terrainClusterIdPrimitive.wgsl`), 20 B a
// vertex either way. Composed after terrainLit.wgsl, its vertex stage made the function
// `lit_vertex`, and before one of the two paths' entry points, the vertex path's source and
// terrainCluster.wgsl, its entry point made a function (`terrainMaterial.ts`'
// `terrainClusterIdMaterialSpec`).

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The lit material's: the camera less the body's centre, body-fixed, metres, w 1 ÷ a; unread.
  cameraBodyFixedM : vec4f,
  // The lit material's normals atlas; unread here.
  atlas : vec4f,
  // The bucket's first entry in the lists (`clusters/buckets.ts`' `clusterBase`).
  clusterBase : u32,
  // The first index of the body's range of the view's index space, 0 for the frame's first body
  // (`clusters/visibility.ts`' `VISIBLE_FIRST_UNIFORM`, plan R14, R14.T2.d).
  visibleFirst : u32,
}

@group(1) @binding(0) var<uniform> draw : Draw;

// The raster places the lit surface, which takes the stride morph (plan R14, R14.T3.c;
// `terrainStrides.wgsl`), as the lit cluster form does.
const STRIDE_MORPH : bool = true;

// `clusters/visibility.ts`' VISIBILITY_TRIANGLE_BITS and DEINDEXED_TRIANGLE_STEP.
const VISIBILITY_TRIANGLE_BITS : u32 = 7u;
const DEINDEXED_TRIANGLE_STEP : u32 = 8u;

// What the raster's vertex stage hands its fragment stage: the clip position and one flat word.
struct TerrainIdVertex {
  @builtin(position) @invariant clip : vec4f,
  @location(0) @interpolate(flat) word : u32,
}

// The vertex `local` of the bucket entry `entry` of the draw: its clip position, as the lit cluster
// form's `vertexMain` places it, and its visible-cluster index, the entry's index in the lists past
// its body's range's first index. Its
// `local.z` holds its lit-mesh code below DEINDEXED_TRIANGLE_STEP, and a de-indexed mesh's triangle
// above it, which the code's decode drops.
struct TerrainIdPlace {
  clip : vec4f,
  visible : u32,
  packed : u32,
}

fn terrain_id_place(local : vec3f, entry : u32) -> TerrainIdPlace {
  let listed = draw.clusterBase + entry;
  let item = cluster_lists[listed];
  let instance = item >> 8u;
  let packed = u32(local.z + 0.5);
  let grid = terrain_cluster_grid(
    item & 0xffu, vec3f(local.xy, f32(packed % DEINDEXED_TRIANGLE_STEP)), instances[instance].sub.x
  );
  let placed = stride_place(grid, instance);
  var out : TerrainIdPlace;
  out.clip = frame.clipProjection * frame.viewRotation * vec4f(placed.fromCameraM, 1.0);
  out.visible = draw.visibleFirst + listed;
  out.packed = packed;
  return out;
}
