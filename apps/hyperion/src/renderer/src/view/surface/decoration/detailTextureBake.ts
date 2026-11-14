/**
 * The world's generated detail sets (plan R11, R11.T2.c; Design note 21): for each material a
 * body's field names, a form and its palette entry, one layer of slopes, albedo and hue deviations
 * that `detailTextures.wgsl` samples, made by `detailTextureBake.wgsl`'s kernels from the form's
 * family, the entry's values and the body's conditions, keyed by the body's seed; none authored.
 *
 * @remarks
 * - **The mapping** ({@link detailParams}). The form picks the family (Design note 21's lean):
 *   graded grains for `Regolith`, `Soil`, `Sand` and `Dust`, at each form's clast size; fractured
 *   blocks for `Bedrock`; vesicles for a dark `VolcanicRock` and flow bands for a bright one;
 *   laminae for `Sediment` and `OrganicSediment`; crystal facets for `Snow`, `Frost` and `Glacier`;
 *   rafted plates for `SeaIce`; desiccation polygons for `Evaporite`; a cracked skin for `Melt`;
 *   none for `Seabed`, which the ocean draws. The entry's B, V and R albedos give the hue axis
 *   ({@link hueAxisOf}); on an airless body the maturity nodes' fresh and mature ends give it
 *   instead, R10's space-weathering law's own darkening and reddening, through which the entry's
 *   `FeO_T` and the ground's maturity act, never a TypeScript copy of the law. The body's pressure
 *   decides the grains' sorting and rounding: airless ground is gardened, poorly sorted and its
 *   grains a mix of fresh and mature, while under air the wind sorts and rounds them. Its surface
 *   age thins the clasts; its seed keys every hash with the material.
 * - **Generation** ({@link DetailSetBake}). A body's sets are one `texture_2d_array` of
 *   {@link DETAIL_LAYERS} layers of {@link DETAIL_TEXELS}² `rgba16float` texels and their
 *   {@link DETAIL_MIPS} levels, under the `decoration` memory category, made when the body's first
 *   patch at its finest level lands and freed with the body. One layer is generated a call of
 *   {@link DetailSetBake.step}, eleven dispatches, which the terrain session makes for one body a
 *   frame ({@link DETAIL_LAYERS_PER_FRAME}), and not while the view's frame-rate floor has stepped
 *   it down, so that an arrival's materials spread over frames. Until its layer is made a material
 *   draws undecorated (Design note 21). The kernels are the session's ({@link DetailKernelSet}),
 *   compiled when it is made, so that a hand-over compiles none; the low setting makes no kernel
 *   set and no bake, and so creates no generator pipeline.
 * - **The figures**: the feature sizes follow the forms' grains, clasts and plates as their sources
 *   state them where the pod reached one, and are leans otherwise ({@link DETAIL_FORMS}); the
 *   relief, the albedo spread and the shares are presentation leans for R12; the albedo span and
 *   the mechanics family, which the client does not hold, are each form's lean and the V albedo's.
 */

import type { SurfaceHeaderDto } from "@hyperion/protocol";

import { BUFFER_USAGE, TEXTURE_USAGE } from "../../engine/gpuFlags";
import type { KernelPair } from "../../engine/kernels";
import { EngineUnavailable } from "../../engine/resilientEngine";
import type { BufferHandle, ComputeHandle, RenderEngine, TextureHandle } from "../../engine/types";
import { finestLevel } from "../../terrain/cube";
import {
  PALETTE_NONE,
  PHOTOMETRY_STRIDE,
  type TerrainLayers,
} from "../../terrain/workers/messages";
import { DECORATION_WGSL } from "./decorationTwin";
import { decorationSeed, largestCellM, lowbias32 } from "./decorationLattice";
import detailTextureBakeWgsl from "./detailTextureBake.wgsl?raw";

/** A layer's side, texels. */
export const DETAIL_TEXELS = 256;

/** A layer's mip levels, 256² down to 1². */
export const DETAIL_MIPS = 9;

/** The layers a body's sets hold: its materials past these draw undecorated. */
export const DETAIL_LAYERS = 16;

/** The bytes of one layer with its mips, `rgba16float`: 8 B a texel, 256² (1 + ¼ + …). */
export const DETAIL_LAYER_BYTES = 8 * ((4 * DETAIL_TEXELS * DETAIL_TEXELS - 1) / 3);

/** The bytes of a body's sets: {@link DETAIL_LAYERS} layers, 11.2 MB. */
export const DETAIL_SET_BYTES = DETAIL_LAYERS * DETAIL_LAYER_BYTES;

/** The generator families, `detailTextureBake.wgsl`'s codes. */
export const DETAIL_FAMILY = {
  grains: 0,
  fractured: 1,
  vesicular: 2,
  flowBanded: 3,
  laminae: 4,
  crystalline: 5,
  plates: 6,
  polygons: 7,
  skin: 8,
} as const;

/** A family's name. */
export type DetailFamily = keyof typeof DETAIL_FAMILY;

