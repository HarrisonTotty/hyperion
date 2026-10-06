import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeAll, describe, expect, it } from "vitest";

import {
  FRAME_MEASURE,
  GPU_PROCESS_SLICES,
  reduceTraceFile,
  type TraceFigures,
} from "./reduceTrace";
import { spikeTraceConfig } from "./spike";
import {
  decodeProtoPackets,
  decodeProtoTrace,
  type ProtoTraceEvent,
  readByReducer,
  readProtoTraceEvents,
  tracePackets,
  trimProtoTrace,
} from "./traceProto";
import { FRAME_SPAN_IDENTITY_MS, FRAME_SPAN_TOLERANCE_MS, frameSpanFailure } from "./traceWindows";

/*
 * Provenance of the fixtures (R05.T14.h, decision-r05-trace-windows-2.md and its addendum A).
 *
 * Recorded on 2026-10-05 from Electron 44.4.3 (Chromium 152.0.7977.130, V8 15.2.124.28) on the RTX
 * 3080 (NVIDIA 615.71.09, Linux, X `:0` with the window hidden), from two short hidden partial
 * runs of the descent spike at the low setting, seed 7, each ended at 30 s of script time, so one
 * trace window over the orbit coast. The tree was this task's parent (e22a2f8) with the renderer's
 * `callbackStartsMs` series, and lane D's uncommitted `experiment.patch` applied for the recording
 * only: the trace over CDP's `Tracing` domain as a protobuf stream (`HYPERION_T14F_CDP=proto`),
 * each window's file kept (`HYPERION_T14F_KEEP`), the renderer's report dumped
 * (`HYPERION_T14F_REPORT`) and the 30-s cap. After `pnpm --filter hyperion build` and
 * `cargo build --release -p hyperion-server`, each run was
 *
 *   HYPERION_SPIKE_PORT=7893 HYPERION_T14F_CDP=proto HYPERION_T14F_KEEP=<dir> \
 *   HYPERION_T14F_REPORT=<file> bash apps/hyperion/scripts/descentSpike.sh \
 *     --setting low --seed 7 --hidden --out <dir> [--trace-profile on]
 *
 * in a capped `systemd-run --user --scope` (8G). The timed run's window is 24,584,692 bytes (2.5 %
 * of its 768 MiB buffer, no data loss), the profiled run's 50,582,974 (5.2 %). The fixtures are cut
 * from them with this module's own `trimProtoTrace`:
 *
 * - `spike.pftrace`: the timed window from 20.0 s to 20.5 s after its first event (at
 *   29,242,496,762.752 µs), `trimProtoTrace(window, first + 20.5e6, first + 20.0e6)`: 388,210 bytes,
 *   steady state, since the window's first 0.3 s are the page's start-up (its GC and slow frames);
 * - `spike-profiled.pftrace`: the profiled window's first 0.21 s (its first event at
 *   29,306,544,082.197 µs), `trimProtoTrace(window, first + 0.21e6)`: 357,362 bytes, a prefix, so
 *   that it holds the CPU profiles' `Profile` events, which only a window's start has;
 * - `spike.pftrace.report.json`: the timed run's report cut to the 27 frames whose `spike.frame`
 *   spans `spike.pftrace` holds: `scriptStartMs`, its window, and each frame's script time,
 *   `ourCodeMs` and `callbackStartsMs`.
 *
 * Recorded on the untrimmed windows, not tests (the plan's T14.h as-built record):
 * - The frames by identity: of the timed report's 1,753 frames, 1,752 have exactly one span whose
 *   `startTime` is their callback start (the first frame's callback ran before the trace began),
 *   and no span is left over; the profiled run's, 1,618 of 1,619. The largest |duration −
 *   `ourCodeMs`| is 0.0010 ms in both, and each begin and end lies within 0.0010 ms of the values
 *   passed: Chromium writes the endpoints as passed (clamped to 0.1 ms), not unclamped.
 * - `trace_processor_shell` v58.2 on both windows: per process and thread, the counts and summed
 *   durations of `RunTask`, `GPUTask`, `MinorGC`, `MajorGC`, `WebGPU` and
 *   `VulkanQueueSubmitHook` equal the decoder's to the nanosecond (23 and 25 groups). Its
 *   `PipelineReporter` counts equal the decoder's (3,549 and 3,273), but 2,353 of the timed
 *   window's are 1–11 µs longer in it, since it sorts each track's ends by time and so ends a
 *   reporter at a child's `Swap` end written 1 µs after the reporter's own.
 * - The decoder reduced the timed window in 1.0 s at a peak of 315 MB, about 25 MB/s.
 */

