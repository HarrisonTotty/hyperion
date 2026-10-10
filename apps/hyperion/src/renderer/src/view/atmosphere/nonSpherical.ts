/**
 * The optics of non-spherical aerosol modes, minerals and ice crystals (plan R08, R08.T5.c; Design
 * note 6; science-r08-nonspherical.md): published models' tables, integrated over a mode's size
 * distribution, with the rules for where a model holds, where an analogue stands in, and where a
 * labelled fallback remains.
 *
 * @remarks
 * - **Minerals** (`nonSphericalMineral`: dust, the silicates, solid iron; and a `crystal` that is
 *   not an ice) take TAMUdust2020's ensemble of 20 irregular hexahedra at sphericity 0.71 (Saito et
 *   al. 2021; the best fit to Martian analogues' measured matrices, Martikainen et al. 2025), from
 *   the reduced tables in `materials/phase/` (adapted CC BY 4.0 material; `src/tools/nonSpherical.ts`
 *   made them). The hexahedra run 20–37% low at side and back angles against a Martian analogue
 *   and 57–85% high against enstatite, a weakly absorbing crystalline silicate, with every sign
 *   right (R08's Risks).
 * - **The floor rule.** Below a kernel's k floor (10⁻⁴ shortwave, 10⁻³ longwave) the table is the
 *   floor's, with its absorption scaled by k ÷ k_floor, a `model` while the mode's own ω on the
 *   floor's table is at least {@link FLOOR_ALBEDO_MIN}; past that an `analogue`, or a `fallback`
 *   above n 1.70, where no kernel reaches.
 * - **Water ice** takes Yang et al. 2013's severely roughened aggregate of eight columns (version 2),
 *   the habit that best fits in-situ cirrus scattering (Järvinen et al. 2018). Crystals below the
 *   table's smallest maximum dimension, 2 µm, are each a collection of independent spheres of their
 *   volume-to-area radius, Ã ÷ πr̃² of them at the table's smallest crystal, so that the crystal's
 *   V and A are kept (T. C. Grenfell and S. G. Warren, J. Geophys. Res. 104 (1999) 31697–31709),
 *   by Mie; the mode is an `analogue` when that tail carries more than
 *   {@link SMALL_CRYSTAL_SHARE_MAX} of its scattering at 550 nm.
 * - **The other ices** ({@link ICE_KEYS}) take water ice's table at their own size parameter as a
 *   named `analogue`, with Mie's cross-sections and ω for their own index; their asymmetry is
 *   expected to run high by {@link ICE_ANALOGUE_ASYMMETRY_BIAS}.
 * - **The fallback**, for a mineral that no table or kernel covers, is a Henyey–Greenstein phase
 *   from Mie's g with Mie's ω and no matrix; below {@link SMALL_GRAIN_SIZE_PARAMETER} the sphere's
 *   own Mie matrix instead, still a `fallback`, since every compact shape tends to the dipole phase
 *   (3 ÷ 4)(1 + cos²θ) as x → 0, where Henyey–Greenstein tends to isotropy.
 *
 * A mode's radius is the volume-to-area radius r = 3V ÷ 4A, so its r_eff is the effective radius
 * (3 ÷ 4)⟨V⟩ ÷ ⟨A⟩ of non-spherical particles. A table node of size parameter x = 2πD ÷ λ (D the
 * maximum dimension) stands at r̃ = 3Ṽ ÷ 4Ã in units of λ ÷ 2π. Between a table's size nodes the
 * cross-sections are log-linear and a₁ log-linear in ln r̃, g and the ratios linear; between its
 * wavelength nodes the mode's sums are linear in λ. Below a table's smallest size, the sphere of
 * the same volume (minerals, right in the Rayleigh limit the tail lies in) or Grenfell and Warren's
 * spheres of the same volume-to-area radius (ice) by Mie.
 */

import enstatiteGlassPhase from "./materials/phase/enstatite-glass.json" with { type: "json" };
import forsteriteAmorphousPhase from "./materials/phase/forsterite-amorphous.json" with { type: "json" };
import ironPhase from "./materials/phase/iron.json" with { type: "json" };
import marsDustPhase from "./materials/phase/mars-dust.json" with { type: "json" };
import waterIcePhase from "./materials/phase/water-ice.json" with { type: "json" };
import {
  type MaterialIndexFile,
  type MaterialProvenance,
  type ResolvedMaterial,
  indexAt,
} from "./materials/materials";
import { gaussLegendre } from "../lighting/quadrature";
import { type ComplexIndex, mieSphere } from "./mie";
import {
  type AerosolMode,
  type ModeOptics,
  type PhaseBasis,
  type PhaseModel,
  PHASE_TABLE_MU,
  type ScatteringMatrix,
  type SizeDistribution,
  logNumberPerLnRadius,
  modeShape,
  resolveModeMaterial,
  sizeRange,
  sphereModeOptics,
} from "./sizeDistribution";

// ---------------------------------------------------------------------------------------------
// The phase files.

/** One wavelength node of a phase file, decoded. */
export interface PhaseTableNode {
  readonly wavelengthNm: number;
  /** The material's index at the node. */
  readonly n: number;
  readonly k: number;
  /** The TAMUdust2020 kernel read, or undefined for Yang et al.'s table. */
  readonly kernel: "shortwave" | "longwave" | undefined;
  /** The k the table was made at: the material's, or the kernel's floor beneath it. */
  readonly kTable: number;
  /** Per size node: x = 2πD ÷ λ, and Ṽ, Ã, C̃_ext, C̃_sca, C̃_sca·g in powers of 2π ÷ λ. */
  readonly sizeParameters: Float64Array;
  readonly volumes: Float64Array;
  readonly areas: Float64Array;
  readonly extinction: Float64Array;
  readonly scattering: Float64Array;
  readonly scatteringAsymmetry: Float64Array;
  /** ln a₁, size-major over the file's phase angles (∫ a₁ dΩ = 4π for each size). */
  readonly logA1: Float64Array;
  /** a₂ ÷ a₁, a₃ ÷ a₁, a₄ ÷ a₁, b₁ ÷ a₁ and b₂ ÷ a₁, size-major over the matrix angles. */
  readonly ratios: readonly [Float64Array, Float64Array, Float64Array, Float64Array, Float64Array];
}

/** A phase file, checked and decoded: one material's reduced table. */
export interface PhaseFileTable {
  /** The index file it was made for, by name, and that file's key, phase and variant. */
  readonly material: string;
  readonly key: string;
  readonly phase: string;
  readonly variant: string | undefined;
  readonly model: "tamudust2020" | "yang2013";
  readonly particles: string;
  readonly paper: string;
  readonly source: string;
  readonly licence: string;
  readonly reduction: string;
  readonly phaseAnglesDeg: Float64Array;
  readonly matrixAnglesDeg: Float64Array;
  readonly nodes: ReadonlyArray<PhaseTableNode>;
}

