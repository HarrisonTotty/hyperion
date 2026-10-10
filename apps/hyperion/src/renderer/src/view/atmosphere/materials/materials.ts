/**
 * The aerosol materials' refractive indices, a registry keyed by the sim's substance keys (plan R08,
 * R08.T5.b; decision-composition §1.9): one file per material and phase, each with its paper, its
 * source, its licence basis and its provenance.
 *
 * @remarks
 * The files beside this module are written by `apps/hyperion/src/tools/materials.ts` from sources
 * fetched by URL and SHA-256 and never committed (decision-r08-licences.md): every file gives n and
 * k at 380–780 nm every 5 nm, the wavelengths of every bake bin and render channel (Design note 5).
 * They are also galaxy plan 14's inputs (P14.T49.e computes the sim's mass extinction from them),
 * so a changed file moves generated output.
 *
 * A key is the wire's string and is never narrowed: an unknown key (a newer server's) takes the
 * generic stand-in, a non-absorbing sphere of real index 1.5, a stated convention. A material that
 * has no file for the phase or variant asked takes its first file as a named analogue. Both, and
 * every file whose provenance is `standIn` (NH₄SH, whose visible constants are not published), are
 * stand-ins: a body whose medium shows such a mode with less than
 * {@link CLOUD_DECK_SPLIT_OPTICAL_DEPTH} above it at 550 nm reads `ATMOSPHERE: APPROXIMATE`
 * (Design note 12), which clears only when measured constants replace the stand-in.
 */

import type { AtmosphereLabel } from "../labels";
import type { ComplexIndex } from "../mie";
import { CLOUD_DECK_SPLIT_OPTICAL_DEPTH } from "../thick/regime";
import ammoniaIce from "./ammonia-ice.json" with { type: "json" };
import ammoniumHydrosulphide from "./ammonium-hydrosulphide.json" with { type: "json" };
import carbonDioxideIce from "./carbon-dioxide-ice.json" with { type: "json" };
import enstatiteGlass from "./enstatite-glass.json" with { type: "json" };
import forsteriteAmorphous from "./forsterite-amorphous.json" with { type: "json" };
import iron from "./iron.json" with { type: "json" };
import marsDust from "./mars-dust.json" with { type: "json" };
import methaneIce from "./methane-ice.json" with { type: "json" };
import methaneLiquid from "./methane-liquid.json" with { type: "json" };
import soot from "./soot.json" with { type: "json" };
import sulphuricAcid75 from "./sulphuric-acid-75.json" with { type: "json" };
import sulphuricAcid84 from "./sulphuric-acid-84.json" with { type: "json" };
import tholin from "./tholin.json" with { type: "json" };
import waterIce from "./water-ice.json" with { type: "json" };
import water from "./water.json" with { type: "json" };

/** The phase a material file describes. */
export type MaterialPhase = "liquid" | "solid";

/**
 * The shape class of a material's particles (Design note 6; P14.T24.c's closed physics enum):
 * spheres go through Mie; a non-spherical mineral takes TAMUdust2020's hexahedra and a crystal Yang
 * et al. 2013's roughened ice, or an analogue or fallback (R08.T5.c, `nonSpherical.ts`); an
 * aggregate's monomers are spheres, through the MMF (`aggregate.ts`). A liquid is a sphere
 * whatever its material's file says (`aerosol.ts`).
 */
export type ShapeClass = "sphere" | "nonSphericalMineral" | "crystal" | "aggregate";

/**
 * How a file's values were obtained (decision-composition §1.9): `measured` from a laboratory or
 * an observational retrieval, `derived` from a published table by a fit made here, `standIn` a
 * stated stand-in where nothing measured exists.
 */
export type MaterialProvenance = "measured" | "derived" | "standIn";

/** A derived file's parameterisation: n = a + b ÷ λ² (λ in µm) and k = αλ ÷ 4π. */
export interface CauchyFit {
  readonly form: "cauchy";
  readonly a: number;
  readonly bUm2: number;
  /** α, m⁻¹. */
  readonly absorptionPerM: number;
  /** The range of the data the fit was made on, µm. */
  readonly fittedFromUm: number;
  readonly fittedToUm: number;
}

