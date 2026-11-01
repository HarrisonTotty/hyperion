import {
  type ContactDto,
  galacticPositionFromLy,
  type UniverseIdHex,
  universeTimeFromYears,
} from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  aBearingContact,
  aCensusLine,
  alertsNotification,
  aResolvedContact,
} from "../test/alertFixtures";
import { FakeWebSocket } from "../test/FakeWebSocket";
import { ServerLinkHarness } from "../test/ServerLinkHarness";
import { type AlertObserver, STAND_IN_SENSOR, useAlerts } from "./useAlerts";
import { REQUEST_TIMEOUT_MS } from "./useServerRequest";

const UNIVERSE: UniverseIdHex = "00000000000000a1";
const OBSERVER: AlertObserver = { positionLy: [0, 26_000, 20], timeYr: 0 };

/** Lets the outcomes the server's messages settle reach the store and React. */
async function settle(): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** Renders the hook under a welcomed server link, its observer `observer` until rerendered. */
function renderAlerts(observer: AlertObserver | null = OBSERVER) {
  const hook = renderHook(({ at }: { at: AlertObserver | null }) => useAlerts(UNIVERSE, at), {
    wrapper: ServerLinkHarness,
    initialProps: { at: observer },
  });
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { ...hook, socket };
}

/** Answers the latest `subscribe` with subscription `id` opening on `contacts`. */
async function opens(
  socket: FakeWebSocket,
  id: number,
  contacts: ReadonlyArray<ContactDto> = [aBearingContact()],
): Promise<void> {
  act(() => {
    socket.serverAnswers("subscribe", () => ({
      kind: "subscribe",
      subscription: id,
      state: { topic: "alerts", census: [], contacts: [...contacts] },
    }));
  });
  await settle();
}

