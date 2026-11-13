/**
 * The mesh page producer (plan R14, R14.T5.b): the worker's reader of fetched files, which
 * fetches each file once and refuses rather than throws, and the render thread's client of the
 * worker, which matches replies by number and settles every request when the worker fails or is
 * stopped.
 */

import { describe, expect, it, vi } from "vitest";

import { threeLevelMesh, writeMeshFile } from "../../test/meshPages";
import { createWorkerMeshPageProducer, type MeshPageWorker } from "./meshPageProducer";
import {
  fetchAssetBytes,
  MeshFileReader,
  type MeshPageReply,
  type MeshPageRequest,
  meshPageTransferables,
} from "./meshPageWork";
import { type MeshPageEntry, readMeshIndex } from "./pageFormat";

const URL = "mesh:three-level";

/** A stand-in for the producer's worker: it answers on this thread a macrotask later, or fails. */
class FakeMeshPageWorker implements MeshPageWorker {
  readonly posted: MeshPageRequest[] = [];
  terminated = false;
  readonly #events = new EventTarget();
  readonly #reader: MeshFileReader;
  readonly #fail: boolean;

  constructor(fail = false) {
    this.#fail = fail;
    this.#reader = new MeshFileReader(() => Promise.resolve(writeMeshFile(threeLevelMesh())));
  }

  addEventListener(type: "message", listener: (event: MessageEvent<MeshPageReply>) => void): void;
  addEventListener(type: "error" | "messageerror", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error" | "messageerror",
    listener: ((event: MessageEvent<MeshPageReply>) => void) | ((event: Event) => void),
  ): void {
    this.#events.addEventListener(type, { handleEvent: listener });
  }

  postMessage(message: MeshPageRequest, _transfer: Transferable[]): void {
    this.posted.push(message);
    if (this.#fail) {
      // A worker that fails raises its error once, and answers nothing after.
      if (this.posted.length === 1) {
        queueMicrotask(() => {
          this.#events.dispatchEvent(new ErrorEvent("error", { message: "worker script failed" }));
        });
      }
      return;
    }
    void this.#reply(message).catch((error: unknown) => {
      this.#events.dispatchEvent(new ErrorEvent("error", { message: String(error) }));
    });
  }

  /** Answers `message` as the worker would. */
  async #reply(message: MeshPageRequest): Promise<void> {
    const reply = await this.#reader.answer(message);
    if (reply !== null) {
      this.#events.dispatchEvent(new MessageEvent("message", { data: reply }));
    }
  }

  terminate(): void {
    this.terminated = true;
  }
}

/** The three-level DAG's page table entry `page`, read by `reader`. */
async function entryOf(reader: MeshFileReader, page: number): Promise<MeshPageEntry> {
  const index = await reader.answer({ kind: "index", id: 0, url: URL });
  const entry =
    index?.kind === "index" && index.read.kind === "read"
      ? index.read.value.pages[page]
      : undefined;
  if (entry === undefined) {
    throw new Error("no index");
  }
  return entry;
}

