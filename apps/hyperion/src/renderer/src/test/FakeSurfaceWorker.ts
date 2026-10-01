import { vi } from "vitest";

import type { SurfaceReply, SurfaceRequest } from "../wasm/handleRequest";
import type { SurfaceWorker } from "../wasm/loadSurfaceModule";

/** What a {@link FakeSurfaceWorker} does with a request. */
export type FakeSurfaceAnswer =
  /** Replies with this message. */
  | SurfaceReply
  /** Fires an `ErrorEvent`, as a worker whose script threw. */
  | "error"
  /** Fires a plain `Event`, as a module worker whose script failed to load. */
  | "load-error"
  /** Never answers. */
  | "silence";

/**
 * A stand-in for the surface worker, for the loader's tests: it records the requests and answers
 * each, a microtask later, as `answer` says.
 */
export class FakeSurfaceWorker implements SurfaceWorker {
  readonly requests: SurfaceRequest[] = [];
  terminated = false;
  private readonly events = new EventTarget();

  constructor(private readonly answer: FakeSurfaceAnswer) {}

  addEventListener(type: "message", listener: (event: MessageEvent<SurfaceReply>) => void): void;
  addEventListener(type: "error", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<SurfaceReply>) => void) | ((event: Event) => void),
  ): void {
    this.events.addEventListener(type, { handleEvent: listener });
  }

  postMessage(message: SurfaceRequest): void {
    this.requests.push(message);
    const answer = this.answer;
    queueMicrotask(() => {
      if (this.terminated) {
        return;
      }
      if (answer === "error") {
        this.events.dispatchEvent(new ErrorEvent("error", { message: "worker script failed" }));
      } else if (answer === "load-error") {
        this.events.dispatchEvent(new Event("error"));
      } else if (answer !== "silence") {
        this.events.dispatchEvent(new MessageEvent("message", { data: answer }));
      }
    });
  }

  terminate(): void {
    this.terminated = true;
  }
}

/**
 * Stubs the global `Worker` with a constructor that makes {@link FakeSurfaceWorker}s answering
 * `answer`, for code that creates its worker itself. Returns the workers it makes, in order.
 */
export function stubSurfaceWorker(answer: FakeSurfaceAnswer): readonly FakeSurfaceWorker[] {
  const created: FakeSurfaceWorker[] = [];
  class StubWorker extends FakeSurfaceWorker {
    constructor() {
      super(answer);
      created.push(this);
    }
  }
  vi.stubGlobal("Worker", StubWorker);
  return created;
}
