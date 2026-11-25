// The authoritative rocks as casters into the high setting's shadow cascades (plan R14, R14.T6.c):
// each vertex of a finest patch's rock mesh decoded from its page (`rockPage.wgsl`) and placed as
// the lit draw places it (`rock.wgsl`'s `rock_vertex`): its offset from its instance's origin, the
// patch's origin and its pages' origin less the camera, narrowed once from `f64` on the CPU in the
// non-rotating axes, turned from the body-fixed axes by the body's rotation, then by the pass's
// `Frame`, which a cascade's pass fills with its light rotation and reversed orthographic
// projection (`cascades.ts`' `Cascade.viewRotation` and `projection`). A template vertex past its
// cluster's last triangle is placed outside the clip volume, one point for all three corners.
//
// Drawn into a `depth-only` target (the engine's `DepthLayerTarget`), as the terrain's casters are
// (`terrainClusterShadow.wgsl`): the fragment stage writes nothing and discards nothing, the depth
// the rasteriser's, pushed away from the light by the pipeline's slope-scaled bias
// (`CASCADE_DEPTH_BIAS`, as `depthBiasAway`), with culling off so that a rock casts from either
// face, and, where the adapter offers `depth-clip-control`, `unclippedDepth`, which pancakes a
// caster nearer the sun than the cascade's near plane onto it. Composed after frame.wgsl and
// `rockPage.wgsl` (`rockMaterial.ts`' `rockCasterMaterialSpec`).

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The bucket's first entry in the lists (`clusters/buckets.ts`' `clusterBase`).
  clusterBase : u32,
}

@group(1) @binding(0) var<uniform> draw : Draw;

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> @builtin(position) @invariant vec4f {
  let item = cluster_lists[draw.clusterBase + entry];
  let instance = rock_instances[item >> 8u];
  let corner = rock_corner(instance, item & 0xffu, u32(local.y), u32(local.x));
  if (!corner.drawn) {
    return vec4f(0.0, 0.0, 2.0, 1.0);
  }
  let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
  let fromCameraM = instance.originM + rotation * corner.offset;
  return frame.clipProjection * frame.viewRotation * vec4f(fromCameraM, 1.0);
}

@fragment
fn fragmentMain() {
}