/**
 * The surface pressure below which a body's ground takes no wind's sorting, Pa: the sim's
 * `AIRLESS_PRESSURE`, 100 Pa (`planetary/derive/atmosphere.rs`).
 */
export const AIRLESS_PRESSURE_PA = 100;

/**
 * The V albedo above which a `VolcanicRock` is felsic and flow-banded rather than mafic and
 * vesicular: a lean, since the client holds no mechanics family; basalts lie near 0.05–0.15 and
 * rhyolites above 0.25 (from memory, not re-checked), though a dark obsidian is felsic too.
 */
const FELSIC_ALBEDO_V = 0.25;

/** A form's generator, by its class code. */
interface DetailForm {
  readonly family: DetailFamily;
  /** The features' size, m: clasts, blocks, vesicles, crystals, plates or polygons. */
  readonly featureM: number;
  /** The relief's height, m, at the features' scale. */
  readonly reliefM: number;
  /** The relative albedo spread among its features. */
  readonly albedoSpread: number;
  /** The share of feature cells that hold one. */
  readonly featureShare: number;
}

/**
 * Each form's generator by its class code (`MaterialClass`, 0 to 14), `null` for `Seabed`. A
 * tile's texel is about 3 mm (a lattice cell of 0.5–1 m over 256 texels: 2.5–2.8 mm on the Moon,
 * Mars and Earth), so a feature below it is drawn at the texel ({@link detailParams}' clamp) and
 * finer grains are the speckle alone.
 *
 * @remarks
 * Checked 2026-11-13 (their abstracts or glossary entries, through the pod's reach):
 * - `Regolith`: lunar soil's mean grain 51 µm (Basu, Wentworth and McKay 2001, Meteorit. Planet.
 *   Sci. 36, 177; Apollo 11's 10084 under 1 mm), sub-texel; the clasts drawn, 2 cm at 8 % of the
 *   cells, are coarse fragments, a lean, the share over 1 cm not reached (Carrier, Olhoeft and
 *   Mendell 1991, Lunar Sourcebook ch. 9).
 * - `Sand`: Earth's dune sands of mean 250 µm and Mars's of about 500 µm (Edgett and Christensen
 *   1991, JGR 96, 22765), sub-texel; the features drawn are granules of 2–4 mm (Sharp 1963, J.
 *   Geol. 71, 617, its granule ripples), 3 mm at 5 % of the cells.
 * - `SeaIce`: pancake ice's plates are 0.03–3 m across (the NSIDC cryosphere glossary, after WMO
 *   No. 259, not reached), drawn at 0.3 m.
 *
 * The rest are leans from memory, not re-checked (no source reached from the pod): joint spacing
 * of decimetres in exposed rock (Priest and Hudson 1976, Int. J. Rock Mech. 13, 135); a soil's
 * gravel of 2–64 mm (Wentworth 1922, J. Geol. 30, 377); laminae under 1 cm and beds above
 * (Campbell 1967, Sedimentology 8, 7), drawn as bands of 3–5 cm; vesicles of millimetres to
 * centimetres; glacier ice's crystals of centimetres; snow's grains of 0.1–2 mm, sub-texel, drawn
 * as the texel's facets; hoar frost's crystals of millimetres; playa polygons of decimetres; a
 * lava's cooling skin cracked at decimetres; dust's clods of centimetres.
 */
export const DETAIL_FORMS: ReadonlyArray<DetailForm | null> = [
  { family: "fractured", featureM: 0.25, reliefM: 0.02, albedoSpread: 0.12, featureShare: 1 }, // Bedrock
  { family: "grains", featureM: 0.02, reliefM: 0.006, albedoSpread: 0.15, featureShare: 0.08 }, // Regolith
  { family: "grains", featureM: 0.03, reliefM: 0.008, albedoSpread: 0.12, featureShare: 0.3 }, // Soil
  { family: "grains", featureM: 0.003, reliefM: 0.001, albedoSpread: 0.08, featureShare: 0.05 }, // Sand
  { family: "laminae", featureM: 0.05, reliefM: 0.004, albedoSpread: 0.1, featureShare: 1 }, // Sediment
  { family: "vesicular", featureM: 0.015, reliefM: 0.006, albedoSpread: 0.1, featureShare: 0.3 }, // VolcanicRock
  { family: "skin", featureM: 0.25, reliefM: 0.015, albedoSpread: 0.1, featureShare: 1 }, // Melt
  { family: "crystalline", featureM: 0.06, reliefM: 0.004, albedoSpread: 0.05, featureShare: 1 }, // Glacier
  { family: "crystalline", featureM: 0.004, reliefM: 0.001, albedoSpread: 0.05, featureShare: 1 }, // Snow
  { family: "plates", featureM: 0.3, reliefM: 0.02, albedoSpread: 0.08, featureShare: 1 }, // SeaIce
  { family: "crystalline", featureM: 0.004, reliefM: 0.001, albedoSpread: 0.06, featureShare: 1 }, // Frost
  { family: "laminae", featureM: 0.03, reliefM: 0.003, albedoSpread: 0.1, featureShare: 1 }, // OrganicSediment
  null, // Seabed
  { family: "polygons", featureM: 0.3, reliefM: 0.015, albedoSpread: 0.1, featureShare: 1 }, // Evaporite
  { family: "grains", featureM: 0.02, reliefM: 0.003, albedoSpread: 0.06, featureShare: 0.15 }, // Dust
];

