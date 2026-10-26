/**
 * The check of what the several-views check's renderer sends the main process (plan R07, T20):
 * its record, which the IPC handler treats as `unknown` until it passes; and the terrain run's
 * (plan R10, R10.T16.d), the hand-over check's (R10.T10.g) and the atmosphere run's (plan R08,
 * R08.T10.b), checked the same way.
 */

import type {
  AtmosphereCheckCapture,
  AtmosphereCheckFrames,
  AtmosphereCheckRecord,
  HandoverAngles,
  HandoverCheckRecord,
  HandoverComparisonRecord,
  HandoverExposureRecord,
  HandoverFrameRecord,
  HandoverLookName,
  HandoverLookRecord,
  HandoverOwnMeans,
  HandoverRegime,
  HandoverSharedMeans,
  HandoverSwitchName,
  HandoverTexelRecord,
  TerrainCheckBytes,
  TerrainCheckFault,
  TerrainCheckLossRecord,
  TerrainCheckMemory,
  TerrainCheckPhaseName,
  TerrainCheckPhaseRecord,
  TerrainCheckRecord,
  TerrainCheckSessionFault,
  TerrainCheckOpeningRecord,
  TerrainCheckSessionCompile,
  TerrainCheckSessionRecord,
  TerrainCheckSpanRecord,
  TerrainCheckTeardownRecord,
  TerrainCheckViewRecord,
  ViewsCheckCanvas,
  ViewsCheckNamedCanvas,
  ViewsCheckPhaseName,
  ViewsCheckPhaseRecord,
  ViewsCheckRecord,
  ViewsCheckResizeRecord,
  ViewsCheckStyle,
  ViewsCheckViewName,
  ViewsCheckViewRecord,
} from "../preload/api";
import { HANDOVER_LOOK_RULES, HANDOVER_SWITCH_REGIMES } from "./handoverCheckResults";

type Rec = Readonly<Record<string, unknown>>;

function isRecord(value: unknown): value is Rec {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isCount(value: unknown): value is number {
  return isFiniteNumber(value) && Number.isInteger(value) && value >= 0;
}

function numbers(value: unknown): ReadonlyArray<number> | null {
  return Array.isArray(value) && value.every(isFiniteNumber) ? value : null;
}

function strings(value: unknown): ReadonlyArray<string> | null {
  return Array.isArray(value) && value.every((each) => typeof each === "string") ? value : null;
}

/** Every list member read by `read`, or `null` if any is not one. */
function listOf<T>(value: unknown, read: (each: unknown) => T | null): ReadonlyArray<T> | null {
  if (!Array.isArray(value)) {
    return null;
  }
  const out: T[] = [];
  for (const each of value) {
    const read1 = read(each);
    if (read1 === null) {
      return null;
    }
    out.push(read1);
  }
  return out;
}

const PHASES: ReadonlyArray<ViewsCheckPhaseName> = [
  "photoreal-alone",
  "photoreal-two-wireframe",
  "wireframe-two-wireframe",
  "wireframe-photoreal-wireframe",
  "wireframe-alone",
];

function phaseName(value: unknown): ViewsCheckPhaseName | null {
  return PHASES.find((name) => name === value) ?? null;
}

const VIEW_NAMES: ReadonlyArray<ViewsCheckViewName> = ["view", "instrument-1", "instrument-2"];

function viewName(value: unknown): ViewsCheckViewName | null {
  return VIEW_NAMES.find((name) => name === value) ?? null;
}

function style(value: unknown): ViewsCheckStyle | null {
  return value === "wireframe" || value === "photorealistic" ? value : null;
}

function canvas(value: unknown): ViewsCheckCanvas | null {
  return isRecord(value) && isCount(value["widthPx"]) && isCount(value["heightPx"])
    ? { widthPx: value["widthPx"], heightPx: value["heightPx"] }
    : null;
}

function namedCanvas(value: unknown): ViewsCheckNamedCanvas | null {
  if (!isRecord(value)) {
    return null;
  }
  const name = viewName(value["name"]);
  const size = canvas(value["canvas"]);
  return name === null || size === null ? null : { name, canvas: size };
}

function view(value: unknown): ViewsCheckViewRecord | null {
  if (!isRecord(value) || !isCount(value["draws"])) {
    return null;
  }
  const name = viewName(value["name"]);
  const drawn = style(value["style"]);
  const size = canvas(value["canvas"]);
  const gpuMs = numbers(value["gpuMs"]);
  const submitMs = numbers(value["submitMs"]);
  const passLabels = strings(value["passLabels"]);
  const scales = numbers(value["scales"]);
  if (
    name === null ||
    drawn === null ||
    size === null ||
    gpuMs === null ||
    submitMs === null ||
    passLabels === null ||
    scales === null
  ) {
    return null;
  }
  return {
    name,
    style: drawn,
    canvas: size,
    draws: value["draws"],
    gpuMs,
    submitMs,
    passLabels,
    scales,
  };
}

function phase(value: unknown): ViewsCheckPhaseRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const name = phaseName(value["name"]);
  const views = listOf(value["views"], view);
  const frameIntervalsMs = numbers(value["frameIntervalsMs"]);
  const primaryIntervalsMs = numbers(value["primaryIntervalsMs"]);
  const mainThreadMs = numbers(value["mainThreadMs"]);
  const frameGpuMs = numbers(value["frameGpuMs"]);
  const { startMs, endMs, untimedFrames, droppedResolves, unattributedGpuMs } = value;
  if (
    name === null ||
    views === null ||
    frameIntervalsMs === null ||
    primaryIntervalsMs === null ||
    mainThreadMs === null ||
    frameGpuMs === null ||
    !isFiniteNumber(startMs) ||
    !isFiniteNumber(endMs) ||
    endMs < startMs ||
    !isCount(untimedFrames) ||
    !isCount(droppedResolves) ||
    !isFiniteNumber(unattributedGpuMs)
  ) {
    return null;
  }
  return {
    name,
    startMs,
    endMs,
    views,
    frameIntervalsMs,
    primaryIntervalsMs,
    mainThreadMs,
    frameGpuMs,
    untimedFrames,
    droppedResolves,
    unattributedGpuMs,
  };
}

