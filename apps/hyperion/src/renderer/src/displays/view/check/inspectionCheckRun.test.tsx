import { RequestClient } from "@hyperion/protocol";
import { describe, expect, it, vi } from "vitest";

import type { InspectionTimingWindow, SpikeEnd } from "../../../../../preload/api";
import { freeRateMPerS, MAX_FREE_STEP_S } from "../../../view/camera/freeCamera";
import type { DrawItem, FrameSubmission } from "../../../view/engine/types";
import { LIGHTING_NOT_RECEIVED, LIGHTING_PENDING } from "../../../view/lighting/hostLights";
import { TERRAIN_LOWEST_ALTITUDE_M } from "../../../view/scenes/terrainScene";
import { positionRangeM } from "./atmosphereCheckRun";
import {
  awaitedOrNot,
  cameraRangeM,
  cameraRateStep,
  closeRateStep,
  drawsLitBody,
  INSPECTION_CHECK_TIMING,
  inspectionCaptureLabel,
  inspectionGround,
  inspectionLighting,
  LitBodyDraws,
  CLOSE_TOLERANCE,
  GROUNDED_SLANT_M,
  holdForStep,
  ReckonedSlant,
  LOW_APPROACH_M,
  LOW_APPROACH_TOLERANCE_M,
  LOW_FRESH_MAX_DRAWS,
  LOW_HEIGHT_M,
  LOW_HOLD_SHARE,
  LOW_REST_DRAWS,
  lowApproachRangeM,
  lowRemainingM,
  nextHoldShare,
  readoutHeldBack,
  recordInspectionCheck,
  restedSlantM,
  runInspectionCheck,
  slantRangeM,
  starsPending,
} from "./inspectionCheckRun";
import { lowerYieldLine } from "../viewRoom";
import { freeRateReading } from "../viewRun";
import { STOPPED } from "./viewsCheckRun";
import type { SubmissionWatcher } from "./viewsProbe";

/** A profiled window, as the frame profile measured it. */
const WINDOW: InspectionTimingWindow = {
  name: "orbit",
  startMs: 0,
  endMs: 1,
  canvas: { widthPx: 1, heightPx: 1 },
  scales: [],
  intervalsMs: [],
  mainThreadMs: [],
  submitMs: [],
  gpuMs: [],
  passes: [],
  untimedFrames: 0,
  memory: {
    uploaded: { count: 0, bytes: 0 },
    created: { count: 0, bytes: 0 },
    destroyed: { count: 0, bytes: 0 },
    uploads: [],
  },
  gpuCalls: null,
  gpuAttachments: null,
  atmosphereReads: [],
  react: null,
  clusters: null,
  marchSamples: 36,
  position: null,
  statements: [],
  marchCensus: null,
};

/** A body the launch names. */
const BODY = "61ffd967fe000023.0100";

/** The main process's rule for a capture's label (`viewsCheckRecord.ts`' `CAPTURE_LABEL`). */
const CAPTURE_LABEL = /^[a-z0-9-]{1,48}$/u;

/** A request client whose server answers every request with a list of no universe. */
function noUniverses(): RequestClient {
  const client: RequestClient = new RequestClient((message) => {
    if (message.type === "request") {
      queueMicrotask(() => {
        client.handleServerMessage({
          type: "response",
          id: message.id,
          body: { kind: "list_universes", universes: [], server_generator_version: 24 },
        });
      });
    }
    return true;
  });
  return client;
}

