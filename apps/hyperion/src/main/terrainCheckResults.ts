/**
 * The views check's terrain run's results file and its summary (plan R10, R10.T16.d;
 * decision-r10-t16-contracts §6): what the renderer measured of each phase on `TERRAIN TEST`,
 * judged against what the hidden run holds.
 *
 * @remarks
 * The run holds, and each is a finding here with its verdict:
 *
 * - **The phases**: every phase of the launch's setting ran, in order, and held its views' styles.
 * - **Every frame drawn**: no view missed an animation frame it was due at its own cadence, in any
 *   phase, outside the forced loss's restore; the longest time between two of its draws is
 *   recorded beside it, provisional (a stall of the whole page, when no animation frame runs).
 * - **Every frame finite**: every read-back of a photorealistic view's scene target was finite.
 * - **The terrain drawn once handed over**: the primary handed `TEST MOON` over in the first phase,
 *   and every view that handed it over with a drawn patch in its view, a photorealistic one to its
 *   lit pass or a wireframe one to its wireframe pass (plan R10, R10.T11), drew its style's terrain
 *   material within {@link TERRAIN_DELAY_LIMIT_MS}, and in every such draw after (outside the
 *   restore); a view past the survey, which sees no patch, draws the reference surface alone, and
 *   its draws are counted apart.
 * - **The faults**: exactly one `device-lost` with reason `destroyed` inside the loss phase's
 *   window, then exactly one restore, and no other fault and no exit of the GPU process
 *   ({@link terrainFaultsVerdict}).
 * - **The loss**: the device destroyed with blocks still to come and bakes in flight; after it the
 *   terrain drawn again, no `surface_field` asked, no `field-init` posted, no `field-chunk` but the
 *   answer in flight's blocks, once to each worker; and the mask holding the same cells.
 * - **The teardown**: the session disposed, no height worker left and no GPU bytes of the terrain's.
 *
 * "No pixel drawn by both surfaces on the datum" is held by T16.c's part-6 smoke group
 * (`R10.T16 the frame`, on SwiftShader), which reads both surfaces back; the hidden run records it
 * as held there and probes no production shader.
 *
 * The patches drawn, the bakes a second, the frame times and the session's memory are recorded,
 * provisional: the run is hidden, and a timed figure on a shared machine is provisional (the load
 * average at the start is recorded with it).
 */

