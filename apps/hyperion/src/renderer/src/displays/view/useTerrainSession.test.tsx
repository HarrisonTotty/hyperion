import {
  type ClientMessage,
  RequestClient,
  type RequestKind,
  type ResponseFor,
  type UniverseIdHex,
} from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { RECONNECT_DELAY_MS } from "../../lib/connection";
import type { SceneSurfaceEvent } from "../../lib/scene/useScene";
import { useServerLink } from "../../lib/serverLink";
import { binaryFrame } from "../../test/binaryFrames";
import {
  fieldBlock,
  fieldCover,
  fieldPayload,
  latestFieldAsk,
  moonLikeHeader,
  readyField,
  sendFieldFrames,
} from "../../test/coarseFieldFixtures";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { ScriptedHeightWorker } from "../../test/scriptedHeightWorker";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { SESSION_MOON, SESSION_SCENE, viewAt } from "../../test/terrainSessionFixtures";
import { viewId } from "../../view/camera/state";
import { IDENTITY_ROTATION } from "../../view/coords/rotation";
import { EngineUnavailable } from "../../view/engine/resilientEngine";
import type { RenderEngine } from "../../view/engine/types";
import {
  ANNUNCIATION_ONSET_MS,
  TERRAIN_COMPUTATION_FAILED,
  TERRAIN_NOT_AVAILABLE,
} from "../../view/terrain/annunciation";
import { TerrainFailureGuard } from "../../view/terrain/failureGuard";
import type { TerrainSession } from "../../view/terrain/session";
import { MAX_CONSECUTIVE_WORKER_FAILURES, type WorkerLike } from "../../view/terrain/workers/pool";
import { precisionScene } from "../../view/scenes/precision";
import { TERRAIN_SCENE_NAME } from "../../view/scenes/terrainScene";
import {
  stepTerrainSession,
  type SurfaceEvents,
  type TerrainFixture,
  terrainSessionSource,
  useTerrainFaults,
  useTerrainSession,
} from "./useTerrainSession";
import { SCENE_OPTIONS, type SceneOption } from "./viewRun";

const UNIVERSE: UniverseIdHex = "000000000000002a";
const OTHER_UNIVERSE: UniverseIdHex = "00000000000000b2";
const PRIMARY = viewId("primary");

/** A scene topic's surface events, told by the test. */
class SurfaceFeed {
  readonly #listeners = new Set<(event: SceneSurfaceEvent) => void>();

  readonly onSurface: SurfaceEvents = (listener) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** Listeners subscribed now. */
  get listening(): number {
    return this.#listeners.size;
  }

  tell(event: SceneSurfaceEvent): void {
    for (const listener of this.#listeners) {
      listener(event);
    }
  }
}

/** A terrain fixture over a request client of its own, recording what it is asked and its life. */
interface RecordedFixture {
  readonly fixture: TerrainFixture;
  readonly feed: SurfaceFeed;
  /** Every message its clients sent, first first. */
  readonly sent: ClientMessage[];
  /** Its clients, one a link opened. */
  readonly clients: RequestClient[];
  readonly life: { opened: number; closed: number };
}

function recordedFixture(): RecordedFixture {
  const feed = new SurfaceFeed();
  const sent: ClientMessage[] = [];
  const clients: RequestClient[] = [];
  const life = { opened: 0, closed: 0 };
  const fixture: TerrainFixture = {
    universe: OTHER_UNIVERSE,
    open: () => {
      life.opened += 1;
      const client = new RequestClient((message) => {
        sent.push(message);
        return true;
      });
      clients.push(client);
      return {
        requests: client,
        onSurface: feed.onSurface,
        close: () => {
          life.closed += 1;
          client.linkLost();
        },
      };
    },
  };
  return { fixture, feed, sent, clients, life };
}

/** The kinds of the requests `sent` holds. */
function kindsOf(sent: ReadonlyArray<ClientMessage>): RequestKind[] {
  return sent.flatMap((message) => (message.type === "request" ? [message.body.kind] : []));
}

/** What the stage under test draws: the server's scene, or a kept scene. */
type Choice =
  | {
      readonly kind: "server";
      readonly universe: UniverseIdHex | null;
      readonly onSurface: SurfaceEvents;
    }
  | { readonly kind: "kept"; readonly option: SceneOption };

interface Props {
  readonly choice: Choice;
  readonly engine: RenderEngine | null;
}

