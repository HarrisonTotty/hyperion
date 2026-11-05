import { describe, expect, it, vi } from "vitest";

import { FakeLinkWorker } from "../test/FakeLinkWorker";
import { FakeWebSocket } from "../test/FakeWebSocket";
import { openLinkSocket, WorkerLinkSocket } from "./linkSocket";

const SERVER_URL = "ws://ship/ws";

/** A socket over a fake worker, with the events it dispatched, by type and data. */
function workerSocket(): {
  socket: WorkerLinkSocket;
  worker: FakeLinkWorker;
  events: Array<{ readonly type: string; readonly data: unknown }>;
} {
  const socket = new WorkerLinkSocket(SERVER_URL, () => new FakeLinkWorker("fake"));
  const events: Array<{ readonly type: string; readonly data: unknown }> = [];
  for (const type of ["open", "message", "close"] as const) {
    socket.addEventListener(type, (event) => {
      events.push({ type, data: event instanceof MessageEvent ? event.data : undefined });
    });
  }
  return { socket, worker: FakeLinkWorker.latest(), events };
}

describe("WorkerLinkSocket", () => {
  it("opens its worker's socket to its URL", () => {
    const { worker } = workerSocket();

    expect(worker.posted).toEqual([{ kind: "open", url: SERVER_URL }]);
  });

  it("dispatches the worker's reports as the socket's own events, in their order", () => {
    const { worker, events } = workerSocket();
    const frame = new ArrayBuffer(4);

    worker.reports({ kind: "open" });
    worker.reports({ kind: "message", data: "text" });
    worker.reports({ kind: "message", data: frame });

    expect(events).toEqual([
      { type: "open", data: undefined },
      { type: "message", data: "text" },
      { type: "message", data: frame },
    ]);
  });

  it("posts each send and its close to the worker", () => {
    const { socket, worker } = workerSocket();

    socket.send('{"type":"ping","nonce":0}');
    socket.close();

    expect(worker.posted.slice(1)).toEqual([
      { kind: "send", data: '{"type":"ping","nonce":0}' },
      { kind: "close" },
    ]);
  });

  it("stops its worker once its socket closes, and dispatches nothing after", () => {
    const { worker, events } = workerSocket();

    worker.reports({ kind: "close" });
    worker.reports({ kind: "message", data: "late" });

    expect(events).toEqual([{ type: "close", data: undefined }]);
    expect(worker.terminated).toBe(true);
  });

  it("closes, so that the link tries again, when its worker fails to load", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { worker, events } = workerSocket();

    worker.fails();

    expect(events).toEqual([{ type: "close", data: undefined }]);
    expect(worker.terminated).toBe(true);
    expect(error).toHaveBeenCalledWith("the server link's socket worker failed to load");
  });
});

describe("openLinkSocket", () => {
  it("opens a worker's socket where the page can start a worker", () => {
    FakeLinkWorker.instances = [];
    vi.stubGlobal("Worker", FakeLinkWorker);

    const socket = openLinkSocket(SERVER_URL);

    expect(socket).toBeInstanceOf(WorkerLinkSocket);
    expect(FakeLinkWorker.latest().options).toEqual({ type: "module", name: "server link" });
  });

  it("opens the page's own socket on a page with no Worker, a test's", () => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("Worker", undefined);
    vi.stubGlobal("WebSocket", FakeWebSocket);

    const socket = openLinkSocket(SERVER_URL);

    expect(socket).toBe(FakeWebSocket.latest());
    expect(FakeWebSocket.latest().url).toBe(SERVER_URL);
  });
});
