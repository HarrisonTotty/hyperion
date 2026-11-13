/**
 * The descent spike's finished segments written on a worker thread (R14's Mac measurements;
 * `spikePartials.ts`), so that building their results never holds the main process's thread, which
 * sets a hidden run's offscreen frames' pace. Imported by the launch alone: the worker's module is
 * electron-vite's to bundle (`?nodeWorker`), which no test resolves.
 */

// electron-vite's worker module (`?nodeWorker`), which `electron-vite/node`'s types declare and
// the import plugin cannot resolve.
// oxlint-disable-next-line import/default
import createPartialsWorker from "./spikePartials.worker?nodeWorker";

import type { PartialsReply, SpikePartialsSink, SpikePartialsStart } from "./spikePartials";

/** Whether `value` is one of the worker's replies. */
function isReply(value: unknown): value is PartialsReply {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const kind: unknown = Reflect.get(value, "kind");
  return kind === "closed" || (kind === "log" && typeof Reflect.get(value, "line") === "string");
}

/** A run's finished segments' sink on a worker thread, its lines and failures logged to `log`. */
export function workerPartials(
  start: SpikePartialsStart,
  log: (line: string) => void,
): SpikePartialsSink {
  const worker = createPartialsWorker({ workerData: start });
  const closed = new Promise<void>((resolve) => {
    worker.on("message", (message: unknown) => {
      if (!isReply(message)) {
        return;
      }
      if (message.kind === "log") {
        log(message.line);
      } else {
        resolve();
      }
    });
    // A worker that died writes nothing more; its run goes on.
    worker.once("exit", () => {
      resolve();
    });
  });
  worker.on("error", (error: Error) => {
    log(`descent spike: the finished segments' worker failed: ${error.message}`);
  });
  return {
    add: (part) => {
      // A Node worker's `postMessage` takes no target origin, which the rule asks of a window's.
      // oxlint-disable-next-line unicorn/require-post-message-target-origin
      worker.postMessage({ kind: "part", part });
    },
    close: async (written) => {
      // As in `add`: a Node worker's message, with no target origin.
      // oxlint-disable-next-line unicorn/require-post-message-target-origin
      worker.postMessage({ kind: "close", written });
      await closed;
      await worker.terminate();
    },
  };
}
