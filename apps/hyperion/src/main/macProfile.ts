/**
 * The owner's Mac batch (plans R14, R08 and R10's pending Mac runs; `scripts/macProfile.mjs`): the
 * matrix of inspection profiles it runs, what it records of each, and its summary.
 *
 * @remarks
 * Every pending Mac call (the levers' A/Bs, a Retina display's ratio of 2, the height-worker sweep)
 * waits on one run of the owner's on a quiet machine. The script runs `views-check --inspection
 * --timing --hidden --hyperion-gpu-timing` over the matrix ({@link macProfileMatrix}): each setting
 * at device-pixel ratios 1 and 2, then each lever pair ({@link MAC_PROFILE_LEVERS}) at the high
 * setting as A B A B, so that a drift of the machine's heat or load falls on both sides alike; then
 * the sweep. It keeps each run's files as the run ends, so that a stopped batch resumes at its first
 * run not kept, and samples the 1-minute load, the server's CPU and macOS's thermal state before
 * and after each run ({@link MacProfileSample}). After the pairs come the counted runs
 * (`--count-gpu-calls`; R14.T2.e), each setting's own way and each side of the pairs that split a
 * pass (occlusion's and shading's, Design note 7) at DPR 1, apart from the timed pairs so that the
 * count costs them nothing. This module is the batch's arithmetic and its summary
 * ({@link macProfileSummaryLines}): each window's interval median and p95, its GPU time by pass and
 * each body's atmosphere read, each lever's delta against its pair beside its sides' reads and, for
 * a pair that splits a pass, its counted sides' attachments' bytes, and the load.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";

import {
  atmosphereReadsText,
  attachmentText,
  INSPECTION_TIMING_SCHEMA,
  type InspectionTimingResults,
  type InspectionTimingWindowResults,
  type PassFigures,
} from "./inspectionTiming";

/** A setting the batch profiles. */
export type MacProfileSetting = "high" | "low";

/** A device-pixel ratio the batch draws at: an ordinary display's 1, or a Retina display's 2. */
export type MacProfileRatio = 1 | 2;

/** One lever pair: the views check's arguments for its A runs and for its B runs. */
export interface MacProfileLever {
  /** Its name, in run IDs and `--levers`. */
  readonly name: string;
  /** The task it decides and what its sides draw, as the summary heads the pair. */
  readonly decides: string;
  readonly a: ReadonlyArray<string>;
  readonly b: ReadonlyArray<string>;
  /** The ratios its pair runs at. */
  readonly ratios: ReadonlyArray<MacProfileRatio>;
  /**
   * Whether each side takes a counted run too (R14.T2.e): a lever that moves the passes'
   * attachments, which a tile-based GPU pays for in memory traffic.
   */
  readonly countSides: boolean;
}

/**
 * The lever pairs, each A the setting's own way unless it names one and B the lever, with the
 * switches `viewsCheck.sh` takes. Shading names both sides (main's ask, 2026-11-05): forward is
 * R14.T2.a's, against which T2.e's acceptance reads the visibility buffer at both ratios.
 */
export const MAC_PROFILE_LEVERS: ReadonlyArray<MacProfileLever> = [
  {
    name: "limb-rays",
    decides: "R08.T8.d, the far limb's node rays: packed (A) against per texel (B)",
    a: [],
    b: ["--limb-rays", "per-texel"],
    ratios: [1],
    countSides: false,
  },
  {
    name: "march-sources",
    decides: "R08.T8.e, a march step's sources: from the table (A) against per step (B)",
    a: [],
    b: ["--march-sources", "per-step"],
    ratios: [1],
    countSides: false,
  },
  {
    name: "lit-per-fragment",
    decides: "R08.T9.b, the lit fragment's sources: from the table (A) against per fragment (B)",
    a: [],
    b: ["--lit-probe", "per-fragment"],
    ratios: [1],
    countSides: false,
  },
  {
    name: "composite-per-pixel",
    decides: "the composite's transmittance: as the setting forms it (A) against per pixel (B)",
    a: [],
    b: ["--composite-probe", "per-pixel"],
    ratios: [1],
    countSides: false,
  },
  {
    name: "occlusion",
    decides: "R14.T3.d, occlusion: off (A) against the depth pyramid's (B)",
    a: [],
    b: ["--geometry", "occlusion=hzb"],
    ratios: [1],
    countSides: true,
  },
  {
    name: "strides",
    decides: "R14.T3.c, strides: measured (A) against off (B)",
    a: [],
    b: ["--geometry", "strides=off"],
    ratios: [1],
    countSides: false,
  },
  {
    name: "far-cascades",
    decides: "R14.T6.b, the far cascades: every fourth frame (A) against every frame (B)",
    a: [],
    b: ["--geometry", "farCascadeRate=1"],
    ratios: [1],
    countSides: false,
  },
  {
    name: "shading",
    decides:
      "R14.T2.e, the terrain's shading: forward, T2.a's (A), against the visibility buffer (B)",
    a: ["--geometry", "shading=forward"],
    b: ["--geometry", "shading=visibility"],
    ratios: [1, 2],
    countSides: true,
  },
];

/** The lever named `name`, or `undefined`. */
export function leverNamed(name: string): MacProfileLever | undefined {
  return MAC_PROFILE_LEVERS.find((lever) => lever.name === name);
}

/** A run's place in a lever pair. */
export interface MacProfilePairSide {
  readonly lever: string;
  readonly side: "a" | "b";
  /** Which of the pair's A B rounds, from 1. */
  readonly round: number;
}

