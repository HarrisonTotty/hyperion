/**
 * `useStreamReadout` (plan 10, P10.T11): what a stream member's readout asks of its stream, where
 * its requests stand, and its track asked again after a `pending` answer.
 */
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FakeWebSocket } from "../../test/FakeWebSocket";
import { UNIVERSE_ID } from "../../test/galaxyFixtures";
import { aGlobalFeatures, aStreamEntry, aStreamTrack } from "../../test/globalListFixtures";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { TRACK_ASK_AGAIN_MS, useStreamReadout } from "./useStreamReadout";

function Wrapper({ children }: { readonly children: ReactNode }) {
  return <ServerLinkHarness>{children}</ServerLinkHarness>;
}

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/** The hook for stream 1 of the fixtures' universe, with the link welcomed, and its socket. */
function renderStream(generation = 0) {
  const hook = renderHook(
    ({ asked }: { readonly asked: number }) => useStreamReadout(UNIVERSE_ID, 1, asked),
    { wrapper: Wrapper, initialProps: { asked: generation } },
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { hook, socket };
}

describe("useStreamReadout (plan 10, P10.T11)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("asks for the list and the stream's track, and stands pending until both answer", async () => {
    const { hook, socket } = renderStream();

    expect(hook.result.current.state).toEqual({ kind: "pending" });
    expect(socket.requestsOfKind("stream_track").map(({ body }) => body.stream)).toEqual([1]);
    await server(() => {
      socket.serverAnswers("global_features", () =>
        aGlobalFeatures({ entries: [aStreamEntry({ number: 1 })] }),
      );
      socket.serverAnswers("stream_track", () =>
        aStreamTrack(1, [[30_000, 0, 0]], [[31_000, 0, 0]]),
      );
    });

    expect(hook.result.current.state).toBeNull();
    expect(hook.result.current.list?.streams.map((stream) => stream.number)).toEqual([1]);
    expect(hook.result.current.track?.lengthLy).toBe(100_000);
  });

  it("asks for a track answered pending again after a while, until it is ready", async () => {
    const { hook, socket } = renderStream();
    await server(() => {
      socket.serverAnswers("global_features", () => aGlobalFeatures());
      socket.serverAnswers("stream_track", (body) => ({
        kind: "stream_track",
        universe: body.universe,
        stream: body.stream,
        track: { status: "pending" },
      }));
    });

    expect(hook.result.current.state).toEqual({ kind: "pending" });
    expect(hook.result.current.track).toBeNull();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(TRACK_ASK_AGAIN_MS);
    });
    expect(socket.requestsOfKind("stream_track")).toHaveLength(2);
    await server(() => {
      socket.serverAnswers("stream_track", () =>
        aStreamTrack(1, [[30_000, 0, 0]], [[31_000, 0, 0]]),
      );
    });

    expect(hook.result.current.track?.number).toBe(1);
    expect(hook.result.current.state).toBeNull();
  });

  it("says a refused request, and asks again when the generation changes", async () => {
    const { hook, socket } = renderStream();
    const [request] = socket.requestsOfKind("global_features");
    if (request === undefined) {
      throw new Error("the hook asked for no list");
    }
    await server(() => {
      socket.serverRejects(request.id, {
        code: "internal",
        message: "the list could not be built",
        field: null,
      });
    });

    expect(hook.result.current.state).toMatchObject({ kind: "rejected" });
    hook.rerender({ asked: 1 });
    expect(socket.requestsOfKind("global_features")).toHaveLength(2);
  });
});