/** The files' quantisation of a₁, round(1000 ln a₁), and of the ratios, round(30,000 × ratio). */
const A1_SCALE = 1000;
const RATIO_SCALE = 30_000;

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function textOf(raw: Readonly<Record<string, unknown>>, field: string, file: string): string {
  const value = raw[field];
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${file}: "${field}" must be a non-empty string`);
  }
  return value;
}

function numbersOf(
  raw: Readonly<Record<string, unknown>>,
  field: string,
  file: string,
): Float64Array {
  const value = raw[field];
  if (!Array.isArray(value) || !value.every((v) => typeof v === "number" && Number.isFinite(v))) {
    throw new Error(`${file}: "${field}" must be an array of finite numbers`);
  }
  return Float64Array.from(value, Number);
}

/** Little-endian 16-bit integers from base64. */
function int16Of(text: string): Int16Array {
  const bytes = atob(text);
  const out = new Int16Array(bytes.length / 2);
  for (let i = 0; i < out.length; i += 1) {
    const value = bytes.charCodeAt(2 * i) | (bytes.charCodeAt(2 * i + 1) << 8);
    out[i] = value >= 0x8000 ? value - 0x10000 : value;
  }
  return out;
}

function nodeOf(
  raw: unknown,
  file: string,
  phaseCount: number,
  matrixCount: number,
): PhaseTableNode {
  if (!isRecord(raw)) {
    throw new Error(`${file}: a node is not an object`);
  }
  const number = (field: string): number => {
    const v = raw[field];
    if (typeof v !== "number" || !Number.isFinite(v)) {
      throw new Error(`${file}: a node's "${field}" must be a finite number`);
    }
    return v;
  };
  const kernel = raw["kernel"];
  if (kernel !== null && kernel !== "shortwave" && kernel !== "longwave") {
    throw new Error(`${file}: a node's kernel must be shortwave, longwave or null`);
  }
  const sizeParameters = numbersOf(raw, "sizeParameters", file);
  const sizes = sizeParameters.length;
  const perSize = [
    numbersOf(raw, "volumes", file),
    numbersOf(raw, "areas", file),
    numbersOf(raw, "extinction", file),
    numbersOf(raw, "scattering", file),
    numbersOf(raw, "scatteringAsymmetry", file),
  ] as const;
  if (sizes < 2 || perSize.some((a) => a.length !== sizes)) {
    throw new Error(`${file}: a node needs at least two sizes and one value each`);
  }
  if (perSize.slice(0, 4).some((a) => a.some((v) => !(v > 0)))) {
    throw new Error(`${file}: a node's volumes, areas and cross-sections must be > 0`);
  }
  const [volumes, areas, extinction, scattering, scatteringAsymmetry] = perSize;
  for (let s = 0; s < sizes; s += 1) {
    const rTilde = (volumes[s] ?? Number.NaN) / (areas[s] ?? Number.NaN);
    if (s > 0 && !(rTilde > (volumes[s - 1] ?? Number.NaN) / (areas[s - 1] ?? Number.NaN))) {
      throw new Error(`${file}: a node's sizes must ascend strictly in V ÷ A, at size ${s}`);
    }
    const sca = scattering[s] ?? Number.NaN;
    // Rounding to the files' figures may put C_sca a part in 10⁹ above C_ext for ω = 1.
    if (!(sca <= (extinction[s] ?? Number.NaN) * (1 + 1e-8))) {
      throw new Error(`${file}: a node's C_sca exceeds its C_ext at size ${s}`);
    }
    if (!(Math.abs((scatteringAsymmetry[s] ?? Number.NaN) / sca) <= 1)) {
      throw new Error(`${file}: a node's |g| exceeds 1 at size ${s}`);
    }
  }
  const decoded = (field: string, count: number, scale: number): Float64Array => {
    const values = int16Of(textOf(raw, field, file));
    if (values.length !== sizes * count) {
      throw new Error(`${file}: "${field}" has ${values.length} values, not ${sizes * count}`);
    }
    return Float64Array.from(values, (v) => v / scale);
  };
  return {
    wavelengthNm: number("wavelengthNm"),
    n: number("n"),
    k: number("k"),
    kernel: kernel === null ? undefined : kernel,
    kTable: number("kTable"),
    sizeParameters,
    volumes,
    areas,
    extinction,
    scattering,
    scatteringAsymmetry,
    logA1: decoded("a1", phaseCount, A1_SCALE),
    ratios: [
      decoded("a2", matrixCount, RATIO_SCALE),
      decoded("a3", matrixCount, RATIO_SCALE),
      decoded("a4", matrixCount, RATIO_SCALE),
      decoded("b1", matrixCount, RATIO_SCALE),
      decoded("b2", matrixCount, RATIO_SCALE),
    ],
  };
}

/**
 * A phase file checked and decoded.
 *
 * @throws Error naming the file and the field when it breaks the format.
 */
export function parsePhaseFile(raw: unknown, file: string): PhaseFileTable {
  if (!isRecord(raw)) {
    throw new Error(`${file}: not a JSON object`);
  }
  const model = raw["model"];
  if (model !== "tamudust2020" && model !== "yang2013") {
    throw new Error(`${file}: "model" must be tamudust2020 or yang2013`);
  }
  const phaseAnglesDeg = numbersOf(raw, "phaseAnglesDeg", file);
  const matrixAnglesDeg = numbersOf(raw, "matrixAnglesDeg", file);
  for (const angles of [phaseAnglesDeg, matrixAnglesDeg]) {
    if (
      angles[0] !== 0 ||
      angles.at(-1) !== 180 ||
      angles.some((a, i) => i > 0 && !(a > (angles[i - 1] ?? Infinity)))
    ) {
      throw new Error(`${file}: angles must ascend strictly from 0° to 180°`);
    }
  }
  const rawNodes = raw["nodes"];
  if (!Array.isArray(rawNodes) || rawNodes.length === 0) {
    throw new Error(`${file}: "nodes" must be a non-empty array`);
  }
  const nodes = rawNodes.map((n) => nodeOf(n, file, phaseAnglesDeg.length, matrixAnglesDeg.length));
  if (nodes.some((n, i) => i > 0 && !(n.wavelengthNm > (nodes[i - 1]?.wavelengthNm ?? Infinity)))) {
    throw new Error(`${file}: nodes must ascend in wavelength`);
  }
  const variant = raw["variant"];
  if (variant !== null && (typeof variant !== "string" || variant.length === 0)) {
    throw new Error(`${file}: "variant" must be null or a non-empty string`);
  }
  return {
    material: textOf(raw, "material", file),
    key: textOf(raw, "key", file),
    phase: textOf(raw, "phase", file),
    variant: variant ?? undefined,
    model,
    particles: textOf(raw, "particles", file),
    paper: textOf(raw, "paper", file),
    source: textOf(raw, "source", file),
    licence: textOf(raw, "licence", file),
    reduction: textOf(raw, "reduction", file),
    phaseAnglesDeg,
    matrixAnglesDeg,
    nodes,
  };
}

/** Every phase file, by the material index file each was made for. */
export const PHASE_FILES: ReadonlyArray<PhaseFileTable> = [
  parsePhaseFile(marsDustPhase, "phase/mars-dust.json"),
  parsePhaseFile(enstatiteGlassPhase, "phase/enstatite-glass.json"),
  parsePhaseFile(forsteriteAmorphousPhase, "phase/forsterite-amorphous.json"),
  parsePhaseFile(ironPhase, "phase/iron.json"),
  parsePhaseFile(waterIcePhase, "phase/water-ice.json"),
];

