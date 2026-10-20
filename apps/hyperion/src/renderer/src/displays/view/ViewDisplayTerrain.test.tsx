import type { ResponseBody, ResponseFor, SurfaceHeaderDto } from "@hyperion/protocol";
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
import { vec3 } from "../../geometry/vec3";
import { earthDetail, FIXTURE_EARTH } from "../../test/planetaryFixture";
import { SCENE_TIDAL_RADIUS_M, sceneClock, sliceSceneSystem } from "../../test/sceneFixture";
import {
  ANNUNCIATION_ONSET_MS,
  TERRAIN_COMPUTATION_FAILED,
  TERRAIN_NOT_AVAILABLE,
} from "../../view/terrain/annunciation";
import { PALETTE_NONE, type SurfaceGroundReadout } from "../../view/terrain/workers/messages";
import { NO_BREAK_SPACE, SURVEY_PENDING, TERRAIN_NOT_MODELLED } from "../../view/terrain/readouts";
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
import { blockBytes } from "../../view/scenes/terrainGolden";
import {
  TERRAIN_SCENE_NAME,
  terrainSceneFixture,
  terrainTestGolden,
} from "../../view/scenes/terrainScene";
import type { TerrainSession } from "../../view/terrain/session";
import type { TerrainFixture } from "./useTerrainSession";
import { instrumentViewId, PRIMARY_VIEW_ID } from "./viewNames";
import { GROUND_NOTE_NAME } from "./ViewMarkList";
import { type ViewFrameDrawn, ViewFrameDrawer, type ViewFrameInputs } from "./viewFrameDrawer";
import { runPose, SCENE_OPTIONS } from "./viewRun";

afterEach(() => {
  vi.useRealTimers();
});

/** `VIEW` rendered over a fake engine, its first frames drawn, with the views check's seams given. */
async function setup(
  seams: Pick<Parameters<typeof renderViewDisplay>[0], "sceneOptions" | "watchTerrain"> = {},
): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
    source: fake.source,
    engines: fake.engines,
    ...seams,
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

  describe("each view's own terrain annunciation on its own label block (G10; R10.T16.c)", () => {
    /**
     * The Earth handed over to the photorealistic primary, its field answered and its bakes held
     * back, INSTRUMENT 1 open in the wireframe beside it, past the annunciation's onset.
     */
    async function earthStreaming(): Promise<void> {
      const view = await earthHandedOver();
      await openInstrument(view, "INSTRUMENT 1");
      view.advance(ANNUNCIATION_ONSET_MS + 500);
      await settle();
    }

    it("states TERRAIN: STREAMING on the primary's, whose terrain waits for its bakes", async () => {
      await earthStreaming();
      expect(within(primaryLabelBlock()).getByText("TERRAIN: STREAMING")).toBeInTheDocument();
    });

    it("states none on a wireframe instrument's, which hands nothing over", async () => {
      await earthStreaming();
      expect(
        within(screen.getByRole("region", { name: "INSTRUMENT 1" })).queryByText(/^TERRAIN:/u),
      ).toBeNull();
    });
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

/** The `TERRAIN TEST` option's fixture, as `SCENE_OPTIONS` holds it. */
function terrainTestFixtureOfOptions(): TerrainFixture {
  const fixture = SCENE_OPTIONS.find(({ name }) => name === TERRAIN_SCENE_NAME)?.terrain;
  if (fixture === undefined) {
    throw new Error("TERRAIN TEST carries a terrain fixture");
  }
  return fixture;
}

describe("VIEW's TERRAIN TEST (R10.T16.d)", () => {
  it("makes its session over its fixture, the first payload reaching the height workers and nothing sent over the link", async () => {
    const view = await setup();
    await view.user.click(screen.getByRole("button", { name: TERRAIN_SCENE_NAME }));
    for (let step = 0; step < 4; step += 1) {
      view.advance(100);
      // Each step waits on the last.
      // oxlint-disable-next-line no-await-in-loop
      await settle();
    }
    const first = view.heightWorkers[0]?.fieldMessages ?? [];
    const blocks = terrainTestGolden().payloads[0]?.blocksHex ?? [];
    expect(
      first.map((message) =>
        message.kind === "field-release" ? null : Array.from(new Uint8Array(message.block)),
      ),
    ).toEqual(blocks.map((hex) => Array.from(blockBytes(hex))));
    expect([
      view.socket.requestsOfKind("surface_field"),
      view.socket.requestsOfKind("body_detail"),
    ]).toEqual([[], []]);
  });

  it("tells its fixture, each frame, how far its script has played", async () => {
    const fixture = terrainTestFixtureOfOptions();
    if (fixture.notePlayed === undefined) {
      throw new Error("TERRAIN TEST's fixture follows its script");
    }
    const played = vi.spyOn(fixture, "notePlayed");
    const view = await setup();
    await view.user.click(screen.getByRole("button", { name: TERRAIN_SCENE_NAME }));
    played.mockClear();
    view.advance(1_000);
    const times = played.mock.calls.map(([scriptS]) => scriptS);
    expect([
      times.length > 10,
      times.every((t, at) => at === 0 || t >= (times[at - 1] ?? 0)),
      (times.at(-1) ?? 0) > 0.5,
    ]).toEqual([true, true, true]);
  });
});

/** A survey-edge note of `TEST MOON`, by its accessible name, whatever its range. */
const TEST_MOON_EDGE = /^TEST MOON, survey edge, range [\d.,]+ (?:m|km|Mm)$/u;

/** `VIEW` on `TERRAIN TEST` over a nominal adapter, its first payload arrived and drawn. */
async function terrainTestShown(): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
  });
  await settle();
  view.advance(100);
  await view.user.click(screen.getByRole("button", { name: TERRAIN_SCENE_NAME }));
  await playFor(view, 6);
  return view;
}

