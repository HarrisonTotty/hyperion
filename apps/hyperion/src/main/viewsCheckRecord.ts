/**
 * The check of what the several-views check's renderer sends the main process (plan R07, T20):
 * its record, which the IPC handler treats as `unknown` until it passes; and the terrain run's
 * (plan R10, R10.T16.d), the hand-over check's (R10.T10.g), the atmosphere run's (plan R08,
 * R08.T10.b) and the inspection run's frame profile (the owner's report, 2026-11-01), checked the
 * same way.
 */

import type {
  AtmosphereCheckCapture,
  AtmosphereCheckCaptureTaken,
  AtmosphereCheckFrames,
  AtmosphereCheckRecord,
  AtmosphereCheckStage,
  HandoverAngles,
  HandoverCheckRecord,
  HandoverComparisonRecord,
  HandoverExposureRecord,
  HandoverFrameRecord,
  HandoverLookRecord,
  HandoverOwnMeans,
  HandoverRegime,
  HandoverSharedMeans,
  HandoverSwitchName,
  HandoverResampleGaps,
  HandoverTexelRecord,
  InspectionAtmosphereRead,
  InspectionAtmosphereReadName,
  InspectionAttachmentLoad,
  InspectionAttachmentSlot,
  InspectionAttachmentStore,
  InspectionAttachmentView,
  InspectionGpuAttachment,
  InspectionGpuCall,
  InspectionReactCommits,
  InspectionReactComponent,
  InspectionTimingBytes,
  InspectionTimingMarchCensus,
  InspectionTimingClusters,
  InspectionTimingMemory,
  InspectionTimingPass,
  InspectionTimingRecord,
  InspectionTimingUpload,
  InspectionTimingWindow,
  InspectionTimingWindowName,
  TerrainCheckBytes,
  TerrainCheckCachePressure,
  TerrainCheckFault,
  TerrainCheckLossRecord,
  TerrainCheckMemory,
  TerrainCheckPhaseName,
  TerrainCheckPhaseRecord,
  TerrainCheckPhaseUpdate,
  TerrainCheckRecord,
  TerrainCheckSessionFault,
  TerrainCheckOpeningRecord,
  TerrainCheckSessionCompile,
  TerrainCheckSessionRecord,
  TerrainCheckSpanRecord,
  TerrainCheckTeardownRecord,
  TerrainWholeLookName,
  TerrainWholeLookRecord,
  TerrainWholeRecord,
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
import { NAMED_HANDOVER_LOOK_NAMES } from "../preload/viewsCheckLaunch";
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
  const statements = strings(value["statements"]);
  const { startMs, endMs, untimedFrames, droppedResolves, unattributedGpuMs } = value;
  if (
    name === null ||
    views === null ||
    frameIntervalsMs === null ||
    primaryIntervalsMs === null ||
    mainThreadMs === null ||
    frameGpuMs === null ||
    statements === null ||
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
    statements,
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

/**
 * `value` as a phase's draws at which the patch cache's pins passed its slots (plan R10,
 * R10.T16.i): `null` where absent, as in a record written before them, or `null`; `undefined` if
 * it is neither nor one.
 */
function cachePressureOf(value: unknown): TerrainCheckCachePressure | null | undefined {
  if (value === undefined || value === null) {
    return null;
  }
  if (!isRecord(value)) {
    return undefined;
  }
  const { draws } = value;
  const firstScriptS = finiteOrNull(value["firstScriptS"]);
  return isCount(draws) && firstScriptS !== undefined && (draws > 0 || firstScriptS === null)
    ? { draws, firstScriptS }
    : undefined;
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
  const patchesSelected = numbers(value["patchesSelected"]);
  const cachePressure = cachePressureOf(value["cachePressure"]);
  const trianglesDrawn = numbers(value["trianglesDrawn"]);
  const casterTriangles = numbers(value["casterTriangles"]);
  const bakes = value["bakes"];
  const posts = value["fieldPosts"];
  const memory = value["memory"] === null ? null : memoryOf(value["memory"]);
  const passes = value["passes"] === null ? null : listOf(value["passes"], timingPass);
  const { startMs, endMs, scriptStartS, scriptEndS, fieldAsks, blocksSent } = value;
  if (
    name === null ||
    views === null ||
    (value["passes"] !== null && passes === null) ||
    frameIntervalsMs === null ||
    mainThreadMs === null ||
    frameGpuMs === null ||
    patchesDrawn === null ||
    !patchesDrawn.every(isCount) ||
    patchesSelected === null ||
    !patchesSelected.every(isCount) ||
    cachePressure === undefined ||
    (cachePressure !== null && cachePressure.draws > patchesDrawn.length) ||
    trianglesDrawn === null ||
    !trianglesDrawn.every(isCount) ||
    casterTriangles === null ||
    !casterTriangles.every(isCount) ||
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
    patchesSelected,
    cachePressure,
    trianglesDrawn,
    casterTriangles,
    bakes: { posted: bakes["posted"], baked: bakes["baked"], refused: bakes["refused"] },
    fieldPosts: { inits: posts["inits"], chunks: posts["chunks"], releases: posts["releases"] },
    fieldAsks,
    blocksSent,
    memory,
    passes,
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

/** The whole-body run's looks' names (`TerrainWholeLookName`). */
const WHOLE_LOOK_NAMES: ReadonlyArray<TerrainWholeLookName> = ["orbit", "cube-corner", "antipode"];

/** `value` as one of the whole-body run's looks (R10.T14.c), or `null` if it is not one. */
function wholeLook(value: unknown): TerrainWholeLookRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const name = WHOLE_LOOK_NAMES.find((each) => each === value["name"]) ?? null;
  const { settled } = value;
  if (name === null || typeof settled !== "boolean") {
    return null;
  }
  if (value["kind"] === "failed") {
    const { failure } = value;
    return typeof failure === "string" ? { kind: "failed", name, settled, failure } : null;
  }
  const { discPixels, skyPixels, outsidePixels, litOutsidePixels, finite } = value;
  return value["kind"] === "read" &&
    isCount(discPixels) &&
    isCount(skyPixels) &&
    isCount(outsidePixels) &&
    isCount(litOutsidePixels) &&
    typeof finite === "boolean"
    ? {
        kind: "read",
        name,
        settled,
        discPixels,
        skyPixels,
        outsidePixels,
        litOutsidePixels,
        finite,
      }
    : null;
}

/**
 * `value` as the whole-body run's readings (R10.T14.c): absent (`undefined`) on any other run, or
 * `null` if it is there and not one.
 */
function wholeOf(value: unknown): TerrainWholeRecord | null | undefined {
  if (value === undefined) {
    return undefined;
  }
  if (!isRecord(value)) {
    return null;
  }
  const { cells, heldCells, textSamples, notSurveyedSamples, surveyEdgeSamples, cacheBytes } =
    value;
  const decodeMs = finiteOrNull(value["decodeMs"]);
  const decodeLongestFrameGapMs = finiteOrNull(value["decodeLongestFrameGapMs"]);
  const selectionBudget = value["selectionBudget"];
  const looks = listOf(value["looks"], wholeLook);
  return isCount(cells) &&
    isCount(heldCells) &&
    isCount(textSamples) &&
    isCount(notSurveyedSamples) &&
    isCount(surveyEdgeSamples) &&
    isCount(cacheBytes) &&
    decodeMs !== undefined &&
    decodeLongestFrameGapMs !== undefined &&
    (selectionBudget === null || isCount(selectionBudget)) &&
    looks !== null
    ? {
        cells,
        heldCells,
        textSamples,
        notSurveyedSamples,
        surveyEdgeSamples,
        decodeMs,
        decodeLongestFrameGapMs,
        cacheBytes,
        selectionBudget,
        looks,
      }
    : null;
}

/**
 * `value` as a terrain run's phase as it ended (R14's Mac measurements), or `null` if it is not
 * one: the phase's record, checked as the run's record checks each of its phases, and the timer's
 * kind and the page's device-pixel ratio, as the run's record checks them.
 */
export function readTerrainPhaseUpdate(value: unknown): TerrainCheckPhaseUpdate | null {
  if (!isRecord(value)) {
    return null;
  }
  const { timer, devicePixelRatio } = value;
  const ended = terrainPhase(value["phase"]);
  return (timer === "full" || timer === "quantized" || timer === "absent") &&
    isFiniteNumber(devicePixelRatio) &&
    devicePixelRatio > 0 &&
    ended !== null
    ? { timer, devicePixelRatio, phase: ended }
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
  const whole = wholeOf(value["whole"]);
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
    range === undefined ||
    whole === null
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
    ...(whole === undefined ? {} : { whole }),
  };
}

/** A finite number of zero or more. */
function isNonNegative(value: unknown): value is number {
  return isFiniteNumber(value) && value >= 0;
}

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
  const { texel, sharedPixels, meanBefore, meanAfter, muFloored } = value;
  return isCount(texel) &&
    isCount(sharedPixels) &&
    sharedPixels >= 1 &&
    isNonNegative(meanBefore) &&
    isNonNegative(meanAfter) &&
    typeof muFloored === "boolean"
    ? { texel, sharedPixels, meanBefore, meanAfter, muFloored }
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
  const resampleGaps = handoverResampleGaps(value["resampleGaps"]);
  const { framesApart, beforeWasLast, afterWasFirst } = value;
  if (
    name === null ||
    resampleGaps === null ||
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
    resampleGaps,
  };
}

/** A switch's resampling gaps (R10.T15.b.2), or `null`. */
function handoverResampleGaps(value: unknown): HandoverResampleGaps | null {
  if (!isRecord(value)) {
    return null;
  }
  const { pixels, cells } = value;
  return isCount(pixels) && isCount(cells) && cells <= pixels ? { pixels, cells } : null;
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
  const name = NAMED_HANDOVER_LOOK_NAMES.find((each) => each === value["name"]) ?? null;
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

/** A view's style, or `null`; `undefined` where it is neither. */
function styleOrNull(value: unknown): ViewsCheckStyle | null | undefined {
  return value === null ? null : (style(value) ?? undefined);
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

/**
 * What every capture of the atmosphere run records, whichever its kind (R08.T10.b), its style a
 * style or `null` (R08.T10.g), its primary's scale above 0 or `null`, and its air's setting
 * (fix-retina-frame).
 */
function atmosphereCaptureTaken(value: Rec): AtmosphereCheckCaptureTaken | null {
  const frames = atmosphereFrames(value["frames"]);
  const statements = strings(value["statements"]);
  const faults = strings(value["faults"]);
  const drawn = styleOrNull(value["style"]);
  const { label, camera, exposure, exposureSettleMs, atMs, primaryScale, airSetting } = value;
  if (
    !isCaptureLabel(label) ||
    !isTextOrNull(camera) ||
    !isTextOrNull(exposure) ||
    drawn === undefined ||
    !isNonNegative(exposureSettleMs) ||
    !isFiniteNumber(atMs) ||
    frames === null ||
    statements === null ||
    faults === null ||
    !(primaryScale === null || (isFiniteNumber(primaryScale) && primaryScale > 0)) ||
    (airSetting !== "high" && airSetting !== "low")
  ) {
    return null;
  }
  return {
    label,
    camera,
    exposure,
    style: drawn,
    exposureSettleMs,
    atMs,
    frames,
    statements,
    faults,
    primaryScale,
    airSetting,
  };
}

/** A fixture capture's own fields, which the night's capture never carries. */
const FIXTURE_CAPTURE_FIELDS = ["designation", "rangeM", "phaseDeg"] as const;

/**
 * One capture of the atmosphere run (R08.T10.b), by its `kind`: a fixture's, with its designation,
 * a range above zero and a phase from 0° to 180°, or the night's, with none of them.
 */
function atmosphereCapture(value: unknown): AtmosphereCheckCapture | null {
  if (!isRecord(value)) {
    return null;
  }
  const taken = atmosphereCaptureTaken(value);
  if (taken === null) {
    return null;
  }
  const { kind, designation, rangeM, phaseDeg } = value;
  if (kind === "night") {
    return FIXTURE_CAPTURE_FIELDS.some((field) => field in value) ? null : { kind, ...taken };
  }
  return kind === "fixture" &&
    typeof designation === "string" &&
    designation.length > 0 &&
    isFiniteNumber(rangeM) &&
    rangeM > 0 &&
    isFiniteNumber(phaseDeg) &&
    phaseDeg >= 0 &&
    phaseDeg <= 180
    ? { kind, designation, rangeM, phaseDeg, ...taken }
    : null;
}

/**
 * One stage of the atmosphere run's flight as flown (R08.T10.b): its target, its place from 1, a
 * rate step the free camera has (0 to 31), finite distances asked, flown and short of the centre
 * (below zero back, or past it), and holds planned and held and a longest step of zero or more.
 */
function atmosphereStage(value: unknown): AtmosphereCheckStage | null {
  if (!isRecord(value)) {
    return null;
  }
  const { target, stage, rateStep, askedM, holdMs, heldMs, longestStepMs, flownM, shortOfCentreM } =
    value;
  return typeof target === "string" &&
    target.length > 0 &&
    isCount(stage) &&
    stage >= 1 &&
    isCount(rateStep) &&
    rateStep <= 31 &&
    isFiniteNumber(askedM) &&
    isNonNegative(holdMs) &&
    isNonNegative(heldMs) &&
    isNonNegative(longestStepMs) &&
    isFiniteNumber(flownM) &&
    isFiniteNumber(shortOfCentreM)
    ? { target, stage, rateStep, askedM, holdMs, heldMs, longestStepMs, flownM, shortOfCentreM }
    : null;
}

/** `value` as the atmosphere run's record (plan R08, R08.T10.b), or `null` if it is not one. */
export function readAtmosphereCheckRecord(value: unknown): AtmosphereCheckRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const { timer, devicePixelRatio, mediaReadyMs } = value;
  const size = canvas(value["canvas"]);
  const captures = listOf(value["captures"], atmosphereCapture);
  const stages = listOf(value["stages"], atmosphereStage);
  const faults = strings(value["faults"]);
  if (
    (timer !== "full" && timer !== "quantized" && timer !== "absent") ||
    !isFiniteNumber(devicePixelRatio) ||
    !(devicePixelRatio > 0) ||
    size === null ||
    !isNonNegative(mediaReadyMs) ||
    captures === null ||
    stages === null ||
    faults === null ||
    // Each label at most once: a second would save its picture over the first's.
    new Set(captures.map((each) => each.label)).size !== captures.length
  ) {
    return null;
  }
  return { timer, devicePixelRatio, canvas: size, mediaReadyMs, captures, stages, faults };
}

/** The inspection run's profiled windows (the owner's report, 2026-11-01). */
const TIMING_WINDOWS: ReadonlyArray<InspectionTimingWindowName> = ["orbit", "close", "low"];

/** `value` as a profiled window's name, or `null`. */
export function readTimingWindowName(value: unknown): InspectionTimingWindowName | null {
  return TIMING_WINDOWS.find((name) => name === value) ?? null;
}

/** Times of zero or more, ms, or `null`. */
function times(value: unknown): ReadonlyArray<number> | null {
  const read = numbers(value);
  return read !== null && read.every((each) => each >= 0) ? read : null;
}

/**
 * One pass label's times over a profiled window: a label, its times, and, for a dispatched one, its
 * kernels' names and a workgroup count for each time; none of either for a render pass.
 */
function timingPass(value: unknown): InspectionTimingPass | null {
  if (!isRecord(value)) {
    return null;
  }
  const { label } = value;
  const ms = times(value["ms"]);
  const kernels = strings(value["kernels"]);
  const workgroups = numbers(value["workgroups"]);
  if (
    typeof label !== "string" ||
    label.length === 0 ||
    ms === null ||
    kernels === null ||
    workgroups === null ||
    !workgroups.every(isCount) ||
    (kernels.length === 0 ? workgroups.length !== 0 : workgroups.length !== ms.length)
  ) {
    return null;
  }
  return { label, ms, kernels, workgroups };
}

/** A count and its bytes, each a count. */
function timingBytes(value: unknown): InspectionTimingBytes | null {
  return isRecord(value) && isCount(value["count"]) && isCount(value["bytes"])
    ? { count: value["count"], bytes: value["bytes"] }
    : null;
}

/** One resource's writes: its name, a count and its bytes. */
function timingUpload(value: unknown): InspectionTimingUpload | null {
  const sum = timingBytes(value);
  const name = isRecord(value) ? value["name"] : undefined;
  return sum !== null && typeof name === "string" && name.length > 0 ? { name, ...sum } : null;
}

/** A profiled window's writes, allocations and releases, and at most ten resources' writes. */
function timingMemory(value: unknown): InspectionTimingMemory | null {
  if (!isRecord(value)) {
    return null;
  }
  const uploaded = timingBytes(value["uploaded"]);
  const created = timingBytes(value["created"]);
  const destroyed = timingBytes(value["destroyed"]);
  const uploads = listOf(value["uploads"], timingUpload);
  return uploaded === null ||
    created === null ||
    destroyed === null ||
    uploads === null ||
    uploads.length > 10
    ? null
    : { uploaded, created, destroyed, uploads };
}

/** One call's count under one pass label (plan R14): a label, a call, a count and its bytes. */
function gpuCall(value: unknown): InspectionGpuCall | null {
  if (!isRecord(value)) {
    return null;
  }
  const { label, call, count, bytes } = value;
  return typeof label === "string" &&
    label.length > 0 &&
    typeof call === "string" &&
    call.length > 0 &&
    isCount(count) &&
    isCount(bytes)
    ? { label, call, count, bytes }
    : null;
}

/** `value` as one pass's cluster counters (plan R14), or `null`. */
function clusterPass(value: unknown): InspectionTimingClusters["passes"][number] | null {
  if (!isRecord(value)) {
    return null;
  }
  const { label, clusters, triangles } = value;
  return typeof label === "string" && isCount(clusters) && isCount(triangles)
    ? { label, clusters, triangles }
    : null;
}

/**
 * A window's WebGPU calls (plan R14), each label's call once, or `null` for a window that did not
 * count them; `undefined` where the value is neither.
 */
function gpuCalls(value: unknown): ReadonlyArray<InspectionGpuCall> | null | undefined {
  if (value === null) {
    return null;
  }
  const calls = listOf(value, gpuCall);
  return calls !== null &&
    new Set(calls.map((each) => `${each.label}\u0000${each.call}`)).size === calls.length
    ? calls
    : undefined;
}

/** The load operations an attachment's record may name. */
const ATTACHMENT_LOADS: ReadonlyArray<InspectionAttachmentLoad> = ["load", "clear", "read-only"];

/** The store operations an attachment's record may name. */
const ATTACHMENT_STORES: ReadonlyArray<InspectionAttachmentStore> = [
  "store",
  "discard",
  "read-only",
  "resolve",
];

/** Whether `value` is a count or `null`. */
function isCountOrNull(value: unknown): value is number | null {
  return value === null || isCount(value);
}

/** `value` as an attachment's slot (`colour 0`, `resolve 0`, `depth`, `stencil`), or `null`. */
function attachmentSlot(value: unknown): InspectionAttachmentSlot | null {
  if (value === "depth" || value === "stencil") {
    return value;
  }
  if (typeof value !== "string" || !/^(?:colour|resolve) \d+$/u.test(value)) {
    return null;
  }
  // A colour or resolve target's slot, its number checked just now: its template's own form.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return value as InspectionAttachmentSlot;
}

/**
 * `value` as an attachment's view (a format, a size of a pixel or more, a sample or more), `null`
 * for none seen made, or `undefined` where it is neither.
 */
function attachmentView(value: unknown): InspectionAttachmentView | null | undefined {
  if (value === null) {
    return null;
  }
  if (!isRecord(value)) {
    return undefined;
  }
  const { format, widthPx, heightPx, sampleCount } = value;
  return typeof format === "string" &&
    format.length > 0 &&
    isCount(widthPx) &&
    widthPx > 0 &&
    isCount(heightPx) &&
    heightPx > 0 &&
    isCount(sampleCount) &&
    sampleCount > 0
    ? { format, widthPx, heightPx, sampleCount }
    : undefined;
}

/**
 * One kind of a pass label's attachments (plan R14, R14.T2.e): a label and an attachment slot, its
 * view or `null`, its operations as named or `null`, a pass or more, and its bytes.
 */
function gpuAttachment(value: unknown): InspectionGpuAttachment | null {
  if (!isRecord(value)) {
    return null;
  }
  const { label, passes, bytesLoaded, bytesStored } = value;
  const attachment = attachmentSlot(value["attachment"]);
  const seen = attachmentView(value["view"]);
  const loadOp = ATTACHMENT_LOADS.find((each) => each === value["loadOp"]) ?? null;
  const storeOp = ATTACHMENT_STORES.find((each) => each === value["storeOp"]) ?? null;
  return typeof label === "string" &&
    label.length > 0 &&
    attachment !== null &&
    seen !== undefined &&
    (loadOp !== null || value["loadOp"] === null) &&
    (storeOp !== null || value["storeOp"] === null) &&
    isCount(passes) &&
    passes > 0 &&
    isCountOrNull(bytesLoaded) &&
    isCountOrNull(bytesStored)
    ? {
        label,
        attachment,
        view: seen,
        loadOp,
        storeOp,
        passes,
        bytesLoaded,
        bytesStored,
      }
    : null;
}

/**
 * A window's render passes' attachments (plan R14, R14.T2.e), or `null` for a window that did not
 * count them; `undefined` where the value is neither.
 */
function gpuAttachments(value: unknown): ReadonlyArray<InspectionGpuAttachment> | null | undefined {
  if (value === null) {
    return null;
  }
  return listOf(value, gpuAttachment) ?? undefined;
}

/** The reads an atmosphere's record may name. */
const ATMOSPHERE_READS: ReadonlyArray<InspectionAtmosphereReadName> = [
  "thin",
  "thick",
  "deck",
  "beneath a deck",
];

/** One body's atmosphere read over a window: a body from 1, a read named, its frames and kernels. */
function atmosphereRead(value: unknown): InspectionAtmosphereRead | null {
  if (!isRecord(value)) {
    return null;
  }
  const { air, thermal, frames } = value;
  const read = ATMOSPHERE_READS.find((each) => each === value["read"]);
  const kernels = strings(value["kernels"]);
  return isCount(air) &&
    air > 0 &&
    read !== undefined &&
    typeof thermal === "boolean" &&
    isCount(frames) &&
    kernels !== null
    ? { air, read, thermal, frames, kernels }
    : null;
}

/** One prop's changed renders (lane perf-render-thread-2): a name and a count above 0. */
function changedProp(value: unknown): InspectionReactComponent["propsChanged"][number] | null {
  if (!isRecord(value)) {
    return null;
  }
  const { prop, renders } = value;
  return typeof prop === "string" && isCount(renders) && renders > 0 ? { prop, renders } : null;
}

/**
 * One component's renders (lane perf-render-thread-2): its name, and no more mounts, renders with
 * nothing changed, or renders with its state or a prop changed than its renders after a mount.
 */
function reactComponent(value: unknown): InspectionReactComponent | null {
  if (!isRecord(value)) {
    return null;
  }
  const { name, renders, mounts, unchanged, stateChanged } = value;
  const propsChanged = listOf(value["propsChanged"], changedProp);
  if (
    typeof name !== "string" ||
    !isCount(renders) ||
    !isCount(mounts) ||
    !isCount(unchanged) ||
    !isCount(stateChanged) ||
    propsChanged === null ||
    mounts > renders ||
    unchanged + stateChanged > renders - mounts ||
    propsChanged.some((each) => each.renders > renders - mounts)
  ) {
    return null;
  }
  return { name, renders, mounts, unchanged, stateChanged, propsChanged };
}

/**
 * A window's React commits (lane perf-render-thread-2), or `null` for a window React did not
 * report to; `undefined` where the value is neither, or lists a component twice or more renders
 * than its total.
 */
function reactCommits(value: unknown): InspectionReactCommits | null | undefined {
  if (value === null) {
    return null;
  }
  if (!isRecord(value)) {
    return undefined;
  }
  const { commits, renders } = value;
  const components = listOf(value["components"], reactComponent);
  return !isCount(commits) ||
    !isCount(renders) ||
    components === null ||
    new Set(components.map((each) => each.name)).size !== components.length ||
    components.reduce((sum, each) => sum + each.renders, 0) > renders
    ? undefined
    : { commits, renders, components };
}

/**
 * A window's march census (plan R08, R08.T8.d), or `undefined` where `value` is not one: its
 * kernel's name, the texels by node rays 0 to 10, and its counts, whole numbers, no more limb tiles
 * than tiles and no fewer lanes' rays than rays.
 */
function timingCensus(value: unknown): InspectionTimingMarchCensus | null | undefined {
  if (value === null) {
    return null;
  }
  if (!isRecord(value)) {
    return undefined;
  }
  const { kernel, wholeMeans, centresInAir, tiles, limbTiles, rays, laneRays } = value;
  const texelsByNodes = numbers(value["texelsByNodes"]);
  return typeof kernel !== "string" ||
    kernel.length === 0 ||
    texelsByNodes === null ||
    texelsByNodes.length !== 11 ||
    !texelsByNodes.every(isCount) ||
    !isCount(wholeMeans) ||
    !isCount(centresInAir) ||
    !isCount(tiles) ||
    !isCount(limbTiles) ||
    limbTiles > tiles ||
    !isCount(rays) ||
    !isCount(laneRays) ||
    laneRays < rays
    ? undefined
    : { kernel, texelsByNodes, wholeMeans, centresInAir, tiles, limbTiles, rays, laneRays };
}

/**
 * `value` as a window's cluster counters (plan R14): `null` where it has none; `undefined` where it
 * is neither, each pass's label once.
 */
function timingClusters(value: unknown): InspectionTimingClusters | null | undefined {
  if (value === null) {
    return null;
  }
  const passes = isRecord(value) ? listOf(value["passes"], clusterPass) : null;
  return passes === null || new Set(passes.map((pass) => pass.label)).size !== passes.length
    ? undefined
    : { passes };
}

/**
 * One profiled window (the owner's report): its name, an end not before its start, and its times,
 * each pass's as many as the frames timed in full, each label once, its WebGPU calls and
 * attachments or `null`, its atmosphere reads, its React commits or `null`, and its cluster
 * counters or `null`.
 */
function timingWindow(value: unknown): InspectionTimingWindow | null {
  if (!isRecord(value)) {
    return null;
  }
  const name = readTimingWindowName(value["name"]);
  const size = canvas(value["canvas"]);
  const scales = numbers(value["scales"]);
  const intervalsMs = times(value["intervalsMs"]);
  const mainThreadMs = times(value["mainThreadMs"]);
  const submitMs = times(value["submitMs"]);
  const gpuMs = times(value["gpuMs"]);
  const passes = listOf(value["passes"], timingPass);
  const statements = strings(value["statements"]);
  const memory = timingMemory(value["memory"]);
  const calls = gpuCalls(value["gpuCalls"]);
  const attachments = gpuAttachments(value["gpuAttachments"]);
  const atmosphereReads = listOf(value["atmosphereReads"], atmosphereRead);
  const react = reactCommits(value["react"]);
  const marchCensus = timingCensus(value["marchCensus"]);
  const clusters = timingClusters(value["clusters"]);
  const { startMs, endMs, untimedFrames, position, marchSamples } = value;
  if (
    name === null ||
    !isFiniteNumber(startMs) ||
    !isFiniteNumber(endMs) ||
    endMs < startMs ||
    size === null ||
    scales === null ||
    intervalsMs === null ||
    mainThreadMs === null ||
    submitMs === null ||
    gpuMs === null ||
    passes === null ||
    passes.some((pass) => pass.ms.length !== gpuMs.length) ||
    new Set(passes.map((pass) => pass.label)).size !== passes.length ||
    !isCount(untimedFrames) ||
    memory === null ||
    calls === undefined ||
    attachments === undefined ||
    atmosphereReads === null ||
    react === undefined ||
    clusters === undefined ||
    !isCount(marchSamples) ||
    marchSamples === 0 ||
    !isTextOrNull(position) ||
    statements === null ||
    marchCensus === undefined
  ) {
    return null;
  }
  return {
    name,
    startMs,
    endMs,
    canvas: size,
    scales,
    intervalsMs,
    mainThreadMs,
    submitMs,
    gpuMs,
    passes,
    untimedFrames,
    memory,
    gpuCalls: calls,
    gpuAttachments: attachments,
    atmosphereReads,
    react,
    clusters,
    marchSamples,
    position,
    statements,
    marchCensus,
  };
}

/**
 * `value` as the inspection run's frame profile's record (the owner's report, 2026-11-01), or
 * `null` if it is not one: each window at most once.
 */
export function readInspectionTimingRecord(value: unknown): InspectionTimingRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const { timer, devicePixelRatio } = value;
  const windows = listOf(value["windows"], timingWindow);
  if (
    (timer !== "full" && timer !== "quantized" && timer !== "absent") ||
    !isFiniteNumber(devicePixelRatio) ||
    !(devicePixelRatio > 0) ||
    windows === null ||
    new Set(windows.map((each) => each.name)).size !== windows.length
  ) {
    return null;
  }
  return { timer, devicePixelRatio, windows };
}
