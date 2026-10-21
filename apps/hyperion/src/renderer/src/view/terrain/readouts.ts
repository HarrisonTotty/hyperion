/**
 * The terrain lines of a view's label block, as a pure model (plan R10, R10.T12): the survey line
 * and the state after it, the `DATUM` line, and the terrain readout under
 * `GROUND AT LINE OF SIGHT`, each line typed down to its fixed-width fields, so that a component
 * renders it with no logic of its own; with the line of sight the readout is asked along
 * ({@link lineOfSightOf}).
 *
 * @remarks
 * - **The guide.** "Views": the survey line and its sub-bullets, the terrain readout and its
 *   properties; "Numbers, units and time": an Estimated value rounded to its uncertainty
 *   ({@link formatWithUncertainty}), fixed field widths, grouping and signs; "Data states": Stale,
 *   Missing and Estimated; and the nomenclature rows of every label here.
 * - **The decisions.** decision-r10-t16-leans §12, the survey line's states; decision-r10-t12-leans
 *   §4 (the stale forms), §5 (no readout under the terrain's fault) and §7 (a survey held is a held
 *   header, `NONE` included).
 * - **The model.** A line is a reading (a label and its segments, which a renderer joins with the
 *   guide's middle dot, `PART_SEPARATOR`, and keeps each on one line where it fits), a status line
 *   with its colour token, or the readout's heading. A segment's pieces are set end to end as they
 *   are, their spaces in them: a value in a fixed field ({@link ReadoutField}: right-aligned, or
 *   after a `±` left-aligned with its `S` in it, in its width in `ch` of B612 Mono), the place kept
 *   for its `S` ({@link ReadoutMark}, {@link STALE_MARK_CH} wide), the console's own text, and a
 *   status. Every string is the guide's as written; {@link readoutLineText} gives a line's whole
 *   text without its fields' padding.
 * - **Its own model, not `viewRun.ts`'s `LabelLine`.** `LabelLine` carries a reading as one string
 *   whose fields `ViewLabelBlock`'s `FieldRun` finds again by matching its runs (`UNBREAKABLE`,
 *   which admits no `~`, `±`, `%`, `K`, `°` reading or pressure unit), with one stale flag for the
 *   whole line or a field, and its `S` inside the field's width. These lines need what that cannot
 *   carry: two-part `±` fields whose `S` follows the uncertainty, a place for the `S` kept after
 *   the field, values stale beside statuses and console text that are not, the em dash muted in a
 *   field, and status lines in their colour. Typed pieces carry all of it with no text matching.
 * - **Hysteresis.** The units of `FINEST`, `RESOLUTION`, `SLANT RANGE` and the capacity are passed
 *   in and returned ({@link ReadoutUnits}), so that a view keeps them between publishes; a unit is
 *   kept while its value is not shown.
 */

import type { SurfaceHeaderDto } from "@hyperion/protocol";

import { norm, scale, vec3 } from "../../geometry/vec3";
import {
  DISTANCE_HYSTERESIS,
  formatBearingCapacity,
  formatBodyDistance,
  formatNumber,
  formatSci,
  formatSignificant,
  formatSurfaceRange,
  formatTemperatureK,
  formatTwoFigures,
  KM_PER_AU,
  type PressureUnit,
  type SurfaceRangeUnit,
} from "../../lib/format";
import { substanceName } from "../../lib/substances";
import type { CameraPose } from "../camera/pose";
import { rotationRows } from "../camera/quaternion";
import { relativeToCamera } from "../coords/relative";
import { rotateToBodyFixed } from "../coords/rotation";
import { sceneOrigins, type ViewBody, type ViewScene } from "../scene/model";
import { isTerrainFault } from "./annunciation";
import type { FieldFailure } from "./coarseFieldStore";
import type { CoverageView } from "./coverage";
import type { GroundSight, TerrainSurvey } from "./session";
import type {
  SlopeBaselineM,
  SurfaceGroundMaterial,
  SurfaceGroundProperties,
  SurfaceGroundReadout,
  SurfaceReading,
  SurfaceSlantRange,
} from "./workers/messages";

/** The Missing value: an em dash, set in `--text-muted` (the guide's "Data states"). */
export const MISSING = "—";

/** The no-break space either side of `±` (the guide's `±` row), so that a reading never breaks. */
export const NO_BREAK_SPACE = " ";

/** The Estimated mark before a value (the guide's "Data states"). */
const ESTIMATED = "~";

/** The place kept after a value for its stale mark, ch: a space and the `S` (`StaleMark`). */
export const STALE_MARK_CH = 2;

/** The survey line's state before the body's survey first arrives. */
export const SURVEY_PENDING = "SURVEY COVERAGE: PENDING";

/** The survey line's place where this generator version computes no surface for the body. */
export const TERRAIN_NOT_MODELLED = "TERRAIN: NOT YET MODELLED";

/** The terrain readout's heading (the guide's `GROUND AT LINE OF SIGHT` row). */
export const GROUND_HEADING = "GROUND AT LINE OF SIGHT";

/** The readout's one status over ground the ship has not surveyed. */
export const NOT_SURVEYED = "NOT SURVEYED";

/** The readout's one status where the line of sight passes the body without meeting its ground. */
export const NO_GROUND = "NO GROUND";

/** The status after a `SLANT RANGE` in its uncertainty's place, where the line of sight grazes. */
export const GRAZING = "GRAZING";

/**
 * What the survey line's two failed states end with: `VIEW` shown again asks the field afresh. A
 * host with no `VIEW` to reopen gives its own remedy (`surveyLines`' `remedy`; plan R10, R10.T14.c,
 * decision-r10-t14c-guide Q2: the descent spike's `relaunch to retry`).
 */
const RETRY = "reopen VIEW to retry";

// --- Field widths -------------------------------------------------------------------------------

/**
 * `SURVEY COVERAGE`'s field, ch: `ABOVE 99.9 %`, the longest of its forms, against `12.4 %`,
 * `0.00123 %`, `2.00E-5 %` (three figures, in E notation below 0.001 %) and `NONE`.
 */
export const COVERAGE_FIELD_CH = "ABOVE 99.9 %".length;

/**
 * `FINEST`'s and `RESOLUTION`'s field, ch: `36,000 km`, the resolution code ladder's coarsest code
 * (255, 1 cm × 2^(254 ÷ 8), about 36,100 km; `coverage.ts`' `resolutionOfCodeM`) at two figures;
 * its finest, code 1's 1 cm, reads `0.010 m`, and under hysteresis `1000 m` and `0.96 km`.
 */
export const RESOLUTION_FIELD_CH = "36,000 km".length;

/**
 * `ELEVATION`'s value, ch: `~-99,999.9 m`, an Estimated elevation within 100 km of the datum at its
 * finest 0.1 m. The generator's hypsometry is Gaussian populations truncated at ±3 σ_h about the
 * datum (`hyperion-sim`'s `relief.rs`, `Hypsometry`), and σ_h's structural, basin and
 * constructional shares are each of the order of 10 km at most (the basin's capped at 2 % of the
 * radius, the volcanism level held to 1.984), so that no elevation reaches 100 km.
 */
export const ELEVATION_FIELD_CH = "~-99,999.9 m".length;

/**
 * `ELEVATION`'s uncertainty, ch: ` ± 100,000 m`, its no-break spaces included, the longest the
 * rounding gives: the omitted relief's standard deviation is of the relief, below 100 km as the
 * elevation is, and the rule takes 95,000 m and up to `100,000 m` ({@link formatWithUncertainty}).
 */
export const ELEVATION_UNCERTAINTY_CH = `${NO_BREAK_SPACE}±${NO_BREAK_SPACE}100,000 m`.length;

/** `SLOPE`'s value, ch: `~90°`, a slope being at most 90° from the horizontal. */
export const SLOPE_FIELD_CH = "~90°".length;

/** `SLOPE`'s uncertainty, ch: ` ± 90°`, the uncertainty of an angle of at most 90°. */
export const SLOPE_UNCERTAINTY_CH = `${NO_BREAK_SPACE}±${NO_BREAK_SPACE}90°`.length;

/**
 * `SLANT RANGE`'s value, ch: `~5.00E-3 m`, a range below 1 cm in E notation, grazing
 * (`formatSurfaceRange`) or Estimated ({@link formatSlantRange}), against the three figures'
 * longest elsewhere, `~0.960 km` under its hysteresis, and an Estimated range's plain digits to
 * four decimals, `~0.0050 m`; the reach is at most 10¹² m (`wasm.rs`' `MAX_SIGHT_M`), `6.68 AU`.
 */
export const SLANT_RANGE_FIELD_CH = "~5.00E-3 m".length;

