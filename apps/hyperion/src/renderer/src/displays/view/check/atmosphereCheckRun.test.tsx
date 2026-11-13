import { render, type RenderResult } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { AtmosphereCheckRecord, ViewsCheckApi } from "../../../../../preload/api";
import { add, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import { ATMOSPHERE_STATEMENTS } from "../../../view/atmosphere/labels";
import { IMAGE_DETAIL_LIMITED } from "../../../view/budget/viewBudget";
import { freeRateMPerS, MAX_FREE_STEP_S } from "../../../view/camera/freeCamera";
import { rotate } from "../../../view/camera/quaternion";
import { formatBodyDistance } from "../../../lib/format";
import { STYLE_TOGGLE_KEY } from "../../../view/photoreal/style";
import {
  ATMOSPHERE_FIXTURES,
  ATMOSPHERE_MOON_CENTRE_M,
  ATMOSPHERE_MOON_RADIUS_M,
  ATMOSPHERE_SCENE_NAME,
  type AtmosphereFixtureMedia,
  atmosphereSeatM,
} from "../../../view/scenes/atmosphereScene";
import { ViewLabelBlock } from "../ViewLabelBlock";
import { freeRateReading, PHOTOREAL_PREPARING } from "../viewRun";
import {
  ATMOSPHERE_CHECK_TIMING,
  ATMOSPHERE_END_TOLERANCE,
  ATMOSPHERE_GOAL_RADII,
  ATMOSPHERE_SMOKE_TOUR,
  ATMOSPHERE_STAGE_FRACTION,
  ATMOSPHERE_TOUR,
  type AtmosphereCheckDeps,
  type AtmosphereCheckEnding,
  atmosphereLegs,
  captureFrames,
  exposureEv100,
  fixtureCaptureLabel,
  JUPITER_CAPTURE_RANGES_M,
  heldMs,
  legRateStep,
  PLACE_RANGE_TOLERANCE,
  quantile,
  rangeCaptureLabel,
  rateStepOfCameraLine,
  recordAtmosphereCheck,
  runAtmosphereCheck,
  stageRangesM,
  TITAN_LIMB_RANGE_M,
} from "./atmosphereCheckRun";
import {
  ATMOSPHERE_LOOKS,
  type AtmospherePlace,
  atmospherePlaces,
  PLACE_SCENE_NAMES,
} from "./atmospherePlaces";
import { STOPPED } from "./viewsCheckRun";
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
      "fixture-test-earth-o3-slab",
      "fixture-test-mars",
      "fixture-test-saturn",
      "fixture-test-venus",
      "fixture-test-venus-5-80-mpa",
      "fixture-test-titan",
      "fixture-test-saturn-nh3-ice-deck",
      "fixture-test-jupiter",
    ]);
  });

  it("refuse a designation that makes no capture label", () => {
    expect(() => fixtureCaptureLabel("TEST MARS · 2")).toThrow("makes no capture label");
  });

  it("name a range's capture by its fixture and its whole kilometres (R08.T16.a.2)", () => {
    expect(
      JUPITER_CAPTURE_RANGES_M.map((rangeM) => rangeCaptureLabel("TEST JUPITER", rangeM)),
    ).toEqual([
      "fixture-test-jupiter-950000-km",
      "fixture-test-jupiter-500000-km",
      "fixture-test-jupiter-220000-km",
      "fixture-test-jupiter-150000-km",
    ]);
  });

  it("name TEST TITAN's limb capture by its range (R08.T14.d.4)", () => {
    expect(rangeCaptureLabel("TEST TITAN", TITAN_LIMB_RANGE_M)).toBe("fixture-test-titan-9000-km");
  });

  it("refuse a range of no whole kilometres", () => {
    expect(() => rangeCaptureLabel("TEST JUPITER", 1.5e8 + 1)).toThrow("makes no capture label");
  });
});

/** Two captures' labels and ranges in their labels' order. */
function byLabel(a: readonly [string, number], b: readonly [string, number]): number {
  return a[0].localeCompare(b[0]);
}

