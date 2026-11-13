import { describe, expect, it } from "vitest";

import { FakeWebSocket } from "../test/FakeWebSocket";
import {
  type LinkSocketCommand,
  type LinkSocketReport,
  type LinkSocketScope,
  serveLinkSocket,
} from "./linkSocketWorker";

/** A stand-in for the worker's scope: the page's commands in, the reports and transfers out. */
class FakeScope implements LinkSocketScope {
  readonly reports: LinkSocketReport[] = [];
  readonly transfers: Transferable[][] = [];
  readonly #events = new EventTarget();

  addEventListener(
    type: "message",
    listener: (event: MessageEvent<LinkSocketCommand>) => void,
  ): void {
    this.#events.addEventListener(type, { handleEvent: listener });
  }

  postMessage(report: LinkSocketReport, transfer: Transferable[]): void {
    this.reports.push(report);
    this.transfers.push(transfer);
  }

  /** The page posts `command` to the worker. */
  commands(command: LinkSocketCommand): void {
    this.#events.dispatchEvent(new MessageEvent("message", { data: command }));
  }
}

/** A served scope whose socket is a fake, opened. */
function served(): { scope: FakeScope; socket: FakeWebSocket } {
  const scope = new FakeScope();
  const opened: FakeWebSocket[] = [];
  serveLinkSocket(scope, (url) => {
    const socket = new FakeWebSocket(url);
    opened.push(socket);
    return socket;
  });
  scope.commands({ kind: "open", url: "ws://ship/ws" });
  const socket = opened[0];
  if (socket === undefined) {
    throw new Error("the worker opened no socket");
  }
  return { scope, socket };
}

describe("serveLinkSocket", () => {
  it("opens its socket at the page's URL, every binary frame an ArrayBuffer", () => {
    const { socket } = served();

    expect(socket.url).toBe("ws://ship/ws");
    expect(socket.binaryType).toBe("arraybuffer");
  });

  it("posts each of its socket's events to the page as it comes, a frame's buffer transferred", () => {
    const { scope, socket } = served();
    const frame = new ArrayBuffer(8);

    socket.serverOpens();
    socket.serverSendsBinary(frame);
    socket.serverSends({ type: "pong", nonce: 3 });
    socket.close();

    expect(scope.reports).toEqual([
      { kind: "open" },
      { kind: "message", data: frame },
      { kind: "message", data: JSON.stringify({ type: "pong", nonce: 3 }) },
      { kind: "close" },
    ]);
    expect(scope.transfers).toEqual([[], [frame], [], []]);
  });

  it("sends what the page sends, and closes its socket when the page closes it", () => {
    const { scope, socket } = served();

    scope.commands({ kind: "send", data: '{"type":"ping","nonce":0}' });
    scope.commands({ kind: "close" });

    expect(socket.sent).toEqual([{ type: "ping", nonce: 0 }]);
    expect(scope.reports).toEqual([{ kind: "close" }]);
  });

  it("reports a socket it cannot make as closed, so that the link tries again", () => {
    const scope = new FakeScope();
    serveLinkSocket(scope, () => {
      throw new SyntaxError("not a WebSocket URL");
    });

    scope.commands({ kind: "open", url: "nowhere" });

    expect(scope.reports).toEqual([{ kind: "close" }]);
  });
});
