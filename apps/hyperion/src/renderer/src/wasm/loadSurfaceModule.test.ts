import { afterEach, describe, expect, it, vi } from "vitest";

import { FakeSurfaceWorker } from "../test/FakeSurfaceWorker";
import {
  checkGeneratorVersion,
  describeSurfaceModule,
  loadSurfaceModule,
} from "./loadSurfaceModule";

function load(worker: FakeSurfaceWorker, serverVersion: number, timeoutMs = 1_000) {
  return loadSurfaceModule(serverVersion, { createWorker: () => worker, timeoutMs });
}

afterEach(() => {
  vi.useRealTimers();
});

describe("loading the surface module", () => {
  it("is ready when the module's generator version is the server's", async () => {
    const worker = new FakeSurfaceWorker({ kind: "generator-version", version: 16 });
    await expect(load(worker, 16)).resolves.toEqual({ kind: "ready", generatorVersion: 16 });
    expect(worker.requests).toEqual([{ kind: "generator-version" }]);
    expect(worker.terminated).toBe(true);
  });

  it("reports a fault of the client's module when the server's version differs", async () => {
    const worker = new FakeSurfaceWorker({ kind: "generator-version", version: 15 });
    const status = await load(worker, 16);
    expect(status).toEqual({
      kind: "fault",
      fault: { kind: "version-mismatch", module: 15, server: 16 },
    });
    expect(describeSurfaceModule(status)).toBe(
      "surface module fault: the module is for generator version 15, the server's is 16: " +
        "run just gen-surface",
    );
  });

  it("passes on a refusal by a Content Security Policy as its own fault", async () => {
    const cause = { kind: "csp-refused", message: "violates … script-src 'self'" } as const;
    const status = await load(new FakeSurfaceWorker({ kind: "load-failed", cause }), 16);
    expect(status).toEqual({ kind: "fault", fault: { kind: "load-failed", cause } });
    expect(describeSurfaceModule(status)).toMatch(/Content Security Policy refused the compile/);
  });

  it("reports a worker that fails", async () => {
    const worker = new FakeSurfaceWorker("error");
    await expect(load(worker, 16)).resolves.toEqual({
      kind: "fault",
      fault: { kind: "worker-failed", message: "worker script failed" },
    });
    expect(worker.terminated).toBe(true);
  });

  it("gives up on a worker that never answers", async () => {
    vi.useFakeTimers();
    const worker = new FakeSurfaceWorker("silence");
    const status = load(worker, 16, 250);
    await vi.advanceTimersByTimeAsync(250);
    await expect(status).resolves.toEqual({
      kind: "fault",
      fault: { kind: "no-answer", afterMs: 250 },
    });
    expect(worker.terminated).toBe(true);
  });

  it("reports a module worker whose script did not load", async () => {
    await expect(load(new FakeSurfaceWorker("load-error"), 16)).resolves.toEqual({
      kind: "fault",
      fault: { kind: "worker-failed", message: "its script did not load" },
    });
  });

  it("reports an answer that threw in the worker as the worker's failure", async () => {
    const worker = new FakeSurfaceWorker({ kind: "answer-failed", message: "unreachable" });
    await expect(load(worker, 16)).resolves.toEqual({
      kind: "fault",
      fault: { kind: "worker-failed", message: "unreachable" },
    });
  });

  it("terminates the worker when aborted, and never settles", async () => {
    const worker = new FakeSurfaceWorker({ kind: "generator-version", version: 16 });
    const abort = new AbortController();
    let settled = false;
    const markSettled = (): void => {
      settled = true;
    };
    void loadSurfaceModule(16, { createWorker: () => worker, signal: abort.signal }).then(
      markSettled,
      markSettled,
    );
    abort.abort();
    // The fake's answer is a queued microtask; let it and the promise's handlers run.
    await Promise.resolve();
    await Promise.resolve();
    expect(worker.terminated).toBe(true);
    expect(settled).toBe(false);
  });

  it("compares versions exactly", () => {
    expect(checkGeneratorVersion(16, 16)).toEqual({ kind: "ready", generatorVersion: 16 });
    expect(checkGeneratorVersion(17, 16).kind).toBe("fault");
  });
});
