/**
 * What the mesh page producer's worker does (plan R14, R14.T5.b, Design note 14): fetches a mesh
 * file from the app's own assets, once a file, and answers each request with its index or one of
 * its pages, read and checked by `pageFormat.ts`.
 *
 * @remarks
 * The worker (`meshPage.worker.ts`) holds no logic of its own: it hands each request here with the
 * worker's `fetch`, and posts the reply with its page's buffers transferred
 * ({@link meshPageTransferables}). The render thread's side is `meshPageProducer.ts`. A fetch that
 * fails, or a file the format refuses, is a refusal in the reply, never a throw. Each file is held
 * whole in the worker from its first request until it is forgotten, when its last mesh is
 * unloaded (lean, 2026-11-02: a test mesh's file is 1–3 MB, and the GPU's pool is the budget
 * Design note 15 holds).
 */

import {
  type FormatRead,
  type MeshIndex,
  type MeshPage,
  type MeshPageEntry,
  readMeshIndex,
  readMeshPage,
} from "./pageFormat";

/** A message the render thread sends the producer's worker. */
export type MeshPageRequest =
  | { readonly kind: "index"; readonly id: number; readonly url: string }
  | {
      readonly kind: "page";
      readonly id: number;
      readonly url: string;
      readonly entry: MeshPageEntry;
      readonly page: number;
    }
  /** Drops the file's bytes; a later request fetches it again. */
  | { readonly kind: "forget"; readonly url: string };

/** The worker's answer to a request, matched by its `id`. */
export type MeshPageReply =
  | { readonly kind: "index"; readonly id: number; readonly read: FormatRead<MeshIndex> }
  | { readonly kind: "page"; readonly id: number; readonly read: FormatRead<MeshPage> };

/** How the worker fetches a file's bytes: its `fetch`, or a test's. */
export type FetchBytes = (url: string) => Promise<ArrayBuffer>;

/** The worker's `fetch` of an app asset's bytes; a status other than OK rejects. */
export async function fetchAssetBytes(url: string): Promise<ArrayBuffer> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`${url}: ${String(response.status)} ${response.statusText}`);
  }
  return response.arrayBuffer();
}

/** The buffers of `reply` that are posted transferred: a page's bytes and its group tables. */
export function meshPageTransferables(reply: MeshPageReply): Transferable[] {
  if (reply.kind === "index" || reply.read.kind === "refused") {
    return [];
  }
  const { body, generatingGroups, consumingGroups } = reply.read.value;
  return [body, generatingGroups.buffer, consumingGroups.buffer];
}

/** The producer's files and its answers (see the module's remarks). */
export class MeshFileReader {
  readonly #fetchBytes: FetchBytes;
  readonly #files = new Map<string, Promise<ArrayBuffer>>();

  constructor(fetchBytes: FetchBytes) {
    this.#fetchBytes = fetchBytes;
  }

  /** The reply to `request`, or `null` for one that has none. */
  async answer(request: MeshPageRequest): Promise<MeshPageReply | null> {
    if (request.kind === "forget") {
      this.#files.delete(request.url);
      return null;
    }
    const bytes = await this.#file(request.url);
    if (request.kind === "index") {
      return {
        kind: "index",
        id: request.id,
        read: bytes.kind === "read" ? readMeshIndex(bytes.value) : bytes,
      };
    }
    if (bytes.kind === "refused") {
      return { kind: "page", id: request.id, read: bytes };
    }
    const { offsetBytes, bytes: length } = request.entry;
    const read =
      offsetBytes + length <= bytes.value.byteLength
        ? readMeshPage(
            bytes.value.slice(offsetBytes, offsetBytes + length),
            request.entry,
            request.page,
          )
        : ({
            kind: "refused",
            reason: `page ${String(request.page)} lies past ${request.url}'s ${String(bytes.value.byteLength)} B`,
          } as const);
    return { kind: "page", id: request.id, read };
  }

  /** The file at `url`, fetched once, or why it could not be. */
  async #file(url: string): Promise<FormatRead<ArrayBuffer>> {
    let file = this.#files.get(url);
    if (file === undefined) {
      file = this.#fetchBytes(url);
      this.#files.set(url, file);
    }
    try {
      return { kind: "read", value: await file };
    } catch (error: unknown) {
      this.#files.delete(url);
      return {
        kind: "refused",
        reason: error instanceof Error ? error.message : `${url} could not be fetched`,
      };
    }
  }
}
