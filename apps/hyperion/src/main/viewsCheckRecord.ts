/**
 * The check of what the several-views check's renderer sends the main process (plan R07, T20):
 * its record, which the IPC handler treats as `unknown` until it passes.
 */

import type {
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