describe("useAlerts (plan 12, P12.T12)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("subscribes to the alerts seen from the observer, with the stand-in sensor", () => {
    const { socket, result } = renderAlerts();

    const [sent] = socket.requestsOfKind("subscribe");
    expect(sent?.body).toEqual({
      kind: "subscribe",
      universe: UNIVERSE,
      topic: {
        topic: "alerts",
        observer: galacticPositionFromLy(OBSERVER.positionLy),
        time: universeTimeFromYears(0),
        sensor: STAND_IN_SENSOR,
      },
    });
    expect(result.current.status).toEqual({ kind: "pending" });
  });

  it("asks for nothing with no observer, and says so", () => {
    const { socket, result } = renderAlerts(null);

    expect(socket.requestsOfKind("subscribe")).toEqual([]);
    expect(result.current.status).toEqual({ kind: "no_observer" });
  });

  it("states each band's two limits, the resolve limit no fainter than the detection's", () => {
    for (const limits of STAND_IN_SENSOR.bands) {
      expect(limits.detect_w_m2).toBeGreaterThan(0);
      expect(limits.resolve_w_m2).toBeGreaterThanOrEqual(limits.detect_w_m2);
    }
    // V = 17: 3.631E-11 W/m²/nm × 84 nm × 10^-6.8 (Bessell, Castelli and Plez 1998; Bessell 2005).
    expect(STAND_IN_SENSOR.bands[0]?.detect_w_m2).toBeCloseTo(4.834e-16, 19);
  });

  it("holds the contacts the subscription opens with, live", async () => {
    const { socket, result } = renderAlerts();
    await opens(socket, 4);

    expect(result.current.status).toEqual({ kind: "live" });
    expect(result.current.contacts).toEqual([aBearingContact()]);
  });

  it("moves the observer with the display's time, one move in flight and the latest after it", async () => {
    const { socket, rerender } = renderAlerts();
    await opens(socket, 4);

    rerender({ at: { ...OBSERVER, timeYr: 1 } });
    rerender({ at: { ...OBSERVER, timeYr: 2 } });
    const moves = () => socket.requestsOfKind("alerts_observer");
    expect(moves().map(({ body }) => [body.subscription, body.time])).toEqual([
      [4, universeTimeFromYears(1)],
    ]);

    act(() => {
      socket.serverAnswers("alerts_observer", () => ({ kind: "alerts_observer" }));
    });
    await settle();

    expect(moves().map(({ body }) => body.time)).toEqual([
      universeTimeFromYears(1),
      universeTimeFromYears(2),
    ]);
  });

  it("reports a move of the observer the server refuses", async () => {
    const { socket, rerender, result } = renderAlerts();
    await opens(socket, 4);

    rerender({ at: { ...OBSERVER, timeYr: 5000 } });
    const [move] = socket.requestsOfKind("alerts_observer");
    act(() => {
      socket.serverRejects(move?.id ?? 0, {
        code: "bad_request",
        message: "the time lies outside the clock window",
        field: "time",
      });
    });
    await settle();

    expect(result.current.observerFault).toEqual({
      kind: "refused",
      reason: "the time lies outside the clock window",
    });
  });

  it("takes notifications in sequence order, applying each contact as it now stands", async () => {
    const { socket, result } = renderAlerts();
    await opens(socket, 4);

    act(() => {
      socket.serverNotifies(
        4,
        alertsNotification(1, [aBearingContact({ flux_w_m2: 6e-12 })], [aCensusLine("pending")]),
      );
    });

    expect(result.current.contacts.map((contact) => contact.flux_w_m2)).toEqual([6e-12]);
    expect(result.current.census).toEqual([aCensusLine("pending")]);
  });

  it("drops a notification with an older sequence than the last applied", async () => {
    const { socket, result } = renderAlerts();
    await opens(socket, 4);

    act(() => {
      socket.serverNotifies(4, alertsNotification(1, [aBearingContact({ flux_w_m2: 6e-12 })]));
      socket.serverNotifies(4, alertsNotification(2, [aBearingContact({ flux_w_m2: 7e-12 })]));
      socket.serverNotifies(4, alertsNotification(1, [aBearingContact({ flux_w_m2: 1e-15 })]));
    });

    expect(result.current.contacts.map((contact) => contact.flux_w_m2)).toEqual([7e-12]);
    expect(result.current.status).toEqual({ kind: "live" });
    expect(socket.requestsOfKind("subscribe")).toHaveLength(1);
  });

  it("keeps the census it holds when a notification sends none, its census unchanged", async () => {
    const { socket, result } = renderAlerts();
    await opens(socket, 4);

    act(() => {
      socket.serverNotifies(4, alertsNotification(1, [], [aCensusLine("complete")]));
      socket.serverNotifies(4, alertsNotification(2, [aBearingContact({ contact: 3 })]));
    });

    expect(result.current.census).toEqual([aCensusLine("complete")]);
    expect(result.current.contacts.map((contact) => contact.contact)).toEqual([1, 3]);
  });

  it("opens the subscription again after a gap in the numbering, stale meanwhile", async () => {
    const { socket, result } = renderAlerts();
    await opens(socket, 4);
    vi.spyOn(console, "error").mockImplementation(() => undefined);

    act(() => {
      socket.serverNotifies(4, alertsNotification(3, [aBearingContact({ flux_w_m2: 1e-15 })]));
    });

    expect(result.current.status).toEqual({ kind: "stale", reason: "resubscribing" });
    expect(result.current.contacts).toEqual([aBearingContact()]);
    expect(socket.requestsOfKind("subscribe")).toHaveLength(2);
  });

  it("marks everything held stale on the link's loss, and keeps it on show", async () => {
    const { socket, result } = renderAlerts();
    await opens(socket, 4);

    act(() => {
      socket.close();
    });
    await settle();

    expect(result.current.status).toEqual({ kind: "stale", reason: "link_down" });
    expect(result.current.contacts).toEqual([aBearingContact()]);
  });

  it("acknowledges closed-loop: pending until the server accepts, then acknowledged", async () => {
    const { socket, result } = renderAlerts();
    await opens(socket, 4);

    act(() => {
      result.current.acknowledge(1);
    });
    const [sent] = socket.requestsOfKind("alerts_acknowledge");

    expect(sent?.body).toEqual({ kind: "alerts_acknowledge", subscription: 4, contact: 1 });
    expect(result.current.acknowledgements.get(1)).toEqual({ kind: "pending" });
    expect(result.current.contacts[0]?.acknowledged).toBe(false);

    act(() => {
      socket.serverAnswers("alerts_acknowledge", () => ({ kind: "alerts_acknowledge" }));
    });
    await settle();

    expect(result.current.acknowledgements.has(1)).toBe(false);
    expect(result.current.contacts[0]?.acknowledged).toBe(true);
  });

  it("clears a pending acknowledgement when the push of its contact comes first", async () => {
    const { socket, result } = renderAlerts();
    await opens(socket, 4);

    act(() => {
      result.current.acknowledge(1);
      socket.serverNotifies(4, alertsNotification(1, [aBearingContact({ acknowledged: true })]));
    });

    expect(result.current.acknowledgements.has(1)).toBe(false);
    expect(result.current.contacts[0]?.acknowledged).toBe(true);
  });

  it("holds a refused acknowledgement's reason, and sends none for an acknowledged contact", async () => {
    const { socket, result } = renderAlerts();
    await opens(socket, 4, [
      aBearingContact(),
      aResolvedContact([0, 26_100, 0], { acknowledged: true }),
    ]);

    act(() => {
      result.current.acknowledge(2);
      result.current.acknowledge(1);
      result.current.acknowledge(1);
    });
    const sent = socket.requestsOfKind("alerts_acknowledge");
    expect(sent.map(({ body }) => body.contact)).toEqual([1]);

    act(() => {
      socket.serverRejects(sent[0]?.id ?? 0, {
        code: "bad_request",
        message: "no such contact",
        field: "contact",
      });
    });
    await settle();

    expect(result.current.acknowledgements.get(1)).toEqual({
      kind: "rejected",
      reason: "no such contact",
    });
  });

  it("times out an acknowledgement the server never answers", async () => {
    const { socket, result } = renderAlerts();
    await opens(socket, 4);

    act(() => {
      result.current.acknowledge(1);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(REQUEST_TIMEOUT_MS);
    });

    expect(result.current.acknowledgements.get(1)).toEqual({ kind: "timed_out" });
  });

  it("refuses an opening state whose contact cannot be shown", async () => {
    const { socket, result } = renderAlerts();
    await opens(socket, 4, [aBearingContact({ flux_w_m2: Number.NaN })]);

    expect(result.current.status).toMatchObject({ kind: "rejected", code: "unusable" });
    expect(result.current.contacts).toEqual([]);
  });
});
