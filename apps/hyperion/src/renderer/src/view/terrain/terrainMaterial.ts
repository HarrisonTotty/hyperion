/**
 * The lit style's terrain material (plan R10, R10.T10.b; Design notes 8, 9 and 11): a body's
 * patches shaded from their baked palette and class weights, each class by its own lunar-Lambert
 * law through R07's `body_brdf` and its own row of the phase table, summed by weight, lit as R07's
 * disc is lit, through R08's surface lighting. This module holds the material's spec, the class
 * laws and their rows, the packing of a frame's lights, and the CPU twin of the blend.
 *
 * @remarks
 * - **One law per class, summed by weight.** A texel reflects Σ wₖ rₖ, each rₖ its class's own
 *   `body_brdf` I/F, never the I/F of one law of weighted parameters, which differs from the sum by
 *   the covariance of A_N and L: 1.7× too bright at μ₀ = 0.5, μ = 0.1 for an even blend of regolith
 *   (A_N 0.12, L 1) and snow (A_N 0.9, L 0) (Design note 8; decision-r07-t8b). A zero weight adds
 *   nothing, and the largest weight's class is the one a readout names ({@link dominantSlot}).
 * - **A class's law** is its palette slot's photometry, as the worker resolves it from R10.T10.a's
 *   scaled albedo and R10.T1.a's law (`TerrainLayers.photometry`): A = c A_N per display channel
 *   (Johnson R, V and B as r, g and b, as R07's `modelledPhotometry` maps them), times the figure's
 *   factor R07's disc gives each of its laws, its class-map classes' included (`oblateAlbedoScale`
 *   of the class's L(0), 1 for a sphere; R10.T10.e), so that the disc and the terrain draw one
 *   law a class on an oblate body too; L(0) the law's at c A_N, and the law's template and
 *   exponent s ({@link CLASS_LAW_TEMPLATES}). Its row of R07's
 *   phase table holds its f against its L(α), L(0) times McEwen's profile for the particulate laws
 *   and 0 for Lambert's, in alpha ({@link TerrainPhaseRows}): one row per (law, L(0)), so the
 *   materials of L(0) = 1, every one up to A_N 0.60, share one row of their law, and a bright
 *   frost takes its own (decision-r10-limb). The task's "three rows" are read as three kinds.
 * - **Law 0** is the body's uniform law (R07's `elsewhere`, the photometry's `lawFor`), its A
 *   scaled for the figure as R07's disc scales it (`oblateAlbedoScale`), and takes what the
 *   weights leave: the share of a slot whose palette names no substance, and of the vertices
 *   outside the survey, which have no weights, so that the terrain shows there what the disc shows
 *   ({@link terrainShares}; T10.c then discards those texels over the reference surface).
 * - **Maturity** (R10.T1.c's client half; decision-r10-t1c-client-scope §1.4). Where a bake carries
 *   a maturity layer (`TerrainLayers.maturity`), each named slot's A is drawn from its albedo nodes
 *   (`maturityNodes.ts`): the bilinear mean over the texel's quad's four vertices of T_n at each
 *   vertex's maturity at the patch's band level n, mixed by the morph factor μ with the parent's
 *   term, the bilinear mean over the parent's quad, on the even vertices, of T_{n−1} at each
 *   vertex's maturity at n − 1, so that at μ = 1 the patch draws its parent's field exactly (the
 *   albedo interpolated, not the maturity: the halo ruling's finding 8). The nodes take the
 *   figure's factor as A does ({@link packClassNodes}); L(0), the row and law 0 stay the slot's and
 *   the body's. A flag in each slot's law ({@link packClassLaws}) marks a slot drawn from its
 *   nodes, so that without a layer every A is the law's, bit for bit. The twin is
 *   {@link texelClassAlbedo}, which {@link terrainTexelLaws} takes for a texel's place and morph.
 * - **The twin** ({@link terrainReflectance}) is R07.T4.c's: each law's I/F through `brdfFromTable`
 *   with its own row, as `body_brdf` reads it; the shader's `terrain_reflectance` replaces μ₀ by
 *   a source's irradiance factor and floors the Lommel–Seeliger term at the source's angular
 *   radius, as R07's disc does (`lit_disc_term`), which equals the twin wherever the whole source
 *   is up and μ₀ + μ exceeds that radius.
 * - **The shadow cascades** (R10.T9.b; Design note 11): the lit material receives them
 *   (`shadows/cascadeShadow.wgsl`, its twin `shadows/cascadeShadow.ts`), star 0's direct light
 *   taking min(1, c′ ÷ m) beside the horizon map; the caster ({@link terrainShadowMaterialSpec})
 *   draws the lit vertices into each cascade, depth alone.
 */
import { brdfFromTable } from "../appearance/brdf";
import {
  type PhaseFactorTable,
  phaseFactorTableOf,
  type PhaseTemplateId,
  type PhotometricLaw,
} from "../appearance/law";
import { withTemplateCurve } from "../appearance/phase";
import { SURFACE_LIGHTING_WGSL, surfaceLightingTextures } from "../atmosphere/surfaceLighting";
import type { DiscLight, DiscOccluder, DiscSecondary } from "../bodies/discShading";
import { oblateAlbedoScale } from "../bodies/oblate";
import type { StorageBufferSpec, UniformSpec, WgslMaterialSpec } from "../engine/types";
import type { Rgb } from "../photometry/toneCurve";
import type { TerrainVertexPath } from "../quality/qualitySetting";
import frameWgsl from "../shaders/frame.wgsl?raw";
import litBodyWgsl from "../shaders/litBody.wgsl?raw";
import maturityNodesWgsl from "../shaders/maturityNodes.wgsl?raw";
import { PATH_SOURCES, TERRAIN_BINDINGS, terrainVertexStorage } from "./gpu/material";
import {
  MATURITY_NODES,
  maturityCodeRatio,
  maturityNodeAlbedo,
  type MaturitySegment,
  maturitySegment,
} from "./maturityNodes";
import { CASCADE_UNIFORM_SPECS } from "./shadows/cascadeShadow";
import cascadeShadowWgsl from "./shadows/cascadeShadow.wgsl?raw";
import { CASCADE_DEPTH_BIAS } from "./shadows/cascades";
import horizonWgsl from "./shadows/horizon.wgsl?raw";
import patchVertexWgsl from "./shaders/patchVertex.wgsl?raw";
import terrainDepthWgsl from "./shaders/terrainDepth.wgsl?raw";
import terrainLitWgsl from "./shaders/terrainLit.wgsl?raw";
import terrainLitShadeWgsl from "./shaders/terrainLitShade.wgsl?raw";
import terrainPatchWgsl from "./shaders/terrainPatch.wgsl?raw";
import terrainShadowWgsl from "./shaders/terrainShadow.wgsl?raw";
import terrainSurveyWgsl from "./shaders/terrainSurvey.wgsl?raw";
import { CLASS_NODES_BYTES, PATCH_VERTICES_PER_SIDE } from "./slotLayout";
import { PHOTOMETRY_STRIDE, type TerrainMaturity } from "./workers/messages";

