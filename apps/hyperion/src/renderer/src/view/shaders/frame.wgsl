// The pass's frame, `@group(0)` of every material and post-process (plan R01, Design note 23).
// Included by string concatenation of this file's `?raw` import ahead of a shader's own source.
// The engine writes it once a pass, packed by WGSL's uniform layout: two mat4x4f, then a vec4f.

struct Frame {
  // FrameSubmission.viewRotation: column-major, translation zero, right-handed.
  viewRotation : mat4x4f,
  // FrameSubmission.projection: reversed-Z, WebGPU clip space, column-major.
  clipProjection : mat4x4f,
  // The output's width and height in pixels, then their reciprocals.
  viewport : vec4f,
}

@group(0) @binding(0) var<uniform> frame : Frame;