describe("the atmosphere run's tour", () => {
  const legs = atmosphereLegs(atmosphereSeatM(), ATMOSPHERE_TOUR);
  const captured = legs.flatMap((leg) => (leg.capture === null ? [] : [leg.capture]));

  it("captures every air fixture but the giant once, each at its goal's range, TEST TITAN again from its limb, and TEST JUPITER at each of its ranges", () => {
    expect(captured.map((each) => [each.label, each.rangeM] as const).toSorted(byLabel)).toEqual(
      [
        ...ATMOSPHERE_FIXTURES.filter(({ designation }) => designation !== "TEST JUPITER").map(
          (fixture) =>
            [
              fixtureCaptureLabel(fixture.designation),
              ATMOSPHERE_GOAL_RADII * fixture.figure.equatorialRadiusM,
            ] as const,
        ),
        [rangeCaptureLabel("TEST TITAN", TITAN_LIMB_RANGE_M), TITAN_LIMB_RANGE_M] as const,
        ...JUPITER_CAPTURE_RANGES_M.map(
          (rangeM) => [rangeCaptureLabel("TEST JUPITER", rangeM), rangeM] as const,
        ),
      ].toSorted(byLabel),
    );
  });

  it("comes on each fixture's day side, between 40° and 75° of phase", () => {
    expect(captured.map(({ phaseDeg }) => phaseDeg > 40 && phaseDeg < 75)).toEqual(
      captured.map(() => true),
    );
  });

  it("closes on TEST JUPITER along one line from inside 10⁹ m to its limb (R08.T16.a.2)", () => {
    const jupiter = legs.filter((leg) => leg.target === "TEST JUPITER");
    const [first] = jupiter;
    if (first === undefined) {
      throw new Error("the tour has no leg to TEST JUPITER");
    }
    const along = normalise(sub(first.centreM, first.fromM));
    expect([
      jupiter.map((leg) => leg.capture?.rangeM),
      jupiter.every((leg) => dot(normalise(sub(leg.centreM, leg.toM)), along) > 1 - 1e-12),
      norm(sub(first.centreM, first.fromM)) > 1e9,
    ]).toEqual([JUPITER_CAPTURE_RANGES_M, true, true]);
  });

  it("chains its legs and their stages, each starting where the one before ended", () => {
    const stages = legs.flatMap((leg) => leg.stages);
    expect([
      legs.every((leg, i) => i === 0 || leg.fromM === legs[i - 1]?.toM),
      legs.every((leg) => leg.stages[0]?.fromM === leg.fromM && leg.stages.at(-1)?.toM === leg.toM),
      stages.every((stage, i) => i === 0 || stage.fromM === stages[i - 1]?.toM),
      stages.every((stage) => Math.abs(norm(sub(stage.toM, stage.fromM)) - stage.distanceM) < 1e-3),
    ]).toEqual([true, true, true, true]);
  });

  it("ends each stage at a twentieth of the range it starts at, or at its leg's end, whichever is farther", () => {
    const misplaced = legs.flatMap((leg) => {
      const endRangeM = norm(sub(leg.centreM, leg.toM));
      return leg.stages.filter((stage) => {
        const startRangeM = norm(sub(leg.centreM, stage.fromM));
        const wantedM = Math.max(startRangeM * ATMOSPHERE_STAGE_FRACTION, endRangeM);
        return (
          Math.abs(stage.endRangeM - wantedM) > 1e-6 * wantedM ||
          Math.abs(norm(sub(leg.centreM, stage.toM)) - stage.endRangeM) > 1e-6 * wantedM
        );
      });
    });
    expect(misplaced).toEqual([]);
  });

  it("climbs from the seat in one stage, then flies the long leg to the first fixture in three", () => {
    expect(legs.map((leg) => [leg.target, leg.stages.length])).toEqual([
      ["TEST MOON", 1],
      ["TEST EARTH O3 SLAB", 3],
      ["TEST MARS", 2],
      ["TEST SATURN", 1],
      ["TEST VENUS", 2],
      ["TEST VENUS 5.80 MPa", 2],
      ["TEST TITAN", 2],
      ["TEST TITAN", 1],
      ["TEST SATURN NH3 ICE DECK", 1],
      ["TEST JUPITER", 1],
      ["TEST JUPITER", 1],
      ["TEST JUPITER", 1],
      ["TEST JUPITER", 1],
      ["TEST EARTH", 3],
    ]);
  });

  it("passes no body but its target within five of its radii, but the planet it climbs from and a body it leaves", () => {
    const near = legs.flatMap((leg, i) =>
      OBSTACLES.filter(
        (body) =>
          body.designation !== leg.target &&
          !(i === 0 && body.designation === "TEST EARTH") &&
          // The legs after TEST TITAN's limb and TEST JUPITER's last capture start at 3.5 and 2.1
          // of their radii, and close on them by less than a tenth of that as they leave.
          !(
            body.designation === legs[i - 1]?.target &&
            closestM(leg.fromM, leg.toM, body.centreM) > 0.9 * norm(sub(leg.fromM, body.centreM))
          ) &&
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

  it("is the climb and its first fixture in a smoke", () => {
    expect([ATMOSPHERE_CHECK_TIMING.smoke.tour, ATMOSPHERE_SMOKE_TOUR]).toEqual([
      ATMOSPHERE_TOUR.slice(0, 2),
      [
        { kind: "toward", designation: "TEST MOON", distanceM: 5e7 },
        { kind: "fixture", designation: "TEST EARTH O3 SLAB" },
      ],
    ]);
  });

  it("refuses a fixture the scene does not have", () => {
    expect(() =>
      atmosphereLegs(atmosphereSeatM(), [{ kind: "fixture", designation: "TEST NEPTUNE" }]),
    ).toThrow("has no fixture TEST NEPTUNE");
  });

  it("refuses a leg towards a body that would reach it", () => {
    expect(() =>
      atmosphereLegs(atmosphereSeatM(), [
        { kind: "toward", designation: "TEST MOON", distanceM: 4e8 },
      ]),
    ).toThrow("would reach it");
  });
});

describe("the atmosphere run's stages", () => {
  it("end at a twentieth of the range each starts at until the goal is nearer", () => {
    const startM = 2.1e11;
    const fraction = ATMOSPHERE_STAGE_FRACTION;
    expect(stageRangesM(startM, 5.1e7)).toEqual([
      startM * fraction,
      startM * fraction * fraction,
      5.1e7,
    ]);
  });

  it("are none for a leg that starts at its end", () => {
    expect(stageRangesM(5.1e7, 5.1e7)).toEqual([]);
  });

  it("refuse a fraction that would never reach the end", () => {
    expect(() => stageRangesM(1e9, 1e7, 1)).toThrow("no stages");
  });
});

describe("the atmosphere run's legs' rates", () => {
  it.each(["full", "smoke"] as const)(
    "fly each stage of the full tour within the %s run's longest hold, and above a third of it",
    (run) => {
      const { legMaxS } = ATMOSPHERE_CHECK_TIMING[run];
      const holdsS = atmosphereLegs(atmosphereSeatM(), ATMOSPHERE_TOUR)
        .flatMap((leg) => leg.stages)
        .map((stage) => stage.distanceM / freeRateMPerS(legRateStep(stage.distanceM, legMaxS, 31)));
      expect(holdsS.filter((s) => !(s <= legMaxS && s > legMaxS / Math.sqrt(10)))).toEqual([]);
    },
  );

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

describe("the atmosphere run's reckoning", () => {
  it("counts the primary's steps from its draw before the press to its last with the key held, each at most a quarter second", () => {
    expect([heldMs([1_000, 1_016, 1_032, 1_400, 1_416]), heldMs([5]), heldMs([])]).toEqual([
      298, 0, 0,
    ]);
  });

  it("reads the EXPOSURE line's EV100, or none", () => {
    expect(
      ["EV100 -3.0 AUTO", "EV100 15.1 INHIBITED · OPERATOR", "—", null].map(exposureEv100),
    ).toEqual([-3, 15.1, null, null]);
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
  stages: [],
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

/** Each test's listeners on the page, which outlive its elements, ended after it. */
let listening = new AbortController();

afterEach(() => {
  listening.abort();
  listening = new AbortController();
  document.body.replaceChildren();
});

/** The fake page's animation frames, ms apart: every one a draw of the primary. */
const FRAME_MS = 16;

/** How long after `ATMOSPHERE TEST` is chosen its media land, ms of the test's clock. */
const MEDIA_MS = 2_000;

/** The exposure `VIEW` opens with under `MAN`, EV100. */
const OPENING_EV = 15;

/** Where `AUTO` takes the exposure: the moonlit night from the seat, and a lit fixture. */
const NIGHT_EV = -3;
const DAY_EV = 12;

/** The plate's statement and fault line once the media have landed, which each capture records. */
const PLATE_STATEMENTS = ["BODY PHOTOMETRY: NOT YET MODELLED"];
const PLATE_FAULTS = [
  "ATMOSPHERE NOT AVAILABLE: computation failed for TEST MARS, reopen VIEW to retry",
];

/** The fake's marks, in the order its target keys step through them. */
const MARKS: ReadonlyArray<{ readonly designation: string; readonly centreM: Vec3 }> = [
  { designation: "TEST STAR", centreM: vec3(0, 0, 0) },
  ...ATMOSPHERE_FIXTURES.map(({ designation, centreM }) => ({ designation, centreM })),
  { designation: "TEST MOON", centreM: ATMOSPHERE_MOON_CENTRE_M },
];

/** The marks' rows the fake's windowed list renders, fewer than its marks. */
const MARK_WINDOW_ROWS = 6;

/** How the fake `AUTO` moves the exposure, EV/s: `AUTO`'s own speeds unless a test slows them. */
interface AutoSpeeds {
  readonly brightenEvPerS: number;
  readonly darkenEvPerS: number;
}

/**
 * A frame that comes late, once: in the `hold`-th hold of a flight key (from 1), the first frame
 * `afterMs` or more into it is followed by the next only `forMs` later.
 */
interface Stall {
  readonly hold: number;
  readonly afterMs: number;
  readonly forMs: number;
}

/**
 * The places a fake offers as check-only scenes (the software mode's), how long each one's bake
 * runs once chosen, ms, and how far off its range the fake stands the camera, a factor.
 */
interface PlacedFake {
  readonly places: ReadonlyArray<AtmospherePlace>;
  readonly computingMs: number;
  readonly misplacement: number;
  /** How long a place's stage draws the wireframe while its pipelines are made, ms. */
  readonly preparingMs?: number;
  /**
   * How long a place's stage reads photorealistic once it stands before it asks for its pipelines,
   * ms, as `VIEW`'s new stage does for a frame or more.
   */
  readonly preparingAfterMs?: number;
  /**
   * How long after a scene's choice its stage stands in for the one before, ms, as `VIEW`'s does
   * until it renders the new one.
   */
  readonly swapMs?: number;
}

/** No places: the tour is flown. */
const NO_PLACES: PlacedFake = { places: [], computingMs: 0, misplacement: 1 };

/**
 * `VIEW`'s controls, canvas and primary label block as the run finds them, drawn into the page,
 * with the state they set: a free camera flown by `W` (or back by `S`) at its rate step in every
 * frame, each step at most `MAX_FREE_STEP_S` as `VIEW`'s, cut to look at a target's centre by the
 * target keys, and an `AUTO` that moves the exposure each frame.
 */
class FakeView {
  scene = "SERVER";
  chosenAtMs: number | null = null;
  style: "wireframe" | "photorealistic" = "wireframe";
  auto = false;
  meter = "AVG";
  preset: "SEAT" | "FREE" = "SEAT";
  rateStep = 6;
  ev = OPENING_EV;
  positionM: Vec3 = atmosphereSeatM();
  forward: Vec3 = normalise(sub(ATMOSPHERE_MOON_CENTRE_M, atmosphereSeatM()));
  selected: number | null = null;
  /** The flight key held, or `null`. */
  held: "w" | "s" | null = null;
  /** Every press and release of a flight key, in order. */
  readonly flightKeys: string[] = [];
  /** Called as a flight key is pressed. */
  onHold: () => void = () => undefined;
  /**
   * Whether its frame-rate floor stands stepped: its plate states `IMAGE: DETAIL LIMITED` and its
   * scene target's scale reads 0.67 after 1.
   */
  limited = false;
  /** Until when, ms of the test's clock, a body's record is building: `ATMOSPHERE: PENDING`. */
  buildingUntilMs: number | null = null;
  /** The place whose scene is chosen, or `null`. */
  place: AtmospherePlace | null = null;
  /** The place the run last offered, which either of the two placed scenes makes. */
  offered: AtmospherePlace | null = null;
  /** Until when, ms of the test's clock, a chosen place's bake runs: `ATMOSPHERE: COMPUTING`. */
  computingUntilMs: number | null = null;
  /**
   * Until when, ms of the test's clock, a chosen place's stage makes its pipelines: the wireframe
   * drawn, `PHOTOREALISTIC: PREPARING`.
   */
  preparingUntilMs: number | null = null;
  /** The scene the plate shows: the chosen one, once its stage stands. */
  shownScene = "SERVER";
  /** The primary's `GRID`: `ON` as each stage mounts. */
  grid = true;
  /** The `GRID` at each capture, in order. */
  readonly gridAtCaptures: boolean[] = [];
  /** A chosen scene whose stage is still to stand, and when it does, ms of the test's clock. */
  #swap: { readonly scene: string; readonly atMs: number } | null = null;
  readonly #clock: () => number;
  readonly #media: () => AtmosphereFixtureMedia;
  readonly #speeds: AutoSpeeds;
  readonly #placed: PlacedFake;
  readonly #canvas: HTMLCanvasElement;
  readonly #marks: HTMLElement;
  readonly #gridRow: HTMLElement;
  readonly #plate: RenderResult;
  /** The frames run so far, ms of the test's clock: every one a draw of the primary. */
  readonly #frames: number[] = [];
  #lastFrameMs = 0;
  #holds = 0;
  #pressedAtMs = 0;
  #stall: Stall | null;
  /** The plate's readings and statements as last drawn. */
  #shown = "";

  constructor(
    clock: () => number,
    media: () => AtmosphereFixtureMedia,
    speeds: AutoSpeeds,
    stall: Stall | null,
    placed: PlacedFake = NO_PLACES,
  ) {
    this.#clock = clock;
    this.#media = media;
    this.#speeds = speeds;
    this.#stall = stall;
    this.#placed = placed;
    const scenes = this.#fieldset("Scene", [
      "SERVER",
      ATMOSPHERE_SCENE_NAME,
      ...(placed.places.length === 0 ? [] : PLACE_SCENE_NAMES),
    ]);
    scenes.setAttribute("aria-label", "Scene");
    this.#fieldset("CONTROLS", ["PRIMARY", "INSTRUMENT 1", "INSTRUMENT 2"]);
    const exposure = document.createElement("section");
    exposure.className = "view-exposure";
    const enable = document.createElement("button");
    enable.textContent = "ENABLE";
    enable.addEventListener("click", () => {
      this.auto = true;
      enable.setAttribute("aria-disabled", "true");
      this.#redraw();
    });
    exposure.append(enable);
    const style = document.createElement("section");
    style.className = "view-style";
    const gridRow = document.createElement("fieldset");
    gridRow.className = "view-style__grid";
    for (const label of ["ON", "OFF"]) {
      const button = document.createElement("button");
      button.textContent = label;
      button.addEventListener("click", () => {
        this.grid = label === "ON";
        this.#redraw();
      });
      gridRow.append(button);
    }
    style.append(gridRow);
    this.#gridRow = gridRow;
    const meter = document.createElement("section");
    meter.className = "view-meter";
    for (const mode of ["AVG", "LIT", "DARK"]) {
      const button = document.createElement("button");
      button.textContent = mode;
      button.setAttribute("aria-pressed", String(mode === this.meter));
      button.addEventListener("click", () => {
        this.meter = mode;
        for (const each of meter.querySelectorAll("button")) {
          each.setAttribute("aria-pressed", String(each === button));
        }
      });
      meter.append(button);
    }
    this.#marks = document.createElement("div");
    this.#marks.setAttribute("role", "listbox");
    this.#marks.setAttribute("aria-label", "Marks in view");
    this.#canvas = document.createElement("canvas");
    this.#canvas.setAttribute("role", "application");
    this.#canvas.setAttribute("aria-describedby", "view-legend-label view-legend");
    this.#canvas.addEventListener("keydown", (event) => this.#canvasKey(event, true), {
      signal: listening.signal,
    });
    this.#canvas.addEventListener("keyup", (event) => this.#canvasKey(event, false), {
      signal: listening.signal,
    });
    document.body.append(exposure, style, meter, this.#marks, this.#canvas);
    document.body.addEventListener("keydown", (event) => this.#singleKey(event), {
      signal: listening.signal,
    });
    this.#plate = render(this.#block());
    this.#redraw();
  }

  /** The `CAMERA` line, as `VIEW` reads it. */
  cameraLine(): string {
    return this.preset === "SEAT" ? "SEAT" : `FREE · ${freeRateReading(this.rateStep)}`;
  }

  /** The `EXPOSURE` line, as `VIEW` reads it. */
  exposureLine(): string {
    return `EV100 ${this.ev.toFixed(1)} ${this.auto ? "AUTO" : "MAN"}`;
  }

  /** The camera's distance from a mark's centre, m. */
  rangeTo(designation: string): number {
    const mark = MARKS.find((each) => each.designation === designation);
    if (mark === undefined) {
      throw new Error(`the fake has no mark ${designation}`);
    }
    return norm(sub(mark.centreM, this.positionM));
  }

  /** When the page's next frame comes: a frame on, or the stall's length on where it falls due. */
  nextFrameMs(): number {
    const stall = this.#stall;
    return stall !== null &&
      this.held !== null &&
      this.#holds === stall.hold &&
      this.#lastFrameMs - this.#pressedAtMs >= stall.afterMs
      ? this.#lastFrameMs + stall.forMs
      : this.#lastFrameMs + FRAME_MS;
  }

  /** `VIEW`'s loop in each of the page's frames up to `toMs`: the camera flown, `AUTO` stepped. */
  runFramesTo(toMs: number): void {
    for (let atMs = this.nextFrameMs(); atMs <= toMs; atMs = this.nextFrameMs()) {
      if (atMs - this.#lastFrameMs > FRAME_MS) {
        this.#stall = null;
      }
      const dtS = Math.min(atMs - this.#lastFrameMs, 1000 * MAX_FREE_STEP_S) / 1000;
      this.#lastFrameMs = atMs;
      this.#frames.push(atMs);
      if (this.held !== null && this.preset === "FREE") {
        const sign = this.held === "w" ? 1 : -1;
        this.positionM = add(
          this.positionM,
          scale(this.forward, sign * freeRateMPerS(this.rateStep) * dtS),
        );
      }
      if (this.auto) {
        const targetEv = this.preset === "SEAT" ? NIGHT_EV : DAY_EV;
        const gap = targetEv - this.ev;
        const stepEv = (gap > 0 ? this.#speeds.brightenEvPerS : this.#speeds.darkenEvPerS) * dtS;
        this.ev = Math.abs(gap) <= stepEv ? targetEv : this.ev + Math.sign(gap) * stepEv;
      }
    }
    if (this.#swap !== null && toMs >= this.#swap.atMs) {
      this.#show(this.#swap.scene);
      this.#swap = null;
    }
    if (this.#plateText() !== this.#shown) {
      this.#redraw();
    }
  }

  /**
   * Stands the stage of `scene`: a place's, the one last offered, for one of the two scenes that
   * make them, its `SCENE` line the place's; else the scene's own.
   */
  #show(scene: string): void {
    const place = PLACE_SCENE_NAMES.some((name) => name === scene) ? this.offered : null;
    this.shownScene = place?.scene ?? scene;
    this.grid = true;
    this.#choosePlace(place);
  }

  /** The animation frames drawn from `startMs` up to `endMs`, by now. */
  drawTimes(startMs: number, endMs: number): number[] {
    return this.#frames.filter((atMs) => atMs >= startMs && atMs < endMs);
  }

  #statements(): ReadonlyArray<string> {
    if (this.scene === "SERVER") {
      return [];
    }
    if (this.#media().kind === "loading") {
      return [ATMOSPHERE_STATEMENTS.atmospherePending];
    }
    let statements =
      this.buildingUntilMs !== null && this.#clock() < this.buildingUntilMs
        ? [ATMOSPHERE_STATEMENTS.atmospherePending, ...PLATE_STATEMENTS]
        : PLATE_STATEMENTS;
    if (this.computingUntilMs !== null && this.#clock() < this.computingUntilMs) {
      statements = [ATMOSPHERE_STATEMENTS.atmosphereComputing, ...statements];
    }
    if (this.#preparing()) {
      statements = [PHOTOREAL_PREPARING, ...statements];
    }
    return this.limited ? [...statements, IMAGE_DETAIL_LIMITED] : statements;
  }

  /**
   * The `SCENE` line of a kept scene, and the `FRAME` and `POSITION` lines at a place, its
   * fixture's, as `VIEW` reads them.
   */
  #placeLines(): ReadonlyArray<{ readonly label: string; readonly value: string }> {
    const { place } = this;
    const scene = this.shownScene === "SERVER" ? [] : [{ label: "SCENE", value: this.shownScene }];
    if (place === null) {
      return scene;
    }
    const range = formatBodyDistance(this.rangeTo(place.designation) / 1000, null);
    return [
      ...scene,
      { label: "FRAME", value: `BODY ${place.designation}` },
      { label: "POSITION", value: `${range.value} ${range.unit} 047° +12°` },
    ];
  }

  /** When a chosen place's stage asked for its pipelines, ms of the test's clock. */
  preparingFromMs: number | null = null;

  /** Whether a chosen place's stage still makes its pipelines. */
  #preparing(): boolean {
    const now = this.#clock();
    return (
      this.preparingUntilMs !== null &&
      now < this.preparingUntilMs &&
      (this.preparingFromMs === null || now >= this.preparingFromMs)
    );
  }

  /** The style the primary draws: the wireframe while its pipelines are made. */
  #drawnStyle(): "wireframe" | "photorealistic" {
    return this.#preparing() ? "wireframe" : this.style;
  }

  #plateText(): string {
    return [
      this.#drawnStyle(),
      this.cameraLine(),
      this.exposureLine(),
      ...this.#placeLines().map((line) => line.value),
      ...this.#statements(),
    ].join("|");
  }

  #block() {
    return (
      <ViewLabelBlock
        id="view-legend-label"
        lines={[
          { label: "CAMERA", value: this.cameraLine() },
          { label: "EXPOSURE", value: this.exposureLine() },
          ...this.#placeLines(),
        ]}
        statements={this.#statements()}
        countLine={null}
        faults={this.scene === "SERVER" ? [] : PLATE_FAULTS}
      />
    );
  }

  #fieldset(legend: string, labels: ReadonlyArray<string>): HTMLFieldSetElement {
    const set = document.createElement("fieldset");
    const title = document.createElement("legend");
    title.textContent = legend;
    set.append(title);
    for (const label of labels) {
      const button = document.createElement("button");
      button.textContent = label;
      button.addEventListener("click", () => {
        if (legend === "Scene") {
          this.scene = label;
          // The media are the page's, loading from the first choice of a scene that draws them.
          this.chosenAtMs ??= this.#clock();
          const swapMs = this.#placed.swapMs ?? 0;
          if (swapMs > 0) {
            this.#swap = { scene: label, atMs: this.#clock() + swapMs };
          } else {
            this.#show(label);
          }
          for (const each of set.querySelectorAll("button")) {
            each.setAttribute("aria-pressed", String(each === button));
          }
          this.#redraw();
        }
      });
      set.append(button);
    }
    document.body.append(set);
    return set;
  }

  /** Stands the free camera at `place`, if any, its bake running a while. */
  #choosePlace(place: AtmospherePlace | null): void {
    this.place = place;
    if (place === null) {
      return;
    }
    const centreM = MARKS.find((mark) => mark.designation === place.designation)?.centreM;
    if (centreM === undefined) {
      throw new Error(`the fake has no mark ${place.designation}`);
    }
    this.preset = "FREE";
    this.positionM = add(centreM, scale(place.fromCentreM, this.#placed.misplacement));
    this.forward = rotate(place.orientation, vec3(0, 0, -1));
    this.computingUntilMs = this.#clock() + this.#placed.computingMs;
    this.preparingFromMs = this.#clock() + (this.#placed.preparingAfterMs ?? 0);
    this.preparingUntilMs = this.preparingFromMs + (this.#placed.preparingMs ?? 0);
  }

  #singleKey(event: KeyboardEvent): void {
    if (event.target !== document.body) {
      return;
    }
    if (event.key === STYLE_TOGGLE_KEY) {
      this.style = this.style === "wireframe" ? "photorealistic" : "wireframe";
    } else if (event.key === "3") {
      this.preset = "FREE";
    } else if (event.key === "]") {
      this.selected = this.selected === null ? 0 : (this.selected + 1) % MARKS.length;
      const mark = MARKS[this.selected];
      if (mark !== undefined && this.preset === "FREE") {
        this.forward = normalise(sub(mark.centreM, this.positionM));
      }
    }
    this.#redraw();
  }

  #canvasKey(event: KeyboardEvent, down: boolean): void {
    if (event.key === "w" || event.key === "s") {
      this.held = down ? event.key : null;
      this.flightKeys.push(`${event.key.toUpperCase()} ${down ? "down" : "up"}`);
      if (down) {
        this.#holds += 1;
        this.#pressedAtMs = this.#lastFrameMs;
        this.onHold();
      }
    } else if (down && event.key === "PageUp") {
      this.rateStep = Math.min(31, this.rateStep + 1);
    } else if (down && event.key === "PageDown") {
      this.rateStep = Math.max(0, this.rateStep - 1);
    }
    this.#redraw();
  }

  #redraw(): void {
    this.#shown = this.#plateText();
    for (const button of this.#gridRow.querySelectorAll("button")) {
      button.setAttribute("aria-pressed", String((button.textContent === "ON") === this.grid));
    }
    this.#canvas.setAttribute(
      "aria-label",
      `VIEW, ${this.#drawnStyle() === "wireframe" ? "WIREFRAME" : "PHOTOREALISTIC"}, PRIMARY, ${this.preset}`,
    );
    // Windowed as `ViewMarkList` is: the rows about the selected one alone, each giving the
    // list's length as its `aria-setsize`.
    const from = Math.max(0, Math.min((this.selected ?? 0) - 2, MARKS.length - MARK_WINDOW_ROWS));
    this.#marks.replaceChildren(
      ...MARKS.slice(from, from + MARK_WINDOW_ROWS).map((mark, shown) => {
        const index = from + shown;
        const option = document.createElement("div");
        option.setAttribute("role", "option");
        option.setAttribute("aria-selected", String(index === this.selected));
        option.setAttribute("aria-posinset", String(index + 1));
        option.setAttribute("aria-setsize", String(MARKS.length));
        option.setAttribute("aria-label", `${mark.designation}, PLANET, range 1.00 AU`);
        return option;
      }),
    );
    this.#plate.rerender(this.#block());
  }
}

