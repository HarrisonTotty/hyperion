/**
 * A height worker: loads its own instance of the surface crate's WebAssembly module and bakes the
 * patches the render thread's pool asks for (plan R05, T10.b, Design note 11).
 *
 * @remarks
 * A same-origin module worker, started by the pool's factory with
 * `new Worker(new URL("./height.worker.ts", import.meta.url), { type: "module" })`, after R04's
 * probe worker (`wasm/surface.worker.ts`): served from `file://` or the dev server it has no
 * Content Security Policy of its own, so it compiles WebAssembly under the page's policy
 * unchanged, which the render thread cannot (R04.T10.a's ruling). The module's file is a `?url`
 * import, a hashed asset the compile streams from. Its logic is `heightBake.ts`'s.
 *
 * A load failure, or a module that bakes another test planet than this client's, is raised as the
 * worker's error (`reportError`), described by R04's `describeLoadFailure` so that a policy
 * refusal reads `csp-refused`: the pool then replaces the worker and re-queues its requests, and
 * gives its place up after three failures. Requests that arrive while the module loads wait for
 * it. The module's generator version is checked against the server's by R04's own loader, which
 * loads the same module file (`loadSurfaceModule`); the test planet belongs to no universe, so the
 * height worker checks its `testPlanetVersion` instead.
 *
 * It is typed with the worker's library, not the DOM's (`tsconfig.worker.json`).
 */

import init, { bakePatch, testPlanetVersion } from "../../../generated/surface/hyperion_surface";
import wasmUrl from "../../../generated/surface/hyperion_surface_bg.wasm?url";
import { describeLoadFailure } from "../../../wasm/handleRequest";
import { answerRequest, type HeightModule, staleModuleMessage } from "./heightBake";
import type { HeightWorkerRequest } from "./messages";

const module: HeightModule = { bakePatch, testPlanetVersion };

/** Loads the module, once; `null` once it can serve, or the error saying why it cannot. */
async function load(): Promise<Error | null> {
  try {
    await init({ module_or_path: wasmUrl });
  } catch (error: unknown) {
    const cause = describeLoadFailure(error);
    return new Error(`the surface module did not load (${cause.kind}): ${cause.message}`, {
      cause: error,
    });
  }
  const stale = staleModuleMessage(module);
  return stale === null ? null : new Error(stale);
}

const loaded = load();

/**
 * The coarse field the pool posted, held for the worker's life (Design note 11's 15 MB a worker
 * at rest); the test planet does not read it, R09 copies it into the module.
 */
const held: { field: ArrayBuffer | null } = { field: null };

function holdField(bytes: ArrayBuffer): void {
  held.field = bytes;
}

/** Answers one request once the module's load has settled. */
async function answer(request: HeightWorkerRequest): Promise<void> {
  const failure = await loaded;
  if (failure !== null) {
    reportError(failure);
    return;
  }
  const { reply, transfer } = answerRequest(module, request, holdField);
  postMessage(reply, transfer);
}

// The message is a structured clone of a `HeightWorkerRequest` that our own render thread posted,
// from a same-origin page, so its type is trusted rather than checked.
addEventListener("message", (event: MessageEvent<HeightWorkerRequest>) => {
  void answer(event.data).catch((error: unknown) => {
    // Answering threw outside a bake (a trap in the module, say): the worker's error, so that the
    // pool replaces it rather than waiting on an answer that will not come.
    reportError(error instanceof Error ? error : new Error(String(error), { cause: error }));
  });
});
