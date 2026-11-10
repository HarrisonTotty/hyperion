/**
 * The sky bake's solid-angles worker (lane perf-long-tasks-2): forms one face size's texel solid
 * angles off the page's thread, ahead of the first bake, and posts the table back transferred.
 *
 * @remarks
 * It holds no logic of its own: {@link solidAnglesReply} answers each message. No file imports
 * this module; `bake.ts`'s `warmTexelSolidAngles` starts it with
 * `new Worker(new URL("./solidAngles.worker.ts", import.meta.url))`. It is typed with the worker's
 * library (`tsconfig.worker.json`), so `addEventListener` and `postMessage` are the dedicated
 * worker scope's.
 */

import { solidAnglesReply } from "./solidAnglesJob";

addEventListener("message", (event: MessageEvent<unknown>) => {
  const angles = solidAnglesReply(event.data);
  postMessage(angles, angles === null ? [] : [angles.buffer]);
});