/**
 * A run's place in the matrix: a setting's own run, a side of a lever pair, or a counted run
 * (`--count-gpu-calls`; R14.T2.e) of a setting's own way or of one side of a pair whose sides are
 * counted.
 */
export type MacProfilePlace =
  | { readonly kind: "own" }
  | ({ readonly kind: "pair" } & MacProfilePairSide)
  | { readonly kind: "counted" }
  | { readonly kind: "counted side"; readonly lever: string; readonly side: "a" | "b" };

/** Whether a run is a counted run, either kind. */
export function isCounted(run: MacProfileRun): boolean {
  return run.place.kind === "counted" || run.place.kind === "counted side";
}

/** The views check's switch a counted run adds: its WebGPU calls and attachments counted. */
export const COUNT_GPU_CALLS = "--count-gpu-calls";

/** The ratio the counted runs draw at: a screen-sized target's bytes scale with its pixels. */
export const COUNTED_RATIO: MacProfileRatio = 1;

/** One launch of the matrix. */
export interface MacProfileRun {
  /** Its name, its directory's under the batch's `runs/`. */
  readonly id: string;
  readonly setting: MacProfileSetting;
  readonly ratio: MacProfileRatio;
  /** Its own arguments to the views check, after the batch's. */
  readonly args: ReadonlyArray<string>;
  readonly place: MacProfilePlace;
}

/** What the matrix is made of. */
export interface MacProfileMatrixOptions {
  /** The settings of the settings' own runs, each at every ratio of {@link ratios}. */
  readonly settings: ReadonlyArray<MacProfileSetting>;
  readonly ratios: ReadonlyArray<MacProfileRatio>;
  /** The lever pairs, by name, in {@link MAC_PROFILE_LEVERS}' order whatever the order given. */
  readonly levers: ReadonlyArray<string>;
  /** The setting the pairs run at. */
  readonly pairSetting: MacProfileSetting;
  /** Each pair's A B rounds: 2 is A B A B. */
  readonly rounds: number;
  /**
   * Whether the counted runs are made: each setting's own way, then each side of every pair of
   * {@link levers} whose sides are counted, at the pair setting.
   */
  readonly counted: boolean;
}

/** The owner's batch: both settings at both ratios, every pair at high as A B A B. */
export const MAC_PROFILE_DEFAULTS: MacProfileMatrixOptions = {
  settings: ["high", "low"],
  ratios: [1, 2],
  levers: MAC_PROFILE_LEVERS.map((lever) => lever.name),
  pairSetting: "high",
  rounds: 2,
  counted: true,
};

/** The views check's argument for a ratio: none at 1, which is its default. */
function ratioArgs(ratio: MacProfileRatio): ReadonlyArray<string> {
  return ratio === 1 ? [] : ["--device-pixel-ratio", String(ratio)];
}

/**
 * The batch's runs, in the order they run: each setting's own runs, then each pair's, A and B by
 * turns, its ratios one after the other, then the counted runs at {@link COUNTED_RATIO}.
 *
 * @throws Error for a lever not in {@link MAC_PROFILE_LEVERS}, which the script refuses first.
 */
export function macProfileMatrix(options: MacProfileMatrixOptions): ReadonlyArray<MacProfileRun> {
  const unknown = options.levers.filter((name) => leverNamed(name) === undefined);
  if (unknown.length > 0) {
    throw new Error(`no lever is named ${unknown.join(", ")}`);
  }
  const own = options.settings.flatMap((setting) =>
    options.ratios.map((ratio): MacProfileRun => ({
      id: `${setting}-dpr${String(ratio)}`,
      setting,
      ratio,
      args: ratioArgs(ratio),
      place: { kind: "own" },
    })),
  );
  const setting = options.pairSetting;
  const pairs = MAC_PROFILE_LEVERS.filter((lever) => options.levers.includes(lever.name)).flatMap(
    (lever) =>
      lever.ratios.flatMap((ratio) =>
        Array.from({ length: options.rounds }, (_, i) => i + 1).flatMap((round) =>
          (["a", "b"] as const).map((side): MacProfileRun => ({
            id: `${lever.name}-${setting}-dpr${String(ratio)}-${side}${String(round)}`,
            setting,
            ratio,
            args: [...ratioArgs(ratio), ...(side === "a" ? lever.a : lever.b)],
            place: { kind: "pair", lever: lever.name, side, round },
          })),
        ),
      ),
  );
  if (!options.counted) {
    return [...own, ...pairs];
  }
  const ratio = String(COUNTED_RATIO);
  const counted = [
    ...options.settings.map((each): MacProfileRun => ({
      id: `counted-${each}-dpr${ratio}`,
      setting: each,
      ratio: COUNTED_RATIO,
      args: [...ratioArgs(COUNTED_RATIO), COUNT_GPU_CALLS],
      place: { kind: "counted" },
    })),
    ...MAC_PROFILE_LEVERS.filter(
      (lever) => lever.countSides && options.levers.includes(lever.name),
    ).flatMap((lever) =>
      (["a", "b"] as const).map((side): MacProfileRun => ({
        id: `counted-${lever.name}-${setting}-dpr${ratio}-${side}`,
        setting,
        ratio: COUNTED_RATIO,
        args: [...ratioArgs(COUNTED_RATIO), COUNT_GPU_CALLS, ...(side === "a" ? lever.a : lever.b)],
        place: { kind: "counted side", lever: lever.name, side },
      })),
    ),
  ];
  return [...own, ...pairs, ...counted];
}

