/**
 * The thick cases' worker (plan R08, R08.T12.b.2): builds `thickCases.ts`'s four cases, about a
 * minute of Mie and mean-field optics, off the render thread, for `ATMOSPHERE TEST`'s thick
 * bodies.
 *
 * @remarks
 * A same-origin module worker, started by `atmosphereScene.ts`'s `startThickCasesWorker` with
 * `new Worker(new URL("../atmosphere/reference/thickCases.worker.ts", import.meta.url),
 * { type: "module" })`. It builds the cases as it starts and posts one `ThickCasesReply`
 * (`thickCasesReplyOf`), each case's medium, figure, curves and sources with their buffers
 * transferred, or `failed` with the build's message; the render thread then ends it.
 *
 * It is typed with the worker's library, not the DOM's (`tsconfig.worker.json`), whose list holds
 * its whole import graph, free of the engine and the DOM.
 */

import { transferablesOf } from "../opticsMessages";
import { thickReferenceCases } from "./thickCases";
import { thickCasesReplyOf } from "./thickCasesMessages";

const reply = thickCasesReplyOf(thickReferenceCases);
postMessage(reply, transferablesOf(reply));