import type {
  SpikePassTimer,
  TerrainCheckBytes,
  TerrainCheckFault,
  TerrainCheckMemory,
  TerrainCheckPhaseName,
  TerrainCheckPhaseRecord,
  TerrainCheckRecord,
  ViewsCheckStyle,
  ViewsCheckViewName,
} from "../preload/api";
import type { GraphicsLaunchMode } from "../preload/api";
import { keepsLoadAverage, QUIET_RULE_UNCHECKED, quietOf } from "./machineLoad";
import {
  ADDED_FILE_LIMIT_BYTES,
  formatAsPrettier,
  type MachineDescription,
  type Measured,
  type ResultsFiles,
  type SpikeSetting,
  type Verdict,
} from "./results";
import { type DurationStats, durationStats, type ViewSlotName } from "./viewsCheckResults";
import { access, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

/** The file's schema name. */
export const TERRAIN_CHECK_SCHEMA = "hyperion.views-check.terrain-results";

/** The schema's version; bumped with any change to the file's shape. */
export const TERRAIN_CHECK_VERSION = 1;

/**
 * The longest from a view's hand-over to its first terrain draw, ms: the lit material compiles and
 * the first bakes land. The first compile of a launch took 4.0 s on the development machine
 * (2026-10-20, hidden, high), the second 0.13 s; the limit catches a terrain that never draws.
 */
export const TERRAIN_DELAY_LIMIT_MS = 15_000;

/** One phase a launch runs, and the styles it shows: the primary's, then each open instrument's. */
export interface TerrainPhasePlan {
  readonly name: TerrainCheckPhaseName;
  readonly views: ReadonlyArray<ViewsCheckStyle>;
}

/**
 * The phases a launch of each setting runs, in order (decision-r10-t16-contracts §6): on high the
 * primary alone, with a photorealistic and a wireframe instrument, and the loss; on low the primary
 * alone, with a wireframe instrument, and the loss.
 */
export const TERRAIN_PHASES: Readonly<Record<SpikeSetting, ReadonlyArray<TerrainPhasePlan>>> = {
  high: [
    { name: "terrain-alone", views: ["photorealistic"] },
    {
      name: "terrain-photoreal-wireframe",
      views: ["photorealistic", "photorealistic", "wireframe"],
    },
    { name: "terrain-loss", views: ["photorealistic"] },
  ],
  low: [
    { name: "terrain-alone", views: ["photorealistic"] },
    { name: "terrain-wireframe", views: ["photorealistic", "wireframe"] },
    { name: "terrain-loss", views: ["photorealistic"] },
  ],
};

/** What each phase is for, in the summary's words. */
const PHASE_PURPOSE: Readonly<Record<TerrainCheckPhaseName, string>> = {
  "terrain-alone":
    "the photorealistic primary alone, through the hand-over, the descent and the sunset",
  "terrain-photoreal-wireframe":
    "the same, with instrument 1 photorealistic and instrument 2 in the wireframe",
  "terrain-wireframe":
    "the primary with one wireframe instrument (QUALITY LOW allows one photorealistic view)",
  "terrain-loss": "a fresh session, its device destroyed by the harness mid-stream",
};

/** Where "no pixel drawn by both surfaces on the datum" is held (decision-r10-t16-contracts §6). */
export const BOTH_SURFACES_HELD_BY =
  "held by T16.c's part-6 smoke group, `R10.T16 the frame`, on SwiftShader; not probed here";

/** The run, as the main process describes it at its start. */
export interface TerrainCheckRunInput {
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
  readonly switches: ReadonlyArray<string>;
  readonly shown: boolean;
  /** The window's content size when the run ended, DIP. */
  readonly window: { readonly widthDip: number; readonly heightDip: number };
  /** Where the page's captures were saved, or why none were. */
  readonly captures: Measured<string>;
}

/** Everything the results are built from. */
export interface TerrainCheckInput {
  readonly run: TerrainCheckRunInput;
  readonly record: TerrainCheckRecord;
  /** The GPU process's exits over the run. */
  readonly gpuProcessExits: number;
  /** The page's console errors over the run, in order. */
  readonly consoleErrors: ReadonlyArray<string>;
}

/** One view's figures over a phase. */
export interface TerrainViewFigures {
  readonly name: ViewsCheckViewName;
  readonly slot: ViewSlotName;
  readonly style: ViewsCheckStyle;
  readonly canvas: { readonly widthPx: number; readonly heightPx: number };
  readonly draws: number;
  readonly gpuMs: DurationStats | null;
  readonly submitMs: DurationStats | null;
  readonly missedFrames: number;
  readonly longestGapMs: number;
  readonly handedOverDraws: number;
  readonly offSurveyDraws: number;
  readonly terrainDraws: number;
  readonly terrainGaps: number;
  readonly terrainGapSpansS: ReadonlyArray<readonly [number, number]>;
  readonly firstHandoverScriptS: number | null;
  /** From its first hand-over to its first terrain draw, ms; `null` where either is missing. */
  readonly terrainDelayMs: number | null;
  readonly readBacks: number;
  readonly nonFiniteTexels: number;
}

/** One phase's figures. */
export interface TerrainPhaseFigures {
  readonly name: TerrainCheckPhaseName;
  readonly purpose: string;
  /** Whether its views drew the styles the phase asks for when it ended. */
  readonly held: boolean;
  readonly windowS: number;
  readonly script: { readonly startS: number; readonly endS: number };
  readonly views: ReadonlyArray<TerrainViewFigures>;
  readonly animationFrames: DurationStats | null;
  readonly mainThreadMs: DurationStats | null;
  readonly frameGpuMs: DurationStats | null;
  /** The patches drawn at the primary's draws while the body had a pass. */
  readonly patchesDrawn: {
    readonly draws: number;
    readonly mean: number;
    readonly max: number;
    readonly last: number;
  } | null;
  readonly bakes: TerrainCheckPhaseRecord["bakes"];
  /** Bakes answered a second over the window. */
  readonly bakedPerS: number;
  readonly fieldPosts: TerrainCheckPhaseRecord["fieldPosts"];
  readonly fieldAsks: number;
  readonly blocksSent: number;
  readonly memory: TerrainCheckMemory | null;
}

/** A finding's verdict and why it failed, if it did. */
export interface TerrainFinding {
  readonly verdict: Verdict;
  /** What failed, in words; empty for a pass. */
  readonly reasons: ReadonlyArray<string>;
}

/** The faults verdict, with the fault it excludes. */
export interface TerrainFaultsFinding extends TerrainFinding {
  /** The forced loss's one `device-lost`, excluded from the verdict, or `null` where none was. */
  readonly expected: TerrainCheckFault | null;
  /** Every other fault, each a failure. */
  readonly others: ReadonlyArray<TerrainCheckFault>;
  /** The restores after the expected loss inside the loss phase's window. */
  readonly restores: number;
  readonly gpuProcessExits: number;
}

/** A results file of the terrain run. */
export interface TerrainCheckResults {
  readonly schema: typeof TERRAIN_CHECK_SCHEMA;
  readonly version: typeof TERRAIN_CHECK_VERSION;
  readonly run: {
    readonly startedAt: string;
    readonly machine: MachineDescription;
    readonly versions: TerrainCheckRunInput["versions"];
    readonly platform: NodeJS.Platform;
    readonly launchMode: GraphicsLaunchMode;
    readonly setting: SpikeSetting;
    readonly smoke: boolean;
    readonly switches: ReadonlyArray<string>;
    readonly shown: boolean;
    readonly window: TerrainCheckRunInput["window"];
    readonly devicePixelRatio: number;
    readonly timer: SpikePassTimer;
    readonly captures: Measured<string>;
    /** Every figure is provisional: the note says why. */
    readonly provisional: string;
  };
  readonly phases: ReadonlyArray<TerrainPhaseFigures>;
  readonly findings: {
    readonly phases: TerrainFinding;
    readonly framesDrawn: TerrainFinding;
    readonly framesFinite: TerrainFinding & { readonly readBacks: number };
    readonly terrainDrawn: TerrainFinding;
    readonly faults: TerrainFaultsFinding;
    readonly loss: TerrainFinding & { readonly record: TerrainCheckRecord["loss"] };
    readonly teardown: TerrainFinding & { readonly record: TerrainCheckRecord["teardown"] };
    readonly bothSurfaces: string;
    /** The hand-over's range and script time, and when the primary handed over in the first phase. */
    readonly handover: {
      readonly rangeM: number | null;
      readonly expectedScriptS: number | null;
      readonly observedScriptS: number | null;
    };
    readonly consoleErrors: { readonly count: number; readonly first: ReadonlyArray<string> };
  };
  /** Every finding's verdict together: `pass` only if each passes. */
  readonly overall: Verdict;
}

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

function phaseFigures(record: TerrainCheckPhaseRecord, setting: SpikeSetting): TerrainPhaseFigures {
  const expected = TERRAIN_PHASES[setting].find((plan) => plan.name === record.name)?.views ?? [];
  const held =
    record.views.length === expected.length &&
    record.views.every((view, i) => view.style === expected[i]);
  const windowS = (record.endMs - record.startMs) / 1000;
  const patches = record.patchesDrawn;
  const last = patches.at(-1);
  return {
    name: record.name,
    purpose: PHASE_PURPOSE[record.name],
    held,
    windowS,
    script: { startS: record.scriptStartS, endS: record.scriptEndS },
    views: record.views.map((view) => ({
      name: view.name,
      slot: slotOf(view.name),
      style: view.style,
      canvas: view.canvas,
      draws: view.draws,
      gpuMs: durationStats(view.gpuMs),
      submitMs: durationStats(view.submitMs),
      missedFrames: view.missedFrames,
      longestGapMs: view.longestGapMs,
      handedOverDraws: view.handedOverDraws,
      offSurveyDraws: view.offSurveyDraws,
      terrainDraws: view.terrainDraws,
      terrainGaps: view.terrainGaps,
      terrainGapSpansS: view.terrainGapSpansS,
      firstHandoverScriptS: view.firstHandoverScriptS,
      terrainDelayMs:
        view.firstHandoverMs === null || view.firstTerrainMs === null
          ? null
          : view.firstTerrainMs - view.firstHandoverMs,
      readBacks: view.readBacks,
      nonFiniteTexels: view.nonFiniteTexels,
    })),
    animationFrames: durationStats(record.frameIntervalsMs),
    mainThreadMs: durationStats(record.mainThreadMs),
    frameGpuMs: durationStats(record.frameGpuMs),
    patchesDrawn:
      last === undefined
        ? null
        : {
            draws: patches.length,
            mean: patches.reduce((sum, count) => sum + count, 0) / patches.length,
            max: patches.reduce((most, count) => Math.max(most, count), 0),
            last,
          },
    bakes: record.bakes,
    bakedPerS: windowS > 0 ? record.bakes.baked / windowS : 0,
    fieldPosts: record.fieldPosts,
    fieldAsks: record.fieldAsks,
    blocksSent: record.blocksSent,
    memory: record.memory,
  };
}

/** A finding from its failures: `pass` with none. */
function finding(reasons: ReadonlyArray<string>): TerrainFinding {
  return { verdict: reasons.length === 0 ? "pass" : "fail", reasons };
}

/** What {@link terrainFaultsVerdict} judges. */
export interface TerrainFaultsInput {
  readonly faults: ReadonlyArray<TerrainCheckFault>;
  readonly restoresAtMs: ReadonlyArray<number>;
  /** The loss phase's window, ms, or `null` where it did not run. */
  readonly lossWindow: { readonly startMs: number; readonly endMs: number } | null;
  /**
   * When the harness destroyed the device (the record's loss), ms, or `null` where it did not: the
   * loss excluded is the one at or after it, so that a loss of the device's own just before is
   * not taken for the harness's (the batch review's D-C).
   */
  readonly destroyedAtMs: number | null;
  readonly gpuProcessExits: number;
}

/**
 * The faults verdict (decision-r10-t16-contracts §6): the forced loss is expected exactly once, one
 * `device-lost` with reason `destroyed` inside the loss phase's window, then one restore; that one
 * fault alone is excluded. Any other fault, a second loss, a loss outside the window, no loss, or
 * an exit of the GPU process fails it.
 */
export function terrainFaultsVerdict(input: TerrainFaultsInput): TerrainFaultsFinding {
  const { faults, restoresAtMs, lossWindow, destroyedAtMs, gpuProcessExits } = input;
  const reasons: string[] = [];
  const losses = faults.filter((fault) => fault.kind === "device-lost");
  const inside = (atMs: number): boolean =>
    lossWindow !== null && atMs >= lossWindow.startMs && atMs <= lossWindow.endMs;
  const harnessed = (atMs: number): boolean => destroyedAtMs !== null && atMs >= destroyedAtMs;
  const expected =
    losses.length === 1 &&
    losses[0]?.reason === "destroyed" &&
    inside(losses[0].atMs) &&
    harnessed(losses[0].atMs)
      ? losses[0]
      : null;
  if (losses.length === 0) {
    reasons.push("no device loss was seen: the forced loss is expected once");
  } else if (losses.length > 1) {
    reasons.push(`${String(losses.length)} device losses: the forced loss is expected once`);
  } else if (expected === null) {
    const [loss] = losses;
    reasons.push(
      loss !== undefined && loss.reason !== "destroyed"
        ? `the device was lost with reason ${String(loss.reason)}, not destroyed`
        : loss !== undefined && inside(loss.atMs)
          ? "the device loss came before the harness destroyed the device"
          : "the device loss fell outside the loss phase's window",
    );
  }
  const others = faults.filter((fault) => fault !== expected && fault.kind !== "device-lost");
  for (const fault of others) {
    reasons.push(`a fault: ${fault.line}`);
  }
  const restores =
    expected === null
      ? 0
      : restoresAtMs.filter((atMs) => atMs >= expected.atMs && inside(atMs)).length;
  if (expected !== null && restores !== 1) {
    reasons.push(`${String(restores)} restores after the forced loss, where one is expected`);
  }
  const stray = restoresAtMs.length - restores;
  if (expected !== null && stray > 0) {
    reasons.push(`${String(stray)} restores outside the forced loss's`);
  }
  if (gpuProcessExits > 0) {
    reasons.push(`${String(gpuProcessExits)} exits of the GPU process`);
  }
  return { ...finding(reasons), expected, others, restores, gpuProcessExits };
}

/** The phases ran as the setting's plan, in order, each holding its configuration. */
function phasesFinding(
  phases: ReadonlyArray<TerrainPhaseFigures>,
  setting: SpikeSetting,
): TerrainFinding {
  const plan = TERRAIN_PHASES[setting].map((each) => each.name);
  const ran = phases.map((phase) => phase.name);
  const reasons: string[] = [];
  if (ran.join(",") !== plan.join(",")) {
    const ranText = ran.length === 0 ? "none" : ran.join(", ");
    reasons.push(`the phases ran ${ranText}, where ${plan.join(", ")} are asked`);
  }
  for (const phase of phases.filter((each) => !each.held)) {
    reasons.push(`${phase.name} did not hold its views' styles`);
  }
  return finding(reasons);
}

function framesDrawnFinding(phases: ReadonlyArray<TerrainPhaseFigures>): TerrainFinding {
  const reasons = phases.flatMap((phase) =>
    phase.views.flatMap((view) =>
      view.draws === 0
        ? [`${phase.name}: ${view.slot} drew nothing`]
        : view.missedFrames > 0
          ? [`${phase.name}: ${view.slot} missed ${String(view.missedFrames)} frames it was due`]
          : [],
    ),
  );
  return finding(reasons);
}

function framesFiniteFinding(
  phases: ReadonlyArray<TerrainPhaseFigures>,
): TerrainFinding & { readonly readBacks: number } {
  const photoreal = phases.flatMap((phase) =>
    phase.views.filter((view) => view.style === "photorealistic").map((view) => ({ phase, view })),
  );
  const readBacks = photoreal.reduce((sum, { view }) => sum + view.readBacks, 0);
  const reasons = photoreal.flatMap(({ phase, view }) =>
    view.readBacks === 0
      ? [`${phase.name}: ${view.slot}'s scene target was never read back`]
      : view.nonFiniteTexels > 0
        ? [`${phase.name}: ${view.slot} had ${String(view.nonFiniteTexels)} texels not finite`]
        : [],
  );
  return { ...finding(reasons), readBacks };
}

function terrainDrawnFinding(phases: ReadonlyArray<TerrainPhaseFigures>): TerrainFinding {
  const reasons: string[] = [];
  const primary = phases.find((phase) => phase.name === "terrain-alone")?.views[0];
  if (primary === undefined || primary.firstHandoverScriptS === null) {
    reasons.push("terrain-alone: the primary never handed TEST MOON over");
  }
  for (const phase of phases) {
    // A view of either style hands the body over: the photorealistic to its lit pass, the
    // wireframe to its wireframe pass (R10.T11), and each draws its own terrain's material.
    for (const view of phase.views.filter((each) => each.handedOverDraws > 0)) {
      if (view.terrainDelayMs === null) {
        reasons.push(`${phase.name}: ${view.slot} handed over and never drew the terrain`);
      } else if (view.terrainDelayMs > TERRAIN_DELAY_LIMIT_MS) {
        reasons.push(
          `${phase.name}: ${view.slot} drew the terrain ${view.terrainDelayMs.toFixed(0)} ms after its hand-over, over ${String(TERRAIN_DELAY_LIMIT_MS)} ms`,
        );
      }
      if (view.terrainGaps > 0) {
        const spans = view.terrainGapSpansS
          .map(([from, to]) => `${from.toFixed(1)}–${to.toFixed(1)} s`)
          .join(", ");
        reasons.push(
          `${phase.name}: ${view.slot} drew ${String(view.terrainGaps)} handed-over frames without the terrain after its first (${spans} of the script)`,
        );
      }
    }
  }
  return finding(reasons);
}

function lossFinding(record: TerrainCheckRecord["loss"]): TerrainFinding {
  const reasons: string[] = [];
  if (record.destroyedAtMs === null) {
    reasons.push(`the device was not destroyed: ${record.notDestroyed ?? "no reason given"}`);
    return finding(reasons);
  }
  if (!(record.blocksToCome > 0)) {
    reasons.push("no block was still to come at the destruction");
  }
  if (!(record.bakesInFlight > 0)) {
    reasons.push("no bake was in flight at the destruction");
  }
  if (record.terrainAgainAtMs === null) {
    reasons.push("the terrain was not drawn again after the restore");
  }
  const { after, workers, masks } = record;
  if (after.fieldAsks > 0) {
    reasons.push(`${String(after.fieldAsks)} surface_field asks after the loss`);
  }
  if (after.fieldInits > 0) {
    reasons.push(`${String(after.fieldInits)} field-init posts after the loss`);
  }
  if (after.fieldChunks !== after.blocksSent * workers) {
    reasons.push(
      `${String(after.fieldChunks)} field-chunk posts after the loss, where the ${String(after.blocksSent)} blocks then sent to ${String(workers)} workers make ${String(after.blocksSent * workers)}`,
    );
  }
  if (masks.before === null || masks.afterRestore === null || masks.end === null) {
    reasons.push("a mask was not read back");
  } else {
    if (!masks.beforeKept) {
      reasons.push("the mask after the restore lost a cell it held before the loss");
    }
    if (!masks.endAsExpected) {
      reasons.push(
        `the mask holds ${String(masks.end)} cells at the end, not the coverage's ${String(masks.expected)}`,
      );
    }
  }
  return finding(reasons);
}

function teardownFinding(record: TerrainCheckRecord["teardown"]): TerrainFinding {
  const reasons: string[] = [];
  if (!record.disposed) {
    reasons.push("the session was not disposed");
  }
  if (record.workersLive > 0) {
    reasons.push(`${String(record.workersLive)} height workers left`);
  }
  if (record.sessionGpuBytes > 0) {
    reasons.push(`${String(record.sessionGpuBytes)} B of the session's textures left`);
  }
  for (const entry of record.liveBytes.filter((each) => each.bytes > 0)) {
    reasons.push(`${String(entry.bytes)} B of the terrain's ${entry.category} left`);
  }
  return finding(reasons);
}

/** The results of a run, judged. */
export function buildTerrainCheckResults(input: TerrainCheckInput): TerrainCheckResults {
  const { run, record } = input;
  const phases = record.phases.map((phase) => phaseFigures(phase, run.setting));
  const lossPhase = record.phases.find((phase) => phase.name === "terrain-loss");
  const faults = terrainFaultsVerdict({
    faults: record.faults,
    restoresAtMs: record.restoresAtMs,
    lossWindow:
      lossPhase === undefined ? null : { startMs: lossPhase.startMs, endMs: lossPhase.endMs },
    destroyedAtMs: record.loss.destroyedAtMs,
    gpuProcessExits: input.gpuProcessExits,
  });
  const findings: TerrainCheckResults["findings"] = {
    phases: phasesFinding(phases, run.setting),
    framesDrawn: framesDrawnFinding(phases),
    framesFinite: framesFiniteFinding(phases),
    terrainDrawn: terrainDrawnFinding(phases),
    faults,
    loss: { ...lossFinding(record.loss), record: record.loss },
    teardown: { ...teardownFinding(record.teardown), record: record.teardown },
    bothSurfaces: BOTH_SURFACES_HELD_BY,
    handover: {
      rangeM: record.handover?.rangeM ?? null,
      expectedScriptS: record.handover?.scriptS ?? null,
      observedScriptS:
        phases.find((phase) => phase.name === "terrain-alone")?.views[0]?.firstHandoverScriptS ??
        null,
    },
    consoleErrors: { count: input.consoleErrors.length, first: input.consoleErrors.slice(0, 5) },
  };
  const verdicts = [
    findings.phases,
    findings.framesDrawn,
    findings.framesFinite,
    findings.terrainDrawn,
    findings.faults,
    findings.loss,
    findings.teardown,
  ].map((each) => each.verdict);
  const quiet = quietOf(run.platform, run.machine.loadAverage);
  const load = run.machine.loadAverage[0];
  const provisional = [
    keepsLoadAverage(run.platform)
      ? `load average ${load.toFixed(2)} at the start`
      : QUIET_RULE_UNCHECKED,
    ...(quiet.provisional ? ["a shared machine"] : []),
    ...(run.shown ? [] : ["a hidden run: no presentation"]),
    ...(run.smoke ? ["a smoke run: short windows"] : []),
  ].join("; ");
  return {
    schema: TERRAIN_CHECK_SCHEMA,
    version: TERRAIN_CHECK_VERSION,
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
      captures: run.captures,
      provisional: `provisional: ${provisional}`,
    },
    phases,
    findings,
    overall: verdicts.every((verdict) => verdict === "pass") ? "pass" : "fail",
  };
}

