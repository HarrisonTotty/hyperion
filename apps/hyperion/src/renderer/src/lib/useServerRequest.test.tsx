import {
  type PendingRequest,
  RequestClient,
  type RequestOf,
  type RequestOutcome,
  type ResponseFor,
} from "@hyperion/protocol";
import { act, render, renderHook, screen } from "@testing-library/react";
import { Activity } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FakeWebSocket } from "../test/FakeWebSocket";
import { someGalaxyParameters } from "../test/galaxyFixtures";
import { ServerLinkHarness } from "../test/ServerLinkHarness";
import type { ConnectionStatus } from "./connection";
import { ServerLinkContext } from "./serverLink";
import {
  followRequest,
  REQUEST_TIMEOUT_MS,
  type SettledRequestState,
  useServerRequest,
} from "./useServerRequest";

const RECONNECT_DELAY_MS = 2_000;

type Body = RequestOf<"galaxy_parameters"> | null;

function parametersOf(universe: string): RequestOf<"galaxy_parameters"> {
  return { kind: "galaxy_parameters", universe };
}

const FIRST = "00000000000000a1";
const SECOND = "00000000000000b2";

interface HookProps {
  readonly body: Body;
  readonly timeoutMs?: number;
  readonly generation?: number;
}

/** Renders the hook under a server link, welcomed unless told otherwise. */
function renderRequest(body: Body, { welcomed = true }: { welcomed?: boolean } = {}) {
  const initial: HookProps = { body };
  const hook = renderHook(
    ({ body: current, timeoutMs, generation }: HookProps) =>
      useServerRequest(current, timeoutMs, generation),
    { initialProps: initial, wrapper: ServerLinkHarness },
  );
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

interface ProbeProps {
  readonly body: Body;
}

interface RefreshProbeProps {
  readonly body: RequestOf<"galaxy_parameters">;
  readonly generation: number;
}

/** Shows the state of a refreshed request, and the seed of its answer. */
function RefreshProbe({ body, generation }: RefreshProbeProps) {
  const state = useServerRequest(body, REQUEST_TIMEOUT_MS, generation);
  return (
    <output aria-label="Request">
      {state.kind} {state.kind === "ok" ? state.response.seed : ""}
    </output>
  );
}

/** Shows the state of one request, so that it can be unmounted while its link stays up. */
function Probe({ body }: ProbeProps) {
  const state = useServerRequest(body);
  return <output aria-label="Request">{state.kind}</output>;
}

describe("useServerRequest", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("is idle, and sends nothing, without a body", () => {
    const { result, socket } = renderRequest(null);

    expect(result.current).toEqual({ kind: "idle" });
    expect(socket.requestsOfKind("galaxy_parameters")).toEqual([]);
  });

  it("is pending once sent, then ok with the response", async () => {
    const { result, socket } = renderRequest(parametersOf(FIRST));

    expect(result.current).toEqual({ kind: "pending" });
    expect(socket.requestsOfKind("galaxy_parameters")).toEqual([
      { id: 1, body: parametersOf(FIRST) },
    ]);

    await server(() => {
      socket.serverAnswers("galaxy_parameters", (body) => someGalaxyParameters(body.universe));
    });

    expect(result.current).toEqual({ kind: "ok", response: someGalaxyParameters(FIRST) });
  });

  it("cancels the request of a changed body and ignores its late response", async () => {
    const { result, socket, rerender } = renderRequest(parametersOf(FIRST));

    rerender({ body: parametersOf(SECOND) });
    expect(socket.cancelledIds()).toEqual([1]);
    expect(socket.requestsOfKind("galaxy_parameters")).toEqual([
      { id: 1, body: parametersOf(FIRST) },
      { id: 2, body: parametersOf(SECOND) },
    ]);

    await server(() => {
      socket.serverResponds(1, someGalaxyParameters(FIRST));
    });
    expect(result.current).toEqual({ kind: "pending" });

    await server(() => {
      socket.serverResponds(2, someGalaxyParameters(SECOND));
    });
    expect(result.current).toEqual({ kind: "ok", response: someGalaxyParameters(SECOND) });
  });

  it("does not send again for an equal body built afresh", async () => {
    const { result, socket, rerender } = renderRequest(parametersOf(FIRST));
    await server(() => {
      socket.serverAnswers("galaxy_parameters", (body) => someGalaxyParameters(body.universe));
    });
    const answered = result.current;

    rerender({ body: parametersOf(FIRST) });

    expect(socket.requestsOfKind("galaxy_parameters")).toHaveLength(1);
    expect(result.current).toBe(answered);
  });

  it("reports the server's reason when the request is rejected", async () => {
    const { result, socket } = renderRequest(parametersOf(FIRST));

    await server(() => {
      socket.serverRejects(1, {
        code: "unknown_universe",
        message: "no universe 00000000000000a1",
        field: "universe",
      });
    });

    expect(result.current).toEqual({
      kind: "rejected",
      code: "unknown_universe",
      reason: "no universe 00000000000000a1",
    });
  });

  it.each([
    ["unknown_system", "system", "no system 0200080020000000 in this universe"],
    ["unknown_body", "body", "no body 0200080020000000.0300 in its system"],
  ] as const)(
    "reports %s as a rejection with the server's reason",
    async (code, field, message) => {
      const { result, socket } = renderRequest(parametersOf(FIRST));

      await server(() => {
        socket.serverRejects(1, { code, message, field });
      });

      expect(result.current).toEqual({ kind: "rejected", code, reason: message });
    },
  );

  it("rejects a response of the wrong kind as a protocol violation", async () => {
    const { result, socket } = renderRequest(parametersOf(FIRST));

    await server(() => {
      socket.serverResponds(1, {
        kind: "list_universes",
        universes: [],
        server_generator_version: 2,
      });
    });

    expect(result.current).toMatchObject({ kind: "rejected", code: "protocol_violation" });
  });

  it("times out after 30 s, cancelling the request on the wire", () => {
    vi.useFakeTimers();
    const { result, socket } = renderRequest(parametersOf(FIRST));

    act(() => {
      vi.advanceTimersByTime(REQUEST_TIMEOUT_MS - 1);
    });
    expect(result.current).toEqual({ kind: "pending" });
    act(() => {
      vi.advanceTimersByTime(1);
    });

    expect(result.current).toEqual({ kind: "timed_out" });
    expect(socket.cancelledIds()).toEqual([1]);
  });

  it("takes a timeout of its caller's", () => {
    vi.useFakeTimers();
    const { result, rerender } = renderRequest(null);
    rerender({ body: parametersOf(FIRST), timeoutMs: 120_000 });

    act(() => {
      vi.advanceTimersByTime(REQUEST_TIMEOUT_MS);
    });
    expect(result.current).toEqual({ kind: "pending" });
    act(() => {
      vi.advanceTimersByTime(120_000 - REQUEST_TIMEOUT_MS);
    });

    expect(result.current).toEqual({ kind: "timed_out" });
  });

  it("cancels the request in flight on unmount", () => {
    const { rerender } = render(
      <ServerLinkHarness>
        <Probe body={parametersOf(FIRST)} />
      </ServerLinkHarness>,
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    expect(screen.getByRole("status", { name: "Request" })).toHaveTextContent("pending");

    rerender(<ServerLinkHarness>{null}</ServerLinkHarness>);

    expect(socket.cancelledIds()).toEqual([1]);
  });

  it("reports the link's state and sends nothing while the link is down", () => {
    const { result, socket } = renderRequest(parametersOf(FIRST), { welcomed: false });

    expect(result.current).toEqual({ kind: "link_down", reason: "ESTABLISHING LINK" });
    expect(socket.requestsOfKind("galaxy_parameters")).toEqual([]);

    act(() => {
      socket.serverWelcomes();
    });

    expect(result.current).toEqual({ kind: "pending" });
    expect(socket.requestsOfKind("galaxy_parameters")).toHaveLength(1);
  });

  it("reads NO CARRIER when the socket closes, and sends again once the link returns", async () => {
    vi.useFakeTimers();
    const { result, socket } = renderRequest(parametersOf(FIRST));

    await server(() => {
      socket.close();
    });
    expect(result.current).toEqual({ kind: "link_down", reason: "NO CARRIER" });

    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });
    const reconnected = FakeWebSocket.latest();
    expect(reconnected).not.toBe(socket);
    act(() => {
      reconnected.serverWelcomes();
    });

    expect(reconnected.requestsOfKind("galaxy_parameters")).toEqual([
      { id: 2, body: parametersOf(FIRST) },
    ]);
    expect(result.current).toEqual({ kind: "pending" });
  });

  it("keeps an answer through a link loss without fetching it again", async () => {
    vi.useFakeTimers();
    const { result, socket } = renderRequest(parametersOf(FIRST));
    await server(() => {
      socket.serverAnswers("galaxy_parameters", (body) => someGalaxyParameters(body.universe));
    });

    await server(() => {
      socket.close();
    });
    expect(result.current).toEqual({ kind: "ok", response: someGalaxyParameters(FIRST) });
    act(() => {
      vi.advanceTimersByTime(RECONNECT_DELAY_MS);
    });
    act(() => {
      FakeWebSocket.latest().serverWelcomes();
    });

    expect(FakeWebSocket.latest().requestsOfKind("galaxy_parameters")).toEqual([]);
    expect(result.current).toEqual({ kind: "ok", response: someGalaxyParameters(FIRST) });
  });

  it("sends the same body again on a new generation, keeping the last answer until the next", async () => {
    const earlier = someGalaxyParameters(FIRST, "0000000000000001");
    const later = someGalaxyParameters(FIRST, "0000000000000002");
    const { result, socket, rerender } = renderRequest(parametersOf(FIRST));
    await server(() => {
      socket.serverAnswers("galaxy_parameters", () => earlier);
    });

    rerender({ body: parametersOf(FIRST), generation: 1 });
    expect(socket.requestsOfKind("galaxy_parameters")).toHaveLength(2);
    expect(result.current).toEqual({ kind: "ok", response: earlier });

    await server(() => {
      socket.serverAnswers("galaxy_parameters", () => later);
    });
    expect(result.current).toEqual({ kind: "ok", response: later });
  });

  it("keeps the last answer when the link is lost during a refresh", async () => {
    // A request client whose link the test drops by hand, so that the refresh can be lost before
    // the link's new state renders, which is the order a browser runs them in.
    const requests = new RequestClient(() => true);
    const earlier = someGalaxyParameters(FIRST, "0000000000000001");
    const probe = (status: ConnectionStatus, generation: number) => (
      <ServerLinkContext value={{ status, requests }}>
        <RefreshProbe body={parametersOf(FIRST)} generation={generation} />
      </ServerLinkContext>
    );
    const { rerender } = render(probe("connected", 0));
    await act(async () => {
      requests.handleServerMessage({ type: "response", id: 1, body: earlier });
      await Promise.resolve();
    });
    rerender(probe("connected", 1));

    await act(async () => {
      requests.linkLost();
      await Promise.resolve();
    });
    expect(screen.getByRole("status", { name: "Request" })).toHaveTextContent(
      "ok 0000000000000001",
    );
    rerender(probe("disconnected", 1));

    expect(screen.getByRole("status", { name: "Request" })).toHaveTextContent(
      "ok 0000000000000001",
    );
  });

  it("replaces the last answer with the failure of a refresh the server rejects", async () => {
    const { result, socket, rerender } = renderRequest(parametersOf(FIRST));
    await server(() => {
      socket.serverAnswers("galaxy_parameters", () => someGalaxyParameters(FIRST));
    });
    rerender({ body: parametersOf(FIRST), generation: 1 });

    await server(() => {
      socket.serverRejects(2, { code: "queue_full", message: "busy", field: null });
    });

    expect(result.current).toEqual({ kind: "rejected", code: "queue_full", reason: "busy" });
  });

  it.each([
    [
      "a rejection",
      (socket: FakeWebSocket) => {
        socket.serverRejects(1, { code: "queue_full", message: "busy", field: null });
      },
      "rejected",
    ],
    [
      "a timeout",
      () => {
        vi.advanceTimersByTime(REQUEST_TIMEOUT_MS);
      },
      "timed_out",
    ],
  ])(
    "keeps %s, and sends nothing, when its display is hidden and shown again",
    async (_failure, fail, shown) => {
      vi.useFakeTimers();
      const view = (mode: "visible" | "hidden") => (
        <ServerLinkHarness>
          <Activity mode={mode}>
            <Probe body={parametersOf(FIRST)} />
          </Activity>
        </ServerLinkHarness>
      );
      const { rerender } = render(view("visible"));
      const socket = FakeWebSocket.latest();
      act(() => {
        socket.serverWelcomes();
      });
      await server(() => {
        fail(socket);
      });

      rerender(view("hidden"));
      rerender(view("visible"));

      expect(socket.requestsOfKind("galaxy_parameters")).toHaveLength(1);
      expect(screen.getByRole("status", { name: "Request" })).toHaveTextContent(shown);
    },
  );

  it("keeps each state's identity across renders that change nothing", () => {
    const { result, rerender } = renderRequest(parametersOf(FIRST));
    const pending = result.current;

    rerender({ body: parametersOf(FIRST) });

    expect(result.current).toBe(pending);
  });
});

