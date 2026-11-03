/**
 * The descent spike's finished segments (R14's Mac measurements): the parts of the renderer's
 * report put together as the run flies, and its results over them written each time another
 * segment has ended, so that a run stopped part-way keeps its finished segments.
 *
 * @remarks
 * The owner's Mac run of `just descent-spike --hidden`, stopped near the descent's end, wrote
 * nothing: the results were built only from the whole report, at the run's end. The renderer now
 * hands its frames over every few seconds of script ({@link SpikeReportPart}), and the results of
 * the frames so far are written as `<name>-partial.json` and `.md` beside where the run's results
 * go, which take their place as the run ends. Their trace is missing, its windows being read only
 * at the run's end, so their frame figures are `requestAnimationFrame`'s, and they carry neither
 * the memory series, which the sampler gives only at the run's end, nor the geometry and the sizes
 * blocks, which only the whole report carries.
 *
 * Building a whole descent's results takes some 70 to 160 ms (pod, 16 vCPU, 2026-11-03), which on
 * the main process's thread would hold a hidden run's offscreen frames, whose pace that thread
 * sets, so the run writes them on a worker thread (`spikePartials.worker.ts`), through a
 * {@link SpikePartialsSink}.
 */

import type { DescentSpikeReport, SpikePassSeries, SpikeReportPart } from "../preload/api";
import { missing } from "./measured";
import {
  buildResults,
  NODE_PARTIAL_FILES,
  PARTIAL_SUFFIX,
  type PartialResultsFiles,
  type RunDescription,
  writeResults,
} from "./results";
import { join } from "node:path";

/** Why a run's finished segments' file has no trace. */
export const PARTIAL_TRACE_REASON =
  "a partial file, written as a segment ended: the trace's windows are read only as the run ends";

/** `length` frames with no time. */
function nulls(length: number): Array<number | null> {
  return Array.from({ length }, () => null);
}

/**
 * `part` put after `merged`, the report so far: each frame series continued, a pass label new to
 * the part given `null` in the frames before it, and one it lacks `null` in its own; the rest of
 * the report the part's, which stands for the whole run so far.
 *
 * @throws Error for a part that does not begin where the frames so far end.
 */
export function mergeReportPart(
  merged: DescentSpikeReport | null,
  part: SpikeReportPart,
): DescentSpikeReport {
  const before = merged?.frames.scriptTimesS.length ?? 0;
  if (part.fromFrame !== before) {
    throw new Error(
      `the report's part begins at frame ${String(part.fromFrame)}, after ${String(before)} frames`,
    );
  }
  if (merged === null) {
    return part.report;
  }
  const was = merged.frames;
  const next = part.report.frames;
  const added = new Map(next.passes.map((series) => [series.label, series]));
  const passes: SpikePassSeries[] = [
    ...was.passes.map((series) => ({
      label: series.label,
      row: series.row,
      gpuMs: [
        ...series.gpuMs,
        ...(added.get(series.label)?.gpuMs ?? nulls(next.scriptTimesS.length)),
      ],
    })),
    ...next.passes
      .filter((series) => !was.passes.some(({ label }) => label === series.label))
      .map((series) => ({
        label: series.label,
        row: series.row,
        gpuMs: [...nulls(before), ...series.gpuMs],
      })),
  ];
  return {
    ...part.report,
    frames: {
      scriptTimesS: [...was.scriptTimesS, ...next.scriptTimesS],
      rafIntervalsMs: [...was.rafIntervalsMs, ...next.rafIntervalsMs],
      ourCodeMs: [...was.ourCodeMs, ...next.ourCodeMs],
      callbackStartsMs: [...was.callbackStartsMs, ...next.callbackStartsMs],
      missingResolves: [...was.missingResolves, ...next.missingResolves],
      inFlightResolves: [...was.inFlightResolves, ...next.inFlightResolves],
      passes,
    },
  };
}

/**
 * The run as its results record it: the terrain variant the renderer drew with, where the report
 * gives one, beside the flags that asked for it.
 */
