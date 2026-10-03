/**
 * Checks of what the descent spike's renderer sends the main process (plan R05, T13.c): its report
 * (T14.a's `DescentSpikeReport`) and its capture (T15.a), which every IPC handler treats as
 * `unknown` until they pass.
 */

import type {
  DescentSpikeReport,
  SpikeLatePipeline,
  SpikePassSeries,
  SpikeSegmentSpan,
  SpikeStreamingSegment,
} from "../preload/api";

type Rec = Readonly<Record<string, unknown>>;

function isRecord(value: unknown): value is Rec {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFinite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isCount(value: unknown): value is number {
  return isFinite(value) && Number.isInteger(value) && value >= 0;
}

function finiteList(value: unknown): value is ReadonlyArray<number> {
  return Array.isArray(value) && value.every(isFinite);
}

function listOf<T>(value: unknown, read: (item: unknown) => T | null): T[] | null {
  if (!Array.isArray(value)) {
    return null;
  }
  const out: T[] = [];
  for (const item of value) {
    const read1 = read(item);
    if (read1 === null) {
      return null;
    }
    out.push(read1);
  }
  return out;
}

function segment(value: unknown): SpikeSegmentSpan | null {
  if (!isRecord(value)) {
    return null;
  }
  const { name, startS, endS } = value;
  return typeof name === "string" && isFinite(startS) && isFinite(endS) && endS >= startS
    ? { name, startS, endS }
    : null;
}

function level(value: unknown): DescentSpikeReport["levels"][number] | null {
  if (!isRecord(value)) {
    return null;
  }
  const { level: n, epsilonM, k } = value;
  return isCount(n) && isFinite(epsilonM) && isFinite(k) ? { level: n, epsilonM, k } : null;
}

function pass(value: unknown, frames: number): SpikePassSeries | null {
  if (!isRecord(value)) {
    return null;
  }
  const { label, row, gpuMs } = value;
  if (
    typeof label !== "string" ||
    (row !== "terrain" && row !== "atmosphere" && row !== "other") ||
    !Array.isArray(gpuMs) ||
    gpuMs.length !== frames ||
    !gpuMs.every((ms) => ms === null || isFinite(ms))
  ) {
    return null;
  }
  return { label, row, gpuMs: gpuMs.map((ms: unknown) => (isFinite(ms) ? ms : null)) };
}

const STREAMING_FIGURES = [
  "requestedPerS",
  "bakedPerS",
  "residentPerS",
  "predictedHardPerS",
  "predictedCalibratedPerS",
  "patchesHard",
  "patchesCalibrated",
  "streamingS",
] as const;

function streaming(value: unknown): SpikeStreamingSegment | null {
  if (!isRecord(value) || typeof value["segment"] !== "string") {
    return null;
  }
  const figures: Partial<Record<(typeof STREAMING_FIGURES)[number], number>> = {};
  for (const name of STREAMING_FIGURES) {
    const figure = value[name];
    if (!isFinite(figure)) {
      return null;
    }
    figures[name] = figure;
  }
  return {
    segment: value["segment"],
    requestedPerS: figures.requestedPerS ?? 0,
    bakedPerS: figures.bakedPerS ?? 0,
    residentPerS: figures.residentPerS ?? 0,
    predictedHardPerS: figures.predictedHardPerS ?? 0,
    predictedCalibratedPerS: figures.predictedCalibratedPerS ?? 0,
    patchesHard: figures.patchesHard ?? 0,
    patchesCalibrated: figures.patchesCalibrated ?? 0,
    streamingS: figures.streamingS ?? 0,
  };
}

function latePipeline(value: unknown): SpikeLatePipeline | null {
  if (!isRecord(value)) {
    return null;
  }
  const { label, kind, async: isAsync, scriptTimeS } = value;
  return typeof label === "string" &&
    (kind === "render" || kind === "compute") &&
    typeof isAsync === "boolean" &&
    isFinite(scriptTimeS)
    ? { label, kind, async: isAsync, scriptTimeS }
    : null;
}

/**
 * `value` as the renderer's report, or `null` if any part of it is missing or of the wrong type:
 * every series as long as the frames', every figure finite.
 */
export function readDescentSpikeReport(value: unknown): DescentSpikeReport | null {
  if (!isRecord(value)) {
    return null;
  }
  const frames = value["frames"];
  if (!isRecord(frames)) {
    return null;
  }
  const { scriptTimesS, rafIntervalsMs, ourCodeMs } = frames;
  if (!finiteList(scriptTimesS) || !finiteList(rafIntervalsMs) || !finiteList(ourCodeMs)) {
    return null;
  }
  const count = scriptTimesS.length;
  if (rafIntervalsMs.length !== count || ourCodeMs.length !== count) {
    return null;
  }
  const passes = listOf(frames["passes"], (item) => pass(item, count));
  const segments = listOf(value["segments"], segment);
  const levels = listOf(value["levels"], level);
  const streamingList = listOf(value["streaming"], streaming);
  const late = listOf(value["latePipelines"], latePipeline);
  const canvas = value["canvas"];
  const { warmupS, timer, untimedPasses, uploadBytes, adapterPeakBytes } = value;
  if (
    passes === null ||
    segments === null ||
    levels === null ||
    streamingList === null ||
    late === null ||
    !isFinite(warmupS) ||
    (timer !== "full" && timer !== "quantized" && timer !== "absent") ||
    !isCount(untimedPasses) ||
    !isCount(uploadBytes) ||
    !isCount(adapterPeakBytes) ||
    !isRecord(canvas) ||
    !isCount(canvas["widthPx"]) ||
    !isCount(canvas["heightPx"])
  ) {
    return null;
  }
  return {
    warmupS,
    segments,
    levels,
    timer,
    untimedPasses,
    frames: { scriptTimesS, rafIntervalsMs, ourCodeMs, passes },
    streaming: streamingList,
    uploadBytes,
    latePipelines: late,
    adapterPeakBytes,
    canvas: { widthPx: canvas["widthPx"], heightPx: canvas["heightPx"] },
  };
}

/** A capture as the renderer sends it: `capture.json`'s text and `capture.bin`'s bytes. */
export interface SpikeCaptureFiles {
  readonly json: string;
  readonly bin: Uint8Array;
}

/** The largest capture accepted, bytes: a span of the descent is far smaller. */
export const MAX_CAPTURE_BYTES = 4 * 1024 ** 3;

/** `value` as a capture's two files, or `null`: JSON text that parses, and a byte array. */
export function readSpikeCapture(value: unknown): SpikeCaptureFiles | null {
  if (!isRecord(value)) {
    return null;
  }
  const { json, bin } = value;
  if (typeof json !== "string" || !(bin instanceof Uint8Array)) {
    return null;
  }
  if (json.length + bin.byteLength > MAX_CAPTURE_BYTES) {
    return null;
  }
  try {
    const parsed: unknown = JSON.parse(json);
    return isRecord(parsed) ? { json, bin } : null;
  } catch {
    return null;
  }
}
