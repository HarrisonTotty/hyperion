/**
 * R08.T13.c2's recompute of the sweep and its record (plan R08, R08.T13; `decision-r08-t13-scope.md`,
 * rulings 5, 6 and 9, "#107's twin ground", "#107's floor misses" and "#112's floor refusals"):
 * every verdict of R08.T13.b's walk taken again with `gate.ts` itself, the boundary it sets, and
 * the attribution and classes `sweepBoundary.json` records.
 *
 * @remarks
 * - **The reader.** Each sweep output (`ms-sweep-w*.json`, `hyperion-fit atmosphere-sweep`'s) and
 *   its case (`ms-sweep-w*.case.json`, `test/atmosphereSweep.ts`'s) is narrowed from `unknown`
 *   field by field ({@link readSweepOutput}, {@link readSweepCase}), only the fields this module
 *   reads, a wrong shape a thrown error naming its path.
 * - **The recompute** ({@link recomputeSweep}). Each traced point's geometries are read as
 *   `gate.ts`'s {@link ReferenceReading}s from its `traced` fields: the sweep is grey, so its one
 *   channel stands in all three, and channel 1 is read; each image is the case's; N_q is
 *   ⌊N ÷ 4⌋, which the output does not write. The client's radiance is the case's twin row, which
 *   `sweepCases.test.ts` holds current, checked against the output's echo of it to its seven
 *   significant figures (a difference is a finding: the case re-blessed after the trace). `gate`
 *   gives each geometry's verdict and refusal, a refusal within {@link NEAR_SUN_DEG}° of its sun
 *   at a VROOM point listed only. The walk is then `sweep.rs`'s, rule for rule
 *   (`walk_column`, `classify`, `sort_floor`): the floor misses, the climb by octaves and on past
 *   a drift as the cell's `walkPast` says, the half-octave below the first stop, the stop's kind
 *   and τ\*, and the floor's misses sorted by the scan into fixed, ground and unsorted, with A\*.
 *   Each cell and point is held against the output's, field by field.
 * - **The boundary** ({@link boundaryOfRecompute}): the 49 cells over the walk's ground of 0.3,
 *   each at the lower τ\* of the recompute's and the walk's where they differ (ruling 6), with its
 *   kind and A\*. The bright column, over 0.8, is recorded and not in it.
 * - **The record** ({@link sweepBoundaryRecord}): a pure function of the recompute, the twin's
 *   values (each kept at seven significant figures, as `caseGate.ts` keeps its) and the classes,
 *   so that the record computed from its own values is itself.
 *
 * It makes no runtime import outside `view/atmosphere/` and none from `test/`, as `caseGate.ts`.
 */

import {
  type Comparison,
  gate,
  METRIC_FLOOR,
  METRIC_RELATIVE_TOLERANCE,
  METRIC_SIGMA_ALLOWANCE,
  METRIC_SIGMA_DIVISOR,
  type ReferenceReading,
  type RefusalReason,
  SIGMA_SCALING_TOLERANCE,
} from "../gate";
import type { AtmosphereMedium } from "../medium";
import {
  approximateRegime,
  type AtmosphereRegime,
  type BodyCover,
  type BoundaryKind,
  type ChannelRegime,
  classifyRegime,
  regimeChannels,
  type ThickBoundary,
  verticalOpticalDepth,
} from "../thick/regime";
import { recorded, type RecordValue, RECORD_SIGNIFICANT_FIGURES } from "./caseGate";

// --- The constants -------------------------------------------------------------------------------

/** The record's format: 1. */
export const SWEEP_BOUNDARY_FORMAT = 1;

/**
 * The ground the walk climbs over, the grid's: 0.3, `sweep.rs`'s `WALK_GROUND_ALBEDO`, a hand
 * value (ruling 2). The files over another ground (the bright column's 0.8) are recorded and are
 * not the table's.
 */
export const SWEEP_WALK_GROUND_ALBEDO = 0.3;

/**
 * A refused geometry this close to its sun at a VROOM point is listed only, degrees: 15
 * (`sweep.rs`'s `NEAR_SUN_DEG`, ruling 5), read inclusively, to 10⁻¹² in cosine, as the walk reads
 * it.
 */
export const NEAR_SUN_DEG = 15;

/**
 * The top-ups past which a geometry's scaling check is a heavy-tail suspect: 3 doublings, N grown
 * eightfold before σ fell as N^(−1/2) within 25% (a hand value; "#112's floor refusals", item 1).
 */
export const HEAVY_TAIL_DOUBLINGS = 3;

/** The channel the grey sweep is read in: green, 1, its three channels being equal. */
const SWEEP_CHANNEL = 1;

// --- The reader ----------------------------------------------------------------------------------

/** A τ candidate's place in the walk (`sweep.rs`'s `TauRole`). */
export type SweepTauRole = "floor" | "octave" | "half-octave";

/** A sweep geometry's kind, as the case records it. */
export type SweepGeometryKind = "ground" | "disc" | "limb" | "aerial";

/** A direction in the observer's frame, degrees. */
export interface SweepDirection {
  readonly zenithDeg: number;
  readonly azimuthDeg: number;
}

/** A case's geometry, as far as the recompute reads it. */
export interface SweepCaseGeometry {
  readonly name: string;
  readonly image: string;
  readonly kind: SweepGeometryKind;
  readonly view: SweepDirection;
  readonly sun: SweepDirection;
  /** Its cone's share that meets the ground (`twin.ts`'s `groundConeShare`). */
  readonly groundConeShare: number;
}

/** A case's column: its g, its points' indices and the twin's radiance at each. */
export interface SweepCaseColumn {
  readonly asymmetry: number;
  /** Each τ candidate's point index. */
  readonly points: ReadonlyArray<number>;
  /** The twin's radiance at each τ candidate (rows), by geometry. */
  readonly twin: ReadonlyArray<ReadonlyArray<number>>;
  /** Each scan ground's point index. */
  readonly scanPoints: ReadonlyArray<number>;
  /** The twin's radiance at the floor over each scan ground (rows), by geometry. */
  readonly scanTwin: ReadonlyArray<ReadonlyArray<number>>;
}

/** A sweep case (`SWEEP_FORMAT` 1), as far as the recompute reads it. */
export interface SweepCase {
  readonly name: string;
  readonly singleScatteringAlbedo: number;
  /** The walk's ground, the template's. */
  readonly groundAlbedo: number;
  readonly tau: ReadonlyArray<number>;
  readonly tauRoles: ReadonlyArray<SweepTauRole>;
  readonly geometries: ReadonlyArray<SweepCaseGeometry>;
  readonly scanGroundAlbedos: ReadonlyArray<number>;
  readonly columns: ReadonlyArray<SweepCaseColumn>;
}

/** How far a column's walk climbs past its first drift (`sweep.rs`'s `WalkPast`). */
export type SweepWalkPast =
  | { readonly kind: "no" }
  | { readonly kind: "through"; readonly tau: number }
  | { readonly kind: "top" };

/** A cell of an output, as the walk wrote it. */
export interface SweepCell {
  readonly singleScatteringAlbedo: number;
  readonly asymmetry: number;
  readonly groundAlbedo: number;
  readonly vroom: boolean;
  readonly walkPast: SweepWalkPast;
  readonly kind: BoundaryKind;
  /** Rounded to seven significant figures. */
  readonly tauStar: number;
  /** The points of its walk, in walk order. */
  readonly points: ReadonlyArray<number>;
  readonly floorMisses: ReadonlyArray<string>;
  readonly listedRefusals: ReadonlyArray<string>;
  readonly scanPoints: ReadonlyArray<number>;
  readonly fixedMisses: ReadonlyArray<string>;
  /** Each ground miss with the least albedo it fails over. */
  readonly groundMisses: ReadonlyArray<readonly [string, number]>;
  readonly unsortedMisses: ReadonlyArray<string>;
  readonly albedoStar: number;
}

/** Why a top-up stopped at a refused geometry. */
export type SweepTopUpCap = "geometry" | "point";

/** A refused geometry of a traced point, as the walk wrote it. */
export interface SweepRefused {
  readonly geometry: number;
  readonly refusal: RefusalReason;
  readonly listed: boolean;
  readonly cap: SweepTopUpCap;
}

/** A traced point's final trace, by geometry. */
export interface SweepTraced {
  readonly samples: ReadonlyArray<number>;
  readonly doublings: ReadonlyArray<number>;
  readonly radiance: ReadonlyArray<number>;
  readonly standardError: ReadonlyArray<number>;
  readonly quarterStandardError: ReadonlyArray<number>;
  /** The twin's radiance echoed from the case, rounded. */
  readonly twin: ReadonlyArray<number>;
  readonly failing: ReadonlyArray<number>;
  readonly refused: ReadonlyArray<SweepRefused>;
}

/** A point's class as the walk wrote it. */
export type SweepPointClass = "pass" | "drift" | "reach" | "scan";