/** One material a body's field names, as its patches' bakes give it. */
export interface DetailMaterial {
  /** Its form, `MaterialClass`'s code. */
  readonly classCode: number;
  /** Its palette entry. */
  readonly entry: number;
  /** Its normal albedo in B, V and R under the field's albedo scale (`TerrainLayers.photometry`). */
  readonly albedoBvr: readonly [number, number, number];
  /**
   * Its albedo freshest and most mature, B, V and R, the first and last of its maturity nodes
   * (`TerrainMaturity.nodes`), or `null` where the bake has none.
   */
  readonly weathering: {
    readonly fresh: readonly [number, number, number];
    readonly mature: readonly [number, number, number];
  } | null;
}

/** What a body's conditions give its sets. */
export interface DetailBody {
  /** Its decoration seed (`decorationSeed` of its detail seed). */
  readonly seed: number;
  /** Its equatorial radius, m. */
  readonly radiusM: number;
  /** Its `decorationLevel`. */
  readonly level: number;
  /**
   * The level of its finest patches (`finestLevel` of its radius), from whose first bake its sets
   * are generated (R11.T2.c).
   */
  readonly startLevel: number;
  readonly surfacePressurePa: number;
  readonly surfaceAgeGyr: number;
  /** Its ground's background maturity, 0 to 1, or `null` where not modelled. */
  readonly maturity: number | null;
  /** Each palette entry's `FeO_T`, mass fraction, `null` where not modelled. */
  readonly feoByEntry: ReadonlyArray<number | null>;
}

/**
 * A body's {@link DetailBody} from its field header and detail seed.
 *
 * @param level - Its `decorationLevel`.
 */
export function detailBodyOf(
  header: SurfaceHeaderDto,
  detailSeed: string,
  level: number,
): DetailBody {
  return {
    seed: decorationSeed(detailSeed),
    radiusM: header.equatorial_radius_m,
    level,
    startLevel: finestLevel(header.equatorial_radius_m),
    surfacePressurePa: header.surface_pressure_pa,
    surfaceAgeGyr: header.surface_age_gyr,
    maturity: header.maturity ?? null,
    feoByEntry: header.palette.map((entry) => entry.feo_mass_fraction ?? null),
  };
}

/** A layer's generator, `detailTextureBake.wgsl`'s `DetailParams`. */
export interface DetailParams {
  readonly seed: number;
  readonly family: DetailFamily;
  /** A texel's width, m. */
  readonly texelM: number;
  /** The features' cells across the tile, a power of 2 from 2 to 256. */
  readonly cells: number;
  /** The speckle's cells across the tile, a power of 2. */
  readonly fineCells: number;
  readonly reliefM: number;
  readonly albedoSpread: number;
  readonly hueSpread: number;
  readonly featureShare: number;
  readonly weathered: number;
  readonly rounding: number;
  readonly sorting: number;
  /** The bands' whole turns across the tile along x and y. */
  readonly bands: readonly [number, number];
}

/** The power of 2 from 2 to `DETAIL_TEXELS` nearest `cells`, in its logarithm. */
function cellsOf(cells: number): number {
  const power = Math.round(Math.log2(Math.max(cells, 1)));
  return 2 ** Math.min(Math.log2(DETAIL_TEXELS), Math.max(1, power));
}

/**
 * The tile's side, m: one cell of the body's decoration lattice at its widest, so that the first
 * scale repeats every cell (`detailTextures.wgsl`'s period of 1) and the second every three.
 */
export function detailTileM(body: Pick<DetailBody, "radiusM" | "level">): number {
  return largestCellM(body.radiusM, body.level);
}

/**
 * A material's generator on a body (Design note 21; the module's remarks), or `null` for a form
 * that takes none (`Seabed`) or a code past `MaterialClass`'s.
 */
