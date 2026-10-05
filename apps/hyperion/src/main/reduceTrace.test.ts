import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeAll, describe, expect, it } from "vitest";

import { readTraceEvents, reduceTrace, reduceTraceFile, type TraceFigures } from "./reduceTrace";

/**
 * A trace recorded on 2026-10-02 from Electron 44.4.3 (a WebGPU page on SwiftShader, headless and
 * offscreen, with `performance.measure` around each frame's work and an `engine-*.js` script doing
 * part of it), trimmed to 190 ms of the events the reducer reads, its process and thread IDs
 * renumbered and its script URLs rewritten. Chromium's layout, one event a line, is kept.
 */
const FIXTURE = join(__dirname, "fixtures/spike.trace.json");

/** A parsed event's field, or `undefined`. */
function field(value: unknown, key: string): unknown {
  return typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;
}

/** The fixture's renderer process and its main thread, after renumbering. */
const RENDERER_PID = 1500;
const RENDERER_MAIN_TID = 1518;

describe("the trace reducer on a recorded trace", () => {
  let figures: TraceFigures | undefined;

  beforeAll(async () => {
    figures = await reduceTraceFile(FIXTURE);
  });

  function reduced(): TraceFigures {
    if (figures === undefined) {
      throw new Error("the fixture was not reduced");
    }
    return figures;
  }

  it("finds the renderer's compositor frames and their states", () => {
    const { frames } = reduced();
    expect(frames.pid).toBe(RENDERER_PID);
    expect(frames.layerTreeHostId).toBe(1);
    // Twelve frames begin in the window and eleven end in it: two presented by one presentation
    // at 28,792,462,819 µs, one dropped, and eight the hidden page did not need to draw.
    expect(frames.presentedAtUs).toEqual([28_792_462_819]);
    expect(frames.presented).toBe(1);
    expect(frames.dropped).toBe(1);
    expect(frames.droppedAtUs).toHaveLength(1);
    expect(frames.noUpdate).toBe(8);
    expect(frames.intervalsMs).toEqual([]);
  });

  it("sets the trace's clock against the page's by its spans' starts, which agree within 0.2 ms", async () => {
    const offsetsUs: number[] = [];
    for await (const event of readTraceEvents(FIXTURE)) {
      const ts = field(event, "ts");
      const startMs = field(field(event, "args"), "startTime");
      if (
        field(event, "ph") === "b" &&
        field(event, "cat") === "blink.user_timing" &&
        field(event, "pid") === RENDERER_PID &&
        field(event, "tid") === RENDERER_MAIN_TID &&
        typeof ts === "number" &&
        typeof startMs === "number"
      ) {
        offsetsUs.push(ts - 1000 * startMs);
      }
    }
    // Eight `spike.frame` begins, `performance.now()` coarsened to 0.1 ms.
    expect(offsetsUs).toHaveLength(8);
    const sorted = offsetsUs.toSorted((a, b) => a - b);
    expect((sorted.at(-1) ?? 0) - (sorted[0] ?? 0)).toBeLessThanOrEqual(200);
    const offsetUs = reduced().clockOffsetUs;
    expect(offsetUs).toBeCloseTo(((sorted[3] ?? 0) + (sorted[4] ?? 0)) / 2, 6);
    for (const us of offsetsUs) {
      expect(Math.abs(us - (offsetUs ?? Number.NaN))).toBeLessThanOrEqual(200);
    }
  });

  it("finds the GC slices per thread, nested phases merged into one pause", () => {
    const { threads } = reduced();
    const main = threads.find(({ pid, tid }) => pid === RENDERER_PID && tid === RENDERER_MAIN_TID);
    expect(main?.thread).toBe("CrRendererMain");
    expect(main?.process).toBe("Renderer");
    // Three scavenges (MinorGC of 6.495, 3.567 and 11.287 ms, their phases inside them) and 26
    // short sweeping and safepoint slices between them.
    expect(main?.gc.count).toBe(29);
    expect(main?.gc.maxMs).toBeCloseTo(11.287, 6);
    expect(main?.gc.totalMs).toBeGreaterThan(6.495 + 3.567 + 11.287);
    const background = threads.filter(
      ({ thread, gc }) => thread === "ThreadPoolForegroundWorker" && gc.count > 0,
    );
    expect(background.length).toBeGreaterThan(0);
  });

  it("finds the page's performance.measure spans", () => {
    const { userTiming } = reduced();
    expect(userTiming.map(({ name, pid, tid }) => [name, pid, tid])).toEqual([
      ["spike.frame", RENDERER_PID, RENDERER_MAIN_TID],
    ]);
    const [spans] = userTiming;
    // Eight spans begin in the window and seven end in it.
    expect(spans?.count).toBe(7);
    expect(spans?.durationsMs).toHaveLength(7);
    expect(spans?.startsUs).toHaveLength(7);
    expect(spans?.startsUs).toEqual(spans?.startsUs.toSorted((a, b) => a - b));
    for (const ms of spans?.durationsMs ?? []) {
      expect(ms).toBeGreaterThan(0);
    }
  });

  it("splits the main thread into our code, the engine chunk and idle", () => {
    const { mainThread, span } = reduced();
    expect(span).not.toBeNull();
    expect(mainThread?.pid).toBe(RENDERER_PID);
    expect(mainThread?.tid).toBe(RENDERER_MAIN_TID);
    expect(mainThread?.wallMs).toBeCloseTo(((span?.lastUs ?? 0) - (span?.firstUs ?? 0)) / 1000, 9);
    expect(mainThread?.busyMs).toBeGreaterThan(0);
    expect(mainThread?.busyMs).toBeLessThanOrEqual(mainThread?.wallMs ?? 0);
    expect(mainThread?.idleMs).toBeCloseTo(
      (mainThread?.wallMs ?? 0) - (mainThread?.busyMs ?? 0),
      9,
    );
    // The measured spans enclose the engine script's work, which the profile samples.
    expect(mainThread?.ourCodeMs).toBeGreaterThan(0);
    expect(mainThread?.engineSelfMs).toBeGreaterThan(0);
    expect(mainThread?.engineSelfMs).toBeLessThanOrEqual(mainThread?.sampledMs ?? 0);
  });

  it("summarises the GPU process's WebGPU work", () => {
    const { gpuProcess } = reduced();
    expect(gpuProcess?.busyMs).toBeGreaterThan(0);
    const webGpu = gpuProcess?.slices.find(({ name }) => name === "WebGPU");
    expect(webGpu?.count).toBe(10);
    expect(webGpu?.totalMs).toBeCloseTo(85.87, 6);
  });

  it("reads the same events streamed as parsed whole", async () => {
    const streamed: unknown[] = [];
    for await (const event of readTraceEvents(FIXTURE)) {
      streamed.push(event);
    }
    const { readFile } = await import("node:fs/promises");
    const whole: unknown = JSON.parse(await readFile(FIXTURE, "utf8"));
    expect(whole).toMatchObject({ traceEvents: streamed });
  });
});