/**
 * `SLANT RANGE`'s uncertainty, ch: ` ± 10,000 km`, its no-break spaces included, the longest the
 * rounding gives: an uncertainty below 10,000 of the range's unit, which the rule takes from 9,500
 * up to `10,000`. The finest, below 1 cm in E notation (`± 3.4E-6 m`) or at four decimals
 * (`± 0.0001 m`, a 1 cm range's third figure), are shorter.
 */
export const SLANT_RANGE_UNCERTAINTY_CH = `${NO_BREAK_SPACE}±${NO_BREAK_SPACE}10,000 km`.length;

/**
 * `ULTIMATE BEARING CAPACITY`'s field, ch: `~0.95 MPa`, as the guide sizes it: two figures make at
 * most four characters on the capacity's ladder (`0.95` and `1000` under its hysteresis, `990`),
 * with the `~` and a three-letter unit. Every shear table's cohesion, 0.52 kPa and up
 * (`properties.rs`), keeps a level pad's capacity in kPa or MPa.
 */
export const CAPACITY_FIELD_CH = "~0.95 MPa".length;

/**
 * `PAD FRICTION`'s field, ch: `~0.71`, the highest of its sources' figures (Liefferink et al.
 * 2021's ice at 176 K) and every friction below 10, at two decimals.
 */
export const FRICTION_FIELD_CH = "~0.71".length;

/** `AT`'s temperature, ch: `272 K`, the ground's temperature in whole kelvin below 1000 K. */
export const TEMPERATURE_FIELD_CH = "272 K".length;

// --- The model ----------------------------------------------------------------------------------

/**
 * How a field's text is set: a value in `--text`, a stale one in `--text-muted`, or Missing, the
 * em dash in `--text-muted` (the guide's "Data states").
 */
export type FieldTone = "value" | "stale" | "missing";

/**
 * A value in its fixed field: its text set in `widthCh` of B612 Mono, right-aligned (`end`) or, for
 * the uncertainty after a `±`, left-aligned (`start`); `widthCh` 0 for a name, which keeps its own
 * width.
 */
export interface ReadoutField {
  readonly kind: "field";
  readonly text: string;
  readonly widthCh: number;
  readonly align: "end" | "start";
  readonly tone: FieldTone;
  /**
   * Whether the stale mark `S` follows the text inside the field, after a space: a `±` reading's
   * uncertainty while stale, whose field holds the mark's place ({@link STALE_MARK_CH}) in its
   * width, so that the `S` follows the uncertainty, `~2140 m ± 180 m S`, not its padding.
   */
  readonly staleMark: boolean;
}

/**
 * The place kept after a value for its `S`, {@link STALE_MARK_CH} wide: the mark while the value is
 * stale, blank otherwise, so that the reading after it never moves.
 */
export interface ReadoutMark {
  readonly kind: "mark";
  readonly stale: boolean;
}

/**
 * The console's own text in a reading, in `--text`, never stale, with its spaces: a label within it
 * (`FINEST `, `AT `) or one of the console's settings (`BASELINE 2 m`, `PAD WIDTH 1.0 m`).
 */
export interface ReadoutText {
  readonly kind: "text";
  readonly text: string;
}

/** A status within a reading (`GRAZING`): in `--text`, never stale, since it is not a value. */
export interface ReadoutStatus {
  readonly kind: "status";
  readonly text: string;
}

/** One piece of a reading's segment. */
export type ReadoutPiece = ReadoutField | ReadoutMark | ReadoutText | ReadoutStatus;

/** A part of a reading between its middle dots: pieces set end to end, kept on one line. */
export type ReadoutSegment = ReadonlyArray<ReadoutPiece>;

/** A status line's colour token: `--text`, or `--status-caution` for a failed system or a fault. */
export type StatusColour = "text" | "status-caution";

/** One line of the terrain's part of a label block. */
export type ReadoutLine =
  /** A reading: its label, and its segments, joined by the guide's middle dot. */
  | {
      readonly kind: "reading";
      /** A key stable from one publish to the next, its label's. */
      readonly key: string;
      readonly label: string;
      readonly segments: ReadonlyArray<ReadoutSegment>;
    }
  /** A status standing in a line's place, or in place of the readings: steady, in its colour. */
  | {
      readonly kind: "status";
      readonly key: string;
      readonly text: string;
      readonly colour: StatusColour;
      /**
       * What it reports, where it is a request's outcome rather than a statement: the server's
       * refusal of what was asked (`refused`, in `--text`) or a failure (`fault`, in
       * `--status-caution`), the survey line's `SURVEY COVERAGE REJECTED` and `SURVEY DATA
       * INVALID`, which a renderer sets as every request's status is; `null` for a statement.
       */
      readonly failure: "refused" | "fault" | null;
    }
  /** The readout's heading, {@link GROUND_HEADING}. */
  | { readonly kind: "heading"; readonly key: string; readonly text: string };

/** The units a view's terrain values were last shown in, kept between publishes for hysteresis. */
export interface ReadoutUnits {
  readonly finest: ResolutionUnit | null;
  readonly resolution: ResolutionUnit | null;
  readonly slantRange: SurfaceRangeUnit | null;
  readonly capacity: PressureUnit | null;
}

/** No unit shown yet: a view's first publish. */
export const NO_READOUT_UNITS: ReadoutUnits = {
  finest: null,
  resolution: null,
  slantRange: null,
  capacity: null,
};

/** A field. */
function field(
  text: string,
  widthCh: number,
  tone: FieldTone,
  align: "end" | "start" = "end",
  staleMark = false,
): ReadoutField {
  return { kind: "field", text, widthCh, align, tone, staleMark };
}

/** The place kept for a value's `S`. */
function mark(stale: boolean): ReadoutMark {
  return { kind: "mark", stale };
}

/** The console's own text. */
function consoleText(words: string): ReadoutText {
  return { kind: "text", text: words };
}

/** A value's tone: stale while its source does not back it. */
function toneOf(stale: boolean): FieldTone {
  return stale ? "stale" : "value";
}

/** A reading line. */
function reading(label: string, segments: ReadonlyArray<ReadoutSegment>): ReadoutLine {
  return { kind: "reading", key: label, label, segments };
}

/** A status line: a statement, unless it reports a request's refusal or failure. */
function status(
  key: string,
  words: string,
  colour: StatusColour,
  failure: "refused" | "fault" | null = null,
): ReadoutLine {
  return { kind: "status", key, text: words, colour, failure };
}

/**
 * A line's whole text as the guide writes it: its label, then its segments joined by a middle dot,
 * each field's text without its padding and each kept place blank but for its `S`
 * (`ELEVATION ~2140 m ± 180 m S`).
 */
export function readoutLineText(line: ReadoutLine): string {
  if (line.kind !== "reading") {
    return line.text;
  }
  const segments = line.segments.map((segment) => segment.map(pieceText).join(""));
  return `${line.label} ${segments.join(" · ")}`;
}

/** A piece's text without its field's padding: a kept place is blank but for its `S`. */
function pieceText(piece: ReadoutPiece): string {
  let shown: string;
  switch (piece.kind) {
    case "field":
      shown = piece.staleMark ? `${piece.text} S` : piece.text;
      break;
    case "text":
    case "status":
      shown = piece.text;
      break;
    case "mark":
      shown = piece.stale ? " S" : "";
      break;
  }
  return shown;
}

// --- Rounding by the uncertainty ----------------------------------------------------------------

/** An Estimated value and its uncertainty as shown, digits and sign only, in the same unit. */
export interface RoundedEstimate {
  /** The value, rounded to the uncertainty's last digit: `2140`, `-410`, `12,500`. */
  readonly value: string;
  /** The uncertainty: `180`, `0.2`, `1000`. */
  readonly uncertainty: string;
}

/** The tolerance by which a decimal figure stored as a binary float is read at its decimal value. */
const DECIMAL_TOLERANCE = 1e-9;

/** `x` ÷ 10^place, multiplying by an exact power of ten where the place is negative. */
function inPlaceUnits(x: number, place: number): number {
  return place >= 0 ? x / 10 ** place : x * 10 ** -place;
}

/** `k` × 10^place, dividing by an exact power of ten where the place is negative, as a decimal. */
function atPlace(k: number, place: number): number {
  return place >= 0 ? k * 10 ** place : k / 10 ** -place;
}

/** `value` rounded to a multiple of 10^place, halves away from zero. */
function roundToPlace(value: number, place: number): number {
  return Math.sign(value) * atPlace(Math.round(Math.abs(inPlaceUnits(value, place))), place);
}

/** The power of ten `step` is, or a `RangeError` for a step that is not one. */
function placeOfStep(step: number): number {
  const place = Math.round(Math.log10(step));
  if (
    !(Number.isFinite(step) && step > 0) ||
    Math.abs(inPlaceUnits(step, place) - 1) > DECIMAL_TOLERANCE
  ) {
    throw new RangeError(`a quantity's step is a power of ten, not ${String(step)}`);
  }
  return place;
}

