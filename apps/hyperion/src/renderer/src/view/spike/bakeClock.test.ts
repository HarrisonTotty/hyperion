import { describe, expect, it } from "vitest";

import { bakeOf, ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import type { PatchKey } from "../terrain/patchKey";
import type { BakeSettings, HeightWorkerRequest } from "../terrain/workers/messages";
import { HeightWorkerPool, type WorkerLike } from "../terrain/workers/pool";
import { BakeClock, msSpread } from "./bakeClock";

const SETTINGS: BakeSettings = {
  vertexPath: "face-differences",
  normals: "mesh",
  source: { kind: "test-planet", ridges: "off" },
};

function keyAt(i: number): PatchKey {
  return { face: 0, level: 10, i, j: 0 };
}

function bakeRequest(id: number): HeightWorkerRequest {
  return { kind: "bake", id, key: keyAt(id), generation: 1, settings: SETTINGS };
}

/** A clock on a time the test sets, and a worker it wraps. */
function clocked(segment?: () => string | null): {
  readonly clock: BakeClock;
  readonly time: { nowMs: number };
  readonly wrap: () => { readonly inner: ScriptedHeightWorker; readonly outer: WorkerLike };
} {
  const time = { nowMs: 0 };
  const clock = new BakeClock({
    nowMs: () => time.nowMs,
    ...(segment === undefined ? {} : { segment }),
  });
  return {
    clock,
    time,
    wrap: () => {
      const inner = new ScriptedHeightWorker();
      return { inner, outer: clock.wrap(inner) };
    },
  };
}

/** Answers bake `id` on `worker` with a bake. */
function bake(worker: ScriptedHeightWorker, id: number): void {
  worker.reply({ kind: "baked", id, bake: bakeOf(keyAt(id), 1) });
}

describe("the bake clock's times (decision-r10-t14b-measure §6(c))", () => {
  it("times each bake from its post to its reply, and the worker's own time from its last reply", () => {
    const { clock, time, wrap } = clocked();
    const { inner, outer } = wrap();
    outer.postMessage(bakeRequest(1), []);
    outer.postMessage(bakeRequest(2), []);
    time.nowMs = 10;
    bake(inner, 1);
    time.nowMs = 25;
    bake(inner, 2);
    const total = clock.summary().total;
    expect({ postToReply: total.postToReplyMs, compute: total.computeMs }).toEqual({
      postToReply: { p50: 10, p95: 25, max: 25 },
      compute: { p50: 10, p95: 15, max: 15 },
    });
  });

  it("counts a field message's answer as the worker's time before the next bake", () => {
    const { clock, time, wrap } = clocked();
    const { inner, outer } = wrap();
    time.nowMs = 30;
    outer.postMessage({ kind: "field-release", id: 7, body: "0000000000000001" }, []);
    time.nowMs = 35;
    outer.postMessage(bakeRequest(3), []);
    time.nowMs = 40;
    inner.answerFields();
    time.nowMs = 50;
    bake(inner, 3);
    expect(clock.summary().total).toMatchObject({
      postToReplyMs: { max: 15 },
      computeMs: { max: 10 },
    });
  });

  it("keeps each worker's order apart", () => {
    const { clock, time, wrap } = clocked();
    const a = wrap();
    const b = wrap();
    a.outer.postMessage(bakeRequest(1), []);
    b.outer.postMessage(bakeRequest(2), []);
    a.outer.postMessage(bakeRequest(3), []);
    time.nowMs = 10;
    bake(a.inner, 1);
    time.nowMs = 20;
    bake(a.inner, 3);
    time.nowMs = 30;
    bake(b.inner, 2);
    const byWorker = clock.summary().byWorker.map((w) => [w.worker, w.posted, w.computeMs.max]);
    expect(byWorker).toEqual([
      [0, 2, 10],
      [1, 1, 30],
    ]);
  });

  it("forwards every message unchanged, timing the pool's bakes before it posts the next", () => {
    const { clock, time } = clocked();
    const workers: ScriptedHeightWorker[] = [];
    const pool = new HeightWorkerPool({
      workers: 1,
      createWorker: () => {
        const worker = new ScriptedHeightWorker();
        workers.push(worker);
        return clock.wrap(worker);
      },
      bake: SETTINGS,
    });
    const baked: number[] = [];
    pool.onBaked((patch) => baked.push(patch.key.i));
    pool.reprioritise([1, 2, 3].map((i) => ({ key: keyAt(i), priority: 4 - i, forced: false })));
    const [worker] = workers;
    if (worker === undefined) {
      throw new Error("the pool made its worker");
    }
    for (const nowMs of [10, 20, 35]) {
      time.nowMs = nowMs;
      worker.bakeOldest();
    }
    const total = clock.summary().total;
    expect({
      baked,
      posted: total.posted,
      postToReply: total.postToReplyMs,
      compute: total.computeMs,
    }).toEqual({
      baked: [1, 2, 3],
      posted: 3,
      postToReply: { p50: 20, p95: 25, max: 25 },
      compute: { p50: 10, p95: 15, max: 15 },
    });
    pool.terminate();
  });
});

describe("the bake clock's counts (decision-r10-t14b-measure §6(c), §6(e))", () => {
  it("counts posts and replies by kind, refusals apart from bakes", () => {
    const { clock, wrap } = clocked();
    const { inner, outer } = wrap();
    for (const id of [1, 2, 3]) {
      outer.postMessage(bakeRequest(id), []);
    }
    outer.postMessage({ kind: "field-release", id: 9, body: "0000000000000001" }, []);
    bake(inner, 1);
    inner.reply({ kind: "bake-failed", id: 2, message: "trap" });
    inner.reply({ kind: "bake-unsurveyed", id: 3, message: "later" });
    inner.answerFields();
    const summary = clock.summary();
    expect({
      total: [
        summary.total.posted,
        summary.total.baked,
        summary.total.failed,
        summary.total.unsurveyed,
      ],
      requests: [summary.requests.bake, summary.requests["field-release"]],
      replies: [
        summary.replies.baked,
        summary.replies["bake-failed"],
        summary.replies["bake-unsurveyed"],
        summary.replies["field-held"],
      ],
      inFlight: summary.inFlight,
    }).toEqual({ total: [3, 1, 1, 1], requests: [3, 1], replies: [1, 1, 1, 1], inFlight: 0 });
  });

  it("leaves out the bakes posted before the flight's start, while their replies still free the worker", () => {
    const { clock, time, wrap } = clocked();
    const { inner, outer } = wrap();
    outer.postMessage(bakeRequest(1), []);
    time.nowMs = 5;
    clock.markFlightStart();
    time.nowMs = 6;
    outer.postMessage(bakeRequest(2), []);
    time.nowMs = 10;
    bake(inner, 1);
    time.nowMs = 20;
    bake(inner, 2);
    const summary = clock.summary();
    expect({
      start: summary.flightStartMs,
      posted: summary.total.posted,
      baked: summary.total.baked,
      postToReply: summary.total.postToReplyMs.max,
      compute: summary.total.computeMs.max,
      requests: summary.requests.bake,
      replies: summary.replies.baked,
      span: summary.total.spanMs,
    }).toEqual({
      start: 5,
      posted: 1,
      baked: 1,
      postToReply: 14,
      compute: 10,
      requests: 1,
      replies: 2,
      span: 15,
    });
  });

  it("gives each segment its bakes, residents and rate, and the bakes in each window", () => {
    let segment: string | null = "approach";
    const { clock, time, wrap } = clocked(() => segment);
    const { inner, outer } = wrap();
    clock.markFlightStart();
    for (const id of [1, 2, 3]) {
      outer.postMessage(bakeRequest(id), []);
    }
    time.nowMs = 500;
    bake(inner, 1);
    clock.noteResident();
    time.nowMs = 1_000;
    bake(inner, 2);
    segment = "final";
    time.nowMs = 2_500;
    bake(inner, 3);
    clock.noteResident();
    clock.noteResident();
    const summary = clock.summary();
    expect(
      summary.bySegment.map((s) => [
        s.segment,
        s.posted,
        s.baked,
        s.residents,
        s.bakedNeverResident,
        s.bakesPerSecond,
      ]),
    ).toEqual([
      ["approach", 3, 2, 1, 1, 2],
      ["final", 0, 1, 2, -1, null],
    ]);
    expect(summary.total).toMatchObject({ baked: 3, residents: 3, bakesPerSecond: 3 / 2.5 });
    expect(clock.bakedPerWindow(1_000)).toEqual([1, 1, 1]);
    expect(() => clock.bakedPerWindow(0)).toThrow(RangeError);
  });

  it("counts the bakes in flight on a worker that fails or is terminated as abandoned", () => {
    const { clock, wrap } = clocked();
    const failing = wrap();
    const ended = wrap();
    failing.outer.postMessage(bakeRequest(1), []);
    failing.outer.postMessage(bakeRequest(2), []);
    ended.outer.postMessage(bakeRequest(3), []);
    failing.inner.fail();
    ended.outer.terminate();
    const summary = clock.summary();
    expect({
      abandoned: summary.abandoned,
      inFlight: summary.inFlight,
      terminated: ended.inner.terminated,
    }).toEqual({ abandoned: 3, inFlight: 0, terminated: true });
  });

  it("gives no per-segment figures without a segment callback", () => {
    const { clock, wrap } = clocked();
    const { inner, outer } = wrap();
    outer.postMessage(bakeRequest(1), []);
    bake(inner, 1);
    expect(clock.summary().bySegment).toEqual([]);
  });
});

describe("msSpread", () => {
  it("takes the percentiles by nearest rank, and nulls of no values", () => {
    const values = Array.from({ length: 20 }, (_, n) => 20 - n);
    expect([msSpread(values), msSpread([])]).toEqual([
      { p50: 10, p95: 19, max: 20 },
      { p50: null, p95: null, max: null },
    ]);
  });
});
