// The lit style on generated terrain (plan R10, R10.T10.b; Design notes 8, 9 and 11; R08 Design
// note 17): each texel of a body's patch reflects its palette's classes' `body_brdf` radiances
// summed by its class weights, each class through its own `LunarLambert` and its own row of the
// phase table, never one law of weighted parameters (decision-r07-t8b), lit as R07's disc lights
// a body (`bodyDisc.wgsl`'s `shade`): each star through the horizon term on the terrain's own
// normal, the eclipse term of up to two occluders and R08's `atmosphere_sun_transmittance`, the
// sky through `atmosphere_sky_irradiance`, and up to two lit neighbours' planetshine through
// `atmosphere_source_transmittance`. The aerial perspective is no part of this shader: the lit
// terrain is drawn into the `AtmosphereScene` that `HillaireAtmosphere.drawFrame` composites.
// The TypeScript twin of the blend is `terrainMaterial.ts`' `terrainReflectance`.
//
// Composed after frame.wgsl, patchVertex.wgsl, R08's surface lighting (`SURFACE_LIGHTING_WGSL`:
// preciseTrig.wgsl, common.wgsl, oblate.wgsl, surfaceLighting.wgsl), R07's litBody.wgsl,
// terrainPatch.wgsl and the horizon map's lookup (`shadows/horizon.wgsl`, R10.T8.b), with one of
// the two vertex paths (`terrainMaterial.ts`).
//
// Each star's horizon term takes its local horizon from the patch's horizon map (R10.T8.b; Design
// note 10): `terrain_horizon` looks the horizon up towards the star's grid azimuth, blends it to the
// parent's map by the morph factor, and carries it into the element's frame, as R07's
// `sphere_irradiance(h, phi, horizon)` takes it (used unchanged), whose visible fraction of the disc
// is then Design note 10's lit fraction about the horizon. R08 draws no refraction yet, so the star's
// direction is the geometric one. Planetshine is not shadowed by the map (R10's Risks).
//
// Each lookup of R08's takes the texel's geodetic height above the datum (the baked heights,
// mixed by the morph factor, never |P| − R), its geodetic latitude from the datum's normal ν
// against the pole (body-fixed +z), and the source's cosine against ν, not against the terrain's
// normal, with its azimuth from local north (Design note 17), and the light's curve slot, 0 for a
// planetshine source.
//
// Every light and occluder reaches the GPU relative to the body's centre over its equatorial
// radius a, body-fixed, and the texel's place is formed from the camera's body-fixed place and the
// vertex's offset from the camera, both in f32: their metre-scale rounding moves only the lighting's
// geometry, never the drawn position (`terrainPatch.wgsl`'s rule).
//
// The ground's thermal emission (`Melt`'s, and any texel's above R08.T18's visibility cut) is not
// drawn: it waits on R08.T18's shared function (decision-composition).
//
// The survey's edge (R10.T10.c, Design note 12): a fragment of the terrain outside the survey is
// discarded, so that R07's reference surface, drawn beneath with the inverse of the GPU's coverage
// mask (`smoothMesh.wgsl`), shows there, and each coarse cell is the terrain's or the reference
// surface's, never both. The state of the fragment's cell is the bake's survey mask at that cell's
// centre vertex (`survey_surveyed`; `gpu/resources.ts`' `surveyedAt`), since a vertex on a cell's
// edge holds either cell's; the mask's half between the quad's vertices stands in where a cell spans
// one quad or less. Where the two surfaces meet at different heights, the lit mesh's skirts and fins
// (`gpu/resources.ts`' `litPatchMeshData`) join them, each drawn by the side whose surface stands
// higher above it, in the reference surface's appearance: the body's uniform law (law 0) under the
// datum's normal, without the horizon map's shadows, as R07's disc draws the reference surface.

struct Draw {
  // Zero: each instance carries its own origin.
  offsetFromCameraM : vec3f,
  // The body-fixed axes into the frame the instance origins are in (the body's rotation).
  bodyRotation : mat4x4f,
  // The camera less the body's centre, body-fixed, metres; w, 1 ÷ a, a the equatorial radius.
  cameraBodyFixedM : vec4f,
  // The normals atlas: tiles across a layer, tiles a layer, a tile's stored texels, samples a side.
  atlas : vec4f,
}

