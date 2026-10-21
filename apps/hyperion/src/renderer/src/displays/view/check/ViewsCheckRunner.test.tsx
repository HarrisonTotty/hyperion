import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ViewsCheckApi } from "../../../../../preload/api";
import { HANDOVER_APPROACH } from "./handoverGeometry";
import { HandoverScenes, terrainTestHandoverWorld } from "./handoverScenes";
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
    launch: { setting: "high", smoke: true, out: null, run: "several-views" },
    startPhase: () => Promise.resolve(),
    endPhase: () => Promise.resolve(),
    askRightWayUp: () => Promise.resolve(),
    writeTerrainResults: () => Promise.resolve({ json: "a", markdown: "b" }),
    writeHandoverResults: () => Promise.resolve({ json: "a", markdown: "b" }),
    capture: () => Promise.resolve(),
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
      <ViewsCheckRunner api={api} probe={new ViewsProbe()} run={{ kind: "several-views" }} />,
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
      <ViewsCheckRunner api={api} probe={views} run={{ kind: "handover", terrain, scenes }} />,
    );

    // jsdom draws no VIEW, so the check's first wait runs out.
    await vi.advanceTimersByTimeAsync(61_000);

    expect(failed.mock.calls[0]?.[0]).toBe("views check: the hand-over check failed:");
  });
});
