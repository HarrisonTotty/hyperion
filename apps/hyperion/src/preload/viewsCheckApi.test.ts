import { describe, expect, it, vi } from "vitest";

import { VIEWS_CHECK_CHANNELS } from "../main/viewsCheck";
import type { ViewsCheckRecord } from "./api";
import { VIEWS_CHECK_CHANNEL_NAMES, viewsCheckApi, viewsCheckMember } from "./viewsCheckApi";
import { type ViewsCheckLaunch, viewsCheckSwitch } from "./viewsCheckLaunch";

const LAUNCH: ViewsCheckLaunch = { setting: "high", smoke: true, out: null };

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

  it("refuses a results answer of the wrong shape", async () => {
    const api = viewsCheckApi(LAUNCH, { invoke: () => Promise.resolve(undefined) });
    await expect(api.writeResults(RECORD)).rejects.toThrow(/results file/);
  });
});
