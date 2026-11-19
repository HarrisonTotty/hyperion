/**
 * Perf mode's IPC handlers in the main process (plan R12, R12.T4.b; Design note 14): one fixed
 * channel an operation of `window.hyperion.perf`, each call checked against the run window's own
 * page and each argument read by `@hyperion/perf`'s schema before the runner sees it.
 *
 * @remarks
 * The channel names are repeated in `preload/perfApi.ts`'s `PERF_CHANNEL_NAMES`, which the preload
 * bundles apart; `perfApi.test.ts` holds them equal.
 */

import {
  BUDGET_ROWS,
  type BudgetRow,
  isMemoryItem,
  isPassKey,
  type PassKey,
  perfSchema,
  PerfRecordError,
  VIEW_ROLES,
} from "@hyperion/perf";

import type {
  PerfEnd,
  PerfRepetitionResult,
  PerfRunDescription,
  PerfWarmUpSample,
} from "../../preload/api";

/** Perf mode's channels, one an operation. */
export const PERF_CHANNELS = {
  config: "hyperion:perf:config",
  describe: "hyperion:perf:describe",
  warmUpSample: "hyperion:perf:warm-up-sample",
  beginRepetition: "hyperion:perf:begin-repetition",
  endRepetition: "hyperion:perf:end-repetition",
  finish: "hyperion:perf:finish",
} as const;

const {
  array,
  bool,
  count,
  fraction,
  jsonObject,
  keyed,
  narrowed,
  nonEmpty,
  nonNegative,
  nullable,
  num,
  object,
  oneOf,
  positiveInt,
  str,
} = perfSchema;

const nNonNegative = nullable(nonNegative);
const budgetRow = narrowed<BudgetRow>(
  (text): text is BudgetRow => BUDGET_ROWS.some((row) => row === text),
  "a budget row",
);

/** `value` as a finite number's array. */
const numbers = array(num);

/** An object of `schema`'s values under keys that are not empty, every key present. */
function named<V>(schema: perfSchema.Schema<V>): perfSchema.Schema<Readonly<Record<string, V>>> {
  return (value, path) => {
    const read = keyed(
      (key: string): key is string => key.length > 0,
      "a name",
      schema,
    )(value, path);
    const out: Record<string, V> = {};
    for (const [key, item] of Object.entries(read)) {
      if (item !== undefined) {
        out[key] = item;
      }
    }
    return out;
  };
}

/** Reads the renderer's description of the run (`PerfApi.describe`). */
export const readRunDescription: perfSchema.Schema<PerfRunDescription> = object({
  scene: object({ version: positiveInt, seed: nonEmpty, world: nonEmpty }),
  settingValues: jsonObject,
  views: array(
    object({
      role: oneOf(VIEW_ROLES),
      style: oneOf(["wireframe", "photorealistic"]),
      widthPx: positiveInt,
      heightPx: positiveInt,
      rateHz: nonNegative,
      lineAntialiasing: nullable(oneOf(["analytic", "msaa4", "none"])),
      exposure: nullable(object({ mode: nonEmpty, evMin: num, evMax: num })),
    }),
  ),
  pacing: oneOf(["every-vsync", "every-second-vsync"]),
  repetitionSeconds: nNonNegative,
  heightWorkers: count,
  adapter: nullable(object({ vendor: str, architecture: str, description: str, fallback: bool })),
  capabilities: nullable(named(bool)),
  kernelPaths: nullable(named(str)),
});

/** Reads a warm-up sample (`PerfApi.warmUpSample`). */
export const readWarmUpSample: perfSchema.Schema<PerfWarmUpSample> = object({
  intervalsMs: array(nonNegative),
});

/** Reads a repetition's result (`PerfApi.endRepetition`). */
export const readRepetitionResult: perfSchema.Schema<PerfRepetitionResult> = object({
  spanS: nonNegative,
  timeOriginMs: nonNegative,
  frames: object({
    intervalsMs: array(nonNegative),
    callbackStartsMs: numbers,
    callbackMs: array(nonNegative),
    scriptTimesS: numbers,
  }),
  segments: array(object({ name: nonEmpty, startS: num, endS: num })),
  passes: array(
    object({
      key: narrowed<PassKey>(isPassKey, "a pass key"),
      row: budgetRow,
      frames: count,
      p50Ms: nNonNegative,
      p95Ms: nNonNegative,
      p99Ms: nNonNegative,
      ownP50Ms: nNonNegative,
      ownP95Ms: nNonNegative,
      ownP99Ms: nNonNegative,
    }),
  ),
  rows: array(
    object({
      row: budgetRow,
      frames: count,
      meanMs: nNonNegative,
      p50Ms: nNonNegative,
      p95Ms: nNonNegative,
      p99Ms: nNonNegative,
    }),
  ),
  timedPassesPerFrameMs: nNonNegative,
  timer: oneOf(["quantized", "full", "absent"]),
  memory: object({
    itemPeakBytes: keyed(isMemoryItem, "a memory item", count),
    tallyPeakBytes: nullable(count),
    workerFieldBytes: array(count),
    workerWasmBytes: array(count),
  }),
  releases: array(object({ atEpochMs: nonNegative, bytes: count })),
  streaming: array(
    object({
      segment: nonEmpty,
      patchesPerS: nNonNegative,
      demandPerS: nNonNegative,
      streamingSeconds: nNonNegative,
    }),
  ),
  latePipelines: count,
  heightPointUs: nNonNegative,
  internalScale: array(object({ min: fraction, median: fraction })),
  arrival: nullable(
    object({
      fieldBytes: count,
      fieldTransferMs: nonNegative,
      skyBytes: count,
      skyTransferMs: nonNegative,
    }),
  ),
});