/** The machine as a run starts or ends. */
export interface MacProfileSample {
  /** When, as an ISO 8601 time. */
  readonly at: string;
  /** The 1-minute load average. */
  readonly load1: number;
  /**
   * The server's `%cpu` as `ps` gives it (on macOS a decaying average of its recent use, on Linux
   * its use over its life), or `null` with no server running.
   */
  readonly serverCpuPercent: number | null;
  /** The server's CPU time so far, s, from `ps`'s `time`, or `null` with no server running. */
  readonly serverCpuS: number | null;
  /** `pmset -g therm`'s report, or `null` off macOS. */
  readonly thermal: string | null;
}

/** The server's skies over a run, counted from its log ({@link skyEventOf}). */
export interface SkyTally {
  readonly started: number;
  /** Answered with a kept final reply, computing nothing. */
  readonly kept: number;
  /** Computed to their final reply. */
  readonly final: number;
  /** Ended without their final reply: failed, cancelled or given up. */
  readonly unfinished: number;
}

/** The schema name of a run's record, `run.json` in its directory. */
export const MAC_PROFILE_RUN_SCHEMA = "hyperion.mac-profile.run";

/** A kept run's record, written last as the run's directory is kept. */
export interface MacProfileRunRecord {
  readonly schema: typeof MAC_PROFILE_RUN_SCHEMA;
  readonly version: 1;
  readonly id: string;
  /** The views check's whole command line after `viewsCheck.sh`. */
  readonly argv: ReadonlyArray<string>;
  /** The views check's exit: a run is kept with its timing file whatever it is. */
  readonly exit: number;
  /** The views check's `FAIL` line, without its prefix, where it exited otherwise than 0. */
  readonly failure: string | null;
  readonly before: MacProfileSample;
  readonly after: MacProfileSample;
  readonly skies: SkyTally;
  /** The timing file's name in the run's directory. */
  readonly timing: string;
  /** Every file the run's directory holds, its timing file, summary and captures. */
  readonly files: ReadonlyArray<string>;
}

/** A sky's line in the server's log, by what it says of the sky (`requests/sky.rs`' `info` lines). */
export type SkyEvent = "started" | "kept" | "final" | "unfinished";

/** The messages of the server's sky lines, by event. */
const SKY_MESSAGES: ReadonlyArray<readonly [string, SkyEvent]> = [
  ["a sky started", "started"],
  ["a sky is answered with its kept final reply", "kept"],
  ["a sky's final reply is made", "final"],
  ["a sky ended without its final reply", "unfinished"],
  ["a sky was given up before its final reply", "unfinished"],
];

/**
 * What one line of the server's log says of a sky, or `null` for a line about none: found by its
 * message alone, which a terminal's colours, where the server writes them, leave whole.
 */
export function skyEventOf(line: string): SkyEvent | null {
  const found = SKY_MESSAGES.find(([message]) => line.includes(message));
  return found === undefined ? null : found[1];
}

/** `tally` with one more `event`. */
export function tallied(tally: SkyTally, event: SkyEvent): SkyTally {
  return { ...tally, [event]: tally[event] + 1 };
}

/** No skies. */
export const NO_SKIES: SkyTally = { started: 0, kept: 0, final: 0, unfinished: 0 };

/** The skies started and not yet ended. */
export function skiesInFlight(tally: SkyTally): number {
  return tally.started - tally.kept - tally.final - tally.unfinished;
}

/** `later`'s skies less `earlier`'s: those of the span between them. */
export function skiesBetween(earlier: SkyTally, later: SkyTally): SkyTally {
  return {
    started: later.started - earlier.started,
    kept: later.kept - earlier.kept,
    final: later.final - earlier.final,
    unfinished: later.unfinished - earlier.unfinished,
  };
}

/**
 * `ps`'s `time` of a process, s: macOS's `M:SS.ss`, procps' `[DD-]HH:MM:SS`, or `null` for a
 * text of neither form.
 */
export function cpuTimeSeconds(text: string): number | null {
  const match = /^(?:(\d+)-)?((?:\d+:)*\d+(?:\.\d+)?)$/u.exec(text.trim());
  if (match === null) {
    return null;
  }
  const days = match[1] === undefined ? 0 : Number(match[1]);
  const parts = (match[2] ?? "").split(":").map(Number);
  return parts.reduce((total, part) => total * 60 + part, 0) + days * 86_400;
}

/**
 * The server's mean CPU over a run, in cores (1 is one core busy throughout), from its CPU time at
 * the run's start and end; `null` without both.
 */
export function serverCoresOver(before: MacProfileSample, after: MacProfileSample): number | null {
  const wallS = (Date.parse(after.at) - Date.parse(before.at)) / 1000;
  if (before.serverCpuS === null || after.serverCpuS === null || !(wallS > 0)) {
    return null;
  }
  return (after.serverCpuS - before.serverCpuS) / wallS;
}

/**
 * A thermal report in a few words: `nominal` where `pmset -g therm` recorded no warning, no
 * performance warning and no CPU power status, its lines that say otherwise joined, or `—` off
 * macOS.
 */
export function thermalText(report: string | null): string {
  if (report === null) {
    return "—";
  }
  const lines = report
    .split("\n")
    .map((line) => line.replaceAll(/\s+/gu, " ").trim())
    .filter((line) => line.length > 0 && !/^Note: No .* has been recorded$/u.test(line));
  return lines.length === 0 ? "nominal" : lines.join("; ");
}