/** A palette's slots: `hyperion_surface`'s `MAX_CLASSES`. */
export const TERRAIN_CLASSES = 8;

/** The laws a texel shades with: law 0, `elsewhere`, and one per palette slot. */
export const TERRAIN_LAWS = TERRAIN_CLASSES + 1;

/** The `@group(2)` bindings of `terrainLit.wgsl`: R05's, then the lit material's. */
export const TERRAIN_LIT_BINDINGS = {
  ...TERRAIN_BINDINGS,
  /**
   * Each slot's class weights and laws, and its maturity codes and nodes (R10.T1.c), one storage
   * buffer (`slotLayout.ts`' `class-layers`).
   */
  classLayers: 6,
  phaseFactorTable: 7,
  terrainLights: 8,
  /** The first of the seven tables of R08's surface lighting. */
  atmosphere: 9,
  /** Each slot's horizon map (`slotLayout.ts`' `horizon-map`, R10.T8.b). */
  horizonMap: 16,
  /** Each slot's survey mask (`slotLayout.ts`' `survey-mask`, R10.T10.c). */
  surveyMask: 17,
  /** The shadow cascades' layers, a `texture_depth_2d_array` (`cascadeShadow.wgsl`, R10.T9.b). */
  shadowCascades: 19,
  /** The comparison sampler that reads them, a `sampler_comparison` (R10.T9.b). */
  shadowCompare: 20,
} as const;

/**
 * The `DrawItem.textures` name under which a lit terrain draw binds the shadow cascades' layers
 * (R10.T9.b): the engine's `DepthLayerTarget.depth`, or, where no cascades are drawn, any
 * `depth32float` array texture, which the receiver never samples at w = 0.
 */
export const SHADOW_CASCADES_TEXTURE = "shadowCascades";

/**
 * The lit material's comparison sampler (R10.T9.b): declared by the spec and made by the engine,
 * linear for the 2 × 2 PCF, clamped to the edge and `greater-equal`; a draw binds nothing for it.
 */
export const SHADOW_COMPARE_SAMPLER = "shadowCompare";

/**
 * The lit `Draw`'s fields after `offsetFromCameraM` that {@link LIT_TERRAIN_SOURCE} reads, which
 * the depth material and the caster mirror: the body's rotation, the camera's body-fixed place and
 * the normals atlas.
 */
const LIT_DRAW_UNIFORMS: ReadonlyArray<UniformSpec> = [
  { name: "bodyRotation", type: "mat4x4f" },
  { name: "cameraBodyFixedM", type: "vec4f" },
  { name: "atlas", type: "vec4f" },
];

/** The console names of the two paths' lit materials. */
const DISPLAY_NAMES: Readonly<Record<TerrainVertexPath, string>> = {
  "baked-offsets": "LIT TERRAIN OFFSETS",
  "face-differences": "LIT TERRAIN",
};

/**
 * What every lit terrain module composes before its `Draw` and fragment stage: `frame.wgsl`,
 * `patchVertex.wgsl`, R08's surface lighting, R07's `litBody.wgsl`, `terrainPatch.wgsl`, the
 * horizon map's lookup (`shadows/horizon.wgsl`, R10.T8.b), the survey's placement and discard
 * (`terrainSurvey.wgsl`, which the wireframe's terrain shares, R10.T11), the maturity nodes'
 * formula (`shaders/maturityNodes.wgsl`, R10.T1.c) and `terrainLit.wgsl`, its vertices and
 * shading.
 */
const LIT_TERRAIN_SOURCE =
  frameWgsl +
  patchVertexWgsl +
  SURFACE_LIGHTING_WGSL +
  litBodyWgsl +
  terrainPatchWgsl +
  horizonWgsl +
  terrainSurveyWgsl +
  maturityNodesWgsl +
  terrainLitWgsl;

/**
 * The lit terrain material of a vertex path: {@link LIT_TERRAIN_SOURCE}, its own `Draw` and
 * fragment stage (`terrainLitShade.wgsl`), the shadow cascades' receiver
 * (`shadows/cascadeShadow.wgsl`, R10.T9.b) and the path's source.
 *
 * @remarks
 * Its uniforms are the `Draw`'s own fields, then the cascades' ({@link CASCADE_UNIFORM_SPECS},
 * which `packCascadeUniforms` writes; zeros read as no cascades), and it binds the cascades' layers
 * as {@link SHADOW_CASCADES_TEXTURE} at binding 19, `depth` and `2d-array`, read through the
 * comparison sampler {@link SHADOW_COMPARE_SAMPLER} at binding 20. The receiver is composed here
 * alone, not into {@link LIT_TERRAIN_SOURCE}, which the depth material and the caster share: WGSL
 * type-checks every function, and their `Draw`s have none of the cascades' fields
 * (decision-r10-t9b-design D5(a)). Textures and samplers are visible to both stages, as the engine
 * lays out every material's.
 *
 * Each storage buffer is visible only to the stages that read it (decision-r10-t10b-r09-t2e §5):
 * R05's five, four on `face-differences`, to the vertex stage, the class layers, the lights and
 * the horizon map to the fragment stage, and the survey mask to both (R10.T10.c), the vertex stage
 * placing the survey edge's fins by its cell code and the fragment stage discarding by its mask:
 * six and four, five and four on `face-differences`, so that neither stage counts more than
 * WebGPU's default eight, which the engine never raises. It draws the lit mesh
 * (`gpu/resources.ts`' `litPatchMeshData`), whose skirts and fins join the terrain to the reference
 * surface at the survey's edge.
 */
