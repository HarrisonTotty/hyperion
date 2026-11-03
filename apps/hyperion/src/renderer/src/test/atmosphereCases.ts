/**
 * The client's writer of the reference tracer's case format (plan R08, R08.T12.b): a case of
 * `view/atmosphere/reference/cases.ts` or `thickCases.ts` as the JSON `hyperion-fit
 * atmosphere-reference` reads (`crates/hyperion-fit/src/atmosphere/case.rs`, `CASE_FORMAT` 1), every
 * term written from the full medium.
 *
 * @remarks
 * - **Terms** are the medium's `MediumTerm`s at the three fitted channels, the case's wavelengths.
 *   A `tabulated` density loses its `edgesM`, which the format refuses: the tracer splits its
 *   shells at every node, and so at every kink.
 * - **Phases.** Rayleigh with its depolarisation per channel; a table with its matrix and its
 *   forward delta as the client holds it, its entries divided by their integral over the sphere
 *   read linearly in u (the tracer's rule, `mieMatrices.ts`'s `linearInUMoments`), and its
 *   `asymmetry` the source's mean cosine (the mode's g), not the table's (R08.T12.c's finding); a
 *   table without a matrix, and Cornette and Shanks's function, marked `depolarising`, a stated
 *   approximation (`decision-r08-vector.md`).
 * - **Absorbers** (`decision-r08-t12b-scope.md`, ruling 4) carry n at relative density 1 and each
 *   sun's curve, ln u at the nodes and −ln T per channel, the client's `AbsorberCurve`: a column
 *   absorber's n in molecules m⁻³, or since R08.T12.b.2 a pair absorber's n_A n_B in molecule pairs
 *   m⁻⁶; and a curve of several pressure rows with its rows' ln p, its read's top and its depths
 *   row-major, the medium's `pressureBar` written beside the terms for it.
 * - **Figures** (R08.T12.b.2). A case that gives its `significantFigures` has every number of its
 *   terms and its pressure profile written rounded to that many, its geometries in full, so that a
 *   medium of several tabulated phases stays under the added-file hook; rounding is monotone, so a
 *   matrix element no larger than a₁ stays so.
 * - **Counts.** A geometry or aggregate takes its own `samples` where the case's counts give one
 *   (`decision-r08-t12b-scope.md`, ruling 6).
 *
 * Typed arrays become plain arrays, which `bless.ts`'s `tableText` requires.
 */

import type { Rgb } from "../view/photometry/toneCurve";
import type { AbsorberCurve } from "../view/atmosphere/absorbers";
import {
  CHANNEL_WAVELENGTHS_NM,
  type DensityProfile,
  type MediumTerm,
  type PhaseTable,
} from "../view/atmosphere/medium";
import type { ReferenceCase } from "../view/atmosphere/reference/cases";
import type {
  LocalDirection,
  ReferenceAggregate,
  ReferenceGeometry,
} from "../view/atmosphere/reference/geometries";
import type {
  SpectralCaseAbsorber,
  SpectralCasePhase,
  SpectralReferenceCase,
} from "../view/atmosphere/reference/spectralCases";
import { linearInUMoments } from "./mieMatrices";

/** The case format the writer writes, `case.rs`'s `CASE_FORMAT`. */
export const CASE_FORMAT = 1;

/** A case's own sample counts, by geometry and aggregate name, where it gives them. */
export interface SampleCounts {
  readonly geometries?: Readonly<Record<string, number>>;
  readonly aggregates?: Readonly<Record<string, number>>;
}

/** A density profile as the format writes it. */
export type DensityJson =
  | { readonly kind: "exponential"; readonly scaleHeightM: number }
  | {
      readonly kind: "tent";
      readonly bottomM: number;
      readonly peakM: number;
      readonly topM: number;
    }
  | {
      readonly kind: "tabulated";
      readonly altitudesM: readonly number[];
      readonly relative: readonly number[];
    };

/** A phase function as the format writes it. */
export type PhaseJson =
  | { readonly kind: "rayleigh"; readonly depolarisation: readonly number[] }
  | { readonly kind: "cornette-shanks"; readonly asymmetry: number }
  | { readonly kind: "none" }
  | {
      readonly kind: "tabulated";
      readonly u: readonly number[];
      readonly a1: readonly (readonly number[])[];
      readonly asymmetry: readonly number[];
      readonly matrix?: Readonly<
        Record<"a2" | "a3" | "a4" | "b1" | "b2", readonly (readonly number[])[]>
      >;
    };

