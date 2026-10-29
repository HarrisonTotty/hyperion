/**
 * Venus's geometric albedo on the reference tracer's plane-parallel Venus deck (plan R08, R08.T5.e;
 * `science-r08-unowned-absorbers.md` §1.2): the case `hyperion-fit atmosphere-reference` traces for
 * it, from `venus-92bar`'s own medium, and the albedo read from its reference.
 *
 * @remarks
 * - **The albedo.** p(λ) = 2 ∫₀¹ R(μ, μ; α = 0) μ dμ, R = π I ÷ F the radiance factor at
 *   opposition, the sun behind the observer: 2A ÷ 3 for a Lambert plane of albedo A. On
 *   {@link ALBEDO_NODES}' eight Gauss–Legendre nodes in μ, p = Σ wₖ μₖ R(μₖ), the case's sun of
 *   irradiance π so that R is the radiance itself.
 * - **The case**: `venus-92bar`'s column and terms (`reference/thickCases.ts`) on a sphere of
 *   10¹² m, plane-parallel, at {@link ALBEDO_WAVELENGTHS_NM}, its four modes' optics at each, mode 2
 *   on the material file it is handed (the committed `acid_uv_absorber`'s, or a fit's trial), seen
 *   from above the top along each node's μ with a pencil detector. Its SO₂ is left out: the client's
 *   cross-sections start at 380 nm and are 0 from 417 nm, so it holds nothing at the bands' and
 *   nothing to draw at 365 nm, a stated omission (its tail below the deck darkens the deep air
 *   there).
 * - **The terms** are written to the thick cases' 9 figures, as `venus-92bar`'s are, each table with
 *   its mode's Mie g as its mean cosine but where the table's own as read parts from it by more than
 *   the tracer's check ({@link ALBEDO_TABLE_G_TOLERANCE}), a stated exception.
 */

import { gaussLegendre } from "../view/lighting/quadrature";
import type { Rgb } from "../view/photometry/toneCurve";
import {
  type AerosolTermOptics,
  aerosolModeOptics,
  aerosolTerm,
  phaseTableAsymmetry,
} from "../view/atmosphere/aerosol";
import type { MediumTerm } from "../view/atmosphere/medium";
import { indexAt, type MaterialIndexFile } from "../view/atmosphere/materials/materials";
import { molecularMixture, molecularTerm } from "../view/atmosphere/rayleigh";
import { modeOpticsWithIndex, type ModeOptics } from "../view/atmosphere/sizeDistribution";
import {
  THICK_CASE_SIGNIFICANT_FIGURES,
  VENUS_AIR,
  VENUS_DECK_MODES,
  VENUS_GROUND_ALBEDO,
  VENUS_SURFACE_PRESSURES_PA,
  venusColumn,
  type VenusDeckMode,
  venusModeParticles,
  venusModeSpec,
} from "../view/atmosphere/reference/thickCases";
import {
  CASE_FORMAT,
  type CaseJson,
  densityJson,
  roundedTermJson,
  tabulatedPhaseJson,
  type TermJson,
} from "./atmosphereCases";

/**
 * The wavelengths the albedo is taken at, nm (R08.T5.e): 365, Lee et al. 2019's (Y. J. Lee et
 * al., AJ 158 (2019) 126); and Johnson B and V and Sloan g′ and r′ at the effective wavelengths
 * Mallama et al. state for the bands their albedos are in (A. Mallama, B. Krobusek and H. Pavlov,
 * Icarus 282 (2017) 19, Tables 1 and 2: B 0.436, V 0.549, g′ 0.469 and r′ 0.616 µm), since no
 * response's licence was found permissive (the plan's fallback; Bessell 1990's tables are the
 * Astronomical Society of the Pacific's, the USNO 40-inch curves state no licence).
 */
export const ALBEDO_WAVELENGTHS_NM = {
  u365: 365,
  b: 436,
  v: 549,
  g: 469,
  r: 616,
} as const;

/** A band of {@link ALBEDO_WAVELENGTHS_NM}. */
export type AlbedoBand = keyof typeof ALBEDO_WAVELENGTHS_NM;

/** The bands in the order the case traces them. */
export const ALBEDO_BANDS: ReadonlyArray<AlbedoBand> = ["u365", "b", "v", "g", "r"];