export function terrainLitMaterialSpec(path: TerrainVertexPath): WgslMaterialSpec {
  const source = LIT_TERRAIN_SOURCE + terrainLitShadeWgsl + cascadeShadowWgsl + PATH_SOURCES[path];
  const b = TERRAIN_LIT_BINDINGS;
  const fragment = ["fragment"] as const;
  const storageBuffers: StorageBufferSpec[] = [
    ...terrainVertexStorage(path),
    { name: "classLayers", binding: b.classLayers, stages: fragment },
    { name: "terrainLights", binding: b.terrainLights, stages: fragment },
    { name: "horizonMap", binding: b.horizonMap, stages: fragment },
    { name: "surveyMask", binding: b.surveyMask, stages: ["vertex", "fragment"] },
  ];
  return {
    name: `terrain-lit-${path}`,
    displayName: DISPLAY_NAMES[path],
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [...LIT_DRAW_UNIFORMS, ...CASCADE_UNIFORM_SPECS],
    samplers: [
      {
        name: SHADOW_COMPARE_SAMPLER,
        filter: "linear",
        address: "clamp-to-edge",
        binding: b.shadowCompare,
        compare: "greater-equal",
      },
    ],
    textures: [
      { name: "normals", binding: b.normals, viewDimension: "2d-array" },
      { name: "phaseFactorTable", binding: b.phaseFactorTable, sampleType: "unfilterable-float" },
      ...surfaceLightingTextures(b.atmosphere),
      {
        name: SHADOW_CASCADES_TEXTURE,
        binding: b.shadowCascades,
        sampleType: "depth",
        viewDimension: "2d-array",
      },
    ],
    cullMode: "back",
    depthWrite: true,
    colourWrites: true,
    blend: "none",
    storageBuffers,
  };
}

/** The console names of the two paths' depth materials. */
const DEPTH_DISPLAY_NAMES: Readonly<Record<TerrainVertexPath, string>> = {
  "baked-offsets": "TERRAIN DEPTH OFFSETS",
  "face-differences": "TERRAIN DEPTH",
};

/**
 * The lit terrain of a vertex path as depth alone, for the symbology's pass over the
 * photorealistic image (plan R10, R10.T16.c; `terrainDepth.wgsl`): {@link LIT_TERRAIN_SOURCE}, the
 * depth's own `Draw` and fragment stage and the path's source, drawn with a lit view draw's
 * resources (`TerrainPass.drawFor`'s) into the canvas's depth before the overlay's lines, so that
 * nearer ground hides them, each fragment pushed away by `occluderSlopePx` as the occluders are.
 *
 * @remarks
 * It binds what its entry points read alone: R05's vertex storage (four on `face-differences`)
 * and the survey mask, in both stages as the lit material's, which places the fins by its code and
 * discards by its mask, and no texture. Colour writes are off, and it writes depth from its
 * fragment, as R02's occluders do.
 */
export function terrainDepthMaterialSpec(path: TerrainVertexPath): WgslMaterialSpec {
  const source = LIT_TERRAIN_SOURCE + terrainDepthWgsl + PATH_SOURCES[path];
  return {
    name: `terrain-depth-${path}`,
    displayName: DEPTH_DISPLAY_NAMES[path],
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [...LIT_DRAW_UNIFORMS, { name: "occluderSlopePx", type: "f32" }],
    samplers: [],
    textures: [],
    cullMode: "back",
    depthWrite: true,
    colourWrites: false,
    blend: "none",
    storageBuffers: [
      ...terrainVertexStorage(path),
      {
        name: "surveyMask",
        binding: TERRAIN_LIT_BINDINGS.surveyMask,
        stages: ["vertex", "fragment"],
      },
    ],
  };
}

/** The console names of the two paths' shadow casters (decision-r10-t9b-design D4). */
const SHADOW_DISPLAY_NAMES: Readonly<Record<TerrainVertexPath, string>> = {
  "baked-offsets": "TERRAIN SHADOWS OFFSETS",
  "face-differences": "TERRAIN SHADOWS",
};

/**
 * The lit terrain of a vertex path as a caster into the high setting's shadow cascades, depth
 * alone (plan R10, R10.T9.b; Design note 11; `terrainShadow.wgsl`): {@link LIT_TERRAIN_SOURCE},
 * the caster's own `Draw` and fragment stage and the path's source, drawn into each cascade's
 * layer of a `depth-only` target with a lit view draw's own records, storage and `Draw` values,
 * the pass's `Frame` being the cascade's.
 *
 * @remarks
 * Its vertex stage is the lit material's, unchanged, so a patch casts the surface the view draws,
 * at the view's own morph factors. It binds what its entry points read alone, as the depth
 * material does: R05's vertex storage (four on `face-differences`) and the survey mask in both
 * stages, and no texture. Its uniforms are the lit `Draw`'s own three, without the cascades'.
 * Culling is off, so that either face casts (decision-r10-t9b-design D1); colour writes are off,
 * and its depth, the rasteriser's, is biased away from the light by `CASCADE_DEPTH_BIAS`.
 *
 * @param unclippedDepth - `primitive.unclippedDepth`, pancaking casters in front of a cascade's
 *   near plane onto it: the engine's `capabilities.depthClipControl` at every creation and
 *   re-creation, since the engine refuses it on a device without the feature (D1).
 */
