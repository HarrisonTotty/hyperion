import { screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { AtmosphereLeversOverride } from "../../../../preload/api";
import { statusLine } from "../../test/statusLine";
import {
  nominalStore,
  renderViewDisplay,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import { IMAGE_DETAIL_LIMITED } from "../../view/budget/viewBudget";
import { type PhotorealFrame, PhotorealRenderer } from "../../view/photoreal/renderer";

afterEach(() => {
  vi.useRealTimers();
});

/** The primary's stage, CSS px and device px: 900 rows, above the low setting's 720. */
const STAGE_PX = { widthPx: 1600, heightPx: 900 } as const;

interface Setup extends ViewDisplayHarness {
  readonly fake: ReturnType<typeof timedEngineSource>;
}

/**
 * `VIEW` on the high setting drawing `PHASE TEST` photorealistic, its adapter with `features`
 * (`timestamp-query` gives it a pass timer; without it the floor reads the frames' intervals).
 */
async function photoreal(
  features: ReadonlyArray<GPUFeatureName> = [],
  atmosphereLevers?: AtmosphereLeversOverride,
): Promise<Setup> {
  const fake = timedEngineSource();
  const harness = renderViewDisplay({
    store: await nominalStore(features),
    source: fake.source,
    engines: fake.engines,
    setting: "high",
    stagePx: STAGE_PX,
    ...(atmosphereLevers === undefined ? {} : { atmosphereLevers }),
  });
  await settle();
  harness.advance(100);
  await harness.user.click(screen.getByRole("button", { name: "PHASE TEST" }));
  harness.advance(100);
  await harness.user.keyboard("4");
  harness.advance(100);
  await settle();
  harness.advance(300);
  return { ...harness, fake };
}

/** Draws `frames` of the primary `intervalMs` apart, each pass of every resolve timed at `passMs`. */
function draw(view: Setup, frames: number, intervalMs: number, passMs: number): void {
  for (let frame = 0; frame < frames; frame += 1) {
    view.fake.deliver(() => passMs);
    view.advance(intervalMs);
  }
}

/** The size the primary's scene target was last given. */
function sceneTarget(view: Setup): { readonly widthPx: number; readonly heightPx: number } | null {
  return (
    view.fake.targets.findLast((target) => target.spec.name === "view:hdr")?.sizes.at(-1) ?? null
  );
}

describe("VIEW's frame-rate floor (the owner's report, 2026-11-01)", () => {
  it("keeps a photorealistic primary alone at its setting's resolution while its frames keep their rate", async () => {
    const view = await photoreal(["timestamp-query"]);
    draw(view, 200, 17, 0.2);
    expect(sceneTarget(view)).toEqual(STAGE_PX);
    expect(screen.queryByText(statusLine(IMAGE_DETAIL_LIMITED))).toBeNull();
  });

  it("steps its image down to the low setting's 720 rows once its frames run long, and says so", async () => {
    const render = vi.spyOn(PhotorealRenderer.prototype, "render");
    const view = await photoreal(["timestamp-query"]);
    const lastAir = (): PhotorealFrame["atmosphereSetting"] =>
      render.mock.calls.at(-1)?.[1].atmosphereSetting;
    draw(view, 50, 17, 0.2);
    expect(lastAir()).toBe("high");
    draw(view, 200, 17, 10);
    expect(sceneTarget(view)).toEqual({ widthPx: 1280, heightPx: 720 });
    // Its air drawn at the low setting's sizes as its first step, the march at half resolution.
    expect(lastAir()).toBe("low");
    expect(screen.getByText(statusLine(IMAGE_DETAIL_LIMITED))).toBeTruthy();
  });

  it("draws its large discs' limbs at the floor's fewer cells only once its image stands at the floor's lowest step (R07.T8.i)", async () => {
    const render = vi.spyOn(PhotorealRenderer.prototype, "render");
    const view = await photoreal(["timestamp-query"]);
    const lastLowest = (): PhotorealFrame["floorLowest"] =>
      render.mock.calls.at(-1)?.[1].floorLowest;
    draw(view, 50, 17, 0.2);
    const keeping = lastLowest();
    draw(view, 200, 17, 10);
    expect([keeping, sceneTarget(view), lastLowest()]).toEqual([
      false,
      { widthPx: 1280, heightPx: 720 },
      true,
    ]);
  });
});

describe("VIEW's atmosphere levers over its setting's (plan R08, R08.T8.d)", () => {
  it("hands a frame profile's levers to every photorealistic frame", async () => {
    const render = vi.spyOn(PhotorealRenderer.prototype, "render");
    const levers = { marchCover: "tiles", limbRays: "packed" } as const;
    const view = await photoreal(["timestamp-query"], levers);
    draw(view, 5, 17, 0.2);
    const given = render.mock.calls.map(([, frame]) => frame.atmosphereLevers);
    expect([given.length > 0, given.every((each) => each === levers)]).toEqual([true, true]);
  });

  it("hands none where the launch gives none, the setting's own", async () => {
    const render = vi.spyOn(PhotorealRenderer.prototype, "render");
    const view = await photoreal(["timestamp-query"]);
    draw(view, 5, 17, 0.2);
    const given = render.mock.calls.map(([, frame]) => frame.atmosphereLevers);
    expect([given.length > 0, given.every((each) => each === undefined)]).toEqual([true, true]);
  });
});
