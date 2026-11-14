// The world's generated detail sets (plan R11, R11.T2.c; Design note 21): for each material a
// body's field names, one layer of a `texture_2d_array` of slopes, albedo and hue deviations,
// generated from the material's form, its palette entry's values and the body's conditions, keyed
// by the body's decoration seed and the material, and never authored. Composed after
// `decoration.wgsl`, whose hash it keys by; `detailTextureBake.ts` holds the mapping from a
// material and a body to `DetailParams` and the kernels' entries, which follow this module:
//
// - **generate**: each texel's height (m), relative albedo and hue deviation, raw, over a tile of
//   `DETAIL_TEXELS`² that repeats, every lattice taken modulo its cells across the tile;
// - **mean**: the tile's mean albedo and hue deviation, one workgroup in a fixed order;
// - **level 0**: each texel's slopes, the height's central differences across the tile's period,
//   and its albedo and hue less their means, so that the layer is zero-mean about the class albedo
//   and the shading normal (Design note 3), into the layer's level 0 and the mip chain's buffer;
// - **mip**: each finer level's 2 × 2 means into the next, a zero-mean level's means zero-mean.
//
// Every texel of a layer is a function of its params and its place alone, summed in a fixed
// order, so that one body gives byte-equal sets in two runs on one GPU; not across GPUs, as
// decoration need not be (Design note 3).

const DETAIL_TEXELS : u32 = 256u;

// The families of `DETAIL_FAMILY` (`detailTextureBake.ts`).
const DETAIL_GRAINS : u32 = 0u;
const DETAIL_FRACTURED : u32 = 1u;
const DETAIL_VESICULAR : u32 = 2u;
const DETAIL_FLOW_BANDED : u32 = 3u;
const DETAIL_LAMINAE : u32 = 4u;
const DETAIL_CRYSTALLINE : u32 = 5u;
const DETAIL_PLATES : u32 = 6u;
const DETAIL_POLYGONS : u32 = 7u;
const DETAIL_SKIN : u32 = 8u;

const DETAIL_TAU : f32 = 6.28318530717958648;

// One layer's generator (`DetailParams`), 16 words.
struct DetailParams {
  seed : u32,
  family : u32,
  // The layer the write kernels write, and the level.
  layer : u32,
  level : u32,
  // A texel's width, m.
  texel_m : f32,
  // The features' cells and the speckle's cells across the tile, each a power of 2 to 256.
  cells : u32,
  fine_cells : u32,
  // The relief's height scale, m.
  relief_m : f32,
  // The albedo's and the hue's relative spreads.
  albedo_spread : f32,
  hue_spread : f32,
  // The share of cells that hold a feature: clasts, vesicles.
  feature_share : f32,
  // The share of grains that are mature (weathered), 0 off an airless body.
  weathered : f32,
  // 0 angular to 1 rounded, and 0 poorly to 1 well sorted.
  rounding : f32,
  sorting : f32,
  // The bands' turns across the tile along x and y, whole, for the banded families.
  bands : vec2i,
}

// A periodic lattice point's hash: (x, y) modulo `cells`.
fn detail_point(prefix : u32, x : i32, y : i32, cells : u32) -> u32 {
  let n = i32(cells);
  let px = u32(((x % n) + n) % n);
  let py = u32(((y % n) + n) % n);
  return decoration_point(prefix, px, py);
}

// A periodic value noise of `cells` across the tile at tile coordinates `t` ∈ [0, 1)², in
// [−1, 1], quintic between its lattice's values.
fn detail_value(prefix : u32, t : vec2f, cells : u32) -> f32 {
  let p = t * f32(cells);
  let c = floor(p);
  let f = p - c;
  let ci = vec2i(c);
  let v00 = decoration_upper(detail_point(prefix, ci.x, ci.y, cells));
  let v10 = decoration_upper(detail_point(prefix, ci.x + 1, ci.y, cells));
  let v01 = decoration_upper(detail_point(prefix, ci.x, ci.y + 1, cells));
  let v11 = decoration_upper(detail_point(prefix, ci.x + 1, ci.y + 1, cells));
  let q = decoration_quintic(f);
  return 2.0 * mix(mix(v00, v10, q.x), mix(v01, v11, q.x), q.y) - 1.0;
}

// The nearest two features of a periodic cellular field of `cells` across the tile, in cells:
// F1, F2 and the nearest feature's hash. A cell's feature lies at its hash's offset.
struct DetailCells {
  f1 : f32,
  f2 : f32,
  id : u32,
}

fn detail_cells(prefix : u32, t : vec2f, cells : u32) -> DetailCells {
  let p = t * f32(cells);
  let c = vec2i(floor(p));
  var out = DetailCells(1e9, 1e9, 0u);
  for (var dy = -1; dy <= 1; dy = dy + 1) {
    for (var dx = -1; dx <= 1; dx = dx + 1) {
      let at = c + vec2i(dx, dy);
      let h = detail_point(prefix, at.x, at.y, cells);
      let feature = vec2f(at) + vec2f(decoration_lower(h), decoration_upper(h));
      let d = length(p - feature);
      if (d < out.f1) {
        out.f2 = out.f1;
        out.f1 = d;
        out.id = h;
      } else if (d < out.f2) {
        out.f2 = d;
      }
    }
  }
  return out;
}

// A feature's radius in cells, at most ½ so that it lies in its own cell's reach: ½ less a spread
// of sizes that sorting narrows.
fn detail_radius(hash : u32, sorting : f32) -> f32 {
  let size = decoration_lower(decoration_mix(hash ^ 0x5bd1e995u));
  return 0.5 * (1.0 - (1.0 - sorting) * 0.75 * size);
}