function resize(value: unknown): ViewsCheckResizeRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const before = listOf(value["before"], namedCanvas);
  const steps = listOf(value["steps"], (step) => {
    if (
      !isRecord(step) ||
      !isFiniteNumber(step["widthFraction"]) ||
      !isFiniteNumber(step["longestFrameMs"])
    ) {
      return null;
    }
    const views = listOf(step["views"], namedCanvas);
    return views === null
      ? null
      : {
          widthFraction: step["widthFraction"],
          longestFrameMs: step["longestFrameMs"],
          views,
        };
  });
  const allocations = listOf(
    value["allocations"],
    (event): ViewsCheckResizeRecord["allocations"][number] | null => {
      if (!isRecord(event) || typeof event["name"] !== "string") {
        return null;
      }
      const kind = event["kind"];
      return kind === "created" || kind === "destroyed" ? { kind, name: event["name"] } : null;
    },
  );
  return before === null || steps === null || allocations === null
    ? null
    : { before, steps, allocations };
}

/** `value` as the check's record, or `null` if it is not one. */
export function readViewsCheckRecord(value: unknown): ViewsCheckRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const { timer, devicePixelRatio, perCanvasOverheadMs } = value;
  const phases = listOf(value["phases"], phase);
  const resized = resize(value["resize"]);
  const faults = strings(value["faults"]);
  if (
    (timer !== "full" && timer !== "quantized" && timer !== "absent") ||
    !isFiniteNumber(devicePixelRatio) ||
    devicePixelRatio <= 0 ||
    !isFiniteNumber(perCanvasOverheadMs) ||
    phases === null ||
    resized === null ||
    faults === null
  ) {
    return null;
  }
  return { timer, devicePixelRatio, phases, resize: resized, faults, perCanvasOverheadMs };
}

/** A finite number or `null`, `undefined` where it is neither. */
function finiteOrNull(value: unknown): number | null | undefined {
  return value === null ? null : isFiniteNumber(value) ? value : undefined;
}

/** The terrain run's phases (`TerrainCheckPhaseName`). */
const TERRAIN_PHASES: ReadonlyArray<TerrainCheckPhaseName> = [
  "terrain-alone",
  "terrain-photoreal-wireframe",
  "terrain-wireframe",
  "terrain-loss",
];

function terrainPhaseName(value: unknown): TerrainCheckPhaseName | null {
  return TERRAIN_PHASES.find((name) => name === value) ?? null;
}

function bytesOf(value: unknown): TerrainCheckBytes | null {
  return isRecord(value) && typeof value["category"] === "string" && isCount(value["bytes"])
    ? { category: value["category"], bytes: value["bytes"] }
    : null;
}

