// The wireframe's terrain (plan R10, R10.T11; Design note 14): a body's surveyed patches drawn in
// the wireframe style as the ground's depth, black, with their contours, their coastline and the
// body's graticule written over it in the same pass; nothing else of the ground. Composed after
// frame.wgsl, preciseTrig.wgsl, patchVertex.wgsl, terrainPatch.wgsl and terrainSurvey.wgsl (the lit
// mesh's placement and the survey's discard, which the lit material shares), before one of the two vertex paths
// (`terrainWire.ts`' `terrainWireMaterialSpec`). Its TypeScript twins are `terrainWire.ts`'
// `contourShade` and `graticuleOffsetsRad`.
//
// - The contours. Each vertex's height varies patch-local, h − h₀ for its slot record's origin
//   height h₀, both f32 as the bake and the record hold them, plus the patch's phase
//   φ = h₀ mod 5 CI formed in f64 on the CPU (`contourPhase`), so that u = (h − h₀ + φ) ÷ CI
//   crosses a whole number at each contour and a multiple of five at each index contour, and the
//   f32 height above the datum, which steps 2 mm at ±20 km, never quantises the derivative near
//   the camera. Each line is antialiased at constant width from its distance in pixels,
//   |fract(u − ½) − ½| ÷ length(dpdxFine(u), dpdyFine(u)), the gradient's length rather than
//   `fwidth` (after Golus 2021), the derivatives taken in uniform control flow, before the
//   survey's discard. A family fades from 6 px of its lines' spacing to nothing at 3 px, the minor
//   first, the spacing judged over a coarser footprint than the pixel's: u's change a pixel from
//   each grid vertex to the vertices CONTOUR_FADE_QUADS along its row and its column, as placed for
//   the frame (`wire_footprint`), interpolated, so that a far line whose facets' slopes scatter
//   about the fade's band is drawn whole rather than in pieces; the coastline, at the field's sea
//   level where the world has a sea, is always drawn.
// - The graticule, R02's, on the terrain's own height (Design note 14): each vertex's geocentric
//   latitude and longitude less its patch origin's, from its offset (`wire_graticule_offsets`),
//   plus the origin's less its lines', formed in f64 on the CPU (`graticulePatch`), so that no
//   difference of nearly equal numbers is formed and a line stands still at the ground. Its
//   parallels (not the poles) and meridians at the body's step, the equator and the prime meridian
//   at the heavy width, each cased in `--surface-0` as R02's lines are, over the contours.
// - The skirts and fins, which join the terrain to the reference surface, the occluder sphere, at
//   the survey's edge, are the ground's depth alone: no line is drawn on them.
// - The depth is pushed away from the camera by 2⁻¹⁶ of itself alone (R10.T16.c, the grazing fix):
//   a slope term, as the occluders' (R07.T16.d), grew as the cotangent of the grazing angle, some
//   15 km 287 km out from 10 km up, and let a mark that far behind a ridge show through it. The
//   mark on the ground, the survey edge on its rim, pulls its own depth towards the camera instead
//   (`lineStroke.wgsl`'s `line_depth`), so that it stands in front of the ground it lies on.

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The normals atlas `terrainPatch.wgsl`'s `patchNormalAt` names; unread here, since the
  // wireframe's layout holds no normals.
  atlas : vec4f,
  // x the contour interval CI, m; y the stroke scale, device px drawn for each CSS px of a line;
  // z 1 where the coastline is drawn, a world with a sea, else 0; w 1 ÷ CI, formed in f64, by
  // which the contour coordinate is multiplied, WGSL rounding a product correctly where it allows
  // a quotient 2.5 ULP.
  contour : vec4f,
  // The minor, index and coastline widths, device px; w unused.
  contourWidthsPx : vec4f,
  // x the graticule's step, rad, 0 where the body draws none; y its thin width, z its heavy width
  // and w its casing's on each side, device px.
  graticule : vec4f,
  // `--text-muted`, `--text` and `--surface-0`, linear, alpha 1 (`linearColour`).
  muted : vec4f,
  text : vec4f,
  casing : vec4f,
  // The draw list's `occluderSlopePx`, px; unread since R10.T16.c's grazing fix, which pulls the
  // survey edge's depth towards the camera rather than pushing the terrain's away by its slope.
  occluderSlopePx : f32,
}

