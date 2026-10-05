/**
 * The trace reducer of the descent spike (plan R05, T14.b): Chromium's JSON trace in, the frame,
 * GPU-process, main-thread and GC figures of Design note 18 out.
 *
 * @remarks
 * A full descent's trace is about a gigabyte, beyond what one `JSON.parse` of the file can hold, so
 * {@link readTraceEvents} streams it a line at a time, relying on the layout Chromium writes: a
 * `{"traceEvents":[` line, one event a line, and the last event's line ending `],"metadata":`.
 * {@link TraceReducer} folds events in any order and summarises at the end.
 *
 * The events read, as recorded from Electron 44.4.3 on 2026-10-02 (the test's fixture):
 *
 * - frames: the compositor's `PipelineReporter` async slices in the renderer, whose end is the
 *   frame's presentation and whose `args.frame_reporter.state` says whether it was presented
 *   (`STATE_PRESENTED_ALL`, `STATE_PRESENTED_PARTIAL`), dropped (`STATE_DROPPED`) or not wanted;
 * - thread busy time: the timeline's `RunTask` slices, merged where they nest;
 * - the GPU process: its `CrGpuMain` thread's tasks and its `WebGPU`, `GPUTask` and
 *   `VulkanQueueSubmitHook` slices, the CPU side of Chromium's command transport and Dawn;
 * - GC: every complete slice in a `v8.gc` category, and `MinorGC` and `MajorGC`, merged per thread
 *   so that nested phases count once;
 * - `performance.measure` spans: `blink.user_timing` async begin and end pairs, whose begins also
 *   carry the span's start in `performance.now()` ms (`args.startTime`), which sets the trace's
 *   clock against the page's ({@link TraceFigures.clockOffsetUs});
 * - the engine adapter's self time: V8's CPU profile chunks of the renderer's main thread, whose
 *   samples at a leaf in the lazily imported `engine-*.js` chunk are counted.
 *
 * GPU time per pass is not here: it comes from R01's `onPassTimes` in the renderer (T14.a).
 */

import { createReadStream } from "node:fs";
import { readFile } from "node:fs/promises";
import { createInterface } from "node:readline";

/** Matches the URL of the renderer's lazily imported engine chunk, `engine-<hash>.js`. */
export const ENGINE_CHUNK_PATTERN = /\/engine-[\w-]+\.js$/;

/** The GPU process's slices summarised by name. */
export const GPU_PROCESS_SLICES: ReadonlyArray<string> = [
  "WebGPU",
  "GPUTask",
  "VulkanQueueSubmitHook",
];

/** Count, sum and largest of a set of durations. */
export interface DurationSummary {
  readonly count: number;
  readonly totalMs: number;
  readonly maxMs: number;
}

/** The renderer's compositor frames. */
export interface FrameFigures {
  /** The renderer process the frames are from, or `null` when the trace has none. */
  readonly pid: number | null;
  /**
   * The compositor (`layer_tree_host_id`) whose frames these are: the renderer's busiest, since a
   * renderer may run more than one and each reports every frame it begins.
   */
  readonly layerTreeHostId: number | null;
  /**
   * Presentation times of the presented frames, in the trace's clock, µs, ascending, one a
   * presentation.
   */
  readonly presentedAtUs: ReadonlyArray<number>;
  /** Intervals between consecutive presentations, ms. */
  readonly intervalsMs: ReadonlyArray<number>;
  readonly presented: number;
  readonly dropped: number;
  /** The end of each dropped frame, in the trace's clock, µs, ascending, one a dropped frame. */
  readonly droppedAtUs: ReadonlyArray<number>;
  /** Frames the compositor began and did not need to draw, as a hidden window's are. */
  readonly noUpdate: number;
}

/** One thread's busy time and GC pauses. */
export interface ThreadFigures {
  readonly pid: number;
  readonly tid: number;
  readonly process: string | null;
  readonly thread: string | null;
  /** The union of its `RunTask` slices, ms. */
  readonly busyMs: number;
  /** Its GC slices, nested phases merged into one pause. */
  readonly gc: DurationSummary;
}

