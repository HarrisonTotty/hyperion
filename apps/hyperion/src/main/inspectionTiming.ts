/**
 * The inspection run's frame profile (`--views-check --inspection --timing`; the owner's report,
 * 2026-11-01: "Viewing a planet with terrain and atmosphere enabled drops my mac to 1 fps or
 * lower"): where a frame of `VIEW`'s body inspection goes, from orbit, closed in and low over the
 * ground (main's ask, where the shadow cascades draw), per pass on the GPU and per thread on the
 * CPU, written under `target/views-check/timing/` for the owner to run on their own machine,
 * hidden.
 *
 * @remarks
 * Each window is the renderer's and a trace's. The renderer gives its views probe's figures
 * (`InspectionTimingWindow`): the primary's draws and their intervals, each animation frame's
 * main-thread time, the primary's encoding, and each frame's GPU time by pass label from R01's
 * pass timer, where the adapter has `timestamp-query`. The trace, started and stopped around the
 * window, records the views check's categories and V8's CPU profiler on every thread; it is
 * reduced here ({@link ProfileScan}), clipped to the window, to each thread's busy time (its
 * `RunTask` slices' union), its GC, and its hottest functions by sampled self and inclusive time,
 * the renderer's main thread, its workers (the height workers, the optics worker) and the GPU
 * process's among them.
 *
 * The figures are a profile, not a measurement held to a budget: the run is hidden, and its
 * machine may be shared. Nothing is judged; the run ends with 1 only where the file could not be
 * written.
 */