/** Plays `steps` × 300 ms of frames, letting each step's work settle. */
async function playFor(view: ViewDisplayHarness, steps: number): Promise<void> {
  for (let step = 0; step < steps; step += 1) {
    view.advance(300);
    // Each step waits on the last.
    // oxlint-disable-next-line no-await-in-loop
    await settle();
  }
}

/**
 * Switches the `CONTROLS` view to the photorealistic style by its key, checking that its label
 * block (`block`) then states it.
 */
async function photorealistic(view: ViewDisplayHarness, block: () => HTMLElement): Promise<void> {
  await view.user.keyboard("4");
  await playFor(view, 4);
  const style = within(block()).getByText("STYLE", { selector: "dt" }).nextElementSibling;
  if (style?.textContent !== "PHOTOREALISTIC") {
    throw new Error(`the view draws ${String(style?.textContent)}, not PHOTOREALISTIC`);
  }
}

/** Opens INSTRUMENT 1 and points `CONTROLS` at it, so that the side column shows its list. */
async function controlInstrument(view: ViewDisplayHarness): Promise<void> {
  await openInstrument(view, "INSTRUMENT 1");
  await view.user.click(
    within(screen.getByRole("group", { name: "CONTROLS" })).getByRole("button", {
      name: "INSTRUMENT 1",
    }),
  );
  await playFor(view, 4);
}

describe("VIEW's lists note the survey edges (R10.T16.c; decision-r10-t16-review §1)", () => {
  it("notes TEST MOON's edge in the primary's list in the wireframe", async () => {
    await terrainTestShown();
    expect(
      within(screen.getByRole("region", { name: "Targets PRIMARY" })).getByRole("note", {
        name: TEST_MOON_EDGE,
      }),
    ).toBeInTheDocument();
  });

  it("notes TEST MOON's edge in the primary's list in the photorealistic style", async () => {
    const view = await terrainTestShown();
    await photorealistic(view, primaryLabelBlock);
    expect(
      within(screen.getByRole("region", { name: "Targets PRIMARY" })).getByRole("note", {
        name: TEST_MOON_EDGE,
      }),
    ).toBeInTheDocument();
  });

  it("notes TEST MOON's edge in an instrument's own list in the wireframe", async () => {
    const view = await terrainTestShown();
    await controlInstrument(view);
    expect(
      within(screen.getByRole("region", { name: "Targets INSTRUMENT 1" })).getByRole("note", {
        name: TEST_MOON_EDGE,
      }),
    ).toBeInTheDocument();
  });

  it("notes TEST MOON's edge in an instrument's own list in the photorealistic style", async () => {
    const view = await terrainTestShown();
    await controlInstrument(view);
    await photorealistic(view, () => screen.getByRole("region", { name: "INSTRUMENT 1" }));
    expect(
      within(screen.getByRole("region", { name: "Targets INSTRUMENT 1" })).getByRole("note", {
        name: TEST_MOON_EDGE,
      }),
    ).toBeInTheDocument();
  });

  it("notes no survey edge in a kept scene with no terrain", async () => {
    await setup();
    expect(screen.queryByRole("note")).toBeNull();
  });
});

describe("VIEW's seams for the views check's terrain run (R10.T16.d, part 2)", () => {
  it("offers the kept scenes it is given, TERRAIN TEST's fixture theirs, and lets the run watch its session", async () => {
    const fixture = terrainSceneFixture(0);
    const sceneOptions = SCENE_OPTIONS.map((option) =>
      option.name === TERRAIN_SCENE_NAME ? Object.assign({}, option, { terrain: fixture }) : option,
    );
    const watched: string[] = [];
    const view = await setup({
      sceneOptions,
      watchTerrain: (session) => {
        watched.push(`made ${String(session.workerCount)}`);
        return () => {
          watched.push("let go");
        };
      },
    });
    await view.user.click(screen.getByRole("button", { name: TERRAIN_SCENE_NAME }));
    for (let step = 0; step < 4; step += 1) {
      view.advance(100);
      // Each step waits on the last.
      // oxlint-disable-next-line no-await-in-loop
      await settle();
    }
    expect([watched, fixture.traffic().fieldAsks]).toEqual([[expect.stringMatching(/^made /)], 1]);
    await view.user.click(screen.getByRole("button", { name: "PHASE TEST" }));
    view.advance(100);
    await settle();
    expect(watched).toEqual([expect.stringMatching(/^made /), "let go"]);
  });
});