/** One `performance.measure` name's spans on one thread. */
export interface UserTimingFigures extends DurationSummary {
  readonly name: string;
  readonly pid: number;
  readonly tid: number;
  /** Every span's start, in the trace's clock, µs, ascending. */
  readonly startsUs: ReadonlyArray<number>;
  /** Every span's duration, ms, in order of its start. */
  readonly durationsMs: ReadonlyArray<number>;
}

/** The renderer's main thread, split as Design note 18 asks. */
export interface MainThreadFigures {
  readonly pid: number;
  readonly tid: number;
  /** The trace's span, ms. */
  readonly wallMs: number;
  /** The union of its tasks, ms. */
  readonly busyMs: number;
  /** The union of its `performance.measure` spans: our own per-frame code, ms. */
  readonly ourCodeMs: number;
  /**
   * Sampled self time in the engine chunk, ms; `null` without a CPU profile of the thread. Where
   * our spans enclose engine calls it lies inside `ourCodeMs` as well.
   */
  readonly engineSelfMs: number | null;
  /** Sampled self time of the whole profile, ms; `null` without one. */
  readonly sampledMs: number | null;
  /** `wallMs` less `busyMs`. */
  readonly idleMs: number;
}

/** The GPU process's main thread. */
export interface GpuProcessFigures {
  readonly pid: number;
  readonly tid: number;
  readonly busyMs: number;
  /** {@link GPU_PROCESS_SLICES} by name, absent names with a count of 0. */
  readonly slices: ReadonlyArray<DurationSummary & { readonly name: string }>;
}

/** What a trace reduces to. */
export interface TraceFigures {
  /** The first and last event times, µs, or `null` for a trace with no timed event. */
  readonly span: { readonly firstUs: number; readonly lastUs: number } | null;
  /**
   * The trace's clock less the page's, µs: a trace time is `clockOffsetUs + 1000 × t` for a
   * `performance.now()` time t in ms. The median of `ts − 1000 × args.startTime` over the
   * `blink.user_timing` begins on the renderer's main thread, whose `performance.now()` the page's
   * own times are on (a worker's clock has its own origin); `null` without one.
   */
  readonly clockOffsetUs: number | null;
  readonly frames: FrameFigures;
  readonly mainThread: MainThreadFigures | null;
  readonly gpuProcess: GpuProcessFigures | null;
  /** Every thread with a task or a GC slice, by process and thread ID. */
  readonly threads: ReadonlyArray<ThreadFigures>;
  /** By process, thread and name. */
  readonly userTiming: ReadonlyArray<UserTimingFigures>;
}

/** Options of a reduction. */
export interface ReduceOptions {
  /**
   * Matches the engine chunk's script URL; {@link ENGINE_CHUNK_PATTERN} by default. A `g` or `y`
   * flag is dropped, since it would make each test start where the last one stopped.
   */
  readonly engineChunk?: RegExp;
}

/** The fields of a trace event the reducer reads, narrowed from JSON. */
interface TraceEvent {
  readonly name: string;
  readonly cat: string;
  readonly ph: string;
  readonly pid: number;
  readonly tid: number;
  readonly ts: number;
  readonly dur: number | undefined;
  /** `id`, or `id2.local`, or `id2.global`, for async events. */
  readonly id: string | undefined;
  readonly args: Readonly<Record<string, unknown>>;
}