/** A renderer with its main and compositor threads. */
const META = [
  { ph: "M", name: "process_name", pid: 1, tid: 0, args: { name: "Renderer" } },
  { ph: "M", name: "thread_name", pid: 1, tid: 10, args: { name: "CrRendererMain" } },
  { ph: "M", name: "thread_name", pid: 1, tid: 11, args: { name: "Compositor" } },
];

/** One compositor frame's `PipelineReporter` pair. */
function frame(id: string, beginUs: number, endUs: number, state: string): unknown[] {
  const reporter = { frame_reporter: { state } };
  return [
    {
      ph: "b",
      cat: "cc",
      name: "PipelineReporter",
      pid: 1,
      tid: 11,
      ts: beginUs,
      id2: { local: id },
      args: reporter,
    },
    {
      ph: "e",
      cat: "cc",
      name: "PipelineReporter",
      pid: 1,
      tid: 11,
      ts: endUs,
      id2: { local: id },
    },
  ];
}

/** A `performance.measure` span's begin on renderer 1's thread `tid`, started at `startTime` ms. */
function begin(tid: number, ts: number, startTime: number, id: string): unknown {
  return {
    ph: "b",
    cat: "blink.user_timing",
    name: "terrain.frame",
    pid: 1,
    tid,
    ts,
    id2: { local: id },
    args: { startTime },
  };
}

/** A task on the renderer's main thread. */
function task(ts: number, dur: number): unknown {
  return {
    ph: "X",
    cat: "disabled-by-default-devtools.timeline",
    name: "RunTask",
    pid: 1,
    tid: 10,
    ts,
    dur,
  };
}

/** A CPU profile node at a script URL. */
function node(id: number, url: string): unknown {
  return { id, callFrame: { url } };
}

/** Every event of a trace file. */
async function all(path: string): Promise<unknown[]> {
  const events: unknown[] = [];
  for await (const event of readTraceEvents(path)) {
    events.push(event);
  }
  return events;
}

