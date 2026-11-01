/**
 * The atmosphere run's results file and its summary (plan R08, R08.T10.b): what the renderer
 * recorded of its hand flight on `ATMOSPHERE TEST`, the moonlit night captured from the seat and
 * then each air fixture from a few radii, judged by its captures and by its faults.
 *
 * @remarks
 * The run passes when both its findings pass (R08.T10.g).
 *
 * - The captures' ({@link atmosphereCaptureFailures}): the run took a capture, the main process
 *   saved its pictures, and every capture's primary was drawn photorealistic with nothing on its
 *   fault line. Every fault line on this plate is a graphics fault, a terrain fault (which this
 *   kept scene, with no terrain, cannot raise) or an atmosphere fault (`ATMOSPHERE NOT AVAILABLE:
 *   …`, `ATMOSPHERE DATA INVALID: …`); the reasons name each failing capture and its line, and
 *   class the line as `atmosphere` (those two) or `graphics` (any other)
 *   ({@link atmosphereFaultKind}).
 * - The faults' ({@link atmosphereFaultFailures}): no engine fault over the run and no exit of the
 *   GPU process, as the hand-over check's rule.
 *
 * The fault lines alone miss a lost device. The GPU process's fault clears once the adapter answers
 * again, and where the photorealistic style cannot be remade the primary goes back to the
 * wireframe with no fault line, so a capture taken after the loss shows a clean plate over a lost
 * picture: its drawn style names it. A device that comes back photorealistic may still have lost
 * its state (the air's tables, the exposure): the engine's faults and the GPU process's exits name
 * that. The page's console errors are recorded, not judged.
 *
 * The frame and GPU times are provisional: the run is hidden, on a shared machine (the load
 * average at the start is recorded with them).
 */

