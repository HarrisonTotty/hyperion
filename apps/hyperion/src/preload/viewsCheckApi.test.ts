import { describe, expect, it, vi } from "vitest";

import { VIEWS_CHECK_CHANNELS } from "../main/viewsCheck";
import { ATMOSPHERE_RECORD } from "../main/fixtures/atmosphereRecord";
import { HANDOVER_RECORD } from "../main/fixtures/handoverRecord";
import { TERRAIN_RECORD } from "../main/fixtures/terrainRecord";
import type { ViewsCheckRecord } from "./api";
import { VIEWS_CHECK_CHANNEL_NAMES, viewsCheckApi, viewsCheckMember } from "./viewsCheckApi";
import { type ViewsCheckLaunch, viewsCheckSwitch } from "./viewsCheckLaunch";

const LAUNCH: ViewsCheckLaunch = {
  setting: "high",
  smoke: true,
  out: null,
  run: "several-views",
  fixture: "terrain-test",
};

const RECORD: ViewsCheckRecord = {
  timer: "full",
  devicePixelRatio: 1,
  phases: [],
  resize: { before: [], steps: [], allocations: [] },
  faults: [],
  perCanvasOverheadMs: 0.3,
};

describe("the preload's views-check functions", () => {
  it("are absent on another launch, and present on a check launch", () => {
    const deps = { invoke: () => Promise.resolve() };
    expect(viewsCheckMember(["--hyperion-server-url=ws://127.0.0.1:7878/ws"], deps)).toEqual({});
    expect(viewsCheckMember([viewsCheckSwitch(LAUNCH)], deps).viewsCheck?.launch).toEqual(LAUNCH);
  });

  it("use the main process's channels", () => {
    expect(VIEWS_CHECK_CHANNEL_NAMES).toEqual(VIEWS_CHECK_CHANNELS);
  });

  it("send each operation on its own channel", async () => {
    const invoke = vi.fn<(channel: string, ...args: unknown[]) => Promise<unknown>>((channel) =>
      Promise.resolve(
        channel === VIEWS_CHECK_CHANNEL_NAMES.writeResults ? { json: "a", markdown: "b" } : null,
      ),
    );
    const api = viewsCheckApi(LAUNCH, { invoke });
    await api.startPhase("photoreal-alone");
    await api.endPhase("photoreal-alone", { startMs: 1, endMs: 2 });
    await api.askRightWayUp();
    expect(await api.writeResults(RECORD)).toEqual({ json: "a", markdown: "b" });
    await api.end({ status: "pass" });
    expect(invoke.mock.calls).toEqual([
      [VIEWS_CHECK_CHANNEL_NAMES.startPhase, "photoreal-alone"],
      [VIEWS_CHECK_CHANNEL_NAMES.endPhase, "photoreal-alone", { startMs: 1, endMs: 2 }],
      [VIEWS_CHECK_CHANNEL_NAMES.askRightWayUp],
      [VIEWS_CHECK_CHANNEL_NAMES.writeResults, RECORD],
      [VIEWS_CHECK_CHANNEL_NAMES.end, { status: "pass" }],
    ]);
  });

  it("send a terrain run's operations on their own channels (R10.T16.d)", async () => {
    const invoke = vi.fn<(channel: string, ...args: unknown[]) => Promise<unknown>>((channel) =>
      Promise.resolve(
        channel === VIEWS_CHECK_CHANNEL_NAMES.writeTerrainResults
          ? { json: "a", markdown: "b" }
          : null,
      ),
    );
    const api = viewsCheckApi({ ...LAUNCH, run: "terrain" }, { invoke });
    await api.startPhase("terrain-loss");
    await api.capture("restored");
    await api.endPhase("terrain-loss", { startMs: 1, endMs: 2 });
    expect(await api.writeTerrainResults(TERRAIN_RECORD)).toEqual({ json: "a", markdown: "b" });
    expect(invoke.mock.calls).toEqual([
      [VIEWS_CHECK_CHANNEL_NAMES.startPhase, "terrain-loss"],
      [VIEWS_CHECK_CHANNEL_NAMES.capture, "restored"],
      [VIEWS_CHECK_CHANNEL_NAMES.endPhase, "terrain-loss", { startMs: 1, endMs: 2 }],
      [VIEWS_CHECK_CHANNEL_NAMES.writeTerrainResults, TERRAIN_RECORD],
    ]);
  });

  it("send a hand-over check's record on its own channel (R10.T10.g)", async () => {
    const invoke = vi.fn<(channel: string, ...args: unknown[]) => Promise<unknown>>(() =>
      Promise.resolve({ json: "a", markdown: "b" }),
    );
    const api = viewsCheckApi({ ...LAUNCH, run: "handover" }, { invoke });
    expect(await api.writeHandoverResults(HANDOVER_RECORD)).toEqual({ json: "a", markdown: "b" });
    expect(invoke.mock.calls).toEqual([
      [VIEWS_CHECK_CHANNEL_NAMES.writeHandoverResults, HANDOVER_RECORD],
    ]);
  });

  it("send an atmosphere run's captures and record on their own channels (R08.T10.b)", async () => {
    const invoke = vi.fn<(channel: string, ...args: unknown[]) => Promise<unknown>>((channel) =>
      Promise.resolve(
        channel === VIEWS_CHECK_CHANNEL_NAMES.writeAtmosphereResults
          ? { json: "a", markdown: "b" }
          : null,
      ),
    );
    const api = viewsCheckApi({ ...LAUNCH, run: "atmosphere" }, { invoke });
    await api.capture("fixture-test-earth");
    expect(await api.writeAtmosphereResults(ATMOSPHERE_RECORD)).toEqual({
      json: "a",
      markdown: "b",
    });
    await api.end({ status: "pass" });
    expect(invoke.mock.calls).toEqual([
      [VIEWS_CHECK_CHANNEL_NAMES.capture, "fixture-test-earth"],
      [VIEWS_CHECK_CHANNEL_NAMES.writeAtmosphereResults, ATMOSPHERE_RECORD],
      [VIEWS_CHECK_CHANNEL_NAMES.end, { status: "pass" }],
    ]);
  });

  it("refuses an atmosphere results answer of the wrong shape", async () => {
    const api = viewsCheckApi(LAUNCH, { invoke: () => Promise.resolve(undefined) });
    await expect(api.writeAtmosphereResults(ATMOSPHERE_RECORD)).rejects.toThrow(
      "the main process wrote no atmosphere results file",
    );
  });

  it("refuses a hand-over results answer of the wrong shape", async () => {
    const api = viewsCheckApi(LAUNCH, { invoke: () => Promise.resolve(undefined) });
    await expect(api.writeHandoverResults(HANDOVER_RECORD)).rejects.toThrow(/hand-over results/);
  });

  it("refuses a terrain results answer of the wrong shape", async () => {
    const api = viewsCheckApi(LAUNCH, { invoke: () => Promise.resolve(undefined) });
    await expect(api.writeTerrainResults(TERRAIN_RECORD)).rejects.toThrow(/terrain results/);
  });

  it("refuses a results answer of the wrong shape", async () => {
    const api = viewsCheckApi(LAUNCH, { invoke: () => Promise.resolve(undefined) });
    await expect(api.writeResults(RECORD)).rejects.toThrow(/results file/);
  });
});
