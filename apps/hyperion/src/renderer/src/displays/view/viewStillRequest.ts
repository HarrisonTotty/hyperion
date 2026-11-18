/**
 * What `VIEW` holds at a still's request as the still's label takes it (plan R11, R11.T10.b): its
 * scene's time, its camera, its exposure, its label block's readings and annunciations, and each
 * body's survey coverage, from its `CoverageView`.
 *
 * @remarks
 * Pure: the stage hands it what it holds and the still source (`PhotorealStills`) adds the still's
 * size. Every list is held to the label's bounds (`stillLabel.ts`): a reading or an annunciation
 * the schema would refuse, empty or over its length, is left out rather than failing the still, a
 * later one of a label already given is left out, and each list stops at its most.
 */

import type {
  StillCamera,
  StillCameraFrame,
  StillExposure,
  StillReading,
  StillSurveyCoverage,
} from "../../../../preload/api";
import type { BodyIdHex } from "@hyperion/protocol";

import type { CameraFrame } from "../../view/camera/pose";
import {
  controlEv100,
  type ExposureControl,
  exposureLevelReading,
} from "../../view/photometry/exposure";
import type { QualitySetting } from "../../view/quality/qualitySetting";
import type { ViewScene } from "../../view/scene/model";
import {
  STILL_ANNUNCIATIONS_MAX,
  STILL_READINGS_MAX,
  STILL_SURVEY_MAX,
  STILL_TEXT_MAX_CHARS,
  type StillRequest,
} from "../../view/stills/stillLabel";
import type { CoverageView } from "../../view/terrain/coverage";
import {
  type LabelLine,
  type LabelStatement,
  runPose,
  statementText,
  type ViewRun,
} from "./viewRun";

/** What the stage holds at a still's request, from which its label's inputs are made. */
export interface ViewStillInputs {
  readonly run: ViewRun;
  /** The exposure the view's frames are drawn at: its `AutoExposure`'s applied control. */
  readonly exposure: ExposureControl;
  /** The primary's label block's lines, whole, as last rendered. */
  readonly lines: ReadonlyArray<LabelLine>;
  /** The display's status lines over the stage, as last rendered: the scene's standing. */
  readonly scene: ReadonlyArray<string>;
  /** The block's faults, then its statements, as last rendered. */
  readonly faults: ReadonlyArray<string>;
  readonly statements: ReadonlyArray<LabelStatement>;
  readonly setting: QualitySetting;
  /** Whether the view's last frame drew decoration (`DECORATION ON`). */
  readonly decoration: boolean;
  /** A body's coverage as the view's terrain session holds it, or `null`. */
  readonly coverage: (body: BodyIdHex) => CoverageView | null;
}

/**
 * Whether `words` holds a C0 or C1 control character, or a line or paragraph separator, which a
 * label's text may not (`stillLabel.ts`, after the PNG specification's §11.3.3.4).
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

/** `words`, trimmed, where a label takes them: 1 to the label's most characters, none a control. */
function labelText(words: string): string | null {
  const trimmed = words.trim();
  return trimmed.length > 0 && trimmed.length <= STILL_TEXT_MAX_CHARS && !hasControl(trimmed)
    ? trimmed
    : null;
}

/**
 * A line's reading as the block shows it: each stale field with its `S` after its text, and a stale
 * line with its `S` after the whole (the guide's "Data states": never a stale value shown as live).
 */
function shownValue(line: LabelLine): string {
  let value = line.value;
  let from = 0;
  for (const field of line.fields ?? []) {
    const at = value.indexOf(field.text, from);
    if (at < 0) {
      continue;
    }
    from = at + field.text.length;
    if (field.stale) {
      value = `${value.slice(0, from)} ${STALE_MARK}${value.slice(from)}`;
      from += STALE_MARK.length + 1;
    }
  }
  return line.stale === true ? `${value} ${STALE_MARK}` : value;
}

/** The stale mark the block sets after a stale value (`StaleMark`). */
const STALE_MARK = "S";

/**
 * The label block's lines as a still's readings, each with its stale marks as the block shows
 * them: each label once, the first, at most 32.
 */
export function stillReadingsOf(lines: ReadonlyArray<LabelLine>): ReadonlyArray<StillReading> {
  const readings: StillReading[] = [];
  const labels = new Set<string>();
  for (const line of lines) {
    const label = labelText(line.label);
    const value = labelText(shownValue(line));
    if (label === null || value === null || labels.has(label)) {
      continue;
    }
    labels.add(label);
    readings.push({ label, value });
    if (readings.length === STILL_READINGS_MAX) {
      break;
    }
  }
  return readings;
}

