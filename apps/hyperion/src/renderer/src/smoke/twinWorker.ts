/**
 * The smoke page's twin worker: answers each {@link TwinRequest} with {@link replyTo}, so that the
 * atmosphere's CPU twins run beside the GPU's work instead of after it on the page's thread.
 *
 * @remarks
 * No file imports this module; `twinPool.ts` starts it with
 * `new Worker(new URL("./twinWorker.ts", import.meta.url), { type: "module" })`, a same-origin
 * module file as the height worker is.
 *
 * Unlike the client's workers it is not named `*.worker.ts` (accepted by "main", perf-smoke
 * 2026-10-13): it imports `tablesCpu.ts`, whose imports reach much of the renderer (230
 * files), and `tsconfig.worker.json`, a composite project, must list every file its workers
 * import. So the page's project (`tsconfig.web.json`) types it, under the DOM library, where `self`
 * is a window. The worker therefore reaches its scope only through {@link TwinWorkerScope}: the two
 * members it uses, with the signatures `DedicatedWorkerGlobalScope` gives them (a `postMessage`
 * with structured-serialize options, no target origin), to which the window's are assignable, so
 * that no cast is needed. The imported graph also typechecks under the WebWorker library alone
 * (`tsc` with `lib: ["es2023", "es2025.float16", "webworker"]` on this file: no error,
 * 2026-10-13), so none of it needs the DOM's types.
 */

import { replyTo, type TwinReply, type TwinRequest } from "./twinJobs";

/** The members of `DedicatedWorkerGlobalScope` this worker uses. */
interface TwinWorkerScope {
  addEventListener(type: "message", listener: (event: MessageEvent<TwinRequest>) => void): void;
  postMessage(message: TwinReply, options: StructuredSerializeOptions): void;
}

const scope: TwinWorkerScope = self;

scope.addEventListener("message", (event) => {
  // Nothing is transferred: the reply's tables are copied.
  scope.postMessage(replyTo(event.data), { transfer: [] });
});
