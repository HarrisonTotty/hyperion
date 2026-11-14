import { act, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { FakeStillDevice, FakeStillSource } from "../../test/fakeStillJob";
import { statusLine } from "../../test/statusLine";
import { TEST_STILL_REQUEST } from "../../test/stillFixtures";
import {
  COMPACT_VIEW_PX,
  renderViewDisplay,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import { GraphicsStatusStore, initialGraphicsStatus } from "../../view/engine/status";
import type { StillSource } from "../../view/stills/stillJob";

afterEach(() => {
  vi.useRealTimers();
});

/** `VIEW` laid out as `viewPx`, the full layout by default, with a still source or none. */
async function setup(
  stillSource: StillSource | undefined,
  viewPx?: typeof COMPACT_VIEW_PX,
): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
    source: fake.source,
    engines: fake.engines,
    ...(stillSource === undefined ? {} : { stillSource }),
    ...(viewPx === undefined ? {} : { viewPx }),
  });
  await settle();
  view.advance(100);
  return view;
}

/** A still source over the fakes, its device the test's. */
function source(device = new FakeStillDevice("absent")): FakeStillSource {
  return new FakeStillSource(device, {
    ...TEST_STILL_REQUEST,
    size: { widthPx: 100, heightPx: 70 },
  });
}

/** VIEW's own disclosure buttons on show, by name, in their order. */
function disclosures(): ReadonlyArray<string | null> {
  const display = document.querySelector<HTMLElement>(".view");
  if (display === null) {
    throw new Error("VIEW is not laid out");
  }
  return within(display)
    .getAllByRole("button")
    .filter((button) => button.hasAttribute("aria-expanded"))
    .map((button) => button.textContent);
}

/** A still taken by `TAKE STILL`, its job run to its end. */
async function takeStill(view: ViewDisplayHarness): Promise<void> {
  await view.user.click(screen.getByRole("button", { name: "TAKE STILL" }));
  await act(async () => {
    await settle();
  });
}

/** The stills' lines away from their panel: under the fold row or MAXIMIZE VIEW's line. */
function awayLines(): ReadonlyArray<string | null> {
  return screen
    .queryAllByText(statusLine(/^STILL/u), { selector: ".view-stills__standing output" })
    .map((line) => line.textContent);
}

describe("VIEW's stills (R11.T10.b)", () => {
  it("stands no stills panel, and folds none, while it has no still source", async () => {
    await setup(undefined, COMPACT_VIEW_PX);
    expect([
      screen.queryByRole("region", { name: "Stills PRIMARY" }),
      screen.queryByRole("button", { name: "STILLS" }),
    ]).toEqual([null, null]);
  });

  it("stands its stills panel with a still source, TAKE STILL offered", async () => {
    await setup(source());
    expect(
      screen.getByRole("button", { name: "TAKE STILL" }).getAttribute("aria-disabled"),
    ).toBeNull();
  });

  it("folds the stills behind STILLS, after EXPOSURE, in the compact layout", async () => {
    await setup(source(), COMPACT_VIEW_PX);
    expect(disclosures()).toEqual(["Instruments", "CAMERA", "STYLE", "EXPOSURE", "STILLS"]);
  });

  it("opens the stills alone, folding CAMERA", async () => {
    const view = await setup(source(), COMPACT_VIEW_PX);
    const folded = screen.queryByRole("region", { name: "Stills PRIMARY" });
    await view.user.click(screen.getByRole("button", { name: "STILLS" }));
    expect([
      folded,
      screen.getByRole("region", { name: "Stills PRIMARY" }) instanceof HTMLElement,
      screen.queryByRole("region", { name: "Camera PRIMARY" }),
    ]).toEqual([null, true, null]);
  });

  it("states STILLS SUSPENDED and how the still ended under the fold row while folded", async () => {
    const device = new FakeStillDevice("absent");
    device.loseAtBatch = 0;
    const view = await setup(source(device), COMPACT_VIEW_PX);
    await view.user.click(screen.getByRole("button", { name: "STILLS" }));
    await takeStill(view);
    // Folding STILLS opens CAMERA again: the stills' lines go under the row.
    await view.user.click(screen.getByRole("button", { name: "STILLS" }));
    expect(awayLines()).toEqual([
      "STILLS SUSPENDED",
      "STILL ABANDONED: the graphics device was lost, TAKE STILL to retry",
    ]);
  });

  it("states the stills' lines under MAXIMIZE VIEW, STILLS DISABLED with its cause", async () => {
    const device = new FakeStillDevice("absent");
    device.loseAtBatch = 0;
    const view = await setup(source(device));
    await takeStill(view);
    device.loseAtBatch = device.batches.length;
    await takeStill(view);
    await view.user.click(screen.getByRole("button", { name: /MAXIMIZE VIEW/u }));
    expect(awayLines()).toEqual([
      "STILLS DISABLED: a second device loss, relaunch to retry",
      "STILL ABANDONED: the graphics device was lost, relaunch to retry",
    ]);
  });

  it("states nothing of the stills away from the panel while it stands open", async () => {
    await setup(source());
    expect(awayLines()).toEqual([]);
  });
});
