// A body's survey coverage over the wireframe's reference surface (plan R10, R10.T11; Design note
// 12): where the wireframe draws a body's surveyed terrain, its occluder sphere and its own lines,
// its limb and graticule at the datum, which stand for the reference surface there, are discarded
// wherever the body's GPU coverage mask holds the cell beneath them surveyed, as R07's figure is in
// the photorealistic style (`smoothMesh.wgsl`), so that no datum is drawn over or through surveyed
// ground. A library composed by concatenation after frame.wgsl and cubeTexel.wgsl, before an
// includer that declares `draw` with `coverageAxes` and `coverageStretch`.

// The body's coverage mask (`terrain/coverage.ts`' `CoverageMask`): one `r8unorm` layer a face of
// R05's cube sphere, 2ᴸ × 2ᴸ texels at the coarse field's level L, 1 where the cell is surveyed.
@group(2) @binding(1) var coverage_mask : texture_2d_array<f32>;

// Whether the point `p`, m from the body's centre along the camera frame's axes, lies over a cell
// the mask holds surveyed: its direction in the body-fixed axes (`draw.coverageAxes`, the body's
// own, into the frame's), stretched along the pole by a ÷ c (`draw.coverageStretch.x`), the
// spheroid's point of unit direction d being M d, and that direction's texel of the mask
// (`class_map_texel`, the mask's lookup).
fn coverage_surveyed(p : vec3f) -> bool {
  let bodyFixed = vec3f(
    dot(p, draw.coverageAxes[0].xyz),
    dot(p, draw.coverageAxes[1].xyz),
    dot(p, draw.coverageAxes[2].xyz) * draw.coverageStretch.x,
  );
  let texel = class_map_texel(bodyFixed, textureDimensions(coverage_mask).x);
  return textureLoad(coverage_mask, texel.xy, texel.z, 0).r >= 0.5;
}
