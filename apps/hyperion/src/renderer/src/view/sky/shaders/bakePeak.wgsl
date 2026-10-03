// The bake's first pass over a splatted face (plan R06, T13.g): the brightest channel of its
// luminances, flux ÷ solid angle, into one u32 by an atomic maximum of the f32's bits, which order
// as the values do for non-negative floats. Integer maxima in any order give the same bytes.

struct Params {
  // The face's side, texels, then 0, 0, 0.
  size : vec4u,
}

@group(0) @binding(0) var<uniform> params : Params;
@group(0) @binding(1) var splat : texture_storage_2d<rgba32float, read>;
@group(0) @binding(2) var<storage, read> omega : array<f32>;
@group(0) @binding(3) var<storage, read_write> peak : array<atomic<u32>, 1>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = params.size.x;
  if (id.x >= size || id.y >= size) {
    return;
  }
  let flux = textureLoad(splat, vec2i(id.xy)).rgb;
  let luminance = flux / omega[id.y * size + id.x];
  let brightest = max(luminance.r, max(luminance.g, luminance.b));
  // A finite positive peak only, as `mips.ts`'s `peakScaleExponent` (a solid angle is never zero,
  // so this holds without relying on NaN, which WGSL may assume away).
  if (brightest > 0.0 && brightest <= 3.4028234e38) {
    atomicMax(&peak[0], bitcast<u32>(brightest));
  }
}