/**
 * An uncertainty's three highest digits, 100 to 999, and the place of the last of them, so that
 * the uncertainty is `digits` × 10^`place` and a part of that place more: taken literally, as the
 * rule's own words have them, by flooring the uncertainty in units of that place, its decimal
 * value read through a small tolerance rather than its binary float's, so that 0.3549 gives 354,
 * and 0.355 and 0.95 give 355 and 950.
 */
function leadingDigits(sigma: number): { readonly digits: number; readonly place: number } {
  let place = Math.floor(Math.log10(sigma)) - 2;
  const digitsAt = (at: number): number =>
    Math.floor(inPlaceUnits(sigma, at) * (1 + DECIMAL_TOLERANCE));
  let digits = digitsAt(place);
  // log10's floor may be one off about a power of ten, and the tolerance may carry 999.9… to 1000.
  if (digits >= 1_000) {
    place += 1;
    digits = digitsAt(place);
  } else if (digits < 100) {
    place -= 1;
    digits = digitsAt(place);
  }
  return { digits, place };
}

/** An Estimated value and its uncertainty rounded for the screen, as numbers, with their place. */
interface RoundedNumbers {
  /** The value, a multiple of 10^`place`. */
  readonly value: number;
  /** The uncertainty, a multiple of 10^`place`. */
  readonly sigma: number;
  /** The power of ten of both numbers' last digit. */
  readonly place: number;
}

/** {@link formatWithUncertainty}'s rounding, as numbers. */
function roundByUncertainty(value: number, sigma: number, step: number): RoundedNumbers {
  if (!Number.isFinite(value) || !Number.isFinite(sigma) || sigma < 0) {
    throw new RangeError(
      `an Estimated value needs a finite value and uncertainty, not ${String(value)} with an ` +
        `uncertainty of ${String(sigma)}`,
    );
  }
  const stepPlace = placeOfStep(step);
  let place = stepPlace - 1;
  let shownSigma = 0;
  if (sigma > 0) {
    const lead = leadingDigits(sigma);
    if (lead.digits <= 354) {
      place = lead.place + 1;
      shownSigma = atPlace(Math.round(lead.digits / 10), place);
    } else if (lead.digits <= 949) {
      place = lead.place + 2;
      shownSigma = atPlace(Math.round(lead.digits / 100), place);
    } else {
      // Up to 1000 of the leading place, kept to two figures: "10" at the next place but one.
      place = lead.place + 2;
      shownSigma = atPlace(10, place);
    }
  }
  if (place < stepPlace) {
    // The rule's figure is finer than the quantity's own precision: that figure, not the raw
    // uncertainty, goes up to whole steps, never down, at least one.
    const steps = Math.ceil(inPlaceUnits(shownSigma, stepPlace) - DECIMAL_TOLERANCE);
    place = stepPlace;
    shownSigma = atPlace(Math.max(1, steps), place);
  }
  return { value: roundToPlace(value, place), sigma: shownSigma, place };
}

/**
 * Rounds an Estimated value and its standard uncertainty for the screen by the Particle Data
 * Group's rule, as the guide's "Numbers, units and time" gives it for an Estimated value (plan
 * R10, Design note 6): `~2140 m ± 180 m`, `~2100 m ± 500 m`, `~2100 m ± 1000 m`.
 *
 * @remarks
 * - The uncertainty's three highest digits decide, taken as they stand, not rounded
 *   ({@link leadingDigits}; "the three highest order digits of the error", Review of Particle
 *   Physics 2024, Introduction §5.3): 100 to 354 keep two significant figures, 355 to 949 keep one,
 *   and 950 to 999 round up to 1000 and keep two. So σ = 0.3549 reads two figures, `0.35`.
 * - The value is rounded to the same decimal place, so that no value is shown finer than its
 *   uncertainty.
 * - Neither number goes finer than the quantity's own `step`: where the rule's uncertainty would,
 *   that rounded figure is rounded up to a whole number of steps, never down, at least one, and the
 *   value to the nearest step. An elevation's 0.04 m and 0.12 m read `± 0.1 m` and `± 0.2 m`, while
 *   0.1004 m, the rule's 0.10, reads `± 0.1 m`, and a slope's 1.02°, the rule's 1.0, `± 1°`. Rounding
 *   an uncertainty up rather than to the nearest digit is the GUM's allowance for a reported result
 *   (JCGM 100:2008 §7.2.6).
 * - Digits are grouped in threes from five, and a negative value carries `-`; one that rounds to
 *   zero carries no sign.
 *
 * @param value - The value, in the unit it is shown in.
 * @param sigma - Its standard uncertainty, one standard deviation, in the same unit: finite and at
 *   least 0; 0 reads one step.
 * @param step - The quantity's own precision in that unit, a power of ten: 0.1 for an elevation in
 *   m, 1 for a slope in degrees.
 * @throws RangeError for a value or uncertainty that is not finite, a negative uncertainty, or a
 *   step that is not a positive power of ten.
 */
export function formatWithUncertainty(value: number, sigma: number, step: number): RoundedEstimate {
  const rounded = roundByUncertainty(value, sigma, step);
  const decimals = Math.max(0, -rounded.place);
  return {
    value: formatNumber(rounded.value, decimals),
    uncertainty: formatNumber(rounded.sigma, decimals),
  };
}

// --- The readings' formats ----------------------------------------------------------------------

/** A reading as its fields show it: the value with its marks and unit, and its uncertainty. */
export interface FieldReading {
  /** The value with its `~` where Estimated, and its unit: `~2140 m`, `2143.7 m`, `7°`. */
  readonly value: string;
  /**
   * The uncertainty with its sign and unit, `± 180 m`, a no-break space after the sign; `null`
   * where the value is resolved, or Estimated with no uncertainty stated.
   */
  readonly uncertainty: string | null;
}

/** The uncertainty after `±`, the unit on it, joined by a no-break space after the sign. */
function plusMinus(digits: string, unit: string): string {
  return `±${NO_BREAK_SPACE}${digits}${unit}`;
}

/**
 * Formats an `ELEVATION` (the guide's row): resolved, in metres at every value to 0.1 m, with `-`
 * below the datum and no `+` above it, digits grouped from five, `2143.7 m`, `-410.0 m`,
 * `12,480.0 m`; Estimated, rounded by its uncertainty ({@link formatWithUncertainty}),
 * `~2140 m ± 180 m`.
 *
 * @param elevationM - Above the datum, m.
 * @param sigmaM - Its standard uncertainty, m, while the survey has not resolved the point; `null`
 *   once it has.
 */
export function formatElevationReading(elevationM: number, sigmaM: number | null): FieldReading {
  if (sigmaM === null) {
    return { value: `${formatNumber(elevationM, 1)} m`, uncertainty: null };
  }
  const rounded = formatWithUncertainty(elevationM, sigmaM, 0.1);
  return {
    value: `${ESTIMATED}${rounded.value} m`,
    uncertainty: plusMinus(rounded.uncertainty, " m"),
  };
}

/** Degrees in a radian. */
const DEG_PER_RAD = 180 / Math.PI;

/**
 * Formats a `SLOPE` (the guide's row): resolved, in whole degrees, `7°`; Estimated, the RMS slope
 * rounded by its uncertainty at whole degrees, `~12° ± 4°`. Its `· BASELINE` follows in the line.
 *
 * @param slopeRad - The slope, rad: resolved, or the RMS slope.
 * @param sigmaRad - Its standard uncertainty, rad, while the point is not resolved; `null` once it
 *   is.
 */
export function formatSlopeReading(slopeRad: number, sigmaRad: number | null): FieldReading {
  const slopeDeg = slopeRad * DEG_PER_RAD;
  if (sigmaRad === null) {
    return { value: `${formatNumber(slopeDeg, 0)}°`, uncertainty: null };
  }
  const rounded = formatWithUncertainty(slopeDeg, sigmaRad * DEG_PER_RAD, 1);
  return {
    value: `${ESTIMATED}${rounded.value}°`,
    uncertainty: plusMinus(rounded.uncertainty, "°"),
  };
}

/** A `SLANT RANGE` as its fields show it, and the unit to keep for its hysteresis. */
export interface SlantRangeReading extends FieldReading {
  /** Whether it grazes: Estimated with no uncertainty, `GRAZING` in the uncertainty's place. */
  readonly grazing: boolean;
  readonly unit: SurfaceRangeUnit;
}

/** Metres in each of a range's units. */
const METRES_PER_RANGE_UNIT: Readonly<Record<SurfaceRangeUnit, number>> = {
  m: 1,
  km: 1e3,
  Mm: 1e6,
  Gm: 1e9,
  AU: KM_PER_AU * 1e3,
};