@group(1) @binding(0) var<uniform> draw : Draw;

// Each slot's class layers, CLASS_LAYER_WORDS words (`slotLayout.ts`' `class-layers` field): first
// each vertex's eight class-weight bytes over its patch's palette, two words a vertex (the bake's
// `weights`, vertex (x, y) at 65 y + x; 0 where the vertex has no weights); then, from word
// CLASS_LAW_WORD, the palette as laws (`terrainMaterial.ts`' `packClassLaws`), TERRAIN_CLASSES of
// two `vec4f` as `f32` words: A = c A_N per display channel (r, g, b) and its L(0); its row of
// `phase_factor_table`, −1 for a slot with no law (empty, or of an unnamed substance), whose
// weight goes to law 0. One buffer for both. Read by the fragment stage alone, as the lights and
// the horizon map are, the vertex stage reading R05's five (decision-r10-t10b-r09-t2e §5).
@group(2) @binding(6) var<storage, read> class_layers : array<u32>;

// The frame's phase table (`litBody.wgsl`): row 0 the body's uniform law, `elsewhere`, then one row
// per class law (template, s and L(0)).
@group(2) @binding(7) var phase_factor_table : texture_2d<f32>;

// The lights, TERRAIN_LIGHT_ROWS (`terrainMaterial.ts`' `packTerrainLighting`), body-fixed:
//   0  the stars, the occluders, the planetshine sources, the annuli in use;
//   1  law 0's A per channel (r, g, b), scaled for the figure; w, its L(0);
//   2  x law 0's row; y the exposure scale ÷ π;
//   3 + 8j, star j: its unit direction from the body's centre, w its distance ÷ a; its illuminance
//      face-on at the body per channel (lx), w its radius ÷ a; then its annuli (`DiscAnnuli`'s
//      outer edges and fluxes) for r, g and b, two rows each;
//   19 + k, occluder k: its centre from the body's centre ÷ a; w, its radius ÷ a;
//   21 + 2j, source j: the neighbour's unit direction from the body's centre, w its distance ÷ a;
//      its illuminance face-on at the body's centre per channel (lx), w its radius ÷ a.
@group(2) @binding(8) var<storage, read> terrain_lights : array<vec4f>;

// The body's atmosphere, which `surfaceLighting.wgsl` reads (R08.T9.b), each by textureLoad;
// one-texel tables of zeros for a body without one, which read as no air.
@group(2) @binding(9) var atmosphere_body : texture_2d<f32>;
@group(2) @binding(10) var atmosphere_transmittance : texture_2d_array<f32>;
@group(2) @binding(11) var atmosphere_absorber_columns : texture_2d_array<f32>;
@group(2) @binding(12) var atmosphere_absorber_pressure_columns : texture_2d_array<f32>;
@group(2) @binding(13) var atmosphere_last_absorber_pressure_column : texture_2d_array<f32>;
@group(2) @binding(14) var atmosphere_absorber_curves : texture_2d<f32>;
@group(2) @binding(15) var atmosphere_irradiance : texture_2d_array<f32>;

// Each slot's survey mask, SURVEY_MASK_WORDS words (`slotLayout.ts`' `survey-mask` field): vertex
// (x, y)'s byte at 65 y + x, four to a word from its low byte, 255 surveyed and 0 not, then at byte
// SURVEY_CELL_CODE_BYTE log₂ of the quads a coarse cell of the survey spans along the patch's side,
// 1 to 6, or 0 where a cell spans one quad or less (`gpu/resources.ts`' `packSurveyMask`). Read by
// both stages: the vertex stage places the fins on the cells' edges by the code, the fragment stage
// discards by the mask (decision-r10-t10b-r09-t2e §5).
@group(2) @binding(17) var<storage, read> survey_mask : array<u32>;