/**
 * The display's status lines (`SCENE STALE: NO CARRIER`), then the block's faults and statements, as
 * a still's annunciations: each once, at most 64.
 */
export function stillAnnunciationsOf(
  scene: ReadonlyArray<string>,
  faults: ReadonlyArray<string>,
  statements: ReadonlyArray<LabelStatement>,
): ReadonlyArray<string> {
  const held: string[] = [];
  const all = [...scene, ...faults, ...statements.map((statement) => statementText(statement))];
  for (const words of all) {
    const text = labelText(words);
    if (text !== null && !held.includes(text)) {
      held.push(text);
      if (held.length === STILL_ANNUNCIATIONS_MAX) {
        break;
      }
    }
  }
  return held;
}

/** The camera's frame as a still records it. */
function stillFrameOf(frame: CameraFrame): StillCameraFrame {
  let still: StillCameraFrame;
  switch (frame.kind) {
    case "galactic":
      still = {
        kind: "galactic",
        origin: {
          cell_ly: [frame.origin.cell_ly[0], frame.origin.cell_ly[1], frame.origin.cell_ly[2]],
          offset_m: [frame.origin.offset_m[0], frame.origin.offset_m[1], frame.origin.offset_m[2]],
        },
      };
      break;
    case "system":
      still = { kind: "system", system: frame.system };
      break;
    case "body":
      still = { kind: "body", body: frame.body };
      break;
    case "craft":
      still = { kind: "craft", craft: frame.craft };
      break;
  }
  return still;
}

/** The run's camera as a still records it: its pose and horizontal field of view. */
export function stillCameraOf(run: ViewRun): StillCamera {
  const pose = runPose(run);
  const { w, x, y, z } = pose.orientation;
  return {
    frame: stillFrameOf(pose.frame),
    positionM: [pose.positionM.x, pose.positionM.y, pose.positionM.z],
    orientation: { w, x, y, z },
    fovXDeg: run.camera.fovDeg,
  };
}

/** The prefix of an inhibited exposure's reading before who or what inhibited it. */
const INHIBITED_PREFIX = "INHIBITED · ";

/**
 * The exposure as a still records it: its EV100 and automation level, an inhibit's cause in its
 * reading's words after the middle dot (`OPERATOR`, `NO IMAGE TO METER`).
 */
export function stillExposureOf(control: ExposureControl): StillExposure {
  const ev100 = controlEv100(control);
  let exposure: StillExposure;
  switch (control.kind) {
    case "manual":
    case "auto":
      exposure = { kind: control.kind, ev100 };
      break;
    case "inhibited": {
      const reading = exposureLevelReading(control);
      exposure = {
        kind: "inhibited",
        ev100,
        cause: reading.startsWith(INHIBITED_PREFIX)
          ? reading.slice(INHIBITED_PREFIX.length)
          : reading,
      };
      break;
    }
  }
  return exposure;
}

/**
 * Each of the scene's bodies' survey coverage, in the scene's order, from the view's terrain
 * session's `CoverageView`: its level, the fraction surveyed and the finest resolution; a body
 * with no coverage held is left out, and the list stops at 64.
 */
export function stillSurveyOf(
  scene: ViewScene,
  coverage: (body: BodyIdHex) => CoverageView | null,
): ReadonlyArray<StillSurveyCoverage> {
  const survey: StillSurveyCoverage[] = [];
  for (const body of scene.bodies) {
    const held = coverage(body.id);
    const name = labelText(body.designation.toUpperCase());
    if (held === null || name === null) {
      continue;
    }
    survey.push({
      body: body.id,
      name,
      level: held.level,
      surveyedFraction: held.surveyedFraction(),
      bestResolutionM: held.bestResolutionM(),
    });
    if (survey.length === STILL_SURVEY_MAX) {
      break;
    }
  }
  return survey;
}

/** The label's inputs at a still's request, all but its size, which the still source adds. */
export function stillRequestOf(inputs: ViewStillInputs): Omit<StillRequest, "size"> {
  const { run } = inputs;
  return {
    time: { seconds: run.scene.time.seconds, nanos: run.scene.time.nanos },
    camera: stillCameraOf(run),
    exposure: stillExposureOf(inputs.exposure),
    readings: stillReadingsOf(inputs.lines),
    setting: inputs.setting,
    decoration: inputs.decoration,
    survey: stillSurveyOf(run.scene, inputs.coverage),
    annunciations: stillAnnunciationsOf(inputs.scene, inputs.faults, inputs.statements),
  };
}
