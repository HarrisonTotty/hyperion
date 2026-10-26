/**
 * The aerosol materials' refractive indices, a registry keyed by the sim's substance keys (plan R08,
 * R08.T5.b; decision-composition §1.9): one file per material and phase, each with its paper, its
 * source, its licence basis and its provenance.
 *
 * @remarks
 * The files beside this module are written by `apps/hyperion/src/tools/materials.ts` from sources
 * fetched by URL and SHA-256 and never committed (decision-r08-licences.md): every file gives n and
 * k every 5 nm on its own grid, which ends at 780 nm and starts at or below 380 nm, so that it
 * covers {@link MATERIAL_WAVELENGTHS_NM}, the wavelengths of every bake bin and render channel
 * (Design note 5). Two start lower, at 360 nm, for Venus's ultraviolet fit (R08.T5.d,
 * decision-r08-t5d ruling 1): `acid_uv_absorber` and the 75 wt% sulphuric acid. The files are also
 * galaxy plan 14's inputs (P14.T49.e computes the sim's mass extinction from them), so a changed
 * file moves generated output, and a reader finds a wavelength in a file by the file's own grid.
 *
 * A key is the wire's string and is never narrowed: an unknown key (a newer server's) takes the
 * generic stand-in, a non-absorbing sphere of real index 1.5, a stated convention. A material that
 * has no file for the phase or variant asked takes its first file as a named analogue. Both, and
 * every file whose provenance is `standIn` (NH₄SH, whose visible constants are not published), are
 * stand-ins: a body whose medium shows such a mode with less than
 * {@link CLOUD_DECK_SPLIT_OPTICAL_DEPTH} above it at 550 nm reads `ATMOSPHERE: APPROXIMATE`
 * (Design note 12), which clears only when measured constants replace the stand-in.
 */

import { substanceName } from "../../../lib/substances";
import type { AtmosphereLabel } from "../labels";
import type { ComplexIndex } from "../mie";
import { CLOUD_DECK_SPLIT_OPTICAL_DEPTH } from "../thick/regime";
import aluminiumOxide from "./aluminium-oxide.json" with { type: "json" };
import ammoniaIce from "./ammonia-ice.json" with { type: "json" };
import ammoniaLiquid from "./ammonia-liquid.json" with { type: "json" };
import ammoniumHydrosulphide from "./ammonium-hydrosulphide.json" with { type: "json" };
import argonIce from "./argon-ice.json" with { type: "json" };
import argonLiquid from "./argon-liquid.json" with { type: "json" };
import basaltFile from "./basalt.json" with { type: "json" };
import basalticGlass from "./basaltic-glass.json" with { type: "json" };
import calciumTitanate from "./calcium-titanate.json" with { type: "json" };
import carbonDioxideIce from "./carbon-dioxide-ice.json" with { type: "json" };
import carbonDioxideLiquid from "./carbon-dioxide-liquid.json" with { type: "json" };
import carbonMonoxideIce from "./carbon-monoxide-ice.json" with { type: "json" };
import carbonMonoxideLiquid from "./carbon-monoxide-liquid.json" with { type: "json" };
import chromium from "./chromium.json" with { type: "json" };
import enstatiteGlass from "./enstatite-glass.json" with { type: "json" };
import ethaneLiquid from "./ethane-liquid.json" with { type: "json" };
import forsteriteAmorphous from "./forsterite-amorphous.json" with { type: "json" };
import graphiteFile from "./graphite.json" with { type: "json" };
import hydrogenSulphideIce from "./hydrogen-sulphide-ice.json" with { type: "json" };
import hydrogenSulphideLiquid from "./hydrogen-sulphide-liquid.json" with { type: "json" };
import iceGiantHaze from "./ice-giant-haze.json" with { type: "json" };
import ironSulphide from "./iron-sulphide.json" with { type: "json" };
import iron from "./iron.json" with { type: "json" };
import jovianChromophore from "./jovian-chromophore.json" with { type: "json" };
import kryptonLiquid from "./krypton-liquid.json" with { type: "json" };
import manganeseSulphide from "./manganese-sulphide.json" with { type: "json" };
import marsDust from "./mars-dust.json" with { type: "json" };
import marsRegolithSimulant from "./mars-regolith-simulant.json" with { type: "json" };
import methaneIce from "./methane-ice.json" with { type: "json" };
import methaneLiquid from "./methane-liquid.json" with { type: "json" };
import nitrogenIce from "./nitrogen-ice.json" with { type: "json" };
import nitrogenLiquid from "./nitrogen-liquid.json" with { type: "json" };
import nitrousOxideLiquid from "./nitrous-oxide-liquid.json" with { type: "json" };
import oxygenIce from "./oxygen-ice.json" with { type: "json" };
import oxygenLiquid from "./oxygen-liquid.json" with { type: "json" };
import potassiumChloride from "./potassium-chloride.json" with { type: "json" };
import seawater from "./seawater.json" with { type: "json" };
import siliconDioxide from "./silicon-dioxide.json" with { type: "json" };
import sodiumChloride from "./sodium-chloride.json" with { type: "json" };
import sodiumSulphide from "./sodium-sulphide.json" with { type: "json" };
import soot from "./soot.json" with { type: "json" };
import sulphurDioxideLiquid from "./sulphur-dioxide-liquid.json" with { type: "json" };
import sulphurLiquid from "./sulphur-liquid.json" with { type: "json" };
import sulphur from "./sulphur.json" with { type: "json" };
import sulphuricAcid75 from "./sulphuric-acid-75.json" with { type: "json" };
import sulphuricAcid84 from "./sulphuric-acid-84.json" with { type: "json" };
import sulphuricAcidUvAbsorber from "./sulphuric-acid-uv-absorber.json" with { type: "json" };
import tholin from "./tholin.json" with { type: "json" };
import titaniumDioxide from "./titanium-dioxide.json" with { type: "json" };
import waterIce from "./water-ice.json" with { type: "json" };
import water from "./water.json" with { type: "json" };
import xenonLiquid from "./xenon-liquid.json" with { type: "json" };
import zincSulphide from "./zinc-sulphide.json" with { type: "json" };

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