/** One material file: its header and its index on {@link MATERIAL_WAVELENGTHS_NM}. */
export interface MaterialIndexFile {
  /** The sim's substance key (P14.T49.b). */
  readonly key: string;
  readonly phase: MaterialPhase;
  /** A composition variant (an acid's concentration), or undefined for the key's only one. */
  readonly variant: string | undefined;
  /** What the sample is. */
  readonly name: string;
  /** The sample's temperature, K, where the source states one. */
  readonly temperatureK: number | undefined;
  readonly shape: ShapeClass;
  readonly provenance: MaterialProvenance;
  readonly paper: string;
  /** Where the values were read, with the URL, the checksum and the date fetched. */
  readonly source: string;
  /** The licence basis (decision-composition §5). */
  readonly licence: string;
  /** How the values were brought onto the grid, with every exception. */
  readonly reduction: string;
  /** For a stand-in file, what stands in and why. */
  readonly standIn: string | undefined;
  /** For a derived file, its parameterisation. */
  readonly fit: CauchyFit | undefined;
  /** The real index at each of {@link MATERIAL_WAVELENGTHS_NM}. */
  readonly n: Float64Array;
  /** The imaginary index, ≥ 0, at each of {@link MATERIAL_WAVELENGTHS_NM}. */
  readonly k: Float64Array;
}

/** The wavelengths every material file gives its index at, nm: 380 to 780 every 5. */
export const MATERIAL_WAVELENGTHS_NM: Float64Array = Float64Array.from(
  { length: 81 },
  (_, i) => 380 + 5 * i,
);

/**
 * The generic stand-in's index, for a key with no file (a newer server's material): a
 * non-absorbing sphere of real index 1.5, a stated convention, not physics (decision-composition
 * §1.9).
 */
export const GENERIC_STAND_IN_INDEX: ComplexIndex = { n: 1.5, k: 0 };

const PHASES: ReadonlyArray<MaterialPhase> = ["liquid", "solid"];
const SHAPES: ReadonlyArray<ShapeClass> = ["sphere", "nonSphericalMineral", "crystal", "aggregate"];
const PROVENANCES: ReadonlyArray<MaterialProvenance> = ["measured", "derived", "standIn"];

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function member<T extends string>(values: ReadonlyArray<T>, value: unknown): value is T {
  return values.some((v) => v === value);
}

function text(raw: Readonly<Record<string, unknown>>, field: string, file: string): string {
  const value = raw[field];
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${file}: "${field}" must be a non-empty string`);
  }
  return value;
}

function optionalText(
  raw: Readonly<Record<string, unknown>>,
  field: string,
  file: string,
): string | undefined {
  return raw[field] === null ? undefined : text(raw, field, file);
}

function numbers(
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

function fitOf(value: unknown, file: string): CauchyFit | undefined {
  if (value === null) {
    return undefined;
  }
  if (!isRecord(value) || value["form"] !== "cauchy") {
    throw new Error(`${file}: "fit" must be null or a Cauchy fit`);
  }
  const field = (name: string): number => {
    const v = value[name];
    if (typeof v !== "number" || !Number.isFinite(v)) {
      throw new Error(`${file}: the fit's "${name}" must be a finite number`);
    }
    return v;
  };
  return {
    form: "cauchy",
    a: field("a"),
    bUm2: field("bUm2"),
    absorptionPerM: field("absorptionPerM"),
    fittedFromUm: field("fittedFromUm"),
    fittedToUm: field("fittedToUm"),
  };
}

/**
 * A material file checked and typed.
 *
 * @throws Error naming the file and the field when the file breaks the format (a bad edit or a bad
 *   reduction; the files are committed, never fetched).
 */
export function parseMaterialFile(raw: unknown, file: string): MaterialIndexFile {
  if (!isRecord(raw)) {
    throw new Error(`${file}: not a JSON object`);
  }
  const { phase, shape, provenance, temperatureK } = raw;
  if (!member(PHASES, phase) || !member(SHAPES, shape) || !member(PROVENANCES, provenance)) {
    throw new Error(`${file}: bad phase, shape or provenance`);
  }
  if (temperatureK !== null && !(typeof temperatureK === "number" && temperatureK > 0)) {
    throw new Error(`${file}: "temperatureK" must be null or a positive number`);
  }
  const grid = numbers(raw, "wavelengthsNm", file);
  if (
    grid.length !== MATERIAL_WAVELENGTHS_NM.length ||
    grid.some((nm, i) => nm !== MATERIAL_WAVELENGTHS_NM[i])
  ) {
    throw new Error(`${file}: "wavelengthsNm" must be 380–780 nm every 5 nm`);
  }
  const n = numbers(raw, "n", file);
  const k = numbers(raw, "k", file);
  if (n.length !== grid.length || k.length !== grid.length) {
    throw new Error(`${file}: n and k must have one value per wavelength`);
  }
  if (n.some((v) => v <= 0) || k.some((v) => v < 0)) {
    throw new Error(`${file}: n must be > 0 and k ≥ 0`);
  }
  const standIn = optionalText(raw, "standIn", file);
  const fit = fitOf(raw["fit"], file);
  if ((provenance === "standIn") !== (standIn !== undefined)) {
    throw new Error(`${file}: a stand-in file, and only one, states its stand-in`);
  }
  if ((provenance === "derived") !== (fit !== undefined)) {
    throw new Error(`${file}: a derived file, and only one, carries its fit`);
  }
  return {
    key: text(raw, "key", file),
    phase,
    variant: optionalText(raw, "variant", file),
    name: text(raw, "name", file),
    temperatureK: temperatureK === null ? undefined : temperatureK,
    shape,
    provenance,
    paper: text(raw, "paper", file),
    source: text(raw, "source", file),
    licence: text(raw, "licence", file),
    reduction: text(raw, "reduction", file),
    standIn,
    fit,
    n,
    k,
  };
}