describe("the trace reducer on hand-made events", () => {
  it("takes frame intervals from presentation times, in time order, once a presentation", async () => {
    const figures = await reduceTrace([
      ...META,
      ...frame("0x3", 30_000, 50_000, "STATE_PRESENTED_ALL"),
      ...frame("0x1", 0, 16_700, "STATE_PRESENTED_ALL"),
      ...frame("0x2", 16_700, 33_400, "STATE_PRESENTED_PARTIAL"),
      ...frame("0x6", 40_000, 50_000, "STATE_PRESENTED_ALL"),
      ...frame("0x4", 50_000, 60_000, "STATE_DROPPED"),
      ...frame("0x5", 60_000, 70_000, "STATE_NO_UPDATE_DESIRED"),
    ]);
    expect(figures.frames.presentedAtUs).toEqual([16_700, 33_400, 50_000]);
    expect(figures.frames.intervalsMs).toEqual([16.7, 16.6]);
    expect(figures.frames).toMatchObject({ presented: 3, dropped: 1, noUpdate: 1 });
    expect(figures.frames.droppedAtUs).toEqual([60_000]);
  });

  it("takes the clock offset from the main thread's spans alone, a worker's having its own clock", async () => {
    const figures = await reduceTrace([
      ...META,
      { ph: "M", name: "thread_name", pid: 1, tid: 12, args: { name: "DedicatedWorker thread" } },
      begin(10, 5_100_000, 100, "0x1"),
      begin(10, 5_200_080, 200.03, "0x2"),
      begin(10, 5_300_090, 300, "0x3"),
      begin(12, 9_000_000, 1, "0x4"),
    ]);
    // Offsets 5,000,000, 5,000,050 and 5,000,090 µs on the main thread: their median.
    expect(figures.clockOffsetUs).toBeCloseTo(5_000_050, 3);
  });

  it("gives no clock offset without a span on the main thread", async () => {
    const figures = await reduceTrace([...META, task(1_000, 10_000)]);
    expect(figures.clockOffsetUs).toBeNull();
  });

  it("counts nested tasks once in a thread's busy time", async () => {
    const figures = await reduceTrace([
      ...META,
      task(1_000, 10_000),
      task(3_000, 3_000),
      task(21_000, 5_000),
    ]);
    expect(figures.mainThread?.busyMs).toBe(15);
    expect(figures.mainThread?.wallMs).toBe(25);
    expect(figures.mainThread?.idleMs).toBe(10);
  });

  it("attributes each profile delta to the sample before it", async () => {
    const figures = await reduceTrace([
      ...META,
      { ph: "P", name: "Profile", pid: 1, tid: 10, ts: 1, id: "0x1", args: { data: {} } },
      {
        ph: "P",
        name: "ProfileChunk",
        pid: 1,
        tid: 99,
        ts: 2,
        id: "0x1",
        args: {
          data: {
            cpuProfile: {
              nodes: [
                node(1, ""),
                node(2, "file:///app/out/renderer/assets/engine-Ab12_c.js"),
                node(3, "file:///app/out/renderer/assets/index-Zz.js"),
              ],
              samples: [2, 3, 2, 1],
            },
            timeDeltas: [5, 100, 40, 7],
          },
        },
      },
    ]);
    // Node 2 runs 100 µs then 7 µs; node 3 runs 40 µs.
    expect(figures.mainThread?.engineSelfMs).toBeCloseTo(0.107, 9);
    expect(figures.mainThread?.sampledMs).toBeCloseTo(0.147, 9);
  });

  it("gives no main thread and no frames for a trace without a renderer", async () => {
    const figures = await reduceTrace([
      { ph: "X", name: "RunTask", pid: 2, tid: 2, ts: 5, dur: 1 },
    ]);
    expect(figures.mainThread).toBeNull();
    expect(figures.gpuProcess).toBeNull();
    expect(figures.frames).toMatchObject({ pid: null, presented: 0, intervalsMs: [] });
  });
});

describe("reading a trace file", () => {
  let dir: string | undefined;

  afterEach(async () => {
    if (dir !== undefined) {
      await rm(dir, { recursive: true });
      dir = undefined;
    }
  });

  async function written(text: string): Promise<string> {
    dir = await mkdtemp(join(tmpdir(), "hyperion-trace-"));
    const path = join(dir, "trace.json");
    await writeFile(path, text);
    return path;
  }

  it("reads a trace without metadata", async () => {
    const path = await written('{"traceEvents":[\n{"name":"a"},\n{"name":"b"}]}\n');
    await expect(all(path)).resolves.toEqual([{ name: "a" }, { name: "b" }]);
  });

  it("reads a trace written on one line", async () => {
    const path = await written('{"traceEvents":[{"name":"a"}],"metadata":{}}');
    await expect(all(path)).resolves.toEqual([{ name: "a" }]);
  });

  it("reads a bare array of events", async () => {
    const path = await written('[{"name":"a"},{"name":"b"}]');
    await expect(all(path)).resolves.toEqual([{ name: "a" }, { name: "b" }]);
  });

  it("refuses a file with no trace events", async () => {
    const path = await written('{"metadata":{}}');
    await expect(all(path)).rejects.toThrow(/trace\.json has no trace events/);
  });

  it("refuses a file that is not JSON", async () => {
    const path = await written("not a trace");
    await expect(all(path)).rejects.toThrow(/trace\.json is not a trace/);
  });

  it("names the line that does not parse", async () => {
    const path = await written('{"traceEvents":[\n{"name":"a"},\n{"name":\n');
    await expect(all(path)).rejects.toThrow(/trace\.json:3 is not a trace event/);
  });
});