/** The slice's Earth's equatorial radius, m (the fixture's figure, WGS 84's). */
const EARTH_RADIUS_M = 6_378_137;

/**
 * Answers the scene subscription with the ship `heightM` (2,000 km by default) over the slice's
 * Earth, in its frame.
 */
async function shipOverEarthArrives(view: ViewDisplayHarness, heightM = 2e6): Promise<void> {
  const answer: ResponseBody = {
    kind: "subscribe",
    subscription: 5,
    state: {
      topic: "scene",
      sequence: 0,
      clock: sceneClock(3_000),
      ship: {
        position: {
          frame: "body",
          body: FIXTURE_EARTH,
          offset_m: [0, 0, EARTH_RADIUS_M + heightM],
        },
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

/** The slice's Earth's field header: the Moon-like fixture's, at the Earth's figure (WGS 84's). */
function earthHeader(): SurfaceHeaderDto {
  return {
    ...moonLikeHeader(FIXTURE_EARTH),
    radius_m: 6_371_000,
    equatorial_radius_m: EARTH_RADIUS_M,
    polar_radius_m: 6_356_752.314_245,
  };
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

/**
 * `VIEW` drawing the server's scene photorealistically, the ship 2,000 km over the slice's Earth,
 * the Earth's field (surveyed whole) and seed answered and its bakes held back by the scripted
 * workers, so that the primary hands it over with its terrain waiting.
 */
async function earthHandedOver(
  seams: Pick<Parameters<typeof renderViewDisplay>[0], "watchTerrain"> = {},
): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
    ...seams,
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
      // Surveyed whole at level 5, so that the ground in view waits for its bakes.
      readyField(earthHeader(), 1, fieldCover([0, 6 * 4 ** 5, 30]), bulk),
    );
    view.socket.serverAnswers("body_detail", () => earthWithSeed());
    await vi.advanceTimersByTimeAsync(0);
  });
  view.advance(500);
  await settle();
  return view;
}

describe("the terrain's fault on a live VIEW (R10.T16.b's line, a body handed over in a live frame; R10.T16.c)", () => {
  it("states the fault on the primary's label block while it hands the Earth over and every height worker is lost", async () => {
    const view = await earthHandedOver();
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

  /** The Earth handed over, with the session `VIEW` made. */
  async function earthAndSession(): Promise<{
    readonly view: ViewDisplayHarness;
    readonly session: TerrainSession;
  }> {
    const sessions: TerrainSession[] = [];
    const view = await earthHandedOver({
      watchTerrain: (session) => {
        sessions.push(session);
        return () => undefined;
      },
    });
    const session = sessions.at(-1);
    if (session === undefined) {
      throw new Error("VIEW made no terrain session");
    }
    return { view, session };
  }

  it("states the computation's fault, and draws no terrain, once the session's step has thrown for 250 ms (R10.T16.e)", async () => {
    const { view, session } = await earthAndSession();
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const before = faultLines();
    vi.spyOn(session, "step").mockImplementation(() => {
      throw new Error("a bug of the session's step");
    });
    view.advance(ANNUNCIATION_ONSET_MS + 250);
    await settle();
    expect([
      before,
      faultLines(),
      session.pass(FIXTURE_EARTH)?.faulted,
      within(primaryLabelBlock()).queryByText("TERRAIN: STREAMING"),
      // Said once, however many steps threw.
      error.mock.calls.filter(([what]) => what === "the terrain session's step failed:").length,
    ]).toEqual([[], [TERRAIN_COMPUTATION_FAILED], true, null, 1]);
  });

  it("states nothing for a single step that throws", async () => {
    const { view, session } = await earthAndSession();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    vi.spyOn(session, "step").mockImplementationOnce(() => {
      throw new Error("a bug of the session's step");
    });
    view.advance(ANNUNCIATION_ONSET_MS + 250);
    await settle();
    expect([faultLines(), session.pass(FIXTURE_EARTH)?.faulted]).toEqual([[], false]);
  });
});

/**
 * Answers every bake the height workers hold with a flat patch over a field surveyed whole, as the
 * real workers would bake the slice's Earth; returns how many.
 */
function answerEveryBake(view: ViewDisplayHarness): number {
  let answered = 0;
  for (const worker of view.heightWorkers) {
    for (const request of worker.pendingBakes) {
      const { key, settings } = request;
      const n = settings.normals === "double" ? 129 : 65;
      worker.reply({
        kind: "baked",
        id: request.id,
        bake: {
          key,
          generation: request.generation,
          originM: vec3(0, 0, EARTH_RADIUS_M),
          heights: new Float32Array(65 * 65 * 2),
          offsets: settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
          normals: new Float16Array(n * n * 2),
          heightRangeM: [0, 0],
          boundingRadiusM: 1,
          originHeightM: 0,
          skirtDepthM: 1,
          layers: {
            palette: new Uint8Array(16).fill(PALETTE_NONE),
            weights: new Uint8Array(),
            surveyMask: new Uint8Array(65 * 65).fill(255),
            fieldLevel: 5,
            boundM: 1,
            sigmaM: 0,
            complete: true,
            unsurveyedCells: new Uint32Array(),
            photometry: new Float32Array(40),
            horizonNear: new Uint32Array(),
            horizonFar: new Uint32Array(),
            horizonComplete: true,
            horizonRefusedCells: new Uint32Array(),
          },
        },
      });
      answered += 1;
    }
  }
  return answered;
}

/** The ship 500 km over the slice's Earth, inside its wireframe hand-over range at 1280 px. */
const LOW_OVER_EARTH_M = 5e5;

/**
 * `VIEW` drawing the server's scene in the wireframe, the ship 500 km over the slice's Earth, the
 * Earth's field (surveyed whole) and seed answered, its bakes held back by the scripted workers,
 * with the session `VIEW` made given to `sessions`.
 */
async function earthInWireframe(sessions: TerrainSession[]): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
    watchTerrain: (session) => {
      sessions.push(session);
      return () => undefined;
    },
  });
  await settle();
  view.advance(100);
  await openUniverse(view);
  await shipOverEarthArrives(view, LOW_OVER_EARTH_M);
  await settle();
  view.advance(300);
  await settle();
  const { id } = latestFieldAsk(view.socket);
  const bulk = sendFieldFrames(view.socket, id, fieldPayload([fieldBlock(50, 1)]));
  await act(async () => {
    view.socket.serverResponds(
      id,
      readyField(earthHeader(), 1, fieldCover([0, 6 * 4 ** 5, 30]), bulk),
    );
    view.socket.serverAnswers("body_detail", () => earthWithSeed());
    await vi.advanceTimersByTimeAsync(0);
  });
  view.advance(500);
  await settle();
  return view;
}

