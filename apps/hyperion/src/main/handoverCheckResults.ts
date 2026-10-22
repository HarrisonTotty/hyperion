/**
 * The hand-over check's results file and its summary (plan R10, R10.T10.g; decision-r10-t16-leans
 * §1): what the renderer read of each look's frames either side of each switch, judged.
 *
 * @remarks
 * The verdict, on the launch's setting:
 *
 * - **Each look ran whole**: the three looks in order, each with its three switches compared (the
 *   regime's switch from the disc, the terrain's first draw against the disc's last frame, and the
 *   hand-back), its exposure locked and the lock held to its end, its survey held and every
 *   surveyed cell's centre mapping back to its cell, the camera model agreeing with each frame's
 *   pixels (no more than {@link HANDOVER_MISS_LIMIT} of the body's pixels missing the datum), and
 *   no harness failure: a look of each, the terminator's included.
 * - **The photometry, at 30° and 90°** (the terminator's recorded, not judged): at every switch,
 *   the body's and the survey's mean luminance, over each frame's own pixels and over the pixels
 *   both share, within {@link HANDOVER_BOUND_STOPS}, and every surveyed cell of at least
 *   {@link HANDOVER_MIN_SHARED_PIXELS} pixels both frames assign to it within it too, its mean over
 *   those pixels, with no floor (decision-r10-t16-leans §1). A cell dark on one side and not the
 *   other is over the bound, counted and listed, never dropped; one of a single shared pixel is
 *   listed and not judged.
 * - **No engine fault** over the run, and no exit of the GPU process; the page's console errors
 *   are recorded, not judged, as the terrain run's are.
 * - **Which looks are judged** is the look's name's ({@link HANDOVER_LOOK_RULES}), not the
 *   record's word.
 *
 * Every cell with a shared pixel is kept in the file, judged or not, with its change in stops.
 * The figures are provisional: the run is hidden, on a shared machine (the load average at the
 * start is recorded with them).
 */