describe("the inspection run (the owner's request)", () => {
  it("names its capture for the body's system and index, as a capture may be named", () => {
    const label = inspectionCaptureLabel(BODY);
    expect(label).toBe("inspection-61ffd967fe000023-0100");
    expect(CAPTURE_LABEL.test(label)).toBe(true);
  });

  it("names its maximized and GRID ON captures as a capture may be named (2026-11-05)", () => {
    const label = inspectionCaptureLabel(BODY);
    expect(
      [`${label}-maximized`, `${label}-grid-on`].map((each) => CAPTURE_LABEL.test(each)),
    ).toEqual([true, true]);
  });

  it("fails naming the universe where the server lists none of that ID", async () => {
    const openSystem = vi.fn<() => void>();
    await expect(
      runInspectionCheck({
        api: { capture: () => Promise.resolve() },
        views: { drawTimes: () => [], watch: () => () => undefined },
        requests: noUniverses(),
        served: { universe: "3504954ff4feabf4", body: BODY },
        openSystem,
        document,
        nowMs: () => 0,
        sleep: () => Promise.resolve(),
        signal: new AbortController().signal,
        timing: INSPECTION_CHECK_TIMING,
        profile: null,
      }),
    ).rejects.toThrow("views check: the server lists no universe 3504954ff4feabf4");
    expect(openSystem).not.toHaveBeenCalled();
  });

  it("ends passed once its capture is taken", async () => {
    const end = vi.fn<() => Promise<void>>(() => Promise.resolve());

    await recordInspectionCheck(
      { end },
      () =>
        Promise.resolve({
          label: "a",
          frame: null,
          exposure: null,
          statements: [],
          ground: [],
          closeGround: null,
        }),
      new AbortController().signal,
      null,
    );

    expect(end.mock.calls).toEqual([[{ status: "pass" }]]);
  });

  it("writes its frame profile's timing file before it ends passed (the owner's report)", async () => {
    const order: string[] = [];
    await recordInspectionCheck(
      {
        end: (outcome) => {
          order.push(`end ${outcome.status}`);
          return Promise.resolve();
        },
      },
      () =>
        Promise.resolve({
          label: "a",
          frame: null,
          exposure: null,
          statements: [],
          ground: [],
          closeGround: null,
        }),
      new AbortController().signal,
      {
        windows: [WINDOW],
        write: () => {
          order.push("write timing");
          return Promise.resolve();
        },
      },
    );

    expect(order).toEqual(["write timing", "end pass"]);
  });

  it("writes the windows it measured before it ends failed, a later step having failed", async () => {
    const order: string[] = [];
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    await recordInspectionCheck(
      {
        end: (outcome) => {
          order.push(`end ${outcome.status}`);
          return Promise.resolve();
        },
      },
      () =>
        Promise.reject(new Error("views check: the primary did not draw 5 frames within 300 s")),
      new AbortController().signal,
      {
        windows: [WINDOW],
        write: () => {
          order.push("write timing");
          return Promise.resolve();
        },
      },
    );

    expect(order).toEqual(["write timing", "end fail"]);
  });

  it("ends failed with why where its timing file could not be written", async () => {
    const end = vi.fn<(outcome: SpikeEnd) => Promise<void>>(() => Promise.resolve());
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    await recordInspectionCheck(
      { end },
      () =>
        Promise.resolve({
          label: "a",
          frame: null,
          exposure: null,
          statements: [],
          ground: [],
          closeGround: null,
        }),
      new AbortController().signal,
      {
        windows: [WINDOW],
        write: () => Promise.reject(new Error("the main process wrote no timing file")),
      },
    );

    expect(end.mock.calls).toEqual([
      [{ status: "fail", reason: "the main process wrote no timing file" }],
    ]);
  });

  it("writes no timing file where it measured no window", async () => {
    const write = vi.fn<() => Promise<void>>(() => Promise.resolve());
    const end = vi.fn<() => Promise<void>>(() => Promise.resolve());
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    await recordInspectionCheck(
      { end },
      () => Promise.reject(new Error("views check: VIEW BODY was held back")),
      new AbortController().signal,
      { windows: [], write },
    );

    expect([write.mock.calls.length, end.mock.calls.length]).toEqual([0, 1]);
  });

  it("ends failed with the reason the run threw", async () => {
    const end = vi.fn<() => Promise<void>>(() => Promise.resolve());
    vi.spyOn(console, "error").mockImplementation(() => undefined);

    await recordInspectionCheck(
      { end },
      () => Promise.reject(new Error("views check: VIEW BODY was held back")),
      new AbortController().signal,
      null,
    );

    expect(end.mock.calls).toEqual([
      [{ status: "fail", reason: "views check: VIEW BODY was held back" }],
    ]);
  });
});

/** A draw of the material `material`, its other fields empty. */
function drawOf(material: string): DrawItem {
  return {
    mesh: { kind: "mesh", name: "bodies:quad" },
    material: { kind: "material", name: material },
    offsetFromCameraM: new Float32Array(3),
    uniforms: {},
    textures: {},
  };
}

