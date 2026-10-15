// The terrain pass (plan R05, R05.T11.b, Design notes 4 to 6 and 17): one instanced draw of the
// shared 65 × 65 mesh with skirts, an instance per drawn patch, read from the cache's slots, shaded
// by one hand albedo under one sun (the descent spike's and the test planet's). Composed with
// frame.wgsl, patchVertex.wgsl, terrainPatch.wgsl (the records, bindings and the vertex and normal
// arithmetic, which plan R10's lit material shares) and one of the two vertex paths,
// terrainBakedOffsets.wgsl or terrainFaceDifferences.wgsl.

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The unit direction towards the sun, body-fixed; w unused.
  sunDirection : vec4f,
  // albedo ÷ π × the sun's illuminance per channel × the pre-exposure; w unused (Design note 17).
  sunRadiance : vec4f,
  // The normals atlas: tiles across a layer, tiles a layer, a tile's stored texels, samples a side.
  atlas : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;

struct VertexOut {
  @builtin(position) clip : vec4f,
  // The vertex's place in its patch, (x, y) ÷ 64.
  @location(0) patchUv : vec2f,
  @location(1) @interpolate(flat) slot : u32,
}

@vertex
fn vertexMain(@location(0) grid : vec3f, @builtin(instance_index) instance : u32) -> VertexOut {
  let v = patchVertexOf(grid, instance);
  var out : VertexOut;
  out.clip = frame.clipProjection * frame.viewRotation * vec4f(v.fromCameraM, 1.0);
  out.patchUv = v.patchUv;
  out.slot = v.slot;
  return out;
}

@fragment
fn fragmentMain(in : VertexOut) -> @location(0) vec4f {
  let n = patchNormalAt(in.slot, in.patchUv);
  let cosine = max(dot(n, draw.sunDirection.xyz), 0.0);
  return vec4f(min(draw.sunRadiance.rgb * cosine, vec3f(HALF_FLOAT_MAX)), METER_LIT_BODY);
}