/** The media as they land: loading until `mediaMs` after the scene's choice, then ready. */
function landing(
  view: () => FakeView | null,
  clock: () => number,
  mediaMs: number,
): () => AtmosphereFixtureMedia {
  return () => {
    const chosenAtMs = view()?.chosenAtMs ?? null;
    return chosenAtMs !== null && clock() >= chosenAtMs + mediaMs
      ? { kind: "ready", media: new Map() }
      : { kind: "loading" };
  };
}

/** What a run on the fake saw: its view, the captures asked for, and its deps. */
interface FakeRun {
  readonly view: FakeView;
  readonly captured: string[];
  readonly controller: AbortController;
  readonly deps: AtmosphereCheckDeps;
}

/** The run's deps over a fake `VIEW` and probe on the test's clock, a smoke's timing. */
function fakeRun(
  options: {
    readonly media?: AtmosphereFixtureMedia;
    /** When the media land after the scene's choice, ms: {@link MEDIA_MS} by default. */
    readonly mediaMs?: number;
    readonly speeds?: AutoSpeeds;
    readonly stall?: Stall;
    /**
     * Whether the run's frame callback comes before `VIEW`'s in each frame, rather than after it:
     * `VIEW`'s then runs once the run's has acted.
     */
    readonly runFirst?: boolean;
    /** The software mode's places, the fake's scenes for them, and the run's. */
    readonly placed?: PlacedFake;
  } = {},
): FakeRun {
  let now = 0;
  const clock = (): number => now;
  let made: FakeView | null = null;
  const given = options.media;
  const media =
    given === undefined ? landing(() => made, clock, options.mediaMs ?? MEDIA_MS) : () => given;
  const view = new FakeView(
    clock,
    media,
    options.speeds ?? { brightenEvPerS: 3, darkenEvPerS: 1 },
    options.stall ?? null,
    options.placed,
  );
  made = view;
  const captured: string[] = [];
  let offers = 0;
  const controller = new AbortController();
  /** A frame the run has been called back in whose `VIEW` callback is still to come. */
  let pendingMs: number | null = null;
  const advanceTo = (toMs: number): void => {
    if (pendingMs !== null) {
      view.runFramesTo(pendingMs);
      pendingMs = null;
    }
    now = toMs;
    view.runFramesTo(toMs);
  };
  const deps: AtmosphereCheckDeps = {
    api: {
      capture: (label) => {
        captured.push(`${label}: ${view.meter} ${view.auto ? "AUTO" : "MAN"} ${view.preset}`);
        view.gridAtCaptures.push(view.grid);
        return Promise.resolve();
      },
    },
    views: {
      canvasOf: () => ({ widthPx: 1126, heightPx: 906 }),
      timer: () => "full",
      faults: () => [],
      window: (startMs, endMs): WindowFigures => {
        const draws = view.drawTimes(startMs, endMs);
        return {
          startMs,
          endMs,
          views: [
            {
              name: "view",
              style: "photorealistic",
              canvas: { widthPx: 1126, heightPx: 906 },
              draws: draws.length,
              gpuMs: [],
              submitMs: [],
              passLabels: [],
              scales: view.limited ? [1, 0.67] : [],
            },
          ],
          frameIntervalsMs: [],
          primaryIntervalsMs: draws.slice(1).map((atMs, i) => atMs - (draws[i] ?? atMs)),
          mainThreadMs: [],
          frameGpuMs: [],
          untimedFrames: 0,
          droppedResolves: 0,
          unattributedGpuMs: 0,
        };
      },
      drawTimes: (_view, startMs, endMs) => view.drawTimes(startMs, endMs),
    },
    fixtures: {
      get media() {
        return media();
      },
    },
    document,
    setting: "high",
    timing: ATMOSPHERE_CHECK_TIMING.smoke,
    nowMs: clock,
    sleep: (ms) => {
      advanceTo(now + ms);
      return Promise.resolve();
    },
    frame: () => {
      advanceTo(now);
      const atMs = view.nextFrameMs();
      if (options.runFirst === true) {
        now = atMs;
        pendingMs = atMs;
      } else {
        advanceTo(atMs);
      }
      return Promise.resolve(atMs);
    },
    signal: controller.signal,
    devicePixelRatio: 1,
    placed:
      options.placed === undefined
        ? null
        : {
            places: options.placed.places,
            offer: (place) => {
              view.offered = place;
              offers += 1;
              return PLACE_SCENE_NAMES[offers % 2] ?? PLACE_SCENE_NAMES[0];
            },
          },
  };
  return { view, captured, controller, deps };
}