/** Plays `steps` × 100 ms of frames, answering every bake asked after each. */
async function bakeFor(view: ViewDisplayHarness, steps: number): Promise<void> {
  for (let step = 0; step < steps; step += 1) {
    view.advance(100);
    answerEveryBake(view);
    // Each step answers the bakes the one before asked.
    // oxlint-disable-next-line no-await-in-loop
    await settle();
  }
}

/** A label block's `CONTOUR INTERVAL` reading, or `null` where it states none. */
function contourIntervalIn(block: HTMLElement): string | null {
  const label = within(block).queryByText("CONTOUR INTERVAL", { selector: "dt" });
  const line = label?.parentElement ?? null;
  return line === null ? null : within(line).getByRole("status").textContent;
}

describe("the wireframe's terrain on a live VIEW (R10.T11)", () => {
  it("makes a wireframe pass of the Earth that the wireframe primary hands over, and no lit pass", async () => {
    const sessions: TerrainSession[] = [];
    await earthInWireframe(sessions);
    const session = sessions.at(-1);
    expect([
      [...(session?.wirePass(FIXTURE_EARTH)?.viewIds ?? [])],
      session?.pass(FIXTURE_EARTH) ?? null,
    ]).toEqual([[PRIMARY_VIEW_ID], null]);
  });

  it("states the contour interval on the primary's label block once its terrain draws", async () => {
    const view = await earthInWireframe([]);
    const before = contourIntervalIn(primaryLabelBlock());
    await bakeFor(view, 10);
    expect([before, contourIntervalIn(primaryLabelBlock())]).toEqual([
      null,
      expect.stringMatching(/^[\d.]+ k?m · INDEX [\d.]+ k?m$/u),
    ]);
  });

  it("states an instrument's own contour interval after its FOV once it hands the Earth over too (R10.T12.b)", async () => {
    const sessions: TerrainSession[] = [];
    const view = await earthInWireframe(sessions);
    await openInstrument(view, "INSTRUMENT 1");
    await bakeFor(view, 10);
    if (sessions.at(-1)?.wirePass(FIXTURE_EARTH)?.viewIds.has(instrumentViewId(1)) !== true) {
      throw new Error("INSTRUMENT 1 does not hand the Earth over in the wireframe");
    }
    const slot = screen.getByRole("region", { name: "INSTRUMENT 1" });
    const labels = within(slot)
      .getAllByRole("term")
      .map((term) => term.textContent);
    expect([
      contourIntervalIn(slot),
      labels.indexOf("CONTOUR INTERVAL") - labels.indexOf("FOV"),
    ]).toEqual([expect.stringMatching(/^[\d.]+ k?m · INDEX [\d.]+ k?m$/u), 1]);
  });

  describe("with the primary in CHASE and an instrument opened at its pose (R10.T12.b; decision-r10-view-block-room §10)", () => {
    /** The primary in CHASE by its key, then INSTRUMENT 1 opened, which starts in CHASE. */
    async function bothInChase(): Promise<ViewDisplayHarness> {
      const view = await earthInWireframe([]);
      await view.user.keyboard("2");
      view.advance(300);
      await openInstrument(view, "INSTRUMENT 1");
      await bakeFor(view, 10);
      return view;
    }

    it("draws the instrument at the primary's pose in every frame both draw", async () => {
      const view = await bothInChase();
      const frames = framesOver(view, 100);
      // Each instrument frame's camera position, beside the primary's in the frame of that time.
      const pairs = frames.flatMap(({ inputs }) => {
        const terrain = inputs.terrain;
        if (terrain?.view !== instrumentViewId(1)) {
          return [];
        }
        const primary = frames.find(
          ({ inputs: other }) =>
            other.terrain?.view === PRIMARY_VIEW_ID && other.terrain.nowMs === terrain.nowMs,
        );
        return [
          {
            primary: primary === undefined ? null : runPose(primary.inputs.run).positionM,
            instrument: runPose(inputs.run).positionM,
          },
        ];
      });
      expect({
        drawn: pairs.length > 0,
        positions: pairs.map(({ primary }) => primary),
      }).toEqual({ drawn: true, positions: pairs.map(({ instrument }) => instrument) });
    });

    it("states one CONTOUR INTERVAL on both views' label blocks", async () => {
      await bothInChase();
      const primary = contourIntervalIn(primaryLabelBlock());
      expect([
        primary,
        contourIntervalIn(screen.getByRole("region", { name: "INSTRUMENT 1" })),
      ]).toEqual([expect.stringMatching(/^[\d.]+ k?m · INDEX [\d.]+ k?m$/u), primary]);
    });
  });

  it("states the fault on the wireframe primary's label block while it hands the Earth over and every height worker is lost", async () => {
    const view = await earthInWireframe([]);
    const before = faultLines();
    const workers = view.heightWorkers.length;
    for (let round = 0; round < MAX_CONSECUTIVE_WORKER_FAILURES; round += 1) {
      for (const worker of view.heightWorkers.slice(-workers)) {
        worker.fail();
      }
    }
    view.advance(500);
    await settle();
    expect([before, faultLines()]).toEqual([[], [TERRAIN_NOT_AVAILABLE]]);
  });
});

