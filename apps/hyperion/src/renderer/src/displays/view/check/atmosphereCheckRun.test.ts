import { describe, expect, it, vi } from "vitest";

import type { AtmosphereCheckRecord, ViewsCheckApi } from "../../../../../preload/api";
import { add, dot, norm, scale, sub, type Vec3 } from "../../../geometry/vec3";
import { freeRateMPerS } from "../../../view/camera/freeCamera";
import {
  ATMOSPHERE_FIXTURES,
  ATMOSPHERE_MOON_CENTRE_M,
  ATMOSPHERE_MOON_RADIUS_M,
  atmosphereSeatM,
} from "../../../view/scenes/atmosphereScene";
import { freeRateReading } from "../viewRun";
import {
  ATMOSPHERE_CHECK_TIMING,
  ATMOSPHERE_GOAL_RADII,
  ATMOSPHERE_TOUR,
  type AtmosphereCheckEnding,
  atmosphereLegs,
  captureFrames,
  fixtureCaptureLabel,
  legRateStep,
  quantile,
  rateStepOfCameraLine,
  recordAtmosphereCheck,
} from "./atmosphereCheckRun";
import type { WindowFigures } from "./viewsProbe";

/** The closest the segment from `fromM` to `toM` comes to `pointM`, m. */
function closestM(fromM: Vec3, toM: Vec3, pointM: Vec3): number {
  const along = sub(toM, fromM);
  const t = Math.min(1, Math.max(0, dot(sub(pointM, fromM), along) / dot(along, along)));
  return norm(sub(pointM, add(fromM, scale(along, t))));
}

/** Every body the camera could fly into: the air fixtures and the moon, with their radii. */
const OBSTACLES = [
  ...ATMOSPHERE_FIXTURES.map((fixture) => ({
    designation: fixture.designation,
    centreM: fixture.centreM,
    radiusM: fixture.figure.equatorialRadiusM,
  })),
  {
    designation: "TEST MOON",
    centreM: ATMOSPHERE_MOON_CENTRE_M,
    radiusM: ATMOSPHERE_MOON_RADIUS_M,
  },
];

describe("the atmosphere run's captures' labels", () => {
  it("name a fixture in lower kebab case", () => {
    expect(ATMOSPHERE_FIXTURES.map(({ designation }) => fixtureCaptureLabel(designation))).toEqual([
      "fixture-test-earth",
      "fixture-test-earth-slab",
      "fixture-test-mars",
      "fixture-test-saturn",
    ]);
  });

  it("refuse a designation that makes no capture label", () => {
    expect(() => fixtureCaptureLabel("TEST MARS · 2")).toThrow("makes no capture label");
  });
});

describe("the atmosphere run's tour", () => {
  const legs = atmosphereLegs(atmosphereSeatM(), ATMOSPHERE_TOUR);
  const captured = legs.flatMap((leg) => (leg.capture === null ? [] : [leg.capture]));

  it("captures every air fixture once, each at its goal's range", () => {
    expect(
      captured
        .map((each) => [each.label, each.rangeM])
        .toSorted((a, b) => String(a[0]).localeCompare(String(b[0]))),
    ).toEqual(
      ATMOSPHERE_FIXTURES.map((fixture) => [
        fixtureCaptureLabel(fixture.designation),
        ATMOSPHERE_GOAL_RADII * fixture.figure.equatorialRadiusM,
      ]).toSorted((a, b) => String(a[0]).localeCompare(String(b[0]))),
    );
  });

  it("comes on each fixture's day side, between 50° and 75° of phase", () => {
    expect(captured.every(({ phaseDeg }) => phaseDeg > 50 && phaseDeg < 75)).toBe(true);
  });

  it("chains its legs, each starting where the one before ended", () => {
    expect(
      legs.every((leg, i) => i === 0 || leg.fromM === legs[i - 1]?.toM) &&
        legs.every((leg) => Math.abs(norm(sub(leg.toM, leg.fromM)) - leg.distanceM) < 1e-3),
    ).toBe(true);
  });

  it("passes no body but its target within five of its radii, but the seat's planet it leaves", () => {
    const near = legs.flatMap((leg, i) =>
      OBSTACLES.filter(
        (body) =>
          body.designation !== leg.target &&
          !(i === 0 && body.designation === "TEST EARTH") &&
          closestM(leg.fromM, leg.toM, body.centreM) < 5 * body.radiusM,
      ).map((body) => `${leg.target}: ${body.designation}`),
    );
    expect(near).toEqual([]);
  });

  it("leaves TEST EARTH's ground upwards from the seat", () => {
    const [first] = legs;
    const earth = ATMOSPHERE_FIXTURES[0];
    if (first === undefined || earth === undefined) {
      throw new Error("the tour has no first leg");
    }
    const up = sub(first.fromM, earth.centreM);
    expect(dot(sub(first.toM, first.fromM), up)).toBeGreaterThan(0);
  });

  it("is its first fixture alone in a smoke", () => {
    expect(ATMOSPHERE_CHECK_TIMING.smoke.tour).toEqual([ATMOSPHERE_TOUR[0]]);
  });

  it("refuses a fixture the scene does not have", () => {
    expect(() =>
      atmosphereLegs(atmosphereSeatM(), [{ kind: "fixture", designation: "TEST VENUS" }]),
    ).toThrow("has no fixture TEST VENUS");
  });
});