const FIXTURES = join(__dirname, "fixtures");
const TIMED_FIXTURE = join(FIXTURES, "spike.pftrace");
const PROFILED_FIXTURE = join(FIXTURES, "spike-profiled.pftrace");
const REPORT_FIXTURE = join(FIXTURES, "spike.pftrace.report.json");
const JSON_FIXTURE = join(FIXTURES, "spike.trace.json");

/** A timed run's categories, and a profiled run's. */
const TIMED = { categories: spikeTraceConfig().included_categories ?? [] };
const PROFILED = { categories: spikeTraceConfig({ profiled: true }).included_categories ?? [] };

/** The timed fixture's renderer, its main thread, and the GPU process, by their recorded IDs. */
const RENDERER_PID = 1_370_674;
const RENDERER_MAIN_TID = 1;
const GPU_PID = 1_370_648;

/** The report fixture's frames. */
interface FixtureReport {
  readonly frames: {
    readonly scriptTimesS: ReadonlyArray<number>;
    readonly ourCodeMs: ReadonlyArray<number>;
    readonly callbackStartsMs: ReadonlyArray<number>;
  };
}

function numbers(value: unknown): ReadonlyArray<number> {
  if (!Array.isArray(value) || !value.every((x): x is number => typeof x === "number")) {
    throw new Error("the report fixture's series is not a list of numbers");
  }
  return value;
}

function field(value: unknown, key: string): unknown {
  return typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;
}

async function fixtureReport(): Promise<FixtureReport> {
  const parsed: unknown = JSON.parse(await readFile(REPORT_FIXTURE, "utf8"));
  const frames = field(parsed, "frames");
  return {
    frames: {
      scriptTimesS: numbers(field(frames, "scriptTimesS")),
      ourCodeMs: numbers(field(frames, "ourCodeMs")),
      callbackStartsMs: numbers(field(frames, "callbackStartsMs")),
    },
  };
}

async function eventsOf(path: string, readBytes?: number): Promise<ProtoTraceEvent[]> {
  const events: ProtoTraceEvent[] = [];
  const options = readBytes === undefined ? {} : { readBytes };
  for await (const event of readProtoTraceEvents(path, options)) {
    events.push(event);
  }
  return events;
}

/** A trace's main thread, known present. */
function mainOf(trace: TraceFigures): NonNullable<TraceFigures["mainThread"]> {
  if (trace.mainThread === null) {
    throw new Error("the trace has no renderer main thread");
  }
  return trace.mainThread;
}

/** Whether an event is a GC slice as the reducer counts one. */
function isGc(event: ProtoTraceEvent): boolean {
  return (
    event.ph === "X" &&
    (event.cat.split(",").some((category) => category.endsWith("v8.gc")) ||
      event.name === "MinorGC" ||
      event.name === "MajorGC")
  );
}

