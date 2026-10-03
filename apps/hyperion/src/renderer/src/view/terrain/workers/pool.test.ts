import { describe, expect, it } from "vitest";

import { vec3 } from "../../../geometry/vec3";
import { type PatchKey, patchKeyString } from "../patchKey";
import type { PatchRequest } from "../select";
import type { BakedPatch, HeightWorkerReply, HeightWorkerRequest } from "./messages";
import {
  HeightWorkerPool,
  MAX_CONSECUTIVE_WORKER_FAILURES,
  MAX_IN_FLIGHT_PER_WORKER,
  type WorkerLike,
} from "./pool";

/** A height worker driven by the test: it records what it is sent and answers when told. */
class ScriptedWorker implements WorkerLike {
  private readonly events = new EventTarget();
  private readonly wrappers = new Map<unknown, EventListenerObject>();
  /** Listeners added and not yet removed, by event type. */
  readonly listening = { message: 0, error: 0 };
  readonly received: HeightWorkerRequest[] = [];
  terminated = false;

  postMessage(message: HeightWorkerRequest): void {
    this.received.push(message);
  }

  terminate(): void {
    this.terminated = true;
  }

  addEventListener(
    type: "message",
    listener: (event: MessageEvent<HeightWorkerReply>) => void,
  ): void;
  addEventListener(type: "error", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<HeightWorkerReply>) => void) | ((event: Event) => void),
  ): void {
    this.listening[type] += 1;
    const wrapper = { handleEvent: listener };
    this.wrappers.set(listener, wrapper);
    this.events.addEventListener(type, wrapper);
  }

  removeEventListener(
    type: "message",
    listener: (event: MessageEvent<HeightWorkerReply>) => void,
  ): void;
  removeEventListener(type: "error", listener: (event: Event) => void): void;
  removeEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<HeightWorkerReply>) => void) | ((event: Event) => void),
  ): void {
    const wrapper = this.wrappers.get(listener);
    if (wrapper !== undefined) {
      this.listening[type] -= 1;
      this.wrappers.delete(listener);
      this.events.removeEventListener(type, wrapper);
    }
  }

  /** The bake requests still awaiting an answer, oldest first. */
  get pendingBakes(): Extract<HeightWorkerRequest, { kind: "bake" }>[] {
    const answered = new Set(this.answered);
    return this.received.flatMap((m) => (m.kind === "bake" && !answered.has(m.id) ? [m] : []));
  }

  private readonly answered: number[] = [];

  reply(message: HeightWorkerReply): void {
    if ("id" in message) {
      this.answered.push(message.id);
    }
    this.events.dispatchEvent(new MessageEvent("message", { data: message }));
  }

  /** Answers the oldest pending bake with a bake of its key. */
  bakeOldest(): void {
    const next = this.pendingBakes[0];
    if (next === undefined) {
      throw new Error("no bake is pending");
    }
    this.reply({ kind: "baked", id: next.id, bake: bakeOf(next.key, next.generation) });
  }

  fail(): void {
    this.events.dispatchEvent(new Event("error"));
  }

  /** Acknowledges the last field it was sent. */
  ackField(): void {
    const field = this.received.findLast((m) => m.kind === "field");
    if (field === undefined) {
      throw new Error("no field was posted");
    }
    this.reply({ kind: "field-loaded", id: field.id });
  }
}

function bakeOf(key: PatchKey, generation: number): BakedPatch {
  return {
    key,
    generation,
    originM: vec3(0, 0, 0),
    heights: new Float32Array(2),
    offsets: null,
    normals: new Float16Array(2),
    heightRangeM: [0, 0],
    boundingRadiusM: 1,
    originHeightM: 0,
    skirtDepthM: 1,
  };
}

function keyAt(i: number): PatchKey {
  return { face: 0, level: 10, i, j: 0 };
}

function req(i: number, priority: number, forced = false): PatchRequest {
  return { key: keyAt(i), priority, forced };
}

function poolOf(workers: number): {
  pool: HeightWorkerPool;
  created: ScriptedWorker[];
  baked: string[];
} {
  const created: ScriptedWorker[] = [];
  const pool = new HeightWorkerPool({
    workers,
    createWorker: () => {
      const worker = new ScriptedWorker();
      created.push(worker);
      return worker;
    },
    bake: { vertexPath: "face-differences", normals: "mesh", ridges: "off" },
  });
  const baked: string[] = [];
  pool.onBaked((bake) => baked.push(patchKeyString(bake.key)));
  return { pool, created, baked };
}

