// The pass's frame, `@group(0)` of every material and post-process (plan R01, Design note 23).
// Included by string concatenation of this file's `?raw` import ahead of a shader's own source.
// The engine writes it once a pass, packed by WGSL's uniform layout: two mat4x4f, then two vec4f.

struct Frame {
  // FrameSubmission.viewRotation: column-major, translation zero, right-handed.
  viewRotation : mat4x4f,
  // FrameSubmission.projection: reversed-Z, WebGPU clip space, column-major.
  clipProjection : mat4x4f,
  // The output's width and height in pixels, then their reciprocals.
  viewport : vec4f,
  // FrameSubmission.tileOriginPx (plan R11, R11.T10.e.2): the image's pixel at the output's top
  // left, a still's tile's origin with its sample's sub-pixel offset, then 0 and 0; 0 throughout
  // for every view's frame. The shaders that place the CPU's pixels or rebuild a ray by pixel add
  // it to the output's pixels, and take clipProjection's third column, M's off-axis terms.
  tile : vec4f,
}

@group(0) @binding(0) var<uniform> frame : Frame;