export function detailParams(material: DetailMaterial, body: DetailBody): DetailParams | null {
  const form = DETAIL_FORMS[material.classCode];
  if (form === undefined || form === null) {
    return null;
  }
  const airless = body.surfacePressurePa < AIRLESS_PRESSURE_PA;
  const albedoV = material.albedoBvr[1];
  const family: DetailFamily =
    form.family === "vesicular" && albedoV > FELSIC_ALBEDO_V ? "flowBanded" : form.family;
  const tileM = detailTileM(body);
  const texelM = tileM / DETAIL_TEXELS;
  const cells = cellsOf(tileM / form.featureM);
  // The same feature sizes on every body: the relief is the form's at its own size.
  const reliefM = form.reliefM * Math.min(1, tileM / cells / form.featureM);
  // Darker ground shows more contrast among its grains, relative: a lean, bounded.
  const albedoSpread =
    form.albedoSpread * Math.min(2, Math.max(0.5, 0.15 / Math.max(albedoV, 0.01)) ** 0.5);
  // A young surface keeps more of its clasts (the scars' and the weathering's density thin them).
  const youth = Math.min(1.5, Math.max(0.5, 1.5 - 0.25 * body.surfaceAgeGyr));
  const featureShare = Math.min(1, form.featureShare * (form.family === "grains" ? youth : 1));
  const feo = body.feoByEntry[material.entry] ?? null;
  const weathered = airless ? (body.maturity ?? 0) : 0;
  const hueSpread = airless ? weatheringContrast(material, feo) : 0.05;
  const k = lowbias32(
    (body.seed ^ lowbias32(((material.classCode << 8) | material.entry) >>> 0)) >>> 0,
  );
  return {
    seed: k,
    family,
    texelM,
    cells,
    fineCells: DETAIL_TEXELS,
    reliefM,
    albedoSpread,
    hueSpread,
    featureShare,
    weathered,
    rounding: airless ? 0.3 + 0.5 * weathered : 0.85,
    sorting: airless ? 0.2 : 0.8,
    bands: family === "laminae" ? [1, cells] : [cells / 4, Math.max(1, cells / 16)],
  };
}

/**
 * The relative contrast between an airless material's fresh and mature grains: the maturity
 * nodes' ends' difference over their mean in V; with none, `FeO_T` times 1.5, the darkening a
 * lunar mare soil's iron gives at full maturity, a lean.
 */
function weatheringContrast(material: DetailMaterial, feo: number | null): number {
  const w = material.weathering;
  if (w !== null) {
    const mean = (w.fresh[1] + w.mature[1]) / 2;
    return mean > 0 ? Math.abs(w.fresh[1] - w.mature[1]) / mean : 0;
  }
  return Math.min(1, 1.5 * (feo ?? 0));
}

/**
 * A layer's hue axis, the relative change of its B, V and R albedos per unit of its hue channel:
 * on an airless body with maturity nodes, from fresh to mature (darker and redder); otherwise the
 * entry's own colour, each band's albedo over V less 1, so that the variation deepens its hue.
 */
export function hueAxisOf(
  material: DetailMaterial,
  body: Pick<DetailBody, "surfacePressurePa">,
): readonly [number, number, number] {
  const w = material.weathering;
  if (body.surfacePressurePa < AIRLESS_PRESSURE_PA && w !== null) {
    const mean = (w.fresh[1] + w.mature[1]) / 2;
    if (mean > 0) {
      const axis = [0, 1, 2].map((c) => ((w.mature[c] ?? 0) - (w.fresh[c] ?? 0)) / mean);
      const length = Math.hypot(...axis);
      return length > 0
        ? [(axis[0] ?? 0) / length, (axis[1] ?? 0) / length, (axis[2] ?? 0) / length]
        : [0, 0, 0];
    }
  }
  const v = material.albedoBvr[1];
  if (!(v > 0)) {
    return [0, 0, 0];
  }
  return [material.albedoBvr[0] / v - 1, 0, material.albedoBvr[2] / v - 1];
}

/** `DetailParams`' words, as its uniform holds them, for layer `layer` and level `level`. */
export function packDetailParams(p: DetailParams, layer: number, level: number): ArrayBuffer {
  const buffer = new ArrayBuffer(64);
  const u32 = new Uint32Array(buffer);
  const f32 = new Float32Array(buffer);
  const i32 = new Int32Array(buffer);
  u32.set([p.seed >>> 0, DETAIL_FAMILY[p.family], layer, level]);
  f32[4] = p.texelM;
  u32.set([p.cells, p.fineCells], 5);
  f32.set(
    [p.reliefM, p.albedoSpread, p.hueSpread, p.featureShare, p.weathered, p.rounding, p.sorting],
    7,
  );
  i32.set([p.bands[0], p.bands[1]], 14);
  return buffer;
}

/**
 * The materials a patch's layers name: each slot with a class and a named entry, in slot order,
 * with its photometry and its maturity nodes' ends at the patch's level.
 */
export function detailMaterialsOf(layers: TerrainLayers): DetailMaterial[] {
  const materials: DetailMaterial[] = [];
  const nodes = layers.maturity?.nodes ?? null;
  const slots = layers.palette.length / 2;
  const k = nodes === null ? 0 : nodes.length / (slots * 2 * 3);
  for (let slot = 0; slot < slots; slot += 1) {
    const classCode = layers.palette[2 * slot] ?? PALETTE_NONE;
    const entry = layers.palette[2 * slot + 1] ?? PALETTE_NONE;
    const at = PHOTOMETRY_STRIDE * slot;
    const law = layers.photometry[at + 3] ?? 0;
    if (classCode === PALETTE_NONE || entry === PALETTE_NONE || !(law > 0)) {
      continue;
    }
    const albedoBvr = [
      layers.photometry[at] ?? 0,
      layers.photometry[at + 1] ?? 0,
      layers.photometry[at + 2] ?? 0,
    ] as const;
    let weathering: DetailMaterial["weathering"] = null;
    if (nodes !== null && k >= 2) {
      const first = 2 * k * 3 * slot;
      const last = first + 3 * (k - 1);
      weathering = {
        fresh: [nodes[first] ?? 0, nodes[first + 1] ?? 0, nodes[first + 2] ?? 0],
        mature: [nodes[last] ?? 0, nodes[last + 1] ?? 0, nodes[last + 2] ?? 0],
      };
    }
    materials.push({ classCode, entry, albedoBvr, weathering });
  }
  return materials;
}