function memoryOf(value: unknown): TerrainCheckMemory | null {
  if (!isRecord(value)) {
    return null;
  }
  const { workerFieldBytes, keptFieldBytes } = value;
  const sessionGpuBytes = listOf(value["sessionGpuBytes"], bytesOf);
  const liveBytes = listOf(value["liveBytes"], bytesOf);
  const peakBytes = listOf(value["peakBytes"], bytesOf);
  return isCount(workerFieldBytes) &&
    isCount(keptFieldBytes) &&
    sessionGpuBytes !== null &&
    liveBytes !== null &&
    peakBytes !== null
    ? { workerFieldBytes, keptFieldBytes, sessionGpuBytes, liveBytes, peakBytes }
    : null;
}

function terrainView(value: unknown): TerrainCheckViewRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const name = viewName(value["name"]);
  const drawn = style(value["style"]);
  const size = canvas(value["canvas"]);
  const gpuMs = numbers(value["gpuMs"]);
  const submitMs = numbers(value["submitMs"]);
  const firstHandoverMs = finiteOrNull(value["firstHandoverMs"]);
  const firstHandoverScriptS = finiteOrNull(value["firstHandoverScriptS"]);
  const firstTerrainMs = finiteOrNull(value["firstTerrainMs"]);
  const terrainHandoverMs = finiteOrNull(value["terrainHandoverMs"]);
  const longestGapEndScriptS = finiteOrNull(value["longestGapEndScriptS"]);
  const { draws, missedFrames, longestGapMs, handedOverDraws, offSurveyDraws } = value;
  const { terrainDraws, terrainGaps } = value;
  const terrainGapSpansS = listOf(value["terrainGapSpansS"], (span) =>
    Array.isArray(span) && span.length === 2 && isFiniteNumber(span[0]) && isFiniteNumber(span[1])
      ? ([span[0], span[1]] as const)
      : null,
  );
  const { readBacks, nonFiniteTexels } = value;
  if (
    name === null ||
    drawn === null ||
    size === null ||
    gpuMs === null ||
    submitMs === null ||
    firstHandoverMs === undefined ||
    firstHandoverScriptS === undefined ||
    firstTerrainMs === undefined ||
    terrainHandoverMs === undefined ||
    longestGapEndScriptS === undefined ||
    !isCount(draws) ||
    !isCount(missedFrames) ||
    !isFiniteNumber(longestGapMs) ||
    longestGapMs < 0 ||
    !isCount(handedOverDraws) ||
    !isCount(offSurveyDraws) ||
    terrainGapSpansS === null ||
    !isCount(terrainDraws) ||
    !isCount(terrainGaps) ||
    !isCount(readBacks) ||
    !isCount(nonFiniteTexels)
  ) {
    return null;
  }
  return {
    name,
    style: drawn,
    canvas: size,
    draws,
    gpuMs,
    submitMs,
    missedFrames,
    longestGapMs,
    longestGapEndScriptS,
    handedOverDraws,
    offSurveyDraws,
    terrainDraws,
    terrainGaps,
    terrainGapSpansS,
    firstHandoverMs,
    firstHandoverScriptS,
    firstTerrainMs,
    terrainHandoverMs,
    readBacks,
    nonFiniteTexels,
  };
}

function terrainPhase(value: unknown): TerrainCheckPhaseRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const name = terrainPhaseName(value["name"]);
  const views = listOf(value["views"], terrainView);
  const frameIntervalsMs = numbers(value["frameIntervalsMs"]);
  const mainThreadMs = numbers(value["mainThreadMs"]);
  const frameGpuMs = numbers(value["frameGpuMs"]);
  const patchesDrawn = numbers(value["patchesDrawn"]);
  const bakes = value["bakes"];
  const posts = value["fieldPosts"];
  const memory = value["memory"] === null ? null : memoryOf(value["memory"]);
  const { startMs, endMs, scriptStartS, scriptEndS, fieldAsks, blocksSent } = value;
  if (
    name === null ||
    views === null ||
    frameIntervalsMs === null ||
    mainThreadMs === null ||
    frameGpuMs === null ||
    patchesDrawn === null ||
    !patchesDrawn.every(isCount) ||
    !isFiniteNumber(startMs) ||
    !isFiniteNumber(endMs) ||
    endMs < startMs ||
    !isFiniteNumber(scriptStartS) ||
    !isFiniteNumber(scriptEndS) ||
    !isCount(fieldAsks) ||
    !isCount(blocksSent) ||
    !isRecord(bakes) ||
    !isCount(bakes["posted"]) ||
    !isCount(bakes["baked"]) ||
    !isCount(bakes["refused"]) ||
    !isRecord(posts) ||
    !isCount(posts["inits"]) ||
    !isCount(posts["chunks"]) ||
    !isCount(posts["releases"]) ||
    (value["memory"] !== null && memory === null)
  ) {
    return null;
  }
  return {
    name,
    startMs,
    endMs,
    scriptStartS,
    scriptEndS,
    views,
    frameIntervalsMs,
    mainThreadMs,
    frameGpuMs,
    patchesDrawn,
    bakes: { posted: bakes["posted"], baked: bakes["baked"], refused: bakes["refused"] },
    fieldPosts: { inits: posts["inits"], chunks: posts["chunks"], releases: posts["releases"] },
    fieldAsks,
    blocksSent,
    memory,
  };
}

