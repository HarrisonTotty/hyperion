// The detail textures' projection (plan R11, R11.T2.a; the brainstorm's Materials section): a
// world's generated detail set (R11.T2.c, Design note 21) sampled by the triplanar projection,
// three planes blended by the shading normal, or by its biplanar variant, the two planes nearest
// the normal; each plane at two scales, which hide its repetition. Its reference is
// `detailTextures.ts`. Composed after `decoration.wgsl`, whose `DecorationPlace` it reads; it
// declares no binding: the set's array and sampler are its caller's, and the sampler repeats.
//
// - **Coordinates.** Every plane's coordinates are the decoration lattice's, never world-space `f32`
//   positions (Design note 3): along the face's x and y the cell and fraction of the lattice at the
//   set's level, each cell reduced modulo the tile in integers, so that two patches, and two views
//   of one, sample the same texel; along the ground's normal its height in that level's cells, an
//   `f32` reduced modulo the tile.
// - **Two scales.** The first sample tiles every `period` cells; the second is turned by
//   atan(4 ÷ 3), 53.13°, a rotation of integers over 5, and tiles every 3 × `period` cells, each
//   offset by whole cells drawn from the body's seed; their sum ÷ √2 keeps a zero-mean set's
//   variance. The rotation's integers are reduced modulo 15 × `period`, so the turned coordinate
//   is exact at any cell. The brainstorm's lean turns and offsets each patch; a turn per patch would
//   cut the detail at every patch's edge, so the turn and offsets here are the body's (a lean of
//   2026-11-02, the plan's records).
// - **Level of detail.** Each sample's mip is the pixel's footprint at the texel's distance over the
//   texel's width, `textureSampleLevel`'s, the second scale's log₂ 3 coarser, so the projection
//   reads no derivative and runs in any control flow, a compute pass's included.
// - **Weights.** The triplanar weights are |n · axis|⁴, normalised; the biplanar's subtract the
//   least part from the other two before the power (Quilez's biplanar mapping), so that the plane
//   it drops weighs nothing where it changes, and the blend has no seam.
// - **What a set holds.** Each texel's four channels are deviations about the material's mean,
//   zero-mean over the tile: the height's slopes along the tile's two axes, then the albedo's
//   relative change and its hue's (`detailTextureBake.wgsl`). Rotated samples' slopes are turned
//   back to the face's axes. The side planes' slopes add their along-ground part alone.

// The weights' sharpness, the power of each part (a lean).
const DETAIL_SHARPNESS : f32 = 4.0;

// The second scale's tiles a first tile's, and the rotation's denominator.
const DETAIL_SECOND_SCALE : u32 = 3u;
const DETAIL_TURN_DENOMINATOR : u32 = 5u;

// log₂ 3, by which the second scale's mip is coarser.
const DETAIL_SECOND_LOD : f32 = 1.58496250072115618;

const DETAIL_INV_SQRT_TWO : f32 = 0.70710678118654752;

// A detail set's projection (`DetailProjection`): where its tiles lie on the lattice and how they
// are offset.
struct DetailProjection {
  // The tile's period in the set's lattice cells.
  period : u32,
  // The first and second scales' offsets in whole cells, along x then y.
  offsets : vec4u,
  // The texel's height above the datum in the set's cells.
  height_cells : f32,
  // The first scale's mip: log₂ of the pixel's footprint over a texel.
  lod : f32,
}

// `detailCoordinate`: a tile coordinate in [0, 1) of the cell `cell` and fraction `frac` along
// one axis, with a whole offset, at a period of `period` cells.
fn detail_coordinate(cell : u32, frac : f32, offset : u32, period : u32) -> f32 {
  return (f32((cell % period + offset % period) % period) + frac) / f32(period);
}

// `detailFirst`: the first scale's tile coordinates of a lattice place.
fn detail_first(place : DecorationPlace, projection : DetailProjection) -> vec2f {
  return vec2f(
    detail_coordinate(place.cell.x, place.frac.x, projection.offsets.x, projection.period),
    detail_coordinate(place.cell.y, place.frac.y, projection.offsets.y, projection.period),
  );
}

// `detailSecond`: the second scale's tile coordinates, the place turned by (3, 4) ÷ 5 and offset,
// at a period of three tiles, its integers reduced modulo 15 × `period`.
fn detail_second(place : DecorationPlace, projection : DetailProjection) -> vec2f {
  let period = DETAIL_SECOND_SCALE * projection.period;
  let wrap = DETAIL_TURN_DENOMINATOR * period;
  let x = (place.cell.x % wrap + projection.offsets.z % wrap) % wrap;
  let y = (place.cell.y % wrap + projection.offsets.w % wrap) % wrap;
  // (3x − 4y, 4x + 3y), each made non-negative modulo the wrap first.
  let turned_x = (3u * x + 4u * (wrap - y)) % wrap;
  let turned_y = (4u * x + 3u * y) % wrap;
  let fx = 3.0 * place.frac.x - 4.0 * place.frac.y;
  let fy = 4.0 * place.frac.x + 3.0 * place.frac.y;
  return fract(vec2f(f32(turned_x) + fx, f32(turned_y) + fy) / f32(wrap));
}