/**
 * How far a table's own mean cosine as read may part from its mode's Mie g before the case gives
 * it the table's: 10⁻⁴, the tracer's load check (`case.rs`'s `PHASE_CHECK_TOLERANCE`). At 365 nm
 * mode 3's 256 entries read 1.2 × 10⁻⁴ above its g, its glory and ripple finer than the entries;
 * the channels' tables pass.
 */
export const ALBEDO_TABLE_G_TOLERANCE = 1e-4;

/** The plane-parallel slab's sphere, m: 10¹², as the tracer's benchmarks lay their slabs. */
export const ALBEDO_SLAB_RADIUS_M = 1e12;

/** How far above the top the observer stands, m. */
export const ALBEDO_OBSERVER_ABOVE_TOP_M = 1000;

/** One Gauss–Legendre node of the albedo's μ quadrature on [0, 1]. */
export interface AlbedoNode {
  readonly mu: number;
  /** wₖ on [−1, 1], so that p = Σ wₖ μₖ R(μₖ). */
  readonly weight: number;
}

/** The albedo's eight nodes in μ, ascending: μ = (1 + x) ÷ 2 of the 8-point rule. */
export const ALBEDO_NODES: ReadonlyArray<AlbedoNode> = (() => {
  const rule = gaussLegendre(8);
  return Array.from(rule.x, (x, k) => ({
    mu: 0.5 * (1 + x),
    weight: rule.w[k] ?? Number.NaN,
  })).toSorted((a, b) => a.mu - b.mu);
})();

/** The geometry at node `k`: from above the top, looking down along μₖ, the sun behind. */
function albedoGeometry(
  k: number,
  node: AlbedoNode,
  topHeightM: number,
): CaseJson["geometries"][number] {
  const zenithDeg = (Math.acos(node.mu) * 180) / Math.PI;
  return {
    name: `albedo-n${k}`,
    observer: { heightM: topHeightM + ALBEDO_OBSERVER_ABOVE_TOP_M, latitudeDeg: 0 },
    view: { zenithDeg: 180 - zenithDeg, azimuthDeg: 0 },
    sunDirections: [{ zenithDeg, azimuthDeg: 180 }],
  };
}

/** What a mode's optics are read from: its committed material, or mode 2 on a given file. */
function modeOpticsAt(
  deckMode: VenusDeckMode,
  acidFile: MaterialIndexFile | undefined,
  wavelengthNm: number,
): ModeOptics {
  const particles = venusModeParticles(deckMode);
  if (deckMode.material !== "acid_uv_absorber" || acidFile === undefined) {
    return aerosolModeOptics(particles, wavelengthNm);
  }
  if (particles.kind !== "distribution") {
    throw new Error("the deck's modes are distributions");
  }
  const index = indexAt(
    {
      material: deckMode.material,
      file: acidFile,
      provenance: "standIn",
      standIn: acidFile.standIn,
    },
    wavelengthNm,
  );
  return modeOpticsWithIndex(particles.mode, index, wavelengthNm);
}

/** The wavelengths in groups of three, the last padded with its own last, for `aerosolTerm`. */
function triples(
  wavelengthsNm: ReadonlyArray<number>,
): ReadonlyArray<readonly [number, number, number]> {
  const out: Array<readonly [number, number, number]> = [];
  for (let i = 0; i < wavelengthsNm.length; i += 3) {
    const a = wavelengthsNm[i] ?? Number.NaN;
    const b = wavelengthsNm[i + 1] ?? a;
    const c = wavelengthsNm[i + 2] ?? b;
    out.push([a, b, c]);
  }
  return out;
}

/**
 * A deck mode's term at every one of `wavelengthsNm`: `aerosolTerm` on the column at each three in
 * turn, so that its density and σ_ext(550) are `venus-92bar`'s, its rows each wavelength's own.
 */
