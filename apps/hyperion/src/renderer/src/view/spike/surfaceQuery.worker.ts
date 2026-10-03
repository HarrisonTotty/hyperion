/**
 * The spike's surface query worker (plan R05, T13.b): its own instance of the surface crate's
 * module, answering `surfaceQuery.ts`'s questions.
 *
 * @remarks
 * A same-origin module worker, loaded as `../terrain/workers/height.worker.ts` loads its module
 * (R04.T10.a: the render thread cannot compile WebAssembly). A load failure, or a module that bakes
 * another test planet, is raised as the worker's error, which fails the client's questions.
 *
 * It is typed with the worker's library, not the DOM's (`tsconfig.worker.json`).
 */

import init, {
  bakePatch,
  levelTable,
  surfaceHeightM,
  testPlanetVersion,
} from "../../generated/surface/hyperion_surface";
import wasmUrl from "../../generated/surface/hyperion_surface_bg.wasm?url";
import { describeLoadFailure } from "../../wasm/handleRequest";
import { staleModuleMessage } from "../terrain/workers/heightBake";
import {
  answerSurfaceQuery,
  type SurfaceQueryModule,
  type SurfaceQueryRequest,
} from "./surfaceQuery";

const module: SurfaceQueryModule = { bakePatch, levelTable, surfaceHeightM };

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
  const stale = staleModuleMessage({ bakePatch, testPlanetVersion });
  return stale === null ? null : new Error(stale);
}

const loaded = load();

async function answer(request: SurfaceQueryRequest): Promise<void> {
  const failure = await loaded;
  if (failure !== null) {
    reportError(failure);
    return;
  }
  postMessage(answerSurfaceQuery(module, request));
}

// The message is a structured clone of a `SurfaceQueryRequest` that our own render thread posted,
// from a same-origin page, so its type is trusted rather than checked.
addEventListener("message", (event: MessageEvent<SurfaceQueryRequest>) => {
  void answer(event.data).catch((error: unknown) => {
    reportError(error instanceof Error ? error : new Error(String(error), { cause: error }));
  });
});
