import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FakeWebSocket } from "../test/FakeWebSocket";
import {
  aCreatedUniverse,
  anOpenedUniverse,
  aUniverse,
  aUniverseList,
} from "../test/galaxyFixtures";
import { ServerLinkHarness } from "../test/ServerLinkHarness";
import { useUniverse, useUniverseSession } from "./universe";
import { REQUEST_TIMEOUT_MS } from "./useServerRequest";

const RECONNECT_DELAY_MS = 2_000;

const BEEF = aUniverse({ id: "00000000000000b2", name: "SURVEY 2", seed: "000000000000beef" });

/** Renders the session under a server link, welcomed unless told otherwise. */
function renderSession({ welcomed = true }: { welcomed?: boolean } = {}) {
  const hook = renderHook(() => useUniverseSession(), { wrapper: ServerLinkHarness });
  const socket = FakeWebSocket.latest();
  if (welcomed) {
    act(() => {
      socket.serverWelcomes();
    });
  }
  return { ...hook, socket };
}

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

describe("useUniverseSession", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("requests the universe list once the link is up, and not before", async () => {
    const { result, socket } = renderSession({ welcomed: false });
    expect(socket.requestsOfKind("list_universes")).toEqual([]);
    expect(result.current.list).toEqual({ kind: "link_down", reason: "ESTABLISHING LINK" });

    act(() => {
      socket.serverWelcomes();
    });
    expect(socket.requestsOfKind("list_universes")).toHaveLength(1);
    await server(() => {
      socket.serverAnswers("list_universes", () => aUniverseList());
    });

    expect(result.current.list).toEqual({ kind: "ok", response: aUniverseList() });
    expect(result.current.open).toBeNull();
    expect(result.current.command).toEqual({ kind: "idle" });
  });

  it("creates a universe from a name and a seed, and opens it", async () => {
    const { result, socket } = renderSession();

    act(() => {
      result.current.create("SURVEY 2", "000000000000beef");
    });
    expect(socket.requestsOfKind("create_universe")).toEqual([
      { id: 2, body: { kind: "create_universe", name: "SURVEY 2", seed: "000000000000beef" } },
    ]);
    expect(result.current.command).toEqual({ kind: "pending" });
    expect(result.current.open).toBeNull();

    await server(() => {
      socket.serverAnswers("create_universe", () => aCreatedUniverse(BEEF));
    });

    expect(result.current.open).toEqual(BEEF);
    expect(result.current.command).toMatchObject({ kind: "ok" });
    expect(socket.requestsOfKind("open_universe")).toEqual([]);
  });

  it("asks the server to draw the seed when none is given", () => {
    const { result, socket } = renderSession();

    act(() => {
      result.current.create("SURVEY 3", null);
    });

    expect(socket.requestsOfKind("create_universe")[0]?.body).toEqual({
      kind: "create_universe",
      name: "SURVEY 3",
      seed: null,
    });
  });

  it("leaves the open universe alone and gives the reason when a create is rejected", async () => {
    const { result, socket } = renderSession();
    act(() => {
      result.current.openUniverse(aUniverse().id);
    });
    await server(() => {
      socket.serverAnswers("open_universe", () => anOpenedUniverse());
    });

    act(() => {
      result.current.create("SURVEY 1", null);
    });
    await server(() => {
      socket.serverRejects(socket.requestsOfKind("create_universe")[0]?.id ?? -1, {
        code: "name_taken",
        message: "a universe named SURVEY 1 exists",
        field: "name",
      });
    });

    expect(result.current.open).toEqual(aUniverse());
    expect(result.current.command).toEqual({
      kind: "rejected",
      code: "name_taken",
      reason: "a universe named SURVEY 1 exists",
    });
  });

  it("opens a listed universe once the server has checked it", async () => {
    const { result, socket } = renderSession();

    act(() => {
      result.current.openUniverse(BEEF.id);
    });
    expect(socket.requestsOfKind("open_universe")[0]?.body).toEqual({
      kind: "open_universe",
      universe: BEEF.id,
    });
    expect(result.current.open).toBeNull();
    await server(() => {
      socket.serverAnswers("open_universe", () => anOpenedUniverse(BEEF));
    });

    expect(result.current.open).toEqual(BEEF);
  });

  it("requests the list again after a create", async () => {
    const { result, socket } = renderSession();
    await server(() => {
      socket.serverAnswers("list_universes", () => aUniverseList([]));
    });

    act(() => {
      result.current.create("SURVEY 2", "000000000000beef");
    });
    expect(socket.requestsOfKind("list_universes")).toHaveLength(1);
    await server(() => {
      socket.serverAnswers("create_universe", () => aCreatedUniverse(BEEF));
    });

    expect(socket.requestsOfKind("list_universes")).toHaveLength(2);
    await server(() => {
      socket.serverAnswers("list_universes", () => aUniverseList([BEEF]));
    });
    expect(result.current.list).toEqual({ kind: "ok", response: aUniverseList([BEEF]) });
  });

  it("requests the list again after a reconnect, and keeps the open universe", async () => {
    vi.useFakeTimers();
    const { result, socket } = renderSession();
    await server(() => {
      socket.serverAnswers("list_universes", () => aUniverseList());
    });
    act(() => {
      result.current.openUniverse(aUniverse().id);
    });
    await server(() => {
      socket.serverAnswers("open_universe", () => anOpenedUniverse());
    });

    await server(() => {
      socket.close();
    });
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });
    const reconnected = FakeWebSocket.latest();
    act(() => {
      reconnected.serverWelcomes();
    });

    expect(reconnected.requestsOfKind("list_universes")).toHaveLength(1);
    expect(reconnected.requestsOfKind("open_universe")).toEqual([]);
    expect(result.current.open).toEqual(aUniverse());
  });

  it("requests the list again on refresh", () => {
    const { result, socket } = renderSession();

    act(() => {
      result.current.refresh();
    });

    expect(socket.requestsOfKind("list_universes")).toHaveLength(2);
  });

  it("reports a command the server does not answer in 30 s as timed out", () => {
    vi.useFakeTimers();
    const { result, socket } = renderSession();
    act(() => {
      result.current.create("SURVEY 2", null);
    });

    act(() => {
      vi.advanceTimersByTime(REQUEST_TIMEOUT_MS);
    });

    expect(result.current.command).toEqual({ kind: "timed_out" });
    const [create] = socket.requestsOfKind("create_universe");
    expect(socket.cancelledIds()).toContain(create?.id);
  });

  it("requests the list again after a create times out, since the server may have made it", async () => {
    vi.useFakeTimers();
    const { result, socket } = renderSession();
    await server(() => {
      socket.serverAnswers("list_universes", () => aUniverseList([]));
    });
    act(() => {
      result.current.create("SURVEY 2", null);
    });

    act(() => {
      vi.advanceTimersByTime(REQUEST_TIMEOUT_MS);
    });

    expect(socket.requestsOfKind("list_universes")).toHaveLength(2);
    expect(result.current.open).toBeNull();
  });

  it("reports NO CARRIER for a command whose link drops, and drops that once the link is back", async () => {
    vi.useFakeTimers();
    const { result, socket } = renderSession();
    act(() => {
      result.current.create("SURVEY 2", null);
    });

    await server(() => {
      socket.close();
    });
    expect(result.current.command).toEqual({ kind: "link_down", reason: "NO CARRIER" });
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });
    act(() => {
      FakeWebSocket.latest().serverWelcomes();
    });

    expect(FakeWebSocket.latest().requestsOfKind("create_universe")).toEqual([]);
    expect(result.current.command).toEqual({ kind: "idle" });
  });

  it("keeps its identity while nothing changes", () => {
    const { result, rerender } = renderSession();
    const before = result.current;

    rerender();

    expect(result.current).toBe(before);
  });
});

describe("useUniverse", () => {
  it("names the missing provider when used outside one", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});

    expect(() => renderHook(() => useUniverse())).toThrow(
      "useUniverse needs a UniverseContext provider",
    );
  });
});