/**
 * A derived file's real index from its gas's molar refraction scaled to the liquid (R08.T5.k;
 * decision-p14-t49h-gas-index A.1): at each wavelength x = c R(λ) ρ_m and n = √((1 + 2x) ÷ (1 − x)),
 * R the Lorentz–Lorenz molar refraction of the gas row of the file's key in `../rayleigh.ts`
 * (`GAS_DISPERSION`, or for SO₂ the dispersion of its `ESTIMATED_RAYLEIGH` row) at the row's own
 * reference state and compressibility.
 *
 * @remarks
 * The form describes n alone: the file's k is its arrays, its derivation stated in `reduction`.
 */
export interface ScaledMolarRefractionFit {
  readonly form: "scaledMolarRefraction";
  /** ρ_m, the saturated liquid's molar density at the file's temperature, mol/l. */
  readonly molarDensityMolPerL: number;
  /** c, the one factor on the gas's molar refraction. */
  readonly factor: number;
  /** The shortest wavelength of the measurements that set c, µm. */
  readonly anchoredFromUm: number;
  /** The longest wavelength of the measurements that set c, µm. */
  readonly anchoredToUm: number;
}

/**
 * A derived file's absorption from its gas's coefficients scaled to the liquid (R08.T5.j;
 * decision-p14-t49h-liquids §2.4): over `fromUm`–`toUm`, a = κN and k = aλ ÷ 4π, κ the gas's
 * absorption coefficient per km-amagat in the file's `paper` and N the liquid's number density in
 * amagat at `molarDensityMolPerL`.
 *
 * @remarks
 * The form describes k alone over its span: the file's n, and its k elsewhere, are its arrays, their
 * derivations stated in `reduction`.
 */
export interface ScaledGasAbsorptionFit {
  readonly form: "scaledGasAbsorption";
  /** ρ_m, the liquid's molar density at the file's state, mol/l. */
  readonly molarDensityMolPerL: number;
  /** The span's shortest wavelength, µm. */
  readonly fromUm: number;
  /** The span's longest wavelength, µm. */
  readonly toUm: number;
}

/** A derived file's parameterisation, by its `form`. */
export type DerivedFit = CauchyFit | ScaledMolarRefractionFit | ScaledGasAbsorptionFit;

