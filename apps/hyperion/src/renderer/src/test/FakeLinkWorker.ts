import type { LinkSocketWorker } from "../lib/linkSocket";
import type { LinkSocketCommand, LinkSocketReport } from "../lib/linkSocketWorker";

/**
 * In-memory stand-in for the server link's socket worker (`linkSocket.worker.ts`), letting tests
 * see what the page posts it and play the worker's side: its socket's events, posted to the page.
 */
export class FakeLinkWorker implements LinkSocketWorker {
  static instances: FakeLinkWorker[] = [];

  readonly posted: LinkSocketCommand[] = [];
  terminated = false;
  readonly #events = new EventTarget();

  constructor(
    readonly script: URL | string,
    readonly options?: WorkerOptions,
  ) {
    FakeLinkWorker.instances.push(this);
  }

  static latest(): FakeLinkWorker {
    const worker = FakeLinkWorker.instances.at(-1);
    if (worker === undefined) {
      throw new Error("no worker has been started");
    }
    return worker;
  }

  addEventListener(
    type: "message",
    listener: (event: MessageEvent<LinkSocketReport>) => void,
  ): void;
  addEventListener(type: "error", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<LinkSocketReport>) => void) | ((event: Event) => void),
  ): void {
    this.#events.addEventListener(type, { handleEvent: listener });
  }

  postMessage(command: LinkSocketCommand): void {
    this.posted.push(command);
  }

  terminate(): void {
    this.terminated = true;
  }

  /** Posts the worker's report of its socket's event to the page. */
  reports(report: LinkSocketReport): void {
    this.#events.dispatchEvent(new MessageEvent("message", { data: report }));
  }

  /** The worker failing to load, as a browser reports it. */
  fails(): void {
    this.#events.dispatchEvent(new Event("error"));
  }

  /** The messages the page asked the worker's socket to send, parsed. */
  sent(): unknown[] {
    const sent: unknown[] = [];
    for (const command of this.posted) {
      if (command.kind === "send") {
        const message: unknown = JSON.parse(command.data);
        sent.push(message);
      }
    }
    return sent;
  }
}