/** One sun's curve of growth as the format writes it: rows only for a curve of several. */
export interface AbsorberCurveJson {
  readonly logColumns: readonly number[];
  readonly logPressures?: readonly number[];
  readonly readTopLogPressure?: number;
  readonly opticalDepth: readonly (readonly number[])[];
}

/** An absorber as the format writes it. */
export type AbsorberJson =
  | {
      readonly kind: "column";
      readonly numberDensityPerM3: number;
      readonly curves: readonly AbsorberCurveJson[];
    }
  | {
      readonly kind: "pair";
      readonly pairDensityPerM6: number;
      readonly curves: readonly AbsorberCurveJson[];
    };

/** A term as the format writes it. */
export interface TermJson {
  readonly name: string;
  readonly density: DensityJson;
  readonly scattering: readonly number[];
  readonly absorption: readonly number[];
  readonly phase: PhaseJson;
  readonly depolarising?: true;
  readonly absorber?: AbsorberJson;
}

/** A geometry as the format writes it. */
export interface GeometryJson {
  readonly name: string;
  readonly observer: { readonly heightM: number; readonly latitudeDeg: number };
  readonly view: LocalDirection;
  readonly sunDirections: readonly LocalDirection[];
  readonly maxDistanceM?: number;
  readonly detectorHalfAngleDeg?: number;
  readonly samples?: number;
}

/** An aggregate as the format writes it. */
export interface AggregateJson {
  readonly name: string;
  readonly latitudeDeg: number;
  readonly sunDirections: readonly LocalDirection[];
  readonly samples?: number;
}

/** The case file. */
export interface CaseJson {
  readonly format: number;
  readonly name: string;
  readonly seed: number;
  readonly wavelengthsNm: readonly number[];
  readonly shells:
    | { readonly kind: "sphere"; readonly radiusM: number }
    | {
        readonly kind: "spheroid";
        readonly equatorialRadiusM: number;
        readonly polarRadiusM: number;
        readonly gmM3S2: number;
        readonly angularVelocityRadS: number;
        readonly gravityOffsetMS2: number;
      };
  readonly topHeightM: number;
  readonly groundAlbedo: readonly number[];
  readonly pressureBar?: DensityJson;
  readonly terms: readonly TermJson[];
  readonly suns: readonly { readonly name: string; readonly irradiance: readonly number[] }[];
  readonly detectorHalfAngleDeg: number;
  readonly geometries: readonly GeometryJson[];
  readonly aggregates: readonly AggregateJson[];
}

function rgbArray(value: Rgb): number[] {
  return [value[0], value[1], value[2]];
}

/** A profile in the format: `edgesM` dropped (the module's remarks). */
export function densityJson(profile: DensityProfile): DensityJson {
  let json: DensityJson;
  switch (profile.kind) {
    case "exponential":
      json = { kind: "exponential", scaleHeightM: profile.scaleHeightM };
      break;
    case "tent":
      json = { kind: "tent", bottomM: profile.bottomM, peakM: profile.peakM, topM: profile.topM };
      break;
    case "tabulated":
      json = {
        kind: "tabulated",
        altitudesM: Array.from(profile.altitudesM),
        relative: Array.from(profile.relative),
      };
      break;
  }
  return json;
}

/**
 * A phase table in the format: each channel's entries divided by its integral over the sphere
 * read linearly in u, the tracer's rule, with `asymmetry` the source's g per channel.
 */
export function tabulatedPhaseJson(table: PhaseTable, asymmetry: Rgb): PhaseJson {
  const integrals = table.values.map((channel) => linearInUMoments(table.u, channel).integral);
  const rows = (element: readonly [Float64Array, Float64Array, Float64Array]): number[][] =>
    element.map((channel, c) => Array.from(channel, (v) => v / (integrals[c] ?? Number.NaN)));
  const { matrix } = table;
  return {
    kind: "tabulated",
    u: Array.from(table.u),
    a1: rows(table.values),
    asymmetry: rgbArray(asymmetry),
    ...(matrix === undefined
      ? {}
      : {
          matrix: {
            a2: rows(matrix.a2),
            a3: rows(matrix.a3),
            a4: rows(matrix.a4),
            b1: rows(matrix.b1),
            b2: rows(matrix.b2),
          },
        }),
  };
}

/**
 * One sun's curve of growth in the format: a curve of one row as before R08.T12.b.2, and one of
 * several with its rows' ln p and its read's top, its depths row-major, `[r × nodes + j]`, as the
 * client holds them.
 */