/** One material file: its header and its index on its own grid. */
export interface MaterialIndexFile {
  /** The sim's substance key (P14.T49.b). */
  readonly key: string;
  readonly phase: MaterialPhase;
  /**
   * A composition variant (an acid's concentration, a sea's salinity; {@link MaterialForm.variant}),
   * or undefined for the key's only one.
   */
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
  /**
   * For a file whose colour rests on no measurement of the material though its index does, a
   * statement of what its absorption is (the file's optional `absorptionStandIn`; `signoff-12.md`,
   * item 38(c)): liquid O₂'s, its gas's O₂–O₂ pair absorption carried to the liquid. Player text,
   * which the stand-in label's reasons read under the same optical-depth rule
   * ({@link materialApproximations}), and which leaves the file's `provenance` as it is: the fit
   * (`hyperion-fit`'s `materials.rs`) reads none of it.
   */
  readonly absorptionStandIn: string | undefined;
  /** For a derived file, its parameterisation. */
  readonly fit: DerivedFit | undefined;
  /**
   * The file's own grid, nm: λ₀, λ₀ + 5, …, 780, with λ₀ a whole multiple of 5 nm at or below
   * 380 nm, so that it covers {@link MATERIAL_WAVELENGTHS_NM} (decision-r08-t5d ruling 1).
   */
  readonly wavelengthsNm: Float64Array;
  /** The real index at each of {@link MaterialIndexFile.wavelengthsNm}. */
  readonly n: Float64Array;
  /** The imaginary index, ≥ 0, at each of {@link MaterialIndexFile.wavelengthsNm}. */
  readonly k: Float64Array;
}

/**
 * The span every material file covers, and the wavelengths the renderer draws at, nm: 380 to 780
 * every 5. The bake bins (380–760 nm) and the channels never go below 380 nm; a file's own grid
 * ({@link MaterialIndexFile.wavelengthsNm}) may start lower.
 */
export const MATERIAL_WAVELENGTHS_NM: Float64Array = Float64Array.from(
  { length: 81 },
  (_, i) => 380 + 5 * i,
);

/** The step of every material file's grid, nm. */
const GRID_STEP_NM = 5;

/** The last wavelength of every material file's grid, nm. */
const GRID_LAST_NM = 780;

/** The latest a file's grid may start, nm: the first of {@link MATERIAL_WAVELENGTHS_NM}. */
const GRID_FIRST_MAX_NM = 380;

/**
 * Whether a grid runs every 5 nm, each value exact, from a positive whole multiple of 5 nm at or
 * below 380 nm to 780 nm.
 */
function isFileGrid(grid: Float64Array): boolean {
  const first = grid[0] ?? Number.NaN;
  return (
    first > 0 &&
    first <= GRID_FIRST_MAX_NM &&
    Number.isInteger(first / GRID_STEP_NM) &&
    grid.length === (GRID_LAST_NM - first) / GRID_STEP_NM + 1 &&
    grid.every((nm, i) => nm === first + GRID_STEP_NM * i)
  );
}

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

/** The forms a derived file's fit may take. */
const FIT_FORMS: ReadonlyArray<DerivedFit["form"]> = [
  "cauchy",
  "scaledMolarRefraction",
  "scaledGasAbsorption",
];

/**
 * A file's fit checked and typed: null, a Cauchy fit, a scaled molar refraction (whose density
 * and factor are positive, and whose anchors run from the shorter wavelength to the longer) or a
 * scaled gas absorption (whose density is positive, and whose span runs from a positive wavelength
 * to a longer one).
 */
