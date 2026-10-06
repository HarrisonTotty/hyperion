import { describe, expect, it, vi } from "vitest";

import type { ViewsCheckRecord } from "../../../../../preload/api";
import {
  INSTRUMENT_SLOTS,
  instrumentName,
  instrumentViewId,
  PRIMARY_NAME,
  PRIMARY_VIEW_NAME,
} from "../viewNames";
import { engineName, recordViewsCheck, type ViewsCheckEnding } from "./viewsCheckRun";

const RECORD: ViewsCheckRecord = {
  timer: "full",
  devicePixelRatio: 1,
  phases: [],
  resize: { before: [], steps: [], allocations: [] },
  faults: [],
  perCanvasOverheadMs: 0.3,
};

/** The ending's calls, in order. */
function ending(): ViewsCheckEnding & { readonly calls: unknown[][] } {
  const calls: unknown[][] = [];
  return {
    calls,
    writeResults: (record) => {
      calls.push(["write", record]);
      return Promise.resolve({ json: "a.json", markdown: "a.md" });
    },
    end: (outcome) => {
      calls.push(["end", outcome]);
      return Promise.resolve();
    },
  };
}

describe("the views check's ending", () => {
  it("writes the record and passes the run", async () => {
    const api = ending();
    await recordViewsCheck(api, () => Promise.resolve(RECORD), new AbortController().signal);
    expect(api.calls).toEqual([
      ["write", RECORD],
      ["end", { status: "pass" }],
    ]);
  });

  it("fails the run with the reason the script threw", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const api = ending();
    await recordViewsCheck(
      api,
      () => Promise.reject(new Error("views check: INSTRUMENT 1 did not open")),
      new AbortController().signal,
    );
    expect(api.calls).toEqual([
      ["end", { status: "fail", reason: "views check: INSTRUMENT 1 did not open" }],
    ]);
  });

  it("ends nothing once stopped", async () => {
    const api = ending();
    const stop = new AbortController();
    stop.abort();
    await recordViewsCheck(
      api,
      () => Promise.reject(new Error("views check: stopped")),
      stop.signal,
    );
    await recordViewsCheck(api, () => Promise.resolve(RECORD), stop.signal);
    expect(api.calls).toEqual([]);
  });
});

describe("engineName", () => {
  it("names the primary's view as VIEW names it to the engine", () => {
    expect(engineName(PRIMARY_NAME)).toBe(PRIMARY_VIEW_NAME);
  });

  it.each(INSTRUMENT_SLOTS)("names instrument %i's view as VIEW names it to the engine", (slot) => {
    expect(engineName(instrumentName(slot))).toBe(instrumentViewId(slot));
  });

  it("names no view for a slot VIEW does not have", () => {
    expect(engineName("INSTRUMENT 3")).toBeNull();
  });
});