/** The generator kernels' bindings, which each entry below declares. */
const PARAMS_BINDING = "@group(0) @binding(0) var<uniform> params : DetailParams;";

/** The generate entry: each texel's raw height, albedo and hue. */
const GENERATE_ENTRY = `
${PARAMS_BINDING}
@group(0) @binding(1) var<storage, read_write> raw : array<vec4f>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  if (id.x >= DETAIL_TEXELS || id.y >= DETAIL_TEXELS) {
    return;
  }
  let t = (vec2f(id.xy) + 0.5) / f32(DETAIL_TEXELS);
  raw[id.y * DETAIL_TEXELS + id.x] = detail_texel(params, t);
}
`;

/** The mean entry: one workgroup sums each column, then the columns in a fixed tree. */
const MEAN_ENTRY = `
@group(0) @binding(0) var<storage, read> raw : array<vec4f>;
@group(0) @binding(1) var<storage, read_write> means : array<vec4f, 1>;
var<workgroup> partial : array<vec4f, 256>;

@compute @workgroup_size(256)
fn main(@builtin(local_invocation_index) local : u32) {
  var sum = vec4f(0.0);
  for (var row = 0u; row < DETAIL_TEXELS; row = row + 1u) {
    sum = sum + raw[row * DETAIL_TEXELS + local];
  }
  partial[local] = sum;
  workgroupBarrier();
  for (var stride = 128u; stride > 0u; stride = stride / 2u) {
    if (local < stride) {
      partial[local] = partial[local] + partial[local + stride];
    }
    workgroupBarrier();
  }
  if (local == 0u) {
    means[0] = partial[0] / f32(DETAIL_TEXELS * DETAIL_TEXELS);
  }
}
`;

/** The level-0 entry: slopes from the height's central differences, albedo and hue less their means. */
const LEVEL_0_ENTRY = `
${PARAMS_BINDING}
@group(0) @binding(1) var<storage, read> raw : array<vec4f>;
@group(0) @binding(2) var<storage, read> means : array<vec4f, 1>;
@group(0) @binding(3) var<storage, read_write> chain : array<vec4f>;
@group(0) @binding(4) var detail_out : texture_storage_2d_array<rgba16float, write>;

fn raw_at(x : i32, y : i32) -> vec4f {
  let n = i32(DETAIL_TEXELS);
  return raw[u32(((y % n) + n) % n) * DETAIL_TEXELS + u32(((x % n) + n) % n)];
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  if (id.x >= DETAIL_TEXELS || id.y >= DETAIL_TEXELS) {
    return;
  }
  let x = i32(id.x);
  let y = i32(id.y);
  let here = raw_at(x, y);
  let mean = means[0];
  let slopes = vec2f(raw_at(x + 1, y).x - raw_at(x - 1, y).x, raw_at(x, y + 1).x - raw_at(x, y - 1).x)
    / (2.0 * params.texel_m);
  let texel = vec4f(slopes, here.y - mean.y, here.z - mean.z);
  chain[id.y * DETAIL_TEXELS + id.x] = texel;
  textureStore(detail_out, vec2i(id.xy), i32(params.layer), texel);
}
`;

/** The mip entry: each texel of level \`params.level\` the mean of its four at the level above. */
const MIP_ENTRY = `
${PARAMS_BINDING}
@group(0) @binding(1) var<storage, read_write> chain : array<vec4f>;
@group(0) @binding(2) var detail_out : texture_storage_2d_array<rgba16float, write>;

// Where a level's texels start in the chain: the levels above it, each a quarter of the last.
fn level_offset(level : u32) -> u32 {
  var offset = 0u;
  for (var j = 0u; j < level; j = j + 1u) {
    let side = DETAIL_TEXELS >> j;
    offset = offset + side * side;
  }
  return offset;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let side = DETAIL_TEXELS >> params.level;
  if (id.x >= side || id.y >= side) {
    return;
  }
  let above = 2u * side;
  let first = level_offset(params.level - 1u) + 2u * id.y * above + 2u * id.x;
  let texel =
    (chain[first] + chain[first + 1u] + chain[first + above] + chain[first + above + 1u]) * 0.25;
  chain[level_offset(params.level) + id.y * side + id.x] = texel;
  textureStore(detail_out, vec2i(id.xy), i32(params.layer), texel);
}
`;

/** A generator kernel of `entry`: the decoration library, the generator and the entry. */
function kernel(name: string, entry: string): KernelPair {
  return {
    name,
    reference: `${DECORATION_WGSL}\n${detailTextureBakeWgsl}\n${entry}`,
    subgroup: null,
    readback: "bit-exact",
  };
}

