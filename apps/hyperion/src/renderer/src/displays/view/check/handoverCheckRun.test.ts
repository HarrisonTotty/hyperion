import { describe, expect, it, vi } from "vitest";

import type { HandoverCheckRecord, ViewsCheckApi } from "../../../../../preload/api";
import {
  type HandoverCheckEnding,
  parseExposureReading,
  recordHandoverCheck,
  regimeOf,
} from "./handoverCheckRun";

describe("a frame's regime", () => {
  it.each([
    [["bodies:disc", "bodies:discLimb", "sky:stars"], "disc"],
    [["bodies:smoothMesh", "bodies:discLimb"], "figure"],
    [["bodies:smoothMesh", "terrain-lit-high"], "terrain"],
    [["sky:stars"], "none"],
  ] as const)("of the materials %j is %s", (materials, regime) => {
    expect(regimeOf(materials)).toBe(regime);
  });
});

describe("EXPOSURE's reading", () => {
  it.each([
    ["EV100 15.1 AUTO", { ev100: 15.1, level: "AUTO" }],
    ["EV100 -1.0 MAN", { ev100: -1, level: "MAN" }],
    ["EV100 13.5 INHIBITED · OPERATOR", { ev100: 13.5, level: "INHIBITED · OPERATOR" }],
  ])("reads %s", (text, reading) => {
    expect(parseExposureReading(text)).toEqual(reading);
  });

  it("reads nothing of another shape", () => {
    expect(parseExposureReading("AUTO NOT AVAILABLE")).toBeNull();
  });

  it("reads nothing where there is no reading", () => {
    expect(parseExposureReading(null)).toBeNull();
  });
});

/** A record of no look, for the ending alone. */
const HANDOVER_RECORD: HandoverCheckRecord = {
  world: "TERRAIN TEST",
  devicePixelRatio: 1,
  canvas: { widthPx: 1126, heightPx: 906 },
  target: { widthPx: 1126, heightPx: 906 },
  reliefM: 15_812,
  tauPx: 1,
  radPerPx: 1.0255e-3,
  rangeM: 1.5419e7,
  cellLevel: 6,
  approach: { ratePerS: 0.01, startRatio: 3, closestRatio: 0.85, holdS: 12, endRatio: 1.25 },
  looks: [],
  faults: [],
};

/** The ending's two calls, each a mock. */
function ending(): HandoverCheckEnding & {
  readonly writeHandoverResults: ReturnType<typeof vi.fn<ViewsCheckApi["writeHandoverResults"]>>;
  readonly end: ReturnType<typeof vi.fn<ViewsCheckApi["end"]>>;
} {
  return {
    writeHandoverResults: vi.fn<ViewsCheckApi["writeHandoverResults"]>(() =>
      Promise.resolve({ json: "a", markdown: "b" }),
    ),
    end: vi.fn<ViewsCheckApi["end"]>(() => Promise.resolve()),
  };
}

describe("the hand-over check's ending", () => {
  it("writes the record and ends the run, which the main process judges", async () => {
    const api = ending();
    await recordHandoverCheck(
      api,
      () => Promise.resolve(HANDOVER_RECORD),
      new AbortController().signal,
    );
    expect([api.writeHandoverResults.mock.calls, api.end.mock.calls]).toEqual([
      [[HANDOVER_RECORD]],
      [[{ status: "pass" }]],
    ]);
  });

  it("fails the run with the reason the check threw, writing nothing", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const api = ending();
    await recordHandoverCheck(
      api,
      () =>
        Promise.reject(new Error("views check: VIEW did not show its primary view within 60 s")),
      new AbortController().signal,
    );
    expect([api.writeHandoverResults.mock.calls, api.end.mock.calls]).toEqual([
      [],
      [[{ status: "fail", reason: "views check: VIEW did not show its primary view within 60 s" }]],
    ]);
  });

  it("does nothing once stopped", async () => {
    const api = ending();
    const controller = new AbortController();
    const run = (): Promise<HandoverCheckRecord> => {
      controller.abort();
      return Promise.resolve(HANDOVER_RECORD);
    };
    await recordHandoverCheck(api, run, controller.signal);
    expect([api.writeHandoverResults.mock.calls, api.end.mock.calls]).toEqual([[], []]);
  });
});
