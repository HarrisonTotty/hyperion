// The scene's depth as the symbology's pass's (plan R14, R14.T1.e; `overlayDepth.ts`): a full-screen
// draw that writes each canvas pixel's depth from the scene target's at the same pixel, the sizes
// being the same, pushed away from the camera by TERRAIN_DEPTH_FRACTION of itself, as
// terrainDepth.wgsl pushes the terrain's, so that the terrain's depth there is that material's bit
// for bit. No colour is written: the material's colour writes are off. Composed after
// `view/shaders/frame.wgsl`.

struct Draw {
  // Zero: the draw is the screen's.
  offsetFromCameraM : vec3f,
}

@group(1) @binding(0) var<uniform> draw : Draw;
@group(2) @binding(0) var sceneDepth : texture_depth_2d;

// terrainDepth.wgsl's push, 2^-16 of the depth.
const TERRAIN_DEPTH_FRACTION : f32 = 0.0000152587890625;

struct OverlayDepth {
  // Unwritten: the material's colour writes are off.
  @location(0) colour : vec4f,
  @builtin(frag_depth) depth : f32,
}

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return vec4f(position.xy, 0.0, 1.0);
}

@fragment
fn fragmentMain(@builtin(position) at : vec4f) -> OverlayDepth {
  var out : OverlayDepth;
  out.colour = vec4f(0.0);
  // Away is smaller under reversed-Z.
  out.depth = clamp(textureLoad(sceneDepth, vec2i(at.xy), 0) * (1.0 - TERRAIN_DEPTH_FRACTION), 0.0, 1.0);
  return out;
}