function modeTermJson(
  deckMode: VenusDeckMode,
  acidFile: MaterialIndexFile | undefined,
  wavelengthsNm: ReadonlyArray<number>,
): TermJson {
  const column = venusColumn(VENUS_SURFACE_PRESSURES_PA["venus-92bar"]);
  // Each wavelength's Mie once, the triples' padding and 550 nm reusing it.
  const computed = new Map<number, ModeOptics>();
  const at = (nm: number): ModeOptics => {
    const known = computed.get(nm);
    if (known !== undefined) {
      return known;
    }
    const optics = modeOpticsAt(deckMode, acidFile, nm);
    computed.set(nm, optics);
    return optics;
  };
  const at550 = at(550);
  const parts = triples(wavelengthsNm).map((three) => {
    const optics: AerosolTermOptics = {
      channels: [at(three[0]), at(three[1]), at(three[2])],
      at550,
    };
    const term = aerosolTerm(venusModeSpec(deckMode), column, optics);
    const { phase: drawn } = term.term;
    if (drawn.kind !== "tabulated") {
      throw new Error(`the deck's mode ${deckMode.name} has a tabulated phase`);
    }
    // The mode's Mie g, or the table's own as read where the two part by more than the tracer's
    // check (the module's remarks).
    const pick = (c: 0 | 1 | 2): number => {
      const g = optics.channels[c].asymmetry;
      const read = phaseTableAsymmetry(drawn.table, c);
      return Math.abs(read - g) > ALBEDO_TABLE_G_TOLERANCE ? read : g;
    };
    const asymmetry: Rgb = [pick(0), pick(1), pick(2)];
    const phase = tabulatedPhaseJson(drawn.table, asymmetry);
    if (phase.kind !== "tabulated" || phase.matrix === undefined) {
      throw new Error(`the deck's mode ${deckMode.name} has a tabulated phase with its matrix`);
    }
    return { term: term.term, phase, matrix: phase.matrix };
  });
  const take = <T>(rows: (part: (typeof parts)[number]) => readonly T[]): T[] =>
    parts.flatMap(rows).slice(0, wavelengthsNm.length);
  const first = parts[0];
  if (first === undefined) {
    throw new Error("the albedo is taken at one wavelength or more");
  }
  return {
    name: deckMode.name,
    density: densityJson(first.term.density),
    scattering: take((p) => p.term.scattering),
    absorption: take((p) => p.term.absorption),
    phase: {
      kind: "tabulated",
      u: first.phase.u,
      a1: take((p) => p.phase.a1),
      asymmetry: take((p) => p.phase.asymmetry),
      matrix: {
        a2: take((p) => p.matrix.a2),
        a3: take((p) => p.matrix.a3),
        a4: take((p) => p.matrix.a4),
        b1: take((p) => p.matrix.b1),
        b2: take((p) => p.matrix.b2),
      },
    },
  };
}

/** The Venus-class air's molecular term at every one of `wavelengthsNm`, on `venus-92bar`'s column. */
function rayleighTermJson(wavelengthsNm: ReadonlyArray<number>): TermJson {
  const column = venusColumn(VENUS_SURFACE_PRESSURES_PA["venus-92bar"]);
  const term: MediumTerm = molecularTerm(column, VENUS_AIR).term;
  const mixture = molecularMixture(VENUS_AIR);
  return {
    name: term.name,
    density: densityJson(term.density),
    scattering: wavelengthsNm.map(
      (nm) => column.surfaceNumberDensityPerM3 * mixture.crossSectionM2(nm),
    ),
    absorption: wavelengthsNm.map(() => 0),
    phase: {
      kind: "rayleigh",
      depolarisation: wavelengthsNm.map((nm) => mixture.depolarisation(nm)),
    },
  };
}

/** What an albedo case is built for. */
export interface AlbedoCaseOptions {
  readonly name: string;
  readonly seed: number;
  /** The bands to trace, {@link ALBEDO_BANDS} by default. */
  readonly bands?: ReadonlyArray<AlbedoBand>;
  /**
   * Mode 2's material file: a fit's trial, or for the deck without the absorber the 75 wt% acid's;
   * the committed `acid_uv_absorber`'s if absent.
   */
  readonly acidFile?: MaterialIndexFile;
}

