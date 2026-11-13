import { RequestClient } from "@hyperion/protocol";
import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ViewsCheckApi } from "../../../../../preload/api";
import { HANDOVER_APPROACH } from "./handoverGeometry";
import { HandoverScenes, terrainTestHandoverWorld } from "./handoverScenes";
import { InspectionClusters } from "./inspectionClusters";
import { TerrainProbe } from "./terrainProbe";
import { ViewsCheckRunner } from "./ViewsCheckRunner";
import { ViewsProbe } from "./viewsProbe";

afterEach(() => {
  vi.useRealTimers();
});

/** A check launch's functions, the ending's two mocks. */
function checkApi(): ViewsCheckApi & {
  readonly end: ReturnType<typeof vi.fn<ViewsCheckApi["end"]>>;
  readonly writeResults: ReturnType<typeof vi.fn<ViewsCheckApi["writeResults"]>>;
} {
  return {
    launch: {
      setting: "high",
      smoke: true,
      out: null,
      run: "several-views",
      fixture: "terrain-test",
    },
    startPhase: () => Promise.resolve(),
    endPhase: () => Promise.resolve(),
    askRightWayUp: () => Promise.resolve(),
    writeTerrainResults: () => Promise.resolve({ json: "a", markdown: "b" }),
    writeTerrainPhase: () => Promise.resolve({ json: "a", markdown: "b" }),
    writeHandoverResults: () => Promise.resolve({ json: "a", markdown: "b" }),
    writeAtmosphereResults: () => Promise.resolve({ json: "a", markdown: "b" }),
    startProfile: () => Promise.resolve(),
    endProfile: () => Promise.resolve(),
    writeTimingResults: () => Promise.resolve({ json: "a", markdown: "b" }),
    capture: () => Promise.resolve(),
    wholeField: () => Promise.reject(new Error("no whole-body field")),
    writeResults: vi.fn<ViewsCheckApi["writeResults"]>(() =>
      Promise.resolve({ json: "a", markdown: "b" }),
    ),
    end: vi.fn<ViewsCheckApi["end"]>(() => Promise.resolve()),
  };
}

describe("ViewsCheckRunner", () => {
  it("gives the page its own animation frames back on unmount", () => {
    const own = window.requestAnimationFrame;
    const { unmount } = render(
      <ViewsCheckRunner
        api={checkApi()}
        probe={new ViewsProbe()}
        run={{ kind: "several-views" }}
        launchMode="vulkan"
        openSystem={() => undefined}
      />,
    );
    expect(window.requestAnimationFrame).not.toBe(own);

    unmount();

    expect(window.requestAnimationFrame).toBe(own);
  });

  it("ends nothing once unmounted", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    const api = checkApi();
    const { unmount } = render(
      <ViewsCheckRunner
        api={api}
        probe={new ViewsProbe()}
        run={{ kind: "several-views" }}
        launchMode="vulkan"
        openSystem={() => undefined}
      />,
    );

    unmount();
    // Past the script's first wait, which a run still going would have failed.
    await vi.advanceTimersByTimeAsync(61_000);

    expect(api.end).not.toHaveBeenCalled();
    expect(api.writeResults).not.toHaveBeenCalled();
  });

  it("runs the hand-over check on a hand-over launch, in the terrain run's place (R10.T10.g)", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    const failed = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const api = { ...checkApi(), launch: { ...checkApi().launch, run: "handover" as const } };
    const views = new ViewsProbe();
    const scenes = new HandoverScenes(terrainTestHandoverWorld(), HANDOVER_APPROACH.smoke, 1.5e7);
    const terrain = new TerrainProbe(views, {
      paceMs: 0,
      readBackMs: 1_000,
      checkScenes: scenes.sceneOptions,
    });
    render(
      <ViewsCheckRunner
        api={api}
        probe={views}
        run={{ kind: "handover", terrain, scenes }}
        launchMode="vulkan"
        openSystem={() => undefined}
      />,
    );

    // jsdom draws no VIEW, so the check's first wait runs out.
    await vi.advanceTimersByTimeAsync(61_000);

    expect(failed.mock.calls[0]?.[0]).toBe("views check: the hand-over check failed:");
  });

  it("runs the atmosphere run on an atmosphere launch (R08.T10.b)", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    const failed = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const api = { ...checkApi(), launch: { ...checkApi().launch, run: "atmosphere" as const } };
    render(
      <ViewsCheckRunner
        api={api}
        probe={new ViewsProbe()}
        run={{ kind: "atmosphere", placed: null }}
        launchMode="vulkan"
        openSystem={() => undefined}
      />,
    );

    // jsdom draws no VIEW, so the run's first wait runs out.
    await vi.advanceTimersByTimeAsync(61_000);

    expect([failed.mock.calls[0]?.[0], api.end.mock.calls]).toEqual([
      "views check: the atmosphere run failed:",
      [[{ status: "fail", reason: "views check: VIEW did not show its primary view within 60 s" }]],
    ]);
  });

  it("runs the inspection run on an inspection launch (the owner's request)", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const api = { ...checkApi(), launch: { ...checkApi().launch, run: "inspection" as const } };
    const requests = new RequestClient(() => false);
    render(
      <ViewsCheckRunner
        api={api}
        probe={new ViewsProbe()}
        run={{
          kind: "inspection",
          requests,
          served: null,
          calls: null,
          clusters: new InspectionClusters(),
        }}
        launchMode="vulkan"
        openSystem={() => undefined}
      />,
    );

    await vi.waitFor(() => {
      expect(api.end).toHaveBeenCalledWith({
        status: "fail",
        reason: "views check: the inspection launch names no universe and body",
      });
    });
  });

  it("waits five times as long in the check-only software mode (R01.T16)", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const api = { ...checkApi(), launch: { ...checkApi().launch, run: "atmosphere" as const } };
    render(
      <ViewsCheckRunner
        api={api}
        probe={new ViewsProbe()}
        run={{ kind: "atmosphere", placed: null }}
        launchMode="software"
        openSystem={() => undefined}
      />,
    );

    // Past the hardware's 60 s wait, the software mode's still waits.
    await vi.advanceTimersByTimeAsync(61_000);
    const early = api.end.mock.calls.length;
    await vi.advanceTimersByTimeAsync(240_000);

    expect([early, api.end.mock.calls]).toEqual([
      0,
      [
        [
          {
            status: "fail",
            reason: "views check: VIEW did not show its primary view within 300 s",
          },
        ],
      ],
    ]);
  });
});
