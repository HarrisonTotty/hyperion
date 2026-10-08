import type { SkyRequest } from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { RECONNECT_DELAY_MS } from "../../lib/connection";
import { binaryFrame } from "../../test/binaryFrames";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { type FixtureStar, skyPayload, skyRequest, skyResponse } from "../../test/skyFixtures";
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

/** Plays one reply to sky `id` of `stars`: its payload's frame, then its message. */
async function reply(
  socket: FakeWebSocket,
  id: number,
  stars: ReadonlyArray<FixtureStar>,
  final: boolean,
): Promise<void> {
  const request = latestSky(socket).body;
  const payload = skyPayload(stars, 2, 6.6);
  const body = { kind: "sky" as const, ...skyResponse(request, payload, stars.length, 2), final };
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
});