// A palette's slots (`hyperion_surface`'s `MAX_CLASSES`), and the laws a texel shades with: law 0
// and one per slot.
const TERRAIN_CLASSES : u32 = 8u;
const TERRAIN_LAWS : u32 = 9u;
// A slot's class layers in words: 65 × 65 × 2 of weights, then 8 × 2 × 4 of laws.
const CLASS_LAW_WORD : u32 = 8450u;
const CLASS_LAYER_WORDS : u32 = 8514u;
const FIRST_TERRAIN_LIGHT_ROW : u32 = 3u;
const TERRAIN_LIGHT_ROWS_EACH : u32 = 8u;
const FIRST_TERRAIN_OCCLUDER_ROW : u32 = 19u;
const FIRST_TERRAIN_SECONDARY_ROW : u32 = 21u;
const TERRAIN_SECONDARY_ROWS_EACH : u32 = 2u;
const MAX_TERRAIN_LIGHTS : u32 = 2u;
const MAX_TERRAIN_OCCLUDERS : u32 = 2u;
const MAX_TERRAIN_SECONDARIES : u32 = 2u;
// A slot's survey mask in words, 4,228 B, and the byte of its cell code (`slotLayout.ts`).
const SURVEY_MASK_WORDS : u32 = 1057u;
const SURVEY_CELL_CODE_BYTE : u32 = 4225u;
// The lit mesh's vertex codes (`gpu/resources.ts`' `LIT_VERTEX_CODE`), each triangle's first
// vertex's its part: the grid, a skirt's foot strip, a skirt's head strip, a column's fin, a row's.
const LIT_GRID : u32 = 0u;
const LIT_SKIRT_FOOT : u32 = 1u;
const LIT_SKIRT_HEAD : u32 = 2u;
const LIT_COLUMN_FIN : u32 = 4u;
const LIT_ROW_FIN : u32 = 5u;

// `METER_CLASS.unlitBody` and `other` (`view/post/meter.ts`).
const METER_UNLIT_BODY : f32 = 3.0;
const METER_OTHER : f32 = 1.0;

// A texel is lit directly where a star's horizon and eclipse terms exceed this share of the face-on
// irradiance (`bodyDisc.wgsl`'s `LIT_IRRADIANCE`).
const TERRAIN_LIT_IRRADIANCE : f32 = 1e-5;

// The body-fixed pole.
const BODY_POLE : vec3f = vec3f(0.0, 0.0, 1.0);

// R11's hooks (plan R10, Provides; R11's Design notes 10, 11 and 14), each with no effect here.
// `p` is the texel's place from the body's centre ÷ a and `sun` the unit direction to the star,
// both body-fixed.

// R11's decoration (R11.T2.b): detail below the band limit, zero-mean about the class albedo, as a
// factor per channel on the texel's reflectance and its shading normal; the texel's own here.
struct TerrainDecoration {
  albedo : vec3f,
  normal : vec3f,
}

fn terrain_decoration(p : vec3f, normal : vec3f) -> TerrainDecoration {
  return TerrainDecoration(vec3f(1.0), normal);
}

// The direct sun's lit fraction beyond the horizon map's, which enters each star's horizon term
// (R10.T8.b): T9's cascades, into which R11's cloud shadows multiply; 1 until T9 lands.
fn terrain_shadow_factor(p : vec3f) -> f32 {
  return 1.0;
}

// The ring's shadow on the direct sun per channel (R11's `ring_shadow_on_body`); 1 here.
fn terrain_ring_shadow(p : vec3f, sun : vec3f) -> vec3f {
  return vec3f(1.0);
}

// A factor on the ground's diffuse sky irradiance, into which R11 puts a cloud deck's diffuse
// transmittance; 1 here.
fn terrain_sky_factor(p : vec3f) -> vec3f {
  return vec3f(1.0);
}

fn terrain_row(i : u32) -> vec4f {
  return terrain_lights[i];
}

// This file's angles go through `preciseTrig.wgsl`'s `preciseAtan2`, never WGSL's `acos`, `asin` or
// `atan2` (R08's Design note 18, which this plan's Consumes adopts). The angle in [0, π] between
// unit `a` and unit `b`, from their cross and dot products, which keep small angles.
fn angle_between(a : vec3f, b : vec3f) -> f32 {
  return preciseAtan2(length(cross(a, b)), dot(a, b));
}

// The angular radius of a sphere of radius `radius` at `distance` from its centre, in one unit of
// length: asin(radius ÷ distance), π ÷ 2 from inside it.
fn angular_radius(radius : f32, distance : f32) -> f32 {
  return preciseAtan2(radius, sqrt(max(distance * distance - radius * radius, 0.0)));
}

