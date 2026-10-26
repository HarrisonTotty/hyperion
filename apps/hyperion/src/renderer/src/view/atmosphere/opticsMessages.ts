/**
 * The messages between the render thread's optics pool (`opticsPool.ts`) and its workers
 * (`optics.worker.ts`), and the buffers each transfers (plan R08, R08.T10.a; Design notes 1 and
 * 14).
 *
 * @remarks
 * A light module: beside type imports it holds only {@link transferablesOf}, so that the render
 * thread's pool takes nothing of the optics' tables. Each request carries the body's input
 * whole, so that any worker can serve it from its own k-tables and its own assembly
 * (`opticsJobs.ts`); a join's request carries the shares' sums, transferred from the render
 * thread, which received them transferred.
 */

import type { AbsorberCurve, AbsorberCurveSums, CurveShare } from "./absorbers";
import type { AssembledMedium } from "./assemble";
import type { BodyAtmosphereInput } from "./atmosphereInput";

/**
 * What the pool asks of an optics worker.
 *
 * - `assemble`: the body's medium and labels (`assemble.ts`'s `assembleMedium`), with no curves.
 * - `curve-sums`: one share of the curves of the medium's absorbers under the lights in `slots`
 *   (`absorbers.ts`'s `absorberCurveSums`; `decision-r08-t4d-kdist-chain.md` §15, ruling 3).
 * - `curve-join`: the curves under the lights in `slots` from every share's sums
 *   (`absorberCurvesFromSums`), `sums[share][absorber]`.
 *
 * `slots` index the input's lights, which are its light slots in `lightsAt`'s order.
 */
export type OpticsRequest =
  | { readonly kind: "assemble"; readonly id: number; readonly input: BodyAtmosphereInput }
  | {
      readonly kind: "curve-sums";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly share: CurveShare;
    }
  | {
      readonly kind: "curve-join";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly sums: ReadonlyArray<ReadonlyArray<AbsorberCurveSums>>;
    };

/**
 * What an optics worker answers, by its request's `id`.
 *
 * - `assembled`: the medium and its labels, its curves empty.
 * - `curve-sums`: the share's sums, one an absorber in the medium's absorber terms' order; none for
 *   a body that draws no medium or has no absorber.
 * - `curves`: the light slots' channel curves, `curves[slot asked][absorber]`.
 * - `failed`: the job threw, a bug (`decision-r08-t10a-leans.md` item 6), with its message.
 */
export type OpticsReply =
  | { readonly kind: "assembled"; readonly id: number; readonly assembled: AssembledMedium }
  | {
      readonly kind: "curve-sums";
      readonly id: number;
      readonly sums: ReadonlyArray<AbsorberCurveSums>;
    }
  | {
      readonly kind: "curves";
      readonly id: number;
      readonly curves: ReadonlyArray<ReadonlyArray<AbsorberCurve>>;
    }
  | { readonly kind: "failed"; readonly id: number; readonly message: string };

/** A worker's reply and the buffers it transfers with it. */
export interface OpticsAnswer {
  readonly reply: OpticsReply;
  readonly transfer: ArrayBuffer[];
}

/**
 * Every distinct `ArrayBuffer` reachable from `value`, through objects, arrays, maps, sets and
 * typed arrays' buffers, each once, for a message's transfer list.
 *
 * @remarks
 * A `SharedArrayBuffer` is left out, since it is shared and not transferred. Listing a buffer
 * twice would make `postMessage` throw, so each is listed once however many arrays view it. The
 * walk is iterative, so a long array of objects cannot overflow the stack, and each object is
 * visited once, so a cycle ends it.
 */
export function transferablesOf(value: unknown): ArrayBuffer[] {
  const found = new Set<ArrayBuffer>();
  const seen = new WeakSet<object>();
  const stack: unknown[] = [value];
  while (stack.length > 0) {
    const item = stack.pop();
    if (typeof item !== "object" || item === null || seen.has(item)) {
      continue;
    }
    seen.add(item);
    if (item instanceof ArrayBuffer) {
      found.add(item);
    } else if (ArrayBuffer.isView(item)) {
      if (item.buffer instanceof ArrayBuffer) {
        found.add(item.buffer);
      }
    } else if (item instanceof Map) {
      for (const [key, member] of item) {
        stack.push(key, member);
      }
    } else if (item instanceof Set) {
      for (const member of item) {
        stack.push(member);
      }
    } else {
      stack.push(...Object.values(item));
    }
  }
  return [...found];
}
