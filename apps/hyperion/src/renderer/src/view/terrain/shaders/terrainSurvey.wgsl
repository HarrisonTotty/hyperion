// The survey's edge on a body's terrain (plan R10, R10.T10.c, Design note 12; shared by the lit
// material and, from R10.T11, the wireframe's): where each vertex of the lit mesh stands and which
// fragments are kept. A library composed by concatenation after frame.wgsl, patchVertex.wgsl and
// terrainPatch.wgsl, before a material's own source (terrainLit.wgsl, terrainWire.wgsl), which
// declares the `Draw` uniform `draw` with `bodyRotation`.
//
// A fragment of the terrain outside the survey is discarded, so that what stands for the reference
// surface beneath shows there: R07's figure, drawn with the inverse of the GPU's coverage mask
// (`smoothMesh.wgsl`), in the photorealistic style, and the body's occluder sphere, discarded where
// the mask holds a cell surveyed (`occluderSphereSurveyed.wgsl`), in the wireframe. Each coarse cell
// is the terrain's or the reference surface's, never both. The state of the fragment's cell is the
// bake's survey mask at that cell's centre vertex (`survey_surveyed`; `gpu/resources.ts`'
// `surveyedAt`), since a vertex on a cell's edge holds either cell's; the mask's half between the
// quad's vertices stands in where a cell spans one quad or less. Where the two surfaces meet at
// different heights, the lit mesh's skirts and fins (`gpu/resources.ts`' `litPatchMeshData`) join
// them, each drawn by the side whose surface stands higher above it, in the reference surface's
// appearance.

// Each slot's survey mask, SURVEY_MASK_WORDS words (`slotLayout.ts`' `survey-mask` field): vertex
// (x, y)'s byte at 65 y + x, four to a word from its low byte, 255 surveyed and 0 not, then at byte
// SURVEY_CELL_CODE_BYTE log₂ of the quads a coarse cell of the survey spans along the patch's side,
// 1 to 6, or 0 where a cell spans one quad or less (`gpu/resources.ts`' `packSurveyMask`). Read by
// both stages: the vertex stage places the fins on the cells' edges by the code, the fragment stage
// discards by the mask (decision-r10-t10b-r09-t2e §5).
@group(2) @binding(17) var<storage, read> survey_mask : array<u32>;

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

// Whether a fragment of `slot`'s patch at `patch_uv` ((x, y) ÷ 64) in the lit mesh's part `kind`
// is drawn at the survey's edge: the grid's on surveyed ground; a skirt's foot strip, hanging from
// the terrain's edge to the datum and below it, by the surveyed side; its head strip, rising from
// the terrain's edge to the datum, by the unsurveyed side; a fin where the cells either side of its
// line differ. So the side whose surface stands higher draws each skirt, and a skirt between two
// cells of one state lies beneath their surfaces. A skirt's state is the cell inside the patch at
// its place along the edge, read half a quad in.
fn survey_kept_at(slot : u32, patch_uv : vec2f, kind : u32) -> bool {
  let f = patch_uv * 64.0;
  switch kind {
    case LIT_SKIRT_FOOT: {
      return survey_surveyed(slot, clamp(f, vec2f(0.5), vec2f(63.5)));
    }
    case LIT_SKIRT_HEAD: {
      return !survey_surveyed(slot, clamp(f, vec2f(0.5), vec2f(63.5)));
    }
    case LIT_COLUMN_FIN: {
      let c = round(f.x);
      return survey_surveyed(slot, vec2f(c - 0.5, f.y)) != survey_surveyed(slot, vec2f(c + 0.5, f.y));
    }
    case LIT_ROW_FIN: {
      let c = round(f.y);
      return survey_surveyed(slot, vec2f(f.x, c - 0.5)) != survey_surveyed(slot, vec2f(f.x, c + 0.5));
    }
    default: {
      return survey_surveyed(slot, f);
    }
  }
}

// Where a vertex of the lit mesh stands (R10.T10.c): its grid vertex's (`patchVertexOf`), moved
// along the datum's normal to the height its part stands at, a skirt's foot at
// min(h − skirt depth, 0), its head at max(h, 0), a fin's datum end at 0 on a cell's edge and at h,
// collapsed, elsewhere.
struct LitPlace {
  // The grid vertex's: its offset, height, datum normal, place, slot and morph.
  vertex : PatchVertex,
  // Its part of the lit mesh (LIT_GRID to LIT_ROW_FIN), its `position`'s z.
  kind : u32,
  // The height above the datum it stands at, metres.
  heightM : f32,
  // Where it stands, less the camera, in the frame the instance origins are in, metres.
  fromCameraM : vec3f,
}

fn lit_place(grid : vec3f, instance : u32) -> LitPlace {
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
  var out : LitPlace;
  out.vertex = v;
  out.kind = code;
  out.heightM = height;
  out.fromCameraM = v.fromCameraM + rotation * ((height - v.heightM) * v.datumNormal);
  return out;
}
