import { describe, expect, it } from "vitest";

import type { AtmosphereMedium } from "../view/atmosphere/medium";
import type { TwinTable } from "../view/atmosphere/tablesCpu";
import type { TwinReply, TwinRequest } from "./twinJobs";
import { TwinPool, type TwinWorker, twinWorkerCount } from "./twinPool";

/** A worker that keeps what it is sent and replies when the test says. */
class FakeTwinWorker implements TwinWorker {
  readonly posted: TwinRequest[] = [];
  terminated = false;
  // An `Event` listener takes a `MessageEvent` too, so every listener is kept as a message's.
  readonly #listeners: Array<{
    readonly type: string;
    readonly listener: (event: MessageEvent<TwinReply>) => void;
  }> = [];

  addEventListener(type: "message", listener: (event: MessageEvent<TwinReply>) => void): void;
  addEventListener(type: "error" | "messageerror", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error" | "messageerror",
    listener: ((event: MessageEvent<TwinReply>) => void) | ((event: Event) => void),
  ): void {
    this.#listeners.push({ type, listener });
  }

  /** Whether the next message cannot be copied, as a `DataCloneError`. */
  refuseNext = false;

  postMessage(message: TwinRequest): void {
    if (this.refuseNext) {
      throw new DOMException("could not be cloned", "DataCloneError");
    }
    this.posted.push(message);
  }

  terminate(): void {
    this.terminated = true;
  }

  reply(reply: TwinReply): void {
    this.#emit("message", new MessageEvent("message", { data: reply }));
  }

  /** Fires the worker's `error`, with `message` as an `ErrorEvent` has one. */
  fail(message?: string): void {
    const event = new MessageEvent<TwinReply>("error");
    if (message !== undefined) {
      Object.defineProperty(event, "message", { value: message });
    }
    this.#emit("error", event);
  }

  #emit(type: string, event: MessageEvent<TwinReply>): void {
    for (const each of this.#listeners) {
      if (each.type === type) {
        each.listener(event);
      }
    }
  }
}

/** A pool of `count` fake workers. */
function fakePool(count: number): { readonly pool: TwinPool; readonly workers: FakeTwinWorker[] } {
  const workers: FakeTwinWorker[] = [];
  const pool = new TwinPool(count, () => {
    const worker = new FakeTwinWorker();
    workers.push(worker);
    return worker;
  });
  return { pool, workers };
}

const MEDIUM: AtmosphereMedium = {
  name: "test",
  topHeightM: 100_000,
  groundAlbedo: [0.3, 0.3, 0.3],
  terms: [],
};

/** A one-texel table holding `value`. */
function table(value: number): TwinTable {
  return { widthTexels: 1, heightTexels: 1, texels: Float64Array.of(value, value, value, 1) };
}

/** The worker of `workers` running the job numbered `id`, which the test then answers. */
function runner(workers: readonly FakeTwinWorker[], id: number): FakeTwinWorker {
  const worker = workers.find((each) => each.posted.at(-1)?.id === id);
  if (worker === undefined) {
    throw new Error(`no worker runs job ${id}`);
  }
  return worker;
}

