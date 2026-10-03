// The exposure histogram (plan R07, T12, Design note 10): 256 bins over log2 of the pre-exposed
// luminance from -14 to +16, each pixel weighted by its meter class (the HDR target's alpha) under
// the operator's meter. Workgroup-memory atomics, then one global add per non-empty bin: integer
// adds, so any order of reduction gives the same bytes and the result may be read back.
//
// Its CPU twin is `cpuHistogram` in `histogram.ts`, which the smoke harness compares bin for bin.

struct Params {
  // The integer weight of each meter class, by class (`meterWeights`).
  weights : vec4<u32>,
  // The input's size, texels.
  size : vec2<u32>,
  // 1 reads every texel, 2 every other on each axis (the low setting's quarter resolution).
  stride : u32,
  pad : u32,
}

@group(0) @binding(0) var<uniform> params : Params;
@group(0) @binding(1) var hdr : texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> bins : array<atomic<u32>, 256>;

var<workgroup> local : array<atomic<u32>, 256>;

// Rec. 709 luminance of a linear colour.
const LUMA = vec3f(0.2126, 0.7152, 0.0722);
// 2^-14, the smallest normal half float: everything below it, zeros and NaN included, is bin 0.
const LOWEST = 6.103515625e-5;
// Bins per stop: 255 bins over the 30 stops from -14 to +16.
const BINS_PER_STOP = 8.5;

fn binOf(luminance : f32) -> u32 {
  if (!(luminance >= LOWEST)) {
    return 0u;
  }
  // An infinite texel (a pass that wrote above 65,504) is the brightest bin, never black: log2 and
  // a saturating u32 of +inf would wrap 1 + 0xffffffff to bin 0. WGSL leaves infinities to the
  // implementation, so the value is clamped to the format's largest before anything else.
  let finite = min(luminance, 65504.0);
  let position = (log2(finite) + 14.0) * BINS_PER_STOP;
  return 1u + u32(clamp(floor(position), 0.0, 254.0));
}

@compute @workgroup_size(16, 16)
fn main(
  @builtin(global_invocation_id) global : vec3<u32>,
  @builtin(local_invocation_index) index : u32,
) {
  atomicStore(&local[index], 0u);
  workgroupBarrier();
  let texel = global.xy * params.stride;
  if (texel.x < params.size.x && texel.y < params.size.y) {
    let colour = textureLoad(hdr, vec2<i32>(texel), 0);
    let meterClass = min(u32(round(max(colour.a, 0.0))), 3u);
    let weight = params.weights[meterClass];
    if (weight > 0u) {
      atomicAdd(&local[binOf(dot(colour.rgb, LUMA))], weight);
    }
  }
  workgroupBarrier();
  let count = atomicLoad(&local[index]);
  if (count > 0u) {
    atomicAdd(&bins[index], count);
  }
}
