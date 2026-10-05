/**
 * The several-views check's main-process side (plan R07, T20): its IPC handlers, a trace window a
 * phase, the captures, the question put to the person at a shown run, and the results file.
 *
 * @remarks
 * As the descent spike's (`spike.ts`, `spikeSession.ts`): nothing here runs on an ordinary
 * launch, every call is checked against the check window's own page, and the measurement switch
 * (`--hyperion-gpu-timing`'s lifting of the timestamp quantum) is applied only on the check's
 * launch. Each phase is one trace window, started at its measured window and stopped at its end,
 * reduced by R05's reducer for the presentations and the GPU process's main thread, and scanned
 * for the GPU process's slices named as copies; the trace file is removed once read.
 */

import type { ContentTracing, IpcMainInvokeEvent, TraceConfig } from "electron";

import type {
  SpikeResultsPaths,
  ViewsCheckLaunch,
  ViewsCheckPhaseName,
  ViewsCheckRecord,
  ViewsCheckWindow,
} from "../preload/api";
import { type Measured, measured, missing } from "./measured";
import {
  GPU_PROCESS_SLICES,
  readTraceEvents,
  TraceReducer,
  type TraceFigures,
} from "./reduceTrace";
import { readSpikeEnd } from "./spike";
import { readViewsCheckRecord } from "./viewsCheckRecord";
import {
  buildViewsCheckResults,
  type NamedSlice,
  NO_WINDOW_REASON,
  type PhaseTraceFigures,
  type ViewsCheckResults,
  type ViewsCheckRunInput,
  writeViewsCheckResults,
} from "./viewsCheckResults";

/** The check's IPC channels, one per operation; no channel name crosses the bridge. */
export const VIEWS_CHECK_CHANNELS = {
  startPhase: "hyperion:views-check:start-phase",
  endPhase: "hyperion:views-check:end-phase",
  askRightWayUp: "hyperion:views-check:ask-right-way-up",
  writeResults: "hyperion:views-check:write-results",
  end: "hyperion:views-check:end",
} as const;

/**
 * The check's trace categories: the five of R05's timed runs (decision-r05-trace-windows-2.md,
 * item 2), without `gpu` and V8's profiler. They keep the compositor's frames, the GPU process's
 * busy time and its `GPUTask` slices, and the page's `performance.measure` spans, of which the
 * window's sets the trace's clock against the page's.
 */
export const VIEWS_CHECK_TRACE_CATEGORIES: ReadonlyArray<string> = [
  "devtools.timeline",
  "disabled-by-default-devtools.timeline",
  "disabled-by-default-devtools.timeline.frame",
  "disabled-by-default-v8.gc",
  "blink.user_timing",
];

/**
 * A phase's trace buffer, 512 MiB: a 20 s window of these categories is tens of megabytes; the
 * buffer stops recording when full rather than dropping the window's start.
 */
export const VIEWS_CHECK_TRACE_BUFFER_KB = 512 * 1024;

/** The tracing configuration of a phase's window. */
export function viewsCheckTraceConfig(): TraceConfig {
  return {
    recording_mode: "record-until-full",
    trace_buffer_size_in_kb: VIEWS_CHECK_TRACE_BUFFER_KB,
    included_categories: [...VIEWS_CHECK_TRACE_CATEGORIES],
    excluded_categories: ["*"],
  };
}

/** A slice's name that says it copies. */
const COPY_SLICE = /copy|blit/iu;

/** The page's console warnings of R01's pass timer (`view/engine/webgpu/timing.ts`). */
const PASS_TIMER_DROP = /pass times are still being read; dropping some/u;
const UNTIMED_PASSES = /passes in a frame; the rest are not timed/u;

/** A half-open span of the trace's clock, µs. */
type Span = readonly [startUs: number, endUs: number];