/** The reasons a run's results failed, as one line; empty for a pass. */
export function terrainFailures(results: TerrainCheckResults): string {
  const { findings } = results;
  return [
    findings.phases,
    findings.framesDrawn,
    findings.framesFinite,
    findings.terrainDrawn,
    findings.faults,
    findings.loss,
    findings.teardown,
  ]
    .flatMap((each) => each.reasons)
    .join("; ");
}

/** A figure's text, or its reason in brackets. */
function textOf<T>(figure: Measured<T>, text: (value: T) => string): string {
  return figure.value === null ? `— (${figure.reason})` : text(figure.value);
}

function ms(value: number): string {
  return value >= 10 ? value.toFixed(1) : value.toFixed(2);
}

function durations(stats: DurationStats | null): string {
  return stats === null ? "—" : `${ms(stats.p50Ms)} / ${ms(stats.p95Ms)} / ${ms(stats.maxMs)}`;
}

function mib(bytes: number): string {
  return `${(bytes / 2 ** 20).toFixed(2)} MiB`;
}

function bytesText(entries: ReadonlyArray<TerrainCheckBytes>): string {
  const held = entries.filter((each) => each.bytes > 0);
  return held.length === 0
    ? "none"
    : held.map((each) => `${each.category} ${mib(each.bytes)}`).join(", ");
}