/** The hook as `ViewPanels` calls it, over the harness's link. */
function useUnderTest(
  { choice, engine }: Props,
  createWorker: () => WorkerLike,
): TerrainSession | null {
  const link = useServerLink();
  return useTerrainSession({
    engineState: engine === null ? { kind: "pending" } : { kind: "ready", engine },
    source: terrainSessionSource(choice.kind === "server" ? { ...choice, link } : choice),
    createWorker,
    workers: 2,
  });
}

/** Lets the outcomes' promise chains settle. */
async function settle(): Promise<void> {
  await act(async () => {
    for (let hop = 0; hop < 10; hop += 1) {
      // Each hop waits on the last.
      // oxlint-disable-next-line no-await-in-loop
      await Promise.resolve();
    }
  });
}

/** Renders the hook under a welcomed link, recording the height workers it starts. */
function renderSession(initial: Props) {
  const workers: ScriptedHeightWorker[] = [];
  const createWorker = (): WorkerLike => {
    const worker = new ScriptedHeightWorker();
    workers.push(worker);
    return worker;
  };
  const hook = renderHook((props: Props) => useUnderTest(props, createWorker), {
    initialProps: initial,
    wrapper: ServerLinkHarness,
  });
  act(() => {
    FakeWebSocket.latest().serverWelcomes();
  });
  return { ...hook, workers };
}

/** The session the hook holds, which the test expects there to be. */
function sessionOf(session: TerrainSession | null): TerrainSession {
  if (session === null) {
    throw new Error("the hook holds no session");
  }
  return session;
}

/** The moon wanted as the primary's local body, its field answered on `socket` at revision 1. */
async function holdMoon(session: TerrainSession, socket: FakeWebSocket): Promise<void> {
  session.noteView(PRIMARY, viewAt(5e6));
  session.step();
  const { id } = latestFieldAsk(socket);
  const bulk = sendFieldFrames(socket, id, fieldPayload([fieldBlock(50, 1)]));
  socket.serverResponds(
    id,
    readyField(moonLikeHeader(SESSION_MOON), 1, fieldCover([0, 2, 30]), bulk),
  );
  await settle();
}