/** The phase file made for a material index file, if any. */
export function phaseFileOf(file: MaterialIndexFile): PhaseFileTable | undefined {
  return PHASE_FILES.find(
    (p) => p.key === file.key && p.phase === file.phase && p.variant === file.variant,
  );
}

// ---------------------------------------------------------------------------------------------
// Constants of the rules.

/** The floor rule holds while the mode's ω on the floor's table is at least this (science-r08-nonspherical.md §2.2). */
export const FLOOR_ALBEDO_MIN = 0.99;

/** The largest real index either kernel reaches with k below 10⁻³: the shortwave kernel's 1.70. */
export const KERNEL_TRANSPARENT_N_MAX = 1.7;

/**
 * The share of a water-ice mode's scattering at 550 nm that crystals below the table's smallest,
 * 2 µm in maximum dimension, may carry before the mode is an `analogue` (a stated convention,
 * science-r08-nonspherical.md §2.3).
 */
export const SMALL_CRYSTAL_SHARE_MAX = 0.1;

/**
 * The effective size parameter 2π r_eff ÷ λ below which a fallback-class mineral takes the
 * sphere's own Mie table and matrix rather than Henyey–Greenstein (science-r08-nonspherical.md
 * §2.5): where the shortwave kernel's hexahedra and volume-to-area spheres of the same r agree to 5%
 * in a₁ at every angle of the phase files, measured at n 1.60, k 10⁻⁴ and sphericity 0.712 on the
 * committed excerpt (`fixtures/tamudust-excerpt.json`, `nonSpherical.test.ts`).
 */
export const SMALL_GRAIN_SIZE_PARAMETER = 0.22;

/**
 * The ices: a `crystal` of one of these takes Yang et al.'s water-ice table, as the model (H₂O)
 * or a named analogue; a `crystal` of anything else takes the hexahedra like a mineral.
 *
 * @remarks
 * Closed set, for the composition audit: the volatile ices R08 and T5.d hold files for or name
 * (science-r08-nonspherical.md §2.3). A new ice the registry adds belongs here.
 */
export const ICE_KEYS: ReadonlySet<string> = new Set([
  "H2O",
  "NH3",
  "CH4",
  "CO2",
  "NH4SH",
  "H2S",
  "N2",
  "CO",
]);

/**
 * The expected error in g of an ice drawn on water ice's table, Δg = g_expected − g_water, by van
 * Diedenhoven et al. 2014's eq. (16) with g_tot = (1 + g_RT) ÷ 2 at 550 nm (J. Atmos. Sci. 71,
 * 1763; science-r08-nonspherical.md §2.3): estimates of the bias, not corrections. Ices not listed
 * have none estimated. NH₄SH's n of 1.644 lies past the fit's 1.18–1.43, where eq. (16) would give
 * about −0.10, the largest of all; it is left out as an extrapolation (R08.T5.c's science check).
 */
export const ICE_ANALOGUE_ASYMMETRY_BIAS: Readonly<Record<string, number>> = {
  NH3: -0.056,
  CO2: -0.048,
  CH4: -0.007,
};

// ---------------------------------------------------------------------------------------------
// Distributions.

/**
 * A mode's number density over the volume-to-area radius, for the tables' integral: the range it
 * covers, µm, and ln(r n(r)) up to a constant as a function of ln(r ÷ 1 µm).
 */
export interface RadiusDistribution {
  readonly fromUm: number;
  readonly toUm: number;
  readonly logNumberPerLnR: (lnRadiusUm: number) => number;
}

/**
 * A gamma or log-normal distribution as a {@link RadiusDistribution}, over {@link sizeRange}.
 *
 * @throws RangeError for an effective radius or variance outside the distribution's range.
 */
export function radiusDistribution(sizes: SizeDistribution): RadiusDistribution {
  const range = sizeRange(sizes);
  return {
    fromUm: range.fromUm,
    toUm: range.toUm,
    logNumberPerLnR: logNumberPerLnRadius(sizes),
  };
}

/**
 * A tabulated number distribution n(r) as a {@link RadiusDistribution}: ln(r n(r)) linear in ln r
 * between the points, nothing outside them.
 *
 * @param radiiUm - Ascending, > 0.
 * @param numberDensity - n(r) at each radius, per µm, ≥ 0.
 * @throws RangeError for fewer than two points, mismatched lengths or a bad value.
 */
export function tabulatedRadiusDistribution(
  radiiUm: ArrayLike<number>,
  numberDensity: ArrayLike<number>,
): RadiusDistribution {
  const count = radiiUm.length;
  if (count < 2 || numberDensity.length !== count) {
    throw new RangeError("a tabulated distribution needs at least two radii and one density each");
  }
  const lnR = Float64Array.from({ length: count }, (_, i) => Math.log(radiiUm[i] ?? Number.NaN));
  const lnRn = Float64Array.from({ length: count }, (_, i) => {
    const r = radiiUm[i] ?? Number.NaN;
    const n = numberDensity[i] ?? Number.NaN;
    if (!(r > 0 && n >= 0 && Number.isFinite(n))) {
      throw new RangeError(`a tabulated distribution's radius ${r} and density ${n} are bad`);
    }
    return n > 0 ? Math.log(r * n) : -Infinity;
  });
  if (lnR.some((v, i) => i > 0 && !(v > (lnR[i - 1] ?? Infinity)))) {
    throw new RangeError("a tabulated distribution's radii must ascend strictly");
  }
  return {
    fromUm: radiiUm[0] ?? Number.NaN,
    toUm: radiiUm[count - 1] ?? Number.NaN,
    logNumberPerLnR: (y) => {
      if (y < (lnR[0] ?? Infinity) || y > (lnR[count - 1] ?? -Infinity)) {
        return -Infinity;
      }
      let i = 0;
      while (i < count - 2 && (lnR[i + 1] ?? Infinity) <= y) {
        i += 1;
      }
      const a = lnRn[i] ?? -Infinity;
      const b = lnRn[i + 1] ?? -Infinity;
      if (a === -Infinity || b === -Infinity) {
        return -Infinity;
      }
      const t =
        (y - (lnR[i] ?? Number.NaN)) / ((lnR[i + 1] ?? Number.NaN) - (lnR[i] ?? Number.NaN));
      return a + t * (b - a);
    },
  };
}

// ---------------------------------------------------------------------------------------------
// The integral over a table.

/** No scattering angles: Mie then sums the cross-sections alone. */
const NO_ANGLES = new Float64Array(0);

/** The indices of a matrix's five elements beside a₁: a₂, a₃, a₄, b₁, b₂. */
export const ELEMENT_INDICES = [0, 1, 2, 3, 4] as const;

/** The Gauss–Legendre rule on each panel of the size integral (order 6), as (node, weight). */
const SIZE_GL: ReadonlyArray<readonly [number, number]> = (() => {
  const rule = gaussLegendre(6);
  return Array.from(rule.x, (x, i) => [x, rule.w[i] ?? Number.NaN] as const);
})();

