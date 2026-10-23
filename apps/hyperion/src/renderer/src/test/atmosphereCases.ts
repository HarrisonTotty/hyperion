/**
 * The client's writer of the reference tracer's case format (plan R08, R08.T12.b.1): a case of
 * `view/atmosphere/reference/cases.ts` as the JSON `hyperion-fit atmosphere-reference` reads
 * (`crates/hyperion-fit/src/atmosphere/case.rs`, `CASE_FORMAT` 1), every term written from the full
 * medium.
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
 *   sun's curve row, ln u at the nodes and −ln T per channel, the client's `AbsorberCurve` of one
 *   pressure row. A curve with pressure rows, or a pair absorber, is refused until R08.T12.b.2.
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
import type { LocalDirection } from "../view/atmosphere/reference/geometries";
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

/** One sun's curve of growth as the format writes it. */
export interface AbsorberCurveJson {
  readonly logColumns: readonly number[];
  readonly opticalDepth: readonly (readonly number[])[];
}

/** A term as the format writes it. */
export interface TermJson {
  readonly name: string;
  readonly density: DensityJson;
  readonly scattering: readonly number[];
  readonly absorption: readonly number[];
  readonly phase: PhaseJson;
  readonly depolarising?: true;
  readonly absorber?: {
    readonly kind: "column";
    readonly numberDensityPerM3: number;
    readonly curves: readonly AbsorberCurveJson[];
  };
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
 * One sun's curve of growth in the format.
 *
 * @throws RangeError for a curve with pressure rows, refused until R08.T12.b.2.
 */
export function absorberCurveJson(curve: AbsorberCurve): AbsorberCurveJson {
  if (curve.logPressures.length !== 1) {
    throw new RangeError(
      `a curve of ${curve.logPressures.length} pressure rows is traced from R08.T12.b.2, not here`,
    );
  }
  return {
    logColumns: Array.from(curve.logColumns),
    opticalDepth: curve.opticalDepth.map((channel) => Array.from(channel)),
  };
}

/**
 * A term in the format, `curve` its absorber's curve under the case's sun.
 *
 * @throws RangeError for a pair absorber, a column absorber with no curve, or a table with no
 *   asymmetry given, as {@link absorberCurveJson}.
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
    if (absorber.kind === "pair") {
      throw new RangeError(`pair absorber ${term.name} is traced from R08.T12.b.2, not here`);
    }
    if (curve === undefined) {
      throw new RangeError(`absorber ${term.name} has no curve`);
    }
    absorberJson = {
      kind: "column",
      numberDensityPerM3: absorber.numberDensityPerM3,
      curves: [absorberCurveJson(curve)],
    };
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
  const terms = medium.terms.map((term) => {
    const slot = absorbers.indexOf(term);
    return termJson(
      term,
      slot < 0 ? undefined : referenceCase.curves[slot],
      referenceCase.phaseAsymmetry[term.name],
    );
  });
  return {
    format: CASE_FORMAT,
    name: referenceCase.name,
    seed: referenceCase.seed,
    wavelengthsNm: rgbArray(CHANNEL_WAVELENGTHS_NM),
    shells: shellsJson(referenceCase.figure),
    topHeightM: medium.topHeightM,
    groundAlbedo: rgbArray(medium.groundAlbedo),
    terms,
    suns: [{ name: "sun", irradiance: rgbArray(referenceCase.sunIlluminanceLx) }],
    detectorHalfAngleDeg: referenceCase.detectorHalfAngleDeg,
    geometries: referenceCase.geometries.map((g) => ({
      name: g.name,
      observer: { heightM: g.observer.heightM, latitudeDeg: g.observer.latitudeDeg },
      view: { zenithDeg: g.view.zenithDeg, azimuthDeg: g.view.azimuthDeg },
      sunDirections: [{ zenithDeg: g.sun.zenithDeg, azimuthDeg: g.sun.azimuthDeg }],
      ...(g.maxDistanceM === undefined ? {} : { maxDistanceM: g.maxDistanceM }),
      ...(g.detectorHalfAngleDeg === undefined
        ? {}
        : { detectorHalfAngleDeg: g.detectorHalfAngleDeg }),
      ...ownSamples(counts?.geometries?.[g.name]),
    })),
    aggregates: referenceCase.aggregates.map((a) => ({
      name: a.name,
      latitudeDeg: a.latitudeDeg,
      sunDirections: [{ zenithDeg: a.sun.zenithDeg, azimuthDeg: a.sun.azimuthDeg }],
      ...ownSamples(counts?.aggregates?.[a.name]),
    })),
  };
}

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