describe("the protobuf decoder on a recorded timed window", () => {
  let figures: TraceFigures | undefined;
  let events: ProtoTraceEvent[] = [];
  let report: FixtureReport | undefined;

  beforeAll(async () => {
    figures = await reduceTraceFile(TIMED_FIXTURE, TIMED);
    events = await eventsOf(TIMED_FIXTURE);
    report = await fixtureReport();
  });

  function reduced(): TraceFigures {
    if (figures === undefined) {
      throw new Error("the fixture was not reduced");
    }
    return figures;
  }

  function frames(): FixtureReport["frames"] {
    if (report === undefined) {
      throw new Error("the report fixture was not read");
    }
    return report.frames;
  }

  it("finds the renderer, its CrRendererMain, the GPU process and its CrGpuMain", () => {
    const trace = reduced();
    expect(trace.frames.pid).toBe(RENDERER_PID);
    expect(trace.mainThread).toMatchObject({ pid: RENDERER_PID, tid: RENDERER_MAIN_TID });
    expect(trace.gpuProcess).toMatchObject({ pid: GPU_PID, tid: GPU_PID });
    const named = trace.threads.map(({ pid, tid, process, thread }) => [pid, tid, process, thread]);
    expect(named).toContainEqual([RENDERER_PID, RENDERER_MAIN_TID, "Renderer", "CrRendererMain"]);
    expect(named).toContainEqual([GPU_PID, GPU_PID, "GPU Process", "CrGpuMain"]);
  });

  it("holds one spike.frame span for each of the renderer's frames, by its start, of its ourCodeMs", () => {
    const { callbackStartsMs, ourCodeMs } = frames();
    const spans = mainOf(reduced()).frameSpans;
    expect(callbackStartsMs).toHaveLength(27);
    expect(spans.startTimesMs).toHaveLength(27);
    for (const [i, startMs] of callbackStartsMs.entries()) {
      const matching = spans.startTimesMs.flatMap((spanStartMs, k) =>
        spanStartMs !== null && Math.abs(spanStartMs - startMs) <= FRAME_SPAN_IDENTITY_MS
          ? [k]
          : [],
      );
      expect(matching).toHaveLength(1);
      const deviationMs = Math.abs(
        (spans.durationsMs[matching[0] ?? -1] ?? Number.NaN) - (ourCodeMs[i] ?? 0),
      );
      expect(deviationMs).toBeLessThanOrEqual(FRAME_SPAN_TOLERANCE_MS);
    }
  });

  it("passes the merge's frame-span check, every frame 0.5 s inside a window around them", () => {
    const { callbackStartsMs } = frames();
    const window = {
      startedMs: (callbackStartsMs[0] ?? 0) - 500,
      stopRequestedMs: (callbackStartsMs.at(-1) ?? 0) + 500,
      failure: null,
    };
    expect(frameSpanFailure(reduced(), window, { frames: frames() })).toBeNull();
  });

  it("writes each span's ends as the page passed them, within 0.002 ms, not unclamped", () => {
    const { callbackStartsMs, ourCodeMs } = frames();
    const trace = reduced();
    const spans = mainOf(trace).frameSpans;
    const offsetUs = trace.clockOffsetUs ?? Number.NaN;
    for (const [k, startUs] of spans.startsUs.entries()) {
      const i = callbackStartsMs.findIndex((ms) => ms === spans.startTimesMs[k]);
      const beginMs = (startUs - offsetUs) / 1000;
      const endMs = beginMs + (spans.durationsMs[k] ?? Number.NaN);
      expect(Math.abs(beginMs - (callbackStartsMs[i] ?? Number.NaN))).toBeLessThanOrEqual(0.002);
      expect(
        Math.abs(endMs - (callbackStartsMs[i] ?? Number.NaN) - (ourCodeMs[i] ?? Number.NaN)),
      ).toBeLessThanOrEqual(0.002);
    }
  });

  it("sets the window's clock against the page's by its begins, which agree within 0.2 ms", () => {
    const offsetsUs = events.flatMap((event) => {
      const startMs = event.args["startTime"];
      return event.ph === "b" &&
        event.cat === "blink.user_timing" &&
        event.pid === RENDERER_PID &&
        event.tid === RENDERER_MAIN_TID &&
        typeof startMs === "number"
        ? [event.ts - 1000 * startMs]
        : [];
    });
    expect(offsetsUs.length).toBeGreaterThanOrEqual(27);
    const offsetUs = reduced().clockOffsetUs ?? Number.NaN;
    for (const us of offsetsUs) {
      expect(Math.abs(us - offsetUs)).toBeLessThanOrEqual(200);
    }
  });

  it("starts each spike.frame span 0–17 ms after a PipelineReporter begin on the busiest compositor", () => {
    // Across sequences: the spans are the main thread's, the reporters the compositor's. A frame's
    // reporter is written when the frame is presented, so the spans of the fixture's last 100 ms
    // may have theirs after it.
    const trace = reduced();
    const lastUs = trace.span?.lastUs ?? 0;
    const beginsUs = events
      .filter(
        (event) =>
          event.name === "PipelineReporter" &&
          event.ph === "b" &&
          event.pid === trace.frames.pid &&
          field(event.args["frame_reporter"], "layer_tree_host_id") ===
            trace.frames.layerTreeHostId,
      )
      .map(({ ts }) => ts)
      .toSorted((a, b) => a - b);
    const starts = mainOf(trace).frameSpans.startsUs.filter((us) => us <= lastUs - 100_000);
    expect(starts.length).toBeGreaterThanOrEqual(20);
    for (const startUs of starts) {
      const beginUs = beginsUs.findLast((us) => us <= startUs) ?? Number.NEGATIVE_INFINITY;
      expect((startUs - beginUs) / 1000).toBeLessThanOrEqual(17);
    }
  });

  it("places every spike.frame span inside a RunTask on CrRendererMain", () => {
    // Its end within 1 µs; its start within the duration's tolerance, since the start the page
    // passed is clamped to `performance.now()`'s 0.1 ms grid and may precede the task's.
    const tasks = events.filter(
      (event) =>
        event.ph === "X" &&
        event.name === "RunTask" &&
        event.pid === RENDERER_PID &&
        event.tid === RENDERER_MAIN_TID,
    );
    const spans = mainOf(reduced()).frameSpans;
    for (const [k, startUs] of spans.startsUs.entries()) {
      const endUs = startUs + 1000 * (spans.durationsMs[k] ?? 0);
      const inside = tasks.some(
        ({ ts, dur = 0 }) =>
          ts <= startUs + 1000 * FRAME_SPAN_TOLERANCE_MS && ts + dur >= endUs - 1,
      );
      expect(inside).toBe(true);
    }
  });

  it("decodes the PipelineReporter states to the JSON's names", () => {
    const states = events
      .filter(({ name, ph }) => name === "PipelineReporter" && ph === "b")
      .map(({ args }) => field(args["frame_reporter"], "state"));
    expect(new Set(states)).toEqual(
      new Set(["STATE_PRESENTED_ALL", "STATE_PRESENTED_PARTIAL", "STATE_DROPPED"]),
    );
    expect(reduced().frames).toMatchObject({ presented: 28, dropped: 2, noUpdate: 0 });
  });

  it("gives complete GC slices on the renderer, its pauses of positive durations", () => {
    const gc = events.filter((event) => isGc(event) && event.pid === RENDERER_PID);
    expect(gc.length).toBeGreaterThan(0);
    // V8's phases are timed in whole µs, so a few are 0 µs long.
    for (const { dur } of gc) {
      expect(dur).toBeGreaterThanOrEqual(0);
    }
    const pauses = gc.filter(({ name }) => name === "MinorGC" || name === "MajorGC");
    expect(pauses.map(({ name }) => name).toSorted()).toEqual(["MajorGC", "MinorGC", "MinorGC"]);
    for (const { dur, tid } of pauses) {
      expect(dur).toBeGreaterThan(0);
      expect(tid).toBe(RENDERER_MAIN_TID);
    }
  });

  it("gives complete GPUTask slices on the GPU process, of positive durations", () => {
    const gpuTasks = events.filter(({ name }) => name === "GPUTask");
    expect(gpuTasks.length).toBeGreaterThan(0);
    for (const { ph, dur, pid } of gpuTasks) {
      expect([ph, pid]).toEqual(["X", GPU_PID]);
      expect(dur).toBeGreaterThan(0);
    }
  });

  it("reads the same events 8 KiB at a time as at its default read size", async () => {
    expect(await eventsOf(TIMED_FIXTURE, 8 * 1024)).toEqual(events);
  });

  it("cuts each sequence at a time into a trace that decodes as the whole one's prefix", async () => {
    const bytes = new Uint8Array(await readFile(TIMED_FIXTURE));
    const untilUs = (reduced().span?.firstUs ?? 0) + 250_000;
    const expected: ProtoTraceEvent[] = [];
    const ended = new Set<number>();
    for (const packet of decodeProtoPackets(bytes)) {
      if (ended.has(packet.sequence)) {
        continue;
      }
      if (packet.timeUs !== null && packet.timeUs > untilUs) {
        ended.add(packet.sequence);
        continue;
      }
      expected.push(...packet.events);
    }
    const cut = trimProtoTrace(bytes, untilUs);
    expect(cut.length).toBeLessThan(bytes.length);
    expect(expected.length).toBeGreaterThan(1000);
    expect([...decodeProtoTrace(cut)]).toEqual(expected);
  });
});