function fitOf(value: unknown, file: string): DerivedFit | undefined {
  if (value === null) {
    return undefined;
  }
  const form = isRecord(value) ? value["form"] : undefined;
  if (!isRecord(value) || !member(FIT_FORMS, form)) {
    throw new Error(
      `${file}: "fit" must be null, a Cauchy fit, a scaled molar refraction or a scaled gas absorption`,
    );
  }
  const field = (name: string): number => {
    const v = value[name];
    if (typeof v !== "number" || !Number.isFinite(v)) {
      throw new Error(`${file}: the fit's "${name}" must be a finite number`);
    }
    return v;
  };
  if (form === "cauchy") {
    return {
      form,
      a: field("a"),
      bUm2: field("bUm2"),
      absorptionPerM: field("absorptionPerM"),
      fittedFromUm: field("fittedFromUm"),
      fittedToUm: field("fittedToUm"),
    };
  }
  if (form === "scaledGasAbsorption") {
    const absorption: ScaledGasAbsorptionFit = {
      form,
      molarDensityMolPerL: field("molarDensityMolPerL"),
      fromUm: field("fromUm"),
      toUm: field("toUm"),
    };
    if (!(absorption.molarDensityMolPerL > 0)) {
      throw new Error(`${file}: the fit's density must be positive`);
    }
    if (!(absorption.fromUm > 0 && absorption.fromUm < absorption.toUm)) {
      throw new Error(`${file}: the fit's "fromUm" must be positive and below "toUm"`);
    }
    return absorption;
  }
  const fit: ScaledMolarRefractionFit = {
    form,
    molarDensityMolPerL: field("molarDensityMolPerL"),
    factor: field("factor"),
    anchoredFromUm: field("anchoredFromUm"),
    anchoredToUm: field("anchoredToUm"),
  };
  if (!(fit.molarDensityMolPerL > 0 && fit.factor > 0)) {
    throw new Error(`${file}: the fit's density and factor must be positive`);
  }
  if (!(fit.anchoredFromUm <= fit.anchoredToUm)) {
    throw new Error(`${file}: the fit's "anchoredFromUm" must not exceed "anchoredToUm"`);
  }
  return fit;
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
  if (!isFileGrid(grid)) {
    throw new Error(
      `${file}: "wavelengthsNm" must run every 5 nm from a multiple of 5 nm at or below 380 nm to 780 nm`,
    );
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
  // Optional in the files, unlike `standIn`: only a file with a statement carries the field.
  const absorptionStandIn =
    raw["absorptionStandIn"] === undefined
      ? undefined
      : optionalText(raw, "absorptionStandIn", file);
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
    absorptionStandIn,
    fit,
    wavelengthsNm: grid,
    n,
    k,
  };
}

/**
 * Every material file, in the registry's order: a key's first file is its default, taken when a
 * mode names no phase or variant (liquid water before ice, liquid methane before its ice, 75 wt%
 * sulphuric acid before 84.5 wt%, a choice: both lie inside Hansen and Hovenier 1974's 1.44 ±
 * 0.015 at 550 nm, who name only "a concentrated solution of sulfuric acid"). R08.T5.d's
 * condensates, hazes, ices and dusts follow T5.b's (decision-r08-t5d). R08.T5.g's liquids come
 * next, so that every key's default is unchanged: seawater as liquid H₂O's `S35` variant, liquid
 * N₂, NH₃ and S₈ after their solids, and liquid C₂H₆, the key's only file. R08.T5.k's nine liquids
 * come last (decision-p14-t49h-gas-index): liquid O₂, Ar, CO, CO₂ and H₂S after their solids,
 * which stay their keys' defaults, so that a liquid of one is its own file and no longer its ice
 * as a named analogue; and liquid Kr, Xe, N₂O and SO₂, each its key's only file.
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
  parseMaterialFile(potassiumChloride, "potassium-chloride.json"),
  parseMaterialFile(sodiumChloride, "sodium-chloride.json"),
  parseMaterialFile(zincSulphide, "zinc-sulphide.json"),
  parseMaterialFile(sodiumSulphide, "sodium-sulphide.json"),
  parseMaterialFile(manganeseSulphide, "manganese-sulphide.json"),
  parseMaterialFile(chromium, "chromium.json"),
  parseMaterialFile(ironSulphide, "iron-sulphide.json"),
  parseMaterialFile(siliconDioxide, "silicon-dioxide.json"),
  parseMaterialFile(aluminiumOxide, "aluminium-oxide.json"),
  parseMaterialFile(calciumTitanate, "calcium-titanate.json"),
  parseMaterialFile(titaniumDioxide, "titanium-dioxide.json"),
  parseMaterialFile(graphiteFile, "graphite.json"),
  parseMaterialFile(sulphur, "sulphur.json"),
  parseMaterialFile(hydrogenSulphideIce, "hydrogen-sulphide-ice.json"),
  parseMaterialFile(nitrogenIce, "nitrogen-ice.json"),
  parseMaterialFile(carbonMonoxideIce, "carbon-monoxide-ice.json"),
  parseMaterialFile(oxygenIce, "oxygen-ice.json"),
  parseMaterialFile(argonIce, "argon-ice.json"),
  parseMaterialFile(jovianChromophore, "jovian-chromophore.json"),
  parseMaterialFile(iceGiantHaze, "ice-giant-haze.json"),
  parseMaterialFile(sulphuricAcidUvAbsorber, "sulphuric-acid-uv-absorber.json"),
  parseMaterialFile(basaltFile, "basalt.json"),
  parseMaterialFile(marsRegolithSimulant, "mars-regolith-simulant.json"),
  parseMaterialFile(basalticGlass, "basaltic-glass.json"),
  parseMaterialFile(seawater, "seawater.json"),
  parseMaterialFile(nitrogenLiquid, "nitrogen-liquid.json"),
  parseMaterialFile(ammoniaLiquid, "ammonia-liquid.json"),
  parseMaterialFile(ethaneLiquid, "ethane-liquid.json"),
  parseMaterialFile(sulphurLiquid, "sulphur-liquid.json"),
  parseMaterialFile(argonLiquid, "argon-liquid.json"),
  parseMaterialFile(kryptonLiquid, "krypton-liquid.json"),
  parseMaterialFile(xenonLiquid, "xenon-liquid.json"),
  parseMaterialFile(oxygenLiquid, "oxygen-liquid.json"),
  parseMaterialFile(nitrousOxideLiquid, "nitrous-oxide-liquid.json"),
  parseMaterialFile(sulphurDioxideLiquid, "sulphur-dioxide-liquid.json"),
  parseMaterialFile(carbonDioxideLiquid, "carbon-dioxide-liquid.json"),
  parseMaterialFile(carbonMonoxideLiquid, "carbon-monoxide-liquid.json"),
  parseMaterialFile(hydrogenSulphideLiquid, "hydrogen-sulphide-liquid.json"),
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
  /**
   * A file's `variant`, a composition identifier never shown on screen ({@link variantWords}): a
   * solute's mass fraction in per cent as `<mass fraction>wt%` (`75wt%`), or a practical salinity
   * as `S<practical salinity>` (`S35`, `S38.5`), the number as written.
   */
  readonly variant?: string;
}