/** A point of an output. */
export interface SweepOutputPoint {
  readonly index: number;
  readonly tau: number;
  readonly singleScatteringAlbedo: number;
  readonly asymmetry: number;
  readonly groundAlbedo: number;
  readonly vroom: boolean;
  readonly class: SweepPointClass;
  /** Its final trace, or `null` for a point the point cap stopped, which holds its pilot alone. */
  readonly traced: SweepTraced | null;
}

/** A sweep output, as far as the recompute reads it. */
export interface SweepOutput {
  readonly case: string;
  readonly caseSha256: string;
  readonly binarySha256: string;
  readonly minFinalSamples: number;
  readonly countMargin: number;
  readonly geometries: ReadonlyArray<string>;
  readonly cells: ReadonlyArray<SweepCell>;
  readonly points: ReadonlyArray<SweepOutputPoint>;
}

/** Whether a value is a plain record of fields. */
function isFields(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** A record of fields, or `undefined`. */
function fieldsOf(value: unknown): Readonly<Record<string, unknown>> | undefined {
  return isFields(value) ? value : undefined;
}

/** A narrowing failure, naming the file and the path. */
function shapeError(path: string, what: string): Error {
  return new Error(`${path} is not ${what}`);
}

/** The field `key` of the record at `path`. */
function fieldAt(value: unknown, key: string, path: string): unknown {
  const fields = fieldsOf(value);
  if (fields === undefined) {
    throw shapeError(path, "a record");
  }
  return fields[key];
}

function numberAt(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw shapeError(path, "a finite number");
  }
  return value;
}

function integerAt(value: unknown, path: string): number {
  const n = numberAt(value, path);
  if (!Number.isInteger(n) || n < 0) {
    throw shapeError(path, "a count");
  }
  return n;
}

function stringAt(value: unknown, path: string): string {
  if (typeof value !== "string") {
    throw shapeError(path, "a string");
  }
  return value;
}

function booleanAt(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") {
    throw shapeError(path, "a boolean");
  }
  return value;
}

function arrayAt(value: unknown, path: string): ReadonlyArray<unknown> {
  if (!Array.isArray(value)) {
    throw shapeError(path, "an array");
  }
  const items: ReadonlyArray<unknown> = value;
  return items;
}

function listAt<T>(value: unknown, path: string, each: (item: unknown, at: string) => T): T[] {
  return arrayAt(value, path).map((item, i) => each(item, `${path}[${i}]`));
}

function oneOf<T extends string>(value: unknown, options: ReadonlyArray<T>, path: string): T {
  const found = options.find((option) => option === value);
  if (found === undefined) {
    throw shapeError(path, `one of ${options.join(", ")}`);
  }
  return found;
}

function directionAt(value: unknown, path: string): SweepDirection {
  return {
    zenithDeg: numberAt(fieldAt(value, "zenithDeg", path), `${path}.zenithDeg`),
    azimuthDeg: numberAt(fieldAt(value, "azimuthDeg", path), `${path}.azimuthDeg`),
  };
}

/**
 * A sweep case narrowed from its JSON (the module's remarks).
 *
 * @param file - The file's name, for the errors.
 * @throws Error naming the path of a field of the wrong shape.
 */
export function readSweepCase(value: unknown, file: string): SweepCase {
  const at = (key: string): unknown => fieldAt(value, key, file);
  const numbers = (item: unknown, path: string): number => numberAt(item, path);
  return {
    name: stringAt(at("name"), `${file}.name`),
    singleScatteringAlbedo: numberAt(
      at("singleScatteringAlbedo"),
      `${file}.singleScatteringAlbedo`,
    ),
    groundAlbedo: numberAt(
      fieldAt(at("template"), "groundAlbedo", `${file}.template`),
      `${file}.template.groundAlbedo`,
    ),
    tau: listAt(at("tau"), `${file}.tau`, numbers),
    tauRoles: listAt(at("tauRoles"), `${file}.tauRoles`, (item, path) =>
      oneOf(item, ["floor", "octave", "half-octave"] as const, path),
    ),
    geometries: listAt(at("geometries"), `${file}.geometries`, (item, path) => ({
      name: stringAt(fieldAt(item, "name", path), `${path}.name`),
      image: stringAt(fieldAt(item, "image", path), `${path}.image`),
      kind: oneOf(
        fieldAt(item, "kind", path),
        ["ground", "disc", "limb", "aerial"] as const,
        `${path}.kind`,
      ),
      view: directionAt(fieldAt(item, "view", path), `${path}.view`),
      sun: directionAt(fieldAt(item, "sun", path), `${path}.sun`),
      groundConeShare: numberAt(fieldAt(item, "groundConeShare", path), `${path}.groundConeShare`),
    })),
    scanGroundAlbedos: listAt(at("scanGroundAlbedos"), `${file}.scanGroundAlbedos`, numbers),
    columns: listAt(at("columns"), `${file}.columns`, (item, path) => {
      const rows = (key: string): number[][] =>
        listAt(fieldAt(item, key, path), `${path}.${key}`, (row, rowPath) =>
          listAt(row, rowPath, numbers),
        );
      return {
        asymmetry: numberAt(fieldAt(item, "asymmetry", path), `${path}.asymmetry`),
        points: listAt(fieldAt(item, "points", path), `${path}.points`, integerAt),
        twin: rows("twin"),
        scanPoints: listAt(fieldAt(item, "scanPoints", path), `${path}.scanPoints`, integerAt),
        scanTwin: rows("scanTwin"),
      };
    }),
  };
}

function walkPastAt(value: unknown, path: string): SweepWalkPast {
  const kind = oneOf(
    fieldAt(value, "kind", path),
    ["no", "through", "top"] as const,
    `${path}.kind`,
  );
  let past: SweepWalkPast;
  switch (kind) {
    case "no":
    case "top":
      past = { kind };
      break;
    case "through":
      past = { kind, tau: numberAt(fieldAt(value, "tau", path), `${path}.tau`) };
      break;
  }
  return past;
}

function cellAt(value: unknown, path: string): SweepCell {
  const at = (key: string): unknown => fieldAt(value, key, path);
  const names = (key: string): string[] =>
    listAt(at(key), `${path}.${key}`, (item, itemPath) => stringAt(item, itemPath));
  return {
    singleScatteringAlbedo: numberAt(
      at("singleScatteringAlbedo"),
      `${path}.singleScatteringAlbedo`,
    ),
    asymmetry: numberAt(at("asymmetry"), `${path}.asymmetry`),
    groundAlbedo: numberAt(at("groundAlbedo"), `${path}.groundAlbedo`),
    vroom: booleanAt(at("vroom"), `${path}.vroom`),
    walkPast: walkPastAt(at("walkPast"), `${path}.walkPast`),
    kind: oneOf(at("kind"), ["drift", "reach", "top"] as const, `${path}.kind`),
    tauStar: numberAt(at("tauStar"), `${path}.tauStar`),
    points: listAt(at("points"), `${path}.points`, integerAt),
    floorMisses: names("floorMisses"),
    listedRefusals: listAt(at("listedRefusals"), `${path}.listedRefusals`, (item, itemPath) =>
      stringAt(fieldAt(item, "geometry", itemPath), `${itemPath}.geometry`),
    ),
    scanPoints: listAt(at("scanPoints"), `${path}.scanPoints`, integerAt),
    fixedMisses: names("fixedMisses"),
    groundMisses: listAt(
      at("groundMisses"),
      `${path}.groundMisses`,
      (item, itemPath): readonly [string, number] => [
        stringAt(fieldAt(item, "geometry", itemPath), `${itemPath}.geometry`),
        numberAt(fieldAt(item, "groundAlbedo", itemPath), `${itemPath}.groundAlbedo`),
      ],
    ),
    unsortedMisses: names("unsortedMisses"),
    albedoStar: numberAt(at("albedoStar"), `${path}.albedoStar`),
  };
}

function tracedAt(value: unknown, path: string): SweepTraced {
  const at = (key: string): unknown => fieldAt(value, key, path);
  const numbers = (key: string): number[] =>
    listAt(at(key), `${path}.${key}`, (item, itemPath) => numberAt(item, itemPath));
  return {
    samples: listAt(at("samples"), `${path}.samples`, integerAt),
    doublings: listAt(at("doublings"), `${path}.doublings`, integerAt),
    radiance: numbers("radiance"),
    standardError: numbers("standardError"),
    quarterStandardError: numbers("quarterStandardError"),
    twin: numbers("twin"),
    failing: listAt(at("failing"), `${path}.failing`, integerAt),
    refused: listAt(at("refused"), `${path}.refused`, (item, itemPath) => ({
      geometry: integerAt(fieldAt(item, "geometry", itemPath), `${itemPath}.geometry`),
      refusal: oneOf(
        fieldAt(item, "refusal", itemPath),
        ["noisy", "unscaled"] as const,
        `${itemPath}.refusal`,
      ),
      listed: booleanAt(fieldAt(item, "listed", itemPath), `${itemPath}.listed`),
      cap: oneOf(fieldAt(item, "cap", itemPath), ["geometry", "point"] as const, `${itemPath}.cap`),
    })),
  };
}