/** The step of a value's third significant figure, as three figures show it: 0.1 for 41.2. */
function thirdFigureStep(value: number): number {
  const shown = Math.abs(Number(value.toPrecision(3)));
  if (shown === 0) {
    return 0.01;
  }
  let exponent = Math.floor(Math.log10(shown));
  if (atPlace(1, exponent + 1) <= shown) {
    exponent += 1;
  } else if (atPlace(1, exponent) > shown) {
    exponent -= 1;
  }
  return atPlace(1, exponent - 2);
}

/**
 * Formats a `SLANT RANGE` (the guide's row): its unit and precision `formatSurfaceRange`'s, `m`
 * below 1 km, then `km`, `Mm`, `Gm` and `AU`, three figures, the unit kept within its hysteresis
 * from `previous`; Estimated, rounded by its uncertainty in that unit, the step its third
 * significant figure, `~41.2 km ± 0.3 km`, and below 1 cm, where that would take more than four
 * decimals, in E notation as the resolved form is there, `~5.00E-3 m ± 3.4E-4 m`; grazing,
 * Estimated at its own precision with no uncertainty, `~41.2 km`, which the line follows with
 * `· GRAZING`. A resolved range is never grazing.
 *
 * @param range - The answer's range: metres, its uncertainty in metres or `null` where it grazes,
 *   and whether the survey resolves the ground there.
 * @param previous - The unit it was last shown in, which the caller keeps; `null` for the first.
 */
export function formatSlantRange(
  range: Pick<SurfaceSlantRange, "rangeM" | "sigmaM" | "resolved">,
  previous: SurfaceRangeUnit | null,
): SlantRangeReading {
  const shown = formatSurfaceRange(range.rangeM, previous);
  const { unit } = shown;
  if (range.resolved) {
    return { value: `${shown.value} ${unit}`, uncertainty: null, grazing: false, unit };
  }
  if (range.sigmaM === null) {
    return { value: `${ESTIMATED}${shown.value} ${unit}`, uncertainty: null, grazing: true, unit };
  }
  const perUnit = METRES_PER_RANGE_UNIT[unit];
  const inUnit = range.rangeM / perUnit;
  const rounded = roundByUncertainty(inUnit, range.sigmaM / perUnit, thirdFigureStep(inUnit));
  let value: string;
  let uncertainty: string;
  if (rounded.place < -PLAIN_RANGE_DECIMALS) {
    // Finer than plain digits hold, which only a range below 1 cm reaches, whose third figure is
    // finer than 10⁻⁴ m: both numbers in E notation, as the resolved form is there, each to the
    // figures down to their place (`~5.00E-3 m ± 3.4E-4 m`).
    value = formatSci(
      rounded.value,
      "reading",
      Math.min(3, figuresTo(rounded.value, rounded.place)),
    );
    uncertainty = formatSci(rounded.sigma, "reading", figuresTo(rounded.sigma, rounded.place));
  } else {
    const decimals = Math.max(0, -rounded.place);
    value = formatNumber(rounded.value, decimals);
    uncertainty = formatNumber(rounded.sigma, decimals);
  }
  return {
    value: `${ESTIMATED}${value} ${unit}`,
    uncertainty: plusMinus(uncertainty, ` ${unit}`),
    grazing: false,
    unit,
  };
}

/** The most decimals an Estimated `SLANT RANGE`'s plain digits take: `0.0100 m` at 1 cm. */
const PLAIN_RANGE_DECIMALS = 4;

/** How many significant figures `x`, a multiple of 10^`place` and not 0, has down to that place. */
function figuresTo(x: number, place: number): number {
  const [, exponent] = Math.abs(x).toExponential().split("e");
  return Number(exponent) - place + 1;
}

/** Unit of a survey resolution as {@link formatSurveyResolution} writes it. */
export type ResolutionUnit = "m" | "km";

/** A survey resolution split into its digits and unit. */
export interface FormattedResolution {
  readonly value: string;
  readonly unit: ResolutionUnit;
}

/** Resolutions below this many metres, after rounding, are read in `m`. */
const RESOLUTION_METRES_BELOW_M = 1_000;

/**
 * Formats a survey resolution as `FINEST` and `RESOLUTION` read it (the guide's survey line): a
 * length to two significant figures, in `m` below 1 km and in `km` above, `2.0 m`, `410 m`,
 * `1.5 km`, switching with `DISTANCE_HYSTERESIS` as a view's ranges do.
 *
 * @remarks
 * Without a previous unit the unit is decided after rounding, so 999.6 m reads `1.0 km`. One shown
 * in `m` keeps it up to 5% past 1 km (`1000 m`), and one shown in `km` keeps it down to 5% under
 * (`0.96 km`).
 *
 * @param resolutionM - The resolution, m, positive.
 * @param previous - The unit it was last shown in, which the caller keeps; `null` for the first.
 */
export function formatSurveyResolution(
  resolutionM: number,
  previous: ResolutionUnit | null,
): FormattedResolution {
  let metres: boolean;
  switch (previous) {
    case "m":
      metres = resolutionM < RESOLUTION_METRES_BELOW_M * (1 + DISTANCE_HYSTERESIS);
      break;
    case "km":
      metres = resolutionM < RESOLUTION_METRES_BELOW_M * (1 - DISTANCE_HYSTERESIS);
      break;
    case null:
      metres = Math.abs(Number(resolutionM.toPrecision(2))) < RESOLUTION_METRES_BELOW_M;
      break;
  }
  return metres
    ? { value: formatTwoFigures(resolutionM), unit: "m" }
    : { value: formatTwoFigures(resolutionM / 1_000), unit: "km" };
}

/**
 * The material forms, by `hyperion-surface`'s `MaterialClass` code (`material/class.rs`), in the
 * guide's words (the material form row): the glacier's code reads `ICE`.
 */
const MATERIAL_FORMS: ReadonlyArray<string> = [
  "BEDROCK",
  "REGOLITH",
  "SOIL",
  "SAND",
  "SEDIMENT",
  "VOLCANIC ROCK",
  "MELT",
  "ICE",
  "SNOW",
  "SEA ICE",
  "FROST",
  "ORGANIC SEDIMENT",
  "SEABED",
  "EVAPORITE",
  "DUST",
];

/**
 * A material form's word for a `MaterialClass` code (the guide's material forms: `BEDROCK` to
 * `DUST`), or `null` for a code outside the table, which the module of the same build never gives.
 */
export function materialFormName(classCode: number): string | null {
  return Number.isInteger(classCode) ? (MATERIAL_FORMS[classCode] ?? null) : null;
}

/**
 * The substance of a material's palette entry, in a label's case (`substanceName`): `UNLISTED
 * MATERIAL` for a key the console's list does not name, and `null`, the em dash, where the entry is
 * none or outside the palette.
 */
function substanceOf(entry: number | null, palette: SurfaceHeaderDto["palette"]): string | null {
  if (entry === null) {
    return null;
  }
  const named = palette[entry];
  return named === undefined ? null : substanceName(named.substance, "upper");
}

/**
 * The survey's coverage as `SURVEY COVERAGE` reads it: its share in per cent to three figures, in E
 * notation below 0.001 % (`2.00E-5 %`); `ABOVE 99.9 %` where the share is below the whole and its
 * figure rounds to the whole's, and `100 %` only for the whole body (`CoverageView`'s share is
 * exactly 1 only then); `NONE` where no cell is surveyed.
 */
export function formatCoverage(
  coverage: Pick<CoverageView, "surveyedFraction" | "bestResolutionM">,
): string {
  if (coverage.bestResolutionM() === null) {
    return "NONE";
  }
  const fraction = coverage.surveyedFraction();
  const shown = formatSignificant(fraction * 100);
  // Ruled on the figure as shown, as BodyRecordReadings' shares are, so that rule and figure agree.
  return fraction < 1 && shown === formatSignificant(100) ? "ABOVE 99.9 %" : `${shown} %`;
}

// --- The lines ----------------------------------------------------------------------------------

/**
 * The survey line's state for a failure (decision-r10-t16-leans §12), in its colour, ending with
 * `remedy`.
 */
function failureLine(failure: FieldFailure, remedy: string): ReadoutLine {
  let line: ReadoutLine;
  switch (failure.kind) {
    case "rejected": {
      const reason = failure.message.trim();
      const words = reason.length > 0 ? `${reason}, ${remedy}` : remedy;
      line = status(
        "survey-state",
        `SURVEY COVERAGE REJECTED: ${words}`,
        failure.refusal ? "text" : "status-caution",
        failure.refusal ? "refused" : "fault",
      );
      break;
    }
    case "invalid":
      line = status(
        "survey-state",
        `SURVEY DATA INVALID: ${failure.check}, ${remedy}`,
        "status-caution",
        "fault",
      );
      break;
  }
  return line;
}

/** The survey line and the units it kept. */
export interface SurveyLines {
  /** The survey line or its state in its place, then, with a survey held, any state after it. */
  readonly lines: ReadonlyArray<ReadoutLine>;
  readonly units: ReadoutUnits;
}

