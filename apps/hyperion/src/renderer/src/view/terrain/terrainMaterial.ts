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
 *   (Johnson R, V and B as r, g and b, as R07's `modelledPhotometry` maps them), L(0) the law's at
 *   c A_N, and the law's template and exponent s ({@link CLASS_LAW_TEMPLATES}). Its row of R07's
 *   phase table holds its f against its L(α), L(0) times McEwen's profile for the particulate laws
 *   and 0 for Lambert's, in alpha ({@link TerrainPhaseRows}): one row per (law, L(0)), so the
 *   materials of L(0) = 1, every one up to A_N 0.60, share one row of their law, and a bright
 *   frost takes its own (decision-r10-limb). The task's "three rows" are read as three kinds.
 * - **Law 0** is the body's uniform law (R07's `elsewhere`, the photometry's `lawFor`), its A
 *   scaled for the figure as R07's disc scales it (`oblateAlbedoScale`), and takes what the
 *   weights leave: the share of a slot whose palette names no substance, and of the vertices
 *   outside the survey, which have no weights, so that the terrain shows there what the disc shows
 *   ({@link terrainShares}; T10.c then discards those texels over the reference surface).
 * - **The twin** ({@link terrainReflectance}) is R07.T4.c's: each law's I/F through `brdfFromTable`
 *   with its own row, as `body_brdf` reads it; the shader's `terrain_reflectance` replaces μ₀ by
 *   a source's irradiance factor and floors the Lommel–Seeliger term at the source's angular
 *   radius, as R07's disc does (`lit_disc_term`), which equals the twin wherever the whole source
 *   is up and μ₀ + μ exceeds that radius.
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
import type { StorageBufferSpec, WgslMaterialSpec } from "../engine/types";
import type { Rgb } from "../photometry/toneCurve";
import type { TerrainVertexPath } from "../quality/qualitySetting";
import frameWgsl from "../shaders/frame.wgsl?raw";
import litBodyWgsl from "../shaders/litBody.wgsl?raw";
import { PATH_SOURCES, TERRAIN_BINDINGS } from "./gpu/material";
import patchVertexWgsl from "./shaders/patchVertex.wgsl?raw";
import terrainLitWgsl from "./shaders/terrainLit.wgsl?raw";
import terrainPatchWgsl from "./shaders/terrainPatch.wgsl?raw";
import { PATCH_VERTICES_PER_SIDE } from "./slotLayout";
import { PHOTOMETRY_STRIDE } from "./workers/messages";

/** A palette's slots: `hyperion_surface`'s `MAX_CLASSES`. */
export const TERRAIN_CLASSES = 8;

/** The laws a texel shades with: law 0, `elsewhere`, and one per palette slot. */
export const TERRAIN_LAWS = TERRAIN_CLASSES + 1;

/** The `@group(2)` bindings of `terrainLit.wgsl`: R05's, then the lit material's. */
export const TERRAIN_LIT_BINDINGS = {
  ...TERRAIN_BINDINGS,
  /** Each slot's class weights and laws, one storage buffer (`slotLayout.ts`' `class-layers`). */
  classLayers: 6,
  phaseFactorTable: 7,
  terrainLights: 8,
  /** The first of the seven tables of R08's surface lighting. */
  atmosphere: 9,
} as const;

/** The console names of the two paths' lit materials. */
const DISPLAY_NAMES: Readonly<Record<TerrainVertexPath, string>> = {
  "baked-offsets": "LIT TERRAIN OFFSETS",
  "face-differences": "LIT TERRAIN",
};

/**
 * The lit terrain material of a vertex path: `frame.wgsl`, `patchVertex.wgsl`, R08's surface
 * lighting, R07's `litBody.wgsl`, `terrainPatch.wgsl`, `terrainLit.wgsl` and the path's source.
 */
export function terrainLitMaterialSpec(path: TerrainVertexPath): WgslMaterialSpec {
  const source =
    frameWgsl +
    patchVertexWgsl +
    SURFACE_LIGHTING_WGSL +
    litBodyWgsl +
    terrainPatchWgsl +
    terrainLitWgsl +
    PATH_SOURCES[path];
  const b = TERRAIN_LIT_BINDINGS;
  const storageBuffers: StorageBufferSpec[] = [
    { name: "heights", binding: b.heights },
    { name: "slots", binding: b.slots },
    { name: "instances", binding: b.instances },
    { name: "contacts", binding: b.contacts },
    { name: "classLayers", binding: b.classLayers },
    { name: "terrainLights", binding: b.terrainLights },
  ];
  if (path === "baked-offsets") {
    storageBuffers.push({ name: "offsets", binding: b.offsets });
  }
  return {
    name: `terrain-lit-${path}`,
    displayName: DISPLAY_NAMES[path],
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [
      { name: "bodyRotation", type: "mat4x4f" },
      { name: "cameraBodyFixedM", type: "vec4f" },
      { name: "atlas", type: "vec4f" },
    ],
    samplers: [],
    textures: [
      { name: "normals", binding: b.normals, viewDimension: "2d-array" },
      { name: "phaseFactorTable", binding: b.phaseFactorTable, sampleType: "unfilterable-float" },
      ...surfaceLightingTextures(b.atmosphere),
    ],
    cullMode: "back",
    depthWrite: true,
    colourWrites: true,
    blend: "none",
    storageBuffers,
  };
}

/** Both paths' lit materials, for the catalogue. */
export const TERRAIN_LIT_MATERIALS: ReadonlyArray<WgslMaterialSpec> = [
  terrainLitMaterialSpec("baked-offsets"),
  terrainLitMaterialSpec("face-differences"),
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

  /** Puts `law` in row 0; whether it changed. */
  setElsewhere(law: PhotometricLaw): boolean {
    if (this.#laws[0] === law) {
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
 * A patch's class laws as `terrainLit.wgsl`'s `class_layers` holds them after its slot's weights,
 * each palette slot two `vec4f`: A per display channel and L(0), then its row (−1 for a slot with
 * no law), the rows taken from `rows`, which adds any it lacks.
 */
export function packClassLaws(photometry: Float32Array, rows: TerrainPhaseRows): Float32Array {
  const out = new Float32Array(CLASS_LAW_ROWS * 4);
  for (let k = 0; k < TERRAIN_CLASSES; k += 1) {
    const law = classPhotometryOf(photometry, k);
    if (law === null) {
      out[8 * k + 4] = -1;
      continue;
    }
    out.set([...law.a, law.l0], 8 * k);
    out[8 * k + 4] = rows.rowOf(law.code, law.l0);
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
 * A class's law as a texel draws it: its A, and the table of its row's law, f against its L(α)
 * ({@link classRowLaw}).
 */
export function classTexelLaw(photometry: ClassPhotometry): TexelLaw {
  return {
    a: photometry.a,
    table: phaseFactorTableOf(classRowLaw(photometry.code, photometry.l0)),
  };
}

/**
 * The reflectance I/F of a texel, R07.T4.c's twin (`brdfFromTable`): Σ sₘ rₘ over its laws, each
 * rₘ its law's I/F through its own row, a zero share skipped, so that a law with a zero share adds
 * nothing whatever its values.
 *
 * @param laws - Law 0 (`elsewhere`) then each slot's, `null` for a slot with none.
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