const FAULT_KINDS: ReadonlyArray<TerrainCheckFault["kind"]> = [
  "device-lost",
  "gpu-process-gone",
  "shader-refused",
  "view-refused",
];

function fault(value: unknown): TerrainCheckFault | null {
  if (!isRecord(value)) {
    return null;
  }
  const kind = FAULT_KINDS.find((each) => each === value["kind"]) ?? null;
  const { atMs, reason, line } = value;
  return kind !== null &&
    isFiniteNumber(atMs) &&
    (reason === null || typeof reason === "string") &&
    typeof line === "string"
    ? { atMs, kind, reason, line }
    : null;
}

function sessionFault(value: unknown): TerrainCheckSessionFault | null {
  if (!isRecord(value)) {
    return null;
  }
  const { atMs, text } = value;
  const views = strings(value["views"]);
  return isFiniteNumber(atMs) && typeof text === "string" && views !== null
    ? { atMs, text, views }
    : null;
}

function loss(value: unknown): TerrainCheckLossRecord | null {
  if (!isRecord(value) || !isRecord(value["after"]) || !isRecord(value["masks"])) {
    return null;
  }
  const after = value["after"];
  const masks = value["masks"];
  const destroyedAtMs = finiteOrNull(value["destroyedAtMs"]);
  const destroyedAtScriptS = finiteOrNull(value["destroyedAtScriptS"]);
  const terrainAgainAtMs = finiteOrNull(value["terrainAgainAtMs"]);
  const before = finiteOrNull(masks["before"]);
  const afterRestore = finiteOrNull(masks["afterRestore"]);
  const end = finiteOrNull(masks["end"]);
  const { notDestroyed, blocksToCome, bakesInFlight, workers } = value;
  const { expected, beforeKept, endAsExpected } = masks;
  if (
    destroyedAtMs === undefined ||
    destroyedAtScriptS === undefined ||
    terrainAgainAtMs === undefined ||
    before === undefined ||
    afterRestore === undefined ||
    end === undefined ||
    !(notDestroyed === null || typeof notDestroyed === "string") ||
    !isCount(blocksToCome) ||
    !isCount(bakesInFlight) ||
    !isCount(workers) ||
    !isCount(after["fieldAsks"]) ||
    !isCount(after["fieldInits"]) ||
    !isCount(after["fieldChunks"]) ||
    !isCount(after["blocksSent"]) ||
    !isCount(expected) ||
    typeof beforeKept !== "boolean" ||
    typeof endAsExpected !== "boolean"
  ) {
    return null;
  }
  return {
    destroyedAtMs,
    destroyedAtScriptS,
    notDestroyed,
    blocksToCome,
    bakesInFlight,
    after: {
      fieldAsks: after["fieldAsks"],
      fieldInits: after["fieldInits"],
      fieldChunks: after["fieldChunks"],
      blocksSent: after["blocksSent"],
    },
    workers,
    terrainAgainAtMs,
    masks: { before, afterRestore, end, expected, beforeKept, endAsExpected },
  };
}

function teardown(value: unknown): TerrainCheckTeardownRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const liveBytes = listOf(value["liveBytes"], bytesOf);
  const { disposed, workersMade, workersLive, sessionGpuBytes } = value;
  return typeof disposed === "boolean" &&
    isCount(workersMade) &&
    isCount(workersLive) &&
    isCount(sessionGpuBytes) &&
    liveBytes !== null
    ? { disposed, workersMade, workersLive, sessionGpuBytes, liveBytes }
    : null;
}

