/**
 * `VIEW` under a perf run's hooks (plan R12, R12.T4.c; `viewPerf.ts`): the primary's free
 * scripted camera over the server's scene, and each frame it drew, told to the run.
 */
import { screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { InThreadSkyWorker } from "../../test/skyFixtures";
import {
  nominalStore,
  openUniverse,
  renderViewDisplay,
  sceneArrives,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import { MAXIMIZE_KEY } from "./displayKeys";
import type { ScriptedCamera, ViewPerfFrame, ViewPerfHooks } from "./viewPerf";
import type { ViewRun } from "./viewRun";

afterEach(() => {
  vi.useRealTimers();
});

/** A script's position from the barycentre, m: far from every body of the slice's system. */
const SCRIPTED_M = vec3(0, 0, 7e11);

/** The hooks, recording each run they are asked a camera for and each frame told. */
interface Recorded {
  readonly perf: ViewPerfHooks;
  readonly asked: ViewRun[];
  readonly frames: ViewPerfFrame[];
}

function recorded(): Recorded {
  const asked: ViewRun[] = [];
  const frames: ViewPerfFrame[] = [];
  return {
    asked,
    frames,
    perf: {
      cameraAt: (run): ScriptedCamera => {
        asked.push(run);
        return {
          pose: {
            frame: { kind: "system", system: run.scene.system },
            positionM: SCRIPTED_M,
            orientation: { w: 1, x: 0, y: 0, z: 0 },
          },
          style: "photorealistic",
          fovDeg: 60,
        };
      },
      onDrawn: (frame) => {
        frames.push(frame);
      },
    },
  };
}

async function setup(perf: ViewPerfHooks): Promise<ViewDisplayHarness> {
  vi.stubGlobal("Worker", InThreadSkyWorker);
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
    setting: "low",
    perf,
  });
  await settle();
  view.advance(100);
  return view;
}

/** `VIEW` under `perf`, a universe opened and the server's scene arrived and drawn. */
async function served(perf: ViewPerfHooks): Promise<ViewDisplayHarness> {
  const view = await setup(perf);
  await openUniverse(view);
  await sceneArrives(view);
  await settle();
  view.advance(300);
  await settle();
  view.advance(300);
  return view;
}

describe("VIEW under a perf run's scripted camera", () => {
  it("mounts with MAXIMIZE VIEW on, its label block whole, its primary at the most of the stage", async () => {
    await setup(recorded().perf);
    expect(
      screen
        .getByRole("button", { name: `${MAXIMIZE_KEY} MAXIMIZE VIEW` })
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(screen.getAllByText("QUALITY").length).toBeGreaterThan(0);
  });

  it("asks no scripted camera while a kept scene stands in for the server's", async () => {
    const { perf, asked, frames } = recorded();
    const view = await setup(perf);
    view.advance(300);
    expect([asked, frames]).toEqual([[], []]);
  });

  it("sets its primary at the script's pose over the server's scene", async () => {
    const { perf, asked } = recorded();
    const view = await served(perf);
    const reported = view.socket.requestsOfKind("scene_cameras").at(-1)?.body.cameras[0];
    expect(asked.length).toBeGreaterThan(0);
    expect(asked.every(({ source }) => source.kind === "server")).toBe(true);
    expect(reported?.pose.position).toEqual({
      frame: "system",
      system: asked.at(-1)?.scene.system,
      offset_m: [0, 0, 7e11],
    });
  });

  it("names its camera SCRIPTED, a script's and no preset's", async () => {
    await served(recorded().perf);
    expect(
      screen.getByRole("application", { name: /^VIEW, [A-Z]+, PRIMARY, SCRIPTED$/u }),
    ).toBeDefined();
  });

  it("tells each frame drawn at the script, with the primary's size and scale", async () => {
    const { perf, frames } = recorded();
    await served(perf);
    expect(frames.length).toBeGreaterThan(0);
    for (const { size, renderScale } of frames) {
      expect([size.widthPx > 0, size.heightPx > 0, renderScale > 0, renderScale <= 1]).toEqual([
        true,
        true,
        true,
        true,
      ]);
    }
  });
});