function pointAt(value: unknown, path: string): SweepOutputPoint {
  const at = (key: string): unknown => fieldAt(value, key, path);
  const traced = at("traced");
  return {
    index: integerAt(at("index"), `${path}.index`),
    tau: numberAt(at("tau"), `${path}.tau`),
    singleScatteringAlbedo: numberAt(
      at("singleScatteringAlbedo"),
      `${path}.singleScatteringAlbedo`,
    ),
    asymmetry: numberAt(at("asymmetry"), `${path}.asymmetry`),
    groundAlbedo: numberAt(at("groundAlbedo"), `${path}.groundAlbedo`),
    vroom: booleanAt(at("vroom"), `${path}.vroom`),
    class: oneOf(at("class"), ["pass", "drift", "reach", "scan"] as const, `${path}.class`),
    traced: traced === undefined ? null : tracedAt(traced, `${path}.traced`),
  };
}

/**
 * A sweep output narrowed from its JSON (the module's remarks).
 *
 * @param file - The file's name, for the errors.
 * @throws Error naming the path of a field of the wrong shape, or for an output of another format
 *   or kind.
 */
export function readSweepOutput(value: unknown, file: string): SweepOutput {
  const at = (key: string): unknown => fieldAt(value, key, file);
  if (at("format") !== 1 || at("kind") !== "sweep") {
    throw shapeError(file, "a sweep output of format 1");
  }
  const settings = at("settings");
  return {
    case: stringAt(at("case"), `${file}.case`),
    caseSha256: stringAt(at("caseSha256"), `${file}.caseSha256`),
    binarySha256: stringAt(at("binarySha256"), `${file}.binarySha256`),
    minFinalSamples: integerAt(
      fieldAt(settings, "minFinalSamples", `${file}.settings`),
      `${file}.settings.minFinalSamples`,
    ),
    countMargin: numberAt(
      fieldAt(settings, "countMargin", `${file}.settings`),
      `${file}.settings.countMargin`,
    ),
    geometries: listAt(at("geometries"), `${file}.geometries`, (item, path) =>
      stringAt(item, path),
    ),
    cells: listAt(at("cells"), `${file}.cells`, cellAt),
    points: listAt(at("points"), `${file}.points`, pointAt),
  };
}

// --- The verdicts --------------------------------------------------------------------------------

/** A point of a column: a τ candidate of the walk, or the floor over a scan ground. */
export type RecomputeSpot =
  | { readonly kind: "walk"; readonly candidate: number }
  | { readonly kind: "scan"; readonly ground: number };

/** A refused geometry as the recompute finds it. */
export interface RecomputedRefusal {
  readonly geometry: number;
  readonly refusal: RefusalReason;
  readonly listed: boolean;
}

/** A traced point's verdicts by `gate.ts`, against the walk's. */
export interface RecomputedPoint {
  readonly file: string;
  readonly column: number;
  readonly spot: RecomputeSpot;
  readonly point: SweepOutputPoint;
  /** The geometries that fail where the reference is not refused, ascending. */
  readonly failing: ReadonlyArray<number>;
  readonly refused: ReadonlyArray<RecomputedRefusal>;
  /** Each geometry's comparison in the sweep's channel, in the case's order. */
  readonly comparisons: ReadonlyArray<Comparison>;
  /** The case's twin row the comparison read. */
  readonly twin: ReadonlyArray<number>;
  /** Whether the failing geometries are the walk's. */
  readonly failingAgrees: boolean;
  /** Whether the refused geometries, their reasons and listings, are the walk's. */
  readonly refusedAgrees: boolean;
  /** The geometries whose echoed twin is not the case's to seven significant figures. */
  readonly echoDiffers: ReadonlyArray<number>;
}

/** A unit vector of a local direction: (sin θ cos φ, sin θ sin φ, cos θ). */
function unitOf(direction: SweepDirection): readonly [number, number, number] {
  const zenith = (direction.zenithDeg * Math.PI) / 180;
  const azimuth = (direction.azimuthDeg * Math.PI) / 180;
  const s = Math.sin(zenith);
  return [s * Math.cos(azimuth), s * Math.sin(azimuth), Math.cos(zenith)];
}

/** Whether a geometry's view lies within {@link NEAR_SUN_DEG}° of its sun, `sweep.rs`'s `near_sun`. */
export function nearSun(geometry: SweepCaseGeometry): boolean {
  const v = unitOf(geometry.view);
  const s = unitOf(geometry.sun);
  const cos = v[0] * s[0] + v[1] * s[1] + v[2] * s[2];
  return cos >= Math.cos((NEAR_SUN_DEG * Math.PI) / 180) - 1e-12;
}

/** A value as the output writes it: seven significant figures, Rust's `{:.6e}`. */
function asWritten(value: number): number {
  return Number(value.toExponential(6));
}

/** The case's twin row of a column's spot. */
function twinRowOf(
  sweepCase: SweepCase,
  column: number,
  spot: RecomputeSpot,
): ReadonlyArray<number> {
  const col = sweepCase.columns[column];
  const row = spot.kind === "walk" ? col?.twin[spot.candidate] : col?.scanTwin[spot.ground];
  if (row?.length !== sweepCase.geometries.length) {
    throw new Error(`${sweepCase.name} has no twin row of column ${column} at ${spot.kind}`);
  }
  return row;
}

/** A refusal's key, to compare two sets of refusals: its geometry, reason and listing. */
function refusalKey(r: RecomputedRefusal): string {
  return `${r.geometry}/${r.refusal}/${r.listed}`;
}

/**
 * A traced point's verdicts by `gate.ts` (the module's remarks).
 *
 * @throws Error for a point whose per-geometry arrays do not match the case's geometries.
 */
export function recomputePoint(
  sweepCase: SweepCase,
  file: string,
  column: number,
  spot: RecomputeSpot,
  point: SweepOutputPoint,
  traced: SweepTraced,
): RecomputedPoint {
  const { geometries } = sweepCase;
  const twin = twinRowOf(sweepCase, column, spot);
  const each = (values: ReadonlyArray<number>, name: string): ReadonlyArray<number> => {
    if (values.length !== geometries.length) {
      throw new Error(`${file} point ${point.index}'s ${name} has ${values.length} geometries`);
    }
    return values;
  };
  const radiance = each(traced.radiance, "radiance");
  const sigma = each(traced.standardError, "standardError");
  const quarter = each(traced.quarterStandardError, "quarterStandardError");
  const samples = each(traced.samples, "samples");
  const readings: ReferenceReading[] = geometries.map((geometry, g) => {
    const l = radiance[g] ?? Number.NaN;
    const s = sigma[g] ?? Number.NaN;
    const q = quarter[g] ?? Number.NaN;
    const n = samples[g] ?? Number.NaN;
    return {
      name: geometry.name,
      image: geometry.image,
      radiance: [l, l, l],
      standardError: [s, s, s],
      samples: n,
      quarterSamples: Math.floor(n / 4),
      quarterStandardError: [q, q, q],
    };
  });
  const client = new Map(
    geometries.map((geometry, g) => {
      const t = twin[g] ?? Number.NaN;
      return [geometry.name, [t, t, t] as const];
    }),
  );
  const report = gate(readings, client);
  const comparisons = report.comparisons.filter((c) => c.channel === SWEEP_CHANNEL);
  const indexOf = new Map(geometries.map((geometry, g) => [geometry.name, g]));
  const failing = comparisons.flatMap((c, g) => (!c.passes && !c.refused ? [g] : []));
  const refused = report.refusals
    .filter((r) => r.channel === SWEEP_CHANNEL)
    .map((r): RecomputedRefusal => {
      const g = indexOf.get(r.name) ?? -1;
      const geometry = geometries[g];
      if (geometry === undefined) {
        throw new Error(`${file} refuses a geometry it does not hold: ${r.name}`);
      }
      return { geometry: g, refusal: r.reason, listed: point.vroom && nearSun(geometry) };
    });
  const walked = traced.failing.toSorted((a, b) => a - b);
  const echo = each(traced.twin, "twin");
  return {
    file,
    column,
    spot,
    point,
    failing,
    refused,
    comparisons,
    twin,
    failingAgrees: walked.length === failing.length && walked.every((g, i) => g === failing[i]),
    refusedAgrees:
      refused.map(refusalKey).toSorted().join() ===
      traced.refused.map(refusalKey).toSorted().join(),
    echoDiffers: twin.flatMap((t, g) => (asWritten(t) === echo[g] ? [] : [g])),
  };
}

// --- The walk, `sweep.rs`'s rules ----------------------------------------------------------------

/**
 * A point's outcome as the walk reads it (`sweep.rs`'s `PointOutcome`): traced, with the geometries
 * that fail and those refused; over the point cap, piloted alone; or, in the recompute alone, a
 * point the output does not hold, which the walk takes as a stop and the record as a finding.
 */
export type SweepOutcome =
  | {
      readonly kind: "traced";
      readonly failing: ReadonlyArray<number>;
      readonly refused: ReadonlyArray<RecomputedRefusal>;
    }
  | { readonly kind: "overCap" }
  | { readonly kind: "untraced" };

/** A point's class in its column's walk (`sweep.rs`'s `PointClass`). */
export type WalkClass = "pass" | "drift" | "reach";