/** The widest panel of the size integral in ln r, so a narrow distribution is resolved. */
const SIZE_PANEL_LN = 0.1;

/** A mode's sums over one table node: per particle when divided by `number`. */
interface NodeSums {
  readonly number: number;
  readonly area: number;
  readonly volume: number;
  readonly extinction: number;
  readonly scattering: number;
  readonly scatteringAsymmetry: number;
  /** Σ C_sca a₁ at the phase angles. */
  readonly a1: Float64Array;
  /** Σ C_sca a₁ at the matrix angles. */
  readonly a1Matrix: Float64Array;
  /** Σ C_sca a₁ × ratio at the matrix angles, for a₂, a₃, a₄, b₁, b₂. */
  readonly matrix: readonly [Float64Array, Float64Array, Float64Array, Float64Array, Float64Array];
  /** Σ C_sca of the particles below the table's smallest size, by Mie. */
  readonly tailScattering: number;
  /** Σ C_ext of the same particles, at their own index. */
  readonly tailExtinction: number;
}

/** The index of the last node at or below v on an ascending array, clamped to [0, length − 2]. */
function lowerNode(values: Float64Array, v: number): number {
  let lo = 0;
  let hi = values.length - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >>> 1;
    if ((values[mid] ?? Number.NaN) <= v) {
      lo = mid;
    } else {
      hi = mid;
    }
  }
  return lo;
}

/** a₁ at the abscissae u = √(θ ÷ π) asked, from its values at the phase angles: ln a₁ linear in u. */
function a1At(
  logA1: Float64Array,
  offset: number,
  phaseU: Float64Array,
  targetU: Float64Array,
): Float64Array {
  return targetU.map((u) => {
    const i = lowerNode(phaseU, u);
    const u0 = phaseU[i] ?? Number.NaN;
    const t = Math.min(Math.max((u - u0) / ((phaseU[i + 1] ?? Number.NaN) - u0), 0), 1);
    const a = logA1[offset + i] ?? Number.NaN;
    const b = logA1[offset + i + 1] ?? Number.NaN;
    return Math.exp(a + t * (b - a));
  });
}

/** A table's angles as the integral reads them: u = √(θ ÷ π) of each set, and the tail's cosines. */
interface TableAngles {
  readonly phaseU: Float64Array;
  readonly matrixU: Float64Array;
  readonly tailMu: Float64Array;
}

const TABLE_ANGLES = new WeakMap<PhaseFileTable, TableAngles>();

function cosineOfDegrees(degrees: number): number {
  return Math.cos((degrees * Math.PI) / 180);
}

/** A table's {@link TableAngles}, computed once a table. */
function anglesOf(table: PhaseFileTable): TableAngles {
  let angles = TABLE_ANGLES.get(table);
  if (angles === undefined) {
    angles = {
      phaseU: table.phaseAnglesDeg.map((a) => Math.sqrt(a / 180)),
      matrixU: table.matrixAnglesDeg.map((a) => Math.sqrt(a / 180)),
      tailMu: Float64Array.from(
        [...table.phaseAnglesDeg, ...table.matrixAnglesDeg],
        cosineOfDegrees,
      ),
    };
    TABLE_ANGLES.set(table, angles);
  }
  return angles;
}

/** What a table's integral needs of the particles and the light. */
interface TableIntegral {
  readonly table: PhaseFileTable;
  readonly node: PhaseTableNode;
  readonly wavelengthNm: number;
  readonly index: ComplexIndex;
  /** The equivalent spheres below the table: `volume` for minerals, `volumeToArea` for ice (V and A kept). */
  readonly tailSphere: "volume" | "volumeToArea";
}