/** The middle of `values`, the mean of the two middles for an even count, or `null` for none. */
export function median(values: ReadonlyArray<number>): number | null {
  const sorted = values.toSorted((a, b) => a - b);
  const half = Math.floor(sorted.length / 2);
  const upper = sorted[half];
  if (upper === undefined) {
    return null;
  }
  return sorted.length % 2 === 1 ? upper : ((sorted[half - 1] ?? upper) + upper) / 2;
}

/** What the summary reads of a window of a timing file. */
export interface ProfiledWindow {
  readonly name: InspectionTimingWindowResults["name"];
  readonly frame: Pick<InspectionTimingWindowResults["frame"], "intervalMs" | "gpuMs">;
  readonly passes: ReadonlyArray<Pick<PassFigures, "label" | "medianMs" | "shareOfGpu">>;
  /** Its attachments counted, `null` uncounted; absent before the file's version 7, as none. */
  readonly attachments?: InspectionTimingWindowResults["attachments"];
  /** Each body's atmosphere read; absent before the file's version 7, as none known. */
  readonly atmosphereReads?: InspectionTimingWindowResults["atmosphereReads"];
}

/**
 * A window's atmosphere reads by body without their frames, so that runs of one side drawn alike
 * read alike: `—` for none and `not recorded` before version 7.
 */
function readKindsText(window: ProfiledWindow): string {
  if (window.atmosphereReads === undefined) {
    return "not recorded";
  }
  return window.atmosphereReads.length === 0
    ? "—"
    : window.atmosphereReads
        .map((each) => `air ${String(each.air)}: ${each.read}${each.thermal ? " (thermal)" : ""}`)
        .join("; ");
}

/** A window's atmosphere reads in words, `—` for none and `not recorded` before version 7. */
function readsText(window: ProfiledWindow): string {
  return window.atmosphereReads === undefined
    ? "not recorded"
    : atmosphereReadsText(window.atmosphereReads);
}

/** What the summary reads of a timing file. */
export interface ProfiledTiming {
  readonly run: Pick<InspectionTimingResults["run"], "timer">;
  readonly windows: ReadonlyArray<ProfiledWindow>;
}

/** A kept run: its place in the matrix, its record and its timing file. */
export interface KeptRun {
  readonly run: MacProfileRun;
  readonly record: MacProfileRunRecord;
  readonly timing: ProfiledTiming;
}

/** One side of a pair over a window: each figure the median of its runs' own. */
export interface PairSideFigures {
  readonly runs: number;
  /** Its runs' atmosphere reads by body, without their frames, each distinct one once. */
  readonly reads: ReadonlyArray<string>;
  readonly intervalMedianMs: number | null;
  readonly intervalP95Ms: number | null;
  readonly gpuMedianMs: number | null;
  /** Each pass label's median, by label. */
  readonly passMs: ReadonlyMap<string, number>;
}

/** A pair over one window: its sides and B less A. */
export interface PairWindowFigures {
  readonly window: InspectionTimingWindowResults["name"];
  readonly a: PairSideFigures;
  readonly b: PairSideFigures;
  /** B's interval median less A's, ms. */
  readonly deltaMs: number | null;
  /** {@link deltaMs} as a share of A's interval median. */
  readonly deltaShare: number | null;
  /** B's GPU median less A's, ms. */
  readonly gpuDeltaMs: number | null;
}

/** The windows of a timing file, in the run's order. */
const WINDOW_ORDER: ReadonlyArray<InspectionTimingWindowResults["name"]> = [
  "orbit",
  "close",
  "low",
];

/** One side's figures over `window`, from its runs that measured it. */
function sideFigures(
  runs: ReadonlyArray<KeptRun>,
  window: InspectionTimingWindowResults["name"],
): PairSideFigures {
  const windows = runs.flatMap((kept) =>
    kept.timing.windows.filter((each) => each.name === window),
  );
  const of = (figure: (each: ProfiledWindow) => number | null): number | null =>
    median(windows.flatMap((each) => figure(each) ?? []));
  const labels = [...new Set(windows.flatMap((each) => each.passes.map((pass) => pass.label)))];
  const passMs = new Map<string, number>();
  for (const label of labels) {
    const ms = median(
      windows.flatMap((each) => each.passes.find((pass) => pass.label === label)?.medianMs ?? []),
    );
    if (ms !== null) {
      passMs.set(label, ms);
    }
  }
  return {
    runs: windows.length,
    reads: [...new Set(windows.map(readKindsText))],
    intervalMedianMs: of((each) => each.frame.intervalMs.medianMs),
    intervalP95Ms: of((each) => each.frame.intervalMs.p95Ms),
    gpuMedianMs: of((each) => each.frame.gpuMs.medianMs),
    passMs,
  };
}

/** `b` less `a`, or `null` without both. */
function less(b: number | null, a: number | null): number | null {
  return a === null || b === null ? null : b - a;
}

/**
 * A lever's pair at one ratio over each window either side measured: each side's figures the
 * median of its kept runs' (the mean of two for A B A B), and B less A.
 */