struct LitVertexOut {
  @builtin(position) clip : vec4f,
  // The vertex's place in its patch, (x, y) ÷ 64.
  @location(0) patchUv : vec2f,
  @location(1) @interpolate(flat) slot : u32,
  // The vertex less the camera, body-fixed, metres.
  @location(2) fromCameraBf : vec3f,
  // The datum's normal ν, body-fixed (renormalised in the fragment).
  @location(3) datumNormal : vec3f,
  // The geodetic height above the datum, metres.
  @location(4) heightM : f32,
  // The vertex's cell coordinates (s, t) on its patch's face, for the star's grid azimuth.
  @location(5) horizonSt : vec2f,
  // The vertex's morph factor, which blends the horizon to the parent's map as it mixes the heights.
  @location(6) morph : f32,
  @location(7) @interpolate(flat) horizonFace : u32,
  // The parent's slot (`HORIZON_NO_PARENT` for none) and the patch's quadrant in it.
  @location(8) @interpolate(flat) parent : vec2u,
  // The triangle's part of the lit mesh, its first vertex's code (LIT_GRID to LIT_ROW_FIN).
  @location(9) @interpolate(flat) kind : u32,
}

// The mask byte at `index` of `slot`'s survey mask.
fn survey_byte(slot : u32, index : u32) -> u32 {
  let word = survey_mask[slot * SURVEY_MASK_WORDS + index / 4u];
  return (word >> (8u * (index % 4u))) & 0xffu;
}

// Vertex (x, y)'s survey mask, 1 surveyed and 0 not.
fn survey_vertex(slot : u32, x : u32, y : u32) -> f32 {
  return f32(survey_byte(slot, y * VERTICES_PER_SIDE + x)) / 255.0;
}

// log₂ of the quads a coarse cell spans along `slot`'s patch's side, 1 to 6, or 0.
fn survey_cell_code(slot : u32) -> u32 {
  return min(survey_byte(slot, SURVEY_CELL_CODE_BYTE), 6u);
}

// Whether the point `f` of `slot`'s patch, in quads from its corner, lies on surveyed ground
// (`gpu/resources.ts`' `surveyedAt`): the coarse cell holding it, read at that cell's centre vertex,
// which lies on none of its edges; where a cell spans one quad or less, the mask interpolated
// between the quad's four vertices, at a half or more (Design note 12).
fn survey_surveyed(slot : u32, f : vec2f) -> bool {
  let code = survey_cell_code(slot);
  let g = max(f, vec2f(0.0));
  if (code == 0u) {
    let base = min(vec2u(floor(g)), vec2u(63u));
    let t = clamp(g - vec2f(base), vec2f(0.0), vec2f(1.0));
    let m00 = survey_vertex(slot, base.x, base.y);
    let m10 = survey_vertex(slot, base.x + 1u, base.y);
    let m01 = survey_vertex(slot, base.x, base.y + 1u);
    let m11 = survey_vertex(slot, base.x + 1u, base.y + 1u);
    return mix(mix(m00, m10, t.x), mix(m01, m11, t.x), t.y) >= 0.5;
  }
  let quads = 1u << code;
  let cells = max(64u / quads, 1u);
  let cell = min(vec2u(floor(g / f32(quads))), vec2u(cells - 1u));
  let centre = min(cell * quads + vec2u(quads / 2u), vec2u(63u));
  return survey_vertex(slot, centre.x, centre.y) >= 0.5;
}

// Whether column or row `c` of `slot`'s patch lies on an edge of its survey's coarse cells inside
// the patch, where its fin may join the terrain to the reference surface: a multiple of a cell's
// quads, where a cell is narrower than the patch and two quads or more.
fn survey_cell_line(slot : u32, c : u32) -> bool {
  let code = survey_cell_code(slot);
  if (code == 0u || code >= 6u) {
    return false;
  }
  return c % (1u << code) == 0u;
}