/** Reads how a run ends (`PerfApi.finish`). */
export function readPerfEnd(value: unknown): PerfEnd {
  const status: unknown =
    typeof value === "object" && value !== null ? Reflect.get(value, "status") : undefined;
  return status === "complete"
    ? object<PerfEnd & { readonly status: "complete" }>({
        status: oneOf(["complete"]),
        notes: str,
      })(value, "")
    : object<PerfEnd & { readonly status: "failed" }>({
        status: oneOf(["failed"]),
        reason: nonEmpty,
      })(value, "");
}

/** A repetition's number: a whole number from 1. */
const repetitionNumber = positiveInt;

/** What the runner does for each call, once its sender and arguments are checked. */
export interface PerfOperations {
  config(): Promise<unknown>;
  describe(description: PerfRunDescription): Promise<void>;
  warmUpSample(sample: PerfWarmUpSample): Promise<unknown>;
  beginRepetition(n: number): Promise<unknown>;
  endRepetition(n: number, result: PerfRepetitionResult): Promise<void>;
  finish(end: PerfEnd): Promise<void>;
}

/** What the handlers need of the main process. */
export interface PerfHandlerDeps<E> {
  /** `ipcMain.handle`. */
  readonly handle: (
    channel: string,
    listener: (event: E, ...args: unknown[]) => Promise<unknown>,
  ) => void;
  /** Whether a call came from the run window's own page (`isOwnPage`). */
  readonly isSender: (event: E) => boolean;
  readonly operations: PerfOperations;
}

/** A call the main process refuses: not the run window's page, or an argument of no shape. */
export class PerfCallRefused extends Error {
  override readonly name = "PerfCallRefused";
}

/** Reads `value` by `schema`, refusing `channel`'s call where it does not fit. */
function argument<T>(schema: perfSchema.Schema<T>, value: unknown, channel: string): T {
  try {
    return schema(value, "");
  } catch (error: unknown) {
    if (error instanceof PerfRecordError) {
      throw new PerfCallRefused(`${channel} refused: ${error.message}`, { cause: error });
    }
    throw error;
  }
}

/** Registers every perf channel's handler. */
export function registerPerfHandlers<E>(deps: PerfHandlerDeps<E>): void {
  const { operations } = deps;
  const guarded = (
    channel: string,
    run: (args: ReadonlyArray<unknown>) => Promise<unknown>,
  ): void => {
    deps.handle(channel, async (event, ...args) => {
      if (!deps.isSender(event)) {
        throw new PerfCallRefused(`${channel} refused: not the run window's own page`);
      }
      return run(args);
    });
  };
  guarded(PERF_CHANNELS.config, () => operations.config());
  guarded(PERF_CHANNELS.describe, ([value]) =>
    operations.describe(argument(readRunDescription, value, PERF_CHANNELS.describe)),
  );
  guarded(PERF_CHANNELS.warmUpSample, ([value]) =>
    operations.warmUpSample(argument(readWarmUpSample, value, PERF_CHANNELS.warmUpSample)),
  );
  guarded(PERF_CHANNELS.beginRepetition, ([n]) =>
    operations.beginRepetition(argument(repetitionNumber, n, PERF_CHANNELS.beginRepetition)),
  );
  guarded(PERF_CHANNELS.endRepetition, ([n, value]) =>
    operations.endRepetition(
      argument(repetitionNumber, n, PERF_CHANNELS.endRepetition),
      argument(readRepetitionResult, value, PERF_CHANNELS.endRepetition),
    ),
  );
  guarded(PERF_CHANNELS.finish, ([value]) =>
    operations.finish(argument(readPerfEnd, value, PERF_CHANNELS.finish)),
  );
}