/** A session the terrain run saw made, and its materials' compile (R10.T14.d). */
function sessionMade(value: unknown): TerrainCheckSessionRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const { madeAtMs } = value;
  const compile = sessionCompile(value["compile"]);
  return isFiniteNumber(madeAtMs) && compile !== null ? { madeAtMs, compile } : null;
}

/** A session's materials' compile, pending or ended (R10.T14.d). */
function sessionCompile(value: unknown): TerrainCheckSessionCompile | null {
  if (!isRecord(value)) {
    return null;
  }
  if (value["kind"] === "pending") {
    return { kind: "pending" };
  }
  const { compileMs, compiled } = value;
  const longestFrameGapMs = finiteOrNull(value["longestFrameGapMs"]);
  return value["kind"] === "ended" &&
    isFiniteNumber(compileMs) &&
    compileMs >= 0 &&
    isCount(compiled) &&
    longestFrameGapMs !== undefined &&
    (longestFrameGapMs === null || longestFrameGapMs >= 0)
    ? { kind: "ended", compileMs, compiled, longestFrameGapMs }
    : null;
}

/** The terrain run's judged span (R10.T14.d). */
function spanOf(value: unknown): TerrainCheckSpanRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const { fromMs, toMs, longestFrameGapMs, longestGapEndMs, harnessMs } = value;
  const longestGapEndScriptS = finiteOrNull(value["longestGapEndScriptS"]);
  return isFiniteNumber(fromMs) &&
    isFiniteNumber(toMs) &&
    toMs >= fromMs &&
    isFiniteNumber(longestFrameGapMs) &&
    longestFrameGapMs >= 0 &&
    isFiniteNumber(longestGapEndMs) &&
    longestGapEndScriptS !== undefined &&
    isFiniteNumber(harnessMs) &&
    harnessMs >= 0
    ? { fromMs, toMs, longestFrameGapMs, longestGapEndMs, longestGapEndScriptS, harnessMs }
    : null;
}

/** VIEW's opening as the terrain run saw it (R10.T14.d). */
function openingOf(value: unknown): TerrainCheckOpeningRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const { preparedMs, preparations } = value;
  const longestFrameGapMs = finiteOrNull(value["longestFrameGapMs"]);
  return isFiniteNumber(preparedMs) &&
    preparedMs >= 0 &&
    isCount(preparations) &&
    longestFrameGapMs !== undefined &&
    (longestFrameGapMs === null || longestFrameGapMs >= 0)
    ? { preparedMs, preparations, longestFrameGapMs }
    : null;
}

/** `value` as the terrain run's record (R10.T16.d), or `null` if it is not one. */
export function readTerrainCheckRecord(value: unknown): TerrainCheckRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const { timer, devicePixelRatio } = value;
  const phases = listOf(value["phases"], terrainPhase);
  const faults = listOf(value["faults"], fault);
  const terrainFaults = listOf(value["terrainFaults"], sessionFault);
  const sessions = listOf(value["sessions"], sessionMade);
  const opening = openingOf(value["opening"]);
  const span = spanOf(value["span"]);
  const restoresAtMs = numbers(value["restoresAtMs"]);
  const lost = loss(value["loss"]);
  const torn = teardown(value["teardown"]);
  const handover = value["handover"];
  const range =
    handover === null
      ? null
      : isRecord(handover) &&
          isFiniteNumber(handover["rangeM"]) &&
          handover["rangeM"] > 0 &&
          isFiniteNumber(handover["scriptS"])
        ? { rangeM: handover["rangeM"], scriptS: handover["scriptS"] }
        : undefined;
  if (
    (timer !== "full" && timer !== "quantized" && timer !== "absent") ||
    !isFiniteNumber(devicePixelRatio) ||
    devicePixelRatio <= 0 ||
    phases === null ||
    faults === null ||
    terrainFaults === null ||
    sessions === null ||
    opening === null ||
    span === null ||
    restoresAtMs === null ||
    lost === null ||
    torn === null ||
    range === undefined
  ) {
    return null;
  }
  return {
    timer,
    devicePixelRatio,
    phases,
    faults,
    terrainFaults,
    sessions,
    opening,
    span,
    restoresAtMs,
    loss: lost,
    teardown: torn,
    handover: range,
  };
}

