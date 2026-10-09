/**
 * The several-views check's results file and its summary (plan R07, T20): what the renderer
 * measured of each phase and what the main process read of its trace, judged against T20's
 * acceptance.
 *
 * @remarks
 * T20 asks, of a full-window photorealistic primary with two wireframe instruments: each view the
 * right way up (the person at a shown run answers; captures are kept to look at again), no GPU
 * time in copies, a resize of one leaving the others' attachments alone, the frame time with the
 * instruments open (against the low setting's 33 ms on the UHD 620), the per-canvas overhead that
 * replaces `PER_CANVAS_OVERHEAD_MS`'s provisional 0.3 ms, and the pass timer's drop warning never
 * appearing; and, on the UHD 620's low setting, a wireframe primary with two wireframe
 * instruments and one with a photorealistic and a wireframe instrument against R05 Design note
 * 21's criteria at 60 Hz. Each is a finding here with its verdict.
 *
 * The frame rows are Design note 21's, read from presentation times (the trace's), against T the
 * display's vsync period, or twice it for the low setting's 30 Hz photorealistic primary. A hidden
 * run has no window, so T and the presentation figures are missing with "no window shown"; such a
 * run proves the harness and the file, not the criterion, as the descent spike's hidden runs do.
 *
 * The per-canvas overhead is what the GPU process's main thread spends per primary draw on each
 * canvas opened beside the primary: the difference between a phase with the two instruments and
 * the same primary alone, halved. It is an upper bound, not the presenting cost alone: every GPU
 * command of a canvas passes through that thread, its own passes' decoding as well as its
 * presentation, while R01's pass timer sees only the views' passes; and it is per primary draw,
 * where on the high setting the 30 Hz instruments draw in every second one. The instruments' own
 * pass times are reported beside it. Which figure replaces `PER_CANVAS_OVERHEAD_MS` is the
 * owner's ruling (R07's Risks).
 *
 * Every trace figure is clipped to the phase's measured window, the span the page marks with
 * `performance.measure`, a second after the trace's start (R05's boundary guard) to its stop.
 */