describe("a closed instrument forgotten by the terrain session (the batch review's S17)", () => {
  /**
   * The Earth handed over to the primary and to INSTRUMENT 1, both photorealistic, with the
   * session `VIEW` made given to `sessions`.
   */
  async function bothHandOver(sessions: TerrainSession[]): Promise<ViewDisplayHarness> {
    const view = await earthHandedOver({
      watchTerrain: (session) => {
        sessions.push(session);
        return () => undefined;
      },
    });
    await controlInstrument(view);
    await photorealistic(view, () => screen.getByRole("region", { name: "INSTRUMENT 1" }));
    const views = sessions.at(-1)?.pass(FIXTURE_EARTH)?.viewIds;
    if (views?.has(instrumentViewId(1)) !== true) {
      throw new Error("INSTRUMENT 1 does not hand the Earth over");
    }
    return view;
  }

  it("leaves the Earth's pass to the primary alone once INSTRUMENT 1 is closed", async () => {
    const sessions: TerrainSession[] = [];
    const view = await bothHandOver(sessions);
    await view.user.click(
      within(screen.getByRole("group", { name: "INSTRUMENT 1" })).getByRole("button", {
        name: "CLOSE",
      }),
    );
    await playFor(view, 2);
    expect(sessions.at(-1)?.pass(FIXTURE_EARTH)?.viewIds).toEqual(new Set([PRIMARY_VIEW_ID]));
  });
});

/** A readout over surveyed ground under a 410 m survey, Estimated: the guide's Moon-like example's. */
const GROUND_READOUT: SurfaceGroundReadout = {
  kind: "ground",
  sight: {
    rangeM: 41_200,
    sigmaM: 300,
    resolved: false,
    hitM: [0, 0, EARTH_RADIUS_M],
    at: { face: 2, u: 0, v: 0 },
  },
  reading: {
    elevationM: 2140,
    elevationSigmaM: 180,
    slopeRmsRad: 0.21,
    slopeSigmaRad: 0.07,
    slopeBaselineM: 2,
    resolutionM: 410,
    resolved: false,
  },
  material: { classCode: 1, entry: null },
  properties: {
    bearingCapacityPa: 3.4e6,
    capacitySlopeRad: 0.21,
    padFriction: 0.2,
    frictionOnTemperature: false,
    temperatureK: null,
  },
};

