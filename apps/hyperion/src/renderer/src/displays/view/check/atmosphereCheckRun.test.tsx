import { render, type RenderResult } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { AtmosphereCheckRecord, ViewsCheckApi } from "../../../../../preload/api";
import { add, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import { ATMOSPHERE_STATEMENTS } from "../../../view/atmosphere/labels";
import { freeRateMPerS, MAX_FREE_STEP_S } from "../../../view/camera/freeCamera";
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
import { freeRateReading } from "../viewRun";
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
  heldMs,
  legRateStep,
  quantile,
  rateStepOfCameraLine,
  recordAtmosphereCheck,
  runAtmosphereCheck,
  stageRangesM,
} from "./atmosphereCheckRun";
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

  it("comes on each fixture's day side, between 40° and 75° of phase", () => {
    expect(captured.map(({ phaseDeg }) => phaseDeg > 40 && phaseDeg < 75)).toEqual([
      true,
      true,
      true,
      true,
    ]);
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
      ["TEST EARTH", 3],
    ]);
  });

  it("passes no body but its target within five of its radii, but the planet it climbs from", () => {
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
      atmosphereLegs(atmosphereSeatM(), [{ kind: "fixture", designation: "TEST VENUS" }]),
    ).toThrow("has no fixture TEST VENUS");
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
  readonly #clock: () => number;
  readonly #media: () => AtmosphereFixtureMedia;
  readonly #speeds: AutoSpeeds;
  readonly #canvas: HTMLCanvasElement;
  readonly #marks: HTMLElement;
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
  ) {
    this.#clock = clock;
    this.#media = media;
    this.#speeds = speeds;
    this.#stall = stall;
    const scenes = this.#fieldset("Scene", ["SERVER", ATMOSPHERE_SCENE_NAME]);
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
    document.body.append(exposure, meter, this.#marks, this.#canvas);
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
    if (this.#plateText() !== this.#shown) {
      this.#redraw();
    }
  }

  /** The animation frames drawn from `startMs` up to `endMs`, by now. */
  drawTimes(startMs: number, endMs: number): number[] {
    return this.#frames.filter((atMs) => atMs >= startMs && atMs < endMs);
  }

  #statements(): ReadonlyArray<string> {
    if (this.scene !== ATMOSPHERE_SCENE_NAME) {
      return [];
    }
    return this.#media().kind === "loading"
      ? [ATMOSPHERE_STATEMENTS.atmospherePending]
      : PLATE_STATEMENTS;
  }

  #plateText(): string {
    return [this.cameraLine(), this.exposureLine(), ...this.#statements()].join("|");
  }

  #block() {
    return (
      <ViewLabelBlock
        id="view-legend-label"
        lines={[
          { label: "CAMERA", value: this.cameraLine() },
          { label: "EXPOSURE", value: this.exposureLine() },
        ]}
        statements={this.#statements()}
        countLine={null}
        faults={this.scene === ATMOSPHERE_SCENE_NAME ? PLATE_FAULTS : []}
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
          this.chosenAtMs = this.#clock();
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
    this.#canvas.setAttribute(
      "aria-label",
      `VIEW, ${this.style === "wireframe" ? "WIREFRAME" : "PHOTOREALISTIC"}, PRIMARY, ${this.preset}`,
    );
    this.#marks.replaceChildren(
      ...MARKS.map((mark, index) => {
        const option = document.createElement("div");
        option.setAttribute("role", "option");
        option.setAttribute("aria-selected", String(index === this.selected));
        option.setAttribute("aria-label", `${mark.designation}, PLANET, range 1.00 AU`);
        return option;
      }),
    );
    this.#plate.rerender(this.#block());
  }
}

/** The media as they land: loading until {@link MEDIA_MS} after the scene's choice, then ready. */
function landing(view: () => FakeView | null, clock: () => number): () => AtmosphereFixtureMedia {
  return () => {
    const chosenAtMs = view()?.chosenAtMs ?? null;
    return chosenAtMs !== null && clock() >= chosenAtMs + MEDIA_MS
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
    readonly speeds?: AutoSpeeds;
    readonly stall?: Stall;
    /**
     * Whether the run's frame callback comes before `VIEW`'s in each frame, rather than after it:
     * `VIEW`'s then runs once the run's has acted.
     */
    readonly runFirst?: boolean;
  } = {},
): FakeRun {
  let now = 0;
  const clock = (): number => now;
  let made: FakeView | null = null;
  const given = options.media;
  const media = given === undefined ? landing(() => made, clock) : () => given;
  const view = new FakeView(
    clock,
    media,
    options.speeds ?? { brightenEvPerS: 3, darkenEvPerS: 1 },
    options.stall ?? null,
  );
  made = view;
  const captured: string[] = [];
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
              scales: [],
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