/** The generate kernel (`detailTextureBake.wgsl`'s `detail_texel` over a layer). */
export const DETAIL_GENERATE_KERNEL: KernelPair = kernel("R11 detail generate", GENERATE_ENTRY);

/** The mean kernel, one workgroup. */
export const DETAIL_MEAN_KERNEL: KernelPair = kernel("R11 detail mean", MEAN_ENTRY);

/** The level-0 kernel: the layer's slopes and zero-mean deviations. */
export const DETAIL_LEVEL_0_KERNEL: KernelPair = kernel("R11 detail level 0", LEVEL_0_ENTRY);

/** The mip kernel: one level's 2 × 2 means. */
export const DETAIL_MIP_KERNEL: KernelPair = kernel("R11 detail mip", MIP_ENTRY);

/** The four generator kernels, for the catalogue. */
export const DETAIL_KERNELS: ReadonlyArray<KernelPair> = [
  DETAIL_GENERATE_KERNEL,
  DETAIL_MEAN_KERNEL,
  DETAIL_LEVEL_0_KERNEL,
  DETAIL_MIP_KERNEL,
];

/** The texels of the mip chain's buffer: every level's, 256² (1 + ¼ + …) = 87,381. */
const CHAIN_TEXELS = (4 * DETAIL_TEXELS * DETAIL_TEXELS - 1) / 3;

/** The bytes of a bake's scratch: the raw layer, the means and the mip chain, `vec4f` each. */
export const DETAIL_SCRATCH_BYTES = (DETAIL_TEXELS * DETAIL_TEXELS + 1 + CHAIN_TEXELS) * 16;

/**
 * The most a body's sets may hold on the GPU, bytes: 16 MB, its array and its scratch together,
 * 13.6 MB as built (a lean, 2026-11-13; the brainstorm's memory table has no decoration row yet,
 * which R12 itemises under the `decoration` category).
 */
export const DETAIL_BODY_BYTES_MAX = 16_000_000;

/**
 * The name the generator's dispatches are timed under (`PassTimes`): R11's `decoration`, which
 * R12's `PASS_ROWS` maps to the budget's terrain row (R12's Consumes).
 */
export const DETAIL_PASS_LABEL = "decoration";

/**
 * The layers the session generates in one animation frame, of whichever body waits first: one, its
 * eleven dispatches, so that the generation's GPU time stays within a lean of 1 ms a frame (an
 * estimate, 2026-11-13: 256² texels of at most three 3 × 3 cellular searches each; the owner's
 * RTX 3080 and UHD 620 runs measure it, `PassTimes` under {@link DETAIL_PASS_LABEL}).
 */
export const DETAIL_LAYERS_PER_FRAME = 1;

/** One generated layer of a body's sets, for the terrain's draw (R11.T2.b). */
export interface DetailLayer {
  readonly classCode: number;
  readonly entry: number;
  /** Its layer in the sets' array. */
  readonly layer: number;
  /** The relative change of B, V and R per unit of its hue channel ({@link hueAxisOf}). */
  readonly hueAxis: readonly [number, number, number];
}

/** The four generator kernels, compiled. */
export interface DetailKernels {
  readonly generate: ComputeHandle;
  readonly mean: ComputeHandle;
  readonly level0: ComputeHandle;
  readonly mip: ComputeHandle;
}

/** Where a bake finds the generator's kernels: `null` until they are compiled, or where none can be. */
export interface DetailKernelSource {
  readonly kernels: DetailKernels | null;
}

/** What the kernel set asks of the engine. */
export type DetailKernelEngine = Pick<
  RenderEngine,
  "createComputeAsync" | "releaseCompute" | "onRestored"
>;

/**
 * The generator's kernels, compiled once a terrain session on a setting that draws decoration
 * (Design note 21; R10.T14.d's rule that a hand-over compiles nothing), shared by every body's bake.
 *
 * @remarks
 * Made only where the setting's `decoration` is on ({@link DetailKernelSet.forSetting}), so that the
 * low setting creates no generator pipeline. At a device's restore the lost handles are dropped and
 * the four compiled again: a compile that settles after the restore is dropped too, its handles the
 * lost engine's, and one that settles after the set is let go is released at once. Owns its
 * handles and releases them at {@link DetailKernelSet.dispose}. Its compile never rejects: a
 * failure is reported and settles {@link DetailKernelSet.ready} `false`.
 */
export class DetailKernelSet implements DetailKernelSource {
  readonly #engine: DetailKernelEngine;
  readonly #offRestored: () => void;
  #kernels: DetailKernels | null = null;
  #epoch = 0;
  #disposed = false;
  #ready: Promise<boolean>;

  /** The set of `engine`'s kernels where `settings` draws decoration, else `null`. */
  static forSetting(
    settings: { readonly decoration: boolean },
    engine: DetailKernelEngine,
  ): DetailKernelSet | null {
    return settings.decoration ? new DetailKernelSet(engine) : null;
  }