@group(1) @binding(0) var<uniform> draw : Draw;

// `uniforms.ts`' WIRE_PHASE_RECORD_BYTES: one record a drawn patch, in the instances' order.
struct WirePhase {
  // x φ = h₀ mod 5 CI, m; y h₀ less the sea level, m; z |P₀|, m; w unused.
  contour : vec4f,
  // x the origin's latitude less its nearest parallel's, rad; y its longitude less its nearest
  // meridian's; z that parallel's index (0 the equator) and w that meridian's (0 the prime one).
  latLon : vec4f,
  // sin φ₀, cos φ₀, sin λ₀ and cos λ₀ of the origin's latitude φ₀ and longitude λ₀.
  frame : vec4f,
}

@group(2) @binding(18) var<storage, read> wire_phases : array<WirePhase>;

// The fade's spacings, CSS px (`terrainWire.ts`' CONTOUR_FADE_PX), its footprint, quads
// (CONTOUR_FADE_QUADS), and the index's period.
const CONTOUR_FULL_PX : f32 = 6.0;
const CONTOUR_GONE_PX : f32 = 3.0;
const CONTOUR_FADE_QUADS : f32 = 4.0;
const CONTOUR_INDEX_EVERY : f32 = 5.0;
// The constant depth push, a fraction of the depth: 2^-16, as the occluders' and `terrainDepth.wgsl`'s.
const WIRE_DEPTH_FRACTION : f32 = 0.0000152587890625;
// π ÷ 2 and 2π.
const HALF_PI : f32 = 1.5707963267948966;
const TWO_PI : f32 = 6.283185307179586;

struct WireVertexOut {
  @builtin(position) clip : vec4f,
  // The vertex's place in its patch, (x, y) ÷ 64.
  @location(0) patchUv : vec2f,
  @location(1) @interpolate(flat) slot : u32,
  // The triangle's part of the lit mesh, its first vertex's code (LIT_GRID to LIT_ROW_FIN).
  @location(2) @interpolate(flat) kind : u32,
  // The contour coordinate u = (h − h₀ + φ) ÷ CI.
  @location(3) contourU : f32,
  // The height above the sea level, m, patch-local: h − h₀ + (h₀ − sea level).
  @location(4) aboveSeaM : f32,
  // The latitude and longitude less their nearest lines' at the patch origin, over the step.
  @location(5) graticuleUv : vec2f,
  // The patch origin's nearest parallel's and meridian's indices.
  @location(6) @interpolate(flat) graticuleLines : vec2f,
  // How far u changes a device pixel over the footprint at the grid vertex (`wire_footprint`), a
  // skirt's or a fin's that it hangs from.
  @location(7) fadePerPx : f32,
}

// atan2(y, x) of either sign of y, through preciseTrig.wgsl's `preciseAtan2` (R08's Design note
// 18): WGSL's own `atan2` is bounded only to 4,096 ULP, which at a coarse patch's offsets would move
// a graticule line by about a metre on the Moon.
fn wire_atan2(y : f32, x : f32) -> f32 {
  return select(1.0, -1.0, y < 0.0) * preciseAtan2(abs(y), x);
}

