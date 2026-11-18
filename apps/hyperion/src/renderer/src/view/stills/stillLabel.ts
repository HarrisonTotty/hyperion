/**
 * A still image's label (plan R11, R11.T10.c, Design note 17): its schema, its one text, and the
 * label of a still from what the view held at the request.
 *
 * @remarks
 * The label is a record: the scene's time, the camera's frame, position, orientation and field of
 * view, the exposure, the view's label block's readings of them, the setting, whether decoration was
 * drawn, the survey coverage and every annunciation that held. It is written twice, as the JSON
 * sidecar and as the PNG's `iTXt` chunk (`pngLabel.ts`), both {@link stillLabelText}'s one text, so
 * that the two read back equal.
 *
 * Pure, with no platform API and nothing imported but types: the main process (`main/stills.ts`)
 * and the preload (`preload/stillsApi.ts`) read labels through {@link parseStillLabel} too, which is
 * why this file is in `tsconfig.node.json`'s list. Every input is bounded before it is walked: a
 * label is made only of plain objects, arrays of bounded length and strings of bounded length.
 */

import type {
  QualitySettingName,
  StillCamera,
  StillCameraFrame,
  StillExposure,
  StillGalacticPosition,
  StillLabel,
  StillReading,
  StillSize,
  StillSurveyCoverage,
  StillTime,
} from "../../../../preload/api";

/** The label's form, its `schema` field. */
export const STILL_LABEL_SCHEMA = "hyperion-still/1";

/**
 * The annunciation a still carries when its frustum had not streamed to the level its pixels
 * warrant within `STILL_STREAMING_TIMEOUT_S` (Design note 17): the view's own `TERRAIN: STREAMING`
 * (`terrain/annunciation.ts`'s `TerrainAnnunciation`, to which `stillLabel.test.ts` holds it).
 */
export const STILL_TERRAIN_STREAMING = "TERRAIN: STREAMING";

/**
 * The annunciation a still carries whose terrain is drawn coarser than the high setting it states
 * (plan R11, R11.T10.e.3): the view's own `TERRAIN: DETAIL LIMITED`, the quality setting holding it
 * there (`terrain/annunciation.ts`' `TerrainAnnunciation`); {@link STILL_TERRAIN_STREAMING} stands
 * in its place where both hold.
 */
export const STILL_TERRAIN_DETAIL_LIMITED = "TERRAIN: DETAIL LIMITED";

/**
 * The longest side of a still, px: 8,192, WebGPU's default `maxTextureDimension2D` (the
 * specification's Limits table), which holds an 8K UHD still of 7,680 × 4,320.
 *
 * @remarks
 * Design note 17 bounds the tiles' sides and not the still's; this bound is plan R11's lean
 * (2026-11-13), the most the main process takes.
 */
export const STILL_MAX_SIDE_PX = 8192;

/** The most bodies a label's survey coverage names. */
export const STILL_SURVEY_MAX = 64;

/** The most annunciations a label carries. */
export const STILL_ANNUNCIATIONS_MAX = 64;

/** The most readings of the view's label block a label carries. */
export const STILL_READINGS_MAX = 32;

/**
 * The longest annunciation, reading, name or cause, UTF-16 code units: a label block's line is far
 * shorter.
 */
export const STILL_TEXT_MAX_CHARS = 240;

/** The longest craft key a camera's frame names, UTF-16 code units. */
const CRAFT_ID_MAX_CHARS = 128;

/** The bytes a label's text may take in UTF-8: its `iTXt` chunk's text and its sidecar's. */
export const STILL_LABEL_MAX_BYTES = 65_536;

/**
 * Metres in a light-year, c × one Julian year: 299,792,458 m/s × 31,557,600 s (the SI metre's c
 * and the IAU's Julian year of 365.25 days), the bound of a `GalacticPosition`'s offset.
 */
const METRES_PER_LY = 9_460_730_472_580_800;

/** A coarse field's levels as R09 serves them (`terrain/coverage.ts`'s `MIN_FIELD_LEVEL`, `MAX_FIELD_LEVEL`). */
const FIELD_LEVELS = { min: 5, max: 8 } as const;

/** A system's ID on the wire: 16 lowercase hexadecimal digits (`SystemIdHex`). */
const SYSTEM_ID = /^[0-9a-f]{16}$/u;

/** A body's ID on the wire: its system's, a full stop and four lowercase hex digits (`BodyIdHex`). */
const BODY_ID = /^[0-9a-f]{16}\.[0-9a-f]{4}$/u;