describe("the protobuf decoder on a recorded profiled window", () => {
  let figures: TraceFigures | undefined;
  let events: ProtoTraceEvent[] = [];

  beforeAll(async () => {
    figures = await reduceTraceFile(PROFILED_FIXTURE, PROFILED);
    events = await eventsOf(PROFILED_FIXTURE);
  });

  function reduced(): TraceFigures {
    if (figures === undefined) {
      throw new Error("the fixture was not reduced");
    }
    return figures;
  }

  it("summarises the GPU process's WebGPU and VulkanQueueSubmitHook slices", () => {
    const slices = reduced().gpuProcess?.slices ?? [];
    expect(slices.map(({ name }) => name)).toEqual(["WebGPU", "GPUTask", "VulkanQueueSubmitHook"]);
    for (const { count, totalMs } of slices) {
      expect(count).toBeGreaterThan(0);
      expect(totalMs).toBeGreaterThan(0);
    }
  });

  it("samples the main thread over the span its profile was sampled, within 10 %", () => {
    // V8 began the main thread's profile 90 ms into the window and took its first sample 95 ms
    // later, so its samples cover the fixture's last 24 ms: from the profile's start plus its
    // first delta to its last chunk, which carries its samples to within 0.2 ms of its time.
    const main = mainOf(reduced());
    const profile = events.find(
      ({ name, pid, tid }) => name === "Profile" && pid === main.pid && tid === main.tid,
    );
    const chunks = events.filter(
      ({ name, pid, id }) => name === "ProfileChunk" && pid === main.pid && id === profile?.id,
    );
    expect(chunks.length).toBeGreaterThan(10);
    const startUs = field(profile?.args["data"], "startTime");
    const [firstDeltaUs] = numbers(field(chunks[0]?.args["data"], "timeDeltas"));
    const sampledOverMs = ((chunks.at(-1)?.ts ?? 0) - Number(startUs) - (firstDeltaUs ?? 0)) / 1000;
    expect(main.sampledMs).not.toBeNull();
    expect(Math.abs((main.sampledMs ?? 0) - sampledOverMs)).toBeLessThanOrEqual(
      0.1 * sampledOverMs,
    );
    expect(main.engineSelfMs).toBeGreaterThan(0);
    expect(main.engineSelfMs).toBeLessThanOrEqual(main.sampledMs ?? 0);
  });
});

