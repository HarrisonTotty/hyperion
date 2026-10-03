import type { SkyRequest } from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { RECONNECT_DELAY_MS } from "../../lib/connection";
import { binaryFrame } from "../../test/binaryFrames";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { skyPayload, skyRequest, skyResponse } from "../../test/skyFixtures";
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