/** A submission labelled `label` of draws of `materials`. */
function submissionOf(label: string, materials: ReadonlyArray<string>): FrameSubmission {
  return {
    label,
    viewRotation: new Float32Array(16),
    projection: new Float32Array(16),
    draws: materials.map(drawOf),
    postProcesses: [],
  };
}

/** A plate whose `STARS` line reads `stars`, and no other line. */
function plateWith(stars: string | null): { readonly line: (label: string) => string | null } {
  return { line: (label) => (label === "STARS" ? stars : null) };
}

describe("the inspection run's lighting and its bodies drawn (fix-retina-frame)", () => {
  it("reads the lighting as pending, not received, or lit from the statements", () => {
    expect(
      [
        [LIGHTING_PENDING, "ATMOSPHERE: APPROXIMATE"],
        ["TERRAIN: STREAMING", LIGHTING_NOT_RECEIVED],
        ["ATMOSPHERE: APPROXIMATE"],
      ].map(inspectionLighting),
    ).toEqual(["pending", "not received", "lit"]);
  });

  it("reads the stars as pending while the STARS line reads PENDING alone (plans R06, R07)", () => {
    expect(
      [plateWith("PENDING"), plateWith("V 10.1 mag S CAM"), plateWith(null)].map(starsPending),
    ).toEqual([true, false, false]);
  });

  it("counts a body's disc, its figure's mesh or its terrain in the bodies and discs passes", () => {
    expect(
      [
        submissionOf("bodies", ["bodies:smoothMeshAir"]),
        submissionOf("bodies", ["terrain-lit-baked-offsets"]),
        submissionOf("discs", ["sky:hostDisc", "bodies:discLimb"]),
      ].map(drawsLitBody),
    ).toEqual([true, true, true]);
  });

  it("counts the terrain's clusters in the bodies pass's late raster (R14.T3.d)", () => {
    expect(drawsLitBody(submissionOf("bodies late", ["terrain-lit-cluster-baked-offsets"]))).toBe(
      true,
    );
  });

  it("counts no host disc alone, and no body material in another pass", () => {
    expect(
      [
        submissionOf("discs", ["sky:hostDisc"]),
        submissionOf("discs", []),
        submissionOf("sky", ["bodies:disc"]),
      ].map(drawsLitBody),
    ).toEqual([false, false, false]);
  });

  it("sees the primary's lit bodies since a time, and not an instrument's or outside a frame", () => {
    const bodies = new LitBodyDraws();
    bodies.submitted("instrument-1:hdr", submissionOf("bodies", ["bodies:smoothMesh"]), 50);
    bodies.submitted("view:hdr", submissionOf("bodies", ["bodies:smoothMesh"]), null);
    const before = [bodies.drewSince(0)];
    bodies.submitted("view:hdr", submissionOf("discs", ["bodies:disc"]), 20);
    bodies.submitted("view:hdr", submissionOf("discs", ["sky:hostDisc"]), 40);
    expect([...before, bodies.drewSince(20), bodies.drewSince(21)]).toEqual([false, true, false]);
  });
});

