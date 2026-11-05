/**
 * The server link's socket (fix-night-sky-stars): the app's read on a worker's thread
 * (`linkSocketWorker.ts` says why), or the page's own `WebSocket`.
 */

import type { LinkSocketCommand, LinkSocketReport } from "./linkSocketWorker";

/**
 * The part of a `WebSocket` the link uses (`useServerConnection`): its `open`, `message` and
 * `close` events, a `message`'s event a `MessageEvent`.
 */
export interface LinkSocket {
  binaryType: BinaryType;
  addEventListener(type: "open" | "message" | "close", listener: (event: Event) => void): void;
  send(data: string): void;
  close(): void;
}

/** The part of a `Worker` the page's half uses, so that a test can stand in for one. */
export interface LinkSocketWorker {
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<LinkSocketReport>) => void,
  ): void;
  addEventListener(type: "error", listener: (event: Event) => void): void;
  postMessage(command: LinkSocketCommand, transfer: Transferable[]): void;
  terminate(): void;
}

/** Starts the link's socket worker, a same-origin module file as the sky's decode worker is. */
function startLinkSocketWorker(): LinkSocketWorker {
  return new Worker(new URL("./linkSocket.worker.ts", import.meta.url), {
    type: "module",
    name: "server link",
  });
}

/**
 * The page's half of a socket read on a worker's thread: each of the worker's reports dispatched
 * as the socket's own event, and each send and close posted to it.
 *
 * @remarks
 * A binary frame arrives as an `ArrayBuffer` whatever `binaryType` is set to: the worker's socket
 * takes every one so. Once closed, by the server, the page or the worker failing to load, it
 * stops its worker and dispatches nothing more.
 */
export class WorkerLinkSocket extends EventTarget implements LinkSocket {
  binaryType: BinaryType = "arraybuffer";
  readonly #worker: LinkSocketWorker;
  #closed = false;

  /**
   * Starts the worker and opens its socket to `url`.
   *
   * @param start - Starts the worker: the module worker's by default.
   */
  constructor(url: string, start: () => LinkSocketWorker = startLinkSocketWorker) {
    super();
    this.#worker = start();
    this.#worker.addEventListener("message", (event) => {
      this.#report(event.data);
    });
    this.#worker.addEventListener("error", () => {
      console.error("the server link's socket worker failed to load");
      this.#report({ kind: "close" });
    });
    this.#worker.postMessage({ kind: "open", url }, []);
  }

  send(data: string): void {
    this.#worker.postMessage({ kind: "send", data }, []);
  }

  close(): void {
    this.#worker.postMessage({ kind: "close" }, []);
  }

  #report(report: LinkSocketReport): void {
    if (this.#closed) {
      return;
    }
    switch (report.kind) {
      case "open":
        this.dispatchEvent(new Event("open"));
        break;
      case "message":
        this.dispatchEvent(new MessageEvent("message", { data: report.data }));
        break;
      case "close":
        this.#closed = true;
        this.#worker.terminate();
        this.dispatchEvent(new Event("close"));
        break;
    }
  }
}

/** The page's own socket to `url`, read on the page's thread. */
export function openPageSocket(url: string): LinkSocket {
  return new WebSocket(url);
}

/**
 * The app's link socket to `url`: a worker's, else, on a page with no `Worker` (a test's under
 * jsdom), the page's own.
 */
export function openLinkSocket(url: string): LinkSocket {
  return typeof Worker === "function" ? new WorkerLinkSocket(url) : openPageSocket(url);
}