/**
 * The survey line of a view whose `FRAME` is a body's with a surface (the guide's survey line;
 * decision-r10-t16-leans §12): `SURVEY COVERAGE 12.4 % · FINEST 2.0 m`.
 *
 * @remarks
 * - No survey of the body known, or none held and no failure: `SURVEY COVERAGE: PENDING`, which a
 *   busy server leaves standing (the store asks again itself).
 * - Not modelled: `TERRAIN: NOT YET MODELLED` in the line's place.
 * - A failure with no survey held: its state in the line's place, `SURVEY COVERAGE REJECTED:
 *   <the server's message>, reopen VIEW to retry` (`--text` for a refusal, `--status-caution` for
 *   the server's own failure) or `SURVEY DATA INVALID: <check>, reopen VIEW to retry`
 *   (`--status-caution`).
 * - A survey held (its header, from its first answer, `NONE` included; decision-r10-t12-leans §7):
 *   the line's values, each stale with its `S` while the survey is (`TerrainSurvey.stale`: the link
 *   down, or a failure standing), `SURVEY COVERAGE NONE S` included; and any failure's state on a
 *   line of its own after it. A status takes no `S`.
 *
 * @param remedy - What the two failed states end with: `VIEW`'s `reopen VIEW to retry` unless
 *   given.
 */
export function surveyLines(
  survey: TerrainSurvey | null,
  units: ReadoutUnits,
  remedy: string = RETRY,
): SurveyLines {
  if (survey === null) {
    return { lines: [status("survey", SURVEY_PENDING, "text")], units };
  }
  const { status: standing, coverage, header } = survey;
  if (standing.notModelled !== null) {
    return { lines: [status("survey", TERRAIN_NOT_MODELLED, "text")], units };
  }
  if (header === null || coverage === null) {
    const line =
      standing.failure === null
        ? status("survey", SURVEY_PENDING, "text")
        : failureLine(standing.failure, remedy);
    return { lines: [line], units };
  }
  const { stale } = survey;
  const tone = toneOf(stale);
  const best = coverage.bestResolutionM();
  const coverageSegment: ReadoutSegment = [
    field(formatCoverage(coverage), COVERAGE_FIELD_CH, tone),
    mark(stale),
  ];
  let segments: ReadonlyArray<ReadoutSegment> = [coverageSegment];
  let finest = units.finest;
  if (best !== null) {
    const shown = formatSurveyResolution(best, finest);
    finest = shown.unit;
    segments = [
      coverageSegment,
      [
        consoleText("FINEST "),
        field(`${shown.value} ${shown.unit}`, RESOLUTION_FIELD_CH, tone),
        mark(stale),
      ],
    ];
  }
  const lines: ReadoutLine[] = [reading("SURVEY COVERAGE", segments)];
  if (standing.failure !== null) {
    lines.push(failureLine(standing.failure, remedy));
  }
  return { lines, units: { ...units, finest } };
}

/** Kilometres from metres, to 0.1 km, digits grouped from five: `6378.1`, `12,742.0`. */
function radiusKm(radiusM: number): string {
  return formatNumber(radiusM / 1_000, 1);
}

/**
 * A reference spheroid's figure in `DATUM`, `REFERENCE SPHEROID 6378.1 km × 6356.8 km`, its
 * equatorial and polar radii as two runs, `6378.1 km ×` and `6356.8 km`, with the figure's one `S`
 * after the second (plan R10, R10.T12.a; decision-r10-t12a-scope §4.3).
 *
 * @remarks
 * The space between the two runs is the console's own, so that a narrow plate breaks the figure
 * after its `×` and each radius stands whole, never running past the plate; its text, and so its
 * {@link readoutLineText}, are the single run's they replace.
 *
 * @param equatorialM - The equatorial radius, m.
 * @param polarM - The polar radius, m.
 */
function spheroidFigure(equatorialM: number, polarM: number, stale: boolean): ReadoutSegment {
  const tone = toneOf(stale);
  return [
    consoleText("REFERENCE SPHEROID "),
    field(`${radiusKm(equatorialM)} km ×`, 0, tone),
    consoleText(" "),
    field(`${radiusKm(polarM)} km`, 0, tone),
    mark(stale),
  ];
}

/**
 * The `DATUM` line (the guide's row): the body's reference spheroid, its equatorial and polar radii
 * to 0.1 km with the unit on both, `DATUM REFERENCE SPHEROID 6378.1 km × 6356.8 km`, or, where the
 * two are equal, a sphere, `DATUM REFERENCE SPHERE 1737.4 km`; then, on a world with a sea (a main
 * liquid; the header's sea level is the lowest cell's otherwise), its `SEA LEVEL` in
 * `ELEVATION`'s form, `· SEA LEVEL -2.4 m`.
 *
 * @remarks
 * Stale, the figure takes one `S` after its second radius and the sea level its own
 * (decision-r10-t12-leans §4). Both are the body's constants, so each keeps its own width; the
 * figure's `S` has its place kept, so that the sea level never moves. A spheroid's two radii are
 * two runs ({@link spheroidFigure}; R10.T12.a), which may break after the `×`; a sphere's one
 * radius is one run.
 */
export function datumLine(header: SurfaceHeaderDto, stale: boolean): ReadoutLine {
  const tone = toneOf(stale);
  const a = header.equatorial_radius_m;
  const c = header.polar_radius_m;
  const figure: ReadoutSegment =
    a === c
      ? [consoleText("REFERENCE SPHERE "), field(`${radiusKm(a)} km`, 0, tone), mark(stale)]
      : spheroidFigure(a, c, stale);
  if (header.main_liquid === null) {
    return reading("DATUM", [figure]);
  }
  const seaLevel = `${formatNumber(header.sea_level_m, 1)} m`;
  return reading("DATUM", [
    figure,
    [consoleText("SEA LEVEL "), field(seaLevel, 0, tone), mark(stale)],
  ]);
}

/**
 * A `±` reading's segment: the value right-aligned in its part before the sign, then the
 * uncertainty left-aligned in the part after it with its `S` straight after it, in one field whose
 * width holds the mark's place; resolved, the `S` straight after the value and the part after it
 * kept blank. So the value's unit, the line's width and the reading after it keep their places from
 * Estimated to resolved and stale.
 */
function uncertainSegment(
  shown: FieldReading,
  valueCh: number,
  uncertaintyCh: number,
  stale: boolean,
): ReadoutSegment {
  const tone = toneOf(stale);
  const value = field(shown.value, valueCh, tone);
  return shown.uncertainty === null
    ? [value, mark(stale), field("", uncertaintyCh, tone, "start")]
    : [
        value,
        field(
          `${NO_BREAK_SPACE}${shown.uncertainty}`,
          uncertaintyCh + STALE_MARK_CH,
          tone,
          "start",
          stale,
        ),
      ];
}

/** A plain value's segment: the value right-aligned in its field, then its `S`'s place. */
function valueSegment(value: string | null, widthCh: number, stale: boolean): ReadoutSegment {
  // Missing takes no S: the em dash has no last value; its place is kept for what follows.
  return value === null
    ? [field(MISSING, widthCh, "missing"), mark(false)]
    : [field(value, widthCh, toneOf(stale)), mark(stale)];
}

/**
 * A name's segment: the name at its own width, then its `S`'s place, the mark in it while stale;
 * else the em dash, its place kept blank.
 */
function nameSegment(name: string | null, stale: boolean): ReadoutSegment {
  // The S's place is kept after a name, as after every value, so that the substance after the
  // form keeps its place as the readings go stale; the em dash takes none, its place kept blank.
  if (name === null) {
    return [field(MISSING, 0, "missing"), mark(false)];
  }
  return [field(name, 0, toneOf(stale)), mark(stale)];
}

/** What the readout's lines take besides the answer. */
export interface ReadoutContext {
  /** Whether the readings are stale: the survey's `TerrainSurvey.stale`. */
  readonly stale: boolean;
  /** Whether the slant range is the camera's off the hull, `SLANT RANGE FROM CAMERA`. */
  readonly slantFromCamera: boolean;
  /** The width of the pad the capacity was asked under, m (`GroundReadoutOptions.padWidthM`). */
  readonly padWidthM: number;
}

/** The readout's lines and the units they kept. */
export interface ReadoutLines {
  readonly lines: ReadonlyArray<ReadoutLine>;
  readonly units: ReadoutUnits;
}

