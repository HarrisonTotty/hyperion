// The composite's census over its output (plan R08, perf-composite-close; check-only, the inspection
// profile's): each pixel of the frame classed by the terms `composite.wgsl`'s `fragmentMain` forms
// for it, from the march's own uniforms and the scene's depth, so that a profile names each term's
// share of the pixels beside its time. Dispatched over the output in tiles of 8 x 8 pixels beside the
// march's census (`marchProbe.ts`' COMPOSITE_CENSUS_KERNEL), the census zeroed before it. Follows
// `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `view.wgsl` and `marchArea.wgsl`.
//
// It classes a pixel by what the census binds alone: the composite's read at the march's
// resolution is counted where it is tried, the setting's `march` (`levers.x` on both settings),
// though it falls back to the pixel's own where the four texels are thick and what lies behind is
// bright (`behindAtMarch`'s `thin` and `dim`), and under the `per-pixel` way back or a probe that
// forms it otherwise it is counted as drawn; a deck's route (`levers.z`) is not read; and the
// march's view holds the march's sources alone, at most its cap, so that a disc past them is not
// counted.

// The census (`marchProbe.ts`' CompositeCensus, word for word): the output's pixels; those that
// return at once from above the top, outside the march's area with no disc on them; the sky's
// pixels composited, and of those from above the top the ones whose behind is read at the march's
// resolution and the ones that form it, the far-limb footprint and its lookups, per pixel; the
// surfaces' pixels, and of those the ones that read the aerial volume; the pixels whose drawn edge
// is searched (`rayMarchEdgeOf`); and the sky's pixels within a drawn source's disc, whose light
// the composite adds.
struct CompositeCensus {
  pixels : atomic<u32>,
  outside : atomic<u32>,
  sky : atomic<u32>,
  skyFromMarch : atomic<u32>,
  skyPerPixel : atomic<u32>,
  surface : atomic<u32>,
  aerial : atomic<u32>,
  edges : atomic<u32>,
  discs : atomic<u32>,
}

@group(0) @binding(0) var<uniform> medium : Medium;
@group(0) @binding(1) var<uniform> view : AtmosphereView;
@group(0) @binding(2) var<uniform> marchArea : MarchArea;
@group(0) @binding(3) var sceneDepth : texture_depth_2d;
@group(0) @binding(4) var<storage, read_write> census : CompositeCensus;

// Whether the march wrote the texels the pixel reads (`composite.wgsl`'s `marchedHere`), its
// area the march's tiles held to its target.
fn censusMarchedHere(pixel : vec2u, scale : f32, size : vec2i) -> bool {
  var low = vec2i(pixel);
  var high = low;
  if (scale < 1.0) {
    let base = vec2i(vec2f(pixel) * scale - 0.5);
    low = clamp(base, vec2i(0), size - 1);
    high = clamp(base + vec2i(1), vec2i(0), size - 1);
  }
  let first = vec2i(marchArea.tiles.xy) * i32(MARCH_TILE_TEXELS);
  let past = min(first + vec2i(marchArea.tiles.zw) * i32(MARCH_TILE_TEXELS), vec2i(marchArea.bound.zw));
  return all(low >= first) && all(high < past);
}

// Whether `dir` falls within any of the view's sources' discs (`composite.wgsl`'s `withinDisc`), in
// `x`, which keeps the sky from the march's resolution; and within one whose disc the composite
// draws, its light above 0 (`anyDiscAlong`), in `y`.
fn censusDiscsAlong(dir : vec3f) -> vec2<bool> {
  var within = false;
  var drawn = false;
  for (var k = 0u; k < skySourceCount(view); k++) {
    let source = view.sources[k];
    let s = source.direction.xyz;
    let sinRadius = source.direction.w;
    if (sinRadius > 0.0 && dot(dir, s) > 0.0 && length(cross(dir, s)) <= sinRadius) {
      within = true;
      drawn = drawn || dot(source.disc.rgb, vec3f(1.0)) > 0.0;
    }
  }
  return vec2<bool>(within, drawn);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let full = vec2u(view.output.xy);
  let pixel = id.xy;
  if (pixel.x >= full.x || pixel.y >= full.y) {
    return;
  }
  atomicAdd(&census.pixels, 1u);
  let scale = view.output.z;
  let size = vec2i(ceil(view.output.xy * scale));
  let aboveTop = view.camera.w * view.curvature.z >= medium.topHeightM;
  let marchWeight = view.exact.w;
  let uv = (vec2f(pixel) + 0.5) / view.output.xy;
  let dir = rayThrough(view, uv);
  let here = censusMarchedHere(pixel, scale, size);
  let discs = censusDiscsAlong(dir);
  if (aboveTop && marchWeight >= 1.0 && !here && !discs.y) {
    atomicAdd(&census.outside, 1u);
    return;
  }
  let depth = textureLoad(sceneDepth, pixel, 0);
  // The drawn edge's search (`rayMarchTexelFrom`'s `edge`, `rayMarchEdgeOf`'s `across`): at half
  // resolution, from above the top, where the four candidates' source pixels are some sky, some not.
  if (here && aboveTop && scale < 1.0) {
    let base = vec2i(vec2f(pixel) * scale - 0.5);
    var skies = 0u;
    for (var j = 0; j < 2; j++) {
      for (var i = 0; i < 2; i++) {
        let t = vec2u(clamp(base + vec2i(i, j), vec2i(0), size - 1));
        let source = min(vec2u((vec2f(t) + 0.5) / scale), full - 1u);
        skies += select(0u, 1u, textureLoad(sceneDepth, source, 0) == 0.0);
      }
    }
    if (skies > 0u && skies < 4u) {
      atomicAdd(&census.edges, 1u);
    }
  }
  if (depth == 0.0) {
    atomicAdd(&census.sky, 1u);
    if (discs.y) {
      atomicAdd(&census.discs, 1u);
    }
    if (aboveTop && here) {
      // `behindAtMarch`'s read, tried where the frame forms the march's resolution's target, the
      // ray is in no source's disc and its four texels around the pixel's centre are all sky.
      var fromMarch = scale < 1.0 && marchWeight >= 1.0 && !discs.x;
      let at = (vec2f(pixel) + 0.5) * scale - 0.5;
      let base = vec2i(floor(at));
      for (var j = 0; j < 2; j++) {
        for (var i = 0; i < 2; i++) {
          let t = vec2u(clamp(base + vec2i(i, j), vec2i(0), size - 1));
          let source = min(vec2u((vec2f(t) + 0.5) / scale), full - 1u);
          fromMarch = fromMarch && textureLoad(sceneDepth, source, 0) == 0.0;
        }
      }
      if (fromMarch) {
        atomicAdd(&census.skyFromMarch, 1u);
      } else {
        atomicAdd(&census.skyPerPixel, 1u);
      }
    }
    return;
  }
  atomicAdd(&census.surface, 1u);
  if (distanceAtDepth(view, uv, depth) <= view.tables.y && marchWeight < 1.0) {
    atomicAdd(&census.aerial, 1u);
  }
}