/** Sums of a mode over one node's table at a wavelength (the module's remarks). */
function sumsOverNode(integral: TableIntegral, distribution: RadiusDistribution): NodeSums {
  const { table, node, wavelengthNm, index } = integral;
  const lengthUm = wavelengthNm / 1000 / (2 * Math.PI);
  const sizes = node.sizeParameters.length;
  // r̃ = 3Ṽ ÷ 4Ã at each node, in units of λ ÷ 2π.
  const rTilde = Float64Array.from(
    { length: sizes },
    (_, s) => (0.75 * (node.volumes[s] ?? 0)) / (node.areas[s] ?? 1),
  );
  const lnR = rTilde.map((r) => Math.log(r * lengthUm));
  const top = lnR[sizes - 1] ?? Number.NaN;
  const lnFrom = Math.log(distribution.fromUm);
  const lnTo = Math.log(distribution.toUm);
  if (lnTo > top + 1e-9) {
    throw new RangeError(
      `the distribution reaches ${distribution.toUm} µm, past the ${table.material} table's ${Math.exp(top)} µm at ${wavelengthNm} nm`,
    );
  }
  const phaseCount = table.phaseAnglesDeg.length;
  const matrixCount = table.matrixAnglesDeg.length;
  const { phaseU, matrixU, tailMu } = anglesOf(table);
  // a₁ at the matrix angles, per size node, which each panel's two ends read.
  const a1AtMatrix = new Map<number, Float64Array>();
  const a1MatrixAt = (node0: number): Float64Array => {
    let values = a1AtMatrix.get(node0);
    if (values === undefined) {
      values = a1At(node.logA1, node0 * phaseCount, phaseU, matrixU);
      a1AtMatrix.set(node0, values);
    }
    return values;
  };
  const first = lnR[0] ?? Number.NaN;
  // The equivalent sphere's radius over r_VA at the smallest node, and the spheres per particle:
  // one volume-equivalent sphere for a mineral; for ice, Grenfell and Warren 1999's Ã ÷ πr̃²
  // spheres of the crystal's r_VA, which keep its V and A.
  const v0 = node.volumes[0] ?? Number.NaN;
  const r0 = rTilde[0] ?? Number.NaN;
  let sphereOverVa: number;
  let spheresPerParticle: number;
  switch (integral.tailSphere) {
    case "volume":
      sphereOverVa = Math.cbrt((3 * v0) / (4 * Math.PI)) / r0;
      spheresPerParticle = 1;
      break;
    case "volumeToArea":
      sphereOverVa = 1;
      spheresPerParticle = (node.areas[0] ?? Number.NaN) / (Math.PI * r0 * r0);
      break;
  }

  // Panel ends: the distribution's ends, every node inside, and steps of at most SIZE_PANEL_LN.
  const ends = [lnFrom];
  for (const l of lnR) {
    if (l > lnFrom && l < lnTo) {
      ends.push(l);
    }
  }
  ends.push(lnTo);
  const panels: number[] = [ends[0] ?? lnFrom];
  for (let i = 1; i < ends.length; i += 1) {
    const a = ends[i - 1] ?? Number.NaN;
    const b = ends[i] ?? Number.NaN;
    const steps = Math.max(1, Math.ceil((b - a) / SIZE_PANEL_LN));
    for (let s = 1; s <= steps; s += 1) {
      panels.push(a + ((b - a) * s) / steps);
    }
  }
  // The number weights, relative to the largest, so no exponent overflows.
  const nodesY: number[] = [];
  const nodesW: number[] = [];
  let peak = -Infinity;
  for (let p = 0; p + 1 < panels.length; p += 1) {
    const a = panels[p] ?? Number.NaN;
    const half = 0.5 * ((panels[p + 1] ?? Number.NaN) - a);
    for (const [x, w] of SIZE_GL) {
      const y = a + half * (1 + x);
      const logW = Math.log(half * w) + distribution.logNumberPerLnR(y);
      nodesY.push(y);
      nodesW.push(logW);
      peak = Math.max(peak, logW);
    }
  }
  let number = 0;
  let area = 0;
  let volume = 0;
  let extinction = 0;
  let scattering = 0;
  let scatteringAsymmetry = 0;
  let tailScattering = 0;
  let tailExtinction = 0;
  const a1 = new Float64Array(phaseCount);
  const a1Matrix = new Float64Array(matrixCount);
  const matrix = [
    new Float64Array(matrixCount),
    new Float64Array(matrixCount),
    new Float64Array(matrixCount),
    new Float64Array(matrixCount),
    new Float64Array(matrixCount),
  ] as const;
  const um2 = lengthUm * lengthUm;
  for (const [q, y] of nodesY.entries()) {
    const w = Math.exp((nodesW[q] ?? -Infinity) - peak);
    if (!(w > 0)) {
      continue;
    }
    const rUm = Math.exp(y);
    number += w;
    if (y < first) {
      // Below the table: the equivalent spheres by Mie, each particle's weight on all but number.
      const ws = w * spheresPerParticle;
      const sphereUm = sphereOverVa * rUm;
      const x = sphereUm / lengthUm;
      const mie = mieSphere(x, index, tailMu);
      const geometric = Math.PI * sphereUm * sphereUm;
      const cSca = mie.qSca * geometric;
      area += ws * geometric;
      volume += ws * (4 / 3) * Math.PI * sphereUm ** 3;
      extinction += ws * mie.qExt * geometric;
      scattering += ws * cSca;
      tailScattering += ws * cSca;
      tailExtinction += ws * mie.qExt * geometric;
      scatteringAsymmetry += ws * cSca * mie.asymmetry;
      const amplitude = (j: number): readonly [number, number, number, number] => [
        mie.s1[2 * j] ?? 0,
        mie.s1[2 * j + 1] ?? 0,
        mie.s2[2 * j] ?? 0,
        mie.s2[2 * j + 1] ?? 0,
      ];
      // a₁ = 2(|S₁|² + |S₂|²) ÷ (x² Q_sca), so C_sca a₁ = 2π (|S₁|² + |S₂|²) r² ÷ x².
      const perSr = (2 * geometric) / (x * x * Math.PI);
      for (let j = 0; j < phaseCount; j += 1) {
        const [r1, i1, r2, i2] = amplitude(j);
        a1[j] = (a1[j] ?? 0) + ws * perSr * Math.PI * (r1 * r1 + i1 * i1 + r2 * r2 + i2 * i2);
      }
      for (let j = 0; j < matrixCount; j += 1) {
        const [r1, i1, r2, i2] = amplitude(phaseCount + j);
        const s1Sq = r1 * r1 + i1 * i1;
        const s2Sq = r2 * r2 + i2 * i2;
        const c = ws * perSr * Math.PI * 2;
        const at = (e: 0 | 1 | 2 | 3 | 4, v: number): void => {
          matrix[e][j] = (matrix[e][j] ?? 0) + c * v;
        };
        a1Matrix[j] = (a1Matrix[j] ?? 0) + c * 0.5 * (s1Sq + s2Sq);
        at(0, 0.5 * (s1Sq + s2Sq));
        at(1, r1 * r2 + i1 * i2);
        at(2, r1 * r2 + i1 * i2);
        at(3, 0.5 * (s2Sq - s1Sq));
        at(4, r1 * i2 - i1 * r2);
      }
      continue;
    }
    const s = lowerNode(lnR, y);
    const t = Math.min(Math.max((y - (lnR[s] ?? 0)) / ((lnR[s + 1] ?? 0) - (lnR[s] ?? 0)), 0), 1);
    const logLerp = (values: Float64Array): number =>
      Math.exp(
        Math.log(values[s] ?? Number.NaN) +
          t * (Math.log(values[s + 1] ?? Number.NaN) - Math.log(values[s] ?? Number.NaN)),
      );
    const cExt = logLerp(node.extinction) * um2;
    const cSca = logLerp(node.scattering) * um2;
    const g0 = (node.scatteringAsymmetry[s] ?? 0) / (node.scattering[s] ?? 1);
    const g1 = (node.scatteringAsymmetry[s + 1] ?? 0) / (node.scattering[s + 1] ?? 1);
    area += w * logLerp(node.areas) * um2;
    volume += w * logLerp(node.volumes) * um2 * lengthUm;
    extinction += w * cExt;
    scattering += w * cSca;
    scatteringAsymmetry += w * cSca * (g0 + t * (g1 - g0));
    for (let j = 0; j < phaseCount; j += 1) {
      const la = node.logA1[s * phaseCount + j] ?? Number.NaN;
      const lb = node.logA1[(s + 1) * phaseCount + j] ?? Number.NaN;
      a1[j] = (a1[j] ?? 0) + w * cSca * Math.exp(la + t * (lb - la));
    }
    const a1Lo = a1MatrixAt(s);
    const a1Hi = a1MatrixAt(s + 1);
    for (let j = 0; j < matrixCount; j += 1) {
      const a1Here = Math.exp(
        Math.log(a1Lo[j] ?? Number.NaN) +
          t * (Math.log(a1Hi[j] ?? Number.NaN) - Math.log(a1Lo[j] ?? Number.NaN)),
      );
      a1Matrix[j] = (a1Matrix[j] ?? 0) + w * cSca * a1Here;
      for (const e of ELEMENT_INDICES) {
        const ratio = node.ratios[e];
        const ra = ratio[s * matrixCount + j] ?? Number.NaN;
        const rb = ratio[(s + 1) * matrixCount + j] ?? Number.NaN;
        const element = matrix[e];
        element[j] = (element[j] ?? 0) + w * cSca * a1Here * (ra + t * (rb - ra));
      }
    }
  }
  return {
    number,
    area,
    volume,
    extinction,
    scattering,
    scatteringAsymmetry,
    a1,
    a1Matrix,
    matrix,
    tailScattering,
    tailExtinction,
  };
}

/**
 * A node's sums with the absorption of its table's particles scaled by a factor: the floor rule's
 * k ÷ k_floor, or ice's k ÷ k_node between nodes. The particles below the table, by Mie at their
 * own index already, keep theirs.
 */
function scaledAbsorption(sums: NodeSums, factor: number): NodeSums {
  const tableAbsorption =
    sums.extinction - sums.tailExtinction - (sums.scattering - sums.tailScattering);
  const tailAbsorption = sums.tailExtinction - sums.tailScattering;
  return { ...sums, extinction: sums.scattering + tableAbsorption * factor + tailAbsorption };
}

