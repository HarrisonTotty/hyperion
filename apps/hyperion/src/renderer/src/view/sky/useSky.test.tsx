import type { SkyLayerCensusDto, SkyRequest } from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { RECONNECT_DELAY_MS } from "../../lib/connection";
import { binaryFrame } from "../../test/binaryFrames";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  type FixtureStar,
  nearestFirstCensus,
  skyPayload,
  skyRequest,
  skyResponse,
} from "../../test/skyFixtures";
import { galacticTranslated } from "../coords/position";
import { decodeSkyPayload } from "./decodePayload";
import type { SkyCamera } from "./model";
import { type SkyDecoder, useSky } from "./useSky";

const CAMERAS: ReadonlyArray<SkyCamera> = [
  { position: skyRequest(0).observer, fovDeg: 60, widthPx: 1_920 },
];

const STARS = [{ direction: [1, 0, 0], distanceLy: 20, vMag: -1.46 }] as const;

/** A decoder on the test's own thread, as the worker's is, made once so its identity holds. */
function inThreadDecoder(): SkyDecoder {
  return {
    decode: (job) => Promise.resolve(decodeSkyPayload({ ...job, id: 1 })),
    dispose: () => undefined,
  };
}

interface SkyProps {
  readonly request: SkyRequest | null;
  readonly cameras: ReadonlyArray<SkyCamera>;
}

function renderSky(initial: SkyRequest | null) {
  const hook = renderHook(({ request, cameras }: SkyProps) => useSky(request, cameras, OPTIONS), {
    initialProps: { request: initial, cameras: CAMERAS },
    wrapper: ServerLinkHarness,
  });
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { ...hook, socket };
}

const OPTIONS = { createDecoder: inThreadDecoder };

/** The hook's props at `years`, with the default camera. */
function at(years: number, system?: string): SkyProps {
  return { request: skyRequest(years, system), cameras: CAMERAS };
}

/** Plays the server's answer to the latest sky request: its payload's frames, then its response. */
async function answerSky(socket: FakeWebSocket): Promise<void> {
  const sent = socket.requestsOfKind("sky").at(-1);
  if (sent === undefined) {
    throw new Error("no sky was asked");
  }
  const payload = skyPayload(STARS, 2, 6.6);
  await act(async () => {
    socket.serverSendsBinary(binaryFrame(sent.id, 0, 1, [...payload]));
    socket.serverResponds(sent.id, {
      kind: "sky",
      ...skyResponse(sent.body, payload, STARS.length, 2),
    });
    await Promise.resolve();
    await Promise.resolve();
  });
}

/** The latest sky request the socket carried. */
function latestSky(socket: FakeWebSocket): { readonly id: number; readonly body: SkyRequest } {
  const sent = socket.requestsOfKind("sky").at(-1);
  if (sent === undefined) {
    throw new Error("no sky was asked");
  }
  return sent;
}

/**
 * Plays one reply to sky `id` of `stars`: its payload's frame, then its message, its census `census`
 * (none by default).
 */
async function reply(
  socket: FakeWebSocket,
  id: number,
  stars: ReadonlyArray<FixtureStar>,
  final: boolean,
  census: ReadonlyArray<SkyLayerCensusDto> = [],
): Promise<void> {
  const request = latestSky(socket).body;
  const payload = skyPayload(stars, 2, 6.6);
  const body = {
    kind: "sky" as const,
    ...skyResponse(request, payload, stars.length, 2),
    census: [...census],
    final,
  };
  await act(async () => {
    socket.serverSendsBinary(binaryFrame(id, 0, 1, [...payload]));
    if (final) {
      socket.serverResponds(id, body);
    } else {
      socket.serverAnswersInPart(id, body);
    }
    await Promise.resolve();
    await Promise.resolve();
  });
}

/** Plays a reply before the last to sky `id` (R06.T11.d). */
async function answerInPart(
  socket: FakeWebSocket,
  id: number,
  stars: ReadonlyArray<FixtureStar>,
): Promise<void> {
  await reply(socket, id, stars, false);
}

/** Plays the last reply to sky `id`. */
async function answerSkyWith(
  socket: FakeWebSocket,
  id: number,
  stars: ReadonlyArray<FixtureStar>,
): Promise<void> {
  await reply(socket, id, stars, true);
}

