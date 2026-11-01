import { screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { statusLine } from "../../test/statusLine";
import {
  nominalStore,
  renderViewDisplay,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import { IMAGE_DETAIL_LIMITED } from "../../view/budget/viewBudget";

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
async function photoreal(features: ReadonlyArray<GPUFeatureName> = []): Promise<Setup> {
  const fake = timedEngineSource();
  const harness = renderViewDisplay({
    store: await nominalStore(features),
    source: fake.source,
    engines: fake.engines,
    setting: "high",
    stagePx: STAGE_PX,
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
    const view = await photoreal(["timestamp-query"]);
    draw(view, 200, 17, 10);
    expect(sceneTarget(view)).toEqual({ widthPx: 1280, heightPx: 720 });
    expect(screen.getByText(statusLine(IMAGE_DETAIL_LIMITED))).toBeTruthy();
  });
});