  private constructor(engine: DetailKernelEngine) {
    this.#engine = engine;
    this.#ready = this.#compiled();
    this.#offRestored = engine.onRestored(() => {
      // The lost device's handles went with it: `ResilientEngine` drops their release.
      this.#kernels = null;
      this.#epoch += 1;
      this.#ready = this.#compiled();
    });
  }

  /** The kernels, or `null` until they are compiled, after a failed compile and once let go. */
  get kernels(): DetailKernels | null {
    return this.#kernels;
  }

  /** Settles `true` once the current device's kernels are compiled, `false` where they are not. */
  get ready(): Promise<boolean> {
    return this.#ready;
  }

  /** {@link DetailKernelSet}'s `#compile`, its failure reported: it never rejects. */
  #compiled(): Promise<boolean> {
    return this.#compile().catch((error: unknown) => {
      console.error("the detail sets' kernels could not be compiled:", error);
      return false;
    });
  }

  async #compile(): Promise<boolean> {
    const epoch = this.#epoch;
    const engine = this.#engine;
    // Each ask in an async function, so that one that throws at once is a rejection like the rest.
    const settled = await Promise.allSettled(
      DETAIL_KERNELS.map(async (pair) => engine.createComputeAsync(pair)),
    );
    const handles = settled.flatMap((each) => (each.status === "fulfilled" ? [each.value] : []));
    const [generate, mean, level0, mip] = handles;
    if (
      this.#disposed ||
      epoch !== this.#epoch ||
      generate === undefined ||
      mean === undefined ||
      level0 === undefined ||
      mip === undefined
    ) {
      // A compile of the lost device's, settled after the restore, holds the lost engine's
      // handles, which `ResilientEngine` drops at the loss: none is released.
      if (epoch === this.#epoch) {
        for (const handle of handles) {
          engine.releaseCompute(handle);
        }
      }
      const failure = settled.find((each) => each.status === "rejected");
      if (failure !== undefined && !this.#disposed && epoch === this.#epoch) {
        const reason: unknown = failure.reason;
        if (!(reason instanceof EngineUnavailable)) {
          console.error("the detail sets' kernels could not compile:", reason);
        }
      }
      return false;
    }
    this.#kernels = { generate, mean, level0, mip };
    return true;
  }

  /** Releases the kernels and stops following restores; idempotent. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#offRestored();
    const kernels = this.#kernels;
    this.#kernels = null;
    if (kernels !== null) {
      for (const handle of [kernels.generate, kernels.mean, kernels.level0, kernels.mip]) {
        this.#engine.releaseCompute(handle);
      }
    }
  }
}

/** What a lit terrain pass makes its body's sets from: the session's kernels and the body's conditions. */
export interface DetailSetSource {
  readonly kernels: DetailKernelSource;
  readonly body: DetailBody;
}

/** What a bake asks of the engine. */
export type DetailEngine = Pick<
  RenderEngine,
  "createBuffer" | "releaseBuffer" | "createTexture" | "releaseTexture" | "dispatch"
>;

/** A material waiting for its layer. */
interface Waiting {
  readonly material: DetailMaterial;
  readonly params: DetailParams;
  readonly layer: number;
}

/** The GPU objects a bake makes at its first layer. */
interface DetailObjects {
  readonly texture: TextureHandle;
  readonly raw: BufferHandle;
  readonly means: BufferHandle;
  readonly chain: BufferHandle;
}

/** A side's workgroups of 8 × 8. */
function groupsOf(side: number): readonly [number, number, number] {
  return [Math.ceil(side / 8), Math.ceil(side / 8), 1];
}

/**
 * A body's detail sets (the module's remarks): its materials as its patches name them, each
 * generated into its own layer one at a time by the session's {@link DetailKernelSet}, and the
 * array they lie in.
 *
 * @remarks
 * Made by a lit terrain pass whose session holds a kernel set, so that none is made on the low
 * setting. Owns its texture and its scratch buffers, released by {@link DetailSetBake.dispose}, the
 * body's going; the kernels are the set's.
 */
export class DetailSetBake {
  readonly #engine: DetailEngine;
  readonly #kernels: DetailKernelSource;
  readonly #body: DetailBody;
  readonly #name: string;
  readonly #known = new Map<string, number>();
  readonly #waiting: Waiting[] = [];
  readonly #ready: DetailLayer[] = [];
  #objects: DetailObjects | null = null;
  /** Whether a patch at the body's finest level has landed, from which every patch is noted. */
  #started = false;
  #disposed = false;

  constructor(engine: DetailEngine, kernels: DetailKernelSource, body: DetailBody, name: string) {
    this.#engine = engine;
    this.#kernels = kernels;
    this.#body = body;
    this.#name = name;
  }

  /** The body's conditions, as the sets take them. */
  get body(): DetailBody {
    return this.#body;
  }

  /**
   * Notes a patch at `level`'s materials: from the body's first patch at its finest level
   * ({@link DetailBody.startLevel}) on, each material not yet known waits for a layer, until the
   * layers are all taken.
   */
  note(layers: TerrainLayers, level: number): void {
    this.noteMaterials(detailMaterialsOf(layers), level);
  }

