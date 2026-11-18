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
import {
  type PhotorealStillView,
  PhotorealStills,
  STILL_UNAVAILABLE,
} from "../../view/photoreal/renderer";
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

/**
 * `TAKE STILL`'s held-back reason for `cause` as its accessible description reads: its state, set
 * apart by `StatusText`, then the cause.
 */
function reason(cause: string): RegExp {
  return new RegExp(`^NOT AVAILABLE: ?${cause}$`, "u");
}

/** The photorealistic renderer's hook, holding the view the stage attached. */
class CapturingStills extends PhotorealStills {
  attached: PhotorealStillView | null = null;

  override attach(view: PhotorealStillView): () => void {
    this.attached = view;
    return super.attach(view);
  }
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

/** The stills panel opened by its disclosure, in either layout. */
async function openStills(view: ViewDisplayHarness): Promise<void> {
  await view.user.click(screen.getByRole("button", { name: "STILLS" }));
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
    const view = await setup(source());
    await openStills(view);
    expect(
      screen.getByRole("button", { name: "TAKE STILL" }).getAttribute("aria-disabled"),
    ).toBeNull();
  });

  it("holds TAKE STILL back through the photorealistic renderer's hook while the view draws its wireframe", async () => {
    const view = await setup(new PhotorealStills());
    view.advance(500);
    await act(async () => {
      await settle();
    });
    await openStills(view);
    const take = screen.getByRole("button", { name: "TAKE STILL" });
    expect(take).toHaveAttribute("aria-disabled", "true");
    expect(take).toHaveAccessibleDescription(reason(STILL_UNAVAILABLE.noImage));
  });

  it("holds TAKE STILL back before the view is drawn, with no engine ready", async () => {
    const view = renderViewDisplay({
      store: new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
      source: {
        requestAdapter: () => new Promise(() => undefined),
        load: () => new Promise(() => undefined),
      },
      engines: [],
      stillSource: new PhotorealStills(),
    });
    await settle();
    await openStills(view);
    expect(screen.getByRole("button", { name: "TAKE STILL" })).toHaveAccessibleDescription(
      reason(STILL_UNAVAILABLE.noView),
    );
  });

  it("gives a still's label the label block whole, whatever MAXIMIZE VIEW folds", async () => {
    const stills = new CapturingStills();
    const view = await setup(stills);
    view.advance(500);
    const whole = stills.attached?.request().readings.map(({ label }) => label) ?? [];
    await view.user.click(screen.getByRole("button", { name: /MAXIMIZE VIEW/u }));
    view.advance(500);
    const maximized = stills.attached?.request().readings.map(({ label }) => label) ?? [];
    expect([whole.includes("TIME"), maximized]).toEqual([true, whole]);
  });

  it("folds the stills behind STILLS, after CAMERA, whose place they share, in the compact layout", async () => {
    await setup(source(), COMPACT_VIEW_PX);
    expect(disclosures()).toEqual(["Instruments", "CAMERA", "STILLS", "STYLE", "EXPOSURE"]);
  });

  it("folds the stills with CAMERA in the full layout, behind their own two buttons, CAMERA open", async () => {
    const view = await setup(source());
    const before = [
      disclosures(),
      screen.queryByRole("region", { name: "Stills PRIMARY" }),
      screen.getByRole("region", { name: "Camera PRIMARY" }) instanceof HTMLElement,
    ];
    await openStills(view);
    expect([
      ...before,
      screen.getByRole("region", { name: "Stills PRIMARY" }) instanceof HTMLElement,
      screen.queryByRole("region", { name: "Camera PRIMARY" }),
    ]).toEqual([["CAMERA", "STILLS"], null, true, true, null]);
  });

  it("folds no panel in the full layout while it has no still source", async () => {
    await setup(undefined);
    expect([
      disclosures(),
      screen.getByRole("region", { name: "Camera PRIMARY" }) instanceof HTMLElement,
    ]).toEqual([[], true]);
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

  it("states STILLS LIMITED and how the still ended under the fold row while folded", async () => {
    const device = new FakeStillDevice("absent");
    device.loseAtBatch = 0;
    const view = await setup(source(device), COMPACT_VIEW_PX);
    await view.user.click(screen.getByRole("button", { name: "STILLS" }));
    await takeStill(view);
    // Folding STILLS opens CAMERA again: the stills' lines go under the row, as one.
    await view.user.click(screen.getByRole("button", { name: "STILLS" }));
    expect(awayLines()).toEqual([
      "STILLS LIMITED · STILL NOT SAVED: the graphics device was lost, TAKE STILL to retry",
    ]);
  });

  it("states the stills' lines under MAXIMIZE VIEW, STILLS DISABLED with its cause", async () => {
    const device = new FakeStillDevice("absent");
    device.loseAtBatch = 0;
    const view = await setup(source(device));
    await openStills(view);
    await takeStill(view);
    device.loseAtBatch = device.batches.length;
    await takeStill(view);
    await view.user.click(screen.getByRole("button", { name: /MAXIMIZE VIEW/u }));
    // The cause and remedy alone, which say why the still was abandoned.
    expect(awayLines()).toEqual(["STILLS DISABLED: a second device loss, relaunch to retry"]);
  });

  it("states nothing of the stills away from the panel while it stands open", async () => {
    const view = await setup(source());
    await openStills(view);
    await takeStill(view);
    expect(awayLines()).toEqual([]);
  });
});