import type {
  SpikePassTimer,
  ViewsCheckPhaseName,
  ViewsCheckPhaseRecord,
  ViewsCheckRecord,
  ViewsCheckStyle,
  ViewsCheckViewName,
} from "../preload/api";
import type { GraphicsLaunchMode } from "../preload/api";
import {
  ADDED_FILE_LIMIT_BYTES,
  type Criterion,
  type FrameStats,
  formatAsPrettier,
  frameRows,
  frameStats,
  type MachineDescription,
  type Measured,
  measured,
  missing,
  nearestRank,
  ofPeriod,
  overallOf,
  type ResultsFiles,
  row,
  type SpikeSetting,
  type Verdict,
} from "./results";
import { keepsLoadAverage, QUIET_RULE_UNCHECKED, quietOf } from "./machineLoad";
import { access, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

/** The file's schema name. */
export const VIEWS_CHECK_SCHEMA = "hyperion.views-check.results";

/** The schema's version; bumped with any change to the file's shape. */
export const VIEWS_CHECK_VERSION = 1;

/** The reason every figure read against T or from presentation is missing in a hidden run. */
export const NO_WINDOW_REASON = "no window shown";

/** Durations summarised by nearest rank. */
export interface DurationStats {
  readonly count: number;
  readonly p50Ms: number;
  readonly p95Ms: number;
  readonly p99Ms: number;
  readonly maxMs: number;
  readonly meanMs: number;
}

/** `values` summarised, or `null` for none. */
export function durationStats(values: ReadonlyArray<number>): DurationStats | null {
  if (values.length === 0) {
    return null;
  }
  const sorted = values.toSorted((a, b) => a - b);
  return {
    count: sorted.length,
    p50Ms: nearestRank(sorted, 0.5),
    p95Ms: nearestRank(sorted, 0.95),
    p99Ms: nearestRank(sorted, 0.99),
    maxMs: sorted.at(-1) ?? 0,
    meanMs: sorted.reduce((sum, value) => sum + value, 0) / sorted.length,
  };
}

/** A GPU-process slice by name, counted and summed over a window. */
export interface NamedSlice {
  readonly name: string;
  readonly count: number;
  readonly totalMs: number;
}

/** What the main process read of one phase's trace window. */
export interface PhaseTraceFigures {
  /** The compositor's presentations: their intervals, ms, and the frames it dropped. */
  readonly presentation: Measured<{
    readonly intervalsMs: ReadonlyArray<number>;
    readonly dropped: number;
  }>;
  /**
   * The GPU process's main thread: its busy time over the window, ms, R05's slices of it
   * (`WebGPU`, `GPUTask`, `VulkanQueueSubmitHook`) and its copy slices.
   */
  readonly gpuProcess: Measured<{
    readonly busyMs: number;
    readonly slices: ReadonlyArray<NamedSlice>;
    readonly copySlices: ReadonlyArray<NamedSlice>;
  }>;
}

/** The run, as the main process describes it at its start. */
export interface ViewsCheckRunInput {
  readonly startedAt: Date;
  readonly machine: MachineDescription;
  readonly versions: {
    readonly app: string;
    readonly electron: string;
    readonly chromium: string;
    readonly node: string;
    readonly v8: string;
  };
  readonly platform: NodeJS.Platform;
  readonly launchMode: GraphicsLaunchMode;
  readonly setting: SpikeSetting;
  readonly smoke: boolean;
  /** The Chromium switches the launch applied, as `--name=value`. */
  readonly switches: ReadonlyArray<string>;
  readonly shown: boolean;
  /** The display's refresh rate (`Display.displayFrequency`), Hz; `null` hidden. */
  readonly displayHz: number | null;
  /**
   * The window's content size when the run ended, DIP; times the record's device-pixel ratio, in
   * pixels. A tiling window manager sizes it to its tile.
   */
  readonly window: { readonly widthDip: number; readonly heightDip: number };
  /** Where the page's captures were saved, or why none were. */
  readonly captures: Measured<string>;
}

/** Everything the results are built from. */
export interface ViewsCheckInput {
  readonly run: ViewsCheckRunInput;
  readonly record: ViewsCheckRecord;
  readonly traces: ReadonlyMap<ViewsCheckPhaseName, PhaseTraceFigures>;
  /** The page's console warnings from R01's pass timer over the run. */
  readonly warnings: {
    /** "… resolves' pass times are still being read; dropping some". */
    readonly passTimerDrops: number;
    /** "more than … passes in a frame; the rest are not timed". */
    readonly untimedPasses: number;
  };
  /** The GPU process's exits over the run. */
  readonly gpuProcessExits: number;
  /** The person's answer at a shown run, or why there is none. */
  readonly rightWayUp: Measured<"yes" | "no">;
}

/** What each phase should show: the primary's style, then each open instrument's in slot order. */
export const PHASE_VIEWS: Readonly<Record<ViewsCheckPhaseName, ReadonlyArray<ViewsCheckStyle>>> = {
  "photoreal-alone": ["photorealistic"],
  "photoreal-two-wireframe": ["photorealistic", "wireframe", "wireframe"],
  "wireframe-two-wireframe": ["wireframe", "wireframe", "wireframe"],
  "wireframe-photoreal-wireframe": ["wireframe", "photorealistic", "wireframe"],
  "wireframe-alone": ["wireframe"],
};

/** What each phase is for, in the summary's words. */
const PHASE_PURPOSE: Readonly<Record<ViewsCheckPhaseName, string>> = {
  "photoreal-alone": "the photorealistic primary alone: the overhead's baseline",
  "photoreal-two-wireframe":
    "T20's cockpit: the photorealistic primary and two wireframe instruments",
  "wireframe-two-wireframe": "a wireframe primary and two wireframe instruments (low: at 60 Hz)",
  "wireframe-photoreal-wireframe":
    "a wireframe primary, a photorealistic and a wireframe instrument (low: at 60 Hz)",
  "wireframe-alone": "the wireframe primary alone: the overhead's baseline",
};

/** The overhead's pairs: the primary alone, then with the two instruments. */
const OVERHEAD_PAIRS: ReadonlyArray<readonly [ViewsCheckPhaseName, ViewsCheckPhaseName]> = [
  ["photoreal-alone", "photoreal-two-wireframe"],
  ["wireframe-alone", "wireframe-two-wireframe"],
];

/** What T20 does on a miss of each low-setting case (decision-r07-t18, items 4 and 5). */
const LOW_CASE_CONSEQUENCE: Readonly<Partial<Record<ViewsCheckPhaseName, string>>> = {
  "wireframe-two-wireframe":
    "on its miss, the low setting's wireframe primary moves to 30 Hz, as a `budget` field",
  "wireframe-photoreal-wireframe":
    "on its miss alone, the photorealistic instrument's scale goes under the controller, against the primary's period, rather than the primary's rate dropping",
};

/** A pass's label that says it copies. */
const COPY_LABEL = /copy|blit/iu;

/** One view's figures over a phase. */
export interface ViewFigures {
  readonly name: ViewsCheckViewName;
  readonly slot: ViewSlotName;
  readonly style: ViewsCheckStyle;
  readonly canvas: { readonly widthPx: number; readonly heightPx: number };
  readonly draws: number;
  readonly gpuMs: DurationStats | null;
  readonly submitMs: DurationStats | null;
  readonly passLabels: ReadonlyArray<string>;
  /** Its scene target's scale over the phase, or `null` for a view without one. */
  readonly scale: {
    readonly first: number;
    readonly min: number;
    readonly max: number;
    readonly last: number;
  } | null;
}

/** One phase's figures and verdicts. */
export interface PhaseFigures {
  readonly name: ViewsCheckPhaseName;
  readonly purpose: string;
  /** Whether every view drew the style the phase asks for when it ended. */
  readonly held: boolean;
  /** The primary's rate: 30 Hz for a photorealistic primary on the low setting, else 60 Hz. */
  readonly rateHz: 60 | 30;
  /** T for this phase: the vsync period, or twice it at 30 Hz. */
  readonly periodMs: Measured<number>;
  readonly windowS: number;
  readonly views: ReadonlyArray<ViewFigures>;
  /** Intervals between animation frames and between the primary's draws, from their timestamps. */
  readonly animationFrames: FrameStats | null;
  readonly primaryDraws: FrameStats | null;
  /** Intervals between presentations, and the frames dropped. */
  readonly presentation: Measured<FrameStats & { readonly dropped: number }>;
  /** Each frame's time in the page's animation-frame callbacks: our code's main-thread time. */
  readonly mainThreadMs: DurationStats | null;
  /** Each frame's timed GPU time, every view's passes. */
  readonly frameGpuMs: DurationStats | null;
  readonly untimedFrames: number;
  readonly droppedResolves: number;
  readonly unattributedGpuMs: number;
  /**
   * The GPU process's main thread per primary draw, R05's slices of it per primary draw, and its
   * copy slices per primary draw.
   */
  readonly gpuProcess: Measured<{
    readonly busyMsPerFrame: number;
    readonly slicesMsPerFrame: ReadonlyArray<{ readonly name: string; readonly ms: number }>;
    readonly copySlicesPerFrame: number;
    readonly copySlices: ReadonlyArray<NamedSlice>;
  }>;
  /** Design note 21's frame and headroom rows, from presentation times. */
  readonly criteria: { readonly rows: ReadonlyArray<Criterion>; readonly overall: Verdict };
}

/** The per-canvas overhead read from one pair of phases. */
export interface OverheadPair {
  readonly without: ViewsCheckPhaseName;
  readonly with: ViewsCheckPhaseName;
  readonly extraCanvases: number;
  /** The GPU process's main thread per primary draw, without and with the instruments, ms. */
  readonly gpuProcessMsPerFrame: {
    readonly without: Measured<number>;
    readonly with: Measured<number>;
  };
  /** Their difference over the extra canvases, ms. */
  readonly perCanvasMs: Measured<number>;
  /** Each instrument's own timed GPU time a draw, and its submission's CPU time, means, ms. */
  readonly instruments: ReadonlyArray<{
    readonly name: string;
    readonly gpuMsPerDraw: Measured<number>;
    readonly submitMsPerDraw: Measured<number>;
  }>;
}

/** A results file of the several-views check. */
export interface ViewsCheckResults {
  readonly schema: typeof VIEWS_CHECK_SCHEMA;
  readonly version: typeof VIEWS_CHECK_VERSION;
  readonly run: {
    readonly startedAt: string;
    readonly machine: MachineDescription;
    readonly versions: ViewsCheckRunInput["versions"];
    readonly platform: NodeJS.Platform;
    readonly launchMode: GraphicsLaunchMode;
    readonly setting: SpikeSetting;
    readonly smoke: boolean;
    readonly switches: ReadonlyArray<string>;
    readonly shown: boolean;
    readonly window: ViewsCheckRunInput["window"];
    readonly devicePixelRatio: number;
    readonly timer: SpikePassTimer;
    readonly displayHz: Measured<number>;
    /** The display's vsync period, ms. */
    readonly vsyncMs: Measured<number>;
    readonly captures: Measured<string>;
    readonly quiet: { readonly provisional: boolean; readonly note: string | null };
  };
  readonly phases: ReadonlyArray<PhaseFigures>;
  readonly findings: {
    readonly rightWayUp: { readonly verdict: Verdict; readonly answer: Measured<"yes" | "no"> };
    readonly copies: {
      readonly verdict: Verdict;
      /** Labels of the views' own passes that say they copy; none is the pass. */
      readonly copyPasses: ReadonlyArray<string>;
      /** The GPU process's copy slices a primary draw, by phase: a count that grows with the
       * instruments would be a copy a canvas, outside the views' passes. */
      readonly gpuProcessCopiesPerFrame: ReadonlyArray<{
        readonly phase: ViewsCheckPhaseName;
        readonly perFrame: Measured<number>;
      }>;
    };
    readonly resize: {
      readonly verdict: Verdict;
      readonly primaryResized: boolean;
      readonly othersUnchanged: boolean;
      /**
       * The allocations made and destroyed during the resize, by the view they belong to, or
       * {@link ENGINE_OWNER}'s.
       */
      readonly byView: ReadonlyArray<{
        readonly view: string;
        readonly created: number;
        readonly destroyed: number;
        readonly names: ReadonlyArray<string>;
      }>;
      readonly record: ViewsCheckRecord["resize"];
    };
    /** T20's cockpit against Design note 21 (the low setting's 33 ms on the UHD 620). */
    readonly frameTime: {
      readonly phase: ViewsCheckPhaseName;
      readonly verdict: Verdict;
      readonly rows: ReadonlyArray<Criterion>;
    };
    /** T20's two low-setting cases against Design note 21 at 60 Hz, and what a miss does. */
    readonly lowCases: ReadonlyArray<{
      readonly phase: ViewsCheckPhaseName;
      readonly verdict: Verdict;
      readonly consequence: string;
      /** Whether T20 asks this case of the run: on the low setting. */
      readonly applies: boolean;
      /**
       * Whether this case's consequence is the one that follows: the first case's on its miss,
       * the second's on its miss alone.
       */
      readonly triggered: boolean;
    }>;
    readonly perCanvasOverhead: {
      readonly provisionalMs: number;
      readonly pairs: ReadonlyArray<OverheadPair>;
      /** The larger pair's figure, an upper bound, for the owner's ruling on the constant. */
      readonly measuredMs: Measured<number>;
    };
    readonly passTimer: {
      readonly verdict: Verdict;
      readonly consoleDrops: number;
      readonly untimedPassWarnings: number;
      readonly droppedResolves: number;
      readonly untimedFrames: number;
    };
    readonly faults: {
      readonly verdict: Verdict;
      readonly faults: ReadonlyArray<string>;
      readonly gpuProcessExits: number;
    };
  };
}

/** The slot a view stands in, as the operator names it. */
export type ViewSlotName = "PRIMARY" | "INSTRUMENT 1" | "INSTRUMENT 2";

/** The slot a view's engine name stands for. */
function slotOf(name: ViewsCheckViewName): ViewSlotName {
  let slot: ViewSlotName;
  switch (name) {
    case "view":
      slot = "PRIMARY";
      break;
    case "instrument-1":
      slot = "INSTRUMENT 1";
      break;
    case "instrument-2":
      slot = "INSTRUMENT 2";
      break;
  }
  return slot;
}

function viewFigures(record: ViewsCheckPhaseRecord["views"][number]): ViewFigures {
  const scales = record.scales;
  const first = scales[0];
  const last = scales.at(-1);
  return {
    name: record.name,
    slot: slotOf(record.name),
    style: record.style,
    canvas: record.canvas,
    draws: record.draws,
    gpuMs: durationStats(record.gpuMs),
    submitMs: durationStats(record.submitMs),
    passLabels: record.passLabels,
    scale:
      first === undefined || last === undefined
        ? null
        : { first, min: Math.min(...scales), max: Math.max(...scales), last },
  };
}

function phaseFigures(
  record: ViewsCheckPhaseRecord,
  trace: PhaseTraceFigures | undefined,
  vsyncMs: Measured<number>,
  setting: SpikeSetting,
): PhaseFigures {
  const expected = PHASE_VIEWS[record.name];
  const held =
    record.views.length === expected.length &&
    record.views.every((view, i) => view.style === expected[i]);
  const rateHz = setting === "low" && expected[0] === "photorealistic" ? 30 : 60;
  const periodMs = ofPeriod(vsyncMs, (t) => (rateHz === 30 ? 2 * t : t));
  const period = periodMs.value;
  const primaryDrawCount = record.views[0]?.draws ?? 0;
  const presentation: PhaseFigures["presentation"] =
    trace === undefined
      ? missing("no trace of this phase")
      : trace.presentation.value === null
        ? missing(trace.presentation.reason)
        : (() => {
            const stats = frameStats(trace.presentation.value.intervalsMs, period);
            return stats === null
              ? missing("no presentation in the window")
              : measured({ ...stats, dropped: trace.presentation.value.dropped });
          })();
  const gpuProcess: PhaseFigures["gpuProcess"] =
    trace === undefined
      ? missing("no trace of this phase")
      : trace.gpuProcess.value === null
        ? missing(trace.gpuProcess.reason)
        : primaryDrawCount === 0
          ? missing("the primary drew no frame in the window")
          : measured({
              busyMsPerFrame: trace.gpuProcess.value.busyMs / primaryDrawCount,
              slicesMsPerFrame: trace.gpuProcess.value.slices.map((slice) => ({
                name: slice.name,
                ms: slice.totalMs / primaryDrawCount,
              })),
              copySlicesPerFrame:
                trace.gpuProcess.value.copySlices.reduce((sum, slice) => sum + slice.count, 0) /
                primaryDrawCount,
              copySlices: trace.gpuProcess.value.copySlices,
            });
  const mainThreadMs = durationStats(record.mainThreadMs);
  const frameGpuMs = durationStats(record.frameGpuMs);
  const headroom = ofPeriod(periodMs, (t) => 0.8 * t);
  const rows = [
    ...frameRows(
      presentation.value === null ? missing(presentation.reason) : measured(presentation.value),
      periodMs,
      rateHz === 30 ? "low" : "high",
    ),
    row(
      "headroom-main",
      "main thread ≤ 0.8 T at the 95th percentile",
      headroom,
      "ms",
      mainThreadMs === null ? missing("no animation frame") : measured(mainThreadMs.p95Ms),
    ),
    row(
      "headroom-gpu",
      "GPU pass sum ≤ 0.8 T at the 95th percentile",
      headroom,
      "ms",
      frameGpuMs === null ? missing("no timed frame") : measured(frameGpuMs.p95Ms),
    ),
  ];
  return {
    name: record.name,
    purpose: PHASE_PURPOSE[record.name],
    held,
    rateHz,
    periodMs,
    windowS: (record.endMs - record.startMs) / 1000,
    views: record.views.map(viewFigures),
    animationFrames: frameStats(record.frameIntervalsMs, vsyncMs.value),
    primaryDraws: frameStats(record.primaryIntervalsMs, period),
    presentation,
    mainThreadMs,
    frameGpuMs,
    untimedFrames: record.untimedFrames,
    droppedResolves: record.droppedResolves,
    unattributedGpuMs: record.unattributedGpuMs,
    gpuProcess,
    // A phase that did not hold its configuration measured something else.
    criteria: { rows, overall: held ? overallOf(rows) : "not-measured" },
  };
}

/** The GPU process's main thread a primary draw in `phase`, or why it is missing. */
function gpuProcessPerFrame(phase: PhaseFigures | undefined): Measured<number> {
  return phase === undefined
    ? missing("the phase did not run")
    : !phase.held
      ? missing(`${phase.name} did not hold its configuration`)
      : phase.gpuProcess.value === null
        ? missing(phase.gpuProcess.reason)
        : measured(phase.gpuProcess.value.busyMsPerFrame);
}

function overheadPair(
  phases: ReadonlyArray<PhaseFigures>,
  without: ViewsCheckPhaseName,
  withInstruments: ViewsCheckPhaseName,
): OverheadPair {
  const alone = phases.find((phase) => phase.name === without);
  const opened = phases.find((phase) => phase.name === withInstruments);
  const extraCanvases = Math.max(0, (opened?.views.length ?? 1) - (alone?.views.length ?? 1));
  const a = gpuProcessPerFrame(alone);
  const b = gpuProcessPerFrame(opened);
  const perCanvasMs: Measured<number> =
    a.value === null
      ? missing(a.reason)
      : b.value === null
        ? missing(b.reason)
        : extraCanvases === 0
          ? missing("no canvas was added")
          : measured((b.value - a.value) / extraCanvases);
  return {
    without,
    with: withInstruments,
    extraCanvases,
    gpuProcessMsPerFrame: { without: a, with: b },
    perCanvasMs,
    instruments: (opened?.views ?? []).slice(1).map((view) => ({
      name: view.name,
      gpuMsPerDraw: view.gpuMs === null ? missing("no timed draw") : measured(view.gpuMs.meanMs),
      submitMsPerDraw: view.submitMs === null ? missing("no draw") : measured(view.submitMs.meanMs),
    })),
  };
}

/** Whose an allocation is when no shown view's name begins it: the engine's own. */
export const ENGINE_OWNER = "engine";

/** A canvas's size, device px. */
type CanvasSize = ViewsCheckRecord["resize"]["before"][number]["canvas"];

/** Whether two canvases, both known, are the same size. */
function sameSize(a: CanvasSize | undefined, b: CanvasSize | undefined): boolean {
  return a !== undefined && b !== undefined && a.widthPx === b.widthPx && a.heightPx === b.heightPx;
}

/** A view's canvas after a step of the resize. */
function sizeIn(
  step: ViewsCheckRecord["resize"]["steps"][number],
  name: string,
): CanvasSize | undefined {
  return step.views.find((view) => view.name === name)?.canvas;
}

function resizeFinding(
  record: ViewsCheckRecord["resize"],
): ViewsCheckResults["findings"]["resize"] {
  const before = new Map(record.before.map(({ name, canvas }) => [name, canvas]));
  const primary = "view";
  const narrowed = record.steps.filter((step) => step.widthFraction < 1);
  const restored = record.steps.at(-1);
  const primaryResized =
    narrowed.length > 0 &&
    narrowed.every((step) => !sameSize(sizeIn(step, primary), before.get(primary))) &&
    restored !== undefined &&
    sameSize(sizeIn(restored, primary), before.get(primary));
  const others = record.before.filter(({ name }) => name !== primary).map(({ name }) => name);
  const views = new Map<string, { created: number; destroyed: number; names: Set<string> }>();
  for (const event of record.allocations) {
    // A name no view's begins with is the engine's own, such as the pass timer's buffers.
    const prefix = event.name.split(/[: ]/u)[0] ?? event.name;
    const view = record.before.some(({ name }) => name === prefix) ? prefix : ENGINE_OWNER;
    const entry = views.get(view) ?? { created: 0, destroyed: 0, names: new Set<string>() };
    if (event.kind === "created") {
      entry.created += 1;
    } else {
      entry.destroyed += 1;
    }
    entry.names.add(event.name);
    views.set(view, entry);
  }
  const othersUnchanged =
    others.every((name) =>
      record.steps.every((step) => sameSize(sizeIn(step, name), before.get(name))),
    ) && others.every((name) => !views.has(name));
  const primaryRemade = (views.get(primary)?.created ?? 0) > 0;
  return {
    verdict:
      record.steps.length === 0
        ? "not-measured"
        : primaryResized && primaryRemade && othersUnchanged
          ? "pass"
          : "fail",
    primaryResized,
    othersUnchanged,
    byView: [...views].map(([view, entry]) => ({
      view,
      created: entry.created,
      destroyed: entry.destroyed,
      names: [...entry.names].toSorted(),
    })),
    record,
  };
}

/** The results of a run, judged. */
export function buildViewsCheckResults(input: ViewsCheckInput): ViewsCheckResults {
  const { run, record } = input;
  const displayHz: Measured<number> =
    run.displayHz === null || run.displayHz <= 0
      ? missing(run.shown ? "the display reports no refresh rate" : NO_WINDOW_REASON)
      : measured(run.displayHz);
  const vsyncMs: Measured<number> =
    displayHz.value === null ? missing(displayHz.reason) : measured(1000 / displayHz.value);
  const phases = record.phases.map((phase) =>
    phaseFigures(phase, input.traces.get(phase.name), vsyncMs, run.setting),
  );
  const copyPasses = [
    ...new Set(
      phases.flatMap((phase) =>
        phase.views.flatMap((view) => view.passLabels.filter((label) => COPY_LABEL.test(label))),
      ),
    ),
  ].toSorted();
  const passesSeen = phases.some((phase) => phase.views.some((view) => view.passLabels.length > 0));
  const cockpit = phases.find((phase) => phase.name === "photoreal-two-wireframe");
  const pairs = OVERHEAD_PAIRS.map(([without, withInstruments]) =>
    overheadPair(phases, without, withInstruments),
  );
  const measuredPairs = pairs.flatMap((pair) =>
    pair.perCanvasMs.value === null ? [] : [pair.perCanvasMs.value],
  );
  const droppedResolves = phases.reduce((sum, phase) => sum + phase.droppedResolves, 0);
  const untimedFrames = phases.reduce((sum, phase) => sum + phase.untimedFrames, 0);
  const load = run.machine.loadAverage[0];
  const low = run.setting === "low";
  // Design note 27's rule as R05.T20 reads it on every platform: a Windows run keeps no load
  // average, so whether its machine was quiet is unchecked and it is always provisional.
  const quiet = quietOf(run.platform, run.machine.loadAverage);
  // Since R07.T17 the low setting reaches VIEW's sky and photorealistic frame too, so a low run
  // is provisional only as a high one is.
  const provisional = quiet.provisional || run.smoke || !run.shown;
  const loadNote = keepsLoadAverage(run.platform)
    ? `load average ${load.toFixed(2)} at the start (under 1 asked)`
    : QUIET_RULE_UNCHECKED;
  const quietNote = [
    ...(quiet.provisional ? [loadNote] : []),
    ...(run.shown ? [] : ["a hidden run: no presentation and no T"]),
    ...(run.smoke ? ["a smoke run: short windows"] : []),
  ].join("; ");
  const verdictOf = (name: ViewsCheckPhaseName): Verdict =>
    phases.find((phase) => phase.name === name)?.criteria.overall ?? "not-measured";
  const firstMissed = verdictOf("wireframe-two-wireframe") === "fail";
  return {
    schema: VIEWS_CHECK_SCHEMA,
    version: VIEWS_CHECK_VERSION,
    run: {
      startedAt: run.startedAt.toISOString(),
      machine: run.machine,
      versions: run.versions,
      platform: run.platform,
      launchMode: run.launchMode,
      setting: run.setting,
      smoke: run.smoke,
      switches: run.switches,
      shown: run.shown,
      window: run.window,
      devicePixelRatio: record.devicePixelRatio,
      timer: record.timer,
      displayHz,
      vsyncMs,
      captures: run.captures,
      quiet: { provisional, note: provisional ? `provisional: ${quietNote}` : null },
    },
    phases,
    findings: {
      rightWayUp: {
        verdict:
          input.rightWayUp.value === null
            ? "not-measured"
            : input.rightWayUp.value === "yes"
              ? "pass"
              : "fail",
        answer: input.rightWayUp,
      },
      copies: {
        verdict: !passesSeen ? "not-measured" : copyPasses.length === 0 ? "pass" : "fail",
        copyPasses,
        gpuProcessCopiesPerFrame: phases.map((phase) => ({
          phase: phase.name,
          perFrame:
            phase.gpuProcess.value === null
              ? missing(phase.gpuProcess.reason)
              : measured(phase.gpuProcess.value.copySlicesPerFrame),
        })),
      },
      resize: resizeFinding(record.resize),
      frameTime: {
        phase: "photoreal-two-wireframe",
        verdict: cockpit?.criteria.overall ?? "not-measured",
        rows: cockpit?.criteria.rows ?? [],
      },
      lowCases: (["wireframe-two-wireframe", "wireframe-photoreal-wireframe"] as const).map(
        (name) => ({
          phase: name,
          verdict: verdictOf(name),
          consequence: LOW_CASE_CONSEQUENCE[name] ?? "",
          applies: low,
          triggered:
            low &&
            verdictOf(name) === "fail" &&
            (name === "wireframe-two-wireframe" || !firstMissed),
        }),
      ),
      perCanvasOverhead: {
        provisionalMs: record.perCanvasOverheadMs,
        pairs,
        measuredMs:
          measuredPairs.length === 0
            ? missing("no pair of phases had both GPU-process figures")
            : measured(Math.max(...measuredPairs)),
      },
      passTimer: {
        verdict:
          input.warnings.passTimerDrops === 0 && droppedResolves === 0
            ? record.timer === "absent"
              ? "not-measured"
              : "pass"
            : "fail",
        consoleDrops: input.warnings.passTimerDrops,
        untimedPassWarnings: input.warnings.untimedPasses,
        droppedResolves,
        untimedFrames,
      },
      faults: {
        verdict: record.faults.length === 0 && input.gpuProcessExits === 0 ? "pass" : "fail",
        faults: record.faults,
        gpuProcessExits: input.gpuProcessExits,
      },
    },
  };
}

/** A figure's text, or its reason in brackets. */
function textOf<T>(figure: Measured<T>, text: (value: T) => string): string {
  return figure.value === null ? `— (${figure.reason})` : text(figure.value);
}

function ms(value: number): string {
  return value >= 10 ? value.toFixed(1) : value.toFixed(2);
}

function intervals(stats: FrameStats | null): string {
  return stats === null
    ? "—"
    : `${ms(stats.p50Ms)} / ${ms(stats.p95Ms)} / ${ms(stats.p99Ms)} (max ${ms(stats.maxMs)}, n ${String(stats.count)})`;
}

function durations(stats: DurationStats | null): string {
  return stats === null ? "—" : `${ms(stats.p50Ms)} / ${ms(stats.p95Ms)}`;
}

function rowText(entry: Criterion): string {
  const value =
    entry.value === null
      ? "—"
      : entry.unit === "fraction"
        ? `${(entry.value * 100).toFixed(2)}%`
        : entry.unit === "ms"
          ? `${ms(entry.value)} ms`
          : String(entry.value);
  const limit =
    entry.limit === null
      ? "—"
      : entry.unit === "fraction"
        ? `${(entry.limit * 100).toFixed(0)}%`
        : entry.unit === "ms"
          ? `${ms(entry.limit)} ms`
          : String(entry.limit);
  return `${entry.criterion}: ${value} against ${limit}, ${entry.verdict}${entry.note === null ? "" : ` (${entry.note})`}`;
}

/** The results' summary, for a person reading the record. */
export function viewsCheckMarkdown(results: ViewsCheckResults): string {
  const { run, phases, findings } = results;
  const overhead = findings.perCanvasOverhead;
  const cockpit = phases.find((phase) => phase.name === findings.frameTime.phase);
  const resizeText = (): string => {
    const views = findings.resize.byView
      .map(
        (entry) =>
          `${entry.view} ${String(entry.created)} made, ${String(entry.destroyed)} destroyed`,
      )
      .join("; ");
    const primary = findings.resize.record.steps
      .map((step) => {
        const canvas = step.views.find((view) => view.name === "view")?.canvas;
        return canvas === undefined
          ? "?"
          : `${String(canvas.widthPx)} × ${String(canvas.heightPx)}`;
      })
      .join(" → ");
    const longest = findings.resize.record.steps
      .map((step) => `${ms(step.longestFrameMs)} ms`)
      .join(", ");
    return `the primary's canvas ${primary}; the instruments' ${findings.resize.othersUnchanged ? "unchanged" : "CHANGED"}; allocations: ${views.length > 0 ? views : "none"}; the longest frame at each step (the bloom's refit) ${longest}`;
  };
  const copiesText = (): string => {
    const per = findings.copies.gpuProcessCopiesPerFrame
      .map((entry) => `${entry.phase} ${textOf(entry.perFrame, (value) => value.toFixed(2))}`)
      .join(", ");
    return `the views' passes named as copies: ${findings.copies.copyPasses.length === 0 ? "none" : findings.copies.copyPasses.join(", ")}; the GPU process's copy slices a frame: ${per}`;
  };
  const answer = findings.rightWayUp.answer;
  return [
    `# Several views: ${run.machine.name}, ${run.setting}, ${run.startedAt.slice(0, 10)}`,
    "",
    `- **Machine:** ${run.machine.cpu}, ${String(run.machine.logicalCores)} threads; GPU ${textOf(run.machine.gpu, (gpu) => gpu.description ?? `${String(gpu.vendorId)}:${String(gpu.deviceId)}`)}; governor ${textOf(run.machine.governor, (governor) => governor)}; load average ${keepsLoadAverage(run.platform) ? run.machine.loadAverage.map((value) => value.toFixed(2)).join(", ") : "none"}${run.quiet.note === null ? "" : ` (${run.quiet.note})`}`,
    `- **Versions:** app ${run.versions.app}, Electron ${run.versions.electron}, Chromium ${run.versions.chromium}`,
    `- **Launch:** ${run.platform}, ${run.launchMode} mode, timer ${run.timer}, setting ${run.setting}, window ${run.shown ? "shown" : "hidden"} ${String(run.window.widthDip)} × ${String(run.window.heightDip)} DIP (${String(Math.round(run.window.widthDip * run.devicePixelRatio))} × ${String(Math.round(run.window.heightDip * run.devicePixelRatio))} px at a device-pixel ratio of ${String(run.devicePixelRatio)}), vsync ${textOf(run.vsyncMs, (value) => `${ms(value)} ms`)}`,
    `- **Captures (not committed):** ${textOf(run.captures, (dir) => `\`${dir}\``)}`,
    "",
    "## T20's checks",
    "",
    "| Check | Verdict | Reading |",
    "| --- | --- | --- |",
    `| Each view the right way up (by eye) | ${findings.rightWayUp.verdict} | ${answer.value === null ? `not answered (${answer.reason})` : `answered "${answer.value}" at the run`} |`,
    `| No GPU time in copies | ${findings.copies.verdict} | ${copiesText()} |`,
    `| A resize of the primary leaves the instruments' attachments alone | ${findings.resize.verdict} | ${resizeText()} |`,
    `| The frame time with the instruments open (${findings.frameTime.phase}) | ${findings.frameTime.verdict} | ${cockpit === undefined ? "the phase did not run" : `presented ${textOf(cockpit.presentation, (stats) => intervals(stats))}; GPU frame ${durations(cockpit.frameGpuMs)} ms; T ${textOf(cockpit.periodMs, (value) => `${ms(value)} ms`)}`} |`,
    `| The pass timer's drop warning never appears | ${findings.passTimer.verdict} | ${String(findings.passTimer.consoleDrops)} warnings, ${String(findings.passTimer.droppedResolves)} resolves dropped, ${String(findings.passTimer.untimedFrames)} frames untimed, ${String(findings.passTimer.untimedPassWarnings)} "untimed passes" warnings |`,
    `| No fault, no GPU-process exit | ${findings.faults.verdict} | ${findings.faults.faults.length === 0 ? "no fault" : findings.faults.faults.join("; ")}; ${String(findings.faults.gpuProcessExits)} exits |`,
    `| The per-canvas overhead (\`PER_CANVAS_OVERHEAD_MS\` ${ms(overhead.provisionalMs)} ms) | — | ${textOf(overhead.measuredMs, (value) => `at most ${value.toFixed(3)} ms a canvas a primary draw, of the GPU process's main thread, the instruments' own command decoding included`)}: ${overhead.pairs.map((pair) => `${pair.with} against ${pair.without} ${textOf(pair.perCanvasMs, (value) => `${value.toFixed(3)} ms`)}`).join("; ")}; for the owner's ruling |`,
    ...findings.lowCases.map(
      (entry) =>
        `| ${entry.phase} at 60 Hz${entry.applies ? "" : " (T20 asks it on the low setting)"} | ${entry.verdict} | ${entry.triggered ? `**this follows:** ${entry.consequence}` : entry.consequence} |`,
    ),
    "",
    "Rows of the frame time (Design note 21):",
    "",
    ...findings.frameTime.rows.map((entry) => `- ${rowText(entry)}`),
    "",
    "## Phases",
    "",
    "Intervals as p50 / p95 / p99 ms; durations as p50 / p95 ms.",
    "",
    "| Phase | Held | T | Primary's draws | Presented (dropped) | Main thread | GPU frame | GPU process a frame | Verdict |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ...phases.map(
      (phase) =>
        `| ${phase.name} | ${phase.held ? "yes" : "NO"} | ${textOf(phase.periodMs, (value) => `${ms(value)} ms`)} | ${intervals(phase.primaryDraws)} | ${textOf(phase.presentation, (stats) => `${intervals(stats)} (${String(stats.dropped)})`)} | ${durations(phase.mainThreadMs)} | ${durations(phase.frameGpuMs)} | ${textOf(phase.gpuProcess, (value) => `${ms(value.busyMsPerFrame)} ms (${value.slicesMsPerFrame.map((slice) => `${slice.name} ${ms(slice.ms)}`).join(", ")})`)} | ${phase.criteria.overall} |`,
    ),
    "",
    "## Views",
    "",
    "| Phase | View | Style | Canvas (px) | Draws | GPU (ms) | Submit (ms) | Scale | Passes |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ...phases.flatMap((phase) =>
      phase.views.map(
        (view) =>
          `| ${phase.name} | ${view.slot} | ${view.style} | ${String(view.canvas.widthPx)} × ${String(view.canvas.heightPx)} | ${String(view.draws)} | ${durations(view.gpuMs)} | ${durations(view.submitMs)} | ${view.scale === null ? "—" : `${view.scale.min.toFixed(2)}–${view.scale.max.toFixed(2)}`} | ${view.passLabels.join(", ")} |`,
      ),
    ),
    "",
    "## For the owner, by eye",
    "",
    `- [${answer.value === "yes" ? "x" : " "}] Each view the right way up: ${answer.value === null ? "not answered at the run; look at the captures" : `answered "${answer.value}" at the run`}.`,
    "- [ ] Nothing flickered or tore while the primary was resized.",
    "",
  ].join("\n");
}