/** `sweep.rs`'s `classify`: a point's class given its column's floor misses. */
function classify(outcome: SweepOutcome, floorMisses: ReadonlySet<number>): WalkClass {
  let found: WalkClass;
  switch (outcome.kind) {
    case "overCap":
    case "untraced":
      found = "reach";
      break;
    case "traced":
      if (outcome.failing.some((g) => !floorMisses.has(g))) {
        found = "drift";
      } else {
        found = outcome.refused.some((r) => !r.listed) ? "reach" : "pass";
      }
      break;
  }
  return found;
}

/** A column's walk as the recompute takes it. */
export interface ColumnWalk {
  readonly visited: ReadonlyArray<{ readonly candidate: number; readonly class: WalkClass }>;
  readonly kind: BoundaryKind;
  readonly tauStar: number;
  readonly floorMisses: ReadonlySet<number>;
  readonly listed: ReadonlyArray<{ readonly candidate: number; readonly geometry: number }>;
  /** The candidates the walk asks for that the output does not hold. */
  readonly untraced: ReadonlyArray<number>;
}

/**
 * `sweep.rs`'s `walk_column` (the module's remarks), `evaluate(candidate)` the point's outcome:
 * the floor, the octaves in turn to the first stop and on past a drift as `past` says, then the
 * half-octave below that stop; τ\* the least drift's τ, else the last pass's at a stop (`reach`),
 * else the last point's (`top`).
 */
export function walkSweepColumn(
  taus: ReadonlyArray<number>,
  roles: ReadonlyArray<SweepTauRole>,
  past: SweepWalkPast,
  evaluate: (candidate: number) => SweepOutcome,
): ColumnWalk {
  const visited: Array<{ readonly candidate: number; readonly class: WalkClass }> = [];
  const listed: Array<{ readonly candidate: number; readonly geometry: number }> = [];
  const untraced: number[] = [];
  let floorMisses: ReadonlySet<number> = new Set();
  const visit = (candidate: number): WalkClass => {
    const outcome = evaluate(candidate);
    if (outcome.kind === "traced") {
      if (roles[candidate] === "floor") {
        floorMisses = new Set(outcome.failing);
      }
      for (const r of outcome.refused) {
        if (r.listed) {
          listed.push({ candidate, geometry: r.geometry });
        }
      }
    } else if (outcome.kind === "untraced") {
      untraced.push(candidate);
    }
    const found = classify(outcome, floorMisses);
    visited.push({ candidate, class: found });
    return found;
  };
  const climb = [0, ...roles.flatMap((role, k) => (k > 0 && role === "octave" ? [k] : []))];
  let stop: number | undefined;
  let drifted = false;
  for (const candidate of climb) {
    const found = visit(candidate);
    if (found !== "pass" && stop === undefined) {
      stop = candidate;
    }
    const tau = taus[candidate] ?? Number.NaN;
    let goesOn: boolean;
    switch (found) {
      case "pass":
        goesOn = past.kind === "through" ? !drifted || tau < past.tau : true;
        break;
      case "drift":
        drifted = true;
        goesOn = past.kind === "through" ? tau < past.tau : past.kind === "top";
        break;
      case "reach":
        goesOn = false;
        break;
    }
    if (!goesOn) {
      break;
    }
  }
  if (stop !== undefined && stop > 0 && roles[stop - 1] === "half-octave") {
    visit(stop - 1);
  }
  const tausOf = (wanted: WalkClass): number[] =>
    visited.filter((v) => v.class === wanted).map((v) => taus[v.candidate] ?? Number.NaN);
  const drifts = tausOf("drift");
  let kind: BoundaryKind;
  let tauStar: number;
  if (drifts.length > 0) {
    kind = "drift";
    tauStar = Math.min(...drifts);
  } else if (stop !== undefined) {
    kind = "reach";
    const passes = tausOf("pass");
    tauStar = passes.length > 0 ? Math.max(...passes) : 0;
  } else {
    kind = "top";
    const last = visited.at(-1);
    tauStar = last === undefined ? 0 : (taus[last.candidate] ?? Number.NaN);
  }
  return { visited, kind, tauStar, floorMisses, listed, untraced };
}

/** A column's floor misses sorted by the scan (`sweep.rs`'s `FloorSort`). */
export interface FloorSort {
  readonly fixed: ReadonlyArray<number>;
  /** Each ground miss with the least albedo it fails over, by geometry. */
  readonly ground: ReadonlyMap<number, number>;
  readonly unsorted: ReadonlyArray<number>;
  readonly albedoStar: number;
}

/**
 * `sweep.rs`'s `sort_floor` (the module's remarks): `black` the floor over the black ground,
 * `brighter` its other points with their grounds. A geometry that fails over the black is fixed;
 * one that passes there and fails over a brighter ground is a ground miss, at the least albedo it
 * fails over; one refused or not traced over the black is unsorted. A\* is the least ground miss's
 * albedo, or 1.
 */
export function sortSweepFloor(
  black: SweepOutcome,
  brighter: ReadonlyArray<readonly [number, SweepOutcome]>,
): FloorSort {
  const blackVerdicts = black.kind === "traced" ? black : null;
  const fixed = blackVerdicts === null ? [] : [...blackVerdicts.failing];
  const ground = new Map<number, number>();
  const unsorted = new Set<number>();
  for (const [albedo, outcome] of brighter) {
    if (outcome.kind !== "traced" || !(albedo > 0)) {
      continue;
    }
    for (const g of outcome.failing) {
      if (blackVerdicts?.failing.includes(g) === true) {
        continue;
      }
      if (blackVerdicts !== null && !blackVerdicts.refused.some((r) => r.geometry === g)) {
        ground.set(g, Math.min(ground.get(g) ?? albedo, albedo));
      } else {
        unsorted.add(g);
      }
    }
  }
  const sortedGround = new Map([...ground.entries()].toSorted(([a], [b]) => a - b));
  return {
    fixed,
    ground: sortedGround,
    unsorted: [...unsorted].toSorted((a, b) => a - b),
    albedoStar: Math.min(1, ...ground.values()),
  };
}

// --- The recompute -------------------------------------------------------------------------------

/** One file's output and case, by name. */
export interface SweepFilePair {
  /** The file's name, `ms-sweep-w950`. */
  readonly name: string;
  readonly output: SweepOutput;
  readonly sweepCase: SweepCase;
}

/** One cell recomputed, beside the walk's. */
export interface RecomputedCell {
  readonly file: string;
  readonly column: number;
  readonly omega: number;
  readonly g: number;
  /** The walk's ground: 0.3 for the grid, 0.8 for the bright column. */
  readonly groundAlbedo: number;
  readonly vroom: boolean;
  readonly visited: ReadonlyArray<{ readonly candidate: number; readonly class: WalkClass }>;
  readonly kind: BoundaryKind;
  /** τ\*, its exact grid value. */
  readonly tauStar: number;
  /** The candidate τ\* stands at, for a `drift` or a `reach` above 0. */
  readonly tauStarCandidate: number | null;
  readonly floorMisses: ReadonlyArray<number>;
  readonly listed: ReadonlyArray<{ readonly candidate: number; readonly geometry: number }>;
  readonly fixed: ReadonlyArray<number>;
  readonly ground: ReadonlyMap<number, number>;
  readonly unsorted: ReadonlyArray<number>;
  readonly albedoStar: number;
  readonly untraced: ReadonlyArray<number>;
  /** The walk's cell. */
  readonly walked: SweepCell;
  /** The walk's τ\*, its exact grid value: the candidate whose τ it rounds. */
  readonly walkedTauStar: number;
  /** The fields in which the recompute and the walk differ, by name. */
  readonly disagreements: ReadonlyArray<string>;
  /** Each point of the column, walk and scan, by its spot, recomputed where it is traced. */
  readonly points: ReadonlyArray<RecomputedPoint>;
}

/** The whole recompute. */
export interface SweepRecompute {
  readonly files: ReadonlyArray<SweepFilePair>;
  readonly cells: ReadonlyArray<RecomputedCell>;
}

/** Whether two lists of names are equal in order. */
function sameNames(a: ReadonlyArray<string>, b: ReadonlyArray<string>): boolean {
  return a.length === b.length && a.every((name, i) => name === b[i]);
}

/**
 * Every cell of the files recomputed from their traced points with `gate.ts` (the module's
 * remarks), each held against the walk's.
 *
 * @throws Error for an output whose case is not its pair's, or whose geometries are not its
 *   case's, or a cell the case has no column for.
 */
