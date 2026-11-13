// The exposure histogram (plan R07, T12, Design note 10): 256 bins over log2 of the pre-exposed
// luminance from -14 to +16, each pixel weighted by its meter class (the HDR target's alpha) under
// the operator's meter. Workgroup-memory atomics, then one global add per non-empty bin: integer
// adds, so any order of reduction gives the same bytes and the result may be read back.
//
// A pixel's luminance is the light the optic delivers there: its stored value plus every glare
// source's veil at it, of the source's whole light, which no pre-exposure changes (`glare_per_unit`
// from `glare.wgsl`, with the frame's view rotation and projection as the tone-mapping pass reads
// them). The host disc's own pixels still weigh nothing, so the disc's own light never steps the
// exposure; its veil does by its role (`disc.ts`): an eye view's rises smoothly over its 45° reach
// beyond the frame, and a camera view's, every console view's, stands whole as soon as the disc
// touches the frame and is gone once it leaves (ux-calls-view, item 3a). The bloom chain's gathered
// light is left out: the chain only moves the weighed texels' light, conserving it, and leaves the
// disc's texels out, whose light the veil of the whole light holds.
//
// Concatenated after `glare.wgsl`. Its CPU twin is `cpuHistogram` in `histogram.ts`, which the smoke
// harness compares bin for bin.

struct Params {
  // The integer weight of each meter class, by class (`meterWeights`).
  weights : vec4<u32>,
  // The input's size, texels.
  size : vec2<u32>,
  // 1 reads every texel, 2 every other on each axis (the low setting's quarter resolution).
  stride : u32,
  // The glare sources in `glareSources`.
  sourceCount : u32,
}

// The frame's view rotation and projection (`frame.wgsl`'s `Frame`) and the view's spread
// function (`GlareTerms`), as `histogramGlareUniform` packs them.
struct MeteredGlare {
  viewRotation : mat4x4f,
  clipProjection : mat4x4f,
  poisson0 : vec4f,
  poisson1 : vec4f,
  poisson2 : vec4f,
  broad : vec4f,
  misc : vec4f,
}

@group(0) @binding(0) var<uniform> params : Params;
@group(0) @binding(1) var hdr : texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> bins : array<atomic<u32>, 256>;
@group(0) @binding(3) var<uniform> glare : MeteredGlare;
@group(0) @binding(4) var<storage, read> glareSources : array<GlareSourceGpu>;

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

// The veil of every glare source at `texel`, through Rec. 709's weights, in the target's units.
fn meteredVeil(texel : vec2<u32>) -> f32 {
  let size = vec2f(params.size);
  let viewport = vec4f(size, 1.0 / size);
  let pixel = glare_pixel_direction(vec2f(texel) + 0.5, viewport, glare.clipProjection);
  let terms = GlareTerms(glare.poisson0, glare.poisson1, glare.poisson2, glare.broad, glare.misc);
  var veil = 0.0;
  for (var i = 0u; i < params.sourceCount; i++) {
    var source = glareSources[i];
    source.direction = normalize((glare.viewRotation * vec4f(source.direction, 0.0)).xyz);
    veil += source.inside.w * glare_per_unit(source, pixel, terms);
  }
  return veil;
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
      atomicAdd(&local[binOf(dot(colour.rgb, LUMA) + meteredVeil(texel))], weight);
    }
  }
  workgroupBarrier();
  let count = atomicLoad(&local[index]);
  if (count > 0u) {
    atomicAdd(&bins[index], count);
  }
}