describe("the inspection run's reading of the ground and the camera's range", () => {
  it("reads the ground as surveyed by its ELEVATION, not by NOT SURVEYED", () => {
    expect(
      [
        ["SURVEY COVERAGE 100 % · FINEST 50 m", "GROUND AT LINE OF SIGHT", "ELEVATION 1204 ± 40 m"],
        ["SURVEY COVERAGE: PENDING", "GROUND AT LINE OF SIGHT", "NOT SURVEYED"],
        ["TERRAIN: NOT YET MODELLED"],
        [],
      ].map(inspectionGround),
    ).toEqual(["surveyed", "waiting", "not modelled", "waiting"]);
  });

  it("reads the free camera's rate from the CAMERA line, or under MAXIMIZE VIEW from the camera panel (lane perf-render-thread-ground)", () => {
    const panel = new DOMParser().parseFromString(
      `<output aria-label="Free camera rate">${freeRateReading(7)}</output>`,
      "text/html",
    );
    const none = new DOMParser().parseFromString("<p></p>", "text/html");
    expect([
      cameraRateStep(panel, `FREE · ${freeRateReading(3)}`),
      cameraRateStep(panel, null),
      cameraRateStep(none, null),
      cameraRateStep(panel, "SEAT"),
    ]).toEqual([3, 7, null, null]);
  });

  it("reads the camera's range from the POSITION line, or under MAXIMIZE VIEW from the camera panel (perf-frame-encoding)", () => {
    const panel = new DOMParser().parseFromString(
      '<output aria-label="Camera position"><span>6.42 Mm</span> <span>268° +49°</span></output>',
      "text/html",
    );
    const none = new DOMParser().parseFromString("<p></p>", "text/html");
    expect([
      cameraRangeM(panel, "6.76 Mm 268° +49°"),
      cameraRangeM(panel, null),
      cameraRangeM(none, null),
    ]).toEqual([6.76e6, 6.42e6, null]);
  });

  it("tells a readout the room holds back from the ground it reads", () => {
    expect([
      readoutHeldBack([lowerYieldLine(["readout"])]),
      readoutHeldBack([lowerYieldLine(["survey", "datum", "readout"])]),
      readoutHeldBack([lowerYieldLine(["survey"])]),
      readoutHeldBack(["GROUND AT LINE OF SIGHT", "ELEVATION ~617.9 m ± 0.4 m"]),
    ]).toEqual([true, true, false, false]);
  });

  it("reads the POSITION line's range in its unit, its digits grouped", () => {
    expect(
      ["6.69 Mm 023° -12°", "1,000 km 001° +00°", "87.0 km 359° +89°", "—", null].map(
        positionRangeM,
      ),
    ).toEqual([6.69e6, 1e6, 87_000, null, null]);
  });

  it("reads the camera's height above the ground from the slant range, resolved or grazing", () => {
    expect(
      [
        ["ELEVATION 1,204 ± 40 m", "SLANT RANGE FROM CAMERA 4.12 km ± 0.03 km"],
        ["SLANT RANGE FROM CAMERA ~4.0 km GRAZING"],
        ["SLANT RANGE 1,250 m"],
        ["SLANT RANGE FROM CAMERA —"],
        ["NOT SURVEYED"],
      ].map(slantRangeM),
    ).toEqual([4_120, 4_000, 1_250, null, null]);
  });

  it("times its low window at TERRAIN TEST's lowest, where the cascades draw (main's ask)", () => {
    expect(LOW_HEIGHT_M).toBe(TERRAIN_LOWEST_ALTITUDE_M);
  });

  it("halves a hold's share of what is left after a hold that flew most of it or past it", () => {
    expect([
      nextHoldShare(1, 320_000, 200_000),
      nextHoldShare(1, 320_000, 60_000),
      nextHoldShare(1, 320_000, -150_000),
      nextHoldShare(0.5, -60_000, -2_000),
      nextHoldShare(0.5, 100_000, 99_000),
      nextHoldShare(1 / 64, 1_000, -1_000),
    ]).toEqual([1, 0.5, 0.5, 0.25, 0.5, 1 / 64]);
  });

  it("reads the low window's height from the slant range, and backs out of the ground by doubles", () => {
    expect([
      lowRemainingM(5_000, 0),
      lowRemainingM(0, 1),
      lowRemainingM(0.5, 3),
      lowApproachRangeM(357_000, 6_760_000),
    ]).toEqual([
      5_000 - LOW_HEIGHT_M,
      -2 * LOW_HEIGHT_M,
      -8 * LOW_HEIGHT_M,
      6_760_000 - 357_000 + LOW_APPROACH_M,
    ]);
  });

  it("reads the slant range once the camera rests and the readout repeats it (perf-frame-encoding)", async () => {
    // The readout answers a hold late: stale values while the camera coasts, then its rest's.
    const readings = [12_000, null, 9_000, 8_000, 7_100, 7_000, 7_000, 7_000, 6_000];
    let drawn = 0;
    const slantM = await restedSlantM({
      framesDrawn: (count) => {
        drawn += count;
        return Promise.resolve();
      },
      slantM: () => readings[Math.min(drawn, readings.length - 1)] ?? null,
    });
    expect([slantM, drawn]).toEqual([7_000, 7]);
  });

  it("takes the last reading where the readout never repeats within its draws", async () => {
    let drawn = 0;
    const slantM = await restedSlantM({
      framesDrawn: (count) => {
        drawn += count;
        return Promise.resolve();
      },
      slantM: () => 10_000 - drawn,
    });
    expect([slantM, drawn]).toEqual([
      10_000 - LOW_REST_DRAWS - LOW_FRESH_MAX_DRAWS,
      LOW_REST_DRAWS + LOW_FRESH_MAX_DRAWS,
    ]);
  });

  it("reaches the low window from the close one in its holds, never into the ground", () => {
    // A model of the step on SwiftShader: a hold flies a camera step (`holdForStep`), a frame's
    // more on a fast renderer, the POSITION range reads to 10 km, and the slant range is read at
    // rest, so current.
    const reached = [[1], [1.07]].flatMap((frames) =>
      [6_403_456, 6_398_000].map((groundM) => {
        let heightM = 330_000;
        let lowest = heightM;
        let hold = 0;
        const fly = (remainingM: number, share: number): void => {
          const stepM =
            freeRateMPerS(closeRateStep(share * Math.abs(remainingM))) * MAX_FREE_STEP_S;
          heightM -= Math.sign(remainingM) * stepM * (frames[hold % frames.length] ?? 1);
          lowest = Math.min(lowest, heightM);
          hold += 1;
        };
        const goalM = lowApproachRangeM(
          Math.round(heightM / 1_000) * 1_000,
          shownRangeM(groundM + heightM),
        );
        // The approach by the POSITION range.
        let share = LOW_HOLD_SHARE;
        let before: number | null = null;
        for (let left = INSPECTION_CHECK_TIMING.closeHolds; left > 0; left -= 1) {
          const remainingM = shownRangeM(groundM + heightM) - goalM;
          if (Math.abs(remainingM) <= LOW_APPROACH_TOLERANCE_M) {
            break;
          }
          share = before === null ? share : nextHoldShare(share, before, remainingM);
          before = remainingM;
          fly(remainingM, share);
        }
        const approached = lowest > 0;
        // The last of it by the slant range at rest, backing out of the ground by doubles.
        share = LOW_HOLD_SHARE;
        before = null;
        let groundedReadings = 0;
        for (let left = INSPECTION_CHECK_TIMING.closeHolds; left > 0; left -= 1) {
          groundedReadings = heightM > GROUNDED_SLANT_M ? 0 : groundedReadings + 1;
          const remainingM = lowRemainingM(Math.max(heightM, 0), groundedReadings);
          if (Math.abs(remainingM) <= CLOSE_TOLERANCE * LOW_HEIGHT_M) {
            return approached;
          }
          share = before === null ? share : nextHoldShare(share, before, remainingM);
          before = remainingM;
          fly(remainingM, share);
        }
        return false;
      }),
    );
    expect(reached.every(Boolean)).toBe(true);
  });

  it("closes in by holds whose one camera step flies at most half of what is left", () => {
    const steps = [3e3, 1e5, 1.55e7].map((remainingM) => {
      const flownM = freeRateMPerS(closeRateStep(remainingM)) * MAX_FREE_STEP_S;
      return flownM <= remainingM / 2 && flownM > remainingM / 2 / Math.sqrt(10);
    });
    expect(steps).toEqual([true, true, true]);
  });
});

