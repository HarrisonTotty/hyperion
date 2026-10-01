// The pass's uniforms, shared by every material (R01's draft Design note 23, 2026-09-30): written
// once per pass at @group(0) @binding(0). Each material declares its own `Draw` at @group(1)
// @binding(0) (the draw's offset from the camera, then the spec's uniforms in order) and its
// resources at @group(2), and is composed after this file by string concatenation.
struct Frame {
  // The camera's rotation alone, column-major, its translation column zero (plan R02, Design
  // note 3).
  viewRotation: mat4x4f,
  // Reversed-Z, infinite far, WebGPU clip space, column-major (plan R02, Design note 4): s at
  // [0][0], s * aspect at [1][1], -1 at [2][3] and the near plane at [3][2].
  clipProjection: mat4x4f,
  // The target's size, px: width, height.
  viewport: vec2f,
}

@group(0) @binding(0) var<uniform> frame: Frame;
