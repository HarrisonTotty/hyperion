/**
 * The render thread's side of the mesh page producer (plan R14, R14.T5.b, Design note 14): a
 * {@link MeshPageProducer} over the producer's worker (`meshPage.worker.ts`), and one on this
 * thread over the same {@link MeshFileReader}, which tests and the fly-by take.
 *
 * @remarks
 * Each request is numbered and its reply matched by number. A worker that fails, to load or to
 * answer, settles every request waiting with a refusal naming the reason, and refuses every later
 * one with it; a reply that cannot be read settles every request waiting, as does a request the
 * worker cannot take; after {@link MeshPageProducer.dispose} every request is refused.
 */

import {
  type FetchBytes,
  MeshFileReader,
  type MeshPageReply,
  type MeshPageRequest,
} from "./meshPageWork";
import type { FormatRead, MeshIndex, MeshPage, MeshPageEntry } from "./pageFormat";
import type { MeshPageProducer } from "./residency";

/** What the producer needs of its worker: `Worker`'s own, so that a test may stand in. */
export interface MeshPageWorker {
  addEventListener(type: "message", listener: (event: MessageEvent<MeshPageReply>) => void): void;
  addEventListener(type: "error" | "messageerror", listener: (event: Event) => void): void;
  postMessage(message: MeshPageRequest, transfer: Transferable[]): void;
  terminate(): void;
}

/** Starts the producer's module worker, a same-origin module file as the sky's is. */
function startMeshPageWorker(): MeshPageWorker {
  return new Worker(new URL("./meshPage.worker.ts", import.meta.url), {
    type: "module",
    name: "mesh pages",
  });
}

/** A request's refusal of `reason`. */
function refused(reason: string): { readonly kind: "refused"; readonly reason: string } {
  return { kind: "refused", reason };
}

/**
 * A producer over the mesh page worker (see the module's remarks).
 *
 * @param start - Starts the worker; the module worker's by default.
 */
export function createWorkerMeshPageProducer(
  start: () => MeshPageWorker = startMeshPageWorker,
): MeshPageProducer {
  const worker = start();
  const waiting = new Map<number, (reply: MeshPageReply | string) => void>();
  let disposed = false;
  // Why the worker failed, once it has: every later request is refused with it.
  let failed: string | null = null;
  const failAll = (reason: string): void => {
    for (const settle of waiting.values()) {
      settle(reason);
    }
    waiting.clear();
  };
  worker.addEventListener("message", (event: MessageEvent<MeshPageReply>) => {
    const settle = waiting.get(event.data.id);
    waiting.delete(event.data.id);
    settle?.(event.data);
  });
  worker.addEventListener("error", (event: Event) => {
    failed = event instanceof ErrorEvent ? event.message : "the mesh page worker failed";
    failAll(failed);
  });
  worker.addEventListener("messageerror", () => {
    failAll("the mesh page worker's reply could not be read");
  });
  let next = 0;
  const ask = (request: (id: number) => MeshPageRequest): Promise<MeshPageReply | string> =>
    new Promise((resolve) => {
      if (disposed || failed !== null) {
        resolve(failed ?? "the mesh page producer is stopped");
        return;
      }
      next += 1;
      const id = next;
      waiting.set(id, resolve);
      try {
        worker.postMessage(request(id), []);
      } catch (error: unknown) {
        waiting.delete(id);
        resolve(error instanceof Error ? error.message : "the request could not be posted");
      }
    });
  return {
    index: async (url) => {
      const reply = await ask((id) => ({ kind: "index", id, url }));
      if (typeof reply === "string") {
        return refused(reply);
      }
      return reply.kind === "index" ? reply.read : refused("the worker answered with a page");
    },
    page: async (url, entry, page) => {
      const reply = await ask((id) => ({ kind: "page", id, url, entry, page }));
      if (typeof reply === "string") {
        return refused(reply);
      }
      return reply.kind === "page" ? reply.read : refused("the worker answered with an index");
    },
    forget: (url) => {
      if (!disposed && failed === null) {
        worker.postMessage({ kind: "forget", url }, []);
      }
    },
    dispose: () => {
      disposed = true;
      failAll("the mesh page producer is stopped");
      worker.terminate();
    },
  };
}

/** A producer on this thread over `fetchBytes` (see the module's remarks). */
export class InThreadMeshPageProducer implements MeshPageProducer {
  readonly #reader: MeshFileReader;
  #disposed = false;

  constructor(fetchBytes: FetchBytes) {
    this.#reader = new MeshFileReader(fetchBytes);
  }

  async index(url: string): Promise<FormatRead<MeshIndex>> {
    const reply = await this.#reader.answer({ kind: "index", id: 0, url });
    return this.#disposed || reply?.kind !== "index"
      ? refused("the mesh page producer is stopped")
      : reply.read;
  }

  async page(url: string, entry: MeshPageEntry, page: number): Promise<FormatRead<MeshPage>> {
    const reply = await this.#reader.answer({ kind: "page", id: 0, url, entry, page });
    return this.#disposed || reply?.kind !== "page"
      ? refused("the mesh page producer is stopped")
      : reply.read;
  }

  forget(url: string): void {
    void this.#reader.answer({ kind: "forget", url }).catch((error: unknown) => {
      console.warn(`the mesh file ${url} could not be forgotten`, error);
    });
  }

  dispose(): void {
    this.#disposed = true;
  }
}