/** A finite number of zero or more. */
function isNonNegative(value: unknown): value is number {
  return isFiniteNumber(value) && value >= 0;
}

const HANDOVER_LOOK_NAMES: ReadonlyArray<HandoverLookName> = [
  "handover-30",
  "handover-90",
  "handover-terminator",
];

const HANDOVER_REGIMES: ReadonlyArray<HandoverRegime> = ["disc", "figure", "terrain", "none"];

const HANDOVER_SWITCHES: ReadonlyArray<HandoverSwitchName> = [
  "disc-to-figure",
  "disc-to-terrain",
  "terrain-to-disc",
];

function handoverAngles(value: unknown): HandoverAngles | null {
  if (!isRecord(value)) {
    return null;
  }
  const { incidenceDeg, emissionDeg, phaseDeg } = value;
  return isNonNegative(incidenceDeg) && isNonNegative(emissionDeg) && isNonNegative(phaseDeg)
    ? { incidenceDeg, emissionDeg, phaseDeg }
    : null;
}

function handoverFrame(value: unknown): HandoverFrameRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const regime = HANDOVER_REGIMES.find((each) => each === value["regime"]) ?? null;
  const target = canvas(value["target"]);
  const angles = handoverAngles(value["angles"]);
  const { atMs, scriptS, rangeRatio, bodyPixels, bodyMisses, surveyPixels } = value;
  if (
    regime === null ||
    target === null ||
    angles === null ||
    !isFiniteNumber(atMs) ||
    !isNonNegative(scriptS) ||
    !isFiniteNumber(rangeRatio) ||
    !(rangeRatio > 0) ||
    !isCount(bodyPixels) ||
    !isCount(bodyMisses) ||
    !isCount(surveyPixels) ||
    bodyMisses > bodyPixels ||
    surveyPixels > bodyPixels
  ) {
    return null;
  }
  return {
    atMs,
    scriptS,
    regime,
    rangeRatio,
    target,
    bodyPixels,
    bodyMisses,
    surveyPixels,
    angles,
  };
}

function ownMeans(value: unknown): HandoverOwnMeans | null {
  if (!isRecord(value)) {
    return null;
  }
  const { pixelsBefore, pixelsAfter, meanBefore, meanAfter } = value;
  return isCount(pixelsBefore) &&
    isCount(pixelsAfter) &&
    isNonNegative(meanBefore) &&
    isNonNegative(meanAfter)
    ? { pixelsBefore, pixelsAfter, meanBefore, meanAfter }
    : null;
}

function sharedMeans(value: unknown): HandoverSharedMeans | null {
  if (!isRecord(value)) {
    return null;
  }
  const { pixels, meanBefore, meanAfter } = value;
  return isCount(pixels) && isNonNegative(meanBefore) && isNonNegative(meanAfter)
    ? { pixels, meanBefore, meanAfter }
    : null;
}

/** A body's or a survey's means, own and shared, or `null`. */
function meansPair(
  value: unknown,
): { readonly own: HandoverOwnMeans; readonly shared: HandoverSharedMeans } | null {
  if (!isRecord(value)) {
    return null;
  }
  const own = ownMeans(value["own"]);
  const shared = sharedMeans(value["shared"]);
  return own === null || shared === null ? null : { own, shared };
}

function handoverTexel(value: unknown): HandoverTexelRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const { texel, sharedPixels, meanBefore, meanAfter } = value;
  return isCount(texel) &&
    isCount(sharedPixels) &&
    sharedPixels >= 1 &&
    isNonNegative(meanBefore) &&
    isNonNegative(meanAfter)
    ? { texel, sharedPixels, meanBefore, meanAfter }
    : null;
}

function handoverComparison(value: unknown): HandoverComparisonRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const name = HANDOVER_SWITCHES.find((each) => each === value["name"]) ?? null;
  const before = handoverFrame(value["before"]);
  const after = handoverFrame(value["after"]);
  const body = meansPair(value["body"]);
  const survey = meansPair(value["survey"]);
  const texels = listOf(value["texels"], handoverTexel);
  const { framesApart, beforeWasLast, afterWasFirst } = value;
  if (
    name === null ||
    before === null ||
    after === null ||
    body === null ||
    survey === null ||
    texels === null ||
    !isCount(framesApart) ||
    framesApart < 1 ||
    typeof beforeWasLast !== "boolean" ||
    typeof afterWasFirst !== "boolean" ||
    // The frames hold the regimes the switch is between.
    !HANDOVER_SWITCH_REGIMES[name].before.includes(before.regime) ||
    !HANDOVER_SWITCH_REGIMES[name].after.includes(after.regime) ||
    // In texel order, each once.
    !texels.every((each, i) => i === 0 || (texels[i - 1]?.texel ?? -1) < each.texel)
  ) {
    return null;
  }
  return {
    name,
    before,
    after,
    framesApart,
    beforeWasLast,
    afterWasFirst,
    body,
    survey,
    texels,
  };
}