export function runWithTerrain(run: RunDescription, report: DescentSpikeReport): RunDescription {
  return report.terrain === undefined
    ? run
    : {
        ...run,
        options: {
          ...run.options,
          terrainVertexPath: report.terrain.vertexPath,
          terrainNormals: report.terrain.normals,
        },
      };
}

/** Where a run's finished segments go, and the run they are of. */
export interface SpikePartialsStart {
  /** The results' directory (`--out`, or `docs/measurements/descent-spike/`). */
  readonly outDir: string;
  /** The run's results' name, taken as it started; its finished segments' is `<name>-partial`. */
  readonly name: string;
  readonly run: RunDescription;
}

/** A run's finished segments, written each time another has ended. */
export class SpikePartials {
  readonly #start: SpikePartialsStart;
  readonly #files: PartialResultsFiles;
  #report: DescentSpikeReport | null = null;
  /** The segments ended by the last file written. */
  #ended = 0;
  #written = false;

  constructor(start: SpikePartialsStart, files: PartialResultsFiles = NODE_PARTIAL_FILES) {
    this.#start = start;
    this.#files = files;
  }

  /**
   * Puts `part` after those before it, and writes the results so far where it has ended another
   * segment.
   *
   * @returns The results' path where they were written, else `null`.
   * @throws Error for a part that does not follow the last, or results that do not match their
   * schema.
   */
  async add(part: SpikeReportPart): Promise<string | null> {
    const report = mergeReportPart(this.#report, part);
    this.#report = report;
    const ended = report.segments.filter(({ endS }) => endS <= part.throughS).length;
    if (ended <= this.#ended) {
      return null;
    }
    this.#ended = ended;
    const results = buildResults({
      run: runWithTerrain(this.#start.run, report),
      report,
      trace: missing(PARTIAL_TRACE_REASON),
      memory: [],
    });
    const { json } = await writeResults(
      this.#start.outDir,
      results,
      this.#files,
      `${this.#start.name}${PARTIAL_SUFFIX}`,
    );
    this.#written = true;
    return json;
  }

  /** Removes the finished segments' files where any were written: the run's results stand. */
  async remove(): Promise<void> {
    if (!this.#written) {
      return;
    }
    const name = `${this.#start.name}${PARTIAL_SUFFIX}`;
    await this.#files.remove(join(this.#start.outDir, `${name}.json`));
    await this.#files.remove(join(this.#start.outDir, `${name}.md`));
  }
}

/**
 * Where a run hands the parts of its report (`SpikeSession`): a worker thread's
 * {@link SpikePartials} in a run (`spikePartialsWorker.ts`), so that building their results never
 * holds the main process's thread.
 */
export interface SpikePartialsSink {
  /** Takes the next part; its results, where it ends a segment, are written in the background. */
  add(part: SpikeReportPart): void;
  /**
   * Ends the writing, after every part taken: once the run's results are written (`written`),
   * the finished segments' files are removed.
   */
  close(written: boolean): Promise<void>;
}

/** What the finished segments' worker tells the main process: a line for its log, or that it has closed. */
export type PartialsReply =
  { readonly kind: "log"; readonly line: string } | { readonly kind: "closed" };

/**
 * A sink that writes on the thread it is called on, each part after the one before it, logging
 * what it writes and what fails: the worker's own, and a test's.
 */
export function partialsSink(
  partials: Pick<SpikePartials, "add" | "remove">,
  log: (line: string) => void,
): SpikePartialsSink {
  let queue: Promise<void> = Promise.resolve();
  const then = (step: () => Promise<void>): Promise<void> => {
    queue = queue.then(step).catch((error: unknown) => {
      log(
        `descent spike: the finished segments were not written: ${error instanceof Error ? error.message : String(error)}`,
      );
    });
    return queue;
  };
  return {
    add: (part) => {
      void then(async () => {
        const json = await partials.add(part);
        if (json !== null) {
          log(`descent spike: the segments finished so far in ${json}`);
        }
      });
    },
    close: (written) =>
      then(async () => {
        if (written) {
          await partials.remove();
        }
      }),
  };
}
