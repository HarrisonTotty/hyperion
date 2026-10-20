import { vec3 } from "../geometry/vec3";
import type { PatchKey } from "../view/terrain/patchKey";
import { StandInFields } from "../view/terrain/workers/heightBake";
import type {
  BakedPatch,
  HeightWorkerReply,
  HeightWorkerRequest,
} from "../view/terrain/workers/messages";
import type { WorkerLike } from "../view/terrain/workers/pool";

/** A field message as a height worker receives it. */
export type FieldMessage = Extract<
  HeightWorkerRequest,
  { kind: "field-init" | "field-chunk" | "field-release" }
>;

/** A bake of `key` for `generation`, with placeholder arrays, as a scripted worker answers. */
export function bakeOf(key: PatchKey, generation: number): BakedPatch {
  return {
    key,
    generation,
    originM: vec3(0, 0, 0),
    heights: new Float32Array(2),
    offsets: null,
    normals: new Float16Array(2),
    heightRangeM: [0, 0],
    boundingRadiusM: 1,
    originHeightM: 0,
    skirtDepthM: 1,
    layers: null,
  };
}

/**
 * A height worker driven by the test (plan R05, T10.a; moved here from `pool.test.ts` for R10.T7's
 * store): it records what it is sent and answers when told, its field messages through the real
 * worker's {@link StandInFields}, so that what it reports holding is each block's bytes, decoding
 * none (the real worker's module decodes a payload's: `fieldsWasm.test.ts`).
 */
export class ScriptedHeightWorker implements WorkerLike {
  private readonly events = new EventTarget();
  private readonly wrappers = new Map<unknown, EventListenerObject>();
  private readonly answered: number[] = [];
  /** Listeners added and not yet removed, by event type. */
  readonly listening = { message: 0, error: 0 };
  readonly received: HeightWorkerRequest[] = [];
  /** The fields as the worker would hold them, once {@link ScriptedHeightWorker.answerFields} runs. */
  readonly fields = new StandInFields();
  terminated = false;

  postMessage(message: HeightWorkerRequest): void {
    this.received.push(message);
  }

  terminate(): void {
    this.terminated = true;
  }

  addEventListener(
    type: "message",
    listener: (event: MessageEvent<HeightWorkerReply>) => void,
  ): void;
  addEventListener(type: "error", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<HeightWorkerReply>) => void) | ((event: Event) => void),
  ): void {
    this.listening[type] += 1;
    const wrapper = { handleEvent: listener };
    this.wrappers.set(listener, wrapper);
    this.events.addEventListener(type, wrapper);
  }

  removeEventListener(
    type: "message",
    listener: (event: MessageEvent<HeightWorkerReply>) => void,
  ): void;
  removeEventListener(type: "error", listener: (event: Event) => void): void;
  removeEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<HeightWorkerReply>) => void) | ((event: Event) => void),
  ): void {
    const wrapper = this.wrappers.get(listener);
    if (wrapper !== undefined) {
      this.listening[type] -= 1;
      this.wrappers.delete(listener);
      this.events.removeEventListener(type, wrapper);
    }
  }

  /** The bake requests still awaiting an answer, oldest first. */
  get pendingBakes(): Extract<HeightWorkerRequest, { kind: "bake" }>[] {
    const answered = new Set(this.answered);
    return this.received.flatMap((m) => (m.kind === "bake" && !answered.has(m.id) ? [m] : []));
  }

  /** The field messages received, oldest first. */
  get fieldMessages(): FieldMessage[] {
    return this.received.flatMap((m) =>
      m.kind === "field-init" || m.kind === "field-chunk" || m.kind === "field-release" ? [m] : [],
    );
  }

  /** The class-map queries still awaiting an answer, oldest first (R10.T10.d). */
  get pendingClassMaps(): Extract<HeightWorkerRequest, { kind: "class-map" }>[] {
    const answered = new Set(this.answered);
    return this.received.flatMap((m) => (m.kind === "class-map" && !answered.has(m.id) ? [m] : []));
  }

  /** The ground-readout queries still awaiting an answer, oldest first (R10.T12). */
  get pendingGroundReadouts(): Extract<HeightWorkerRequest, { kind: "ground-readout" }>[] {
    const answered = new Set(this.answered);
    return this.received.flatMap((m) =>
      m.kind === "ground-readout" && !answered.has(m.id) ? [m] : [],
    );
  }

  reply(message: HeightWorkerReply): void {
    this.answered.push(message.id);
    this.events.dispatchEvent(new MessageEvent("message", { data: message }));
  }

  /** Answers the oldest pending bake with a bake of its key. */
  bakeOldest(): void {
    const next = this.pendingBakes[0];
    if (next === undefined) {
      throw new Error("no bake is pending");
    }
    this.reply({ kind: "baked", id: next.id, bake: bakeOf(next.key, next.generation) });
  }

  /** Answers every field message not yet answered, in order, as the real worker would. */
  answerFields(): void {
    const answered = new Set(this.answered);
    for (const message of this.fieldMessages) {
      if (answered.has(message.id)) {
        continue;
      }
      let refused = false;
      switch (message.kind) {
        case "field-init":
          this.fields.init(message.body, message.block);
          break;
        case "field-chunk":
          refused = !this.fields.chunk(message.body, message.block);
          break;
        case "field-release":
          this.fields.release(message.body);
          break;
      }
      this.reply(
        refused
          ? {
              kind: "field-refused",
              id: message.id,
              body: message.body,
              message: "refused",
              heldBytes: this.fields.heldBytes,
            }
          : {
              kind: "field-held",
              id: message.id,
              body: message.body,
              heldBytes: this.fields.heldBytes,
              block: null,
            },
      );
    }
  }

  fail(): void {
    this.events.dispatchEvent(new Event("error"));
  }
}