export function pairFigures(
  kept: ReadonlyArray<KeptRun>,
  lever: string,
  ratio: MacProfileRatio,
): ReadonlyArray<PairWindowFigures> {
  const ofSide = (side: "a" | "b"): ReadonlyArray<KeptRun> =>
    kept.filter(
      (each) =>
        each.run.place.kind === "pair" &&
        each.run.place.lever === lever &&
        each.run.place.side === side &&
        each.run.ratio === ratio,
    );
  const aRuns = ofSide("a");
  const bRuns = ofSide("b");
  return WINDOW_ORDER.flatMap((window): PairWindowFigures[] => {
    const a = sideFigures(aRuns, window);
    const b = sideFigures(bRuns, window);
    if (a.runs === 0 && b.runs === 0) {
      return [];
    }
    const deltaMs = less(b.intervalMedianMs, a.intervalMedianMs);
    return [
      {
        window,
        a,
        b,
        deltaMs,
        deltaShare:
          deltaMs === null || a.intervalMedianMs === null || !(a.intervalMedianMs > 0)
            ? null
            : deltaMs / a.intervalMedianMs,
        gpuDeltaMs: less(b.gpuMedianMs, a.gpuMedianMs),
      },
    ];
  });
}

/** A time, ms, to `digits` decimals, or `—`. */
function msText(value: number | null, digits = 1): string {
  return value === null ? "—" : value.toFixed(digits);
}

/** A signed time, ms, or `—`. */
function signedMs(value: number | null): string {
  return value === null ? "—" : `${value >= 0 ? "+" : "−"}${Math.abs(value).toFixed(1)}`;
}

/** Frames a second at an interval, ms, or `—`. */
function fps(intervalMs: number | null): string {
  return intervalMs === null || !(intervalMs > 0) ? "—" : (1000 / intervalMs).toFixed(1);
}

/** A share as a signed percentage, or `—`. */
function signedShare(share: number | null): string {
  return share === null ? "—" : `${share >= 0 ? "+" : "−"}${Math.abs(100 * share).toFixed(1)}%`;
}

/** A share as a percentage, or `—`. */
function shareText(value: number | null): string {
  return value === null ? "—" : `${(100 * value).toFixed(0)}%`;
}

/** A run's exit as the runs' table gives it: its status, and its `FAIL` line where it failed. */
function exitText(record: MacProfileRunRecord): string {
  return record.exit === 0 || record.failure === null
    ? String(record.exit)
    : `${String(record.exit)}: ${record.failure}`;
}

/** A percentage, or `—`. */
function percentText(value: number | null): string {
  return value === null ? "—" : `${value.toFixed(0)}%`;
}

/** A table cell's text, its pipes escaped. */
function cell(text: string): string {
  return text.replaceAll("|", "\\|");
}

/** The passes a window lists in the runs' table, its dearest by median. */
const LISTED_PASSES = 6;

/** The passes a pair's table lists of each window, the dearest of A and B together. */
const PAIR_PASSES = 8;

/** A run's lever, as the runs' table names it: a counted run's as counted. */
function leverText(run: MacProfileRun): string {
  const { place } = run;
  let text: string;
  switch (place.kind) {
    case "own":
      text = "—";
      break;
    case "pair":
      text = `${place.lever} ${place.side.toUpperCase()}${String(place.round)}`;
      break;
    case "counted":
      text = "counted";
      break;
    case "counted side":
      text = `counted, ${place.lever} ${place.side.toUpperCase()}`;
      break;
  }
  return text;
}

/** The runs' table: each run of the matrix, kept or not, with its load and the server's CPU. */
function runsTable(
  matrix: ReadonlyArray<MacProfileRun>,
  kept: ReadonlyMap<string, KeptRun>,
): string[] {
  return [
    "| Run | Setting | DPR | Lever | Exit | Wall | Load before → after | Server ps CPU before → after | Server over the run | Skies computed, kept | Thermal before → after |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ...matrix.map((run) => {
      const record = kept.get(run.id)?.record;
      if (record === undefined) {
        return `| ${run.id} | ${run.setting} | ${String(run.ratio)} | ${leverText(run)} | not kept | | | | | | |`;
      }
      const { before, after, skies } = record;
      const wallS = (Date.parse(after.at) - Date.parse(before.at)) / 1000;
      const cores = serverCoresOver(before, after);
      return (
        `| ${run.id} | ${run.setting} | ${String(run.ratio)} | ${leverText(run)} | ${cell(exitText(record))} | ` +
        `${wallS.toFixed(0)} s | ${before.load1.toFixed(2)} → ${after.load1.toFixed(2)} | ` +
        `${percentText(before.serverCpuPercent)} → ${percentText(after.serverCpuPercent)} | ` +
        `${cores === null ? "—" : `${(100 * cores).toFixed(0)}%`} | ` +
        `${String(skies.started - skies.kept)}, ${String(skies.kept)} | ` +
        `${cell(thermalText(before.thermal))} → ${cell(thermalText(after.thermal))} |`
      );
    }),
  ];
}

/** The windows' table: each kept run's windows, their intervals, GPU and dearest passes. */
function windowsTable(
  matrix: ReadonlyArray<MacProfileRun>,
  kept: ReadonlyMap<string, KeptRun>,
): string[] {
  return [
    "| Run | Window | Interval median | p95 | fps at the median | GPU median | Dearest passes, median ms (share of the GPU) | Atmosphere read |",
    "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ...matrix.flatMap((run) =>
      (kept.get(run.id)?.timing.windows ?? []).map((window) => {
        const passes = window.passes
          .filter((pass) => pass.medianMs !== null)
          .toSorted((a, b) => (b.medianMs ?? 0) - (a.medianMs ?? 0))
          .slice(0, LISTED_PASSES)
          .map((pass) => `${pass.label} ${msText(pass.medianMs)} (${shareText(pass.shareOfGpu)})`);
        return (
          `| ${run.id} | ${window.name} | ${msText(window.frame.intervalMs.medianMs)} | ` +
          `${msText(window.frame.intervalMs.p95Ms)} | ${fps(window.frame.intervalMs.medianMs)} | ` +
          `${msText(window.frame.gpuMs.medianMs)} | ${cell(passes.length > 0 ? passes.join(", ") : "—")} | ` +
          `${cell(readsText(window))} |`
        );
      }),
    ),
  ];
}

