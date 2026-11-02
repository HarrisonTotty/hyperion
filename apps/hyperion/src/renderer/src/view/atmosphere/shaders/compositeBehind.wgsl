// The transmittance to space from above the top at the march's resolution (the owner's priority,
// 2026-11-01): what lies behind each sky texel of the ray march's area, light slot 0's, per
// channel, across the far-limb footprint of an output pixel at the texel's centre
// (`compositeTransmittance.wgsl`'s transmittanceAcrossFromAbove, R08.T8.b and R08.T8.c), the
// composite's term formed once a texel rather than once a pixel. The footprint is a pixel's, not
// the texel's, so that the composite's read between texels samples the field its per-pixel path
// takes: over a texel's footprint, twice a pixel's, the limb's rows next to the face's edge stood
// stops apart from it. Dispatched over the march's tiles (`marchArea.wgsl`) where the march
// runs below the output's resolution and the camera's h* is above the top; the composite reads a
// texel, one of a pixel's four candidates, only where its source pixel is sky (`composite.wgsl`'s
// behindAtMarch), so a texel is a sky texel here by that pixel's depth, and a texel that is not is
// left as it was. Its ray is through the texel's centre as the composite's read places it, and it
// stores each channel's ln tau, tau = -ln T, which the composite interpolates.
// Follows `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `view.wgsl`, `oblate.wgsl`,
// `farLimb.wgsl`, `marchArea.wgsl` and `compositeTransmittance.wgsl`.

// The composite's `Draw.shell`, which `compositeTransmittance.wgsl` reads.
struct BehindDraw {
  shell : vec4f,
}

@group(0) @binding(0) var<uniform> draw : BehindDraw;
@group(0) @binding(1) var<uniform> view : AtmosphereView;
@group(0) @binding(2) var<uniform> marchArea : MarchArea;
@group(0) @binding(3) var sceneDepth : texture_depth_2d;
@group(0) @binding(4) var transmittance : texture_2d_array<f32>;
@group(0) @binding(5) var absorberColumns : texture_2d_array<f32>;
@group(0) @binding(6) var absorberCurves : texture_2d<f32>;
@group(0) @binding(7) var absorberPressureColumns : texture_2d_array<f32>;
@group(0) @binding(8) var lastAbsorberPressureColumn : texture_2d_array<f32>;
@group(0) @binding(9) var oblateBody : texture_2d<f32>;
@group(0) @binding(10) var behindOut : texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(
  @builtin(workgroup_id) group : vec3u,
  @builtin(local_invocation_id) local : vec3u,
) {
  let texel = marchTexelOf(marchArea, group.xy, local.xy);
  // The target may be larger than this frame's output (it grows and is never remade smaller).
  let scale = view.output.z;
  let size = vec2u(ceil(view.output.xy * scale));
  if (texel.x >= size.x || texel.y >= size.y) {
    return;
  }
  // The texel's source pixel, as `behindAtMarch` and `rayMarched` take its depth.
  let full = vec2u(view.output.xy);
  let source = min(vec2u((vec2f(texel) + 0.5) / scale), full - 1u);
  if (textureLoad(sceneDepth, source, 0) != 0.0) {
    return;
  }
  // The texel's centre where the composite's read places it, (t + 1/2) / scale output pixels, not
  // the march's (t + 1/2) / size, which drifts from it by up to a pixel where output x scale is
  // not whole (481 rows at half scale, 241 texels).
  let uv = (vec2f(texel) + 0.5) / (scale * view.output.xy);
  let body = oblateBodyOf(oblateBody);
  var footprint = farLimbFootprintOf(farLimbRaysOf(view), body, oblateBody, uv, view.output.xy);
  let behind = transmittanceAcrossFromAbove(view, body, &footprint, rayThrough(view, uv), 0u);
  // ln tau, the composite's interpolant, formed once a texel: tau at least 10^-13 (ln tau about
  // -30), where T is 1 in f32 and e^-e^(ln tau) is 1 again; T at least 10^-30, below any light
  // the half float holds once exposed.
  let lnDepth = log(max(-log(max(behind, vec3f(1e-30))), vec3f(1e-13)));
  textureStore(behindOut, texel, vec4f(lnDepth, 1.0));
}