// A point's geocentric latitude and longitude less its patch origin's, rad, from its offset `q`
// from the origin, body-fixed (`terrainWire.ts`' `graticuleOffsetsRad`): in the frame of east,
// north and up at the origin, (e, n, w), Δλ = atan2(e, C) for C = w cos φ₀ − n sin φ₀, and
// Δφ = atan2(n − δ sin φ₀, w + δ cos φ₀) for δ = e² ÷ (B + C), B = √(C² + e²).
fn wire_graticule_offsets(phase : WirePhase, q : vec3f) -> vec2f {
  let sinLat = phase.frame.x;
  let cosLat = phase.frame.y;
  let sinLon = phase.frame.z;
  let cosLon = phase.frame.w;
  let e = -sinLon * q.x + cosLon * q.y;
  let n = -sinLat * cosLon * q.x - sinLat * sinLon * q.y + cosLat * q.z;
  let w = phase.contour.z + cosLat * cosLon * q.x + cosLat * sinLon * q.y + sinLat * q.z;
  let c = w * cosLat - n * sinLat;
  let b = sqrt(c * c + e * e);
  let dLon = wire_atan2(e, c);
  if (!(c > 0.0)) {
    return vec2f(wire_atan2(w * sinLat + n * cosLat, b) - wire_atan2(sinLat, cosLat), dLon);
  }
  let delta = (e * e) / (b + c);
  return vec2f(wire_atan2(n - delta * sinLat, w + delta * cosLat), dLon);
}

// The grid index CONTOUR_FADE_QUADS from `i` along a row or a column, inside the patch
// (`terrainWire.ts`' `contourFadeNeighbour`).
fn wire_fade_neighbour(i : f32) -> f32 {
  return select(i - CONTOUR_FADE_QUADS, i + CONTOUR_FADE_QUADS, i + CONTOUR_FADE_QUADS <= 64.0);
}

// A grid vertex of the footprint, as placed for the frame: its clip position and its contour
// coordinate.
struct WireFootprintVertex {
  clip : vec4f,
  u : f32,
}

// Grid vertex `grid` of `instance`, its u from the patch's h₀ and φ.
fn wire_footprint_vertex(
  grid : vec2f,
  instance : u32,
  h0 : f32,
  phase : f32,
) -> WireFootprintVertex {
  let v = patchVertexOf(vec3f(grid, 0.0), instance);
  var out : WireFootprintVertex;
  out.clip = frame.clipProjection * frame.viewRotation * vec4f(v.fromCameraM, 1.0);
  out.u = (v.heightM - h0 + phase) * draw.contour.w;
  return out;
}

// A clip position's place on the screen, device px about its centre.
fn wire_screen_px(clip : vec4f) -> vec2f {
  return clip.xy / clip.w * 0.5 * frame.viewport.xy;
}

// How far a value changes a device pixel over a footprint whose two steps on the screen, px, are
// `a` and `b` and across which it changes by `dv_a` and `dv_b` (`terrainWire.ts`'
// `footprintPerPx`): the length of the screen gradient g with Δv = g · Δs for both, never less than
// its change a pixel along either, which bounds |g| from below and stands for it where the
// footprint is seen edge-on, its two steps on one line of the screen.
fn wire_footprint_slope(a : vec2f, b : vec2f, dv_a : f32, dv_b : f32) -> f32 {
  let det = max(abs(a.x * b.y - a.y * b.x), 1e-30);
  let along = max(abs(dv_a) / max(length(a), 1e-30), abs(dv_b) / max(length(b), 1e-30));
  return max(length(vec2f(dv_a * b.y - dv_b * a.y, a.x * dv_b - b.x * dv_a)) / det, along);
}

