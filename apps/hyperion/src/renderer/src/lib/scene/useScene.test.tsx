import type {
  NotificationBody,
  SceneStateDto,
  ServerMessage,
  UniverseIdHex,
} from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { type ReactNode, StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { FIXTURE_EARTH, FIXTURE_SYSTEM } from "../../test/planetaryFixture";
import {
  craftFixture,
  designateFixture,
  SCENE_TIDAL_RADIUS_M,
  sceneClock,
  shipInSystem,
  sliceSceneSystem,
} from "../../test/sceneFixture";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { viewId } from "../../view/camera/state";
import type { SceneFrame } from "./apparent";
import { CAMERA_REPORT_INTERVAL_MS } from "./cameraReports";
import type { SceneKinematics } from "./model";
import { RECONNECT_DELAY_MS } from "../connection";
import { REQUEST_TIMEOUT_MS } from "../useServerRequest";
import { SCENE_SILENCE_MS, type SceneOptions, useScene } from "./useScene";

const UNIVERSE: UniverseIdHex = "00000000000000a1";
const OTHER_UNIVERSE: UniverseIdHex = "00000000000000b2";
const SOME_UNIVERSE: { readonly universe: UniverseIdHex | null } = { universe: UNIVERSE };

const OPTIONS: SceneOptions = { detail: "full", designate: designateFixture };
const RATE = 1_000;

/** The scene's opening state: the ship 1 AU out in the slice's system, a craft in view. */
function stateInSystem(seconds = 3_000): SceneStateDto {
  return {
    sequence: 0,
    clock: sceneClock(seconds, RATE),
    ship: shipInSystem(seconds),
    system: sliceSceneSystem(),
    tidal_radius_m: SCENE_TIDAL_RADIUS_M,
    craft: [craftFixture()],
  };
}

/** A heartbeat numbered `sequence`, at `seconds`. */
function heartbeat(sequence: number, seconds: number): NotificationBody {
  return { topic: "scene", sequence, clock: sceneClock(seconds, RATE), bodies: [] };
}

/** Lets the outcomes the server's messages settle reach the store and React. */
async function settle(): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** Renders the hook under a welcomed server link. */
function renderScene() {
  const hook = renderHook(() => useScene(UNIVERSE, OPTIONS), { wrapper: ServerLinkHarness });
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { ...hook, socket };
}

/** Answers the latest `subscribe` with subscription `id` opening on `state`. */
async function opens(socket: FakeWebSocket, id: number, state = stateInSystem()): Promise<void> {
  act(() => {
    socket.serverAnswers("subscribe", () => ({
      kind: "subscribe",
      subscription: id,
      state: { topic: "scene", ...state },
    }));
  });
  await settle();
}

/** The server's ending of subscription `subscription`, its topic having failed. */
function ended(subscription: number): ServerMessage {
  return {
    type: "subscription_ended",
    subscription,
    error: { code: "internal", message: "the scene could not be advanced", field: null },
  };
}

/** A push of the fixture's craft, numbered `push`, stating its time `push` seconds on. */
function craftPush(push: number): NotificationBody {
  const craft = craftFixture();
  const time = sceneClock(3_000 + push, RATE).time;
  return {
    topic: "scene",
    sequence: push,
    clock: sceneClock(3_000 + push, RATE),
    bodies: [],
    craft: [{ ...craft, state: { ...craft.state, time } }],
  };
}

function distance(a: Vec3, b: Vec3): number {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  const dz = a.z - b.z;
  return Math.sqrt(dx * dx + dy * dy + dz * dz);
}

/** A body or star of a frame: where it is seen and, unless a contact, where it is. */
interface Source {
  readonly id: string;
  readonly apparentM: Vec3;
  readonly geometricM: Vec3 | null;
}

function sources(frame: SceneFrame): Source[] {
  return [
    ...frame.bodies.map((body) => ({
      id: body.id,
      apparentM: body.apparentM,
      geometricM: body.kind === "placed" ? body.geometricM : null,
    })),
    ...frame.stars.map((star) => ({
      id: star.id,
      apparentM: star.apparentM,
      geometricM: star.geometricM,
    })),
  ];
}

function framed(frame: SceneFrame | null): SceneFrame {
  if (frame === null) {
    throw new Error("the scene gave no frame");
  }
  return frame;
}

/** A camera 10⁹ m from the slice's barycentre, in its frame. */
function camera(x = 1e9): SceneKinematics {
  return {
    position: { kind: "system", system: FIXTURE_SYSTEM, offsetM: { x, y: 0, z: 0 } },
    velocityMPerS: { x: 0, y: 0, z: 0 },
    time: { seconds: 3_000, nanos: 0 },
  };
}

describe("useScene", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("subscribes to the universe's scene once the link is up, with no cameras", () => {
    const { result, socket } = renderScene();

    expect(result.current.status).toEqual({ kind: "pending" });
    expect(socket.requestsOfKind("subscribe").map(({ body }) => body)).toEqual([
      {
        kind: "subscribe",
        universe: UNIVERSE,
        topic: { topic: "scene", detail: "full", cameras: [] },
      },
    ]);
  });

  it("receives the scene and renders it at the pushed time extrapolated at its rate", async () => {
    const { result, socket } = renderScene();
    await opens(socket, 3);

    expect(result.current.status).toEqual({ kind: "live" });
    expect(result.current.model?.system?.tidalRadiusM).toBe(SCENE_TIDAL_RADIUS_M);

    vi.advanceTimersByTime(1_500);
    const frame = framed(result.current.frameAt(performance.now()));

    expect(frame.time).toEqual({ seconds: 3_000 + 1.5 * RATE, nanos: 0 });
    expect(frame.bodies.map(({ id }) => id)).toContain(FIXTURE_EARTH);

    act(() => {
      socket.serverNotifies(3, heartbeat(1, 4_600));
    });
    vi.advanceTimersByTime(200);

    expect(result.current.model?.sequence).toBe(1);
    expect(framed(result.current.frameAt(performance.now())).time).toEqual({
      seconds: 4_600 + 0.2 * RATE,
      nanos: 0,
    });
  });

  it("marks the scene stale when the link drops, holding its clock", async () => {
    const { result, socket } = renderScene();
    await opens(socket, 3);
    vi.advanceTimersByTime(1_000);

    act(() => {
      socket.close();
    });
    const heldAt = framed(result.current.frameAt(performance.now())).time;
    vi.advanceTimersByTime(1_000);

    expect(result.current.status).toEqual({ kind: "stale", reason: "link_down" });
    expect(result.current.model?.sequence).toBe(0);
    expect(heldAt).toEqual({ seconds: 3_000 + RATE, nanos: 0 });
    expect(framed(result.current.frameAt(performance.now())).time).toEqual(heldAt);
  });

  it("subscribes again when the link returns", async () => {
    const { result, socket } = renderScene();
    await opens(socket, 3);
    act(() => {
      socket.close();
    });

    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });
    const next = FakeWebSocket.latest();
    act(() => {
      next.serverWelcomes();
    });

    expect(next).not.toBe(socket);
    expect(next.requestsOfKind("subscribe")).toHaveLength(1);

    await opens(next, 1, stateInSystem(9_000));

    expect(result.current.status).toEqual({ kind: "live" });
    expect(result.current.model?.clock.time).toEqual({ seconds: 9_000, nanos: 0 });
  });

  it("subscribes again after a gap in the pushes' numbering", async () => {
    const { result, socket } = renderScene();
    await opens(socket, 3);
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);

    act(() => {
      socket.serverNotifies(3, heartbeat(2, 4_000));
    });

    expect(errors).toHaveBeenCalledOnce();
    expect(result.current.status).toEqual({ kind: "stale", reason: "resubscribing" });
    expect(socket.requestsOfKind("unsubscribe").map(({ body }) => body.subscription)).toEqual([3]);
    expect(socket.requestsOfKind("subscribe")).toHaveLength(2);

    await opens(socket, 4, stateInSystem(4_000));

    expect(result.current.status).toEqual({ kind: "live" });
    expect(result.current.model?.clock.time.seconds).toBe(4_000);
  });

  it("reopens the scene the server ended, after the link's reconnection delay", async () => {
    const { result, socket } = renderScene();
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    await opens(socket, 3);

    act(() => {
      socket.serverSends({
        type: "subscription_ended",
        subscription: 3,
        error: { code: "internal", message: "the scene could not be advanced", field: null },
      });
    });

    expect(result.current.status).toEqual({ kind: "stale", reason: "resubscribing" });
    expect(errors).toHaveBeenCalledOnce();
    expect(socket.requestsOfKind("subscribe")).toHaveLength(1);
    expect(socket.requestsOfKind("unsubscribe")).toHaveLength(0);

    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });

    expect(socket.requestsOfKind("subscribe")).toHaveLength(2);

    await opens(socket, 4);

    expect(result.current.status).toEqual({ kind: "live" });
  });

  it("waits out the delay as resubscribing, however long, then opens the scene again", async () => {
    const { result, socket } = renderScene();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    await opens(socket, 3);
    act(() => {
      socket.serverSends(ended(3));
    });

    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS - 1);
    });

    expect(result.current.status).toEqual({ kind: "stale", reason: "resubscribing" });
    expect(socket.requestsOfKind("subscribe")).toHaveLength(1);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    act(() => {
      vi.advanceTimersByTime(SCENE_SILENCE_MS * 2);
    });

    expect(socket.requestsOfKind("subscribe")).toHaveLength(2);
    expect(result.current.status).toEqual({ kind: "stale", reason: "resubscribing" });
  });

  it("does not reopen a scene the server ended once the link has dropped meanwhile", async () => {
    const { result, socket } = renderScene();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    await opens(socket, 3);
    act(() => {
      socket.serverSends(ended(3));
    });

    act(() => {
      socket.close();
    });
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS - 1);
    });

    expect(socket.requestsOfKind("subscribe")).toHaveLength(1);
    expect(result.current.status).toEqual({ kind: "stale", reason: "link_down" });
  });

  it("does not reopen the old universe's scene after the universe changes", async () => {
    const { rerender } = renderHook(
      ({ universe }: { readonly universe: UniverseIdHex | null }) => useScene(universe, OPTIONS),
      { initialProps: SOME_UNIVERSE, wrapper: ServerLinkHarness },
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    await opens(socket, 3);
    act(() => {
      socket.serverSends(ended(3));
    });

    rerender({ universe: null });
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS * 2);
    });

    expect(socket.requestsOfKind("subscribe")).toHaveLength(1);
  });

  it("gives up with the server's code after it ends the scene four times running", async () => {
    const { result, socket } = renderScene();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    await opens(socket, 1);
    const endedThenOpened = async (subscription: number): Promise<void> => {
      act(() => {
        socket.serverSends(ended(subscription));
      });
      act(() => {
        vi.advanceTimersByTime(RECONNECT_DELAY_MS);
      });
      await opens(socket, subscription + 1);
    };
    await endedThenOpened(1);
    await endedThenOpened(2);
    await endedThenOpened(3);

    act(() => {
      socket.serverSends(ended(4));
    });
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });

    expect(socket.requestsOfKind("subscribe")).toHaveLength(4);
    expect(result.current.status).toEqual({
      kind: "rejected",
      code: "internal",
      reason: "the server ended the scene (internal: the scene could not be advanced)",
    });
  });

  it("shows the scene stale after two seconds without a push, and live at the next", async () => {
    const { result, socket } = renderScene();
    await opens(socket, 3);

    act(() => {
      vi.advanceTimersByTime(SCENE_SILENCE_MS - 1);
    });

    expect(result.current.status).toEqual({ kind: "live" });

    act(() => {
      vi.advanceTimersByTime(1);
    });
    const heldAt = framed(result.current.frameAt(performance.now())).time;
    act(() => {
      vi.advanceTimersByTime(500);
    });

    expect(result.current.status).toEqual({ kind: "stale", reason: "silent" });
    expect(framed(result.current.frameAt(performance.now())).time).toEqual(heldAt);

    act(() => {
      socket.serverNotifies(3, heartbeat(1, 5_600));
    });

    expect(result.current.status).toEqual({ kind: "live" });
  });

  it("reports the scene's refusal of the subscription", async () => {
    const { result, socket } = renderScene();
    const [subscribe] = socket.requestsOfKind("subscribe");
    if (subscribe === undefined) {
      throw new Error("the hook subscribes");
    }

    act(() => {
      socket.serverRejects(subscribe.id, {
        code: "unknown_universe",
        message: "no such universe",
        field: null,
      });
    });
    await settle();

    expect(result.current.status).toEqual({
      kind: "rejected",
      code: "unknown_universe",
      reason: "no such universe",
    });
  });

  it("sends the cameras held once the subscription opens, then at most at 4 Hz", async () => {
    const { result, socket } = renderScene();
    act(() => {
      result.current.reportCamera(viewId("forward"), camera());
    });

    expect(socket.requestsOfKind("scene_cameras")).toHaveLength(0);

    await opens(socket, 3);

    expect(socket.requestsOfKind("scene_cameras").map(({ body }) => body)).toEqual([
      {
        kind: "scene_cameras",
        subscription: 3,
        cameras: [
          {
            view: 0,
            pose: {
              position: { frame: "system", system: FIXTURE_SYSTEM, offset_m: [1e9, 0, 0] },
              velocity_m_s: [0, 0, 0],
              time: { seconds: 3_000, nanos: 0 },
            },
          },
        ],
      },
    ]);

    act(() => {
      result.current.reportCamera(viewId("forward"), camera(2e9));
      result.current.reportCamera(viewId("forward"), camera(3e9));
    });

    expect(socket.requestsOfKind("scene_cameras")).toHaveLength(1);

    act(() => {
      vi.advanceTimersByTime(CAMERA_REPORT_INTERVAL_MS);
    });

    // The report in flight is answered before the next goes out.
    expect(socket.requestsOfKind("scene_cameras")).toHaveLength(1);

    act(() => {
      socket.serverAnswers("scene_cameras", () => ({ kind: "scene_cameras" }));
    });
    await settle();

    expect(socket.requestsOfKind("scene_cameras")).toHaveLength(2);
    expect(socket.requestsOfKind("scene_cameras").at(-1)?.body.cameras[0]?.pose.position).toEqual({
      frame: "system",
      system: FIXTURE_SYSTEM,
      offset_m: [3e9, 0, 0],
    });
  });

  it("says why the server refused a camera report", async () => {
    const { result, socket } = renderScene();
    await opens(socket, 3);
    act(() => {
      result.current.reportCamera(viewId("forward"), camera());
    });
    const [report] = socket.requestsOfKind("scene_cameras");
    if (report === undefined) {
      throw new Error("the camera is reported");
    }

    act(() => {
      socket.serverRejects(report.id, {
        code: "bad_request",
        message: "a camera is outside the scene's reach",
        field: "cameras",
      });
    });
    await settle();

    expect(result.current.cameraRefusal).toBe("a camera is outside the scene's reach");
  });

  it("places every body within v × 20 ms × rate across two clients fed 20 ms apart", async () => {
    const first = renderScene();
    const second = renderScene();
    await opens(first.socket, 1);
    vi.advanceTimersByTime(20);
    await opens(second.socket, 1);
    act(() => {
      first.socket.serverNotifies(1, heartbeat(1, 3_100));
    });
    vi.advanceTimersByTime(20);
    act(() => {
      second.socket.serverNotifies(1, heartbeat(1, 3_100));
    });
    vi.advanceTimersByTime(500);

    const now = performance.now();
    const a = framed(first.result.current.frameAt(now));
    const b = framed(second.result.current.frameAt(now));
    // Each source's own speed, apparent and geometric, in m per scene second, from the first
    // client's frame a real millisecond on.
    const later = framed(first.result.current.frameAt(now + 1));
    const stepS = 0.001 * RATE;
    const deliveryS = 0.02 * RATE;
    // 1% and a metre cover a millisecond chord's estimate of the speed and the rounding.
    const bound = (speedMPerS: number): number => 1.01 * speedMPerS * deliveryS + 1;
    const ofB = new Map(sources(b).map((source) => [source.id, source]));
    const ofLater = new Map(sources(later).map((source) => [source.id, source]));
    let largestShare = 0;
    for (const source of sources(a)) {
      const other = ofB.get(source.id);
      const moved = ofLater.get(source.id);
      if (other === undefined || moved === undefined) {
        throw new Error(`both clients and both frames place ${source.id}`);
      }
      const apparentBound = bound(distance(source.apparentM, moved.apparentM) / stepS);
      const apart = distance(source.apparentM, other.apparentM);
      expect(apart).toBeLessThanOrEqual(apparentBound);
      largestShare = Math.max(largestShare, apart / apparentBound);
      // A contact has no geometric position, on either client.
      const here = source.geometricM ?? source.apparentM;
      const there = other.geometricM ?? other.apparentM;
      const on = moved.geometricM ?? moved.apparentM;
      expect(distance(here, there)).toBeLessThanOrEqual(bound(distance(here, on) / stepS));
    }

    expect(a.bodies.length).toBeGreaterThan(0);
    expect(a.stars.length).toBeGreaterThan(0);
    expect(ofB.size).toBe(sources(a).length);
    // The two clients do differ, by the 20 ms of delivery: the test is not vacuous.
    expect(largestShare).toBeGreaterThan(0.5);
  });

  it("holds, in the second of two clients fed 20 ms apart, the craft the first held 20 ms before", async () => {
    const first = renderScene();
    const second = renderScene();
    await opens(first.socket, 1);
    await opens(second.socket, 1);
    const deliveryMs = 20;
    // A whole-millisecond stand-in for the 15.625 ms craft tick.
    const pushMs = 16;
    const pushes = 12;
    const startMs = performance.now();
    const deliveries: Array<{ atMs: number; socket: FakeWebSocket; push: number }> = [];
    for (let push = 1; push <= pushes; push += 1) {
      deliveries.push({ atMs: startMs + push * pushMs, socket: first.socket, push });
      deliveries.push({ atMs: startMs + push * pushMs + deliveryMs, socket: second.socket, push });
    }
    deliveries.sort((x, y) => x.atMs - y.atMs);

    // The first client's craft after each push, with when it got them.
    const heldByFirst = [{ atMs: startMs, craft: first.result.current.model?.craft }];
    let next = 0;
    const endMs = startMs + pushes * pushMs + 2 * deliveryMs;
    for (let atMs = startMs + 1; atMs <= endMs; atMs += 1) {
      vi.advanceTimersByTime(1);
      for (
        let due = deliveries[next];
        due !== undefined && due.atMs <= atMs;
        due = deliveries[next]
      ) {
        next += 1;
        const { socket, push } = due;
        act(() => {
          socket.serverNotifies(1, craftPush(push));
        });
        if (socket === first.socket) {
          heldByFirst.push({ atMs, craft: first.result.current.model?.craft });
        }
      }
      const then = heldByFirst.findLast((held) => held.atMs <= atMs - deliveryMs) ?? heldByFirst[0];
      expect(second.result.current.model?.craft).toEqual(then?.craft);
    }

    expect(next).toBe(deliveries.length);
    expect(heldByFirst).toHaveLength(pushes + 1);
    expect(second.result.current.model?.craft).toEqual(first.result.current.model?.craft);
  });

  it("unsubscribes and goes idle when no universe is asked for", async () => {
    const { rerender, result } = renderHook(
      ({ universe }: { readonly universe: UniverseIdHex | null }) => useScene(universe, OPTIONS),
      {
        initialProps: SOME_UNIVERSE,
        wrapper: ServerLinkHarness,
      },
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    await opens(socket, 3);

    rerender({ universe: null });

    expect(socket.requestsOfKind("unsubscribe").map(({ body }) => body.subscription)).toEqual([3]);
    expect(result.current.status).toEqual({ kind: "idle" });
    expect(result.current.frameAt(performance.now())).toBeNull();
  });

  it("opens the new universe's scene and ends the old one when the universe changes", async () => {
    const { rerender, result } = renderHook(
      ({ universe }: { readonly universe: UniverseIdHex }) => useScene(universe, OPTIONS),
      { initialProps: { universe: UNIVERSE }, wrapper: ServerLinkHarness },
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    await opens(socket, 3);

    rerender({ universe: OTHER_UNIVERSE });

    expect(socket.requestsOfKind("unsubscribe").map(({ body }) => body.subscription)).toEqual([3]);
    expect(socket.requestsOfKind("subscribe").at(-1)?.body.universe).toBe(OTHER_UNIVERSE);
    expect(result.current.status).toEqual({ kind: "pending" });
    expect(result.current.model).toBeNull();
  });

  it("reports a subscription unanswered in time as timed out", () => {
    const { result } = renderScene();

    act(() => {
      vi.advanceTimersByTime(REQUEST_TIMEOUT_MS);
    });

    expect(result.current.status).toEqual({ kind: "timed_out" });
  });

  it("refuses an opening state it cannot read", async () => {
    const { result, socket } = renderScene();
    const state = stateInSystem();

    await opens(socket, 3, { ...state, clock: { ...state.clock, time_rate: 7 } });

    expect(result.current.status).toEqual({
      kind: "rejected",
      code: "unusable",
      reason: "clock rate unusable",
    });
    expect(socket.requestsOfKind("unsubscribe").map(({ body }) => body.subscription)).toEqual([3]);
  });

  it("subscribes again after a push it cannot use, and gives up after three in a row", async () => {
    const { result, socket } = renderScene();
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    await opens(socket, 1);
    const unusable: NotificationBody = {
      ...heartbeat(1, 3_100),
      clock: { ...sceneClock(3_100, RATE), time_rate: 7 },
    };

    // Pushes an unusable notification on `subscription`, and opens the one that follows.
    const refusedThenOpened = async (subscription: number): Promise<void> => {
      act(() => {
        socket.serverNotifies(subscription, unusable);
      });

      expect(result.current.status).toEqual({ kind: "stale", reason: "resubscribing" });
      expect(socket.requestsOfKind("subscribe")).toHaveLength(subscription + 1);

      await opens(socket, subscription + 1);
    };
    await refusedThenOpened(1);
    await refusedThenOpened(2);
    await refusedThenOpened(3);
    act(() => {
      socket.serverNotifies(4, unusable);
    });

    expect(socket.requestsOfKind("subscribe")).toHaveLength(4);
    expect(result.current.status).toMatchObject({ kind: "rejected", code: "unusable" });
    expect(errors).toHaveBeenCalledTimes(4);
  });

  it("sends the cameras held again on the subscription opened after a resubscription", async () => {
    const { result, socket } = renderScene();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    await opens(socket, 3);
    act(() => {
      result.current.reportCamera(viewId("forward"), camera());
    });
    const [report] = socket.requestsOfKind("scene_cameras");
    if (report === undefined) {
      throw new Error("the camera is reported");
    }
    act(() => {
      socket.serverResponds(report.id, { kind: "scene_cameras" });
    });
    await settle();

    act(() => {
      socket.serverNotifies(3, heartbeat(5, 3_100));
    });
    await opens(socket, 4);

    expect(socket.requestsOfKind("scene_cameras").map(({ body }) => body.subscription)).toEqual([
      3, 4,
    ]);
  });

  it("keeps one camera report in flight, and shows a refusal answered after a newer pose", async () => {
    const { result, socket } = renderScene();
    await opens(socket, 3);
    act(() => {
      result.current.reportCamera(viewId("forward"), camera(1e9));
    });
    act(() => {
      vi.advanceTimersByTime(CAMERA_REPORT_INTERVAL_MS);
      result.current.reportCamera(viewId("forward"), camera(2e9));
      vi.advanceTimersByTime(CAMERA_REPORT_INTERVAL_MS);
    });
    const [refused] = socket.requestsOfKind("scene_cameras");
    if (refused === undefined) {
      throw new Error("the camera is reported");
    }

    expect(socket.requestsOfKind("scene_cameras")).toHaveLength(1);

    act(() => {
      socket.serverRejects(refused.id, {
        code: "bad_request",
        message: "a camera is outside the scene's reach",
        field: "cameras",
      });
    });
    await settle();

    expect(result.current.cameraRefusal).toBe("a camera is outside the scene's reach");
    expect(socket.requestsOfKind("scene_cameras").at(-1)?.body.cameras[0]?.pose.position).toEqual({
      frame: "system",
      system: FIXTURE_SYSTEM,
      offset_m: [2e9, 0, 0],
    });
  });

  it("does not re-render its caller for an accepted camera report", async () => {
    let renders = 0;
    const hook = renderHook(
      () => {
        renders += 1;
        return useScene(UNIVERSE, OPTIONS);
      },
      { wrapper: ServerLinkHarness },
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    await opens(socket, 3);
    act(() => {
      hook.result.current.reportCamera(viewId("forward"), camera());
    });
    const before = renders;

    act(() => {
      socket.serverAnswers("scene_cameras", () => ({ kind: "scene_cameras" }));
    });
    await settle();

    expect(renders).toBe(before);
  });

  it("subscribes once under StrictMode's double effects, and receives the scene", async () => {
    const hook = renderHook(() => useScene(UNIVERSE, OPTIONS), {
      wrapper: ({ children }: { readonly children: ReactNode }) => (
        <StrictMode>
          <ServerLinkHarness>{children}</ServerLinkHarness>
        </StrictMode>
      ),
    });
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    await opens(socket, 3);

    expect(hook.result.current.status).toEqual({ kind: "live" });
    expect(socket.requestsOfKind("subscribe")).toHaveLength(1);
  });
});
