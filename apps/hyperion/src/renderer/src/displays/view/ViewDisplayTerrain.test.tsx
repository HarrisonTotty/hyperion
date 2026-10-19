import { act, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  openUniverse,
  renderViewDisplay,
  sceneArrives,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import { GraphicsStatusStore, initialGraphicsStatus } from "../../view/engine/status";
import { ViewFrameDrawer, type ViewFrameInputs } from "./viewFrameDrawer";

afterEach(() => {
  vi.useRealTimers();
});

/** `VIEW` rendered over a fake engine, its first frames drawn. */
async function setup(): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
    source: fake.source,
    engines: fake.engines,
  });
  await settle();
  view.advance(100);
  return view;
}

/** `VIEW` drawing the server's scene of an open universe. */
async function serverScene(): Promise<ViewDisplayHarness> {
  const view = await setup();
  await openUniverse(view);
  await sceneArrives(view);
  await settle();
  view.advance(300);
  await settle();
  return view;
}

async function openInstrument(view: ViewDisplayHarness, name: string): Promise<void> {
  await view.user.click(
    within(screen.getByRole("group", { name })).getByRole("button", { name: "OPEN" }),
  );
  view.advance(300);
}

/** Every frame the views' drawers draw over the next `ms`, with the drawer that drew it. */
function framesOver(
  view: ViewDisplayHarness,
  ms: number,
): ReadonlyArray<{ readonly drawer: unknown; readonly inputs: ViewFrameInputs }> {
  const draw = vi.spyOn(ViewFrameDrawer.prototype, "draw");
  view.advance(ms);
  const frames = draw.mock.calls.map(([inputs], at) => ({
    drawer: draw.mock.contexts[at],
    inputs,
  }));
  draw.mockRestore();
  return frames;
}

describe("VIEW's terrain session (R10.T16.a)", () => {
  it("is made for the server's scene of the open universe, its height workers started", async () => {
    const view = await serverScene();
    expect(view.heightWorkers.length).toBeGreaterThan(0);
  });

  it("is made for no kept scene that carries no terrain fixture", async () => {
    const view = await setup();
    expect(view.heightWorkers).toEqual([]);
  });

  it("gives every view's frames its class maps and reliefs, the instruments' included", async () => {
    const view = await serverScene();
    await openInstrument(view, "INSTRUMENT 1");
    const frames = framesOver(view, 200);
    const first = frames[0]?.inputs;
    expect([
      new Set(frames.map(({ drawer }) => drawer)).size,
      frames.every(
        ({ inputs }) =>
          inputs.classMaps !== undefined &&
          inputs.classMaps === first?.classMaps &&
          inputs.reliefs !== undefined &&
          inputs.reliefs === first.reliefs,
      ),
    ]).toEqual([2, true]);
  });

  it("gives a kept scene's frames no class maps or reliefs", async () => {
    const view = await setup();
    const frames = framesOver(view, 100);
    expect(
      frames.length > 0 &&
        frames.every(
          ({ inputs }) => inputs.classMaps === undefined && inputs.reliefs === undefined,
        ),
    ).toBe(true);
  });

  it("is disposed when a kept scene is chosen, its height workers stopped", async () => {
    const view = await serverScene();
    await view.user.click(screen.getByRole("button", { name: "PRECISION TEST" }));
    await act(async () => {
      await Promise.resolve();
    });
    expect([
      view.heightWorkers.length > 0,
      view.heightWorkers.every(
        (w) => w.terminated && w.listening.message === 0 && w.listening.error === 0,
      ),
    ]).toEqual([true, true]);
  });
});
