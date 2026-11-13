/**
 * The server link's socket on a worker's thread (fix-night-sky-stars): the worker's half, and the
 * messages between it and the page's (`linkSocket.ts`).
 *
 * @remarks
 * A browser reads a `WebSocket`'s frames on the thread that made it. The page's thread draws
 * `VIEW`, and while a task of its own ran there the link's socket went unread: the browser's buffer
 * and then TCP's window filled, and the server closed the connection once one of its 262,144-byte
 * frames had waited past its 10 s write timeout (`WRITE_TIMEOUT`, plan 04, P04.T15), often in a
 * sky's reply, which the link then asked again from the start, so that the stars never arrived
 * (the owner's report, 2026-11-05). A worker's thread runs no task of the page's: it reads each
 * frame as it comes and posts it on, and the page's frames wait in its own memory while it is busy.
 */

/** What the page asks of the worker's socket. */
export type LinkSocketCommand =
  | { readonly kind: "open"; readonly url: string }
  | { readonly kind: "send"; readonly data: string }
  | { readonly kind: "close" };

/** What the worker's socket reports to the page, in the order it happened. */
export type LinkSocketReport =
  | { readonly kind: "open" }
  | { readonly kind: "message"; readonly data: string | ArrayBuffer }
  | { readonly kind: "close" };

/**
 * The part of a `WebSocket` the worker uses, so that a test can stand in for one: its `open`,
 * `message` and `close` events, a `message`'s event a `MessageEvent`.
 */
export interface WorkerSocket {
  binaryType: BinaryType;
  addEventListener(type: "open" | "message" | "close", listener: (event: Event) => void): void;
  send(data: string): void;
  close(): void;
}

/** The part of a dedicated worker's scope the worker's half uses. */
export interface LinkSocketScope {
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<LinkSocketCommand>) => void,
  ): void;
  postMessage(report: LinkSocketReport, transfer: Transferable[]): void;
}

/**
 * Serves the page's commands on `scope` with one socket: opened at the page's `open`, its events
 * posted back as they come, a binary frame's `ArrayBuffer` transferred uncopied.
 *
 * @param open - Makes the socket: the worker's `WebSocket` by default.
 * @remarks
 * A socket that cannot be made (a malformed URL) is reported closed, so that the link tries again
 * after its delay, as it does after any close.
 */
export function serveLinkSocket(
  scope: LinkSocketScope,
  open: (url: string) => WorkerSocket = (url) => new WebSocket(url),
): void {
  let socket: WorkerSocket | null = null;
  scope.addEventListener("message", (event) => {
    const command = event.data;
    switch (command.kind) {
      case "open": {
        let opened: WorkerSocket;
        try {
          opened = open(command.url);
        } catch {
          scope.postMessage({ kind: "close" }, []);
          return;
        }
        // Each binary frame one `ArrayBuffer`, handed over whole (R03, Design note 10).
        opened.binaryType = "arraybuffer";
        opened.addEventListener("open", () => {
          scope.postMessage({ kind: "open" }, []);
        });
        opened.addEventListener("message", (message) => {
          if (!(message instanceof MessageEvent)) {
            return;
          }
          const data: unknown = message.data;
          if (data instanceof ArrayBuffer) {
            scope.postMessage({ kind: "message", data }, [data]);
          } else if (typeof data === "string") {
            scope.postMessage({ kind: "message", data }, []);
          }
        });
        opened.addEventListener("close", () => {
          scope.postMessage({ kind: "close" }, []);
        });
        socket = opened;
        break;
      }
      case "send":
        socket?.send(command.data);
        break;
      case "close":
        socket?.close();
        break;
    }
  });
}