/** A kept scene option for the test alone, with `terrain` its fixture. */
function fixtureOption(fixture: TerrainFixture): SceneOption {
  return { name: "FIXTURE TEST", make: precisionScene, terrain: fixture };
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("the terrain session's source (R10.T16.a)", () => {
  it("is the server's scene of the open universe, over the link's request client", () => {
    const { result } = renderHook(() => useServerLink(), { wrapper: ServerLinkHarness });
    const link = result.current;
    const { onSurface } = new SurfaceFeed();
    expect(terrainSessionSource({ kind: "server", universe: UNIVERSE, link, onSurface })).toEqual({
      kind: "server",
      universe: UNIVERSE,
      requests: link.requests,
      linkUp: false,
      onSurface,
    });
  });

  it("is none for the server's scene with no universe open", () => {
    const { result } = renderHook(() => useServerLink(), { wrapper: ServerLinkHarness });
    const { onSurface } = new SurfaceFeed();
    expect(
      terrainSessionSource({ kind: "server", universe: null, link: result.current, onSurface }),
    ).toBeNull();
  });

  it("is a kept scene's terrain fixture", () => {
    const { fixture } = recordedFixture();
    expect(terrainSessionSource({ kind: "kept", option: fixtureOption(fixture) })).toEqual({
      kind: "fixture",
      fixture,
    });
  });

  it("is none for a kept scene with no terrain fixture, every one but TERRAIN TEST (R10.T16.d)", () => {
    expect(
      SCENE_OPTIONS.map((option) => [
        option.name,
        terrainSessionSource({ kind: "kept", option })?.kind ?? null,
      ]),
    ).toEqual(
      SCENE_OPTIONS.map(({ name }) => [name, name === TERRAIN_SCENE_NAME ? "fixture" : null]),
    );
  });
});

describe("useTerrainSession (R10.T16.a)", () => {
  it("makes a session for the server's scene of the open universe", async () => {
    const engine = await countingRenderEngine();
    const feed = new SurfaceFeed();
    const { result, workers } = renderSession({
      choice: { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface },
      engine,
    });
    expect([result.current === null, workers.length]).toEqual([false, 2]);
  });

  it("asks a server session's fields of the link", async () => {
    const engine = await countingRenderEngine();
    const feed = new SurfaceFeed();
    const { result } = renderSession({
      choice: { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface },
      engine,
    });
    const session = sessionOf(result.current);
    session.noteView(PRIMARY, viewAt(5e6));
    session.step();
    expect(latestFieldAsk(FakeWebSocket.latest())).toMatchObject({
      body: SESSION_MOON,
      have: null,
    });
  });

  it("makes one for a kept scene with a terrain fixture, asking its fields of the fixture", async () => {
    const engine = await countingRenderEngine();
    const recorded = recordedFixture();
    const { result } = renderSession({
      choice: { kind: "kept", option: fixtureOption(recorded.fixture) },
      engine,
    });
    const session = sessionOf(result.current);
    session.noteView(PRIMARY, viewAt(5e6));
    session.step();
    expect([
      kindsOf(recorded.sent),
      FakeWebSocket.latest().requestsOfKind("surface_field"),
    ]).toEqual([["surface_field", "body_detail"], []]);
  });

  it("makes none for any other kept scene", async () => {
    const engine = await countingRenderEngine();
    const option = SCENE_OPTIONS[0];
    if (option === undefined) {
      throw new Error("VIEW keeps a scene");
    }
    const { result, workers } = renderSession({ choice: { kind: "kept", option }, engine });
    expect([result.current, workers.length]).toEqual([null, 0]);
  });

  it("makes none before the engine is ready", () => {
    const feed = new SurfaceFeed();
    const { result, workers } = renderSession({
      choice: { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface },
      engine: null,
    });
    expect([result.current, workers.length]).toEqual([null, 0]);
  });

  it("keeps its session, asking no field again, on a new scene of the same source", async () => {
    const engine = await countingRenderEngine();
    const feed = new SurfaceFeed();
    const { result, rerender } = renderSession({
      choice: { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface },
      engine,
    });
    const session = sessionOf(result.current);
    await holdMoon(session, FakeWebSocket.latest());
    rerender({ choice: { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface }, engine });
    session.step();
    expect([
      result.current === session,
      FakeWebSocket.latest().requestsOfKind("surface_field").length,
    ]).toEqual([true, 1]);
  });

  it("disposes its session with a change of source, leaving no worker", async () => {
    const engine = await countingRenderEngine();
    const feed = new SurfaceFeed();
    const { result, rerender, workers } = renderSession({
      choice: { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface },
      engine,
    });
    const option = SCENE_OPTIONS[0];
    if (option === undefined) {
      throw new Error("VIEW keeps a scene");
    }
    rerender({ choice: { kind: "kept", option }, engine });
    expect([
      result.current,
      workers.map((w) => [w.terminated, w.listening.message, w.listening.error]),
      feed.listening,
    ]).toEqual([
      null,
      [
        [true, 0, 0],
        [true, 0, 0],
      ],
      0,
    ]);
  });

  it("makes a new session for another universe", async () => {
    const engine = await countingRenderEngine();
    const feed = new SurfaceFeed();
    const { result, rerender } = renderSession({
      choice: { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface },
      engine,
    });
    const first = result.current;
    rerender({
      choice: { kind: "server", universe: OTHER_UNIVERSE, onSurface: feed.onSurface },
      engine,
    });
    expect([first === null, result.current === null, result.current === first]).toEqual([
      false,
      false,
      false,
    ]);
  });

  it("makes a new session with a new engine, and stops the old one's workers", async () => {
    const feed = new SurfaceFeed();
    const choice: Choice = { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface };
    const { result, rerender, workers } = renderSession({
      choice,
      engine: await countingRenderEngine(),
    });
    const first = result.current;
    const engine = await countingRenderEngine();
    rerender({ choice, engine });
    expect([result.current === first, workers.map((w) => w.terminated)]).toEqual([
      false,
      [true, true, false, false],
    ]);
  });

  it("disposes its session on unmount, closing a fixture's link", async () => {
    const engine = await countingRenderEngine();
    const recorded = recordedFixture();
    const { unmount, workers } = renderSession({
      choice: { kind: "kept", option: fixtureOption(recorded.fixture) },
      engine,
    });
    unmount();
    expect([workers.map((w) => w.terminated), recorded.life, recorded.feed.listening]).toEqual([
      [true, true],
      { opened: 1, closed: 1 },
      0,
    ]);
  });

  it("asks a held field again with its revision once the link is lost and back", async () => {
    vi.useFakeTimers();
    const engine = await countingRenderEngine();
    const feed = new SurfaceFeed();
    const { result } = renderSession({
      choice: { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface },
      engine,
    });
    const session = sessionOf(result.current);
    const socket = FakeWebSocket.latest();
    await holdMoon(session, socket);
    act(() => {
      socket.close();
    });
    const stale = session.survey(SESSION_MOON)?.stale;
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });
    const next = FakeWebSocket.latest();
    act(() => {
      next.serverWelcomes();
    });
    expect([stale, next === socket, latestFieldAsk(next)]).toEqual([
      true,
      false,
      { id: latestFieldAsk(next).id, body: SESSION_MOON, have: 1 },
    ]);
  });

  it("asks a body's delta alone at a revision rise its scene's topic tells", async () => {
    const engine = await countingRenderEngine();
    const feed = new SurfaceFeed();
    const { result } = renderSession({
      choice: { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface },
      engine,
    });
    const session = sessionOf(result.current);
    const socket = FakeWebSocket.latest();
    await holdMoon(session, socket);
    feed.tell({ kind: "revisions", revisions: [{ body: SESSION_MOON, revision: 2 }] });
    expect(latestFieldAsk(socket)).toMatchObject({ body: SESSION_MOON, have: 1 });
  });

  it("asks every held field again with its revision when its scene is opened again", async () => {
    const engine = await countingRenderEngine();
    const feed = new SurfaceFeed();
    const { result } = renderSession({
      choice: { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface },
      engine,
    });
    const session = sessionOf(result.current);
    const socket = FakeWebSocket.latest();
    await holdMoon(session, socket);
    feed.tell({ kind: "resync" });
    expect([socket.requestsOfKind("surface_field").length, latestFieldAsk(socket).have]).toEqual([
      2, 1,
    ]);
  });

  it("takes a fixture's surface events into its session", async () => {
    const engine = await countingRenderEngine();
    const recorded = recordedFixture();
    const { result } = renderSession({
      choice: { kind: "kept", option: fixtureOption(recorded.fixture) },
      engine,
    });
    const session = sessionOf(result.current);
    session.noteView(PRIMARY, viewAt(5e6));
    session.step();
    // The fixture answers the field at revision 1, as R09.T19.b would.
    const [client] = recorded.clients;
    const ask = recorded.sent.find(
      (message) => message.type === "request" && message.body.kind === "surface_field",
    );
    if (client === undefined || ask?.type !== "request") {
      throw new Error("the fixture was asked for no field");
    }
    const block = fieldBlock(50, 1);
    client.handleBinaryFrame(binaryFrame(ask.id, 0, 1, [...block]));
    const body: ResponseFor<"surface_field"> = readyField(
      moonLikeHeader(SESSION_MOON),
      1,
      fieldCover([0, 2, 30]),
      { chunks: 1, bytes: block.length },
    );
    client.handleServerMessage({ type: "response", id: ask.id, body });
    await settle();
    recorded.feed.tell({ kind: "revisions", revisions: [{ body: SESSION_MOON, revision: 2 }] });
    const asks = recorded.sent.flatMap((message) =>
      message.type === "request" && message.body.kind === "surface_field"
        ? [message.body.have_revision]
        : [],
    );
    expect(asks).toEqual([null, 1]);
  });
});