/** A pair's section at one ratio: its windows' figures, then its dearest passes by window. */
function pairSection(
  lever: MacProfileLever,
  setting: MacProfileSetting,
  ratio: MacProfileRatio,
  kept: ReadonlyArray<KeptRun>,
): string[] {
  const figures = pairFigures(kept, lever.name, ratio);
  const heading = `### ${lever.name}, ${setting} at DPR ${String(ratio)}: ${lever.decides}`;
  if (figures.length === 0) {
    return [heading, "", "No run of the pair is kept.", ""];
  }
  const passRows = figures.flatMap((each) => {
    const labels = [...new Set([...each.a.passMs.keys(), ...each.b.passMs.keys()])]
      .toSorted(
        (x, y) =>
          (each.a.passMs.get(y) ?? 0) +
          (each.b.passMs.get(y) ?? 0) -
          ((each.a.passMs.get(x) ?? 0) + (each.b.passMs.get(x) ?? 0)),
      )
      .slice(0, PAIR_PASSES);
    return labels.map((label) => {
      const a = each.a.passMs.get(label) ?? null;
      const b = each.b.passMs.get(label) ?? null;
      return `| ${each.window} | ${cell(label)} | ${msText(a)} | ${msText(b)} | ${signedMs(less(b, a))} |`;
    });
  });
  return [
    heading,
    "",
    "| Window | Runs A, B | A interval median | B | B − A | B − A, share | A fps | B fps | A p95 | B p95 | A GPU | B GPU | B − A GPU |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ...figures.map(
      (each) =>
        `| ${each.window} | ${String(each.a.runs)}, ${String(each.b.runs)} | ` +
        `${msText(each.a.intervalMedianMs)} | ${msText(each.b.intervalMedianMs)} | ${signedMs(each.deltaMs)} | ` +
        `${signedShare(each.deltaShare)} | ${fps(each.a.intervalMedianMs)} | ${fps(each.b.intervalMedianMs)} | ` +
        `${msText(each.a.intervalP95Ms)} | ${msText(each.b.intervalP95Ms)} | ${msText(each.a.gpuMedianMs)} | ` +
        `${msText(each.b.gpuMedianMs)} | ${signedMs(each.gpuDeltaMs)} |`,
    ),
    "",
    "| Window | Pass | A median ms | B median ms | B − A |",
    "| --- | --- | --- | --- | --- |",
    ...passRows,
    "",
    "| Window | A's atmosphere read | B's |",
    "| --- | --- | --- |",
    ...figures.map(
      (each) =>
        `| ${each.window} | ${cell(each.a.reads.length === 0 ? "—" : each.a.reads.join(" / "))} | ${cell(each.b.reads.length === 0 ? "—" : each.b.reads.join(" / "))} |`,
    ),
    "",
  ];
}

/** The passes a counted run's window lists by their attachments' bytes, the most first. */
const COUNTED_PASSES = 12;

/** Bytes as MiB, to two decimals, or `—`. */
function mibText(bytes: number | null | undefined): string {
  return bytes === null || bytes === undefined ? "—" : (bytes / 1_048_576).toFixed(2);
}

/** A counted pass's bytes a draw, loaded and stored. */
type CountedPass = NonNullable<ProfiledWindow["attachments"]>["passes"][number];

/** A counted pass's bytes a draw, loaded and stored together, 0 for none. */
function passBytes(pass: CountedPass | undefined): number {
  return (pass?.bytesLoadedPerDraw ?? 0) + (pass?.bytesStoredPerDraw ?? 0);
}

/** A counted run's window's passes by label, or none for a window counted not or before 7. */
function countedPasses(window: ProfiledWindow | undefined): ReadonlyMap<string, CountedPass> {
  return new Map((window?.attachments?.passes ?? []).map((pass) => [pass.label, pass]));
}

/**
 * A pair's counted sides' attachments by window and pass (R14.T2.e), beside its timed figures:
 * each side's bytes loaded and stored a draw of the primary where no tile is kept.
 */