/** The mode's ω on the table as made: its table's particles alone, unless they are none. */
function tableAlbedoOf(sums: NodeSums): number {
  const extinction = sums.extinction - sums.tailExtinction;
  return extinction > 0
    ? (sums.scattering - sums.tailScattering) / extinction
    : sums.scattering / sums.extinction;
}

function blend(a: NodeSums, b: NodeSums, t: number): NodeSums {
  if (t === 0) {
    return a;
  }
  const mix = (x: number, y: number): number => (1 - t) * x + t * y;
  const mixArray = (x: Float64Array, y: Float64Array): Float64Array =>
    x.map((v, i) => mix(v, y[i] ?? Number.NaN));
  return {
    number: mix(a.number, b.number),
    area: mix(a.area, b.area),
    volume: mix(a.volume, b.volume),
    extinction: mix(a.extinction, b.extinction),
    scattering: mix(a.scattering, b.scattering),
    scatteringAsymmetry: mix(a.scatteringAsymmetry, b.scatteringAsymmetry),
    a1: mixArray(a.a1, b.a1),
    a1Matrix: mixArray(a.a1Matrix, b.a1Matrix),
    matrix: [
      mixArray(a.matrix[0], b.matrix[0]),
      mixArray(a.matrix[1], b.matrix[1]),
      mixArray(a.matrix[2], b.matrix[2]),
      mixArray(a.matrix[3], b.matrix[3]),
      mixArray(a.matrix[4], b.matrix[4]),
    ],
    tailScattering: mix(a.tailScattering, b.tailScattering),
    tailExtinction: mix(a.tailExtinction, b.tailExtinction),
  };
}

/** A per-particle normalisation of the sums: each is divided by the number. */
function perParticle(sums: NodeSums): NodeSums {
  const per = 1 / sums.number;
  const scale = (x: Float64Array): Float64Array => x.map((v) => v * per);
  return {
    number: 1,
    area: sums.area * per,
    volume: sums.volume * per,
    extinction: sums.extinction * per,
    scattering: sums.scattering * per,
    scatteringAsymmetry: sums.scatteringAsymmetry * per,
    a1: scale(sums.a1),
    a1Matrix: scale(sums.a1Matrix),
    matrix: [
      scale(sums.matrix[0]),
      scale(sums.matrix[1]),
      scale(sums.matrix[2]),
      scale(sums.matrix[3]),
      scale(sums.matrix[4]),
    ],
    tailScattering: sums.tailScattering * per,
    tailExtinction: sums.tailExtinction * per,
  };
}

/** A table's optics for a mode at one wavelength: the bulk values, the matrix, and the rules' inputs. */
export interface TableOptics {
  readonly wavelengthNm: number;
  /** Per particle, m². */
  readonly geometricCrossSectionM2: number;
  readonly extinctionCrossSectionM2: number;
  readonly scatteringCrossSectionM2: number;
  /** Per particle, m³. */
  readonly volumeM3: number;
  readonly asymmetry: number;
  /** The mode's matrix at the cosines asked, normalised as {@link ScatteringMatrix} is. */
  readonly matrix: ScatteringMatrix;
  /**
   * The mode's ω on the table as made, before the floor's or the ice's scaling, over its table's
   * particles (those below the table, by Mie, left out).
   */
  readonly tableAlbedo: number;
  /** Whether a node used lies on a kernel's floor. */
  readonly floored: boolean;
  /** The share of the mode's scattering carried by particles below the table's smallest size. */
  readonly tailShare: number;
}

/**
 * A mode's optics from a phase table at one wavelength: its sums over the two wavelength nodes
 * that bracket λ, linear in λ between them (the nearest node's beyond them), with the particles'
 * own k scaling the absorption on a floor's table and on Yang et al.'s table.
 *
 * @param index - The particles' own index at λ: the floor's scaling and the tail's Mie read it.
 * @param mu - The cosines to give the matrix at; the phase tables' by default.
 * @throws RangeError if the distribution reaches past the table's largest size.
 */
export function tableModeOptics(
  table: PhaseFileTable,
  distribution: RadiusDistribution,
  index: ComplexIndex,
  wavelengthNm: number,
  mu: Float64Array = PHASE_TABLE_MU,
): TableOptics {
  const nodes = table.nodes;
  const last = nodes.length - 1;
  let i = 0;
  while (i < last - 1 && (nodes[i + 1]?.wavelengthNm ?? Infinity) <= wavelengthNm) {
    i += 1;
  }
  const lower = nodes[i];
  const upper = nodes[Math.min(i + 1, last)];
  if (lower === undefined || upper === undefined) {
    throw new Error(`${table.material} has no nodes`);
  }
  const t =
    upper === lower
      ? 0
      : Math.min(
          Math.max(
            (wavelengthNm - lower.wavelengthNm) / (upper.wavelengthNm - lower.wavelengthNm),
            0,
          ),
          1,
        );
  const tailSphere = table.model === "yang2013" ? "volumeToArea" : "volume";
  const sumsOf = (node: PhaseTableNode): { readonly raw: NodeSums; readonly scaled: NodeSums } => {
    const raw = perParticle(
      sumsOverNode({ table, node, wavelengthNm, index, tailSphere }, distribution),
    );
    // A floor's table, and Yang et al.'s between its nodes, take the particles' own k: weak
    // absorption is linear in k at a fixed size parameter.
    const scales = (node.kTable > node.k || table.model === "yang2013") && node.kTable > 0;
    return { raw, scaled: scales ? scaledAbsorption(raw, index.k / node.kTable) : raw };
  };
  const a = sumsOf(lower);
  const b = t > 0 ? sumsOf(upper) : a;
  const sums = blend(a.scaled, b.scaled, t);
  const raw = blend(a.raw, b.raw, t);
  return {
    wavelengthNm,
    geometricCrossSectionM2: sums.area * 1e-12,
    extinctionCrossSectionM2: sums.extinction * 1e-12,
    scatteringCrossSectionM2: sums.scattering * 1e-12,
    volumeM3: sums.volume * 1e-18,
    asymmetry: sums.scatteringAsymmetry / sums.scattering,
    matrix: matrixAt(table, sums, mu),
    tableAlbedo: tableAlbedoOf(raw),
    floored: lower.kTable > lower.k || (t > 0 && upper.kTable > upper.k),
    tailShare: sums.tailScattering / sums.scattering,
  };
}

/** A phase's split into its resolved part's scale and an unresolved forward peak's share. */
export interface ForwardPeakSplit {
  /** The factor on the resolved part. */
  readonly scale: number;
  /** The forward peak's share of the scattering, ≥ 0. */
  readonly forwardPeak: number;
}

/**
 * The split of a phase whose resolved part integrates to `integral` with first moment `moment`
 * (∫ p dΩ and ∫ p cos θ dΩ, in units of the whole's ∫ p dΩ) into a scale on that part and a delta
 * of mean cosine `peakCosine` (1 for θ = 0), so that the whole integrates to 1 with mean cosine
 * `asymmetry`: s·I + f = 1 and s·G + f·μ_δ = g. Where that needs a negative peak, the resolved
 * part's own mean cosine being above g, the part is only normalised (s = 1 ÷ I, f = 0).
 */
