import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { FakeWebSocket } from "../../test/FakeWebSocket";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { sunLikeHostDisc } from "./hostDisc";
import { type HostDiscsAsk, useHostDiscs } from "./useHostDiscs";

afterEach(() => {
  vi.useRealTimers();
});

const UNIVERSE = "000000000000002a";

/** An ask of `system` at `seconds` past the epoch. */
function askOf(system: string, seconds = 0): HostDiscsAsk {
  return { universe: UNIVERSE, system, time: { seconds, nanos: 0 } };
}

function renderDiscs(initial: HostDiscsAsk | null) {
  vi.stubGlobal("WebSocket", FakeWebSocket);
  const hook = renderHook(({ ask }: { readonly ask: HostDiscsAsk | null }) => useHostDiscs(ask), {
    initialProps: { ask: initial },
    wrapper: ServerLinkHarness,
  });
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { ...hook, socket };
}

/** Answers the latest unanswered `host_discs` with a solar twin's disc. */
async function answer(socket: FakeWebSocket): Promise<void> {
  await act(async () => {
    socket.serverAnswers("host_discs", (body) => ({
      ...body,
      hosts: [sunLikeHostDisc()],
    }));
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("useHostDiscs", () => {
  it("asks a system's discs once it is known, pending until they are answered", async () => {
    const { result, socket } = renderDiscs(askOf("0200080020000000", 3_600));
    const [asked] = socket.requestsOfKind("host_discs");
    expect([asked?.body, result.current.pending, result.current.held]).toEqual([
      { kind: "host_discs", ...askOf("0200080020000000", 3_600) },
      true,
      null,
    ]);
    await answer(socket);
    expect(result.current).toEqual({
      held: { universe: UNIVERSE, system: "0200080020000000", discs: [sunLikeHostDisc()] },
      pending: false,
    });
  });

  it("asks nothing again as the scene's time runs on in the same system", async () => {
    const { rerender, socket } = renderDiscs(askOf("0200080020000000", 0));
    await answer(socket);
    rerender({ ask: askOf("0200080020000000", 60) });
    expect(socket.requestsOfKind("host_discs")).toHaveLength(1);
  });

  it("cancels a system left before its answer, and asks the next", () => {
    const { rerender, socket, result } = renderDiscs(askOf("0200080020000000"));
    const [first] = socket.requestsOfKind("host_discs");
    rerender({ ask: askOf("0200080020000001") });
    expect([
      first !== undefined && socket.cancelledIds().includes(first.id),
      socket.requestsOfKind("host_discs").at(-1)?.body.system,
      result.current.pending,
    ]).toEqual([true, "0200080020000001", true]);
  });

  it("holds the discs held while it asks another system's, which the lighting tells apart", async () => {
    const { rerender, socket, result } = renderDiscs(askOf("0200080020000000"));
    await answer(socket);
    rerender({ ask: askOf("0200080020000001") });
    expect([result.current.held?.system, result.current.pending]).toEqual([
      "0200080020000000",
      true,
    ]);
  });

  it("holds a refusal, as an older server's unsupported, and does not ask again", async () => {
    const { rerender, socket, result } = renderDiscs(askOf("0200080020000000"));
    const [asked] = socket.requestsOfKind("host_discs");
    await act(async () => {
      if (asked !== undefined) {
        socket.serverRejects(asked.id, {
          code: "unsupported",
          message: "unsupported request kind `host_discs`",
          field: null,
        });
      }
      await Promise.resolve();
    });
    rerender({ ask: askOf("0200080020000000", 60) });
    expect([socket.requestsOfKind("host_discs").length, result.current]).toEqual([
      1,
      { held: null, pending: false },
    ]);
  });

  it("asks nothing where the view asks none", () => {
    const { socket, result } = renderDiscs(null);
    expect([socket.requestsOfKind("host_discs").length, result.current]).toEqual([
      0,
      { held: null, pending: false },
    ]);
  });
});
