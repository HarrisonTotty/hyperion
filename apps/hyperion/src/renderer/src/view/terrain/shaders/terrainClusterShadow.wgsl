// The interior patches' casters into the high setting's shadow cascades on the cluster path (plan
// R14, R14.T1.b): terrainShadow.wgsl's caster but its vertex stage, which reads a bucket's list
// (terrainCluster.wgsl), and its survey discard, which an interior patch's fragment never takes
// (plan R14, R14.T1.a). Depth alone, into a `depth-only` target: the fragment stage writes nothing
// and discards nothing, the depth the rasteriser's, biased as the caster's (`CASCADE_DEPTH_BIAS`).
// Composed after terrainLit.wgsl, in place of terrainLitShade.wgsl, then terrainCluster.wgsl and
// the vertex path's source (`terrainMaterial.ts`' `terrainClusterShadowMaterialSpec`).

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
}

@group(1) @binding(0) var<uniform> draw : Draw;

@fragment
fn fragmentMain(in : LitVertexOut) {
}
