/**
 * The surface worker: loads the surface crate's WebAssembly module and answers the render thread.
 *
 * @remarks
 * A same-origin module worker, created by `loadSurfaceModule.ts` with
 * `new Worker(new URL("./surface.worker.ts", import.meta.url), { type: "module" })`. Served from
 * `file://` or the dev server it has no Content Security Policy of its own, so it compiles
 * WebAssembly under the page's policy unchanged, which the render thread cannot (R04 Design note 16
 * and T10.a's ruling). It is the one kind of renderer file that may import `generated/surface/`
 * (`surfaceImports.test.ts`). The module's file comes from a `?url` import, which Vite turns into
 * a hashed asset served as `application/wasm`, so the compile streams. Its logic is
 * `handleRequest`'s; R05's height workers extend it.
 *
 * It is typed with the worker's library, not the DOM's (`tsconfig.worker.json`), so
 * `addEventListener` and `postMessage` are the dedicated worker scope's.
 */

import init, { generatorVersion } from "../generated/surface/hyperion_surface";
import wasmUrl from "../generated/surface/hyperion_surface_bg.wasm?url";
import {
  describeLoadFailure,
  handleRequest,
  type SurfaceLoadFailure,
  type SurfaceReply,
  type SurfaceRequest,
} from "./handleRequest";

/** The module's state once its load has settled. */
type LoadState =
  { readonly kind: "loaded" } | { readonly kind: "failed"; readonly cause: SurfaceLoadFailure };

/** Loads the module, once, reporting a failure rather than throwing it. */
async function load(): Promise<LoadState> {
  try {
    await init({ module_or_path: wasmUrl });
    return { kind: "loaded" };
  } catch (error: unknown) {
    return { kind: "failed", cause: describeLoadFailure(error) };
  }
}

const loaded = load();

/** Answers one request once the module's load has settled. */
async function answer(request: SurfaceRequest): Promise<void> {
  const state = await loaded;
  let reply: SurfaceReply;
  switch (state.kind) {
    case "loaded":
      reply = handleRequest({ generatorVersion }, request);
      break;
    case "failed":
      reply = { kind: "load-failed", cause: state.cause };
      break;
  }
  postMessage(reply, []);
}

// The message is a structured clone of a `SurfaceRequest` that our own render thread posted, from
// a same-origin page, so its type is trusted rather than checked.
addEventListener("message", (event: MessageEvent<SurfaceRequest>) => {
  void answer(event.data).catch((error: unknown) => {
    // Answering threw (a trap in the module, say): the render thread is told at once, rather than
    // waiting out its timeout.
    const message = error instanceof Error ? error.message : String(error);
    const reply: SurfaceReply = { kind: "answer-failed", message };
    postMessage(reply, []);
  });
});