// Whether a feature is present: its hash's fraction under the share.
fn detail_present(hash : u32, share : f32) -> bool {
  return decoration_upper(decoration_mix(hash ^ 0x27d4eb2fu)) < share;
}

// A feature's own deviation in [−1, 1].
fn detail_tone(hash : u32) -> f32 {
  return 2.0 * decoration_lower(decoration_mix(hash ^ 0x165667b1u)) - 1.0;
}

// Whether a grain is mature: its hash's fraction under the weathered share.
fn detail_mature(hash : u32, weathered : f32) -> f32 {
  return select(0.0, 1.0, decoration_upper(decoration_mix(hash ^ 0x9e3779b9u)) < weathered);
}

// A bump of radius r (cells) at distance d: a hemisphere's when rounded, a cone's when angular.
fn detail_bump(d : f32, r : f32, rounding : f32) -> f32 {
  if (!(d < r)) {
    return 0.0;
  }
  let rho = d / r;
  return r * mix(1.0 - rho, sqrt(max(1.0 - rho * rho, 0.0)), rounding);
}

// A crack's depth share at a cellular field's edge, F2 − F1 under `width` cells.
fn detail_crack(cells : DetailCells, width : f32) -> f32 {
  return 1.0 - smoothstep(0.0, width, cells.f2 - cells.f1);
}

// The raw texel (height m, albedo, hue, 0) of `params` at tile coordinates `t`.
fn detail_texel(params : DetailParams, t : vec2f) -> vec4f {
  let feature_prefix = decoration_prefix(params.seed, 11u, 0u, 0u);
  let fine_prefix = decoration_prefix(params.seed, 12u, 0u, 0u);
  let warp_prefix = decoration_prefix(params.seed, 13u, 0u, 0u);
  let fine = detail_value(fine_prefix, t, params.fine_cells);
  // Every family's fine speckle: sub-texel grains' mottle, a tenth of the relief.
  var height = 0.1 * params.relief_m * fine;
  var albedo = 0.5 * params.albedo_spread * fine;
  var hue = 0.0;
  switch (params.family) {
    case DETAIL_GRAINS: {
      let c = detail_cells(feature_prefix, t, params.cells);
      let r = detail_radius(c.id, params.sorting);
      if (detail_present(c.id, params.feature_share)) {
        let bump = detail_bump(c.f1, r, params.rounding);
        height = height + params.relief_m * bump / 0.5;
        if (bump > 0.0) {
          albedo = albedo + params.albedo_spread * detail_tone(c.id);
        }
      }
      // A grain's maturity, mature grains darker and redder: the hue channel carries it whole.
      let grains = detail_cells(fine_prefix, t, params.fine_cells);
      hue = detail_mature(grains.id, params.weathered) - params.weathered;
    }
    case DETAIL_FRACTURED: {
      let c = detail_cells(feature_prefix, t, params.cells);
      let crack = detail_crack(c, 0.06);
      let rough = detail_value(warp_prefix, t, params.cells * 8u);
      height = height + params.relief_m * (0.3 * rough - crack);
      albedo = albedo + params.albedo_spread * (0.6 * detail_tone(c.id) - 0.8 * crack);
    }
    case DETAIL_VESICULAR: {
      let c = detail_cells(feature_prefix, t, params.cells);
      let r = detail_radius(c.id, params.sorting);
      let rough = detail_value(warp_prefix, t, params.cells / 2u);
      height = height + 0.3 * params.relief_m * rough;
      if (detail_present(c.id, params.feature_share)) {
        let pit = detail_bump(c.f1, r, 1.0);
        height = height - params.relief_m * pit / 0.5;
        albedo = albedo - params.albedo_spread * pit / 0.5;
      }
    }
    case DETAIL_FLOW_BANDED, DETAIL_LAMINAE: {
      let warp = detail_value(warp_prefix, t, 4u);
      let phase = DETAIL_TAU * (f32(params.bands.x) * t.x + f32(params.bands.y) * t.y) + 1.5 * warp;
      var band = sin(phase);
      if (params.family == DETAIL_LAMINAE) {
        // Laminae: sharp-edged beds, each of its own tone.
        band = smoothstep(-0.2, 0.2, band) * 2.0 - 1.0;
      }
      height = height + 0.3 * params.relief_m * band;
      albedo = albedo + params.albedo_spread * band;
      hue = 0.5 * band;
    }
    case DETAIL_CRYSTALLINE: {
      let c = detail_cells(feature_prefix, t, params.cells);
      // Facets: the cones about each crystal's centre, flat to first order, each its own tilt.
      height = height - params.relief_m * c.f1;
      albedo = albedo + params.albedo_spread * detail_tone(c.id);
    }
    case DETAIL_PLATES, DETAIL_POLYGONS: {
      let c = detail_cells(feature_prefix, t, params.cells);
      let width = select(0.15, 0.05, params.family == DETAIL_POLYGONS);
      let ridge = detail_crack(c, width);
      let dome = select(0.0, 0.3 * (1.0 - min(c.f1, 1.0)), params.family == DETAIL_POLYGONS);
      height = height + params.relief_m * (ridge + dome);
      albedo = albedo + params.albedo_spread * (0.5 * detail_tone(c.id) + ridge);
    }
    case DETAIL_SKIN: {
      let c = detail_cells(feature_prefix, t, params.cells);
      let crack = detail_crack(c, 0.04);
      let swell = detail_value(warp_prefix, t, params.cells * 2u);
      height = height + params.relief_m * (0.5 * swell - crack);
      albedo = albedo + params.albedo_spread * (0.3 * swell - crack);
    }
    default: {}
  }
  return vec4f(height, albedo, params.hue_spread * hue, 0.0);
}