/** Answers every terrain-readout query the height workers hold with `readout`; returns how many. */
function answerReadouts(view: ViewDisplayHarness, readout: SurfaceGroundReadout): number {
  let answered = 0;
  for (const worker of view.heightWorkers) {
    for (const query of worker.pendingGroundReadouts) {
      worker.reply({ kind: "ground-readout", id: query.id, body: query.body, readout });
      answered += 1;
    }
  }
  return answered;
}

/** Plays `steps` × 250 ms, answering after each step every readout query asked; returns how many. */
async function readFor(
  view: ViewDisplayHarness,
  steps: number,
  readout: SurfaceGroundReadout = GROUND_READOUT,
): Promise<number> {
  let answered = 0;
  for (let step = 0; step < steps; step += 1) {
    view.advance(250);
    answered += answerReadouts(view, readout);
    // Each step answers what the one before asked.
    // oxlint-disable-next-line no-await-in-loop
    await settle();
  }
  return answered;
}

/**
 * `VIEW` on `TERRAIN TEST` over a nominal adapter, its first payload arrived, the session it made
 * given to `sessions`: its primary's camera on `TEST HULL`, held in `TEST MOON`'s frame, so that its
 * `FRAME` is the moon's.
 */
async function terrainTestReading(sessions: TerrainSession[] = []): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
    watchTerrain: (session) => {
      sessions.push(session);
      return () => undefined;
    },
  });
  await settle();
  view.advance(100);
  await view.user.click(screen.getByRole("button", { name: TERRAIN_SCENE_NAME }));
  await playFor(view, 6);
  return view;
}

/** The primary's lower plate, by its canvas's description, or `null` while it states no terrain. */
function lowerPlate(): HTMLElement | null {
  const canvas = screen.getByRole("application", { name: /, PRIMARY, /u });
  const id = (canvas.getAttribute("aria-describedby") ?? "")
    .split(" ")
    .find((each) => each.endsWith("-lower"));
  return id === undefined ? null : document.getElementById(id);
}

/** The primary's lower plate, which a test expects to stand. */
function plate(): HTMLElement {
  const found = lowerPlate();
  if (found === null) {
    throw new Error("the primary has no lower plate");
  }
  return found;
}

/** A plate's reading of `label`, its value's text as it reads, or `null` where it has none. */
function readingIn(block: HTMLElement, label: string): string | null {
  const term = within(block).queryByText(label, { selector: "dt" });
  return term?.nextElementSibling?.textContent ?? null;
}

/** The primary's frames' line-of-sight inputs, and an instrument's, over the next `ms`. */
function sightsOver(
  view: ViewDisplayHarness,
  ms: number,
): { readonly primary: ReadonlyArray<unknown>; readonly instruments: ReadonlyArray<unknown> } {
  const frames = framesOver(view, ms);
  return {
    primary: frames
      .filter(({ inputs }) => inputs.terrain?.view === PRIMARY_VIEW_ID)
      .map(({ inputs }) => inputs.lineOfSightPoint),
    instruments: frames
      .filter(({ inputs }) => inputs.terrain?.view === instrumentViewId(1))
      .map(({ inputs }) => inputs.lineOfSightPoint),
  };
}