/**
 * Whether `words` holds a C0 or C1 control character, or a line or paragraph separator, none of
 * which a label's text holds: the PNG specification discourages them in an `iTXt` chunk's text
 * (§11.3.3.4), and a label block's line has none.
 */
function hasControl(words: string): boolean {
  for (let i = 0; i < words.length; i += 1) {
    const code = words.charCodeAt(i);
    if (code <= 0x1f || (code >= 0x7f && code <= 0x9f) || code === 0x2028 || code === 0x2029) {
      return true;
    }
  }
  return false;
}

/**
 * Whether `value` is a plain object, as JSON and IPC's structured clone make them: never an array,
 * a typed array or any other kind of object, whose keys could be many more than a label's.
 */
function isRecord(value: unknown): value is Record<string, unknown> {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const prototype: unknown = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

/** `value`'s fields where it is a plain object with exactly `keys`, else `null`. */
function exactly(value: unknown, keys: ReadonlyArray<string>): Record<string, unknown> | null {
  if (!isRecord(value)) {
    return null;
  }
  const own = Object.keys(value);
  return own.length === keys.length && keys.every((key) => Object.hasOwn(value, key))
    ? value
    : null;
}

/** `value` where it is a finite number, with `-0` as `0`, else `null`. */
function finite(value: unknown): number | null {
  // `+ 0` turns −0 into 0, which JSON writes as 0 anyway, so that a label and its text agree.
  return typeof value === "number" && Number.isFinite(value) ? value + 0 : null;
}

/** `value` where it is an array of three finite numbers, else `null`. */
function triple(value: unknown): readonly [number, number, number] | null {
  if (!Array.isArray(value) || value.length !== 3) {
    return null;
  }
  const a = finite(value[0]);
  const b = finite(value[1]);
  const c = finite(value[2]);
  return a === null || b === null || c === null ? null : [a, b, c];
}

/**
 * `value` where it is a string of 1 to `maxChars` units with no control character, and no space at
 * either end, else `null`.
 */
function text(value: unknown, maxChars: number): string | null {
  return typeof value === "string" &&
    value.length > 0 &&
    value.length <= maxChars &&
    value.trim() === value &&
    !hasControl(value)
    ? value
    : null;
}

function readTime(value: unknown): StillTime | null {
  const fields = exactly(value, ["seconds", "nanos"]);
  if (fields === null) {
    return null;
  }
  const seconds = finite(fields["seconds"]);
  const nanos = finite(fields["nanos"]);
  if (seconds === null || nanos === null || !Number.isSafeInteger(seconds)) {
    return null;
  }
  return Number.isInteger(nanos) && nanos >= 0 && nanos < 1e9 ? { seconds, nanos } : null;
}

function readGalactic(value: unknown): StillGalacticPosition | null {
  const fields = exactly(value, ["cell_ly", "offset_m"]);
  if (fields === null) {
    return null;
  }
  const cell = triple(fields["cell_ly"]);
  const offset = triple(fields["offset_m"]);
  if (cell === null || offset === null || !cell.every((ly) => Number.isSafeInteger(ly))) {
    return null;
  }
  return offset.every((m) => m >= 0 && m < METRES_PER_LY)
    ? { cell_ly: cell, offset_m: offset }
    : null;
}

function readFrame(value: unknown): StillCameraFrame | null {
  if (!isRecord(value)) {
    return null;
  }
  const kind = value["kind"];
  switch (kind) {
    case "galactic": {
      const fields = exactly(value, ["kind", "origin"]);
      const origin = fields === null ? null : readGalactic(fields["origin"]);
      return origin === null ? null : { kind, origin };
    }
    case "system": {
      const system = exactly(value, ["kind", "system"])?.["system"];
      return typeof system === "string" && SYSTEM_ID.test(system) ? { kind, system } : null;
    }
    case "body": {
      const body = exactly(value, ["kind", "body"])?.["body"];
      return typeof body === "string" && BODY_ID.test(body) ? { kind, body } : null;
    }
    case "craft": {
      const craft = text(exactly(value, ["kind", "craft"])?.["craft"], CRAFT_ID_MAX_CHARS);
      return craft === null ? null : { kind, craft };
    }
    default:
      return null;
  }
}

function readCamera(value: unknown): StillCamera | null {
  const fields = exactly(value, ["frame", "positionM", "orientation", "fovXDeg"]);
  if (fields === null) {
    return null;
  }
  const frame = readFrame(fields["frame"]);
  const positionM = triple(fields["positionM"]);
  const turn = exactly(fields["orientation"], ["w", "x", "y", "z"]);
  const fovXDeg = finite(fields["fovXDeg"]);
  if (frame === null || positionM === null || turn === null || fovXDeg === null) {
    return null;
  }
  const w = finite(turn["w"]);
  const x = finite(turn["x"]);
  const y = finite(turn["y"]);
  const z = finite(turn["z"]);
  if (w === null || x === null || y === null || z === null || !(fovXDeg > 0 && fovXDeg < 180)) {
    return null;
  }
  return { frame, positionM, orientation: { w, x, y, z }, fovXDeg };
}

function readExposure(value: unknown): StillExposure | null {
  if (!isRecord(value)) {
    return null;
  }
  const kind = value["kind"];
  switch (kind) {
    case "manual":
    case "auto": {
      const ev100 = finite(exactly(value, ["kind", "ev100"])?.["ev100"]);
      return ev100 === null ? null : { kind, ev100 };
    }
    case "inhibited": {
      const fields = exactly(value, ["kind", "ev100", "cause"]);
      const ev100 = finite(fields?.["ev100"]);
      const cause = text(fields?.["cause"], STILL_TEXT_MAX_CHARS);
      return ev100 === null || cause === null ? null : { kind, ev100, cause };
    }
    default:
      return null;
  }
}

function readCoverage(value: unknown): StillSurveyCoverage | null {
  const fields = exactly(value, ["body", "name", "level", "surveyedFraction", "bestResolutionM"]);
  if (fields === null) {
    return null;
  }
  const body = fields["body"];
  const name = text(fields["name"], STILL_TEXT_MAX_CHARS);
  const level = finite(fields["level"]);
  const fraction = finite(fields["surveyedFraction"]);
  const best = fields["bestResolutionM"];
  const bestM = finite(best);
  if (
    typeof body !== "string" ||
    !BODY_ID.test(body) ||
    name === null ||
    level === null ||
    !Number.isInteger(level) ||
    level < FIELD_LEVELS.min ||
    level > FIELD_LEVELS.max ||
    fraction === null ||
    fraction < 0 ||
    fraction > 1 ||
    (best !== null && (bestM === null || !(bestM > 0)))
  ) {
    return null;
  }
  return {
    body,
    name,
    level,
    surveyedFraction: fraction,
    bestResolutionM: best === null ? null : bestM,
  };
}

function readReading(value: unknown): StillReading | null {
  const fields = exactly(value, ["label", "value"]);
  const label = text(fields?.["label"], STILL_TEXT_MAX_CHARS);
  const shown = text(fields?.["value"], STILL_TEXT_MAX_CHARS);
  return label === null || shown === null ? null : { label, value: shown };
}

/** Whether `px` is a still's side: a whole number from 1 to {@link STILL_MAX_SIDE_PX}. */
export function isStillSide(px: number): boolean {
  return Number.isInteger(px) && px >= 1 && px <= STILL_MAX_SIDE_PX;
}

function readSize(value: unknown): StillSize | null {
  const fields = exactly(value, ["widthPx", "heightPx"]);
  if (fields === null) {
    return null;
  }
  const widthPx = finite(fields["widthPx"]);
  const heightPx = finite(fields["heightPx"]);
  return widthPx !== null && heightPx !== null && isStillSide(widthPx) && isStillSide(heightPx)
    ? { widthPx, heightPx }
    : null;
}

/**
 * Each of `value`'s entries read by `read`, or `null` where it is not an array of at most `max`
 * such, or two entries share a key by `keyOf`, so that a list of them keys each entry by its own.
 */
function readList<T>(
  value: unknown,
  max: number,
  read: (entry: unknown) => T | null,
  keyOf: (entry: T) => string,
): ReadonlyArray<T> | null {
  if (!Array.isArray(value) || value.length > max) {
    return null;
  }
  const entries: T[] = [];
  const keys = new Set<string>();
  for (const entry of value) {
    const each = read(entry);
    if (each === null || keys.has(keyOf(each))) {
      return null;
    }
    keys.add(keyOf(each));
    entries.push(each);
  }
  return entries;
}

/** The label's fields, in the order its text writes them. */
const LABEL_KEYS = [
  "schema",
  "time",
  "camera",
  "exposure",
  "readings",
  "setting",
  "decoration",
  "survey",
  "annunciations",
  "size",
] as const;

/**
 * `value` rebuilt as a label where it is one of the schema's, else `null`: every object made anew
 * with its fields in the schema's order, which is the order its JSON writes them in.
 */
function rebuild(value: unknown): StillLabel | null {
  const fields = exactly(value, LABEL_KEYS);
  if (fields === null || fields["schema"] !== STILL_LABEL_SCHEMA) {
    return null;
  }
  const time = readTime(fields["time"]);
  const camera = readCamera(fields["camera"]);
  const exposure = readExposure(fields["exposure"]);
  const readings = readList(
    fields["readings"],
    STILL_READINGS_MAX,
    readReading,
    (reading) => reading.label,
  );
  const setting = fields["setting"];
  const decoration = fields["decoration"];
  const survey = readList(fields["survey"], STILL_SURVEY_MAX, readCoverage, ({ body }) => body);
  const annunciations = readList(
    fields["annunciations"],
    STILL_ANNUNCIATIONS_MAX,
    (entry) => text(entry, STILL_TEXT_MAX_CHARS),
    (words) => words,
  );
  const size = readSize(fields["size"]);
  if (
    time === null ||
    camera === null ||
    exposure === null ||
    readings === null ||
    (setting !== "high" && setting !== "low") ||
    typeof decoration !== "boolean" ||
    survey === null ||
    annunciations === null ||
    size === null
  ) {
    return null;
  }
  return {
    schema: STILL_LABEL_SCHEMA,
    time,
    camera,
    exposure,
    readings,
    setting,
    decoration,
    survey,
    annunciations,
    size,
  };
}

/** The bytes `words` takes in UTF-8. */
function utf8Bytes(words: string): number {
  return new TextEncoder().encode(words).byteLength;
}

/**
 * `value` as a still's label where it is one of the schema's: every field present, of its type and
 * in its range, with no field besides and no list repeating an entry, and its text within
 * {@link STILL_LABEL_MAX_BYTES}; `null` otherwise.
 *
 * @remarks
 * The label returned is made anew, its fields in {@link stillLabelText}'s order and `-0` read as
 * `0`, so that a label read back from either of its copies is equal to the other, field for field.
 *
 * @param value - Anything: JSON parsed from a sidecar or a chunk, or an IPC argument.
 */
export function parseStillLabel(value: unknown): StillLabel | null {
  const label = rebuild(value);
  return label !== null && utf8Bytes(JSON.stringify(label)) <= STILL_LABEL_MAX_BYTES ? label : null;
}

/**
 * A label's one text: compact JSON with every object's fields in the schema's order, whatever order
 * the label was built in, which is the sidecar's whole content and the PNG's `iTXt` chunk's text.
 *
 * @throws RangeError for a label that is not one of the schema's ({@link parseStillLabel}).
 */
export function stillLabelText(label: StillLabel): string {
  const rebuilt = parseStillLabel(label);
  if (rebuilt === null) {
    throw new RangeError("not a still label of the schema");
  }
  return JSON.stringify(rebuilt);
}

/** What the view held at a still's request, from which its label is made (Design note 17). */
export interface StillRequest {
  readonly time: StillTime;
  readonly camera: StillCamera;
  readonly exposure: StillExposure;
  /** The view's label block's readings at the request, as it stated them. */
  readonly readings: ReadonlyArray<StillReading>;
  readonly setting: QualitySettingName;
  readonly decoration: boolean;
  readonly survey: ReadonlyArray<StillSurveyCoverage>;
  /** The annunciations that held at the request, as the view stated them. */
  readonly annunciations: ReadonlyArray<string>;
  readonly size: StillSize;
}

/**
 * The label of a still made at `request`: its annunciations those that held, each once, with
 * {@link STILL_TERRAIN_STREAMING} after them where its frustum had not streamed in time, in
 * {@link STILL_TERRAIN_DETAIL_LIMITED}'s place where that held (one terrain annunciation, as the
 * view's own line shows the more pressing).
 *
 * @throws RangeError where the label is not one of the schema's ({@link parseStillLabel}): the
 *   caller's bug, since everything in it is the view's own.
 */
export function stillLabel(request: StillRequest, streamed: boolean): StillLabel {
  // Each once, in the order the view stated them.
  const held = [...new Set(request.annunciations)];
  const annunciations = streamed
    ? held
    : [
        ...held.filter((words) => words !== STILL_TERRAIN_DETAIL_LIMITED),
        ...(held.includes(STILL_TERRAIN_STREAMING) ? [] : [STILL_TERRAIN_STREAMING]),
      ];
  const label = parseStillLabel({
    schema: STILL_LABEL_SCHEMA,
    time: request.time,
    camera: request.camera,
    exposure: request.exposure,
    readings: request.readings,
    setting: request.setting,
    decoration: request.decoration,
    survey: request.survey,
    annunciations,
    size: request.size,
  });
  if (label === null) {
    throw new RangeError("a still's request does not make a label of the schema");
  }
  return label;
}
