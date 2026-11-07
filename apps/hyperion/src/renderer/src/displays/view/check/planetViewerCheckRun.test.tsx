import { RequestClient } from "@hyperion/protocol";
import { describe, expect, it, vi } from "vitest";

import { FIXTURE_MOON } from "../../../test/inspectionFixture";
import { populatedBodies, sliceBodies } from "../../../test/planetaryFixture";
import type { FrameSubmission } from "../../../view/engine/types";
import {
  consoleOverflowPx,
  PLANET_VIEWER_CHECK_TIMING,
  RANDOMIZE_BALANCES,
  randomizeCaptureLabel,
  runPlanetViewerCheck,
  PrimaryDraws,
  elevationM,
  firstMoon,
  goalAltitudeM,
  inputsCutPx,
  recordPlanetViewerCheck,
  skyAnswered,
  SURFACE_HEIGHT_M,
  zoomStepsTo,
} from "./planetViewerCheckRun";

describe("the planet viewer run's readings (plan R15, T6)", () => {
  it("reads the ground's elevation in m or km, estimated or not, below the datum too", () => {
    expect([
      elevationM(["SLANT RANGE FROM CAMERA 4.12 km ± 0.03 km", "ELEVATION ~2,140 m ± 180 m"]),
      elevationM(["ELEVATION -1.25 km ± 0.02 km"]),
      elevationM(["NOT SURVEYED"]),
    ]).toEqual([2140, -1250, null]);
  });

  it("works out the wheel's steps over the datum, where ORBIT zooms", () => {
    // 1,000 km above ground 2 km up comes to 300 m above it by the altitudes' ratio, 1.25 a step.
    const goal = goalAltitudeM(2000, SURFACE_HEIGHT_M);
    const steps = zoomStepsTo(1e6 + 2000, goal);
    expect((1e6 + 2000) / 1.25 ** steps - 2000).toBeCloseTo(SURFACE_HEIGHT_M, 6);
  });

  it("aims above the datum over ground below it, where ORBIT holds the camera", () => {
    expect([goalAltitudeM(-3000, SURFACE_HEIGHT_M), goalAltitudeM(500, 300)]).toEqual([300, 800]);
  });
});

/** An element laid out from `top` to `bottom`, CSS px. */
function box(top: number, bottom: number): Pick<Element, "getBoundingClientRect"> {
  return {
    getBoundingClientRect: () =>
      DOMRect.fromRect({ x: 0, y: top, width: 416, height: bottom - top }),
  };
}

describe("the planet viewer run's moon (R15.T5's follow-up)", () => {
  it("takes the first moon present in a system's bodies, and none from a system with none", () => {
    expect([firstMoon(populatedBodies()), firstMoon(sliceBodies())]).toEqual([FIXTURE_MOON, null]);
  });
});

describe("the planet viewer run's wait for the server's warm-up", () => {
  it("reads the sky unanswered while its STARS line reads PENDING", () => {
    expect(skyAnswered("PENDING")).toBe(false);
  });

  it("reads it answered from its first reply on, or where the view asks no sky", () => {
    expect([skyAnswered("V 10.2 mag CAM · BEYOND 250 ly: STREAMING"), skyAnswered(null)]).toEqual([
      true,
      true,
    ]);
  });
});

describe("the planet viewer run's RANDOMIZE captures (the owner's report, 2026-11-05)", () => {
  it("are named for each draw and white balance, HOST first", () => {
    expect(
      [1, 8].flatMap((index) => RANDOMIZE_BALANCES.map((b) => randomizeCaptureLabel(index, b))),
    ).toEqual([
      "planet-viewer-draw-1-host",
      "planet-viewer-draw-1-d65",
      "planet-viewer-draw-8-host",
      "planet-viewer-draw-8-d65",
    ]);
  });
});