/** A range as the `POSITION` line shows one near a body of 6,400 km: to three figures, 10 km. */
function shownRangeM(rangeM: number): number {
  return Math.round(rangeM / 10_000) * 10_000;
}

/** A views probe whose submissions a test makes, its primary last drawn at `lastMs`. */
function submittingViews(lastMs: number): {
  readonly views: Parameters<typeof holdForStep>[0]["views"];
  readonly submit: (owner: string, frameMs: number) => void;
} {
  const watchers = new Set<SubmissionWatcher>();
  return {
    views: {
      drawTimes: () => [lastMs],
      watch: (watcher) => {
        watchers.add(watcher);
        return () => {
          watchers.delete(watcher);
        };
      },
    },
    submit: (owner, frameMs) => {
      for (const watcher of watchers) {
        watcher.submitted(owner, submissionOf("view", []), frameMs);
      }
    },
  };
}

/** The keys `target` heard, as `down w` and `up w`. */
function heard(target: EventTarget): string[] {
  const keys: string[] = [];
  target.addEventListener("keydown", (event) => {
    keys.push(`down ${event instanceof KeyboardEvent ? event.key : "?"}`);
  });
  target.addEventListener("keyup", (event) => {
    keys.push(`up ${event instanceof KeyboardEvent ? event.key : "?"}`);
  });
  return keys;
}

