/**
 * The check of what the several-views check's renderer sends the main process (plan R07, T20):
 * its record, which the IPC handler treats as `unknown` until it passes; and the terrain run's
 * (plan R10, R10.T16.d), checked the same way.
 */

import type {
  TerrainCheckBytes,
  TerrainCheckFault,
  TerrainCheckLossRecord,
  TerrainCheckMemory,
  TerrainCheckPhaseName,
  TerrainCheckPhaseRecord,
  TerrainCheckRecord,
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
    handedOverDraws,
    offSurveyDraws,
    terrainDraws,
    terrainGaps,
    terrainGapSpansS,
    firstHandoverMs,
    firstHandoverScriptS,
    firstTerrainMs,
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

/** `value` as the terrain run's record (R10.T16.d), or `null` if it is not one. */
export function readTerrainCheckRecord(value: unknown): TerrainCheckRecord | null {
  if (!isRecord(value)) {
    return null;
  }
  const { timer, devicePixelRatio } = value;
  const phases = listOf(value["phases"], terrainPhase);
  const faults = listOf(value["faults"], fault);
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
    restoresAtMs,
    loss: lost,
    teardown: torn,
    handover: range,
  };
}