import { access, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type {
  AtmosphereCheckRecord,
  AtmosphereCheckStage,
  AtmosphereFixtureCapture,
  AtmosphereNightCapture,
  SpikePassTimer,
  ViewsCheckCanvas,
} from "../preload/api";
import { keepsLoadAverage, QUIET_RULE_UNCHECKED, quietOf } from "./machineLoad";
import {
  ADDED_FILE_LIMIT_BYTES,
  formatAsPrettier,
  type Measured,
  type ResultsFiles,
  type Verdict,
} from "./results";
import type { TerrainCheckRunInput } from "./terrainCheckResults";

/** The file's schema name. */
export const ATMOSPHERE_CHECK_SCHEMA = "hyperion.views-check.atmosphere-results";

/**
 * The schema's version; bumped with any change to the file's shape: 2 with each capture's `kind`
 * (a fixture's or the night's, the night's with no designation, range or phase) and its
 * `exposureSettleMs`; 3 with the flight's `stages`; 4 with each capture's drawn `style` and the
 * faults' `verdict` and `reasons` (R08.T10.g); 5 with each capture's `primaryScale` and
 * `airSetting` (fix-retina-frame).
 */
export const ATMOSPHERE_CHECK_VERSION = 5;

/**
 * The run, as the main process describes it at its start: the terrain run's description, without
 * the served field (`ATMOSPHERE TEST` serves none).
 */
export type AtmosphereCheckRunInput = Omit<TerrainCheckRunInput, "fixture">;

/** Everything the results are built from. */
export interface AtmosphereCheckInput {
  readonly run: AtmosphereCheckRunInput;
  readonly record: AtmosphereCheckRecord;
  /** The GPU process's exits over the run, judged with the engine's faults. */
  readonly gpuProcessExits: number;
  /** The page's console errors over the run, in order, recorded, not judged. */
  readonly consoleErrors: ReadonlyArray<string>;
}

/**
 * A fault line's class on the primary's label block: an atmosphere fault (`ATMOSPHERE NOT
 * AVAILABLE`, `ATMOSPHERE DATA INVALID`), or a graphics fault (any other line).
 */
export type AtmosphereFaultKind = "atmosphere" | "graphics";

/** A fault line with its class. */
export interface AtmosphereFaultLine {
  readonly kind: AtmosphereFaultKind;
  readonly line: string;
}

/** The atmosphere's two fault lines (the renderer's `view/atmosphere/labels.ts`). */
const ATMOSPHERE_FAULT = /^ATMOSPHERE (?:NOT AVAILABLE|DATA INVALID)\b/u;

/** The class of a fault line of the primary's label block. */
export function atmosphereFaultKind(line: string): AtmosphereFaultKind {
  return ATMOSPHERE_FAULT.test(line) ? "atmosphere" : "graphics";
}

/** A capture's judgement: its fault lines classed, and its verdict. */
export interface AtmosphereCaptureJudgement {
  readonly faults: ReadonlyArray<AtmosphereFaultLine>;
  /** `pass` when its primary was drawn photorealistic and its fault line held no line. */
  readonly verdict: Verdict;
}

/** One capture's figures, of either kind: its record, its fault lines classed, and its verdict. */
export type AtmosphereCaptureFigures =
  | (Omit<AtmosphereFixtureCapture, "faults"> & AtmosphereCaptureJudgement)
  | (Omit<AtmosphereNightCapture, "faults"> & AtmosphereCaptureJudgement);

/** A results file of the atmosphere run. */
export interface AtmosphereCheckResults {
  readonly schema: typeof ATMOSPHERE_CHECK_SCHEMA;
  readonly version: typeof ATMOSPHERE_CHECK_VERSION;
  readonly run: {
    readonly startedAt: string;
    readonly machine: AtmosphereCheckRunInput["machine"];
    readonly versions: AtmosphereCheckRunInput["versions"];
    readonly platform: NodeJS.Platform;
    readonly launchMode: AtmosphereCheckRunInput["launchMode"];
    readonly setting: AtmosphereCheckRunInput["setting"];
    readonly smoke: boolean;
    readonly switches: ReadonlyArray<string>;
    readonly shown: boolean;
    readonly window: AtmosphereCheckRunInput["window"];
    readonly devicePixelRatio: number;
    readonly captures: Measured<string>;
    /** Every figure is provisional: the note says why. */
    readonly provisional: string;
  };
  /** The pass timer's kind, which the GPU times were read by. */
  readonly timer: SpikePassTimer;
  /** The primary's canvas as the run ended, px (the presented size). */
  readonly canvas: ViewsCheckCanvas;
  /** From the scene's choice to its fixtures' media ready, ms. */
  readonly mediaReadyMs: number;
  /** The captures, in the order taken. */
  readonly captures: ReadonlyArray<AtmosphereCaptureFigures>;
  /** The flight's stages as flown, in order, recorded, not judged. */
  readonly stages: ReadonlyArray<AtmosphereCheckStage>;
  readonly findings: {
    /**
     * The captures' rule: a capture taken and saved, every capture drawn photorealistic, and no
     * capture's fault line holding a line.
     */
    readonly captures: {
      readonly verdict: Verdict;
      readonly reasons: ReadonlyArray<string>;
      /** The captures' fault lines of each class, counted over every capture. */
      readonly atmosphereFaults: number;
      readonly graphicsFaults: number;
    };
    /** The faults' rule: no engine fault over the run and no exit of the GPU process. */
    readonly faults: {
      readonly verdict: Verdict;
      readonly reasons: ReadonlyArray<string>;
      readonly record: ReadonlyArray<string>;
      readonly gpuProcessExits: number;
    };
    /** The page's console errors, recorded, not judged: their count and the first five. */
    readonly consoleErrors: { readonly count: number; readonly first: ReadonlyArray<string> };
  };
  /** `pass` when both findings pass. */
  readonly overall: Verdict;
}

/** The drawn style a capture's verdict needs. */
const CAPTURE_STYLE = "photorealistic";

/**
 * Why the run's captures fail, one reason a failure; empty for a pass: the captures' rule
 * (R08.T10.b, R08.T10.g).
 *
 * @remarks
 * A run fails when its record holds no capture, when the main process saved no picture of any,
 * when any capture's primary was not drawn photorealistic, named with its capture and the style it
 * drew, or when any capture's fault line held a line, each such line named with its capture and its
 * class.
 *
 * @param captured - Whether the main process saved a picture of the run's captures (the session's
 * captures directory measured).
 */
export function atmosphereCaptureFailures(
  record: AtmosphereCheckRecord,
  captured: boolean,
): ReadonlyArray<string> {
  return [
    ...(record.captures.length === 0 ? ["no capture was taken"] : []),
    ...(captured ? [] : ["the main process saved no capture"]),
    ...record.captures.flatMap((capture) => [
      ...(capture.style === CAPTURE_STYLE
        ? []
        : [`${capture.label}: drawn ${capture.style ?? "in no style"}, not ${CAPTURE_STYLE}`]),
      ...capture.faults.map(
        (line) => `${capture.label}: ${atmosphereFaultKind(line)} fault: ${line}`,
      ),
    ]),
  ];
}

/**
 * Why the run's faults fail, one reason a failure; empty for a pass: the faults' rule (R08.T10.g),
 * worded as the hand-over check's: each engine fault over the run, and the GPU process's exits.
 *
 * @param gpuProcessExits - The GPU process's exits over the run, as the session counted them.
 */
export function atmosphereFaultFailures(
  record: AtmosphereCheckRecord,
  gpuProcessExits: number,
): ReadonlyArray<string> {
  return [
    ...record.faults.map((line) => `a fault: ${line}`),
    ...(gpuProcessExits > 0 ? [`${String(gpuProcessExits)} exits of the GPU process`] : []),
  ];
}

/** The results of a run, judged. */
export function buildAtmosphereCheckResults(input: AtmosphereCheckInput): AtmosphereCheckResults {
  const { run, record } = input;
  const captures = record.captures.map((capture): AtmosphereCaptureFigures => ({
    ...capture,
    faults: capture.faults.map((line) => ({ kind: atmosphereFaultKind(line), line })),
    verdict: capture.style === CAPTURE_STYLE && capture.faults.length === 0 ? "pass" : "fail",
  }));
  const lines = captures.flatMap((capture) => capture.faults);
  const reasons = atmosphereCaptureFailures(record, run.captures.value !== null);
  const faultsReasons = atmosphereFaultFailures(record, input.gpuProcessExits);
  const quiet = quietOf(run.platform, run.machine.loadAverage);
  const load = run.machine.loadAverage[0];
  const provisional = [
    keepsLoadAverage(run.platform)
      ? `load average ${load.toFixed(2)} at the start`
      : QUIET_RULE_UNCHECKED,
    ...(quiet.provisional ? ["a shared machine"] : []),
    ...(run.shown ? [] : ["a hidden run: no presentation"]),
    ...(run.smoke
      ? ["a smoke run: a fast flight, which proves the harness and not the times"]
      : []),
  ].join("; ");
  const verdict: Verdict = reasons.length === 0 ? "pass" : "fail";
  const faultsVerdict: Verdict = faultsReasons.length === 0 ? "pass" : "fail";
  return {
    schema: ATMOSPHERE_CHECK_SCHEMA,
    version: ATMOSPHERE_CHECK_VERSION,
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
      captures: run.captures,
      provisional: `provisional: ${provisional}`,
    },
    timer: record.timer,
    canvas: record.canvas,
    mediaReadyMs: record.mediaReadyMs,
    captures,
    stages: record.stages,
    findings: {
      captures: {
        verdict,
        reasons,
        atmosphereFaults: lines.filter((each) => each.kind === "atmosphere").length,
        graphicsFaults: lines.filter((each) => each.kind === "graphics").length,
      },
      faults: {
        verdict: faultsVerdict,
        reasons: faultsReasons,
        record: record.faults,
        gpuProcessExits: input.gpuProcessExits,
      },
      consoleErrors: { count: input.consoleErrors.length, first: input.consoleErrors.slice(0, 5) },
    },
    overall: verdict === "pass" && faultsVerdict === "pass" ? "pass" : "fail",
  };
}