function handoverExposure(value: unknown): HandoverExposureRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const lockedEv100 = finiteOrNull(value["lockedEv100"]);
  const { lockedReading, endReading, held } = value;
  return lockedEv100 !== undefined &&
    (lockedReading === null || typeof lockedReading === "string") &&
    (endReading === null || typeof endReading === "string") &&
    typeof held === "boolean"
    ? { lockedEv100, lockedReading, endReading, held }
    : null;
}

function handoverLook(value: unknown): HandoverLookRecord | null {
  if (!isRecord(value) || !isRecord(value["switchesS"])) {
    return null;
  }
  const name = HANDOVER_LOOK_NAMES.find((each) => each === value["name"]) ?? null;
  const nominal = handoverAngles(value["nominal"]);
  const atRange = handoverAngles(value["atRange"]);
  const exposure = handoverExposure(value["exposure"]);
  const comparisons = listOf(value["comparisons"], handoverComparison);
  const failures = strings(value["failures"]);
  const switches = value["switchesS"];
  const figure = finiteOrNull(switches["figure"]);
  const terrain = finiteOrNull(switches["terrain"]);
  const disc = finiteOrNull(switches["disc"]);
  const { scene, judged, surveyedCells, mappingMismatches } = value;
  if (
    name === null ||
    nominal === null ||
    atRange === null ||
    exposure === null ||
    comparisons === null ||
    failures === null ||
    figure === undefined ||
    terrain === undefined ||
    disc === undefined ||
    // The look's name decides its scene and whether it is judged (R10.T10.g).
    scene !== HANDOVER_LOOK_RULES[name].scene ||
    judged !== HANDOVER_LOOK_RULES[name].judged ||
    !isCount(surveyedCells) ||
    !isCount(mappingMismatches) ||
    // Each switch at most once.
    new Set(comparisons.map((each) => each.name)).size !== comparisons.length
  ) {
    return null;
  }
  return {
    name,
    ...HANDOVER_LOOK_RULES[name],
    nominal,
    atRange,
    exposure,
    surveyedCells,
    mappingMismatches,
    switchesS: { figure, terrain, disc },
    comparisons,
    failures,
  };
}

/** `value` as the hand-over check's record (R10.T10.g), or `null` if it is not one. */
export function readHandoverCheckRecord(value: unknown): HandoverCheckRecord | null {
  if (!isRecord(value) || !isRecord(value["approach"])) {
    return null;
  }
  const looks = listOf(value["looks"], handoverLook);
  const faults = listOf(value["faults"], fault);
  const size = canvas(value["canvas"]);
  const target = canvas(value["target"]);
  const { world, devicePixelRatio, reliefM, tauPx, radPerPx, rangeM, cellLevel } = value;
  const approach = value["approach"];
  const { ratePerS, startRatio, closestRatio, holdS, endRatio } = approach;
  if (
    looks === null ||
    faults === null ||
    size === null ||
    target === null ||
    typeof world !== "string" ||
    !isFiniteNumber(devicePixelRatio) ||
    !(devicePixelRatio > 0) ||
    !isFiniteNumber(reliefM) ||
    !(reliefM > 0) ||
    !isFiniteNumber(tauPx) ||
    !(tauPx > 0) ||
    !isFiniteNumber(radPerPx) ||
    !(radPerPx > 0) ||
    !isFiniteNumber(rangeM) ||
    !(rangeM > 0) ||
    !isCount(cellLevel) ||
    !isFiniteNumber(ratePerS) ||
    !(ratePerS > 0) ||
    !isFiniteNumber(startRatio) ||
    !isFiniteNumber(closestRatio) ||
    !isNonNegative(holdS) ||
    !isFiniteNumber(endRatio) ||
    !(startRatio > closestRatio && closestRatio > 0 && endRatio > closestRatio) ||
    // Each look at most once.
    new Set(looks.map((each) => each.name)).size !== looks.length
  ) {
    return null;
  }
  return {
    world,
    devicePixelRatio,
    canvas: size,
    target,
    reliefM,
    tauPx,
    radPerPx,
    rangeM,
    cellLevel,
    approach: { ratePerS, startRatio, closestRatio, holdS, endRatio },
    looks,
    faults,
  };
}