export function terrainShadowMaterialSpec(
  path: TerrainVertexPath,
  unclippedDepth: boolean,
): WgslMaterialSpec {
  const source = LIT_TERRAIN_SOURCE + terrainShadowWgsl + PATH_SOURCES[path];
  return {
    name: `terrain-shadow-${path}`,
    displayName: SHADOW_DISPLAY_NAMES[path],
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: LIT_DRAW_UNIFORMS,
    samplers: [],
    textures: [],
    cullMode: "none",
    depthWrite: true,
    colourWrites: false,
    blend: "none",
    storageBuffers: [
      ...terrainVertexStorage(path),
      {
        name: "surveyMask",
        binding: TERRAIN_LIT_BINDINGS.surveyMask,
        stages: ["vertex", "fragment"],
      },
    ],
    depthBiasAway: {
      constant: CASCADE_DEPTH_BIAS.constant,
      slopeScale: CASCADE_DEPTH_BIAS.slopeScale,
      clamp: CASCADE_DEPTH_BIAS.clamp,
    },
    unclippedDepth,
  };
}

/** Both paths' lit materials, for the catalogue. */
export const TERRAIN_LIT_MATERIALS: ReadonlyArray<WgslMaterialSpec> = [
  terrainLitMaterialSpec("baked-offsets"),
  terrainLitMaterialSpec("face-differences"),
];

/** Both paths' depth materials (R10.T16.c), for the catalogue. */
export const TERRAIN_DEPTH_MATERIALS: ReadonlyArray<WgslMaterialSpec> = [
  terrainDepthMaterialSpec("baked-offsets"),
  terrainDepthMaterialSpec("face-differences"),
];

/**
 * Both paths' shadow casters (R10.T9.b), for the catalogue, which compiles them for `depth-only`:
 * without `unclippedDepth`, the path every adapter can take, SwiftShader among them.
 */
export const TERRAIN_SHADOW_MATERIALS: ReadonlyArray<WgslMaterialSpec> = [
  terrainShadowMaterialSpec("baked-offsets", false),
  terrainShadowMaterialSpec("face-differences", false),
];

/**
 * R10.T1.a's photometric laws' codes by name (`material/properties.rs`'s `PhaseLaw`; code 0 names
 * no law): Regolith's, Ice's and Lambert's.
 */
export const PHASE_LAW = { regolith: 1, ice: 2, lambert: 3 } as const;

/** A photometric law's code ({@link PHASE_LAW}). */
export type PhaseLawCode = (typeof PHASE_LAW)[keyof typeof PHASE_LAW];

/**
 * The exponent s of the Ice law's f on R07's `airless-ice` template: the generator's solved
 * exponent for Ganymede's q of 0.80 at L = 1 (plan 14's `photometry.rs`, `S_AIRLESS_ICE`, solved by
 * `exponent_for` to Squyres and Veverka 1981, Icarus 46, 137; provisional there, labelled), which
 * the generator's `templates.golden` pins as `airless_ice`'s `s`.
 */
export const AIRLESS_ICE_EXPONENT = 0.8215365032066868;

/**
 * Each law's template and exponent s, grey (R10.T1.a's `PhaseLaw::template`; decision-r10-limb):
 * Regolith's the Moon's curve at s = 1 (q 0.626), Ice's `airless-ice` at {@link
 * AIRLESS_ICE_EXPONENT}, and Lambert's `lambert` at s = 1, f ≡ 1 (q 3/2; provisional, labelled
 * there).
 */
export const CLASS_LAW_TEMPLATES: Readonly<
  Record<PhaseLawCode, { readonly template: PhaseTemplateId; readonly phaseExponent: number }>
> = {
  [PHASE_LAW.regolith]: { template: "moon", phaseExponent: 1 },
  [PHASE_LAW.ice]: { template: "airless-ice", phaseExponent: AIRLESS_ICE_EXPONENT },
  [PHASE_LAW.lambert]: { template: "lambert", phaseExponent: 1 },
};

function isPhaseLawCode(code: number): code is PhaseLawCode {
  return code === PHASE_LAW.regolith || code === PHASE_LAW.ice || code === PHASE_LAW.lambert;
}

/**
 * The law whose row holds f and L(α) for code `code` at L(0) `l0`: A of 1 (a row holds no A), the
 * law's template and grey s, and the template's L(α) curve (`withTemplateCurve`).
 */
export function classRowLaw(code: PhaseLawCode, l0: number): PhotometricLaw {
  const { template, phaseExponent: s } = CLASS_LAW_TEMPLATES[code];
  return withTemplateCurve({
    a: [1, 1, 1],
    lommelSeeligerShare: l0,
    template,
    phaseExponent: [s, s, s],
  });
}

/** A palette slot's law as the worker resolved it (`TerrainLayers.photometry`). */
export interface ClassPhotometry {
  /** c A_N per display channel (r, g, b): the Johnson R, V and B bands. */
  readonly a: Rgb;
  /** L(0), the law's at c A_N in V. */
  readonly l0: number;
  readonly code: PhaseLawCode;
}

/**
 * Slot `slot`'s law in a bake's photometry, or `null` for a slot with none: empty, or of a
 * substance the palette does not name (law code 0).
 *
 * @throws Error for a law code no law has, which no worker of this build gives.
 */
export function classPhotometryOf(photometry: Float32Array, slot: number): ClassPhotometry | null {
  const at = PHOTOMETRY_STRIDE * slot;
  const [b = 0, v = 0, r = 0, code = 0, l0 = 0] = photometry.subarray(at, at + PHOTOMETRY_STRIDE);
  if (code === 0) {
    return null;
  }
  if (!isPhaseLawCode(code)) {
    throw new Error(`palette slot ${String(slot)} names law ${String(code)}, which no law has`);
  }
  return { a: [r, v, b], l0, code };
}

/**
 * Whether two laws give the same phase table row: the same A, L(0), template and exponents, and the
 * same L(α) profile object (the templates' own, shared), or each none.
 */
function sameLaw(a: PhotometricLaw, b: PhotometricLaw): boolean {
  return (
    a === b ||
    (a.a[0] === b.a[0] &&
      a.a[1] === b.a[1] &&
      a.a[2] === b.a[2] &&
      a.lommelSeeligerShare === b.lommelSeeligerShare &&
      a.lommelSeeligerCurve === b.lommelSeeligerCurve &&
      a.template === b.template &&
      a.phaseExponent[0] === b.phaseExponent[0] &&
      a.phaseExponent[1] === b.phaseExponent[1] &&
      a.phaseExponent[2] === b.phaseExponent[2])
  );
}