export function forwardPeakSplit(
  integral: number,
  moment: number,
  asymmetry: number,
  peakCosine = 1,
): ForwardPeakSplit {
  const scale = (peakCosine - asymmetry) / (peakCosine * integral - moment);
  const forwardPeak = 1 - scale * integral;
  return scale > 0 && forwardPeak >= 0
    ? { scale, forwardPeak }
    : { scale: 1 / integral, forwardPeak: 0 };
}

/**
 * The mode's matrix at the cosines asked: a₁ log-linear in √θ between the phase angles, the other
 * elements as their ratios to a₁, linear in θ between the matrix angles, and the forward peak the
 * phase angles miss as a delta.
 *
 * @remarks
 * Each source normalises P₁₁ on its 498 angles, 0.01° apart near forward, but the files keep 48
 * (64) of them, the first beyond 0° at 0.08° (0.05°): a large particle's diffraction peak, about
 * 3.8 ÷ x_A wide, falls between, and the interpolant over a size node of x_D 11,810 holds only 0.59
 * of its scattering. The rest is put back as a forward delta, sized with the interpolant's scale so
 * that the whole integrates to 4π and its mean cosine is the source's g (Σ C_sca g, which the
 * tables carry), {@link forwardPeakSplit}; rescaling every angle instead would raise the side and
 * back angles by the deficit (R08.T5.c's science check). The scale stays within 0.5% of 1 on the
 * committed files, so those angles keep the source's values.
 */
function matrixAt(table: PhaseFileTable, sums: NodeSums, mu: Float64Array): ScatteringMatrix {
  const { phaseU } = anglesOf(table);
  const logA1 = sums.a1.map((v) => Math.log(v / sums.scattering));
  // ∫ a₁ (1, cos θ) dΩ = 2π ∫₀¹ a₁(u) (1, cos πu²) sin(πu²) 2πu du, by Gauss–Legendre on each
  // interval, in units of 4π.
  let integral = 0;
  let moment = 0;
  for (let j = 0; j + 1 < phaseU.length; j += 1) {
    const u0 = phaseU[j] ?? Number.NaN;
    const u1 = phaseU[j + 1] ?? Number.NaN;
    const half = 0.5 * (u1 - u0);
    const l0 = logA1[j] ?? Number.NaN;
    const l1 = logA1[j + 1] ?? Number.NaN;
    for (const [x, w] of SIZE_GL) {
      const t = 0.5 * (1 + x);
      const u = u0 + t * (u1 - u0);
      const angle = Math.PI * u * u;
      const term = half * w * Math.exp(l0 + t * (l1 - l0)) * Math.sin(angle) * u;
      integral += term;
      moment += term * Math.cos(angle);
    }
  }
  // 2π × 2π ÷ 4π.
  integral *= Math.PI;
  moment *= Math.PI;
  const split = forwardPeakSplit(integral, moment, sums.scatteringAsymmetry / sums.scattering);
  const norm = split.scale;
  const clamped = mu.map((c) => Math.acos(Math.min(Math.max(c, -1), 1)));
  const targetU = clamped.map((theta) => Math.sqrt(theta / Math.PI));
  const a1 = a1At(logA1, 0, phaseU, targetU).map((v) => v * norm);
  const angles = table.matrixAnglesDeg;
  const ratio = (e: 0 | 1 | 2 | 3 | 4, j: number): number =>
    (sums.matrix[e][j] ?? Number.NaN) / (sums.a1Matrix[j] ?? Number.NaN);
  const element = (e: 0 | 1 | 2 | 3 | 4): Float64Array =>
    clamped.map((theta, q) => {
      const deg = (theta * 180) / Math.PI;
      const j = lowerNode(angles, deg);
      const d0 = angles[j] ?? Number.NaN;
      const t = Math.min(Math.max((deg - d0) / ((angles[j + 1] ?? Number.NaN) - d0), 0), 1);
      const r0 = ratio(e, j);
      return (a1[q] ?? Number.NaN) * (r0 + t * (ratio(e, j + 1) - r0));
    });
  return {
    mu: mu.slice(),
    forwardPeak: split.forwardPeak,
    a1,
    a2: element(0),
    a3: element(1),
    a4: element(2),
    b1: element(3),
    b2: element(4),
  };
}

// ---------------------------------------------------------------------------------------------
// The rules.

/** What a non-spherical mode's particles are, for {@link opticsOfParticles}. */
export interface NonSphericalParticles {
  /** The material key, for the notes. */
  readonly material: string;
  readonly shape: "nonSphericalMineral" | "crystal";
  /** The particles' index at a wavelength, nm. */
  readonly index: (wavelengthNm: number) => ComplexIndex;
  readonly provenance: MaterialProvenance;
  /** Their own phase file, if one is on file. */
  readonly table: PhaseFileTable | undefined;
}

/** Water ice's phase file (Yang et al. 2013), the ices' model and analogue. */
export function waterIceTable(): PhaseFileTable | undefined {
  return PHASE_FILES.find((p) => p.model === "yang2013" && p.key === "H2O");
}

/** Whether a mode's material is an ice by {@link ICE_KEYS}. */
export function isIce(material: string): boolean {
  return ICE_KEYS.has(material);
}

function fromTable(
  optics: TableOptics,
  particles: NonSphericalParticles,
  basis: PhaseBasis,
  model: PhaseModel,
  note: string | undefined,
): ModeOptics {
  const geometric = optics.geometricCrossSectionM2;
  return {
    wavelengthNm: optics.wavelengthNm,
    qExt: optics.extinctionCrossSectionM2 / geometric,
    qSca: optics.scatteringCrossSectionM2 / geometric,
    singleScatteringAlbedo: optics.scatteringCrossSectionM2 / optics.extinctionCrossSectionM2,
    asymmetry: optics.asymmetry,
    geometricCrossSectionM2: geometric,
    extinctionCrossSectionM2: optics.extinctionCrossSectionM2,
    scatteringCrossSectionM2: optics.scatteringCrossSectionM2,
    volumeM3: optics.volumeM3,
    matrix: optics.matrix,
    shape: particles.shape,
    provenance: particles.provenance,
    phaseBasis: basis,
    phaseModel: model,
    phaseNote: note,
  };
}

/**
 * The fallback for a mineral no table or kernel covers: Mie's cross-sections and ω for the
 * volume-to-area spheres of the mode, with Henyey–Greenstein from Mie's g and no matrix, or, below
 * {@link SMALL_GRAIN_SIZE_PARAMETER}, the sphere's own Mie matrix.
 */