// Whether the fragment `in` is drawn at the survey's edge: the grid's on surveyed ground; a skirt's
// foot strip, hanging from the terrain's edge to the datum and below it, by the surveyed side; its
// head strip, rising from the terrain's edge to the datum, by the unsurveyed side; a fin where the
// cells either side of its line differ. So the side whose surface stands higher draws each skirt,
// and a skirt between two cells of one state lies beneath their surfaces. A skirt's state is the
// cell inside the patch at its place along the edge, read half a quad in.
fn survey_kept(in : LitVertexOut) -> bool {
  let f = in.patchUv * 64.0;
  switch in.kind {
    case LIT_SKIRT_FOOT: {
      return survey_surveyed(in.slot, clamp(f, vec2f(0.5), vec2f(63.5)));
    }
    case LIT_SKIRT_HEAD: {
      return !survey_surveyed(in.slot, clamp(f, vec2f(0.5), vec2f(63.5)));
    }
    case LIT_COLUMN_FIN: {
      let c = round(f.x);
      return survey_surveyed(in.slot, vec2f(c - 0.5, f.y)) != survey_surveyed(in.slot, vec2f(c + 0.5, f.y));
    }
    case LIT_ROW_FIN: {
      let c = round(f.y);
      return survey_surveyed(in.slot, vec2f(f.x, c - 0.5)) != survey_surveyed(in.slot, vec2f(f.x, c + 0.5));
    }
    default: {
      return survey_surveyed(in.slot, f);
    }
  }
}

@vertex
fn vertexMain(@location(0) grid : vec3f, @builtin(instance_index) instance : u32) -> LitVertexOut {
  // Every vertex is its grid vertex's, moved along the datum's normal to the height its part
  // stands at: a skirt's foot at min(h − skirt depth, 0), its head at max(h, 0), a fin's datum end
  // at 0 on a cell's edge and at h, collapsed, elsewhere.
  let code = u32(grid.z + 0.5);
  let v = patchVertexOf(vec3f(grid.xy, 0.0), instance);
  let inst = instances[instance];
  let rec = slots[inst.slot];
  let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
  var height = v.heightM;
  switch code {
    case LIT_SKIRT_FOOT: {
      height = min(v.heightM - rec.skirtDepthM, 0.0);
    }
    case LIT_SKIRT_HEAD: {
      height = max(v.heightM, 0.0);
    }
    case LIT_COLUMN_FIN: {
      if (survey_cell_line(v.slot, u32(grid.x))) {
        height = 0.0;
      }
    }
    case LIT_ROW_FIN: {
      if (survey_cell_line(v.slot, u32(grid.y))) {
        height = 0.0;
      }
    }
    default: {}
  }
  let fromCameraM = v.fromCameraM + rotation * ((height - v.heightM) * v.datumNormal);
  var out : LitVertexOut;
  out.clip = frame.clipProjection * frame.viewRotation * vec4f(fromCameraM, 1.0);
  out.patchUv = v.patchUv;
  out.slot = v.slot;
  out.fromCameraBf = transpose(rotation) * fromCameraM;
  out.datumNormal = v.datumNormal;
  out.heightM = height;
  // A skirt's or a fin's vertex takes its grid vertex's (x, y), so its shadows are the grid's.
  out.horizonSt = horizon_face_st(rec, u32(grid.x), u32(grid.y));
  out.morph = v.morph;
  out.horizonFace = horizon_face_of(rec);
  out.parent = vec2u(inst.parentSlot, inst.parentQuadrant);
  out.kind = code;
  return out;
}

// The two words of vertex (x, y) of `slot` as eight weights in [0, 1], byte ÷ 255.
struct VertexWeights {
  low : vec4f,
  high : vec4f,
}

fn vertex_weights(slot : u32, x : u32, y : u32) -> VertexWeights {
  let at = slot * CLASS_LAYER_WORDS + 2u * (y * VERTICES_PER_SIDE + x);
  return VertexWeights(unpack4x8unorm(class_layers[at]), unpack4x8unorm(class_layers[at + 1u]));
}

// The `vec4f` of four `f32` words from word `at` of the class layers.
fn class_layer_vec4(at : u32) -> vec4f {
  return bitcast<vec4f>(vec4u(class_layers[at], class_layers[at + 1u], class_layers[at + 2u], class_layers[at + 3u]));
}

// The class law of palette slot k of `slot`, and whether it has one.
struct ClassLaw {
  law : LunarLambert,
  named : bool,
}

fn class_law(slot : u32, k : u32) -> ClassLaw {
  let at = slot * CLASS_LAYER_WORDS + CLASS_LAW_WORD + 8u * k;
  let albedo = class_layer_vec4(at);
  let row = bitcast<f32>(class_layers[at + 4u]);
  var out : ClassLaw;
  out.law = LunarLambert(albedo.xyz, albedo.w, vec3f(1.0), u32(max(row, 0.0)));
  out.named = row >= 0.0;
  return out;
}

