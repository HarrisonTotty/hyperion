import { describe, expect, it, vi } from "vitest";

import type { HandoverCheckRecord, ViewsCheckApi } from "../../../../../preload/api";
import {
  HANDOVER_CHECK_TIMING,
  type HandoverCheckEnding,
  handoverCheckTimingFor,
  handoverLooksFor,
  LookDeadline,
  parseExposureReading,
  recordHandoverCheck,
  regimeOf,
  sceneBeforeTheLooks,
  SERVED_HANDOVER_CHECK_TIMING,
} from "./handoverCheckRun";

describe("a frame's regime", () => {
  it.each([
    [["bodies:disc", "bodies:discLimb", "sky:stars"], "disc"],
    [["bodies:smoothMesh", "bodies:discLimb"], "figure"],
    [["bodies:smoothMeshEmissive", "bodies:discLimbEmissive"], "figure"],
    [["bodies:discEmissive", "bodies:discLimbEmissive"], "disc"],
    [["bodies:smoothMeshAir", "bodies:discLimbAir"], "figure"],
    [["bodies:discAir", "bodies:discLimbAir"], "disc"],
    [["bodies:smoothMesh", "terrain-lit-high"], "terrain"],
    [["sky:stars"], "none"],
  ] as const)("of the materials %j is %s", (materials, regime) => {
    expect(regimeOf(materials)).toBe(regime);
  });
});

describe("the scene the check opens on (R10.T15.b.2)", () => {
  it("leaves SERVER, where a served launch opens with no universe, for PHASE TEST", () => {
    expect(sceneBeforeTheLooks((scene) => scene === "SERVER")).toBe("PHASE TEST");
  });

  it("stays on a kept scene, where a fixture's launch opens", () => {
    expect(sceneBeforeTheLooks((scene) => scene === "PRECISION TEST")).toBeNull();
    expect(sceneBeforeTheLooks(() => false)).toBeNull();
  });
});

describe("a look's deadline (R10.T15.b.2)", () => {
  it("passes on the wall clock at the script's length and the overrun, however the script runs", () => {
    const deadline = new LookDeadline({ overrunMs: 60_000, overrunFrom: "start" }, 1_000, 100);
    expect(deadline.passed(161_000, 99)).toBe(false);
    expect(deadline.passed(161_001, 99.5)).toBe(true);
  });

  it("passes in the software mode only once the script has stood still for the overrun", () => {
    const deadline = new LookDeadline({ overrunMs: 300_000, overrunFrom: "stall" }, 0, 270);
    // SwiftShader's frames carry the script at a tenth of the wall clock's pace: still running.
    for (let n = 1; n <= 100; n += 1) {
      expect(deadline.passed(n * 25_000, n * 2.5)).toBe(false);
    }
    // Then the script stands at 250 s: given up 300 s after its last step.
    expect(deadline.passed(2_500_000 + 300_000, 250)).toBe(false);
    expect(deadline.passed(2_500_000 + 300_001, 250)).toBe(true);
  });

  it("gives a software look whose script never starts the overrun from its start", () => {
    const deadline = new LookDeadline({ overrunMs: 300_000, overrunFrom: "stall" }, 1_000, 270);
    expect(deadline.passed(301_000, 0)).toBe(false);
    expect(deadline.passed(301_001, 0)).toBe(true);
  });
});

describe("the looks a launch makes (R10.T15.b.2)", () => {
  it("are all three in order without --look", () => {
    expect(handoverLooksFor(undefined).map((look) => look.name)).toEqual([
      "handover-30",
      "handover-90",
      "handover-terminator",
    ]);
  });

  it("are the one --look names", () => {
    for (const name of ["handover-30", "handover-90", "handover-terminator"] as const) {
      expect(handoverLooksFor(name).map((look) => look.name)).toEqual([name]);
    }
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

describe("the hand-over check's timing (R10.T15.b)", () => {
  it("waits 5 minutes for a served world's survey, and as before for a fixture's", () => {
    expect([
      handoverCheckTimingFor(true),
      handoverCheckTimingFor(false),
      SERVED_HANDOVER_CHECK_TIMING,
    ]).toEqual([
      SERVED_HANDOVER_CHECK_TIMING,
      HANDOVER_CHECK_TIMING,
      { ...HANDOVER_CHECK_TIMING, surveyWaitMs: 300_000 },
    ]);
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