function fallbackOptics(
  particles: NonSphericalParticles,
  sizes: SizeDistribution,
  wavelengthNm: number,
  mu: Float64Array,
  reason: string,
): ModeOptics {
  const effective = (2 * Math.PI * sizes.effectiveRadiusUm * 1000) / wavelengthNm;
  const small = effective < SMALL_GRAIN_SIZE_PARAMETER;
  // The amplitudes are summed only where the sphere's matrix is kept.
  const optics = sphereModeOptics(
    sizes,
    particles.index(wavelengthNm),
    wavelengthNm,
    small ? mu : NO_ANGLES,
  );
  return {
    wavelengthNm,
    qExt: optics.qExt,
    qSca: optics.qSca,
    singleScatteringAlbedo: optics.singleScatteringAlbedo,
    asymmetry: optics.asymmetry,
    geometricCrossSectionM2: optics.geometricCrossSectionM2,
    extinctionCrossSectionM2: optics.extinctionCrossSectionM2,
    scatteringCrossSectionM2: optics.scatteringCrossSectionM2,
    volumeM3: optics.volumeM3,
    matrix: small ? optics.matrix : undefined,
    shape: particles.shape,
    provenance: particles.provenance,
    phaseBasis: "fallback",
    phaseModel: small ? "mie" : "henyeyGreenstein",
    phaseNote: `${particles.material}: ${reason}; ${
      small
        ? `a small grain (2πr_eff ÷ λ ${effective.toFixed(2)} < ${SMALL_GRAIN_SIZE_PARAMETER}), drawn with the sphere's own Mie matrix`
        : "a Henyey–Greenstein phase from Mie's g, with Mie's ω and no matrix"
    }, a fallback`,
  };
}

/**
 * A non-spherical mode's optics at one wavelength from its particles, by the module's rules: the
 * table's model, the floor rule, the ices' analogue, or the fallback.
 *
 * @param wavelengthNm - The vacuum wavelength, nm, in 380–780.
 * @param mu - The cosines to give the matrix at; the phase tables' by default.
 * @throws RangeError as {@link tableModeOptics} and `sphereModeOptics` do.
 */
export function opticsOfParticles(
  particles: NonSphericalParticles,
  sizes: SizeDistribution,
  wavelengthNm: number,
  mu: Float64Array = PHASE_TABLE_MU,
): ModeOptics {
  const index = particles.index(wavelengthNm);
  const distribution = radiusDistribution(sizes);
  const water = waterIceTable();
  if (particles.shape === "crystal" && isIce(particles.material)) {
    if (water === undefined) {
      return fallbackOptics(particles, sizes, wavelengthNm, mu, "water ice's table is not on file");
    }
    const own = particles.table === water;
    const optics = tableModeOptics(water, distribution, index, wavelengthNm, mu);
    // The tail's share is judged at 550 nm (science-r08-nonspherical.md §2.3).
    const share =
      wavelengthNm === 550
        ? optics.tailShare
        : tableModeOptics(water, distribution, particles.index(550), 550, NO_ANGLES).tailShare;
    const tail = share > SMALL_CRYSTAL_SHARE_MAX;
    const tailNote = tail
      ? `crystals below the table's 2 µm carry ${(100 * share).toFixed(0)}% of the scattering at 550 nm, drawn as volume-to-area spheres`
      : undefined;
    if (own) {
      return fromTable(optics, particles, tail ? "analogue" : "model", "yang2013", tailNote);
    }
    // Another ice: water ice's phase and matrix at its own size parameter, Mie's C_ext and ω.
    const mie = sphereModeOptics(sizes, index, wavelengthNm, NO_ANGLES);
    const bias = ICE_ANALOGUE_ASYMMETRY_BIAS[particles.material];
    return {
      ...fromTable(optics, particles, "analogue", "yang2013", undefined),
      qExt: mie.qExt,
      qSca: mie.qSca,
      singleScatteringAlbedo: mie.singleScatteringAlbedo,
      geometricCrossSectionM2: mie.geometricCrossSectionM2,
      extinctionCrossSectionM2: mie.extinctionCrossSectionM2,
      scatteringCrossSectionM2: mie.scatteringCrossSectionM2,
      volumeM3: mie.volumeM3,
      phaseNote: `${particles.material} ice drawn on water ice's roughened 8-column table, a named analogue${
        bias === undefined
          ? ""
          : `; its g expected ${bias.toFixed(3)} from water's (van Diedenhoven et al. 2014)`
      }${tailNote === undefined ? "" : `; ${tailNote}`}`,
    };
  }
  const table = particles.table;
  if (table === undefined || table.model !== "tamudust2020") {
    const outside = index.n > KERNEL_TRANSPARENT_N_MAX && index.k < 1e-3;
    return fallbackOptics(
      particles,
      sizes,
      wavelengthNm,
      mu,
      outside
        ? `n ${index.n.toFixed(3)} above ${KERNEL_TRANSPARENT_N_MAX} with k under 10⁻³, below the longwave kernel's k floor, with no floor table on file (R08.T5.d)`
        : "no reduced TAMUdust2020 table on file (R08.T5.d)",
    );
  }
  const optics = tableModeOptics(table, distribution, index, wavelengthNm, mu);
  if (optics.floored && optics.tableAlbedo < FLOOR_ALBEDO_MIN) {
    if (index.n > KERNEL_TRANSPARENT_N_MAX) {
      return fallbackOptics(
        particles,
        sizes,
        wavelengthNm,
        mu,
        `the k-floor rule exceeded (the floor's ω ${optics.tableAlbedo.toFixed(4)} < ${FLOOR_ALBEDO_MIN}) at n ${index.n.toFixed(3)} above ${KERNEL_TRANSPARENT_N_MAX}`,
      );
    }
    return fromTable(
      optics,
      particles,
      "analogue",
      "tamudust2020",
      `${particles.material}: the k-floor rule exceeded (the floor's ω ${optics.tableAlbedo.toFixed(4)} < ${FLOOR_ALBEDO_MIN}), the floor's table stands in, a named analogue`,
    );
  }
  return fromTable(optics, particles, "model", "tamudust2020", undefined);
}

/** The particles a non-spherical mode's material resolves to. */
export function particlesOf(
  mode: AerosolMode,
  material: ResolvedMaterial,
  shape: "nonSphericalMineral" | "crystal",
): NonSphericalParticles {
  return {
    material: mode.material,
    shape,
    index: (wavelengthNm) => indexAt(material, wavelengthNm),
    provenance: material.provenance,
    table: material.file === undefined ? undefined : phaseFileOf(material.file),
  };
}

/**
 * A non-spherical mode's optics at one wavelength (R08.T5.c): its material resolved through the
 * registry, then {@link opticsOfParticles}. A mineral or crystal is a `nonSphericalMineral` or a
 * `crystal` by the mode's shape, or its material file's.
 *
 * @param wavelengthNm - The vacuum wavelength, nm, in 380–780.
 * @param mu - The cosines to give the matrix at; the phase tables' by default.
 * @throws RangeError for a mode whose shape is a sphere or an aggregate, a wavelength outside the
 *   files' range, or as {@link opticsOfParticles}.
 */
export function nonSphericalModeOptics(
  mode: AerosolMode,
  wavelengthNm: number,
  mu: Float64Array = PHASE_TABLE_MU,
): ModeOptics {
  const shape = modeShape(mode, resolveModeMaterial(mode));
  if (shape !== "nonSphericalMineral" && shape !== "crystal") {
    throw new RangeError(`a ${shape} mode of ${mode.material} is not non-spherical`);
  }
  // Resolved again with its shape, so that a mode with no phase is drawn as its solid.
  const material = resolveModeMaterial({ ...mode, shape });
  return opticsOfParticles(particlesOf(mode, material, shape), mode.sizes, wavelengthNm, mu);
}