/** Runs of one day, machine and setting the writer numbers before it gives up. */
const MAX_RUNS_A_DAY = 100;

const NODE_FILES: ResultsFiles = {
  mkdir: (path) => mkdir(path, { recursive: true }),
  exists: (path) =>
    access(path).then(
      () => true,
      () => false,
    ),
  writeFile: (path, text) => writeFile(path, text, "utf8"),
};

/**
 * Writes the results and their summary into `dir` as `<date>-<machine>-<setting>.json` and `.md`
 * (`-smoke` before the extension for a smoke run), adding `-2`, `-3` and so on where a run of the
 * same name is there, and warns of a file over the repository's limit for added files.
 *
 * @throws Error if the day's names are used up.
 */
export async function writeViewsCheckResults(
  dir: string,
  results: ViewsCheckResults,
  files: ResultsFiles = NODE_FILES,
): Promise<{ readonly json: string; readonly markdown: string }> {
  await files.mkdir(dir);
  const stem = `${results.run.startedAt.slice(0, 10)}-${results.run.machine.name}-${results.run.setting}${results.run.smoke ? "-smoke" : ""}`;
  const candidates = Array.from({ length: MAX_RUNS_A_DAY }, (_, i) =>
    i === 0 ? stem : `${stem}-${String(i + 1)}`,
  );
  const taken = await Promise.all(
    candidates.map((candidate) => files.exists(join(dir, `${candidate}.json`))),
  );
  const name = candidates[taken.indexOf(false)];
  if (name === undefined) {
    throw new Error(`${dir} already holds ${String(MAX_RUNS_A_DAY)} runs named ${stem}`);
  }
  const json = join(dir, `${name}.json`);
  const markdown = join(dir, `${name}.md`);
  const text = `${JSON.stringify(results, null, 2)}\n`;
  await files.writeFile(json, text);
  await files.writeFile(markdown, viewsCheckMarkdown(results));
  const bytes = Buffer.byteLength(formatAsPrettier(text), "utf8");
  if (bytes > ADDED_FILE_LIMIT_BYTES) {
    console.warn(
      `views check: ${json} is ${String(bytes)} B once formatted, over the ${String(ADDED_FILE_LIMIT_BYTES)} B the repository accepts for an added file`,
    );
  }
  return { json, markdown };
}