export function absorberCurveJson(curve: AbsorberCurve): AbsorberCurveJson {
  const rows =
    curve.logPressures.length > 1
      ? {
          logPressures: Array.from(curve.logPressures),
          readTopLogPressure: curve.readTopLogPressure,
        }
      : {};
  return {
    logColumns: Array.from(curve.logColumns),
    ...rows,
    opticalDepth: curve.opticalDepth.map((channel) => Array.from(channel)),
  };
}

/**
 * A term in the format, `curve` its absorber's curve under the case's sun.
 *
 * @throws RangeError for an absorber with no curve or with a curve of the other kind, or a table
 *   with no asymmetry given.
 */
export function termJson(
  term: MediumTerm,
  curve: AbsorberCurve | undefined,
  asymmetry: Rgb | undefined,
): TermJson {
  const common = {
    name: term.name,
    density: densityJson(term.density),
    scattering: rgbArray(term.scattering),
    absorption: rgbArray(term.absorption),
  };
  const absorber = term.absorber;
  let absorberJson: TermJson["absorber"];
  if (absorber !== undefined) {
    if (curve === undefined) {
      throw new RangeError(`absorber ${term.name} has no curve`);
    }
    if (curve.kind !== absorber.kind) {
      throw new RangeError(`${absorber.kind} absorber ${term.name} has a ${curve.kind} curve`);
    }
    const curves = [absorberCurveJson(curve)];
    switch (absorber.kind) {
      case "column":
        absorberJson = { kind: "column", numberDensityPerM3: absorber.numberDensityPerM3, curves };
        break;
      case "pair":
        absorberJson = { kind: "pair", pairDensityPerM6: absorber.pairDensityPerM6, curves };
        break;
    }
  }
  const withAbsorber = absorberJson === undefined ? {} : { absorber: absorberJson };
  const { phase } = term;
  let json: TermJson;
  switch (phase.kind) {
    case "rayleigh":
      json = {
        ...common,
        phase: { kind: "rayleigh", depolarisation: rgbArray(phase.depolarisation) },
        ...withAbsorber,
      };
      break;
    case "cornette-shanks":
      json = {
        ...common,
        phase: { kind: "cornette-shanks", asymmetry: phase.asymmetry },
        depolarising: true,
        ...withAbsorber,
      };
      break;
    case "none":
      json = { ...common, phase: { kind: "none" }, ...withAbsorber };
      break;
    case "tabulated": {
      if (asymmetry === undefined) {
        throw new RangeError(`tabulated term ${term.name} needs its source's asymmetry`);
      }
      json = {
        ...common,
        phase: tabulatedPhaseJson(phase.table, asymmetry),
        ...(phase.table.matrix === undefined ? { depolarising: true as const } : {}),
        ...withAbsorber,
      };
      break;
    }
  }
  return json;
}

/** A number rounded to `figures` significant figures. */
function roundedNumber(value: number, figures: number): number {
  return Number(value.toPrecision(figures));
}

/** Numbers rounded to `figures` significant figures. */
function roundedNumbers(values: readonly number[], figures: number): number[] {
  return values.map((v) => roundedNumber(v, figures));
}

/** Rows of numbers rounded to `figures` significant figures. */
function roundedRows(rows: readonly (readonly number[])[], figures: number): number[][] {
  return rows.map((row) => roundedNumbers(row, figures));
}

/** A profile with its numbers rounded to `figures` significant figures. */
function roundedDensity(density: DensityJson, figures: number): DensityJson {
  let rounded: DensityJson;
  switch (density.kind) {
    case "exponential":
      rounded = { kind: "exponential", scaleHeightM: roundedNumber(density.scaleHeightM, figures) };
      break;
    case "tent":
      rounded = {
        kind: "tent",
        bottomM: roundedNumber(density.bottomM, figures),
        peakM: roundedNumber(density.peakM, figures),
        topM: roundedNumber(density.topM, figures),
      };
      break;
    case "tabulated":
      rounded = {
        kind: "tabulated",
        altitudesM: roundedNumbers(density.altitudesM, figures),
        relative: roundedNumbers(density.relative, figures),
      };
      break;
  }
  return rounded;
}