function reasonsText(entry: TerrainFinding): string {
  return entry.reasons.length === 0 ? "" : ` — ${entry.reasons.join("; ")}`;
}

/** The results' summary, for a person reading the record. */
export function terrainCheckMarkdown(results: TerrainCheckResults): string {
  const { run, phases, findings } = results;
  const loss = findings.loss.record;
  const teardown = findings.teardown.record;
  const handover = findings.handover;
  return [
    `# Terrain run: ${run.machine.name}, ${run.setting}, ${run.startedAt.slice(0, 10)}`,
    "",
    `- **Overall:** ${results.overall}`,
    `- **Machine:** ${run.machine.cpu}, ${String(run.machine.logicalCores)} threads; GPU ${textOf(run.machine.gpu, (gpu) => gpu.description ?? `${String(gpu.vendorId)}:${String(gpu.deviceId)}`)}; governor ${textOf(run.machine.governor, (governor) => governor)}; load average ${keepsLoadAverage(run.platform) ? run.machine.loadAverage.map((value) => value.toFixed(2)).join(", ") : "none"} (${run.provisional})`,
    `- **Versions:** app ${run.versions.app}, Electron ${run.versions.electron}, Chromium ${run.versions.chromium}`,
    `- **Launch:** ${run.platform}, ${run.launchMode} mode, timer ${run.timer}, setting ${run.setting}, window ${run.shown ? "shown" : "hidden"} ${String(run.window.widthDip)} × ${String(run.window.heightDip)} DIP at a device-pixel ratio of ${String(run.devicePixelRatio)}`,
    `- **Captures (not committed):** ${textOf(run.captures, (dir) => `\`${dir}\``)}`,
    "",
    "## The hidden run's checks",
    "",
    "| Check | Verdict | Reading |",
    "| --- | --- | --- |",
    `| The setting's phases ran and held their views | ${findings.phases.verdict} | ${phases.map((phase) => `${phase.name}${phase.held ? "" : " (NOT held)"}`).join(", ")}${reasonsText(findings.phases)} |`,
    `| Every frame drawn (no frame a view was due missed, outside the restore) | ${findings.framesDrawn.verdict} | missed frames, and the longest time between draws (provisional): ${phases.map((phase) => `${phase.name} ${phase.views.map((view) => `${view.slot} ${String(view.missedFrames)}, ${view.longestGapMs.toFixed(0)} ms`).join(", ")}`).join("; ")}${reasonsText(findings.framesDrawn)} |`,
    `| Every frame finite (the photorealistic scene targets read back) | ${findings.framesFinite.verdict} | ${String(findings.framesFinite.readBacks)} read-backs${reasonsText(findings.framesFinite)} |`,
    `| The terrain drawn once handed over | ${findings.terrainDrawn.verdict} | ${phases.map((phase) => `${phase.name} ${phase.views.map((view) => `${view.slot} ${String(view.terrainDraws)} of ${String(view.handedOverDraws)} handed-over draws with a patch in view (${String(view.offSurveyDraws)} with none)${view.terrainDelayMs === null ? "" : `, first ${view.terrainDelayMs.toFixed(0)} ms after the hand-over`}`).join(", ")}`).join("; ")}${reasonsText(findings.terrainDrawn)} |`,
    `| One forced loss, then one restore, and no other fault | ${findings.faults.verdict} | ${findings.faults.expected === null ? "no forced loss excluded" : `excluded: ${findings.faults.expected.line}`}; ${String(findings.faults.restores)} restores; ${String(findings.faults.others.length)} other faults; ${String(findings.faults.gpuProcessExits)} GPU-process exits${reasonsText(findings.faults)} |`,
    `| The loss mid-stream, and after it no field asked again | ${findings.loss.verdict} | ${loss.destroyedAtScriptS === null ? "not destroyed" : `destroyed at ${loss.destroyedAtScriptS.toFixed(1)} s of the script, ${String(loss.blocksToCome)} blocks to come, ${String(loss.bakesInFlight)} bakes in flight`}; after: ${String(loss.after.fieldAsks)} asks, ${String(loss.after.fieldInits)} inits, ${String(loss.after.fieldChunks)} chunks for ${String(loss.after.blocksSent)} blocks to ${String(loss.workers)} workers; the mask ${String(loss.masks.before ?? "—")} cells before, ${String(loss.masks.afterRestore ?? "—")} after the restore, ${String(loss.masks.end ?? "—")} at the end of ${String(loss.masks.expected)}${reasonsText(findings.loss)} |`,
    `| The session torn down | ${findings.teardown.verdict} | ${teardown.disposed ? "disposed" : "NOT disposed"}; ${String(teardown.workersLive)} of ${String(teardown.workersMade)} workers left; the session's textures ${String(teardown.sessionGpuBytes)} B; the terrain's GPU bytes left: ${bytesText(teardown.liveBytes)}${reasonsText(findings.teardown)} |`,
    `| No pixel drawn by both surfaces on the datum | — | ${findings.bothSurfaces} |`,
    "",
    `The hand-over: ${handover.rangeM === null ? "no range" : `range ${(handover.rangeM / 1000).toFixed(0)} km, crossed at ${handover.expectedScriptS?.toFixed(1) ?? "—"} s of the script`}; the primary handed over at ${handover.observedScriptS?.toFixed(1) ?? "—"} s. Console errors: ${String(findings.consoleErrors.count)}${findings.consoleErrors.first.length === 0 ? "" : ` (${findings.consoleErrors.first.join(" | ")})`}.`,
    "",
    "## Phases (provisional)",
    "",
    "Durations as p50 / p95 / max ms.",
    "",
    "| Phase | Held | Window (s) | Script (s) | Animation frames | Main thread | GPU frame | Patches drawn (mean, max) | Bakes (a second) | Field posts (init, chunk) | Asks, blocks |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ...phases.map(
      (phase) =>
        `| ${phase.name} | ${phase.held ? "yes" : "NO"} | ${phase.windowS.toFixed(1)} | ${phase.script.startS.toFixed(1)} → ${phase.script.endS.toFixed(1)} | ${durations(phase.animationFrames)} | ${durations(phase.mainThreadMs)} | ${durations(phase.frameGpuMs)} | ${phase.patchesDrawn === null ? "—" : `${phase.patchesDrawn.mean.toFixed(1)}, ${String(phase.patchesDrawn.max)}`} | ${String(phase.bakes.baked)} (${phase.bakedPerS.toFixed(1)}) | ${String(phase.fieldPosts.inits)}, ${String(phase.fieldPosts.chunks)} | ${String(phase.fieldAsks)}, ${String(phase.blocksSent)} |`,
    ),
    "",
    "## Views (provisional)",
    "",
    "| Phase | View | Style | Canvas (px) | Draws | GPU (ms) | Submit (ms) | Hand-over (script s) | Terrain draws | Read-backs |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ...phases.flatMap((phase) =>
      phase.views.map(
        (view) =>
          `| ${phase.name} | ${view.slot} | ${view.style} | ${String(view.canvas.widthPx)} × ${String(view.canvas.heightPx)} | ${String(view.draws)} | ${durations(view.gpuMs)} | ${durations(view.submitMs)} | ${view.firstHandoverScriptS?.toFixed(1) ?? "—"} | ${String(view.terrainDraws)} of ${String(view.handedOverDraws)} | ${String(view.readBacks)} |`,
      ),
    ),
    "",
    "## The session's memory at each phase's end (provisional)",
    "",
    "| Phase | Workers' fields | Kept blocks | Session's textures | Terrain's GPU bytes alive | Their peak |",
    "| --- | --- | --- | --- | --- | --- |",
    ...phases.map((phase) =>
      phase.memory === null
        ? `| ${phase.name} | — | — | — | — | — |`
        : `| ${phase.name} | ${mib(phase.memory.workerFieldBytes)} | ${mib(phase.memory.keptFieldBytes)} | ${bytesText(phase.memory.sessionGpuBytes)} | ${bytesText(phase.memory.liveBytes)} | ${bytesText(phase.memory.peakBytes)} |`,
    ),
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
 * Writes the results and their summary into `dir` as `<date>-<machine>-<setting>-terrain.json` and
 * `.md` (`-smoke` before the extension for a smoke run), adding `-2`, `-3` and so on where a run of
 * the same name is there, and warns of a file over the repository's limit for added files.
 *
 * @throws Error if the day's names are used up.
 */
export async function writeTerrainResults(
  dir: string,
  results: TerrainCheckResults,
  files: ResultsFiles = NODE_FILES,
): Promise<{ readonly json: string; readonly markdown: string }> {
  await files.mkdir(dir);
  const stem = `${results.run.startedAt.slice(0, 10)}-${results.run.machine.name}-${results.run.setting}-terrain${results.run.smoke ? "-smoke" : ""}`;
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
  await files.writeFile(markdown, terrainCheckMarkdown(results));
  const bytes = Buffer.byteLength(formatAsPrettier(text), "utf8");
  if (bytes > ADDED_FILE_LIMIT_BYTES) {
    console.warn(
      `views check: ${json} is ${String(bytes)} B once formatted, over the ${String(ADDED_FILE_LIMIT_BYTES)} B the repository accepts for an added file`,
    );
  }
  return { json, markdown };
}