/** The `ULTIMATE BEARING CAPACITY` line, and the unit it kept. */
function capacityLine(
  properties: SurfaceGroundProperties,
  resolved: boolean,
  context: ReadoutContext,
  previous: PressureUnit | null,
): { readonly line: ReadoutLine; readonly unit: PressureUnit | null } {
  const label = "ULTIMATE BEARING CAPACITY";
  const pa = properties.bearingCapacityPa;
  if (pa === null) {
    // Missing: the em dash, with no PAD WIDTH part, which nothing shown depends on.
    return { line: reading(label, [valueSegment(null, CAPACITY_FIELD_CH, false)]), unit: previous };
  }
  const shown = formatBearingCapacity(pa, previous);
  const value = `${resolved ? "" : ESTIMATED}${shown.value} ${shown.unit}`;
  return {
    line: reading(label, [
      valueSegment(value, CAPACITY_FIELD_CH, context.stale),
      [consoleText(`PAD WIDTH ${formatTwoFigures(context.padWidthM)} m`)],
    ]),
    unit: shown.unit,
  };
}

/**
 * The `PAD FRICTION` line: the coefficient to two decimals, `~` until resolved; `· AT` and the
 * temperature it was taken at where it depends on one, the em dash's included where that
 * temperature lies outside its source's (`— · AT 102 K`); plain `—` where no source gives it, or
 * where the temperature it depends on is not computed.
 */
function frictionLine(
  properties: SurfaceGroundProperties,
  resolved: boolean,
  stale: boolean,
): ReadoutLine {
  const { padFriction, frictionOnTemperature, temperatureK } = properties;
  const friction =
    padFriction === null ? null : `${resolved ? "" : ESTIMATED}${formatNumber(padFriction, 2)}`;
  const segments: ReadoutSegment[] = [valueSegment(friction, FRICTION_FIELD_CH, stale)];
  if (frictionOnTemperature && temperatureK !== null) {
    // Stated as taken, with no ~ (the guide's AT row), and stale with the server's data.
    const at = `${formatTemperatureK(temperatureK)} K`;
    segments.push([consoleText("AT "), ...valueSegment(at, TEMPERATURE_FIELD_CH, stale)]);
  }
  return reading("PAD FRICTION", segments);
}

/** The `MATERIAL` line: its form and its substance, each a name, never `~`. */
function materialLine(
  material: SurfaceGroundMaterial,
  palette: SurfaceHeaderDto["palette"],
  stale: boolean,
): ReadoutLine {
  return reading("MATERIAL", [
    nameSegment(materialFormName(material.classCode), stale),
    nameSegment(substanceOf(material.entry, palette), stale),
  ]);
}

/** The slant range's label: `SLANT RANGE FROM CAMERA` whenever it is the camera's off the hull. */
function slantLabel(fromCamera: boolean): string {
  return fromCamera ? "SLANT RANGE FROM CAMERA" : "SLANT RANGE";
}

/**
 * The terrain readout's lines (the guide's terrain readout): the heading, then, where the line of
 * sight meets surveyed ground, `ELEVATION`, `SLOPE` with its `· BASELINE`, `SLANT RANGE`,
 * `MATERIAL`, `ULTIMATE BEARING CAPACITY` with its `· PAD WIDTH`, `PAD FRICTION` with any `· AT`,
 * and `RESOLUTION`, in that order; or `NOT SURVEYED` or `NO GROUND` once in place of them all.
 *
 * @remarks
 * `ELEVATION`, `SLOPE`, `SLANT RANGE` and the two properties are Estimated until the survey
 * resolves the point (`SurfaceReading.resolved`, and the range's own); the properties with their
 * `~` alone.
 * Stale, each value the server's data gives takes one `S` (decision-r10-t12-leans §4): after its
 * uncertainty where it has one, after the form and after the substance, after `AT`'s temperature;
 * none after a status (`GRAZING`, `NOT SURVEYED`, `NO GROUND`), the em dash, `BASELINE` or
 * `PAD WIDTH`.
 */
export function groundReadoutLines(
  readout: SurfaceGroundReadout,
  header: SurfaceHeaderDto,
  context: ReadoutContext,
  units: ReadoutUnits,
): ReadoutLines {
  const heading: ReadoutLine = { kind: "heading", key: "ground-heading", text: GROUND_HEADING };
  switch (readout.kind) {
    case "no-ground":
      return { lines: [heading, status("ground-status", NO_GROUND, "text")], units };
    case "not-surveyed":
      return { lines: [heading, status("ground-status", NOT_SURVEYED, "text")], units };
    case "ground":
      break;
  }
  const { stale } = context;
  const point: SurfaceReading = readout.reading;
  const { resolved } = point;
  const elevation = formatElevationReading(
    point.elevationM,
    resolved ? null : point.elevationSigmaM,
  );
  const slope = formatSlopeReading(point.slopeRmsRad, resolved ? null : point.slopeSigmaRad);
  const range = formatSlantRange(readout.sight, units.slantRange);
  const rangeSegments: ReadonlyArray<ReadoutSegment> = range.grazing
    ? [valueSegment(range.value, SLANT_RANGE_FIELD_CH, stale), [{ kind: "status", text: GRAZING }]]
    : [uncertainSegment(range, SLANT_RANGE_FIELD_CH, SLANT_RANGE_UNCERTAINTY_CH, stale)];
  const capacity = capacityLine(readout.properties, resolved, context, units.capacity);
  const resolution = formatSurveyResolution(point.resolutionM, units.resolution);
  const lines: ReadoutLine[] = [
    heading,
    reading("ELEVATION", [
      uncertainSegment(elevation, ELEVATION_FIELD_CH, ELEVATION_UNCERTAINTY_CH, stale),
    ]),
    reading("SLOPE", [
      uncertainSegment(slope, SLOPE_FIELD_CH, SLOPE_UNCERTAINTY_CH, stale),
      [consoleText(`BASELINE ${formatNumber(point.slopeBaselineM, 0)} m`)],
    ]),
    reading(slantLabel(context.slantFromCamera), rangeSegments),
    materialLine(readout.material, header.palette, stale),
    capacity.line,
    frictionLine(readout.properties, resolved, stale),
    reading("RESOLUTION", [
      valueSegment(`${resolution.value} ${resolution.unit}`, RESOLUTION_FIELD_CH, stale),
    ]),
  ];
  return {
    lines,
    units: {
      ...units,
      slantRange: range.unit,
      capacity: capacity.unit,
      resolution: resolution.unit,
    },
  };
}

/**
 * Whether a view's terrain readout, its `DATUM` line and its tripod may stand
 * (decision-r10-t12-leans §5 and §7): a survey of the `FRAME` body held (its header, `NONE`
 * included), the body modelled,
 * no terrain fault on the view's fault lines (`isTerrainFault`, R10.T16.e), and the readout's first
 * answer held, so that nothing stands before it. Whether the view is the `PRIMARY` one is the
 * caller's (§3).
 *
 * @remarks
 * Where no line of sight can be formed no answer is held, yet `DATUM` and the readout's
 * `NOT AVAILABLE` line stand (`TerrainLabelInput.sightGap`; R10.T12.a), with no tripod:
 * {@link terrainLabelModel} states them, and this answers for the answer's readout alone.
 *
 * @param faults - The view's fault lines.
 * @param readout - The readout's latest answer for the view, `null` before its first.
 */
export function terrainReadoutStands(
  survey: TerrainSurvey | null,
  faults: ReadonlyArray<string>,
  readout: SurfaceGroundReadout | null,
): boolean {
  return readoutMayStand(survey, faults) && readout !== null;
}

/**
 * Whether a view's terrain readout could stand but for its answer ({@link terrainReadoutStands}'s
 * rule less the answer): a survey of the `FRAME` body held, the body modelled, no terrain fault.
 */
function readoutMayStand(survey: TerrainSurvey | null, faults: ReadonlyArray<string>): boolean {
  return (
    survey !== null &&
    survey.header !== null &&
    survey.status.notModelled === null &&
    !faults.some(isTerrainFault)
  );
}

/** What a view's terrain lines are made from. */
export interface TerrainLabelInput {
  /** The `FRAME` body's survey (`TerrainSession.survey`), `null` while it is not wanted. */
  readonly survey: TerrainSurvey | null;
  /** The readout's latest answer for the view (`TerrainSession.groundReadout`), else `null`. */
  readonly readout: SurfaceGroundReadout | null;
  /** The view's fault lines, the terrain's among them. */
  readonly faults: ReadonlyArray<string>;
  /** Whether the camera is off the hull or there is no own ship (`slantRangeFromCamera`). */
  readonly slantFromCamera: boolean;
  /** The width of the pad the capacity was asked under, m. */
  readonly padWidthM: number;
  /**
   * Whether a kept scene leaves the body's surface unset (decision-r10-t12-commit3 §5): the survey
   * line's place reads `TERRAIN: NOT YET MODELLED`, and no `DATUM` or readout stands; not by
   * default.
   */
  readonly surfaceUnset?: boolean | undefined;
  /**
   * What the survey line's two failed states end with (`surveyLines`' `remedy`): `VIEW`'s
   * `reopen VIEW to retry` unless given.
   */
  readonly remedy?: string | undefined;
  /**
   * Why the view's line of sight cannot be formed ({@link sightGapOf}; plan R10, R10.T12.a,
   * decision-r10-t12a-scope §3.2), while no answer is held: the readout then reads its
   * `NOT AVAILABLE` line by its cause ({@link sightGapLine}); none, a sight formed or not yet
   * asked, by default.
   */
  readonly sightGap?: SightGap | undefined;
}