// The turn back from the second scale's axes to the face's: Rᵀ of R = [[3, −4], [4, 3]] ÷ 5.
fn detail_unturn(slope : vec2f) -> vec2f {
  return vec2f(0.6 * slope.x + 0.8 * slope.y, -0.8 * slope.x + 0.6 * slope.y);
}

// One plane's sample at both scales, its slopes on the plane's own axes: the first scale's, and the
// second's turned back, their sum ÷ √2.
fn detail_plane(
  detail_set : texture_2d_array<f32>,
  samp : sampler,
  layer : i32,
  place : DecorationPlace,
  projection : DetailProjection,
) -> vec4f {
  let a = textureSampleLevel(detail_set, samp, detail_first(place, projection), layer, projection.lod);
  let b = textureSampleLevel(
    detail_set, samp, detail_second(place, projection), layer, projection.lod - DETAIL_SECOND_LOD
  );
  return DETAIL_INV_SQRT_TWO * (a + vec4f(detail_unturn(b.xy), b.zw));
}

// `detailSidePlace`: a side plane's place, one face axis's cell and fraction beside the height in
// cells, reduced modulo the tile.
fn detail_side_place(cell : u32, frac : f32, height_cells : f32, period : u32) -> DecorationPlace {
  let wrapped = height_cells - f32(period) * floor(height_cells / f32(period));
  let whole = floor(wrapped);
  var out : DecorationPlace;
  out.cell = vec2u(cell, min(u32(whole), period - 1u));
  out.frac = vec2f(frac, wrapped - whole);
  return out;
}

// `triplanarWeights`: the weights of the planes facing the face's x, y and the ground's up, by the
// shading normal's parts `along` them.
fn detail_triplanar_weights(along : vec3f) -> vec3f {
  let w = pow(abs(along), vec3f(DETAIL_SHARPNESS));
  return w / max(w.x + w.y + w.z, 1e-6);
}

// `biplanarWeights`: each part less the least part, to the power, normalised: the least's weight 0.
fn detail_biplanar_weights(along : vec3f) -> vec3f {
  let a = abs(along);
  let least = min(a.x, min(a.y, a.z));
  let w = pow(max(a - vec3f(least), vec3f(0.0)), vec3f(DETAIL_SHARPNESS));
  return w / max(w.x + w.y + w.z, 1e-12);
}

// The detail set's deviation at the texel for planes of weights `w` (x, y, up): its slopes along
// the face's x and y, then its albedo's and hue's relative changes, from the top plane (x, y) and
// the side planes (x, height) and (y, height); a plane of no weight takes no sample.
fn detail_projected(
  detail_set : texture_2d_array<f32>,
  samp : sampler,
  layer : i32,
  place : DecorationPlace,
  projection : DetailProjection,
  w : vec3f,
) -> vec4f {
  var out = vec4f(0.0);
  if (w.z > 0.0) {
    out = out + w.z * detail_plane(detail_set, samp, layer, place, projection);
  }
  if (w.y > 0.0) {
    let side_x = detail_plane(
      detail_set, samp, layer,
      detail_side_place(place.cell.x, place.frac.x, projection.height_cells, projection.period),
      projection,
    );
    out = out + vec4f(w.y * side_x.x, 0.0, w.y * side_x.zw);
  }
  if (w.x > 0.0) {
    let side_y = detail_plane(
      detail_set, samp, layer,
      detail_side_place(place.cell.y, place.frac.y, projection.height_cells, projection.period),
      projection,
    );
    out = out + vec4f(0.0, w.x * side_y.x, w.x * side_y.zw);
  }
  return out;
}

// The triplanar projection: three planes, six samples.
fn detail_triplanar(
  detail_set : texture_2d_array<f32>,
  samp : sampler,
  layer : i32,
  place : DecorationPlace,
  projection : DetailProjection,
  along : vec3f,
) -> vec4f {
  return detail_projected(detail_set, samp, layer, place, projection, detail_triplanar_weights(along));
}

// The biplanar variant: two planes, four samples.
fn detail_biplanar(
  detail_set : texture_2d_array<f32>,
  samp : sampler,
  layer : i32,
  place : DecorationPlace,
  projection : DetailProjection,
  along : vec3f,
) -> vec4f {
  return detail_projected(detail_set, samp, layer, place, projection, detail_biplanar_weights(along));
}