/**
 * A file's composition variant in words, for a note. The files write two patterns: `75wt%`, a
 * solute's mass fraction in per cent, reads `mass fraction 75 %`, and `S35`, a practical salinity,
 * reads `practical salinity 35` (decision-r08-t5g ruling 4).
 *
 * @remarks
 * The quantity is named and the % sign carries nothing (the SI Brochure, 9th edition, §5.4.7:
 * "percentage by mass" shall not be used). Practical salinity is dimensionless (PSS-78), so it takes
 * no unit. Any other variant, or none, reads `another composition`, so that a variant's identifier
 * never reaches the screen.
 */
export function variantWords(variant: string | undefined): string {
  if (variant === undefined) {
    return "another composition";
  }
  const percent = /^([0-9]+(?:\.[0-9]+)?)wt%$/u.exec(variant)?.[1];
  if (percent !== undefined) {
    return `mass fraction ${percent} %`;
  }
  const salinity = /^S([0-9]+(?:\.[0-9]+)?)$/u.exec(variant)?.[1];
  return salinity === undefined ? "another composition" : `practical salinity ${salinity}`;
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
 * The volatile ices, whose solid planetary science names an ice (`CO2 ice`, `SO2 ice`): every key
 * of `nonSpherical.ts`'s `ICE_KEYS` (held to contain it by `materials.test.ts`, kept apart here
 * since `nonSpherical.ts` imports this module), and the volatiles whose files are liquids alone,
 * SO₂, N₂O, C₂H₆, Kr and Xe (`signoff-12.md`, item 39(c)).
 *
 * @remarks
 * Closed set, for the composition audit: a volatile the registry adds a file of belongs here. Its
 * atomic solids ({@link ATOMIC_SOLID_KEYS}) take `solid`, as the guide's Voice and nomenclature
 * has "ice" name a molecular solid.
 */
export const VOLATILE_ICE_KEYS: ReadonlySet<string> = new Set([
  "H2O",
  "NH3",
  "CH4",
  "CO2",
  "NH4SH",
  "H2S",
  "N2",
  "CO",
  "O2",
  "Ar",
  "SO2",
  "N2O",
  "C2H6",
  "Kr",
  "Xe",
]);

/**
 * The volatile ices that are atomic solids, whose solid a note names `solid Ar`, not `Ar ice`
 * (`nonSpherical.ts`'s phase notes; the guide, Voice and nomenclature).
 */
export const ATOMIC_SOLID_KEYS: ReadonlySet<string> = new Set(["Ar", "Kr", "Xe"]);