describe("the primary's terrain lines on its lower plate (R10.T12; decision-r10-view-block-room §3)", () => {
  it("states the survey line and no DATUM or readout before the readout's first answer", async () => {
    const view = await terrainTestReading();
    view.advance(300);
    await settle();
    expect([
      readingIn(plate(), "SURVEY COVERAGE") !== null,
      within(plate()).queryByText("GROUND AT LINE OF SIGHT"),
      readingIn(plate(), "DATUM"),
    ]).toEqual([true, null, null]);
  });

  it("states DATUM and the readout from its first answer, Estimated", async () => {
    const view = await terrainTestReading();
    await readFor(view, 4);
    expect({
      heading: within(plate()).getByText("GROUND AT LINE OF SIGHT").textContent,
      datum: readingIn(plate(), "DATUM"),
      elevation: readingIn(plate(), "ELEVATION")?.trim(),
    }).toEqual({
      heading: "GROUND AT LINE OF SIGHT",
      datum: expect.stringMatching(/^REFERENCE SPHER(?:E|OID) [\d.,]+ km/u),
      elevation: `~2140 m${NO_BREAK_SPACE}±${NO_BREAK_SPACE}180 m`,
    });
  });

  it("asks the primary's readout about four times a second, and an instrument's none", async () => {
    const view = await terrainTestReading();
    await openInstrument(view, "INSTRUMENT 1");
    await readFor(view, 2);
    const answered = await readFor(view, 8);
    expect(answered >= 6 && answered <= 9).toBe(true);
  });

  it("gives the primary's frames its point and range while the readout stands, and an instrument's none", async () => {
    const view = await terrainTestReading();
    await openInstrument(view, "INSTRUMENT 1");
    await readFor(view, 4);
    const sights = sightsOver(view, 200);
    expect({
      primary: [...new Set(sights.primary.map((each) => JSON.stringify(each)))],
      instruments: [...new Set(sights.instruments)],
    }).toEqual({
      primary: [JSON.stringify({ hasPoint: true, rangeM: 41_200 })],
      instruments: [undefined],
    });
  });

  it("states no survey line or readout on an instrument in its slot (decision-r10-t12-leans §3)", async () => {
    const view = await terrainTestReading();
    await openInstrument(view, "INSTRUMENT 1");
    await readFor(view, 4);
    const slot = screen.getByRole("region", { name: "INSTRUMENT 1" });
    expect([
      within(slot).queryByText("GROUND AT LINE OF SIGHT"),
      within(slot).queryByText("SURVEY COVERAGE"),
    ]).toEqual([null, null]);
  });

  it("notes GROUND AT LINE OF SIGHT first in the primary's list while the readout has a point", async () => {
    const view = await terrainTestReading();
    await readFor(view, 4);
    const targets = screen.getByRole("region", { name: "Targets PRIMARY" });
    expect([
      within(targets).getAllByRole("note")[0]?.getAttribute("aria-label"),
      within(targets).queryByRole("option", { name: /line of sight/iu }),
    ]).toEqual([GROUND_NOTE_NAME, null]);
  });

  it("leaves the tripod out in CHASE, the own hull over the point, its note still first (decision-r10-t12-commit3 §1)", async () => {
    const view = await terrainTestReading();
    await view.user.keyboard("2");
    await readFor(view, 4);
    const draw = vi.spyOn(ViewFrameDrawer.prototype, "draw");
    view.advance(200);
    const primary = draw.mock.calls.flatMap(([inputs], at): Array<ViewFrameDrawn | null> => {
      const result = draw.mock.results[at];
      return inputs.terrain?.view === PRIMARY_VIEW_ID && result?.type === "return"
        ? [result.value]
        : [];
    });
    draw.mockRestore();
    const targets = screen.getByRole("region", { name: "Targets PRIMARY" });
    expect({
      drawn: primary.length > 0,
      givenWay: primary.every(
        (drawn) => drawn?.lineOfSight?.boxPx === null && drawn.lineOfSight.yielded,
      ),
      note: within(targets).getAllByRole("note")[0]?.getAttribute("aria-label"),
    }).toEqual({ drawn: true, givenWay: true, note: GROUND_NOTE_NAME });
  });

  it("reads NO GROUND with no tripod's point and no note where the line of sight misses the ground", async () => {
    const view = await terrainTestReading();
    await readFor(view, 4, { kind: "no-ground" });
    const sights = sightsOver(view, 200);
    expect({
      status: within(plate()).getByText("NO GROUND").textContent,
      point: [...new Set(sights.primary.map((each) => JSON.stringify(each)))],
      note: screen.queryByRole("note", { name: GROUND_NOTE_NAME }),
    }).toEqual({
      status: "NO GROUND",
      point: [JSON.stringify({ hasPoint: false, rangeM: null })],
      note: null,
    });
  });

  it("marks the readout's values stale with their S on loss of the link, its constants not", async () => {
    // The server's scene, its FREE camera in the Earth's frame, whose FRAME is then the Earth's.
    const view = await earthInWireframe([]);
    await view.user.keyboard("3");
    await readFor(view, 4);
    if (readingIn(plate(), "ELEVATION") === null) {
      throw new Error("the readout does not stand before the link is lost");
    }
    act(() => {
      view.socket.close();
    });
    await readFor(view, 4);
    expect({
      elevation: readingIn(plate(), "ELEVATION")?.trim(),
      slope: readingIn(plate(), "SLOPE")?.trim(),
    }).toEqual({
      elevation: `~2140 m${NO_BREAK_SPACE}±${NO_BREAK_SPACE}180 m Sstale`,
      slope: `~12°${NO_BREAK_SPACE}±${NO_BREAK_SPACE}4° Sstale · BASELINE 2 m`,
    });
  });

  it("states no readout, DATUM or tripod under the terrain's fault, the survey line and its fault line standing (decision-r10-t12-leans §5)", async () => {
    const sessions: TerrainSession[] = [];
    const view = await terrainTestReading(sessions);
    await readFor(view, 4);
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    sessions.at(-1)?.failComputation();
    await readFor(view, 2);
    const sights = sightsOver(view, 200);
    expect({
      survey: readingIn(plate(), "SURVEY COVERAGE") !== null,
      heading: within(plate()).queryByText("GROUND AT LINE OF SIGHT"),
      datum: readingIn(plate(), "DATUM"),
      point: [...new Set(sights.primary)],
      faults: faultLines(),
    }).toEqual({
      survey: true,
      heading: null,
      datum: null,
      point: [undefined],
      faults: [TERRAIN_COMPUTATION_FAILED],
    });
  });

  it("faults the terrain once the readout's queries have been refused for 250 ms, saying so once", async () => {
    const view = await terrainTestReading();
    await readFor(view, 4);
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    for (let step = 0; step < 4; step += 1) {
      view.advance(250);
      for (const worker of view.heightWorkers) {
        for (const query of worker.pendingGroundReadouts) {
          worker.reply({
            kind: "ground-readout-failed",
            id: query.id,
            body: query.body,
            message: "a bug of the module's readout",
          });
        }
      }
      // Each step refuses what the one before asked.
      // oxlint-disable-next-line no-await-in-loop
      await settle();
    }
    view.advance(500);
    await settle();
    expect([
      faultLines(),
      error.mock.calls.filter(([what]) => what === "the terrain readout's query failed:").length,
    ]).toEqual([[TERRAIN_COMPUTATION_FAILED], 1]);
  });

  it("moves the readout's line of sight by the keyboard's NEXT TARGET, no pointer needed", async () => {
    const view = await terrainTestReading();
    await readFor(view, 4);
    view.advance(250);
    const before = view.heightWorkers.flatMap((worker) => worker.pendingGroundReadouts).at(-1);
    answerReadouts(view, GROUND_READOUT);
    await settle();
    await view.user.keyboard("]");
    await readFor(view, 4);
    view.advance(250);
    const after = view.heightWorkers.flatMap((worker) => worker.pendingGroundReadouts).at(-1);
    expect([
      before === undefined || after === undefined,
      JSON.stringify(after?.direction) === JSON.stringify(before?.direction),
    ]).toEqual([false, false]);
  });
});