/** The length of `spans` within [fromUs, toUs], overlaps counted once, µs. */
function clippedUnionUs(spans: ReadonlyArray<Span>, fromUs: number, toUs: number): number {
  const clipped = spans
    .map(([start, end]): Span => [Math.max(start, fromUs), Math.min(end, toUs)])
    .filter(([start, end]) => end > start)
    .toSorted((a, b) => a[0] - b[0]);
  let total = 0;
  let open: [number, number] | null = null;
  for (const [start, end] of clipped) {
    if (open !== null && start <= open[1]) {
      open[1] = Math.max(open[1], end);
      continue;
    }
    if (open !== null) {
      total += open[1] - open[0];
    }
    open = [start, end];
  }
  return open === null ? total : total + open[1] - open[0];
}

/**
 * The spans of a trace that the check reads within its measured window: every thread's `RunTask`
 * slices (its busy time, as R05's reducer takes it), and every process's slices of R05's GPU
 * names (`GPU_PROCESS_SLICES`) and those named as copies, a begin without its end as an instant.
 */
export class WindowScan {
  readonly #tasks = new Map<string, Span[]>();
  readonly #slices = new Map<string, Span[]>();

  /** Adds one parsed trace event; any other kind of event is ignored. */
  add(raw: unknown): void {
    if (typeof raw !== "object" || raw === null) {
      return;
    }
    const name: unknown = Reflect.get(raw, "name");
    const ph: unknown = Reflect.get(raw, "ph");
    const pid: unknown = Reflect.get(raw, "pid");
    const tid: unknown = Reflect.get(raw, "tid");
    const ts: unknown = Reflect.get(raw, "ts");
    const dur: unknown = Reflect.get(raw, "dur");
    if (
      typeof name !== "string" ||
      typeof pid !== "number" ||
      typeof tid !== "number" ||
      typeof ts !== "number"
    ) {
      return;
    }
    const span: Span | null =
      ph === "X" && typeof dur === "number" ? [ts, ts + dur] : ph === "B" ? [ts, ts] : null;
    if (span === null) {
      return;
    }
    if (name === "RunTask" && ph === "X") {
      const key = `${String(pid)}:${String(tid)}`;
      const tasks = this.#tasks.get(key) ?? [];
      tasks.push(span);
      this.#tasks.set(key, tasks);
    }
    if (GPU_PROCESS_SLICES.includes(name) || COPY_SLICE.test(name)) {
      const key = `${String(pid)}\u0000${name}`;
      const slices = this.#slices.get(key) ?? [];
      slices.push(span);
      this.#slices.set(key, slices);
    }
  }