/** Layers C, D and E's edges in a reply near the Sun, each final where `null` (R06.T11.d). */
type Edges = readonly [number | null, number | null, number | null];

/** The edges of a final reply. */
const FINAL: Edges = [null, null, null];

/**
 * Plays a reply to sky `id` of `count` stars 20 ly away whose census reaches `edgesLy`: final
 * where every layer is.
 */
async function replyAt(
  socket: FakeWebSocket,
  id: number,
  count: number,
  edgesLy: Edges,
): Promise<void> {
  const stars = Array.from({ length: count }, () => STARS[0]);
  const final = edgesLy.every((edge) => edge === null);
  await reply(socket, id, stars, final, nearestFirstCensus(edgesLy));
}

/** One pixel's angle at the default camera, rad: 60° over 1,920 px. */
const PIXEL_RAD = Math.PI / 3 / 1_920;

const LIGHT_YEAR_M = 9_460_730_472_580_800;

/** The default camera moved so that the fixture's star 20 ly away shifts by `px` pixels. */
function shiftedBy(px: number): SkyCamera {
  return {
    position: galacticTranslated(
      skyRequest(0).observer,
      vec3(px * PIXEL_RAD * 20 * LIGHT_YEAR_M, 0, 0),
    ),
    fovDeg: 60,
    widthPx: 1_920,
  };
}

/** The hook's props for a camera moved to `camera`, the sky asked from where it is. */
function from(camera: SkyCamera, years = 0): SkyProps {
  return { request: { ...skyRequest(years), observer: camera.position }, cameras: [camera] };
}

