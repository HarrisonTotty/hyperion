// The high setting's cascaded shadows as the lit terrain receives them (plan R10, R10.T9.b; Design
// note 11; decision-r10-t9b-design D5): a texel's lit fraction c′ of the direct sun from the four
// cascades the caster passes drew (`terrainShadow.wgsl`), which `terrainLit.wgsl`'s `shade_terrain`
// takes beside the horizon map by the minimum, as min(1, c′ ÷ m) (decision-r10-t8b §2). Composed
// into the lit material's own source alone, after terrainLitShade.wgsl, whose `Draw` carries the
// cascades' fields; not into `LIT_TERRAIN_SOURCE`, which the depth material and the caster share,
// since WGSL type-checks every function and their `Draw`s have no such fields (D5(a)). The
// TypeScript twin is `shadows/cascadeShadow.ts`, whose `packCascadeUniforms` fills the fields from
// R10.T9.a's `CascadeSet`.
//
// Every place, normal and direction here is camera-relative or a direction in the body's
// non-rotating axes, the frame the terrain's instance origins are drawn in: a body-fixed vector
// turned by the `Draw`'s `bodyRotation`, as the cascades' light matrices take them
// (`cascades.ts`' `Cascade.lightFromCamera`).
//
// - _The cascade_: i, the first whose far end C_(i+1) the texel's distance d along the view axis
//   does not pass, since the splits are distances along that axis (decision-r10-t9b-design's
//   reading of decision-r10-t9-split §3, "by d against the splits"); beyond C₄ the texel is lit.
// - _The blend_: over [fadeStart_i, C_(i+1)], cascade i's last 10% (`CASCADE_FADE_FRACTION`), it
//   blends linearly into cascade i + 1, and the last into lit, so that nothing steps at a split or
//   at C₄.
// - _The overhang_: snapping moves a map by up to half a texel off its slice (R10.T9.a's finding),
//   so where the point with the filter's footprint (CASCADE_FOOTPRINT_TEXELS) falls outside
//   [0, 1]² of cascade j's map, cascade j + 1 stands in, and past the last, lit.
// - _The fit_: the cascades are fitted to the top edge of star 0's disc (decision-r10-t9b-penumbra;
//   `cascades.ts`' `discTopDirection`), so their shadow begins where the horizon map's share m
//   reaches 0, and min(m, c′) is m across the penumbra; non-terrain casters' shadows are short by
//   about half a penumbra, ρL (4.7 cm at 10 m).
// - _The normal offset_ (Holbert 2011, "Saying 'Goodbye' to Shadow Acne"): before each cascade's
//   projection the receiver moves along its geometric normal by that cascade's `normalOffsetM`
//   (`NORMAL_OFFSET_TEXELS` of its texels) × saturate(1 − N·L), L star 0's own direction, its
//   centre's, not the fitted top edge. This is Holbert's basic variant: the offset moves the
//   compared depth as well as the uv, not his "offset in UV space only" refinement.
// - _The filter_: four taps on a disc of one texel (CASCADE_TAP_0 to 3), each a 2 × 2 PCF of the
//   comparison sampler's `linear` filter, averaged. Each is a `textureSampleCompareLevel`, not
//   Design note 11's `textureSampleCompare`: the receiver runs after the survey's discard and the
//   reference test, in non-uniform control flow, where WGSL allows only the former (R10.T9.a's
//   WGSL check; recorded as a T9.b deviation). WebGPU leaves a comparison sampler's filtering to
//   the adapter (§7.1.1), so where it is a single compare the four taps carry the filtering alone.
// - _The weight_: c′ = 1 − w (1 − c), w R10.T9.a's `cascadeWeight` in the view axis's w. At w = 0,
//   the cascades off or not drawn (the low setting, orbit, no star), it returns 1 before any
//   sample, a branch uniform across the draw.
//
// The maps' depth is reversed orthographic, as the caster passes draw it: 1 at a cascade's near
// plane towards the sun, cleared to 0, so the comparison sampler's `greater-equal` lights a texel
// whose depth is at least the nearest caster's. The convention is kept for uniformity only
// (`cascades.ts`' `cascadeMatricesF64`). The reference depth is clamped to [0, 1], as a caster's is
// stored: a receiver beyond the far plane compares at 0 and is lit only where nothing was drawn,
// and one nearer the sun than the near plane at 1, lit.