/** A view's terrain lines, in the label block's order, and the units they kept. */
export interface TerrainLabelModel {
  /** The survey line, or its state in its place, and any state after it. */
  readonly survey: ReadonlyArray<ReadoutLine>;
  /** The `DATUM` line while the readout stands, else `null`. */
  readonly datum: ReadoutLine | null;
  /** The readout's heading and readings while it stands, else none. */
  readonly readout: ReadonlyArray<ReadoutLine>;
  readonly units: ReadoutUnits;
}

/**
 * A view's terrain lines (the guide's survey line, `DATUM` and terrain readout): the survey line
 * always ({@link surveyLines}), or `TERRAIN: NOT YET MODELLED` in its place where a kept scene
 * leaves the body's surface unset; while the readout may stand ({@link terrainReadoutStands}), the
 * `DATUM` line ({@link datumLine}) and the readout ({@link groundReadoutLines}), stale as the
 * survey is.
 *
 * @remarks
 * Where the readout could stand but its line of sight cannot be formed, no answer being held
 * (`TerrainLabelInput.sightGap`; plan R10, R10.T12.a, decision-r10-t12a-scope §3.2), the readout
 * is its heading and one `NOT AVAILABLE` status by the cause ({@link sightGapLine}), in `--text`,
 * steady, with no `S` on loss of the link, since it is a status; `DATUM` stands with it, as with
 * `NO GROUND`. There is no point, so the readout gives no answer for a tripod or a note.
 *
 * @param units - The units the view's last publish kept.
 */
export function terrainLabelModel(
  input: TerrainLabelInput,
  units: ReadoutUnits,
): TerrainLabelModel {
  if (input.surfaceUnset === true) {
    return {
      survey: [status("survey", TERRAIN_NOT_MODELLED, "text")],
      datum: null,
      readout: [],
      units,
    };
  }
  const survey = surveyLines(input.survey, units, input.remedy);
  const header = input.survey?.header ?? null;
  const { readout, sightGap } = input;
  if (header === null || !readoutMayStand(input.survey, input.faults)) {
    return { survey: survey.lines, datum: null, readout: [], units: survey.units };
  }
  const stale = input.survey?.stale ?? false;
  if (readout === null) {
    // Nothing stands before the first answer; where no line of sight can be formed, nothing is
    // asked, and the readout says why in the readings' place (R10.T12.a).
    return sightGap === undefined
      ? { survey: survey.lines, datum: null, readout: [], units: survey.units }
      : {
          survey: survey.lines,
          datum: datumLine(header, stale),
          readout: [
            { kind: "heading", key: "ground-heading", text: GROUND_HEADING },
            status("ground-status", sightGapLine(sightGap), "text"),
          ],
          units: survey.units,
        };
  }
  const ground = groundReadoutLines(
    readout,
    header,
    { stale, slantFromCamera: input.slantFromCamera, padWidthM: input.padWidthM },
    survey.units,
  );
  return {
    survey: survey.lines,
    datum: datumLine(header, stale),
    readout: ground.lines,
    units: ground.units,
  };
}

// --- The tallest forms --------------------------------------------------------------------------

/**
 * The baselines a terrain readout's `SLOPE` may be read over, m, shortest first: the guide's
 * `BASELINE` choices, the crate's `SLOPE_BASELINES` (`reading.rs`), and the buttons of the `STYLE`
 * panel's `BASELINE` row in their order (plan R10, R10.T12.a; decision-r10-t12a-scope §3.1).
 */
export const SLOPE_BASELINES_M: ReadonlyArray<SlopeBaselineM> = [2, 5, 100];

/**
 * The longest `BASELINE` the guide offers, m, taken from {@link SLOPE_BASELINES_M}: the tallest
 * `SLOPE` line is reckoned at it, so that its room holds whichever the `STYLE` panel's selector
 * sets (R10.T12.a).
 */
export const LONGEST_BASELINE_M: number = Math.max(...SLOPE_BASELINES_M);

/** The longest of a list of names, the first where two are as long; `null` for none. */
function longestOf(names: ReadonlyArray<string>): string | null {
  return names.reduce<string | null>(
    (longest, name) => (longest === null || name.length > longest.length ? name : longest),
    null,
  );
}

/** A view's terrain lines in their tallest forms, by which its lower plate's room is reckoned. */
export interface TallestTerrainLines {
  /** The survey line with both its values, stale. */
  readonly survey: ReadoutLine;
  /** `DATUM` as a spheroid with its `SEA LEVEL`, stale. */
  readonly datum: ReadoutLine;
  /** The readout's heading and every reading, Estimated, stale and grazing. */
  readonly readout: ReadonlyArray<ReadoutLine>;
}

/**
 * A view's terrain lines in their tallest forms (decision-r10-view-block-room §3): the survey line,
 * `DATUM` and the readout, each at least as wide as any form the model gives them for the body, so
 * that the lower plate's room, reckoned on them, never comes and goes with the readings.
 *
 * @remarks
 * - Every value fills its fixed field with its longest reading and takes its `S`: the survey line's
 *   `ABOVE 99.9 %` and `FINEST 36,000 km`; the readout's Estimated `ELEVATION`, `SLOPE` and
 *   `SLANT RANGE` with their widest uncertainties, the capacity's `~0.95 MPa`, the friction's
 *   `~0.71` with its `· AT 272 K`, and `RESOLUTION 36,000 km`.
 * - `DATUM` is the header's spheroid with both radii, as two runs ({@link spheroidFigure}), and its
 *   `SEA LEVEL`, whether or not the body has a sea or is a sphere.
 * - `SLANT RANGE` takes the longer label, `SLANT RANGE FROM CAMERA`, and both of its forms at once:
 *   its widest Estimated value and uncertainty, and the `· GRAZING` part that stands in the
 *   uncertainty's place where it grazes, so that it is as tall as either at any width.
 * - `MATERIAL` takes the longest form's name and the longest substance the header's palette names
 *   (or `UNLISTED MATERIAL` where that is longer), each stale.
 * - `SLOPE` takes the longest `BASELINE`, 100 m ({@link LONGEST_BASELINE_M}), so that the room
 *   holds whichever the `STYLE` panel's selector sets (R10.T12.a).
 *
 * @param padWidthM - The width of the pad the capacity is asked under, m (`ReadoutContext`).
 */
export function tallestTerrainLines(
  header: SurfaceHeaderDto,
  padWidthM: number,
): TallestTerrainLines {
  const stale = true;
  const tone = toneOf(stale);
  const survey = reading("SURVEY COVERAGE", [
    [field("ABOVE 99.9 %", COVERAGE_FIELD_CH, tone), mark(stale)],
    [consoleText("FINEST "), field("36,000 km", RESOLUTION_FIELD_CH, tone), mark(stale)],
  ]);
  const datum = reading("DATUM", [
    spheroidFigure(header.equatorial_radius_m, header.polar_radius_m, stale),
    [
      consoleText("SEA LEVEL "),
      field(`${formatNumber(header.sea_level_m, 1)} m`, 0, tone),
      mark(stale),
    ],
  ]);
  const substances = header.palette.map((named) => substanceName(named.substance, "upper"));
  const form = longestOf(MATERIAL_FORMS) ?? MISSING;
  const substance = longestOf([...substances, "UNLISTED MATERIAL"]) ?? MISSING;
  const estimated = (value: string, uncertainty: string): FieldReading => ({
    value: `${ESTIMATED}${value}`,
    uncertainty: `±${NO_BREAK_SPACE}${uncertainty}`,
  });
  const readout: ReadonlyArray<ReadoutLine> = [
    { kind: "heading", key: "ground-heading", text: GROUND_HEADING },
    reading("ELEVATION", [
      uncertainSegment(
        estimated("-99,999.9 m", "100,000 m"),
        ELEVATION_FIELD_CH,
        ELEVATION_UNCERTAINTY_CH,
        stale,
      ),
    ]),
    reading("SLOPE", [
      uncertainSegment(estimated("90°", "90°"), SLOPE_FIELD_CH, SLOPE_UNCERTAINTY_CH, stale),
      [consoleText(`BASELINE ${formatNumber(LONGEST_BASELINE_M, 0)} m`)],
    ]),
    reading(slantLabel(true), [
      uncertainSegment(
        estimated("5.00E-3 m", "10,000 km"),
        SLANT_RANGE_FIELD_CH,
        SLANT_RANGE_UNCERTAINTY_CH,
        stale,
      ),
      [{ kind: "status", text: GRAZING }],
    ]),
    reading("MATERIAL", [nameSegment(form, stale), nameSegment(substance, stale)]),
    reading("ULTIMATE BEARING CAPACITY", [
      valueSegment(`${ESTIMATED}0.95 MPa`, CAPACITY_FIELD_CH, stale),
      [consoleText(`PAD WIDTH ${formatTwoFigures(padWidthM)} m`)],
    ]),
    reading("PAD FRICTION", [
      valueSegment(`${ESTIMATED}0.71`, FRICTION_FIELD_CH, stale),
      [consoleText("AT "), ...valueSegment("272 K", TEMPERATURE_FIELD_CH, stale)],
    ]),
    reading("RESOLUTION", [valueSegment("36,000 km", RESOLUTION_FIELD_CH, stale)]),
  ];
  return { survey, datum, readout };
}