/** A half-open interval in µs. */
type Interval = readonly [startUs: number, endUs: number];

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringOf(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function numberOf(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function asyncId(raw: Readonly<Record<string, unknown>>): string | undefined {
  const id = raw["id"];
  if (typeof id === "string" || typeof id === "number") {
    return String(id);
  }
  const id2 = raw["id2"];
  if (isRecord(id2)) {
    const local = stringOf(id2["local"]);
    if (local !== undefined) {
      return `local:${local}`;
    }
    const global = stringOf(id2["global"]);
    return global === undefined ? undefined : `global:${global}`;
  }
  return undefined;
}

/** A trace event, or `null` for anything that is not one. */
function toEvent(raw: unknown): TraceEvent | null {
  if (!isRecord(raw)) {
    return null;
  }
  const name = stringOf(raw["name"]);
  const ph = stringOf(raw["ph"]);
  const pid = numberOf(raw["pid"]);
  const tid = numberOf(raw["tid"]);
  if (name === undefined || ph === undefined || pid === undefined || tid === undefined) {
    return null;
  }
  const args = raw["args"];
  return {
    name,
    ph,
    pid,
    tid,
    cat: stringOf(raw["cat"]) ?? "",
    ts: numberOf(raw["ts"]) ?? 0,
    dur: numberOf(raw["dur"]),
    id: asyncId(raw),
    args: isRecord(args) ? args : {},
  };
}

/** The total length of a set of intervals, overlaps counted once, µs. */
function unionLengthUs(intervals: ReadonlyArray<Interval>): number {
  const sorted = intervals.toSorted((a, b) => a[0] - b[0]);
  let total = 0;
  let open: [number, number] | undefined;
  for (const [start, end] of sorted) {
    if (open !== undefined && start <= open[1]) {
      open[1] = Math.max(open[1], end);
      continue;
    }
    if (open !== undefined) {
      total += open[1] - open[0];
    }
    open = [start, end];
  }
  return open === undefined ? total : total + open[1] - open[0];
}

/** The merged intervals' lengths, overlaps joined: one pause for nested GC phases, µs. */
function mergedLengthsUs(intervals: ReadonlyArray<Interval>): number[] {
  const sorted = intervals.toSorted((a, b) => a[0] - b[0]);
  const lengths: number[] = [];
  let open: [number, number] | undefined;
  for (const [start, end] of sorted) {
    if (open !== undefined && start <= open[1]) {
      open[1] = Math.max(open[1], end);
      continue;
    }
    if (open !== undefined) {
      lengths.push(open[1] - open[0]);
    }
    open = [start, end];
  }
  if (open !== undefined) {
    lengths.push(open[1] - open[0]);
  }
  return lengths;
}

/** The median of values that are not empty: the mean of the middle two for an even count. */
function median(values: ReadonlyArray<number>): number | null {
  const sorted = values.toSorted((a, b) => a - b);
  const mid = sorted.length >> 1;
  const upper = sorted[mid];
  if (upper === undefined) {
    return null;
  }
  return sorted.length % 2 === 1 ? upper : ((sorted[mid - 1] ?? upper) + upper) / 2;
}

function summarise(durationsMs: ReadonlyArray<number>): DurationSummary {
  let totalMs = 0;
  let maxMs = 0;
  for (const ms of durationsMs) {
    totalMs += ms;
    maxMs = Math.max(maxMs, ms);
  }
  return { count: durationsMs.length, totalMs, maxMs };
}

function threadKey(pid: number, tid: number): string {
  return `${pid}:${tid}`;
}

function pushTo<K, V>(map: Map<K, V[]>, key: K, value: V): void {
  const list = map.get(key);
  if (list === undefined) {
    map.set(key, [value]);
  } else {
    list.push(value);
  }
}

function isGcSlice(event: TraceEvent): boolean {
  return (
    event.ph === "X" &&
    (event.cat.split(",").some((cat) => cat.endsWith("v8.gc")) ||
      event.name === "MinorGC" ||
      event.name === "MajorGC")
  );
}

/** One profile's state: its thread, its nodes' URLs, its last sample and its tallies. */
interface ProfileState {
  tid: number | undefined;
  readonly urls: Map<number, string>;
  lastNode: number | undefined;
  engineUs: number;
  sampledUs: number;
}

/** A pending async begin: its thread, time and the state a `PipelineReporter` carries. */
interface Begin {
  readonly tid: number;
  readonly ts: number;
  readonly state: string | undefined;
  readonly host: number | undefined;
}

/** A frame's end: its presentation time and state. */
type FrameEnd = readonly [endUs: number, state: string | undefined];

/**
 * Folds trace events into {@link TraceFigures}.
 *
 * @remarks
 * Events may come in any order: everything is kept as intervals and resolved in
 * {@link TraceReducer.figures}. The async pairs (frames, `performance.measure` spans) are matched
 * by process, ID and name, so a begin must precede its end in the stream, as Chromium writes them.
 */
export class TraceReducer {
  readonly #engineChunk: RegExp;
  readonly #threadNames = new Map<string, string>();
  readonly #processNames = new Map<number, string>();
  readonly #tasks = new Map<string, Interval[]>();
  readonly #gc = new Map<string, Interval[]>();
  readonly #gpuSlices = new Map<string, number[]>();
  readonly #begins = new Map<string, Begin>();
  /** Per process, per compositor: frame ends. */
  readonly #frames = new Map<number, Map<number, FrameEnd[]>>();
  readonly #userTiming = new Map<string, Array<readonly [number, number]>>();
  /** Per thread, each user-timing begin's `ts − 1000 × args.startTime`, µs. */
  readonly #clockOffsets = new Map<string, number[]>();
  readonly #profiles = new Map<string, ProfileState>();
  #firstUs = Number.POSITIVE_INFINITY;
  #lastUs = Number.NEGATIVE_INFINITY;

  constructor(options: ReduceOptions = {}) {
    const pattern = options.engineChunk ?? ENGINE_CHUNK_PATTERN;
    this.#engineChunk = new RegExp(pattern.source, pattern.flags.replaceAll(/[gy]/g, ""));
  }

  /** Adds one event, as parsed JSON; anything that is not an event is ignored. */
  add(raw: unknown): void {
    const event = toEvent(raw);
    if (event === null) {
      return;
    }
    if (event.ph === "M") {
      this.#metadata(event);
      return;
    }
    if (event.ts > 0) {
      this.#firstUs = Math.min(this.#firstUs, event.ts);
      this.#lastUs = Math.max(this.#lastUs, event.ts + (event.dur ?? 0));
    }
    const key = threadKey(event.pid, event.tid);
    if (event.ph === "X" && event.dur !== undefined) {
      const interval: Interval = [event.ts, event.ts + event.dur];
      if (event.name === "RunTask") {
        pushTo(this.#tasks, key, interval);
      }
      if (isGcSlice(event)) {
        pushTo(this.#gc, key, interval);
      }
      if (GPU_PROCESS_SLICES.includes(event.name)) {
        pushTo(this.#gpuSlices, `${key}:${event.name}`, event.dur / 1000);
      }
      return;
    }
    if (event.ph === "P") {
      this.#profile(event);
      return;
    }
    if (event.ph === "b" || event.ph === "e") {
      this.#async(event);
    }
  }

  #metadata(event: TraceEvent): void {
    const name = stringOf(event.args["name"]);
    if (name === undefined) {
      return;
    }
    if (event.name === "thread_name") {
      this.#threadNames.set(threadKey(event.pid, event.tid), name);
    } else if (event.name === "process_name") {
      this.#processNames.set(event.pid, name);
    }
  }

  #async(event: TraceEvent): void {
    const isFrame = event.name === "PipelineReporter";
    const isMeasure = event.cat === "blink.user_timing";
    if ((!isFrame && !isMeasure) || event.id === undefined) {
      return;
    }
    const key = `${event.pid}:${event.id}:${event.name}`;
    if (event.ph === "b") {
      const reporter = event.args["frame_reporter"];
      const state = isRecord(reporter) ? stringOf(reporter["state"]) : undefined;
      const host = isRecord(reporter) ? numberOf(reporter["layer_tree_host_id"]) : undefined;
      this.#begins.set(key, { tid: event.tid, ts: event.ts, state, host });
      const startMs = isMeasure ? numberOf(event.args["startTime"]) : undefined;
      if (startMs !== undefined) {
        pushTo(this.#clockOffsets, threadKey(event.pid, event.tid), event.ts - 1000 * startMs);
      }
      return;
    }
    const begin = this.#begins.get(key);
    if (begin === undefined) {
      return;
    }
    this.#begins.delete(key);
    if (isFrame) {
      let hosts = this.#frames.get(event.pid);
      if (hosts === undefined) {
        hosts = new Map();
        this.#frames.set(event.pid, hosts);
      }
      pushTo(hosts, begin.host ?? -1, [event.ts, begin.state] as const);
    } else {
      pushTo(this.#userTiming, `${event.pid}\u0000${begin.tid}\u0000${event.name}`, [
        begin.ts,
        event.ts,
      ] as const);
    }
  }

  #profile(event: TraceEvent): void {
    if (event.id === undefined) {
      return;
    }
    const key = `${event.pid}:${event.id}`;
    let state = this.#profiles.get(key);
    if (state === undefined) {
      state = { tid: undefined, urls: new Map(), lastNode: undefined, engineUs: 0, sampledUs: 0 };
      this.#profiles.set(key, state);
    }
    if (event.name === "Profile") {
      // The profile's start is written on the profiled thread; its chunks on V8's profiler thread.
      state.tid = event.tid;
      return;
    }
    if (event.name !== "ProfileChunk") {
      return;
    }
    const data = event.args["data"];
    if (!isRecord(data)) {
      return;
    }
    const profile = data["cpuProfile"];
    const nodes = isRecord(profile) ? profile["nodes"] : undefined;
    for (const node of Array.isArray(nodes) ? nodes : []) {
      const id = isRecord(node) ? numberOf(node["id"]) : undefined;
      const frame = isRecord(node) ? node["callFrame"] : undefined;
      if (id !== undefined && isRecord(frame)) {
        state.urls.set(id, stringOf(frame["url"]) ?? "");
      }
    }
    const samples = isRecord(profile) ? profile["samples"] : undefined;
    const deltas = data["timeDeltas"];
    if (!Array.isArray(samples) || !Array.isArray(deltas)) {
      return;
    }
    for (let i = 0; i < samples.length; i += 1) {
      // A sample's time runs until the next one, so each delta belongs to the sample before it.
      const deltaUs = numberOf(deltas[i]) ?? 0;
      if (state.lastNode !== undefined && deltaUs > 0) {
        state.sampledUs += deltaUs;
        if (this.#engineChunk.test(state.urls.get(state.lastNode) ?? "")) {
          state.engineUs += deltaUs;
        }
      }
      state.lastNode = numberOf(samples[i]);
    }
  }

  /** The figures of every event added so far. */
  figures(): TraceFigures {
    const span =
      this.#firstUs <= this.#lastUs ? { firstUs: this.#firstUs, lastUs: this.#lastUs } : null;
    const rendererPid = this.#rendererPid();
    const mainTid =
      rendererPid === null ? undefined : this.#threadOf(rendererPid, "CrRendererMain");
    const offsets =
      rendererPid === null || mainTid === undefined
        ? undefined
        : this.#clockOffsets.get(threadKey(rendererPid, mainTid));
    return {
      span,
      clockOffsetUs: offsets === undefined ? null : median(offsets),
      frames: this.#frameFigures(rendererPid),
      mainThread: this.#mainThread(rendererPid, span),
      gpuProcess: this.#gpuProcess(),
      threads: this.#threadFigures(),
      userTiming: this.#userTimingFigures(),
    };
  }

  #threadOf(pid: number, name: string): number | undefined {
    for (const [key, thread] of this.#threadNames) {
      const [p, t] = key.split(":").map(Number);
      if (p === pid && thread === name && t !== undefined) {
        return t;
      }
    }
    return undefined;
  }

  /** The renderer with the most compositor frames, then the lowest process ID. */
  #rendererPid(): number | null {
    let best: number | null = null;
    let bestFrames = -1;
    const renderers = [...this.#processNames]
      .filter(([, name]) => name === "Renderer")
      .map(([pid]) => pid)
      .toSorted((a, b) => a - b);
    for (const pid of renderers) {
      let frames = 0;
      for (const ends of this.#frames.get(pid)?.values() ?? []) {
        frames += ends.length;
      }
      if (frames > bestFrames) {
        best = pid;
        bestFrames = frames;
      }
    }
    return best;
  }

  #frameFigures(pid: number | null): FrameFigures {
    let host: number | null = null;
    let ends: ReadonlyArray<FrameEnd> = [];
    const hosts = [...(pid === null ? [] : (this.#frames.get(pid) ?? []))].toSorted(
      (a, b) => a[0] - b[0],
    );
    for (const [id, hostEnds] of hosts) {
      if (hostEnds.length > ends.length) {
        host = id;
        ends = hostEnds;
      }
    }
    const presented = new Set<number>();
    const droppedAtUs: number[] = [];
    let noUpdate = 0;
    for (const [ts, state] of ends) {
      if (state === "STATE_PRESENTED_ALL" || state === "STATE_PRESENTED_PARTIAL") {
        presented.add(ts);
      } else if (state === "STATE_DROPPED") {
        droppedAtUs.push(ts);
      } else {
        noUpdate += 1;
      }
    }
    const presentedAtUs = [...presented].toSorted((a, b) => a - b);
    const intervalsMs = presentedAtUs
      .slice(1)
      .map((ts, i) => (ts - (presentedAtUs[i] ?? ts)) / 1000);
    return {
      pid,
      layerTreeHostId: host === -1 ? null : host,
      presentedAtUs,
      intervalsMs,
      presented: presentedAtUs.length,
      dropped: droppedAtUs.length,
      droppedAtUs: droppedAtUs.toSorted((a, b) => a - b),
      noUpdate,
    };
  }

  #mainThread(pid: number | null, span: TraceFigures["span"]): MainThreadFigures | null {
    const tid = pid === null ? undefined : this.#threadOf(pid, "CrRendererMain");
    if (pid === null || tid === undefined) {
      return null;
    }
    const key = threadKey(pid, tid);
    const wallMs = span === null ? 0 : (span.lastUs - span.firstUs) / 1000;
    const busyMs = unionLengthUs(this.#tasks.get(key) ?? []) / 1000;
    const spans: Interval[] = [];
    for (const [userKey, pairs] of this.#userTiming) {
      const [p, t] = userKey.split("\u0000");
      if (Number(p) === pid && Number(t) === tid) {
        spans.push(...pairs);
      }
    }
    const profile = [...this.#profiles]
      .filter(([profileKey, state]) => profileKey.startsWith(`${pid}:`) && state.tid === tid)
      .map(([, state]) => state)[0];
    return {
      pid,
      tid,
      wallMs,
      busyMs,
      ourCodeMs: unionLengthUs(spans) / 1000,
      engineSelfMs: profile === undefined ? null : profile.engineUs / 1000,
      sampledMs: profile === undefined ? null : profile.sampledUs / 1000,
      idleMs: Math.max(0, wallMs - busyMs),
    };
  }

  #gpuProcess(): GpuProcessFigures | null {
    const pid = [...this.#processNames].find(([, name]) => name === "GPU Process")?.[0];
    const tid = pid === undefined ? undefined : this.#threadOf(pid, "CrGpuMain");
    if (pid === undefined || tid === undefined) {
      return null;
    }
    const key = threadKey(pid, tid);
    return {
      pid,
      tid,
      busyMs: unionLengthUs(this.#tasks.get(key) ?? []) / 1000,
      slices: GPU_PROCESS_SLICES.map((name) =>
        Object.assign({ name }, summarise(this.#gpuSlices.get(`${key}:${name}`) ?? [])),
      ),
    };
  }

  #threadFigures(): ThreadFigures[] {
    const keys = new Set([...this.#tasks.keys(), ...this.#gc.keys()]);
    return [...keys]
      .map((key) => {
        const [pid = 0, tid = 0] = key.split(":").map(Number);
        return {
          pid,
          tid,
          process: this.#processNames.get(pid) ?? null,
          thread: this.#threadNames.get(key) ?? null,
          busyMs: unionLengthUs(this.#tasks.get(key) ?? []) / 1000,
          gc: summarise(mergedLengthsUs(this.#gc.get(key) ?? []).map((us) => us / 1000)),
        };
      })
      .toSorted((a, b) => a.pid - b.pid || a.tid - b.tid);
  }

  #userTimingFigures(): UserTimingFigures[] {
    return [...this.#userTiming]
      .map(([key, pairs]) => {
        const [pid = "0", tid = "0", name = ""] = key.split("\u0000");
        const sorted = pairs.toSorted((a, b) => a[0] - b[0]);
        const startsUs = sorted.map(([start]) => start);
        const durationsMs = sorted.map(([start, end]) => (end - start) / 1000);
        return Object.assign(
          { name, pid: Number(pid), tid: Number(tid), startsUs, durationsMs },
          summarise(durationsMs),
        );
      })
      .toSorted((a, b) => a.pid - b.pid || a.tid - b.tid || a.name.localeCompare(b.name));
  }
}

/** The header line Chromium writes before the first event. */
const TRACE_HEADER = '{"traceEvents":[';
/** The end of the last event's line, where the metadata begins. */
const EVENTS_END_BEFORE_METADATA = '],"metadata":';
/** The end of the last event's line in a trace without metadata. */
const EVENTS_END = "]}";

/**
 * The events of a trace file, read a line at a time.
 *
 * @remarks
 * A file not in Chromium's one-event-a-line layout (its first line is not the header) is read
 * whole and parsed once, as a JSON object with `traceEvents` or as a bare array of events.
 * @throws Error naming the line when a line of a Chromium layout does not parse; naming the file
 * when a file read whole does not parse or holds no events.
 */
export async function* readTraceEvents(path: string): AsyncGenerator<unknown, void, undefined> {
  const input = createReadStream(path, "utf8");
  const lines = createInterface({ input, crlfDelay: Infinity });
  let lineNumber = 0;
  try {
    for await (const raw of lines) {
      lineNumber += 1;
      const line = raw.trim();
      if (lineNumber === 1) {
        if (line === TRACE_HEADER) {
          continue;
        }
        yield* await readWholeTrace(path);
        return;
      }
      let body = line;
      let last = false;
      if (body.endsWith(EVENTS_END_BEFORE_METADATA)) {
        body = body.slice(0, -EVENTS_END_BEFORE_METADATA.length);
        last = true;
      } else if (body.endsWith(EVENTS_END)) {
        body = body.slice(0, -EVENTS_END.length);
        last = true;
      } else if (body.endsWith(",")) {
        body = body.slice(0, -1);
      }
      if (body.length > 0) {
        yield parseLine(body, path, lineNumber);
      }
      if (last) {
        return;
      }
    }
  } finally {
    // Closing the interface leaves its stream open when the generator stops before the end.
    lines.close();
    input.destroy();
  }
}

function parseLine(body: string, path: string, lineNumber: number): unknown {
  try {
    const event: unknown = JSON.parse(body);
    return event;
  } catch (error: unknown) {
    throw new Error(`${path}:${lineNumber} is not a trace event`, { cause: error });
  }
}

async function readWholeTrace(path: string): Promise<ReadonlyArray<unknown>> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(await readFile(path, "utf8"));
  } catch (error: unknown) {
    throw new Error(`${path} is not a trace`, { cause: error });
  }
  if (Array.isArray(parsed)) {
    return parsed;
  }
  const events = isRecord(parsed) ? parsed["traceEvents"] : undefined;
  if (!Array.isArray(events)) {
    throw new Error(`${path} has no trace events`);
  }
  return events;
}

/** Reduces events, from memory or from {@link readTraceEvents}. */
export async function reduceTrace(
  events: Iterable<unknown> | AsyncIterable<unknown>,
  options: ReduceOptions = {},
): Promise<TraceFigures> {
  const reducer = new TraceReducer(options);
  for await (const event of events) {
    reducer.add(event);
  }
  return reducer.figures();
}

/** Reduces a trace file written by {@link SpikeTrace}'s `stop`. */
export function reduceTraceFile(path: string, options: ReduceOptions = {}): Promise<TraceFigures> {
  return reduceTrace(readTraceEvents(path), options);
}
