// The scene's depth before the near sea (plan R11, R11.T8.c; `oceanNearPass.ts`): a full-screen draw
// into an r32uint target that writes each pixel's reversed-Z depth as its f32 bits, so that the near
// sea reads the bed's depth while it draws into, and writes, the scene's own. Composed after
// `view/shaders/frame.wgsl`.

struct Draw {
  // Zero: the draw is the screen's.
  offsetFromCameraM : vec3f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var sceneDepth : texture_depth_2d;

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return vec4f(position.xy, 0.0, 1.0);
}

@fragment
fn fragmentMain(@builtin(position) at : vec4f) -> @location(0) u32 {
  return bitcast<u32>(textureLoad(sceneDepth, vec2i(at.xy), 0));
}