/** Protobuf encoding of the few fields the hand-made traces need. */
function varint(value: number | bigint): number[] {
  const out: number[] = [];
  // A negative value as its 64-bit two's complement, ten bytes.
  let rest = BigInt.asUintN(64, BigInt(value));
  while (rest >= 0x80n) {
    out.push(Number(rest % 0x80n) | 0x80);
    rest /= 0x80n;
  }
  out.push(Number(rest));
  return out;
}

function int(fieldNumber: number, value: number | bigint): number[] {
  return [...varint(fieldNumber * 8), ...varint(value)];
}

function message(fieldNumber: number, ...fields: ReadonlyArray<number[]>): number[] {
  const body = fields.flat();
  return [...varint(fieldNumber * 8 + 2), ...varint(body.length), ...body];
}

function text(fieldNumber: number, value: string): number[] {
  const body = [...new TextEncoder().encode(value)];
  return [...varint(fieldNumber * 8 + 2), ...varint(body.length), ...body];
}

/** The tracing service's packet: a snapshot naming MONOTONIC (clock 3) the trace's clock. */
const SNAPSHOT = message(1, message(6, message(1, int(1, 3), int(2, 1)), int(2, 3)), int(10, 1));

/**
 * A packet of sequence 2 at `atNs` that clears its state: its defaults (clock 3, its thread's
 * track) and `RunTask` interned.
 */
function clearing(atNs: number): number[] {
  return message(
    1,
    int(10, 2),
    int(13, 1),
    int(8, atNs),
    message(59, int(58, 3), message(11, int(11, 10))),
    message(
      12,
      message(1, int(1, 1), text(2, "disabled-by-default-devtools.timeline")),
      message(2, int(1, 1), text(2, "RunTask")),
    ),
  );
}

/** Sequence 2's process and thread tracks: renderer 5's `CrRendererMain`, thread 6. */
function tracks(atNs: number): number[][] {
  return [
    message(
      1,
      int(10, 2),
      int(13, 2),
      int(8, atNs),
      message(60, int(1, 9), message(3, int(1, 5), text(6, "Renderer"))),
    ),
    message(
      1,
      int(10, 2),
      int(13, 2),
      int(8, atNs),
      message(
        60,
        int(1, 10),
        int(5, 9),
        message(4, int(1, 5), int(2, 6), text(5, "CrRendererMain")),
      ),
    ),
  ];
}