describe("the atmosphere run's legs' rates", () => {
  it("fly each leg of the full tour within its longest hold, and above a third of it", () => {
    const { legMaxS } = ATMOSPHERE_CHECK_TIMING.full;
    const holdsS = atmosphereLegs(atmosphereSeatM(), ATMOSPHERE_TOUR).map(
      (leg) => leg.distanceM / freeRateMPerS(legRateStep(leg.distanceM, legMaxS, 31)),
    );
    expect(holdsS.every((s) => s <= legMaxS && s > legMaxS / Math.sqrt(10))).toBe(true);
  });

  it("are held within the free camera's steps", () => {
    expect([legRateStep(1e-3, 30, 31), legRateStep(1e30, 30, 31)]).toEqual([0, 31]);
  });

  it("are read back from the CAMERA line's FREE reading", () => {
    expect([
      rateStepOfCameraLine(`FREE · ${freeRateReading(17)}`),
      rateStepOfCameraLine("SEAT"),
      rateStepOfCameraLine(null),
    ]).toEqual([17, null, null]);
  });
});

describe("the atmosphere run's frame figures", () => {
  it("take quantiles by nearest rank", () => {
    const values = Array.from({ length: 100 }, (_, i) => 100 - i);
    expect([quantile(values, 0.5), quantile(values, 0.95), quantile([], 0.5)]).toEqual([
      50,
      95,
      null,
    ]);
  });

  it("are the primary's draws, intervals and GPU times over the window", () => {
    const window: WindowFigures = {
      startMs: 10,
      endMs: 90,
      views: [
        {
          name: "view",
          style: "photorealistic",
          canvas: { widthPx: 1920, heightPx: 1080 },
          draws: 4,
          gpuMs: [5, 7, 6],
          submitMs: [1, 1, 1, 1],
          passLabels: [],
          scales: [1],
        },
      ],
      frameIntervalsMs: [16, 17, 16],
      primaryIntervalsMs: [16, 34, 17],
      mainThreadMs: [2, 2, 2, 2],
      frameGpuMs: [5, 7, 6],
      untimedFrames: 1,
      droppedResolves: 0,
      unattributedGpuMs: 0,
    };
    expect(captureFrames(window)).toEqual({
      fromMs: 10,
      toMs: 90,
      draws: 4,
      intervalMedianMs: 17,
      intervalP95Ms: 34,
      gpuMedianMs: 6,
      gpuP95Ms: 7,
    });
  });
});

/** An atmosphere run's record, as short as one can be. */
const RECORD: AtmosphereCheckRecord = {
  timer: "absent",
  devicePixelRatio: 1,
  canvas: { widthPx: 1920, heightPx: 1080 },
  mediaReadyMs: 1_000,
  captures: [],
  faults: [],
};

/** The ending's two calls, each a mock. */
function ending(): AtmosphereCheckEnding & {
  readonly writeAtmosphereResults: ReturnType<
    typeof vi.fn<ViewsCheckApi["writeAtmosphereResults"]>
  >;
  readonly end: ReturnType<typeof vi.fn<ViewsCheckApi["end"]>>;
} {
  return {
    writeAtmosphereResults: vi.fn<ViewsCheckApi["writeAtmosphereResults"]>(() =>
      Promise.resolve({ json: "a", markdown: "b" }),
    ),
    end: vi.fn<ViewsCheckApi["end"]>(() => Promise.resolve()),
  };
}

describe("the atmosphere run's ending", () => {
  it("writes the record and ends the run, the main process judging it", async () => {
    const api = ending();
    await recordAtmosphereCheck(api, () => Promise.resolve(RECORD), new AbortController().signal);
    expect([api.writeAtmosphereResults.mock.calls, api.end.mock.calls]).toEqual([
      [[RECORD]],
      [[{ status: "pass" }]],
    ]);
  });

  it("fails the run with the reason its script threw, writing nothing", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const api = ending();
    await recordAtmosphereCheck(
      api,
      () => Promise.reject(new Error("views check: ATMOSPHERE TEST's media did not load")),
      new AbortController().signal,
    );
    expect([api.writeAtmosphereResults.mock.calls, api.end.mock.calls]).toEqual([
      [],
      [[{ status: "fail", reason: "views check: ATMOSPHERE TEST's media did not load" }]],
    ]);
  });

  it("ends nothing once stopped", async () => {
    const api = ending();
    const controller = new AbortController();
    const run = (): Promise<AtmosphereCheckRecord> => {
      controller.abort();
      return Promise.resolve(RECORD);
    };
    await recordAtmosphereCheck(api, run, controller.signal);
    expect([api.writeAtmosphereResults.mock.calls, api.end.mock.calls]).toEqual([[], []]);
  });
});