  /** A thread's busy time within [fromUs, toUs], ms. */
  busyMs(pid: number, tid: number, fromUs: number, toUs: number): number {
    return (
      clippedUnionUs(this.#tasks.get(`${String(pid)}:${String(tid)}`) ?? [], fromUs, toUs) / 1000
    );
  }

  /** A process's slices whose names pass `named`, within [fromUs, toUs], by name. */
  slices(
    pid: number,
    fromUs: number,
    toUs: number,
    named: (name: string) => boolean,
  ): ReadonlyArray<NamedSlice> {
    const prefix = `${String(pid)}\u0000`;
    return [...this.#slices]
      .flatMap(([key, spans]) => {
        const name = key.slice(prefix.length);
        if (!key.startsWith(prefix) || !named(name)) {
          return [];
        }
        const inside = spans.filter(([start, end]) => end >= fromUs && start <= toUs);
        return inside.length === 0
          ? []
          : [{ name, count: inside.length, totalMs: clippedUnionUs(inside, fromUs, toUs) / 1000 }];
      })
      .toSorted((a, b) => (a.name < b.name ? -1 : 1));
  }
}

/** A trace window read: R05's figures and the spans the check clips to its measured window. */
export interface ReducedWindow {
  readonly figures: TraceFigures;
  readonly scan: WindowScan;
}

/** Reads a trace file once, through R05's reducer and the check's scan. */
export async function reduceWindowFile(path: string): Promise<ReducedWindow> {
  const reducer = new TraceReducer();
  const scan = new WindowScan();
  for await (const event of readTraceEvents(path)) {
    reducer.add(event);
    scan.add(event);
  }
  return { figures: reducer.figures(), scan };
}

/** The intervals between consecutive times of `atUs` within [fromUs, toUs], ms. */
function intervalsWithin(atUs: ReadonlyArray<number>, fromUs: number, toUs: number): number[] {
  const inside = atUs.filter((us) => us >= fromUs && us <= toUs);
  return inside.slice(1).map((us, i) => (us - (inside[i] ?? us)) / 1000);
}

/**
 * A reduced window as a phase's figures, each clipped to the measured `window`: the page's clock
 * set against the trace's by the reducer's `clockOffsetUs` (the page's `performance.measure`
 * spans). A hidden run has no presentation.
 */
export function phaseTraceFigures(
  reduced: ReducedWindow,
  shown: boolean,
  window: ViewsCheckWindow,
): PhaseTraceFigures {
  const { frames, gpuProcess, clockOffsetUs } = reduced.figures;
  if (clockOffsetUs === null) {
    const reason = "the trace holds no span of the page's to set its clock by";
    return { presentation: missing(reason), gpuProcess: missing(reason) };
  }
  const fromUs = clockOffsetUs + 1000 * window.startMs;
  const toUs = clockOffsetUs + 1000 * window.endMs;
  return {
    presentation: !shown
      ? missing(NO_WINDOW_REASON)
      : frames.pid === null
        ? missing("the trace has no compositor frame")
        : measured({
            intervalsMs: intervalsWithin(frames.presentedAtUs, fromUs, toUs),
            dropped: frames.droppedAtUs.filter((us) => us >= fromUs && us <= toUs).length,
          }),
    gpuProcess:
      gpuProcess === null
        ? missing("the trace has no GPU process")
        : measured({
            busyMs: reduced.scan.busyMs(gpuProcess.pid, gpuProcess.tid, fromUs, toUs),
            slices: reduced.scan.slices(gpuProcess.pid, fromUs, toUs, (name) =>
              GPU_PROCESS_SLICES.includes(name),
            ),
            copySlices: reduced.scan.slices(gpuProcess.pid, fromUs, toUs, (name) =>
              COPY_SLICE.test(name),
            ),
          }),
  };
}

/** What a run's session needs of Electron and the file system, injected for tests. */
export interface ViewsCheckSessionDeps {
  readonly launch: ViewsCheckLaunch;
  /** The run's description, read once at its start (the load average then). */
  readonly describe: () => Promise<Omit<ViewsCheckRunInput, "captures" | "window">>;
  /** The window's content size as it stands, DIP: read when the results are written. */
  readonly windowSize: () => ViewsCheckRunInput["window"];
  readonly tracing: Pick<ContentTracing, "startRecording" | "stopRecording">;
  /** Where a phase's trace is written. */
  readonly tracePath: (phase: ViewsCheckPhaseName) => string;
  readonly reduce: (path: string) => Promise<ReducedWindow>;
  readonly removeFile: (path: string) => Promise<void>;
  /** Where the captures go, or `null` for none. */
  readonly capturesDir: string | null;
  /** Saves the page as a PNG at `path`. */
  readonly capture: (path: string) => Promise<void>;
  /** Asks the person whether every view is the right way up; `null` when unanswered. */
  readonly ask: (() => Promise<"yes" | "no" | null>) | null;
  readonly outDir: string;
  /** Writes the results into a directory: {@link writeViewsCheckResults} by default. */
  readonly write?: (
    dir: string,
    results: ViewsCheckResults,
  ) => Promise<{ readonly json: string; readonly markdown: string }>;
  readonly exit: (code: number) => void;
  readonly log: (line: string) => void;
}

/** One run's state in the main process, from its start to its results. */
export class ViewsCheckSession {
  readonly #deps: ViewsCheckSessionDeps;
  readonly #run: Promise<Omit<ViewsCheckRunInput, "captures" | "window">>;
  readonly #traces = new Map<ViewsCheckPhaseName, PhaseTraceFigures>();
  #recording: ViewsCheckPhaseName | null = null;
  #captured = false;
  #rightWayUp: Measured<"yes" | "no">;
  #passTimerDrops = 0;
  #untimedPasses = 0;
  #gpuProcessExits = 0;
  #ended = false;

  constructor(deps: ViewsCheckSessionDeps) {
    this.#deps = deps;
    // Read at the start, for the load average then; a failure surfaces where it is awaited.
    this.#run = deps.describe();
    this.#run.catch(() => undefined);
    this.#rightWayUp = missing(
      deps.ask === null ? "a hidden run: nobody to ask; see the captures" : "not asked",
    );
  }

  /** Whether the run has ended through {@link end}. */
  get ended(): boolean {
    return this.#ended;
  }

  /** Counts a console message of the page that is one of R01's pass-timer warnings. */
  consoleMessage(message: string): void {
    if (PASS_TIMER_DROP.test(message)) {
      this.#passTimerDrops += 1;
    }
    if (UNTIMED_PASSES.test(message)) {
      this.#untimedPasses += 1;
    }
  }

  /** Counts an exit of the GPU process. */
  gpuProcessGone(): void {
    this.#gpuProcessExits += 1;
  }

  /**
   * Starts a phase's trace window.
   *
   * @throws Error if another phase's window is still recording.
   */
  async startPhase(name: ViewsCheckPhaseName): Promise<void> {
    if (this.#recording !== null) {
      throw new Error(`the trace of ${this.#recording} is still recording`);
    }
    this.#recording = name;
    try {
      await this.#deps.tracing.startRecording(viewsCheckTraceConfig());
    } catch (error: unknown) {
      this.#recording = null;
      throw new Error(`the trace of ${name} did not start`, { cause: error });
    }
  }

  /**
   * Ends a phase's window: its trace stopped, reduced over the measured `window` and removed, and
   * the page captured.
   *
   * @throws Error if `name` is not the phase recording.
   */
  async endPhase(name: ViewsCheckPhaseName, window: ViewsCheckWindow): Promise<void> {
    if (this.#recording !== name) {
      throw new Error(`${name} is not the phase recording`);
    }
    this.#recording = null;
    const path = await this.#deps.tracing.stopRecording(this.#deps.tracePath(name));
    const shown = (await this.#run).shown;
    try {
      this.#traces.set(name, phaseTraceFigures(await this.#deps.reduce(path), shown, window));
    } catch (error: unknown) {
      const reason = `the trace did not reduce: ${error instanceof Error ? error.message : String(error)}`;
      this.#traces.set(name, { presentation: missing(reason), gpuProcess: missing(reason) });
    } finally {
      await this.#deps.removeFile(path).catch((error: unknown) => {
        this.#deps.log(`views check: the trace ${path} was not removed: ${String(error)}`);
      });
    }
    const dir = this.#deps.capturesDir;
    if (dir !== null) {
      try {
        await this.#deps.capture(`${dir}/${name}.png`);
        this.#captured = true;
      } catch (error: unknown) {
        this.#deps.log(`views check: the capture of ${name} failed: ${String(error)}`);
      }
    }
  }