/**
 * Every material file, in the registry's order: a key's first file is its default, taken when a
 * mode names no phase or variant (liquid water before ice, liquid methane before its ice, 75 wt%
 * sulphuric acid before 84.5 wt%, a choice: both lie inside Hansen and Hovenier 1974's 1.44 ±
 * 0.015 at 550 nm, who name only "a concentrated solution of sulfuric acid").
 */
export const MATERIAL_FILES: ReadonlyArray<MaterialIndexFile> = [
  parseMaterialFile(water, "water.json"),
  parseMaterialFile(waterIce, "water-ice.json"),
  parseMaterialFile(ammoniaIce, "ammonia-ice.json"),
  parseMaterialFile(methaneLiquid, "methane-liquid.json"),
  parseMaterialFile(methaneIce, "methane-ice.json"),
  parseMaterialFile(carbonDioxideIce, "carbon-dioxide-ice.json"),
  parseMaterialFile(sulphuricAcid75, "sulphuric-acid-75.json"),
  parseMaterialFile(sulphuricAcid84, "sulphuric-acid-84.json"),
  parseMaterialFile(ammoniumHydrosulphide, "ammonium-hydrosulphide.json"),
  parseMaterialFile(iron, "iron.json"),
  parseMaterialFile(soot, "soot.json"),
  parseMaterialFile(tholin, "tholin.json"),
  parseMaterialFile(marsDust, "mars-dust.json"),
  parseMaterialFile(enstatiteGlass, "enstatite-glass.json"),
  parseMaterialFile(forsteriteAmorphous, "forsterite-amorphous.json"),
];

const FILES_BY_KEY: ReadonlyMap<string, ReadonlyArray<MaterialIndexFile>> = (() => {
  const byKey = new Map<string, MaterialIndexFile[]>();
  for (const file of MATERIAL_FILES) {
    const list = byKey.get(file.key) ?? [];
    list.push(file);
    byKey.set(file.key, list);
  }
  return byKey;
})();

/** The phase or variant a mode is in, where its material has several files. */
export interface MaterialForm {
  readonly phase?: MaterialPhase;
  /** A file's `variant`, such as `75wt%`. */
  readonly variant?: string;
}

/** A material key resolved to the file it is drawn with, and how far that file is its own. */
export interface ResolvedMaterial {
  /** The key asked for. */
  readonly material: string;
  /** The file drawn, or undefined for the generic stand-in ({@link GENERIC_STAND_IN_INDEX}). */
  readonly file: MaterialIndexFile | undefined;
  /** The file's provenance, or `standIn` when the file is not the material's own for its form. */
  readonly provenance: MaterialProvenance;
  /** For a stand-in, what stands in and why. */
  readonly standIn: string | undefined;
}

/**
 * The file a material key and form are drawn with.
 *
 * @remarks
 * A key with no file takes the generic stand-in. A form the key's files lack (liquid iron, solid
 * sulphuric acid, an acid concentration not on file) takes the key's first file of the phase, or
 * its first file, as a named analogue. Each of those, and a file whose own provenance is
 * `standIn`, resolves as `standIn`.
 */
export function resolveMaterial(material: string, form?: MaterialForm): ResolvedMaterial {
  const files = FILES_BY_KEY.get(material);
  const first = files?.[0];
  if (files === undefined || first === undefined) {
    return {
      material,
      file: undefined,
      provenance: "standIn",
      standIn: `no index file for "${material}": the generic stand-in, a non-absorbing sphere of real index ${GENERIC_STAND_IN_INDEX.n}, a stated convention`,
    };
  }
  const phase = form?.phase;
  const ofPhase = phase === undefined ? files : files.filter((f) => f.phase === phase);
  const phaseFirst = ofPhase[0];
  if (phaseFirst === undefined) {
    return {
      material,
      file: first,
      provenance: "standIn",
      standIn: `no ${phase ?? ""} ${material} on file: ${first.name} stands in, a named analogue`,
    };
  }
  const variant = form?.variant;
  const file = variant === undefined ? phaseFirst : ofPhase.find((f) => f.variant === variant);
  if (file === undefined) {
    return {
      material,
      file: phaseFirst,
      provenance: "standIn",
      standIn: `no ${variant ?? ""} ${material} on file: ${phaseFirst.name} stands in, a named analogue`,
    };
  }
  return { material, file, provenance: file.provenance, standIn: file.standIn };
}