/** A smoke run on the fake, to its end. */
async function smoke(): Promise<{ readonly record: AtmosphereCheckRecord; readonly run: FakeRun }> {
  const run = fakeRun();
  return { record: await runAtmosphereCheck(run.deps), run };
}

/** `TEST EARTH O3 SLAB`'s goal range, m. */
const SLAB_GOAL_M =
  ATMOSPHERE_GOAL_RADII *
  (ATMOSPHERE_FIXTURES.find((each) => each.designation === "TEST EARTH O3 SLAB")?.figure
    .equatorialRadiusM ?? Number.NaN);

describe("the atmosphere run's script (R08.T10.b)", () => {
  it("ends the run with its reason when ATMOSPHERE TEST's media fail to load", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { deps, controller } = fakeRun({ media: { kind: "failed" } });
    const api = ending();
    await recordAtmosphereCheck(api, () => runAtmosphereCheck(deps), controller.signal);
    expect(api.end.mock.calls).toEqual([
      [{ status: "fail", reason: "views check: ATMOSPHERE TEST's media did not load" }],
    ]);
  });

  it("waits past a control's wait for media that land within the media's, as the thick cases' do", async () => {
    const run = fakeRun({ mediaMs: 90_000 });
    const record = await runAtmosphereCheck(run.deps);
    expect([
      record.mediaReadyMs >= 90_000,
      ATMOSPHERE_CHECK_TIMING.smoke.waitMs < 90_000,
      record.captures.length,
    ]).toEqual([true, true, 2]);
  });

  it("ends the run with its reason when the media do not land within the media's wait", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { deps, controller } = fakeRun({ media: { kind: "loading" } });
    const api = ending();
    await recordAtmosphereCheck(api, () => runAtmosphereCheck(deps), controller.signal);
    expect(api.end.mock.calls).toEqual([
      [
        {
          status: "fail",
          reason: "views check: ATMOSPHERE TEST's media did not load within 180 s",
        },
      ],
    ]);
  });

  it("takes the moonlit night first, under AUTO on AVG from the seat, then the smoke's fixture on LIT", async () => {
    const { record, run } = await smoke();
    expect([record.captures.map((each) => [each.kind, each.label]), run.captured]).toEqual([
      [
        ["night", "moonlit-night"],
        ["fixture", "fixture-test-earth-o3-slab"],
      ],
      ["moonlit-night: AVG AUTO SEAT", "fixture-test-earth-o3-slab: LIT AUTO FREE"],
    ]);
  });

  it("records each capture with the plate's statements and fault lines", async () => {
    const { record } = await smoke();
    expect(record.captures.map((each) => [each.statements, each.faults])).toEqual([
      [PLATE_STATEMENTS, PLATE_FAULTS],
      [PLATE_STATEMENTS, PLATE_FAULTS],
    ]);
  });

  it("records each capture's primary scale and air: none recorded, and the setting's", async () => {
    const { record } = await smoke();
    expect(record.captures.map((each) => [each.primaryScale, each.airSetting])).toEqual([
      [null, "high"],
      [null, "high"],
    ]);
  });

  it("records a stepped capture's last scale and the low setting's air (fix-retina-frame)", async () => {
    const run = fakeRun();
    run.view.limited = true;
    const record = await runAtmosphereCheck(run.deps);
    expect(record.captures.map((each) => [each.primaryScale, each.airSetting])).toEqual([
      [0.67, "low"],
      [0.67, "low"],
    ]);
  });

  it("records each capture drawn photorealistic, as the primary's canvas names it", async () => {
    const { record } = await smoke();
    expect(record.captures.map((each) => each.style)).toEqual(["photorealistic", "photorealistic"]);
  });

  it("records a capture drawn in the wireframe where the primary's canvas names it so", async () => {
    const run = fakeRun();
    let holds = 0;
    // From the fixture's leg's first hold, after the night's capture, the canvas reads
    // `VIEW, WIREFRAME, PRIMARY, FREE`, as where the photorealistic style could not be remade.
    run.view.onHold = () => {
      holds += 1;
      if (holds === 2) {
        run.view.style = "wireframe";
      }
    };
    const record = await runAtmosphereCheck(run.deps);
    expect(record.captures.map((each) => [each.label, each.style])).toEqual([
      ["moonlit-night", "photorealistic"],
      ["fixture-test-earth-o3-slab", "wireframe"],
    ]);
  });

  it("captures a fixture only once no ATMOSPHERE: PENDING stands, as while TEST JUPITER's record builds (R08.T16.a.2)", async () => {
    const run = fakeRun();
    let holds = 0;
    let buildingUntilMs = 0;
    // From the fixture's leg's first hold, a record builds for 40 s of the test's clock.
    run.view.onHold = () => {
      holds += 1;
      if (holds === 2) {
        buildingUntilMs = run.deps.nowMs() + 40_000;
        run.view.buildingUntilMs = buildingUntilMs;
      }
    };
    const record = await runAtmosphereCheck(run.deps);
    const [, fixture] = record.captures;
    expect([
      (fixture?.atMs ?? 0) >= buildingUntilMs,
      fixture?.statements.includes(ATMOSPHERE_STATEMENTS.atmospherePending),
    ]).toEqual([true, false]);
  });

  it("takes the night only once AUTO has settled there, and records the wait", async () => {
    const { record } = await smoke();
    const [night] = record.captures;
    // AUTO darkens at 1 EV/s, from the opening's 15 to the night's -3.
    expect([night?.exposure, night?.camera, (night?.exposureSettleMs ?? 0) >= 18_000]).toEqual([
      "EV100 -3.0 AUTO",
      "SEAT",
      true,
    ]);
  });

  it("fails with AUTO's last readings when it does not settle within the longest wait", async () => {
    const { deps } = fakeRun({ speeds: { brightenEvPerS: 3, darkenEvPerS: 0.25 } });
    await expect(runAtmosphereCheck(deps)).rejects.toThrow(
      /^views check: AUTO did not settle before moonlit-night within 45 s \(the EXPOSURE line read EV100 [\d.-]+, then [\d.-]+\)$/u,
    );
  });

  it("captures the fixture at its goal's range as reckoned, to a few percent, and at the camera's true range", async () => {
    const { record, run } = await smoke();
    const slab = record.captures[1];
    if (slab?.kind !== "fixture") {
      throw new Error("the smoke took no fixture capture");
    }
    expect([
      slab.designation,
      Math.abs(slab.rangeM / SLAB_GOAL_M - 1) < 0.05,
      Math.abs(slab.rangeM - run.view.rangeTo("TEST EARTH O3 SLAB")) < 1,
      slab.phaseDeg > 40 && slab.phaseDeg < 50,
    ]).toEqual(["TEST EARTH O3 SLAB", true, true, true]);
  });

  it("lets W go after each hold, ending with it released", async () => {
    const { run } = await smoke();
    expect([run.view.held, run.view.flightKeys]).toEqual([
      null,
      ["W down", "W up", "W down", "W up", "W down", "W up", "W down", "W up"],
    ]);
  });

  it("records each stage as flown, at the rate step it asks for, held to half a frame of its plan", async () => {
    const { record } = await smoke();
    expect(
      record.stages.map((each) => [
        each.target,
        each.stage,
        each.rateStep ===
          legRateStep(Math.abs(each.askedM), ATMOSPHERE_CHECK_TIMING.smoke.legMaxS, 31),
        Math.abs(each.heldMs - each.holdMs) <= FRAME_MS / 2,
      ]),
    ).toEqual([
      ["TEST MOON", 1, true, true],
      ["TEST EARTH O3 SLAB", 1, true, true],
      ["TEST EARTH O3 SLAB", 2, true, true],
      ["TEST EARTH O3 SLAB", 3, true, true],
    ]);
  });

  it("flies back with S where a late frame overran the last stage, to its goal", async () => {
    // The fourth hold, the first fixture's last stage, about 4.7 s long, overruns by 0.1 s.
    const run = fakeRun({ stall: { hold: 4, afterMs: 4_600, forMs: 1_000 } });
    const record = await runAtmosphereCheck(run.deps);
    const slab = record.captures[1];
    if (slab?.kind !== "fixture") {
      throw new Error("the run took no fixture capture");
    }
    expect([
      run.view.flightKeys.slice(-4),
      record.stages.filter((each) => each.longestStepMs >= 1_000).map((each) => each.stage),
      (record.stages.at(-1)?.askedM ?? 0) < 0,
      Math.abs(slab.rangeM / SLAB_GOAL_M - 1) < ATMOSPHERE_END_TOLERANCE,
      Math.abs(slab.rangeM - run.view.rangeTo("TEST EARTH O3 SLAB")) < 1,
    ]).toEqual([["W down", "W up", "S down", "S up"], [3], true, true, true]);
  });

  it("reckons the camera's true range where the run's frame callback comes before VIEW's", async () => {
    const run = fakeRun({ runFirst: true });
    const record = await runAtmosphereCheck(run.deps);
    const slab = record.captures[1];
    if (slab?.kind !== "fixture") {
      throw new Error("the run took no fixture capture");
    }
    expect([
      Math.abs(slab.rangeM / SLAB_GOAL_M - 1) < ATMOSPHERE_END_TOLERANCE,
      Math.abs(slab.rangeM - run.view.rangeTo("TEST EARTH O3 SLAB")) < 1,
    ]).toEqual([true, true]);
  });

  it("lets W go when it is stopped mid-hold", async () => {
    const { deps, view, controller } = fakeRun();
    view.onHold = () => {
      controller.abort();
    };
    await expect(runAtmosphereCheck(deps)).rejects.toThrow(STOPPED);
    expect([view.held, view.flightKeys]).toEqual([null, ["W down", "W up"]]);
  });
});