  /** {@link DetailSetBake.note} of the materials themselves, as a patch at `level` names them. */
  noteMaterials(materials: ReadonlyArray<DetailMaterial>, level: number): void {
    if (this.#disposed) {
      return;
    }
    if (!this.#started) {
      if (level < this.#body.startLevel) {
        return;
      }
      this.#started = true;
    }
    for (const material of materials) {
      const key = `${material.classCode}/${material.entry}`;
      if (this.#known.has(key) || this.#known.size >= DETAIL_LAYERS) {
        continue;
      }
      const params = detailParams(material, this.#body);
      if (params === null) {
        continue;
      }
      const layer = this.#known.size;
      this.#known.set(key, layer);
      this.#waiting.push({ material, params, layer });
    }
  }

  /**
   * Generates the next waiting material's layer, eleven dispatches under
   * {@link DETAIL_PASS_LABEL}, where the kernels are compiled; its caller calls it at most
   * {@link DETAIL_LAYERS_PER_FRAME} times a frame.
   *
   * @returns Whether a layer was generated.
   */
  step(): boolean {
    const kernels = this.#kernels.kernels;
    const next = this.#waiting[0];
    if (this.#disposed || kernels === null || next === undefined) {
      return false;
    }
    const objects = this.#objects ?? this.#make();
    this.#waiting.shift();
    const engine = this.#engine;
    const uniform = (level: number): Uint32Array =>
      new Uint32Array(packDetailParams(next.params, next.layer, level));
    engine.dispatch(
      kernels.generate,
      { uniforms: { params: uniform(0) }, buffers: { raw: objects.raw }, sampled: {}, storage: {} },
      groupsOf(DETAIL_TEXELS),
      DETAIL_PASS_LABEL,
    );
    engine.dispatch(
      kernels.mean,
      {
        uniforms: {},
        buffers: { raw: objects.raw, means: objects.means },
        sampled: {},
        storage: {},
      },
      [1, 1, 1],
      DETAIL_PASS_LABEL,
    );
    engine.dispatch(
      kernels.level0,
      {
        uniforms: { params: uniform(0) },
        buffers: { raw: objects.raw, means: objects.means, chain: objects.chain },
        sampled: {},
        storage: { detail_out: { texture: objects.texture, level: 0 } },
      },
      groupsOf(DETAIL_TEXELS),
      DETAIL_PASS_LABEL,
    );
    for (let level = 1; level < DETAIL_MIPS; level += 1) {
      engine.dispatch(
        kernels.mip,
        {
          uniforms: { params: uniform(level) },
          buffers: { chain: objects.chain },
          sampled: {},
          storage: { detail_out: { texture: objects.texture, level } },
        },
        groupsOf(DETAIL_TEXELS >> level),
        DETAIL_PASS_LABEL,
      );
    }
    this.#ready.push({
      classCode: next.material.classCode,
      entry: next.material.entry,
      layer: next.layer,
      hueAxis: hueAxisOf(next.material, this.#body),
    });
    return true;
  }

  /** The texture, its scratch buffers, made at the first layer. */
  #make(): DetailObjects {
    const engine = this.#engine;
    const storage = (name: string, bytes: number): BufferHandle =>
      engine.createBuffer({
        name: `${this.#name} ${name}`,
        bytes,
        usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
        category: "decoration",
      });
    const objects: DetailObjects = {
      texture: engine.createTexture({
        name: `${this.#name} detail sets`,
        size: { width: DETAIL_TEXELS, height: DETAIL_TEXELS, depthOrArrayLayers: DETAIL_LAYERS },
        dimension: "2d",
        format: "rgba16float",
        mips: DETAIL_MIPS,
        usage:
          TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.COPY_SRC,
        category: "decoration",
      }),
      raw: storage("detail raw", DETAIL_TEXELS * DETAIL_TEXELS * 16),
      means: storage("detail means", 16),
      chain: storage("detail chain", CHAIN_TEXELS * 16),
    };
    this.#objects = objects;
    return objects;
  }

  /** The layer generated for a material, or `null` while it waits or has none. */
  layerOf(classCode: number, entry: number): DetailLayer | null {
    return this.#ready.find((l) => l.classCode === classCode && l.entry === entry) ?? null;
  }

  /** Every layer generated, in the order they were. */
  get layers(): ReadonlyArray<DetailLayer> {
    return this.#ready;
  }

  /** How many materials wait for their layers. */
  get waiting(): number {
    return this.#waiting.length;
  }

  /** The sets' array, or `null` before the first layer. */
  get texture(): TextureHandle | null {
    return this.#objects?.texture ?? null;
  }

  /** The bytes the bake holds on the GPU: its array and scratch, 0 before the first layer. */
  get memoryBytes(): number {
    return this.#objects === null ? 0 : DETAIL_SET_BYTES + DETAIL_SCRATCH_BYTES;
  }

  /** Releases the texture and the scratch buffers; idempotent. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#waiting.length = 0;
    const objects = this.#objects;
    this.#objects = null;
    if (objects !== null) {
      this.#engine.releaseTexture(objects.texture);
      this.#engine.releaseBuffer(objects.raw);
      this.#engine.releaseBuffer(objects.means);
      this.#engine.releaseBuffer(objects.chain);
    }
  }
}