export function recomputeSweep(files: ReadonlyArray<SweepFilePair>): SweepRecompute {
  const cells: RecomputedCell[] = [];
  for (const { name, output, sweepCase } of files) {
    if (output.case !== sweepCase.name) {
      throw new Error(`${name}'s output is of ${output.case}, not ${sweepCase.name}`);
    }
    if (
      !sameNames(
        output.geometries,
        sweepCase.geometries.map((g) => g.name),
      )
    ) {
      throw new Error(`${name}'s output lists other geometries than its case`);
    }
    const byIndex = new Map(output.points.map((p) => [p.index, p]));
    const names = (indices: Iterable<number>): string[] =>
      [...indices].map((g) => sweepCase.geometries[g]?.name ?? `#${g}`);
    for (const [column, walked] of output.cells.entries()) {
      const col = sweepCase.columns[column];
      if (col?.asymmetry !== walked.asymmetry) {
        throw new Error(`${name}'s cell ${column} is not its case's column`);
      }
      const recomputed: RecomputedPoint[] = [];
      const outcomeAt = (spot: RecomputeSpot, index: number | undefined): SweepOutcome => {
        const point = index === undefined ? undefined : byIndex.get(index);
        if (point === undefined) {
          return { kind: "untraced" };
        }
        if (point.traced === null) {
          return { kind: "overCap" };
        }
        const verdicts = recomputePoint(sweepCase, name, column, spot, point, point.traced);
        recomputed.push(verdicts);
        return { kind: "traced", failing: verdicts.failing, refused: verdicts.refused };
      };
      const walk = walkSweepColumn(
        sweepCase.tau,
        sweepCase.tauRoles,
        walked.walkPast,
        (candidate) => outcomeAt({ kind: "walk", candidate }, col.points[candidate]),
      );
      const scan = sweepCase.scanGroundAlbedos.map((_, ground) =>
        outcomeAt({ kind: "scan", ground }, col.scanPoints[ground]),
      );
      const floor = recomputed.find((p) => p.spot.kind === "walk" && p.spot.candidate === 0);
      const untraced: SweepOutcome = { kind: "untraced" };
      const brighter: Array<readonly [number, SweepOutcome]> = [
        ...sweepCase.scanGroundAlbedos
          .map((albedo, a): readonly [number, SweepOutcome] => [albedo, scan[a] ?? untraced])
          .slice(1),
        [
          sweepCase.groundAlbedo,
          floor === undefined
            ? untraced
            : { kind: "traced", failing: floor.failing, refused: floor.refused },
        ],
      ];
      const sort = sortSweepFloor(scan[0] ?? untraced, brighter);
      const walkedTauStarCandidate = sweepCase.tau.findIndex(
        (t) => asWritten(t) === walked.tauStar,
      );
      const walkedTauStar =
        walkedTauStarCandidate < 0
          ? walked.tauStar
          : (sweepCase.tau[walkedTauStarCandidate] ?? Number.NaN);
      const tauStarCandidate = sweepCase.tau.findIndex((t) => t === walk.tauStar);
      const disagreements: string[] = [];
      const check = (field: string, agrees: boolean): void => {
        if (!agrees) {
          disagreements.push(field);
        }
      };
      check("kind", walk.kind === walked.kind);
      check("tauStar", walk.tauStar === walkedTauStar);
      check("albedoStar", sort.albedoStar === walked.albedoStar);
      check(
        "points",
        walk.visited.length === walked.points.length &&
          walk.visited.every((v, i) => col.points[v.candidate] === walked.points[i]),
      );
      check("floorMisses", sameNames(names(walk.floorMisses), walked.floorMisses));
      check("fixedMisses", sameNames(names(sort.fixed), walked.fixedMisses));
      check(
        "groundMisses",
        sort.ground.size === walked.groundMisses.length &&
          [...sort.ground.entries()].every(([g, albedo], i) => {
            const [walkedName, walkedAlbedo] = walked.groundMisses[i] ?? ["", Number.NaN];
            return sweepCase.geometries[g]?.name === walkedName && albedo === walkedAlbedo;
          }),
      );
      check("unsortedMisses", sameNames(names(sort.unsorted), walked.unsortedMisses));
      check(
        "listedRefusals",
        sameNames(
          walk.listed.map((l) => sweepCase.geometries[l.geometry]?.name ?? ""),
          walked.listedRefusals,
        ),
      );
      check("untraced", walk.untraced.length === 0);
      check(
        "pointVerdicts",
        recomputed.every((p) => p.failingAgrees && p.refusedAgrees),
      );
      check(
        "twinEcho",
        recomputed.every((p) => p.echoDiffers.length === 0),
      );
      cells.push({
        file: name,
        column,
        omega: sweepCase.singleScatteringAlbedo,
        g: walked.asymmetry,
        groundAlbedo: sweepCase.groundAlbedo,
        vroom: walked.vroom,
        visited: walk.visited,
        kind: walk.kind,
        tauStar: walk.tauStar,
        tauStarCandidate: tauStarCandidate < 0 ? null : tauStarCandidate,
        floorMisses: [...walk.floorMisses].toSorted((a, b) => a - b),
        listed: walk.listed,
        fixed: sort.fixed,
        ground: sort.ground,
        unsorted: sort.unsorted,
        albedoStar: sort.albedoStar,
        untraced: walk.untraced,
        walked,
        walkedTauStar,
        disagreements,
        points: recomputed,
      });
    }
  }
  return { files, cells };
}

/** The grid's cells, over the walk's ground, in the boundary's order (ω descending, then g). */
export function gridCells(recompute: SweepRecompute): RecomputedCell[] {
  return recompute.cells
    .filter((c) => c.groundAlbedo === SWEEP_WALK_GROUND_ALBEDO)
    .toSorted((a, b) => b.omega - a.omega || a.g - b.g);
}

/**
 * The boundary the recompute sets (ruling 6): each grid cell's τ\* the lower of the recompute's and
 * the walk's, with that one's kind, and its A\* the recompute's.
 *
 * @throws Error for a grid that is not whole, its cells not every (ω, g) of its rows and columns.
 */
export function boundaryOfRecompute(recompute: SweepRecompute): ThickBoundary {
  const cells = gridCells(recompute);
  const omega = [...new Set(cells.map((c) => c.omega))];
  const g = [...new Set(cells.map((c) => c.g))];
  const rowMajor = cells.every(
    (c, k) => c.omega === omega[Math.floor(k / g.length)] && c.g === g[k % g.length],
  );
  const ascending = g.every((x, i) => i === 0 || x > (g[i - 1] ?? Number.NaN));
  if (omega.length * g.length !== cells.length || !rowMajor || !ascending) {
    throw new Error(`the sweep's grid is not whole: ${cells.length} cells`);
  }
  return {
    omega: Float64Array.from(omega),
    g: Float64Array.from(g),
    tauStar: Float64Array.from(cells, (c) => Math.min(c.tauStar, c.walkedTauStar)),
    albedoStar: Float64Array.from(cells, (c) => c.albedoStar),
    kind: cells.map((c) => (c.walkedTauStar < c.tauStar ? c.walked.kind : c.kind)),
  };
}

// --- The drift points and the raises -------------------------------------------------------------

/** A point of a cell, the walk's candidate `candidate`: its recompute, or `undefined`. */
export function walkPointOf(cell: RecomputedCell, candidate: number): RecomputedPoint | undefined {
  return cell.points.find((p) => p.spot.kind === "walk" && p.spot.candidate === candidate);
}

/** The τ candidates a cell traced in its walk, ascending in τ. */
export function tracedCandidates(cell: RecomputedCell): number[] {
  return cell.visited.map((v) => v.candidate).toSorted((a, b) => a - b);
}

/**
 * The raises' cells (`decision-r08-t13-scope.md`, ruling 12 as amended on 2026-10-27, after #112): Earth's six at ω 1, 0.99 and 0.95
 * and g 0 and 0.3, and Mars's six at ω 0.9 and 0.95 and g 0.5, 0.65 and 0.75, each at its drift
 * point and the next traced τ above it; and (1, 0) at every traced τ through 1.
 */
export const RAISE_CELLS: ReadonlyArray<{
  readonly body: "earth" | "mars" | "(1, 0)";
  readonly omega: number;
  readonly g: number;
}> = [
  ...[1, 0.99, 0.95].flatMap((omega) =>
    [0, 0.3].map((g) => ({ body: "earth" as const, omega, g })),
  ),
  ...[0.9, 0.95].flatMap((omega) =>
    [0.5, 0.65, 0.75].map((g) => ({ body: "mars" as const, omega, g })),
  ),
  { body: "(1, 0)", omega: 1, g: 0 },
];

/** A point the attribution reads: a cell's walk point. */
export interface RaisePoint {
  readonly body: "earth" | "mars" | "(1, 0)";
  readonly cell: RecomputedCell;
  readonly candidate: number;
  readonly point: RecomputedPoint;
}

/** The highest τ (1) through which (1, 0)'s attribution runs. */
const ONE_ZERO_THROUGH_TAU = 1;

/**
 * The raises' points ("#112's floor refusals", item 3) of the recompute's grid.
 *
 * @throws Error for a raise cell the grid does not hold, or a point it did not trace.
 */