/**
 * A capture's label: lower case letters, digits and hyphens, 1 to 48 of them (the session's
 * `capture`, the capture channel's check and the atmosphere run's record share it).
 */
export const CAPTURE_LABEL = /^[a-z0-9-]{1,48}$/u;

/**
 * Whether `value` is a label a capture may be named by (`ViewsCheckApi.capture`), and so a label an
 * atmosphere run's record may name a capture by (R08.T10.b).
 */
export function isCaptureLabel(value: unknown): value is string {
  return typeof value === "string" && CAPTURE_LABEL.test(value);
}

/** A string or `null`. */
function isTextOrNull(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

/** A finite number of zero or more, or `null`; `undefined` where it is neither. */
function nonNegativeOrNull(value: unknown): number | null | undefined {
  return value === null ? null : isNonNegative(value) ? value : undefined;
}

/** The primary's frames over an atmosphere-run capture's window (R08.T10.b). */
function atmosphereFrames(value: unknown): AtmosphereCheckFrames | null {
  if (!isRecord(value)) {
    return null;
  }
  const { fromMs, toMs, draws } = value;
  const intervalMedianMs = nonNegativeOrNull(value["intervalMedianMs"]);
  const intervalP95Ms = nonNegativeOrNull(value["intervalP95Ms"]);
  const gpuMedianMs = nonNegativeOrNull(value["gpuMedianMs"]);
  const gpuP95Ms = nonNegativeOrNull(value["gpuP95Ms"]);
  return isFiniteNumber(fromMs) &&
    isFiniteNumber(toMs) &&
    toMs >= fromMs &&
    isCount(draws) &&
    intervalMedianMs !== undefined &&
    intervalP95Ms !== undefined &&
    gpuMedianMs !== undefined &&
    gpuP95Ms !== undefined
    ? { fromMs, toMs, draws, intervalMedianMs, intervalP95Ms, gpuMedianMs, gpuP95Ms }
    : null;
}

/** One capture of the atmosphere run (R08.T10.b). */
function atmosphereCapture(value: unknown): AtmosphereCheckCapture | null {
  if (!isRecord(value)) {
    return null;
  }
  const frames = atmosphereFrames(value["frames"]);
  const statements = strings(value["statements"]);
  const faults = strings(value["faults"]);
  const rangeM = finiteOrNull(value["rangeM"]);
  const phaseDeg = finiteOrNull(value["phaseDeg"]);
  const { label, designation, camera, exposure, atMs } = value;
  if (
    !isCaptureLabel(label) ||
    !isTextOrNull(designation) ||
    !isTextOrNull(camera) ||
    !isTextOrNull(exposure) ||
    rangeM === undefined ||
    (rangeM !== null && !(rangeM > 0)) ||
    phaseDeg === undefined ||
    (phaseDeg !== null && !(phaseDeg >= 0 && phaseDeg <= 180)) ||
    !isFiniteNumber(atMs) ||
    frames === null ||
    statements === null ||
    faults === null
  ) {
    return null;
  }
  return {
    label,
    designation,
    camera,
    rangeM,
    phaseDeg,
    exposure,
    atMs,
    frames,
    statements,
    faults,
  };
}

/** `value` as the atmosphere run's record (plan R08, R08.T10.b), or `null` if it is not one. */
export function readAtmosphereCheckRecord(value: unknown): AtmosphereCheckRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const { timer, devicePixelRatio, mediaReadyMs } = value;
  const size = canvas(value["canvas"]);
  const captures = listOf(value["captures"], atmosphereCapture);
  const faults = strings(value["faults"]);
  if (
    (timer !== "full" && timer !== "quantized" && timer !== "absent") ||
    !isFiniteNumber(devicePixelRatio) ||
    !(devicePixelRatio > 0) ||
    size === null ||
    !isNonNegative(mediaReadyMs) ||
    captures === null ||
    faults === null ||
    // Each label at most once: a second would save its picture over the first's.
    new Set(captures.map((each) => each.label)).size !== captures.length
  ) {
    return null;
  }
  return { timer, devicePixelRatio, canvas: size, mediaReadyMs, captures, faults };
}