/** A phase with its numbers rounded to `figures` significant figures. */
function roundedPhase(phase: PhaseJson, figures: number): PhaseJson {
  let rounded: PhaseJson;
  switch (phase.kind) {
    case "rayleigh":
      rounded = { kind: "rayleigh", depolarisation: roundedNumbers(phase.depolarisation, figures) };
      break;
    case "cornette-shanks":
      rounded = { kind: "cornette-shanks", asymmetry: roundedNumber(phase.asymmetry, figures) };
      break;
    case "none":
      rounded = phase;
      break;
    case "tabulated": {
      const { matrix } = phase;
      rounded = {
        kind: "tabulated",
        u: roundedNumbers(phase.u, figures),
        a1: roundedRows(phase.a1, figures),
        asymmetry: roundedNumbers(phase.asymmetry, figures),
        ...(matrix === undefined
          ? {}
          : {
              matrix: {
                a2: roundedRows(matrix.a2, figures),
                a3: roundedRows(matrix.a3, figures),
                a4: roundedRows(matrix.a4, figures),
                b1: roundedRows(matrix.b1, figures),
                b2: roundedRows(matrix.b2, figures),
              },
            }),
      };
      break;
    }
  }
  return rounded;
}

/** A curve with its numbers rounded to `figures` significant figures. */
function roundedCurve(curve: AbsorberCurveJson, figures: number): AbsorberCurveJson {
  const { logPressures, readTopLogPressure } = curve;
  return {
    logColumns: roundedNumbers(curve.logColumns, figures),
    ...(logPressures === undefined ? {} : { logPressures: roundedNumbers(logPressures, figures) }),
    ...(readTopLogPressure === undefined
      ? {}
      : { readTopLogPressure: roundedNumber(readTopLogPressure, figures) }),
    opticalDepth: roundedRows(curve.opticalDepth, figures),
  };
}

/** An absorber with its numbers rounded to `figures` significant figures. */
function roundedAbsorber(absorber: AbsorberJson, figures: number): AbsorberJson {
  const curves = absorber.curves.map((c) => roundedCurve(c, figures));
  let rounded: AbsorberJson;
  switch (absorber.kind) {
    case "column":
      rounded = {
        kind: "column",
        numberDensityPerM3: roundedNumber(absorber.numberDensityPerM3, figures),
        curves,
      };
      break;
    case "pair":
      rounded = {
        kind: "pair",
        pairDensityPerM6: roundedNumber(absorber.pairDensityPerM6, figures),
        curves,
      };
      break;
  }
  return rounded;
}

/**
 * A term with every number rounded to `figures` significant figures (the module's remarks), its
 * fields in their order.
 */
export function roundedTermJson(term: TermJson, figures: number): TermJson {
  const { depolarising, absorber } = term;
  return {
    name: term.name,
    density: roundedDensity(term.density, figures),
    scattering: roundedNumbers(term.scattering, figures),
    absorption: roundedNumbers(term.absorption, figures),
    phase: roundedPhase(term.phase, figures),
    ...(depolarising === undefined ? {} : { depolarising }),
    ...(absorber === undefined ? {} : { absorber: roundedAbsorber(absorber, figures) }),
  };
}

/** An observation's own `samples`, where its case's counts give one. */
function ownSamples(count: number | undefined): { readonly samples?: number } {
  return count === undefined ? {} : { samples: count };
}

/** A case's figure as the format's shells. */
function shellsJson(figure: ReferenceCase["figure"]): CaseJson["shells"] {
  let shells: CaseJson["shells"];
  switch (figure.kind) {
    case "sphere":
      shells = { kind: "sphere", radiusM: figure.radiusM };
      break;
    case "spheroid":
      shells = {
        kind: "spheroid",
        equatorialRadiusM: figure.gravity.spheroid.equatorialRadiusM,
        polarRadiusM: figure.gravity.spheroid.polarRadiusM,
        gmM3S2: figure.gravity.spheroid.gmM3S2,
        angularVelocityRadS: figure.gravity.spheroid.angularVelocityRadS,
        gravityOffsetMS2: figure.gravity.gravityOffsetMS2,
      };
      break;
  }
  return shells;
}

/**
 * A case in the format (the module's remarks), with its own `counts` where it gives them.
 *
 * @throws RangeError as {@link termJson}, or for curves that are not one an absorber term.
 */