import { access, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type {
  HandoverAngles,
  HandoverCheckRecord,
  HandoverComparisonRecord,
  HandoverFrameRecord,
  HandoverLookName,
  HandoverLookRecord,
  HandoverOwnMeans,
  HandoverRegime,
  HandoverSharedMeans,
  HandoverSwitchName,
  HandoverTexelRecord,
  TerrainCheckFault,
  ViewsCheckTerrainFixture,
} from "../preload/api";
import { keepsLoadAverage, QUIET_RULE_UNCHECKED, quietOf } from "./machineLoad";
import {
  ADDED_FILE_LIMIT_BYTES,
  formatAsPrettier,
  type Measured,
  type ResultsFiles,
  type Verdict,
} from "./results";
import { fixtureNaming, fixtureRecord, type TerrainCheckRunInput } from "./terrainCheckResults";

/** The file's schema name. */
export const HANDOVER_CHECK_SCHEMA = "hyperion.views-check.handover-results";

/** The schema's version; bumped with any change to the file's shape. */
export const HANDOVER_CHECK_VERSION = 1;

/**
 * The bound on a change across a switch, stops (log₂ of the ratio): plan 10's T10.e and T10.g, a
 * third of a stop either way, as decision-r10-t16-leans §1 reads it.
 */
export const HANDOVER_BOUND_STOPS = 1 / 3;

/**
 * The fewest pixels both frames assign to a cell for the bound to judge it: two, the least that is
 * not one pixel's aliasing, with no floor above it (decision-r10-t16-leans §1).
 */
export const HANDOVER_MIN_SHARED_PIXELS = 2;

/**
 * The largest share of a frame's body pixels whose ray through the centre may miss the datum, the
 * camera model's check: the limb's relief and its edge pixels alone miss, a ring a pixel or two
 * wide (under 2% of a disc 120 px across); a camera model that disagreed with the frame's would
 * miss far more.
 */
export const HANDOVER_MISS_LIMIT = 0.05;

/** The looks a run takes, in order. */
const LOOKS: ReadonlyArray<HandoverLookName> = [
  "handover-30",
  "handover-90",
  "handover-terminator",
];

/**
 * Each look's kept scene and whether the verdict judges it, by the look's name: the main process
 * decides which looks are judged (decision-r10-t16-leans §1: 30° and 90°, the terminator's
 * recorded), and the record's reader refuses a look whose scene or `judged` says otherwise. The
 * renderer's `HANDOVER_LOOKS` gives the same names, scenes and flags.
 */
export const HANDOVER_LOOK_RULES: Readonly<
  Record<HandoverLookName, { readonly scene: string; readonly judged: boolean }>
> = {
  "handover-30": { scene: "HAND-OVER 30°", judged: true },
  "handover-90": { scene: "HAND-OVER 90°", judged: true },
  "handover-terminator": { scene: "HAND-OVER TERMINATOR", judged: false },
};

/**
 * The regimes each switch's two frames may hold: the regime's switch leaves the disc for the figure
 * (or for the terrain, where its first mesh frame drew it), the terrain's first draw is the
 * terrain's against the disc's last frame, and the hand-back leaves the terrain for the disc. The
 * record's reader refuses a comparison of other regimes.
 */
export const HANDOVER_SWITCH_REGIMES: Readonly<
  Record<
    HandoverSwitchName,
    {
      readonly before: ReadonlyArray<HandoverRegime>;
      readonly after: ReadonlyArray<HandoverRegime>;
    }
  >
> = {
  "disc-to-figure": { before: ["disc"], after: ["figure", "terrain"] },
  "disc-to-terrain": { before: ["disc"], after: ["terrain"] },
  "terrain-to-disc": { before: ["terrain"], after: ["disc"] },
};

/** The switches each look compares, in order. */
const SWITCHES: ReadonlyArray<HandoverSwitchName> = [
  "disc-to-figure",
  "disc-to-terrain",
  "terrain-to-disc",
];

/** Everything the results are built from. */
export interface HandoverCheckInput {
  /** The run, as the main process describes it at its start: the terrain run's description. */
  readonly run: TerrainCheckRunInput;
  readonly record: HandoverCheckRecord;
  /** The GPU process's exits over the run. */
  readonly gpuProcessExits: number;
  /** The page's console errors over the run, in order. */
  readonly consoleErrors: ReadonlyArray<string>;
}

/** log₂(after ÷ before), stops; `null` where either is not above 0. */
export function stopsBetween(before: number, after: number): number | null {
  return before > 0 && after > 0 ? Math.log2(after / before) : null;
}

/** A pair of means with their change, stops. */
export type MeansFigures<T> = T & { readonly log2: number | null };

/** A cell's figures across a switch: its record, its change, and the verdict's reading of it. */
export interface HandoverTexelFigures extends HandoverTexelRecord {
  /** log₂(after ÷ before), stops; `null` where a side is dark. */
  readonly log2: number | null;
  /** Which side's mean is 0, if either. */
  readonly dark: "before" | "after" | "both" | null;
  /** At least {@link HANDOVER_MIN_SHARED_PIXELS} shared pixels. */
  readonly judged: boolean;
  /** Judged and changed by more than the bound (dark on one side among them). */
  readonly over: boolean;
}

/** One switch's figures, and their verdict. */
export interface HandoverComparisonFigures {
  readonly name: HandoverSwitchName;
  readonly before: HandoverFrameRecord;
  readonly after: HandoverFrameRecord;
  readonly framesApart: number;
  readonly beforeWasLast: boolean;
  readonly afterWasFirst: boolean;
  readonly body: {
    readonly own: MeansFigures<HandoverOwnMeans>;
    readonly shared: MeansFigures<HandoverSharedMeans>;
  };
  readonly survey: {
    readonly own: MeansFigures<HandoverOwnMeans>;
    readonly shared: MeansFigures<HandoverSharedMeans>;
  };
  /** Every cell with a shared pixel, in texel order. */
  readonly texels: ReadonlyArray<HandoverTexelFigures>;
  readonly judgedTexels: number;
  /** Judged cells over the bound, the dark ones among them. */
  readonly texelsOver: number;
  /** Judged cells dark on one side. */
  readonly darkTexels: number;
  /** The judged cell of the largest change (a dark one first), or `null` with none judged. */
  readonly worst: HandoverTexelFigures | null;
  /** `pass` or `fail` at a judged look; `recorded` at the terminator's. */
  readonly verdict: Verdict | "recorded";
  readonly reasons: ReadonlyArray<string>;
}

/** One look's figures, and their verdict. */
export interface HandoverLookFigures extends Omit<HandoverLookRecord, "comparisons"> {
  readonly comparisons: ReadonlyArray<HandoverComparisonFigures>;
  readonly verdict: Verdict;
  readonly reasons: ReadonlyArray<string>;
}

/** A results file of the hand-over check. */
export interface HandoverCheckResults {
  readonly schema: typeof HANDOVER_CHECK_SCHEMA;
  readonly version: typeof HANDOVER_CHECK_VERSION;
  readonly run: {
    readonly startedAt: string;
    readonly machine: TerrainCheckRunInput["machine"];
    readonly versions: TerrainCheckRunInput["versions"];
    readonly platform: NodeJS.Platform;
    readonly launchMode: TerrainCheckRunInput["launchMode"];
    readonly setting: TerrainCheckRunInput["setting"];
    readonly smoke: boolean;
    /** The weathered fixture on a `--weathered` run; absent on `TERRAIN TEST`'s (`fixtureRecord`). */
    readonly fixture?: Exclude<ViewsCheckTerrainFixture, "terrain-test">;
    readonly switches: ReadonlyArray<string>;
    readonly shown: boolean;
    readonly window: TerrainCheckRunInput["window"];
    readonly devicePixelRatio: number;
    readonly captures: Measured<string>;
    /** Every figure is provisional: the note says why. */
    readonly provisional: string;
  };
  readonly world: string;
  /** The primary's canvas (the presented size) and its scene target (the internal), px. */
  readonly canvas: HandoverCheckRecord["canvas"];
  readonly target: HandoverCheckRecord["target"];
  readonly reliefM: number;
  readonly tauPx: number;
  readonly radPerPx: number;
  readonly rangeM: number;
  readonly cellLevel: number;
  readonly approach: HandoverCheckRecord["approach"];
  /** The verdict's bounds, as this file was judged by them. */
  readonly bounds: {
    readonly stops: number;
    readonly minSharedPixels: number;
    readonly missLimit: number;
  };
  readonly looks: ReadonlyArray<HandoverLookFigures>;
  readonly findings: {
    /** The three looks ran, in order. */
    readonly looks: { readonly verdict: Verdict; readonly reasons: ReadonlyArray<string> };
    /** No engine fault and no exit of the GPU process over the run. */
    readonly faults: {
      readonly verdict: Verdict;
      readonly reasons: ReadonlyArray<string>;
      readonly record: ReadonlyArray<TerrainCheckFault>;
      readonly gpuProcessExits: number;
    };
    /** The page's console errors, recorded, not judged: their count and the first five. */
    readonly consoleErrors: { readonly count: number; readonly first: ReadonlyArray<string> };
  };
  /** Every look's verdict and the findings' together: `pass` only if each passes. */
  readonly overall: Verdict;
}

/** Own means with their change. */
function ownFigures(means: HandoverOwnMeans): MeansFigures<HandoverOwnMeans> {
  return { ...means, log2: stopsBetween(means.meanBefore, means.meanAfter) };
}

/** Shared means with their change. */
function sharedFigures(means: HandoverSharedMeans): MeansFigures<HandoverSharedMeans> {
  return { ...means, log2: stopsBetween(means.meanBefore, means.meanAfter) };
}

/** A cell's figures. */
export function texelFigures(texel: HandoverTexelRecord): HandoverTexelFigures {
  const dark =
    texel.meanBefore === 0 && texel.meanAfter === 0
      ? "both"
      : texel.meanBefore === 0
        ? "before"
        : texel.meanAfter === 0
          ? "after"
          : null;
  const log2 = stopsBetween(texel.meanBefore, texel.meanAfter);
  const judged = texel.sharedPixels >= HANDOVER_MIN_SHARED_PIXELS;
  const over =
    judged &&
    (dark === "before" ||
      dark === "after" ||
      (log2 !== null && Math.abs(log2) > HANDOVER_BOUND_STOPS));
  return { ...texel, log2, dark, judged, over };
}

/** The larger change of two cells, a dark one counting as the larger. */
function worseOf(a: HandoverTexelFigures, b: HandoverTexelFigures): HandoverTexelFigures {
  return changeSize(b) > changeSize(a) ? b : a;
}

/** A cell's change in stops, either way; a cell dark on one side's the largest. */
function changeSize(texel: HandoverTexelFigures): number {
  return texel.dark === "before" || texel.dark === "after" ? Infinity : Math.abs(texel.log2 ?? 0);
}

/** A change in stops as the summary writes it: signed, to 3 decimals, or why there is none. */
function stopsText(log2: number | null): string {
  return log2 === null ? "dark" : `${log2 >= 0 ? "+" : ""}${log2.toFixed(3)}`;
}

/** Where a mean's change breaks the bound, its reason; `null` where it is within it. */
function meanReason(what: string, log2: number | null, pixels: number): string | null {
  if (pixels === 0) {
    return `${what}: no pixel`;
  }
  if (log2 === null) {
    return `${what}: dark on one side`;
  }
  return Math.abs(log2) > HANDOVER_BOUND_STOPS
    ? `${what} changed by ${stopsText(log2)} stop`
    : null;
}

/** The camera model's check on a frame: its reason where too many body pixels miss the datum. */
function missReason(side: string, frame: HandoverFrameRecord): string | null {
  if (frame.bodyPixels === 0) {
    return `${side}: no pixel of the body`;
  }
  const share = frame.bodyMisses / frame.bodyPixels;
  return share > HANDOVER_MISS_LIMIT
    ? `${side}: ${String(frame.bodyMisses)} of ${String(frame.bodyPixels)} body pixels missed the datum (the camera model)`
    : null;
}

/** One switch's figures, judged where the look is. */
function comparisonFigures(
  comparison: HandoverComparisonRecord,
  judged: boolean,
): HandoverComparisonFigures {
  const texels = comparison.texels.map(texelFigures);
  const judgedTexels = texels.filter((texel) => texel.judged);
  const over = judgedTexels.filter((texel) => texel.over);
  const dark = judgedTexels.filter((texel) => texel.dark === "before" || texel.dark === "after");
  const body = {
    own: ownFigures(comparison.body.own),
    shared: sharedFigures(comparison.body.shared),
  };
  const survey = {
    own: ownFigures(comparison.survey.own),
    shared: sharedFigures(comparison.survey.shared),
  };
  const harness = [missReason("before", comparison.before), missReason("after", comparison.after)];
  const photometry = judged
    ? [
        meanReason(
          "the body over its own pixels",
          body.own.log2,
          Math.min(body.own.pixelsBefore, body.own.pixelsAfter),
        ),
        meanReason("the body over its shared pixels", body.shared.log2, body.shared.pixels),
        meanReason(
          "the survey over its own pixels",
          survey.own.log2,
          Math.min(survey.own.pixelsBefore, survey.own.pixelsAfter),
        ),
        meanReason("the survey over its shared pixels", survey.shared.log2, survey.shared.pixels),
        ...(judgedTexels.length === 0 ? ["no cell of 2 shared pixels or more to judge"] : []),
        ...(over.length === 0
          ? []
          : [
              `${String(over.length)} of ${String(judgedTexels.length)} cells over 1/3 stop${dark.length === 0 ? "" : ` (${String(dark.length)} dark on one side)`}`,
            ]),
      ]
    : [];
  const reasons = [...harness, ...photometry].flatMap((reason) =>
    reason === null ? [] : [`${comparison.name}: ${reason}`],
  );
  return {
    name: comparison.name,
    before: comparison.before,
    after: comparison.after,
    framesApart: comparison.framesApart,
    beforeWasLast: comparison.beforeWasLast,
    afterWasFirst: comparison.afterWasFirst,
    body,
    survey,
    texels,
    judgedTexels: judgedTexels.length,
    texelsOver: over.length,
    darkTexels: dark.length,
    worst: judgedTexels.reduce<HandoverTexelFigures | null>(
      (worst, texel) => (worst === null ? texel : worseOf(worst, texel)),
      null,
    ),
    verdict:
      !judged && harness.every((reason) => reason === null)
        ? "recorded"
        : reasons.length === 0
          ? "pass"
          : "fail",
    reasons,
  };
}

/** One look's figures and verdict. */
function lookFigures(look: HandoverLookRecord): HandoverLookFigures {
  // Judged by its name's rule, whatever the record says (the reader refuses a disagreement).
  const { judged } = HANDOVER_LOOK_RULES[look.name];
  const comparisons = look.comparisons.map((each) => comparisonFigures(each, judged));
  const reasons = [
    ...look.failures,
    ...(look.exposure.lockedEv100 === null ? ["the exposure was not locked"] : []),
    ...(look.exposure.held
      ? []
      : [
          `the exposure's lock did not hold (it read ${look.exposure.endReading ?? "nothing"} at the end)`,
        ]),
    ...(look.surveyedCells === 0 ? ["no surveyed cell was held"] : []),
    ...(look.mappingMismatches > 0
      ? [
          `${String(look.mappingMismatches)} surveyed cells' centres did not map back to their cells`,
        ]
      : []),
    ...SWITCHES.filter((name) => !comparisons.some((each) => each.name === name)).map(
      (name) => `${name} was not compared`,
    ),
    ...comparisons.flatMap((each) => each.reasons),
  ];
  return {
    ...look,
    judged,
    comparisons,
    verdict: reasons.length === 0 ? "pass" : "fail",
    reasons: reasons.map((reason) => `${look.name}: ${reason}`),
  };
}

/** The results of a run, judged. */
export function buildHandoverCheckResults(input: HandoverCheckInput): HandoverCheckResults {
  const { run, record } = input;
  const looks = record.looks.map(lookFigures);
  const ran = looks.map((look) => look.name);
  const looksReasons =
    ran.join(",") === LOOKS.join(",")
      ? []
      : [
          `the looks ran ${ran.length === 0 ? "none" : ran.join(", ")}, where ${LOOKS.join(", ")} are asked`,
        ];
  const faultsReasons = [
    ...record.faults.map((fault) => `a fault: ${fault.line}`),
    ...(input.gpuProcessExits > 0
      ? [`${String(input.gpuProcessExits)} exits of the GPU process`]
      : []),
  ];
  const quiet = quietOf(run.platform, run.machine.loadAverage);
  const load = run.machine.loadAverage[0];
  const provisional = [
    keepsLoadAverage(run.platform)
      ? `load average ${load.toFixed(2)} at the start`
      : QUIET_RULE_UNCHECKED,
    ...(quiet.provisional ? ["a shared machine"] : []),
    ...(run.shown ? [] : ["a hidden run: no presentation"]),
    ...(run.smoke
      ? ["a smoke run: a fast line, which proves the harness and not the photometry"]
      : []),
  ].join("; ");
  const verdicts = [
    ...looks.map((look) => look.verdict),
    looksReasons.length === 0 ? "pass" : "fail",
    faultsReasons.length === 0 ? "pass" : "fail",
  ];
  return {
    schema: HANDOVER_CHECK_SCHEMA,
    version: HANDOVER_CHECK_VERSION,
    run: {
      startedAt: run.startedAt.toISOString(),
      machine: run.machine,
      versions: run.versions,
      platform: run.platform,
      launchMode: run.launchMode,
      setting: run.setting,
      smoke: run.smoke,
      ...fixtureRecord(run.fixture),
      switches: run.switches,
      shown: run.shown,
      window: run.window,
      devicePixelRatio: record.devicePixelRatio,
      captures: run.captures,
      provisional: `provisional: ${provisional}`,
    },
    world: record.world,
    canvas: record.canvas,
    target: record.target,
    reliefM: record.reliefM,
    tauPx: record.tauPx,
    radPerPx: record.radPerPx,
    rangeM: record.rangeM,
    cellLevel: record.cellLevel,
    approach: record.approach,
    bounds: {
      stops: HANDOVER_BOUND_STOPS,
      minSharedPixels: HANDOVER_MIN_SHARED_PIXELS,
      missLimit: HANDOVER_MISS_LIMIT,
    },
    looks,
    findings: {
      looks: { verdict: looksReasons.length === 0 ? "pass" : "fail", reasons: looksReasons },
      faults: {
        verdict: faultsReasons.length === 0 ? "pass" : "fail",
        reasons: faultsReasons,
        record: record.faults,
        gpuProcessExits: input.gpuProcessExits,
      },
      consoleErrors: { count: input.consoleErrors.length, first: input.consoleErrors.slice(0, 5) },
    },
    overall: verdicts.every((verdict) => verdict === "pass") ? "pass" : "fail",
  };
}

/** The reasons a run's results failed, as one line; empty for a pass. */
export function handoverFailures(results: HandoverCheckResults): string {
  return [
    ...results.findings.looks.reasons,
    ...results.looks.flatMap((look) => look.reasons),
    ...results.findings.faults.reasons,
  ].join("; ");
}

/** A figure's text, or its reason in brackets. */
function textOf<T>(figure: Measured<T>, text: (value: T) => string): string {
  return figure.value === null ? `— (${figure.reason})` : text(figure.value);
}

/** Angles as the summary writes them: i, e and α, degrees. */
function anglesText(angles: HandoverAngles): string {
  return `${angles.incidenceDeg.toFixed(2)}°, ${angles.emissionDeg.toFixed(2)}°, ${angles.phaseDeg.toFixed(2)}°`;
}

/** A frame as the summary writes it: its script time, regime and range ratio. */
function frameText(frame: HandoverFrameRecord): string {
  return `${frame.scriptS.toFixed(2)} s ${frame.regime} at ${frame.rangeRatio.toFixed(4)}`;
}

/** Own and shared changes, `own / shared`. */
function pairText(pair: {
  readonly own: { readonly log2: number | null };
  readonly shared: { readonly log2: number | null };
}): string {
  return `${stopsText(pair.own.log2)} / ${stopsText(pair.shared.log2)}`;
}

/** The results' summary, for a person reading the record. */
export function handoverCheckMarkdown(results: HandoverCheckResults): string {
  const { run, looks } = results;
  const comparisonRows = looks.flatMap((look) =>
    look.comparisons.map((each) => {
      const worst =
        each.worst === null
          ? "—"
          : `${stopsText(each.worst.log2)} (cell ${String(each.worst.texel)}, ${String(each.worst.sharedPixels)} px)`;
      return `| ${look.name} | ${each.name} | ${frameText(each.before)} → ${frameText(each.after)} | ${String(each.framesApart)}${each.beforeWasLast && each.afterWasFirst ? "" : " (not adjacent in regime)"} | ${pairText(each.body)} | ${pairText(each.survey)} | ${String(each.judgedTexels)} of ${String(each.texels.length)} | ${String(each.texelsOver)} | ${String(each.darkTexels)} | ${worst} | ${each.verdict} |`;
    }),
  );
  const overRows = looks.flatMap((look) =>
    look.comparisons.flatMap((each) =>
      each.texels
        .filter((texel) => texel.over)
        .map(
          (texel) =>
            `| ${look.name} | ${each.name} | ${String(texel.texel)} | ${String(texel.sharedPixels)} | ${texel.meanBefore.toPrecision(4)} | ${texel.meanAfter.toPrecision(4)} | ${stopsText(texel.log2)} |`,
        ),
    ),
  );
  const reasons = [
    ...results.findings.looks.reasons,
    ...looks.flatMap((look) => look.reasons),
    ...results.findings.faults.reasons,
  ];
  return [
    `# Hand-over check: ${run.machine.name}, ${run.setting}, ${run.startedAt.slice(0, 10)}`,
    "",
    `- **Overall:** ${results.overall}`,
    `- **Machine:** ${run.machine.cpu}, ${String(run.machine.logicalCores)} threads; GPU ${textOf(run.machine.gpu, (gpu) => gpu.description ?? `${String(gpu.vendorId)}:${String(gpu.deviceId)}`)}; load average ${keepsLoadAverage(run.platform) ? run.machine.loadAverage.map((value) => value.toFixed(2)).join(", ") : "none"} (${run.provisional})`,
    `- **Versions:** app ${run.versions.app}, Electron ${run.versions.electron}, Chromium ${run.versions.chromium}`,
    `- **Launch:** ${run.platform}, ${run.launchMode} mode, setting ${run.setting}, window ${run.shown ? "shown" : "hidden"} ${String(run.window.widthDip)} × ${String(run.window.heightDip)} DIP at a device-pixel ratio of ${String(run.devicePixelRatio)}`,
    ...fixtureNaming(run.fixture).lines,
    `- **World:** ${results.world}; the primary ${String(results.canvas.widthPx)} × ${String(results.canvas.heightPx)} px presented, its scene target ${String(results.target.widthPx)} × ${String(results.target.heightPx)} px; the hand-over range ${(results.rangeM / 1000).toFixed(0)} km (relief ${results.reliefM.toFixed(0)} m at τ ${String(results.tauPx)} px, ${results.radPerPx.toExponential(4)} rad a pixel); cells of level ${String(results.cellLevel)}`,
    `- **The line:** ${(results.approach.ratePerS * 100).toFixed(0)}% of the limb distance a second, from ${String(results.approach.startRatio)} to ${String(results.approach.closestRatio)} of the range, a ${String(results.approach.holdS)} s hold, out to ${String(results.approach.endRatio)}`,
    `- **Bounds:** ${results.bounds.stops.toFixed(3)} stop at every switch, cells of ${String(results.bounds.minSharedPixels)} shared pixels or more judged, no floor; at most ${String(results.bounds.missLimit * 100)}% of a frame's body pixels off the datum`,
    `- **Captures (not committed):** ${textOf(run.captures, (dir) => `\`${dir}\``)}`,
    `- **Faults:** ${String(results.findings.faults.record.length)} engine faults, ${String(results.findings.faults.gpuProcessExits)} GPU-process exits; console errors ${String(results.findings.consoleErrors.count)}${results.findings.consoleErrors.first.length === 0 ? "" : ` (${results.findings.consoleErrors.first.join(" | ")})`}`,
    "",
    "## Looks",
    "",
    "| Look | Verdict | Judged | i, e, α asked | i, e, α at the range | EV100 locked (held) | Survey cells | Switches (script s: figure, terrain, disc) |",
    "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ...looks.map(
      (look) =>
        `| ${look.name} | ${look.verdict} | ${look.judged ? "yes" : "no, recorded"} | ${anglesText(look.nominal)} | ${anglesText(look.atRange)} | ${look.exposure.lockedEv100 === null ? "—" : look.exposure.lockedEv100.toFixed(1)} (${look.exposure.held ? "held" : "NOT held"}) | ${String(look.surveyedCells)} | ${[look.switchesS.figure, look.switchesS.terrain, look.switchesS.disc].map((s) => (s === null ? "—" : s.toFixed(2))).join(", ")} |`,
    ),
    "",
    "## Switches (provisional)",
    "",
    "Changes are log₂(after ÷ before), stops, over each frame's own pixels / over the pixels both share.",
    "",
    "| Look | Switch | Frames (script s, regime, limb distance ÷ range) | Frames apart | Body | Survey | Cells judged | Over 1/3 stop | Dark on one side | Worst cell | Verdict |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ...comparisonRows,
    "",
    "## Cells over 1/3 stop",
    "",
    ...(overRows.length === 0
      ? ["None."]
      : [
          "| Look | Switch | Cell | Shared pixels | Mean before | Mean after | Change (stops) |",
          "| --- | --- | --- | --- | --- | --- | --- |",
          ...overRows,
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
 * Writes the results and their summary into `dir` as `<date>-<machine>-<setting>-handover.json`
 * and `.md` (`-handover-weathered` for a run on the weathered fixture, R10.T1.c; `-smoke` before
 * the extension for a smoke run), adding `-2`, `-3` and so on where a run of the same name is
 * there, and warns of a file over the repository's limit for added files.
 *
 * @throws Error if the day's names are used up.
 */
export async function writeHandoverResults(
  dir: string,
  results: HandoverCheckResults,
  files: ResultsFiles = NODE_FILES,
): Promise<{ readonly json: string; readonly markdown: string }> {
  await files.mkdir(dir);
  const stem = `${results.run.startedAt.slice(0, 10)}-${results.run.machine.name}-${results.run.setting}-handover${fixtureNaming(results.run.fixture).stem}${results.run.smoke ? "-smoke" : ""}`;
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
  await files.writeFile(markdown, handoverCheckMarkdown(results));
  const bytes = Buffer.byteLength(formatAsPrettier(text), "utf8");
  if (bytes > ADDED_FILE_LIMIT_BYTES) {
    console.warn(
      `views check: ${json} is ${String(bytes)} B once formatted, over the ${String(ADDED_FILE_LIMIT_BYTES)} B the repository accepts for an added file`,
    );
  }
  return { json, markdown };
}