// Law m of a texel of `slot`: 0, the body's uniform law (`elsewhere`), from the lights' rows 1 and
// 2; m ≥ 1 palette slot m − 1's.
fn terrain_law(slot : u32, m : u32) -> LunarLambert {
  if (m == 0u) {
    let albedo = terrain_row(1u);
    return LunarLambert(albedo.xyz, albedo.w, vec3f(1.0), u32(terrain_row(2u).x));
  }
  return class_law(slot, m - 1u).law;
}

// The share of each of a texel's laws at `patch_uv` of `slot` (`terrainShares`): each slot's weight
// interpolated bilinearly between the quad's four vertices, the slots with a law kept, scaled down
// to sum to 1 where they sum to more, and law 0 (`elsewhere`) what they leave: the share of an
// unnamed substance's slot and of the vertices with no weights, outside the survey, as R07's disc
// gives `elsewhere` what its class map leaves (`surface_shares`).
fn terrain_shares(slot : u32, patch_uv : vec2f) -> array<f32, TERRAIN_LAWS> {
  let s = patch_uv * 64.0;
  let base = min(vec2u(floor(max(s, vec2f(0.0)))), vec2u(63u));
  let f = clamp(s - vec2f(base), vec2f(0.0), vec2f(1.0));
  let w00 = vertex_weights(slot, base.x, base.y);
  let w10 = vertex_weights(slot, base.x + 1u, base.y);
  let w01 = vertex_weights(slot, base.x, base.y + 1u);
  let w11 = vertex_weights(slot, base.x + 1u, base.y + 1u);
  let low = mix(mix(w00.low, w10.low, f.x), mix(w01.low, w11.low, f.x), f.y);
  let high = mix(mix(w00.high, w10.high, f.x), mix(w01.high, w11.high, f.x), f.y);
  var shares : array<f32, TERRAIN_LAWS>;
  var total = 0.0;
  for (var k = 0u; k < TERRAIN_CLASSES; k = k + 1u) {
    var w : f32;
    if (k < 4u) {
      w = low[k];
    } else {
      w = high[k - 4u];
    }
    if (class_law(slot, k).named) {
      shares[k + 1u] = w;
      total = total + w;
    }
  }
  if (total > 1.0) {
    for (var m = 1u; m < TERRAIN_LAWS; m = m + 1u) {
      shares[m] = shares[m] / total;
    }
  }
  shares[0] = select(1.0 - total, 0.0, total >= 1.0);
  return shares;
}

// The reflectance I/F of a texel's laws by their shares lit by one source of irradiance factor
// `h` (`litBody.wgsl`'s `lit_disc_term`), each law's f and L(α) from its own row, a zero share
// skipped (`bodyDisc.wgsl`'s `surface_reflectance`).
fn terrain_reflectance(
  shares : array<f32, TERRAIN_LAWS>,
  slot : u32,
  h : f32,
  mu0 : f32,
  mu : f32,
  alpha : f32,
  source_radius : f32,
) -> vec3f {
  var reflectance = vec3f(0.0);
  for (var m = 0u; m < TERRAIN_LAWS; m = m + 1u) {
    let share = shares[m];
    if (!(share > 0.0)) {
      continue;
    }
    let law = terrain_law(slot, m);
    let row = phase_row(law.table_row, alpha);
    let disc_term = lit_disc_term(row.a, h, mu0, mu, source_radius);
    reflectance = reflectance + share * law.a * row.rgb * disc_term;
  }
  return reflectance;
}

// One annulus set of star j and channel c (0 r, 1 g, 2 b).
fn terrain_annuli(j : u32, c : u32, count : u32) -> DiscAnnuli {
  let first = FIRST_TERRAIN_LIGHT_ROW + j * TERRAIN_LIGHT_ROWS_EACH + 2u + 2u * c;
  var annuli : DiscAnnuli;
  annuli.outer = terrain_row(first);
  annuli.flux = terrain_row(first + 1u);
  annuli.count = count;
  return annuli;
}

// The light a texel reflects towards the camera, pre-exposed, and whether any star lights it.
struct TerrainShaded {
  radiance : vec3f,
  lit : bool,
}