describe("useSky", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("asks for the sky on arrival and holds it once decoded", async () => {
    const { result, socket } = renderSky(skyRequest(0));
    expect(socket.requestsOfKind("sky")).toHaveLength(1);
    expect(result.current.pending).toBe(true);
    await answerSky(socket);
    expect(result.current.pending).toBe(false);
    expect(result.current.model?.stars.count).toBe(1);
    expect(result.current.model?.band.count).toBe(24);
    expect(result.current.model?.stale).toBe(false);
  });

  it("starts no decoder for a request that is never answered, or cancelled before its answer", () => {
    let made = 0;
    const counting = (): SkyDecoder => {
      made += 1;
      return inThreadDecoder();
    };
    const hook = renderHook(
      ({ request }: { readonly request: SkyRequest }) =>
        useSky(request, CAMERAS, { createDecoder: counting }),
      { initialProps: { request: skyRequest(0) }, wrapper: ServerLinkHarness },
    );
    act(() => {
      FakeWebSocket.latest().serverWelcomes();
    });
    hook.rerender({ request: skyRequest(0, "0200080020000001") });
    hook.unmount();
    expect(made).toBe(0);
  });

  it("asks nothing where no request can be made", () => {
    const { socket } = renderSky(null);
    expect(socket.requestsOfKind("sky")).toHaveLength(0);
  });

  it("holds the sky as time passes within its validity, and asks again past it", async () => {
    const { rerender, socket } = renderSky(skyRequest(0));
    await answerSky(socket);
    rerender(at(0.5));
    expect(socket.requestsOfKind("sky")).toHaveLength(1);
    rerender(at(1.5));
    expect(socket.requestsOfKind("sky")).toHaveLength(2);
  });

  it("cancels a request in flight that another arrival supersedes", () => {
    const { rerender, socket } = renderSky(skyRequest(0));
    const first = socket.requestsOfKind("sky")[0];
    rerender(at(0, "0200080020000001"));
    expect(socket.cancelledIds()).toEqual([first?.id]);
    expect(socket.requestsOfKind("sky")).toHaveLength(2);
  });

  it("keeps the sky through a lost link, marked stale", async () => {
    const { result, socket } = renderSky(skyRequest(0));
    await answerSky(socket);
    act(() => {
      socket.close();
    });
    expect(result.current.model?.stale).toBe(true);
    expect(result.current.model?.stars.count).toBe(1);
  });

  it("asks again once the link returns when it cut a request off", async () => {
    vi.useFakeTimers();
    const { result, socket } = renderSky(skyRequest(0));
    expect(socket.requestsOfKind("sky")).toHaveLength(1);
    await act(async () => {
      socket.close();
      await Promise.resolve();
    });
    expect(result.current.failure).toBeNull();
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });
    const next = FakeWebSocket.latest();
    expect(next).not.toBe(socket);
    act(() => {
      next.serverWelcomes();
    });
    expect(next.requestsOfKind("sky")).toHaveLength(1);
  });

  it("asks once for a camera's move that shifts the nearest baked star past the rule", async () => {
    const { rerender, socket } = renderSky(skyRequest(0));
    await answerSky(socket);
    const moved: SkyCamera = {
      position: galacticTranslated(skyRequest(0).observer, vec3(1e15, 0, 0)),
      fovDeg: 60,
      widthPx: 1_920,
    };
    rerender({ request: skyRequest(0), cameras: [moved] });
    rerender({ request: skyRequest(0), cameras: [moved] });
    expect(socket.requestsOfKind("sky")).toHaveLength(2);
  });

  it("holds each reply of a sky arriving nearest first whole, in place of the one before", async () => {
    const { result, socket } = renderSky(skyRequest(0));
    const seen: Array<number | null> = [];
    const record = (): void => {
      seen.push(result.current.model?.stars.count ?? null);
    };
    const sent = latestSky(socket);
    await answerInPart(socket, sent.id, STARS.slice(0, 1));
    record();
    expect(result.current.pending).toBe(false);
    expect(result.current.model?.response.final).toBe(false);
    await answerInPart(socket, sent.id, [...STARS, ...STARS]);
    record();
    // The request is still in flight: no other is asked while its replies arrive.
    expect(socket.requestsOfKind("sky")).toHaveLength(1);
    await answerSkyWith(socket, sent.id, [...STARS, ...STARS, ...STARS]);
    record();
    expect(seen).toEqual([1, 2, 3]);
    expect(result.current.model?.response.final).toBe(true);
    expect(result.current.pending).toBe(false);
  });

  it("passes over a partial reply still waiting for a later one, and always holds the final", async () => {
    let decodes = 0;
    let release: (() => void) | null = null;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    // The first decode waits for the test; the rest decode at once.
    const slow: SkyDecoder = {
      decode: async (job) => {
        decodes += 1;
        if (decodes === 1) {
          await held;
        }
        return decodeSkyPayload({ ...job, id: decodes });
      },
      dispose: () => undefined,
    };
    // Stable in identity, as `useSky` asks of its options.
    const options = { createDecoder: () => slow };
    const hook = renderHook(
      ({ request }: { readonly request: SkyRequest }) => useSky(request, CAMERAS, options),
      {
        initialProps: { request: skyRequest(0) },
        wrapper: ServerLinkHarness,
      },
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    const sent = latestSky(socket);
    await answerInPart(socket, sent.id, STARS.slice(0, 1));
    await answerInPart(socket, sent.id, [...STARS, ...STARS]);
    await answerSkyWith(socket, sent.id, [...STARS, ...STARS, ...STARS]);
    expect(hook.result.current.model).toBeNull();
    await act(async () => {
      release?.();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(decodes).toBe(2);
    expect(hook.result.current.model?.stars.count).toBe(3);
    expect(hook.result.current.model?.response.final).toBe(true);
  });

  it("keeps the reply held while the next one decodes, then swaps it in whole", async () => {
    let decodes = 0;
    let release: (() => void) | null = null;
    const second = new Promise<void>((resolve) => {
      release = resolve;
    });
    // The second decode waits for the test; the others decode at once.
    const gated: SkyDecoder = {
      decode: async (job) => {
        decodes += 1;
        if (decodes === 2) {
          await second;
        }
        return decodeSkyPayload({ ...job, id: decodes });
      },
      dispose: () => undefined,
    };
    // Stable in identity, as `useSky` asks of its options.
    const options = { createDecoder: () => gated };
    const hook = renderHook(
      ({ request }: { readonly request: SkyRequest }) => useSky(request, CAMERAS, options),
      { initialProps: { request: skyRequest(0) }, wrapper: ServerLinkHarness },
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    const sent = latestSky(socket);
    await answerInPart(socket, sent.id, STARS.slice(0, 1));
    expect(hook.result.current.model?.stars.count).toBe(1);
    await answerInPart(socket, sent.id, [...STARS, ...STARS]);
    // The second reply is decoding: the first stays held, whole, and the sky is not pending.
    expect(decodes).toBe(2);
    expect(hook.result.current.model?.stars.count).toBe(1);
    expect(hook.result.current.pending).toBe(false);
    await act(async () => {
      release?.();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(hook.result.current.model?.stars.count).toBe(2);
  });

  it("asks again once the link returns when it cut a sky off before its final reply", async () => {
    vi.useFakeTimers();
    const { result, socket } = renderSky(skyRequest(0));
    await answerInPart(socket, latestSky(socket).id, STARS.slice(0, 1));
    await act(async () => {
      socket.close();
      await Promise.resolve();
    });
    expect(result.current.model?.stale).toBe(true);
    expect(result.current.model?.stars.count).toBe(1);
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });
    const next = FakeWebSocket.latest();
    act(() => {
      next.serverWelcomes();
    });
    expect(next.requestsOfKind("sky")).toHaveLength(1);
    expect(result.current.model?.stars.count).toBe(1);
  });

  it("holds a refusal without asking again until the next arrival", async () => {
    const { result, rerender, socket } = renderSky(skyRequest(0));
    const sent = socket.requestsOfKind("sky")[0];
    if (sent === undefined) {
      throw new Error("no sky was asked");
    }
    await act(async () => {
      socket.serverRejects(sent.id, {
        code: "bad_request",
        message: "n_max too large",
        field: null,
      });
      await Promise.resolve();
    });
    expect(result.current.failure).toBe("n_max too large");
    rerender(at(0.1));
    expect(socket.requestsOfKind("sky")).toHaveLength(1);
    rerender(at(0.1, "0200080020000001"));
    expect(socket.requestsOfKind("sky")).toHaveLength(2);
    expect(result.current.failure).toBeNull();
  });

  describe("a sky asked again within its arrival (R06.T12.b)", () => {
    /** What a view has of the sky after each reply: its stars, whether final, and pending. */
    type Seen = readonly [number | null, boolean | null, boolean];

    /** Renders the hook and holds a final sky of three stars 20 ly away. */
    async function heldFinal(initial: SkyRequest = skyRequest(0)) {
      const hook = renderSky(initial);
      await replyAt(hook.socket, latestSky(hook.socket).id, 3, FINAL);
      const seen: Seen[] = [];
      const record = (): void => {
        const { model, pending } = hook.result.current;
        seen.push([model?.stars.count ?? null, model?.response.final ?? null, pending]);
      };
      return { ...hook, seen, record };
    }

    it("keeps a final sky re-asked for parallax through the partial replies, then holds the final", async () => {
      const { rerender, result, socket, seen, record } = await heldFinal();
      const camera = shiftedBy(0.5);
      rerender(from(camera));
      expect(socket.requestsOfKind("sky")).toHaveLength(2);
      const again = latestSky(socket).id;
      record();
      await replyAt(socket, again, 1, [125, 125, 125]);
      record();
      await replyAt(socket, again, 2, [1_000, 1_000, 1_000]);
      record();
      await replyAt(socket, again, 4, FINAL);
      record();
      // No reply of fewer stars than the held sky's three is ever held, and it is never pending.
      expect(seen).toEqual([
        [3, true, false],
        [3, true, false],
        [3, true, false],
        [4, true, false],
      ]);
      expect(result.current.model?.request.observer).toEqual(camera.position);
      // Asked from where the camera is, the final sky stands: nothing more is asked.
      expect(socket.requestsOfKind("sky")).toHaveLength(2);
    });

    it("keeps a final sky re-asked for a deeper camera limit through the partial replies", async () => {
      const { rerender, socket, seen, record } = await heldFinal();
      rerender({ request: { ...skyRequest(0), camera_limit_v: 10.5 }, cameras: CAMERAS });
      expect(latestSky(socket).body.camera_limit_v).toBe(10.5);
      const again = latestSky(socket).id;
      await replyAt(socket, again, 1, [125, 125, 125]);
      record();
      await replyAt(socket, again, 2, [500, 500, 500]);
      record();
      await replyAt(socket, again, 5, FINAL);
      record();
      expect(seen).toEqual([
        [3, true, false],
        [3, true, false],
        [5, true, false],
      ]);
    });

    it("holds the newest reply once the held sky's parallax reaches a pixel, and each later one", async () => {
      const { rerender, socket, seen, record } = await heldFinal();
      const camera = shiftedBy(0.5);
      rerender(from(camera));
      const again = latestSky(socket).id;
      await replyAt(socket, again, 1, [125, 125, 125]);
      record();
      // The camera moves on while the request runs: no new request, but the held sky gives way.
      rerender({ ...from(camera), cameras: [shiftedBy(1.01)] });
      record();
      await replyAt(socket, again, 2, [250, 250, 250]);
      record();
      expect(seen).toEqual([
        [3, true, false],
        [1, false, false],
        [2, false, false],
      ]);
      expect(socket.requestsOfKind("sky")).toHaveLength(2);
    });

    it.each([
      { reason: "expired", held: 0, now: 1.5 },
      { reason: "jump", held: 10, now: 9 },
    ])("holds each reply of a sky re-asked as $reason", async ({ held, now }) => {
      const { rerender, socket, seen, record } = await heldFinal(skyRequest(held));
      rerender(at(now));
      expect(socket.requestsOfKind("sky")).toHaveLength(2);
      const again = latestSky(socket).id;
      record();
      await replyAt(socket, again, 1, [125, 125, 125]);
      record();
      await replyAt(socket, again, 2, [250, 250, 250]);
      record();
      // The held sky stands for another time, so each reply is held; the sky is never pending.
      expect(seen).toEqual([
        [3, true, false],
        [1, false, false],
        [2, false, false],
      ]);
    });

    it("gives a partial sky left by a lost link up to the first reply that reaches it, before the final", async () => {
      vi.useFakeTimers();
      const { result, socket } = renderSky(skyRequest(0));
      await replyAt(socket, latestSky(socket).id, 3, [500, 500, 500]);
      await act(async () => {
        socket.close();
        await Promise.resolve();
      });
      act(() => {
        vi.advanceTimersByTime(RECONNECT_DELAY_MS);
      });
      const next = FakeWebSocket.latest();
      act(() => {
        next.serverWelcomes();
      });
      expect(next.requestsOfKind("sky")).toHaveLength(1);
      const again = latestSky(next).id;
      const seen: Array<readonly [number | null, number | null, boolean]> = [];
      const record = (): void => {
        const { model, pending } = result.current;
        const c = model?.response.census.find((layer) => layer.layer === "c");
        seen.push([model?.stars.count ?? null, c?.complete_to_ly ?? null, pending]);
      };
      record();
      await replyAt(next, again, 1, [125, 125, 125]);
      record();
      await replyAt(next, again, 2, [250, 250, 250]);
      record();
      await replyAt(next, again, 4, [500, 500, 500]);
      record();
      await replyAt(next, again, 5, [1_000, 1_000, 1_000]);
      record();
      expect(seen).toEqual([
        [3, 500, false],
        [3, 500, false],
        [3, 500, false],
        [4, 500, false],
        [5, 1_000, false],
      ]);
      expect(result.current.model?.stale).toBe(false);
    });

    it("is pending on a new arrival until its first reply", async () => {
      const { rerender, result, socket } = await heldFinal();
      rerender(at(0, "0200080020000001"));
      expect([result.current.pending, result.current.model]).toEqual([true, null]);
      await replyAt(socket, latestSky(socket).id, 1, [125, 125, 125]);
      expect([result.current.pending, result.current.model?.stars.count]).toEqual([false, 1]);
    });

    it("keeps the held sky, asking nothing more, back in its arrival before another's first reply", async () => {
      const { rerender, result, socket } = await heldFinal();
      rerender(at(0, "0200080020000001"));
      const other = latestSky(socket).id;
      rerender(at(0));
      expect([
        socket.cancelledIds(),
        socket.requestsOfKind("sky").length,
        result.current.pending,
        result.current.model?.stars.count,
      ]).toEqual([[other], 2, false, 3]);
    });
  });
});
