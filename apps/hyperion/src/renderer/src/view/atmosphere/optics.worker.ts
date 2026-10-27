/**
 * An optics worker: assembles bodies' media, shares their curves of growth and bakes thick
 * atmospheres' multiple-scattering tables off the render thread (plan R08, R08.T10.a and
 * R08.T14.b; Design notes 1 and 9), for the render thread's pool (`opticsPool.ts`).
 *
 * @remarks
 * A same-origin module worker, started by `opticsPool.ts`'s `startOpticsWorker` with
 * `new Worker(new URL("./optics.worker.ts", import.meta.url), { type: "module" })`. It holds no
 * logic of its own: {@link OpticsJobs} answers each request, reading the committed k-distribution
 * parts through `fetchPart` (their `?url` assets, as the height worker reads its WebAssembly).
 *
 * A part that cannot be read is raised as the worker's error (`reportError`), so that the pool
 * replaces the worker and asks again, giving its place up after three failures in a row; a job
 * that throws is answered `failed` by {@link OpticsJobs.answer} itself.
 *
 * It is typed with the worker's library, not the DOM's (`tsconfig.worker.json`), whose list holds
 * its whole import graph, free of the engine and the DOM.
 */

import { fetchPart, loadKDistributions } from "./absorbers/kDistribution";
import { OpticsJobs } from "./opticsJobs";
import type { OpticsRequest } from "./opticsMessages";

const jobs = new OpticsJobs({
  loadKDistributions: async (species) => loadKDistributions(species, fetchPart),
});

/** Answers one request, its reply's buffers transferred. */
async function answer(request: OpticsRequest): Promise<void> {
  const { reply, transfer } = await jobs.answer(request);
  postMessage(reply, transfer);
}

// The message is a structured clone of an `OpticsRequest` that our own render thread posted, from
// a same-origin page, so its type is trusted rather than checked.
addEventListener("message", (event: MessageEvent<OpticsRequest>) => {
  void answer(event.data).catch((error: unknown) => {
    // A k-table that could not be read: the worker's error, so that the pool replaces it rather
    // than waiting on an answer that will not come.
    reportError(error instanceof Error ? error : new Error(String(error), { cause: error }));
  });
});