// The radiance of the texel `in` with the terrain's normal `terrain_normal`, both body-fixed; with
// `reference`, a skirt's or a fin's in the reference surface's appearance (R10.T10.c): law 0 alone
// under `terrain_normal`, the datum's, seen from either side, with no decoration and no horizon map.
fn shade_terrain(in : LitVertexOut, terrain_normal : vec3f, reference : bool) -> TerrainShaded {
  var out : TerrainShaded;
  out.radiance = vec3f(0.0);
  out.lit = false;
  // The texel from the body's centre ÷ a.
  let q = (in.fromCameraBf + draw.cameraBodyFixedM.xyz) * draw.cameraBodyFixedM.w;
  var decoration = TerrainDecoration(vec3f(1.0), terrain_normal);
  if (!reference) {
    decoration = terrain_decoration(q, terrain_normal);
  }
  let normal = decoration.normal;
  let ray = normalize(in.fromCameraBf);
  let facing = dot(normal, -ray);
  let mu = select(facing, abs(facing), reference);
  if (mu <= 0.0) {
    return out;
  }
  let counts = terrain_row(0u);
  let misc = terrain_row(2u);
  let annulus_count = u32(counts.w);
  var shares : array<f32, TERRAIN_LAWS>;
  if (reference) {
    shares[0] = 1.0;
  } else {
    shares = terrain_shares(in.slot, in.patchUv);
  }
  // The texel's mean A, for R08's sky light on the Lambert share's flat term.
  var mean_a = vec3f(0.0);
  for (var m = 0u; m < TERRAIN_LAWS; m = m + 1u) {
    if (shares[m] > 0.0) {
      mean_a = mean_a + shares[m] * terrain_law(in.slot, m).a;
    }
  }
  mean_a = mean_a * decoration.albedo;
  // R08's lookups: the texel's geodetic latitude and north from the datum's normal, its height.
  let nu = normalize(in.datumNormal);
  let local = local_frame(nu, BODY_POLE);
  let height = in.heightM;
  // R08's sky irradiance is a horizontal surface's; a texel tilted by β from the datum sees
  // (1 + cos β) ÷ 2 of an isotropic sky (Bruneton 2017's `GetSunAndSkyIrradiance`, its
  // `(1 + dot(normal, point) ÷ r) ÷ 2`; R08's note on a tilted surface makes it the caller's),
  // and R11's clouds multiply into the hook.
  let sky_factor = terrain_sky_factor(q) * (0.5 * (1.0 + dot(normal, nu)));
  let lights = min(u32(counts.x), MAX_TERRAIN_LIGHTS);
  let occluders = min(u32(counts.y), MAX_TERRAIN_OCCLUDERS);
  for (var j = 0u; j < lights; j = j + 1u) {
    let place = terrain_row(FIRST_TERRAIN_LIGHT_ROW + j * TERRAIN_LIGHT_ROWS_EACH);
    let light = terrain_row(FIRST_TERRAIN_LIGHT_ROW + j * TERRAIN_LIGHT_ROWS_EACH + 1u);
    let to_star = place.xyz * place.w - q;
    let distance = length(to_star);
    let towards = to_star / distance;
    let mu0 = dot(normal, towards);
    let mu_datum = dot(nu, towards);
    // The sky's light at the star's cosine against the datum, before the horizon's `continue`, so
    // that a star just set still lights the twilight ground (`bodyDisc.wgsl`).
    let sky = atmosphere_sky_irradiance(height, mu_datum, local.latitude, j) * sky_factor;
    // The horizon term on the terrain's normal, above the local horizon of the patch's horizon map
    // towards the star (R10.T8.b).
    let phi = angle_between(normal, towards);
    var local_horizon = 0.0;
    if (!reference) {
      local_horizon = terrain_horizon(
        in.slot, in.parent, in.horizonFace, in.horizonSt, in.patchUv, in.morph, nu, towards, phi
      );
    }
    let horizon = sphere_irradiance(
      terrain_star_distance_radii(distance, light.w), phi, local_horizon
    );
    if (horizon <= 0.0 && !any(sky > vec3f(0.0))) {
      continue;
    }
    let star_radius = angular_radius(light.w, distance);
    var visible = vec3f(1.0);
    for (var k = 0u; k < occluders; k = k + 1u) {
      let occluder = terrain_row(FIRST_TERRAIN_OCCLUDER_ROW + k);
      let to_occluder = occluder.xyz - q;
      let occluder_distance = length(to_occluder);
      if (occluder_distance >= distance) {
        continue;
      }
      if (occluder_distance <= occluder.w) {
        visible = vec3f(0.0);
        break;
      }
      let occluder_radius = angular_radius(occluder.w, occluder_distance);
      let along = to_occluder / occluder_distance;
      let separation = angle_between(along, towards);
      for (var c = 0u; c < 3u; c = c + 1u) {
        let kept = eclipse_visible(
          star_radius,
          terrain_annuli(j, c, annulus_count),
          occluder_radius,
          separation,
        );
        visible[c] = visible[c] - (1.0 - kept);
      }
    }
    visible = max(visible, vec3f(0.0));
    // The sky light shares the texel's eclipse, as an umbra's air does; the terrain's and the
    // ring's shadows fall on the direct sun alone (Design note 11).
    out.radiance = out.radiance + light.xyz * misc.y * mean_a * sky * visible;
    if (horizon <= 0.0) {
      continue;
    }
    let alpha = angle_between(towards, -ray);
    let reflectance =
      terrain_reflectance(shares, in.slot, horizon, mu0, mu, alpha, star_radius) * decoration.albedo;
    let transmitted = atmosphere_sun_transmittance(
      height, mu_datum, local.latitude, local_azimuth(local, towards), j
    );
    let shadowed = terrain_shadow_factor(q) * terrain_ring_shadow(q, towards);
    out.radiance = out.radiance + light.xyz * misc.y * reflectance * visible * transmitted * shadowed;
    out.lit = out.lit || (horizon > TERRAIN_LIT_IRRADIANCE && any(visible > vec3f(TERRAIN_LIT_IRRADIANCE)));
  }
  // Planetshine (R07's Design note 7), as R07's disc takes it: each lit neighbour a uniform sphere,
  // its direct term through R08's three-node rule over its disc and its sky term at its own
  // cosine, both at light slot 0, never shadowed; it lights the night side, which stays unlit for
  // the meter.
  let secondaries = min(u32(counts.z), MAX_TERRAIN_SECONDARIES);
  for (var j = 0u; j < secondaries; j = j + 1u) {
    let place = terrain_row(FIRST_TERRAIN_SECONDARY_ROW + j * TERRAIN_SECONDARY_ROWS_EACH);
    let source = terrain_row(FIRST_TERRAIN_SECONDARY_ROW + j * TERRAIN_SECONDARY_ROWS_EACH + 1u);
    let to_source = place.xyz * place.w - q;
    let distance = length(to_source);
    let towards = to_source / distance;
    let mu0 = dot(normal, towards);
    let mu_datum = dot(nu, towards);
    let near = place.w / distance;
    let sky = atmosphere_sky_irradiance(height, mu_datum, local.latitude, 0u) * sky_factor;
    out.radiance = out.radiance + source.xyz * (near * near) * misc.y * mean_a * sky;
    let irradiance = planetshine_irradiance(to_source, source.w, place.w, normal, 0.0);
    if (irradiance <= 0.0) {
      continue;
    }
    let alpha = angle_between(towards, -ray);
    let source_radius = angular_radius(source.w, distance);
    let reflectance =
      terrain_reflectance(shares, in.slot, irradiance, mu0, mu, alpha, source_radius) * decoration.albedo;
    let transmitted = atmosphere_source_transmittance(
      height, mu_datum, local.latitude, local_azimuth(local, towards), source_radius, 0u
    );
    out.radiance = out.radiance + source.xyz * misc.y * reflectance * transmitted;
  }
  return out;
}

@fragment
fn fragmentMain(in : LitVertexOut) -> @location(0) vec4f {
  if (!survey_kept(in)) {
    discard;
  }
  // One call of the shading: a compiler that inlines every call (SwiftShader's) takes one copy.
  let reference = in.kind != LIT_GRID;
  let normal = select(patchNormalAt(in.slot, in.patchUv), normalize(in.datumNormal), reference);
  let shaded = shade_terrain(in, normal, reference);
  var meter = METER_OTHER;
  if (u32(terrain_row(0u).x) > 0u) {
    meter = select(METER_UNLIT_BODY, METER_LIT_BODY, shaded.lit);
  }
  return vec4f(min(shaded.radiance, vec3f(HALF_FLOAT_MAX)), meter);
}