/**
 * A resolved material's index at a wavelength: n linear and k linear in log k between the file's
 * wavelengths, as the files were reduced.
 *
 * @param wavelengthNm - The vacuum wavelength, nm, in 380–780.
 * @throws RangeError for a wavelength outside the files' range.
 */
export function indexAt(resolved: ResolvedMaterial, wavelengthNm: number): ComplexIndex {
  const first = MATERIAL_WAVELENGTHS_NM[0] ?? Number.NaN;
  const last = MATERIAL_WAVELENGTHS_NM.at(-1) ?? Number.NaN;
  if (!(wavelengthNm >= first && wavelengthNm <= last)) {
    throw new RangeError(`wavelength ${wavelengthNm} nm is outside the files' ${first}–${last} nm`);
  }
  const { file } = resolved;
  if (file === undefined) {
    return GENERIC_STAND_IN_INDEX;
  }
  const step = (last - first) / (MATERIAL_WAVELENGTHS_NM.length - 1);
  const i = Math.min(Math.floor((wavelengthNm - first) / step), MATERIAL_WAVELENGTHS_NM.length - 2);
  const t = (wavelengthNm - first - i * step) / step;
  const n0 = file.n[i] ?? Number.NaN;
  const n1 = file.n[i + 1] ?? Number.NaN;
  const k0 = file.k[i] ?? Number.NaN;
  const k1 = file.k[i + 1] ?? Number.NaN;
  return {
    n: n0 + t * (n1 - n0),
    k: k0 > 0 && k1 > 0 ? k0 * (k1 / k0) ** t : k0 + t * (k1 - k0),
  };
}

/** A material's index at a wavelength and where it came from. */
export interface MaterialIndex {
  readonly index: ComplexIndex;
  readonly provenance: MaterialProvenance;
}

/**
 * A material's refractive index at a wavelength, by its registry key.
 *
 * @param material - The sim's material key; an unknown key takes the generic stand-in.
 * @param wavelengthNm - The vacuum wavelength, nm, in 380–780.
 * @param form - The mode's phase or variant, where the material has several files.
 * @throws RangeError for a wavelength outside the files' range.
 */
export function refractiveIndex(
  material: string,
  wavelengthNm: number,
  form?: MaterialForm,
): MaterialIndex {
  const resolved = resolveMaterial(material, form);
  return { index: indexAt(resolved, wavelengthNm), provenance: resolved.provenance };
}

/** A mode as the stand-in label sees it: its material, and the optical depth above its top. */
export interface MaterialLayer {
  readonly material: string;
  readonly form?: MaterialForm;
  /** The vertical optical depth at 550 nm of everything above the mode's top, ≥ 0. */
  readonly opticalDepthAbove550: number;
}

/** A material drawn with a stand-in where it can be seen, and why. */
export interface MaterialApproximation {
  readonly material: string;
  readonly reason: string;
}

/**
 * The stand-in materials of a body's modes that lie under less than
 * {@link CLOUD_DECK_SPLIT_OPTICAL_DEPTH} at 550 nm: the reasons R08.T10.a's `atmosphereApproximate`
 * collects (Design note 12). A deeper mode does not reach the picture, so it is not reported.
 *
 * @throws RangeError for an optical depth that is not finite and ≥ 0.
 */
export function materialApproximations(
  layers: Iterable<MaterialLayer>,
): ReadonlyArray<MaterialApproximation> {
  const out: MaterialApproximation[] = [];
  for (const layer of layers) {
    const depth = layer.opticalDepthAbove550;
    if (!(depth >= 0 && Number.isFinite(depth))) {
      throw new RangeError(`optical depth ${depth} above a mode must be finite and ≥ 0`);
    }
    const resolved = resolveMaterial(layer.material, layer.form);
    if (resolved.provenance === "standIn" && depth < CLOUD_DECK_SPLIT_OPTICAL_DEPTH) {
      out.push({ material: layer.material, reason: resolved.standIn ?? "a stand-in" });
    }
  }
  return out;
}

/**
 * The labels a body's modes give by their materials: `atmosphereApproximate` for a visible stand-in.
 *
 * @throws RangeError for an optical depth that is not finite and ≥ 0.
 */
export function materialLabels(layers: Iterable<MaterialLayer>): ReadonlyArray<AtmosphereLabel> {
  return materialApproximations(layers).length > 0 ? ["atmosphereApproximate"] : [];
}
