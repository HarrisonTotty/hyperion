import { RequestClient } from "@hyperion/protocol";
import { describe, expect, it, vi } from "vitest";

import type { InspectionTimingWindow, SpikeEnd } from "../../../../../preload/api";
import { freeRateMPerS, MAX_FREE_STEP_S } from "../../../view/camera/freeCamera";
import type { DrawItem, FrameSubmission } from "../../../view/engine/types";
import { LIGHTING_NOT_RECEIVED, LIGHTING_PENDING } from "../../../view/lighting/hostLights";
import { TERRAIN_LOWEST_ALTITUDE_M } from "../../../view/scenes/terrainScene";
import {
  closeRateStep,
  drawsLitBody,
  INSPECTION_CHECK_TIMING,
  inspectionCaptureLabel,
  inspectionGround,
  inspectionLighting,
  LitBodyDraws,
  LOW_HEIGHT_M,
  positionRangeM,
  readoutHeldBack,
  recordInspectionCheck,
  runInspectionCheck,
  slantRangeM,
} from "./inspectionCheckRun";
import { lowerYieldLine } from "../viewRoom";

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
  marchSamples: 36,
  position: null,
  statements: [],
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

  it("names its maximized and GRID OFF captures as a capture may be named (2026-11-01)", () => {
    const label = inspectionCaptureLabel(BODY);
    expect(
      [`${label}-maximized`, `${label}-grid-off`].map((each) => CAPTURE_LABEL.test(each)),
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

  it("counts a body's disc, its figure's mesh or its terrain in the bodies and discs passes", () => {
    expect(
      [
        submissionOf("bodies", ["bodies:smoothMeshAir"]),
        submissionOf("bodies", ["terrain-lit-baked-offsets"]),
        submissionOf("discs", ["sky:hostDisc", "bodies:discLimb"]),
      ].map(drawsLitBody),
    ).toEqual([true, true, true]);
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

  it("closes in by holds whose one camera step flies at most half of what is left", () => {
    const steps = [3e3, 1e5, 1.55e7].map((remainingM) => {
      const flownM = freeRateMPerS(closeRateStep(remainingM)) * MAX_FREE_STEP_S;
      return flownM <= remainingM / 2 && flownM > remainingM / 2 / Math.sqrt(10);
    });
    expect(steps).toEqual([true, true, true]);
  });
});