function countedPairSection(lever: MacProfileLever, kept: ReadonlyArray<KeptRun>): string[] {
  const side = (which: "a" | "b"): KeptRun | undefined =>
    kept.find(
      (each) =>
        each.run.place.kind === "counted side" &&
        each.run.place.lever === lever.name &&
        each.run.place.side === which,
    );
  const a = side("a");
  const b = side("b");
  const heading = `The counted sides' attachments (\`${COUNT_GPU_CALLS}\` at DPR ${String(COUNTED_RATIO)}, apart from the timed runs), MiB a draw of the primary where a tile-based GPU keeps no tile from one pass to the next:`;
  if (a === undefined && b === undefined) {
    return [heading, "", "No counted run of the pair is kept.", ""];
  }
  const rows = WINDOW_ORDER.flatMap((name) => {
    const aPasses = countedPasses(a?.timing.windows.find((each) => each.name === name));
    const bPasses = countedPasses(b?.timing.windows.find((each) => each.name === name));
    return [...new Set([...aPasses.keys(), ...bPasses.keys()])]
      .toSorted(
        (x, y) =>
          passBytes(aPasses.get(y)) +
          passBytes(bPasses.get(y)) -
          passBytes(aPasses.get(x)) -
          passBytes(bPasses.get(x)),
      )
      .slice(0, PAIR_PASSES)
      .map((label) => {
        const ofA = aPasses.get(label);
        const ofB = bPasses.get(label);
        return `| ${name} | ${cell(label)} | ${mibText(ofA?.bytesLoadedPerDraw)} | ${mibText(ofA?.bytesStoredPerDraw)} | ${mibText(ofB?.bytesLoadedPerDraw)} | ${mibText(ofB?.bytesStoredPerDraw)} |`;
      });
  });
  return [
    heading,
    "",
    "| Window | Pass | A loaded | A stored | B loaded | B stored |",
    "| --- | --- | --- | --- | --- | --- |",
    ...(rows.length === 0 ? ["| — | no attachment counted | | | | |"] : rows),
    "",
  ];
}

/** A counted window's lines: its bytes a draw and its read, then its dearest passes' attachments. */
function countedWindowLines(window: ProfiledWindow): string[] {
  const figures = window.attachments;
  if (figures === undefined || figures === null) {
    return [`- **${window.name}:** no attachment counted.`];
  }
  const lines = [
    `- **${window.name}:** ${mibText(figures.bytesLoadedPerDraw)} MiB loaded, ${mibText(figures.bytesStoredPerDraw)} MiB stored a draw; atmosphere read ${cell(readsText(window))}`,
    "",
    "| Pass | Passes a draw | Loaded | Stored | Attachments (load/store) |",
    "| --- | --- | --- | --- | --- |",
  ];
  for (const pass of figures.passes.slice(0, COUNTED_PASSES)) {
    lines.push(
      `| ${cell(pass.label)} | ${pass.passesPerDraw.toFixed(1)} | ${mibText(pass.bytesLoadedPerDraw)} | ${mibText(pass.bytesStoredPerDraw)} | ${cell(pass.attachments.map(attachmentText).join("; "))} |`,
    );
  }
  lines.push("");
  return lines;
}

/** The counted runs' section: each window's bytes a draw, then its dearest passes' attachments. */
function countedSection(
  matrix: ReadonlyArray<MacProfileRun>,
  kept: ReadonlyMap<string, KeptRun>,
): string[] {
  const runs = matrix.filter(isCounted);
  if (runs.length === 0) {
    return [];
  }
  const lines = [
    "## The counted runs",
    "",
    `Each counted run draws with \`${COUNT_GPU_CALLS}\` at DPR ${String(COUNTED_RATIO)}, apart from the timed pairs, so its times are not a player's. Its attachments' bytes are MiB a draw of the primary where a tile-based GPU keeps no tile from one pass to the next (a load reads an attachment into tiles, a store writes it back; a clear and a discard move none).`,
    "",
  ];
  for (const run of runs) {
    lines.push(`### ${run.id} (${leverText(run)})`, "");
    const windows = kept.get(run.id)?.timing.windows;
    if (windows === undefined) {
      lines.push("Not kept.", "");
      continue;
    }
    for (const window of windows) {
      lines.push(...countedWindowLines(window));
    }
    lines.push("");
  }
  return lines;
}

/** One count of the height-worker sweep, as the batch sampled the machine around its descent. */
export interface SweepRunRecord {
  readonly workers: number;
  readonly exit: number;
  readonly before: MacProfileSample;
  readonly after: MacProfileSample;
}

/** Everything the summary is written from. */
export interface MacProfileSummaryInput {
  /** The machine, as the owner names it. */
  readonly machine: string;
  /** The tree the batch was built from, its commit. */
  readonly tree: string;
  /** The body every launch inspects. */
  readonly body: string;
  readonly options: MacProfileMatrixOptions;
  readonly kept: ReadonlyArray<KeptRun>;
  readonly sweep: {
    /** The sweep's table, `worker-sweep.md`'s lines, or `null` where it is not written yet. */
    readonly table: ReadonlyArray<string> | null;
    /** The counts flown, as sampled. */
    readonly runs: ReadonlyArray<SweepRunRecord>;
  };
}

/** The sweep's section: its table, then the machine around each count's descent. */
function sweepSection(sweep: MacProfileSummaryInput["sweep"]): string[] {
  const runs = sweep.runs.toSorted((a, b) => a.workers - b.workers);
  return [
    ...(sweep.table ?? ["Not written yet."]),
    ...(runs.length === 0
      ? []
      : [
          "",
          "| Workers | Exit | Wall | Load before → after | Thermal before → after |",
          "| --- | --- | --- | --- | --- |",
          ...runs.map(
            (each) =>
              `| ${String(each.workers)} | ${String(each.exit)} | ` +
              `${((Date.parse(each.after.at) - Date.parse(each.before.at)) / 1000).toFixed(0)} s | ` +
              `${each.before.load1.toFixed(2)} → ${each.after.load1.toFixed(2)} | ` +
              `${cell(thermalText(each.before.thermal))} → ${cell(thermalText(each.after.thermal))} |`,
          ),
        ]),
  ];
}

/**
 * The batch's summary in Markdown: each run's load and the server's CPU, each kept run's windows
 * (interval median and p95, fps at the median, GPU median, the dearest passes and the atmosphere
 * read), each lever pair's sides and B less A by window and by pass with each side's reads (and a
 * counted pair's attachments' bytes by side), the counted runs' attachments, and the worker
 * sweep's table.
 */
