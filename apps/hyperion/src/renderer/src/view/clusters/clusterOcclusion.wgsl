// The cluster cull's two-pass occlusion (plan R14, R14.T3.d; `clusters/hzb.ts`), composed into
// `clusterCull.wgsl` below its bindings for `cull.ts`' CLUSTER_CULL_OCCLUSION_KERNEL: the late pass's
// pyramid (`hzb.ts`' `packHzbParams`; no levels refuse nothing), its texels and the clusters'
// visibility, a bit each, with the early pass's and the late pass's reads and writes of them.

struct HzbParams {
  right : vec4f,
  up : vec4f,
  forward : vec4f,
  size : vec4u,
  offsets0 : vec4u,
  offsets1 : vec4u,
  offsets2 : vec4u,
  offsets3 : vec4u,
}

@group(0) @binding(9) var<uniform> hzb : HzbParams;
@group(0) @binding(10) var<storage, read> pyramid : array<f32>;
@group(0) @binding(11) var<storage, read_write> history : array<atomic<u32>>;

// Level `level`'s first texel in the pyramid.
fn hzb_offset(level : u32) -> u32 {
  return word_of(hzb.offsets0, hzb.offsets1, hzb.offsets2, hzb.offsets3, level);
}

// A pixel's column or row of an NDC coordinate on a side of `side` px, held to within two of it.
fn hzb_pixel(ndc : f32, side : u32) -> i32 {
  return i32(floor(clamp((ndc + 1.0) * 0.5 * f32(side), -2.0, f32(side) + 2.0)));
}

// Whether the late pass refuses the sphere at `centre`, camera-relative, of radius `radius` against
// the pyramid (`hzb.ts`' `hzbRefuses`): its nearest depth behind the farthest over every pixel of
// its rectangle, read at the first level where it spans at most two texels each way; never across
// the near plane, about the camera, wholly off the depth, or with no pyramid.
fn hzb_refuses(centre : vec3f, radius : f32) -> bool {
  let levels = min(hzb.size.z, 16u);
  if (levels == 0u) {
    return false;
  }
  let w = hzb.size.x;
  let h = hzb.size.y;
  let r = radius + ROUNDING_FRACTION * length(centre) + ROUNDING_M;
  let x = dot(centre, hzb.right.xyz);
  let y = dot(centre, hzb.up.xyz);
  let z = dot(centre, hzb.forward.xyz);
  let near_z = z - r;
  if (!(near_z > hzb.forward.w)) {
    return false;
  }
  let far_z = z + r;
  let u0 = (x - r) / select(near_z, far_z, x - r >= 0.0);
  let u1 = (x + r) / select(far_z, near_z, x + r >= 0.0);
  let v0 = (y - r) / select(near_z, far_z, y - r >= 0.0);
  let v1 = (y + r) / select(far_z, near_z, y + r >= 0.0);
  let x0 = hzb_pixel(hzb.right.w * u0, w) - 1;
  let x1 = hzb_pixel(hzb.right.w * u1, w) + 1;
  let y0 = hzb_pixel(-hzb.up.w * v1, h) - 1;
  let y1 = hzb_pixel(-hzb.up.w * v0, h) + 1;
  if (x1 < 0 || y1 < 0 || x0 > i32(w) - 1 || y0 > i32(h) - 1) {
    return false;
  }
  let tx0 = u32(max(x0, 0)) >> 1u;
  let ty0 = u32(max(y0, 0)) >> 1u;
  let tx1 = u32(min(x1, i32(w) - 1)) >> 1u;
  let ty1 = u32(min(y1, i32(h) - 1)) >> 1u;
  var level = 0u;
  while (level < levels - 1u && ((tx1 >> level) - (tx0 >> level) > 1u || (ty1 >> level) - (ty0 >> level) > 1u)) {
    level = level + 1u;
  }
  let width = (w + (2u << level) - 1u) >> (level + 1u);
  let base = hzb_offset(level);
  var farthest = 3.4e38;
  for (var ty = ty0 >> level; ty <= ty1 >> level; ty = ty + 1u) {
    for (var tx = tx0 >> level; tx <= tx1 >> level; tx = tx + 1u) {
      farthest = min(farthest, pyramid[base + ty * width + tx]);
    }
  }
  return hzb.forward.w / near_z < farthest;
}

// Whether the cluster of visibility word `word`'s bit `bit` was visible last frame.
fn occlusion_seen(word : u32, bit : u32) -> bool {
  return (atomicLoad(&history[word]) & bit) != 0u;
}

// Whether the late pass refuses the sphere at `centre` of radius `radius` against the pyramid.
fn occlusion_refuses(centre : vec3f, radius : f32) -> bool {
  return hzb_refuses(centre, radius);
}

// Writes the cluster's visibility, its word `word`'s bit `bit`.
fn occlusion_write(word : u32, bit : u32, visible : bool) {
  if (visible) {
    atomicOr(&history[word], bit);
  } else {
    atomicAnd(&history[word], ~bit);
  }
}
