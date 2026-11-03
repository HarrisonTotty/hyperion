// The depth pyramid (plan R14, R14.T3.d; Design note 4): one dispatch a level, each texel the
// farthest depth of every source texel it covers, reversed-Z's least, the odd row or column
// included: texel (x, y) of a level covers (2x, 2y) to (2x + 1, 2y + 1) of the one below, the last
// clamped to its edge, so that a level of n texels has ⌈n ÷ 2⌉ above it (540 → 270 → 135 → 68).
// Level 0, half the depth's resolution, reads the depth target; each level after it the level
// before, in the pyramid's buffer, whose levels lie one after another, row by row. A minimum is
// exact, so the result is the same on every path (`bit-exact`). The TypeScript twin is
// `clusters/hzb.ts`' `depthPyramidTwin`.
//
// PyramidParams: `src` (the source's width and height, texels; the level written; the source
// level's first texel in the buffer, unread at level 0) and `dst` (the written level's width,
// height and first texel; `target` is a reserved word in WGSL).

struct PyramidParams {
  src : vec4u,
  dst : vec4u,
}

@group(0) @binding(0) var<uniform> params : PyramidParams;
@group(0) @binding(1) var depth : texture_depth_2d;
@group(0) @binding(2) var<storage, read_write> pyramid : array<f32>;

// Texel (x, y) of the source: the depth target's at level 0, else the level before's.
fn source_at(x : u32, y : u32) -> f32 {
  if (params.src.z == 0u) {
    return textureLoad(depth, vec2u(x, y), 0);
  }
  return pyramid[params.src.w + y * params.src.x + x];
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  if (id.x >= params.dst.x || id.y >= params.dst.y) {
    return;
  }
  let x0 = 2u * id.x;
  let y0 = 2u * id.y;
  let x1 = min(x0 + 1u, params.src.x - 1u);
  let y1 = min(y0 + 1u, params.src.y - 1u);
  let farthest = min(min(source_at(x0, y0), source_at(x1, y0)), min(source_at(x0, y1), source_at(x1, y1)));
  pyramid[params.dst.z + id.y * params.dst.x + id.x] = farthest;
}
