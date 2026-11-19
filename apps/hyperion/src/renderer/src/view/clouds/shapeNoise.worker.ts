/**
 * The cloud volume's shape-noise worker (plan R11, R11.T7.c): makes a world's three shape noises
 * (`cloudShapeNoise.ts`) off the page's thread, about 1.3 s of work a world (pod, 2026-11-19),
 * which on the page's thread would take frames' budgets for many seconds.
 *
 * @remarks
 * It holds no logic of its own: `serveShapeNoises` answers the page's requests. No file imports this
 * module; `cloudVolumePass.ts` starts it with
 * `new Worker(new URL("./shapeNoise.worker.ts", import.meta.url), { type: "module" })`. It is typed
 * with the worker's library (`tsconfig.worker.json`).
 */

import { serveShapeNoises } from "./cloudShapeNoise";

serveShapeNoises({
  addEventListener: (type, listener) => {
    addEventListener(type, listener);
  },
  postMessage: (reply, transfer) => {
    postMessage(reply, transfer);
  },
});