/** A packet of sequence 2 at `atNs` holding a `TrackEvent` of `fields`, and `extra`. */
function eventAt(atNs: number, fields: number[], extra: number[] = []): number[] {
  return message(1, int(10, 2), int(13, 2), int(8, atNs), message(11, fields), extra);
}

/** A `RunTask` begin, and a slice's end. */
const RUN_TASK_BEGIN = [...int(9, 1), ...int(10, 1), ...int(3, 1)];
const SLICE_END = int(9, 2);

/**
 * A trace of one renderer's main thread, its state cleared at 0.9 s and `events` (each a
 * `TrackEvent`'s fields) at 1 s, 1.0005 s, …, each packet carrying `extra` too.
 */
function handMade(events: ReadonlyArray<number[]>, extra: number[] = []): Uint8Array {
  return new Uint8Array(
    [
      SNAPSHOT,
      clearing(0.9e9),
      ...tracks(0.9e9),
      ...events.map((event, i) => eventAt(1e9 + 500_000 * i, event, extra)),
    ].flat(),
  );
}

/** The starts of a trace's complete events, µs. */
function taskStarts(bytes: Uint8Array): ReadonlyArray<number> {
  return [...decodeProtoTrace(bytes)].filter(({ ph }) => ph === "X").map(({ ts }) => ts);
}

