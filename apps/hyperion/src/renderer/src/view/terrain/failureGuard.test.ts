import { describe, expect, it, vi } from "vitest";

import { EngineUnavailable } from "../engine/resilientEngine";
import { ANNUNCIATION_ONSET_MS } from "./annunciation";
import { TerrainFailureGuard } from "./failureGuard";

/** A call that throws `error`. */
function throwing(error: Error): () => number {
  return () => {
    throw error;
  };
}

/** A guard's calls at `timesMs`, each `call`, with the faults it raised counted. */
function runAt(
  guard: TerrainFailureGuard,
  timesMs: ReadonlyArray<number>,
  call: (atMs: number) => number,
): { readonly values: number[]; readonly faults: number } {
  let faults = 0;
  const values = timesMs.map((atMs) =>
    guard.run(() => call(atMs), -1, atMs, {
      describe: "the computation failed:",
      onFault: () => {
        faults += 1;
      },
    }),
  );
  return { values, faults };
}

describe("a terrain computation's guard (R10.T16.e; decision-r10-t16-leans §8)", () => {
  const bug = throwing(new Error("a bug of the computation's"));

  it("returns the call's value while it succeeds, and raises nothing", () => {
    const { values, faults } = runAt(new TerrainFailureGuard(), [0, 16, 33], (atMs) => atMs);
    expect([values, faults]).toEqual([[0, 16, 33], 0]);
  });

  it("returns the fallback for a failure, and raises nothing for a single failed frame", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { values, faults } = runAt(new TerrainFailureGuard(), [0, 16], (atMs) =>
      atMs === 0 ? bug() : atMs,
    );
    expect([values, faults]).toEqual([[-1, 16], 0]);
  });

  it("faults the terrain once, when its failures have lasted the onset", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const guard = new TerrainFailureGuard();
    const before = runAt(guard, [1_000, 1_100, 1_000 + ANNUNCIATION_ONSET_MS - 1], bug);
    const after = runAt(guard, [1_000 + ANNUNCIATION_ONSET_MS, 1_400, 2_000], bug);
    expect([before.faults, after.faults]).toEqual([0, 1]);
  });

  it("says a run of failures once, the error with it", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    runAt(new TerrainFailureGuard(), [0, 16, 33, 500], bug);
    expect(error.mock.calls).toEqual([["the computation failed:", expect.any(Error)]]);
  });

  it("counts the onset afresh, and says the failure again, after a success", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const guard = new TerrainFailureGuard();
    const first = runAt(guard, [0, 200], bug);
    runAt(guard, [216], (atMs) => atMs);
    const second = runAt(guard, [232, 400], bug);
    expect([first.faults, second.faults, error.mock.calls.length]).toEqual([0, 0, 2]);
  });

  const lost = throwing(new EngineUnavailable("createBuffer"));

  it("lets a loss pass in silence, starting no run of failures", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const guard = new TerrainFailureGuard();
    const { values, faults } = runAt(guard, [0, 300, 600], lost);
    expect([values, faults, guard.failing, error.mock.calls.length]).toEqual([
      [-1, -1, -1],
      0,
      false,
      0,
    ]);
  });

  it("ends no run of failures with a loss: the run's onset still counts from its first failure", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const guard = new TerrainFailureGuard();
    runAt(guard, [0], bug);
    const through = runAt(guard, [100, ANNUNCIATION_ONSET_MS], lost);
    const next = runAt(guard, [ANNUNCIATION_ONSET_MS + 1], bug);
    expect([through.faults, next.faults]).toEqual([0, 1]);
  });
});