/**
 * A material's name with its phase word, as a stand-in's note writes it (`signoff-12.md`, item
 * 39(c)): `SO2 ice` for a volatile molecular solid, `solid Kr` for an atomic one and `solid Fe`
 * for a refractory, `liquid SO2` for a liquid, and the name alone with no phase.
 */
function phasedName(material: string, phase: MaterialPhase | undefined): string {
  const name = substanceName(material);
  if (phase === undefined) {
    return name;
  }
  let phased: string;
  switch (phase) {
    case "liquid":
      phased = `liquid ${name}`;
      break;
    case "solid":
      phased =
        VOLATILE_ICE_KEYS.has(material) && !ATOMIC_SOLID_KEYS.has(material)
          ? `${name} ice`
          : `solid ${name}`;
      break;
  }
  return phased;
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
      standIn: `no index file for ${substanceName(material)}: the generic stand-in, a non-absorbing sphere of real index ${GENERIC_STAND_IN_INDEX.n}, a stated convention`,
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
      standIn: `no ${phasedName(material, phase)} on file: ${first.name} stands in, a named analogue`,
    };
  }
  const variant = form?.variant;
  const file = variant === undefined ? phaseFirst : ofPhase.find((f) => f.variant === variant);
  if (file === undefined) {
    return {
      material,
      file: phaseFirst,
      provenance: "standIn",
      standIn: `no ${substanceName(material)}, ${variantWords(variant)}, on file: ${phaseFirst.name} stands in, a named analogue`,
    };
  }
  return { material, file, provenance: file.provenance, standIn: file.standIn };
}

/**
 * A resolved material's index at a wavelength: n linear and k linear in log k between the file's
 * wavelengths, as the files were reduced.
 *
 * @remarks
 * A file is read on its own grid, from its first wavelength λ₀ to 780 nm: i = ⌊(λ − λ₀) ÷ 5⌋, held
 * at most at the grid's length minus 2, and t = (λ − λ₀ − 5i) ÷ 5, so a file that starts at 380 nm
 * gives the bits it gave before the grids started per file. The generic stand-in, which has no
 * file, covers 380–780 nm, a stated convention.
 *
 * @param wavelengthNm - The vacuum wavelength, nm, in the file's own range: 380–780 for most files
 *   and the generic stand-in, 360–780 for the two files that start at 360 nm.
 * @throws RangeError for a wavelength outside the file's range.
 */
export function indexAt(resolved: ResolvedMaterial, wavelengthNm: number): ComplexIndex {
  const { file } = resolved;
  const first = file?.wavelengthsNm[0] ?? GRID_FIRST_MAX_NM;
  if (!(wavelengthNm >= first && wavelengthNm <= GRID_LAST_NM)) {
    throw new RangeError(
      `wavelength ${wavelengthNm} nm is outside the file's ${first}–${GRID_LAST_NM} nm`,
    );
  }
  if (file === undefined) {
    return GENERIC_STAND_IN_INDEX;
  }
  const step = GRID_STEP_NM;
  const i = Math.min(Math.floor((wavelengthNm - first) / step), file.wavelengthsNm.length - 2);
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
 * @param wavelengthNm - The vacuum wavelength, nm, in the resolved file's own range
 *   ({@link indexAt}): 380–780, or 360–780 for a file that starts at 360 nm.
 * @param form - The mode's phase or variant, where the material has several files.
 * @throws RangeError for a wavelength outside the resolved file's range.
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
 * @remarks
 * A file drawn whose colour rests on its gas's absorption (its
 * {@link MaterialIndexFile.absorptionStandIn}, liquid O₂'s) gives a reason of its own under the same
 * rule, its phase and name first (`signoff-12.md`, item 38(c)), whatever its provenance.
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
    if (!(depth < CLOUD_DECK_SPLIT_OPTICAL_DEPTH)) {
      continue;
    }
    const resolved = resolveMaterial(layer.material, layer.form);
    if (resolved.provenance === "standIn") {
      out.push({ material: layer.material, reason: resolved.standIn ?? "a stand-in" });
    }
    const drawn = resolved.file;
    if (drawn?.absorptionStandIn !== undefined) {
      out.push({
        material: layer.material,
        reason: `${phasedName(drawn.key, drawn.phase)}: ${drawn.absorptionStandIn}`,
      });
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
