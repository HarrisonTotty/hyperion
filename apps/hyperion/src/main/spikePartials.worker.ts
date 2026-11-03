/**
 * The descent spike's finished segments, written on a worker thread of the main process (R14's Mac
 * measurements; `spikePartials.ts`): its messages are the main process's own, each part checked
 * again as the IPC handler checked it.
 */

import { parentPort, workerData } from "node:worker_threads";

import {
  type PartialsReply,
  partialsSink,
  SpikePartials,
  type SpikePartialsStart,
} from "./spikePartials";
import { readSpikeReportPart } from "./spikeReport";

/** Whether `value` is an object, so that its fields can be read. */
function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null;
}

/**
 * The start the main process gave the worker (`spikePartialsWorker.ts`): its directory and name
 * checked, its run description its own, cloned across.
 */
function startOf(value: unknown): SpikePartialsStart {
  if (
    !isRecord(value) ||
    typeof value["outDir"] !== "string" ||
    typeof value["name"] !== "string" ||
    !isRecord(value["run"])
  ) {
    throw new Error("the finished segments' worker was started without its run");
  }
  // A trust boundary: the main process's own description of the run (`SpikeSession`), cloned
  // across to this thread, whose fields its results' schema checks again as they are written.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  const run = value["run"] as unknown as SpikePartialsStart["run"];
  return { outDir: value["outDir"], name: value["name"], run };
}

const port = parentPort;
if (port === null) {
  throw new Error("the finished segments' writer runs on a worker thread");
}
const reply = (message: PartialsReply): void => {
  port.postMessage(message);
};
const sink = partialsSink(new SpikePartials(startOf(workerData)), (line) => {
  reply({ kind: "log", line });
});
port.on("message", (message: unknown) => {
  if (!isRecord(message)) {
    return;
  }
  if (message["kind"] === "part") {
    const part = readSpikeReportPart(message["part"]);
    if (part === null) {
      reply({ kind: "log", line: "descent spike: the finished segments' worker refused a part" });
      return;
    }
    sink.add(part);
  } else if (message["kind"] === "close") {
    void sink.close(message["written"] === true).then(() => {
      reply({ kind: "closed" });
      return undefined;
    });
  }
});
