// The interior patches' depth for the symbology's pass over the photorealistic image on the cluster
// path (plan R14, R14.T1.e): terrainDepth.wgsl's depth but its vertex stage, which reads a bucket's
// list (terrainCluster.wgsl), and its survey discard, which an interior patch's fragment never
// takes (plan R14, R14.T1.a). Each fragment's depth is pushed away from the camera by
// TERRAIN_DEPTH_FRACTION of itself alone, as terrainDepth.wgsl's, so that it is that material's
// depth of the same triangle, bit for bit; no colour is written. Composed after terrainLit.wgsl, in
// place of terrainLitShade.wgsl, then terrainCluster.wgsl and the vertex path's source
// (`terrainMaterial.ts`' `terrainClusterDepthMaterialSpec`).

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The lit material's: the camera less the body's centre, body-fixed, metres, w 1 ÷ a; unread.
  cameraBodyFixedM : vec4f,
  // The lit material's normals atlas; unread here.
  atlas : vec4f,
  // The overlay's draw list's `occluderSlopePx`, px; unread, as terrainDepth.wgsl's.
  occluderSlopePx : f32,
  // The bucket's first entry in the lists (`clusters/buckets.ts`' `clusterBase`).
  clusterBase : u32,
}

@group(1) @binding(0) var<uniform> draw : Draw;

// terrainDepth.wgsl's push, 2^-16 of the depth.
const TERRAIN_DEPTH_FRACTION : f32 = 0.0000152587890625;

struct TerrainDepth {
  // Unwritten: the material's colour writes are off.
  @location(0) colour : vec4f,
  @builtin(frag_depth) depth : f32,
}

@fragment
fn fragmentMain(in : LitVertexOut) -> TerrainDepth {
  var out : TerrainDepth;
  out.colour = vec4f(0.0);
  // Away is smaller under reversed-Z.
  out.depth = clamp(in.clip.z * (1.0 - TERRAIN_DEPTH_FRACTION), 0.0, 1.0);
  return out;
}
