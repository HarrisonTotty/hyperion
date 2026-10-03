import { describe, expect, it } from "vitest";

import { decodedSky, skyPayload } from "../../test/skyFixtures";
import { decodeSkyPayload, type SkyDecodeReply, type SkyDecodeRequest } from "./decodePayload";
import { createWorkerSkyDecoder, type SkyDecodeJob, type SkyDecodeWorker } from "./useSky";

/** A stand-in for the decode worker: it decodes on this thread a microtask later, or fails. */
class FakeSkyWorker implements SkyDecodeWorker {
  readonly posted: Array<{
    readonly message: SkyDecodeRequest;
    readonly transfer: Transferable[];
  }> = [];
  terminated = false;
  readonly #events = new EventTarget();
  readonly #answer: "decode" | "error" | "throw";

  constructor(answer: "decode" | "error" | "throw") {
    this.#answer = answer;
  }

  addEventListener(type: "message", listener: (event: MessageEvent<SkyDecodeReply>) => void): void;
  addEventListener(type: "error" | "messageerror", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error" | "messageerror",
    listener: ((event: MessageEvent<SkyDecodeReply>) => void) | ((event: Event) => void),
  ): void {
    this.#events.addEventListener(type, { handleEvent: listener });
  }

  postMessage(message: SkyDecodeRequest, transfer: Transferable[]): void {
    if (this.#answer === "throw") {
      throw new Error("could not clone the payload");
    }
    this.posted.push({ message, transfer });
    const answer = this.#answer;
    queueMicrotask(() => {
      if (answer === "error") {
        this.#events.dispatchEvent(new ErrorEvent("error", { message: "worker script failed" }));
      } else {
        this.#events.dispatchEvent(
          new MessageEvent("message", { data: decodeSkyPayload(message) }),
        );
      }
    });
  }

  terminate(): void {
    this.terminated = true;
  }
}

const STARS = [{ direction: [1, 0, 0], distanceLy: 8.6, vMag: -1.46 }] as const;

/** A payload of one star and its decode job, in two chunks. */
function job(): { readonly payload: Uint8Array; readonly job: SkyDecodeJob } {
  const payload = skyPayload(STARS, 2, 6.6);
  return {
    payload,
    job: {
      chunks: [payload.slice(0, 20), payload.slice(20)],
      starsBytes: 24,
      bandBytes: payload.byteLength - 24,
    },
  };
}

describe("createWorkerSkyDecoder", () => {
  it("numbers each decode, transfers its chunks and matches the reply by number", async () => {
    const worker = new FakeSkyWorker("decode");
    const decoder = createWorkerSkyDecoder(() => worker);
    const first = job();
    const replies = await Promise.all([decoder.decode(first.job), decoder.decode(job().job)]);
    expect(replies.map((reply) => reply.id)).toEqual([1, 2]);
    expect(worker.posted[0]?.transfer).toHaveLength(2);
    const [reply] = replies;
    if (!reply.ok) {
      throw new Error("the first decode failed");
    }
    expect(reply.stars).toEqual(decodedSky(first.payload, 1).stars);
  });

  it("settles a decode waiting when the worker fails, with its reason", async () => {
    const decoder = createWorkerSkyDecoder(() => new FakeSkyWorker("error"));
    await expect(decoder.decode(job().job)).resolves.toEqual({
      id: 1,
      ok: false,
      message: "worker script failed",
    });
  });

  it("settles a decode the worker could not take, with the reason", async () => {
    const decoder = createWorkerSkyDecoder(() => new FakeSkyWorker("throw"));
    await expect(decoder.decode(job().job)).resolves.toEqual({
      id: 1,
      ok: false,
      message: "could not clone the payload",
    });
  });

  it("terminates the worker when disposed", () => {
    const worker = new FakeSkyWorker("decode");
    createWorkerSkyDecoder(() => worker).dispose();
    expect(worker.terminated).toBe(true);
  });
});