import { access, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type { TraceConfig } from "electron";

import type {
  AtmosphereLeversOverride,
  CompositeProbeName,
  InspectionGpuCall,
  InspectionReactCommits,
  InspectionTimingMarchCensus,
  InspectionTimingClusters,
  InspectionTimingMemory,
  InspectionTimingRecord,
  InspectionTimingWindow,
  InspectionTimingWindowName,
  MarchProbeName,
  LitProbeName,
  ServedWorldNames,
  SpikePassTimer,
  ViewsCheckCanvas,
  ViewsCheckWindow,
} from "../preload/api";
import { type Measured, measured, missing } from "./measured";
import { readTraceFileEvents, TraceReducer } from "./reduceTrace";
import { formatAsPrettier, ADDED_FILE_LIMIT_BYTES, type ResultsFiles } from "./results";
import type { ViewsCheckRunInput } from "./viewsCheckResults";

/** The file's schema name. */
export const INSPECTION_TIMING_SCHEMA = "hyperion.views-check.inspection-timing";

/**
 * The schema's version; bumped with any change to the file's shape: 2 since each window's march
 * census, the run's march probe and its levers over the setting's (plan R08, R08.T8.d), 3 since the
 * run's lit probe (R08.T9.b), 4 since each window's terrain cluster counters (plan R14, R14.T3.d), 5
 * since each window's React commits and each thread's React work (lane perf-render-thread-2), 6
 * since each thread's longest task (lane perf-render-thread-ground).
 */
export const INSPECTION_TIMING_VERSION = 6;

/**
 * V8's CPU profiler's category, which samples every isolate's stack: the renderer's main thread,
 * its workers' and the main process's.
 */
export const CPU_PROFILER_CATEGORY = "disabled-by-default-v8.cpu_profiler";

/**
 * A profiled window's trace categories: the views check's own (`VIEWS_CHECK_TRACE_CATEGORIES`, the
 * threads' tasks, GC and the page's `performance.measure` spans, by which the trace's clock is set
 * against the page's) and V8's CPU profiler ({@link CPU_PROFILER_CATEGORY}).
 */
export const INSPECTION_TIMING_TRACE_CATEGORIES: ReadonlyArray<string> = [
  "devtools.timeline",
  "disabled-by-default-devtools.timeline",
  "disabled-by-default-devtools.timeline.frame",
  "disabled-by-default-v8.gc",
  "blink.user_timing",
  CPU_PROFILER_CATEGORY,
];

/**
 * A profiled window's trace buffer, 1 GiB: a minute of these categories with every thread's
 * samples is some hundreds of megabytes; the buffer stops recording when full rather than dropping
 * the window's start.
 */
export const INSPECTION_TIMING_TRACE_BUFFER_KB = 1024 * 1024;

/** The tracing configuration of a profiled window. */
export function inspectionTimingTraceConfig(): TraceConfig {
  return {
    recording_mode: "record-until-full",
    trace_buffer_size_in_kb: INSPECTION_TIMING_TRACE_BUFFER_KB,
    included_categories: [...INSPECTION_TIMING_TRACE_CATEGORIES],
    excluded_categories: ["*"],
  };
}

/** The functions each thread lists, by self time and by inclusive time. */
export const PROFILED_FUNCTIONS = 15;

/**
 * The functions each thread lists by inclusive time, more than by self time, so that a React
 * component's sampled renders, its function's inclusive time, are among them where they count
 * (lane perf-render-thread-2): React calls each component apart, its children after it.
 */
export const PROFILED_INCLUSIVE_FUNCTIONS = 100;

/** The threads a window lists, the busiest first. */
export const PROFILED_THREADS = 12;

/** The longest function name kept, characters: a Rust symbol of the surface module runs to 400. */
const FUNCTION_NAME_CHARS = 120;

/** One function's sampled time on a thread over a window. */
export interface ProfiledFunction {
  /** Its name, script file and line (`surveyEdges styles-….js:36206`), or V8's own (`(program)`). */
  readonly name: string;
  readonly ms: number;
}

/** One thread over a window, from its trace. */
export interface ProfiledThread {
  /** Its process's name (`Renderer`, `GPU Process`, `Browser`), or `null` where the trace has none. */
  readonly process: string | null;
  /** Its name (`CrRendererMain`, `DedicatedWorker thread`, `CrGpuMain`), or `null`. */
  readonly thread: string | null;
  /** The union of its tasks within the window, ms. */
  readonly busyMs: number;
  /** The union of its GC slices within the window, ms. */
  readonly gcMs: number;
  /** Its longest task within the window, its part inside the window alone, ms; 0 with none. */
  readonly longestTaskMs: number;
  /** Its CPU profile's sampled time within the window, idle left out, ms; `null` with none. */
  readonly sampledMs: number | null;
  /** Its hottest functions by self time, at most {@link PROFILED_FUNCTIONS}. */
  readonly self: ReadonlyArray<ProfiledFunction>;
  /**
   * Its hottest functions by time with their callees, at most
   * {@link PROFILED_INCLUSIVE_FUNCTIONS}.
   */
  readonly inclusive: ReadonlyArray<ProfiledFunction>;
  /**
   * Its sampled time in React's work, with its callees, ms ({@link REACT_WORK_FUNCTIONS}), or
   * `null` with no samples: on the renderer's main thread, every render and commit and their
   * effects.
   */
  readonly reactMs: ReactWorkMs | null;
}

/**
 * React's work, sampled (lane perf-render-thread-2): its renders with their commits and layout
 * effects, and its passive effects, run apart, each with its callees, ms.
 */
export interface ReactWorkMs {
  readonly renderAndCommit: number;
  readonly passiveEffects: number;
}

/**
 * The functions of `react-dom`'s production build whose inclusive time is React's work, by
 * {@link ReactWorkMs}' parts: `performWorkOnRoot` renders a root's lanes and commits them, its
 * layout effects among them; `flushPassiveEffects` runs the passive effects a commit left.
 */
export const REACT_WORK_FUNCTIONS: Readonly<Record<keyof ReactWorkMs, string>> = {
  renderAndCommit: "performWorkOnRoot",
  passiveEffects: "flushPassiveEffects",
};

/** A window's trace, reduced: its threads, the busiest first, at most {@link PROFILED_THREADS}. */
export interface ProfileTraceFigures {
  readonly threads: ReadonlyArray<ProfiledThread>;
}

/** A half-open span of the trace's clock, µs. */
type Span = readonly [startUs: number, endUs: number];

/** One node of a CPU profile: its caller's ID and its name. */
interface ProfileNode {
  readonly parent: number | undefined;
  readonly name: string;
}

/** One CPU profile, its samples kept for the window they fall in. */
interface ProfileState {
  /** The profiled thread, from its `Profile` event; its chunks come on V8's profiler thread. */
  tid: number | undefined;
  readonly nodes: Map<number, ProfileNode>;
  /** The trace's clock at the last sample, µs. */
  atUs: number | undefined;
  lastNode: number | undefined;
  /** Each sample's node, its time and how long it lasted, until the next sample, µs. */
  readonly sampleNodes: number[];
  readonly sampleAtUs: number[];
  readonly sampleUs: number[];
}

/** V8's pseudo-functions that are no code's: the profile's root and its idle time. */
const NOT_CODE = new Set(["(root)", "(idle)"]);

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function numberOf(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function stringOf(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

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

/** The longest of `spans` within [fromUs, toUs], each clipped to it, µs; 0 with none. */
function clippedLongestUs(spans: ReadonlyArray<Span>, fromUs: number, toUs: number): number {
  let longest = 0;
  for (const [start, end] of spans) {
    longest = Math.max(longest, Math.min(end, toUs) - Math.max(start, fromUs));
  }
  return longest;
}

/** A call frame's name as the profile lists it: function, script file and line. */
function functionName(frame: Readonly<Record<string, unknown>>): string {
  const name = stringOf(frame["functionName"]) ?? "";
  const url = stringOf(frame["url"]) ?? "";
  const line = numberOf(frame["lineNumber"]);
  const file = url.split(/[?#]/u)[0]?.split("/").at(-1) ?? "";
  const full =
    file.length === 0
      ? name.length === 0
        ? "(anonymous)"
        : name
      : `${name.length === 0 ? "(anonymous)" : name} ${file}${line === undefined ? "" : `:${String(line + 1)}`}`;
  return full.length > FUNCTION_NAME_CHARS ? `${full.slice(0, FUNCTION_NAME_CHARS - 1)}…` : full;
}

/** The `count` largest of `totals`, ms, largest first, ties by name. */
function hottest(totals: ReadonlyMap<string, number>, count: number): ProfiledFunction[] {
  return [...totals]
    .filter(([name]) => !NOT_CODE.has(name))
    .toSorted(([an, a], [bn, b]) => b - a || (an < bn ? -1 : 1))
    .slice(0, count)
    .map(([name, us]) => ({ name, ms: us / 1000 }));
}

/**
 * Folds a trace's events into each thread's busy time, GC and sampled functions, for a window of
 * its clock ({@link ProfileScan.figures}).
 *
 * @remarks
 * As R05's reducer reads them (`reduceTrace.ts`): a thread's busy time is the union of its
 * `RunTask` slices, its GC the union of its `v8.gc` slices and `MinorGC` and `MajorGC`, and a CPU
 * profile's chunk's deltas each the time from the sample before it, so that each sample lasts
 * until the next. A profile's samples are placed on the trace's clock from its `Profile` event's
 * `startTime`. The events may come in any order but a profile's own, which Chromium writes in
 * order.
 */
export class ProfileScan {
  readonly #threadNames = new Map<string, string>();
  readonly #processNames = new Map<number, string>();
  readonly #tasks = new Map<string, Span[]>();
  readonly #gc = new Map<string, Span[]>();
  readonly #profiles = new Map<string, ProfileState>();

  /** Adds one parsed trace event; anything that is not one it reads is ignored. */
  add(raw: unknown): void {
    if (!isRecord(raw)) {
      return;
    }
    const name = stringOf(raw["name"]);
    const ph = stringOf(raw["ph"]);
    const pid = numberOf(raw["pid"]);
    const tid = numberOf(raw["tid"]);
    if (name === undefined || ph === undefined || pid === undefined || tid === undefined) {
      return;
    }
    const args = isRecord(raw["args"]) ? raw["args"] : {};
    const key = `${String(pid)}:${String(tid)}`;
    if (ph === "M") {
      const label = stringOf(args["name"]);
      if (label !== undefined && name === "thread_name") {
        this.#threadNames.set(key, label);
      } else if (label !== undefined && name === "process_name") {
        this.#processNames.set(pid, label);
      }
      return;
    }
    const ts = numberOf(raw["ts"]) ?? 0;
    const dur = numberOf(raw["dur"]);
    if (ph === "X" && dur !== undefined) {
      const span: Span = [ts, ts + dur];
      if (name === "RunTask") {
        push(this.#tasks, key, span);
      }
      const cat = stringOf(raw["cat"]) ?? "";
      if (
        cat.split(",").some((each) => each.endsWith("v8.gc")) ||
        name === "MinorGC" ||
        name === "MajorGC"
      ) {
        push(this.#gc, key, span);
      }
      return;
    }
    const id = raw["id"];
    if (ph === "P" && (typeof id === "string" || typeof id === "number")) {
      this.#profile(`${String(pid)}:${String(id)}`, name, tid, ts, args);
    }
  }

  #profile(
    key: string,
    name: string,
    tid: number,
    ts: number,
    args: Readonly<Record<string, unknown>>,
  ): void {
    let state = this.#profiles.get(key);
    if (state === undefined) {
      state = {
        tid: undefined,
        nodes: new Map(),
        atUs: undefined,
        lastNode: undefined,
        sampleNodes: [],
        sampleAtUs: [],
        sampleUs: [],
      };
      this.#profiles.set(key, state);
    }
    const data = isRecord(args["data"]) ? args["data"] : {};
    if (name === "Profile") {
      state.tid = tid;
      state.atUs ??= numberOf(data["startTime"]) ?? ts;
      return;
    }
    if (name !== "ProfileChunk") {
      return;
    }
    const profile = isRecord(data["cpuProfile"]) ? data["cpuProfile"] : {};
    const nodes = profile["nodes"];
    for (const node of Array.isArray(nodes) ? nodes : []) {
      const nodeId = isRecord(node) ? numberOf(node["id"]) : undefined;
      const frame = isRecord(node) ? node["callFrame"] : undefined;
      if (nodeId !== undefined && isRecord(node) && isRecord(frame)) {
        state.nodes.set(nodeId, { parent: numberOf(node["parent"]), name: functionName(frame) });
      }
    }
    const samples = profile["samples"];
    const deltas = data["timeDeltas"];
    if (!Array.isArray(samples) || !Array.isArray(deltas)) {
      return;
    }
    for (let i = 0; i < samples.length; i += 1) {
      const deltaUs = numberOf(deltas[i]) ?? 0;
      const atUs = (state.atUs ?? ts) + deltaUs;
      if (state.lastNode !== undefined && state.atUs !== undefined && deltaUs > 0) {
        state.sampleNodes.push(state.lastNode);
        state.sampleAtUs.push(state.atUs);
        state.sampleUs.push(deltaUs);
      }
      state.atUs = atUs;
      state.lastNode = numberOf(samples[i]);
    }
  }

  /** Each thread's figures within [fromUs, toUs] of the trace's clock, the busiest first. */
  figures(fromUs: number, toUs: number): ProfileTraceFigures {
    const keys = new Set([...this.#tasks.keys(), ...this.#gc.keys()]);
    const profiles = new Map<string, ProfileState>();
    for (const [profileKey, state] of this.#profiles) {
      const pid = profileKey.split(":")[0];
      if (state.tid !== undefined && pid !== undefined) {
        const key = `${pid}:${String(state.tid)}`;
        profiles.set(key, state);
        keys.add(key);
      }
    }
    const threads = [...keys].map((key): ProfiledThread => {
      const pid = Number(key.split(":")[0]);
      const sampled = profiles.get(key);
      const functions = sampled === undefined ? null : sampledFunctions(sampled, fromUs, toUs);
      return {
        process: this.#processNames.get(pid) ?? null,
        thread: this.#threadNames.get(key) ?? null,
        busyMs: clippedUnionUs(this.#tasks.get(key) ?? [], fromUs, toUs) / 1000,
        longestTaskMs: clippedLongestUs(this.#tasks.get(key) ?? [], fromUs, toUs) / 1000,
        gcMs: clippedUnionUs(this.#gc.get(key) ?? [], fromUs, toUs) / 1000,
        sampledMs: functions === null ? null : functions.sampledUs / 1000,
        self: functions === null ? [] : hottest(functions.self, PROFILED_FUNCTIONS),
        inclusive:
          functions === null ? [] : hottest(functions.inclusive, PROFILED_INCLUSIVE_FUNCTIONS),
        reactMs: functions === null ? null : reactWorkOf(functions.inclusive),
      };
    });
    return {
      threads: threads
        .filter((thread) => thread.busyMs > 0 || (thread.sampledMs ?? 0) > 0)
        .toSorted(
          (a, b) =>
            b.busyMs - a.busyMs ||
            (b.sampledMs ?? 0) - (a.sampledMs ?? 0) ||
            `${a.process ?? ""}${a.thread ?? ""}`.localeCompare(
              `${b.process ?? ""}${b.thread ?? ""}`,
            ),
        )
        .slice(0, PROFILED_THREADS),
    };
  }
}

/**
 * A profile's sampled time within [fromUs, toUs]: by function, self and with its callees (a
 * function on a sample's stack more than once counted once), and in all, idle left out, µs.
 */
function sampledFunctions(
  state: ProfileState,
  fromUs: number,
  toUs: number,
): {
  readonly self: Map<string, number>;
  readonly inclusive: Map<string, number>;
  readonly sampledUs: number;
} {
  const byNode = new Map<number, number>();
  for (let i = 0; i < state.sampleNodes.length; i += 1) {
    const atUs = state.sampleAtUs[i] ?? Number.NaN;
    const node = state.sampleNodes[i];
    if (node !== undefined && atUs >= fromUs && atUs < toUs) {
      byNode.set(node, (byNode.get(node) ?? 0) + (state.sampleUs[i] ?? 0));
    }
  }
  const self = new Map<string, number>();
  const inclusive = new Map<string, number>();
  let sampledUs = 0;
  for (const [id, us] of byNode) {
    const leaf = state.nodes.get(id);
    const name = leaf?.name ?? "(unknown)";
    self.set(name, (self.get(name) ?? 0) + us);
    if (name !== "(idle)") {
      sampledUs += us;
    }
    const seen = new Set<string>();
    for (let node = leaf; node !== undefined;) {
      if (!seen.has(node.name)) {
        seen.add(node.name);
        inclusive.set(node.name, (inclusive.get(node.name) ?? 0) + us);
      }
      node = node.parent === undefined ? undefined : state.nodes.get(node.parent);
    }
  }
  return { self, inclusive, sampledUs };
}

/**
 * React's work in a thread's inclusive times by function ({@link REACT_WORK_FUNCTIONS}), ms: each
 * part the sum over the functions of its name, whatever their script and line.
 */
function reactWorkOf(inclusive: ReadonlyMap<string, number>): ReactWorkMs {
  const of = (name: string): number => {
    let us = 0;
    for (const [each, eachUs] of inclusive) {
      if (each === name || each.startsWith(`${name} `)) {
        us += eachUs;
      }
    }
    return us / 1000;
  };
  return {
    renderAndCommit: of(REACT_WORK_FUNCTIONS.renderAndCommit),
    passiveEffects: of(REACT_WORK_FUNCTIONS.passiveEffects),
  };
}

function push<K, V>(map: Map<K, V[]>, key: K, value: V): void {
  const list = map.get(key);
  if (list === undefined) {
    map.set(key, [value]);
  } else {
    list.push(value);
  }
}

/** A profiled window's trace read: its clock against the page's, and its threads' scan. */
export interface ReducedProfile {
  /** The trace's clock less the page's, µs (`TraceFigures.clockOffsetUs`), or `null`. */
  readonly clockOffsetUs: number | null;
  readonly scan: ProfileScan;
}

/** Reads a profiled window's trace file once: R05's reducer for its clock, and the scan. */
export async function reduceProfileFile(path: string): Promise<ReducedProfile> {
  const reducer = new TraceReducer({ categories: INSPECTION_TIMING_TRACE_CATEGORIES });
  const scan = new ProfileScan();
  for await (const event of readTraceFileEvents(path)) {
    reducer.add(event);
    scan.add(event);
  }
  return { clockOffsetUs: reducer.figures().clockOffsetUs, scan };
}

/**
 * A reduced profile's figures over the measured `window`, the page's clock set against the
 * trace's by the page's `performance.measure` spans; missing, with why, without one.
 */
export function profileFigures(
  reduced: ReducedProfile,
  window: ViewsCheckWindow,
): Measured<ProfileTraceFigures> {
  if (reduced.clockOffsetUs === null) {
    return missing("the trace holds no span of the page's to set its clock by");
  }
  return measured(
    reduced.scan.figures(
      reduced.clockOffsetUs + 1000 * window.startMs,
      reduced.clockOffsetUs + 1000 * window.endMs,
    ),
  );
}

/** A set of times summarised, ms: `null` each where there are none. */
export interface TimeSummary {
  readonly count: number;
  readonly medianMs: number | null;
  readonly p95Ms: number | null;
  readonly meanMs: number | null;
  readonly maxMs: number | null;
}

/** The nearest-rank `q` quantile of `values`, or `null` for none. */
function quantile(values: ReadonlyArray<number>, q: number): number | null {
  if (values.length === 0) {
    return null;
  }
  const sorted = values.toSorted((a, b) => a - b);
  const rank = Math.min(sorted.length, Math.max(1, Math.ceil(q * sorted.length)));
  return sorted[rank - 1] ?? null;
}

/** `values` summarised. */
export function summariseTimes(values: ReadonlyArray<number>): TimeSummary {
  const total = values.reduce((sum, value) => sum + value, 0);
  return {
    count: values.length,
    medianMs: quantile(values, 0.5),
    p95Ms: quantile(values, 0.95),
    meanMs: values.length === 0 ? null : total / values.length,
    maxMs: values.length === 0 ? null : values.reduce((most, v) => Math.max(most, v), 0),
  };
}

/** One pass label's GPU time over a window, summarised. */
export interface PassFigures {
  readonly label: string;
  readonly medianMs: number | null;
  readonly p95Ms: number | null;
  readonly meanMs: number | null;
  /** Its mean over the mean of the frames' GPU time, 0 to 1, or `null` with no frame timed. */
  readonly shareOfGpu: number | null;
  /** The kernels its dispatches ran, each once; none for a render pass. */
  readonly kernels: ReadonlyArray<string>;
  /** Its dispatches' workgroups a frame, the median, or `null` for a render pass or none counted. */
  readonly workgroupsMedian: number | null;
  /** Its time a workgroup, ns, the median over the frames that dispatched any; `null` otherwise. */
  readonly nsPerWorkgroup: number | null;
  /**
   * For the atmosphere's ray march ({@link isMarchKernel}), its time a texel a step, ns: a
   * workgroup's over its 8 × 8 texels and the window's samples a ray (`marchSamples`); `null`
   * for any other pass.
   */
  readonly nsPerTexelStep: number | null;
}

/** The texels of one of the atmosphere's 8 × 8 workgroups (`rayMarchFootprint.wgsl`'s). */
export const MARCH_WORKGROUP_TEXELS = 64;

/**
 * Whether a kernel is the atmosphere's per-pixel ray march, of any of its forms (`hillaire.ts`'
 * `atmosphere ray march…` kernels), but not its fill of the target's rest.
 */
export function isMarchKernel(name: string): boolean {
  return name.startsWith("atmosphere ray march") && !name.startsWith("atmosphere ray march's");
}

/**
 * The kinds a window's WebGPU calls are summed into (plan R14, the owner's Mac): the draws, the
 * dispatches, the state a pass sets, the writes, the objects made, the passes, encoders and
 * submissions, and the rest.
 */
export const GPU_CALL_KINDS = [
  "draws",
  "dispatches",
  "setPipeline",
  "setBindGroup",
  "setVertexBuffer",
  "setIndexBuffer",
  "writeBuffer",
  "writeTexture",
  "createBindGroup",
  "createBuffer or createTexture",
  "createView",
  "getBindGroupLayout",
  "passes",
  "encoders",
  "submits",
  "other",
] as const;

/** One of {@link GPU_CALL_KINDS}. */
export type GpuCallKind = (typeof GPU_CALL_KINDS)[number];

/** The kind a call, `<object>.<method>` as the counter names it, is summed into. */
export function gpuCallKind(call: string): GpuCallKind {
  const method = call.slice(call.lastIndexOf(".") + 1);
  if (call.startsWith("render pass.") && method.startsWith("draw")) {
    return "draws";
  }
  if (call.startsWith("compute pass.") && method.startsWith("dispatchWorkgroups")) {
    return "dispatches";
  }
  switch (call) {
    case "render pass.setPipeline":
    case "compute pass.setPipeline":
      return "setPipeline";
    case "render pass.setBindGroup":
    case "compute pass.setBindGroup":
      return "setBindGroup";
    case "render pass.setVertexBuffer":
      return "setVertexBuffer";
    case "render pass.setIndexBuffer":
      return "setIndexBuffer";
    case "queue.writeBuffer":
      return "writeBuffer";
    case "queue.writeTexture":
      return "writeTexture";
    case "device.createBindGroup":
      return "createBindGroup";
    case "device.createBuffer":
    case "device.createTexture":
      return "createBuffer or createTexture";
    case "texture.createView":
      return "createView";
    case "pipeline.getBindGroupLayout":
      return "getBindGroupLayout";
    case "encoder.beginRenderPass":
    case "encoder.beginComputePass":
      return "passes";
    case "device.createCommandEncoder":
      return "encoders";
    case "queue.submit":
      return "submits";
    default:
      return "other";
  }
}

/** A kind's calls and their bytes a draw of the primary. */
export interface GpuCallKindFigures {
  readonly kind: GpuCallKind;
  readonly perDraw: number;
  readonly bytesPerDraw: number;
}

/** A pass label's calls a draw of the primary, and its dearest kinds. */
export interface GpuCallPassFigures {
  readonly label: string;
  readonly perDraw: number;
  /** Its kinds by their calls, the most first, those it made none of left out. */
  readonly kinds: ReadonlyArray<{ readonly kind: GpuCallKind; readonly perDraw: number }>;
}

/** A window's WebGPU calls (`--count-gpu-calls`; plan R14), each a draw of the primary. */
export interface GpuCallFigures {
  /** Every call a draw. */
  readonly perDraw: number;
  /** Each kind's, in {@link GPU_CALL_KINDS}' order. */
  readonly kinds: ReadonlyArray<GpuCallKindFigures>;
  /** Each pass label's, the most calls first. */
  readonly passes: ReadonlyArray<GpuCallPassFigures>;
  /** The counter's own tallies, as the renderer gave them. */
  readonly calls: ReadonlyArray<InspectionGpuCall>;
}

/** A window's counted calls a draw of the primary over `draws` draws, by kind and by pass. */
export function gpuCallFigures(
  calls: ReadonlyArray<InspectionGpuCall>,
  draws: number,
): GpuCallFigures {
  const per = (count: number): number => (draws > 0 ? count / draws : 0);
  const kinds = GPU_CALL_KINDS.map((kind): GpuCallKindFigures => {
    const ofKind = calls.filter((each) => gpuCallKind(each.call) === kind);
    return {
      kind,
      perDraw: per(ofKind.reduce((sum, each) => sum + each.count, 0)),
      bytesPerDraw: per(ofKind.reduce((sum, each) => sum + each.bytes, 0)),
    };
  });
  const labels = [...new Set(calls.map((each) => each.label))];
  const passes = labels
    .map((label): GpuCallPassFigures => {
      const ofLabel = calls.filter((each) => each.label === label);
      return {
        label,
        perDraw: per(ofLabel.reduce((sum, each) => sum + each.count, 0)),
        kinds: GPU_CALL_KINDS.map((kind) => ({
          kind,
          perDraw: per(
            ofLabel
              .filter((each) => gpuCallKind(each.call) === kind)
              .reduce((sum, each) => sum + each.count, 0),
          ),
        }))
          .filter((each) => each.perDraw > 0)
          .toSorted((a, b) => b.perDraw - a.perDraw),
      };
    })
    .toSorted((a, b) => b.perDraw - a.perDraw);
  return {
    perDraw: per(calls.reduce((sum, each) => sum + each.count, 0)),
    kinds,
    passes,
    calls,
  };
}

/** One window's figures. */
export interface InspectionTimingWindowResults {
  readonly name: InspectionTimingWindowName;
  /** The window's length, ms. */
  readonly durationMs: number;
  /** The primary's draws in it. */
  readonly draws: number;
  /** Its draws a second over the window. */
  readonly drawsPerSecond: number;
  readonly canvas: ViewsCheckCanvas;
  readonly scales: ReadonlyArray<number>;
  /** The samples a ray of the atmosphere's march takes at the sizes its air was drawn at. */
  readonly marchSamples: number;
  readonly position: string | null;
  readonly statements: ReadonlyArray<string>;
  readonly frame: {
    /** Between the primary's draws. */
    readonly intervalMs: TimeSummary;
    /** Each animation frame's callbacks on the main thread. */
    readonly mainThreadMs: TimeSummary;
    /** The primary's encoding and submission each draw. */
    readonly submitMs: TimeSummary;
    /** Each frame's passes on the GPU, every view's, of the frames timed in full. */
    readonly gpuMs: TimeSummary;
  };
  /** Each pass label's GPU time, the dearest by its mean first. */
  readonly passes: ReadonlyArray<PassFigures>;
  readonly untimedFrames: number;
  /**
   * The share of the frames' GPU time in the passes that draw the terrain (the lit terrain and
   * the mesh bodies' figures, its shadow cascades, and the symbology over it with the terrain's
   * depth), 0 to 1, or `null` with no frame timed: the terrain's items, which the cluster path
   * changes (R14), told apart from the rest.
   */
  readonly terrainShareOfGpu: number | null;
  /** What the engine wrote to the GPU and made or released over the window. */
  readonly memory: InspectionTimingMemory;
  /** Its WebGPU calls a draw (`--count-gpu-calls`; plan R14), or `null` where none were counted. */
  readonly gpuCalls: GpuCallFigures | null;
  /** React's commits over it (lane perf-render-thread-2), or `null` where React took no hook. */
  readonly react: InspectionReactCommits | null;
  /** The far limb's census of the march the frame after the window dispatched, or `null`. */
  readonly marchCensus: InspectionTimingMarchCensus | null;
  /** The primary's terrain's cluster counters as the window ended (plan R14), or `null`. */
  readonly clusters: InspectionTimingClusters | null;
  /** The window's trace, reduced, or why it is missing. */
  readonly trace: Measured<ProfileTraceFigures>;
  /** The renderer's own times, as it measured them. */
  readonly samples: Pick<
    InspectionTimingWindow,
    "intervalsMs" | "mainThreadMs" | "submitMs" | "gpuMs"
  >;
}

/** A timing file of the inspection run. */
export interface InspectionTimingResults {
  readonly schema: typeof INSPECTION_TIMING_SCHEMA;
  readonly version: typeof INSPECTION_TIMING_VERSION;
  readonly run: {
    readonly startedAt: string;
    readonly machine: ViewsCheckRunInput["machine"];
    readonly versions: ViewsCheckRunInput["versions"];
    readonly platform: NodeJS.Platform;
    readonly launchMode: ViewsCheckRunInput["launchMode"];
    readonly setting: ViewsCheckRunInput["setting"];
    readonly switches: ReadonlyArray<string>;
    readonly shown: boolean;
    readonly window: ViewsCheckRunInput["window"];
    readonly devicePixelRatio: number;
    readonly served: ServedWorldNames | null;
    readonly timer: SpikePassTimer;
    /**
     * The composite's probe the profile drew under (`--composite-probe`; the owner's priority,
     * 2026-11-01), or `null`, the composite as a player's is drawn.
     */
    readonly compositeProbe: CompositeProbeName | null;
    /**
     * The march's probe the profile drew under (`--march-probe`; plan R08, R08.T8.d), or `null`,
     * the march as a player's is drawn.
     */
    readonly marchProbe: MarchProbeName | null;
    /**
     * The atmosphere's levers the profile drew with over its setting's (`--march-cover`,
     * `--limb-rays`; plan R08, R08.T8.d), or `null`, the setting's own.
     */
    readonly atmosphereLevers: AtmosphereLeversOverride | null;
    /**
     * The lit passes' probe the profile drew under (`--lit-probe`; plan R08, R08.T9.b's
     * measurement), or `null`, the surfaces lit as a player's are.
     */
    readonly litProbe: LitProbeName | null;
  };
  readonly windows: ReadonlyArray<InspectionTimingWindowResults>;
}

/** Everything a timing file is built from. */
export interface InspectionTimingInput {
  readonly run: Omit<ViewsCheckRunInput, "captures" | "displayHz" | "smoke">;
  readonly served: ServedWorldNames | null;
  /** The composite's probe the launch drew under, or `null` (`ViewsCheckLaunch.compositeProbe`). */
  readonly compositeProbe: CompositeProbeName | null;
  /** The march's probe the launch drew under, or `null` (`ViewsCheckLaunch.marchProbe`). */
  readonly marchProbe: MarchProbeName | null;
  /** The levers the launch drew with over its setting's, or `null` (`ViewsCheckLaunch.atmosphereLevers`). */
  readonly atmosphereLevers: AtmosphereLeversOverride | null;
  /** The lit passes' probe the launch drew under, or `null` (`ViewsCheckLaunch.litProbe`). */
  readonly litProbe: LitProbeName | null;
  readonly record: InspectionTimingRecord;
  /** Each window's reduced trace, by name; a window without one is recorded as missing. */
  readonly traces: ReadonlyMap<InspectionTimingWindowName, Measured<ProfileTraceFigures>>;
}

/**
 * The pass labels whose passes draw the terrain (`photoreal/passes.ts`' order): the bodies pass,
 * which draws the mesh bodies' figures and the terrain handed over after them, and its late raster
 * with the depth pyramid it is culled against (plan R14, R14.T3.d); by the visibility buffer, the
 * terrain's ID raster and the resolve's pass in the bodies pass's place (R14.T2.c to T2.e); the
 * cluster path's cull and
 * compaction, early and late under one label each; the shadow cascades' caster passes; and the
 * symbology's canvas pass, which draws the terrain's depth before its marks; each numbered from the
 * second where a frame repeats it. The renderer's labels, repeated here across the bundles.
 */
export const TERRAIN_PASS_LABELS =
  /^(?:bodies(?: late)?|depth pyramid|terrain (?:id raster|resolve)|terrain cluster (?:cull|compact)|terrain shadows|symbology)(?: #\d+)?$/u;

/** One window's figures from the renderer's record and its trace. */
function windowResults(
  window: InspectionTimingWindow,
  trace: Measured<ProfileTraceFigures> | undefined,
): InspectionTimingWindowResults {
  const durationMs = window.endMs - window.startMs;
  const draws = window.submitMs.length;
  const gpu = summariseTimes(window.gpuMs);
  const passes = window.passes
    .map((pass): PassFigures => {
      const summary = summariseTimes(pass.ms);
      const dispatched = pass.workgroups.filter((count) => count > 0);
      const perWorkgroup = pass.ms.flatMap((ms, i) => {
        const count = pass.workgroups[i] ?? 0;
        return count > 0 ? [(ms * 1e6) / count] : [];
      });
      const nsPerWorkgroup = summariseTimes(perWorkgroup).medianMs;
      return {
        label: pass.label,
        medianMs: summary.medianMs,
        p95Ms: summary.p95Ms,
        meanMs: summary.meanMs,
        shareOfGpu:
          summary.meanMs === null || gpu.meanMs === null || !(gpu.meanMs > 0)
            ? null
            : summary.meanMs / gpu.meanMs,
        kernels: pass.kernels,
        workgroupsMedian: summariseTimes(dispatched).medianMs,
        nsPerWorkgroup,
        nsPerTexelStep:
          nsPerWorkgroup !== null && pass.kernels.length > 0 && pass.kernels.every(isMarchKernel)
            ? nsPerWorkgroup / MARCH_WORKGROUP_TEXELS / window.marchSamples
            : null,
      };
    })
    .toSorted((a, b) => (b.meanMs ?? 0) - (a.meanMs ?? 0));
  const terrainMeanMs = passes
    .filter((pass) => TERRAIN_PASS_LABELS.test(pass.label))
    .reduce((sum, pass) => sum + (pass.meanMs ?? 0), 0);
  return {
    name: window.name,
    durationMs,
    draws,
    drawsPerSecond: durationMs > 0 ? (1000 * draws) / durationMs : 0,
    canvas: window.canvas,
    scales: window.scales,
    marchSamples: window.marchSamples,
    position: window.position,
    statements: window.statements,
    frame: {
      intervalMs: summariseTimes(window.intervalsMs),
      mainThreadMs: summariseTimes(window.mainThreadMs),
      submitMs: summariseTimes(window.submitMs),
      gpuMs: gpu,
    },
    passes,
    untimedFrames: window.untimedFrames,
    terrainShareOfGpu: gpu.meanMs === null || !(gpu.meanMs > 0) ? null : terrainMeanMs / gpu.meanMs,
    memory: window.memory,
    gpuCalls: window.gpuCalls === null ? null : gpuCallFigures(window.gpuCalls, draws),
    react: window.react,
    marchCensus: window.marchCensus,
    clusters: window.clusters,
    trace: trace ?? missing("no trace was recorded over the window"),
    samples: {
      intervalsMs: window.intervalsMs,
      mainThreadMs: window.mainThreadMs,
      submitMs: window.submitMs,
      gpuMs: window.gpuMs,
    },
  };
}

/** Builds the timing file from the run's description, the renderer's record and the traces. */
export function buildInspectionTimingResults(
  input: InspectionTimingInput,
): InspectionTimingResults {
  const { run, record } = input;
  return {
    schema: INSPECTION_TIMING_SCHEMA,
    version: INSPECTION_TIMING_VERSION,
    run: {
      startedAt: run.startedAt.toISOString(),
      machine: run.machine,
      versions: run.versions,
      platform: run.platform,
      launchMode: run.launchMode,
      setting: run.setting,
      switches: run.switches,
      shown: run.shown,
      window: run.window,
      devicePixelRatio: record.devicePixelRatio,
      served: input.served,
      timer: record.timer,
      compositeProbe: input.compositeProbe,
      marchProbe: input.marchProbe,
      atmosphereLevers: input.atmosphereLevers,
      litProbe: input.litProbe,
    },
    windows: record.windows.map((window) => windowResults(window, input.traces.get(window.name))),
  };
}

/** A table cell's text, its pipes escaped. */
function cell(text: string): string {
  return text.replaceAll("|", "\\|");
}

/** A time or `—`, ms, to `digits` decimals. */
function msText(ms: number | null, digits = 1): string {
  return ms === null ? "—" : ms.toFixed(digits);
}

/** A count of writes or allocations and their bytes, as the summary writes them. */
function bytesText(sum: { readonly count: number; readonly bytes: number }): string {
  return `${String(sum.count)}, ${(sum.bytes / 1_048_576).toFixed(2)} MiB`;
}

/** A summary's row: median, p95, mean and max. */
function summaryRow(name: string, summary: TimeSummary): string {
  return `| ${name} | ${String(summary.count)} | ${msText(summary.medianMs)} | ${msText(summary.p95Ms)} | ${msText(summary.meanMs)} | ${msText(summary.maxMs)} |`;
}

/** A thread's name as the summary writes it. */
function threadName(thread: ProfiledThread): string {
  return `${thread.process ?? "?"} / ${thread.thread ?? "?"}`;
}

/** The threads whose functions the summary lists: those with samples, the busiest four. */
const LISTED_THREADS = 4;

/** The functions the summary lists of each thread, by self and by inclusive time. */
const LISTED_FUNCTIONS = 10;

/** A thread's hottest functions, by self time beside inclusive time, as the summary lists them. */
function functionsMarkdown(thread: ProfiledThread): string[] {
  const lines = [
    `### Hottest functions: ${cell(threadName(thread))}`,
    "",
    "| Self (ms) | Function | Inclusive (ms) | Function |",
    "| --- | --- | --- | --- |",
  ];
  const rows = Math.min(LISTED_FUNCTIONS, Math.max(thread.self.length, thread.inclusive.length));
  for (let i = 0; i < rows; i += 1) {
    const own = thread.self[i];
    const all = thread.inclusive[i];
    lines.push(
      `| ${own === undefined ? "" : own.ms.toFixed(0)} | ${own === undefined ? "" : `\`${cell(own.name)}\``} | ${all === undefined ? "" : all.ms.toFixed(0)} | ${all === undefined ? "" : `\`${cell(all.name)}\``} |`,
    );
  }
  lines.push("");
  return lines;
}

/** The share of the GPU's time under which the summary leaves a pass out, as the JSON keeps it. */
const LISTED_PASS_SHARE = 0.001;

/** The passes the summary lists: those of {@link LISTED_PASS_SHARE} of the GPU's time or more. */
function listedPasses(passes: ReadonlyArray<PassFigures>): ReadonlyArray<PassFigures> {
  return passes.filter((pass) => pass.shareOfGpu === null || pass.shareOfGpu >= LISTED_PASS_SHARE);
}

/** The passes the summary lists by their calls, the most first; the JSON keeps every one. */
const LISTED_CALL_PASSES = 20;

/** The kinds the summary lists of each pass, the most calls first. */
const LISTED_CALL_KINDS = 4;

/** A count a draw as the summary writes it. */
function perDrawText(perDraw: number): string {
  return perDraw >= 100 ? perDraw.toFixed(0) : perDraw.toFixed(1);
}

/** A window's counted WebGPU calls, by kind and by pass, as the summary lists them. */
function gpuCallsMarkdown(calls: GpuCallFigures | null): string[] {
  if (calls === null) {
    return [];
  }
  const listed = calls.passes.slice(0, LISTED_CALL_PASSES);
  return [
    "### WebGPU calls (counted)",
    "",
    `- **Every call:** ${perDrawText(calls.perDraw)} a draw of the primary; the counter's own cost is in this window's times.`,
    "",
    "| Kind | Calls a draw | KiB a draw |",
    "| --- | --- | --- |",
    ...calls.kinds
      .filter((each) => each.perDraw > 0)
      .map(
        (each) =>
          `| ${each.kind} | ${perDrawText(each.perDraw)} | ${each.bytesPerDraw > 0 ? (each.bytesPerDraw / 1024).toFixed(1) : "—"} |`,
      ),
    "",
    "| Pass | Calls a draw | The most |",
    "| --- | --- | --- |",
    ...listed.map(
      (pass) =>
        `| ${cell(pass.label)} | ${perDrawText(pass.perDraw)} | ${pass.kinds
          .slice(0, LISTED_CALL_KINDS)
          .map((each) => `${each.kind} ${perDrawText(each.perDraw)}`)
          .join(", ")} |`,
    ),
    ...(calls.passes.length > listed.length
      ? [
          "",
          `${String(calls.passes.length - listed.length)} more pass labels, of ${perDrawText(calls.passes.slice(listed.length).reduce((sum, pass) => sum + pass.perDraw, 0))} calls a draw in all, are listed in the JSON.`,
        ]
      : []),
    "",
  ];
}

/** The renderer's main thread's name in a trace, whose React work the summary states. */
const RENDERER_MAIN_THREAD = "CrRendererMain";

/**
 * A window's React commits and work as lines of the summary (lane perf-render-thread-2): the
 * commits and renders a draw, the renderer's main thread's sampled React work, and the components
 * the most rendered, with what changed for each render; nothing where React took no hook.
 */
function reactMarkdown(window: InspectionTimingWindowResults): string[] {
  const { react } = window;
  if (react === null) {
    return [];
  }
  const perDraw = (count: number): string =>
    window.draws === 0 ? "—" : perDrawText(count / window.draws);
  const main = window.trace.value?.threads.find(
    (thread) => thread.thread === RENDERER_MAIN_THREAD && thread.reactMs !== null,
  );
  const work =
    main?.reactMs === null || main?.reactMs === undefined
      ? "—"
      : `render and commit ${main.reactMs.renderAndCommit.toFixed(0)} ms (${perDraw(main.reactMs.renderAndCommit)} a draw), passive effects ${main.reactMs.passiveEffects.toFixed(0)} ms (${perDraw(main.reactMs.passiveEffects)} a draw)`;
  return [
    "### React (commits counted)",
    "",
    `- **Commits:** ${String(react.commits)} (${perDraw(react.commits)} a draw of the primary); **renders:** ${String(react.renders)} (${perDraw(react.renders)} a draw); the hook's own walk is in this window's times.`,
    `- **React's work on the renderer's main thread (sampled):** ${work}`,
    "",
    "| Component | Renders | A draw | Sampled (ms) | Mounts | Nothing changed | State changed | Props changed (renders) |",
    "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ...react.components.map((each) => {
      // Its function's own time with its callees: React calls its children after it, not in it.
      const sampled = main?.inclusive.find(
        (fn) => fn.name === each.name || fn.name.startsWith(`${each.name} `),
      );
      return `| ${cell(each.name)} | ${String(each.renders)} | ${perDraw(each.renders)} | ${sampled === undefined ? "—" : sampled.ms.toFixed(0)} | ${String(each.mounts)} | ${String(each.unchanged)} | ${String(each.stateChanged)} | ${each.propsChanged.length === 0 ? "—" : cell(each.propsChanged.map(({ prop, renders }) => `${prop} ${String(renders)}`).join(", "))} |`;
    }),
    "",
  ];
}

/**
 * A window's march census as lines of the summary (plan R08, R08.T8.d): the texels by their
 * footprints' node rays, the limb's tiles, and the rays against the lanes' rays, each tile's texels
 * waiting on its slowest; nothing where none was taken.
 */
function censusMarkdown(census: InspectionTimingMarchCensus | null): string[] {
  if (census === null) {
    return [];
  }
  const counted = census.texelsByNodes.flatMap((texels, nodes) =>
    texels > 0 ? [`${String(nodes)}: ${String(texels)}`] : [],
  );
  const waiting = census.rays > 0 ? (census.laneRays / census.rays).toFixed(2) : "—";
  return [
    `The march's far limb, the frame after the window (\`${cell(census.kernel)}\`): texels by node rays ${counted.length === 0 ? "—" : counted.join(", ")}; ${String(census.wholeMeans)} at w_L 1, with no centre ray; ${String(census.centresInAir)} centre rays in the air; ${String(census.limbTiles)} of ${String(census.tiles)} tiles hold a node ray; ${String(census.rays)} rays in the air, ${String(census.laneRays)} as each tile waits on its slowest texel (${waiting} times).`,
    "",
  ];
}

/** A window's part of the summary. */
function windowMarkdown(window: InspectionTimingWindowResults): string[] {
  const trace = window.trace;
  const threads = trace.value?.threads ?? [];
  const perDraw = (total: number): string =>
    window.draws === 0 ? "—" : (total / window.draws).toFixed(1);
  const sampled = threads.filter((thread) => thread.self.length > 0).slice(0, LISTED_THREADS);
  return [
    `## ${window.name}: ${String(window.draws)} draws in ${(window.durationMs / 1000).toFixed(1)} s, ${window.drawsPerSecond.toFixed(2)} a second`,
    "",
    `- **Primary:** ${String(window.canvas.widthPx)} × ${String(window.canvas.heightPx)} px; internal scale ${window.scales.length === 0 ? "—" : window.scales.map((scale) => scale.toFixed(3)).join(" → ")}; frames not timed in full ${String(window.untimedFrames)}`,
    `- **Position:** ${cell(window.position ?? "—")}`,
    `- **Statements:** ${cell(window.statements.length === 0 ? "—" : window.statements.join(" · "))}`,
    "",
    "| Frame | Count | Median (ms) | p95 (ms) | Mean (ms) | Max (ms) |",
    "| --- | --- | --- | --- | --- | --- |",
    summaryRow("Interval between the primary's draws", window.frame.intervalMs),
    summaryRow("Main thread, each animation frame's callbacks", window.frame.mainThreadMs),
    summaryRow("The primary's encoding and submission", window.frame.submitMs),
    summaryRow("GPU, every pass of the frame", window.frame.gpuMs),
    "",
    "### GPU by pass",
    "",
    ...(window.passes.length === 0
      ? ["No pass was timed (the adapter has no `timestamp-query`, or no frame was timed in full)."]
      : [
          "| Pass | Median (ms) | p95 (ms) | Mean (ms) | Share of GPU | Kernels | Workgroups | µs a workgroup | ns a texel a step |",
          "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
          ...listedPasses(window.passes).map(
            (pass) =>
              `| ${cell(pass.label)} | ${msText(pass.medianMs, 2)} | ${msText(pass.p95Ms, 2)} | ${msText(pass.meanMs, 2)} | ${pass.shareOfGpu === null ? "—" : `${(100 * pass.shareOfGpu).toFixed(1)}%`} | ${pass.kernels.length === 0 ? "—" : cell(pass.kernels.join(", "))} | ${pass.workgroupsMedian === null ? "—" : pass.workgroupsMedian.toFixed(0)} | ${pass.nsPerWorkgroup === null ? "—" : (pass.nsPerWorkgroup / 1000).toFixed(2)} | ${pass.nsPerTexelStep === null ? "—" : pass.nsPerTexelStep.toFixed(1)} |`,
          ),
          ...(window.passes.length > listedPasses(window.passes).length
            ? [
                "",
                `${String(window.passes.length - listedPasses(window.passes).length)} passes of under ${String(100 * LISTED_PASS_SHARE)}% of the GPU's time each are left out here, and listed in the JSON.`,
              ]
            : []),
          "",
          `The atmosphere's ray march takes ${String(window.marchSamples)} samples a ray here; its time a texel a step is a workgroup's over its ${String(MARCH_WORKGROUP_TEXELS)} texels and those samples.`,
        ]),
    "",
    ...censusMarkdown(window.marchCensus),
    `The terrain's passes (\`bodies\` and \`bodies late\`, \`depth pyramid\`, the \`terrain cluster\` dispatches, \`terrain shadows\`, \`symbology\`) take ${window.terrainShareOfGpu === null ? "—" : `${(100 * window.terrainShareOfGpu).toFixed(1)}%`} of the GPU's time; the rest is the sky, the discs, the atmosphere and the post-processing.`,
    "",
    `- **Clusters** (the terrain's cluster counters as the window ended, by pass): ${window.clusters === null ? "—" : cell(window.clusters.passes.map((pass) => `${pass.label} ${String(pass.clusters)} (${String(pass.triangles)} triangles)`).join(", "))}`,
    "",
    "### Uploads and allocations",
    "",
    `- **Written:** ${bytesText(window.memory.uploaded)} (${perDraw(window.memory.uploaded.bytes / 1024)} KiB a draw); **made:** ${bytesText(window.memory.created)}; **released:** ${bytesText(window.memory.destroyed)}`,
    ...(window.memory.uploads.length === 0
      ? []
      : [
          "",
          "| Written to | Writes | Bytes |",
          "| --- | --- | --- |",
          ...window.memory.uploads.map(
            (each) => `| ${cell(each.name)} | ${String(each.count)} | ${String(each.bytes)} |`,
          ),
        ]),
    "",
    ...gpuCallsMarkdown(window.gpuCalls),
    ...reactMarkdown(window),
    "### Threads (trace)",
    "",
    ...(trace.value === null
      ? [`— (${trace.reason})`]
      : [
          "| Process / thread | Busy (ms) | Busy a draw (ms) | Share of the window | Longest task (ms) | GC (ms) | Sampled (ms) |",
          "| --- | --- | --- | --- | --- | --- | --- |",
          ...threads.map(
            (thread) =>
              `| ${cell(threadName(thread))} | ${thread.busyMs.toFixed(0)} | ${perDraw(thread.busyMs)} | ${window.durationMs > 0 ? `${((100 * thread.busyMs) / window.durationMs).toFixed(1)}%` : "—"} | ${thread.longestTaskMs.toFixed(1)} | ${thread.gcMs.toFixed(0)} | ${thread.sampledMs === null ? "—" : thread.sampledMs.toFixed(0)} |`,
          ),
        ]),
    "",
    ...sampled.flatMap(functionsMarkdown),
  ];
}

/** The launch line's part for the levers a run drew with over its setting's, or nothing. */
function leversText(levers: AtmosphereLeversOverride | null): string {
  if (levers === null) {
    return "";
  }
  const named = [
    ...(levers.marchCover === undefined ? [] : [`march cover ${levers.marchCover}`]),
    ...(levers.limbRays === undefined ? [] : [`limb rays ${levers.limbRays}`]),
    ...(levers.marchSourceTransmittance === undefined
      ? []
      : [`march sources ${levers.marchSourceTransmittance}`]),
  ];
  return `; atmosphere levers ${named.join(", ")} over the setting's`;
}

/** The timing file's summary, for a person reading it. */
export function inspectionTimingMarkdown(results: InspectionTimingResults): string {
  const { run } = results;
  const gpu =
    run.machine.gpu.value === null
      ? `— (${run.machine.gpu.reason})`
      : (run.machine.gpu.value.description ??
        `${String(run.machine.gpu.value.vendorId)}:${String(run.machine.gpu.value.deviceId)}`);
  return [
    `# Inspection frame profile: ${run.machine.name}, ${run.setting}, ${run.startedAt.slice(0, 10)}`,
    "",
    `- **Body:** ${run.served === null ? "—" : `\`${run.served.body}\` of universe \`${run.served.universe}\``}`,
    `- **Machine:** ${cell(run.machine.cpu)}, ${String(run.machine.logicalCores)} threads; GPU ${cell(gpu)}`,
    `- **Versions:** app ${run.versions.app}, Electron ${run.versions.electron}, Chromium ${run.versions.chromium}`,
    `- **Launch:** ${run.platform}, ${run.launchMode} mode, setting ${run.setting}, window ${run.shown ? "shown" : "hidden"} ${String(run.window.widthDip)} × ${String(run.window.heightDip)} DIP at a device-pixel ratio of ${String(run.devicePixelRatio)}; pass timer ${run.timer}${run.compositeProbe === null ? "" : `; composite probe ${run.compositeProbe}, not the picture a player sees`}${run.marchProbe === null ? "" : `; march probe ${run.marchProbe}, not the picture a player sees`}${leversText(run.atmosphereLevers)}${run.litProbe === null ? "" : `; lit probe ${run.litProbe}, not the picture a player sees`}`,
    "- **A profile, not a measurement:** nothing is judged; a hidden run's frames are never presented, and the machine may be shared.",
    "",
    ...results.windows.flatMap(windowMarkdown),
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
 * Writes the timing file and its summary into `dir` as
 * `<date>-<machine>-<setting>-inspection-timing.json` and `.md` (with `-composite-<probe>` and
 * `-march-<probe>` after it for a probed run, `-cover-<cover>` and `-limb-<rays>` for one drawn
 * with a lever over its setting's, and `-lit-<probe>` for one drawn under the lit probe), adding
 * `-2`, `-3` and so on where a run of the same name is there, and warns of a file over the
 * repository's limit for added files.
 *
 * @throws Error if the day's names are used up.
 */
export async function writeInspectionTimingResults(
  dir: string,
  results: InspectionTimingResults,
  files: ResultsFiles = NODE_FILES,
): Promise<{ readonly json: string; readonly markdown: string }> {
  await files.mkdir(dir);
  const probe = results.run.compositeProbe;
  const march = results.run.marchProbe;
  const levers = results.run.atmosphereLevers;
  const lit = results.run.litProbe;
  const stem = `${results.run.startedAt.slice(0, 10)}-${results.run.machine.name}-${results.run.setting}-inspection-timing${probe === null ? "" : `-composite-${probe}`}${march === null ? "" : `-march-${march}`}${levers?.marchCover === undefined ? "" : `-cover-${levers.marchCover}`}${levers?.limbRays === undefined ? "" : `-limb-${levers.limbRays}`}${levers?.marchSourceTransmittance === undefined ? "" : `-sources-${levers.marchSourceTransmittance}`}${lit === null ? "" : `-lit-${lit}`}`;
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
  await files.writeFile(markdown, inspectionTimingMarkdown(results));
  const bytes = Buffer.byteLength(formatAsPrettier(text), "utf8");
  if (bytes > ADDED_FILE_LIMIT_BYTES) {
    console.warn(
      `views check: ${json} is ${String(bytes)} B once formatted, over the ${String(ADDED_FILE_LIMIT_BYTES)} B the repository accepts for an added file`,
    );
  }
  return { json, markdown };
}