/** Every reason a run's results failed, the captures' then the faults'. */
function failureReasons(results: AtmosphereCheckResults): ReadonlyArray<string> {
  return [...results.findings.captures.reasons, ...results.findings.faults.reasons];
}

/** The reasons a run's results failed, as one line; empty for a pass. */
export function atmosphereFailures(results: AtmosphereCheckResults): string {
  return failureReasons(results).join("; ");
}

/** A figure's text, or its reason in brackets. */
function textOf<T>(figure: Measured<T>, text: (value: T) => string): string {
  return figure.value === null ? `— (${figure.reason})` : text(figure.value);
}

/** A table cell's text, its pipes escaped. */
function cell(text: string): string {
  return text.replaceAll("|", "\\|");
}

/** A time or `—`, ms, to `digits` decimals. */
function msText(ms: number | null, digits: number): string {
  return ms === null ? "—" : ms.toFixed(digits);
}

/** A range as the summary writes it: km under 10 Mm, Mm from there; `—` for none. */
export function rangeText(rangeM: number | null): string {
  if (rangeM === null) {
    return "—";
  }
  return rangeM < 1e7 ? `${(rangeM / 1000).toFixed(0)} km` : `${(rangeM / 1e6).toFixed(1)} Mm`;
}

/** A capture's designation, range and phase cells: the fixture's, or `—` for the night. */
function placeCells(capture: AtmosphereCaptureFigures): readonly [string, string, string] {
  let cells: readonly [string, string, string];
  switch (capture.kind) {
    case "fixture":
      cells = [
        cell(capture.designation),
        rangeText(capture.rangeM),
        `${capture.phaseDeg.toFixed(1)}°`,
      ];
      break;
    case "night":
      cells = ["—", "—", "—"];
      break;
  }
  return cells;
}

