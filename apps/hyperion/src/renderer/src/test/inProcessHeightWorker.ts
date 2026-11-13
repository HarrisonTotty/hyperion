import { answerRequest, type HeightModule, WorkerFields } from "../view/terrain/workers/heightBake";
import type { HeightWorkerReply, HeightWorkerRequest } from "../view/terrain/workers/messages";
import type { WorkerLike } from "../view/terrain/workers/pool";

/**
 * A height worker run on the test's own thread (plan R10, R10.T14.c): each request answered on a
 * later task by the real worker's own logic (`heightBake.ts`' `answerRequest`) over the real
 * surface module and its {@link WorkerFields}, as `height.worker.ts` answers in its module worker.
 *
 * @remarks
 * The module is the caller's: only workers and tests import `generated/surface/`
 * (`wasm/surfaceImports.test.ts`), so the test builds its `HeightModule` from the generated glue
 * (`initSync` over the inlined `.wasm`, as `fieldsWasm.test.ts` does) and passes it in. Replies are
 * neither cloned nor their buffers detached, which the pool does not notice. A trap the module
 * raises is the worker's `error` event, as `height.worker.ts` raises it, and is logged.
 */
export class InProcessHeightWorker implements WorkerLike {
  readonly #module: HeightModule;
  readonly #fields: WorkerFields;
  readonly #events = new EventTarget();
  readonly #wrappers = new Map<unknown, EventListenerObject>();
  readonly #timers = new Set<ReturnType<typeof setTimeout>>();
  #terminated = false;
  /** How many requests of each kind it has answered. */
  readonly answered = new Map<HeightWorkerRequest["kind"], number>();

  constructor(module: HeightModule) {
    this.#module = module;
    this.#fields = new WorkerFields(module);
  }

  postMessage(message: HeightWorkerRequest): void {
    if (this.#terminated) {
      return;
    }
    const timer = setTimeout(() => {
      this.#timers.delete(timer);
      if (this.#terminated) {
        return;
      }
      let reply: HeightWorkerReply;
      try {
        reply = answerRequest(this.#module, message, this.#fields).reply;
      } catch (error: unknown) {
        console.error("the in-process height worker trapped:", error);
        this.#events.dispatchEvent(new Event("error"));
        return;
      }
      this.answered.set(message.kind, (this.answered.get(message.kind) ?? 0) + 1);
      this.#events.dispatchEvent(new MessageEvent("message", { data: reply }));
    }, 0);
    this.#timers.add(timer);
  }

  /** Ends the worker: its answers still to come are dropped and its fields freed. */
  terminate(): void {
    if (this.#terminated) {
      return;
    }
    this.#terminated = true;
    for (const timer of this.#timers) {
      clearTimeout(timer);
    }
    this.#timers.clear();
    this.#fields.free();
  }

  addEventListener(
    type: "message",
    listener: (event: MessageEvent<HeightWorkerReply>) => void,
  ): void;
  addEventListener(type: "error", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<HeightWorkerReply>) => void) | ((event: Event) => void),
  ): void {
    const wrapper = { handleEvent: listener };
    this.#wrappers.set(listener, wrapper);
    this.#events.addEventListener(type, wrapper);
  }

  removeEventListener(
    type: "message",
    listener: (event: MessageEvent<HeightWorkerReply>) => void,
  ): void;
  removeEventListener(type: "error", listener: (event: Event) => void): void;
  removeEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<HeightWorkerReply>) => void) | ((event: Event) => void),
  ): void {
    const wrapper = this.#wrappers.get(listener);
    if (wrapper !== undefined) {
      this.#wrappers.delete(listener);
      this.#events.removeEventListener(type, wrapper);
    }
  }
}