export function raisePoints(recompute: SweepRecompute): RaisePoint[] {
  const cells = gridCells(recompute);
  const out: RaisePoint[] = [];
  for (const raise of RAISE_CELLS) {
    const cell = cells.find((c) => c.omega === raise.omega && c.g === raise.g);
    if (cell === undefined) {
      throw new Error(`the grid has no cell (${raise.omega}, ${raise.g})`);
    }
    const sweepCase = recompute.files.find((f) => f.name === cell.file)?.sweepCase;
    const tauOf = (k: number): number => sweepCase?.tau[k] ?? Number.NaN;
    const traced = tracedCandidates(cell);
    let candidates: number[];
    if (raise.body === "(1, 0)") {
      candidates = traced.filter((k) => tauOf(k) <= ONE_ZERO_THROUGH_TAU);
    } else {
      const drift = cell.tauStarCandidate;
      const above = traced.find((k) => drift !== null && tauOf(k) > tauOf(drift));
      candidates = [drift, above].flatMap((k) => (k === null || k === undefined ? [] : [k]));
    }
    for (const candidate of candidates) {
      const point = walkPointOf(cell, candidate);
      if (point === undefined) {
        throw new Error(`(${raise.omega}, ${raise.g}) traced no point at candidate ${candidate}`);
      }
      out.push({ body: raise.body, cell, candidate, point });
    }
  }
  return out;
}

// --- The twin's values ---------------------------------------------------------------------------

/**
 * A value the record reads from a twin of a sweep point, in the sweep's channel: the drawn
 * surface's part of its radiance (`surface`, `twin.ts`'s `surfaceRadiance`) or its radiance with
 * the multiple scattering zeroed (`zeroed`).
 */
export interface SweepTwinSpec {
  readonly quantity: "surface" | "zeroed";
  readonly file: string;
  readonly column: number;
  readonly candidate: number;
  readonly geometry: string;
}

/** A twin value's key in the record. */
export function sweepTwinKey(spec: SweepTwinSpec): string {
  return `${spec.file}/c${spec.column}/t${spec.candidate}/${spec.geometry}/${spec.quantity}`;
}

/** The twin's values, by key, each at seven significant figures. */
export type SweepTwinValues = ReadonlyMap<string, number>;

/** A drift cell's attribution points ("#107's twin ground", the drift cells' surface shares): its first failing geometries. */
export interface DriftAttribution {
  readonly cell: RecomputedCell;
  /** The drift point, at τ\*. */
  readonly point: RecomputedPoint;
  /** The lowest index among the drift point's failing geometries. */
  readonly firstFailing: number;
  /** The lowest index among them that is not a floor miss. */
  readonly firstNotFloorMiss: number;
}

/**
 * Each `drift` cell's first failing geometry and its first that is not a floor miss, at its drift
 * point (ruling 5's "first failing", the lowest index in the drift point's failing), the bright
 * column's among them.
 *
 * @throws Error for a drift cell whose drift point is not traced, or holds no drift.
 */
export function driftAttributions(recompute: SweepRecompute): DriftAttribution[] {
  return recompute.cells
    .filter((c) => c.kind === "drift")
    .map((cell) => {
      const point =
        cell.tauStarCandidate === null ? undefined : walkPointOf(cell, cell.tauStarCandidate);
      const firstFailing = point?.failing[0];
      const firstNotFloorMiss = point?.failing.find((g) => !cell.floorMisses.includes(g));
      if (point === undefined || firstFailing === undefined || firstNotFloorMiss === undefined) {
        throw new Error(`${cell.file}'s cell of g ${cell.g} has no drift point`);
      }
      return { cell, point, firstFailing, firstNotFloorMiss };
    });
}

/** Every twin value the record reads, each once, in the record's order. */
export function sweepTwinSpecs(recompute: SweepRecompute): SweepTwinSpec[] {
  const specs = new Map<string, SweepTwinSpec>();
  const add = (spec: SweepTwinSpec): void => {
    specs.set(sweepTwinKey(spec), spec);
  };
  const nameOf = (point: RecomputedPoint, g: number): string =>
    recompute.files.find((f) => f.name === point.file)?.sweepCase.geometries[g]?.name ?? `#${g}`;
  for (const drift of driftAttributions(recompute)) {
    for (const g of [drift.firstFailing, drift.firstNotFloorMiss]) {
      add({
        quantity: "surface",
        file: drift.cell.file,
        column: drift.cell.column,
        candidate: drift.cell.tauStarCandidate ?? -1,
        geometry: nameOf(drift.point, g),
      });
    }
  }
  for (const raise of raisePoints(recompute)) {
    for (const g of raise.point.failing) {
      for (const quantity of ["surface", "zeroed"] as const) {
        add({
          quantity,
          file: raise.cell.file,
          column: raise.cell.column,
          candidate: raise.candidate,
          geometry: nameOf(raise.point, g),
        });
      }
    }
  }
  return [...specs.values()];
}

/** The keys of `specs` that `values` lacks. */
export function missingSweepTwinValues(
  values: SweepTwinValues,
  specs: ReadonlyArray<SweepTwinSpec>,
): string[] {
  return specs.map(sweepTwinKey).filter((key) => !values.has(key));
}

/** The keys of the values that are not finite or are negative: ruling 12's blockers. */
export function blockingSweepTwinValues(values: SweepTwinValues): string[] {
  return [...values.entries()]
    .filter(([, value]) => !(Number.isFinite(value) && value >= 0))
    .map(([key]) => key);
}

/** A value of `values`, or a thrown error naming its key. */
function valueOf(values: SweepTwinValues, spec: SweepTwinSpec): number {
  const key = sweepTwinKey(spec);
  const value = values.get(key);
  if (value === undefined) {
    throw new Error(`the record reads a twin value it is not given: ${key}`);
  }
  return value;
}

// --- The classes ---------------------------------------------------------------------------------

/** A medium the record classifies, with the cover it is classified under. */
export interface ClassifiedMedium {
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly cover: BodyCover;
}

/** Which of the boundary's lines make a channel thick: `depth`, `ground`, both or neither. */
function lineOf(channel: ChannelRegime): string {
  if (channel.thickByDepth && channel.thickByGround) {
    return "depth and ground";
  }
  if (channel.thickByDepth) {
    return "depth";
  }
  return channel.thickByGround ? "ground" : "none";
}

/** A regime's name and its altitude, for a row. */
function regimeLabel(regime: AtmosphereRegime): string {
  let label: string;
  switch (regime.kind) {
    case "thin":
    case "thickScattering":
      label = regime.kind;
      break;
    case "cloudDeck":
      label = `cloudDeck at ${recorded(regime.deckAltitudeM)} m`;
      break;
  }
  return label;
}

/** The channels' letters. */
const CHANNEL_LETTERS = ["r", "g", "b"] as const;

/**
 * Each medium's class and its channels' readings (ruling 12 as amended): per channel τ, ω̄ and
 * ḡ after the similarity, τ\* and A\* read there, τ ÷ τ\*, the ground against A\*, the deciding
 * line and the clamps.
 */
function classRows(media: ReadonlyArray<ClassifiedMedium>): RecordValue {
  return media.map(({ name, medium, cover }) => {
    const channels = regimeChannels(medium);
    return {
      name,
      regime: regimeLabel(classifyRegime(medium, cover)),
      channels: channels.map((c, i) => [
        CHANNEL_LETTERS[i] ?? "?",
        recorded(c.opticalDepth),
        recorded(c.singleScatteringAlbedo),
        recorded(c.asymmetry),
        recorded(c.boundary.tauStar),
        recorded(c.boundary.albedoStar),
        recorded(c.opticalDepth / c.boundary.tauStar),
        recorded(c.groundAlbedo),
        lineOf(c),
        c.boundary.albedoBelowGrid,
        c.boundary.asymmetryOffGrid,
      ]),
    };
  });
}

/**
 * R08.T10.a's provisional thick line, retired by R08.T13.d (`decision-r08-t13c2-classes.md` §5,
 * item 4): a medium whose {@link verticalOpticalDepth} reaches 1 on its worst channel. A private
 * copy, kept only to write the record's `provisionalThick` column, so that `sweepBoundary.json`
 * stays byte for byte; no label reads it.
 */
function retiredProvisionalThick(medium: AtmosphereMedium): boolean {
  return Math.max(...verticalOpticalDepth(medium)) >= 1;
}

/**
 * The label changes for R08.T13.d (ruling 9's label changes): each medium's class by the retired
 * provisional line ({@link retiredProvisionalThick}) against `classifyRegime`, and whether its
 * label changes, every thick regime carrying `ATMOSPHERE: APPROXIMATE` as the provisional line did.
 */
function labelRows(media: ReadonlyArray<ClassifiedMedium>): RecordValue {
  return media.map(({ name, medium, cover }) => {
    const provisional = retiredProvisionalThick(medium);
    const regime = classifyRegime(medium, cover);
    const channels = regimeChannels(medium);
    const lines = channels.flatMap((c, i) =>
      lineOf(c) === "none" ? [] : [`${CHANNEL_LETTERS[i] ?? "?"}: ${lineOf(c)}`],
    );
    const thick = approximateRegime(regime);
    let line = lines.length > 0 ? lines.join("; ") : "none";
    if (regime.kind === "cloudDeck") {
      line = "the deck rule";
    }
    let label = "the same";
    if (provisional !== thick) {
      label = thick ? "gains APPROXIMATE" : "loses APPROXIMATE";
    }
    return [name, provisional ? "thick" : "thin", regimeLabel(regime), line, label];
  });
}