/** A request in flight whose outcome the test settles. */
function aPendingRequest(): {
  pending: PendingRequest<"list_universes">;
  settle: (outcome: RequestOutcome<"list_universes">) => void;
  cancel: ReturnType<typeof vi.fn<() => void>>;
} {
  const resolvers: { resolve?: (outcome: RequestOutcome<"list_universes">) => void } = {};
  const outcome = new Promise<RequestOutcome<"list_universes">>((resolve) => {
    resolvers.resolve = resolve;
  });
  const cancel = vi.fn<() => void>();
  return {
    pending: { outcome, cancel },
    settle: (settled) => {
      resolvers.resolve?.(settled);
    },
    cancel,
  };
}

const LIST_ANSWER = {
  kind: "list_universes",
  universes: [],
  server_generator_version: 2,
} as const satisfies ResponseFor<"list_universes">;

describe("followRequest", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("reports the state the outcome settles in", async () => {
    const { pending, settle } = aPendingRequest();
    const onSettled = vi.fn<(state: SettledRequestState<"list_universes">) => void>();
    followRequest(pending, REQUEST_TIMEOUT_MS, onSettled);

    settle({ ok: true, response: LIST_ANSWER });
    await pending.outcome;

    expect(onSettled).toHaveBeenCalledExactlyOnceWith({ kind: "ok", response: LIST_ANSWER });
  });

  it.each(["aborted", "superseded"] as const)(
    "drops an outcome the client caused: %s",
    async (code) => {
      const { pending, settle } = aPendingRequest();
      const onSettled = vi.fn<(state: SettledRequestState<"list_universes">) => void>();
      followRequest(pending, REQUEST_TIMEOUT_MS, onSettled);

      settle({ ok: false, error: { code, message: "ended by the client" } });
      await pending.outcome;

      expect(onSettled).not.toHaveBeenCalled();
    },
  );

  it("reports nothing once stopped, even for an outcome already on its way, and cancels", async () => {
    const { pending, settle, cancel } = aPendingRequest();
    const onSettled = vi.fn<(state: SettledRequestState<"list_universes">) => void>();
    const stop = followRequest(pending, REQUEST_TIMEOUT_MS, onSettled);

    settle({ ok: true, response: LIST_ANSWER });
    stop();
    await pending.outcome;

    expect(onSettled).not.toHaveBeenCalled();
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("cancels and reports timed_out once, when the timeout passes first", async () => {
    vi.useFakeTimers();
    const { pending, settle, cancel } = aPendingRequest();
    const onSettled = vi.fn<(state: SettledRequestState<"list_universes">) => void>();
    followRequest(pending, 5_000, onSettled);

    vi.advanceTimersByTime(5_000);
    settle({ ok: false, error: { code: "aborted", message: "the request was cancelled" } });
    await pending.outcome;

    expect(cancel).toHaveBeenCalledOnce();
    expect(onSettled).toHaveBeenCalledExactlyOnceWith({ kind: "timed_out" });
  });
});
