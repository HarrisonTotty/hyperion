import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { FakeWebSocket } from "../../test/FakeWebSocket";
import { aRangeRequest, aSystemsInRange, UNIVERSE_ID } from "../../test/galaxyFixtures";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { type RangeQueryInput, useRangeQuery } from "./useRangeQuery";

const CENTRE = [26_000, 0, 0] as const;
const OTHER_UNIVERSE = "00000000000000b2";

function anInput(overrides: Partial<RangeQueryInput> = {}): RangeQueryInput {
  return {
    universe: UNIVERSE_ID,
    centreLy: CENTRE,
    queryRadiusLy: 50,
    minLayer: "a",
    timeYr: 0,
    generation: 0,
    ...overrides,
  };
}

/** Renders the hook under a welcomed server link. */
function renderQuery(input: RangeQueryInput = anInput()) {
  const hook = renderHook((current: RangeQueryInput) => useRangeQuery(current), {
    initialProps: input,
    wrapper: ServerLinkHarness,
  });
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { ...hook, socket };
}

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

describe("useRangeQuery", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("asks for the systems around the centre, with the chart's limit", () => {
    const { socket } = renderQuery(anInput({ queryRadiusLy: 20, minLayer: "c", timeYr: 12.5 }));

    expect(socket.requestsOfKind("systems_in_range").map(({ body }) => body)).toEqual([
      aRangeRequest({ centreLy: CENTRE, radiusLy: 20, minLayer: "c", timeYr: 12.5, limit: 4_000 }),
    ]);
  });

  it("asks nothing without a centre", () => {
    const { socket, result } = renderQuery(anInput({ centreLy: null }));

    expect(socket.requestsOfKind("systems_in_range")).toHaveLength(0);
    expect(result.current.state.kind).toBe("idle");
    expect(result.current.shown).toBeNull();
  });

  it("asks nothing again when the same chart re-renders", () => {
    const { socket, rerender } = renderQuery();

    rerender(anInput());

    expect(socket.requestsOfKind("systems_in_range")).toHaveLength(1);
  });

  it("gives the answer as a chart, nearest system first", async () => {
    const { socket, result } = renderQuery();

    await server(() => {
      socket.serverAnswers("systems_in_range", () =>
        aSystemsInRange({
          centreLy: CENTRE,
          systems: [
            { relLy: [30, 0, 0], layer: "a" },
            { relLy: [10, 0, 0], layer: "a" },
          ],
        }),
      );
    });

    expect(result.current.shown?.systems.map((system) => system.distanceLy)).toEqual([10, 30]);
    expect(result.current.shown?.radiusLy).toBe(50);
  });

  it("cancels the query in flight and asks again when the radius changes", async () => {
    const { socket, rerender } = renderQuery();
    const first = socket.requestsOfKind("systems_in_range")[0];

    rerender(anInput({ queryRadiusLy: 20 }));

    expect(socket.cancelledIds()).toEqual([first?.id]);
    expect(socket.requestsOfKind("systems_in_range")).toHaveLength(2);
  });

  it("ignores the late answer to a superseded query", async () => {
    const { socket, rerender, result } = renderQuery();
    const first = socket.requestsOfKind("systems_in_range")[0];
    rerender(anInput({ queryRadiusLy: 20 }));

    await server(() => {
      if (first === undefined) {
        throw new Error("no query was sent");
      }
      socket.serverResponds(first.id, aSystemsInRange({ centreLy: CENTRE, radiusLy: 50 }));
    });

    expect(result.current.shown).toBeNull();
    expect(result.current.state.kind).toBe("pending");
  });

  it("keeps the answer on show while a newer query is pending", async () => {
    const { socket, rerender, result } = renderQuery();
    await server(() => {
      socket.serverAnswers("systems_in_range", () =>
        aSystemsInRange({ centreLy: CENTRE, systems: [{ relLy: [10, 0, 0], layer: "a" }] }),
      );
    });

    rerender(anInput({ queryRadiusLy: 20 }));

    expect(result.current.state.kind).toBe("pending");
    expect(result.current.shown?.systems).toHaveLength(1);
  });

  it("keeps the answer on show when a newer query is rejected", async () => {
    const { socket, rerender, result } = renderQuery();
    await server(() => {
      socket.serverAnswers("systems_in_range", () =>
        aSystemsInRange({ centreLy: CENTRE, systems: [{ relLy: [10, 0, 0], layer: "a" }] }),
      );
    });
    rerender(anInput({ queryRadiusLy: 20 }));

    await server(() => {
      const pending = socket.requestsOfKind("systems_in_range").at(-1);
      if (pending === undefined) {
        throw new Error("no query was sent");
      }
      socket.serverRejects(pending.id, {
        code: "queue_full",
        message: "the queue is full",
        field: null,
      });
    });

    expect(result.current.state).toEqual({
      kind: "rejected",
      code: "queue_full",
      reason: "the queue is full",
    });
    expect(result.current.shown?.systems).toHaveLength(1);
  });

  it("asks the same query again when the generation changes, as RETRY does", async () => {
    const { socket, rerender } = renderQuery();
    await server(() => {
      const pending = socket.requestsOfKind("systems_in_range")[0];
      if (pending === undefined) {
        throw new Error("no query was sent");
      }
      socket.serverRejects(pending.id, {
        code: "queue_full",
        message: "the queue is full",
        field: null,
      });
    });

    rerender(anInput({ generation: 1 }));

    expect(socket.requestsOfKind("systems_in_range")).toHaveLength(2);
  });

  it("drops the answer when another universe is opened", async () => {
    const { socket, rerender, result } = renderQuery();
    await server(() => {
      socket.serverAnswers("systems_in_range", () =>
        aSystemsInRange({ centreLy: CENTRE, systems: [{ relLy: [10, 0, 0], layer: "a" }] }),
      );
    });

    rerender(anInput({ universe: OTHER_UNIVERSE }));

    expect(result.current.shown).toBeNull();
  });
});