// --- The record ----------------------------------------------------------------------------------

/** The row columns of each table of the record, in order. */
export const SWEEP_BOUNDARY_COLUMNS = {
  cell: [
    "file",
    "omega",
    "g",
    "groundAlbedo",
    "kind",
    "tauStar",
    "albedoStar",
    "walkedKind",
    "walkedTauStar",
    "walkedAlbedoStar",
    "disagreements",
    "points",
    "floorMisses",
    "fixed",
    "ground",
    "unsorted",
    "listed",
  ],
  point: [
    "index",
    "file",
    "omega",
    "g",
    "tau",
    "groundAlbedo",
    "walkedClass",
    "class",
    "failing",
    "refused",
    "verdictsAgree",
    "echoAgrees",
    "toppedUp",
    "maxDoublings",
    "minSamples",
    "maxSamples",
  ],
  drift: [
    "file",
    "omega",
    "g",
    "tauStar",
    "failing",
    "failingMeetingTheGround",
    "firstFailing",
    "firstFailingSort",
    "firstFailingConeShare",
    "firstFailingSurfaceShare",
    "firstNotFloorMiss",
    "firstNotFloorMissKind",
    "firstNotFloorMissConeShare",
    "firstNotFloorMissSurfaceShare",
  ],
  raise: [
    "body",
    "omega",
    "g",
    "tau",
    "geometry",
    "kind",
    "sort",
    "coneShare",
    "reference",
    "sigmaOverReference",
    "twinOverReference",
    "boundRatio",
    "missPastBound",
    "missOverTwin",
    "surfaceShare",
    "multipleScatteringShare",
    "missAgainstShare",
    "attribution",
  ],
  channel: [
    "channel",
    "tau",
    "omegaBar",
    "gBar",
    "tauStar",
    "albedoStar",
    "tauOverTauStar",
    "groundAlbedo",
    "line",
    "omegaBelowGrid",
    "gOffGrid",
  ],
  label: ["medium", "provisionalThick", "classifyRegime", "line", "label"],
  heavyTail: [
    "index",
    "file",
    "tau",
    "groundAlbedo",
    "geometry",
    "doublings",
    "samples",
    "verdict",
  ],
  twin: ["key", "value"],
} as const;

/** What the record's head says of its writer and the twin. */
export interface SweepBoundaryHead {
  readonly writer: string;
  readonly twin: string;
}

/** The case gate's headlines the record sets beside the classes: c1's, from `caseGate.json`. */
export interface CaseGateSummary {
  /** Each case's headline, `pass` or `miss`, by name. */
  readonly headlines: Readonly<Record<string, RecordValue>>;
  /** `mars`'s ground views more than 15° from the sun: its verdict and tally. */
  readonly marsOffSun: RecordValue;
}

/**
 * The c1 headlines and `mars`'s off-sun section of a committed `caseGate.json`, narrowed; `null`
 * for each value it lacks.
 */
export function caseGateSummary(record: unknown): CaseGateSummary {
  const cases = fieldsOf(fieldsOf(record)?.["cases"]);
  const headlines: Record<string, RecordValue> = {};
  for (const [name, section] of Object.entries(cases ?? {})) {
    const headline = fieldsOf(section)?.["headline"];
    const all = fieldsOf(fieldsOf(section)?.["all"]);
    const count = (key: string): number | null => {
      const value = all?.[key];
      return typeof value === "number" ? value : null;
    };
    headlines[name] = [
      typeof headline === "string" ? headline : null,
      count("passes"),
      count("misses"),
      count("refusals"),
    ];
  }
  const offSun = fieldsOf(fieldsOf(record)?.["marsOffSun"]);
  const tally = fieldsOf(offSun?.["tally"]);
  const tallyCount = (key: string): number | null => {
    const value = tally?.[key];
    return typeof value === "number" ? value : null;
  };
  const verdict = offSun?.["verdict"];
  const views = offSun?.["views"];
  return {
    headlines,
    marsOffSun: {
      views: typeof views === "number" ? views : null,
      verdict: typeof verdict === "string" ? verdict : null,
      comparisons: tallyCount("comparisons"),
      passes: tallyCount("passes"),
      misses: tallyCount("misses"),
      refusals: tallyCount("refusals"),
    },
  };
}

/** The inputs of the record beside the recompute. */
export interface SweepBoundaryInputs {
  readonly twinValues: SweepTwinValues;
  /** The kept bodies, as traced (clear, with a ground beneath). */
  readonly bodies: ReadonlyArray<ClassifiedMedium>;
  /** Every medium whose label R08.T13.d may change. */
  readonly labelMedia: ReadonlyArray<ClassifiedMedium>;
  readonly caseGate: CaseGateSummary;
  readonly head: SweepBoundaryHead;
}

/** A twin value as the record keeps it: seven significant figures. */
export function recordedTwinValue(value: number): number {
  return recorded(value);
}

/** A geometry of a recomputed point's case, by index. */
function geometryOf(recompute: SweepRecompute, file: string, g: number): SweepCaseGeometry {
  const geometry = recompute.files.find((f) => f.name === file)?.sweepCase.geometries[g];
  if (geometry === undefined) {
    throw new Error(`${file} has no geometry ${g}`);
  }
  return geometry;
}

/** A τ candidate's value in a file's case. */
function tauOfCandidate(recompute: SweepRecompute, file: string, candidate: number): number {
  return recompute.files.find((f) => f.name === file)?.sweepCase.tau[candidate] ?? Number.NaN;
}

/** How a failing geometry of a cell sorts: a drift, or a floor miss by the scan's sort. */
function sortOf(cell: RecomputedCell, g: number): string {
  if (!cell.floorMisses.includes(g)) {
    return "drift";
  }
  if (cell.fixed.includes(g)) {
    return "floor miss, fixed";
  }
  if (cell.ground.has(g)) {
    return "floor miss, ground";
  }
  return cell.unsorted.includes(g) ? "floor miss, unsorted" : "floor miss";
}

/** The cells' rows. */
function cellRows(recompute: SweepRecompute): RecordValue {
  return recompute.cells.map((c) => {
    const nameOf = (g: number): string => geometryOf(recompute, c.file, g).name;
    return [
      c.file,
      c.omega,
      c.g,
      c.groundAlbedo,
      c.kind,
      recorded(c.tauStar),
      c.albedoStar,
      c.walked.kind,
      c.walked.tauStar,
      c.walked.albedoStar,
      c.disagreements.join(", "),
      c.visited
        .map((v) => `${recorded(tauOfCandidate(recompute, c.file, v.candidate))}:${v.class}`)
        .join(" "),
      c.floorMisses.length,
      c.fixed.map(nameOf).join(" "),
      [...c.ground.entries()].map(([g, a]) => `${nameOf(g)}:${a}`).join(" "),
      c.unsorted.map(nameOf).join(" "),
      c.listed.map((l) => nameOf(l.geometry)).join(" "),
    ];
  });
}

/** Each traced point's row: its verdicts against the walk's, and its top-ups. */
function pointRows(recompute: SweepRecompute): RecordValue {
  const rows = recompute.cells.flatMap((cell) =>
    cell.points.map((p) => {
      const traced = p.point.traced;
      const doublings = traced?.doublings ?? [];
      const samples = traced?.samples ?? [];
      const nameOf = (g: number): string => geometryOf(recompute, p.file, g).name;
      const visited =
        p.spot.kind === "walk"
          ? cell.visited.find((v) => p.spot.kind === "walk" && v.candidate === p.spot.candidate)
          : undefined;
      return {
        index: p.point.index,
        row: [
          p.point.index,
          p.file,
          p.point.singleScatteringAlbedo,
          p.point.asymmetry,
          p.point.tau,
          p.point.groundAlbedo,
          p.point.class,
          visited?.class ?? "scan",
          p.failing.length,
          p.refused.length,
          p.failingAgrees && p.refusedAgrees,
          p.echoDiffers.length === 0,
          doublings.flatMap((d, g) => (d > 0 ? [`${nameOf(g)}:${d}`] : [])).join(" "),
          Math.max(0, ...doublings),
          Math.min(...samples),
          Math.max(...samples),
        ],
      };
    }),
  );
  return rows.toSorted((a, b) => a.index - b.index).map((r) => r.row);
}

/** The geometries past {@link HEAVY_TAIL_DOUBLINGS} top-ups, each with its verdict. */
function heavyTailRows(recompute: SweepRecompute): RecordValue {
  return recompute.cells
    .flatMap((cell) => cell.points)
    .toSorted((a, b) => a.point.index - b.point.index)
    .flatMap((p) =>
      (p.point.traced?.doublings ?? []).flatMap((d, g) => {
        if (d < HEAVY_TAIL_DOUBLINGS) {
          return [];
        }
        const comparison = p.comparisons[g];
        let verdict = "pass";
        if (comparison?.refused === true) {
          verdict = "refused";
        } else if (comparison?.passes === false) {
          verdict = "miss";
        }
        return [
          [
            p.point.index,
            p.file,
            p.point.tau,
            p.point.groundAlbedo,
            geometryOf(recompute, p.file, g).name,
            d,
            p.point.traced?.samples[g] ?? null,
            verdict,
          ],
        ];
      }),
    );
}