describe("hand-made and damaged traces", () => {
  const dirs: string[] = [];

  afterEach(async () => {
    await Promise.all(dirs.splice(0).map((dir) => rm(dir, { recursive: true })));
  });

  /** `bytes` written to a file of its own, removed after the test. */
  async function written(bytes: Uint8Array): Promise<string> {
    const dir = await mkdtemp(join(tmpdir(), "hyperion-pftrace-"));
    dirs.push(dir);
    const path = join(dir, "trace.pftrace");
    await writeFile(path, bytes);
    return path;
  }

  it("pairs a thread's slice into a complete event and names its process and thread", () => {
    expect([...decodeProtoTrace(handMade([RUN_TASK_BEGIN, SLICE_END]))]).toEqual([
      {
        name: "process_name",
        cat: "__metadata",
        ph: "M",
        pid: 5,
        tid: 0,
        ts: 0,
        args: { name: "Renderer" },
      },
      {
        name: "thread_name",
        cat: "__metadata",
        ph: "M",
        pid: 5,
        tid: 6,
        ts: 0,
        args: { name: "CrRendererMain" },
      },
      {
        name: "RunTask",
        cat: "disabled-by-default-devtools.timeline",
        ph: "X",
        pid: 5,
        tid: 6,
        ts: 1e6,
        dur: 500,
        args: {},
      },
    ]);
  });

  it("skips the fields it does not read", () => {
    const begin = [...RUN_TASK_BEGIN, ...text(500, "unread"), ...int(9999, 7)];
    const decoded = [...decodeProtoTrace(handMade([begin, SLICE_END], int(900, 1)))];
    expect(decoded.filter(({ ph }) => ph === "X")).toEqual([
      expect.objectContaining({ name: "RunTask", ts: 1e6, dur: 500 }),
    ]);
  });

  it("cuts a stretch from a later start at each sequence's first clearing of its state", () => {
    // Two generations of sequence 2's state, each with its tracks and a task.
    const trace = new Uint8Array(
      [
        SNAPSHOT,
        clearing(0.9e9),
        ...tracks(0.9e9),
        eventAt(1e9, RUN_TASK_BEGIN),
        eventAt(1.0005e9, SLICE_END),
        clearing(2e9),
        ...tracks(2e9),
        eventAt(2e9, RUN_TASK_BEGIN),
        eventAt(2.0005e9, SLICE_END),
      ].flat(),
    );
    expect(taskStarts(trace)).toEqual([1e6, 2e6]);
    // From 1.5 s: the second generation alone, which interns and describes all it uses.
    const stretch = trimProtoTrace(trace, Number.POSITIVE_INFINITY, 1.5e6);
    expect(taskStarts(stretch)).toEqual([2e6]);
    expect([...decodeProtoTrace(stretch)].filter(({ ph }) => ph === "M")).toHaveLength(2);
    // From 2.5 s, after the sequence's last clearing: none of its packets.
    expect(taskStarts(trimProtoTrace(trace, Number.POSITIVE_INFINITY, 2.5e6))).toEqual([]);
  });

  it("fails an event the reducer reads in an encoding it does not know, naming it", () => {
    const counter = [...int(9, 4), ...int(10, 1), ...int(3, 1)];
    expect(() => [...decodeProtoTrace(handMade([counter]))]).toThrow(
      /the trace's RunTask arrives in an encoding the decoder does not know: a counter, in the packet at byte \d+/,
    );
  });

  it("fails a packet that is not one with its offset", () => {
    const fixture = handMade([]);
    // A packet whose one field has wire type 7, after the fixture's packets.
    const broken = new Uint8Array([...fixture, 0x0a, 0x02, 0x0f, 0x00]);
    expect(() => [...decodeProtoTrace(broken)]).toThrow(
      `the trace's packet at byte ${fixture.length} is not a packet`,
    );
  });

  it("decodes negative integer annotations exactly, as scalars and in nested values", () => {
    // `debug_annotations` (4): `int_value` (4) of −1 and −5000, and a `nested_value` (8) dictionary
    // (`nested_type` 1) of `k` to `int_value` (5) −2.
    const begin = [
      ...RUN_TASK_BEGIN,
      ...message(4, text(10, "one"), int(4, -1)),
      ...message(4, text(10, "many"), int(4, -5000)),
      ...message(4, text(10, "dict"), message(8, int(1, 1), text(2, "k"), message(3, int(5, -2)))),
    ];
    const task = [...decodeProtoTrace(handMade([begin, SLICE_END]))].find(({ ph }) => ph === "X");
    expect(task?.args).toEqual({ one: -1, many: -5000, dict: { k: -2 } });
  });

  it("frames a trace read a byte at a time as one read whole", async () => {
    const bytes = handMade([RUN_TASK_BEGIN, SLICE_END, RUN_TASK_BEGIN, SLICE_END]);
    const path = await written(bytes);
    expect(await eventsOf(path, 1)).toEqual([...decodeProtoTrace(bytes)]);
  });

  it.each([1, 8 * 1024])(
    "fails bytes that begin no field between packets, at their offset, read %i bytes at a time",
    async (readBytes) => {
      const fixture = handMade([RUN_TASK_BEGIN, SLICE_END]);
      // A tag of wire type 7, which no field has.
      const path = await written(new Uint8Array([...fixture, 0x0f, 0x00]));
      await expect(eventsOf(path, readBytes)).rejects.toThrow(
        `${path}: the trace at byte ${fixture.length} is not a packet: a field of unknown wire type 7`,
      );
    },
  );

  it("fails a sequence that lost packets, naming it", () => {
    // `previous_packet_dropped` (42) on a packet other than the sequence's first.
    expect(() => [...decodeProtoTrace(handMade([RUN_TASK_BEGIN], int(42, 1)))]).toThrow(
      /the trace lost packets of sequence 2 before this one, in the packet at byte \d+/,
    );
  });

  it("refuses rather than skips every event the reducer reads", () => {
    // The decoder's copy of the reducer's names, against the reducer's own.
    for (const { name, category } of GPU_PROCESS_SLICES) {
      expect(readByReducer(name, category)).toBe(true);
    }
    expect(readByReducer(FRAME_MEASURE, "blink.user_timing")).toBe(true);
    for (const name of [
      "RunTask",
      "MinorGC",
      "MajorGC",
      "PipelineReporter",
      "Profile",
      "ProfileChunk",
    ]) {
      expect(readByReducer(name, "devtools.timeline")).toBe(true);
    }
    expect(readByReducer("V8.GC_SCAVENGER", "disabled-by-default-v8.gc")).toBe(true);
    expect(readByReducer("DrawFrame", "cc")).toBe(false);
  });

  it("fails a truncated packet with its offset", async () => {
    const bytes = new Uint8Array(await readFile(TIMED_FIXTURE));
    const offsets = [...tracePackets(bytes)].map(({ offset }) => offset);
    const last = offsets.at(-1) ?? 0;
    const path = await written(bytes.subarray(0, bytes.length - 3));
    await expect(eventsOf(path)).rejects.toThrow(
      `${path}: the trace ends inside a packet at byte ${last}`,
    );
  });

  it("leaves the JSON fixture to the JSON reader, which still reduces it", async () => {
    const json = await reduceTraceFile(JSON_FIXTURE, PROFILED);
    expect(json.frames.presentedAtUs).toEqual([28_792_462_819]);
    expect(json.mainThread?.frameSpans.startTimesMs).toHaveLength(7);
  });
});