/** A session stand-in whose steps throw `error` while `throws` holds, its faults counted. */
function throwing(
  error: Error,
  throws: () => boolean = () => true,
): Pick<TerrainSession, "step" | "failComputation"> & { readonly faulted: () => number } {
  let faulted = 0;
  return {
    step: () => {
      if (throws()) {
        throw error;
      }
    },
    failComputation: () => {
      faulted += 1;
    },
    faulted: () => faulted,
  };
}

/** Steps `session` through `guard` once at each of `timesMs`. */
function stepAt(
  session: Pick<TerrainSession, "step" | "failComputation"> | null,
  guard: TerrainFailureGuard,
  timesMs: ReadonlyArray<number>,
): void {
  for (const atMs of timesMs) {
    stepTerrainSession(session, guard, atMs);
  }
}

describe("the loop's guarded step of its session (the batch review's S5; R10.T16.e)", () => {
  it("reports a step's bug once, however many steps fail after it, and keeps the loop going", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const guard = new TerrainFailureGuard();
    stepAt(throwing(new Error("a bug of the session's")), guard, [0, 16, 33]);
    expect([guard.failing, error.mock.calls.length]).toEqual([true, 1]);
  });

  it("faults the session's terrain once its steps have failed for 250 ms, and not before", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const session = throwing(new Error("a bug of the session's"));
    const guard = new TerrainFailureGuard();
    stepAt(session, guard, [1_000, 1_100, 1_000 + ANNUNCIATION_ONSET_MS - 1]);
    const before = session.faulted();
    stepAt(session, guard, [1_000 + ANNUNCIATION_ONSET_MS, 1_300, 1_400]);
    expect([before, session.faulted()]).toEqual([0, 1]);
  });

  it("raises nothing for a single failed step", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    let atMs = 0;
    const session = throwing(new Error("a bug of the session's"), () => atMs === 0);
    const guard = new TerrainFailureGuard();
    for (atMs = 0; atMs <= 1_000; atMs += 16) {
      stepTerrainSession(session, guard, atMs);
    }
    expect([session.faulted(), guard.failing]).toEqual([0, false]);
  });

  it("lets a loss pass in silence, however long it lasts", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const session = throwing(new EngineUnavailable("createBuffer"));
    const guard = new TerrainFailureGuard();
    stepAt(session, guard, [0, 300, 600]);
    expect([guard.failing, session.faulted(), error.mock.calls.length]).toEqual([false, 0, 0]);
  });

  it("clears once a step succeeds, and steps no session where there is none", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const failed = new TerrainFailureGuard();
    stepAt(throwing(new Error("a bug of the session's")), failed, [0]);
    stepAt(
      throwing(new Error("never"), () => false),
      failed,
      [16],
    );
    const none = new TerrainFailureGuard();
    stepAt(null, none, [0, 300]);
    expect([failed.failing, none.failing]).toEqual([false, false]);
  });
});

