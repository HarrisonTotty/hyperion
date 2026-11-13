import { galacticPositionFromLy, universeTimeFromYears } from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { NOW_MODE, type QueryMode } from "../../lib/galaxy/model";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { anObservation } from "../../test/galaxyFixtures";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  A_CENTURY,
  aSummaryResponse,
  aSystemSummary,
  PIN_SYSTEM,
  PIN_UNIVERSE,
} from "../../test/systemFixtures";
import { type SummaryTarget, useSystemSummary } from "./useSystemSummary";

const TARGET: SummaryTarget = {
  universe: PIN_UNIVERSE,
  system: PIN_SYSTEM,
  designation: "H7K 4C0RFZ C-1",
};

const OBSERVER_LY = [26_000, 0, 0] as const;
const OBSERVED: QueryMode = { kind: "observed", observerLy: OBSERVER_LY };

interface Input {
  readonly mode?: QueryMode;
}

function renderSummary(input: Input) {
  const hook = renderHook(({ mode }: Input) => useSystemSummary(TARGET, A_CENTURY, 0, mode), {
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

describe("useSystemSummary's mode (plan 12, P12.T11)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("asks in now, with no mode, as the SYSTEM display does", () => {
    const { socket } = renderSummary({});

    expect(socket.requestsOfKind("system_summary").map(({ body }) => body)).toEqual([
      { kind: "system_summary", universe: PIN_UNIVERSE, system: PIN_SYSTEM, time: A_CENTURY },
    ]);
  });

  it("asks in observed mode from the observer", () => {
    const { socket } = renderSummary({ mode: OBSERVED });

    expect(socket.requestsOfKind("system_summary")[0]?.body.mode).toEqual({
      type: "observed",
      observer: galacticPositionFromLy(OBSERVER_LY),
    });
  });

  it("supersedes the request in flight when the mode changes", () => {
    const { socket, rerender } = renderSummary({ mode: NOW_MODE });
    const first = socket.requestsOfKind("system_summary")[0];

    rerender({ mode: OBSERVED });

    expect(socket.cancelledIds()).toEqual([first?.id]);
    expect(socket.requestsOfKind("system_summary")).toHaveLength(2);
  });

  it("says an observed answer is in the mode asked, with when its light left", async () => {
    const { socket, result } = renderSummary({ mode: OBSERVED });
    const observed = anObservation({ apparentLy: OBSERVER_LY, lightAgeYr: 3_000, timeYr: 100 });

    await server(() => {
      socket.serverAnswers("system_summary", () => aSummaryResponse(aSystemSummary({ observed })));
    });

    expect(result.current.shownMode).toEqual(OBSERVED);
    expect(result.current.shownEmitted).toEqual(universeTimeFromYears(100 - 3_000));
  });

  it("says an answer without its observation is the present's, whatever was asked", async () => {
    const { socket, result } = renderSummary({ mode: OBSERVED });

    await server(() => {
      socket.serverAnswers("system_summary", () => aSummaryResponse(aSystemSummary()));
    });

    expect(result.current.shownMode).toEqual(NOW_MODE);
    expect(result.current.shownEmitted).toBeNull();
  });

  it("keeps the mode of the answer on show while a request in another is pending", async () => {
    const { socket, result, rerender } = renderSummary({ mode: NOW_MODE });
    await server(() => {
      socket.serverAnswers("system_summary", () => aSummaryResponse(aSystemSummary()));
    });

    rerender({ mode: OBSERVED });

    expect(result.current.state.kind).toBe("pending");
    expect(result.current.shownMode).toEqual(NOW_MODE);
    expect(result.current.shown?.kind).toBe("ok");
  });
});