  /** Asks the person at a shown run, and keeps the answer. */
  async askRightWayUp(): Promise<void> {
    const ask = this.#deps.ask;
    if (ask === null) {
      return;
    }
    const answer = await ask();
    this.#rightWayUp = answer === null ? missing("not answered") : measured(answer);
  }

  /** Builds the results from the renderer's record and writes them. */
  async writeResults(record: ViewsCheckRecord): Promise<SpikeResultsPaths> {
    const run = await this.#run;
    const dir = this.#deps.capturesDir;
    const results = buildViewsCheckResults({
      run: {
        ...run,
        window: this.#deps.windowSize(),
        captures:
          dir === null
            ? missing("no capture directory")
            : this.#captured
              ? measured(dir)
              : missing("no capture was saved"),
      },
      record,
      traces: this.#traces,
      warnings: { passTimerDrops: this.#passTimerDrops, untimedPasses: this.#untimedPasses },
      gpuProcessExits: this.#gpuProcessExits,
      rightWayUp: this.#rightWayUp,
    });
    const write = this.#deps.write ?? writeViewsCheckResults;
    const paths = await write(this.#deps.outDir, results);
    this.#deps.log(`views check: wrote ${paths.json} and ${paths.markdown}`);
    return paths;
  }

  /** Ends the run with an exit status, and a reason for a failure. */
  end(code: number, reason: string | null): void {
    this.#ended = true;
    this.#deps.log(
      code === 0 ? "views check: PASS the run is recorded" : `views check: FAIL ${reason ?? ""}`,
    );
    this.#deps.exit(code);
  }
}

/** Thrown back to the renderer for a call the main process refuses. */
export class ViewsCheckCallRefused extends Error {}

/** What the handlers need: Electron's registration and the session's operations. */
export interface ViewsCheckHandlerDeps<E = IpcMainInvokeEvent> {
  /** Registers a handler (`ipcMain.handle`). */
  readonly handle: (
    channel: string,
    listener: (event: E, ...args: unknown[]) => Promise<unknown>,
  ) => void;
  /** Whether an event comes from the check window's own page (`isOwnPage`). */
  readonly isSender: (event: E) => boolean;
  readonly session: Pick<
    ViewsCheckSession,
    "startPhase" | "endPhase" | "askRightWayUp" | "writeResults" | "end"
  >;
}

const PHASE_NAMES: ReadonlyArray<ViewsCheckPhaseName> = [
  "photoreal-alone",
  "photoreal-two-wireframe",
  "wireframe-two-wireframe",
  "wireframe-photoreal-wireframe",
  "wireframe-alone",
];

function readPhaseName(value: unknown): ViewsCheckPhaseName | null {
  return PHASE_NAMES.find((name) => name === value) ?? null;
}

/** `value` as a measured window, or `null`: two finite times, the end not before the start. */
function readWindow(value: unknown): ViewsCheckWindow | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }
  const startMs: unknown = Reflect.get(value, "startMs");
  const endMs: unknown = Reflect.get(value, "endMs");
  return typeof startMs === "number" &&
    typeof endMs === "number" &&
    Number.isFinite(startMs) &&
    Number.isFinite(endMs) &&
    endMs >= startMs
    ? { startMs, endMs }
    : null;
}