/** A stage's row of the summary's table. */
function stageRow(stage: AtmosphereCheckStage): string {
  return [
    "",
    cell(stage.target),
    String(stage.stage),
    String(stage.rateStep),
    stage.askedM.toExponential(3),
    `${(stage.holdMs / 1000).toFixed(3)} / ${(stage.heldMs / 1000).toFixed(3)}`,
    stage.longestStepMs.toFixed(0),
    stage.flownM.toExponential(3),
    stage.shortOfCentreM.toExponential(3),
    "",
  ]
    .join(" | ")
    .trim();
}

/** The results' summary, for a person reading the record. */
export function atmosphereCheckMarkdown(results: AtmosphereCheckResults): string {
  const { run, findings } = results;
  const rows = results.captures.map((capture) => {
    const { frames } = capture;
    const faults =
      capture.faults.length === 0
        ? "—"
        : capture.faults.map((each) => `${each.kind}: ${each.line}`).join("; ");
    const [designation, range, phase] = placeCells(capture);
    return `| ${cell(capture.label)} | ${designation} | ${cell(capture.camera ?? "—")} | ${capture.style ?? "—"} | ${range} | ${phase} | ${cell(capture.exposure ?? "—")} | ${(capture.exposureSettleMs / 1000).toFixed(1)} | ${String(frames.draws)} | ${msText(frames.intervalMedianMs, 1)} / ${msText(frames.intervalP95Ms, 1)} | ${msText(frames.gpuMedianMs, 2)} / ${msText(frames.gpuP95Ms, 2)} | ${capture.primaryScale === null ? "—" : capture.primaryScale.toFixed(2)} · ${capture.airSetting} | ${cell(capture.statements.length === 0 ? "—" : capture.statements.join(" · "))} | ${cell(faults)} |`;
  });
  const errors = findings.consoleErrors;
  const reasons = failureReasons(results);
  return [
    `# Atmosphere flight: ${run.machine.name}, ${run.setting}, ${run.startedAt.slice(0, 10)}`,
    "",
    `- **Overall:** ${results.overall}`,
    `- **Machine:** ${run.machine.cpu}, ${String(run.machine.logicalCores)} threads; GPU ${textOf(run.machine.gpu, (gpu) => gpu.description ?? `${String(gpu.vendorId)}:${String(gpu.deviceId)}`)}; load average ${keepsLoadAverage(run.platform) ? run.machine.loadAverage.map((value) => value.toFixed(2)).join(", ") : "none"} (${run.provisional})`,
    `- **Versions:** app ${run.versions.app}, Electron ${run.versions.electron}, Chromium ${run.versions.chromium}`,
    `- **Launch:** ${run.platform}, ${run.launchMode} mode, setting ${run.setting}, window ${run.shown ? "shown" : "hidden"} ${String(run.window.widthDip)} × ${String(run.window.heightDip)} DIP at a device-pixel ratio of ${String(run.devicePixelRatio)}; the primary ${String(results.canvas.widthPx)} × ${String(results.canvas.heightPx)} px; pass timer ${results.timer}`,
    `- **Media ready:** ${(results.mediaReadyMs / 1000).toFixed(1)} s after the scene's choice`,
    `- **Captures (not committed):** ${textOf(run.captures, (dir) => `\`${dir}\``)}`,
    `- **Faults:** fault lines ${String(findings.captures.atmosphereFaults)} atmosphere, ${String(findings.captures.graphicsFaults)} graphics; ${String(findings.faults.record.length)} engine faults, ${String(findings.faults.gpuProcessExits)} GPU-process exits; recorded, not judged: console errors ${String(errors.count)}${errors.first.length === 0 ? "" : ` (${errors.first.join(" | ")})`}`,
    "",
    "## Captures (provisional times)",
    "",
    "| Capture | Designation | Camera | Style | Range | Phase | Exposure | AUTO settled (s) | Draws | Frame interval median / p95 (ms) | GPU median / p95 (ms) | Scale · air | Statements | Fault lines |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ...rows,
    "",
    "## Stages (as reckoned)",
    "",
    ...(results.stages.length === 0
      ? ["None."]
      : [
          "| Target | Stage | Rate step | Asked (m) | Hold planned / held (s) | Longest step (ms) | Flown (m) | Short of centre (m) |",
          "| --- | --- | --- | --- | --- | --- | --- | --- |",
          ...results.stages.map(stageRow),
        ]),
    "",
    "## Reasons",
    "",
    ...(reasons.length === 0 ? ["None."] : reasons.map((reason) => `- ${reason}`)),
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
 * Writes the results and their summary into `dir` as `<date>-<machine>-<setting>-atmosphere.json`
 * and `.md` (`-smoke` before the extension for a smoke run), adding `-2`, `-3` and so on where a
 * run of the same name is there, and warns of a file over the repository's limit for added files.
 *
 * @throws Error if the day's names are used up.
 */
export async function writeAtmosphereResults(
  dir: string,
  results: AtmosphereCheckResults,
  files: ResultsFiles = NODE_FILES,
): Promise<{ readonly json: string; readonly markdown: string }> {
  await files.mkdir(dir);
  const stem = `${results.run.startedAt.slice(0, 10)}-${results.run.machine.name}-${results.run.setting}-atmosphere${results.run.smoke ? "-smoke" : ""}`;
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
  await files.writeFile(markdown, atmosphereCheckMarkdown(results));
  const bytes = Buffer.byteLength(formatAsPrettier(text), "utf8");
  if (bytes > ADDED_FILE_LIMIT_BYTES) {
    console.warn(
      `views check: ${json} is ${String(bytes)} B once formatted, over the ${String(ADDED_FILE_LIMIT_BYTES)} B the repository accepts for an added file`,
    );
  }
  return { json, markdown };
}