describe("the smoke page's twin workers", () => {
  it("starts jobs in the order asked, each on the first worker free", async () => {
    const { pool, workers } = fakePool(2);
    const first = pool.transmittance(MEDIUM, 6e6);
    const second = pool.transmittance(MEDIUM, 7e6);
    const third = pool.transmittance(MEDIUM, 8e6);
    expect(workers.map((worker) => worker.posted.map((request) => request.id))).toEqual([[0], [1]]);
    runner(workers, 1).reply({
      kind: "answer",
      id: 1,
      answer: { kind: "transmittance", twin: table(2) },
    });
    expect(workers[1]?.posted.map((request) => request.id)).toEqual([1, 2]);
    expect(workers[1]?.posted[1]?.job).toEqual({
      kind: "transmittance",
      medium: MEDIUM,
      bottomRadiusM: 8e6,
    });
    runner(workers, 0).reply({
      kind: "answer",
      id: 0,
      answer: { kind: "transmittance", twin: table(1) },
    });
    runner(workers, 2).reply({
      kind: "answer",
      id: 2,
      answer: { kind: "transmittance", twin: table(3) },
    });
    expect((await first).texels[0]).toBe(1);
    expect((await second).texels[0]).toBe(2);
    expect((await third).texels[0]).toBe(3);
    pool.dispose();
  });

  it("rejects with the twin's own name and message when the twin throws", async () => {
    const { pool, workers } = fakePool(1);
    const twin = pool.transmittance(MEDIUM, 6e6);
    runner(workers, 0).reply({
      kind: "error",
      id: 0,
      name: "RangeError",
      message: "a tabulated term ends below the top",
    });
    await expect(twin).rejects.toMatchObject({
      name: "RangeError",
      message: "a tabulated term ends below the top",
    });
    pool.dispose();
  });

  it("rejects an answer of another kind than the job's", async () => {
    const { pool, workers } = fakePool(1);
    const twin = pool.transmittance(MEDIUM, 6e6);
    runner(workers, 0).reply({
      kind: "answer",
      id: 0,
      answer: { kind: "skyView", twin: table(1) },
    });
    await expect(twin).rejects.toThrow(
      "a twin worker answered a transmittance job with a skyView twin",
    );
    pool.dispose();
  });

  it("rejects every job, running, waiting and later, once a worker fails, and stops them all", async () => {
    const { pool, workers } = fakePool(2);
    const running = pool.transmittance(MEDIUM, 6e6);
    const other = pool.transmittance(MEDIUM, 6e6);
    const waiting = pool.transmittance(MEDIUM, 6e6);
    workers[0]?.fail();
    const later = pool.transmittance(MEDIUM, 6e6);
    for (const twin of [running, other, waiting, later]) {
      // Each job's rejection is read in turn; they are all settled already.
      // oxlint-disable-next-line no-await-in-loop
      await expect(twin).rejects.toThrow("a twin worker failed");
    }
    expect(workers.every((worker) => worker.terminated)).toBe(true);
  });

  it("starts no worker before its first job, and all of them with it", () => {
    const { pool, workers } = fakePool(2);
    expect(workers).toHaveLength(0);
    void pool.transmittance(MEDIUM, 6e6);
    expect(workers).toHaveLength(2);
    void pool.transmittance(MEDIUM, 6e6);
    expect(workers).toHaveLength(2);
    pool.dispose();
  });

  it("closes, stopping every worker, when a worker answers a job it was not running", async () => {
    const { pool, workers } = fakePool(2);
    const running = pool.transmittance(MEDIUM, 6e6);
    runner(workers, 0).reply({
      kind: "answer",
      id: 7,
      answer: { kind: "transmittance", twin: table(1) },
    });
    await expect(running).rejects.toThrow("a twin worker answered job 7");
    expect(workers.every((worker) => worker.terminated)).toBe(true);
  });

  it("closes with the cause when a job cannot be sent to its worker", async () => {
    const { pool, workers } = fakePool(1);
    const first = pool.transmittance(MEDIUM, 6e6);
    const worker = runner(workers, 0);
    worker.refuseNext = true;
    const second = pool.transmittance(MEDIUM, 7e6);
    worker.reply({ kind: "answer", id: 0, answer: { kind: "transmittance", twin: table(1) } });
    expect((await first).texels[0]).toBe(1);
    await expect(second).rejects.toMatchObject({
      message: "a transmittance job could not be sent",
      cause: { name: "DataCloneError" },
    });
  });

  it("names a worker's own error in its failure", async () => {
    const { pool, workers } = fakePool(1);
    const twin = pool.transmittance(MEDIUM, 6e6);
    workers[0]?.fail("RangeError: out of memory");
    await expect(twin).rejects.toThrow("a twin worker failed: RangeError: out of memory");
  });

  it("rejects what it has not answered when disposed", async () => {
    const { pool, workers } = fakePool(1);
    const twin = pool.transmittance(MEDIUM, 6e6);
    pool.dispose();
    await expect(twin).rejects.toThrow("the twin pool was disposed");
    expect(workers[0]?.terminated).toBe(true);
  });

  it("takes a quarter of the hardware threads, from 1 to 4", () => {
    expect([1, 4, 8, 12, 16, 32].map((threads) => twinWorkerCount(threads, null))).toEqual([
      1, 1, 2, 3, 4, 4,
    ]);
  });

  it("takes the count asked for, but never more than 4, and the default for one not whole", () => {
    expect(twinWorkerCount(16, 2)).toBe(2);
    expect(twinWorkerCount(4, 3)).toBe(3);
    expect(twinWorkerCount(16, 9)).toBe(4);
    expect(twinWorkerCount(8, 0)).toBe(2);
    expect(twinWorkerCount(8, Number.NaN)).toBe(2);
    expect(twinWorkerCount(8, 1.5)).toBe(2);
  });
});