/** The drift cells' rows (item 1). */
function driftRows(recompute: SweepRecompute, values: SweepTwinValues): RecordValue {
  return driftAttributions(recompute).map((d) => {
    const { cell, point } = d;
    const share = (g: number): number => {
      const geometry = geometryOf(recompute, cell.file, g);
      const surface = valueOf(values, {
        quantity: "surface",
        file: cell.file,
        column: cell.column,
        candidate: cell.tauStarCandidate ?? -1,
        geometry: geometry.name,
      });
      const radiance = point.twin[g] ?? Number.NaN;
      return radiance > 0 ? recorded(surface / radiance) : 0;
    };
    const meeting = point.failing.filter(
      (g) => geometryOf(recompute, cell.file, g).groundConeShare > 0,
    ).length;
    const first = geometryOf(recompute, cell.file, d.firstFailing);
    const notFloor = geometryOf(recompute, cell.file, d.firstNotFloorMiss);
    return [
      cell.file,
      cell.omega,
      cell.g,
      recorded(cell.tauStar),
      point.failing.length,
      meeting,
      first.name,
      sortOf(cell, d.firstFailing),
      recorded(first.groundConeShare),
      share(d.firstFailing),
      notFloor.name,
      notFloor.kind,
      recorded(notFloor.groundConeShare),
      share(d.firstNotFloorMiss),
    ];
  });
}

/** What "#112's floor refusals" item 3's comparison says of one miss. */
export interface MissAttribution {
  /** The twin's multiple-scattering share, (L_twin − L_zeroed) ÷ L_twin. */
  readonly share: number;
  /** The miss, |L_twin − L_ref| ÷ L_twin. */
  readonly miss: number;
  /** The miss |L_twin − L_ref| ÷ L_twin against the twin's multiple-scattering share: item 3's. */
  readonly againstShare: "larger" | "smaller";
  /**
   * What that can say, by the miss's sign: `kernels` where the twin is too bright by more than its
   * share, so that its single scattering alone outshines the reference; `undetermined` where it is
   * too dim by more than its share, the analytic term then short by more than its own size, which
   * nothing bounds, or the kernels; `analytic term` where the miss lies within the share, which
   * does not exclude it.
   */
  readonly attribution: "kernels" | "analytic term" | "undetermined";
}

/**
 * Item 3's attribution of a miss (`decision-r08-t13-scope.md`, "#112's floor refusals"): the twin
 * drawn L_twin = L₀ + M, L₀ its radiance with the multiple scattering zeroed and M ≥ 0 the
 * analytic term's part. A miss larger than M is the kernels' only where the twin is too bright: L₀
 * then exceeds the reference. Too dim, it says only that the reference's multiple scattering is
 * more than twice M (R08.T13.c2's science check).
 */
export function attributionOf(twin: number, reference: number, zeroed: number): MissAttribution {
  const share = twin > 0 ? (twin - zeroed) / twin : 0;
  const miss = twin > 0 ? Math.abs(twin - reference) / twin : Number.POSITIVE_INFINITY;
  if (!(miss > share)) {
    return { share, miss, againstShare: "smaller", attribution: "analytic term" };
  }
  const attribution = twin > reference ? "kernels" : "undetermined";
  return { share, miss, againstShare: "larger", attribution };
}

/**
 * The raises' rows (item 2): at each failing geometry of each raise point, its comparison, its
 * surface share and its multiple-scattering share, and item 3's attribution ({@link attributionOf}).
 */
function raiseRows(recompute: SweepRecompute, values: SweepTwinValues): RecordValue {
  return raisePoints(recompute).flatMap((raise) =>
    raise.point.failing.map((g) => {
      const { cell, point } = raise;
      const geometry = geometryOf(recompute, cell.file, g);
      const comparison = point.comparisons[g];
      if (comparison === undefined) {
        throw new Error(`${cell.file} has no comparison at ${geometry.name}`);
      }
      const spec = {
        file: cell.file,
        column: cell.column,
        candidate: raise.candidate,
        geometry: geometry.name,
      };
      const twin = comparison.client;
      const reference = comparison.reference;
      const surface = valueOf(values, { ...spec, quantity: "surface" });
      const zeroed = valueOf(values, { ...spec, quantity: "zeroed" });
      const delta = twin - reference;
      const verdict = attributionOf(twin, reference, zeroed);
      return [
        raise.body,
        cell.omega,
        cell.g,
        recorded(tauOfCandidate(recompute, cell.file, raise.candidate)),
        geometry.name,
        geometry.kind,
        sortOf(cell, g),
        recorded(geometry.groundConeShare),
        recorded(reference),
        recorded(comparison.standardError / reference),
        recorded(twin / reference),
        recorded(1 + (Math.sign(delta) * comparison.bound) / reference),
        recorded((Math.abs(delta) - comparison.bound) / reference),
        recorded(verdict.miss),
        twin > 0 ? recorded(surface / twin) : 0,
        recorded(verdict.share),
        verdict.againstShare,
        verdict.attribution,
      ];
    }),
  );
}

/**
 * The record (the module's remarks): a pure function of the recompute, the twin's values and the
 * classes.
 *
 * @throws Error for a twin value the record reads that it is not given.
 */
export function sweepBoundaryRecord(
  recompute: SweepRecompute,
  inputs: SweepBoundaryInputs,
): RecordValue {
  const { twinValues } = inputs;
  const specs = sweepTwinSpecs(recompute);
  const refusalsLeft = recompute.cells.flatMap((c) =>
    c.points.flatMap((p) =>
      (p.point.traced?.refused ?? []).map((r) => [
        p.point.index,
        geometryOf(recompute, p.file, r.geometry).name,
        r.refusal,
        r.cap,
      ]),
    ),
  );
  const recomputedRefusals = recompute.cells.flatMap((c) =>
    c.points.flatMap((p) =>
      p.refused.map((r) => [
        p.point.index,
        geometryOf(recompute, p.file, r.geometry).name,
        r.refusal,
      ]),
    ),
  );
  const disagreeing = recompute.cells.filter((c) => c.disagreements.length > 0);
  return {
    description:
      "R08.T13.c2's recompute of the multiple-scattering sweep with gate.ts itself, the boundary it sets, the attribution of its raises and the classes of the kept bodies (plan R08). Written by sweepBoundary.test.ts through sweepBoundary.ts; do not edit by hand.",
    format: SWEEP_BOUNDARY_FORMAT,
    writer: inputs.head.writer,
    twin: inputs.head.twin,
    significantFigures: RECORD_SIGNIFICANT_FIGURES,
    metric: {
      relativeTolerance: METRIC_RELATIVE_TOLERANCE,
      floor: METRIC_FLOOR,
      sigmaAllowance: METRIC_SIGMA_ALLOWANCE,
      sigmaDivisor: METRIC_SIGMA_DIVISOR,
      sigmaScalingTolerance: SIGMA_SCALING_TOLERANCE,
      nearSunDeg: NEAR_SUN_DEG,
      walkGroundAlbedo: SWEEP_WALK_GROUND_ALBEDO,
      heavyTailDoublings: HEAVY_TAIL_DOUBLINGS,
    },
    files: recompute.files.map((f) => [f.name, f.output.caseSha256, f.output.binarySha256]),
    columns: Object.fromEntries(
      Object.entries(SWEEP_BOUNDARY_COLUMNS).map(([key, names]) => [key, [...names]]),
    ),
    agreement: {
      cells: recompute.cells.length,
      agreeing: recompute.cells.length - disagreeing.length,
      disagreeing: disagreeing.map((c) => `${c.file}/g${c.g}: ${c.disagreements.join(", ")}`),
      refusalsLeft,
      recomputedRefusals,
    },
    cells: cellRows(recompute),
    drift: driftRows(recompute, twinValues),
    raises: raiseRows(recompute, twinValues),
    bodies: classRows(inputs.bodies),
    caseGate: { headlines: inputs.caseGate.headlines, marsOffSun: inputs.caseGate.marsOffSun },
    labels: labelRows(inputs.labelMedia),
    heavyTail: heavyTailRows(recompute),
    points: pointRows(recompute),
    twinValues: specs.map((spec) => [sweepTwinKey(spec), valueOf(twinValues, spec)]),
  };
}

/** The twin's values a committed record holds, or an empty map for a placeholder. */
export function sweepTwinValuesOfRecord(record: unknown): Map<string, number> {
  const values = new Map<string, number>();
  const fields = fieldsOf(record);
  if (fields?.["format"] !== SWEEP_BOUNDARY_FORMAT) {
    return values;
  }
  const rows = fields["twinValues"];
  if (!Array.isArray(rows)) {
    return values;
  }
  for (const row of rows) {
    if (!Array.isArray(row)) {
      continue;
    }
    const [key, value]: ReadonlyArray<unknown> = row;
    if (typeof key === "string" && typeof value === "number") {
      values.set(key, value);
    }
  }
  return values;
}