export function caseJson(referenceCase: ReferenceCase, counts: SampleCounts | undefined): CaseJson {
  const { medium } = referenceCase;
  const absorbers = medium.terms.filter((term) => term.absorber !== undefined);
  if (absorbers.length !== referenceCase.curves.length) {
    throw new RangeError(
      `case ${referenceCase.name} has ${referenceCase.curves.length} curves for ${absorbers.length} absorbers`,
    );
  }
  const figures = referenceCase.significantFigures;
  const terms = medium.terms.map((term) => {
    const slot = absorbers.indexOf(term);
    const json = termJson(
      term,
      slot < 0 ? undefined : referenceCase.curves[slot],
      referenceCase.phaseAsymmetry[term.name],
    );
    return figures === undefined ? json : roundedTermJson(json, figures);
  });
  const profile = medium.pressureBar === undefined ? undefined : densityJson(medium.pressureBar);
  const pressure =
    profile === undefined
      ? {}
      : { pressureBar: figures === undefined ? profile : roundedDensity(profile, figures) };
  return {
    format: CASE_FORMAT,
    name: referenceCase.name,
    seed: referenceCase.seed,
    wavelengthsNm: rgbArray(CHANNEL_WAVELENGTHS_NM),
    shells: shellsJson(referenceCase.figure),
    topHeightM: medium.topHeightM,
    groundAlbedo: rgbArray(medium.groundAlbedo),
    ...pressure,
    terms,
    suns: [{ name: "sun", irradiance: rgbArray(referenceCase.sunIlluminanceLx) }],
    detectorHalfAngleDeg: referenceCase.detectorHalfAngleDeg,
    geometries: referenceCase.geometries.map((g) => geometryJson(g, counts)),
    aggregates: referenceCase.aggregates.map((a) => aggregateJson(a, counts)),
  };
}

/** A geometry in the format, with its own count where `counts` gives one. */
function geometryJson(g: ReferenceGeometry, counts: SampleCounts | undefined): GeometryJson {
  return {
    name: g.name,
    observer: { heightM: g.observer.heightM, latitudeDeg: g.observer.latitudeDeg },
    view: { zenithDeg: g.view.zenithDeg, azimuthDeg: g.view.azimuthDeg },
    sunDirections: [{ zenithDeg: g.sun.zenithDeg, azimuthDeg: g.sun.azimuthDeg }],
    ...(g.maxDistanceM === undefined ? {} : { maxDistanceM: g.maxDistanceM }),
    ...(g.detectorHalfAngleDeg === undefined
      ? {}
      : { detectorHalfAngleDeg: g.detectorHalfAngleDeg }),
    ...ownSamples(counts?.geometries?.[g.name]),
  };
}

/** An aggregate in the format, with its own count where `counts` gives one. */
function aggregateJson(a: ReferenceAggregate, counts: SampleCounts | undefined): AggregateJson {
  return {
    name: a.name,
    latitudeDeg: a.latitudeDeg,
    sunDirections: [{ zenithDeg: a.sun.zenithDeg, azimuthDeg: a.sun.azimuthDeg }],
    ...ownSamples(counts?.aggregates?.[a.name]),
  };
}

/**
 * A spectral case's phase in the format (R08.T17): a table's channel 0 at each wavelength divided
 * by its integral over the sphere read linearly in u, the tracer's rule, on the one u every
 * wavelength's table shares.
 *
 * @throws RangeError for tables on different entries.
 */
function spectralPhaseJson(phase: SpectralCasePhase): PhaseJson {
  let json: PhaseJson;
  switch (phase.kind) {
    case "rayleigh":
      json = { kind: "rayleigh", depolarisation: [...phase.depolarisation] };
      break;
    case "cornette-shanks":
      json = { kind: "cornette-shanks", asymmetry: phase.asymmetry };
      break;
    case "none":
      json = { kind: "none" };
      break;
    case "tabulated": {
      const [first] = phase.tables;
      if (first === undefined) {
        throw new RangeError("a tabulated spectral phase has a table a wavelength");
      }
      const u = first.u;
      if (phase.tables.some((t) => t.u.length !== u.length || t.u.some((v, i) => v !== u[i]))) {
        throw new RangeError("a tabulated spectral phase's tables share their entries");
      }
      const integrals = phase.tables.map((t) => linearInUMoments(u, t.values[0]).integral);
      const rows = (element: (t: PhaseTable) => Float64Array | undefined): number[][] =>
        phase.tables.map((t, w) =>
          Array.from(element(t) ?? [], (v) => v / (integrals[w] ?? Number.NaN)),
        );
      const matrix = phase.tables.every((t) => t.matrix !== undefined)
        ? {
            matrix: {
              a2: rows((t) => t.matrix?.a2[0]),
              a3: rows((t) => t.matrix?.a3[0]),
              a4: rows((t) => t.matrix?.a4[0]),
              b1: rows((t) => t.matrix?.b1[0]),
              b2: rows((t) => t.matrix?.b2[0]),
            },
          }
        : {};
      json = {
        kind: "tabulated",
        u: Array.from(u),
        a1: rows((t) => t.values[0]),
        asymmetry: [...phase.asymmetry],
        ...matrix,
      };
      break;
    }
  }
  return json;
}