// The four cascades' layers (`cascades.ts`' CASCADE_TEXTURE, `depth32float`), layer i cascade i,
// nearest first; and the comparison sampler that reads them, linear and `greater-equal`
// (`terrainMaterial.ts`' TERRAIN_LIT_BINDINGS).
@group(2) @binding(19) var shadow_cascades : texture_depth_2d_array;
@group(2) @binding(20) var shadow_compare : sampler_comparison;

// `cascades.ts`' CASCADE_COUNT and CASCADE_SIZE_PX.
const CASCADE_COUNT : u32 = 4u;
const CASCADE_SIZE_PX : f32 = 2048.0;

// The filter's taps, texels from the receiver (`cascadeShadow.ts`' CASCADE_FILTER_TAPS): the
// standard four-sample rotated grid, (−2, −6), (6, −2), (−6, 2) and (2, 6) sixteenths of a pixel
// from its centre (Vulkan's standard sample locations for four samples; Direct3D 11's standard
// multisample pattern), scaled to the unit circle by 1 ÷ √40 sixteenths. Four points of a disc
// whose least distance apart is greatest are a square on its rim, √2 apart, so this is the
// four-point maximin (best-spread) set on a disc of one texel; its mean is 0, so it moves no shadow
// edge, and turned by atan(1 ÷ 3) its four offsets differ along each axis (−3, −1, 1 and 3 of
// 1 ÷ √10 texel, neighbours 2 ÷ √10 ≈ 0.63 texel apart), so that an edge along either axis steps
// through five levels, not three.
const CASCADE_TAP_0 : vec2f = vec2f(-0.316227766, -0.948683298);
const CASCADE_TAP_1 : vec2f = vec2f(0.948683298, -0.316227766);
const CASCADE_TAP_2 : vec2f = vec2f(-0.948683298, 0.316227766);
const CASCADE_TAP_3 : vec2f = vec2f(0.316227766, 0.948683298);

// The filter's reach about the receiver, texels: the taps' radius, 1, and the 2 × 2 PCF's texel
// beyond each (`cascadeShadow.ts`' CASCADE_FOOTPRINT_TEXELS).
const CASCADE_FOOTPRINT_TEXELS : f32 = 2.0;

// The least |cross|² of a fragment's two screen derivatives, m⁴, taken as a face: under it (a
// degenerate triangle, or derivatives lost to rounding) the datum's normal stands in. A pixel at
// the 0.1 m near plane spans some 6 × 10⁻⁵ m at 1080p and 60°, a |cross|² of order 10⁻¹⁷.
const CASCADE_MIN_CROSS_SQUARED : f32 = 1e-30;

// The fragment's geometric normal from the fine screen derivatives `dx` and `dy` of its place, on
// the side of the datum's unit normal `datum`; `datum` where the two give no face. Holbert's offset
// takes the geometric normal, not a shading one.
fn cascade_geometric_normal(dx : vec3f, dy : vec3f, datum : vec3f) -> vec3f {
  let n = cross(dx, dy);
  let squared = dot(n, n);
  if (!(squared > CASCADE_MIN_CROSS_SQUARED)) {
    return datum;
  }
  let unit = n * inverseSqrt(squared);
  return select(unit, -unit, dot(unit, datum) < 0.0);
}

// Cascade j's `lightFromCamera`: a camera-relative place in the non-rotating axes to its clip x,
// y and depth.
fn cascade_light(j : u32) -> mat4x4f {
  switch j {
    case 0u: {
      return draw.cascadeLight0;
    }
    case 1u: {
      return draw.cascadeLight1;
    }
    case 2u: {
      return draw.cascadeLight2;
    }
    default: {
      return draw.cascadeLight3;
    }
  }
}

// The receiver `rel` in cascade j's map, moved along `offset_dir`, the geometric normal ×
// saturate(1 − N·L), by the cascade's normal offset: its uv in x and y, (½ + ½ x, ½ − ½ y) of its
// clip x and y, since a framebuffer's y runs down and a texture's v with it; its depth in z.
fn cascade_map_point(j : u32, rel : vec3f, offset_dir : vec3f) -> vec3f {
  let moved = rel + offset_dir * draw.cascadeNormalOffsetM[j];
  let clip = cascade_light(j) * vec4f(moved, 1.0);
  return vec3f(0.5 + 0.5 * clip.x, 0.5 - 0.5 * clip.y, clip.z);
}