// How far u changes a device pixel over the footprint at grid vertex `grid` of `instance`, whose
// clip position is `clip` and coordinate `u`, to the grid vertices CONTOUR_FADE_QUADS from it along
// its row and its column, which fades the contours; 0 where any of the three lies behind the camera,
// ground a few quads from it, where no line stands close to the next.
fn wire_footprint(
  grid : vec2f,
  instance : u32,
  clip : vec4f,
  u : f32,
  h0 : f32,
  phase : f32,
) -> f32 {
  let along_row = vec2f(wire_fade_neighbour(grid.x), grid.y);
  let along_column = vec2f(grid.x, wire_fade_neighbour(grid.y));
  let row = wire_footprint_vertex(along_row, instance, h0, phase);
  let column = wire_footprint_vertex(along_column, instance, h0, phase);
  if (!(clip.w > 0.0 && row.clip.w > 0.0 && column.clip.w > 0.0)) {
    return 0.0;
  }
  let s0 = wire_screen_px(clip);
  let a = wire_screen_px(row.clip) - s0;
  let b = wire_screen_px(column.clip) - s0;
  return wire_footprint_slope(a, b, row.u - u, column.u - u);
}

@vertex
fn vertexMain(@location(0) grid : vec3f, @builtin(instance_index) instance : u32) -> WireVertexOut {
  let placed = lit_place(grid, instance);
  let v = placed.vertex;
  let rec = slots[v.slot];
  let phase = wire_phases[instance];
  var out : WireVertexOut;
  out.clip = frame.clipProjection * frame.viewRotation * vec4f(placed.fromCameraM, 1.0);
  out.patchUv = v.patchUv;
  out.slot = v.slot;
  out.kind = placed.kind;
  // The grid vertex's height, patch-local: a skirt or fin draws no line.
  let local = v.heightM - rec.h0M;
  out.contourU = (local + phase.contour.x) * draw.contour.w;
  out.aboveSeaM = local + phase.contour.y;
  let step = max(draw.graticule.x, 1e-30);
  out.graticuleUv = (phase.latLon.xy + wire_graticule_offsets(phase, v.offsetM)) / step;
  out.graticuleLines = phase.latLon.zw;
  // The footprint at the grid vertex, a skirt's or a fin's that it hangs from.
  let gridClip = frame.clipProjection * frame.viewRotation * vec4f(v.fromCameraM, 1.0);
  out.fadePerPx =
    wire_footprint(grid.xy, instance, gridClip, out.contourU, rec.h0M, phase.contour.x);
  return out;
}

// The coverage of a line of `width_px` at `distance_px` from its centre, as R02's lines take it.
fn wire_line_coverage(distance_px : f32, width_px : f32) -> f32 {
  return clamp(width_px * 0.5 + 0.5 - distance_px, 0.0, 1.0);
}

// The distance from the nearest whole number of `u`, which changes by `per_px` a pixel, px.
fn wire_line_distance_px(u : f32, per_px : f32) -> f32 {
  return abs(fract(u - 0.5) - 0.5) / max(per_px, 1e-30);
}

// A family's opacity at its lines' spacing, device px (`terrainWire.ts`' `contourFade`).
fn wire_contour_fade(spacing_px : f32) -> f32 {
  let scale = draw.contour.y;
  return clamp(
    (spacing_px - CONTOUR_GONE_PX * scale) / ((CONTOUR_FULL_PX - CONTOUR_GONE_PX) * scale),
    0.0,
    1.0,
  );
}

// `x` modulo `m`, for a whole `x` of either sign and a whole positive `m`, in [0, m).
fn wire_modulo(x : f32, m : f32) -> f32 {
  return x - m * floor(x / m);
}

// A fragment's contours over black (`terrainWire.ts`' `contourShade`): the minor and index
// families in `--text-muted`, each antialiased by the fragment's own gradient and faded by its
// spacing over the fade's footprint, then the coastline in `--text`.
fn wire_contours(in : WireVertexOut, u_per_px : f32, sea_per_px : f32) -> vec3f {
  let u = in.contourU;
  let on_index = wire_modulo(round(u), CONTOUR_INDEX_EVERY) == 0.0;
  let minor_spacing = 1.0 / max(in.fadePerPx, 1e-30);
  var minor = 0.0;
  if (!on_index) {
    minor = wire_line_coverage(wire_line_distance_px(u, u_per_px), draw.contourWidthsPx.x) *
      wire_contour_fade(minor_spacing);
  }
  let index = wire_line_coverage(
    wire_line_distance_px(u / CONTOUR_INDEX_EVERY, u_per_px / CONTOUR_INDEX_EVERY),
    draw.contourWidthsPx.y,
  ) * wire_contour_fade(CONTOUR_INDEX_EVERY * minor_spacing);
  var colour = mix(vec3f(0.0), draw.muted.rgb, max(minor, index));
  if (draw.contour.z > 0.5) {
    let coast = wire_line_coverage(abs(in.aboveSeaM) / max(sea_per_px, 1e-30), draw.contourWidthsPx.z);
    colour = mix(colour, draw.text.rgb, coast);
  }
  return colour;
}