/** A smoke's places and a look from the ground: the software mode's, as `App` gives them. */
const PLACES = atmospherePlaces(
  ATMOSPHERE_SMOKE_TOUR,
  ATMOSPHERE_LOOKS.filter((look) => look.label === "ground-test-earth-noon"),
);

describe("the atmosphere run placed, in the software mode (R08.T17)", () => {
  it("takes the night from the seat, then each place from its own scene with GRID OFF, flying nothing", async () => {
    const run = fakeRun({ placed: { places: PLACES, computingMs: 0, misplacement: 1 } });
    const record = await runAtmosphereCheck(run.deps);
    expect([
      record.captures.map((each) => [each.kind, each.label]),
      run.view.gridAtCaptures,
      run.view.flightKeys,
      record.stages,
    ]).toEqual([
      [
        ["night", "moonlit-night"],
        ["fixture", "fixture-test-earth-o3-slab"],
        ["fixture", "ground-test-earth-noon"],
      ],
      [true, false, false],
      [],
      [],
    ]);
  });

  it("captures a place only once its stage draws photorealistic, its pipelines made, past a control's wait", async () => {
    const preparingMs = 90_000;
    // Each place's stage stands a second after its choice, the one before standing in meanwhile.
    const run = fakeRun({
      placed: { places: PLACES, computingMs: 0, misplacement: 1, preparingMs, swapMs: 1_000 },
    });
    const record = await runAtmosphereCheck(run.deps);
    const placed = record.captures.slice(1);
    expect([
      ATMOSPHERE_CHECK_TIMING.smoke.waitMs < preparingMs,
      placed.map((each) => [each.style, each.statements.includes(PHOTOREAL_PREPARING)]),
      placed.every((each, i) => i === 0 || each.atMs - (placed[i - 1]?.atMs ?? 0) >= preparingMs),
    ]).toEqual([
      true,
      [
        ["photorealistic", false],
        ["photorealistic", false],
      ],
      true,
    ]);
  });

  it("takes a place again where its stage asked for its pipelines after it first read photorealistic", async () => {
    // Each place's stage reads photorealistic for 2 s, past the frames the run waits for, then
    // prepares for 5 s: the run's first capture of the slab is taken over its wireframe, and again;
    // the noon's, whose AUTO settles past the 2 s, once.
    const run = fakeRun({
      placed: {
        places: PLACES,
        computingMs: 0,
        misplacement: 1,
        preparingAfterMs: 2_000,
        preparingMs: 5_000,
      },
    });
    const record = await runAtmosphereCheck(run.deps);
    expect([
      record.captures.map((each) => [
        each.label,
        each.style,
        each.statements.includes(PHOTOREAL_PREPARING),
      ]),
      run.captured.map((each) => each.split(":")[0]),
    ]).toEqual([
      [
        ["moonlit-night", "photorealistic", false],
        ["fixture-test-earth-o3-slab", "photorealistic", false],
        ["ground-test-earth-noon", "photorealistic", false],
      ],
      [
        "moonlit-night",
        "fixture-test-earth-o3-slab",
        "fixture-test-earth-o3-slab",
        "ground-test-earth-noon",
      ],
    ]);
  });

  it("captures a place only once no ATMOSPHERE: COMPUTING stands, past a control's wait", async () => {
    const computingMs = 90_000;
    const run = fakeRun({ placed: { places: PLACES, computingMs, misplacement: 1 } });
    const record = await runAtmosphereCheck(run.deps);
    const placed = record.captures.slice(1);
    expect([
      ATMOSPHERE_CHECK_TIMING.smoke.waitMs < computingMs,
      placed.map(
        (each) =>
          each.statements.includes(ATMOSPHERE_STATEMENTS.atmosphereComputing) ||
          each.statements.includes(ATMOSPHERE_STATEMENTS.atmospherePending),
      ),
      placed.every((each, i) => i === 0 || each.atMs - (placed[i - 1]?.atMs ?? 0) >= computingMs),
    ]).toEqual([true, [false, false], true]);
  });

  it("records each place's range as its POSITION line reads it, within the tolerance of its own", async () => {
    const run = fakeRun({ placed: { places: PLACES, computingMs: 0, misplacement: 1 } });
    const record = await runAtmosphereCheck(run.deps);
    expect(
      record.captures.flatMap((each) =>
        each.kind === "fixture"
          ? [
              [
                each.designation,
                Math.abs(
                  each.rangeM /
                    (PLACES.find((place) => place.label === each.label)?.rangeM ?? Number.NaN) -
                    1,
                ) < 0.005,
              ],
            ]
          : [],
      ),
    ).toEqual([
      ["TEST EARTH O3 SLAB", true],
      ["TEST EARTH", true],
    ]);
  });

  it("fails where a place's camera reads more than the tolerance off its range", async () => {
    const run = fakeRun({
      placed: { places: PLACES, computingMs: 0, misplacement: 1 + 2 * PLACE_RANGE_TOLERANCE },
    });
    await expect(runAtmosphereCheck(run.deps)).rejects.toThrow(
      /^views check: the camera at fixture-test-earth-o3-slab read POSITION 52\.0 Mm 047° \+12°, placed 50968 km from TEST EARTH O3 SLAB's centre$/u,
    );
  });

  it("fails where a place's bake outlasts the media's wait", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const run = fakeRun({ placed: { places: PLACES, computingMs: 1e9, misplacement: 1 } });
    await expect(runAtmosphereCheck(run.deps)).rejects.toThrow(
      "views check: ATMOSPHERE: COMPUTING still stood at fixture-test-earth-o3-slab within 180 s",
    );
  });
});