// Whether the map point `at` and the filter's footprint about it lie inside the map.
fn cascade_covers(at : vec3f) -> bool {
  let margin = CASCADE_FOOTPRINT_TEXELS / CASCADE_SIZE_PX;
  return all(at.xy >= vec2f(margin)) && all(at.xy <= vec2f(1.0 - margin));
}

// One tap's 2 × 2 PCF of layer j at `uv`, against the reference depth `depth`.
fn cascade_tap(j : u32, uv : vec2f, tap : vec2f, depth : f32) -> f32 {
  return textureSampleCompareLevel(
    shadow_cascades, shadow_compare, uv + tap / CASCADE_SIZE_PX, j, depth
  );
}

// Cascade j's lit fraction at the map point `at`, the four taps' mean.
fn cascade_sample(j : u32, at : vec3f) -> f32 {
  let depth = saturate(at.z);
  let sum = cascade_tap(j, at.xy, CASCADE_TAP_0, depth) +
    cascade_tap(j, at.xy, CASCADE_TAP_1, depth) +
    cascade_tap(j, at.xy, CASCADE_TAP_2, depth) +
    cascade_tap(j, at.xy, CASCADE_TAP_3, depth);
  return 0.25 * sum;
}

// The first cascade from j whose map covers the receiver, CASCADE_COUNT for none, and its map
// point.
struct CascadeHit {
  index : u32,
  at : vec3f,
}

fn cascade_first_covering(j : u32, rel : vec3f, offset_dir : vec3f) -> CascadeHit {
  var hit = CascadeHit(CASCADE_COUNT, vec3f(0.0));
  for (var k = j; k < CASCADE_COUNT; k = k + 1u) {
    let at = cascade_map_point(k, rel, offset_dir);
    if (cascade_covers(at)) {
      hit = CascadeHit(k, at);
      break;
    }
  }
  return hit;
}

// The lit fraction from cascade j on, the first that covers the receiver, 1 where none does.
fn cascade_lit_from(j : u32, rel : vec3f, offset_dir : vec3f) -> f32 {
  let hit = cascade_first_covering(j, rel, offset_dir);
  if (hit.index >= CASCADE_COUNT) {
    return 1.0;
  }
  return cascade_sample(hit.index, hit.at);
}

// c′, the cascades' lit fraction of the direct sun at the receiver `rel`, camera-relative, with its
// geometric unit normal `normal` and star 0's unit direction `sun` (its centre's, for the offset's
// N·L, not the top edge the cascades are fitted to), all in the body's non-rotating axes
// (`cascadeShadow.ts`' `cascadeReceiverLit`): 1 − w (1 − c), c cascade i's, blended into the
// next over its last 10%, each cascade falling to the next where it does not cover the receiver,
// and 1 at w = 0 and beyond C₄.
fn cascade_lit_fraction(rel : vec3f, normal : vec3f, sun : vec3f) -> f32 {
  let weight = draw.cascadeViewAxis.w;
  if (!(weight > 0.0)) {
    return 1.0;
  }
  let d = dot(rel, draw.cascadeViewAxis.xyz);
  var i = CASCADE_COUNT;
  for (var j = 0u; j < CASCADE_COUNT; j = j + 1u) {
    if (d <= draw.cascadeFarM[j]) {
      i = j;
      break;
    }
  }
  if (i >= CASCADE_COUNT) {
    return 1.0;
  }
  let offset_dir = normal * saturate(1.0 - dot(normal, sun));
  let hit = cascade_first_covering(i, rel, offset_dir);
  if (hit.index >= CASCADE_COUNT) {
    return 1.0;
  }
  var c = cascade_sample(hit.index, hit.at);
  // Only cascade i's own blend band blends: a receiver it does not cover takes a later cascade's
  // value, the next's or beyond, which the blend into i + 1 would only repeat.
  if (hit.index == i) {
    let fade_start = draw.cascadeFadeStartM[i];
    let blend = saturate((d - fade_start) / (draw.cascadeFarM[i] - fade_start));
    if (blend > 0.0) {
      c = mix(c, cascade_lit_from(i + 1u, rel, offset_dir), blend);
    }
  }
  return 1.0 - weight * (1.0 - c);
}
