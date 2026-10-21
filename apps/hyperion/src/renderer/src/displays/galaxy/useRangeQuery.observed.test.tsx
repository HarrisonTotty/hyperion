import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { NOW_MODE, type QueryMode } from "../../lib/galaxy/model";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { aRangeRequest, aSystemsInRange, UNIVERSE_ID } from "../../test/galaxyFixtures";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { type RangeQueryInput, useRangeQuery } from "./useRangeQuery";

const CENTRE = [26_000, 0, 0] as const;
const OBSERVED: QueryMode = { kind: "observed", observerLy: CENTRE };
const ELSEWHERE: QueryMode = { kind: "observed", observerLy: [25_000, 10, 0] };

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

function renderQuery(input: RangeQueryInput) {
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

async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

describe("useRangeQuery's mode (plan 12, P12.T11)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("asks in observed mode from the observer", () => {
    const { socket } = renderQuery(anInput({ mode: OBSERVED }));

    expect(socket.requestsOfKind("systems_in_range").map(({ body }) => body)).toEqual([
      aRangeRequest({ centreLy: CENTRE, observerLy: CENTRE }),
    ]);
  });

  it("asks in now, with no mode, when none is given or now is", () => {
    const { socket, rerender } = renderQuery(anInput());
    rerender(anInput({ mode: NOW_MODE }));

    expect(socket.requestsOfKind("systems_in_range").map(({ body }) => body)).toEqual([
      aRangeRequest({ centreLy: CENTRE }),
    ]);
  });

  it.each([
    ["the mode", anInput({ mode: OBSERVED }), anInput({ mode: NOW_MODE })],
    ["the observer", anInput({ mode: OBSERVED }), anInput({ mode: ELSEWHERE })],
  ] as const)("supersedes the query in flight when %s changes", (_what, before, after) => {
    const { socket, rerender } = renderQuery(before);
    const first = socket.requestsOfKind("systems_in_range")[0];

    rerender(after);

    expect(socket.cancelledIds()).toEqual([first?.id]);
    expect(socket.requestsOfKind("systems_in_range")).toHaveLength(2);
  });

  it("charts an answer with the mode of the request it answers, kept when the mode changes", async () => {
    const { socket, rerender, result } = renderQuery(anInput({ mode: OBSERVED }));
    await server(() => {
      socket.serverAnswers("systems_in_range", () => aSystemsInRange({ centreLy: CENTRE }));
    });
    expect(result.current.shown?.mode).toEqual(OBSERVED);

    rerender(anInput({ mode: ELSEWHERE }));

    // The answer on show stays, still labelled with the mode it answers, while the new one is asked.
    expect(result.current.state.kind).toBe("pending");
    expect(result.current.shown?.mode).toEqual(OBSERVED);
  });
});