function bakedKeys(worker: ScriptedWorker): string[] {
  return worker.received.flatMap((m) => (m.kind === "bake" ? [patchKeyString(m.key)] : []));
}

function workerAt(created: readonly ScriptedWorker[], n: number): ScriptedWorker {
  const w = created[n];
  if (w === undefined) {
    throw new Error(`no worker ${n}`);
  }
  return w;
}

function fields(w: ScriptedWorker): HeightWorkerRequest[] {
  return w.received.filter((m) => m.kind === "field");
}

describe("the height-worker pool", () => {
  it("hands requests to idle workers in priority order, forced first", () => {
    const { pool, created } = poolOf(1);
    pool.reprioritise([req(1, 5), req(2, 9), req(3, 1, true), req(4, 7)]);
    expect(bakedKeys(workerAt(created, 0))).toEqual([
      patchKeyString(keyAt(3)),
      patchKeyString(keyAt(2)),
    ]);
    workerAt(created, 0).bakeOldest();
    expect(bakedKeys(workerAt(created, 0))[2]).toBe(patchKeyString(keyAt(4)));
  });

  it("never gives a worker more than two requests at once", () => {
    const { pool, created } = poolOf(2);
    pool.reprioritise(Array.from({ length: 10 }, (_, i) => req(i, i)));
    for (const w of created) {
      expect(w.pendingBakes.length).toBeLessThanOrEqual(MAX_IN_FLIGHT_PER_WORKER);
    }
    expect(pool.inFlightCount).toBe(4);
    expect(pool.queuedCount).toBe(6);
  });

  it("reorders the queue and drops what left the demand, leaving requests in flight alone", () => {
    const { pool, created, baked } = poolOf(1);
    pool.reprioritise([req(1, 9), req(2, 8), req(3, 7), req(4, 6)]);
    // 1 and 2 are in flight; 3 and 4 queued. Now 4 outranks 3, 3 is gone, 1 stays wanted.
    pool.reprioritise([req(1, 1), req(4, 9), req(5, 2)]);
    expect(pool.queuedCount).toBe(2);
    const w = workerAt(created, 0);
    w.bakeOldest();
    w.bakeOldest();
    // 1 is delivered (still wanted), 2 dropped (no longer wanted), then 4 before 5.
    expect(baked).toEqual([patchKeyString(keyAt(1))]);
    expect(bakedKeys(w).slice(2)).toEqual([patchKeyString(keyAt(4)), patchKeyString(keyAt(5))]);
  });

  it("never sends a cancelled request to a worker", () => {
    const { pool, created } = poolOf(1);
    pool.reprioritise([req(1, 9), req(2, 8), req(3, 7)]);
    pool.cancelStale(new Set([patchKeyString(keyAt(1)), patchKeyString(keyAt(2))]));
    const w = workerAt(created, 0);
    w.bakeOldest();
    w.bakeOldest();
    expect(bakedKeys(w)).not.toContain(patchKeyString(keyAt(3)));
  });

  it("keeps a stale generation's result whose key is still wanted, and drops one that is not", () => {
    const { pool, created, baked } = poolOf(1);
    pool.reprioritise([req(1, 9), req(2, 8)]);
    const firstGeneration = pool.currentGeneration;
    pool.reprioritise([req(1, 9)]);
    expect(pool.currentGeneration).toBeGreaterThan(firstGeneration);
    const w = workerAt(created, 0);
    w.bakeOldest();
    w.bakeOldest();
    expect(baked).toEqual([patchKeyString(keyAt(1))]);
  });

  it("posts the field to the second worker only after the first acknowledges", () => {
    const { pool, created } = poolOf(2);
    pool.postField(new ArrayBuffer(16));
    expect(fields(workerAt(created, 0))).toHaveLength(1);
    expect(fields(workerAt(created, 1))).toHaveLength(0);
    const first = fields(workerAt(created, 0))[0];
    if (first === undefined) {
      throw new Error("no field was posted");
    }
    workerAt(created, 0).reply({ kind: "field-loaded", id: first.id });
    expect(fields(workerAt(created, 1))).toHaveLength(1);
  });

  it("replaces a worker that errors and re-queues its requests", () => {
    const { pool, created, baked } = poolOf(1);
    pool.postField(new ArrayBuffer(16));
    workerAt(created, 0).ackField();
    pool.reprioritise([req(1, 9), req(2, 8)]);
    expect(bakedKeys(workerAt(created, 0))).toHaveLength(2);
    workerAt(created, 0).fail();
    expect(workerAt(created, 0).terminated).toBe(true);
    expect(workerAt(created, 0).listening).toEqual({ message: 0, error: 0 });
    const replacement = workerAt(created, 1);
    expect(fields(replacement)).toHaveLength(1);
    expect(bakedKeys(replacement)).toEqual([]);
    replacement.ackField();
    expect(bakedKeys(replacement)).toEqual([patchKeyString(keyAt(1)), patchKeyString(keyAt(2))]);
    replacement.bakeOldest();
    expect(baked).toEqual([patchKeyString(keyAt(1))]);
  });

  it("gives a worker's place up after it fails three times in a row", () => {
    const { pool, created } = poolOf(2);
    pool.reprioritise([req(1, 9)]);
    for (let n = 0; n < MAX_CONSECUTIVE_WORKER_FAILURES; n += 1) {
      workerAt(created, created.length === 2 ? 0 : created.length - 1).fail();
    }
    expect(created).toHaveLength(2 + MAX_CONSECUTIVE_WORKER_FAILURES - 1);
    expect(pool.lostWorkers).toBe(1);
    // The request went to the surviving worker.
    expect(bakedKeys(workerAt(created, 1))).toContain(patchKeyString(keyAt(1)));
  });

  it("keeps every worker busy before giving any a second request", () => {
    const { pool, created } = poolOf(3);
    pool.reprioritise([req(1, 9), req(2, 8), req(3, 7)]);
    expect(created.map((w) => w.pendingBakes.length)).toEqual([1, 1, 1]);
  });

  it("sends no bake to a worker before it acknowledges the field", () => {
    const { pool, created } = poolOf(2);
    pool.postField(new ArrayBuffer(16));
    pool.reprioritise([req(1, 9), req(2, 8)]);
    expect(created.map((w) => bakedKeys(w).length)).toEqual([0, 0]);
    workerAt(created, 0).ackField();
    expect(created.map((w) => bakedKeys(w).length)).toEqual([2, 0]);
  });

  it("bakes again what was baked from an older field", () => {
    const { pool, created, baked } = poolOf(1);
    pool.postField(new ArrayBuffer(16));
    workerAt(created, 0).ackField();
    pool.reprioritise([req(1, 9)]);
    pool.postField(new ArrayBuffer(16));
    workerAt(created, 0).bakeOldest();
    expect(baked).toEqual([]);
    workerAt(created, 0).ackField();
    workerAt(created, 0).bakeOldest();
    expect(baked).toEqual([patchKeyString(keyAt(1))]);
    expect(bakedKeys(workerAt(created, 0))).toEqual([
      patchKeyString(keyAt(1)),
      patchKeyString(keyAt(1)),
    ]);
  });

  it("reports a failed bake and gives the request up", () => {
    const { pool, created } = poolOf(1);
    const failures: string[] = [];
    pool.onFailed((f) => failures.push(f.keyString));
    pool.reprioritise([req(1, 9)]);
    const pending = workerAt(created, 0).pendingBakes[0];
    if (pending === undefined) {
      throw new Error("no bake was sent");
    }
    workerAt(created, 0).reply({ kind: "bake-failed", id: pending.id, message: "out of range" });
    expect(failures).toEqual([patchKeyString(keyAt(1))]);
    expect(pool.inFlightCount).toBe(0);
    pool.reprioritise([req(1, 9)]);
    expect(pool.queuedCount + pool.inFlightCount).toBe(0);
  });

  it("leaves no listener behind when terminated", () => {
    const { pool, created, baked } = poolOf(2);
    pool.reprioritise([req(1, 9)]);
    const pending = workerAt(created, 0).pendingBakes[0];
    pool.terminate();
    for (const w of created) {
      expect(w.terminated).toBe(true);
      expect(w.listening).toEqual({ message: 0, error: 0 });
    }
    if (pending !== undefined) {
      workerAt(created, 0).reply({ kind: "baked", id: pending.id, bake: bakeOf(keyAt(1), 1) });
    }
    expect(baked).toEqual([]);
    expect(() => pool.reprioritise([])).toThrow(/terminated/u);
  });
});