/** The albedo case (the module's remarks), in the tracer's case format. */
export function albedoCaseJson(options: AlbedoCaseOptions): CaseJson {
  const bands = options.bands ?? ALBEDO_BANDS;
  const wavelengthsNm = bands.map((band) => ALBEDO_WAVELENGTHS_NM[band]);
  const column = venusColumn(VENUS_SURFACE_PRESSURES_PA["venus-92bar"]);
  const terms = [
    rayleighTermJson(wavelengthsNm),
    ...VENUS_DECK_MODES.map((m) => modeTermJson(m, options.acidFile, wavelengthsNm)),
  ].map((t) => roundedTermJson(t, THICK_CASE_SIGNIFICANT_FIGURES));
  return {
    format: CASE_FORMAT,
    name: options.name,
    seed: options.seed,
    wavelengthsNm,
    shells: { kind: "sphere", radiusM: ALBEDO_SLAB_RADIUS_M },
    topHeightM: column.topHeightM,
    groundAlbedo: wavelengthsNm.map(() => VENUS_GROUND_ALBEDO),
    terms,
    suns: [{ name: "sun", irradiance: wavelengthsNm.map(() => Math.PI) }],
    detectorHalfAngleDeg: 0,
    geometries: ALBEDO_NODES.map((node, k) => albedoGeometry(k, node, column.topHeightM)),
    aggregates: [],
  };
}

/** One traced geometry, the fields the albedo reads. */
export interface AlbedoGeometryFile {
  readonly name: string;
  readonly radiance: readonly number[];
  readonly standardError: readonly number[];
}

/** A traced albedo case, the fields the albedo reads. */
export interface AlbedoReferenceFile {
  readonly wavelengthsNm: readonly number[];
  readonly geometries: readonly AlbedoGeometryFile[];
}

/** The numbers of a JSON array, or a thrown error naming `what`. */
function numbersOf(value: unknown, what: string): number[] {
  if (!Array.isArray(value) || !value.every((v) => typeof v === "number")) {
    throw new Error(`${what} is an array of numbers`);
  }
  return value;
}

/**
 * A traced albedo reference read from its JSON, the fields {@link geometricAlbedo} reads.
 *
 * @throws Error for a file without them.
 */
export function readAlbedoReference(raw: unknown): AlbedoReferenceFile {
  if (typeof raw !== "object" || raw === null) {
    throw new Error("an albedo reference is a JSON object");
  }
  const geometries: unknown = Reflect.get(raw, "geometries");
  if (!Array.isArray(geometries)) {
    throw new Error("an albedo reference has its geometries");
  }
  return {
    wavelengthsNm: numbersOf(Reflect.get(raw, "wavelengthsNm"), "wavelengthsNm"),
    geometries: geometries.map((g: unknown, k) => {
      const name: unknown =
        typeof g === "object" && g !== null ? Reflect.get(g, "name") : undefined;
      if (typeof g !== "object" || g === null || typeof name !== "string") {
        throw new Error(`geometry ${k} has its name`);
      }
      return {
        name,
        radiance: numbersOf(Reflect.get(g, "radiance"), `${name}'s radiance`),
        standardError: numbersOf(Reflect.get(g, "standardError"), `${name}'s standard error`),
      };
    }),
  };
}

/** A geometric albedo and its standard error. */
export interface GeometricAlbedo {
  readonly albedo: number;
  readonly standardError: number;
}

/**
 * The geometric albedo at a traced wavelength (the module's remarks): Σ wₖ μₖ Iₖ over the nodes,
 * F = π, its σ from the nodes' independent σ.
 *
 * @throws Error for a reference that lacks a node or the wavelength.
 */
export function geometricAlbedo(
  reference: AlbedoReferenceFile,
  wavelengthNm: number,
): GeometricAlbedo {
  const w = reference.wavelengthsNm.indexOf(wavelengthNm);
  if (w < 0) {
    throw new Error(`the albedo reference has no ${wavelengthNm} nm`);
  }
  let albedo = 0;
  let variance = 0;
  for (const [k, node] of ALBEDO_NODES.entries()) {
    const geometry = reference.geometries.find((g) => g.name === `albedo-n${k}`);
    const radiance = geometry?.radiance[w];
    const sigma = geometry?.standardError[w];
    if (radiance === undefined || sigma === undefined) {
      throw new Error(`the albedo reference has no node ${k}`);
    }
    albedo += node.weight * node.mu * radiance;
    variance += (node.weight * node.mu * sigma) ** 2;
  }
  return { albedo, standardError: Math.sqrt(variance) };
}