/**
 * The rows of a terrain's phase table (`phase_factor_table`): row 0 the body's uniform law, then
 * one row per (law, L(0)) as the patches' class laws ask for them, kept in the order asked.
 *
 * @remarks
 * Rows are only added: a body's palette has at most fifteen entries, and every material up to A_N
 * 0.60 shares its law's row at L(0) = 1, so a body takes a handful. {@link version} rises with each
 * change, so that the holder re-uploads the table.
 */
export class TerrainPhaseRows {
  readonly #laws: PhotometricLaw[];
  readonly #rows = new Map<string, number>();
  #version = 0;

  constructor(elsewhere: PhotometricLaw) {
    this.#laws = [elsewhere];
  }

  /** The rows' laws, row 0 first. */
  get laws(): ReadonlyArray<PhotometricLaw> {
    return this.#laws;
  }

  /** A count of the changes made: a row added or row 0 replaced. */
  get version(): number {
    return this.#version;
  }

  /**
   * Puts `law` in row 0; whether it changed. A law equal to row 0's by value changes nothing, so
   * that the views of one body, each lit by its own frame's law object, upload the table once
   * (plan R10, R10.T16.b).
   */
  setElsewhere(law: PhotometricLaw): boolean {
    const held = this.#laws[0];
    if (held !== undefined && sameLaw(held, law)) {
      return false;
    }
    this.#laws[0] = law;
    this.#version += 1;
    return true;
  }

  /** The row of law `code` at L(0) `l0`, added if it is new. */
  rowOf(code: PhaseLawCode, l0: number): number {
    const key = `${String(code)}:${String(l0)}`;
    const known = this.#rows.get(key);
    if (known !== undefined) {
      return known;
    }
    const row = this.#laws.length;
    this.#laws.push(classRowLaw(code, l0));
    this.#rows.set(key, row);
    this.#version += 1;
    return row;
  }
}

/** `vec4f` rows of one slot's class laws in `class_layers`: two per palette slot. */
export const CLASS_LAW_ROWS = 2 * TERRAIN_CLASSES;

/**
 * A class's A as a texel draws it, per display channel: c A_N times the figure's factor
 * (`oblateAlbedoScale` of its L(0)), as R07's disc scales each of its laws' A (R10.T10.e).
 *
 * @param polarOverEquatorial - The body's c ÷ a, 1 for a sphere.
 */
export function figureScaledA(photometry: ClassPhotometry, polarOverEquatorial: number): Rgb {
  const k = oblateAlbedoScale(photometry.l0, polarOverEquatorial);
  return [photometry.a[0] * k, photometry.a[1] * k, photometry.a[2] * k];
}

/**
 * A patch's class laws as `terrainLit.wgsl`'s `class_layers` holds them after its slot's weights,
 * each palette slot two `vec4f`: A per display channel, scaled for the figure ({@link
 * figureScaledA}), and L(0), then its row (−1 for a slot with no law), the rows taken from `rows`,
 * which adds any it lacks, then whether the slot is drawn from its maturity nodes (R10.T1.c): 1 for
 * a slot with a law where `drawnFromNodes`, else 0, so that a patch without a maturity layer packs
 * what it packed before, bit for bit.
 *
 * @param polarOverEquatorial - The body's c ÷ a, 1 for a sphere.
 * @param drawnFromNodes - Whether the patch carries a maturity layer (`TerrainLayers.maturity`),
 *   whose nodes ({@link packClassNodes}) its named slots are then drawn from.
 */
export function packClassLaws(
  photometry: Float32Array,
  rows: TerrainPhaseRows,
  polarOverEquatorial: number,
  drawnFromNodes = false,
): Float32Array {
  const out = new Float32Array(CLASS_LAW_ROWS * 4);
  for (let k = 0; k < TERRAIN_CLASSES; k += 1) {
    const law = classPhotometryOf(photometry, k);
    if (law === null) {
      out[8 * k + 4] = -1;
      continue;
    }
    out.set([...figureScaledA(law, polarOverEquatorial), law.l0], 8 * k);
    out[8 * k + 4] = rows.rowOf(law.code, law.l0);
    if (drawnFromNodes) {
      out[8 * k + 5] = 1;
    }
  }
  return out;
}

/** The `f32` values of a patch's maturity nodes, as the module gives them and as packed. */
const CLASS_NODE_VALUES = CLASS_NODES_BYTES / 4;

/** The values of one palette slot's nodes: both levels' K nodes of three channels. */
const SLOT_NODE_VALUES = 2 * MATURITY_NODES * 3;

/**
 * A patch's maturity nodes as `terrainLit.wgsl`'s `class_layers` holds them after its maturity
 * codes (R10.T1.c's client half): the module's layout (`TerrainMaturity.nodes`, slot k's level ℓ's
 * node j at `3 (K (2k + ℓ) + j)`), each node's B, V and R turned to the display channels r, g and
 * b as {@link classPhotometryOf} turns a law's A, and scaled for the figure as {@link
 * figureScaledA} scales A: each times `oblateAlbedoScale` of the slot's L(0), the product taken in
 * `f64` and narrowed once; zeros for a slot with no law.
 *
 * @param nodes - The bake's `TerrainMaturity.nodes`.
 * @param photometry - The bake's `TerrainLayers.photometry`, which names each slot's law.
 * @param polarOverEquatorial - The body's c ÷ a, 1 for a sphere.
 * @throws RangeError for nodes of another length than 8 × 2 × K × 3.
 */