describe("the mesh page worker's reader (plan R14, R14.T5.b)", () => {
  it("fetches a file once for its index and pages, and again once forgotten", async () => {
    const fetched: string[] = [];
    const reader = new MeshFileReader((url) => {
      fetched.push(url);
      return Promise.resolve(writeMeshFile(threeLevelMesh()));
    });
    await reader.answer({
      kind: "page",
      id: 2,
      url: URL,
      entry: await entryOf(reader, 3),
      page: 3,
    });
    await reader.answer({ kind: "forget", url: URL });
    await reader.answer({ kind: "index", id: 3, url: URL });
    expect(fetched).toEqual([URL, URL]);
  });

  it("answers a page with its buffers to transfer: its bytes and its two group tables", async () => {
    const reader = new MeshFileReader(() => Promise.resolve(writeMeshFile(threeLevelMesh())));
    const page = await reader.answer({
      kind: "page",
      id: 2,
      url: URL,
      entry: await entryOf(reader, 3),
      page: 3,
    });
    expect([
      page?.kind === "page" && page.read.kind === "read" ? page.read.value.clusters : null,
      page === null ? 0 : meshPageTransferables(page).length,
    ]).toEqual([4, 3]);
  });

  it("refuses a file that cannot be fetched, and fetches it again at the next request", async () => {
    let fail = true;
    const reader = new MeshFileReader(() =>
      fail
        ? Promise.reject(new Error("404 Not Found"))
        : Promise.resolve(writeMeshFile(threeLevelMesh())),
    );
    const refused = await reader.answer({ kind: "index", id: 1, url: URL });
    fail = false;
    const index = await reader.answer({ kind: "index", id: 2, url: URL });
    expect([refused, index?.kind === "index" ? index.read.kind : null]).toEqual([
      { kind: "index", id: 1, read: { kind: "refused", reason: "404 Not Found" } },
      "read",
    ]);
  });

  it("refuses a page past its file's bytes", async () => {
    const reader = new MeshFileReader(() => Promise.resolve(writeMeshFile(threeLevelMesh())));
    const entry = await entryOf(reader, 0);
    const past = await reader.answer({
      kind: "page",
      id: 3,
      url: URL,
      entry: { ...entry, offsetBytes: 1 << 24 },
      page: 0,
    });
    expect(past?.kind === "page" ? past.read.kind : null).toBe("refused");
  });

  it("fetches an asset's bytes, and rejects a status other than OK", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) =>
        Promise.resolve(
          url === "ok"
            ? new Response(new Uint8Array([1, 2, 3]))
            : new Response(null, { status: 404, statusText: "Not Found" }),
        ),
      ),
    );
    expect([(await fetchAssetBytes("ok")).byteLength]).toEqual([3]);
    await expect(fetchAssetBytes("missing")).rejects.toThrow("missing: 404 Not Found");
  });
});

describe("the mesh page producer over its worker (plan R14, R14.T5.b)", () => {
  it("answers an index and a page through the worker, matched by number", async () => {
    const worker = new FakeMeshPageWorker();
    const producer = createWorkerMeshPageProducer(() => worker);
    const index = await producer.index(URL);
    const entry = index.kind === "read" ? index.value.pages[5] : undefined;
    if (entry === undefined) {
      throw new Error("no index");
    }
    const page = await producer.page(URL, entry, 5);
    const local = readMeshIndex(writeMeshFile(threeLevelMesh()));
    expect([
      index.kind === "read" && local.kind === "read" ? index.value.groups : null,
      page.kind === "read" ? [...page.value.consumingGroups] : page,
      worker.posted.map((m) => (m.kind === "forget" ? m.kind : [m.kind, m.id])),
    ]).toEqual([
      local.kind === "read" ? local.value.groups : "local",
      [3],
      [
        ["index", 1],
        ["page", 2],
      ],
    ]);
  });

  it("asks its worker to forget a file", () => {
    const worker = new FakeMeshPageWorker();
    createWorkerMeshPageProducer(() => worker).forget(URL);
    expect(worker.posted).toEqual([{ kind: "forget", url: URL }]);
  });

  it("terminates its worker when disposed, and refuses every request after", async () => {
    const worker = new FakeMeshPageWorker();
    const producer = createWorkerMeshPageProducer(() => worker);
    producer.dispose();
    expect([worker.terminated, await producer.index(URL)]).toEqual([
      true,
      { kind: "refused", reason: "the mesh page producer is stopped" },
    ]);
  });

  it("refuses every request waiting when its worker fails, and every request after", async () => {
    const producer = createWorkerMeshPageProducer(() => new FakeMeshPageWorker(true));
    const refused = { kind: "refused", reason: "worker script failed" };
    expect([await producer.index(URL), await producer.index(URL)]).toEqual([refused, refused]);
  });
});