/** A reckoner over rested readings in turn, and how many it took. */
function reckoner(readings: ReadonlyArray<number>): {
  readonly slant: ReckonedSlant;
  readonly reads: () => number;
} {
  let reads = 0;
  const slant = new ReckonedSlant(() => {
    const reading = readings[Math.min(reads, readings.length - 1)] ?? 0;
    reads += 1;
    return Promise.resolve(reading);
  });
  return { slant, reads: () => reads };
}

describe("the low window's streaming wait (perf-frame-encoding)", () => {
  it("tells a wait that ended from one that gave up, and lets a stop through", async () => {
    const gaveUp = Promise.reject(
      new Error("views check: the terrain did not stream within 1500 s"),
    );
    const stopped = Promise.reject(new Error(STOPPED));
    expect([await awaitedOrNot(Promise.resolve()), await awaitedOrNot(gaveUp)]).toEqual([
      true,
      false,
    ]);
    await expect(awaitedOrNot(stopped)).rejects.toThrow(STOPPED);
  });
});

describe("the low window's last stretch by dead reckoning (perf-frame-encoding)", () => {
  it("reckons what is left from a rested reading and the holds flown, reading again only near the goal", async () => {
    const tolerance = CLOSE_TOLERANCE * LOW_HEIGHT_M;
    const { slant, reads } = reckoner([33_000, 4_300]);
    const first = await slant.remainingM(tolerance);
    slant.flew(7_900);
    const second = await slant.remainingM(tolerance);
    slant.flew(20_500);
    const near = await slant.remainingM(tolerance);
    expect([first, second, near, reads()]).toEqual([29_000, 21_100, 300, 2]);
  });

  it("reads again after each hold out of the ground, which it cannot reckon from", async () => {
    const tolerance = CLOSE_TOLERANCE * LOW_HEIGHT_M;
    const { slant, reads } = reckoner([0, 0, 6_000]);
    const first = await slant.remainingM(tolerance);
    slant.flew(-5_000);
    const second = await slant.remainingM(tolerance);
    slant.flew(-5_000);
    const out = await slant.remainingM(tolerance);
    expect([first, second, out, reads()]).toEqual([-2 * LOW_HEIGHT_M, -4 * LOW_HEIGHT_M, 2_000, 3]);
  });
});

describe("the inspection run's held steps (perf-frame-encoding)", () => {
  it("releases a key in the primary's first submission on a renderer of slow frames", () => {
    const target = new EventTarget();
    const keys = heard(target);
    const { views, submit } = submittingViews(1_000);
    const hold = holdForStep({ views }, target, "w");
    submit("view:bloom down 1", 60_000);
    const beforeOwn = [hold.released(), [...keys]];
    submit("view", 60_000);
    expect([beforeOwn, hold.released(), keys, hold.integratedS()]).toEqual([
      [false, ["down w"]],
      true,
      ["down w", "up w"],
      MAX_FREE_STEP_S,
    ]);
  });

  it("holds a key over a fast renderer's frames until they integrate one step's time", () => {
    const target = new EventTarget();
    const keys = heard(target);
    const { views, submit } = submittingViews(1_000);
    const hold = holdForStep({ views }, target, "s");
    const frameMs = 1_000 / 60;
    const releasedAt = Array.from({ length: 20 }, (_, i) => {
      submit("view", 1_000 + (i + 1) * frameMs);
      return hold.released();
    }).indexOf(true);
    hold.release();
    expect([releasedAt + 1, keys]).toEqual([15, ["down s", "up s"]]);
    expect(hold.integratedS()).toBeCloseTo(MAX_FREE_STEP_S, 9);
  });
});