export function packClassNodes(
  nodes: Float32Array,
  photometry: Float32Array,
  polarOverEquatorial: number,
): Float32Array {
  if (nodes.length !== CLASS_NODE_VALUES) {
    throw new RangeError(
      `${String(nodes.length)} maturity node values, where a patch has ${String(CLASS_NODE_VALUES)}`,
    );
  }
  const out = new Float32Array(CLASS_NODE_VALUES);
  for (let k = 0; k < TERRAIN_CLASSES; k += 1) {
    const law = classPhotometryOf(photometry, k);
    if (law === null) {
      continue;
    }
    const factor = oblateAlbedoScale(law.l0, polarOverEquatorial);
    for (let at = SLOT_NODE_VALUES * k; at < SLOT_NODE_VALUES * (k + 1); at += 3) {
      out[at] = (nodes[at + 2] ?? 0) * factor;
      out[at + 1] = (nodes[at + 1] ?? 0) * factor;
      out[at + 2] = (nodes[at] ?? 0) * factor;
    }
  }
  return out;
}

/** The stars that light one terrain at most: the brightest two, as R07's disc's. */
export const MAX_TERRAIN_LIGHTS = 2;

/** The occluders one terrain's stars are cut by at most, as R07's disc's. */
export const MAX_TERRAIN_OCCLUDERS = 2;

/** The lit neighbours that light one terrain by planetshine at most, as R07's disc's. */
export const MAX_TERRAIN_SECONDARIES = 2;

/** `vec4f` rows of `terrain_lights` (`terrainLit.wgsl`'s head). */
export const TERRAIN_LIGHT_ROWS = 25;

const FIRST_LIGHT_ROW = 3;
const LIGHT_ROWS = 8;
const FIRST_OCCLUDER_ROW = 19;
const FIRST_SECONDARY_ROW = 21;
const SECONDARY_ROWS = 2;

/**
 * What lights a body's terrain in a frame, as R07's disc record carries it, but along the body's
 * own axes (body-fixed): its stars, the bodies that may eclipse them and its planetshine sources,
 * each relative to the body's centre over its equatorial radius a.
 */
export interface TerrainLighting {
  /** The stars, the brightest first; at most {@link MAX_TERRAIN_LIGHTS} are drawn. */
  readonly lights: ReadonlyArray<DiscLight>;
  readonly occluders: ReadonlyArray<DiscOccluder>;
  readonly secondaries: ReadonlyArray<DiscSecondary>;
  /** The body's uniform law, R07's `elsewhere`: the photometry's law. */
  readonly elsewhere: PhotometricLaw;
  /** The frame's pre-exposure (R02's `exposureScale` of its EV100). */
  readonly exposureScale: number;
}

/**
 * The lights as `terrainLit.wgsl`'s `terrain_lights`, {@link TERRAIN_LIGHT_ROWS} rows, law 0 in row
 * 0 of the phase table and its A scaled for a figure of c ÷ a `polarOverEquatorial`.
 */
export function packTerrainLighting(
  lighting: TerrainLighting,
  polarOverEquatorial: number,
): Float32Array {
  const out = new Float32Array(TERRAIN_LIGHT_ROWS * 4);
  const put = (row: number, values: ReadonlyArray<number>): void => {
    out.set(values, row * 4);
  };
  const lights = lighting.lights.slice(0, MAX_TERRAIN_LIGHTS);
  const occluders = lighting.occluders.slice(0, MAX_TERRAIN_OCCLUDERS);
  const secondaries = lighting.secondaries.slice(0, MAX_TERRAIN_SECONDARIES);
  put(0, [
    lights.length,
    occluders.length,
    secondaries.length,
    lights[0]?.annuli[0].flux.length ?? 0,
  ]);
  const { elsewhere } = lighting;
  const k = oblateAlbedoScale(elsewhere.lommelSeeligerShare, polarOverEquatorial);
  put(1, [
    elsewhere.a[0] * k,
    elsewhere.a[1] * k,
    elsewhere.a[2] * k,
    elsewhere.lommelSeeligerShare,
  ]);
  put(2, [0, lighting.exposureScale / Math.PI, 0, 0]);
  lights.forEach((light, j) => {
    const first = FIRST_LIGHT_ROW + j * LIGHT_ROWS;
    put(first, [light.direction.x, light.direction.y, light.direction.z, light.distance]);
    put(first + 1, [...light.illuminance, light.radius]);
    light.annuli.forEach((set, c) => {
      put(
        first + 2 + 2 * c,
        [1, 2, 3, 4].map((edge) => set.edges[edge] ?? 1),
      );
      put(
        first + 3 + 2 * c,
        [0, 1, 2, 3].map((annulus) => set.flux[annulus] ?? 0),
      );
    });
  });
  occluders.forEach((occluder, slot) => {
    const c = occluder.centre;
    put(FIRST_OCCLUDER_ROW + slot, [c.x, c.y, c.z, occluder.radius]);
  });
  secondaries.forEach((source, j) => {
    const first = FIRST_SECONDARY_ROW + j * SECONDARY_ROWS;
    const d = source.direction;
    put(first, [d.x, d.y, d.z, source.distance]);
    put(first + 1, [...source.illuminance, source.radius]);
  });
  return out;
}

/**
 * A texel's class weights at (u, v) of its patch, each in [0, 1]: each slot's byte ÷ 255
 * interpolated bilinearly between its quad's four vertices, as `terrainLit.wgsl`'s
 * `terrain_shares` reads them; all 0 where the weights were not baked.
 *
 * @param weights - The bake's `TerrainLayers.weights`, 8 bytes a vertex.
 * @param u - The patch's x ÷ 64, 0 to 1.
 * @param v - The patch's y ÷ 64, 0 to 1.
 */
export function texelWeights(weights: Uint8Array, u: number, v: number): number[] {
  const out = Array.from({ length: TERRAIN_CLASSES }, () => 0);
  if (weights.length === 0) {
    return out;
  }
  const quads = PATCH_VERTICES_PER_SIDE - 1;
  const sx = Math.max(u * quads, 0);
  const sy = Math.max(v * quads, 0);
  const x = Math.min(Math.floor(sx), quads - 1);
  const y = Math.min(Math.floor(sy), quads - 1);
  const fx = Math.min(Math.max(sx - x, 0), 1);
  const fy = Math.min(Math.max(sy - y, 0), 1);
  const at = (vx: number, vy: number, k: number): number =>
    (weights[TERRAIN_CLASSES * (vy * PATCH_VERTICES_PER_SIDE + vx) + k] ?? 0) / 255;
  for (let k = 0; k < TERRAIN_CLASSES; k += 1) {
    const below = at(x, y, k) + fx * (at(x + 1, y, k) - at(x, y, k));
    const above = at(x, y + 1, k) + fx * (at(x + 1, y + 1, k) - at(x, y + 1, k));
    out[k] = below + fy * (above - below);
  }
  return out;
}

