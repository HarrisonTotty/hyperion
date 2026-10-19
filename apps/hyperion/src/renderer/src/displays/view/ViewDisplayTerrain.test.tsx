import type { ResponseBody, ResponseFor } from "@hyperion/protocol";
import { act, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  fieldBlock,
  fieldCover,
  fieldPayload,
  latestFieldAsk,
  moonLikeHeader,
  readyField,
  sendFieldFrames,
} from "../../test/coarseFieldFixtures";
import { earthDetail, FIXTURE_EARTH } from "../../test/planetaryFixture";
import { SCENE_TIDAL_RADIUS_M, sceneClock, sliceSceneSystem } from "../../test/sceneFixture";
import { TERRAIN_NOT_AVAILABLE } from "../../view/terrain/annunciation";
import { MAX_CONSECUTIVE_WORKER_FAILURES } from "../../view/terrain/workers/pool";

import {
  nominalStore,
  openUniverse,
  primaryLabelBlock,
  renderViewDisplay,
  sceneArrives,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import { GraphicsStatusStore, initialGraphicsStatus } from "../../view/engine/status";
import { instrumentViewId, PRIMARY_VIEW_ID } from "./viewNames";
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

  it("gives each view's frames its own terrain of the session, by its ID (R10.T16.c)", async () => {
    const view = await serverScene();
    await openInstrument(view, "INSTRUMENT 1");
    const frames = framesOver(view, 200);
    const sources = new Set(frames.map(({ inputs }) => inputs.terrain?.source));
    expect([
      sources.size,
      sources.has(undefined),
      new Set(frames.map(({ inputs }) => inputs.terrain?.view)),
    ]).toEqual([1, false, new Set([PRIMARY_VIEW_ID, instrumentViewId(1)])]);
  });

  it("gives a kept scene's frames no terrain (R10.T16.c)", async () => {
    const view = await setup();
    const frames = framesOver(view, 100);
    expect(frames.length > 0 && frames.every(({ inputs }) => inputs.terrain === undefined)).toBe(
      true,
    );
  });

  it("states each view's own terrain annunciation on its own label block (G10; R10.T16.c)", async () => {
    const view = await serverScene();
    await openInstrument(view, "INSTRUMENT 1");
    // Each view's frame says what its own terrain annunciates: the primary's streaming, the
    // instrument's detail limited, as `LiveFrameTerrain` gives them from the session's passes.
    // The drawer's own draw, called on its drawer as the loop calls it.
    // oxlint-disable-next-line typescript/unbound-method
    const draw = ViewFrameDrawer.prototype.draw;
    vi.spyOn(ViewFrameDrawer.prototype, "draw").mockImplementation(function (
      this: ViewFrameDrawer,
      inputs: ViewFrameInputs,
    ) {
      const drawn = draw.call(this, inputs);
      const own =
        inputs.terrain?.view === PRIMARY_VIEW_ID ? "TERRAIN: STREAMING" : "TERRAIN: DETAIL LIMITED";
      return drawn === null ? null : { ...drawn, terrain: own };
    });
    view.advance(600);
    await settle();
    const primary = primaryLabelBlock();
    const instrument = screen.getByRole("region", { name: "INSTRUMENT 1" });
    expect([
      within(primary).queryByText("TERRAIN: STREAMING") !== null,
      within(primary).queryByText("TERRAIN: DETAIL LIMITED"),
      within(instrument).queryByText("TERRAIN: DETAIL LIMITED") !== null,
      within(instrument).queryByText("TERRAIN: STREAMING"),
    ]).toEqual([true, null, true, null]);
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

/** The slice's Earth's equatorial radius, m (the fixture's figure, WGS 84's). */
const EARTH_RADIUS_M = 6_378_137;

/** Answers the scene subscription with the ship 2,000 km over the slice's Earth, in its frame. */
async function shipOverEarthArrives(view: ViewDisplayHarness): Promise<void> {
  const answer: ResponseBody = {
    kind: "subscribe",
    subscription: 5,
    state: {
      topic: "scene",
      sequence: 0,
      clock: sceneClock(3_000),
      ship: {
        position: { frame: "body", body: FIXTURE_EARTH, offset_m: [0, 0, EARTH_RADIUS_M + 2e6] },
        velocity_m_s: [0, 0, 0],
        time: { seconds: 3_000, nanos: 0 },
      },
      system: sliceSceneSystem(),
      tidal_radius_m: SCENE_TIDAL_RADIUS_M,
      craft: [],
    },
  };
  await act(async () => {
    view.socket.serverAnswers("subscribe", () => answer);
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** The primary's label block's fault lines, as they read. */
function faultLines(): string[] {
  return within(primaryLabelBlock())
    .queryAllByRole("status")
    .filter((output) => output.classList.contains("request-status__text--fault"))
    .map((output) => output.textContent);
}

/** The slice's Earth's record, its hooks `ok` with a detail seed. */
function earthWithSeed(): ResponseFor<"body_detail"> {
  const detail = earthDetail();
  return {
    ...detail,
    record: {
      ...detail.record,
      hooks: { state: "ok", value: { detail_seed: { state: "ok", value: "0123456789abcdef" } } },
    },
  };
}

describe("the terrain's fault on a live VIEW (R10.T16.b's line, a body handed over in a live frame; R10.T16.c)", () => {
  it("states the fault on the primary's label block while it hands the Earth over and every height worker is lost", async () => {
    const fake = timedEngineSource();
    const view = renderViewDisplay({
      store: await nominalStore(),
      source: fake.source,
      engines: fake.engines,
    });
    await settle();
    view.advance(100);
    await openUniverse(view);
    await shipOverEarthArrives(view);
    await settle();
    view.advance(300);
    // The primary draws photorealistically.
    await view.user.keyboard("4");
    view.advance(100);
    await settle();
    view.advance(300);
    await settle();
    // The Earth, the camera's local body, has its field and its seed asked; both answered.
    const { id } = latestFieldAsk(view.socket);
    const bulk = sendFieldFrames(view.socket, id, fieldPayload([fieldBlock(50, 1)]));
    await act(async () => {
      view.socket.serverResponds(
        id,
        readyField(moonLikeHeader(FIXTURE_EARTH), 1, fieldCover([0, 2, 30]), bulk),
      );
      view.socket.serverAnswers("body_detail", () => earthWithSeed());
      await vi.advanceTimersByTimeAsync(0);
    });
    view.advance(500);
    await settle();
    const before = faultLines();
    // Every height worker the session runs fails, each replacement too, until none is left.
    const workers = view.heightWorkers.length;
    for (let round = 0; round < MAX_CONSECUTIVE_WORKER_FAILURES; round += 1) {
      for (const worker of view.heightWorkers.slice(-workers)) {
        worker.fail();
      }
    }
    view.advance(500);
    await settle();
    // The fault stands only for a view that hands a body over: the primary's live frame does.
    expect([before, faultLines()]).toEqual([[], [TERRAIN_NOT_AVAILABLE]]);
    expect(within(primaryLabelBlock()).queryByText("TERRAIN: STREAMING")).toBeNull();
  });
});