/**
 * Registers the check's handlers, each refusing a sender other than the check window's own page
 * and arguments that do not check, by rejecting the call.
 */
export function registerViewsCheckHandlers<E>(deps: ViewsCheckHandlerDeps<E>): void {
  const guarded = (
    channel: string,
    run: (args: ReadonlyArray<unknown>) => Promise<unknown>,
  ): void => {
    deps.handle(channel, async (event, ...args) => {
      if (!deps.isSender(event)) {
        throw new ViewsCheckCallRefused(`${channel} refused: not the check window's own page`);
      }
      return run(args);
    });
  };
  guarded(VIEWS_CHECK_CHANNELS.startPhase, async ([value]) => {
    const name = readPhaseName(value);
    if (name === null) {
      throw new ViewsCheckCallRefused(`${VIEWS_CHECK_CHANNELS.startPhase} refused: not a phase`);
    }
    await deps.session.startPhase(name);
  });
  guarded(VIEWS_CHECK_CHANNELS.endPhase, async ([value, span]) => {
    const name = readPhaseName(value);
    const window = readWindow(span);
    if (name === null || window === null) {
      throw new ViewsCheckCallRefused(
        `${VIEWS_CHECK_CHANNELS.endPhase} refused: not a phase and its window`,
      );
    }
    await deps.session.endPhase(name, window);
  });
  guarded(VIEWS_CHECK_CHANNELS.askRightWayUp, () => deps.session.askRightWayUp());
  guarded(VIEWS_CHECK_CHANNELS.writeResults, async ([value]) => {
    const record = readViewsCheckRecord(value);
    if (record === null) {
      throw new ViewsCheckCallRefused(
        `${VIEWS_CHECK_CHANNELS.writeResults} refused: not the check's record`,
      );
    }
    return deps.session.writeResults(record);
  });
  guarded(VIEWS_CHECK_CHANNELS.end, async ([value]) => {
    const end = readSpikeEnd(value);
    if (end === null) {
      throw new ViewsCheckCallRefused(`${VIEWS_CHECK_CHANNELS.end} refused: not an end of the run`);
    }
    deps.session.end(end.status === "pass" ? 0 : 1, end.status === "pass" ? null : end.reason);
  });
}
