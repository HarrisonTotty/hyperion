import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { toFeaturesRequest } from "../../lib/galaxy/featureWire";
import {
  aFeaturesInRange,
  GOLDEN_CENTRE_LY,
  OPEN_CLUSTER,
  STAR_FORMING_REGION,
} from "../../test/featureFixtures";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { UNIVERSE_ID } from "../../test/galaxyFixtures";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { type FeaturesInRangeInput, useFeaturesInRange } from "./useFeaturesInRange";

const OTHER_UNIVERSE = "00000000000000b2";

function anInput(overrides: Partial<FeaturesInRangeInput> = {}): FeaturesInRangeInput {
  return {
    universe: UNIVERSE_ID,
    centreLy: GOLDEN_CENTRE_LY,
    queryRadiusLy: 300,
    timeYr: 0,
    generation: 0,
    ...overrides,
  };
}

/** Renders the hook under a welcomed server link. */
function renderQuery(input: FeaturesInRangeInput = anInput()) {
  const hook = renderHook((current: FeaturesInRangeInput) => useFeaturesInRange(current), {
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

describe("useFeaturesInRange (plan 09, P09.T41.a)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("asks for every kind of feature around the chart's centre, at its radius and time", () => {
    const { socket } = renderQuery(anInput({ queryRadiusLy: 20, timeYr: 12.5 }));

    expect(socket.requestsOfKind("features_in_range").map(({ body }) => body)).toEqual([
      toFeaturesRequest(UNIVERSE_ID, GOLDEN_CENTRE_LY, 20, 12.5),
    ]);
  });

  it("asks nothing without a centre, and nothing again when the same chart re-renders", () => {
    const { socket, result, rerender } = renderQuery(anInput({ centreLy: null }));

    expect(socket.requestsOfKind("features_in_range")).toHaveLength(0);
    expect(result.current.state.kind).toBe("idle");
    rerender(anInput());
    rerender(anInput());
    expect(socket.requestsOfKind("features_in_range")).toHaveLength(1);
  });

  it("gives the answer as the chart's features, nearest first", async () => {
    const { socket, result } = renderQuery();

    await server(() => {
      socket.serverAnswers("features_in_range", () =>
        aFeaturesInRange({ features: [STAR_FORMING_REGION, OPEN_CLUSTER] }),
      );
    });

    expect(result.current.state.kind).toBe("ok");
    expect(result.current.shown?.features.map((feature) => feature.designation)).toEqual([
      "GPF F207",
      "GPG F336",
    ]);
  });

  it("keeps the answer while a newer query is pending, and drops it with the universe", async () => {
    const { socket, result, rerender } = renderQuery();
    await server(() => {
      socket.serverAnswers("features_in_range", () => aFeaturesInRange());
    });

    rerender(anInput({ queryRadiusLy: 100 }));
    expect(result.current.state.kind).toBe("pending");
    expect(result.current.shown?.radiusLy).toBe(300);

    rerender(anInput({ universe: OTHER_UNIVERSE }));
    expect(result.current.shown).toBeNull();
  });

  it("asks the same query again when its generation changes, as RETRY does", () => {
    const { socket, rerender } = renderQuery();

    rerender(anInput({ generation: 1 }));

    expect(socket.requestsOfKind("features_in_range")).toHaveLength(2);
  });
});
