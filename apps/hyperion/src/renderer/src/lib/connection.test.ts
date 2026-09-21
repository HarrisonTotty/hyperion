import { PROTOCOL_VERSION, type ResponseBody } from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FakeWebSocket } from "../test/FakeWebSocket";
import { useServerConnection } from "./connection";

const SERVER_URL = "ws://ship/ws";

const PING_INTERVAL_MS = 2_000;

const LIST: ResponseBody = { kind: "list_universes", universes: [], server_generator_version: 2 };

/** Renders the hook and returns it with the socket it opened. */
function renderConnection(): {
  result: { readonly current: ReturnType<typeof useServerConnection> };
  socket: FakeWebSocket;
  unmount: () => void;
} {
  const { result, unmount } = renderHook(() => useServerConnection(SERVER_URL, "1.2.3"));
  return { result, socket: FakeWebSocket.latest(), unmount };
}

/** Plays the server's side of a successful handshake at `protocolVersion`. */
function welcome(socket: FakeWebSocket, protocolVersion: number): void {
  act(() => {
    socket.serverOpens();
    socket.serverSends({
      type: "welcome",
      server_version: "9.9.9",
      protocol_version: protocolVersion,
      generator_version: 2,
    });
  });
}

/** The messages of `type` that the client has sent. */
function sentOfType(socket: FakeWebSocket, type: string): unknown[] {
  return socket.sent.filter(
    (message) =>
      typeof message === "object" && message !== null && "type" in message && message.type === type,
  );
}

describe("useServerConnection", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("sends a request made while connected and resolves it with the server's response", async () => {
    const { result, socket } = renderConnection();
    welcome(socket, PROTOCOL_VERSION);

    const pending = result.current.requests.request({ kind: "list_universes" });
    expect(sentOfType(socket, "request")).toEqual([
      { type: "request", id: 1, body: { kind: "list_universes" } },
    ]);
    act(() => {
      socket.serverResponds(1, LIST);
    });

    await expect(pending.outcome).resolves.toEqual({ ok: true, response: LIST });
  });

  it("passes a server's refusal to the request", async () => {
    const { result, socket } = renderConnection();
    welcome(socket, PROTOCOL_VERSION);

    const pending = result.current.requests.request({ kind: "list_universes" });
    act(() => {
      socket.serverRejects(1, { code: "queue_full", message: "busy", field: null });
    });

    await expect(pending.outcome).resolves.toEqual({
      ok: false,
      error: { code: "queue_full", message: "busy", field: null },
    });
  });

  it("settles a request made while disconnected as link_lost without sending it", async () => {
    const { result, socket } = renderConnection();
    welcome(socket, PROTOCOL_VERSION);
    act(() => {
      socket.close();
    });

    const pending = result.current.requests.request({ kind: "list_universes" });

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "link_lost" },
    });
    expect(sentOfType(socket, "request")).toEqual([]);
  });

  it("settles a request made before the welcome as link_lost", async () => {
    const { result, socket } = renderConnection();
    act(() => {
      socket.serverOpens();
    });

    const pending = result.current.requests.request({ kind: "list_universes" });

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "link_lost" },
    });
    expect(sentOfType(socket, "request")).toEqual([]);
  });

  it("settles pending requests as link_lost when the socket closes", async () => {
    const { result, socket } = renderConnection();
    welcome(socket, PROTOCOL_VERSION);
    const pending = result.current.requests.request({ kind: "list_universes" });

    act(() => {
      socket.close();
    });

    expect(result.current.status).toBe("disconnected");
    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "link_lost" },
    });
  });

  it("settles pending requests as link_lost when it unmounts", async () => {
    const { result, socket, unmount } = renderConnection();
    welcome(socket, PROTOCOL_VERSION);
    const pending = result.current.requests.request({ kind: "list_universes" });

    unmount();

    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "link_lost" },
    });
  });

  it("keeps one request client across a reconnect", () => {
    vi.useFakeTimers();
    const { result, socket } = renderConnection();
    welcome(socket, PROTOCOL_VERSION);
    const before = result.current.requests;

    act(() => {
      socket.close();
      vi.advanceTimersByTime(PING_INTERVAL_MS);
    });
    const reconnected = FakeWebSocket.latest();
    welcome(reconnected, PROTOCOL_VERSION);
    result.current.requests.request({ kind: "list_universes" });

    expect(reconnected).not.toBe(socket);
    expect(result.current.requests).toBe(before);
    expect(sentOfType(reconnected, "request")).toHaveLength(1);
  });

  it("keeps requests on a new socket when a replaced socket's close arrives late", async () => {
    const { result, rerender } = renderHook(
      ({ url }: { readonly url: string }) => useServerConnection(url, "1.2.3"),
      { initialProps: { url: SERVER_URL } },
    );
    const replaced = FakeWebSocket.latest();
    welcome(replaced, PROTOCOL_VERSION);
    // A browser fires a closed socket's close event only once the closing handshake is over.
    vi.spyOn(replaced, "close").mockImplementation(() => undefined);

    rerender({ url: "ws://ship/ws2" });
    const current = FakeWebSocket.latest();
    welcome(current, PROTOCOL_VERSION);
    const pending = result.current.requests.request({ kind: "list_universes" });
    act(() => {
      replaced.dispatchEvent(new Event("close"));
      current.serverResponds(1, LIST);
    });

    expect(current).not.toBe(replaced);
    await expect(pending.outcome).resolves.toEqual({ ok: true, response: LIST });
  });

  it("reports both versions from the server's welcome", () => {
    const { result, socket } = renderConnection();

    welcome(socket, PROTOCOL_VERSION);

    expect(result.current).toMatchObject({
      status: "connected",
      serverVersion: "9.9.9",
      serverProtocolVersion: PROTOCOL_VERSION,
      serverGeneratorVersion: 2,
    });
  });

  it("marks a server of another protocol version incompatible and never pings it", async () => {
    vi.useFakeTimers();
    const { result, socket } = renderConnection();

    welcome(socket, 1);
    act(() => {
      vi.advanceTimersByTime(3 * PING_INTERVAL_MS);
    });

    expect(result.current.status).toBe("incompatible");
    expect(result.current.serverProtocolVersion).toBe(1);
    expect(sentOfType(socket, "ping")).toEqual([]);
    expect(FakeWebSocket.instances).toHaveLength(1);
    const pending = result.current.requests.request({ kind: "list_universes" });
    await expect(pending.outcome).resolves.toMatchObject({
      ok: false,
      error: { code: "link_lost" },
    });
  });
});