/** One level's quad of a texel's maturity: its four corners' segments and the bilinear fractions. */
interface MaturityQuad {
  /** Corners (0, 0), (1, 0), (0, 1) and (1, 1) of the quad, in that order. */
  readonly corners: ReadonlyArray<MaturitySegment>;
  readonly fx: number;
  readonly fy: number;
}

/**
 * The quad of corner (`x`, `y`) and side `span` vertices (1 the patch's own, 2 the parent's on the
 * even vertices), each corner's code at `level` (0 the patch's band level, 1 its parent's) as its
 * segment, as `terrainLit.wgsl`'s `maturity_quad`.
 */
function maturityQuad(
  codes: Uint16Array,
  x: number,
  y: number,
  span: number,
  level: number,
  fx: number,
  fy: number,
): MaturityQuad {
  const corners = [0, 1, 2, 3].map((c) => {
    const vx = x + span * (c & 1);
    const vy = y + span * (c >> 1);
    const code = codes[2 * (vy * PATCH_VERTICES_PER_SIDE + vx) + level];
    if (code === undefined) {
      throw new RangeError(`vertex (${String(vx)}, ${String(vy)}) has no maturity code`);
    }
    return maturitySegment(maturityCodeRatio(code));
  });
  return { corners, fx, fy };
}

/** WGSL's `mix(a, b, t)`, a (1 − t) + b t. */
function wgslMix(a: number, b: number, t: number): number {
  return a * (1 - t) + b * t;
}

/**
 * Palette slot `slot`'s albedo over `quad` at `level` from the packed nodes, the bilinear mean of
 * its corners' interpolated nodes, as `terrainLit.wgsl`'s `maturity_quad_albedo`.
 */
function maturityQuadAlbedo(
  nodes: Float32Array,
  slot: number,
  level: number,
  quad: MaturityQuad,
): Rgb {
  const first = 3 * MATURITY_NODES * (2 * slot + level);
  const [c00, c10, c01, c11] = quad.corners.map((segment) =>
    maturityNodeAlbedo(nodes, first, segment),
  );
  if (c00 === undefined || c10 === undefined || c01 === undefined || c11 === undefined) {
    throw new Error("a maturity quad has four corners");
  }
  const channel = (c: 0 | 1 | 2): number =>
    wgslMix(wgslMix(c00[c], c10[c], quad.fx), wgslMix(c01[c], c11[c], quad.fx), quad.fy);
  return [channel(0), channel(1), channel(2)];
}

/**
 * Palette slot `slot`'s A at (u, v) of its patch with morph factor `morph`, drawn from its maturity
 * nodes (R10.T1.c's client half; decision-r10-t1c-client-scope §1.4), the twin of `terrainLit.wgsl`'s
 * `terrain_class_albedo`, operation for operation in `f64`: the bilinear mean over the texel's quad
 * of the slot's nodes at the patch's band level n, each vertex at its own code at n, mixed by the
 * morph factor μ with the bilinear mean over the parent's quad, on the even vertices, of its nodes
 * at n − 1, each vertex at its code at n − 1; with explicit ends, so that μ ≤ 0 draws the patch's
 * own field and μ ≥ 1 the parent's exactly, which a bilinear field on the parent's grid draws on
 * the child's.
 *
 * @param codes - The bake's `TerrainMaturity.codes`.
 * @param nodes - The patch's packed nodes ({@link packClassNodes}): display channels, figure
 *   scaled.
 * @param slot - The palette slot, 0 to 7.
 * @param u - The patch's x ÷ 64, 0 to 1.
 * @param v - The patch's y ÷ 64, 0 to 1.
 * @param morph - The texel's morph factor μ: 0 the patch's own field, 1 its parent's.
 * @throws RangeError for codes or nodes too short for the patch's vertices or the slot.
 */
export function texelClassAlbedo(
  codes: Uint16Array,
  nodes: Float32Array,
  slot: number,
  u: number,
  v: number,
  morph: number,
): Rgb {
  const quads = PATCH_VERTICES_PER_SIDE - 1;
  const sx = Math.max(u * quads, 0);
  const sy = Math.max(v * quads, 0);
  const own = (): Rgb => {
    const x = Math.min(Math.floor(sx), quads - 1);
    const y = Math.min(Math.floor(sy), quads - 1);
    const fx = Math.min(Math.max(sx - x, 0), 1);
    const fy = Math.min(Math.max(sy - y, 0), 1);
    return maturityQuadAlbedo(nodes, slot, 0, maturityQuad(codes, x, y, 1, 0, fx, fy));
  };
  const parent = (): Rgb => {
    const x = Math.min(2 * Math.floor(sx / 2), quads - 2);
    const y = Math.min(2 * Math.floor(sy / 2), quads - 2);
    const fx = Math.min(Math.max((sx - x) / 2, 0), 1);
    const fy = Math.min(Math.max((sy - y) / 2, 0), 1);
    return maturityQuadAlbedo(nodes, slot, 1, maturityQuad(codes, x, y, 2, 1, fx, fy));
  };
  if (!(morph > 0)) {
    return own();
  }
  if (morph >= 1) {
    return parent();
  }
  const [a, b] = [own(), parent()];
  return [
    (1 - morph) * a[0] + morph * b[0],
    (1 - morph) * a[1] + morph * b[1],
    (1 - morph) * a[2] + morph * b[2],
  ];
}

/**
 * The share of each of a texel's {@link TERRAIN_LAWS} laws from its slots' weights: each slot with
 * a law at its weight, scaled down to sum to 1 where they sum to more, and law 0 (`elsewhere`)
 * what they leave, an unnamed slot's weight among it (`terrain_shares`).
 *
 * @param weights - The slots' weights, each in [0, 1] ({@link texelWeights}).
 * @param named - Whether each slot has a law (its `classPhotometryOf` is not `null`).
 */
