/**
 * The server link's socket worker (fix-night-sky-stars): reads the link's `WebSocket` off the
 * page's thread and posts its frames on.
 *
 * @remarks
 * It holds no logic of its own: {@link serveLinkSocket} serves the page's commands. No file
 * imports this module; `linkSocket.ts` starts it with
 * `new Worker(new URL("./linkSocket.worker.ts", import.meta.url))`. It is typed with the worker's
 * library (`tsconfig.worker.json`), so `addEventListener` and `postMessage` are the dedicated
 * worker scope's.
 */

import { serveLinkSocket } from "./linkSocketWorker";

serveLinkSocket({
  addEventListener: (type, listener) => {
    addEventListener(type, listener);
  },
  postMessage: (report, transfer) => {
    postMessage(report, transfer);
  },
});