/** A spectral case's absorber in the format: its bins' curves as one curve of 15 wavelengths. */
function spectralAbsorberJson(absorber: SpectralCaseAbsorber): AbsorberJson {
  const { curves } = absorber;
  const rows =
    curves.logPressures.length > 1
      ? {
          logPressures: Array.from(curves.logPressures),
          readTopLogPressure: curves.readTopLogPressure,
        }
      : {};
  const curve: AbsorberCurveJson = {
    logColumns: Array.from(curves.logColumns),
    ...rows,
    opticalDepth: curves.opticalDepth.map((bin) => Array.from(bin)),
  };
  let json: AbsorberJson;
  switch (absorber.kind) {
    case "column":
      json = { kind: "column", numberDensityPerM3: absorber.numberDensityPerM3, curves: [curve] };
      break;
    case "pair":
      json = { kind: "pair", pairDensityPerM6: absorber.pairDensityPerM6, curves: [curve] };
      break;
  }
  return json;
}

/**
 * A spectral case in the format (plan R08, R08.T17; `spectralCases.ts`), at its 15 wavelengths,
 * with its own `counts`, every number of its terms rounded to
 * {@link SPECTRAL_CASE_SIGNIFICANT_FIGURES} so that the files stay under the added-file hook.
 */
export function spectralCaseJson(
  spectral: SpectralReferenceCase,
  counts: SampleCounts | undefined,
): CaseJson {
  const { base } = spectral;
  const terms = spectral.terms.map((term) => {
    const phase = spectralPhaseJson(term.phase);
    const json: TermJson = {
      name: term.name,
      density: densityJson(term.density),
      scattering: [...term.scatteringPerM],
      absorption: [...term.absorptionPerM],
      phase,
      ...(phase.kind === "cornette-shanks" ||
      (phase.kind === "tabulated" && phase.matrix === undefined)
        ? { depolarising: true as const }
        : {}),
      ...(term.absorber === undefined ? {} : { absorber: spectralAbsorberJson(term.absorber) }),
    };
    return roundedTermJson(json, SPECTRAL_CASE_SIGNIFICANT_FIGURES);
  });
  const profile =
    base.medium.pressureBar === undefined ? undefined : densityJson(base.medium.pressureBar);
  return {
    format: CASE_FORMAT,
    name: spectral.name,
    seed: spectral.seed,
    wavelengthsNm: [...spectral.wavelengthsNm],
    shells: shellsJson(base.figure),
    topHeightM: base.medium.topHeightM,
    groundAlbedo: [...spectral.groundAlbedo],
    ...(profile === undefined ? {} : { pressureBar: profile }),
    terms,
    suns: [{ name: "sun", irradiance: [...spectral.sunIrradianceWPerM2Nm] }],
    detectorHalfAngleDeg: base.detectorHalfAngleDeg,
    geometries: spectral.geometries.map((g) => geometryJson(g, counts)),
    aggregates: spectral.aggregates.map((a) => aggregateJson(a, counts)),
  };
}

/**
 * The significant figures a spectral case's terms are written to: 7, as `caseGate.ts` records its
 * figures, so that Mars's dust at 15 wavelengths stays under the added-file hook.
 */
export const SPECTRAL_CASE_SIGNIFICANT_FIGURES = 7;

/**
 * The bytes a committed file stays under: 500,000, a little inside the repository's added-file hook
 * (pre-commit's `check-added-large-files` at its default 500 KiB, 512,000 B), as the atmosphere's
 * other committed tables are held.
 */
export const ADDED_FILE_LIMIT_BYTES = 500_000;

/**
 * The count of a geometry or aggregate its case gives none of its own, the command's `--samples`:
 * 4,096, the top-level `samples` every committed reference records.
 */
export const REFERENCE_DEFAULT_SAMPLES = 4096;

/** The command, from the repository's root, that traces a case's reference. */
export function traceCommand(name: string): string {
  const directory = "apps/hyperion/src/renderer/src/view/atmosphere/reference";
  return (
    `cargo run --release -p hyperion-fit -- atmosphere-reference ${directory}/${name}.case.json ` +
    `--out ${directory}/${name}.reference.json --samples ${REFERENCE_DEFAULT_SAMPLES} --stokes ` +
    `--threads 4`
  );
}