export function terrainShares(
  weights: ReadonlyArray<number>,
  named: ReadonlyArray<boolean>,
): number[] {
  const shares = Array.from({ length: TERRAIN_LAWS }, () => 0);
  let total = 0;
  for (let k = 0; k < TERRAIN_CLASSES; k += 1) {
    if (named[k] === true) {
      const w = weights[k] ?? 0;
      shares[k + 1] = w;
      total += w;
    }
  }
  if (total > 1) {
    for (let m = 1; m < TERRAIN_LAWS; m += 1) {
      shares[m] = (shares[m] ?? 0) / total;
    }
  }
  shares[0] = total >= 1 ? 0 : 1 - total;
  return shares;
}

/** One of a texel's laws as the twin reads it: its A and its own row's table. */
export interface TexelLaw {
  readonly a: Rgb;
  readonly table: PhaseFactorTable;
}

/** A law's {@link TexelLaw}: its A and its own table. */
export function texelLawOf(law: PhotometricLaw): TexelLaw {
  return { a: law.a, table: phaseFactorTableOf(law) };
}

/**
 * A class's law as a texel draws it: its A scaled for the figure ({@link figureScaledA}), and the
 * table of its row's law, f against its L(α) ({@link classRowLaw}).
 *
 * @param polarOverEquatorial - The body's c ÷ a, 1 for a sphere.
 */
export function classTexelLaw(photometry: ClassPhotometry, polarOverEquatorial: number): TexelLaw {
  return {
    a: figureScaledA(photometry, polarOverEquatorial),
    table: phaseFactorTableOf(classRowLaw(photometry.code, photometry.l0)),
  };
}

/** Where a texel lies in a patch with a maturity layer, and its morph (R10.T1.c's client half). */
export interface TexelMaturity {
  /** The bake's `TerrainLayers.maturity`. */
  readonly maturity: TerrainMaturity;
  /** The patch's x ÷ 64, 0 to 1. */
  readonly u: number;
  /** The patch's y ÷ 64, 0 to 1. */
  readonly v: number;
  /** The texel's morph factor μ: 0 the patch's own field, 1 its parent's. */
  readonly morph: number;
}

/**
 * A bake's {@link TERRAIN_LAWS} laws as the lit terrain draws them, the twin of {@link
 * packTerrainLighting}'s law 0 and {@link packClassLaws}' slots: law 0, `elsewhere` with its A
 * scaled for the figure (`oblateAlbedoScale` of its L(0)), then each palette slot's ({@link
 * classTexelLaw}), `null` for a slot with none; and, at a texel of a patch with a maturity layer
 * (`at`), each slot's A drawn from its nodes there ({@link texelClassAlbedo} over {@link
 * packClassNodes}), its table and law 0 as without (R10.T1.c's client half). Without `at` it is
 * what it was, bit for bit.
 *
 * @param photometry - The bake's `TerrainLayers.photometry`.
 * @param polarOverEquatorial - The body's c ÷ a, 1 for a sphere.
 * @param at - The texel's place in a patch with a maturity layer, and its morph factor.
 * @throws RangeError for a maturity layer of another size than a patch's.
 */
export function terrainTexelLaws(
  photometry: Float32Array,
  elsewhere: PhotometricLaw,
  polarOverEquatorial: number,
  at?: TexelMaturity,
): Array<TexelLaw | null> {
  const k = oblateAlbedoScale(elsewhere.lommelSeeligerShare, polarOverEquatorial);
  const law0: TexelLaw = {
    a: [elsewhere.a[0] * k, elsewhere.a[1] * k, elsewhere.a[2] * k],
    table: phaseFactorTableOf(elsewhere),
  };
  const nodes =
    at === undefined ? null : packClassNodes(at.maturity.nodes, photometry, polarOverEquatorial);
  return [
    law0,
    ...Array.from({ length: TERRAIN_CLASSES }, (_, slot): TexelLaw | null => {
      const law = classPhotometryOf(photometry, slot);
      if (law === null) {
        return null;
      }
      const texel = classTexelLaw(law, polarOverEquatorial);
      return at === undefined || nodes === null
        ? texel
        : {
            ...texel,
            a: texelClassAlbedo(at.maturity.codes, nodes, slot, at.u, at.v, at.morph),
          };
    }),
  ];
}

/**
 * The reflectance I/F of a texel, R07.T4.c's twin (`brdfFromTable`): Σ sₘ rₘ over its laws, each
 * rₘ its law's I/F through its own row, a zero share skipped, so that a law with a zero share adds
 * nothing whatever its values.
 *
 * @param laws - Law 0 (`elsewhere`) then each slot's, `null` for a slot with none; at a texel of a
 *   patch with a maturity layer, each slot's A drawn from its nodes there ({@link
 *   terrainTexelLaws} with its texel, R10.T1.c's client half).
 * @param shares - Each law's share ({@link terrainShares}).
 * @param mu0 - Cosine of the incidence angle on the texel's normal.
 * @param mu - Cosine of the emission angle.
 * @param phaseRad - Phase angle, rad.
 */
export function terrainReflectance(
  laws: ReadonlyArray<TexelLaw | null>,
  shares: ReadonlyArray<number>,
  mu0: number,
  mu: number,
  phaseRad: number,
): Rgb {
  const out: [number, number, number] = [0, 0, 0];
  laws.forEach((law, m) => {
    const share = shares[m] ?? 0;
    if (law === null || !(share > 0)) {
      return;
    }
    const r = brdfFromTable(law, law.table, mu0, mu, phaseRad);
    out[0] += share * r[0];
    out[1] += share * r[1];
    out[2] += share * r[2];
  });
  return out;
}

/**
 * The palette slot of a texel's largest weight, the class a readout names; the lowest such slot on
 * a tie; `null` where no slot has weight.
 */
export function dominantSlot(weights: ReadonlyArray<number>): number | null {
  let best: number | null = null;
  let largest = 0;
  weights.forEach((w, k) => {
    if (w > largest) {
      largest = w;
      best = k;
    }
  });
  return best;
}