describe("the planet viewer run's fit check", () => {
  it("reads how far the console's frame overflows the window, none where it fits", () => {
    expect([
      consoleOverflowPx({ scrollHeight: 760, clientHeight: 720 }),
      consoleOverflowPx({ scrollHeight: 720, clientHeight: 720 }),
      consoleOverflowPx(null),
    ]).toEqual([40, 0, 0]);
  });

  it("reads how far the INPUTS panel's foot stands below its column's, none where it fits", () => {
    expect([
      inputsCutPx(box(67, 732), box(67, 666)),
      inputsCutPx(box(67, 624), box(67, 666)),
      inputsCutPx(null, box(67, 666)),
    ]).toEqual([66, 0, 0]);
  });
});

/** A pass `label` of one draw of `material`. */
function frame(label: string, material: string): FrameSubmission {
  return {
    label,
    viewRotation: new Float32Array(16),
    projection: new Float32Array(16),
    draws: [
      {
        mesh: { kind: "mesh", name: "bodies:quad" },
        material: { kind: "material", name: material },
        offsetFromCameraM: new Float32Array(3),
        uniforms: {},
        textures: {},
      },
    ],
    postProcesses: [],
  };
}

describe("the primary's draws a failure names", () => {
  it("are its last frame's passes and materials, an instrument's left out", () => {
    const draws = new PrimaryDraws();
    draws.submitted("view:hdr", frame("bodies", "bodies:smoothMesh"), 10);
    draws.submitted("view:hdr", frame("bodies late", "terrain-lit-cluster-x"), 20);
    draws.submitted("instrument-1:hdr", frame("bodies", "bodies:disc"), 20);
    expect([draws.named(), new PrimaryDraws().named()]).toEqual([
      "bodies late: terrain-lit-cluster-x",
      "none",
    ]);
  });
});

/** A request client whose server lists no universe. */
function noUniverses(): RequestClient {
  const client: RequestClient = new RequestClient((message) => {
    if (message.type === "request") {
      queueMicrotask(() => {
        client.handleServerMessage({
          type: "response",
          id: message.id,
          body: { kind: "list_universes", universes: [], server_generator_version: 28 },
        });
      });
    }
    return true;
  });
  return client;
}

describe("the planet viewer run", () => {
  it("fails, saying so, where the server lists no universe", async () => {
    const capture = vi.fn<() => Promise<void>>(() => Promise.resolve());
    await expect(
      runPlanetViewerCheck({
        api: { capture },
        views: { drawTimes: () => [], watch: () => () => undefined },
        requests: noUniverses(),
        document,
        nowMs: () => 0,
        sleep: () => Promise.resolve(),
        signal: new AbortController().signal,
        timing: PLANET_VIEWER_CHECK_TIMING,
      }),
    ).rejects.toThrow("views check: the server lists no universe");
    expect(capture).not.toHaveBeenCalled();
  });
});

describe("the planet viewer run's ending", () => {
  it("ends the check passed once its captures are taken", async () => {
    const end = vi.fn<(outcome: unknown) => Promise<void>>(() => Promise.resolve());
    await recordPlanetViewerCheck({ end }, () => Promise.resolve([]), new AbortController().signal);
    expect(end).toHaveBeenCalledWith({ status: "pass" });
  });

  it("ends nothing once the run was stopped", async () => {
    const end = vi.fn<(outcome: unknown) => Promise<void>>(() => Promise.resolve());
    const stop = new AbortController();
    stop.abort();
    await recordPlanetViewerCheck(
      { end },
      () => Promise.reject(new Error("views check: stopped")),
      stop.signal,
    );
    expect(end).not.toHaveBeenCalled();
  });

  it("ends it failed with the reason the run threw", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const end = vi.fn<(outcome: unknown) => Promise<void>>(() => Promise.resolve());
    await recordPlanetViewerCheck(
      { end },
      () => Promise.reject(new Error("views check: GENERATE was held back within 60 s")),
      new AbortController().signal,
    );
    expect(end).toHaveBeenCalledWith({
      status: "fail",
      reason: "views check: GENERATE was held back within 60 s",
    });
  });
});