// The graticule over `colour`: its casing, then its stroke, each the larger of a parallel's and a
// meridian's at the fragment.
fn wire_graticule(colour : vec3f, in : WireVertexOut, lat_per_px : f32, lon_per_px : f32) -> vec3f {
  let step = draw.graticule.x;
  if (!(step > 0.0)) {
    return colour;
  }
  let thin = draw.graticule.y;
  let heavy = draw.graticule.z;
  let casing = draw.graticule.w;
  // The parallel nearest the fragment: none at a pole, where no parallel is drawn.
  let parallel = in.graticuleLines.x + round(in.graticuleUv.x);
  let lat_distance = wire_line_distance_px(in.graticuleUv.x, lat_per_px);
  let lat_width = select(thin, heavy, parallel == 0.0);
  let at_pole = abs(parallel * step) >= HALF_PI - 1e-4;
  // The meridian nearest it, the prime meridian at the heavy width.
  let meridians = round(TWO_PI / step);
  let meridian = wire_modulo(in.graticuleLines.y + round(in.graticuleUv.y), meridians);
  let lon_distance = wire_line_distance_px(in.graticuleUv.y, lon_per_px);
  let lon_width = select(thin, heavy, meridian == 0.0);
  var cased = wire_line_coverage(lon_distance, lon_width + 2.0 * casing);
  var stroke = wire_line_coverage(lon_distance, lon_width);
  if (!at_pole) {
    cased = max(cased, wire_line_coverage(lat_distance, lat_width + 2.0 * casing));
    stroke = max(stroke, wire_line_coverage(lat_distance, lat_width));
  }
  return mix(mix(colour, draw.casing.rgb, cased), draw.muted.rgb, stroke);
}

struct WireFragment {
  @location(0) colour : vec4f,
  @builtin(frag_depth) depth : f32,
}

@fragment
fn fragmentMain(in : WireVertexOut) -> WireFragment {
  // Every derivative in uniform control flow, before the survey's discard.
  let u_per_px = length(vec2f(dpdxFine(in.contourU), dpdyFine(in.contourU)));
  let sea_per_px = length(vec2f(dpdxFine(in.aboveSeaM), dpdyFine(in.aboveSeaM)));
  let lat_per_px = length(vec2f(dpdxFine(in.graticuleUv.x), dpdyFine(in.graticuleUv.x)));
  let lon_per_px = length(vec2f(dpdxFine(in.graticuleUv.y), dpdyFine(in.graticuleUv.y)));
  if (!survey_kept_at(in.slot, in.patchUv, in.kind)) {
    discard;
  }
  var colour = vec3f(0.0);
  if (in.kind == LIT_GRID) {
    colour = wire_graticule(wire_contours(in, u_per_px, sea_per_px), in, lat_per_px, lon_per_px);
  }
  var out : WireFragment;
  out.colour = vec4f(colour, 1.0);
  // Away is smaller under reversed-Z: by the constant fraction alone, the grid, the skirts and the
  // fins alike, so that a skirt beneath the ground stays behind it (R10.T16.c; R10.T11's skirts).
  out.depth = clamp(in.clip.z * (1.0 - WIRE_DEPTH_FRACTION), 0.0, 1.0);
  return out;
}
