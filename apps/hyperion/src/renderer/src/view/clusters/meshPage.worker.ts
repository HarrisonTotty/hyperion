/**
 * The mesh page producer's worker (plan R14, R14.T5.b, Design note 14): fetches mesh files from
 * the app's own assets and reads their indexes and pages off the render thread, posting each page
 * back with its buffers transferred.
 *
 * @remarks
 * It holds no logic of its own: {@link MeshFileReader} answers each request. No file imports this
 * module; `meshPageProducer.ts` starts it with
 * `new Worker(new URL("./meshPage.worker.ts", import.meta.url), { type: "module" })`. It is typed
 * with the worker's library (`tsconfig.worker.json`).
 */

import {
  fetchAssetBytes,
  MeshFileReader,
  type MeshPageRequest,
  meshPageTransferables,
} from "./meshPageWork";

const reader = new MeshFileReader(fetchAssetBytes);

/** Answers `request`, its page's buffers transferred. */
async function answer(request: MeshPageRequest): Promise<void> {
  const reply = await reader.answer(request);
  if (reply !== null) {
    postMessage(reply, meshPageTransferables(reply));
  }
}

// The message is a structured clone of a `MeshPageRequest` that our own render thread posted,
// from a same-origin page, so its type is trusted rather than checked.
addEventListener("message", (event: MessageEvent<MeshPageRequest>) => {
  void answer(event.data).catch((error: unknown) => {
    // Answering threw outside the reader's refusals: the worker's error, so that the render thread
    // refuses every request waiting and every later one.
    reportError(error instanceof Error ? error : new Error(String(error), { cause: error }));
  });
});