describe("the terrain's faults for the label blocks (useTerrainFaults; decision-r10-t16-contracts §7)", () => {
  const moon = SESSION_SCENE.bodies.find((body) => body.id === SESSION_MOON);
  const turningMoon = moon === undefined ? undefined : { ...moon, rotation: IDENTITY_ROTATION };
  /** The scene with its moon turning, so that a view can hand it over. */
  const TURNING = {
    ...SESSION_SCENE,
    bodies: SESSION_SCENE.bodies.map((body) =>
      body.id === SESSION_MOON ? (turningMoon ?? body) : body,
    ),
  };

  it("gives no view without a session", () => {
    const { result } = renderHook(() => useTerrainFaults(null));
    expect(result.current.size).toBe(0);
  });

  /** A session holding the moon, which the primary hands over, and the hook's faults of it. */
  async function handingSession(): Promise<{
    readonly session: TerrainSession;
    readonly workers: ScriptedHeightWorker[];
    readonly faults: () => ReadonlyMap<string, string>;
  }> {
    const engine = await countingRenderEngine();
    const feed = new SurfaceFeed();
    const { result, workers } = renderSession({
      choice: { kind: "server", universe: UNIVERSE, onSurface: feed.onSurface },
      engine,
    });
    const session = sessionOf(result.current);
    await holdMoon(session, FakeWebSocket.latest());
    session.noteView(PRIMARY, {
      ...viewAt(5e6, "photorealistic", { scene: TURNING }),
      regimes: new Map([[SESSION_MOON, "terrain"]]),
    });
    session.step();
    const hook = renderHook(() => useTerrainFaults(session));
    return { session, workers, faults: () => hook.result.current };
  }

  it("gives every view that hands a body over once every height worker is lost", async () => {
    const { session, workers, faults } = await handingSession();
    act(() => {
      for (let round = 0; round < MAX_CONSECUTIVE_WORKER_FAILURES; round += 1) {
        for (const worker of workers.slice(-2)) {
          worker.fail();
        }
      }
      session.step();
    });
    expect([...faults()]).toEqual([[PRIMARY, TERRAIN_NOT_AVAILABLE]]);
  });

  it("gives the computation's fault at once when the session's terrain computation is faulted (R10.T16.e)", async () => {
    const { session, faults } = await handingSession();
    act(() => {
      session.failComputation();
    });
    expect([...faults()]).toEqual([[PRIMARY, TERRAIN_COMPUTATION_FAILED]]);
  });
});