export function macProfileSummaryLines(input: MacProfileSummaryInput): string[] {
  const matrix = macProfileMatrix(input.options);
  const byId = new Map(input.kept.map((each) => [each.run.id, each]));
  const timers = [...new Set(input.kept.map((each) => each.timing.run.timer))];
  const pairs = MAC_PROFILE_LEVERS.filter((lever) => input.options.levers.includes(lever.name));
  return [
    `# The Mac batch: ${input.machine}`,
    "",
    `Tree ${input.tree}, body ${input.body}; ${String(byId.size)} of ${String(matrix.length)} runs kept. Each figure is ` +
      "a window's over its primary's draws: the interval between draws (median, p95 and fps at the " +
      "median) and the GPU's frame and passes (median ms; the pass timer " +
      `${timers.length === 0 ? "—" : timers.join(", ")}). A pair's side is the median of its runs' ` +
      "figures (the mean of two for A B A B). The load is the 1-minute load average; the server's " +
      "CPU over a run is its CPU time over the run's wall time, 100% a core.",
    "",
    "## The runs",
    "",
    ...runsTable(matrix, byId),
    "",
    "## Each run's windows",
    "",
    ...windowsTable(matrix, byId),
    "",
    "## The lever pairs",
    "",
    ...pairs.flatMap((lever) =>
      lever.ratios.flatMap((ratio) => [
        ...pairSection(lever, input.options.pairSetting, ratio, input.kept),
        ...(lever.countSides && ratio === COUNTED_RATIO && input.options.counted
          ? countedPairSection(lever, input.kept)
          : []),
      ]),
    ),
    ...countedSection(matrix, byId),
    "## The height-worker sweep",
    "",
    ...sweepSection(input.sweep),
  ];
}

/** Whether a value read from JSON is an object, not an array. */
function isObject(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** The JSON of the file at `path`, or `undefined` where it is not there or not JSON. */
function readJsonFile(path: string): unknown {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    // Not written: a run not kept, or a sweep not flown.
    return undefined;
  }
}

/**
 * The kept runs of `matrix` in a batch's `runs/` directory: each with its record, `run.json`, and the
 * timing file it names.
 *
 * @remarks
 * Both files are the batch's own, its record written last and the run's directory renamed into
 * place after it, so that a run is either kept whole or not at all; each is checked by its schema's
 * name and then read as it was written.
 */
export function keptRunsIn(
  runsDir: string,
  matrix: ReadonlyArray<MacProfileRun>,
): ReadonlyArray<KeptRun> {
  return matrix.flatMap((run): KeptRun[] => {
    const dir = join(runsDir, run.id);
    const record = readJsonFile(join(dir, "run.json"));
    if (!isObject(record) || record["schema"] !== MAC_PROFILE_RUN_SCHEMA) {
      return [];
    }
    const name = record["timing"];
    const timing = typeof name === "string" ? readJsonFile(join(dir, name)) : undefined;
    if (!isObject(timing) || timing["schema"] !== INSPECTION_TIMING_SCHEMA) {
      return [];
    }
    return [
      {
        run,
        // The batch's own record and the views check's own file, each by its schema's name.
        // oxlint-disable-next-line typescript/no-unsafe-type-assertion
        record: record as unknown as MacProfileRunRecord,
        // oxlint-disable-next-line typescript/no-unsafe-type-assertion
        timing: timing as unknown as ProfiledTiming,
      },
    ];
  });
}

/** The sweep's table and samples in its directory, as the summary reads them. */
export function sweepIn(sweepDir: string): MacProfileSummaryInput["sweep"] {
  let table: ReadonlyArray<string> | null = null;
  try {
    table = readFileSync(join(sweepDir, "worker-sweep.md"), "utf8").trimEnd().split("\n");
  } catch {
    // The sweep's table is written once every count is flown.
  }
  const samples = readJsonFile(join(sweepDir, "samples.json"));
  // The batch's own file, written whole by it: a list of its sweep's records.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  const runs = Array.isArray(samples) ? (samples as ReadonlyArray<SweepRunRecord>) : [];
  return { table, runs };
}

/** What a batch was started on, its `batch.json`: its commit and its body; `null` for none. */
export interface MacProfileBatch {
  readonly tree: string;
  /** The body its launches inspect, or `null` for a batch that did not record one: the default. */
  readonly body: string | null;
}

/** The batch started in `out`, or `null` for a batch not started there. */
export function batchIn(out: string): MacProfileBatch | null {
  const batch = readJsonFile(join(out, "batch.json"));
  if (!isObject(batch) || typeof batch["tree"] !== "string") {
    return null;
  }
  const body = batch["body"];
  return { tree: batch["tree"], body: typeof body === "string" ? body : null };
}

/**
 * The executable cargo built for the binary `target`, from its `--message-format json` lines, or
 * `null` where none of them names one.
 */
export function cargoExecutable(messages: string, target: string): string | null {
  let found: string | null = null;
  for (const line of messages.split("\n")) {
    if (!line.startsWith("{")) {
      continue;
    }
    const message: unknown = JSON.parse(line);
    if (
      isObject(message) &&
      message["reason"] === "compiler-artifact" &&
      isObject(message["target"]) &&
      message["target"]["name"] === target &&
      typeof message["executable"] === "string"
    ) {
      found = message["executable"];
    }
  }
  return found;
}