describe("a kept scene's terrain lines (decision-r10-t12-commit3 §5)", () => {
  it("reads TERRAIN: NOT YET MODELLED where its FRAME is a body it gives no terrain", async () => {
    const view = await setup();
    await view.user.click(screen.getByRole("button", { name: "ECLIPSE TEST" }));
    await playFor(view, 2);
    expect([
      within(plate()).getByText("TERRAIN: NOT YET MODELLED").textContent,
      readingIn(plate(), "SURVEY COVERAGE"),
      within(plate()).queryByText("GROUND AT LINE OF SIGHT"),
      screen.queryByRole("note", { name: GROUND_NOTE_NAME }),
    ]).toEqual(["TERRAIN: NOT YET MODELLED", null, null, null]);
  });

  it("states no terrain lines where its FRAME is the system's", async () => {
    await setup();
    expect(lowerPlate()).toBeNull();
  });
});

/**
 * `VIEW` drawing the server's scene, its primary at the own ship's seat, the ship 2,000 km over the
 * slice's Earth in its frame: far inside 0.9 of the Earth's Hill radius (about 1.5 × 10⁹ m, which
 * its `full` grant gives), so that the ship is held in the Earth's frame; the Earth's field asked
 * and not yet answered.
 *
 * @throws Error when the primary's camera is not at the seat.
 */
async function seatOverEarth(): Promise<ViewDisplayHarness> {
  const view = await setup();
  await openUniverse(view);
  await shipOverEarthArrives(view);
  await settle();
  view.advance(300);
  await settle();
  if (readingIn(primaryLabelBlock(), "CAMERA") !== "SEAT") {
    throw new Error("the primary's camera is at the own ship's seat");
  }
  return view;
}

describe("the primary's survey line from the own ship in a served system (R10.T12; decision-r10-ship-frame §3)", () => {
  it("states SURVEY COVERAGE: PENDING on the primary's lower plate in SEAT beside the Earth, before its field answers", async () => {
    await seatOverEarth();
    expect(within(plate()).getByText(SURVEY_PENDING)).toBeInTheDocument();
  });

  it("states TERRAIN: NOT YET MODELLED there in SEAT once the server answers the Earth's terrain not modelled", async () => {
    const view = await seatOverEarth();
    const { id } = latestFieldAsk(view.socket);
    await act(async () => {
      view.socket.serverResponds(id, {
        kind: "surface_field",
        state: "not_modelled",
        section: "surface",
      });
      await vi.advanceTimersByTimeAsync(0);
    });
    view.advance(300);
    await settle();
    expect(within(plate()).getByText(TERRAIN_NOT_MODELLED)).toBeInTheDocument();
  });

  it("stands no lower plate in SEAT with the ship 1 au out, outside every sphere", async () => {
    await serverScene();
    if (readingIn(primaryLabelBlock(), "CAMERA") !== "SEAT") {
      throw new Error("the primary's camera is at the own ship's seat");
    }
    expect(lowerPlate()).toBeNull();
  });
});