// --- The line of sight --------------------------------------------------------------------------

/**
 * The nearest a line of sight's origin may be to the body's centre, m: the binding's
 * (`wasm.rs`' `MIN_SIGHT_ORIGIN_M`).
 */
export const MIN_SIGHT_ORIGIN_M = 1;

/**
 * The farthest a line of sight's origin may be from the body's centre, and its longest reach, m:
 * the binding's 10¹² m (`wasm.rs`' `MAX_SIGHT_M`).
 */
export const MAX_SIGHT_M = 1e12;

/**
 * A view's line of sight in its `FRAME` body's body-fixed axes, as the terrain readout's query
 * takes it (`GroundSight`; plan R10, R10.T12): the ray through the picture's centre.
 *
 * @remarks
 * - Its origin is the camera from the body's centre, `relativeToCamera` of the body's frame origin
 *   negated, rotated into the body-fixed axes (`rotateToBodyFixed`), as `terrainPassViewOf` places
 *   a pass's camera.
 * - Its direction is the camera's forward, its view axes' −z (the third column of
 *   `rotationRows(pose.orientation)`, negated), which `projection.ts`' `project` puts at the
 *   picture's centre, rotated the same way.
 * - Its reach runs past the body's far side, the origin's distance plus the datum's equatorial
 *   radius and the field's realised relief, so that a line that misses reads `NO GROUND`; held to
 *   the binding's {@link MAX_SIGHT_M}, which the ground's near side is always within.
 * - `null` where the body's rotation is not known, or the origin lies outside the binding's limits
 *   ({@link MIN_SIGHT_ORIGIN_M} to {@link MAX_SIGHT_M} from the centre), or its distance from the
 *   centre is not finite; {@link sightGapOf} says which (R10.T12.a).
 *
 * @param pose - The pose the view draws from (`runPose`).
 */
export function lineOfSightOf(
  pose: CameraPose,
  scene: ViewScene,
  body: ViewBody,
  header: SurfaceHeaderDto,
): GroundSight | null {
  const start = sightStartOf(pose, scene, body);
  if (start.kind !== "start") {
    return null;
  }
  const { rotation, originM, fromCentreM } = start;
  const [r0, r1, r2] = rotationRows(pose.orientation);
  const direction = rotateToBodyFixed(rotation, vec3(-r0.z, -r1.z, -r2.z));
  const reachM = fromCentreM + header.equatorial_radius_m + Math.max(0, header.realised_relief_m);
  return { originM, direction, maxM: Math.min(MAX_SIGHT_M, reachM) };
}

/**
 * Why a view's line of sight cannot be formed, by {@link lineOfSightOf}'s three `null`s in its
 * order (plan R10, R10.T12.a; decision-r10-t12a-scope §3.2): the body's rotation not known
 * (`rotation`), the camera more than {@link MAX_SIGHT_M} from its centre (`far`), or within
 * {@link MIN_SIGHT_ORIGIN_M} of it (`centre`).
 */
export type SightGap = "rotation" | "far" | "centre";

/**
 * Where a view's line of sight starts, as {@link lineOfSightOf} and {@link sightGapOf} both read
 * it (R10.T12.a), so that the two agree by construction: the camera from the body's centre in its
 * body-fixed axes, m, within the binding's limits; or why none can start; or a distance that is
 * not finite, a broken invariant.
 */
type SightStart =
  | {
      readonly kind: "start";
      readonly rotation: NonNullable<ViewBody["rotation"]>;
      readonly originM: GroundSight["originM"];
      readonly fromCentreM: number;
    }
  | { readonly kind: "gap"; readonly gap: SightGap }
  | { readonly kind: "broken"; readonly fromCentreM: number };

/**
 * The start of `pose`'s line of sight to `body` ({@link SightStart}): its origin is
 * `relativeToCamera` of the body's frame origin negated, rotated into the body-fixed axes
 * (`rotateToBodyFixed`), tested against the binding's limits ({@link MIN_SIGHT_ORIGIN_M} to
 * {@link MAX_SIGHT_M}).
 */
function sightStartOf(pose: CameraPose, scene: ViewScene, body: ViewBody): SightStart {
  const rotation = body.rotation;
  if (rotation === null) {
    return { kind: "gap", gap: "rotation" };
  }
  const centreM = relativeToCamera(
    { kind: "body", body: body.id, m: vec3(0, 0, 0) },
    pose,
    sceneOrigins(scene),
  );
  const originM = rotateToBodyFixed(rotation, scale(centreM, -1));
  const fromCentreM = norm(originM);
  let start: SightStart;
  if (!Number.isFinite(fromCentreM)) {
    start = { kind: "broken", fromCentreM };
  } else if (fromCentreM > MAX_SIGHT_M) {
    start = { kind: "gap", gap: "far" };
  } else if (fromCentreM < MIN_SIGHT_ORIGIN_M) {
    start = { kind: "gap", gap: "centre" };
  } else {
    start = { kind: "start", rotation, originM, fromCentreM };
  }
  return start;
}

/**
 * Why {@link lineOfSightOf} forms no line of sight for the same pose and body (plan R10,
 * R10.T12.a; decision-r10-t12a-scope §3.2): its {@link SightGap}, tested in `lineOfSightOf`'s
 * order, the camera's distance from the body's centre reckoned as it reckons it; `null` exactly
 * where it forms one.
 *
 * @throws Error where that distance is not finite: no cause, but a broken invariant of the scene
 *   and the pose, which the readout's ask guards.
 */
export function sightGapOf(pose: CameraPose, scene: ViewScene, body: ViewBody): SightGap | null {
  const start = sightStartOf(pose, scene, body);
  let gap: SightGap | null;
  switch (start.kind) {
    case "start":
      gap = null;
      break;
    case "gap":
      gap = start.gap;
      break;
    case "broken":
      throw new Error(
        `the camera's distance from ${body.id}'s centre is not finite (${String(start.fromCentreM)} m)`,
      );
  }
  return gap;
}

/**
 * The readout's line where the body's rotation is not known, in whose axes it reads the ground:
 * the upper plate's `ROTATION: NOT YET MODELLED` as a cause (R10.T12.a; decision-r10-t12a-scope
 * §3.2).
 */
export const NO_SIGHT_ROTATION = "NOT AVAILABLE: ROTATION not yet modelled";

/** {@link MAX_SIGHT_M} in the view's range form, `6.68 AU` (10¹² m is 6.6846 AU). */
const FARTHEST_SIGHT = formatBodyDistance(MAX_SIGHT_M / 1_000, null);

/**
 * The readout's line where the camera is more than {@link MAX_SIGHT_M} from the body's centre,
 * beyond the readout's reach, its figure formatted from the constant (R10.T12.a;
 * decision-r10-t12a-scope §3.2).
 *
 * @remarks
 * The figure and its unit are joined by a no-break space, as every quantity on a label block is
 * kept whole (`.view-label__run`; decision-r07-t19-layout, 2), so that a narrow plate never breaks
 * `6.68` from `AU`.
 */
export const NO_SIGHT_FAR = `NOT AVAILABLE: camera more than ${FARTHEST_SIGHT.value}${NO_BREAK_SPACE}${FARTHEST_SIGHT.unit} from the body`;

/**
 * The readout's line where the camera is within {@link MIN_SIGHT_ORIGIN_M} of the body's centre
 * (R10.T12.a; decision-r10-t12a-scope §3.2).
 */
export const NO_SIGHT_CENTRE = "NOT AVAILABLE: camera at the body's centre";

/** The readout's `NOT AVAILABLE` line for a {@link SightGap}, in place of its readings. */
export function sightGapLine(gap: SightGap): string {
  let line: string;
  switch (gap) {
    case "rotation":
      line = NO_SIGHT_ROTATION;
      break;
    case "far":
      line = NO_SIGHT_FAR;
      break;
    case "centre":
      line = NO_SIGHT_CENTRE;
      break;
  }
  return line;
}
